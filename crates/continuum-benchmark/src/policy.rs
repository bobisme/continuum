//! The agent model: scripted deterministic policies, and the faults they are made to make.
//!
//! # What the "agent" is, and what it deliberately is not
//!
//! It is a state machine. [`Policy::decide`] is a pure function of the task, the schedule,
//! and what the arm has *learned so far* ([`AgentView`]); it reads no clock, draws no random
//! number, and calls no model. That is a deliberate narrowing of research/25's experiment 1
//! ("native handles vs shell/CLI on identical agent/model"), and the narrowing is the whole
//! reason the result is reproducible: a live LLM would make the two arms differ by sampling
//! noise, and no seed count fixes that.
//!
//! What it costs is stated rather than hidden. A scripted policy cannot *discover* a
//! strategy, so this instrument cannot answer "would a real agent use the typed surface
//! better?" — only "does the same behaviour cost less and fail less through the typed
//! surface?". The first question needs a live agent and is the falsification bone's
//! (bn-2c0a); `crate::report`'s own limits section says so beside the numbers.
//!
//! # One policy, two arms
//!
//! The same [`Policy`] drives both arms. This is the load-bearing fairness property of the
//! whole comparison: the arms are handed *identical* decisions, so a measured difference is a
//! difference in what the interface costs and in what the interface lets the agent *learn*,
//! never a difference in how well each arm was written.
//!
//! The learning half is where the interface actually shows up. A decision is a function of
//! [`AgentView`], and the view is updated from a [`Reading`](crate::surface::Reading) that
//! each arm produces from its own answer — typed fields on one side, scraped text on the
//! other. An arm that cannot recover a datum leaves the view's field `None`, and the policy
//! then does what an agent does: it asks again.
//!
//! # Seeds without randomness
//!
//! Plan §24.5's ratified margins require "every metric paired per task over at least 3
//! seeds". A seed here selects a **declared schedule** — [`SCHEDULES`] — rather than seeding
//! a generator. Each schedule fixes two ordering choices an agent genuinely has (does it read
//! the result before or after confirming the task settled; how many times does it poll) and,
//! for the fallible policy, which mistakes it makes. Three schedules, three seeds, no entropy
//! (INV-005).

use continuumd::protocol::scalar::{ContinuationHandle, TaskHandle, WorkspaceHandle};
use continuumd::protocol::vocabulary::{ErrorCode, SemanticVerdict, TaskStatus};

use crate::families::{Injection, Mistake, MistakeClass, MistakeFamily};
use crate::rig::Principal;
use crate::surface::Reading;
use crate::task::BenchmarkTask;

/// The most operations one run may attempt before it is declared stuck.
///
/// A ceiling rather than a timeout, because a timeout is a clock. Every scheduled run
/// finishes far inside it; reaching it means the policy is looping, which is a defect in this
/// crate and is reported as [`Stuck::AttemptsExhausted`] rather than as a slow success.
pub const MAX_ATTEMPTS: u32 = 48;

/// The ceiling [`Fault::TightBudget`] declares: far below any task's frozen state count.
pub const TIGHT_CEILING: u64 = 4;

/// The bytes [`Fault::UnregisteredModel`] overwrites a `.ctm` with.
///
/// A comment line, so the overwrite is a *legal* file that is simply not the model any
/// daemon registered — which is the mistake being modelled ("I edited the spec, then asked
/// for a verdict"), rather than a corrupt snapshot.
pub const EDITED_MODULE: &[u8] = b"// edited by an agent that did not re-register the model\n";

