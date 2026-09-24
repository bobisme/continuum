//! The replicated register's required mutants, M01 to M10, each with its expected
//! result and a deterministic campaign derived from the correct version's
//! (PR-16/IMPL-05, bn-28oa).
//!
//! Include it beside the other three support files:
//!
//! ```text
//! #[path = "support/primitive_conformance_model.rs"] mod model;
//! #[path = "support/replicated_register.rs"] mod register;
//! #[path = "support/register_baseline.rs"] mod baseline;
//! #[path = "support/register_mutants.rs"] mod mutants;
//! ```
//!
//! # Where the mutants come from
//!
//! `notes/plan/examples/replicated_register.md`, "Required mutants", lists ten defects,
//! [`Id::defect`] holds them verbatim, and a test compares the two. START_HERE PR 16
//! names four of them: ack-before-sync (M01), lost-abort (M02), stale-epoch (M03) and
//! orphan (M06). The PR 16 exit is "each mutant has an expected intent/property and
//! deterministic campaign".
//!
//! # What each mutant is
//!
//! A mutant is a change to the correct program of `register_baseline.rs`, never to a
//! check. A **plan mutant** changes the replica scripts ([`Id::M01`], [`Id::M02`],
//! [`Id::M04`], [`Id::M05`], [`Id::M07`]); its campaign is
//! `baseline().mutated(name, transform)`, so each plan of the correct campaign is
//! replaced by its mutated plan, and `discipline` names the protocol rule it breaks. A
//! **program mutant** changes the built program and keeps the plans ([`Id::M03`],
//! [`Id::M06`], [`Id::M08`]); its campaign is `baseline().mutated(name, Plan::clone)`
//! run through `execute_with` with the mutant's builder. So every mutant campaign has
//! the baseline's seed, bounds, groups, plans (before the mutation) and log counts, and
//! its own name, which is also its evidence ID. Its identity is the same kind of digest
//! as the baseline's, over every plan, log and journal.
//!
//! # What each mutant is expected to do ([`Expected`])
//!
//! - **Detected**: the campaign must refute a named scenario property with a named
//!   symptom, and a shallowest failing run is replayed as the witness ([`witness`]).
//!   The findings must come only from plans the mutant changed.
//! - **Excluded**: asupersync makes the mutant impossible to realize in this program's
//!   shape, and the campaign shows how, with a typed binding refusal or a counted
//!   substrate event. This is never reported as a detection. The version of the defect
//!   that the substrate cannot exclude needs semantics the program does not have, and
//!   the residual names them.
//! - **Deferred**: the mutant is not a change to the program at all, and its owner is
//!   another bone.
//!
//! # Scope
//!
//! As the baseline's: the explored runs only, most logs sampled, not DPOR. A witness is
//! the shallowest failing run the campaign found, replayed, not a minimized one (PR 18
//! reduces). The binding has no data-dependent control flow, so a mutant's decisions are
//! its scripts' structure.

use std::collections::{BTreeMap, BTreeSet};

use continuum_asupersync::binding::{BindingRefusal, SubstrateOp, run};
use continuum_asupersync::choice::ChoiceLog;
use continuum_asupersync::family::lifecycle::TaskLabel;

use crate::baseline::{self, Breach, Campaign, Finding, Outcome, Property, RunReport};
use crate::register::{self, Act, Built, Plan, Role};

/// The required mutants of `replicated_register.md`, by their IDs there.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Id {
    /// Acknowledge after volatile append but before `sync`.
    M01,
    /// A cancelled sender loses a reserved message without abort.
    M02,
    /// A restart reuses the old process epoch.
    M03,
    /// A duplicate `Commit` is not idempotent.
    M04,
    /// The coordinator counts one replica twice across a restart.
    M05,
    /// The loser task from a quorum race is not drained.
    M06,
    /// Recovery ignores checksum or truncated-tail state.
    M07,
    /// A timer from an old epoch fires in a new process.
    M08,
    /// The view maps `Submitted` storage to abstract `Chosen`.
    M09,
    /// The independence rule says two writes to the same epoch commute.
    M10,
}

