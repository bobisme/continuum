//! PR 18 (bn-5kmuf): the mechanism check of `continuum_debugger::mechanism`, on synthetic
//! stories whose answer is known, and its enforcement at every entry point of `reduce`
//! and `scenario`. The register instantiation is `continuum-asupersync`'s
//! `tests/pr18_impl03_mechanism.rs`.

use continuum_debugger::mechanism::{
    Allowance, AllowanceExhausted, Edge, Guard, Mechanism, MechanismError, Mismatch, Order,
    Outcome, Rejection, Stage, Story, StoryReplay, Undecided, Validator, embed,
};
use continuum_debugger::reduce::{
    self, Budget, CausalOrder, Deletion, Reduction, Refusal, Replayed, Spent,
};
use continuum_debugger::scenario::{
    Candidate, ConfigVerdict, Dimension, Preserved, Ran, Scenario, ScenarioGuarantee,
    ScenarioReduction, reduce_scenario,
};
use continuum_value::assurance::InconclusiveReason;

fn edge(from: usize, to: usize, order: Order) -> Edge {
    Edge { from, to, order }
}

fn total(labels: &[&'static str]) -> Story<&'static str> {
    let preds = (0..labels.len()).map(|k| (0..k).collect()).collect();
    Story::new(labels.to_vec(), preds).expect("a total order")
}

fn run(m: &Mechanism<&'static str>, s: &Story<&'static str>) -> Result<Vec<usize>, Mismatch> {
    embed(
        m,
        s,
        &mut Allowance {
            ..Allowance::DEFAULT
        },
        &mut Spent::default(),
    )
    .expect("decided")
}

#[test]
fn malformed_mechanisms_are_refused() {
    let e = |a, b| edge(a, b, Order::Precedes);
    assert_eq!(
        Mechanism::<&str>::new(vec![], vec![], vec![]),
        Err(MechanismError::Empty)
    );
    assert_eq!(
        Mechanism::new(vec!["a"], vec![e(0, 1)], vec![]),
        Err(MechanismError::BadStep { constraint: 0 })
    );
    assert_eq!(
        Mechanism::new(vec!["a"], vec![e(0, 0)], vec![]),
        Err(MechanismError::BadStep { constraint: 0 })
    );
    assert_eq!(
        Mechanism::new(vec!["a", "b"], vec![e(0, 1), e(1, 0)], vec![]),
        Err(MechanismError::Cycle)
    );
    let g = Guard {
        after: 1,
        before: 0,
        forbidden: vec!["x"],
        until: vec![],
    };
    assert_eq!(
        Mechanism::new(vec!["a", "b"], vec![e(0, 1)], vec![g]),
        Err(MechanismError::GuardWithoutEdge { guard: 0 })
    );
    assert_eq!(
        Mechanism::new(vec!["a"; 65], vec![], vec![]),
        Err(MechanismError::TooLarge)
    );
}

/// Each mismatch is typed and names the first failing step, edge or guard.
#[test]
fn every_mismatch_is_typed() {
    let m = Mechanism::new(
        vec!["submit", "ack", "lose", "ack2"],
        vec![
            edge(0, 1, Order::HappensBefore),
            edge(0, 2, Order::Precedes),
            edge(2, 3, Order::Precedes),
        ],
        vec![Guard {
            after: 0,
            before: 1,
            forbidden: vec!["sync"],
            until: vec!["lose"],
        }],
    )
    .expect("a mechanism");
    assert_eq!(
        run(&m, &total(&["submit", "ack", "lose", "ack2"])),
        Ok(vec![0, 1, 2, 3])
    );
    assert_eq!(
        run(&m, &total(&["submit", "ack", "ack2"])),
        Err(Mismatch::MissingStep { step: 2 })
    );
    assert_eq!(
        run(&m, &total(&["submit", "ack", "ack2", "lose"])),
        Err(Mismatch::OrderBroken { edge: 2 })
    );
    assert_eq!(
        run(&m, &total(&["submit", "sync", "ack", "lose", "ack2"])),
        Err(Mismatch::GuardBroken { guard: 0 })
    );
    // A Sync after the loss is another write's: the guard ends at the loss.
    assert_eq!(
        run(&m, &total(&["submit", "lose", "sync", "ack", "ack2"])),
        Ok(vec![0, 3, 1, 4])
    );
    // Happens-before is the story's own order, not its positions.
    let unordered = Story::new(
        vec!["submit", "ack", "lose", "ack2"],
        vec![vec![], vec![], vec![0], vec![2]],
    )
    .expect("an order");
    assert_eq!(run(&m, &unordered), Err(Mismatch::OrderBroken { edge: 0 }));
    // Injective: two steps with one label need two positions.
    let twice = Mechanism::new(vec!["x", "x"], vec![], vec![]).expect("a mechanism");
    assert_eq!(
        run(&twice, &total(&["x"])),
        Err(Mismatch::MissingStep { step: 1 })
    );
    assert_eq!(run(&twice, &total(&["x", "y", "x"])), Ok(vec![0, 2]));
}

/// The search is charged before its work: an allowance that runs out is typed.
#[test]
fn an_exhausted_allowance_is_typed() {
    let m = Mechanism::new(vec!["a", "b"], vec![edge(0, 1, Order::Precedes)], vec![])
        .expect("a mechanism");
    let mut small = Allowance {
        replays: 1,
        work: 3,
    };
    assert_eq!(
        embed(&m, &total(&["a", "b"]), &mut small, &mut Spent::default()),
        Err(AllowanceExhausted)
    );
}

/// A validator over synthetic traces: the mechanism is one step at event `needed`, a
/// replay shows it when it holds that event, and `reject` makes every result after the
/// input a different mechanism.
struct Synthetic {
    needed: usize,
    mechanism: Result<Mechanism<usize>, Undecided>,
    reject_below: usize,
    stories: u64,
}

impl Synthetic {
    fn new(needed: usize) -> Self {
        Self {
            needed,
            mechanism: Ok(Mechanism::new(vec![needed], vec![], vec![]).expect("a mechanism")),
            reject_below: 0,
            stories: 0,
        }
    }
}

impl Validator<[usize]> for Synthetic {
    type Label = usize;

    fn mechanism(&mut self) -> Result<Mechanism<usize>, Undecided> {
        self.mechanism.clone()
    }

    fn story_cost(&self, kept: &[usize]) -> u64 {
        kept.len() as u64 + 1
    }

    fn story(&mut self, kept: &[usize]) -> StoryReplay<usize> {
        self.stories += 1;
        if !kept.contains(&self.needed) {
            return StoryReplay::Holds("no needed event".to_owned());
        }
        // Below the threshold, the failure comes through another step.
        let labels: Vec<usize> = if kept.len() < self.reject_below {
            kept.iter().map(|&e| e + 1000).collect()
        } else {
            kept.to_vec()
        };
        let preds = vec![Vec::new(); labels.len()];
        StoryReplay::Fails(Story::new(labels, preds).expect("a story"))
    }
}

fn fails_on(needed: usize) -> impl FnMut(&[usize]) -> Replayed {
    move |kept: &[usize]| {
        if kept.contains(&needed) {
            Replayed::Fails {
                witnesses: vec![needed],
            }
        } else {
            Replayed::Holds
        }
    }
}

/// Every entry point of `reduce` returns a core only through the check, and a result
/// the check rejects is a typed rejection, never a core.
#[test]
fn every_reduce_entry_point_goes_through_the_check() {
    let order = CausalOrder::from_predecessors(vec![Vec::new(); 12]).expect("an order");
    let whole: Vec<usize> = (0..12).collect();
    let entries: [&dyn Fn(&mut Synthetic) -> Reduction; 5] = [
        &|v| reduce::closure_pass(&order, &mut fails_on(7), v, Budget::new(1000, 1 << 30)),
        &|v| {
            reduce::deletion_pass(
                &order,
                whole.clone(),
                Deletion::Configurations,
                &mut fails_on(7),
                v,
                Budget::new(1000, 1 << 30),
            )
        },
        &|v| {
            reduce::deletion_pass(
                &order,
                whole.clone(),
                Deletion::Atoms,
                &mut fails_on(7),
                v,
                Budget::new(1000, 1 << 30),
            )
        },
        &|v| {
            reduce::minimize(
                &order,
                Deletion::Configurations,
                &mut fails_on(7),
                v,
                Budget::new(1000, 1 << 30),
            )
        },
        &|v| {
            reduce::minimize(
                &order,
                Deletion::Atoms,
                &mut fails_on(7),
                v,
                Budget::new(1000, 1 << 30),
            )
        },
    ];
    for (k, entry) in entries.iter().enumerate() {
        let mut ok = Synthetic::new(7);
        let r = entry(&mut ok);
        let core = r.core().unwrap_or_else(|| panic!("#{k}: {r:?}"));
        assert_eq!(core.events, vec![7]);
        assert_eq!(ok.stories, r.checks().expect("checks").len() as u64);
        assert_eq!(r.checks().expect("checks")[0].stage, Stage::Input);
        // Every result smaller than the input comes through another mechanism.
        let mut wrong = Synthetic::new(7);
        wrong.reject_below = 12;
        let r = entry(&mut wrong);
        assert!(
            matches!(
                r,
                Reduction::Rejected {
                    why: Rejection::Mechanism(Mismatch::MissingStep { step: 0 }),
                    ..
                }
            ),
            "#{k}: {r:?}"
        );
        assert!(r.core().is_none());
        // The input itself shows another mechanism: refused, nothing reduced.
        let mut never = Synthetic::new(7);
        never.reject_below = 13;
        assert!(matches!(
            entry(&mut never),
            Reduction::Refused(Refusal::MechanismNotInInput(Rejection::Mechanism(_)))
        ));
        // A validator that cannot decide: INV-008 inconclusive, never a core.
        let mut undecided = Synthetic::new(7);
        undecided.mechanism = Err(Undecided {
            reason: InconclusiveReason::AbstractionAmbiguity,
            detail: "two readings".to_owned(),
        });
        assert!(matches!(
            entry(&mut undecided),
            Reduction::Inconclusive {
                reason: InconclusiveReason::AbstractionAmbiguity,
                ..
            }
        ));
    }
    // The allowance is apart from the passes: none left is undecided, not a pass.
    let mut v = Synthetic::new(7);
    let r = reduce::minimize(
        &order,
        Deletion::Atoms,
        &mut fails_on(7),
        &mut v,
        Budget::new(1000, 1 << 30).with_validation(0, 1 << 30),
    );
    assert!(matches!(
        r,
        Reduction::Inconclusive {
            reason: InconclusiveReason::ResourceExhausted,
            ..
        }
    ));
    assert_eq!(v.stories, 0);
}

/// A toy scenario whose failure reproduces while the owner count is at least one, but
/// whose failing run below `lost_below` owners shows another mechanism.
struct Lossy {
    lost_below: u64,
    exhausted: bool,
    /// What the start's run answers, if not `Fails`.
    start: Option<Ran>,
}

impl Scenario for Lossy {
    type Config = [u64; 3];

    fn declare(&self, _d: Dimension) -> Result<Vec<Preserved>, String> {
        Ok(vec![Preserved::Property])
    }

    fn measure(&self, c: &[u64; 3]) -> [u64; 3] {
        *c
    }

    fn candidates(&self, c: &[u64; 3], d: Dimension) -> Vec<Candidate<[u64; 3]>> {
        let i = Dimension::ALL
            .iter()
            .position(|x| *x == d)
            .expect("a dimension");
        if c[i] == 0 {
            return Vec::new();
        }
        let mut n = *c;
        n[i] -= 1;
        vec![Candidate {
            config: n,
            label: format!("{d:?} -1"),
        }]
    }

    fn candidates_cost(&self, _c: &[u64; 3]) -> u64 {
        1
    }

    fn admits(&self, _d: Dimension, _c: &[u64; 3]) -> Result<(), String> {
        Ok(())
    }

    fn run_cost(&self, _c: &[u64; 3]) -> u64 {
        1
    }

    fn run(&mut self, c: &[u64; 3]) -> Ran {
        if c[1] == 9 {
            if let Some(r) = &self.start {
                return r.clone();
            }
        }
        if c[0] >= 1 {
            Ran::Fails
        } else {
            Ran::Holds("no owner".to_owned())
        }
    }

    fn search_exhausted(&self, _c: &[u64; 3]) -> bool {
        self.exhausted
    }
}

impl Validator<[u64; 3]> for Lossy {
    type Label = &'static str;

    fn mechanism(&mut self) -> Result<Mechanism<&'static str>, Undecided> {
        Ok(Mechanism::new(vec!["m"], vec![], vec![]).expect("a mechanism"))
    }

    fn story_cost(&self, _c: &[u64; 3]) -> u64 {
        1
    }

    fn story(&mut self, c: &[u64; 3]) -> StoryReplay<&'static str> {
        let label = if c[0] < self.lost_below { "other" } else { "m" };
        StoryReplay::Fails(Story::new(vec![label], vec![Vec::new()]).expect("a story"))
    }
}

