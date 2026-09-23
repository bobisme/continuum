//! **PHASE-A-DEL-03 (bn-cho5): are the lifecycle suite's checks load-bearing?** The
//! seeded-defect campaign for the three concurrency defect classes the deliverable names
//! — **lost update**, **orphan worker**, **half-publication** — at the task-lifecycle
//! grain.
//!
//! # Why this file exists
//!
//! A schedule matrix that passes proves nothing on its own — a test that asserts nothing
//! also passes, and so does a test whose assertions happen to hold for a broken daemon.
//! The obligation (the G0-DX-13 mutation-campaign precedent,
//! `crates/continuum-workspace/tests/dx13_mutation_campaign.rs`, and the KCOV-08 rule
//! that every defect class is owned by a named check, fail-closed) is to show the checks
//! **fail** when the semantics are broken in ways a real implementer would plausibly
//! break them.
//!
//! `crates/continuumd/src/` is not modified, here or anywhere in this campaign. This file
//! contains [`MutantLifecycle`] — a small, deliberately separate re-implementation of the
//! task-lifecycle core — and runs the *same three checks* against the real daemon and
//! against each mutant. The checks are written once, as functions over a
//! [`LifecycleUnderTest`] seam, so there is no possibility of the daemon being graded on
//! an easier rubric than the mutants.
//!
//! Each mutation is a bug someone would actually write in a daemon that dispatches
//! *concurrently* — which is exactly what this daemon does not do (`Daemon::dispatch`
//! holds `&mut DaemonState`), and the campaign is the demonstration that the suite would
//! *see* it if that ever changed:
//!
//! - **`StaleRecordUpdate`** is the classic lost update: a budget update computes the new
//!   task record from the snapshot it read at arrival and writes the whole record back,
//!   clobbering the publication a concurrent advance committed in between;
//! - **`AbandonWorkerOnCancel`** answers the cancel and never drains the worker — the
//!   leak G0-DX-14 asks about, "a cancellation requested and never finalized";
//! - **`NameBeforeCommit`** puts the artifact's name in the task record before its bytes
//!   are durable — the half-publication: a crash-shaped exit (here, the cancel) leaves a
//!   record claiming content that does not exist.
//!
//! # The matrix
//!
//! | lifecycle | no lost update | no orphan worker | no half-publication |
//! |---|---|---|---|
//! | real daemon | pass | pass | pass |
//! | `Faithful` (control) | pass | pass | pass |
//! | `StaleRecordUpdate` | **caught** | pass | pass |
//! | `AbandonWorkerOnCancel` | pass | **caught** | pass |
//! | `NameBeforeCommit` | pass | pass | **caught** |
//!
//! The passes matter as much as the failures: they show each check is aimed at one defect
//! class rather than rejecting anything unfamiliar, and the `Faithful` control shows the
//! checks do not simply reject hand-written lifecycles. The diagonal is asserted with the
//! *reason string* of each catch, so a coincidental catch cannot be mistaken for a
//! targeted one.
//!
//! # The harness
//!
//! The daemon fixtures are `dx14_cancellation_matrix.rs`'s, duplicated locally rather
//! than imported, because a `tests/*.rs` file is its own crate and nothing here can `use`
//! a sibling one. The Die Hard model and contract are `include_str!`'d from the one copy
//! of each in this repository.

use std::collections::BTreeSet;

