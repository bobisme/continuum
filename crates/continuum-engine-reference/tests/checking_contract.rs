//! What checking promises, on models built to test one promise each (PR 8, IMPL-03).
//!
//! Die Hard is in `tests/checking_diehard.rs`; it is the corpus model and its answers
//! are frozen elsewhere. The models here are the opposite kind, and follow the
//! discipline `tests/bfs_contract.rs` set: each is the smallest declaration that
//! exercises one rule, and each is *linear* in the rule it tests — a chain, a
//! self-loop, a two-way branch. Nothing here is combinatorial, and nothing here needs
//! to be, because the rules are independent.
//!
//! The suite covers: both deadlock policies over the same exploration, a deadlock
//! found inside a bounded run and inside its *unexpanded* frontier, a predicate that
//! cannot be evaluated at a reachable state, the refused-request error, the obligation
//! set's own rules, the verdict fold including its vacuous case, the INV-008 reason
//! spellings, the rendering, and determinism.

use std::collections::BTreeSet;

use continuum_engine_reference::bfs::{self, Bound, Bounds, Exploration};
use continuum_engine_reference::checking::{
    self, CheckError, CheckOutcome, CheckReport, DeadlockOutcome, DeadlockPolicy, Evidence,
    Obligations, Scope, Unresolved, Verdict,
};
use continuum_engine_reference::expr::{ArithOp, BoolExpr, CmpOp, EvalError, IntExpr};
use continuum_engine_reference::model::{ActionDecl, EvaluationError, Model, ModelBuilder, State};
use continuum_engine_reference::witness::NoWitness;

// ---------------------------------------------------------------------------
// the models
// ---------------------------------------------------------------------------

/// A chain: `n` counts from `0` to `limit`, one step at a time, and then stops.
///
/// The last state is the only terminal one, and the guard is the only thing that makes
/// it terminal. `Positive` is false at exactly one state — the initial one — so the
/// shallowest violation of it is at depth 0, which is a witness with no steps rather
/// than an absent witness.
fn counter(limit: i64) -> Model {
    ModelBuilder::new()
        .variable("n", 0, limit)
        .initial_state(&[("n", 0)])
        .action(ActionDecl::deterministic(
            "Step",
            BoolExpr::compare(CmpOp::Lt, IntExpr::var("n"), IntExpr::constant(limit)),
            vec![("n", IntExpr::plus(IntExpr::var("n"), IntExpr::constant(1)))],
        ))
        .predicate("InRange", BoolExpr::in_range(IntExpr::var("n"), 0, limit))
        .predicate(
            "Positive",
            BoolExpr::compare(CmpOp::Gt, IntExpr::var("n"), IntExpr::constant(0)),
        )
        .build()
        .expect("the counter is a valid model")
}

/// One state, one always-enabled action that assigns nothing: no state is terminal,
/// and the set closes at one.
fn self_loop() -> Model {
    ModelBuilder::new()
        .variable("x", 0, 1)
        .initial_state(&[("x", 0)])
        .action(ActionDecl::deterministic(
            "Stay",
            BoolExpr::Const(true),
            vec![],
        ))
        .predicate(
            "Zero",
            BoolExpr::compare(CmpOp::Eq, IntExpr::var("x"), IntExpr::constant(0)),
        )
        .build()
        .expect("the self-loop is a valid model")
}

/// A two-way branch whose *larger* successor is the terminal one.
///
/// From `0`, `Fork` reaches `1` and `2`. `2` has no enabled action; `1` continues to
/// `3`, which spins. Ascending admission puts `1` at the head of the queue, so a state
/// bound that trips while `1` is being expanded leaves the terminal state `2`
/// discovered and **never expanded** — which is the case that decides whether a
/// deadlock report reads the exploration's expansions or recomputes the rows.
fn branch() -> Model {
    ModelBuilder::new()
        .variable("n", 0, 3)
        .initial_state(&[("n", 0)])
        .action(ActionDecl::enumerated(
            "Fork",
            BoolExpr::compare(CmpOp::Eq, IntExpr::var("n"), IntExpr::constant(0)),
            vec![
                vec![("n", IntExpr::constant(1))],
                vec![("n", IntExpr::constant(2))],
            ],
        ))
        .action(ActionDecl::deterministic(
            "Walk",
            BoolExpr::compare(CmpOp::Eq, IntExpr::var("n"), IntExpr::constant(1)),
            vec![("n", IntExpr::constant(3))],
        ))
        .action(ActionDecl::deterministic(
            "Spin",
            BoolExpr::compare(CmpOp::Eq, IntExpr::var("n"), IntExpr::constant(3)),
            vec![],
        ))
        .predicate("Always", BoolExpr::Const(true))
        .build()
        .expect("the branch is a valid model")
}

