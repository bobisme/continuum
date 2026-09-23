//! The substrate binding: asupersync's lab runtime, driven by a Continuum [`ChoiceLog`],
//! observed into the lifecycle family's journal events.
//!
//! # What is bound, and what is not
//!
//! The binding drives one family, [`Family::Lifecycle`]. The other five families are
//! still uninstrumented (their files under `src/family/` say so), and for them
//! [`substrate_binding`] answers with the typed absence
//! [`BindingAbsence::FamilyNotBound`], whose INV-008 reading is
//! [`InconclusiveReason::Unsupported`]. A caller never gets an empty journal that looks
//! like a run in which nothing happened.
//!
//! # Who decides the schedule
//!
//! A run is a set of actor [`Program`]s and a [`ChoiceLog`], exactly as for the scripted
//! source ([`crate::source::record`]): at each step the enabled actors are those with
//! operations left, in ascending actor index, and a choice indexes into that list. Each
//! chosen [`SubstrateOp`] is one call into the substrate's public API, followed by
//! `LabRuntime::run_until_idle`. Every operation makes at most one task runnable, except
//! a cancellation, whose drain set is recorded as a set. So the substrate's own seeded
//! scheduler has no choice left to make that reaches the journal: the choice log is the
//! scheduling authority (INV-005), and the lab seed is an explicit, journal-invisible
//! parameter ([`BindingConfig::lab_seed`]). The tests hold that as a metamorphic
//! relation over several seeds.
//!
//! # Where events come from
//!
//! Events are observations of the substrate, not restatements of the program. The
//! binding reads the lab runtime's own trace buffer after every operation, in the
//! buffer's sequence order, and maps each trace event it recognizes to one lifecycle
//! event:
//!
//! | substrate trace event | journal event |
//! |---|---|
//! | `RegionCreated` with a parent | [`LifecycleEvent::RegionOpened`] |
//! | `RegionCreated` of the root | none: `r0` is implicit |
//! | `Spawn` | [`LifecycleEvent::TaskSpawned`] (resumability as the program declared it: the substrate has no such notion) |
//! | gate trace `begin` / `suspend` / `resume` | [`LifecycleEvent::TaskStepped`] |
//! | `Complete`, join result `Ok` | [`LifecycleEvent::TaskStepped`] `complete` |
//! | `Complete`, join result `Cancelled` | the task joins the next drain set of an enclosing region |
//! | `RegionCancelled`, first in its subtree | [`LifecycleEvent::RegionCancelRequested`] |
//! | `RegionCloseComplete` | [`LifecycleEvent::RegionDrained`] then [`LifecycleEvent::RegionFinalized`] |
//!
//! [`LifecycleEvent::RegionCloseRequested`] is the one exception. The substrate traces no
//! event for a normal close request, so the binding records it when `begin_close` on the
//! region record returns `true`. That return value is the substrate's acknowledgement.
//!
//! The *gate* is the instrumented primitive for task suspension. Every bound task's body
//! is a loop over one `GateWait` future. The substrate polls it. The gate writes a
//! `begin`, `suspend` or `resume` mark into the substrate's trace through `Cx::trace`
//! when, and only when, the poll changes the task's lifecycle phase. So those marks are
//! ordered with the substrate's own events in one sequence. The gate also observes
//! cancellation through `Cx::checkpoint`, as asupersync's cooperative cancellation
//! requires.
//!
//! Some recognized trace events have no lifecycle event of their own:
//! `RegionCloseBegin` (the start of a close that the request event already reports),
//! and the task-level `CancelRequest` / `CancelAck` phases. Those phases are
//! PR-14-IMPL-03's family. At lifecycle grain they are summarized by the drain's
//! cancelled set. A trace event of any other kind is a typed refusal
//! ([`BindingRefusal::UninstrumentedEvent`]), never a dropped event.
//!
//! # No ambient output
//!
//! The binding builds its `LabRuntime` from an explicit `LabConfig` and never calls the
//! lab-test harness (`run_async_lab_test_with_config`, `run_async_under_lab*`) or the
//! `write_auto_crashpack*` methods. Those are the only paths in asupersync 0.5.0 that
//! write crashpacks to `./target/test-artifacts` (`src/lab/runtime.rs:5667-5686`). A test
//! pins the absence of those names from this file. The workspace's `.cargo/config.toml`
//! also sets `ASUPERSYNC_AUTO_ARTIFACTS=0` for every process cargo runs, as a second
//! barrier. The oracles' panic switches are turned off, so a substrate finding is a
//! typed refusal and not an abort.
//!
//! # Refusals
//!
//! A run that cannot be observed completely is a typed [`BindingRefusal`] and no
//! journal (INV-008). [`BindingRefusal::inconclusive_reason`] gives the INV-008 reading
//! of each refusal that has one.

