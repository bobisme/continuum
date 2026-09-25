//! PR 18 (bn-25z9o): owner, fault and value-domain reduction of M01's failing runs, each
//! candidate re-run as a real run of the register program, composed with bn-2z08o's
//! closure and deletion. START_HERE PR 18 ("Implement deletion, causal-closure,
//! owner/fault/value reduction, and replay validation"; exit: "ack-before-sync failure
//! reduces to a compact core and does not delete the actual causal mechanism").
//!
//! # Why the configuration, not the events
//!
//! bn-2z08o's program replay keeps M01's mechanism, but the core stays most of the run:
//! the program's setup opens every region and spawns every task before any write, so a
//! core that keeps one write keeps the whole setup, and a replica that plays no part in
//! the failure still owns a region, a writer and a coordinator. No deletion of events
//! removes them: a candidate without them is not a run of the program. RFC 0028 names
//! three classes for this, `OwnerMinimal`, `FaultMinimal` and `ValueMinimal`, each a
//! minimizer transcript over a dimension of the configuration.
//! `continuum_debugger::scenario` reduces the configuration, and this file instantiates
//! it on the register.
//!
//! # The construction
//!
//! A configuration ([`Config`]) is what the campaign's plans are made of: the epochs, and
//! for each replica its values and the fate of each slot (`register_baseline::Replica`,
//! the scenario's `[domains]` and `[faults]` vocabulary), plus whether the build spawns
//! only the owners the scripts use (`register::build_pruned`). The defect is not part of
//! it: every configuration is built as M01's plan (`register_mutants::ack_before_sync`
//! of the correct plan), so the code under diagnosis never changes. The three passes:
//!
//! - **owner**: spawn only the owners the scripts use; or make one replica's slot idle,
//!   so the replica neither writes it nor owns a writer, a region or a coordinator
//!   confirmation for it. The model keeps three nodes: an idle replica is a node that
//!   never starts, and its slots stay free. Measure: tasks plus regions of the build.
//! - **fault**: drop one crash or cancellation (the slot is written cleanly, with the
//!   value before the fault or the value the fault re-proposed), or move one crash to an
//!   earlier point of its write (after reply, after sync, after submit, after reserve).
//!   Measure: 16 per fault plus its depth.
//! - **value**: drop one epoch (the other is renamed `0`), or rename one value to
//!   another, so the domain has one value fewer. Measure: four per epoch plus the values
//!   used.
//!
//! A candidate is **run**, never assumed ([`RegisterScenario::run`]): it is built and
//! executed through the binding, and the campaign's own checker judges it, with the
//! target and the mechanism both required. The schedule is not the original choice log,
//! which indexes the original program's actors. It is the original run's order carried
//! over ([`OpKey`], `register::guided_log`): each operation of the reduced program is
//! identified by its replica, act, epoch and occurrence, or its coordinator and index;
//! the guided walk runs the admissible operation that came first in the original run.
//! The start configuration is run this way too, and its log is the original log
//! exactly. When the guided run does not fail, or parks every actor, a seeded search runs
//! more schedules
//! (`register_baseline::logs_for`): all of them when there are few, or a sample. The
//! verdict is then `Fails`, `Holds` when the search was exhaustive, or an INV-008
//! inconclusive when it was sampled: a sample that finds nothing decides nothing.
//!
//! Most removals are decided without a schedule. An acknowledgement is a coordinator's
//! commit, and a coordinator's program has one only when at least two replicas confirm
//! its value (`register::build_with_shutdown`). So a program in which no epoch has two
//! acknowledging coordinators has no run that acknowledges two values of one epoch
//! (Agreement), and a program with no acknowledging coordinator has no run that
//! acknowledges at all (AckedNotDurable). This is read from the built program's
//! operations. The evidence runs every distinct configuration so decided, by the
//! witnesses' and the corpus's reductions, on its schedules: all of them where there are
//! at most [`SEARCH_LOGS`], a seeded sample otherwise (`pr18-impl02-06`).
//!
//! # Scope (INV-013)
//!
//! Every pass declares that it keeps the property, the defect, the semantics and the
//! campaign's bounds. Each candidate must be a correct plan within the scenario's
//! declared bounds (`register_baseline::discipline`), and under Agreement it must keep
//! two values: Agreement is a statement about two distinct values of one epoch, and a
//! one-value domain makes it vacuous. A candidate outside the scope is recorded and never
//! run. A question that names a value (the scenario's `pinned` value) refuses the value
//! pass altogether: renaming values would change the checked property.
//!
//! # Composition
//!
//! The reduced configuration's failing run is then reduced by bn-2z08o's closure and
//! deletion over atoms under the program replay. The core is measured against the
//! **original** run's length, which is what research/26's ratified sentence measures
//! ("the replay-preserving causal core must be ≤10% of trace length at the corpus
//! median"), and also against the reduced run.
//!
//! # The evidence, by stable artifact ID
//!
//! `tests/golden/pr18_impl02_scenario_reduction.evidence.txt` and
//! `tests/golden/pr18_impl02_transcripts.txt`. Regenerate with `PR18_IMPL02_BLESS=1
//! cargo test -p continuum-asupersync --test pr18_impl02_scenario_reduction` and review
//! the diff.
//!
//! - `pr18-impl02-01-m01-agreement-scenario` and
//!   `pr18-impl02-02-m01-acked-not-durable-scenario`: each witness's scenario reduction,
//!   the reduced configuration and its run, the composed core, and the ratios;
//! - `pr18-impl02-03-corpus`: the 400 failing runs of M01's scenario plan, the core ratio
//!   at the median and the maximum, and the PR-18 exit measurement;
//! - `pr18-impl02-04-scope`: the INV-013 refusals;
//! - `pr18-impl02-05-determinism`;
//! - `pr18-impl02-06-static-decisions`: the schedule-free decisions checked against
//!   sampled schedules;
//! - `pr18-impl02-07-m01-agreement-transcript` and
//!   `pr18-impl02-08-m01-acked-not-durable-transcript`: RFC 0028's transcripts, the
//!   scenario's and then the events'.

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

#[path = "support/pr18_program.rs"]
#[allow(dead_code)]
mod program;

use baseline::{Fate, Replica};
use continuum_asupersync::binding::SubstrateOp;
use continuum_asupersync::choice::ChoiceLog;
use continuum_debugger::reduce::{Budget, Deletion, Guarantee, Reduction};
use continuum_debugger::scenario::{
    Candidate, ConfigAttempt, ConfigVerdict, Dimension, DimensionEnd, Preserved, Ran, Scenario,
    ScenarioGuarantee, ScenarioReduction, reduce_scenario,
};
use continuum_value::assurance::InconclusiveReason;
use program::{
    Case, NODES, ProgramReduction, Target, acked_witness, agreement_witness, attempt_line,
    campaign_finds_target, case_built, corpus, keeps_mechanism, median, minimize_program, story,
    transcript_licenses,
};
use register::{Act, Built, Plan, Raw, Role};

/// The scenario reduction's budget: runs, and work units.
const SCENARIO_BUDGET: Budget = Budget::new(512, 1 << 34);

/// Schedules the search runs when the guided run does not fail: every admissible one when
/// there are at most this many, otherwise this many sampled.
const SEARCH_LOGS: usize = 24;

/// Predicted work per act of a configuration's plan, per schedule run: the binding's
/// events, the lift, the model and the projection, each linear in the run.
const WORK_PER_ACT: u64 = 1 << 12;

// ---------------------------------------------------------------------------
// the configuration
// ---------------------------------------------------------------------------

/// A register configuration: the plan's epochs and replicas, and whether the build spawns
/// only the owners the scripts use.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct Config {
    epochs: u8,
    replicas: [Replica; 3],
    pruned: bool,
}

impl Config {
    fn start(epochs: u8, replicas: [Replica; 3]) -> Self {
        Self {
            epochs,
            replicas,
            pruned: false,
        }
    }

    /// The correct plan, before the defect.
    fn correct_plan(&self) -> Plan {
        baseline::plan_of("scenario-reduction", self.epochs, self.replicas)
    }

    /// M01's plan: the correct plan with its defect.
    fn plan(&self) -> Plan {
        mutants::ack_before_sync(&self.correct_plan())
    }

    fn built(&self) -> Built {
        let plan = self.plan();
        if self.pruned {
            register::build_pruned(&plan)
        } else {
            register::build_with_shutdown(&plan)
        }
    }

