//! The twenty node kinds, and one node's immutable versions (plan §11.2, PR 7).
//!
//! # The vocabulary is closed at twenty
//!
//! plan §11.2 lists twenty node types and the node schema's `kind` enum lists the same
//! twenty as wire tokens, with one rename recorded in the schema itself:
//!
//! > Node kinds mirror plan §11.2's 20 node types; `candidate` corresponds to §11.2
//! > `SynthesisCandidate` (see `synthesis-candidate.schema.json`).
//! >
//! > — `evidence-graph-node.schema.json`
//!
//! Four of the twenty are spelled differently in the two documents —
//! `model`/`ModelVersion`, `program`/`ProgramSnapshot`, `run`/`VerificationRun`,
//! `receipt`/`ProofReceipt`, plus `explanation`/`CausalExplanation` and
//! `decision`/`ReviewDecision`. [`NodeKind::plan_name`] carries the plan's spelling beside
//! [`NodeKind::as_str`]'s wire token so a reader of either document finds the same variant,
//! and `every_kind_names_its_plan_11_2_type` holds the correspondence to twenty pairs. The
//! declaration order is the **schema's**, because RFC 0038 says the schemas decide.
//!
//! # A node lands at the bottom of the lattice, and there is no other constructor
//!
//! > | (creation) → proposed | any authorized human/agent — there is no `draft` status |
//! >
//! > — docs/44, "Status authority"
//!
//! [`EvidenceNode::propose`] is the only constructor and it takes no status: a new node's
//! history is one [`StatusWrite`] at [`ClaimStatus::BOTTOM`]. That is INV-004's producer
//! half stated as a type rather than as a check — the same device `observe.ingest` gets from
//! the wire's shape, where "there is no status field, no confidence field, and no
//! service-identity field, so the strongest thing a producer can say about its own claim is
//! which bytes it is about". A [`StatusWrite`] above the bottom is minted only by
//! [`crate::authority::Promotion`], whose fields are private to that module.
//!
//! # Immutable versions
//!
//! > Typed immutable candidates […] The graph is append-only; nothing is edited in place.
//! >
//! > — RFC 0038, "Nodes/edges" and "Write and concurrency model"
//!
//! A node's *identity* is a function of what it says — kind, artifact, claim, labels and
//! provenance — and deliberately **not** of its status history, because a promotion that
//! changed a node's identity would break the two sentences the write model rests on: status
//! promotion is "a compare-and-set against the claim's current status" (there must be one
//! claim to compare against) and "a replayed write returns the original node identity".
//!
//! So the history is versioned state hanging off a fixed identity. Every status write
//! appends; nothing is overwritten; [`EvidenceNode::version`] counts the writes and
//! [`EvidenceNode::history`] replays them in order. A caller holding an older
//! [`EvidenceNode`] value still holds a valid record of that version — the type has no
//! interior mutability and no setter, and promotion produces a *new* value.
//!
//! `idempotency_key` is likewise outside the identity. It is the mechanism that makes a
//! retry safe (plan §11.7, RFC 0026), not part of what the node says, and two writers who
//! independently produce the identical node under two keys have produced one node — which is
//! exactly the convergence the append-only store implements as put-if-absent.
//!
//! # Two things a caller cannot spell
//!
//! Both are `compile_fail` doc tests rather than assertions, because an assertion would be
//! evidence that a check runs and what is claimed here is that there is nothing to check.
//! A doc test compiles as an external crate, so these are the view a client, an adapter or
//! the daemon has of this API.
//!
//! A producer cannot name a status — [`EvidenceNode::propose`] has no status parameter:
//!
//! ```compile_fail
//! use continuum_evidence::actor::ActorId;
//! use continuum_evidence::claim_status::ClaimStatus;
//! use continuum_evidence::node::{ClaimId, EvidenceNode, IdempotencyKey, NodeKind};
//! use continuum_evidence::provenance::{ArtifactRef, Provenance, Timestamp};
//!
//! let provenance = Provenance::new(
//!     ActorId::new("agent:swarm-1").unwrap(),
//!     Timestamp::new("2026-08-01T12:00:00.000Z").unwrap(),
//!     [],
//! );
//! let _ = EvidenceNode::propose(
//!     NodeKind::Run,
//!     ArtifactRef::new("trace_9f").unwrap(),
//!     ClaimId::new("claim-1").unwrap(),
//!     IdempotencyKey::new("key-1").unwrap(),
//!     provenance,
//!     ClaimStatus::Validated,
//! );
//! ```
//!
//! …and a node cannot be built without provenance — it is a by-value parameter, not an
//! option:
//!
//! ```compile_fail
//! use continuum_evidence::node::{ClaimId, EvidenceNode, IdempotencyKey, NodeKind};
//! use continuum_evidence::provenance::ArtifactRef;
//!
//! let _ = EvidenceNode::propose(
//!     NodeKind::Run,
//!     ArtifactRef::new("trace_9f").unwrap(),
//!     ClaimId::new("claim-1").unwrap(),
//!     IdempotencyKey::new("key-1").unwrap(),
//! );
//! ```
//!
//! The same construction, with every argument supplied, compiles and lands at the bottom of
//! the lattice:
//!
//! ```
//! use continuum_evidence::actor::ActorId;
//! use continuum_evidence::claim_status::ClaimStatus;
//! use continuum_evidence::node::{ClaimId, EvidenceNode, IdempotencyKey, NodeKind};
//! use continuum_evidence::provenance::{ArtifactRef, Provenance, Timestamp};
//!
//! let provenance = Provenance::new(
//!     ActorId::new("agent:swarm-1").unwrap(),
//!     Timestamp::new("2026-08-01T12:00:00.000Z").unwrap(),
//!     [],
//! );
//! let node = EvidenceNode::propose(
//!     NodeKind::Run,
//!     ArtifactRef::new("trace_9f").unwrap(),
//!     ClaimId::new("claim-1").unwrap(),
//!     IdempotencyKey::new("key-1").unwrap(),
//!     provenance,
//! );
//! assert_eq!(node.status(), ClaimStatus::Proposed);
//! ```
//!
//! # Clause → test
//!
//! | Clause | Source | Test |
//! |---|---|---|
//! | twenty kinds, the schema's tokens | plan §11.2, node schema | `the_kind_set_is_exactly_the_schema_enum` |
//! | `candidate` is §11.2 `SynthesisCandidate` | node schema | `every_kind_names_its_plan_11_2_type` |
//! | creation lands at `proposed` | docs/44 | `a_new_node_is_proposed_and_nothing_else` |
//! | no constructor takes a status | INV-004 | module doc, first `compile_fail` |
//! | provenance is structural | PR-7 / IMPL-02 | module doc, second `compile_fail` |
//! | identity excludes status and key | plan §11.7 | `identity_is_fixed_across_versions`, `the_idempotency_key_is_outside_the_identity`, `what_a_node_says_is_in_its_identity` |
//! | append-only versions | RFC 0038 | `a_status_write_appends_and_never_replaces` |
//! | the record is the schema's members | node schema | `the_record_is_the_schema_object`, `the_conditional_members_appear_exactly_where_the_schema_says` |

