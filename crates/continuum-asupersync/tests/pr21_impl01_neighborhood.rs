//! PR 21 (bn-4ykgg): semantic neighbors around M01's causal core, run against the
//! correct replicated register and against M01 (ack-before-sync). START_HERE PR 21
//! ("Generate semantic neighbors around the causal core"); RFC 0032 gate 5.
//!
//! # The instantiation
//!
//! `continuum_repair::neighborhood` is generic over a [`CausalCore`] and a
//! [`Substrate`]. Here:
//!
//! - **The failing run** is M01's witness on the scenario's own plan
//!   (`cancel-after-submit-before-sync`): the shallowest `abstract_register::Agreement`
//!   failure among the plan's campaign logs, as PR 16's mutant campaign draws them.
//! - **The core's events** are that run's operations after the setup and before the
//!   shutdown: each is one act of a replica script or one step of a coordinator, named
//!   by a program-independent [`Label`]. The order is at operation grain, not at
//!   journal-event grain: one operation is one scheduler decision, which is what a
//!   neighbor's schedule reorders.
//! - **Hard edges** are program order within each actor, and, for each coordinator, the
//!   `j`-th confirmation it is sent before the step after its `j`-th receive: the
//!   admissibility rule of `replicated_register.rs` (a coordinator that has received
//!   more than it was sent is parked). Committing each receive to one sender is
//!   conservative: a schedule that swaps two confirmations to one coordinator is still
//!   admitted (they are dependent, not hard-ordered), but one that runs a coordinator
//!   step before the specific confirmation the core had it consume is rejected even
//!   when another confirmation would have served.
//! - **Dependence** is two confirmations to one coordinator (its queue), and a
//!   confirmation with that coordinator's receives.
//! - **The core proper** is the happens-before past (hard edges and dependence, oriented
//!   by the run) of the two acknowledgements the Agreement finding observes: the causal
//!   closure pass of PR 18 at operation grain. PR 18's deletion-reduced journal core
//!   (bn-3km4z, in review) is not on this branch; feeding it in is a follow-up, and a
//!   CIR core (PR 17) after that.
//! - **A neighbor** is a scenario (each replica's value and fate, from
//!   `register_baseline.rs`) and a schedule preference over labels. It names no
//!   program: each program under evaluation builds its own plan from the fates (the
//!   correct protocol, or M01's transformation of it), and [`register::guided_run`]
//!   realizes the preference as an admissible choice log of that program. A scenario's
//!   own order is the one M01 takes under it (`feasible`), so the generator never
//!   proposes a schedule the core's program cannot run.
//! - **Is the neighbor** is the library's call, not this harness's: each run reports a
//!   witness (`continuum_repair::neighborhood::Witness`). For a schedule neighbor it is
//!   the core events the run took, by label, and those the program does not have; the
//!   library executes the run only when it takes every core event the program has,
//!   keeps every pair the core's dependence relates in the requested order, and takes
//!   the neighbor's causal decision as requested. For a scenario neighbor the run
//!   echoes the scenario's bytes. So the correct program is executed on an interleaving
//!   neighbor of M01's core only when one of its runs really is that causal neighbor:
//!   most are not (it confirms after its syncs), and those are `NotRealizable` with the
//!   check's reason.
//! - **Realized** means the binding runs the log and the run is a run: the lift
//!   conforms, the A7 model accepts, and the projection accepts. Otherwise the neighbor
//!   is `NotRealizable`, typed, and not executed. **Executed** means the scenario's four
//!   properties are checked on it (`register_baseline::run_one`).
//! - **The envelope** is the scenario's Intent Contract fault model and bounds (every
//!   fault class the scenario's `[network]` and `[faults]` allow; `values = 2`,
//!   `nodes = 3`, `faults = 3`) with its `[faults]` caps (`max_crashes = 2`,
//!   `max_cancellations = 3`, `max_partitions = 1`).
//! - Message duplication and loss are well typed against that contract but the harness
//!   models no network (`register_baseline.rs`, scope), so they are `Unsupported`, not
//!   invalid and not passed. `abstraction-map-variants` and `hidden-corpus-mutations`
//!   do not run: there is no abstraction map (RFC 0031) and no held-out corpus (docs/50).
//!
//! # The evidence, by stable artifact ID
//!
//! Everything renders into `tests/golden/pr21_impl01_neighborhood.evidence.txt`.
//! Regenerate with `PR21_IMPL01_BLESS=1 cargo test -p continuum-asupersync --test
//! pr21_impl01_neighborhood` and review the diff.
//!
//! - `pr21-impl01-01-m01-core-neighborhood`: the core, per-strategy coverage for the
//!   correct program and for M01, and the discrimination: the correct program passes
//!   every executed neighbor, M01 fails on some, and every M01 failure is disclosed;
//! - `pr21-impl01-02-determinism`: identical inputs give byte-identical records; a
//!   different seed moves only the seeded walks;
//! - `pr21-impl01-03-invalid-rejected`: invalid perturbations (past a fault bound,
//!   against the causal order, outside the value domain or the fault model) are
//!   rejected with a typed reason and never realized or executed;
//! - `pr21-impl01-04-receipt-coverage`: the projection onto
//!   `promotion-receipt.schema.json`'s `coverage.neighborhood`, checked against the
//!   schema's own shape;
//! - `pr21-impl01-05-symmetry`: the metamorphic relation: every value and name
//!   permutation of the core keeps the core's outcome under each program.

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

use baseline::{Fate, Finding};
use continuum_asupersync::choice::ChoiceLog;
use continuum_repair::neighborhood::intent::{
    Bounds, DeclaredBound, ExplorationBound, FaultClass, FaultModel, Json,
};
use continuum_repair::neighborhood::{
    self, Budget, Caps, CausalCore, Config, CoreOrder, Disposition, Edit, Envelope, EventClass,
    Execution, Exploration, NeighborhoodRecord, NotRun, Profile, Realized, Rejection, Spent,
    Strategy, StrategyStatus, Substrate, Verdict,
};
use register::{Act, Built, Plan};

/// What realizes and judges a neighbor here, bound into every record.
const ENGINE: &str = "pr21 register substrate (guided_log with a label witness, build_with_shutdown, run_one: binding, lift, A7 model, projection, four scenario properties)";

/// The engine identity: the description above and a digest of the source that
/// realizes and judges a neighbor (this file and the four support files), so any change
/// to that code is a new engine and makes older records and continuations stale.
fn engine_identity() -> &'static str {
    static E: OnceLock<String> = OnceLock::new();
    E.get_or_init(|| {
        let sources = [
            include_str!("pr21_impl01_neighborhood.rs"),
            include_str!("support/replicated_register.rs"),
            include_str!("support/register_baseline.rs"),
            include_str!("support/register_mutants.rs"),
            include_str!("support/primitive_conformance_model.rs"),
        ]
        .concat();
        format!(
            "{ENGINE}; source {}",
            <continuum_value::identity::Blake3Hasher as continuum_value::identity::ContentHasher>::hash(sources.as_bytes())
        )
    })
}

/// The repair transaction the evidence campaigns name.
const TRANSACTION: &str = "rt_pr21-impl01-m01-core";

/// The seed of every campaign here: the scenario's.
const SEED: u64 = baseline::SCENARIO_SEED;

/// The budget every campaign here grants: far above what the core's neighborhood needs.
const BUDGET: Budget = Budget {
    candidates: 4_096,
    runs: 4_096,
    work: 1 << 26,
};

// ---------------------------------------------------------------------------
// labels
// ---------------------------------------------------------------------------

/// A replica act, without its operands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Kind {
    Reserve,
    Submit,
    Sync,
    Release,
    Abort,
    Confirm,
    Crash,
}

/// A coordinator step.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Step {
    Recv(u8),
    AckReserve,
    AckCommit,
}

/// One operation, named without reference to any program's operation indices, so one
/// schedule means the same thing to the correct program and to M01.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Label {
    /// An act of incarnation `inc` of replica `node` (`epoch` is `u8::MAX` for a crash).
    Replica {
        node: u8,
        inc: u8,
        kind: Kind,
        epoch: u8,
    },
    /// A step of the coordinator of `(epoch, value)`.
    Coord { epoch: u8, value: u8, step: Step },
}

impl Label {
    fn render(self) -> String {
        match self {
            Self::Replica {
                node,
                inc,
                kind,
                epoch,
            } => {
                let n = ["a", "b", "c"][usize::from(node)];
                let k = format!("{kind:?}").to_lowercase();
                if kind == Kind::Crash {
                    format!("{n}{inc}.crash")
                } else {
                    format!("{n}{inc}.{k}{epoch}")
                }
            }
            Self::Coord { epoch, value, step } => {
                let s = match step {
                    Step::Recv(j) => format!("recv{j}"),
                    Step::AckReserve => "ack-reserve".to_owned(),
                    Step::AckCommit => "ack-commit".to_owned(),
                };
                format!("k{epoch}{}.{s}", register::VALUES[usize::from(value)])
            }
        }
    }

    /// The label under a replica renaming `perm` (`perm[old] = new`) and a value
    /// renaming `swap`.
    fn renamed(self, perm: [u8; 3], swap: bool) -> Self {
        let v = |x: u8| if swap { 1 - x } else { x };
        match self {
            Self::Replica {
                node,
                inc,
                kind,
                epoch,
            } => Self::Replica {
                node: perm[usize::from(node)],
                inc,
                kind,
                epoch,
            },
            Self::Coord { epoch, value, step } => Self::Coord {
                epoch,
                value: v(value),
                step,
            },
        }
    }
}