    /// The written slots, `(replica, slot index, epoch, fate)`.
    fn slots(&self) -> impl Iterator<Item = (usize, usize, u8, Fate)> + '_ {
        self.replicas.iter().enumerate().flat_map(move |(n, r)| {
            r.slots[..usize::from(self.epochs)]
                .iter()
                .enumerate()
                .filter(|(_, (_, f))| *f != Fate::Idle)
                .map(move |(k, &(e, f))| (n, k, e, f))
        })
    }

    /// The values the written slots use: each slot's value and each re-proposed value.
    fn values(&self) -> BTreeSet<u8> {
        let mut out = BTreeSet::new();
        for (n, _, e, f) in self.slots() {
            out.insert(self.replicas[n].values[usize::from(e)]);
            if let Fate::CrashReserved { retry } | Fate::CrashSubmitted { retry } = f {
                out.insert(retry);
            }
        }
        out
    }

    fn acts(&self) -> u64 {
        self.plan()
            .replicas
            .iter()
            .map(|s| s.len() as u64)
            .sum::<u64>()
    }

    fn render(&self) -> String {
        let replicas: Vec<String> = self
            .replicas
            .iter()
            .enumerate()
            .map(|(n, r)| {
                let slots: Vec<String> = r.slots[..usize::from(self.epochs)]
                    .iter()
                    .map(|&(e, f)| {
                        if f == Fate::Idle {
                            format!("e{e} idle")
                        } else {
                            format!(
                                "e{e} {} {}",
                                register::VALUES[usize::from(r.values[usize::from(e)])],
                                f.token()
                            )
                        }
                    })
                    .collect();
                format!("{}: {}", NODES[n], slots.join(", "))
            })
            .collect();
        format!(
            "epochs {}, {}; {}",
            self.epochs,
            if self.pruned {
                "only used owners spawned"
            } else {
                "every owner spawned"
            },
            replicas.join(" | ")
        )
    }
}

/// How deep in its write a fault falls: the owner-free part of the fault measure.
const fn depth(f: Fate) -> u64 {
    match f {
        Fate::AbortRetry | Fate::CrashReserved { .. } => 1,
        Fate::CrashSubmitted { .. } => 2,
        Fate::CrashSynced => 3,
        Fate::CrashReplied => 4,
        Fate::Idle | Fate::Clean => 0,
    }
}

const fn faults(f: Fate) -> bool {
    depth(f) > 0
}

// ---------------------------------------------------------------------------
// the original run's order, carried to a reduced program
// ---------------------------------------------------------------------------

/// An operation's identity across configurations: its replica, act, epoch and how many
/// times that act on that epoch came before it in the replica's script; or its
/// coordinator and index; or its place in the setup or the shutdown.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum OpKey {
    Setup(usize),
    Replica {
        node: usize,
        act: u8,
        epoch: u8,
        occurrence: usize,
    },
    Coordinator {
        epoch: u8,
        value: u8,
        index: usize,
    },
    Shutdown(usize),
}

const fn act_code(a: Act) -> (u8, u8) {
    match a {
        Act::Reserve(e) => (0, e),
        Act::Submit(e) => (1, e),
        Act::Sync(e) => (2, e),
        Act::Release(e) => (3, e),
        Act::Abort(e) => (4, e),
        Act::Confirm(e) => (5, e),
        Act::Crash | Act::CrashRepropose(..) => (6, 0),
    }
}

/// Each actor's operations' keys. A build without carriers has one operation per act in
/// each replica's program; that is checked.
fn op_keys(plan: &Plan, built: &Built) -> Vec<Vec<OpKey>> {
    let actors = built.programs.len();
    let mut out = vec![(0..built.programs[0].len()).map(OpKey::Setup).collect()];
    for (n, script) in plan.replicas.iter().enumerate() {
        assert_eq!(
            script.len(),
            built.programs[1 + n].len(),
            "one operation per act"
        );
        let mut seen: BTreeMap<(u8, u8), usize> = BTreeMap::new();
        out.push(
            script
                .iter()
                .map(|&a| {
                    let (act, epoch) = act_code(a);
                    let k = seen.entry((act, epoch)).or_default();
                    let key = OpKey::Replica {
                        node: n,
                        act,
                        epoch,
                        occurrence: *k,
                    };
                    *k += 1;
                    key
                })
                .collect(),
        );
    }
    let coordinators: Vec<(u8, u8)> = built
        .roles
        .tasks
        .iter()
        .filter_map(|(r, _)| match *r {
            Role::Coordinator { epoch, value } => Some((epoch, value)),
            _ => None,
        })
        .collect();
    assert_eq!(
        4 + coordinators.len() + 1,
        actors,
        "setup, replicas, coordinators, shutdown"
    );
    for (k, &(epoch, value)) in coordinators.iter().enumerate() {
        out.push(
            (0..built.programs[4 + k].len())
                .map(|index| OpKey::Coordinator {
                    epoch,
                    value,
                    index,
                })
                .collect(),
        );
    }
    out.push(
        (0..built.programs[actors - 1].len())
            .map(OpKey::Shutdown)
            .collect(),
    );
    out
}

/// The original run's order: each operation's key, by its position in the run.
fn original_rank(c: &Case, plan: &Plan) -> BTreeMap<OpKey, u64> {
    let keys = op_keys(plan, &c.built);
    let programs = &c.built.programs;
    let mut cursor = vec![0_usize; programs.len()];
    let mut out = BTreeMap::new();
    for (at, choice) in c.log.choices().iter().enumerate() {
        let enabled: Vec<usize> = (0..programs.len())
            .filter(|&a| cursor[a] < programs[a].len())
            .collect();
        let actor = enabled[choice.0 as usize];
        out.insert(keys[actor][cursor[actor]], at as u64);
        cursor[actor] += 1;
    }
    out
}

// ---------------------------------------------------------------------------
// the scenario
// ---------------------------------------------------------------------------

/// The durable register's reachable states, by epochs, computed once each.
fn reach(epochs: u8) -> &'static BTreeSet<Raw> {
    static CELL: OnceLock<[BTreeSet<Raw>; 2]> = OnceLock::new();
    &CELL.get_or_init(|| [register::reachable(1), register::reachable(2)])[usize::from(epochs - 1)]
}

/// How a configuration's run was found.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Found {
    /// The original order, carried over.
    Guided,
    /// The `k`th schedule of the search.
    Searched(usize),
}

/// The register's configuration space around one failing run.
struct RegisterScenario {
    target: Target,
    /// A value the question names, if any: the value pass is then refused (INV-013).
    pinned: Option<u8>,
    rank: BTreeMap<OpKey, u64>,
    /// Each configuration that failed: its failing schedule and how it was found.
    found: BTreeMap<Config, (ChoiceLog, Found)>,
    /// Each configuration decided without a schedule.
    decided_statically: Vec<Config>,
    /// Binding executions: two per schedule judged (the campaign's checker, then the
    /// journal the mechanism is read from).
    executions: std::cell::Cell<u64>,
}

impl RegisterScenario {
    fn new(c: &Case, start: &Config) -> Self {
        let mut s = Self::for_target(c.target);
        s.rank = original_rank(c, &start.plan());
        s
    }

    /// A scenario with no original run to follow: every operation ranks alike, so the
    /// guided walk is the lowest admissible actor first.
    fn for_target(target: Target) -> Self {
        Self {
            target,
            pinned: None,
            rank: BTreeMap::new(),
            found: BTreeMap::new(),
            decided_statically: Vec::new(),
            executions: std::cell::Cell::new(0),
        }
    }

    fn measure_of(c: &Config) -> [u64; 3] {
        let built = c.built();
        let owners = built.roles.tasks.len() as u64 + u64::from(built.roles.regions);
        let faults: u64 = c
            .slots()
            .filter(|(_, _, _, f)| faults(*f))
            .map(|(_, _, _, f)| 16 + depth(f))
            .sum();
        let values = 4 * u64::from(c.epochs) + c.values().len() as u64;
        [owners, faults, values]
    }

    /// The minimum number of values the target's statement needs.
    const fn values_needed(&self) -> usize {
        match self.target {
            Target::Agreement => 2,
            Target::AckedNotDurable => 1,
        }
    }

