//! The `assurance` field group: the level the contract demands, the checker
//! requirements, and the evidence classes it accepts (PR-4 / IMPL-07).
//!
//! # What this group says
//!
//! `assurance` is the contract's statement of *how well* its claims must be
//! established before evidence for them counts:
//!
//! > | `assurance` | `{minimum, independent_checker?, clean_recompute?, accepted_evidence_classes?}` | `minimum` | `minimum` ∈ `observed < sampled < bounded < validated < proved` (total order); the two checker flags are booleans; `accepted_evidence_classes` ⊆ the thirteen evidence kinds of `assurance-result.schema.json` (an **unordered** set) |
//! >
//! > — RFC 0037, "Field types and enums"
//!
//! It is a *contract-level* object. RFC 0037's field table and
//! `notes/plan/schemas/intent-contract.schema.json` place `assurance` beside
//! `claims`, not inside a claim, and the schema's `claims[]` items are
//! `{id, kind, expression, observer?}` with `additionalProperties: false` — there is
//! no per-claim assurance key to carry a different demand. So "which assurance level
//! each claim demands" has exactly one answer per contract, and
//! [`AssurancePolicy::demanded_of_every_claim`] is that answer, named so a caller
//! cannot mistake the scope. *Raised as a flag against RFC 0037: nothing in the
//! contract can demand `proved` of one claim and `sampled` of another; if per-claim
//! assurance is wanted it is a schema change, not a reading.*
//!
//! # The ladder is `continuum-value`'s, not this crate's
//!
//! [`AssuranceLevel`], [`AssuranceRequirement`], [`AssuranceChange`], and
//! [`EvidenceClass`] are imported from `crates/continuum-value/src/assurance.rs`.
//! Nothing here re-declares them, because a second declaration is a second
//! vocabulary:
//!
//! > The `assurance.minimum` enum and its order are shared with RFC 0031 and with
//! > `crates/continuum-value/src/assurance.rs`. Reordering or renaming any member is a
//! > coordinated breaking change across this RFC, RFC 0031, the contract schema, and
//! > that crate.
//! >
//! > — RFC 0037, "Versioning and revision"
//!
//! The landed enum and the schema agree today, member for member and token for token
//! — five levels `observed < sampled < bounded < validated < proved`, thirteen
//! evidence classes — and `tests/assurance_policy_contract.rs` asserts the agreement
//! against the schema text rather than assuming it.
//!
//! What `continuum-value` does *not* carry is a token → value direction:
//! `AssuranceLevel::as_str` exists and no `from_str` does. This module supplies
//! [`level_from_wire`] and [`evidence_class_from_wire`] by searching the landed `ALL`
//! arrays, so the reverse map is derived from the same declaration rather than
//! transcribed beside it. A reverse constructor belongs in `continuum-value` when
//! some other crate needs one; it is not this bone's file to edit.
//!
//! # Two vocabularies that share the token `sampled`
//!
//! > The five `assurance.minimum` levels and the thirteen `accepted_evidence_classes`
//! > are **different vocabularies that share the token `sampled`**. A level is what
//! > the intent *demands*; an evidence class is what an artifact *is*. A checker MUST
//! > NOT rank the evidence classes or derive one vocabulary from the other (RFC 0010:
//! > evidence is a partial order, not a ladder).
//! >
//! > — RFC 0037
//!
//! So this module offers two independent predicates —
//! [`AssurancePolicy::meets_minimum`] over the ladder and
//! [`AssurancePolicy::accepts`] over the set — and no third that composes them into a
//! single "is this evidence good enough" answer. Composing them is the caller's, and
//! the caller must state which question it is asking. `EvidenceClass` derives no
//! [`Ord`] in `continuum-value` for the same reason, so a ranking cannot be written
//! here even by accident.
//!
//! # Absence is not `false`
//!
//! `independent_checker` and `clean_recompute` are optional in the schema, and
//! RFC 0037's absence rules give a default to optional *sets* and to nothing else:
//!
//! > An absent optional set (`optimization.hard`, `security_policy.redaction_classes`,
//! > `fault_model.profiles`, an observer projection) MUST be read as the empty set for
//! > classification, and MUST be encoded explicitly as the empty set when the identity
//! > preimage is built (ID5) […]
//! >
//! > There are no other defaults. A checker MUST NOT invent a value for a key the
//! > schema leaves optional.
//!
//! Reading an absent `independent_checker` as `false` is inventing a value, and it is
//! the *dangerous* direction: it would let a contract that never declared a checker
//! requirement compare as equal to one that declared it off. So the two flags are
//! [`Option<bool>`] here, declaredness is preserved through the encoding, and
//! [`AssurancePolicy::requirement`] hands back `continuum-value`'s
//! [`AssuranceRequirement`] only when both are declared.
//!
//! [`AssurancePolicy::change_to`] therefore returns a `Result`: a change in the
//! *declaredness* of either flag has no relation in RFC 0031's three-member assurance
//! set (`unchanged`, `upgraded`, `downgraded`, and "`incomparable` MUST NOT be emitted
//! for `assurance`"), so it is reported as a typed refusal for the classifier to
//! record as `unknown` and fail closed on. Where declaredness agrees on both sides the
//! comparison is delegated to `AssuranceRequirement::change_to` unchanged, including
//! its mixed-movement rule. *Raised as a flag against RFC 0037 and RFC 0031: neither
//! states what an assurance checker flag's declaredness change classifies, though
//! RFC 0037 states the analogous rule for `assumptions[].classification` (a
//! declaredness change is `incomparable`) and for `bounds`' `nodes`/`faults` (a
//! declaredness change is `unknown` and fails closed).*
//!
//! # Seams left open on purpose
//!
//! - **The verdict is `change_policy`'s.** [`AssuranceChange::blocked_by_no_downgrade`]
//!   says what `assurance = "no-downgrade"` blocks; turning that into an `allow` /
//!   `review` / `block` is [`crate::change_policy::PolicyTable::verdict`].
//! - **Membership changes carry no direction.** [`AssurancePolicy::evidence_class_changes`]
//!   emits only [`Relation::Added`] and [`Relation::Removed`], because RFC 0031 says
//!   `accepted_evidence_classes` "MUST NOT contribute an `upgraded` or `downgraded`
//!   relation, and MUST NOT be ranked". The verdict reads them without a direction
//!   too: under `no-downgrade`, both relations contribute `review`, and under
//!   `proposal-only` they allow only on the human acceptance path (RFC 0031
//!   correction 20, in `PolicyTable::verdict`).
//! - **The contract identity is not here.** `in_*` is computed over the whole document
//!   (ID1); this module supplies the `assurance` portion of that preimage.

