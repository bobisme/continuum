//! Acceptance re-derivation for release gate **G1-05** (bone `bn-1tkrp`).
//!
//! > cancellation closes obligations and publishes no partial finality
//! >
//! > — `notes/plan/docs/52_RELEASE_GATES_REV3.md:22`, G1 bullet 5
//!
//! # The normative sources this file is written against
//!
//! | Source | What it obliges |
//! |---|---|
//! | [ADR-0010] | cancellation, obligations, reserve/commit/abort and finalization are *formal semantics*, not task disappearance |
//! | [ADR-0003] / INV-005 | cancellation reaches controlled code through an explicit capability, never ambient |
//! | [RFC 0001], "Obligations" | obligations are **linear semantic resources**; child-region quiescence and cancellation finalization are two of its five named examples, and obligation conservation is a validator obligation |
//! | [RFC 0026], `rule task.cancel_correct` | `task.cancel` triggers request → drain → finalize and MUST leave *either* committed partial evidence plus a valid continuation *or* nothing published (INV-009, plan B19) |
//! | [RFC 0026], "Atomicity of publication" | "Cancellation MUST NOT truncate a publication in progress: `task.cancel` finalizes or discards, never both halves" (INV-017) |
//! | `notes/plan/notes/G0_SPIKE_MATRIX.md`, DX-14 | pass condition: **no leaked obligations**; **resumable artifacts either committed or absent** |
//!
//! [ADR-0010]: ../../../notes/plan/adr/0010-cancellation-calculus.md
//! [ADR-0003]: ../../../notes/plan/adr/0003-no-ambient-nondeterminism.md
//! [RFC 0001]: ../../../notes/plan/rfcs/0001-causal-intermediate-representation.md
//! [RFC 0026]: ../../../notes/plan/rfcs/0026-continuumd-native-protocol.md
//!
//! # Why this file exists beside evidence that already passes
//!
//! [`PHASE_A_EXIT_PACKAGE.md`] §7.2 records G1-05 as "evidenced — DX-14 matrix … and
//! `dx14_falsification.rs`", and §9.4 A17 states the package's own limit: it "re-ran the
//! *delivering* suites, not an independent re-derivation". Re-running
//! `dx14_cancellation_matrix.rs` establishes that the delivering suite still passes. It does
//! not re-derive the criterion, and the reason is specific rather than rhetorical: **every
//! obligation check in the delivering suite is a read of one summary value.**
//! `assert_total(..)` calls `TaskRegions::is_total()`, which is the daemon's own six-conjunct
//! self-assessment, and the closest thing to an accounting anywhere in that file is
//! `tree.ledger().is_balanced()` — a boolean that is *also* true of a ledger that opened
//! nothing at all. And **no test in that file reads the publication store**, so "publishes no
//! partial finality" is checked there against `TaskEntry::publications()`, a count the
//! cancelled task keeps about itself.
//!
//! # Method, stated so the difference is checkable
//!
//! | Delivering evidence (`dx14_cancellation_matrix.rs`, `dx14_falsification.rs`) | This file |
//! |---|---|
//! | asserts `TaskRegions::is_total()` — one boolean summarising six conjuncts | enumerates the **whole obligation space**: all four [`ObligationKind`]s × every region **and** worker ordinal the tree ever allocated, probed one at a time through [`Ledger::holds`] ([`census`]) |
//! | asserts `ledger().is_balanced()` at one point in one test | predicts the ledger's `opened`/`discharged` **deltas dispatch by dispatch** from *wire-observable facts only*, and compares ([`ledger_shadow`], [`obligations_match_a_wire_derived_prediction_dispatch_by_dispatch`]) |
//! | never touches the publication store | fingerprints the **published namespace** — every store identity, its bytes, its receipts, the abort log, storage attribution, and `fsck` — and requires bit-for-bit equality across a cancel ([`PublishedNamespace`]) |
//! | reads `TaskEntry::publications()`, a count on the cancelled task | reads the store the daemon actually published into, through the store's own capability-checked audit view, plus each task's *named* publication list |
//! | shows a denied cancel opens no scope (unknown task) | shows a cancel is a **capability**, not ambient: a `Read`-level actor is refused, nothing is torn down, and nothing is published (ADR-0003) |
//! | never re-drives work after a cancel | runs a second campaign *after* a cancel and requires it to be byte-identical to the same campaign in a daemon that never cancelled anything ([`the_world_after_a_cancel_is_the_world_a_cancel_never_touched`]) |
//! | negative control: a task the daemon does not hold | negative controls that exercise **the instruments**: the fingerprint must move when an uncancelled run publishes, the census must report each of the four obligation kinds when one is deliberately left open, and an injected storage fault must produce a store `fsck` finds fault with |
//!
//! The last row is the one that decides whether this file was worth writing. An instrument
//! that cannot report a defect has not certified anything, so every instrument here is shown
//! failing on a world constructed to make it fail.
//!
//! # Anti-vacuity: three defects injected into the daemon, and who caught them
//!
//! A green suite over a correct daemon says nothing about what the suite would notice. Three
//! defects were injected into the production sources while this file was written, each one
//! run against **both** this file and the delivering suite, and each reverted afterwards.
//! The tree in this commit contains none of them; what follows is the record of that audit.
//!
//! | Injected defect | `dx14_cancellation_matrix.rs` | this file |
//! |---|---|---|
//! | `RegionTree::finalize` stops discharging `cancellation-finalization` — DX-14's first conjunct, a *leaked obligation* | caught (`is_total()` reads `balanced=false`) | caught, **by name and subject**: `census` reports `cancellation-finalization:r2`, and the shadow ledger reports the discharge that did not happen |
//! | `task.cancel` stages and commits a publication of its own — a *half-publication during teardown* | caught (`entry.publications()` moved) | caught by three tests, one of which names the offending row: `+ publication 1 mutant-partial …` |
//! | `verification::publish_record` returns without writing — the task **claims a publication the store never holds** | **passes 7/7** | caught: the fingerprint's artifact set does not grow when a run publishes, and the injected-fault control finds no residue to classify |
//!
//! The third row is the load-bearing one. It is the exact direction docs/35 forbids — "the
//! task can never claim a publication the store does not hold" — and it is invisible to a
//! suite whose only reading of what was published is a count the task keeps about itself.
//!
//! # The verdict this file reaches
//!
//! **SATISFIED-AT-NARROWER-SCOPE.** The narrowing is
//! [`scope_the_cancellable_operation_classes_this_daemon_serves`] and
//! [`scope_no_publication_is_staged_at_rest_and_why_that_is_a_bound_not_a_proof`], both
//! executable, and it is this:
//!
//! 1. **One cancellable operation class is reachable.** `task.cancel` names a `task_*`
//!    handle, and the only `@task_starting` operation this daemon serves is
//!    `verification.start` (with `task.resume` advancing what it started). The IDL declares
//!    26 `@task_starting` operations; 25 have no handler here, so 25 task classes are
//!    **unprobed** — an absence, not a pass (INV-007).
//! 2. **No cancellation lands mid-publication, and none can.** `Daemon::dispatch` holds
//!    `&mut DaemonState` for a whole call, so a publication is staged and resolved inside one
//!    dispatch and no second operation can observe a staged one. This file therefore *probes*
//!    the property — [`Publications::is_staging`] is asserted false at every observable point,
//!    and the store's abort log and `fsck` are read at every one — and constructs the nearest
//!    reachable approximation with an injected storage fault
//!    ([`negative_control_an_injected_storage_fault_leaves_content_no_reader_can_reach`]).
//!    What it cannot do is interrupt a live publication, because there is no concurrency to
//!    interrupt. That is a property of the architecture, and it is a **bound on the evidence**,
//!    not a proof of the property under concurrency.
//! 3. **The evidence graph is empty throughout.** No `evidence.*` family is mounted, so the
//!    "closes obligations" half is probed over region/worker obligations and publication
//!    obligations, and not over evidence-graph obligations, which do not exist here.
//!
//! Everything inside that scope is **satisfied**, and no counterexample was found.
//!
//! # Findings this file states rather than papers over (INV-007, INV-008)
//!
//! 1. **`task.cancel` cannot reach the publication store at all, and that is why its
//!    store-side half is trivially safe.** `TaskFamily::handle` receives `store` and routes
//!    `Arguments::TaskCancel(request) => cancel(request, state, services)` — the handler has
//!    no store parameter, so "a cancel publishes nothing durable" is a property of the
//!    function's signature and not of the handler's care. Stated because it *bounds this
//!    file's own store-side evidence for the cancel*: the store rows of
//!    [`PublishedNamespace`] are guaranteed constant across a cancel by the type. They are
//!    still load-bearing everywhere else — for the publish path
//!    ([`negative_control_the_fingerprint_moves_when_an_uncancelled_run_publishes`]), for the
//!    injured store, and for
//!    [`the_world_after_a_cancel_is_the_world_a_cancel_never_touched`] — and the rows a cancel
//!    *can* move, because `cancel` holds `&mut DaemonState`, are the task-side publication
//!    list, the evidence graph, and the intent registry. All three are in the fingerprint,
//!    and the second injected defect above moved the first of them.
//! 2. **The delivering suite is blind to one direction of the criterion.** Injected defect 3
//!    is a daemon in which every task claims a publication the store never received, and
//!    `dx14_cancellation_matrix.rs` is green on it. That is not a defect in the daemon —
//!    the shipped daemon is correct — it is a gap in what the delivering evidence *reads*,
//!    and it is the specific gap this file closes.
//! 3. **Cancellation of already-final work is an idempotent no-op, not a too-late error.**
//!    RFC 0026 declares no code for "the task is already terminal", and `rule
//!    task.status_monotonic` makes the terminal status unchangeable, so the typed outcome is
//!    [`StructuralOutcome::Unchanged`] with the terminal status reported. This file asserts
//!    that reading rather than assuming a refusal was intended.
//!
//! # The harness
//!
//! Fixtures follow `dx14_cancellation_matrix.rs` and `daemon_task_regions.rs`, duplicated
//! locally rather than imported: a `tests/*.rs` file is its own crate and cannot `use` a
//! sibling. The Die Hard model, configuration and contract are `include_str!`'d from the one
//! copy of each in this repository, so nothing here can drift from the corpus.
//!
//! [`PHASE_A_EXIT_PACKAGE.md`]: ../../../notes/plan/notes/PHASE_A_EXIT_PACKAGE.md
//! [`Ledger::holds`]: continuum_task::region::obligation::Ledger::holds
//! [`ObligationKind`]: continuum_task::region::obligation::ObligationKind
//! [`Publications::is_staging`]: continuumd::daemon::budget::Publications::is_staging

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use continuum_engine_reference::diehard;
use continuum_intent::canonical_json::Json;
use continuum_intent::contract::IntentContract;
use continuum_task::region::obligation::{Obligation, ObligationKind, Subject};
use continuum_task::region::worker::{Resumability, WorkerId, WorkerStep};
use continuum_task::region::{RegionId, RegionTree};
use continuum_value::epoch::ProtocolWindow;
use continuum_workspace::artifact_path::ArtifactClass;
use continuum_workspace::publication::{
    AbortReason, CapabilityToken, PublicationPhase, StorageFaults,
};
use continuum_workspace::snapshot::WorkspacePath;
use continuumd::daemon::family::{Arguments, Payload};
use continuumd::daemon::identity::Blake3Identity;
use continuumd::daemon::intent::IntentFamily;
use continuumd::daemon::state::{IntentRecord, RegistryStatus};
use continuumd::daemon::task::TaskFamily;
use continuumd::daemon::verification::{VerificationFamily, model_source};
use continuumd::daemon::workspace::WorkspaceFamily;
use continuumd::daemon::{Daemon, OperationOutcome, OperationRequest, identity};
use continuumd::protocol::envelope::{
    Budget, EpochSet, RequestEnvelope, StructuralVerdictValue, Verdict,
};
use continuumd::protocol::handshake::{
    CapabilityDescriptor, CapabilityProfile, ClientHello, Negotiated, VersionRange, negotiate,
};
use continuumd::protocol::operations::intent::IntentAcceptRequest;
use continuumd::protocol::operations::task::{
    TaskCancelRequest, TaskResumeRequest, TaskStatusRequest,
};
use continuumd::protocol::operations::verification::{
    VerificationResultRequest, VerificationStartRequest,
};
use continuumd::protocol::operations::workspace::WorkspaceCreateRequest;
use continuumd::protocol::registry::{ENCODINGS, OPERATIONS};
use continuumd::protocol::scalar::{
    ActorId, CapabilityHandle, Commitment, ContinuationHandle, EpochIdentity, IntentHandle, Opaque,
    OperationName, ProtocolVersion, RequestId, TaskHandle, Timestamp, WorkspaceHandle,
};
use continuumd::protocol::shared::{SnapshotComponents, SnapshotEpochs, Target};
use continuumd::protocol::spec::{Annotation, Nullable, Optional};
use continuumd::protocol::vocabulary::{
    AuthorityLevel, Encoding, ErrorCode, Portfolio, ResultStatus, StructuralOutcome, TargetKind,
    TaskStatus,
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

// =========================================================================================
// the five residual profiles this file drives
// =========================================================================================

/// One dispatch a profile's program makes, and what the shadow ledger expects of it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Beat {
    /// `verification.start` under a state ceiling.
    Start(u64),
    /// `task.resume` under a ceiling.
    Resume(u64),
    /// `verification.start` against a predicate no model declares: the dispatch faults.
    StartRefused,
}

