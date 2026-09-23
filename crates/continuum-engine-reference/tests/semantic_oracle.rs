//! The tiny exhaustive oracle's contract (bn-1zgs, docs/19 §2).
//!
//! - **Known answers.** Hand-built systems with counts worked out on paper: an
//!   independent pair has two interleavings and one Mazurkiewicz class; a dependent
//!   pair has two of each.
//! - **Determinism.** One seed gives byte-identical systems and artifacts.
//! - **Negative control.** Unmutated generated systems are clean, so a finding in a
//!   mutant is the mutation's.
//! - **Detection and shrinking.** Each seeded defect class is detected with a
//!   replayable witness, and shrinks to a 1-minimal system that still shows it.
//! - **Limits.** Above each bound the oracle refuses with a typed reason.

use std::collections::{BTreeMap, BTreeSet};

use continuum_engine_reference::expr::{BoolExpr, CmpOp, IntExpr};
use continuum_engine_reference::model::{ActionDecl, Model, ModelBuilder, State};
use continuum_engine_reference::semantic::{
    ActionMeta, Artifact, DefectClass, Fairness, Finding, Footprint, GenerateError, Limits,
    Measure, ObligationDecl, Refusal, Role, SeedError, Shape, SplitMix64, System, SystemParts,
    Trace, VarKind, Verdict, generate, run, seed_defect, shrink,
};

const SEEDS: std::ops::Range<u64> = 0..8;

fn eq(name: &str, value: i64) -> BoolExpr {
    BoolExpr::compare(CmpOp::Eq, IntExpr::var(name), IntExpr::constant(value))
}

/// A hand-built system over `model`: every variable data, every action process 0 with
/// its syntactic footprint, unless `fair` names it weakly fair.
fn hand(model: Model, obligations: Vec<ObligationDecl>, fair: &[&str]) -> System {
    let obligation_vars: BTreeSet<String> =
        obligations.iter().map(|o| o.variable.clone()).collect();
    let kinds: BTreeMap<String, VarKind> = model
        .variables()
        .iter()
        .map(|v| {
            let name = v.name().as_str().to_owned();
            let kind = if obligation_vars.contains(&name) {
                VarKind::Obligation
            } else {
                VarKind::Data
            };
            (name, kind)
        })
        .collect();
    let meta: BTreeMap<String, ActionMeta> = model
        .actions()
        .iter()
        .map(|a| {
            let name = a.name().as_str();
            (
                name.to_owned(),
                ActionMeta {
                    process: 0,
                    role: Role::Work,
                    footprint: Footprint::syntactic(a),
                    fairness: if fair.contains(&name) {
                        Fairness::Weak
                    } else {
                        Fairness::Unfair
                    },
                },
            )
        })
        .collect();
    System::new(SystemParts {
        model,
        kinds,
        meta,
        conflicts: BTreeSet::new(),
        obligations,
        phases: Vec::new(),
        lineage: vec!["hand".to_owned()],
    })
    .expect("hand-built system")
}

fn oracle(system: &System) -> Artifact {
    run(system, &Limits::TINY).expect("oracle within limits")
}

/// Replay a witness against the model: every step must be a real transition.
fn replays(system: &System, trace: &Trace) {
    let model = system.model();
    assert!(
        model.initial_states().contains(&trace.start),
        "starts at an initial state"
    );
    let mut current: State = trace.start.clone();
    for (action, target) in &trace.steps {
        let index = model.action_index(action).expect("declared action");
        let successors = model.action_successors(index, &current).expect("evaluates");
        assert!(
            successors.contains(target),
            "{action} does not reach {target}"
        );
        current = target.clone();
    }
}

