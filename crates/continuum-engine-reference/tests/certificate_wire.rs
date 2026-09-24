//! The certificate as bytes: layout, purity, the closed-set seam, and the envelope
//! (PR 8, IMPL-05).
//!
//! # What this file proves, and what it deliberately does not
//!
//! Everything here is about the *bytes the engine writes*, checked without linking a
//! checker. `byte_layout_of_a_two_state_model_is_exactly_the_grammar` re-writes the
//! wire form by hand for a model small enough to spell out in full, so the layout is
//! asserted against an independent transcription of
//! `crates/continuum-kernel-core/src/wire.rs:43-74` rather than against the module that
//! produced it.
//!
//! Whether those bytes *check* — the fourth frozen TV-009 fact, "certificate accepted
//! by an independent checker" — is `tests/certificate_kernel_differential.rs`, which is
//! the only place in this crate that links `continuum-kernel-core`.

use continuum_engine_reference::bfs::{self, Bounds, Exploration};
use continuum_engine_reference::certificate::{
    self, ClaimEnvelope, ClosedSet, EmissionError, EnvelopeError, FINITE_CLOSURE_KIND, Field,
    MAGIC, MAX_DOMAIN_PACKS, PRODUCER, PROPERTY_CLASS_STATE_DOMAIN, WIRE_EPOCH,
};
use continuum_engine_reference::diehard;
use continuum_engine_reference::expr::{BoolExpr, IntExpr};
use continuum_engine_reference::ident::{IdentError, MAX_IDENT_BYTES};
use continuum_engine_reference::model::{ActionDecl, Model, ModelBuilder};

// ---------------------------------------------------------------------------
// fixtures
// ---------------------------------------------------------------------------

/// The envelope the differential also uses: placeholder digests, named as such.
///
/// Nothing here is computed by this crate and nothing here pretends to be. The strings
/// are the ones `crates/continuum-kernel-core/src/fixture.rs:129-135` uses for the same
/// model, so the two sides of the differential name the same claim.
fn envelope() -> ClaimEnvelope<'static> {
    ClaimEnvelope {
        model_digest: "blake3:diehard-model",
        semantic_epoch: "continuum-semantics-1",
        property_digest: "blake3:diehard-typeok",
        scope_digest: "blake3:diehard-scope",
        assumptions_digest: "blake3:empty-assumptions",
        producer: PRODUCER,
        domain_pack_digests: &[],
    }
}

fn diehard_model() -> Model {
    diehard::model().expect("the Die Hard transcription is a valid model")
}

/// A two-state model: `x in 0..=1`, one action that flips it.
///
/// Small enough that its whole certificate can be written out by hand below.
fn flip_model() -> Model {
    ModelBuilder::new()
        .variable("x", 0, 1)
        .initial_state(&[("x", 0)])
        .action(ActionDecl::deterministic(
            "Flip",
            BoolExpr::Const(true),
            vec![("x", IntExpr::minus(IntExpr::constant(1), IntExpr::var("x")))],
        ))
        .build()
        .expect("the flip model is a valid declaration")
}

fn complete(model: &Model) -> Exploration {
    bfs::explore(model, Bounds::CERTIFIABLE).expect("the model evaluates everywhere")
}

fn emit(model: &Model, exploration: &Exploration) -> Vec<u8> {
    let closed = ClosedSet::of(exploration).expect("the exploration completed");
    certificate::emit_finite_closure(model, closed, &envelope())
        .expect("a closed exploration of a declared model emits")
}

// ---------------------------------------------------------------------------
// an independent transcription of the wire grammar
// ---------------------------------------------------------------------------

/// A byte sink written for this test alone.
///
/// Deliberately not `certificate::Writer`, which is private: an expectation built with
/// the code under test would assert only that the module agrees with itself.
#[derive(Default)]
struct Expected {
    out: Vec<u8>,
}

impl Expected {
    fn bytes(&mut self, bytes: &[u8]) -> &mut Self {
        self.out.extend_from_slice(bytes);
        self
    }