/// A residual profile: the program that reaches it, and what the cancel must then find.
///
/// "Residual profile" is `crates/continuumd/src/daemon/task.rs`'s own term — `CancelOutcome`
/// is computed from how much a task has published and whether it holds a continuation, and
/// from nothing else — so a program that reaches a profile has reached everything
/// `rule task.cancel_correct` can see.
#[derive(Debug)]
struct Profile {
    /// A stable token, used in failure messages and in the canonical render.
    token: &'static str,
    /// The campaign this profile asks for. Distinct per profile: a `task_*` handle is the
    /// content identity of its request, so two profiles naming one target under one budget
    /// would be one task.
    kind: TargetKind,
    /// The target identity.
    id: &'static str,
    /// The dispatch program.
    beats: &'static [Beat],
    /// The status the task holds when the cancel arrives.
    found: TaskStatus,
    /// How many publications the task holds when the cancel arrives.
    publications: u32,
    /// Whether the task holds a continuation when the cancel arrives.
    resumable: bool,
    /// The structural verdict the cancel answers with.
    outcome: StructuralOutcome,
}

/// The five residual profiles reachable through this daemon's wire surface.
///
/// Four are the shapes `rule task.cancel_correct`'s three arms can be reached on; the fifth
/// is the profile no successful program produces — a dispatch that *faulted*, leaving a task
/// that never ran.
const PROFILES: &[Profile] = &[
    Profile {
        token: "parked-once",
        kind: TargetKind::AllClaims,
        id: "DieHard",
        beats: &[Beat::Start(4)],
        found: TaskStatus::Suspended,
        publications: 1,
        resumable: true,
        outcome: StructuralOutcome::Cancelled,
    },
    Profile {
        token: "parked-twice",
        kind: TargetKind::Property,
        id: "TypeOK",
        beats: &[Beat::Start(4), Beat::Resume(8)],
        found: TaskStatus::Suspended,
        publications: 2,
        resumable: true,
        outcome: StructuralOutcome::Cancelled,
    },
    Profile {
        token: "closed",
        kind: TargetKind::Module,
        id: "DieHard",
        beats: &[Beat::Start(64)],
        found: TaskStatus::Completed,
        publications: 1,
        resumable: false,
        outcome: StructuralOutcome::Unchanged,
    },
    Profile {
        token: "budget-refused",
        kind: TargetKind::Claim,
        id: "NotSolved",
        beats: &[Beat::Start(0)],
        found: TaskStatus::Failed,
        publications: 0,
        resumable: false,
        outcome: StructuralOutcome::Unchanged,
    },
    Profile {
        token: "run-faulted",
        kind: TargetKind::Property,
        id: "NoSuchPredicate",
        beats: &[Beat::StartRefused],
        found: TaskStatus::Created,
        publications: 0,
        resumable: false,
        outcome: StructuralOutcome::Cancelled,
    },
];

impl Profile {
    fn target(&self) -> Target {
        Target {
            kind: self.kind,
            id: self.id.to_owned(),
        }
    }
}

// =========================================================================================
// instrument 1 — the obligation census
// =========================================================================================

/// The four obligation kinds, in declaration order. RFC 0001's linear semantic resources, as
/// `continuum-task` names them.
const KINDS: [ObligationKind; 4] = [
    ObligationKind::WorkerTermination,
    ObligationKind::ChildRegionQuiescence,
    ObligationKind::CancellationFinalization,
    ObligationKind::ProvisionalPublication,
];

