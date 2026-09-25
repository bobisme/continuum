//! PR 18's register instantiation, shared by the reduction tests: the failing runs of
//! M01, the replay oracles over them (the substrate replay and the program-level replay,
//! bn-3km4z and bn-2z08o), the mechanism predicate, the transcript's independent check,
//! and the program's causal order. Moved out of `pr18_impl01_reduction.rs` unchanged, so
//! that `pr18_impl02_scenario_reduction.rs` (bn-25z9o) reduces the same runs with the same
//! oracles. Include it with `#[path = "support/pr18_program.rs"] mod program;` next to the
//! `model`, `register`, `baseline` and `mutants` modules, under those names.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;

use continuum_asupersync::binding::run;
use continuum_asupersync::causal::{self, Access, Footprint, Key, Renaming, Restriction};
use continuum_asupersync::choice::ChoiceLog;
use continuum_asupersync::journal::Journal;
use continuum_asupersync::lift::{LiftVerdict, lift};
use continuum_debugger::reduce::{
    self, Attempts, Budget, CausalOrder, Deletion, NotReplayable, Pass, Reduction, Replay,
    Replayed, Trial, Verdict,
};
use continuum_value::assurance::InconclusiveReason;

use crate::model::{Alphabet, FamilyTag, judge};
use crate::register::{self, Built, Expect, Plan, Raw, Role, Roles};
use crate::{baseline, mutants};

/// The work bound for deriving a journal's relation: far above any register journal.
pub const CAUSAL_WORK: u64 = 1 << 24;

/// The reduction budget every test here grants: replays and work units.
pub const BUDGET: Budget = Budget::new(4_096, 1 << 26);

/// The failure a reduction preserves: the finding kind the original run showed first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Target {
    /// `abstract_register::Agreement`: two values acknowledged for one epoch.
    Agreement,
    /// `RuntimeToAbstract`'s `AckedNotDurable`: an acknowledgement with no durable
    /// majority.
    AckedNotDurable,
}

impl Target {
    pub const fn name(self) -> &'static str {
        match self {
            Self::Agreement => "abstract_register::Agreement",
            Self::AckedNotDurable => "RuntimeToAbstract/AckedNotDurable",
        }
    }

    /// The acknowledged `(epoch, value)` pairs that make `raw` violate the target.
    pub fn offending(self, raw: &Raw) -> BTreeSet<(u8, u8)> {
        let acks = register::acks_of(raw);
        match self {
            Self::Agreement => acks
                .iter()
                .filter(|(e, v)| acks.iter().any(|(f, w)| e == f && v != w))
                .copied()
                .collect(),
            Self::AckedNotDurable => {
                let durable = register::durable_of(raw);
                acks.iter()
                    .filter(|&&(e, v)| {
                        durable
                            .iter()
                            .filter(|&&(_, f, w)| (f, w) == (e, v))
                            .count()
                            < 2
                    })
                    .copied()
                    .collect()
            }
        }
    }
}

pub fn alphabet() -> Alphabet {
    Alphabet::new([
        FamilyTag::Lifecycle,
        FamilyTag::Effect,
        FamilyTag::Cancellation,
        FamilyTag::Obligation,
        FamilyTag::Time,
        FamilyTag::Channel,
    ])
}

/// `roles` carried across a restriction: each kept task keeps its role, its region is
/// renamed, and the region count is the number of the setup's regions the restriction
/// kept. A task whose region the restriction dropped gets a region no journal names, so
/// the projection refuses the candidate rather than guessing.
pub fn carry_roles(roles: &Roles, renaming: &Renaming) -> Roles {
    let tasks = renaming
        .tasks
        .iter()
        .map(|&old| {
            let (role, region) = roles
                .tasks
                .get(old as usize)
                .copied()
                .unwrap_or((Role::Supervisor { node: u8::MAX }, u32::MAX));
            (role, renaming.region(region).unwrap_or(u32::MAX))
        })
        .collect();
    let regions = u32::try_from(
        renaming
            .regions
            .iter()
            .filter(|&&r| r != 0 && r <= roles.regions)
            .count(),
    )
    .expect("few regions");
    let mailboxes = roles
        .mailboxes
        .iter()
        .filter_map(|mb| {
            Some(register::Mailbox {
                channel: renaming.channel(mb.channel)?,
                sender: renaming.task(mb.sender).unwrap_or(u32::MAX),
                taker: renaming.task(mb.taker).unwrap_or(u32::MAX),
                ..*mb
            })
        })
        .collect();
    Roles {
        tasks,
        regions,
        mailboxes,
    }
}

/// The replay oracle over one register journal.
pub struct RegisterOracle<'a> {
    pub journal: &'a Journal,
    pub roles: &'a Roles,
    pub target: Target,
    /// Replays run: the determinism test compares it.
    pub replays: u64,
}