/// The labels of replica `n`'s script.
fn script_labels(plan: &Plan, n: usize) -> Vec<Label> {
    let mut inc = 0_u8;
    plan.replicas[n]
        .iter()
        .map(|act| {
            let node = u8::try_from(n).expect("three replicas");
            let (kind, epoch) = match *act {
                Act::Reserve(e) => (Kind::Reserve, e),
                Act::Submit(e) => (Kind::Submit, e),
                Act::Sync(e) => (Kind::Sync, e),
                Act::Release(e) => (Kind::Release, e),
                Act::Abort(e) => (Kind::Abort, e),
                Act::Confirm(e) => (Kind::Confirm, e),
                Act::Crash | Act::CrashRepropose(..) => {
                    let l = Label::Replica {
                        node,
                        inc,
                        kind: Kind::Crash,
                        epoch: u8::MAX,
                    };
                    inc += 1;
                    return l;
                }
            };
            Label::Replica {
                node,
                inc,
                kind,
                epoch,
            }
        })
        .collect()
}

/// The coordinators of a plan, in the order `build` gives them actors, with their steps.
fn coord_labels(plan: &Plan) -> Vec<Vec<Label>> {
    mutants::confirmation_counts(plan)
        .into_iter()
        .map(|((epoch, value), writers)| {
            let mut out: Vec<Label> = (0..writers.min(2))
                .map(|j| Label::Coord {
                    epoch,
                    value,
                    step: Step::Recv(u8::try_from(j).expect("two")),
                })
                .collect();
            if writers >= 2 {
                out.push(Label::Coord {
                    epoch,
                    value,
                    step: Step::AckReserve,
                });
                out.push(Label::Coord {
                    epoch,
                    value,
                    step: Step::AckCommit,
                });
            }
            out
        })
        .collect()
}

/// Each actor's operations, labelled: `None` for the setup and the shutdown.
fn actor_labels(plan: &Plan, built: &Built) -> Option<Vec<Vec<Option<Label>>>> {
    let mut out = vec![vec![None; built.programs[0].len()]];
    for n in 0..3 {
        out.push(script_labels(plan, n).into_iter().map(Some).collect());
    }
    for c in coord_labels(plan) {
        out.push(c.into_iter().map(Some).collect());
    }
    if out.len() + 1 != built.programs.len() {
        return None;
    }
    out.push(vec![None; built.programs[built.programs.len() - 1].len()]);
    out.iter()
        .zip(&built.programs)
        .all(|(l, p)| l.len() == p.len())
        .then_some(out)
}

// ---------------------------------------------------------------------------
// scenarios
// ---------------------------------------------------------------------------

/// What a neighbor fixes: each replica's first value and fate, and a schedule
/// preference over labels.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Scenario {
    fates: [(u8, Fate); 3],
    order: Vec<Label>,
}

fn plan_of(fates: [(u8, Fate); 3]) -> Plan {
    baseline::plan_of("pr21-neighbor", 1, fates.map(baseline::one_epoch_replica))
}

fn swapped(f: Fate) -> Fate {
    match f {
        Fate::CrashReserved { retry } => Fate::CrashReserved { retry: 1 - retry },
        Fate::CrashSubmitted { retry } => Fate::CrashSubmitted { retry: 1 - retry },
        other => other,
    }
}

fn profile_of(fates: &[(u8, Fate); 3]) -> Profile {
    let crashes = fates.iter().filter(|(_, f)| f.crashes()).count();
    let aborts = fates.iter().filter(|(_, f)| *f == Fate::AbortRetry).count();
    let max_value = fates
        .iter()
        .flat_map(|&(v, f)| {
            let retry = match f {
                Fate::CrashReserved { retry } | Fate::CrashSubmitted { retry } => Some(retry),
                _ => None,
            };
            [Some(v), retry]
        })
        .flatten()
        .max()
        .map(u64::from);
    let crashes = u32::try_from(crashes).expect("three");
    let mut p = Profile::default();
    if crashes > 0 {
        p = p
            .with_fault(FaultClass::Crash, crashes)
            .expect("fits")
            .with_fault(FaultClass::Recovery, crashes)
            .expect("fits");
    }
    p.cancellations = crashes + u32::try_from(aborts).expect("three");
    p.max_value = max_value;
    p.nodes = 3;
    p
}

/// A program under evaluation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Program {
    /// The correct protocol: ack after sync.
    Correct,
    /// M01: every confirmation right after its submit.
    M01,
}

impl Program {
    fn plan(self, fates: [(u8, Fate); 3]) -> Plan {
        let plan = plan_of(fates);
        match self {
            Self::Correct => plan,
            Self::M01 => mutants::ack_before_sync(&plan),
        }
    }

    const fn name(self) -> &'static str {
        match self {
            Self::Correct => "replicated-register/correct (ack after sync)",
            Self::M01 => "replicated-register/M01 (ack before sync)",
        }
    }
}

/// The order M01, the core's program, actually takes under `fates` when it follows
/// `preference`: a scenario's schedule is always one the core's program can run, so a
/// deviation of another program is that program's, never the generator's. Falls back
/// to `preference` when the plan cannot be labelled or deadlocks (the realization then
/// reports it).
fn feasible(preference: Vec<Label>, fates: [(u8, Fate); 3]) -> Vec<Label> {
    if fates.iter().any(|(v, f)| {
        usize::from(*v) >= register::VALUES.len()
            || matches!(f, Fate::CrashReserved { retry } | Fate::CrashSubmitted { retry }
                if usize::from(*retry) >= register::VALUES.len())
    }) {
        // Out of the value domain: the envelope rejects it before any realization.
        return preference;
    }
    let plan = Program::M01.plan(fates);
    let built = register::build_with_shutdown(&plan);
    let Some(labels) = actor_labels(&plan, &built) else {
        return preference;
    };
    let rank: BTreeMap<Label, usize> = preference
        .iter()
        .enumerate()
        .map(|(i, l)| (*l, i))
        .collect();
    match register::guided_run(&built, |a, i| {
        labels[a][i].and_then(|l| rank.get(&l).copied())
    }) {
        Ok((_, taken)) => taken.iter().filter_map(|&(a, i)| labels[a][i]).collect(),
        Err(_) => preference,
    }
}

/// `base` with each replica's labels laid onto the slots its core labels took, the new
/// coordinators' steps appended: the scenario's schedule preference under `fates`.
fn relayout(base: &[Label], fates: [(u8, Fate); 3]) -> Vec<Label> {
    let plan = Program::M01.plan(fates);
    let mut fresh: Vec<std::collections::VecDeque<Label>> =
        (0..3).map(|n| script_labels(&plan, n).into()).collect();
    let coords: Vec<Label> = coord_labels(&plan).into_iter().flatten().collect();
    let keep: BTreeSet<Label> = coords.iter().copied().collect();
    let last_slot: BTreeMap<u8, usize> = base
        .iter()
        .enumerate()
        .filter_map(|(i, l)| match l {
            Label::Replica { node, .. } => Some((*node, i)),
            Label::Coord { .. } => None,
        })
        .collect();
    let mut out = Vec::new();
    let mut placed = BTreeSet::new();
    for (i, l) in base.iter().enumerate() {
        match *l {
            Label::Replica { node, .. } => {
                let q = &mut fresh[usize::from(node)];
                if let Some(x) = q.pop_front() {
                    out.push(x);
                }
                if last_slot.get(&node) == Some(&i) {
                    out.extend(q.drain(..));
                }
            }
            Label::Coord { .. } => {
                if keep.contains(l) {
                    placed.insert(*l);
                    out.push(*l);
                }
            }
        }
    }
    for q in &mut fresh {
        out.extend(q.drain(..));
    }
    out.extend(coords.into_iter().filter(|c| !placed.contains(c)));
    out
}

// ---------------------------------------------------------------------------
// the core
// ---------------------------------------------------------------------------

/// M01's core: the failing run, its order, and the scenario edits around it.
struct Core {
    fates: [(u8, Fate); 3],
    log: ChoiceLog,
    /// The run's operations after setup and before shutdown, by label.
    labels: Vec<Label>,
    order: CoreOrder,
    /// The Agreement finding's event.
    finding: Finding,
    identity: String,
    edits: BTreeMap<Strategy, Vec<(String, Scenario, Profile)>>,
}

/// The scenario's own fates: `a` proposes `v0` and crashes after its submit, its next
/// incarnation proposed `v1`; `b` writes `v0`; `c` writes `v1`.
const CORE_FATES: [(u8, Fate); 3] = [
    (0, Fate::CrashSubmitted { retry: 1 }),
    (0, Fate::Clean),
    (1, Fate::Clean),
];

fn core() -> &'static Core {
    static CORE: OnceLock<Core> = OnceLock::new();
    CORE.get_or_init(build_core)
}

fn reachable() -> &'static BTreeSet<register::Raw> {
    static R: OnceLock<BTreeSet<register::Raw>> = OnceLock::new();
    R.get_or_init(|| register::reachable(1))
}

/// The operations a log takes after the setup, as `(actor, operation index)`.
fn decode(built: &Built, log: &ChoiceLog) -> Vec<(usize, usize)> {
    let mut cursors = vec![0_usize; built.programs.len()];
    let mut out = Vec::new();
    for c in log.choices() {
        let enabled: Vec<usize> = (0..built.programs.len())
            .filter(|a| cursors[*a] < built.programs[*a].len())
            .collect();
        let actor = enabled[usize::try_from(c.0).expect("small")];
        if actor != 0 {
            out.push((actor, cursors[actor]));
        }
        cursors[actor] += 1;
    }
    out
}

