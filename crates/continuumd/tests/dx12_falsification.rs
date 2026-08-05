//! G0-DX-12 falsification campaign: *can intent remain stable across swarm work?*
//!
//! # Why this file exists
//!
//! `notes/plan/notes/G0_SPIKE_MATRIX.md` states the G0-DX-12 required experiment as
//! **"planner, repairer, prover, reviewer with adversarial patch"**, its pass condition as
//! **"evidence graph catches every unauthorized intent edge"**, and its failure consequence
//! as **"block multi-agent autonomy"**. Its recorded Phase A evidence was
//! `spikes/R3_SPIKE_REPORT.md` §8 — a finite Python evidence graph that validated an
//! *artifact shape* and an *authority table*, "not enforcement" (docs/53, and the row's own
//! Decision cell). The G0 promotion rule refuses to turn a spike into architecture without
//! adversarial mutations and a reference implementation.
//!
//! This file is that campaign against the production implementation. The four swarm roles
//! are driven as **principals over the real daemon wire** — `Daemon::dispatch` and nothing
//! else — and the "adversarial patch" is an untrusted role attempting to move the stable
//! intent, or to forge the evidence-graph record that would license moving it, through every
//! channel the RFCs and the IDL expose. The roles are *identities plus capability profiles*,
//! not new engines: the planner proposes (`intent.propose_revision`), the repairer patches
//! (`observe.ingest` — a producer's candidate), the prover submits proof artifacts
//! (`observe.ingest` of a certificate/observation), and the reviewer reviews (the privileged
//! `intent.accept`/`reject`/`lock` authority, and the trusted verification service that
//! promotes claims and appends `CHECKED_BY` edges). `src/` is not touched: a falsification
//! campaign that edits the thing it is falsifying proves nothing (the bn-21dd precedent,
//! `dx13_falsification.rs`'s and `dx03_falsification.rs`'s house rule).
//!
//! # The four roles, as capability profiles (RFC 0027, docs/49)
//!
//! | role | actor | level | profile | what it may do |
//! |---|---|---|---|---|
//! | planner  | `agent:planner`   | `revise-intent` | no privileged op | propose a revision — and nothing that accepts one (docs/49 "revise intent: proposal only") |
//! | repairer | `agent:repairer`  | `execute` + production-trace grant | — | append a candidate observation; the adversary of this campaign |
//! | prover   | `agent:prover`    | `execute` + production-trace grant | — | append a proof/observation node |
//! | reviewer | `service:reviewer`| `promote` + production-trace grant | privileged `intent.accept`/`reject`/`lock` | accept/lock intent; and, as the daemon's verification service, promote claims and append `CHECKED_BY` edges |
//!
//! The reviewer is a `service:` actor deliberately: RFC 0038 "Authority" makes both a status
//! promotion and the `CHECKED_BY` edge that records a check the province of a *service*
//! identity, never an `agent:`/`human:`/`ci:` one, and RFC 0027 P3 says an evidence-status
//! write is "performed by a trusted service identity". The daemon under test verifies *as*
//! `service:reviewer` (`EvidenceFamily::verifying_as`), so the reviewer is the one trusted
//! checker in the swarm.
//!
//! # The channels, enumerated from the RFCs and the IDL (never invented)
//!
//! An "unauthorized intent edge" is any write that would let the stable intent change, or
//! that would forge the graph record licensing such a change, without crossing the one
//! privileged path RFC 0027/0037/0038 fix. The channels are exactly these, and every one is
//! a sentence in a governing text:
//!
//! **Intent-authority channels — moving the contract without the privileged path:**
//!
//! - **A1** a producer (`agent:repairer`) calls `intent.accept` — RFC 0027 T1∧T3.
//! - **A2** the planner has the `revise-intent` *level* but not the *privilege*, and calls
//!   `intent.accept` — RFC 0027's "privilege is not a level", docs/49's proposal-only cell.
//!   This is the channel that isolates T3.
//! - **A3** a producer calls `intent.lock` — RFC 0027 T3, plan §5.4.
//! - **A4** the planner *proposes* a change to a `locked` field — `IntentMutationDenied`,
//!   "a change set naming a field whose change policy is `locked`" (RFC 0037 P4, plan §5.4).
//!   This is the one where admission passes and the *daemon* enforces the lock.
//! - **A5** a producer calls `intent.reject` — RFC 0027 T3.
//!
//! **Evidence-graph authority channels — forging the record that licenses a change:**
//!
//! - **B1 direct edge mint by a non-service principal** — the repairer (or planner) calls
//!   `evidence.link` — RFC 0038 "Authority": a checker is a `service:` actor; the refusal is
//!   `CapabilityDenied` (RFC 0027 X1).
//! - **B2 promotion forgery** — a producer names or forges a promoted status: there is no
//!   status field on `observe.ingest`; a producer's append lands at `proposed`; a caller's
//!   `expected_status` is the compare-and-set *guard*, never the written value; and the
//!   status a verify writes is what the daemon's own re-derivation established, under the
//!   *service's* identity, never the caller's (INV-004, RFC 0038 "Authority").
//! - **B3 self-certification** — the verification service promotes, or checks, its own
//!   production — `InsufficientEvidence`, checked before the artifact is read (INV-004,
//!   RFC 0038 "No self-certification").
//! - **B4 edge retargeting** — a node filed under an identity it does not derive, or
//!   retargeted at other content — `CertificateRejected` (the re-derivation discipline,
//!   RFC 0038 D2/D4, `evidence.verify`).
//! - **B5 replay of an authorized edge to smuggle content** — a `CHECKED_BY` edge replayed
//!   to attach different content: an edge's identity *is* the content of what it asserts
//!   (RFC 0038 D2), and its provenance is outside that identity (D4), so an identical replay
//!   converges on one edge and a replay with different content mints a *distinct, visible,
//!   attributed* edge rather than silently rewriting the authorized one.
//!
//! **Intent-status-alteration-via-evidence-writes channel:**
//!
//! - **C1** — the evidence graph and the intent registry are separate stores, and **no
//!   `evidence.*`/`observe.*` operation writes an intent registry status.** A producer's
//!   whole evidence workflow around a stable, accepted contract leaves the contract's status
//!   and bytes byte-identical: the channel a naive design would leave open — an evidence edge
//!   that flips an intent's acceptance — does not exist to be attacked.
//!
//! # The campaign map
//!
//! | # | Channel | Attack | Pre-registered refusal | Result |
//! |---|---|---|---|---|
//! | A1 | intent authority | repairer accepts a proposal | `CapabilityDenied`; registry unchanged | held |
//! | A2 | intent authority (T3) | planner (has the level) accepts | `CapabilityDenied`; registry unchanged | held |
//! | A3 | intent authority | repairer locks a field | `CapabilityDenied`; registry unchanged | held |
//! | A4 | intent authority (lock) | planner proposes onto a `locked` field | `IntentMutationDenied`; registry unchanged | held |
//! | A5 | intent authority | repairer rejects a proposal | `CapabilityDenied`; proposal survives | held |
//! | B1 | direct edge mint | repairer/planner mint a `CHECKED_BY` edge | `CapabilityDenied`; no edge | held |
//! | B2 | promotion forgery | producer names/forges a promoted status | no field; `proposed`; guard ≠ write; service-attributed | held |
//! | B3 | self-certification | reviewer verifies/links its own production | `InsufficientEvidence`; nothing written | held |
//! | B4 | edge retargeting | misfiled / retargeted node | `CertificateRejected`; status unmoved | held |
//! | B5 | authorized-edge replay | replay a check edge to smuggle content | identical → one edge; different → distinct visible edge | held |
//! | C1 | intent-status via evidence | a full evidence workflow around a stable contract | contract byte-identical; no such verb | held |
//!
//! # The named baseline and controls (as load-bearing as the attacks)
//!
//! - `baseline_authorized_accept_by_the_reviewer_moves_the_proposal_to_accepted`,
//! - `baseline_authorized_lock_by_the_reviewer_mints_a_successor_and_supersedes`,
//! - `baseline_ordinary_evidence_work_flows_cleanly_end_to_end`.
//!
//! Every A/B attack that isolates one gate carries a control that satisfies *only that gate*
//! and shows the same call land, so the refusal is attributable to the authority check and
//! not to a malformed request: B1's control is the reviewer's service scheme, B3's is a
//! different producer, B4's is the node under its own identity, A2's is the privileged
//! reviewer.
//!
//! # Anti-vacuity: the harness can fail (a disabled authority check flips the verdict)
//!
//! Two `mutant_*` tests remove exactly one authority gate through the capability system it is
//! built on, and assert the attack **lands** under the loosened configuration:
//!
//! - `mutant_granting_the_planner_the_accept_privilege_lands_the_unauthorized_accept` grants
//!   the planner's profile `intent.accept` — same actor, same `revise-intent` level, one
//!   privileged op added — and the A2 accept then moves the proposal to `Accepted`. A daemon
//!   that read T3 fail-open would land A2 under the tight profile, and A2's own assertion
//!   (expect `CapabilityDenied`) would then fail: the gate is load-bearing.
//! - `mutant_a_service_actor_lands_the_check_edge_the_agent_could_not` issues B1's identical
//!   link from a `service:` actor and lands the edge, pinning the actor-scheme gate the same
//!   way.
//!
//! # House rules
//!
//! - `src/` is not touched, and no pre-existing test is edited, weakened, or moved.
//! - Every assertion is on a typed value or on canonical bytes, never on timing. This file
//!   makes **no concurrency claim**: `Daemon::dispatch` takes `&mut self`, so one principal
//!   drives one request at a time — the swarm is the *linearized-per-claim* write model
//!   RFC 0038 fixes, and true concurrency is G0-DX-13's axis, evidenced in
//!   `continuum-workspace`'s own campaign. The graph "catching" an unauthorized edge is a
//!   typed refusal plus an unchanged, observable record, which is exactly what a
//!   single-threaded dispatch grain can establish.
//! - Every pre-registered refusal was fixed from the governing texts before the campaign ran.
//! - Classifier-grain intent weakening (weaken property, strengthen assumptions, reduce
//!   bounds, hide observers, remove faults) is **G0-DX-02's** axis and is not re-run here:
//!   `crates/continuum-semantic-diff/tests/dx02_falsification.rs` (bn-3cgt) is its
//!   reference-implementation evidence. This campaign is about *authority over the record*,
//!   the dimension DX-02 does not touch.
//! - Bounded by construction: a fixed number of principals, fixed inputs, fixed budgets, no
//!   clock beyond the reading the daemon is handed, no entropy, `BTreeMap`-ordered
//!   everything, no loop-bound allocation and no thread.
//!
//! # Result of the campaign
//!
//! **Every unauthorized channel was refused by a typed authority check and left the record
//! unchanged; the graph absorbed none of them silently. The three baselines flowed cleanly
//! and both anti-vacuity mutants flipped the verdict.** The pass condition holds at
//! production grain. Scope is stated plainly in the house rules above: single-threaded
//! dispatch (concurrency is DX-13), and the classifier lane is DX-02's (cited, not re-run).
//! One boundary is recorded, not a defect: `intent.propose_revision` returns
//! `UnsupportedSemanticFeature` for a change to an *unlocked* field because the RFC 0031
//! classification lane is a later PR-12 daemon obligation — but a change to a *locked* field
//! is refused `IntentMutationDenied` first (A4), and the lock, not the classifier, is what
//! holds the stable intent stable here.

