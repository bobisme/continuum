//! The assembled Intent Contract: RFC 0037's whole document, its one canonical
//! byte spelling, and the `in_*` identity that spelling defines.
//!
//! # What this module adds to the fifteen field groups
//!
//! The nine PR-4 field-group bones each delivered one protected group — its types,
//! its strict reader, its canonical encoder, and its own identity. What none of them
//! could deliver is the *document*: RFC 0037's contract is a twenty-key object with
//! `additionalProperties: false`, and six of its groups (`scope`, `trust_boundaries`,
//! `completion_policy`, `nondeterminism`, `abstraction_maps`, `security_policy`) plus
//! its four header/metadata keys (`schema_id`, `schema_epoch`, `intent_id`, `name`)
//! had no owning bone, because the plan's nine implementation bullets do not name
//! them. They are modelled here rather than in six further modules: none of them
//! carries a property AST, each is a string set, a closed enum, a scalar, or a keyed
//! array of two strings, and the reason they exist at all is that a contract cannot be
//! assembled without them.
//!
//! [`IntentContract`] is therefore the first type in the crate that *is* an artifact
//! the rest of the system names. Everything below it was a part.
//!
//! # ID1: exactly what the identity preimage is
//!
//! > **ID1.** The `in_*` identity MUST derive from a canonical encoding of the
//! > semantic content: sorted keys, property ASTs in the CPNF-1 normal form specified
//! > in the next section (alpha-renamed binders, flattened and ordered
//! > associative/commutative connectives), and no comment/whitespace/label content.
//! >
//! > **ID2.** The exclusion list is **closed**. Exactly these bytes are excluded from
//! > the preimage: `name`; `expression.source`; `expression.normal_form`; every
//! > `$comment`; and `intent_id` itself, since a self-reference cannot be part of its
//! > own preimage. […] Nothing else is excluded — in particular the `policy` table,
//! > `policy_reviewers`, `schema_id`, and `schema_epoch` are **in** the preimage.
//! >
//! > — `notes/plan/rfcs/0037-intent-contract.md`
//!
//! [`IntentContract::identity`] is [`IntentIdentity`], and per ADR-0013 that identity
//! *is* [`IntentContract::identity_preimage_bytes`] — not a digest of them. Those
//! bytes are the canonical JSON encoding of an object carrying **exactly these
//! eighteen keys**, and nothing else:
//!
//! ```text
//! abstraction_maps  assumptions   assurance          bounds
//! claims            completion_policy                fairness
//! fault_model       nondeterminism                   observers
//! optimization      policy        policy_reviewers   schema_epoch
//! schema_id         scope         security_policy    trust_boundaries
//! ```
//!
//! That list is the schema's twenty top-level keys minus the ID2 exclusions, and the
//! exclusions are applied at exactly two levels:
//!
//! - **Top level.** `name` is absent (ID2: "metadata and MUST NOT affect identity")
//!   and `intent_id` is absent (ID2: "a self-reference cannot be part of its own
//!   preimage"). `policy_reviewers` is present *even when the document omits it*,
//!   encoded as the empty object, because ID2 puts it in the preimage and ID5 forbids
//!   "absent" and "empty" from yielding two identities for one meaning.
//! - **Inside `claims[]` and `assumptions[]`.** Their `expression` objects drop
//!   `source` and `normal_form`; this module does not re-implement that reduction, it
//!   calls [`crate::property::ClaimSet::identity_preimage_json`] and
//!   [`crate::assumptions::AssumptionSet::identity_preimage_json`], which are the same
//!   `Layer::Identity` spelling those two modules have used for their own group
//!   identities since they landed.
//!
//! `$comment` never appears, at any depth, because no encoder in this crate emits one
//! and every reader rejects unknown keys. [`IntentContract::identity_preimage_json`]
//! is public so that this claim is *checkable* rather than asserted — the exit
//! evidence walks the preimage recursively and fails if any excluded key is reachable
//! from it.
//!
//! The thirteen groups whose artifact form carries nothing on the exclusion list
//! (`observers`, `bounds`, `fault_model`, `fairness`, `assurance`, `optimization`,
//! `policy`, `policy_reviewers`, and the six modelled here) contribute the *same*
//! bytes to both spellings. That is a fact about their encoders, not a convention, and
//! the recursive walk is what holds it.
//!
//! # ID5, and the crate-wide array-ordering rule
//!
//! ID5 fixes the byte spelling — "object keys sorted ascending by Unicode code point,
//! no insignificant whitespace, UTF-8 output, integers in shortest decimal form […]
//! and explicit encoding of empty sets" — and says nothing about the order of *array*
//! members. `bn-7f23` adopted an answer for `observers`/`fault_model`/`fairness` and
//! flagged that every group with a declared unit key needs the same one. It is stated
//! here once, for the whole crate:
//!
//! > **Array ordering.** An array whose members are a set (`uniqueItems`) or are keyed
//! > by a declared RFC 0031 unit key is encoded **ascending by that unit locator, in
//! > Unicode code-point order** — the rule ID5 already states for object keys.
//!
//! Surveyed across all fifteen groups as they are encoded today, every one of them
//! already obeys it, and three of them only because a deliberate choice was made:
//!
//! | Array | Ordered by | How |
//! |---|---|---|
//! | `claims[]`, `assumptions[]` | `id` | `BTreeMap<UnitKey, _>`; `UnitKey`'s `Ord` is `String`'s |
//! | `observers[]` | `id` | `BTreeMap<ObserverId, _>` |
//! | `observers[].{events,…}` | the element | `BTreeSet<String>` |
//! | `fairness[]` | `kind:action` | `BTreeMap<FairnessKey, _>`, and [`crate::fairness::FairnessKind`]'s `Ord` is **hand-written as `wire()` order** so the pair sorts exactly as its rendered locator does |
//! | `fault_model.enabled` | the wire token | `BTreeSet<FaultClass>`, and [`crate::faults::FaultClass`]'s `Ord` is **hand-written as `wire()` order**, not the schema's enum-declaration order |
//! | `fault_model.profiles`, `optimization.*`, `security_policy.*`, `scope.components`, `trust_boundaries.*`, `policy_reviewers[…]` | the element | `BTreeSet<String>` |
//! | `assurance.accepted_evidence_classes` | the wire token | `BTreeMap<&'static str, EvidenceClass>` — **keyed by the token on purpose**, because `EvidenceClass` is `continuum-value`'s type and its ordering is RFC 0010's listing order |
//! | `nondeterminism[]` | `site` | `BTreeMap<ChoiceSite, _>` |
//! | `abstraction_maps[]` | `role` | `BTreeMap<MapRole, _>` |
//! | `scope.fragments` | the wire token | `BTreeMap<&'static str, Fragment>` — **keyed by the token on purpose**; see below |
//!
//! [`crate::ast::Fragment`] derives `Ord`, so a `BTreeSet<Fragment>` iterates in
//! *declaration* order (`Finite, Symbolic, Temporal, Probabilistic, Theorem,
//! Runtime`), which is not code-point order (`Finite, Probabilistic, Runtime,
//! Symbolic, Temporal, Theorem`). `Fragment`'s `Ord` is not changed here — it is
//! landed public behavior and the parameter type of two `check_fragments` methods —
//! so [`Scope`] holds its fragments keyed by wire token instead, exactly as
//! `assurance_policy.rs` holds its evidence classes. The trap is recorded because it
//! is the one place in the crate where using the obvious container would have
//! produced a non-canonical document that still round-tripped.
//!
//! No group disagrees with any other, so no module's output is re-ordered here.
//!
//! # Decode is strict; the cross-field rules are a separate verdict
//!
//! [`IntentContract::decode`] rejects rather than repairs, and it decides exactly the
//! rules that can be decided from the document alone: JSON admissibility, the closed
//! twenty-key top level, required keys, every group's own reader (which carries W1 for
//! its unit keys, W4 for fairness conditions, and W6 for policy verbs), and **W10** —
//! `schema_id` and `schema_epoch` are `const` in the schema, so a document disagreeing
//! with the header is schema-invalid and is refused here rather than reported later.
//!
//! Everything that needs *two* groups, or a fact from outside the document, is
//! [`IntentContract::check`], which returns a [`ContractVerdict`] rather than an
//! error:
//!
//! | Rule | What it relates | Discharged by |
//! |---|---|---|
//! | W2 | `claims[].observer` → `observers[].id` | [`crate::property::ClaimSet::check_observers`] |
//! | W3 | `scope.fragments` → every `expression.fragment` | `ClaimSet`/`AssumptionSet::check_fragments` |
//! | W7 | `policy` → `policy_reviewers` | [`crate::change_policy::PolicyTable::check_reviewers`] |
//! | W8 | `fault_model.profiles` → the snapshot's domain packs; `abstraction_maps[].map_id` → the §16 correspondence graph | [`crate::faults::FaultModel::check_profiles`] and [`AbstractionMaps`] |
//! | W9 | `intent_id` → the identity of the contract's own canonical encoding | [`IntentContract::check`] against [`CheckEnvironment::with_minted_intent_id`] |
//!
//! The split is not stylistic. `notes/plan/schemas/examples/intent-contract.example.json`
//! is validated against the schema on every `just check` and **violates W7**: nine of
//! its fifteen fields carry the `review` verb and the instance has no
//! `policy_reviewers` key at all (found by `bn-fp0g`, adjudication pending on
//! `bn-16pyr`). A schema-valid document that a checker rejects must still *decode*, or
//! this crate could not read the artifact the dossier says is legal — so the violation
//! surfaces as [`ContractFinding::UnenforceableReview`] from `check`, and
//! `IntentContract::decode` accepts the example unchanged. W5 (bound variables are
//! bound) is not decided at any level here: it quantifies over "the declared free
//! variables of the model", which is a `continuum-model` fact this crate is below.
//!
//! Each of the four rules a landed group already owns is *decided by that group*: this
//! module calls `check_observers`, `check_fragments`, `check_reviewers`, and
//! `check_profiles` rather than opening a second implementation of a rule whose first
//! one is the adversarial suites' spec. Those four report their **first** violation, in
//! their own declared order, so a verdict names one violation per (rule, group) pair —
//! a fixed one, not an arbitrary one — and fixing it and re-running surfaces the next.
//! The two halves this module owns outright, `abstraction_maps` under W8 and the whole
//! of W9, report every violation they find.
//!
//! A verdict fails closed. [`CheckEnvironment`] has no "skip this rule" spelling: a
//! contract naming a domain-pack profile is checked against the profiles you supply,
//! and if you supply none, the profile is unresolved — which is W8's own instruction
//! ("rejected at acceptance time; it MUST NOT be deferred into an `unknown`
//! classification later"). Likewise W9 with no minted handle is
//! [`ContractFinding::DeclaredIdentityUnverified`], never a pass.
//!
//! ## Why the group errors are wrapped one-for-one
//!
//! `bn-3lfmi` left a sketch for a `FieldDecodeError` trait that would let
//! [`ContractDecodeError`] carry one boxed field error instead of ten typed variants.
//! It is not adopted, and the reason is the crate's own test suite: the nine
//! adversarial suites are the behavioral spec and they match on *variants*
//! (`ObserverDecodeError::UnknownField`, `FaultDecodeError::UnknownToken`, …). Boxing
//! erases exactly that, and buys a shorter enum in a crate that is inside the smallest
//! trust base and has no dynamic dispatch anywhere else. Ten variants and ten `From`
//! impls are mechanical; a checker that can no longer name which vocabulary failed is
//! not.
//!
//! # Immutability
//!
//! > Accepted contracts are immutable. A revision never edits a stored contract: it
//! > mints a new `in_*` and records a supersession edge. Nothing here permits an
//! > in-place edit of an accepted contract, including a "cosmetic" one.
//! >
//! > — RFC 0037, "Versioning and revision"
//!
//! No type in this module has a `&mut self` method, and neither does any other type in
//! the crate. Every accessor lends, every derivation returns a new value, and
//! [`CheckEnvironment`]'s builder consumes `self` rather than borrowing it mutably.
//! That is what INV-001 — "ordinary operations cannot mutate intent" — costs at the
//! API level, and the following does not compile:
//!
//! ```compile_fail
//! # use continuum_intent::contract::IntentContract;
//! # fn demo(contract: IntentContract) {
//! let mut contract = contract;
//! // There is no setter, and no `_mut` accessor, for any field group.
//! contract.set_name(Some("renamed".to_owned()));
//! # }
//! ```
//!
//! while the same value is freely *readable*, which is the passing companion that
//! keeps the check above from rotting into a test of its own preamble:
//!
//! ```
//! # use continuum_intent::contract::IntentContract;
//! # fn demo(contract: IntentContract) {
//! let contract = contract;
//! let _ = contract.name();
//! let _ = contract.to_artifact_bytes();
//! # }
//! ```

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use continuum_value::identity::ContentHasher;

