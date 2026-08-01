//! Die Hard checked: `TypeOK` established, `NotSolved` refuted by the film's solution
//! (PR 8, IMPL-03).
//!
//! # What this file owns
//!
//! The corpus model is the one place where both sides of a safety answer are frozen
//! independently of this crate. `DieHard.ctm:30-31` declares two predicates and says
//! which way each goes: `TypeOK` holds, and `NotSolved` is "intentionally false". So
//! one model, checked once, exercises [`CheckOutcome::Holds`] and
//! [`CheckOutcome::Violated`] against artifacts nobody here wrote —
//! `lean/Continuum/Examples/DieHard.lean:14-22` for the path, and the four restatements
//! of the depth-6 fact listed in `tests/witness_diehard.rs`.
//!
//! It also owns the INV-008 case that matters most, because Die Hard is the only model
//! in the crate whose *complete* answer is known: a bounded run of it must not report
//! `TypeOK` as holding, even though `TypeOK` does hold, because the run that saw six
//! of sixteen states did not establish it. All three bounds are exercised, on the
//! three bounded explorations `tests/bfs_diehard.rs` already froze state-for-state.
//!
//! Contract-shaped rules — the policy arms, the error arms, the rendering, the
//! deterministic fold — are in `tests/checking_contract.rs`, on models built one rule
//! at a time.

use continuum_engine_reference::bfs::{self, Bound, Bounds, Exploration};
use continuum_engine_reference::checking::{
    self, CheckOutcome, DeadlockOutcome, DeadlockPolicy, Evidence, Obligations, Scope, Unresolved,
    Verdict,
};
use continuum_engine_reference::diehard;
use continuum_engine_reference::model::{Model, State};
use continuum_engine_reference::witness::{self, NoWitness, Target, Witness};

fn model() -> Model {
    diehard::model().expect("the Die Hard transcription is a valid model")
}

fn state(model: &Model, big: i64, small: i64) -> State {
    model
        .state(&[big, small])
        .unwrap_or_else(|error| panic!("({big}, {small}) is a Die Hard state: {error}"))
}

fn complete(model: &Model) -> Exploration {
    bfs::explore(model, Bounds::CERTIFIABLE).expect("Die Hard evaluates everywhere")
}

fn predicate(model: &Model, name: &str) -> usize {
    model
        .predicate_index(name)
        .unwrap_or_else(|| panic!("{name} is declared"))
}

/// Every declared predicate upheld, with terminal states treated as defects — the
/// strictest reading of the corpus model.
fn strict(model: &Model) -> Obligations {
    Obligations::every_predicate(model, DeadlockPolicy::Defect)
}

fn outcome(report: &checking::CheckReport, index: usize) -> &CheckOutcome {
    report
        .invariant(index)
        .unwrap_or_else(|| panic!("predicate {index} was upheld"))
        .outcome()
}

/// The six-step path `lean/Continuum/Examples/DieHard.lean:14-22` fixes, in the
/// `(action name, target vector)` shape `tests/witness_diehard.rs` writes it in.
fn lean_path() -> Vec<(String, Vec<i64>)> {
    vec![
        ("FillBig".to_owned(), vec![5, 0]),
        ("BigToSmall".to_owned(), vec![2, 3]),
        ("EmptySmall".to_owned(), vec![2, 0]),
        ("BigToSmall".to_owned(), vec![0, 2]),
        ("FillBig".to_owned(), vec![5, 2]),
        ("BigToSmall".to_owned(), vec![4, 3]),
    ]
}

fn labelled(found: &Witness) -> Vec<(String, Vec<i64>)> {
    found
        .steps()
        .iter()
        .map(|step| {
            (
                step.name().as_str().to_owned(),
                step.target().as_slice().to_vec(),
            )
        })
        .collect()
}

// ---------------------------------------------------------------------------
// the declaration
// ---------------------------------------------------------------------------

/// Predicates are held in canonical name order like everything else, so `NotSolved`
/// is index 0 and `TypeOK` is index 1 whatever order `diehard.rs` declares them in.
/// Every index in this file is looked up rather than written, but the order itself is
/// worth pinning once: it is what a report's rows are sorted by.
#[test]
fn the_two_corpus_predicates_are_in_canonical_name_order() {
    let model = model();
    assert_eq!(predicate(&model, diehard::NOT_SOLVED), 0);
    assert_eq!(predicate(&model, diehard::TYPE_OK), 1);
    let obligations = strict(&model);
    assert_eq!(
        obligations.invariants().iter().copied().collect::<Vec<_>>(),
        vec![0, 1]
    );
    assert_eq!(obligations.deadlock(), DeadlockPolicy::Defect);
}

