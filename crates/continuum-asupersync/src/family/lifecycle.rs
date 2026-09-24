//! The task and region lifecycle family (PR-14-IMPL-01, bn-lf4i).
//!
//! # The vocabulary, and where it comes from
//!
//! docs/01 §6 maps the substrate's *task* to "task identity and program order" and its
//! *region* to "ownership/lifecycle resource". The lifecycle those words must obey is
//! already written down in `continuum_task::region` (bn-2gk), so this family's events
//! are that calculus's operations, reported as facts:
//!
//! | event | region calculus operation |
//! |---|---|
//! | [`LifecycleEvent::RegionOpened`] | `RegionTree::open_child` |
//! | [`LifecycleEvent::TaskSpawned`] | `RegionTree::spawn` |
//! | [`LifecycleEvent::TaskStepped`] | `RegionTree::advance` (begin, suspend, resume, complete, fail; `cancel` is the calculus's single-task `CompleteCancelled`, bn-36wy3) |
//! | [`LifecycleEvent::RegionCloseRequested`] | `RegionTree::close` |
//! | [`LifecycleEvent::RegionCancelRequested`] | `RegionTree::cancel` |
//! | [`LifecycleEvent::RegionDrained`] | `RegionTree::drain`, with the tasks the drain cancelled |
//! | [`LifecycleEvent::RegionFinalized`] | `RegionTree::finalize` |
//! | [`LifecycleEvent::RegionCrashed`] | a fail-stop crash of the region's subtree (bn-20d8u): each live task `Fail(crashed)` (a parked one taken off its park first, `Resume`), then `RegionTree::close` on the subtree's open regions |
//!
//! Two steps of the calculus are deliberately **not** here. `Reserve` and `Commit` are
//! the reserve/commit/abort family's (PR-14-IMPL-02), and the cancellation *phases* a
//! task observes are PR-14-IMPL-03's. A lifecycle journal therefore never stages a
//! publication. The region-level cancel request and its drain are here, because they
//! are region lifecycle transitions (`Open → Draining(cancelled) → Finalized`).
//!
//! # Identity
//!
//! A region and a task are named in the journal by dense ordinals in the order the
//! journal allocated them, root region `r0` first and implicit. They are not the
//! substrate's handles and not the model's: the lift checks that the model allocates
//! the same ordinal for the same event, which is a conformance check of its own.
//! Labels (`RegionLabel`, `TaskLabel`) are the scripted source's program-local names;
//! they never reach the journal, so two scripts that differ only in label spelling
//! produce the same bytes.

use std::collections::{BTreeMap, BTreeSet};

use continuum_task::region::worker::{
    CancelPhase, FailureReason, NonResumableReason, Resumability, WorkerId, WorkerState, WorkerStep,
};
use continuum_task::region::{DrainCause, RegionId, RegionState};

use crate::encoding::{DecodeError, Decoder, EncodeError, Encoder};
use crate::family::EventBody;
use crate::lift::{LiftContext, LiftStop, Nonconformance};
use crate::source::{RecordContext, RecordRefusal};

/// Whether this family's events exist yet.
pub const INSTRUMENTED: bool = true;

/// A region, by its position in the journal's allocation order. `r0` is the root.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RegionOrdinal(pub u32);

/// A task, by its position in the journal's spawn order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TaskOrdinal(pub u32);

/// A set of tasks, held strictly ascending so it has one spelling.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TaskSet(Vec<TaskOrdinal>);

impl TaskSet {
    /// The set of these tasks, in canonical order.
    #[must_use]
    pub fn new(tasks: impl IntoIterator<Item = TaskOrdinal>) -> Self {
        let mut tasks: Vec<TaskOrdinal> = tasks.into_iter().collect();
        tasks.sort_unstable();
        tasks.dedup();
        Self(tasks)
    }

    /// The members, ascending.
    #[must_use]
    pub fn as_slice(&self) -> &[TaskOrdinal] {
        &self.0
    }
}

/// A task step the lifecycle family reports: `continuum_task`'s `WorkerStep` without the
/// two publication steps, which are PR-14-IMPL-02's.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TaskStep {
    /// `Created → Running`.
    Begin,
    /// `Running → Suspended`.
    Suspend,
    /// `Suspended → Running`.
    Resume,
    /// `Running → Completed`.
    Complete,
    /// `Created | Running → Failed`, naming why.
    Fail(FailureReason),
    /// The task ended as cancelled on its own, while its region stayed as it was: a
    /// deadline cancellation's end (the calculus's `CompleteCancelled`, RFC 0026
    /// correction 53; bn-36wy3). A region's cancellation ends its tasks in the region's
    /// drain instead.
    Cancel,
    /// The task's own cancellation was requested, outside any region's cancellation: its
    /// deadline passed (the calculus's `RequestCancel`; bn-36wy3). The single-task
    /// counterpart of [`LifecycleEvent::RegionCancelRequested`], and like it journaled
    /// before the cleanup it starts.
    CancelRequested,
}