    fn u16(&mut self, value: u16) -> &mut Self {
        self.bytes(&value.to_be_bytes())
    }

    fn u32(&mut self, value: u32) -> &mut Self {
        self.bytes(&value.to_be_bytes())
    }

    fn i64(&mut self, value: i64) -> &mut Self {
        self.bytes(&value.to_be_bytes())
    }

    fn token(&mut self, value: &str) -> &mut Self {
        let bytes = value.as_bytes();
        self.u16(u16::try_from(bytes.len()).expect("test tokens are short"));
        self.bytes(bytes)
    }
}

// ---------------------------------------------------------------------------
// layout
// ---------------------------------------------------------------------------

#[test]
fn byte_layout_of_a_two_state_model_is_exactly_the_grammar() {
    let model = flip_model();
    let exploration = complete(&model);
    let bytes = emit(&model, &exploration);

    let claim = envelope();
    // The model section, transcribed from `continuum-model/1`'s documented grammar
    // (`continuum-model-core/src/identity.rs`): counts and token lengths are u64.
    let mut model_bytes = Expected::default();
    let count = |e: &mut Expected, n: u64| {
        e.bytes(&n.to_be_bytes());
    };
    model_bytes.bytes(b"continuum-model/1");
    count(&mut model_bytes, 1); // one variable
    count(&mut model_bytes, 1);
    model_bytes.bytes(b"x").i64(0).i64(1);
    count(&mut model_bytes, 1); // one action
    count(&mut model_bytes, 4);
    model_bytes.bytes(b"Flip").bytes(&[0x10, 1]); // guard: true
    count(&mut model_bytes, 1); // one outcome
    count(&mut model_bytes, 1); // one assignment
    count(&mut model_bytes, 1);
    model_bytes.bytes(b"x").bytes(&[0x03, 1, 0x01]).i64(1); // 1 - x
    model_bytes.bytes(&[0x02]);
    count(&mut model_bytes, 1);
    model_bytes.bytes(b"x");
    count(&mut model_bytes, 1); // one initial state
    count(&mut model_bytes, 1);
    model_bytes.i64(0);
    count(&mut model_bytes, 0); // no predicates

    let mut want = Expected::default();
    // header := magic:8 wire_epoch:u16 kind:u16
    want.bytes(b"CONTCERT").u16(2).u16(1);
    // envelope := six tokens, schema_epoch:u16, domain_pack_count:u16 token*
    want.token(claim.model_digest)
        .token(claim.semantic_epoch)
        .token(claim.property_digest)
        .token(claim.scope_digest)
        .token(claim.assumptions_digest)
        .token(claim.producer)
        .u16(2)
        .u16(0);
    // model_len:u32 model
    want.u32(u32::try_from(model_bytes.out.len()).expect("small model"))
        .bytes(&model_bytes.out);
    // property := 1 (state-domain)
    want.u16(1);
    // table := state_count:u32 state*, ascending
    want.u32(2).i64(0).i64(1);
    // row * state_count; row := transition_count:u32 transition*,
    // transition := action:u16 target:u32 (a table index)
    want.u32(1).u16(0).u32(1);
    want.u32(1).u16(0).u32(0);

    assert_eq!(bytes, want.out);
}

#[test]
fn the_header_names_this_epoch_and_this_family() {
    let model = diehard_model();
    let bytes = emit(&model, &complete(&model));

    assert_eq!(bytes.get(..8), Some(MAGIC.as_slice()));
    assert_eq!(bytes.get(8..10), Some(WIRE_EPOCH.to_be_bytes().as_slice()));
    assert_eq!(
        bytes.get(10..12),
        Some(FINITE_CLOSURE_KIND.to_be_bytes().as_slice())
    );
    assert_eq!(
        WIRE_EPOCH, 2,
        "the model-bound wire epoch this build writes"
    );
    assert_eq!(FINITE_CLOSURE_KIND, 1, "CertificateKind::FiniteClosure");
    assert_eq!(PROPERTY_CLASS_STATE_DOMAIN, 1, "PropertyClass::StateDomain");
}

