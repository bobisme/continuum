//! What a shortest witness promises, on models built to test one promise each (PR 8,
//! IMPL-04).
//!
//! Die Hard is in `tests/witness_diehard.rs`; it is the corpus model and its path is
//! frozen elsewhere. The models here are the opposite kind: each is the smallest
//! declaration that exercises one rule, and each is *linear* in the rule it tests — a
//! chain, a diamond, a fork, an island, a pair of models that disagree about how many
//! actions exist. Nothing here is combinatorial, and nothing here needs to be.
//!
//! The suite covers: the ordinary path, the zero-step path, the deadlock endpoint,
//! tie-breaking among equally short paths, the four ways there is no witness, and
//! determinism.

use continuum_engine_reference::bfs::{self, Bound, Bounds, Exploration};
use continuum_engine_reference::expr::{BoolExpr, CmpOp, IntExpr};
use continuum_engine_reference::model::{ActionDecl, EvaluationError, Model, ModelBuilder, State};
use continuum_engine_reference::witness::{self, NoWitness, Target, Witness};

// ---------------------------------------------------------------------------
// the models
// ---------------------------------------------------------------------------

/// A chain: `n` counts from `0` to `limit`, one step at a time, and then stops. The
/// last state is a deadlock — the guard is what makes it one.
fn counter(limit: i64) -> Model {
    ModelBuilder::new()
        .variable("n", 0, limit)
        .initial_state(&[("n", 0)])
        .action(ActionDecl::deterministic(
            "Step",
            BoolExpr::compare(CmpOp::Lt, IntExpr::var("n"), IntExpr::constant(limit)),
            vec![("n", IntExpr::plus(IntExpr::var("n"), IntExpr::constant(1)))],
        ))
        .predicate(
            "AtTheEnd",
            BoolExpr::compare(CmpOp::Eq, IntExpr::var("n"), IntExpr::constant(limit)),
        )
        .predicate("Never", BoolExpr::Const(false))
        .build()
        .expect("the counter is a valid model")
}

/// Two disjoint one-step routes from `0` to `3`, so `3` has two shortest paths and the
/// recorded one has to be a choice the model makes rather than a coin toss.
///
/// The action names are chosen so that byte order disagrees with the route order:
/// `JoinLeft` and `JoinRight` are the second legs, `Left` and `Right` the first, so
/// the actions are indexed `0..3` in that order and the *state* order, not the action
/// order, is what decides.
fn diamond() -> Model {
    let at = |value: i64| BoolExpr::compare(CmpOp::Eq, IntExpr::var("x"), IntExpr::constant(value));
    ModelBuilder::new()
        .variable("x", 0, 3)
        .initial_state(&[("x", 0)])
        .action(ActionDecl::deterministic(
            "Left",
            at(0),
            vec![("x", IntExpr::constant(1))],
        ))
        .action(ActionDecl::deterministic(
            "Right",
            at(0),
            vec![("x", IntExpr::constant(2))],
        ))
        .action(ActionDecl::deterministic(
            "JoinLeft",
            at(1),
            vec![("x", IntExpr::constant(3))],
        ))
        .action(ActionDecl::deterministic(
            "JoinRight",
            at(2),
            vec![("x", IntExpr::constant(3))],
        ))
        .predicate(
            "Merged",
            BoolExpr::compare(CmpOp::Eq, IntExpr::var("x"), IntExpr::constant(3)),
        )
        .predicate(
            "Halfway",
            BoolExpr::and(
                BoolExpr::compare(CmpOp::Gt, IntExpr::var("x"), IntExpr::constant(0)),
                BoolExpr::compare(CmpOp::Lt, IntExpr::var("x"), IntExpr::constant(3)),
            ),
        )
        .build()
        .expect("the diamond is a valid model")
}

/// One action with two outcomes, so both leaves land in one layer and a predicate that
/// admits both has a tie to break.
fn fork() -> Model {
    ModelBuilder::new()
        .variable("x", 0, 2)
        .initial_state(&[("x", 0)])
        .action(ActionDecl::enumerated(
            "Pick",
            BoolExpr::compare(CmpOp::Eq, IntExpr::var("x"), IntExpr::constant(0)),
            vec![
                vec![("x", IntExpr::constant(1))],
                vec![("x", IntExpr::constant(2))],
            ],
        ))
        .predicate(
            "Picked",
            BoolExpr::compare(CmpOp::Gt, IntExpr::var("x"), IntExpr::constant(0)),
        )
        .build()
        .expect("the fork is a valid model")
}