    /// The schedule-free decision: whether the built program has the acknowledgements the
    /// target needs. `None` when it may; a reason when no run can fail.
    fn statically_holds(&self, built: &Built) -> Option<String> {
        // The layout of a build without carriers: setup, three replicas, the coordinators
        // in role order, the shutdown.
        let coordinators = built
            .roles
            .tasks
            .iter()
            .filter(|(r, _)| matches!(r, Role::Coordinator { .. }))
            .count();
        assert!(
            built.roles.mailboxes.is_empty() && built.programs.len() == 4 + coordinators + 1,
            "setup, replicas, coordinators, shutdown"
        );
        let mut ackers: BTreeMap<u8, usize> = BTreeMap::new();
        let mut k = 0;
        for (role, _) in &built.roles.tasks {
            if let Role::Coordinator { epoch, .. } = *role {
                if built.programs[4 + k]
                    .iter()
                    .any(|op| matches!(op, SubstrateOp::Reserve { .. }))
                {
                    *ackers.entry(epoch).or_default() += 1;
                }
                k += 1;
            }
        }
        match self.target {
            Target::Agreement if ackers.values().all(|&n| n < 2) => Some(
                "decided without a schedule: no epoch has two coordinators with an acknowledgement in their program, so no run acknowledges two values of one epoch"
                    .to_owned(),
            ),
            Target::AckedNotDurable if ackers.is_empty() => Some(
                "decided without a schedule: no coordinator has an acknowledgement in its program, so no run acknowledges"
                    .to_owned(),
            ),
            _ => None,
        }
    }

    /// Run `built` under `log` and check the target and the mechanism.
    fn fails(&self, config: &Config, built: &Built, log: &ChoiceLog) -> Result<(), String> {
        self.executions.set(self.executions.get() + 2);
        let Some(c) = case_built(
            String::new(),
            built.clone(),
            config.epochs,
            log,
            Some(self.target),
            reach(config.epochs),
        ) else {
            return Err("the target does not fail".to_owned());
        };
        let all: Vec<usize> = (0..c.journal.len()).collect();
        if keeps_mechanism(&story(&c, &all), self.target) {
            Ok(())
        } else {
            Err("the target fails without the mechanism".to_owned())
        }
    }

    fn seed(config: &Config) -> u64 {
        // FNV-1a over the configuration's fields, each spelled by this file (the fates by
        // their stable tokens): a function of the configuration alone (INV-005, INV-006).
        let mut bytes = vec![config.epochs, u8::from(config.pruned)];
        for r in &config.replicas {
            bytes.extend(r.values);
            bytes.push(r.epochs);
            for &(e, f) in &r.slots {
                bytes.push(e);
                bytes.extend(f.token().bytes());
                bytes.push(0);
            }
        }
        let mut h: u64 = 0xcbf2_9ce4_8422_2325;
        for b in bytes {
            h ^= u64::from(b);
            h = h.wrapping_mul(0x0100_0000_01b3);
        }
        h ^ baseline::SCENARIO_SEED
    }
}

impl Scenario for RegisterScenario {
    type Config = Config;

    fn declare(&self, dimension: Dimension) -> Result<Vec<Preserved>, String> {
        if dimension == Dimension::Value {
            if let Some(v) = self.pinned {
                return Err(format!(
                    "the question names {}: renaming values would change the checked property (INV-013)",
                    register::VALUES[usize::from(v)]
                ));
            }
        }
        Ok(vec![
            Preserved::Property,
            Preserved::Defect,
            Preserved::Semantics,
            Preserved::Bounds,
        ])
    }

    fn measure(&self, config: &Config) -> [u64; 3] {
        Self::measure_of(config)
    }

    fn candidates(&self, config: &Config, dimension: Dimension) -> Vec<Candidate<Config>> {
        let mut out: Vec<Candidate<Config>> = Vec::new();
        let slot_name = |n: usize, e: u8| format!("replica {} epoch {e}", NODES[n]);
        match dimension {
            Dimension::Owner => {
                // Pruning is offered only when some owner is unused: otherwise the pruned
                // build is the same program (`pruning_changes_nothing_when_every_owner_is_used`)
                // and removes nothing. Every other candidate goes to the engine, which
                // records and stops on any that is not smaller.
                if !config.pruned {
                    let mut c = config.clone();
                    c.pruned = true;
                    if Self::measure_of(&c)[0] < Self::measure_of(config)[0] {
                        out.push(Candidate {
                            config: c,
                            label: "spawn only the owners the scripts use".to_owned(),
                        });
                    }
                }
                for n in (0..3).rev() {
                    for k in (0..usize::from(config.epochs)).rev() {
                        let (e, f) = config.replicas[n].slots[k];
                        if f == Fate::Idle {
                            continue;
                        }
                        let mut c = config.clone();
                        c.replicas[n].slots[k] = (e, Fate::Idle);
                        c.pruned = true;
                        out.push(Candidate {
                            config: c,
                            label: format!("{} idle: its writers are not spawned", slot_name(n, e)),
                        });
                    }
                }
            }
            Dimension::Fault => {
                for n in (0..3).rev() {
                    for k in (0..usize::from(config.epochs)).rev() {
                        let (e, f) = config.replicas[n].slots[k];
                        if !faults(f) {
                            continue;
                        }
                        let value = config.replicas[n].values[usize::from(e)];
                        let mut clean = vec![value];
                        if let Fate::CrashReserved { retry } | Fate::CrashSubmitted { retry } = f {
                            if retry != value {
                                clean.push(retry);
                            }
                        }
                        for v in clean {
                            let mut c = config.clone();
                            c.replicas[n].slots[k] = (e, Fate::Clean);
                            c.replicas[n].values[usize::from(e)] = v;
                            out.push(Candidate {
                                config: c,
                                label: format!(
                                    "{}: fault {} dropped, writes {}",
                                    slot_name(n, e),
                                    f.token(),
                                    register::VALUES[usize::from(v)]
                                ),
                            });
                        }
                        let earlier = match f {
                            Fate::CrashReplied => Some(Fate::CrashSynced),
                            Fate::CrashSynced => Some(Fate::CrashSubmitted { retry: value }),
                            Fate::CrashSubmitted { retry } => Some(Fate::CrashReserved { retry }),
                            _ => None,
                        };
                        if let Some(g) = earlier {
                            let mut c = config.clone();
                            c.replicas[n].slots[k] = (e, g);
                            out.push(Candidate {
                                config: c,
                                label: format!(
                                    "{}: fault {} moved earlier to {}",
                                    slot_name(n, e),
                                    f.token(),
                                    g.token()
                                ),
                            });
                        }
                    }
                }
            }
            Dimension::Value => {
                if config.epochs == 2 {
                    for keep in 0..2_u8 {
                        let mut c = config.clone();
                        c.epochs = 1;
                        for r in &mut c.replicas {
                            let fate = r
                                .slots
                                .iter()
                                .find(|(e, _)| *e == keep)
                                .map_or(Fate::Idle, |s| s.1);
                            *r = Replica {
                                values: [r.values[usize::from(keep)], 0],
                                slots: [(0, fate), (1, Fate::Idle)],
                                epochs: 1,
                            };
                        }
                        out.push(Candidate {
                            config: c,
                            label: format!("epoch {} dropped, epoch {keep} renamed 0", 1 - keep),
                        });
                    }
                }
                let used = config.values();
                for &w in &used {
                    for &u in &used {
                        if w == u {
                            continue;
                        }
                        let rename = |v: u8| if v == w { u } else { v };
                        let mut c = config.clone();
                        for r in &mut c.replicas {
                            r.values = r.values.map(rename);
                            for s in &mut r.slots {
                                s.1 = match s.1 {
                                    Fate::CrashReserved { retry } => Fate::CrashReserved {
                                        retry: rename(retry),
                                    },
                                    Fate::CrashSubmitted { retry } => Fate::CrashSubmitted {
                                        retry: rename(retry),
                                    },
                                    other => other,
                                };
                            }
                        }
                        out.push(Candidate {
                            config: c,
                            label: format!(
                                "{} renamed {}: one value fewer",
                                register::VALUES[usize::from(w)],
                                register::VALUES[usize::from(u)]
                            ),
                        });
                    }
                }
            }
        }
        out
    }

    fn candidates_cost(&self, config: &Config) -> u64 {
        // At most 3 * 2 * 3 candidates per dimension, each built twice (its measure here
        // and the engine's), each build linear in the acts.
        64 * WORK_PER_ACT * (8 + config.acts())
    }

    fn admits(&self, _dimension: Dimension, config: &Config) -> Result<(), String> {
        if let Err(v) = baseline::discipline(&config.correct_plan(), baseline::SCENARIO) {
            return Err(format!(
                "outside the campaign's declared bounds or the correct protocol: {v:?}"
            ));
        }
        if let Some(v) = self.pinned {
            if !config.values().contains(&v) {
                return Err(format!(
                    "the question names {}: a configuration that no longer uses it changes the checked property (INV-013)",
                    register::VALUES[usize::from(v)]
                ));
            }
        }
        if config.values().len() < self.values_needed() {
            return Err(format!(
                "{} is a statement about two distinct values of one epoch: a one-value domain makes it vacuous, which changes what is checked (INV-013)",
                self.target.name()
            ));
        }
        Ok(())
    }

