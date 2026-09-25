//! PR 18 (bn-3km4z): deletion and causal-closure reduction over the replicated register's
//! failing journals. START_HERE PR 18 ("Implement deletion, causal-closure, …, and replay
//! validation"; exit: "ack-before-sync failure reduces to a compact core and does not
//! delete the actual causal mechanism").
//!
//! # The instantiation
//!
//! `continuum_debugger::reduce` is generic over a trace with a happens-before relation
//! and a replay oracle. Here the trace is a PR-14 semantic journal of the replicated
//! register (PR 16), the relation is `continuum_asupersync::causal::predecessors` (the
//! journal's declared footprint dependence), and the oracle is [`RegisterOracle`]: a
//! candidate configuration is restricted to a sub-journal (`causal::restrict`, ordinals
//! renamed), and that sub-journal is **replayed** through the same checks a campaign run
//! gets — the lift must conform, the A7 model must accept, the projection must accept
//! with the role table carried across the renaming — and the target property is checked
//! on the projected states. A candidate that fails any of the first three is not
//! replayable and is never kept. The oracle assumes nothing about a candidate.
//!
//! Replay here is replay of the event set under the journal's semantics (the lift, the
//! A7 model and the projection), research/26's "replay/partial execution reaches a
//! violating monitor state". It is not a re-run of the substrate: an arbitrary subset of
//! events has no choice log. The original run is replayed through the binding from its
//! plan and log before any reduction.
//!
//! # Which deletion the exit rests on
//!
//! The deletion pass runs over two candidate spaces (`reduce::Deletion`). Over
//! configurations, every candidate keeps the happens-before past of what it keeps, so
//! the core keeps the mechanism: M01's cores keep the confirmations, the submits and the
//! loss. Over arbitrary unions of atoms (classic `ddmin`) the cores are far smaller and
//! still replay to the failure, but this replay is the substrate's semantics, which does
//! not know that a coordinator acknowledges only after a majority confirms, so those
//! cores drop the mechanism. `deletion_over_atoms_loses_the_mechanism_under_a_substrate_replay`
//! pins that finding. The exit's core is deletion over configurations.
//!
//! # The evidence, by stable artifact ID
//!
//! Everything renders into `tests/golden/pr18_impl01_reduction.evidence.txt`. Regenerate
//! with `PR18_IMPL01_BLESS=1 cargo test -p continuum-asupersync --test
//! pr18_impl01_reduction` and review the diff.
//!
//! - `pr18-impl01-01-m01-agreement-core`: M01's scenario witness (`abstract_register::
//!   Agreement`), event counts before and after each pass, the core and its causal story;
//! - `pr18-impl01-02-m01-acked-not-durable-core`: M01's shallowest `RuntimeToAbstract`
//!   witness (`AckedNotDurable`), the same;
//! - `pr18-impl01-03-closure-preservation`: the corpus property over every failing run of
//!   M01's scenario plan;
//! - `pr18-impl01-04-determinism`: identical inputs give byte-identical cores;
//! - `pr18-impl01-05-symmetry-renaming`: the metamorphic relation.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::sync::OnceLock;

#[path = "support/primitive_conformance_model.rs"]
#[allow(dead_code)]
mod model;

#[path = "support/replicated_register.rs"]
#[allow(dead_code)]
mod register;

#[path = "support/register_baseline.rs"]
#[allow(dead_code)]
mod baseline;

#[path = "support/register_mutants.rs"]
#[allow(dead_code)]
mod mutants;

use continuum_asupersync::binding::run;
use continuum_asupersync::causal::{self, Access, Footprint, Key, Renaming, Restriction};
use continuum_asupersync::choice::ChoiceLog;
use continuum_asupersync::family::EventBody;
use continuum_asupersync::family::lifecycle::LifecycleEvent;
use continuum_asupersync::journal::Journal;
use continuum_asupersync::lift::{LiftVerdict, lift};
use continuum_debugger::reduce::{
    self, Budget, CausalOrder, Deletion, Guarantee, NotReplayable, Pass, PassEnd, Reduction,
    Replay, Replayed,
};
use model::{Alphabet, FamilyTag, judge};
use register::{Built, Expect, Plan, Raw, Role, Roles};