use std::collections::BTreeMap;

use continuum_evidence::claim_status::ClaimStatus;
use continuum_intent::canonical_json::Json;
use continuum_intent::change_policy::{PolicyField, PolicyVerb};
use continuum_intent::contract::IntentContract;
use continuum_value::epoch::ProtocolWindow;
use continuum_workspace::artifact_path::ArtifactClass;
use continuum_workspace::publication::ContentIdentifier;
use continuum_workspace::snapshot::WorkspacePath;
use continuumd::daemon::evidence::EvidenceFamily;
use continuumd::daemon::family::{Arguments, Payload};
use continuumd::daemon::identity::{Blake3Identity, intent_to_wire};
use continuumd::daemon::intent::IntentFamily;
use continuumd::daemon::observe::ObserveFamily;
use continuumd::daemon::state::{EvidenceNode, IntentRecord, RegistryStatus};
use continuumd::daemon::{Daemon, OperationOutcome, OperationRequest};
use continuumd::protocol::envelope::{Budget, RequestEnvelope};
use continuumd::protocol::handshake::{
    CapabilityDescriptor, CapabilityProfile, ClientHello, Negotiated, VersionRange, negotiate,
};
use continuumd::protocol::operations::evidence::{EvidenceLinkRequest, EvidenceVerifyRequest};
use continuumd::protocol::operations::intent::{
    IntentAcceptRequest, IntentLockRequest, IntentProposeRevisionRequest, IntentRejectRequest,
};
use continuumd::protocol::operations::observe::ObserveIngestRequest;
use continuumd::protocol::registry::{self, ENCODINGS};
use continuumd::protocol::scalar::{
    ActorId, CapabilityHandle, Commitment, EvidenceHandle, IntentHandle, Opaque, OperationName,
    ProtocolVersion, RequestId, Timestamp,
};
use continuumd::protocol::shared::IntentChangeSet;
use continuumd::protocol::spec::{Nullable, Optional};
use continuumd::protocol::vocabulary::{
    AuthorityLevel, DataGrant, Encoding, ErrorCode, EvidenceStatus, ResultStatus,
};

/// The Die Hard Intent Contract, as `continuum-intent`'s own suites use it: `properties` and
/// `assumptions` `locked`, `bounds` `no-decrease`, `fairness` `unlocked`.
const DIE_HARD_CONTRACT: &str =
    include_str!("../../continuum-intent/tests/fixtures/die-hard-contract.json");

