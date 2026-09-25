//! The kernel's definedness classification against the model core's (bn-iu8eh).
//!
//! # What is compared
//!
//! `continuum-kernel-core` classifies a carried model's definedness chains with its own
//! code (`crates/continuum-kernel-core/src/definedness.rs`), because the checking base
//! may not depend on `continuum-model-core` (INV-004, `tools/check_crate_boundaries.py`).
//! Both implement the rule `continuum_model_core::definedness` states: a chain guards
//! the predicate its base names only when it is well formed, and every other chain is
//! an action's (fail closed). Two implementations of one rule can disagree, and a
//! disagreement is either a false rejection or a certificate over an undefined read
//! that verifies. This file looks for one.
//!
//! The kernel's classification is private, so it is reached the only way anything
//! reaches it: through `check_certificate` on bytes. For each generated model, the test
//! writes a wire-epoch-2 certificate whose table is the model's whole declared domain
//! and whose rows are the model core's successors, once for the state domain and once
//! per declared predicate as the invariant. The expected verdict is derived here from
//! `continuum_model_core::definedness::Definedness::of` and the precedence of
//! `continuum_engine_reference::definedness` (an undefined action read, then an
//! undefined read in the invariant, then a violation), with the least table state of
//! each kind. The kernel must return exactly that verdict, predicate index included.
//!
//! A second sweep sets one definedness predicate false and every other predicate true.
//! Then the state-domain verdict names that predicate exactly when the model core reads
//! its chain as an action's, so each predicate's classification is compared alone.
//!
//! # The corpus
//!
//! A seeded stream (INV-005) of models over one variable `x` in `0..=3`. Chain bases
//! are drawn from ordinary names, action names, instance names `X(k=0)`, nested
//! instance names, and `#defined` itself. Chains have depth 1 to 3, with gaps (a member
//! whose shallower name is not declared), with or without their base declared, and
//! with action names that collide with a base, an intermediate or a member (`I`,
//! `I#defined`, `I(k=0)`, `P(a)(b)`). Predicate bodies are constants or comparisons of
//! `x`, so a predicate is false at some states and true at others.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects
)]

use continuum_engine_reference::{
    ActionDecl, BoolExpr, CmpOp, Definedness, Guarded, IntExpr, Model, ModelBuilder,
};
use continuum_kernel_core::check_certificate;
use continuum_kernel_core::verdict::{Rejection, Verdict};

/// A seeded xorshift stream.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }

    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }

    fn chance(&mut self, percent: u64) -> bool {
        self.below(100) < percent
    }

    fn pick<'a>(&mut self, items: &[&'a str]) -> &'a str {
        items[usize::try_from(self.below(items.len() as u64)).expect("small")]
    }
}

const TOP: i64 = 3;

/// Chain bases: ordinary names, names an action may take, instance names, nested
/// instance names, and a base that is itself the suffix.
const BASES: [&str; 9] = [
    "I", "J", "Inv", "A", "Step", "A(k=0)", "P(a)", "#defined", "I#x",
];

/// Action names: plain, the schema of an instance, instances, a nested instance, and
/// names that collide with a base, an intermediate or a member of a chain.
const ACTIONS: [&str; 10] = [
    "A",
    "Step",
    "A(k=0)",
    "A(k=1)",
    "P(a)(b)",
    "I",
    "I#defined",
    "J(k=0)",
    "Inv#defined#defined",
    "Other",
];

fn with_suffixes(base: &str, depth: usize) -> String {
    let mut name = base.to_owned();
    for _ in 0..depth {
        name.push_str("#defined");
    }
    name
}

fn body(rng: &mut Rng) -> BoolExpr {
    let c = i64::try_from(rng.below(4)).expect("small");
    match rng.below(7) {
        // Overflows where x >= 1: an evaluation fault, which outranks every finding.
        5 if rng.chance(20) => BoolExpr::compare(
            CmpOp::Ge,
            IntExpr::plus(IntExpr::var("x"), IntExpr::constant(i64::MAX)),
            IntExpr::constant(0),
        ),
        0 | 5 | 6 => BoolExpr::constant(true),
        1 => BoolExpr::constant(false),
        2 => BoolExpr::compare(CmpOp::Ne, IntExpr::var("x"), IntExpr::constant(c)),
        3 => BoolExpr::compare(CmpOp::Lt, IntExpr::var("x"), IntExpr::constant(c)),
        _ => BoolExpr::compare(CmpOp::Ge, IntExpr::var("x"), IntExpr::constant(c)),
    }
}