fn build_core() -> Core {
    let plan = Program::M01.plan(CORE_FATES);
    assert_eq!(
        plan.replicas,
        mutants::ack_before_sync(&baseline::scenario_plan()).replicas,
        "the core's plan is M01's scenario plan"
    );
    let built = register::build_with_shutdown(&plan);
    let seed = baseline::baseline().plan_seed(0, 0);
    let (_, logs) = baseline::logs_for(&built, baseline::SCENARIO_LOGS, seed);
    // The shallowest Agreement failure: least event, then shortest log, then order.
    let (log, finding) = logs
        .iter()
        .filter_map(|log| {
            let r = baseline::run_one(&built, 1, log, reachable());
            let f = r
                .findings
                .iter()
                .filter(|f| matches!(f, Finding::Agreement(_)))
                .min_by_key(|f| mutants::seq_of(f))
                .cloned()?;
            Some((log.clone(), f))
        })
        .min_by_key(|(log, f)| (mutants::seq_of(f), log.len()))
        .expect("M01 fails Agreement on its scenario plan");
    let labels_by_actor = actor_labels(&plan, &built).expect("labels");
    let shutdown = built.programs.len() - 1;
    let ops: Vec<(usize, usize)> = decode(&built, &log)
        .into_iter()
        .filter(|(a, _)| *a != shutdown)
        .collect();
    let labels: Vec<Label> = ops
        .iter()
        .map(|&(a, i)| labels_by_actor[a][i].expect("a labelled operation"))
        .collect();
    let n = labels.len();
    // Hard edges: program order, and each coordinator's step after its j-th receive
    // after the j-th confirmation sent to it.
    let mut hard: Vec<Vec<usize>> = vec![Vec::new(); n];
    let mut last_of: BTreeMap<usize, usize> = BTreeMap::new();
    for (e, &(a, _)) in ops.iter().enumerate() {
        if let Some(&p) = last_of.get(&a) {
            hard[e].push(p);
        }
        last_of.insert(a, e);
    }
    let channel_of = |l: Label| -> Option<(u8, u8)> {
        // The coordinator a confirmation goes to: its incarnation's value.
        match l {
            Label::Replica {
                node,
                inc,
                kind: Kind::Confirm,
                epoch,
            } => {
                let values = register::incarnation_values(&plan, usize::from(node));
                Some((epoch, values[usize::from(inc)][usize::from(epoch)]))
            }
            _ => None,
        }
    };
    let mut sends: BTreeMap<(u8, u8), Vec<usize>> = BTreeMap::new();
    for (e, &l) in labels.iter().enumerate() {
        if let Some(k) = channel_of(l) {
            sends.entry(k).or_default().push(e);
        }
    }
    let mut conflicts = Vec::new();
    for (e, &l) in labels.iter().enumerate() {
        if let Label::Coord { epoch, value, step } = l {
            let consumed = match step {
                Step::Recv(j) => usize::from(j),
                Step::AckReserve => 2,
                Step::AckCommit => continue,
            };
            if consumed > 0 {
                let s = sends[&(epoch, value)][consumed - 1];
                assert!(s < e, "the core run is admissible");
                hard[e].push(s);
            }
            if let Step::Recv(_) = step {
                for &s in &sends[&(epoch, value)] {
                    conflicts.push((s, e));
                }
            }
        }
    }
    for list in sends.values() {
        assert!(
            list.len() < register::CONFIRM_CAPACITY as usize,
            "the channel capacity never binds"
        );
        for (i, &a) in list.iter().enumerate() {
            for &b in &list[i + 1..] {
                conflicts.push((a, b));
            }
        }
    }
    let class: BTreeMap<usize, EventClass> = labels
        .iter()
        .enumerate()
        .filter_map(|(e, &l)| match l {
            Label::Replica {
                kind: Kind::Confirm,
                ..
            } => {
                let (ep, v) = channel_of(l).expect("a confirmation");
                Some((
                    e,
                    EventClass::Send {
                        channel: u64::from(ep) * 2 + u64::from(v),
                    },
                ))
            }
            Label::Replica {
                kind: Kind::Crash, ..
            } => Some((e, EventClass::Fault(FaultClass::Crash))),
            _ => None,
        })
        .collect();
    // The core proper: the happens-before past of both acknowledgements.
    let witnesses: Vec<usize> = labels
        .iter()
        .enumerate()
        .filter(|(_, l)| {
            matches!(
                l,
                Label::Coord {
                    step: Step::AckCommit,
                    ..
                }
            )
        })
        .map(|(e, _)| e)
        .collect();
    assert_eq!(
        witnesses.len(),
        2,
        "Agreement observes two acknowledgements"
    );
    let mut preds: Vec<BTreeSet<usize>> =
        hard.iter().map(|l| l.iter().copied().collect()).collect();
    for &(a, b) in &conflicts {
        preds[a.max(b)].insert(a.min(b));
    }
    let mut member = vec![false; n];
    let mut stack = witnesses;
    while let Some(e) = stack.pop() {
        if !std::mem::replace(&mut member[e], true) {
            stack.extend(preds[e].iter().copied());
        }
    }
    let core_members: Vec<usize> = (0..n).filter(|e| member[*e]).collect();
    let order =
        CoreOrder::new(hard, &conflicts, &core_members, &class).expect("a well-formed core");
    let mut identity = String::new();
    for l in &labels {
        let _ = write!(identity, "{} ", l.render());
    }
    let identity = format!(
        "m01-scenario-core log={log} ops={}",
        <continuum_value::identity::Blake3Hasher as continuum_value::identity::ContentHasher>::hash(
            identity.as_bytes()
        )
    );
    let mut core = Core {
        fates: CORE_FATES,
        log,
        labels,
        order,
        finding,
        identity,
        edits: BTreeMap::new(),
    };
    core.edits = scenario_edits(&core);
    core
}

fn checkpoint_name(f: Fate) -> String {
    f.token()
}

/// The substrate's scenario edits, by strategy, in a fixed order.
fn scenario_edits(core: &Core) -> BTreeMap<Strategy, Vec<(String, Scenario, Profile)>> {
    let base = &core.labels;
    let fates = core.fates;
    let names = ["a", "b", "c"];
    let edit = |fates: [(u8, Fate); 3]| Scenario {
        fates,
        order: feasible(relayout(base, fates), fates),
    };
    let entry = |id: String, fates: [(u8, Fate); 3]| (id, edit(fates), profile_of(&fates));
    let mut out = BTreeMap::new();

    // fault-window: remove each crash; insert one crash at each checkpoint of each
    // crash-free replica; insert one at every crash-free replica at once.
    let mut fw = Vec::new();
    for n in 0..3 {
        if fates[n].1.crashes() {
            let mut f = fates;
            f[n].1 = Fate::Clean;
            fw.push(entry(format!("remove-crash({})", names[n]), f));
        }
    }
    let crash_fates = |v: u8| {
        vec![
            Fate::CrashReserved { retry: v },
            Fate::CrashReserved { retry: 1 - v },
            Fate::CrashSubmitted { retry: v },
            Fate::CrashSubmitted { retry: 1 - v },
            Fate::CrashSynced,
            Fate::CrashReplied,
        ]
    };
    for n in 0..3 {
        if fates[n].1.crashes() {
            continue;
        }
        for c in crash_fates(fates[n].0) {
            let mut f = fates;
            f[n].1 = c;
            fw.push(entry(
                format!("insert({}:{})", names[n], checkpoint_name(c)),
                f,
            ));
        }
    }
    for kind in 0..4 {
        let mut f = fates;
        for slot in f.iter_mut().filter(|(_, x)| !x.crashes()) {
            let v = slot.0;
            slot.1 = [
                Fate::CrashReserved { retry: v },
                Fate::CrashSubmitted { retry: v },
                Fate::CrashSynced,
                Fate::CrashReplied,
            ][kind];
        }
        fw.push(entry(
            format!(
                "insert-all({})",
                [
                    "crash-reserved",
                    "crash-submitted",
                    "crash-synced",
                    "crash-replied"
                ][kind]
            ),
            f,
        ));
    }
    out.insert(Strategy::FaultWindow, fw);

    // cancellation-checkpoints: move each crash to its adjacent checkpoints; replace it
    // by the writer's own cancellation before bytes; add that cancellation to each
    // crash-free replica, and to all of them at once.
    let mut cc = Vec::new();
    let ladder = |f: Fate| -> Vec<Fate> {
        match f {
            Fate::CrashReserved { retry } => vec![Fate::CrashSubmitted { retry }],
            Fate::CrashSubmitted { retry } => {
                vec![Fate::CrashReserved { retry }, Fate::CrashSynced]
            }
            Fate::CrashSynced => vec![Fate::CrashSubmitted { retry: 0 }, Fate::CrashReplied],
            Fate::CrashReplied => vec![Fate::CrashSynced],
            _ => vec![],
        }
    };
    for n in 0..3 {
        for g in ladder(fates[n].1) {
            let g = match g {
                Fate::CrashSubmitted { .. } if fates[n].1 == Fate::CrashSynced => {
                    Fate::CrashSubmitted { retry: fates[n].0 }
                }
                other => other,
            };
            let mut f = fates;
            f[n].1 = g;
            cc.push(entry(
                format!(
                    "move({}:{} to {})",
                    names[n],
                    checkpoint_name(fates[n].1),
                    checkpoint_name(g)
                ),
                f,
            ));
        }
    }
    for n in 0..3 {
        if fates[n].1.crashes() {
            let mut f = fates;
            f[n].1 = Fate::AbortRetry;
            cc.push(entry(format!("abort-instead({})", names[n]), f));
        }
    }
    for n in 0..3 {
        if !fates[n].1.crashes() && fates[n].1 != Fate::Idle {
            let mut f = fates;
            f[n].1 = Fate::AbortRetry;
            cc.push(entry(format!("abort-retry({})", names[n]), f));
        }
    }
    {
        let mut f = fates;
        for slot in f
            .iter_mut()
            .filter(|(_, x)| !x.crashes() && *x != Fate::Idle)
        {
            slot.1 = Fate::AbortRetry;
        }
        cc.push(entry("abort-retry(all)".to_owned(), f));
    }
    out.insert(Strategy::CancellationCheckpoints, cc);

    // value-name-permutation: the symmetric permutations (value swap, replica renamings,
    // both), then value-domain steps of one replica's value or retry value by one.
    let mut vp = Vec::new();
    let perms: [[u8; 3]; 6] = [
        [0, 1, 2],
        [0, 2, 1],
        [1, 0, 2],
        [1, 2, 0],
        [2, 0, 1],
        [2, 1, 0],
    ];
    for swap in [false, true] {
        for perm in perms {
            if !swap && perm == [0, 1, 2] {
                continue;
            }
            let mut f = fates;
            for n in 0..3 {
                let (v, fate) = fates[n];
                f[usize::from(perm[n])] = if swap {
                    (1 - v, swapped(fate))
                } else {
                    (v, fate)
                };
            }
            let order = base.iter().map(|l| l.renamed(perm, swap)).collect();
            let name: String = perm.iter().map(|&p| names[usize::from(p)]).collect();
            let id = match (swap, perm == [0, 1, 2]) {
                (true, true) => "swap-values".to_owned(),
                (true, false) => format!("swap-values+rename({name})"),
                (false, _) => format!("rename({name})"),
            };
            vp.push((id, Scenario { fates: f, order }, profile_of(&f)));
        }
    }
    for n in 0..3 {
        let (v, fate) = fates[n];
        for next in [v.checked_add(1), v.checked_sub(1)].into_iter().flatten() {
            let mut f = fates;
            f[n].0 = next;
            vp.push(entry(format!("value({}:{v}->{next})", names[n]), f));
        }
        if let Fate::CrashReserved { retry } | Fate::CrashSubmitted { retry } = fate {
            for next in [retry.checked_add(1), retry.checked_sub(1)]
                .into_iter()
                .flatten()
            {
                let mut f = fates;
                f[n].1 = match fate {
                    Fate::CrashReserved { .. } => Fate::CrashReserved { retry: next },
                    _ => Fate::CrashSubmitted { retry: next },
                };
                vp.push(entry(format!("retry({}:{retry}->{next})", names[n]), f));
            }
        }
    }
    out.insert(Strategy::ValueNamePermutation, vp);
    out
}