/// The repairer's candidate — a production trace it wants believed. Distinct bytes per role
/// so distinct nodes derive (a node's identity is a function of its content, RFC 0038 D4).
const PATCH_TRACE: &str = "{\"events\":[{\"at\":0,\"op\":\"patch-fill\"}]}\n";
/// The prover's observation.
const PROOF_TRACE: &str = "{\"events\":[{\"at\":0,\"op\":\"proof-pour\"}]}\n";
/// The reviewer's own production, for the self-certification channel (B3).
const REVIEW_TRACE: &str = "{\"events\":[{\"at\":0,\"op\":\"review-empty\"}]}\n";
/// A receipt blob, for the check edge's `to` content.
const RECEIPT_BLOB: &str = "{\"receipt\":\"kernel-core re-checked this\"}\n";
/// A second receipt blob, for the B5 smuggling attempt.
const OTHER_RECEIPT_BLOB: &str = "{\"receipt\":\"a different check entirely\"}\n";
/// The instrumentation profile the traces were captured under (plan §18.4).
const PROFILE: &str = "otel-1.0/sampled";

// =========================================================================================
// fixture plumbing — the idioms `daemon_evidence.rs` and `dx03_falsification.rs` share
// =========================================================================================

fn version() -> ProtocolVersion {
    ProtocolVersion::new(3, 3)
}

fn cap(handle: &str) -> CapabilityHandle {
    CapabilityHandle::new(handle).expect("a well-formed capability handle")
}

fn who(actor: &str) -> ActorId {
    ActorId::new(actor).expect("a well-formed actor identity")
}

fn name(operation: &str) -> OperationName {
    OperationName::new(operation).expect("a well-formed operation name")
}

fn now() -> Timestamp {
    Timestamp::new("2026-08-01T00:00:00.000Z").expect("a well-formed timestamp")
}

/// A capability profile granting the named privileged operations and, optionally, the
/// production-trace data grant `observe.ingest` requires beyond its level (RFC 0027 R-4).
fn profile(privileged: &[&str], trace: bool) -> CapabilityProfile {
    CapabilityProfile {
        privileged_operations: privileged.iter().map(|entry| name(entry)).collect(),
        denied_operations: Vec::new(),
        data_grants: if trace {
            vec![DataGrant::ProductionTrace]
        } else {
            Vec::new()
        },
        cross_principal_sharing: false,
    }
}

fn grant(
    handle: &str,
    actor: &str,
    level: AuthorityLevel,
    depth: u32,
    profile: Optional<CapabilityProfile>,
) -> CapabilityDescriptor {
    CapabilityDescriptor {
        capability: cap(handle),
        actor: who(actor),
        level,
        snapshots: Vec::new(),
        intents: Vec::new(),
        artifact_classes: Vec::new(),
        expires_at: Nullable::Null,
        delegation_depth: depth,
        profile,
    }
}

fn negotiated() -> Negotiated {
    let hello = ClientHello {
        protocol_versions: VersionRange {
            low: version(),
            high: version(),
        },
        encodings: vec![Encoding::CanonicalJson],
        client: "continuumd-dx12-falsification".to_owned(),
        actor: who("service:continuumd"),
        capability: cap("cap_root"),
        features: Optional::Absent,
    };
    negotiate(&[version()], ProtocolWindow::new(3), ENCODINGS, &hello).expect("3.3 is served")
}

/// The daemon under test. The reviewer is the verification service, and the four roles are
/// registered as delegations of the connection capability. `planner_can_accept` is the
/// anti-vacuity mutant switch: the one thing that differs between the honest daemon and the
/// mutant is whether the planner's profile grants `intent.accept`.
fn daemon_with(planner_can_accept: bool) -> Daemon {
    let root = Some(cap("cap_root"));
    let planner_privileged: &[&str] = if planner_can_accept {
        &["intent.accept"]
    } else {
        &[]
    };
    Daemon::builder(Blake3Identity, negotiated(), cap("cap_root"))
        .now(now())
        // The connection capability. A `service:` root at `promote`.
        .capability(
            grant(
                "cap_root",
                "service:continuumd",
                AuthorityLevel::Promote,
                4,
                Optional::Present(profile(
                    &["intent.accept", "intent.reject", "intent.lock"],
                    true,
                )),
            ),
            None,
        )
        // planner: `revise-intent` level, and — honestly — no privileged operation, which is
        // docs/49's "revise intent: proposal only" cell.
        .capability(
            grant(
                "cap_planner",
                "agent:planner",
                AuthorityLevel::ReviseIntent,
                3,
                Optional::Present(profile(planner_privileged, false)),
            ),
            root.clone(),
        )
        // repairer: `execute` plus the production-trace grant, no privilege. The adversary.
        .capability(
            grant(
                "cap_repairer",
                "agent:repairer",
                AuthorityLevel::Execute,
                3,
                Optional::Present(profile(&[], true)),
            ),
            root.clone(),
        )
        // prover: `execute` plus the production-trace grant.
        .capability(
            grant(
                "cap_prover",
                "agent:prover",
                AuthorityLevel::Execute,
                3,
                Optional::Present(profile(&[], true)),
            ),
            root.clone(),
        )
        // reviewer: the privileged `service:` principal, also the daemon's checker.
        .capability(
            grant(
                "cap_reviewer",
                "service:reviewer",
                AuthorityLevel::Promote,
                3,
                Optional::Present(profile(
                    &["intent.accept", "intent.reject", "intent.lock"],
                    true,
                )),
            ),
            root.clone(),
        )
        // a plain reader: `evidence.verify`/`get` are read authority.
        .capability(
            grant(
                "cap_reader",
                "agent:reader",
                AuthorityLevel::Read,
                3,
                Optional::Absent,
            ),
            root,
        )
        // The daemon verifies *as* the reviewer, so the reviewer is the trusted checker.
        .family(EvidenceFamily::verifying_as(who("service:reviewer")))
        .family(ObserveFamily)
        .family(IntentFamily)
        .build()
}

/// The swarm's shared world: a daemon holding a `Proposed` Die Hard contract and the four
/// staged blobs the roles work over.
struct Swarm {
    daemon: Daemon,
    /// The intent under swarm work.
    proposal: IntentHandle,
    /// The repairer's candidate trace.
    patch: Commitment,
    /// The prover's observation trace.
    proof: Commitment,
    /// The reviewer's own-production trace (B3).
    review: Commitment,
    /// A receipt blob (a check edge's `to` content).
    receipt: Commitment,
    /// A second receipt blob (B5).
    other_receipt: Commitment,
}

fn die_hard_contract() -> IntentContract {
    IntentContract::decode(DIE_HARD_CONTRACT.trim_end().as_bytes()).expect("the fixture decodes")
}

fn intent_handle(contract: &IntentContract) -> IntentHandle {
    let stored = ContentIdentifier::identify(
        &Blake3Identity,
        ArtifactClass::IntentContract,
        &contract.identity_preimage_bytes(),
    )
    .expect("blake3 names every input");
    intent_to_wire(&stored).expect("an `in_` handle")
}