/// A chain carrying a predicate that cannot be *evaluated* at a reachable state.
///
/// `Overflows` is fine at `n == 0` and overflows `i64` from `n == 1` on. The model is
/// well formed and the exploration succeeds; it is the check that fails, which is the
/// separation this file is testing.
fn unevaluable() -> Model {
    ModelBuilder::new()
        .variable("n", 0, 2)
        .initial_state(&[("n", 0)])
        .action(ActionDecl::deterministic(
            "Step",
            BoolExpr::compare(CmpOp::Lt, IntExpr::var("n"), IntExpr::constant(2)),
            vec![("n", IntExpr::plus(IntExpr::var("n"), IntExpr::constant(1)))],
        ))
        .predicate("Bounded", BoolExpr::in_range(IntExpr::var("n"), 0, 2))
        .predicate(
            "Overflows",
            BoolExpr::compare(
                CmpOp::Gt,
                IntExpr::plus(IntExpr::var("n"), IntExpr::constant(i64::MAX)),
                IntExpr::constant(0),
            ),
        )
        .build()
        .expect("the overflowing predicate is a valid model")
}

fn explore(model: &Model, bounds: Bounds) -> Exploration {
    bfs::explore(model, bounds).expect("this model evaluates everywhere")
}

fn complete(model: &Model) -> Exploration {
    explore(model, Bounds::CERTIFIABLE)
}

fn state(model: &Model, vector: &[i64]) -> State {
    model
        .state(vector)
        .unwrap_or_else(|error| panic!("{vector:?} is a state of this model: {error}"))
}

fn index(model: &Model, name: &str) -> usize {
    model
        .predicate_index(name)
        .unwrap_or_else(|| panic!("{name} is declared"))
}

fn check(model: &Model, exploration: &Exploration, obligations: &Obligations) -> CheckReport {
    checking::check(model, exploration, obligations).expect("every obligation is declared")
}

fn outcome(report: &CheckReport, at: usize) -> &CheckOutcome {
    report
        .invariant(at)
        .unwrap_or_else(|| panic!("predicate {at} was upheld"))
        .outcome()
}

fn vectors(states: &[State]) -> Vec<Vec<i64>> {
    states.iter().map(|s| s.as_slice().to_vec()).collect()
}

// ---------------------------------------------------------------------------
// the closed answers
// ---------------------------------------------------------------------------

/// The chain, checked whole: one invariant holds over five states, the other is
/// refuted at the initial state, and the last state is a deadlock under the policy
/// that says so.
#[test]
fn the_chain_is_checked_whole() {
    let model = counter(4);
    let exploration = complete(&model);
    let report = check(
        &model,
        &exploration,
        &Obligations::every_predicate(&model, DeadlockPolicy::Defect),
    );
    assert_eq!(report.scope(), Scope::Complete { states: 5 });
    assert_eq!(
        outcome(&report, index(&model, "InRange")),
        &CheckOutcome::Holds { states: 5 }
    );
    assert_eq!(report.verdict(), Verdict::Refuted);
}

/// A violation at depth 0 is a witness with no steps, not an absent witness. The
/// distinction is [`Witness::is_empty`]'s, and a report has to preserve it or a
/// property violated by an initial state would look unwitnessed.
#[test]
fn a_violation_at_an_initial_state_has_an_empty_witness() {
    let model = counter(4);
    let exploration = complete(&model);
    let report = check(
        &model,
        &exploration,
        &Obligations::new(DeadlockPolicy::Allowed).invariant(index(&model, "Positive")),
    );
    let CheckOutcome::Violated {
        state: at,
        depth,
        violations,
        evidence,
    } = outcome(&report, index(&model, "Positive"))
    else {
        panic!("Positive is false at n == 0");
    };
    assert_eq!(at, &state(&model, &[0]));
    assert_eq!(*depth, 0);
    assert_eq!(*violations, 1);
    let Evidence::Shortest(found) = evidence else {
        panic!("the exploration is closed");
    };
    assert!(found.is_empty(), "the initial state needs no steps");
    assert_eq!(found.start(), &state(&model, &[0]));
    assert_eq!(found.target(), &state(&model, &[0]));
}