use core::fmt;
use std::collections::BTreeSet;

use continuum_value::assurance::{InconclusiveReason, ValidationBasis};
use continuum_value::value::Value;

use crate::actor::field;
use crate::claim_status::ClaimStatus;
use crate::identity::EvidenceIdentity;
use crate::provenance::{ArtifactRef, Provenance};

/// One of plan §11.2's twenty node types.
///
/// Declared in the node schema's enum order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum NodeKind {
    /// A claim taken from the Intent Contract (plan §11.2 `IntentClaim`).
    IntentClaim,
    /// An assumption a claim is conditional on (`Assumption`).
    Assumption,
    /// A property to be established (`Property`).
    Property,
    /// A version of a model (`ModelVersion`).
    Model,
    /// A snapshot of a program (`ProgramSnapshot`).
    Program,
    /// A goal handed to a proof worker (`ProofGoal`).
    ProofGoal,
    /// A synthesis candidate (`SynthesisCandidate`; the schema renames it `candidate`).
    Candidate,
    /// A candidate invariant (`InvariantCandidate`).
    InvariantCandidate,
    /// A candidate ranking function (`RankingCandidate`).
    RankingCandidate,
    /// A map between two levels of abstraction (`AbstractionMap`).
    AbstractionMap,
    /// A counterexample (`Counterexample`).
    Counterexample,
    /// A causal explanation (`CausalExplanation`).
    Explanation,
    /// A hypothesis about how to repair a failure (`RepairHypothesis`).
    RepairHypothesis,
    /// A patch (`Patch`).
    Patch,
    /// A verification run (`VerificationRun`).
    Run,
    /// A certificate (`Certificate`).
    Certificate,
    /// A proof receipt (`ProofReceipt`).
    Receipt,
    /// A benchmark result (`BenchmarkResult`).
    BenchmarkResult,
    /// Two or more claims in tension (`Conflict`). See [`crate::conflict`].
    Conflict,
    /// A recorded review decision (`ReviewDecision`).
    Decision,
}