impl<'a> RegisterOracle<'a> {
    pub fn new(journal: &'a Journal, roles: &'a Roles, target: Target) -> Self {
        Self {
            journal,
            roles,
            target,
            replays: 0,
        }
    }
}

/// Replay a restricted journal: the lift, the A7 model, the projection, and the target.
/// Witnesses are positions in the restricted journal.
pub fn replay_restricted(r: &Restriction, roles: &Roles, target: Target) -> Replayed {
    match lift(&r.journal) {
        LiftVerdict::Conforms(_) => {}
        LiftVerdict::Violates { seq, reason } => {
            return Replayed::NotReplayable(NotReplayable::Nonconforming(format!(
                "lift: event {seq}: {reason}"
            )));
        }
        LiftVerdict::Inconclusive { seq, reason, .. } => {
            return Replayed::NotReplayable(NotReplayable::Inconclusive(
                reason,
                format!("lift: event {seq}"),
            ));
        }
    }
    let bytes = r.journal.encode().expect("encodes");
    if !judge(&alphabet(), &bytes).is_accepted() {
        return Replayed::NotReplayable(NotReplayable::Nonconforming("A7 model".to_owned()));
    }
    let carried = carry_roles(roles, &r.renaming);
    let steps = match register::observe(&carried, &r.journal) {
        Ok(steps) => steps,
        Err(refusal) => {
            return Replayed::NotReplayable(NotReplayable::Nonconforming(format!(
                "projection: {refusal:?}"
            )));
        }
    };
    // The first state that violates the target, and the ack steps that introduced its
    // offending pairs.
    let mut introduced: BTreeMap<(u8, u8), usize> = BTreeMap::new();
    for s in &steps {
        let (Some(pre), Some(post)) = (&s.pre, &s.post) else {
            if let Some(post) = &s.post {
                for p in register::acks_of(post) {
                    introduced.entry(*p).or_insert(s.seq as usize);
                }
            }
            continue;
        };
        for p in register::acks_of(post).difference(register::acks_of(pre)) {
            introduced.entry(*p).or_insert(s.seq as usize);
        }
        let offending = target.offending(post);
        if !offending.is_empty() {
            let mut witnesses: BTreeSet<usize> = BTreeSet::from([s.seq as usize]);
            witnesses.extend(offending.iter().filter_map(|p| introduced.get(p)));
            return Replayed::Fails {
                witnesses: witnesses.into_iter().collect(),
            };
        }
    }
    Replayed::Holds
}

impl Replay for RegisterOracle<'_> {
    fn replay(&mut self, kept: &[usize]) -> Replayed {
        self.replays += 1;
        let r = match causal::restrict(self.journal, kept) {
            Ok(r) => r,
            Err(e) => {
                return Replayed::NotReplayable(NotReplayable::Nonconforming(format!(
                    "restriction: {e:?}"
                )));
            }
        };
        match replay_restricted(&r, self.roles, self.target) {
            // Positions in the sub-journal back to trace indices.
            Replayed::Fails { witnesses } => Replayed::Fails {
                witnesses: witnesses.into_iter().map(|w| kept[w]).collect(),
            },
            other => other,
        }
    }
}

// ---------------------------------------------------------------------------
// the program-level replay (bn-2z08o)
// ---------------------------------------------------------------------------

/// The original run's operations, and the journal events each one emitted.
///
/// A register run is the binding executing the program's actors (the setup, the three
/// replicas, the coordinators and the shutdown), one operation per choice. An operation's
/// events are the journal events the binding records while it runs it. They are found by
/// re-execution, never assumed: the program truncated to the first `k` operations of the
/// run is run through the binding, and when its journal is a prefix of the full one, `k`
/// is a boundary. The operations between two boundaries are one **group**: a truncation
/// inside a group is refused by the binding (a cancellation it cannot yet observe, for
/// example) or emits events that are not the run's, so a group stands or falls whole.
/// An operation that emits nothing is grouped with the operations after it.
#[derive(Debug, Clone)]
pub struct Operations {
    /// Per choice of the log: the actor, and the operation's index in its program.
    pub ops: Vec<(usize, usize)>,
    /// Each group: its operations `ops[start..end]` and its events `events.0..events.1`.
    pub groups: Vec<OpGroup>,
    /// The group of each journal event.
    pub group_of: Vec<usize>,
    /// Truncations the binding refused, and truncations whose journal was not a prefix.
    pub refused: usize,
    pub diverged: usize,
}

#[derive(Debug, Clone, Copy)]
pub struct OpGroup {
    pub start: usize,
    pub end: usize,
    pub events: (usize, usize),
}

