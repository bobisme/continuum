//! The `optimization` field group: hard constraints, soft objectives, and the
//! non-vacuity obligations (PR-4 / IMPL-08).
//!
//! # One contract group, two policy keys
//!
//! This is the only field group RFC 0037 governs with *two* verbs, and the split is
//! the point:
//!
//! > `optimization` is one contract group governed by **two** policy keys:
//! > `optimization.hard` and `optimization.soft` under `optimization`, and
//! > `optimization.non_vacuity` under `non_vacuity`. The split is required, not
//! > cosmetic — `non_vacuity = "no-removal"` (INV-012) is unenforceable if a removed
//! > non-vacuity behavior is reported as an `optimization` change (RFC 0031).
//! >
//! > — RFC 0037, "Fields"
//!
//! So the two sets are separate types' worth of separate, and a classified unit
//! cannot be produced without the policy key that governs it: [`Optimization::units`]
//! yields [`OptimizationUnit`] values, and every one of them answers
//! [`OptimizationUnit::policy_field`] with either
//! [`PolicyField::Optimization`](crate::change_policy::PolicyField::Optimization) or
//! [`PolicyField::NonVacuity`](crate::change_policy::PolicyField::NonVacuity). There
//! is no accessor that hands back the three sets flattened into one, because that is
//! precisely the accessor a "removed non-vacuity behavior reported as an
//! `optimization` change" would be written with.
//!
//! # Why non-vacuity is in the intent at all
//!
//! > ### INV-012 — Non-vacuous synthesis
//! >
//! > Forge objectives include required progress/availability behaviors and mutation
//! > challenges. Safety by disabling the system is rejected.
//! >
//! > — `notes/plan/plan.md`
//!
//! A property that cannot fire is not evidence. A system that admits no behavior
//! satisfies every safety property, so a contract carrying only safety claims can be
//! "satisfied" by a system that does nothing — the vacuity attack that plan §19.5
//! lists among the gaming mutations ("substitute a trivially true property") and that
//! G0-DX-07 exists to test Forge against. The non-vacuity set is the contract's
//! statement of what must remain *possible*, and `non_vacuity = "no-removal"` is what
//! stops a repair from deleting the obligation instead of meeting it.
//!
//! [`Optimization::require_non_vacuity`] is that obligation as a checkable predicate.
//! It is a *caller-invoked* check rather than a decode-time rejection, and the reason
//! is INV-003: `intent-contract.schema.json` makes `optimization` an object with no
//! required keys, so a contract with no non-vacuity behaviors is schema-valid, and a
//! decoder that rejected it would reject artifacts the shape authority admits. The
//! callers with standing to invoke it are the ones INV-012 names — a synthesis task
//! assembled against this contract — and they are PR 22's, not this bone's.
//!
//! *Raised as a flag against RFC 0037: a contract whose `policy.non_vacuity` is
//! `no-removal` while `optimization.non_vacuity` is empty carries a lock over nothing,
//! which is the same unenforceable-block shape W7 rejects for a `review` verb naming
//! no reviewer. W7 has no non-vacuity sibling, so the case is reported here rather
//! than rejected.*
//!
//! # Strings, sets, and what the diff does with them
//!
//! All three sets are string sets today:
//!
//! > `optimization.hard` and `optimization.soft` are classified under field
//! > `optimization`; `optimization.non_vacuity` is classified under field
//! > `non_vacuity`. […] All three are string sets today and are classified by
//! > membership.
//! >
//! > — RFC 0031, "`optimization` and `non_vacuity`"
//!
//! Membership is decidable by canonical comparison, so [`Optimization::changes`]
//! computes it here — that is the *field's order*, not a classifier decision requiring
//! an obligation. Whether these strings grow typed forms is an open question in both
//! RFCs; when they do, this module's unit types are where the typing lands, and the
//! two policy keys are already separated so the change is local.
//!
//! # Encoding
//!
//! The three sets are always written, empty included:
//!
//! > An absent optional set (`optimization.hard`, `security_policy.redaction_classes`,
//! > `fault_model.profiles`, an observer projection) MUST be read as the empty set for
//! > classification, and MUST be encoded explicitly as the empty set when the identity
//! > preimage is built (ID5), so that "absent" and "empty" cannot yield two identities
//! > for one meaning.
//! >
//! > — RFC 0037, "Absence, null, and defaults"
//!
//! The RFC names `optimization.hard` in that list by way of example; the same holds
//! for `soft` and `non_vacuity`, which are the same shape with the same `uniqueItems`
//! declaration. Members are written in code-point order, because a set has no authored
//! order and ID7 requires that "reformatting […] MUST NOT change" the identity — the
//! same reasoning [`crate::property::ClaimSet`] applies to its keyed array, and the
//! same flag: *ID5 states an ordering rule for object keys and none for arrays.*

