//! **PHASE-A-DEL-03 (bn-cho5): the task lifecycle under enumerated dispatch schedules,
//! composed with the crash seam.**
//!
//! `Daemon::dispatch` holds `&mut DaemonState` for a whole call (INV-005: no ambient
//! scheduling; the daemon's concurrency is reached through its capability seams), so at
//! this grain "concurrency" is precisely **which order the dispatches of independent
//! clients land in** — and that order is a value a test can enumerate, exactly as
//! `crates/continuum-task/src/region/schedule.rs` states for the calculus one crate down.
//! This file enumerates shuffle products of per-lane dispatch programs and asserts, over
//! every member:
//!
//! - **commutation** — the finished daemon is a function of the programs, not of the
//!   interleaving ([`positive_two_lane_interleavings_commute_and_leave_no_orphans`]);
//! - **cancellation anywhere** — `task.cancel` spliced at every schedule position leaves
//!   the *other* lane untouched, publications intact (INV-009), and the region ledger
//!   total ([`positive_cancellation_at_every_position_isolates_the_other_lane`]);
//! - **duplicate coalescing** — a duplicate `verification.start` woven anywhere resolves
//!   to the same task, opens no second scope, and changes nothing observable
//!   ([`positive_a_duplicate_start_at_every_position_coalesces_to_one_task`]);
//! - **restart boundaries** — a crash at every one of the nine dispatch boundaries at
//!   every position of the lifecycle leaves a recoverable store with no half-publication,
//!   and a restarted daemon resumes (or, with nothing parked, re-runs, or, with the task
//!   already `Completed`, restores) the lifecycle to the byte-identical record
//!   ([`positive_a_crash_at_every_boundary_of_every_position_restarts_to_the_same_record`]).
//!
//! # How this composes with what already exists, rather than duplicating it
//!
//! - `tests/dx14_cancellation_matrix.rs` cancels each of four lanes at every phase of its
//!   *own* program, one lane per daemon (plus one sequential portfolio). This file's
//!   marginal claim is about **interleaving**: cancellation landing at every position of
//!   a *woven two-lane schedule*, with the surviving lane's outcome pinned byte-for-byte.
//! - `tests/g1_crash_recovery_evidence.rs` (bn-3dr) proves the durable image is a step
//!   function of the nine boundaries for one dispatch, and that recovery classifies what
//!   survives. This file reuses that seam unchanged — [`CrashPoint::ALL`], a local
//!   `KillAt`, [`Daemon::crash`], `Builder::over`, [`recovery::recover`] — and adds the
//!   *schedule* axis: the crash lands at every boundary of every position of a running
//!   lifecycle, and the claim is extended to what a restart can then *do* (re-run the
//!   lifecycle to the identical record), not only to what it holds.
//! - the in-flight store window (a crash *between* `commit_content` and `commit_index`)
//!   is the store seam's business, proved in
//!   `crates/continuum-workspace/tests/publication_schedule_matrix.rs` at the phase grain
//!   and in g1's store-seam composition at this one; it is deliberately not re-proved here.
//!
//! # House rules, inherited unchanged
//!
//! - assertions are on outcomes of a written-down schedule, never on timing;
//! - every sweep carries an anti-vacuity guard (space sizes asserted as literals; a sweep
//!   whose members all produced one outcome fails);
//! - bounded by construction: 20 + 140 + 140 + 27 + 27 + 40 runs, nothing built by doubling;
//! - `src/` is untouched; the daemon is driven through `dispatch`/`dispatch_or_die` only.
//!
//! # The harness
//!
//! The fixtures are `dx14_cancellation_matrix.rs`'s and `g1_crash_recovery_evidence.rs`'s,
//! duplicated locally rather than imported, because a `tests/*.rs` file is its own crate
//! and nothing here can `use` a sibling one — the same reason `pr8_exit_evidence.rs`
//! gives. The Die Hard model and contract are `include_str!`'d from the one copy of each
//! in this repository, so no fixture here can drift from the corpus.

use std::collections::BTreeSet;

use continuum_engine_reference::diehard;
use continuum_intent::canonical_json::Json;
use continuum_intent::contract::IntentContract;
use continuum_value::epoch::ProtocolWindow;
use continuum_workspace::publication::CapabilityToken;
use continuum_workspace::snapshot::WorkspacePath;
use continuumd::daemon::family::{Arguments, Payload};
use continuumd::daemon::identity::Blake3Identity;
use continuumd::daemon::intent::IntentFamily;
use continuumd::daemon::recovery::{CrashInjector, CrashPoint, Resolution, Verdict};
use continuumd::daemon::region::TaskRegions;
use continuumd::daemon::state::{IntentRecord, RegistryStatus};
use continuumd::daemon::task::TaskFamily;
use continuumd::daemon::verification::{VerificationFamily, model_source};
use continuumd::daemon::workspace::WorkspaceFamily;
use continuumd::daemon::{
    Builder, Daemon, DurableSubstrate, OperationOutcome, OperationRequest, recovery,
};
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
    AuthorityLevel, Encoding, Portfolio, ResultStatus, TargetKind, TaskStatus,
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

// --- the two lanes -------------------------------------------------------------------------

