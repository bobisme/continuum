//! PR 18 (bn-5kmuf): replay validation. Every reduction's result is replayed and checked
//! to reproduce the same failure through the same causal mechanism, not only the same
//! verdict. START_HERE PR 18 ("Implement deletion, causal-closure, owner/fault/value
//! reduction, and replay validation"; exit: "... does not delete the actual causal
//! mechanism").
//!
//! # The validator
//!
//! `continuum_debugger::mechanism` defines a mechanism: labelled steps, edges in the
//! replay's order or its happens-before order, and guards that state an absence. A
//! result preserves it when the validator's own replay of the result is a run, shows the
//! failure, and embeds the mechanism. Every public entry point of `reduce` and
//! `scenario` checks its input and every pass's result, and returns a core only as
//! `Validated`, which nothing else can build. The scenario keeps a candidate only when
//! the check preserves it.
//!
//! The register instantiation (`support/pr18_program.rs`): `derive_mechanism` reads M01's
//! mechanism from the original run's witnesses (`replicated_register.md`'s causal core:
//! the acknowledgement over volatile bytes, its two submissions, and under Agreement the
//! crash that loses one of them and the later acknowledgement of the other value), with
//! each edge `HappensBefore` where the original run orders it. `ProgramValidator`
//! replays a core through the program, apart from the reduction's oracle, and requires
//! the campaign's own checker to report the target on the re-execution. The scenario's
//! check (`support/pr18_scenario.rs`) replays the configuration's proposed run and
//! renames the mechanism to the configuration's names.
//!
//! # The evidence, by stable artifact ID
//!
//! `tests/golden/pr18_impl03_mechanism.evidence.txt`. Regenerate with
//! `PR18_IMPL03_BLESS=1 cargo test -p continuum-asupersync --test pr18_impl03_mechanism`
//! and review the diff.
//!
//! - `pr18-impl03-01-m01-agreement` and `pr18-impl03-02-m01-acked-not-durable`: each
//!   witness's derived mechanism, and its program-replay and composed cores, each with the
//!   check of every stage;
//! - `pr18-impl03-03-corpus`: every program-replay core and every composed core of the
//!   400 corpus runs is validated, with the distinct mechanisms;
//! - `pr18-impl03-04-substrate-cores`: the substrate replay's cores, rejected as no run
//!   of the program;
//! - `pr18-impl03-05-bn-25z9o-regressions`: the 31 composed reductions bn-25z9o accepted
//!   through another mechanism, each rejected with its typed reason;
//! - `pr18-impl03-06-seeded-wrong-mechanism`: right verdict, wrong mechanism, seeded:
//!   Agreement cores checked against another run's mechanism, and story mutants;
//! - `pr18-impl03-07-entry-points`: each public entry point returns only validated cores,
//!   and checks exactly the stages it names;
//! - `pr18-impl03-08-undecided`: a validator that cannot decide gives an INV-008
//!   inconclusive, never a core.

use std::collections::BTreeMap;
use std::fmt::Write as _;

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

#[path = "support/pr18_scenario.rs"]
#[allow(dead_code)]
mod scenario;

use baseline::Fate;
use continuum_debugger::mechanism::{
    self, Allowance, CheckEntry, Mechanism, Mismatch, Outcome, Rejection, Stage, Story,
    StoryReplay, Undecided, Validator,
};
use continuum_debugger::reduce::{self, Deletion, Reduction, Refusal, Spent};
use continuum_debugger::scenario::{ScenarioReduction, reduce_scenario};
use continuum_value::assurance::InconclusiveReason;
use program::{
    BUDGET, Case, NoMechanism, ProgramOracle, ProgramValidator, RegisterOracle, Target,
    acked_witness, agreement_witness, corpus, derive_mechanism, minimize, minimize_program,
    operations, order_of, program_order, register_story, render_mechanism,
};
use scenario::{
    Config, Found, Names, RegisterScenario, acked_start, agreement_start, compose, op_keys,
};

/// The 24 composed reductions bn-25z9o kept at the round-0 Owner pass through another
/// mechanism: M01's scenario plan, by log.
const OWNER_REGRESSIONS: [usize; 24] = [
    14, 15, 22, 26, 30, 45, 82, 129, 132, 139, 202, 214, 218, 251, 258, 277, 281, 287, 310, 312,
    317, 318, 334, 379,
];

/// The 7 bn-25z9o kept at the round-0 Fault pass through another mechanism.
const FAULT_REGRESSIONS: [usize; 7] = [20, 131, 180, 211, 282, 389, 394];

fn outcome_line(o: &Outcome) -> String {
    match o {
        Outcome::Preserved(r) => format!(
            "preserved: mechanism {:?} embedded at {:?} of a {}-step story ({} replay, {} work)",
            r.mechanism, r.embedding, r.story, r.spent.replays, r.spent.work
        ),
        Outcome::Rejected(why, _) => format!("rejected: {}", rejection_line(why)),
        Outcome::Undecided(reason, detail, _) => format!("undecided {reason:?}: {detail}"),
    }
}

fn rejection_line(r: &Rejection) -> String {
    match r {
        Rejection::NotARun(why) => format!("NotARun ({why})"),
        Rejection::FailureLost(why) => format!("FailureLost ({why})"),
        Rejection::Mechanism(m) => format!("{m:?}"),
    }
}

fn checks_line(checks: &[CheckEntry]) -> String {
    checks
        .iter()
        .map(|e| format!("{:?}: {}", e.stage, outcome_line(&e.outcome)))
        .collect::<Vec<_>>()
        .join("; ")
}

