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

use std::collections::BTreeMap;

use continuum_engine_reference::bfs::{self, Bounds, ExplorationError, Partial};
use continuum_engine_reference::checking::{
    self, DeadlockPolicy, Obligations, Scope, Verdict as EngineVerdict,
};
use continuum_engine_reference::model::Model;
use continuum_task::region::worker::WorkerStep;
use continuum_workspace::artifact_path::ArtifactClass;
use continuum_workspace::publication::{ContentIdentifier, ReferenceStore};
use continuum_workspace::snapshot::Snapshot;
use continuum_workspace::staleness::check_current;

use super::Services;
use super::family::{Arguments, Call, Effect, Fault, OperationFamily, Payload, ScopeClaim};
// `Scope` is `continuum_engine_reference::checking::Scope` in this module — the exploration's
// completeness — so the region scope is imported under a name that says which of the two it
// is rather than shadowing the engine's vocabulary.
use super::region::{self, Scope as RegionScope};
use super::state::DaemonState;
use super::task::{
    Campaign, Continuation, PinnedEpochs, Preimage, TaskEntry, budget_preimage,
    continuation_handle, epochs_preimage, task_handle, unsupported,
};
use crate::protocol::envelope::{
    AssuranceEnvelope, Budget, EnvelopeDimension, Omission, ProducedDimension,
    SemanticVerdictValue, UnsupportedDimension, Verdict,
};
use crate::protocol::operations::verification::{
    VerificationStartRequest, VerificationStartResponse,
};
use crate::protocol::scalar::{Commitment, TaskHandle};
use crate::protocol::shared::{Target, VerificationResult};
use crate::protocol::spec::{Nullable, Optional, ProtocolEnum};
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

/// The engine bounds a wire budget declares.
///
/// `states` is the one dimension with an enforcement path; depth and transitions have no
/// wire dimension and take the kernel wire form's own ceilings through `Bounds::CERTIFIABLE`.
/// A budget declaring no `states` therefore runs at the certifiable ceiling rather than at a
/// default nobody reviewed — the ceiling is *declared*, in `bfs`'s own constants, which is
/// what that module's "no silent caps" rule asks for.
#[must_use]
pub fn bounds_of(budget: &Budget) -> Bounds {
    match budget.states.value() {
        Some(states) => {
            Bounds::CERTIFIABLE.with_states(usize::try_from(*states).unwrap_or(usize::MAX))
        }
        None => Bounds::CERTIFIABLE,
    }
}

/// Which bound a partial exploration tripped.
#[must_use]
pub fn tripped(partial: &Partial) -> bfs::Bound {
    partial.tripped()
}