use core::fmt;
use std::collections::BTreeMap;

use crate::canonical_json::{Json, JsonError};
use crate::change_policy::Relation;
use crate::identity::canonical_identity;
/// Re-exported because they are already this module's public API surface —
/// [`AssurancePolicy::minimum`] returns an [`AssuranceLevel`],
/// [`AssurancePolicy::accepted_evidence_classes`] yields [`EvidenceClass`]es, and
/// [`AssurancePolicy::change_to`] returns an [`AssuranceChange`] — so a downstream
/// crate consuming those accessors (the semantic-diff assembler takes RFC 0031's
/// `requested_assurance` input, typed as the level the returned values already are)
/// can name the types through the same crate that handed them out, without opening
/// its own `continuum-value` edge for a vocabulary it only reads.
pub use continuum_value::assurance::{
    AssuranceChange, AssuranceLevel, AssuranceRequirement, EvidenceClass,
};

/// Recover an [`AssuranceLevel`] from its `assurance.minimum` wire literal.
///
/// Derived from `continuum_value::assurance::AssuranceLevel::ALL` and its `as_str`, so
/// the closed set is the landed declaration and not a copy of it. Returns `None` for
/// any other token — including one of the thirteen evidence classes, which is a
/// different vocabulary that happens to share the token `sampled`.
#[must_use]
pub fn level_from_wire(token: &str) -> Option<AssuranceLevel> {
    AssuranceLevel::ALL
        .into_iter()
        .find(|level| level.as_str() == token)
}