use crate::assumptions::{AssumptionDecodeError, AssumptionSet};
use crate::assurance_policy::{AssuranceDecodeError, AssurancePolicy};
use crate::ast::Fragment;
use crate::bounds::{Bounds, BoundsDecodeError};
use crate::canonical_json::{Json, JsonError};
use crate::change_policy::{ChangePolicyDecodeError, PolicyField, PolicyReviewers, PolicyTable};
use crate::fairness::{FairnessDecodeError, FairnessSet};
use crate::faults::{FaultDecodeError, FaultModel};
use crate::identity::canonical_identity;
use crate::observers::{ObserverDecodeError, ObserverSet};
use crate::optimization::{Optimization, OptimizationDecodeError};
use crate::property::{ClaimSet, PropertyDecodeError};

/// The artifact-class identity of the governing schema.
///
/// The schema declares this a `const`, so it is a fact about the document rather than
/// a value a contract chooses; W10 is therefore decided by the decoder.
pub const SCHEMA_ID: &str = "https://continuum.dev/schema/intent-contract.json";

/// The schema epoch this crate is written against.
///
/// Also a schema `const`. It "advances only on a breaking change to this artifact
/// class […] and pins no semantics" (`schemas/intent-contract.schema.json`).
pub const SCHEMA_EPOCH: i64 = 1;

/// Every key `intent-contract.schema.json` admits at the top level, in code-point
/// order.
///
/// The schema sets `additionalProperties: false`, so this array is the closed
/// vocabulary the decoder tests against: a reader that ignored an unknown key would
/// accept documents the schema rejects, and — for a contract — would silently drop a
/// protected field group.
const TOP_LEVEL_KEYS: [&str; 20] = [
    "abstraction_maps",
    "assumptions",
    "assurance",
    "bounds",
    "claims",
    "completion_policy",
    "fairness",
    "fault_model",
    "intent_id",
    "name",
    "nondeterminism",
    "observers",
    "optimization",
    "policy",
    "policy_reviewers",
    "schema_epoch",
    "schema_id",
    "scope",
    "security_policy",
    "trust_boundaries",
];

// --- header ------------------------------------------------------------------------

/// The contract's own handle: `intent_id`, matching `^in_[A-Za-z0-9_-]+$`.
///
/// Plan §4.4: "Handles carry a kind prefix but are otherwise structureless." The
/// prefix `in_` is this artifact class's; what follows it is opaque, so this type
/// validates the pattern and nothing more. It deliberately does not *derive* the
/// handle — W9's check is a comparison against a handle some lane minted, not a
/// re-derivation this crate could perform without making a hash-vendor decision
/// (ADR-0013, and [`IntentContract::mint_intent_id`]).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct IntentId(String);

impl IntentId {
    /// Validate and wrap an intent handle.
    ///
    /// # Errors
    ///
    /// [`ContractError::MalformedIntentId`] when the string is not
    /// `^in_[A-Za-z0-9_-]+$`.
    pub fn new(handle: &str) -> Result<Self, ContractError> {
        let Some(suffix) = handle.strip_prefix("in_") else {
            return Err(ContractError::MalformedIntentId {
                value: handle.to_owned(),
            });
        };
        if suffix.is_empty() || !suffix.chars().all(is_handle_char) {
            return Err(ContractError::MalformedIntentId {
                value: handle.to_owned(),
            });
        }
        Ok(Self(handle.to_owned()))
    }

    /// The handle.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for IntentId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// The `[A-Za-z0-9_-]` class plan §4.4 handles are drawn from.
const fn is_handle_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_' || c == '-'
}

// --- scope -------------------------------------------------------------------------

/// `scope`: what the contract's claims are stated about, and in which semantic
/// fragments.
///
/// `fragments` is held keyed by wire token rather than as a `BTreeSet<Fragment>`
/// because [`Fragment`]'s derived `Ord` is declaration order, and the crate-wide
/// ordering rule (see this module's documentation) is code-point order on the locator.
/// [`Scope::declared_fragments`] rebuilds the set the two `check_fragments` methods
/// take.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Scope {
    components: BTreeSet<String>,
    fragments: BTreeMap<&'static str, Fragment>,
    abstraction_level: Option<String>,
}

impl Scope {
    /// Build a scope.
    ///
    /// `abstraction_level` is [`Option`] because RFC 0037 gives `null` a meaning of
    /// its own — "no abstraction level is declared. It is not the empty string, and
    /// the two MUST NOT be normalized to each other" — so `Some(String::new())` and
    /// `None` are different scopes and encode differently.
    ///
    /// # Errors
    ///
    /// - [`ContractError::DuplicateElement`] — `uniqueItems` on `components` or
    ///   `fragments`.
    /// - [`ContractError::NoFragments`] — W3's first half and the schema's
    ///   `minItems: 1`: an empty declaration makes every unit `unsupported` and every
    ///   promotion blocked, silently.
    pub fn new(
        components: impl IntoIterator<Item = String>,
        fragments: impl IntoIterator<Item = Fragment>,
        abstraction_level: Option<String>,
    ) -> Result<Self, ContractError> {
        let components = unique_strings(components, "scope.components")?;
        let mut declared: BTreeMap<&'static str, Fragment> = BTreeMap::new();
        for fragment in fragments {
            if declared.insert(fragment.wire(), fragment).is_some() {
                return Err(ContractError::DuplicateElement {
                    field: "scope.fragments",
                    value: fragment.wire().to_owned(),
                });
            }
        }
        if declared.is_empty() {
            return Err(ContractError::NoFragments);
        }
        Ok(Self {
            components,
            fragments: declared,
            abstraction_level,
        })
    }

    /// The declared components, in code-point order.
    pub fn components(&self) -> impl Iterator<Item = &str> {
        self.components.iter().map(String::as_str)
    }

    /// The declared fragments, in wire code-point order.
    pub fn fragments(&self) -> impl Iterator<Item = Fragment> + '_ {
        self.fragments.values().copied()
    }

    /// The declared fragments as the set W3's two checkers take.
    #[must_use]
    pub fn declared_fragments(&self) -> BTreeSet<Fragment> {
        self.fragments.values().copied().collect()
    }

    /// The declared abstraction level, or `None` for the `null` spelling.
    #[must_use]
    pub fn abstraction_level(&self) -> Option<&str> {
        self.abstraction_level.as_deref()
    }

    /// The artifact-form JSON.
    ///
    /// `abstraction_level` is always written, `null` included. RFC 0037 enumerates the
    /// keys whose absence reads as `null` (`bounds.values`, `bounds.depth`,
    /// `fairness[].condition`) and does not list this one, but it does give `null` the
    /// meaning "no abstraction level is declared", which is also what an absent key
    /// says; encoding it explicitly is ID5's own argument against "absent" and a
    /// declared value for one meaning yielding two identities. *Raised as a flag
    /// against RFC 0037's "Absence, null, and defaults" list, which should say so.*
    #[must_use]
    pub fn artifact_json(&self) -> Json {
        Json::object([
            (
                "abstraction_level".to_owned(),
                self.abstraction_level
                    .as_ref()
                    .map_or(Json::Null, |level| Json::String(level.clone())),
            ),
            ("components".to_owned(), string_array(&self.components)),
            (
                "fragments".to_owned(),
                Json::Array(
                    self.fragments
                        .keys()
                        .map(|token| Json::String((*token).to_owned()))
                        .collect(),
                ),
            ),
        ])
        .unwrap_or(Json::Null)
    }

    /// Decode a scope from an already-parsed JSON object.
    ///
    /// # Errors
    ///
    /// [`ContractDecodeError`], naming the field that failed.
    pub fn from_json(json: &Json) -> Result<Self, ContractDecodeError> {
        let fields = object(json, "scope")?;
        reject_unknown_keys(
            fields,
            "scope",
            &["abstraction_level", "components", "fragments"],
        )?;
        let components = string_set(
            fields.get("components"),
            "scope.components",
            Presence::Required,
        )?;
        let tokens = string_set(
            fields.get("fragments"),
            "scope.fragments",
            Presence::Required,
        )?;
        let mut fragments = Vec::with_capacity(tokens.len());
        for token in &tokens {
            fragments.push(Fragment::from_wire(token).ok_or_else(|| {
                ContractDecodeError::UnknownToken {
                    field: "scope.fragments[]",
                    token: token.clone(),
                }
            })?);
        }
        let abstraction_level = match fields.get("abstraction_level") {
            None | Some(Json::Null) => None,
            Some(Json::String(level)) => Some(level.clone()),
            Some(other) => {
                return Err(ContractDecodeError::TypeMismatch {
                    field: "scope.abstraction_level",
                    expected: "string or null",
                    found: other.type_name(),
                });
            }
        };
        Ok(Self::new(components, fragments, abstraction_level)?)
    }
}

// --- trust boundaries --------------------------------------------------------------

/// `trust_boundaries`: the pair of string sets whose *opaque* half is the one docs/41
/// calls the "opaque escape".
///
/// Both keys are required by the schema and both are always encoded, empty included.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrustBoundaries {
    trusted: BTreeSet<String>,
    opaque: BTreeSet<String>,
}

impl TrustBoundaries {
    /// Build the pair.
    ///
    /// # Errors
    ///
    /// [`ContractError::DuplicateElement`] — `uniqueItems` on either set.
    pub fn new(
        trusted: impl IntoIterator<Item = String>,
        opaque: impl IntoIterator<Item = String>,
    ) -> Result<Self, ContractError> {
        Ok(Self {
            trusted: unique_strings(trusted, "trust_boundaries.trusted")?,
            opaque: unique_strings(opaque, "trust_boundaries.opaque")?,
        })
    }

    /// The trusted set, in code-point order.
    pub fn trusted(&self) -> impl Iterator<Item = &str> {
        self.trusted.iter().map(String::as_str)
    }

    /// The opaque set, in code-point order.
    pub fn opaque(&self) -> impl Iterator<Item = &str> {
        self.opaque.iter().map(String::as_str)
    }

    /// The artifact-form JSON.
    #[must_use]
    pub fn artifact_json(&self) -> Json {
        Json::object([
            ("opaque".to_owned(), string_array(&self.opaque)),
            ("trusted".to_owned(), string_array(&self.trusted)),
        ])
        .unwrap_or(Json::Null)
    }

    /// Decode the pair from an already-parsed JSON object.
    ///
    /// # Errors
    ///
    /// [`ContractDecodeError`], naming the field that failed.
    pub fn from_json(json: &Json) -> Result<Self, ContractDecodeError> {
        let fields = object(json, "trust_boundaries")?;
        reject_unknown_keys(fields, "trust_boundaries", &["opaque", "trusted"])?;
        let trusted = string_set(
            fields.get("trusted"),
            "trust_boundaries.trusted",
            Presence::Required,
        )?;
        let opaque = string_set(
            fields.get("opaque"),
            "trust_boundaries.opaque",
            Presence::Required,
        )?;
        Ok(Self::new(trusted, opaque)?)
    }
}

