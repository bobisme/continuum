//! The `assumptions` field group: environmental assumptions the model may rely on
//! (PR-4 / IMPL-02).
//!
//! # What an assumption *is*, in this crate
//!
//! RFC 0037's field table gives the shape and RFC 0031 gives the unit of
//! classification. An assumption is four things, the same quadruple shape
//! [`crate::property::Claim`] has, with the *meaning* of the last two parts
//! inverted:
//!
//! | Part | Type | Source |
//! |---|---|---|
//! | its **unit key** | [`UnitKey`] | RFC 0031: the `assumptions` field is classified "one assumption, keyed by `id`", exactly as `properties` |
//! | its **formula**, normalized | [`crate::property::PropertyExpression`] | the *same* CPNF-1 carrier claims use — "`assumptions[].expression` reuses `PropertyExpression`" (`crate::lib` module docs) |
//! | its **classification** | [`AssumptionClassification`] | RFC 0037: an optional closed six-member enum naming the kind of environment fact assumed |
//! | its **fidelity profile** | [`FidelityProfile`] | RFC 0002/RFC 0037: an optional closed four-member enum; profiles are explicitly NOT ordered |
//!
//! [`Assumption`] is that quadruple, and — like [`crate::property::Claim`] — it is
//! immutable: no `&mut` method, no setter. RFC 0037's "Accepted contracts are
//! immutable […] it mints a new `in_*`" applies here exactly as it does to claims.
//!
//! # Direction and policy valence differ from `properties`
//!
//! The comparison is the same order as `properties` (RFC 0031, "`assumptions`":
//! "computed on `expression` exactly as for claims — the same order, on the same
//! normal form"), but the *meaning* of a direction inverts:
//!
//! > an assumption `strengthened` admits fewer environments and makes verification
//! > easier, and is the gaming move plan §5.1 names ("adding a fairness assumption
//! > that schedules away the bug" is its fairness sibling).
//! >
//! > — `notes/plan/rfcs/0031-semantic-and-intent-diff.md`, "`assumptions`"
//!
//! This module does not compute that direction (see "What is not here" below); it
//! exists so the direction has something sound to be computed *over* — the same
//! service `property::PropertyExpression` provides for claims.
//!
//! # Declaredness is not a default: `classification` and `fidelity_profile`
//!
//! Both fields are OPTIONAL and carry no `null` spelling in the schema (plain
//! string enums, not `["string", "null"]`), which makes their absence a real third
//! state rather than a defaultable one:
//!
//! > `assumptions[].classification` and `assumptions[].fidelity_profile` are
//! > optional, and absence means *undeclared*, which is distinct from every
//! > declared value: a change from absent to present, or the reverse, is a
//! > declaredness change and classifies `incomparable` […], never `unchanged`.
//! >
//! > — RFC 0037, "Field types and enums"
//!
//! So unlike `claims[].observer` (which [`crate::property`] normalizes so that
//! absent and explicit `null` are one meaning with one spelling, because the
//! schema admits `null` there), `classification` and `fidelity_profile` have no
//! null form to collapse onto: [`Assumption::from_json`] rejects a `null` for
//! either ([`AssumptionDecodeError::TypeMismatch`], expecting a string), and
//! [`Assumption::artifact_json`] omits the key entirely when the value is
//! undeclared. Presence or absence of the *key* is the only spelling of
//! declaredness, and it participates in identity (ID2's exclusion list does not
//! name either field, so both are in the preimage — a declaredness change moves
//! `in_*`, exactly as RFC 0031 says it must be classified, never `unchanged`).
//!
//! `fidelity_profile`'s four values are RFC 0002 domain-pack fidelity levels and
//! are explicitly not an order: "RFC 0002's fidelity profiles are explicitly not
//! ordered, so no direction may be inferred." [`FidelityProfile`] therefore derives
//! no [`Ord`], unlike [`AssumptionClassification`], which is ordered only because
//! every closed enum in this crate is (for stable `BTreeMap`/`BTreeSet` use), not
//! because RFC 0037 assigns classification a direction — it does not.
//!
//! # Why this module shares `property.rs`'s reader, and keeps its own error type
//!
//! `assumptions[].expression` has the identical `$defs/property_expression` shape
//! `claims[].expression` does, so this module reuses
//! [`crate::property::PropertyExpression`] as a *type* — its normalization, its
//! two byte spellings (artifact vs. identity preimage), and its identity
//! discipline are exactly the claim's — and now reuses its *reader* too, through
//! [`PropertyExpression::from_json`]. It did not always: while the eight PR-4
//! field-group bones were in flight in separate workspace copies of this crate,
//! `property.rs`'s recursive-descent formula/term parser was private to that
//! module and a shared-module change was an edit that had to be designed around
//! rather than made, so this module carried a second copy of the same descent
//! built only from `property.rs`'s public surface. The copy raised its own flag
//! against itself; the flag is answered and the copy is gone. One grammar has one
//! reader — `crate::codec` — because two readers is how a document comes to have
//! two meanings.
//!
//! What does *not* move is the error type. A rejection has to name the field the
//! author wrote, and this group's fields are `assumptions[].id`,
//! `assumptions[].expression`, `assumptions[].classification`, and
//! `assumptions[].fidelity_profile`. So [`AssumptionDecodeError`] stays this
//! module's own vocabulary and the shared reader's [`PropertyDecodeError`] is
//! translated into it variant for variant, with every message preserved verbatim.
//! Sharing a reader is not the same as sharing a voice.
//!
//! # What is not here
//!
//! - **Classification is not here.** Whether an edit to an assumption's
//!   expression is `strengthened` or `unknown` is RFC 0031's question, deferred
//!   exactly as `property.rs` defers it for claims.
//! - **W3 needs the rest of the contract.** [`AssumptionSet::check_fragments`] is
//!   this group's half of the same check `ClaimSet::check_fragments` performs for
//!   claims; `scope.fragments` comes from a sibling group.
//! - **The contract identity is not here.** This module supplies the
//!   `assumptions` portion of the ID1 preimage through [`AssumptionSet::identity`]
//!   and nothing more.