use continuum_engine_reference::diehard;
use continuum_intent::canonical_json::Json;
use continuum_intent::contract::IntentContract;
use continuum_value::epoch::ProtocolWindow;
use continuum_workspace::artifact_path::{ArtifactClass, ArtifactHandle};
use continuum_workspace::publication::CapabilityToken;
use continuum_workspace::snapshot::WorkspacePath;
use continuumd::daemon::family::{Arguments, Payload};
use continuumd::daemon::identity::Blake3Identity;
use continuumd::daemon::intent::IntentFamily;
use continuumd::daemon::state::{IntentRecord, RegistryStatus};
use continuumd::daemon::task::TaskFamily;
use continuumd::daemon::verification::{VerificationFamily, model_source};
use continuumd::daemon::workspace::WorkspaceFamily;
use continuumd::daemon::{Daemon, OperationRequest};
use continuumd::protocol::envelope::{Budget, EpochSet, RequestEnvelope};
use continuumd::protocol::handshake::{
    CapabilityDescriptor, CapabilityProfile, ClientHello, Negotiated, VersionRange, negotiate,
};
use continuumd::protocol::operations::intent::IntentAcceptRequest;
use continuumd::protocol::operations::task::{
    TaskCancelRequest, TaskResumeRequest, TaskUpdateBudgetRequest,
};
use continuumd::protocol::operations::verification::VerificationStartRequest;
use continuumd::protocol::operations::workspace::WorkspaceCreateRequest;
use continuumd::protocol::registry::ENCODINGS;
use continuumd::protocol::scalar::{
    ActorId, CapabilityHandle, Commitment, EpochIdentity, IntentHandle, Opaque, OperationName,
    ProtocolVersion, RequestId, TaskHandle, Timestamp, WorkspaceHandle,
};
use continuumd::protocol::shared::{SnapshotComponents, SnapshotEpochs, Target};
use continuumd::protocol::spec::{Nullable, Optional};
use continuumd::protocol::vocabulary::{
    AuthorityLevel, Encoding, Portfolio, ResultStatus, TargetKind,
};

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

// --- the seam the checks are written against -----------------------------------------------

/// The worker accounting a lifecycle exposes after its work is over.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct WorkerAccounting {
    /// Workers ever admitted.
    admitted: usize,
    /// Workers neither terminal nor finalized — the leak.
    orphans: usize,
    /// The layer's own totality claim: everything opened was finalized, nothing dangles.
    total: bool,
}

/// The lifecycle surface the three checks need, and nothing more.
///
/// Deliberately minimal: the checks must be expressible against a lifecycle that is *not*
/// the daemon, or the campaign would be grading the daemon against itself.
trait LifecycleUnderTest {
    /// Start the campaign under a ceiling too small to close it: commits one partial
    /// artifact and parks.
    fn start_bounded(&mut self);
    /// Raise the ceiling without re-running — the interleaved second updater.
    fn update_budget(&mut self);
    /// Resume under a ceiling large enough to close; commits at least one more artifact.
    fn resume_to_completion(&mut self);
    /// Request cancellation.
    fn cancel(&mut self);
    /// The artifact identities the task's record claims committed, in commit order.
    fn claimed(&self) -> Vec<String>;
    /// Whether the named artifact's bytes are durably readable.
    fn readable(&self, artifact: &str) -> bool;
    /// Whether a staged (uncommitted) publication is observable from outside a dispatch.
    fn staging_observable(&self) -> bool;
    /// The worker accounting.
    fn workers(&self) -> WorkerAccounting;
}

// --- the checks ----------------------------------------------------------------------------

/// **No lost update.** A publication committed before a concurrent budget update is still
/// on the record after it, and after the resume that follows: the record is append-only
/// under interleaved updaters (INV-009: "resuming a task may add evidence … it may not
/// silently replace prior artifacts").
fn check_no_lost_update(lifecycle: &mut dyn LifecycleUnderTest) -> Result<(), String> {
    lifecycle.start_bounded();
    let parked = lifecycle.claimed();
    if parked.is_empty() {
        return Err("the lane parked without committing, so the check can see nothing".to_owned());
    }
    lifecycle.update_budget();
    lifecycle.resume_to_completion();
    let done = lifecycle.claimed();
    // The defect first, the vacuity guard second: a clobbered record is *shorter*, and a
    // guard that fired before the comparison would report the wrong complaint.
    if done.len() < parked.len() || done[..parked.len()] != parked[..] {
        return Err(format!(
            "lost update: a committed artifact vanished from the record — {parked:?} then {done:?}"
        ));
    }
    if done.len() == parked.len() {
        return Err(format!(
            "the resume committed nothing ({} artifacts before and after), so the check \
             swept one commit",
            done.len()
        ));
    }
    for artifact in &done {
        if !lifecycle.readable(artifact) {
            return Err(format!(
                "the record claims {artifact} but its bytes are gone"
            ));
        }
    }
    Ok(())
}