fn verdict_line(r: &Reduction) -> String {
    match r {
        Reduction::Reduced { core, .. } => {
            format!("validated core of {} events", core.events.len())
        }
        Reduction::Rejected { candidate, why, .. } => format!(
            "rejected candidate of {} events: {}",
            candidate.events.len(),
            rejection_line(why)
        ),
        Reduction::Inconclusive { reason, .. } => format!("inconclusive {reason:?}"),
        Reduction::Refused(why) => format!("refused {why:?}"),
    }
}

// ---------------------------------------------------------------------------
// the witnesses
// ---------------------------------------------------------------------------

fn witness_section(id: &str, c: &Case, start: Config) -> String {
    let mut s = String::new();
    let _ = writeln!(s, "[{id}]");
    let _ = writeln!(
        s,
        "run: M01 {}, target {}, {} events",
        c.label,
        c.target.name(),
        c.journal.len()
    );
    let m = derive_mechanism(c).expect("a mechanism");
    let _ = writeln!(
        s,
        "mechanism derived from the original failure: {}",
        render_mechanism(&m)
    );
    let p = minimize_program(c, Deletion::Atoms);
    let _ = writeln!(
        s,
        "program replay, closure and deletion over atoms: {}; {} validation replays; checks: {}",
        verdict_line(&p.reduction),
        p.validations,
        checks_line(p.reduction.checks().expect("checks"))
    );
    let k = compose(c, start);
    let _ = writeln!(
        s,
        "scenario reduction: checks: {}",
        checks_line(k.scenario.checks().expect("checks"))
    );
    let _ = writeln!(
        s,
        "composed event reduction of the reduced run ({} events): {}; checks: {}",
        k.reduced.journal.len(),
        verdict_line(&k.events.reduction),
        checks_line(k.events.reduction.checks().expect("checks"))
    );
    s
}

// ---------------------------------------------------------------------------
// the corpus
// ---------------------------------------------------------------------------

struct CorpusFacts {
    runs: usize,
    program_validated: usize,
    composed_validated: usize,
    /// Distinct mechanisms, with how many runs have each, and a representative run.
    mechanisms: BTreeMap<String, (usize, usize)>,
    lost_candidates: u64,
    lost_runs: Vec<usize>,
}

fn corpus_facts() -> &'static CorpusFacts {
    static CELL: std::sync::OnceLock<CorpusFacts> = std::sync::OnceLock::new();
    CELL.get_or_init(|| {
        let mut f = CorpusFacts {
            runs: 0,
            program_validated: 0,
            composed_validated: 0,
            mechanisms: BTreeMap::new(),
            lost_candidates: 0,
            lost_runs: Vec::new(),
        };
        for (i, c) in corpus().iter().enumerate() {
            f.runs += 1;
            let m = render_mechanism(&derive_mechanism(c).expect("a mechanism"));
            f.mechanisms
                .entry(format!("[{}] {m}", c.target.name()))
                .or_insert((0, i))
                .0 += 1;
            if minimize_program(c, Deletion::Atoms)
                .reduction
                .core()
                .is_some()
            {
                f.program_validated += 1;
            }
            let k = compose(c, agreement_start());
            if k.scenario.config().is_some() && k.events.reduction.core().is_some() {
                f.composed_validated += 1;
            }
            let lost = k
                .scenario
                .checks()
                .expect("checks")
                .iter()
                .filter(|e| matches!(e.outcome, Outcome::Rejected(..)))
                .count() as u64;
            if lost > 0 {
                f.lost_candidates += lost;
                f.lost_runs.push(label_log(c));
            }
        }
        f
    })
}

fn label_log(c: &Case) -> usize {
    c.label
        .rsplit(' ')
        .next()
        .and_then(|x| x.parse().ok())
        .expect("a corpus label")
}

fn corpus_case(log: usize) -> &'static Case {
    corpus()
        .iter()
        .find(|c| label_log(c) == log)
        .expect("a corpus run")
}

fn corpus_section() -> String {
    let f = corpus_facts();
    let mut s = String::new();
    let _ = writeln!(s, "[pr18-impl03-03-corpus]");
    let _ = writeln!(
        s,
        "corpus: the {} failing runs of M01's scenario plan; program-replay cores validated: {}; composed cores validated: {}",
        f.runs, f.program_validated, f.composed_validated
    );
    let _ = writeln!(
        s,
        "scenario candidates whose failing run the mechanism check rejected, never kept: {} on {} runs (logs {:?})",
        f.lost_candidates,
        f.lost_runs.len(),
        f.lost_runs
    );
    let _ = writeln!(
        s,
        "distinct mechanisms derived from the original failures: {}",
        f.mechanisms.len()
    );
    for (m, (n, _)) in &f.mechanisms {
        let _ = writeln!(s, "  {n} runs: {m}");
    }
    s
}

// ---------------------------------------------------------------------------
// the negative corpus
// ---------------------------------------------------------------------------

/// The substrate replay's cores: each rejected as no run of the program.
fn substrate_facts() -> Vec<(String, Deletion, Reduction)> {
    let mut out = Vec::new();
    for c in [agreement_witness(), acked_witness()] {
        for mode in [Deletion::Configurations, Deletion::Atoms] {
            out.push((c.label.clone(), mode, minimize(c, mode).0));
        }
    }
    out
}

fn substrate_section() -> String {
    let mut s = String::new();
    let _ = writeln!(s, "[pr18-impl03-04-substrate-cores]");
    for (label, mode, r) in substrate_facts() {
        let _ = writeln!(
            s,
            "M01 {label}, substrate replay, deletion over {mode:?}: {}",
            verdict_line(&r)
        );
    }
    s
}

