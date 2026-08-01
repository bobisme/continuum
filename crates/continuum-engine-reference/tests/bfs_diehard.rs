//! The frozen Die Hard facts, asserted **through the engine** (PR 8, IMPL-02).
//!
//! # What changed, and why this file exists beside the other one
//!
//! `tests/diehard_evidence.rs` asserts the same numbers by walking the model with a
//! nine-line `VecDeque` closure written *inside that test*, deliberately, so that the
//! model layer never had a private path to the answer. That file stays exactly as it
//! is. This one re-derives the same facts through [`continuum_engine_reference::bfs`],
//! and the two are now a differential pair: the engine and the hand walk are
//! independent implementations of the same breadth-first definition, and they must
//! agree state for state and depth for depth. `the_engine_agrees_with_the_hand_walk`
//! makes that comparison the assertion rather than the assumption.
//!
//! # The frozen facts
//!
//! | Fact | Sources |
//! |---|---|
//! | 16 reachable states | `notes/plan/spikes/SPIKE_REPORT.md:9-12`; `notes/plan/docs/27_SPIKE_FINDINGS_REV2.md:24-27`; `notes/plan/corpus/tla-examples/ports/TV-009/port.json:31-35`; `crates/continuum-kernel-core/src/fixture.rs:11-14`; `lean/Continuum/Examples/DieHard.lean:66-74` |
//! | 96 labelled transitions | the same four, plus `crates/continuum-kernel-core/src/check.rs:254-257` |
//! | shortest `big == 4` witness at depth 6 | the same four, plus `lean/Continuum/Examples/DieHard.lean:14-26` |
//!
//! The fourth frozen fact — "exact closure/type certificate accepted by an independent
//! checker" — needs certificate emission, which is PR 8's fifth bone, and is not
//! asserted here either.

use std::collections::{BTreeMap, VecDeque};

use continuum_engine_reference::bfs::{self, Bound, Bounds, Exploration};
use continuum_engine_reference::diehard;
use continuum_engine_reference::model::{Model, State};

fn model() -> Model {
    diehard::model().expect("the Die Hard transcription is a valid model")
}

fn state(model: &Model, big: i64, small: i64) -> State {
    model
        .state(&[big, small])
        .unwrap_or_else(|error| panic!("({big}, {small}) is a Die Hard state: {error}"))
}

/// The complete exploration, which every test below starts from.
fn complete(model: &Model) -> Exploration {
    bfs::explore(model, Bounds::CERTIFIABLE).expect("Die Hard evaluates everywhere")
}

fn vectors(states: &[State]) -> Vec<Vec<i64>> {
    states.iter().map(|s| s.as_slice().to_vec()).collect()
}

/// The depth map `tests/diehard_evidence.rs` computes, recomputed here by its own
/// independent means so the two can be compared.
fn hand_walk(model: &Model) -> BTreeMap<Vec<i64>, usize> {
    let mut depth: BTreeMap<Vec<i64>, usize> = BTreeMap::new();
    let mut frontier: VecDeque<State> = VecDeque::new();
    for initial in model.initial_states() {
        if !depth.contains_key(initial.as_slice()) {
            depth.insert(initial.as_slice().to_vec(), 0);
            frontier.push_back(initial.clone());
        }
    }
    while let Some(here) = frontier.pop_front() {
        let here_depth = *depth
            .get(here.as_slice())
            .expect("queued states have depths");
        for step in model.successors(&here).expect("Die Hard evaluates") {
            let target = step.target();
            if !depth.contains_key(target.as_slice()) {
                depth.insert(target.as_slice().to_vec(), here_depth + 1);
                frontier.push_back(target.clone());
            }
        }
    }
    depth
}

// ---------------------------------------------------------------------------
// the frozen facts, through the engine
// ---------------------------------------------------------------------------

/// 16 reachable states, and *which* 16 — the same list
/// `lean/Continuum/Examples/DieHard.lean:66-74` enumerates, the same table
/// `crates/continuum-kernel-core/src/fixture.rs:281-303` builds, and the same list
/// `tests/diehard_evidence.rs` freezes.
#[test]
fn the_engine_finds_the_sixteen_frozen_states() {
    let model = model();
    let exploration = complete(&model);
    let reachable = exploration.reachable();
    assert_eq!(reachable.len(), 16);
    assert_eq!(
        vectors(reachable.states()),
        vec![
            vec![0, 0],
            vec![0, 1],
            vec![0, 2],
            vec![0, 3],
            vec![1, 0],
            vec![1, 3],
            vec![2, 0],
            vec![2, 3],
            vec![3, 0],
            vec![3, 3],
            vec![4, 0],
            vec![4, 3],
            vec![5, 0],
            vec![5, 1],
            vec![5, 2],
            vec![5, 3],
        ]
    );
}