/// `programs` with each actor truncated to the operations `ops` names, and the choice
/// log that runs them in `ops`'s order, re-indexed against the truncated actors. `None`
/// when `ops` does not hold a prefix of each actor's program, in order: that is no run
/// of the program.
pub fn truncation(
    programs: &[continuum_asupersync::binding::Program],
    ops: &[(usize, usize)],
) -> Option<(Vec<continuum_asupersync::binding::Program>, ChoiceLog)> {
    let mut count = vec![0_usize; programs.len()];
    for &(actor, index) in ops {
        if count.get(actor) != Some(&index) || index >= programs[actor].len() {
            return None;
        }
        count[actor] += 1;
    }
    let truncated = programs
        .iter()
        .zip(&count)
        .map(|(p, &n)| p[..n].to_vec())
        .collect();
    let mut cursor = vec![0_usize; programs.len()];
    let mut choices = Vec::with_capacity(ops.len());
    for &(actor, _) in ops {
        let enabled = (0..actor).filter(|&a| cursor[a] < count[a]).count();
        choices.push(u32::try_from(enabled).expect("few actors"));
        cursor[actor] += 1;
    }
    Some((truncated, ChoiceLog::new(choices)))
}

/// Run the program truncated to `ops` through the binding ([`truncation`]).
pub fn rerun(
    programs: &[continuum_asupersync::binding::Program],
    ops: &[(usize, usize)],
) -> Option<Result<Journal, continuum_asupersync::binding::BindingRefusal>> {
    let (truncated, log) = truncation(programs, ops)?;
    Some(run(&truncated, &log, &baseline::config()))
}

/// The operations a set of whole operation groups holds, in the run's order.
pub fn ops_of(o: &Operations, events: &[usize]) -> Vec<(usize, usize)> {
    let groups: BTreeSet<usize> = events.iter().map(|&e| o.group_of[e]).collect();
    groups
        .into_iter()
        .flat_map(|g| o.ops[o.groups[g].start..o.groups[g].end].iter().copied())
        .collect()
}

/// The differential: the campaign's own checker (`register_baseline::run_one`: the
/// binding, the lift, the A7 model, the projection and the scenario properties), run on
/// the program truncated to `core`'s operations, reports `c`'s target finding. Apart
/// from [`ProgramOracle`]: it reads neither the restriction nor the target predicate.
pub fn campaign_finds_target(c: &Case, o: &Operations, core: &[usize]) -> bool {
    let Some((programs, log)) = truncation(&c.built.programs, &ops_of(o, core)) else {
        return false;
    };
    let mut built = c.built.clone();
    built.programs = programs;
    let report = baseline::run_one(&built, c.epochs, &log, &register::reachable(c.epochs));
    report.findings.iter().any(|f| {
        matches!(
            (f, c.target),
            (baseline::Finding::Agreement(_), Target::Agreement)
                | (
                    baseline::Finding::AckedNotDurable(_),
                    Target::AckedNotDurable
                )
        )
    })
}

pub fn operations_of(c: &Case) -> Operations {
    let programs = &c.built.programs;
    let mut cursor = vec![0_usize; programs.len()];
    let mut ops = Vec::new();
    for choice in c.log.choices() {
        let enabled: Vec<usize> = (0..programs.len())
            .filter(|&a| cursor[a] < programs[a].len())
            .collect();
        let actor = enabled[choice.0 as usize];
        ops.push((actor, cursor[actor]));
        cursor[actor] += 1;
    }
    let full = c.journal.events();
    let (mut refused, mut diverged) = (0, 0);
    // (operations, events) at each boundary, strictly increasing in both. A truncation
    // the binding refuses, or whose journal is not a prefix of the run's, is no boundary:
    // its operations join a coarser group. That only removes candidates; it never admits
    // one, since the oracle still re-executes every candidate it accepts.
    let mut bounds: Vec<(usize, usize)> = vec![(0, 0)];
    for k in 1..=ops.len() {
        let journal = match rerun(programs, &ops[..k]).expect("a prefix of the run") {
            Ok(j) => j,
            Err(_) => {
                refused += 1;
                continue;
            }
        };
        let len = journal.len();
        if len > full.len() || journal.events() != &full[..len] {
            diverged += 1;
            continue;
        }
        if len > bounds.last().expect("a bound").1 {
            bounds.push((k, len));
        } else if k == ops.len() && len == full.len() {
            // Trailing operations that emit nothing join the last group.
            bounds.last_mut().expect("a bound").0 = k;
        }
    }
    assert_eq!(
        bounds.last(),
        Some(&(ops.len(), full.len())),
        "{}: the whole run re-executes to its journal",
        c.label
    );
    let mut groups = Vec::new();
    let mut group_of = vec![0; full.len()];
    for w in bounds.windows(2) {
        let g = groups.len();
        groups.push(OpGroup {
            start: w[0].0,
            end: w[1].0,
            events: (w[0].1, w[1].1),
        });
        for slot in &mut group_of[w[0].1..w[1].1] {
            *slot = g;
        }
    }
    Operations {
        ops,
        groups,
        group_of,
        refused,
        diverged,
    }
}

