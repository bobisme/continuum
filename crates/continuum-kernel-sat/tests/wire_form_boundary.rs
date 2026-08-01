//! The serialization boundary, exercised from outside the crate (PR 9).
//!
//! > serialization boundary: certificates are checked from wire form, never from
//! > shared memory;
//! >
//! > — `notes/plan/plan.md` §20, "Dependency rules"
//!
//! An integration test sees exactly what a consumer sees: the public surface, with no
//! access to the crate's `cfg(test)` fixtures. Everything below therefore builds its
//! certificate as bytes with a hand-written encoder that shares nothing with the
//! decoder under test, which is both the point of the boundary and the point of
//! docs/03 §8's independence requirement.
//!
//! The negative half of the claim is a compile-time property and cannot be asserted
//! here: `continuum_kernel_sat::wire::Certificate` has no public constructor, no public
//! fields, and no `From`/`Default` impl, so `decode(&[u8])` is the only way to obtain
//! one and `check_certificate(&[u8])` is the only way to obtain a `Verdict::Verified`.
//! A change that added one would make this file's opening sentence false, which is why
//! it is written down.

use continuum_kernel_sat::wire::{MAGIC, WIRE_EPOCH, decode};
use continuum_kernel_sat::{CertificateKind, Rejection, Verdict, check_certificate};

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

    fn i32(&mut self, value: i32) -> &mut Self {
        self.0.extend_from_slice(&value.to_be_bytes());
        self
    }

    fn token(&mut self, value: &str) -> &mut Self {
        self.u16(u16::try_from(value.len()).expect("fixture tokens are short"));
        self.0.extend_from_slice(value.as_bytes());
        self
    }

    fn clause(&mut self, literals: &[i32]) -> &mut Self {
        self.u32(u32::try_from(literals.len()).expect("fixture clauses are short"));
        for literal in literals {
            self.i32(*literal);
        }
        self
    }
}

/// `(x) ∧ (¬x)`: the smallest interesting refutation, and one whose whole proof is a
/// single two-antecedent chain that a reader can verify from these bytes.
fn contradiction_certificate() -> Vec<u8> {
    let mut out = Bytes::default();
    out.0.extend_from_slice(&MAGIC);
    out.u16(WIRE_EPOCH);
    out.u16(1); // lrat

    out.token("blake3:contradiction-model");
    out.token("continuum-semantics-1");
    out.token("blake3:contradiction-property");
    out.token("blake3:contradiction-scope");
    out.token("blake3:empty-assumptions");
    out.token("continuum-solver-adapter/0.0.0");
    out.u16(WIRE_EPOCH);
    out.u16(0); // no domain packs

    out.u32(1); // one variable
    out.u32(2); // two clauses
    out.clause(&[1]); // id 1: (x)
    out.clause(&[-1]); // id 2: (¬x)

    out.u32(1); // one proof step
    out.u16(1); // add
    out.u32(3); // id 3
    out.clause(&[]); // the empty clause
    out.u32(2); // two antecedents
    out.u32(1); // clause 1 propagates x
    out.u32(2); // clause 2 is then falsified

    out.0
}

#[test]
fn a_certificate_is_checked_from_bytes_a_consumer_can_write() {
    let bytes = contradiction_certificate();
    let Verdict::Verified(claim) = check_certificate(&bytes) else {
        panic!("the hand-encoded refutation must check green");
    };
    assert_eq!(claim.kind(), CertificateKind::Lrat);
    assert_eq!(claim.variables(), 1);
    assert_eq!(claim.input_clauses(), 2);
    assert_eq!(claim.derived_clauses(), 1);
    assert_eq!(claim.deleted_clauses(), 0);
    assert_eq!(claim.propagations(), 1);
    assert_eq!(
        claim.envelope().producer().as_str(),
        "continuum-solver-adapter/0.0.0"
    );
    assert_eq!(
        claim.trusted_components(),
        ["envelope-digest-binding", "formula-model-correspondence"],
        "a verified refutation must still name what it could not check"
    );
}

#[test]
fn the_decoder_and_the_checker_agree_on_the_same_bytes() {
    let bytes = contradiction_certificate();
    let certificate = decode(&bytes).expect("the green certificate decodes");
    assert_eq!(certificate.kind(), CertificateKind::Lrat);
    assert_eq!(certificate.envelope().schema_epoch(), WIRE_EPOCH);
    assert!(check_certificate(&bytes).is_verified());
}

#[test]
fn a_public_consumer_sees_typed_rejections_not_panics() {
    let mut bytes = contradiction_certificate();
    bytes.push(0x00);
    assert_eq!(
        check_certificate(&bytes),
        Verdict::Rejected(Rejection::TrailingBytes { extra: 1 })
    );

    // Every prefix, from outside the crate.
    let green = contradiction_certificate();
    assert!(
        green.len() < 8192,
        "the fixture stays small enough to sweep"
    );
    for cut in 0..green.len() {
        let prefix = green.get(..cut).expect("cut is within the certificate");
        assert!(
            !check_certificate(prefix).is_verified(),
            "a {cut}-byte prefix was accepted"
        );
    }
}

/// The contradiction fixture with `hints` written verbatim.
fn contradiction_with_chain(hints: &[u32]) -> Vec<u8> {
    let mut out = Bytes::default();
    out.0.extend_from_slice(&MAGIC);
    out.u16(WIRE_EPOCH);
    out.u16(1);
    out.token("blake3:contradiction-model");
    out.token("continuum-semantics-1");
    out.token("blake3:contradiction-property");
    out.token("blake3:contradiction-scope");
    out.token("blake3:empty-assumptions");
    out.token("continuum-solver-adapter/0.0.0");
    out.u16(WIRE_EPOCH);
    out.u16(0);
    out.u32(1);
    out.u32(2);
    out.clause(&[1]);
    out.clause(&[-1]);
    out.u32(1);
    out.u16(1);
    out.u32(3);
    out.clause(&[]);
    out.u32(u32::try_from(hints.len()).expect("fixture chains are short"));
    for hint in hints {
        out.u32(*hint);
    }
    out.0
}

#[test]
fn a_chain_must_be_a_propagation_sequence_not_a_bag_of_clauses() {
    // One antecedent assigns `x` but nothing then contradicts it.
    assert_eq!(
        check_certificate(&contradiction_with_chain(&[1])),
        Verdict::Rejected(Rejection::NoConflict { step: 0 })
    );

    // Repeating it is worse than useless: the second occurrence is already satisfied,
    // so it is not a propagation at all.
    assert_eq!(
        check_certificate(&contradiction_with_chain(&[1, 1])),
        Verdict::Rejected(Rejection::HintSatisfied {
            step: 0,
            position: 1,
            id: 1,
        })
    );

    // An antecedent that was never defined cannot be reached for.
    assert_eq!(
        check_certificate(&contradiction_with_chain(&[1, 4])),
        Verdict::Rejected(Rejection::HintOutOfRange {
            step: 0,
            position: 1,
            id: 4,
        })
    );
}