use core::fmt;
use std::collections::{BTreeMap, BTreeSet};

use continuum_value::identity::{ContentHasher, Digest256};

use crate::canonical_json::{Json, JsonError};
use crate::change_policy::{PolicyField, Relation};

/// Which of the two `optimization` sets an objective belongs to.
///
/// Both are governed by the `optimization` policy key; the distinction is what the
/// contract asks of a candidate, not who governs it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ObjectiveRole {
    /// `optimization.hard` — a constraint a candidate must satisfy.
    Hard,
    /// `optimization.soft` — an objective a candidate is scored against.
    Soft,
}

impl ObjectiveRole {
    /// Both roles, in wire code-point order.
    pub const ALL: [Self; 2] = [Self::Hard, Self::Soft];

    /// The wire key inside the `optimization` object.
    #[must_use]
    pub const fn wire(self) -> &'static str {
        match self {
            Self::Hard => "hard",
            Self::Soft => "soft",
        }
    }
}

impl fmt::Display for ObjectiveRole {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.wire())
    }
}

/// One `optimization.hard` or `optimization.soft` string.
///
/// A newtype rather than a bare `String` so a hard constraint cannot be passed where a
/// non-vacuity obligation is expected. RFC 0031 makes "one constraint or objective"
/// the classified unit, and `semantic-diff.schema.json` gives a unit locator
/// `minLength: 1`, so the empty string is rejected for the same reason
/// [`crate::property::UnitKey`] rejects it: a unit that names nothing cannot appear in
/// a diff record.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Objective(String);

impl Objective {
    /// Validate and wrap an objective string.
    ///
    /// # Errors
    ///
    /// [`OptimizationError::EmptyObjective`] for the empty string.
    pub fn new(text: &str) -> Result<Self, OptimizationError> {
        if text.is_empty() {
            return Err(OptimizationError::EmptyObjective);
        }
        Ok(Self(text.to_owned()))
    }

    /// The objective's text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Objective {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// One `optimization.non_vacuity` behavior.
///
/// A distinct type from [`Objective`], not an alias. The two are governed by different
/// policy verbs, and RFC 0037 calls the split "required, not cosmetic"; a shared type
/// would make the two sets interchangeable at every call site that handles them, which
/// is exactly where the misreporting INV-012 forbids would happen.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NonVacuityObligation(String);

impl NonVacuityObligation {
    /// Validate and wrap a non-vacuity behavior.
    ///
    /// # Errors
    ///
    /// [`OptimizationError::EmptyObjective`] for the empty string.
    pub fn new(text: &str) -> Result<Self, OptimizationError> {
        if text.is_empty() {
            return Err(OptimizationError::EmptyObjective);
        }
        Ok(Self(text.to_owned()))
    }