impl NodeKind {
    /// Every kind, in the node schema's enum order.
    pub const ALL: [Self; 20] = [
        Self::IntentClaim,
        Self::Assumption,
        Self::Property,
        Self::Model,
        Self::Program,
        Self::ProofGoal,
        Self::Candidate,
        Self::InvariantCandidate,
        Self::RankingCandidate,
        Self::AbstractionMap,
        Self::Counterexample,
        Self::Explanation,
        Self::RepairHypothesis,
        Self::Patch,
        Self::Run,
        Self::Certificate,
        Self::Receipt,
        Self::BenchmarkResult,
        Self::Conflict,
        Self::Decision,
    ];

    /// The stable wire token, byte-identical to the node schema's `kind` enum.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::IntentClaim => "intent_claim",
            Self::Assumption => "assumption",
            Self::Property => "property",
            Self::Model => "model",
            Self::Program => "program",
            Self::ProofGoal => "proof_goal",
            Self::Candidate => "candidate",
            Self::InvariantCandidate => "invariant_candidate",
            Self::RankingCandidate => "ranking_candidate",
            Self::AbstractionMap => "abstraction_map",
            Self::Counterexample => "counterexample",
            Self::Explanation => "explanation",
            Self::RepairHypothesis => "repair_hypothesis",
            Self::Patch => "patch",
            Self::Run => "run",
            Self::Certificate => "certificate",
            Self::Receipt => "receipt",
            Self::BenchmarkResult => "benchmark_result",
            Self::Conflict => "conflict",
            Self::Decision => "decision",
        }
    }

    /// The plan §11.2 spelling of the same node type.
    ///
    /// Six of the twenty differ from the wire token; the schema records the reason for one
    /// of them and the rest are the same rename in the other direction. Carrying both means
    /// a reader of §11.2 and a reader of the schema land on one variant.
    #[must_use]
    pub const fn plan_name(self) -> &'static str {
        match self {
            Self::IntentClaim => "IntentClaim",
            Self::Assumption => "Assumption",
            Self::Property => "Property",
            Self::Model => "ModelVersion",
            Self::Program => "ProgramSnapshot",
            Self::ProofGoal => "ProofGoal",
            Self::Candidate => "SynthesisCandidate",
            Self::InvariantCandidate => "InvariantCandidate",
            Self::RankingCandidate => "RankingCandidate",
            Self::AbstractionMap => "AbstractionMap",
            Self::Counterexample => "Counterexample",
            Self::Explanation => "CausalExplanation",
            Self::RepairHypothesis => "RepairHypothesis",
            Self::Patch => "Patch",
            Self::Run => "VerificationRun",
            Self::Certificate => "Certificate",
            Self::Receipt => "ProofReceipt",
            Self::BenchmarkResult => "BenchmarkResult",
            Self::Conflict => "Conflict",
            Self::Decision => "ReviewDecision",
        }
    }

    /// Parse a wire token.
    ///
    /// Returns [`None`] for anything outside the twenty, including a plan §11.2 spelling:
    /// the wire vocabulary is closed and this is not a place to be lenient (GOV-1-12).
    #[must_use]
    pub fn from_token(token: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|kind| kind.as_str() == token)
    }
}

impl fmt::Display for NodeKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// The claim identity a node's status promotion is linearized against (plan §11.7).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ClaimId(String);

impl ClaimId {
    /// Name a claim.
    ///
    /// # Errors
    ///
    /// [`EmptyToken`] for the empty string: a claim identity that names nothing cannot be
    /// the thing a compare-and-set linearizes against.
    pub fn new(text: &str) -> Result<Self, EmptyToken> {
        if text.is_empty() {
            return Err(EmptyToken::ClaimId);
        }
        Ok(Self(text.to_owned()))
    }

    /// The claim identity.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ClaimId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// The idempotency key that makes an agent retry safe (plan §11.7, RFC 0026).
///
/// > `"idempotency_key": { "minLength": 1 }`
/// >
/// > — `evidence-graph-node.schema.json`
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct IdempotencyKey(String);

impl IdempotencyKey {
    /// Name a key.
    ///
    /// # Errors
    ///
    /// [`EmptyToken`] for the empty string, which the schema's `minLength: 1` forbids.
    pub fn new(text: &str) -> Result<Self, EmptyToken> {
        if text.is_empty() {
            return Err(EmptyToken::IdempotencyKey);
        }
        Ok(Self(text.to_owned()))
    }

