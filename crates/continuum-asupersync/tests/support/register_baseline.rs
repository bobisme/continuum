//! The correct version of the replicated register and its baseline campaign
//! (PR-16/IMPL-04, bn-5fpl).
//!
//! Include it beside the program, the projection and the A7 model:
//!
//! ```text
//! #[path = "support/primitive_conformance_model.rs"] mod model;
//! #[path = "support/replicated_register.rs"] mod register;
//! #[path = "support/register_baseline.rs"] mod baseline;
//! ```
//!
//! # What "the correct version" is
//!
//! `notes/plan/examples/replicated_register.md` and its scenario
//! (`replicated_register.scenario.toml`) define it by what must hold of it:
//!
//! - acceptance claim B: over the correct implementation, exploration "finds no concrete
//!   invariant/refinement violation within declared bounds and emits an exact campaign
//!   envelope";
//! - the scenario's `[properties] check` list: `abstract_register::Agreement`,
//!   `RuntimeToAbstract`, `asupersync::Quiescence` and
//!   `asupersync::ObligationConservation`, over its `[domains]` and `[faults]` bounds;
//! - research/09's promotion criterion: "zero false alarms on the correct
//!   implementation", the other half of detecting the mutants;
//! - PR 16's exit: "each mutant has an expected intent/property and deterministic
//!   campaign", which needs one fixed campaign for the mutants to be run beside.
//!
//! So the correct version here is three things.
//!
//! 1. **The correct protocol**, as a rule on programs: [`discipline`]. The binding has no
//!    data-dependent control flow, so the program's decisions are its scripts'
//!    structure (`replicated_register.rs`, part 1). A script obeys the protocol when it
//!    writes each slot at most once to durable storage, submits only under a permit,
//!    syncs only submitted bytes, releases a permit only after its bytes are durable,
//!    aborts a permit only before bytes, confirms a slot only once it is durable, with
//!    the durable value, and at most once per replica and epoch, re-proposes only a slot
//!    that is not durable, and ends with no permit or volatile bytes in hand. Every
//!    breach is a typed [`Breach`]. The plan also stays inside the scenario's fault
//!    bounds ([`SCENARIO`]).
//! 2. **The correct program with its shutdown**: `register::build_with_shutdown`, so a
//!    run ends quiescent and quiescence can be checked, not assumed.
//! 3. **The baseline campaign** ([`baseline`]): a named, seeded, deterministic set of
//!    correct plans and choice logs at the scenario's bounds, and the four properties
//!    checked on every run ([`execute`]). Its identity is a digest over every plan, every
//!    log and every journal ([`Outcome::digest`]).
//!
//! # The four properties, per run
//!
//! - `abstract_register::Agreement`: no projected state acknowledges two values for one
//!   epoch ([`Finding::Agreement`]);
//! - `RuntimeToAbstract`: every observed step is a durable-register step or a stutter,
//!   every `Ack` an enabled `Choose`, no step breaks durability, every projected state is
//!   reachable, and every acknowledgement has a durable majority under it
//!   (`register::check` and [`Finding::AckedNotDurable`]);
//! - `asupersync::Quiescence`: at the journal's end every task ended, every region,
//!   the root included, is finalized, and every finalization the lift reports is total
//!   with no orphan and no unresolved publication ([`Unsettled`]);
//! - `asupersync::ObligationConservation`: every obligation opened is discharged exactly
//!   once and never leaked, every region settles balanced, and every confirmation sent
//!   is received ([`Unbalanced`]).
//!
//! A run is also a run: the binding does not refuse it, the lift says `Conforms`, the A7
//! model accepts it, and the projection does not refuse it ([`Finding::Refused`] and
//! the others). Those are not scenario properties, and are reported apart.
//!
//! # Scope, stated so no one takes more from it
//!
//! Refinement over the explored runs only. The plan, not the log, fixes which value each
//! incarnation writes and where each crash falls; the log fixes every interleaving. A
//! crashed writer's value is a role-table label the journal does not corroborate. The
//! shutdown runs after every other actor has finished, not concurrently with them; the
//! coordinators never crash; virtual time is not used. Agreement over these plans is a
//! consequence of the discipline (each replica confirms each epoch at most once, and
//! two majorities of three intersect), so no plan of the campaign can even send a
//! majority of confirmations for two values of one epoch. The runtime claims, over
//! interleavings, are refinement, quiescence and conservation. This is a test-level
//! campaign, not the PR-17 refinement checker and not DPOR: most plans' logs are a
//! seeded sample, the one-epoch fates are 17 single-fault fates, and the campaign's
//! identity is a digest, not claim B's campaign envelope artifact. The scenario's
//! `[network]` faults, `max_partitions` and `[storage]` torn tails are not modeled.
//!
//! Every crash of the campaign ([`execute`]) is graceful region cancellation, not a
//! fail-stop crash (`register::CRASH_SEMANTICS`, bn-20d8u). The crashed incarnation's
//! cleanup aborts its permit and unsynced bytes, its tasks end and its region finalizes.
//! So the zero findings of `RuntimeToAbstract`, `asupersync::Quiescence` and
//! `asupersync::ObligationConservation` on plans with crashes rest on that cleanup: the
//! projection reads a crash's `Lose` from the cleanup's aborts, so the refinement map
//! itself is specific to graceful cancellation (IMPL-03 `neg-03-crash-keeps-bytes`). The
//! counts of runs with a crash, such as a crash after an ack, count region cancellations
//! there.
//!
//! # The fail-stop reading (bn-20d8u)
//!
//! [`execute_in`] with `register::CrashMode::FailStop` runs the same campaign with each
//! crash a fail-stop `Crash` (`register::FAIL_STOP_SEMANTICS`). The three runtime
//! properties then read the crash as the process profile states it, and count what it
//! stopped apart, never as a success of the protocol and never hidden:
//!
//! - `RuntimeToAbstract`: the projection reads the crash's `Lose` steps from the
//!   `region-crashed` event, not from any cleanup;
//! - `asupersync::Quiescence`: a task the crash stopped ended, as fenced
//!   ([`Tally::fenced`]); a finalization whose only outstanding obligations are fenced
//!   ones, with no orphan and no unresolved publication, is the crash's, not a failure;
//!   anything else outstanding still fails it;
//! - `asupersync::ObligationConservation`: an obligation opened is discharged exactly
//!   once, or fenced by its holder's crash ([`Tally::fenced`]), and never leaked; a
//!   settle's fenced set is the crash's, and an open or leaked one still fails it.
//!
//! The coordinator counts confirmations: it has no data to tell senders apart. So
//! "count each replica once" (M05's subject) rests on the replica rule
//! [`Breach::ConfirmTwice`], and a restarted replica that sends its confirmation again
//! breaks the protocol here rather than being tolerated by the coordinator.
//!
//! # Hooks for bn-28oa (IMPL-05 mutants)
//!
//! A plan mutant is a [`Campaign`] made from [`baseline`] with [`Campaign::mutated`],
//! which renames it, applies one plan transformation to every plan, and keeps the seed,
//! bounds, groups and log counts; it may use a new [`register::Act`]. A program mutant
//! (a shutdown without a `Finish`, a lost abort, a reused process epoch) is a builder
//! for [`execute_with`], with `register::without_op` to drop an operation and keep the
//! admissibility facts in step. A builder that refuses a plan gives the typed
//! [`Finding::Unbuildable`]. [`execute`] runs the baseline the same way, and [`Outcome`]
//! gives each plan's first [`Finding`] and its property. [`discipline`] names the rule a
//! mutated plan breaks, as the mutant's expected intent. M05 can be written only as a
//! replica script that breaks [`Breach::ConfirmTwice`] (see the scope above).
//!
//! bn-28oa used these hooks: the mutants are in `support/register_mutants.rs`, and
//! [`Tally::timers`] counts the virtual timers the stale-timer mutant sets.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

