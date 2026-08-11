//! G1-07 acceptance — **authorization is checked independently of handle possession**.
//!
//! > authorization is checked independently of handle possession
//! >
//! > — `notes/plan/docs/52_RELEASE_GATES_REV3.md:22`, the G1 bullet list
//!
//! > The `capability` in the request envelope is checked below the adapter, independently
//! > of handle possession, before any semantic work runs. […] Possession of an artifact
//! > handle never implies authorization (ADR-0037).
//! >
//! > — `rule authorization.independent_of_handles`, the IDL
//!
//! > **H7 — possession of a stale handle never confers authority**, and losing authority
//! > never changes a handle's meaning. The two are independent: a handle names content, a
//! > capability authorizes acts (ADR-0037).
//! >
//! > — RFC 0027, "Handoff"
//!
//! # What this file is, and what it deliberately is not
//!
//! It is an **independent re-derivation** of the criterion, not a re-run of the suites that
//! delivered it. `notes/plan/notes/PHASE_A_EXIT_PACKAGE.md:763` records G1-07 as evidenced
//! by three existing files, and this one takes a deliberately different instrument to each
//! of them:
//!
//! | already-landed evidence | its method | why this file is not that |
//! |---|---|---|
//! | `inv015_agent_least_authority_evidence.rs` | source-text pins and registry-grain sweeps over the `@privileged` perimeter | nothing here reads a `src/` file as text; every claim is a *dispatch* or a call of `admission::admit` |
//! | `dx12_falsification.rs` | four swarm roles attacking *intent stability* through the intent-authority channels | the axis here is not intent: it is the **handle channel**, over every operation shape this build can decode |
//! | `g2_injection_corpus_evidence.rs` | forty-five hostile *payloads* through the byte wire, privileged operations only | the payload is held constant and the **handle** is varied; the sweep is over all thirty decodable operations, not the five privileged ones |
//!
//! The property under test is a *two-sided independence*, and testing only one side proves
//! nothing. A daemon that denied everything would pass a possession-without-authority
//! sweep; a daemon that allowed everything would pass the dual. Both directions are
//! measured here, against each other, with an in-test possession-based reference authorizer
//! ([`PossessionLedger`]) whose verdict must *disagree* with the daemon on every pinned
//! case — the negative control that would fire if authorization ever consulted possession.
//!
//! # The instrument, and why it can be uniform over thirty operations
//!
//! [`Daemon::dispatch`]'s step 5 records an [`AdmissionRecord`] for **every** request that
//! reaches it, whichever way the decision went, and step 6 (the annotation obligations) runs
//! *after* it. So a probe that omits `idempotency_key` and `budget` reaches the admission
//! decision for every one of the thirty decodable operations and then stops: an admitted
//! `@mutation` is refused `MalformedRequest` before its handler runs, and the daemon's state
//! is unchanged. That is what makes one fixture answer thirty operations × eight principals
//! without the probes interfering with each other, and it is why the *decision* rather than
//! the *effect* is what this file reads. Reading the ledger is also the only way to tell
//! "refused by admission" from "the handler holds no such record", because RFC 0027 X2
//! forbids a distinguishable not-found and both answer `CapabilityDenied`.
//!
//! Two cross-checks keep the ledger honest rather than assumed: every denial it reports is
//! confirmed as `CapabilityDenied` on the returned envelope, and every record's operation,
//! actor, and capability are checked against the request that produced it.
//!
//! # The six probes
//!
//! 1. [`possession_without_authority`] — a principal that legitimately *knows* a handle,
//!    because the daemon disclosed it in the answer to an authorized call, and lacks the
//!    authority for the operation. Swept over all thirty decodable operations on two
//!    independent authority axes (the level ladder, T1; the profile deny-list, T3).
//! 2. [`authority_without_possession`] — the dual. A principal with the authority,
//!    presenting a handle reconstructed from its **wire text** and never handed to it,
//!    must be admitted: an authorization that secretly required possession would deny here.
//! 3. [`confused_deputy`] — an unauthorized principal's chosen handle, carried inside an
//!    authorized principal's request. The decision must follow the *caller's* capability,
//!    and the unauthorized principal must gain nothing from having its handle honoured.
//! 4. [`ordering`] — authority revoked, and authority expired, *between* handle disclosure
//!    and handle use. The handle keeps its meaning as a name; it carries no authority
//!    across either event.
//! 5. [`decision_point`] — the mechanical core. For all thirty operations × eight
//!    principals, [`admission::admit`] returns the same answer against a state holding
//!    every fixture resource and against a state holding **none** of them. The decision is
//!    a function of the capability registry, the operation, and the actor; it cannot be a
//!    function of what exists or of who was told about it.
//! 6. [`negative_controls`] — [`PossessionLedger`], and a mutant sweep asserting the
//!    detector is not stuck.
//!
//! # What the spec makes handle-sensitive, and why that is not possession
//!
//! RFC 0027 T2 scopes a capability by "every snapshot, intent, and artifact class the
//! request names". So a decision *may* depend on which handle is named — when the
//! capability itself declares a snapshot or intent scope. [`declared_scope`] shows that
//! this dependence runs the opposite way from possession: a scoped capability is admitted
//! on an in-scope handle it was never given, and denied on an out-of-scope handle it was
//! explicitly handed. The handle-sensitivity is a property of the *capability*, not of the
//! caller's history.
//!
//! # Honest scope (INV-007)
//!
//! - **Thirty of the registry's seventy-five operations can be authorization-probed at
//!   all.** The other forty-five have no decodable request shape in this build, and step 3
//!   of the dispatch — shape agreement — precedes step 5, so no capability decision is ever
//!   reached for them. [`coverage::the_forty_five_unshaped_operations_never_reach_an_admission_decision`]
//!   asserts that mechanically rather than leaving it implied.
//! - **Seven handle classes are reachable through those thirty request bodies.** Six are
//!   minted live through the wire in this fixture (`ws_`, `in_`, `ev_`, `task_`, `cont_`,
//!   and the class-agnostic `ArtifactHandle` spelling of an `ev_`). The seventh, `ctx_`, has
//!   no minting path here — `context.compile` needs a registered compile projection this
//!   daemon holds none of — so its probes carry a wire-form handle. The
//!   possession-without-authority direction is unaffected; the authority-without-possession
//!   direction for `ctx_` is measured at admission only, which is where the criterion lives.
//! - **T2's instance scope covers two of the nineteen handle classes**: snapshots and
//!   intents. For `ev_`, `task_`, `cont_`, and `ctx_` the scope claim names the artifact
//!   *class* and never the instance, so no capability in this protocol version can be
//!   scoped to one evidence node or one task. That is recorded here as a measured fact
//!   ([`declared_scope::the_instance_scope_axis_covers_snapshots_and_intents_and_no_other_class`]),
//!   not as a defect: it is what the IDL's `CapabilityDescriptor` declares.
//!
//! # Falsification check: what this file would have caught
//!
//! An acceptance sweep that nothing could fail is a decoration. Two mutants were applied to
//! `daemon/admission.rs` during this bone's work — **temporarily, and reverted; neither is
//! committed and `src/` is unchanged** — and each was run against this file unmodified:
//!
//! | mutant | what it models | tests that failed |
//! |---|---|---|
//! | deny when a named snapshot is absent from the store | authorization coupled to *existence* | 3 of 22 |
//! | admit when a named snapshot resolves, before T1 | authorization coupled to *possession* — the handle confers authority | 9 of 22 |
//!
//! The second is the criterion's own negation, and it takes down the level sweep, the
//! deny-list sweep, both confused-deputy tests, both decision-point tests, the declared-scope
//! asymmetry, the stuck-detector control, and the one-denial control together. The first is
//! the weaker cousin — a daemon that merely *consulted* the store during admission — and the
//! two decision-point tests plus the wire-form probe catch it even though every refusal in
//! the sweep still looks right. Reproducing either is two lines in `admit`; the table above
//! records the outcome so a reader need not take the sweep's greenness on trust.
//!
//! # Verdict (INV-008)
//!
//! **SATISFIED-AT-NARROWER-SCOPE.** The criterion holds, without exception, over every
//! operation whose authorization can be reached in this build: thirty of seventy-five, seven
//! handle classes, eight principals, both directions of the independence, and four
//! ordering and confused-deputy shapes. No counterexample was found on any axis.
//!
//! The narrowing is the two scope bullets above, and it is a narrowing of *reach*, not of
//! confidence: the forty-five remaining operations are not weakly authorized — they are not
//! authorized at all, because no request naming one of them can get past shape agreement, so
//! there is nothing yet to check. When their bodies land, this file's
//! [`coverage::the_probe_table_is_exactly_the_shaped_operations_and_agrees_with_the_registry`]
//! fails, which is the intended way to find out.
//!
//! Two facts are recorded as measurements rather than as findings, because neither is a
//! violation and both are load-bearing for anyone reading the result: T2's instance scope
//! reaches two of the nineteen handle classes, and a `ctx_` handle has no minting path in
//! this build.
//!
//! House rules: `src/` untouched, no existing test edited, no new dependency, no clock, no
//! entropy, no filesystem, no network. Every fixture is a pure function of committed bytes.
//!
//! [`AdmissionRecord`]: continuumd::daemon::state::AdmissionRecord

use std::collections::{BTreeMap, BTreeSet};

use continuum_engine_reference::diehard;
use continuum_intent::canonical_json::Json;
use continuum_intent::contract::IntentContract;
use continuum_value::epoch::ProtocolWindow;
use continuum_workspace::snapshot::WorkspacePath;

use continuumd::daemon::admission;
use continuumd::daemon::context::ContextFamily;
use continuumd::daemon::evidence::EvidenceFamily;
use continuumd::daemon::family::{Arguments, Payload, ScopeClaim};
use continuumd::daemon::identity::Blake3Identity;
use continuumd::daemon::intent::IntentFamily;
use continuumd::daemon::observe::ObserveFamily;
use continuumd::daemon::state::{DaemonState, IntentRecord, RegistryStatus};
use continuumd::daemon::task::TaskFamily;
use continuumd::daemon::verification::{VerificationFamily, model_source};
use continuumd::daemon::whiteboard::WhiteboardFamily;
use continuumd::daemon::workspace::WorkspaceFamily;
use continuumd::daemon::{Daemon, OperationOutcome, OperationRequest};
use continuumd::protocol::envelope::{Budget, EpochSet, RequestEnvelope};
use continuumd::protocol::handshake::{
    CapabilityDescriptor, CapabilityProfile, ClientHello, Negotiated, VersionRange, negotiate,
};
use continuumd::protocol::operations::context::{ContextCompileRequest, ContextExpandRequest};
use continuumd::protocol::operations::evidence::{
    EvidenceGetRequest, EvidenceLinkRequest, EvidenceQueryRequest, EvidenceSubscribeRequest,
    EvidenceVerifyRequest,
};
use continuumd::protocol::operations::intent::{
    IntentAcceptRequest, IntentDiffRequest, IntentGetRequest, IntentLockRequest,
    IntentProposeRevisionRequest, IntentRejectRequest,
};
use continuumd::protocol::operations::observe::{
    ObserveClassifyRequest, ObserveIngestRequest, ObserveResultRequest,
};
use continuumd::protocol::operations::task::{
    TaskCancelRequest, TaskResumeRequest, TaskStatusRequest, TaskSubscribeRequest,
    TaskUpdateBudgetRequest,
};
use continuumd::protocol::operations::verification::{
    VerificationAwaitRequest, VerificationResultRequest, VerificationStartRequest,
};
use continuumd::protocol::operations::whiteboard::WhiteboardCompileRequest;
use continuumd::protocol::operations::workspace::{
    WorkspaceCreateByReferenceRequest, WorkspaceCreateRequest, WorkspaceDiffRequest,
    WorkspaceForkRequest, WorkspaceSealRequest,
};
use continuumd::protocol::registry::{self, ENCODINGS};
use continuumd::protocol::scalar::{
    ActorId, ArtifactHandle, CapabilityHandle, Commitment, ContextHandle, ContinuationHandle,
    EpochIdentity, EvidenceHandle, IntentHandle, Opaque, OperationName, ProtocolVersion, RequestId,
    TaskHandle, Timestamp, WorkspaceHandle,
};
use continuumd::protocol::shared::{
    EvidenceQuery, IntentChangeSet, SnapshotComponents, SnapshotEpochs, Target,
};
use continuumd::protocol::spec::{Nullable, Optional, Presence, StructSpec};
use continuumd::protocol::vocabulary::{
    AuthorityLevel, DataGrant, DiffLayer, Encoding, ErrorCode, ExpansionRelation, Portfolio,
    TargetKind,
};