    fn run_cost(&self, config: &Config) -> u64 {
        (1 + SEARCH_LOGS as u64) * WORK_PER_ACT * (8 + config.acts())
    }

    fn run(&mut self, config: &Config) -> Ran {
        let plan = config.plan();
        let built = config.built();
        if let Some(why) = self.statically_holds(&built) {
            self.decided_statically.push(config.clone());
            return Ran::Holds(why);
        }
        let keys = op_keys(&plan, &built);
        let fallback = u64::MAX / 2;
        let guided = register::guided_log(&built, |a, i| {
            self.rank.get(&keys[a][i]).copied().unwrap_or(fallback)
        });
        // A guided walk that parks every actor says nothing of the program's other
        // schedules: the search below still runs.
        let first = match guided {
            None => "every actor parks under it".to_owned(),
            Some(guided) => match self.fails(config, &built, &guided) {
                Ok(()) => {
                    self.found.insert(config.clone(), (guided, Found::Guided));
                    return Ran::Fails;
                }
                Err(why) => why,
            },
        };
        let (scope, logs) = baseline::logs_for(&built, SEARCH_LOGS, Self::seed(config));
        let mut without_mechanism = 0;
        for (k, log) in logs.iter().enumerate() {
            match self.fails(config, &built, log) {
                Ok(()) => {
                    self.found
                        .insert(config.clone(), (log.clone(), Found::Searched(k)));
                    return Ran::Fails;
                }
                Err(why) if why.contains("mechanism") => without_mechanism += 1,
                Err(_) => {}
            }
        }
        match scope {
            baseline::Scope::Exhaustive => Ran::Holds(format!(
                "the original order: {first}; every one of the {} admissible schedules holds ({without_mechanism} fail without the mechanism)",
                logs.len()
            )),
            baseline::Scope::Sampled { seed } => Ran::Inconclusive(
                InconclusiveReason::ResourceExhausted,
                format!(
                    "the original order: {first}; none of {} schedules sampled from seed {seed} fails with the mechanism ({without_mechanism} fail without it); the search is not exhaustive",
                    logs.len()
                ),
            ),
        }
    }
}

// ---------------------------------------------------------------------------
// the composition
// ---------------------------------------------------------------------------

/// A failing run's scenario reduction composed with the event reduction.
struct Composed {
    start: Config,
    scenario: ScenarioReduction<Config>,
    /// How the reduced configuration's run was found, and its case.
    found: Found,
    reduced: Case,
    /// bn-2z08o's closure and deletion over atoms of the reduced run.
    events: ProgramReduction,
    /// Configurations decided without a schedule.
    decided_statically: Vec<Config>,
    /// The original run's length.
    original_len: usize,
    /// Binding executions the minimization spent: the scenario passes' schedules (two
    /// each), the reduced run's case (two), and the event reduction's operation
    /// boundaries (one per operation prefix) and replays (at most one each).
    executions: u64,
}

impl Composed {
    fn core_len(&self) -> usize {
        self.events.reduction.core().expect("reduced").events.len()
    }

    fn ratio(&self) -> f64 {
        self.core_len() as f64 / self.original_len as f64
    }
}

fn compose_with(c: &Case, start: Config, pinned: Option<u8>, budget: Budget) -> Composed {
    let mut s = RegisterScenario::new(c, &start);
    s.pinned = pinned;
    let scenario = reduce_scenario(&mut s, start.clone(), budget);
    let config = scenario
        .config()
        .expect("the scenario reduction finished")
        .clone();
    let (log, found) = s.found[&config].clone();
    let reduced = case_built(
        format!("{} reduced", c.label),
        config.built(),
        config.epochs,
        &log,
        Some(c.target),
        reach(config.epochs),
    )
    .expect("the reduced configuration fails");
    let events = minimize_program(&reduced, Deletion::Atoms);
    let executions = s.executions.get() + 2 + events.ops.ops.len() as u64 + events.replays;
    Composed {
        start,
        scenario,
        found,
        reduced,
        events,
        decided_statically: s.decided_statically,
        original_len: c.journal.len(),
        executions,
    }
}

fn compose(c: &Case, start: Config) -> Composed {
    compose_with(c, start, None, SCENARIO_BUDGET)
}

/// M01's two witnesses, as `pr18_impl01_reduction.rs` reduces them, and their start
/// configurations.
fn agreement_start() -> Config {
    Config::start(1, baseline::scenario_replicas())
}

fn acked_start() -> Config {
    Config::start(
        1,
        baseline::one_epoch_sweep_replicas(baseline::SCENARIO)[81],
    )
}

fn agreement_composed() -> &'static Composed {
    static CELL: OnceLock<Composed> = OnceLock::new();
    CELL.get_or_init(|| compose(agreement_witness(), agreement_start()))
}

fn acked_composed() -> &'static Composed {
    static CELL: OnceLock<Composed> = OnceLock::new();
    CELL.get_or_init(|| compose(acked_witness(), acked_start()))
}

/// Independent check of the scenario's minimality claims from its transcript and passes
/// alone: the kept entries rebuild the version count; in the last round every claimed
/// dimension's entries were tried against the final version, every one is a decided
/// non-failure, and there are as many as its pass offered. Returns the dimensions whose
/// claim the transcript licenses.
fn scenario_licenses(r: &ScenarioReduction<Config>) -> Result<Vec<Dimension>, String> {
    let ScenarioReduction::Reduced {
        versions,
        guarantees,
        passes,
        attempts,
        ..
    } = r
    else {
        return Err("not reduced".to_owned());
    };
    if attempts.omitted != 0 {
        return Err(format!("{} attempts omitted", attempts.omitted));
    }
    let kept = attempts
        .entries
        .iter()
        .filter(|a| a.dimension.is_some() && a.verdict == ConfigVerdict::Fails)
        .count();
    if kept + 1 != versions.len() {
        return Err("the kept entries do not rebuild the versions".to_owned());
    }
    let last_round = passes.iter().map(|p| p.round).max().unwrap_or(0);
    let final_version = (versions.len() - 1) as u64;
    let mut out = Vec::new();
    for d in Dimension::ALL {
        let claimed = guarantees.contains(&d.minimality());
        let record = passes
            .iter()
            .find(|p| p.round == last_round && p.dimension == d)
            .ok_or("no last-round pass")?;
        let entries: Vec<&ConfigAttempt> = attempts
            .entries
            .iter()
            .filter(|a| a.dimension == Some(d) && a.round == last_round)
            .collect();
        let licensed = record.end == DimensionEnd::Done
            && entries.len() as u64 == record.offered
            && entries
                .iter()
                .all(|a| a.version == final_version && a.verdict.decided_non_failure());
        if claimed && !licensed {
            return Err(format!("{d:?}: the claim is not licensed"));
        }
        if licensed {
            out.push(d);
        }
    }
    Ok(out)
}

// ---------------------------------------------------------------------------
// rendering
// ---------------------------------------------------------------------------

fn config_attempt_line(a: &ConfigAttempt) -> String {
    let dim = a
        .dimension
        .map_or_else(|| "start".to_owned(), |d| format!("{d:?}"));
    let verdict = match a.verdict {
        ConfigVerdict::Fails => "fails".to_owned(),
        ConfigVerdict::Holds => "holds".to_owned(),
        ConfigVerdict::NotARun => "not-a-run".to_owned(),
        ConfigVerdict::Inconclusive(r) => format!("inconclusive({r:?})"),
        ConfigVerdict::OutOfScope => "out-of-scope (not run)".to_owned(),
        ConfigVerdict::NotSmaller => "not-smaller".to_owned(),
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
        "#{} {dim} round {} v{} [{}] measure {:?}: {verdict}{memo}{reason}",
        a.index, a.round, a.version, a.label, a.measure
    )
}

fn scenario_lines(r: &ScenarioReduction<Config>) -> Vec<String> {
    let (passes, spent) = match r {
        ScenarioReduction::Reduced { passes, spent, .. }
        | ScenarioReduction::Inconclusive { passes, spent, .. } => (passes, spent),
        ScenarioReduction::Refused { why, .. } => return vec![format!("refused: {why:?}")],
    };
    let mut out: Vec<String> = passes
        .iter()
        .map(|p| {
            format!(
                "round {} pass {:?}: measure {:?} -> {:?}; {} offered, {} run, {} kept, {} held, {} not a run, {} out of scope, {} inconclusive, {} memo; {:?}",
                p.round,
                p.dimension,
                p.before,
                p.after,
                p.offered,
                p.runs,
                p.kept,
                p.held,
                p.not_a_run,
                p.out_of_scope,
                p.inconclusive,
                p.memo,
                p.end
            )
        })
        .collect();
    out.push(format!(
        "spent: {} runs, {} work units",
        spent.replays, spent.work
    ));
    out
}

