//! The serialization boundary and the assurance boundary, from outside the crate.
//!
//! > serialization boundary: certificates are checked from wire form, never from
//! > shared memory;
//! >
//! > — `notes/plan/plan.md` §20, "Dependency rules"
//!
//! An integration test sees exactly what a consumer sees. The encoder below is written
//! against the documented grammar rather than against the decoder, and the assertions
//! are about the two things a consumer must not be able to miss: that a certificate is
//! checked from bytes, and that a `Verified` verdict carries its assurance class.
//!
//! `continuum_kernel_smt::wire::Certificate` has no public constructor, no public
//! fields, and no `From`/`Default` impl, so `decode(&[u8])` is the only way to obtain
//! one and `check_certificate(&[u8])` the only way to obtain a `Verdict::Verified`.

use continuum_kernel_smt::wire::{MAGIC, WIRE_EPOCH, decode};
use continuum_kernel_smt::{
    AssuranceClass, CertificateKind, Rejection, Verdict, check_certificate,
};

/// A minimal encoder, written against the documented grammar.
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

    fn envelope(&mut self) -> &mut Self {
        self.token("blake3:boundary-model");
        self.token("continuum-semantics-1");
        self.token("blake3:boundary-property");
        self.token("blake3:boundary-scope");
        self.token("blake3:empty-assumptions");
        self.token("continuum-solver-adapter/0.0.0");
        self.u16(WIRE_EPOCH);
        self.u16(0)
    }
}

/// `a < b` and `b < a`, refuted by one arithmetic lemma. `lemma` selects whether the
/// antisymmetry clause is carried as a theory lemma (the honest case) or smuggled in as
/// an assertion (the dishonest one).
fn antisymmetry_certificate(as_lemma: bool) -> Vec<u8> {
    let mut out = Bytes::default();
    out.0.extend_from_slice(&MAGIC);
    out.u16(WIRE_EPOCH);
    out.u16(1); // smt-proof
    out.envelope();

    out.u32(2); // two atoms
    out.token("a-lt-b");
    out.token("b-lt-a");

    if as_lemma {
        out.u32(2); // assertions: ids 1, 2
        out.clause(&[1]);
        out.clause(&[2]);
        out.u32(1); // lemmas: id 3
        out.token("LIA");
        out.clause(&[-1, -2]);
    } else {
        out.u32(3); // assertions: ids 1, 2, 3
        out.clause(&[1]);
        out.clause(&[2]);
        out.clause(&[-1, -2]);
        out.u32(0); // no lemmas
    }

    out.u32(1); // one proof step
    out.u16(1); // resolve
    out.u32(4); // id 4
    out.clause(&[]); // the empty clause
    out.u32(3);
    out.u32(1); // (a<b) propagates
    out.u32(2); // (b<a) propagates
    out.u32(3); // antisymmetry is then falsified

    out.0
}

#[test]
fn a_theory_backed_certificate_reports_its_trusted_theory() {
    let bytes = antisymmetry_certificate(true);
    let Verdict::Verified(claim) = check_certificate(&bytes) else {
        panic!("the hand-encoded refutation must check green");
    };
    assert_eq!(claim.kind(), CertificateKind::SmtProof);
    assert_eq!(claim.assurance_class(), AssuranceClass::TrustedSolver);
    assert_eq!(claim.assurance_class().as_str(), "TRUSTED_SOLVER");
    assert_eq!(claim.lemmas_used(), 1);
    assert_eq!(
        claim.trusted_components(),
        [
            "envelope-digest-binding",
            "skeleton-model-correspondence",
            "theory-lemma:LIA",
        ]
    );
}

#[test]
fn the_same_clause_as_an_assertion_is_a_stronger_claim_not_a_weaker_one() {
    // The two certificates differ only in whether antisymmetry arrives as a *premise*
    // or as a *theory lemma*. Both verify — but the first claims "these three clauses
    // conflict", which needs no theory, and the second claims "these two conflict once
    // LIA is believed". The verdicts say which is which, which is the whole point of
    // ADR-0017's two classes.
    let bytes = antisymmetry_certificate(false);
    let Verdict::Verified(claim) = check_certificate(&bytes) else {
        panic!("the assertion-only refutation must check green");
    };
    assert_eq!(claim.assurance_class(), AssuranceClass::CheckedCertificate);
    assert_eq!(claim.assurance_class().as_str(), "CHECKED_CERTIFICATE");
    assert_eq!(claim.lemmas_carried(), 0);
    assert!(claim.theories().is_empty());
    assert_eq!(
        claim.trusted_components(),
        ["envelope-digest-binding", "skeleton-model-correspondence"]
    );
}

#[test]
fn the_decoder_and_the_checker_agree_on_the_same_bytes() {
    let bytes = antisymmetry_certificate(true);
    let certificate = decode(&bytes).expect("the green certificate decodes");
    assert_eq!(certificate.kind(), CertificateKind::SmtProof);
    assert_eq!(certificate.envelope().schema_epoch(), WIRE_EPOCH);
    assert!(check_certificate(&bytes).is_verified());
}

#[test]
fn a_public_consumer_sees_typed_rejections_not_panics() {
    let mut bytes = antisymmetry_certificate(true);
    bytes.push(0x00);
    assert_eq!(
        check_certificate(&bytes),
        Verdict::Rejected(Rejection::TrailingBytes { extra: 1 })
    );

    let green = antisymmetry_certificate(true);
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