/// Recover an [`EvidenceClass`] from its wire literal.
///
/// Derived from `continuum_value::assurance::EvidenceClass::ALL`. Returns `None` for
/// any other token — including one of the five assurance levels.
#[must_use]
pub fn evidence_class_from_wire(token: &str) -> Option<EvidenceClass> {
    EvidenceClass::ALL
        .into_iter()
        .find(|class| class.as_str() == token)
}

canonical_identity! {
    /// The canonical identity of an `assurance` field group.
    ///
    /// As `crate::identity::CanonicalIdentity`, this type *is* the canonical preimage
    /// bytes rather than a digest of them (ADR-0013): equality is canonical comparison,
    /// and [`digest`](Self::digest) can only index.
    AssurancePolicyIdentity
}

/// The `assurance` object: a demanded level, two optional checker flags, and the
/// accepted evidence classes.
///
/// Immutable: no `&mut` method, no setter. A contract that demands more is a new
/// contract (RFC 0037, "Accepted contracts are immutable").
///
/// The accepted classes are held in a `BTreeMap` keyed by their wire tokens. That is
/// how a *set* of [`EvidenceClass`] gets one deterministic encoding without anyone
/// declaring an order over the classes themselves: the keys are strings, so the order
/// is ID5's code-point order over tokens, which is manifestly not a strength ranking.
/// `EvidenceClass` derives no [`Ord`] in `continuum-value` precisely so that a
/// ranking cannot be written, and nothing here writes one.
///
/// [`PartialEq`] is hand-written over the declared content and never consults the
/// derived identity.
#[derive(Debug, Clone)]
pub struct AssurancePolicy {
    minimum: AssuranceLevel,
    independent_checker: Option<bool>,
    clean_recompute: Option<bool>,
    accepted: BTreeMap<&'static str, EvidenceClass>,
    identity: AssurancePolicyIdentity,
}

impl AssurancePolicy {
    /// Declare an assurance policy.
    ///
    /// `independent_checker` and `clean_recompute` are `None` when the contract does
    /// not declare them, which is distinct from `Some(false)`; see the module
    /// documentation.
    ///
    /// # Errors
    ///
    /// [`AssurancePolicyError::DuplicateEvidenceClass`] when a class is offered twice.
    /// The schema sets `uniqueItems: true` on `accepted_evidence_classes`, so a repeat
    /// is a document the schema rejects, and silently collapsing it would accept what
    /// the schema does not.
    pub fn new(
        minimum: AssuranceLevel,
        independent_checker: Option<bool>,
        clean_recompute: Option<bool>,
        accepted_evidence_classes: impl IntoIterator<Item = EvidenceClass>,
    ) -> Result<Self, AssurancePolicyError> {
        let mut accepted: BTreeMap<&'static str, EvidenceClass> = BTreeMap::new();
        for class in accepted_evidence_classes {
            if accepted.insert(class.as_str(), class).is_some() {
                return Err(AssurancePolicyError::DuplicateEvidenceClass {
                    class: class.as_str(),
                });
            }
        }
        let identity = AssurancePolicyIdentity::of_bytes(
            assurance_json(minimum, independent_checker, clean_recompute, &accepted)
                .to_canonical_bytes(),
        );
        Ok(Self {
            minimum,
            independent_checker,
            clean_recompute,
            accepted,
            identity,
        })
    }

    /// The minimum level the contract demands.
    #[must_use]
    pub const fn minimum(&self) -> AssuranceLevel {
        self.minimum
    }

