//! The fourth frozen Die Hard fact: a certificate this engine emitted, accepted by an
//! independent checker (PR 8, IMPL-05).
//!
//! # What is being closed here
//!
//! > **16** reachable states, **96** labelled transitions, a shortest `big == 4`
//! > witness at depth **6**, and an independently accepted certificate. The first three
//! > are asserted against this model by `tests/diehard_evidence.rs`; the fourth is the
//! > certificate bone's.
//! >
//! > — `crates/continuum-engine-reference/src/diehard.rs:41-50`
//!
//! `tests/bfs_diehard.rs` says the same thing and leaves the same gap: "The fourth
//! frozen fact […] needs certificate emission, which is PR 8's fifth bone, and is not
//! asserted here either." This file asserts it. The engine explores Die Hard, emits the
//! certificate, and hands the **bytes** to `continuum-kernel-core::check_certificate`.
//!
//! # Why this is a differential and not a round trip
//!
//! The two sides share no code. `continuum-kernel-core` depends on no workspace crate
//! and no external crate; this crate depends on the kernel only here, in dev position,
//! which `tools/check_crate_boundaries.py:37-39` admits for exactly this purpose
//! ("dev-dependencies are reported but not enforced, because a differential test may
//! legitimately link an engine it would never ship against"). The certificate's
//! encoder lives in `src/certificate.rs` and was written from the documented grammar;
//! the decoder lives in `kernel-core/src/wire.rs` and was written from the same
//! document. Neither has seen the other's types. That is docs/03 §8's "certificate
//! decoding" independence axis, and it is why an agreement between them is evidence.
//!
//! # The negative half
//!
//! A checker that accepted everything would pass the test above. So every byte of the
//! canonical state table is mutated, twice, and none of the 512 resulting artifacts may
//! verify; the header, the trailing bytes and every prefix are attacked separately with
//! the exact rejection each must produce. The pair — one verified certificate and 500-odd
//! rejected mutations of it — is the claim.

use continuum_engine_reference::bfs::{self, Bounds, Exploration};
use continuum_engine_reference::certificate::{self, ClaimEnvelope, ClosedSet, PRODUCER};
use continuum_engine_reference::diehard;
use continuum_engine_reference::model::Model;

use continuum_kernel_core::check_certificate;
use continuum_kernel_core::verdict::{CertificateKind, Feature, PropertyClass, Rejection, Verdict};
use continuum_kernel_core::wire::{self, Body};

/// The claim envelope: placeholder digests, and the seam is the point.
///
/// Not one of these six strings is computed by the engine. `model_digest`,
/// `property_digest`, `scope_digest` and `assumptions_digest` are digests of artifacts
/// this crate never sees; `semantic_epoch` is the CIR semantics version in force. The
/// kernel checks their *shape* and reports the rest as trusted — `CheckedClaim::
/// trusted_components` names `certificate-model-correspondence` and
/// `envelope-digest-binding` — so this test asserts the envelope survives the round
/// trip, never that it is true.
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

fn model() -> Model {
    diehard::model().expect("the Die Hard transcription is a valid model")
}

fn exploration(model: &Model) -> Exploration {
    bfs::explore(model, Bounds::CERTIFIABLE).expect("Die Hard evaluates everywhere")
}

/// The certificate under test: explored, then emitted, then handed over as bytes.
fn certificate(model: &Model) -> Vec<u8> {
    let exploration = exploration(model);
    let closed = ClosedSet::of(&exploration).expect("Die Hard's exploration completes");
    certificate::emit_finite_closure(model, closed, &envelope())
        .expect("a closed exploration of a declared model emits")
}

// ---------------------------------------------------------------------------
// the fourth frozen fact
// ---------------------------------------------------------------------------

#[test]
fn the_engines_die_hard_certificate_is_verified_by_the_independent_checker() {
    let bytes = certificate(&model());

    let Verdict::Verified(claim) = check_certificate(&bytes) else {
        panic!("the kernel refused a certificate the engine emitted from a closed walk");
    };

    // The frozen TV-009 facts, re-derived by the checker from the bytes alone — the
    // same three numbers `crates/continuum-kernel-core/src/check.rs:253-257` pins for
    // its own hand-built fixture.
    assert_eq!(claim.states(), 16);
    assert_eq!(claim.transitions(), 96);
    assert_eq!(claim.initial_states(), 1);
    assert_eq!(claim.kind(), CertificateKind::FiniteClosure);
    assert_eq!(claim.property(), PropertyClass::StateDomain);

    // The envelope the engine was given, as the checker read it back.
    let sent = envelope();
    let received = claim.envelope();
    assert_eq!(received.model_digest().as_str(), sent.model_digest);
    assert_eq!(received.semantic_epoch().as_str(), sent.semantic_epoch);
    assert_eq!(received.property_digest().as_str(), sent.property_digest);
    assert_eq!(received.scope_digest().as_str(), sent.scope_digest);
    assert_eq!(
        received.assumptions_digest().as_str(),
        sent.assumptions_digest
    );
    assert_eq!(received.producer().as_str(), sent.producer);
    assert_eq!(received.schema_epoch(), 1);
    assert_eq!(received.domain_pack_digests().len(), 0);

    // And what a verified verdict still trusts, which is not nothing and is reported
    // rather than hidden (INV-008; docs/03 §2).
    assert_eq!(
        claim.trusted_components(),
        [
            "certificate-model-correspondence",
            "envelope-digest-binding"
        ]
    );
}