// ---------------------------------------------------------------------------
// the closed answer
// ---------------------------------------------------------------------------

/// `TypeOK` over the closed sixteen: established, and the arm says over how many.
#[test]
fn type_ok_holds_over_the_closed_sixteen() {
    let model = model();
    let exploration = complete(&model);
    let report = checking::check(&model, &exploration, &strict(&model)).expect("both are declared");
    assert_eq!(report.scope(), Scope::Complete { states: 16 });
    assert_eq!(
        outcome(&report, predicate(&model, diehard::TYPE_OK)),
        &CheckOutcome::Holds { states: 16 }
    );
}

/// `NotSolved` is refuted, at `(4, 3)`, at depth six, and the path is the Lean one.
///
/// The equality with the Lean path is `tests/witness_diehard.rs`'s pin, restated here
/// because this file is where the path arrives as a *check result* rather than as a
/// witness extraction: it is an observed fact about this model under this admission
/// order, not a guarantee the checker makes.
#[test]
fn not_solved_is_refuted_by_the_six_step_solution() {
    let model = model();
    let exploration = complete(&model);
    let report = checking::check(&model, &exploration, &strict(&model)).expect("both are declared");
    let CheckOutcome::Violated {
        state: at,
        depth,
        violations,
        evidence,
    } = outcome(&report, predicate(&model, diehard::NOT_SOLVED))
    else {
        panic!("NotSolved is intentionally false (DieHard.ctm:31)");
    };
    assert_eq!(at, &state(&model, 4, 3));
    assert_eq!(*depth, 6);
    assert_eq!(
        *violations, 2,
        "big == 4 at (4, 0) and (4, 3), and the report says so rather than showing one \
         of two silently"
    );
    let Evidence::Shortest(found) = evidence else {
        panic!("a closed exploration supports the word shortest");
    };
    assert_eq!(found.len(), 6);
    assert_eq!(labelled(found), lean_path());
    assert_eq!(found.target(), &state(&model, 4, 3));
    assert_eq!(found.start(), &state(&model, 0, 0));
}

/// The state the report names and the endpoint the witness extractor picks are the
/// same state, because both apply the same rule — first strict improvement over the
/// ascending table — to the same data. Asserted rather than assumed: the two scans
/// live in different modules and a change to either would be caught here.
#[test]
fn the_reported_state_is_the_witness_endpoint() {
    let model = model();
    let exploration = complete(&model);
    let index = predicate(&model, diehard::NOT_SOLVED);
    let report = checking::check(&model, &exploration, &strict(&model)).expect("both are declared");
    let CheckOutcome::Violated { state: at, .. } = outcome(&report, index) else {
        panic!("NotSolved is refuted");
    };
    let extracted = witness::shortest(&model, &exploration, &Target::Fails(index))
        .expect("the closed exploration has a shortest violation");
    assert_eq!(at, extracted.target());
}

/// Every guard in the corpus model is `true`, so no state has an empty successor row
/// and the strictest deadlock policy finds nothing to report.
#[test]
fn die_hard_has_no_deadlock_under_the_strictest_policy() {
    let model = model();
    let exploration = complete(&model);
    let report = checking::check(&model, &exploration, &strict(&model)).expect("both are declared");
    assert_eq!(report.deadlock(), &DeadlockOutcome::Free { states: 16 });
    assert!(report.deadlock().deadlocks().is_empty());
    assert_eq!(report.deadlock().verdict(), Verdict::Established);
}

/// The whole report folds to `refuted`: one invariant established, one refuted, no
/// deadlock. A refutation dominates, which is what makes the fold usable.
#[test]
fn the_corpus_report_folds_to_refuted() {
    let model = model();
    let exploration = complete(&model);
    let report = checking::check(&model, &exploration, &strict(&model)).expect("both are declared");
    assert_eq!(report.verdict(), Verdict::Refuted);
    assert_eq!(report.invariants().len(), 2);
    assert_eq!(report.invariants().first().map(|r| r.index()), Some(0));
    assert_eq!(
        report
            .invariants()
            .first()
            .map(|r| r.name().as_str().to_owned()),
        Some(diehard::NOT_SOLVED.to_owned())
    );
}

/// The rendering, byte for byte. This is what a daemon task result carries, so it is
/// pinned rather than described.
#[test]
fn the_corpus_report_renders_exactly() {
    let model = model();
    let exploration = complete(&model);
    let report = checking::check(&model, &exploration, &strict(&model)).expect("both are declared");
    assert_eq!(
        report.to_string(),
        "scope complete states=16\n\
         policy deadlock=defect\n\
         invariant 0 NotSolved refuted state=(4, 3) depth=6 violations=2\n  \
           witness (0, 0) --FillBig--> (5, 0) --BigToSmall--> (2, 3) --EmptySmall--> (2, 0) \
         --BigToSmall--> (0, 2) --FillBig--> (5, 2) --BigToSmall--> (4, 3)\n\
         invariant 1 TypeOK established states=16\n\
         deadlock established states=16\n\
         verdict refuted"
    );
}