use continuum_asupersync::binding::{BindingConfig, run};
use continuum_asupersync::choice::ChoiceLog;
use continuum_asupersync::family::channel::ChannelEvent;
use continuum_asupersync::family::effect::EffectEvent;
use continuum_asupersync::family::lifecycle::{LifecycleEvent, TaskStep};
use continuum_asupersync::family::obligation::{Discharge, ObligationEvent};
use continuum_asupersync::family::time::TimeEvent;
use continuum_asupersync::family::{EventBody, Family};
use continuum_asupersync::journal::Journal;
use continuum_asupersync::lift::{LiftVerdict, lift};
use continuum_task::region::obligation::Subject;
use continuum_value::identity::{Blake3Hasher, ContentHasher};

use crate::model::{Alphabet, FamilyTag, judge};
use crate::register::{
    self, Act, Built, Correspondence, Expect, Mismatch, Plan, ProjectionRefusal, Raw, VALUES,
};

// ---------------------------------------------------------------------------
// the scenario's bounds
// ---------------------------------------------------------------------------

/// The bounds a campaign declares: `[domains]` and `[faults]` of a scenario.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Bounds {
    /// Replicas.
    pub nodes: u8,
    /// Values.
    pub values: u8,
    /// Epochs.
    pub epochs: u8,
    /// Crashes per plan, over every replica.
    pub max_crashes: usize,
    /// Cancellations per plan: explicit permit aborts and crashes (a crash cancels a
    /// region: graceful region cancellation, `register::CRASH_SEMANTICS`).
    pub max_cancellations: usize,
}

/// `notes/plan/examples/replicated_register.scenario.toml`'s bounds. The tests read the
/// file and compare.
pub const SCENARIO: Bounds = Bounds {
    nodes: 3,
    values: 2,
    epochs: 2,
    max_crashes: 2,
    max_cancellations: 3,
};

/// The scenario's seed, the baseline campaign's.
pub const SCENARIO_SEED: u64 = 104_729;

/// The scenario's name.
pub const SCENARIO_NAME: &str = "cancel-after-submit-before-sync";

/// The baseline campaign's name.
pub const BASELINE: &str = "pr16-correct-baseline";

// ---------------------------------------------------------------------------
// 1. the correct protocol, as a rule on plans
// ---------------------------------------------------------------------------

/// A rule of the correct protocol that a script breaks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Breach {
    /// An act names an epoch the plan does not have.
    EpochOutOfRange,
    /// A value index outside [`VALUES`].
    ValueOutOfRange,
    /// A reserve while this incarnation already holds a permit or bytes for the slot.
    ReserveHeld,
    /// A reserve or re-proposal for a slot that is already durable: a second write.
    RewriteDurable,
    /// A submit, release or abort with no permit held.
    NoPermit,
    /// A second submit under one permit.
    SubmitTwice,
    /// A sync with no submitted bytes.
    SyncWithoutBytes,
    /// A release before the bytes are durable.
    ReleaseBeforeSync,
    /// An abort after bytes were submitted.
    AbortAfterBytes,
    /// A confirmation of a slot that is not durable: ack-before-sync's premise.
    ConfirmBeforeSync,
    /// A second confirmation of one epoch by one replica, in any incarnation.
    ConfirmTwice,
    /// The script ends holding a permit or volatile bytes.
    UnfinishedAtEnd,
    /// More crashes than the bounds allow.
    CrashBound,
    /// More cancellations than the bounds allow.
    CancellationBound,
}

/// Where a plan breaks the protocol: replica, act index (the script's length for
/// [`Breach::UnfinishedAtEnd`]; `None` for a bound), and the rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Violation {
    /// The replica, `0..3`, or `None` for a plan-wide bound.
    pub replica: Option<usize>,
    /// The act's index in the replica's script.
    pub at: usize,
    /// The rule.
    pub breach: Breach,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Bytes {
    Volatile,
    Durable,
}