// --- committed fixture bytes ---------------------------------------------------------------

/// The TV-009 port's model, verbatim — the bytes that go into the snapshot.
const DIE_HARD_MODEL: &str =
    include_str!("../../../notes/plan/corpus/tla-examples/ports/TV-009/DieHard.ctm");

/// The TV-009 port's default model configuration.
const DIE_HARD_CONFIG: &str =
    include_str!("../../../notes/plan/corpus/tla-examples/ports/TV-009/default.model.toml");

/// The Die Hard Intent Contract, as `continuum-intent`'s own suites use it.
const DIE_HARD_CONTRACT: &str =
    include_str!("../../continuum-intent/tests/fixtures/die-hard-contract.json");

/// Where the model lives inside the workspace.
const MODULE_PATH: &str = "DieHard.ctm";

/// A production trace, as `daemon_evidence.rs`'s producer ingests one.
const TRACE: &str = "{\"events\":[{\"at\":0,\"op\":\"fill\"},{\"at\":1,\"op\":\"pour\"}]}\n";

/// The instrumentation profile the trace above was captured under (plan §18.4).
const PROFILE: &str = "otel-1.0/sampled";

// --- the operations this build can decode ---------------------------------------------------

/// Every operation whose request body [`Arguments`] can carry.
///
/// The list is not a second registry: [`coverage`] checks each name against
/// [`registry::OPERATIONS`], checks that the probe table produces exactly these thirty
/// names through [`Arguments::operation`], and checks that every *other* registry
/// operation is undecodable — so a shape that lands later fails this file rather than
/// silently escaping the sweep.
const SHAPED: &[&str] = &[
    "workspace.create",
    "workspace.create_by_reference",
    "workspace.fork",
    "workspace.diff",
    "workspace.seal",
    "intent.get",
    "intent.diff",
    "intent.propose_revision",
    "intent.accept",
    "intent.reject",
    "intent.lock",
    "evidence.get",
    "evidence.query",
    "evidence.verify",
    "evidence.subscribe",
    "evidence.link",
    "observe.ingest",
    "observe.classify",
    "observe.result",
    "verification.start",
    "verification.result",
    "verification.await",
    "task.status",
    "task.cancel",
    "task.resume",
    "task.subscribe",
    "task.update_budget",
    "context.compile",
    "context.expand",
    "whiteboard.compile",
];

/// The handle class the IDL declares as a class-agnostic alias rather than a
/// [`registry::HANDLES`] member.
///
/// `ArtifactHandle` is `^[a-z][a-z0-9_]*_[A-Za-z0-9_-]+$` — "used where an operation accepts
/// more than one class" — so it is in [`registry::ALIASES`] and not in the handle table. It
/// still *names a resource*, which is the only property this criterion cares about, so the
/// census counts it beside the nineteen.
const CLASS_AGNOSTIC_HANDLE: &str = "ArtifactHandle";

// --- principals ------------------------------------------------------------------------------

/// One principal: an actor, the capability token it presents, and what that confers.
///
/// The pair is the whole of what admission is allowed to decide from (plus the operation),
/// which is why the probes carry it as one value: a probe that could vary the actor without
/// the capability would be testing a request the daemon refuses at T4 anyway.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Principal {
    actor: &'static str,
    capability: &'static str,
}

/// The connection capability, and the positive control for every operation.
const ROOT: Principal = Principal {
    actor: "service:continuumd",
    capability: "cap_root",
};

/// `read`, and nothing above it: below eighteen of the thirty shaped operations on the
/// ladder alone (T1).
const READER: Principal = Principal {
    actor: "agent:reader",
    capability: "cap_reader",
};

/// `promote` with every privilege — and a `denied_operations` list naming all thirty shaped
/// operations. The sharp instrument: it differs from [`ROOT`] in authority and in nothing
/// else, so a denial under it isolates T3 for operations the ladder cannot separate.
const DENIED: Principal = Principal {
    actor: "agent:denied",
    capability: "cap_denied",
};

/// The builder that creates the snapshots, at `propose`.
const BUILDER: Principal = Principal {
    actor: "agent:builder",
    capability: "cap_builder",
};

/// The producer that ingests the trace, at `execute` with the production-trace grant.
const PRODUCER: Principal = Principal {
    actor: "agent:producer",
    capability: "cap_producer",
};

/// The runner that starts the campaign, at `execute`.
const RUNNER: Principal = Principal {
    actor: "agent:runner",
    capability: "cap_runner",
};

/// The steward that accepts the contract, at `revise-intent` with the three intent
/// privileges.
const STEWARD: Principal = Principal {
    actor: "human:steward",
    capability: "cap_steward",
};

/// An `execute` principal that is told nothing by anybody. The authority-without-possession
/// probe runs as this one.
const STRANGER: Principal = Principal {
    actor: "agent:stranger",
    capability: "cap_stranger",
};

/// Registered so it can be revoked between a disclosure and a use.
const EPHEMERAL: Principal = Principal {
    actor: "agent:ephemeral",
    capability: "cap_ephemeral",
};

/// Expiring before the deployment's clock reading, so its authority is already gone when the
/// handle it was given is used.
const EXPIRED: Principal = Principal {
    actor: "agent:expired",
    capability: "cap_expired",
};

/// The eight principals the decision-point matrix runs over.
const MATRIX: &[Principal] = &[
    ROOT, READER, DENIED, BUILDER, PRODUCER, RUNNER, STEWARD, STRANGER,
];

/// The five `@privileged` operations, which is what a root capability may delegate.
const PRIVILEGED: &[&str] = &[
    "intent.accept",
    "intent.reject",
    "intent.lock",
    "repair.promote",
    "repair.reject",
];

// --- small constructors ----------------------------------------------------------------------

