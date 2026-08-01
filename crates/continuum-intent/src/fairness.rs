//! The `fairness` field group: which actions the environment must eventually take
//! (PR-4 / IMPL-06).
//!
//! # What a fairness constraint *is*, in this crate
//!
//! > | `fairness[]` | `{kind, action, condition?}` | `kind`, `action` | `kind` ∈
//! > {`weak`, `strong`}; `condition` is a temporal-operator-free property AST or
//! > `null`; fairness strengthening is the canonical gaming vector (docs/50) |
//! >
//! > — `notes/plan/rfcs/0037-intent-contract.md`, "Field types and enums"
//!
//! > Constraints are keyed by (`kind`, `action`); the schema gives fairness items no
//! > `id`.
//! >
//! > — `notes/plan/rfcs/0031-semantic-and-intent-diff.md`, "`fairness`"
//!
//! So a fairness constraint is a **two-part unit key and an optional state formula**.
//! [`FairnessKey`] is the pair, [`FairnessConstraint`] is the constraint, and
//! [`FairnessSet`] is the keyed array W1 requires to have no repeated pair.
//!
//! # The protected weakening this shape makes visible
//!
//! Fairness is the attack docs/50 and plan §5.1 name first among the
//! environment-strengthening moves, and RFC 0031's attack table spells it out: *add a
//! fairness assumption that schedules away the bug → `fairness` → `added` /
//! `strengthened` → `review`*. A stronger fairness assumption does not weaken a
//! property directly; it deletes the *behaviors* in which the property fails, so the
//! same claim passes over a smaller world.
//!
//! Three movements carry it, and each is structurally visible here:
//!
//! - **Membership.** A constraint present on one side only is `added` or `removed`.
//!   [`FairnessSet::unit_keys`] is the set that comparison runs over, and the key is
//!   the (`kind`, `action`) pair, so it cannot be confused with a content match.
//! - **`kind`.** `weak → strong` on the same action is `strengthened`, because strong
//!   fairness implies weak fairness. The kind is *part of the unit key*, which is the
//!   schema's decision and not this module's; RFC 0031 notes the same movement is
//!   equivalently one `removed` plus one `added` and requires a classifier to pick one
//!   encoding.
//! - **`condition`, antitonely.** "A fairness constraint applies wherever its
//!   condition holds, so weakening the condition strengthens the constraint […]
//!   `null` means unconditional and is the weakest condition, hence the strongest
//!   constraint: `C → null` classifies `strengthened`." The condition is therefore
//!   stored as a *structure* in CPNF-1 — never a string — so that the formula relation
//!   the duality is applied to is the same one claims are compared by.
//!
//! # W4: a fairness condition is a state formula, and that is checked twice
//!
//! > **W4 — Fairness conditions are state formulas.** `fairness[].condition` MUST
//! > contain no `always`, `eventually`, or `leads_to` node at any depth. The schema
//! > enforces this structurally (`$defs/state_formula` over `$defs/temporal_free`); it
//! > MUST be rejected by the schema, not by prose.
//! >
//! > — RFC 0037
//!
//! Because W4 is a rule about the *document*, [`FairnessConstraint::from_json`]
//! applies it the way the schema does: as a structural exclusion over the parsed
//! document, before the AST is built or normalized, so the rejection names the
//! operator the author actually wrote rather than whatever CPNF-1 rewrote it into.
//! [`FairnessConstraint::new`] applies the same rule to a caller-supplied AST through
//! [`crate::ast::Formula::is_temporal_free`], which is `ast`'s statement of W4. Both
//! paths raise [`FairnessError::TemporalCondition`], and the normalized result is
//! re-checked: CPNF-1 introduces no temporal operator, and an assertion is cheaper
//! than the assumption.
//!
//! `MalformedRequest` — RFC 0037's protocol-level code for a W1–W10 failure — is RFC
//! 0026's error taxonomy and belongs to the protocol crate. What this crate owes it is
//! a *typed*, nameable cause, which is what these variants are.
//!
//! # The bare condition, and the one seam it needed
//!
//! > The two carriers differ in shape, and the difference is normative:
//! > `claims[].expression` and `assumptions[].expression` are `property_expression`
//! > objects — `{ast, source?, fragment?, normal_form?}` — while
//! > `fairness[].condition` is a **bare** temporal-operator-free formula with no
//! > `source`, no `fragment`, and no `normal_form` declaration. A fairness condition
//! > is therefore normalized under the contract's declared fragments (W3) and has no
//! > display-only rendering that could disagree with it.
//! >
//! > — RFC 0037
//!
//! A bare formula has no `source`, so no member of ID2's exclusion list has a site
//! here and — as in [`crate::observers`] and [`crate::faults`] — the artifact form *is*
//! the identity preimage.
//!
//! It also means this module needs to *read* a bare `$defs/formula`. There is exactly
//! one reader for that grammar in the crate — `crate::codec` — because two readers
//! for one grammar is how a document comes to have two meanings, and
//! [`crate::ast::Formula::from_json`] is its bare-formula entry point. Every rejection
//! a caller sees is therefore the AST's own, named by the AST's own field paths
//! (`formula.kind`, `compare.op`, `term.kind`, …), which is what
//! [`FairnessDecodeError::Condition`] carries.
//!
//! It was not always reachable: while the PR-4 field-group bones were in flight, that
//! reader was private to `property.rs` and this module got at it by wrapping the
//! condition in a minimal synthetic claim — a wrapper built from constants that could
//! never itself be rejected, and a bone comment asking for the public constructor. The
//! constructor exists now and the wrapper is gone.
//!
//! # Ordering
//!
//! [`FairnessSet`] encodes in [`FairnessKey`] order — `kind` then `action`, the order
//! the RFC writes the pair in — so reordering the authored array does not move the
//! identity. *Same flag as [`crate::property::ClaimSet`] raises: ID5 fixes the order of
//! object keys and says nothing about arrays.*
//!
//! # Seams left open on purpose
//!
//! - **W3's fragment rule.** A fairness condition declares no fragment of its own, so
//!   it is "normalized under the contract's declared fragments"; there is nothing for
//!   this group to check and nothing for it to store.
//! - **Classification is not here.** The antitone rule, the `weak → strong` direction,
//!   and the `null` extremum are RFC 0031's (PR 12). This module supplies the keys,
//!   the CPNF-1 conditions, and the identity S1 permits `unchanged` to be claimed on.