/// One interface operation, with every handle it names already resolved.
///
/// Resolved rather than symbolic so that both arms are handed exactly the same call: an arm
/// that had to look a handle up would be doing work the other arm was not, and the
/// difference would land in the measurement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Call {
    /// `workspace.create` over the task's corpus port.
    CreateWorkspace {
        /// Whether to seal on creation.
        seal: bool,
    },
    /// `workspace.fork`, overwriting the port's `.ctm` with [`EDITED_MODULE`].
    ForkModule {
        /// The snapshot forked from.
        base: WorkspaceHandle,
    },
    /// `workspace.fork`, putting the port's own `.ctm` bytes back.
    ///
    /// The recovery from [`Fault::UnregisteredModel`], and the only one the daemon's own
    /// semantics leave open: a fork advances the snapshot's lineage, so the pre-fork
    /// snapshot is no longer current and `verification.start` over it is `StaleSnapshot`
    /// (`continuum-workspace::staleness`). Re-creating the original does not help for the
    /// same reason. What does is what a person would do — put the file back — which is a
    /// second fork carrying the corpus bytes, and which resolves to the registered model
    /// because a model is keyed by its source bytes and those are now the source bytes
    /// again.
    RestoreModule {
        /// The snapshot forked from.
        base: WorkspaceHandle,
    },
    /// `workspace.seal`.
    SealWorkspace {
        /// The snapshot sealed.
        snapshot: WorkspaceHandle,
    },
    /// `verification.start`.
    StartVerification {
        /// The sealed snapshot verified.
        snapshot: WorkspaceHandle,
        /// The declared state ceiling.
        states: u64,
    },
    /// `task.status`.
    PollTask {
        /// The task read.
        task: TaskHandle,
    },
    /// `verification.result`.
    FetchResult {
        /// The task whose verdict is read.
        task: TaskHandle,
    },
    /// `task.resume`.
    Resume {
        /// The continuation spent.
        continuation: ContinuationHandle,
        /// The new ceiling.
        states: u64,
    },
    /// `task.cancel`.
    Cancel {
        /// The task closed.
        task: TaskHandle,
    },
}

impl Call {
    /// The wire operation this call is.
    #[must_use]
    pub const fn operation(&self) -> &'static str {
        match self {
            Self::CreateWorkspace { .. } => "workspace.create",
            Self::ForkModule { .. } | Self::RestoreModule { .. } => "workspace.fork",
            Self::SealWorkspace { .. } => "workspace.seal",
            Self::StartVerification { .. } => "verification.start",
            Self::PollTask { .. } => "task.status",
            Self::FetchResult { .. } => "verification.result",
            Self::Resume { .. } => "task.resume",
            Self::Cancel { .. } => "task.cancel",
        }
    }

    /// The principal this call is *meant* to be made as.
    ///
    /// The **least** authority the operation's registry entry accepts, never more:
    /// `workspace.*` is `propose`, `verification.*`/`task.cancel`/`task.resume` are
    /// `execute`, and `task.status` is `read`
    /// (`continuumd::protocol::registry`). That is INV-015 as a habit rather than a slogan,
    /// and it makes [`Fault::WrongPrincipal`] a *real* mistake — presenting `read` where
    /// `execute` is required — rather than a contrived one against an over-provisioned
    /// agent. Nothing else in this crate departs from the table.
    #[must_use]
    pub const fn principal(&self) -> Principal {
        match self {
            Self::CreateWorkspace { .. }
            | Self::ForkModule { .. }
            | Self::RestoreModule { .. }
            | Self::SealWorkspace { .. } => Principal::BUILDER,
            Self::StartVerification { .. }
            | Self::Resume { .. }
            | Self::Cancel { .. }
            | Self::FetchResult { .. } => Principal::RUNNER,
            Self::PollTask { .. } => Principal::READER,
        }
    }
}

/// One decision: a call, and the principal to make it as.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Step {
    /// What to call.
    pub call: Call,
    /// Whose authority to present.
    pub principal: Principal,
    /// The fault this step embodies, when it is a deliberate mistake.
    pub fault: Option<Fault>,
    /// The fallible-policy family mistake this step embodies, when a family injected one.
    ///
    /// [`None`] on every landed run: a family is selected explicitly, and
    /// [`Policy::new`] selects none. See [`crate::families`] for what a mistake is and why
    /// the same one is handed to both arms.
    pub mistake: Option<Mistake>,
}