impl Id {
    /// Every mutant, in order.
    pub const ALL: [Self; 10] = [
        Self::M01,
        Self::M02,
        Self::M03,
        Self::M04,
        Self::M05,
        Self::M06,
        Self::M07,
        Self::M08,
        Self::M09,
        Self::M10,
    ];

    /// `1..=10`.
    #[must_use]
    pub const fn number(self) -> u8 {
        self as u8 + 1
    }

    /// The defect, verbatim from `replicated_register.md`'s table.
    #[must_use]
    pub const fn defect(self) -> &'static str {
        match self {
            Self::M01 => "acknowledge after volatile append but before `sync`",
            Self::M02 => "canceled sender loses a reserved message without abort",
            Self::M03 => "restart reuses the old process epoch",
            Self::M04 => "duplicate `Commit` is not idempotent",
            Self::M05 => "coordinator counts one replica twice across restart",
            Self::M06 => "loser task from a quorum race is not drained",
            Self::M07 => "recovery ignores checksum/truncated-tail state",
            Self::M08 => "timer from an old epoch fires in a new process",
            Self::M09 => "view maps `Submitted` storage to abstract `Chosen`",
            Self::M10 => "independence rule says two writes to same epoch commute",
        }
    }

    /// The campaign's name and the evidence's stable artifact ID:
    /// `pr16-impl05-mut-NN-<slug>`. START_HERE's four names are the slugs of M01, M02,
    /// M03 and M06.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::M01 => "pr16-impl05-mut-01-ack-before-sync",
            Self::M02 => "pr16-impl05-mut-02-lost-abort",
            Self::M03 => "pr16-impl05-mut-03-stale-epoch",
            Self::M04 => "pr16-impl05-mut-04-duplicate-commit",
            Self::M05 => "pr16-impl05-mut-05-double-count",
            Self::M06 => "pr16-impl05-mut-06-orphan",
            Self::M07 => "pr16-impl05-mut-07-torn-tail",
            Self::M08 => "pr16-impl05-mut-08-stale-timer",
            Self::M09 => "pr16-impl05-mut-09-submitted-as-chosen",
            Self::M10 => "pr16-impl05-mut-10-same-epoch-commute",
        }
    }

    /// How the defect is written as a change to the correct program.
    #[must_use]
    pub const fn mutation(self) -> &'static str {
        match self {
            Self::M01 => {
                "plan mutant: every Confirm moves to right after its Submit, so a replica \
                 confirms bytes that are still volatile, and a restarted replica confirms \
                 again after its retry is submitted, which also breaks ConfirmTwice after \
                 the first breach"
            }
            Self::M02 => {
                "plan mutant: the explicit Abort of a cancelled writer's permit is dropped, \
                 so the writer retries while the lost permit stays reserved. The spec's \
                 reserved message is a channel send permit; the binding's Send has no \
                 reserve phase, so the mutant uses the write permit, a Transaction. A run \
                 whose writer ends holding it is the binding's ReservationDropped refusal, \
                 an INV-008 Unsupported inconclusive, so ObligationConservation never sees \
                 the leak"
            }
            Self::M03 => {
                "program mutant: every command to a restarted incarnation's writer, the \
                 shutdown's Finish included, goes to the first incarnation's writer of \
                 that epoch, so the new process acts in the epoch the crash cancelled"
            }
            Self::M04 => {
                "plan mutant: after each Confirm the replica applies the same Commit again, \
                 a second reserve, submit, sync and release of the slot it already holds \
                 durably"
            }
            Self::M05 => {
                "plan mutant: a restarted replica confirms again every epoch its crashed \
                 incarnation confirmed, and the coordinator counts confirmations, so it \
                 counts that replica twice"
            }
            Self::M06 => {
                "program mutant: the shutdown does not finish the loser of a quorum race, \
                 the coordinator of a value that fewer than two replicas confirm in an \
                 epoch where another value gets a majority. The program has no race or \
                 quorum combinator and no per-reply task, so the loser is that \
                 coordinator; its detection is the one IMPL-04's missing-Finish probe \
                 shows, on exactly the plans that have a loser"
            }
            Self::M07 => {
                "plan mutant: a replica that crashes after its submit and before its sync \
                 recovers the lost record as if it were durable: its next incarnation keeps \
                 the old value and confirms it without writing"
            }
            Self::M08 => {
                "program mutant: before each crash the in-flight writer arms a timer \
                 (Sleep), and after the crash the clock advances past its deadline, so the \
                 timer comes due in the new process"
            }
            Self::M09 => "none: a mutant of the refinement view, not of the program",
            Self::M10 => "none: a mutant of the independence relation, not of the program",
        }
    }
}

