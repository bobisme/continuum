//! The `bounds` field group: finite exploration bounds (PR-4 / IMPL-04).
//!
//! # What `bounds` *is*, in this crate
//!
//! Unlike every other PR-4 field group, `bounds` has no per-item unit key: RFC
//! 0031's field table classifies it as a single unit, "the tuple `(values, nodes,
//! faults, depth)`", not a set keyed by an `id`. [`Bounds`] is that tuple, and it
//! is immutable exactly as [`crate::property::Claim`] is: no `&mut` method, no
//! setter, a changed bound is a new [`Bounds`] with a new [`BoundsIdentity`].
//!
//! | Component | Type | Minimum | What `null`/absence means |
//! |---|---|---|---|
//! | `values` | [`ExplorationBound`] | 1 | `null` or absent: unbounded |
//! | `nodes` | [`DeclaredBound`] | 1 | no null form; absent: undeclared |
//! | `faults` | [`DeclaredBound`] | 0 | no null form; absent: undeclared |
//! | `depth` | [`ExplorationBound`] | 0 | `null` or absent: unbounded |
//!
//! # Two different kinds of "no value here", and why they get two types
//!
//! RFC 0037 draws a sharp line between the two nullable components and the two
//! undeclarable ones, and gives each a different failure mode:
//!
//! > In `bounds`, `null` denotes **unbounded** and is the top of that component's
//! > order. An absent `values` or `depth` key MUST be read as `null`. `nodes` and
//! > `faults` have no null form and no default: a change in the declaredness of
//! > either MUST classify `bounds` as `unknown` and fail closed.
//! >
//! > — RFC 0037, "Absence, null, and defaults"
//!
//! So `values`/`depth` have a genuine top element with a defined order relation to
//! every finite bound (unbounded is bigger than any `n`), while `nodes`/`faults`
//! declaredness is not comparable to anything — a change in whether the key is
//! present at all is not a movement in the order, it is a reason the whole tuple
//! must fail closed. Collapsing all four components onto one `Option<i64>` would
//! erase that distinction in the type itself, so there are two component types:
//!
//! - [`ExplorationBound`] (`values`, `depth`): `Bounded(n)` or `Unbounded`, a
//!   genuine total order with `Unbounded` on top.
//! - [`DeclaredBound`] (`nodes`, `faults`): `Declared(n)` or `Undeclared`, which
//!   this module deliberately does *not* give an [`Ord`] impl — see
//!   [`DeclaredBound::movement_to`] for why a movement between the two states is
//!   computed directly rather than through a derived comparison.
//!
//! # One spelling per meaning, encoder side
//!
//! ID5 requires that "absent" and an equivalent explicit spelling never produce
//! two identities for one meaning. The reader is liberal (it accepts either
//! spelling RFC 0037 permits), but the writer commits to one:
//!
//! - `values` and `depth` are **always** written, `null` standing for unbounded —
//!   the same choice [`crate::property::Claim`] makes for its always-`null`-when-
//!   absent `observer` field.
//! - `nodes` and `faults` are written **only when declared**; there is no legal
//!   `null` for either (the schema types them `integer`, not `["integer",
//!   "null"]`), so omission is the only spelling of "undeclared" and
//!   [`Bounds::decode`] rejects an explicit `null` for either as a type mismatch
//!   rather than reading it as "undeclared" — a silent `null → undeclared`
//!   coercion would let two distinct input documents collapse onto one meaning
//!   through a spelling the schema does not itself sanction.
//!
//! # The comparison RFC 0031 needs
//!
//! RFC 0037 flags bounds as the field whose diff needs a *typed direction*, not a
//! boolean:
//!
//! > shrinking a bound is a WEAKENING per RFC 0031's classification
//!
//! and RFC 0031 fixes the rule this module implements — the componentwise partial
//! order over the tuple, and nothing past it (no unit keying beyond the tuple
//! itself, no policy verdict, no evidence impact — those are RFC 0031's diff
//! engine and RFC 0032's promotion path, not this crate's smallest-trust-base
//! remit):
//!
//! > **Bounds:** componentwise partial order on `(values, nodes, faults, depth)`;
//! > a decrease in any component with no increase elsewhere is `contracted`; mixed
//! > changes are `incomparable`. `no-decrease` blocks `contracted` and blocks
//! > `incomparable` pending review.
//! >
//! > […] An absent `nodes` or `faults` key has no null form and no default; a
//! > change in the declaredness of either MUST classify `bounds` as `unknown` and
//! > fail closed.
//! >
//! > — `notes/plan/rfcs/0031-semantic-and-intent-diff.md`, "`bounds`"
//!
//! [`Bounds::relation_to`] computes exactly that: [`BoundsRelation`], one of the
//! five relations the field table admits for `bounds` (`unchanged`, `expanded`,
//! `contracted`, `incomparable`, `unknown`) — never a `bool`, because "a shrunk
//! bound compares as a weakening direction" and a boolean cannot also carry
//! "undecidable because declaredness changed" or "grew in one dimension and
//! shrank in another". This is the field's own order, stated once here; it is not
//! RFC 0031's unit-keyed diff *record* (which also carries a `fragment`
//! attribution, a `protected` flag, and a place in a `diff_*` artifact) — that
//! assembly is the contract-diff bone's job, not a single field group's.
//!
//! The concrete gaming move this order exists to catch, from RFC 0031's mapping
//! table: "reduce node count from five to three" classifies `contracted` and is
//! what the `no-decrease` verb blocks (plan §5.1, docs/50). This module does not
//! enforce `no-decrease` — that is RFC 0037's change-policy field group — it only
//! makes `contracted` a fact [`Bounds::relation_to`] can state.
//!
//! # What is not here
//!
//! - **The policy verb and its enforcement** (`no-decrease`, and the P1–P7
//!   verdict computation) are the `change_policy` field group's.
//! - **The `diff_*` artifact assembly** — unit locators, `fragment` attribution,
//!   the join across all fifteen fields — is RFC 0031's diff engine, a later PR.
//! - **The contract identity is not here.** This module supplies the `bounds`
//!   portion of the ID1 preimage through [`Bounds::identity`] and nothing more.

