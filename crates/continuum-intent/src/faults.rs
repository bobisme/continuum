//! The `faults` field group: the fault model in force (PR-4 / IMPL-05).
//!
//! # What a fault model *is*, in this crate
//!
//! The contract path is `fault_model` and the policy and diff name is `faults`; the
//! schema's `x-policy-field-map` relates them, and RFC 0037 is explicit that "the
//! *policy* name is the diff name; the contract path is not". Both spellings appear
//! below, each where it belongs.
//!
//! > | `fault_model` | `{enabled, profiles?}` | `enabled` | `enabled` ⊆ {`crash`,
//! > `recovery`, `partition`, `loss`, `duplication`, `delay`}; `profiles` are
//! > domain-pack profile names (RFC 0002) |
//! >
//! > — `notes/plan/rfcs/0037-intent-contract.md`, "Field types and enums"
//!
//! > `enabled` is a closed set of six classes […] and `profiles` is a set of pack
//! > profile names. Both are classified by membership: one record per class or
//! > profile added or removed. `removed` is the "removing crash-after-submit" attack
//! > and is what `no-removal` blocks.
//! >
//! > — `notes/plan/rfcs/0031-semantic-and-intent-diff.md`, "`faults`"
//!
//! So a fault model is **two sets and nothing else**, and its classified unit is *one
//! member* of either set — not the model as a whole. [`FaultModel`] is those two
//! sets; [`FaultUnit`] is that member; and [`FaultModel::units`] is the surface RFC
//! 0031 compares by membership.
//!
//! # The protected weakening this shape makes visible
//!
//! "Remove faults" is on
//! `notes/plan/docs/50_AGENT_EVALUATION_AND_REWARD_HACKING.md`'s intent-attack list,
//! and RFC 0031's attack table names the concrete move: *remove crash-after-submit →
//! `faults` → `removed` → `no-removal`*. `faults` is one of only four fields whose
//! applicable-verb set includes `no-removal` (RFC 0037, "Verb applicability"), which
//! is a verb that can only be enforced if a removal is *individually* visible.
//!
//! That is why this module refuses to summarize. There is no "fault envelope" scalar
//! and no count: RFC 0031 is explicit that plan §5.3's "fault envelope
//! expansion/contraction" is "the informal alias for a set of `added`/`removed`
//! records". A representation that collapsed six classes into an envelope would make
//! `no-removal` unenforceable exactly the way RFC 0037 says reporting a non-vacuity
//! removal as an `optimization` change would: "The split is required, not cosmetic".
//!
//! Three consequences, each load-bearing:
//!
//! 1. **Membership is the comparison surface.** [`FaultModel::enabled`],
//!    [`FaultModel::profiles`], and [`FaultModel::units`] hand a classifier sets, so a
//!    dropped class is a set difference rather than a diff of two renderings.
//! 2. **Absence is not a third state.** `profiles` is optional in the document and is
//!    always encoded — RFC 0037 names `fault_model.profiles` among the sets that "MUST
//!    be encoded explicitly as the empty set when the identity preimage is built
//!    (ID5), so that 'absent' and 'empty' cannot yield two identities for one
//!    meaning". Deleting the key is therefore not a way to drop the profiles quietly.
//! 3. **Every removal moves the identity.** ID7: "Changing any claim, assumption,
//!    observer, bound, fault, fairness constraint […] MUST change it."
//!
//! # The unit locator, and one flag against the diff schema
//!
//! `semantic-diff.schema.json` fixes the per-unit locator spelling: "`faults` by the
//! member of `enabled` or `profiles`". [`FaultUnit::locator`] returns exactly that —
//! the bare token. *Raised as a flag against RFC 0031 and
//! `semantic-diff.schema.json`: the bare member does not say which set it came from,
//! so a domain-pack profile named `crash` and the fault class `crash` produce the
//! same locator. The two are different units with different policy consequences (a
//! profile is checked against the snapshot's packs by W8; a class is not), so the
//! locator should be qualified. This module keeps the schema's spelling — INV-003
//! says the schema decides shape — and exposes [`FaultUnit`] itself, which is not
//! ambiguous, beside it.*
//!
//! # One spelling, not two
//!
//! As in [`crate::observers`], no member of ID2's closed exclusion list has an
//! admissible site inside `fault_model`, so the artifact form *is* the identity
//! preimage and [`FaultModel::to_artifact_bytes`] and [`FaultModel::identity`] agree
//! byte for byte.
//!
//! Both arrays are encoded in Unicode code-point order of the wire token — ID5's order
//! for object keys, applied to a `uniqueItems` array whose members are the diff's unit
//! locators. [`FaultClass`]'s [`Ord`] is therefore token order and *not* the schema's
//! enum-declaration order; [`FaultClass::ALL`] keeps the declaration order, since that
//! is the order the RFC lists the vocabulary in. *Same flag as
//! [`crate::property::ClaimSet`] raises: ID5 states the order of object keys and says
//! nothing about arrays.*
//!
//! # Seams left open on purpose
//!
//! - **W8's other half.** "`fault_model.profiles` MUST name domain-pack profiles
//!   available in the snapshot's domain packs" needs the snapshot's packs, which this
//!   crate may not import. [`FaultModel::check_profiles`] takes the available set and
//!   is the half that lives here.
//! - **`bounds.faults` is a different thing.** The `bounds` tuple carries a `faults`
//!   integer — a *budget* on how many faults a search may inject — and it is the
//!   `bounds` group's field, classified by the componentwise partial order. It is not
//!   this group and is a separate bone.
//! - **Classification is not here.** Which member was `added` or `removed` is RFC
//!   0031's question (PR 12).