fn swarm() -> Swarm {
    swarm_with(false)
}

fn swarm_with(planner_can_accept: bool) -> Swarm {
    let mut daemon = daemon_with(planner_can_accept);
    let contract = die_hard_contract();
    let proposal = intent_handle(&contract);
    daemon.state_mut().put_intent(
        proposal.clone(),
        IntentRecord {
            contract,
            status: RegistryStatus::Proposed,
            supersedes: None,
            superseded_by: None,
            acceptance: None,
        },
    );

    let mut stage = |path: &str, content: &str| {
        daemon
            .state_mut()
            .stage(
                &Blake3Identity,
                WorkspacePath::new(path).expect("a workspace path"),
                content.as_bytes().to_vec(),
            )
            .expect("staging names its content")
    };
    let patch = stage("traces/patch.jsonl", PATCH_TRACE);
    let proof = stage("traces/proof.jsonl", PROOF_TRACE);
    let review = stage("traces/review.jsonl", REVIEW_TRACE);
    let receipt = stage("receipts/kernel-core.json", RECEIPT_BLOB);
    let other_receipt = stage("receipts/other.json", OTHER_RECEIPT_BLOB);

    Swarm {
        daemon,
        proposal,
        patch,
        proof,
        review,
        receipt,
        other_receipt,
    }
}

// --- envelopes ---------------------------------------------------------------------------

fn envelope(operation: &str, actor: &str, capability: &str, request: &str) -> RequestEnvelope {
    RequestEnvelope {
        protocol_version: version(),
        request_id: RequestId::new(request).expect("a well-formed request id"),
        idempotency_key: Optional::Absent,
        actor: who(actor),
        capability: cap(capability),
        operation: name(operation),
        snapshot: Nullable::Null,
        intent: Nullable::Null,
        arguments: Opaque::from_bytes(Vec::new()),
        budget: Optional::Absent,
        output_policy: Optional::Absent,
        trace: Optional::Absent,
        page: Optional::Absent,
    }
}

fn budget() -> Budget {
    Budget {
        wall_ms: Optional::Absent,
        cpu_ms: Optional::Absent,
        memory_bytes: Optional::Absent,
        states: Optional::Absent,
        solver_ms: Optional::Absent,
        proof_ms: Optional::Absent,
        tokens: Optional::Absent,
        candidates: Optional::Absent,
        bytes: Optional::Absent,
    }
}

/// A `@mutation` envelope with only its idempotency key set.
fn keyed(mut envelope: RequestEnvelope, key: &str) -> RequestEnvelope {
    envelope.idempotency_key = Optional::Present(key.to_owned());
    envelope
}

/// A `@mutation @task_starting` envelope: key plus a budget, which `obligation` requires
/// before an ingesting or verifying family runs.
fn started(mut envelope: RequestEnvelope, key: &str) -> RequestEnvelope {
    envelope.idempotency_key = Optional::Present(key.to_owned());
    envelope.budget = Optional::Present(budget());
    envelope
}

// --- the operations, each as one dispatch ------------------------------------------------

fn ingest_as(
    sw: &mut Swarm,
    actor: &str,
    capability: &str,
    trace: &Commitment,
    request: &str,
    key: &str,
) -> OperationOutcome {
    sw.daemon.dispatch(&OperationRequest {
        envelope: started(envelope("observe.ingest", actor, capability, request), key),
        arguments: Arguments::ObserveIngest(ObserveIngestRequest {
            trace: trace.clone(),
            instrumentation_profile: PROFILE.to_owned(),
        }),
    })
}

fn ingested_handle(outcome: &OperationOutcome) -> EvidenceHandle {
    match &outcome.payload {
        Payload::ObserveIngest(response) => response
            .evidence
            .first()
            .cloned()
            .expect("an ingest names the node it appended"),
        other => panic!("expected an observe.ingest payload, got {other:?}"),
    }
}

fn verify_as(
    sw: &mut Swarm,
    actor: &str,
    capability: &str,
    handle: &EvidenceHandle,
    expected: Optional<EvidenceStatus>,
    request: &str,
    key: &str,
) -> OperationOutcome {
    sw.daemon.dispatch(&OperationRequest {
        envelope: started(envelope("evidence.verify", actor, capability, request), key),
        arguments: Arguments::EvidenceVerify(EvidenceVerifyRequest {
            evidence: handle.clone(),
            expected_status: expected,
        }),
    })
}

fn link_as(
    sw: &mut Swarm,
    actor: &str,
    capability: &str,
    subject: &EvidenceHandle,
    receipt: &Commitment,
    request: &str,
    key: &str,
) -> OperationOutcome {
    sw.daemon.dispatch(&OperationRequest {
        envelope: keyed(envelope("evidence.link", actor, capability, request), key),
        arguments: Arguments::EvidenceLink(EvidenceLinkRequest {
            subject: subject.clone(),
            receipt: receipt.clone(),
            checker_profile: "kernel-core/1".to_owned(),
        }),
    })
}

fn accept_as(
    sw: &mut Swarm,
    actor: &str,
    capability: &str,
    proposal: &IntentHandle,
    request: &str,
    key: &str,
) -> OperationOutcome {
    sw.daemon.dispatch(&OperationRequest {
        envelope: keyed(envelope("intent.accept", actor, capability, request), key),
        arguments: Arguments::IntentAccept(IntentAcceptRequest {
            proposal: proposal.clone(),
            acceptance: acceptance_bytes(),
            bundle: Optional::Absent,
        }),
    })
}

fn reject_as(
    sw: &mut Swarm,
    actor: &str,
    capability: &str,
    proposal: &IntentHandle,
    request: &str,
    key: &str,
) -> OperationOutcome {
    sw.daemon.dispatch(&OperationRequest {
        envelope: keyed(envelope("intent.reject", actor, capability, request), key),
        arguments: Arguments::IntentReject(IntentRejectRequest {
            proposal: proposal.clone(),
            reason: "an unauthorized reject attempt".to_owned(),
        }),
    })
}

fn lock_as(
    sw: &mut Swarm,
    actor: &str,
    capability: &str,
    intent: &IntentHandle,
    edit: (PolicyField, PolicyVerb),
    request: &str,
    key: &str,
) -> OperationOutcome {
    let (field, verb) = edit;
    let mut policy = BTreeMap::new();
    policy.insert(field.wire().to_owned(), verb.wire().to_owned());
    sw.daemon.dispatch(&OperationRequest {
        envelope: keyed(envelope("intent.lock", actor, capability, request), key),
        arguments: Arguments::IntentLock(IntentLockRequest {
            intent: intent.clone(),
            policy,
        }),
    })
}