    /// The level demanded of every claim in the contract.
    ///
    /// The same value as [`minimum`](Self::minimum), under the name that answers the
    /// question "which assurance level does this claim demand?". The contract carries
    /// one demand and every claim inherits it; there is no per-claim assurance key in
    /// `intent-contract.schema.json`, so a claim cannot demand more or less than its
    /// contract does.
    #[must_use]
    pub const fn demanded_of_every_claim(&self) -> AssuranceLevel {
        self.minimum
    }

    /// Whether an independent checker is required, or `None` when undeclared.
    #[must_use]
    pub const fn independent_checker(&self) -> Option<bool> {
        self.independent_checker
    }

    /// Whether clean recomputation is required, or `None` when undeclared.
    #[must_use]
    pub const fn clean_recompute(&self) -> Option<bool> {
        self.clean_recompute
    }

    /// `continuum-value`'s requirement triple, when both checker flags are declared.
    ///
    /// `None` when either is undeclared: RFC 0031 classifies "the requirement triple
    /// (`minimum`, `independent_checker`, `clean_recompute`)", and a triple with an
    /// undeclared component is not that triple. Substituting `false` would invent a
    /// value the schema left optional (RFC 0037, "Absence, null, and defaults").
    #[must_use]
    pub fn requirement(&self) -> Option<AssuranceRequirement> {
        Some(AssuranceRequirement::new(
            self.minimum,
            self.independent_checker?,
            self.clean_recompute?,
        ))
    }

    /// Every accepted evidence class, in wire code-point order.
    ///
    /// The order is the encoding's, not a ranking: RFC 0010 makes evidence a partial
    /// order, and RFC 0037 forbids ranking these thirteen.
    pub fn accepted_evidence_classes(&self) -> impl Iterator<Item = EvidenceClass> {
        self.accepted.values().copied()
    }

    /// How many evidence classes the contract accepts.
    #[must_use]
    pub fn accepted_len(&self) -> usize {
        self.accepted.len()
    }

    /// Whether the contract accepts `class` toward its minimum.
    ///
    /// An absent `accepted_evidence_classes` reads as the empty set (RFC 0037), so
    /// this answers `false` for every class when the contract declared none. That is
    /// the reading the RFC states, and it is not a default invented here: the empty
    /// set is a declaration that no class is named, not a declaration that all are.
    #[must_use]
    pub fn accepts(&self, class: EvidenceClass) -> bool {
        self.accepted.contains_key(class.as_str())
    }

    /// Whether `level` reaches the demanded minimum, in the ladder's total order.
    ///
    /// Deliberately not composed with [`accepts`](Self::accepts): a level and an
    /// evidence class are different vocabularies, and "a checker MUST NOT […] derive
    /// one vocabulary from the other" (RFC 0037).
    #[must_use]
    pub fn meets_minimum(&self, level: AssuranceLevel) -> bool {
        level >= self.minimum
    }

    /// This group's canonical identity.
    #[must_use]
    pub const fn identity(&self) -> &AssurancePolicyIdentity {
        &self.identity
    }

    /// The artifact-form JSON: the contract's `assurance` object.
    ///
    /// `accepted_evidence_classes` is always written, empty included, because absence
    /// and emptiness are one meaning here and ID5 requires the preimage to spell it
    /// one way. The two checker flags are written only when declared, because absence
    /// is a *distinct* meaning for them.
    #[must_use]
    pub fn artifact_json(&self) -> Json {
        assurance_json(
            self.minimum,
            self.independent_checker,
            self.clean_recompute,
            &self.accepted,
        )
    }

    /// The identity-preimage JSON.
    ///
    /// Identical to [`artifact_json`](Self::artifact_json): ID2's closed exclusion list
    /// — `name`, `expression.source`, `expression.normal_form`, every `$comment`, and
    /// `intent_id` — has no member inside `assurance`. The method exists so a
    /// contract-level preimage builder can call one name across all fifteen groups.
    #[must_use]
    pub fn identity_preimage_json(&self) -> Json {
        self.artifact_json()
    }

