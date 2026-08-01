//! Die Hard's frozen depth-6 witness, extracted through the engine (PR 8, IMPL-04).
//!
//! # The frozen fact this file owns
//!
//! | Fact | Sources |
//! |---|---|
//! | shortest `big == 4` witness at depth 6 | `notes/plan/spikes/SPIKE_REPORT.md:9-12`; `notes/plan/docs/27_SPIKE_FINDINGS_REV2.md:24-27`; `notes/plan/corpus/tla-examples/ports/TV-009/port.json:31-35`; `crates/continuum-kernel-core/src/fixture.rs:11-14`; `lean/Continuum/Examples/DieHard.lean:14-26` |
//!
//! `tests/diehard_evidence.rs` and `tests/bfs_diehard.rs` already assert the *number*
//! six, from the model layer and from the engine's depth map respectively. This file
//! asserts the *path*, which is what PR 8's exit sentence — "Die Hard returns 16 states
//! and depth-6 solution" — actually promises.
//!
//! # The Lean path, and why it can be pinned
//!
//! `lean/Continuum/Examples/DieHard.lean:14-22` fixes one six-step solution, and
//! `tests/diehard_evidence.rs::the_six_step_solution_replays_as_real_transitions`
//! replays it against the model. Nothing required the engine's canonical witness to be
//! *that* path: several shortest paths could exist, and the canonical one is whichever
//! the breadth-first admission order records. It turns out to be exactly the Lean path,
//! so this file pins it — the equality is an observed fact about this model under this
//! admission order, asserted so that a change to either is caught, and not a property
//! the witness extractor guarantees in general. What it does guarantee for every model
//! is asserted separately below: the length equals the depth map, and every step
//! replays through `Model::successors`.

use continuum_engine_reference::bfs::{self, Bound, Bounds, Exploration};
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

/// `(action name, target vector)` per step, which is the shape
/// `tests/diehard_evidence.rs` writes the Lean path in.
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

/// The Lean solution as `tests/diehard_evidence.rs` writes it
/// (`lean/Continuum/Examples/DieHard.lean:14-22`).
fn lean_path() -> Vec<(String, Vec<i64>)> {
    let states = [[5, 0], [2, 3], [2, 0], [0, 2], [5, 2], [4, 3]];
    let actions = [
        diehard::FILL_BIG,
        diehard::BIG_TO_SMALL,
        diehard::EMPTY_SMALL,
        diehard::BIG_TO_SMALL,
        diehard::FILL_BIG,
        diehard::BIG_TO_SMALL,
    ];
    actions
        .iter()
        .zip(states.iter())
        .map(|(action, target)| ((*action).to_owned(), target.to_vec()))
        .collect()
}

// ---------------------------------------------------------------------------
// the depth-6 solution
// ---------------------------------------------------------------------------

/// Asking for the shortest violation of `NotSolved` returns the film's answer: six
/// steps, ending with four gallons in the big jug.
#[test]
fn the_shortest_violation_of_not_solved_is_the_six_step_solution() {
    let model = model();
    let exploration = complete(&model);
    let target = Target::Fails(predicate(&model, diehard::NOT_SOLVED));
    let found = witness::shortest(&model, &exploration, &target).expect("NotSolved is violable");
    assert_eq!(found.len(), 6);
    assert_eq!(found.start(), &state(&model, 0, 0));
    assert_eq!(found.target(), &state(&model, 4, 3));
    assert_eq!(
        found.target().as_slice().first(),
        Some(&diehard::TARGET_GALLONS)
    );
    assert_eq!(
        labelled(&found),
        vec![
            (diehard::FILL_BIG.to_owned(), vec![5, 0]),
            (diehard::BIG_TO_SMALL.to_owned(), vec![2, 3]),
            (diehard::EMPTY_SMALL.to_owned(), vec![2, 0]),
            (diehard::BIG_TO_SMALL.to_owned(), vec![0, 2]),
            (diehard::FILL_BIG.to_owned(), vec![5, 2]),
            (diehard::BIG_TO_SMALL.to_owned(), vec![4, 3]),
        ]
    );
}

/// The canonical witness *is* the Lean path — same length and, as it happens, the same
/// six transitions. Pinned, with the caveat the module docs state.
#[test]
fn the_canonical_witness_is_the_lean_path() {
    let model = model();
    let exploration = complete(&model);
    let target = Target::Fails(predicate(&model, diehard::NOT_SOLVED));
    let found = witness::shortest(&model, &exploration, &target).expect("NotSolved is violable");
    let lean = lean_path();
    assert_eq!(found.len(), lean.len(), "six steps either way");
    assert_eq!(labelled(&found), lean);
    // Seven states for six steps, starting at Init.
    let states: Vec<Vec<i64>> = found
        .states()
        .iter()
        .map(|s| s.as_slice().to_vec())
        .collect();
    assert_eq!(
        states,
        vec![
            vec![0, 0],
            vec![5, 0],
            vec![2, 3],
            vec![2, 0],
            vec![0, 2],
            vec![5, 2],
            vec![4, 3],
        ]
    );
    assert_eq!(states.len(), found.len() + 1);
}