use core::cmp::Ordering;
use core::fmt;
use std::collections::BTreeSet;

use crate::canonical_json::{Json, JsonError};
use crate::identity::canonical_identity;

/// One member of `fault_model.enabled`: the closed six-member fault vocabulary.
///
/// RFC 0037 lists `fault_model.enabled` among the closed vocabularies whose
/// membership cannot change without "a `schema_epoch` advance *and* an explicit
/// revision of this RFC". An unrecognized token is therefore a rejection and never an
/// opaque seventh class: a reader that carried one through would be reporting a fault
/// model it does not understand as one it does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FaultClass {
    /// A process stops.
    Crash,
    /// A crashed process returns.
    Recovery,
    /// The network splits.
    Partition,
    /// A message is lost.
    Loss,
    /// A message is delivered more than once.
    Duplication,
    /// A message is delayed.
    Delay,
}

impl FaultClass {
    /// Every class, in the order RFC 0037 and the schema list the enum.
    ///
    /// This is *not* the canonical encoding order — see [`Ord`], which is code-point
    /// order on the wire token.
    pub const ALL: [Self; 6] = [
        Self::Crash,
        Self::Recovery,
        Self::Partition,
        Self::Loss,
        Self::Duplication,
        Self::Delay,
    ];

    /// The wire literal, which is also the RFC 0031 unit locator.
    #[must_use]
    pub const fn wire(self) -> &'static str {
        match self {
            Self::Crash => "crash",
            Self::Recovery => "recovery",
            Self::Partition => "partition",
            Self::Loss => "loss",
            Self::Duplication => "duplication",
            Self::Delay => "delay",
        }
    }

    /// Recover a class from its wire literal.
    ///
    /// Returns `None` for an unrecognized token. The caller rejects: "Forward
    /// compatibility is achieved by rejecting, never by ignoring" (RFC 0037).
    #[must_use]
    pub fn from_wire(token: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|class| class.wire() == token)
    }
}

impl Ord for FaultClass {
    /// Code-point order on the wire token, so `enabled` encodes in the same order
    /// [`ProfileName`] and every other keyed collection in this crate does.
    fn cmp(&self, other: &Self) -> Ordering {
        self.wire().cmp(other.wire())
    }
}

impl PartialOrd for FaultClass {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl fmt::Display for FaultClass {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.wire())
    }
}

/// One member of `fault_model.profiles`: a domain-pack profile name (RFC 0002).
///
/// Unlike a fault class this vocabulary is *open* — it is whatever the snapshot's
/// domain packs declare — so the type validates only what the diff schema requires of
/// a unit locator: `minLength: 1`. An empty name could not appear as a `unit` in an
/// `intent_changes` record, so a removal of it could not be reported.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ProfileName(String);

