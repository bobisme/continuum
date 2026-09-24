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
/// kernel checks their *shape* and reports the rest as trusted — at wire epoch 2,
/// `CheckedClaim::trusted_components` names only `envelope-digest-binding`, because
/// the certificate carries the model itself — so this test asserts the envelope
/// survives the round trip, never that it is true.
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
    assert_eq!(received.schema_epoch(), 2);
    assert_eq!(received.domain_pack_digests().len(), 0);

    // And what a verified verdict still trusts, which is not nothing and is reported
    // rather than hidden (INV-008; docs/03 §2). At wire epoch 2 the kernel re-derived
    // the relation from the carried model, so the correspondence is no longer trusted
    // (bn-35y4f); the binding of the carried model to the caller's is.
    assert_eq!(claim.wire_epoch(), 2);
    assert_eq!(claim.trusted_components(), ["envelope-digest-binding"]);
    assert_eq!(
        claim.model_identity(),
        Some(model().identity().as_bytes()),
        "the carried model is this model, compared as canonical identity (ADR-0013)"
    );
}

/// `INV-005` ("no ambient nondeterminism") at exactly the seam this file exists to
/// exercise, and a comparison the rest of the file does not make. Every test above either
/// runs the pipeline once (the test just above) or attacks a mutation of *one* run's bytes
/// (`every_single_byte_mutation_of_the_state_table_is_rejected` and its neighbors); none
/// compares two independently produced runs against each other. That is the gap this test
/// closes, and it is the gap `tools/governance/check_code_policy.py`'s GOV-1-04 source scan
/// cannot close no matter how it is extended: a scan proves no clock or RNG is *named* in
/// this crate's source, never that two runs of the composition it names actually agree.
/// Sharing no value between the two runs (fresh `model()`, fresh `bfs::explore`, fresh
/// `ClosedSet`, fresh emission, fresh independent check — `certificate` and
/// `check_certificate` are called twice, each time from nothing) is what makes agreement
/// evidence rather than a tautology.
#[test]
fn two_independent_pipelines_from_a_declared_model_to_a_checked_certificate_are_byte_identical() {
    let bytes_a = certificate(&model());
    let bytes_b = certificate(&model());
    assert_eq!(
        bytes_a, bytes_b,
        "two independent explore-then-emit runs of the same declared Die Hard model produced \
         different certificate bytes; INV-005 requires this pipeline to carry no ambient state \
         between one run and the next"
    );

    // The independent checker, run once per independent pipeline — not once against both.
    let verdict_a = check_certificate(&bytes_a);
    let verdict_b = check_certificate(&bytes_b);
    assert_eq!(
        verdict_a, verdict_b,
        "two byte-identical certificates, checked independently, produced different verdicts; \
         the checker itself (continuum-kernel-core, which this file's own header notes shares no \
         code with the encoder) must be as free of ambient state as the engine it audits"
    );
}

#[test]
fn the_decoded_body_carries_the_model_and_the_reachable_set() {
    let model = model();
    let bytes = certificate(&model);
    let decoded = wire::decode(&bytes).expect("the kernel decodes what the engine wrote");
    assert_eq!(decoded.kind(), CertificateKind::FiniteClosure);

    let Body::ModelClosure(body) = decoded.body() else {
        panic!("the wire-epoch-2 certificate decoded as another family");
    };

    // The carried model is the model's canonical identity, byte for byte.
    assert_eq!(body.model_identity(), model.identity().as_bytes());
    assert_eq!(body.property(), PropertyClass::StateDomain);

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

    // The rows' raw material. Whether each row is the model's is the kernel's check,
    // not this test's: `check_certificate` re-derives every row from the carried model
    // and verified the certificate above.
    assert_eq!(
        body.transition_count(),
        96,
        "the frozen labelled-transition count"
    );
}

// ---------------------------------------------------------------------------
// the checker is not accepting vacuously
// ---------------------------------------------------------------------------

/// Where the canonical state table starts, derived from the grammar rather than
/// measured from the bytes.
///
/// `header:12 | envelope | model_len:4 model | property_class:2 | state_count:4`.
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
    at += model_offset_within_body();
    at += model.identity().as_bytes().len();
    at += 2; // property_class: state-domain
    at += 4; // state_count
    at
}

/// The model section's bytes start four bytes (its length) into the body.
const fn model_offset_within_body() -> usize {
    4
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
        *byte = 3;
    }
    assert_eq!(
        check_certificate(&other_epoch),
        Verdict::Unsupported(Feature::WireEpoch { found: 3 })
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
fn every_single_byte_mutation_of_the_successor_rows_is_rejected() {
    // The wire-epoch-2 closure of bn-2npu's residual, from the engine's own bytes: a
    // row is fully determined by the carried model, so no change to any byte of any
    // row — a count, an action index or a target index — can verify.
    let model = model();
    let green = certificate(&model);
    let start = table_offset(&envelope(), &model) + 16 * 2 * 8;
    let mut checked = 0_usize;
    for offset in start..green.len() {
        for mutate in [|byte: u8| byte ^ 0xFF, |byte: u8| byte.wrapping_add(1)] {
            let mut bytes = green.clone();
            let slot = bytes.get_mut(offset).expect("offset is inside the rows");
            *slot = mutate(*slot);
            let verdict = check_certificate(&bytes);
            assert!(
                !verdict.is_verified(),
                "byte {offset} of the successor rows was mutated and the kernel still \
                 verified the certificate: {verdict:?}"
            );
            checked += 1;
        }
    }
    // 16 row counts and 96 transitions of `action:u16 target:u32`, two ways each.
    assert_eq!(checked, (16 * 4 + 96 * 6) * 2);
}

#[test]
fn a_widened_domain_in_the_carried_model_verifies_a_different_model_identity() {
    // Widening a declared bound inside the carried model leaves a self-consistent
    // certificate about a *different* model: every reachable state is still in range
    // and no update reaches the new values. The kernel verifies it, and the claim
    // reports the identity it checked, which is not this model's. That comparison is
    // the caller's `envelope-digest-binding` obligation, stated here so the residual is
    // not left implicit.
    let model = model();
    let green = certificate(&model);
    let identity = model.identity();
    let identity = identity.as_bytes();
    let model_start = table_offset(&envelope(), &model) - 4 - 2 - identity.len();
    assert_eq!(
        green.get(model_start..model_start + identity.len()),
        Some(identity)
    );

    // `continuum-model/1` (17) | count:8 | token "big" (8 + 3) | lo:8 | hi:8 — the low
    // byte of `big`'s `hi` is the last of those.
    let hi_of_big = model_start + 17 + 8 + 11 + 8 + 7;
    let mut widened = green.clone();
    let slot = widened
        .get_mut(hi_of_big)
        .expect("the bound is inside the certificate");
    assert_eq!(*slot, 5, "the declared hi bound of `big`");
    *slot = 9;

    let Verdict::Verified(claim) = check_certificate(&widened) else {
        panic!("a wider carried domain still admits every reachable state");
    };
    assert_ne!(claim.model_identity(), Some(identity));
    assert_eq!(claim.trusted_components(), ["envelope-digest-binding"]);
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