/// The scenario keeps a candidate only when the check preserves its failing run: a
/// candidate whose failure comes through another mechanism is recorded, typed, and never
/// kept. The rejection decides the candidate only when the scenario tried every run.
#[test]
fn a_scenario_candidate_through_another_mechanism_is_never_kept() {
    for exhausted in [true, false] {
        let mut s = Lossy {
            lost_below: 3,
            exhausted,
            start: None,
        };
        let r = reduce_scenario(&mut s, [4, 0, 0], Budget::new(100, 1 << 20));
        let ScenarioReduction::Reduced {
            config,
            guarantees,
            attempts,
            passes,
            ..
        } = &r
        else {
            panic!("reduced: {r:?}");
        };
        assert_eq!(
            *config.subject(),
            [3, 0, 0],
            "kept only while the mechanism holds"
        );
        let lost = attempts
            .entries
            .iter()
            .find(|a| matches!(a.verdict, ConfigVerdict::MechanismLost { .. }))
            .expect("the rejected candidate is recorded");
        assert_eq!(
            lost.verdict,
            ConfigVerdict::MechanismLost {
                why: continuum_debugger::scenario::LostKind::Mechanism(Mismatch::MissingStep {
                    step: 0
                }),
                decided: exhausted,
            }
        );
        assert!(passes.iter().any(|p| p.mechanism_lost == 1));
        assert_eq!(
            guarantees.contains(&ScenarioGuarantee::OwnerMinimal),
            exhausted,
            "an undecided rejection withholds OwnerMinimal"
        );
        assert!(
            r.checks()
                .expect("checks")
                .iter()
                .any(|e| matches!(e.outcome, Outcome::Rejected(Rejection::Mechanism(_), _)))
        );
    }
}