#[test]
fn the_decoded_body_is_the_die_hard_transition_relation() {
    let model = model();
    let bytes = certificate(&model);
    let decoded = wire::decode(&bytes).expect("the kernel decodes what the engine wrote");
    assert_eq!(decoded.kind(), CertificateKind::FiniteClosure);

    let Body::FiniteClosure(body) = decoded.body() else {
        panic!("the finite-closure certificate decoded as another family");
    };

    // The declared state domain is the model's, variable for variable.
    assert_eq!(body.domain().arity(), 2);
    let names: Vec<&str> = body
        .domain()
        .variables()
        .iter()
        .map(|variable| variable.name().as_str())
        .collect();
    assert_eq!(names, ["big", "small"]);
    let ranges: Vec<(i64, i64)> = body
        .domain()
        .variables()
        .iter()
        .map(|variable| (variable.lo(), variable.hi()))
        .collect();
    assert_eq!(ranges, [(0, 5), (0, 3)]);

    // The table is the engine's reachable set, in the engine's order, and the kernel's
    // own binary search agrees with the position each state came from.
    let walk = exploration(&model);
    let states = walk.reachable().states();
    assert_eq!(body.table().len(), 16);
    for (index, state) in states.iter().enumerate() {
        let position = u32::try_from(index).expect("16 states");
        assert_eq!(body.table().state(position), Some(state.as_slice()));
        assert_eq!(body.table().position(state.as_slice()), Some(position));
    }

    // Initial states, action names, and one row per table state.
    assert_eq!(body.initial_states(), [vec![0, 0]]);
    let actions: Vec<&str> = body
        .actions()
        .iter()
        .map(continuum_kernel_core::wire::Token::as_str)
        .collect();
    assert_eq!(
        actions,
        [
            "BigToSmall",
            "EmptyBig",
            "EmptySmall",
            "FillBig",
            "FillSmall",
            "SmallToBig"
        ],
        "the corpus spellings, in the wire form's byte order"
    );
    assert_eq!(body.rows().len(), 16);

    // Every row is the model's own successor row, transition for transition. This is
    // the closure obligation's raw material: the kernel located every one of these
    // targets in the table above, which is what `Post(S) ⊆ S` means.
    let mut total = 0_usize;
    for (index, state) in states.iter().enumerate() {
        let expected = model
            .successors(state)
            .expect("Die Hard evaluates everywhere");
        let row = body.rows().get(index).expect("one row per table state");
        assert_eq!(row.len(), expected.len(), "row {index}");
        for (transition, step) in row.iter().zip(expected.iter()) {
            assert_eq!(usize::from(transition.action()), step.action());
            assert_eq!(transition.target(), step.target().as_slice());
        }
        total += row.len();
    }
    assert_eq!(total, 96, "the frozen labelled-transition count");
}

// ---------------------------------------------------------------------------
// the checker is not accepting vacuously
// ---------------------------------------------------------------------------

/// Where the canonical state table starts, derived from the grammar rather than
/// measured from the bytes.
///
/// `header:12 | envelope | variable_count:2 variable* | state_count:4`.
fn table_offset(claim: &ClaimEnvelope<'_>, model: &Model) -> usize {
    let token = |value: &str| 2 + value.len();
    let mut at = 8 + 2 + 2;
    at += token(claim.model_digest);
    at += token(claim.semantic_epoch);
    at += token(claim.property_digest);
    at += token(claim.scope_digest);
    at += token(claim.assumptions_digest);
    at += token(claim.producer);
    at += 2 + 2; // schema_epoch, domain_pack_count (no packs)
    at += 2; // variable_count
    for variable in model.variables() {
        at += token(variable.name().as_str()) + 16;
    }
    at += 4; // state_count
    at
}