// ---------------------------------------------------------------------------
// INV-008: a bound is not a pass
// ---------------------------------------------------------------------------

/// Depth 2: six of sixteen states, no `big == 4` among them. `TypeOK` is true at every
/// one of the six and is still **not** established — the six are a prefix, not the
/// model.
///
/// The explored set is `tests/bfs_diehard.rs::die_hard_bounded_by_depth_stops_at_the_third_layer`'s,
/// state for state.
#[test]
fn the_depth_bound_makes_both_invariants_inconclusive() {
    let model = model();
    let exploration =
        bfs::explore(&model, Bounds::CERTIFIABLE.with_depth(2)).expect("Die Hard evaluates");
    let report = checking::check(&model, &exploration, &strict(&model)).expect("both are declared");
    assert_eq!(
        report.scope(),
        Scope::Bounded {
            states: 6,
            tripped: Bound::Depth,
            frontier: 3,
        }
    );
    let expected = CheckOutcome::Inconclusive(Unresolved::ResourceExhausted {
        tripped: Bound::Depth,
        explored: 6,
        frontier: 3,
    });
    assert_eq!(outcome(&report, 0), &expected);
    assert_eq!(outcome(&report, 1), &expected);
    assert_eq!(report.verdict(), Verdict::Inconclusive);
}

/// Three states under the state bound: same answer, different tripped bound. A caller
/// that hit the state bound raises the state bound; the reason has to say which.
#[test]
fn the_state_bound_makes_both_invariants_inconclusive() {
    let model = model();
    let exploration =
        bfs::explore(&model, Bounds::CERTIFIABLE.with_states(3)).expect("Die Hard evaluates");
    let report = checking::check(&model, &exploration, &strict(&model)).expect("both are declared");
    let expected = CheckOutcome::Inconclusive(Unresolved::ResourceExhausted {
        tripped: Bound::States,
        explored: 3,
        frontier: 2,
    });
    assert_eq!(outcome(&report, 0), &expected);
    assert_eq!(outcome(&report, 1), &expected);
    assert_eq!(report.verdict(), Verdict::Inconclusive);
}

/// And the transition bound, which is the one no depth or state count implies.
#[test]
fn the_transition_bound_makes_both_invariants_inconclusive() {
    let model = model();
    let exploration =
        bfs::explore(&model, Bounds::CERTIFIABLE.with_transitions(10)).expect("Die Hard evaluates");
    let report = checking::check(&model, &exploration, &strict(&model)).expect("both are declared");
    let expected = CheckOutcome::Inconclusive(Unresolved::ResourceExhausted {
        tripped: Bound::Transitions,
        explored: 3,
        frontier: 2,
    });
    assert_eq!(outcome(&report, 0), &expected);
    assert_eq!(outcome(&report, 1), &expected);
    assert_eq!(report.verdict(), Verdict::Inconclusive);
}

/// No bounded run of this model reports `Holds` for anything, at any bound, even
/// though both predicates are true at every state each run saw and one of them is true
/// at every state there is. Nine bounds, one rule.
#[test]
fn no_bounded_run_ever_reports_holds() {
    let model = model();
    let bounds = [
        Bounds::CERTIFIABLE.with_states(1),
        Bounds::CERTIFIABLE.with_states(3),
        Bounds::CERTIFIABLE.with_states(9),
        Bounds::CERTIFIABLE.with_states(15),
        Bounds::CERTIFIABLE.with_depth(0),
        Bounds::CERTIFIABLE.with_depth(2),
        Bounds::CERTIFIABLE.with_depth(6),
        Bounds::CERTIFIABLE.with_transitions(10),
        Bounds::CERTIFIABLE.with_transitions(95),
    ];
    for bound in bounds {
        let exploration = bfs::explore(&model, bound).expect("Die Hard evaluates");
        assert!(
            exploration.closed().is_none(),
            "{bound:?} was meant to trip a bound"
        );
        let report =
            checking::check(&model, &exploration, &strict(&model)).expect("both are declared");
        for result in report.invariants() {
            assert!(
                !matches!(result.outcome(), CheckOutcome::Holds { .. }),
                "{} reported Holds under {bound:?}",
                result.name()
            );
        }
        assert_ne!(
            report.deadlock(),
            &DeadlockOutcome::Free { states: 16 },
            "deadlock freedom is a closure claim too"
        );
    }
}