/// A model whose declared domain has a state nothing reaches.
fn island() -> Model {
    ModelBuilder::new()
        .variable("x", 0, 2)
        .initial_state(&[("x", 0)])
        .action(ActionDecl::deterministic(
            "Only",
            BoolExpr::compare(CmpOp::Eq, IntExpr::var("x"), IntExpr::constant(0)),
            vec![("x", IntExpr::constant(1))],
        ))
        .predicate(
            "IsTwo",
            BoolExpr::compare(CmpOp::Eq, IntExpr::var("x"), IntExpr::constant(2)),
        )
        .build()
        .expect("the island is a valid model")
}

/// Two models over the same variable and domain that disagree about how many actions
/// exist. Exploring `wide` and asking `narrow` for the witness is the one model
/// mismatch that can be caught mechanically.
fn wide() -> Model {
    ModelBuilder::new()
        .variable("x", 0, 2)
        .initial_state(&[("x", 0)])
        .action(ActionDecl::deterministic(
            "Aa",
            BoolExpr::compare(CmpOp::Eq, IntExpr::var("x"), IntExpr::constant(0)),
            vec![("x", IntExpr::constant(1))],
        ))
        .action(ActionDecl::deterministic(
            "Bb",
            BoolExpr::compare(CmpOp::Eq, IntExpr::var("x"), IntExpr::constant(1)),
            vec![("x", IntExpr::constant(2))],
        ))
        .build()
        .expect("the wide model is valid")
}

fn narrow() -> Model {
    ModelBuilder::new()
        .variable("x", 0, 2)
        .initial_state(&[("x", 0)])
        .action(ActionDecl::deterministic(
            "Aa",
            BoolExpr::compare(CmpOp::Eq, IntExpr::var("x"), IntExpr::constant(0)),
            vec![("x", IntExpr::constant(1))],
        ))
        .build()
        .expect("the narrow model is valid")
}

// ---------------------------------------------------------------------------
// helpers
// ---------------------------------------------------------------------------

fn explore(model: &Model) -> Exploration {
    bfs::explore(model, Bounds::CERTIFIABLE).expect("this model evaluates everywhere")
}

fn state(model: &Model, values: &[i64]) -> State {
    model
        .state(values)
        .unwrap_or_else(|error| panic!("{values:?} is a state: {error}"))
}

