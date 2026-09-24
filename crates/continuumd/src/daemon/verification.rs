//! The `verification` family — `start`, `result`, `await` — and the one place in this
//! daemon where an engine actually runs.
//!
//! # What `verification.start` can honestly construct, and what it refuses
//!
//! An operation that starts a verification campaign needs a *model*, and the only path from
//! a workspace snapshot to a model is a CML front end. **There is no CML front end.** The
//! Finite core fragment's elaborator is PR 15a; `continuum-cml-syntax` and
//! `continuum-cml-elab` are scaffolds, and `continuum-engine-reference`'s own Die Hard
//! module says outright that "until the CML front end exists, this module *is* the port"
//! (`crates/continuum-engine-reference/src/diehard.rs`).
//!
//! So this family draws the line where the truth is, rather than guessing across it:
//!
//! - a deployment **registers** the models this daemon can construct, out of band, keyed by
//!   the content identity of the CML source they were transcribed from — the same
//!   administrative surface capability provisioning and content staging already use
//!   (`DaemonState`, IDL §7);
//! - `verification.start` derives that identity from the sealed snapshot's own `.ctm`
//!   modules and looks it up. A snapshot whose modules are not a registered model is
//!   [`ErrorCode::UnsupportedSemanticFeature`] — the code
//!   `rule errors.unsupported_surface` requires and this operation's `errors` clause
//!   declares — and never a degraded answer, an empty success, or a guess.
//!
//! The identity is derived from the source bytes rather than from the snapshot handle, so a
//! fork that edits `README.md` still resolves to the same model, and one that edits the
//! `.ctm` resolves to nothing until a model for it is registered. That is the fail-closed
//! direction: a changed model is a different model.
//!
//! What the PR 8 exit needs is exactly this much. Die Hard is registered, the campaign runs,
//! and "Die Hard returns 16 states and depth-6 solution through the daemon API" is a test
//! that goes through [`Daemon::dispatch`](super::Daemon::dispatch) and nothing else.
//!
//! # The campaign, and where its answers come from
//!
//! | Wire answer | Engine value |
//! |---|---|
//! | `Cost.states` | `CheckReport::scope().states()` |
//! | `SemanticVerdictValue.verdict` | `CheckReport::verdict()`, whose fold is "a refutation dominates" |
//! | `SemanticVerdictValue.inconclusive_reason` | the report's own `Unresolved`, never inferred from the scope |
//! | `SemanticVerdictValue.assurance_class` | `Scope::Complete` → `validated`; `Scope::Bounded` → `bounded` |
//! | `VerificationResult.continuation` | the parked frontier's `cont_*` handle |
//! | `AssuranceEnvelope.bounds` | docs/03 §3's `EXHAUSTIVE_FINITE` / `BOUNDED_STATES` |
//!
//! Nothing here re-decides anything the engine decided. In particular **budget exhaustion is
//! not a verdict**: a bounded run answers `inconclusive` with the typed
//! [`InconclusiveReason::ResourceExhausted`] and carries the continuation, which is INV-008
//! and docs/49 stated as a value rather than as a discipline.
//!
//! # The one budget dimension this daemon enforces
//!
//! RFC 0026's `Budget` has nine dimensions and this daemon can honestly enforce one:
//! `states`, which is the engine's own state bound. `wall_ms` and `cpu_ms` need a clock that
//! INV-005 keeps out of the core, `memory_bytes` needs an allocator this layer does not own,
//! and `solver_ms`, `proof_ms`, `tokens`, `candidates` and `bytes` name lanes that are not
//! running. A dimension a caller *declares* and this daemon does not enforce is reported as
//! a typed [`Omission`] on the result, because INV-007 asks a bounded answer to name what it
//! left out — silently ignoring a declared ceiling is exactly the failure that rule exists
//! to prevent. The engine's depth and transition bounds have no wire dimension at all and
//! are taken from `Bounds::CERTIFIABLE`, the kernel wire form's own ceilings.
//!
//! That paragraph is now a *value* rather than a claim (bn-23j7s). It is
//! [`budget::METERS`](super::budget::METERS), one `MeterSet` naming the single meter this
//! daemon has, and the omission manifest is derived from it by
//! [`budget::omissions_of`](super::budget::omissions_of). The eight-name `unenforced` array
//! this module used to carry is gone: it said the same thing, but a deployment that grew a
//! clock would have kept reporting `budget.wall_ms` as unenforced until somebody remembered
//! to delete a line. `bounds_of` moved with it, to
//! [`budget::bounds_of`](super::budget::bounds_of), where it is a projection of the ledger's
//! `states` ceiling instead of a second read of the same wire field.

use std::borrow::Cow;
use std::collections::BTreeMap;

use continuum_engine_reference::bfs::{self, Bounds, ExplorationError, Partial};
use continuum_engine_reference::checking::{
    self, DeadlockPolicy, Obligations, Scope, Verdict as EngineVerdict,
};
use continuum_engine_reference::model::{Model, State};
use continuum_task::budget::dimension::CostDimension;
use continuum_task::region::worker::WorkerStep;
use continuum_workspace::artifact_path::ArtifactClass;
use continuum_workspace::publication::{
    CapabilityToken, ContentIdentifier, PublishRefusal, Published, ReferenceStore,
};
use continuum_workspace::snapshot::Snapshot;
use continuum_workspace::staleness::{LineageError, check_current};

use super::Services;
use super::admission::Derived;
use super::budget::{self, Publications};
use super::continuation::{self, Checkpointed, ContinuationRecord, ParkState};
use super::family::{Arguments, Call, Effect, Fault, OperationFamily, Payload, ScopeClaim};
use super::terminal::{self, TerminalRecord, TerminalState};
// `Scope` is `continuum_engine_reference::checking::Scope` in this module — the exploration's
// completeness — so the region scope is imported under a name that says which of the two it
// is rather than shadowing the engine's vocabulary.
use super::recovery::{Resolution, ResolvedTask};
use super::region::{self, Scope as RegionScope};
use super::state::DaemonState;
use super::task::{
    Campaign, Continuation, PinnedEpochs, Preimage, RefusedRun, TaskEntry, budget_preimage,
    epochs_preimage, published, task_handle, unsupported,
};
use crate::protocol::envelope::{
    ArtifactRef, AssuranceEnvelope, Budget, EnvelopeDimension, Omission, ProducedDimension,
    SemanticVerdictValue, UnsupportedDimension, Verdict,
};
use crate::protocol::operations::verification::{
    VerificationStartRequest, VerificationStartResponse,
};
use crate::protocol::scalar::{
    ArtifactHandle as WireArtifactHandle, Commitment, ContinuationHandle, TaskHandle,
};
use crate::protocol::shared::{Target, VerificationResult};
use crate::protocol::spec::{Nullable, Optional, ProtocolEnum};
use crate::protocol::task::Milestone;
use crate::protocol::vocabulary::{
    AssuranceClass, ErrorCode, Fragment, InconclusiveReason, Portfolio, PriorityClass,
    SemanticVerdict, TargetKind, TaskStatus,
};

/// The engine every dimension of this daemon's assurance envelope names as its producer.
///
/// One `const`, because a producing engine is a *fact about the artifact* and a second
/// spelling of it is how two answers come to disagree about which engine produced them.
pub const ENGINE: &str = "continuum-engine-reference";

/// The milestone a closed exploration records.
pub const MILESTONE_CLOSED: &str = "exploration.closed";
/// The milestone a bounded exploration records.
pub const MILESTONE_BOUNDED: &str = "exploration.bounded";
/// The milestone a finished check records.
pub const MILESTONE_CHECKED: &str = "checking.complete";

// ---------------------------------------------------------------------------
// the model catalog
// ---------------------------------------------------------------------------

/// The models this daemon can construct, keyed by the content identity of the CML source
/// each was transcribed from.
///
/// Not a compiler and not a cache: it is the honest surface of a daemon that has no CML
/// front end yet. Registration is out of band for the same reason capability minting is
/// (IDL §7) — it is administration, not an operation — and keying on source identity is what
/// makes it fail closed, because a snapshot whose `.ctm` bytes changed derives a different
/// identity and resolves to nothing.
#[derive(Debug, Default)]
pub struct ModelCatalog {
    models: BTreeMap<Commitment, Model>,
}

impl ModelCatalog {
    /// The empty catalog.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Register `model` as the elaboration of the CML source `source` names.
    pub fn register(&mut self, source: Commitment, model: Model) {
        self.models.insert(source, model);
    }

    /// The model `source` names, or [`None`] when this daemon cannot construct it.
    #[must_use]
    pub fn get(&self, source: &Commitment) -> Option<&Model> {
        self.models.get(source)
    }

    /// How many models are registered.
    #[must_use]
    pub fn len(&self) -> usize {
        self.models.len()
    }

    /// Whether the catalog is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.models.is_empty()
    }
}

/// The extension every CML module carries (`notes/plan/corpus/tla-examples/ports/`).
const MODULE_EXTENSION: &str = ".ctm";

/// The content identity of a set of CML modules.
///
/// The preimage is the length-prefixed `(path, content)` record of every module in path
/// order, so two snapshots agree on this identity exactly when they agree on every module's
/// path *and* bytes — the same canonical-record discipline `DaemonState::stage` uses, and for
/// the same reason: a model is placed source, not a bag of bytes.
///
/// Returns [`None`] when the set is empty, or when the identity seam refuses to name it.
#[must_use]
pub fn model_source<'a, I>(identifier: &dyn ContentIdentifier, modules: I) -> Option<Commitment>
where
    I: IntoIterator<Item = (&'a str, &'a [u8])>,
{
    let mut preimage = Preimage::new();
    let mut named = false;
    for (path, content) in modules {
        preimage.text(path);
        preimage.push(content);
        named = true;
    }
    if !named {
        return None;
    }
    identifier
        .identify(ArtifactClass::ElaboratedModel, preimage.bytes())
        .ok()
        .map(|handle| Commitment::new(&handle.to_string()))
}

