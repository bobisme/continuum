//! The kernel's model evaluator against the model core's, over whole domains
//! (bn-35y4f, RFC 0005 "Kernel tests": "differential tests against engine evaluators").
//!
//! # What is compared
//!
//! A wire-epoch-2 certificate carries the model, and `continuum-kernel-core`
//! re-derives every successor row from it with its own decoder and evaluator
//! (`crates/continuum-kernel-core/src/model.rs`), which share no code with
//! `continuum-model-core`. Two independent evaluators of one semantics can disagree,
//! and a disagreement is either a false rejection or, worse, a relation both sides
//! get wrong in different ways. This file looks for one.
//!
//! The kernel's evaluator is private, so it is reached the only way anything reaches
//! it: through `check_certificate` on bytes. For each model, the test writes a
//! certificate whose table is the model's **whole declared domain** (not just the
//! reachable set) and whose rows are the model core's `successors` at every state.
//! The kernel verifies that certificate exactly when its own derivation equals the
//! model core's at every state of the domain. When the model core faults at a state
//! (overflow, or an update leaving the domain), the kernel must reject at that same
//! state, with an evaluation reason.
//!
//! Guards are random boolean expressions, so the relation also compares the two
//! boolean evaluators state by state; each declared predicate is also certified as
//! an invariant over the whole domain.
//!
//! # The corpus
//!
//! The Die Hard transcription, plus a deterministic stream of random models over
//! small domains: every operator of the expression language (checked `+ - *`, `min`,
//! `max`, the six comparisons, `not`, non-short-circuit `and`/`or`/`implies`, and
//! `in`), constants near `i64::MIN`/`i64::MAX` so overflow happens, and updates that
//! leave the domain so the domain rule fires. The seed is fixed (INV-005).

use continuum_engine_reference::diehard;
use continuum_engine_reference::expr::{BoolExpr, CmpOp, IntExpr};
use continuum_engine_reference::model::{ActionDecl, Model, ModelBuilder};

use continuum_kernel_core::check_certificate;
use continuum_kernel_core::verdict::{PropertyClass, Verdict};

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

    fn pick<'a, T>(&mut self, items: &'a [T]) -> &'a T {
        &items[usize::try_from(self.below(items.len() as u64)).expect("small")]
    }
}

const NAMES: [&str; 3] = ["a", "b", "c"];

fn int_expr(rng: &mut Rng, vars: usize, depth: u32) -> IntExpr {
    if depth <= 1 || rng.below(3) == 0 {
        return match rng.below(8) {
            0 => IntExpr::constant(*rng.pick(&[i64::MAX, i64::MIN, i64::MAX - 1, 1 << 62])),
            1..=3 => IntExpr::constant(i64::try_from(rng.below(7)).expect("small") - 3),
            _ => IntExpr::var(NAMES[usize::try_from(rng.below(vars as u64)).expect("small")]),
        };
    }
    let (left, right) = (
        int_expr(rng, vars, depth - 1),
        int_expr(rng, vars, depth - 1),
    );
    match rng.below(5) {
        0 => IntExpr::plus(left, right),
        1 => IntExpr::minus(left, right),
        2 => IntExpr::times(left, right),
        3 => IntExpr::min(left, right),
        _ => IntExpr::max(left, right),
    }
}

fn bool_expr(rng: &mut Rng, vars: usize, depth: u32) -> BoolExpr {
    if depth <= 1 || rng.below(3) == 0 {
        return match rng.below(4) {
            0 => BoolExpr::constant(rng.below(2) == 0),
            1 => {
                let lo = i64::try_from(rng.below(5)).expect("small") - 2;
                BoolExpr::in_range(int_expr(rng, vars, 2), lo, lo + 2)
            }
            _ => {
                let op = *rng.pick(&[
                    CmpOp::Eq,
                    CmpOp::Ne,
                    CmpOp::Lt,
                    CmpOp::Le,
                    CmpOp::Gt,
                    CmpOp::Ge,
                ]);
                BoolExpr::compare(
                    op,
                    int_expr(rng, vars, depth.min(3)),
                    int_expr(rng, vars, depth.min(3)),
                )
            }
        };
    }
    match rng.below(4) {
        0 => BoolExpr::negate(bool_expr(rng, vars, depth - 1)),
        1 => BoolExpr::and(
            bool_expr(rng, vars, depth - 1),
            bool_expr(rng, vars, depth - 1),
        ),
        2 => BoolExpr::or(
            bool_expr(rng, vars, depth - 1),
            bool_expr(rng, vars, depth - 1),
        ),
        _ => BoolExpr::implies(
            bool_expr(rng, vars, depth - 1),
            bool_expr(rng, vars, depth - 1),
        ),
    }
}

