//! Evidence that the Dining Philosophers model this harness declares is the corpus's.
//!
//! # Why this file exists
//!
//! START_HERE's PR 10 stem names two tasks — "Die Hard **and Dining Philosophers**" — and
//! plan §21's Phase A exit names the same two. Die Hard was already a `Model` in
//! `continuum-engine-reference`; Dining Philosophers was not, so `continuum_benchmark::
//! philosophers` declares it. A benchmark task whose model nobody checked would make every
//! number computed over it unfalsifiable, so this file checks it against the dossier before
//! any arm runs.
//!
//! # Clause → test
//!
//! - **"573 reachable states"** (`SPIKE_REPORT.md:18`, `TV-007/README.md`,
//!   `spike-results.json`) → [`the_frozen_state_transition_and_depth_counts_are_the_spikes`].
//! - **"2,365 enumerated transitions"** (`SPIKE_REPORT.md:19`) → same test.
//! - **"shortest deadlock depth 10"** (`SPIKE_REPORT.md:20`) → same test, and
//!   [`the_deadlock_is_the_state_the_spike_recorded`] for *which* state it is.
//! - **"the reference kernel can distinguish ordinary invariant closure from deadlock
//!   reachability"** (`SPIKE_REPORT.md:22`) →
//!   [`the_ownership_invariant_is_established_while_the_deadlock_refutes_the_campaign`].
//! - **the invariant is not vacuous** →
//!   [`the_ownership_invariant_refuses_a_state_that_violates_it`], which builds a state the
//!   predicate must reject rather than trusting that a predicate which never fails is
//!   checking anything.
//! - **the declaration is legal at more than one size** →
//!   [`the_model_builds_at_every_size_the_variable_budget_admits`], which is what makes the
//!   balanced conjunction in `fork_ownership` load-bearing rather than decorative.

use continuum_benchmark::philosophers::{
    self, EATING, FREE, FROZEN_DEADLOCK_DEPTH, FROZEN_STATES, FROZEN_TRANSITIONS, HAS_LEFT, N,
    THINKING, fork_var, phase_var,
};
use continuum_engine_reference::bfs::{self, Bounds};
use continuum_engine_reference::checking::{
    self, DeadlockOutcome, DeadlockPolicy, Obligations, Verdict,
};
use continuum_engine_reference::model::Model;

/// The three numbers four places in the dossier agree on.
#[test]
fn the_frozen_state_transition_and_depth_counts_are_the_spikes() {
    let model = philosophers::model().expect("the port builds");
    let exploration = bfs::explore(&model, Bounds::CERTIFIABLE).expect("the walk closes");
    let reachable = exploration.reachable();

    assert_eq!(
        reachable.states().len() as u64,
        FROZEN_STATES,
        "notes/plan/spikes/SPIKE_REPORT.md:18 — 573 reachable states"
    );
    assert_eq!(
        reachable.transitions(),
        FROZEN_TRANSITIONS,
        "notes/plan/spikes/SPIKE_REPORT.md:19 — 2,365 enumerated transitions"
    );
    assert_eq!(
        reachable.max_depth(),
        Some(FROZEN_DEADLOCK_DEPTH),
        "the deepest layer is the deadlock's own: every philosopher holding its left fork"
    );
}

/// Which state the deadlock is, not just that there is one.
///
/// `spike-results.json`'s `dining_philosophers_5.deadlock_state` records both halves —
/// `phase` all `HAS_LEFT`, `fork_owner` `[0, 1, 2, 3, 4]` — and a test that only counted
/// deadlocks would pass against a model that deadlocked somewhere else entirely.
#[test]
fn the_deadlock_is_the_state_the_spike_recorded() {
    let model = philosophers::model().expect("the port builds");
    let exploration = bfs::explore(&model, Bounds::CERTIFIABLE).expect("the walk closes");
    let report = checking::check(
        &model,
        &exploration,
        &Obligations::every_predicate(&model, DeadlockPolicy::Defect),
    )
    .expect("checking succeeds");

    let DeadlockOutcome::Deadlocked { states } = report.deadlock() else {
        panic!(
            "the left-then-right protocol deadlocks: {:?}",
            report.deadlock()
        );
    };
    assert_eq!(states.len(), 1, "one deadlock, up to the rotation symmetry");
    let deadlock = &states[0];
    assert_eq!(
        deadlock.depth(),
        FROZEN_DEADLOCK_DEPTH,
        "notes/plan/spikes/SPIKE_REPORT.md:20 — shortest deadlock depth 10"
    );
    for index in 0..N {
        assert_eq!(
            model.binding(deadlock.state(), &phase_var(index)),
            Some(HAS_LEFT),
            "spike-results.json: every philosopher is HAS_LEFT"
        );
        assert_eq!(
            model.binding(deadlock.state(), &fork_var(index)),
            Some(index as i64),
            "spike-results.json: fork f is held by philosopher f"
        );
    }
}