/// The CML modules a snapshot carries, in path order.
fn snapshot_modules(snapshot: &Snapshot) -> Vec<(String, Vec<u8>)> {
    snapshot
        .files()
        .into_iter()
        .filter_map(|(path, node)| {
            let rendered = path.to_string();
            rendered
                .ends_with(MODULE_EXTENSION)
                .then(|| (rendered, node.content().to_vec()))
        })
        .collect()
}

// ---------------------------------------------------------------------------
// budget → bounds
// ---------------------------------------------------------------------------

/// Which bound a partial exploration tripped.
#[must_use]
pub fn tripped(partial: &Partial) -> bfs::Bound {
    partial.tripped()
}

// ---------------------------------------------------------------------------
// running one campaign
// ---------------------------------------------------------------------------

/// What the caller's target asks to be checked, over which completion policy.
///
/// The deadlock half is the caller's declaration in the intent contract's vocabulary, and
/// reading it off the contract is RFC 0037 work this bone does not do. The strictest policy
/// is taken instead of a guess, and it is the safe direction rather than a convenient one:
/// [`DeadlockPolicy::Defect`] can only turn a state with no enabled action into a *reported*
/// defect, never hide one, so a campaign under it claims no more than a campaign under the
/// contract's own policy would.
///
/// # Errors
///
/// [`ErrorCode::MalformedRequest`] when the target names a predicate the model does not
/// declare — there is no "unknown property" code in the §10.3 taxonomy, and a `Target.id`
/// that names nothing in the model the request's own snapshot elaborates to does not validate
/// against what a target means. Nothing is revealed by saying so: the caller is already
/// admitted for this snapshot, so this is not the existence oracle RFC 0027 X2 forbids.
///
/// [`ErrorCode::UnsupportedSemanticFeature`] for a target kind no finite reachability
/// campaign answers.
fn obligations(model: &Model, target: &Target) -> Result<Obligations, Fault> {
    match target.kind {
        TargetKind::AllClaims | TargetKind::Module => {
            Ok(Obligations::every_predicate(model, DeadlockPolicy::Defect))
        }
        TargetKind::Property | TargetKind::Claim => {
            let index = model.predicate_index(&target.id).ok_or_else(|| {
                Fault::new(
                    ErrorCode::MalformedRequest,
                    "the target names a predicate the model this snapshot elaborates to does \
                     not declare",
                )
            })?;
            Ok(Obligations::new(DeadlockPolicy::Defect).invariant(index))
        }
        TargetKind::Obligation
        | TargetKind::Refinement
        | TargetKind::Program
        | TargetKind::BenchmarkTask => Err(Fault::new(
            ErrorCode::UnsupportedSemanticFeature,
            "this daemon answers a finite reachability campaign, which this target kind is \
             not a question about",
        )),
    }
}

/// One bounded run: explore, then check.
///
/// # `prior_frontier`, and what it guards
///
/// The parked continuation's own frontier — empty on a fresh `verification.start`, and the
/// resumed continuation's `Continuation::frontier` on a `task.resume`. Nothing downstream of
/// this function reads it as a search seed: `bfs::explore` has no partial-state entry point,
/// and this call always re-explores `model` from its initial states under `bounds`. That is
/// RFC 0026's "cont_* semantics" disposition ("Continuations and resume admissibility") —
/// `bounds`/`frontier` are pinned as *provenance*, not fed back in as a resume instruction —
/// and it is sound today only because canonical breadth-first exploration is a deterministic
/// function of `(model, bounds)`: a larger bound's reachable set is provably a superset of a
/// smaller bound's, so the parked frontier is always rediscovered. `prior_frontier` is here so
/// that fact is *checked*, not trusted: a future engine binding — a real partial-state resume,
/// a non-canonical search order, a portfolio solver — that broke the superset property while
/// `Continuation::frontier` stayed unread would otherwise fail silently, which is exactly the
/// concern the disposition names. See `tests/daemon_task_operations.rs` for the same property
/// re-derived independently, and `tests/dx03_falsification.rs`'s attack 16.
///
/// # Errors
///
/// [`ErrorCode::UnsupportedSemanticFeature`] when the model's transition relation is
/// undefined at a state the model itself reaches — docs/16 PO-MOD-003 failing. That is a
/// defect in the declaration rather than a budget question, and resuming would fail
/// identically because the engine is deterministic, so no task is left behind for it.
///
/// [`Ok(Err(..))`] is not a shape here: a bound that trips is a *result*
/// ([`Exploration::Exhausted`]), never an error, which is the whole reason a parked
/// continuation exists.
fn run(
    model: &Model,
    target: &Target,
    bounds: Bounds,
    prior_frontier: &[State],
) -> Result<Result<Campaign, Fault>, Fault> {
    let obligations = obligations(model, target)?;
    let exploration = match bfs::explore(model, bounds) {
        Ok(exploration) => exploration,
        Err(ExplorationError::InitialStatesExceedBound { .. }) => {
            // Not a partial result and not resumable: there is no prefix of the walk to
            // report, because initial states are what exploration starts *from*. This is the
            // `failed_reason = BudgetExhausted` with no continuation that RFC 0026 requires a
            // `non_resumable_reason` beside.
            return Ok(Err(Fault::exhausted(
                "the declared state budget cannot hold the model's own initial states",
            )));
        }
        Err(ExplorationError::Evaluation { .. }) => {
            return Err(Fault::new(
                ErrorCode::UnsupportedSemanticFeature,
                "the model's transition relation is undefined at a state it reaches",
            ));
        }
    };
    // The non-prefix guard bn-10wdo's disposition requires: every state the parked
    // continuation left on its frontier MUST be rediscovered by this run. Debug-only because
    // it is a property of the *engine binding*, checked on every run rather than sampled, and
    // this daemon has exactly one engine to check it against; a future non-BFS engine that
    // needs this to be a hard, release-mode refusal is the moment to promote it, not remove
    // it, per the same disposition.
    debug_assert!(
        prior_frontier
            .iter()
            .all(|state| exploration.reachable().contains(state)),
        "a resumed run's exploration did not rediscover a state the parked continuation's \
         frontier named — RFC 0026's cont_* semantics disposition holds only while every \
         engine's resume is a full re-exploration that provably extends the parked one; a \
         partial-state or non-canonical engine breaks that silently unless this fires"
    );
    let report = checking::check(model, &exploration, &obligations).map_err(|_| {
        Fault::new(
            ErrorCode::MalformedRequest,
            "the target names a predicate the model this snapshot elaborates to does not \
             declare",
        )
    })?;
    // An undefined read at a state the model reaches (RFC 0003 "Definedness", RFC 0013:
    // it invalidates the model, bn-24a5c) is the same defect class as a transition
    // relation that is undefined there: no `InconclusiveReason` names it, no verdict is
    // one, and a resume would find it again. So it is refused the same way — unless the
    // report is refuted anyway: an undefined read in one invariant leaves another
    // invariant's counterexample sound (every action on its path is defined, because an
    // undefined action read makes every outcome `Undefined`), and "a refutation
    // dominates" is the report's own fold.
    if report.undefined().is_some() && report.verdict() != EngineVerdict::Refuted {
        return Err(Fault::new(
            ErrorCode::UnsupportedSemanticFeature,
            "the model reads a value that is undefined at a state it reaches",
        ));
    }
    Ok(Ok(Campaign::of(&exploration, report)))
}

/// Run the task `handle` names one more time under `bounds`, and write what happened.
///
/// This is the single writer of a task's outcome: `verification.start` calls it to run a
/// fresh task and `task.resume` calls it to run a parked one, so the two cannot drift into
/// recording different things about the same campaign. It is therefore also the one place
/// PR 6's rule applies — **a unit of task work runs inside a region** — and
/// [`region::scoped`] is the bracket: the scope is opened before the model is looked up and
/// finalized on every path out, including the two that return a [`Fault`] and the one where
/// the engine's own bound tripped.
///
/// # The run, as the region layer sees it
///
/// | What happened | Worker step | Why there |
/// |---|---|---|
/// | the run started | `Begin` | `TaskStatus::Running`, which is representable and never reported |
/// | the model is not one this daemon holds | `Fail(UnsupportedSemanticFeature)` | unreachable: both callers refuse it before their first write (see [`run_in`]); kept typed rather than a panic |
/// | the state budget cannot hold the initial states | `Fail(BudgetExhausted)` | RFC 0026's failure with no continuation and a `non_resumable_reason` beside it |
/// | a campaign came back | `Reserve` | staged: the result exists and no reader can see it yet |
/// | it was written onto the task | `Commit` | `verification.result` can read it now, and INV-009 makes it monotone |
/// | the exploration closed | `Complete` | |
/// | a bound tripped | `Suspend` | parked with committed partial evidence plus a valid continuation (B18) |
///
/// The order is not decoration: `Suspend` after `Commit` is the only order the region layer
/// admits, because "cancellation MUST NOT truncate a publication in progress" makes parking
/// mid-publication a typed refusal one layer down. A wiring that parked first would be
/// caught as a defect rather than shipped as a subtly wrong lifecycle.
///
/// And then the scope ends. A parked worker is not terminal, so the teardown escalates from
/// a close to a cancellation and the parked *execution* is terminated by the scope exit —
/// which is exactly right, because what survives a suspension is the `cont_*` artifact this
/// function minted and filed, not a live worker. [`region`](super::region) states the RFC
/// argument for that resolution in full.
///
/// # The durable half (bn-3dr)
///
/// The publication this function commits is written **into the store**, through
/// `continuum-workspace`'s two-phase protocol, before it is committed onto the task. That is
/// what makes IMPL-02's durability criterion — "committed partial evidence survives daemon
/// restart; uncommitted partials are absent, never half-visible" — a statement with a
/// referent: the task table is volatile and does not survive a restart, but the campaign
/// record does, under the identity the task named it by. See [`publish_record`] for the
/// ordering and [`recovery`](super::recovery) for what a restart can then say about it.
///
/// `prior_frontier` is the parked continuation's frontier on a `task.resume`, and empty on a
/// fresh `verification.start` — see [`run`]'s doc for what it guards (bn-10wdo's cont_*
/// semantics disposition).
///
/// # Errors
///
/// [`Fault`] when the model can no longer be constructed, when the identity seam refuses to
/// name a continuation, when the run itself is not a question about this model, or —
/// [`ErrorCode::PublicationAborted`] — when the durable publication of the campaign record
/// could not complete atomically (INV-017).
#[allow(clippy::too_many_arguments)]
pub fn advance(
    handle: &TaskHandle,
    bounds: Bounds,
    prior_frontier: &[State],
    state: &mut DaemonState,
    services: &Services,
    store: &ReferenceStore,
    publisher: &CapabilityToken,
    authorize: &mut dyn FnMut(&ContinuationHandle, u64) -> bool,
) -> Result<(), Fault> {
    let (source, target, snapshot, intent) = {
        let entry = state.tasks().get(handle).ok_or_else(Fault::denied)?;
        (
            entry.model.clone(),
            entry.target.clone(),
            entry.snapshot.clone(),
            entry.intent.clone(),
        )
    };
    let now = services.now().cloned();
    // A campaign is admitted as resumable work: the reference engine's own
    // `Exploration::Exhausted` carries the queue-ordered frontier, so a bounded run has a
    // resume point by construction. The one outcome that has none — a budget too small to
    // hold the initial states — is a *failure*, and a failure does not park.
    region::scoped(state, region::resumability(true), |state, scope| {
        {
            let entry = state
                .tasks_mut()
                .get_mut(handle)
                .ok_or_else(Fault::denied)?;
            entry.region = Some(scope.region());
            entry.worker = Some(scope.worker());
        }
        state.regions_mut().step(scope, WorkerStep::Begin);
        run_in(
            scope,
            handle,
            bounds,
            prior_frontier,
            &source,
            &target,
            &snapshot,
            &intent,
            &now,
            state,
            services,
            store,
            publisher,
            authorize,
        )
    })?;
    Ok(())
}