fn replay_finding(system: &System, finding: &Finding) {
    match finding {
        Finding::FootprintReadEscape { .. } => {}
        Finding::FootprintWriteEscape { witness, .. }
        | Finding::IndependenceViolated { witness, .. }
        | Finding::QuiescenceLeak { witness, .. }
        | Finding::CompletionLeak { witness, .. }
        | Finding::PhaseRegression { witness, .. } => replays(system, witness),
        Finding::OpenForever { stem, cycle, .. } => {
            replays(system, stem);
            assert!(!cycle.is_empty(), "a lasso cycle has at least one step");
            let lap = Trace {
                start: stem.end().clone(),
                steps: cycle.clone(),
            };
            // The cycle starts at the stem's end and returns to it.
            let model = system.model();
            let mut current = lap.start.clone();
            for (action, target) in &lap.steps {
                let index = model.action_index(action).expect("declared action");
                let successors = model.action_successors(index, &current).expect("evaluates");
                assert!(successors.contains(target));
                current = target.clone();
            }
            assert_eq!(&current, stem.end(), "the cycle closes");
        }
    }
}

// ---------------------------------------------------------------------------
// known answers
// ---------------------------------------------------------------------------

#[test]
fn an_independent_pair_has_two_interleavings_and_one_class() {
    let model = ModelBuilder::new()
        .variable("x", 0, 1)
        .variable("y", 0, 1)
        .action(ActionDecl::deterministic(
            "a",
            eq("x", 0),
            vec![("x", IntExpr::constant(1))],
        ))
        .action(ActionDecl::deterministic(
            "b",
            eq("y", 0),
            vec![("y", IntExpr::constant(1))],
        ))
        .initial_state(&[("x", 0), ("y", 0)])
        .build()
        .expect("model");
    let artifact = oracle(&hand(model, Vec::new(), &[]));
    assert_eq!(artifact.states().len(), 4);
    assert_eq!(artifact.edges().len(), 4);
    assert_eq!(artifact.quiescent().len(), 1);
    assert!(artifact.dependent_pairs().is_empty());
    let counts = artifact.interleavings();
    assert_eq!(
        (counts.complete, counts.truncated, counts.classes),
        (2, 0, 1)
    );
    assert_eq!(artifact.verdict(), Verdict::Clean);
}

#[test]
fn a_dependent_pair_has_two_interleavings_and_two_classes() {
    // `a` and `b` race for `x`: whichever goes first disables the other.
    let model = ModelBuilder::new()
        .variable("x", 0, 2)
        .action(ActionDecl::deterministic(
            "a",
            eq("x", 0),
            vec![("x", IntExpr::constant(1))],
        ))
        .action(ActionDecl::deterministic(
            "b",
            eq("x", 0),
            vec![("x", IntExpr::constant(2))],
        ))
        .initial_state(&[("x", 0)])
        .build()
        .expect("model");
    let artifact = oracle(&hand(model, Vec::new(), &[]));
    assert_eq!(artifact.states().len(), 3);
    assert_eq!(artifact.dependent_pairs(), vec![("a", "b")]);
    let counts = artifact.interleavings();
    assert_eq!(
        (counts.complete, counts.truncated, counts.classes),
        (2, 0, 2)
    );
    // Truthful footprints: the dependence is declared, so it is not a finding.
    assert_eq!(artifact.verdict(), Verdict::Clean);
}

#[test]
fn a_weakly_fair_discharge_closes_the_lasso_and_an_unfair_one_does_not() {
    // `spin` loops forever; `release` discharges `ob`. Only fairness separates them.
    let build = || {
        ModelBuilder::new()
            .variable("ob", 0, 1)
            .variable("t", 0, 1)
            .action(ActionDecl::deterministic(
                "spin",
                BoolExpr::constant(true),
                vec![("t", IntExpr::minus(IntExpr::constant(1), IntExpr::var("t")))],
            ))
            .action(ActionDecl::deterministic(
                "release",
                eq("ob", 1),
                vec![("ob", IntExpr::constant(0))],
            ))
            .initial_state(&[("ob", 1), ("t", 0)])
            .build()
            .expect("model")
    };
    let obligation = || {
        vec![ObligationDecl {
            name: "held".to_owned(),
            variable: "ob".to_owned(),
            owner_phase: None,
        }]
    };
    let fair = hand(build(), obligation(), &["release"]);
    assert_eq!(oracle(&fair).verdict(), Verdict::Clean);

    let unfair = hand(build(), obligation(), &[]);
    let artifact = oracle(&unfair);
    let lasso = artifact
        .findings()
        .iter()
        .find(|f| matches!(f, Finding::OpenForever { .. }))
        .expect("an unfair release leaves the obligation open forever");
    replay_finding(&unfair, lasso);
    assert!(
        !artifact.has(DefectClass::Obligation),
        "no state is quiescent"
    );
}