// --- completion policy -------------------------------------------------------------

/// `completion_policy`: the RFC 0015 behavior-completion policy for terminal states.
///
/// A scalar, and the schema's description says the thing that matters about it: "No
/// engine may silently change this."
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum CompletionPolicy {
    /// Terminal states stutter forever.
    StutterForever,
    /// A terminal state is a violation.
    DeadlockViolation,
    /// Only finite traces are considered.
    FiniteTraceOnly,
    /// The behavior is closed.
    Closed,
}

impl CompletionPolicy {
    /// Every policy, in schema-declaration order.
    pub const ALL: [Self; 4] = [
        Self::StutterForever,
        Self::DeadlockViolation,
        Self::FiniteTraceOnly,
        Self::Closed,
    ];

    /// The wire literal.
    #[must_use]
    pub const fn wire(self) -> &'static str {
        match self {
            Self::StutterForever => "stutter-forever",
            Self::DeadlockViolation => "deadlock-violation",
            Self::FiniteTraceOnly => "finite-trace-only",
            Self::Closed => "closed",
        }
    }

    /// Recover a policy from its wire literal, or `None` — never a default.
    #[must_use]
    pub fn from_wire(token: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|policy| policy.wire() == token)
    }
}

impl fmt::Display for CompletionPolicy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.wire())
    }
}

// --- nondeterminism ----------------------------------------------------------------

/// The choice class declared at one nondeterminism site.
///
/// "Probabilistic, timed, and epistemic classes are declarative until the ADR-0016
/// lanes ship" (the schema's own description). They are members of the closed
/// vocabulary regardless, because a contract that could not *name* them would have no
/// way to record that a site is one of them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ChoiceClass {
    /// Adversarial choice.
    Demonic,
    /// Cooperative choice.
    Angelic,
    /// Scheduler choice.
    Scheduler,
    /// Probabilistic choice (declarative).
    Probabilistic,
    /// Timed choice (declarative).
    Timed,
    /// Epistemic choice (declarative).
    Epistemic,
}

impl ChoiceClass {
    /// Every class, in schema-declaration order.
    pub const ALL: [Self; 6] = [
        Self::Demonic,
        Self::Angelic,
        Self::Scheduler,
        Self::Probabilistic,
        Self::Timed,
        Self::Epistemic,
    ];

    /// The wire literal.
    #[must_use]
    pub const fn wire(self) -> &'static str {
        match self {
            Self::Demonic => "demonic",
            Self::Angelic => "angelic",
            Self::Scheduler => "scheduler",
            Self::Probabilistic => "probabilistic",
            Self::Timed => "timed",
            Self::Epistemic => "epistemic",
        }
    }

    /// Recover a class from its wire literal, or `None` — never a default.
    #[must_use]
    pub fn from_wire(token: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|class| class.wire() == token)
    }
}

impl fmt::Display for ChoiceClass {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.wire())
    }
}

/// `nondeterminism[].site`: the RFC 0031 unit key of a choice site.
///
/// The schema types it as a bare string. The one rule enforced is the one the diff
/// schema requires of a unit locator — non-empty — because an empty locator could not
/// appear as a `unit` in an `intent_changes` record, so a change to it could not be
/// reported. The same argument [`crate::faults::ProfileName`] and
/// [`crate::fairness::ActionName`] make.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ChoiceSite(String);

impl ChoiceSite {
    /// Validate and wrap a site name.
    ///
    /// # Errors
    ///
    /// [`ContractError::EmptyUnitKey`].
    pub fn new(site: &str) -> Result<Self, ContractError> {
        if site.is_empty() {
            return Err(ContractError::EmptyUnitKey {
                field: "nondeterminism[].site",
            });
        }
        Ok(Self(site.to_owned()))
    }

    /// The name, which is also the RFC 0031 unit locator.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ChoiceSite {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// `nondeterminism`: typed choice classes per choice site, keyed by `site` (W1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NondeterminismSet {
    sites: BTreeMap<ChoiceSite, ChoiceClass>,
}

impl NondeterminismSet {
    /// Build the set, enforcing W1 on `site`.
    ///
    /// # Errors
    ///
    /// [`ContractError::DuplicateUnitKey`] — W1: duplicates make the diff's unit
    /// keying ambiguous, "a soundness hazard rather than a matter of taste".
    pub fn new(
        entries: impl IntoIterator<Item = (ChoiceSite, ChoiceClass)>,
    ) -> Result<Self, ContractError> {
        let mut sites: BTreeMap<ChoiceSite, ChoiceClass> = BTreeMap::new();
        for (site, class) in entries {
            if sites.insert(site.clone(), class).is_some() {
                return Err(ContractError::DuplicateUnitKey {
                    field: "nondeterminism[].site",
                    unit: site.to_string(),
                });
            }
        }
        Ok(Self { sites })
    }

    /// The empty declaration — "a *declaration that the set is empty*, […] not
    /// 'unspecified'" (RFC 0037).
    #[must_use]
    pub fn empty() -> Self {
        Self {
            sites: BTreeMap::new(),
        }
    }

    /// Every entry, in site code-point order.
    pub fn iter(&self) -> impl Iterator<Item = (&ChoiceSite, ChoiceClass)> {
        self.sites.iter().map(|(site, class)| (site, *class))
    }

    /// The class declared at a site, if any.
    #[must_use]
    pub fn class_at(&self, site: &ChoiceSite) -> Option<ChoiceClass> {
        self.sites.get(site).copied()
    }

    /// How many sites are declared.
    #[must_use]
    pub fn len(&self) -> usize {
        self.sites.len()
    }

    /// Whether the contract declares the set empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.sites.is_empty()
    }

    /// The artifact-form JSON: an array ascending by `site`.
    #[must_use]
    pub fn artifact_json(&self) -> Json {
        Json::Array(
            self.sites
                .iter()
                .map(|(site, class)| {
                    Json::object([
                        ("class".to_owned(), Json::String(class.wire().to_owned())),
                        ("site".to_owned(), Json::String(site.as_str().to_owned())),
                    ])
                    .unwrap_or(Json::Null)
                })
                .collect(),
        )
    }

    /// Decode the set from an already-parsed JSON array.
    ///
    /// # Errors
    ///
    /// [`ContractDecodeError`], naming the field that failed.
    pub fn from_json(json: &Json) -> Result<Self, ContractDecodeError> {
        let items = array(json, "nondeterminism")?;
        let mut entries = Vec::with_capacity(items.len());
        for item in items {
            let fields = object(item, "nondeterminism[]")?;
            reject_unknown_keys(fields, "nondeterminism[]", &["class", "site"])?;
            let site = ChoiceSite::new(required_string(
                fields.get("site"),
                "nondeterminism[].site",
            )?)?;
            let token = required_string(fields.get("class"), "nondeterminism[].class")?;
            let class =
                ChoiceClass::from_wire(token).ok_or_else(|| ContractDecodeError::UnknownToken {
                    field: "nondeterminism[].class",
                    token: token.to_owned(),
                })?;
            entries.push((site, class));
        }
        Ok(Self::new(entries)?)
    }
}

// --- abstraction maps --------------------------------------------------------------

/// `abstraction_maps[].role`: the RFC 0031 unit key of a map binding.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct MapRole(String);

impl MapRole {
    /// Validate and wrap a role.
    ///
    /// # Errors
    ///
    /// [`ContractError::EmptyUnitKey`].
    pub fn new(role: &str) -> Result<Self, ContractError> {
        if role.is_empty() {
            return Err(ContractError::EmptyUnitKey {
                field: "abstraction_maps[].role",
            });
        }
        Ok(Self(role.to_owned()))
    }

    /// The role, which is also the RFC 0031 unit locator.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for MapRole {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// `abstraction_maps[].map_id`: a §4.4 content-addressed handle, pattern
/// `^[a-z][a-z0-9_]*_[A-Za-z0-9_-]+$`.
///
/// The pattern is validated here because the schema declares it, and INV-003 makes the
/// schema the authority on shape: a reader that admitted a handle the schema rejects
/// would accept documents `just check`'s dossier validator does not.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct MapId(String);

impl MapId {
    /// Validate and wrap a map handle.
    ///
    /// # Errors
    ///
    /// [`ContractError::MalformedMapId`].
    pub fn new(handle: &str) -> Result<Self, ContractError> {
        if matches_handle_pattern(handle) {
            Ok(Self(handle.to_owned()))
        } else {
            Err(ContractError::MalformedMapId {
                value: handle.to_owned(),
            })
        }
    }

    /// The handle.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for MapId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// `^[a-z][a-z0-9_]*_[A-Za-z0-9_-]+$`, decided without a regex engine.
///
/// The prefix class contains `_`, so the separator is not determined by the first
/// underscore; every underscore position is tried, which is what the regex's
/// backtracking would do.
fn matches_handle_pattern(handle: &str) -> bool {
    let bytes = handle.as_bytes();
    if bytes.first().is_none_or(|c| !c.is_ascii_lowercase()) {
        return false;
    }
    (1..bytes.len()).any(|separator| {
        bytes[separator] == b'_'
            && bytes[1..separator]
                .iter()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || *c == b'_')
            && separator + 1 < bytes.len()
            && bytes[separator + 1..]
                .iter()
                .all(|c| c.is_ascii_alphanumeric() || *c == b'_' || *c == b'-')
    })
}

/// `abstraction_maps`: the §16 correspondence maps the claims are stated against,
/// keyed by `role` (W1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AbstractionMaps {
    bindings: BTreeMap<MapRole, MapId>,
}

impl AbstractionMaps {
    /// Build the bindings, enforcing W1 on `role`.
    ///
    /// # Errors
    ///
    /// [`ContractError::DuplicateUnitKey`].
    pub fn new(
        bindings: impl IntoIterator<Item = (MapRole, MapId)>,
    ) -> Result<Self, ContractError> {
        let mut keyed: BTreeMap<MapRole, MapId> = BTreeMap::new();
        for (role, map_id) in bindings {
            if keyed.insert(role.clone(), map_id).is_some() {
                return Err(ContractError::DuplicateUnitKey {
                    field: "abstraction_maps[].role",
                    unit: role.to_string(),
                });
            }
        }
        Ok(Self { bindings: keyed })
    }

    /// The empty declaration.
    #[must_use]
    pub fn empty() -> Self {
        Self {
            bindings: BTreeMap::new(),
        }
    }

    /// Every binding, in role code-point order.
    pub fn iter(&self) -> impl Iterator<Item = (&MapRole, &MapId)> {
        self.bindings.iter()
    }

    /// The map bound to a role, if any.
    #[must_use]
    pub fn map_for(&self, role: &MapRole) -> Option<&MapId> {
        self.bindings.get(role)
    }

    /// How many bindings there are.
    #[must_use]
    pub fn len(&self) -> usize {
        self.bindings.len()
    }