/// Write one campaign record into the store, and check the two identity seams agree.
///
/// The store's own protocol, in its own order: stage, commit the content, commit the index.
/// A crash between the last two — the INV-017 point docs/35 names, injectable through
/// [`StorageFaults`](continuum_workspace::publication::StorageFaults) — leaves unreachable
/// content and no index entry, which is the asymmetry docs/35 chose deliberately and which
/// `crates/continuum-workspace/tests/dx13_falsification.rs` proved for the store. Nothing
/// here re-proves it; what this function adds is that the *daemon's* name for the record and
/// the *store's* are checked against each other, the way
/// [`SealedWorkspace::seal`](continuum_workspace::seal::SealedWorkspace::seal) checks its
/// own. A disagreement is refused rather than reconciled: adopting the store's name would
/// rewrite the publication's identity, and adopting the daemon's would file a fetch that
/// misses.
///
/// # Errors
///
/// [`ErrorCode::CapabilityDenied`] when the publisher may not publish an `ArtifactClass::Task`
/// record, and [`ErrorCode::PublicationAborted`] when the publication could not complete
/// atomically or when the two identity seams disagree. On every error path nothing is
/// published and nothing is truncated.
///
/// On success, `named` tied to the store's receipt (bn-283p6): the one value
/// [`Publications::commit`](super::budget::Publications::commit) accepts, so the task can
/// name the record only after the store has committed it (INV-017).
fn publish_record(
    store: &ReferenceStore,
    publisher: &CapabilityToken,
    record: Vec<u8>,
    named: &Commitment,
) -> Result<Published<Commitment>, Fault> {
    let aborted = || {
        Fault::new(
            ErrorCode::PublicationAborted,
            "the campaign record's publication aborted; nothing was published and nothing \
             was truncated",
        )
    };
    let receipt = store
        .stage(ArtifactClass::Task, record, publisher)
        .and_then(|staged| Ok(staged.commit_content()?.commit_index()?))
        .map_err(|refusal| match refusal {
            PublishRefusal::CapabilityDenied(_) => Fault::denied(),
            PublishRefusal::Aborted(_) => aborted(),
        })?;
    // The two identity seams disagree on the same bytes when the receipt does not attest
    // the daemon's name, so a retry derives the same disagreement: deterministic, whatever
    // the taxonomy default says.
    Published::attest(&receipt, named.clone()).map_err(|_| aborted().not_retryable())
}

/// Publish a terminal record (bn-2g3ei) through [`publish_record`], under the identity the
/// daemon's own seam names its bytes by.
///
/// # Errors
///
/// As [`publish_record`], and [`ErrorCode::PublicationAborted`] when no identity can be
/// derived for the bytes. Nothing is published on an error path.
fn publish_terminal(
    store: &ReferenceStore,
    publisher: &CapabilityToken,
    services: &Services,
    record: &TerminalRecord,
) -> Result<(), Fault> {
    let named = record.commitment(services.identifier())?;
    // No daemon record names the terminal record: a restart finds it by reading the store
    // (`recovery::resolve_tasks`). So its receipt-tied name is not kept.
    publish_record(store, publisher, record.encode(), &named).map(|_| ())
}

/// One run, inside the scope that owns it. See [`advance`] for the step table.
///
/// `prior_frontier` is the parked continuation's frontier on a resume, and empty on a fresh
/// `verification.start` — see [`run`]'s doc for what it guards.
/// The denial for a run whose minted continuation is outside the caller's grant
/// (`rule capability.instance_scope`, the minted-handle clause; cr-3hcpn4).
///
/// The run has happened once, and its results are discarded: the staged publication is
/// dropped, nothing is published, and no continuation or record exists. The region records
/// the failure. Nothing about the task itself is written — a task identity is shared by
/// every caller that names it, so a refusal must not change it for anyone else. The work
/// is charged to the refused caller instead: the operation that called [`advance`] learns
/// the explored state count through its `authorize` callback and records the refused run
/// against the presenting capability ([`TaskTable::refuse_run`]), which refuses the same
/// run again before any exploration.
///
/// The fault is the one `CapabilityDenied` (X1), flagged for the denial counter.
///
/// [`TaskTable::refuse_run`]: super::task::TaskTable::refuse_run
fn refuse_minted(scope: RegionScope, handle: &TaskHandle, state: &mut DaemonState) -> Fault {
    discard(handle, state);
    state.regions_mut().fail(scope, ErrorCode::CapabilityDenied);
    let mut denied = Fault::denied();
    denied.derived_denial = true;
    denied
}