/// **No orphan worker.** After a cancellation, every admitted worker is terminal and
/// finalized, and the accounting layer's own totality claim holds — nothing was
/// "requested and never finalized" (G0-DX-14).
fn check_no_orphan_worker(lifecycle: &mut dyn LifecycleUnderTest) -> Result<(), String> {
    lifecycle.start_bounded();
    lifecycle.cancel();
    let workers = lifecycle.workers();
    if workers.admitted == 0 {
        return Err("no worker was ever admitted, so the check saw no work".to_owned());
    }
    if workers.orphans > 0 {
        return Err(format!(
            "orphan worker: {} of {} workers were never finalized",
            workers.orphans, workers.admitted
        ));
    }
    if !workers.total {
        return Err("the accounting is not total: something opened was never torn down".to_owned());
    }
    Ok(())
}

/// **No half-publication.** Across a cancellation, the record's claims do not change, no
/// staged publication is observable, and everything claimed is durably readable — an
/// artifact is committed or absent, never named-but-missing (`rule task.cancel_correct`,
/// INV-017).
fn check_no_half_publication(lifecycle: &mut dyn LifecycleUnderTest) -> Result<(), String> {
    lifecycle.start_bounded();
    let before = lifecycle.claimed();
    if before.is_empty() {
        return Err("the lane parked without committing, so the check can see nothing".to_owned());
    }
    lifecycle.cancel();
    let after = lifecycle.claimed();
    if after != before {
        return Err(format!(
            "the cancel changed the record's claims: {before:?} then {after:?}"
        ));
    }
    if lifecycle.staging_observable() {
        return Err("a staged publication is observable after the cancel".to_owned());
    }
    for artifact in &after {
        if !lifecycle.readable(artifact) {
            return Err(format!(
                "half-publication: the record claims {artifact} but its bytes are not durable"
            ));
        }
    }
    Ok(())
}

// --- the real daemon, behind the seam ------------------------------------------------------

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

fn epoch(token: &str) -> EpochIdentity {
    EpochIdentity::new(token).expect("a well-formed epoch identity")
}

fn operator() -> CapabilityToken {
    continuumd::daemon::identity::capability_to_store(&cap("cap_root"))
        .expect("`cap_root` is a well-formed store token")
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
        instances: Optional::Absent,
    }
}

fn profile(privileged: &[&str]) -> CapabilityProfile {
    CapabilityProfile {
        privileged_operations: privileged.iter().map(|entry| name(entry)).collect(),
        denied_operations: Vec::new(),
        data_grants: Vec::new(),
        cross_principal_sharing: true,
    }
}

fn negotiated() -> Negotiated {
    let hello = ClientHello {
        protocol_versions: VersionRange {
            low: version(),
            high: version(),
        },
        encodings: vec![Encoding::CanonicalJson],
        client: "continuumd-lifecycle-mutation-test".to_owned(),
        actor: who("service:continuumd"),
        capability: cap("cap_root"),
        features: Optional::Absent,
    };
    negotiate(&[version()], ProtocolWindow::new(3), ENCODINGS, &hello).expect("3.1 is served")
}

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

fn now() -> Timestamp {
    Timestamp::new("2026-08-01T00:00:00.000Z").expect("a well-formed timestamp")
}

fn daemon() -> Daemon {
    let root = Some(cap("cap_root"));
    Daemon::builder(Blake3Identity, negotiated(), cap("cap_root"))
        .epochs(epochs())
        .now(now())
        .capability(
            grant(
                "cap_root",
                "service:continuumd",
                AuthorityLevel::Promote,
                4,
                Optional::Present(profile(&["intent.accept", "intent.reject", "intent.lock"])),
            ),
            None,
        )
        .capability(
            grant(
                "cap_builder",
                "agent:builder",
                AuthorityLevel::Propose,
                3,
                Optional::Absent,
            ),
            root.clone(),
        )
        .capability(
            grant(
                "cap_runner",
                "agent:runner",
                AuthorityLevel::Execute,
                3,
                Optional::Absent,
            ),
            root.clone(),
        )
        .capability(
            grant(
                "cap_steward",
                "human:steward",
                AuthorityLevel::ReviseIntent,
                3,
                Optional::Present(profile(&["intent.accept", "intent.reject", "intent.lock"])),
            ),
            root,
        )
        .family(WorkspaceFamily)
        .family(IntentFamily)
        .family(TaskFamily)
        .family(VerificationFamily)
        .build()
}

fn die_hard_contract() -> IntentContract {
    IntentContract::decode(DIE_HARD_CONTRACT.trim_end().as_bytes()).expect("the fixture decodes")
}