use core::fmt;
use std::collections::{BTreeMap, BTreeSet};

use crate::ast::{AstError, Fragment};
use crate::canonical_json::{Json, JsonError};
use crate::codec::{known_keys, object, string_field};
use crate::cpnf::NormalizeError;
use crate::identity::canonical_identity;
use crate::property::{PropertyDecodeError, PropertyError, PropertyExpression};

/// The classified-unit key of an assumption: `assumptions[].id`.
///
/// A separate type from [`crate::property::UnitKey`], deliberately: RFC 0037
/// flags that "every group with a declared unit key (`assumptions`, `observers`,
/// `nondeterminism`, `abstraction_maps`, `fairness`) has the same question, and
/// they must all answer it the same way" — the same *discipline* (a non-empty
/// string, `Ord` by UTF-8 byte order, excluded from nothing in the identity
/// preimage), not a shared Rust type, since the sibling groups are independent
/// bones built in independent workspace copies of this crate.
///
/// [`Ord`] is byte order over UTF-8, which is ID5's code-point order — the same
/// property [`crate::property::UnitKey`] states, for the same reason: it is what
/// makes [`AssumptionSet`]'s encoding stable and its diff keying deterministic.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct UnitKey(String);

impl UnitKey {
    /// Validate and wrap an assumption id.
    ///
    /// # Errors
    ///
    /// [`AssumptionError::EmptyUnitKey`] for the empty string.
    pub fn new(id: &str) -> Result<Self, AssumptionError> {
        if id.is_empty() {
            return Err(AssumptionError::EmptyUnitKey);
        }
        Ok(Self(id.to_owned()))
    }

    /// The key.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for UnitKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// What kind of environment fact an assumption states: `assumptions[].classification`,
/// the closed six-member enum.
///
/// Optional and undeclared by default; see the module documentation's
/// "Declaredness is not a default" section. A change in `classification` alone,
/// with the expression held fixed, MUST classify `incomparable` (RFC 0031): the
/// value carries no direction of its own, so nothing here provides one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum AssumptionClassification {
    /// A fact about the surrounding deployment environment.
    Environment,
    /// A fact about scheduling behavior.
    Scheduler,
    /// A fact about timing.
    Timing,
    /// A fact about storage behavior.
    Storage,
    /// A fact about the network.
    Network,
    /// A trust assumption about another party or component.
    Trust,
}

impl AssumptionClassification {
    /// Every classification, in schema-enum order.
    pub const ALL: [Self; 6] = [
        Self::Environment,
        Self::Scheduler,
        Self::Timing,
        Self::Storage,
        Self::Network,
        Self::Trust,
    ];

    /// The wire literal.
    #[must_use]
    pub const fn wire(self) -> &'static str {
        match self {
            Self::Environment => "environment",
            Self::Scheduler => "scheduler",
            Self::Timing => "timing",
            Self::Storage => "storage",
            Self::Network => "network",
            Self::Trust => "trust",
        }
    }

    /// Recover a classification from its wire literal.
    ///
    /// Returns `None` for an unrecognized token; the caller rejects rather than
    /// guesses (RFC 0037: "Forward compatibility is achieved by rejecting, never
    /// by ignoring").
    #[must_use]
    pub fn from_wire(token: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|kind| kind.wire() == token)
    }
}

impl fmt::Display for AssumptionClassification {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.wire())
    }
}

/// Which domain-pack fidelity an assumption relies on: `assumptions[].fidelity_profile`,
/// the closed four-member enum (RFC 0002).
///
/// Deliberately carries no [`Ord`]: "RFC 0002's fidelity profiles are explicitly
/// not ordered, so no direction may be inferred" (RFC 0037). Every other closed
/// enum in this crate derives `Ord` for stable container use; this one does not,
/// so that a future call site cannot accidentally sort or compare profiles as if
/// a coarser-to-finer direction existed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FidelityProfile {
    /// An idealized model with no platform-specific compromise.
    Ideal,
    /// A model qualified against a contractual specification.
    Contractual,
    /// A model qualified against an observed platform.
    PlatformQualified,
    /// A model qualified against an adversarial envelope.
    AdversarialEnvelope,
}

