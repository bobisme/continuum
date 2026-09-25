//! The wire `intent_changes[]`/`diff_*` artifact assembler: RFC 0031's one diff
//! artifact shape, assembled across all fifteen protected fields (PR-12,
//! `bn-7ek41`).
//!
//! # What this module answers
//!
//! Every field classifier in this crate ends its module doc at the same fence:
//! the wire `intent_changes[]` shape, the whole-artifact assembly, the P1
//! completeness a verdict needs, `unsupported` (which needs `scope.fragments`),
//! and the `requested_assurance` clamp all "belong to whatever later bone
//! assembles a full `diff_*` artifact across all fifteen fields". This is that
//! bone. [`assemble`] takes a revision pair plus the artifact's identities and
//! produces the one artifact shape every `DiffHandle` site carries:
//!
//! > Every `diff_*` artifact, from every one of those sites, MUST validate
//! > against `semantic-diff.schema.json`. There is one diff artifact shape; the
//! > intent diff travels as the `intent_changes` set inside it (RFC 0032).
//! >
//! > — `notes/plan/rfcs/0031-semantic-and-intent-diff.md`, "Wire surface and
//! > diff layers"
//!
//! The schema decides the shape (`notes/plan/schemas/semantic-diff.schema.json`,
//! the RFC's own normative-schema rule): `schema_id`/`schema_epoch`, the five
//! identities, `classifier` (F3, paid in the schema), `requested_assurance` (F2,
//! paid), `intent_changes[]` with the per-unit `unit` locator (F1, paid),
//! `semantic_changes[]`, `impact`, and `policy`.
//!
//! # No second classification authority
//!
//! The assembler *consumes* the family's classifications; it never re-derives
//! one. Every relation on the wire comes from exactly one of:
//!
//! - the whole-contract `unchanged` shortcut ([`crate::equality`]);
//! - the seven landed field classifiers ([`crate::properties`],
//!   [`crate::assumptions`], [`crate::observers`], [`crate::bounds`],
//!   [`crate::faults`], [`crate::fairness`], [`crate::assurance`] — the last
//!   folded with `AssurancePolicy::evidence_class_changes`, the two halves its
//!   module doc says "a caller assembling a full `assurance` classification"
//!   folds);
//! - the R2/fail-closed closure for the eight fields no classifier exists for
//!   (`abstraction_maps`, `scope`, `trust_boundaries`, `completion_policy`,
//!   `nondeterminism`, `optimization`, `non_vacuity`, `security_policy`): equal
//!   canonical content is `unchanged`, anything else is `unknown` — the two
//!   relations that cannot manufacture a pass, the same closure the DX-02
//!   campaign and the PR-12 exit ran as harness code and this module makes
//!   production. Per-unit grain follows the schema's own locator vocabulary
//!   (`scope` by sub-field; `abstraction_maps` by `role`; `nondeterminism` by
//!   `site`; `optimization`/`non_vacuity` by the string;
//!   `security_policy` by `data_classification`/class/requirement), because the
//!   schema *requires* a `unit` on those fields' records. No affirmative
//!   relation — no `added`, no direction — is ever minted here: membership on an
//!   unclassified field is a claim this module has no authority to make, exactly
//!   the boundary the exit evidence pinned (`abstraction_maps: unknown`, never
//!   an overclaim);
//! - the two assembler-owned overrides below, both of which only ever *narrow*
//!   an affirmative relation to a non-affirmative one, never the reverse.
//!
//! # The two assembler-owned overrides
//!
//! **`unsupported` (fragment scope).** "A unit whose fragment is not in the
//! after-contract's `scope.fragments` MUST classify `unsupported`" and "removing
//! a fragment from `scope.fragments` MUST force re-classification of every unit
//! stated in that fragment, and those units then classify `unsupported`" (RFC
//! 0031). Only this module holds the after-contract's `scope.fragments`, which
//! is why every family classifier records `unsupported` as out of its reach. The
//! rule is applied to the units that declare a fragment — `claims[].expression`
//! and `assumptions[].expression` (`observers`/`fairness`/`faults` carry no
//! `fragment` member, pinned by their own modules against the live schema): any
//! declared fragment of the unit, on any side the unit exists on, outside the
//! after-contract's declared set forces the record to `unsupported`. The MUST is
//! unconditional, so it reaches `unchanged` pairs too — narrowing scope
//! "converts a blocked change into a fail-closed one", never into an allowed
//! one. An *undeclared* expression fragment denotes the contract's declared
//! fragments (RFC 0037 W3) and is therefore always in scope.
//!
//! **The `requested_assurance` clamp.** RFC 0031's admissibility table:
//! `observed`/`sampled` admit *no* directional evidence for expression
//! relations — "every expression-level direction classifies `unknown`" — while
//! `bounded` admits "exact language inclusion over the bounded universe in
//! `Finite`", which is precisely the evidence class the family's implication
//! oracle produces (`crates/continuum-semantic-diff/src/properties.rs`, "The
//! directions this module affirms are witnessed by universally valid entailment
//! and so are admissible at `bounded` and above"). So: below
//! [`AssuranceLevel::Bounded`], every *expression-derived* direction is reported
//! `unknown`, on the wire and in the verdict input (the two must agree — the
//! schema's fail-closed `allOf` reads the wire relations, and the verdict must
//! be computed from the classification the artifact actually states). The
//! clamped set is exactly the directions that rest on a formula comparison:
//!
//! - `properties`/`assumptions` `strengthened`/`weakened` — always
//!   oracle-derived (their only directional source is the `Finite`-licensed
//!   oracle);
//! - `fairness` same-key `strengthened`/`weakened` where **both** sides declare
//!   a `condition` — the only same-key shape whose direction could rest on a
//!   formula comparison. Today `fairness::formula_relation` holds no fragment
//!   license and that shape classifies `unknown` anyway, so the arm is
//!   unreachable; it is stated now so the recorded fairness-oracle upgrade path
//!   (`bn-8mlg`) cannot silently open a below-`bounded` direction later.
//!
//! Not clamped, at any level: "relations decidable by canonical encoding alone
//! ... do not consume this budget and are admissible at every level" (RFC 0031).
//! That is the table's enumerated nine, and, by the same sentence's own concept
//! — decidability by canonical encoding, with no evidence class involved — the
//! two directional shapes that never consult a formula's language: a fairness
//! `kind` swap (`weak`→`strong` is definitional) and a fairness `condition`
//! declaredness move (`C → null` is RFC 0031's stated bottom of the condition
//! order, decided by presence alone). Both are pinned by this module's tests so
//! the reading is checkable rather than asserted.
//!
//! # Correction 14: the `unchanged` shortcut's wire shape
//!
//! "If `before_intent` and `after_intent` are the same `in_*` identity,
//! `intent_changes` MUST be empty." [`assemble`] takes the shortcut from
//! [`crate::equality`] — canonical preimage bytes, not the handle strings — and
//! emits a literally empty `intent_changes` array, while the verdict is still
//! computed from the fifteen explicit `unchanged` records
//! ([`crate::equality::unchanged_classification`]) that P1 demands: zero records
//! on the wire, fifteen for the verdict, exactly the split `equality`'s module
//! doc promised the assembler would honor. On the non-shortcut path the emission
//! rule is the schema's: "one record per classified unit that is not
//! `unchanged`" — `unchanged` records are omitted (the RFC's MAY, resolved one
//! way so the artifact is canonical), and the verdict input carries what the
//! wire omits.
//!
//! # Policy section
//!
//! The verdict is [`PolicyTable::verdict`] — the crate consumes it, never
//! reimplements it — computed against the **before** contract's own policy table
//! and reviewers (RFC 0037 P7: the registry's current state decides; the same
//! closure the DX-02 campaign and the exit evidence run) on the caller's
//! [`AcceptancePath`]. `policy.reasons` is the P6 set — every record that
//! forbade `allow`, and only those — rendered through
//! `VerdictReason`'s own `Display` and deduplicated into the schema's
//! `uniqueItems` array; the typed facts live in `intent_changes[]` (field,
//! relation, `unit`) and in the verdict's typed reasons, never only in the
//! strings (INV-003).
//!
//! # Determinism (INV-005)
//!
//! "For fixed inputs the diff artifact MUST be byte-identical across platforms
//! and releases within a `schema_version`." Everything is ordered: object keys
//! by the canonical-JSON `BTreeMap`, `intent_changes` by (field, unit,
//! relation), `semantic_changes` by their rendered key, the impact arrays by
//! artifact identity, `policy.reasons` as a sorted set — and every collection
//! the classifiers return is already deterministic in its inputs. The classifier
//! identity the artifact records ([`CLASSIFIER_VERSION`],
//! [`continuum_intent::cpnf::CPNF_VERSION`]) is part of the RFC 0030 query key;
//! a change to either is a new artifact, never an edit (INV-009).
//!
//! # Recorded divergences (not silently resolved)
//!
//! - The schema's `unit` `$comment` counts `assurance` among the fields
//!   "classified as a whole" with "no sub-unit to name", but RFC 0031's
//!   `assurance` section classifies `accepted_evidence_classes` "by membership
//!   only: one `added`/`removed` record per class" — those records *have* a
//!   sub-unit. This module emits the evidence-class token as the record's
//!   `unit` (schema-legal: `unit` is a defined property on every record; the
//!   conditional merely does not require it for `assurance`), because dropping
//!   it would leave the class identity with no typed home — the INV-003 failure
//!   F1 was paid to end. Schema-sweep (SD-08) candidate.
//! - A collapsed fairness `kind` movement is one record for one unit whose key
//!   moved, and the schema's `kind:action` rendering cannot name both kinds in
//!   one token. The record carries the **after**-side key (`strong:A` for a
//!   `weak`→`strong` swap); the before-side key is derivable from the relation
//!   (`strengthened` ⇔ `weak`→`strong`, `weakened` ⇔ `strong`→`weak`,
//!   correction 16's clean-swap scope). Schema-sweep candidate alongside F5's
//!   identical locator-ambiguity note.