/// Every obligation a region tree still owes, found by **enumerating the obligation space**
/// rather than by asking the ledger for its own summary.
///
/// The delivering evidence reads `Ledger::is_balanced()`, which is one boolean over the whole
/// tree. This walks all four [`ObligationKind`]s across every region ordinal *and* every
/// worker ordinal the tree ever allocated — `4 × (regions + workers)` probes, each one a
/// separate [`Ledger::holds`] call — so an obligation left open is reported *by kind and by
/// subject*, and an obligation keyed to the wrong kind of subject (a
/// `provisional-publication` filed against a region, say) is found too. Nothing here reads
/// `is_balanced`, `outstanding`, or any counter.
///
/// [`Ledger::holds`]: continuum_task::region::obligation::Ledger::holds
fn census(tree: &RegionTree) -> Vec<Obligation> {
    let mut subjects = Vec::new();
    for ordinal in 0..tree.region_count() {
        subjects.push(Subject::Region(RegionId::at(ordinal as u32)));
    }
    for ordinal in 0..tree.worker_count() {
        subjects.push(Subject::Worker(WorkerId::at(ordinal as u32)));
    }
    let mut held = Vec::new();
    for kind in KINDS {
        for subject in &subjects {
            let obligation = Obligation::new(kind, *subject);
            if tree.ledger().holds(obligation) {
                held.push(obligation);
            }
        }
    }
    held.sort();
    held
}

/// How many separate `holds` probes [`census`] made — the denominator of its claim.
///
/// An empty census over an empty space proves nothing, so every assertion of emptiness in
/// this file reports this number beside it.
fn census_probes(tree: &RegionTree) -> usize {
    KINDS.len() * (tree.region_count() + tree.worker_count())
}

/// The census as one canonical line, for failure messages and for the byte-identity render.
fn render_census(tree: &RegionTree) -> String {
    let held = census(tree);
    let mut out = format!("census probes={} held={}", census_probes(tree), held.len());
    for obligation in &held {
        out.push_str(&format!(" {obligation}"));
    }
    out.push('\n');
    out
}

// =========================================================================================
// instrument 2 — the wire-derived shadow ledger
// =========================================================================================

/// What one dispatch is predicted to do to the obligation ledger, derived from RFC 0001's
/// obligation kinds and from facts a **wire client can observe**.
///
/// The derivation, stated once so it can be checked against the dossier rather than against
/// the daemon:
///
/// - a unit of task work runs in a scope of its own, and opening one opens two obligations —
///   the child region owes its parent quiescence, and the worker owes a quiescent state
///   (RFC 0001's "child-region quiescence"; `ObligationKind::WorkerTermination`);
/// - each staged publication opens one `provisional-publication`, discharged by its commit or
///   by the drain that discards it — this is DX-14's "resumable artifacts either committed or
///   absent" as a linear resource;
/// - a teardown that has to *cancel* the work rather than close it opens one
///   `cancellation-finalization`, RFC 0001's other named example, discharged when the region
///   finalizes.
///
/// Every input below comes off the wire: how many artifacts the result envelope named, and
/// what status the task landed in. Nothing reads the region tree.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct Predicted {
    /// Scopes this dispatch opened. One per unit of task work; zero for a read, for a
    /// refused admission, and for a cancel of a task this daemon does not hold.
    scopes: u32,
    /// Publications staged inside those scopes.
    reserves: u32,
    /// Teardowns that escalated from a close to a cancellation, because the work had not
    /// reached a terminal state on its own.
    escalations: u32,
}

impl Predicted {
    /// The obligations this dispatch is predicted to open. Every one of them is discharged
    /// before the dispatch returns, so the same number is predicted for `discharged`.
    const fn obligations(self) -> u64 {
        (2 * self.scopes + self.reserves + self.escalations) as u64
    }
}

/// One reading of the ledger's two counters. Nothing else is read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct LedgerCounters {
    opened: u64,
    discharged: u64,
}

fn counters(fixture: &Fixture) -> LedgerCounters {
    let ledger = fixture.daemon.state().regions().tree().ledger();
    LedgerCounters {
        opened: ledger.opened(),
        discharged: ledger.discharged(),
    }
}

/// The shadow ledger: run `dispatch`, and check the ledger moved by exactly what the wire
/// facts predict.
///
/// Returns the outcome so the caller can go on reading it.
fn ledger_shadow(
    fixture: &mut Fixture,
    label: &str,
    predicted: Predicted,
    dispatch: impl FnOnce(&mut Fixture) -> OperationOutcome,
) -> OperationOutcome {
    let before = counters(fixture);
    let outcome = dispatch(fixture);
    let after = counters(fixture);
    let expected = predicted.obligations();
    assert_eq!(
        after.opened - before.opened,
        expected,
        "{label}: the ledger opened {} obligations; {predicted:?} predicts {expected}\n{}",
        after.opened - before.opened,
        fixture.daemon.state().regions().render()
    );
    assert_eq!(
        after.discharged - before.discharged,
        expected,
        "{label}: the ledger discharged {} obligations; {predicted:?} predicts {expected}\n{}",
        after.discharged - before.discharged,
        fixture.daemon.state().regions().render()
    );
    outcome
}

// =========================================================================================
// instrument 3 — the published-namespace fingerprint
// =========================================================================================

/// Everything this daemon has *published*, as bytes that can be compared bit for bit.
///
/// # What is in it, and why each part is
///
/// | Row | Read from | Why it is finality |
/// |---|---|---|
/// | `identity` | [`StoreAudit::identities`] plus [`ReferenceStore::read`] | the artifact and **its bytes**; an index that gained, lost, or rewrote an entry moves this |
/// | `receipt` | [`StoreAudit::receipts`] | plan §18.5's append-only publication ledger: a publication that left no receipt is unobservable, and one that left an extra receipt published twice |
/// | `abort` | [`StoreAudit::aborts`] | INV-017's own audit trail |
/// | `attribution` | [`StoreAudit::storage_attribution`] | committed bytes per artifact class |
/// | `fsck` | [`StoreAudit::fsck`] | docs/35's index verifier: unreachable content, missing referent, identity mismatch |
/// | `task-publications` | `TaskEntry::evidence` | what each task *claims* it committed, named rather than counted, plus whether anything is staged |
/// | `evidence`/`intent` | [`DaemonState`] | the graph and the registry, so a cancel that touched either would move this |
///
/// # What is deliberately **not** in it
///
/// - **The task's own lifecycle record.** A cancel is *supposed* to move `status`, append a
///   `task.cancelled` milestone, and emit a status-change event. Folding those into a
///   fingerprint that must not move would make the criterion untestable. They are asserted
///   separately, as the cancel's intended and complete effect.
/// - **`cap_*` tokens.** RFC 0027 S5: a capability handle never appears in a trace or a
///   rendering, and this rendering is a trace. Receipts contribute their actor and their
///   cost, not the token that produced them.
///
/// [`StoreAudit::identities`]: continuum_workspace::publication::StoreAudit::identities
/// [`StoreAudit::receipts`]: continuum_workspace::publication::StoreAudit::receipts
/// [`StoreAudit::aborts`]: continuum_workspace::publication::StoreAudit::aborts
/// [`StoreAudit::storage_attribution`]: continuum_workspace::publication::StoreAudit::storage_attribution
/// [`StoreAudit::fsck`]: continuum_workspace::publication::StoreAudit::fsck
/// [`ReferenceStore::read`]: continuum_workspace::publication::ReferenceStore::read
/// [`DaemonState`]: continuumd::daemon::state::DaemonState
#[derive(Debug, Clone, PartialEq, Eq)]
struct PublishedNamespace {
    rendered: String,
    /// Every published identity with the bytes the store holds for it, so a caller can name
    /// the row that moved rather than diffing a wall of text.
    artifacts: BTreeMap<String, Vec<u8>>,
}