/// The program's causal order over the journal: the footprint dependence, plus program
/// order (each operation's events follow its actor's previous operation's), with each
/// group of operations as one atom.
pub fn program_order(c: &Case, o: &Operations) -> CausalOrder {
    let mut preds = causal::predecessors_with(&c.journal, CAUSAL_WORK, &c.observer)
        .expect("within the work bound");
    let mut last: BTreeMap<usize, usize> = BTreeMap::new();
    for (g, group) in o.groups.iter().enumerate() {
        for &(actor, _) in &o.ops[group.start..group.end] {
            if let Some(&before) = last.get(&actor) {
                if before != g {
                    let (_, end) = o.groups[before].events;
                    preds[group.events.0].push(end - 1);
                }
            }
            last.insert(actor, g);
        }
    }
    let atoms = o
        .groups
        .iter()
        .map(|g| (g.events.0..g.events.1).collect())
        .collect();
    CausalOrder::with_atoms(preds, atoms).expect("a causal order")
}

/// The program-level replay oracle (bn-2z08o). A candidate is accepted as a run only
/// when the replicated-register program produces it: the candidate must hold whole
/// operation groups, the operations it holds must be a prefix of each actor's program,
/// and the program truncated to them, re-executed through the binding in the run's
/// order, must emit exactly the candidate's sub-journal. That journal then gets the
/// substrate's checks and the target property, as [`RegisterOracle`] gives them. So a
/// coordinator's acknowledgement is kept only with the receives its program puts before
/// it, a receive only with its send, and a replica's confirmation only with the acts its
/// script puts before it: the program's own discipline, enforced by running it.
///
/// An accepted candidate is a complete run of the truncated program. It is a partial run
/// of the whole program under two facts of the binding, which
/// `a_program_replay_core_is_a_prefix_of_a_whole_run` checks on the witnesses' cores: an
/// operation does not depend on the operations its actor has left, since the binding
/// has no data-dependent control flow; and a choice indexes the actors with operations
/// left, so the re-indexed log schedules the same operations in the same order.
pub struct ProgramOracle<'a> {
    pub case: &'a Case,
    pub ops: &'a Operations,
    pub replays: u64,
}

impl Replay for ProgramOracle<'_> {
    fn replay(&mut self, kept: &[usize]) -> Replayed {
        self.replays += 1;
        let o = self.ops;
        let mut held: BTreeMap<usize, usize> = BTreeMap::new();
        for &e in kept {
            let Some(&g) = o.group_of.get(e) else {
                // The reduction passed an index outside the trace: its contract, not a
                // judgement of the program.
                return Replayed::NotReplayable(NotReplayable::Inconclusive(
                    InconclusiveReason::EngineError,
                    format!("event {e} is not in the journal"),
                ));
            };
            *held.entry(g).or_default() += 1;
        }
        for (&g, &n) in &held {
            let (a, b) = o.groups[g].events;
            if n != b - a {
                return Replayed::NotReplayable(NotReplayable::Nonconforming(format!(
                    "operation group {g} is held in part ({n} of {} events)",
                    b - a
                )));
            }
        }
        let ops = ops_of(o, kept);
        let Some(rerun) = rerun(&self.case.built.programs, &ops) else {
            let mut next = BTreeMap::new();
            let (actor, index) = ops
                .iter()
                .copied()
                .find(|&(a, i)| {
                    let n = next.entry(a).or_insert(0_usize);
                    let skip = *n != i;
                    *n += 1;
                    skip
                })
                .expect("a skipped operation");
            return Replayed::NotReplayable(NotReplayable::Nonconforming(format!(
                "not a run of the program: actor {actor} runs operation {index} without the ones before it"
            )));
        };
        let journal = match rerun {
            Ok(j) => j,
            Err(refusal) => {
                return Replayed::NotReplayable(match refusal.inconclusive_reason() {
                    Some(reason) => {
                        NotReplayable::Inconclusive(reason, format!("binding: {refusal}"))
                    }
                    None => NotReplayable::Nonconforming(format!("binding: {refusal}")),
                });
            }
        };
        let r = match causal::restrict(&self.case.journal, kept) {
            Ok(r) => r,
            Err(e @ causal::CausalError::TooManyEvents) => {
                return Replayed::NotReplayable(NotReplayable::Inconclusive(
                    InconclusiveReason::ResourceExhausted,
                    format!("restriction: {e:?}"),
                ));
            }
            Err(e) => {
                return Replayed::NotReplayable(NotReplayable::Inconclusive(
                    InconclusiveReason::EngineError,
                    format!("restriction: {e:?}"),
                ));
            }
        };
        if let Some(at) = (0..journal.len().max(r.journal.len()))
            .find(|&i| journal.events().get(i) != r.journal.events().get(i))
        {
            return Replayed::NotReplayable(NotReplayable::Nonconforming(format!(
                "the program's re-execution differs from the candidate at event {at}"
            )));
        }
        match replay_restricted(&r, &self.case.built.roles, self.case.target) {
            Replayed::Fails { witnesses } => Replayed::Fails {
                witnesses: witnesses.into_iter().map(|w| kept[w]).collect(),
            },
            other => other,
        }
    }
}