fn version() -> ProtocolVersion {
    ProtocolVersion::new(3, 1)
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

fn epoch(identity: &str) -> EpochIdentity {
    EpochIdentity::new(identity).expect("a well-formed epoch identity")
}

fn when(text: &str) -> Timestamp {
    Timestamp::new(text).expect("a well-formed timestamp")
}

/// The deployment's clock reading. Supplied, never ambient (INV-005, ADR-0003).
fn now() -> Timestamp {
    when("2026-08-01T00:00:00.000Z")
}

fn profile(privileged: &[&str], denied: &[&str], grants: &[DataGrant]) -> CapabilityProfile {
    CapabilityProfile {
        privileged_operations: privileged.iter().map(|entry| name(entry)).collect(),
        denied_operations: denied.iter().map(|entry| name(entry)).collect(),
        data_grants: grants.to_vec(),
        cross_principal_sharing: false,
    }
}

fn descriptor(
    principal: Principal,
    level: AuthorityLevel,
    depth: u32,
    profile: Optional<CapabilityProfile>,
) -> CapabilityDescriptor {
    CapabilityDescriptor {
        capability: cap(principal.capability),
        actor: who(principal.actor),
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
        client: "continuumd-gate-g1-07-acceptance".to_owned(),
        actor: who(ROOT.actor),
        capability: cap(ROOT.capability),
        features: Optional::Absent,
    };
    negotiate(&[version()], ProtocolWindow::new(3), ENCODINGS, &hello).expect("3.1 is served")
}

/// The epochs this daemon serves. Pinned, so a parked campaign has something to pin and a
/// `cont_*` handle exists to probe.
fn epochs() -> EpochSet {
    EpochSet {
        protocol: version(),
        semantic: Nullable::Value(epoch("semantic-1")),
        intent: Nullable::Value(epoch("intent-1")),
        evidence: Nullable::Null,
        proof: Nullable::Value(epoch("proof-1")),
        corpus: Nullable::Null,
        engine: Nullable::Value(epoch("engine-reference-1")),
    }
}

fn budget(states: Option<u64>) -> Budget {
    Budget {
        wall_ms: Optional::Absent,
        cpu_ms: Optional::Absent,
        memory_bytes: Optional::Absent,
        states: match states {
            Some(states) => Optional::Present(states),
            None => Optional::Absent,
        },
        solver_ms: Optional::Absent,
        proof_ms: Optional::Absent,
        tokens: Optional::Absent,
        candidates: Optional::Absent,
        bytes: Optional::Absent,
    }
}

/// A bare request envelope: no idempotency key, no budget, no pinned snapshot or intent.
///
/// Bareness is the instrument. Every `@mutation` and `@task_starting` operation is refused
/// at step 6 — *after* the admission decision of step 5 is taken and recorded — so a sweep
/// built on this envelope reaches the decision for all thirty operations and reaches no
/// handler that could change the daemon's state.
fn bare(operation: &str, principal: Principal, request: &str) -> RequestEnvelope {
    RequestEnvelope {
        protocol_version: version(),
        request_id: RequestId::new(request).expect("a well-formed request id"),
        idempotency_key: Optional::Absent,
        actor: who(principal.actor),
        capability: cap(principal.capability),
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

fn keyed(mut envelope: RequestEnvelope, key: &str) -> RequestEnvelope {
    envelope.idempotency_key = Optional::Present(key.to_owned());
    envelope
}

fn budgeted(mut envelope: RequestEnvelope, states: Option<u64>) -> RequestEnvelope {
    envelope.budget = Optional::Present(budget(states));
    envelope
}

fn on(mut envelope: RequestEnvelope, snapshot: &WorkspaceHandle) -> RequestEnvelope {
    envelope.snapshot = Nullable::Value(snapshot.clone());
    envelope
}

// --- the daemon and the handles it discloses -------------------------------------------------

/// The whole capability tree, as descriptor/parent pairs.
///
/// One list, two consumers. [`daemon`] provisions it through [`Daemon::builder`] — which is
/// what also registers the *store*-side subset a publication runs under — and
/// [`decision_point`] replays it into a second, resource-free [`DaemonState`]. Two spellings
/// of the tree would make that comparison a comparison of two fixtures rather than of two
/// states.
fn principals() -> Vec<(CapabilityDescriptor, Option<CapabilityHandle>)> {
    let root = Some(cap(ROOT.capability));
    let promote_all = || Optional::Present(profile(PRIVILEGED, &[], &[DataGrant::ProductionTrace]));
    vec![
        (
            descriptor(ROOT, AuthorityLevel::Promote, 5, promote_all()),
            None,
        ),
        (
            descriptor(READER, AuthorityLevel::Read, 4, Optional::Absent),
            root.clone(),
        ),
        (
            // `promote` with every privilege the root holds, and a deny-list naming all
            // thirty shaped operations: it differs from the root in authority alone.
            descriptor(
                DENIED,
                AuthorityLevel::Promote,
                4,
                Optional::Present(profile(PRIVILEGED, SHAPED, &[DataGrant::ProductionTrace])),
            ),
            root.clone(),
        ),
        (
            descriptor(BUILDER, AuthorityLevel::Propose, 4, Optional::Absent),
            root.clone(),
        ),
        (
            descriptor(
                PRODUCER,
                AuthorityLevel::Execute,
                4,
                Optional::Present(profile(&[], &[], &[DataGrant::ProductionTrace])),
            ),
            root.clone(),
        ),
        (
            descriptor(RUNNER, AuthorityLevel::Execute, 4, Optional::Absent),
            root.clone(),
        ),
        (
            descriptor(
                STEWARD,
                AuthorityLevel::ReviseIntent,
                4,
                Optional::Present(profile(
                    &["intent.accept", "intent.reject", "intent.lock"],
                    &[],
                    &[],
                )),
            ),
            root.clone(),
        ),
        (
            descriptor(STRANGER, AuthorityLevel::Promote, 4, promote_all()),
            root.clone(),
        ),
        (
            descriptor(EPHEMERAL, AuthorityLevel::Promote, 4, promote_all()),
            root.clone(),
        ),
        (
            {
                let mut expiring = descriptor(EXPIRED, AuthorityLevel::Promote, 4, promote_all());
                // Before `now()`, so this capability's authority is already gone when a probe
                // presents it — while the handles it was given remain perfectly good names.
                expiring.expires_at = Nullable::Value(when("2026-07-01T00:00:00.000Z"));
                expiring
            },
            root,
        ),
    ]
}

/// Replay [`principals`] into a bare [`DaemonState`].
fn register_principals(state: &mut DaemonState) {
    for (descriptor, parent) in principals() {
        state.register_capability(descriptor, parent);
    }
}

/// A daemon with all eight landed families registered and no resource in it yet.
fn daemon() -> Daemon {
    let mut builder = Daemon::builder(Blake3Identity, negotiated(), cap(ROOT.capability))
        .epochs(epochs())
        .now(now())
        .family(WorkspaceFamily)
        .family(IntentFamily)
        .family(EvidenceFamily::new())
        .family(ObserveFamily)
        .family(VerificationFamily)
        .family(TaskFamily)
        .family(ContextFamily)
        .family(WhiteboardFamily);
    for (descriptor, parent) in principals() {
        builder = builder.capability(descriptor, parent);
    }
    builder.build()
}

/// Every handle the fixture disclosed, and who it was disclosed to.
///
/// "Disclosed" is exact and is the whole point of the file: each of these — except the one
/// noted — is a value the daemon *returned* in the answer to an authorized call, so a
/// principal that made that call legitimately knows it. Nothing here is scraped out of
/// daemon-internal state.
struct Handles {
    /// `ws_*`, from `workspace.create` by [`BUILDER`].
    snapshot: WorkspaceHandle,
    /// A second `ws_*`, from a second `workspace.create` by [`BUILDER`].
    other_snapshot: WorkspaceHandle,
    /// `in_*`, from `intent.get` by [`READER`].
    intent: IntentHandle,
    /// `ev_*`, from `observe.ingest` by [`PRODUCER`].
    evidence: EvidenceHandle,
    /// `task_*`, from `verification.start` by [`RUNNER`].
    task: TaskHandle,
    /// `cont_*`, from `task.status` on the parked campaign.
    continuation: ContinuationHandle,
    /// `ctx_*`. **The one wire-form handle**: `context.compile` has no registered compile
    /// projection in this build, so no `ctx_*` is mintable here. See the module docs.
    context: ContextHandle,
    /// The staged trace's content identity, for `observe.ingest` and `evidence.link`.
    trace: Commitment,
    /// The registered component set's content identity, for
    /// `workspace.create_by_reference`.
    components: Commitment,
}

struct Fixture {
    daemon: Daemon,
    handles: Handles,
}

fn die_hard_contract() -> IntentContract {
    IntentContract::decode(DIE_HARD_CONTRACT.trim_end().as_bytes()).expect("the fixture decodes")
}

fn intent_handle(contract: &IntentContract) -> IntentHandle {
    let stored = continuum_workspace::publication::ContentIdentifier::identify(
        &Blake3Identity,
        continuum_workspace::artifact_path::ArtifactClass::IntentContract,
        &contract.identity_preimage_bytes(),
    )
    .expect("blake3 names every input");
    continuumd::daemon::identity::intent_to_wire(&stored).expect("an `in_` handle")
}

/// The content identity the daemon derives for a snapshot carrying only `DieHard.ctm`.
fn die_hard_source() -> Commitment {
    model_source(&Blake3Identity, [(MODULE_PATH, DIE_HARD_MODEL.as_bytes())])
        .expect("blake3 names the module set")
}

fn acceptance_bytes() -> Opaque {
    let mut fields: BTreeMap<String, Json> = BTreeMap::new();
    for (key, value) in [
        ("accepted_by", STEWARD.actor),
        ("capability", "revise-intent"),
        ("signature", "sig-die-hard-v1"),
        ("audit_record", "supplied-by-the-caller-and-overwritten"),
        ("timestamp", "2026-08-01T00:00:00.000Z"),
    ] {
        fields.insert(key.to_owned(), Json::String(value.to_owned()));
    }
    Opaque::from_bytes(Json::Object(fields).to_canonical_bytes())
}

fn components_of(files: Vec<Commitment>, intent: &IntentHandle) -> SnapshotComponents {
    SnapshotComponents {
        files,
        cml_modules: Vec::new(),
        rust_extraction: Vec::new(),
        domain_packs: Vec::new(),
        dependencies: Vec::new(),
        epochs: SnapshotEpochs {
            semantic: epoch("semantic-1"),
            proof: epoch("proof-1"),
            toolchain: Optional::Absent,
        },
        intent: intent.clone(),
        correspondence: Vec::new(),
        proof_environment: Vec::new(),
        configuration: Vec::new(),
        file_components: Optional::Absent,
    }
}

fn ok(outcome: &OperationOutcome, what: &str) {
    assert_eq!(
        outcome.error_code(),
        None,
        "the fixture's {what} must succeed: {:?}",
        outcome.envelope.error
    );
}

/// The daemon, its resources, and the handles it disclosed while building them.
fn fixture() -> Fixture {
    let mut daemon = daemon();
    let contract = die_hard_contract();
    let intent = intent_handle(&contract);
    daemon.state_mut().put_intent(
        intent.clone(),
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
    let module = stage(MODULE_PATH, DIE_HARD_MODEL);
    let readme = stage("README.md", "# TV-009\n");
    let other_readme = stage("OTHER.md", "# a second snapshot\n");
    let configuration = stage("default.model.toml", DIE_HARD_CONFIG);
    let trace = stage("traces/die-hard.jsonl", TRACE);

    daemon.state_mut().models_mut().register(
        die_hard_source(),
        diehard::model().expect("the port builds"),
    );

    // The steward accepts, so the contract governs a snapshot.
    let accepted = daemon.dispatch(&OperationRequest {
        envelope: keyed(
            bare("intent.accept", STEWARD, "req_fixture_accept"),
            "idem-fixture-accept",
        ),
        arguments: Arguments::IntentAccept(IntentAcceptRequest {
            proposal: intent.clone(),
            acceptance: acceptance_bytes(),
            bundle: Optional::Absent,
        }),
    });
    ok(&accepted, "intent.accept");

    // Disclosure 1: the builder creates a snapshot and is told its `ws_*`.
    let mut primary = components_of(vec![module.clone(), readme], &intent);
    primary.configuration = vec![configuration];
    let created = daemon.dispatch(&OperationRequest {
        envelope: keyed(
            bare("workspace.create", BUILDER, "req_fixture_create"),
            "idem-fixture-create",
        ),
        arguments: Arguments::WorkspaceCreate(WorkspaceCreateRequest {
            components: primary,
            overlay: Optional::Absent,
            seal: Optional::Present(true),
        }),
    });
    ok(&created, "workspace.create");
    let snapshot = match &created.payload {
        Payload::WorkspaceCreate(response) => response.snapshot.clone(),
        other => panic!("expected a workspace.create payload, got {other:?}"),
    };

    // Disclosure 2: a second snapshot, so "another principal's handle" is a real handle and
    // not a fiction.
    let secondary = components_of(vec![module, other_readme], &intent);
    let components = daemon
        .state_mut()
        .register_components(&Blake3Identity, secondary.clone())
        .expect("a component set has a content identity");
    let other = daemon.dispatch(&OperationRequest {
        envelope: keyed(
            bare("workspace.create", BUILDER, "req_fixture_create_other"),
            "idem-fixture-create-other",
        ),
        arguments: Arguments::WorkspaceCreate(WorkspaceCreateRequest {
            components: secondary,
            overlay: Optional::Absent,
            seal: Optional::Present(true),
        }),
    });
    ok(&other, "the second workspace.create");
    let other_snapshot = match &other.payload {
        Payload::WorkspaceCreate(response) => response.snapshot.clone(),
        other => panic!("expected a workspace.create payload, got {other:?}"),
    };

    // Disclosure 3: the reader asks for the contract and is told its `in_*` back.
    let got = daemon.dispatch(&OperationRequest {
        envelope: bare("intent.get", READER, "req_fixture_get"),
        arguments: Arguments::IntentGet(IntentGetRequest {
            intent: intent.clone(),
        }),
    });
    ok(&got, "intent.get");
    let disclosed_intent = match &got.payload {
        Payload::IntentGet(response) => response.intent.clone(),
        other => panic!("expected an intent.get payload, got {other:?}"),
    };
    assert_eq!(
        disclosed_intent, intent,
        "the reader is told the same `in_` the fixture registered"
    );

    // Disclosure 4: the producer ingests the trace and is told the `ev_*` it appended.
    let ingested = daemon.dispatch(&OperationRequest {
        envelope: budgeted(
            keyed(
                bare("observe.ingest", PRODUCER, "req_fixture_ingest"),
                "idem-fixture-ingest",
            ),
            None,
        ),
        arguments: Arguments::ObserveIngest(ObserveIngestRequest {
            trace: trace.clone(),
            instrumentation_profile: PROFILE.to_owned(),
        }),
    });
    ok(&ingested, "observe.ingest");
    let evidence = match &ingested.payload {
        Payload::ObserveIngest(response) => response
            .evidence
            .first()
            .cloned()
            .expect("an ingest names the node it appended"),
        other => panic!("expected an observe.ingest payload, got {other:?}"),
    };

    // Disclosure 5: the runner starts a bounded campaign and is told its `task_*`. The bound
    // parks it, so a `cont_*` exists too.
    let started = daemon.dispatch(&OperationRequest {
        envelope: on(
            budgeted(
                keyed(
                    bare("verification.start", RUNNER, "req_fixture_start"),
                    "idem-fixture-start",
                ),
                Some(4),
            ),
            &snapshot,
        ),
        arguments: Arguments::VerificationStart(VerificationStartRequest {
            target: Target {
                kind: TargetKind::AllClaims,
                id: "DieHard".to_owned(),
            },
            portfolio: Portfolio::Interactive,
            context_policy: Optional::Absent,
            priority_class: Optional::Absent,
        }),
    });
    ok(&started, "verification.start");
    let task = match &started.payload {
        Payload::VerificationStart(response) => response
            .task
            .value()
            .cloned()
            .expect("a fresh start names a task"),
        other => panic!("expected a verification.start payload, got {other:?}"),
    };

    // Disclosure 6: `task.status` on the parked campaign hands out the continuation.
    let parked = daemon.dispatch(&OperationRequest {
        envelope: bare("task.status", RUNNER, "req_fixture_status"),
        arguments: Arguments::TaskStatus(TaskStatusRequest { task: task.clone() }),
    });
    ok(&parked, "task.status");
    let continuation = match &parked.payload {
        Payload::TaskStatus(record) => record
            .continuation
            .value()
            .cloned()
            .expect("a bounded campaign parks with a continuation"),
        other => panic!("expected a task.status payload, got {other:?}"),
    };

    Fixture {
        daemon,
        handles: Handles {
            snapshot,
            other_snapshot,
            intent,
            evidence,
            task,
            continuation,
            // No minting path in this build; see the module documentation's scope note.
            context: ContextHandle::new("ctx_wire_form_only").expect("a `ctx_` handle"),
            trace,
            components,
        },
    }
}

// --- the probe table -------------------------------------------------------------------------

/// One operation, its arguments, and every handle those arguments name.
struct Probe {
    operation: &'static str,
    arguments: Arguments,
    /// The handle classes the arguments carry, as [`registry::HANDLES`] spells them.
    classes: BTreeSet<&'static str>,
}

fn probe(operation: &'static str, arguments: Arguments, classes: &[&'static str]) -> Probe {
    Probe {
        operation,
        arguments,
        classes: classes.iter().copied().collect(),
    }
}

fn evidence_query(root: &EvidenceHandle) -> EvidenceQuery {
    EvidenceQuery {
        node_kinds: Optional::Absent,
        edge_kinds: Optional::Absent,
        statuses: Optional::Absent,
        claim_id: Optional::Absent,
        roots: Optional::Present(vec![root.clone()]),
        max_depth: Optional::Absent,
    }
}

fn change_set() -> IntentChangeSet {
    IntentChangeSet {
        changes: Opaque::from_bytes(Json::Object(BTreeMap::new()).to_canonical_bytes()),
        rationale: "a typed rationale for reviewers".to_owned(),
    }
}

/// The thirty probes, every handle-bearing field filled from `handles`.
///
/// The arguments are *well-formed values of the declared shape*, which is all the criterion
/// needs: the decision under test is taken at step 5, and steps 1–4 only require that the
/// envelope names the negotiated version, a registry operation, and a body of that
/// operation's shape.
fn probes(handles: &Handles) -> Vec<Probe> {
    let artifact_root =
        ArtifactHandle::new(handles.evidence.as_str()).expect("an `ev_` is an artifact handle");
    vec![
        probe(
            "workspace.create",
            Arguments::WorkspaceCreate(WorkspaceCreateRequest {
                components: components_of(Vec::new(), &handles.intent),
                overlay: Optional::Absent,
                seal: Optional::Absent,
            }),
            &["IntentHandle"],
        ),
        probe(
            "workspace.create_by_reference",
            Arguments::WorkspaceCreateByReference(WorkspaceCreateByReferenceRequest {
                components: handles.components.clone(),
                epochs: SnapshotEpochs {
                    semantic: epoch("semantic-1"),
                    proof: epoch("proof-1"),
                    toolchain: Optional::Absent,
                },
                intent: handles.intent.clone(),
                seal: Optional::Absent,
            }),
            &["IntentHandle"],
        ),
        probe(
            "workspace.fork",
            Arguments::WorkspaceFork(WorkspaceForkRequest {
                base: handles.snapshot.clone(),
                overlay: Optional::Absent,
                patches: Optional::Absent,
            }),
            &["WorkspaceHandle"],
        ),
        probe(
            "workspace.diff",
            Arguments::WorkspaceDiff(WorkspaceDiffRequest {
                before: handles.snapshot.clone(),
                after: handles.other_snapshot.clone(),
                layers: vec![DiffLayer::Textual],
            }),
            &["WorkspaceHandle"],
        ),
        probe(
            "workspace.seal",
            Arguments::WorkspaceSeal(WorkspaceSealRequest {
                snapshot: handles.snapshot.clone(),
            }),
            &["WorkspaceHandle"],
        ),
        probe(
            "intent.get",
            Arguments::IntentGet(IntentGetRequest {
                intent: handles.intent.clone(),
            }),
            &["IntentHandle"],
        ),
        probe(
            "intent.diff",
            Arguments::IntentDiff(IntentDiffRequest {
                before: handles.intent.clone(),
                after: handles.intent.clone(),
            }),
            &["IntentHandle"],
        ),
        probe(
            "intent.propose_revision",
            Arguments::IntentProposeRevision(IntentProposeRevisionRequest {
                base: handles.intent.clone(),
                changes: change_set(),
            }),
            &["IntentHandle"],
        ),
        probe(
            "intent.accept",
            Arguments::IntentAccept(IntentAcceptRequest {
                proposal: handles.intent.clone(),
                acceptance: acceptance_bytes(),
                bundle: Optional::Absent,
            }),
            &["IntentHandle"],
        ),
        probe(
            "intent.reject",
            Arguments::IntentReject(IntentRejectRequest {
                proposal: handles.intent.clone(),
                reason: "a typed reason".to_owned(),
            }),
            &["IntentHandle"],
        ),
        probe(
            "intent.lock",
            Arguments::IntentLock(IntentLockRequest {
                intent: handles.intent.clone(),
                policy: BTreeMap::new(),
            }),
            &["IntentHandle"],
        ),
        probe(
            "evidence.get",
            Arguments::EvidenceGet(EvidenceGetRequest {
                evidence: handles.evidence.clone(),
                inline: Optional::Absent,
            }),
            &["EvidenceHandle"],
        ),
        probe(
            "evidence.query",
            Arguments::EvidenceQuery(EvidenceQueryRequest {
                query: evidence_query(&handles.evidence),
            }),
            &["EvidenceHandle"],
        ),
        probe(
            "evidence.verify",
            Arguments::EvidenceVerify(EvidenceVerifyRequest {
                evidence: handles.evidence.clone(),
                expected_status: Optional::Absent,
            }),
            &["EvidenceHandle"],
        ),
        probe(
            "evidence.subscribe",
            Arguments::EvidenceSubscribe(EvidenceSubscribeRequest {
                scope: evidence_query(&handles.evidence),
            }),
            &["EvidenceHandle"],
        ),
        probe(
            "evidence.link",
            Arguments::EvidenceLink(EvidenceLinkRequest {
                subject: handles.evidence.clone(),
                receipt: handles.trace.clone(),
                checker_profile: "kernel-core/1".to_owned(),
            }),
            &["EvidenceHandle"],
        ),
        probe(
            "observe.ingest",
            Arguments::ObserveIngest(ObserveIngestRequest {
                trace: handles.trace.clone(),
                instrumentation_profile: PROFILE.to_owned(),
            }),
            &[],
        ),
        probe(
            "observe.classify",
            Arguments::ObserveClassify(ObserveClassifyRequest {
                evidence: handles.evidence.clone(),
            }),
            &["EvidenceHandle"],
        ),
        probe(
            "observe.result",
            Arguments::ObserveResult(ObserveResultRequest {
                evidence: handles.evidence.clone(),
            }),
            &["EvidenceHandle"],
        ),
        probe(
            "verification.start",
            Arguments::VerificationStart(VerificationStartRequest {
                target: Target {
                    kind: TargetKind::AllClaims,
                    id: "DieHard".to_owned(),
                },
                portfolio: Portfolio::Interactive,
                context_policy: Optional::Absent,
                priority_class: Optional::Absent,
            }),
            &[],
        ),
        probe(
            "verification.result",
            Arguments::VerificationResult(VerificationResultRequest {
                task: handles.task.clone(),
            }),
            &["TaskHandle"],
        ),
        probe(
            "verification.await",
            Arguments::VerificationAwait(VerificationAwaitRequest {
                task: handles.task.clone(),
                timeout_ms: Optional::Absent,
            }),
            &["TaskHandle"],
        ),
        probe(
            "task.status",
            Arguments::TaskStatus(TaskStatusRequest {
                task: handles.task.clone(),
            }),
            &["TaskHandle"],
        ),
        probe(
            "task.cancel",
            Arguments::TaskCancel(TaskCancelRequest {
                task: handles.task.clone(),
            }),
            &["TaskHandle"],
        ),
        probe(
            "task.resume",
            Arguments::TaskResume(TaskResumeRequest {
                continuation: handles.continuation.clone(),
                budget: Optional::Absent,
            }),
            &["ContinuationHandle"],
        ),
        probe(
            "task.subscribe",
            Arguments::TaskSubscribe(TaskSubscribeRequest {
                task: handles.task.clone(),
            }),
            &["TaskHandle"],
        ),
        probe(
            "task.update_budget",
            Arguments::TaskUpdateBudget(TaskUpdateBudgetRequest {
                task: handles.task.clone(),
                budget: budget(Some(64)),
            }),
            &["TaskHandle"],
        ),
        probe(
            "context.compile",
            Arguments::ContextCompile(ContextCompileRequest {
                evidence_root: artifact_root,
                question: "what does this evidence support?".to_owned(),
                audience: Optional::Absent,
                guarantees: Optional::Absent,
            }),
            &[CLASS_AGNOSTIC_HANDLE],
        ),
        probe(
            "context.expand",
            Arguments::ContextExpand(ContextExpandRequest {
                context: handles.context.clone(),
                anchor: "anchor-1".to_owned(),
                relation: ExpansionRelation::CausalPredecessors,
                depth: Optional::Absent,
            }),
            &["ContextHandle"],
        ),
        probe(
            "whiteboard.compile",
            Arguments::WhiteboardCompile(WhiteboardCompileRequest {
                note: Opaque::from_bytes(Json::Object(BTreeMap::new()).to_canonical_bytes()),
            }),
            &[],
        ),
    ]
}

// --- reading the decision ---------------------------------------------------------------------

/// The admission decision one dispatch took, read from the ledger RFC 0027 P5 requires and
/// cross-checked against the envelope the caller got.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Decision {
    admitted: bool,
    code: Option<ErrorCode>,
}

/// Dispatch one probe as `principal` and report the decision.
///
/// The ledger is not trusted on its own word. Every record is checked to name this request's
/// operation, actor, and capability, and every *denial* it reports is checked to have
/// produced `CapabilityDenied` on the wire. The converse is deliberately not asserted: a
/// handler may answer `CapabilityDenied` for a record the daemon does not hold, because
/// RFC 0027 X2 forbids a distinguishable not-found — which is exactly why this file reads
/// the ledger instead of the code.
fn decide(
    daemon: &mut Daemon,
    probe: &Probe,
    principal: Principal,
    request: &str,
    envelope: impl FnOnce(RequestEnvelope) -> RequestEnvelope,
) -> Decision {
    let before = daemon.state().admissions().len();
    let outcome = daemon.dispatch(&OperationRequest {
        envelope: envelope(bare(probe.operation, principal, request)),
        arguments: probe.arguments.clone(),
    });
    let records = daemon.state().admissions();
    assert_eq!(
        records.len(),
        before + 1,
        "{} reached the admission decision exactly once",
        probe.operation
    );
    let record = records.last().expect("the record just written");
    assert_eq!(record.operation, probe.operation);
    assert_eq!(record.actor, principal.actor);
    assert_eq!(record.capability, cap(principal.capability));
    let code = outcome.error_code();
    if !record.admitted {
        assert_eq!(
            code,
            Some(ErrorCode::CapabilityDenied),
            "{} refused at admission answers the one denial",
            probe.operation
        );
    }
    Decision {
        admitted: record.admitted,
        code,
    }
}

/// The same dispatch with the envelope left bare.
fn decide_bare(
    daemon: &mut Daemon,
    probe: &Probe,
    principal: Principal,
    request: &str,
) -> Decision {
    decide(daemon, probe, principal, request, |envelope| envelope)
}

fn spec_of(operation: &str) -> &'static continuumd::protocol::spec::OperationSpec {
    registry::operation(operation).expect("a registry operation")
}

// --- coverage, counted against the IDL ---------------------------------------------------------

mod coverage {
    use super::{
        Arguments, BTreeSet, CLASS_AGNOSTIC_HANDLE, Presence, SHAPED, StructSpec,
        TaskStatusRequest, fixture, name, probes, registry, spec_of,
    };
    use continuumd::daemon::OperationRequest;
    use continuumd::protocol::vocabulary::ErrorCode;

    /// The handle-class names the IDL declares, plus the class-agnostic alias.
    fn handle_names() -> BTreeSet<&'static str> {
        registry::HANDLES
            .iter()
            .map(|handle| handle.name)
            .chain([CLASS_AGNOSTIC_HANDLE])
            .collect()
    }

    /// Resolve an IDL type spelling down to its element type.
    fn element(ty: &'static str) -> &'static str {
        if let Some(inner) = ty.strip_prefix("list<").and_then(|ty| ty.strip_suffix('>')) {
            return element(inner);
        }
        if let Some(inner) = ty
            .strip_prefix("map<String,")
            .and_then(|ty| ty.strip_suffix('>'))
        {
            return element(inner);
        }
        ty
    }

    fn named(ty: &str) -> Option<&'static StructSpec> {
        registry::NAMED_STRUCTS
            .iter()
            .find(|entry| entry.name == ty)
    }

    /// Every handle class a request body names in a field the IDL declares `required` or
    /// `nullable`, resolved through the named structs it nests.
    ///
    /// `optional` fields are excluded on purpose: a caller may omit one, so a probe that
    /// filled it would be claiming coverage the IDL does not oblige.
    fn required_classes(
        body: &StructSpec,
        seen: &mut BTreeSet<&'static str>,
    ) -> BTreeSet<&'static str> {
        let handles = handle_names();
        let mut found = BTreeSet::new();
        for field in body.fields {
            if field.presence == Presence::Optional {
                continue;
            }
            let ty = element(field.ty);
            if handles.contains(ty) {
                found.insert(ty);
            } else if let Some(nested) = named(ty) {
                if seen.insert(nested.name) {
                    found.extend(required_classes(nested, seen));
                }
            }
        }
        found
    }

    #[test]
    fn the_probe_table_is_exactly_the_shaped_operations_and_agrees_with_the_registry() {
        let fixture = fixture();
        let probes = probes(&fixture.handles);
        let names: Vec<&str> = probes.iter().map(|probe| probe.operation).collect();
        assert_eq!(
            names, SHAPED,
            "the probe table is the shaped list, in order and without duplicates"
        );
        for probe in &probes {
            assert_eq!(
                probe.arguments.operation(),
                probe.operation,
                "a probe's decoded body is its own operation's shape"
            );
            assert!(
                registry::operation(probe.operation).is_some(),
                "{} is a registry operation",
                probe.operation
            );
        }
        assert_eq!(
            probes.len(),
            30,
            "thirty of the registry's operations have a decodable request body"
        );
        assert_eq!(
            registry::OPERATIONS.len(),
            registry::OPERATION_COUNT,
            "the registry's own count"
        );
    }

    #[test]
    fn every_handle_class_the_idl_requires_of_a_shaped_operation_is_carried_by_its_probe() {
        let fixture = fixture();
        let mut union: BTreeSet<&'static str> = BTreeSet::new();
        let mut carried: BTreeSet<&'static str> = BTreeSet::new();
        for probe in probes(&fixture.handles) {
            let spec = spec_of(probe.operation);
            let mut seen = BTreeSet::new();
            let required = required_classes(&spec.request, &mut seen);
            assert!(
                required.is_subset(&probe.classes),
                "{}: the IDL requires handle classes {required:?}, the probe carries {:?}",
                probe.operation,
                probe.classes
            );
            union.extend(required);
            carried.extend(probe.classes);
        }
        // The census, stated as a value rather than a claim: these are the handle classes a
        // caller *must* name to reach one of the thirty admission decisions, and every one
        // of them is filled from a handle the fixture disclosed.
        let expected: BTreeSet<&str> = [
            "ContextHandle",
            "ContinuationHandle",
            "EvidenceHandle",
            "IntentHandle",
            "TaskHandle",
            "WorkspaceHandle",
            CLASS_AGNOSTIC_HANDLE,
        ]
        .into_iter()
        .collect();
        assert_eq!(
            union, expected,
            "the shaped operations require exactly these handle classes"
        );
        assert_eq!(
            carried, expected,
            "and the probes carry exactly those and no invented extras"
        );
    }

    #[test]
    fn the_forty_five_unshaped_operations_never_reach_an_admission_decision() {
        let mut fixture = fixture();
        let shaped: BTreeSet<&str> = SHAPED.iter().copied().collect();
        // A well-formed body of *some* operation. Step 3 compares it against the envelope's
        // operation name, so for every unshaped operation the two disagree.
        let stand_in = Arguments::TaskStatus(TaskStatusRequest {
            task: fixture.handles.task.clone(),
        });
        let mut probed = 0_usize;
        for spec in registry::OPERATIONS {
            if shaped.contains(spec.name) {
                continue;
            }
            probed += 1;
            let before = fixture.daemon.state().admissions().len();
            let mut envelope = super::bare(spec.name, super::ROOT, "req_unshaped");
            envelope.operation = name(spec.name);
            let outcome = fixture.daemon.dispatch(&OperationRequest {
                envelope,
                arguments: stand_in.clone(),
            });
            assert_eq!(
                outcome.error_code(),
                Some(ErrorCode::MalformedRequest),
                "{} has no decodable body in this build",
                spec.name
            );
            assert_eq!(
                fixture.daemon.state().admissions().len(),
                before,
                "{}: shape agreement precedes admission, so no decision is taken",
                spec.name
            );
        }
        assert_eq!(
            probed, 45,
            "forty-five of the seventy-five operations are unshaped in this build"
        );
    }
}