use core::fmt;
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::task::{Context, Poll, Waker};

use asupersync::runtime::{JoinError, TaskHandle};
use asupersync::trace::event::{TraceData, TraceEvent, TraceEventKind};
use asupersync::{
    Budget, CancelReason, Cx, LabConfig, LabRuntime, RegionId as SubstrateRegion,
    TaskId as SubstrateTask,
};
use continuum_task::region::worker::Resumability;
use continuum_value::assurance::InconclusiveReason;

use crate::choice::ChoiceLog;
use crate::family::lifecycle::{
    LifecycleEvent, RegionLabel, RegionOrdinal, TaskLabel, TaskOrdinal, TaskSet, TaskStep,
};
use crate::family::{EventBody, Family};
use crate::journal::Journal;
use crate::source::RecordContext;

// --- which families are bound ------------------------------------------------------

/// Why the substrate binding does not drive a family.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BindingAbsence {
    /// The family's instrumentation is not bound to the substrate yet.
    FamilyNotBound(Family),
}

impl BindingAbsence {
    /// A stable token.
    #[must_use]
    pub const fn token(self) -> &'static str {
        match self {
            Self::FamilyNotBound(_) => "family-not-bound",
        }
    }
}

impl fmt::Display for BindingAbsence {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::FamilyNotBound(family) => write!(f, "{} ({family})", self.token()),
        }
    }
}

/// Whether events of a family can come from the real substrate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubstrateBinding {
    /// They can: [`run`] drives the substrate and observes this family's events.
    Bound,
    /// They cannot, for this reason.
    Absent(BindingAbsence),
}

impl SubstrateBinding {
    /// The INV-008 reason a request for this family's substrate events is
    /// inconclusive, or `None` when the family is bound.
    #[must_use]
    pub const fn inconclusive_reason(self) -> Option<InconclusiveReason> {
        match self {
            Self::Bound => None,
            Self::Absent(_) => Some(InconclusiveReason::Unsupported),
        }
    }
}

/// The binding this build has for `family`.
#[must_use]
pub const fn substrate_binding(family: Family) -> SubstrateBinding {
    match family {
        Family::Lifecycle => SubstrateBinding::Bound,
        other => SubstrateBinding::Absent(BindingAbsence::FamilyNotBound(other)),
    }
}

// --- programs ----------------------------------------------------------------------

/// One call into the substrate, by the program's labels.
///
/// Labels are program-local names, bound by [`SubstrateOp::OpenRegion`] and
/// [`SubstrateOp::Spawn`]. They never reach the journal. [`RegionLabel::ROOT`] is bound
/// to the substrate's root region before the first operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SubstrateOp {
    /// `RuntimeState::create_child_region(parent)`, bound to `child`.
    OpenRegion {
        /// The parent.
        parent: RegionLabel,
        /// The label the new region is bound to.
        child: RegionLabel,
    },
    /// `RuntimeState::create_task(region, gated body)`, bound to `task`. The task is
    /// not scheduled until [`SubstrateOp::Begin`].
    Spawn {
        /// The owning region.
        region: RegionLabel,
        /// The label the new task is bound to.
        task: TaskLabel,
        /// Declared resumability. The substrate has no such notion, so the journal
        /// carries the declaration.
        resumability: Resumability,
    },
    /// Schedule `task` for its first poll. Its body parks on its gate.
    Begin {
        /// The task.
        task: TaskLabel,
    },
    /// Wake `task` with a command to park again.
    Continue {
        /// The task.
        task: TaskLabel,
    },
    /// Wake `task` with a command to return.
    Finish {
        /// The task.
        task: TaskLabel,
    },
    /// `RegionRecord::begin_close(None)`, then `RuntimeState::advance_region_state`.
    Close {
        /// The region.
        region: RegionLabel,
    },
    /// `RuntimeState::cancel_request`, with its effects dispatched as the lab runtime's
    /// own helpers dispatch them.
    Cancel {
        /// The region.
        region: RegionLabel,
    },
}