/// The names of one generated model: its actions and its predicates.
#[derive(Debug, Clone)]
struct Shape {
    actions: Vec<String>,
    predicates: Vec<String>,
}

fn shape(rng: &mut Rng) -> Shape {
    let mut actions: Vec<String> = Vec::new();
    for _ in 0..=rng.below(3) {
        let name = rng.pick(&ACTIONS).to_owned();
        if !actions.contains(&name) {
            actions.push(name);
        }
    }
    let mut predicates: Vec<String> = Vec::new();
    let mut add = |name: String| {
        if !predicates.contains(&name) {
            predicates.push(name);
        }
    };
    for _ in 0..=rng.below(3) {
        let base = rng.pick(&BASES);
        if rng.chance(70) {
            add(base.to_owned());
        }
        let deepest = usize::try_from(rng.below(3) + 1).expect("small");
        for depth in 1..=deepest {
            // Always the deepest member; each shallower one may be a gap.
            if depth == deepest || rng.chance(65) {
                add(with_suffixes(base, depth));
            }
        }
    }
    // Sometimes the definedness predicate of an action, and an ordinary predicate.
    if rng.chance(50) {
        let action = actions[usize::try_from(rng.below(actions.len() as u64)).unwrap()].clone();
        add(with_suffixes(&action, 1));
    }
    if rng.chance(60) {
        add("K".to_owned());
    }
    Shape {
        actions,
        predicates,
    }
}

fn build(shape: &Shape, bodies: &[BoolExpr]) -> Option<Model> {
    let mut builder = ModelBuilder::new()
        .variable("x", 0, TOP)
        .initial_state(&[("x", 0)]);
    for action in &shape.actions {
        builder = builder.action(ActionDecl::deterministic(
            action,
            BoolExpr::constant(true),
            vec![("x", IntExpr::constant(0))],
        ));
    }
    for (name, body) in shape.predicates.iter().zip(bodies) {
        builder = builder.predicate(name, body.clone());
    }
    builder.build().ok()
}

/// A wire-epoch-2 certificate, written for this test from the documented grammar.
fn certificate(model: &Model, rows: &[Vec<(u16, u32)>], invariant: Option<&str>) -> Vec<u8> {
    let mut out: Vec<u8> = Vec::new();
    let token = |out: &mut Vec<u8>, text: &str| {
        out.extend_from_slice(&u16::try_from(text.len()).expect("short").to_be_bytes());
        out.extend_from_slice(text.as_bytes());
    };
    out.extend_from_slice(b"CONTCERT");
    out.extend_from_slice(&2_u16.to_be_bytes());
    out.extend_from_slice(&1_u16.to_be_bytes());
    for field in [
        "blake3:definedness",
        "continuum-semantics-1",
        "blake3:definedness-property",
        "blake3:whole-domain",
        "blake3:empty-assumptions",
        "kernel-definedness-differential",
    ] {
        token(&mut out, field);
    }
    out.extend_from_slice(&2_u16.to_be_bytes());
    out.extend_from_slice(&0_u16.to_be_bytes());
    let identity = model.identity();
    let identity = identity.as_bytes();
    out.extend_from_slice(&u32::try_from(identity.len()).expect("small").to_be_bytes());
    out.extend_from_slice(identity);
    match invariant {
        Some(name) => {
            out.extend_from_slice(&2_u16.to_be_bytes());
            token(&mut out, name);
        }
        None => out.extend_from_slice(&1_u16.to_be_bytes()),
    }
    let states = TOP + 1;
    out.extend_from_slice(&u32::try_from(states).expect("small").to_be_bytes());
    for x in 0..states {
        out.extend_from_slice(&x.to_be_bytes());
    }
    for row in rows {
        out.extend_from_slice(&u32::try_from(row.len()).expect("small").to_be_bytes());
        for (action, target) in row {
            out.extend_from_slice(&action.to_be_bytes());
            out.extend_from_slice(&target.to_be_bytes());
        }
    }
    out
}