fn intent_handle(contract: &IntentContract) -> IntentHandle {
    let stored = continuum_workspace::publication::ContentIdentifier::identify(
        &Blake3Identity,
        ArtifactClass::IntentContract,
        &contract.identity_preimage_bytes(),
    )
    .expect("blake3 names every input");
    continuumd::daemon::identity::intent_to_wire(&stored).expect("an `in_` handle")
}

fn die_hard_source() -> Commitment {
    model_source(&Blake3Identity, [(MODULE_PATH, DIE_HARD_MODEL.as_bytes())])
        .expect("blake3 names the module set")
}

fn acceptance_bytes() -> Opaque {
    let mut fields: std::collections::BTreeMap<String, Json> = std::collections::BTreeMap::new();
    for (key, value) in [
        ("accepted_by", "human:steward"),
        ("capability", "revise-intent"),
        ("signature", "sig-die-hard-v1"),
        ("audit_record", "supplied-by-the-caller-and-overwritten"),
        ("timestamp", "2026-08-01T00:00:00.000Z"),
    ] {
        fields.insert(key.to_owned(), Json::String(value.to_owned()));
    }
    Opaque::from_bytes(Json::Object(fields).to_canonical_bytes())
}

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

fn keyed(mut envelope: RequestEnvelope, key: &str) -> RequestEnvelope {
    envelope.idempotency_key = Optional::Present(key.to_owned());
    envelope
}

fn budgeted(mut envelope: RequestEnvelope, states: u64) -> RequestEnvelope {
    envelope.budget = Optional::Present(budget(states));
    envelope
}

fn budget(states: u64) -> Budget {
    Budget {
        wall_ms: Optional::Absent,
        cpu_ms: Optional::Absent,
        memory_bytes: Optional::Absent,
        states: Optional::Present(states),
        solver_ms: Optional::Absent,
        proof_ms: Optional::Absent,
        tokens: Optional::Absent,
        candidates: Optional::Absent,
        bytes: Optional::Absent,
    }
}

/// The real thing: one daemon, the dpor-shaped campaign, driven through `dispatch` only.
struct DaemonLifecycle {
    daemon: Daemon,
    snapshot: WorkspaceHandle,
    task: Option<TaskHandle>,
    requests: u32,
}

impl DaemonLifecycle {
    fn new() -> Self {
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

        let mut files = Vec::new();
        for (path, content) in [(MODULE_PATH, DIE_HARD_MODEL), ("README.md", "# TV-009\n")] {
            files.push(
                daemon
                    .state_mut()
                    .stage(
                        &Blake3Identity,
                        WorkspacePath::new(path).expect("a workspace path"),
                        content.as_bytes().to_vec(),
                    )
                    .expect("staging names its content"),
            );
        }
        let configuration = daemon
            .state_mut()
            .stage(
                &Blake3Identity,
                WorkspacePath::new("default.model.toml").expect("a workspace path"),
                DIE_HARD_CONFIG.as_bytes().to_vec(),
            )
            .expect("staging names its content");

        daemon.state_mut().models_mut().register(
            die_hard_source(),
            diehard::model().expect("the port builds"),
        );

        let accepted = daemon.dispatch(&OperationRequest {
            envelope: keyed(
                envelope(
                    "intent.accept",
                    "human:steward",
                    "cap_steward",
                    "req_accept",
                ),
                "idem-accept",
            ),
            arguments: Arguments::IntentAccept(IntentAcceptRequest {
                proposal: intent.clone(),
                acceptance: acceptance_bytes(),
                bundle: Optional::Absent,
            }),
        });
        assert_eq!(accepted.envelope.status, ResultStatus::Ok);

        let created = daemon.dispatch(&OperationRequest {
            envelope: keyed(
                envelope(
                    "workspace.create",
                    "agent:builder",
                    "cap_builder",
                    "req_create",
                ),
                "idem-create",
            ),
            arguments: Arguments::WorkspaceCreate(WorkspaceCreateRequest {
                components: SnapshotComponents {
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
                    intent,
                    correspondence: Vec::new(),
                    proof_environment: Vec::new(),
                    configuration: vec![configuration],
                    file_components: Optional::Absent,
                },
                overlay: Optional::Absent,
                seal: Optional::Present(true),
            }),
        });
        assert_eq!(created.envelope.status, ResultStatus::Ok);
        let snapshot = match &created.payload {
            Payload::WorkspaceCreate(response) => response.snapshot.clone(),
            other => panic!("expected a workspace.create payload, got {other:?}"),
        };

        Self {
            daemon,
            snapshot,
            task: None,
            requests: 0,
        }
    }