// ---------------------------------------------------------------------------
// the substrate
// ---------------------------------------------------------------------------

/// A realized run: its report.
struct Realization {
    findings: Vec<Finding>,
    digest: String,
}

/// One program's view of the core's neighborhood.
struct Register<'a> {
    core: &'a Core,
    program: Program,
    /// Realizations the substrate was asked for, by neighbor: the negative test reads it.
    realized: usize,
    executed: usize,
}

impl<'a> Register<'a> {
    const fn new(core: &'a Core, program: Program) -> Self {
        Self {
            core,
            program,
            realized: 0,
            executed: 0,
        }
    }

    /// Run the program under `fates`, following the schedule preference `order`, and
    /// report what it did: the run, and the operations it took and the ones it does not
    /// have, by label. Whether that is the requested neighbor is the library's call
    /// (`continuum_repair::neighborhood::Witness`), never this harness's.
    fn run(&self, fates: [(u8, Fate); 3], order: &[Label]) -> Result<Ran, String> {
        let plan = self.program.plan(fates);
        let built = register::build_with_shutdown(&plan);
        let Some(labels) = actor_labels(&plan, &built) else {
            return Err("label-mismatch".to_owned());
        };
        let rank: BTreeMap<Label, usize> = order.iter().enumerate().map(|(i, l)| (*l, i)).collect();
        let guided = register::guided_run(&built, |a, i| {
            labels[a][i].and_then(|l| rank.get(&l).copied())
        });
        let taken = match &guided {
            Ok((_, taken)) => taken.clone(),
            Err(prefix) => decode(&built, prefix),
        };
        let got: Vec<Label> = taken.iter().filter_map(|&(a, i)| labels[a][i]).collect();
        let has: BTreeSet<Label> = labels.iter().flatten().flatten().copied().collect();
        let halted = guided.is_err();
        let realization = match guided {
            Ok((log, _)) => {
                // Every finding, a non-run included (a binding refusal, a lift that does
                // not conform, an A7 or projection rejection), is the program's.
                let report = baseline::run_one(&built, plan.epochs, &log, reachable());
                Realization {
                    findings: report.findings,
                    digest: report.digest,
                }
            }
            Err(prefix) => {
                // A real deadlock of the program under this schedule: every actor with
                // work left is parked. It is the program's failure.
                let what = format!("deadlock {} after {prefix}", baseline::render_plan(&plan));
                Realization {
                    findings: vec![Finding::Refused(what.clone())],
                    digest: <continuum_value::identity::Blake3Hasher as continuum_value::identity::ContentHasher>::hash(what.as_bytes()).to_string(),
                }
            }
        };
        Ok(Ran {
            realization,
            got,
            has,
            halted,
        })
    }
}

/// What one run of the program did.
struct Ran {
    realization: Realization,
    /// The labelled operations it took, in order.
    got: Vec<Label>,
    /// Every labelled operation the program has.
    has: BTreeSet<Label>,
    halted: bool,
}

impl Substrate for Register<'_> {
    type Core = CoreOrder;
    type Scenario = Scenario;
    type Run = Realization;

    fn core(&self) -> &CoreOrder {
        &self.core.order
    }

    fn core_identity(&self) -> String {
        self.core.identity.clone()
    }

    fn subject(&self) -> String {
        // The program's content identity: its name and the binding programs it builds
        // for the core's scenario.
        let built = register::build_with_shutdown(&self.program.plan(self.core.fates));
        let text = format!("{}\n{:?}", self.program.name(), built.programs);
        format!(
            "{} {}",
            self.program.name(),
            <continuum_value::identity::Blake3Hasher as continuum_value::identity::ContentHasher>::hash(text.as_bytes())
        )
    }

    fn engine_identity(&self) -> String {
        format!(
            "{}; journal encoding {}",
            engine_identity(),
            continuum_asupersync::journal::ENCODING_VERSION
        )
    }

    fn scenario_bytes(&self, s: &Scenario) -> Vec<u8> {
        // Debug spelling: total over every scenario, an out-of-domain value included
        // (such a candidate is committed, then rejected by the envelope).
        format!("{:?}\n{:?}", s.fates, s.order).into_bytes()
    }

    fn core_profile(&self) -> Profile {
        profile_of(&self.core.fates)
    }

    fn not_run(&self, strategy: Strategy) -> Option<NotRun> {
        match strategy {
            Strategy::AbstractionMapVariants => Some(NotRun(
                "no abstraction map: RFC 0031's map and CIR (PR 17) are not built".to_owned(),
            )),
            Strategy::HiddenCorpusMutations => Some(NotRun(
                "no held-out corpus: the docs/50 gaming corpus does not exist".to_owned(),
            )),
            _ => None,
        }
    }

    fn scenario_count(&self, strategy: Strategy) -> usize {
        self.core.edits.get(&strategy).map_or(0, Vec::len)
    }

    fn scenario(&self, strategy: Strategy, index: usize) -> Option<(String, Scenario, Profile)> {
        self.core.edits.get(&strategy)?.get(index).cloned()
    }

    fn realize(&mut self, edit: &Edit<Scenario>) -> Realized<Realization> {
        self.realized += 1;
        match edit {
            Edit::Schedule(order) => {
                let labels: Vec<Label> = order.iter().map(|&e| self.core.labels[e]).collect();
                match self.run(self.core.fates, &labels) {
                    Err(why) => Realized::NotRealizable(why),
                    Ok(ran) => {
                        // The run in the core's terms: the core events it took, in
                        // order, and those the program does not have at all.
                        let index: BTreeMap<Label, usize> = self
                            .core
                            .labels
                            .iter()
                            .enumerate()
                            .map(|(i, l)| (*l, i))
                            .collect();
                        let taken = ran
                            .got
                            .iter()
                            .filter_map(|l| index.get(l).copied())
                            .collect();
                        let absent = (0..self.core.labels.len())
                            .filter(|&e| !ran.has.contains(&self.core.labels[e]))
                            .collect();
                        Realized::Run {
                            run: ran.realization,
                            witness: neighborhood::Witness::Schedule {
                                taken,
                                absent,
                                halted: ran.halted,
                            },
                        }
                    }
                }
            }
            Edit::Message { .. } => Realized::Unsupported(
                "no network: the harness models no message duplication or loss".to_owned(),
            ),
            Edit::Scenario(s) => match self.run(s.fates, &s.order) {
                Err(why) => Realized::NotRealizable(why),
                // A scenario neighbor is its faults and values: the run is of this
                // scenario, and says so with the scenario's bytes.
                Ok(ran) => Realized::Run {
                    run: ran.realization,
                    witness: neighborhood::Witness::Edit(self.scenario_bytes(s)),
                },
            },
        }
    }

    fn execute(&mut self, run: &Realization) -> Execution {
        self.executed += 1;
        let handle = format!("ev_{}", run.digest);
        // Every property the run refutes, not only the first.
        let refuted: BTreeSet<&str> = run
            .findings
            .iter()
            .map(|f| f.property().scenario_name().unwrap_or("run"))
            .collect();
        if refuted.is_empty() {
            Execution::Pass { run: handle }
        } else {
            Execution::Fail {
                property: refuted.into_iter().collect::<Vec<_>>().join("+"),
                run: handle,
            }
        }
    }
}

