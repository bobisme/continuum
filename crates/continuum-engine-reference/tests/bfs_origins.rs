//! What recording a predecessor added to exploration, and what it did not (PR 8,
//! IMPL-04).
//!
//! # The two halves
//!
//! **It added a column.** Every discovered state now carries the labelled transition
//! that first reached it ([`Discovery`]), and the tests below check that column
//! against the model rather than against itself: every recorded step is a transition
//! `Model::successors` will reproduce, from a state exactly one layer shallower, under
//! the least action index that reaches it.
//!
//! **It changed nothing else.** That is the load-bearing claim, because
//! `tests/bfs_contract.rs` and `tests/bfs_diehard.rs` freeze the exploration's every
//! other observable and were written before this column existed. They are unchanged
//! and they pass; this file adds the two independent checks that make the claim
//! explicit rather than inferred — an independent hand walk of Die Hard compared state
//! for state and depth for depth, and the frozen *queue-ordered* frontier of a bounded
//! run, which is the one observable that an admission-order change would have to
//! disturb and the one a re-sorted result could not hide.

use std::collections::{BTreeMap, VecDeque};

use continuum_engine_reference::bfs::{self, Bound, Bounds, Discovery, Exploration};
use continuum_engine_reference::diehard;
use continuum_engine_reference::expr::{BoolExpr, CmpOp, IntExpr};
use continuum_engine_reference::model::{ActionDecl, Model, ModelBuilder, State};

fn die_hard() -> Model {
    diehard::model().expect("the Die Hard transcription is a valid model")
}

/// A chain: `n` counts from `0` to `limit` and then stops.
fn counter(limit: i64) -> Model {
    ModelBuilder::new()
        .variable("n", 0, limit)
        .initial_state(&[("n", 0)])
        .action(ActionDecl::deterministic(
            "Step",
            BoolExpr::compare(CmpOp::Lt, IntExpr::var("n"), IntExpr::constant(limit)),
            vec![("n", IntExpr::plus(IntExpr::var("n"), IntExpr::constant(1)))],
        ))
        .build()
        .expect("the counter is a valid model")
}

/// Two differently named actions with the same effect, so one target is reached under
/// two action indices and the recorded label has to choose.
fn twins() -> Model {
    ModelBuilder::new()
        .variable("x", 0, 1)
        .initial_state(&[("x", 0)])
        .action(ActionDecl::deterministic(
            "Alpha",
            BoolExpr::Const(true),
            vec![("x", IntExpr::constant(1))],
        ))
        .action(ActionDecl::deterministic(
            "Beta",
            BoolExpr::Const(true),
            vec![("x", IntExpr::constant(1))],
        ))
        .build()
        .expect("the twin actions are a valid model")
}

fn vectors(states: &[State]) -> Vec<Vec<i64>> {
    states.iter().map(|s| s.as_slice().to_vec()).collect()
}

fn explore(model: &Model, bounds: Bounds) -> Exploration {
    bfs::explore(model, bounds).expect("this model evaluates everywhere")
}

/// The depth map, plus the expansion and transition counts, computed by a local
/// closure that knows nothing about origins. The pre-origin exploration's whole
/// observable surface for a complete run.
fn hand_walk(model: &Model) -> (BTreeMap<Vec<i64>, usize>, usize, u64) {
    let mut depth: BTreeMap<Vec<i64>, usize> = BTreeMap::new();
    let mut frontier: VecDeque<State> = VecDeque::new();
    for initial in model.initial_states() {
        if !depth.contains_key(initial.as_slice()) {
            depth.insert(initial.as_slice().to_vec(), 0);
            frontier.push_back(initial.clone());
        }
    }
    let mut expanded = 0_usize;
    let mut transitions = 0_u64;
    while let Some(here) = frontier.pop_front() {
        let here_depth = *depth
            .get(here.as_slice())
            .expect("queued states have depths");
        let row = model.successors(&here).expect("this model evaluates");
        expanded += 1;
        transitions += row.len() as u64;
        for step in row {
            let target = step.target();
            if !depth.contains_key(target.as_slice()) {
                depth.insert(target.as_slice().to_vec(), here_depth + 1);
                frontier.push_back(target.clone());
            }
        }
    }
    (depth, expanded, transitions)
}