    /// The key.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for IdempotencyKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// A free-form label (node schema `labels`, `uniqueItems: true`).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Label(String);

impl Label {
    /// Name a label.
    ///
    /// # Errors
    ///
    /// [`EmptyToken`] for the empty string.
    pub fn new(text: &str) -> Result<Self, EmptyToken> {
        if text.is_empty() {
            return Err(EmptyToken::Label);
        }
        Ok(Self(text.to_owned()))
    }

    /// The label.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Label {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// A token that may not be empty was empty.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum EmptyToken {
    /// A [`ClaimId`].
    ClaimId,
    /// An [`IdempotencyKey`]; the node schema states `minLength: 1`.
    IdempotencyKey,
    /// A [`Label`].
    Label,
}

impl fmt::Display for EmptyToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let subject = match self {
            Self::ClaimId => "a claim identity",
            Self::IdempotencyKey => "an idempotency key",
            Self::Label => "a label",
        };
        write!(f, "{subject} is not the empty string")
    }
}

impl core::error::Error for EmptyToken {}

/// One entry in a node's append-only status history.
///
/// The fields are private and there are exactly two ways to obtain one:
/// [`StatusWrite::creation`], which is fixed at [`ClaimStatus::BOTTOM`] and names nothing
/// else, and [`crate::authority::Promotion::write`], which requires a promotion witness only
/// [`crate::authority`] can mint. "Creation is not a promotion" is therefore the shape of
/// the API rather than a rule stated in prose.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatusWrite {
    status: ClaimStatus,
    service_identity: Option<String>,
    validation_basis: Option<ValidationBasis>,
    inconclusive_reason: Option<InconclusiveReason>,
}

impl StatusWrite {
    /// The entry write: `proposed`, naming no service and carrying no payload.
    ///
    /// docs/44's `(creation) → proposed` row is the only one whose authority is "any
    /// authorized human/agent", and it is the only status this constructor can produce.
    #[must_use]
    pub const fn creation() -> Self {
        Self {
            status: ClaimStatus::BOTTOM,
            service_identity: None,
            validation_basis: None,
            inconclusive_reason: None,
        }
    }

    /// Build a promoted write.
    ///
    /// `pub(crate)` on purpose: [`crate::authority::Promotion::write`] is the only caller,
    /// and a doc test — which compiles as an external crate — pins that a client cannot
    /// reach it (see [`crate::authority`]'s second `compile_fail`).
    pub(crate) const fn promoted(
        status: ClaimStatus,
        service_identity: Option<String>,
        validation_basis: Option<ValidationBasis>,
        inconclusive_reason: Option<InconclusiveReason>,
    ) -> Self {
        Self {
            status,
            service_identity,
            validation_basis,
            inconclusive_reason,
        }
    }

    /// The status this write installed.
    #[must_use]
    pub const fn status(&self) -> ClaimStatus {
        self.status
    }

    /// The service identity that performed it, present exactly for the four
    /// assurance-bearing statuses (`evidence-graph-node.schema.json`).
    #[must_use]
    pub fn service_identity(&self) -> Option<&str> {
        self.service_identity.as_deref()
    }

    /// Whether the solver evidence was a checked certificate or a trusted solver, present
    /// exactly for `validated` (plan §11.4).
    #[must_use]
    pub const fn validation_basis(&self) -> Option<ValidationBasis> {
        self.validation_basis
    }

    /// The typed INV-008 reason, present exactly for `inconclusive` (plan §11.4).
    #[must_use]
    pub const fn inconclusive_reason(&self) -> Option<InconclusiveReason> {
        self.inconclusive_reason
    }
}

/// A typed, immutable evidence node.
///
/// Cloning is cheap enough to be the versioning mechanism: a promotion produces a new value
/// with one more history entry, and the old value stays a valid record of the version it
/// described.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvidenceNode {
    kind: NodeKind,
    artifact: ArtifactRef,
    claim_id: ClaimId,
    idempotency_key: IdempotencyKey,
    labels: BTreeSet<Label>,
    provenance: Provenance,
    history: Vec<StatusWrite>,
}

impl EvidenceNode {
    /// Propose a node.
    ///
    /// The only constructor. It takes no status — the history starts at
    /// [`StatusWrite::creation`] — and it takes a [`Provenance`] by value, so a node with no
    /// producer is unrepresentable rather than rejected.
    #[must_use]
    pub fn propose(
        kind: NodeKind,
        artifact: ArtifactRef,
        claim_id: ClaimId,
        idempotency_key: IdempotencyKey,
        provenance: Provenance,
    ) -> Self {
        Self {
            kind,
            artifact,
            claim_id,
            idempotency_key,
            labels: BTreeSet::new(),
            provenance,
            history: vec![StatusWrite::creation()],
        }
    }