/// A random model, or `None` when the builder refuses the draw.
fn random_model(rng: &mut Rng) -> Option<Model> {
    let vars = usize::try_from(rng.below(3) + 1).expect("small");
    let mut builder = ModelBuilder::new();
    let mut init: Vec<(&str, i64)> = Vec::new();
    for name in NAMES.iter().take(vars) {
        let lo = -i64::try_from(rng.below(3)).expect("small");
        let hi = i64::try_from(rng.below(3)).expect("small");
        builder = builder.variable(name, lo, hi);
        init.push((name, lo));
    }
    builder = builder.initial_state(&init);
    let actions = rng.below(4) + 1;
    for index in 0..actions {
        let guard = bool_expr(rng, vars, 4);
        let outcomes = rng.below(3) + 1;
        let mut declared: Vec<Vec<(&str, IntExpr)>> = Vec::new();
        for _ in 0..outcomes {
            let mut outcome = Vec::new();
            for name in NAMES.iter().take(vars) {
                if rng.below(2) == 0 {
                    outcome.push((*name, int_expr(rng, vars, 4)));
                }
            }
            declared.push(outcome);
        }
        builder = builder.action(ActionDecl::enumerated(
            &format!("A{index}"),
            guard,
            declared,
        ));
    }
    for index in 0..rng.below(3) {
        builder = builder.predicate(&format!("P{index}"), bool_expr(rng, vars, 4));
    }
    builder.build().ok()
}

/// Every vector of the declared domain, in ascending (table) order.
fn whole_domain(model: &Model) -> Vec<Vec<i64>> {
    let mut states: Vec<Vec<i64>> = vec![Vec::new()];
    for variable in model.variables() {
        let (lo, hi) = (variable.domain().lo(), variable.domain().hi());
        states = states
            .into_iter()
            .flat_map(|prefix| {
                (lo..=hi).map(move |value| {
                    let mut next = prefix.clone();
                    next.push(value);
                    next
                })
            })
            .collect();
    }
    states
}