/// A typed binding refusal a campaign may meet, matched by its rendering.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refusal {
    /// [`BindingRefusal::ReservationDropped`]: a task returned while it held a
    /// reservation, and the substrate aborted it for error.
    ReservationDropped,
    /// [`BindingRefusal::TaskEnded`]: an operation commands a task that has ended.
    TaskEnded,
}

impl Refusal {
    /// Whether `rendered` is this refusal's rendering for some ordinal: equal to the
    /// rendering for ordinal 0 once every run of digits is read as one placeholder.
    #[must_use]
    pub fn matches(self, rendered: &str) -> bool {
        let template = match self {
            Self::ReservationDropped => BindingRefusal::ReservationDropped { reservation: 0 },
            Self::TaskEnded => BindingRefusal::TaskEnded { task: 0 },
        };
        digits_blank(rendered) == digits_blank(&template.to_string())
    }

    /// Whether `refusal` is this one.
    #[must_use]
    pub const fn is(self, refusal: &BindingRefusal) -> bool {
        matches!(
            (self, refusal),
            (
                Self::ReservationDropped,
                BindingRefusal::ReservationDropped { .. }
            ) | (Self::TaskEnded, BindingRefusal::TaskEnded { .. })
        )
    }
}

fn digits_blank(s: &str) -> String {
    let mut out = String::new();
    let mut in_digits = false;
    for c in s.chars() {
        if c.is_ascii_digit() {
            if !in_digits {
                out.push('#');
            }
            in_digits = true;
        } else {
            out.push(c);
            in_digits = false;
        }
    }
    out
}

/// The finding a detected mutant's witness must show.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Symptom {
    /// [`Finding::AckedNotDurable`]: an acknowledgement with no durable majority under it.
    AckedNotDurable,
    /// A refinement [`register::Mismatch::Unprojectable`]: two live writes on one slot,
    /// a state the durable register does not have.
    Unprojectable,
    /// [`baseline::Unsettled::TaskLive`] of a coordinator: a task that never ends.
    CoordinatorLive,
    /// [`Finding::Agreement`]: two values acknowledged for one epoch.
    Agreement,
}

impl Symptom {
    /// Whether `finding` is this symptom.
    #[must_use]
    pub const fn matches(self, finding: &Finding) -> bool {
        matches!(
            (self, finding),
            (Self::AckedNotDurable, Finding::AckedNotDurable(_))
                | (
                    Self::Unprojectable,
                    Finding::Refinement(register::Mismatch::Unprojectable, _)
                )
                | (
                    Self::CoordinatorLive,
                    Finding::Quiescence(baseline::Unsettled::TaskLive(_))
                )
                | (Self::Agreement, Finding::Agreement(_))
        )
    }

    /// The property it refutes.
    #[must_use]
    pub const fn property(self) -> Property {
        match self {
            Self::AckedNotDurable | Self::Unprojectable => Property::RuntimeToAbstract,
            Self::CoordinatorLive => Property::Quiescence,
            Self::Agreement => Property::Agreement,
        }
    }
}