use std::collections::BTreeSet;

use continuum_intent::assurance_policy::AssuranceLevel;
use continuum_intent::ast::Fragment;
use continuum_intent::canonical_json::Json;
use continuum_intent::change_policy::{
    AcceptancePath, ChangePolicyError, ClassificationRecord, EnforcementError, PolicyDecision,
    PolicyField, Relation, Verdict,
};
use continuum_intent::contract::IntentContract;
use continuum_intent::contract::IntentId;
use continuum_intent::cpnf::CPNF_VERSION;
use continuum_intent::optimization::ObjectiveRole;

use crate::fairness::FairnessUnit;
use crate::impact::{EvidenceRecord, ImpactError, ImpactSet, ProgramLayer, compute_impact_under};
use crate::{assumptions, assurance, bounds, equality, fairness, faults, observers, properties};

/// The classifier version recorded in the artifact's `classifier` section (RFC
/// 0031 F3, paid by the schema; RFC 0030 query-key identity). It advances
/// whenever a classification could change for any input — including a change to
/// any family classifier or to this assembler's own closure rules.
pub const CLASSIFIER_VERSION: &str = "intent-diff-0.1.0";

/// The artifact-class identity of the governing schema
/// (`notes/plan/schemas/semantic-diff.schema.json`, `schema_id` `const`).
pub const SCHEMA_ID: &str = "https://continuum.dev/schema/semantic-diff.json";

/// The schema epoch this assembler writes (`schema_epoch` `const`).
pub const SCHEMA_EPOCH: i64 = 1;