    /// The behavior's text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for NonVacuityObligation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// One classified unit of the `optimization` group, carrying the policy key that
/// governs it.
///
/// The type exists so that "which verb governs this unit?" is answered by the unit
/// rather than by the caller's memory. See [`OptimizationUnit::policy_field`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OptimizationUnit<'a> {
    /// A hard constraint or soft objective, governed by `optimization`.
    Objective {
        /// Which of the two sets it came from.
        role: ObjectiveRole,
        /// The objective.
        objective: &'a Objective,
    },
    /// A non-vacuity behavior, governed by `non_vacuity`.
    NonVacuity(&'a NonVacuityObligation),
}

impl<'a> OptimizationUnit<'a> {
    /// The policy key — and therefore the RFC 0031 diff `field` — governing this unit.
    ///
    /// This is the whole of RFC 0037 correction 5, made unavoidable: "one contract
    /// group, two policy keys, two verbs; a non-vacuity removal MUST NOT be reported or
    /// governed as an `optimization` change (INV-012)."
    #[must_use]
    pub const fn policy_field(&self) -> PolicyField {
        match self {
            Self::Objective { .. } => PolicyField::Optimization,
            Self::NonVacuity(_) => PolicyField::NonVacuity,
        }
    }

    /// The unit's string, whichever set it came from.
    ///
    /// Borrows from the group rather than from the unit — the unit is [`Copy`] and
    /// holds a reference — so a caller may pair the string with its
    /// [`policy_field`](Self::policy_field) in one expression, which is how the two are
    /// meant to travel.
    #[must_use]
    pub fn as_str(self) -> &'a str {
        match self {
            Self::Objective { objective, .. } => objective.as_str(),
            Self::NonVacuity(behavior) => behavior.as_str(),
        }
    }
}

impl fmt::Display for OptimizationUnit<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.policy_field(), self.as_str())
    }
}

/// The canonical identity of an `optimization` field group.
///
/// As [`crate::property::PropertyIdentity`], this type *is* the canonical preimage
/// bytes rather than a digest of them (ADR-0013).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct OptimizationIdentity {
    canonical: Vec<u8>,
}

impl OptimizationIdentity {
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

impl fmt::Display for OptimizationIdentity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match core::str::from_utf8(&self.canonical) {
            Ok(text) => f.write_str(text),
            Err(_) => Err(fmt::Error),
        }
    }
}

/// The `optimization` object: `{hard, soft, non_vacuity}`, three string sets.
///
/// Immutable. [`PartialEq`] compares the three sets and never the derived identity.
#[derive(Debug, Clone)]
pub struct Optimization {
    hard: BTreeSet<Objective>,
    soft: BTreeSet<Objective>,
    non_vacuity: BTreeSet<NonVacuityObligation>,
    identity: OptimizationIdentity,
}

impl Optimization {
    /// Build the group from its three sets.
    ///
    /// # Errors
    ///
    /// [`OptimizationError::DuplicateObjective`] or
    /// [`OptimizationError::DuplicateNonVacuity`] when one set carries a member twice.
    /// The schema sets `uniqueItems: true` on all three, so a repeat is a document the
    /// schema rejects, and collapsing it silently would accept what the schema does
    /// not. A string may appear in two *different* sets: `hard` and `soft` are
    /// different questions about it, and the schema constrains each array on its own.
    pub fn new(
        hard: impl IntoIterator<Item = Objective>,
        soft: impl IntoIterator<Item = Objective>,
        non_vacuity: impl IntoIterator<Item = NonVacuityObligation>,
    ) -> Result<Self, OptimizationError> {
        let hard = objective_set(hard, ObjectiveRole::Hard)?;
        let soft = objective_set(soft, ObjectiveRole::Soft)?;
        let mut behaviors: BTreeSet<NonVacuityObligation> = BTreeSet::new();
        for behavior in non_vacuity {
            if !behaviors.insert(behavior.clone()) {
                return Err(OptimizationError::DuplicateNonVacuity {
                    behavior: behavior.as_str().to_owned(),
                });
            }
        }
        let identity = OptimizationIdentity::of_bytes(
            optimization_json(&hard, &soft, &behaviors).to_canonical_bytes(),
        );
        Ok(Self {
            hard,
            soft,
            non_vacuity: behaviors,
            identity,
        })
    }