fn propose_as(
    sw: &mut Swarm,
    actor: &str,
    capability: &str,
    base: &IntentHandle,
    change_key: &str,
    request: &str,
    key: &str,
) -> OperationOutcome {
    let mut changes: BTreeMap<String, Json> = BTreeMap::new();
    changes.insert(change_key.to_owned(), Json::Array(Vec::new()));
    sw.daemon.dispatch(&OperationRequest {
        envelope: keyed(
            envelope("intent.propose_revision", actor, capability, request),
            key,
        ),
        arguments: Arguments::IntentProposeRevision(IntentProposeRevisionRequest {
            base: base.clone(),
            changes: IntentChangeSet {
                changes: Opaque::from_bytes(Json::Object(changes).to_canonical_bytes()),
                rationale: "a typed rationale for reviewers".to_owned(),
            },
        }),
    })
}

/// A valid acceptance record for this registry: `capability` is fixed to `revise-intent` by
/// the schema, and `audit_record` is overwritten by the daemon (`rule audit.correlation`).
fn acceptance_bytes() -> Opaque {
    let mut fields: BTreeMap<String, Json> = BTreeMap::new();
    for (key, value) in [
        ("accepted_by", "service:reviewer"),
        ("capability", "revise-intent"),
        ("signature", "sig-die-hard-v1"),
        ("audit_record", "supplied-by-the-caller-and-overwritten"),
        ("timestamp", "2026-08-01T00:00:00.000Z"),
    ] {
        fields.insert(key.to_owned(), Json::String(value.to_owned()));
    }
    Opaque::from_bytes(Json::Object(fields).to_canonical_bytes())
}

// --- observers of the record -------------------------------------------------------------

fn code(outcome: &OperationOutcome) -> ErrorCode {
    outcome.error_code().expect("an error result")
}

fn node(sw: &Swarm, handle: &EvidenceHandle) -> EvidenceNode {
    sw.daemon
        .state()
        .evidence(handle)
        .expect("the graph holds the node")
        .clone()
}

/// A byte-stable fingerprint of the whole graph and of one intent record: the pass condition
/// is "the graph never silently absorbs an unauthorized edge", so an attack that is *caught*
/// leaves this value unchanged and an attack that *lands* moves it. Reading it before and
/// after each refused attack is how the refusal is checked to be more than a returned error.
fn fingerprint(sw: &Swarm, intent: &IntentHandle) -> (usize, usize, String, Vec<u8>) {
    let record = sw
        .daemon
        .state()
        .intent(intent)
        .expect("the registry holds it");
    (
        sw.daemon.state().evidence_nodes().count(),
        sw.daemon.state().evidence_edges().count(),
        record.status.as_wire().to_owned(),
        record.contract.to_artifact_bytes(),
    )
}

/// The reviewer accepts the proposal, so a test can attack a *stable, accepted* contract.
fn reviewer_accepts(sw: &mut Swarm) {
    let proposal = sw.proposal.clone();
    let accepted = accept_as(
        sw,
        "service:reviewer",
        "cap_reviewer",
        &proposal,
        "req_accept",
        "idem-accept",
    );
    assert_eq!(
        accepted.envelope.status,
        ResultStatus::Ok,
        "the named baseline must hold before an attack runs against it: {:?}",
        accepted.envelope.error
    );
}

// =========================================================================================
// The named baseline: authorized intent revision, and ordinary evidence work, flow cleanly
// =========================================================================================

#[test]
fn baseline_authorized_accept_by_the_reviewer_moves_the_proposal_to_accepted() {
    let mut sw = swarm();
    let proposal = sw.proposal.clone();
    assert_eq!(
        sw.daemon.state().intent(&proposal).unwrap().status,
        RegistryStatus::Proposed
    );

    let accepted = accept_as(
        &mut sw,
        "service:reviewer",
        "cap_reviewer",
        &proposal,
        "req_accept",
        "idem-accept",
    );
    assert_eq!(
        accepted.envelope.status,
        ResultStatus::Ok,
        "{:?}",
        accepted.envelope.error
    );
    assert_eq!(
        sw.daemon.state().intent(&proposal).unwrap().status,
        RegistryStatus::Accepted,
        "the privileged principal on the privileged path moves the contract"
    );
}

#[test]
fn baseline_authorized_lock_by_the_reviewer_mints_a_successor_and_supersedes() {
    let mut sw = swarm();
    reviewer_accepts(&mut sw);
    let base = sw.proposal.clone();

    // Tighten `fairness` from `unlocked` to `locked` — a legitimate governance revision.
    let locked = lock_as(
        &mut sw,
        "service:reviewer",
        "cap_reviewer",
        &base,
        (PolicyField::Fairness, PolicyVerb::Locked),
        "req_lock",
        "idem-lock",
    );
    assert_eq!(
        locked.envelope.status,
        ResultStatus::Ok,
        "{:?}",
        locked.envelope.error
    );
    let successor = match &locked.payload {
        Payload::IntentLock(response) => {
            assert_ne!(response.intent, base, "ID3 mints a successor contract");
            response.intent.clone()
        }
        other => panic!("expected an intent.lock payload, got {other:?}"),
    };
    // The predecessor is superseded and names its successor; the successor is accepted.
    assert_eq!(
        sw.daemon.state().intent(&base).unwrap().status,
        RegistryStatus::Superseded
    );
    assert_eq!(
        sw.daemon.state().intent(&successor).unwrap().status,
        RegistryStatus::Accepted
    );
    assert_eq!(
        sw.daemon
            .state()
            .intent(&successor)
            .unwrap()
            .contract
            .policy()
            .verb(PolicyField::Fairness),
        PolicyVerb::Locked
    );
}

#[test]
fn baseline_ordinary_evidence_work_flows_cleanly_end_to_end() {
    let mut sw = swarm();
    reviewer_accepts(&mut sw);
    let proof = sw.proof.clone();
    let receipt = sw.receipt.clone();

    // The prover appends an observation. A producer's append lands at the bottom.
    let node_handle = ingested_handle(&ingest_as(
        &mut sw,
        "agent:prover",
        "cap_prover",
        &proof,
        "req_ingest",
        "idem-ingest",
    ));
    assert_eq!(node(&sw, &node_handle).status(), ClaimStatus::BOTTOM);
    assert_eq!(node(&sw, &node_handle).producer.as_str(), "agent:prover");

    // The reviewer, as the trusted service, independently verifies it — and it promotes to
    // `observed`, under the *service's* identity, exactly as far as a re-derived reference
    // supports and no further.
    let verified = verify_as(
        &mut sw,
        "agent:reader",
        "cap_reader",
        &node_handle,
        Optional::Absent,
        "req_verify",
        "idem-verify",
    );
    assert_eq!(
        verified.envelope.status,
        ResultStatus::Ok,
        "{:?}",
        verified.envelope.error
    );
    assert_eq!(node(&sw, &node_handle).status(), ClaimStatus::Observed);
    let promoted = node(&sw, &node_handle);
    assert_eq!(
        promoted.history.last().unwrap().service_identity,
        None,
        "an `observed` promotion names no service in the node's own record (schema)",
    );

    // The reviewer records a genuine `CHECKED_BY` edge over the prover's node.
    let linked = link_as(
        &mut sw,
        "service:reviewer",
        "cap_reviewer",
        &node_handle,
        &receipt,
        "req_link",
        "idem-link",
    );
    assert_eq!(
        linked.envelope.status,
        ResultStatus::Ok,
        "{:?}",
        linked.envelope.error
    );
    let (edge_handle, checker) = match &linked.payload {
        Payload::EvidenceLink(response) => (response.edge.clone(), response.checker.clone()),
        other => panic!("expected an evidence.link payload, got {other:?}"),
    };
    assert_eq!(checker, "service:reviewer");
    assert_eq!(sw.daemon.state().evidence_edges().count(), 1);
    let edge = sw.daemon.state().evidence_edge(&edge_handle).expect("held");
    assert_eq!(edge.relation.kind().as_str(), "CHECKED_BY");
    assert_eq!(edge.from, node_handle);
}