// ---------------------------------------------------------------------------
// determinism
// ---------------------------------------------------------------------------

#[test]
fn one_seed_gives_byte_identical_systems_and_artifacts() {
    let mut encodings = BTreeSet::new();
    for seed in SEEDS {
        let first = generate(seed, Shape::TINY).expect("generate");
        let second = generate(seed, Shape::TINY).expect("generate");
        assert_eq!(first.encode(), second.encode());
        let bytes = oracle(&first).canonical_bytes();
        assert_eq!(bytes, oracle(&second).canonical_bytes());
        assert!(bytes.is_ascii());
        encodings.insert(first.encode());
    }
    assert!(
        encodings.len() > 1,
        "different seeds reach different systems"
    );
}

#[test]
fn the_stream_is_a_function_of_the_seed() {
    let draw = |seed| {
        let mut stream = SplitMix64::new(seed);
        (0..16).map(|_| stream.next_u64()).collect::<Vec<_>>()
    };
    assert_eq!(draw(42), draw(42));
    assert_ne!(draw(42), draw(43));
}

// ---------------------------------------------------------------------------
// negative control
// ---------------------------------------------------------------------------

#[test]
fn unmutated_generated_systems_are_clean() {
    let shapes = [
        Shape::TINY,
        Shape {
            processes: 1,
            data_variables: 2,
            data_max: 2,
            work_per_process: 2,
            extra_conflicts: 0,
        },
        Shape {
            processes: 2,
            data_variables: 2,
            data_max: 1,
            work_per_process: 2,
            extra_conflicts: 3,
        },
    ];
    for shape in shapes {
        for seed in SEEDS {
            let system = generate(seed, shape).expect("generate");
            let artifact = oracle(&system);
            assert_eq!(
                artifact.verdict(),
                Verdict::Clean,
                "seed {seed} {shape:?}: {:?}",
                artifact.findings()
            );
            let counts = artifact.interleavings();
            assert!(counts.complete + counts.truncated >= counts.classes);
            assert!(counts.classes > 0);
            if shape.processes >= 2 {
                assert!(
                    artifact.dependent_pairs().contains(&("p0_w0", "p1_w0")),
                    "the planted pair is semantically dependent"
                );
            }
        }
    }
}

// ---------------------------------------------------------------------------
// seeded defects
// ---------------------------------------------------------------------------

/// The classes a mutant of each class must *not* show, so detection is attributed.
fn excluded(class: DefectClass) -> &'static [DefectClass] {
    match class {
        DefectClass::Dependence => &[
            DefectClass::Fairness,
            DefectClass::Obligation,
            DefectClass::Cancellation,
        ],
        DefectClass::Fairness => &[
            DefectClass::Dependence,
            DefectClass::Obligation,
            DefectClass::Cancellation,
        ],
        // A completion leak also leaves the obligation open forever: Fairness may co-occur.
        DefectClass::Obligation => &[DefectClass::Dependence, DefectClass::Cancellation],
        DefectClass::Cancellation => &[DefectClass::Dependence],
    }
}

#[test]
fn every_seeded_defect_class_is_detected_with_a_replayable_witness() {
    for class in DefectClass::ALL {
        for seed in SEEDS {
            let base = generate(seed, Shape::TINY).expect("generate");
            assert!(!oracle(&base).has(class), "negative control, seed {seed}");
            let seeded = seed_defect(&base, class, seed).expect("a site exists");
            assert_eq!(seeded.class, class);
            let mutant = seeded.system;
            assert_ne!(mutant.encode(), base.encode());
            let artifact = oracle(&mutant);
            assert!(artifact.has(class), "{class:?} seed {seed} undetected");
            for other in excluded(class) {
                assert!(
                    !artifact.has(*other),
                    "{class:?} seed {seed} also shows {other:?}"
                );
            }
            for finding in artifact.findings() {
                replay_finding(&mutant, finding);
            }
        }
    }
}