impl Step {
    /// A faithful step: the call, made as the principal it is meant to be made as.
    #[must_use]
    pub const fn faithful(call: Call) -> Self {
        let principal = call.principal();
        Self {
            call,
            principal,
            fault: None,
            mistake: None,
        }
    }
}

/// What the policy decided.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Decision {
    /// Make this call.
    Call(Box<Step>),
    /// The task is finished: everything the frozen answer needs has been read.
    Done,
    /// The policy cannot proceed.
    Stuck(Stuck),
}

/// Why a run stopped without finishing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stuck {
    /// [`MAX_ATTEMPTS`] operations were attempted.
    AttemptsExhausted,
    /// An arm admitted an operation and could not recover the handle it answered with, and
    /// re-reading did not help. This is the interface failure the benchmark exists to catch:
    /// the daemon answered, and the agent could not use the answer.
    UnreadableAnswer,
}

/// The deliberate mistakes the fallible policy makes.
///
/// Every one of them is an error surface **the daemon already has** — none is simulated, and
/// none is injected by patching a response. That is what the goal bone's IMPL-04 asks for:
/// "the daemon's own error surfaces are the injection points".
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Fault {
    /// Start a campaign presenting a `read` capability. The daemon answers
    /// `CapabilityDenied`, checked before any semantic work runs.
    WrongPrincipal,
    /// Start a campaign over a snapshot that was never sealed. The daemon answers
    /// `StaleSnapshot`; the typed client's register refuses it before the wire.
    UnsealedStart,
    /// Declare a ceiling of [`TIGHT_CEILING`] states. The campaign cannot close inside it, and
    /// recovery is the continuation the protocol parks.
    TightBudget,
    /// Edit the `.ctm` and verify the fork. The daemon answers
    /// `UnsupportedSemanticFeature`: a changed model is a different model.
    UnregisteredModel,
}

impl Fault {
    /// Every fault, in report order.
    pub const ALL: [Self; 4] = [
        Self::WrongPrincipal,
        Self::UnsealedStart,
        Self::TightBudget,
        Self::UnregisteredModel,
    ];

    /// The stable token a report writes this fault as.
    #[must_use]
    pub const fn token(self) -> &'static str {
        match self {
            Self::WrongPrincipal => "wrong-principal",
            Self::UnsealedStart => "unsealed-start",
            Self::TightBudget => "tight-budget",
            Self::UnregisteredModel => "unregistered-model",
        }
    }
}

/// A declared schedule: one seed's worth of choices, with no generator behind it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Schedule {
    /// The seed this schedule is named by.
    pub seed: u16,
    /// Read the verdict before confirming the task settled.
    pub result_first: bool,
    /// How many extra `task.status` calls the agent makes beyond the first.
    pub extra_polls: u32,
    /// The mistakes the fallible policy makes, fired in this order.
    pub faults: &'static [Fault],
}

/// The three declared schedules.
///
/// Plan §24.5's ratified margins require "at least 3 seeds"; these are those three. They
/// differ in the two orderings a real agent genuinely chooses between and in which mistakes
/// it makes, and they are declared rather than sampled so that seed 2 means the same thing
/// on every machine, forever.
pub const SCHEDULES: &[Schedule] = &[
    Schedule {
        seed: 1,
        result_first: false,
        extra_polls: 0,
        faults: &[Fault::TightBudget],
    },
    Schedule {
        seed: 2,
        result_first: true,
        extra_polls: 1,
        faults: &[Fault::WrongPrincipal, Fault::UnsealedStart],
    },
    Schedule {
        seed: 3,
        result_first: false,
        extra_polls: 2,
        faults: &[Fault::UnregisteredModel],
    },
];

/// Which agent is being simulated.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PolicyKind {
    /// Makes no mistakes. The IMPL-01/02/03/05 arm.
    Faithful,
    /// Makes the schedule's mistakes, once each. The IMPL-04 arm.
    Fallible,
}

impl PolicyKind {
    /// Every policy, in report order.
    pub const ALL: [Self; 2] = [Self::Faithful, Self::Fallible];