/// The start's refusal keeps its kind: an inconclusive start is not a start that holds,
/// and no start is refused as holding when it is no run.
#[test]
fn a_start_refusal_keeps_its_kind() {
    use continuum_debugger::scenario::ScenarioRefusal;
    for (ran, want) in [
        (
            Ran::Inconclusive(InconclusiveReason::ResourceExhausted, "sampled".to_owned()),
            "inconclusive",
        ),
        (Ran::NotARun("no run".to_owned()), "not a run"),
        (Ran::Holds("holds".to_owned()), "holds"),
    ] {
        let mut s = Lossy {
            lost_below: 0,
            exhausted: true,
            start: Some(ran),
        };
        let r = reduce_scenario(&mut s, [4, 9, 0], Budget::new(100, 1 << 20));
        let ScenarioReduction::Refused { why, .. } = &r else {
            panic!("refused: {r:?}");
        };
        let got = match why {
            ScenarioRefusal::InputInconclusive(..) => "inconclusive",
            ScenarioRefusal::InputNotARun(_) => "not a run",
            ScenarioRefusal::InputHolds(_) => "holds",
            ScenarioRefusal::MechanismNotInInput(_) => "mechanism",
        };
        assert_eq!(got, want);
    }
}

/// A validation allowance that runs out mid-reduction stops the scenario reduction,
/// inconclusive, rather than let it spend runs it can never keep.
#[test]
fn an_exhausted_validation_allowance_stops_the_scenario_reduction() {
    let mut s = Lossy {
        lost_below: 0,
        exhausted: true,
        start: None,
    };
    let r = reduce_scenario(
        &mut s,
        [4, 0, 0],
        Budget::new(100, 1 << 20).with_validation(1, 1 << 20),
    );
    assert!(
        matches!(
            r,
            ScenarioReduction::Inconclusive {
                reason: InconclusiveReason::ResourceExhausted,
                ..
            }
        ),
        "{r:?}"
    );
}