/// The work bound for deriving a journal's relation: far above any register journal.
const CAUSAL_WORK: u64 = 1 << 24;

/// The reduction budget every test here grants: replays and work units.
const BUDGET: Budget = Budget::new(4_096, 1 << 26);

/// The failure a reduction preserves: the finding kind the original run showed first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Target {
    /// `abstract_register::Agreement`: two values acknowledged for one epoch.
    Agreement,
    /// `RuntimeToAbstract`'s `AckedNotDurable`: an acknowledgement with no durable
    /// majority.
    AckedNotDurable,
}

impl Target {
    const fn name(self) -> &'static str {
        match self {
            Self::Agreement => "abstract_register::Agreement",
            Self::AckedNotDurable => "RuntimeToAbstract/AckedNotDurable",
        }
    }

    /// The acknowledged `(epoch, value)` pairs that make `raw` violate the target.
    fn offending(self, raw: &Raw) -> BTreeSet<(u8, u8)> {
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

/// `roles` carried across a restriction: each kept task keeps its role, its region is
/// renamed, and the region count is the number of the setup's regions the restriction
/// kept. A task whose region the restriction dropped gets a region no journal names, so
/// the projection refuses the candidate rather than guessing.
fn carry_roles(roles: &Roles, renaming: &Renaming) -> Roles {
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
struct RegisterOracle<'a> {
    journal: &'a Journal,
    roles: &'a Roles,
    target: Target,
    /// Replays run: the determinism test compares it.
    replays: u64,
}

impl<'a> RegisterOracle<'a> {
    fn new(journal: &'a Journal, roles: &'a Roles, target: Target) -> Self {
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
fn replay_restricted(r: &Restriction, roles: &Roles, target: Target) -> Replayed {
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

/// The register observer's declared footprint of each event, for `target` (RFC 0028: an
/// abstraction-relevant hidden event stays in the core; INV-013: relevance is the named
/// observer's and property's). The projection reads state no journal event names: each
/// replica's storage slot `(node, epoch)`, which a crashed incarnation and its successor
/// both write, and each epoch's chosen value. So every projected `Reserve`, `Submit`,
/// `Sync`, `Abort` and `Lose` writes its slot, and every `Ack` writes its epoch's
/// choice. Under `AckedNotDurable`, whose predicate reads durability, an `Ack` also reads
/// its epoch's slots; `Agreement` reads only the choices.
/// The replicas' names, as the projection's labels spell them.
const NODES: [&str; 3] = ["a", "b", "c"];

fn observer_footprints(journal: &Journal, roles: &Roles, target: Target) -> Vec<Footprint> {
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

fn order_of(c: &Case) -> CausalOrder {
    CausalOrder::with_atoms(
        causal::predecessors_with(&c.journal, CAUSAL_WORK, &c.observer)
            .expect("within the work bound"),
        causal::atoms(&c.journal),
    )
    .expect("a causal order")
}

/// One failing run: its plan, log, journal, roles and target.
#[derive(Clone)]
struct Case {
    label: String,
    built: Built,
    log: ChoiceLog,
    journal: Journal,
    /// The register observer's footprints ([`observer_footprints`]).
    observer: Vec<Footprint>,
    target: Target,
}

/// The run of `plan` under `log`, when it fails: its target is `want` when the run
/// shows that finding, otherwise its first target finding.
fn case(label: String, plan: &Plan, log: &ChoiceLog, want: Option<Target>) -> Option<Case> {
    let built = register::build_with_shutdown(plan);
    let reach = register::reachable(plan.epochs);
    let report = baseline::run_one(&built, plan.epochs, log, &reach);
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
    })
}

/// M01's plan `plan` of group `group`, and that group's logs, as the M01 campaign draws
/// them (`register_mutants::mutant(M01, baseline())`).
fn m01_plan_and_logs(group: usize, plan: usize) -> (Plan, Vec<ChoiceLog>) {
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
fn agreement_witness() -> &'static Case {
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
fn acked_witness() -> &'static Case {
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
fn corpus() -> &'static Vec<Case> {
    static CELL: OnceLock<Vec<Case>> = OnceLock::new();
    CELL.get_or_init(|| {
        let (plan, logs) = m01_plan_and_logs(0, 0);
        logs.iter()
            .enumerate()
            .filter_map(|(i, log)| case(format!("scenario plan 0 log {i}"), &plan, log, None))
            .collect()
    })
}

fn minimize(c: &Case, mode: Deletion) -> (Reduction, u64) {
    let order = order_of(c);
    let mut oracle = RegisterOracle::new(&c.journal, &c.built.roles, c.target);
    let r = reduce::minimize(&order, mode, &mut oracle, BUDGET);
    (r, oracle.replays)
}

fn core_journal(c: &Case, events: &[usize]) -> Journal {
    causal::restrict(&c.journal, events)
        .expect("restricts")
        .journal
}

/// The durable-register steps of a restricted journal that are not stutters.
fn story(c: &Case, events: &[usize]) -> Vec<String> {
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
fn naive_past(c: &Case, witnesses: &[usize]) -> (BTreeSet<usize>, BTreeSet<usize>) {
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

fn median(mut v: Vec<f64>) -> f64 {
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

fn transcript_lines(r: &Reduction) -> Vec<String> {
    let (transcript, spent) = match r {
        Reduction::Reduced {
            transcript, spent, ..
        }
        | Reduction::Inconclusive {
            transcript, spent, ..
        } => (transcript, spent),
        Reduction::Refused(why) => return vec![format!("refused: {why:?}")],
    };
    let mut out: Vec<String> = transcript
        .iter()
        .map(|p| {
            format!(
                "pass {:?}: {} -> {} events, {} replays ({} held, {} nonconforming, {} inconclusive), {:?}",
                p.pass, p.before, p.after, p.replays, p.held, p.nonconforming, p.inconclusive, p.end
            )
        })
        .collect();
    out.push(format!(
        "spent: {} replays, {} work units",
        spent.replays, spent.work
    ));
    out
}

fn core_section(id: &str, c: &Case) -> String {
    let mut s = String::new();
    let _ = writeln!(s, "[{id}]");
    let _ = writeln!(s, "run: M01 {}, target {}", c.label, c.target.name());
    let _ = writeln!(
        s,
        "  log {:?}",
        c.log.choices().iter().map(|x| x.0).collect::<Vec<_>>()
    );
    let _ = writeln!(
        s,
        "journal: {} events, digest {}",
        c.journal.len(),
        c.journal.digest().expect("digests")
    );
    for mode in [Deletion::Configurations, Deletion::Atoms] {
        let (r, _) = minimize(c, mode);
        let core = r.core().expect("reduced");
        let _ = writeln!(s, "reduction with deletion over {mode:?}:");
        for line in transcript_lines(&r) {
            let _ = writeln!(s, "  {line}");
        }
        let cj = core_journal(c, &core.events);
        let _ = writeln!(
            s,
            "  core: {} of {} events ({:.1}%), guarantees {:?}, digest {}",
            core.events.len(),
            c.journal.len(),
            100.0 * core.events.len() as f64 / c.journal.len() as f64,
            core.guarantees,
            cj.digest().expect("digests")
        );
        let _ = writeln!(s, "  core events (original seq): {:?}", core.events);
        let _ = writeln!(s, "  witnesses (original seq): {:?}", core.witnesses);
        let _ = writeln!(s, "  causal story: {}", story(c, &core.events).join(" "));
        for line in cj.render().lines() {
            let _ = writeln!(s, "    {line}");
        }
    }
    s
}

/// The corpus property's numbers, computed once.
struct CorpusFacts {
    runs: usize,
    by_target: BTreeMap<Target, usize>,
    trace_len: Vec<usize>,
    closure_len: Vec<usize>,
    /// Core lengths: deletion over configurations, then over atoms.
    core_len: [Vec<usize>; 2],
    closure_not_preserving: usize,
    dropped_checked: usize,
    on_path: usize,
    atom_mates: usize,
    /// Cores of deletion over atoms whose story has no `Submit`: the mechanism lost.
    atoms_without_submit: usize,
    replays: u64,
}

fn corpus_facts() -> &'static CorpusFacts {
    static CELL: OnceLock<CorpusFacts> = OnceLock::new();
    CELL.get_or_init(|| {
        let mut f = CorpusFacts {
            runs: 0,
            by_target: BTreeMap::new(),
            trace_len: Vec::new(),
            closure_len: Vec::new(),
            core_len: [Vec::new(), Vec::new()],
            closure_not_preserving: 0,
            dropped_checked: 0,
            on_path: 0,
            atom_mates: 0,
            atoms_without_submit: 0,
            replays: 0,
        };
        for c in corpus() {
            f.runs += 1;
            *f.by_target.entry(c.target).or_default() += 1;
            let order = order_of(c);
            let whole: Vec<usize> = (0..c.journal.len()).collect();
            // The anchors: the whole run's own replay's witnesses.
            let mut oracle = RegisterOracle::new(&c.journal, &c.built.roles, c.target);
            let Replayed::Fails { witnesses } = oracle.replay(&whole) else {
                panic!("{}: the whole journal replays to its failure", c.label);
            };
            let closed = reduce::closure_pass(&order, &mut oracle, BUDGET);
            let Reduction::Reduced {
                core, transcript, ..
            } = &closed
            else {
                panic!("{}: closure finished: {closed:?}", c.label);
            };
            if matches!(transcript[0].end, PassEnd::ClosureNotReplayPreserving(_)) {
                eprintln!("{}: {:?}", c.label, transcript[0].end);
                f.closure_not_preserving += 1;
                continue;
            }
            // Every kept core still fails, on a fresh, independent replay.
            let mut fresh = RegisterOracle::new(&c.journal, &c.built.roles, c.target);
            assert!(
                matches!(fresh.replay(&core.events), Replayed::Fails { .. }),
                "{}: the closure core fails",
                c.label
            );
            assert!(core.guarantees.contains(&Guarantee::CausallyClosed));
            // Against the independent reachability: no event on a happens-before path to
            // a witness is dropped, and the kept set is exactly that past closed under the
            // journal's atoms.
            let (hb, closed) = naive_past(c, &witnesses);
            let kept: BTreeSet<usize> = core.events.iter().copied().collect();
            assert!(
                hb.is_subset(&kept),
                "{}: closure dropped a happens-before predecessor",
                c.label
            );
            assert_eq!(kept, closed, "{}: closure is the atom-closed past", c.label);
            f.on_path += hb.len();
            f.atom_mates += kept.len() - hb.len();
            f.dropped_checked += c.journal.len() - kept.len();
            // Then each deletion pass: its core also fails on a fresh replay.
            f.trace_len.push(c.journal.len());
            f.closure_len.push(core.events.len());
            for (k, mode) in [Deletion::Configurations, Deletion::Atoms]
                .into_iter()
                .enumerate()
            {
                let (full, replays) = minimize(c, mode);
                f.replays += replays;
                let core2 = full.core().expect("reduced");
                assert!(
                    matches!(fresh.replay(&core2.events), Replayed::Fails { .. }),
                    "{}: the {mode:?} deletion core fails",
                    c.label
                );
                if mode == Deletion::Configurations {
                    assert!(core2.guarantees.contains(&Guarantee::CausallyMinimal));
                    assert!(order.is_down_closed(&core2.events));
                } else if !story(c, &core2.events)
                    .iter()
                    .any(|l| l.starts_with("Submit("))
                {
                    f.atoms_without_submit += 1;
                }
                f.core_len[k].push(core2.events.len());
            }
        }
        f
    })
}

fn corpus_section() -> String {
    let f = corpus_facts();
    let ratio = |v: &[usize]| {
        median(
            v.iter()
                .zip(&f.trace_len)
                .map(|(a, b)| *a as f64 / *b as f64)
                .collect(),
        )
    };
    let mut s = String::new();
    let _ = writeln!(s, "[pr18-impl01-03-closure-preservation]");
    let _ = writeln!(
        s,
        "corpus: every failing run of M01's scenario plan ({} of {} logs), targets {:?}",
        f.runs,
        baseline::SCENARIO_LOGS,
        f.by_target
            .iter()
            .map(|(t, n)| format!("{}={n}", t.name()))
            .collect::<Vec<_>>()
    );
    let _ = writeln!(
        s,
        "closure: kept {} events on a happens-before path to a witness and {} more that share an operation atom with one; dropped {}, none on such a path; checked against pairwise footprint dependence walked apart from the predecessor lists and the reduction; not replay-preserving on {} runs",
        f.on_path, f.atom_mates, f.dropped_checked, f.closure_not_preserving
    );
    let _ = writeln!(
        s,
        "every closure core and every deletion core fails on a fresh replay; every closure core is causally closed; every core of deletion over configurations is causally closed and causally minimal"
    );
    let med = |v: &[usize]| median(v.iter().map(|x| *x as f64).collect());
    let _ = writeln!(
        s,
        "median length: trace {}, closure {} ({:.1}% of trace), core after deletion over configurations {} ({:.1}%), core after deletion over atoms {} ({:.1}%); {} replays in all",
        med(&f.trace_len),
        med(&f.closure_len),
        100.0 * ratio(&f.closure_len),
        med(&f.core_len[0]),
        100.0 * ratio(&f.core_len[0]),
        med(&f.core_len[1]),
        100.0 * ratio(&f.core_len[1]),
        f.replays
    );
    let _ = writeln!(
        s,
        "deletion over atoms: {} of {} cores keep no Submit, so their acknowledgement is not over any write: the substrate replay accepts a journal the program cannot produce, and the mechanism is lost; the causal core is the one of deletion over configurations",
        f.atoms_without_submit, f.runs
    );
    s
}

fn determinism_section() -> String {
    let c = agreement_witness();
    // A fresh run of the binding from the same plan and log.
    let (plan, logs) = m01_plan_and_logs(0, 0);
    let again = case(
        "again".to_owned(),
        &plan,
        &logs[146],
        Some(Target::Agreement),
    )
    .expect("fails");
    assert_eq!(
        again.journal.encode().expect("encodes"),
        c.journal.encode().expect("encodes")
    );
    let (a, ra) = minimize(c, Deletion::Configurations);
    let (b, rb) = minimize(&again, Deletion::Configurations);
    assert_eq!(a, b, "identical inputs give identical reductions");
    assert_eq!(ra, rb);
    let ca = core_journal(c, &a.core().expect("core").events)
        .encode()
        .expect("encodes");
    let cb = core_journal(&again, &b.core().expect("core").events)
        .encode()
        .expect("encodes");
    assert_eq!(ca, cb, "byte-identical cores");
    let mut s = String::new();
    let _ = writeln!(s, "[pr18-impl01-04-determinism]");
    let _ = writeln!(
        s,
        "M01 scenario plan 0 log 146, run twice through the binding and reduced twice: identical reductions ({ra} replays each) and byte-identical core encodings ({} bytes)",
        ca.len()
    );
    s
}

/// The value symmetry: swap v0 and v1 in every role. The journal carries no value, and
/// both targets are symmetric in values, so the core must not move.
fn swap_values(roles: &Roles) -> Roles {
    let swap = |v: u8| 1 - v;
    Roles {
        tasks: roles
            .tasks
            .iter()
            .map(|&(role, region)| {
                let role = match role {
                    Role::Writer { node, epoch, value } => Role::Writer {
                        node,
                        epoch,
                        value: swap(value),
                    },
                    Role::Coordinator { epoch, value } => Role::Coordinator {
                        epoch,
                        value: swap(value),
                    },
                    other => other,
                };
                (role, region)
            })
            .collect(),
        regions: roles.regions,
        mailboxes: roles.mailboxes.clone(),
    }
}

fn symmetry_section() -> String {
    let mut s = String::new();
    let _ = writeln!(s, "[pr18-impl01-05-symmetry-renaming]");
    for c in [agreement_witness(), acked_witness()] {
        let order = order_of(c);
        let swapped = swap_values(&c.built.roles);
        assert_ne!(swapped, c.built.roles);
        let mut o1 = RegisterOracle::new(&c.journal, &c.built.roles, c.target);
        let mut o2 = RegisterOracle::new(&c.journal, &swapped, c.target);
        let a = reduce::minimize(&order, Deletion::Configurations, &mut o1, BUDGET);
        let b = reduce::minimize(&order, Deletion::Configurations, &mut o2, BUDGET);
        assert_eq!(a, b, "{}: the value swap moves nothing", c.label);
        let _ = writeln!(
            s,
            "M01 {}: roles with v0 and v1 swapped give the identical reduction, core {} events",
            c.label,
            a.core().expect("core").events.len()
        );
    }
    s
}

fn evidence() -> String {
    let mut s = String::new();
    s.push_str(
        "# PR 18 (bn-3km4z): deletion and causal-closure reduction over the replicated register's failing journals.\n\
         # Regenerate: PR18_IMPL01_BLESS=1 cargo test -p continuum-asupersync --test pr18_impl01_reduction\n\
         # Causality: continuum_asupersync::causal's declared footprint dependence. Replay: lift, A7 model, projection, target property.\n\n",
    );
    s.push_str(&core_section(
        "pr18-impl01-01-m01-agreement-core",
        agreement_witness(),
    ));
    s.push('\n');
    s.push_str(&core_section(
        "pr18-impl01-02-m01-acked-not-durable-core",
        acked_witness(),
    ));
    s.push('\n');
    s.push_str(&corpus_section());
    s.push('\n');
    s.push_str(&determinism_section());
    s.push('\n');
    s.push_str(&symmetry_section());
    s
}

// ---------------------------------------------------------------------------
// tests
// ---------------------------------------------------------------------------

/// The witnesses are the ones `pr16_impl05_mutants.evidence.txt` records.
#[test]
fn the_witnesses_are_the_mutant_campaigns() {
    let a = agreement_witness();
    assert_eq!(a.journal.len(), 137);
    let acked = acked_witness();
    assert_eq!(acked.journal.len(), 97);
}

/// The events up to and including the last witness: the failing prefix a plain
/// truncation would keep.
fn failing_prefix(core: &reduce::Core) -> usize {
    core.witnesses.iter().max().map_or(0, |w| w + 1)
}

/// Exit: M01's ack-before-sync Agreement failure reduces to a causal core that keeps the
/// mechanism: `v0` acknowledged over volatile bytes, the crash loses them, `v1`
/// acknowledged. The core is smaller than the trace and than its failing prefix, is
/// replay-preserving, causally closed and causally minimal (1-minimal among configurations).
#[test]
fn m01_agreement_failure_reduces_to_a_causal_core_with_its_mechanism() {
    let c = agreement_witness();
    let (r, _) = minimize(c, Deletion::Configurations);
    let core = r.core().expect("reduced");
    assert!(core.events.len() < failing_prefix(core));
    assert!(core.events.len() < c.journal.len());
    assert_eq!(
        core.guarantees,
        vec![
            Guarantee::ReplayPreserving,
            Guarantee::CausallyClosed,
            Guarantee::CausallyMinimal
        ]
    );
    let st = story(c, &core.events);
    let at = |l: &str| st.iter().position(|s| s == l);
    let first = at("Ack(epoch=0,value=v0)").expect("v0 acked");
    let lost = at("Lose(n=a,epoch=0,value=v0)").expect("a's bytes lost");
    let second = at("Ack(epoch=0,value=v1)").expect("v1 acked");
    assert!(first < lost && lost < second, "{st:?}");
    // The v0 ack is over volatile bytes: no sync precedes it in the core.
    assert!(
        !st[..first].iter().any(|s| s.starts_with("Sync(")),
        "{st:?}"
    );
}

/// M01's RuntimeToAbstract failure reduces too: the core acknowledges `v0` over two
/// submits and no sync.
#[test]
fn m01_acked_not_durable_failure_reduces_to_a_causal_core() {
    let c = acked_witness();
    let (r, _) = minimize(c, Deletion::Configurations);
    let core = r.core().expect("reduced");
    assert!(core.events.len() < failing_prefix(core));
    assert!(core.guarantees.contains(&Guarantee::CausallyClosed));
    let st = story(c, &core.events);
    assert_eq!(
        st.iter().filter(|s| s.starts_with("Submit(")).count(),
        2,
        "{st:?}"
    );
    assert!(st.iter().any(|s| s.starts_with("Ack(")), "{st:?}");
    assert!(!st.iter().any(|s| s.starts_with("Sync(")), "{st:?}");
}

/// The negative result that decides which deletion the exit rests on. Deletion over
/// arbitrary unions of atoms (classic `ddmin`) goes far smaller, and every core it keeps
/// replays to the failure — but the replay here is the substrate's semantics (lift, A7
/// model, projection), which knows nothing of the program's logic. So it keeps two
/// coordinators' acknowledgements and drops the confirmations, the submits and the loss
/// that make them M01's: it deletes the mechanism. Deletion over configurations keeps
/// every happens-before predecessor of the failure and cannot. If a program-level replay
/// ever backs the oracle, this pin flips and the finding is revisited.
#[test]
fn deletion_over_atoms_loses_the_mechanism_under_a_substrate_replay() {
    let c = agreement_witness();
    let (r, _) = minimize(c, Deletion::Atoms);
    let core = r.core().expect("reduced");
    assert!(!core.guarantees.contains(&Guarantee::CausallyClosed));
    let st = story(c, &core.events);
    assert!(st.iter().all(|s| s.starts_with("Ack(")), "{st:?}");
    assert!(!st.iter().any(|s| s.starts_with("Lose(")), "{st:?}");
}

/// Closure preservation over the corpus, and the differential: the reduction's closure
/// (`continuum_debugger::reduce`, over `causal::predecessors_with`'s lists) against
/// pairwise reachability computed apart from both, over every pair of
/// `continuum_asupersync::causal`'s footprints. The footprints themselves are held to
/// the lift by `independent_adjacent_swaps_of_every_corpus_journal_still_lift`. Every kept core still fails; every
/// dropped event is on no happens-before path to a witness; every kept one is.
#[test]
fn closure_keeps_exactly_the_happens_before_past_over_the_corpus() {
    let f = corpus_facts();
    assert!(f.runs > 0);
    assert_eq!(f.closure_not_preserving, 0);
    assert!(f.dropped_checked > 0 && f.on_path > 0);
}

/// The declared footprints over-approximate the lift's own order: in every fourth
/// journal of the corpus, every adjacent pair of events the footprints call independent
/// is swapped (`causal::permute`, ordinals renamed in the new order), and the swapped
/// journal still lifts as a conforming run. A missing dependence the lift enforces would
/// show here as a refused swap. Adjacent swaps only: a sampled check of the relation
/// against the semantics, not a proof.
#[test]
fn independent_adjacent_swaps_of_every_corpus_journal_still_lift() {
    let mut swaps = 0_usize;
    for c in corpus().iter().step_by(4) {
        let prints = causal::footprints(&c.journal, CAUSAL_WORK).expect("within the bound");
        let n = c.journal.len();
        for i in 0..n - 1 {
            if causal::dependent(&prints[i], &prints[i + 1]) {
                continue;
            }
            let mut seq: Vec<usize> = (0..n).collect();
            seq.swap(i, i + 1);
            let swapped = causal::permute(&c.journal, &seq).expect("permutes");
            let verdict = lift(&swapped.journal);
            assert!(
                matches!(verdict, LiftVerdict::Conforms(_)),
                "{}: swapping independent events {i} and {} gives {verdict:?}",
                c.label,
                i + 1
            );
            swaps += 1;
        }
    }
    assert!(swaps > 1_000, "{swaps} swaps");
}

/// INV-006: identical input traces give byte-identical cores.
#[test]
fn identical_inputs_give_byte_identical_cores() {
    let _ = determinism_section();
}

/// Metamorphic, symmetry renaming: swapping the values in the role table (an
/// event-irrelevant renaming: no journal event names a value) gives the identical core.
#[test]
fn symmetry_renaming_of_values_leaves_the_core_unchanged() {
    let _ = symmetry_section();
}

/// A candidate that cannot be replayed is not kept: dropping a spawn but keeping its
/// task's steps is refused by the lift, and the reduction never returns such a set.
#[test]
fn an_unreplayable_candidate_is_refused_not_kept() {
    let c = agreement_witness();
    let spawn = c
        .journal
        .events()
        .iter()
        .position(|e| {
            matches!(
                e.body(),
                EventBody::Lifecycle(LifecycleEvent::TaskSpawned { .. })
            )
        })
        .expect("a spawn");
    let kept: Vec<usize> = (0..c.journal.len()).filter(|&i| i != spawn).collect();
    let mut oracle = RegisterOracle::new(&c.journal, &c.built.roles, c.target);
    assert!(matches!(oracle.replay(&kept), Replayed::NotReplayable(_)));
    // As a starting set it is not even a configuration.
    let order = order_of(c);
    assert!(!order.is_down_closed(&kept));
    assert_eq!(
        reduce::deletion_pass(&order, kept, Deletion::Configurations, &mut oracle, BUDGET),
        Reduction::Refused(reduce::Refusal::NotAConfiguration)
    );
}

/// A correct run does not fail: the reduction refuses it rather than returning a core.
#[test]
fn a_run_that_does_not_fail_is_refused() {
    let base = baseline::baseline();
    let plan = &base.groups[0].plans[0];
    let built = register::build_with_shutdown(plan);
    let log = &baseline::logs_for(&built, 1, 0).1[0];
    let journal = run(&built.programs, log, &baseline::config()).expect("a journal");
    let observer = observer_footprints(&journal, &built.roles, Target::Agreement);
    let order = CausalOrder::with_atoms(
        causal::predecessors_with(&journal, CAUSAL_WORK, &observer).expect("within the bound"),
        causal::atoms(&journal),
    )
    .expect("a causal order");
    for target in [Target::Agreement, Target::AckedNotDurable] {
        let mut oracle = RegisterOracle::new(&journal, &built.roles, target);
        assert_eq!(
            reduce::minimize(&order, Deletion::Configurations, &mut oracle, BUDGET),
            Reduction::Refused(reduce::Refusal::InputHolds)
        );
    }
}

/// INV-008: a budget too small to finish is a typed inconclusive with the best
/// replay-validated set, never a success and never a minimality claim.
#[test]
fn an_exhausted_budget_is_inconclusive_with_the_best_core() {
    let c = agreement_witness();
    let order = order_of(c);
    let mut oracle = RegisterOracle::new(&c.journal, &c.built.roles, c.target);
    let r = reduce::minimize(
        &order,
        Deletion::Configurations,
        &mut oracle,
        Budget::new(5, 1 << 26),
    );
    let Reduction::Inconclusive {
        reason,
        best,
        transcript,
        spent,
    } = r
    else {
        panic!("inconclusive: {r:?}");
    };
    assert_eq!(
        reason,
        continuum_value::assurance::InconclusiveReason::ResourceExhausted
    );
    assert_eq!(spent.replays, 5);
    let best = best.expect("the closure core");
    assert!(!best.guarantees.contains(&Guarantee::CausallyMinimal));
    assert!(matches!(
        oracle.replay(&best.events),
        Replayed::Fails { .. }
    ));
    assert_eq!(
        transcript.last().map(|p| p.pass),
        Some(Pass::Deletion(Deletion::Configurations))
    );
    // No replay at all: nothing validated.
    let mut oracle = RegisterOracle::new(&c.journal, &c.built.roles, c.target);
    let r = reduce::minimize(
        &order,
        Deletion::Configurations,
        &mut oracle,
        Budget::new(0, 1 << 26),
    );
    assert!(matches!(r, Reduction::Inconclusive { best: None, .. }));
    assert_eq!(oracle.replays, 0, "charged before the replay runs");
}

#[test]
fn the_evidence_matches_its_golden() {
    const PATH: &str = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/golden/pr18_impl01_reduction.evidence.txt"
    );
    let got = evidence();
    if std::env::var_os("PR18_IMPL01_BLESS").is_some() {
        std::fs::write(PATH, &got).expect("writes the golden");
        panic!("the golden was rewritten: review the diff and rerun without PR18_IMPL01_BLESS");
    }
    let want = std::fs::read_to_string(PATH).expect("the golden exists");
    let first = got
        .lines()
        .zip(want.lines())
        .position(|(g, w)| g != w)
        .map_or_else(String::new, |i| {
            format!(
                "line {}: `{}`",
                i + 1,
                got.lines().nth(i).unwrap_or_default()
            )
        });
    assert!(
        got == want,
        "the evidence drifted at {first}; regenerate with PR18_IMPL01_BLESS=1 and review"
    );
    let ids: Vec<&str> = got
        .lines()
        .filter_map(|l| l.strip_prefix('[').and_then(|l| l.strip_suffix(']')))
        .collect();
    assert_eq!(
        ids,
        [
            "pr18-impl01-01-m01-agreement-core",
            "pr18-impl01-02-m01-acked-not-durable-core",
            "pr18-impl01-03-closure-preservation",
            "pr18-impl01-04-determinism",
            "pr18-impl01-05-symmetry-renaming",
        ]
    );
}