/// A wire-epoch-2 certificate, written for this test from the documented grammar.
fn certificate(
    model: &Model,
    states: &[Vec<i64>],
    rows: &[Vec<(u16, u32)>],
    invariant: Option<&str>,
) -> Vec<u8> {
    let mut out: Vec<u8> = Vec::new();
    let token = |out: &mut Vec<u8>, text: &str| {
        out.extend_from_slice(&u16::try_from(text.len()).expect("short").to_be_bytes());
        out.extend_from_slice(text.as_bytes());
    };
    out.extend_from_slice(b"CONTCERT");
    out.extend_from_slice(&2_u16.to_be_bytes());
    out.extend_from_slice(&1_u16.to_be_bytes());
    for field in [
        "blake3:differential",
        "continuum-semantics-1",
        "blake3:differential-property",
        "blake3:whole-domain",
        "blake3:empty-assumptions",
        "c018-kernel-evaluator-differential",
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
    out.extend_from_slice(&u32::try_from(states.len()).expect("small").to_be_bytes());
    for state in states {
        for value in state {
            out.extend_from_slice(&value.to_be_bytes());
        }
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

/// How one model compared.
#[derive(Debug, Default)]
struct Tally {
    verified: usize,
    rejected_at_the_same_state: usize,
    invariants_verified: usize,
    invariants_refuted: usize,
}

/// Compare the two evaluators over `model`'s whole domain.
fn compare(model: &Model, tally: &mut Tally) {
    let states = whole_domain(model);
    let mut rows: Vec<Vec<(u16, u32)>> = Vec::new();
    let mut first_fault: Option<usize> = None;
    for (index, vector) in states.iter().enumerate() {
        let state = model.state(vector).expect("a domain vector is a state");
        match model.successors(&state) {
            Ok(steps) => rows.push(
                steps
                    .iter()
                    .map(|step| {
                        let target = states
                            .binary_search(&step.target().as_slice().to_vec())
                            .expect("a successor stays in the domain");
                        (
                            u16::try_from(step.action()).expect("small"),
                            u32::try_from(target).expect("small"),
                        )
                    })
                    .collect(),
            ),
            Err(_) => {
                first_fault = Some(index);
                break;
            }
        }
    }
    while rows.len() < states.len() {
        rows.push(Vec::new());
    }

    let verdict = check_certificate(&certificate(model, &states, &rows, None));
    match first_fault {
        None => {
            let Verdict::Verified(claim) = &verdict else {
                panic!(
                    "the kernel refused the model core's relation over the whole domain: \
                     {verdict:?}\n{model:?}"
                );
            };
            let total: usize = rows.iter().map(Vec::len).sum();
            assert_eq!(claim.transitions(), total as u64);
            tally.verified += 1;
        }
        Some(fault) => {
            let Verdict::Rejected(rejection) = &verdict else {
                panic!("the model core faults at state {fault}, the kernel said {verdict:?}");
            };
            let at = match rejection {
                continuum_kernel_core::Rejection::EvaluationOverflow { state }
                | continuum_kernel_core::Rejection::UpdateOutsideDomain { state, .. } => {
                    usize::try_from(*state).expect("small")
                }
                other => panic!(
                    "the model core faults at state {fault}, the kernel rejected for another \
                     reason: {other:?}\n{model:?}"
                ),
            };
            assert_eq!(at, fault, "the two evaluators fault at different states");
            tally.rejected_at_the_same_state += 1;
            return;
        }
    }

    for (index, predicate) in model.predicates().iter().enumerate() {
        // The first state where the model core faults or finds the predicate false.
        let mut first: Option<(usize, bool)> = None;
        for (at, vector) in states.iter().enumerate() {
            let state = model.state(vector).expect("a domain vector is a state");
            match model.evaluate_predicate(index, &state) {
                Ok(true) => {}
                Ok(false) => {
                    first = Some((at, false));
                    break;
                }
                Err(_) => {
                    first = Some((at, true));
                    break;
                }
            }
        }
        let name = predicate.name().as_str();
        let verdict = check_certificate(&certificate(model, &states, &rows, Some(name)));
        match (&verdict, first) {
            (Verdict::Verified(claim), None) => {
                assert_eq!(claim.property(), PropertyClass::Invariant);
                tally.invariants_verified += 1;
            }
            (
                Verdict::Rejected(continuum_kernel_core::Rejection::InvariantViolated { state }),
                Some((at, false)),
            )
            | (
                Verdict::Rejected(continuum_kernel_core::Rejection::EvaluationOverflow { state }),
                Some((at, true)),
            ) if usize::try_from(*state).expect("small") == at => {
                tally.invariants_refuted += 1;
            }
            _ => panic!(
                "predicate {name}: model core first failure {first:?}, kernel \
                 {verdict:?}\n{model:?}"
            ),
        }
    }
}

#[test]
fn the_kernel_and_the_model_core_derive_the_same_die_hard_relation_over_its_whole_domain() {
    let model = diehard::model().expect("the Die Hard transcription is a valid model");
    let mut tally = Tally::default();
    compare(&model, &mut tally);
    assert_eq!(tally.verified, 1);
    // TypeOK holds on the whole domain; NotSolved does not.
    assert_eq!(tally.invariants_verified, 1);
    assert_eq!(tally.invariants_refuted, 1);
}

#[test]
fn the_kernel_and_the_model_core_agree_on_every_random_model() {
    let mut rng = Rng(0x0c01_8b35_4f00_d1ff);
    let mut tally = Tally::default();
    let mut models = 0_usize;
    while models < 400 {
        if let Some(model) = random_model(&mut rng) {
            compare(&model, &mut tally);
            models += 1;
        }
    }
    println!("C018-EVALUATOR-DIFFERENTIAL {tally:?}");
    // The corpus must exercise both sides of every comparison, or agreement is vacuous.
    assert!(tally.verified >= 50, "{tally:?}");
    assert!(tally.rejected_at_the_same_state >= 50, "{tally:?}");
    assert!(tally.invariants_verified >= 10, "{tally:?}");
    assert!(tally.invariants_refuted >= 10, "{tally:?}");
}
