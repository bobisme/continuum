//! PR 21 (bn-4ykgg): the generic neighborhood generator over random causal cores and a
//! toy substrate. The replicated register instantiation and its evidence are in
//! `crates/continuum-asupersync/tests/pr21_impl01_neighborhood.rs`.

use std::collections::{BTreeMap, BTreeSet};

use continuum_repair::neighborhood::intent::{
    Bounds, DeclaredBound, ExplorationBound, FaultClass, FaultModel, Json,
};
use continuum_repair::neighborhood::{
    self, BoundKind, Budget, Caps, CausalCore, Config, CoreError, CoreOrder, Disposition, Edit,
    Envelope, EventClass, Execution, Exploration, NeighborhoodRecord, NotRun, Profile,
    ProjectionRefusal, Realized, Rejection, ResumeError, Spent, Strategy, StrategyStatus,
    Substrate, Verdict, Witness,
};
use continuum_value::assurance::InconclusiveReason;

/// SplitMix64 for the random cores (the seed is in each test).
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }
    fn below(&mut self, n: u64) -> u64 {
        self.next() % n.max(1)
    }
}

/// A random core: `n` events over three actors, program order plus some cross-actor
/// hard edges, random conflicts, a random core subset, some sends and one crash.
fn random_core(seed: u64) -> (CoreOrder, (usize, usize)) {
    let mut rng = Rng(seed);
    let n = 6 + usize::try_from(rng.below(9)).expect("small");
    let actor: Vec<u64> = (0..n).map(|_| rng.below(3)).collect();
    let mut hard = vec![Vec::new(); n];
    let mut conflicts = Vec::new();
    for e in 0..n {
        if let Some(p) = (0..e).rev().find(|&p| actor[p] == actor[e]) {
            hard[e].push(p);
        }
        for p in 0..e {
            if actor[p] != actor[e] {
                match rng.below(10) {
                    0 => hard[e].push(p),
                    1..=3 => conflicts.push((p, e)),
                    _ => {}
                }
            }
        }
    }
    let mut core: Vec<usize> = (0..n).filter(|_| rng.below(3) != 0).collect();
    if core.is_empty() {
        core.push(n - 1);
    }
    let mut class = BTreeMap::new();
    for e in 0..n {
        match rng.below(5) {
            0 => {
                class.insert(
                    e,
                    EventClass::Send {
                        channel: rng.below(2),
                    },
                );
            }
            1 if !class.values().any(|c| matches!(c, EventClass::Fault(_))) => {
                class.insert(e, EventClass::Fault(FaultClass::Crash));
            }
            _ => {}
        }
    }
    // The toy property: it fails while the first conflicting pair keeps its core order.
    let pair = conflicts.first().copied().unwrap_or((0, n - 1));
    (
        CoreOrder::new(hard, &conflicts, &core, &class).expect("a core"),
        pair,
    )
}

/// A toy program: a run is a schedule; it fails while `pair.0` runs before `pair.1`.
struct Toy {
    core: CoreOrder,
    pair: (usize, usize),
    scenarios: Vec<(String, u32, Profile)>,
    realized: Vec<Edit<u32>>,
    executed: usize,
    bad_handle: bool,
}

impl Toy {
    fn new(seed: u64) -> Self {
        let (core, pair) = random_core(seed);
        let crash = |n: u32| {
            Profile {
                cancellations: n,
                nodes: 3,
                max_value: Some(1),
                ..Profile::default()
            }
            .with_fault(FaultClass::Crash, n)
            .expect("fits")
        };
        Self {
            core,
            pair,
            scenarios: vec![
                ("one-crash".to_owned(), 1, crash(1)),
                ("two-crashes".to_owned(), 2, crash(2)),
                ("three-crashes".to_owned(), 3, crash(3)),
                ("one-crash".to_owned(), 1, crash(1)),
            ],
            realized: Vec::new(),
            executed: 0,
            bad_handle: false,
        }
    }
}

impl Substrate for Toy {
    type Core = CoreOrder;
    type Scenario = u32;
    type Run = Vec<usize>;

    fn core(&self) -> &CoreOrder {
        &self.core
    }
    fn core_identity(&self) -> String {
        format!("toy-core-{}", self.core.len())
    }
    fn subject(&self) -> String {
        "toy".to_owned()
    }
    fn engine_identity(&self) -> String {
        "toy-engine/1".to_owned()
    }
    fn scenario_bytes(&self, s: &u32) -> Vec<u8> {
        s.to_be_bytes().to_vec()
    }
    fn core_profile(&self) -> Profile {
        Profile {
            nodes: 3,
            max_value: Some(1),
            ..Profile::default()
        }
    }
    fn not_run(&self, strategy: Strategy) -> Option<NotRun> {
        (strategy == Strategy::HiddenCorpusMutations).then(|| NotRun("no corpus".to_owned()))
    }
    fn scenario_count(&self, strategy: Strategy) -> usize {
        if strategy == Strategy::FaultWindow {
            self.scenarios.len()
        } else {
            0
        }
    }
    fn scenario(&self, _: Strategy, index: usize) -> Option<(String, u32, Profile)> {
        self.scenarios.get(index).cloned()
    }
    fn realize(&mut self, edit: &Edit<u32>) -> Realized<Vec<usize>> {
        self.realized.push(edit.clone());
        match edit {
            Edit::Schedule(order) => Realized::Run {
                run: order.clone(),
                witness: Witness::Schedule {
                    taken: order.clone(),
                    absent: vec![],
                    halted: false,
                },
            },
            Edit::Message { .. } => Realized::Unsupported("no network".to_owned()),
            Edit::Scenario(3) => Realized::NotRealizable("deadlock".to_owned()),
            Edit::Scenario(x) => Realized::Run {
                run: (0..self.core.len()).collect(),
                witness: Witness::Edit(x.to_be_bytes().to_vec()),
            },
        }
    }
    fn execute(&mut self, run: &Vec<usize>) -> Execution {
        self.executed += 1;
        let pos = |e: usize| run.iter().position(|x| *x == e).expect("a permutation");
        let tag: String = run.iter().map(|e| format!("{e:x}")).collect();
        let handle = if self.bad_handle {
            "no handle".to_owned()
        } else {
            format!("ev_{tag}")
        };
        if pos(self.pair.0) < pos(self.pair.1) {
            Execution::Fail {
                property: "toy".to_owned(),
                run: handle,
            }
        } else {
            Execution::Pass { run: handle }
        }
    }
}

fn envelope() -> Envelope {
    Envelope::new(
        FaultModel::new(FaultClass::ALL, []).expect("model"),
        Bounds::new(
            ExplorationBound::Bounded(2),
            DeclaredBound::Declared(3),
            DeclaredBound::Declared(3),
            ExplorationBound::Unbounded,
        )
        .expect("bounds"),
        Caps {
            crashes: Some(2),
            cancellations: Some(3),
            partitions: Some(1),
        },
    )
}

fn config(seed: u64) -> Config {
    Config {
        seed,
        strategies: Strategy::ALL.to_vec(),
        walks: 4,
        walk_length: 5,
        max_delay: 2,
        budget: Budget {
            candidates: 10_000,
            runs: 10_000,
            work: 1 << 30,
        },
        transaction: "rt_toy".to_owned(),
    }
}

fn explore(toy: &mut Toy, cfg: &Config) -> NeighborhoodRecord {
    neighborhood::explore(toy, &envelope(), cfg)
        .expect("a campaign")
        .into_record()
}

/// The strategies the toy supports in full: no not-run strategy, no unsupported
/// message fault.
fn supported(seed: u64) -> Config {
    let mut c = config(seed);
    c.strategies = vec![
        Strategy::AlternateEnabledEvents,
        Strategy::FaultWindow,
        Strategy::SchedulePerturbation,
    ];
    c
}

/// The projection a verifier makes: against the inputs a fresh toy of `seed`, the
/// envelope and the record's configuration give now.
fn project(
    r: &NeighborhoodRecord,
    seed: u64,
) -> Result<continuum_repair::neighborhood::intent::Json, ProjectionRefusal> {
    r.receipt_coverage(&Toy::new(seed), &envelope(), r.config())
}

fn hard_ok(core: &CoreOrder, order: &[usize]) -> bool {
    let pos: Vec<usize> = (0..order.len())
        .map(|e| order.iter().position(|x| *x == e).expect("a permutation"))
        .collect();
    (0..core.len()).all(|e| core.hard_predecessors(e).iter().all(|&p| pos[p] < pos[e]))
}

fn reversed_conflicts(core: &CoreOrder, order: &[usize]) -> usize {
    let n = core.len();
    let pos: Vec<usize> = (0..n)
        .map(|e| order.iter().position(|x| *x == e).expect("a permutation"))
        .collect();
    let mut count = 0;
    for a in 0..n {
        for b in a + 1..n {
            let conflict = core.dependent(a, b) || core.hard_predecessors(b).contains(&a);
            if conflict && pos[a] > pos[b] {
                count += 1;
            }
        }
    }
    count
}

#[test]
fn valid_schedules_respect_the_hard_order_on_random_cores() {
    for seed in 0..64 {
        let mut toy = Toy::new(seed);
        let r = explore(&mut toy, &config(seed));
        // The toy has no network and no corpus: a gap, never Complete.
        assert_eq!(
            r.verdict(),
            Verdict::Inconclusive {
                reason: InconclusiveReason::Unsupported
            }
        );
        let schedules: Vec<(Strategy, Vec<usize>)> = r
            .neighbors()
            .iter()
            .filter(|n| !matches!(n.disposition, Disposition::Rejected(_)))
            .map(|n| n.strategy)
            .zip(toy.realized.iter())
            .filter_map(|(s, e)| match e {
                Edit::Schedule(o) => Some((s, o.clone())),
                _ => None,
            })
            .collect();
        for (s, order) in &schedules {
            assert!(hard_ok(&toy.core, order), "seed {seed}: {order:?}");
            match s {
                Strategy::SchedulePerturbation => {
                    assert_eq!(reversed_conflicts(&toy.core, order), 0, "stays in class");
                }
                Strategy::AlternateEnabledEvents => {
                    assert!(
                        reversed_conflicts(&toy.core, order) >= 1,
                        "crosses a decision"
                    );
                }
                _ => {}
            }
            // Property-directed: some core event moved.
            assert!(
                order
                    .iter()
                    .enumerate()
                    .any(|(k, &e)| k != e && toy.core.in_core(e))
            );
        }
    }
}

#[test]
fn rejected_candidates_never_reach_the_substrate() {
    for seed in 0..64 {
        let mut toy = Toy::new(seed);
        let r = explore(&mut toy, &config(seed));
        let valid: u64 = r
            .strategies()
            .iter()
            .filter_map(|(_, st)| match st {
                StrategyStatus::Ran(c) => Some(c.valid),
                StrategyStatus::NotRun(_) => None,
            })
            .sum();
        assert_eq!(u64::try_from(toy.realized.len()).expect("small"), valid);
        let fw = r.coverage(Strategy::FaultWindow).expect("ran");
        // Three crashes past the cap, and the repeated id, are rejected.
        assert_eq!(fw.rejected.get("exceeds-fault-bound"), Some(&1));
        assert_eq!(fw.rejected.get("duplicate"), Some(&1));
        assert_eq!(fw.valid, 2);
        // Message faults are valid and unsupported, never executed.
        let m = r
            .coverage(Strategy::MessageDuplicationLossDelay)
            .expect("ran");
        assert_eq!(m.valid, m.executed + m.unsupported);
        assert_eq!(
            u64::try_from(toy.executed).expect("small"),
            r.strategies()
                .iter()
                .filter_map(|(_, st)| match st {
                    StrategyStatus::Ran(c) => Some(c.executed),
                    StrategyStatus::NotRun(_) => None,
                })
                .sum::<u64>()
        );
    }
}

#[test]
fn identical_inputs_give_identical_records_and_the_seed_moves_only_walks() {
    for seed in 0..16 {
        let a = explore(&mut Toy::new(seed), &config(seed));
        let b = explore(&mut Toy::new(seed), &config(seed));
        assert_eq!(a.canonical_bytes(), b.canonical_bytes());
        let parsed = Json::parse(&a.canonical_bytes()).expect("canonical JSON");
        assert_eq!(parsed.to_canonical_bytes(), a.canonical_bytes());
        let c = explore(&mut Toy::new(seed), &config(seed ^ 0xabcdef));
        for (x, y) in a.neighbors().iter().zip(c.neighbors()) {
            if x != y {
                assert!(x.id.contains(":walk("), "{} moved with the seed", x.id);
            }
        }
    }
}

#[test]
fn an_exhausted_budget_suspends_and_resumes_exactly() {
    // Every run-budget boundary and a sweep of work-budget boundaries: the campaign is
    // Pending with a continuation, never projected, and the continuation, serialized and
    // decoded, resumes to the uninterrupted result byte for byte, without re-running a
    // committed neighbor.
    for seed in [7, 11] {
        let full = explore(&mut Toy::new(seed), &config(seed));
        let full_runs = full.spent().runs;
        let full_work = full.spent().work;
        let mut budgets: Vec<(u64, u64)> = (0..=full_runs).map(|r| (r, 1 << 30)).collect();
        budgets.extend(
            (0..=full_work)
                .step_by(usize::try_from(full_work / 50 + 1).expect("small"))
                .map(|w| (10_000, w)),
        );
        for (runs, work) in budgets {
            let mut cfg = config(seed);
            cfg.budget.runs = runs;
            cfg.budget.work = work;
            let mut toy = Toy::new(seed);
            let first = match neighborhood::explore(&mut toy, &envelope(), &cfg) {
                Err(CoreError::CoreCheckExhausted) => continue,
                Err(e) => panic!("{e:?}"),
                Ok(x) => x,
            };
            assert!(first.record().spent().runs <= runs);
            assert!(first.record().spent().work <= work);
            let Exploration::Suspended {
                record,
                continuation,
            } = first
            else {
                assert!(
                    runs >= full_runs && work >= full_work,
                    "runs {runs} work {work}"
                );
                continue;
            };
            assert_eq!(record.verdict(), Verdict::Pending);
            assert_eq!(
                project(&record, seed),
                Err(ProjectionRefusal::Pending),
                "a pending campaign is never projected"
            );
            let frontiers = record
                .strategies()
                .iter()
                .filter(|(_, st)| matches!(st, StrategyStatus::Ran(c) if c.frontier.is_some()))
                .count();
            assert!(frontiers >= 1);
            // Resume the in-process continuation with the full budget.
            let mut toy2 = Toy::new(seed);
            let resumed = neighborhood::resume(
                &mut toy2,
                &envelope(),
                &config(seed),
                &continuation,
                &mut Spent::default(),
            )
            .expect("resumes");
            // The bytes, from outside, are only checked against a fresh derivation.
            let checked = neighborhood::resume_untrusted(
                &mut Toy::new(seed),
                &envelope(),
                &config(seed),
                &continuation.canonical_bytes(),
                &mut Spent::default(),
            )
            .expect("genuine bytes check out");
            assert_eq!(checked.record().result_bytes(), full.result_bytes());
            let Exploration::Finished(done) = resumed else {
                panic!("the full budget finishes");
            };
            assert_eq!(
                done.result_bytes(),
                full.result_bytes(),
                "runs {runs} work {work}"
            );
            // No committed neighbor was realized again.
            let committed_runs = usize::try_from(record.spent().runs).expect("small");
            assert_eq!(
                toy2.realized.len(),
                usize::try_from(full.spent().runs).expect("small") - committed_runs
            );
        }
    }
}

#[test]
fn chained_resumes_advance_monotonically_to_the_same_result() {
    let seed = 7;
    let full = explore(&mut Toy::new(seed), &config(seed));
    let mut cfg = config(seed);
    cfg.budget.runs = 0;
    let mut toy = Toy::new(seed);
    let mut outcome = neighborhood::explore(&mut toy, &envelope(), &cfg).expect("ran");
    let mut last = (0_u64, 0_u64);
    let mut steps = 0;
    while let Exploration::Suspended { continuation, .. } = outcome {
        let (s, i) = continuation.frontier();
        assert!((s.index(), i) >= last, "the frontier never moves back");
        last = (s.index(), i);
        cfg.budget.runs += 3;
        outcome = neighborhood::resume(
            &mut toy,
            &envelope(),
            &cfg,
            &continuation,
            &mut Spent::default(),
        )
        .expect("resumes");
        steps += 1;
        assert!(steps < 1_000);
    }
    let Exploration::Finished(done) = outcome else {
        unreachable!()
    };
    assert_eq!(done.result_bytes(), full.result_bytes());
    assert_eq!(
        toy.realized.len(),
        usize::try_from(full.spent().runs).expect("small")
    );
    // A budget below the committed spend suspends again at the same frontier.
    let mut small = config(seed);
    small.budget.runs = 2;
    let Exploration::Suspended { continuation, .. } =
        neighborhood::explore(&mut Toy::new(seed), &envelope(), &small).expect("ran")
    else {
        panic!("suspends");
    };
    let again = neighborhood::resume(
        &mut Toy::new(seed),
        &envelope(),
        &small,
        &continuation,
        &mut Spent::default(),
    )
    .expect("resumes");
    let Exploration::Suspended {
        continuation: c2, ..
    } = again
    else {
        panic!("still suspended");
    };
    assert_eq!(c2.frontier(), continuation.frontier());
}

/// A suspended toy campaign with a committed failing prefix, and its bytes.
fn suspended_with_a_failure(seed: u64) -> (NeighborhoodRecord, Vec<u8>) {
    for runs in 1..64 {
        let mut cfg = config(seed);
        cfg.budget.runs = runs;
        if let Exploration::Suspended {
            record,
            continuation,
        } = neighborhood::explore(&mut Toy::new(seed), &envelope(), &cfg).expect("ran")
            && !record.failing().is_empty()
        {
            return ((*record).clone(), continuation.canonical_bytes());
        }
    }
    panic!("no suspended prefix with a failure");
}

/// Rewrite one committed failure into a pass in `bytes`, with every redundant field
/// (the strategy's passed and failed counts, the failing list) edited to agree.
fn coherent_forgery(bytes: &[u8]) -> Vec<u8> {
    let mut doc = Json::parse(bytes).expect("json");
    let Json::Object(top) = &mut doc else {
        panic!()
    };
    let Some(Json::Object(rec)) = top.get_mut("record") else {
        panic!()
    };
    let Some(Json::Array(fails)) = rec.get_mut("failing_neighbors") else {
        panic!()
    };
    let victim = fails.remove(0);
    let v = victim.as_object().expect("entry");
    let id = v["neighbor"].as_str().expect("id").to_owned();
    let strategy = v["strategy"].as_str().expect("s").to_owned();
    let Some(Json::Array(ns)) = rec.get_mut("neighbors") else {
        panic!()
    };
    for n in ns.iter_mut() {
        let Json::Object(n) = n else { panic!() };
        if n["neighbor"].as_str() == Some(id.as_str()) {
            n.insert("disposition".to_owned(), Json::String("pass".to_owned()));
            n.remove("detail");
        }
    }
    let Some(Json::Array(ss)) = rec.get_mut("strategies") else {
        panic!()
    };
    for st in ss.iter_mut() {
        let Json::Object(st) = st else { panic!() };
        if st["strategy"].as_str() == Some(strategy.as_str()) {
            let Json::Integer(f) = st["failed"] else {
                panic!()
            };
            let Json::Integer(p) = st["passed"] else {
                panic!()
            };
            st.insert("failed".to_owned(), Json::Integer(f - 1));
            st.insert("passed".to_owned(), Json::Integer(p + 1));
        }
    }
    doc.to_canonical_bytes()
}