impl FidelityProfile {
    /// Every profile, in schema-enum order.
    pub const ALL: [Self; 4] = [
        Self::Ideal,
        Self::Contractual,
        Self::PlatformQualified,
        Self::AdversarialEnvelope,
    ];

    /// The wire literal.
    #[must_use]
    pub const fn wire(self) -> &'static str {
        match self {
            Self::Ideal => "ideal",
            Self::Contractual => "contractual",
            Self::PlatformQualified => "platform-qualified",
            Self::AdversarialEnvelope => "adversarial-envelope",
        }
    }

    /// Recover a profile from its wire literal, or `None` for an unknown token.
    #[must_use]
    pub fn from_wire(token: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|profile| profile.wire() == token)
    }
}

impl fmt::Display for FidelityProfile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.wire())
    }
}

canonical_identity! {
    /// The canonical identity of an assumption artifact (one assumption, or the whole
    /// `assumptions` set).
    ///
    /// Mirrors [`crate::property::PropertyIdentity`] exactly, for the same reason
    /// (ADR-0013): this type *is* the identity-preimage bytes, not a struct holding a
    /// digest, so two assumptions (or two sets) are the same precisely when their
    /// canonical encodings agree. The discipline itself lives in
    /// `crate::identity::CanonicalIdentity`; this is the `assumptions` group's name
    /// for it.
    AssumptionIdentity
}

/// One assumption: `assumptions[]`.
///
/// Immutable, exactly as [`crate::property::Claim`] is: no `&mut` method, no
/// setter, no identity-preserving constructor. A "changed assumption" is a new
/// value with a new [`AssumptionIdentity`].
///
/// [`PartialEq`] is hand-written and compares the four declared parts, never the
/// derived [`identity`](Self::identity) — the same discipline `Claim` uses, for
/// the same reason.
#[derive(Debug, Clone)]
pub struct Assumption {
    unit: UnitKey,
    expression: PropertyExpression,
    classification: Option<AssumptionClassification>,
    fidelity_profile: Option<FidelityProfile>,
    identity: AssumptionIdentity,
}

impl Assumption {
    /// Build an assumption and derive its identity.
    #[must_use]
    pub fn new(
        unit: UnitKey,
        expression: PropertyExpression,
        classification: Option<AssumptionClassification>,
        fidelity_profile: Option<FidelityProfile>,
    ) -> Self {
        let identity = AssumptionIdentity::of_bytes(
            assumption_json(
                &unit,
                &expression,
                classification,
                fidelity_profile,
                Layer::Identity,
            )
            .to_canonical_bytes(),
        );
        Self {
            unit,
            expression,
            classification,
            fidelity_profile,
            identity,
        }
    }

    /// The classified-unit key.
    #[must_use]
    pub const fn unit(&self) -> &UnitKey {
        &self.unit
    }

    /// The CPNF-1 expression this assumption states.
    #[must_use]
    pub const fn expression(&self) -> &PropertyExpression {
        &self.expression
    }

    /// The declared classification, if any.
    #[must_use]
    pub const fn classification(&self) -> Option<AssumptionClassification> {
        self.classification
    }

    /// The declared fidelity profile, if any.
    #[must_use]
    pub const fn fidelity_profile(&self) -> Option<FidelityProfile> {
        self.fidelity_profile
    }

    /// The assumption's canonical identity.
    #[must_use]
    pub const fn identity(&self) -> &AssumptionIdentity {
        &self.identity
    }

    /// The artifact-form JSON: what a contract document carries.
    #[must_use]
    pub fn artifact_json(&self) -> Json {
        assumption_json(
            &self.unit,
            &self.expression,
            self.classification,
            self.fidelity_profile,
            Layer::Artifact,
        )
    }

    /// The artifact-form bytes, under the ID5 canonical-JSON rules.
    #[must_use]
    pub fn to_artifact_bytes(&self) -> Vec<u8> {
        self.artifact_json().to_canonical_bytes()
    }

    /// Decode an assumption from its artifact form.
    ///
    /// # Errors
    ///
    /// [`AssumptionDecodeError`], naming the field or the token that failed.
    pub fn decode(bytes: &[u8]) -> Result<Self, AssumptionDecodeError> {
        let json = Json::parse(bytes)?;
        Self::from_json(&json)
    }