/// The budget dimensions a caller declared and this daemon does not enforce (INV-007).
fn unenforced(budget: &Budget) -> Vec<Omission> {
    let declared: [(&str, bool); 8] = [
        ("budget.wall_ms", budget.wall_ms.value().is_some()),
        ("budget.cpu_ms", budget.cpu_ms.value().is_some()),
        ("budget.memory_bytes", budget.memory_bytes.value().is_some()),
        ("budget.solver_ms", budget.solver_ms.value().is_some()),
        ("budget.proof_ms", budget.proof_ms.value().is_some()),
        ("budget.tokens", budget.tokens.value().is_some()),
        ("budget.candidates", budget.candidates.value().is_some()),
        ("budget.bytes", budget.bytes.value().is_some()),
    ];
    declared
        .into_iter()
        .filter(|(_, present)| *present)
        .map(|(subject, _)| unsupported(subject))
        .collect()
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
fn run(model: &Model, target: &Target, bounds: Bounds) -> Result<Result<Campaign, Fault>, Fault> {
    let obligations = obligations(model, target)?;
    let exploration = match bfs::explore(model, bounds) {
        Ok(exploration) => exploration,
        Err(ExplorationError::InitialStatesExceedBound { .. }) => {
            // Not a partial result and not resumable: there is no prefix of the walk to
            // report, because initial states are what exploration starts *from*. This is the
            // `failed_reason = BudgetExhausted` with no continuation that RFC 0026 requires a
            // `non_resumable_reason` beside.
            return Ok(Err(Fault::new(
                ErrorCode::BudgetExhausted,
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
    let report = checking::check(model, &exploration, &obligations).map_err(|_| {
        Fault::new(
            ErrorCode::MalformedRequest,
            "the target names a predicate the model this snapshot elaborates to does not \
             declare",
        )
    })?;
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
/// | the model is not one this daemon holds | `Fail(UnsupportedSemanticFeature)` | nothing was published, and nothing is left behind for a resume |
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
/// # Errors
///
/// [`Fault`] when the model can no longer be constructed, when the identity seam refuses to
/// name a continuation, or when the run itself is not a question about this model.
pub fn advance(
    handle: &TaskHandle,
    bounds: Bounds,
    state: &mut DaemonState,
    services: &Services,
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
            scope, handle, bounds, &source, &target, &snapshot, &intent, &now, state, services,
        )
    })?;
    Ok(())
}

/// One run, inside the scope that owns it. See [`advance`] for the step table.
#[allow(clippy::too_many_arguments)]
fn run_in(
    scope: RegionScope,
    handle: &TaskHandle,
    bounds: Bounds,
    source: &Commitment,
    target: &Target,
    snapshot: &Nullable<crate::protocol::scalar::WorkspaceHandle>,
    intent: &Nullable<crate::protocol::scalar::IntentHandle>,
    now: &Option<crate::protocol::scalar::Timestamp>,
    state: &mut DaemonState,
    services: &Services,
) -> Result<(), Fault> {
    // The model is data — named variables, named actions, an explicit initial-state
    // enumeration — so cloning it out of the catalog costs a copy of that data and buys the
    // disjoint borrow the task table needs. Nothing about the run depends on the copy.
    let Some(model) = state.models().get(source).cloned() else {
        let fault = no_model();
        state.regions_mut().fail(scope, fault.code);
        return Err(fault);
    };

    let outcome = match run(&model, target, bounds) {
        Ok(outcome) => outcome,
        Err(fault) => {
            state.regions_mut().fail(scope, fault.code);
            return Err(fault);
        }
    };
    let campaign = match outcome {
        Ok(campaign) => campaign,
        Err(fault) => {
            state.regions_mut().fail(scope, fault.code);
            let entry = state
                .tasks_mut()
                .get_mut(handle)
                .ok_or_else(Fault::denied)?;
            entry.failed_reason = Some(fault.code);
            entry.non_resumable_reason = Some(fault.detail.to_owned());
            entry.advance(TaskStatus::Failed, now.as_ref());
            return Ok(());
        }
    };

    // The result exists and nothing a reader can observe does. The store-side counterpart is
    // `StagedPublication`, whose phase says outright: "nothing is stored".
    state.regions_mut().step(scope, WorkerStep::Reserve);

    // Park before the status moves, so a `Suspended` task never exists without the
    // continuation `rule task.status_monotonic` says it has by definition.
    let continuation = if campaign.is_closed() {
        None
    } else {
        let pinned = PinnedEpochs::of(services.epochs());
        let Some(snapshot) = snapshot.value().cloned() else {
            // Unreachable, and it discards the staged publication rather than leaving it in
            // flight: a campaign without a snapshot could not have been started.
            state.regions_mut().fail(scope, ErrorCode::CapabilityDenied);
            return Err(Fault::denied());
        };
        let mut preimage = Preimage::new();
        preimage.text(handle.as_str());
        preimage.text(snapshot.as_str());
        preimage.text(&campaign.states().to_string());
        preimage.text(&campaign.frontier.len().to_string());
        for state in &campaign.frontier {
            for component in state.as_slice() {
                preimage.push(&component.to_be_bytes());
            }
        }
        epochs_preimage(&mut preimage, services.epochs());
        let named = match continuation_handle(services.identifier(), &preimage) {
            Ok(named) => named,
            Err(fault) => {
                state.regions_mut().fail(scope, fault.code);
                return Err(fault);
            }
        };
        let parked = Continuation {
            handle: named.clone(),
            task: handle.clone(),
            snapshot,
            intent: intent.clone(),
            pinned,
            bounds,
            frontier: campaign.frontier.clone(),
        };
        state.tasks_mut().park(parked);
        Some(named)
    };

    let closed = campaign.is_closed();
    {
        let entry = state
            .tasks_mut()
            .get_mut(handle)
            .ok_or_else(Fault::denied)?;
        entry.campaign = Some(campaign);
        entry.continuation = continuation;
        entry.publications += 1;
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
            },
            Arguments::VerificationResult(_) | Arguments::VerificationAwait(_) => ScopeClaim {
                snapshots: Vec::new(),
                intents: Vec::new(),
                classes: vec![ArtifactClass::Task.token()],
            },
            _ => ScopeClaim::default(),
        }
    }

    fn handle(
        &self,
        call: &Call<'_>,
        state: &mut DaemonState,
        services: &Services,
        _store: &ReferenceStore,
    ) -> Result<Effect, Fault> {
        match call.arguments {
            Arguments::VerificationStart(request) => start(call, request, state, services),
            Arguments::VerificationResult(request) => result(&request.task, state, false),
            Arguments::VerificationAwait(request) => result(&request.task, state, true),
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
) -> Result<Effect, Fault> {
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
    if !record.sealed {
        return Err(Fault::new(
            ErrorCode::StaleSnapshot,
            "a verification campaign runs over a sealed snapshot",
        ));
    }
    let intent = record.intent.clone();
    let lineage_name = record.lineage.clone();
    let head = record.descriptor.source().identity().clone();
    let modules = snapshot_modules(record.descriptor.source());
    let lineage = state.lineage(&lineage_name).ok_or_else(Fault::denied)?;
    check_current(lineage, &head).map_err(|_| {
        Fault::new(
            ErrorCode::StaleSnapshot,
            "the named snapshot has been superseded in its lineage",
        )
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

    // The cached-result lane. A completed task under this identity is the same campaign, and
    // re-running it could only produce the same answer more slowly.
    if let Some(entry) = state.tasks().get(&handle) {
        if entry.status == TaskStatus::Completed {
            let (payload, verdict, omissions) = cached(entry)?;
            // The cached lane answers with a *result*, not a task, so its status is `ok`;
            // the envelope still names the task the result is about, which is the second
            // clause of the `task` presence rule.
            let mut effect = Effect::new(payload, verdict.0).observing(entry.handle.clone());
            effect.assurance = verdict.1;
            effect.omissions = omissions;
            return Ok(effect);
        }
        return Ok(started(entry, &budget));
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
        bounds: bounds_of(&budget),
        budget: budget.clone(),
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
        publications: 0,
    };
    let bounds = entry.bounds;
    state.tasks_mut().put(entry);
    advance(&handle, bounds, state, services)?;

    let entry = state.tasks().get(&handle).ok_or_else(Fault::denied)?;
    Ok(started(entry, &budget))
}

/// The `verification.start` answer that names a task.
///
/// The envelope's status lane is read off the task rather than fixed: a campaign that
/// parked reports `task_suspended` and names the continuation that resumes it, and every
/// other outcome reports `task_started`, which is what `ResultStatus::task_started` says
/// — "a long operation was started; `task` is present". Both lanes are open to this
/// operation because it is `@task_starting`, and until bn-i4aem item 9 neither was
/// reachable: `result::success` hard-coded `ok` with both handles absent, so a parked
/// campaign's continuation was reachable only by a second `task.status` call.
fn started(entry: &TaskEntry, budget: &Budget) -> Effect {
    let mut effect = Effect::new(
        Payload::VerificationStart(VerificationStartResponse {
            task: Optional::Present(entry.handle.clone()),
            result: Optional::Absent,
        }),
        Nullable::Null,
    );
    effect.omissions = [entry.omissions(), unenforced(budget)].concat();
    match (entry.status, &entry.continuation) {
        (TaskStatus::Suspended, Some(continuation)) => {
            effect.suspended(entry.handle.clone(), continuation.clone())
        }
        _ => effect.started(entry.handle.clone()),
    }
}

/// The `verification.start` answer that carries a cached result instead of a task.
type Cached = (
    Payload,
    (Nullable<Verdict>, Optional<AssuranceEnvelope>),
    Vec<Omission>,
);

fn cached(entry: &TaskEntry) -> Result<Cached, Fault> {
    let (result, _) = verification_result(entry)?;
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
fn result(handle: &TaskHandle, state: &DaemonState, awaiting: bool) -> Result<Effect, Fault> {
    let entry = state.tasks().get(handle).ok_or_else(Fault::denied)?;
    let (result, verdict) = verification_result(entry)?;
    let payload = if awaiting {
        Payload::VerificationAwait(result)
    } else {
        Payload::VerificationResult(result)
    };
    let mut effect = Effect::new(payload, Nullable::Value(Verdict::Semantic(verdict)))
        .observing(entry.handle.clone());
    effect.assurance = Optional::Present(assurance(entry));
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

/// The typed result of one task, and the verdict it carries.
fn verification_result(
    entry: &TaskEntry,
) -> Result<(VerificationResult, SemanticVerdictValue), Fault> {
    let Some(campaign) = &entry.campaign else {
        return Err(Fault::new(
            ErrorCode::BudgetExhausted,
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
/// reason because this engine declares no fault model, no fairness constraint, no
/// concurrency, no memory model, no observer and no certificate. Every summary is a stable
/// machine token — docs/03 §3's own scope names, and the IDL's own
/// `sequential-consistency-only` example — and none of them interpolates anything
/// (`rule envelope.no_prose`).
fn assurance(entry: &TaskEntry) -> AssuranceEnvelope {
    let scope = entry
        .campaign
        .as_ref()
        .map_or("no-exploration", |campaign| match campaign.report.scope() {
            Scope::Complete { .. } => "exhaustive-finite",
            Scope::Bounded { .. } => "bounded-states",
        });
    AssuranceEnvelope {
        bounds: produced(scope),
        faults: unsupported_dimension("no-fault-model"),
        fairness: unsupported_dimension("no-fairness-model"),
        values: produced("finite-declared-domains"),
        schedules: unsupported_dimension("no-concurrency-model"),
        memory_model: unsupported_dimension("sequential-consistency-only"),
        observer: unsupported_dimension("no-observer-model"),
        proof_status: unsupported_dimension("no-certificate-emitted"),
        unknowns: produced(
            if entry
                .campaign
                .as_ref()
                .is_some_and(super::task::Campaign::is_closed)
            {
                "none"
            } else {
                "bounded-frontier"
            },
        ),
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

fn no_model() -> Fault {
    Fault::new(
        ErrorCode::UnsupportedSemanticFeature,
        "this daemon compiles no CML source; it verifies the models a deployment registered, \
         and the named snapshot's modules are not one of them",
    )
}