// =========================================================================================
// Intent-authority channels (A1-A5)
// =========================================================================================

#[test]
fn attack_a1_a_producer_cannot_accept_a_proposal() {
    let mut sw = swarm();
    let before = fingerprint(&sw, &sw.proposal.clone());
    let proposal = sw.proposal.clone();

    let refused = accept_as(
        &mut sw,
        "agent:repairer",
        "cap_repairer",
        &proposal,
        "req_evil_accept",
        "idem-evil-accept",
    );
    assert_eq!(code(&refused), ErrorCode::CapabilityDenied);
    assert_eq!(
        sw.daemon.state().intent(&proposal).unwrap().status,
        RegistryStatus::Proposed,
        "a refused accept leaves the registry untouched"
    );
    assert_eq!(before, fingerprint(&sw, &proposal));
}

#[test]
fn attack_a2_the_planner_has_the_level_but_not_the_privilege_to_accept() {
    // The channel that isolates T3: the planner is at the `revise-intent` level — enough for
    // T1 — and is refused only because its profile grants no `intent.accept`. This is
    // docs/49's "revise intent: proposal only" cell, and RFC 0027's "privilege is not a
    // level".
    let mut sw = swarm();
    let proposal = sw.proposal.clone();
    let before = fingerprint(&sw, &proposal);

    let refused = accept_as(
        &mut sw,
        "agent:planner",
        "cap_planner",
        &proposal,
        "req_planner_accept",
        "idem-planner-accept",
    );
    assert_eq!(code(&refused), ErrorCode::CapabilityDenied);
    assert_eq!(before, fingerprint(&sw, &proposal));

    // The control that makes the refusal attributable: the reviewer, one privileged
    // operation richer at the same-or-higher level, accepts the identical proposal.
    let admitted = accept_as(
        &mut sw,
        "service:reviewer",
        "cap_reviewer",
        &proposal,
        "req_reviewer_accept",
        "idem-reviewer-accept",
    );
    assert_eq!(
        admitted.envelope.status,
        ResultStatus::Ok,
        "{:?}",
        admitted.envelope.error
    );
}

#[test]
fn attack_a3_a_producer_cannot_lock_a_field() {
    let mut sw = swarm();
    reviewer_accepts(&mut sw);
    let intent = sw.proposal.clone();
    let before = fingerprint(&sw, &intent);

    let refused = lock_as(
        &mut sw,
        "agent:repairer",
        "cap_repairer",
        &intent,
        (PolicyField::Fairness, PolicyVerb::Locked),
        "req_evil_lock",
        "idem-evil-lock",
    );
    assert_eq!(code(&refused), ErrorCode::CapabilityDenied);
    assert_eq!(before, fingerprint(&sw, &intent));
}

#[test]
fn attack_a4_a_proposal_onto_a_locked_field_is_refused_by_the_daemon() {
    // Admission passes — the planner has the level and propose_revision is not privileged —
    // so this is the daemon *itself* enforcing the lock, not the capability layer. `claims`
    // is the contract path of the `properties` policy key, which the Die Hard contract locks.
    let mut sw = swarm();
    reviewer_accepts(&mut sw);
    let intent = sw.proposal.clone();
    let before = fingerprint(&sw, &intent);

    let refused = propose_as(
        &mut sw,
        "agent:planner",
        "cap_planner",
        &intent,
        "claims",
        "req_propose_locked",
        "idem-propose-locked",
    );
    assert_eq!(code(&refused), ErrorCode::IntentMutationDenied);
    assert_eq!(
        before,
        fingerprint(&sw, &intent),
        "a refused proposal writes nothing to the registry"
    );
}

#[test]
fn attack_a5_a_producer_cannot_reject_a_proposal() {
    let mut sw = swarm();
    let proposal = sw.proposal.clone();
    let before = fingerprint(&sw, &proposal);

    let refused = reject_as(
        &mut sw,
        "agent:repairer",
        "cap_repairer",
        &proposal,
        "req_evil_reject",
        "idem-evil-reject",
    );
    assert_eq!(code(&refused), ErrorCode::CapabilityDenied);
    assert_eq!(
        sw.daemon.state().intent(&proposal).unwrap().status,
        RegistryStatus::Proposed,
        "a refused reject leaves the proposal alive",
    );
    assert_eq!(before, fingerprint(&sw, &proposal));
}

// =========================================================================================
// Evidence-graph authority channels (B1-B5)
// =========================================================================================

#[test]
fn attack_b1_a_non_service_principal_cannot_mint_a_check_edge() {
    let mut sw = swarm();
    let proof = sw.proof.clone();
    let receipt = sw.receipt.clone();
    // A real, held subject node, so the refusal is about the *checker* and not the subject.
    let subject = ingested_handle(&ingest_as(
        &mut sw,
        "agent:prover",
        "cap_prover",
        &proof,
        "req_ingest",
        "idem-ingest",
    ));
    let edges_before = sw.daemon.state().evidence_edges().count();

    // The repairer (an `agent:`) is refused at the actor scheme — RFC 0038 "Authority".
    let refused = link_as(
        &mut sw,
        "agent:repairer",
        "cap_repairer",
        &subject,
        &receipt,
        "req_evil_link",
        "idem-evil-link",
    );
    assert_eq!(code(&refused), ErrorCode::CapabilityDenied);
    assert_eq!(sw.daemon.state().evidence_edges().count(), edges_before);

    // The planner, at a *higher* level than execute, is refused for the same reason: the
    // gate is the scheme, not the ladder.
    let planner_refused = link_as(
        &mut sw,
        "agent:planner",
        "cap_planner",
        &subject,
        &receipt,
        "req_planner_link",
        "idem-planner-link",
    );
    assert_eq!(code(&planner_refused), ErrorCode::CapabilityDenied);
    assert_eq!(sw.daemon.state().evidence_edges().count(), edges_before);
}