/// Why a mutant cannot be realized in this program's shape. Neither is a detection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Exclusion {
    /// A crash cancels the incarnation's region, and so ends its tasks. Every run of a
    /// changed plan is the binding's typed [`Refusal::TaskEnded`] when the new process
    /// commands a task of the old epoch. That refusal is the binding's own check before
    /// it sends a command, with no INV-008 reason: the binding reads such a program as
    /// malformed, not as a run. So the mutant is not expressible through task identity;
    /// this does not show that asupersync fences a stale epoch that is carried in data.
    EpochFenced,
    /// A crash's region cancellation drops the old incarnation's timer: the substrate
    /// traces `TimerCancelled`, and the binding fires only timers of tasks that still
    /// sleep. No timer fires after its task's region is cancelled, and no run has a
    /// finding. The zero is the binding's bookkeeping over the substrate's trace, not an
    /// independent observation.
    TimerDropped,
}

/// What a mutant's campaign is expected to show: the mutant's expected intent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Expected {
    /// The campaign refutes `symptom.property()`, and the shallowest such run is a
    /// replayed witness.
    Detected {
        /// The protocol rule each changed plan breaks first; `None` for a program mutant,
        /// whose plans are the correct ones.
        breach: Option<Breach>,
        /// The witness's finding.
        symptom: Symptom,
        /// A second property some run also refutes.
        also: Option<Symptom>,
        /// A typed refusal some runs of changed plans end in instead, not counted as a
        /// detection.
        refusal: Option<Refusal>,
    },
    /// Not realizable here; the campaign shows why.
    Excluded {
        /// How.
        exclusion: Exclusion,
        /// The version of the defect the substrate does not exclude, and what it needs.
        residual: &'static str,
    },
    /// Not a program mutant; `owner` runs it.
    Deferred {
        /// The owning bones.
        owner: &'static str,
        /// Why it is not a mutant of the program.
        why: &'static str,
    },
}

/// Each mutant's expected result.
#[must_use]
pub const fn expected(id: Id) -> Expected {
    match id {
        Id::M01 => Expected::Detected {
            breach: Some(Breach::ConfirmBeforeSync),
            symptom: Symptom::AckedNotDurable,
            also: Some(Symptom::Agreement),
            refusal: None,
        },
        Id::M02 => Expected::Detected {
            breach: Some(Breach::ReserveHeld),
            symptom: Symptom::Unprojectable,
            also: None,
            refusal: Some(Refusal::ReservationDropped),
        },
        Id::M03 => Expected::Excluded {
            exclusion: Exclusion::EpochFenced,
            residual: "a stale epoch carried in message or record data, accepted by a \
                       receiver that compares epochs; the program's messages and records \
                       carry no data, so it needs data-carrying Prepare, Prepared and \
                       Commit messages",
        },
        Id::M04 => Expected::Detected {
            breach: Some(Breach::RewriteDurable),
            symptom: Symptom::Unprojectable,
            also: None,
            refusal: None,
        },
        Id::M05 => Expected::Detected {
            breach: Some(Breach::ConfirmTwice),
            symptom: Symptom::AckedNotDurable,
            also: Some(Symptom::Agreement),
            refusal: None,
        },
        Id::M06 => Expected::Detected {
            breach: None,
            symptom: Symptom::CoordinatorLive,
            also: None,
            refusal: None,
        },
        Id::M07 => Expected::Detected {
            breach: Some(Breach::ConfirmBeforeSync),
            symptom: Symptom::AckedNotDurable,
            also: None,
            refusal: None,
        },
        Id::M08 => Expected::Excluded {
            exclusion: Exclusion::TimerDropped,
            residual: "a timer armed outside the incarnation's region, whose firing \
                       carries the old epoch into the new process; the binding's timers \
                       carry no data and the correct program has no timers, so it needs \
                       data-carrying timer callbacks",
        },
        Id::M09 => Expected::Deferred {
            owner: "bn-2et (PR 17 concrete/abstract projection) and bn-1mm (mutants fail at mapped transitions)",
            why: "it changes the refinement view, which PR 17 owns; the program is unchanged",
        },
        Id::M10 => Expected::Deferred {
            owner: "bn-voq4 (C005, baseline DPOR)",
            why: "it changes the independence relation DPOR reduces with; no campaign here \
                  uses one",
        },
    }
}