use core::cmp::Ordering;
use core::fmt;
use std::collections::BTreeMap;

use continuum_value::identity::{ContentHasher, Digest256};

use crate::canonical_json::Json;

/// One `values`/`depth` component: a finite integer bound, or no bound at all.
///
/// # Why `derive(Ord)` is the correct order here
///
/// Rust derives `Ord`/`PartialOrd` for an enum by comparing the variant's
/// declaration index first, and only then its payload. Declaring `Bounded`
/// before `Unbounded` therefore derives *exactly* RFC 0037's rule: every
/// `Bounded(_)` sorts below `Unbounded` regardless of the wrapped integer, and
/// two `Bounded` values compare by that integer — "`null` denotes unbounded and
/// is the top of that component's order". This is stated once, in the `derive`,
/// rather than hand-written as a `match`, so the order cannot drift from the
/// declaration order by an editing mistake.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ExplorationBound {
    /// A finite bound.
    Bounded(i64),
    /// No bound: the top of the component's order.
    Unbounded,
}

impl ExplorationBound {
    /// How this bound moved to `after`, in this component's own order.
    fn movement_to(self, after: Self) -> ComponentMovement {
        match self.cmp(&after) {
            Ordering::Equal => ComponentMovement::Unchanged,
            Ordering::Less => ComponentMovement::Increased,
            Ordering::Greater => ComponentMovement::Decreased,
        }
    }

    /// The component as JSON: `null` for unbounded, the integer otherwise.
    fn to_json(self) -> Json {
        match self {
            Self::Bounded(n) => Json::Integer(n),
            Self::Unbounded => Json::Null,
        }
    }
}

/// One `nodes`/`faults` component: a declared integer bound, or not declared at
/// all.
///
/// Deliberately carries no [`Ord`] impl. RFC 0037 gives `nodes`/`faults`
/// declaredness no order relation to any value — "a change in the declaredness
/// of either MUST classify `bounds` as `unknown`" is not a comparison outcome,
/// it is a refusal to compare. [`DeclaredBound::movement_to`] states that
/// directly instead of routing it through a derived `Ord` that would have to
/// invent a placement for `Undeclared` (below every integer? above? RFC 0037
/// answers "neither" by never asking the question).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeclaredBound {
    /// The bound is not declared.
    Undeclared,
    /// The bound is declared at this value.
    Declared(i64),
}