/// Whether `plan` is a correct plan within `bounds`: the first rule it breaks, or
/// `Ok(())`.
///
/// # Errors
///
/// The first [`Violation`], replica by replica, act by act, then the bounds.
pub fn discipline(plan: &Plan, bounds: Bounds) -> Result<(), Violation> {
    let nvalues = u8::try_from(VALUES.len()).expect("two values");
    if plan.epochs == 0 || plan.epochs > bounds.epochs {
        return Err(Violation {
            replica: None,
            at: 0,
            breach: Breach::EpochOutOfRange,
        });
    }
    let (mut crashes, mut aborts) = (0, 0);
    for n in 0..3 {
        let fail = |at: usize, breach: Breach| Violation {
            replica: Some(n),
            at,
            breach,
        };
        if plan.values[n].iter().any(|v| *v >= nvalues) {
            return Err(fail(0, Breach::ValueOutOfRange));
        }
        let mut durable: BTreeSet<u8> = BTreeSet::new();
        let mut confirmed: BTreeSet<u8> = BTreeSet::new();
        let mut permit: BTreeSet<u8> = BTreeSet::new();
        let mut bytes: BTreeMap<u8, Bytes> = BTreeMap::new();
        for (at, act) in plan.replicas[n].iter().enumerate() {
            let epoch = match *act {
                Act::Reserve(e)
                | Act::Submit(e)
                | Act::Sync(e)
                | Act::Release(e)
                | Act::Abort(e)
                | Act::Confirm(e)
                | Act::CrashRepropose(e, _) => Some(e),
                Act::Crash => None,
            };
            if epoch.is_some_and(|e| e >= plan.epochs) {
                return Err(fail(at, Breach::EpochOutOfRange));
            }
            match *act {
                Act::Reserve(e) => {
                    if durable.contains(&e) {
                        return Err(fail(at, Breach::RewriteDurable));
                    }
                    if permit.contains(&e) || bytes.contains_key(&e) {
                        return Err(fail(at, Breach::ReserveHeld));
                    }
                    permit.insert(e);
                }
                Act::Submit(e) => {
                    if !permit.contains(&e) {
                        return Err(fail(at, Breach::NoPermit));
                    }
                    if bytes.contains_key(&e) {
                        return Err(fail(at, Breach::SubmitTwice));
                    }
                    bytes.insert(e, Bytes::Volatile);
                }
                Act::Sync(e) => {
                    if bytes.get(&e) != Some(&Bytes::Volatile) {
                        return Err(fail(at, Breach::SyncWithoutBytes));
                    }
                    bytes.insert(e, Bytes::Durable);
                    durable.insert(e);
                }
                Act::Release(e) => {
                    if !permit.contains(&e) {
                        return Err(fail(at, Breach::NoPermit));
                    }
                    if bytes.get(&e) != Some(&Bytes::Durable) {
                        return Err(fail(at, Breach::ReleaseBeforeSync));
                    }
                    permit.remove(&e);
                }
                Act::Abort(e) => {
                    if !permit.contains(&e) {
                        return Err(fail(at, Breach::NoPermit));
                    }
                    if bytes.contains_key(&e) {
                        return Err(fail(at, Breach::AbortAfterBytes));
                    }
                    permit.remove(&e);
                    aborts += 1;
                }
                Act::Confirm(e) => {
                    // The confirmation goes to the coordinator of the incarnation's
                    // value, which is the durable one: a durable slot's value never
                    // changes, since a re-proposal of it is `RewriteDurable`.
                    if !durable.contains(&e) {
                        return Err(fail(at, Breach::ConfirmBeforeSync));
                    }
                    if !confirmed.insert(e) {
                        return Err(fail(at, Breach::ConfirmTwice));
                    }
                }
                Act::Crash | Act::CrashRepropose(..) => {
                    if let Act::CrashRepropose(e, v) = *act {
                        if v >= nvalues {
                            return Err(fail(at, Breach::ValueOutOfRange));
                        }
                        if durable.contains(&e) {
                            return Err(fail(at, Breach::RewriteDurable));
                        }
                    }
                    permit.clear();
                    bytes.clear();
                    crashes += 1;
                }
            }
        }
        if !permit.is_empty() || bytes.values().any(|b| *b == Bytes::Volatile) {
            return Err(fail(plan.replicas[n].len(), Breach::UnfinishedAtEnd));
        }
    }
    let bound = |breach| Violation {
        replica: None,
        at: 0,
        breach,
    };
    if crashes > bounds.max_crashes {
        return Err(bound(Breach::CrashBound));
    }
    if crashes + aborts > bounds.max_cancellations {
        return Err(bound(Breach::CancellationBound));
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// the correct plans: fates
// ---------------------------------------------------------------------------

/// What happens to one replica's write of one slot, in the correct program.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Fate {
    /// The replica gets no proposal for the slot.
    Idle,
    /// Reserve, submit, sync, release, confirm.
    Clean,
    /// The writer is cancelled before it submits and releases its permit (`Abort`),
    /// then writes.
    AbortRetry,
    /// A crash while the permit is held; the next incarnation writes `retry`.
    CrashReserved {
        /// The value the next incarnation writes.
        retry: u8,
    },
    /// A crash after the bytes are submitted and before `sync`: the scenario's
    /// `inject_cancellation_after = "storage.append:Submitted"`. The volatile bytes are
    /// lost, and the next incarnation writes `retry`.
    CrashSubmitted {
        /// The value the next incarnation writes.
        retry: u8,
    },
    /// A crash after `sync` and before the confirmation; the next incarnation confirms
    /// the durable record and writes nothing.
    CrashSynced,
    /// A crash after the confirmation: the scenario's `crash_after =
    /// "client.reply:Committed"` when the ack precedes it in the run.
    CrashReplied,
}

impl Fate {
    /// Whether the fate crashes the replica.
    #[must_use]
    pub const fn crashes(self) -> bool {
        matches!(
            self,
            Self::CrashReserved { .. }
                | Self::CrashSubmitted { .. }
                | Self::CrashSynced
                | Self::CrashReplied
        )
    }

    /// A stable token.
    #[must_use]
    pub fn token(self) -> String {
        match self {
            Self::Idle => "idle".to_owned(),
            Self::Clean => "clean".to_owned(),
            Self::AbortRetry => "abort-retry".to_owned(),
            Self::CrashReserved { retry } => {
                format!("crash-reserved->{}", VALUES[usize::from(retry)])
            }
            Self::CrashSubmitted { retry } => {
                format!("crash-submitted->{}", VALUES[usize::from(retry)])
            }
            Self::CrashSynced => "crash-synced".to_owned(),
            Self::CrashReplied => "crash-replied".to_owned(),
        }
    }

    /// The same fate with the values swapped.
    #[must_use]
    const fn swapped(self) -> Self {
        match self {
            Self::CrashReserved { retry } => Self::CrashReserved { retry: 1 - retry },
            Self::CrashSubmitted { retry } => Self::CrashSubmitted { retry: 1 - retry },
            other => other,
        }
    }
}

/// The acts of `fate` at `epoch` for an incarnation that proposes `value` there.
#[must_use]
pub fn fate_acts(epoch: u8, value: u8, fate: Fate) -> Vec<Act> {
    let crash = |retry: u8| {
        if retry == value {
            Act::Crash
        } else {
            Act::CrashRepropose(epoch, retry)
        }
    };
    let mut out = Vec::new();
    match fate {
        Fate::Idle => {}
        Fate::Clean => out.extend(register::write(epoch)),
        Fate::AbortRetry => {
            out.extend([Act::Reserve(epoch), Act::Abort(epoch)]);
            out.extend(register::write(epoch));
        }
        Fate::CrashReserved { retry } => {
            out.extend([Act::Reserve(epoch), crash(retry)]);
            out.extend(register::write(epoch));
        }
        Fate::CrashSubmitted { retry } => {
            out.extend([Act::Reserve(epoch), Act::Submit(epoch), crash(retry)]);
            out.extend(register::write(epoch));
        }
        Fate::CrashSynced => out.extend([
            Act::Reserve(epoch),
            Act::Submit(epoch),
            Act::Sync(epoch),
            Act::Crash,
            Act::Confirm(epoch),
        ]),
        Fate::CrashReplied => {
            out.extend(register::write(epoch));
            out.push(Act::Crash);
        }
    }
    out
}

/// One replica of a correct plan: its first incarnation's values by epoch, and the
/// fate of each slot, in the order the replica writes them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Replica {
    /// Values by epoch.
    pub values: [u8; 2],
    /// `(epoch, fate)` in write order.
    pub slots: [(u8, Fate); 2],
    /// How many of `slots` the plan has: its epochs.
    pub epochs: u8,
}

impl Replica {
    fn script(&self) -> Vec<Act> {
        let mut values = self.values;
        let mut out = Vec::new();
        for &(e, fate) in &self.slots[..usize::from(self.epochs)] {
            out.extend(fate_acts(e, values[usize::from(e)], fate));
            if let Fate::CrashReserved { retry } | Fate::CrashSubmitted { retry } = fate {
                values[usize::from(e)] = retry;
            }
        }
        out
    }

    fn crashes(&self) -> usize {
        self.slots[..usize::from(self.epochs)]
            .iter()
            .filter(|(_, f)| f.crashes())
            .count()
    }

    fn cancellations(&self) -> usize {
        self.crashes()
            + self.slots[..usize::from(self.epochs)]
                .iter()
                .filter(|(_, f)| *f == Fate::AbortRetry)
                .count()
    }
}