    /// Whether the contract declares the set empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.bindings.is_empty()
    }

    /// W8's second half, given the map ids the §16 correspondence graph resolves.
    ///
    /// > **W8 — Profiles and maps resolve.** […] every `abstraction_maps[].map_id`
    /// > MUST resolve in the §16 correspondence graph. An unresolvable reference is
    /// > rejected at acceptance time; it MUST NOT be deferred into an `unknown`
    /// > classification later.
    /// >
    /// > — RFC 0037
    ///
    /// Returns every unresolved binding, in role order, rather than the first: the
    /// contract-level verdict is a list, and an author fixing one dangling map should
    /// not have to re-run to discover the second.
    #[must_use]
    pub fn unresolved(&self, available: &BTreeSet<String>) -> Vec<(MapRole, MapId)> {
        self.bindings
            .iter()
            .filter(|(_, map_id)| !available.contains(map_id.as_str()))
            .map(|(role, map_id)| (role.clone(), map_id.clone()))
            .collect()
    }

    /// The artifact-form JSON: an array ascending by `role`.
    #[must_use]
    pub fn artifact_json(&self) -> Json {
        Json::Array(
            self.bindings
                .iter()
                .map(|(role, map_id)| {
                    Json::object([
                        (
                            "map_id".to_owned(),
                            Json::String(map_id.as_str().to_owned()),
                        ),
                        ("role".to_owned(), Json::String(role.as_str().to_owned())),
                    ])
                    .unwrap_or(Json::Null)
                })
                .collect(),
        )
    }

    /// Decode the bindings from an already-parsed JSON array.
    ///
    /// # Errors
    ///
    /// [`ContractDecodeError`], naming the field that failed.
    pub fn from_json(json: &Json) -> Result<Self, ContractDecodeError> {
        let items = array(json, "abstraction_maps")?;
        let mut bindings = Vec::with_capacity(items.len());
        for item in items {
            let fields = object(item, "abstraction_maps[]")?;
            reject_unknown_keys(fields, "abstraction_maps[]", &["map_id", "role"])?;
            let role = MapRole::new(required_string(
                fields.get("role"),
                "abstraction_maps[].role",
            )?)?;
            let map_id = MapId::new(required_string(
                fields.get("map_id"),
                "abstraction_maps[].map_id",
            )?)?;
            bindings.push((role, map_id));
        }
        Ok(Self::new(bindings)?)
    }
}

// --- security policy ---------------------------------------------------------------

/// `security_policy.data_classification`, a total order (RFC 0037).
///
/// `Ord` is the *strength* order the RFC states — `public < internal < confidential <
/// restricted` — so that "weakening it is a protected change" is a comparison a
/// checker can make. It is never an encoding order: the field is a scalar, so the
/// crate-wide array-ordering rule does not reach it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum DataClassification {
    /// Public.
    Public,
    /// Internal.
    Internal,
    /// Confidential.
    Confidential,
    /// Restricted.
    Restricted,
}

impl DataClassification {
    /// Every classification, weakest first.
    pub const ALL: [Self; 4] = [
        Self::Public,
        Self::Internal,
        Self::Confidential,
        Self::Restricted,
    ];

    /// The wire literal.
    #[must_use]
    pub const fn wire(self) -> &'static str {
        match self {
            Self::Public => "public",
            Self::Internal => "internal",
            Self::Confidential => "confidential",
            Self::Restricted => "restricted",
        }
    }

    /// Recover a classification from its wire literal, or `None` — never a default.
    #[must_use]
    pub fn from_wire(token: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|level| level.wire() == token)
    }
}

impl fmt::Display for DataClassification {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.wire())
    }
}

/// `security_policy`: data classification, redaction classes, capability
/// requirements.
///
/// Both optional sets are always encoded, empty included: "An absent optional set MUST
/// be read as the empty set for classification, and MUST be encoded explicitly as the
/// empty set when the identity preimage is built (ID5), so that 'absent' and 'empty'
/// cannot yield two identities for one meaning."
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SecurityPolicy {
    data_classification: DataClassification,
    redaction_classes: BTreeSet<String>,
    capability_requirements: BTreeSet<String>,
}

impl SecurityPolicy {
    /// Build the group.
    ///
    /// # Errors
    ///
    /// [`ContractError::DuplicateElement`] — `uniqueItems` on either set.
    pub fn new(
        data_classification: DataClassification,
        redaction_classes: impl IntoIterator<Item = String>,
        capability_requirements: impl IntoIterator<Item = String>,
    ) -> Result<Self, ContractError> {
        Ok(Self {
            data_classification,
            redaction_classes: unique_strings(
                redaction_classes,
                "security_policy.redaction_classes",
            )?,
            capability_requirements: unique_strings(
                capability_requirements,
                "security_policy.capability_requirements",
            )?,
        })
    }

    /// The declared classification.
    #[must_use]
    pub const fn data_classification(&self) -> DataClassification {
        self.data_classification
    }

    /// The redaction classes, in code-point order.
    pub fn redaction_classes(&self) -> impl Iterator<Item = &str> {
        self.redaction_classes.iter().map(String::as_str)
    }

    /// The capability requirements, in code-point order.
    pub fn capability_requirements(&self) -> impl Iterator<Item = &str> {
        self.capability_requirements.iter().map(String::as_str)
    }

    /// The artifact-form JSON.
    #[must_use]
    pub fn artifact_json(&self) -> Json {
        Json::object([
            (
                "capability_requirements".to_owned(),
                string_array(&self.capability_requirements),
            ),
            (
                "data_classification".to_owned(),
                Json::String(self.data_classification.wire().to_owned()),
            ),
            (
                "redaction_classes".to_owned(),
                string_array(&self.redaction_classes),
            ),
        ])
        .unwrap_or(Json::Null)
    }

    /// Decode the group from an already-parsed JSON object.
    ///
    /// # Errors
    ///
    /// [`ContractDecodeError`], naming the field that failed.
    pub fn from_json(json: &Json) -> Result<Self, ContractDecodeError> {
        let fields = object(json, "security_policy")?;
        reject_unknown_keys(
            fields,
            "security_policy",
            &[
                "capability_requirements",
                "data_classification",
                "redaction_classes",
            ],
        )?;
        let token = required_string(
            fields.get("data_classification"),
            "security_policy.data_classification",
        )?;
        let classification = DataClassification::from_wire(token).ok_or_else(|| {
            ContractDecodeError::UnknownToken {
                field: "security_policy.data_classification",
                token: token.to_owned(),
            }
        })?;
        let redaction = string_set(
            fields.get("redaction_classes"),
            "security_policy.redaction_classes",
            Presence::Optional,
        )?;
        let capabilities = string_set(
            fields.get("capability_requirements"),
            "security_policy.capability_requirements",
            Presence::Optional,
        )?;
        Ok(Self::new(classification, redaction, capabilities)?)
    }
}

// --- the contract ------------------------------------------------------------------

canonical_identity! {
    /// The canonical identity of a whole Intent Contract: RFC 0037's `in_*` preimage.
    ///
    /// Per ADR-0013 this type *is* the preimage bytes described in this module's
    /// documentation — the eighteen-key canonical JSON object with the ID2 exclusions
    /// applied — so two contracts are the same contract precisely when those bytes
    /// agree, and [`IntentIdentity::digest`] can only ever index.
    ///
    /// It is deliberately not the same type as `intent_id`: [`IntentId`] is a §4.4
    /// handle, a *name* for this identity minted by whatever lane stored the contract,
    /// and W9 relates the two by comparison rather than by definition.
    IntentIdentity
}

/// The eighteen field groups an [`IntentContract`] is assembled from.
///
/// A plain argument bundle. `IntentContract::new` takes eighteen values and a
/// positional constructor for eighteen values of six distinct string-ish types is a
/// transposition defect waiting to happen, so they are named instead. The fields are
/// public because this struct is *authoring* input: it is consumed by value, an
/// [`IntentContract`] shares no storage with it, and there is no path from a contract
/// back to the parts it was built from. Editing a draft is not mutating a contract,
/// which is what INV-001 is about.
#[derive(Debug, Clone)]
pub struct ContractParts {
    /// `intent_id` — the handle this contract is stored under.
    pub intent_id: IntentId,
    /// `name` — optional metadata, excluded from the identity preimage (ID2).
    pub name: Option<String>,
    /// `claims` — at least one.
    pub claims: ClaimSet,
    /// `assumptions`.
    pub assumptions: AssumptionSet,
    /// `observers`.
    pub observers: ObserverSet,
    /// `abstraction_maps`.
    pub abstraction_maps: AbstractionMaps,
    /// `scope`.
    pub scope: Scope,
    /// `trust_boundaries`.
    pub trust_boundaries: TrustBoundaries,
    /// `bounds`.
    pub bounds: Bounds,
    /// `fault_model`.
    pub fault_model: FaultModel,
    /// `fairness`.
    pub fairness: FairnessSet,
    /// `completion_policy`.
    pub completion_policy: CompletionPolicy,
    /// `nondeterminism`.
    pub nondeterminism: NondeterminismSet,
    /// `assurance`.
    pub assurance: AssurancePolicy,
    /// `optimization`.
    pub optimization: Optimization,
    /// `security_policy`.
    pub security_policy: SecurityPolicy,
    /// `policy` — all fifteen keys.
    pub policy: PolicyTable,
    /// `policy_reviewers` — possibly empty; in the preimage either way (ID2).
    pub policy_reviewers: PolicyReviewers,
}

/// A whole Intent Contract: RFC 0037's document, canonically encoded, with its
/// identity.
///
/// Construction cannot fail — every part validated itself on the way in, and the
/// cross-field rules are [`IntentContract::check`]'s, deliberately, so that a
/// schema-valid document which a checker rejects can still be read (see this module's
/// documentation for why the dossier's own example needs that).
///
/// [`PartialEq`] is hand-written and compares the eighteen groups. It never consults
/// [`identity`](Self::identity), so a future identity that is not a pure function of
/// content cannot quietly become the equality relation. `name` participates: two
/// contracts differing only in `name` are *different documents* with the *same*
/// identity, which is exactly ID2's claim and is asserted as such in the tests.
#[derive(Debug, Clone)]
pub struct IntentContract {
    parts: ContractParts,
    identity: IntentIdentity,
}

impl IntentContract {
    /// Assemble a contract from its parts and derive its identity.
    #[must_use]
    pub fn new(parts: ContractParts) -> Self {
        let identity = IntentIdentity::of_bytes(preimage_json(&parts).to_canonical_bytes());
        Self { parts, identity }
    }

    /// `intent_id`.
    #[must_use]
    pub const fn intent_id(&self) -> &IntentId {
        &self.parts.intent_id
    }

    /// `name`, which is metadata and is outside the identity preimage (ID2).
    #[must_use]
    pub fn name(&self) -> Option<&str> {
        self.parts.name.as_deref()
    }

    /// `claims`.
    #[must_use]
    pub const fn claims(&self) -> &ClaimSet {
        &self.parts.claims
    }

    /// `assumptions`.
    #[must_use]
    pub const fn assumptions(&self) -> &AssumptionSet {
        &self.parts.assumptions
    }

    /// `observers`.
    #[must_use]
    pub const fn observers(&self) -> &ObserverSet {
        &self.parts.observers
    }

    /// `abstraction_maps`.
    #[must_use]
    pub const fn abstraction_maps(&self) -> &AbstractionMaps {
        &self.parts.abstraction_maps
    }

    /// `scope`.
    #[must_use]
    pub const fn scope(&self) -> &Scope {
        &self.parts.scope
    }

    /// `trust_boundaries`.
    #[must_use]
    pub const fn trust_boundaries(&self) -> &TrustBoundaries {
        &self.parts.trust_boundaries
    }

    /// `bounds`.
    #[must_use]
    pub const fn bounds(&self) -> &Bounds {
        &self.parts.bounds
    }

    /// `fault_model`.
    #[must_use]
    pub const fn fault_model(&self) -> &FaultModel {
        &self.parts.fault_model
    }

    /// `fairness`.
    #[must_use]
    pub const fn fairness(&self) -> &FairnessSet {
        &self.parts.fairness
    }

    /// `completion_policy`.
    #[must_use]
    pub const fn completion_policy(&self) -> CompletionPolicy {
        self.parts.completion_policy
    }

    /// `nondeterminism`.
    #[must_use]
    pub const fn nondeterminism(&self) -> &NondeterminismSet {
        &self.parts.nondeterminism
    }

    /// `assurance`.
    #[must_use]
    pub const fn assurance(&self) -> &AssurancePolicy {
        &self.parts.assurance
    }