impl DeclaredBound {
    /// How this bound moved to `after`.
    ///
    /// [`ComponentMovement::DeclarednessChanged`] whenever exactly one side is
    /// [`Self::Undeclared`] — the RFC 0031 fail-closed case — never a guessed
    /// direction.
    fn movement_to(self, after: Self) -> ComponentMovement {
        match (self, after) {
            (Self::Undeclared, Self::Undeclared) => ComponentMovement::Unchanged,
            (Self::Declared(before), Self::Declared(after)) => match before.cmp(&after) {
                Ordering::Equal => ComponentMovement::Unchanged,
                Ordering::Less => ComponentMovement::Increased,
                Ordering::Greater => ComponentMovement::Decreased,
            },
            (Self::Undeclared, Self::Declared(_)) | (Self::Declared(_), Self::Undeclared) => {
                ComponentMovement::DeclarednessChanged
            }
        }
    }

    /// The component as JSON, or `None` when undeclared (the key is then
    /// omitted entirely — there is no `null` spelling for this component).
    fn to_json(self) -> Option<Json> {
        match self {
            Self::Undeclared => None,
            Self::Declared(n) => Some(Json::Integer(n)),
        }
    }
}

/// How one component of the tuple moved between two [`Bounds`] values.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ComponentMovement {
    /// The component did not change.
    Unchanged,
    /// The component grew (a larger bound, or a newly unbounded component).
    Increased,
    /// The component shrank.
    Decreased,
    /// A `nodes`/`faults` component's declaredness changed. Forces the whole
    /// tuple's relation to [`BoundsRelation::Unknown`], regardless of every
    /// other component.
    DeclarednessChanged,
}

/// The `bounds` field's relation, restricted to the five tokens RFC 0031's field
/// table admits for it: `unchanged`, `expanded`, `contracted`, `incomparable`,
/// `unknown` (the schema's sixteen-token `relation` enum, narrowed to what this
/// one field can ever emit — `bounds` never classifies `strengthened`,
/// `refined`, `merged`, `upgraded`, `added`, or any of the other eleven tokens
/// that belong to other fields' orders).
///
/// This is the "typed ordering, not a bool" the field's comparison needs: a
/// shrunk bound must compare as a weakening *direction*
/// ([`BoundsRelation::Contracted`]), and a mixed or undecidable change must be
/// distinguishable from both a genuine expansion and a genuine contraction
/// rather than collapsing onto "changed: yes/no".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoundsRelation {
    /// Every component's canonical encoding is equal.
    Unchanged,
    /// At least one component grew and none shrank: a larger verification
    /// envelope, the direction `no-decrease` permits.
    Expanded,
    /// At least one component shrank and none grew: a smaller verification
    /// envelope — RFC 0031's "reduce node count from five to three" — the
    /// direction `no-decrease` blocks.
    Contracted,
    /// Some component grew while another shrank: mixed movement, with no
    /// single direction.
    Incomparable,
    /// A `nodes`/`faults` component's declaredness changed between the two
    /// sides. Reported instead of, never alongside, a componentwise verdict:
    /// RFC 0031 makes this fail closed unconditionally.
    Unknown,
}

impl BoundsRelation {
    /// The wire literal, matching `semantic-diff.schema.json`'s `relation` enum.
    #[must_use]
    pub const fn wire(self) -> &'static str {
        match self {
            Self::Unchanged => "unchanged",
            Self::Expanded => "expanded",
            Self::Contracted => "contracted",
            Self::Incomparable => "incomparable",
            Self::Unknown => "unknown",
        }
    }
}

impl fmt::Display for BoundsRelation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.wire())
    }
}

/// The canonical identity of a `bounds` artifact.
///
/// Mirrors [`crate::property::PropertyIdentity`] (ADR-0013): this type *is* the
/// identity-preimage bytes, not a struct holding a digest.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct BoundsIdentity {
    canonical: Vec<u8>,
}

impl BoundsIdentity {
    fn of_bytes(canonical: Vec<u8>) -> Self {
        Self { canonical }
    }

    /// The canonical bytes this identity is.
    #[must_use]
    pub fn canonical_bytes(&self) -> &[u8] {
        &self.canonical
    }

    /// A digest of the canonical bytes, for indexing only (ADR-0013).
    #[must_use]
    pub fn digest<H: ContentHasher>(&self) -> Digest256 {
        H::hash(&self.canonical)
    }
}

impl fmt::Display for BoundsIdentity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match core::str::from_utf8(&self.canonical) {
            Ok(text) => f.write_str(text),
            Err(_) => Err(fmt::Error),
        }
    }
}