/// bn-25z9o's reduced configuration for the 31 regressions: a writes v0 clean (its crash
/// dropped), b writes v0 clean, c idle, only used owners spawned.
fn bn25z9o_config() -> Config {
    let mut cfg = agreement_start();
    cfg.replicas[0].slots[0].1 = Fate::Clean;
    cfg.replicas[0].values[0] = 0;
    cfg.replicas[2].slots[0].1 = Fate::Idle;
    cfg.pruned = true;
    cfg
}

/// bn-25z9o's run of its reduced configuration for corpus run `c`, found as bn-25z9o
/// found it: the original order carried over, then the seeded search, the first run that
/// fails the target with some chain (`keeps_mechanism`, its "any chain" acceptance).
/// That run checked against `c`'s mechanism: the typed reason.
fn bn25z9o_check(c: &Case) -> Result<Vec<usize>, Mismatch> {
    let start = agreement_start();
    let mut s = RegisterScenario::new(c, &start);
    let cfg = bn25z9o_config();
    let built = cfg.built();
    let keys = op_keys(&cfg.plan(), &built);
    let guided = register::guided_log(&built, |a, i| {
        s.rank.get(&keys[a][i]).copied().unwrap_or(u64::MAX / 2)
    });
    let (_, logs) = baseline::logs_for(&built, scenario::SEARCH_LOGS, RegisterScenario::seed(&cfg));
    let log = guided
        .into_iter()
        .chain(logs)
        .find(|log| {
            program::case_built(
                String::new(),
                built.clone(),
                cfg.epochs,
                log,
                Some(c.target),
                scenario::reach(cfg.epochs),
            )
            .is_some_and(|r| {
                let all: Vec<usize> = (0..r.journal.len()).collect();
                program::keeps_mechanism(&program::story(&r, &all), c.target)
            })
        })
        .expect("bn-25z9o kept a failing run");
    s.found.insert(cfg.clone(), (log, Found::Guided));
    let StoryReplay::Fails(story) = s.story(&cfg) else {
        panic!("{}: bn-25z9o's run fails the target", c.label);
    };
    let m = s.mechanism().expect("a mechanism");
    mechanism::embed(
        &m,
        &story,
        &mut Allowance {
            ..Allowance::DEFAULT
        },
        &mut Spent::default(),
    )
    .expect("decided")
}