// ---------------------------------------------------------------------------
// plan mutants
// ---------------------------------------------------------------------------

fn each_script(plan: &Plan, f: impl Fn(&[Act]) -> Vec<Act>) -> Plan {
    let mut out = plan.clone();
    for (to, from) in out.replicas.iter_mut().zip(&plan.replicas) {
        *to = f(from);
    }
    out
}

/// M01: every `Confirm(e)` moves to right after each `Submit(e)`.
#[must_use]
pub fn ack_before_sync(plan: &Plan) -> Plan {
    each_script(plan, |s| {
        let mut out = Vec::new();
        for act in s {
            match *act {
                Act::Confirm(_) => {}
                Act::Submit(e) => out.extend([Act::Submit(e), Act::Confirm(e)]),
                other => out.push(other),
            }
        }
        out
    })
}

/// M02: every explicit `Abort` is dropped.
#[must_use]
pub fn lost_abort(plan: &Plan) -> Plan {
    each_script(plan, |s| {
        s.iter()
            .copied()
            .filter(|a| !matches!(a, Act::Abort(_)))
            .collect()
    })
}

/// M04: after each `Confirm(e)`, the write of `e` again, without a second confirmation.
#[must_use]
pub fn duplicate_commit(plan: &Plan) -> Plan {
    each_script(plan, |s| {
        let mut out = Vec::new();
        for act in s {
            out.push(*act);
            if let Act::Confirm(e) = *act {
                out.extend([
                    Act::Reserve(e),
                    Act::Submit(e),
                    Act::Sync(e),
                    Act::Release(e),
                ]);
            }
        }
        out
    })
}

/// M05: after each crash, the epochs the crashed incarnation confirmed are confirmed
/// again by the next incarnation.
#[must_use]
pub fn double_count(plan: &Plan) -> Plan {
    each_script(plan, |s| {
        let mut out = Vec::new();
        let mut confirmed = Vec::new();
        for act in s {
            out.push(*act);
            match *act {
                Act::Confirm(e) => confirmed.push(e),
                Act::Crash | Act::CrashRepropose(..) => {
                    out.extend(confirmed.drain(..).map(Act::Confirm));
                }
                _ => {}
            }
        }
        out
    })
}

/// M07: `Submit(e), crash, <the retry write of e>` becomes `Submit(e), Crash,
/// Confirm(e)`: the next incarnation keeps the lost record's value and confirms it.
#[must_use]
pub fn torn_tail(plan: &Plan) -> Plan {
    each_script(plan, |s| {
        let mut out = Vec::new();
        let mut i = 0;
        while i < s.len() {
            if let (Act::Submit(e), Some(Act::Crash | Act::CrashRepropose(..))) =
                (s[i], s.get(i + 1))
                && s.get(i + 2..i + 7) == Some(&register::write(e)[..])
            {
                out.extend([Act::Submit(e), Act::Crash, Act::Confirm(e)]);
                i += 7;
                continue;
            }
            out.push(s[i]);
            i += 1;
        }
        out
    })
}

// ---------------------------------------------------------------------------
// program mutants
// ---------------------------------------------------------------------------

/// Each writer's label, by `(node, incarnation, epoch)`: the setup allocates writers
/// node by node, incarnation by incarnation, epoch by epoch.
#[must_use]
pub fn writers(built: &Built) -> BTreeMap<(u8, usize, u8), TaskLabel> {
    let mut next: BTreeMap<(u8, u8), usize> = BTreeMap::new();
    let mut out = BTreeMap::new();
    for (i, (role, _)) in built.roles.tasks.iter().enumerate() {
        if let Role::Writer { node, epoch, .. } = *role {
            let inc = next.entry((node, epoch)).or_default();
            out.insert((node, *inc, epoch), built.labels[i]);
            *inc += 1;
        }
    }
    out
}