#[test]
fn the_producer_token_names_this_crate_and_its_version() {
    assert_eq!(PRODUCER, "continuum-engine-reference/0.0.0");
    // The one envelope field this crate can honestly state about itself is still a
    // token the decoder has to accept.
    assert!(PRODUCER.len() <= MAX_IDENT_BYTES);
    assert!(PRODUCER.bytes().all(|byte| byte.is_ascii_graphic()));
}

#[test]
fn the_die_hard_certificate_has_the_size_the_frozen_facts_imply() {
    let model = diehard_model();
    let bytes = emit(&model, &complete(&model));

    // 16 states of two i64s, and 96 transitions of a u16 action and a u32 target
    // index, plus one u32 count per row, plus the model's canonical encoding.
    // Everything else is header and envelope.
    let table = 16 * 2 * 8;
    let rows = 16 * 4 + 96 * (2 + 4);
    let model_len = model.identity().as_bytes().len();
    assert!(
        bytes.len() > table + rows + model_len,
        "the certificate carries the model, the table and every row"
    );
    assert!(bytes.len() < 4096, "and nothing like 64 MiB of anything");
}

// ---------------------------------------------------------------------------
// purity
// ---------------------------------------------------------------------------

#[test]
fn emission_is_a_pure_function_of_its_three_inputs() {
    let model = diehard_model();
    let exploration = complete(&model);
    let closed = ClosedSet::of(&exploration).expect("the exploration completed");

    let first = certificate::emit_finite_closure(&model, closed, &envelope())
        .expect("Die Hard emits a certificate");
    let second = certificate::emit_finite_closure(&model, closed, &envelope())
        .expect("Die Hard emits a certificate");
    assert_eq!(first, second, "same inputs, same bytes");
}

#[test]
fn two_independent_explorations_emit_the_same_certificate() {
    // Not the same `Exploration` value twice: two separate walks, so the equality is a
    // statement about the pipeline rather than about `Clone`.
    let model = diehard_model();
    let first = emit(&model, &complete(&model));
    let second = emit(&model, &complete(&model));
    assert_eq!(first, second);

    // And a second declaration of the same model, built independently.
    let rebuilt = diehard_model();
    let third = emit(&rebuilt, &complete(&rebuilt));
    assert_eq!(first, third, "the model is data; so is its certificate");
}

#[test]
fn a_bounded_but_complete_exploration_emits_the_same_bytes() {
    // Bounds are a budget, not a parameter of the answer: an exploration that completes
    // under the exact frozen counts is the same closed set as one under the certifiable
    // ceiling, so it is the same certificate.
    let model = diehard_model();
    let exact = bfs::explore(&model, Bounds::new(16, 7, 96)).expect("Die Hard completes");
    assert!(exact.is_complete());
    assert_eq!(emit(&model, &exact), emit(&model, &complete(&model)));
}

// ---------------------------------------------------------------------------
// the closed-set seam
// ---------------------------------------------------------------------------

#[test]
fn an_exhausted_exploration_cannot_reach_emission() {
    let model = diehard_model();
    // One state short of the frozen 16.
    let partial = bfs::explore(&model, Bounds::CERTIFIABLE.with_states(15))
        .expect("a bounded Die Hard walk is a result, not an error");
    assert!(!partial.is_complete());
    assert!(partial.exhausted().is_some());

    // `Exploration::closed` is `None`, and `ClosedSet` has no other constructor and a
    // private field — so there is no value of the type `emit_finite_closure` requires,
    // and no `EmissionError` arm for "not closed" to report either. The truncated set
    // is still reachable through `Exploration::reachable`, deliberately (bfs.rs:452-466);
    // it just cannot be certified.
    assert!(partial.closed().is_none());
    assert!(ClosedSet::of(&partial).is_none());
    assert_eq!(partial.reachable().len(), 15);
}