fn composed_section(id: &str, c: &Case, k: &Composed) -> String {
    let mut s = String::new();
    let _ = writeln!(s, "[{id}]");
    let _ = writeln!(
        s,
        "run: M01 {}, target {}; the original run has {} events",
        c.label,
        c.target.name(),
        k.original_len
    );
    let _ = writeln!(s, "start configuration: {}", k.start.render());
    let ScenarioReduction::Reduced {
        config,
        versions,
        guarantees,
        ..
    } = &k.scenario
    else {
        unreachable!("reduced");
    };
    for line in scenario_lines(&k.scenario) {
        let _ = writeln!(s, "  {line}");
    }
    for (v, cfg) in versions.iter().enumerate() {
        let _ = writeln!(s, "  version {v}: {}", cfg.render());
    }
    let _ = writeln!(s, "reduced configuration: {}", config.render());
    let _ = writeln!(
        s,
        "  scenario guarantees {guarantees:?}; licensed by the transcript alone: {:?}; decided without a schedule: {}",
        scenario_licenses(&k.scenario),
        k.decided_statically.len()
    );
    let r = &k.reduced;
    let _ = writeln!(
        s,
        "  its run ({:?}): {} events, digest {}; log {:?}",
        k.found,
        r.journal.len(),
        r.journal.digest().expect("digests"),
        r.log.choices().iter().map(|x| x.0).collect::<Vec<_>>()
    );
    let red = &k.events.reduction;
    let core = red.core().expect("reduced");
    let Reduction::Reduced {
        transcript, spent, ..
    } = red
    else {
        unreachable!("reduced");
    };
    let _ = writeln!(
        s,
        "event reduction of the reduced run (bn-2z08o's closure and deletion over atoms under the program replay): closure {} -> {}, deletion {} -> {}, {} replays, guarantees {:?}",
        transcript[0].before,
        transcript[0].after,
        transcript[1].before,
        transcript[1].after,
        spent.replays,
        core.guarantees
    );
    let st = story(r, &core.events);
    let _ = writeln!(
        s,
        "  core: {} events: {:.1}% of the original run, {:.1}% of the reduced run; mechanism kept: {}; campaign checker on the core's re-execution reports {}: {}; atom minimality from the transcript alone: {:?}",
        core.events.len(),
        100.0 * k.ratio(),
        100.0 * core.events.len() as f64 / r.journal.len() as f64,
        keeps_mechanism(&st, r.target),
        r.target.name(),
        campaign_finds_target(r, &k.events.ops, &core.events),
        transcript_licenses(&k.events.order, core, red.attempts().expect("attempts"))
    );
    let _ = writeln!(s, "  causal story: {}", st.join(" "));
    let cj = continuum_asupersync::causal::restrict(&r.journal, &core.events)
        .expect("restricts")
        .journal;
    let _ = writeln!(s, "  core digest {}", cj.digest().expect("digests"));
    for line in cj.render().lines() {
        let _ = writeln!(s, "    {line}");
    }
    s
}

// ---------------------------------------------------------------------------
// the corpus
// ---------------------------------------------------------------------------

struct CorpusFacts {
    runs: usize,
    by_target: BTreeMap<Target, usize>,
    /// Per run: the original length, the reduced run's length, the core length.
    original: Vec<usize>,
    reduced: Vec<usize>,
    core: Vec<usize>,
    /// Runs whose configuration the passes changed.
    configs_changed: usize,
    /// Reduced configurations, with how many runs reduced to each.
    configs: BTreeMap<String, usize>,
    guarantees: BTreeMap<ScenarioGuarantee, usize>,
    licensed: usize,
    mechanism: usize,
    campaign: usize,
    atom_minimal: usize,
    start_log_is_original: usize,
    static_decisions: usize,
    /// Distinct configurations decided without a schedule, with the target.
    decided: BTreeSet<(Config, Target)>,
    runs_spent: u64,
    /// Binding executions each run's minimization spent.
    executions: Vec<u64>,
    /// Binding executions of M01's whole campaign, and of its scenario plan: what the
    /// verification spent to find these failures.
    campaign_executions: u64,
    /// The bn-2z08o cores of the same runs, without the scenario passes.
    before: Vec<usize>,
}

fn corpus_facts() -> &'static CorpusFacts {
    static CELL: OnceLock<CorpusFacts> = OnceLock::new();
    CELL.get_or_init(|| {
        let mut f = CorpusFacts {
            runs: 0,
            by_target: BTreeMap::new(),
            original: Vec::new(),
            reduced: Vec::new(),
            core: Vec::new(),
            configs_changed: 0,
            configs: BTreeMap::new(),
            guarantees: BTreeMap::new(),
            licensed: 0,
            mechanism: 0,
            campaign: 0,
            atom_minimal: 0,
            start_log_is_original: 0,
            static_decisions: 0,
            decided: BTreeSet::new(),
            runs_spent: 0,
            executions: Vec::new(),
            campaign_executions: campaign_executions(),
            before: Vec::new(),
        };
        for c in corpus() {
            let k = compose(c, agreement_start());
            f.runs += 1;
            *f.by_target.entry(c.target).or_default() += 1;
            let ScenarioReduction::Reduced {
                config,
                versions,
                guarantees,
                spent,
                ..
            } = &k.scenario
            else {
                panic!("{}: the scenario reduction finished", c.label);
            };
            f.runs_spent += spent.replays;
            if versions.len() > 1 {
                f.configs_changed += 1;
            }
            *f.configs
                .entry(format!("{} [{}]", config.render(), c.target.name()))
                .or_default() += 1;
            for g in guarantees {
                *f.guarantees.entry(*g).or_default() += 1;
            }
            if scenario_licenses(&k.scenario).is_ok() {
                f.licensed += 1;
            }
            let core = k.events.reduction.core().expect("reduced");
            if keeps_mechanism(&story(&k.reduced, &core.events), c.target) {
                f.mechanism += 1;
            }
            if campaign_finds_target(&k.reduced, &k.events.ops, &core.events) {
                f.campaign += 1;
            }
            if core.guarantees.contains(&Guarantee::AtomMinimal)
                || core.guarantees.contains(&Guarantee::OneMinimal)
            {
                f.atom_minimal += 1;
            }
            f.static_decisions += k.decided_statically.len();
            for cfg in &k.decided_statically {
                f.decided.insert((cfg.clone(), c.target));
            }
            f.executions.push(k.executions);
            f.original.push(k.original_len);
            f.reduced.push(k.reduced.journal.len());
            f.core.push(core.events.len());
            // The start configuration's own run is the original run.
            let mut s = RegisterScenario::new(c, &k.start);
            if matches!(s.run(&k.start), Ran::Fails) && s.found[&k.start].0 == c.log {
                f.start_log_is_original += 1;
            }
            let p = minimize_program(c, Deletion::Atoms);
            f.before
                .push(p.reduction.core().expect("reduced").events.len());
        }
        f
    })
}

/// Binding executions of M01's campaign: one per log of every plan, as
/// `register_baseline::execute_with` runs them.
fn campaign_executions() -> u64 {
    let base = baseline::baseline();
    let m = mutants::mutant(mutants::Id::M01, &base).expect("M01");
    let mut n = 0;
    for (g, group) in m.campaign.groups.iter().enumerate() {
        for (p, plan) in group.plans.iter().enumerate() {
            let built = register::build_with_shutdown(plan);
            n += baseline::logs_for(&built, group.logs, m.campaign.plan_seed(g, p))
                .1
                .len() as u64;
        }
    }
    n
}

fn ratios(core: &[usize], of: &[usize]) -> Vec<f64> {
    core.iter()
        .zip(of)
        .map(|(a, b)| *a as f64 / *b as f64)
        .collect()
}

fn max(v: &[f64]) -> f64 {
    v.iter().copied().fold(0.0, f64::max)
}

fn min(v: &[f64]) -> f64 {
    v.iter().copied().fold(f64::INFINITY, f64::min)
}

/// research/26's ratified threshold (plan §24.5, `quote-id=causal-minimization-core-ratio`):
/// the replay-preserving causal core is at most 10% of trace length at the corpus median.
const RATIFIED_RATIO: f64 = 0.10;