// --- probe 1: possession without authority ------------------------------------------------------

mod possession_without_authority {
    use super::{
        AuthorityLevel, DENIED, Decision, MATRIX, READER, ROOT, SHAPED, decide_bare, fixture,
        probes, spec_of,
    };

    /// The authority levels strictly above `read`, which is what makes the ladder axis
    /// applicable to an operation.
    fn above_read(level: AuthorityLevel) -> bool {
        !matches!(level, AuthorityLevel::Read)
    }

    /// Every shaped operation, probed by a principal that legitimately holds every handle in
    /// the request and sits below the operation on the ladder.
    ///
    /// The reader was *told* the `in_` (it called `intent.get`), and the `ws_`, `ev_`,
    /// `task_` and `cont_` are values the daemon published in answers to authorized calls.
    /// Knowing them buys nothing.
    #[test]
    fn the_level_ladder_refuses_a_holder_of_every_named_handle_on_eighteen_operations() {
        let mut fixture = fixture();
        let probes = probes(&fixture.handles);
        let mut refused = 0_usize;
        for (index, probe) in probes.iter().enumerate() {
            let spec = spec_of(probe.operation);
            if !above_read(spec.authority) {
                continue;
            }
            let request = format!("req_ladder_{index}");
            let decision = decide_bare(&mut fixture.daemon, probe, READER, &request);
            assert_eq!(
                decision,
                Decision {
                    admitted: false,
                    code: Some(super::ErrorCode::CapabilityDenied),
                },
                "{}: a `read` capability holding every named handle is refused",
                probe.operation
            );
            refused += 1;
        }
        assert_eq!(
            refused, 18,
            "eighteen of the thirty shaped operations sit above `read`"
        );
    }