    /// The stable token a report writes this policy as.
    #[must_use]
    pub const fn token(self) -> &'static str {
        match self {
            Self::Faithful => "faithful",
            Self::Fallible => "fallible",
        }
    }
}

/// What an arm has learned so far.
///
/// This is the agent's whole memory. It is deliberately small, and every field is something
/// an interface either hands over or does not: the difference between the arms is *which of
/// these fields come back filled in, and at what cost*.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AgentView {
    /// The first snapshot created, which is the one the corpus port actually resolves to.
    pub origin: Option<WorkspaceHandle>,
    /// The snapshot currently being worked on.
    pub snapshot: Option<WorkspaceHandle>,
    /// Whether that snapshot is sealed.
    pub sealed: bool,
    /// The campaign's task handle.
    pub task: Option<TaskHandle>,
    /// The continuation a suspension parked.
    pub continuation: Option<ContinuationHandle>,
    /// Whether the task reported a terminal status.
    pub settled: bool,
    /// The reachable-state count read off a task record.
    pub states: Option<u64>,
    /// The campaign's verdict.
    pub verdict: Option<SemanticVerdict>,
    /// Whether the declared state ceiling was metered (INV-007).
    ///
    /// An agent that does not know this cannot read a bounded answer: "inconclusive under a
    /// ceiling of 64 states" and "inconclusive under a ceiling nobody applied" call for
    /// different next actions. The daemon answers it in the omission manifest, and the two
    /// arms pay differently to get it — see [`crate::shell`].
    pub ceiling_enforced: Option<bool>,
    /// Whether `verification.result` has been read.
    pub result_read: bool,
    /// How many `task.status` calls have been admitted.
    pub polls: u32,
    /// How many operations have been attempted.
    pub attempts: u32,
    /// The typed code of the last refusal, when there was one.
    pub last_code: Option<ErrorCode>,
    /// Whether the last attempt was admitted.
    pub last_admitted: bool,
    /// Faults fired so far, in order.
    pub fired: Vec<Fault>,
    /// Family mistakes made so far, in order.
    ///
    /// Separate from [`AgentView::fired`] because a fault and a family mistake are different
    /// injections with different owners: the four faults are IMPL-04's landed schedule, and a
    /// mistake is a [`crate::families::MistakeFamily`]'s. Keeping them apart is what lets a
    /// family sweep run *over* the landed matrix without either injection rewriting the
    /// other's step.
    pub mistakes: Vec<MistakeClass>,
    /// How many times the agent has re-issued a read because the answer was unreadable.
    pub rereads: u32,
}

impl AgentView {
    /// Fold one arm's reading of one answer into the agent's memory.
    ///
    /// The rule is uniform and is the honest one for both arms: a field the arm *recovered*
    /// overwrites the memory; a field it did not recover leaves the memory alone. A shell arm
    /// that cannot find a handle in a rendering therefore does not lose a handle it already
    /// had — which is the disciplined behaviour, and keeps the baseline from losing on an
    /// accounting artefact.
    pub fn absorb(&mut self, call: &Call, admitted: bool, code: Option<ErrorCode>, read: &Reading) {
        self.attempts += 1;
        self.last_admitted = admitted;
        self.last_code = code;
        if !admitted {
            return;
        }
        if let Some(snapshot) = &read.snapshot {
            self.snapshot = Some(snapshot.clone());
            if self.origin.is_none() {
                self.origin = Some(snapshot.clone());
            }
        }
        if let Some(sealed) = read.sealed {
            self.sealed = sealed;
        }
        if let Some(task) = &read.task {
            self.task = Some(task.clone());
        }
        if let Some(states) = read.states {
            self.states = Some(states);
        }
        if let Some(verdict) = read.verdict {
            self.verdict = Some(verdict);
        }
        if let Some(enforced) = read.ceiling_enforced {
            self.ceiling_enforced = Some(enforced);
        }
        // A continuation is three-valued on the wire — present, or absent because there is
        // nothing parked — so `Reading::continuation_known` says which, and only a *known*
        // absence clears one the agent is holding.
        if read.continuation_known {
            self.continuation = read.continuation.clone();
        }
        if let Some(status) = read.status {
            self.settled = matches!(
                status,
                TaskStatus::Completed | TaskStatus::Failed | TaskStatus::Cancelled
            );
        }
        match call {
            Call::PollTask { .. } => self.polls += 1,
            Call::FetchResult { .. } => self.result_read = true,
            Call::ForkModule { .. } | Call::RestoreModule { .. } => self.sealed = false,
            _ => {}
        }
    }