/// No state of the self-loop model is terminal, so the strictest policy establishes
/// deadlock freedom rather than reporting nothing found.
#[test]
fn a_self_loop_is_not_a_deadlock() {
    let model = self_loop();
    let exploration = complete(&model);
    let report = check(
        &model,
        &exploration,
        &Obligations::every_predicate(&model, DeadlockPolicy::Defect),
    );
    assert_eq!(report.deadlock(), &DeadlockOutcome::Free { states: 1 });
    assert_eq!(report.verdict(), Verdict::Established);
}

// ---------------------------------------------------------------------------
// the policy is the caller's
// ---------------------------------------------------------------------------

/// One exploration, two policies, two different reports — and the difference is a
/// judgement, not a discovery. Under `Defect` the terminal state is reported with a
/// path; under `Allowed` it is counted and nothing is claimed about it.
#[test]
fn the_same_terminal_state_is_a_defect_or_is_not_by_declaration() {
    let model = counter(4);
    let exploration = complete(&model);

    let defect = check(
        &model,
        &exploration,
        &Obligations::new(DeadlockPolicy::Defect),
    );
    let DeadlockOutcome::Deadlocked { states } = defect.deadlock() else {
        panic!("n == 4 has no enabled action");
    };
    assert_eq!(states.len(), 1);
    let found = states.first().expect("one deadlock");
    assert_eq!(found.state(), &state(&model, &[4]));
    assert_eq!(found.depth(), 4);
    let Evidence::Shortest(path) = found.evidence() else {
        panic!("the exploration is closed");
    };
    assert_eq!(path.len(), 4);
    assert_eq!(
        path.steps()
            .iter()
            .map(|step| step.name().as_str().to_owned())
            .collect::<Vec<_>>(),
        vec!["Step"; 4]
    );
    assert_eq!(path.target(), &state(&model, &[4]));
    assert_eq!(defect.verdict(), Verdict::Refuted);

    let allowed = check(
        &model,
        &exploration,
        &Obligations::new(DeadlockPolicy::Allowed),
    );
    assert_eq!(
        allowed.deadlock(),
        &DeadlockOutcome::NotJudged { terminal: 1 },
        "the state is still counted; it is simply not judged"
    );
    assert!(allowed.deadlock().deadlocks().is_empty());
    assert_eq!(allowed.verdict(), Verdict::Established);
    assert_eq!(allowed.policy(), DeadlockPolicy::Allowed);
}

/// The two policies this layer distinguishes, and their stable names. `Defect` is the
/// intent contract's `deadlock-violation` exactly; `Allowed` covers two of that
/// vocabulary's members, so it is deliberately spelled in neither's words.
#[test]
fn the_policy_vocabulary_is_closed_and_named() {
    assert_eq!(DeadlockPolicy::ALL.len(), 2);
    assert_eq!(DeadlockPolicy::Defect.as_str(), "defect");
    assert_eq!(DeadlockPolicy::Allowed.as_str(), "allowed");
    assert_eq!(DeadlockPolicy::Defect.to_string(), "defect");
    let names: BTreeSet<&str> = DeadlockPolicy::ALL.iter().map(|p| p.as_str()).collect();
    assert_eq!(names.len(), DeadlockPolicy::ALL.len(), "distinct spellings");
}

// ---------------------------------------------------------------------------
// a deadlock inside a bounded run
// ---------------------------------------------------------------------------

/// The terminal state is discovered, never expanded, and reported anyway.
///
/// This is the test the deadlock scan's design rests on: `n == 2` sits in the frontier
/// when the state bound trips, so an implementation that read the walk's expansions
/// would miss it. Terminality is a fact about one state and the model — an empty
/// [`Model::successors`] — and the row is recomputed for every discovered state.
#[test]
fn a_terminal_state_in_the_frontier_is_still_reported() {
    let model = branch();
    let exploration = explore(&model, Bounds::CERTIFIABLE.with_states(3));
    let partial = exploration.exhausted().expect("the state bound trips");
    assert_eq!(partial.tripped(), Bound::States);
    assert_eq!(vectors(partial.explored().states()), vec![[0], [1], [2]]);
    assert_eq!(
        partial.explored().expanded(),
        1,
        "only the initial state was expanded"
    );
    assert_eq!(vectors(partial.frontier()), vec![[1], [2]]);

    let report = check(
        &model,
        &exploration,
        &Obligations::new(DeadlockPolicy::Defect),
    );
    let DeadlockOutcome::Deadlocked { states } = report.deadlock() else {
        panic!("n == 2 has no enabled action and is in the explored set");
    };
    assert_eq!(states.len(), 1);
    let found = states.first().expect("one deadlock");
    assert_eq!(found.state(), &state(&model, &[2]));
    assert_eq!(found.depth(), 1);
    assert_eq!(
        found.evidence(),
        &Evidence::Unwitnessed(NoWitness::Truncated {
            tripped: Bound::States,
            explored: 3,
        }),
        "a real deadlock, and no path claimed to be shortest"
    );
    assert_eq!(report.verdict(), Verdict::Refuted);
}