    fn request(&mut self) -> String {
        self.requests += 1;
        format!("req_{}", self.requests)
    }

    fn task(&self) -> &TaskHandle {
        self.task.as_ref().expect("the campaign started")
    }
}

impl LifecycleUnderTest for DaemonLifecycle {
    fn start_bounded(&mut self) {
        let request = self.request();
        let snapshot = self.snapshot.clone();
        let outcome = self.daemon.dispatch(&OperationRequest {
            envelope: {
                let mut envelope = budgeted(
                    keyed(
                        envelope("verification.start", "agent:runner", "cap_runner", &request),
                        &format!("idem-{request}"),
                    ),
                    4,
                );
                envelope.snapshot = Nullable::Value(snapshot);
                envelope
            },
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
        assert!(
            outcome.error_code().is_none(),
            "the start was refused — {:?}",
            outcome.envelope.error
        );
        self.task = match &outcome.payload {
            Payload::VerificationStart(response) => response.task.value().cloned(),
            other => panic!("expected a verification.start payload, got {other:?}"),
        };
    }

    fn update_budget(&mut self) {
        let request = self.request();
        let task = self.task().clone();
        let outcome = self.daemon.dispatch(&OperationRequest {
            envelope: keyed(
                envelope("task.update_budget", "agent:runner", "cap_runner", &request),
                &format!("idem-{request}"),
            ),
            arguments: Arguments::TaskUpdateBudget(TaskUpdateBudgetRequest {
                task,
                budget: budget(64),
            }),
        });
        assert_eq!(outcome.envelope.status, ResultStatus::Ok);
    }

    fn resume_to_completion(&mut self) {
        let request = self.request();
        let continuation = self
            .daemon
            .state()
            .tasks()
            .get(self.task())
            .expect("the daemon holds the task")
            .continuation
            .clone()
            .expect("a parked lane holds a continuation");
        let outcome = self.daemon.dispatch(&OperationRequest {
            envelope: budgeted(
                keyed(
                    envelope("task.resume", "agent:runner", "cap_runner", &request),
                    &format!("idem-{request}"),
                ),
                64,
            ),
            arguments: Arguments::TaskResume(TaskResumeRequest {
                continuation,
                budget: Optional::Present(budget(64)),
            }),
        });
        assert!(
            outcome.error_code().is_none(),
            "the resume was refused — {:?}",
            outcome.envelope.error
        );
    }

    fn cancel(&mut self) {
        let request = self.request();
        let task = self.task().clone();
        let outcome = self.daemon.dispatch(&OperationRequest {
            envelope: keyed(
                envelope("task.cancel", "agent:runner", "cap_runner", &request),
                &format!("idem-{request}"),
            ),
            arguments: Arguments::TaskCancel(TaskCancelRequest { task }),
        });
        assert_eq!(outcome.envelope.status, ResultStatus::Ok);
    }

    fn claimed(&self) -> Vec<String> {
        self.daemon
            .state()
            .tasks()
            .get(self.task())
            .expect("the daemon holds the task")
            .evidence
            .committed()
            .iter()
            .map(|publication| publication.commitment().as_str().to_owned())
            .collect()
    }

    fn readable(&self, artifact: &str) -> bool {
        // A publication's identity is the `commitment` on its `ArtifactRef` — a `task_`
        // handle in the store (`g1_crash_recovery_evidence.rs` documents the mapping).
        let Some(identity) = artifact.strip_prefix(ArtifactClass::Task.prefix()) else {
            return false;
        };
        let Ok(handle) = ArtifactHandle::new(ArtifactClass::Task, identity) else {
            return false;
        };
        self.daemon.store().read(&handle, &operator()).is_ok()
    }

    fn staging_observable(&self) -> bool {
        self.daemon
            .state()
            .tasks()
            .get(self.task())
            .is_some_and(|entry| entry.evidence.is_staging())
    }

    fn workers(&self) -> WorkerAccounting {
        let regions = self.daemon.state().regions();
        let admitted: usize = regions
            .finalizations()
            .iter()
            .map(|finalization| finalization.workers().len())
            .sum::<usize>()
            + regions.orphans().len();
        WorkerAccounting {
            admitted,
            orphans: regions.orphans().len(),
            total: regions.is_total(),
        }
    }
}

// --- the mutants ---------------------------------------------------------------------------

/// One plausible way an implementer breaks the lifecycle core under concurrency.
///
/// See this file's header for what bug each stands for. `Faithful` is the control: a
/// faithful re-implementation must pass every check, or the checks are rejecting
/// unfamiliarity rather than incorrectness.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mutation {
    Faithful,
    /// `update_budget` writes back the record it read at arrival, losing the commit that
    /// landed in between.
    StaleRecordUpdate,
    /// `cancel` answers without draining or finalizing the worker.
    AbandonWorkerOnCancel,
    /// The artifact's name enters the record at stage time, before its bytes are durable.
    NameBeforeCommit,
}

/// A separate, deliberately buggy re-implementation of the lifecycle core.
///
/// It exists only to prove the checks above are load-bearing. It is a *copy* rather than
/// an edit: `crates/continuumd/src/` is untouched by this campaign, so there is nothing
/// to revert and no window in which the shipped code was wrong.
struct MutantLifecycle {
    mutation: Mutation,
    /// Durable artifact bytes, by identity.
    store: BTreeSet<String>,
    /// The task record's claims, in commit order.
    record: Vec<String>,
    /// A publication in flight.
    staged: Option<String>,
    /// Admitted workers; `true` when terminal and finalized.
    workers: Vec<bool>,
    /// The record snapshot the stale updater will write back.
    snapshot_at_arrival: Vec<String>,
}

impl MutantLifecycle {
    fn new(mutation: Mutation) -> Self {
        Self {
            mutation,
            store: BTreeSet::new(),
            record: Vec::new(),
            staged: None,
            workers: Vec::new(),
            snapshot_at_arrival: Vec::new(),
        }
    }