    /// The empty group — three empty sets.
    ///
    /// A legal contract value: the schema's `optimization` object has no required
    /// keys, and RFC 0037's field table marks the group "object (MAY be empty)". It is
    /// a *declaration that the sets are empty*, not an absence of one; see
    /// [`require_non_vacuity`](Self::require_non_vacuity) for the obligation that
    /// makes the empty non-vacuity set a problem where INV-012 applies.
    #[must_use]
    pub fn empty() -> Self {
        Self::new([], [], []).expect("three empty sets carry no duplicate")
    }

    /// The `hard` constraints, in code-point order.
    pub fn hard(&self) -> impl Iterator<Item = &Objective> {
        self.hard.iter()
    }

    /// The `soft` objectives, in code-point order.
    pub fn soft(&self) -> impl Iterator<Item = &Objective> {
        self.soft.iter()
    }

    /// The `non_vacuity` behaviors, in code-point order.
    pub fn non_vacuity(&self) -> impl Iterator<Item = &NonVacuityObligation> {
        self.non_vacuity.iter()
    }

    /// The objectives of one role.
    pub fn objectives(&self, role: ObjectiveRole) -> impl Iterator<Item = &Objective> {
        match role {
            ObjectiveRole::Hard => self.hard.iter(),
            ObjectiveRole::Soft => self.soft.iter(),
        }
    }