/// How many confirmations each `(epoch, value)` gets in `plan`.
#[must_use]
pub fn confirmation_counts(plan: &Plan) -> BTreeMap<(u8, u8), usize> {
    let mut out = BTreeMap::new();
    for n in 0..3 {
        let values = register::incarnation_values(plan, n);
        let mut inc = 0;
        for act in &plan.replicas[n] {
            match *act {
                Act::Crash | Act::CrashRepropose(..) => inc += 1,
                Act::Confirm(e) => *out.entry((e, values[inc][usize::from(e)])).or_default() += 1,
                _ => {}
            }
        }
    }
    out
}

/// M03's builder. Every later incarnation reuses the first incarnation's epoch, so after
/// two crashes "the old epoch" is the first one, not the one just before.
///
/// # Errors
///
/// Never; the signature is `execute_with`'s.
pub fn stale_epoch(plan: &Plan) -> Result<Built, String> {
    let mut built = register::build_with_shutdown(plan);
    let w = writers(&built);
    let old: BTreeMap<TaskLabel, TaskLabel> = w
        .iter()
        .map(|(&(n, _, e), &t)| (t, w[&(n, 0, e)]))
        .collect();
    for program in built.programs.iter_mut().skip(1) {
        for op in program.iter_mut() {
            if let SubstrateOp::Reserve { task, .. }
            | SubstrateOp::Acquire { task, .. }
            | SubstrateOp::Send { task, .. }
            | SubstrateOp::Finish { task } = op
                && let Some(t) = old.get(task)
            {
                *task = *t;
            }
        }
    }
    Ok(built)
}

/// The losers of the quorum race in `plan`, as task ordinals: the coordinator of each
/// `(epoch, value)` that fewer than two replicas confirm, in an epoch where another
/// value gets a majority. The loser parks on its channel for a confirmation that never
/// comes, and only the shutdown ends it.
#[must_use]
pub fn losers(plan: &Plan, built: &Built) -> Vec<usize> {
    let counts = confirmation_counts(plan);
    let won = |e: u8, v: u8| {
        counts
            .iter()
            .any(|(&(f, w), &n)| f == e && w != v && n >= 2)
    };
    built
        .roles
        .tasks
        .iter()
        .enumerate()
        .filter_map(|(i, (role, _))| match *role {
            Role::Coordinator { epoch, value }
                if counts.get(&(epoch, value)).copied().unwrap_or(0) < 2 && won(epoch, value) =>
            {
                Some(i)
            }
            _ => None,
        })
        .collect()
}

/// M06's builder.
///
/// # Errors
///
/// When the shutdown has no `Finish` for a loser, which `build_with_shutdown` never
/// builds.
pub fn orphan(plan: &Plan) -> Result<Built, String> {
    let mut built = register::build_with_shutdown(plan);
    let last = built.programs.len() - 1;
    for ordinal in losers(plan, &built) {
        let finish = SubstrateOp::Finish {
            task: built.labels[ordinal],
        };
        let at = built.programs[last]
            .iter()
            .position(|op| *op == finish)
            .ok_or_else(|| format!("no finish for coordinator t{ordinal}"))?;
        built = register::without_op(&built, last, at);
    }
    Ok(built)
}

/// The timer M08 arms, and how far the clock moves after the crash.
pub const STALE_TIMER: (u64, u64) = (10, 20);

/// M08's builder: for each crash act of each replica, from the last, a `Sleep` of the
/// crashing incarnation's writer for the epoch of the act before the crash, and an
/// `Advance` right after the crash. A replica's operations are its acts, one for one.
///
/// # Errors
///
/// Never; the signature is `execute_with`'s.
pub fn stale_timer(plan: &Plan) -> Result<Built, String> {
    let mut built = register::build_with_shutdown(plan);
    let w = writers(&built);
    for n in 0..3_u8 {
        let script = &plan.replicas[usize::from(n)];
        let mut sites = Vec::new();
        let (mut inc, mut epoch) = (0, 0);
        for (at, act) in script.iter().enumerate() {
            match *act {
                Act::Crash | Act::CrashRepropose(..) => {
                    sites.push((at, w[&(n, inc, epoch)]));
                    inc += 1;
                }
                Act::Reserve(e)
                | Act::Submit(e)
                | Act::Sync(e)
                | Act::Release(e)
                | Act::Abort(e)
                | Act::Confirm(e) => epoch = e,
            }
        }
        let actor = 1 + usize::from(n);
        for (at, task) in sites.into_iter().rev() {
            built = register::with_op(
                &built,
                actor,
                at + 1,
                SubstrateOp::Advance {
                    nanos: STALE_TIMER.1,
                },
            );
            built = register::with_op(
                &built,
                actor,
                at,
                SubstrateOp::Sleep {
                    task,
                    nanos: STALE_TIMER.0,
                },
            );
        }
    }
    Ok(built)
}