#[test]
fn a_foreign_or_forged_continuation_is_refused() {
    let seed = 7;
    let (_, bytes) = suspended_with_a_failure(seed);
    // Other inputs: the configuration, the core, the engine, the transaction.
    let mut other = config(seed);
    other.seed ^= 1;
    assert_eq!(
        neighborhood::resume_untrusted(
            &mut Toy::new(seed),
            &envelope(),
            &other,
            &bytes,
            &mut Spent::default()
        )
        .err(),
        Some(ResumeError::Mismatch("config".to_owned()))
    );
    assert!(matches!(
        neighborhood::resume_untrusted(
            &mut Toy::new(seed + 1),
            &envelope(),
            &config(seed),
            &bytes,
            &mut Spent::default()
        )
        .err(),
        Some(ResumeError::Mismatch(_))
    ));
    let mut moved = config(seed);
    moved.transaction = "rt_another".to_owned();
    assert_eq!(
        neighborhood::resume_untrusted(
            &mut Toy::new(seed),
            &envelope(),
            &moved,
            &bytes,
            &mut Spent::default()
        )
        .err(),
        Some(ResumeError::Mismatch("transaction".to_owned()))
    );
    // Truncated bytes are malformed, never a panic.
    for cut in [0, 1, bytes.len() / 2, bytes.len() - 1] {
        assert!(matches!(
            neighborhood::resume_untrusted(
                &mut Toy::new(seed),
                &envelope(),
                &config(seed),
                &bytes[..cut],
                &mut Spent::default()
            ),
            Err(ResumeError::Malformed(_))
        ));
    }
    // One redundant field edited alone is malformed.
    let text = String::from_utf8(bytes.clone()).expect("utf-8");
    let doctored = text.replacen("\"disposition\":\"fail\"", "\"disposition\":\"pass\"", 1);
    assert_ne!(doctored, text);
    assert!(matches!(
        neighborhood::resume_untrusted(
            &mut Toy::new(seed),
            &envelope(),
            &config(seed),
            doctored.as_bytes(),
            &mut Spent::default()
        ),
        Err(ResumeError::Malformed(_))
    ));
    // A coherent forgery (a failure rewritten into a pass with every count and the
    // failing list edited to agree) reconciles, and the fresh derivation refuses it.
    let forged = coherent_forgery(&bytes);
    assert!(matches!(
        neighborhood::resume_untrusted(
            &mut Toy::new(seed),
            &envelope(),
            &config(seed),
            &forged,
            &mut Spent::default()
        ),
        Err(ResumeError::Forged { .. })
    ));
    let mut tiny = config(seed);
    tiny.budget.work = 3;
    assert_eq!(
        neighborhood::resume_untrusted(
            &mut Toy::new(seed),
            &envelope(),
            &tiny,
            &bytes,
            &mut Spent::default()
        )
        .err(),
        Some(ResumeError::Exhausted)
    );
}

#[test]
fn hostile_counts_fail_closed_not_a_panic() {
    // Counts at and past the u64 sum boundary, in an otherwise canonical continuation.
    let (_, bytes) = suspended_with_a_failure(7);
    for big in [i64::MAX, i64::MAX - 1, 1 << 62] {
        let mut doc = Json::parse(&bytes).expect("json");
        let Json::Object(top) = &mut doc else {
            panic!()
        };
        let Some(Json::Object(rec)) = top.get_mut("record") else {
            panic!()
        };
        let Some(Json::Array(ss)) = rec.get_mut("strategies") else {
            panic!()
        };
        for st in ss.iter_mut() {
            let Json::Object(st) = st else { panic!() };
            if let Some(Json::Object(rej)) = st.get_mut("rejected") {
                for t in [
                    "duplicate",
                    "malformed-schedule",
                    "violates-causal-order",
                    "leaves-trace-class",
                ] {
                    rej.insert(t.to_owned(), Json::Integer(big));
                }
            }
            if let Some(Json::Object(nr)) = st.get_mut("not_realizable") {
                for t in ["a", "b", "c"] {
                    nr.insert(t.to_owned(), Json::Integer(big));
                }
            }
        }
        let hostile = doc.to_canonical_bytes();
        assert!(matches!(
            neighborhood::resume_untrusted(
                &mut Toy::new(7),
                &envelope(),
                &config(7),
                &hostile,
                &mut Spent::default()
            ),
            Err(ResumeError::Malformed(_))
        ));
    }
}

#[test]
fn a_stale_record_or_continuation_is_refused() {
    struct Engine(Toy, &'static str, bool);
    impl Substrate for Engine {
        type Core = CoreOrder;
        type Scenario = u32;
        type Run = Vec<usize>;
        fn core(&self) -> &CoreOrder {
            self.0.core()
        }
        fn core_identity(&self) -> String {
            self.0.core_identity()
        }
        fn subject(&self) -> String {
            self.0.subject()
        }
        fn engine_identity(&self) -> String {
            self.1.to_owned()
        }
        fn scenario_bytes(&self, s: &u32) -> Vec<u8> {
            // A same-identity scenario whose edit changed.
            let mut b = self.0.scenario_bytes(s);
            if self.2 {
                b.push(1);
            }
            b
        }
        fn core_profile(&self) -> Profile {
            self.0.core_profile()
        }
        fn not_run(&self, s: Strategy) -> Option<NotRun> {
            self.0.not_run(s)
        }
        fn scenario_count(&self, s: Strategy) -> usize {
            self.0.scenario_count(s)
        }
        fn scenario(&self, s: Strategy, i: usize) -> Option<(String, u32, Profile)> {
            self.0.scenario(s, i)
        }
        fn realize(&mut self, e: &Edit<u32>) -> Realized<Vec<usize>> {
            self.0.realize(e)
        }
        fn execute(&mut self, r: &Vec<usize>) -> Execution {
            self.0.execute(r)
        }
    }
    let seed = 7;
    let cfg = supported(seed);
    let r = neighborhood::explore(&mut Engine(Toy::new(seed), "e1", false), &envelope(), &cfg)
        .expect("ran")
        .into_record();
    // The projection is bound to the inputs it was computed for, derived by the
    // verifier from the live substrate, envelope and configuration.
    let e = |engine: &'static str| Engine(Toy::new(seed), engine, false);
    if r.verdict() == Verdict::Complete {
        assert!(r.receipt_coverage(&e("e1"), &envelope(), &cfg).is_ok());
    }
    assert_eq!(
        r.receipt_coverage(&e("e2"), &envelope(), &cfg),
        Err(ProjectionRefusal::Stale {
            field: "engine".to_owned()
        })
    );
    let mut moved = cfg.clone();
    moved.transaction = "rt_other".to_owned();
    assert_eq!(
        r.receipt_coverage(&e("e1"), &envelope(), &moved),
        Err(ProjectionRefusal::Stale {
            field: "transaction".to_owned()
        })
    );
    // Another core under the same identity string is stale too.
    assert_eq!(
        r.receipt_coverage(&Engine(Toy::new(seed + 1), "e1", false), &envelope(), &cfg)
            .err()
            .map(|x| matches!(x, ProjectionRefusal::Stale { .. })),
        Some(true)
    );
    // A continuation resumes only under its engine, and a changed scenario edit under
    // an unchanged identity diverges.
    let mut small = cfg.clone();
    small.strategies = vec![Strategy::FaultWindow, Strategy::SchedulePerturbation];
    small.budget.runs = 3;
    let Exploration::Suspended { continuation, .. } = neighborhood::explore(
        &mut Engine(Toy::new(seed), "e1", false),
        &envelope(),
        &small,
    )
    .expect("ran") else {
        panic!("suspends");
    };
    let mut full = small.clone();
    full.budget.runs = 10_000;
    assert_eq!(
        neighborhood::resume(
            &mut Engine(Toy::new(seed), "e2", false),
            &envelope(),
            &full,
            &continuation,
            &mut Spent::default()
        )
        .err(),
        Some(ResumeError::Mismatch("engine".to_owned()))
    );
    assert_eq!(
        neighborhood::resume(
            &mut Engine(Toy::new(seed), "e1", true),
            &envelope(),
            &full,
            &continuation,
            &mut Spent::default()
        )
        .err(),
        Some(ResumeError::Diverged)
    );
    assert!(
        neighborhood::resume(
            &mut Engine(Toy::new(seed), "e1", false),
            &envelope(),
            &full,
            &continuation,
            &mut Spent::default()
        )
        .is_ok()
    );
}

#[test]
fn a_pending_campaign_with_an_undecided_prefix_stays_pending() {
    // An inconclusive run before the budget runs out: the record is Pending and its
    // projection says so, never a terminal InconclusiveRuns.
    struct Flaky(Toy);
    impl Substrate for Flaky {
        type Core = CoreOrder;
        type Scenario = u32;
        type Run = Vec<usize>;
        fn core(&self) -> &CoreOrder {
            self.0.core()
        }
        fn core_identity(&self) -> String {
            self.0.core_identity()
        }
        fn subject(&self) -> String {
            self.0.subject()
        }
        fn engine_identity(&self) -> String {
            self.0.engine_identity()
        }
        fn scenario_bytes(&self, s: &u32) -> Vec<u8> {
            self.0.scenario_bytes(s)
        }
        fn core_profile(&self) -> Profile {
            self.0.core_profile()
        }
        fn not_run(&self, s: Strategy) -> Option<NotRun> {
            self.0.not_run(s)
        }
        fn scenario_count(&self, s: Strategy) -> usize {
            self.0.scenario_count(s)
        }
        fn scenario(&self, s: Strategy, i: usize) -> Option<(String, u32, Profile)> {
            self.0.scenario(s, i)
        }
        fn realize(&mut self, e: &Edit<u32>) -> Realized<Vec<usize>> {
            self.0.realize(e)
        }
        fn execute(&mut self, _: &Vec<usize>) -> Execution {
            Execution::Inconclusive {
                reason: InconclusiveReason::InsufficientTelemetry,
            }
        }
    }
    let mut cfg = supported(7);
    cfg.budget.runs = 2;
    let r = neighborhood::explore(&mut Flaky(Toy::new(7)), &envelope(), &cfg)
        .expect("ran")
        .into_record();
    assert_eq!(r.verdict(), Verdict::Pending);
    assert_eq!(project(&r, 7), Err(ProjectionRefusal::Pending));
}

#[test]
fn a_long_scenario_identity_that_exhausts_the_budget_takes_its_charge_back() {
    struct Long(Toy);
    impl Substrate for Long {
        type Core = CoreOrder;
        type Scenario = u32;
        type Run = Vec<usize>;
        fn core(&self) -> &CoreOrder {
            self.0.core()
        }
        fn core_identity(&self) -> String {
            self.0.core_identity()
        }
        fn subject(&self) -> String {
            self.0.subject()
        }
        fn engine_identity(&self) -> String {
            self.0.engine_identity()
        }
        fn scenario_bytes(&self, s: &u32) -> Vec<u8> {
            self.0.scenario_bytes(s)
        }
        fn core_profile(&self) -> Profile {
            self.0.core_profile()
        }
        fn not_run(&self, s: Strategy) -> Option<NotRun> {
            self.0.not_run(s)
        }
        fn scenario_count(&self, s: Strategy) -> usize {
            self.0.scenario_count(s)
        }
        fn scenario(&self, s: Strategy, i: usize) -> Option<(String, u32, Profile)> {
            self.0
                .scenario(s, i)
                .map(|(id, e, p)| (format!("{id}{}", "x".repeat(100_000)), e, p))
        }
        fn realize(&mut self, e: &Edit<u32>) -> Realized<Vec<usize>> {
            self.0.realize(e)
        }
        fn execute(&mut self, r: &Vec<usize>) -> Execution {
            self.0.execute(r)
        }
    }
    let mut cfg = config(7);
    cfg.strategies = vec![Strategy::FaultWindow];
    let full = neighborhood::explore(&mut Long(Toy::new(7)), &envelope(), &cfg)
        .expect("ran")
        .into_record();
    // A work budget that pays for the core and the first candidate charge but not the
    // identity: nothing is committed and no candidate is left charged.
    for work in [
        full.spent().work / 3,
        full.spent().work / 2,
        full.spent().work - 1,
    ] {
        let mut c = cfg.clone();
        c.budget.work = work;
        let Ok(Exploration::Suspended {
            record,
            continuation,
        }) = neighborhood::explore(&mut Long(Toy::new(7)), &envelope(), &c)
        else {
            continue;
        };
        let generated: u64 = record
            .strategies()
            .iter()
            .filter_map(|(_, st)| match st {
                StrategyStatus::Ran(cv) => Some(cv.generated),
                StrategyStatus::NotRun(_) => None,
            })
            .sum();
        assert_eq!(record.spent().candidates, generated, "work {work}");
        // Resuming needs no larger candidate ceiling than the uninterrupted run.
        let mut again = cfg.clone();
        again.budget.candidates = full.spent().candidates;
        let done = neighborhood::resume(
            &mut Long(Toy::new(7)),
            &envelope(),
            &again,
            &continuation,
            &mut Spent::default(),
        )
        .expect("resumes");
        assert_eq!(done.record().result_bytes(), full.result_bytes());
    }
}

#[test]
fn coverage_gaps_are_never_projected_as_complete() {
    let unsupported = Verdict::Inconclusive {
        reason: InconclusiveReason::Unsupported,
    };
    let refused = Err(ProjectionRefusal::Inconclusive {
        reason: InconclusiveReason::Unsupported,
    });
    // Empty selection.
    let mut c = config(3);
    c.strategies = vec![];
    let r = explore(&mut Toy::new(3), &c);
    assert_eq!(r.verdict(), unsupported);
    assert_eq!(project(&r, 3), refused);
    // Every selected strategy not run.
    c.strategies = vec![Strategy::HiddenCorpusMutations];
    let r = explore(&mut Toy::new(3), &c);
    assert_eq!(r.verdict(), unsupported);
    assert_eq!(project(&r, 3), refused);
    // Every valid neighbor unsupported: message faults with no delays.
    c.strategies = vec![Strategy::MessageDuplicationLossDelay];
    c.max_delay = 0;
    for seed in 0..16 {
        let r = explore(&mut Toy::new(seed), &c);
        assert_eq!(r.verdict(), unsupported, "seed {seed}");
        assert_eq!(project(&r, seed), refused);
    }
    // Mixed: delays executed, duplication and loss unsupported.
    c.max_delay = 2;
    for seed in 0..16 {
        let r = explore(&mut Toy::new(seed), &c);
        let m = r
            .coverage(Strategy::MessageDuplicationLossDelay)
            .expect("ran");
        if m.unsupported > 0 {
            assert_eq!(r.verdict(), unsupported, "seed {seed}");
            assert_eq!(project(&r, seed), refused);
        }
    }
    // A selected strategy that generates nothing (the toy has no abstraction-map
    // edits and does not say it cannot run them) is a gap, not a vacuous completion.
    let mut c = supported(3);
    c.strategies.push(Strategy::AbstractionMapVariants);
    let r = explore(&mut Toy::new(3), &c);
    assert_eq!(
        r.coverage(Strategy::AbstractionMapVariants)
            .map(|cv| cv.generated),
        Some(0)
    );
    assert_eq!(r.verdict(), unsupported);
    assert_eq!(project(&r, 3), refused);
    // One supported strategy next to a not-run one.
    let mut c = supported(3);
    c.strategies.push(Strategy::HiddenCorpusMutations);
    let r = explore(&mut Toy::new(3), &c);
    assert_eq!(r.verdict(), unsupported);
    assert_eq!(project(&r, 3), refused);
    // The fully supported profile is Complete and projects.
    for seed in 0..16 {
        let r = explore(&mut Toy::new(seed), &supported(seed));
        if r.verdict() == Verdict::Complete {
            assert!(project(&r, seed).is_ok());
        } else {
            assert_eq!(r.verdict(), unsupported, "seed {seed}: nothing executed");
        }
    }
}

#[test]
fn core_refusals_are_typed() {
    let env = envelope();
    let cfg = config(1);
    let mut toy = Toy::new(1);
    toy.core = CoreOrder::new(vec![vec![], vec![1]], &[], &[0], &BTreeMap::new()).expect("a core");
    assert_eq!(
        neighborhood::explore(&mut toy, &env, &cfg).err(),
        Some(CoreError::PredecessorNotEarlier {
            event: 1,
            predecessor: 1
        })
    );
    toy.core = CoreOrder::new(vec![vec![], vec![0]], &[], &[], &BTreeMap::new()).expect("a core");
    assert_eq!(
        neighborhood::explore(&mut toy, &env, &cfg).err(),
        Some(CoreError::NoCoreEvent)
    );
    assert_eq!(
        CoreOrder::new(
            vec![Vec::new(); neighborhood::MAX_CORE_EVENTS + 1],
            &[],
            &[0],
            &BTreeMap::new(),
        )
        .err(),
        Some(CoreError::TooLarge {
            len: neighborhood::MAX_CORE_EVENTS + 1
        })
    );
    // Entries out of range are refused, never dropped.
    let empty = BTreeMap::new();
    for (conflicts, core, class) in [
        (vec![(0, 5)], vec![0], empty.clone()),
        (vec![(1, 1)], vec![0], empty.clone()),
        (vec![], vec![9], empty.clone()),
        (vec![], vec![0], BTreeMap::from([(3, EventClass::Step)])),
    ] {
        assert!(matches!(
            CoreOrder::new(vec![vec![], vec![0]], &conflicts, &core, &class),
            Err(CoreError::EntryOutOfRange { .. })
        ));
    }
    let mut big = cfg.clone();
    big.budget.work = u64::MAX;
    assert_eq!(
        neighborhood::explore(&mut Toy::new(1), &env, &big).err(),
        Some(CoreError::BudgetTooLarge)
    );
    let mut none = cfg.clone();
    none.budget.work = 0;
    assert_eq!(
        neighborhood::explore(&mut Toy::new(1), &env, &none).err(),
        Some(CoreError::CoreCheckExhausted)
    );
    // A core outside its own envelope is refused, not explored.
    struct Wide(Toy);
    impl Substrate for Wide {
        type Core = CoreOrder;
        type Scenario = u32;
        type Run = Vec<usize>;
        fn core(&self) -> &CoreOrder {
            self.0.core()
        }
        fn core_identity(&self) -> String {
            self.0.core_identity()
        }
        fn subject(&self) -> String {
            self.0.subject()
        }
        fn engine_identity(&self) -> String {
            self.0.engine_identity()
        }
        fn scenario_bytes(&self, s: &u32) -> Vec<u8> {
            self.0.scenario_bytes(s)
        }
        fn core_profile(&self) -> Profile {
            Profile {
                nodes: 4,
                ..Profile::default()
            }
        }
        fn not_run(&self, s: Strategy) -> Option<NotRun> {
            self.0.not_run(s)
        }
        fn scenario_count(&self, s: Strategy) -> usize {
            self.0.scenario_count(s)
        }
        fn scenario(&self, s: Strategy, i: usize) -> Option<(String, u32, Profile)> {
            self.0.scenario(s, i)
        }
        fn realize(&mut self, e: &Edit<u32>) -> Realized<Vec<usize>> {
            self.0.realize(e)
        }
        fn execute(&mut self, r: &Vec<usize>) -> Execution {
            self.0.execute(r)
        }
    }
    assert_eq!(
        neighborhood::explore(&mut Wide(Toy::new(1)), &env, &cfg).err(),
        Some(CoreError::CoreOutsideEnvelope(
            Rejection::OutsideNodeDomain { nodes: 4, bound: 3 }
        ))
    );
}

#[test]
fn the_envelope_types_each_profile() {
    let env = envelope();
    let base = Profile {
        nodes: 3,
        max_value: Some(1),
        ..Profile::default()
    };
    assert_eq!(env.check(&base), Ok(()));
    let crash = |n| base.clone().with_fault(FaultClass::Crash, n).expect("fits");
    assert_eq!(env.check(&crash(2)), Ok(()));
    assert_eq!(
        env.check(&crash(3)),
        Err(Rejection::ExceedsFaultBound {
            what: BoundKind::Crashes,
            count: 3,
            bound: 2
        })
    );
    // Recovery completes a crash and is not a fault of its own under `faults`.
    assert_eq!(
        env.check(
            &crash(2)
                .with_fault(FaultClass::Recovery, 2)
                .expect("fits")
                .with_fault(FaultClass::Delay, 1)
                .expect("fits")
        ),
        Ok(())
    );
    assert_eq!(
        env.check(
            &crash(2)
                .with_fault(FaultClass::Delay, 1)
                .expect("fits")
                .with_fault(FaultClass::Loss, 1)
                .expect("fits")
        ),
        Err(Rejection::ExceedsFaultBound {
            what: BoundKind::Faults,
            count: 4,
            bound: 3
        })
    );
    assert_eq!(
        env.check(
            &base
                .clone()
                .with_fault(FaultClass::Partition, 2)
                .expect("fits")
        ),
        Err(Rejection::ExceedsFaultBound {
            what: BoundKind::Partitions,
            count: 2,
            bound: 1
        })
    );
    let mut cancels = base.clone();
    cancels.cancellations = 4;
    assert_eq!(
        env.check(&cancels),
        Err(Rejection::ExceedsFaultBound {
            what: BoundKind::Cancellations,
            count: 4,
            bound: 3
        })
    );
    let mut value = base.clone();
    value.max_value = Some(2);
    assert_eq!(
        env.check(&value),
        Err(Rejection::OutsideValueDomain { value: 2, bound: 2 })
    );
    let narrow = Envelope::new(
        FaultModel::new([FaultClass::Crash], []).expect("model"),
        Bounds::new(
            ExplorationBound::Unbounded,
            DeclaredBound::Undeclared,
            DeclaredBound::Undeclared,
            ExplorationBound::Unbounded,
        )
        .expect("bounds"),
        Caps::default(),
    );
    assert_eq!(
        narrow.check(&base.clone().with_fault(FaultClass::Loss, 1).expect("fits")),
        Err(Rejection::OutsideFaultModel {
            class: FaultClass::Loss
        })
    );
    // Undeclared bounds fail closed.
    assert_eq!(
        narrow.check(&crash(1)),
        Err(Rejection::BoundUndeclared {
            component: "faults"
        })
    );
    assert_eq!(
        narrow.check(&base),
        Err(Rejection::BoundUndeclared { component: "nodes" })
    );
}

#[test]
fn handles_are_checked_and_an_undisclosable_run_is_inconclusive() {
    for good in ["ev_1", "ev_abc-DEF_9", "rt_x_y", "a_b"] {
        assert!(neighborhood::is_handle(good), "{good}");
    }
    for bad in [
        "", "ev", "ev_", "_ev_1", "Ev_1", "ev 1", "ev_1 ", "e-v_1", "ev_a.b",
    ] {
        assert!(!neighborhood::is_handle(bad), "{bad}");
    }
    let mut toy = Toy::new(3);
    toy.bad_handle = true;
    let r = explore(&mut toy, &config(3));
    assert_eq!(
        r.verdict(),
        Verdict::Inconclusive {
            reason: InconclusiveReason::EngineError
        }
    );
    // The failing runs stay in the record as they came out; the projection that would
    // omit them is refused.
    let failed: u64 = r
        .strategies()
        .iter()
        .filter_map(|(_, st)| match st {
            StrategyStatus::Ran(c) => Some(c.failed),
            StrategyStatus::NotRun(_) => None,
        })
        .sum();
    assert_eq!(failed, u64::try_from(r.failing().len()).expect("small"));
    if r.failing().is_empty() {
        assert!(matches!(
            project(&r, 3),
            Err(ProjectionRefusal::Inconclusive { .. })
        ));
    } else {
        assert!(matches!(
            project(&r, 3),
            Err(ProjectionRefusal::UndisclosableRun { .. })
        ));
    }
    // A long near-handle is decided in linear time.
    let long = format!("a{}!", "_".repeat(200_000));
    assert!(!neighborhood::is_handle(&long));
    assert!(
        !neighborhood::is_handle(&format!("a{}x", "_".repeat(200_000))),
        "past the bound"
    );
    assert!(neighborhood::is_handle(&format!("a{}x", "_".repeat(200))));
}

#[test]
fn undecided_runs_and_silent_caps_are_never_rendered_as_passes() {
    // Every run inconclusive: the verdict says so and the projection is refused.
    struct Undecided(Toy);
    impl Substrate for Undecided {
        type Core = CoreOrder;
        type Scenario = u32;
        type Run = Vec<usize>;
        fn core(&self) -> &CoreOrder {
            self.0.core()
        }
        fn core_identity(&self) -> String {
            self.0.core_identity()
        }
        fn subject(&self) -> String {
            self.0.subject()
        }
        fn engine_identity(&self) -> String {
            self.0.engine_identity()
        }
        fn scenario_bytes(&self, s: &u32) -> Vec<u8> {
            self.0.scenario_bytes(s)
        }
        fn core_profile(&self) -> Profile {
            self.0.core_profile()
        }
        fn not_run(&self, s: Strategy) -> Option<NotRun> {
            self.0.not_run(s)
        }
        fn scenario_count(&self, s: Strategy) -> usize {
            // Declares more edits than it gives: a silent cap unless disclosed.
            self.0.scenario_count(s) * 10
        }
        fn scenario(&self, s: Strategy, i: usize) -> Option<(String, u32, Profile)> {
            self.0.scenario(s, i)
        }
        fn realize(&mut self, e: &Edit<u32>) -> Realized<Vec<usize>> {
            self.0.realize(e)
        }
        fn execute(&mut self, _: &Vec<usize>) -> Execution {
            Execution::Inconclusive {
                reason: InconclusiveReason::InsufficientTelemetry,
            }
        }
    }
    let mut cfg = config(5);
    cfg.strategies = vec![Strategy::SchedulePerturbation];
    let r = neighborhood::explore(&mut Undecided(Toy::new(5)), &envelope(), &cfg)
        .expect("ran")
        .into_record();
    assert_eq!(
        r.verdict(),
        Verdict::Inconclusive {
            reason: InconclusiveReason::InsufficientTelemetry
        }
    );
    assert!(matches!(
        project(&r, 5),
        Err(ProjectionRefusal::InconclusiveRuns { .. })
    ));
    let json = String::from_utf8(r.canonical_bytes()).expect("utf-8");
    assert!(
        json.contains("insufficient") || json.contains("Insufficient"),
        "{json}"
    );
    // The short scenario enumeration is disclosed at its frontier, never a silent cap.
    let mut cfg = config(5);
    cfg.strategies = vec![Strategy::FaultWindow];
    let r = neighborhood::explore(&mut Undecided(Toy::new(5)), &envelope(), &cfg)
        .expect("ran")
        .into_record();
    let fw = r.coverage(Strategy::FaultWindow).expect("ran");
    assert_eq!(fw.frontier.as_deref(), Some("fault-window#4"));
    assert_eq!(
        r.verdict(),
        Verdict::Inconclusive {
            reason: InconclusiveReason::EngineError
        }
    );
}

#[test]
fn a_malformed_core_is_rejected_not_a_panic() {
    let core = CoreOrder::new(vec![vec![], vec![7]], &[], &[0], &BTreeMap::new()).expect("lists");
    let got = neighborhood::check_candidate(
        &core,
        &envelope(),
        &Profile::default(),
        &Edit::<u32>::Schedule(vec![1, 0]),
        &Profile::default(),
    );
    assert_eq!(got, Err(Rejection::MalformedCore { event: 1 }));
    // Crashes count as cancellations under the cancellation cap.
    let env = Envelope::new(
        FaultModel::new(FaultClass::ALL, []).expect("model"),
        Bounds::new(
            ExplorationBound::Unbounded,
            DeclaredBound::Declared(3),
            DeclaredBound::Declared(3),
            ExplorationBound::Unbounded,
        )
        .expect("bounds"),
        Caps {
            cancellations: Some(1),
            ..Caps::default()
        },
    );
    let p = Profile {
        nodes: 1,
        ..Profile::default()
    }
    .with_fault(FaultClass::Crash, 2)
    .expect("fits");
    assert!(matches!(
        env.check(&p),
        Err(Rejection::ExceedsFaultBound {
            what: BoundKind::Cancellations,
            count: 2,
            ..
        })
    ));
}

#[test]
fn receipt_coverage_counts_match_the_record() {
    for seed in 0..16 {
        let r = explore(&mut Toy::new(seed), &supported(seed));
        let Ok(cov) = project(&r, seed) else {
            assert_ne!(r.verdict(), Verdict::Complete);
            continue;
        };
        let o = cov.as_object().expect("object");
        let strategies = o["strategies"].as_array().expect("array");
        let failing = o["failing_neighbors"].as_array().expect("array");
        let ran: BTreeSet<&str> = r
            .strategies()
            .iter()
            .filter(|(_, st)| matches!(st, StrategyStatus::Ran(_)))
            .map(|(s, _)| s.token())
            .collect();
        let listed: BTreeSet<&str> = strategies
            .iter()
            .map(|s| {
                s.as_object().expect("object")["strategy"]
                    .as_str()
                    .expect("token")
            })
            .collect();
        assert_eq!(ran, listed, "not-run strategies are omitted");
        for s in strategies {
            let s = s.as_object().expect("object");
            let token = s["strategy"].as_str().expect("token");
            let n = failing
                .iter()
                .filter(|f| f.as_object().expect("object")["strategy"].as_str() == Some(token))
                .count();
            assert_eq!(
                s["failing"],
                Json::Integer(i64::try_from(n).expect("small"))
            );
            let c = r
                .coverage(Strategy::from_token(token).expect("a strategy"))
                .expect("ran");
            assert_eq!(
                s["explored"],
                Json::Integer(i64::try_from(c.distinct_runs).expect("small"))
            );
        }
        assert_eq!(failing.len(), r.failing().len());
    }
}

#[test]
fn strategy_tokens_round_trip_and_are_closed() {
    for s in Strategy::ALL {
        assert_eq!(Strategy::from_token(s.token()), Some(s));
        assert_eq!(Strategy::ALL[usize::try_from(s.index()).expect("small")], s);
    }
    assert_eq!(Strategy::from_token("reorder-bytes"), None);
    let generic: Vec<Strategy> = Strategy::ALL
        .into_iter()
        .filter(|s| s.is_generic())
        .collect();
    assert_eq!(
        generic,
        [
            Strategy::AlternateEnabledEvents,
            Strategy::MessageDuplicationLossDelay,
            Strategy::SchedulePerturbation
        ]
    );
}

#[test]
fn check_candidate_matches_the_campaign() {
    // Differential: the public single-candidate check against the campaign's
    // dispositions, on every schedule candidate of random cores, re-derived by
    // regenerating each rejected schedule from its id.
    let env = envelope();
    for seed in 0..32 {
        let mut toy = Toy::new(seed);
        let r = explore(&mut toy, &config(seed));
        let n = toy.core.len();
        for nb in r.neighbors() {
            let Some(rest) = nb.id.split_once(":hoist(").map(|(_, r)| r) else {
                continue;
            };
            let _ = (rest, n);
            let order = schedule_of(&toy.core, &nb.id).expect("a hoist");
            let got = neighborhood::check_candidate(
                &toy.core,
                &env,
                &toy.core_profile(),
                &Edit::<u32>::Schedule(order),
                &Profile::default(),
            );
            match &nb.disposition {
                Disposition::Rejected(want) => assert_eq!(got.as_ref().err(), Some(want)),
                _ => assert_eq!(got, Ok(())),
            }
        }
    }
}

// ---------------------------------------------------------------------------
// cr-3hfw5b round 2: the causal-neighbor definition and the continuation state
// ---------------------------------------------------------------------------

/// How a test program realizes a neighbor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    /// It runs the requested schedule or edit.
    Faithful,
    /// A repaired program that ignores the schedule: it always runs the core's order.
    CoreOrder,
    /// A program that removed the odd events: they are absent.
    DropOdd,
    /// It stops half way and says it did not halt.
    Truncate,
    /// It stops half way, says it halted, and passes.
    HaltedPass,
    /// It stops half way, says it halted, and fails.
    HaltedFail,
    /// It runs another scenario than the one requested.
    WrongEdit,
    /// It cannot decide the run.
    Undecided,
    /// It names the run with a malformed handle.
    BadHandle,
}