    /// The other twelve — the `read`-floor operations the ladder cannot separate — refused on
    /// the privilege axis instead.
    ///
    /// `cap_denied` is `promote` with every privilege the root holds. It differs from the
    /// root in exactly one thing: its profile denies these operations. So a denial here is
    /// T3 and nothing else, and it reaches every shaped operation rather than eighteen.
    #[test]
    fn the_profile_deny_list_refuses_a_holder_of_every_named_handle_on_all_thirty() {
        let mut fixture = fixture();
        let probes = probes(&fixture.handles);
        for (index, probe) in probes.iter().enumerate() {
            let request = format!("req_deny_{index}");
            let decision = decide_bare(&mut fixture.daemon, probe, DENIED, &request);
            assert!(
                !decision.admitted,
                "{}: a denied operation is refused however much the caller holds",
                probe.operation
            );
        }
        assert_eq!(probes.len(), SHAPED.len());
    }

    /// The positive control the two sweeps above need in order to mean anything.
    ///
    /// The same thirty requests, the same handles, byte for byte — under the connection
    /// capability. Every one is admitted, so the refusals above are a property of the
    /// *capability* and not of the arguments, the handles, or the fixture's state.
    #[test]
    fn the_identical_thirty_requests_are_admitted_under_the_root_capability() {
        let mut fixture = fixture();
        let probes = probes(&fixture.handles);
        for (index, probe) in probes.iter().enumerate() {
            let request = format!("req_control_{index}");
            let decision = decide_bare(&mut fixture.daemon, probe, ROOT, &request);
            assert!(
                decision.admitted,
                "{}: the control must be admitted, or the sweep proves nothing",
                probe.operation
            );
        }
        assert_eq!(MATRIX.len(), 8);
    }

