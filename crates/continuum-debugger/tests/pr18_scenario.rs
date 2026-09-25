//! PR 18 (bn-25z9o): the owner, fault and value-domain passes of `scenario`, on a toy
//! configuration space whose failure condition is known, so each property of the engine
//! is checked against a closed-form answer. The register instantiation is
//! `continuum-asupersync/tests/pr18_impl02_scenario_reduction.rs`.

use continuum_debugger::reduce::{Budget, TranscriptBound};
use continuum_debugger::scenario::{
    Candidate, ConfigVerdict, Dimension, DimensionEnd, Preserved, Ran, Scenario, ScenarioGuarantee,
    ScenarioReduction, ScenarioRefusal, reduce_scenario,
};
use continuum_value::assurance::InconclusiveReason;

const BUDGET: Budget = Budget::new(1_000, 1 << 20);

/// A configuration: how many owners, faults and values.
type Config = [u64; 3];

/// A toy scenario: the failure reproduces when `fails` says so; each candidate removes
/// one unit along its dimension.
struct Toy {
    fails: fn(Config) -> bool,
    /// Dimensions the scenario refuses to reduce, with the reason.
    refuse: Option<(Dimension, &'static str)>,
    /// Omit `Preserved::Property` from every declaration.
    no_property: bool,
    /// Configurations outside the scope.
    out: fn(Config) -> bool,
    /// Configurations the scenario cannot decide.
    undecided: fn(Config) -> bool,
    /// Offer the current configuration itself as a candidate (a contract breach).
    offer_same: bool,
    /// Offer each candidate twice (the memo).
    offer_twice: bool,
    run_cost: u64,
    /// Added to every measure, and raised by one after each run when `drifts`: a measure
    /// that changes after a run.
    drift: std::cell::Cell<u64>,
    drifts: bool,
    /// Every configuration run, in order.
    runs: Vec<Config>,
}

impl Toy {
    fn new(fails: fn(Config) -> bool) -> Self {
        Self {
            fails,
            refuse: None,
            no_property: false,
            out: |_| false,
            undecided: |_| false,
            offer_same: false,
            offer_twice: false,
            run_cost: 1,
            drift: std::cell::Cell::new(0),
            drifts: false,
            runs: Vec::new(),
        }
    }
}

const fn index(d: Dimension) -> usize {
    match d {
        Dimension::Owner => 0,
        Dimension::Fault => 1,
        Dimension::Value => 2,
    }
}

impl Scenario for Toy {
    type Config = Config;

    fn declare(&self, dimension: Dimension) -> Result<Vec<Preserved>, String> {
        if let Some((d, why)) = self.refuse {
            if d == dimension {
                return Err(why.to_owned());
            }
        }
        if self.no_property {
            return Ok(vec![Preserved::Defect]);
        }
        Ok(vec![Preserved::Property, Preserved::Defect])
    }

    fn measure(&self, config: &Config) -> [u64; 3] {
        let d = self.drift.get();
        config.map(|x| x + d)
    }

    fn candidates(&self, config: &Config, dimension: Dimension) -> Vec<Candidate<Config>> {
        let i = index(dimension);
        let mut out = Vec::new();
        if self.offer_same {
            out.push(Candidate {
                config: *config,
                label: "the same".to_owned(),
            });
        }
        if config[i] > 0 {
            let mut c = *config;
            c[i] -= 1;
            out.push(Candidate {
                config: c,
                label: format!("{dimension:?} -1"),
            });
            if self.offer_twice {
                out.push(Candidate {
                    config: c,
                    label: format!("{dimension:?} -1 again"),
                });
            }
        }
        out
    }

    fn candidates_cost(&self, _config: &Config) -> u64 {
        4
    }

    fn admits(&self, _dimension: Dimension, config: &Config) -> Result<(), String> {
        if (self.out)(*config) {
            Err("outside the scope".to_owned())
        } else {
            Ok(())
        }
    }

    fn run_cost(&self, _config: &Config) -> u64 {
        self.run_cost
    }