impl TaskStep {
    /// A stable token, as the rendering spells the step.
    pub(crate) const fn token(&self) -> &'static str {
        match self {
            Self::Begin => "begin",
            Self::Suspend => "suspend",
            Self::Resume => "resume",
            Self::Complete => "complete",
            Self::Fail(_) => "fail",
            Self::Cancel => "cancel",
            Self::CancelRequested => "cancel-requested",
        }
    }

    const fn tag(&self) -> u8 {
        match self {
            Self::Begin => 1,
            Self::Suspend => 2,
            Self::Resume => 3,
            Self::Complete => 4,
            Self::Fail(_) => 5,
            Self::Cancel => 6,
            Self::CancelRequested => 7,
        }
    }

    /// The model step this report corresponds to.
    #[must_use]
    pub fn to_worker_step(&self) -> WorkerStep {
        match self {
            Self::Begin => WorkerStep::Begin,
            Self::Suspend => WorkerStep::Suspend,
            Self::Resume => WorkerStep::Resume,
            Self::Complete => WorkerStep::Complete,
            Self::Fail(reason) => WorkerStep::Fail(reason.clone()),
            Self::Cancel => WorkerStep::CompleteCancelled,
            Self::CancelRequested => WorkerStep::RequestCancel,
        }
    }

    fn render(&self) -> String {
        match self {
            Self::Begin => "begin".to_owned(),
            Self::Suspend => "suspend".to_owned(),
            Self::Resume => "resume".to_owned(),
            Self::Complete => "complete".to_owned(),
            Self::Fail(reason) => format!("fail({reason})"),
            Self::Cancel => "cancel".to_owned(),
            Self::CancelRequested => "cancel-requested".to_owned(),
        }
    }
}

/// One task or region lifecycle event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LifecycleEvent {
    /// A child region was opened inside `parent`.
    RegionOpened {
        /// The new region.
        region: RegionOrdinal,
        /// Its parent.
        parent: RegionOrdinal,
    },
    /// A task was admitted into `region`.
    TaskSpawned {
        /// The new task.
        task: TaskOrdinal,
        /// Its owning region.
        region: RegionOrdinal,
        /// Whether its work can be resumed, as declared at spawn.
        resumability: Resumability,
    },
    /// A task took one lifecycle step.
    TaskStepped {
        /// The task.
        task: TaskOrdinal,
        /// The step.
        step: TaskStep,
    },
    /// A normal close was requested on `region` and its open subtree.
    RegionCloseRequested {
        /// The region.
        region: RegionOrdinal,
    },
    /// Cancellation was requested on `region` and its subtree.
    RegionCancelRequested {
        /// The region.
        region: RegionOrdinal,
    },
    /// `region`'s subtree was drained, and these tasks were cancelled by the drain.
    RegionDrained {
        /// The region.
        region: RegionOrdinal,
        /// The tasks the drain moved to `cancelled`.
        cancelled: TaskSet,
    },
    /// `region`'s subtree was finalized.
    RegionFinalized {
        /// The region.
        region: RegionOrdinal,
    },
    /// `region`'s subtree crashed, fail-stop (bn-20d8u; RFC 0026 correction 58): these
    /// tasks, every task of the subtree that had not ended, stopped where they were.
    /// None of their code ran at the crash or after it, no cancellation handler and no
    /// cleanup; what they held is fenced, not aborted. The subtree's open regions are
    /// closed with it. Encoding version 3.
    RegionCrashed {
        /// The subtree's root.
        region: RegionOrdinal,
        /// The tasks the crash stopped.
        fenced: TaskSet,
    },
}

impl LifecycleEvent {
    /// A stable token for the event's kind (a task step is named by its step).
    pub(crate) const fn token(&self) -> &'static str {
        match self {
            Self::RegionOpened { .. } => "region-opened",
            Self::TaskSpawned { .. } => "task-spawned",
            Self::TaskStepped { step, .. } => step.token(),
            Self::RegionCloseRequested { .. } => "region-close-requested",
            Self::RegionCancelRequested { .. } => "region-cancel-requested",
            Self::RegionDrained { .. } => "region-drained",
            Self::RegionFinalized { .. } => "region-finalized",
            Self::RegionCrashed { .. } => "region-crashed",
        }
    }

    const fn tag(&self) -> u8 {
        match self {
            Self::RegionOpened { .. } => 1,
            Self::TaskSpawned { .. } => 2,
            Self::TaskStepped { .. } => 3,
            Self::RegionCloseRequested { .. } => 4,
            Self::RegionCancelRequested { .. } => 5,
            Self::RegionDrained { .. } => 6,
            Self::RegionFinalized { .. } => 7,
            Self::RegionCrashed { .. } => 8,
        }
    }
}

fn encode_tasks(tasks: &TaskSet, out: &mut Encoder) -> Result<(), EncodeError> {
    let count = u32::try_from(tasks.0.len())
        .map_err(|_| EncodeError::FieldTooLong { len: tasks.0.len() })?;
    out.u32(count);
    for task in &tasks.0 {
        out.u32(task.0);
    }
    Ok(())
}