/// The plan of three replicas.
#[must_use]
pub fn plan_of(name: &'static str, epochs: u8, replicas: [Replica; 3]) -> Plan {
    Plan {
        name,
        epochs,
        values: replicas.map(|r| r.values),
        replicas: replicas.map(|r| r.script()),
    }
}

/// Every fate at one epoch, with the value the replica proposes, once per value; `Idle`
/// once, with value `v0`.
#[must_use]
pub fn one_epoch_options() -> Vec<(u8, Fate)> {
    let mut out = vec![(0, Fate::Idle)];
    for v in 0..2 {
        out.push((v, Fate::Clean));
        out.push((v, Fate::AbortRetry));
        for retry in 0..2 {
            out.push((v, Fate::CrashReserved { retry }));
            out.push((v, Fate::CrashSubmitted { retry }));
        }
        out.push((v, Fate::CrashSynced));
        out.push((v, Fate::CrashReplied));
    }
    out
}

/// One replica of a one-epoch plan.
#[must_use]
pub fn one_epoch_replica((value, fate): (u8, Fate)) -> Replica {
    Replica {
        values: [value, 0],
        slots: [(0, fate), (1, Fate::Idle)],
        epochs: 1,
    }
}

/// The one-epoch sweep: one plan per orbit of the replica configurations under
/// renaming the replicas and swapping the two values, with at most
/// `bounds.max_crashes` crashes. Each replica is `(value, fate)` over every [`Fate`].
#[must_use]
pub fn one_epoch_sweep(bounds: Bounds) -> Vec<Plan> {
    one_epoch_sweep_replicas(bounds)
        .into_iter()
        .map(|r| plan_of("sweep-one-epoch", 1, r))
        .collect()
}

/// The replicas of each plan of [`one_epoch_sweep`], in its order: the configuration a
/// scenario reduction starts from (PR 18, bn-25z9o).
#[must_use]
pub fn one_epoch_sweep_replicas(bounds: Bounds) -> Vec<[Replica; 3]> {
    let options = one_epoch_options();
    let index = |o: (u8, Fate)| options.iter().position(|x| *x == o).expect("an option");
    let swap = |(v, f): (u8, Fate)| -> (u8, Fate) {
        if f == Fate::Idle {
            (0, Fate::Idle)
        } else {
            (1 - v, f.swapped())
        }
    };
    let mut out = Vec::new();
    for i in 0..options.len() {
        for j in i..options.len() {
            for k in j..options.len() {
                let triple = [options[i], options[j], options[k]];
                let crashes = triple.iter().filter(|(_, f)| f.crashes()).count();
                if crashes > bounds.max_crashes {
                    continue;
                }
                let mut swapped = triple.map(|o| index(swap(o)));
                swapped.sort_unstable();
                if swapped < [i, j, k] {
                    continue;
                }
                out.push(triple.map(one_epoch_replica));
            }
        }
    }
    out
}

/// SplitMix64: the explicit, seeded source of the sampled two-epoch plans and of each
/// plan's log seed (INV-005).
struct SplitMix(u64);

impl SplitMix {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }

    fn below(&mut self, n: usize) -> usize {
        usize::try_from(self.next() % u64::try_from(n).expect("small")).expect("small")
    }
}

/// `count` distinct correct two-epoch plans drawn from `seed`: each replica writes both
/// epochs in a drawn order, with drawn values and a drawn fate per slot, and the plan
/// stays within `bounds`. Every plan satisfies [`discipline`].
///
/// # Panics
///
/// When `count` distinct plans are not found within `64 * count` draws.
#[must_use]
pub fn two_epoch_sample(bounds: Bounds, count: usize, seed: u64) -> Vec<Plan> {
    let fates = one_epoch_options();
    let mut rng = SplitMix(seed);
    let mut seen = BTreeSet::new();
    let mut out = Vec::new();
    let mut draws = 0;
    while out.len() < count {
        draws += 1;
        assert!(draws <= 64 * count, "too few distinct two-epoch plans");
        let replicas = [(); 3].map(|()| {
            let first = u8::try_from(rng.below(2)).expect("0 or 1");
            let values = [0, 1].map(|_| u8::try_from(rng.below(2)).expect("0 or 1"));
            let fate = |rng: &mut SplitMix| fates[rng.below(fates.len())].1;
            let slots = [(first, fate(&mut rng)), (1 - first, fate(&mut rng))];
            Replica {
                values,
                slots,
                epochs: 2,
            }
        });
        let crashes: usize = replicas.iter().map(Replica::crashes).sum();
        let cancellations: usize = replicas.iter().map(Replica::cancellations).sum();
        if crashes > bounds.max_crashes || cancellations > bounds.max_cancellations {
            continue;
        }
        if seen.insert(replicas) {
            out.push(plan_of("sample-two-epochs", 2, replicas));
        }
    }
    out
}

/// The scenario's own plan, `cancel-after-submit-before-sync`: `a` proposes `v0`, is
/// cancelled after its bytes are submitted and before `sync`, and its next incarnation
/// is proposed `v1`; `b` writes `v0`, `c` writes `v1`. This is M01's causal core
/// (`replicated_register.md`) run by the correct program: `a` confirms nothing before
/// its crash, so only `v1` ever gets a majority.
#[must_use]
pub fn scenario_plan() -> Plan {
    plan_of(SCENARIO_NAME, 1, scenario_replicas())
}

/// The replicas of [`scenario_plan`]: the configuration a scenario reduction starts from
/// (PR 18, bn-25z9o).
#[must_use]
pub fn scenario_replicas() -> [Replica; 3] {
    [
        one_epoch_replica((0, Fate::CrashSubmitted { retry: 1 })),
        one_epoch_replica((0, Fate::Clean)),
        one_epoch_replica((1, Fate::Clean)),
    ]
}

/// `crash-after-reply`: every replica writes `v0`, and `c` crashes after its
/// confirmation; in the runs where the ack precedes that crash it is the scenario's
/// `crash_after = "client.reply:Committed"`.
#[must_use]
pub fn crash_after_reply_plan() -> Plan {
    plan_of(
        "crash-after-reply",
        1,
        [
            one_epoch_replica((0, Fate::Clean)),
            one_epoch_replica((0, Fate::Clean)),
            one_epoch_replica((0, Fate::CrashReplied)),
        ],
    )
}

// ---------------------------------------------------------------------------
// 3. the campaign
// ---------------------------------------------------------------------------

/// One group of plans, with how many logs each gets.
#[derive(Debug, Clone)]
pub struct Group {
    /// A stable ID.
    pub id: &'static str,
    /// The plans.
    pub plans: Vec<Plan>,
    /// A plan with at most this many admissible logs runs all of them; otherwise it
    /// runs this many, sampled.
    pub logs: usize,
}

/// A named, seeded campaign.
#[derive(Debug, Clone)]
pub struct Campaign {
    /// A stable name.
    pub name: &'static str,
    /// The seed every sampled log is drawn from.
    pub seed: u64,
    /// The declared bounds.
    pub bounds: Bounds,
    /// The groups, in order.
    pub groups: Vec<Group>,
}

