//! The tests' own notion of a genuinely complete journal prefix (cr-3pu5cu), shared by
//! the truncation sweeps of `a7_primitive_conformance.rs` and
//! `pr14_deadline_cancellation.rs`. It reads the event list alone and calls no lift.

use std::collections::{BTreeMap, BTreeSet};

use continuum_asupersync::family::EventBody;
use continuum_asupersync::family::lifecycle::{LifecycleEvent, TaskStep};
use continuum_asupersync::family::obligation::ObligationEvent;
use continuum_asupersync::family::time::TimeEvent;

/// Whether a journal prefix ends inside a protocol the substrate completes within one
/// operation. This is the test's own definition of a *genuinely complete* prefix, stated
/// from the event list alone, not from the lift: a prefix is complete when it ends
/// outside every one of these, and mid-protocol otherwise.
///
/// - a task's cancellation phase not yet absorbed by the drain that lists it or by the
///   task's own lifecycle `cancel`;
/// - a task's own `cancel-requested` without its `cancel`;
/// - a region's cancel request whose region has not been drained;
/// - a drained region not finalized;
/// - with the obligations family in the prefix, a finalized region not settled;
/// - a timer whose deadline the clock has passed, neither fired nor cancelled.
///
/// A parked task, a held reservation, an open obligation, an armed timer that is not
/// due, a blocked channel operation, and a normally closing region that waits for owned
/// work are states a run can end in: prefixes ending there are complete.
///
/// Completeness is relative to the families a prefix carries. A journal has no alphabet
/// header, so a prefix with no event of a family reads as that family's projection,
/// which is complete without it: before its first obligation event, a finalized region
/// owes no settle.
pub fn mid_protocol(prefix: &[EventBody]) -> Option<&'static str> {
    let obligations_observed = prefix
        .iter()
        .any(|body| matches!(body, EventBody::Obligation(_)));
    let mut phased = BTreeSet::new();
    let mut absorbed = BTreeSet::new();
    let mut alone = BTreeSet::new();
    let mut ended_alone = BTreeSet::new();
    let mut cancel_requested = BTreeSet::new();
    let mut drained = BTreeSet::new();
    let mut finalized = BTreeSet::new();
    let mut settled = BTreeSet::new();
    let mut timers: BTreeMap<u32, u64> = BTreeMap::new();
    let mut now = 0;
    for body in prefix {
        match body {
            EventBody::Cancellation(event) => {
                phased.insert(event.task());
            }
            EventBody::Lifecycle(LifecycleEvent::TaskStepped { task, step }) => match step {
                TaskStep::CancelRequested => {
                    alone.insert(*task);
                }
                TaskStep::Cancel => {
                    ended_alone.insert(*task);
                    absorbed.insert(*task);
                }
                _ => {}
            },
            EventBody::Lifecycle(LifecycleEvent::RegionCancelRequested { region }) => {
                cancel_requested.insert(*region);
            }
            EventBody::Lifecycle(LifecycleEvent::RegionDrained { region, cancelled }) => {
                drained.insert(*region);
                absorbed.extend(cancelled.as_slice().iter().copied());
            }
            EventBody::Lifecycle(LifecycleEvent::RegionFinalized { region }) => {
                finalized.insert(*region);
            }
            EventBody::Obligation(ObligationEvent::RegionSettled { region, .. }) => {
                settled.insert(*region);
            }
            EventBody::Time(TimeEvent::Scheduled {
                timer, deadline, ..
            }) => {
                timers.insert(timer.0, deadline.0);
            }
            EventBody::Time(
                TimeEvent::Fired { timer, .. } | TimeEvent::Cancelled { timer, .. },
            ) => {
                timers.remove(&timer.0);
            }
            EventBody::Time(TimeEvent::Advanced { to, .. }) => now = to.0,
            _ => {}
        }
    }
    if phased.iter().any(|task| !absorbed.contains(task)) {
        return Some("a cancellation not absorbed");
    }
    if alone.iter().any(|task| !ended_alone.contains(task)) {
        return Some("a single-task cancellation not ended");
    }
    if cancel_requested
        .iter()
        .any(|region| !drained.contains(region))
    {
        return Some("a region's cancellation not drained");
    }
    if drained.iter().any(|region| !finalized.contains(region)) {
        return Some("a drain not finalized");
    }
    if obligations_observed && finalized.iter().any(|region| !settled.contains(region)) {
        return Some("a finalize not settled");
    }
    if timers.values().any(|deadline| *deadline <= now) {
        return Some("a due timer not fired");
    }
    None
}