/// Every recorded discovery, checked against the model: one layer shallower, a real
/// labelled transition, and the least action index that reaches the state.
fn check_origins(model: &Model, exploration: &Exploration) {
    let reachable = exploration.reachable();
    assert_eq!(reachable.origins().len(), reachable.len());
    for ((state, depth), origin) in reachable
        .states()
        .iter()
        .zip(reachable.depths().iter())
        .zip(reachable.origins().iter())
    {
        match origin {
            Discovery::Initial => {
                assert_eq!(*depth, 0, "{state} is initial but not at depth 0");
                assert!(
                    model.initial_states().contains(state),
                    "{state} claims to be initial and is not declared so"
                );
            }
            Discovery::Step {
                predecessor,
                action,
            } => {
                assert_eq!(
                    reachable.depth_of(predecessor),
                    depth.checked_sub(1),
                    "{predecessor} -> {state} does not cross exactly one layer"
                );
                let row = model.successors(predecessor).expect("evaluates");
                let reaching: Vec<usize> = row
                    .iter()
                    .filter(|step| step.target() == state)
                    .map(continuum_engine_reference::Step::action)
                    .collect();
                assert!(
                    reaching.contains(action),
                    "{predecessor} --{action}--> {state} is not a transition of the model"
                );
                assert_eq!(
                    reaching.first(),
                    Some(action),
                    "the recorded label is not the least action reaching {state}"
                );
            }
        }
    }
}

// ---------------------------------------------------------------------------
// what did not change
// ---------------------------------------------------------------------------

/// The engine and an origin-blind hand walk agree on the whole pre-origin surface:
/// which states, at which depths, with how many expansions and how many transitions.
#[test]
fn the_die_hard_walk_is_unchanged_by_recording_predecessors() {
    let model = die_hard();
    let exploration = explore(&model, Bounds::CERTIFIABLE);
    let reachable = exploration.reachable();
    let by_engine: BTreeMap<Vec<i64>, usize> = reachable
        .states()
        .iter()
        .zip(reachable.depths().iter())
        .map(|(s, d)| (s.as_slice().to_vec(), *d))
        .collect();
    let (by_hand, expanded, transitions) = hand_walk(&model);
    assert_eq!(by_engine, by_hand);
    assert_eq!(reachable.expanded(), expanded);
    assert_eq!(reachable.transitions(), transitions);
    // And the frozen numbers themselves, so this file does not merely agree with a
    // walk that could have drifted with it.
    assert_eq!(reachable.len(), 16);
    assert_eq!(reachable.transitions(), 96);
    assert_eq!(reachable.max_depth(), Some(7));
}

/// The same agreement on the contract suite's models, which cover the shapes Die Hard
/// does not have: a chain, an enumerated fork, two actions to one target.
#[test]
fn the_small_models_are_unchanged_too() {
    for model in [counter(4), counter(20), twins()] {
        let exploration = explore(&model, Bounds::CERTIFIABLE);
        let reachable = exploration.reachable();
        let by_engine: BTreeMap<Vec<i64>, usize> = reachable
            .states()
            .iter()
            .zip(reachable.depths().iter())
            .map(|(s, d)| (s.as_slice().to_vec(), *d))
            .collect();
        let (by_hand, expanded, transitions) = hand_walk(&model);
        assert_eq!(by_engine, by_hand);
        assert_eq!(reachable.expanded(), expanded);
        assert_eq!(reachable.transitions(), transitions);
    }
}

/// The frontier of a bounded run, in **queue order**, is the sequence
/// `tests/bfs_diehard.rs` froze before origins existed.
///
/// This is the assertion that admission order did not move. Every other observable is
/// a set or an ascending table and would survive a reordering; the frontier would not.
#[test]
fn the_frozen_queue_ordered_frontier_is_unchanged() {
    let model = die_hard();
    let bounds = Bounds::CERTIFIABLE.with_depth(2);
    let exploration = explore(&model, bounds);
    let partial = exploration.exhausted().expect("the depth bound tripped");
    assert_eq!(partial.tripped(), Bound::Depth);
    assert_eq!(
        vectors(partial.frontier()),
        vec![vec![3, 0], vec![5, 3], vec![2, 3]]
    );
    assert_eq!(
        vectors(partial.explored().states()),
        vec![
            vec![0, 0],
            vec![0, 3],
            vec![2, 3],
            vec![3, 0],
            vec![5, 0],
            vec![5, 3],
        ]
    );
    assert_eq!(partial.explored().depths(), [0, 1, 2, 2, 1, 2]);
    assert_eq!(partial.explored().expanded(), 3);
    assert_eq!(partial.explored().transitions(), 18);
}