#[allow(clippy::too_many_arguments)]
fn run_in(
    scope: RegionScope,
    handle: &TaskHandle,
    bounds: Bounds,
    prior_frontier: &[State],
    source: &Commitment,
    target: &Target,
    snapshot: &Nullable<crate::protocol::scalar::WorkspaceHandle>,
    intent: &Nullable<crate::protocol::scalar::IntentHandle>,
    now: &Option<crate::protocol::scalar::Timestamp>,
    state: &mut DaemonState,
    services: &Services,
    store: &ReferenceStore,
    publisher: &CapabilityToken,
    authorize: &mut dyn FnMut(&ContinuationHandle, u64) -> bool,
) -> Result<(), Fault> {
    // The model is data — named variables, named actions, an explicit initial-state
    // enumeration — so cloning it out of the catalog costs a copy of that data and buys the
    // disjoint borrow the task table needs. Nothing about the run depends on the copy.
    //
    // The `else` arm is unreachable from both callers, and it guards no race. Both
    // `verification::start` and `task::resume` refuse an uncatalogued model *before* their
    // first write (bn-3oocz: that is what keeps the resume refusal zero-trace). Between that
    // guard and this line the handler holds `&mut DaemonState` for the whole dispatch, and
    // `ModelCatalog` has no removal operation, so nothing can deregister the model in
    // between. The arm stays because the lookup returns an `Option` and the run needs the
    // value; it answers the same typed code rather than panicking if a future caller skips
    // the guard.
    let Some(model) = state.models().get(source).cloned() else {
        let fault = no_model();
        state.regions_mut().fail(scope, fault.code);
        return Err(fault);
    };

    let outcome = match run(&model, target, bounds, prior_frontier) {
        Ok(outcome) => outcome,
        Err(fault) => {
            state.regions_mut().fail(scope, fault.code);
            return Err(fault);
        }
    };
    let campaign = match outcome {
        Ok(campaign) => campaign,
        Err(fault) => {
            // The durable half (bn-2g3ei): the terminal record of the task *after* it fails,
            // projected and published before the status moves. The run committed no campaign
            // record, so this record's index commit is the commit point. An abort answers
            // `PublicationAborted` and the task does not become `Failed`.
            let projected = {
                let entry = state.tasks().get(handle).ok_or_else(Fault::denied)?;
                let mut record = TerminalRecord::of(entry, TerminalState::Failed);
                record.failed_reason = Some(fault.code);
                record.non_resumable_reason = Some(fault.detail.to_owned());
                record
            };
            if let Err(aborted) = publish_terminal(store, publisher, services, &projected) {
                state.regions_mut().fail(scope, aborted.code);
                return Err(aborted);
            }
            state.regions_mut().fail(scope, fault.code);
            let entry = state
                .tasks_mut()
                .get_mut(handle)
                .ok_or_else(Fault::denied)?;
            entry.failed_reason = Some(fault.code);
            entry.non_resumable_reason = Some(fault.detail.to_owned());
            entry.advance(TaskStatus::Failed, now.as_ref());
            terminal::settle(state.tasks().get(handle), &projected);
            return Ok(());
        }
    };

    // The result exists and nothing a reader can observe does. The store-side counterpart is
    // `StagedPublication`, whose phase says outright: "nothing is stored".
    //
    // The publication is *named* here and committed below (bn-23j7s). Between the two lines
    // it is staged: `TaskEntry::evidence` holds a commitment nothing can read it through —
    // `Publications::committed` does not see it and `Publications::artifacts` cannot name it
    // — which is G0-DX-14's "artifacts either committed or absent" as a property of the type
    // rather than of the paths below happening to be careful. Every path out of this function
    // from here either commits it or discards it.
    state.regions_mut().step(scope, WorkerStep::Reserve);
    let (staged, record) = {
        let sequence = state
            .tasks()
            .get(handle)
            .ok_or_else(Fault::denied)?
            .publications();
        let record = budget::publication_record(
            handle,
            sequence,
            snapshot.value().map(|snapshot| snapshot.as_str()),
            campaign.states() as u64,
            campaign.is_closed(),
            &campaign.frontier,
        );
        match budget::commitment_of(services.identifier(), &record) {
            Ok(commitment) => (commitment, record),
            Err(fault) => {
                state.regions_mut().fail(scope, fault.code);
                return Err(fault);
            }
        }
    };
    state
        .tasks_mut()
        .get_mut(handle)
        .ok_or_else(Fault::denied)?
        .evidence
        .stage(staged.clone());

    // Mint the continuation before anything durable happens, because its record is
    // published first (bn-20142). Nothing here is observable yet: the continuation is parked
    // in the table only after both durable writes succeed.
    let parked = if campaign.is_closed() {
        None
    } else {
        let pinned = PinnedEpochs::of(services.epochs());
        let Some(snapshot) = snapshot.value().cloned() else {
            // Unreachable, and it discards the staged publication rather than leaving it in
            // flight: a campaign without a snapshot could not have been started.
            state.regions_mut().fail(scope, ErrorCode::CapabilityDenied);
            discard(handle, state);
            return Err(Fault::denied());
        };
        let states = campaign.states() as u64;
        let frontier = continuation::vectors(&campaign.frontier);
        // The pin preimage, whose part order the handle has always had: task, snapshot,
        // state count, frontier, then the epochs. `continuation::pin_preimage` is the one
        // spelling, shared with the startup pass that re-derives the handle.
        let pins = continuation::pin_preimage(handle, &snapshot, states, &frontier, &pinned);
        let named = match services
            .identifier()
            .identify(ArtifactClass::Continuation, &pins)
            .map_err(|_| ())
            .and_then(|stored| ContinuationHandle::new(&stored.to_string()).map_err(|_| ()))
        {
            Ok(named) => named,
            Err(()) => {
                let fault = Fault::new(
                    ErrorCode::PublicationAborted,
                    "no content identity could be derived for a task record",
                )
                .not_retryable();
                state.regions_mut().fail(scope, fault.code);
                discard(handle, state);
                return Err(fault);
            }
        };
        // The continuation is minted here, by the run, so no request named it. It is decided
        // against the caller's grant now, after the one run and before anything durable
        // (`rule capability.instance_scope`, the minted-handle clause; cr-3hcpn4).
        if !authorize(&named, campaign.states() as u64) {
            return Err(refuse_minted(scope, handle, state));
        }
        let continuation = Continuation {
            handle: named.clone(),
            task: handle.clone(),
            snapshot,
            intent: intent.clone(),
            pinned,
            bounds,
            frontier: campaign.frontier.clone(),
        };
        // The record of the task *after* this run commits, projected before it does: the
        // milestones `reach` will append, the spend `charge_states` will land, the
        // checkpoint and the publication the commit block below writes. `continuation::
        // settle` checks the projection against the task the commit leaves.
        let record = {
            let entry = state.tasks().get(handle).ok_or_else(Fault::denied)?;
            let mut record = ContinuationRecord::of(
                entry,
                &continuation,
                frontier,
                states,
                state.tasks().next_revision(&named),
                ParkState::Suspended,
            );
            continuation::reach(&mut record, MILESTONE_BOUNDED, now.as_ref());
            continuation::reach(&mut record, MILESTONE_CHECKED, now.as_ref());
            let slot = CostDimension::States.index();
            let recorded = record.spend[slot].unwrap_or_default();
            record.spend[slot] = Some(recorded.max(states));
            record.checkpoints.push(Checkpointed {
                committed: entry.publications() + 1,
                spend: record.spend,
            });
            record.publications.push(staged.clone());
            record
        };
        Some((continuation, record))
    };

    // The terminal record of the task *after* this run closes it (bn-2g3ei), projected as
    // the continuation record is: the milestones `reach` will append, the spend
    // `charge_states` will land, the checkpoint and the publication the commit block below
    // writes. A closed run parks nothing, so the task's record carries no continuation.
    let closing = if campaign.is_closed() {
        let entry = state.tasks().get(handle).ok_or_else(Fault::denied)?;
        let mut record = TerminalRecord::of(entry, TerminalState::Completed);
        let mut reached = |name: &str| {
            if let Some(at) = now.as_ref() {
                record.milestones.push(Milestone {
                    name: name.to_owned(),
                    at: at.clone(),
                });
            }
        };
        reached(MILESTONE_CLOSED);
        reached(MILESTONE_CHECKED);
        let slot = CostDimension::States.index();
        let states = campaign.states() as u64;
        let recorded = record.spend[slot].unwrap_or_default();
        record.spend[slot] = Some(recorded.max(states));
        record.checkpoints.push(Checkpointed {
            committed: entry.publications() + 1,
            spend: record.spend,
        });
        record.publications.push(staged.clone());
        record.continuation = None;
        Some(record)
    } else {
        None
    };

    // The durable writes (bn-3dr, bn-20142), and they happen *here*: after the task-side
    // stage, which no reader can observe, and before anything that makes the publication
    // observable. The continuation record goes first and the campaign record second, so
    // the campaign record's index commit is the commit point of the park: a crash between
    // the two leaves a continuation record no task claims, which the startup pass reports
    // and does not use, and never a parked head without its continuation.
    //
    // So the two ledgers can only disagree in the direction docs/35 chose — the store may
    // hold a record the task never committed, and the task can never claim a publication
    // the store does not hold.
    //
    // An abort discards the staged publication and returns, which is the both-or-neither
    // direction: no commitment on the task, no continuation parked, no `Suspended` status
    // claiming committed evidence that is not there.
    let parked_record = match &parked {
        Some((_, record)) => {
            match continuation::publish(store, publisher, services.identifier(), record) {
                Ok(published) => Some(published),
                Err(fault) => {
                    state.regions_mut().fail(scope, fault.code);
                    discard(handle, state);
                    return Err(fault);
                }
            }
        }
        None => None,
    };
    // A closing run's terminal record goes first for the same reason (bn-2g3ei): a crash
    // between the two leaves a terminal record no task claims, never a `closed` head without
    // its terminal record.
    if let Some(record) = &closing {
        if let Err(fault) = publish_terminal(store, publisher, services, record) {
            state.regions_mut().fail(scope, fault.code);
            discard(handle, state);
            return Err(fault);
        }
    }
    let published = match publish_record(store, publisher, record, &staged) {
        Ok(published) => published,
        Err(fault) => {
            state.regions_mut().fail(scope, fault.code);
            discard(handle, state);
            return Err(fault);
        }
    };

    // Park before the status moves, so a `Suspended` task never exists without the
    // continuation `rule task.status_monotonic` says it has by definition.
    let (continuation, durable) = match parked {
        Some((continuation, record)) => {
            let named = continuation.handle.clone();
            state.tasks_mut().park(continuation);
            (Some(named), Some(record))
        }
        None => (None, None),
    };

    let closed = campaign.is_closed();
    {
        let entry = state
            .tasks_mut()
            .get_mut(handle)
            .ok_or_else(Fault::denied)?;
        // The one meter this daemon has, read (bn-23j7s). The charge is the *difference*
        // against what is already recorded, because `bfs::explore` has no partial-state
        // entry point and a resumed walk re-explores the parked prefix: charging the whole
        // count on every run would report a Die Hard campaign that parked at 3 and closed at
        // 16 as having cost 19. See `budget::charge_states`.
        //
        // A charge that could not land would be a defect in this wiring rather than an answer
        // to the caller — `bfs` refuses the expansion that would carry the explored set past
        // `Bounds::states`, and `budget::bounds_of` never sets that bound below the ceiling —
        // so an exhaustion here is recorded on the ledger, where `BudgetLedger::exhaustion`
        // makes it readable, and never raised.
        let charged = budget::charge_states(&mut entry.ledger, campaign.states() as u64);
        debug_assert!(
            charged.is_some_and(|outcome| outcome.is_admitted()),
            "a campaign explored more states than its own bound admitted"
        );
        entry.campaign = Some(campaign);
        entry.continuation = continuation;
        // The checkpoint binds this commit's *count* to the spend that produced it, and the
        // publication carries the *identity*. bn-1gc's `Checkpoint` is the first half and
        // could not be the second: `continuum-task` declares no `continuum-workspace` edge,
        // so nothing there can name an artifact.
        let committed = entry.publications() + 1;
        match entry.ledger.checkpoint(committed) {
            Ok(checkpoint) => {
                let named = entry.evidence.commit(checkpoint, published);
                debug_assert!(
                    named.is_some(),
                    "the publication staged before the park is the one committed here"
                );
            }
            Err(_) => {
                // Unreachable: this daemon holds no reservation at rest and
                // `committed` is monotone by construction. Recorded as a discard rather than
                // raised — an accounting that cannot price a commit does not get to publish
                // it, which is the both-or-neither direction.
                entry.evidence.discard();
            }
        }
        entry.reach(
            if closed {
                MILESTONE_CLOSED
            } else {
                MILESTONE_BOUNDED
            },
            now.as_ref(),
        );
        entry.reach(MILESTONE_CHECKED, now.as_ref());
        entry.advance(
            if closed {
                TaskStatus::Completed
            } else {
                TaskStatus::Suspended
            },
            now.as_ref(),
        );
    }
    if let (Some(record), Some(published)) = (&durable, parked_record) {
        continuation::settle(state.tasks_mut(), record, published);
    }
    if let Some(record) = &closing {
        terminal::settle(state.tasks().get(handle), record);
    }
    // The campaign is on the task now, so a reader can observe it: the commit and the write
    // are one event reported in the order they happened.
    state.regions_mut().step(scope, WorkerStep::Commit);
    state.regions_mut().step(
        scope,
        if closed {
            WorkerStep::Complete
        } else {
            WorkerStep::Suspend
        },
    );
    Ok(())
}

/// Drop the publication this run staged, on a path that will not commit it.
///
/// > While it is open, the artifact is neither committed nor absent — the state G0-DX-14's
/// > second conjunct forbids at rest.
/// >
/// > — `ObligationKind::ProvisionalPublication`
///
/// So every path out of [`run_in`] past the `Reserve` step resolves the staged publication
/// exactly once: this on the two refusals, and `Publications::commit` on the one success.
/// Nothing here reports a failure, because a task the daemon no longer holds has no staged
/// publication to leak.
fn discard(handle: &TaskHandle, state: &mut DaemonState) {
    if let Some(entry) = state.tasks_mut().get_mut(handle) {
        entry.evidence.discard();
    }
}