/// Not finding a terminal state in a bounded run is not deadlock freedom.
#[test]
fn a_bounded_run_without_a_terminal_state_is_inconclusive_about_deadlock() {
    let model = counter(4);
    let exploration = explore(&model, Bounds::CERTIFIABLE.with_states(3));
    let report = check(
        &model,
        &exploration,
        &Obligations::new(DeadlockPolicy::Defect),
    );
    assert_eq!(
        report.deadlock(),
        &DeadlockOutcome::Inconclusive(Unresolved::ResourceExhausted {
            tripped: Bound::States,
            explored: 3,
            frontier: 1,
        })
    );
    assert_eq!(report.verdict(), Verdict::Inconclusive);
}

/// Under `Allowed` a bound changes nothing, because no claim was made for it to
/// weaken. The count is over the explored states and the scope line says so.
#[test]
fn an_unjudged_deadlock_report_is_unaffected_by_a_bound() {
    let model = counter(4);
    let exploration = explore(&model, Bounds::CERTIFIABLE.with_states(3));
    let report = check(
        &model,
        &exploration,
        &Obligations::new(DeadlockPolicy::Allowed),
    );
    assert_eq!(
        report.deadlock(),
        &DeadlockOutcome::NotJudged { terminal: 0 }
    );
    assert_eq!(report.deadlock().verdict(), Verdict::Established);
    assert!(!report.scope().is_complete());
}

// ---------------------------------------------------------------------------
// the engine failing rather than answering
// ---------------------------------------------------------------------------

/// A predicate that cannot be evaluated at a reachable state is `EngineError`, and it
/// names the state and the predicate. Not a verdict: "An engine defect is never a
/// semantic verdict" (RFC 0026).
#[test]
fn a_predicate_that_cannot_be_evaluated_is_an_engine_error() {
    let model = unevaluable();
    let exploration = complete(&model);
    let report = check(
        &model,
        &exploration,
        &Obligations::every_predicate(&model, DeadlockPolicy::Allowed),
    );
    let at = index(&model, "Overflows");
    let CheckOutcome::Inconclusive(reason) = outcome(&report, at) else {
        panic!("`n + i64::MAX` overflows from n == 1 on");
    };
    let Unresolved::EngineError {
        state: where_,
        source,
    } = reason
    else {
        panic!("this is not a budget question");
    };
    assert_eq!(
        where_,
        &state(&model, &[1]),
        "the ascending-first failing state, deterministically"
    );
    assert_eq!(
        **source,
        EvaluationError::Expression(EvalError::Overflow {
            operator: ArithOp::Add,
            left: 1,
            right: i64::MAX,
        })
    );
    assert_eq!(
        report
            .invariant(at)
            .map(|result| result.name().as_str().to_owned()),
        Some("Overflows".to_owned()),
        "the predicate is named beside the state"
    );

    // The sibling invariant is unaffected: one predicate failing does not cost the
    // report the answers it does have.
    assert_eq!(
        outcome(&report, index(&model, "Bounded")),
        &CheckOutcome::Holds { states: 3 }
    );
    assert_eq!(report.verdict(), Verdict::Inconclusive);
}

/// The failure is a *check* failure, not an exploration failure: the same model
/// explores cleanly and closes.
#[test]
fn the_unevaluable_model_still_explores() {
    let model = unevaluable();
    let exploration = complete(&model);
    let reachable = exploration.closed().expect("the chain closes");
    assert_eq!(vectors(reachable.states()), vec![[0], [1], [2]]);
}