    /// Every classified unit, each carrying the policy key that governs it.
    ///
    /// Order: the two `optimization` sets first (hard, then soft), then the
    /// `non_vacuity` set, each in code-point order.
    pub fn units(&self) -> impl Iterator<Item = OptimizationUnit<'_>> {
        let hard = self
            .hard
            .iter()
            .map(|objective| OptimizationUnit::Objective {
                role: ObjectiveRole::Hard,
                objective,
            });
        let soft = self
            .soft
            .iter()
            .map(|objective| OptimizationUnit::Objective {
                role: ObjectiveRole::Soft,
                objective,
            });
        hard.chain(soft)
            .chain(self.non_vacuity.iter().map(OptimizationUnit::NonVacuity))
    }

    /// Whether all three sets are empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.hard.is_empty() && self.soft.is_empty() && self.non_vacuity.is_empty()
    }

    /// How many non-vacuity behaviors the contract declares.
    #[must_use]
    pub fn non_vacuity_len(&self) -> usize {
        self.non_vacuity.len()
    }

    /// INV-012's obligation: the non-vacuity set must not be empty.
    ///
    /// > Forge objectives include required progress/availability behaviors and
    /// > mutation challenges. Safety by disabling the system is rejected.
    /// >
    /// > — plan §, INV-012
    ///
    /// > Every synthesis task includes positive behaviors or progress scenarios […]
    /// >
    /// > — plan §14.5, "Non-vacuity and anti-gaming"
    ///
    /// Invoked by the caller that knows the contract governs a synthesis task, not by
    /// the decoder: the schema admits an empty set, so rejecting one at decode time
    /// would reject artifacts the shape authority accepts (INV-003). See the module
    /// documentation.
    ///
    /// # Errors
    ///
    /// [`OptimizationError::NoNonVacuityObligation`] when the set is empty.
    pub fn require_non_vacuity(&self) -> Result<(), OptimizationError> {
        if self.non_vacuity.is_empty() {
            return Err(OptimizationError::NoNonVacuityObligation);
        }
        Ok(())
    }

    /// This group's canonical identity.
    #[must_use]
    pub const fn identity(&self) -> &OptimizationIdentity {
        &self.identity
    }

    /// The artifact-form JSON: the contract's `optimization` object.
    #[must_use]
    pub fn artifact_json(&self) -> Json {
        optimization_json(&self.hard, &self.soft, &self.non_vacuity)
    }

    /// The identity-preimage JSON.
    ///
    /// Identical to [`artifact_json`](Self::artifact_json): ID2's closed exclusion list
    /// has no member inside `optimization`, and all three sets are written explicitly
    /// so absence and emptiness cannot fork the identity.
    #[must_use]
    pub fn identity_preimage_json(&self) -> Json {
        self.artifact_json()
    }

    /// The artifact-form bytes, under the ID5 canonical-JSON rules.
    #[must_use]
    pub fn to_artifact_bytes(&self) -> Vec<u8> {
        self.artifact_json().to_canonical_bytes()
    }

    /// Decode the group from its artifact form.
    ///
    /// # Errors
    ///
    /// [`OptimizationDecodeError`], naming the field that failed.
    pub fn decode(bytes: &[u8]) -> Result<Self, OptimizationDecodeError> {
        Self::from_json(&Json::parse(bytes)?)
    }

    /// Decode the group from an already-parsed JSON object.
    ///
    /// # Errors
    ///
    /// [`OptimizationDecodeError`], as [`Optimization::decode`].
    pub fn from_json(json: &Json) -> Result<Self, OptimizationDecodeError> {
        let fields = json
            .as_object()
            .ok_or_else(|| OptimizationDecodeError::TypeMismatch {
                field: "optimization",
                expected: "object",
                found: json.type_name(),
            })?;
        for key in fields.keys() {
            if !matches!(key.as_str(), "hard" | "non_vacuity" | "soft") {
                return Err(OptimizationDecodeError::UnknownField {
                    field: "optimization",
                    key: key.clone(),
                });
            }
        }
        let hard = strings(fields.get("hard"), "optimization.hard")?;
        let soft = strings(fields.get("soft"), "optimization.soft")?;
        let behaviors = strings(fields.get("non_vacuity"), "optimization.non_vacuity")?;
        let hard = hard
            .iter()
            .map(|text| Objective::new(text))
            .collect::<Result<Vec<_>, _>>()?;
        let soft = soft
            .iter()
            .map(|text| Objective::new(text))
            .collect::<Result<Vec<_>, _>>()?;
        let behaviors = behaviors
            .iter()
            .map(|text| NonVacuityObligation::new(text))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self::new(hard, soft, behaviors)?)
    }

    /// The membership changes from this group to `after`, one record per unit.
    ///
    /// Set membership is the field's whole order (RFC 0031), so the only relations
    /// returned are [`Relation::Added`] and [`Relation::Removed`], and each is paired
    /// with the [`PolicyField`] that governs it — `non_vacuity` for a behavior,
    /// `optimization` for a constraint or objective — so a removal cannot be reported
    /// under the wrong verb.
    ///
    /// Order: `optimization` units before `non_vacuity` units, removals before
    /// additions within a set, each in code-point order.
    #[must_use]
    pub fn changes(&self, after: &Self) -> Vec<(PolicyField, String, Relation)> {
        let mut changes = Vec::new();
        for role in ObjectiveRole::ALL {
            membership(
                self.objectives(role).map(Objective::as_str),
                after.objectives(role).map(Objective::as_str),
                PolicyField::Optimization,
                &mut changes,
            );
        }
        membership(
            self.non_vacuity.iter().map(NonVacuityObligation::as_str),
            after.non_vacuity.iter().map(NonVacuityObligation::as_str),
            PolicyField::NonVacuity,
            &mut changes,
        );
        changes
    }
}

impl PartialEq for Optimization {
    fn eq(&self, other: &Self) -> bool {
        self.hard == other.hard && self.soft == other.soft && self.non_vacuity == other.non_vacuity
    }
}

impl Eq for Optimization {}