// ---------------------------------------------------------------------------
// the family
// ---------------------------------------------------------------------------

/// The `verification` namespace's three operations.
#[derive(Debug, Clone, Copy, Default)]
pub struct VerificationFamily;

/// Every `(operation, code)` pair this family can answer with.
pub const FAULTS: &[(&str, ErrorCode)] = &[
    ("verification.start", ErrorCode::CapabilityDenied),
    ("verification.start", ErrorCode::MalformedRequest),
    ("verification.start", ErrorCode::StaleSnapshot),
    ("verification.start", ErrorCode::UnsupportedSemanticFeature),
    ("verification.start", ErrorCode::PublicationAborted),
    ("verification.result", ErrorCode::CapabilityDenied),
    ("verification.result", ErrorCode::BudgetExhausted),
    ("verification.await", ErrorCode::CapabilityDenied),
    ("verification.await", ErrorCode::BudgetExhausted),
];

impl OperationFamily for VerificationFamily {
    fn namespace(&self) -> &'static str {
        "verification"
    }

    fn scope(&self, arguments: &Arguments) -> ScopeClaim {
        // The snapshot and intent a campaign runs over arrive on the *envelope*, which the
        // dispatcher already folds into T2. What the arguments name beyond that is the class
        // set the campaign touches.
        match arguments {
            Arguments::VerificationStart(_) => ScopeClaim {
                snapshots: Vec::new(),
                intents: Vec::new(),
                classes: vec![
                    ArtifactClass::WorkspaceSnapshot.token(),
                    ArtifactClass::ElaboratedModel.token(),
                    ArtifactClass::Task.token(),
                ],
                instances: Vec::new(),
            },
            Arguments::VerificationResult(request) => ScopeClaim {
                snapshots: Vec::new(),
                intents: Vec::new(),
                classes: vec![ArtifactClass::Task.token()],
                instances: vec![request.task.as_str().to_owned()],
            },
            Arguments::VerificationAwait(request) => ScopeClaim {
                snapshots: Vec::new(),
                intents: Vec::new(),
                classes: vec![ArtifactClass::Task.token()],
                instances: vec![request.task.as_str().to_owned()],
            },
            _ => ScopeClaim::default(),
        }
    }

    fn handle(
        &self,
        call: &Call<'_>,
        state: &mut DaemonState,
        services: &Services,
        store: &ReferenceStore,
    ) -> Result<Effect, Fault> {
        match call.arguments {
            Arguments::VerificationStart(request) => start(call, request, state, services, store),
            Arguments::VerificationResult(request) => result(call, &request.task, state, false),
            Arguments::VerificationAwait(request) => result(call, &request.task, state, true),
            // Unreachable: the dispatcher checked shape agreement against the registry
            // before routing.
            _ => Err(Fault::new(
                ErrorCode::MalformedRequest,
                "the request body is not the shape this operation declares",
            )),
        }
    }
}

/// `verification.start` — admit, resolve, construct, run.
///
/// > Start a verification campaign against a target under an intent and a budget. Returns a
/// > task handle, or a cached result when one exists for the same snapshot, intent, target,
/// > and epochs.
/// >
/// > — the IDL's own summary of this operation
///
/// The second clause is not a cache bolted on: a `task_*` handle *is* the content identity of
/// that tuple (see [`task`]'s "Identity" section), so "one exists for the same snapshot,
/// intent, target, and epochs" is a lookup by handle. Two consequences a caller can rely on:
///
/// - **an identical replay returns the identical task identity**, before the idempotency
///   ledger is consulted at all — which is the PR 5 exit, held by the identity function
///   rather than by the ledger;
/// - **a second start under a *different* idempotency key returns the cached result**, with
///   `task` absent and `result` present, exactly as the IDL's `optional`/`optional` pair
///   describes. No second campaign runs, and no second task identity exists to diverge.
///
/// A task that ran and *failed* is a task, not an error: a state budget too small to hold the
/// model's initial states leaves `status = failed`, `failed_reason = BudgetExhausted` and the
/// `non_resumable_reason` RFC 0026 requires beside it, all readable through `task.status`.
/// Returning the fault from `start` instead would have nowhere to put the reason, because
/// this layer's failure envelope carries no `non_resumable_reason`.
fn start(
    call: &Call<'_>,
    request: &VerificationStartRequest,
    state: &mut DaemonState,
    services: &Services,
    store: &ReferenceStore,
) -> Result<Effect, Fault> {
    // The campaign record is published under the caller's own capability, so the store
    // decides and audits the write against the identity the wire presented — "authorization
    // separate from handle possession" (ADR-0037), the same rule `observe.ingest` publishes
    // a trace under.
    let publisher = super::identity::capability_to_store(&call.envelope.capability)
        .map_err(|_| Fault::denied())?;
    let Nullable::Value(snapshot) = &call.envelope.snapshot else {
        return Err(Fault::new(
            ErrorCode::MalformedRequest,
            "a verification campaign requires the envelope's `snapshot` to name the sealed \
             workspace it runs over",
        ));
    };
    if request.portfolio == Portfolio::Custom {
        return Err(Fault::new(
            ErrorCode::UnsupportedSemanticFeature,
            "a custom portfolio has no definition on the wire for this daemon to read",
        ));
    }
    let budget = call.envelope.budget.value().cloned().ok_or_else(|| {
        Fault::new(
            ErrorCode::MalformedRequest,
            "a @task_starting operation requires a `budget`",
        )
    })?;

    // The sealed snapshot. `StaleSnapshot` is "the named snapshot is not current, **or is not
    // sealed where sealing is required**" — a campaign is required over a sealed snapshot,
    // because a result pinned to a mutable tree pins nothing.
    let record = state.workspace(snapshot).ok_or_else(Fault::denied)?;
    // The snapshot's intent is not named by the request; the task records it and
    // `task.status` reports it. It is decided by the grant before anything else is read
    // about it (`rule capability.instance_scope`, the derived-handle clause; cr-3hcpn4).
    call.derived(Derived::Intent(&record.intent))?;
    if !record.sealed() {
        return Err(Fault::new(
            ErrorCode::StaleSnapshot,
            "a verification campaign runs over a sealed snapshot",
        )
        .with_recovery(super::workspace::reseal_current_head(
            state,
            &record.lineage,
        )));
    }
    let intent = record.intent.clone();
    let lineage_name = record.lineage.clone();
    let head = record.descriptor.source().identity().clone();
    let modules = snapshot_modules(record.descriptor.source());
    let lineage = state.lineage(&lineage_name).ok_or_else(Fault::denied)?;
    // Both `LineageError` arms collapse to `StaleSnapshot` here, deliberately including
    // `Unknown`: `record` above is already a snapshot this daemon fully resolved, so there is
    // no existence-oracle question left to protect, only a currency one — RFC 0026's "An
    // unplaceable lineage identity" disposition (bn-10wdo), which also documents why
    // `daemon::workspace`'s guarded writes answer the same `Unknown` arm `CapabilityDenied`.
    //
    // A `Stale` refusal names the head that superseded the snapshot, as a recovery offer
    // (RFC 0027 H8, bn-27mx7); an `Unknown` one has no head to name.
    check_current(lineage, &head).map_err(|error| match error {
        LineageError::Stale(stale) => {
            super::workspace::superseded(state, &lineage_name, stale.current())
        }
        LineageError::Unknown(_) => Fault::new(
            ErrorCode::StaleSnapshot,
            "the named snapshot has been superseded in its lineage",
        ),
    })?;

    let source = model_source(
        services.identifier(),
        modules
            .iter()
            .map(|(path, content)| (path.as_str(), content.as_slice())),
    )
    .ok_or_else(no_model)?;
    if state.models().get(&source).is_none() {
        return Err(no_model());
    }

    let priority_class = request
        .priority_class
        .value()
        .copied()
        .unwrap_or(PriorityClass::Interactive);
    let handle = start_handle(
        services,
        snapshot,
        &intent,
        &request.target,
        request.portfolio,
        priority_class,
        &budget,
    )?;
    // The task identity is a function of the request, so it is decided here, before any
    // lookup: an existing task under it is reported, and a new one is created under it. A
    // grant that instance-scopes `task` and does not list this identity is refused either
    // way, so the answer does not say whether the identity already exists (X2; cr-3hcpn4).
    call.derived(Derived::Instance(handle.as_str()))?;

    // A task the startup resolution pass resolved (plan §4.5 O2, bn-1z09m). One handle has
    // exactly one state, so this is decided before the live table is consulted:
    //
    // - `Failed(_)` is terminal. RFC 0026 "Task lifecycle": "a terminal status never
    //   changes", and a terminal non-`Completed` identity is answered on the observing lane
    //   at `ok`, running nothing (F20, paid at 3.4). So a re-issued start is that answer, and
    //   no live entry is created beside the resolution.
    // - `Settled` is a task that reached `Completed` before the crash in a store written
    //   before terminal records existed (bn-2g3ei), so nothing about it is durable but its
    //   campaign records: no status, milestone or ledger was ever answered after the
    //   restart, and a re-run contradicts nothing a client read. The cached-result lane has
    //   no result to hand back, and re-running a completed identity "could only produce the
    //   same answer" (the `Completed` arm below). So the resolution is superseded explicitly
    //   and the campaign runs again under the same identity. It republishes the same
    //   campaign records, publishes the terminal record the old store lacked, and reaches
    //   `Completed` again.
    //
    // A task whose terminal record is durable is not here: the startup pass restored it as a
    // live terminal entry (bn-2g3ei), and the live lookup below answers it without a re-run.
    // RFC 0026 "Task lifecycle" is why a re-run is not available for it: "reported
    // milestones and `committed_evidence` only grow, and a terminal status never changes",
    // and a re-run under a fresh entry would re-time the milestones `task.status` already
    // answered. The `Completed` arm answers from the restored entry, re-deriving only the
    // report ([`rederived`]).
    if let Some(resolved) = state.tasks().resolution(&handle) {
        match resolved.resolution {
            Resolution::Failed(_) => return resolved_terminal(resolved),
            Resolution::Settled => {
                state.tasks_mut().supersede(&handle);
            }
            // A restored or terminal task is loaded into the live table, not the resolved set
            // (bn-20142, bn-2g3ei), and the live lookup below answers it. Startup never files
            // one here: a restore that fails is filed as `Failed(Unreceipted)` (bn-283p6).
            // If one is here with no live task anyway, it is answered as resolved and never
            // falls through to a fresh run under the same identity.
            Resolution::Restored(_) | Resolution::Terminal(_) => {
                if state.tasks().get(&handle).is_none() {
                    return resolved_terminal(resolved);
                }
            }
        }
    }

    // The terminal short-circuit, ahead of the cached/started split (RFC 0026 F20, paid
    // at protocol 3.4 by bn-3jrtz). `task.resume` answers "this identity is already
    // terminal" on the task-observing lane at `status = ok`, and until 3.4 this operation
    // answered the identical fact `task_started` — one fact, spelled two ways by the two
    // operations that can reach it (bn-3p32's A12; bn-y9f7i's disposition, which ratified
    // the old lane as no violation and raised F20 for exactly this branch). A terminal
    // identity is now one of two `ok` answers:
    if let Some(entry) = state.tasks().get(&handle) {
        super::task::task_scope(call, entry)?;
        if entry.is_terminal() {
            // A completed task under this identity is the same campaign, and re-running
            // it could only produce the same answer more slowly: the cached-result lane,
            // `ok` with `result` present. The envelope still names the task the result
            // is about, which is the second clause of the `task` presence rule.
            if entry.status == TaskStatus::Completed {
                let (payload, verdict, omissions) = cached(entry, state.models())?;
                let mut effect = Effect::new(payload, verdict.0)
                    .observing(entry.handle.clone())
                    .with_artifacts(published(entry));
                effect.assurance = verdict.1;
                effect.omissions = omissions;
                return Ok(effect);
            }
            // A `Cancelled` or `Failed` task produced no output for the result branch to
            // reuse (RFC 0030 reuse classes), and it is not being started either — so the
            // answer mirrors `task.resume`'s terminal short-circuit: `ok` on the
            // task-observing lane, `task` present, `result` absent. Nothing runs, nothing
            // is published, and `task.status` remains the authoritative reading of what
            // this task will (never) do next.
            return Ok(terminal(entry));
        }
        // A live identity — `Created`, `Running`, or `Suspended` — takes the task lane.
        return Ok(started(entry));
    }

    let entry = TaskEntry {
        handle: handle.clone(),
        operation: call.envelope.operation.clone(),
        status: TaskStatus::Created,
        snapshot: Nullable::Value(snapshot.clone()),
        intent: Nullable::Value(intent),
        target: request.target.clone(),
        portfolio: request.portfolio,
        priority_class,
        ledger: budget::ledger_of(&budget),
        epochs: services.epochs().clone(),
        model: source,
        milestones: Vec::new(),
        committed_evidence: Vec::new(),
        events: Vec::new(),
        continuation: None,
        failed_reason: None,
        non_resumable_reason: None,
        campaign: None,
        // Nothing has run yet, so this task has been in no region and published nothing.
        // `advance` writes all three, inside the scope it opens.
        region: None,
        worker: None,
        evidence: Publications::new(),
    };
    let bounds = entry.bounds();
    // The same run, refused before for this capability on the continuation it would mint,
    // is refused again before any exploration (cr-3hcpn4).
    let refused_key = RefusedRun {
        capability: call.envelope.capability.clone(),
        task: handle.clone(),
        from: None,
        bounds,
    };
    if state.tasks().refused(&refused_key) {
        let mut denied = Fault::denied();
        denied.derived_denial = true;
        return Err(denied);
    }
    state.tasks_mut().put(entry);
    // A fresh task has no parked continuation, so there is no prior frontier to rediscover.
    let mut explored = None;
    let mut authorize = |minted: &ContinuationHandle, states: u64| {
        explored = Some(states);
        call.admits(Derived::Instance(minted.as_str()))
    };
    let ran = advance(
        &handle,
        bounds,
        &[],
        state,
        services,
        store,
        &publisher,
        &mut authorize,
    );
    if let Err(fault) = ran {
        // A run refused on its minted continuation leaves no task: the entry this call
        // created is removed, and the work is charged to the presenting capability.
        if fault.derived_denial {
            state.tasks_mut().remove(&handle);
            state
                .tasks_mut()
                .refuse_run(refused_key, explored.unwrap_or_default());
        }
        return Err(fault);
    }

    let entry = state.tasks().get(&handle).ok_or_else(Fault::denied)?;
    Ok(started(entry))
}