use core::cmp::Ordering;
use core::fmt;
use std::collections::{BTreeMap, BTreeSet};

use crate::ast::Formula;
use crate::canonical_json::{Json, JsonError};
use crate::cpnf::{self, NormalizeError};
use crate::identity::canonical_identity;
use crate::property::PropertyDecodeError;

/// Which fairness a constraint declares: `fairness[].kind`, the closed two-member
/// enum.
///
/// The vocabulary is closed, and the two members are ordered by implication rather
/// than by spelling: strong fairness implies weak fairness, so `weak → strong` on one
/// action is `strengthened` and the reverse is `weakened` (RFC 0031). That order is
/// deliberately *not* this type's [`Ord`], which is code-point order on the wire token
/// so that [`FairnessKey`] sorts the way every other keyed collection in this crate
/// does. A direction is a classification, and classification belongs to RFC 0031.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FairnessKind {
    /// Weak fairness: an action continuously enabled is eventually taken.
    Weak,
    /// Strong fairness: an action repeatedly enabled is eventually taken.
    Strong,
}

impl FairnessKind {
    /// Every kind, in schema-enum order.
    pub const ALL: [Self; 2] = [Self::Weak, Self::Strong];

    /// The wire literal.
    #[must_use]
    pub const fn wire(self) -> &'static str {
        match self {
            Self::Weak => "weak",
            Self::Strong => "strong",
        }
    }

    /// Recover a kind from its wire literal.
    ///
    /// Returns `None` for an unrecognized token. The caller rejects: "Forward
    /// compatibility is achieved by rejecting, never by ignoring" (RFC 0037).
    #[must_use]
    pub fn from_wire(token: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|kind| kind.wire() == token)
    }
}

impl Ord for FairnessKind {
    /// Code-point order on the wire token, so a [`FairnessKey`] sorts by the same rule
    /// ID5 fixes for object keys.
    fn cmp(&self, other: &Self) -> Ordering {
        self.wire().cmp(other.wire())
    }
}

impl PartialOrd for FairnessKind {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl fmt::Display for FairnessKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.wire())
    }
}