/// The model core's rows over the whole domain `0..=TOP`.
fn rows(model: &Model) -> Vec<Vec<(u16, u32)>> {
    (0..=TOP)
        .map(|x| {
            let state = model.state(&[x]).expect("a domain value is a state");
            let mut row: Vec<(u16, u32)> = model
                .successors(&state)
                .expect("constant updates stay in the domain")
                .iter()
                .map(|step| {
                    (
                        u16::try_from(step.action()).expect("few actions"),
                        u32::try_from(step.target().as_slice()[0]).expect("in domain"),
                    )
                })
                .collect();
            row.sort_unstable();
            row.dedup();
            row
        })
        .collect()
}

/// The verdict the model core's classification and the engine's precedence predict.
fn expected(model: &Model, definedness: &Definedness, subject: Option<usize>) -> Verdict2 {
    let holds = |index: usize, x: i64| {
        model
            .evaluate_predicate(index, &model.state(&[x]).expect("a state"))
            .map_err(|_| {
                Verdict2::Rejected(Rejection::EvaluationOverflow {
                    state: u32::try_from(x).expect("small"),
                })
            })
    };
    // The first fault in scan order ends the scan, as in the engine's `scan`.
    let false_at = |index: usize, x: i64| -> Result<bool, Verdict2> { Ok(!holds(index, x)?) };
    let mut action = None;
    let mut invariant = None;
    let mut violated = None;
    for x in 0..=TOP {
        let state = u32::try_from(x).expect("small");
        let mut undefined = None;
        for guard in definedness
            .action_chains()
            .flat_map(|chain| chain.iter().copied())
        {
            match false_at(guard, x) {
                Err(fault) => return fault,
                Ok(true) => {
                    undefined = Some(guard);
                    break;
                }
                Ok(false) => {}
            }
        }
        if let Some(guard) = undefined {
            action.get_or_insert((state, guard));
            continue;
        }
        let Some(subject) = subject else {
            continue;
        };
        let mut undefined = None;
        for &guard in definedness.guards_of(subject) {
            match false_at(guard, x) {
                Err(fault) => return fault,
                Ok(true) => {
                    undefined = Some(guard);
                    break;
                }
                Ok(false) => {}
            }
        }
        if let Some(guard) = undefined {
            invariant.get_or_insert((state, guard));
            continue;
        }
        let subject_false = match false_at(subject, x) {
            Err(fault) => return fault,
            Ok(value) => value,
        };
        if subject_false {
            if definedness.guards(subject).is_some() {
                invariant.get_or_insert((state, subject));
            } else {
                violated.get_or_insert(state);
            }
        }
    }
    let index = |i: usize| u32::try_from(i).expect("few predicates");
    if let Some((state, guard)) = action {
        return Verdict2::Rejected(Rejection::UndefinedActionRead {
            state,
            predicate: index(guard),
        });
    }
    if let Some((state, guard)) = invariant {
        return Verdict2::Rejected(Rejection::UndefinedInvariantRead {
            state,
            predicate: index(guard),
        });
    }
    if let Some(state) = violated {
        return Verdict2::Rejected(Rejection::InvariantViolated { state });
    }
    Verdict2::Verified
}

/// A verdict, reduced to what the comparison needs.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Verdict2 {
    Verified,
    Rejected(Rejection),
}

fn observed(verdict: &Verdict) -> Verdict2 {
    match verdict {
        Verdict::Verified(_) => Verdict2::Verified,
        Verdict::Rejected(rejection) => Verdict2::Rejected(rejection.clone()),
        Verdict::Unsupported(feature) => panic!("unsupported: {feature:?}"),
    }
}

#[derive(Debug, Default)]
struct Tally {
    models: usize,
    claims: usize,
    verified: usize,
    undefined_action: usize,
    undefined_invariant: usize,
    violated: usize,
    faulted: usize,
    action_chains: usize,
    predicate_chains: usize,
    sweep_action: usize,
    sweep_predicate: usize,
    sweep_subject: usize,
}