fn decode_tasks(input: &mut Decoder<'_>) -> Result<TaskSet, DecodeError> {
    let set_at = input.offset();
    let count = input.u32()?;
    let mut tasks: Vec<TaskOrdinal> = Vec::new();
    for _ in 0..count {
        let task = TaskOrdinal(input.u32()?);
        if tasks.last().is_some_and(|last| *last >= task) {
            return Err(DecodeError::UnsortedSet { at: set_at });
        }
        tasks.push(task);
    }
    Ok(TaskSet(tasks))
}

pub(crate) fn encode(event: &LifecycleEvent, out: &mut Encoder) -> Result<(), EncodeError> {
    out.tag(event.tag());
    match event {
        LifecycleEvent::RegionOpened { region, parent } => {
            out.u32(region.0);
            out.u32(parent.0);
        }
        LifecycleEvent::TaskSpawned {
            task,
            region,
            resumability,
        } => {
            out.u32(task.0);
            out.u32(region.0);
            match resumability {
                Resumability::Resumable => out.tag(1),
                Resumability::NonResumable(reason) => {
                    out.tag(2);
                    out.token(reason.as_str())?;
                }
            }
        }
        LifecycleEvent::TaskStepped { task, step } => {
            out.u32(task.0);
            out.tag(step.tag());
            if let TaskStep::Fail(reason) = step {
                out.token(reason.as_str())?;
            }
        }
        LifecycleEvent::RegionCloseRequested { region }
        | LifecycleEvent::RegionCancelRequested { region }
        | LifecycleEvent::RegionFinalized { region } => out.u32(region.0),
        LifecycleEvent::RegionDrained {
            region,
            cancelled: tasks,
        }
        | LifecycleEvent::RegionCrashed {
            region,
            fenced: tasks,
        } => {
            out.u32(region.0);
            encode_tasks(tasks, out)?;
        }
    }
    Ok(())
}

fn reason_token<T>(
    input: &mut Decoder<'_>,
    make: fn(&str) -> Result<T, continuum_task::region::worker::ReasonError>,
) -> Result<T, DecodeError> {
    let at = input.offset();
    let token = input.token()?;
    make(token).map_err(|_| DecodeError::NonCanonicalToken { at })
}

pub(crate) fn decode(input: &mut Decoder<'_>, _seq: u64) -> Result<LifecycleEvent, DecodeError> {
    let at = input.offset();
    let tag = input.tag()?;
    Ok(match tag {
        1 => LifecycleEvent::RegionOpened {
            region: RegionOrdinal(input.u32()?),
            parent: RegionOrdinal(input.u32()?),
        },
        2 => {
            let task = TaskOrdinal(input.u32()?);
            let region = RegionOrdinal(input.u32()?);
            let tag_at = input.offset();
            let resumability = match input.tag()? {
                1 => Resumability::Resumable,
                2 => Resumability::NonResumable(reason_token(input, NonResumableReason::new)?),
                other => {
                    return Err(DecodeError::UnknownTag {
                        table: "resumability",
                        tag: other,
                        at: tag_at,
                    });
                }
            };
            LifecycleEvent::TaskSpawned {
                task,
                region,
                resumability,
            }
        }
        3 => {
            let task = TaskOrdinal(input.u32()?);
            let tag_at = input.offset();
            let step = match input.tag()? {
                1 => TaskStep::Begin,
                2 => TaskStep::Suspend,
                3 => TaskStep::Resume,
                4 => TaskStep::Complete,
                5 => TaskStep::Fail(reason_token(input, FailureReason::new)?),
                6 => {
                    input.require_version(2, "lifecycle task step", 6, tag_at)?;
                    TaskStep::Cancel
                }
                7 => {
                    input.require_version(2, "lifecycle task step", 7, tag_at)?;
                    TaskStep::CancelRequested
                }
                other => {
                    return Err(DecodeError::UnknownTag {
                        table: "lifecycle task step",
                        tag: other,
                        at: tag_at,
                    });
                }
            };
            LifecycleEvent::TaskStepped { task, step }
        }
        4 => LifecycleEvent::RegionCloseRequested {
            region: RegionOrdinal(input.u32()?),
        },
        5 => LifecycleEvent::RegionCancelRequested {
            region: RegionOrdinal(input.u32()?),
        },
        6 => LifecycleEvent::RegionDrained {
            region: RegionOrdinal(input.u32()?),
            cancelled: decode_tasks(input)?,
        },
        7 => LifecycleEvent::RegionFinalized {
            region: RegionOrdinal(input.u32()?),
        },
        8 => {
            input.require_version(3, "lifecycle event", 8, at)?;
            LifecycleEvent::RegionCrashed {
                region: RegionOrdinal(input.u32()?),
                fenced: decode_tasks(input)?,
            }
        }
        other => {
            return Err(DecodeError::UnknownTag {
                table: "lifecycle event",
                tag: other,
                at,
            });
        }
    })
}