// ---------------------------------------------------------------------------
// a refutation survives truncation
// ---------------------------------------------------------------------------

/// At depth 6 the run stops one layer short of `(4, 0)` but has already found
/// `(4, 3)`. That is a real refutation — the state was genuinely reached — and it is
/// reported as one, with no path, because `witness::shortest` answers only for a
/// closed exploration.
///
/// This is the fourth cell of the module's table, and it is the one that would be
/// wrong in both directions: reporting it as inconclusive would throw away a found
/// counterexample, and offering a path with it would call a path shortest that a
/// larger budget could beat.
#[test]
fn a_violation_inside_a_bounded_run_is_still_a_refutation() {
    let model = model();
    let exploration =
        bfs::explore(&model, Bounds::CERTIFIABLE.with_depth(6)).expect("Die Hard evaluates");
    let partial = exploration.exhausted().expect("depth 7 is out of reach");
    assert_eq!(partial.explored().len(), 14);
    assert!(partial.explored().contains(&state(&model, 4, 3)));
    assert!(!partial.explored().contains(&state(&model, 4, 0)));
    let report = checking::check(&model, &exploration, &strict(&model)).expect("both are declared");

    let CheckOutcome::Violated {
        state: at,
        depth,
        violations,
        evidence,
    } = outcome(&report, predicate(&model, diehard::NOT_SOLVED))
    else {
        panic!("(4, 3) is in the explored set and falsifies NotSolved");
    };
    assert_eq!(at, &state(&model, 4, 3));
    assert_eq!(*depth, 6, "a bounded run is a depth-preserving prefix");
    assert_eq!(*violations, 1, "(4, 0) was never discovered");
    assert_eq!(
        evidence,
        &Evidence::Unwitnessed(NoWitness::Truncated {
            tripped: Bound::Depth,
            explored: 14,
        })
    );
    assert!(evidence.witness().is_none());

    // The other invariant, over the same truncated run, still cannot be established.
    assert!(matches!(
        outcome(&report, predicate(&model, diehard::TYPE_OK)),
        CheckOutcome::Inconclusive(_)
    ));
    assert_eq!(
        report.verdict(),
        Verdict::Refuted,
        "a refutation dominates an inconclusive sibling"
    );
}

// ---------------------------------------------------------------------------
// determinism
// ---------------------------------------------------------------------------

/// Two checks of one model over two independent explorations are equal, and render
/// the same bytes. Independent explorations, not one reused, so the equality is a
/// property of the pipeline rather than of a cached value.
#[test]
fn checking_die_hard_is_deterministic() {
    let other = model();
    let model = model();
    let first = checking::check(&model, &complete(&model), &strict(&model)).expect("declared");
    let second = checking::check(&model, &complete(&model), &strict(&model)).expect("declared");
    assert_eq!(first, second);
    assert_eq!(first.to_string(), second.to_string());

    let third = checking::check(&other, &complete(&other), &strict(&other)).expect("declared");
    assert_eq!(first, third, "and of the model, not of the Model value");
}

/// The same, for a bounded run: the tripped bound, the explored count and the frontier
/// count all ride on the outcome, and all three are functions of the model and the
/// bounds alone.
#[test]
fn checking_a_bounded_die_hard_is_deterministic() {
    let model = model();
    let bounds = Bounds::CERTIFIABLE.with_states(9);
    let render = || {
        let exploration = bfs::explore(&model, bounds).expect("Die Hard evaluates");
        checking::check(&model, &exploration, &strict(&model))
            .expect("declared")
            .to_string()
    };
    assert_eq!(render(), render());
}

// ---------------------------------------------------------------------------
// the policy is the caller's, on the corpus model too
// ---------------------------------------------------------------------------

/// The same exploration, checked under both policies, differs in exactly one line.
/// Die Hard has no terminal state, so the two agree on the facts and disagree only on
/// what claim was made about them.
#[test]
fn the_deadlock_policy_changes_only_the_deadlock_line() {
    let model = model();
    let exploration = complete(&model);
    let defect = checking::check(&model, &exploration, &strict(&model)).expect("declared");
    let allowed = checking::check(
        &model,
        &exploration,
        &Obligations::every_predicate(&model, DeadlockPolicy::Allowed),
    )
    .expect("declared");
    assert_eq!(defect.invariants(), allowed.invariants());
    assert_eq!(defect.deadlock(), &DeadlockOutcome::Free { states: 16 });
    assert_eq!(
        allowed.deadlock(),
        &DeadlockOutcome::NotJudged { terminal: 0 }
    );
    assert_eq!(defect.verdict(), allowed.verdict());
}