    /// The artifact-form bytes, under the ID5 canonical-JSON rules.
    #[must_use]
    pub fn to_artifact_bytes(&self) -> Vec<u8> {
        self.artifact_json().to_canonical_bytes()
    }

    /// Decode an assurance policy from its artifact form.
    ///
    /// # Errors
    ///
    /// [`AssuranceDecodeError`], naming the field or the token that failed. An
    /// unrecognized `minimum` — a sixth level, an evidence-class token, a capitalized
    /// spelling — is rejected, never approximated to a neighbouring rung.
    pub fn decode(bytes: &[u8]) -> Result<Self, AssuranceDecodeError> {
        Self::from_json(&Json::parse(bytes)?)
    }

    /// Decode an assurance policy from an already-parsed JSON object.
    ///
    /// # Errors
    ///
    /// [`AssuranceDecodeError`], as [`AssurancePolicy::decode`].
    pub fn from_json(json: &Json) -> Result<Self, AssuranceDecodeError> {
        let fields = json
            .as_object()
            .ok_or_else(|| AssuranceDecodeError::TypeMismatch {
                field: "assurance",
                expected: "object",
                found: json.type_name(),
            })?;
        for key in fields.keys() {
            if !matches!(
                key.as_str(),
                "accepted_evidence_classes" | "clean_recompute" | "independent_checker" | "minimum"
            ) {
                return Err(AssuranceDecodeError::UnknownField {
                    field: "assurance",
                    key: key.clone(),
                });
            }
        }
        let minimum_json = fields
            .get("minimum")
            .ok_or(AssuranceDecodeError::MissingField {
                field: "assurance.minimum",
            })?;
        let token = minimum_json
            .as_str()
            .ok_or_else(|| AssuranceDecodeError::TypeMismatch {
                field: "assurance.minimum",
                expected: "string",
                found: minimum_json.type_name(),
            })?;
        let minimum = level_from_wire(token).ok_or_else(|| AssuranceDecodeError::UnknownToken {
            field: "assurance.minimum",
            token: token.to_owned(),
        })?;
        let independent_checker = optional_bool(fields, "independent_checker")?;
        let clean_recompute = optional_bool(fields, "clean_recompute")?;
        let mut accepted = Vec::new();
        // Absent reads as the empty set (RFC 0037), and the encoder always writes it,
        // so an absent key and an empty array reach one value with one identity.
        if let Some(value) = fields.get("accepted_evidence_classes") {
            let items = value
                .as_array()
                .ok_or_else(|| AssuranceDecodeError::TypeMismatch {
                    field: "assurance.accepted_evidence_classes",
                    expected: "array",
                    found: value.type_name(),
                })?;
            for item in items {
                let token = item
                    .as_str()
                    .ok_or_else(|| AssuranceDecodeError::TypeMismatch {
                        field: "assurance.accepted_evidence_classes[]",
                        expected: "string",
                        found: item.type_name(),
                    })?;
                accepted.push(evidence_class_from_wire(token).ok_or_else(|| {
                    AssuranceDecodeError::UnknownToken {
                        field: "assurance.accepted_evidence_classes[]",
                        token: token.to_owned(),
                    }
                })?);
            }
        }
        Ok(Self::new(
            minimum,
            independent_checker,
            clean_recompute,
            accepted,
        )?)
    }