/// The action family a fairness constraint applies to: `fairness[].action`.
///
/// The schema types it as a bare string with no pattern — an action family is a model
/// name, not a property-AST identifier — so the only rule enforced here is the one the
/// diff schema states of a unit locator: `minLength: 1`. An empty action could not
/// appear as a `unit` in an `intent_changes` record, so its removal could not be
/// reported.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ActionName(String);

impl ActionName {
    /// Validate and wrap an action name.
    ///
    /// # Errors
    ///
    /// [`FairnessError::EmptyAction`] for the empty string.
    pub fn new(action: &str) -> Result<Self, FairnessError> {
        if action.is_empty() {
            return Err(FairnessError::EmptyAction);
        }
        Ok(Self(action.to_owned()))
    }

    /// The name.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ActionName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// The classified-unit key of a fairness constraint: the (`kind`, `action`) pair.
///
/// The schema gives fairness items no `id`, so the pair *is* the key, and W1 requires
/// it to be unique within the array. `semantic-diff.schema.json` fixes its rendering:
/// "fairness constraints by the (`kind`, `action`) pair, rendered `kind:action`".
/// [`FairnessKey::locator`] is that rendering, and it is injective even for an action
/// containing a colon, because `kind` is drawn from a closed colon-free vocabulary and
/// the split is at the first colon.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct FairnessKey {
    kind: FairnessKind,
    action: ActionName,
}

impl FairnessKey {
    /// The pair.
    #[must_use]
    pub const fn new(kind: FairnessKind, action: ActionName) -> Self {
        Self { kind, action }
    }

    /// Which fairness.
    #[must_use]
    pub const fn kind(&self) -> FairnessKind {
        self.kind
    }

    /// The action family.
    #[must_use]
    pub const fn action(&self) -> &ActionName {
        &self.action
    }

    /// The RFC 0031 per-unit locator, rendered `kind:action`.
    #[must_use]
    pub fn locator(&self) -> String {
        format!("{}:{}", self.kind.wire(), self.action)
    }
}

impl fmt::Display for FairnessKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.locator())
    }
}

canonical_identity! {
    /// The canonical identity of a fairness artifact.
    ///
    /// The discipline is `crate::identity::CanonicalIdentity`'s, stated in full
    /// there: the identity *is* the canonical preimage bytes, so equality is canonical
    /// comparison (ADR-0013) and [`FairnessIdentity::digest`] can only index.
    FairnessIdentity
}

/// One fairness constraint: `fairness[]`.
///
/// Immutable, for the reason `lib.rs` states. A strengthened constraint is a *new*
/// [`FairnessConstraint`] with a new [`FairnessIdentity`].
///
/// [`PartialEq`] is hand-written and compares the key and the normalized condition,
/// never the identity.
#[derive(Debug, Clone)]
pub struct FairnessConstraint {
    key: FairnessKey,
    condition: Option<Formula>,
    identity: FairnessIdentity,
}

impl FairnessConstraint {
    /// Build a constraint, normalizing its condition to CPNF-1 and enforcing W4.
    ///
    /// `None` is *unconditional*, which RFC 0031 records as the weakest condition and
    /// therefore the strongest constraint. It is not "no condition declared": RFC 0037
    /// says "An absent `condition` key MUST be read as `null`", so absence and `null`
    /// are one meaning, and this type gives them one spelling.
    ///
    /// # Errors
    ///
    /// - [`FairnessError::TemporalCondition`] — W4: the condition carries `always`,
    ///   `eventually`, or `leads_to` at some depth.
    /// - [`FairnessError::Normalize`] — the condition is not one CPNF-1 can normalize.
    pub fn new(key: FairnessKey, condition: Option<&Formula>) -> Result<Self, FairnessError> {
        let condition = match condition {
            None => None,
            Some(formula) => {
                if let Some(operator) = first_temporal_operator(formula) {
                    return Err(FairnessError::TemporalCondition { operator });
                }
                let normalized = cpnf::normalize(formula)?;
                // CPNF-1's rewrites introduce no temporal operator. Asserting it costs
                // one linear walk and removes an assumption from a soundness argument.
                if let Some(operator) = first_temporal_operator(&normalized) {
                    return Err(FairnessError::TemporalCondition { operator });
                }
                Some(normalized)
            }
        };
        let identity = FairnessIdentity::of_bytes(
            constraint_json(&key, condition.as_ref()).to_canonical_bytes(),
        );
        Ok(Self {
            key,
            condition,
            identity,
        })
    }