impl SubstrateOp {
    const fn name(&self) -> &'static str {
        match self {
            Self::OpenRegion { .. } => "open-region",
            Self::Spawn { .. } => "spawn",
            Self::Begin { .. } => "begin",
            Self::Continue { .. } => "continue",
            Self::Finish { .. } => "finish",
            Self::Close { .. } => "close",
            Self::Cancel { .. } => "cancel",
        }
    }
}

/// One actor's operations, in program order.
pub type Program = Vec<SubstrateOp>;

/// The explicit parameters of a run. Nothing else configures the substrate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BindingConfig {
    /// The lab runtime's seed. The journal must not depend on it.
    pub lab_seed: u64,
    /// The substrate trace buffer's capacity. A run that overflows it is refused.
    pub trace_capacity: usize,
    /// The lab runtime's step limit. A run that reaches it is refused.
    pub max_steps: u64,
}

impl BindingConfig {
    /// A configuration with this lab seed and the default bounds.
    #[must_use]
    pub const fn new(lab_seed: u64) -> Self {
        Self {
            lab_seed,
            trace_capacity: 1 << 16,
            max_steps: 1 << 20,
        }
    }
}

// --- refusals ----------------------------------------------------------------------

/// Why a run could not be observed into a journal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BindingRefusal {
    /// An operation names a region label no earlier operation bound.
    UnboundRegion(u32),
    /// An operation names a task label no earlier operation bound.
    UnboundTask(u32),
    /// An operation binds a region label that is already bound.
    RegionLabelRebound(u32),
    /// An operation binds a task label that is already bound.
    TaskLabelRebound(u32),
    /// A choice indexes past the enabled actors.
    ChoiceOutOfRange {
        /// Position in the log.
        position: usize,
        /// The choice.
        choice: u32,
        /// How many actors were enabled.
        enabled: usize,
    },
    /// The log ended while operations remained.
    ChoiceLogExhausted {
        /// Operations not yet applied.
        remaining: usize,
    },
    /// Choices remained after every program finished.
    ChoiceLogOverrun {
        /// Position of the first unused choice.
        position: usize,
    },
    /// The journal could not be extended.
    JournalFull,
    /// The substrate refused an operation.
    SubstrateRefused {
        /// The operation.
        operation: &'static str,
        /// The substrate's own description.
        detail: String,
    },
    /// The lab runtime reached its step limit with work still runnable.
    StepLimit,
    /// The substrate trace buffer overflowed, so events were lost.
    TraceOverflow {
        /// Events pushed.
        pushed: u64,
        /// Buffer capacity.
        capacity: usize,
    },
    /// A trace event arrived after a later-sequenced one was already observed.
    TraceOutOfOrder {
        /// Its sequence number.
        seq: u64,
    },
    /// A trace event names a region or task this run did not create.
    UnknownSubstrateEntity {
        /// What the event named.
        what: &'static str,
    },
    /// A gate could not write its mark, because no `Cx` was current at the poll.
    GateUntraced,
    /// A completed task's outcome was not published.
    TaskOutcomeUnobserved {
        /// The task's journal ordinal.
        task: u32,
    },
    /// A task panicked. Task failure is not bound yet.
    TaskPanicked {
        /// The task's journal ordinal.
        task: u32,
    },
    /// A task was cancelled but no enclosing region's close completed.
    UndrainedCancellation {
        /// The task's journal ordinal.
        task: u32,
    },
    /// The substrate traced an event of a kind the lifecycle binding does not map.
    UninstrumentedEvent {
        /// The family the kind belongs to, when it belongs to one.
        family: Option<Family>,
        /// The substrate's name for the kind.
        kind: String,
    },
}

impl BindingRefusal {
    /// The INV-008 reason this refusal is an instance of, when it is one.
    ///
    /// Malformed programs and choice logs have none: they do not describe a run. Neither
    /// does [`Self::SubstrateRefused`]: the substrate answered an illegal call (a spawn
    /// into a closed region, say) with a refusal, so the program does not describe a
    /// run of this substrate either.
    #[must_use]
    pub const fn inconclusive_reason(&self) -> Option<InconclusiveReason> {
        match self {
            Self::UninstrumentedEvent { .. } | Self::TaskPanicked { .. } => {
                Some(InconclusiveReason::Unsupported)
            }
            Self::StepLimit => Some(InconclusiveReason::ResourceExhausted),
            Self::TraceOverflow { .. }
            | Self::TraceOutOfOrder { .. }
            | Self::UnknownSubstrateEntity { .. }
            | Self::GateUntraced
            | Self::TaskOutcomeUnobserved { .. }
            | Self::UndrainedCancellation { .. } => Some(InconclusiveReason::InsufficientTelemetry),
            Self::SubstrateRefused { .. }
            | Self::UnboundRegion(_)
            | Self::UnboundTask(_)
            | Self::RegionLabelRebound(_)
            | Self::TaskLabelRebound(_)
            | Self::ChoiceOutOfRange { .. }
            | Self::ChoiceLogExhausted { .. }
            | Self::ChoiceLogOverrun { .. }
            | Self::JournalFull => None,
        }
    }
}