#[test]
fn every_bound_that_trips_closes_the_same_door() {
    let model = diehard_model();
    for bounds in [
        Bounds::CERTIFIABLE.with_states(15),
        Bounds::CERTIFIABLE.with_depth(6),
        Bounds::CERTIFIABLE.with_transitions(95),
    ] {
        let exploration = bfs::explore(&model, bounds).expect("a bounded walk is a result");
        assert!(ClosedSet::of(&exploration).is_none(), "{bounds:?}");
    }
}

#[test]
fn a_complete_exploration_opens_it() {
    let model = diehard_model();
    let exploration = complete(&model);
    let closed = ClosedSet::of(&exploration).expect("the exploration completed");
    assert_eq!(closed.reachable().len(), 16);
    assert_eq!(closed.reachable().transitions(), 96);
    assert_eq!(closed.reachable().expanded(), 16);
}

// ---------------------------------------------------------------------------
// the envelope is checked before anything is written
// ---------------------------------------------------------------------------

fn emission_error(claim: &ClaimEnvelope<'_>) -> EmissionError {
    let model = diehard_model();
    let exploration = complete(&model);
    let closed = ClosedSet::of(&exploration).expect("the exploration completed");
    certificate::emit_finite_closure(&model, closed, claim)
        .expect_err("the envelope is out of spec")
}

fn envelope_error(claim: &ClaimEnvelope<'_>) -> EnvelopeError {
    match emission_error(claim) {
        EmissionError::Envelope(error) => error,
        other => panic!("expected an envelope error, got {other:?}"),
    }
}

#[test]
fn an_empty_envelope_field_is_a_typed_error_naming_the_field() {
    for (field, claim) in [
        (
            Field::ModelDigest,
            ClaimEnvelope {
                model_digest: "",
                ..envelope()
            },
        ),
        (
            Field::SemanticEpoch,
            ClaimEnvelope {
                semantic_epoch: "",
                ..envelope()
            },
        ),
        (
            Field::PropertyDigest,
            ClaimEnvelope {
                property_digest: "",
                ..envelope()
            },
        ),
        (
            Field::ScopeDigest,
            ClaimEnvelope {
                scope_digest: "",
                ..envelope()
            },
        ),
        (
            Field::AssumptionsDigest,
            ClaimEnvelope {
                assumptions_digest: "",
                ..envelope()
            },
        ),
        (
            Field::Producer,
            ClaimEnvelope {
                producer: "",
                ..envelope()
            },
        ),
    ] {
        assert_eq!(
            envelope_error(&claim),
            EnvelopeError::MalformedToken {
                field,
                source: IdentError::Empty,
            },
            "{field}"
        );
        assert!(claim.validate().is_err());
    }
}

#[test]
fn a_non_printable_envelope_field_is_refused_at_the_offending_byte() {
    let claim = ClaimEnvelope {
        model_digest: "blake3:die hard",
        ..envelope()
    };
    assert_eq!(
        envelope_error(&claim),
        EnvelopeError::MalformedToken {
            field: Field::ModelDigest,
            source: IdentError::NotPrintableAscii {
                index: 10,
                byte: b' ',
            },
        }
    );
}

#[test]
fn the_token_length_boundary_is_the_wire_forms_own() {
    let longest = "d".repeat(MAX_IDENT_BYTES);
    let over = "d".repeat(MAX_IDENT_BYTES + 1);

    let admitted = ClaimEnvelope {
        model_digest: &longest,
        ..envelope()
    };
    assert!(
        admitted.validate().is_ok(),
        "{MAX_IDENT_BYTES} bytes is a token"
    );

    let refused = ClaimEnvelope {
        model_digest: &over,
        ..envelope()
    };
    assert_eq!(
        envelope_error(&refused),
        EnvelopeError::MalformedToken {
            field: Field::ModelDigest,
            source: IdentError::TooLong {
                bytes: MAX_IDENT_BYTES + 1,
                max: MAX_IDENT_BYTES,
            },
        }
    );
}