    /// `optimization`.
    #[must_use]
    pub const fn optimization(&self) -> &Optimization {
        &self.parts.optimization
    }

    /// `security_policy`.
    #[must_use]
    pub const fn security_policy(&self) -> &SecurityPolicy {
        &self.parts.security_policy
    }

    /// `policy`.
    #[must_use]
    pub const fn policy(&self) -> &PolicyTable {
        &self.parts.policy
    }

    /// `policy_reviewers`.
    #[must_use]
    pub const fn policy_reviewers(&self) -> &PolicyReviewers {
        &self.parts.policy_reviewers
    }

    /// The contract's identity: the ID1/ID2 preimage bytes themselves (ADR-0013).
    #[must_use]
    pub const fn identity(&self) -> &IntentIdentity {
        &self.identity
    }

    /// The ID1/ID2 identity preimage, as JSON.
    ///
    /// Public so that the exclusion list is checkable: the exit evidence walks this
    /// document recursively and fails if `name`, `intent_id`, `source`, `normal_form`,
    /// or any `$comment` is reachable from it.
    #[must_use]
    pub fn identity_preimage_json(&self) -> Json {
        preimage_json(&self.parts)
    }

    /// The ID1/ID2 identity preimage, as canonical bytes.
    ///
    /// Equal to `self.identity().canonical_bytes()` by construction.
    #[must_use]
    pub fn identity_preimage_bytes(&self) -> Vec<u8> {
        self.identity.canonical_bytes().to_vec()
    }

    /// The artifact-form JSON: the schema-conformant contract document.
    ///
    /// This is the preimage plus `intent_id`, plus `name` when one is declared, and
    /// with `claims`/`assumptions` carrying their display-only `source` and their
    /// `normal_form` declaration.
    #[must_use]
    pub fn artifact_json(&self) -> Json {
        let mut fields = preimage_fields(&self.parts);
        fields.push((
            "intent_id".to_owned(),
            Json::String(self.parts.intent_id.as_str().to_owned()),
        ));
        if let Some(name) = &self.parts.name {
            fields.push(("name".to_owned(), Json::String(name.clone())));
        }
        fields.push((
            "assumptions".to_owned(),
            self.parts.assumptions.artifact_json(),
        ));
        fields.push(("claims".to_owned(), self.parts.claims.artifact_json()));
        Json::object(fields).unwrap_or(Json::Null)
    }

    /// The artifact-form bytes, under the ID5 canonical-JSON rules.
    ///
    /// One byte spelling: `decode(c.to_artifact_bytes()) == c`, and re-encoding is
    /// idempotent regardless of how the input was spelled.
    #[must_use]
    pub fn to_artifact_bytes(&self) -> Vec<u8> {
        self.artifact_json().to_canonical_bytes()
    }

    /// One admissible `in_*` handle for this contract's identity.
    ///
    /// ADR-0013's second clause: the digest *indexes*, it is not the identity. The
    /// hasher is a type parameter so this crate makes no hash-vendor decision, and a
    /// lane keying a store by the minted handle must still compare
    /// [`IntentIdentity::canonical_bytes`] on a hit. Provided because W9 has to be
    /// checkable without standing up a store; a lane that mints handles differently
    /// passes its own through [`CheckEnvironment::with_minted_intent_id`].
    #[must_use]
    pub fn mint_intent_id<H: ContentHasher>(&self) -> String {
        format!("in_{}", self.identity.digest::<H>().to_token())
    }

    /// Decode a contract from its artifact form.
    ///
    /// Strict: unknown top-level keys, missing required keys, wrong types, unknown
    /// tokens, a header disagreeing with the schema's `const`s (W10), and every
    /// group's own rejections are refusals, never repairs.
    ///
    /// # Errors
    ///
    /// [`ContractDecodeError`], naming the field or key that failed.
    pub fn decode(bytes: &[u8]) -> Result<Self, ContractDecodeError> {
        Self::from_json(&Json::parse(bytes)?)
    }

    /// Decode a contract from an already-parsed JSON value.
    ///
    /// # Errors
    ///
    /// [`ContractDecodeError`], as [`IntentContract::decode`].
    pub fn from_json(json: &Json) -> Result<Self, ContractDecodeError> {
        let fields = object(json, "intent-contract")?;
        reject_unknown_keys(fields, "intent-contract", &TOP_LEVEL_KEYS)?;

        // W10 first: a document whose header names another schema is not this schema's
        // to read, so nothing below it is interpreted.
        let schema_id = required_string(fields.get("schema_id"), "schema_id")?;
        if schema_id != SCHEMA_ID {
            return Err(ContractDecodeError::HeaderMismatch {
                field: "schema_id",
                expected: SCHEMA_ID.to_owned(),
                found: schema_id.to_owned(),
            });
        }
        let epoch = match fields.get("schema_epoch") {
            Some(Json::Integer(value)) => *value,
            Some(other) => {
                return Err(ContractDecodeError::TypeMismatch {
                    field: "schema_epoch",
                    expected: "integer",
                    found: other.type_name(),
                });
            }
            None => {
                return Err(ContractDecodeError::MissingField {
                    field: "schema_epoch",
                });
            }
        };
        if epoch != SCHEMA_EPOCH {
            return Err(ContractDecodeError::HeaderMismatch {
                field: "schema_epoch",
                expected: SCHEMA_EPOCH.to_string(),
                found: epoch.to_string(),
            });
        }

        let intent_id = IntentId::new(required_string(fields.get("intent_id"), "intent_id")?)?;
        let name = match fields.get("name") {
            None => None,
            Some(Json::String(text)) => Some(text.clone()),
            Some(other) => {
                return Err(ContractDecodeError::TypeMismatch {
                    field: "name",
                    expected: "string",
                    found: other.type_name(),
                });
            }
        };

        let parts = ContractParts {
            intent_id,
            name,
            claims: ClaimSet::from_json(required(fields.get("claims"), "claims")?)?,
            assumptions: AssumptionSet::from_json(required(
                fields.get("assumptions"),
                "assumptions",
            )?)?,
            observers: ObserverSet::from_json(required(fields.get("observers"), "observers")?)?,
            abstraction_maps: AbstractionMaps::from_json(required(
                fields.get("abstraction_maps"),
                "abstraction_maps",
            )?)?,
            scope: Scope::from_json(required(fields.get("scope"), "scope")?)?,
            trust_boundaries: TrustBoundaries::from_json(required(
                fields.get("trust_boundaries"),
                "trust_boundaries",
            )?)?,
            bounds: Bounds::from_json(required(fields.get("bounds"), "bounds")?)?,
            fault_model: FaultModel::from_json(required(
                fields.get("fault_model"),
                "fault_model",
            )?)?,
            fairness: FairnessSet::from_json(required(fields.get("fairness"), "fairness")?)?,
            completion_policy: {
                let token = required_string(fields.get("completion_policy"), "completion_policy")?;
                CompletionPolicy::from_wire(token).ok_or_else(|| {
                    ContractDecodeError::UnknownToken {
                        field: "completion_policy",
                        token: token.to_owned(),
                    }
                })?
            },
            nondeterminism: NondeterminismSet::from_json(required(
                fields.get("nondeterminism"),
                "nondeterminism",
            )?)?,
            assurance: AssurancePolicy::from_json(required(fields.get("assurance"), "assurance")?)?,
            optimization: Optimization::from_json(required(
                fields.get("optimization"),
                "optimization",
            )?)?,
            security_policy: SecurityPolicy::from_json(required(
                fields.get("security_policy"),
                "security_policy",
            )?)?,
            policy: PolicyTable::from_json(required(fields.get("policy"), "policy")?)?,
            // Absent reads as the empty map, and the encoder writes it back
            // explicitly: ID2 puts `policy_reviewers` in the preimage, so "absent" and
            // "{}" must not be two identities for one meaning.
            policy_reviewers: match fields.get("policy_reviewers") {
                None => PolicyReviewers::empty(),
                Some(value) => PolicyReviewers::from_json(value)?,
            },
        };
        Ok(Self::new(parts))
    }

    /// The cross-field well-formedness rules: W2, W3, W7, W8, and W9.
    ///
    /// Returns a [`ContractVerdict`] rather than an error because a contract can be
    /// schema-valid, decodable, and still not acceptable — the dossier's own validated
    /// example is exactly that, violating W7 — and the two answers must be
    /// distinguishable. Every rule fails closed: a fact the environment does not
    /// supply is a finding, never a pass.
    ///
    /// W1 (unique unit keys), W4 (fairness conditions are state formulas), W6 (verb
    /// applicability), and W10 (header agrees) are decided earlier, by the group
    /// constructors and by [`IntentContract::decode`]; a contract in hand already
    /// satisfies them. W5 (bound variables are bound) quantifies over the model's
    /// declared free variables, which is not a fact this crate holds.
    #[must_use]
    pub fn check(&self, environment: &CheckEnvironment) -> ContractVerdict {
        let mut findings = Vec::new();

        // W2 — every claims[].observer resolves.
        if let Err(error) = self
            .parts
            .claims
            .check_observers(&self.parts.observers.declared_ids())
        {
            findings.push(ContractFinding::from_claim_rule(&error));
        }

        // W3 — every expression fragment is declared. Both AST-carrying groups.
        let declared = self.parts.scope.declared_fragments();
        if let Err(error) = self.parts.claims.check_fragments(&declared) {
            findings.push(ContractFinding::from_claim_rule(&error));
        }
        if let Err(error) = self.parts.assumptions.check_fragments(&declared) {
            findings.push(ContractFinding::from_assumption_rule(&error));
        }

        // W7 — a review verb names its reviewers.
        if let Err(error) = self
            .parts
            .policy
            .check_reviewers(&self.parts.policy_reviewers)
        {
            let crate::change_policy::WellFormednessError::UnenforceableReview { field } = error;
            findings.push(ContractFinding::UnenforceableReview { field });
        }

        // W8 — profiles and maps resolve.
        if let Err(error) = self
            .parts
            .fault_model
            .check_profiles(&environment.domain_pack_profiles)
        {
            let crate::faults::FaultWellFormednessError::UnresolvedProfile { name } = error;
            findings.push(ContractFinding::UnresolvedProfile { name });
        }
        for (role, map_id) in self
            .parts
            .abstraction_maps
            .unresolved(&environment.correspondence_map_ids)
        {
            findings.push(ContractFinding::UnresolvedAbstractionMap {
                role: role.to_string(),
                map_id: map_id.to_string(),
            });
        }

        // W9 — the declared handle names this contract's own canonical encoding.
        match &environment.minted_intent_id {
            None => findings.push(ContractFinding::DeclaredIdentityUnverified),
            Some(minted) if minted != self.parts.intent_id.as_str() => {
                findings.push(ContractFinding::DeclaredIdentityMismatch {
                    declared: self.parts.intent_id.to_string(),
                    minted: minted.clone(),
                });
            }
            Some(_) => {}
        }

        ContractVerdict { findings }
    }
}

impl PartialEq for IntentContract {
    fn eq(&self, other: &Self) -> bool {
        let left = &self.parts;
        let right = &other.parts;
        left.intent_id == right.intent_id
            && left.name == right.name
            && left.claims == right.claims
            && left.assumptions == right.assumptions
            && left.observers == right.observers
            && left.abstraction_maps == right.abstraction_maps
            && left.scope == right.scope
            && left.trust_boundaries == right.trust_boundaries
            && left.bounds == right.bounds
            && left.fault_model == right.fault_model
            && left.fairness == right.fairness
            && left.completion_policy == right.completion_policy
            && left.nondeterminism == right.nondeterminism
            && left.assurance == right.assurance
            && left.optimization == right.optimization
            && left.security_policy == right.security_policy
            && left.policy == right.policy
            && left.policy_reviewers == right.policy_reviewers
    }
}

impl Eq for IntentContract {}