/// The `verification.start` answer that names a task that can still run.
///
/// The envelope's status lane is read off the task rather than fixed: a campaign that
/// parked reports `task_suspended` and names the continuation that resumes it, and every
/// other *live* outcome reports `task_started`, which is what `ResultStatus::task_started`
/// says — "a long operation was started; `task` is present". Both lanes are open to this
/// operation because it is `@task_starting`, and until bn-i4aem item 9 neither was
/// reachable: `result::success` hard-coded `ok` with both handles absent, so a parked
/// campaign's continuation was reachable only by a second `task.status` call.
///
/// As of protocol 3.4 this lane is reached by a live identity — `Created`, `Running`,
/// `Suspended` — and by a task this very call created, whatever status it reached before
/// the answer was written (a fresh campaign that runs to completion inside the call still
/// answers `task_started`: the work genuinely began here, and the result is the *next*
/// call's cached answer). What no longer reaches it is an identity that was terminal
/// before the call: a `Failed` or `Cancelled` identity used to fall through here too,
/// answered `task_started` for a call that started nothing. RFC 0026's "What
/// `task_started` means when an identity resolves to a task that will not run again"
/// ratified that as no violation (bn-3p32's A12, bn-y9f7i) while recording the asymmetry
/// with `task.resume`'s terminal short-circuit as F20, and the 3.4 bundle (bn-3jrtz)
/// paid it: [`terminal`] answers those identities at `status = ok` on the observing lane.
fn started(entry: &TaskEntry) -> Effect {
    let mut effect = Effect::new(
        Payload::VerificationStart(VerificationStartResponse {
            task: Optional::Present(entry.handle.clone()),
            result: Optional::Absent,
        }),
        Nullable::Null,
    )
    .with_artifacts(published(entry));
    // The declared ceilings this daemon cannot enforce, *derived* from the task's own ledger
    // (bn-23j7s). It was `unenforced(budget)`, an eight-name array walked against the
    // request's budget; the subjects, the reason token and their order are unchanged,
    // because `DimensionOmission` emits the spelling that array emitted and
    // `CostDimension::ALL` is the IDL's declaration order.
    effect.omissions = [entry.omissions(), budget::omissions_of(&entry.ledger)].concat();
    match (entry.status, &entry.continuation) {
        (TaskStatus::Suspended, Some(continuation)) => {
            effect.suspended(entry.handle.clone(), continuation.clone())
        }
        _ => effect.started(entry.handle.clone()),
    }
}

/// The `verification.start` answer for an identity that resolves to a terminal,
/// non-`Completed` task: `status = ok` on the task-observing lane, `task` present,
/// `result` absent (RFC 0026 F20, protocol 3.4, bn-3jrtz).
///
/// The mirror of `task.resume`'s terminal short-circuit (`daemon::task::resume`,
/// bn-10093), which answers the identical "this identity names a task that will never run
/// again" fact the same way: `ok` because a terminal task is not being started and this
/// call ran nothing, the observing lane because the answer is *about* the task the
/// identity resolves to, and no `result` because a `Cancelled` or non-resumable `Failed`
/// campaign produced no output the result branch is licensed to reuse (RFC 0030 reuse
/// classes). The payload keeps the response's task-carrying spelling — `task` present,
/// `result` absent — because the operation's two-optional response is the declared
/// surface and the envelope's `ok` is what distinguishes this from a start
/// (`AnswerShape` in `continuum-cli` derives its token from the body, deliberately not
/// from the status, and both readings stay coherent here).
///
/// Omissions and artifacts are [`started`]'s: the record's own manifest plus the ledger's
/// unenforced-ceiling omissions, and the artifacts the task already published — this
/// answer names held state, exactly as `started` does for a live suspended identity.
fn terminal(entry: &TaskEntry) -> Effect {
    let mut effect = Effect::new(
        Payload::VerificationStart(VerificationStartResponse {
            task: Optional::Present(entry.handle.clone()),
            result: Optional::Absent,
        }),
        Nullable::Null,
    )
    .observing(entry.handle.clone())
    .with_artifacts(published(entry));
    effect.omissions = [entry.omissions(), budget::omissions_of(&entry.ledger)].concat();
    effect
}

/// The `verification.start` answer for an identity the startup resolution pass resolved to
/// `Failed` (bn-1z09m): [`terminal`]'s lane, built from the resolution.
///
/// `ok` on the task-observing lane, `task` present, `result` absent, nothing run. The
/// artifacts are the records the task committed before the crash, which the resolution
/// claims. The task record itself was not durable, and the omission says so.
fn resolved_terminal(resolved: &ResolvedTask) -> Result<Effect, Fault> {
    // Every record is receipt-tied by `ResolvedTask`'s type (bn-283p6), so the response
    // names no identity the ledger holds no receipt for.
    let artifacts = resolved
        .records
        .iter()
        .map(Published::handle)
        .map(|record| {
            Ok(ArtifactRef {
                kind: ArtifactClass::Task.token().to_owned(),
                handle: WireArtifactHandle::new(resolved.task.as_str()).map_err(|_| {
                    Fault::new(
                        ErrorCode::PublicationAborted,
                        "the task identity is not a well-formed artifact handle",
                    )
                })?,
                commitment: Optional::Present(Commitment::new(&record.identity.to_string())),
                redacted: Optional::Absent,
            })
        })
        .collect::<Result<Vec<_>, Fault>>()?;
    let mut effect = Effect::new(
        Payload::VerificationStart(VerificationStartResponse {
            task: Optional::Present(resolved.task.clone()),
            result: Optional::Absent,
        }),
        Nullable::Null,
    )
    .observing(resolved.task.clone())
    .with_artifacts(artifacts);
    effect.omissions = vec![unsupported("task.record")];
    Ok(effect)
}