pub(crate) fn render(event: &LifecycleEvent) -> String {
    match event {
        LifecycleEvent::RegionOpened { region, parent } => {
            format!("region-opened r{} parent=r{}", region.0, parent.0)
        }
        LifecycleEvent::TaskSpawned {
            task,
            region,
            resumability,
        } => {
            let resumability = match resumability {
                Resumability::Resumable => "resumable".to_owned(),
                Resumability::NonResumable(reason) => format!("non-resumable({reason})"),
            };
            format!(
                "task-spawned t{} region=r{} {resumability}",
                task.0, region.0
            )
        }
        LifecycleEvent::TaskStepped { task, step } => {
            format!("task-stepped t{} {}", task.0, step.render())
        }
        LifecycleEvent::RegionCloseRequested { region } => {
            format!("region-close-requested r{}", region.0)
        }
        LifecycleEvent::RegionCancelRequested { region } => {
            format!("region-cancel-requested r{}", region.0)
        }
        LifecycleEvent::RegionDrained { region, cancelled } => {
            let tasks: Vec<String> = cancelled.0.iter().map(|t| format!("t{}", t.0)).collect();
            format!(
                "region-drained r{} cancelled=[{}]",
                region.0,
                tasks.join(",")
            )
        }
        LifecycleEvent::RegionFinalized { region } => format!("region-finalized r{}", region.0),
        LifecycleEvent::RegionCrashed { region, fenced } => {
            let tasks: Vec<String> = fenced.0.iter().map(|t| format!("t{}", t.0)).collect();
            format!("region-crashed r{} fenced=[{}]", region.0, tasks.join(","))
        }
    }
}

// --- lift ----------------------------------------------------------------------------

fn ordinal(count: usize) -> u32 {
    u32::try_from(count).unwrap_or(u32::MAX)
}

/// The fences a fail-stop crash owes the journal, per family, by ordinal (bn-20d8u): the
/// reservations, obligations and timers its stopped tasks held, each of a family the
/// journal carries. They come right after the crash, before any other event.
#[derive(Debug, Default)]
pub struct FencesDue {
    pub(crate) reservations: BTreeSet<u32>,
    pub(crate) obligations: BTreeSet<u32>,
    pub(crate) timers: BTreeSet<u32>,
}

impl FencesDue {
    /// Whether nothing is owed.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.reservations.is_empty() && self.obligations.is_empty() && self.timers.is_empty()
    }

    /// The first fence owed, in the binding's order: reservations, then obligations,
    /// then timers, each by ascending ordinal.
    fn first(&self) -> Option<(crate::family::Family, u32)> {
        use crate::family::Family;
        self.reservations
            .first()
            .map(|r| (Family::Effect, *r))
            .or_else(|| self.obligations.first().map(|o| (Family::Obligation, *o)))
            .or_else(|| self.timers.first().map(|k| (Family::Time, *k)))
    }
}

/// While the last crash owes a fence, the next event is the first fence it owes, in the
/// binding's order (bn-20d8u); anything else, another owed fence included, is
/// [`Nonconformance::FenceOwed`].
pub(crate) fn check_fences_first(body: &EventBody, cx: &LiftContext) -> Result<(), LiftStop> {
    use crate::family::Family;
    let Some((family, ordinal)) = cx.fences.first() else {
        return Ok(());
    };
    // Exactly the next owed fence, in the binding's order: one spelling per crash.
    let fence = match body {
        EventBody::Effect(crate::family::effect::EffectEvent::Fenced { reservation }) => {
            Some((Family::Effect, reservation.0))
        }
        EventBody::Obligation(crate::family::obligation::ObligationEvent::Fenced {
            obligation,
        }) => Some((Family::Obligation, obligation.0)),
        EventBody::Time(crate::family::time::TimeEvent::Fenced { timer }) => {
            Some((Family::Time, timer.0))
        }
        _ => None,
    };
    if fence == Some((family, ordinal)) {
        Ok(())
    } else {
        Err(LiftStop::Violation(Nonconformance::FenceOwed {
            family,
            ordinal,
        }))
    }
}

/// The reason a crashed task's calculus worker fails with.
fn crashed_reason() -> FailureReason {
    FailureReason::new("crashed")
        .unwrap_or_else(|_| unreachable!("`crashed` is a canonical reason token"))
}