/// One regression: its log, the pass, bn-25z9o's run's check, and whether the
/// reduction now returns a validated core.
type Regression = (usize, &'static str, Result<Vec<usize>, Mismatch>, bool);

fn regression_facts() -> Vec<Regression> {
    let mut out = Vec::new();
    for (kind, logs) in [
        ("Owner", OWNER_REGRESSIONS.as_slice()),
        ("Fault", FAULT_REGRESSIONS.as_slice()),
    ] {
        for &log in logs {
            let c = corpus_case(log);
            let k = compose(c, agreement_start());
            let validated = k.scenario.config().is_some() && k.events.reduction.core().is_some();
            out.push((log, kind, bn25z9o_check(c), validated));
        }
    }
    out
}

fn regression_section() -> String {
    let mut s = String::new();
    let _ = writeln!(s, "[pr18-impl03-05-bn-25z9o-regressions]");
    let _ = writeln!(
        s,
        "bn-25z9o's reduced configuration for these runs: {}; its run, the original order carried over, fails the target through another mechanism",
        bn25z9o_config().render()
    );
    for (log, kind, check, validated) in regression_facts() {
        let _ = writeln!(
            s,
            "  scenario plan 0 log {log} (round-0 {kind} pass): bn-25z9o's run checked against the original mechanism: {}; the reduction now: composed core validated {validated}",
            match check {
                Ok(at) => format!("preserved at {at:?}"),
                Err(m) => format!("rejected {m:?}"),
            }
        );
    }
    s
}

/// `c`'s validated program-replay core checked, through `reduce::deletion_pass`, against
/// `mechanism`: the input's check is the outcome.
fn check_core_against(c: &Case, mechanism: Result<Mechanism<String>, Undecided>) -> Outcome {
    let p = minimize_program(c, Deletion::Atoms);
    let core = p.reduction.core().expect("validated");
    let mut v = ProgramValidator::with(c, operations(c), mechanism);
    let r = reduce::deletion_pass(
        &p.order,
        core.events.clone(),
        Deletion::Atoms,
        &mut ProgramOracle {
            case: c,
            ops: operations(c),
            replays: 0,
        },
        &mut v,
        BUDGET,
    );
    match r {
        Reduction::Refused(Refusal::MechanismNotInInput(why)) => {
            Outcome::Rejected(why, Spent::default())
        }
        other => other.checks().and_then(|c| c.first()).map_or_else(
            || {
                Outcome::Undecided(
                    InconclusiveReason::EngineError,
                    format!("{other:?}"),
                    Spent::default(),
                )
            },
            |e| e.outcome.clone(),
        ),
    }
}

/// Swap two names in every label of `m`.
fn swapped(m: &Mechanism<String>, x: &str, y: &str) -> Mechanism<String> {
    m.renamed(|l| l.replace(x, "\u{0}").replace(y, x).replace('\u{0}', y))
}

/// Right verdict, wrong mechanism, seeded. Each core fails its target through its own
/// chain and is checked against a mechanism with other steps: the mechanism of another
/// corpus run of the same target whose steps differ, and the Agreement witness's own
/// mechanism with the lost replica, or the two values, swapped.
fn swap_facts() -> Vec<(String, Outcome)> {
    let classes: Vec<(String, usize)> = corpus_facts()
        .mechanisms
        .iter()
        .map(|(m, &(_, i))| (m.clone(), i))
        .collect();
    let steps = |i: usize| {
        let mut v = derive_mechanism(&corpus()[i])
            .expect("a mechanism")
            .steps()
            .to_vec();
        v.sort();
        v
    };
    let mut out = Vec::new();
    for (mi, i) in &classes {
        for (mj, j) in &classes {
            let (ci, cj) = (&corpus()[*i], &corpus()[*j]);
            if ci.target != cj.target || steps(*i) == steps(*j) || mi == mj {
                continue;
            }
            out.push((
                format!(
                    "{} core of log {} against the mechanism of log {}",
                    ci.target.name(),
                    label_log(ci),
                    label_log(cj)
                ),
                check_core_against(ci, derive_mechanism(cj)),
            ));
        }
    }
    let c = agreement_witness();
    let m = derive_mechanism(c).expect("a mechanism");
    out.push((
        format!(
            "M01 {}'s core against its mechanism with replicas a and b swapped (b's bytes lost)",
            c.label
        ),
        check_core_against(c, Ok(swapped(&m, "n=a", "n=b"))),
    ));
    out.push((
        format!(
            "M01 {}'s core against its mechanism with v0 and v1 swapped (v1 acknowledged first)",
            c.label
        ),
        check_core_against(c, Ok(swapped(&m, "value=v0", "value=v1"))),
    ));
    out
}

/// Story mutants of the Agreement witness's validated core, each checked against its
/// mechanism: the story as a total order, then one change.
fn mutant_facts() -> Vec<(&'static str, Result<Vec<usize>, Mismatch>)> {
    let c = agreement_witness();
    let m = derive_mechanism(c).expect("a mechanism");
    let p = minimize_program(c, Deletion::Atoms);
    let core = p.reduction.core().expect("validated");
    let r = continuum_asupersync::causal::restrict(&c.journal, &core.events).expect("restricts");
    let roles = program::carry_roles(&c.built.roles, &r.renaming);
    let (labels, _, _) = register_story(&r.journal, &roles, c.target).expect("a story");
    let pos = |l: &str| labels.iter().position(|x| x == l).expect("present");
    let lose = pos("Lose(n=a,epoch=0,value=v0)");
    let ack1 = pos("Ack(epoch=0,value=v1)");
    let sub_a = pos("Submit(n=a,epoch=0,value=v0)");
    let mut mutants: Vec<(&'static str, Vec<String>)> =
        vec![("the core's own story", labels.clone())];
    let mut without_lose = labels.clone();
    without_lose.remove(lose);
    mutants.push(("the crash's Lose removed", without_lose));
    let mut late_lose = labels.clone();
    let l = late_lose.remove(lose);
    late_lose.insert(ack1, l);
    mutants.push(("the Lose moved after the second acknowledgement", late_lose));
    let mut synced = labels.clone();
    synced.insert(sub_a + 1, "Sync(n=a,epoch=0)".to_owned());
    mutants.push((
        "a Sync of a between its Submit and the first acknowledgement",
        synced,
    ));
    let mut resubmit = labels.clone();
    resubmit.insert(lose, "Submit(n=a,epoch=0,value=v1)".to_owned());
    mutants.push(("a resubmits before its bytes are lost", resubmit));
    let mut retried = labels.clone();
    retried.splice(
        sub_a + 1..sub_a + 1,
        [
            "Lose(n=a,epoch=0,value=v0)".to_owned(),
            "Submit(n=a,epoch=0,value=v0)".to_owned(),
            "Sync(n=a,epoch=0)".to_owned(),
        ],
    );
    mutants.push((
        "a's bytes lost, v0 submitted again and synced before the first acknowledgement",
        retried,
    ));
    let mut no_second = labels;
    no_second.remove(ack1);
    mutants.push(("the second acknowledgement removed", no_second));
    mutants
        .into_iter()
        .map(|(name, labels)| {
            let preds = (0..labels.len()).map(|k| (0..k).collect()).collect();
            let story = Story::new(labels, preds).expect("a total order");
            let got = mechanism::embed(
                &m,
                &story,
                &mut Allowance {
                    ..Allowance::DEFAULT
                },
                &mut Spent::default(),
            )
            .expect("decided");
            (name, got)
        })
        .collect()
}

fn seeded_section() -> String {
    let mut s = String::new();
    let _ = writeln!(s, "[pr18-impl03-06-seeded-wrong-mechanism]");
    let _ = writeln!(
        s,
        "validated cores checked against a mechanism with other steps (each core fails its target through its own chain):"
    );
    for (name, o) in swap_facts() {
        let _ = writeln!(s, "  {name}: {}", outcome_line(&o));
    }
    let _ = writeln!(
        s,
        "story mutants of M01 {}'s validated core:",
        agreement_witness().label
    );
    for (name, got) in mutant_facts() {
        let _ = writeln!(
            s,
            "  {name}: {}",
            match got {
                Ok(at) => format!("preserved at {at:?}"),
                Err(m) => format!("rejected {m:?}"),
            }
        );
    }
    s
}

// ---------------------------------------------------------------------------
// every entry point
// ---------------------------------------------------------------------------

/// A validator that counts its replays and passes them on.
struct Counting<'a> {
    inner: ProgramValidator<'a>,
    stories: u64,
}

impl Validator<[usize]> for Counting<'_> {
    type Label = String;

    fn mechanism(&mut self) -> Result<Mechanism<String>, Undecided> {
        self.inner.mechanism()
    }

    fn story_cost(&self, kept: &[usize]) -> u64 {
        self.inner.story_cost(kept)
    }

    fn story(&mut self, kept: &[usize]) -> StoryReplay<String> {
        self.stories += 1;
        self.inner.story(kept)
    }
}