// ---------------------------------------------------------------------------
// the campaigns
// ---------------------------------------------------------------------------

/// A builder for `execute_with`.
pub type Builder = fn(&Plan) -> Result<Built, String>;

/// One mutant's campaign.
#[derive(Debug, Clone)]
pub struct Mutant {
    /// Which.
    pub id: Id,
    /// The campaign, named [`Id::name`].
    pub campaign: Campaign,
    /// The builder: `build_with_shutdown` for a plan mutant.
    pub builder: Builder,
    /// Whether it changes plans rather than the program.
    pub plan_mutant: bool,
}

fn correct(plan: &Plan) -> Result<Built, String> {
    Ok(register::build_with_shutdown(plan))
}

/// The campaign of `id`, derived from `base` (the baseline campaign); `None` for a
/// deferred mutant.
#[must_use]
pub fn mutant(id: Id, base: &Campaign) -> Option<Mutant> {
    let plan: Option<fn(&Plan) -> Plan> = match id {
        Id::M01 => Some(ack_before_sync),
        Id::M02 => Some(lost_abort),
        Id::M04 => Some(duplicate_commit),
        Id::M05 => Some(double_count),
        Id::M07 => Some(torn_tail),
        _ => None,
    };
    let program: Option<Builder> = match id {
        Id::M03 => Some(stale_epoch),
        Id::M06 => Some(orphan),
        Id::M08 => Some(stale_timer),
        _ => None,
    };
    match (plan, program) {
        (Some(f), _) => Some(Mutant {
            id,
            campaign: base.mutated(id.name(), f),
            builder: correct,
            plan_mutant: true,
        }),
        (None, Some(b)) => Some(Mutant {
            id,
            campaign: base.mutated(id.name(), Plan::clone),
            builder: b,
            plan_mutant: false,
        }),
        (None, None) => None,
    }
}

/// Run a mutant's campaign.
#[must_use]
pub fn execute(m: &Mutant) -> Outcome {
    baseline::execute_with(&m.campaign, m.builder)
}

/// Whether the mutant changed plan `plan` of group `group` against `base`: its plan
/// (plan mutant) or its built program (program mutant).
#[must_use]
pub fn changed(m: &Mutant, base: &Campaign, group: usize, plan: usize) -> bool {
    let before = &base.groups[group].plans[plan];
    if m.plan_mutant {
        let after = &m.campaign.groups[group].plans[plan];
        before.replicas != after.replicas || before.values != after.values
    } else {
        (m.builder)(before).map(|b| b.programs)
            != Ok(register::build_with_shutdown(before).programs)
    }
}

// ---------------------------------------------------------------------------
// witnesses
// ---------------------------------------------------------------------------