const MODES: [Mode; 9] = [
    Mode::Faithful,
    Mode::CoreOrder,
    Mode::DropOdd,
    Mode::Truncate,
    Mode::HaltedPass,
    Mode::HaltedFail,
    Mode::WrongEdit,
    Mode::Undecided,
    Mode::BadHandle,
];

/// A toy program whose realization follows `mode`, recording what each run was.
struct Moded {
    toy: Toy,
    mode: Mode,
    /// Per realized run, whether it was faithful and whether it halted.
    last: Option<(bool, bool)>,
}

impl Substrate for Moded {
    type Core = CoreOrder;
    type Scenario = u32;
    type Run = Vec<usize>;
    fn core(&self) -> &CoreOrder {
        self.toy.core()
    }
    fn core_identity(&self) -> String {
        self.toy.core_identity()
    }
    fn subject(&self) -> String {
        format!("toy-{:?}", self.mode)
    }
    fn engine_identity(&self) -> String {
        self.toy.engine_identity()
    }
    fn scenario_bytes(&self, s: &u32) -> Vec<u8> {
        self.toy.scenario_bytes(s)
    }
    fn core_profile(&self) -> Profile {
        self.toy.core_profile()
    }
    fn not_run(&self, s: Strategy) -> Option<NotRun> {
        self.toy.not_run(s)
    }
    fn scenario_count(&self, s: Strategy) -> usize {
        self.toy.scenario_count(s)
    }
    fn scenario(&self, s: Strategy, i: usize) -> Option<(String, u32, Profile)> {
        self.toy.scenario(s, i)
    }
    fn realize(&mut self, edit: &Edit<u32>) -> Realized<Vec<usize>> {
        let n = self.toy.core.len();
        match edit {
            Edit::Schedule(order) => {
                let (taken, absent, halted) = match self.mode {
                    Mode::CoreOrder => ((0..n).collect(), vec![], false),
                    Mode::DropOdd => (
                        order.iter().copied().filter(|e| e % 2 == 0).collect(),
                        (0..n).filter(|e| e % 2 == 1).collect(),
                        false,
                    ),
                    Mode::Truncate => (order[..n / 2].to_vec(), vec![], false),
                    Mode::HaltedPass | Mode::HaltedFail => (order[..n / 2].to_vec(), vec![], true),
                    _ => (order.clone(), vec![], false),
                };
                // A truncated run is never the neighbor; the other modes may or may not
                // be causal neighbors, which the reference judges after the campaign.
                let possible = self.mode != Mode::Truncate;
                self.last = Some((possible, halted));
                Realized::Run {
                    run: taken.clone(),
                    witness: Witness::Schedule {
                        taken,
                        absent,
                        halted,
                    },
                }
            }
            Edit::Message { .. } => Realized::Unsupported("no network".to_owned()),
            Edit::Scenario(x) => {
                let wrong = self.mode == Mode::WrongEdit;
                self.last = Some((!wrong, false));
                Realized::Run {
                    run: (0..n).collect(),
                    witness: Witness::Edit((x + u32::from(wrong)).to_be_bytes().to_vec()),
                }
            }
        }
    }
    fn execute(&mut self, run: &Vec<usize>) -> Execution {
        let (faithful, halted) = self.last.take().expect("realized first");
        // The library must never execute a run that cannot be the neighbor (a halted
        // run is executed to learn whether it failed, and kept only if it did).
        assert!(faithful || halted, "an impossible realization was executed");
        match self.mode {
            Mode::Undecided => Execution::Inconclusive {
                reason: InconclusiveReason::InsufficientTelemetry,
            },
            Mode::BadHandle => Execution::Pass {
                run: "not a handle".to_owned(),
            },
            Mode::HaltedPass => Execution::Pass {
                run: format!("ev_h{}", run.len()),
            },
            Mode::HaltedFail => Execution::Fail {
                property: "deadlock".to_owned(),
                run: format!("ev_h{}", run.len()),
            },
            _ => {
                // The toy property on a possibly partial run: it fails while the pair
                // runs in its core order.
                let pos = |e: usize| run.iter().position(|x| *x == e);
                let tag: String = run.iter().map(|e| format!("{e:x}")).collect();
                let (a, b) = self.toy.pair;
                match (pos(a), pos(b)) {
                    (Some(x), Some(y)) if x < y => Execution::Fail {
                        property: "toy".to_owned(),
                        run: format!("ev_{tag}"),
                    },
                    _ => Execution::Pass {
                        run: format!("ev_{tag}"),
                    },
                }
            }
        }
    }
}