/// Each public entry point of `reduce`, under the program replay and under the
/// substrate replay, on `c`: its name, result, the stages it checked, and the
/// validator's replay count.
fn entry_facts(c: &Case) -> Vec<(String, Reduction, u64)> {
    let ops = operations(c);
    let porder = program_order(c, ops);
    let sorder = order_of(c);
    let whole: Vec<usize> = (0..c.journal.len()).collect();
    let mut out = Vec::new();
    for program in [true, false] {
        let order = if program { &porder } else { &sorder };
        let runs: [(&str, Option<Deletion>, bool); 5] = [
            ("closure_pass", None, false),
            (
                "deletion_pass over Configurations",
                Some(Deletion::Configurations),
                false,
            ),
            ("deletion_pass over Atoms", Some(Deletion::Atoms), false),
            (
                "minimize over Configurations",
                Some(Deletion::Configurations),
                true,
            ),
            ("minimize over Atoms", Some(Deletion::Atoms), true),
        ];
        for (name, mode, both) in runs {
            let mut v = Counting {
                inner: ProgramValidator::new(c, ops),
                stories: 0,
            };
            let r = if program {
                let mut o = ProgramOracle {
                    case: c,
                    ops,
                    replays: 0,
                };
                run_entry(order, &whole, mode, both, &mut o, &mut v)
            } else {
                let mut o = RegisterOracle::new(&c.journal, &c.built.roles, c.target);
                run_entry(order, &whole, mode, both, &mut o, &mut v)
            };
            let replay = if program { "program" } else { "substrate" };
            out.push((format!("{name} ({replay} replay)"), r, v.stories));
        }
    }
    out
}

fn run_entry<R: reduce::Replay, V: Validator<[usize]>>(
    order: &reduce::CausalOrder,
    whole: &[usize],
    mode: Option<Deletion>,
    both: bool,
    oracle: &mut R,
    v: &mut V,
) -> Reduction {
    match (mode, both) {
        (None, _) => reduce::closure_pass(order, oracle, v, BUDGET),
        (Some(m), false) => reduce::deletion_pass(order, whole.to_vec(), m, oracle, v, BUDGET),
        (Some(m), true) => reduce::minimize(order, m, oracle, v, BUDGET),
    }
}

/// The stages a finished reduction must have checked: the input, then each pass whose
/// result differs from its input.
fn expected_stages(r: &Reduction) -> Vec<Stage> {
    let mut out = vec![Stage::Input];
    for p in r.transcript().expect("started") {
        if p.after != p.before {
            out.push(Stage::Pass(p.pass));
        }
    }
    out
}

fn entry_section() -> String {
    let mut s = String::new();
    let _ = writeln!(s, "[pr18-impl03-07-entry-points]");
    for c in [agreement_witness(), acked_witness()] {
        for (name, r, stories) in entry_facts(c) {
            let stages: Vec<Stage> = r
                .checks()
                .expect("checks")
                .iter()
                .map(|e| e.stage)
                .collect();
            let _ = writeln!(
                s,
                "M01 {} {name}: {}; stages checked {:?}; validator replays {stories}",
                c.label,
                verdict_line(&r),
                stages
            );
        }
        let mut sc = RegisterScenario::new(c, &start_of(c));
        let r = reduce_scenario(&mut sc, start_of(c), scenario::SCENARIO_BUDGET);
        let _ = writeln!(
            s,
            "M01 {} reduce_scenario: {}; stages checked {:?}",
            c.label,
            match &r {
                ScenarioReduction::Reduced { config, .. } =>
                    format!("validated configuration {}", config.render()),
                other => format!("{other:?}"),
            },
            r.checks()
                .expect("checks")
                .iter()
                .map(|e| e.stage)
                .collect::<Vec<_>>()
        );
    }
    s
}

fn start_of(c: &Case) -> Config {
    if c.target == Target::Agreement {
        agreement_start()
    } else {
        acked_start()
    }
}

// ---------------------------------------------------------------------------
// the validator that cannot decide
// ---------------------------------------------------------------------------

fn undecided_facts() -> Vec<(&'static str, Reduction)> {
    let c = agreement_witness();
    let ops = operations(c);
    let order = program_order(c, ops);
    let mut o = ProgramOracle {
        case: c,
        ops,
        replays: 0,
    };
    let no_mechanism = reduce::minimize(&order, Deletion::Atoms, &mut o, &mut NoMechanism, BUDGET);
    let mut o = ProgramOracle {
        case: c,
        ops,
        replays: 0,
    };
    let no_replays = reduce::minimize(
        &order,
        Deletion::Atoms,
        &mut o,
        &mut ProgramValidator::new(c, ops),
        BUDGET.with_validation(0, 1 << 40),
    );
    let mut o = ProgramOracle {
        case: c,
        ops,
        replays: 0,
    };
    let little_work = reduce::minimize(
        &order,
        Deletion::Atoms,
        &mut o,
        &mut ProgramValidator::new(c, ops),
        BUDGET.with_validation(64, 1_000),
    );
    // One replay: the input's check, and none left for the closure's.
    let mut o = ProgramOracle {
        case: c,
        ops,
        replays: 0,
    };
    let one_check = reduce::minimize(
        &order,
        Deletion::Atoms,
        &mut o,
        &mut ProgramValidator::new(c, ops),
        BUDGET.with_validation(1, 1 << 40),
    );
    vec![
        ("a validator with no mechanism", no_mechanism),
        ("a validation allowance of no replay", no_replays),
        ("a validation allowance of 1000 work units", little_work),
        ("a validation allowance of one replay", one_check),
    ]
}

fn undecided_section() -> String {
    let mut s = String::new();
    let _ = writeln!(s, "[pr18-impl03-08-undecided]");
    for (name, r) in undecided_facts() {
        let best = match &r {
            Reduction::Inconclusive { best, .. } => best.as_ref().map_or_else(
                || "no best set".to_owned(),
                |b| {
                    format!(
                        "best set of {} events, its check {}",
                        b.subject().events.len(),
                        outcome_line(&b.outcome())
                    )
                },
            ),
            _ => String::new(),
        };
        let _ = writeln!(
            s,
            "M01 {} under {name}: {}; {best}; checks: {}",
            agreement_witness().label,
            verdict_line(&r),
            r.checks().map_or_else(String::new, checks_line)
        );
    }
    s
}