    /// The classified-unit key.
    #[must_use]
    pub const fn key(&self) -> &FairnessKey {
        &self.key
    }

    /// Which fairness.
    #[must_use]
    pub const fn kind(&self) -> FairnessKind {
        self.key.kind()
    }

    /// The action family.
    #[must_use]
    pub const fn action(&self) -> &ActionName {
        self.key.action()
    }

    /// The enabling predicate in CPNF-1, or `None` for an unconditional constraint.
    ///
    /// The position is **antitone**: a weaker condition is a stronger constraint, and
    /// `None` is the extremum. RFC 0031 applies the duality; this returns the formula
    /// the relation is computed on.
    #[must_use]
    pub const fn condition(&self) -> Option<&Formula> {
        self.condition.as_ref()
    }

    /// Whether the constraint is unconditional — the strongest it can be.
    #[must_use]
    pub const fn is_unconditional(&self) -> bool {
        self.condition.is_none()
    }

    /// The constraint's canonical identity, which is also its artifact bytes.
    #[must_use]
    pub const fn identity(&self) -> &FairnessIdentity {
        &self.identity
    }

    /// The artifact-form JSON: what a contract document carries.
    #[must_use]
    pub fn artifact_json(&self) -> Json {
        constraint_json(&self.key, self.condition.as_ref())
    }

    /// The artifact-form bytes, under the ID5 canonical-JSON rules.
    #[must_use]
    pub fn to_artifact_bytes(&self) -> Vec<u8> {
        self.artifact_json().to_canonical_bytes()
    }

    /// Decode a constraint from its artifact form.
    ///
    /// # Errors
    ///
    /// [`FairnessDecodeError`], naming the field, the token, or the temporal operator
    /// that failed.
    pub fn decode(bytes: &[u8]) -> Result<Self, FairnessDecodeError> {
        Self::from_json(&Json::parse(bytes)?)
    }

    /// Decode a constraint from an already-parsed JSON value.
    ///
    /// # Errors
    ///
    /// [`FairnessDecodeError`], as [`FairnessConstraint::decode`].
    pub fn from_json(json: &Json) -> Result<Self, FairnessDecodeError> {
        let fields = json
            .as_object()
            .ok_or_else(|| FairnessDecodeError::TypeMismatch {
                field: "fairness[]",
                expected: "object",
                found: json.type_name(),
            })?;
        for key in fields.keys() {
            if key != "action" && key != "condition" && key != "kind" {
                return Err(FairnessDecodeError::UnknownField {
                    field: "fairness[]",
                    key: key.clone(),
                });
            }
        }
        let kind_json = fields
            .get("kind")
            .ok_or(FairnessDecodeError::MissingField {
                field: "fairness[].kind",
            })?;
        let kind_token = kind_json
            .as_str()
            .ok_or_else(|| FairnessDecodeError::TypeMismatch {
                field: "fairness[].kind",
                expected: "string",
                found: kind_json.type_name(),
            })?;
        let kind = FairnessKind::from_wire(kind_token).ok_or_else(|| {
            FairnessDecodeError::UnknownToken {
                field: "fairness[].kind",
                token: kind_token.to_owned(),
            }
        })?;
        let action_json = fields
            .get("action")
            .ok_or(FairnessDecodeError::MissingField {
                field: "fairness[].action",
            })?;
        let action = ActionName::new(action_json.as_str().ok_or_else(|| {
            FairnessDecodeError::TypeMismatch {
                field: "fairness[].action",
                expected: "string",
                found: action_json.type_name(),
            }
        })?)?;
        let condition = match fields.get("condition") {
            // Absent and `null` are one meaning with one spelling (RFC 0037).
            None | Some(Json::Null) => None,
            Some(document) => Some(decode_condition(document)?),
        };
        Ok(Self::new(
            FairnessKey::new(kind, action),
            condition.as_ref(),
        )?)
    }
}

impl PartialEq for FairnessConstraint {
    fn eq(&self, other: &Self) -> bool {
        self.key == other.key && self.condition == other.condition
    }
}

impl Eq for FairnessConstraint {}

fn constraint_json(key: &FairnessKey, condition: Option<&Formula>) -> Json {
    Json::object([
        (
            "action".to_owned(),
            Json::String(key.action().as_str().to_owned()),
        ),
        (
            "condition".to_owned(),
            condition.map_or(Json::Null, Formula::to_json),
        ),
        (
            "kind".to_owned(),
            Json::String(key.kind().wire().to_owned()),
        ),
    ])
    .unwrap_or(Json::Null)
}