/// The differential: every consecutive pair of the witness is a labelled transition
/// the *model layer* produces, replayed one step at a time through
/// `Model::successors`. The witness is not trusted because the engine emitted it.
#[test]
fn every_step_of_the_witness_replays_through_the_model() {
    let model = model();
    let exploration = complete(&model);
    let target = Target::Fails(predicate(&model, diehard::NOT_SOLVED));
    let found = witness::shortest(&model, &exploration, &target).expect("NotSolved is violable");

    assert!(
        model.initial_states().contains(found.start()),
        "a witness starts at a declared initial state"
    );
    let mut here = found.start().clone();
    for (index, step) in found.steps().iter().enumerate() {
        let row = model.successors(&here).expect("Die Hard evaluates");
        assert!(
            row.iter()
                .any(|actual| actual.action() == step.action() && actual.target() == step.target()),
            "step {index}: {here} --{}--> {} is not in the model's row",
            step.name(),
            step.target()
        );
        // The index and the name agree, so a reader and a decoder see the same step.
        assert_eq!(
            model
                .actions()
                .get(step.action())
                .map(|action| action.name()),
            Some(step.name())
        );
        here = step.target().clone();
    }
    assert_eq!(&here, found.target());
}

/// The path length is the depth map's depth. Asserted for the solution and for every
/// other reachable state, targeted one at a time.
#[test]
fn the_witness_length_is_always_the_recorded_depth() {
    let model = model();
    let exploration = complete(&model);
    let reachable = exploration.reachable();
    for (wanted, depth) in reachable.states().iter().zip(reachable.depths().iter()) {
        let target = Target::State(wanted.clone());
        let found = witness::shortest(&model, &exploration, &target)
            .expect("a discovered state is a target");
        assert_eq!(found.len(), *depth, "{wanted}");
        assert_eq!(found.target(), wanted);
        assert_eq!(found.start(), &state(&model, 0, 0));
    }
    assert_eq!(reachable.len(), 16, "sixteen witnesses, one per state");
}

/// The other four-gallon state is reachable too, one layer further out, and asking for
/// it by name gives a seven-step witness. The predicate target found the shallower one
/// because that is the rule, not because the deeper one is unreachable.
#[test]
fn the_other_four_gallon_state_is_one_step_further() {
    let model = model();
    let exploration = complete(&model);
    let target = Target::State(state(&model, 4, 0));
    let found = witness::shortest(&model, &exploration, &target).expect("(4, 0) is reachable");
    assert_eq!(found.len(), 7);
    assert_eq!(found.target(), &state(&model, 4, 0));
    assert_eq!(exploration.reachable().depth_of(found.target()), Some(7));
}

/// Targeting the state and targeting the violated predicate give the same witness:
/// `NotSolved` fails at exactly `(4, 0)` and `(4, 3)`, and `(4, 3)` is the shallower.
#[test]
fn targeting_the_state_and_the_violated_predicate_agree() {
    let model = model();
    let exploration = complete(&model);
    let by_state = witness::shortest(&model, &exploration, &Target::State(state(&model, 4, 3)))
        .expect("(4, 3) is reachable");
    let by_predicate = witness::shortest(
        &model,
        &exploration,
        &Target::Fails(predicate(&model, diehard::NOT_SOLVED)),
    )
    .expect("NotSolved is violable");
    assert_eq!(by_state, by_predicate);
}

// ---------------------------------------------------------------------------
// the predicates, and the absent witness
// ---------------------------------------------------------------------------

/// `TypeOK` holds at the initial state, so its shortest witness has no steps at all.
/// A zero-length witness is a witness.
#[test]
fn a_predicate_that_holds_at_init_witnesses_in_no_steps() {
    let model = model();
    let exploration = complete(&model);
    let target = Target::Holds(predicate(&model, diehard::TYPE_OK));
    let found = witness::shortest(&model, &exploration, &target).expect("TypeOK holds everywhere");
    assert_eq!(found.len(), 0);
    assert!(found.is_empty());
    assert_eq!(found.start(), &state(&model, 0, 0));
    assert_eq!(found.target(), found.start());
    assert_eq!(found.states().len(), 1);
    assert_eq!(found.to_string(), "(0, 0)");
}

/// `TypeOK` never fails over the reachable set, so there is no witness to its failure —
/// and over a *complete* exploration that is a real negative result.
#[test]
fn type_ok_has_no_counterexample() {
    let model = model();
    let exploration = complete(&model);
    let target = Target::Fails(predicate(&model, diehard::TYPE_OK));
    assert_eq!(
        witness::shortest(&model, &exploration, &target),
        Err(NoWitness::Unreached { searched: 16 })
    );
}

/// A state inside the declared domain but outside the reachable set has no witness.
#[test]
fn an_unreachable_state_has_no_witness() {
    let model = model();
    let exploration = complete(&model);
    // `(1, 1)` is well typed and never reached: the reachable set is the frozen 16.
    let unreachable = state(&model, 1, 1);
    assert!(!exploration.reachable().contains(&unreachable));
    assert_eq!(
        witness::shortest(&model, &exploration, &Target::State(unreachable)),
        Err(NoWitness::Unreached { searched: 16 })
    );
}