/// A reduction under the program-level replay: the run's operations, the program's
/// order, the reduction and how many replays it ran.
pub struct ProgramReduction {
    pub ops: Operations,
    pub order: CausalOrder,
    pub reduction: Reduction,
    pub replays: u64,
}

/// Closure, then deletion with candidates from `mode`, under the program-level replay.
pub fn minimize_program(c: &Case, mode: Deletion) -> ProgramReduction {
    let ops = operations_of(c);
    let order = program_order(c, &ops);
    let mut oracle = ProgramOracle {
        case: c,
        ops: &ops,
        replays: 0,
    };
    let reduction = reduce::minimize(&order, mode, &mut oracle, BUDGET);
    let replays = oracle.replays;
    ProgramReduction {
        ops,
        order,
        reduction,
        replays,
    }
}

/// Deletion over atoms under the program-level replay, of M01's two witnesses.
pub fn agreement_program() -> &'static ProgramReduction {
    static CELL: OnceLock<ProgramReduction> = OnceLock::new();
    CELL.get_or_init(|| minimize_program(agreement_witness(), Deletion::Atoms))
}

pub fn acked_program() -> &'static ProgramReduction {
    static CELL: OnceLock<ProgramReduction> = OnceLock::new();
    CELL.get_or_init(|| minimize_program(acked_witness(), Deletion::Atoms))
}

/// Whether a core's causal story keeps M01's mechanism (`replicated_register.md`'s causal
/// core of ack-before-sync): a value is acknowledged over volatile bytes, that is, two
/// distinct replicas submitted it before its acknowledgement and fewer than two of them
/// synced it by then. Under `Agreement`, also: one of those unsynced submissions is lost
/// to a crash before that replica submits again, and the other value is acknowledged
/// after the loss. The first value's acknowledgement may come before or after the loss:
/// a coordinator can acknowledge after it, over a confirmation the loss made false.
pub fn keeps_mechanism(st: &[String], target: Target) -> bool {
    let field = |label: &str, key: &str| -> Option<String> {
        let inner = label.split_once('(')?.1.trim_end_matches(')');
        inner
            .split(',')
            .find_map(|p| p.strip_prefix(key).map(str::to_owned))
    };
    let kind = |label: &str| label.split_once('(').map_or("", |(k, _)| k).to_owned();
    let acks: Vec<(usize, String)> = st
        .iter()
        .enumerate()
        .filter(|(_, l)| l.starts_with("Ack("))
        .filter_map(|(i, l)| Some((i, field(l, "value=")?)))
        .collect();
    // The replicas that submitted `value` before `i` (their confirmations are sent), each
    // with where, and whether that submission was synced before `i` and before any loss.
    let submitted = |i: usize, value: &str| -> BTreeMap<String, (usize, bool)> {
        let mut out: BTreeMap<String, (usize, bool)> = BTreeMap::new();
        let mut lost: BTreeSet<String> = BTreeSet::new();
        for (k, l) in st[..i].iter().enumerate() {
            let Some(n) = field(l, "n=") else { continue };
            let of_value = field(l, "value=").as_deref() == Some(value);
            match kind(l).as_str() {
                "Submit" if of_value => {
                    out.insert(n.clone(), (k, false));
                    lost.remove(&n);
                }
                "Lose" if of_value => {
                    lost.insert(n);
                }
                "Sync" if !lost.contains(&n) => {
                    if let Some(slot) = out.get_mut(&n) {
                        slot.1 = true;
                    }
                }
                _ => {}
            }
        }
        out
    };
    acks.iter().any(|(i, value)| {
        let by = submitted(*i, value);
        let volatile: Vec<(&String, usize)> = by
            .iter()
            .filter(|(_, (_, synced))| !synced)
            .map(|(n, (k, _))| (n, *k))
            .collect();
        let over_volatile = by.len() >= 2 && by.len() - volatile.len() < 2;
        match target {
            Target::AckedNotDurable => over_volatile,
            Target::Agreement => {
                // A volatile submission of the acknowledged value, lost before the
                // replica submits again, and an acknowledgement of another value after
                // that loss.
                volatile.iter().any(|&(n, k)| {
                    let lost = st[k + 1..]
                        .iter()
                        .take_while(|l| {
                            !(kind(l) == "Submit" && field(l, "n=").as_ref() == Some(n))
                        })
                        .position(|l| {
                            kind(l) == "Lose"
                                && field(l, "n=").as_ref() == Some(n)
                                && field(l, "value=").as_deref() == Some(value.as_str())
                        })
                        .map(|p| k + 1 + p);
                    over_volatile
                        && lost.is_some_and(|at| acks.iter().any(|(j, w)| *j > at && w != value))
                })
            }
        }
    })
}