/// A lifecycle `region-crashed`: `region`'s subtree crashes, fail-stop (bn-20d8u; RFC
/// 0026 correction 58).
///
/// 1. The region is open, or closing normally, no earlier crash covered it, and no
///    region of its subtree is draining under cancellation or crashed and not yet
///    finalized ([`Nonconformance::CrashedRegionState`]); and no timer is already due,
///    since it fires first ([`crate::family::time::TimeFault::Late`]). The binding
///    produces none of these.
/// 2. The reported stopped set is exactly the subtree's live tasks
///    ([`Nonconformance::CrashOutcome`]), the drain check's counterpart.
/// 3. A stopped task in cancellation (its own or its region's, requested or
///    acknowledged), under a budget deadline, or holding a channel's receiver or a
///    blocked send has no crash semantics here: [`LiftStop::Unsupported`] of the family
///    that shows it, never a conformance. The binding refuses those crashes too.
/// 4. The calculus has no crash step, so each stopped worker fails with the reason
///    `crashed` (`Fail` discards what it staged, which publishes nothing), a parked one
///    taken off its park first because `Fail` leaves `Created | Running` only; then the
///    subtree's open regions are closed. The adapter obligations the workers held stay
///    open in the calculus's ledger: they are owed, and the region's finalization is not
///    total.
/// 5. What the stopped tasks held is owed as fences, per family the journal carries
///    ([`FencesDue`]); nothing of theirs happens after the crash, because every family
///    refuses a step of a terminal task.
fn lift_crash(cx: &mut LiftContext, region: u32, fenced: &TaskSet) -> Result<(), LiftStop> {
    let id = RegionId::at(region);
    let state = cx.tree.state(id)?;
    if cx.crashed_regions.contains(&region) {
        return Err(LiftStop::Violation(Nonconformance::CrashedRegionState {
            region,
            state: "crashed",
        }));
    }
    if !matches!(
        state,
        RegionState::Open | RegionState::Draining(DrainCause::Closed)
    ) || cx.drained.contains(&region)
    {
        return Err(LiftStop::Violation(Nonconformance::CrashedRegionState {
            region,
            state: state.token(),
        }));
    }
    // No region of the subtree is under cancellation, or crashed and not yet finalized:
    // the binding refuses both (review finding, bn-20d8u pre-review).
    for member in cx.tree.subtree(id)? {
        let member_state = cx.tree.state(member)?;
        if member_state == RegionState::Draining(DrainCause::Cancelled)
            || (cx.crashed_regions.contains(&member.ordinal())
                && member_state != RegionState::Finalized)
        {
            return Err(LiftStop::Violation(Nonconformance::CrashedRegionState {
                region: member.ordinal(),
                state: if member_state == RegionState::Finalized {
                    "crashed"
                } else {
                    member_state.token()
                },
            }));
        }
    }
    // A timer already due should have fired first: a crash may not fence it away.
    crate::family::time::check_none_late(cx)?;
    let model: Vec<u32> = cx
        .tree
        .outstanding_workers(id)?
        .into_iter()
        .map(WorkerId::ordinal)
        .collect();
    let reported: Vec<u32> = fenced.0.iter().map(|t| t.0).collect();
    if model != reported {
        return Err(LiftStop::Violation(Nonconformance::CrashOutcome {
            region,
            reported,
            model,
        }));
    }
    let channel_holders = crate::family::channel::holders(cx);
    for task in &model {
        let worker = WorkerId::at(*task);
        if cx.tree.cancel_phase(worker)? != CancelPhase::Active
            || crate::family::cancellation::phase_of(cx, *task).is_some()
        {
            return Err(LiftStop::Unsupported(crate::family::Family::Cancellation));
        }
        if crate::family::time::has_deadline(cx, *task) {
            return Err(LiftStop::Unsupported(crate::family::Family::Time));
        }
        if channel_holders.contains(task) {
            return Err(LiftStop::Unsupported(crate::family::Family::Channel));
        }
    }
    for task in &model {
        let worker = WorkerId::at(*task);
        if *cx.tree.worker_state(worker)? == WorkerState::Suspended {
            cx.tree.advance(worker, WorkerStep::Resume)?;
        }
        cx.tree
            .advance(worker, WorkerStep::Fail(crashed_reason()))?;
    }
    if state == RegionState::Open {
        cx.tree.close(id)?;
    }
    for member in cx.tree.subtree(id)? {
        cx.crashed_regions.insert(member.ordinal());
    }
    for task in &model {
        let reservations = crate::family::effect::reserved_by(cx, *task);
        cx.fences.reservations.extend(reservations);
        let obligations = crate::family::obligation::open_held_by(cx, *task);
        cx.fences.obligations.extend(obligations);
        let timers = crate::family::time::armed_by(cx, *task);
        cx.fences.timers.extend(timers);
    }
    Ok(())
}

/// The whole-journal check, run after the last event: the journal does not end inside a
/// region's teardown or a single task's cancellation (cr-3pu5cu).
///
/// A region's cancellation drains within the operation that requested it, a drain is
/// finalized in the same batch, and a task's own cancellation ends in its `cancel` step
/// within the operation that observed it. So a journal that ends with a region still
/// draining under cancellation, a drained region not finalized, or a task requested
/// alone and not ended by its own `cancel` is truncated: [`LiftStop::Incomplete`],
/// never a conformance. A task requested alone that another step ended is a violation,
/// reported at that step. A
/// normally closing region that waits for owned work, a parked task, a held
/// reservation, an open obligation, an armed timer or a blocked channel operation is a
/// state a run can end in, and is not checked here.
pub(crate) fn finish(cx: &LiftContext) -> Result<(), LiftStop> {
    // A violation `finish_alone` finds outranks an incomplete region found here, so the
    // region loop only notes the incomplete case and keeps scanning: it must not return
    // before `finish_alone` runs (cr-19ec8g).
    let count = ordinal(cx.tree.region_count());
    // A crash's fences and its subtree's close come within the operation that crashed
    // it, so a journal that ends before them is truncated (bn-20d8u).
    let mut incomplete = !cx.fences.is_empty();
    for region in 0..count {
        let state = cx.tree.state(RegionId::at(region))?;
        if state == RegionState::Draining(DrainCause::Cancelled)
            || (cx.drained.contains(&region) && state != RegionState::Finalized)
            || (cx.crashed_regions.contains(&region) && state != RegionState::Finalized)
        {
            incomplete = true;
        }
    }
    crate::family::cancellation::finish_alone(cx)?;
    if incomplete {
        return Err(LiftStop::Incomplete(crate::family::Family::Lifecycle));
    }
    Ok(())
}