/// A failing run of a mutant campaign, chosen as its witness.
#[derive(Debug, Clone)]
pub struct Witness {
    /// The group's index and ID.
    pub group: (usize, &'static str),
    /// The plan's index in its group.
    pub plan: usize,
    /// The log's index among the plan's logs.
    pub index: usize,
    /// The log.
    pub log: ChoiceLog,
    /// The finding it was chosen for.
    pub finding: Finding,
    /// Every finding of the run, as the campaign recorded it.
    pub findings: Vec<Finding>,
}

/// The journal event a finding is observed at, when it has one.
#[must_use]
pub const fn seq_of(f: &Finding) -> Option<u64> {
    match f {
        Finding::Agreement(s) | Finding::Refinement(_, s) | Finding::AckedNotDurable(s) => Some(*s),
        _ => None,
    }
}

/// The shallowest run of `outcome` with a finding `pick` accepts, restricted to the
/// plans `within` accepts (group index, plan index): the least event the finding is
/// observed at, then the shortest log, then campaign order. A finding observed only at
/// the run's end has no event and sorts after every one that has.
#[must_use]
pub fn witness(
    m: &Mutant,
    outcome: &Outcome,
    pick: impl Fn(&Finding) -> bool,
    within: impl Fn(usize, usize) -> bool,
) -> Option<Witness> {
    witness_where(m, outcome, pick, within, |_| true)
}

/// As [`witness`], among the candidate runs `accept` also accepts.
#[must_use]
pub fn witness_where(
    m: &Mutant,
    outcome: &Outcome,
    pick: impl Fn(&Finding) -> bool,
    within: impl Fn(usize, usize) -> bool,
    accept: impl Fn(&Witness) -> bool,
) -> Option<Witness> {
    let mut best: Option<((u64, usize), Witness)> = None;
    for po in &outcome.plans {
        let gi = m
            .campaign
            .groups
            .iter()
            .position(|g| g.id == po.group)
            .expect("a group of the campaign");
        if !within(gi, po.index) {
            continue;
        }
        let mut logs = None;
        for (li, findings) in &po.findings {
            let Some(f) = findings
                .iter()
                .filter(|f| pick(f))
                .min_by_key(|f| seq_of(f).unwrap_or(u64::MAX))
            else {
                continue;
            };
            let logs = logs.get_or_insert_with(|| {
                let plan = &m.campaign.groups[gi].plans[po.index];
                let built = (m.builder)(plan).expect("built in the campaign");
                baseline::logs_for(
                    &built,
                    m.campaign.groups[gi].logs,
                    m.campaign.plan_seed(gi, po.index),
                )
                .1
            });
            let key = (seq_of(f).unwrap_or(u64::MAX), logs[*li].len());
            if best.as_ref().is_some_and(|(k, _)| key >= *k) {
                continue;
            }
            let w = Witness {
                group: (gi, po.group),
                plan: po.index,
                index: *li,
                log: logs[*li].clone(),
                finding: f.clone(),
                findings: findings.clone(),
            };
            if accept(&w) {
                best = Some((key, w));
            }
        }
    }
    best.map(|(_, w)| w)
}

/// A witness run again from its plan and log alone: the run's report, and the binding's
/// own result (the journal's event count, or the typed refusal).
pub fn replay(m: &Mutant, w: &Witness) -> (RunReport, Result<usize, BindingRefusal>) {
    let plan = &m.campaign.groups[w.group.0].plans[w.plan];
    let built = (m.builder)(plan).expect("built in the campaign");
    let reach = register::reachable(plan.epochs);
    let report = baseline::run_one(&built, plan.epochs, &w.log, &reach);
    let raw = run(&built.programs, &w.log, &baseline::config()).map(|j| j.events().len());
    (report, raw)
}

/// The durable-register steps of a witness's journal that are not stutters, in order,
/// by label: its causal story.
#[must_use]
pub fn story(m: &Mutant, w: &Witness) -> Vec<String> {
    let plan = &m.campaign.groups[w.group.0].plans[w.plan];
    let built = (m.builder)(plan).expect("built in the campaign");
    let journal = run(&built.programs, &w.log, &baseline::config()).expect("a journal");
    register::observe(&built.roles, &journal)
        .expect("projects")
        .into_iter()
        .filter_map(|s| match s.expect {
            register::Expect::Stutter => None,
            register::Expect::Step(l) | register::Expect::StepPrefix(l) => Some(l),
        })
        .collect()
}

/// The protocol rules the changed plans break, by rule.
#[must_use]
pub fn breaches(outcome: &Outcome) -> BTreeSet<Breach> {
    outcome.breaches.keys().copied().collect()
}
