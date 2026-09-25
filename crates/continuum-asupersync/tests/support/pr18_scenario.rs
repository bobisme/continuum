//! PR 18's register scenario (bn-25z9o, bn-5kmuf): the configuration space around one of
//! M01's failing runs, its owner, fault and value candidates, the scenario that runs them
//! through the binding, its mechanism check, and the composition with the event
//! reduction. Moved out of `pr18_impl02_scenario_reduction.rs` so that
//! `pr18_impl03_mechanism.rs` checks the same reductions. Include it with
//! `#[path = "support/pr18_scenario.rs"] mod scenario;` next to the `model`, `register`,
//! `baseline`, `mutants` and `program` modules, under those names.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;

use continuum_asupersync::binding::SubstrateOp;
use continuum_asupersync::choice::ChoiceLog;
use continuum_debugger::mechanism::{
    self, Allowance, Mechanism, Story, StoryReplay, Undecided, Validator,
};
use continuum_debugger::reduce::{Budget, Deletion, Spent};
use continuum_debugger::scenario::{
    Candidate, ConfigAttempt, ConfigVerdict, Dimension, DimensionEnd, Preserved, Ran, Scenario,
    ScenarioReduction, reduce_scenario,
};
use continuum_value::assurance::InconclusiveReason;

use crate::baseline::{self, Fate, Replica};
use crate::mutants;
use crate::program::{
    Case, NODES, ProgramReduction, Target, acked_witness, agreement_witness, case_built,
    derive_mechanism, minimize_program_with, register_story,
};
use crate::register::{self, Act, Built, Plan, Raw, Role};

/// The scenario reduction's budget: runs, and work units.
pub const SCENARIO_BUDGET: Budget = Budget::new(512, 1 << 34);

/// Schedules the search runs when the guided run does not fail: every admissible one when
/// there are at most this many, otherwise this many sampled.
pub const SEARCH_LOGS: usize = 24;

/// Predicted work per act of a configuration's plan, per schedule run: the binding's
/// events, the lift, the model and the projection, each linear in the run.
pub const WORK_PER_ACT: u64 = 1 << 12;

// ---------------------------------------------------------------------------
// the configuration
// ---------------------------------------------------------------------------

/// A register configuration: the plan's epochs and replicas, and whether the build spawns
/// only the owners the scripts use.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Config {
    pub epochs: u8,
    pub replicas: [Replica; 3],
    pub pruned: bool,
}

impl Config {
    pub fn start(epochs: u8, replicas: [Replica; 3]) -> Self {
        Self {
            epochs,
            replicas,
            pruned: false,
        }
    }

    /// The correct plan, before the defect.
    pub fn correct_plan(&self) -> Plan {
        baseline::plan_of("scenario-reduction", self.epochs, self.replicas)
    }

    /// M01's plan: the correct plan with its defect.
    pub fn plan(&self) -> Plan {
        mutants::ack_before_sync(&self.correct_plan())
    }

    pub fn built(&self) -> Built {
        let plan = self.plan();
        if self.pruned {
            register::build_pruned(&plan)
        } else {
            register::build_with_shutdown(&plan)
        }
    }