/// A lifecycle `cancel-requested`: one task's own cancellation is requested
/// (bn-36wy3). The calculus's `RequestCancel`, which refuses a task a region's
/// cancellation already reached, after the deadline check when the journal shows the
/// clock.
fn lift_single_request(cx: &mut LiftContext, task: u32) -> Result<(), LiftStop> {
    crate::family::time::check_deadline_passed(cx, task)?;
    cx.tree
        .advance(WorkerId::at(task), WorkerStep::RequestCancel)?;
    crate::family::cancellation::note_requested_alone(cx, task);
    Ok(())
}

/// A lifecycle `cancel`: one task ends as cancelled on its own (bn-36wy3).
///
/// It needs the task's own `cancel-requested` before it. When the journal reports the
/// task's cancellation phases, they must have ended in `cancelled` with the `deadline`
/// cause, and the calculus already holds the acknowledgement. When it reports none (a
/// lifecycle projection), the step stands for the acknowledgement and the cleanup
/// checks a `cancelled` phase makes, as a drain's cancelled set stands for a region's.
/// Either way it ends with the calculus's `CompleteCancelled`.
fn lift_single_cancel(cx: &mut LiftContext, task: u32) -> Result<(), LiftStop> {
    let worker = WorkerId::at(task);
    crate::family::cancellation::check_requested_alone(cx, task)?;
    if !crate::family::cancellation::single_task_end(cx, task)? {
        if cx.tree.cancel_phase(worker)? != CancelPhase::Acknowledged {
            cx.tree.advance(worker, WorkerStep::AcknowledgeCancel)?;
        }
        crate::family::obligation::check_none_held(cx, task)?;
        crate::family::time::check_none_armed(cx, task)?;
        crate::family::channel::check_none_held(cx, task)?;
    }
    cx.tree.advance(worker, WorkerStep::CompleteCancelled)?;
    crate::family::cancellation::note_ended_alone(cx, task);
    Ok(())
}

pub(crate) fn lift(event: &LifecycleEvent, cx: &mut LiftContext) -> Result<(), LiftStop> {
    match event {
        LifecycleEvent::RegionCrashed { region, fenced } => lift_crash(cx, region.0, fenced)?,
        LifecycleEvent::RegionOpened { region, parent } => {
            let got = cx.tree.open_child(RegionId::at(parent.0))?;
            if got.ordinal() != region.0 {
                return Err(LiftStop::Violation(Nonconformance::RegionIdentity {
                    journal: region.0,
                    model: got.ordinal(),
                }));
            }
        }
        LifecycleEvent::TaskSpawned {
            task,
            region,
            resumability,
        } => {
            let got = cx
                .tree
                .spawn(RegionId::at(region.0), resumability.clone())?;
            if got.ordinal() != task.0 {
                return Err(LiftStop::Violation(Nonconformance::TaskIdentity {
                    journal: task.0,
                    model: got.ordinal(),
                }));
            }
        }
        LifecycleEvent::TaskStepped {
            task,
            step: TaskStep::Cancel,
        } => lift_single_cancel(cx, task.0)?,
        LifecycleEvent::TaskStepped {
            task,
            step: TaskStep::CancelRequested,
        } => lift_single_request(cx, task.0)?,
        LifecycleEvent::TaskStepped { task, step } => {
            if matches!(step, TaskStep::Complete | TaskStep::Fail(_)) {
                crate::family::cancellation::check_no_phase(cx, task.0, step.token())?;
            }
            cx.tree
                .advance(WorkerId::at(task.0), step.to_worker_step())?;
            // A `complete` or `fail` may not end a task whose own cancellation was
            // requested: only its own `cancel` does (cr-3pu5cu).
            crate::family::cancellation::check_not_absorbed(cx, task.0)?;
            if *step == TaskStep::Resume {
                // A sleeping task is woken by its timer: it may not resume while the
                // timer is still scheduled (bn-1i050).
                crate::family::time::check_not_asleep(cx, task.0)?;
            }
        }
        LifecycleEvent::RegionCloseRequested { region } => cx.tree.close(RegionId::at(region.0))?,
        LifecycleEvent::RegionCancelRequested { region } => {
            let state = cx.tree.state(RegionId::at(region.0))?;
            // A finalized region is the calculus's own `CancelFinalizedRegion`.
            if state == RegionState::Draining(DrainCause::Cancelled)
                || (state != RegionState::Finalized && cx.drained.contains(&region.0))
            {
                return Err(LiftStop::Violation(Nonconformance::RepeatedCancel {
                    region: region.0,
                }));
            }
            crate::family::time::check_cancel_not_raced(cx, region.0)?;
            cx.tree.cancel(RegionId::at(region.0))?;
        }
        LifecycleEvent::RegionDrained { region, cancelled } => {
            let count = ordinal(cx.tree.worker_count());
            let mut live = Vec::new();
            for worker in 0..count {
                if !cx.tree.worker_state(WorkerId::at(worker))?.is_terminal() {
                    live.push(worker);
                }
            }
            cx.tree.drain(RegionId::at(region.0))?;
            for member in cx.tree.subtree(RegionId::at(region.0))? {
                if cx.tree.state(member)? != RegionState::Finalized {
                    cx.drained.insert(member.ordinal());
                }
            }
            let mut model = Vec::new();
            for worker in live {
                if cx.tree.worker_state(WorkerId::at(worker))?.is_terminal() {
                    model.push(TaskOrdinal(worker));
                }
            }
            let model = TaskSet::new(model);
            if &model != cancelled {
                return Err(LiftStop::Violation(Nonconformance::DrainOutcome {
                    region: region.0,
                    reported: cancelled.0.iter().map(|t| t.0).collect(),
                    model: model.0.iter().map(|t| t.0).collect(),
                }));
            }
            // A region's drain may not end a task whose own cancellation was requested:
            // the shared `Cancelled` state does not stand for its missing own `cancel`
            // (RFC 0026 correction 53 items 3 and 6; cr-3pu5cu).
            for task in &model.0 {
                crate::family::cancellation::check_not_absorbed(cx, task.0)?;
            }
        }
        LifecycleEvent::RegionFinalized { region } => {
            // Which region of the subtree has no drain report, read before the
            // calculus's own finalize so that its faults (which name the stronger
            // violation) are reported first.
            let mut undrained = None;
            for member in cx.tree.subtree(RegionId::at(region.0))? {
                if undrained.is_none()
                    && !cx.drained.contains(&member.ordinal())
                    && cx.tree.state(member)? != RegionState::Finalized
                {
                    undrained = Some(member.ordinal());
                }
            }
            let finalization = cx.tree.finalize(RegionId::at(region.0))?;
            if let Some(undrained) = undrained {
                return Err(LiftStop::Violation(Nonconformance::FinalizeWithoutDrain {
                    region: region.0,
                    undrained,
                }));
            }
            cx.finalizations.push((cx.seq, finalization));
        }
    }
    Ok(())
}