impl fmt::Display for BindingRefusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnboundRegion(label) => write!(f, "region label {label} is not bound"),
            Self::UnboundTask(label) => write!(f, "task label {label} is not bound"),
            Self::RegionLabelRebound(label) => write!(f, "region label {label} is already bound"),
            Self::TaskLabelRebound(label) => write!(f, "task label {label} is already bound"),
            Self::ChoiceOutOfRange {
                position,
                choice,
                enabled,
            } => write!(
                f,
                "choice {choice} at position {position} exceeds the {enabled} enabled actors"
            ),
            Self::ChoiceLogExhausted { remaining } => write!(
                f,
                "the choice log ended with {remaining} operations unapplied"
            ),
            Self::ChoiceLogOverrun { position } => write!(
                f,
                "choice {position} was not used: every program had finished"
            ),
            Self::JournalFull => f.write_str("the journal cannot hold another event"),
            Self::SubstrateRefused { operation, detail } => {
                write!(f, "the substrate refused {operation}: {detail}")
            }
            Self::StepLimit => f.write_str("the lab runtime reached its step limit"),
            Self::TraceOverflow { pushed, capacity } => write!(
                f,
                "the substrate trace overflowed: {pushed} events into a buffer of {capacity}"
            ),
            Self::TraceOutOfOrder { seq } => {
                write!(f, "trace event {seq} arrived after a later one")
            }
            Self::UnknownSubstrateEntity { what } => {
                write!(f, "the substrate traced a {what} this run did not create")
            }
            Self::GateUntraced => f.write_str("a gate was polled with no current Cx"),
            Self::TaskOutcomeUnobserved { task } => {
                write!(f, "task t{task} completed but published no outcome")
            }
            Self::TaskPanicked { task } => {
                write!(f, "task t{task} panicked; task failure is not bound")
            }
            Self::UndrainedCancellation { task } => write!(
                f,
                "task t{task} was cancelled but no enclosing region's close completed"
            ),
            Self::UninstrumentedEvent { family, kind } => match family {
                Some(family) => write!(
                    f,
                    "the substrate traced {kind}, a {family} event, which is not bound ({})",
                    family.requirement()
                ),
                None => write!(f, "the substrate traced {kind}, which no family maps"),
            },
        }
    }
}

impl core::error::Error for BindingRefusal {}

// --- the gate ----------------------------------------------------------------------

/// The prefix of every gate mark in the substrate's trace.
const GATE_MARK: &str = "continuum.lifecycle-gate/1";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GatePhase {
    Created,
    Running,
    Suspended,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Command {
    Park,
    Finish,
}

#[derive(Debug)]
struct GateState {
    phase: GatePhase,
    commands: VecDeque<Command>,
    waker: Option<Waker>,
    untraced: bool,
}

type Gate = Arc<Mutex<GateState>>;

fn lock(gate: &Gate) -> MutexGuard<'_, GateState> {
    gate.lock().unwrap_or_else(PoisonError::into_inner)
}

/// One wait on a task's gate. It resolves to the next command, or to `None` when the
/// substrate has requested cancellation.
struct GateWait {
    gate: Gate,
    slot: usize,
}

impl Future for GateWait {
    type Output = Option<Command>;