    /// The written slots, `(replica, slot index, epoch, fate)`.
    pub fn slots(&self) -> impl Iterator<Item = (usize, usize, u8, Fate)> + '_ {
        self.replicas.iter().enumerate().flat_map(move |(n, r)| {
            r.slots[..usize::from(self.epochs)]
                .iter()
                .enumerate()
                .filter(|(_, (_, f))| *f != Fate::Idle)
                .map(move |(k, &(e, f))| (n, k, e, f))
        })
    }

    /// The values the written slots use: each slot's value and each re-proposed value.
    pub fn values(&self) -> BTreeSet<u8> {
        let mut out = BTreeSet::new();
        for (n, _, e, f) in self.slots() {
            out.insert(self.replicas[n].values[usize::from(e)]);
            if let Fate::CrashReserved { retry } | Fate::CrashSubmitted { retry } = f {
                out.insert(retry);
            }
        }
        out
    }

    pub fn acts(&self) -> u64 {
        self.plan()
            .replicas
            .iter()
            .map(|s| s.len() as u64)
            .sum::<u64>()
    }

    pub fn render(&self) -> String {
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
pub const fn depth(f: Fate) -> u64 {
    match f {
        Fate::AbortRetry | Fate::CrashReserved { .. } => 1,
        Fate::CrashSubmitted { .. } => 2,
        Fate::CrashSynced => 3,
        Fate::CrashReplied => 4,
        Fate::Idle | Fate::Clean => 0,
    }
}

pub const fn faults(f: Fate) -> bool {
    depth(f) > 0
}

// ---------------------------------------------------------------------------
// the original run's order, carried to a reduced program
// ---------------------------------------------------------------------------

/// An operation's identity across configurations: its replica, act, epoch and how many
/// times that act on that epoch came before it in the replica's script; or its
/// coordinator and index; or its place in the setup or the shutdown.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum OpKey {
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

pub const fn act_code(a: Act) -> (u8, u8) {
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
pub fn op_keys(plan: &Plan, built: &Built) -> Vec<Vec<OpKey>> {
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
pub fn original_rank(c: &Case, plan: &Plan) -> BTreeMap<OpKey, u64> {
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
pub fn reach(epochs: u8) -> &'static BTreeSet<Raw> {
    crate::program::reachable(epochs)
}

/// How a configuration's run was found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Found {
    /// The original order, carried over.
    Guided,
    /// The `k`th schedule of the search.
    Searched(usize),
}

/// How one schedule of a configuration was judged.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Judged {
    /// The target fails through the original failure's mechanism.
    Fails,
    /// The target fails, through another mechanism.
    WithoutMechanism,
    /// The target does not fail.
    Holds,
    /// The mechanism could not be checked (INV-008).
    Undecided(String),
}

/// The register's configuration space around one failing run.
pub struct RegisterScenario {
    pub target: Target,
    /// A value the question names, if any: the value pass is then refused (INV-013).
    pub pinned: Option<u8>,
    pub rank: BTreeMap<OpKey, u64>,
    /// Each configuration that failed: its failing schedule and how it was found.
    pub found: BTreeMap<Config, (ChoiceLog, Found)>,
    /// Each configuration decided without a schedule.
    pub decided_statically: Vec<Config>,
    /// Binding executions: two per schedule judged (the campaign's checker, then the
    /// journal the mechanism is read from).
    pub executions: std::cell::Cell<u64>,
    /// The original failure's mechanism (bn-5kmuf), derived from its run.
    pub mechanism: Result<Mechanism<String>, Undecided>,
    /// Each configuration's names for the original's values and epochs, recorded when
    /// the candidate that reached it is generated: what the mechanism is renamed by.
    pub names: std::cell::RefCell<BTreeMap<Config, Names>>,
    /// Binding executions of the mechanism checks: two per check (bn-5kmuf), apart from
    /// `executions`.
    pub validations: std::cell::Cell<u64>,
    /// Configurations whose proposed run fails without the mechanism after every
    /// admissible schedule was tried.
    pub exhausted: BTreeSet<Config>,
}

/// A configuration's names for the original configuration's values and epochs: value
/// `v` of the original is `values[v]` here; epoch `e` is `epochs[e]`, or gone.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Names {
    pub values: [u8; 2],
    pub epochs: [Option<u8>; 2],
}

impl Names {
    const IDENTITY: Self = Self {
        values: [0, 1],
        epochs: [Some(0), Some(1)],
    };

    /// `label` with each `value=` and `epoch=` field renamed. An epoch this configuration
    /// dropped is renamed `epoch=dropped`, a name no step of any run carries: a mechanism
    /// step on it has no position and is rejected. It is never left under its old name,
    /// which after an epoch drop names the renamed surviving epoch's operations.
    pub fn rename(self, label: &str) -> String {
        let Some((kind, rest)) = label.split_once('(') else {
            return label.to_owned();
        };
        let open = !rest.ends_with(')');
        let fields: Vec<String> = rest
            .trim_end_matches(')')
            .split(',')
            .map(|f| {
                if let Some(v) = f.strip_prefix("value=") {
                    if let Some(i) = register::VALUES.iter().position(|x| *x == v) {
                        return format!("value={}", register::VALUES[usize::from(self.values[i])]);
                    }
                } else if let Some(e) = f.strip_prefix("epoch=") {
                    match e.parse::<usize>().ok().and_then(|e| self.epochs.get(e)) {
                        Some(Some(to)) => return format!("epoch={to}"),
                        Some(None) => return "epoch=dropped".to_owned(),
                        None => {}
                    }
                }
                f.to_owned()
            })
            .collect();
        format!("{kind}({}{}", fields.join(","), if open { "" } else { ")" })
    }
}