/// W4 as the schema states it: a structural exclusion of the three temporal node kinds
/// at every depth of the *document*.
///
/// `$defs/temporal_free` is applied to every child position of every node, and is
/// "vacuously satisfied" on a term, so a blanket walk over the parsed document is the
/// same rule. Only the value under a `kind` key is consulted, so a string literal
/// spelled `"always"` is data and not an operator.
///
/// Recursion is bounded by [`crate::canonical_json::MAX_DEPTH`], which the parser
/// enforced before this walk runs.
fn temporal_operator_in_document(json: &Json) -> Option<&'static str> {
    match json {
        Json::Object(fields) => {
            if let Some(Json::String(kind)) = fields.get("kind")
                && let Some(operator) = temporal_wire(kind)
            {
                return Some(operator);
            }
            fields.values().find_map(temporal_operator_in_document)
        }
        Json::Array(items) => items.iter().find_map(temporal_operator_in_document),
        _ => None,
    }
}

/// The same rule over an AST, for the constructor path where no document exists.
///
/// [`crate::ast::Formula::is_temporal_free`] is `ast`'s statement of W4 and is the
/// authority for *whether* a formula passes; this walk runs only to name the operator
/// a rejection is about, and asserts the two agree.
fn first_temporal_operator(formula: &Formula) -> Option<&'static str> {
    if formula.is_temporal_free() {
        return None;
    }
    fn walk(formula: &Formula) -> Option<&'static str> {
        match formula {
            Formula::Always { .. } => Some("always"),
            Formula::Eventually { .. } => Some("eventually"),
            Formula::LeadsTo { .. } => Some("leads_to"),
            Formula::Boolean { .. }
            | Formula::Predicate { .. }
            | Formula::Action { .. }
            | Formula::Compare { .. } => None,
            Formula::Not { operand } => walk(operand),
            Formula::And { operands } | Formula::Or { operands } => operands.iter().find_map(walk),
            Formula::Implies {
                antecedent,
                consequent,
            } => walk(antecedent).or_else(|| walk(consequent)),
            Formula::Iff { left, right } => walk(left).or_else(|| walk(right)),
            Formula::Forall { body, .. } | Formula::Exists { body, .. } => walk(body),
        }
    }
    // `is_temporal_free` said there is one; the walk finds which.
    walk(formula)
}

fn temporal_wire(kind: &str) -> Option<&'static str> {
    match kind {
        "always" => Some("always"),
        "eventually" => Some("eventually"),
        "leads_to" => Some("leads_to"),
        _ => None,
    }
}

/// Read a bare `$defs/state_formula` through the crate's one property-AST reader.
///
/// [`Formula::from_json`] is that reader's bare-formula entry point, so the rejections
/// a caller sees are the AST's own, named by the AST's own field paths — which is what
/// [`FairnessDecodeError::Condition`] carries and documents.
///
/// The result is normalized here rather than left to [`FairnessConstraint::new`],
/// because the two report a failed normalization differently: through this path it is
/// the *reader's* failure, `Condition(Normalize(..))`, and that is what
/// `fairness_adversarial_decode.rs` pins. W4 has already been applied structurally to
/// the document above, so no temporal node reaches CPNF-1 from here.
fn decode_condition(document: &Json) -> Result<Formula, FairnessDecodeError> {
    if let Some(operator) = temporal_operator_in_document(document) {
        return Err(FairnessDecodeError::Fairness(
            FairnessError::TemporalCondition { operator },
        ));
    }
    let ast = Formula::from_json(document)?;
    cpnf::normalize(&ast)
        .map_err(|error| FairnessDecodeError::Condition(PropertyDecodeError::Normalize(error)))
}

/// The `fairness` array: a set of constraints keyed by their (`kind`, `action`) pairs.
///
/// W1 requires that "no two `fairness` items may share a (`kind`, `action`) pair", so
/// this is a keyed set: [`FairnessSet::from_constraints`] rejects a duplicate rather
/// than keeping both, and the encoding emits constraints in key order.
///
/// The set MAY be empty, and an empty set is a statement, not an omission: "An empty
/// `assumptions`, `observers`, `fairness`, `nondeterminism`, or `abstraction_maps`
/// array is a *declaration that the set is empty*, and is classified as such; it is not
/// 'unspecified'" (RFC 0037). The distinction matters most here, where the empty set is
/// the *weakest* environment assumption and therefore the honest one.
#[derive(Debug, Clone)]
pub struct FairnessSet {
    constraints: BTreeMap<FairnessKey, FairnessConstraint>,
    identity: FairnessIdentity,
}

