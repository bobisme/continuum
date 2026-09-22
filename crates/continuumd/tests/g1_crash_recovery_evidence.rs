//! G1 evidence: daemon crash recovery leaves no stale index entries, no orphan tasks, and no
//! half-published artifacts — and the recovery is itself deterministic and evidenced (bn-3dr).
//!
//! # What this file is evidence for
//!
//! plan §4.5's G1 bullet and docs/35's "Crash safety and the index verifier", driven through
//! [`Daemon::dispatch_or_die`] and [`recovery::recover`] and nothing else. The distinction
//! from PR 6's cancellation work is the grain: G0-DX-14 kills *a task* mid-dispatch, this
//! kills *the daemon* mid-dispatch and restarts it over what the durable layer still holds.
//!
//! | Clause | Test |
//! |---|---|
//! | the durable image is a step function of the dispatch boundary — steps 1–7 write no store | [`the_durable_image_is_a_step_function_of_the_dispatch_boundary`] |
//! | a daemon killed mid-dispatch answers nothing; there is no envelope | [`a_daemon_killed_mid_dispatch_answers_nothing_at_all`] |
//! | INV-017: a crash between the content commit and the index commit leaves residue, never a stale index entry | [`a_crash_between_the_two_commits_leaves_residue_and_no_stale_index_entry`] |
//! | the report distinguishes the two publication phases, so it reads the store rather than printing a constant | [`the_report_distinguishes_the_two_publication_phases`] |
//! | a composite artifact caught mid-publication is absent under its own name — no half-published artifact | [`a_partly_sealed_workspace_is_absent_under_its_own_name`] |
//! | the records that did land are complete artifacts, and a retry after the restart converges on them | [`a_retry_after_the_restart_converges_on_the_records_that_landed`] |
//! | IMPL-02 (first conjunct): a parked task's committed publication survives the crash and is returned reconciled | [`a_parked_tasks_committed_publication_survives_the_crash`] |
//! | IMPL-02 (second conjunct): an uncommitted partial is absent after recovery, never half-visible | [`an_uncommitted_partial_is_absent_after_recovery`] |
//! | no orphan task artifact: every surviving `task_*` identity is index-resolved and receipted | [`every_surviving_task_artifact_is_reconciled_and_receipted`] |
//! | the volatile declaration is real: the restarted daemon holds none of what it declares lost | [`the_restarted_daemon_holds_none_of_what_the_report_declares_lost`] |
//! | two recoveries from one crashed state render byte-identically | [`two_recoveries_from_one_crashed_state_render_byte_identically`] |
//! | two independently crashed daemons that ran one campaign recover identically | [`two_independently_crashed_daemons_recover_identically`] |
//! | recovery reports residue and never deletes it; `collect_garbage` is the separate operator action | [`recovery_reports_residue_and_never_deletes_it`] |
//! | **control**: without the injected fault the same assertions flip | [`control_without_the_fault_the_seal_completes_and_the_verdict_is_clean`] |
//! | **control**: the receipt count is read from the ledger, not asserted | [`control_the_receipt_count_is_read_from_the_ledger`] |
//! | **mutant**: a corrupted identity seam is caught and flips the verdict to `defective` | [`mutant_a_corrupted_identity_seam_flips_the_verdict_to_defective`] |
//!
//! # The two crash seams, and why there are two
//!
//! [`CrashPoint`] is the *dispatch* seam: nine boundaries, one per step of the eight-step
//! pipeline plus the one after the handler. [`StorageFaults`] is the *store* seam, and it is
//! `continuum-workspace`'s, not this crate's — docs/35's crash-injection point is "between
//! the content commit and the index commit", which is inside the publication protocol, and
//! G0-DX-13's campaign already proved the store's half of it. This file composes the two
//! rather than re-proving either: it kills whole daemons at dispatch boundaries, and it uses
//! the store's own fault seam to reach the INV-017 instant *inside* a daemon's step 8.
//!
//! # The harness
//!
//! The fixtures are `pr6_impl02_budget_evidence.rs`'s, duplicated locally rather than
//! imported, because a `tests/*.rs` file is its own crate and nothing here can `use` a
//! sibling one. The Die Hard model and contract are `include_str!`'d from the one copy of
//! each in this repository, so no fixture here can drift from the corpus.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use continuum_engine_reference::diehard;
use continuum_intent::canonical_json::Json;
use continuum_intent::contract::IntentContract;
use continuum_value::epoch::ProtocolWindow;
use continuum_workspace::artifact_path::{ArtifactClass, ArtifactHandle};
use continuum_workspace::publication::{
    AbortReason, CapabilityToken, ContentIdentifier, IdentityUnavailable, PublicationPhase,
    ReferenceStore, StorageFaults, StoreDefect,
};
use continuumd::daemon::family::{Arguments, Payload};
use continuumd::daemon::identity::Blake3Identity;
use continuumd::daemon::intent::IntentFamily;
use continuumd::daemon::recovery::{
    self, CrashInjector, CrashPoint, Disposition, RecoveryReport, Verdict, VolatileFact,
};
use continuumd::daemon::state::{IntentRecord, RegistryStatus};
use continuumd::daemon::task::TaskFamily;
use continuumd::daemon::verification::{VerificationFamily, model_source};
use continuumd::daemon::workspace::WorkspaceFamily;
use continuumd::daemon::{Builder, Daemon, DurableSubstrate, OperationRequest};
use continuumd::protocol::envelope::{Budget, EpochSet, RequestEnvelope};
use continuumd::protocol::handshake::{
    CapabilityDescriptor, CapabilityProfile, ClientHello, Negotiated, VersionRange, negotiate,
};
use continuumd::protocol::operations::intent::IntentAcceptRequest;
use continuumd::protocol::operations::task::{TaskResumeRequest, TaskStatusRequest};
use continuumd::protocol::operations::verification::VerificationStartRequest;
use continuumd::protocol::operations::workspace::WorkspaceCreateRequest;
use continuumd::protocol::registry::ENCODINGS;
use continuumd::protocol::scalar::{
    ActorId, CapabilityHandle, Commitment, ContinuationHandle, EpochIdentity, IntentHandle, Opaque,
    OperationName, ProtocolVersion, RequestId, TaskHandle, Timestamp, WorkspaceHandle,
};
use continuumd::protocol::shared::{SnapshotComponents, SnapshotEpochs, Target};
use continuumd::protocol::spec::{Nullable, Optional};
use continuumd::protocol::vocabulary::{
    AuthorityLevel, Encoding, ErrorCode, Portfolio, ResultStatus, TargetKind, TaskStatus,
};