fn evidence() -> String {
    let mut s = String::new();
    s.push_str(
        "# PR 18 (bn-5kmuf): replay validation. Each reduction's result is replayed and checked to reproduce the same failure through the same causal mechanism.\n\
         # Regenerate: PR18_IMPL03_BLESS=1 cargo test -p continuum-asupersync --test pr18_impl03_mechanism\n\
         # Mechanism: steps; edges `a hb b` (happens-before) or `a before b` (replay order); guards: forbidden labels between two steps, until a loss. Positions are story indices.\n\n",
    );
    s.push_str(&witness_section(
        "pr18-impl03-01-m01-agreement",
        agreement_witness(),
        agreement_start(),
    ));
    s.push('\n');
    s.push_str(&witness_section(
        "pr18-impl03-02-m01-acked-not-durable",
        acked_witness(),
        acked_start(),
    ));
    s.push('\n');
    s.push_str(&corpus_section());
    s.push('\n');
    s.push_str(&substrate_section());
    s.push('\n');
    s.push_str(&regression_section());
    s.push('\n');
    s.push_str(&seeded_section());
    s.push('\n');
    s.push_str(&entry_section());
    s.push('\n');
    s.push_str(&undecided_section());
    s
}

// ---------------------------------------------------------------------------
// tests
// ---------------------------------------------------------------------------

/// Acceptance: M01's ack-before-sync core replays to the same failure through the same
/// mechanism, under the program replay and composed with the scenario reduction, and the
/// mechanism is the one `replicated_register.md` names.
#[test]
fn the_ack_before_sync_core_keeps_its_mechanism() {
    let c = agreement_witness();
    let m = derive_mechanism(c).expect("a mechanism");
    assert_eq!(
        m.steps(),
        [
            "Ack(epoch=0,value=v0)",
            "Submit(n=a,epoch=0,value=v0)",
            "Submit(n=b,epoch=0,value=v0)",
            "Lose(n=a,epoch=0,value=v0)",
            "Ack(epoch=0,value=v1)",
        ]
    );
    assert!(
        m.edges()
            .iter()
            .all(|e| e.order == mechanism::Order::HappensBefore)
    );
    assert_eq!(m.guards().len(), 5);
    for (c, start) in [
        (agreement_witness(), agreement_start()),
        (acked_witness(), acked_start()),
    ] {
        let p = minimize_program(c, Deletion::Atoms);
        let core = p.reduction.core().expect("validated");
        assert!(core.events.len() < c.journal.len());
        let k = compose(c, start);
        assert!(k.scenario.config().is_some(), "{}", c.label);
        assert!(k.events.reduction.core().is_some(), "{}", c.label);
    }
}

/// Every program-replay core and every composed core of the corpus is validated.
#[test]
fn every_corpus_core_is_validated() {
    let f = corpus_facts();
    assert_eq!(f.runs, baseline::SCENARIO_LOGS);
    assert_eq!(f.program_validated, f.runs);
    assert_eq!(f.composed_validated, f.runs);
    assert_eq!(f.lost_runs, OWNER_REGRESSIONS);
}

/// The substrate replay's cores are no run of the program: every one is rejected, typed,
/// and none is returned as a core.
#[test]
fn substrate_cores_are_rejected_as_no_run() {
    for (label, mode, r) in substrate_facts() {
        assert!(
            matches!(
                r,
                Reduction::Rejected {
                    why: Rejection::NotARun(_),
                    ..
                }
            ),
            "{label} {mode:?}: {r:?}"
        );
    }
}

/// The 31 bn-25z9o regressions: its run of each is rejected with the typed reason (24
/// `MissingStep` at the Owner pass, 7 `GuardBroken` at the Fault pass), and the
/// reduction now returns a validated core for every one.
#[test]
fn the_bn_25z9o_regressions_are_rejected_with_their_reasons() {
    for (log, kind, check, validated) in regression_facts() {
        assert!(validated, "log {log}");
        match kind {
            "Owner" => assert!(
                matches!(check, Err(Mismatch::MissingStep { .. })),
                "log {log}: {check:?}"
            ),
            _ => assert!(
                matches!(check, Err(Mismatch::GuardBroken { .. })),
                "log {log}: {check:?}"
            ),
        }
    }
    // No other corpus run's bn-25z9o reduction lost the mechanism: the list is complete.
    let named: Vec<usize> = OWNER_REGRESSIONS
        .iter()
        .chain(&FAULT_REGRESSIONS)
        .copied()
        .collect();
    for c in corpus() {
        if c.target == Target::AckedNotDurable && !named.contains(&label_log(c)) {
            assert!(bn25z9o_check(c).is_ok(), "{}", c.label);
        }
    }
}

/// Seeded right-verdict, wrong-mechanism cases are rejected with a typed reason: an
/// Agreement core against another run's Agreement mechanism, and each story mutant.
#[test]
fn seeded_wrong_mechanisms_are_rejected() {
    let swaps = swap_facts();
    assert!(!swaps.is_empty());
    for (name, o) in &swaps {
        assert!(
            matches!(o, Outcome::Rejected(Rejection::Mechanism(_), _)),
            "{name}: {o:?}"
        );
    }
    let mutants = mutant_facts();
    assert!(mutants[0].1.is_ok(), "the core's own story embeds");
    for (name, got) in &mutants[1..] {
        assert!(got.is_err(), "{name}");
    }
}