#[test]
fn attack_b2_a_producer_cannot_name_or_forge_a_promoted_status() {
    // (a) The structural half: `observe.ingest`'s request has no field that could carry a
    // status, and `evidence.verify`'s only status input is the compare-and-set guard.
    let ingest_fields: Vec<&str> = registry::operation("observe.ingest")
        .expect("the registry declares it")
        .request
        .fields
        .iter()
        .map(|field| field.name)
        .collect();
    assert_eq!(ingest_fields, vec!["trace", "instrumentation_profile"]);
    let verify_fields: Vec<&str> = registry::operation("evidence.verify")
        .expect("the registry declares it")
        .request
        .fields
        .iter()
        .map(|field| field.name)
        .collect();
    assert_eq!(verify_fields, vec!["evidence", "expected_status"]);

    let mut sw = swarm();
    let patch = sw.patch.clone();

    // (b) The repairer's own append lands at `proposed`, whatever it wanted.
    let handle = ingested_handle(&ingest_as(
        &mut sw,
        "agent:repairer",
        "cap_repairer",
        &patch,
        "req_ingest",
        "idem-ingest",
    ));
    assert_eq!(node(&sw, &handle).status(), ClaimStatus::Proposed);

    // (c) A verify — which any reader may trigger — promotes only to `observed`, the ceiling
    // a re-derived reference supports, and the write is the *service's*, never the caller's.
    let verified = verify_as(
        &mut sw,
        "agent:reader",
        "cap_reader",
        &handle,
        Optional::Absent,
        "req_verify",
        "idem-verify",
    );
    match &verified.payload {
        Payload::EvidenceVerify(response) => {
            assert_eq!(response.status, EvidenceStatus::Observed);
            assert_eq!(
                response.checker, "service:reviewer",
                "the checker is the service, never the reader who asked",
            );
        }
        other => panic!("expected an evidence.verify payload, got {other:?}"),
    }
    assert_eq!(node(&sw, &handle).status(), ClaimStatus::Observed);

    // (d) Naming `validated` as the guard does not write it — the compare-and-set is lost
    // against the claim's real status, and nothing is promoted past `observed`.
    let forged = verify_as(
        &mut sw,
        "agent:reader",
        "cap_reader",
        &handle,
        Optional::Present(EvidenceStatus::Validated),
        "req_forge",
        "idem-forge",
    );
    assert_eq!(code(&forged), ErrorCode::StatusConflict);
    assert_eq!(
        node(&sw, &handle).status(),
        ClaimStatus::Observed,
        "a producer cannot lift its own claim past what the daemon's own check supports",
    );
}

#[test]
fn attack_b3_the_service_cannot_certify_its_own_production() {
    let mut sw = swarm();
    let review = sw.review.clone();
    let receipt = sw.receipt.clone();

    // The reviewer service is *also* a producer here: it ingests its own trace.
    let own = ingested_handle(&ingest_as(
        &mut sw,
        "service:reviewer",
        "cap_reviewer",
        &review,
        "req_self_ingest",
        "idem-self-ingest",
    ));
    assert_eq!(node(&sw, &own).producer.as_str(), "service:reviewer");

    // It may not verify it: the verification service is this node's producer (INV-004).
    let self_verify = verify_as(
        &mut sw,
        "agent:reader",
        "cap_reader",
        &own,
        Optional::Absent,
        "req_self_verify",
        "idem-self-verify",
    );
    assert_eq!(code(&self_verify), ErrorCode::InsufficientEvidence);
    assert_eq!(node(&sw, &own).status(), ClaimStatus::Proposed);

    // Nor record a check of it: `evidence.link` refuses when the checker is the subject's
    // own producer, before it reads the receipt.
    let self_link = link_as(
        &mut sw,
        "service:reviewer",
        "cap_reviewer",
        &own,
        &receipt,
        "req_self_link",
        "idem-self-link",
    );
    assert_eq!(code(&self_link), ErrorCode::InsufficientEvidence);
    assert_eq!(sw.daemon.state().evidence_edges().count(), 0);

    // The control: the *prover's* node — same daemon, same reviewer-as-checker — verifies
    // and links, so the refusals above are about who produced it and nothing else.
    let proof = sw.proof.clone();
    let others = ingested_handle(&ingest_as(
        &mut sw,
        "agent:prover",
        "cap_prover",
        &proof,
        "req_other_ingest",
        "idem-other-ingest",
    ));
    let ok_verify = verify_as(
        &mut sw,
        "agent:reader",
        "cap_reader",
        &others,
        Optional::Absent,
        "req_ok_verify",
        "idem-ok-verify",
    );
    assert_eq!(ok_verify.envelope.status, ResultStatus::Ok);
    let ok_link = link_as(
        &mut sw,
        "service:reviewer",
        "cap_reviewer",
        &others,
        &receipt,
        "req_ok_link",
        "idem-ok-link",
    );
    assert_eq!(
        ok_link.envelope.status,
        ResultStatus::Ok,
        "{:?}",
        ok_link.envelope.error
    );
}

#[test]
fn attack_b4_a_retargeted_or_misfiled_node_is_re_derived_and_refused() {
    let mut sw = swarm();
    let proof = sw.proof.clone();
    let honest_handle = ingested_handle(&ingest_as(
        &mut sw,
        "agent:prover",
        "cap_prover",
        &proof,
        "req_ingest",
        "idem-ingest",
    ));

    // (i) Misfile: the honest node filed under an identity it does not derive.
    let misfiled =
        EvidenceHandle::new("ev_not-the-identity-this-node-derives").expect("a well-formed handle");
    let honest_node = node(&sw, &honest_handle);
    sw.daemon
        .state_mut()
        .append_evidence(misfiled.clone(), honest_node);
    let rejected = verify_as(
        &mut sw,
        "agent:reader",
        "cap_reader",
        &misfiled,
        Optional::Absent,
        "req_misfiled",
        "idem-misfiled",
    );
    assert_eq!(code(&rejected), ErrorCode::CertificateRejected);
    assert_eq!(node(&sw, &misfiled).status(), ClaimStatus::Proposed);

    // (ii) Retarget: leave the node in place and point it at other, real, staged content.
    let mut retargeted = node(&sw, &honest_handle);
    retargeted.artifact = sw.patch.clone();
    let retargeted_handle = EvidenceHandle::new("ev_retargeted").expect("a well-formed handle");
    sw.daemon
        .state_mut()
        .append_evidence(retargeted_handle.clone(), retargeted);
    let retarget_rejected = verify_as(
        &mut sw,
        "agent:reader",
        "cap_reader",
        &retargeted_handle,
        Optional::Absent,
        "req_retarget",
        "idem-retarget",
    );
    assert_eq!(code(&retarget_rejected), ErrorCode::CertificateRejected);

    // The control: under the identity it *does* derive, the same node verifies — so the
    // refusals are the re-derivation catching the retarget, not a malformed request.
    let honest = verify_as(
        &mut sw,
        "agent:reader",
        "cap_reader",
        &honest_handle,
        Optional::Absent,
        "req_honest",
        "idem-honest",
    );
    assert_eq!(
        honest.envelope.status,
        ResultStatus::Ok,
        "{:?}",
        honest.envelope.error
    );
}