use continuum_workspace::snapshot::WorkspacePath;

/// The TV-009 port's model, verbatim.
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

// --- the two crash seams -----------------------------------------------------------------

/// A daemon that dies at exactly one dispatch boundary.
#[derive(Debug, Clone, Copy)]
struct KillAt(CrashPoint);

impl CrashInjector for KillAt {
    fn kills(&self, point: CrashPoint) -> bool {
        point == self.0
    }
}

/// Storage that refuses the `n`-th check of one publication phase and no other.
///
/// One-shot on purpose: the crash it models is a moment, not a condition, so the store the
/// restarted daemon inherits is a working one — which is what lets
/// [`a_retry_after_the_restart_converges_on_the_records_that_landed`] ask whether a retry
/// converges rather than whether it fails again.
#[derive(Debug)]
struct FailNth {
    phase: PublicationPhase,
    at: u64,
    seen: AtomicU64,
}

impl FailNth {
    fn new(phase: PublicationPhase, at: u64) -> Self {
        Self {
            phase,
            at,
            seen: AtomicU64::new(0),
        }
    }
}

impl StorageFaults for FailNth {
    fn check(&self, phase: PublicationPhase) -> Result<(), AbortReason> {
        if phase != self.phase {
            return Ok(());
        }
        if self.seen.fetch_add(1, Ordering::SeqCst) + 1 == self.at {
            return Err(AbortReason::StorageExhausted);
        }
        Ok(())
    }
}

/// Storage that refuses one check of one phase, and only while the test has armed it.
///
/// [`FailNth`] targets a publication by ordinal, which is exact when the ordinal is known —
/// the records of one seal, in one dispatch. A campaign record is published several
/// publications into a longer sequence, so it is targeted by *when* instead: the test arms
/// the fault immediately before the dispatch that publishes it, and the fault disarms itself
/// on the way through. Nothing about the daemon's ordering is assumed by the fixture.
#[derive(Debug, Clone)]
struct ArmedFault {
    phase: PublicationPhase,
    armed: Arc<AtomicBool>,
}

impl ArmedFault {
    fn new(phase: PublicationPhase) -> Self {
        Self {
            phase,
            armed: Arc::new(AtomicBool::new(false)),
        }
    }

    fn arm(&self) {
        self.armed.store(true, Ordering::SeqCst);
    }
}

impl StorageFaults for ArmedFault {
    fn check(&self, phase: PublicationPhase) -> Result<(), AbortReason> {
        if phase == self.phase && self.armed.swap(false, Ordering::SeqCst) {
            return Err(AbortReason::StorageExhausted);
        }
        Ok(())
    }
}

/// An identity seam that stops agreeing with itself after `honest` calls.
///
/// The mutation control. It violates [`ContentIdentifier`]'s purity contract deliberately —
/// "two calls with equal arguments, in any process, must return equal handles" — which is
/// exactly the corruption docs/35's `identity mismatch` class exists to name: "content that
/// does not hash to the identity it is filed under. This is corruption."
#[derive(Debug)]
struct DriftingIdentity {
    honest: u64,
    seen: AtomicU64,
}

impl DriftingIdentity {
    fn new(honest: u64) -> Self {
        Self {
            honest,
            seen: AtomicU64::new(0),
        }
    }
}

impl ContentIdentifier for DriftingIdentity {
    fn identify(
        &self,
        class: ArtifactClass,
        content: &[u8],
    ) -> Result<ArtifactHandle, IdentityUnavailable> {
        let drifted = self.seen.fetch_add(1, Ordering::SeqCst) >= self.honest;
        let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
        for byte in content {
            hash ^= u64::from(*byte);
            hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
        }
        let token = if drifted {
            format!("{hash:016x}drift")
        } else {
            format!("{hash:016x}")
        };
        ArtifactHandle::new(class, &token).map_err(|_| IdentityUnavailable)
    }
}

// --- fixtures ----------------------------------------------------------------------------

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

/// The operator capability recovery runs under: `promote`, which is what
/// [`Action::Audit`](continuum_workspace::publication::Action::Audit) requires.
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
        client: "continuumd-crash-recovery-test".to_owned(),
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

/// The builder both a cold start and a restart go through, so the two daemons differ in
/// exactly one thing: whether a surviving store is adopted.
fn builder() -> Builder {
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
}

fn daemon() -> Daemon {
    builder().build()
}

fn daemon_with(faults: impl StorageFaults + 'static) -> Daemon {
    builder().store_faults(faults).build()
}

/// Restart over what a crash left behind.
fn restart(durable: DurableSubstrate) -> Daemon {
    builder().over(durable).build()
}