#[test]
fn every_single_byte_mutation_of_the_state_table_is_rejected() {
    let model = model();
    let green = certificate(&model);
    let start = table_offset(&envelope(), &model);
    let width = 16 * 2 * 8;
    let end = start + width;

    // The offset is a claim about the layout, so check it before trusting it: the
    // first state of the Die Hard table is `(0, 0)`.
    assert_eq!(
        green.get(start..start + 16),
        Some([0_u8; 16].as_slice()),
        "the table does not start where the grammar says it does"
    );
    assert!(end <= green.len());

    let mut checked = 0_usize;
    for offset in start..end {
        for mutate in [|byte: u8| byte ^ 0xFF, |byte: u8| byte.wrapping_add(1)] {
            let mut bytes = green.clone();
            let slot = bytes.get_mut(offset).expect("offset is inside the table");
            *slot = mutate(*slot);
            if bytes == green {
                continue;
            }
            let verdict = check_certificate(&bytes);
            assert!(
                !verdict.is_verified(),
                "byte {offset} of the state table was mutated and the kernel still \
                 verified the certificate: {verdict:?}"
            );
            checked += 1;
        }
    }
    assert_eq!(checked, width * 2, "every table byte, mutated two ways");
}

#[test]
fn the_header_the_tail_and_every_prefix_are_rejected_by_name() {
    let green = certificate(&model());
    assert!(check_certificate(&green).is_verified());

    // The magic.
    let mut wrong_magic = green.clone();
    if let Some(byte) = wrong_magic.get_mut(0) {
        *byte ^= 0x01;
    }
    assert_eq!(
        check_certificate(&wrong_magic),
        Verdict::Rejected(Rejection::BadMagic)
    );

    // The wire epoch: not a rejection, because the artifact may be valid under a
    // contract this build does not know (kernel-core wire.rs:112-116).
    let mut other_epoch = green.clone();
    if let Some(byte) = other_epoch.get_mut(9) {
        *byte = 2;
    }
    assert_eq!(
        check_certificate(&other_epoch),
        Verdict::Unsupported(Feature::WireEpoch { found: 2 })
    );

    // The family code.
    let mut other_kind = green.clone();
    if let Some(byte) = other_kind.get_mut(11) {
        *byte = 7;
    }
    assert_eq!(
        check_certificate(&other_kind),
        Verdict::Unsupported(Feature::CertificateKind { found: 7 })
    );

    // A tolerated suffix would be a second encoding of one claim.
    let mut trailing = green.clone();
    trailing.push(0x00);
    assert_eq!(
        check_certificate(&trailing),
        Verdict::Rejected(Rejection::TrailingBytes { extra: 1 })
    );

    // Truncation is the most likely corruption of a stored artifact; no prefix decodes.
    for cut in 0..green.len() {
        let prefix = green.get(..cut).expect("cut is inside the certificate");
        assert!(
            !check_certificate(prefix).is_verified(),
            "prefix of length {cut} verified"
        );
    }
}

#[test]
fn a_widened_domain_bound_still_verifies_so_the_binding_lives_in_the_envelope() {
    // The complement of the sweep above: mutating the *declared domain* can leave a
    // verifiable certificate (a wider `hi` keeps every state in range), so the state
    // table sweep is the honest place to make the non-vacuity claim, and this test
    // records why rather than leaving the asymmetry unstated.
    let model = model();
    let green = certificate(&model);
    let start = table_offset(&envelope(), &model);

    // Walking back from the table: `state_count` is 4 bytes, `small`'s whole variable
    // record is `name:(2+5) lo:8 hi:8` = 23, and `big`'s `hi` ends immediately before
    // it — so its low byte, the one that reads 5, is one further back still.
    let hi_of_big = start - 4 - 23 - 1;
    let mut widened = green.clone();
    let slot = widened
        .get_mut(hi_of_big)
        .expect("the bound is inside the certificate");
    assert_eq!(*slot, 5, "the declared hi bound of `big`");
    *slot = 9;

    assert!(
        check_certificate(&widened).is_verified(),
        "a wider declared domain still admits every reachable state"
    );
    // …and the claim it verifies is a *weaker* one, which is exactly why the property
    // digest is in the envelope and why the kernel names `envelope-digest-binding` as
    // trusted: these bytes no longer describe the model the digest names, and no
    // self-contained checker can know that.
    assert_ne!(widened, green);
}

#[test]
fn garbage_is_not_a_certificate() {
    for bytes in [
        Vec::new(),
        b"CONTCERT".to_vec(),
        vec![0x00; 64],
        b"not a certificate at all".to_vec(),
    ] {
        let verdict = check_certificate(&bytes);
        assert!(!verdict.is_verified(), "{verdict:?}");
    }
}