    fn poll(self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Self::Output> {
        let cx = Cx::current();
        if cx.as_ref().is_some_and(|cx| cx.checkpoint().is_err()) {
            return Poll::Ready(None);
        }
        let mut marks: Vec<&'static str> = Vec::new();
        let mut state = lock(&self.gate);
        if state.phase == GatePhase::Created {
            state.phase = GatePhase::Running;
            marks.push("begin");
        }
        let outcome = if let Some(command) = state.commands.pop_front() {
            if state.phase == GatePhase::Suspended {
                state.phase = GatePhase::Running;
                marks.push("resume");
            }
            Poll::Ready(Some(command))
        } else {
            state.waker = Some(context.waker().clone());
            if state.phase == GatePhase::Running {
                state.phase = GatePhase::Suspended;
                marks.push("suspend");
            }
            Poll::Pending
        };
        if !marks.is_empty() {
            match &cx {
                Some(cx) => {
                    for mark in marks {
                        cx.trace(&format!("{GATE_MARK} {mark} {}", self.slot));
                    }
                }
                None => state.untraced = true,
            }
        }
        drop(state);
        outcome
    }
}

async fn gated_body(gate: Gate, slot: usize) {
    while let Some(Command::Park) = (GateWait {
        gate: Arc::clone(&gate),
        slot,
    })
    .await
    {}
}

// --- the driver --------------------------------------------------------------------

struct Slot {
    id: SubstrateTask,
    handle: TaskHandle<()>,
    gate: Gate,
    resumability: Resumability,
    ordinal: Option<TaskOrdinal>,
}

struct Driver {
    lab: LabRuntime,
    config: BindingConfig,
    region_labels: BTreeMap<RegionLabel, SubstrateRegion>,
    task_labels: BTreeMap<TaskLabel, usize>,
    slots: Vec<Slot>,
    slot_of: BTreeMap<SubstrateTask, usize>,
    root: SubstrateRegion,
    region_ordinals: BTreeMap<SubstrateRegion, RegionOrdinal>,
    parents: Vec<Option<u32>>,
    cancelled: BTreeSet<u32>,
    pending_cancelled: Vec<(TaskOrdinal, u32)>,
    next_task: u32,
    observed: BTreeSet<u64>,
    last_seq: Option<u64>,
    record: RecordContext,
}

fn ordinal(count: usize) -> u32 {
    u32::try_from(count).unwrap_or(u32::MAX)
}

impl Driver {
    fn new(config: BindingConfig) -> Result<Self, BindingRefusal> {
        let mut lab_config = LabConfig::new(config.lab_seed)
            .panic_on_leak(false)
            .panic_on_futurelock(false)
            .trace_capacity(config.trace_capacity)
            .max_steps(config.max_steps);
        lab_config.panic_on_cancellation_violation = false;
        let mut lab = LabRuntime::new(lab_config);
        let root = lab.state.create_root_region(Budget::INFINITE);
        let mut region_labels = BTreeMap::new();
        region_labels.insert(RegionLabel::ROOT, root);
        let mut driver = Self {
            lab,
            config,
            region_labels,
            task_labels: BTreeMap::new(),
            slots: Vec::new(),
            slot_of: BTreeMap::new(),
            root,
            region_ordinals: BTreeMap::new(),
            parents: Vec::new(),
            cancelled: BTreeSet::new(),
            pending_cancelled: Vec::new(),
            next_task: 0,
            observed: BTreeSet::new(),
            last_seq: None,
            record: RecordContext::default(),
        };
        driver.sync()?;
        if driver.region_ordinals.get(&root) != Some(&RegionOrdinal(0)) {
            return Err(BindingRefusal::UnknownSubstrateEntity {
                what: "root region",
            });
        }
        Ok(driver)
    }

    fn region(&self, label: RegionLabel) -> Result<SubstrateRegion, BindingRefusal> {
        self.region_labels
            .get(&label)
            .copied()
            .ok_or(BindingRefusal::UnboundRegion(label.0))
    }

    fn slot(&self, label: TaskLabel) -> Result<usize, BindingRefusal> {
        self.task_labels
            .get(&label)
            .copied()
            .ok_or(BindingRefusal::UnboundTask(label.0))
    }

    fn append(&mut self, event: LifecycleEvent) {
        self.record.append(EventBody::Lifecycle(event));
    }

    fn command(&mut self, label: TaskLabel, command: Command) -> Result<(), BindingRefusal> {
        let slot = self.slot(label)?;
        let waker = {
            let mut state = lock(&self.slots[slot].gate);
            state.commands.push_back(command);
            state.waker.take()
        };
        if let Some(waker) = waker {
            waker.wake();
        }
        self.run()
    }