    /// The deny-list principal is not simply broken.
    ///
    /// Every refusal above would look the same if `cap_denied` were unregistered, or a
    /// mis-provisioned delegation the chain walk rejects. It is neither: for an operation
    /// **outside** its deny list the same capability is admitted, so the thirty refusals are
    /// the profile's doing and nothing else. `model.check` is used because it is one of the
    /// forty-five the deny list does not name; it is unreachable through `dispatch` (no
    /// decodable body), so the predicate is called directly.
    #[test]
    fn the_deny_list_principal_is_admitted_for_an_operation_its_profile_does_not_name() {
        let fixture = fixture();
        let outside = spec_of("model.check");
        assert!(
            !SHAPED.contains(&outside.name),
            "the control operation must be outside the deny list"
        );
        let envelope = super::bare(outside.name, DENIED, "req_outside");
        let admitted = super::admission::admit(
            outside,
            &envelope,
            &super::ScopeClaim::default(),
            fixture.daemon.state(),
            &super::cap(ROOT.capability),
            Some(&super::now()),
        )
        .is_ok();
        assert!(
            admitted,
            "`cap_denied` is a live, narrowing delegation at `promote`"
        );
    }

    /// The data-grant axis, which is neither level nor privilege.
    ///
    /// `observe.ingest` requires `production_trace` beyond its `execute` level (RFC 0027
    /// R-4). The runner is at `execute` and holds the trace commitment the request names —
    /// the strongest form of "possession" available for a content identity — and is refused.
    #[test]
    fn a_data_grant_the_capability_lacks_refuses_a_caller_holding_the_named_content() {
        let mut fixture = fixture();
        let probes = probes(&fixture.handles);
        let ingest = probes
            .iter()
            .find(|probe| probe.operation == "observe.ingest")
            .expect("observe.ingest is shaped");
        let refused = decide_bare(&mut fixture.daemon, ingest, super::RUNNER, "req_grant_no");
        assert!(
            !refused.admitted,
            "`execute` without the production-trace grant is refused"
        );
        let admitted = decide_bare(
            &mut fixture.daemon,
            ingest,
            super::PRODUCER,
            "req_grant_yes",
        );
        assert!(
            admitted.admitted,
            "the same request, the same commitment, a capability that carries the grant"
        );
    }
}

// --- probe 2: authority without possession --------------------------------------------------------

mod authority_without_possession {
    use super::{
        Arguments, ContinuationHandle, EvidenceHandle, IntentHandle, OperationRequest, Payload,
        ROOT, STRANGER, TaskHandle, WorkspaceHandle, bare, decide_bare, fixture, keyed, probes,
    };
    use continuumd::protocol::operations::workspace::WorkspaceForkRequest;
    use continuumd::protocol::spec::Optional;
    use continuumd::protocol::vocabulary::ErrorCode;

    /// Reconstruct every disclosed handle from its **wire text**, the way a client that read
    /// the string out of a log or a message would.
    ///
    /// This is the sharp form of "was never given the handle": the value the probe presents
    /// is not the one the daemon handed anybody. It is a fresh value parsed from characters.
    #[test]
    fn a_capability_that_admits_the_operation_is_admitted_on_handles_it_was_never_given() {
        let mut fixture = fixture();
        // "Never given" is checkable, not asserted: this principal has issued no request, so
        // the daemon has taken no decision about it and disclosed nothing to it. Six of the
        // seven handle classes below were published to *other* actors in the fixture's own
        // green path.
        assert!(
            fixture
                .daemon
                .state()
                .admissions()
                .iter()
                .all(|record| record.actor != STRANGER.actor),
            "the stranger has never appeared on this connection before"
        );
        let rebuilt = super::Handles {
            snapshot: WorkspaceHandle::new(fixture.handles.snapshot.as_str())
                .expect("a `ws_` round-trips its wire text"),
            other_snapshot: WorkspaceHandle::new(fixture.handles.other_snapshot.as_str())
                .expect("a `ws_` round-trips its wire text"),
            intent: IntentHandle::new(fixture.handles.intent.as_str())
                .expect("an `in_` round-trips its wire text"),
            evidence: EvidenceHandle::new(fixture.handles.evidence.as_str())
                .expect("an `ev_` round-trips its wire text"),
            task: TaskHandle::new(fixture.handles.task.as_str())
                .expect("a `task_` round-trips its wire text"),
            continuation: ContinuationHandle::new(fixture.handles.continuation.as_str())
                .expect("a `cont_` round-trips its wire text"),
            context: fixture.handles.context.clone(),
            trace: fixture.handles.trace.clone(),
            components: fixture.handles.components.clone(),
        };
        for (index, probe) in probes(&rebuilt).iter().enumerate() {
            let request = format!("req_stranger_{index}");
            let decision = decide_bare(&mut fixture.daemon, probe, STRANGER, &request);
            assert!(
                decision.admitted,
                "{}: authorization must not require a disclosure ledger",
                probe.operation
            );
        }
    }

    /// The dual's other half: authority without possession succeeds only where the *resource*
    /// exists, and the failure when it does not is the one indistinguishable denial rather
    /// than an existence oracle (RFC 0027 X2).
    ///
    /// Both calls are admitted. The difference in the answer is a fact about the store, taken
    /// after the authorization decision, which is the ordering `X3` fixes.
    #[test]
    fn a_wire_form_handle_naming_nothing_is_admitted_and_then_refused_by_the_handler() {
        let mut fixture = fixture();
        let absent = WorkspaceHandle::new("ws_nothing_is_filed_here").expect("a `ws_` handle");
        let before = fixture.daemon.state().admissions().len();
        let outcome = fixture.daemon.dispatch(&OperationRequest {
            envelope: keyed(
                bare("workspace.fork", ROOT, "req_absent_fork"),
                "idem-absent",
            ),
            arguments: Arguments::WorkspaceFork(WorkspaceForkRequest {
                base: absent,
                overlay: Optional::Absent,
                patches: Optional::Absent,
            }),
        });
        let record = fixture
            .daemon
            .state()
            .admissions()
            .get(before)
            .expect("one decision was taken")
            .clone();
        assert!(
            record.admitted,
            "the capability admits the operation, whatever the handle names"
        );
        assert_eq!(
            outcome.error_code(),
            Some(ErrorCode::CapabilityDenied),
            "and the handler's answer for a record it does not hold is the same denial"
        );
        assert!(matches!(outcome.payload, Payload::None));
    }
}

// --- probe 3: the confused deputy, on the handle channel --------------------------------------------

mod confused_deputy {
    use super::{Decision, ErrorCode, READER, ROOT, decide_bare, fixture, probes};

    /// An unauthorized principal chooses the handles; an authorized principal carries them.
    ///
    /// The reader cannot fork, seal, or accept. It *can* name any of those handles, and here
    /// it does: the probe table is built from handles the reader legitimately knows, and the
    /// root issues every request over them. Two things must hold at once, and both are
    /// asserted per operation: the root's decision follows the root's capability, and the
    /// reader gains nothing from having had its handles honoured.
    #[test]
    fn a_handle_chosen_by_an_unauthorized_principal_does_not_travel_with_authority() {
        let mut fixture = fixture();
        let probes = probes(&fixture.handles);
        let mut escalations = 0_usize;
        for (index, probe) in probes.iter().enumerate() {
            let deputy = decide_bare(
                &mut fixture.daemon,
                probe,
                ROOT,
                &format!("req_deputy_a_{index}"),
            );
            assert!(
                deputy.admitted,
                "{}: the deputy's decision is the deputy's capability",
                probe.operation
            );
            let after = decide_bare(
                &mut fixture.daemon,
                probe,
                READER,
                &format!("req_deputy_b_{index}"),
            );
            let spec = super::spec_of(probe.operation);
            if matches!(spec.authority, super::AuthorityLevel::Read) {
                // A `read` operation the reader could always have called. Not an escalation:
                // its authority never depended on the handle in the first place.
                continue;
            }
            assert_eq!(
                after,
                Decision {
                    admitted: false,
                    code: Some(ErrorCode::CapabilityDenied),
                },
                "{}: the deputy's success grants the requester nothing",
                probe.operation
            );
            escalations += 1;
        }
        assert_eq!(
            escalations, 18,
            "eighteen operations are above the requester's level, and none of them moved"
        );
    }

    /// The ledger's own view of the same two calls: identical operation, identical handles,
    /// two principals, two decisions.
    ///
    /// The [`AdmissionRecord`] carries operation, actor, capability and the decision. It
    /// carries **no handle**, which is the shape of "the decision was taken from the
    /// principal and the operation" rather than a claim about it.
    ///
    /// [`AdmissionRecord`]: continuumd::daemon::state::AdmissionRecord
    #[test]
    fn the_admission_ledger_separates_two_principals_over_one_identical_handle_set() {
        let mut fixture = fixture();
        let probes = probes(&fixture.handles);
        let fork = probes
            .iter()
            .find(|probe| probe.operation == "workspace.fork")
            .expect("workspace.fork is shaped");
        let before = fixture.daemon.state().admissions().len();
        decide_bare(&mut fixture.daemon, fork, ROOT, "req_ledger_root");
        decide_bare(&mut fixture.daemon, fork, READER, "req_ledger_reader");
        let records = &fixture.daemon.state().admissions()[before..];
        assert_eq!(records.len(), 2);
        assert_eq!(records[0].operation, records[1].operation);
        assert_ne!(records[0].actor, records[1].actor);
        assert_ne!(records[0].capability, records[1].capability);
        assert!(records[0].admitted);
        assert!(!records[1].admitted);
        assert_ne!(
            records[0].audit, records[1].audit,
            "each decision is correlated to its own request"
        );
    }
}

// --- probe 4: ordering — revocation and expiry between disclosure and use -----------------------------

mod ordering {
    use super::{Decision, EPHEMERAL, EXPIRED, ErrorCode, ROOT, cap, decide_bare, fixture, probes};

    /// Authority revoked *after* the handles were disclosed and *before* they are used.
    ///
    /// Three assertions, in the order that makes the claim: the ephemeral principal is
    /// admitted while it holds authority; every one of the thirty operations is refused after
    /// the revocation, over the identical handles; and the handles keep their meaning as
    /// names, because the root still runs the same thirty requests over them.
    #[test]
    fn revoking_authority_between_disclosure_and_use_refuses_every_shaped_operation() {
        let mut fixture = fixture();
        let probes = probes(&fixture.handles);
        for (index, probe) in probes.iter().enumerate() {
            let decision = decide_bare(
                &mut fixture.daemon,
                probe,
                EPHEMERAL,
                &format!("req_before_revoke_{index}"),
            );
            assert!(
                decision.admitted,
                "{}: the ephemeral capability starts out sufficient",
                probe.operation
            );
        }

        fixture
            .daemon
            .state_mut()
            .revoke_capability(&cap(EPHEMERAL.capability));

        for (index, probe) in probes.iter().enumerate() {
            let decision = decide_bare(
                &mut fixture.daemon,
                probe,
                EPHEMERAL,
                &format!("req_after_revoke_{index}"),
            );
            assert_eq!(
                decision,
                Decision {
                    admitted: false,
                    code: Some(ErrorCode::CapabilityDenied),
                },
                "{}: a handle the daemon itself disclosed carries no authority across a revocation",
                probe.operation
            );
        }

        for (index, probe) in probes.iter().enumerate() {
            let decision = decide_bare(
                &mut fixture.daemon,
                probe,
                ROOT,
                &format!("req_after_revoke_root_{index}"),
            );
            assert!(
                decision.admitted,
                "{}: losing authority did not change what the handle names",
                probe.operation
            );
        }
    }