    /// Attach labels (node schema `labels`, a unique-item array).
    #[must_use]
    pub fn with_labels(mut self, labels: impl IntoIterator<Item = Label>) -> Self {
        self.labels = labels.into_iter().collect();
        self
    }

    /// This node with one more status write appended.
    ///
    /// Append-only: the previous entries are copied unchanged and the receiver is untouched,
    /// so a caller holding the older value still holds a valid record of that version.
    /// `pub(crate)` because the only source of a promoted [`StatusWrite`] is a
    /// [`crate::authority::Promotion`], and the only caller is
    /// [`crate::graph::EvidenceGraph::promote`].
    #[must_use]
    pub(crate) fn appending(&self, write: StatusWrite) -> Self {
        let mut next = self.clone();
        next.history.push(write);
        next
    }

    /// The node kind.
    #[must_use]
    pub const fn kind(&self) -> NodeKind {
        self.kind
    }

    /// The artifact this node is about.
    #[must_use]
    pub const fn artifact(&self) -> &ArtifactRef {
        &self.artifact
    }

    /// The claim its status is linearized against (plan §11.7).
    #[must_use]
    pub const fn claim_id(&self) -> &ClaimId {
        &self.claim_id
    }

    /// The idempotency key of the write that produced it.
    #[must_use]
    pub const fn idempotency_key(&self) -> &IdempotencyKey {
        &self.idempotency_key
    }

    /// Its labels, in canonical ascending order.
    pub fn labels(&self) -> impl ExactSizeIterator<Item = &Label> {
        self.labels.iter()
    }

    /// Who produced it, from what, when, and under which epochs.
    #[must_use]
    pub const fn provenance(&self) -> &Provenance {
        &self.provenance
    }

    /// The status it currently holds.
    ///
    /// # Panics
    ///
    /// Never: the history is non-empty by construction — [`propose`](Self::propose) writes
    /// the creation entry and [`appending`](Self::appending) only ever adds.
    #[must_use]
    pub fn status(&self) -> ClaimStatus {
        self.history
            .last()
            .expect("a node's history is non-empty by construction")
            .status()
    }

    /// The version number: how many promotions have landed since creation.
    #[must_use]
    pub fn version(&self) -> usize {
        self.history.len() - 1
    }

    /// Every status write, oldest first.
    pub fn history(&self) -> impl ExactSizeIterator<Item = &StatusWrite> {
        self.history.iter()
    }

    /// The statuses this claim has held, oldest first — the input
    /// [`crate::claim_status::verify_promotion_history`] accepts or rejects.
    #[must_use]
    pub fn status_history(&self) -> Vec<ClaimStatus> {
        self.history.iter().map(StatusWrite::status).collect()
    }

    /// This node's content identity.
    ///
    /// A function of what the node *says* — kind, artifact, claim, labels, provenance — and
    /// not of its status history or its idempotency key. See the module documentation for
    /// why each exclusion is load-bearing.
    #[must_use]
    pub fn identity(&self) -> EvidenceIdentity {
        EvidenceIdentity::of(&self.to_preimage())
    }

    /// The canonical preimage this node's identity is the encoding of.
    #[must_use]
    pub fn to_preimage(&self) -> Value {
        Value::record([
            (field("artifact"), self.artifact.to_value()),
            (field("claim_id"), Value::text(self.claim_id.as_str())),
            (field("kind"), Value::text(self.kind.as_str())),
            (
                field("labels"),
                Value::seq(self.labels.iter().map(|label| Value::text(label.as_str())))
                    .expect("a label list nests two levels"),
            ),
            (field("provenance"), self.provenance.to_preimage()),
        ])
        .expect("five distinct compile-time field names")
    }