fn corpus_section() -> String {
    let f = corpus_facts();
    let of_original = ratios(&f.core, &f.original);
    let of_reduced = ratios(&f.core, &f.reduced);
    let before = ratios(&f.before, &f.original);
    let med = median(of_original.clone());
    let mut s = String::new();
    let _ = writeln!(s, "[pr18-impl02-03-corpus]");
    let _ = writeln!(
        s,
        "corpus: every failing run of M01's scenario plan ({} runs), targets {:?}",
        f.runs,
        f.by_target
            .iter()
            .map(|(t, n)| format!("{}={n}", t.name()))
            .collect::<Vec<_>>()
    );
    let _ = writeln!(
        s,
        "the start configuration's guided run is the original log on {} of {} runs",
        f.start_log_is_original, f.runs
    );
    let _ = writeln!(
        s,
        "scenario passes changed the configuration on {} runs; {} candidate evaluations decided without a schedule; {} configuration evaluations charged; guarantees {:?}; minimality licensed by the transcript alone on {}; OwnerMinimal covers idle slots and pruned owners, and the model keeps its three nodes",
        f.configs_changed, f.static_decisions, f.runs_spent, f.guarantees, f.licensed
    );
    for (cfg, n) in &f.configs {
        let _ = writeln!(s, "  reduced to {cfg}: {n} runs");
    }
    let _ = writeln!(
        s,
        "composed cores: {} of {} keep the mechanism; the campaign's checker reports the target on {} re-executed cores; {} are atom-minimal",
        f.mechanism, f.runs, f.campaign, f.atom_minimal
    );
    let _ = writeln!(
        s,
        "core as a share of the original run: median {:.1}%, min {:.1}%, max {:.1}%; of the reduced run it was cut from: median {:.1}%, min {:.1}%, max {:.1}%; bn-2z08o's cores of the same runs without the scenario passes: median {:.1}%, max {:.1}%",
        100.0 * med,
        100.0 * min(&of_original),
        100.0 * max(&of_original),
        100.0 * median(of_reduced.clone()),
        100.0 * min(&of_reduced),
        100.0 * max(&of_reduced),
        100.0 * median(before.clone()),
        100.0 * max(&before)
    );
    let _ = writeln!(
        s,
        "median length: original run {}, reduced run {}, core {}",
        median(f.original.iter().map(|x| *x as f64).collect()),
        median(f.reduced.iter().map(|x| *x as f64).collect()),
        median(f.core.iter().map(|x| *x as f64).collect())
    );
    let med_reduced = median(of_reduced);
    let _ = writeln!(
        s,
        "PR-18 exit, research/26 quote-id=causal-minimization-core-ratio (the replay-preserving causal core <= 10% of trace length at the corpus median): {}; the composed core is cut from the reduced run, so it is measured against both traces: {:.1}% of the original run and {:.1}% of the reduced run at the median; the corpus is M01's scenario plan, durability/cancellation, the only research/26 experiment class this campaign produces",
        if med.max(med_reduced) <= RATIFIED_RATIO {
            "MET"
        } else {
            "NOT MET"
        },
        100.0 * med,
        100.0 * med_reduced
    );
    let exec: Vec<f64> = f.executions.iter().map(|x| *x as f64).collect();
    let total: u64 = f.executions.iter().sum();
    let _ = writeln!(
        s,
        "research/26 kill signal (minimization cost dominates verification), in binding executions: one failure's minimization costs a median of {} and at most {}; M01's campaign ran {} executions to verify, {} of them on the scenario plan; minimizing every one of the {} failing runs costs {} ({:.1}x the campaign)",
        median(exec.clone()),
        max(&exec),
        f.campaign_executions,
        baseline::SCENARIO_LOGS,
        f.runs,
        total,
        total as f64 / f.campaign_executions as f64
    );
    s
}

// ---------------------------------------------------------------------------
// the other sections
// ---------------------------------------------------------------------------

fn scope_section() -> String {
    let mut s = String::new();
    let _ = writeln!(s, "[pr18-impl02-04-scope]");
    let c = agreement_witness();
    let k = agreement_composed();
    let out_of_scope: Vec<&ConfigAttempt> = k
        .scenario
        .attempts()
        .expect("attempts")
        .entries
        .iter()
        .filter(|a| a.verdict == ConfigVerdict::OutOfScope)
        .collect();
    let _ = writeln!(
        s,
        "M01 {}: {} candidates out of scope, never run: {}",
        c.label,
        out_of_scope.len(),
        out_of_scope
            .iter()
            .map(|a| format!("[{}] {}", a.label, a.reason))
            .collect::<Vec<_>>()
            .join("; ")
    );
    let pinned = compose_with(acked_witness(), acked_start(), Some(0), SCENARIO_BUDGET);
    let ScenarioReduction::Reduced {
        passes, guarantees, ..
    } = &pinned.scenario
    else {
        unreachable!("reduced");
    };
    let refused: Vec<String> = passes
        .iter()
        .filter_map(|p| match &p.end {
            DimensionEnd::ScopeRefused(why) => {
                Some(format!("round {} {:?}: {why}", p.round, p.dimension))
            }
            _ => None,
        })
        .collect();
    let _ = writeln!(
        s,
        "M01 {} with a question that names v0: the value pass is refused in every round ({}); guarantees {:?}",
        acked_witness().label,
        refused.join("; "),
        guarantees
    );
    s
}

fn determinism_section() -> String {
    let a = compose(agreement_witness(), agreement_start());
    let b = agreement_composed();
    assert_eq!(a.scenario, b.scenario, "identical scenario reductions");
    assert_eq!(
        a.events.reduction, b.events.reduction,
        "identical event reductions"
    );
    let c = compose(acked_witness(), acked_start());
    let d = acked_composed();
    assert_eq!(c.scenario, d.scenario);
    assert_eq!(c.events.reduction, d.events.reduction);
    let mut s = String::new();
    let _ = writeln!(s, "[pr18-impl02-05-determinism]");
    let _ = writeln!(
        s,
        "both witnesses composed twice: identical scenario reductions, transcripts, reduced runs ({} and {} events) and event reductions",
        b.reduced.journal.len(),
        d.reduced.journal.len()
    );
    s
}

/// Every distinct schedule-free decision of the two witnesses' reductions and of the
/// corpus's, run against that configuration's schedules (all of them when there are at
/// most [`SEARCH_LOGS`], otherwise a seeded sample): none shows the target. Returns the
/// configurations checked, the exhaustive ones, and the schedules run.
fn static_facts() -> (usize, usize, usize) {
    let mut decided: BTreeSet<(Config, Target)> = corpus_facts().decided.clone();
    for (c, k) in [
        (agreement_witness(), agreement_composed()),
        (acked_witness(), acked_composed()),
    ] {
        for cfg in &k.decided_statically {
            decided.insert((cfg.clone(), c.target));
        }
    }
    let (mut exhaustive, mut schedules) = (0, 0);
    for (cfg, target) in &decided {
        let s = RegisterScenario::for_target(*target);
        let built = cfg.built();
        let (scope, logs) = baseline::logs_for(&built, SEARCH_LOGS, RegisterScenario::seed(cfg));
        if scope == baseline::Scope::Exhaustive {
            exhaustive += 1;
        }
        for log in &logs {
            schedules += 1;
            assert!(
                s.fails(cfg, &built, log).is_err(),
                "{}: a schedule-free decision holds on every schedule",
                cfg.render()
            );
            let report = baseline::run_one(&built, cfg.epochs, log, reach(cfg.epochs));
            assert!(
                !report.findings.iter().any(|f| matches!(
                    (f, target),
                    (baseline::Finding::Agreement(_), Target::Agreement)
                        | (
                            baseline::Finding::AckedNotDurable(_),
                            Target::AckedNotDurable
                        )
                )),
                "{}: the campaign's checker agrees",
                cfg.render()
            );
        }
    }
    (decided.len(), exhaustive, schedules)
}

fn static_section() -> String {
    let (configs, exhaustive, schedules) = static_facts();
    let mut s = String::new();
    let _ = writeln!(s, "[pr18-impl02-06-static-decisions]");
    let _ = writeln!(
        s,
        "the {configs} distinct configurations the witnesses' and the corpus's reductions decided without a schedule, each run on its schedules ({exhaustive} on every admissible schedule, the others on {SEARCH_LOGS} sampled; {schedules} runs): the campaign's checker reports the target on none"
    );
    s
}