impl PublishedNamespace {
    /// Read the whole published namespace out of a daemon.
    fn of(daemon: &Daemon) -> Self {
        let token = root_token();
        let view = daemon
            .store()
            .audit_view(&token)
            .expect("`cap_root` confers audit");

        let mut artifacts = BTreeMap::new();
        let mut out = String::new();
        out.push_str(&format!("published_count {}\n", view.published_count()));

        let mut identities = view.identities();
        identities.sort();
        for handle in &identities {
            let content = daemon
                .store()
                .read(handle, &token)
                .expect("`cap_root` confers read on a published artifact");
            out.push_str(&format!(
                "identity {handle} bytes={} content={}\n",
                content.len(),
                hex(&content)
            ));
            for (index, receipt) in view.receipts(handle).iter().enumerate() {
                out.push_str(&format!(
                    "receipt {handle} #{index} actor={} content_bytes={} index_entries={}\n",
                    receipt.actor().as_str(),
                    receipt.cost().content_bytes(),
                    receipt.cost().index_entries()
                ));
            }
            artifacts.insert(handle.to_string(), content);
        }

        let aborts = view.aborts();
        out.push_str(&format!("aborts {}\n", aborts.len()));
        for abort in &aborts {
            out.push_str(&format!("abort {} {}\n", abort.phase(), abort.reason()));
        }
        for (class, bytes) in view.storage_attribution() {
            out.push_str(&format!("attribution {} {bytes}\n", class.token()));
        }
        let defects = view.fsck();
        out.push_str(&format!("fsck {}\n", defects.len()));
        for defect in &defects {
            out.push_str(&format!("defect {defect}\n"));
        }

        for handle in daemon.state().tasks().handles() {
            let entry = daemon
                .state()
                .tasks()
                .get(handle)
                .expect("the handle came from the table");
            out.push_str(&format!("task-publications {}\n", handle.as_str()));
            for line in entry.evidence.render().lines() {
                out.push_str(&format!("  {line}\n"));
            }
        }

        out.push_str(&format!(
            "evidence nodes={} edges={} events={}\n",
            daemon.state().evidence_nodes().count(),
            daemon.state().evidence_edges().count(),
            daemon.state().evidence_events().len()
        ));
        for (handle, record) in daemon.state().intents() {
            out.push_str(&format!(
                "intent {} {}\n",
                handle.as_str(),
                record.status.as_wire()
            ));
        }

        Self {
            rendered: out,
            artifacts,
        }
    }

    /// The rows that differ between two readings, as a short report.
    fn diff(&self, other: &Self) -> String {
        let mine: BTreeSet<&str> = self.rendered.lines().collect();
        let theirs: BTreeSet<&str> = other.rendered.lines().collect();
        let mut out = String::new();
        for line in mine.difference(&theirs) {
            out.push_str(&format!("- {}\n", truncate(line)));
        }
        for line in theirs.difference(&mine) {
            out.push_str(&format!("+ {}\n", truncate(line)));
        }
        out
    }
}

/// Lowercase hex, so the comparison is over bytes and the failure message is printable.
fn hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

/// Keep a diff line readable: a rewritten artifact's content is thousands of hex digits.
fn truncate(line: &str) -> String {
    if line.len() <= 120 {
        return line.to_owned();
    }
    format!("{}… ({} chars)", &line[..120], line.len())
}

/// The store token `cap_root` names — audit and read authority over every class.
fn root_token() -> CapabilityToken {
    identity::capability_to_store(&cap("cap_root")).expect("`cap_root` is a store token")
}

// =========================================================================================
// fixtures
// =========================================================================================

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
        client: "continuumd-g1-05-acceptance".to_owned(),
        actor: who("service:continuumd"),
        capability: cap("cap_root"),
        features: Optional::Absent,
    };
    negotiate(&[version()], ProtocolWindow::new(3), ENCODINGS, &hello).expect("3.1 is served")
}

/// The epochs this daemon serves. Pinned, so a continuation has something to pin.
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

