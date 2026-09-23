//! The Die Hard differential: the CML front end against the programmatic front end
//! (PR 15a, bn-ybq).
//!
//! # The pair
//!
//! - **oracle**: `continuum_engine_reference::diehard::model()`, the programmatic Die
//!   Hard model PR 8 transcribed by hand from the corpus file;
//! - **subject**: `notes/plan/corpus/tla-examples/ports/TV-009/DieHard.ctm`, parsed by
//!   `continuum-cml-syntax`, elaborated and lowered by `continuum_cml_elab`.
//!
//! Both are the one `continuum_model_core::Model` type, so the comparison is not a
//! translation between two representations. The test asserts, in increasing strength:
//!
//! 1. the frozen TV-009 facts (16 states, 96 transitions, a depth-6 `big == 4` witness,
//!    `NotSolved` refuted and `TypeOK` established) hold for the elaborated model when
//!    the reference engine checks it;
//! 2. the reference engine's exploration, check report, and witness are equal for the
//!    two models, and so are the finite-closure certificate bytes;
//! 3. the two models have one semantic identity (`Model::identity`).
//!
//! Point 3 implies the others; points 1 and 2 are kept so that a defect in the identity
//! encoding cannot hide a behavioural difference.

use continuum_cml_elab::{elaborate_source, lower};
use continuum_engine_reference::bfs::{self, Bounds, Exploration};
use continuum_engine_reference::certificate::{self, ClaimEnvelope, ClosedSet, PRODUCER};
use continuum_engine_reference::checking::{self, CheckOutcome, DeadlockPolicy, Obligations};
use continuum_engine_reference::diehard;
use continuum_engine_reference::model::Model;
use continuum_engine_reference::witness::{self, Target};

fn corpus_source() -> String {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../notes/plan/corpus/tla-examples/ports/TV-009/DieHard.ctm"
    );
    std::fs::read_to_string(path).expect("the corpus file is present")
}

/// The subject: the corpus file through the CML front end.
fn elaborated() -> Model {
    let norm = elaborate_source(&corpus_source()).expect("DieHard.ctm elaborates");
    lower(&norm).expect("DieHard.ctm lowers")
}

/// The oracle: the programmatic model.
fn programmatic() -> Model {
    diehard::model().expect("the programmatic Die Hard model is valid")
}

fn explore(model: &Model) -> Exploration {
    bfs::explore(model, Bounds::CERTIFIABLE).expect("Die Hard evaluates everywhere")
}

fn report(model: &Model) -> checking::CheckReport {
    let obligations = Obligations::every_predicate(model, DeadlockPolicy::Defect);
    checking::check(model, &explore(model), &obligations).expect("both predicates are declared")
}

fn certificate(model: &Model) -> Vec<u8> {
    let exploration = explore(model);
    let closed = ClosedSet::of(&exploration).expect("the exploration closes");
    let envelope = ClaimEnvelope {
        model_digest: "blake3:diehard-model",
        semantic_epoch: "continuum-semantics-1",
        property_digest: "blake3:diehard-typeok",
        scope_digest: "blake3:diehard-scope",
        assumptions_digest: "blake3:empty-assumptions",
        producer: PRODUCER,
        domain_pack_digests: &[],
    };
    certificate::emit_finite_closure(model, closed, &envelope).expect("a closed set emits")
}

/// Point 1: the frozen facts, from the elaborated model alone.
#[test]
fn the_elaborated_model_reproduces_the_frozen_tv009_facts() {
    let model = elaborated();
    let exploration = explore(&model);
    let closed = exploration.closed().expect("the exploration closes");
    assert_eq!(closed.len(), 16, "reachable states");
    assert_eq!(closed.transitions(), 96, "labelled transitions");

    let report = report(&model);
    let not_solved = model.predicate_index("NotSolved").expect("declared");
    let type_ok = model.predicate_index("TypeOK").expect("declared");
    let outcome = |index: usize| report.invariant(index).expect("upheld").outcome().clone();
    assert_eq!(outcome(type_ok), CheckOutcome::Holds { states: 16 });
    let CheckOutcome::Violated { depth, .. } = outcome(not_solved) else {
        panic!("NotSolved is intentionally false (DieHard.ctm:31)");
    };
    assert_eq!(depth, 6, "shortest big == 4 witness depth");
}

/// Point 2: the reference engine cannot tell the two models apart.
#[test]
fn the_reference_engine_gives_identical_answers_for_both_front_ends() {
    let subject = elaborated();
    let oracle = programmatic();

    assert_eq!(explore(&subject), explore(&oracle), "exploration");
    assert_eq!(report(&subject), report(&oracle), "check report");
    assert_eq!(
        report(&subject).verdict(),
        report(&oracle).verdict(),
        "verdict"
    );

    let not_solved = oracle.predicate_index("NotSolved").expect("declared");
    let witness_of = |m: &Model| {
        witness::shortest(m, &explore(m), &Target::Fails(not_solved)).expect("a witness exists")
    };
    assert_eq!(
        witness_of(&subject),
        witness_of(&oracle),
        "shortest witness"
    );

    assert_eq!(
        certificate(&subject),
        certificate(&oracle),
        "certificate bytes"
    );
}

/// Point 3: one model, one identity.
#[test]
fn the_elaborated_and_programmatic_models_have_one_identity() {
    let subject = elaborated();
    let oracle = programmatic();
    assert_eq!(subject, oracle, "the two front ends built equal models");
    assert_eq!(subject.identity(), oracle.identity());
}

/// The differential is not vacuous: a one-token change to the corpus source (the
/// mutation `port.json` names as M1, "SmallToBig subtracts the wrong transferred
/// amount") changes the identity, the transition relation, and the engine's answer.
#[test]
fn a_mutated_corpus_source_is_told_apart() {
    let mutated = corpus_source().replace(
        "small' == small - (next_big - big)",
        "small' == small - (next_big - small)",
    );
    assert_ne!(mutated, corpus_source(), "the mutation applies");
    let norm = elaborate_source(&mutated).expect("the mutant still elaborates");
    let subject = lower(&norm).expect("the mutant still lowers");
    let oracle = programmatic();
    assert_ne!(subject.identity(), oracle.identity());
    // The mutant either explores differently or leaves the declared domain; both are
    // a detected difference.
    if let Ok(exploration) = bfs::explore(&subject, Bounds::CERTIFIABLE) {
        assert_ne!(exploration, explore(&oracle));
    }
}