/// A `diff_*` artifact identity, validated against the schema's
/// `^diff_[A-Za-z0-9_-]+$` pattern. Minting the handle is the daemon's (a digest
/// indexes, it is not the identity — ADR-0013), so this type validates and never
/// derives, exactly as `continuum_intent::contract::IntentId` does for `in_*`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct DiffId(String);

impl DiffId {
    /// Validate and wrap a diff handle.
    ///
    /// # Errors
    ///
    /// [`AssembleError::MalformedDiffId`] when the string is not
    /// `^diff_[A-Za-z0-9_-]+$`.
    pub fn new(handle: &str) -> Result<Self, AssembleError> {
        if handle_suffix_ok(handle, "diff_") {
            Ok(Self(handle.to_owned()))
        } else {
            Err(AssembleError::MalformedDiffId {
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

/// A `ws_*` workspace-snapshot identity, validated against the schema's
/// `^ws_[A-Za-z0-9_-]+$` pattern.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct SnapshotId(String);

impl SnapshotId {
    /// Validate and wrap a snapshot handle.
    ///
    /// # Errors
    ///
    /// [`AssembleError::MalformedSnapshotId`] when the string is not
    /// `^ws_[A-Za-z0-9_-]+$`.
    pub fn new(handle: &str) -> Result<Self, AssembleError> {
        if handle_suffix_ok(handle, "ws_") {
            Ok(Self(handle.to_owned()))
        } else {
            Err(AssembleError::MalformedSnapshotId {
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

fn handle_suffix_ok(handle: &str, prefix: &str) -> bool {
    handle.strip_prefix(prefix).is_some_and(|suffix| {
        !suffix.is_empty()
            && suffix
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
    })
}

/// RFC 0031's closed seven-member program semantic-change axis set — the
/// `semantic_changes[].kind` enum of the normative schema.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum SemanticAxis {
    /// New/removed/moved effect sites.
    Effects,
    /// Split/merged critical regions.
    Atomicity,
    /// Task-ownership movement.
    TaskOwnership,
    /// Checkpoints and obligations.
    CancellationStructure,
    /// Durability-ordering movement.
    DurabilityOrdering,
    /// Observer-publication movement.
    ObserverPublication,
    /// Which links of §16's correspondence graph a change touches. The one axis
    /// with an impact-set mapping of its own (RFC 0031, "Impact set").
    Correspondence,
}

impl SemanticAxis {
    /// Every axis, in the schema's enum order.
    pub const ALL: [Self; 7] = [
        Self::Effects,
        Self::Atomicity,
        Self::TaskOwnership,
        Self::CancellationStructure,
        Self::DurabilityOrdering,
        Self::ObserverPublication,
        Self::Correspondence,
    ];

    /// The wire token.
    #[must_use]
    pub const fn wire(self) -> &'static str {
        match self {
            Self::Effects => "effects",
            Self::Atomicity => "atomicity",
            Self::TaskOwnership => "task_ownership",
            Self::CancellationStructure => "cancellation_structure",
            Self::DurabilityOrdering => "durability_ordering",
            Self::ObserverPublication => "observer_publication",
            Self::Correspondence => "correspondence",
        }
    }
}

/// One program-side semantic-change record (`semantic_changes[]`). The program
/// diff that produces these is not this crate's (the seven axes' analyses belong
/// to the program-diff layers); the assembler carries them because the artifact
/// is one shape, and reads exactly one fact from them: whether the
/// `correspondence` axis was touched, which feeds the impact set. They never
/// touch the intent verdict — "`semantic_changes` MUST NOT justify an `allow`
/// decision on its own, and a program change MUST NOT be used to explain away an
/// `intent_changes` record" (RFC 0031).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct SemanticChange {
    /// The closed axis.
    pub kind: SemanticAxis,
    /// The record's summary (schema-required).
    pub summary: String,
    /// The fragment the change was classified in, when one applies.
    pub fragment: Option<Fragment>,
    /// Source locations (`uniqueItems` on the wire, so a set here).
    pub locations: BTreeSet<String>,
}

impl SemanticChange {
    /// The wire record.
    #[must_use]
    pub fn artifact_json(&self) -> Json {
        let mut fields = vec![
            ("kind".to_owned(), Json::String(self.kind.wire().to_owned())),
            ("summary".to_owned(), Json::String(self.summary.clone())),
        ];
        if let Some(fragment) = self.fragment {
            fields.push((
                "fragment".to_owned(),
                Json::String(fragment.wire().to_owned()),
            ));
        }
        if !self.locations.is_empty() {
            fields.push((
                "locations".to_owned(),
                Json::Array(
                    self.locations
                        .iter()
                        .map(|location| Json::String(location.clone()))
                        .collect(),
                ),
            ));
        }
        Json::object(fields).unwrap_or(Json::Null)
    }
}

/// One wire `intent_changes[]` record: the typed form of the schema's
/// `{evidence, field, fragment?, protected, relation, unit?}` shape.
///
/// `protected` is the schema's `const: true` and is emitted, not stored — an
/// unprotected intent change is unrepresentable. `evidence` is emitted as the
/// explicit `null` of the schema's worked example: no family classifier attaches
/// a witness artifact today (each records that its derivations are recomputable
/// from the two contracts), and an absent witness stated as `null` is a fact,
/// where an omitted key would be silence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IntentChangeRecord {
    field: PolicyField,
    relation: Relation,
    unit: Option<String>,
    fragment: Option<Fragment>,
}

impl IntentChangeRecord {
    /// The classified field.
    #[must_use]
    pub const fn field(&self) -> PolicyField {
        self.field
    }

    /// The relation recorded.
    #[must_use]
    pub const fn relation(&self) -> Relation {
        self.relation
    }

    /// The per-unit locator, for the eleven fields whose schema rule requires
    /// one (plus the `assurance` evidence-class records; module doc, "Recorded
    /// divergences").
    #[must_use]
    pub fn unit(&self) -> Option<&str> {
        self.unit.as_deref()
    }

    /// The fragment the relation was classified in, where one is unambiguous.
    #[must_use]
    pub const fn fragment(&self) -> Option<Fragment> {
        self.fragment
    }

    /// The wire record.
    #[must_use]
    pub fn artifact_json(&self) -> Json {
        let mut fields = vec![
            ("evidence".to_owned(), Json::Null),
            (
                "field".to_owned(),
                Json::String(self.field.wire().to_owned()),
            ),
            ("protected".to_owned(), Json::Bool(true)),
            (
                "relation".to_owned(),
                Json::String(self.relation.wire().to_owned()),
            ),
        ];
        if let Some(fragment) = self.fragment {
            fields.push((
                "fragment".to_owned(),
                Json::String(fragment.wire().to_owned()),
            ));
        }
        if let Some(unit) = &self.unit {
            fields.push(("unit".to_owned(), Json::String(unit.clone())));
        }
        Json::object(fields).unwrap_or(Json::Null)
    }

    /// The deterministic wire order: field (in [`PolicyField::ALL`] order), then
    /// unit, then relation token.
    fn sort_key(&self) -> (usize, Option<String>, &'static str) {
        let index = PolicyField::ALL
            .iter()
            .position(|field| *field == self.field)
            .unwrap_or(PolicyField::ALL.len());
        (index, self.unit.clone(), self.relation.wire())
    }
}

/// Everything [`assemble`] reads: the revision pair, the five wire identities,
/// the classification's own requested assurance, the acceptance path the verdict
/// is computed for, the program-side semantic changes, and the evidence in the
/// impact set's scope.
#[derive(Debug, Clone)]
pub struct DiffRequest<'a> {
    /// The artifact's own identity.
    pub diff_id: DiffId,
    /// The sealed before-snapshot's handle.
    pub before_snapshot: SnapshotId,
    /// The sealed after-snapshot's handle.
    pub after_snapshot: SnapshotId,
    /// The before Intent Contract's registry handle. Resolution against the
    /// registry — and both snapshots' sealedness — are daemon preconditions (RFC
    /// 0031, "Inputs"); the classification itself reads the contracts below.
    pub before_intent: IntentId,
    /// The after Intent Contract's registry handle.
    pub after_intent: IntentId,
    /// The resolved before contract.
    pub before: &'a IntentContract,
    /// The resolved after contract.
    pub after: &'a IntentContract,
    /// The requested assurance for the classification itself — the clamp's input
    /// (module doc, "The `requested_assurance` clamp").
    pub requested_assurance: AssuranceLevel,
    /// Whether a human principal performs the acceptance the verdict is for.
    pub path: AcceptancePath,
    /// Program-side semantic changes, from the program-diff layers.
    pub semantic_changes: Vec<SemanticChange>,
    /// Every evidence artifact that names either intent identity or either
    /// snapshot, with its recorded dependency facts.
    pub evidence: Vec<EvidenceRecord>,
}

/// The assembled artifact: the typed parts, the verdict they closed into, and
/// the canonical wire bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiffArtifact {
    diff_id: DiffId,
    before_snapshot: SnapshotId,
    after_snapshot: SnapshotId,
    before_intent: IntentId,
    after_intent: IntentId,
    requested_assurance: AssuranceLevel,
    intent_changes: Vec<IntentChangeRecord>,
    semantic_changes: Vec<SemanticChange>,
    impact: ImpactSet,
    verdict: Verdict,
    program: ProgramLayer,
}

impl DiffArtifact {
    /// Whether the program-side layer was classified. Under
    /// [`ProgramLayer::Unclassified`] the empty `semantic_changes` section is not
    /// evidence of absence, and no impact entry is `reused`. The schema has no field
    /// for this fact, so it is typed here and not rendered.
    #[must_use]
    pub const fn program_layer(&self) -> ProgramLayer {
        self.program
    }

    /// The wire `intent_changes[]` records, in wire order. Empty exactly when
    /// the `unchanged` shortcut fired or every classified unit was `unchanged`.
    #[must_use]
    pub fn intent_changes(&self) -> &[IntentChangeRecord] {
        &self.intent_changes
    }

    /// The computed impact set, with its typed per-artifact dispositions.
    #[must_use]
    pub const fn impact(&self) -> &ImpactSet {
        &self.impact
    }

    /// The typed verdict the artifact's `policy` section renders.
    #[must_use]
    pub const fn verdict(&self) -> &Verdict {
        &self.verdict
    }

    /// The `policy.decision` — equal, by construction, to what a
    /// `PolicyVerdictValue` carrying this artifact must state (RFC 0031, "Wire
    /// surface": the two "MUST" agree).
    #[must_use]
    pub const fn decision(&self) -> PolicyDecision {
        self.verdict.decision()
    }

    /// The artifact document, schema-shaped.
    #[must_use]
    pub fn artifact_json(&self) -> Json {
        let reasons: BTreeSet<String> = self
            .verdict
            .reasons()
            .iter()
            .map(ToString::to_string)
            .collect();
        let policy = Json::object([
            (
                "decision".to_owned(),
                Json::String(self.decision().wire().to_owned()),
            ),
            (
                "reasons".to_owned(),
                Json::Array(reasons.into_iter().map(Json::String).collect()),
            ),
        ])
        .unwrap_or(Json::Null);
        let classifier = Json::object([
            (
                "classifier_version".to_owned(),
                Json::String(CLASSIFIER_VERSION.to_owned()),
            ),
            (
                "cpnf_version".to_owned(),
                Json::String(CPNF_VERSION.to_owned()),
            ),
        ])
        .unwrap_or(Json::Null);
        Json::object([
            ("schema_id".to_owned(), Json::String(SCHEMA_ID.to_owned())),
            ("schema_epoch".to_owned(), Json::Integer(SCHEMA_EPOCH)),
            (
                "diff_id".to_owned(),
                Json::String(self.diff_id.as_str().to_owned()),
            ),
            (
                "before_snapshot".to_owned(),
                Json::String(self.before_snapshot.as_str().to_owned()),
            ),
            (
                "after_snapshot".to_owned(),
                Json::String(self.after_snapshot.as_str().to_owned()),
            ),
            (
                "before_intent".to_owned(),
                Json::String(self.before_intent.as_str().to_owned()),
            ),
            (
                "after_intent".to_owned(),
                Json::String(self.after_intent.as_str().to_owned()),
            ),
            ("classifier".to_owned(), classifier),
            (
                "requested_assurance".to_owned(),
                Json::String(self.requested_assurance.as_str().to_owned()),
            ),
            (
                "intent_changes".to_owned(),
                Json::Array(
                    self.intent_changes
                        .iter()
                        .map(IntentChangeRecord::artifact_json)
                        .collect(),
                ),
            ),
            (
                "semantic_changes".to_owned(),
                Json::Array(
                    self.semantic_changes
                        .iter()
                        .map(SemanticChange::artifact_json)
                        .collect(),
                ),
            ),
            ("impact".to_owned(), self.impact.artifact_json()),
            ("policy".to_owned(), policy),
        ])
        .unwrap_or(Json::Null)
    }

    /// The canonical artifact bytes (INV-005: byte-identical for fixed inputs).
    #[must_use]
    pub fn to_artifact_bytes(&self) -> Vec<u8> {
        self.artifact_json().to_canonical_bytes()
    }
}

/// Assemble the diff artifact for one revision pair.
///
/// The whole pipeline, in order: the `unchanged` shortcut (correction 14's wire
/// shape); else the fifteen-field classification through the family plus the
/// R2/fail-closed closure; the two assembler-owned overrides (`unsupported`,
/// then the clamp); the P1-complete verdict against the **before** contract's
/// own governance; and the impact set over the changed fields and the
/// `correspondence` axis. See the module doc for each step's authority.
///
/// # Errors
///
/// - [`AssembleError::Classification`] — a relation inadmissible on its field
///   reached record construction (never in practice; every source is checked).
/// - [`AssembleError::Enforcement`] — the verdict was refused: a field with no
///   record (P1), an unenforceable `review` verb (W7), or an inadmissible
///   relation. A refused verdict is a refused artifact; there is no partial
///   emission (RFC 0031: "A classifier that cannot complete MUST NOT emit a
///   partial `intent_changes` set").
/// - [`AssembleError::Impact`] — a malformed or duplicated evidence identity.
pub fn assemble(request: &DiffRequest<'_>) -> Result<DiffArtifact, AssembleError> {
    assemble_under(request, ProgramLayer::Classified)
}

/// [`assemble`] with the program-side layer's completeness stated.
///
/// Under [`ProgramLayer::Unclassified`] the impact set is computed by
/// [`compute_impact_under`], so no in-scope artifact is `reused`. The intent
/// classification and the policy verdict do not change: RFC 0031 keeps program-side
/// changes out of the intent policy.
///
/// # Errors
///
/// As [`assemble`].
pub fn assemble_under(
    request: &DiffRequest<'_>,
    program: ProgramLayer,
) -> Result<DiffArtifact, AssembleError> {
    let (intent_changes, verdict_records) = classify(request)?;

    let verdict = request
        .before
        .policy()
        .verdict(
            &verdict_records,
            request.before.policy_reviewers(),
            request.path,
        )
        .map_err(AssembleError::Enforcement)?;

    let changed_fields: BTreeSet<PolicyField> = intent_changes
        .iter()
        .map(IntentChangeRecord::field)
        .collect();
    let correspondence_changed = request
        .semantic_changes
        .iter()
        .any(|change| change.kind == SemanticAxis::Correspondence);
    let impact = compute_impact_under(
        &changed_fields,
        correspondence_changed,
        program,
        &request.evidence,
    )
    .map_err(AssembleError::Impact)?;

    let mut semantic_changes = request.semantic_changes.clone();
    semantic_changes.sort();

    Ok(DiffArtifact {
        diff_id: request.diff_id.clone(),
        before_snapshot: request.before_snapshot.clone(),
        after_snapshot: request.after_snapshot.clone(),
        before_intent: request.before_intent.clone(),
        after_intent: request.after_intent.clone(),
        requested_assurance: request.requested_assurance,
        intent_changes,
        semantic_changes,
        impact,
        verdict,
        program,
    })
}

/// The fifteen-field classification: the wire records (non-`unchanged`, in wire
/// order) and the P1-complete verdict input.
fn classify(
    request: &DiffRequest<'_>,
) -> Result<(Vec<IntentChangeRecord>, Vec<ClassificationRecord>), AssembleError> {
    // Correction 14: equal canonical identities entail every field unchanged —
    // an empty wire array, fifteen explicit `unchanged` records for the verdict.
    if let Some(records) =
        equality::unchanged_classification(request.before.identity(), request.after.identity())
    {
        return Ok((Vec::new(), records.to_vec()));
    }

    let mut records = classify_family(request);
    records.extend(classify_fail_closed(request.before, request.after));

    // Deduplicate exactly identical records (two `security_policy` units could
    // render one locator — the F5-shaped collision the module doc records; one
    // record per classified unit means one record, not two copies of it).
    records.sort_by(|a, b| a.sort_key().cmp(&b.sort_key()));
    records.dedup();

    let mut verdict_records = Vec::new();
    for record in &records {
        verdict_records.push(
            ClassificationRecord::new(record.field, record.relation)
                .map_err(AssembleError::Classification)?,
        );
    }
    // P1: a field whose every unit classified `unchanged` (and was therefore
    // omitted from the wire) is still explicitly `unchanged` for the verdict.
    let covered: BTreeSet<PolicyField> = records.iter().map(|record| record.field).collect();
    for field in PolicyField::ALL {
        if !covered.contains(&field) {
            verdict_records.push(
                ClassificationRecord::new(field, Relation::Unchanged)
                    .map_err(AssembleError::Classification)?,
            );
        }
    }

    // The wire emission rule: one record per classified unit that is *not*
    // `unchanged`; the `unchanged` records stay in the verdict input the wire
    // omits (module doc, "Correction 14").
    let wire: Vec<IntentChangeRecord> = records
        .into_iter()
        .filter(|record| record.relation != Relation::Unchanged)
        .collect();

    Ok((wire, verdict_records))
}

/// The seven landed classifiers' fields, consumed record for record, with the
/// two assembler-owned overrides applied where each is defined.
fn classify_family(request: &DiffRequest<'_>) -> Vec<IntentChangeRecord> {
    let before = request.before;
    let after = request.after;
    let requested = request.requested_assurance;
    let in_scope = after.scope().declared_fragments();
    let mut records = Vec::new();

    // `properties`: per-unit, with fragment attribution, the `unsupported`
    // override, and the clamp (all three defined for this field).
    for change in properties::classify_properties(before.claims(), after.claims()) {
        let fragments: Vec<Option<Fragment>> = [
            before.claims().get(change.unit()),
            after.claims().get(change.unit()),
        ]
        .into_iter()
        .flatten()
        .map(|claim| claim.expression().fragment())
        .collect();
        records.push(expression_unit_record(
            PolicyField::Properties,
            change.unit().as_str(),
            change.relation(),
            &fragments,
            &in_scope,
            requested,
        ));
    }

    // `assumptions`: identical shape — "the relation is computed on `expression`
    // exactly as for claims".
    for change in assumptions::classify_assumptions(before.assumptions(), after.assumptions()) {
        let fragments: Vec<Option<Fragment>> = [
            before.assumptions().get(change.unit()),
            after.assumptions().get(change.unit()),
        ]
        .into_iter()
        .flatten()
        .map(|assumption| assumption.expression().fragment())
        .collect();
        records.push(expression_unit_record(
            PolicyField::Assumptions,
            change.unit().as_str(),
            change.relation(),
            &fragments,
            &in_scope,
            requested,
        ));
    }

    // `observers`: no fragment member, no expression directions — records pass
    // through untouched.
    for change in observers::classify_observers(before.observers(), after.observers()) {
        records.push(IntentChangeRecord {
            field: PolicyField::Observers,
            relation: change.relation(),
            unit: Some(change.unit().as_str().to_owned()),
            fragment: None,
        });
    }

    // `bounds`: one whole-field record, encoding-decidable at every level.
    records.push(IntentChangeRecord {
        field: PolicyField::Bounds,
        relation: bounds::classify_bounds(before.bounds(), after.bounds()).relation(),
        unit: None,
        fragment: None,
    });

    // `faults`: membership only.
    for change in faults::classify_faults(before.fault_model(), after.fault_model()) {
        records.push(IntentChangeRecord {
            field: PolicyField::Faults,
            relation: change.relation(),
            unit: Some(change.unit().locator().to_owned()),
            fragment: None,
        });
    }

    // `fairness`: the clamp reaches exactly the same-key directions that could
    // rest on a formula comparison (module doc); kind swaps and declaredness
    // moves are decidable by canonical encoding and pass at every level.
    for change in fairness::classify_fairness(before.fairness(), after.fairness()) {
        let (unit, relation) = match change.unit() {
            FairnessUnit::Key(key) => {
                let expression_derived = matches!(
                    change.relation(),
                    Relation::Strengthened | Relation::Weakened
                ) && before
                    .fairness()
                    .get(key)
                    .is_some_and(|constraint| constraint.condition().is_some())
                    && after
                        .fairness()
                        .get(key)
                        .is_some_and(|constraint| constraint.condition().is_some());
                let relation = if expression_derived {
                    clamp(requested, change.relation())
                } else {
                    change.relation()
                };
                (key.locator(), relation)
            }
            FairnessUnit::KindChange { action, after, .. } => {
                // The after-side key names the collapsed swap (module doc,
                // "Recorded divergences"); the swap is definitional, never
                // clamped.
                (format!("{}:{action}", after.wire()), change.relation())
            }
        };
        records.push(IntentChangeRecord {
            field: PolicyField::Fairness,
            relation,
            unit: Some(unit),
            fragment: None,
        });
    }

    // `assurance`: the requirement triple plus the evidence-class memberships —
    // the two halves the module doc says the assembler folds. All of it is
    // decidable by canonical encoding (total order, set membership).
    records.push(IntentChangeRecord {
        field: PolicyField::Assurance,
        relation: assurance::classify_assurance(before.assurance(), after.assurance()).relation(),
        unit: None,
        fragment: None,
    });
    for (class, relation) in before.assurance().evidence_class_changes(after.assurance()) {
        records.push(IntentChangeRecord {
            field: PolicyField::Assurance,
            relation,
            unit: Some(class.as_str().to_owned()),
            fragment: None,
        });
    }

    records
}

/// One `properties`/`assumptions` unit's wire record: fragment attribution, the
/// `unsupported` override, then the clamp — in that order, because
/// `unsupported` is the more specific non-affirmative answer and the clamp only
/// narrows directions the override left standing.
fn expression_unit_record(
    field: PolicyField,
    unit: &str,
    relation: Relation,
    side_fragments: &[Option<Fragment>],
    in_scope: &BTreeSet<Fragment>,
    requested: AssuranceLevel,
) -> IntentChangeRecord {
    let declared: BTreeSet<Fragment> = side_fragments.iter().flatten().copied().collect();
    let out_of_scope = declared.iter().any(|fragment| !in_scope.contains(fragment));
    let relation = if out_of_scope {
        Relation::Unsupported
    } else {
        clamp(requested, relation)
    };
    // Attribution: the fragment the relation was classified in is unambiguous
    // exactly when every side the unit exists on declares the same one.
    let fragment = match declared.iter().copied().collect::<Vec<_>>().as_slice() {
        [only] if side_fragments.iter().all(Option::is_some) => Some(*only),
        _ => None,
    };
    IntentChangeRecord {
        field,
        relation,
        unit: Some(unit.to_owned()),
        fragment,
    }
}

/// The `requested_assurance` clamp: below `bounded`, an expression-derived
/// direction is reported `unknown`, never the direction (RFC 0031's
/// admissibility table; the caller decides *whether* a relation is
/// expression-derived — see the module doc for the closed list).
const fn clamp(requested: AssuranceLevel, relation: Relation) -> Relation {
    match relation {
        Relation::Strengthened | Relation::Weakened => {
            if matches!(
                requested,
                AssuranceLevel::Observed | AssuranceLevel::Sampled
            ) {
                Relation::Unknown
            } else {
                relation
            }
        }
        _ => relation,
    }
}

/// The R2/fail-closed closure for the eight fields no classifier exists for:
/// equal canonical content per unit is `unchanged` (omitted from the wire),
/// anything else is `unknown` — never a minted direction, never silence. Unit
/// grain follows the schema's locator vocabulary; `trust_boundaries` and
/// `completion_policy` are classified as a whole (the schema requires no unit of
/// either).
fn classify_fail_closed(
    before: &IntentContract,
    after: &IntentContract,
) -> Vec<IntentChangeRecord> {
    let mut records = Vec::new();
    let mut unknown = |field: PolicyField, unit: Option<String>| {
        records.push(IntentChangeRecord {
            field,
            relation: Relation::Unknown,
            unit,
            fragment: None,
        });
    };

    // `scope`, by sub-field.
    let components = |contract: &IntentContract| -> BTreeSet<String> {
        contract
            .scope()
            .components()
            .map(ToOwned::to_owned)
            .collect()
    };
    if components(before) != components(after) {
        unknown(PolicyField::Scope, Some("components".to_owned()));
    }
    if before.scope().declared_fragments() != after.scope().declared_fragments() {
        unknown(PolicyField::Scope, Some("fragments".to_owned()));
    }
    if before.scope().abstraction_level() != after.scope().abstraction_level() {
        unknown(PolicyField::Scope, Some("abstraction_level".to_owned()));
    }

    // `trust_boundaries`, as a whole (the classified unit is the pair).
    if before.trust_boundaries().artifact_json() != after.trust_boundaries().artifact_json() {
        unknown(PolicyField::TrustBoundaries, None);
    }

    // `completion_policy`, the scalar.
    if before.completion_policy() != after.completion_policy() {
        unknown(PolicyField::CompletionPolicy, None);
    }

    // `abstraction_maps`, by `role`.
    let mut roles: BTreeSet<&str> = before
        .abstraction_maps()
        .iter()
        .map(|(role, _)| role.as_str())
        .collect();
    roles.extend(
        after
            .abstraction_maps()
            .iter()
            .map(|(role, _)| role.as_str()),
    );
    for role in roles {
        let binding = |contract: &IntentContract| -> Option<String> {
            contract
                .abstraction_maps()
                .iter()
                .find(|(candidate, _)| candidate.as_str() == role)
                .map(|(_, map)| map.as_str().to_owned())
        };
        if binding(before) != binding(after) {
            unknown(PolicyField::AbstractionMaps, Some(role.to_owned()));
        }
    }

    // `nondeterminism`, by `site`.
    let mut sites: BTreeSet<&str> = before
        .nondeterminism()
        .iter()
        .map(|(site, _)| site.as_str())
        .collect();
    sites.extend(after.nondeterminism().iter().map(|(site, _)| site.as_str()));
    for site in sites {
        let class = |contract: &IntentContract| -> Option<&'static str> {
            contract
                .nondeterminism()
                .iter()
                .find(|(candidate, _)| candidate.as_str() == site)
                .map(|(_, class)| class.wire())
        };
        if class(before) != class(after) {
            unknown(PolicyField::Nondeterminism, Some(site.to_owned()));
        }
    }

    // `optimization` (`hard` and `soft`) and `non_vacuity`, by member string —
    // split into their two fields, as correction 12 requires.
    for role in [ObjectiveRole::Hard, ObjectiveRole::Soft] {
        let members = |contract: &IntentContract| -> BTreeSet<String> {
            contract
                .optimization()
                .objectives(role)
                .map(|objective| objective.as_str().to_owned())
                .collect()
        };
        for member in members(before).symmetric_difference(&members(after)) {
            unknown(PolicyField::Optimization, Some(member.clone()));
        }
    }
    let obligations = |contract: &IntentContract| -> BTreeSet<String> {
        contract
            .optimization()
            .non_vacuity()
            .map(|obligation| obligation.as_str().to_owned())
            .collect()
    };
    for member in obligations(before).symmetric_difference(&obligations(after)) {
        unknown(PolicyField::NonVacuity, Some(member.clone()));
    }

    // `security_policy`, by component.
    if before.security_policy().data_classification()
        != after.security_policy().data_classification()
    {
        unknown(
            PolicyField::SecurityPolicy,
            Some("data_classification".to_owned()),
        );
    }
    let string_set = |members: Box<dyn Iterator<Item = &str> + '_>| -> BTreeSet<String> {
        members.map(ToOwned::to_owned).collect()
    };
    let redactions = |contract: &IntentContract| {
        string_set(Box::new(contract.security_policy().redaction_classes()))
    };
    for member in redactions(before).symmetric_difference(&redactions(after)) {
        unknown(PolicyField::SecurityPolicy, Some(member.clone()));
    }
    let capabilities = |contract: &IntentContract| {
        string_set(Box::new(
            contract.security_policy().capability_requirements(),
        ))
    };
    for member in capabilities(before).symmetric_difference(&capabilities(after)) {
        unknown(PolicyField::SecurityPolicy, Some(member.clone()));
    }

    records
}

/// Why an artifact could not be assembled. A refused artifact is the whole
/// answer — there is no partial emission.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AssembleError {
    /// The diff handle does not match `^diff_[A-Za-z0-9_-]+$`.
    MalformedDiffId {
        /// The rejected string.
        value: String,
    },
    /// A snapshot handle does not match `^ws_[A-Za-z0-9_-]+$`.
    MalformedSnapshotId {
        /// The rejected string.
        value: String,
    },
    /// A relation inadmissible on its field reached record construction.
    Classification(ChangePolicyError),
    /// The verdict was refused (P1 incompleteness, W7 unenforceable review, or
    /// an inadmissible relation).
    Enforcement(EnforcementError),
    /// The impact scope was malformed (a bad or duplicated evidence identity).
    Impact(ImpactError),
}

impl core::fmt::Display for AssembleError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::MalformedDiffId { value } => {
                write!(
                    f,
                    "diff handle {value:?} does not match ^diff_[A-Za-z0-9_-]+$"
                )
            }
            Self::MalformedSnapshotId { value } => {
                write!(
                    f,
                    "snapshot handle {value:?} does not match ^ws_[A-Za-z0-9_-]+$"
                )
            }
            Self::Classification(error) => write!(f, "classification record refused: {error}"),
            Self::Enforcement(error) => write!(f, "policy verdict refused: {error}"),
            Self::Impact(error) => write!(f, "impact set refused: {error}"),
        }
    }
}