/// Plans in the two-epoch sample.
pub const TWO_EPOCH_PLANS: usize = 48;
/// Logs per sweep plan.
pub const SWEEP_LOGS: usize = 6;
/// Logs per sampled two-epoch plan.
pub const TWO_EPOCH_LOGS: usize = 6;
/// Logs for each named scenario plan.
pub const SCENARIO_LOGS: usize = 400;

/// The baseline campaign: the correct version's runs, the ones every mutant is run
/// beside. Four groups, all at [`SCENARIO`]'s bounds and seed:
///
/// - `scenario`: [`scenario_plan`] and [`crash_after_reply_plan`], [`SCENARIO_LOGS`]
///   logs each;
/// - `sweep-one-epoch`: [`one_epoch_sweep`], every orbit, [`SWEEP_LOGS`] logs each;
/// - `sample-two-epochs`: [`two_epoch_sample`], [`TWO_EPOCH_PLANS`] plans drawn from the
///   seed, [`TWO_EPOCH_LOGS`] logs each: claim A's two epochs.
#[must_use]
pub fn baseline() -> Campaign {
    Campaign {
        name: BASELINE,
        seed: SCENARIO_SEED,
        bounds: SCENARIO,
        groups: vec![
            Group {
                id: "scenario",
                plans: vec![scenario_plan(), crash_after_reply_plan()],
                logs: SCENARIO_LOGS,
            },
            Group {
                id: "sweep-one-epoch",
                plans: one_epoch_sweep(SCENARIO),
                logs: SWEEP_LOGS,
            },
            Group {
                id: "sample-two-epochs",
                plans: two_epoch_sample(SCENARIO, TWO_EPOCH_PLANS, SCENARIO_SEED),
                logs: TWO_EPOCH_LOGS,
            },
        ],
    }
}

impl Campaign {
    /// The same campaign under the name `name`, with `f` applied to every plan: a
    /// mutant's campaign. Seed, bounds, groups and log counts are the baseline's. A
    /// mutant of the program rather than the plan is run with [`execute_with`].
    #[must_use]
    pub fn mutated(&self, name: &'static str, f: impl Fn(&Plan) -> Plan) -> Self {
        let mut out = self.clone();
        out.name = name;
        for g in &mut out.groups {
            g.plans = g.plans.iter().map(&f).collect();
        }
        out
    }

    /// The log seed of plan `plan` of group `group`: a function of the campaign seed and
    /// the position alone.
    #[must_use]
    pub fn plan_seed(&self, group: usize, plan: usize) -> u64 {
        let g = u64::try_from(group).expect("few groups");
        let p = u64::try_from(plan).expect("few plans");
        SplitMix(self.seed ^ (g << 48) ^ p).next()
    }
}

/// How a plan's logs were chosen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    /// Every admissible log.
    Exhaustive,
    /// A seeded sample.
    Sampled {
        /// The seed.
        seed: u64,
    },
}

/// A plan's logs: all of them when there are at most `count`, otherwise `count` drawn
/// from `seed`.
#[must_use]
pub fn logs_for(built: &Built, count: usize, seed: u64) -> (Scope, Vec<ChoiceLog>) {
    match register::admissible_logs(built, count) {
        Some(all) => (Scope::Exhaustive, all),
        None => (
            Scope::Sampled { seed },
            register::sample_logs(built, count, seed),
        ),
    }
}

// ---------------------------------------------------------------------------
// 2. quiescence and conservation, read from the journal
// ---------------------------------------------------------------------------

/// Why a run did not end quiescent (`asupersync::Quiescence`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Unsettled {
    /// A task spawned and never ended: no `complete`, `fail` or `cancel` step, and in
    /// no drain's cancelled set.
    TaskLive(u32),
    /// A task ended by failing: it ended, but not as the protocol ends a task.
    TaskFailed(u32),
    /// A region, the root (`0`) included, opened and never finalized.
    RegionUnfinalized(u32),
    /// A finalization the lift reports is not total, or leaves an orphan or an
    /// unresolved publication.
    FinalizationNotTotal(u32),
    /// The lift did not conform, so its finalizations are not there to read.
    NoLift,
}

/// Why a run did not conserve its obligations (`asupersync::ObligationConservation`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Unbalanced {
    /// An obligation opened and never discharged.
    ObligationOpen(u32),
    /// An obligation leaked.
    ObligationLeaked(u32),
    /// A discharge of an obligation that is not open: twice, or never opened.
    DischargedTwice(u32),
    /// A region settled with open or leaked obligations.
    SettledWithObligations(u32),
    /// A confirmation sent and never received: `(channel, message)`.
    MessageUnreceived(u32, u64),
}

/// Counts over one journal, for the evidence.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Tally {
    /// Obligations opened, by kind token.
    pub opened: BTreeMap<String, usize>,
    /// Discharges committed.
    pub committed: usize,
    /// Discharges aborted.
    pub aborted: usize,
    /// Tasks spawned.
    pub tasks: usize,
    /// Tasks that ended with a `complete` or `cancel` step.
    pub completed: usize,
    /// Tasks that ended cancelled, in a drain.
    pub cancelled: usize,
    /// Regions finalized, the root included.
    pub finalized: usize,
    /// Messages sent.
    pub sent: usize,
    /// Virtual timers scheduled, fired, cancelled, and fired after their task's region
    /// was cancelled: a timer of an old epoch firing in the new one. The correct program
    /// sets none; the IMPL-05 stale-timer mutant M08 does (bn-28oa).
    pub timers: [usize; 4],
    /// What fail-stop crashes stopped and fenced, counted apart (bn-20d8u): tasks,
    /// obligations, reservations and timers.
    pub fenced: [usize; 4],
}

impl Tally {
    fn absorb(&mut self, other: &Self) {
        for (k, v) in &other.opened {
            *self.opened.entry(k.clone()).or_default() += v;
        }
        self.committed += other.committed;
        self.aborted += other.aborted;
        self.tasks += other.tasks;
        self.completed += other.completed;
        self.cancelled += other.cancelled;
        self.finalized += other.finalized;
        self.sent += other.sent;
        for (a, b) in self.timers.iter_mut().zip(other.timers) {
            *a += b;
        }
        for (a, b) in self.fenced.iter_mut().zip(other.fenced) {
            *a += b;
        }
    }
}