/// The `verification.start` answer that carries a cached result instead of a task.
type Cached = (
    Payload,
    (Nullable<Verdict>, Optional<AssuranceEnvelope>),
    Vec<Omission>,
);

fn cached(entry: &TaskEntry, models: &ModelCatalog) -> Result<Cached, Fault> {
    let campaign = campaign_of(entry, models)?;
    let (result, _) = verification_result(entry, campaign.as_deref())?;
    Ok((
        Payload::VerificationStart(VerificationStartResponse {
            task: Optional::Absent,
            result: Optional::Present(result),
        }),
        // `verification.start` declares no `verdict` clause, so its result carries none: the
        // verdict rides `verification.result`, whose clause is `SemanticVerdictValue`.
        (Nullable::Null, Optional::Absent),
        [entry.omissions(), coverage()].concat(),
    ))
}

/// `verification.result` and `verification.await`.
///
/// > Block, within the request budget, on a started verification task and return its result.
/// > […] waiting never changes a verdict.
/// >
/// > — the IDL, on `verification.await`
///
/// The two are one function here, and `awaiting` changes nothing about the answer — which is
/// the honest reading rather than a shortcut. A task at this grain is already terminal or
/// already parked by the time any operation can look at it (see [`task`]'s opening section),
/// so there is no interval for a timeout to elapse over and `timeout_ms` has nothing to bound.
/// The IDL's own sentence is what makes that correct: waiting never changes a verdict, so a
/// wait of zero returns the verdict a wait of any length would.
///
/// # Errors
///
/// [`ErrorCode::CapabilityDenied`] for a task this daemon does not hold — X2 again — and
/// [`ErrorCode::BudgetExhausted`] for a task whose budget could not hold the model's initial
/// states, which is the one outcome with no result to report and no continuation to resume.
fn result(
    call: &Call<'_>,
    handle: &TaskHandle,
    state: &DaemonState,
    awaiting: bool,
) -> Result<Effect, Fault> {
    let entry = state.tasks().get(handle).ok_or_else(Fault::denied)?;
    super::task::task_scope(call, entry)?;
    let campaign = campaign_of(entry, state.models())?;
    let (result, verdict) = verification_result(entry, campaign.as_deref())?;
    let payload = if awaiting {
        Payload::VerificationAwait(result)
    } else {
        Payload::VerificationResult(result)
    };
    let mut effect = Effect::new(payload, Nullable::Value(Verdict::Semantic(verdict)))
        .observing(entry.handle.clone());
    effect.assurance = Optional::Present(assurance(
        campaign.as_deref(),
        state.models().get(&entry.model),
    ));
    effect.omissions = [entry.omissions(), coverage()].concat();
    // `verification.await` is `@task_starting` and `verification.result` is not, so only
    // the first may report a parked task on the `task_suspended` lane. The asymmetry is
    // the IDL's annotation, not a preference: `result` is a read of a task's answer, and
    // `await` is the operation that waits for one, so it is the one that can say "still
    // parked, here is the continuation".
    if awaiting {
        if let (TaskStatus::Suspended, Some(continuation)) = (entry.status, &entry.continuation) {
            return Ok(effect.suspended(entry.handle.clone(), continuation.clone()));
        }
    }
    Ok(effect)
}

/// The engine report of a `Completed` task restored from its terminal record (bn-2g3ei),
/// re-derived from durable inputs.
///
/// The report is not in the terminal record: it is the engine's own value, and the terminal
/// record carries what `task.status` projects. What the record does carry is every input
/// the closing run was a function of — the model source by content identity, the target,
/// and the ceilings the bounds are read from — and the reference engine is deterministic.
/// So the same run over the same model under the same bounds is the same report, and this
/// function computes it. It is a pure computation, not a run of the task: no region is
/// opened, nothing is published, and the task's status, milestones, ledger and publications
/// do not change. That is what separates it from the supersede-and-rerun a `Settled`
/// resolution takes (`start` below), which RFC 0026's monotonic `task.status` excludes for
/// a task whose record is durable: a re-run re-times its milestones.
///
/// The result is checked against the durable record rather than trusted: the exploration
/// must close, and over exactly the state count the ledger recorded.
///
/// # Errors
///
/// [`ErrorCode::UnsupportedSemanticFeature`] when the model the task names is not in the
/// catalog — `verification.start`'s code for the same condition, and the model catalog is
/// reprovisioned after a restart — or when the re-derived run does not reproduce the
/// durable record. The second is unreachable while the engine is deterministic.
fn rederived(entry: &TaskEntry, models: &ModelCatalog) -> Result<Campaign, Fault> {
    let model = models.get(&entry.model).ok_or_else(no_model)?;
    let campaign = run(model, &entry.target, entry.bounds(), &[])?.map_err(|_| unreproduced())?;
    let recorded = entry.ledger.spend().measured(CostDimension::States);
    if !campaign.is_closed() || recorded != Some(campaign.states() as u64) {
        return Err(unreproduced());
    }
    Ok(campaign)
}

fn unreproduced() -> Fault {
    Fault::new(
        ErrorCode::UnsupportedSemanticFeature,
        "the completed task's check report could not be re-derived from its durable record",
    )
}

/// The campaign a result is read from: the task's own, or — for a `Completed` task restored
/// from its terminal record (bn-2g3ei) — the one [`rederived`] computes. [`None`] when the
/// task holds none and none can be derived.
///
/// # Errors
///
/// As [`rederived`].
fn campaign_of<'a>(
    entry: &'a TaskEntry,
    models: &ModelCatalog,
) -> Result<Option<Cow<'a, Campaign>>, Fault> {
    Ok(match &entry.campaign {
        Some(campaign) => Some(Cow::Borrowed(campaign)),
        None if entry.status == TaskStatus::Completed => {
            Some(Cow::Owned(rederived(entry, models)?))
        }
        None => None,
    })
}

/// The typed result of one task, and the verdict it carries.
fn verification_result(
    entry: &TaskEntry,
    campaign: Option<&Campaign>,
) -> Result<(VerificationResult, SemanticVerdictValue), Fault> {
    let Some(campaign) = campaign else {
        // A task restored from its continuation record (bn-20142) committed publications,
        // and the engine's report of its last run was not durable. The honest answer is that
        // this task is parked on its budget and its result is re-derived by resuming it: a
        // `BudgetExhausted` resumable from the continuation, never the "initial states"
        // reason below, which would be false.
        if let (Some(continuation), true) = (&entry.continuation, entry.publications() > 0) {
            return Err(Fault::exhausted_from(
                "this task's check report did not survive a daemon restart; resume its \
                 continuation to re-derive it",
                continuation.clone(),
            ));
        }
        // SD-13: never a silent dead end. This task holds no continuation — initial states
        // are what exploration starts *from*, so there is no prefix of the walk to resume —
        // and the typed reason says exactly that rather than leaving a caller to infer it
        // from an absent field.
        return Err(Fault::exhausted(
            "the declared state budget cannot hold the model's own initial states",
        ));
    };
    let closed = campaign.is_closed();
    let verdict = SemanticVerdictValue {
        verdict: match campaign.report.verdict() {
            EngineVerdict::Established => SemanticVerdict::Established,
            EngineVerdict::Refuted => SemanticVerdict::Refuted,
            EngineVerdict::Inconclusive => SemanticVerdict::Inconclusive,
        },
        // INV-008: an inconclusive claim is never silent, and the reason is the engine's own
        // rather than one inferred from the scope. Budget exhaustion reaches the wire here,
        // as a *reason for not deciding* — never as a verdict (docs/49).
        inconclusive_reason: match campaign.report.verdict() {
            EngineVerdict::Inconclusive => Optional::Present(match campaign.unresolved() {
                Some(checking::Unresolved::ResourceExhausted { .. }) | None => {
                    InconclusiveReason::ResourceExhausted
                }
                Some(checking::Unresolved::EngineError { .. }) => InconclusiveReason::EngineError,
            }),
            EngineVerdict::Established | EngineVerdict::Refuted => Optional::Absent,
        },
        // docs/03 §3: a closed exploration is `EXHAUSTIVE_FINITE` — every reachable state was
        // examined by a machine — and a bounded one is `BOUNDED_STATES`. Neither is `proved`:
        // that rung needs a certificate an independent checker accepted, and this daemon
        // emits none.
        assurance_class: if closed {
            AssuranceClass::Validated
        } else {
            AssuranceClass::Bounded
        },
    };
    Ok((
        VerificationResult {
            task: entry.handle.clone(),
            target: entry.target.clone(),
            // The campaign covers the Finite fragment and claims nothing about the other
            // five; `coverage` names each of them in the omission manifest, because "a
            // fragment the campaign did not cover is reported in `omissions`, never implied".
            fragments: vec![Fragment::Finite],
            evidence: Vec::new(),
            crashpack: Optional::Absent,
            context: Optional::Absent,
            continuation: match &entry.continuation {
                Some(handle) => Optional::Present(handle.clone()),
                None => Optional::Absent,
            },
        },
        verdict,
    ))
}

/// The fragments a finite reachability campaign does not cover, and the artifacts this
/// daemon cannot build (INV-007).
fn coverage() -> Vec<Omission> {
    let mut omissions: Vec<Omission> = [
        Fragment::Symbolic,
        Fragment::Temporal,
        Fragment::Probabilistic,
        Fragment::Theorem,
        Fragment::Runtime,
    ]
    .into_iter()
    .map(|fragment| unsupported(fragment.as_wire()))
    .collect();
    // A refutation's counterexample reaches a client as a `crash_*` crashpack, and a Context
    // Pack as a `ctx_*`; neither is an artifact this daemon builds, so both are named rather
    // than left as absent fields a caller has to interpret.
    omissions.push(unsupported("crashpack"));
    omissions.push(unsupported("context"));
    omissions
}