fn envelope() -> Envelope {
    let model = FaultModel::new(
        [
            FaultClass::Crash,
            FaultClass::Recovery,
            FaultClass::Partition,
            FaultClass::Loss,
            FaultClass::Duplication,
            FaultClass::Delay,
        ],
        [],
    )
    .expect("a fault model");
    envelope_with(model)
}

fn envelope_with(model: FaultModel) -> Envelope {
    let b = baseline::SCENARIO;
    let bounds = Bounds::new(
        ExplorationBound::Bounded(i64::from(b.values)),
        DeclaredBound::Declared(i64::from(b.nodes)),
        DeclaredBound::Declared(i64::try_from(b.max_cancellations).expect("small")),
        ExplorationBound::Unbounded,
    )
    .expect("bounds");
    Envelope::new(
        model,
        bounds,
        Caps {
            crashes: Some(u32::try_from(b.max_crashes).expect("small")),
            cancellations: Some(u32::try_from(b.max_cancellations).expect("small")),
            partitions: Some(1),
        },
    )
}

fn config(seed: u64) -> Config {
    Config {
        seed,
        strategies: Strategy::ALL.to_vec(),
        walks: 8,
        walk_length: 6,
        max_delay: 3,
        budget: BUDGET,
        transaction: TRANSACTION.to_owned(),
    }
}

fn campaign(program: Program, seed: u64) -> (NeighborhoodRecord, usize, usize) {
    campaign_with(program, &config(seed))
}

fn campaign_with(program: Program, cfg: &Config) -> (NeighborhoodRecord, usize, usize) {
    let mut s = Register::new(core(), program);
    let r = neighborhood::explore(&mut s, &envelope(), cfg)
        .expect("a campaign")
        .into_record();
    (r, s.realized, s.executed)
}

/// The strategies both programs realize in full: the scenario strategies (fault
/// placement, cancellation, value and name). The interleaving strategies are built
/// around M01's causal decisions; the correct program orders its confirmations after
/// its syncs, so most of those neighbors are not causal neighbors of any run it has,
/// and the library does not execute them as such (the full campaign records it, and is
/// never projected for the correct program).
fn supported_config(seed: u64) -> Config {
    let mut c = config(seed);
    c.strategies = vec![
        Strategy::FaultWindow,
        Strategy::CancellationCheckpoints,
        Strategy::ValueNamePermutation,
    ];
    c
}

/// The projection a verifier makes: the record held to the inputs a fresh substrate of
/// `program`, the envelope and `cfg` give now.
fn project(
    r: &NeighborhoodRecord,
    program: Program,
    cfg: &Config,
) -> Result<Json, neighborhood::ProjectionRefusal> {
    r.receipt_coverage(&Register::new(core(), program), &envelope(), cfg)
}

fn supported_records() -> &'static [NeighborhoodRecord; 2] {
    static R: OnceLock<[NeighborhoodRecord; 2]> = OnceLock::new();
    R.get_or_init(|| {
        [
            campaign_with(Program::Correct, &supported_config(SEED)).0,
            campaign_with(Program::M01, &supported_config(SEED)).0,
        ]
    })
}

fn records() -> &'static [(NeighborhoodRecord, usize, usize); 2] {
    static R: OnceLock<[(NeighborhoodRecord, usize, usize); 2]> = OnceLock::new();
    R.get_or_init(|| {
        [
            campaign(Program::Correct, SEED),
            campaign(Program::M01, SEED),
        ]
    })
}

// ---------------------------------------------------------------------------
// rendering
// ---------------------------------------------------------------------------

fn render_coverage(out: &mut String, r: &NeighborhoodRecord) {
    let _ = writeln!(out, "  subject: {}", r.subject());
    let _ = writeln!(out, "  record digest: {}", r.digest());
    for (s, st) in r.strategies() {
        match st {
            StrategyStatus::NotRun(NotRun(why)) => {
                let _ = writeln!(out, "  {:<32} not run: {why}", s.token());
            }
            StrategyStatus::Ran(c) => {
                let rejected: Vec<String> =
                    c.rejected.iter().map(|(k, v)| format!("{k}={v}")).collect();
                let unreal: Vec<String> = c
                    .not_realizable
                    .iter()
                    .map(|(k, v)| format!("{k}={v}"))
                    .collect();
                let _ = writeln!(
                    out,
                    "  {:<32} generated={} rejected={} [{}] valid={} unsupported={} not-realizable={} [{}] executed={} distinct-runs={} passed={} failed={} inconclusive={} truncated={}",
                    s.token(),
                    c.generated,
                    c.rejected.values().sum::<u64>(),
                    rejected.join(" "),
                    c.valid,
                    c.unsupported,
                    c.not_realizable.values().sum::<u64>(),
                    unreal.join(" "),
                    c.executed,
                    c.distinct_runs,
                    c.passed,
                    c.failed,
                    c.inconclusive,
                    c.frontier.as_deref().unwrap_or("no"),
                );
            }
        }
    }
    let _ = writeln!(
        out,
        "  spent: candidates={} runs={} work={}; verdict {:?}",
        r.spent().candidates,
        r.spent().runs,
        r.spent().work,
        r.verdict()
    );
}

fn totals(r: &NeighborhoodRecord) -> [u64; 5] {
    let mut t = [0; 5];
    for (_, st) in r.strategies() {
        if let StrategyStatus::Ran(c) = st {
            t[0] += c.generated;
            t[1] += c.valid;
            t[2] += c.executed;
            t[3] += c.passed;
            t[4] += c.failed;
        }
    }
    t
}

fn evidence() -> String {
    let core = core();
    let [(correct, ..), (m01, ..)] = records();
    let mut out = String::new();
    let _ = writeln!(
        out,
        "# PR 21 (bn-4ykgg): semantic neighbors around M01's causal core\n"
    );
    let _ = writeln!(out, "## pr21-impl01-01-m01-core-neighborhood\n");
    let _ = writeln!(out, "core: {}", core.identity);
    let _ = writeln!(
        out,
        "failing run: M01 on {} ({}), log {} ; finding {:?}",
        baseline::SCENARIO_NAME,
        baseline::render_plan(&Program::M01.plan(core.fates)),
        core.log,
        core.finding
    );
    let _ = writeln!(
        out,
        "operations: {} ; core proper (happens-before past of both acks): {}",
        core.labels.len(),
        correct.core_members().len()
    );
    for (e, l) in core.labels.iter().enumerate() {
        let hard: Vec<String> = core
            .order
            .hard_predecessors(e)
            .iter()
            .map(ToString::to_string)
            .collect();
        let _ = writeln!(
            out,
            "  {e:>2} {} {:<18} hard<-[{}]",
            if core.order.in_core(e) { "*" } else { " " },
            l.render(),
            hard.join(",")
        );
    }
    let _ = writeln!(out, "\ncorrect program:");
    render_coverage(&mut out, correct);
    let _ = writeln!(out, "\nM01:");
    render_coverage(&mut out, m01);
    let [tc, tm] = [totals(correct), totals(m01)];
    let _ = writeln!(
        out,
        "\ntotals (generated/valid/executed/passed/failed): correct {tc:?}, M01 {tm:?}"
    );
    let _ = writeln!(
        out,
        "discrimination: correct fails on {} of {} executed neighbors; M01 fails on {} of {}",
        tc[4], tc[2], tm[4], tm[2]
    );
    let distinct = |r: &NeighborhoodRecord, fail: Option<bool>| -> usize {
        r.neighbors()
            .iter()
            .filter_map(|n| match &n.disposition {
                Disposition::Executed(Execution::Pass { run }) if fail != Some(true) => Some(run),
                Disposition::Executed(Execution::Fail { run, .. }) if fail != Some(false) => {
                    Some(run)
                }
                _ => None,
            })
            .collect::<BTreeSet<_>>()
            .len()
    };
    let _ = writeln!(
        out,
        "distinct runs (by handle, across strategies): correct {} executed, all passing; M01 {} executed, {} failing",
        distinct(correct, None),
        distinct(m01, None),
        distinct(m01, Some(true)),
    );
    let _ = writeln!(
        out,
        "note: every executed neighbor passed the library's causal-neighbor check on its witness; the correct program realizes few of M01's interleaving neighbors, so its full campaign is inconclusive and only its scenario profile projects"
    );
    let _ = writeln!(
        out,
        "causal-neighbor check (continuum-repair, the library's): a schedule neighbor is executed only when the run takes every core event it has, keeps every core-dependent pair in the requested order, and takes the neighbor's causal decision as requested; a scenario neighbor only when the run is of that scenario"
    );
    let _ = writeln!(out, "\nM01's failing neighbors (every one disclosed):");
    for f in m01.failing() {
        let _ = writeln!(out, "  {} {} {}", f.neighbor, f.property, f.run);
    }
    let passing: Vec<&str> = m01
        .neighbors()
        .iter()
        .filter(|n| matches!(n.disposition, Disposition::Executed(Execution::Pass { .. })))
        .map(|n| n.id.as_str())
        .collect();
    let _ = writeln!(out, "\nM01's passing neighbors: {}", passing.len());
    for p in passing {
        let _ = writeln!(out, "  {p}");
    }

    let _ = writeln!(out, "\n## pr21-impl01-02-determinism\n");
    let again = campaign(Program::M01, SEED).0;
    let other = campaign(Program::M01, SEED ^ 0xdead_beef).0;
    let _ = writeln!(
        out,
        "same seed: byte-identical record = {} ({} bytes)",
        again.canonical_bytes() == m01.canonical_bytes(),
        m01.canonical_bytes().len()
    );
    let differs: Vec<&str> = m01
        .neighbors()
        .iter()
        .zip(other.neighbors())
        .filter(|(a, b)| a != b)
        .map(|(a, _)| a.id.as_str())
        .collect();
    let _ = writeln!(
        out,
        "seed {:#x}: {} of {} neighbors differ, all of them seeded walks: {}",
        SEED ^ 0xdead_beef,
        differs.len(),
        m01.neighbors().len(),
        differs.iter().all(|id| id.contains(":walk("))
    );

    let _ = writeln!(out, "\n## pr21-impl01-03-invalid-rejected\n");
    for n in m01.neighbors().iter() {
        if let Disposition::Rejected(r) = &n.disposition {
            let _ = writeln!(out, "  {} -> {}", n.id, r.render());
        }
    }
    for (name, edit, profile) in hand_built_invalid(core) {
        let r = neighborhood::check_candidate(
            &core.order,
            &envelope(),
            &profile_of(&core.fates),
            &edit,
            &profile,
        );
        let _ = writeln!(
            out,
            "  hand-built {name} -> {}",
            r.err().map_or("accepted".to_owned(), |r| r.render())
        );
    }
    let _ = writeln!(
        out,
        "  with a fault model of crash and recovery only, every message neighbor is rejected (outside-fault-model, or violates-causal-order first) and none is realized: {}",
        message_neighbors_outside_model()
    );

    let _ = writeln!(out, "\n## pr21-impl01-04-receipt-coverage\n");
    let _ = writeln!(
        out,
        "full campaign (every strategy selected): correct verdict {:?}, projection {:?}; M01 verdict {:?}, projection {:?}",
        correct.verdict(),
        project(correct, Program::Correct, &config(SEED)).err(),
        m01.verdict(),
        project(m01, Program::M01, &config(SEED)).err(),
    );
    let _ = writeln!(
        out,
        "  (duplication and loss are unsupported on the register and two strategies do not run, so no coverage is projected: the gaps stay in the record)"
    );
    let [sc, sm] = supported_records();
    let selected: Vec<&str> = supported_config(SEED)
        .strategies
        .iter()
        .map(|s| s.token())
        .collect();
    let _ = writeln!(out, "supported profile [{}]:", selected.join(" "));
    for (name, program, r) in [("correct", Program::Correct, sc), ("M01", Program::M01, sm)] {
        render_coverage(&mut out, r);
        // Projected against the inputs the substrate gives now, never the record's own.
        let projected = match project(r, program, &supported_config(SEED)) {
            Ok(json) => String::from_utf8(json.to_canonical_bytes()).expect("utf-8"),
            Err(refusal) => format!("refused: {refusal:?}"),
        };
        let _ = writeln!(out, "{name} ({:?}): {projected}", r.verdict());
    }

    let _ = writeln!(out, "\n## pr21-impl01-05-symmetry\n");
    for (name, r) in [("correct", correct), ("M01", m01)] {
        let sym: Vec<String> = r
            .neighbors()
            .iter()
            .filter(|n| {
                n.strategy == Strategy::ValueNamePermutation
                    && (n.id.contains(":rename(") || n.id.contains(":swap-values"))
            })
            .map(|n| {
                let d = match &n.disposition {
                    Disposition::Executed(Execution::Pass { .. }) => "pass",
                    Disposition::Executed(Execution::Fail { .. }) => "fail",
                    _ => "other",
                };
                format!("{}={d}", n.id)
            })
            .collect();
        let _ = writeln!(out, "  {name}: {}", sym.join(" "));
    }
    out
}