/// The daemon, optionally over an injected storage seam.
fn daemon(faults: Option<Arc<AtomicBool>>) -> Daemon {
    let root = Some(cap("cap_root"));
    let mut builder = Daemon::builder(Blake3Identity, negotiated(), cap("cap_root"))
        .epochs(epochs())
        .now(now());
    if let Some(armed) = faults {
        builder = builder.store_faults(FailTheIndexCommit { armed });
    }
    builder
        .capability(
            grant(
                "cap_root",
                "service:continuumd",
                AuthorityLevel::Promote,
                4,
                Optional::Present(profile(&[
                    "intent.accept",
                    "intent.reject",
                    "intent.lock",
                    "repair.promote",
                    "repair.reject",
                ])),
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
        .build()
}

/// The storage seam that fails the index commit once it is armed.
///
/// Armed *after* the fixture is built, so the workspace publication that seals the snapshot
/// succeeds and only the campaign record's publication is injured. `StorageFaults` takes
/// `&self`, so the arming flag is an [`AtomicBool`]: no interior mutability beyond what the
/// trait's own signature forces, and nothing here is a clock or a source of entropy.
#[derive(Debug)]
struct FailTheIndexCommit {
    armed: Arc<AtomicBool>,
}

impl StorageFaults for FailTheIndexCommit {
    fn check(&self, phase: PublicationPhase) -> Result<(), AbortReason> {
        if phase == PublicationPhase::CommittingIndex && self.armed.load(Ordering::SeqCst) {
            return Err(AbortReason::StorageExhausted);
        }
        Ok(())
    }
}

/// A daemon holding the Die Hard workspace sealed, its contract accepted, and the model
/// registered in the catalog.
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

fn fixture() -> Fixture {
    fixture_over(None)
}

fn fixture_over(faults: Option<Arc<AtomicBool>>) -> Fixture {
    let mut daemon = daemon(faults);
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
                intent: intent.clone(),
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

fn acceptance_bytes() -> Opaque {
    let mut fields: BTreeMap<String, Json> = BTreeMap::new();
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

// =========================================================================================
// the operations
// =========================================================================================

fn start(fixture: &mut Fixture, request: &str, states: u64, target: Target) -> OperationOutcome {
    let snapshot = fixture.snapshot.clone();
    fixture.daemon.dispatch(&OperationRequest {
        envelope: on(
            budgeted(
                keyed(
                    envelope("verification.start", "agent:runner", "cap_runner", request),
                    &format!("idem-{request}"),
                ),
                Some(states),
            ),
            &snapshot,
        ),
        arguments: Arguments::VerificationStart(VerificationStartRequest {
            target,
            portfolio: Portfolio::Interactive,
            context_policy: Optional::Absent,
            priority_class: Optional::Absent,
        }),
    })
}

fn resume(
    fixture: &mut Fixture,
    continuation: &ContinuationHandle,
    states: u64,
    request: &str,
) -> OperationOutcome {
    fixture.daemon.dispatch(&OperationRequest {
        envelope: budgeted(
            keyed(
                envelope("task.resume", "agent:runner", "cap_runner", request),
                &format!("idem-{request}"),
            ),
            Some(states),
        ),
        arguments: Arguments::TaskResume(TaskResumeRequest {
            continuation: continuation.clone(),
            budget: Optional::Present(budget(Some(states))),
        }),
    })
}

fn cancel_as(
    fixture: &mut Fixture,
    task: &TaskHandle,
    request: &str,
    actor: &str,
    capability: &str,
) -> OperationOutcome {
    fixture.daemon.dispatch(&OperationRequest {
        envelope: keyed(
            envelope("task.cancel", actor, capability, request),
            &format!("idem-{request}"),
        ),
        arguments: Arguments::TaskCancel(TaskCancelRequest { task: task.clone() }),
    })
}

fn cancel(fixture: &mut Fixture, task: &TaskHandle, request: &str) -> OperationOutcome {
    cancel_as(fixture, task, request, "agent:runner", "cap_runner")
}

fn status(fixture: &mut Fixture, task: &TaskHandle, request: &str) -> OperationOutcome {
    fixture.daemon.dispatch(&OperationRequest {
        envelope: envelope("task.status", "agent:reader", "cap_reader", request),
        arguments: Arguments::TaskStatus(TaskStatusRequest { task: task.clone() }),
    })
}

fn verification_result(
    fixture: &mut Fixture,
    task: &TaskHandle,
    request: &str,
) -> OperationOutcome {
    fixture.daemon.dispatch(&OperationRequest {
        envelope: envelope("verification.result", "agent:runner", "cap_runner", request),
        arguments: Arguments::VerificationResult(VerificationResultRequest { task: task.clone() }),
    })
}

// =========================================================================================
// reading the daemon
// =========================================================================================

fn entry<'a>(fixture: &'a Fixture, task: &TaskHandle) -> &'a continuumd::daemon::task::TaskEntry {
    fixture
        .daemon
        .state()
        .tasks()
        .get(task)
        .expect("the daemon holds the task")
}

fn tree(fixture: &Fixture) -> &RegionTree {
    fixture.daemon.state().regions().tree()
}

/// The one task handle this daemon holds. Every program here drives exactly one campaign.
fn only_task(fixture: &Fixture) -> TaskHandle {
    let handles = fixture.daemon.state().tasks().handles();
    assert_eq!(
        handles.len(),
        1,
        "this program was supposed to drive exactly one campaign"
    );
    handles[0].clone()
}

/// How many artifacts a result envelope named — the wire's own count of what the task has
/// published.
fn artifact_count(outcome: &OperationOutcome) -> u32 {
    u32::try_from(outcome.envelope.artifacts.len()).expect("short lists")
}

/// The wire spelling of a structural outcome, as a result envelope's verdict.
fn structural(outcome: StructuralOutcome) -> Nullable<Verdict> {
    Nullable::Value(Verdict::Structural(StructuralVerdictValue { outcome }))
}

/// Run a profile's program, checking the ledger prediction at every dispatch, and return the
/// task it produced.
fn wind_up(fixture: &mut Fixture, profile: &Profile) -> TaskHandle {
    let mut task: Option<TaskHandle> = None;
    for (index, beat) in profile.beats.iter().enumerate() {
        let request = format!("req_{}_{index}", profile.token);
        match beat {
            Beat::Start(states) => {
                let published_before = task
                    .as_ref()
                    .map_or(0, |handle| entry(fixture, handle).publications());
                // Predicted from the profile's declared residual, which is itself checked
                // against the wire below: one scope, one reserve per publication the run
                // committed, and one escalation exactly when the run parked.
                let reserves = profile.publications.min(1);
                let escalations = u32::from(profile.found == TaskStatus::Suspended);
                let outcome = ledger_shadow(
                    fixture,
                    &format!("{}/start", profile.token),
                    Predicted {
                        scopes: 1,
                        reserves,
                        escalations,
                    },
                    |fixture| start(fixture, &request, *states, profile.target()),
                );
                assert!(
                    outcome.error_code().is_none(),
                    "{}: the start was refused — {:?}",
                    profile.token,
                    outcome.envelope.error
                );
                let handle = match &outcome.payload {
                    Payload::VerificationStart(response) => response
                        .task
                        .value()
                        .cloned()
                        .expect("a fresh start names a task"),
                    other => panic!("expected a verification.start payload, got {other:?}"),
                };
                assert_eq!(
                    artifact_count(&outcome),
                    published_before + reserves,
                    "{}: the wire's artifact list and the reserve prediction disagree",
                    profile.token
                );
                task = Some(handle);
            }
            Beat::Resume(states) => {
                let handle = task.clone().expect("a resume follows a start");
                let continuation = entry(fixture, &handle)
                    .continuation
                    .clone()
                    .expect("a parked profile holds a continuation");
                let outcome = ledger_shadow(
                    fixture,
                    &format!("{}/resume", profile.token),
                    Predicted {
                        scopes: 1,
                        reserves: 1,
                        escalations: u32::from(profile.found == TaskStatus::Suspended),
                    },
                    |fixture| resume(fixture, &continuation, *states, &request),
                );
                assert!(
                    outcome.error_code().is_none(),
                    "{}: the resume was refused — {:?}",
                    profile.token,
                    outcome.envelope.error
                );
            }
            Beat::StartRefused => {
                // A dispatch that faults still opens and tears down its scope. The worker
                // fails, so nothing is reserved and the teardown does not escalate.
                let outcome = ledger_shadow(
                    fixture,
                    &format!("{}/start-refused", profile.token),
                    Predicted {
                        scopes: 1,
                        reserves: 0,
                        escalations: 0,
                    },
                    |fixture| start(fixture, &request, 64, profile.target()),
                );
                assert_eq!(
                    outcome.error_code(),
                    Some(ErrorCode::MalformedRequest),
                    "{}: the refused start did not fault as expected",
                    profile.token
                );
                assert_eq!(outcome.payload, Payload::None, "no partial effect");
                task = Some(only_task(fixture));
            }
        }
    }
    let handle = task.expect("every profile runs at least one beat");
    assert_eq!(
        entry(fixture, &handle).status,
        profile.found,
        "{}: the program did not reach the residual profile it declares",
        profile.token
    );
    assert_eq!(
        entry(fixture, &handle).publications(),
        profile.publications,
        "{}: the program published a different number of artifacts",
        profile.token
    );
    assert_eq!(
        entry(fixture, &handle).continuation.is_some(),
        profile.resumable,
        "{}: the program's resumability is not what the profile declares",
        profile.token
    );
    handle
}

// =========================================================================================
// 1. no partial finality — the published namespace is bit-for-bit unchanged by a cancel
// =========================================================================================

/// **The criterion's second half, read off the store rather than off the cancelled task.**
///
/// For every residual profile: fingerprint the published namespace, cancel, fingerprint
/// again, and require the two to be byte-identical. The fingerprint is
/// [`PublishedNamespace`] — every store identity *with its bytes*, every receipt, the abort
/// log, storage attribution, `fsck`, and each task's named publication list.
///
/// This is the check the delivering evidence does not make. `dx14_cancellation_matrix.rs`
/// asserts `entry.publications()` is the same integer before and after the cancel; that is a
/// count the cancelled task keeps about itself, and it is equally true of a daemon whose
/// cancel deleted a store record, rewrote one, or issued a second receipt for one.
#[test]
fn cancellation_leaves_the_published_namespace_bit_for_bit_unchanged() {
    for profile in PROFILES {
        let mut fixture = fixture();
        let task = wind_up(&mut fixture, profile);

        let before = PublishedNamespace::of(&fixture.daemon);
        assert!(
            !before.artifacts.is_empty(),
            "{}: a fingerprint over an empty store proves nothing",
            profile.token
        );

        let cancelled = cancel(&mut fixture, &task, "req_cancel");
        assert_eq!(
            cancelled.envelope.status,
            ResultStatus::Ok,
            "{}: the cancel was refused — {:?}",
            profile.token,
            cancelled.envelope.error
        );
        assert_eq!(
            cancelled.envelope.verdict,
            structural(profile.outcome),
            "{}: the cancel's structural verdict is not the one the profile declares",
            profile.token
        );

        let after = PublishedNamespace::of(&fixture.daemon);
        assert_eq!(
            after.rendered.as_bytes(),
            before.rendered.as_bytes(),
            "{}: the cancel moved the published namespace\n{}",
            profile.token,
            before.diff(&after)
        );

        // And a second cancel, which is where a non-idempotent teardown would republish.
        let again = cancel(&mut fixture, &task, "req_cancel_2");
        assert_eq!(again.envelope.status, ResultStatus::Ok);
        let twice = PublishedNamespace::of(&fixture.daemon);
        assert_eq!(
            twice.rendered.as_bytes(),
            before.rendered.as_bytes(),
            "{}: cancelling twice moved the published namespace\n{}",
            profile.token,
            before.diff(&twice)
        );
    }
}

/// **Negative control for the fingerprint.** The same instrument, on a world where a
/// publication really does happen, must move — and must move by exactly one artifact.
///
/// Without this, [`cancellation_leaves_the_published_namespace_bit_for_bit_unchanged`] would
/// be satisfied by a fingerprint that reads nothing.
#[test]
fn negative_control_the_fingerprint_moves_when_an_uncancelled_run_publishes() {
    let mut fixture = fixture();
    let task = wind_up(&mut fixture, &PROFILES[0]);
    let before = PublishedNamespace::of(&fixture.daemon);

    // The beat the cancelled profile never takes: resume the parked campaign instead.
    let continuation = entry(&fixture, &task)
        .continuation
        .clone()
        .expect("the parked profile holds a continuation");
    let resumed = resume(&mut fixture, &continuation, 8, "req_control_resume");
    assert!(
        resumed.error_code().is_none(),
        "the control resume was refused — {:?}",
        resumed.envelope.error
    );

    let after = PublishedNamespace::of(&fixture.daemon);
    assert_ne!(
        after.rendered.as_bytes(),
        before.rendered.as_bytes(),
        "the fingerprint did not move across a publication; it is not an instrument"
    );
    let added: Vec<&String> = after
        .artifacts
        .keys()
        .filter(|handle| !before.artifacts.contains_key(*handle))
        .collect();
    assert_eq!(
        added.len(),
        1,
        "a resumed run publishes exactly one campaign record; this one published {}",
        added.len()
    );
    assert!(
        before
            .artifacts
            .iter()
            .all(|(handle, bytes)| after.artifacts.get(handle) == Some(bytes)),
        "the publication rewrote an artifact that was already published"
    );
}

// =========================================================================================
// 2. closed obligations — the census, and the ledger a wire client can predict
// =========================================================================================

/// **The criterion's first half, as a census over the whole obligation space.**
///
/// After every cancel of every residual profile, `4 × (regions + workers)` separate
/// [`Ledger::holds`] probes must all answer no. The census is also cross-checked against the
/// ledger's own `outstanding()` list: two independent readings of one fact, and a
/// disagreement would mean the ledger holds an obligation about a subject outside the tree's
/// own ordinal ranges.
///
/// [`Ledger::holds`]: continuum_task::region::obligation::Ledger::holds
#[test]
fn cancellation_closes_every_obligation_probed_subject_by_subject() {
    let mut probed = 0_usize;
    for profile in PROFILES {
        let mut fixture = fixture();
        let task = wind_up(&mut fixture, profile);

        // Before the cancel the world is already clean: the daemon opens and finalizes a
        // scope inside each dispatch. Stating it here is what makes the after-check a
        // statement about the *cancel* rather than about the program.
        assert!(
            census(tree(&fixture)).is_empty(),
            "{}: an obligation was outstanding before the cancel — {}",
            profile.token,
            render_census(tree(&fixture))
        );

        let outcome = cancel(&mut fixture, &task, "req_cancel");
        assert_eq!(outcome.envelope.status, ResultStatus::Ok);

        let held = census(tree(&fixture));
        let probes = census_probes(tree(&fixture));
        assert!(
            held.is_empty(),
            "{}: {} obligation(s) survived the cancel — {}",
            profile.token,
            held.len(),
            render_census(tree(&fixture))
        );
        assert!(
            probes >= 4 * 3,
            "{}: a census over {probes} probes is too small to mean anything",
            profile.token
        );
        // The second reading. `outstanding()` is the ledger's own list; `census` enumerated
        // the space. They must agree, and they are computed by different code.
        assert_eq!(
            tree(&fixture).ledger().outstanding(),
            held,
            "{}: the ledger's own outstanding list and the census disagree",
            profile.token
        );
        probed += probes;
    }
    assert!(
        probed >= 5 * 12,
        "the whole file made only {probed} obligation probes"
    );
}

/// **The ledger's counters, predicted from wire facts, dispatch by dispatch.**
///
/// [`ledger_shadow`] already checks every dispatch of every program in [`wind_up`]. This test
/// adds the cancel itself, which is the dispatch the criterion is about, and states the
/// prediction where a reader can check it against the dossier:
///
/// - a cancel opens **one** scope of its own — two obligations, a child region and a worker;
/// - it re-establishes the task's committed evidence inside that scope, one
///   `provisional-publication` per publication, so the drain finds committed evidence exactly
///   where the task table says there is some — and the count comes off the **wire**, from the
///   result envelope's artifact list, not out of the daemon;
/// - its worker never reaches a terminal state on its own, so the teardown escalates to a
///   cancellation: one `cancellation-finalization`, RFC 0001's own named example.
///
/// A ledger that opened fewer obligations than this has not accounted for the work; one that
/// opened more has double-counted it. `is_balanced()` is true of both.
#[test]
fn obligations_match_a_wire_derived_prediction_dispatch_by_dispatch() {
    for profile in PROFILES {
        let mut fixture = fixture();
        let task = wind_up(&mut fixture, profile);

        // The artifact count is read from a `task.status` answer, so the prediction's one
        // input is a wire fact obtained before the dispatch it predicts.
        let observed = status(&mut fixture, &task, "req_status");
        assert_eq!(observed.envelope.status, ResultStatus::Ok);
        let published = artifact_count(&observed);
        assert_eq!(
            published, profile.publications,
            "{}: the wire and the profile disagree about what is published",
            profile.token
        );

        let outcome = ledger_shadow(
            &mut fixture,
            &format!("{}/cancel", profile.token),
            Predicted {
                scopes: 1,
                reserves: published,
                escalations: 1,
            },
            |fixture| cancel(fixture, &task, "req_cancel"),
        );
        assert_eq!(outcome.envelope.status, ResultStatus::Ok);

        // A second cancel is a no-op on the record and still opens its own scope, because
        // "what did this task leave behind" is the same question whichever answer the status
        // gives. The prediction is identical.
        let again = ledger_shadow(
            &mut fixture,
            &format!("{}/cancel-again", profile.token),
            Predicted {
                scopes: 1,
                reserves: published,
                escalations: 1,
            },
            |fixture| cancel(fixture, &task, "req_cancel_2"),
        );
        assert_eq!(again.envelope.status, ResultStatus::Ok);

        // A read opens nothing at all, so the counters do not move. Without this the
        // denominator above would grow for reasons unrelated to work.
        let read = ledger_shadow(
            &mut fixture,
            &format!("{}/status", profile.token),
            Predicted::default(),
            |fixture| status(fixture, &task, "req_status_2"),
        );
        assert_eq!(read.envelope.status, ResultStatus::Ok);
    }
}

/// **Negative control for the census.** Each of the four obligation kinds is deliberately
/// left open on a bare region tree, and the same [`census`] function must name it.
///
/// The tree is `continuum_task::region::RegionTree` driven directly, so the four kinds are
/// opened by the acts RFC 0001 says open them — a child region, a spawned worker, a staged
/// publication, and a requested cancellation — and no daemon is involved. A census that
/// could not report these four has certified nothing when it reports none.
#[test]
fn negative_control_the_census_names_every_obligation_kind_that_is_open() {
    let mut tree = RegionTree::new();
    let root = tree.root();
    let region = tree.open_child(root).expect("the root is open");
    let worker = tree
        .spawn(region, Resumability::Resumable)
        .expect("a fresh region admits a worker");
    tree.advance(worker, WorkerStep::Begin).expect("begin");
    tree.advance(worker, WorkerStep::Reserve).expect("reserve");
    tree.cancel(region).expect("an open region is cancellable");

    let held = census(&tree);
    let kinds: BTreeSet<ObligationKind> = held
        .iter()
        .map(continuum_task::region::obligation::Obligation::kind)
        .collect();
    assert_eq!(
        kinds,
        BTreeSet::from(KINDS),
        "the census did not report every kind that is open: {}",
        render_census(&tree)
    );
    assert!(
        held.contains(&Obligation::child_region_quiescence(region)),
        "the census lost the child region's quiescence obligation"
    );
    assert!(
        held.contains(&Obligation::worker_termination(worker)),
        "the census lost the worker's termination obligation"
    );
    assert!(
        held.contains(&Obligation::provisional_publication(worker)),
        "the census lost the staged publication — DX-14's second conjunct"
    );
    assert!(
        held.contains(&Obligation::cancellation_finalization(region)),
        "the census lost the cancellation's finalization obligation — DX-14's first conjunct"
    );

    // And the same instrument reports nothing once the teardown discharges them.
    tree.drain(region).expect("a cancelled region drains");
    tree.finalize(region).expect("a drained region finalizes");
    assert!(
        census(&tree).is_empty(),
        "the teardown left an obligation open: {}",
        render_census(&tree)
    );
}

/// **Negative control for the shadow ledger.** A prediction that is wrong must fail.
///
/// [`ledger_shadow`] is only evidence if a mismatch is loud, so the arithmetic is exercised
/// against a known-wrong prediction here rather than trusted. A cancel of a profile that
/// published one artifact opens four obligations; asserting three must panic.
#[test]
#[should_panic(expected = "predicts 3")]
fn negative_control_a_wrong_ledger_prediction_fails() {
    let mut fixture = fixture();
    let task = wind_up(&mut fixture, &PROFILES[0]);
    let _ = ledger_shadow(
        &mut fixture,
        "deliberately-wrong",
        Predicted {
            scopes: 1,
            reserves: 0,
            escalations: 1,
        },
        |fixture| cancel(fixture, &task, "req_cancel"),
    );
}

// =========================================================================================
// 3. cancelling work that already reached finality
// =========================================================================================

/// **Finality is not retracted.** A cancel of a task that already completed answers
/// `unchanged`, leaves the status `completed`, and leaves every published artifact readable
/// and byte-identical — and `verification.result` still answers exactly what it answered
/// before.
///
/// The typed outcome is the one the spec names: `rule task.status_monotonic`'s "a terminal
/// status never changes" makes this an idempotent no-op reported as
/// [`StructuralOutcome::Unchanged`], not a too-late error.
#[test]
fn cancelling_work_that_already_reached_finality_retracts_nothing() {
    let closed = &PROFILES[2];
    assert_eq!(closed.found, TaskStatus::Completed, "profile drift");
    let mut fixture = fixture();
    let task = wind_up(&mut fixture, closed);

    let before_result = verification_result(&mut fixture, &task, "req_result");
    assert_eq!(
        before_result.envelope.status,
        ResultStatus::Ok,
        "{:?}",
        before_result.envelope.error
    );
    assert_ne!(
        before_result.payload,
        Payload::None,
        "a completed campaign with no result is not the case this test is about"
    );
    let before = PublishedNamespace::of(&fixture.daemon);

    let outcome = cancel(&mut fixture, &task, "req_cancel");
    assert_eq!(outcome.envelope.status, ResultStatus::Ok);
    let (status_after, continuation) = match &outcome.payload {
        Payload::TaskCancel(response) => (response.status, response.continuation.clone()),
        other => panic!("expected a task.cancel payload, got {other:?}"),
    };
    assert_eq!(
        status_after,
        TaskStatus::Completed,
        "a terminal status never changes"
    );
    assert_eq!(
        continuation,
        Nullable::Null,
        "a completed campaign has nothing to resume, and says so with the named null"
    );
    assert_eq!(
        artifact_count(&outcome),
        closed.publications,
        "the cancel's own answer stopped naming the artifacts the task published"
    );

    let after = PublishedNamespace::of(&fixture.daemon);
    assert_eq!(
        after.rendered.as_bytes(),
        before.rendered.as_bytes(),
        "the cancel retracted or altered published finality\n{}",
        before.diff(&after)
    );

    let after_result = verification_result(&mut fixture, &task, "req_result_2");
    assert_eq!(after_result.envelope.status, ResultStatus::Ok);
    assert_eq!(
        after_result.payload, before_result.payload,
        "the campaign's verdict changed after its task was cancelled"
    );
    assert_eq!(
        after_result.envelope.verdict, before_result.envelope.verdict,
        "the campaign's wire verdict changed after its task was cancelled"
    );
}

// =========================================================================================
// 4. the world after a cancel
// =========================================================================================

/// **A cancel leaves no residue a later campaign can feel.**
///
/// Two daemons. In the first, a campaign is started, parked, and cancelled, and *then* a
/// second, unrelated campaign runs. In the second daemon only that second campaign runs. The
/// second campaign's task identity, its wire record, its published artifact and the bytes and
/// receipts behind it must be identical in both.
///
/// This is what "the cancel left no poisoned residue" means where there are no locks and no
/// handles to wedge: the cancel is not observable in the world it left behind, beyond its own
/// task record. Nothing in the delivering evidence re-drives work after a cancel at all.
#[test]
fn the_world_after_a_cancel_is_the_world_a_cancel_never_touched() {
    let poisoner = &PROFILES[0];
    let successor = &PROFILES[2];

    // Daemon X: run and cancel the first campaign, then run the successor.
    let mut cancelled_world = fixture();
    let poisoned = wind_up(&mut cancelled_world, poisoner);
    let cancelled = cancel(&mut cancelled_world, &poisoned, "req_cancel");
    assert_eq!(cancelled.envelope.status, ResultStatus::Ok);
    let after_cancel = start(
        &mut cancelled_world,
        "req_successor",
        64,
        successor.target(),
    );

    // Daemon Y: run the successor and nothing else.
    let mut clean_world = fixture();
    let never_cancelled = start(&mut clean_world, "req_successor", 64, successor.target());

    assert!(
        after_cancel.error_code().is_none(),
        "the successor campaign was refused in the cancelled world — {:?}",
        after_cancel.envelope.error
    );
    assert!(
        never_cancelled.error_code().is_none(),
        "the successor campaign was refused in the clean world — {:?}",
        never_cancelled.envelope.error
    );
    assert_eq!(
        after_cancel.payload, never_cancelled.payload,
        "the successor campaign answered differently after a cancel"
    );
    assert_eq!(
        after_cancel.envelope.status, never_cancelled.envelope.status,
        "the successor campaign landed on a different status lane after a cancel"
    );
    assert_eq!(
        after_cancel.envelope.artifacts, never_cancelled.envelope.artifacts,
        "the successor campaign named different artifacts after a cancel"
    );

    let successor_task = match &after_cancel.payload {
        Payload::VerificationStart(response) => response
            .task
            .value()
            .cloned()
            .expect("the successor names a task"),
        other => panic!("expected a verification.start payload, got {other:?}"),
    };
    assert_eq!(
        entry(&cancelled_world, &successor_task).record(),
        entry(&clean_world, &successor_task).record(),
        "the successor's wire record differs between the two worlds"
    );

    // And the durable half: the artifact the successor published, and its receipts, are the
    // same bytes filed the same way in both stores.
    let poisoned_store = PublishedNamespace::of(&cancelled_world.daemon);
    let clean_store = PublishedNamespace::of(&clean_world.daemon);
    let successor_rows: Vec<&String> = clean_store.artifacts.keys().collect();
    assert!(
        !successor_rows.is_empty(),
        "the clean world published nothing; there is nothing to compare"
    );
    for handle in successor_rows {
        assert_eq!(
            poisoned_store.artifacts.get(handle),
            clean_store.artifacts.get(handle),
            "{handle} holds different bytes in a world that cancelled a campaign"
        );
    }
    assert!(
        census(tree(&cancelled_world)).is_empty(),
        "the cancelled world owes an obligation: {}",
        render_census(tree(&cancelled_world))
    );
}

// =========================================================================================
// 5. cancellation is a capability, not an ambient
// =========================================================================================

/// **ADR-0003 at the cancel.** A `Read`-level actor cannot cancel: the request is refused
/// with the one undistinguished denial, no scope is opened, no obligation is created, and the
/// published namespace does not move.
///
/// `task.cancel` declares `authority: Execute` in the plan §10.2 registry, so the refusal is
/// the capability model working rather than an accident of routing. The delivering evidence's
/// only negative case is a task the daemon does not hold, which never reaches an authority
/// question at all.
#[test]
fn cancellation_is_an_explicit_capability_and_not_ambient() {
    let mut fixture = fixture();
    let task = wind_up(&mut fixture, &PROFILES[0]);
    let before = PublishedNamespace::of(&fixture.daemon);
    let record_before = entry(&fixture, &task).record();

    let denied = ledger_shadow(
        &mut fixture,
        "read-level-cancel",
        Predicted::default(),
        |fixture| cancel_as(fixture, &task, "req_denied", "agent:reader", "cap_reader"),
    );
    assert_eq!(
        denied.error_code(),
        Some(ErrorCode::CapabilityDenied),
        "a read capability was allowed to cancel"
    );
    assert_eq!(denied.payload, Payload::None, "no partial effect");
    assert_eq!(
        entry(&fixture, &task).record(),
        record_before,
        "a denied cancel moved the task record"
    );
    assert_eq!(
        PublishedNamespace::of(&fixture.daemon).rendered.as_bytes(),
        before.rendered.as_bytes(),
        "a denied cancel moved the published namespace"
    );
    assert!(
        census(tree(&fixture)).is_empty(),
        "a denied cancel left an obligation open: {}",
        render_census(tree(&fixture))
    );

    // The control: the same request under the Execute capability is admitted, so the refusal
    // above is about authority and not about the request.
    let admitted = cancel(&mut fixture, &task, "req_admitted");
    assert_eq!(
        admitted.envelope.status,
        ResultStatus::Ok,
        "the Execute capability was refused too — {:?}",
        admitted.envelope.error
    );
}

// =========================================================================================
// 6. "either committed or absent" — nothing is staged at rest, and the injured case
// =========================================================================================

/// **DX-14's second conjunct, probed rather than argued.** At every observable point of every
/// profile — before the cancel and after it — no task holds a staged publication, no
/// finalization reports an unresolved one, the store's abort log is empty, and `fsck` is
/// clean.
#[test]
fn scope_no_publication_is_staged_at_rest_and_why_that_is_a_bound_not_a_proof() {
    let token = root_token();
    for profile in PROFILES {
        let mut fixture = fixture();
        let task = wind_up(&mut fixture, profile);

        let check = |when: &str, fixture: &Fixture| {
            for handle in fixture.daemon.state().tasks().handles() {
                let entry = fixture
                    .daemon
                    .state()
                    .tasks()
                    .get(handle)
                    .expect("the handle came from the table");
                assert!(
                    !entry.evidence.is_staging(),
                    "{}/{when}: {} holds a publication that is neither committed nor absent",
                    profile.token,
                    handle.as_str()
                );
            }
            for report in fixture.daemon.state().regions().finalizations() {
                assert!(
                    report.unresolved_publications().is_empty(),
                    "{}/{when}: a teardown left a staged publication behind",
                    profile.token
                );
            }
            let view = fixture
                .daemon
                .store()
                .audit_view(&token)
                .expect("`cap_root` confers audit");
            assert!(
                view.aborts().is_empty(),
                "{}/{when}: the store recorded an abort: {:?}",
                profile.token,
                view.aborts()
            );
            assert!(
                view.fsck().is_empty(),
                "{}/{when}: the store's index verifier found {:?}",
                profile.token,
                view.fsck()
            );
        };

        check("before the cancel", &fixture);
        let outcome = cancel(&mut fixture, &task, "req_cancel");
        assert_eq!(outcome.envelope.status, ResultStatus::Ok);
        check("after the cancel", &fixture);
    }
}

/// **The nearest reachable partial finality, constructed.** A storage seam that fails the
/// index commit puts the store in the one state docs/35 says a crash can leave: content
/// committed, no index entry naming it.
///
/// What must then be true, and is:
///
/// 1. the call fails with `PublicationAborted` — INV-017's "nothing is published and nothing
///    is truncated";
/// 2. the store's index is unchanged and no receipt was issued, so **nothing a reader can
///    reach** moved;
/// 3. `fsck` finds the residue and classifies it as `UnreachableContent`, which is
///    GC-collectable and is not a partial artifact;
/// 4. the task claims **zero** publications and holds nothing staged: the daemon discarded
///    its side, so the two ledgers disagree only in the direction docs/35 chose;
/// 5. cancelling that task lands on the `nothing-published` arm and closes every obligation.
///
/// This doubles as a second negative control for [`PublishedNamespace`]: the `fsck` and
/// `aborts` rows of the fingerprint are shown to be non-empty on a world that deserves it.
#[test]
fn negative_control_an_injected_storage_fault_leaves_content_no_reader_can_reach() {
    let armed = Arc::new(AtomicBool::new(false));
    let mut fixture = fixture_over(Some(Arc::clone(&armed)));
    let token = root_token();

    let before = PublishedNamespace::of(&fixture.daemon);
    let indexed_before = {
        let view = fixture
            .daemon
            .store()
            .audit_view(&token)
            .expect("audit")
            .published_count();
        assert!(view > 0, "the fixture's own seal published nothing");
        view
    };

    armed.store(true, Ordering::SeqCst);
    let refused = start(&mut fixture, "req_injured", 4, PROFILES[0].target());
    assert_eq!(
        refused.error_code(),
        Some(ErrorCode::PublicationAborted),
        "the injected fault did not reach the campaign record's publication"
    );
    assert_eq!(refused.payload, Payload::None, "no partial effect");
    armed.store(false, Ordering::SeqCst);

    let view = fixture
        .daemon
        .store()
        .audit_view(&token)
        .expect("`cap_root` confers audit");
    assert_eq!(
        view.published_count(),
        indexed_before,
        "an aborted publication added an index entry"
    );
    let aborts = view.aborts();
    assert_eq!(aborts.len(), 1, "the abort was not recorded: {aborts:?}");
    assert_eq!(aborts[0].phase(), PublicationPhase::CommittingIndex);
    assert_eq!(aborts[0].reason(), AbortReason::StorageExhausted);

    let defects = view.fsck();
    assert_eq!(
        defects.len(),
        1,
        "the index verifier did not find the residue: {defects:?}"
    );
    let unreachable = matches!(
        &defects[0],
        continuum_workspace::publication::StoreDefect::UnreachableContent(_)
    );
    assert!(
        unreachable,
        "the residue was classified as {:?}, not as unreachable content",
        defects[0]
    );

    // The fingerprint sees it, which is the control this test also serves.
    let after = PublishedNamespace::of(&fixture.daemon);
    assert_ne!(
        after.rendered.as_bytes(),
        before.rendered.as_bytes(),
        "the fingerprint is blind to an injured store"
    );
    assert!(
        after.rendered.contains("fsck 1"),
        "the fingerprint did not carry the defect it is supposed to report"
    );
    assert_eq!(
        after.artifacts.keys().collect::<Vec<_>>(),
        before.artifacts.keys().collect::<Vec<_>>(),
        "an aborted publication became readable"
    );

    // The task's own side: nothing claimed, nothing staged.
    let task = only_task(&fixture);
    assert_eq!(
        entry(&fixture, &task).publications(),
        0,
        "the task claimed a publication the store never indexed"
    );
    assert!(
        !entry(&fixture, &task).evidence.is_staging(),
        "the task is still holding the publication that aborted"
    );

    // And cancelling it is the nothing-published arm, with every obligation closed.
    let cancelled = ledger_shadow(
        &mut fixture,
        "injured/cancel",
        Predicted {
            scopes: 1,
            reserves: 0,
            escalations: 1,
        },
        |fixture| cancel(fixture, &task, "req_cancel"),
    );
    assert_eq!(cancelled.envelope.status, ResultStatus::Ok);
    let arm = fixture
        .daemon
        .state()
        .regions()
        .finalizations()
        .last()
        .and_then(|report| report.workers().first())
        .and_then(|worker| worker.cancel_outcome().map(|outcome| outcome.token()))
        .expect("the cancel's own teardown");
    assert_eq!(
        arm, "nothing-published",
        "a task whose publication aborted reported a committed arm"
    );
    assert!(
        census(tree(&fixture)).is_empty(),
        "the injured world owes an obligation: {}",
        render_census(tree(&fixture))
    );
}

// =========================================================================================
// 7. scope — what is cancellable here, and what is unprobed
// =========================================================================================

/// **The narrowing, as an executable assertion (INV-007).**
///
/// `task.cancel` cancels a `task_*`, and a `task_*` is created by a `@task_starting`
/// operation. The IDL declares 26 of them and this daemon serves one, so 25 task classes are
/// unprobed by this file and by every other file in the tree — an absence, not a pass.
#[test]
fn scope_the_cancellable_operation_classes_this_daemon_serves() {
    let declared: Vec<&str> = OPERATIONS
        .iter()
        .filter(|spec| spec.has(Annotation::TaskStarting))
        .map(|spec| spec.name)
        .collect();
    assert_eq!(
        declared.len(),
        26,
        "the registry's `@task_starting` count moved; this file's scope statement is stale"
    );

    // Which of them this daemon answers, established by asking it. A family that is not
    // mounted refuses with `MalformedRequest` at the dispatcher, before any handler runs.
    let mut fixture = fixture();
    let served = start(&mut fixture, "req_scope", 64, PROFILES[2].target());
    assert!(
        served.error_code().is_none(),
        "verification.start is not served: {:?}",
        served.envelope.error
    );
    assert!(
        declared.contains(&"verification.start"),
        "verification.start is not a `@task_starting` operation"
    );

    // `task.cancel` itself is `@mutation` and `Execute`, and it is not `@task_starting`: it
    // names a task, it does not create one.
    let spec = OPERATIONS
        .iter()
        .find(|spec| spec.name == "task.cancel")
        .expect("task.cancel is in the registry");
    assert!(spec.has(Annotation::Mutation));
    assert!(!spec.has(Annotation::TaskStarting));
    assert_eq!(spec.authority, AuthorityLevel::Execute);

    // The absence, counted rather than described.
    let unprobed = declared.len() - 1;
    assert_eq!(
        unprobed, 25,
        "25 `@task_starting` operation classes have no handler in this daemon and are \
         therefore unprobed by this criterion's evidence"
    );
}

// =========================================================================================
// 8. determinism
// =========================================================================================

/// One canonical rendering of the whole G1-05 script: every profile, its census, and the
/// published namespace the cancel left.
fn render_script() -> String {
    let mut out = String::new();
    for profile in PROFILES {
        out.push_str(&format!("== {} ==\n", profile.token));
        let mut fixture = fixture();
        let task = wind_up(&mut fixture, profile);
        let outcome = cancel(&mut fixture, &task, "req_cancel");
        let (status, continuation) = match &outcome.payload {
            Payload::TaskCancel(response) => (response.status, response.continuation.clone()),
            other => panic!("expected a task.cancel payload, got {other:?}"),
        };
        out.push_str(&format!(
            "cancel status={status:?} continuation={} verdict={:?}\n",
            continuation != Nullable::Null,
            outcome.envelope.verdict
        ));
        out.push_str(&render_census(tree(&fixture)));
        out.push_str(&fixture.daemon.state().regions().render());
        out.push_str(&PublishedNamespace::of(&fixture.daemon).rendered);
    }
    out
}

/// > identical requests against equal states produce equal results
/// >
/// > — `rule ordering.deterministic`
///
/// Bytes rather than structural equality: an ordering difference hides inside set equality
/// and does not hide inside a byte comparison.
#[test]
fn the_whole_g1_05_script_renders_byte_identically_across_two_runs() {
    let first = render_script();
    let second = render_script();
    assert_eq!(first.as_bytes(), second.as_bytes());
    assert!(!first.is_empty(), "a vacuous render proves nothing");
    for token in [
        "census probes=",
        "held=0",
        "fsck 0",
        "aborts 0",
        "staged: none",
        "daemon total: orphans=0 unfinalized=0 defects=0 balanced=true",
    ] {
        assert!(
            first.contains(token),
            "the rendering must carry the facts it compares — {token} is missing"
        );
    }
    assert!(
        first.lines().count() > 60,
        "a rendering this short is not the script"
    );
}