    /// Whether the run has read everything the frozen answer is graded on.
    ///
    /// Four facts and one meta-fact. The four are the answer: the campaign settled, its
    /// verdict was read, and its reachable-state count was read. The meta-fact is
    /// [`AgentView::ceiling_enforced`] — whether the ceiling the agent declared was metered —
    /// and it is part of completion rather than a bonus because an agent that does not know
    /// it cannot say whether the verdict it holds is about the whole model or about however
    /// much of it happened to be explored.
    #[must_use]
    pub const fn complete(&self) -> bool {
        self.settled
            && self.result_read
            && self.states.is_some()
            && self.verdict.is_some()
            && self.ceiling_enforced.is_some()
    }
}

/// A scripted agent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Policy {
    /// Faithful or fallible.
    pub kind: PolicyKind,
    /// The seed's declared schedule.
    pub schedule: Schedule,
    /// The fallible-policy family this policy is injecting, when one was selected.
    ///
    /// [`None`] on every landed run, which is what keeps the five landed metrics exactly
    /// where bn-134i left them: a family is an *addition* beside them, never a repurposing
    /// of one.
    pub injection: Option<Injection>,
}

impl Policy {
    /// The policy for one `(kind, seed)` pair.
    ///
    /// # Panics
    ///
    /// When `seed` names no declared schedule. Seeds are a closed set; asking for one outside
    /// it is a caller error rather than a run-time condition.
    #[must_use]
    pub fn new(kind: PolicyKind, seed: u16) -> Self {
        let schedule = *SCHEDULES
            .iter()
            .find(|schedule| schedule.seed == seed)
            .expect("a declared seed");
        Self {
            kind,
            schedule,
            injection: None,
        }
    }

    /// The same policy, injecting one fallible-policy family's mistake.
    ///
    /// # Panics
    ///
    /// As [`Policy::new`].
    #[must_use]
    pub fn injecting(kind: PolicyKind, seed: u16, injection: Injection) -> Self {
        Self {
            injection: Some(injection),
            ..Self::new(kind, seed)
        }
    }

    /// The family this policy injects, when it injects one.
    #[must_use]
    pub fn family(&self) -> Option<MistakeFamily> {
        self.injection.map(|injection| injection.family)
    }

    /// Decide the next operation.
    #[must_use]
    pub fn decide(&self, task: &BenchmarkTask, view: &AgentView) -> Decision {
        if view.attempts >= MAX_ATTEMPTS {
            return Decision::Stuck(Stuck::AttemptsExhausted);
        }
        let Some(step) = self.faithful(task, view) else {
            return Decision::Done;
        };
        let step = self.perturb(view, step);
        Decision::Call(Box::new(self.mislead(task, view, step)))
    }

    /// Turn the decided step into the selected family's mistake, once per run.
    ///
    /// Applied after [`Policy::perturb`] so the two injections cannot rewrite one step, and
    /// applied identically on both arms because it happens *here*, above the surface seam:
    /// the arms are handed one [`Step`] carrying one [`Mistake`], and what differs is only
    /// whether the surface has a channel for it.
    fn mislead(&self, task: &BenchmarkTask, view: &AgentView, step: Step) -> Step {
        match self.injection {
            Some(injection) => injection.apply(task, view, step),
            None => step,
        }
    }