/// One transcript entry, as the retained artifact renders it.
pub fn attempt_line(a: &reduce::Attempt) -> String {
    let pass = a
        .pass
        .map_or_else(|| "start".to_owned(), |p| format!("{p:?}"));
    let verdict = match a.verdict {
        Verdict::Fails => "fails".to_owned(),
        Verdict::Holds => "holds".to_owned(),
        Verdict::Nonconforming => "not-a-run".to_owned(),
        Verdict::Inconclusive(r) => format!("inconclusive({r:?})"),
        Verdict::ContractBreach => "contract-breach".to_owned(),
        Verdict::Vacuous => "vacuous (empty, not replayed)".to_owned(),
    };
    let memo = a
        .memo_of
        .map_or_else(String::new, |m| format!(" memo-of #{m}"));
    let reason = if a.reason.is_empty() {
        String::new()
    } else if a.reason_bytes > a.reason.len() {
        format!(" -- {} [cut from {} bytes]", a.reason, a.reason_bytes)
    } else {
        format!(" -- {}", a.reason)
    };
    format!(
        "#{} {pass} {:?} v{} from {} n={} removed {:?} -> {} events: {verdict}{memo}{reason}",
        a.index, a.trial, a.version, a.from, a.granularity, a.removed, a.size
    )
}

/// The independent check of a minimality claim over atoms, from the transcript alone
/// (RFC 0028: the minimality classes are checked by their transcripts). It rebuilds
/// every version's set from the transcript's start set and the removals of its kept
/// candidates, checking each entry against the set it names; checks that the final set
/// is the core and that each memo entry's candidate is its original's; and then that
/// every atom of the core has a recorded removal of exactly that atom against the core
/// whose verdict is decided and not a failure. Returns the atoms checked, or what is
/// wrong.
pub fn transcript_licenses(
    order: &CausalOrder,
    core: &reduce::Core,
    attempts: &Attempts,
) -> Result<usize, String> {
    if attempts.omitted != 0 {
        return Err(format!("{} attempts omitted", attempts.omitted));
    }
    let mut sets: Vec<Vec<usize>> = vec![attempts.start.clone()];
    let mut candidates: BTreeMap<u64, Vec<usize>> = BTreeMap::new();
    // The final set's decided, non-failing removals, by removed set.
    let mut removals: BTreeSet<Vec<usize>> = BTreeSet::new();
    let mut last_version = 0;
    for a in &attempts.entries {
        let v = usize::try_from(a.version).map_err(|_| "version".to_owned())?;
        if v + 1 != sets.len() {
            return Err(format!("#{} is not tried against the latest set", a.index));
        }
        let set = &sets[v];
        if a.from != set.len() || !a.removed.iter().all(|e| set.binary_search(e).is_ok()) {
            return Err(format!("#{} does not match the set it names", a.index));
        }
        let candidate: Vec<usize> = set
            .iter()
            .copied()
            .filter(|e| a.removed.binary_search(e).is_err())
            .collect();
        if candidate.len() != a.size {
            return Err(format!("#{}: candidate size", a.index));
        }
        if let Some(m) = a.memo_of {
            if m >= a.index || candidates.get(&m) != Some(&candidate) {
                return Err(format!("#{} disagrees with its memo #{m}", a.index));
            }
        }
        candidates.insert(a.index, candidate.clone());
        if a.verdict == Verdict::Fails && a.trial != Trial::Start {
            sets.push(candidate);
            removals.clear();
        } else if a.trial == Trial::Remove
            && a.pass == Some(Pass::Deletion(Deletion::Atoms))
            && matches!(
                a.verdict,
                Verdict::Holds | Verdict::Nonconforming | Verdict::Vacuous
            )
        {
            removals.insert(a.removed.clone());
        }
        last_version = a.version;
    }
    if last_version != attempts.version || sets.len() as u64 != attempts.version + 1 {
        return Err("the result version does not match the transcript".to_owned());
    }
    if sets.last() != Some(&core.events) {
        return Err("the rebuilt final set is not the core".to_owned());
    }
    let atoms: BTreeSet<Vec<usize>> = core.events.iter().map(|&e| order.atom(e)).collect();
    for atom in &atoms {
        if !removals.contains(atom) {
            return Err(format!("no recorded non-failing removal of atom {atom:?}"));
        }
    }
    Ok(atoms.len())
}