    /// The two-phase commit, at the model's grain: bytes first, then the name.
    fn commit(&mut self, artifact: &str) {
        match self.mutation {
            Mutation::NameBeforeCommit => {
                // The half-publication: the record names the artifact now; the bytes are
                // "flushed later" — which a crash-shaped exit will never do.
                self.record.push(artifact.to_owned());
                self.staged = Some(artifact.to_owned());
            }
            _ => {
                self.store.insert(artifact.to_owned());
                self.record.push(artifact.to_owned());
            }
        }
    }

    /// The deferred flush the `NameBeforeCommit` mutant relies on.
    fn flush(&mut self) {
        if let Some(staged) = self.staged.take() {
            self.store.insert(staged);
        }
    }
}

impl LifecycleUnderTest for MutantLifecycle {
    fn start_bounded(&mut self) {
        self.snapshot_at_arrival = self.record.clone();
        self.workers.push(false); // admitted, parked: not yet terminal
        self.commit("task_frontier-1");
    }

    fn update_budget(&mut self) {
        if self.mutation == Mutation::StaleRecordUpdate {
            // The lost update: the whole record is written back from the snapshot read
            // at arrival, clobbering the commit that landed in between.
            self.record = self.snapshot_at_arrival.clone();
        }
    }

    fn resume_to_completion(&mut self) {
        self.flush();
        self.commit("task_closure");
        self.flush();
        if let Some(worker) = self.workers.last_mut() {
            *worker = true; // completed: terminal and finalized
        }
    }

    fn cancel(&mut self) {
        // The crash-shaped exit: whatever was staged is dropped, never flushed.
        self.staged = None;
        if self.mutation != Mutation::AbandonWorkerOnCancel {
            for worker in &mut self.workers {
                *worker = true; // drained, terminal, finalized
            }
        }
    }

    fn claimed(&self) -> Vec<String> {
        self.record.clone()
    }

    fn readable(&self, artifact: &str) -> bool {
        self.store.contains(artifact)
    }

    fn staging_observable(&self) -> bool {
        self.staged.is_some()
    }