/// An obligation naming a predicate the model does not declare is refused before any
/// state is examined, so a bad request leaves no partial report.
#[test]
fn an_undeclared_invariant_refuses_the_whole_check() {
    let model = counter(4);
    let exploration = complete(&model);
    let refused = checking::check(
        &model,
        &exploration,
        &Obligations::new(DeadlockPolicy::Defect)
            .invariant(0)
            .invariant(7),
    );
    assert_eq!(
        refused,
        Err(CheckError::UnknownPredicate {
            index: 7,
            declared: 2,
        })
    );
    assert_eq!(
        refused.unwrap_err().to_string(),
        "invariant names predicate 7; the model declares 2"
    );
}

/// The lowest offending index is the one reported, whichever order they were declared
/// in, because the obligations are a set.
#[test]
fn the_refusal_names_the_least_undeclared_index() {
    let model = counter(4);
    let exploration = complete(&model);
    let refused = checking::check(
        &model,
        &exploration,
        &Obligations::new(DeadlockPolicy::Defect)
            .invariant(9)
            .invariant(4),
    );
    assert_eq!(
        refused,
        Err(CheckError::UnknownPredicate {
            index: 4,
            declared: 2,
        })
    );
}

// ---------------------------------------------------------------------------
// the obligations
// ---------------------------------------------------------------------------

/// Declaring an invariant twice declares it once, and declaration order is discarded —
/// the same rule the model layer applies to variables and actions.
#[test]
fn obligations_are_a_set() {
    let model = counter(4);
    let twice = Obligations::new(DeadlockPolicy::Defect)
        .invariant(1)
        .invariant(0)
        .invariant(1);
    let once = Obligations::new(DeadlockPolicy::Defect)
        .invariant(0)
        .invariant(1);
    assert_eq!(twice, once);
    assert_eq!(
        twice.invariants().iter().copied().collect::<Vec<_>>(),
        vec![0, 1]
    );

    let exploration = complete(&model);
    assert_eq!(
        check(&model, &exploration, &twice),
        check(&model, &exploration, &once)
    );
    assert_eq!(check(&model, &exploration, &twice).invariants().len(), 2);
}

/// A check with no invariants still answers the deadlock question, and renders as the
/// empty thing it is.
#[test]
fn a_check_may_uphold_no_invariant() {
    let model = counter(4);
    let exploration = complete(&model);
    let report = check(
        &model,
        &exploration,
        &Obligations::new(DeadlockPolicy::Defect),
    );
    assert!(report.invariants().is_empty());
    assert!(report.invariant(0).is_none());
    assert_eq!(
        report.to_string(),
        "scope complete states=5\n\
         policy deadlock=defect\n\
         deadlock refuted count=1\n  \
           deadlock state=(4) depth=4\n    \
             witness (0) --Step--> (1) --Step--> (2) --Step--> (3) --Step--> (4)\n\
         verdict refuted"
    );
}

/// The vacuous report: nothing upheld, terminal states allowed. It folds to
/// `established` the way an empty conjunction is true, and the rendering makes the
/// vacuity visible rather than hiding it behind the word.
#[test]
fn the_empty_report_is_vacuously_established_and_says_so() {
    let model = counter(4);
    let exploration = complete(&model);
    let report = check(
        &model,
        &exploration,
        &Obligations::new(DeadlockPolicy::Allowed),
    );
    assert_eq!(report.verdict(), Verdict::Established);
    assert_eq!(
        report.to_string(),
        "scope complete states=5\n\
         policy deadlock=allowed\n\
         deadlock not-judged terminal=1\n\
         verdict established"
    );
}

// ---------------------------------------------------------------------------
// the fold
// ---------------------------------------------------------------------------

/// A refutation dominates an inconclusive sibling. Letting the inconclusive one win
/// would be budget exhaustion behaving as a semantic answer.
#[test]
fn a_refutation_dominates_an_inconclusive_sibling() {
    let model = unevaluable();
    let exploration = complete(&model);
    // `Bounded` holds, `Overflows` is an engine error, and the terminal state at
    // `n == 2` is a defect under this policy.
    let report = check(
        &model,
        &exploration,
        &Obligations::every_predicate(&model, DeadlockPolicy::Defect),
    );
    assert!(matches!(
        outcome(&report, index(&model, "Overflows")),
        CheckOutcome::Inconclusive(_)
    ));
    assert!(matches!(
        report.deadlock(),
        DeadlockOutcome::Deadlocked { .. }
    ));
    assert_eq!(report.verdict(), Verdict::Refuted);
}