    /// Decode an assumption from an already-parsed JSON value.
    ///
    /// # Errors
    ///
    /// [`AssumptionDecodeError`], as [`Assumption::decode`].
    pub fn from_json(json: &Json) -> Result<Self, AssumptionDecodeError> {
        let fields = object(json, "assumptions[]")?;
        known_keys(
            fields,
            "assumptions[]",
            &["classification", "expression", "fidelity_profile", "id"],
        )?;
        let unit = unit_key(string_field(fields, "assumptions[].id")?)?;
        let expression = PropertyExpression::from_json(fields.get("expression").ok_or(
            AssumptionDecodeError::MissingField {
                field: "assumptions[].expression",
            },
        )?)?;
        let classification = match fields.get("classification") {
            None => None,
            Some(Json::String(token)) => {
                Some(AssumptionClassification::from_wire(token).ok_or_else(|| {
                    AssumptionDecodeError::UnknownToken {
                        field: "assumptions[].classification",
                        token: token.clone(),
                    }
                })?)
            }
            Some(other) => {
                return Err(AssumptionDecodeError::TypeMismatch {
                    field: "assumptions[].classification",
                    expected: "string",
                    found: other.type_name(),
                });
            }
        };
        let fidelity_profile = match fields.get("fidelity_profile") {
            None => None,
            Some(Json::String(token)) => {
                Some(FidelityProfile::from_wire(token).ok_or_else(|| {
                    AssumptionDecodeError::UnknownToken {
                        field: "assumptions[].fidelity_profile",
                        token: token.clone(),
                    }
                })?)
            }
            Some(other) => {
                return Err(AssumptionDecodeError::TypeMismatch {
                    field: "assumptions[].fidelity_profile",
                    expected: "string",
                    found: other.type_name(),
                });
            }
        };
        Ok(Self::new(
            unit,
            expression,
            classification,
            fidelity_profile,
        ))
    }
}

impl PartialEq for Assumption {
    fn eq(&self, other: &Self) -> bool {
        self.unit == other.unit
            && self.expression == other.expression
            && self.classification == other.classification
            && self.fidelity_profile == other.fidelity_profile
    }
}

impl Eq for Assumption {}

/// Which of the two spellings an assumption is being rendered into.
///
/// Only the nested `expression` differs between them (ID2 excludes
/// `expression.source`/`expression.normal_form` from the preimage); `id`,
/// `classification`, and `fidelity_profile` are in both, because none of the
/// three is on ID2's closed exclusion list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Layer {
    /// The ID2-reduced identity preimage.
    Identity,
    /// The schema-conformant contract document form.
    Artifact,
}

fn assumption_json(
    unit: &UnitKey,
    expression: &PropertyExpression,
    classification: Option<AssumptionClassification>,
    fidelity_profile: Option<FidelityProfile>,
    layer: Layer,
) -> Json {
    let expression_json = match layer {
        Layer::Identity => expression.identity_preimage_json(),
        Layer::Artifact => expression.artifact_json(),
    };
    let mut fields = vec![
        ("expression".to_owned(), expression_json),
        ("id".to_owned(), Json::String(unit.as_str().to_owned())),
    ];
    // Absent, never `null`: neither key has a null spelling in the schema, and
    // declaredness itself is part of the meaning (module documentation).
    if let Some(classification) = classification {
        fields.push((
            "classification".to_owned(),
            Json::String(classification.wire().to_owned()),
        ));
    }
    if let Some(profile) = fidelity_profile {
        fields.push((
            "fidelity_profile".to_owned(),
            Json::String(profile.wire().to_owned()),
        ));
    }
    Json::object(fields).unwrap_or(Json::Null)
}

/// The `assumptions` array: a set of assumptions keyed by their unit keys.
///
/// Unlike [`crate::property::ClaimSet`], this set MAY be empty: "Every
/// array-valued group MAY be empty except `claims`" (RFC 0037), and an empty
/// `assumptions` array is itself a declaration — "the set is empty" — not an
/// absence.
///
/// As with `ClaimSet`, reordering the authored array does not change the
/// identity: encoding walks the `BTreeMap` in unit-key order, so authored order
/// is a reformatting (ID7), never a change.
#[derive(Debug, Clone)]
pub struct AssumptionSet {
    assumptions: BTreeMap<UnitKey, Assumption>,
    identity: AssumptionIdentity,
}

impl AssumptionSet {
    /// Build an assumption set, enforcing W1.
    ///
    /// # Errors
    ///
    /// [`AssumptionError::DuplicateUnitKey`] — W1: `assumptions[].id` must be
    /// unique within the array, for the same soundness reason claim ids must be.
    pub fn from_assumptions(
        assumptions: impl IntoIterator<Item = Assumption>,
    ) -> Result<Self, AssumptionError> {
        let mut keyed: BTreeMap<UnitKey, Assumption> = BTreeMap::new();
        for assumption in assumptions {
            if let Some(existing) = keyed.insert(assumption.unit().clone(), assumption) {
                return Err(AssumptionError::DuplicateUnitKey {
                    unit: existing.unit().to_string(),
                });
            }
        }
        let identity = AssumptionIdentity::of_bytes(
            Json::Array(
                keyed
                    .values()
                    .map(|assumption| {
                        assumption_json(
                            assumption.unit(),
                            assumption.expression(),
                            assumption.classification(),
                            assumption.fidelity_profile(),
                            Layer::Identity,
                        )
                    })
                    .collect(),
            )
            .to_canonical_bytes(),
        );
        Ok(Self {
            assumptions: keyed,
            identity,
        })
    }