#[test]
fn a_dependence_mutant_is_caught_by_the_diamond_not_only_the_footprint() {
    let base = generate(3, Shape::TINY).expect("generate");
    let mutant = seed_defect(&base, DefectClass::Dependence, 3)
        .expect("site")
        .system;
    assert!(mutant.claims_independent("p0_w0", "p1_w0"));
    let artifact = oracle(&mutant);
    assert!(artifact.findings().iter().any(|f| matches!(
        f,
        Finding::IndependenceViolated { first, second, .. } if first == "p0_w0" && second == "p1_w0"
    )));
    assert!(
        artifact
            .findings()
            .iter()
            .any(|f| matches!(f, Finding::FootprintWriteEscape { .. }))
    );
}

#[test]
fn every_seeded_defect_shrinks_to_a_one_minimal_witness() {
    for class in DefectClass::ALL {
        for seed in 0..3 {
            let base = generate(seed, Shape::TINY).expect("generate");
            let seeded = seed_defect(&base, class, seed).expect("site");
            let mutant = &seeded.system;
            let before = kinds(&run(mutant, &Limits::TINY).expect("runs"), class);
            let shrunk = shrink(mutant, class, &seeded.sites, &Limits::TINY).expect("shrinks");
            assert!(
                before.is_subset(&kinds(&shrunk.artifact, class)),
                "{class:?}: a kind was lost"
            );
            for site in &seeded.sites {
                assert!(
                    shrunk.system.model().action_index(site).is_some(),
                    "site {site} kept"
                );
            }
            assert!(shrunk.deletions > 0);
            assert!(
                shrunk.system.model().actions().len() < mutant.model().actions().len(),
                "{class:?}: nothing was removed"
            );
            // 1-minimality: no single remaining unprotected action can go.
            for action in shrunk.system.model().actions() {
                if seeded.sites.contains(action.name().as_str()) {
                    continue;
                }
                if let Ok(smaller) = shrunk.system.without_action(action.name().as_str()) {
                    let shows = run(&smaller, &Limits::TINY)
                        .is_ok_and(|a| before.is_subset(&kinds(&a, class)));
                    assert!(
                        !shows,
                        "{class:?} seed {seed}: {} was removable",
                        action.name()
                    );
                }
            }
            for finding in shrunk.artifact.findings() {
                replay_finding(&shrunk.system, finding);
            }
            // Shrinking is deterministic too.
            let again = shrink(mutant, class, &seeded.sites, &Limits::TINY).expect("shrinks");
            assert_eq!(
                again.artifact.canonical_bytes(),
                shrunk.artifact.canonical_bytes()
            );
        }
    }
}

fn kinds(artifact: &Artifact, class: DefectClass) -> BTreeSet<&'static str> {
    artifact
        .findings()
        .iter()
        .filter(|f| f.class() == class)
        .map(Finding::kind)
        .collect()
}

#[test]
fn shrinking_keeps_the_planted_defect_not_a_cheaper_one() {
    // Seed 3's minimal witnesses, spelled out: each is the mutation and the least
    // context that makes it observable.
    let base = generate(3, Shape::TINY).expect("generate");
    let actions = |class| {
        let seeded = seed_defect(&base, class, 3).expect("site");
        let shrunk = shrink(&seeded.system, class, &seeded.sites, &Limits::TINY).expect("shrinks");
        shrunk
            .system
            .model()
            .actions()
            .iter()
            .map(|a| a.name().as_str().to_owned())
            .collect::<Vec<_>>()
    };
    // Two racing stores declared independent: both stay, nothing else is needed.
    assert_eq!(actions(DefectClass::Dependence), ["p0_w0", "p1_w0"]);
    // The completion leak needs the whole path to Cancelled with the obligation held.
    let obligation = actions(DefectClass::Obligation);
    for suffix in ["acquire", "cancel", "drain", "finalize"] {
        assert!(
            obligation.iter().any(|a| a.ends_with(suffix)),
            "{obligation:?}"
        );
    }
    // The unfair release stays, next to something that can loop forever.
    let fairness = actions(DefectClass::Fairness);
    assert!(
        fairness.iter().any(|a| a.ends_with("_release")),
        "{fairness:?}"
    );
}