    fn apply(&mut self, op: &SubstrateOp) -> Result<(), BindingRefusal> {
        let refused = |detail: String| BindingRefusal::SubstrateRefused {
            operation: op.name(),
            detail,
        };
        match op {
            SubstrateOp::OpenRegion { parent, child } => {
                let parent = self.region(*parent)?;
                if self.region_labels.contains_key(child) {
                    return Err(BindingRefusal::RegionLabelRebound(child.0));
                }
                let region = self
                    .lab
                    .state
                    .create_child_region(parent, Budget::INFINITE)
                    .map_err(|error| refused(format!("{error:?}")))?;
                self.region_labels.insert(*child, region);
                self.sync()
            }
            SubstrateOp::Spawn {
                region,
                task,
                resumability,
            } => {
                let region = self.region(*region)?;
                if self.task_labels.contains_key(task) {
                    return Err(BindingRefusal::TaskLabelRebound(task.0));
                }
                let slot = self.slots.len();
                let gate: Gate = Arc::new(Mutex::new(GateState {
                    phase: GatePhase::Created,
                    commands: VecDeque::new(),
                    waker: None,
                    untraced: false,
                }));
                let (id, handle) = self
                    .lab
                    .state
                    .create_task(
                        region,
                        Budget::INFINITE,
                        gated_body(Arc::clone(&gate), slot),
                    )
                    .map_err(|error| refused(format!("{error:?}")))?;
                self.slots.push(Slot {
                    id,
                    handle,
                    gate,
                    resumability: resumability.clone(),
                    ordinal: None,
                });
                self.slot_of.insert(id, slot);
                self.task_labels.insert(*task, slot);
                self.sync()
            }
            SubstrateOp::Begin { task } => {
                let id = self.slots[self.slot(*task)?].id;
                self.lab
                    .scheduler
                    .lock()
                    .schedule(id, Budget::INFINITE.priority);
                self.run()
            }
            SubstrateOp::Continue { task } => self.command(*task, Command::Park),
            SubstrateOp::Finish { task } => self.command(*task, Command::Finish),
            SubstrateOp::Close { region } => {
                let id = self.region(*region)?;
                let ordinal = self.ordinal_of(id)?;
                let began = self
                    .lab
                    .state
                    .region(id)
                    .ok_or_else(|| refused("the region record is gone".to_owned()))?
                    .begin_close(None);
                if !began {
                    return Err(refused("begin_close returned false".to_owned()));
                }
                self.append(LifecycleEvent::RegionCloseRequested { region: ordinal });
                self.lab.state.advance_region_state(id);
                self.run()
            }
            SubstrateOp::Cancel { region } => {
                let id = self.region(*region)?;
                if self.lab.state.region(id).is_none() {
                    return Err(refused("the region record is gone".to_owned()));
                }
                let effects = self.lab.state.cancel_request(
                    id,
                    &CancelReason::user("continuum lifecycle binding"),
                    None,
                );
                let (targets, wakes) = effects.into_parts();
                {
                    let mut scheduler = self.lab.scheduler.lock();
                    for (task, priority) in targets {
                        scheduler.schedule_cancel(task, priority);
                    }
                }
                wakes.dispatch();
                self.run()
            }
        }
    }

    fn run(&mut self) -> Result<(), BindingRefusal> {
        self.lab.run_until_idle();
        if !self.lab.scheduler.lock().is_empty() {
            return Err(BindingRefusal::StepLimit);
        }
        self.sync()
    }

    fn ordinal_of(&self, region: SubstrateRegion) -> Result<RegionOrdinal, BindingRefusal> {
        self.region_ordinals
            .get(&region)
            .copied()
            .ok_or(BindingRefusal::UnknownSubstrateEntity { what: "region" })
    }

    fn slot_of(&self, task: SubstrateTask) -> Result<usize, BindingRefusal> {
        self.slot_of
            .get(&task)
            .copied()
            .ok_or(BindingRefusal::UnknownSubstrateEntity { what: "task" })
    }

    fn task_ordinal(&self, slot: usize) -> Result<TaskOrdinal, BindingRefusal> {
        self.slots[slot]
            .ordinal
            .ok_or(BindingRefusal::UnknownSubstrateEntity {
                what: "task before its spawn",
            })
    }

    /// Whether `region` or one of its ancestors is already cancel-requested.
    fn under_cancel(&self, region: u32) -> bool {
        let mut at = Some(region);
        while let Some(current) = at {
            if self.cancelled.contains(&current) {
                return true;
            }
            at = self.parents.get(current as usize).copied().flatten();
        }
        false
    }