/// Quiescence and conservation of one journal, with the lift's verdict on it.
#[must_use]
pub fn settle(
    journal: &Journal,
    verdict: &LiftVerdict,
) -> (Vec<Unsettled>, Vec<Unbalanced>, Tally) {
    let mut tally = Tally::default();
    let mut unbalanced = Vec::new();
    let mut live = BTreeSet::new();
    let mut failed = Vec::new();
    let mut regions = BTreeSet::from([0_u32]);
    let mut finalized = BTreeSet::new();
    let mut open = BTreeSet::new();
    let mut sent = BTreeSet::new();
    let mut received = BTreeSet::new();
    let mut region_of: BTreeMap<u32, u32> = BTreeMap::new();
    let mut timer_task: BTreeMap<u32, u32> = BTreeMap::new();
    let mut cancelled_regions = BTreeSet::new();
    let mut fenced_obligations = BTreeSet::new();
    let mut parent_of: BTreeMap<u32, u32> = BTreeMap::new();
    for event in journal.events() {
        match event.body() {
            // A fail-stop crash (bn-20d8u): the stopped tasks ended, counted apart.
            EventBody::Lifecycle(LifecycleEvent::RegionCrashed { region, fenced }) => {
                // The whole crashed subtree, so a stale fire in a subregion counts too.
                for r in regions.iter().copied().collect::<Vec<_>>() {
                    let mut at = Some(r);
                    while let Some(x) = at {
                        if x == region.0 {
                            cancelled_regions.insert(r);
                            break;
                        }
                        at = parent_of.get(&x).copied();
                    }
                }
                for t in fenced.as_slice() {
                    tally.fenced[0] += 1;
                    live.remove(&t.0);
                }
            }
            EventBody::Obligation(ObligationEvent::Fenced { obligation }) => {
                tally.fenced[1] += 1;
                if !open.remove(&obligation.0) {
                    unbalanced.push(Unbalanced::DischargedTwice(obligation.0));
                }
                fenced_obligations.insert(u64::from(obligation.0));
            }
            EventBody::Effect(EffectEvent::Fenced { .. }) => tally.fenced[2] += 1,
            EventBody::Time(TimeEvent::Fenced { .. }) => tally.fenced[3] += 1,
            EventBody::Lifecycle(LifecycleEvent::TaskSpawned { task, region, .. }) => {
                tally.tasks += 1;
                live.insert(task.0);
                region_of.insert(task.0, region.0);
            }
            EventBody::Lifecycle(LifecycleEvent::RegionCancelRequested { region }) => {
                cancelled_regions.insert(region.0);
            }
            EventBody::Lifecycle(LifecycleEvent::TaskStepped { task, step }) => match step {
                TaskStep::Complete | TaskStep::Cancel => {
                    tally.completed += 1;
                    live.remove(&task.0);
                }
                TaskStep::Fail(_) => {
                    failed.push(task.0);
                    live.remove(&task.0);
                }
                _ => {}
            },
            EventBody::Lifecycle(LifecycleEvent::RegionDrained { cancelled, .. }) => {
                for t in cancelled.as_slice() {
                    tally.cancelled += 1;
                    live.remove(&t.0);
                }
            }
            EventBody::Lifecycle(LifecycleEvent::RegionOpened { region, parent }) => {
                regions.insert(region.0);
                parent_of.insert(region.0, parent.0);
            }
            EventBody::Lifecycle(LifecycleEvent::RegionFinalized { region }) => {
                tally.finalized += 1;
                finalized.insert(region.0);
            }
            EventBody::Obligation(ObligationEvent::Opened {
                obligation, kind, ..
            }) => {
                *tally.opened.entry(format!("{kind:?}")).or_default() += 1;
                open.insert(obligation.0);
            }
            EventBody::Obligation(ObligationEvent::Discharged { obligation, how }) => {
                if !open.remove(&obligation.0) {
                    unbalanced.push(Unbalanced::DischargedTwice(obligation.0));
                }
                if *how == Discharge::Committed {
                    tally.committed += 1;
                } else {
                    tally.aborted += 1;
                }
            }
            EventBody::Obligation(ObligationEvent::Leaked { obligation }) => {
                open.remove(&obligation.0);
                unbalanced.push(Unbalanced::ObligationLeaked(obligation.0));
            }
            EventBody::Obligation(ObligationEvent::RegionSettled {
                region,
                open: o,
                leaked,
                ..
            }) => {
                if !o.is_empty() || !leaked.is_empty() {
                    unbalanced.push(Unbalanced::SettledWithObligations(region.0));
                }
            }
            EventBody::Channel(ChannelEvent::Sent {
                channel, message, ..
            }) => {
                tally.sent += 1;
                sent.insert((channel.0, message.0));
            }
            EventBody::Channel(ChannelEvent::Received { channel, message }) => {
                received.insert((channel.0, message.0));
            }
            EventBody::Time(TimeEvent::Scheduled { timer, task, .. }) => {
                tally.timers[0] += 1;
                timer_task.insert(timer.0, task.0);
            }
            EventBody::Time(TimeEvent::Fired { timer, .. }) => {
                tally.timers[1] += 1;
                let region = timer_task.get(&timer.0).and_then(|t| region_of.get(t));
                if region.is_some_and(|r| cancelled_regions.contains(r)) {
                    tally.timers[3] += 1;
                }
            }
            EventBody::Time(TimeEvent::Cancelled { .. }) => tally.timers[2] += 1,
            _ => {}
        }
    }
    unbalanced.extend(open.iter().map(|o| Unbalanced::ObligationOpen(*o)));
    unbalanced.extend(
        sent.difference(&received)
            .map(|(c, m)| Unbalanced::MessageUnreceived(*c, *m)),
    );
    let mut unsettled: Vec<Unsettled> = live.iter().map(|t| Unsettled::TaskLive(*t)).collect();
    unsettled.extend(failed.into_iter().map(Unsettled::TaskFailed));
    unsettled.extend(
        regions
            .difference(&finalized)
            .map(|r| Unsettled::RegionUnfinalized(*r)),
    );
    match verdict {
        LiftVerdict::Conforms(lifted) => {
            for (_, f) in lifted.finalizations() {
                // What a crash fenced stays owed in the calculus's ledger: a finalization
                // whose only outstanding obligations are fenced ones is the crash's.
                let only_fenced = f.ledger().outstanding().iter().all(|o| {
                    matches!(o.subject(), Subject::Substrate(id) if fenced_obligations.contains(&id.ordinal()))
                });
                if !(f.is_total() || only_fenced)
                    || !f.orphans().is_empty()
                    || !f.unresolved_publications().is_empty()
                {
                    unsettled.push(Unsettled::FinalizationNotTotal(f.region().ordinal()));
                }
            }
        }
        _ => unsettled.push(Unsettled::NoLift),
    }
    (unsettled, unbalanced, tally)
}

// ---------------------------------------------------------------------------
// the run and its findings
// ---------------------------------------------------------------------------

/// A scenario property, or the run itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Property {
    /// The run is a run: the binding, the lift, the A7 model and the projection accept it.
    Run,
    /// `abstract_register::Agreement`.
    Agreement,
    /// `RuntimeToAbstract`.
    RuntimeToAbstract,
    /// `asupersync::Quiescence`.
    Quiescence,
    /// `asupersync::ObligationConservation`.
    ObligationConservation,
}

impl Property {
    /// The scenario's name for it; `None` for [`Property::Run`].
    #[must_use]
    pub const fn scenario_name(self) -> Option<&'static str> {
        match self {
            Self::Run => None,
            Self::Agreement => Some("abstract_register::Agreement"),
            Self::RuntimeToAbstract => Some("RuntimeToAbstract"),
            Self::Quiescence => Some("asupersync::Quiescence"),
            Self::ObligationConservation => Some("asupersync::ObligationConservation"),
        }
    }

    /// The four scenario properties, in the scenario's order.
    pub const CHECKED: [Self; 4] = [
        Self::Agreement,
        Self::RuntimeToAbstract,
        Self::Quiescence,
        Self::ObligationConservation,
    ];
}