    fn run(&mut self, config: &Config) -> Ran {
        self.runs.push(*config);
        if self.drifts {
            self.drift.set(self.drift.get() + 10);
        }
        if (self.undecided)(*config) {
            Ran::Inconclusive(InconclusiveReason::ResourceExhausted, "sampled".to_owned())
        } else if (self.fails)(*config) {
            Ran::Fails
        } else {
            Ran::Holds("decided".to_owned())
        }
    }
}

fn reduced(r: &ScenarioReduction<Config>) -> (Config, &[ScenarioGuarantee]) {
    match r {
        ScenarioReduction::Reduced {
            config, guarantees, ..
        } => (*config, guarantees),
        other => panic!("reduced: {other:?}"),
    }
}

const ALL_MINIMAL: [ScenarioGuarantee; 4] = [
    ScenarioGuarantee::Reproduces,
    ScenarioGuarantee::OwnerMinimal,
    ScenarioGuarantee::FaultMinimal,
    ScenarioGuarantee::ValueMinimal,
];

#[test]
fn reduces_each_dimension_to_its_minimum_and_claims_minimality() {
    let mut toy = Toy::new(|c| c[0] >= 2 && c[1] >= 1 && c[2] >= 2);
    let r = reduce_scenario(&mut toy, [5, 3, 4], BUDGET);
    let (config, guarantees) = reduced(&r);
    assert_eq!(config, [2, 1, 2]);
    assert_eq!(guarantees, ALL_MINIMAL);
    // Every kept version ran and failed.
    let ScenarioReduction::Reduced { versions, .. } = &r else {
        unreachable!()
    };
    assert_eq!(versions.len(), 1 + 3 + 2 + 2);
    assert!(versions.iter().all(|v| (toy.fails)(*v)));
}

/// A dropped fault can make an owner unnecessary: rounds repeat until one keeps nothing,
/// and the last round's verdicts are the minimality evidence.
#[test]
fn rounds_repeat_until_nothing_is_kept() {
    let mut toy = Toy::new(|c| c[0] > c[1] && c[2] >= 1);
    let r = reduce_scenario(&mut toy, [5, 3, 1], BUDGET);
    let (config, guarantees) = reduced(&r);
    assert_eq!(config, [1, 0, 1]);
    assert_eq!(guarantees, ALL_MINIMAL);
    let ScenarioReduction::Reduced {
        passes, attempts, ..
    } = &r
    else {
        unreachable!()
    };
    let rounds = passes.iter().map(|p| p.round).max().expect("passes");
    assert_eq!(
        rounds, 2,
        "round 1 drops owners again; round 2 keeps nothing"
    );
    assert!(
        passes
            .iter()
            .filter(|p| p.round == rounds)
            .all(|p| p.kept == 0 && p.end == DimensionEnd::Done)
    );
    // The transcript's entries rebuild the versions.
    let kept = attempts
        .entries
        .iter()
        .filter(|a| a.dimension.is_some() && a.verdict == ConfigVerdict::Fails)
        .count();
    let ScenarioReduction::Reduced { versions, .. } = &r else {
        unreachable!()
    };
    assert_eq!(kept + 1, versions.len());
}

/// INV-013: a pass the scenario cannot run without changing the checked property is
/// refused: it runs nothing and claims nothing. The others still run.
#[test]
fn a_pass_that_would_change_the_property_is_refused() {
    let mut toy = Toy::new(|c| c[0] >= 1 && c[1] >= 1 && c[2] >= 1);
    toy.refuse = Some((Dimension::Value, "the question names a value"));
    let r = reduce_scenario(&mut toy, [3, 3, 3], BUDGET);
    let (config, guarantees) = reduced(&r);
    assert_eq!(config, [1, 1, 3], "the value domain is untouched");
    assert!(!guarantees.contains(&ScenarioGuarantee::ValueMinimal));
    assert!(guarantees.contains(&ScenarioGuarantee::OwnerMinimal));
    let ScenarioReduction::Reduced {
        passes, attempts, ..
    } = &r
    else {
        unreachable!()
    };
    for p in passes.iter().filter(|p| p.dimension == Dimension::Value) {
        assert_eq!(
            p.end,
            DimensionEnd::ScopeRefused("the question names a value".to_owned())
        );
        assert_eq!((p.offered, p.runs), (0, 0));
    }
    assert!(
        attempts
            .entries
            .iter()
            .all(|a| a.dimension != Some(Dimension::Value))
    );
    assert!(toy.runs.iter().all(|c| c[2] == 3));
}

/// A declaration that does not name the checked property refuses every pass.
#[test]
fn a_declaration_without_the_property_refuses_the_pass() {
    let mut toy = Toy::new(|c| c[0] >= 1);
    toy.no_property = true;
    let r = reduce_scenario(&mut toy, [3, 3, 3], BUDGET);
    let (config, guarantees) = reduced(&r);
    assert_eq!(config, [3, 3, 3]);
    assert_eq!(guarantees, [ScenarioGuarantee::Reproduces]);
    assert_eq!(toy.runs, [[3, 3, 3]], "only the start ran");
}

/// A candidate outside the scope is recorded, never run and never kept; the minimality
/// claim is over the in-scope candidates.
#[test]
fn an_out_of_scope_candidate_is_recorded_not_run() {
    let mut toy = Toy::new(|_| true);
    toy.out = |c| c[2] < 2;
    let r = reduce_scenario(&mut toy, [1, 1, 3], BUDGET);
    let (config, guarantees) = reduced(&r);
    assert_eq!(config, [0, 0, 2]);
    assert_eq!(guarantees, ALL_MINIMAL);
    assert!(toy.runs.iter().all(|c| c[2] >= 2));
    let out: Vec<_> = r
        .attempts()
        .expect("attempts")
        .entries
        .iter()
        .filter(|a| a.verdict == ConfigVerdict::OutOfScope)
        .collect();
    assert!(!out.is_empty());
    assert!(out.iter().all(|a| a.reason == "outside the scope"));
}

/// An inconclusive candidate is not a non-failure: its dimension claims no minimality.
#[test]
fn an_undecided_candidate_withholds_minimality() {
    let mut toy = Toy::new(|c| c[1] >= 1);
    toy.undecided = |c| c[1] == 0;
    let r = reduce_scenario(&mut toy, [1, 2, 1], BUDGET);
    let (config, guarantees) = reduced(&r);
    assert_eq!(config, [0, 1, 0]);
    assert!(!guarantees.contains(&ScenarioGuarantee::FaultMinimal));
    assert!(guarantees.contains(&ScenarioGuarantee::OwnerMinimal));
    assert!(guarantees.contains(&ScenarioGuarantee::ValueMinimal));
}

/// A candidate that is not smaller breaks the scenario's contract: the reduction stops,
/// inconclusive with `EngineError`, with the last failing configuration.
#[test]
fn a_candidate_that_is_not_smaller_is_a_contract_breach() {
    let mut toy = Toy::new(|_| true);
    toy.offer_same = true;
    let r = reduce_scenario(&mut toy, [2, 2, 2], BUDGET);
    let ScenarioReduction::Inconclusive {
        reason,
        best,
        attempts,
        ..
    } = &r
    else {
        panic!("inconclusive: {r:?}");
    };
    assert_eq!(*reason, InconclusiveReason::EngineError);
    assert_eq!(*best, Some([2, 2, 2]));
    assert_eq!(
        attempts.entries.last().map(|a| a.verdict),
        Some(ConfigVerdict::NotSmaller)
    );
}

/// A repeated candidate is judged from the memo and recorded with the attempt that ran
/// it.
#[test]
fn a_repeated_candidate_is_judged_from_the_memo() {
    let mut toy = Toy::new(|c| c[0] >= 1);
    toy.offer_twice = true;
    let r = reduce_scenario(&mut toy, [1, 0, 0], BUDGET);
    let (config, _) = reduced(&r);
    assert_eq!(config, [1, 0, 0]);
    let entries = &r.attempts().expect("attempts").entries;
    let memo: Vec<_> = entries.iter().filter(|a| a.memo_of.is_some()).collect();
    assert!(!memo.is_empty());
    for m in memo {
        let of = m.memo_of.expect("memo");
        assert!(of < m.index);
        assert_eq!(
            entries[usize::try_from(of).expect("small")].verdict,
            m.verdict
        );
    }
    assert_eq!(toy.runs, [[1, 0, 0], [0, 0, 0]], "the repeat was not run");
}

/// The budget is charged before the work: a run the budget cannot pay for is never
/// made, and the result is the typed inconclusive with the last failing configuration.
#[test]
fn an_exhausted_budget_is_inconclusive_and_charged_before_work() {
    let mut toy = Toy::new(|c| c[0] >= 1);
    toy.run_cost = 100;
    let r = reduce_scenario(&mut toy, [3, 0, 0], Budget::new(1_000, 250));
    let ScenarioReduction::Inconclusive { reason, best, .. } = &r else {
        panic!("inconclusive: {r:?}");
    };
    assert_eq!(*reason, InconclusiveReason::ResourceExhausted);
    assert_eq!(*best, Some([2, 0, 0]));
    assert_eq!(toy.runs, [[3, 0, 0], [2, 0, 0]], "no unpaid run");
    let mut toy = Toy::new(|c| c[0] >= 1);
    let r = reduce_scenario(&mut toy, [3, 0, 0], Budget::new(0, 1 << 20));
    assert!(matches!(
        r,
        ScenarioReduction::Inconclusive { best: None, .. }
    ));
    assert!(toy.runs.is_empty());
}

#[test]
fn an_input_that_does_not_fail_is_refused() {
    let mut toy = Toy::new(|_| false);
    let r = reduce_scenario(&mut toy, [1, 1, 1], BUDGET);
    let ScenarioReduction::Refused {
        why,
        attempts,
        spent,
    } = &r
    else {
        panic!("refused: {r:?}");
    };
    assert_eq!(*why, ScenarioRefusal::InputHolds("decided".to_owned()));
    assert_eq!(attempts.entries.len(), 1, "the start's run is on record");
    assert_eq!(spent.replays, 1);
}

/// A transcript that runs out of room counts what it omits and withholds every
/// minimality class.
#[test]
fn a_full_transcript_is_counted_and_withholds_minimality() {
    let mut toy = Toy::new(|c| c[0] >= 1);
    let bound = TranscriptBound {
        entries: 2,
        units: 1 << 20,
    };
    let r = reduce_scenario(&mut toy, [3, 1, 1], BUDGET.with_transcript(bound));
    let (config, guarantees) = reduced(&r);
    assert_eq!(config, [1, 0, 0]);
    assert_eq!(guarantees, [ScenarioGuarantee::Reproduces]);
    let a = r.attempts().expect("attempts");
    assert_eq!(a.entries.len(), 2);
    assert!(a.omitted > 0);
}

#[test]
fn identical_inputs_give_identical_reductions() {
    let run = || {
        let mut toy = Toy::new(|c| c[0] > c[1] && c[2] >= 1);
        toy.undecided = |c| c == [2, 0, 1];
        reduce_scenario(&mut toy, [5, 3, 2], BUDGET)
    };
    assert_eq!(run(), run());
}

/// A measure that grows after each run cannot make the reduction cycle: the engine
/// compares each candidate with the measure it observed when it kept the version, so the
/// drifted candidate is not smaller, and the reduction stops as an engine error.
#[test]
fn a_measure_that_drifts_after_a_run_is_a_contract_breach() {
    let mut toy = Toy::new(|_| true);
    toy.drifts = true;
    let r = reduce_scenario(&mut toy, [3, 3, 3], Budget::new(u64::MAX, u64::MAX));
    let ScenarioReduction::Inconclusive { reason, best, .. } = &r else {
        panic!("inconclusive: {r:?}");
    };
    assert_eq!(*reason, InconclusiveReason::EngineError);
    assert_eq!(*best, Some([3, 3, 3]));
}