fn objective_set(
    objectives: impl IntoIterator<Item = Objective>,
    role: ObjectiveRole,
) -> Result<BTreeSet<Objective>, OptimizationError> {
    let mut set = BTreeSet::new();
    for objective in objectives {
        if !set.insert(objective.clone()) {
            return Err(OptimizationError::DuplicateObjective {
                role,
                objective: objective.as_str().to_owned(),
            });
        }
    }
    Ok(set)
}

fn membership<'a>(
    before: impl Iterator<Item = &'a str>,
    after: impl Iterator<Item = &'a str>,
    field: PolicyField,
    out: &mut Vec<(PolicyField, String, Relation)>,
) {
    let before: BTreeSet<&str> = before.collect();
    let after: BTreeSet<&str> = after.collect();
    for member in before.difference(&after) {
        out.push((field, (*member).to_owned(), Relation::Removed));
    }
    for member in after.difference(&before) {
        out.push((field, (*member).to_owned(), Relation::Added));
    }
}

fn strings(
    json: Option<&Json>,
    field: &'static str,
) -> Result<Vec<String>, OptimizationDecodeError> {
    match json {
        // An absent optional set reads as the empty set (RFC 0037); the encoder always
        // writes it, so absent and empty reach one value with one identity.
        None => Ok(Vec::new()),
        Some(Json::Array(items)) => items
            .iter()
            .map(|item| {
                item.as_str().map(str::to_owned).ok_or_else(|| {
                    OptimizationDecodeError::TypeMismatch {
                        field,
                        expected: "array of strings",
                        found: item.type_name(),
                    }
                })
            })
            .collect(),
        Some(other) => Err(OptimizationDecodeError::TypeMismatch {
            field,
            expected: "array",
            found: other.type_name(),
        }),
    }
}

fn optimization_json(
    hard: &BTreeSet<Objective>,
    soft: &BTreeSet<Objective>,
    non_vacuity: &BTreeSet<NonVacuityObligation>,
) -> Json {
    let mut fields = BTreeMap::new();
    fields.insert(
        "hard".to_owned(),
        Json::Array(
            hard.iter()
                .map(|objective| Json::String(objective.as_str().to_owned()))
                .collect(),
        ),
    );
    fields.insert(
        "non_vacuity".to_owned(),
        Json::Array(
            non_vacuity
                .iter()
                .map(|behavior| Json::String(behavior.as_str().to_owned()))
                .collect(),
        ),
    );
    fields.insert(
        "soft".to_owned(),
        Json::Array(
            soft.iter()
                .map(|objective| Json::String(objective.as_str().to_owned()))
                .collect(),
        ),
    );
    Json::Object(fields)
}

// --- errors --------------------------------------------------------------------------------

/// Why an `optimization` group is not well formed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OptimizationError {
    /// A member of one of the three sets is the empty string.
    EmptyObjective,
    /// `hard` or `soft` carries one objective twice (`uniqueItems: true`).
    DuplicateObjective {
        /// Which set.
        role: ObjectiveRole,
        /// The repeated string.
        objective: String,
    },
    /// `non_vacuity` carries one behavior twice (`uniqueItems: true`).
    DuplicateNonVacuity {
        /// The repeated string.
        behavior: String,
    },
    /// INV-012: the non-vacuity set is empty where an obligation is required.
    NoNonVacuityObligation,
}

impl fmt::Display for OptimizationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyObjective => f.write_str(
                "an optimization or non-vacuity string is the classified unit's key (RFC 0031) and \
                 cannot be empty",
            ),
            Self::DuplicateObjective { role, objective } => write!(
                f,
                "optimization.{role} carries {objective:?} twice; the set declares uniqueItems, \
                 and collapsing a repeat would accept a document the schema rejects"
            ),
            Self::DuplicateNonVacuity { behavior } => write!(
                f,
                "optimization.non_vacuity carries {behavior:?} twice; the set declares uniqueItems"
            ),
            Self::NoNonVacuityObligation => f.write_str(
                "the contract declares no non-vacuity behavior; a property that cannot fire is not \
                 evidence, and a system that admits no behavior satisfies every safety claim \
                 (INV-012, plan §14.5)",
            ),
        }
    }
}