impl ProfileName {
    /// Validate and wrap a profile name.
    ///
    /// # Errors
    ///
    /// [`FaultError::EmptyProfileName`] for the empty string.
    pub fn new(name: &str) -> Result<Self, FaultError> {
        if name.is_empty() {
            return Err(FaultError::EmptyProfileName);
        }
        Ok(Self(name.to_owned()))
    }

    /// The name, which is also the RFC 0031 unit locator.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ProfileName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// One classified unit of the `faults` field: "one member of `enabled` or of
/// `profiles`" (RFC 0031's unit-of-classification table).
///
/// A fault unit has no structure to encode, so it has no identity type of its own: its
/// canonical identity *is* its token, and [`FaultUnit::locator`] is that token. What
/// the group needs an identity for is the whole model, which is
/// [`FaultModel::identity`].
///
/// [`Ord`] puts every class before every profile and orders within each by token, so
/// [`FaultModel::units`] enumerates in the order the two encoded arrays are written.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum FaultUnit {
    /// A member of `enabled`.
    Class(FaultClass),
    /// A member of `profiles`.
    Profile(ProfileName),
}

impl FaultUnit {
    /// The per-unit locator `semantic-diff.schema.json` specifies: "`faults` by the
    /// member of `enabled` or `profiles`".
    ///
    /// See the module documentation for the flag this raises: the bare member does not
    /// record which set it came from.
    #[must_use]
    pub fn locator(&self) -> &str {
        match self {
            Self::Class(class) => class.wire(),
            Self::Profile(name) => name.as_str(),
        }
    }

    /// Which of the two sets this unit belongs to, as a contract path.
    ///
    /// The disambiguation the locator lacks, for a caller that wants it.
    #[must_use]
    pub const fn field(&self) -> &'static str {
        match self {
            Self::Class(_) => "fault_model.enabled",
            Self::Profile(_) => "fault_model.profiles",
        }
    }
}

impl fmt::Display for FaultUnit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.locator())
    }
}

canonical_identity! {
    /// The canonical identity of a fault-model artifact.
    ///
    /// The discipline is `crate::identity::CanonicalIdentity`'s, stated in full
    /// there: the identity *is* the canonical preimage bytes, so equality is canonical
    /// comparison (ADR-0013) and [`FaultModelIdentity::digest`] can only index.
    FaultModelIdentity
}

/// The fault model in force: the contract's `fault_model` object.
///
/// Immutable, for the reason `lib.rs` states. A fault model with `crash` removed is a
/// *new* [`FaultModel`] with a new [`FaultModelIdentity`]; there is no method that
/// removes a class from one in hand, because that method is precisely the affordance
/// INV-001 forbids and INV-011 reclassifies as an intent revision.
///
/// [`PartialEq`] is hand-written and compares the two sets, never the identity.
#[derive(Debug, Clone)]
pub struct FaultModel {
    enabled: BTreeSet<FaultClass>,
    profiles: BTreeSet<ProfileName>,
    identity: FaultModelIdentity,
}

impl FaultModel {
    /// Build a fault model from its two sets.
    ///
    /// Both may be empty. An empty `enabled` is a declaration that no fault class is
    /// modeled, not an omission; `profiles` is optional in the document and an absent
    /// key reads as the empty set.
    ///
    /// # Errors
    ///
    /// [`FaultError::DuplicateClass`] or [`FaultError::DuplicateProfile`] — the
    /// schema sets `uniqueItems: true` on both arrays, so a repeat is rejected rather
    /// than deduplicated.
    pub fn new(
        enabled: impl IntoIterator<Item = FaultClass>,
        profiles: impl IntoIterator<Item = ProfileName>,
    ) -> Result<Self, FaultError> {
        let mut classes = BTreeSet::new();
        for class in enabled {
            if !classes.insert(class) {
                return Err(FaultError::DuplicateClass { class });
            }
        }
        let mut names = BTreeSet::new();
        for profile in profiles {
            if !names.insert(profile.clone()) {
                return Err(FaultError::DuplicateProfile {
                    name: profile.to_string(),
                });
            }
        }
        let identity =
            FaultModelIdentity::of_bytes(fault_model_json(&classes, &names).to_canonical_bytes());
        Ok(Self {
            enabled: classes,
            profiles: names,
            identity,
        })
    }

    /// The enabled fault classes.
    #[must_use]
    pub const fn enabled(&self) -> &BTreeSet<FaultClass> {
        &self.enabled
    }