/// The seventeen preimage keys that are the same in both spellings, plus the two that
/// are not, assembled as an unsorted field list.
///
/// [`Json::object`] sorts, so the order here is the order the RFC's field table reads
/// in rather than the byte order; the encoder decides the bytes.
fn preimage_fields(parts: &ContractParts) -> Vec<(String, Json)> {
    vec![
        (
            "abstraction_maps".to_owned(),
            parts.abstraction_maps.artifact_json(),
        ),
        ("assurance".to_owned(), parts.assurance.artifact_json()),
        ("bounds".to_owned(), parts.bounds.artifact_json()),
        (
            "completion_policy".to_owned(),
            Json::String(parts.completion_policy.wire().to_owned()),
        ),
        ("fairness".to_owned(), parts.fairness.artifact_json()),
        ("fault_model".to_owned(), parts.fault_model.artifact_json()),
        (
            "nondeterminism".to_owned(),
            parts.nondeterminism.artifact_json(),
        ),
        ("observers".to_owned(), parts.observers.artifact_json()),
        (
            "optimization".to_owned(),
            parts.optimization.artifact_json(),
        ),
        ("policy".to_owned(), parts.policy.artifact_json()),
        (
            "policy_reviewers".to_owned(),
            parts.policy_reviewers.artifact_json(),
        ),
        ("schema_epoch".to_owned(), Json::Integer(SCHEMA_EPOCH)),
        ("schema_id".to_owned(), Json::String(SCHEMA_ID.to_owned())),
        ("scope".to_owned(), parts.scope.artifact_json()),
        (
            "security_policy".to_owned(),
            parts.security_policy.artifact_json(),
        ),
        (
            "trust_boundaries".to_owned(),
            parts.trust_boundaries.artifact_json(),
        ),
    ]
}

/// The ID1/ID2 preimage: the eighteen keys, with `claims` and `assumptions` in their
/// `Layer::Identity` spelling and `intent_id`/`name` absent.
fn preimage_json(parts: &ContractParts) -> Json {
    let mut fields = preimage_fields(parts);
    fields.push((
        "assumptions".to_owned(),
        parts.assumptions.identity_preimage_json(),
    ));
    fields.push(("claims".to_owned(), parts.claims.identity_preimage_json()));
    Json::object(fields).unwrap_or(Json::Null)
}

// --- the checker -------------------------------------------------------------------

/// The facts from outside the contract that W8 and W9 need.
///
/// There is deliberately no way to say "skip this rule". A contract naming a
/// domain-pack profile is checked against the profiles supplied here, and supplying
/// none means the profile is unresolved — which is W8's own instruction. Built by
/// consuming `self`, so a [`CheckEnvironment`] is as immutable as everything else in
/// this crate.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CheckEnvironment {
    domain_pack_profiles: BTreeSet<String>,
    correspondence_map_ids: BTreeSet<String>,
    minted_intent_id: Option<String>,
}

impl CheckEnvironment {
    /// An environment that resolves nothing and mints nothing.
    ///
    /// Every W8 reference and W9's check fail against it, which is the fail-closed
    /// default: a checker that was told nothing has verified nothing.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The domain-pack profiles the snapshot makes available (W8).
    #[must_use]
    pub fn with_domain_pack_profiles(self, profiles: impl IntoIterator<Item = String>) -> Self {
        Self {
            domain_pack_profiles: profiles.into_iter().collect(),
            ..self
        }
    }

    /// The map ids the §16 correspondence graph resolves (W8).
    #[must_use]
    pub fn with_correspondence_maps(self, map_ids: impl IntoIterator<Item = String>) -> Self {
        Self {
            correspondence_map_ids: map_ids.into_iter().collect(),
            ..self
        }
    }

    /// The `in_*` handle the storing lane minted for this contract's canonical
    /// encoding (W9).
    #[must_use]
    pub fn with_minted_intent_id(self, handle: impl Into<String>) -> Self {
        Self {
            minted_intent_id: Some(handle.into()),
            ..self
        }
    }
}

/// Which RFC 0037 well-formedness rule a finding belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum WellFormednessRule {
    /// W2 — observer references resolve.
    W2,
    /// W3 — fragments are declared.
    W3,
    /// W7 — reviewers are named where required.
    W7,
    /// W8 — profiles and maps resolve.
    W8,
    /// W9 — declared identity agrees.
    W9,
}

impl WellFormednessRule {
    /// The rule's name, as RFC 0037 spells it.
    #[must_use]
    pub const fn wire(self) -> &'static str {
        match self {
            Self::W2 => "W2",
            Self::W3 => "W3",
            Self::W7 => "W7",
            Self::W8 => "W8",
            Self::W9 => "W9",
        }
    }
}

impl fmt::Display for WellFormednessRule {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.wire())
    }
}

/// One reason a contract is not well formed.
///
/// Distinct from [`ContractDecodeError`] on purpose: these rules relate two field
/// groups, or a field group and a fact from outside the document, so none of them can
/// be decided while decoding and none of them is the decoder's to report.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContractFinding {
    /// W2: a `claims[].observer` names no declared observer.
    DanglingObserver {
        /// The claim's unit key.
        unit: String,
        /// The unresolved reference.
        observer: String,
    },
    /// W3: `scope.fragments` is empty.
    ///
    /// [`Scope::new`] refuses to build such a scope, so a contract in hand cannot
    /// reach this. It exists because a rule is never discharged by an `unreachable!`:
    /// if a future edit relaxes `Scope`, the rule reports rather than panics.
    NoDeclaredFragments,
    /// W3: an expression declares a fragment `scope.fragments` does not.
    UndeclaredFragment {
        /// Which AST-carrying group, `claims` or `assumptions`.
        group: &'static str,
        /// The unit key.
        unit: String,
        /// The undeclared fragment.
        fragment: Fragment,
    },
    /// W3: an expression omits its fragment where more than one is declared.
    AmbiguousFragment {
        /// Which AST-carrying group, `claims` or `assumptions`.
        group: &'static str,
        /// The unit key.
        unit: String,
        /// How many fragments the contract declares.
        declared: usize,
    },
    /// W7: a field's verb is `review` and no principal is named for it.
    UnenforceableReview {
        /// The field.
        field: PolicyField,
    },
    /// W8: a declared fault profile is not available in the snapshot's domain packs.
    UnresolvedProfile {
        /// The unresolved name.
        name: String,
    },
    /// W8: an abstraction map does not resolve in the §16 correspondence graph.
    UnresolvedAbstractionMap {
        /// The binding's role.
        role: String,
        /// The unresolved handle.
        map_id: String,
    },
    /// W9: no lane minted a handle for these bytes, so the declared one is unchecked.
    DeclaredIdentityUnverified,
    /// W9: the declared `intent_id` is not the handle these bytes mint.
    DeclaredIdentityMismatch {
        /// What the document declares.
        declared: String,
        /// What the lane minted for the contract's own canonical encoding.
        minted: String,
    },
}

impl ContractFinding {
    /// The rule this finding belongs to.
    #[must_use]
    pub const fn rule(&self) -> WellFormednessRule {
        match self {
            Self::DanglingObserver { .. } => WellFormednessRule::W2,
            Self::NoDeclaredFragments
            | Self::UndeclaredFragment { .. }
            | Self::AmbiguousFragment { .. } => WellFormednessRule::W3,
            Self::UnenforceableReview { .. } => WellFormednessRule::W7,
            Self::UnresolvedProfile { .. } | Self::UnresolvedAbstractionMap { .. } => {
                WellFormednessRule::W8
            }
            Self::DeclaredIdentityUnverified | Self::DeclaredIdentityMismatch { .. } => {
                WellFormednessRule::W9
            }
        }
    }

    /// Lift `claims`'s relational error into a contract-level finding.
    fn from_claim_rule(error: &crate::property::WellFormednessError) -> Self {
        use crate::property::WellFormednessError as E;
        match error {
            E::DanglingObserver { unit, observer } => Self::DanglingObserver {
                unit: unit.clone(),
                observer: observer.clone(),
            },
            E::NoDeclaredFragments => Self::NoDeclaredFragments,
            E::UndeclaredFragment { unit, fragment } => Self::UndeclaredFragment {
                group: "claims",
                unit: unit.clone(),
                fragment: *fragment,
            },
            E::AmbiguousFragment { unit, declared } => Self::AmbiguousFragment {
                group: "claims",
                unit: unit.clone(),
                declared: *declared,
            },
        }
    }

    /// Lift `assumptions`'s relational error into a contract-level finding.
    fn from_assumption_rule(error: &crate::assumptions::WellFormednessError) -> Self {
        use crate::assumptions::WellFormednessError as E;
        match error {
            E::NoDeclaredFragments => Self::NoDeclaredFragments,
            E::UndeclaredFragment { unit, fragment } => Self::UndeclaredFragment {
                group: "assumptions",
                unit: unit.clone(),
                fragment: *fragment,
            },
            E::AmbiguousFragment { unit, declared } => Self::AmbiguousFragment {
                group: "assumptions",
                unit: unit.clone(),
                declared: *declared,
            },
        }
    }
}

impl fmt::Display for ContractFinding {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DanglingObserver { unit, observer } => write!(
                f,
                "W2: claim {unit:?} binds the observer {observer:?}, which no observers[].id \
                 declares"
            ),
            Self::NoDeclaredFragments => f.write_str(
                "W3: scope.fragments is empty; every unit would classify unsupported and every \
                 promotion would be blocked, silently",
            ),
            Self::UndeclaredFragment {
                group,
                unit,
                fragment,
            } => write!(
                f,
                "W3: {group} unit {unit:?} is stated in the {fragment} fragment, which \
                 scope.fragments does not declare"
            ),
            Self::AmbiguousFragment {
                group,
                unit,
                declared,
            } => write!(
                f,
                "W3: {group} unit {unit:?} declares no fragment and the contract declares \
                 {declared}"
            ),
            Self::UnenforceableReview { field } => write!(
                f,
                "W7: the field `{field}` carries the review verb and policy_reviewers names no \
                 principal for it; a review verb naming no reviewer is an unenforceable block"
            ),
            Self::UnresolvedProfile { name } => write!(
                f,
                "W8: fault_model.profiles names {name:?}, which the snapshot's domain packs do \
                 not provide"
            ),
            Self::UnresolvedAbstractionMap { role, map_id } => write!(
                f,
                "W8: abstraction_maps[{role:?}] names {map_id:?}, which the §16 correspondence \
                 graph does not resolve"
            ),
            Self::DeclaredIdentityUnverified => f.write_str(
                "W9: no minted handle was supplied, so the declared intent_id is unchecked; an \
                 unverified identity is not a verified one",
            ),
            Self::DeclaredIdentityMismatch { declared, minted } => write!(
                f,
                "W9: the contract declares {declared:?} but its own canonical encoding mints \
                 {minted:?}"
            ),
        }
    }
}

/// The result of [`IntentContract::check`]: every cross-field rule this contract
/// fails.
///
/// A verdict is not an error type. It is the checker's answer, and an *empty* verdict
/// is the only affirmative one — INV-008's "never a bare boolean" applied to
/// acceptance.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContractVerdict {
    findings: Vec<ContractFinding>,
}

impl ContractVerdict {
    /// Whether every checked rule holds.
    #[must_use]
    pub fn is_well_formed(&self) -> bool {
        self.findings.is_empty()
    }

    /// Every failing rule, in the order the rules are checked.
    #[must_use]
    pub fn findings(&self) -> &[ContractFinding] {
        &self.findings
    }

    /// Every finding belonging to one rule.
    pub fn findings_for(&self, rule: WellFormednessRule) -> impl Iterator<Item = &ContractFinding> {
        self.findings
            .iter()
            .filter(move |finding| finding.rule() == rule)
    }