/// One thing wrong with one run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Finding {
    /// The builder refused the plan: why. The plan has no runs.
    Unbuildable(String),
    /// The binding refused the log: rendered refusal.
    Refused(String),
    /// The lift did not conform: rendered verdict.
    LiftNot(String),
    /// The A7 model rejected the journal.
    A7Rejected,
    /// The projection refused the journal.
    ProjectionRefused(ProjectionRefusal),
    /// A projected state acknowledges two values for one epoch, at this event.
    Agreement(u64),
    /// A step fails the refinement check: `register::check`'s mismatch, at this event.
    Refinement(Mismatch, u64),
    /// A projected state has an acknowledgement with no durable majority, at this
    /// event.
    AckedNotDurable(u64),
    /// Not quiescent at the end.
    Quiescence(Unsettled),
    /// Not conserved.
    Conservation(Unbalanced),
}

impl Finding {
    /// The property the finding refutes.
    #[must_use]
    pub const fn property(&self) -> Property {
        match self {
            Self::Unbuildable(_)
            | Self::Refused(_)
            | Self::LiftNot(_)
            | Self::A7Rejected
            | Self::ProjectionRefused(_) => Property::Run,
            Self::Agreement(_) => Property::Agreement,
            Self::Refinement(..) | Self::AckedNotDurable(_) => Property::RuntimeToAbstract,
            Self::Quiescence(_) => Property::Quiescence,
            Self::Conservation(_) => Property::ObligationConservation,
        }
    }
}

/// Everything about one run.
#[derive(Debug, Clone)]
pub struct RunReport {
    /// What is wrong, in check order; empty for a good run.
    pub findings: Vec<Finding>,
    /// The journal's digest, or the refusal's rendering.
    pub digest: String,
    /// The step correspondence.
    pub correspondence: Correspondence,
    /// Quiescence and conservation counts.
    pub tally: Tally,
    /// Whether a volatile write was lost: a cancellation after `Submitted`, before
    /// `sync`.
    pub cancel_after_submit: bool,
    /// Whether a replica region's cancellation follows an `Ack` in the journal.
    pub crash_after_ack: bool,
    /// Whether some projected state holds both values durable in one epoch.
    pub contested: bool,
    /// A carried build's payloads, read from the journal (`register::carried_in_journal`,
    /// bn-2faf1): received, and received and then confirmed by the receiver.
    pub carried: [usize; 2],
}

/// Whether some epoch has two acknowledged values: `abstract_register::Agreement`
/// fails on this state.
#[must_use]
pub fn disagrees(raw: &Raw) -> bool {
    let acks = register::acks_of(raw);
    acks.iter()
        .any(|(e, v)| acks.iter().any(|(f, w)| e == f && v != w))
}

/// Whether some acknowledgement has fewer than two replicas holding it durably:
/// IMPL-02's `AckedIsDurable` fails on this state.
#[must_use]
pub fn ack_without_majority(raw: &Raw) -> bool {
    let durable = register::durable_of(raw);
    register::acks_of(raw).iter().any(|&(e, v)| {
        durable
            .iter()
            .filter(|&&(_, f, w)| (f, w) == (e, v))
            .count()
            < 2
    })
}

fn both_values_durable(raw: &Raw) -> bool {
    let durable = register::durable_of(raw);
    durable
        .iter()
        .any(|&(_, e, v)| durable.iter().any(|&(_, f, w)| e == f && v != w))
}

/// The binding configuration: every family observed, lab seed `0` (the seed does not
/// reach the journal; `pr16_impl03_replicated_register.rs` shows it).
#[must_use]
pub fn config() -> BindingConfig {
    BindingConfig::new(0)
        .observing(Family::Effect)
        .observing(Family::Cancellation)
        .observing(Family::Obligation)
        .observing(Family::Time)
        .observing(Family::Channel)
}

fn alphabet() -> Alphabet {
    Alphabet::new([
        FamilyTag::Lifecycle,
        FamilyTag::Effect,
        FamilyTag::Cancellation,
        FamilyTag::Obligation,
        FamilyTag::Time,
        FamilyTag::Channel,
    ])
}

/// Run one log of `built` and check it. `reachable` is `register::reachable` at
/// `epochs`.
#[must_use]
pub fn run_one(built: &Built, epochs: u8, log: &ChoiceLog, reachable: &BTreeSet<Raw>) -> RunReport {
    let mut report = RunReport {
        findings: Vec::new(),
        digest: String::new(),
        correspondence: Correspondence::default(),
        tally: Tally::default(),
        cancel_after_submit: false,
        crash_after_ack: false,
        contested: false,
        carried: [0; 2],
    };
    let journal = match run(&built.programs, log, &config()) {
        Ok(j) => j,
        Err(r) => {
            report.digest = format!("refused: {r}");
            report.findings.push(Finding::Refused(r.to_string()));
            return report;
        }
    };
    report.digest = journal.digest().expect("digests").to_string();
    for d in register::carried_in_journal(&built.roles, &journal)
        .into_iter()
        .flatten()
    {
        report.carried[0] += 1;
        report.carried[1] += usize::from(d);
    }
    let verdict = lift(&journal);
    if !matches!(verdict, LiftVerdict::Conforms(_)) {
        report
            .findings
            .push(Finding::LiftNot(format!("{verdict:?}")));
    }
    if !judge(&alphabet(), &journal.encode().expect("encodes")).is_accepted() {
        report.findings.push(Finding::A7Rejected);
    }
    match register::observe(&built.roles, &journal) {
        Err(r) => report.findings.push(Finding::ProjectionRefused(r)),
        Ok(steps) => {
            let mut first_ack = None;
            for s in &steps {
                if let Some(post) = &s.post {
                    if disagrees(post) {
                        report.findings.push(Finding::Agreement(s.seq));
                    }
                    if ack_without_majority(post) {
                        report.findings.push(Finding::AckedNotDurable(s.seq));
                    }
                    report.contested |= both_values_durable(post);
                }
                if first_ack.is_none()
                    && matches!(&s.expect, Expect::Step(l) if l.starts_with("Ack("))
                {
                    first_ack = Some(s.seq);
                }
            }
            report.crash_after_ack = first_ack.is_some_and(|at| {
                journal.events().iter().any(|e| {
                    e.seq() > at
                        && matches!(
                            e.body(),
                            EventBody::Lifecycle(
                                LifecycleEvent::RegionCancelRequested { .. }
                                    | LifecycleEvent::RegionCrashed { .. }
                            )
                        )
                })
            });
            let c = register::check(&steps, epochs, reachable);
            report.findings.extend(
                c.failures
                    .iter()
                    .map(|(m, seq, _)| Finding::Refinement(*m, *seq)),
            );
            report.cancel_after_submit = c.kinds.contains_key("Lose/volatile");
            report.correspondence = c;
        }
    }
    let (unsettled, unbalanced, tally) = settle(&journal, &verdict);
    report
        .findings
        .extend(unsettled.into_iter().map(Finding::Quiescence));
    report
        .findings
        .extend(unbalanced.into_iter().map(Finding::Conservation));
    report.tally = tally;
    report
}