/// 96 labelled transitions, counted by the engine the way the wire form counts them.
#[test]
fn the_engine_counts_ninety_six_labelled_transitions() {
    let model = model();
    let exploration = complete(&model);
    let reachable = exploration.reachable();
    assert_eq!(reachable.transitions(), 96);
    // Every state was expanded, so the count covers the whole set: 16 rows of 6.
    assert_eq!(reachable.expanded(), 16);
    assert_eq!(reachable.expanded(), reachable.len());
}

/// The shortest state with four gallons in the big jug is `(4, 3)` at depth 6 —
/// `lean/Continuum/Examples/DieHard.lean:24-26` proves the same endpoint. `(4, 0)` is
/// also reachable, one layer further out.
#[test]
fn the_shortest_four_gallon_state_is_at_depth_six() {
    let model = model();
    let exploration = complete(&model);
    let reachable = exploration.reachable();
    assert_eq!(
        reachable.depth_of(&state(&model, 4, 3)),
        Some(6),
        "the film's answer"
    );
    assert_eq!(reachable.depth_of(&state(&model, 4, 0)), Some(7));
    let shortest = reachable
        .states()
        .iter()
        .zip(reachable.depths().iter())
        .filter(|(s, _)| s.as_slice().first() == Some(&diehard::TARGET_GALLONS))
        .map(|(_, depth)| *depth)
        .min();
    assert_eq!(shortest, Some(6));
}

/// The whole depth map, spelled out. This is the artifact the shortest-witness bone
/// (IMPL-04) reads, so it is frozen here rather than sampled.
#[test]
fn the_depth_map_is_frozen() {
    let model = model();
    let exploration = complete(&model);
    let reachable = exploration.reachable();
    let map: Vec<(Vec<i64>, usize)> = reachable
        .states()
        .iter()
        .zip(reachable.depths().iter())
        .map(|(s, d)| (s.as_slice().to_vec(), *d))
        .collect();
    assert_eq!(
        map,
        vec![
            (vec![0, 0], 0),
            (vec![0, 1], 5),
            (vec![0, 2], 4),
            (vec![0, 3], 1),
            (vec![1, 0], 6),
            (vec![1, 3], 7),
            (vec![2, 0], 3),
            (vec![2, 3], 2),
            (vec![3, 0], 2),
            (vec![3, 3], 3),
            (vec![4, 0], 7),
            (vec![4, 3], 6),
            (vec![5, 0], 1),
            (vec![5, 1], 4),
            (vec![5, 2], 5),
            (vec![5, 3], 2),
        ]
    );
    assert_eq!(reachable.max_depth(), Some(7));
}

/// The same map read layer by layer. Eight layers, and every state appears in exactly
/// one of them.
#[test]
fn the_eight_layers_partition_the_reachable_set() {
    let model = model();
    let exploration = complete(&model);
    let reachable = exploration.reachable();
    let layers: Vec<Vec<Vec<i64>>> = (0..=7)
        .map(|depth| {
            reachable
                .layer(depth)
                .iter()
                .map(|s| s.as_slice().to_vec())
                .collect()
        })
        .collect();
    assert_eq!(
        layers,
        vec![
            vec![vec![0, 0]],
            vec![vec![0, 3], vec![5, 0]],
            vec![vec![2, 3], vec![3, 0], vec![5, 3]],
            vec![vec![2, 0], vec![3, 3]],
            vec![vec![0, 2], vec![5, 1]],
            vec![vec![0, 1], vec![5, 2]],
            vec![vec![1, 0], vec![4, 3]],
            vec![vec![1, 3], vec![4, 0]],
        ]
    );
    let counted: usize = layers.iter().map(Vec::len).sum();
    assert_eq!(counted, reachable.len());
    assert!(reachable.layer(8).is_empty(), "there is no ninth layer");
}