impl core::error::Error for OptimizationError {}

/// Why a byte string is not an `optimization` artifact.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OptimizationDecodeError {
    /// The bytes are not admissible canonical JSON.
    Json(JsonError),
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
    ///
    /// The interesting case is a *fourth* set: a reader that ignored an unrecognized
    /// key would silently drop a category of obligation the contract meant to carry.
    UnknownField {
        /// The object's path.
        field: &'static str,
        /// The offending key.
        key: String,
    },
    /// The decoded sets are not well formed.
    Optimization(OptimizationError),
}

impl fmt::Display for OptimizationDecodeError {
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
                "`{field}` carries the unknown key {key:?}; the schema sets additionalProperties: \
                 false, and a reader that ignored it would drop an obligation the contract \
                 declared"
            ),
            Self::Optimization(error) => error.fmt(f),
        }
    }
}

impl core::error::Error for OptimizationDecodeError {}

impl From<JsonError> for OptimizationDecodeError {
    fn from(error: JsonError) -> Self {
        Self::Json(error)
    }
}

impl From<OptimizationError> for OptimizationDecodeError {
    fn from(error: OptimizationError) -> Self {
        Self::Optimization(error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn objective(text: &str) -> Objective {
        Objective::new(text).expect("a test objective is non-empty")
    }

    fn behavior(text: &str) -> NonVacuityObligation {
        NonVacuityObligation::new(text).expect("a test behavior is non-empty")
    }

    #[test]
    fn a_unit_carries_the_policy_key_that_governs_it() {
        let group = Optimization::new(
            [objective("progress")],
            [objective("stable_writes")],
            [behavior("synced request eventually acknowledged")],
        )
        .expect("distinct members");
        let fields: Vec<PolicyField> = group.units().map(|unit| unit.policy_field()).collect();
        assert_eq!(
            fields,
            vec![
                PolicyField::Optimization,
                PolicyField::Optimization,
                PolicyField::NonVacuity,
            ]
        );
    }

    #[test]
    fn a_removed_non_vacuity_behavior_is_never_reported_under_optimization() {
        let before = Optimization::new(
            [objective("progress")],
            [],
            [behavior(
                "a request is acknowledged in a failure-free fair run",
            )],
        )
        .expect("distinct members");
        let after = Optimization::new([objective("progress")], [], []).expect("distinct members");
        let changes = before.changes(&after);
        assert_eq!(
            changes,
            vec![(
                PolicyField::NonVacuity,
                "a request is acknowledged in a failure-free fair run".to_owned(),
                Relation::Removed,
            )]
        );
        // And the obligation itself is gone, which is what INV-012 asks about.
        assert!(before.require_non_vacuity().is_ok());
        assert_eq!(
            after.require_non_vacuity(),
            Err(OptimizationError::NoNonVacuityObligation)
        );
    }

    #[test]
    fn the_empty_group_is_a_declaration_and_not_an_absence() {
        let empty = Optimization::empty();
        assert!(empty.is_empty());
        assert_eq!(
            String::from_utf8(empty.to_artifact_bytes()).expect("utf-8"),
            r#"{"hard":[],"non_vacuity":[],"soft":[]}"#
        );
    }

    #[test]
    fn one_string_may_be_hard_and_soft_but_not_hard_twice() {
        assert!(
            Optimization::new([objective("progress")], [objective("progress")], []).is_ok(),
            "the two sets ask different questions about one string"
        );
        assert_eq!(
            Optimization::new([objective("progress"), objective("progress")], [], []),
            Err(OptimizationError::DuplicateObjective {
                role: ObjectiveRole::Hard,
                objective: "progress".to_owned()
            })
        );
    }
}