    /// The same ordering through expiry rather than revocation: RFC 0027 E1 evaluates it per
    /// request, so a capability that was sufficient at the handshake is not sufficient now.
    #[test]
    fn an_expired_capability_holding_every_disclosed_handle_is_refused_on_all_thirty() {
        let mut fixture = fixture();
        for (index, probe) in probes(&fixture.handles).iter().enumerate() {
            let decision = decide_bare(
                &mut fixture.daemon,
                probe,
                EXPIRED,
                &format!("req_expired_{index}"),
            );
            assert_eq!(
                decision,
                Decision {
                    admitted: false,
                    code: Some(ErrorCode::CapabilityDenied),
                },
                "{}: expiry is decided per request, not cached at the handshake",
                probe.operation
            );
        }
    }

    /// Revoking the *parent* of a delegation refuses the child, over the same handles.
    ///
    /// The chain walk requires every hop to resolve and to narrow, so an authority withdrawn
    /// one level up is withdrawn here — again without any handle changing meaning.
    #[test]
    fn revoking_a_parent_capability_refuses_its_delegate_over_the_same_handles() {
        let mut fixture = fixture();
        let probes = probes(&fixture.handles);
        let seal = probes
            .iter()
            .find(|probe| probe.operation == "workspace.seal")
            .expect("workspace.seal is shaped");
        let before = decide_bare(
            &mut fixture.daemon,
            seal,
            super::BUILDER,
            "req_chain_before",
        );
        assert!(before.admitted);
        fixture
            .daemon
            .state_mut()
            .revoke_capability(&cap(ROOT.capability));
        let after = decide_bare(&mut fixture.daemon, seal, super::BUILDER, "req_chain_after");
        assert_eq!(
            after,
            Decision {
                admitted: false,
                code: Some(ErrorCode::CapabilityDenied),
            },
            "a delegate whose chain no longer resolves is refused"
        );
    }
}

// --- probe 5: where the decision is taken, and from what ------------------------------------------------

mod decision_point {
    use super::{
        DaemonState, MATRIX, Principal, ScopeClaim, admission, bare, cap, fixture, probes,
        register_principals, spec_of,
    };
    use continuumd::daemon::family::OperationFamily;
    use continuumd::protocol::envelope::RequestEnvelope;

    /// The scope claim the dispatcher would compute for a probe, taken from the same families
    /// the daemon runs.
    fn claim_for(arguments: &super::Arguments) -> ScopeClaim {
        let families: Vec<Box<dyn OperationFamily>> = vec![
            Box::new(super::WorkspaceFamily),
            Box::new(super::IntentFamily),
            Box::new(super::EvidenceFamily::new()),
            Box::new(super::ObserveFamily),
            Box::new(super::VerificationFamily),
            Box::new(super::TaskFamily),
            Box::new(super::ContextFamily),
            Box::new(super::WhiteboardFamily),
        ];
        let namespace = arguments
            .operation()
            .split_once('.')
            .map(|(namespace, _)| namespace)
            .expect("an operation is `namespace.verb`");
        families
            .iter()
            .find(|family| family.namespace() == namespace)
            .map_or_else(ScopeClaim::default, |family| family.scope(arguments))
    }

    fn envelope_for(operation: &str, principal: Principal) -> RequestEnvelope {
        bare(operation, principal, "req_admit")
    }

    /// **The mechanical core.** Two hundred and forty admission decisions, taken twice.
    ///
    /// The first state is the live fixture's: it holds the contract, both snapshots, the
    /// evidence node, the campaign, the continuation, every staged byte. The second holds the
    /// same capability registry and **nothing else** — no intent, no snapshot, no evidence, no
    /// task, no staged content. Every decision agrees.
    ///
    /// That is the criterion measured at its decision point. If authorization consulted what
    /// exists, or who had been told about it, the two columns could not be equal: the second
    /// state has never disclosed anything to anybody and holds none of the named resources.
    #[test]
    fn every_decision_is_identical_against_a_state_that_holds_none_of_the_named_resources() {
        let fixture = fixture();
        let probes = probes(&fixture.handles);

        let mut empty = DaemonState::new();
        register_principals(&mut empty);
        assert_eq!(
            empty.admissions().len(),
            0,
            "the resource-free state has taken no decision and holds no history"
        );

        let connection = cap(super::ROOT.capability);
        let reading = super::now();
        let mut compared = 0_usize;
        let mut admitted = 0_usize;
        let mut refused = 0_usize;
        for probe in &probes {
            let spec = spec_of(probe.operation);
            let claim = claim_for(&probe.arguments);
            for principal in MATRIX {
                let envelope = envelope_for(probe.operation, *principal);
                let live = admission::admit(
                    spec,
                    &envelope,
                    &claim,
                    fixture.daemon.state(),
                    &connection,
                    Some(&reading),
                )
                .is_ok();
                let bare =
                    admission::admit(spec, &envelope, &claim, &empty, &connection, Some(&reading))
                        .is_ok();
                assert_eq!(
                    live, bare,
                    "{} as {}: the decision moved with what the daemon holds",
                    probe.operation, principal.actor
                );
                compared += 1;
                if live {
                    admitted += 1;
                } else {
                    refused += 1;
                }
            }
        }
        assert_eq!(
            compared,
            30 * 8,
            "thirty shaped operations by eight principals"
        );
        // Anti-vacuity. An equality over 240 pairs proves nothing if the predicate answers
        // one way for all of them: a daemon that denied everything, or admitted everything,
        // would satisfy the comparison above and satisfy no criterion at all.
        assert!(
            admitted > 0 && refused > 0,
            "the matrix must contain both answers: {admitted} admitted, {refused} refused"
        );
    }

    /// The same decision is also invariant under the handles themselves, for a capability
    /// that declares no instance scope.
    ///
    /// Three handle sets per operation — the disclosed one, a different existing one, and one
    /// that names nothing at all — and one principal. The answers must agree, because the
    /// only inputs the predicate is allowed are the capability, the operation, and the
    /// actor.
    #[test]
    fn an_unscoped_capability_decides_the_same_way_whatever_handles_the_request_names() {
        let fixture = fixture();
        let connection = cap(super::ROOT.capability);
        let reading = super::now();

        let disclosed = probes(&fixture.handles);
        let swapped = probes(&super::Handles {
            snapshot: fixture.handles.other_snapshot.clone(),
            other_snapshot: fixture.handles.snapshot.clone(),
            intent: fixture.handles.intent.clone(),
            evidence: fixture.handles.evidence.clone(),
            task: fixture.handles.task.clone(),
            continuation: fixture.handles.continuation.clone(),
            context: fixture.handles.context.clone(),
            trace: fixture.handles.trace.clone(),
            components: fixture.handles.components.clone(),
        });
        let nowhere = probes(&super::Handles {
            snapshot: super::WorkspaceHandle::new("ws_nowhere").expect("a `ws_`"),
            other_snapshot: super::WorkspaceHandle::new("ws_nowhere_either").expect("a `ws_`"),
            intent: super::IntentHandle::new("in_nowhere").expect("an `in_`"),
            evidence: super::EvidenceHandle::new("ev_nowhere").expect("an `ev_`"),
            task: super::TaskHandle::new("task_nowhere").expect("a `task_`"),
            continuation: super::ContinuationHandle::new("cont_nowhere").expect("a `cont_`"),
            context: super::ContextHandle::new("ctx_nowhere").expect("a `ctx_`"),
            trace: super::Commitment::new("ws_nowhere_content"),
            components: super::Commitment::new("ws_nowhere_components"),
        });

        let mut admitted = 0_usize;
        let mut refused = 0_usize;
        for principal in MATRIX {
            for index in 0..disclosed.len() {
                let spec = spec_of(disclosed[index].operation);
                let envelope = envelope_for(disclosed[index].operation, *principal);
                let answers: Vec<bool> = [&disclosed[index], &swapped[index], &nowhere[index]]
                    .into_iter()
                    .map(|probe| {
                        admission::admit(
                            spec,
                            &envelope,
                            &claim_for(&probe.arguments),
                            fixture.daemon.state(),
                            &connection,
                            Some(&reading),
                        )
                        .is_ok()
                    })
                    .collect();
                assert!(
                    answers.windows(2).all(|pair| pair[0] == pair[1]),
                    "{} as {}: the decision moved with the handles the request named",
                    disclosed[index].operation,
                    principal.actor
                );
                if answers[0] {
                    admitted += 1;
                } else {
                    refused += 1;
                }
            }
        }
        // The same anti-vacuity control: invariance is only evidence when the invariant
        // quantity is not constant across the fixture.
        assert!(
            admitted > 0 && refused > 0,
            "the sweep must reach both answers: {admitted} admitted, {refused} refused"
        );
    }
}

// --- what the spec *does* make handle-sensitive ------------------------------------------------------

mod declared_scope {
    use super::{
        AuthorityLevel, Decision, ErrorCode, PRIVILEGED, Principal, ROOT, cap, decide, decide_bare,
        descriptor, fixture, name, probes, profile, registry,
    };
    use continuumd::protocol::spec::{Nullable, Optional};

    /// A capability scoped to one snapshot and one intent.
    const PINNED: Principal = Principal {
        actor: "agent:pinned",
        capability: "cap_pinned",
    };

    /// RFC 0027 T2 lets a decision depend on *which* handle a request names — and the
    /// dependence is declared in the capability, not accumulated by the caller.
    ///
    /// The pinned capability is never handed `ws_A`; it is admitted on it. It is handed
    /// `ws_B` by the fixture's own disclosure; it is refused on it. Possession runs one way
    /// and the decision runs the other, which is the cleanest available statement that the
    /// handle-sensitivity here is authority rather than possession.
    #[test]
    fn a_declared_instance_scope_admits_an_undisclosed_handle_and_refuses_a_disclosed_one() {
        let mut fixture = fixture();
        let mut scoped = descriptor(
            PINNED,
            AuthorityLevel::Promote,
            4,
            Optional::Present(profile(PRIVILEGED, &[], &[])),
        );
        scoped.snapshots = vec![fixture.handles.snapshot.clone()];
        scoped.intents = vec![fixture.handles.intent.clone()];
        scoped.expires_at = Nullable::Null;
        fixture
            .daemon
            .state_mut()
            .register_capability(scoped, Some(cap(ROOT.capability)));

        let probes = probes(&fixture.handles);
        let seal = probes
            .iter()
            .find(|probe| probe.operation == "workspace.seal")
            .expect("workspace.seal is shaped");
        let in_scope = decide_bare(&mut fixture.daemon, seal, PINNED, "req_scope_in");
        assert!(
            in_scope.admitted,
            "an in-scope snapshot the capability was never handed is admitted"
        );

        let diff = probes
            .iter()
            .find(|probe| probe.operation == "workspace.diff")
            .expect("workspace.diff is shaped");
        let out_of_scope = decide_bare(&mut fixture.daemon, diff, PINNED, "req_scope_out");
        assert_eq!(
            out_of_scope,
            Decision {
                admitted: false,
                code: Some(ErrorCode::CapabilityDenied),
            },
            "and the second, disclosed snapshot is out of scope however well known it is"
        );

        // The envelope's own `snapshot` field feeds T2 as well, so the same asymmetry holds
        // when the request pins the snapshot rather than naming it in the body.
        let status = probes
            .iter()
            .find(|probe| probe.operation == "task.status")
            .expect("task.status is shaped");
        let elsewhere = fixture.handles.other_snapshot.clone();
        let pinned_elsewhere = decide(
            &mut fixture.daemon,
            status,
            PINNED,
            "req_scope_envelope",
            |envelope| super::on(envelope, &elsewhere),
        );
        assert!(
            !pinned_elsewhere.admitted,
            "an envelope pinning an out-of-scope snapshot is refused for a `read` operation too"
        );
    }