/// Hand-built invalid candidates, one per rejection the negative test names.
fn hand_built_invalid(core: &Core) -> Vec<(&'static str, Edit<Scenario>, Profile)> {
    let n = core.labels.len();
    let pos = |want: Label| {
        core.labels
            .iter()
            .position(|l| *l == want)
            .expect("in the core")
    };
    let a_submit = pos(Label::Replica {
        node: 0,
        inc: 0,
        kind: Kind::Submit,
        epoch: 0,
    });
    let a_confirm = pos(Label::Replica {
        node: 0,
        inc: 0,
        kind: Kind::Confirm,
        epoch: 0,
    });
    // a0's confirmation before its own submit.
    let mut early = (0..n).collect::<Vec<_>>();
    let e = early.remove(a_confirm);
    early.insert(a_submit, e);
    let mut dup = (0..n).collect::<Vec<_>>();
    dup[1] = dup[0];
    let three_crashes = [
        (0, Fate::CrashSubmitted { retry: 1 }),
        (0, Fate::CrashReserved { retry: 0 }),
        (1, Fate::CrashSynced),
    ];
    let four_cancellations = [
        (0, Fate::CrashSubmitted { retry: 1 }),
        (0, Fate::AbortRetry),
        (1, Fate::CrashSynced),
    ];
    let v2 = [(0, Fate::Clean), (0, Fate::Clean), (2, Fate::Clean)];
    let scenario = |f: [(u8, Fate); 3]| {
        Edit::Scenario(Scenario {
            fates: f,
            order: relayout(&core.labels, f),
        })
    };
    let mut four = profile_of(&four_cancellations);
    four.cancellations += 1;
    vec![
        (
            "confirm-before-submit",
            Edit::Schedule(early),
            Profile::default(),
        ),
        ("not-a-permutation", Edit::Schedule(dup), Profile::default()),
        (
            "three-crashes",
            scenario(three_crashes),
            profile_of(&three_crashes),
        ),
        ("four-cancellations", scenario(four_cancellations), four),
        ("value-v2", scenario(v2), profile_of(&v2)),
        (
            "duplicate-a-non-send",
            Edit::Message {
                event: a_submit,
                fault: FaultClass::Duplication,
            },
            Profile::default()
                .with_fault(FaultClass::Duplication, 1)
                .expect("fits"),
        ),
    ]
}

/// With an envelope whose fault model enables only crash and recovery, every message
/// neighbor is rejected, none is realized, and each one the causal order admits is
/// rejected as outside the fault model (a delay that overtakes the send's consumer is
/// rejected for the causal order first).
fn message_neighbors_outside_model() -> bool {
    let model = FaultModel::new([FaultClass::Crash, FaultClass::Recovery], []).expect("model");
    let mut s = Register::new(core(), Program::M01);
    let mut cfg = config(SEED);
    cfg.strategies = vec![Strategy::MessageDuplicationLossDelay];
    let r = neighborhood::explore(&mut s, &envelope_with(model), &cfg)
        .expect("a campaign")
        .into_record();
    let c = r
        .coverage(Strategy::MessageDuplicationLossDelay)
        .expect("ran");
    let rejected: u64 = c.rejected.values().sum();
    let outside = c.rejected.get("outside-fault-model").copied().unwrap_or(0);
    let causal = c
        .rejected
        .get("violates-causal-order")
        .copied()
        .unwrap_or(0);
    let dup_loss_outside = r.neighbors().iter().all(|n| {
        !(n.id.contains(":duplicate(") || n.id.contains(":lose("))
            || matches!(
                n.disposition,
                Disposition::Rejected(Rejection::OutsideFaultModel { .. })
            )
    });
    c.generated > 0
        && rejected == c.generated
        && outside + causal == c.generated
        && outside > 0
        && dup_loss_outside
        && s.realized == 0
        && s.executed == 0
}

// ---------------------------------------------------------------------------
// the tests
// ---------------------------------------------------------------------------

#[test]
fn m01_core_neighborhood_discriminates_the_correct_program_from_m01() {
    let [(correct, ..), (m01, ..)] = records();
    // The full campaign has gaps (unsupported duplication and loss, two strategies not
    // run): it is inconclusive, never Complete, and never projected.
    let unsupported = Verdict::Inconclusive {
        reason: continuum_value::assurance::InconclusiveReason::Unsupported,
    };
    assert_eq!(correct.verdict(), unsupported);
    assert_eq!(m01.verdict(), unsupported);
    assert!(
        project(correct, Program::Correct, &config(SEED)).is_err()
            && project(m01, Program::M01, &config(SEED)).is_err()
    );
    // The supported profile is Complete under both programs.
    let [sc, sm] = supported_records();
    assert_eq!(sc.verdict(), Verdict::Complete);
    assert_eq!(sm.verdict(), Verdict::Complete);
    assert_eq!(
        totals(sc)[4],
        0,
        "the correct program fails no supported neighbor"
    );
    assert!(totals(sm)[4] > 0, "M01 fails supported neighbors");
    for (s, st) in correct.strategies() {
        let StrategyStatus::Ran(c) = st else {
            assert!(
                matches!(
                    s,
                    Strategy::AbstractionMapVariants | Strategy::HiddenCorpusMutations
                ),
                "{} did not run",
                s.token()
            );
            continue;
        };
        assert_eq!(
            c.failed,
            0,
            "the correct program fails a neighbor of {}",
            s.token()
        );
        assert_eq!(c.inconclusive, 0);
        assert_eq!(
            c.valid,
            c.executed + c.unsupported + c.not_realizable.values().sum::<u64>(),
            "every valid neighbor is accounted for"
        );
    }
    // Anti-vacuity: the core's own schedule, realized by M01 through the guided
    // scheduler, is the witness run itself, and it fails.
    let core = core();
    let reg = Register::new(core, Program::M01);
    let ran = reg
        .run(core.fates, &core.labels)
        .expect("the core realizes");
    assert!(ran.realization.findings.contains(&core.finding));
    assert_eq!(ran.got, core.labels, "the witness run itself");
    assert!(!ran.halted);
    let t = totals(m01);
    assert!(t[4] > 0, "M01 fails on some neighbor");
    assert!(
        t[3] > 0,
        "and passes some: the neighborhood is not the core repeated"
    );
    // Every failing neighbor is disclosed, once, with a handle.
    let failing: u64 = m01
        .strategies()
        .iter()
        .filter_map(|(_, st)| match st {
            StrategyStatus::Ran(c) => Some(c.failed),
            StrategyStatus::NotRun(_) => None,
        })
        .sum();
    assert_eq!(failing, u64::try_from(m01.failing().len()).expect("small"));
    assert!(
        m01.failing()
            .iter()
            .all(|f| neighborhood::is_handle(&f.run))
    );
    // The core's own schedule strategies reproduce M01's failure somewhere in each.
    for s in [
        Strategy::SchedulePerturbation,
        Strategy::AlternateEnabledEvents,
        Strategy::FaultWindow,
        Strategy::ValueNamePermutation,
    ] {
        assert!(
            m01.coverage(s).expect("ran").failed > 0,
            "M01 fails in {}",
            s.token()
        );
    }
    // Both programs saw the same neighbors, and the same ones were valid.
    let ids = |r: &NeighborhoodRecord| -> Vec<(String, bool)> {
        r.neighbors()
            .iter()
            .map(|n| {
                (
                    n.id.clone(),
                    matches!(n.disposition, Disposition::Rejected(_)),
                )
            })
            .collect()
    };
    assert_eq!(ids(correct), ids(m01));
}