/// One dispatch a lane's program makes — `dx14_cancellation_matrix.rs`'s vocabulary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Beat {
    /// `verification.start` under a state ceiling. Opens one scope.
    Start(u64),
    /// `task.resume` under a ceiling. Opens one scope.
    Resume(u64),
    /// `task.update_budget`: re-admit under a larger bound without re-running. Opens none.
    Rebudget(u64),
}

/// A dispatch program: a task shape written down as beats.
#[derive(Debug)]
struct Lane {
    token: &'static str,
    kind: TargetKind,
    id: &'static str,
    beats: &'static [Beat],
    /// How many scopes this lane's whole program opens when it creates its own task.
    opens: u32,
}

impl Lane {
    fn target(&self) -> Target {
        Target {
            kind: self.kind,
            id: self.id.to_owned(),
        }
    }
}

/// The `dpor` shape: a frontier committed per round — publications 1 → 2 → 3, resumable
/// at every intermediate step. The lane every splice in this file targets.
const DPOR: Lane = Lane {
    token: "dpor",
    kind: TargetKind::AllClaims,
    id: "DieHard",
    beats: &[Beat::Start(4), Beat::Resume(8), Beat::Resume(64)],
    opens: 3,
};

/// The `proof` shape: parked between lemmas and re-admitted under a larger ceiling
/// without re-running — the lane whose middle beat changes the budget and not the work.
const PROOF: Lane = Lane {
    token: "proof",
    kind: TargetKind::Property,
    id: "TypeOK",
    beats: &[Beat::Start(4), Beat::Rebudget(64), Beat::Resume(64)],
    opens: 2,
};

// --- fixtures, forked from `dx14_cancellation_matrix.rs` / `g1_crash_recovery_evidence.rs` --

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

/// The operator capability recovery runs under: `promote`, which is what `Action::Audit`
/// requires.
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
        client: "continuumd-lifecycle-schedule-test".to_owned(),
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

/// The builder a cold start and a restart both go through, so the two daemons differ in
/// exactly one thing: whether a surviving store is adopted (`g1_crash_recovery_evidence.rs`).
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
                "cap_reader",
                "agent:reader",
                AuthorityLevel::Read,
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

/// Restart over what a crash left behind.
fn restart(durable: DurableSubstrate) -> Daemon {
    builder().over(durable).build()
}