    /// The measured shape of T2's instance scope: two of the nineteen handle classes.
    ///
    /// `CapabilityDescriptor` declares `snapshots`, `intents`, and `artifact_classes` and
    /// nothing else, so no capability in this protocol version can be scoped to one evidence
    /// node, one task, one continuation, or one Context Pack. Stated here as a fact read off
    /// the IDL rather than left implicit in the sweep's results.
    #[test]
    fn the_instance_scope_axis_covers_snapshots_and_intents_and_no_other_class() {
        let descriptor = registry::NAMED_STRUCTS
            .iter()
            .find(|entry| entry.name == "CapabilityDescriptor")
            .expect("the IDL declares the descriptor");
        let scope_fields: Vec<&str> = descriptor
            .fields
            .iter()
            .filter(|field| field.ty.starts_with("list<"))
            .map(|field| field.name)
            .collect();
        assert_eq!(
            scope_fields,
            vec!["snapshots", "intents", "artifact_classes"],
            "the three scope lists, and there is no fourth"
        );
        assert_eq!(
            registry::HANDLES.len(),
            19,
            "nineteen handle classes, two of which have an instance scope"
        );
        // And the class list is a plan §4.4 prefix set, not a handle set: it cannot name an
        // instance at all.
        let classes = descriptor
            .fields
            .iter()
            .find(|field| field.name == "artifact_classes")
            .expect("the class scope");
        assert_eq!(classes.ty, "list<String>");
        let _ = name("workspace.seal");
    }
}

// --- probe 6: negative controls ----------------------------------------------------------------------

/// A reference authorizer that decides from **possession** — the implementation this
/// criterion forbids.
///
/// It is the mutant, expressed as an oracle instead of a patch: the file cannot edit `src/`,
/// so it states what a possession-based daemon *would* answer and requires the real one to
/// disagree on every pinned case. A daemon that ever consulted a disclosure ledger would make
/// one of those assertions fail, which is what makes the sweep above non-vacuous.
#[derive(Debug, Default)]
struct PossessionLedger {
    disclosed: BTreeSet<(String, String)>,
}

impl PossessionLedger {
    /// Record that `actor` was handed `handle` — an authorized read, or another principal's
    /// disclosure.
    fn disclose(&mut self, actor: &str, handle: &str) {
        self.disclosed.insert((actor.to_owned(), handle.to_owned()));
    }

    /// What a possession-based authorizer answers: yes exactly when the caller holds every
    /// handle the request names.
    fn admits(&self, actor: &str, handles: &[&str]) -> bool {
        handles.iter().all(|handle| {
            self.disclosed
                .contains(&(actor.to_owned(), (*handle).to_owned()))
        })
    }
}

mod negative_controls {
    use super::{
        Decision, EPHEMERAL, ErrorCode, PossessionLedger, READER, ROOT, STRANGER, cap, decide_bare,
        fixture, probes,
    };

    /// Four pinned cases where a possession-based authorizer and this daemon must give
    /// **opposite** answers.
    ///
    /// Each is one bit, and each bit flips if authorization ever starts consulting who was
    /// told what. Two run in each direction, so neither a deny-everything nor an
    /// allow-everything daemon could satisfy the set.
    #[test]
    fn a_possession_based_authorizer_disagrees_with_the_daemon_on_every_pinned_case() {
        let mut fixture = fixture();
        let probes = probes(&fixture.handles);
        let snapshot = fixture.handles.snapshot.as_str().to_owned();
        let intent = fixture.handles.intent.as_str().to_owned();

        let mut ledger = PossessionLedger::default();
        // The disclosures the fixture actually made, recorded exactly.
        ledger.disclose(super::BUILDER.actor, &snapshot);
        ledger.disclose(READER.actor, &intent);
        ledger.disclose(EPHEMERAL.actor, &snapshot);
        ledger.disclose(EPHEMERAL.actor, &intent);

        let seal = probes
            .iter()
            .find(|probe| probe.operation == "workspace.seal")
            .expect("workspace.seal is shaped");
        let get = probes
            .iter()
            .find(|probe| probe.operation == "intent.get")
            .expect("intent.get is shaped");

        let mut disagreements = 0_usize;

        // 1. Possession without authority. The reader holds the `in_` and is above nothing;
        //    `intent.propose_revision` names that same `in_`.
        let revise = probes
            .iter()
            .find(|probe| probe.operation == "intent.propose_revision")
            .expect("intent.propose_revision is shaped");
        let daemon_says = decide_bare(&mut fixture.daemon, revise, READER, "req_mutant_1");
        let oracle_says = ledger.admits(READER.actor, &[&intent]);
        assert!(oracle_says, "the possession oracle would allow it");
        assert_ne!(
            daemon_says.admitted, oracle_says,
            "case 1: a possession-based daemon would have admitted this"
        );
        disagreements += 1;

        // 2. Authority without possession. The stranger was told nothing and holds `promote`.
        let daemon_says = decide_bare(&mut fixture.daemon, seal, STRANGER, "req_mutant_2");
        let oracle_says = ledger.admits(STRANGER.actor, &[&snapshot]);
        assert!(!oracle_says, "the possession oracle would refuse it");
        assert_ne!(
            daemon_says.admitted, oracle_says,
            "case 2: a possession-based daemon would have refused this"
        );
        disagreements += 1;

        // 3. Revocation ordering. The ephemeral principal keeps every handle it was given and
        //    loses the capability.
        fixture
            .daemon
            .state_mut()
            .revoke_capability(&cap(EPHEMERAL.capability));
        let daemon_says = decide_bare(&mut fixture.daemon, seal, EPHEMERAL, "req_mutant_3");
        let oracle_says = ledger.admits(EPHEMERAL.actor, &[&snapshot]);
        assert!(oracle_says, "possession is unchanged by a revocation");
        assert_eq!(
            daemon_says,
            Decision {
                admitted: false,
                code: Some(ErrorCode::CapabilityDenied),
            },
        );
        assert_ne!(
            daemon_says.admitted, oracle_says,
            "case 3: a possession-based daemon would still be admitting this caller"
        );
        disagreements += 1;

        // 4. A read the daemon admits for a principal the oracle never told.
        let daemon_says = decide_bare(&mut fixture.daemon, get, ROOT, "req_mutant_4");
        let oracle_says = ledger.admits(ROOT.actor, &[&intent]);
        assert!(!oracle_says);
        assert_ne!(
            daemon_says.admitted, oracle_says,
            "case 4: a possession-based daemon would refuse the connection's own capability"
        );
        disagreements += 1;

        assert_eq!(
            disagreements, 4,
            "four pinned bits, two in each direction, all flipped"
        );
    }

    /// The detector is not stuck.
    ///
    /// A refusal sweep proves nothing if the instrument reports "denied" for everything, and
    /// an admission sweep proves nothing if it reports "admitted" for everything. Over one
    /// fixture the ledger reports both values for the same operation and the same handles,
    /// and the wire code follows.
    #[test]
    fn the_instrument_reports_both_answers_over_one_operation_and_one_handle_set() {
        let mut fixture = fixture();
        let probes = probes(&fixture.handles);
        let seal = probes
            .iter()
            .find(|probe| probe.operation == "workspace.seal")
            .expect("workspace.seal is shaped");
        let admitted = decide_bare(&mut fixture.daemon, seal, ROOT, "req_stuck_yes");
        let refused = decide_bare(&mut fixture.daemon, seal, READER, "req_stuck_no");
        assert!(admitted.admitted);
        assert!(!refused.admitted);
        assert_ne!(admitted.code, Some(ErrorCode::CapabilityDenied));
        assert_eq!(refused.code, Some(ErrorCode::CapabilityDenied));
    }

    /// The one denial, byte for byte, over all thirty operations.
    ///
    /// A refusal that varied with whether the caller's handle resolved to anything would
    /// break the criterion from the other side: possession would become *observable* even
    /// where it is not authorizing, and a caller could map another principal's holdings by
    /// reading its own denials. The principal here is refused for every shaped operation
    /// (T3), so both runs are admission denials and every field of the two envelopes —
    /// status, code, detail, retryable, cost, epochs and the audit correlation — must agree.
    #[test]
    fn a_denial_is_one_value_whatever_the_named_handles_resolve_to() {
        let mut fixture = fixture();
        let existing = probes(&fixture.handles);
        let absent = probes(&super::Handles {
            snapshot: super::WorkspaceHandle::new("ws_absent").expect("a `ws_`"),
            other_snapshot: super::WorkspaceHandle::new("ws_absent_two").expect("a `ws_`"),
            intent: super::IntentHandle::new("in_absent").expect("an `in_`"),
            evidence: super::EvidenceHandle::new("ev_absent").expect("an `ev_`"),
            task: super::TaskHandle::new("task_absent").expect("a `task_`"),
            continuation: super::ContinuationHandle::new("cont_absent").expect("a `cont_`"),
            context: super::ContextHandle::new("ctx_absent").expect("a `ctx_`"),
            trace: super::Commitment::new("ws_absent_content"),
            components: super::Commitment::new("ws_absent_components"),
        });
        for index in 0..existing.len() {
            let held = fixture.daemon.dispatch(&super::OperationRequest {
                envelope: super::bare(existing[index].operation, super::DENIED, "req_same"),
                arguments: existing[index].arguments.clone(),
            });
            let unheld = fixture.daemon.dispatch(&super::OperationRequest {
                envelope: super::bare(absent[index].operation, super::DENIED, "req_same"),
                arguments: absent[index].arguments.clone(),
            });
            assert_eq!(
                held.envelope.error.value().map(|error| error.code),
                Some(ErrorCode::CapabilityDenied),
                "{}: the principal is refused for every shaped operation",
                existing[index].operation
            );
            assert_eq!(
                held.envelope, unheld.envelope,
                "{}: the refusal must not vary with what the named handle resolves to",
                existing[index].operation
            );
            assert_eq!(held.payload, unheld.payload);
            assert_eq!(held.data, unheld.data);
        }
    }
}