fn compare(model: &Model, tally: &mut Tally) {
    let definedness = Definedness::of(model);
    let rows = rows(model);
    let mut subjects: Vec<Option<usize>> = vec![None];
    subjects.extend((0..model.predicates().len()).map(Some));
    for subject in subjects {
        let name = subject.map(|i| model.predicates()[i].name().as_str());
        let want = expected(model, &definedness, subject);
        let got = observed(&check_certificate(&certificate(model, &rows, name)));
        assert_eq!(
            got,
            want,
            "claim {name:?}\nmodel {:?}",
            model
                .predicates()
                .iter()
                .map(|p| p.name().as_str())
                .collect::<Vec<_>>()
        );
        tally.claims += 1;
        match want {
            Verdict2::Verified => tally.verified += 1,
            Verdict2::Rejected(Rejection::UndefinedActionRead { .. }) => {
                tally.undefined_action += 1;
            }
            Verdict2::Rejected(Rejection::UndefinedInvariantRead { .. }) => {
                tally.undefined_invariant += 1;
            }
            Verdict2::Rejected(Rejection::EvaluationOverflow { .. }) => tally.faulted += 1,
            Verdict2::Rejected(_) => tally.violated += 1,
        }
    }
    tally.action_chains += definedness.action_chains().count();
    tally.predicate_chains += (0..model.predicates().len())
        .filter(|&i| matches!(definedness.guards(i), Some(Guarded::Predicate(_))))
        .count();
    tally.models += 1;
}

/// One definedness predicate false, every other predicate true: the state-domain claim
/// names it exactly when the model core reads its chain as an action's.
fn sweep(shape: &Shape, tally: &mut Tally) {
    let Some(probe) = build(
        shape,
        &vec![BoolExpr::constant(true); shape.predicates.len()],
    ) else {
        return;
    };
    let definedness = Definedness::of(&probe);
    for (index, predicate) in probe.predicates().iter().enumerate() {
        let Some(read) = definedness.guards(index) else {
            continue;
        };
        let mut model = ModelBuilder::new()
            .variable("x", 0, TOP)
            .initial_state(&[("x", 0)]);
        for action in &shape.actions {
            model = model.action(ActionDecl::deterministic(
                action,
                BoolExpr::constant(true),
                vec![("x", IntExpr::constant(0))],
            ));
        }
        for (other, p) in probe.predicates().iter().enumerate() {
            model = model.predicate(p.name().as_str(), BoolExpr::constant(other != index));
        }
        let model = model.build().expect("the probe built");
        let got = observed(&check_certificate(&certificate(
            &model,
            &rows(&model),
            None,
        )));
        let want = match read {
            Guarded::Action => {
                tally.sweep_action += 1;
                Verdict2::Rejected(Rejection::UndefinedActionRead {
                    state: 0,
                    predicate: u32::try_from(index).unwrap(),
                })
            }
            Guarded::Predicate(_) => {
                tally.sweep_predicate += 1;
                Verdict2::Verified
            }
        };
        assert_eq!(
            got,
            want,
            "{} alone false\nshape {shape:?}",
            predicate.name().as_str()
        );
        // Per subject: every claim over the same model. The false predicate refutes a
        // claim exactly when it is an action's, when it guards the claimed predicate
        // (the model core's `guards_of`), or when it is the claim itself.
        let rows = rows(&model);
        for (claim, p) in model.predicates().iter().enumerate() {
            let got = observed(&check_certificate(&certificate(
                &model,
                &rows,
                Some(p.name().as_str()),
            )));
            let guarded = definedness.guards_of(claim).contains(&index) || claim == index;
            let want = match read {
                Guarded::Action => Verdict2::Rejected(Rejection::UndefinedActionRead {
                    state: 0,
                    predicate: u32::try_from(index).unwrap(),
                }),
                Guarded::Predicate(_) if guarded => {
                    tally.sweep_subject += 1;
                    Verdict2::Rejected(Rejection::UndefinedInvariantRead {
                        state: 0,
                        predicate: u32::try_from(index).unwrap(),
                    })
                }
                Guarded::Predicate(_) => Verdict2::Verified,
            };
            assert_eq!(
                got,
                want,
                "{} alone false, claim {}\nshape {shape:?}",
                predicate.name().as_str(),
                p.name().as_str()
            );
        }
    }
}