/// Every outcome's own verdict, and the three spellings the assurance result uses.
#[test]
fn the_verdict_vocabulary_is_the_assurance_results() {
    assert_eq!(Verdict::ALL.len(), 3);
    assert_eq!(Verdict::Established.as_str(), "established");
    assert_eq!(Verdict::Refuted.as_str(), "refuted");
    assert_eq!(Verdict::Inconclusive.as_str(), "inconclusive");
    assert_eq!(Verdict::Refuted.to_string(), "refuted");
    assert_eq!(
        CheckOutcome::Holds { states: 1 }.verdict(),
        Verdict::Established
    );
    assert_eq!(
        DeadlockOutcome::Free { states: 1 }.verdict(),
        Verdict::Established
    );
    assert_eq!(
        DeadlockOutcome::NotJudged { terminal: 3 }.verdict(),
        Verdict::Established
    );
}

/// The two INV-008 reasons this grain can produce, spelled the way
/// `assurance-result.schema.json` and `continuum-value`'s `InconclusiveReason` spell
/// them. This is the mapping that lets a daemon lift an engine outcome into an
/// assurance result without deciding anything, and it is asserted as strings because
/// this crate deliberately keeps no dependency on the crate that owns the enum.
#[test]
fn the_inconclusive_reasons_are_spelled_as_the_schema_spells_them() {
    let exhausted = Unresolved::ResourceExhausted {
        tripped: Bound::Depth,
        explored: 4,
        frontier: 2,
    };
    assert_eq!(exhausted.as_str(), "ResourceExhausted");
    assert_eq!(
        exhausted.to_string(),
        "reason=ResourceExhausted tripped=depth explored=4 frontier=2"
    );

    let model = counter(4);
    let failed = Unresolved::EngineError {
        state: state(&model, &[2]),
        source: Box::new(EvaluationError::UnknownPredicate {
            index: 9,
            declared: 2,
        }),
    };
    assert_eq!(failed.as_str(), "EngineError");
    assert_eq!(
        failed.to_string(),
        "reason=EngineError state=(2) detail=predicate index 9; the model declares 2"
    );

    let spellings: BTreeSet<&str> = [exhausted.as_str(), failed.as_str()].into_iter().collect();
    assert_eq!(spellings.len(), 2);
}

/// `Unresolved` chains to the model layer's error, so a caller walking `source()`
/// reaches the overflow itself rather than stopping at the summary.
#[test]
fn an_engine_error_chains_to_its_cause() {
    use core::error::Error;

    let model = unevaluable();
    let exploration = complete(&model);
    let report = check(
        &model,
        &exploration,
        &Obligations::new(DeadlockPolicy::Allowed).invariant(index(&model, "Overflows")),
    );
    let CheckOutcome::Inconclusive(reason) = outcome(&report, index(&model, "Overflows")) else {
        panic!("the predicate overflows");
    };
    assert!(reason.source().is_some());
    assert!(
        Unresolved::ResourceExhausted {
            tripped: Bound::States,
            explored: 1,
            frontier: 0,
        }
        .source()
        .is_none()
    );
}

// ---------------------------------------------------------------------------
// determinism
// ---------------------------------------------------------------------------

/// Two checks over independent explorations of one model, under four bound settings
/// including three that fail: equal, and byte-identical when rendered.
#[test]
fn checking_is_deterministic_under_every_bound() {
    let model = branch();
    let settings = [
        Bounds::CERTIFIABLE,
        Bounds::CERTIFIABLE.with_states(3),
        Bounds::CERTIFIABLE.with_depth(1),
        Bounds::CERTIFIABLE.with_transitions(2),
    ];
    for bounds in settings {
        let obligations = Obligations::every_predicate(&model, DeadlockPolicy::Defect);
        let first = check(&model, &explore(&model, bounds), &obligations);
        let second = check(&model, &explore(&model, bounds), &obligations);
        assert_eq!(first, second, "{bounds:?}");
        assert_eq!(first.to_string(), second.to_string(), "{bounds:?}");
    }
}

/// And across a rebuilt model: a report is a function of the declaration, not of the
/// `Model` value it was read from.
#[test]
fn checking_is_deterministic_across_a_rebuilt_model() {
    let left = counter(4);
    let right = counter(4);
    let obligations = Obligations::every_predicate(&left, DeadlockPolicy::Defect);
    assert_eq!(
        check(&left, &complete(&left), &obligations),
        check(&right, &complete(&right), &obligations)
    );
}