/// Every public entry point returns only validated cores, and checks the input and every
/// pass that changed its input: one validator replay per check, none skipped.
#[test]
fn every_entry_point_returns_only_validated_cores() {
    for c in [agreement_witness(), acked_witness()] {
        for (name, r, stories) in entry_facts(c) {
            let checks = r.checks().expect("checks");
            assert_eq!(stories, checks.len() as u64, "{name}");
            let stages: Vec<Stage> = checks.iter().map(|e| e.stage).collect();
            assert_eq!(stages, expected_stages(&r), "{name}");
            match &r {
                Reduction::Reduced { core, checks, .. } => {
                    let last = checks.last().expect("a check");
                    assert!(matches!(last.outcome, Outcome::Preserved(_)), "{name}");
                    assert!(
                        name.contains("program"),
                        "{name}: only a program run validates"
                    );
                    assert!(core.record().story > 0);
                }
                Reduction::Rejected { .. } => assert!(name.contains("substrate"), "{name}"),
                other => panic!("{name}: {other:?}"),
            }
        }
        let mut sc = RegisterScenario::new(c, &start_of(c));
        let r = reduce_scenario(&mut sc, start_of(c), scenario::SCENARIO_BUDGET);
        let stages: Vec<Stage> = r
            .checks()
            .expect("checks")
            .iter()
            .map(|e| e.stage)
            .collect();
        assert_eq!(stages[0], Stage::Input);
        assert!(r.config().is_some(), "{}", c.label);
    }
    // The composed pipeline ends in `reduce::minimize`, and its core is validated against
    // the original failure's mechanism, renamed: every corpus run.
    assert_eq!(corpus_facts().composed_validated, corpus_facts().runs);
}

/// INV-008: a validator that cannot decide is a typed inconclusive, never a core.
#[test]
fn an_undecided_check_is_inconclusive_never_a_core() {
    for (name, r) in undecided_facts() {
        match &r {
            Reduction::Inconclusive { reason, .. } => assert!(
                matches!(
                    reason,
                    InconclusiveReason::Unsupported | InconclusiveReason::ResourceExhausted
                ),
                "{name}: {reason:?}"
            ),
            other => panic!("{name}: {other:?}"),
        }
    }
}

fn check_golden(path: &str, got: &str) -> Vec<String> {
    if std::env::var_os("PR18_IMPL03_BLESS").is_some() {
        std::fs::write(path, got).expect("writes the golden");
        panic!("the golden was rewritten: review the diff and rerun without PR18_IMPL03_BLESS");
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
        "{path} drifted at {first}; regenerate with PR18_IMPL03_BLESS=1 and review"
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
            "/tests/golden/pr18_impl03_mechanism.evidence.txt"
        ),
        &evidence(),
    );
    assert_eq!(
        ids,
        [
            "pr18-impl03-01-m01-agreement",
            "pr18-impl03-02-m01-acked-not-durable",
            "pr18-impl03-03-corpus",
            "pr18-impl03-04-substrate-cores",
            "pr18-impl03-05-bn-25z9o-regressions",
            "pr18-impl03-06-seeded-wrong-mechanism",
            "pr18-impl03-07-entry-points",
            "pr18-impl03-08-undecided",
        ]
    );
}

/// The metamorphic relation, symmetry renaming: swap v0 and v1 in every role of each
/// witness. The journal carries no value and both targets are symmetric in values, so
/// the derived mechanism is the original's with the values swapped, and the validated
/// core, its embedding and every check are unchanged.
#[test]
fn symmetry_renaming_renames_the_mechanism_and_keeps_the_check() {
    let swap = |v: u8| 1 - v;
    for c in [agreement_witness(), acked_witness()] {
        let mut renamed = c.clone();
        for (role, _) in &mut renamed.built.roles.tasks {
            *role = match *role {
                register::Role::Writer { node, epoch, value } => register::Role::Writer {
                    node,
                    epoch,
                    value: swap(value),
                },
                register::Role::Coordinator { epoch, value } => register::Role::Coordinator {
                    epoch,
                    value: swap(value),
                },
                other => other,
            };
        }
        let m = derive_mechanism(c).expect("a mechanism");
        assert_eq!(
            derive_mechanism(&renamed).expect("a mechanism"),
            swapped(&m, "value=v0", "value=v1"),
            "{}",
            c.label
        );
        let a = minimize_program(c, Deletion::Atoms);
        let b = minimize_program(&renamed, Deletion::Atoms);
        assert_eq!(a.reduction.checks(), b.reduction.checks(), "{}", c.label);
        assert_eq!(a.reduction.core(), b.reduction.core(), "{}", c.label);
    }
}

/// Dropping an epoch renames it to a name no run carries: the original epoch-0 mechanism
/// never matches the surviving epoch-1 operations renamed to epoch 0.
#[test]
fn a_dropped_epoch_is_never_matched() {
    let keep_one = Names {
        values: [0, 1],
        epochs: [None, Some(0)],
    };
    assert_eq!(
        keep_one.rename("Ack(epoch=0,value=v0)"),
        "Ack(epoch=dropped,value=v0)"
    );
    assert_eq!(
        keep_one.rename("Submit(n=a,epoch=1,value=v1)"),
        "Submit(n=a,epoch=0,value=v1)"
    );
    let m = derive_mechanism(agreement_witness()).expect("a mechanism");
    let renamed = m.renamed(|l| keep_one.rename(l));
    assert!(renamed.steps().iter().all(|l| l.contains("epoch=dropped")));
}

/// The mechanism a synthetic story derives through the register's own derivation
/// (`chains`, then `chain_mechanism`), the story being a total order.
fn derive_from(labels: &[&str], target: Target) -> Option<Mechanism<String>> {
    let st: Vec<String> = labels.iter().map(|l| (*l).to_owned()).collect();
    let before = |a: usize, b: usize| a < b;
    let chain = program::chains(&st, target, &before).into_iter().next()?;
    Some(program::chain_mechanism(&st, &chain, &before).expect("a mechanism"))
}