    /// Classify a change from this policy to `after`, per RFC 0031's assurance rules.
    ///
    /// Delegates to `continuum_value::assurance::AssuranceRequirement::change_to`,
    /// which owns the rule and its mixed-movement fail-closed clause:
    ///
    /// > A change **weakens** iff `minimum` falls, or `independent_checker` goes
    /// > true→false, or `clean_recompute` goes true→false. […] **Fail closed in both
    /// > directions.** A change that both weakens and strengthens MUST classify
    /// > `downgraded`. A net direction MUST NOT be computed.
    /// >
    /// > — RFC 0031, "Assurance movement"
    ///
    /// A flag that is undeclared on *both* sides is passed through as one placeholder
    /// value for both, which cannot change the outcome: `change_to` reads each flag
    /// only through comparisons between the two sides, and equal inputs contribute
    /// neither a weakening nor a strengthening whatever they are.
    ///
    /// `accepted_evidence_classes` takes no part; see
    /// [`evidence_class_changes`](Self::evidence_class_changes).
    ///
    /// # Errors
    ///
    /// [`AssuranceComparisonError::CheckerDeclarednessChanged`] when a checker flag is
    /// declared on one side and not the other. That movement has no member in
    /// RFC 0031's three-relation assurance set, and `incomparable` "MUST NOT be emitted
    /// for `assurance`", so it is refused here for the classifier to record as
    /// `unknown` and fail closed on.
    pub fn change_to(&self, after: &Self) -> Result<AssuranceChange, AssuranceComparisonError> {
        let independent = comparable_flag(
            self.independent_checker,
            after.independent_checker,
            "independent_checker",
        )?;
        let clean = comparable_flag(
            self.clean_recompute,
            after.clean_recompute,
            "clean_recompute",
        )?;
        let before = AssuranceRequirement::new(self.minimum, independent.0, clean.0);
        let after = AssuranceRequirement::new(after.minimum, independent.1, clean.1);
        Ok(before.change_to(after))
    }

    /// The membership changes to `accepted_evidence_classes`, one record per class.
    ///
    /// > `accepted_evidence_classes` is an unordered set (RFC 0010: evidence is a
    /// > partial order, not a ladder), so it is classified by membership only: one
    /// > `added`/`removed` record per class. It MUST NOT contribute an `upgraded` or
    /// > `downgraded` relation, and MUST NOT be ranked.
    /// >
    /// > — RFC 0031, "`assurance`"
    ///
    /// So the only relations this can return are [`Relation::Added`] and
    /// [`Relation::Removed`], in wire code-point order over the tokens.
    #[must_use]
    pub fn evidence_class_changes(&self, after: &Self) -> Vec<(EvidenceClass, Relation)> {
        let mut changes = Vec::new();
        for (token, class) in &self.accepted {
            if !after.accepted.contains_key(token) {
                changes.push((*class, Relation::Removed));
            }
        }
        for (token, class) in &after.accepted {
            if !self.accepted.contains_key(token) {
                changes.push((*class, Relation::Added));
            }
        }
        changes.sort_by_key(|(class, _)| class.as_str());
        changes
    }
}

impl PartialEq for AssurancePolicy {
    fn eq(&self, other: &Self) -> bool {
        self.minimum == other.minimum
            && self.independent_checker == other.independent_checker
            && self.clean_recompute == other.clean_recompute
            && self.accepted == other.accepted
    }
}

impl Eq for AssurancePolicy {}

/// Pair two possibly-undeclared flags for comparison, or refuse.
fn comparable_flag(
    before: Option<bool>,
    after: Option<bool>,
    flag: &'static str,
) -> Result<(bool, bool), AssuranceComparisonError> {
    match (before, after) {
        (Some(before), Some(after)) => Ok((before, after)),
        // Undeclared on both sides: one placeholder for both, which contributes
        // neither direction whatever it is.
        (None, None) => Ok((false, false)),
        _ => Err(AssuranceComparisonError::CheckerDeclarednessChanged { flag }),
    }
}

fn optional_bool(
    fields: &BTreeMap<String, Json>,
    key: &'static str,
) -> Result<Option<bool>, AssuranceDecodeError> {
    match fields.get(key) {
        None => Ok(None),
        Some(Json::Bool(value)) => Ok(Some(*value)),
        Some(other) => Err(AssuranceDecodeError::TypeMismatch {
            field: match key {
                "independent_checker" => "assurance.independent_checker",
                _ => "assurance.clean_recompute",
            },
            expected: "boolean",
            found: other.type_name(),
        }),
    }
}