#[test]
fn the_domain_pack_count_boundary_is_the_wire_forms_own() {
    let names: Vec<String> = (0..MAX_DOMAIN_PACKS + 1)
        .map(|index| format!("pack:{index:03}"))
        .collect();
    let refs: Vec<&str> = names.iter().map(String::as_str).collect();

    let full = &refs[..usize::from(MAX_DOMAIN_PACKS)];
    let admitted = ClaimEnvelope {
        domain_pack_digests: full,
        ..envelope()
    };
    assert!(
        admitted.validate().is_ok(),
        "{MAX_DOMAIN_PACKS} ascending digests are admissible"
    );
    let model = diehard_model();
    let exploration = complete(&model);
    let closed = ClosedSet::of(&exploration).expect("the exploration completed");
    assert!(certificate::emit_finite_closure(&model, closed, &admitted).is_ok());

    let refused = ClaimEnvelope {
        domain_pack_digests: &refs,
        ..envelope()
    };
    assert_eq!(
        envelope_error(&refused),
        EnvelopeError::TooManyDomainPacks {
            found: usize::from(MAX_DOMAIN_PACKS) + 1,
            max: MAX_DOMAIN_PACKS,
        }
    );
}

#[test]
fn domain_pack_digests_must_be_strictly_ascending() {
    // Descending.
    let descending = ClaimEnvelope {
        domain_pack_digests: &["pack:b", "pack:a"],
        ..envelope()
    };
    assert_eq!(
        envelope_error(&descending),
        EnvelopeError::DomainPacksNotAscending { index: 1 }
    );

    // Duplicated: the same rejection, because strictness is what forbids a duplicate
    // (kernel-core wire.rs:88-95).
    let duplicated = ClaimEnvelope {
        domain_pack_digests: &["pack:a", "pack:a"],
        ..envelope()
    };
    assert_eq!(
        envelope_error(&duplicated),
        EnvelopeError::DomainPacksNotAscending { index: 1 }
    );

    // Malformed, at its index.
    let malformed = ClaimEnvelope {
        domain_pack_digests: &["pack:a", ""],
        ..envelope()
    };
    assert_eq!(
        envelope_error(&malformed),
        EnvelopeError::MalformedDomainPack {
            index: 1,
            source: IdentError::Empty,
        }
    );
}

#[test]
fn a_valid_envelope_validates_and_emits() {
    let claim = envelope();
    assert!(claim.validate().is_ok());
    let model = diehard_model();
    assert!(!emit(&model, &complete(&model)).is_empty());
}

// ---------------------------------------------------------------------------
// the model and the exploration have to be the same model
// ---------------------------------------------------------------------------

#[test]
fn a_reachable_set_from_another_model_is_an_emission_error_not_a_certificate() {
    let explored = diehard_model();
    let exploration = complete(&explored);
    let closed = ClosedSet::of(&exploration).expect("the exploration completed");

    let other = flip_model();
    let error = certificate::emit_finite_closure(&other, closed, &envelope())
        .expect_err("Die Hard's states are not states of the flip model");
    match error {
        EmissionError::Evaluation { state, .. } => {
            // The first state of the Die Hard table, offered to a one-variable model.
            assert_eq!(state.as_slice(), [0, 0]);
        }
        other => panic!("expected an evaluation error, got {other:?}"),
    }
}

#[test]
fn the_error_types_render_and_carry_their_source() {
    use core::error::Error as _;

    let claim = ClaimEnvelope {
        producer: "",
        ..envelope()
    };
    let error = emission_error(&claim);
    let rendered = error.to_string();
    assert!(rendered.contains("producer"), "{rendered}");
    assert!(error.source().is_some());

    let envelope_error = envelope_error(&claim);
    assert!(envelope_error.to_string().contains("producer"));
    assert!(envelope_error.source().is_some());

    assert_eq!(Field::TransitionTotal.to_string(), "transition-total");
    assert_eq!(Field::StateCount.as_str(), "state-count");
}