fn evidence() -> String {
    let mut s = String::new();
    s.push_str(
        "# PR 18 (bn-25z9o): owner, fault and value-domain reduction of M01's failing runs, composed with bn-2z08o's closure and deletion.\n\
         # Regenerate: PR18_IMPL02_BLESS=1 cargo test -p continuum-asupersync --test pr18_impl02_scenario_reduction\n\
         # A candidate is a smaller configuration, built as M01's plan and run through the binding; the schedule is the original run's order carried over, then a seeded search. Transcripts: pr18_impl02_transcripts.txt.\n\n",
    );
    s.push_str(&composed_section(
        "pr18-impl02-01-m01-agreement-scenario",
        agreement_witness(),
        agreement_composed(),
    ));
    s.push('\n');
    s.push_str(&composed_section(
        "pr18-impl02-02-m01-acked-not-durable-scenario",
        acked_witness(),
        acked_composed(),
    ));
    s.push('\n');
    s.push_str(&corpus_section());
    s.push('\n');
    s.push_str(&scope_section());
    s.push('\n');
    s.push_str(&determinism_section());
    s.push('\n');
    s.push_str(&static_section());
    s
}

fn transcripts() -> String {
    let mut s = String::new();
    s.push_str(
        "# PR 18 (bn-25z9o): RFC 0028 minimizer transcripts of M01's witnesses: the scenario reduction over owners, faults and values, then the event reduction of the reduced run.\n\
         # Regenerate: PR18_IMPL02_BLESS=1 cargo test -p continuum-asupersync --test pr18_impl02_scenario_reduction\n\
         # Scenario lines: #attempt pass round v<version tried against> [what the candidate changed] measure [owners, faults, values]: verdict [memo-of #attempt] [-- the scenario's reason]\n",
    );
    for (id, c, k) in [
        (
            "pr18-impl02-07-m01-agreement-transcript",
            agreement_witness(),
            agreement_composed(),
        ),
        (
            "pr18-impl02-08-m01-acked-not-durable-transcript",
            acked_witness(),
            acked_composed(),
        ),
    ] {
        let a = k.scenario.attempts().expect("attempts");
        let _ = writeln!(s, "\n[{id}]");
        let _ = writeln!(s, "run: M01 {}, target {}", c.label, c.target.name());
        let _ = writeln!(
            s,
            "scenario: recorded {}, omitted {}",
            a.entries.len(),
            a.omitted
        );
        for e in &a.entries {
            let _ = writeln!(s, "  {}", config_attempt_line(e));
        }
        let e = k.events.reduction.attempts().expect("attempts");
        let _ = writeln!(
            s,
            "events of the reduced run: recorded {}, omitted {}, result version {}",
            e.entries.len(),
            e.omitted,
            e.version
        );
        for x in &e.entries {
            let _ = writeln!(s, "  {}", attempt_line(x));
        }
    }
    s
}

// ---------------------------------------------------------------------------
// tests
// ---------------------------------------------------------------------------

/// The start configurations build the witnesses' programs, and their guided runs are the
/// witnesses' own logs: the reduction starts from the failing run itself.
#[test]
fn the_start_configuration_reruns_the_original_run() {
    for (c, start) in [
        (agreement_witness(), agreement_start()),
        (acked_witness(), acked_start()),
    ] {
        assert_eq!(start.built().programs, c.built.programs, "{}", c.label);
        let mut s = RegisterScenario::new(c, &start);
        assert_eq!(s.run(&start), Ran::Fails, "{}", c.label);
        let (log, found) = &s.found[&start];
        assert_eq!(*found, Found::Guided);
        assert_eq!(*log, c.log, "{}: the guided log is the original", c.label);
    }
}

/// A pruned build of a plan whose every incarnation acts on every epoch is the plain
/// build, operation for operation.
#[test]
fn pruning_changes_nothing_when_every_owner_is_used() {
    let plan = agreement_start().plan();
    let a = register::build_pruned(&plan);
    let b = register::build_with_shutdown(&plan);
    assert_eq!(a.programs, b.programs);
    assert_eq!(a.roles, b.roles);
}

/// M01's Agreement witness is already minimal along all three dimensions: every smaller
/// configuration is decided, most without a schedule, and the core is bn-2z08o's.
#[test]
fn the_agreement_scenario_is_already_owner_fault_and_value_minimal() {
    let k = agreement_composed();
    let ScenarioReduction::Reduced {
        config,
        versions,
        guarantees,
        ..
    } = &k.scenario
    else {
        panic!("reduced: {:?}", k.scenario);
    };
    assert_eq!(versions.len(), 1, "nothing kept");
    assert_eq!(*config, k.start);
    for g in [
        ScenarioGuarantee::Reproduces,
        ScenarioGuarantee::OwnerMinimal,
        ScenarioGuarantee::FaultMinimal,
        ScenarioGuarantee::ValueMinimal,
    ] {
        assert!(guarantees.contains(&g), "{g:?}");
    }
    assert_eq!(scenario_licenses(&k.scenario), Ok(Dimension::ALL.to_vec()));
    let before = program::agreement_program().reduction.core().expect("core");
    assert_eq!(
        k.events.reduction.core().expect("core").events,
        before.events
    );
    let core = k.events.reduction.core().expect("core");
    assert!(keeps_mechanism(
        &story(&k.reduced, &core.events),
        Target::Agreement
    ));
}

/// M01's AckedNotDurable witness loses owners, faults or values it does not need, and the
/// composed core keeps the mechanism and is smaller than bn-2z08o's.
#[test]
fn the_acked_not_durable_scenario_reduces_and_keeps_the_mechanism() {
    let k = acked_composed();
    let ScenarioReduction::Reduced {
        versions,
        guarantees,
        ..
    } = &k.scenario
    else {
        panic!("reduced: {:?}", k.scenario);
    };
    assert!(versions.len() > 1, "a smaller configuration was kept");
    assert!(guarantees.contains(&ScenarioGuarantee::Reproduces));
    assert!(scenario_licenses(&k.scenario).is_ok());
    let core = k.events.reduction.core().expect("core");
    assert!(keeps_mechanism(
        &story(&k.reduced, &core.events),
        Target::AckedNotDurable
    ));
    assert!(campaign_finds_target(
        &k.reduced,
        &k.events.ops,
        &core.events
    ));
    let before = program::acked_program().reduction.core().expect("core");
    assert!(core.events.len() < before.events.len());
    // Every kept version reruns to the failure, on a fresh scenario.
    let mut fresh = RegisterScenario::new(acked_witness(), &k.start);
    for v in versions {
        assert_eq!(fresh.run(v), Ran::Fails, "{}", v.render());
    }
}

/// INV-013: a candidate outside the declared scope is recorded and never run, and a
/// question that names a value refuses the value pass and withholds `ValueMinimal`.
#[test]
fn every_pass_declares_its_scope_and_refuses_to_change_the_property() {
    let k = agreement_composed();
    let entries = &k.scenario.attempts().expect("attempts").entries;
    let out: Vec<&ConfigAttempt> = entries
        .iter()
        .filter(|a| a.verdict == ConfigVerdict::OutOfScope)
        .collect();
    assert!(!out.is_empty(), "the one-value candidates are out of scope");
    assert!(out.iter().all(|a| a.reason.contains("INV-013")));
    assert!(out.iter().all(|a| a.memo_of.is_none()));
    let pinned = compose_with(acked_witness(), acked_start(), Some(0), SCENARIO_BUDGET);
    let ScenarioReduction::Reduced {
        passes, guarantees, ..
    } = &pinned.scenario
    else {
        panic!("reduced");
    };
    let value_passes: Vec<_> = passes
        .iter()
        .filter(|p| p.dimension == Dimension::Value)
        .collect();
    assert!(!value_passes.is_empty());
    assert!(value_passes.iter().all(|p| {
        matches!(p.end, DimensionEnd::ScopeRefused(_)) && p.offered == 0 && p.runs == 0
    }));
    assert!(!guarantees.contains(&ScenarioGuarantee::ValueMinimal));
    // The other passes keep the named value too: a candidate that no longer uses it is
    // out of scope, whatever pass offers it.
    let mut named = RegisterScenario::for_target(Target::AckedNotDurable);
    named.pinned = Some(1);
    let mut without_v1 = agreement_start();
    without_v1.replicas[0].slots[0].1 = Fate::Clean;
    without_v1.replicas[2].slots[0].1 = Fate::Idle;
    without_v1.pruned = true;
    for d in [Dimension::Owner, Dimension::Fault] {
        let why = named.admits(d, &without_v1).expect_err("out of scope");
        assert!(why.contains("names v1") && why.contains("INV-013"), "{why}");
    }
    assert!(named.admits(Dimension::Owner, &agreement_start()).is_ok());
    assert!(
        pinned
            .scenario
            .attempts()
            .expect("attempts")
            .entries
            .iter()
            .all(|a| a.dimension != Some(Dimension::Value))
    );
}