/// A daemon with the intent accepted, the content staged and the model registered — but no
/// workspace created, so the crash tests own the request that publishes.
///
/// Every one of those three is a [`VolatileFact`], so this is also what a restart has to
/// redo: [`the_restarted_daemon_holds_none_of_what_the_report_declares_lost`] runs it again
/// against the restarted daemon and that is not a workaround, it is the declaration being
/// true.
struct Prepared {
    daemon: Daemon,
    intent: IntentHandle,
    files: Vec<Commitment>,
    configuration: Commitment,
}

fn prepare(mut daemon: Daemon) -> Prepared {
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

    Prepared {
        daemon,
        intent,
        files,
        configuration,
    }
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

fn on(mut envelope: RequestEnvelope, snapshot: &WorkspaceHandle) -> RequestEnvelope {
    envelope.snapshot = Nullable::Value(snapshot.clone());
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

fn target(kind: TargetKind, id: &str) -> Target {
    Target {
        kind,
        id: id.to_owned(),
    }
}

// --- driving the operations --------------------------------------------------------------

/// The `workspace.create(seal: true)` request the crash tests publish through.
fn create_request(prepared: &Prepared, request: &str, key: &str) -> OperationRequest {
    OperationRequest {
        envelope: keyed(
            envelope("workspace.create", "agent:builder", "cap_builder", request),
            key,
        ),
        arguments: Arguments::WorkspaceCreate(WorkspaceCreateRequest {
            components: SnapshotComponents {
                files: prepared.files.clone(),
                cml_modules: Vec::new(),
                rust_extraction: Vec::new(),
                domain_packs: Vec::new(),
                dependencies: Vec::new(),
                epochs: SnapshotEpochs {
                    semantic: epoch("semantic-1"),
                    proof: epoch("proof-1"),
                    toolchain: Optional::Absent,
                },
                intent: prepared.intent.clone(),
                correspondence: Vec::new(),
                proof_environment: Vec::new(),
                configuration: vec![prepared.configuration.clone()],
                file_components: Optional::Absent,
            },
            overlay: Optional::Absent,
            seal: Optional::Present(true),
        }),
    }
}

/// Create and seal, expecting success, and answer with the snapshot handle.
fn seal(prepared: &mut Prepared, request: &str, key: &str) -> WorkspaceHandle {
    let created = prepared
        .daemon
        .dispatch(&create_request(prepared, request, key));
    assert_eq!(
        created.envelope.status,
        ResultStatus::Ok,
        "{:?}",
        created.envelope.error
    );
    match &created.payload {
        Payload::WorkspaceCreate(response) => response.snapshot.clone(),
        other => panic!("expected a workspace.create payload, got {other:?}"),
    }
}

/// `verification.start` over a sealed snapshot, under a `states` ceiling.
fn start(
    prepared: &mut Prepared,
    snapshot: &WorkspaceHandle,
    request: &str,
    key: &str,
    states: u64,
) -> continuumd::daemon::OperationOutcome {
    let mut request_envelope = keyed(
        envelope("verification.start", "agent:runner", "cap_runner", request),
        key,
    );
    request_envelope.budget = Optional::Present(budget(states));
    prepared.daemon.dispatch(&OperationRequest {
        envelope: on(request_envelope, snapshot),
        arguments: Arguments::VerificationStart(VerificationStartRequest {
            target: target(TargetKind::AllClaims, "DieHard"),
            portfolio: Portfolio::Interactive,
            context_policy: Optional::Absent,
            priority_class: Optional::Absent,
        }),
    })
}

fn started_task(outcome: &continuumd::daemon::OperationOutcome) -> TaskHandle {
    match &outcome.payload {
        Payload::VerificationStart(response) => response
            .task
            .value()
            .cloned()
            .expect("a fresh start names a task"),
        other => panic!("expected a verification.start payload, got {other:?}"),
    }
}

/// The store identity of a task's committed publications, read off the daemon's own record.
///
/// A publication's identity is the `commitment` on its `ArtifactRef`, not the `handle`: the
/// handle names the *task* the publication belongs to, and "two publications of one task are
/// two refs with one handle and two commitments".
fn committed(daemon: &Daemon, task: &TaskHandle) -> Vec<ArtifactHandle> {
    daemon
        .state()
        .tasks()
        .get(task)
        .expect("the task is held")
        .evidence
        .committed()
        .iter()
        .map(|publication| {
            let text = publication.commitment().as_str();
            ArtifactHandle::new(
                ArtifactClass::Task,
                text.strip_prefix(ArtifactClass::Task.prefix())
                    .expect("a publication commitment is a `task_` handle"),
            )
            .expect("a `task_` handle")
        })
        .collect()
}

fn report(store: &ReferenceStore) -> RecoveryReport {
    recovery::recover(store, &operator()).expect("`cap_root` confers audit")
}

// --- A. the crash-point catalogue ---------------------------------------------------------

/// **The durable image is a step function of the dispatch boundary.**
///
/// Nine boundaries, one daemon each, one `workspace.create(seal: true)` request each. Steps
/// 1–7 are volatile-side work — version negotiation, the registry lookup, shape agreement,
/// the scope claim, admission, the annotation obligations, the idempotency ledger — and step
/// 8 is the only step that reaches the store. So the recovery report after a kill is the
/// *same bytes* for the eight boundaries before the handler and different for the one after
/// it, and [`CrashPoint::reaches_the_store`] predicts which is which.
///
/// This is what makes instrumenting all nine load-bearing rather than decorative: a handler
/// that reached the store from step 5, or a step that published a side effect on its way
/// through, would show up here as two boundaries disagreeing.
#[test]
fn the_durable_image_is_a_step_function_of_the_dispatch_boundary() {
    let renders: Vec<(CrashPoint, String)> = CrashPoint::ALL
        .into_iter()
        .map(|point| {
            let mut prepared = prepare(daemon());
            let request = create_request(&prepared, "req_create", "idem-create");
            let killed = prepared
                .daemon
                .dispatch_or_die(&request, &KillAt(point))
                .expect_err("the injector kills at this boundary");
            assert_eq!(killed.point, point);
            let durable = prepared.daemon.crash();
            (point, report(durable.store()).render())
        })
        .collect();

    let before: Vec<&String> = renders
        .iter()
        .filter(|(point, _)| !point.reaches_the_store())
        .map(|(_, render)| render)
        .collect();
    assert_eq!(before.len(), 8, "eight boundaries precede the handler");
    for render in &before {
        assert_eq!(
            render.as_bytes(),
            before[0].as_bytes(),
            "no step before the handler writes to the store"
        );
    }

    let after = renders
        .iter()
        .find(|(point, _)| point.reaches_the_store())
        .map(|(_, render)| render)
        .expect("one boundary follows the handler");
    assert_ne!(
        after.as_bytes(),
        before[0].as_bytes(),
        "a vacuous agreement proves nothing: the handler must have published"
    );
    assert!(
        after.contains("class ws published="),
        "the seal published the workspace's records: {after}"
    );
}

/// A daemon killed mid-dispatch answers nothing, and the answer is not an error code.
///
/// The refusal shape is deliberate: [`Killed`](recovery::Killed) carries the boundary and no
/// envelope, because a process that is gone sends no frame and the protocol fixes no code for
/// one that is not there.
#[test]
fn a_daemon_killed_mid_dispatch_answers_nothing_at_all() {
    for point in CrashPoint::ALL {
        let mut prepared = prepare(daemon());
        let request = create_request(&prepared, "req_create", "idem-create");
        let outcome = prepared.daemon.dispatch_or_die(&request, &KillAt(point));
        assert_eq!(
            outcome.map(|_| ()).unwrap_err().point,
            point,
            "the kill names the boundary it happened at"
        );
    }
    // The control: the same request under a daemon that does not crash is answered.
    let mut prepared = prepare(daemon());
    let request = create_request(&prepared, "req_create", "idem-create");
    let answered = prepared
        .daemon
        .dispatch_or_die(&request, &recovery::NoCrash)
        .expect("`NoCrash` kills at no boundary");
    assert_eq!(answered.envelope.status, ResultStatus::Ok);
}

// --- B. mid-publication (INV-017) ---------------------------------------------------------

/// **INV-017.** A crash between the content commit and the index commit leaves unreachable
/// content, never a stale index entry.
///
/// > Publication is ordered: content MUST be committed before the index entry that names it.
/// > A crash between the two therefore leaves unreachable content, which is
/// > garbage-collectable, and never a stale index entry pointing at content that was never
/// > written.
/// >
/// > — `docs/35`, "Crash safety and the index verifier"
///
/// The store proves its own half (G0-DX-13); what this asserts is the *daemon-scale*
/// composition — the fault fires inside step 8 of a real dispatch, the daemon is then killed
/// whole, and the reconciliation is run by a restart rather than by the process that wrote
/// the store.
#[test]
fn a_crash_between_the_two_commits_leaves_residue_and_no_stale_index_entry() {
    let mut prepared = prepare(daemon_with(FailNth::new(
        PublicationPhase::CommittingIndex,
        2,
    )));
    let created = prepared
        .daemon
        .dispatch(&create_request(&prepared, "req_create", "idem-create"));
    assert_eq!(created.envelope.status, ResultStatus::Error);
    assert_eq!(
        created.error_code(),
        Some(ErrorCode::PublicationAborted),
        "the composite root was not published and no record was truncated; the record \
         before the fault stays published (RFC 0026, correction 48)"
    );

    let durable = prepared.daemon.crash();
    let report = report(durable.store());

    assert_eq!(report.verdict(), Verdict::Residue);
    assert_eq!(
        report.quarantined().len(),
        1,
        "one record committed its content and never its index entry"
    );
    for defect in report.defects() {
        assert!(
            matches!(defect, StoreDefect::UnreachableContent(_)),
            "the only defect class a crash may produce is residue, found {defect}"
        );
    }
    assert!(
        report.receiptless().is_empty(),
        "an index entry and the receipt naming it are written in one critical section"
    );
    assert!(
        report.orphan_receipts().is_empty(),
        "a receipt for an unpublished identity would be a forged receipt"
    );
    assert_eq!(
        report.reconciled().len(),
        1,
        "the record that landed before the fault resolves; the one caught by it does not"
    );
}

/// The report *reads the store*: the two publication phases produce different reports.
///
/// The anti-vacuity control for the test above. A crash at the content commit writes nothing
/// at all, so it leaves no residue; a crash at the index commit leaves exactly one
/// unreachable record. A report that printed a constant could not tell them apart.
#[test]
fn the_report_distinguishes_the_two_publication_phases() {
    let render_after = |phase| {
        let mut prepared = prepare(daemon_with(FailNth::new(phase, 1)));
        let created =
            prepared
                .daemon
                .dispatch(&create_request(&prepared, "req_create", "idem-create"));
        assert_eq!(created.envelope.status, ResultStatus::Error);
        let durable = prepared.daemon.crash();
        report(durable.store())
    };

    let content = render_after(PublicationPhase::CommittingContent);
    let index = render_after(PublicationPhase::CommittingIndex);

    assert_eq!(content.verdict(), Verdict::Clean);
    assert!(content.quarantined().is_empty());
    assert_eq!(index.verdict(), Verdict::Residue);
    assert_eq!(index.quarantined().len(), 1);
    assert_ne!(content.render().as_bytes(), index.render().as_bytes());
}

/// **No half-published artifact.** A composite caught mid-publication is absent under its own
/// name.
///
/// A sealed workspace is N records published children before parents, so the descriptor root
/// is published *last*. A crash part-way through therefore leaves the root absent: the
/// composite is not readable under the identity a caller would fetch it by, and there is no
/// state in which half of it is.
#[test]
fn a_partly_sealed_workspace_is_absent_under_its_own_name() {
    let mut prepared = prepare(daemon_with(FailNth::new(
        PublicationPhase::CommittingIndex,
        2,
    )));
    let created = prepared
        .daemon
        .dispatch(&create_request(&prepared, "req_create", "idem-create"));
    assert_eq!(created.envelope.status, ResultStatus::Error);

    // What the root *would* have been, derived the way a caller would derive it: from the
    // successful seal of the identical components on a healthy daemon.
    let mut healthy = prepare(daemon());
    let root = seal(&mut healthy, "req_create", "idem-create");
    let root_handle = continuumd::daemon::identity::workspace_to_store(&root)
        .expect("a `ws_` handle crosses to the store");

    let durable = prepared.daemon.crash();
    let report = report(durable.store());
    assert!(
        !report
            .surviving(ArtifactClass::WorkspaceSnapshot)
            .contains(&root_handle),
        "the composite's root is not published, so the composite is not readable"
    );
    assert!(
        durable.store().read(&root_handle, &operator()).is_err(),
        "and a read of it is refused, not half-served"
    );
    // The healthy control: on the daemon whose store was never injured, it *is* readable —
    // so the assertion above is about the crash, not about the handle.
    assert!(
        healthy
            .daemon
            .store()
            .read(&root_handle, &operator())
            .is_ok()
    );
}

/// The records that did land are complete artifacts, and a retry after the restart converges
/// on them — residue included.
///
/// This is the other half of "no half-published artifact": what survives a partial seal is
/// not debris. Each published record is a complete, content-addressed artifact in its own
/// right — the store derived its identity from its bytes — so the retry that follows the
/// restart re-publishes the same identities and converges. The receipt ledger records both
/// publications of the record that landed rather than losing one.
///
/// The residue converges too, and that is worth naming rather than glossing: the record the
/// crash caught between its two commits already had its content durable, so the retry's
/// content commit converges onto those exact bytes and its index commit is the entry that
/// was missing. What was unreachable becomes published — the same identity, not a second copy
/// — and the store ends up `clean` rather than carrying a duplicate. RFC 0026's "the retry is
/// a fresh publication, not a resumption of a partial one" is about the *protocol*: no state
/// is resumed, and convergence on content already held is the store's ordinary behaviour.
#[test]
fn a_retry_after_the_restart_converges_on_the_records_that_landed() {
    let mut prepared = prepare(daemon_with(FailNth::new(
        PublicationPhase::CommittingIndex,
        2,
    )));
    let created = prepared
        .daemon
        .dispatch(&create_request(&prepared, "req_create", "idem-create"));
    assert_eq!(created.envelope.status, ResultStatus::Error);

    let durable = prepared.daemon.crash();
    let before = report(durable.store());
    let landed = before.surviving(ArtifactClass::WorkspaceSnapshot);
    assert_eq!(landed.len(), 1);
    let residue = before.quarantined().to_vec();
    assert_eq!(residue.len(), 1);

    // The restart, and the deployment re-provisioning what the crash lost.
    let mut prepared = prepare(restart(durable));
    let root = seal(&mut prepared, "req_retry", "idem-retry");

    let durable = prepared.daemon.crash();
    let after = report(durable.store());
    let published = after.surviving(ArtifactClass::WorkspaceSnapshot);
    for handle in &landed {
        assert!(
            published.contains(handle),
            "the retry converged on the record that had already landed"
        );
    }
    assert!(
        published.contains(
            &continuumd::daemon::identity::workspace_to_store(&root)
                .expect("a `ws_` handle crosses to the store")
        ),
        "and the root is published now"
    );
    assert_eq!(
        after
            .receipts()
            .iter()
            .filter(|(handle, _)| landed.contains(handle))
            .map(|(_, count)| *count)
            .collect::<Vec<u64>>(),
        vec![2],
        "a converging publication is issued its own receipt; no receipt is lost to convergence"
    );
    assert_eq!(
        after
            .receipts()
            .iter()
            .filter(|(handle, _)| residue.contains(handle))
            .map(|(_, count)| *count)
            .collect::<Vec<u64>>(),
        vec![1],
        "the aborted publication issued none, so the retry's is the only receipt the record \
         that was residue carries"
    );
    assert!(
        published.contains(&residue[0]),
        "the residue converged: the same identity, now named by the index entry that was \
         missing"
    );
    assert_eq!(
        after.verdict(),
        Verdict::Clean,
        "so the retry left no duplicate and no leftover"
    );
}

// --- C. IMPL-02's durability half ---------------------------------------------------------

/// **IMPL-02, first conjunct.** Committed partial evidence survives daemon restart.
///
/// > committed partial evidence survives daemon restart; uncommitted partials are absent,
/// > never half-visible
/// >
/// > — PR 6 / IMPL-02
///
/// bn-23j7s landed the identity half — a publication is *named* by the content identity of
/// the campaign that produced it, and two daemons name it identically. This is the durable
/// half: the record those bytes are of is published into the store, so after the daemon is
/// killed the identity still resolves, is receipted, and reads back.
///
/// What does **not** survive is the task record, and the report says so rather than implying
/// otherwise: [`VolatileFact::TaskTable`] is `declared`, and the restarted daemon denies
/// `task.status` on the handle. That is docs/35's own rule — a restart "MUST NOT reconstruct
/// task state by inference" — and it is the honest scope of this conjunct: the *evidence*
/// survives, which is what the criterion names.
#[test]
fn a_parked_tasks_committed_publication_survives_the_crash() {
    let mut prepared = prepare(daemon());
    let snapshot = seal(&mut prepared, "req_create", "idem-create");
    let outcome = start(&mut prepared, &snapshot, "req_start", "idem-start", 4);
    let task = started_task(&outcome);
    assert_eq!(
        prepared
            .daemon
            .state()
            .tasks()
            .get(&task)
            .expect("the task is held")
            .status,
        TaskStatus::Suspended,
        "a four-state ceiling parks the Die Hard campaign"
    );
    let published = committed(&prepared.daemon, &task);
    assert_eq!(
        published.len(),
        1,
        "the parked run committed one publication"
    );

    let durable = prepared.daemon.crash();
    let report = report(durable.store());

    let surviving = report.surviving(ArtifactClass::Task);
    assert_eq!(
        surviving, published,
        "the committed publication is in the store under the identity the task named it by"
    );
    assert!(
        durable.store().read(&published[0], &operator()).is_ok(),
        "and it reads back"
    );
    assert_eq!(
        report
            .receipts()
            .iter()
            .find(|(handle, _)| handle == &published[0])
            .map(|(_, count)| *count),
        Some(1),
        "with the receipt its publication issued"
    );
    assert_eq!(
        VolatileFact::TaskTable.disposition(),
        Disposition::Declared,
        "the record itself is declared lost, not reconstructed"
    );
}

/// **IMPL-02, second conjunct.** An uncommitted partial is absent after recovery, never
/// half-visible.
///
/// The fault fires at the campaign record's index commit, so the content is durable and no
/// index entry names it. Three things must then be true at once and are: the caller was told
/// `PublicationAborted` rather than being handed a task that claims evidence, the task
/// committed nothing, and after the crash the identity is residue — quarantined, unreadable,
/// unreceipted — rather than an artifact anyone can reach.
#[test]
fn an_uncommitted_partial_is_absent_after_recovery() {
    // What the campaign record's identity would have been, taken from a healthy daemon that
    // ran the identical campaign — the identity function is process-independent
    // (`two_daemons_name_one_campaigns_publications_identically`), so this is the name the
    // injured daemon staged too.
    let mut healthy = prepare(daemon());
    let snapshot = seal(&mut healthy, "req_create", "idem-create");
    let outcome = start(&mut healthy, &snapshot, "req_start", "idem-start", 4);
    let expected = committed(&healthy.daemon, &started_task(&outcome));
    assert_eq!(expected.len(), 1);

    let fault = ArmedFault::new(PublicationPhase::CommittingIndex);
    let mut prepared = prepare(daemon_with(fault.clone()));
    let snapshot = seal(&mut prepared, "req_create", "idem-create");
    fault.arm();
    let refused = start(&mut prepared, &snapshot, "req_start", "idem-start", 4);
    assert_eq!(refused.envelope.status, ResultStatus::Error);
    assert_eq!(refused.error_code(), Some(ErrorCode::PublicationAborted));
    assert!(
        prepared
            .daemon
            .state()
            .tasks()
            .handles()
            .iter()
            .all(|handle| prepared
                .daemon
                .state()
                .tasks()
                .get(handle)
                .is_some_and(|entry| entry.evidence.committed().is_empty())),
        "the task committed nothing: a publication the store refused is not one the task \
         claims"
    );

    let durable = prepared.daemon.crash();
    let report = report(durable.store());
    assert!(
        report.surviving(ArtifactClass::Task).is_empty(),
        "an aborted publication names no published artifact"
    );
    assert!(
        durable.store().read(&expected[0], &operator()).is_err(),
        "and it is not readable: absent, never half-visible"
    );
    assert!(
        report
            .receipts()
            .iter()
            .all(|(handle, _)| handle != &expected[0]),
        "no receipt was issued for it"
    );
    assert!(
        report.quarantined().contains(&expected[0]),
        "what is left is residue, named as residue"
    );
}

/// **No orphan task.** Every `task_*` identity that survives is index-resolved and receipted.
///
/// "Orphan" at this grain has two readings and both are checked: no surviving task artifact
/// is dangling (each resolves through the index and carries its receipts), and no task
/// *record* is fabricated to go with one — the restarted daemon holds no tasks and says so.
#[test]
fn every_surviving_task_artifact_is_reconciled_and_receipted() {
    let mut prepared = prepare(daemon());
    let snapshot = seal(&mut prepared, "req_create", "idem-create");
    let first = started_task(&start(
        &mut prepared,
        &snapshot,
        "req_start",
        "idem-start",
        4,
    ));
    let second = started_task(&start(
        &mut prepared,
        &snapshot,
        "req_start_2",
        "idem-start-2",
        6,
    ));
    assert_ne!(first, second, "two budgets are two tasks");

    let durable = prepared.daemon.crash();
    let report = report(durable.store());
    let surviving = report.surviving(ArtifactClass::Task);
    assert_eq!(surviving.len(), 2, "one publication per parked campaign");
    for handle in &surviving {
        assert!(
            report
                .reconciled()
                .iter()
                .any(|(_, indexed)| indexed == handle),
            "a surviving task artifact resolves through the index"
        );
        assert!(
            report
                .receipts()
                .iter()
                .any(|(receipted, count)| receipted == handle && *count >= 1),
            "and carries the receipt its publication issued"
        );
    }
    assert!(report.receiptless().is_empty());
    assert!(report.orphan_receipts().is_empty());
}

// --- D. the volatile declaration is real --------------------------------------------------

/// The restarted daemon holds none of what the report declares lost.
///
/// The declaration is checked behaviourally rather than by reading a list back: the task the
/// pre-crash daemon parked is denied, the continuation it minted is denied, and the
/// idempotency key that would have replayed does not — it runs afresh. Each of those is one
/// [`VolatileFact`] observed through `Daemon::dispatch`.
#[test]
fn the_restarted_daemon_holds_none_of_what_the_report_declares_lost() {
    let mut prepared = prepare(daemon());
    let snapshot = seal(&mut prepared, "req_create", "idem-create");
    let outcome = start(&mut prepared, &snapshot, "req_start", "idem-start", 4);
    let task = started_task(&outcome);
    let continuation: ContinuationHandle = prepared
        .daemon
        .state()
        .tasks()
        .get(&task)
        .expect("the task is held")
        .continuation
        .clone()
        .expect("a parked task has one");

    let durable = prepared.daemon.crash();
    let report = report(durable.store());
    let mut restarted = restart(durable);

    // TaskTable.
    let status = restarted.dispatch(&OperationRequest {
        envelope: envelope("task.status", "agent:runner", "cap_runner", "req_status"),
        arguments: Arguments::TaskStatus(TaskStatusRequest { task: task.clone() }),
    });
    assert_eq!(status.error_code(), Some(ErrorCode::CapabilityDenied));

    // ContinuationTable. `task.resume` is `@task_starting`, so the envelope carries a budget
    // — otherwise the refusal would be step 6's `MalformedRequest` and would say nothing
    // about whether the continuation survived.
    let mut resume_envelope = keyed(
        envelope("task.resume", "agent:runner", "cap_runner", "req_resume"),
        "idem-resume",
    );
    resume_envelope.budget = Optional::Present(budget(8));
    let resumed = restarted.dispatch(&OperationRequest {
        envelope: resume_envelope,
        arguments: Arguments::TaskResume(TaskResumeRequest {
            continuation,
            budget: Optional::Absent,
        }),
    });
    assert_eq!(resumed.error_code(), Some(ErrorCode::CapabilityDenied));

    // IdempotencyLedger, StagedContent, ModelCatalog, IntentRegistry: the same key that
    // recorded a reply before the crash records nothing now, and the request only reaches the
    // handler at all because the deployment re-provisioned the out-of-band surfaces.
    let mut prepared = prepare(restarted);
    let again = seal(&mut prepared, "req_create", "idem-create");
    assert_eq!(again, snapshot, "content addressing is not process-scoped");

    // And the declaration itself names all of them.
    let declared: Vec<&str> = report
        .volatile()
        .iter()
        .map(|fact| fact.token())
        .collect::<Vec<_>>();
    for token in [
        "task-table",
        "continuation-table",
        "idempotency-ledger",
        "staged-content",
        "model-catalog",
        "intent-registry",
    ] {
        assert!(declared.contains(&token), "{token} is declared volatile");
    }
    assert_eq!(
        report.volatile().len(),
        VolatileFact::ALL.len(),
        "the report carries the whole declaration, not a subset"
    );
}

// --- E. determinism -----------------------------------------------------------------------

/// Two recoveries from one crashed state render byte-identically.
///
/// The acceptance criterion's own words. `recover` reads the store and nothing else — no
/// clock, no counter, no iteration order that depends on insertion history — so running it
/// twice is running one function twice.
#[test]
fn two_recoveries_from_one_crashed_state_render_byte_identically() {
    let mut prepared = prepare(daemon_with(FailNth::new(
        PublicationPhase::CommittingIndex,
        2,
    )));
    let created = prepared
        .daemon
        .dispatch(&create_request(&prepared, "req_create", "idem-create"));
    assert_eq!(created.envelope.status, ResultStatus::Error);
    let durable = prepared.daemon.crash();

    let first = report(durable.store()).render();
    let second = report(durable.store()).render();
    assert_eq!(first.as_bytes(), second.as_bytes());
    assert!(
        first.contains("quarantined "),
        "a vacuous agreement proves nothing: there is something to disagree about"
    );
    assert!(first.contains("verdict residue"));
}

/// Two independently crashed daemons that ran one campaign recover identically.
///
/// The two-fresh-daemons device (`rule ordering.deterministic`) applied to recovery: two
/// daemons built separately, each with its own store, each running the same sequence and each
/// killed, and the recovery report is one value. Nothing about the process that wrote the
/// store reaches the report.
#[test]
fn two_independently_crashed_daemons_recover_identically() {
    let run = || {
        let mut prepared = prepare(daemon());
        let snapshot = seal(&mut prepared, "req_create", "idem-create");
        let _ = start(&mut prepared, &snapshot, "req_start", "idem-start", 4);
        let durable = prepared.daemon.crash();
        report(durable.store()).render()
    };
    let first = run();
    let second = run();
    assert_eq!(first.as_bytes(), second.as_bytes());
    assert!(
        first.contains("class task published=1"),
        "a vacuous agreement proves nothing: {first}"
    );
}

// --- F. recovery reports, and never deletes -----------------------------------------------

/// Recovery reports residue and never deletes it; reclaiming is a separate operator action.
///
/// > What is left over — content that no index entry names and no live publication is holding
/// > — is crash residue, and only that is reclaimed.
/// >
/// > — `ReferenceStore::collect_garbage`
///
/// A recovery that silently reclaimed would destroy the evidence that a crash happened at
/// all, so `recover` takes `&ReferenceStore` and calls no mutating method. The reclaim is
/// `collect_garbage`, under its own capability, and this test runs it *after* the recovery to
/// show the two are separable and that the residue the report named is exactly what it takes.
#[test]
fn recovery_reports_residue_and_never_deletes_it() {
    let mut prepared = prepare(daemon_with(FailNth::new(
        PublicationPhase::CommittingIndex,
        2,
    )));
    let created = prepared
        .daemon
        .dispatch(&create_request(&prepared, "req_create", "idem-create"));
    assert_eq!(created.envelope.status, ResultStatus::Error);
    let durable = prepared.daemon.crash();

    let first = report(durable.store());
    let residue = first.quarantined().to_vec();
    assert_eq!(residue.len(), 1);

    // Twice, to make "never deletes" a statement about repetition rather than about one call.
    let second = report(durable.store());
    assert_eq!(second.quarantined(), residue.as_slice());

    let reclaimed = durable
        .store()
        .collect_garbage(&operator())
        .expect("`cap_root` confers administration");
    assert_eq!(
        reclaimed, residue,
        "the operator's reclaim takes exactly what the report named"
    );

    let third = report(durable.store());
    assert_eq!(third.verdict(), Verdict::Clean);
    assert!(third.quarantined().is_empty());
}

// --- G. controls and mutants ---------------------------------------------------------------

/// **Control.** Without the injected fault the same assertions flip.
///
/// The withheld-mirror control: every negative assertion in the INV-017 tests above is run
/// again against a daemon whose store was never injured, and every one of them comes out the
/// other way. A crash test whose assertions also hold when nothing crashed is a test of
/// nothing.
#[test]
fn control_without_the_fault_the_seal_completes_and_the_verdict_is_clean() {
    let mut prepared = prepare(daemon());
    let root = seal(&mut prepared, "req_create", "idem-create");
    let durable = prepared.daemon.crash();
    let report = report(durable.store());

    assert_eq!(report.verdict(), Verdict::Clean);
    assert!(report.quarantined().is_empty());
    assert!(report.defects().is_empty());
    assert!(
        report.surviving(ArtifactClass::WorkspaceSnapshot).contains(
            &continuumd::daemon::identity::workspace_to_store(&root)
                .expect("a `ws_` handle crosses to the store")
        ),
        "the composite's root *is* published when nothing crashed"
    );
    assert!(!report.reconciled().is_empty());
    assert!(report.receiptless().is_empty());
}

/// **Control.** The receipt count is read from the ledger, not asserted.
///
/// G0-DX-13's "no lost receipts" clause after a crash. Sealing the same components twice
/// publishes each record twice and converges; the second publication is issued its own
/// receipt, so the count the report carries is 2. A `recover` that printed a constant, or
/// that counted index entries instead of receipts, would answer 1.
#[test]
fn control_the_receipt_count_is_read_from_the_ledger() {
    let mut prepared = prepare(daemon());
    let root = seal(&mut prepared, "req_create", "idem-create");
    let again = seal(&mut prepared, "req_create_2", "idem-create-2");
    assert_eq!(root, again, "identical components converge on one identity");

    let durable = prepared.daemon.crash();
    let report = report(durable.store());
    assert!(
        report.receipts().iter().all(|(_, count)| *count == 2),
        "every record was published twice: {:?}",
        report.receipts()
    );
    assert!(!report.receipts().is_empty());
}

/// **Mutant.** A corrupted identity seam is caught, and it flips the verdict to `defective`.
///
/// > identity mismatch — content that does not hash to the identity it is filed under. This
/// > is corruption. The daemon MUST report it and MUST NOT silently repair it.
/// >
/// > — `docs/35`
///
/// [`DriftingIdentity`] violates [`ContentIdentifier`]'s purity contract after a fixed number
/// of calls, which is the only way this corruption is reachable: the store never rewrites
/// content, so an identity mismatch cannot be produced by any legal sequence of operations.
/// The control is the same store built with the same identity seam set never to drift, which
/// recovers `clean` — so the catch is the mutation's, not the fixture's.
#[test]
fn mutant_a_corrupted_identity_seam_flips_the_verdict_to_defective() {
    use continuum_workspace::publication::{
        ActorId as StoreActor, AuditLog, AuthorityLevel as StoreLevel,
        CapabilityDescriptor as StoreDescriptor, ReferenceStore,
    };

    let build = |honest: u64| {
        let store = ReferenceStore::builder(DriftingIdentity::new(honest), AuditLog::new())
            .capability(StoreDescriptor::new(
                operator(),
                StoreActor::new("service:continuumd"),
                StoreLevel::Promote,
            ))
            .build();
        store
            .publish(ArtifactClass::Evidence, b"a trace".to_vec(), &operator())
            .expect("the operator may publish");
        store
    };

    // The control: the seam never drifts, so fsck re-derives the identity it published under.
    let faithful = build(u64::MAX);
    let clean = recovery::recover(&faithful, &operator()).expect("audit");
    assert_eq!(clean.verdict(), Verdict::Clean);
    assert!(clean.defects().is_empty());

    // The mutant: the seam is honest for the publication and drifts before the verification.
    let corrupt = build(1);
    let defective = recovery::recover(&corrupt, &operator()).expect("audit");
    assert_eq!(defective.verdict(), Verdict::Defective);
    assert!(
        defective
            .defects()
            .iter()
            .any(|defect| matches!(defect, StoreDefect::IdentityMismatch(_))),
        "the mismatch is named, not repaired: {:?}",
        defective.defects()
    );
    assert!(
        defective.reconciled().is_empty(),
        "an entry whose content does not identify to it is not reconciled"
    );
    assert_ne!(clean.render().as_bytes(), defective.render().as_bytes());
}
