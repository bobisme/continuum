//! Die Hard (corpus TV-009) as evidence for the model layer (PR 8, IMPL-01).
//!
//! # What this file may and may not do
//!
//! The exploration bone is not written yet, so the walk below is a **local closure in
//! this test**, not an engine module. That is the point: the frozen facts have to be
//! reachable from the model layer's public primitives alone
//! ([`Model::initial_states`], [`Model::successors`], [`Model::evaluate_predicate`]),
//! so that when the deterministic-BFS bone lands it is measured against a model that
//! never had a private path to the answer. Nine lines of `VecDeque` here is the
//! evidence that the seam is real.
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
//! checker" — is deliberately *not* asserted here. It needs certificate emission, which
//! is PR 8's fifth bone; this file must not pretend to cover it.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use continuum_engine_reference::diehard;
use continuum_engine_reference::model::{Model, State};

/// The reachable set, as vectors, with each state's BFS depth from the initial states.
///
/// A plain closure walk: no engine module is involved, and it uses only the model
/// layer's public surface.
fn reachable(model: &Model) -> BTreeMap<Vec<i64>, usize> {
    let mut depth: BTreeMap<Vec<i64>, usize> = BTreeMap::new();
    let mut frontier: VecDeque<State> = VecDeque::new();
    // `insert` overwrites, so a state's depth is only ever *written* the first time it
    // is seen; breadth-first order makes that first write the shortest one.
    for initial in model.initial_states() {
        if !depth.contains_key(initial.as_slice()) {
            depth.insert(initial.as_slice().to_vec(), 0);
            frontier.push_back(initial.clone());
        }
    }
    while let Some(here) = frontier.pop_front() {
        let here_depth = *depth
            .get(here.as_slice())
            .expect("a state on the frontier has a depth");
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

fn model() -> Model {
    diehard::model().expect("the Die Hard transcription is a valid model")
}

fn state(model: &Model, big: i64, small: i64) -> State {
    model
        .state(&[big, small])
        .unwrap_or_else(|error| panic!("({big}, {small}) is a Die Hard state: {error}"))
}

/// `(action name, target)` for a state, which is what a certificate row carries.
fn labelled(model: &Model, from: &State) -> Vec<(String, Vec<i64>)> {
    model
        .successors(from)
        .expect("Die Hard evaluates")
        .iter()
        .map(|step| {
            let name = model
                .actions()
                .get(step.action())
                .expect("a step names a declared action")
                .name()
                .to_string();
            (name, step.target().as_slice().to_vec())
        })
        .collect()
}

// ---------------------------------------------------------------------------
// the transcription
// ---------------------------------------------------------------------------

/// `DieHard.ctm:3-6` — two variables, with the capacities as their domains, in
/// canonical order.
#[test]
fn the_state_domain_is_the_corpus_state_block() {
    let model = model();
    let declared: Vec<(&str, i64, i64)> = model
        .variables()
        .iter()
        .map(|variable| {
            (
                variable.name().as_str(),
                variable.domain().lo(),
                variable.domain().hi(),
            )
        })
        .collect();
    assert_eq!(declared, vec![("big", 0, 5), ("small", 0, 3)]);
    // The same two variables with the same two ranges the kernel's decoder expects
    // (`crates/continuum-kernel-core/src/wire.rs:493-496`).
    assert_eq!(model.arity(), 2);
    assert_eq!(model.domain_cardinality(), 24);
}

/// `DieHard.ctm:8` — `init Init { big == 0 && small == 0 }`, a single state.
#[test]
fn the_initial_state_is_both_jugs_empty() {
    let model = model();
    let initial: Vec<&[i64]> = model.initial_states().iter().map(State::as_slice).collect();
    assert_eq!(initial, vec![[0, 0].as_slice()]);
}

/// `DieHard.ctm:10-25` — six actions. Their indices here are the indices the
/// certificate's action table would give them, and they agree with the kernel test
/// fixture's ordering (`crates/continuum-kernel-core/src/fixture.rs:247-256`).
#[test]
fn the_six_actions_are_indexed_in_wire_order() {
    let model = model();
    let names: Vec<&str> = model
        .actions()
        .iter()
        .map(|action| action.name().as_str())
        .collect();
    assert_eq!(
        names,
        vec![
            diehard::BIG_TO_SMALL,
            diehard::EMPTY_BIG,
            diehard::EMPTY_SMALL,
            diehard::FILL_BIG,
            diehard::FILL_SMALL,
            diehard::SMALL_TO_BIG,
        ]
    );
    // Strictly ascending in *byte* order, which is the wire form's rule
    // (`crates/continuum-kernel-core/src/wire.rs:311-314, 754-761`).
    for pair in names.windows(2) {
        assert!(pair[0].as_bytes() < pair[1].as_bytes(), "{pair:?}");
    }
    // The kernel fixture's kebab-case names sort to the same relative order, so the
    // action *indices* agree even though the spellings do not.
    let kernel_fixture = [
        "big-to-small",
        "empty-big",
        "empty-small",
        "fill-big",
        "fill-small",
        "small-to-big",
    ];
    let mut sorted = kernel_fixture;
    sorted.sort_unstable();
    assert_eq!(sorted, kernel_fixture);
    assert_eq!(sorted.len(), names.len());
}

/// All six actions are unguarded, which is what makes the frozen edge count 96 rather
/// than something smaller.
#[test]
fn every_action_is_enabled_in_every_reachable_state() {
    let model = model();
    for vector in reachable(&model).keys() {
        let here = model
            .state(vector)
            .expect("reachable states are well typed");
        for index in 0..model.actions().len() {
            assert_eq!(
                model.is_enabled(index, &here),
                Ok(true),
                "action {index} at {vector:?}"
            );
        }
    }
}

/// `DieHard.ctm:30-31` — two named predicates, and nothing here says which is an
/// invariant and which is a goal.
#[test]
fn the_two_predicates_are_declared_and_ordered() {
    let model = model();
    let names: Vec<&str> = model
        .predicates()
        .iter()
        .map(|predicate| predicate.name().as_str())
        .collect();
    assert_eq!(names, vec![diehard::NOT_SOLVED, diehard::TYPE_OK]);
}

// ---------------------------------------------------------------------------
// the frozen facts
// ---------------------------------------------------------------------------

/// 16 reachable states, and *which* 16: the same list
/// `lean/Continuum/Examples/DieHard.lean:66-74` enumerates and the same table
/// `crates/continuum-kernel-core/src/fixture.rs:281-303` builds.
#[test]
fn the_reachable_set_is_the_sixteen_frozen_states() {
    let model = model();
    let states: Vec<Vec<i64>> = reachable(&model).into_keys().collect();
    assert_eq!(states.len(), 16);
    assert_eq!(
        states,
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

/// 96 labelled transitions: 16 states with 6 distinct `(action, target)` pairs each.
#[test]
fn the_reachable_set_carries_ninety_six_labelled_transitions() {
    let model = model();
    let mut total = 0_usize;
    for vector in reachable(&model).keys() {
        let here = model.state(vector).expect("well typed");
        let row = model.successors(&here).expect("evaluates");
        assert_eq!(row.len(), 6, "row at {vector:?}");
        total += row.len();
    }
    assert_eq!(total, 96);
}

/// The reachable set is closed under the transition relation — `Post(S) ⊆ S`, the
/// second of RFC 0005's three finite-safety conjuncts. Asserted here as a property of
/// the *model*; carrying it in a certificate is the fifth bone's job.
#[test]
fn the_reachable_set_is_closed_and_well_typed() {
    let model = model();
    let states: BTreeSet<Vec<i64>> = reachable(&model).into_keys().collect();
    for vector in &states {
        assert!(
            model.admits(vector),
            "{vector:?} is outside the state domain"
        );
        let here = model.state(vector).expect("well typed");
        for step in model.successors(&here).expect("evaluates") {
            let target = step.target().as_slice().to_vec();
            assert!(
                states.contains(&target),
                "{vector:?} -> {target:?} escapes S"
            );
        }
    }
}

/// The shortest state with four gallons in the big jug is at depth 6, and it is
/// `(4, 3)` — `lean/Continuum/Examples/DieHard.lean:24-26` proves the same endpoint.
#[test]
fn the_shortest_four_gallon_state_is_at_depth_six() {
    let model = model();
    let depths = reachable(&model);
    let four: Vec<(&Vec<i64>, &usize)> = depths
        .iter()
        .filter(|(vector, _)| vector.first() == Some(&diehard::TARGET_GALLONS))
        .collect();
    assert_eq!(four, vec![(&vec![4, 0], &7_usize), (&vec![4, 3], &6_usize)]);
    let shortest = four.iter().map(|(_, depth)| **depth).min();
    assert_eq!(shortest, Some(6));
}

/// The film's solution, replayed one transition at a time against the model.
///
/// The path is `lean/Continuum/Examples/DieHard.lean:14-22`. Each consecutive pair
/// must be a genuine labelled transition, which is what makes this a *witness* the
/// shortest-witness bone can later be checked against.
#[test]
fn the_six_step_solution_replays_as_real_transitions() {
    let model = model();
    let path = [[0, 0], [5, 0], [2, 3], [2, 0], [0, 2], [5, 2], [4, 3]];
    let expected_actions = [
        diehard::FILL_BIG,
        diehard::BIG_TO_SMALL,
        diehard::EMPTY_SMALL,
        diehard::BIG_TO_SMALL,
        diehard::FILL_BIG,
        diehard::BIG_TO_SMALL,
    ];
    assert_eq!(path.len(), 7, "six steps means seven states");
    assert_eq!(
        path.first().map(|first| first.to_vec()),
        model
            .initial_states()
            .first()
            .map(|s| s.as_slice().to_vec()),
        "the path starts at Init"
    );
    for (index, pair) in path.windows(2).enumerate() {
        let from = state(&model, pair[0][0], pair[0][1]);
        let row = labelled(&model, &from);
        let wanted = expected_actions
            .get(index)
            .expect("one action per step")
            .to_owned();
        let target = pair[1].to_vec();
        assert!(
            row.contains(&(wanted.to_owned(), target.clone())),
            "step {index}: {:?} --{wanted}--> {target:?} is not in {row:?}",
            pair[0]
        );
    }
    let last = path.last().expect("non-empty");
    assert_eq!(last[0], diehard::TARGET_GALLONS);
}

// ---------------------------------------------------------------------------
// predicate evaluation at chosen states
// ---------------------------------------------------------------------------

/// `TypeOK` holds everywhere reachable. Over this model it is a tautology of the
/// declared domains, which is exactly why the *interesting* claim is about the
/// reachable set and belongs in a certificate.
#[test]
fn type_ok_holds_at_every_reachable_state() {
    let model = model();
    let index = model
        .predicate_index(diehard::TYPE_OK)
        .expect("TypeOK is declared");
    for vector in reachable(&model).keys() {
        let here = model.state(vector).expect("well typed");
        assert_eq!(
            model.evaluate_predicate(index, &here),
            Ok(true),
            "{vector:?}"
        );
    }
}

/// `NotSolved` is "intentionally false" (`DieHard.ctm:31`) — and false at exactly the
/// two reachable states with four gallons in the big jug.
#[test]
fn not_solved_fails_at_exactly_the_two_four_gallon_states() {
    let model = model();
    let index = model
        .predicate_index(diehard::NOT_SOLVED)
        .expect("NotSolved is declared");
    let mut violations: Vec<Vec<i64>> = Vec::new();
    for vector in reachable(&model).keys() {
        let here = model.state(vector).expect("well typed");
        if model.evaluate_predicate(index, &here) == Ok(false) {
            violations.push(vector.clone());
        }
    }
    assert_eq!(violations, vec![vec![4, 0], vec![4, 3]]);
}

// ---------------------------------------------------------------------------
// chosen rows, spelled out
// ---------------------------------------------------------------------------

/// The successor row at `Init`. Three actions are self-loops there, which is the
/// "stuttering-producing actions" note of
/// `crates/continuum-kernel-core/src/fixture.rs:262-263` made concrete.
#[test]
fn the_row_at_the_initial_state_is_the_corpus_row() {
    let model = model();
    assert_eq!(
        labelled(&model, &state(&model, 0, 0)),
        vec![
            (diehard::BIG_TO_SMALL.to_owned(), vec![0, 0]),
            (diehard::EMPTY_BIG.to_owned(), vec![0, 0]),
            (diehard::EMPTY_SMALL.to_owned(), vec![0, 0]),
            (diehard::FILL_BIG.to_owned(), vec![5, 0]),
            (diehard::FILL_SMALL.to_owned(), vec![0, 3]),
            (diehard::SMALL_TO_BIG.to_owned(), vec![0, 0]),
        ]
    );
}

/// The row at `(5, 2)`, the state one step before the solution: `BigToSmall` tops the
/// small jug up from 2 to 3 and leaves 4 in the big one.
#[test]
fn the_row_before_the_solution_pours_four_gallons() {
    let model = model();
    assert_eq!(
        labelled(&model, &state(&model, 5, 2)),
        vec![
            (diehard::BIG_TO_SMALL.to_owned(), vec![4, 3]),
            (diehard::EMPTY_BIG.to_owned(), vec![0, 2]),
            (diehard::EMPTY_SMALL.to_owned(), vec![5, 0]),
            (diehard::FILL_BIG.to_owned(), vec![5, 2]),
            (diehard::FILL_SMALL.to_owned(), vec![5, 3]),
            (diehard::SMALL_TO_BIG.to_owned(), vec![5, 2]),
        ]
    );
}

/// A pour that empties the source rather than filling the destination: at `(2, 3)`,
/// `SmallToBig` moves all three gallons across.
#[test]
fn a_pour_that_empties_the_source_moves_everything() {
    let model = model();
    let row = labelled(&model, &state(&model, 2, 3));
    assert!(
        row.contains(&(diehard::SMALL_TO_BIG.to_owned(), vec![5, 0])),
        "{row:?}"
    );
    assert!(
        row.contains(&(diehard::BIG_TO_SMALL.to_owned(), vec![2, 3])),
        "{row:?}"
    );
}

/// Two walks of one model produce byte-identical output. INV-005 at the level the
/// exploration bone will inherit.
#[test]
fn two_walks_of_die_hard_are_byte_identical() {
    let render = || {
        let model = model();
        let mut out = String::new();
        for (vector, depth) in reachable(&model) {
            let here = model.state(&vector).expect("well typed");
            out.push_str(&format!("{depth} {:?}\n", labelled(&model, &here)));
        }
        out
    };
    let first = render();
    let second = render();
    assert_eq!(first.as_bytes(), second.as_bytes());
    assert!(!first.is_empty());
}