/// `Post(S) ⊆ S` — the closure conjunct of docs/03 §6.1 — re-checked against the set
/// the engine returned. The `Complete` arm asserts it by construction; this test is
/// the independent confirmation that the arm is telling the truth.
#[test]
fn the_complete_set_is_closed_under_the_transition_relation() {
    let model = model();
    let exploration = complete(&model);
    let reachable = exploration
        .closed()
        .expect("Die Hard fits inside the certifiable bounds");
    for here in reachable.states() {
        for step in model.successors(here).expect("evaluates") {
            assert!(
                reachable.contains(step.target()),
                "{here} -> {} escapes S",
                step.target()
            );
        }
    }
}

// ---------------------------------------------------------------------------
// the engine against the hand walk
// ---------------------------------------------------------------------------

/// The engine and `tests/diehard_evidence.rs`'s local closure agree exactly.
///
/// Two independent breadth-first implementations over one model layer. A disagreement
/// here means one of them is wrong, and the frozen numbers say which.
#[test]
fn the_engine_agrees_with_the_hand_walk() {
    let model = model();
    let exploration = complete(&model);
    let reachable = exploration.reachable();
    let by_engine: BTreeMap<Vec<i64>, usize> = reachable
        .states()
        .iter()
        .zip(reachable.depths().iter())
        .map(|(s, d)| (s.as_slice().to_vec(), *d))
        .collect();
    assert_eq!(by_engine, hand_walk(&model));
}

/// The state table the engine hands the certificate bone is strictly ascending, which
/// is the wire form's rule for a canonical state table
/// (`crates/continuum-kernel-core/src/wire.rs:89-92, 616-620`).
#[test]
fn the_state_table_is_strictly_ascending() {
    let model = model();
    let exploration = complete(&model);
    let table = vectors(exploration.reachable().states());
    for pair in table.windows(2) {
        assert!(pair[0] < pair[1], "{pair:?}");
    }
}

// ---------------------------------------------------------------------------
// determinism
// ---------------------------------------------------------------------------

/// Two explorations of one model under one bound are equal, and byte-identical when
/// rendered. INV-005 at the exploration grain.
#[test]
fn two_explorations_of_die_hard_are_byte_identical() {
    let first = complete(&model());
    let second = complete(&model());
    assert_eq!(first, second);
    let rendered = format!("{first:?}");
    assert_eq!(rendered.as_bytes(), format!("{second:?}").as_bytes());
    assert!(!rendered.is_empty());
}

/// A bounded exploration is deterministic too — including its frontier, which is the
/// part that is *not* in sorted order.
#[test]
fn two_bounded_explorations_of_die_hard_are_byte_identical() {
    let bounds = Bounds::CERTIFIABLE.with_depth(2);
    let run = || bfs::explore(&model(), bounds).expect("Die Hard evaluates");
    let first = run();
    let second = run();
    assert_eq!(first, second);
    assert_eq!(
        format!("{first:?}").as_bytes(),
        format!("{second:?}").as_bytes()
    );
}

// ---------------------------------------------------------------------------
// Die Hard under each bound
// ---------------------------------------------------------------------------

/// Stopping at depth 2: three layers discovered, three states expanded, and the
/// frontier is the queue where it stood.
#[test]
fn die_hard_bounded_by_depth_stops_at_the_third_layer() {
    let model = model();
    let bounds = Bounds::CERTIFIABLE.with_depth(2);
    let exploration = bfs::explore(&model, bounds).expect("Die Hard evaluates");
    assert!(
        exploration.closed().is_none(),
        "a bounded set is not closed"
    );
    let partial = exploration.exhausted().expect("the depth bound tripped");
    assert_eq!(partial.tripped(), Bound::Depth);
    assert_eq!(partial.bounds(), bounds);
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
    // Queue order, not ascending order: `(3, 0)` is the expansion that tripped the
    // bound and went back to the head.
    assert_eq!(
        vectors(partial.frontier()),
        vec![vec![3, 0], vec![5, 3], vec![2, 3]]
    );
}