    fn in_subtree(&self, member: u32, root: u32) -> bool {
        let mut at = Some(member);
        while let Some(current) = at {
            if current == root {
                return true;
            }
            at = self.parents.get(current as usize).copied().flatten();
        }
        false
    }

    /// Observe every trace event not yet observed, in sequence order.
    fn sync(&mut self) -> Result<(), BindingRefusal> {
        let trace = self.lab.trace();
        let pushed = trace.total_pushed();
        if pushed > u64::try_from(self.config.trace_capacity).unwrap_or(u64::MAX) {
            return Err(BindingRefusal::TraceOverflow {
                pushed,
                capacity: self.config.trace_capacity,
            });
        }
        for event in trace.snapshot() {
            if self.observed.contains(&event.seq) {
                continue;
            }
            if self.last_seq.is_some_and(|last| event.seq < last) {
                return Err(BindingRefusal::TraceOutOfOrder { seq: event.seq });
            }
            self.observed.insert(event.seq);
            self.last_seq = Some(event.seq);
            self.observe(&event)?;
            if self.record.is_full() {
                return Err(BindingRefusal::JournalFull);
            }
        }
        if self.slots.iter().any(|slot| lock(&slot.gate).untraced) {
            return Err(BindingRefusal::GateUntraced);
        }
        Ok(())
    }

    fn observe(&mut self, event: &TraceEvent) -> Result<(), BindingRefusal> {
        match (&event.kind, &event.data) {
            (TraceEventKind::RegionCreated, TraceData::Region { region, parent }) => match parent {
                None if *region == self.root && self.region_ordinals.is_empty() => {
                    self.region_ordinals.insert(*region, RegionOrdinal(0));
                    self.parents.push(None);
                }
                None => {
                    return Err(BindingRefusal::UnknownSubstrateEntity {
                        what: "second root region",
                    });
                }
                Some(parent) => {
                    let parent = self.ordinal_of(*parent)?;
                    let new = RegionOrdinal(ordinal(self.parents.len()));
                    self.region_ordinals.insert(*region, new);
                    self.parents.push(Some(parent.0));
                    self.append(LifecycleEvent::RegionOpened {
                        region: new,
                        parent,
                    });
                }
            },
            (TraceEventKind::Spawn, TraceData::Task { task, region }) => {
                let slot = self.slot_of(*task)?;
                let region = self.ordinal_of(*region)?;
                let task = TaskOrdinal(self.next_task);
                self.next_task = self.next_task.saturating_add(1);
                self.slots[slot].ordinal = Some(task);
                self.append(LifecycleEvent::TaskSpawned {
                    task,
                    region,
                    resumability: self.slots[slot].resumability.clone(),
                });
            }
            (TraceEventKind::UserTrace, TraceData::Message(message)) => {
                // Only the gate writes user traces in a bound run. Any other message is
                // an event this binding cannot place, so it is refused, not dropped.
                let Some(rest) = message.strip_prefix(GATE_MARK) else {
                    return Err(BindingRefusal::UninstrumentedEvent {
                        family: None,
                        kind: "UserTrace".to_owned(),
                    });
                };
                let mut parts = rest.split_whitespace();
                let step = match parts.next() {
                    Some("begin") => TaskStep::Begin,
                    Some("suspend") => TaskStep::Suspend,
                    Some("resume") => TaskStep::Resume,
                    _ => {
                        return Err(BindingRefusal::UnknownSubstrateEntity { what: "gate mark" });
                    }
                };
                let slot = parts
                    .next()
                    .and_then(|slot| slot.parse::<usize>().ok())
                    .filter(|slot| *slot < self.slots.len())
                    .ok_or(BindingRefusal::UnknownSubstrateEntity { what: "gate" })?;
                let task = self.task_ordinal(slot)?;
                self.append(LifecycleEvent::TaskStepped { task, step });
            }
            (TraceEventKind::Complete, TraceData::Task { task, region }) => {
                let slot = self.slot_of(*task)?;
                let region = self.ordinal_of(*region)?;
                let ordinal = self.task_ordinal(slot)?;
                match self.slots[slot].handle.try_join() {
                    Ok(Some(())) => self.append(LifecycleEvent::TaskStepped {
                        task: ordinal,
                        step: TaskStep::Complete,
                    }),
                    Err(JoinError::Cancelled(_)) => {
                        self.pending_cancelled.push((ordinal, region.0));
                    }
                    Err(JoinError::Panicked(_)) => {
                        return Err(BindingRefusal::TaskPanicked { task: ordinal.0 });
                    }
                    Ok(None) | Err(JoinError::PolledAfterCompletion) => {
                        return Err(BindingRefusal::TaskOutcomeUnobserved { task: ordinal.0 });
                    }
                }
            }
            (TraceEventKind::RegionCancelled, TraceData::RegionCancel { region, .. }) => {
                let region = self.ordinal_of(*region)?;
                if !self.under_cancel(region.0) {
                    self.cancelled.insert(region.0);
                    self.append(LifecycleEvent::RegionCancelRequested { region });
                }
            }
            (TraceEventKind::RegionCloseComplete, TraceData::Region { region, .. }) => {
                let region = self.ordinal_of(*region)?;
                let (drained, kept): (Vec<_>, Vec<_>) = self
                    .pending_cancelled
                    .iter()
                    .partition(|(_, owner)| self.in_subtree(*owner, region.0));
                self.pending_cancelled = kept;
                self.append(LifecycleEvent::RegionDrained {
                    region,
                    cancelled: TaskSet::new(drained.into_iter().map(|(task, _)| task)),
                });
                self.append(LifecycleEvent::RegionFinalized { region });
            }
            (
                TraceEventKind::RegionCloseBegin
                | TraceEventKind::CancelRequest
                | TraceEventKind::CancelAck,
                _,
            ) => {}
            (kind, _) => {
                return Err(BindingRefusal::UninstrumentedEvent {
                    family: family_of(*kind),
                    kind: format!("{kind:?}"),
                });
            }
        }
        Ok(())
    }

