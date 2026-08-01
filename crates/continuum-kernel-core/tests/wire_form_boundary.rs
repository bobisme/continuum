//! The serialization boundary, exercised from outside the crate (PR 9, IMPL-04).
//!
//! > serialization boundary: certificates are checked from wire form, never from
//! > shared memory;
//! >
//! > — `notes/plan/notes/START_HERE_IMPLEMENTATION.md`, PR 9
//!
//! An integration test sees exactly what a consumer sees: the public surface, with
//! no access to the crate's `cfg(test)` fixtures. Everything below therefore builds
//! its certificate as bytes with a hand-written encoder that shares nothing with the
//! decoder under test, which is both the point of the boundary and the point of
//! docs/03 §8's independence requirement.
//!
//! The negative half of the claim is a compile-time property and cannot be asserted
//! here: `continuum_kernel_core::wire::Certificate` has no public constructor, no
//! public fields, and no `From`/`Default` impl, so `decode(&[u8])` is the only way to
//! obtain one and `check_certificate(&[u8])` is the only way to obtain a
//! `Verdict::Verified`. A change that added one would make this file's opening
//! sentence false, which is why it is written down.

use continuum_kernel_core::wire::{MAGIC, WIRE_EPOCH, decode};
use continuum_kernel_core::{
    CertificateKind, PropertyClass, Rejection, Verdict, check_certificate,
};

/// A minimal encoder, written against the documented grammar in `wire`'s module
/// documentation rather than against the decoder.
#[derive(Default)]
struct Bytes(Vec<u8>);

impl Bytes {
    fn u16(&mut self, value: u16) -> &mut Self {
        self.0.extend_from_slice(&value.to_be_bytes());
        self
    }

    fn u32(&mut self, value: u32) -> &mut Self {
        self.0.extend_from_slice(&value.to_be_bytes());
        self
    }

    fn i64(&mut self, value: i64) -> &mut Self {
        self.0.extend_from_slice(&value.to_be_bytes());
        self
    }

    fn token(&mut self, value: &str) -> &mut Self {
        self.u16(u16::try_from(value.len()).expect("fixture tokens are short"));
        self.0.extend_from_slice(value.as_bytes());
        self
    }
}

/// A one-bit toggle: states `[0]` and `[1]`, one action `flip`, initial `[0]`.
///
/// Small enough to read as bytes, and closed: both successors are in the table.
fn toggle_certificate() -> Vec<u8> {
    let mut out = Bytes::default();
    out.0.extend_from_slice(&MAGIC);
    out.u16(WIRE_EPOCH);
    out.u16(1); // finite-closure

    out.token("blake3:toggle-model");
    out.token("continuum-semantics-1");
    out.token("blake3:toggle-property");
    out.token("blake3:toggle-scope");
    out.token("blake3:empty-assumptions");
    out.token("continuum-engine-reference/0.0.0");
    out.u16(WIRE_EPOCH);
    out.u16(0); // no domain packs

    out.u16(1); // one variable
    out.token("bit");
    out.i64(0);
    out.i64(1);

    out.u32(2); // two states
    out.i64(0);
    out.i64(1);

    out.u16(1); // property class: state-domain
    out.u32(1); // one initial state
    out.i64(0);
    out.u16(1); // one action
    out.token("flip");

    out.u32(1); // row for state [0]
    out.u16(0);
    out.i64(1);
    out.u32(1); // row for state [1]
    out.u16(0);
    out.i64(0);

    out.0
}

#[test]
fn a_closed_certificate_checks_green_from_bytes_alone() {
    let verdict = check_certificate(&toggle_certificate());
    let Verdict::Verified(claim) = verdict else {
        panic!("a closed, well-typed certificate must verify; got {verdict:?}");
    };
    assert_eq!(claim.kind(), CertificateKind::FiniteClosure);
    assert_eq!(claim.property(), PropertyClass::StateDomain);
    assert_eq!(claim.states(), 2);
    assert_eq!(claim.initial_states(), 1);
    assert_eq!(claim.transitions(), 2);
    assert_eq!(
        claim.envelope().model_digest().as_str(),
        "blake3:toggle-model"
    );
    assert_eq!(
        claim.envelope().semantic_epoch().as_str(),
        "continuum-semantics-1"
    );
    assert!(
        claim
            .trusted_components()
            .contains(&"certificate-model-correspondence"),
        "a verified claim must expose what it still trusts (RFC 0005)"
    );
}

#[test]
fn decode_and_check_agree_on_the_same_bytes() {
    let bytes = toggle_certificate();
    let certificate = decode(&bytes).expect("the fixture decodes");
    assert_eq!(certificate.kind(), CertificateKind::FiniteClosure);
    assert!(check_certificate(&bytes).is_verified());
}

#[test]
fn an_open_certificate_is_rejected_from_bytes_alone() {
    // Drop state [1] from the table but keep the transition that reaches it: the
    // exact shape of an exploration that stopped early.
    let mut out = Bytes::default();
    out.0.extend_from_slice(&MAGIC);
    out.u16(WIRE_EPOCH);
    out.u16(1);
    out.token("blake3:toggle-model");
    out.token("continuum-semantics-1");
    out.token("blake3:toggle-property");
    out.token("blake3:toggle-scope");
    out.token("blake3:empty-assumptions");
    out.token("continuum-engine-reference/0.0.0");
    out.u16(WIRE_EPOCH);
    out.u16(0);
    out.u16(1);
    out.token("bit");
    out.i64(0);
    out.i64(1);
    out.u32(1); // only state [0]
    out.i64(0);
    out.u16(1);
    out.u32(1);
    out.i64(0);
    out.u16(1);
    out.token("flip");
    out.u32(1); // state [0] --flip--> [1], which is not in the table
    out.u16(0);
    out.i64(1);

    assert_eq!(
        check_certificate(&out.0),
        Verdict::Rejected(Rejection::ClosureFailure { state: 0, entry: 0 })
    );
}

#[test]
fn garbage_from_outside_the_crate_never_panics() {
    // The same covenant as the in-crate test, asserted across the public surface a
    // daemon or CLI would call (docs/12 §11, "malformed artifact panic in kernel").
    let green = toggle_certificate();
    for cut in 0..green.len() {
        assert!(!check_certificate(&green[..cut]).is_verified());
    }
    for byte in 0..=u8::MAX {
        let filler = vec![byte; 97];
        assert!(!check_certificate(&filler).is_verified());
    }
    assert!(!check_certificate(&[]).is_verified());
}