/// `labels` as a totally ordered story, checked against `m`.
fn check_story(m: &Mechanism<String>, labels: &[&str]) -> Result<Vec<usize>, Mismatch> {
    let labels: Vec<String> = labels.iter().map(|l| (*l).to_owned()).collect();
    let preds = (0..labels.len()).map(|k| (0..k).collect()).collect();
    let story = Story::new(labels, preds).expect("a total order");
    mechanism::embed(
        m,
        &story,
        &mut Allowance {
            ..Allowance::DEFAULT
        },
        &mut Spent::default(),
    )
    .expect("decided")
}

/// cr-2lnu3c th-2q3qcy: a mechanism is bound to its acknowledgement's epoch. The durable
/// register keeps one slot per replica and epoch, so another epoch's `Submit`, `Lose` or
/// `Sync` writes another slot: it never satisfies a step, never ends a guard, and never
/// breaks one. Each two-epoch replay below keeps the verdict's labels but moves one
/// operation to epoch 1, and is rejected with a typed reason; the derivation from it
/// finds no epoch-0 chain.
#[test]
fn cross_epoch_labels_never_satisfy_a_step_or_clear_a_guard() {
    let base = [
        "Submit(n=a,epoch=0,value=v0)",
        "Submit(n=b,epoch=0,value=v0)",
        "Ack(epoch=0,value=v0)",
        "Lose(n=a,epoch=0,value=v0)",
        "Submit(n=a,epoch=0,value=v1)",
        "Submit(n=c,epoch=0,value=v1)",
        "Ack(epoch=0,value=v1)",
    ];
    let m = derive_from(&base, Target::Agreement).expect("the epoch-0 chain");
    assert!(m.steps().iter().all(|l| l.contains("epoch=0")));
    for g in m.guards() {
        assert!(
            g.forbidden
                .iter()
                .chain(&g.until)
                .all(|l| l.contains("epoch=0"))
        );
    }
    // Positive control.
    assert!(check_story(&m, &base).is_ok());
    let with = |at: usize, label: &'static str| {
        let mut v = base.to_vec();
        v[at] = label;
        v
    };
    let insert = |at: usize, labels: &[&'static str]| {
        let mut v = base.to_vec();
        v.splice(at..at, labels.iter().copied());
        v
    };
    // Submit: a's submission only in epoch 1.
    let submit = with(0, "Submit(n=a,epoch=1,value=v0)");
    assert_eq!(
        check_story(&m, &submit),
        Err(Mismatch::MissingStep { step: 1 })
    );
    assert!(derive_from(&submit, Target::Agreement).is_none());
    // Lose: the loss only in epoch 1.
    let lose = with(3, "Lose(n=a,epoch=1,value=v0)");
    assert_eq!(
        check_story(&m, &lose),
        Err(Mismatch::MissingStep { step: 3 })
    );
    assert!(derive_from(&lose, Target::Agreement).is_none());
    // The later acknowledgement only in epoch 1: Agreement is per epoch.
    let later = with(6, "Ack(epoch=1,value=v1)");
    assert_eq!(
        check_story(&m, &later),
        Err(Mismatch::MissingStep { step: 4 })
    );
    assert!(derive_from(&later, Target::Agreement).is_none());
    // Sync: an epoch-1 loss does not end the no-Sync guard, so a's epoch-0 Sync after it
    // breaks it.
    let cleared = insert(1, &["Lose(n=a,epoch=1,value=v0)", "Sync(n=a,epoch=0)"]);
    assert!(matches!(
        check_story(&m, &cleared),
        Err(Mismatch::GuardBroken { .. })
    ));
    // An epoch-1 Sync is another slot's durability: it does not break the guard.
    assert!(check_story(&m, &insert(1, &["Sync(n=a,epoch=1)"])).is_ok());
    // A resubmission in epoch 1 is another slot's write; one in epoch 0 breaks the guard.
    assert!(check_story(&m, &insert(3, &["Submit(n=a,epoch=1,value=v1)"])).is_ok());
    assert!(matches!(
        check_story(&m, &insert(3, &["Submit(n=a,epoch=0,value=v1)"])),
        Err(Mismatch::GuardBroken { .. })
    ));
    // The derivation reads volatility per epoch: an epoch-1 Sync leaves a's epoch-0
    // bytes volatile, and an epoch-1 Lose does not hide a's epoch-0 Sync.
    let acked = [
        "Submit(n=a,epoch=0,value=v0)",
        "Submit(n=b,epoch=0,value=v0)",
        "Sync(n=a,epoch=1)",
        "Ack(epoch=0,value=v0)",
    ];
    let m2 = derive_from(&acked, Target::AckedNotDurable).expect("a chain");
    assert!(check_story(&m2, &acked).is_ok());
    let hidden = [
        "Submit(n=a,epoch=0,value=v0)",
        "Lose(n=a,epoch=1,value=v0)",
        "Sync(n=a,epoch=0)",
        "Submit(n=b,epoch=0,value=v0)",
        "Sync(n=b,epoch=0)",
        "Ack(epoch=0,value=v0)",
    ];
    assert!(
        derive_from(&hidden, Target::AckedNotDurable).is_none(),
        "both epoch-0 submissions are synced: no ack over volatile bytes"
    );
    // A label with no epoch, or a dropped one, never matches.
    assert!(
        derive_from(
            &[
                "Submit(n=a,value=v0)",
                "Submit(n=b,value=v0)",
                "Ack(value=v0)"
            ],
            Target::AckedNotDurable
        )
        .is_none()
    );
}