#[test]
fn a_single_process_has_no_dependence_site() {
    let shape = Shape {
        processes: 1,
        ..Shape::TINY
    };
    let base = generate(0, shape).expect("generate");
    assert_eq!(
        seed_defect(&base, DefectClass::Dependence, 0)
            .map(|seeded| seeded.system.encode())
            .expect_err("no cross-process pair"),
        SeedError::NoSite {
            class: DefectClass::Dependence
        }
    );
}

// ---------------------------------------------------------------------------
// limits
// ---------------------------------------------------------------------------

#[test]
fn a_state_space_above_the_limit_is_refused_before_exploration() {
    let big = Shape {
        processes: 4,
        data_variables: 4,
        data_max: 4,
        work_per_process: 1,
        extra_conflicts: 0,
    };
    let system = generate(0, big).expect("generate");
    // 8^4 per-process configurations times 5^4 data values.
    assert_eq!(
        run(&system, &Limits::TINY).expect_err("too large"),
        Refusal::TooLarge {
            measure: Measure::StateSpace,
            limit: 4096,
            observed: 4096 * 625,
            at_least: false,
        }
    );
}

#[test]
fn each_limit_refuses_with_its_own_measure() {
    let system = generate(1, Shape::TINY).expect("generate");
    let refused = |limits: Limits| run(&system, &limits).expect_err("refused");
    assert!(matches!(
        refused(Limits {
            max_variables: 3,
            ..Limits::TINY
        }),
        Refusal::TooLarge {
            measure: Measure::Variables,
            limit: 3,
            observed: 5,
            at_least: false
        }
    ));
    assert!(matches!(
        refused(Limits {
            max_actions: 4,
            ..Limits::TINY
        }),
        Refusal::TooLarge {
            measure: Measure::Actions,
            limit: 4,
            observed: 12,
            at_least: false
        }
    ));
    assert!(matches!(
        refused(Limits {
            max_interleavings: 10,
            ..Limits::TINY
        }),
        Refusal::TooLarge {
            measure: Measure::Interleavings,
            limit: 10,
            observed: 11,
            at_least: true
        }
    ));
    // Exactly at the limits it answers.
    let artifact = oracle(&system);
    assert!(u64::try_from(artifact.states().len()).unwrap() <= 4096);
}

#[test]
fn a_nondeterministic_action_is_unsupported_not_too_large() {
    let model = ModelBuilder::new()
        .variable("x", 0, 2)
        .action(ActionDecl::enumerated(
            "choose",
            eq("x", 0),
            vec![
                vec![("x", IntExpr::constant(1))],
                vec![("x", IntExpr::constant(2))],
            ],
        ))
        .initial_state(&[("x", 0)])
        .build()
        .expect("model");
    assert_eq!(
        run(&hand(model, Vec::new(), &[]), &Limits::TINY).expect_err("refused"),
        Refusal::NondeterministicAction {
            action: "choose".to_owned()
        }
    );
}

#[test]
fn a_shape_outside_its_range_is_refused_not_clamped() {
    for shape in [
        Shape {
            processes: 0,
            ..Shape::TINY
        },
        Shape {
            processes: 5,
            ..Shape::TINY
        },
        Shape {
            data_max: 0,
            ..Shape::TINY
        },
        Shape {
            extra_conflicts: 9,
            ..Shape::TINY
        },
    ] {
        assert!(matches!(
            generate(0, shape).expect_err("refused"),
            GenerateError::ShapeOutOfRange { .. }
        ));
    }
}