    /// The verdict as a `Result`, for callers that want `?`.
    ///
    /// # Errors
    ///
    /// The verdict itself, when it carries any finding.
    pub fn into_result(self) -> Result<(), Self> {
        if self.is_well_formed() {
            Ok(())
        } else {
            Err(self)
        }
    }
}

impl fmt::Display for ContractVerdict {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.findings.is_empty() {
            return f.write_str("the contract satisfies W2, W3, W7, W8, and W9");
        }
        for (index, finding) in self.findings.iter().enumerate() {
            if index > 0 {
                f.write_str("; ")?;
            }
            write!(f, "{finding}")?;
        }
        Ok(())
    }
}

// --- shared decode helpers ---------------------------------------------------------

/// Whether an absent key is admissible.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Presence {
    /// The schema's `required` list names it.
    Required,
    /// Absent reads as the empty set, and the encoder writes it back explicitly.
    Optional,
}

fn object<'a>(
    json: &'a Json,
    field: &'static str,
) -> Result<&'a BTreeMap<String, Json>, ContractDecodeError> {
    json.as_object()
        .ok_or_else(|| ContractDecodeError::TypeMismatch {
            field,
            expected: "object",
            found: json.type_name(),
        })
}

fn array<'a>(json: &'a Json, field: &'static str) -> Result<&'a [Json], ContractDecodeError> {
    json.as_array()
        .ok_or_else(|| ContractDecodeError::TypeMismatch {
            field,
            expected: "array",
            found: json.type_name(),
        })
}

fn required<'a>(
    value: Option<&'a Json>,
    field: &'static str,
) -> Result<&'a Json, ContractDecodeError> {
    value.ok_or(ContractDecodeError::MissingField { field })
}

fn required_string<'a>(
    value: Option<&'a Json>,
    field: &'static str,
) -> Result<&'a str, ContractDecodeError> {
    let json = required(value, field)?;
    json.as_str()
        .ok_or_else(|| ContractDecodeError::TypeMismatch {
            field,
            expected: "string",
            found: json.type_name(),
        })
}

fn reject_unknown_keys(
    fields: &BTreeMap<String, Json>,
    field: &'static str,
    known: &[&str],
) -> Result<(), ContractDecodeError> {
    for key in fields.keys() {
        if !known.contains(&key.as_str()) {
            return Err(ContractDecodeError::UnknownField {
                field,
                key: key.clone(),
            });
        }
    }
    Ok(())
}

fn string_set(
    value: Option<&Json>,
    field: &'static str,
    presence: Presence,
) -> Result<Vec<String>, ContractDecodeError> {
    match (value, presence) {
        (None, Presence::Optional) => Ok(Vec::new()),
        (None, Presence::Required) => Err(ContractDecodeError::MissingField { field }),
        (Some(Json::Array(items)), _) => items
            .iter()
            .map(|item| {
                item.as_str()
                    .map(str::to_owned)
                    .ok_or_else(|| ContractDecodeError::TypeMismatch {
                        field,
                        expected: "string",
                        found: item.type_name(),
                    })
            })
            .collect(),
        (Some(other), _) => Err(ContractDecodeError::TypeMismatch {
            field,
            expected: "array",
            found: other.type_name(),
        }),
    }
}

/// Collect a `uniqueItems` array, rejecting a repeat rather than absorbing it.
fn unique_strings(
    values: impl IntoIterator<Item = String>,
    field: &'static str,
) -> Result<BTreeSet<String>, ContractError> {
    let mut set = BTreeSet::new();
    for value in values {
        if !set.insert(value.clone()) {
            return Err(ContractError::DuplicateElement { field, value });
        }
    }
    Ok(set)
}

fn string_array(values: &BTreeSet<String>) -> Json {
    Json::Array(values.iter().map(|v| Json::String(v.clone())).collect())
}

// --- errors ------------------------------------------------------------------------

/// Why a value this module owns cannot be built.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContractError {
    /// `intent_id` does not match `^in_[A-Za-z0-9_-]+$`.
    MalformedIntentId {
        /// The offending value.
        value: String,
    },
    /// An `abstraction_maps[].map_id` does not match the §4.4 handle pattern.
    MalformedMapId {
        /// The offending value.
        value: String,
    },
    /// A unit key is the empty string.
    EmptyUnitKey {
        /// The field's path.
        field: &'static str,
    },
    /// W1: two members of one keyed array share a unit key.
    DuplicateUnitKey {
        /// The field's path.
        field: &'static str,
        /// The repeated key.
        unit: String,
    },
    /// A `uniqueItems` array repeats a member.
    DuplicateElement {
        /// The field's path.
        field: &'static str,
        /// The repeated member.
        value: String,
    },
    /// W3 and the schema's `minItems: 1`: `scope.fragments` is empty.
    NoFragments,
}

impl fmt::Display for ContractError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MalformedIntentId { value } => write!(
                f,
                "{value:?} is not an intent handle; plan §4.4 handles for this class match \
                 ^in_[A-Za-z0-9_-]+$"
            ),
            Self::MalformedMapId { value } => write!(
                f,
                "{value:?} is not a §4.4 content-addressed handle; abstraction_maps[].map_id \
                 matches ^[a-z][a-z0-9_]*_[A-Za-z0-9_-]+$"
            ),
            Self::EmptyUnitKey { field } => write!(
                f,
                "`{field}` is empty; an empty locator could not appear as a `unit` in an \
                 intent_changes record, so a change to it could not be reported"
            ),
            Self::DuplicateUnitKey { field, unit } => write!(
                f,
                "`{field}` repeats {unit:?}; W1 rejects duplicates because they make the diff's \
                 unit keying ambiguous"
            ),
            Self::DuplicateElement { field, value } => write!(
                f,
                "`{field}` repeats {value:?} and the schema sets uniqueItems: true"
            ),
            Self::NoFragments => f.write_str(
                "scope.fragments is empty; W3 and the schema's minItems: 1 both refuse it, \
                 because an empty declaration makes every unit unsupported and every promotion \
                 blocked, silently",
            ),
        }
    }
}

impl core::error::Error for ContractError {}

/// Why a byte string is not an Intent Contract.
///
/// Every variant is a rejection, never a repair: "an intent contract is never
/// best-effort decoded" (RFC 0037). The ten field-group variants keep each group's own
/// typed error rather than boxing it — see this module's documentation for why the
/// `FieldDecodeError` trait sketch was not taken.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContractDecodeError {
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
    /// A closed vocabulary this module owns does not contain the token.
    UnknownToken {
        /// The field's path.
        field: &'static str,
        /// The offending token.
        token: String,
    },
    /// W10: `schema_id` or `schema_epoch` disagrees with the schema's `const`.
    HeaderMismatch {
        /// The field's path.
        field: &'static str,
        /// The schema's constant.
        expected: String,
        /// What the document declares.
        found: String,
    },
    /// A value this module owns is not well formed.
    Contract(ContractError),
    /// The `claims` group rejected the document.
    Claims(PropertyDecodeError),
    /// The `assumptions` group rejected the document.
    Assumptions(AssumptionDecodeError),
    /// The `observers` group rejected the document.
    Observers(ObserverDecodeError),
    /// The `bounds` group rejected the document.
    Bounds(BoundsDecodeError),
    /// The `fault_model` group rejected the document.
    Faults(FaultDecodeError),
    /// The `fairness` group rejected the document.
    Fairness(FairnessDecodeError),
    /// The `assurance` group rejected the document.
    Assurance(AssuranceDecodeError),
    /// The `optimization` group rejected the document.
    Optimization(OptimizationDecodeError),
    /// The `policy`/`policy_reviewers` group rejected the document.
    Policy(ChangePolicyDecodeError),
}

impl fmt::Display for ContractDecodeError {
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
                 the schema rejects — and, for a contract, would silently drop a protected field \
                 group"
            ),
            Self::UnknownToken { field, token } => write!(
                f,
                "`{field}` carries the unknown token {token:?}; the vocabulary is closed and an \
                 unrecognized member is a rejection, never a default"
            ),
            Self::HeaderMismatch {
                field,
                expected,
                found,
            } => write!(
                f,
                "`{field}` is {found:?} but this schema declares the constant {expected:?}; W10 \
                 refuses a document written against another schema rather than reading it under \
                 this one"
            ),
            Self::Contract(error) => error.fmt(f),
            Self::Claims(error) => error.fmt(f),
            Self::Assumptions(error) => error.fmt(f),
            Self::Observers(error) => error.fmt(f),
            Self::Bounds(error) => error.fmt(f),
            Self::Faults(error) => error.fmt(f),
            Self::Fairness(error) => error.fmt(f),
            Self::Assurance(error) => error.fmt(f),
            Self::Optimization(error) => error.fmt(f),
            Self::Policy(error) => error.fmt(f),
        }
    }
}

impl core::error::Error for ContractDecodeError {}

impl From<JsonError> for ContractDecodeError {
    fn from(error: JsonError) -> Self {
        Self::Json(error)
    }
}

impl From<ContractError> for ContractDecodeError {
    fn from(error: ContractError) -> Self {
        Self::Contract(error)
    }
}

impl From<PropertyDecodeError> for ContractDecodeError {
    fn from(error: PropertyDecodeError) -> Self {
        Self::Claims(error)
    }
}

impl From<AssumptionDecodeError> for ContractDecodeError {
    fn from(error: AssumptionDecodeError) -> Self {
        Self::Assumptions(error)
    }
}

impl From<ObserverDecodeError> for ContractDecodeError {
    fn from(error: ObserverDecodeError) -> Self {
        Self::Observers(error)
    }
}

impl From<BoundsDecodeError> for ContractDecodeError {
    fn from(error: BoundsDecodeError) -> Self {
        Self::Bounds(error)
    }
}

impl From<FaultDecodeError> for ContractDecodeError {
    fn from(error: FaultDecodeError) -> Self {
        Self::Faults(error)
    }
}

impl From<FairnessDecodeError> for ContractDecodeError {
    fn from(error: FairnessDecodeError) -> Self {
        Self::Fairness(error)
    }
}

impl From<AssuranceDecodeError> for ContractDecodeError {
    fn from(error: AssuranceDecodeError) -> Self {
        Self::Assurance(error)
    }
}

impl From<OptimizationDecodeError> for ContractDecodeError {
    fn from(error: OptimizationDecodeError) -> Self {
        Self::Optimization(error)
    }
}

impl From<ChangePolicyDecodeError> for ContractDecodeError {
    fn from(error: ChangePolicyDecodeError) -> Self {
        Self::Policy(error)
    }
}

#[cfg(test)]
mod tests {
    use continuum_value::assurance::AssuranceLevel;
    use continuum_value::identity::Fnv1aPlaceholder;

    use super::*;
    use crate::ast::{ComparisonOperator, Formula, Identifier, Literal, Term};
    use crate::bounds::{DeclaredBound, ExplorationBound};
    use crate::change_policy::PolicyVerb;
    use crate::faults::{FaultClass, ProfileName};
    use crate::observers::{Observer, ObserverId, ProjectionKind};
    use crate::property::{Claim, ClaimKind, PropertyExpression, UnitKey};

    fn claim(id: &str) -> Claim {
        let formula = Formula::always(Formula::compare(
            ComparisonOperator::Le,
            Term::State {
                name: Identifier::new("big").expect("identifier"),
                indices: Vec::new(),
            },
            Term::Literal {
                value: Literal::Integer(5),
            },
        ));
        Claim::new(
            UnitKey::new(id).expect("unit key"),
            ClaimKind::Safety,
            PropertyExpression::normalized(
                &formula,
                Some(Fragment::Finite),
                Some("big <= 5".to_owned()),
            )
            .expect("normalizes"),
            None,
        )
    }