/// The schedule-free decisions agree with the campaign's checker on sampled schedules.
#[test]
fn schedule_free_decisions_agree_with_schedules() {
    let (configs, _, schedules) = static_facts();
    assert!(configs > 0 && schedules > 0);
}

/// The corpus: every composed core keeps the mechanism, is re-executed to its target by
/// the campaign's checker, and is atom-minimal; every start reruns the original log.
#[test]
fn composed_cores_keep_the_mechanism_over_the_corpus() {
    let f = corpus_facts();
    assert_eq!(f.runs, baseline::SCENARIO_LOGS);
    assert_eq!(f.start_log_is_original, f.runs);
    assert_eq!(f.mechanism, f.runs);
    assert_eq!(f.campaign, f.runs);
    assert_eq!(f.atom_minimal, f.runs);
    assert_eq!(f.licensed, f.runs);
    for (core, before) in f.core.iter().zip(&f.before) {
        assert!(core <= before, "the scenario passes never grow the core");
    }
}

/// The PR-18 exit measurement, stated either way and pinned: research/26's ratified ratio
/// is not met on this corpus.
#[test]
fn the_ratified_core_ratio_is_measured_not_assumed() {
    let f = corpus_facts();
    let med = median(ratios(&f.core, &f.original));
    assert!(
        med > RATIFIED_RATIO,
        "the corpus median is {med}: the PR-18 exit criterion is met; update the evidence, START_HERE and bn-2nh6"
    );
}

#[test]
fn identical_inputs_give_identical_reductions() {
    let _ = determinism_section();
}

/// `config` with v0 and v1 swapped everywhere: the value symmetry.
fn swap_values(config: &Config) -> Config {
    let swap = |v: u8| 1 - v;
    let mut out = config.clone();
    for r in &mut out.replicas {
        r.values = r.values.map(swap);
        for s in &mut r.slots {
            s.1 = match s.1 {
                Fate::CrashReserved { retry } => Fate::CrashReserved { retry: swap(retry) },
                Fate::CrashSubmitted { retry } => Fate::CrashSubmitted { retry: swap(retry) },
                other => other,
            };
        }
    }
    out
}

/// The metamorphic relation, symmetry renaming: swap v0 and v1 in the witness's
/// configuration and carry its run over (the same order, each coordinator renamed). The
/// renamed run fails the same target, and its scenario reduction is the original's
/// renamed: the same versions up to the swap, the same verdicts and measures, the same
/// guarantees, and a composed core of the same size.
#[test]
fn symmetry_renaming_of_values_renames_the_scenario_reduction() {
    for (c, start, k) in [
        (agreement_witness(), agreement_start(), agreement_composed()),
        (acked_witness(), acked_start(), acked_composed()),
    ] {
        let swapped = swap_values(&start);
        let rank: BTreeMap<OpKey, u64> = original_rank(c, &start.plan())
            .into_iter()
            .map(|(key, at)| match key {
                OpKey::Coordinator {
                    epoch,
                    value,
                    index,
                } => (
                    OpKey::Coordinator {
                        epoch,
                        value: 1 - value,
                        index,
                    },
                    at,
                ),
                other => (other, at),
            })
            .collect();
        let built = swapped.built();
        let keys = op_keys(&swapped.plan(), &built);
        let log = register::guided_log(&built, |a, i| {
            rank.get(&keys[a][i]).copied().unwrap_or(u64::MAX / 2)
        })
        .expect("admissible");
        let renamed = case_built(
            format!("{} with values swapped", c.label),
            built,
            swapped.epochs,
            &log,
            Some(c.target),
            reach(swapped.epochs),
        )
        .expect("the renamed run fails the same target");
        assert_eq!(renamed.journal.len(), c.journal.len(), "{}", c.label);
        let r = compose(&renamed, swapped);
        let (
            ScenarioReduction::Reduced {
                versions: va,
                guarantees: ga,
                attempts: aa,
                ..
            },
            ScenarioReduction::Reduced {
                versions: vb,
                guarantees: gb,
                attempts: ab,
                ..
            },
        ) = (&k.scenario, &r.scenario)
        else {
            panic!("both reduced");
        };
        assert_eq!(
            va.iter().map(swap_values).collect::<Vec<_>>(),
            *vb,
            "{}: the renamed versions",
            c.label
        );
        assert_eq!(ga, gb);
        let view = |a: &ConfigAttempt| (a.dimension, a.round, a.version, a.measure, a.verdict);
        assert_eq!(
            aa.entries.iter().map(view).collect::<Vec<_>>(),
            ab.entries.iter().map(view).collect::<Vec<_>>(),
            "{}: the same attempts",
            c.label
        );
        assert_eq!(r.core_len(), k.core_len(), "{}", c.label);
    }
}

/// A budget that runs out gives the typed inconclusive with the last failing
/// configuration, never a reduced configuration or a minimality claim.
#[test]
fn an_exhausted_budget_is_inconclusive_with_the_last_failing_configuration() {
    let c = acked_witness();
    let start = acked_start();
    let full = acked_composed();
    let runs = match &full.scenario {
        ScenarioReduction::Reduced { spent, .. } => spent.replays,
        _ => unreachable!("reduced"),
    };
    for replays in [0, 1, 2, runs - 1] {
        let mut s = RegisterScenario::new(c, &start);
        let r = reduce_scenario(&mut s, start.clone(), Budget::new(replays, 1 << 34));
        let ScenarioReduction::Inconclusive {
            reason,
            best,
            versions,
            ..
        } = &r
        else {
            panic!("{replays} runs: inconclusive, got {r:?}");
        };
        assert_eq!(*reason, InconclusiveReason::ResourceExhausted);
        if replays == 0 {
            assert!(best.is_none());
        } else {
            let best = best.as_ref().expect("the start failed");
            assert_eq!(versions.last(), Some(best));
            assert_eq!(s.run(best), Ran::Fails);
        }
    }
    let mut s = RegisterScenario::new(c, &start);
    let r = reduce_scenario(&mut s, start.clone(), Budget::new(4_096, 1_000));
    assert!(matches!(
        r,
        ScenarioReduction::Inconclusive {
            reason: InconclusiveReason::ResourceExhausted,
            best: None,
            ..
        }
    ));
}

/// A configuration that does not fail is refused, not reduced.
#[test]
fn a_configuration_that_holds_is_refused() {
    let c = acked_witness();
    let mut start = acked_start();
    for r in &mut start.replicas {
        r.slots[0].1 = Fate::Idle;
    }
    let mut s = RegisterScenario::new(c, &acked_start());
    let r = reduce_scenario(&mut s, start, SCENARIO_BUDGET);
    assert!(matches!(
        r,
        ScenarioReduction::Refused {
            why: continuum_debugger::scenario::ScenarioRefusal::InputHolds(_),
            ..
        }
    ));
}

fn check_golden(path: &str, got: &str) -> Vec<String> {
    if std::env::var_os("PR18_IMPL02_BLESS").is_some() {
        std::fs::write(path, got).expect("writes the golden");
        panic!("the golden was rewritten: review the diff and rerun without PR18_IMPL02_BLESS");
    }
    let want = std::fs::read_to_string(path).expect("the golden exists");
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
        "{path} drifted at {first}; regenerate with PR18_IMPL02_BLESS=1 and review"
    );
    got.lines()
        .filter_map(|l| l.strip_prefix('[').and_then(|l| l.strip_suffix(']')))
        .map(str::to_owned)
        .collect()
}

#[test]
fn the_evidence_matches_its_golden() {
    let ids = check_golden(
        concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/golden/pr18_impl02_scenario_reduction.evidence.txt"
        ),
        &evidence(),
    );
    assert_eq!(
        ids,
        [
            "pr18-impl02-01-m01-agreement-scenario",
            "pr18-impl02-02-m01-acked-not-durable-scenario",
            "pr18-impl02-03-corpus",
            "pr18-impl02-04-scope",
            "pr18-impl02-05-determinism",
            "pr18-impl02-06-static-decisions",
        ]
    );
}

#[test]
fn the_transcripts_match_their_golden() {
    let ids = check_golden(
        concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/golden/pr18_impl02_transcripts.txt"
        ),
        &transcripts(),
    );
    assert_eq!(
        ids,
        [
            "pr18-impl02-07-m01-agreement-transcript",
            "pr18-impl02-08-m01-acked-not-durable-transcript",
        ]
    );
}