/// The distinction the corpus entry exists to show.
#[test]
fn the_ownership_invariant_is_established_while_the_deadlock_refutes_the_campaign() {
    let model = philosophers::model().expect("the port builds");
    let exploration = bfs::explore(&model, Bounds::CERTIFIABLE).expect("the walk closes");
    let report = checking::check(
        &model,
        &exploration,
        &Obligations::every_predicate(&model, DeadlockPolicy::Defect),
    )
    .expect("checking succeeds");

    let invariants = report.invariants();
    assert_eq!(invariants.len(), 1, "one declared predicate");
    assert_eq!(
        invariants[0].outcome().verdict(),
        Verdict::Established,
        "no reachable state gives a fork to a philosopher who did not take it"
    );
    assert_eq!(
        report.verdict(),
        Verdict::Refuted,
        "a refutation dominates the fold, and the deadlock is one"
    );
}

/// Anti-vacuity: the invariant must be able to say no.
#[test]
fn the_ownership_invariant_refuses_a_state_that_violates_it() {
    let model = philosophers::model().expect("the port builds");
    let index = model
        .predicate_index(philosophers::FORK_OWNERSHIP)
        .expect("the predicate is declared");

    // A legal state vector — every domain admits these values — in which philosopher 0 is
    // thinking while holding fork 0. The exploration never reaches it; the predicate must
    // still reject it, or "established over 573 states" would be a statement about the walk
    // rather than about the property.
    let violating = vector(&model, |name| {
        if name == fork_var(0) {
            0
        } else if name.starts_with("fork_") {
            FREE
        } else {
            THINKING
        }
    });
    let state = model.state(&violating).expect("every value is in domain");
    assert!(
        !model
            .evaluate_predicate(index, &state)
            .expect("the predicate is total"),
        "a thinking philosopher holding a fork violates ownership"
    );

    // …and the same predicate accepts the state where that philosopher has taken it.
    let legal = vector(&model, |name| {
        if name == fork_var(0) {
            0
        } else if name.starts_with("fork_") {
            FREE
        } else if name == phase_var(0) {
            HAS_LEFT
        } else {
            THINKING
        }
    });
    let state = model.state(&legal).expect("every value is in domain");
    assert!(
        model
            .evaluate_predicate(index, &state)
            .expect("the predicate is total"),
        "the same state with the philosopher in HAS_LEFT is legal"
    );
}

/// A state vector in the model's canonical variable order, valued by `choose`.
fn vector(model: &Model, choose: impl Fn(&str) -> i64) -> Vec<i64> {
    model
        .variables()
        .iter()
        .map(|variable| choose(variable.name().as_str()))
        .collect()
}

/// The balanced conjunction is what makes this true at `N > 5`.
///
/// `fork_ownership` unfolds `n²` clauses; folded linearly they would nest `n²` deep and the
/// builder's `MAX_EXPR_DEPTH` of 32 would refuse the declaration at `N = 6`. This is the test
/// that would fail if `conjoin` were replaced by a plain fold.
#[test]
fn the_model_builds_at_every_size_the_variable_budget_admits() {
    for n in 2..=16 {
        let model = philosophers::philosophers(n)
            .unwrap_or_else(|error| panic!("philosophers({n}) must build: {error}"));
        assert_eq!(model.variables().len(), 2 * n);
        assert_eq!(model.actions().len(), 4 * n);
        assert_eq!(model.initial_states().len(), 1);
    }
}

/// The corpus port's own vocabulary, transcribed exactly.
#[test]
fn the_phase_domain_is_the_corpus_enum() {
    assert_eq!(
        (THINKING, EATING),
        (0, 3),
        "Thinking .. Eating, four phases"
    );
    let model = philosophers::model().expect("the port builds");
    for index in 0..N {
        let phase = model
            .variable_index(&phase_var(index))
            .expect("every phase variable is declared");
        assert_eq!(
            model.variables()[phase].domain().lo(),
            THINKING,
            "phase domains start at Thinking"
        );
        assert_eq!(model.variables()[phase].domain().hi(), EATING);
        let fork = model
            .variable_index(&fork_var(index))
            .expect("every fork variable is declared");
        assert_eq!(
            model.variables()[fork].domain().lo(),
            FREE,
            "a fork's domain starts at `free`, which is -1 in the spike"
        );
    }
}