    /// The `assumptions` portion of the contract's identity preimage.
    #[must_use]
    pub const fn identity(&self) -> &AssumptionIdentity {
        &self.identity
    }

    /// The `assumptions` portion of the contract's identity preimage, as JSON.
    ///
    /// The same `Layer::Identity` spelling [`AssumptionSet::identity`] is taken over —
    /// every `expression` reduced to its ID2 form, with `source` and `normal_form`
    /// dropped — exposed so that [`crate::contract::IntentContract`] can compose the
    /// whole-document preimage out of its groups' preimages rather than re-deriving
    /// the exclusion list a second time. `identity_preimage_json().to_canonical_bytes()
    /// == identity().canonical_bytes()`, and `tests/` asserts it.
    #[must_use]
    pub fn identity_preimage_json(&self) -> Json {
        Json::Array(
            self.assumptions
                .values()
                .map(|assumption| {
                    assumption_json(
                        assumption.unit(),
                        assumption.expression(),
                        assumption.classification(),
                        assumption.fidelity_profile(),
                        Layer::Identity,
                    )
                })
                .collect(),
        )
    }

    /// How many assumptions the set carries. MAY be zero.
    #[must_use]
    pub fn len(&self) -> usize {
        self.assumptions.len()
    }

    /// Whether the set carries no assumptions.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.assumptions.is_empty()
    }

    /// The assumption under `unit`, if any.
    #[must_use]
    pub fn get(&self, unit: &UnitKey) -> Option<&Assumption> {
        self.assumptions.get(unit)
    }

    /// Every assumption, in unit-key order.
    pub fn iter(&self) -> impl Iterator<Item = &Assumption> {
        self.assumptions.values()
    }

    /// The artifact-form JSON array.
    #[must_use]
    pub fn artifact_json(&self) -> Json {
        Json::Array(
            self.assumptions
                .values()
                .map(Assumption::artifact_json)
                .collect(),
        )
    }

    /// The artifact-form bytes.
    #[must_use]
    pub fn to_artifact_bytes(&self) -> Vec<u8> {
        self.artifact_json().to_canonical_bytes()
    }

    /// Decode an assumption set from its artifact form.
    ///
    /// # Errors
    ///
    /// [`AssumptionDecodeError`], including the W1 violation the array shape
    /// carries. Unlike [`crate::property::ClaimSet::decode`], an empty array
    /// decodes successfully — the schema places no `minItems` on `assumptions`.
    pub fn decode(bytes: &[u8]) -> Result<Self, AssumptionDecodeError> {
        let json = Json::parse(bytes)?;
        Self::from_json(&json)
    }

    /// Decode an assumption set from an already-parsed JSON array.
    ///
    /// # Errors
    ///
    /// [`AssumptionDecodeError`], as [`AssumptionSet::decode`].
    pub fn from_json(json: &Json) -> Result<Self, AssumptionDecodeError> {
        let items = json
            .as_array()
            .ok_or_else(|| AssumptionDecodeError::TypeMismatch {
                field: "assumptions",
                expected: "array",
                found: json.type_name(),
            })?;
        let assumptions = items
            .iter()
            .map(Assumption::from_json)
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self::from_assumptions(assumptions)?)
    }

    /// W3's expression half, given the contract's declared `scope.fragments`.
    ///
    /// The same rule [`crate::property::ClaimSet::check_fragments`] enforces for
    /// claims: `assumptions[].expression` is the identical `property_expression`
    /// shape, so W3's "an expression's `fragment` […] MUST be a member of
    /// `scope.fragments`" applies here too.
    ///
    /// # Errors
    ///
    /// [`WellFormednessError::NoDeclaredFragments`],
    /// [`WellFormednessError::UndeclaredFragment`], or
    /// [`WellFormednessError::AmbiguousFragment`].
    pub fn check_fragments(
        &self,
        declared: &BTreeSet<Fragment>,
    ) -> Result<(), WellFormednessError> {
        if declared.is_empty() {
            return Err(WellFormednessError::NoDeclaredFragments);
        }
        for assumption in self.assumptions.values() {
            match assumption.expression().fragment() {
                Some(fragment) if !declared.contains(&fragment) => {
                    return Err(WellFormednessError::UndeclaredFragment {
                        unit: assumption.unit().to_string(),
                        fragment,
                    });
                }
                Some(_) => {}
                None if declared.len() > 1 => {
                    return Err(WellFormednessError::AmbiguousFragment {
                        unit: assumption.unit().to_string(),
                        declared: declared.len(),
                    });
                }
                None => {}
            }
        }
        Ok(())
    }
}

impl PartialEq for AssumptionSet {
    fn eq(&self, other: &Self) -> bool {
        self.assumptions == other.assumptions
    }
}

impl Eq for AssumptionSet {}