impl RegisterScenario {
    pub fn new(c: &Case, start: &Config) -> Self {
        let mut s = Self::for_target(c.target);
        s.rank = original_rank(c, &start.plan());
        s.mechanism = derive_mechanism(c);
        s
    }

    pub fn names_of(&self, config: &Config) -> Names {
        self.names
            .borrow()
            .get(config)
            .copied()
            .unwrap_or(Names::IDENTITY)
    }

    /// A scenario with no original run to follow: every operation ranks alike, so the
    /// guided walk is the lowest admissible actor first.
    pub fn for_target(target: Target) -> Self {
        Self {
            target,
            pinned: None,
            rank: BTreeMap::new(),
            found: BTreeMap::new(),
            decided_statically: Vec::new(),
            executions: std::cell::Cell::new(0),
            mechanism: Err(Undecided {
                reason: InconclusiveReason::Unsupported,
                detail: "no original failure to derive the mechanism from".to_owned(),
            }),
            names: std::cell::RefCell::new(BTreeMap::new()),
            validations: std::cell::Cell::new(0),
            exhausted: BTreeSet::new(),
        }
    }

    pub fn measure_of(c: &Config) -> [u64; 3] {
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
    pub const fn values_needed(&self) -> usize {
        match self.target {
            Target::Agreement => 2,
            Target::AckedNotDurable => 1,
        }
    }

    /// The schedule-free decision: whether the built program has the acknowledgements the
    /// target needs. `None` when it may; a reason when no run can fail.
    pub fn statically_holds(&self, built: &Built) -> Option<String> {
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
    pub fn fails(&self, config: &Config, built: &Built, log: &ChoiceLog) -> Judged {
        self.executions.set(self.executions.get() + 2);
        let Some(c) = case_built(
            String::new(),
            built.clone(),
            config.epochs,
            log,
            Some(self.target),
            reach(config.epochs),
        ) else {
            return Judged::Holds;
        };
        self.shows_mechanism(config, &c)
    }

    /// Whether the run `c` of `config` shows the original failure's mechanism, renamed to
    /// `config`'s names: the validator's own definition ([`mechanism::embed`], bn-5kmuf),
    /// which replaces bn-25z9o's "any chain" predicate as the scenario's acceptance.
    ///
    /// Its embedding search runs on a fixed allowance of 2^32 work units per schedule,
    /// outside the engine's budget: the scenario's own search, which the engine's check
    /// of the proposed run repeats on the engine's allowance.
    pub fn shows_mechanism(&self, config: &Config, c: &Case) -> Judged {
        let names = self.names_of(config);
        let m = match &self.mechanism {
            Ok(m) => m.renamed(|l| names.rename(l)),
            Err(u) => return Judged::Undecided(format!("no mechanism to check: {}", u.detail)),
        };
        let story = match register_story(&c.journal, &c.built.roles, self.target)
            .and_then(|(labels, _, preds)| Story::new(labels, preds))
        {
            Ok(story) => story,
            Err(why) => return Judged::Undecided(why),
        };
        let mut allowance = Allowance {
            replays: 0,
            work: 1 << 32,
        };
        match mechanism::embed(&m, &story, &mut allowance, &mut Spent::default()) {
            Ok(Ok(_)) => Judged::Fails,
            Ok(Err(_)) => Judged::WithoutMechanism,
            Err(_) => Judged::Undecided("the mechanism search ran out".to_owned()),
        }
    }

    pub fn seed(config: &Config) -> u64 {
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
        let out = self.candidates_of(config, dimension);
        // Each candidate's names, from this configuration's: a value renaming or an
        // epoch drop renames them; every other change keeps them. The first generation
        // that reaches a configuration names it.
        let names = self.names_of(config);
        let mut book = self.names.borrow_mut();
        for c in &out {
            let mut n = names;
            if c.config.epochs < config.epochs {
                let keep = u8::from(c.label.contains("epoch 1 renamed 0"));
                n.epochs = names
                    .epochs
                    .map(|e| e.and_then(|e| (e == keep).then_some(0)));
            } else if c.label.ends_with("one value fewer") {
                let (w, u) = rename_of(&c.label);
                n.values = names.values.map(|x| if x == w { u } else { x });
            }
            book.entry(c.config.clone()).or_insert(n);
        }
        drop(book);
        out
    }

    fn candidates_cost(&self, config: &Config) -> u64 {
        // At most 3 * 2 * 3 candidates per dimension, each built twice (its measure here
        // and the engine's), each build linear in the acts.
        64 * WORK_PER_ACT * (8 + config.acts())
    }

    fn admits(&self, dimension: Dimension, config: &Config) -> Result<(), String> {
        self.admits_scope(dimension, config)
    }

    fn run_cost(&self, config: &Config) -> u64 {
        (1 + SEARCH_LOGS as u64) * WORK_PER_ACT * (8 + config.acts())
    }

    fn run(&mut self, config: &Config) -> Ran {
        self.run_config(config)
    }

    fn search_exhausted(&self, config: &Config) -> bool {
        self.exhausted.contains(config)
    }
}

/// The value renaming a value candidate's label names: `vW renamed vU`.
pub fn rename_of(label: &str) -> (u8, u8) {
    let (w, rest) = label.split_once(" renamed ").expect("a renaming label");
    let u = rest.split_once(':').map_or(rest, |(u, _)| u);
    let at = |name: &str| {
        u8::try_from(
            register::VALUES
                .iter()
                .position(|x| *x == name)
                .expect("a value name"),
        )
        .expect("few values")
    };
    (at(w), at(u))
}

/// The register scenario's mechanism check (bn-5kmuf): the configuration's failing run,
/// the one the scenario found and the reduction keeps, is run again through the binding
/// and the campaign's checker, and its story is what the original failure's mechanism,
/// renamed to the configuration's names, is embedded in.
impl Validator<Config> for RegisterScenario {
    type Label = String;

    fn mechanism(&mut self) -> Result<Mechanism<String>, Undecided> {
        self.mechanism.clone()
    }

    fn story_cost(&self, config: &Config) -> u64 {
        let n = 64 * (8 + config.acts());
        n.saturating_mul(n).saturating_mul(64)
    }

    fn story(&mut self, config: &Config) -> StoryReplay<String> {
        let Some((log, _)) = self.found.get(config) else {
            return StoryReplay::Inconclusive(
                InconclusiveReason::EngineError,
                "no failing run of the configuration is on record".to_owned(),
            );
        };
        self.validations.set(self.validations.get() + 2);
        let Some(c) = case_built(
            String::new(),
            config.built(),
            config.epochs,
            log,
            Some(self.target),
            reach(config.epochs),
        ) else {
            return StoryReplay::Holds(format!(
                "the campaign's checker does not report {} on the recorded run",
                self.target.name()
            ));
        };
        match register_story(&c.journal, &c.built.roles, self.target) {
            Ok((labels, _, preds)) => match Story::new(labels, preds) {
                Ok(story) => StoryReplay::Fails(story),
                Err(why) => StoryReplay::Inconclusive(InconclusiveReason::EngineError, why),
            },
            Err(why) => StoryReplay::NotARun(why),
        }
    }

    fn rename(&self, config: &Config, label: &String) -> String {
        self.names_of(config).rename(label)
    }
}

impl RegisterScenario {
    pub fn candidates_of(&self, config: &Config, dimension: Dimension) -> Vec<Candidate<Config>> {
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

    pub fn admits_scope(&self, _dimension: Dimension, config: &Config) -> Result<(), String> {
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

    pub fn run_config(&mut self, config: &Config) -> Ran {
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
        // schedules: the search below still runs. The first run that fails the target
        // without the mechanism is kept aside: when no run shows the mechanism, it is
        // the run proposed, and the engine's check rejects it with its typed reason.
        let mut lost: Option<(ChoiceLog, Found)> = None;
        // A schedule whose mechanism could not be checked decides nothing (INV-008).
        let mut undecided = 0_usize;
        let first = match guided {
            None => "every actor parks under it".to_owned(),
            Some(guided) => match self.fails(config, &built, &guided) {
                Judged::Fails => {
                    self.found.insert(config.clone(), (guided, Found::Guided));
                    return Ran::Fails;
                }
                Judged::WithoutMechanism => {
                    lost = Some((guided, Found::Guided));
                    "the target fails without the mechanism".to_owned()
                }
                Judged::Holds => "the target does not fail".to_owned(),
                Judged::Undecided(why) => {
                    undecided += 1;
                    why
                }
            },
        };
        let (scope, logs) = baseline::logs_for(&built, SEARCH_LOGS, Self::seed(config));
        for (k, log) in logs.iter().enumerate() {
            match self.fails(config, &built, log) {
                Judged::Fails => {
                    self.found
                        .insert(config.clone(), (log.clone(), Found::Searched(k)));
                    return Ran::Fails;
                }
                Judged::WithoutMechanism if lost.is_none() => {
                    lost = Some((log.clone(), Found::Searched(k)));
                }
                Judged::Undecided(_) => undecided += 1,
                Judged::WithoutMechanism | Judged::Holds => {}
            }
        }
        let decided = scope == baseline::Scope::Exhaustive && undecided == 0;
        if let Some(run) = lost {
            if decided {
                self.exhausted.insert(config.clone());
            }
            self.found.insert(config.clone(), run);
            return Ran::Fails;
        }
        match scope {
            _ if undecided > 0 => Ran::Inconclusive(
                InconclusiveReason::ResourceExhausted,
                format!(
                    "the original order: {first}; the mechanism of {undecided} schedules could not be checked"
                ),
            ),
            baseline::Scope::Exhaustive => Ran::Holds(format!(
                "the original order: {first}; every one of the {} admissible schedules holds",
                logs.len()
            )),
            baseline::Scope::Sampled { seed } => Ran::Inconclusive(
                InconclusiveReason::ResourceExhausted,
                format!(
                    "the original order: {first}; none of {} schedules sampled from seed {seed} fails; the search is not exhaustive",
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
pub struct Composed {
    pub start: Config,
    pub scenario: ScenarioReduction<Config>,
    /// How the reduced configuration's run was found, and its case.
    pub found: Found,
    pub reduced: Case,
    /// bn-2z08o's closure and deletion over atoms of the reduced run.
    pub events: ProgramReduction,
    /// Configurations decided without a schedule.
    pub decided_statically: Vec<Config>,
    /// The original run's length.
    pub original_len: usize,
    /// Binding executions the minimization spent: the scenario passes' schedules (two
    /// each), the reduced run's case (two), and the event reduction's operation
    /// boundaries (one per operation prefix) and replays (at most one each).
    pub executions: u64,
}

impl Composed {
    pub fn core_len(&self) -> usize {
        self.events.reduction.core().expect("reduced").events.len()
    }

    pub fn ratio(&self) -> f64 {
        self.core_len() as f64 / self.original_len as f64
    }
}

pub fn compose_with(c: &Case, start: Config, pinned: Option<u8>, budget: Budget) -> Composed {
    let mut s = RegisterScenario::new(c, &start);
    s.pinned = pinned;
    let scenario = reduce_scenario(&mut s, start.clone(), budget);
    let config = match scenario.config() {
        Some(config) => config.subject().clone(),
        None => panic!(
            "{}: the scenario reduction finished: {:?} {:?}",
            c.label,
            match &scenario {
                ScenarioReduction::Rejected { config, why, .. } =>
                    format!("rejected {} {why:?}", config.render()),
                other => format!("{:?}", other.checks()),
            },
            scenario.checks()
        ),
    };
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
    // The event reduction of the reduced run is checked against the original failure's
    // mechanism, renamed to the reduced configuration's names (bn-5kmuf).
    let mechanism = s
        .mechanism
        .clone()
        .map(|m| m.renamed(|l| s.rename(&config, l)));
    let events = minimize_program_with(&reduced, Deletion::Atoms, Some(mechanism));
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

pub fn compose(c: &Case, start: Config) -> Composed {
    compose_with(c, start, None, SCENARIO_BUDGET)
}

/// M01's two witnesses, as `pr18_impl01_reduction.rs` reduces them, and their start
/// configurations.
pub fn agreement_start() -> Config {
    Config::start(1, baseline::scenario_replicas())
}

pub fn acked_start() -> Config {
    Config::start(
        1,
        baseline::one_epoch_sweep_replicas(baseline::SCENARIO)[81],
    )
}

pub fn agreement_composed() -> &'static Composed {
    static CELL: OnceLock<Composed> = OnceLock::new();
    CELL.get_or_init(|| compose(agreement_witness(), agreement_start()))
}

pub fn acked_composed() -> &'static Composed {
    static CELL: OnceLock<Composed> = OnceLock::new();
    CELL.get_or_init(|| compose(acked_witness(), acked_start()))
}

/// Independent check of the scenario's minimality claims from its transcript and passes
/// alone: the kept entries rebuild the version count; in the last round every claimed
/// dimension's entries were tried against the final version, every one is a decided
/// non-failure, and there are as many as its pass offered. Returns the dimensions whose
/// claim the transcript licenses.
pub fn scenario_licenses(r: &ScenarioReduction<Config>) -> Result<Vec<Dimension>, String> {
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