/// `bounds`: the finite-exploration-bound tuple `(values, nodes, faults, depth)`.
///
/// Immutable, like every carried type in this crate: no `&mut` method, no
/// setter. `bounds` MAY be the empty object — every component then reads as
/// unbounded/undeclared — which is itself a declaration, not an absence (RFC
/// 0037: the `bounds` *key* is required on the contract even though every
/// component inside it is optional).
///
/// [`PartialEq`] is hand-written over the four components only, never consulting
/// the derived [`identity`](Self::identity) — the same discipline
/// [`crate::property::Claim`] uses, so that a future identity that is not a pure
/// function of content cannot quietly become the equality relation.
#[derive(Debug, Clone)]
pub struct Bounds {
    values: ExplorationBound,
    nodes: DeclaredBound,
    faults: DeclaredBound,
    depth: ExplorationBound,
    identity: BoundsIdentity,
}

impl Bounds {
    /// Build a bounds tuple, enforcing the schema's per-component minimums.
    ///
    /// # Errors
    ///
    /// [`BoundsError::BelowMinimum`] when a declared/bounded component is below
    /// its schema minimum: `values` ≥ 1, `nodes` ≥ 1, `faults` ≥ 0, `depth` ≥ 0.
    pub fn new(
        values: ExplorationBound,
        nodes: DeclaredBound,
        faults: DeclaredBound,
        depth: ExplorationBound,
    ) -> Result<Self, BoundsError> {
        check_exploration_minimum(values, "bounds.values", 1)?;
        check_exploration_minimum(depth, "bounds.depth", 0)?;
        check_declared_minimum(nodes, "bounds.nodes", 1)?;
        check_declared_minimum(faults, "bounds.faults", 0)?;
        let identity = BoundsIdentity::of_bytes(
            bounds_json(values, nodes, faults, depth).to_canonical_bytes(),
        );
        Ok(Self {
            values,
            nodes,
            faults,
            depth,
            identity,
        })
    }

    /// The `values` component: how many distinct concrete values a bounded
    /// domain may range over.
    #[must_use]
    pub const fn values(&self) -> ExplorationBound {
        self.values
    }

    /// The `nodes` component: the exploration budget in states/nodes.
    #[must_use]
    pub const fn nodes(&self) -> DeclaredBound {
        self.nodes
    }

    /// The `faults` component: how many simultaneous faults exploration
    /// injects.
    #[must_use]
    pub const fn faults(&self) -> DeclaredBound {
        self.faults
    }

    /// The `depth` component: how many steps exploration takes.
    #[must_use]
    pub const fn depth(&self) -> ExplorationBound {
        self.depth
    }

    /// The bounds tuple's canonical identity.
    #[must_use]
    pub const fn identity(&self) -> &BoundsIdentity {
        &self.identity
    }

    /// The artifact-form JSON: what a contract document carries under `bounds`.
    #[must_use]
    pub fn artifact_json(&self) -> Json {
        bounds_json(self.values, self.nodes, self.faults, self.depth)
    }

    /// The artifact-form bytes, under the ID5 canonical-JSON rules.
    #[must_use]
    pub fn to_artifact_bytes(&self) -> Vec<u8> {
        self.artifact_json().to_canonical_bytes()
    }

    /// This field's relation to `after`, in RFC 0031's `bounds` order.
    ///
    /// `self` is the before-side, `after` the after-side, matching RFC 0031's
    /// "given two snapshots" framing. See the module documentation's "The
    /// comparison RFC 0031 needs" section for the rule this implements.
    #[must_use]
    pub fn relation_to(&self, after: &Self) -> BoundsRelation {
        let movements = [
            self.values.movement_to(after.values),
            self.nodes.movement_to(after.nodes),
            self.faults.movement_to(after.faults),
            self.depth.movement_to(after.depth),
        ];
        if movements
            .iter()
            .any(|movement| matches!(movement, ComponentMovement::DeclarednessChanged))
        {
            // Fail closed unconditionally: RFC 0031 does not let an increase
            // elsewhere excuse a declaredness change on `nodes`/`faults`.
            return BoundsRelation::Unknown;
        }
        let increased = movements
            .iter()
            .any(|movement| matches!(movement, ComponentMovement::Increased));
        let decreased = movements
            .iter()
            .any(|movement| matches!(movement, ComponentMovement::Decreased));
        match (increased, decreased) {
            (false, false) => BoundsRelation::Unchanged,
            (true, false) => BoundsRelation::Expanded,
            (false, true) => BoundsRelation::Contracted,
            (true, true) => BoundsRelation::Incomparable,
        }
    }