struct Fixture {
    daemon: Daemon,
    snapshot: WorkspaceHandle,
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

/// Prepare `daemon` into the fixture every sweep runs over: intent accepted, content
/// staged, model registered, workspace sealed. Every one of these is a
/// [`recovery::VolatileFact`], which is why this runs *again* over a restarted daemon in
/// the crash sweep — that is the volatile declaration being true, not a workaround
/// (`g1_crash_recovery_evidence.rs` proves the refusals; this file needs the redo).
fn fixture_over(mut daemon: Daemon) -> Fixture {
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
    assert_eq!(
        created.envelope.status,
        ResultStatus::Ok,
        "{:?}",
        created.envelope.error
    );
    let snapshot = match &created.payload {
        Payload::WorkspaceCreate(response) => response.snapshot.clone(),
        other => panic!("expected a workspace.create payload, got {other:?}"),
    };

    Fixture { daemon, snapshot }
}

fn fixture() -> Fixture {
    fixture_over(daemon())
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

fn budgeted(mut envelope: RequestEnvelope, states: Option<u64>) -> RequestEnvelope {
    envelope.budget = Optional::Present(budget(states));
    envelope
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

fn on(mut envelope: RequestEnvelope, snapshot: &WorkspaceHandle) -> RequestEnvelope {
    envelope.snapshot = Nullable::Value(snapshot.clone());
    envelope
}

// --- one beat as one request, so `dispatch` and `dispatch_or_die` share a body -------------

/// Build the request one beat of `lane` makes, or [`None`] when a `Resume` finds no
/// continuation to name — a schedule wants to keep going past that, the same way
/// `Schedule::run` keeps going past a refusal, because a skipped beat changes nothing.
fn beat_request(
    fixture: &Fixture,
    lane: &Lane,
    index: usize,
    beat: Beat,
    task: Option<&TaskHandle>,
    tag: &str,
) -> Option<OperationRequest> {
    let request = format!("req_{tag}{}_{index}", lane.token);
    Some(match beat {
        Beat::Start(states) => OperationRequest {
            envelope: on(
                budgeted(
                    keyed(
                        envelope("verification.start", "agent:runner", "cap_runner", &request),
                        &format!("idem-{request}"),
                    ),
                    Some(states),
                ),
                &fixture.snapshot,
            ),
            arguments: Arguments::VerificationStart(VerificationStartRequest {
                target: lane.target(),
                portfolio: Portfolio::Interactive,
                context_policy: Optional::Absent,
                priority_class: Optional::Absent,
            }),
        },
        Beat::Resume(states) => {
            let task = task?;
            let continuation = fixture
                .daemon
                .state()
                .tasks()
                .get(task)?
                .continuation
                .clone()?;
            OperationRequest {
                envelope: budgeted(
                    keyed(
                        envelope("task.resume", "agent:runner", "cap_runner", &request),
                        &format!("idem-{request}"),
                    ),
                    Some(states),
                ),
                arguments: Arguments::TaskResume(TaskResumeRequest {
                    continuation,
                    budget: Optional::Present(budget(Some(states))),
                }),
            }
        }
        Beat::Rebudget(states) => OperationRequest {
            envelope: keyed(
                envelope("task.update_budget", "agent:runner", "cap_runner", &request),
                &format!("idem-{request}"),
            ),
            arguments: Arguments::TaskUpdateBudget(TaskUpdateBudgetRequest {
                task: task?.clone(),
                budget: budget(Some(states)),
            }),
        },
    })
}

/// What a `verification.start` answered: the task it resolved to, or a cached result.
fn start_answer(outcome: &OperationOutcome) -> (Option<TaskHandle>, bool) {
    match &outcome.payload {
        Payload::VerificationStart(response) => (
            response.task.value().cloned(),
            response.result.value().is_some(),
        ),
        other => panic!("expected a verification.start payload, got {other:?}"),
    }
}

/// Play one beat, tolerating refusals and skipped beats (the tolerant runner the spliced
/// sweeps need: after a cancellation lands, a lane's remaining beats may legitimately be
/// refused, and a refusal changes nothing — which is asserted, not assumed).
fn play_beat(
    fixture: &mut Fixture,
    lane: &Lane,
    index: usize,
    task: &mut Option<TaskHandle>,
    tag: &str,
) -> Option<OperationOutcome> {
    let beat = lane.beats[index];
    let request = beat_request(fixture, lane, index, beat, task.as_ref(), tag)?;
    let outcome = fixture.daemon.dispatch(&request);
    if let Beat::Start(_) = beat {
        if let (Some(handle), _) = start_answer(&outcome) {
            *task = Some(handle);
        }
    }
    Some(outcome)
}

/// Play one beat and require it to succeed — the strict runner for unspliced programs.
fn play_beat_strict(
    fixture: &mut Fixture,
    lane: &Lane,
    index: usize,
    task: &mut Option<TaskHandle>,
    tag: &str,
) {
    let outcome = play_beat(fixture, lane, index, task, tag)
        .unwrap_or_else(|| panic!("{}[{index}]: the beat built no request", lane.token));
    assert!(
        outcome.error_code().is_none(),
        "{}[{index}]: the beat was refused — {:?}",
        lane.token,
        outcome.envelope.error
    );
}

// --- reading the daemon --------------------------------------------------------------------

fn regions(fixture: &Fixture) -> &TaskRegions {
    fixture.daemon.state().regions()
}

/// The daemon-grain no-orphan property, with the region ledger as the failure message.
fn assert_total(fixture: &Fixture, when: &str) {
    let regions = regions(fixture);
    assert!(
        regions.is_total(),
        "{when}: the daemon's regions are not total\n{}",
        regions.render()
    );
}

/// A schedule-independent canonical description of one task: its wire record (which
/// carries no request identities), its publication count, and its region accounting.
fn lane_render(fixture: &Fixture, task: &TaskHandle) -> String {
    let entry = fixture
        .daemon
        .state()
        .tasks()
        .get(task)
        .expect("the daemon holds the task");
    format!(
        "record={:?} publications={} staging={}",
        entry.record(),
        entry.evidence.count(),
        entry.evidence.is_staging(),
    )
}

/// A schedule-independent canonical description of the whole daemon after a run: both
/// lanes' renders, the region totals (counts, not the ordered log — the *order* scopes
/// were opened in is the schedule's own business), and the store's identity/receipt map.
fn run_render(fixture: &Fixture, tasks: &[(&str, &TaskHandle)]) -> String {
    let mut render = String::new();
    for (token, task) in tasks {
        render.push_str(&format!("{token}: {}\n", lane_render(fixture, task)));
    }
    let regions = regions(fixture);
    render.push_str(&format!(
        "regions: opened={} finalized={} orphans={} unfinalized={} defects={} total={}\n",
        regions.opened(),
        regions.finalizations().len(),
        regions.orphans().len(),
        regions.unfinalized().len(),
        regions.defects().len(),
        regions.is_total(),
    ));
    let view = fixture
        .daemon
        .store()
        .audit_view(&operator())
        .expect("`cap_root` confers audit");
    let mut ledger: Vec<String> = view
        .identities()
        .into_iter()
        .map(|identity| format!("{identity}×{}", view.receipts(&identity).len()))
        .collect();
    ledger.sort();
    render.push_str(&format!(
        "store: {ledger:?} fsck={:?}\n",
        view.fsck(&Blake3Identity)
    ));
    render
}

// --- schedules as data ---------------------------------------------------------------------

/// Every interleaving of programs with the given lengths, as sequences of program
/// indices: the shuffle product, enumerated deterministically and linear in the output
/// (`continuum_task::region::schedule::interleavings` restated over indices — that
/// function is typed over region `Step`s, and a `tests/*.rs` file is its own crate).
fn weavings(lengths: &[usize]) -> Vec<Vec<usize>> {
    fn extend(remaining: &mut Vec<usize>, prefix: &mut Vec<usize>, out: &mut Vec<Vec<usize>>) {
        if remaining.iter().all(|left| *left == 0) {
            out.push(prefix.clone());
            return;
        }
        for program in 0..remaining.len() {
            if remaining[program] == 0 {
                continue;
            }
            remaining[program] -= 1;
            prefix.push(program);
            extend(remaining, prefix, out);
            prefix.pop();
            remaining[program] += 1;
        }
    }
    let mut out = Vec::new();
    extend(&mut lengths.to_vec(), &mut Vec::new(), &mut out);
    out
}

/// Run one two-lane schedule with the strict runner; answer the two task handles.
fn run_two_lanes(fixture: &mut Fixture, schedule: &[usize]) -> (TaskHandle, TaskHandle) {
    let lanes = [&DPOR, &PROOF];
    let mut tasks: [Option<TaskHandle>; 2] = [None, None];
    let mut cursors = [0_usize; 2];
    for &program in schedule {
        let index = cursors[program];
        cursors[program] += 1;
        let mut task = tasks[program].take();
        play_beat_strict(fixture, lanes[program], index, &mut task, "");
        tasks[program] = task;
    }
    (
        tasks[0].clone().expect("the dpor lane started"),
        tasks[1].clone().expect("the proof lane started"),
    )
}

/// The DPOR task's content identity, precomputed by running its first beat in a scratch
/// daemon: a `task_*` handle is a content identity of the campaign's inputs, so it is the
/// same handle in every fixture — which is what lets a splice name the task *before* the
/// beat that creates it has run, and be answered with the typed refusal for a task this
/// daemon does not hold.
fn dpor_handle() -> TaskHandle {
    let mut scratch = fixture();
    let mut task = None;
    play_beat_strict(&mut scratch, &DPOR, 0, &mut task, "scratch_");
    task.expect("the scratch start names a task")
}

// --- sweep 1: commutation ------------------------------------------------------------------

/// **Two lanes, every interleaving.** 6!/(3!·3!) = 20 schedules of the dpor and proof
/// programs through one daemon each. The finished daemon — both task records, the region
/// totals, the store's identity/receipt map — is byte-identical across all 20: the
/// lifecycle's outcome is a function of the programs, not of the dispatch interleaving,
/// and no interleaving leaks an orphan scope.
#[test]
fn positive_two_lane_interleavings_commute_and_leave_no_orphans() {
    let schedules = weavings(&[DPOR.beats.len(), PROOF.beats.len()]);
    assert_eq!(schedules.len(), 20, "6!/(3!·3!)");

    let mut renders: BTreeSet<String> = BTreeSet::new();
    for schedule in &schedules {
        let mut fixture = fixture();
        let (dpor, proof) = run_two_lanes(&mut fixture, schedule);
        assert_total(&fixture, "after the woven programs");
        assert_eq!(
            regions(&fixture).opened(),
            DPOR.opens + PROOF.opens,
            "work opened a scope count the programs do not declare"
        );
        renders.insert(run_render(&fixture, &[("dpor", &dpor), ("proof", &proof)]));
    }

    assert_eq!(
        renders.len(),
        1,
        "the finished daemon varied with the interleaving:\n{renders:#?}"
    );
    let render = renders.into_iter().next().expect("one render");
    assert!(
        render.contains("publications=3") && render.contains("publications=2"),
        "the render does not carry both lanes' publications, so the sweep proved nothing:\n{render}"
    );
}

// --- sweep 2: cancellation at every position ------------------------------------------------

/// **`task.cancel` woven into every position of the two-lane shuffle.** 7!/(3!·3!·1!) =
/// 140 schedules, in every one of which the cancel names the dpor task's (precomputed)
/// content identity. Asserted per member:
///
/// - a cancel arriving before the task exists is the typed refusal and opens no scope
///   (`dx14_cancellation_matrix.rs`'s negative arm, now swept across positions);
/// - a cancel arriving after is answered, and the publications the task held are held
///   still (INV-009: cancellation truncates nothing);
/// - publications are monotone across the *whole* schedule — no later beat, refused or
///   admitted, ever removes one;
/// - the proof lane finishes byte-identical to its unspliced baseline: cancellation of
///   one lane is invisible to the other, at every position;
/// - the daemon's regions are total after every schedule — drain and finalize happened
///   inside whichever dispatch owned them, wherever the cancel landed.
#[test]
fn positive_cancellation_at_every_position_isolates_the_other_lane() {
    let victim = dpor_handle();

    // The unspliced baseline the proof lane must match at every splice position.
    let proof_baseline = {
        let mut fixture = fixture();
        let mut task = None;
        for index in 0..PROOF.beats.len() {
            play_beat_strict(&mut fixture, &PROOF, index, &mut task, "");
        }
        lane_render(&fixture, &task.expect("the proof lane started"))
    };

    let schedules = weavings(&[DPOR.beats.len(), PROOF.beats.len(), 1]);
    assert_eq!(schedules.len(), 140, "7!/(3!·3!·1!)");

    let mut refused_before_start = 0_usize;
    let mut answered_after = 0_usize;
    let mut statuses_answered: BTreeSet<String> = BTreeSet::new();

    for schedule in &schedules {
        let mut fixture = fixture();
        let lanes = [&DPOR, &PROOF];
        let mut tasks: [Option<TaskHandle>; 2] = [None, None];
        let mut cursors = [0_usize; 2];
        let mut publications_seen = 0_u32;

        for &program in schedule {
            if program == 2 {
                let opened_before = regions(&fixture).opened();
                let outcome = fixture.daemon.dispatch(&OperationRequest {
                    envelope: keyed(
                        envelope("task.cancel", "agent:runner", "cap_runner", "req_cancel"),
                        "idem-cancel",
                    ),
                    arguments: Arguments::TaskCancel(TaskCancelRequest {
                        task: victim.clone(),
                    }),
                });
                if tasks[0].is_none() {
                    // Before the task exists the cancel is a denial, and denial precedes
                    // semantic work: no scope opens (RFC 0027 X3).
                    assert!(
                        outcome.error_code().is_some(),
                        "a cancel of a task this daemon does not hold was answered Ok"
                    );
                    assert_eq!(regions(&fixture).opened(), opened_before);
                    refused_before_start += 1;
                } else {
                    assert!(
                        outcome.error_code().is_none(),
                        "the cancel was refused — {:?}",
                        outcome.envelope.error
                    );
                    let publications_at_cancel = fixture
                        .daemon
                        .state()
                        .tasks()
                        .get(&victim)
                        .expect("the daemon holds the victim")
                        .evidence
                        .count();
                    assert_eq!(
                        publications_at_cancel, publications_seen,
                        "the cancel changed the publication count (INV-009)"
                    );
                    let status = match &outcome.payload {
                        Payload::TaskCancel(response) => format!("{:?}", response.status),
                        other => panic!("expected a task.cancel payload, got {other:?}"),
                    };
                    statuses_answered.insert(status);
                    answered_after += 1;
                }
                continue;
            }

            let index = cursors[program];
            cursors[program] += 1;
            let mut task = tasks[program].take();
            // Tolerant on purpose: after a cancellation the dpor lane's remaining beats
            // may be refused or may legitimately resume a retained continuation; either
            // way the answer is typed and the invariants below must hold.
            play_beat(&mut fixture, lanes[program], index, &mut task, "");
            tasks[program] = task;

            if let Some(task) = &tasks[0] {
                let publications = fixture
                    .daemon
                    .state()
                    .tasks()
                    .get(task)
                    .map_or(0, |entry| entry.evidence.count());
                assert!(
                    publications >= publications_seen,
                    "a later dispatch removed a committed publication (INV-009)"
                );
                publications_seen = publications;
            }
        }

        assert_total(&fixture, "after the spliced schedule");
        let proof = tasks[1].clone().expect("the proof lane started");
        assert_eq!(
            lane_render(&fixture, &proof),
            proof_baseline,
            "cancelling the dpor lane perturbed the proof lane"
        );
    }

    // Anti-vacuity: the sweep reached both sides of the task's existence, and the
    // answered cancels reached more than one status — otherwise 140 runs swept one
    // behavior.
    assert!(
        refused_before_start > 0,
        "no schedule put the cancel before the start"
    );
    assert!(
        answered_after > 0,
        "no schedule put the cancel after the start"
    );
    assert!(
        statuses_answered.len() > 1,
        "every answered cancel found one status: {statuses_answered:?}"
    );
}

// --- sweep 3: duplicate coalescing at every position ---------------------------------------

/// **A duplicate `verification.start` woven into every position.** The duplicate is the
/// dpor lane's own first beat under a fresh request identity: same snapshot, intent,
/// target, portfolio, priority and budget, so it names the same `task_*` content
/// identity. 140 schedules, and at every one:
///
/// - the duplicate's answer resolves to the one task (or, when the campaign has already
///   completed, to its cached result — `verification.start`'s own two response shapes);
/// - the daemon holds exactly two tasks afterwards — the duplicate created no third;
/// - the region ledger opened exactly the scopes the two programs declare — whichever of
///   the two `start`s arrived second ran no work;
/// - the finished daemon renders byte-identical to the unspliced sweep-1 fixed point:
///   a duplicate submission is *observably nothing*, wherever it lands.
#[test]
fn positive_a_duplicate_start_at_every_position_coalesces_to_one_task() {
    let expected = dpor_handle();

    // The sweep-1 fixed point, recomputed here so this test stands alone.
    let baseline = {
        let mut fixture = fixture();
        let (dpor, proof) = run_two_lanes(&mut fixture, &[0, 0, 0, 1, 1, 1]);
        run_render(&fixture, &[("dpor", &dpor), ("proof", &proof)])
    };

    let schedules = weavings(&[DPOR.beats.len(), PROOF.beats.len(), 1]);
    assert_eq!(schedules.len(), 140, "7!/(3!·3!·1!)");

    let mut duplicate_created_the_task = 0_usize;
    let mut duplicate_answered_from_cache = 0_usize;
    let mut duplicate_resolved_to_task = 0_usize;

    for schedule in &schedules {
        let mut fixture = fixture();
        let lanes = [&DPOR, &PROOF];
        let mut tasks: [Option<TaskHandle>; 2] = [None, None];
        let mut cursors = [0_usize; 2];

        for &program in schedule {
            if program == 2 {
                let existed = tasks[0].is_some();
                let request = beat_request(&fixture, &DPOR, 0, DPOR.beats[0], None, "dup_")
                    .expect("a start builds unconditionally");
                let outcome = fixture.daemon.dispatch(&request);
                assert!(
                    outcome.error_code().is_none(),
                    "the duplicate start was refused — {:?}",
                    outcome.envelope.error
                );
                match start_answer(&outcome) {
                    (Some(task), _) => {
                        assert_eq!(task, expected, "the duplicate resolved to another task");
                        duplicate_resolved_to_task += 1;
                        if !existed {
                            // The duplicate arrived first, so *it* is the creator and the
                            // lane's own start becomes the no-op — first wins and all
                            // converge are the same statement here too.
                            tasks[0] = Some(task);
                            duplicate_created_the_task += 1;
                        }
                    }
                    (None, true) => {
                        assert!(existed, "a cached result answered before any campaign ran");
                        duplicate_answered_from_cache += 1;
                    }
                    (None, false) => panic!("the duplicate named neither task nor result"),
                }
                continue;
            }

            let index = cursors[program];
            cursors[program] += 1;
            let mut task = tasks[program].take();
            play_beat_strict(&mut fixture, lanes[program], index, &mut task, "");
            tasks[program] = task;
        }

        let dpor = tasks[0].clone().expect("the dpor lane exists");
        let proof = tasks[1].clone().expect("the proof lane exists");
        assert_eq!(
            fixture.daemon.state().tasks().handles().len(),
            2,
            "the duplicate created a third task"
        );
        assert_eq!(
            regions(&fixture).opened(),
            DPOR.opens + PROOF.opens,
            "the duplicate ran work"
        );
        assert_total(&fixture, "after the duplicate-spliced schedule");
        assert_eq!(
            run_render(&fixture, &[("dpor", &dpor), ("proof", &proof)]),
            baseline,
            "a duplicate submission was observable in the finished daemon"
        );
    }

    // Anti-vacuity: the duplicate landed before the original (creator swap), after
    // completion (cached result), and in between (task-shaped answer) — all three lanes
    // of the coalescing rule were reached.
    assert!(
        duplicate_created_the_task > 0,
        "no schedule put the duplicate before the original start"
    );
    assert!(
        duplicate_answered_from_cache > 0,
        "no schedule put the duplicate after the campaign closed"
    );
    assert!(
        duplicate_resolved_to_task > duplicate_created_the_task,
        "the duplicate never met a live task it did not create"
    );
}

// --- sweep 4: restart boundaries ------------------------------------------------------------

/// Kills at exactly one boundary — `g1_crash_recovery_evidence.rs`'s injector, restated
/// locally (a `tests/*.rs` file is its own crate).
struct KillAt(CrashPoint);

impl CrashInjector for KillAt {
    fn kills(&self, point: CrashPoint) -> bool {
        point == self.0
    }
}

/// **A crash at every boundary of every position of the lifecycle, then a restart that
/// re-runs it.** For each of the dpor lane's three beats and each of the nine
/// [`CrashPoint`]s — 27 runs — the beat is dispatched under `KillAt`, the daemon is
/// crashed, and:
///
/// - the recovery report over the survivor has **no defects and no quarantine** at every
///   one of the 27: a dispatch-boundary crash never leaves a half-publication, at any
///   position of the lifecycle — the store's own two-phase commit ran entirely inside
///   the handler or not at all (the in-store window belongs to the store seam, proved
///   elsewhere; this asserts the *composition* holds at every lifecycle position);
/// - per position, the durable image is a **step function of the boundary**: the eight
///   pre-handler images render byte-identically and `AfterHandler` differs —
///   `g1_crash_recovery_evidence.rs`'s claim, extended from one prepared dispatch to
///   every position of a running lifecycle;
/// - a daemon restarted over the survivor re-prepares its volatile facts and finishes the
///   lane to a final record **byte-identical** to a cold daemon's, with total regions.
///   Where the survivor holds no parked head, it re-runs the whole lane. Where it does, the
///   startup pass restored the task live from its continuation record (plan §4.5 O2, the
///   resume branch, bn-20142), and the lane resumes on the restored `cont_*` handle from the
///   first beat the crash left undone. Before bn-20142 those 18 runs stopped at
///   `Failed(ContinuationNotDurable)`, which RFC 0026 made final. Where the survivor holds
///   the closing run's terminal record (bn-2g3ei), the startup pass restored the task live
///   and `Completed`, nothing is played, and the restored record is the cold one byte for
///   byte. Before bn-2g3ei that run resolved to `Settled` and re-ran the whole lane.
#[test]
fn positive_a_crash_at_every_boundary_of_every_position_restarts_to_the_same_record() {
    // The cold baseline: the lane run to completion on a daemon that never crashed.
    let cold = {
        let mut fixture = fixture();
        let mut task = None;
        for index in 0..DPOR.beats.len() {
            play_beat_strict(&mut fixture, &DPOR, index, &mut task, "");
        }
        lane_render(&fixture, &task.expect("the cold lane completed"))
    };

    let mut runs = 0_usize;
    let mut resumed = 0_usize;
    let mut reran = 0_usize;
    let mut terminal = 0_usize;
    for position in 0..DPOR.beats.len() {
        let mut images: Vec<(CrashPoint, String)> = Vec::new();
        for point in CrashPoint::ALL {
            runs += 1;

            // Wind the lane up to `position`, strictly.
            let mut fixture = fixture();
            let mut task = None;
            for index in 0..position {
                play_beat_strict(&mut fixture, &DPOR, index, &mut task, "");
            }

            // The beat the crash interrupts.
            let request = beat_request(
                &fixture,
                &DPOR,
                position,
                DPOR.beats[position],
                task.as_ref(),
                "",
            )
            .expect("every beat of a strictly wound lane builds");
            let killed = fixture
                .daemon
                .dispatch_or_die(&request, &KillAt(point))
                .expect_err("the injector kills at this boundary");
            assert_eq!(killed.point, point);

            // The crash, and what it left.
            let durable = fixture.daemon.crash();
            let report =
                recovery::recover(durable.store(), &operator()).expect("`cap_root` confers audit");
            assert_eq!(
                report.defects(),
                &[],
                "position {position}, {point}: the survivor is defective"
            );
            assert_eq!(
                report.quarantined(),
                &[],
                "position {position}, {point}: the survivor holds quarantine"
            );
            assert_eq!(report.verdict(), Verdict::Clean);
            images.push((point, report.render()));

            // The restart, over the survivor.
            let mut restarted = fixture_over(restart(durable));
            assert!(
                restarted
                    .daemon
                    .state()
                    .tasks()
                    .resolved()
                    .all(|resolved| !matches!(resolved.resolution, Resolution::Failed(_))),
                "position {position}, {point}: a task resolved to `Failed` although every \
                 parked head committed its continuation record"
            );
            let restored = restarted
                .daemon
                .state()
                .tasks()
                .handles()
                .first()
                .map(|task| {
                    let entry = restarted
                        .daemon
                        .state()
                        .tasks()
                        .get(task)
                        .expect("a handle the table just listed");
                    ((*task).clone(), entry.publications(), entry.status)
                });
            let mut task = None;
            let first_beat = match restored {
                // The survivor held the closing run's terminal record, so the startup pass
                // restored the task live and `Completed` (bn-2g3ei). Nothing is left to play:
                // the restored record itself is compared with the cold one below, which is
                // `task.status` answering byte for byte what it answered before the crash.
                Some((handle, _, TaskStatus::Completed)) => {
                    terminal += 1;
                    task = Some(handle);
                    DPOR.beats.len()
                }
                // The survivor held a parked head and its continuation record, so the
                // startup pass restored the task live at its committed continuation (plan
                // §4.5 O2, the resume branch, bn-20142). Each beat of this lane commits one
                // publication, so the committed count is the index of the first beat the
                // crash left undone. The lane resumes from there on the restored `cont_*`
                // handle, with no re-run of what was committed.
                Some((handle, publications, _)) => {
                    resumed += 1;
                    assert_eq!(
                        restarted.daemon.state().tasks().handles().len(),
                        1,
                        "one lane, one restored task"
                    );
                    task = Some(handle);
                    usize::try_from(publications).expect("a small count")
                }
                // Nothing parked survived: the lane re-runs from its start.
                None => {
                    reran += 1;
                    0
                }
            };
            for index in first_beat..DPOR.beats.len() {
                play_beat_strict(&mut restarted, &DPOR, index, &mut task, "");
            }
            assert_total(&restarted, "after the restarted lane");
            assert_eq!(
                lane_render(&restarted, &task.expect("the restarted lane completed")),
                cold,
                "position {position}, {point}: the restarted lifecycle diverged from cold"
            );
        }

        // The step function, per position (g1's claim, at every lifecycle position).
        let before: Vec<&String> = images
            .iter()
            .filter(|(point, _)| !point.reaches_the_store())
            .map(|(_, image)| image)
            .collect();
        assert_eq!(before.len(), 8, "eight boundaries precede the handler");
        for image in &before {
            assert_eq!(
                image.as_bytes(),
                before[0].as_bytes(),
                "position {position}: two pre-handler boundaries disagree"
            );
        }
        let after = images
            .iter()
            .find(|(point, _)| point.reaches_the_store())
            .map(|(_, image)| image)
            .expect("`AfterHandler` is in the catalogue");
        assert_ne!(
            after.as_bytes(),
            before[0].as_bytes(),
            "position {position}: the handler ran and the durable image did not move — \
             a vacuous agreement proves nothing"
        );
    }
    assert_eq!(runs, 27, "three positions × nine boundaries");
    // Position 0 before the handler: nothing durable, the lane re-runs from scratch (8).
    // Position 2 at `AfterHandler`: the closing run committed its terminal record, so the
    // task is restored `Completed` and nothing re-runs (1). Before bn-2g3ei this run
    // resolved to `Settled`, the start superseded it, and the lane re-ran from scratch.
    // Every other run has a parked head and its continuation record, so the task is
    // restored and the lane resumes from the committed continuation (18) — the runs that
    // stopped at `Failed(ContinuationNotDurable)` before bn-20142. All three arms are
    // exercised, so none is vacuous.
    assert_eq!((reran, resumed, terminal), (8, 18, 1));
}

/// **The same sweep over the proof lane, whose middle beat changes the budget and not the
/// work** (bn-20142). `task.update_budget` on a parked task publishes the next revision of
/// its continuation record, so a crash after it restores the raised ceiling and the
/// `task.budget_updated` milestone, and a crash before it restores neither. For each of the
/// lane's three beats and each of the nine boundaries — 27 runs — the restarted daemon
/// finishes the lane to the cold record byte for byte. The first undone beat is read off the
/// restored task: no publication means nothing parked, one publication and the original
/// ceiling means the update is undone, and the raised ceiling means only the resume is.
#[test]
fn positive_a_crash_around_a_budget_update_resumes_the_proof_lane_to_the_same_record() {
    let cold = {
        let mut fixture = fixture();
        let mut task = None;
        for index in 0..PROOF.beats.len() {
            play_beat_strict(&mut fixture, &PROOF, index, &mut task, "");
        }
        lane_render(&fixture, &task.expect("the cold lane completed"))
    };

    let mut first_beats: Vec<usize> = Vec::new();
    for position in 0..PROOF.beats.len() {
        for point in CrashPoint::ALL {
            let mut fixture = fixture();
            let mut task = None;
            for index in 0..position {
                play_beat_strict(&mut fixture, &PROOF, index, &mut task, "");
            }
            let request = beat_request(
                &fixture,
                &PROOF,
                position,
                PROOF.beats[position],
                task.as_ref(),
                "",
            )
            .expect("every beat of a strictly wound lane builds");
            fixture
                .daemon
                .dispatch_or_die(&request, &KillAt(point))
                .expect_err("the injector kills at this boundary");
            let durable = fixture.daemon.crash();

            let mut restarted = fixture_over(restart(durable));
            let restored = restarted
                .daemon
                .state()
                .tasks()
                .handles()
                .first()
                .map(|task| {
                    let entry = restarted
                        .daemon
                        .state()
                        .tasks()
                        .get(task)
                        .expect("a handle the table just listed");
                    (
                        (*task).clone(),
                        entry.budget().states.value().copied(),
                        entry.status,
                    )
                });
            let mut task = None;
            let first_beat = match restored {
                // The closing resume committed its terminal record (bn-2g3ei): the task is
                // restored `Completed` and nothing is left to play.
                Some((handle, _, TaskStatus::Completed)) => {
                    task = Some(handle);
                    PROOF.beats.len()
                }
                Some((handle, Some(4), _)) => {
                    task = Some(handle);
                    1
                }
                Some((handle, Some(64), _)) => {
                    task = Some(handle);
                    2
                }
                Some((_, ceiling, _)) => panic!("an unexpected restored ceiling {ceiling:?}"),
                None => 0,
            };
            first_beats.push(first_beat);
            for index in first_beat..PROOF.beats.len() {
                play_beat_strict(&mut restarted, &PROOF, index, &mut task, "");
            }
            assert_total(&restarted, "after the restarted proof lane");
            assert_eq!(
                lane_render(&restarted, &task.expect("the restarted lane completed")),
                cold,
                "position {position}, {point}: the restarted proof lane diverged from cold"
            );
        }
    }
    assert_eq!(first_beats.len(), 27, "three positions × nine boundaries");
    // Position 0: nothing before the handler (8 × beat 0), the park after it (beat 1).
    // Position 1: the park before the handler (8 × beat 1), the update after it (beat 2).
    // Position 2: the update before the handler (8 × beat 2), and after it the task is
    // restored `Completed` from its terminal record and nothing re-runs (beat 3). Before
    // bn-2g3ei the closed head was `Settled` and the lane re-ran from its start (beat 0).
    let count = |beat: usize| first_beats.iter().filter(|first| **first == beat).count();
    assert_eq!((count(0), count(1), count(2), count(3)), (8, 9, 9, 1));
}

// --- determinism -----------------------------------------------------------------------------

/// The commutation matrix, rendered whole, is byte-identical across two runs — the
/// repeated-round invariant every campaign in this repository ends on, over bytes because
/// bytes catch an ordering difference that set equality hides.
#[test]
fn positive_the_whole_matrix_renders_byte_identically_across_two_runs() {
    fn render_once() -> String {
        let mut render = String::new();
        for schedule in weavings(&[DPOR.beats.len(), PROOF.beats.len()]) {
            let mut fixture = fixture();
            let (dpor, proof) = run_two_lanes(&mut fixture, &schedule);
            render.push_str(&format!(
                "{schedule:?} =>\n{}",
                run_render(&fixture, &[("dpor", &dpor), ("proof", &proof)])
            ));
        }
        render
    }

    let first = render_once();
    let second = render_once();
    assert_eq!(first.as_bytes(), second.as_bytes());
    assert!(
        first.lines().count() > 80 && first.contains("publications=3"),
        "the render is not the matrix: {} lines",
        first.lines().count()
    );
}