    /// This node as `evidence-graph-node.schema.json` writes it, given the handle it is
    /// filed under.
    ///
    /// The three conditional members — `service_identity`, `validation_basis`,
    /// `inconclusive_reason` — are emitted exactly when the schema's `if`/`then` clauses
    /// require them and omitted otherwise, because `additionalProperties` is false and a
    /// field written where the schema does not admit it is as invalid as one missing where
    /// it does.
    #[must_use]
    pub fn to_record(&self, handle: &crate::identity::EvidenceHandle) -> Value {
        let mut fields = vec![
            (
                field("schema_id"),
                Value::text("https://continuum.dev/schema/evidence-graph-node.json"),
            ),
            (field("schema_epoch"), Value::nat(1)),
            (field("node_id"), Value::text(handle.as_str())),
            (field("kind"), Value::text(self.kind.as_str())),
            (field("artifact"), self.artifact.to_value()),
            (field("status"), Value::text(self.status().as_str())),
            (field("claim_id"), Value::text(self.claim_id.as_str())),
            (
                field("idempotency_key"),
                Value::text(self.idempotency_key.as_str()),
            ),
            (field("provenance"), self.provenance.to_record()),
        ];
        if !self.labels.is_empty() {
            fields.push((
                field("labels"),
                Value::seq(self.labels.iter().map(|label| Value::text(label.as_str())))
                    .expect("a label list nests two levels"),
            ));
        }
        let write = self
            .history
            .last()
            .expect("a node's history is non-empty by construction");
        if let Some(service) = write.service_identity() {
            fields.push((field("service_identity"), Value::text(service)));
        }
        if let Some(basis) = write.validation_basis() {
            fields.push((field("validation_basis"), Value::text(basis.as_str())));
        }
        if let Some(reason) = write.inconclusive_reason() {
            fields.push((field("inconclusive_reason"), Value::text(reason.as_str())));
        }
        Value::record(fields).expect("the member names are distinct compile-time constants")
    }
}

/// A reference to a node: its content identity together with the kind it carries.
///
/// The schemas type an edge endpoint as a bare `ev_` handle and RFC 0038 types **none** of
/// the thirteen relations' endpoints by node kind. Carrying the kind here is therefore not
/// an endpoint rule — it is what makes any endpoint rule *expressible*, which the one open
/// question in this area needs: F14 asks which node kind a `CHECKED_BY` edge's to-handle
/// names, and [`crate::edge::CheckTargetRule`] answers it as a parameter rather than as a
/// commitment. Obtaining a `NodeRef` requires a node ([`NodeRef::of`]), so an edge cannot be
/// built against a kind nobody declared.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct NodeRef {
    identity: EvidenceIdentity,
    kind: NodeKind,
}

impl NodeRef {
    /// The reference to a node.
    #[must_use]
    pub fn of(node: &EvidenceNode) -> Self {
        Self {
            identity: node.identity(),
            kind: node.kind(),
        }
    }

    /// The referenced identity.
    #[must_use]
    pub const fn identity(&self) -> &EvidenceIdentity {
        &self.identity
    }