    /// Decode a bounds tuple from its artifact form.
    ///
    /// # Errors
    ///
    /// [`BoundsDecodeError`], naming the field that failed.
    pub fn decode(bytes: &[u8]) -> Result<Self, BoundsDecodeError> {
        let json = Json::parse(bytes)?;
        Self::from_json(&json)
    }

    /// Decode a bounds tuple from an already-parsed JSON value.
    ///
    /// # Errors
    ///
    /// [`BoundsDecodeError`], as [`Bounds::decode`].
    pub fn from_json(json: &Json) -> Result<Self, BoundsDecodeError> {
        let fields = json
            .as_object()
            .ok_or_else(|| BoundsDecodeError::TypeMismatch {
                field: "bounds",
                expected: "object",
                found: json.type_name(),
            })?;
        known_keys(fields, "bounds", &["depth", "faults", "nodes", "values"])?;
        let values = decode_exploration_bound(fields, "values", "bounds.values")?;
        let depth = decode_exploration_bound(fields, "depth", "bounds.depth")?;
        let nodes = decode_declared_bound(fields, "nodes", "bounds.nodes")?;
        let faults = decode_declared_bound(fields, "faults", "bounds.faults")?;
        Ok(Self::new(values, nodes, faults, depth)?)
    }
}

impl PartialEq for Bounds {
    fn eq(&self, other: &Self) -> bool {
        self.values == other.values
            && self.nodes == other.nodes
            && self.faults == other.faults
            && self.depth == other.depth
    }
}

impl Eq for Bounds {}

fn bounds_json(
    values: ExplorationBound,
    nodes: DeclaredBound,
    faults: DeclaredBound,
    depth: ExplorationBound,
) -> Json {
    let mut fields = vec![
        ("depth".to_owned(), depth.to_json()),
        ("values".to_owned(), values.to_json()),
    ];
    if let Some(nodes_json) = nodes.to_json() {
        fields.push(("nodes".to_owned(), nodes_json));
    }
    if let Some(faults_json) = faults.to_json() {
        fields.push(("faults".to_owned(), faults_json));
    }
    Json::object(fields).unwrap_or(Json::Null)
}

fn check_exploration_minimum(
    bound: ExplorationBound,
    field: &'static str,
    minimum: i64,
) -> Result<(), BoundsError> {
    if let ExplorationBound::Bounded(found) = bound
        && found < minimum
    {
        return Err(BoundsError::BelowMinimum {
            field,
            minimum,
            found,
        });
    }
    Ok(())
}

fn check_declared_minimum(
    bound: DeclaredBound,
    field: &'static str,
    minimum: i64,
) -> Result<(), BoundsError> {
    if let DeclaredBound::Declared(found) = bound
        && found < minimum
    {
        return Err(BoundsError::BelowMinimum {
            field,
            minimum,
            found,
        });
    }
    Ok(())
}

fn known_keys(
    fields: &BTreeMap<String, Json>,
    field: &'static str,
    allowed: &[&str],
) -> Result<(), BoundsDecodeError> {
    for key in fields.keys() {
        if !allowed.contains(&key.as_str()) {
            return Err(BoundsDecodeError::UnknownField {
                field,
                key: key.clone(),
            });
        }
    }
    Ok(())
}

fn decode_exploration_bound(
    fields: &BTreeMap<String, Json>,
    key: &str,
    field: &'static str,
) -> Result<ExplorationBound, BoundsDecodeError> {
    match fields.get(key) {
        None | Some(Json::Null) => Ok(ExplorationBound::Unbounded),
        Some(Json::Integer(n)) => Ok(ExplorationBound::Bounded(*n)),
        Some(other) => Err(BoundsDecodeError::TypeMismatch {
            field,
            expected: "integer or null",
            found: other.type_name(),
        }),
    }
}

fn decode_declared_bound(
    fields: &BTreeMap<String, Json>,
    key: &str,
    field: &'static str,
) -> Result<DeclaredBound, BoundsDecodeError> {
    match fields.get(key) {
        None => Ok(DeclaredBound::Undeclared),
        Some(Json::Integer(n)) => Ok(DeclaredBound::Declared(*n)),
        Some(other) => Err(BoundsDecodeError::TypeMismatch {
            field,
            // `null` is deliberately not admissible here: the schema types
            // `nodes`/`faults` as plain integers, and there is no meaning for
            // this component `null` could stand for (module documentation).
            expected: "integer",
            found: other.type_name(),
        }),
    }
}