/// The register observer's declared footprint of each event, for `target` (RFC 0028: an
/// abstraction-relevant hidden event stays in the core; INV-013: relevance is the named
/// observer's and property's). The projection reads state no journal event names: each
/// replica's storage slot `(node, epoch)`, which a crashed incarnation and its successor
/// both write, and each epoch's chosen value. So every projected `Reserve`, `Submit`,
/// `Sync`, `Abort` and `Lose` writes its slot, and every `Ack` writes its epoch's
/// choice. Under `AckedNotDurable`, whose predicate reads durability, an `Ack` also reads
/// its epoch's slots; `Agreement` reads only the choices.
/// The replicas' names, as the projection's labels spell them.
pub const NODES: [&str; 3] = ["a", "b", "c"];

pub fn observer_footprints(journal: &Journal, roles: &Roles, target: Target) -> Vec<Footprint> {
    // Disjoint key spaces: a tag in the top byte, then node and epoch in a byte each.
    const SLOT: u64 = 1 << 56;
    const CHOSEN: u64 = 2 << 56;
    let slot = |node: u64, epoch: u64| Key::Observer(SLOT | (node << 8) | epoch);
    let mut out: Vec<Footprint> = vec![Vec::new(); journal.len()];
    let steps = register::observe(roles, journal).expect("the original run projects");
    for s in steps {
        let label = match s.expect {
            Expect::Stutter => continue,
            Expect::Step(l) | Expect::StepPrefix(l) => l,
        };
        let (kind, rest) = label.split_once('(').expect("a step label");
        let mut node = None;
        let mut epoch = None;
        for part in rest.trim_end_matches(')').split(',') {
            if let Some(n) = part.strip_prefix("n=") {
                node = NODES.iter().position(|x| *x == n);
            } else if let Some(e) = part.strip_prefix("epoch=") {
                epoch = e.parse::<u64>().ok();
            }
        }
        let epoch = epoch.expect("every step names its epoch");
        assert!(epoch < 256, "an epoch fits its key byte");
        let entry = &mut out[s.seq as usize];
        if kind == "Ack" {
            entry.push((Key::Observer(CHOSEN | epoch), Access::Write));
            if target == Target::AckedNotDurable {
                for n in 0..NODES.len() as u64 {
                    entry.push((slot(n, epoch), Access::Read));
                }
            }
        } else {
            let node = node.expect("a slot step names its node") as u64;
            entry.push((slot(node, epoch), Access::Write));
        }
    }
    for f in &mut out {
        f.sort_unstable();
        f.dedup();
    }
    out
}

pub fn order_of(c: &Case) -> CausalOrder {
    CausalOrder::with_atoms(
        causal::predecessors_with(&c.journal, CAUSAL_WORK, &c.observer)
            .expect("within the work bound"),
        causal::atoms(&c.journal),
    )
    .expect("a causal order")
}

/// One failing run: its plan, log, journal, roles and target.
#[derive(Clone)]
pub struct Case {
    pub label: String,
    pub built: Built,
    pub log: ChoiceLog,
    pub journal: Journal,
    /// The register observer's footprints ([`observer_footprints`]).
    pub observer: Vec<Footprint>,
    pub target: Target,
    /// The plan's epochs.
    pub epochs: u8,
}

/// The run of `plan` under `log`, when it fails: its target is `want` when the run
/// shows that finding, otherwise its first target finding.
pub fn case(label: String, plan: &Plan, log: &ChoiceLog, want: Option<Target>) -> Option<Case> {
    let built = register::build_with_shutdown(plan);
    let reach = register::reachable(plan.epochs);
    case_built(label, built, plan.epochs, log, want, &reach)
}

/// [`case`] of an already built program with `epochs` epochs, given the durable register's
/// reachable states `reach` (PR 18's scenario reduction runs pruned builds, bn-25z9o).
pub fn case_built(
    label: String,
    built: Built,
    epochs: u8,
    log: &ChoiceLog,
    want: Option<Target>,
    reach: &BTreeSet<Raw>,
) -> Option<Case> {
    let report = baseline::run_one(&built, epochs, log, reach);
    let found: Vec<Target> = report
        .findings
        .iter()
        .filter_map(|f| match f {
            baseline::Finding::Agreement(_) => Some(Target::Agreement),
            baseline::Finding::AckedNotDurable(_) => Some(Target::AckedNotDurable),
            _ => None,
        })
        .collect();
    let target = match want {
        Some(t) if found.contains(&t) => t,
        Some(_) => return None,
        None => *found.first()?,
    };
    let journal = run(&built.programs, log, &baseline::config()).expect("a journal");
    let observer = observer_footprints(&journal, &built.roles, target);
    Some(Case {
        label,
        built,
        log: log.clone(),
        journal,
        observer,
        target,
        epochs,
    })
}

/// M01's plan `plan` of group `group`, and that group's logs, as the M01 campaign draws
/// them (`register_mutants::mutant(M01, baseline())`).
pub fn m01_plan_and_logs(group: usize, plan: usize) -> (Plan, Vec<ChoiceLog>) {
    let base = baseline::baseline();
    let m = mutants::mutant(mutants::Id::M01, &base).expect("M01");
    let p = m.campaign.groups[group].plans[plan].clone();
    let built = register::build_with_shutdown(&p);
    let logs = baseline::logs_for(
        &built,
        m.campaign.groups[group].logs,
        m.campaign.plan_seed(group, plan),
    )
    .1;
    (p, logs)
}