    fn minimal_parts() -> ContractParts {
        ContractParts {
            intent_id: IntentId::new("in_test").expect("handle"),
            name: None,
            claims: ClaimSet::from_claims([claim("C1")]).expect("one claim"),
            assumptions: AssumptionSet::from_assumptions([]).expect("empty is legal"),
            observers: ObserverSet::from_observers([]).expect("empty is legal"),
            abstraction_maps: AbstractionMaps::empty(),
            scope: Scope::new(
                ["crate::register".to_owned()],
                [Fragment::Finite],
                Some("protocol".to_owned()),
            )
            .expect("scope"),
            trust_boundaries: TrustBoundaries::new([], []).expect("boundaries"),
            bounds: Bounds::new(
                ExplorationBound::Bounded(2),
                DeclaredBound::Declared(2),
                DeclaredBound::Declared(1),
                ExplorationBound::Bounded(20),
            )
            .expect("bounds"),
            fault_model: FaultModel::new([FaultClass::Crash], []).expect("faults"),
            fairness: FairnessSet::from_constraints([]).expect("empty is legal"),
            completion_policy: CompletionPolicy::StutterForever,
            nondeterminism: NondeterminismSet::empty(),
            assurance: AssurancePolicy::new(AssuranceLevel::Bounded, None, None, [])
                .expect("assurance"),
            optimization: Optimization::empty(),
            security_policy: SecurityPolicy::new(DataClassification::Internal, [], [])
                .expect("security"),
            policy: PolicyTable::all_unlocked(),
            policy_reviewers: PolicyReviewers::empty(),
        }
    }

    #[test]
    fn a_contract_round_trips_through_its_artifact_bytes() {
        let contract = IntentContract::new(minimal_parts());
        let bytes = contract.to_artifact_bytes();
        let decoded = IntentContract::decode(&bytes).expect("decodes");
        assert_eq!(decoded, contract);
        assert_eq!(decoded.identity(), contract.identity());
        assert_eq!(decoded.to_artifact_bytes(), bytes);
    }

    #[test]
    fn the_artifact_form_carries_every_required_key() {
        let contract = IntentContract::new(minimal_parts());
        let json = contract.artifact_json();
        let fields = json.as_object().expect("object");
        for key in TOP_LEVEL_KEYS {
            // `name` is the only key a minimal contract omits.
            if key == "name" {
                assert!(!fields.contains_key(key));
            } else {
                assert!(fields.contains_key(key), "{key} is absent");
            }
        }
    }

    #[test]
    fn the_preimage_excludes_exactly_id2s_list() {
        let mut parts = minimal_parts();
        parts.name = Some("a display name".to_owned());
        let named = IntentContract::new(parts);
        let anonymous = IntentContract::new(minimal_parts());
        // ID2: `name` is metadata and MUST NOT affect identity.
        assert_eq!(named.identity(), anonymous.identity());
        assert_ne!(named.to_artifact_bytes(), anonymous.to_artifact_bytes());

        // ID2: `intent_id` is a self-reference and is out of its own preimage.
        let mut renamed = minimal_parts();
        renamed.intent_id = IntentId::new("in_something_else").expect("handle");
        assert_eq!(
            IntentContract::new(renamed).identity(),
            anonymous.identity()
        );

        let preimage = anonymous.identity_preimage_json();
        let fields = preimage.as_object().expect("object");
        assert!(!fields.contains_key("name"));
        assert!(!fields.contains_key("intent_id"));
        assert_eq!(fields.len(), 18);
    }

    #[test]
    fn an_empty_policy_reviewers_map_is_encoded_explicitly() {
        let contract = IntentContract::new(minimal_parts());
        let encoded = String::from_utf8(contract.to_artifact_bytes()).expect("utf-8");
        assert!(encoded.contains(r#""policy_reviewers":{}"#), "{encoded}");
        // Absent on the wire reads back as the empty map, and re-encodes explicitly,
        // so the two spellings reach one identity.
        let without = encoded.replace(r#""policy_reviewers":{},"#, "");
        let decoded = IntentContract::decode(without.as_bytes()).expect("decodes");
        assert_eq!(decoded.identity(), contract.identity());
        assert_eq!(decoded.to_artifact_bytes(), contract.to_artifact_bytes());
    }

    #[test]
    fn scope_fragments_encode_in_code_point_order_not_declaration_order() {
        let mut parts = minimal_parts();
        parts.scope = Scope::new(
            [],
            [Fragment::Temporal, Fragment::Finite, Fragment::Runtime],
            None,
        )
        .expect("scope");
        let encoded =
            String::from_utf8(IntentContract::new(parts).to_artifact_bytes()).expect("utf-8");
        // Declaration order would be Finite, Temporal, Runtime.
        assert!(
            encoded.contains(r#""fragments":["Finite","Runtime","Temporal"]"#),
            "{encoded}"
        );
    }

    #[test]
    fn an_unknown_top_level_key_is_refused() {
        let contract = IntentContract::new(minimal_parts());
        let encoded = String::from_utf8(contract.to_artifact_bytes()).expect("utf-8");
        let injected = encoded.replacen('{', r#"{"aardvark":1,"#, 1);
        assert_eq!(
            IntentContract::decode(injected.as_bytes()),
            Err(ContractDecodeError::UnknownField {
                field: "intent-contract",
                key: "aardvark".to_owned(),
            })
        );
    }

    #[test]
    fn a_header_naming_another_schema_is_refused() {
        let contract = IntentContract::new(minimal_parts());
        let encoded = String::from_utf8(contract.to_artifact_bytes()).expect("utf-8");
        let other = encoded.replace(SCHEMA_ID, "https://continuum.dev/schema/other.json");
        assert!(matches!(
            IntentContract::decode(other.as_bytes()),
            Err(ContractDecodeError::HeaderMismatch {
                field: "schema_id",
                ..
            })
        ));
    }

    #[test]
    fn w9_fails_closed_without_a_minted_handle_and_passes_with_the_right_one() {
        let contract = IntentContract::new(minimal_parts());
        let blind = contract.check(&CheckEnvironment::new());
        assert!(
            blind
                .findings()
                .contains(&ContractFinding::DeclaredIdentityUnverified)
        );

        let minted = contract.mint_intent_id::<Fnv1aPlaceholder>();
        let wrong = contract.check(&CheckEnvironment::new().with_minted_intent_id(minted.clone()));
        assert!(matches!(
            wrong.findings_for(WellFormednessRule::W9).next(),
            Some(ContractFinding::DeclaredIdentityMismatch { .. })
        ));

        let mut parts = minimal_parts();
        parts.intent_id = IntentId::new(&minted).expect("a minted handle is a handle");
        let agreeing = IntentContract::new(parts);
        let verdict = agreeing.check(
            &CheckEnvironment::new()
                .with_minted_intent_id(agreeing.mint_intent_id::<Fnv1aPlaceholder>()),
        );
        assert!(verdict.is_well_formed(), "{verdict}");
    }

    #[test]
    fn w2_reports_a_dangling_observer_reference() {
        let mut parts = minimal_parts();
        parts.claims = ClaimSet::from_claims([Claim::new(
            UnitKey::new("C1").expect("unit"),
            ClaimKind::Safety,
            claim("C1").expression().clone(),
            Some("client".to_owned()),
        )])
        .expect("one claim");
        let contract = IntentContract::new(parts);
        let verdict = contract.check(&CheckEnvironment::new());
        assert!(
            verdict
                .findings()
                .contains(&ContractFinding::DanglingObserver {
                    unit: "C1".to_owned(),
                    observer: "client".to_owned(),
                })
        );

        // Declaring the observer discharges the rule.
        let mut parts = minimal_parts();
        parts.claims = contract.claims().clone();
        parts.observers = ObserverSet::from_observers([Observer::new(
            ObserverId::new("client").expect("id"),
            [(ProjectionKind::Events, vec!["ReplyPublished".to_owned()])],
        )
        .expect("observer")])
        .expect("one observer");
        let verdict = IntentContract::new(parts).check(&CheckEnvironment::new());
        assert!(
            verdict
                .findings_for(WellFormednessRule::W2)
                .next()
                .is_none()
        );
    }

    #[test]
    fn w3_reports_an_undeclared_fragment_naming_its_group() {
        let mut parts = minimal_parts();
        parts.scope = Scope::new([], [Fragment::Symbolic], None).expect("scope");
        let verdict = IntentContract::new(parts).check(&CheckEnvironment::new());
        assert!(
            verdict
                .findings()
                .contains(&ContractFinding::UndeclaredFragment {
                    group: "claims",
                    unit: "C1".to_owned(),
                    fragment: Fragment::Finite,
                })
        );
    }

    #[test]
    fn w7_reports_a_review_verb_that_names_no_reviewer() {
        let mut parts = minimal_parts();
        parts.policy = PolicyTable::new(
            PolicyField::ALL
                .into_iter()
                .map(|field| (field, PolicyVerb::Review)),
        )
        .expect("review is admissible on every field");
        let verdict = IntentContract::new(parts).check(&CheckEnvironment::new());
        assert!(matches!(
            verdict.findings_for(WellFormednessRule::W7).next(),
            Some(ContractFinding::UnenforceableReview { .. })
        ));
    }

    #[test]
    fn w8_reports_both_halves_and_resolves_with_the_environment() {
        let mut parts = minimal_parts();
        parts.fault_model = FaultModel::new(
            [FaultClass::Crash],
            [ProfileName::new("storage-posix-v1").expect("profile")],
        )
        .expect("faults");
        parts.abstraction_maps = AbstractionMaps::new([(
            MapRole::new("register value abstraction").expect("role"),
            MapId::new("map_register_abs_v1").expect("map id"),
        )])
        .expect("one binding");
        let contract = IntentContract::new(parts);

        let blind = contract.check(&CheckEnvironment::new());
        assert_eq!(blind.findings_for(WellFormednessRule::W8).count(), 2);

        let supplied = contract.check(
            &CheckEnvironment::new()
                .with_domain_pack_profiles(["storage-posix-v1".to_owned()])
                .with_correspondence_maps(["map_register_abs_v1".to_owned()]),
        );
        assert_eq!(supplied.findings_for(WellFormednessRule::W8).count(), 0);
    }

    #[test]
    fn the_handle_patterns_are_the_schemas() {
        assert!(IntentId::new("in_ack_v1").is_ok());
        assert!(IntentId::new("in_").is_err());
        assert!(IntentId::new("ws_ack_v1").is_err());
        assert!(IntentId::new("in_bad char").is_err());

        assert!(MapId::new("map_register_abs_v1").is_ok());
        assert!(MapId::new("m_A").is_ok());
        assert!(MapId::new("Map_x").is_err());
        assert!(MapId::new("map").is_err());
        assert!(MapId::new("map_").is_err());
        // The prefix class contains `_`, so a later separator can be the match.
        assert!(MapId::new("a_b_C-1").is_ok());
    }

    #[test]
    fn w1_refuses_a_repeated_site_or_role() {
        let site = ChoiceSite::new("delivery_order").expect("site");
        assert!(matches!(
            NondeterminismSet::new([
                (site.clone(), ChoiceClass::Scheduler),
                (site, ChoiceClass::Demonic),
            ]),
            Err(ContractError::DuplicateUnitKey { .. })
        ));
        let role = MapRole::new("r").expect("role");
        assert!(matches!(
            AbstractionMaps::new([
                (role.clone(), MapId::new("map_a").expect("map")),
                (role, MapId::new("map_b").expect("map")),
            ]),
            Err(ContractError::DuplicateUnitKey { .. })
        ));
    }

    #[test]
    fn scope_distinguishes_a_null_abstraction_level_from_an_empty_string() {
        let mut parts = minimal_parts();
        parts.scope = Scope::new([], [Fragment::Finite], None).expect("scope");
        let null = IntentContract::new(parts);
        let mut parts = minimal_parts();
        parts.scope = Scope::new([], [Fragment::Finite], Some(String::new())).expect("scope");
        let empty = IntentContract::new(parts);
        assert_ne!(null.identity(), empty.identity());
    }
}