impl<'a> IntoIterator for &'a AssumptionSet {
    type Item = &'a Assumption;
    type IntoIter = std::collections::btree_map::Values<'a, UnitKey, Assumption>;

    fn into_iter(self) -> Self::IntoIter {
        self.assumptions.values()
    }
}

// --- decoding helpers ----------------------------------------------------------------------
//
// The formula/term recursive descent lives in `crate::codec`, the crate's one reader
// for the `$defs/formula`/`$defs/term` node set. This module used to carry a second
// copy of it, built only from `property.rs`'s public surface because that module's
// reader was private and the sibling PR-4 bones were in flight; the copy is gone and
// the flag it raised — "the formula/term decoder should move to a shared, `pub(crate)`
// module both `property` and `assumptions` (and, later, `fairness` …) import" — is
// answered. What stays here is what is genuinely this group's: the `assumptions[].id`
// wrapper, and the variant-for-variant translation of the shared reader's rejections
// into this module's own vocabulary, so an error still names `assumptions[].*`.

fn unit_key(id: &str) -> Result<UnitKey, AssumptionDecodeError> {
    Ok(UnitKey::new(id)?)
}

// --- errors --------------------------------------------------------------------------------

/// Why an assumption could not be built.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AssumptionError {
    /// An assumption id is the empty string.
    EmptyUnitKey,
    /// Two assumptions share an `assumptions[].id` (W1).
    DuplicateUnitKey {
        /// The repeated key.
        unit: String,
    },
}

impl fmt::Display for AssumptionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyUnitKey => f.write_str(
                "an assumption id is its classified-unit key (RFC 0031) and cannot be empty",
            ),
            Self::DuplicateUnitKey { unit } => write!(
                f,
                "two assumptions share the id {unit:?}; W1 requires unique unit keys because the \
                 intent diff keys classification by them"
            ),
        }
    }
}

impl core::error::Error for AssumptionError {}

/// Why a byte string is not an assumption artifact.
///
/// Every variant is a rejection, never a repair — RFC 0037: "an intent contract
/// is never best-effort decoded".
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AssumptionDecodeError {
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
    /// A closed vocabulary carries a token this build does not implement.
    UnknownToken {
        /// The field's path.
        field: &'static str,
        /// The unrecognized token.
        token: String,
    },
    /// An expression declared `normal_form: "cpnf-1"` and its AST is not in CPNF-1.
    FalseNormalForm,
    /// The decoded AST is not a shape the schema admits.
    Ast(AstError),
    /// The decoded AST could not be normalized.
    Normalize(NormalizeError),
    /// The decoded assumptions do not form a well-formed set.
    Assumption(AssumptionError),
    /// A failure surfaced by the reused [`PropertyExpression`] machinery that
    /// does not fit this module's own vocabulary.
    ///
    /// Structurally unreachable today — [`PropertyExpression::normalized`] only
    /// ever fails with [`PropertyError::Normalize`], which is mapped to
    /// [`AssumptionDecodeError::Normalize`] instead — but kept rather than
    /// panicking on an unexpected variant, because a trust-boundary decoder must
    /// not panic even on a case its own analysis says cannot occur (a future,
    /// legitimate change to `property.rs` must not turn into a panic here).
    Property(PropertyError),
}

impl fmt::Display for AssumptionDecodeError {
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
            Self::FalseNormalForm => f.write_str(
                "the expression declares normal_form \"cpnf-1\" and its AST is not in CPNF-1; a \
                 declaration is verified by re-normalizing (RFC 0037 N9)",
            ),
            Self::Ast(error) => error.fmt(f),
            Self::Normalize(error) => error.fmt(f),
            Self::Assumption(error) => error.fmt(f),
            Self::Property(error) => error.fmt(f),
        }
    }
}

impl core::error::Error for AssumptionDecodeError {}

impl From<JsonError> for AssumptionDecodeError {
    fn from(error: JsonError) -> Self {
        Self::Json(error)
    }
}

impl From<AstError> for AssumptionDecodeError {
    fn from(error: AstError) -> Self {
        Self::Ast(error)
    }
}

impl From<NormalizeError> for AssumptionDecodeError {
    fn from(error: NormalizeError) -> Self {
        Self::Normalize(error)
    }
}

impl From<AssumptionError> for AssumptionDecodeError {
    fn from(error: AssumptionError) -> Self {
        Self::Assumption(error)
    }
}

impl From<PropertyError> for AssumptionDecodeError {
    fn from(error: PropertyError) -> Self {
        match error {
            PropertyError::Normalize(error) => Self::Normalize(error),
            other => Self::Property(other),
        }
    }
}