/// The other two bounds stop where they stopped before, with the same frontiers.
#[test]
fn the_other_frozen_partial_results_are_unchanged() {
    let model = die_hard();
    let by_states = explore(&model, Bounds::CERTIFIABLE.with_states(3));
    let partial = by_states.exhausted().expect("the state bound tripped");
    assert_eq!(partial.tripped(), Bound::States);
    assert_eq!(vectors(partial.frontier()), vec![vec![0, 3], vec![5, 0]]);

    let by_transitions = explore(&model, Bounds::CERTIFIABLE.with_transitions(10));
    let partial = by_transitions
        .exhausted()
        .expect("the transition bound tripped");
    assert_eq!(partial.tripped(), Bound::Transitions);
    assert_eq!(partial.explored().transitions(), 6);
    assert_eq!(vectors(partial.frontier()), vec![vec![0, 3], vec![5, 0]]);
}

// ---------------------------------------------------------------------------
// what the column says
// ---------------------------------------------------------------------------

/// Every origin, on every model, is a real transition one layer up.
#[test]
fn every_origin_is_a_labelled_transition_one_layer_shallower() {
    for model in [die_hard(), counter(4), counter(20), twins()] {
        let exploration = explore(&model, Bounds::CERTIFIABLE);
        check_origins(&model, &exploration);
    }
}

/// A target reached under two actions records the smaller index, which is the one the
/// successor row lists first.
#[test]
fn a_target_reached_twice_records_the_least_action() {
    let model = twins();
    let exploration = explore(&model, Bounds::CERTIFIABLE);
    let reachable = exploration.closed().expect("the twins close");
    let one = model.state(&[1]).expect("1 is a state");
    let start = model.state(&[0]).expect("0 is a state");
    assert_eq!(
        reachable.origin_of(&one),
        Some(&Discovery::Step {
            predecessor: start,
            action: 0,
        })
    );
    // Both actions do reach it — the row has two entries with the same target — so the
    // choice above is a choice and not the only option.
    let row = model
        .successors(&model.state(&[0]).expect("0 is a state"))
        .expect("evaluates");
    assert_eq!(row.len(), 2);
    for step in &row {
        assert_eq!(step.target(), &one);
    }
    assert_eq!(
        model
            .actions()
            .iter()
            .map(|action| action.name().as_str())
            .collect::<Vec<_>>(),
        vec!["Alpha", "Beta"]
    );
}

/// Exactly the declared initial states have no predecessor.
#[test]
fn only_the_declared_initial_states_have_no_predecessor() {
    let model = die_hard();
    let exploration = explore(&model, Bounds::CERTIFIABLE);
    let reachable = exploration.reachable();
    let initial: Vec<&State> = reachable
        .states()
        .iter()
        .zip(reachable.origins().iter())
        .filter_map(|(state, origin)| matches!(origin, Discovery::Initial).then_some(state))
        .collect();
    assert_eq!(initial, vec![&model.state(&[0, 0]).expect("Init")]);
    assert_eq!(initial.len(), model.initial_states().len());
}

/// A state outside the discovered set has no recorded origin, and the answer is the
/// same `None` [`bfs::Reachable::depth_of`] gives.
#[test]
fn a_state_outside_the_set_has_no_origin() {
    let model = counter(4);
    let exploration = explore(&model, Bounds::CERTIFIABLE.with_states(3));
    let reachable = exploration.reachable();
    let beyond = model.state(&[4]).expect("4 is a counter state");
    assert_eq!(reachable.origin_of(&beyond), None);
    assert_eq!(reachable.depth_of(&beyond), None);
}

/// A bounded run records origins for what it did discover, frontier states included —
/// the column is not something only a complete exploration has.
#[test]
fn a_bounded_exploration_records_origins_for_what_it_found() {
    let model = die_hard();
    let exploration = explore(&model, Bounds::CERTIFIABLE.with_depth(2));
    check_origins(&model, &exploration);
    let partial = exploration.exhausted().expect("the depth bound tripped");
    for waiting in partial.frontier() {
        assert!(
            partial.explored().origin_of(waiting).is_some(),
            "the frontier state {waiting} has no recorded origin"
        );
    }
}

/// Two explorations of one model record the same origins, byte for byte.
#[test]
fn origins_are_deterministic() {
    let run = || explore(&die_hard(), Bounds::CERTIFIABLE);
    let first = run();
    let second = run();
    assert_eq!(first.reachable().origins(), second.reachable().origins());
    assert_eq!(
        format!("{:?}", first.reachable().origins()).as_bytes(),
        format!("{:?}", second.reachable().origins()).as_bytes()
    );
    assert_eq!(first, second);
}