fn predicate(model: &Model, name: &str) -> usize {
    model
        .predicate_index(name)
        .unwrap_or_else(|| panic!("{name} is declared"))
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

/// Replay a witness through `Model::successors`, one step at a time. The differential
/// every test below can afford to run.
fn replays(model: &Model, found: &Witness) {
    assert!(
        model.initial_states().contains(found.start()),
        "a witness starts at a declared initial state"
    );
    let mut here = found.start().clone();
    for (index, step) in found.steps().iter().enumerate() {
        let row = model.successors(&here).expect("this model evaluates");
        assert!(
            row.iter()
                .any(|actual| actual.action() == step.action() && actual.target() == step.target()),
            "step {index}: {here} --{}--> {} is not in the model's row",
            step.name(),
            step.target()
        );
        here = step.target().clone();
    }
    assert_eq!(&here, found.target());
}

// ---------------------------------------------------------------------------
// the ordinary path
// ---------------------------------------------------------------------------

/// A chain's witness is the chain: four steps, all under the one declared action.
#[test]
fn the_chain_witness_is_the_chain() {
    let model = counter(4);
    let exploration = explore(&model);
    let target = Target::State(state(&model, &[4]));
    let found = witness::shortest(&model, &exploration, &target).expect("4 is reachable");
    assert_eq!(found.len(), 4);
    assert_eq!(
        labelled(&found),
        vec![
            ("Step".to_owned(), vec![1]),
            ("Step".to_owned(), vec![2]),
            ("Step".to_owned(), vec![3]),
            ("Step".to_owned(), vec![4]),
        ]
    );
    assert_eq!(found.start(), &state(&model, &[0]));
    assert_eq!(
        found.to_string(),
        "(0) --Step--> (1) --Step--> (2) --Step--> (3) --Step--> (4)"
    );
    replays(&model, &found);
}

/// A path *to* a deadlock is a path like any other. The witness module reports it and
/// says nothing about whether a state with no successors is a defect — that judgment
/// belongs to the checking bone.
#[test]
fn a_witness_to_a_deadlock_is_a_witness_like_any_other() {
    let model = counter(4);
    let exploration = explore(&model);
    let last = state(&model, &[4]);
    assert!(
        model.successors(&last).expect("evaluates").is_empty(),
        "the guard disables the only action, so (4) is a deadlock"
    );
    let by_state = witness::shortest(&model, &exploration, &Target::State(last.clone()))
        .expect("the deadlock is reachable");
    let by_predicate = witness::shortest(
        &model,
        &exploration,
        &Target::Holds(predicate(&model, "AtTheEnd")),
    )
    .expect("AtTheEnd holds at the deadlock");
    assert_eq!(by_state, by_predicate);
    assert_eq!(by_state.len(), 4);
    assert_eq!(by_state.target(), &last);
    replays(&model, &by_state);
}

/// A target already satisfied at an initial state is a witness with no steps, whose
/// endpoint is its own start. Zero is a length, not an absence.
#[test]
fn a_target_at_an_initial_state_is_a_witness_of_no_steps() {
    let model = counter(4);
    let exploration = explore(&model);
    let target = Target::State(state(&model, &[0]));
    let found = witness::shortest(&model, &exploration, &target).expect("(0) is initial");
    assert!(found.is_empty());
    assert_eq!(found.len(), 0);
    assert_eq!(found.start(), found.target());
    assert_eq!(found.states(), vec![&state(&model, &[0])]);
    replays(&model, &found);
}

/// An enumerated outcome is reached by the action that enumerates it.
#[test]
fn a_fork_leaf_is_reached_by_the_enumerating_action() {
    let model = fork();
    let exploration = explore(&model);
    let target = Target::State(state(&model, &[2]));
    let found = witness::shortest(&model, &exploration, &target).expect("(2) is reachable");
    assert_eq!(labelled(&found), vec![("Pick".to_owned(), vec![2])]);
    replays(&model, &found);
}

// ---------------------------------------------------------------------------
// several shortest paths
// ---------------------------------------------------------------------------

/// The diamond's merge point has two shortest paths, and the recorded one goes through
/// the ascending-least of the two middle states.
///
/// Nothing about the *action* names decides this: `JoinLeft` and `JoinRight` are
/// indices 0 and 1 and the left route wins because state `(1)` sorts before `(2)` and
/// is therefore expanded first. That is the same rule `crate::bfs` states for
/// admission order — determinism that does not depend on how actions are spelled.
#[test]
fn the_diamond_takes_the_ascending_least_route() {
    let model = diamond();
    let exploration = explore(&model);
    let reachable = exploration.closed().expect("the diamond closes");
    assert_eq!(reachable.depths(), [0, 1, 1, 2]);
    let target = Target::State(state(&model, &[3]));
    let found = witness::shortest(&model, &exploration, &target).expect("(3) is reachable");
    assert_eq!(found.len(), 2);
    assert_eq!(
        labelled(&found),
        vec![
            ("Left".to_owned(), vec![1]),
            ("JoinLeft".to_owned(), vec![3])
        ]
    );
    // The other route exists and is exactly as short, which is what makes this a tie
    // that was broken rather than the only answer.
    let right = state(&model, &[2]);
    assert_eq!(reachable.depth_of(&right), Some(1));
    assert!(
        model
            .successors(&right)
            .expect("evaluates")
            .iter()
            .any(|step| step.target() == &state(&model, &[3])),
        "the right route also reaches the merge in one more step"
    );
    replays(&model, &found);
}

/// A predicate satisfied by two states in one layer picks the ascending-least of them.
#[test]
fn a_tie_within_one_layer_goes_to_the_least_state() {
    let model = fork();
    let exploration = explore(&model);
    let target = Target::Holds(predicate(&model, "Picked"));
    let found = witness::shortest(&model, &exploration, &target).expect("both leaves are picked");
    assert_eq!(found.target(), &state(&model, &[1]));
    assert_eq!(found.len(), 1);

    // The diamond's `Halfway` predicate is the same tie one model over.
    let diamond = diamond();
    let exploration = explore(&diamond);
    let target = Target::Holds(predicate(&diamond, "Halfway"));
    let found = witness::shortest(&diamond, &exploration, &target).expect("both middles qualify");
    assert_eq!(found.target(), &state(&diamond, &[1]));
    assert_eq!(found.len(), 1);
}

/// A deeper state satisfying the target never displaces a shallower one, whichever
/// order the canonical table happens to list them in.
#[test]
fn the_shallowest_satisfying_state_wins() {
    let model = counter(4);
    let exploration = explore(&model);
    let reachable = exploration.closed().expect("the chain closes");
    // `AtTheEnd` holds only at the deepest state, so the witness has to be the longest
    // one this model has — a shallower state never displaces it.
    let target = Target::Holds(predicate(&model, "AtTheEnd"));
    let found = witness::shortest(&model, &exploration, &target).expect("the end is reachable");
    assert_eq!(found.len(), 4);
    assert_eq!(reachable.depth_of(found.target()), Some(found.len()));
    for (state, depth) in reachable.states().iter().zip(reachable.depths().iter()) {
        let found = witness::shortest(&model, &exploration, &Target::State(state.clone()))
            .expect("a discovered state is a target");
        assert_eq!(found.len(), *depth, "{state}");
        replays(&model, &found);
    }
}

// ---------------------------------------------------------------------------
// the four ways there is no witness
// ---------------------------------------------------------------------------

/// A well-typed state that nothing reaches has no witness, and over a complete
/// exploration that is a real negative result rather than a budget question.
#[test]
fn an_unreachable_state_has_no_witness() {
    let model = island();
    let exploration = explore(&model);
    let reachable = exploration.closed().expect("the island closes");
    assert_eq!(reachable.len(), 2);
    let marooned = state(&model, &[2]);
    assert!(model.admits(&[2]), "(2) is inside the declared domain");
    assert!(!reachable.contains(&marooned));
    assert_eq!(
        witness::shortest(&model, &exploration, &Target::State(marooned)),
        Err(NoWitness::Unreached { searched: 2 })
    );
}

/// A predicate no reachable state satisfies has no witness either, by the same arm.
#[test]
fn a_predicate_nothing_satisfies_has_no_witness() {
    let model = island();
    let exploration = explore(&model);
    assert_eq!(
        witness::shortest(
            &model,
            &exploration,
            &Target::Holds(predicate(&model, "IsTwo"))
        ),
        Err(NoWitness::Unreached { searched: 2 })
    );
    // And a predicate that is `false` everywhere, so its `Fails` target is everything
    // and its `Holds` target is nothing.
    let counter = counter(4);
    let exploration = explore(&counter);
    assert_eq!(
        witness::shortest(
            &counter,
            &exploration,
            &Target::Holds(predicate(&counter, "Never"))
        ),
        Err(NoWitness::Unreached { searched: 5 })
    );
    let everything = witness::shortest(
        &counter,
        &exploration,
        &Target::Fails(predicate(&counter, "Never")),
    )
    .expect("a predicate that never holds fails at the initial state");
    assert_eq!(everything.len(), 0);
}

/// A truncated exploration is refused whichever bound stopped it, and the refusal
/// names the bound.
#[test]
fn every_truncated_exploration_is_refused() {
    let model = counter(20);
    let target = Target::State(state(&model, &[20]));
    for (bounds, expected) in [
        (Bounds::CERTIFIABLE.with_states(7), Bound::States),
        (Bounds::CERTIFIABLE.with_depth(3), Bound::Depth),
        (Bounds::CERTIFIABLE.with_transitions(11), Bound::Transitions),
    ] {
        let exploration = bfs::explore(&model, bounds).expect("the chain evaluates");
        let outcome = witness::shortest(&model, &exploration, &target);
        let Err(NoWitness::Truncated { tripped, explored }) = outcome else {
            panic!("{bounds:?} must be refused: {outcome:?}");
        };
        assert_eq!(tripped, expected, "{bounds:?}");
        assert_eq!(explored, exploration.reachable().len(), "{bounds:?}");
    }
    // Wide enough, and the same query answers.
    let exploration = explore(&model);
    assert_eq!(
        witness::shortest(&model, &exploration, &target)
            .expect("the chain closes")
            .len(),
        20
    );
}

/// A predicate index the model does not declare is a typed evaluation failure, not a
/// missing witness — the question was malformed, not answered in the negative.
#[test]
fn an_undeclared_predicate_index_is_a_typed_failure() {
    let model = counter(4);
    let exploration = explore(&model);
    let outcome = witness::shortest(&model, &exploration, &Target::Holds(9));
    let Err(NoWitness::Predicate {
        index,
        state: at,
        source,
    }) = outcome
    else {
        panic!("an undeclared predicate index is a typed failure: {outcome:?}");
    };
    assert_eq!(index, 9);
    assert_eq!(at.as_slice(), [0], "reported at the first state scanned");
    assert_eq!(
        *source,
        EvaluationError::UnknownPredicate {
            index: 9,
            declared: 2,
        }
    );
}

/// An exploration of a different model is caught when the recorded chain names an
/// action the model does not declare.
#[test]
fn an_exploration_of_another_model_is_refused() {
    let wide = wide();
    let narrow = narrow();
    let exploration = explore(&wide);
    assert_eq!(exploration.reachable().len(), 3);
    // The witness `wide` gives for its own deepest state is a two-step path whose last
    // step is action 1.
    let target = Target::State(state(&wide, &[2]));
    let honest = witness::shortest(&wide, &exploration, &target).expect("(2) is reachable in wide");
    assert_eq!(honest.len(), 2);
    assert_eq!(
        honest.steps().last().map(|step| step.action()),
        Some(1_usize)
    );
    // `narrow` declares one action, so the same chain cannot be labelled against it.
    assert_eq!(narrow.actions().len(), 1);
    assert_eq!(
        witness::shortest(&narrow, &exploration, &Target::State(state(&narrow, &[2]))),
        Err(NoWitness::ForeignExploration {
            action: 1,
            declared: 1,
        })
    );
}

/// Every refusal prints what it is about, and the predicate arm chains to the model
/// layer's error rather than swallowing it.
#[test]
fn the_refusals_are_reportable() {
    use core::error::Error;

    let model = counter(20);
    let bounded = bfs::explore(&model, Bounds::CERTIFIABLE.with_depth(3)).expect("evaluates");
    let truncated = witness::shortest(&model, &bounded, &Target::State(state(&model, &[20])))
        .expect_err("a bounded run is refused");
    let rendered = truncated.to_string();
    assert!(rendered.contains("depth"), "{rendered}");
    assert!(rendered.contains("shortest"), "{rendered}");
    assert!(truncated.source().is_none());

    let exploration = explore(&model);
    let unreached = witness::shortest(
        &model,
        &exploration,
        &Target::Holds(predicate(&model, "Never")),
    )
    .expect_err("nothing satisfies it");
    assert!(unreached.to_string().contains("21"), "{unreached}");

    let malformed = witness::shortest(&model, &exploration, &Target::Holds(9))
        .expect_err("9 is not a declared predicate");
    assert!(malformed.to_string().contains('9'), "{malformed}");
    assert!(
        malformed.source().is_some(),
        "the model layer's error is chained"
    );
}

// ---------------------------------------------------------------------------
// determinism
// ---------------------------------------------------------------------------

/// Repeat extractions agree exactly, on the tie-breaking model where they have the
/// most room to disagree.
#[test]
fn repeat_extractions_agree_on_every_outcome_shape() {
    for (model, target) in [
        (diamond(), Target::State(state(&diamond(), &[3]))),
        (diamond(), Target::Holds(predicate(&diamond(), "Merged"))),
        (diamond(), Target::Holds(predicate(&diamond(), "Halfway"))),
        (fork(), Target::Holds(predicate(&fork(), "Picked"))),
        (counter(20), Target::State(state(&counter(20), &[20]))),
    ] {
        let extract = || witness::shortest(&model, &explore(&model), &target);
        let first = extract();
        let second = extract();
        assert_eq!(first, second, "{target:?}");
        assert_eq!(
            format!("{first:?}").as_bytes(),
            format!("{second:?}").as_bytes(),
            "{target:?}"
        );
        let found = first.expect("each target above is satisfiable");
        assert_eq!(
            found.to_string().as_bytes(),
            second
                .expect("each target above is satisfiable")
                .to_string()
                .as_bytes()
        );
        replays(&model, &found);
    }
}

/// A refused extraction is deterministic too.
#[test]
fn repeat_refusals_agree() {
    let model = island();
    let target = Target::State(state(&model, &[2]));
    let refuse = || witness::shortest(&model, &explore(&model), &target);
    assert_eq!(refuse(), refuse());
    assert_eq!(
        format!("{:?}", refuse()).as_bytes(),
        format!("{:?}", refuse()).as_bytes()
    );
}