    /// The step a faultless agent would take, or [`None`] when the task is finished.
    fn faithful(&self, task: &BenchmarkTask, view: &AgentView) -> Option<Step> {
        // A model the daemon does not recognize is recovered from by putting the module
        // back: a second fork carrying the corpus bytes. See `Call::RestoreModule` for why
        // neither re-creating the original snapshot nor reaching for the pre-fork handle
        // works. This branch comes first because nothing below it can undo a fork.
        if view.last_code == Some(ErrorCode::UnsupportedSemanticFeature)
            && let Some(base) = view.snapshot.clone()
        {
            return Some(Step::faithful(Call::RestoreModule { base }));
        }
        let Some(snapshot) = view.snapshot.clone() else {
            return Some(Step::faithful(Call::CreateWorkspace { seal: false }));
        };
        if !view.sealed {
            return Some(Step::faithful(Call::SealWorkspace { snapshot }));
        }
        let Some(handle) = view.task.clone() else {
            return Some(Step::faithful(Call::StartVerification {
                snapshot,
                states: task.ceiling,
            }));
        };
        if !view.settled
            && let Some(continuation) = view.continuation.clone()
        {
            return Some(Step::faithful(Call::Resume {
                continuation,
                states: task.ceiling,
            }));
        }
        // A settled campaign whose verdict is inconclusive and which parked nothing has
        // nothing left to resume: the honest recovery is a fresh campaign under the full
        // ceiling, over the same sealed snapshot.
        if view.settled && view.verdict == Some(SemanticVerdict::Inconclusive) {
            return Some(Step::faithful(Call::StartVerification {
                snapshot,
                states: task.ceiling,
            }));
        }
        if self.schedule.result_first && !view.result_read {
            return Some(Step::faithful(Call::FetchResult { task: handle }));
        }
        if !view.settled || view.polls <= self.schedule.extra_polls {
            return Some(Step::faithful(Call::PollTask { task: handle }));
        }
        if !view.result_read {
            return Some(Step::faithful(Call::FetchResult { task: handle }));
        }
        None
    }

    /// Turn a faithful step into the mistake the schedule calls for, once each.
    fn perturb(&self, view: &AgentView, step: Step) -> Step {
        if self.kind == PolicyKind::Faithful {
            return step;
        }
        let Some(fault) = self
            .schedule
            .faults
            .iter()
            .copied()
            .find(|fault| !view.fired.contains(fault) && applies(*fault, &step))
        else {
            return step;
        };
        match (fault, step.call) {
            (Fault::WrongPrincipal, call) => Step {
                call,
                principal: Principal::READER,
                fault: Some(fault),
                mistake: None,
            },
            (Fault::UnsealedStart, Call::SealWorkspace { snapshot }) => Step {
                call: Call::StartVerification {
                    snapshot,
                    states: TIGHT_CEILING.max(1),
                },
                principal: Principal::RUNNER,
                fault: Some(fault),
                mistake: None,
            },
            (Fault::TightBudget, Call::StartVerification { snapshot, .. }) => Step {
                call: Call::StartVerification {
                    snapshot,
                    states: TIGHT_CEILING,
                },
                principal: Principal::RUNNER,
                fault: Some(fault),
                mistake: None,
            },
            (Fault::UnregisteredModel, Call::SealWorkspace { snapshot }) => Step {
                call: Call::ForkModule { base: snapshot },
                principal: Principal::BUILDER,
                fault: Some(fault),
                mistake: None,
            },
            (_, call) => Step::faithful(call),
        }
    }
}

/// Whether a fault can be expressed by perturbing this step.
///
/// A fault fires at the first step it fits and nowhere else, which is what makes a schedule a
/// schedule rather than a probability.
const fn applies(fault: Fault, step: &Step) -> bool {
    match fault {
        Fault::WrongPrincipal | Fault::TightBudget => {
            matches!(step.call, Call::StartVerification { .. })
        }
        Fault::UnsealedStart | Fault::UnregisteredModel => {
            matches!(step.call, Call::SealWorkspace { .. })
        }
    }
}