fn assurance_json(
    minimum: AssuranceLevel,
    independent_checker: Option<bool>,
    clean_recompute: Option<bool>,
    accepted: &BTreeMap<&'static str, EvidenceClass>,
) -> Json {
    let mut fields = BTreeMap::new();
    fields.insert(
        "accepted_evidence_classes".to_owned(),
        Json::Array(
            accepted
                .keys()
                .map(|token| Json::String((*token).to_owned()))
                .collect(),
        ),
    );
    if let Some(value) = clean_recompute {
        fields.insert("clean_recompute".to_owned(), Json::Bool(value));
    }
    if let Some(value) = independent_checker {
        fields.insert("independent_checker".to_owned(), Json::Bool(value));
    }
    fields.insert(
        "minimum".to_owned(),
        Json::String(minimum.as_str().to_owned()),
    );
    Json::Object(fields)
}

// --- errors --------------------------------------------------------------------------------

/// Why an assurance policy could not be built.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AssurancePolicyError {
    /// The same evidence class was offered twice (`uniqueItems: true`).
    DuplicateEvidenceClass {
        /// The repeated class's wire token.
        class: &'static str,
    },
}

impl fmt::Display for AssurancePolicyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateEvidenceClass { class } => write!(
                f,
                "the evidence class {class:?} is accepted twice; accepted_evidence_classes is a \
                 set (uniqueItems), and collapsing a repeat would accept a document the schema \
                 rejects"
            ),
        }
    }
}

impl core::error::Error for AssurancePolicyError {}

/// Why a byte string is not an `assurance` artifact.
///
/// Every variant is a rejection, never a repair: "an intent contract is never
/// best-effort decoded" (RFC 0037).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AssuranceDecodeError {
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
    /// An object carries a key `additionalProperties: false` forbids.
    UnknownField {
        /// The object's path.
        field: &'static str,
        /// The offending key.
        key: String,
    },
    /// A closed vocabulary carries a token this build does not implement.
    ///
    /// A sixth assurance level, a fourteenth evidence class, or a token borrowed from
    /// the *other* of the two vocabularies: all the same answer. "Forward
    /// compatibility is achieved by rejecting, never by ignoring."
    UnknownToken {
        /// The field's path.
        field: &'static str,
        /// The unrecognized token.
        token: String,
    },
    /// The decoded fields do not form a well-formed policy.
    Policy(AssurancePolicyError),
}

impl fmt::Display for AssuranceDecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Json(error) => error.fmt(f),
            Self::MissingField { field } => write!(
                f,
                "the required field `{field}` is absent; RFC 0037 correction 6 makes the demanded \
                 minimum required and totally ordered"
            ),
            Self::TypeMismatch {
                field,
                expected,
                found,
            } => write!(f, "`{field}` must be {expected}; found {found}"),
            Self::UnknownField { field, key } => write!(
                f,
                "`{field}` carries the unknown key {key:?}; the schema sets additionalProperties: \
                 false and a reader that ignored it would accept documents the schema rejects"
            ),
            Self::UnknownToken { field, token } => write!(
                f,
                "`{field}` carries the token {token:?}, which is not a member of that closed \
                 vocabulary; an unrecognized token is rejected, never ignored and never rounded to \
                 a neighbouring level (RFC 0037)"
            ),
            Self::Policy(error) => error.fmt(f),
        }
    }
}

impl core::error::Error for AssuranceDecodeError {}

impl From<JsonError> for AssuranceDecodeError {
    fn from(error: JsonError) -> Self {
        Self::Json(error)
    }
}

impl From<AssurancePolicyError> for AssuranceDecodeError {
    fn from(error: AssurancePolicyError) -> Self {
        Self::Policy(error)
    }
}