impl FairnessSet {
    /// Build a fairness set, enforcing W1.
    ///
    /// # Errors
    ///
    /// [`FairnessError::DuplicateKey`] — two constraints share a (`kind`, `action`)
    /// pair.
    pub fn from_constraints(
        constraints: impl IntoIterator<Item = FairnessConstraint>,
    ) -> Result<Self, FairnessError> {
        let mut keyed: BTreeMap<FairnessKey, FairnessConstraint> = BTreeMap::new();
        for constraint in constraints {
            if let Some(existing) = keyed.insert(constraint.key().clone(), constraint) {
                return Err(FairnessError::DuplicateKey {
                    key: existing.key().locator(),
                });
            }
        }
        let identity = FairnessIdentity::of_bytes(
            Json::Array(
                keyed
                    .values()
                    .map(FairnessConstraint::artifact_json)
                    .collect(),
            )
            .to_canonical_bytes(),
        );
        Ok(Self {
            constraints: keyed,
            identity,
        })
    }

    /// The `fairness` portion of the contract's identity preimage.
    #[must_use]
    pub const fn identity(&self) -> &FairnessIdentity {
        &self.identity
    }

    /// How many constraints the set carries.
    #[must_use]
    pub fn len(&self) -> usize {
        self.constraints.len()
    }

    /// Whether the contract declares that it assumes no fairness.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.constraints.is_empty()
    }

    /// The constraint under `key`, if any.
    #[must_use]
    pub fn get(&self, key: &FairnessKey) -> Option<&FairnessConstraint> {
        self.constraints.get(key)
    }

    /// Every constraint, in key order.
    pub fn iter(&self) -> impl Iterator<Item = &FairnessConstraint> {
        self.constraints.values()
    }

    /// Every unit key, in order: the membership half of RFC 0031's comparison.
    ///
    /// A key present on one side only classifies `added` or `removed`; a key present on
    /// both is classified by [`FairnessConstraint::condition`] under the antitone rule.
    #[must_use]
    pub fn unit_keys(&self) -> BTreeSet<&FairnessKey> {
        self.constraints.keys().collect()
    }

    /// The artifact-form JSON array.
    #[must_use]
    pub fn artifact_json(&self) -> Json {
        Json::Array(
            self.constraints
                .values()
                .map(FairnessConstraint::artifact_json)
                .collect(),
        )
    }

    /// The artifact-form bytes.
    #[must_use]
    pub fn to_artifact_bytes(&self) -> Vec<u8> {
        self.artifact_json().to_canonical_bytes()
    }

    /// Decode a fairness set from its artifact form.
    ///
    /// # Errors
    ///
    /// [`FairnessDecodeError`], including [`FairnessDecodeError::Fairness`] for the W1
    /// violation the array shape carries.
    pub fn decode(bytes: &[u8]) -> Result<Self, FairnessDecodeError> {
        Self::from_json(&Json::parse(bytes)?)
    }

    /// Decode a fairness set from an already-parsed JSON array.
    ///
    /// # Errors
    ///
    /// [`FairnessDecodeError`], as [`FairnessSet::decode`].
    pub fn from_json(json: &Json) -> Result<Self, FairnessDecodeError> {
        let items = json
            .as_array()
            .ok_or_else(|| FairnessDecodeError::TypeMismatch {
                field: "fairness",
                expected: "array",
                found: json.type_name(),
            })?;
        let constraints = items
            .iter()
            .map(FairnessConstraint::from_json)
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self::from_constraints(constraints)?)
    }
}

impl PartialEq for FairnessSet {
    fn eq(&self, other: &Self) -> bool {
        self.constraints == other.constraints
    }
}

impl Eq for FairnessSet {}

impl<'a> IntoIterator for &'a FairnessSet {
    type Item = &'a FairnessConstraint;
    type IntoIter = std::collections::btree_map::Values<'a, FairnessKey, FairnessConstraint>;