    /// The declared domain-pack profiles.
    #[must_use]
    pub const fn profiles(&self) -> &BTreeSet<ProfileName> {
        &self.profiles
    }

    /// Every classified unit, classes first and each set in token order.
    ///
    /// This is the group's comparison surface: RFC 0031 classifies `faults` by "set
    /// membership […] one record per class or profile added or removed", so a
    /// classifier wants the members, not the model.
    pub fn units(&self) -> impl Iterator<Item = FaultUnit> + '_ {
        self.enabled
            .iter()
            .copied()
            .map(FaultUnit::Class)
            .chain(self.profiles.iter().cloned().map(FaultUnit::Profile))
    }

    /// Whether a unit is a member of this model.
    #[must_use]
    pub fn contains(&self, unit: &FaultUnit) -> bool {
        match unit {
            FaultUnit::Class(class) => self.enabled.contains(class),
            FaultUnit::Profile(name) => self.profiles.contains(name),
        }
    }

    /// The fault model's canonical identity, which is also its artifact bytes.
    #[must_use]
    pub const fn identity(&self) -> &FaultModelIdentity {
        &self.identity
    }

    /// W8's fault half, given the profile names the snapshot's domain packs provide.
    ///
    /// > **W8 — Profiles and maps resolve.** `fault_model.profiles` MUST name
    /// > domain-pack profiles available in the snapshot's domain packs […] An
    /// > unresolvable reference is rejected at acceptance time; it MUST NOT be
    /// > deferred into an `unknown` classification later.
    /// >
    /// > — RFC 0037
    ///
    /// # Errors
    ///
    /// [`FaultWellFormednessError::UnresolvedProfile`] for the first profile, in token
    /// order, that the packs do not provide.
    pub fn check_profiles(
        &self,
        available: &BTreeSet<String>,
    ) -> Result<(), FaultWellFormednessError> {
        for profile in &self.profiles {
            if !available.contains(profile.as_str()) {
                return Err(FaultWellFormednessError::UnresolvedProfile {
                    name: profile.to_string(),
                });
            }
        }
        Ok(())
    }

    /// The artifact-form JSON: what a contract document carries.
    #[must_use]
    pub fn artifact_json(&self) -> Json {
        fault_model_json(&self.enabled, &self.profiles)
    }

    /// The artifact-form bytes, under the ID5 canonical-JSON rules.
    #[must_use]
    pub fn to_artifact_bytes(&self) -> Vec<u8> {
        self.artifact_json().to_canonical_bytes()
    }

    /// Decode a fault model from its artifact form.
    ///
    /// # Errors
    ///
    /// [`FaultDecodeError`], naming the field or the token that failed.
    pub fn decode(bytes: &[u8]) -> Result<Self, FaultDecodeError> {
        Self::from_json(&Json::parse(bytes)?)
    }

    /// Decode a fault model from an already-parsed JSON object.
    ///
    /// # Errors
    ///
    /// [`FaultDecodeError`], as [`FaultModel::decode`].
    pub fn from_json(json: &Json) -> Result<Self, FaultDecodeError> {
        let fields = json
            .as_object()
            .ok_or_else(|| FaultDecodeError::TypeMismatch {
                field: "fault_model",
                expected: "object",
                found: json.type_name(),
            })?;
        for key in fields.keys() {
            if key != "enabled" && key != "profiles" {
                return Err(FaultDecodeError::UnknownField {
                    field: "fault_model",
                    key: key.clone(),
                });
            }
        }
        let enabled_json = fields
            .get("enabled")
            .ok_or(FaultDecodeError::MissingField {
                field: "fault_model.enabled",
            })?;
        let mut enabled = Vec::new();
        for item in array(enabled_json, "fault_model.enabled")? {
            let token = item
                .as_str()
                .ok_or_else(|| FaultDecodeError::TypeMismatch {
                    field: "fault_model.enabled",
                    expected: "string",
                    found: item.type_name(),
                })?;
            enabled.push(FaultClass::from_wire(token).ok_or_else(|| {
                FaultDecodeError::UnknownToken {
                    field: "fault_model.enabled",
                    token: token.to_owned(),
                }
            })?);
        }
        let mut profiles = Vec::new();
        // An absent optional set reads as the empty set, and the encoder writes it
        // back explicitly — RFC 0037 names `fault_model.profiles` in that rule.
        if let Some(profiles_json) = fields.get("profiles") {
            for item in array(profiles_json, "fault_model.profiles")? {
                let name = item
                    .as_str()
                    .ok_or_else(|| FaultDecodeError::TypeMismatch {
                        field: "fault_model.profiles",
                        expected: "string",
                        found: item.type_name(),
                    })?;
                profiles.push(ProfileName::new(name)?);
            }
        }
        Ok(Self::new(enabled, profiles)?)
    }
}