#[test]
fn identical_inputs_give_byte_identical_records() {
    let [_, (m01, ..)] = records();
    let again = campaign(Program::M01, SEED).0;
    assert_eq!(again.canonical_bytes(), m01.canonical_bytes());
    assert_eq!(again.digest(), m01.digest());
    // Canonical bytes parse back to the same document.
    let parsed = Json::parse(&m01.canonical_bytes()).expect("canonical JSON");
    assert_eq!(parsed.to_canonical_bytes(), m01.canonical_bytes());
    // Another seed moves only the seeded walks.
    let other = campaign(Program::M01, SEED ^ 0xdead_beef).0;
    assert_eq!(other.neighbors().len(), m01.neighbors().len());
    for (a, b) in m01.neighbors().iter().zip(other.neighbors()) {
        if a != b {
            assert!(a.id.contains(":walk("), "{} moved with the seed", a.id);
        }
    }
    assert_ne!(
        other.canonical_bytes(),
        m01.canonical_bytes(),
        "the seed is recorded"
    );
}

#[test]
fn invalid_perturbations_are_rejected_not_executed() {
    let core = core();
    let [(correct, rc, ec), (m01, rm, em)] = records();
    for (r, realized, executed) in [(correct, rc, ec), (m01, rm, em)] {
        let valid: u64 = r
            .strategies()
            .iter()
            .filter_map(|(_, st)| match st {
                StrategyStatus::Ran(c) => Some(c.valid),
                StrategyStatus::NotRun(_) => None,
            })
            .sum();
        // Only valid neighbors reach the substrate.
        assert_eq!(u64::try_from(*realized).expect("small"), valid);
        let executed_total: u64 = totals(r)[2];
        assert_eq!(u64::try_from(*executed).expect("small"), executed_total);
        let rejected: BTreeSet<&str> = r
            .neighbors()
            .iter()
            .filter_map(|n| match &n.disposition {
                Disposition::Rejected(x) => Some(x.token()),
                _ => None,
            })
            .collect();
        for want in [
            "violates-causal-order",
            "exceeds-fault-bound",
            "outside-value-domain",
            "leaves-trace-class",
        ] {
            assert!(rejected.contains(want), "no {want} rejection generated");
        }
    }
    // Each hand-built invalid candidate is rejected with its own typed reason.
    let got: Vec<(&str, Result<(), Rejection>)> = hand_built_invalid(core)
        .into_iter()
        .map(|(name, edit, profile)| {
            (
                name,
                neighborhood::check_candidate(
                    &core.order,
                    &envelope(),
                    &profile_of(&core.fates),
                    &edit,
                    &profile,
                ),
            )
        })
        .collect();
    let token = |r: &Result<(), Rejection>| r.as_ref().err().map(Rejection::token);
    let want = [
        ("confirm-before-submit", Some("violates-causal-order")),
        ("not-a-permutation", Some("malformed-schedule")),
        ("three-crashes", Some("exceeds-fault-bound")),
        ("four-cancellations", Some("exceeds-fault-bound")),
        ("value-v2", Some("outside-value-domain")),
        ("duplicate-a-non-send", Some("not-a-message")),
    ];
    for ((name, r), (wname, wtoken)) in got.iter().zip(want) {
        assert_eq!(*name, wname);
        assert_eq!(token(r), wtoken, "{name}");
    }
    assert!(message_neighbors_outside_model());
}

#[test]
fn the_envelope_agrees_with_the_scenario_discipline() {
    // Differential: the generic envelope, from the Intent Contract and the scenario's
    // caps, against register_baseline.rs's own bound and value checks on each scenario
    // neighbor's correct plan.
    let core = core();
    let env = envelope();
    let mut checked = 0;
    for list in core.edits.values() {
        for (id, s, profile) in list {
            let plan = Program::Correct.plan(s.fates);
            let theirs = baseline::discipline(&plan, baseline::SCENARIO)
                .err()
                .map(|v| v.breach);
            let ours = env.check(profile).err();
            let agree = match (&ours, theirs) {
                (None, None) => true,
                (Some(Rejection::ExceedsFaultBound { what, .. }), Some(b)) => matches!(
                    (what, b),
                    (
                        neighborhood::BoundKind::Crashes,
                        baseline::Breach::CrashBound
                    ) | (
                        neighborhood::BoundKind::Cancellations,
                        baseline::Breach::CancellationBound
                    )
                ),
                (
                    Some(Rejection::OutsideValueDomain { .. }),
                    Some(baseline::Breach::ValueOutOfRange),
                ) => true,
                _ => false,
            };
            assert!(agree, "{id}: envelope {ours:?}, discipline {theirs:?}");
            checked += 1;
        }
    }
    assert!(checked > 30);
}

#[test]
fn symmetric_neighbors_keep_the_core_outcome() {
    // Metamorphic, symmetry renaming: every value swap and replica renaming of the core
    // keeps each program's outcome.
    let [(correct, ..), (m01, ..)] = records();
    for (r, want_fail) in [(correct, false), (m01, true)] {
        let sym: Vec<_> = r
            .neighbors()
            .iter()
            .filter(|n| {
                n.strategy == Strategy::ValueNamePermutation
                    && (n.id.contains(":rename(") || n.id.contains(":swap-values"))
            })
            .collect();
        assert_eq!(sym.len(), 11, "every value and name permutation");
        for n in sym {
            match &n.disposition {
                Disposition::Executed(Execution::Fail { .. }) => {
                    assert!(want_fail, "{} fails under {}", n.id, r.subject());
                }
                Disposition::Executed(Execution::Pass { .. }) => {
                    assert!(!want_fail, "{} passes under {}", n.id, r.subject());
                }
                other => panic!("{} was {other:?}", n.id),
            }
        }
    }
}

#[test]
fn same_class_schedules_keep_the_core_outcome() {
    // Metamorphic, independent-event swap: a schedule-perturbation neighbor is in the
    // core's trace class, so M01 still fails on it and the correct program still passes.
    let [(correct, ..), (m01, ..)] = records();
    for (r, want_fail) in [(correct, false), (m01, true)] {
        let executed: Vec<_> = r
            .neighbors()
            .iter()
            .filter(|n| n.strategy == Strategy::SchedulePerturbation)
            .filter(|n| matches!(n.disposition, Disposition::Executed(_)))
            .collect();
        // M01 realizes every one; the correct program realizes those that are causal
        // neighbors of its runs (possibly none), and passes each it realizes.
        assert!(!want_fail || executed.len() > 20);
        for n in executed {
            let failed = matches!(n.disposition, Disposition::Executed(Execution::Fail { .. }));
            assert_eq!(failed, want_fail, "{} under {}", n.id, r.subject());
        }
    }
}