impl std::error::Error for AssembleError {}

#[cfg(test)]
mod tests {
    use continuum_intent::contract::IntentContract;

    use super::*;

    const FIXTURE: &str =
        include_str!("../../continuum-intent/tests/fixtures/replicated-register-contract.json");

    fn decode(document: &str) -> IntentContract {
        IntentContract::decode(document.trim_end().as_bytes()).expect("the document decodes")
    }

    fn replace_once(document: &str, needle: &str, replacement: &str) -> String {
        assert!(
            document.contains(needle),
            "the fixture carries the text this mutation edits"
        );
        document.replacen(needle, replacement, 1)
    }

    fn request<'a>(
        before: &'a IntentContract,
        after: &'a IntentContract,
        requested: AssuranceLevel,
    ) -> DiffRequest<'a> {
        DiffRequest {
            diff_id: DiffId::new("diff_test1").expect("well formed"),
            before_snapshot: SnapshotId::new("ws_before1").expect("well formed"),
            after_snapshot: SnapshotId::new("ws_after1").expect("well formed"),
            before_intent: IntentId::new("in_before1").expect("well formed"),
            after_intent: IntentId::new("in_after1").expect("well formed"),
            before,
            after,
            requested_assurance: requested,
            path: AcceptancePath::AgentAccept,
            semantic_changes: Vec::new(),
            evidence: Vec::new(),
        }
    }

    fn bounded() -> AssuranceLevel {
        continuum_intent::assurance_policy::level_from_wire("bounded").expect("a level")
    }

    #[test]
    fn handles_are_validated_against_the_schema_patterns() {
        assert!(DiffId::new("diff_a-B_1").is_ok());
        for bad in ["diff_", "dif_x", "diff x", "", "diff_é"] {
            assert!(DiffId::new(bad).is_err(), "{bad} rejected");
        }
        assert!(SnapshotId::new("ws_demo1").is_ok());
        assert!(SnapshotId::new("wsdemo1").is_err());
    }

    #[test]
    fn equal_identities_take_the_shortcut_to_an_empty_wire_array() {
        let before = decode(FIXTURE);
        let after = decode(FIXTURE);
        let artifact =
            assemble(&request(&before, &after, bounded())).expect("the artifact assembles");
        assert!(artifact.intent_changes().is_empty());
        assert_eq!(artifact.decision(), PolicyDecision::Allow);
        assert!(artifact.verdict().reasons().is_empty());
    }

    #[test]
    fn a_changed_unit_reaches_the_wire_with_its_locator_and_the_unchanged_rest_does_not() {
        let before = decode(FIXTURE);
        // The plan §5.1 bounds attack: reduce node count.
        let after = decode(&replace_once(FIXTURE, "\"nodes\":3", "\"nodes\":2"));
        let artifact =
            assemble(&request(&before, &after, bounded())).expect("the artifact assembles");
        let records = artifact.intent_changes();
        assert_eq!(records.len(), 1, "one classified unit changed");
        assert_eq!(records[0].field(), PolicyField::Bounds);
        assert_eq!(records[0].relation(), Relation::Contracted);
        assert_eq!(records[0].unit(), None, "bounds is classified as a whole");
        assert_eq!(artifact.decision(), PolicyDecision::Block);
    }

    #[test]
    fn the_fail_closed_closure_names_the_changed_unit_and_never_a_direction() {
        let before = decode(FIXTURE);
        let after = decode(&replace_once(
            FIXTURE,
            "\"map_runtime_to_abstract_v1\"",
            "\"map_runtime_to_abstract_v2\"",
        ));
        let artifact =
            assemble(&request(&before, &after, bounded())).expect("the artifact assembles");
        let records = artifact.intent_changes();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].field(), PolicyField::AbstractionMaps);
        assert_eq!(records[0].relation(), Relation::Unknown);
        assert_eq!(records[0].unit(), Some("runtime to abstract register"));
        assert_eq!(artifact.decision(), PolicyDecision::Review);
    }

    #[test]
    fn assembly_is_byte_deterministic() {
        let before = decode(FIXTURE);
        let after = decode(&replace_once(FIXTURE, "\"nodes\":3", "\"nodes\":2"));
        let first = assemble(&request(&before, &after, bounded())).expect("assembles");
        let second = assemble(&request(&before, &after, bounded())).expect("assembles");
        assert_eq!(first.to_artifact_bytes(), second.to_artifact_bytes());
    }
}