/// Why two assurance policies could not be compared.
///
/// The one case: a checker flag whose *declaredness* changed. RFC 0031 gives the
/// `assurance` field exactly three relations and forbids `incomparable` on it, so
/// there is no relation to report and the classifier must record `unknown` and fail
/// closed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AssuranceComparisonError {
    /// A checker flag is declared on one side and undeclared on the other.
    CheckerDeclarednessChanged {
        /// The flag's key: `independent_checker` or `clean_recompute`.
        flag: &'static str,
    },
}

impl fmt::Display for AssuranceComparisonError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CheckerDeclarednessChanged { flag } => write!(
                f,
                "`assurance.{flag}` is declared on one side and not the other; reading the absent \
                 side as false would invent a value the schema left optional, and no assurance \
                 relation names a declaredness change, so the classification is unknown and fails \
                 closed"
            ),
        }
    }
}

impl core::error::Error for AssuranceComparisonError {}

#[cfg(test)]
mod tests {
    use super::*;

    fn policy(minimum: AssuranceLevel) -> AssurancePolicy {
        AssurancePolicy::new(minimum, Some(true), Some(true), []).expect("no duplicates")
    }

    #[test]
    fn the_two_vocabularies_are_the_landed_ones_and_neither_reads_the_others_tokens() {
        for level in AssuranceLevel::ALL {
            assert_eq!(level_from_wire(level.as_str()), Some(level));
        }
        for class in EvidenceClass::ALL {
            assert_eq!(evidence_class_from_wire(class.as_str()), Some(class));
        }
        // The shared token resolves in both, to two different things.
        assert_eq!(level_from_wire("sampled"), Some(AssuranceLevel::Sampled));
        assert_eq!(
            evidence_class_from_wire("sampled"),
            Some(EvidenceClass::Sampled)
        );
        // And nothing else crosses over.
        assert_eq!(level_from_wire("finite-exact"), None);
        assert_eq!(evidence_class_from_wire("proved"), None);
    }

    #[test]
    fn an_undeclared_checker_flag_is_not_a_false_one() {
        let undeclared = AssurancePolicy::new(AssuranceLevel::Bounded, None, None, [])
            .expect("a policy with no checker flags");
        let declared_off =
            AssurancePolicy::new(AssuranceLevel::Bounded, Some(false), Some(false), [])
                .expect("a policy with both flags off");
        assert_ne!(undeclared, declared_off);
        assert_ne!(undeclared.identity(), declared_off.identity());
        assert!(undeclared.requirement().is_none());
        assert!(declared_off.requirement().is_some());
        assert_eq!(
            undeclared.change_to(&declared_off),
            Err(AssuranceComparisonError::CheckerDeclarednessChanged {
                flag: "independent_checker"
            })
        );
    }

    #[test]
    fn the_ladder_is_the_landed_total_order() {
        let demanding = policy(AssuranceLevel::Validated);
        assert!(demanding.meets_minimum(AssuranceLevel::Proved));
        assert!(demanding.meets_minimum(AssuranceLevel::Validated));
        assert!(!demanding.meets_minimum(AssuranceLevel::Bounded));
        assert_eq!(
            demanding.demanded_of_every_claim(),
            AssuranceLevel::Validated
        );
    }

    #[test]
    fn a_membership_change_carries_no_direction() {
        let before = AssurancePolicy::new(
            AssuranceLevel::Bounded,
            Some(true),
            Some(true),
            [EvidenceClass::FiniteExact, EvidenceClass::Inductive],
        )
        .expect("distinct classes");
        let after = AssurancePolicy::new(
            AssuranceLevel::Bounded,
            Some(true),
            Some(true),
            [EvidenceClass::FiniteExact, EvidenceClass::Certificate],
        )
        .expect("distinct classes");
        assert_eq!(
            before.evidence_class_changes(&after),
            vec![
                (EvidenceClass::Certificate, Relation::Added),
                (EvidenceClass::Inductive, Relation::Removed),
            ]
        );
        // The requirement triple is untouched by the membership change.
        assert_eq!(before.change_to(&after), Ok(AssuranceChange::Unchanged));
    }
}