impl PartialEq for FaultModel {
    fn eq(&self, other: &Self) -> bool {
        self.enabled == other.enabled && self.profiles == other.profiles
    }
}

impl Eq for FaultModel {}

fn array<'a>(json: &'a Json, field: &'static str) -> Result<&'a [Json], FaultDecodeError> {
    json.as_array()
        .ok_or_else(|| FaultDecodeError::TypeMismatch {
            field,
            expected: "array",
            found: json.type_name(),
        })
}

fn fault_model_json(enabled: &BTreeSet<FaultClass>, profiles: &BTreeSet<ProfileName>) -> Json {
    Json::object([
        (
            "enabled".to_owned(),
            Json::Array(
                enabled
                    .iter()
                    .map(|class| Json::String(class.wire().to_owned()))
                    .collect(),
            ),
        ),
        (
            "profiles".to_owned(),
            Json::Array(
                profiles
                    .iter()
                    .map(|profile| Json::String(profile.as_str().to_owned()))
                    .collect(),
            ),
        ),
    ])
    .unwrap_or(Json::Null)
}

// --- errors --------------------------------------------------------------------------------

/// Why a fault model could not be built.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FaultError {
    /// A profile name is the empty string.
    EmptyProfileName,
    /// `enabled` repeats a class, which `uniqueItems: true` forbids.
    DuplicateClass {
        /// The repeated class.
        class: FaultClass,
    },
    /// `profiles` repeats a name, which `uniqueItems: true` forbids.
    DuplicateProfile {
        /// The repeated name.
        name: String,
    },
}

impl fmt::Display for FaultError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyProfileName => f.write_str(
                "a domain-pack profile name is a classified-unit locator (RFC 0031) and cannot be \
                 empty",
            ),
            Self::DuplicateClass { class } => write!(
                f,
                "fault_model.enabled repeats `{class}`; the schema sets uniqueItems: true, so a \
                 repeat is rejected rather than deduplicated"
            ),
            Self::DuplicateProfile { name } => write!(
                f,
                "fault_model.profiles repeats {name:?}; the schema sets uniqueItems: true, so a \
                 repeat is rejected rather than deduplicated"
            ),
        }
    }
}

impl core::error::Error for FaultError {}

/// Why a byte string is not a fault-model artifact.
///
/// Every variant is a rejection, never a repair. RFC 0037: "an intent contract is
/// never best-effort decoded".
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FaultDecodeError {
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
    /// `enabled` carries a token outside the closed six-member vocabulary.
    ///
    /// The fail-closed case. A seventh class is a `schema_epoch` advance, and a reader
    /// that carried it through would report a fault model it does not implement as one
    /// it does.
    UnknownToken {
        /// The field's path.
        field: &'static str,
        /// The unrecognized token.
        token: String,
    },
    /// The decoded sets do not form a well-formed fault model.
    Fault(FaultError),
}

impl fmt::Display for FaultDecodeError {
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
            Self::Fault(error) => error.fmt(f),
        }
    }
}

impl core::error::Error for FaultDecodeError {}

impl From<JsonError> for FaultDecodeError {
    fn from(error: JsonError) -> Self {
        Self::Json(error)
    }
}

impl From<FaultError> for FaultDecodeError {
    fn from(error: FaultError) -> Self {
        Self::Fault(error)
    }
}

/// Why a fault model is not well formed against the rest of a snapshot.
///
/// Separate from [`FaultDecodeError`] because the rule is *relational*: it holds
/// between `fault_model.profiles` and the snapshot's domain packs, so it cannot be
/// decided when the model is decoded and is not the decoder's to report.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FaultWellFormednessError {
    /// W8: a declared profile is not available in the snapshot's domain packs.
    UnresolvedProfile {
        /// The unresolved name.
        name: String,
    },
}