/// The nine-dimension assurance envelope for one task.
///
/// > A result whose `verdict` is `semantic` or `evaluation` MUST carry the nine-dimension
/// > `assurance` envelope, and every dimension MUST name a producing engine or carry a typed
/// > `Unsupported(reason)` (plan B11). Hiding uncertainty to save tokens is prohibited.
/// >
/// > — `rule envelope.assurance_required`
///
/// Three dimensions name [`ENGINE`] because it genuinely produced them; six carry a typed
/// reason because this engine declares no fault model, no concurrency, no memory model, no
/// observer and no certificate, and — see [`fairness_dimension`] — either declares no
/// fairness assumption or declares one this campaign does not consume. Every summary is a
/// stable machine token — docs/03 §3's own scope names, and the IDL's own
/// `sequential-consistency-only` example — and none of them interpolates anything
/// (`rule envelope.no_prose`).
fn assurance(campaign: Option<&Campaign>, model: Option<&Model>) -> AssuranceEnvelope {
    let scope = campaign.map_or("no-exploration", |campaign| match campaign.report.scope() {
        Scope::Complete { .. } => "exhaustive-finite",
        Scope::Bounded { .. } => "bounded-states",
    });
    AssuranceEnvelope {
        bounds: produced(scope),
        faults: unsupported_dimension("no-fault-model"),
        fairness: fairness_dimension(model),
        values: produced("finite-declared-domains"),
        schedules: unsupported_dimension("no-concurrency-model"),
        memory_model: unsupported_dimension("sequential-consistency-only"),
        observer: unsupported_dimension("no-observer-model"),
        proof_status: unsupported_dimension("no-certificate-emitted"),
        unknowns: produced(if campaign.is_some_and(Campaign::is_closed) {
            "none"
        } else {
            "bounded-frontier"
        }),
    }
}

/// The `fairness` dimension of the nine-dimension envelope, honest about what
/// [`Model::fairness`] declares and about what this daemon's finite reachability engine
/// actually consumes (bn-rkrt7, following bn-1ln12).
///
/// This binding's `obligations` builds invariant obligations only — `Fragment::Temporal` is
/// unsupported (see [`coverage`]), so no run through [`run`] ever calls
/// `continuum_engine_reference::liveness`, the one consumer [`Model::fairness`] has
/// (`continuum-model-core/src/model.rs`: "consumed by the reference engine's liveness
/// check … A safety check … [ignores] them, as [it] must: fairness constrains infinite
/// executions only"). So this dimension is never `produced` here — claiming a liveness
/// result under a fairness this engine did not use is exactly what it must never do — but
/// "the model has none" and "the model has one this check does not reach" are different
/// facts, and INV-003/`rule envelope.no_prose` forbid collapsing them into one token that is
/// false half the time:
///
/// - no model, or a model with an empty [`Model::fairness`]: `no-fairness-model`, unchanged
///   from before this bone.
/// - a model whose [`Model::fairness`] is non-empty: `fairness-declared-unused`, naming the
///   real fact — the model carries a fairness assumption — without interpolating its
///   strength or scope (`rule envelope.no_prose` keeps a summary a stable token, not a
///   rendering of the model's own declarations) and without claiming this campaign used it.
fn fairness_dimension(model: Option<&Model>) -> EnvelopeDimension {
    match model {
        Some(model) if !model.fairness().is_empty() => {
            unsupported_dimension("fairness-declared-unused")
        }
        Some(_) | None => unsupported_dimension("no-fairness-model"),
    }
}

fn produced(summary: &str) -> EnvelopeDimension {
    EnvelopeDimension::Produced(ProducedDimension {
        engine: ENGINE.to_owned(),
        summary: summary.to_owned(),
    })
}

fn unsupported_dimension(reason: &str) -> EnvelopeDimension {
    EnvelopeDimension::Unsupported(UnsupportedDimension {
        reason: reason.to_owned(),
    })
}

/// The content identity of a task: what it *is*, and nothing about when it was asked for.
#[allow(clippy::too_many_arguments)]
fn start_handle(
    services: &Services,
    snapshot: &crate::protocol::scalar::WorkspaceHandle,
    intent: &crate::protocol::scalar::IntentHandle,
    target: &Target,
    portfolio: Portfolio,
    priority_class: PriorityClass,
    budget: &Budget,
) -> Result<TaskHandle, Fault> {
    let mut preimage = Preimage::new();
    preimage.text("verification.start");
    preimage.text(snapshot.as_str());
    preimage.text(intent.as_str());
    preimage.text(target.kind.as_wire());
    preimage.text(&target.id);
    preimage.text(portfolio.as_wire());
    preimage.text(priority_class.as_wire());
    budget_preimage(&mut preimage, budget);
    epochs_preimage(&mut preimage, services.epochs());
    task_handle(services.identifier(), &preimage)
}

/// Why this daemon holds no model for a snapshot a certificate names (bn-3hk4v).
///
/// Four absences, kept apart so a refusal can say which one it was. Each of them is a
/// reason the daemon cannot bind a claim to a model, and none of them is a verdict about
/// the certificate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum NoHeldModel {
    /// No workspace record is filed under the handle.
    UnknownSnapshot,
    /// The workspace is not sealed, so its content, and with it its model, is not fixed.
    Unsealed,
    /// The snapshot carries no `.ctm` module, so it names no model source at all.
    NoModules,
    /// The snapshot's modules are not a model this deployment registered.
    Unregistered,
}

/// The model this daemon holds for `snapshot`, by the resolution `verification.start` uses.
///
/// The same three steps and in the same order: the sealed workspace record, the content
/// identity of its `.ctm` modules ([`model_source`]), and the catalog entry under that
/// identity. So a certificate bound here is bound to the model a campaign over the same
/// snapshot would explore, and not to a second reading of the snapshot.
///
/// Sealing is required for the reason `verification.start` requires it: a claim bound to
/// a mutable tree is bound to nothing. Currency is not required. A snapshot a later head
/// superseded still has exactly the model it had, and a claim about it stays a claim about
/// it.
///
/// The caller decides the snapshot against its grant *before* calling this
/// (`rule capability.instance_scope`, the derived-handle clause).
pub(super) fn held_model<'state>(
    state: &'state DaemonState,
    services: &Services,
    snapshot: &crate::protocol::scalar::WorkspaceHandle,
) -> Result<&'state Model, NoHeldModel> {
    let record = state
        .workspace(snapshot)
        .ok_or(NoHeldModel::UnknownSnapshot)?;
    if !record.sealed() {
        return Err(NoHeldModel::Unsealed);
    }
    let modules = snapshot_modules(record.descriptor.source());
    let source = model_source(
        services.identifier(),
        modules
            .iter()
            .map(|(path, content)| (path.as_str(), content.as_slice())),
    )
    .ok_or(NoHeldModel::NoModules)?;
    state.models().get(&source).ok_or(NoHeldModel::Unregistered)
}

pub(super) fn no_model() -> Fault {
    Fault::new(
        ErrorCode::UnsupportedSemanticFeature,
        "this daemon compiles no CML source; it verifies the models a deployment registered, \
         and the named snapshot's modules are not one of them",
    )
}

#[cfg(test)]
mod tests {
    use continuum_engine_reference::model::{ActionDecl, ModelBuilder};
    use continuum_engine_reference::{BoolExpr, IntExpr, Strength};

    use super::{EnvelopeDimension, fairness_dimension};

    /// A minimal model with no declared actions worth fairness over, and no fairness
    /// declaration — [`ModelBuilder::fairness`] untouched.
    fn plain_model() -> continuum_engine_reference::model::Model {
        ModelBuilder::new()
            .variable("x", 0, 1)
            .initial_state(&[("x", 0)])
            .action(ActionDecl::deterministic(
                "A",
                BoolExpr::Const(true),
                vec![("x", IntExpr::constant(1))],
            ))
            .build()
            .expect("a two-state model with one action builds")
    }

    /// The same model, plus one weak fairness assumption over `A` (bn-1ln12).
    fn fair_model() -> continuum_engine_reference::model::Model {
        ModelBuilder::new()
            .variable("x", 0, 1)
            .initial_state(&[("x", 0)])
            .action(ActionDecl::deterministic(
                "A",
                BoolExpr::Const(true),
                vec![("x", IntExpr::constant(1))],
            ))
            .fairness(Strength::Weak, ["A"])
            .build()
            .expect("the same model plus one fairness assumption builds")
    }

    fn reason(dimension: &EnvelopeDimension) -> &str {
        match dimension {
            EnvelopeDimension::Unsupported(unsupported) => unsupported.reason.as_str(),
            EnvelopeDimension::Produced(_) => {
                panic!("this daemon runs no liveness check, so `fairness` is never `produced`")
            }
        }
    }

    /// No `Model` at all — a cached or re-derived result over a source this catalog no
    /// longer holds — reads exactly as a model with no fairness does.
    #[test]
    fn no_model_reports_no_fairness_model() {
        assert_eq!(reason(&fairness_dimension(None)), "no-fairness-model");
    }

    /// A model without fairness reports `no-fairness-model`, unchanged (bn-rkrt7).
    #[test]
    fn a_model_without_fairness_reports_no_fairness_model() {
        let model = plain_model();
        assert_eq!(
            reason(&fairness_dimension(Some(&model))),
            "no-fairness-model"
        );
    }

    /// A model with a declared fairness assumption reports that it has one, and the token
    /// is not the one a fairness-free model reports (bn-rkrt7, following bn-1ln12).
    #[test]
    fn a_model_with_fairness_reports_it() {
        let model = fair_model();
        let dimension = fairness_dimension(Some(&model));
        assert_eq!(reason(&dimension), "fairness-declared-unused");
        assert_ne!(reason(&dimension), "no-fairness-model");
    }

    /// This daemon's finite reachability engine never runs a liveness check, so `fairness`
    /// is `unsupported` for every model this catalog can hold — declared or not — never
    /// `produced`: a `produced` fairness dimension would claim a liveness verdict was
    /// established under it, which this engine binding never does.
    #[test]
    fn fairness_is_never_produced_regardless_of_the_model() {
        for dimension in [
            fairness_dimension(None),
            fairness_dimension(Some(&plain_model())),
            fairness_dimension(Some(&fair_model())),
        ] {
            assert!(
                matches!(dimension, EnvelopeDimension::Unsupported(_)),
                "a liveness result must never overstate the fairness the engine used"
            );
        }
    }
}