// --- errors --------------------------------------------------------------------------------

/// Why a bounds tuple could not be built.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BoundsError {
    /// A declared component is below its schema minimum.
    BelowMinimum {
        /// The component's field path.
        field: &'static str,
        /// The schema's minimum for this component.
        minimum: i64,
        /// The value that violated it.
        found: i64,
    },
}

impl fmt::Display for BoundsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BelowMinimum {
                field,
                minimum,
                found,
            } => write!(
                f,
                "`{field}` must be at least {minimum} (intent-contract.schema.json); found {found}"
            ),
        }
    }
}

impl core::error::Error for BoundsError {}

/// Why a byte string is not a `bounds` artifact.
///
/// Every variant is a rejection, never a repair — RFC 0037: "an intent contract
/// is never best-effort decoded".
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BoundsDecodeError {
    /// The bytes are not admissible canonical JSON.
    Json(crate::canonical_json::JsonError),
    /// A field carries the wrong JSON type.
    TypeMismatch {
        /// The field's path.
        field: &'static str,
        /// What the schema admits.
        expected: &'static str,
        /// What was found.
        found: &'static str,
    },
    /// An object carries a key the schema's `additionalProperties: false`
    /// forbids.
    UnknownField {
        /// The object's path.
        field: &'static str,
        /// The offending key.
        key: String,
    },
    /// The decoded components do not satisfy the schema's minimums.
    Bounds(BoundsError),
}

impl fmt::Display for BoundsDecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Json(error) => error.fmt(f),
            Self::TypeMismatch {
                field,
                expected,
                found,
            } => write!(f, "`{field}` must be {expected}; found {found}"),
            Self::UnknownField { field, key } => write!(
                f,
                "`{field}` carries the unknown key {key:?}; the schema sets \
                 additionalProperties: false and a reader that ignored it would accept documents \
                 the schema rejects"
            ),
            Self::Bounds(error) => error.fmt(f),
        }
    }
}

impl core::error::Error for BoundsDecodeError {}

impl From<crate::canonical_json::JsonError> for BoundsDecodeError {
    fn from(error: crate::canonical_json::JsonError) -> Self {
        Self::Json(error)
    }
}