/// The reference causal-neighbor check, written independently of the library: a
/// schedule realization is the neighbor when it takes every event, keeps every
/// dependent pair's requested order, and takes the decision pair as requested.
fn reference_neighbor(
    core: &CoreOrder,
    id: &str,
    order: &[usize],
    taken: &[usize],
    absent: usize,
) -> bool {
    let n = core.len();
    if taken.len() + absent != n {
        return false;
    }
    let pos = |list: &[usize], e: usize| list.iter().position(|x| *x == e);
    // No hard order of the core (the transitive closure) inverted among the taken
    // events.
    let mut ancestors: Vec<BTreeSet<usize>> = Vec::new();
    for e in 0..n {
        let mut anc = BTreeSet::new();
        for &p in core.hard_predecessors(e) {
            anc.insert(p);
            anc.extend(ancestors[p].iter().copied());
        }
        ancestors.push(anc);
    }
    for (i, &later) in taken.iter().enumerate() {
        if taken[..i]
            .iter()
            .any(|earlier| ancestors[*earlier].contains(&later))
        {
            return false;
        }
    }
    for a in 0..n {
        for b in 0..n {
            if a != b
                && core.dependent(a, b)
                && pos(taken, a).is_some()
                && pos(taken, b).is_some()
                && (pos(order, a) < pos(order, b)) != (pos(taken, a) < pos(taken, b))
            {
                return false;
            }
        }
    }
    let decision = if let Some(r) = id.split_once(":swap(").map(|x| x.1) {
        let (i, j) = r.trim_end_matches(')').split_once(',').expect("pair");
        Some((
            j.parse::<usize>().expect("j"),
            i.parse::<usize>().expect("i"),
        ))
    } else if let Some(r) = id.split_once(":hoist(").map(|x| x.1) {
        let (j, i) = r
            .trim_end_matches(')')
            .split_once(" before ")
            .expect("pair");
        Some((
            j.parse::<usize>().expect("j"),
            i.parse::<usize>().expect("i"),
        ))
    } else if let Some(r) = id.split_once(":delay(").map(|x| x.1) {
        let (s, d) = r.trim_end_matches(')').split_once(",+").expect("pair");
        let (s, d): (usize, usize) = (s.parse().expect("s"), d.parse().expect("d"));
        // The last event moved before the send runs before it.
        Some((order[s + d - 1], s))
    } else {
        None
    };
    decision
        .is_none_or(|(x, y)| matches!((pos(taken, x), pos(taken, y)), (Some(p), Some(q)) if p < q))
}

#[test]
fn receipt_coverage_projects_complete_only_over_proven_realizations() {
    // Every (strategy set x realization mode x budget) combination: the projection is
    // Ok exactly when the verdict is Complete, and Complete only when every selected
    // strategy executed something and every executed neighbor is a proven realization
    // (the library executed no unfaithful run: `Moded::execute` asserts it). The
    // library's check agrees with the independent reference on every schedule it
    // executed.
    let sets: Vec<Vec<Strategy>> = vec![
        vec![Strategy::SchedulePerturbation],
        vec![Strategy::AlternateEnabledEvents],
        vec![Strategy::MessageDuplicationLossDelay],
        vec![Strategy::FaultWindow],
        vec![
            Strategy::AlternateEnabledEvents,
            Strategy::FaultWindow,
            Strategy::SchedulePerturbation,
        ],
    ];
    let mut seen_verdicts = BTreeSet::new();
    for seed in 0..12 {
        for set in &sets {
            for mode in MODES {
                for runs in [2, 10_000] {
                    let mut cfg = config(seed);
                    cfg.strategies.clone_from(set);
                    cfg.max_delay = 1;
                    cfg.budget.runs = runs;
                    let mut sub = Moded {
                        toy: Toy::new(seed),
                        mode,
                        last: None,
                    };
                    let r = neighborhood::explore(&mut sub, &envelope(), &cfg)
                        .expect("ran")
                        .into_record();
                    seen_verdicts.insert(format!("{:?}", r.verdict()));
                    let projected = r.receipt_coverage(
                        &Moded {
                            toy: Toy::new(seed),
                            mode,
                            last: None,
                        },
                        &envelope(),
                        &cfg,
                    );
                    assert_eq!(
                        projected.is_ok(),
                        r.verdict() == Verdict::Complete,
                        "{mode:?} {set:?} seed {seed}"
                    );
                    if projected.is_ok() {
                        for (s, st) in r.strategies() {
                            if let StrategyStatus::Ran(c) = st {
                                assert!(c.executed > 0, "{} executed nothing", s.token());
                                assert_eq!(c.inconclusive + c.unsupported, 0);
                                assert!(c.not_realizable.is_empty());
                            }
                        }
                        // Only a mode whose runs can be the neighbors reaches Complete:
                        // never a truncated or halted-passing run, an undecided or
                        // unnamed one, nor another scenario than the one requested.
                        // Each mode shapes only its own kind of edit.
                        let scenarios = set.contains(&Strategy::FaultWindow);
                        let schedules = set.iter().any(|s| s.is_generic());
                        let schedule_ok = !matches!(mode, Mode::Truncate | Mode::HaltedPass);
                        let edit_ok = mode != Mode::WrongEdit;
                        assert!(
                            !matches!(mode, Mode::Undecided | Mode::BadHandle)
                                && (!schedules || schedule_ok)
                                && (!scenarios || edit_ok),
                            "{mode:?} reached Complete over {set:?}"
                        );
                    }
                    // Differential: whatever the library executed as a schedule
                    // neighbor, the reference accepts.
                    let core = &Toy::new(seed).core;
                    for nb in r.neighbors() {
                        let Disposition::Executed(_) = nb.disposition else {
                            continue;
                        };
                        if let Some(order) = schedule_of(core, &nb.id) {
                            let (taken, absent): (Vec<usize>, usize) = match mode {
                                Mode::CoreOrder => ((0..core.len()).collect(), 0),
                                Mode::DropOdd => (
                                    order.iter().copied().filter(|e| e % 2 == 0).collect(),
                                    core.len() / 2,
                                ),
                                Mode::HaltedFail => continue,
                                _ => (order.clone(), 0),
                            };
                            assert!(
                                reference_neighbor(core, &nb.id, &order, &taken, absent),
                                "{mode:?}: {} executed without being a causal neighbor",
                                nb.id
                            );
                        }
                    }
                }
            }
        }
    }
    for v in ["Complete", "Pending", "Inconclusive"] {
        assert!(
            seen_verdicts.iter().any(|x| x.starts_with(v)),
            "the sweep reaches {v}: {seen_verdicts:?}"
        );
    }
}

/// The requested schedule of a generic schedule neighbor, from its identity; `None`
/// for a walk or a non-schedule edit (a walk's order is not in its id). A hoist runs
/// `j` with its hard ancestors after `i` before `i`; a delay moves the next `d` events
/// that do not descend from the send before it.
fn schedule_of(core: &CoreOrder, id: &str) -> Option<Vec<usize>> {
    let n = core.len();
    let anc = closure(core);
    if let Some(r) = id.split_once(":swap(").map(|x| x.1) {
        let (i, _) = r.trim_end_matches(')').split_once(',')?;
        let i: usize = i.parse().ok()?;
        let mut o: Vec<usize> = (0..n).collect();
        o.swap(i, i + 1);
        return Some(o);
    }
    let front = |lifted: &[usize], at: usize| {
        let mut o: Vec<usize> = (0..at).collect();
        o.extend(lifted);
        o.extend((at..n).filter(|k| !lifted.contains(k)));
        o
    };
    if let Some(r) = id.split_once(":hoist(").map(|x| x.1) {
        let (j, i) = r.trim_end_matches(')').split_once(" before ")?;
        let (j, i): (usize, usize) = (j.parse().ok()?, i.parse().ok()?);
        let lifted: Vec<usize> = (i + 1..=j)
            .filter(|&k| k == j || anc[j].contains(&k))
            .collect();
        return Some(front(&lifted, i));
    }
    if let Some(r) = id.split_once(":delay(").map(|x| x.1) {
        let (s, d) = r.trim_end_matches(')').split_once(",+")?;
        let (s, d): (usize, usize) = (s.parse().ok()?, d.parse().ok()?);
        let past: Vec<usize> = (s + 1..n)
            .filter(|&e| !anc[e].contains(&s))
            .take(d)
            .collect();
        return Some(front(&past, s));
    }
    None
}

#[test]
fn a_repair_that_moves_every_causal_decision_is_never_complete() {
    // The review's validation: a repaired program that runs the core's order whatever
    // is requested (so it moves every causal decision) and fails on none. The
    // alternate-enabled-events neighbors reverse a dependent pair, so none is realized,
    // the strategy executes nothing, and the campaign is not Complete.
    let mut checked = 0;
    for seed in 0..32 {
        let mut cfg = config(seed);
        cfg.strategies = vec![Strategy::AlternateEnabledEvents];
        let mut sub = Moded {
            toy: Toy::new(seed),
            mode: Mode::CoreOrder,
            last: None,
        };
        let r = neighborhood::explore(&mut sub, &envelope(), &cfg)
            .expect("ran")
            .into_record();
        let c = r.coverage(Strategy::AlternateEnabledEvents).expect("ran");
        if c.valid == 0 {
            continue;
        }
        checked += 1;
        assert_eq!(c.executed, 0, "seed {seed}");
        assert_ne!(r.verdict(), Verdict::Complete);
        assert!(
            r.receipt_coverage(
                &Moded {
                    toy: Toy::new(seed),
                    mode: Mode::CoreOrder,
                    last: None
                },
                &envelope(),
                &cfg
            )
            .is_err()
        );
    }
    assert!(checked > 5);
}

#[test]
fn an_engine_error_before_exhaustion_still_suspends_with_a_continuation() {
    // An undisclosable run, then the budget runs out: Pending with a continuation, and
    // the campaign resumed to its end is the engine error it was always going to be.
    let seed = 7;
    let mut cfg = supported(seed);
    let full = neighborhood::explore(
        &mut Moded {
            toy: Toy::new(seed),
            mode: Mode::BadHandle,
            last: None,
        },
        &envelope(),
        &cfg,
    )
    .expect("ran")
    .into_record();
    assert_eq!(
        full.verdict(),
        Verdict::Inconclusive {
            reason: InconclusiveReason::EngineError
        }
    );
    for runs in 1..full.spent().runs {
        cfg.budget.runs = runs;
        let out = neighborhood::explore(
            &mut Moded {
                toy: Toy::new(seed),
                mode: Mode::BadHandle,
                last: None,
            },
            &envelope(),
            &cfg,
        )
        .expect("ran");
        let Exploration::Suspended {
            record,
            continuation,
        } = out
        else {
            panic!("runs {runs}: an exhausted campaign always suspends");
        };
        assert_eq!(record.verdict(), Verdict::Pending);
        let done = neighborhood::resume(
            &mut Moded {
                toy: Toy::new(seed),
                mode: Mode::BadHandle,
                last: None,
            },
            &envelope(),
            &supported(seed),
            &continuation,
            &mut Spent::default(),
        )
        .expect("resumes");
        assert_eq!(
            done.record().result_bytes(),
            full.result_bytes(),
            "runs {runs}"
        );
    }
}