#[test]
fn the_kernel_classifies_definedness_chains_as_the_model_core_does() {
    let mut rng = Rng(0x18e4_d3f0_0bad_c0de);
    let mut tally = Tally::default();
    while tally.models < 600 {
        let shape = shape(&mut rng);
        let bodies: Vec<BoolExpr> = shape.predicates.iter().map(|_| body(&mut rng)).collect();
        let Some(model) = build(&shape, &bodies) else {
            continue;
        };
        compare(&model, &mut tally);
        sweep(&shape, &mut tally);
    }
    println!("KERNEL-DEFINEDNESS-DIFFERENTIAL {tally:?}");
    // Every outcome and both classifications must occur, or agreement is vacuous.
    assert!(tally.verified >= 100, "{tally:?}");
    assert!(tally.undefined_action >= 100, "{tally:?}");
    assert!(tally.undefined_invariant >= 50, "{tally:?}");
    assert!(tally.violated >= 50, "{tally:?}");
    assert!(tally.action_chains >= 100, "{tally:?}");
    assert!(tally.predicate_chains >= 100, "{tally:?}");
    assert!(tally.sweep_action >= 100, "{tally:?}");
    assert!(tally.sweep_predicate >= 100, "{tally:?}");
    assert!(tally.sweep_subject >= 300, "{tally:?}");
    assert!(tally.faulted >= 50, "{tally:?}");
}

/// The named cases of the rule, each once: a well-formed chain, a gap, an unnamed
/// base, a collision at the base, at an intermediate and at a member, a schema, an
/// instance name in the chain, and a base that is the suffix itself.
#[test]
fn every_named_case_of_the_rule_agrees() {
    let cases: [(&[&str], &[&str], &str, bool); 10] = [
        (
            &["Step"],
            &["I", "I#defined", "I#defined#defined"],
            "I#defined",
            false,
        ),
        (
            &["Step"],
            &["I", "I#defined#defined"],
            "I#defined#defined",
            true,
        ),
        (&["Step"], &["I#defined"], "I#defined", true),
        (&["I"], &["I", "I#defined"], "I#defined", true),
        (
            &["I#defined"],
            &["I", "I#defined", "I#defined#defined"],
            "I#defined#defined",
            true,
        ),
        (
            &["I#defined#defined"],
            &["I", "I#defined", "I#defined#defined"],
            "I#defined",
            true,
        ),
        (&["I(k=0)"], &["I", "I#defined"], "I#defined", true),
        (
            &["A"],
            &["A(k=0)", "A(k=0)#defined"],
            "A(k=0)#defined",
            true,
        ),
        (
            &["P(a)(b)"],
            &["P(a)", "P(a)#defined"],
            "P(a)#defined",
            true,
        ),
        (
            &["Step"],
            &["#defined", "#defined#defined"],
            "#defined#defined",
            false,
        ),
    ];
    for (actions, predicates, false_one, is_action) in cases {
        let shape = Shape {
            actions: actions.iter().map(|s| (*s).to_owned()).collect(),
            predicates: predicates.iter().map(|s| (*s).to_owned()).collect(),
        };
        let bodies: Vec<BoolExpr> = predicates
            .iter()
            .map(|p| BoolExpr::constant(*p != false_one))
            .collect();
        let model = build(&shape, &bodies).expect("builds");
        let index = model
            .predicates()
            .iter()
            .position(|p| p.name().as_str() == false_one)
            .unwrap();
        let definedness = Definedness::of(&model);
        assert_eq!(
            definedness.guards(index) == Some(Guarded::Action),
            is_action,
            "the model core's reading of {false_one} under {actions:?}"
        );
        let got = observed(&check_certificate(&certificate(
            &model,
            &rows(&model),
            None,
        )));
        let want = if is_action {
            Verdict2::Rejected(Rejection::UndefinedActionRead {
                state: 0,
                predicate: u32::try_from(index).unwrap(),
            })
        } else {
            Verdict2::Verified
        };
        assert_eq!(got, want, "{false_one} under {actions:?}");
        let mut tally = Tally::default();
        compare(&model, &mut tally);
    }
}