impl From<BoundsError> for BoundsDecodeError {
    fn from(error: BoundsError) -> Self {
        Self::Bounds(error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bounds(
        values: ExplorationBound,
        nodes: DeclaredBound,
        faults: DeclaredBound,
        depth: ExplorationBound,
    ) -> Bounds {
        Bounds::new(values, nodes, faults, depth).expect("test bounds are within the minimums")
    }

    fn utf8(bytes: Vec<u8>) -> String {
        String::from_utf8(bytes).expect("canonical output is UTF-8")
    }

    // --- the component order, by construction -------------------------------------

    #[test]
    fn unbounded_is_the_top_of_the_exploration_bound_order() {
        assert!(ExplorationBound::Bounded(1_000_000) < ExplorationBound::Unbounded);
        assert!(ExplorationBound::Bounded(2) > ExplorationBound::Bounded(1));
        assert_eq!(ExplorationBound::Unbounded, ExplorationBound::Unbounded);
    }

    #[test]
    fn declared_bound_carries_no_order_between_declared_and_undeclared() {
        // Not an `Ord` type, so there is nothing to compare with `<`; the only
        // question this module lets a caller ask is `movement_to`, and it
        // answers with `DeclarednessChanged`, not a direction.
        assert_eq!(
            DeclaredBound::Declared(3).movement_to(DeclaredBound::Undeclared),
            ComponentMovement::DeclarednessChanged
        );
        assert_eq!(
            DeclaredBound::Undeclared.movement_to(DeclaredBound::Declared(3)),
            ComponentMovement::DeclarednessChanged
        );
        assert_eq!(
            DeclaredBound::Declared(3).movement_to(DeclaredBound::Declared(3)),
            ComponentMovement::Unchanged
        );
    }

    // --- minimums -------------------------------------------------------------------

    #[test]
    fn each_components_minimum_is_enforced() {
        assert_eq!(
            Bounds::new(
                ExplorationBound::Bounded(0),
                DeclaredBound::Undeclared,
                DeclaredBound::Undeclared,
                ExplorationBound::Unbounded,
            ),
            Err(BoundsError::BelowMinimum {
                field: "bounds.values",
                minimum: 1,
                found: 0,
            })
        );
        assert!(
            Bounds::new(
                ExplorationBound::Bounded(1),
                DeclaredBound::Undeclared,
                DeclaredBound::Undeclared,
                ExplorationBound::Unbounded,
            )
            .is_ok()
        );
        assert_eq!(
            Bounds::new(
                ExplorationBound::Unbounded,
                DeclaredBound::Declared(0),
                DeclaredBound::Undeclared,
                ExplorationBound::Unbounded,
            ),
            Err(BoundsError::BelowMinimum {
                field: "bounds.nodes",
                minimum: 1,
                found: 0,
            })
        );
        assert_eq!(
            Bounds::new(
                ExplorationBound::Unbounded,
                DeclaredBound::Undeclared,
                DeclaredBound::Declared(-1),
                ExplorationBound::Unbounded,
            ),
            Err(BoundsError::BelowMinimum {
                field: "bounds.faults",
                minimum: 0,
                found: -1,
            })
        );
        assert!(
            Bounds::new(
                ExplorationBound::Unbounded,
                DeclaredBound::Undeclared,
                DeclaredBound::Declared(0),
                ExplorationBound::Unbounded,
            )
            .is_ok()
        );
        assert_eq!(
            Bounds::new(
                ExplorationBound::Unbounded,
                DeclaredBound::Undeclared,
                DeclaredBound::Undeclared,
                ExplorationBound::Bounded(-1),
            ),
            Err(BoundsError::BelowMinimum {
                field: "bounds.depth",
                minimum: 0,
                found: -1,
            })
        );
    }

    // --- one spelling per meaning -----------------------------------------------------

    #[test]
    fn values_and_depth_are_always_written_nodes_and_faults_only_when_declared() {
        let empty = bounds(
            ExplorationBound::Unbounded,
            DeclaredBound::Undeclared,
            DeclaredBound::Undeclared,
            ExplorationBound::Unbounded,
        );
        assert_eq!(
            utf8(empty.to_artifact_bytes()),
            r#"{"depth":null,"values":null}"#
        );
        let full = bounds(
            ExplorationBound::Bounded(2),
            DeclaredBound::Declared(2),
            DeclaredBound::Declared(1),
            ExplorationBound::Bounded(20),
        );
        assert_eq!(
            utf8(full.to_artifact_bytes()),
            r#"{"depth":20,"faults":1,"nodes":2,"values":2}"#
        );
    }

    #[test]
    fn absent_and_explicit_null_decode_to_one_identity_for_values_and_depth() {
        let omitted = Bounds::decode(b"{}").expect("empty object decodes");
        let explicit = Bounds::decode(br#"{"depth":null,"values":null}"#).expect("decodes");
        assert_eq!(omitted, explicit);
        assert_eq!(omitted.identity(), explicit.identity());
    }

    #[test]
    fn an_explicit_null_is_rejected_for_nodes_and_faults() {
        assert_eq!(
            Bounds::decode(br#"{"nodes":null}"#),
            Err(BoundsDecodeError::TypeMismatch {
                field: "bounds.nodes",
                expected: "integer",
                found: "null",
            })
        );
        assert_eq!(
            Bounds::decode(br#"{"faults":null}"#),
            Err(BoundsDecodeError::TypeMismatch {
                field: "bounds.faults",
                expected: "integer",
                found: "null",
            })
        );
    }

    #[test]
    fn round_trips_the_schemas_own_example() {
        let example = br#"{"depth":20,"faults":1,"nodes":2,"values":2}"#;
        let decoded = Bounds::decode(example).expect("the schema's own example decodes");
        assert_eq!(decoded.to_artifact_bytes(), example);
        assert_eq!(decoded.values(), ExplorationBound::Bounded(2));
        assert_eq!(decoded.nodes(), DeclaredBound::Declared(2));
        assert_eq!(decoded.faults(), DeclaredBound::Declared(1));
        assert_eq!(decoded.depth(), ExplorationBound::Bounded(20));
    }

    // --- the comparison surface -------------------------------------------------------

    #[test]
    fn no_movement_anywhere_is_unchanged() {
        let a = bounds(
            ExplorationBound::Bounded(2),
            DeclaredBound::Declared(5),
            DeclaredBound::Declared(1),
            ExplorationBound::Bounded(20),
        );
        let b = bounds(
            ExplorationBound::Bounded(2),
            DeclaredBound::Declared(5),
            DeclaredBound::Declared(1),
            ExplorationBound::Bounded(20),
        );
        assert_eq!(a.relation_to(&b), BoundsRelation::Unchanged);
    }

    #[test]
    fn shrinking_node_count_from_five_to_three_is_contracted() {
        // RFC 0031's own gaming-move example: "reduce node count from five to
        // three" classifies `contracted` and is what `no-decrease` blocks.
        let before = bounds(
            ExplorationBound::Unbounded,
            DeclaredBound::Declared(5),
            DeclaredBound::Undeclared,
            ExplorationBound::Unbounded,
        );
        let after = bounds(
            ExplorationBound::Unbounded,
            DeclaredBound::Declared(3),
            DeclaredBound::Undeclared,
            ExplorationBound::Unbounded,
        );
        assert_eq!(before.relation_to(&after), BoundsRelation::Contracted);
        // And the reverse direction is the safe one, `expanded`.
        assert_eq!(after.relation_to(&before), BoundsRelation::Expanded);
    }

    #[test]
    fn going_unbounded_is_an_expansion_and_the_reverse_is_a_contraction() {
        let bounded = bounds(
            ExplorationBound::Bounded(4),
            DeclaredBound::Undeclared,
            DeclaredBound::Undeclared,
            ExplorationBound::Unbounded,
        );
        let unbounded = bounds(
            ExplorationBound::Unbounded,
            DeclaredBound::Undeclared,
            DeclaredBound::Undeclared,
            ExplorationBound::Unbounded,
        );
        assert_eq!(bounded.relation_to(&unbounded), BoundsRelation::Expanded);
        assert_eq!(unbounded.relation_to(&bounded), BoundsRelation::Contracted);
    }

    #[test]
    fn mixed_movement_is_incomparable() {
        let before = bounds(
            ExplorationBound::Bounded(2),
            DeclaredBound::Declared(5),
            DeclaredBound::Undeclared,
            ExplorationBound::Unbounded,
        );
        // `values` grows, `nodes` shrinks: no single direction.
        let after = bounds(
            ExplorationBound::Bounded(3),
            DeclaredBound::Declared(4),
            DeclaredBound::Undeclared,
            ExplorationBound::Unbounded,
        );
        assert_eq!(before.relation_to(&after), BoundsRelation::Incomparable);
    }

    #[test]
    fn a_declaredness_change_is_unknown_even_when_every_other_component_grows() {
        let before = bounds(
            ExplorationBound::Bounded(2),
            DeclaredBound::Undeclared,
            DeclaredBound::Declared(1),
            ExplorationBound::Bounded(10),
        );
        // Every declared/bounded component only grows here, but `faults` goes
        // from declared to undeclared: RFC 0031 fails this closed
        // unconditionally, never letting the other three components excuse it.
        let after = bounds(
            ExplorationBound::Unbounded,
            DeclaredBound::Undeclared,
            DeclaredBound::Undeclared,
            ExplorationBound::Unbounded,
        );
        assert_eq!(before.relation_to(&after), BoundsRelation::Unknown);
    }

    #[test]
    fn the_relation_wire_tokens_match_the_semantic_diff_vocabulary() {
        assert_eq!(BoundsRelation::Unchanged.wire(), "unchanged");
        assert_eq!(BoundsRelation::Expanded.wire(), "expanded");
        assert_eq!(BoundsRelation::Contracted.wire(), "contracted");
        assert_eq!(BoundsRelation::Incomparable.wire(), "incomparable");
        assert_eq!(BoundsRelation::Unknown.wire(), "unknown");
    }

    // --- identity and equality never consult each other's derived state --------------

    #[test]
    fn equal_content_from_different_input_spellings_shares_one_identity() {
        let from_omission = Bounds::decode(br#"{"nodes":2}"#).expect("decodes");
        let from_explicit_unbounded =
            Bounds::decode(br#"{"depth":null,"nodes":2,"values":null}"#).expect("decodes");
        assert_eq!(from_omission, from_explicit_unbounded);
        assert_eq!(from_omission.identity(), from_explicit_unbounded.identity());
    }
}