// ---------------------------------------------------------------------------
// truncation
// ---------------------------------------------------------------------------

/// A bounded exploration is refused, naming the bound. The shallow case: the solution
/// was never discovered.
#[test]
fn a_shallow_exploration_refuses_to_witness() {
    let model = model();
    let exploration =
        bfs::explore(&model, Bounds::CERTIFIABLE.with_depth(2)).expect("Die Hard evaluates");
    let target = Target::Fails(predicate(&model, diehard::NOT_SOLVED));
    assert_eq!(
        witness::shortest(&model, &exploration, &target),
        Err(NoWitness::Truncated {
            tripped: Bound::Depth,
            explored: 6,
        })
    );
}

/// And the case that matters: at depth bound 6 the solution *is* discovered, at its
/// true depth of 6, and the witness is refused anyway.
///
/// This is the [`Exploration::closed`] seam, mirrored. The exploration cannot rule out
/// a shallower violation it never looked for, so it cannot support the word *shortest*
/// — and a path that is not known to be shortest is not this module's answer.
#[test]
fn an_exploration_that_found_the_solution_still_refuses_while_truncated() {
    let model = model();
    let exploration =
        bfs::explore(&model, Bounds::CERTIFIABLE.with_depth(6)).expect("Die Hard evaluates");
    let partial = exploration.exhausted().expect("the depth bound tripped");
    assert_eq!(
        partial.explored().depth_of(&state(&model, 4, 3)),
        Some(6),
        "the endpoint was discovered, at its true depth"
    );
    let target = Target::Fails(predicate(&model, diehard::NOT_SOLVED));
    let refused = witness::shortest(&model, &exploration, &target);
    assert_eq!(
        refused,
        Err(NoWitness::Truncated {
            tripped: Bound::Depth,
            explored: partial.explored().len(),
        })
    );
    // Raise the bound past what the model needs and the same query answers.
    let complete =
        bfs::explore(&model, Bounds::CERTIFIABLE.with_depth(7)).expect("Die Hard evaluates");
    assert!(complete.is_complete());
    assert_eq!(
        witness::shortest(&model, &complete, &target)
            .expect("a closed exploration answers")
            .len(),
        6
    );
}

/// Each bound refuses in its own name, so a caller learns which budget to raise.
#[test]
fn each_bound_refuses_in_its_own_name() {
    let model = model();
    let target = Target::Fails(predicate(&model, diehard::NOT_SOLVED));
    for (bounds, expected) in [
        (Bounds::CERTIFIABLE.with_states(3), Bound::States),
        (Bounds::CERTIFIABLE.with_depth(2), Bound::Depth),
        (Bounds::CERTIFIABLE.with_transitions(10), Bound::Transitions),
    ] {
        let exploration = bfs::explore(&model, bounds).expect("Die Hard evaluates");
        let outcome = witness::shortest(&model, &exploration, &target);
        assert!(
            matches!(outcome, Err(NoWitness::Truncated { tripped, .. }) if tripped == expected),
            "{bounds:?}: {outcome:?}"
        );
    }
}

// ---------------------------------------------------------------------------
// determinism and the entry point
// ---------------------------------------------------------------------------

/// Two extractions of one witness are equal, and render byte-identically.
#[test]
fn two_extractions_of_the_solution_are_byte_identical() {
    let extract = || {
        let model = model();
        let exploration = complete(&model);
        let target = Target::Fails(predicate(&model, diehard::NOT_SOLVED));
        witness::shortest(&model, &exploration, &target).expect("NotSolved is violable")
    };
    let first = extract();
    let second = extract();
    assert_eq!(first, second);
    assert_eq!(
        format!("{first:?}").as_bytes(),
        format!("{second:?}").as_bytes()
    );
    assert_eq!(first.to_string().as_bytes(), second.to_string().as_bytes());
}

/// The rendered witness names every state and every action, in order.
#[test]
fn the_rendered_witness_is_the_readable_solution() {
    let model = model();
    let exploration = complete(&model);
    let target = Target::Fails(predicate(&model, diehard::NOT_SOLVED));
    let found = witness::shortest(&model, &exploration, &target).expect("NotSolved is violable");
    assert_eq!(
        found.to_string(),
        "(0, 0) --FillBig--> (5, 0) --BigToSmall--> (2, 3) --EmptySmall--> (2, 0) \
         --BigToSmall--> (0, 2) --FillBig--> (5, 2) --BigToSmall--> (4, 3)"
    );
}

/// The crate root re-exports the entry point, so a caller need not name the module.
#[test]
fn the_crate_root_re_exports_the_entry_point() {
    let model = model();
    let exploration = complete(&model);
    let target = continuum_engine_reference::Target::Fails(predicate(&model, diehard::NOT_SOLVED));
    let found: continuum_engine_reference::Witness =
        continuum_engine_reference::shortest(&model, &exploration, &target)
            .expect("NotSolved is violable");
    assert_eq!(found.len(), 6);
}