    fn into_iter(self) -> Self::IntoIter {
        self.constraints.values()
    }
}

// --- errors --------------------------------------------------------------------------------

/// Why a fairness constraint could not be built.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FairnessError {
    /// An action name is the empty string.
    EmptyAction,
    /// W4: a condition carries a temporal operator at some depth.
    TemporalCondition {
        /// The wire kind of the offending node: `always`, `eventually`, or
        /// `leads_to`.
        operator: &'static str,
    },
    /// W1: two constraints share a (`kind`, `action`) pair.
    DuplicateKey {
        /// The repeated key, rendered `kind:action`.
        key: String,
    },
    /// The condition could not be normalized to CPNF-1.
    Normalize(NormalizeError),
}

impl fmt::Display for FairnessError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyAction => f.write_str(
                "a fairness action is half of the constraint's classified-unit key (RFC 0031) and \
                 cannot be empty",
            ),
            Self::TemporalCondition { operator } => write!(
                f,
                "a fairness condition carries the temporal operator `{operator}`; W4 requires a \
                 state formula — no `always`, `eventually`, or `leads_to` node at any depth — \
                 because the condition is an enabling predicate over a single state"
            ),
            Self::DuplicateKey { key } => write!(
                f,
                "two fairness constraints share the key {key:?}; W1 forbids two items sharing a \
                 (kind, action) pair because the intent diff keys classification by it"
            ),
            Self::Normalize(error) => error.fmt(f),
        }
    }
}

impl core::error::Error for FairnessError {}

impl From<NormalizeError> for FairnessError {
    fn from(error: NormalizeError) -> Self {
        Self::Normalize(error)
    }
}

/// Why a byte string is not a fairness artifact.
///
/// Every variant is a rejection, never a repair. RFC 0037: "an intent contract is
/// never best-effort decoded".
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FairnessDecodeError {
    /// The bytes are not admissible canonical JSON.
    Json(JsonError),
    /// A required field is absent.
    MissingField {
        /// The field's path.
        field: &'static str,
    },
    /// A field carries the wrong JSON type.
    TypeMismatch {
        /// The field's path.
        field: &'static str,
        /// What the schema admits.
        expected: &'static str,
        /// What was found.
        found: &'static str,
    },
    /// An object carries a key the schema's `additionalProperties: false` forbids.
    UnknownField {
        /// The object's path.
        field: &'static str,
        /// The offending key.
        key: String,
    },
    /// `kind` carries a token outside the closed two-member vocabulary.
    UnknownToken {
        /// The field's path.
        field: &'static str,
        /// The unrecognized token.
        token: String,
    },
    /// The condition is not a property AST this build admits.
    ///
    /// The crate has one property-AST reader and these are its rejections, named by its
    /// own field paths (`formula.kind`, `compare.op`, `term.kind`, …).
    Condition(PropertyDecodeError),
    /// The decoded constraints do not form a well-formed set.
    Fairness(FairnessError),
}

impl fmt::Display for FairnessDecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Json(error) => error.fmt(f),
            Self::MissingField { field } => write!(f, "the required field `{field}` is absent"),
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
            Self::UnknownToken { field, token } => write!(
                f,
                "`{field}` carries the token {token:?}, which this build does not implement; an \
                 unrecognized token in a closed vocabulary is rejected, never ignored (RFC 0037)"
            ),
            Self::Condition(error) => error.fmt(f),
            Self::Fairness(error) => error.fmt(f),
        }
    }
}

impl core::error::Error for FairnessDecodeError {}

impl From<JsonError> for FairnessDecodeError {
    fn from(error: JsonError) -> Self {
        Self::Json(error)
    }
}

impl From<PropertyDecodeError> for FairnessDecodeError {
    fn from(error: PropertyDecodeError) -> Self {
        Self::Condition(error)
    }
}