/// M01's second witness in `pr16_impl05_mutants.evidence.txt`: scenario plan 0 log 146,
/// `Agreement(99)` at event 99 of 137.
pub fn agreement_witness() -> &'static Case {
    static CELL: OnceLock<Case> = OnceLock::new();
    CELL.get_or_init(|| {
        let (plan, logs) = m01_plan_and_logs(0, 0);
        let c = case(
            "scenario plan 0 log 146".to_owned(),
            &plan,
            &logs[146],
            Some(Target::Agreement),
        )
        .expect("fails");
        assert_eq!(c.target, Target::Agreement);
        c
    })
}

/// M01's first witness there: sweep-one-epoch plan 81 log 5, `AckedNotDurable(43)` at
/// event 43 of 97.
pub fn acked_witness() -> &'static Case {
    static CELL: OnceLock<Case> = OnceLock::new();
    CELL.get_or_init(|| {
        let (plan, logs) = m01_plan_and_logs(1, 81);
        let c = case(
            "sweep-one-epoch plan 81 log 5".to_owned(),
            &plan,
            &logs[5],
            Some(Target::AckedNotDurable),
        )
        .expect("fails");
        assert_eq!(c.target, Target::AckedNotDurable);
        c
    })
}

/// Every failing run of M01's scenario plan: the corpus.
pub fn corpus() -> &'static Vec<Case> {
    static CELL: OnceLock<Vec<Case>> = OnceLock::new();
    CELL.get_or_init(|| {
        let (plan, logs) = m01_plan_and_logs(0, 0);
        logs.iter()
            .enumerate()
            .filter_map(|(i, log)| case(format!("scenario plan 0 log {i}"), &plan, log, None))
            .collect()
    })
}

pub fn minimize(c: &Case, mode: Deletion) -> (Reduction, u64) {
    let order = order_of(c);
    let mut oracle = RegisterOracle::new(&c.journal, &c.built.roles, c.target);
    let r = reduce::minimize(&order, mode, &mut oracle, BUDGET);
    (r, oracle.replays)
}

pub fn core_journal(c: &Case, events: &[usize]) -> Journal {
    causal::restrict(&c.journal, events)
        .expect("restricts")
        .journal
}

/// The durable-register steps of a restricted journal that are not stutters.
pub fn story(c: &Case, events: &[usize]) -> Vec<String> {
    let r = causal::restrict(&c.journal, events).expect("restricts");
    let roles = carry_roles(&c.built.roles, &r.renaming);
    register::observe(&roles, &r.journal)
        .expect("projects")
        .into_iter()
        .filter_map(|s| match s.expect {
            Expect::Stutter => None,
            Expect::Step(l) | Expect::StepPrefix(l) => Some(l),
        })
        .collect()
}

/// Independent reachability: pairwise footprint dependence over every pair of events
/// (no use of the predecessor lists the reduction reads), walked backwards from the
/// witnesses. Returns `(hb, closed)`: the witnesses' happens-before past alone, and that
/// past closed under the journal's atoms as well, which is what a configuration must
/// hold.
pub fn naive_past(c: &Case, witnesses: &[usize]) -> (BTreeSet<usize>, BTreeSet<usize>) {
    let prints =
        causal::footprints_with(&c.journal, CAUSAL_WORK, &c.observer).expect("within the bound");
    let n = prints.len();
    let mut back: Vec<Vec<usize>> = vec![Vec::new(); n];
    for b in 0..n {
        for a in 0..b {
            if causal::dependent(&prints[a], &prints[b]) {
                back[b].push(a);
            }
        }
    }
    let walk = |with_atoms: bool| {
        let mut mate: Vec<Vec<usize>> = vec![Vec::new(); n];
        if with_atoms {
            for atom in causal::atoms(&c.journal) {
                for &x in &atom {
                    mate[x] = atom.clone();
                }
            }
        }
        let mut seen: BTreeSet<usize> = witnesses.iter().copied().collect();
        let mut stack: Vec<usize> = witnesses.to_vec();
        while let Some(x) = stack.pop() {
            for &y in back[x].iter().chain(&mate[x]) {
                if seen.insert(y) {
                    stack.push(y);
                }
            }
        }
        seen
    };
    (walk(false), walk(true))
}

pub fn median(mut v: Vec<f64>) -> f64 {
    v.sort_by(f64::total_cmp);
    if v.is_empty() {
        return 0.0;
    }
    let m = v.len() / 2;
    if v.len() % 2 == 1 {
        v[m]
    } else {
        f64::midpoint(v[m - 1], v[m])
    }
}