    fn finish(mut self) -> Result<Journal, BindingRefusal> {
        self.sync()?;
        if let Some((task, _)) = self.pending_cancelled.first() {
            return Err(BindingRefusal::UndrainedCancellation { task: task.0 });
        }
        self.record
            .into_journal()
            .ok_or(BindingRefusal::JournalFull)
    }
}

/// The PR-14 family a substrate trace kind belongs to, for a refusal's reading.
const fn family_of(kind: TraceEventKind) -> Option<Family> {
    match kind {
        TraceEventKind::ObligationReserve
        | TraceEventKind::ObligationCommit
        | TraceEventKind::ObligationAbort
        | TraceEventKind::ObligationLeak => Some(Family::Obligation),
        TraceEventKind::TimeAdvance
        | TraceEventKind::TimerScheduled
        | TraceEventKind::TimerFired
        | TraceEventKind::TimerCancelled => Some(Family::Time),
        TraceEventKind::Spawn
        | TraceEventKind::Complete
        | TraceEventKind::RegionCreated
        | TraceEventKind::RegionCancelled
        | TraceEventKind::RegionCloseBegin
        | TraceEventKind::RegionCloseComplete => Some(Family::Lifecycle),
        TraceEventKind::CancelRequest | TraceEventKind::CancelAck => Some(Family::Cancellation),
        _ => None,
    }
}

/// Run `programs` on the substrate, interleaved by `log`, and observe the lifecycle
/// journal.
///
/// # Errors
///
/// A [`BindingRefusal`] naming the first operation, choice, or observation that could
/// not be completed. No partial journal is returned.
pub fn run(
    programs: &[Program],
    log: &ChoiceLog,
    config: &BindingConfig,
) -> Result<Journal, BindingRefusal> {
    let mut driver = Driver::new(*config)?;
    let mut cursors = vec![0_usize; programs.len()];
    let mut choices = log.choices().iter().enumerate();
    loop {
        let enabled: Vec<usize> = (0..programs.len())
            .filter(|actor| cursors[*actor] < programs[*actor].len())
            .collect();
        let Some((position, choice)) = choices.next() else {
            if enabled.is_empty() {
                break;
            }
            let remaining = enabled
                .iter()
                .map(|actor| programs[*actor].len() - cursors[*actor])
                .sum();
            return Err(BindingRefusal::ChoiceLogExhausted { remaining });
        };
        if enabled.is_empty() {
            return Err(BindingRefusal::ChoiceLogOverrun { position });
        }
        let actor = *enabled
            .get(choice.0 as usize)
            .ok_or(BindingRefusal::ChoiceOutOfRange {
                position,
                choice: choice.0,
                enabled: enabled.len(),
            })?;
        driver.apply(&programs[actor][cursors[actor]])?;
        cursors[actor] += 1;
    }
    driver.finish()
}