/// The shared reader's rejections in this module's vocabulary.
///
/// `crate::codec` is the crate's one reader for `$defs/property_expression`, and it
/// speaks [`PropertyDecodeError`] — the AST-level field paths (`formula.kind`,
/// `compare.op`, `expression.normal_form`, …) that are the same wherever the
/// expression is carried. This translation is total and variant-for-variant, so an
/// assumption's rejection is exactly the rejection it was when this module carried its
/// own copy of the reader: same variant, same field, same message. It exists rather
/// than a shared error type because the *outer* paths differ — a claim's rejection
/// names `claims[].id`, an assumption's names `assumptions[].id` — and a decoder that
/// named the wrong group's field would be worse than a duplicated one.
impl From<PropertyDecodeError> for AssumptionDecodeError {
    fn from(error: PropertyDecodeError) -> Self {
        match error {
            PropertyDecodeError::Json(error) => Self::Json(error),
            PropertyDecodeError::MissingField { field } => Self::MissingField { field },
            PropertyDecodeError::TypeMismatch {
                field,
                expected,
                found,
            } => Self::TypeMismatch {
                field,
                expected,
                found,
            },
            PropertyDecodeError::UnknownField { field, key } => Self::UnknownField { field, key },
            PropertyDecodeError::UnknownToken { field, token } => {
                Self::UnknownToken { field, token }
            }
            PropertyDecodeError::FalseNormalForm => Self::FalseNormalForm,
            PropertyDecodeError::Ast(error) => Self::Ast(error),
            PropertyDecodeError::Normalize(error) => Self::Normalize(error),
            // `PropertyExpression::normalized`'s own failure, which the reader raises
            // through the same `From<PropertyError>` this module already applies.
            PropertyDecodeError::Property(error) => Self::from(error),
        }
    }
}

/// Why an assumption set is not well formed against the rest of a contract.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WellFormednessError {
    /// W3: `scope.fragments` is empty.
    NoDeclaredFragments,
    /// W3: an expression declares a fragment `scope.fragments` does not.
    UndeclaredFragment {
        /// The assumption's unit key.
        unit: String,
        /// The undeclared fragment.
        fragment: Fragment,
    },
    /// W3: an expression omits its fragment where more than one is declared.
    AmbiguousFragment {
        /// The assumption's unit key.
        unit: String,
        /// How many fragments the contract declares.
        declared: usize,
    },
}

impl fmt::Display for WellFormednessError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoDeclaredFragments => f.write_str(
                "scope.fragments is empty; every unit would classify unsupported and every \
                 promotion would be blocked, silently (W3)",
            ),
            Self::UndeclaredFragment { unit, fragment } => write!(
                f,
                "assumption {unit:?} is stated in the {fragment} fragment, which \
                 scope.fragments does not declare (W3)"
            ),
            Self::AmbiguousFragment { unit, declared } => write!(
                f,
                "assumption {unit:?} declares no fragment and the contract declares {declared}; \
                 \"the intent's scope.fragments apply\" has no single reading (W3)"
            ),
        }
    }
}