    fn workers(&self) -> WorkerAccounting {
        let orphans = self.workers.iter().filter(|terminal| !**terminal).count();
        WorkerAccounting {
            admitted: self.workers.len(),
            orphans,
            total: orphans == 0,
        }
    }
}

// --- the campaign --------------------------------------------------------------------------

/// Every check against one lifecycle family, each over a fresh instance.
struct Verdict {
    lost_update: Result<(), String>,
    orphan_worker: Result<(), String>,
    half_publication: Result<(), String>,
}

/// Grade one mutation (or the real daemon, when `mutation` is `None`).
///
/// Each check runs over a fresh lifecycle: the checks drive different beat sequences, and
/// a shared instance would let one check's teardown mask another's defect.
fn grade(mutation: Option<Mutation>) -> Verdict {
    fn fresh(mutation: Option<Mutation>) -> Box<dyn LifecycleUnderTest> {
        match mutation {
            None => Box::new(DaemonLifecycle::new()),
            Some(mutation) => Box::new(MutantLifecycle::new(mutation)),
        }
    }
    Verdict {
        lost_update: check_no_lost_update(fresh(mutation).as_mut()),
        orphan_worker: check_no_orphan_worker(fresh(mutation).as_mut()),
        half_publication: check_no_half_publication(fresh(mutation).as_mut()),
    }
}

#[test]
fn the_real_daemon_passes_every_check() {
    let verdict = grade(None);
    verdict.lost_update.expect("no lost update");
    verdict.orphan_worker.expect("no orphan worker");
    verdict.half_publication.expect("no half-publication");
}

#[test]
fn a_faithful_re_implementation_passes_every_check() {
    // The control. Without it, a mutant failing a check could mean nothing more than
    // "this is not the daemon".
    let verdict = grade(Some(Mutation::Faithful));
    verdict.lost_update.expect("no lost update");
    verdict.orphan_worker.expect("no orphan worker");
    verdict.half_publication.expect("no half-publication");
}

#[test]
fn mutation_stale_record_update_is_caught_by_the_lost_update_check() {
    let verdict = grade(Some(Mutation::StaleRecordUpdate));
    let caught = verdict
        .lost_update
        .expect_err("a stale read-modify-write must be caught");
    assert!(
        caught.contains("lost update"),
        "caught for the wrong reason: {caught}"
    );

    // Targeted, not indiscriminate: the other two properties are untouched by this bug.
    verdict
        .orphan_worker
        .expect("a stale budget write does not orphan a worker");
    verdict
        .half_publication
        .expect("a stale budget write does not half-publish");
}

#[test]
fn mutation_abandon_worker_on_cancel_is_caught_by_the_orphan_check() {
    let verdict = grade(Some(Mutation::AbandonWorkerOnCancel));
    let caught = verdict
        .orphan_worker
        .expect_err("an undrained cancellation must be caught");
    assert!(
        caught.contains("orphan worker"),
        "caught for the wrong reason: {caught}"
    );

    verdict
        .lost_update
        .expect("an undrained cancel does not lose an update (no cancel in that check)");
    verdict
        .half_publication
        .expect("an undrained cancel does not half-publish");
}

#[test]
fn mutation_name_before_commit_is_caught_by_the_half_publication_check() {
    let verdict = grade(Some(Mutation::NameBeforeCommit));
    let caught = verdict
        .half_publication
        .expect_err("naming before committing must be caught");
    assert!(
        caught.contains("half-publication") || caught.contains("staged publication"),
        "caught for the wrong reason: {caught}"
    );

    verdict
        .lost_update
        .expect("with no crash-shaped exit, the deferred flush eventually lands");
    verdict
        .orphan_worker
        .expect("naming early does not orphan a worker");
}

#[test]
fn every_mutation_is_caught_by_at_least_one_check() {
    // The campaign's summary claim, stated once so a future mutation cannot be added
    // without also being caught (the KCOV-08 fail-closed rule, at this file's grain).
    for mutation in [
        Mutation::StaleRecordUpdate,
        Mutation::AbandonWorkerOnCancel,
        Mutation::NameBeforeCommit,
    ] {
        let verdict = grade(Some(mutation));
        assert!(
            verdict.lost_update.is_err()
                || verdict.orphan_worker.is_err()
                || verdict.half_publication.is_err(),
            "{mutation:?} survived every check: the checks are not load-bearing"
        );
    }
}