#[test]
fn attack_b5_replaying_an_authorized_edge_cannot_smuggle_different_content() {
    let mut sw = swarm();
    let proof = sw.proof.clone();
    let receipt = sw.receipt.clone();
    let other_receipt = sw.other_receipt.clone();
    let subject = ingested_handle(&ingest_as(
        &mut sw,
        "agent:prover",
        "cap_prover",
        &proof,
        "req_ingest",
        "idem-ingest",
    ));

    // The reviewer records the one authorized edge.
    let first = link_as(
        &mut sw,
        "service:reviewer",
        "cap_reviewer",
        &subject,
        &receipt,
        "req_link",
        "idem-link",
    );
    let (edge_one, receipt_one) = match &first.payload {
        Payload::EvidenceLink(response) => (response.edge.clone(), response.receipt.clone()),
        other => panic!("expected an evidence.link payload, got {other:?}"),
    };
    assert_eq!(sw.daemon.state().evidence_edges().count(), 1);
    let original_edge_bytes = sw
        .daemon
        .state()
        .evidence_edge(&edge_one)
        .expect("held")
        .clone();

    // (i) An identical replay under a *fresh* idempotency key converges on the one edge —
    // provenance is outside the edge identity (RFC 0038 D4), so a retry is idempotent by
    // content and cannot fork the graph.
    let replay = link_as(
        &mut sw,
        "service:reviewer",
        "cap_reviewer",
        &subject,
        &receipt,
        "req_link_again",
        "idem-link-again",
    );
    let (edge_two, receipt_two) = match &replay.payload {
        Payload::EvidenceLink(response) => (response.edge.clone(), response.receipt.clone()),
        other => panic!("expected an evidence.link payload, got {other:?}"),
    };
    assert_eq!((edge_one.clone(), receipt_one), (edge_two, receipt_two));
    assert_eq!(sw.daemon.state().evidence_edges().count(), 1);

    // (ii) The smuggle: replay the "same" check but with *different* receipt content. Because
    // an edge's identity is the content of what it asserts (RFC 0038 D2), this mints a
    // *distinct* edge — visible, attributed, additive — and cannot rewrite the authorized one.
    let smuggle = link_as(
        &mut sw,
        "service:reviewer",
        "cap_reviewer",
        &subject,
        &other_receipt,
        "req_smuggle",
        "idem-smuggle",
    );
    let edge_three = match &smuggle.payload {
        Payload::EvidenceLink(response) => response.edge.clone(),
        other => panic!("expected an evidence.link payload, got {other:?}"),
    };
    assert_ne!(
        edge_three, edge_one,
        "different asserted content is a different edge, never a silent rewrite",
    );
    assert_eq!(sw.daemon.state().evidence_edges().count(), 2);
    // The original edge's record is byte-for-byte what it was: nothing was smuggled into it.
    assert_eq!(
        &original_edge_bytes,
        sw.daemon.state().evidence_edge(&edge_one).expect("held"),
    );
}

// =========================================================================================
// Intent-status-alteration-via-evidence-writes channel (C1)
// =========================================================================================

#[test]
fn attack_c1_no_evidence_write_reaches_the_intent_registry() {
    // The channel a naive design leaves open: an evidence-graph write that flips an intent's
    // acceptance. It does not exist. The whole evidence workflow runs around a stable,
    // accepted contract, and the contract's status and bytes are byte-identical afterward —
    // the evidence graph and the intent registry are separate stores and no `evidence.*` or
    // `observe.*` operation names an intent status.
    let mut sw = swarm();
    reviewer_accepts(&mut sw);
    let intent = sw.proposal.clone();
    let before = fingerprint(&sw, &intent);
    let contract_before = before.3.clone();

    let proof = sw.proof.clone();
    let receipt = sw.receipt.clone();
    let subject = ingested_handle(&ingest_as(
        &mut sw,
        "agent:prover",
        "cap_prover",
        &proof,
        "req_ingest",
        "idem-ingest",
    ));
    let verified = verify_as(
        &mut sw,
        "agent:reader",
        "cap_reader",
        &subject,
        Optional::Absent,
        "req_verify",
        "idem-verify",
    );
    assert_eq!(verified.envelope.status, ResultStatus::Ok);
    let linked = link_as(
        &mut sw,
        "service:reviewer",
        "cap_reviewer",
        &subject,
        &receipt,
        "req_link",
        "idem-link",
    );
    assert_eq!(linked.envelope.status, ResultStatus::Ok);

    // The graph moved — a node promoted, an edge and a receipt appended — and the intent did
    // not: same status, same contract bytes.
    let record = sw.daemon.state().intent(&intent).unwrap();
    assert_eq!(record.status, RegistryStatus::Accepted);
    assert_eq!(record.contract.to_artifact_bytes(), contract_before);
}

// =========================================================================================
// Anti-vacuity: a disabled authority check flips the verdict, so the harness can fail
// =========================================================================================

#[test]
fn mutant_granting_the_planner_the_accept_privilege_lands_the_unauthorized_accept() {
    // The single difference from `attack_a2` is one privileged operation on the planner's
    // profile. If the T3 gate were fail-open, `attack_a2` would already have landed under the
    // tight profile — this test is the proof that only the gate stood between the planner and
    // the contract, and therefore that `attack_a2`'s refusal is load-bearing rather than
    // vacuous.
    let mut sw = swarm_with(true);
    let proposal = sw.proposal.clone();

    let landed = accept_as(
        &mut sw,
        "agent:planner",
        "cap_planner",
        &proposal,
        "req_mutant_accept",
        "idem-mutant-accept",
    );
    assert_eq!(
        landed.envelope.status,
        ResultStatus::Ok,
        "with the privilege granted, the very call attack_a2 refuses now lands: {:?}",
        landed.envelope.error
    );
    assert_eq!(
        sw.daemon.state().intent(&proposal).unwrap().status,
        RegistryStatus::Accepted,
        "the verdict flips — which is what makes attack_a2 a real test",
    );
}

#[test]
fn mutant_a_service_actor_lands_the_check_edge_the_agent_could_not() {
    // The B1 scheme gate, isolated. The identical link call attack_b1 refuses for an
    // `agent:` actor lands for a `service:` one, so a fail-open scheme check would land B1
    // and flip its verdict.
    let mut sw = swarm();
    let proof = sw.proof.clone();
    let receipt = sw.receipt.clone();
    let subject = ingested_handle(&ingest_as(
        &mut sw,
        "agent:prover",
        "cap_prover",
        &proof,
        "req_ingest",
        "idem-ingest",
    ));

    let landed = link_as(
        &mut sw,
        "service:reviewer",
        "cap_reviewer",
        &subject,
        &receipt,
        "req_service_link",
        "idem-service-link",
    );
    assert_eq!(
        landed.envelope.status,
        ResultStatus::Ok,
        "a `service:` actor lands the edge an `agent:` could not: {:?}",
        landed.envelope.error
    );
    assert_eq!(sw.daemon.state().evidence_edges().count(), 1);
}