#[test]
fn every_suspension_boundary_resumes_to_the_same_result_and_state() {
    // Suspend at every run boundary, every candidate boundary (mid-strategy, between
    // strategies, after the last candidate) and a sweep of work boundaries (mid-walk,
    // mid-scan). Each resumes to the uninterrupted result. Chained one step at a time,
    // each continuation is in the same state as the direct suspension at that
    // boundary: the same committed record and the same frontier.
    for seed in [3, 7, 11] {
        let cfg = config(seed);
        let full = explore(&mut Toy::new(seed), &cfg);
        let direct = |budget: Budget| {
            let mut c = cfg.clone();
            c.budget = budget;
            neighborhood::explore(&mut Toy::new(seed), &envelope(), &c).ok()
        };
        let finish = |out: Exploration| -> Vec<u8> {
            match out {
                Exploration::Finished(r) => r.result_bytes(),
                Exploration::Suspended { continuation, .. } => neighborhood::resume(
                    &mut Toy::new(seed),
                    &envelope(),
                    &cfg,
                    &continuation,
                    &mut Spent::default(),
                )
                .expect("resumes")
                .record()
                .result_bytes(),
            }
        };
        let base = cfg.budget;
        let mut boundaries: Vec<Budget> = Vec::new();
        for runs in 0..=full.spent().runs {
            boundaries.push(Budget { runs, ..base });
        }
        for candidates in 0..=full.spent().candidates {
            boundaries.push(Budget { candidates, ..base });
        }
        let w = full.spent().work;
        for work in (0..=w).step_by(usize::try_from(w / 60 + 1).expect("small")) {
            boundaries.push(Budget { work, ..base });
        }
        for b in &boundaries {
            if let Some(out) = direct(*b) {
                assert_eq!(finish(out), full.result_bytes(), "seed {seed} budget {b:?}");
            }
        }
        // Chained along runs and along candidates: state equality with the direct cut.
        for axis in 0..2 {
            let step = |k: u64| match axis {
                0 => Budget { runs: k, ..base },
                _ => Budget {
                    candidates: k,
                    ..base
                },
            };
            let top = if axis == 0 {
                full.spent().runs
            } else {
                full.spent().candidates
            };
            let Some(Exploration::Suspended {
                continuation: mut chained,
                ..
            }) = direct(step(0))
            else {
                continue;
            };
            for k in 1..=top {
                let mut c = cfg.clone();
                c.budget = step(k);
                let next = neighborhood::resume(
                    &mut Toy::new(seed),
                    &envelope(),
                    &c,
                    &chained,
                    &mut Spent::default(),
                )
                .expect("resumes");
                let d = direct(step(k)).expect("ran");
                match (next, d) {
                    (
                        Exploration::Suspended {
                            record: r1,
                            continuation: c1,
                        },
                        Exploration::Suspended {
                            record: r2,
                            continuation: c2,
                        },
                    ) => {
                        assert_eq!(
                            c1.frontier(),
                            c2.frontier(),
                            "seed {seed} axis {axis} k {k}"
                        );
                        assert_eq!(
                            r1.result_bytes(),
                            r2.result_bytes(),
                            "seed {seed} axis {axis} k {k}"
                        );
                        assert_eq!(r1.spent().runs, r2.spent().runs);
                        assert_eq!(r1.spent().candidates, r2.spent().candidates);
                        chained = c1;
                    }
                    (Exploration::Finished(r1), Exploration::Finished(r2)) => {
                        assert_eq!(r1.result_bytes(), r2.result_bytes());
                        assert_eq!(r1.result_bytes(), full.result_bytes());
                        break;
                    }
                    (a, b) => panic!(
                        "seed {seed} axis {axis} k {k}: chained {:?} direct {:?}",
                        a.record().verdict(),
                        b.record().verdict()
                    ),
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// cr-3hfw5b round 3: scenario identities and outcome strings
// ---------------------------------------------------------------------------

/// A toy program whose scenario list is given, and whose outcomes can be oversized.
struct Scripted {
    toy: Toy,
    scenarios: Vec<(String, u32)>,
    /// Bytes of padding added to every outcome string.
    pad: usize,
}

impl Substrate for Scripted {
    type Core = CoreOrder;
    type Scenario = u32;
    type Run = Vec<usize>;
    fn core(&self) -> &CoreOrder {
        self.toy.core()
    }
    fn core_identity(&self) -> String {
        self.toy.core_identity()
    }
    fn subject(&self) -> String {
        "scripted".to_owned()
    }
    fn engine_identity(&self) -> String {
        self.toy.engine_identity()
    }
    fn scenario_bytes(&self, s: &u32) -> Vec<u8> {
        s.to_be_bytes().to_vec()
    }
    fn core_profile(&self) -> Profile {
        self.toy.core_profile()
    }
    fn not_run(&self, s: Strategy) -> Option<NotRun> {
        self.toy.not_run(s)
    }
    fn scenario_count(&self, s: Strategy) -> usize {
        if s == Strategy::FaultWindow {
            self.scenarios.len()
        } else {
            0
        }
    }
    fn scenario(&self, _: Strategy, i: usize) -> Option<(String, u32, Profile)> {
        self.scenarios.get(i).map(|(id, x)| {
            let mut p = Profile {
                nodes: 3,
                max_value: Some(1),
                ..Profile::default()
            };
            if *x == 5 {
                // Past the envelope's crash cap: rejected before it can run.
                p = p.with_fault(FaultClass::Crash, 3).expect("fits");
            }
            (id.clone(), *x, p)
        })
    }
    fn realize(&mut self, edit: &Edit<u32>) -> Realized<Vec<usize>> {
        match edit {
            Edit::Scenario(x) if x % 5 == 4 => {
                Realized::NotRealizable(format!("no{}", "!".repeat(self.pad)))
            }
            Edit::Scenario(x) => Realized::Run {
                run: vec![usize::try_from(*x).expect("small")],
                witness: Witness::Edit(x.to_be_bytes().to_vec()),
            },
            other => self.toy.realize(other),
        }
    }
    fn execute(&mut self, run: &Vec<usize>) -> Execution {
        let x = run.first().copied().unwrap_or(0);
        if x % 2 == 1 {
            Execution::Fail {
                property: format!("p{}", "q".repeat(self.pad)),
                run: format!("ev_{x}{}", "h".repeat(self.pad)),
            }
        } else {
            Execution::Pass {
                run: format!("ev_{x}"),
            }
        }
    }
}

fn scripted_config(seed: u64) -> Config {
    let mut c = config(seed);
    c.strategies = vec![Strategy::FaultWindow];
    c
}

#[test]
fn a_forced_identity_collision_is_refused_and_never_complete() {
    // The review's case: two valid scenarios under one identity, the first passing and
    // the second (other bytes) failing. It is an identity collision, never a silent
    // duplicate: the campaign is an engine error and projects nothing.
    let scenarios = vec![("same".to_owned(), 2), ("same".to_owned(), 3)];
    let mut sub = Scripted {
        toy: Toy::new(7),
        scenarios: scenarios.clone(),
        pad: 0,
    };
    let r = neighborhood::explore(&mut sub, &envelope(), &scripted_config(7))
        .expect("ran")
        .into_record();
    let c = r.coverage(Strategy::FaultWindow).expect("ran");
    assert_eq!(c.rejected.get("identity-collision"), Some(&1));
    assert_eq!(
        r.verdict(),
        Verdict::Inconclusive {
            reason: InconclusiveReason::EngineError
        }
    );
    assert!(
        r.receipt_coverage(
            &Scripted {
                toy: Toy::new(7),
                scenarios,
                pad: 0
            },
            &envelope(),
            &scripted_config(7)
        )
        .is_err()
    );
    // An exact repeat (same identity, same content) is still a plain duplicate.
    let r = neighborhood::explore(
        &mut Scripted {
            toy: Toy::new(7),
            scenarios: vec![("a".to_owned(), 2), ("a".to_owned(), 2)],
            pad: 0,
        },
        &envelope(),
        &scripted_config(7),
    )
    .expect("ran")
    .into_record();
    let c = r.coverage(Strategy::FaultWindow).expect("ran");
    assert_eq!(c.rejected.get("duplicate"), Some(&1));
    assert_eq!(c.executed, 1);
    assert_eq!(r.verdict(), Verdict::Complete);
    // One content under two identities is a second binding too: never Complete. And the
    // review's alias-first case a/X, b/X, b/Y.
    for scenarios in [
        vec![("a".to_owned(), 2), ("b".to_owned(), 2)],
        vec![
            ("a".to_owned(), 2),
            ("b".to_owned(), 2),
            ("b".to_owned(), 6),
        ],
    ] {
        let r = neighborhood::explore(
            &mut Scripted {
                toy: Toy::new(7),
                scenarios: scenarios.clone(),
                pad: 0,
            },
            &envelope(),
            &scripted_config(7),
        )
        .expect("ran")
        .into_record();
        let c = r.coverage(Strategy::FaultWindow).expect("ran");
        assert!(
            c.rejected.contains_key("identity-collision"),
            "{scenarios:?}"
        );
        assert_eq!(
            r.verdict(),
            Verdict::Inconclusive {
                reason: InconclusiveReason::EngineError
            },
            "{scenarios:?}"
        );
    }
}

/// Whether the distinct (identity, content) pairs of a list form a bijection.
fn bijective(scenarios: &[(String, u32)]) -> bool {
    let pairs: BTreeSet<(&str, u32)> = scenarios.iter().map(|(i, c)| (i.as_str(), *c)).collect();
    let ids: BTreeSet<&str> = pairs.iter().map(|p| p.0).collect();
    let contents: BTreeSet<u32> = pairs.iter().map(|p| p.1).collect();
    ids.len() == pairs.len() && contents.len() == pairs.len()
}

/// A scripted fault-window campaign over `scenarios`: its verdict and its record.
fn scripted(seed: u64, scenarios: &[(String, u32)]) -> NeighborhoodRecord {
    neighborhood::explore(
        &mut Scripted {
            toy: Toy::new(seed),
            scenarios: scenarios.to_vec(),
            pad: 0,
        },
        &envelope(),
        &scripted_config(seed),
    )
    .expect("ran")
    .into_record()
}

#[test]
fn the_identity_content_check_does_not_depend_on_arrival_order() {
    // Random lists mixing aliases (one content, two identities), collisions (one
    // identity, two contents) and exact duplicates, every permutation of each: the
    // verdict is the same for every order, it is Complete exactly when the distinct
    // pairs are a bijection, and whenever it is not, a collision is recorded.
    fn permutations(items: &[(String, u32)]) -> Vec<Vec<(String, u32)>> {
        if items.len() <= 1 {
            return vec![items.to_vec()];
        }
        let mut out = Vec::new();
        for i in 0..items.len() {
            let mut rest = items.to_vec();
            let x = rest.remove(i);
            for mut p in permutations(&rest) {
                p.insert(0, x.clone());
                out.push(p);
            }
        }
        out
    }
    let mut kinds = BTreeSet::new();
    for seed in 0..150_u64 {
        let mut rng = Rng(seed);
        let len = 2 + usize::try_from(rng.below(4)).expect("small");
        let scenarios: Vec<(String, u32)> = (0..len)
            .map(|_| {
                (
                    format!("id{}", rng.below(3)),
                    // Contents the scripted program realizes (4 would be a gap), and
                    // one the envelope rejects (5): a rejected pair is still declared.
                    [0, 2, 5, 6][usize::try_from(rng.below(4)).expect("small")],
                )
            })
            .collect();
        let want_bijection = bijective(&scenarios);
        kinds.insert(want_bijection);
        let mut verdicts = BTreeSet::new();
        for order in permutations(&scenarios) {
            let r = scripted(seed, &order);
            verdicts.insert(format!("{:?}", r.verdict()));
            let c = r.coverage(Strategy::FaultWindow).expect("ran");
            // Complete needs a bijection and something executed (5 never runs).
            let runs_something = scenarios.iter().any(|(_, c)| *c != 5);
            assert_eq!(
                r.verdict() == Verdict::Complete,
                want_bijection && runs_something,
                "seed {seed}: {order:?}"
            );
            assert_eq!(
                c.rejected.contains_key("identity-collision"),
                !want_bijection,
                "seed {seed}: {order:?}"
            );
        }
        assert_eq!(
            verdicts.len(),
            1,
            "seed {seed}: {scenarios:?} -> {verdicts:?}"
        );
    }
    assert_eq!(
        kinds.len(),
        2,
        "the sweep meets both bijections and non-bijections"
    );
}

#[test]
fn scenario_coverage_counts_distinct_content_and_every_collision_is_an_error() {
    // Property over random scenario lists (identities and contents from small pools):
    // the neighbors the campaign keeps (not rejected) have pairwise distinct contents;
    // a campaign has an identity-collision rejection iff some identity names two
    // contents in the list at its first two such positions; and then it is never
    // Complete.
    for seed in 0..200_u64 {
        let mut rng = Rng(seed);
        let len = 1 + usize::try_from(rng.below(8)).expect("small");
        let scenarios: Vec<(String, u32)> = (0..len)
            .map(|_| {
                (
                    format!("id{}", rng.below(4)),
                    2 * u32::try_from(rng.below(4)).expect("small"),
                )
            })
            .collect();
        let r = neighborhood::explore(
            &mut Scripted {
                toy: Toy::new(seed),
                scenarios: scenarios.clone(),
                pad: 0,
            },
            &envelope(),
            &scripted_config(seed),
        )
        .expect("ran")
        .into_record();
        let kept: Vec<u32> = r
            .neighbors()
            .iter()
            .filter(|n| !matches!(n.disposition, Disposition::Rejected(_)))
            .map(|n| {
                let index: usize =
                    n.id.split_once('#')
                        .and_then(|(_, rest)| rest.split_once(':'))
                        .and_then(|(i, _)| i.parse().ok())
                        .expect("index");
                scenarios[index].1
            })
            .collect();
        let distinct: BTreeSet<u32> = kept.iter().copied().collect();
        assert_eq!(distinct.len(), kept.len(), "seed {seed}: {scenarios:?}");
        let collision = !bijective(&scenarios);
        let c = r.coverage(Strategy::FaultWindow).expect("ran");
        assert_eq!(
            c.rejected.contains_key("identity-collision"),
            collision,
            "seed {seed}: {scenarios:?}"
        );
        if collision {
            assert_ne!(r.verdict(), Verdict::Complete);
        }
    }
}

#[test]
fn oversized_outcome_strings_are_bounded_and_make_the_campaign_an_engine_error() {
    // Reasons, properties and handles a megabyte long: none is kept, each is replaced
    // by the typed marker with its length, the outcome keeps its kind, the record stays
    // small, and the campaign is an engine error that projects nothing.
    let scenarios = vec![
        ("pass".to_owned(), 2),
        ("fail".to_owned(), 3),
        ("unreal".to_owned(), 4),
    ];
    let pad = 1 << 20;
    let r = neighborhood::explore(
        &mut Scripted {
            toy: Toy::new(7),
            scenarios: scenarios.clone(),
            pad,
        },
        &envelope(),
        &scripted_config(7),
    )
    .expect("ran")
    .into_record();
    assert_eq!(
        r.verdict(),
        Verdict::Inconclusive {
            reason: InconclusiveReason::EngineError
        }
    );
    assert!(
        r.canonical_bytes().len() < 64 * 1024,
        "nothing oversized kept"
    );
    let kinds: Vec<&str> = r
        .neighbors()
        .iter()
        .map(|n| match &n.disposition {
            Disposition::Executed(Execution::Pass { .. }) => "pass",
            Disposition::Executed(Execution::Fail { property, run }) => {
                assert!(property.starts_with(neighborhood::OVERSIZED));
                assert!(run.starts_with(neighborhood::OVERSIZED));
                "fail"
            }
            Disposition::NotRealizable(w) => {
                assert!(w.starts_with(neighborhood::OVERSIZED));
                "not-realizable"
            }
            _ => "other",
        })
        .collect();
    assert_eq!(
        kinds,
        ["pass", "fail", "not-realizable"],
        "kinds never change"
    );
    assert!(
        r.receipt_coverage(
            &Scripted {
                toy: Toy::new(7),
                scenarios: scenarios.clone(),
                pad
            },
            &envelope(),
            &scripted_config(7)
        )
        .is_err()
    );
    // The storage is charged before the run: a budget that cannot pay the outcome
    // reserve does not run at all, and suspends.
    let mut tight = scripted_config(7);
    tight.budget.work = full_work_before_first_run(&scenarios);
    let out = neighborhood::explore(
        &mut Scripted {
            toy: Toy::new(7),
            scenarios,
            pad,
        },
        &envelope(),
        &tight,
    )
    .expect("ran");
    assert!(matches!(out, Exploration::Suspended { .. }));
    assert_eq!(out.record().spent().runs, 0);
}

/// The work a campaign of `scenarios` spends before its first run charge: the core
/// check and the first candidate. Found by searching the smallest budget that runs.
fn full_work_before_first_run(scenarios: &[(String, u32)]) -> u64 {
    let mut lo = 0_u64;
    let mut hi = 1 << 24;
    while lo + 1 < hi {
        let mid = u64::midpoint(lo, hi);
        let mut c = scripted_config(7);
        c.budget.work = mid;
        let ran = neighborhood::explore(
            &mut Scripted {
                toy: Toy::new(7),
                scenarios: scenarios.to_vec(),
                pad: 0,
            },
            &envelope(),
            &c,
        )
        .map(|o| o.record().spent().runs)
        .unwrap_or(0);
        if ran > 0 {
            hi = mid;
        } else {
            lo = mid;
        }
    }
    lo
}

/// Every place a substrate-supplied string or byte vector enters the crate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Entry {
    CoreIdentity,
    Subject,
    EngineIdentity,
    NotRunReason,
    ScenarioId,
    ScenarioBytes,
    NotRealizableReason,
    UnsupportedReason,
    WitnessEcho,
    WitnessSchedule,
    FailProperty,
    RunHandle,
}

const ENTRIES: [Entry; 12] = [
    Entry::CoreIdentity,
    Entry::Subject,
    Entry::EngineIdentity,
    Entry::NotRunReason,
    Entry::ScenarioId,
    Entry::ScenarioBytes,
    Entry::NotRealizableReason,
    Entry::UnsupportedReason,
    Entry::WitnessEcho,
    Entry::WitnessSchedule,
    Entry::FailProperty,
    Entry::RunHandle,
];

const MIB: usize = 1 << 20;

/// A toy program with one entry point made a mebibyte long.
struct Big {
    toy: Toy,
    big: Entry,
    /// How many times an identity was read: a stateful program could answer a later
    /// read differently.
    reads: std::cell::Cell<usize>,
}

impl Big {
    fn s(&self, entry: Entry, small: &str) -> String {
        if self.big == entry {
            "x".repeat(MIB)
        } else {
            small.to_owned()
        }
    }
}

impl Substrate for Big {
    type Core = CoreOrder;
    type Scenario = u32;
    type Run = Vec<usize>;
    fn core(&self) -> &CoreOrder {
        self.toy.core()
    }
    fn core_identity(&self) -> String {
        self.reads.set(self.reads.get() + 1);
        self.s(Entry::CoreIdentity, "toy-core")
    }
    fn subject(&self) -> String {
        self.s(Entry::Subject, "big")
    }
    fn engine_identity(&self) -> String {
        self.s(Entry::EngineIdentity, "toy-engine/1")
    }
    fn scenario_bytes(&self, s: &u32) -> Vec<u8> {
        if self.big == Entry::ScenarioBytes {
            vec![1; MIB]
        } else {
            s.to_be_bytes().to_vec()
        }
    }
    fn core_profile(&self) -> Profile {
        self.toy.core_profile()
    }
    fn not_run(&self, s: Strategy) -> Option<NotRun> {
        (s == Strategy::HiddenCorpusMutations)
            .then(|| NotRun(self.s(Entry::NotRunReason, "no corpus")))
    }
    fn scenario_count(&self, s: Strategy) -> usize {
        usize::from(s == Strategy::FaultWindow) * 3
    }
    fn scenario(&self, _: Strategy, i: usize) -> Option<(String, u32, Profile)> {
        let x = u32::try_from(i).expect("small");
        Some((
            format!("{}{i}", self.s(Entry::ScenarioId, "s")),
            x,
            Profile {
                nodes: 3,
                max_value: Some(1),
                ..Profile::default()
            },
        ))
    }
    fn realize(&mut self, edit: &Edit<u32>) -> Realized<Vec<usize>> {
        match edit {
            Edit::Scenario(0) => Realized::NotRealizable(self.s(Entry::NotRealizableReason, "no")),
            Edit::Scenario(1) => Realized::Unsupported(self.s(Entry::UnsupportedReason, "no")),
            Edit::Scenario(x) => Realized::Run {
                run: vec![1],
                witness: Witness::Edit(if self.big == Entry::WitnessEcho {
                    vec![0; MIB]
                } else {
                    x.to_be_bytes().to_vec()
                }),
            },
            Edit::Schedule(order) => Realized::Run {
                run: vec![1],
                witness: Witness::Schedule {
                    taken: if self.big == Entry::WitnessSchedule {
                        vec![0; MIB]
                    } else {
                        order.clone()
                    },
                    absent: vec![],
                    halted: false,
                },
            },
            Edit::Message { .. } => Realized::Unsupported("no network".to_owned()),
        }
    }
    fn execute(&mut self, _: &Vec<usize>) -> Execution {
        Execution::Fail {
            property: self.s(Entry::FailProperty, "p"),
            run: if self.big == Entry::RunHandle {
                format!("ev_{}", "h".repeat(MIB))
            } else {
                "ev_1".to_owned()
            },
        }
    }
}

#[test]
fn every_substrate_string_entry_point_is_bounded_at_a_mebibyte() {
    // Each entry point in turn made 1 MiB: the identities refuse the campaign before
    // any work; every other one is replaced by a typed marker (or refused as a
    // witness), nothing of it is kept, and the campaign is never Complete.
    for big in ENTRIES {
        let mut cfg = config(7);
        cfg.strategies = vec![
            Strategy::FaultWindow,
            Strategy::SchedulePerturbation,
            Strategy::HiddenCorpusMutations,
        ];
        let mut sub = Big {
            toy: Toy::new(7),
            big,
            reads: std::cell::Cell::new(0),
        };
        match neighborhood::explore(&mut sub, &envelope(), &cfg) {
            Err(CoreError::IdentityTooLong { len }) => {
                assert!(
                    matches!(
                        big,
                        Entry::CoreIdentity | Entry::Subject | Entry::EngineIdentity
                    ),
                    "{big:?}"
                );
                assert!(len >= MIB);
            }
            Err(e) => panic!("{big:?}: {e:?}"),
            Ok(out) => {
                assert!(
                    !matches!(
                        big,
                        Entry::CoreIdentity | Entry::Subject | Entry::EngineIdentity
                    ),
                    "{big:?} was accepted"
                );
                let r = out.into_record();
                assert!(
                    r.canonical_bytes().len() < 256 * 1024,
                    "{big:?}: {} bytes kept",
                    r.canonical_bytes().len()
                );
                assert_ne!(r.verdict(), Verdict::Complete, "{big:?}");
            }
        }
    }
}

#[test]
fn identities_are_read_once_and_bound_as_charged() {
    // A stateful program answering later identity reads differently gains nothing: the
    // campaign reads each identity once, charges it, and binds exactly that value.
    struct Drifting(Toy, std::cell::Cell<usize>);
    impl Substrate for Drifting {
        type Core = CoreOrder;
        type Scenario = u32;
        type Run = Vec<usize>;
        fn core(&self) -> &CoreOrder {
            self.0.core()
        }
        fn core_identity(&self) -> String {
            let k = self.1.get();
            self.1.set(k + 1);
            if k == 0 {
                "short".to_owned()
            } else {
                "x".repeat(MIB)
            }
        }
        fn subject(&self) -> String {
            self.0.subject()
        }
        fn engine_identity(&self) -> String {
            self.0.engine_identity()
        }
        fn scenario_bytes(&self, s: &u32) -> Vec<u8> {
            self.0.scenario_bytes(s)
        }
        fn core_profile(&self) -> Profile {
            self.0.core_profile()
        }
        fn not_run(&self, s: Strategy) -> Option<NotRun> {
            self.0.not_run(s)
        }
        fn scenario_count(&self, s: Strategy) -> usize {
            self.0.scenario_count(s)
        }
        fn scenario(&self, s: Strategy, i: usize) -> Option<(String, u32, Profile)> {
            self.0.scenario(s, i)
        }
        fn realize(&mut self, e: &Edit<u32>) -> Realized<Vec<usize>> {
            self.0.realize(e)
        }
        fn execute(&mut self, r: &Vec<usize>) -> Execution {
            self.0.execute(r)
        }
    }
    let mut sub = Drifting(Toy::new(7), std::cell::Cell::new(0));
    let r = neighborhood::explore(&mut sub, &envelope(), &supported(7))
        .expect("ran")
        .into_record();
    assert_eq!(sub.1.get(), 1, "read once");
    assert_eq!(r.core(), "short");
    let inputs = String::from_utf8(r.inputs().canonical_bytes()).expect("utf-8");
    assert!(inputs.contains("\"identity\":\"short\""));
    assert!(r.canonical_bytes().len() < 256 * 1024);
}

#[test]
fn an_empty_scenario_identity_is_an_identity_like_any_other() {
    // B1: "" is bound and checked like any identity, so the engine's record reconciles
    // and a collision under "" is the typed engine error, never an inconsistency.
    for (scenarios, collides) in [
        (vec![(String::new(), 2), (String::new(), 6)], true),
        (vec![(String::new(), 2), ("a".to_owned(), 2)], true),
        (vec![(String::new(), 2), (String::new(), 2)], false),
    ] {
        let r = scripted(7, &scenarios);
        let projected = r.receipt_coverage(
            &Scripted {
                toy: Toy::new(7),
                scenarios: scenarios.clone(),
                pad: 0,
            },
            &envelope(),
            &scripted_config(7),
        );
        if collides {
            assert_eq!(
                r.verdict(),
                Verdict::Inconclusive {
                    reason: InconclusiveReason::EngineError
                },
                "{scenarios:?}"
            );
            assert!(matches!(
                projected,
                Err(ProjectionRefusal::Inconclusive { .. })
            ));
        } else {
            assert_eq!(r.verdict(), Verdict::Complete, "{scenarios:?}");
            assert!(projected.is_ok());
        }
    }
}

/// A toy core whose relation or profile answers later reads differently.
struct Drift {
    toy: Toy,
    reads: std::cell::Cell<usize>,
    big: Vec<usize>,
}

impl CausalCore for Drift {
    fn len(&self) -> usize {
        self.toy.core.len()
    }
    fn hard_predecessors(&self, event: usize) -> &[usize] {
        // Every read after the first sees a huge, out-of-range list for the last event.
        if event + 1 == self.len() {
            let k = self.reads.get();
            self.reads.set(k + 1);
            if k > 0 {
                return &self.big;
            }
        }
        self.toy.core.hard_predecessors(event)
    }
    fn dependent(&self, a: usize, b: usize) -> bool {
        self.toy.core.dependent(a, b)
    }
    fn in_core(&self, event: usize) -> bool {
        self.toy.core.in_core(event)
    }
    fn class(&self, event: usize) -> EventClass {
        self.toy.core.class(event)
    }
}

struct DriftSub(Drift, std::cell::Cell<usize>);

impl Substrate for DriftSub {
    type Core = Drift;
    type Scenario = u32;
    type Run = Vec<usize>;
    fn core(&self) -> &Drift {
        &self.0
    }
    fn core_identity(&self) -> String {
        self.0.toy.core_identity()
    }
    fn subject(&self) -> String {
        self.0.toy.subject()
    }
    fn engine_identity(&self) -> String {
        self.0.toy.engine_identity()
    }
    fn scenario_bytes(&self, s: &u32) -> Vec<u8> {
        self.0.toy.scenario_bytes(s)
    }
    fn core_profile(&self) -> Profile {
        // In the envelope on the first read, far outside it afterwards.
        let k = self.1.get();
        self.1.set(k + 1);
        let p = self.0.toy.core_profile();
        if k == 0 {
            p
        } else {
            p.with_fault(FaultClass::Crash, 9).expect("fits")
        }
    }
    fn not_run(&self, s: Strategy) -> Option<NotRun> {
        self.0.toy.not_run(s)
    }
    fn scenario_count(&self, s: Strategy) -> usize {
        self.0.toy.scenario_count(s)
    }
    fn scenario(&self, s: Strategy, i: usize) -> Option<(String, u32, Profile)> {
        self.0.toy.scenario(s, i)
    }
    fn realize(&mut self, e: &Edit<u32>) -> Realized<Vec<usize>> {
        self.0.toy.realize(e)
    }
    fn execute(&mut self, r: &Vec<usize>) -> Execution {
        self.0.toy.execute(r)
    }
}

#[test]
fn the_core_and_its_profile_are_read_once_and_used_as_checked() {
    // B2 and B3: a core whose predecessor list and profile change after the first read.
    // The campaign reads each once: the drifted answers are never used, copied or
    // committed, the spend stays within the budget, and the bound profile is the one
    // the envelope checked. A verifier's own derivation reads the drifted profile and
    // refuses it.
    let mut sub = DriftSub(
        Drift {
            toy: Toy::new(7),
            reads: std::cell::Cell::new(0),
            big: vec![usize::MAX; 1 << 20],
        },
        std::cell::Cell::new(0),
    );
    let mut cfg = supported(7);
    cfg.budget.work = 1_000_000;
    let r = neighborhood::explore(&mut sub, &envelope(), &cfg)
        .expect("ran")
        .into_record();
    assert_eq!(
        sub.0.reads.get(),
        1,
        "the last event's predecessors are read once"
    );
    assert_eq!(sub.1.get(), 1, "the profile is read once");
    assert!(r.spent().work <= cfg.budget.work);
    let inputs = String::from_utf8(r.inputs().canonical_bytes()).expect("utf-8");
    assert!(
        !inputs.contains("\"crash\":9"),
        "the checked profile is bound"
    );
    // The verifier's derivation reads the substrate afresh: the drifted core (charged
    // before it is walked, then a predecessor not earlier than its event) or the
    // drifted profile (outside the envelope) is refused, typed.
    let got = r.receipt_coverage(&sub, &envelope(), &cfg);
    assert!(matches!(
        got,
        Err(ProjectionRefusal::Core(
            // The mebibyte list is charged before it is walked, so the budget
            // refuses it first.
            CoreError::CoreCheckExhausted
                | CoreError::PredecessorNotEarlier { .. }
                | CoreError::CoreOutsideEnvelope(_)
        ))
    ));
}

#[test]
fn one_profile_has_one_spelling() {
    // I1: a zero count and an absent class are one profile; values past i64::MAX keep
    // their exact spelling.
    let a = Profile::default()
        .with_fault(FaultClass::Crash, 0)
        .expect("fits");
    let b = Profile::default();
    assert_eq!(
        a.to_json().to_canonical_bytes(),
        b.to_json().to_canonical_bytes()
    );
    let big = |v| Profile {
        max_value: Some(v),
        ..Profile::default()
    };
    assert_ne!(
        big(1 << 63).to_json().to_canonical_bytes(),
        big(u64::MAX).to_json().to_canonical_bytes()
    );
}

// ---------------------------------------------------------------------------
// cr-3hfw5b round 6: hard-order witnesses and exact comparison (ADR-0013)
// ---------------------------------------------------------------------------

/// The review's core: three events, hard edge 0 -> 1, no dependence, all in the core.
/// A program that realizes every schedule as [2, 1, 0].
struct Inverting {
    core: CoreOrder,
}

impl Substrate for Inverting {
    type Core = CoreOrder;
    type Scenario = u32;
    type Run = Vec<usize>;
    fn core(&self) -> &CoreOrder {
        &self.core
    }
    fn core_identity(&self) -> String {
        "three".to_owned()
    }
    fn subject(&self) -> String {
        "inverting".to_owned()
    }
    fn engine_identity(&self) -> String {
        "inverting/1".to_owned()
    }
    fn scenario_bytes(&self, s: &u32) -> Vec<u8> {
        s.to_be_bytes().to_vec()
    }
    fn core_profile(&self) -> Profile {
        Profile {
            nodes: 3,
            max_value: Some(1),
            ..Profile::default()
        }
    }
    fn not_run(&self, _: Strategy) -> Option<NotRun> {
        None
    }
    fn scenario_count(&self, _: Strategy) -> usize {
        0
    }
    fn scenario(&self, _: Strategy, _: usize) -> Option<(String, u32, Profile)> {
        None
    }
    fn realize(&mut self, _: &Edit<u32>) -> Realized<Vec<usize>> {
        Realized::Run {
            run: vec![2, 1, 0],
            witness: Witness::Schedule {
                taken: vec![2, 1, 0],
                absent: vec![],
                halted: false,
            },
        }
    }
    fn execute(&mut self, _: &Vec<usize>) -> Execution {
        Execution::Pass {
            run: "ev_inverted".to_owned(),
        }
    }
}

#[test]
fn a_witness_that_inverts_a_hard_edge_is_refused_and_never_complete() {
    // The review's case: requested swap(1,2) = [0,2,1]; the program takes [2,1,0],
    // which takes the decision (2 before 1) but inverts the hard edge 0 -> 1. It is not
    // realized, nothing executes, and the campaign is not Complete.
    let core = CoreOrder::new(
        vec![vec![], vec![0], vec![]],
        &[],
        &[0, 1, 2],
        &BTreeMap::new(),
    )
    .expect("a core");
    let mut cfg = config(1);
    cfg.strategies = vec![Strategy::SchedulePerturbation];
    cfg.walks = 0;
    let r = neighborhood::explore(&mut Inverting { core }, &envelope(), &cfg)
        .expect("ran")
        .into_record();
    let c = r.coverage(Strategy::SchedulePerturbation).expect("ran");
    assert!(c.valid > 0, "the swap is a valid candidate");
    assert_eq!(c.executed, 0);
    assert!(
        c.not_realizable
            .keys()
            .all(|w| w == "witness: a hard edge inverted"),
        "{:?}",
        c.not_realizable
    );
    assert_ne!(r.verdict(), Verdict::Complete);
}

#[test]
fn no_executed_neighbor_inverts_a_hard_edge_and_no_candidate_proposes_one() {
    // Property over random cores and every realization mode: every schedule candidate
    // the generator lets through respects every hard edge, and every schedule neighbor
    // executed took no hard edge inverted (the reference now checks it).
    for seed in 0..48 {
        let toy = Toy::new(seed);
        let core = toy.core.clone();
        for mode in MODES {
            let mut cfg = config(seed);
            cfg.strategies = vec![
                Strategy::SchedulePerturbation,
                Strategy::AlternateEnabledEvents,
                Strategy::MessageDuplicationLossDelay,
            ];
            let r = neighborhood::explore(
                &mut Moded {
                    toy: Toy::new(seed),
                    mode,
                    last: None,
                },
                &envelope(),
                &cfg,
            )
            .expect("ran")
            .into_record();
            for nb in r.neighbors() {
                let Some(order) = schedule_of(&core, &nb.id) else {
                    continue;
                };
                if !matches!(nb.disposition, Disposition::Rejected(_)) {
                    assert!(
                        hard_ok(&core, &order),
                        "{} proposes a hard inversion",
                        nb.id
                    );
                }
                // What the run took, per mode, is held to the closure.
                if let Disposition::Executed(_) = nb.disposition {
                    let taken: Vec<usize> = match mode {
                        Mode::CoreOrder => (0..core.len()).collect(),
                        Mode::DropOdd => order.iter().copied().filter(|e| e % 2 == 0).collect(),
                        Mode::HaltedFail => order[..core.len() / 2].to_vec(),
                        _ => order.clone(),
                    };
                    assert!(
                        reference_neighbor(&core, "", &order, &taken, core.len() - taken.len()),
                        "{mode:?} {} executed with a hard inversion",
                        nb.id
                    );
                }
            }
        }
    }
}

/// A content index that maps everything to one bucket: every two contents collide.
fn one_bucket(_: &[u8]) -> String {
    "0".to_owned()
}

#[test]
fn a_forced_digest_collision_is_decided_on_the_bytes() {
    // ADR-0013: the digest indexes, the bytes decide. With every content in one digest
    // bucket, two different contents are a typed digest collision (never a duplicate
    // or an alias), the campaign is an engine error and projects nothing; the same
    // bytes again are still a plain duplicate.
    let scenarios = vec![("a".to_owned(), 2), ("b".to_owned(), 6)];
    let r = neighborhood::explore_with_content_index(
        &mut Scripted {
            toy: Toy::new(7),
            scenarios: scenarios.clone(),
            pad: 0,
        },
        &envelope(),
        &scripted_config(7),
        one_bucket,
    )
    .expect("ran")
    .into_record();
    let c = r.coverage(Strategy::FaultWindow).expect("ran");
    assert_eq!(c.rejected.get("digest-collision"), Some(&1));
    assert_eq!(
        r.verdict(),
        Verdict::Inconclusive {
            reason: InconclusiveReason::EngineError
        }
    );
    let same = neighborhood::explore_with_content_index(
        &mut Scripted {
            toy: Toy::new(7),
            scenarios: vec![("a".to_owned(), 2), ("a".to_owned(), 2)],
            pad: 0,
        },
        &envelope(),
        &scripted_config(7),
        one_bucket,
    )
    .expect("ran")
    .into_record();
    let c = same.coverage(Strategy::FaultWindow).expect("ran");
    assert_eq!(c.rejected.get("duplicate"), Some(&1));
    assert_eq!(same.verdict(), Verdict::Complete);
}

#[test]
fn the_core_binding_and_the_witness_are_kept_as_canonical_text() {
    // ADR-0013: the binding carries the core's canonical relation text (compared
    // exactly), and each executed neighbor its witness text, not only digests.
    let r = explore(&mut Toy::new(3), &supported(3));
    let inputs = String::from_utf8(r.inputs().canonical_bytes()).expect("utf-8");
    assert!(inputs.contains("\"relation\":\"0 "), "{inputs}");
    for n in r.neighbors() {
        if let Disposition::Executed(_) = n.disposition {
            assert!(
                n.witness.starts_with("schedule taken[")
                    || n.witness == "edit echo equal to the neighbor's content",
                "{}",
                n.witness
            );
        }
    }
}

#[test]
fn a_transitive_hard_inversion_through_an_absent_event_is_refused() {
    // Hard order is transitive: with 0 -> 1 -> 2, a program that does not have event 1
    // and takes 2 before 0 inverts the core's order even though no direct edge joins 0
    // and 2. The witness check holds the run to the closure.
    struct Skipping(CoreOrder);
    impl Substrate for Skipping {
        type Core = CoreOrder;
        type Scenario = u32;
        type Run = Vec<usize>;
        fn core(&self) -> &CoreOrder {
            &self.0
        }
        fn core_identity(&self) -> String {
            "chain".to_owned()
        }
        fn subject(&self) -> String {
            "skipping".to_owned()
        }
        fn engine_identity(&self) -> String {
            "skipping/1".to_owned()
        }
        fn scenario_bytes(&self, s: &u32) -> Vec<u8> {
            s.to_be_bytes().to_vec()
        }
        fn core_profile(&self) -> Profile {
            Profile {
                nodes: 3,
                max_value: Some(1),
                ..Profile::default()
            }
        }
        fn not_run(&self, _: Strategy) -> Option<NotRun> {
            None
        }
        fn scenario_count(&self, _: Strategy) -> usize {
            0
        }
        fn scenario(&self, _: Strategy, _: usize) -> Option<(String, u32, Profile)> {
            None
        }
        fn realize(&mut self, _: &Edit<u32>) -> Realized<Vec<usize>> {
            Realized::Run {
                run: vec![3, 2, 0],
                witness: Witness::Schedule {
                    taken: vec![3, 2, 0],
                    absent: vec![1],
                    halted: false,
                },
            }
        }
        fn execute(&mut self, _: &Vec<usize>) -> Execution {
            Execution::Pass {
                run: "ev_skip".to_owned(),
            }
        }
    }
    // 0 -> 1 -> 2, and an independent event 3; swapping 2 and 3 is a valid candidate.
    let core = CoreOrder::new(
        vec![vec![], vec![0], vec![1], vec![]],
        &[],
        &[0, 1, 2, 3],
        &BTreeMap::new(),
    )
    .expect("a core");
    let mut cfg = config(1);
    cfg.strategies = vec![Strategy::SchedulePerturbation];
    cfg.walks = 0;
    let r = neighborhood::explore(&mut Skipping(core), &envelope(), &cfg)
        .expect("ran")
        .into_record();
    let c = r.coverage(Strategy::SchedulePerturbation).expect("ran");
    assert!(c.valid > 0);
    assert_eq!(c.executed, 0, "{:?}", c.not_realizable);
    assert_ne!(r.verdict(), Verdict::Complete);
}

#[test]
fn a_halted_run_cannot_take_an_event_before_a_pending_hard_ancestor() {
    // A1: hard edges 0 -> 1 -> 2 and an independent event 3. A halted run that takes
    // [0, 3, 2] ran 2 while its hard predecessor 1, which the program has, never ran.
    struct Halting(CoreOrder);
    impl Substrate for Halting {
        type Core = CoreOrder;
        type Scenario = u32;
        type Run = Vec<usize>;
        fn core(&self) -> &CoreOrder {
            &self.0
        }
        fn core_identity(&self) -> String {
            "chain".to_owned()
        }
        fn subject(&self) -> String {
            "halting".to_owned()
        }
        fn engine_identity(&self) -> String {
            "halting/1".to_owned()
        }
        fn scenario_bytes(&self, s: &u32) -> Vec<u8> {
            s.to_be_bytes().to_vec()
        }
        fn core_profile(&self) -> Profile {
            Profile {
                nodes: 3,
                max_value: Some(1),
                ..Profile::default()
            }
        }
        fn not_run(&self, _: Strategy) -> Option<NotRun> {
            None
        }
        fn scenario_count(&self, _: Strategy) -> usize {
            0
        }
        fn scenario(&self, _: Strategy, _: usize) -> Option<(String, u32, Profile)> {
            None
        }
        fn realize(&mut self, _: &Edit<u32>) -> Realized<Vec<usize>> {
            Realized::Run {
                run: vec![0, 3, 2],
                witness: Witness::Schedule {
                    taken: vec![0, 3, 2],
                    absent: vec![],
                    halted: true,
                },
            }
        }
        fn execute(&mut self, _: &Vec<usize>) -> Execution {
            Execution::Fail {
                property: "deadlock".to_owned(),
                run: "ev_halt".to_owned(),
            }
        }
    }
    let core = CoreOrder::new(
        vec![vec![], vec![0], vec![1], vec![]],
        &[],
        &[0, 1, 2, 3],
        &BTreeMap::new(),
    )
    .expect("a core");
    let mut cfg = config(1);
    cfg.strategies = vec![Strategy::SchedulePerturbation];
    cfg.walks = 0;
    let r = neighborhood::explore(&mut Halting(core), &envelope(), &cfg)
        .expect("ran")
        .into_record();
    let c = r.coverage(Strategy::SchedulePerturbation).expect("ran");
    assert!(c.valid > 0);
    assert_eq!(c.executed, 0, "{:?}", c.not_realizable);
    assert_ne!(r.verdict(), Verdict::Complete);
}

static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// A stateful index: a new digest on every call.
fn counting(_: &[u8]) -> String {
    format!(
        "{}",
        COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    )
}

#[test]
fn no_content_index_decides_equality_or_inequality_and_a_hook_record_never_projects() {
    // B1: two identities, the same bytes, under an index that never repeats: still one
    // content under two identities, a collision, never Complete.
    let scenarios = vec![("a".to_owned(), 2), ("b".to_owned(), 2)];
    let r = neighborhood::explore_with_content_index(
        &mut Scripted {
            toy: Toy::new(7),
            scenarios: scenarios.clone(),
            pad: 0,
        },
        &envelope(),
        &scripted_config(7),
        counting,
    )
    .expect("ran")
    .into_record();
    assert!(
        r.coverage(Strategy::FaultWindow)
            .expect("ran")
            .rejected
            .contains_key("identity-collision")
    );
    assert_ne!(r.verdict(), Verdict::Complete);
    // B2: a record made through the hook binds the hook's index; a verifier's own
    // derivation binds BLAKE3, so the record is stale and never projects, even when it
    // is otherwise Complete.
    let one = vec![("a".to_owned(), 2)];
    let hooked = neighborhood::explore_with_content_index(
        &mut Scripted {
            toy: Toy::new(7),
            scenarios: one.clone(),
            pad: 0,
        },
        &envelope(),
        &scripted_config(7),
        one_bucket,
    )
    .expect("ran")
    .into_record();
    assert_eq!(hooked.verdict(), Verdict::Complete);
    assert_eq!(
        hooked.receipt_coverage(
            &Scripted {
                toy: Toy::new(7),
                scenarios: one,
                pad: 0
            },
            &envelope(),
            &scripted_config(7)
        ),
        Err(ProjectionRefusal::Stale {
            field: "content_index".to_owned()
        })
    );
}

/// A program with `count` scenario edits whose canonical bytes are `size` bytes each,
/// all different.
struct Heavy {
    toy: Toy,
    count: usize,
    size: usize,
}

impl Substrate for Heavy {
    type Core = CoreOrder;
    type Scenario = u32;
    type Run = Vec<usize>;
    fn core(&self) -> &CoreOrder {
        self.toy.core()
    }
    fn core_identity(&self) -> String {
        self.toy.core_identity()
    }
    fn subject(&self) -> String {
        "heavy".to_owned()
    }
    fn engine_identity(&self) -> String {
        self.toy.engine_identity()
    }
    fn scenario_bytes(&self, s: &u32) -> Vec<u8> {
        let mut b = vec![0_u8; self.size];
        b[..4].copy_from_slice(&s.to_be_bytes());
        b
    }
    fn core_profile(&self) -> Profile {
        self.toy.core_profile()
    }
    fn not_run(&self, _: Strategy) -> Option<NotRun> {
        None
    }
    fn scenario_count(&self, s: Strategy) -> usize {
        if s == Strategy::FaultWindow {
            self.count
        } else {
            0
        }
    }
    fn scenario(&self, _: Strategy, i: usize) -> Option<(String, u32, Profile)> {
        Some((
            format!("s{i}"),
            u32::try_from(i).expect("small"),
            Profile {
                nodes: 3,
                max_value: Some(1),
                ..Profile::default()
            },
        ))
    }
    fn realize(&mut self, edit: &Edit<u32>) -> Realized<Vec<usize>> {
        let Edit::Scenario(x) = edit else {
            return Realized::Unsupported("scenarios only".to_owned());
        };
        Realized::Run {
            run: vec![],
            witness: Witness::Edit(self.scenario_bytes(x)),
        }
    }
    fn execute(&mut self, _: &Vec<usize>) -> Execution {
        Execution::Pass {
            run: "ev_heavy".to_owned(),
        }
    }
}

#[test]
fn retained_content_bytes_are_charged_before_they_are_kept() {
    // Fifty scenario edits of 60 KiB each (3 MiB of content) under a 1 MiB work budget:
    // the campaign suspends long before it could keep them all, and at every point the
    // charged work bounds the canonical content bytes it keeps (each content is one
    // shared allocation, charged with its edit before it is built). The continuation
    // it returns keeps nothing more than the record it shares.
    let size = 60 * 1024;
    let mut cfg = config(7);
    cfg.strategies = vec![Strategy::FaultWindow];
    cfg.budget.work = 1 << 20;
    let out = neighborhood::explore(
        &mut Heavy {
            toy: Toy::new(7),
            count: 50,
            size,
        },
        &envelope(),
        &cfg,
    )
    .expect("ran");
    let Exploration::Suspended {
        record,
        continuation,
    } = out
    else {
        panic!("3 MiB of content cannot fit a 1 MiB budget");
    };
    let committed = u64::try_from(record.neighbors().len()).expect("small");
    let retained = committed * u64::try_from(size).expect("small");
    assert!(committed < 50);
    assert!(
        record.spent().work >= retained,
        "charged {} for {retained} retained bytes",
        record.spent().work
    );
    assert!(record.spent().work <= cfg.budget.work);
    // What the continuation actually keeps, from the lengths: one allocation per
    // committed content (shared, not copied per holder), and its bytes within the
    // charged work.
    let (bytes, allocations) = continuation.retained_content();
    assert_eq!(
        allocations,
        record.neighbors().len(),
        "one shared copy per content"
    );
    assert!(bytes >= retained, "every committed content is kept exactly");
    assert!(
        bytes <= record.spent().work,
        "{bytes} retained content bytes exceed {} charged",
        record.spent().work
    );
    // Resuming with room for everything keeps charging: the finished campaign's spend
    // bounds the whole content it keeps.
    let mut full = cfg.clone();
    full.budget.work = 1 << 26;
    let done = neighborhood::resume(
        &mut Heavy {
            toy: Toy::new(7),
            count: 50,
            size,
        },
        &envelope(),
        &full,
        &continuation,
        &mut Spent::default(),
    )
    .expect("resumes")
    .into_record();
    assert_eq!(done.verdict(), Verdict::Complete);
    assert!(done.spent().work >= 50 * u64::try_from(size).expect("small"));
}

// ---------------------------------------------------------------------------
// cr-3hfw5b round 8: checked bounds and cumulative spend
// ---------------------------------------------------------------------------

/// An envelope that enables every class with a `faults` bound of `u32::MAX` and no
/// caps: only overflow handling stands between a clamp and admission.
fn wide_envelope() -> Envelope {
    Envelope::new(
        FaultModel::new(FaultClass::ALL, []).expect("model"),
        Bounds::new(
            ExplorationBound::Bounded(2),
            DeclaredBound::Declared(3),
            DeclaredBound::Declared(i64::from(u32::MAX)),
            ExplorationBound::Unbounded,
        )
        .expect("bounds"),
        Caps::default(),
    )
}

fn one_send_core() -> CoreOrder {
    CoreOrder::new(
        vec![vec![], vec![]],
        &[],
        &[0, 1],
        &BTreeMap::from([(0, EventClass::Send { channel: 0 })]),
    )
    .expect("a core")
}

#[test]
fn an_exact_bound_is_admitted_and_one_past_it_is_refused_as_outside_the_envelope() {
    // Every bound is inclusive and `u32::MAX` is an ordinary count: no marker, no clamp.
    let core = one_send_core();
    let duplicate = Edit::<u32>::Message {
        event: 0,
        fault: FaultClass::Duplication,
    };
    let one = Profile::default()
        .with_fault(FaultClass::Duplication, 1)
        .expect("fits");
    let base = |dups: u32| Profile {
        nodes: 3,
        max_value: Some(1),
        faults: BTreeMap::from([(FaultClass::Duplication, dups)]),
        ..Profile::default()
    };
    let bounded = |faults: i64| {
        Envelope::new(
            FaultModel::new(FaultClass::ALL, []).expect("model"),
            Bounds::new(
                ExplorationBound::Bounded(2),
                DeclaredBound::Declared(3),
                DeclaredBound::Declared(faults),
                ExplorationBound::Unbounded,
            )
            .expect("bounds"),
            Caps::default(),
        )
    };
    let max = i64::from(u32::MAX);
    // A profile exactly at u32::MAX under a u32::MAX bound: admitted.
    assert_eq!(bounded(max).check(&base(u32::MAX)), Ok(()));
    // u32::MAX - 1 plus a generated duplicate reaches the bound exactly: admitted.
    assert_eq!(
        neighborhood::check_candidate(&core, &bounded(max), &base(u32::MAX - 1), &duplicate, &one),
        Ok(())
    );
    // u32::MAX plus one is past a u32::MAX bound: refused as outside the envelope, with
    // its true count.
    assert_eq!(
        neighborhood::check_candidate(&core, &bounded(max), &base(u32::MAX), &duplicate, &one),
        Err(Rejection::ExceedsFaultBound {
            what: BoundKind::Faults,
            count: u64::from(u32::MAX) + 1,
            bound: u64::from(u32::MAX),
        })
    );
    // ... and admitted when the intent permits it: the composition does not fit a u32
    // but is never refused for that.
    assert_eq!(
        neighborhood::check_candidate(&core, &bounded(max + 1), &base(u32::MAX), &duplicate, &one),
        Ok(())
    );
    // The builder never clamps: a sum past u32::MAX is None.
    assert!(
        base(u32::MAX)
            .with_fault(FaultClass::Duplication, 1)
            .is_none()
    );

    // Each bound of `envelope()` exactly, together: admitted. One past each: refused
    // as outside the envelope, by the bound it passes.
    let at = || Profile {
        faults: BTreeMap::from([
            (FaultClass::Crash, 2),
            (FaultClass::Recovery, 2),
            (FaultClass::Partition, 1),
        ]),
        cancellations: 3,
        max_value: Some(1),
        nodes: 3,
    };
    let env = envelope();
    assert_eq!(env.check(&at()), Ok(()));
    let past = |f: &dyn Fn(&mut Profile)| {
        let mut p = at();
        f(&mut p);
        env.check(&p)
    };
    let exceeds = |what, count, bound| Err(Rejection::ExceedsFaultBound { what, count, bound });
    assert_eq!(
        past(&|p| {
            p.faults.insert(FaultClass::Crash, 3);
            p.faults.insert(FaultClass::Partition, 0);
        }),
        exceeds(BoundKind::Crashes, 3, 2)
    );
    assert_eq!(
        past(&|p| p.cancellations = 4),
        exceeds(BoundKind::Cancellations, 4, 3)
    );
    assert_eq!(
        past(&|p| {
            p.faults.insert(FaultClass::Partition, 2);
            p.faults.insert(FaultClass::Crash, 1);
            p.faults.insert(FaultClass::Recovery, 1);
        }),
        exceeds(BoundKind::Partitions, 2, 1)
    );
    assert_eq!(
        past(&|p| {
            p.faults.insert(FaultClass::Recovery, 3);
        }),
        exceeds(BoundKind::Recoveries, 3, 2)
    );
    assert_eq!(
        past(&|p| {
            p.faults.insert(FaultClass::Delay, 1);
        }),
        exceeds(BoundKind::Faults, 4, 3)
    );
    // `values` is a count of values: index 1 is the last of 2; index 2 is past it.
    assert_eq!(
        past(&|p| p.max_value = Some(2)),
        Err(Rejection::OutsideValueDomain { value: 2, bound: 2 })
    );
    assert_eq!(
        past(&|p| p.nodes = 4),
        Err(Rejection::OutsideNodeDomain { nodes: 4, bound: 3 })
    );
}

#[test]
fn an_exact_bound_neighbor_is_executed_and_disclosed_and_an_engine_refusal_is_never_complete() {
    // A campaign whose only neighbor at the bound fails: it is executed and disclosed,
    // never dropped. Neighbors past a bound are rejected as outside the envelope, which
    // is not a gap. Every scenario run of the toy fails (the core order is kept).
    let seed = 7;
    let mut cfg = config(seed);
    cfg.strategies = vec![Strategy::FaultWindow];
    let at = Profile {
        faults: BTreeMap::from([
            (FaultClass::Crash, 2),
            (FaultClass::Recovery, 2),
            (FaultClass::Partition, 1),
        ]),
        cancellations: 3,
        max_value: Some(1),
        nodes: 3,
    };
    let mut past = at.clone();
    past.nodes = 4;
    let mut toy = Toy::new(seed);
    toy.scenarios = vec![
        ("at-every-bound".to_owned(), 1, at.clone()),
        ("one-node-past".to_owned(), 2, past),
    ];
    let r = explore(&mut toy, &cfg);
    assert_eq!(r.verdict(), Verdict::Complete);
    let at_bound = r
        .neighbors()
        .iter()
        .find(|n| n.id.contains("at-every-bound"))
        .expect("the neighbor at the bound");
    assert!(
        matches!(
            at_bound.disposition,
            Disposition::Executed(Execution::Fail { .. })
        ),
        "{:?}",
        at_bound.disposition
    );
    assert!(r.failing().iter().any(|f| f.neighbor == at_bound.id));
    let past_bound = r
        .neighbors()
        .iter()
        .find(|n| n.id.contains("one-node-past"))
        .expect("the neighbor past the bound");
    assert!(matches!(
        past_bound.disposition,
        Disposition::Rejected(Rejection::OutsideNodeDomain { .. })
    ));

    // The same in-envelope neighbor refused by the engine instead (an oversized
    // identity, or an identity naming two contents): a gap, never Complete.
    for extra in [
        ("x".repeat(4096), 5, at.clone()),
        ("at-every-bound".to_owned(), 6, at.clone()),
    ] {
        let mut toy = Toy::new(seed);
        toy.scenarios = vec![("at-every-bound".to_owned(), 1, at.clone()), extra];
        let r = explore(&mut toy, &cfg);
        assert!(
            r.neighbors().iter().any(|n| matches!(
                &n.disposition,
                Disposition::Rejected(x) if x.is_engine_error()
            )),
            "an engine refusal"
        );
        assert_ne!(r.verdict(), Verdict::Complete);
        assert!(project(&r, seed).is_err());
    }
}

#[test]
fn only_refusals_outside_the_envelope_or_of_non_neighbors_leave_a_campaign_complete() {
    // Over random cores and seeds: a Complete record's rejections are all of
    // candidates outside the envelope or not neighbors, never the engine's.
    for seed in 0..40 {
        let r = explore(&mut Toy::new(seed), &supported(seed));
        if r.verdict() == Verdict::Complete {
            for n in r.neighbors() {
                if let Disposition::Rejected(x) = &n.disposition {
                    assert!(!x.is_engine_error(), "seed {seed}: {x:?}");
                }
            }
        }
    }
}

#[test]
fn no_admitted_neighbor_exceeds_any_envelope_bound() {
    // Property over boundary counts: whenever a message fault is admitted, its true
    // (unclamped) profile is within every bound of the envelope.
    let core = one_send_core();
    let points = [0_u32, 1, 2, 3, u32::MAX - 2, u32::MAX - 1, u32::MAX];
    // (envelope, faults bound, crash cap)
    for (env, bound, crash_cap) in [
        (envelope(), 3_u64, 2_u64),
        (wide_envelope(), u64::from(u32::MAX), u64::MAX),
    ] {
        for &dups in &points {
            for &crashes in &points {
                for fault in [FaultClass::Duplication, FaultClass::Loss] {
                    let core_profile = Profile {
                        nodes: 3,
                        max_value: Some(1),
                        faults: BTreeMap::from([
                            (FaultClass::Duplication, dups),
                            (FaultClass::Crash, crashes),
                        ]),
                        cancellations: crashes,
                    };
                    let own = Profile::default().with_fault(fault, 1).expect("fits");
                    let admitted = neighborhood::check_candidate(
                        &core,
                        &env,
                        &core_profile,
                        &Edit::<u32>::Message { event: 0, fault },
                        &own,
                    )
                    .is_ok();
                    if admitted {
                        let dup_total =
                            u64::from(dups) + u64::from(fault == FaultClass::Duplication);
                        let loss_total = u64::from(fault == FaultClass::Loss);
                        let injected = dup_total + loss_total + u64::from(crashes);
                        assert!(
                            injected <= bound && u64::from(crashes) <= crash_cap,
                            "{dups} {crashes} {fault:?}"
                        );
                    } else {
                        // Refused only when a bound is actually passed: exact bounds
                        // are admitted.
                        let dup_total =
                            u64::from(dups) + u64::from(fault == FaultClass::Duplication);
                        let injected =
                            dup_total + u64::from(fault == FaultClass::Loss) + u64::from(crashes);
                        assert!(
                            injected > bound || u64::from(crashes) > crash_cap,
                            "refused within every bound: {dups} {crashes} {fault:?}"
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn repeated_exhausted_resumes_advance_the_ledger_and_never_repeat_free_work() {
    // Resume again and again with a budget that reaches only part-way through the
    // regeneration of the committed candidates, repeating each call with the same
    // continuation and the same budget: the caller's ledger strictly advances on every
    // call that works (the regeneration's work is recorded), never exceeds the budget,
    // and a call with nothing left above it is refused before any work.
    let seed = 7;
    let full = explore(&mut Toy::new(seed), &config(seed));
    let mut cfg = config(seed);
    cfg.budget.runs = full.spent().runs / 2;
    let Exploration::Suspended {
        continuation,
        record,
    } = neighborhood::explore(&mut Toy::new(seed), &envelope(), &cfg).expect("ran")
    else {
        panic!("suspends");
    };
    let mut ledger = record.spent();
    let mut current = continuation;
    let mut advanced = 0;
    for step in 1..=40_u64 {
        let mut c = config(seed);
        c.budget.runs = full.spent().runs / 2;
        c.budget.work = ledger.work + step * 400;
        for _repeat in 0..3 {
            let before = ledger;
            let mut toy = Toy::new(seed);
            let out = neighborhood::resume(&mut toy, &envelope(), &c, &current, &mut ledger);
            assert!(ledger.work <= c.budget.work, "within the budget given");
            assert!(
                ledger.work >= before.work
                    && ledger.runs >= before.runs
                    && ledger.candidates >= before.candidates,
                "monotone"
            );
            match out {
                Ok(Exploration::Suspended {
                    record,
                    continuation,
                }) => {
                    assert!(ledger.work > before.work, "work is charged");
                    assert_eq!(record.spent(), ledger, "the record carries the ledger");
                    current = continuation;
                    advanced += 1;
                }
                Ok(Exploration::Finished(r)) => {
                    assert_eq!(r.spent(), ledger);
                    return;
                }
                Err(ResumeError::Exhausted) if ledger == before => {
                    // Refused before any work.
                    assert!(toy.realized.is_empty() && toy.executed == 0);
                }
                Err(ResumeError::Exhausted) => {
                    assert!(ledger.work > before.work, "the attempt is charged");
                }
                Err(e) => panic!("{e:?}"),
            }
        }
    }
    assert!(advanced > 0, "the sweep exercises exhausted regeneration");
}

#[test]
fn exhaustion_before_the_campaign_exists_is_charged_and_never_repeated_for_free() {
    // Every work budget just above the committed spend: the resume runs out inside the
    // core check, an identity read, the committed-copy reservation or the replay. The
    // same call repeated with the same ledger either advances the ledger or is refused
    // before any work; it never does uncharged work, and the ledger never goes back.
    let seed = 7;
    let mut cfg = config(seed);
    cfg.budget.runs = 5;
    let Exploration::Suspended {
        continuation,
        record,
    } = neighborhood::explore(&mut Toy::new(seed), &envelope(), &cfg).expect("ran")
    else {
        panic!("suspends");
    };
    let committed = record.spent();
    let mut pre_campaign = 0;
    for extra in 1..=3_000_u64 {
        let mut c = config(seed);
        c.budget.work = committed.work + extra;
        let mut ledger = committed;
        let mut refused = false;
        for _ in 0..4 {
            let before = ledger;
            let mut toy = Toy::new(seed);
            let out = neighborhood::resume(&mut toy, &envelope(), &c, &continuation, &mut ledger);
            assert!(ledger.work <= c.budget.work, "extra {extra}");
            assert!(ledger.work >= before.work, "extra {extra}: monotone");
            match out {
                Err(ResumeError::Exhausted) if ledger == before => {
                    assert!(toy.realized.is_empty() && toy.executed == 0);
                    refused = true;
                }
                Err(ResumeError::Exhausted) => {
                    assert!(!refused, "extra {extra}: work after a refusal");
                    assert!(ledger.work > before.work, "extra {extra}: charged");
                    pre_campaign += 1;
                }
                Ok(Exploration::Suspended { record, .. }) => {
                    assert!(!refused);
                    assert_eq!(record.spent(), ledger);
                }
                Ok(Exploration::Finished(r)) => assert_eq!(r.spent(), ledger),
                Err(e) => panic!("extra {extra}: {e:?}"),
            }
        }
    }
    assert!(
        pre_campaign > 0,
        "the sweep exhausts before the campaign exists"
    );
}

#[test]
fn untrusted_resumes_charge_every_byte_and_check_and_the_ledger_is_monotone() {
    let seed = 7;
    let mut cfg = config(seed);
    cfg.budget.runs = 4;
    let Exploration::Suspended {
        continuation,
        record,
    } = neighborhood::explore(&mut Toy::new(seed), &envelope(), &cfg).expect("ran")
    else {
        panic!("suspends");
    };
    let mut ledger = record.spent();
    let mut bytes = continuation.canonical_bytes();
    // A chain of untrusted resumes, each from the bytes the last one returned: the
    // ledger includes the bytes read and the fresh derivation's work, and advances.
    let mut chained = 0;
    for step in 1..=12_u64 {
        let len = u64::try_from(bytes.len()).expect("small");
        let mut c = config(seed);
        c.budget.work = ledger.work + len + 6_000 * step;
        let before = ledger;
        match neighborhood::resume_untrusted(
            &mut Toy::new(seed),
            &envelope(),
            &c,
            &bytes,
            &mut ledger,
        ) {
            Ok(Exploration::Suspended {
                record,
                continuation,
            }) => {
                assert!(ledger.work > before.work + len, "the bytes and the checks");
                assert_eq!(record.spent(), ledger);
                bytes = continuation.canonical_bytes();
                chained += 1;
            }
            Ok(Exploration::Finished(r)) => {
                assert!(ledger.work > before.work + len);
                assert_eq!(r.spent(), ledger);
                break;
            }
            Err(e) => panic!("step {step}: {e:?}"),
        }
    }
    assert!(chained > 1, "a chain of untrusted resumes");

    // Failure exits keep their spend too.
    let good = continuation.canonical_bytes();
    let len = u64::try_from(good.len()).expect("small");
    let mut ledger = Spent::default();
    let malformed = &good[..good.len() / 2];
    assert!(matches!(
        neighborhood::resume_untrusted(
            &mut Toy::new(seed),
            &envelope(),
            &config(seed),
            malformed,
            &mut ledger
        ),
        Err(ResumeError::Malformed(_))
    ));
    assert_eq!(ledger.work, u64::try_from(malformed.len()).expect("small"));
    // Forged: a claimed outcome the derivation does not give, found after the full
    // derivation, which is charged.
    let forged = coherent_forgery(&good);
    {
        let before = ledger;
        let out = neighborhood::resume_untrusted(
            &mut Toy::new(seed),
            &envelope(),
            &config(seed),
            &forged,
            &mut ledger,
        );
        assert!(
            matches!(
                out,
                Err(ResumeError::Forged { .. } | ResumeError::Malformed(_))
            ),
            "{out:?}"
        );
        assert!(ledger.work >= before.work + u64::try_from(forged.len()).expect("small"));
    }
    // Mismatch: other inputs, found after the derivation, charged.
    let mut other = config(seed);
    other.seed += 1;
    let before = ledger;
    assert!(matches!(
        neighborhood::resume_untrusted(
            &mut Toy::new(seed),
            &envelope(),
            &other,
            &good,
            &mut ledger
        ),
        Err(ResumeError::Mismatch(_))
    ));
    assert!(ledger.work > before.work + len, "the derivation is charged");
    // Nothing left above the ledger: refused before a byte is read, ledger unchanged.
    let mut tight = config(seed);
    tight.budget.work = ledger.work + len;
    let before = ledger;
    assert_eq!(
        neighborhood::resume_untrusted(
            &mut Toy::new(seed),
            &envelope(),
            &tight,
            &good,
            &mut ledger
        )
        .err(),
        Some(ResumeError::Exhausted)
    );
    assert_eq!(ledger, before);
}

/// Toy with a not-run strategy whose reason is 200 bytes long.
struct Reasoned(Toy);

impl Substrate for Reasoned {
    type Core = CoreOrder;
    type Scenario = u32;
    type Run = Vec<usize>;
    fn core(&self) -> &CoreOrder {
        self.0.core()
    }
    fn core_identity(&self) -> String {
        self.0.core_identity()
    }
    fn subject(&self) -> String {
        self.0.subject()
    }
    fn engine_identity(&self) -> String {
        self.0.engine_identity()
    }
    fn scenario_bytes(&self, s: &u32) -> Vec<u8> {
        self.0.scenario_bytes(s)
    }
    fn core_profile(&self) -> Profile {
        self.0.core_profile()
    }
    fn not_run(&self, s: Strategy) -> Option<NotRun> {
        (s == Strategy::CancellationCheckpoints).then(|| NotRun("r".repeat(200)))
    }
    fn scenario_count(&self, s: Strategy) -> usize {
        self.0.scenario_count(s)
    }
    fn scenario(&self, s: Strategy, i: usize) -> Option<(String, u32, Profile)> {
        self.0.scenario(s, i)
    }
    fn realize(&mut self, e: &Edit<u32>) -> Realized<Vec<usize>> {
        self.0.realize(e)
    }
    fn execute(&mut self, r: &Vec<usize>) -> Execution {
        self.0.execute(r)
    }
}

#[test]
fn every_exhaustion_before_the_prior_frontier_suspends_with_the_committed_state() {
    // Break 1 of the adversarial pass: a resume that runs out of budget reading a
    // not-run reason before its prior frontier must suspend with the committed state
    // and the prior frontier, never finish as an engine error. Swept over every work
    // budget from just above the committed spend.
    let seed = 7;
    let mut cfg = config(seed);
    cfg.strategies = vec![
        Strategy::AlternateEnabledEvents,
        Strategy::FaultWindow,
        Strategy::CancellationCheckpoints,
        Strategy::SchedulePerturbation,
    ];
    cfg.budget.runs = 3;
    let Exploration::Suspended {
        continuation,
        record,
    } = neighborhood::explore(&mut Reasoned(Toy::new(seed)), &envelope(), &cfg).expect("ran")
    else {
        panic!("suspends");
    };
    let spent = record.spent();
    let frontier = continuation.frontier();
    let mut suspended = 0;
    for extra in 1..=30_000_u64 {
        let mut c = cfg.clone();
        c.budget.work = spent.work + extra;
        match neighborhood::resume(
            &mut Reasoned(Toy::new(seed)),
            &envelope(),
            &c,
            &continuation,
            &mut Spent::default(),
        ) {
            Ok(Exploration::Suspended {
                record: r,
                continuation: k,
            }) => {
                assert_eq!(r.verdict(), Verdict::Pending, "extra {extra}");
                assert!(
                    k.frontier() >= frontier,
                    "extra {extra}: the frontier never moves back"
                );
                assert!(
                    r.spent().work > spent.work,
                    "extra {extra}: the work is recorded"
                );
                suspended += 1;
            }
            Ok(Exploration::Finished(r)) => {
                assert_ne!(
                    r.verdict(),
                    Verdict::Inconclusive {
                        reason: InconclusiveReason::EngineError
                    },
                    "extra {extra}: exhaustion became terminal"
                );
            }
            Err(ResumeError::Exhausted) => {}
            Err(e) => panic!("extra {extra}: {e:?}"),
        }
    }
    assert!(suspended > 0);
}

#[test]
fn a_resume_that_cannot_progress_is_refused_before_any_work() {
    // Low 2: no work left, or no candidate left, is refused before the core is even
    // checked.
    let seed = 7;
    let mut cfg = config(seed);
    cfg.budget.runs = 5;
    let Exploration::Suspended {
        continuation,
        record,
    } = neighborhood::explore(&mut Toy::new(seed), &envelope(), &cfg).expect("ran")
    else {
        panic!("suspends");
    };
    let sp = record.spent();
    for budget in [
        Budget {
            work: sp.work,
            ..cfg.budget
        },
        Budget {
            candidates: sp.candidates,
            work: 1 << 30,
            ..cfg.budget
        },
        Budget {
            runs: sp.runs - 1,
            work: 1 << 30,
            ..cfg.budget
        },
    ] {
        let mut c = cfg.clone();
        c.budget = budget;
        let mut toy = Toy::new(seed);
        assert_eq!(
            neighborhood::resume(
                &mut toy,
                &envelope(),
                &c,
                &continuation,
                &mut Spent::default()
            )
            .err(),
            Some(ResumeError::Exhausted),
            "{budget:?}"
        );
        assert!(toy.realized.is_empty());
    }
}

#[test]
fn an_untrusted_resume_records_the_bytes_it_reads() {
    // Low 3: the fresh derivation's ledger includes the bytes read.
    let seed = 7;
    let full = explore(&mut Toy::new(seed), &config(seed));
    let mut cfg = config(seed);
    cfg.budget.runs = 4;
    let Exploration::Suspended { continuation, .. } =
        neighborhood::explore(&mut Toy::new(seed), &envelope(), &cfg).expect("ran")
    else {
        panic!("suspends");
    };
    let bytes = continuation.canonical_bytes();
    let len = u64::try_from(bytes.len()).expect("small");
    let r = neighborhood::resume_untrusted(
        &mut Toy::new(seed),
        &envelope(),
        &config(seed),
        &bytes,
        &mut Spent::default(),
    )
    .expect("resumes")
    .into_record();
    assert_eq!(r.spent().work, full.spent().work + len);
}

#[test]
fn recoveries_never_exceed_the_crashes_they_complete() {
    let p = |crashes, recoveries| {
        Profile {
            nodes: 3,
            max_value: Some(1),
            cancellations: crashes,
            ..Profile::default()
        }
        .with_fault(FaultClass::Crash, crashes)
        .expect("fits")
        .with_fault(FaultClass::Recovery, recoveries)
        .expect("fits")
    };
    assert_eq!(envelope().check(&p(1, 1)), Ok(()));
    assert_eq!(
        envelope().check(&p(0, u32::MAX - 1)),
        Err(Rejection::ExceedsFaultBound {
            what: BoundKind::Recoveries,
            count: u64::from(u32::MAX - 1),
            bound: 0
        })
    );
}

// ---------------------------------------------------------------------------
// cr-3hfw5b round 9: no permitted neighbor disappears from the generators
// ---------------------------------------------------------------------------

/// A substrate that realizes every schedule and message edit. Its first `hoist(1
/// before 0)` run halts and passes with an oversized handle when `halted_big_handle`.
struct Adv {
    core: CoreOrder,
    halted_big_handle: bool,
}

impl Substrate for Adv {
    type Core = CoreOrder;
    type Scenario = u32;
    type Run = (Vec<usize>, bool);
    fn core(&self) -> &CoreOrder {
        &self.core
    }
    fn core_identity(&self) -> String {
        "c".into()
    }
    fn subject(&self) -> String {
        "s".into()
    }
    fn engine_identity(&self) -> String {
        "e".into()
    }
    fn scenario_bytes(&self, s: &u32) -> Vec<u8> {
        s.to_be_bytes().to_vec()
    }
    fn core_profile(&self) -> Profile {
        Profile {
            nodes: 2,
            ..Profile::default()
        }
    }
    fn not_run(&self, _: Strategy) -> Option<NotRun> {
        None
    }
    fn scenario_count(&self, _: Strategy) -> usize {
        0
    }
    fn scenario(&self, _: Strategy, _: usize) -> Option<(String, u32, Profile)> {
        None
    }
    fn realize(&mut self, edit: &Edit<u32>) -> Realized<(Vec<usize>, bool)> {
        match edit {
            Edit::Schedule(o) => {
                let halted =
                    self.halted_big_handle && o.first() == Some(&1) && o.get(1) == Some(&0);
                Realized::Run {
                    run: (o.clone(), halted),
                    witness: Witness::Schedule {
                        taken: o.clone(),
                        absent: vec![],
                        halted,
                    },
                }
            }
            Edit::Message { event, fault } => Realized::Run {
                run: (vec![*event, 999], false),
                witness: Witness::Edit(neighborhood::message_bytes(*event, *fault)),
            },
            Edit::Scenario(_) => Realized::Unsupported("x".into()),
        }
    }
    fn execute(&mut self, run: &(Vec<usize>, bool)) -> Execution {
        if run.1 {
            return Execution::Pass {
                run: "x".repeat(300),
            };
        }
        let tag: String = run.0.iter().map(|e| format!("{e:x}")).collect();
        Execution::Pass {
            run: format!("ev_{tag}"),
        }
    }
}

fn adv_envelope() -> Envelope {
    Envelope::new(
        FaultModel::new(FaultClass::ALL, []).expect("model"),
        Bounds::new(
            ExplorationBound::Unbounded,
            DeclaredBound::Declared(10),
            DeclaredBound::Declared(10),
            ExplorationBound::Unbounded,
        )
        .expect("bounds"),
        Caps::default(),
    )
}

fn adv_config(strategies: Vec<Strategy>, max_delay: u32) -> Config {
    Config {
        seed: 1,
        strategies,
        walks: 0,
        walk_length: 0,
        max_delay,
        budget: Budget {
            candidates: 1000,
            runs: 1000,
            work: 1 << 30,
        },
        transaction: "rt_adv".into(),
    }
}

/// The schedule a witness text records as taken.
fn taken(witness: &str) -> Vec<usize> {
    let body = witness
        .strip_prefix("schedule taken[")
        .and_then(|w| w.split(']').next())
        .unwrap_or("");
    body.split(", ")
        .filter(|x| !x.is_empty())
        .map(|x| x.parse().expect("an event"))
        .collect()
}

/// Every hard ancestor of every event (the transitive closure), from the core itself.
fn closure(core: &CoreOrder) -> Vec<BTreeSet<usize>> {
    let mut out: Vec<BTreeSet<usize>> = Vec::new();
    for e in 0..core.len() {
        let mut a = BTreeSet::new();
        for &p in core.hard_predecessors(e) {
            a.insert(p);
            a.extend(out[p].iter().copied());
        }
        out.push(a);
    }
    out
}

#[test]
fn a_decision_behind_a_hard_ancestor_is_hoisted_with_it_and_taken() {
    // The adversarial pass's case: 2 follows 1 in hard order, both conflict with 0.
    // The decision (0, 2) is taken by [1, 2, 0], never dropped as an invalid hoist.
    let core = CoreOrder::new(
        vec![vec![], vec![], vec![1]],
        &[(0, 1), (0, 2)],
        &[0, 1, 2],
        &BTreeMap::new(),
    )
    .expect("core");
    let mut adv = Adv {
        core,
        halted_big_handle: false,
    };
    let c = adv_config(vec![Strategy::AlternateEnabledEvents], 0);
    let r = neighborhood::explore(&mut adv, &adv_envelope(), &c)
        .expect("campaign")
        .into_record();
    let runs: Vec<Vec<usize>> = r
        .neighbors()
        .iter()
        .filter(|n| matches!(n.disposition, Disposition::Executed(_)))
        .map(|n| taken(&n.witness))
        .collect();
    assert!(runs.contains(&vec![1, 0, 2]), "{runs:?}");
    assert!(runs.contains(&vec![1, 2, 0]), "{runs:?}");
    assert!(
        r.neighbors()
            .iter()
            .all(|n| !matches!(n.disposition, Disposition::Rejected(_))),
        "no hoist is refused"
    );
    assert_eq!(r.verdict(), Verdict::Complete);
}

#[test]
fn a_delay_moves_the_send_past_independent_events_not_its_own_successor() {
    // The send 0 is followed by its own successor 1; 2 is independent. The delay is
    // [2, 0, 1]; a longer one does not exist and is not offered.
    let core = CoreOrder::new(
        vec![vec![], vec![0], vec![]],
        &[],
        &[0, 1, 2],
        &BTreeMap::from([(0, EventClass::Send { channel: 0 })]),
    )
    .expect("core");
    let mut adv = Adv {
        core,
        halted_big_handle: false,
    };
    let c = adv_config(vec![Strategy::MessageDuplicationLossDelay], 2);
    let r = neighborhood::explore(&mut adv, &adv_envelope(), &c)
        .expect("campaign")
        .into_record();
    let delays: Vec<Vec<usize>> = r
        .neighbors()
        .iter()
        .filter(|n| n.id.contains(":delay("))
        .map(|n| {
            assert!(
                matches!(n.disposition, Disposition::Executed(_)),
                "{}: {:?}",
                n.id,
                n.disposition
            );
            taken(&n.witness)
        })
        .collect();
    assert_eq!(delays, vec![vec![2, 0, 1]]);
    assert_eq!(r.verdict(), Verdict::Complete);
}

#[test]
fn every_permitted_decision_and_delay_is_generated_and_none_is_dropped_as_invalid() {
    // Property over random cores: the hoist and delay generators never produce a
    // schedule the hard order refuses, every causal decision (a core pair that
    // conflicts, with no hard path between them) is taken by an executed neighbor, and
    // every send is delayed past each count of non-descendant events up to max_delay.
    for seed in 0..60 {
        let (core, _) = random_core(seed);
        let n = core.len();
        let anc = closure(&core);
        let mut adv = Adv {
            core: core.clone(),
            halted_big_handle: false,
        };
        let c = adv_config(
            vec![
                Strategy::AlternateEnabledEvents,
                Strategy::MessageDuplicationLossDelay,
            ],
            3,
        );
        let r = neighborhood::explore(&mut adv, &adv_envelope(), &c)
            .expect("campaign")
            .into_record();
        for nb in r.neighbors() {
            assert!(
                !matches!(
                    nb.disposition,
                    Disposition::Rejected(
                        Rejection::MalformedSchedule
                            | Rejection::ViolatesCausalOrder { .. }
                            | Rejection::LeavesTraceClass { .. }
                    )
                ),
                "seed {seed}: {} {:?}",
                nb.id,
                nb.disposition
            );
        }
        let runs: Vec<(Strategy, Vec<usize>)> = r
            .neighbors()
            .iter()
            .filter(|nb| matches!(nb.disposition, Disposition::Executed(_)))
            .map(|nb| (nb.strategy, taken(&nb.witness)))
            .collect();
        let before = |o: &[usize], a: usize, b: usize| {
            o.iter().position(|x| *x == a) < o.iter().position(|x| *x == b)
        };
        for i in 0..n {
            for (j, anc_j) in anc.iter().enumerate().skip(i + 1) {
                let decision = (core.in_core(i) || core.in_core(j))
                    && (core.dependent(i, j) || core.dependent(j, i))
                    && !anc_j.contains(&i);
                if decision {
                    assert!(
                        runs.iter().any(|(s, o)| *s == Strategy::AlternateEnabledEvents
                            && before(o, j, i)),
                        "seed {seed}: decision ({i}, {j}) not taken"
                    );
                }
            }
        }
        for s in
            (0..n).filter(|&s| core.in_core(s) && matches!(core.class(s), EventClass::Send { .. }))
        {
            let free = (s + 1..n).filter(|&e| !anc[e].contains(&s)).count();
            let want = free.min(3).min(n - 1 - s);
            let got = r
                .neighbors()
                .iter()
                .filter(|nb| {
                    nb.id.contains(&format!(":delay({s},+"))
                        && matches!(nb.disposition, Disposition::Executed(_))
                })
                .count();
            assert_eq!(got, want, "seed {seed}: delays of send {s}");
        }
        if r.verdict() == Verdict::Complete {
            assert!(r.neighbors().iter().all(|nb| match &nb.disposition {
                Disposition::Rejected(x) => !x.is_engine_error(),
                _ => true,
            }));
        }
    }
}

#[test]
fn a_halted_run_is_decided_before_its_outcome_and_resumes_to_the_same_result() {
    // Low 3 of the pass: a halted, passing run with an oversized handle is not the
    // neighbor; that is decided before the handle is admitted, so the uninterrupted and
    // the resumed campaign agree byte for byte.
    let core = CoreOrder::new(
        vec![vec![], vec![], vec![]],
        &[(0, 1), (0, 2), (1, 2)],
        &[0, 1, 2],
        &BTreeMap::new(),
    )
    .expect("core");
    let mk = || Adv {
        core: core.clone(),
        halted_big_handle: true,
    };
    let c = adv_config(vec![Strategy::AlternateEnabledEvents], 0);
    let whole = neighborhood::explore(&mut mk(), &adv_envelope(), &c)
        .expect("campaign")
        .into_record();
    let mut small = c.clone();
    small.budget.runs = 1;
    let Exploration::Suspended { continuation, .. } =
        neighborhood::explore(&mut mk(), &adv_envelope(), &small).expect("campaign")
    else {
        panic!("suspends");
    };
    let resumed = neighborhood::resume(
        &mut mk(),
        &adv_envelope(),
        &c,
        &continuation,
        &mut Spent::default(),
    )
    .expect("resumes")
    .into_record();
    assert_eq!(whole.verdict(), resumed.verdict());
    assert_eq!(whole.result_bytes(), resumed.result_bytes());
    assert_eq!(
        whole.verdict(),
        Verdict::Inconclusive {
            reason: InconclusiveReason::Unsupported
        }
    );
}