/// Stopping at three states: the initial state is admitted and expanded, and its two
/// successors' own expansion would take the set to five.
#[test]
fn die_hard_bounded_by_states_stops_before_the_fourth_state() {
    let model = model();
    let bounds = Bounds::CERTIFIABLE.with_states(3);
    let exploration = bfs::explore(&model, bounds).expect("Die Hard evaluates");
    let partial = exploration.exhausted().expect("the state bound tripped");
    assert_eq!(partial.tripped(), Bound::States);
    assert_eq!(
        vectors(partial.explored().states()),
        vec![vec![0, 0], vec![0, 3], vec![5, 0]]
    );
    assert_eq!(partial.explored().depths(), [0, 1, 1]);
    assert_eq!(partial.explored().expanded(), 1);
    assert_eq!(partial.explored().transitions(), 6);
    assert_eq!(
        vectors(partial.frontier()),
        vec![vec![0, 3], vec![5, 0]],
        "both discovered states are still unexpanded"
    );
}

/// Stopping at ten transitions: one six-transition row fits, a second would make
/// twelve. The count never passes the bound, and it is not clipped to it either.
#[test]
fn die_hard_bounded_by_transitions_stops_between_rows() {
    let model = model();
    let bounds = Bounds::CERTIFIABLE.with_transitions(10);
    let exploration = bfs::explore(&model, bounds).expect("Die Hard evaluates");
    let partial = exploration
        .exhausted()
        .expect("the transition bound tripped");
    assert_eq!(partial.tripped(), Bound::Transitions);
    assert_eq!(partial.explored().transitions(), 6);
    assert!(partial.explored().transitions() <= bounds.transitions());
    assert_eq!(partial.explored().expanded(), 1);
    assert_eq!(vectors(partial.frontier()), vec![vec![0, 3], vec![5, 0]]);
}

/// Every bounded run is a *prefix* of the complete one: the states it found are
/// reachable states, at the depths the complete walk gives them. That is what makes
/// the frontier a resume point rather than a snapshot of a different search.
#[test]
fn every_bounded_run_is_a_depth_preserving_prefix_of_the_complete_one() {
    let model = model();
    let whole = complete(&model);
    let whole = whole.reachable();
    for bounds in [
        Bounds::CERTIFIABLE.with_depth(2),
        Bounds::CERTIFIABLE.with_states(3),
        Bounds::CERTIFIABLE.with_transitions(10),
        Bounds::CERTIFIABLE.with_states(9).with_depth(4),
    ] {
        let exploration = bfs::explore(&model, bounds).expect("Die Hard evaluates");
        let partial = exploration.exhausted().expect("every bound above trips");
        for (found, depth) in partial
            .explored()
            .states()
            .iter()
            .zip(partial.explored().depths().iter())
        {
            assert_eq!(
                whole.depth_of(found),
                Some(*depth),
                "{found} at {depth} under {bounds:?}"
            );
        }
        for waiting in partial.frontier() {
            assert!(
                partial.explored().contains(waiting),
                "the frontier state {waiting} is not in the explored set"
            );
        }
        assert!(
            partial.explored().expanded() < partial.explored().len(),
            "a partial run leaves states unexpanded"
        );
    }
}

/// Raising a bound past what the model needs gives back the complete answer. There is
/// no residue of having been bounded.
#[test]
fn bounds_wide_enough_for_die_hard_return_the_complete_set() {
    let model = model();
    let exact = Bounds::new(16, 7, 96);
    let exploration = bfs::explore(&model, exact).expect("Die Hard evaluates");
    assert!(exploration.is_complete(), "16 states, depth 7, 96 edges");
    assert_eq!(Some(exploration.reachable()), complete(&model).closed());
}

/// One state fewer, one layer shallower, or one transition fewer, and it does not.
/// The bounds are exact, not approximate.
#[test]
fn bounds_one_short_of_die_hard_do_not() {
    let model = model();
    for (bounds, expected) in [
        (Bounds::new(15, 7, 96), Bound::States),
        (Bounds::new(16, 6, 96), Bound::Depth),
        (Bounds::new(16, 7, 95), Bound::Transitions),
    ] {
        let exploration = bfs::explore(&model, bounds).expect("Die Hard evaluates");
        let partial = exploration.exhausted().expect("one short is one short");
        assert_eq!(partial.tripped(), expected, "{bounds:?}");
    }
}

/// The crate root re-exports the entry point, so a caller need not name the module.
#[test]
fn the_crate_root_re_exports_the_entry_point() {
    let model = model();
    let exploration = continuum_engine_reference::explore(
        &model,
        continuum_engine_reference::Bounds::CERTIFIABLE,
    )
    .expect("Die Hard evaluates");
    assert_eq!(exploration.reachable().len(), 16);
}