    /// The referenced node's kind.
    #[must_use]
    pub const fn kind(&self) -> NodeKind {
        self.kind
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use continuum_value::value::Value;

    use super::*;
    use crate::actor::ActorId;
    use crate::provenance::Timestamp;

    pub(crate) fn provenance(actor: &str) -> Provenance {
        Provenance::new(
            ActorId::new(actor).expect("well formed"),
            Timestamp::new("2026-08-01T12:00:00.000Z").expect("well formed"),
            [],
        )
    }

    pub(crate) fn node(kind: NodeKind, artifact: &str, claim: &str) -> EvidenceNode {
        EvidenceNode::propose(
            kind,
            ArtifactRef::new(artifact).expect("well formed"),
            ClaimId::new(claim).expect("non-empty"),
            IdempotencyKey::new("key-1").expect("non-empty"),
            provenance("agent:swarm-1"),
        )
    }

    fn members(value: &Value) -> Vec<String> {
        match value {
            Value::Record(fields) => fields.keys().map(|name| name.as_str().to_owned()).collect(),
            other => panic!("expected a record, got {other:?}"),
        }
    }

    #[test]
    fn the_kind_set_is_exactly_the_schema_enum() {
        let tokens: Vec<&str> = NodeKind::ALL.iter().map(|kind| kind.as_str()).collect();
        assert_eq!(
            tokens,
            [
                "intent_claim",
                "assumption",
                "property",
                "model",
                "program",
                "proof_goal",
                "candidate",
                "invariant_candidate",
                "ranking_candidate",
                "abstraction_map",
                "counterexample",
                "explanation",
                "repair_hypothesis",
                "patch",
                "run",
                "certificate",
                "receipt",
                "benchmark_result",
                "conflict",
                "decision"
            ]
        );
        assert_eq!(NodeKind::ALL.len(), 20);
        for kind in NodeKind::ALL {
            assert_eq!(NodeKind::from_token(kind.as_str()), Some(kind));
            assert_eq!(kind.to_string(), kind.as_str());
        }
        // Closed: neither a plan spelling nor an invention parses.
        assert_eq!(NodeKind::from_token("SynthesisCandidate"), None);
        assert_eq!(NodeKind::from_token("whiteboard"), None);
        assert_eq!(NodeKind::from_token(""), None);
    }

    #[test]
    fn every_kind_names_its_plan_11_2_type() {
        // plan §11.2's twenty, in its own order, matched to the schema's twenty.
        let plan = [
            "IntentClaim",
            "Assumption",
            "Property",
            "ModelVersion",
            "ProgramSnapshot",
            "ProofGoal",
            "InvariantCandidate",
            "RankingCandidate",
            "AbstractionMap",
            "SynthesisCandidate",
            "Counterexample",
            "CausalExplanation",
            "RepairHypothesis",
            "Patch",
            "VerificationRun",
            "Certificate",
            "ProofReceipt",
            "BenchmarkResult",
            "Conflict",
            "ReviewDecision",
        ];
        let mut named: Vec<&str> = NodeKind::ALL.iter().map(|kind| kind.plan_name()).collect();
        named.sort_unstable();
        let mut expected = plan.to_vec();
        expected.sort_unstable();
        assert_eq!(named, expected);
        // The one rename the schema records explicitly.
        assert_eq!(NodeKind::Candidate.as_str(), "candidate");
        assert_eq!(NodeKind::Candidate.plan_name(), "SynthesisCandidate");
    }

    #[test]
    fn a_new_node_is_proposed_and_nothing_else() {
        // docs/44: "(creation) → proposed"; RFC 0038: "there is no `draft` status".
        let node = node(NodeKind::Run, "trace_9f", "claim-1");
        assert_eq!(node.status(), ClaimStatus::Proposed);
        assert_eq!(node.status(), ClaimStatus::BOTTOM);
        assert_eq!(node.version(), 0);
        assert_eq!(node.history().len(), 1);
        let write = node.history().next().expect("the creation write");
        assert_eq!(write.service_identity(), None);
        assert_eq!(write.validation_basis(), None);
        assert_eq!(write.inconclusive_reason(), None);
        assert_eq!(StatusWrite::creation().status(), ClaimStatus::BOTTOM);
    }

    #[test]
    fn a_status_write_appends_and_never_replaces() {
        let node = node(NodeKind::Run, "trace_9f", "claim-1");
        assert_eq!(node.status_history(), [ClaimStatus::Proposed]);
        let promoted = node.appending(StatusWrite::promoted(
            ClaimStatus::Observed,
            None,
            None,
            None,
        ));
        // The receiver is untouched — the old value is still a record of version 0.
        assert_eq!(node.version(), 0);
        assert_eq!(node.status(), ClaimStatus::Proposed);
        assert_eq!(promoted.version(), 1);
        assert_eq!(promoted.status(), ClaimStatus::Observed);
        assert_eq!(
            promoted.status_history(),
            [ClaimStatus::Proposed, ClaimStatus::Observed]
        );
        // The creation entry is copied, not rewritten.
        assert_eq!(
            promoted.history().next().expect("the creation write"),
            node.history().next().expect("the creation write")
        );
    }

    #[test]
    fn identity_is_fixed_across_versions() {
        // plan §11.7: promotion is a compare-and-set against *the claim's* current status,
        // which requires one identity across the history.
        let node = node(NodeKind::Run, "trace_9f", "claim-1");
        let promoted = node.appending(StatusWrite::promoted(
            ClaimStatus::Observed,
            Some("service:x".to_owned()),
            None,
            None,
        ));
        assert_eq!(node.identity(), promoted.identity());
        assert_eq!(node.to_preimage().encode(), promoted.to_preimage().encode());
    }

    #[test]
    fn the_conditional_members_appear_exactly_where_the_schema_says() {
        // The three `if`/`then` clauses of `evidence-graph-node.schema.json`, over a node
        // that has actually reached each status.
        let handle = crate::identity::EvidenceHandle::new("ev_abc").expect("well formed");
        let bare = node(NodeKind::Run, "trace_9f", "claim-1");
        let validated = bare.appending(StatusWrite::promoted(
            ClaimStatus::Validated,
            Some("service:checker".to_owned()),
            Some(ValidationBasis::CheckedCertificate),
            None,
        ));
        let rendered = members(&validated.to_record(&handle));
        assert!(rendered.contains(&"service_identity".to_owned()));
        assert!(rendered.contains(&"validation_basis".to_owned()));
        assert!(!rendered.contains(&"inconclusive_reason".to_owned()));

        let inconclusive = bare.appending(StatusWrite::promoted(
            ClaimStatus::Inconclusive,
            None,
            None,
            Some(InconclusiveReason::ResourceExhausted),
        ));
        let rendered = members(&inconclusive.to_record(&handle));
        assert!(rendered.contains(&"inconclusive_reason".to_owned()));
        assert!(!rendered.contains(&"service_identity".to_owned()));
        assert!(!rendered.contains(&"validation_basis".to_owned()));
    }

    #[test]
    fn the_idempotency_key_is_outside_the_identity() {
        // "a replayed write returns the original node identity" (RFC 0038), and two writers
        // that independently produce the identical node have produced one node.
        let first = node(NodeKind::Run, "trace_9f", "claim-1");
        let second = EvidenceNode::propose(
            NodeKind::Run,
            ArtifactRef::new("trace_9f").expect("well formed"),
            ClaimId::new("claim-1").expect("non-empty"),
            IdempotencyKey::new("key-2").expect("non-empty"),
            provenance("agent:swarm-1"),
        );
        assert_ne!(first.idempotency_key(), second.idempotency_key());
        assert_eq!(first.identity(), second.identity());
    }

    #[test]
    fn what_a_node_says_is_in_its_identity() {
        // The anti-vacuity companion of the two exclusions above: every member that *is*
        // part of what the node says moves the identity.
        let base = node(NodeKind::Run, "trace_9f", "claim-1");
        let others = [
            node(NodeKind::Certificate, "trace_9f", "claim-1"),
            node(NodeKind::Run, "trace_aa", "claim-1"),
            node(NodeKind::Run, "trace_9f", "claim-2"),
            base.clone()
                .with_labels([Label::new("frontier").expect("non-empty")]),
            EvidenceNode::propose(
                NodeKind::Run,
                ArtifactRef::new("trace_9f").expect("well formed"),
                ClaimId::new("claim-1").expect("non-empty"),
                IdempotencyKey::new("key-1").expect("non-empty"),
                provenance("agent:swarm-2"),
            ),
        ];
        for other in others {
            assert_ne!(base.identity(), other.identity());
        }
    }

    #[test]
    fn the_record_is_the_schema_object() {
        let handle = crate::identity::EvidenceHandle::new("ev_abc").expect("well formed");
        let bare = node(NodeKind::Run, "trace_9f", "claim-1");
        let mut rendered = members(&bare.to_record(&handle));
        rendered.sort();
        assert_eq!(
            rendered,
            [
                "artifact",
                "claim_id",
                "idempotency_key",
                "kind",
                "node_id",
                "provenance",
                "schema_epoch",
                "schema_id",
                "status"
            ]
        );
        // A proposed node carries none of the three conditional members, which is what the
        // schema's `if`/`then` clauses say: they attach to statuses a proposal has not
        // reached. Their emission is asserted with the promotion path (PR-7 / IMPL-03).
        assert!(!rendered.contains(&"service_identity".to_owned()));
        assert!(!rendered.contains(&"validation_basis".to_owned()));
        assert!(!rendered.contains(&"inconclusive_reason".to_owned()));
        // `labels` appears only when there are labels.
        assert!(!rendered.contains(&"labels".to_owned()));
        let labelled = bare.with_labels([Label::new("frontier").expect("non-empty")]);
        assert!(members(&labelled.to_record(&handle)).contains(&"labels".to_owned()));
    }

    #[test]
    fn empty_tokens_are_refused() {
        assert_eq!(ClaimId::new(""), Err(EmptyToken::ClaimId));
        assert_eq!(IdempotencyKey::new(""), Err(EmptyToken::IdempotencyKey));
        assert_eq!(Label::new(""), Err(EmptyToken::Label));
        assert_eq!(
            EmptyToken::IdempotencyKey.to_string(),
            "an idempotency key is not the empty string"
        );
    }

    #[test]
    fn a_node_ref_carries_the_kind_it_was_taken_from() {
        let node = node(NodeKind::Certificate, "cert_9f", "claim-1");
        let reference = NodeRef::of(&node);
        assert_eq!(reference.kind(), NodeKind::Certificate);
        assert_eq!(reference.identity(), &node.identity());
    }

    #[test]
    fn labels_are_a_set_in_canonical_order() {
        let node = node(NodeKind::Run, "trace_9f", "claim-1").with_labels([
            Label::new("zeta").expect("non-empty"),
            Label::new("alpha").expect("non-empty"),
            Label::new("zeta").expect("non-empty"),
        ]);
        let listed: Vec<&str> = node.labels().map(Label::as_str).collect();
        assert_eq!(listed, ["alpha", "zeta"]);
    }
}