/// One plan's results.
#[derive(Debug, Clone)]
pub struct PlanOutcome {
    /// The group's ID.
    pub group: &'static str,
    /// The plan's index in its group.
    pub index: usize,
    /// How its logs were chosen.
    pub scope: Scope,
    /// Its runs.
    pub runs: usize,
    /// Each run's findings, by log index, for the runs that have any.
    pub findings: Vec<(usize, Vec<Finding>)>,
}

impl PlanOutcome {
    /// The first finding of the first failing run.
    #[must_use]
    pub fn first(&self) -> Option<&Finding> {
        self.findings.first().and_then(|(_, f)| f.first())
    }
}

/// A campaign's results.
#[derive(Debug, Clone, Default)]
pub struct Outcome {
    /// Per plan, in campaign order.
    pub plans: Vec<PlanOutcome>,
    /// Runs.
    pub runs: usize,
    /// The step correspondence, over every run.
    pub correspondence: Correspondence,
    /// Quiescence and conservation counts, over every run.
    pub tally: Tally,
    /// Runs that lost a volatile write to a cancellation.
    pub cancel_after_submit: usize,
    /// Runs with a replica crash after an ack.
    pub crash_after_ack: usize,
    /// Runs with both values durable in one epoch.
    pub contested: usize,
    /// Carried payloads over every run, from the journals: received, and confirmed by the
    /// receiver (bn-2faf1).
    pub carried: [usize; 2],
    /// Findings, by property.
    pub by_property: BTreeMap<Property, usize>,
    /// Plans that break [`discipline`], by rule.
    pub breaches: BTreeMap<Breach, usize>,
    /// Plans run exhaustively.
    pub exhaustive: usize,
    /// The campaign's identity: BLAKE3 over every plan, log and journal digest, in
    /// order.
    pub digest: String,
}

/// A plan, rendered: values by incarnation and each replica's script.
#[must_use]
pub fn render_plan(plan: &Plan) -> String {
    let mut s = format!("epochs={}", plan.epochs);
    for (n, acts) in plan.replicas.iter().enumerate() {
        let values: Vec<String> = register::incarnation_values(plan, n)
            .iter()
            .map(|v| {
                (0..plan.epochs)
                    .map(|e| VALUES[usize::from(v[usize::from(e)])])
                    .collect::<Vec<_>>()
                    .join("/")
            })
            .collect();
        let parts: Vec<String> = acts
            .iter()
            .map(|a| match a {
                Act::Reserve(e) => format!("reserve{e}"),
                Act::Submit(e) => format!("submit{e}"),
                Act::Sync(e) => format!("sync{e}"),
                Act::Release(e) => format!("release{e}"),
                Act::Abort(e) => format!("abort{e}"),
                Act::Confirm(e) => format!("confirm{e}"),
                Act::Crash => "crash".to_owned(),
                Act::CrashRepropose(e, v) => {
                    format!("crash-repropose{e}={}", VALUES[usize::from(*v)])
                }
            })
            .collect();
        let _ = write!(
            s,
            " | {}[{}]: {}",
            ["a", "b", "c"][n],
            values.join(","),
            if parts.is_empty() {
                "idle".to_owned()
            } else {
                parts.join(" ")
            }
        );
    }
    s
}

/// Run every plan of `campaign` under its logs, with the shutdown, and check every run.
#[must_use]
pub fn execute(campaign: &Campaign) -> Outcome {
    execute_with(campaign, |p| Ok(register::build_with_shutdown(p)))
}

/// As [`execute`], with each crash realized as `mode` says (bn-20d8u).
#[must_use]
pub fn execute_in(campaign: &Campaign, mode: register::CrashMode) -> Outcome {
    execute_with(campaign, |p| Ok(register::build_with_shutdown_in(p, mode)))
}

/// As [`execute`], with `builder` turning each plan into its program: a mutant of the
/// program, not of the plan, is a builder (for example `register::build_with_shutdown`
/// followed by `register::without_op`). A plan the builder refuses is a typed
/// [`Finding::Unbuildable`] for that plan, with no runs, never a panic of the campaign.
#[must_use]
pub fn execute_with(
    campaign: &Campaign,
    builder: impl Fn(&Plan) -> Result<Built, String>,
) -> Outcome {
    let reach: BTreeMap<u8, BTreeSet<Raw>> = [1, 2]
        .into_iter()
        .map(|e| (e, register::reachable(e)))
        .collect();
    let mut out = Outcome::default();
    let mut identity = String::new();
    let _ = writeln!(
        identity,
        "campaign {} seed {:#x} bounds {:?}",
        campaign.name, campaign.seed, campaign.bounds
    );
    for (gi, group) in campaign.groups.iter().enumerate() {
        let _ = writeln!(identity, "group {} logs {}", group.id, group.logs);
        for (pi, plan) in group.plans.iter().enumerate() {
            if let Err(v) = discipline(plan, campaign.bounds) {
                *out.breaches.entry(v.breach).or_default() += 1;
            }
            let _ = writeln!(identity, "plan {pi} {}", render_plan(plan));
            let built = match builder(plan) {
                Ok(b) => b,
                Err(why) => {
                    let _ = writeln!(identity, "unbuildable {why}");
                    *out.by_property.entry(Property::Run).or_default() += 1;
                    out.plans.push(PlanOutcome {
                        group: group.id,
                        index: pi,
                        scope: Scope::Exhaustive,
                        runs: 0,
                        findings: vec![(0, vec![Finding::Unbuildable(why)])],
                    });
                    continue;
                }
            };
            let (scope, logs) = logs_for(&built, group.logs, campaign.plan_seed(gi, pi));
            if scope == Scope::Exhaustive {
                out.exhaustive += 1;
            }
            let mut po = PlanOutcome {
                group: group.id,
                index: pi,
                scope,
                runs: logs.len(),
                findings: Vec::new(),
            };
            for (li, log) in logs.iter().enumerate() {
                let r = run_one(&built, plan.epochs, log, &reach[&plan.epochs]);
                let _ = writeln!(identity, "{log} {}", r.digest);
                out.runs += 1;
                out.cancel_after_submit += usize::from(r.cancel_after_submit);
                out.crash_after_ack += usize::from(r.crash_after_ack);
                out.contested += usize::from(r.contested);
                out.carried[0] += r.carried[0];
                out.carried[1] += r.carried[1];
                out.tally.absorb(&r.tally);
                for f in &r.findings {
                    *out.by_property.entry(f.property()).or_default() += 1;
                }
                if !r.findings.is_empty() {
                    po.findings.push((li, r.findings));
                }
                out.correspondence.absorb(r.correspondence);
            }
            out.plans.push(po);
        }
    }
    out.digest = Blake3Hasher::hash(identity.as_bytes()).to_string();
    out
}