#[test]
fn an_exhausted_budget_suspends_and_resumes_to_the_same_result() {
    // Gate 5 stays pending with a continuation; the continuation, serialized and read
    // back, resumes without re-running a committed neighbor, to the uninterrupted
    // result byte for byte.
    let [_, (full, full_realized, _)] = records();
    for runs in [0, 1, 5, 23, 40, 71] {
        let mut s = Register::new(core(), Program::M01);
        let mut cfg = config(SEED);
        cfg.budget.runs = runs;
        let Exploration::Suspended {
            record,
            continuation,
        } = neighborhood::explore(&mut s, &envelope(), &cfg).expect("a campaign")
        else {
            panic!("{runs} runs cannot finish the campaign");
        };
        assert_eq!(record.verdict(), Verdict::Pending);
        assert_eq!(
            s.realized,
            usize::try_from(runs).expect("small"),
            "no run past the budget"
        );
        assert_eq!(
            project(&record, Program::M01, &cfg),
            Err(neighborhood::ProjectionRefusal::Pending),
            "a pending campaign is never projected"
        );
        assert!(
            record
                .strategies()
                .iter()
                .any(|(_, st)| matches!(st, StrategyStatus::Ran(c) if c.frontier.is_some()))
        );
        // In process: the committed neighbors are carried, not re-run.
        let mut s2 = Register::new(core(), Program::M01);
        let Exploration::Finished(done) = neighborhood::resume(
            &mut s2,
            &envelope(),
            &config(SEED),
            &continuation,
            &mut Spent::default(),
        )
        .expect("resumes") else {
            panic!("the full budget finishes");
        };
        assert_eq!(done.result_bytes(), full.result_bytes(), "runs {runs}");
        assert_eq!(s.realized + s2.realized, *full_realized, "nothing re-run");
        // From bytes: nothing is taken on trust; a fresh derivation checks them.
        let bytes = continuation.canonical_bytes();
        let mut s3 = Register::new(core(), Program::M01);
        let checked = neighborhood::resume_untrusted(
            &mut s3,
            &envelope(),
            &config(SEED),
            &bytes,
            &mut Spent::default(),
        )
        .expect("genuine bytes check out");
        assert_eq!(checked.record().result_bytes(), full.result_bytes());
        assert_eq!(s3.realized, *full_realized, "re-derived in full");
        // A committed M01 failure rewritten into a pass, with every count and the
        // failing list edited to agree, is refused by the derivation.
        if !record.failing().is_empty() {
            let forged = coherent_forgery(&bytes);
            assert!(matches!(
                neighborhood::resume_untrusted(
                    &mut Register::new(core(), Program::M01),
                    &envelope(),
                    &config(SEED),
                    &forged,
                    &mut Spent::default()
                ),
                Err(neighborhood::ResumeError::Forged { .. })
            ));
        }
    }
}

/// Rewrite the first committed failure in continuation `bytes` into a pass, with the
/// strategy's counts and the failing list edited to agree: a coherent forgery.
fn coherent_forgery(bytes: &[u8]) -> Vec<u8> {
    let mut doc = Json::parse(bytes).expect("json");
    let Json::Object(top) = &mut doc else {
        panic!("object")
    };
    let Some(Json::Object(rec)) = top.get_mut("record") else {
        panic!("record")
    };
    let Some(Json::Array(fails)) = rec.get_mut("failing_neighbors") else {
        panic!("list")
    };
    let victim = fails.remove(0);
    let v = victim.as_object().expect("entry");
    let id = v["neighbor"].as_str().expect("id").to_owned();
    let strategy = v["strategy"].as_str().expect("strategy").to_owned();
    let Some(Json::Array(ns)) = rec.get_mut("neighbors") else {
        panic!("neighbors")
    };
    for n in ns.iter_mut() {
        let Json::Object(n) = n else {
            panic!("neighbor")
        };
        if n["neighbor"].as_str() == Some(id.as_str()) {
            n.insert("disposition".to_owned(), Json::String("pass".to_owned()));
            n.remove("detail");
        }
    }
    let Some(Json::Array(ss)) = rec.get_mut("strategies") else {
        panic!("strategies")
    };
    for st in ss.iter_mut() {
        let Json::Object(st) = st else {
            panic!("strategy")
        };
        if st["strategy"].as_str() == Some(strategy.as_str()) {
            let Json::Integer(f) = st["failed"] else {
                panic!("failed")
            };
            let Json::Integer(p) = st["passed"] else {
                panic!("passed")
            };
            st.insert("failed".to_owned(), Json::Integer(f - 1));
            st.insert("passed".to_owned(), Json::Integer(p + 1));
        }
    }
    doc.to_canonical_bytes()
}

#[test]
fn receipt_coverage_conforms_to_the_promotion_receipt_schema() {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../notes/plan/schemas/promotion-receipt.schema.json"
    );
    let schema = Json::parse(&std::fs::read(path).expect("the schema")).expect("JSON");
    let field = |j: &Json, k: &str| -> Json {
        j.as_object()
            .and_then(|o| o.get(k))
            .cloned()
            .unwrap_or_else(|| panic!("no {k}"))
    };
    let defs = field(&schema, "$defs");
    let enum_tokens: BTreeSet<String> = field(&field(&defs, "neighborhood_strategy"), "enum")
        .as_array()
        .expect("enum")
        .iter()
        .map(|t| t.as_str().expect("token").to_owned())
        .collect();
    let ours: BTreeSet<String> = Strategy::ALL.iter().map(|s| s.token().to_owned()).collect();
    assert_eq!(enum_tokens, ours, "the strategy set is the schema's");
    let nb = field(
        &field(&field(&schema, "properties"), "coverage"),
        "properties",
    );
    let nb = field(&nb, "neighborhood");
    let keys =
        |j: &Json| -> BTreeSet<String> { j.as_object().expect("object").keys().cloned().collect() };
    let strings = |j: &Json| -> BTreeSet<String> {
        j.as_array()
            .expect("array")
            .iter()
            .map(|t| t.as_str().expect("string").to_owned())
            .collect()
    };
    let top_props = keys(&field(&nb, "properties"));
    let top_req = strings(&field(&nb, "required"));
    let strat = field(&field(&field(&nb, "properties"), "strategies"), "items");
    let strat_props = keys(&field(&strat, "properties"));
    let strat_req = strings(&field(&strat, "required"));
    let fail = field(
        &field(&field(&nb, "properties"), "failing_neighbors"),
        "items",
    );
    let fail_props = keys(&field(&fail, "properties"));
    let fail_req = strings(&field(&fail, "required"));
    for (r, program) in supported_records()
        .iter()
        .zip([Program::Correct, Program::M01])
    {
        let cov = project(r, program, &supported_config(SEED)).expect("projects");
        let k = keys(&cov);
        assert!(k.is_subset(&top_props) && top_req.is_subset(&k));
        let mut seen = BTreeSet::new();
        for s in field(&cov, "strategies").as_array().expect("array") {
            let k = keys(s);
            assert!(
                k.is_subset(&strat_props) && strat_req.is_subset(&k),
                "{k:?}"
            );
            let token = field(s, "strategy").as_str().expect("token").to_owned();
            assert!(enum_tokens.contains(&token));
            assert!(seen.insert(token.clone()), "one entry per strategy");
            let failing = field(&cov, "failing_neighbors")
                .as_array()
                .expect("array")
                .iter()
                .filter(|f| field(f, "strategy").as_str() == Some(token.as_str()))
                .count();
            assert_eq!(
                field(s, "failing"),
                Json::Integer(i64::try_from(failing).expect("small")),
                "failing equals the disclosed entries"
            );
        }
        let mut unique = BTreeSet::new();
        for f in field(&cov, "failing_neighbors").as_array().expect("array") {
            let k = keys(f);
            assert!(k.is_subset(&fail_props) && fail_req.is_subset(&k), "{k:?}");
            assert!(neighborhood::is_handle(
                field(f, "run").as_str().expect("run")
            ));
            assert!(unique.insert(f.to_canonical_bytes()), "uniqueItems");
        }
    }
}

#[test]
fn the_evidence_matches_its_golden() {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/golden/pr21_impl01_neighborhood.evidence.txt"
    );
    let got = evidence();
    if std::env::var_os("PR21_IMPL01_BLESS").is_some() {
        std::fs::write(path, &got).expect("write the golden");
    }
    let want = std::fs::read_to_string(path).expect("the golden exists");
    assert!(
        got == want,
        "the evidence changed; rerun with PR21_IMPL01_BLESS=1 and review"
    );
}

#[test]
fn the_correct_program_is_executed_only_on_causal_neighbors_it_realizes() {
    // Under the correct program, a schedule neighbor of M01's core is executed only
    // when the library's causal-neighbor check accepts the run; every other valid one
    // is not realizable with the check's reason. M01 realizes every valid schedule
    // neighbor of its own core.
    let [(correct, ..), (m01, ..)] = records();
    for s in [
        Strategy::AlternateEnabledEvents,
        Strategy::SchedulePerturbation,
        Strategy::MessageDuplicationLossDelay,
    ] {
        let c = m01.coverage(s).expect("ran");
        assert_eq!(
            c.executed + c.unsupported,
            c.valid,
            "M01 realizes {}",
            s.token()
        );
        let c = correct.coverage(s).expect("ran");
        for why in c.not_realizable.keys() {
            assert!(why.starts_with("witness: "), "{}: {why}", s.token());
        }
    }
    // Every executed neighbor carries its checked witness.
    for r in [correct, m01] {
        for n in r.neighbors() {
            assert_eq!(
                matches!(n.disposition, Disposition::Executed(_)),
                !n.witness.is_empty(),
                "{}",
                n.id
            );
        }
    }
}

#[test]
fn every_scenario_identity_names_exactly_one_content() {
    // Over the register's whole scenario corpus: within each strategy, one identity
    // names one content and one content has one identity, and no campaign under
    // either program rejects an identity collision.
    let core = core();
    let reg = Register::new(core, Program::M01);
    for (s, list) in &core.edits {
        let mut by_id: BTreeMap<&str, Vec<u8>> = BTreeMap::new();
        let mut by_content: BTreeMap<Vec<u8>, &str> = BTreeMap::new();
        for (id, scenario, _) in list {
            let bytes = reg.scenario_bytes(scenario);
            if let Some(prev) = by_id.insert(id, bytes.clone()) {
                assert_eq!(prev, bytes, "{}: {id} names two contents", s.token());
            }
            if let Some(prev) = by_content.insert(bytes, id) {
                assert_eq!(
                    prev,
                    id.as_str(),
                    "{}: one content, two identities",
                    s.token()
                );
            }
        }
    }
    let [(correct, ..), (m01, ..)] = records();
    for r in [correct, m01] {
        for (_, st) in r.strategies() {
            if let StrategyStatus::Ran(c) = st {
                assert!(!c.rejected.contains_key("identity-collision"));
            }
        }
    }
}