impl core::error::Error for WellFormednessError {}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::ast::{ActionModality, Formula, Identifier};

    fn ident(name: &str) -> Identifier {
        Identifier::new(name).expect("a test identifier is well formed")
    }

    fn unit(id: &str) -> UnitKey {
        UnitKey::new(id).expect("a test unit key is non-empty")
    }

    fn predicate(name: &str) -> Formula {
        Formula::predicate(ident(name), Vec::new())
    }

    fn expression(ast: &Formula) -> PropertyExpression {
        PropertyExpression::normalized(ast, Some(Fragment::Finite), None).expect("normalizes")
    }

    fn utf8(bytes: Vec<u8>) -> String {
        String::from_utf8(bytes).expect("canonical output is UTF-8")
    }

    // --- the four parts of an assumption ------------------------------------------

    #[test]
    fn an_assumption_carries_its_unit_key_expression_classification_and_fidelity_profile() {
        let assumption = Assumption::new(
            unit("A-storage"),
            PropertyExpression::normalized(
                &Formula::always(Formula::implies(
                    Formula::action(ident("SyncCompleted"), ActionModality::Occurs),
                    predicate("stable_recovery"),
                )),
                Some(Fragment::Finite),
                Some("always (SyncCompleted implies stable_recovery)".to_owned()),
            )
            .expect("normalizes"),
            Some(AssumptionClassification::Storage),
            Some(FidelityProfile::Contractual),
        );
        assert_eq!(assumption.unit().as_str(), "A-storage");
        assert_eq!(
            assumption.classification(),
            Some(AssumptionClassification::Storage)
        );
        assert_eq!(
            assumption.fidelity_profile(),
            Some(FidelityProfile::Contractual)
        );
        // `implies` is gone: the stored AST is the normal form.
        let ast = utf8(crate::cpnf::encode(assumption.expression().ast()));
        assert!(!ast.contains("implies"), "{ast}");
    }

    #[test]
    fn a_unit_key_cannot_be_empty() {
        assert_eq!(UnitKey::new(""), Err(AssumptionError::EmptyUnitKey));
        assert!(UnitKey::new("A-1").is_ok());
    }

    #[test]
    fn the_classification_vocabulary_is_closed() {
        for kind in AssumptionClassification::ALL {
            assert_eq!(AssumptionClassification::from_wire(kind.wire()), Some(kind));
        }
        assert_eq!(AssumptionClassification::ALL.len(), 6);
        assert_eq!(AssumptionClassification::from_wire("Storage"), None);
        assert_eq!(AssumptionClassification::from_wire("hardware"), None);
    }

    #[test]
    fn the_fidelity_profile_vocabulary_is_closed_and_carries_no_order() {
        for profile in FidelityProfile::ALL {
            assert_eq!(FidelityProfile::from_wire(profile.wire()), Some(profile));
        }
        assert_eq!(FidelityProfile::ALL.len(), 4);
        assert_eq!(FidelityProfile::from_wire("Ideal"), None);
        assert_eq!(FidelityProfile::from_wire("adversarial_envelope"), None);
    }

    // --- declaredness is part of identity -----------------------------------------

    #[test]
    fn a_declaredness_change_moves_the_identity_never_collapsing_to_the_same_bytes() {
        let bare = Assumption::new(unit("A"), expression(&predicate("p")), None, None);
        let classified = Assumption::new(
            unit("A"),
            expression(&predicate("p")),
            Some(AssumptionClassification::Environment),
            None,
        );
        assert_ne!(bare, classified);
        assert_ne!(bare.identity(), classified.identity());
        assert!(
            !bare
                .to_artifact_bytes()
                .windows(b"classification".len())
                .any(|w| w == b"classification"),
            "an undeclared classification must not appear in the artifact form"
        );
        assert!(
            classified
                .to_artifact_bytes()
                .windows(b"classification".len())
                .any(|w| w == b"classification")
        );
    }

    // --- the whole set --------------------------------------------------------------

    #[test]
    fn an_empty_assumption_set_is_a_declaration_not_an_error() {
        let set = AssumptionSet::from_assumptions(Vec::new()).expect("empty is allowed");
        assert!(set.is_empty());
        assert_eq!(set.to_artifact_bytes(), b"[]");
    }

    #[test]
    fn w1_rejects_a_duplicate_id() {
        let a = Assumption::new(unit("A"), expression(&predicate("p")), None, None);
        let b = Assumption::new(unit("A"), expression(&predicate("q")), None, None);
        assert_eq!(
            AssumptionSet::from_assumptions(vec![a, b]),
            Err(AssumptionError::DuplicateUnitKey {
                unit: "A".to_owned()
            })
        );
    }

    #[test]
    fn reordering_assumptions_does_not_change_the_sets_identity() {
        let a = Assumption::new(unit("A"), expression(&predicate("p")), None, None);
        let b = Assumption::new(unit("B"), expression(&predicate("q")), None, None);
        let one = AssumptionSet::from_assumptions(vec![a.clone(), b.clone()]).expect("builds");
        let other = AssumptionSet::from_assumptions(vec![b, a]).expect("builds");
        assert_eq!(one.identity(), other.identity());
        assert_eq!(one.to_artifact_bytes(), other.to_artifact_bytes());
    }

    #[test]
    fn check_fragments_enforces_w3() {
        // `expression()` declares `Fragment::Finite` explicitly, so it is
        // unambiguous under any declared set that contains `Finite`.
        let with_declared_fragment = AssumptionSet::from_assumptions(vec![Assumption::new(
            unit("A"),
            expression(&predicate("p")),
            None,
            None,
        )])
        .expect("builds");
        assert_eq!(
            with_declared_fragment.check_fragments(&BTreeSet::new()),
            Err(WellFormednessError::NoDeclaredFragments)
        );
        let both = BTreeSet::from([Fragment::Finite, Fragment::Symbolic]);
        assert_eq!(with_declared_fragment.check_fragments(&both), Ok(()));
        let just_finite = BTreeSet::from([Fragment::Finite]);
        assert_eq!(with_declared_fragment.check_fragments(&just_finite), Ok(()));

        // An expression that omits its fragment is ambiguous once more than one
        // is declared, and denotes the sole declared fragment otherwise.
        let fragmentless_expr =
            PropertyExpression::normalized(&predicate("p"), None, None).expect("normalizes");
        let fragmentless = AssumptionSet::from_assumptions(vec![Assumption::new(
            unit("A"),
            fragmentless_expr,
            None,
            None,
        )])
        .expect("builds");
        assert_eq!(
            fragmentless.check_fragments(&both),
            Err(WellFormednessError::AmbiguousFragment {
                unit: "A".to_owned(),
                declared: 2
            })
        );
        assert_eq!(fragmentless.check_fragments(&just_finite), Ok(()));

        let temporal_expr =
            PropertyExpression::normalized(&predicate("p"), Some(Fragment::Temporal), None)
                .expect("normalizes");
        let undeclared = AssumptionSet::from_assumptions(vec![Assumption::new(
            unit("A"),
            temporal_expr,
            None,
            None,
        )])
        .expect("builds");
        assert_eq!(
            undeclared.check_fragments(&just_finite),
            Err(WellFormednessError::UndeclaredFragment {
                unit: "A".to_owned(),
                fragment: Fragment::Temporal,
            })
        );
    }
}