impl From<FairnessError> for FairnessDecodeError {
    fn from(error: FairnessError) -> Self {
        Self::Fairness(error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::Identifier;

    fn action(name: &str) -> ActionName {
        ActionName::new(name).expect("a test action name is non-empty")
    }

    fn key(kind: FairnessKind, name: &str) -> FairnessKey {
        FairnessKey::new(kind, action(name))
    }

    fn predicate(name: &str) -> Formula {
        Formula::predicate(
            Identifier::new(name).expect("a test identifier is well formed"),
            Vec::new(),
        )
    }

    #[test]
    fn the_fairness_kind_vocabulary_is_closed_and_two_members_wide() {
        assert_eq!(FairnessKind::ALL.len(), 2);
        for kind in FairnessKind::ALL {
            assert_eq!(FairnessKind::from_wire(kind.wire()), Some(kind));
        }
        assert_eq!(FairnessKind::from_wire("Weak"), None);
        assert_eq!(FairnessKind::from_wire("unconditional"), None);
    }

    #[test]
    fn an_action_cannot_be_empty() {
        assert_eq!(ActionName::new(""), Err(FairnessError::EmptyAction));
        assert!(ActionName::new("SyncCompleted").is_ok());
    }

    #[test]
    fn the_locator_is_the_diff_schemas_rendering() {
        assert_eq!(
            key(FairnessKind::Weak, "SyncCompleted").locator(),
            "weak:SyncCompleted"
        );
        assert_eq!(
            key(FairnessKind::Strong, "a:b").locator(),
            "strong:a:b",
            "a colon in the action stays recoverable: the kind is colon-free"
        );
    }

    #[test]
    fn w4_rejects_every_temporal_operator_by_name() {
        for (formula, operator) in [
            (Formula::always(predicate("p")), "always"),
            (Formula::eventually(predicate("p")), "eventually"),
            (
                Formula::leads_to(predicate("p"), predicate("q")),
                "leads_to",
            ),
            // At depth, under a binder-free junction.
            (
                Formula::and(vec![predicate("p"), Formula::always(predicate("q"))])
                    .expect("two operands"),
                "always",
            ),
            (
                Formula::not(Formula::eventually(predicate("p"))),
                "eventually",
            ),
        ] {
            assert_eq!(
                FairnessConstraint::new(key(FairnessKind::Weak, "A"), Some(&formula)),
                Err(FairnessError::TemporalCondition { operator })
            );
        }
    }

    #[test]
    fn a_condition_is_stored_in_cpnf_1() {
        // `implies` is eliminated by N1, so an authored condition and its normal form
        // are one value with one identity.
        let authored = Formula::implies(predicate("p"), predicate("q"));
        let constraint = FairnessConstraint::new(key(FairnessKind::Weak, "A"), Some(&authored))
            .expect("temporal-free and normalizable");
        let encoded = String::from_utf8(constraint.to_artifact_bytes()).expect("utf-8");
        assert!(!encoded.contains("implies"), "{encoded}");
        assert!(encoded.contains(r#""kind":"or""#), "{encoded}");
    }

    #[test]
    fn unconditional_is_null_and_absent_is_the_same_meaning() {
        let constraint = FairnessConstraint::new(key(FairnessKind::Weak, "A"), None)
            .expect("unconditional is legal");
        assert!(constraint.is_unconditional());
        let encoded = String::from_utf8(constraint.to_artifact_bytes()).expect("utf-8");
        assert_eq!(encoded, r#"{"action":"A","condition":null,"kind":"weak"}"#);
        assert_eq!(
            constraint,
            FairnessConstraint::decode(br#"{"action":"A","kind":"weak"}"#).expect("decodes")
        );
    }

    #[test]
    fn the_identity_is_the_artifact_bytes_because_no_id2_member_lives_here() {
        let constraint = FairnessConstraint::new(
            key(FairnessKind::Strong, "SyncCompleted"),
            Some(&predicate("node_running")),
        )
        .expect("well formed");
        assert_eq!(
            constraint.identity().canonical_bytes(),
            constraint.to_artifact_bytes()
        );
    }

    #[test]
    fn w1_is_enforced_on_the_pair_and_not_on_the_action_alone() {
        let weak =
            FairnessConstraint::new(key(FairnessKind::Weak, "A"), None).expect("well formed");
        let strong =
            FairnessConstraint::new(key(FairnessKind::Strong, "A"), None).expect("well formed");
        // The same action under both kinds is two units, not a duplicate.
        let set = FairnessSet::from_constraints([weak.clone(), strong]).expect("two units");
        assert_eq!(set.len(), 2);
        assert_eq!(
            FairnessSet::from_constraints([weak.clone(), weak]),
            Err(FairnessError::DuplicateKey {
                key: "weak:A".to_owned()
            })
        );
        assert!(
            FairnessSet::from_constraints([])
                .expect("an empty set is legal")
                .is_empty()
        );
    }
}