impl fmt::Display for FaultWellFormednessError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnresolvedProfile { name } => write!(
                f,
                "fault_model.profiles names {name:?}, which the snapshot's domain packs do not \
                 provide; W8 rejects it at acceptance time rather than deferring it into an \
                 `unknown` classification later"
            ),
        }
    }
}

impl core::error::Error for FaultWellFormednessError {}

#[cfg(test)]
mod tests {
    use super::*;

    fn profile(name: &str) -> ProfileName {
        ProfileName::new(name).expect("a test profile name is non-empty")
    }

    #[test]
    fn the_fault_vocabulary_is_closed_and_six_members_wide() {
        assert_eq!(FaultClass::ALL.len(), 6);
        for class in FaultClass::ALL {
            assert_eq!(FaultClass::from_wire(class.wire()), Some(class));
        }
        assert_eq!(FaultClass::from_wire("byzantine"), None);
        assert_eq!(FaultClass::from_wire("Crash"), None);
        assert_eq!(FaultClass::from_wire(""), None);
    }

    #[test]
    fn a_profile_name_cannot_be_empty() {
        assert_eq!(ProfileName::new(""), Err(FaultError::EmptyProfileName));
        assert!(ProfileName::new("storage-posix-v1").is_ok());
    }

    #[test]
    fn classes_encode_in_token_order_not_declaration_order() {
        let model = FaultModel::new(FaultClass::ALL, []).expect("well formed");
        let encoded = String::from_utf8(model.to_artifact_bytes()).expect("utf-8");
        assert_eq!(
            encoded,
            r#"{"enabled":["crash","delay","duplication","loss","partition","recovery"],"profiles":[]}"#
        );
    }

    #[test]
    fn an_absent_profiles_key_is_the_empty_set_and_is_encoded_as_one() {
        let model = FaultModel::new([FaultClass::Crash], []).expect("well formed");
        let encoded = String::from_utf8(model.to_artifact_bytes()).expect("utf-8");
        assert!(encoded.contains(r#""profiles":[]"#), "{encoded}");
        assert_eq!(
            model,
            FaultModel::decode(br#"{"enabled":["crash"]}"#).expect("decodes")
        );
    }

    #[test]
    fn the_identity_is_the_artifact_bytes_because_no_id2_member_lives_here() {
        let model = FaultModel::new([FaultClass::Crash], [profile("p")]).expect("well formed");
        assert_eq!(
            model.identity().canonical_bytes(),
            model.to_artifact_bytes()
        );
    }

    #[test]
    fn the_units_surface_is_membership_and_never_an_envelope() {
        let model = FaultModel::new(
            [FaultClass::Recovery, FaultClass::Crash],
            [profile("pack.a")],
        )
        .expect("well formed");
        let units: Vec<_> = model.units().collect();
        assert_eq!(
            units.iter().map(FaultUnit::locator).collect::<Vec<_>>(),
            vec!["crash", "recovery", "pack.a"]
        );
        assert_eq!(units[0].field(), "fault_model.enabled");
        assert_eq!(units[2].field(), "fault_model.profiles");
        assert!(model.contains(&FaultUnit::Class(FaultClass::Crash)));
        assert!(!model.contains(&FaultUnit::Class(FaultClass::Delay)));
    }

    #[test]
    fn a_repeat_is_rejected_rather_than_deduplicated() {
        assert_eq!(
            FaultModel::new([FaultClass::Crash, FaultClass::Crash], []),
            Err(FaultError::DuplicateClass {
                class: FaultClass::Crash
            })
        );
        assert_eq!(
            FaultModel::new([], [profile("p"), profile("p")]),
            Err(FaultError::DuplicateProfile {
                name: "p".to_owned()
            })
        );
    }

    #[test]
    fn w8s_fault_half_takes_the_packs_available_set() {
        let model = FaultModel::new([], [profile("storage-posix-v1")]).expect("well formed");
        let mut available = BTreeSet::new();
        assert_eq!(
            model.check_profiles(&available),
            Err(FaultWellFormednessError::UnresolvedProfile {
                name: "storage-posix-v1".to_owned()
            })
        );
        available.insert("storage-posix-v1".to_owned());
        assert_eq!(model.check_profiles(&available), Ok(()));
    }
}