// --- the scripted source -------------------------------------------------------------

/// A scripted source's program-local name for a region. `RegionLabel(0)` is the root,
/// bound before the first report.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RegionLabel(pub u32);

impl RegionLabel {
    /// The root region's label.
    pub const ROOT: Self = Self(0);
}

/// A scripted source's program-local name for a task.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TaskLabel(pub u32);

/// A lifecycle primitive, as a scripted source reports it: what was called, on which
/// handle, by label.
///
/// Only [`Self::Drain`] reports an outcome, and the recorder computes it (see
/// [`RecordState`]); every other report is the call itself.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LifecycleReport {
    /// Open `child` inside `parent`.
    OpenRegion {
        /// The parent.
        parent: RegionLabel,
        /// The label the new region is bound to.
        child: RegionLabel,
    },
    /// Spawn `task` into `region`.
    Spawn {
        /// The owning region.
        region: RegionLabel,
        /// The label the new task is bound to.
        task: TaskLabel,
        /// Declared resumability.
        resumability: Resumability,
    },
    /// Step `task`.
    Step {
        /// The task.
        task: TaskLabel,
        /// The step.
        step: TaskStep,
    },
    /// Request a normal close.
    Close {
        /// The region.
        region: RegionLabel,
    },
    /// Request cancellation.
    Cancel {
        /// The region.
        region: RegionLabel,
    },
    /// Drain.
    Drain {
        /// The region.
        region: RegionLabel,
    },
    /// Finalize.
    Finalize {
        /// The region.
        region: RegionLabel,
    },
    /// Crash the region's subtree, fail-stop (bn-20d8u). The recorder computes the
    /// stopped set: every live task of the subtree.
    Crash {
        /// The region.
        region: RegionLabel,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StandInRegion {
    Open,
    Closed,
    Cancelled,
    Finalized,
}

/// The scripted source's own bookkeeping: label bindings, and just enough of a region
/// tree to say which tasks a drain cancels.
///
/// That bookkeeping is a **second, independent** account of one slice of the region
/// rules — a drain cancels every task that has not completed or failed and whose region
/// is in a cancelled subtree — written here without calling `continuum_task`, so that
/// the lift's drain check compares two implementations rather than one with itself. It
/// judges nothing: an illegal report is still recorded, and the lift says so.
#[derive(Debug)]
pub struct RecordState {
    region_labels: BTreeMap<RegionLabel, RegionOrdinal>,
    task_labels: BTreeMap<TaskLabel, TaskOrdinal>,
    parents: Vec<Option<u32>>,
    regions: Vec<StandInRegion>,
    task_region: Vec<u32>,
    task_live: Vec<bool>,
}

impl Default for RecordState {
    fn default() -> Self {
        let mut region_labels = BTreeMap::new();
        region_labels.insert(RegionLabel::ROOT, RegionOrdinal(0));
        Self {
            region_labels,
            task_labels: BTreeMap::new(),
            parents: vec![None],
            regions: vec![StandInRegion::Open],
            task_region: Vec::new(),
            task_live: Vec::new(),
        }
    }
}

impl RecordState {
    fn region(&self, label: RegionLabel) -> Result<RegionOrdinal, RecordRefusal> {
        self.region_labels
            .get(&label)
            .copied()
            .ok_or(RecordRefusal::UnboundRegion(label.0))
    }

    fn task(&self, label: TaskLabel) -> Result<TaskOrdinal, RecordRefusal> {
        self.task_labels
            .get(&label)
            .copied()
            .ok_or(RecordRefusal::UnboundTask(label.0))
    }

    fn in_subtree(&self, member: u32, root: u32) -> bool {
        let mut at = Some(member);
        while let Some(region) = at {
            if region == root {
                return true;
            }
            at = self.parents.get(region as usize).copied().flatten();
        }
        false
    }

    fn mark_subtree(&mut self, root: u32, rewrite: impl Fn(StandInRegion) -> StandInRegion) {
        for region in 0..ordinal(self.regions.len()) {
            if self.in_subtree(region, root) {
                let slot = &mut self.regions[region as usize];
                *slot = rewrite(*slot);
            }
        }
    }
}

pub(crate) fn record(
    report: &LifecycleReport,
    cx: &mut RecordContext,
) -> Result<(), RecordRefusal> {
    let state = &mut cx.lifecycle;
    let event = match report {
        LifecycleReport::OpenRegion { parent, child } => {
            let parent = state.region(*parent)?;
            if state.region_labels.contains_key(child) {
                return Err(RecordRefusal::RegionLabelRebound(child.0));
            }
            let region = RegionOrdinal(ordinal(state.regions.len()));
            state.region_labels.insert(*child, region);
            state.parents.push(Some(parent.0));
            state.regions.push(StandInRegion::Open);
            LifecycleEvent::RegionOpened { region, parent }
        }
        LifecycleReport::Spawn {
            region,
            task,
            resumability,
        } => {
            let region = state.region(*region)?;
            if state.task_labels.contains_key(task) {
                return Err(RecordRefusal::TaskLabelRebound(task.0));
            }
            let ordinal_now = TaskOrdinal(ordinal(state.task_live.len()));
            state.task_labels.insert(*task, ordinal_now);
            state.task_region.push(region.0);
            state.task_live.push(true);
            LifecycleEvent::TaskSpawned {
                task: ordinal_now,
                region,
                resumability: resumability.clone(),
            }
        }
        LifecycleReport::Step { task, step } => {
            let task = state.task(*task)?;
            if matches!(
                step,
                TaskStep::Complete | TaskStep::Fail(_) | TaskStep::Cancel
            ) {
                state.task_live[task.0 as usize] = false;
            }
            LifecycleEvent::TaskStepped {
                task,
                step: step.clone(),
            }
        }
        LifecycleReport::Close { region } => {
            let region = state.region(*region)?;
            state.mark_subtree(region.0, |s| {
                if s == StandInRegion::Open {
                    StandInRegion::Closed
                } else {
                    s
                }
            });
            LifecycleEvent::RegionCloseRequested { region }
        }
        LifecycleReport::Cancel { region } => {
            let region = state.region(*region)?;
            state.mark_subtree(region.0, |s| {
                if s == StandInRegion::Finalized {
                    s
                } else {
                    StandInRegion::Cancelled
                }
            });
            LifecycleEvent::RegionCancelRequested { region }
        }
        LifecycleReport::Drain { region } => {
            let region = state.region(*region)?;
            let mut cancelled = Vec::new();
            for task in 0..ordinal(state.task_live.len()) {
                let owner = state.task_region[task as usize];
                if state.task_live[task as usize]
                    && state.in_subtree(owner, region.0)
                    && state.regions[owner as usize] == StandInRegion::Cancelled
                {
                    state.task_live[task as usize] = false;
                    cancelled.push(TaskOrdinal(task));
                }
            }
            LifecycleEvent::RegionDrained {
                region,
                cancelled: TaskSet::new(cancelled),
            }
        }
        LifecycleReport::Finalize { region } => {
            let region = state.region(*region)?;
            state.mark_subtree(region.0, |_| StandInRegion::Finalized);
            LifecycleEvent::RegionFinalized { region }
        }
        LifecycleReport::Crash { region } => {
            let region = state.region(*region)?;
            let mut fenced = Vec::new();
            for task in 0..ordinal(state.task_live.len()) {
                let owner = state.task_region[task as usize];
                if state.task_live[task as usize] && state.in_subtree(owner, region.0) {
                    state.task_live[task as usize] = false;
                    fenced.push(TaskOrdinal(task));
                }
            }
            state.mark_subtree(region.0, |s| {
                if s == StandInRegion::Open {
                    StandInRegion::Closed
                } else {
                    s
                }
            });
            LifecycleEvent::RegionCrashed {
                region,
                fenced: TaskSet::new(fenced),
            }
        }
    };
    cx.append(EventBody::Lifecycle(event));
    Ok(())
}
