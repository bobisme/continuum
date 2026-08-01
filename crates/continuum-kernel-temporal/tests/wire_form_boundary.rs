//! The serialization boundary, exercised from outside the crate (PR 9).
//!
//! > serialization boundary: certificates are checked from wire form, never from
//! > shared memory;
//! >
//! > — `notes/plan/plan.md` §20, "Dependency rules"
//!
//! An integration test sees exactly what a consumer sees: the public surface, with no
//! access to the crate's `cfg(test)` fixtures. The encoder below is written against the
//! documented grammar rather than against the decoder, which is both the point of the
//! boundary and the point of docs/03 §8's independence requirement.
//!
//! The negative half of the claim is a compile-time property: `wire::Certificate` has
//! no public constructor, no public fields, and no `From`/`Default` impl, so
//! `decode(&[u8])` is the only way to obtain one and `check_certificate(&[u8])` the only
//! way to obtain a `Verdict::Verified`.

use continuum_kernel_temporal::wire::{MAGIC, WIRE_EPOCH, decode};
use continuum_kernel_temporal::{
    CertificateKind, FairnessClass, PropertyClass, Rejection, Verdict, check_certificate,
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

    fn u64(&mut self, value: u64) -> &mut Self {
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

    fn header(&mut self, kind: u16) -> &mut Self {
        self.0.extend_from_slice(&MAGIC);
        self.u16(WIRE_EPOCH);
        self.u16(kind);
        self.token("blake3:countdown-model");
        self.token("continuum-semantics-1");
        self.token("blake3:eventually-zero");
        self.token("blake3:countdown-scope");
        self.token("blake3:empty-assumptions");
        self.token("continuum-engine-liveness/0.0.0");
        self.u16(WIRE_EPOCH);
        self.u16(0) // no domain packs
    }

    /// `n ∈ 0..2`, three states, one action `tick` taking `n` to `n - 1`, goal `n = 0`.
    fn countdown_graph(&mut self) -> &mut Self {
        self.u16(1); // one variable
        self.token("n");
        self.i64(0);
        self.i64(2);

        self.u32(3); // three states
        self.i64(0);
        self.i64(1);
        self.i64(2);

        self.u16(1); // property class: eventually-state-set

        self.u32(1); // one goal state
        self.i64(0);

        self.u32(1); // one initial state
        self.i64(2);

        self.u16(1); // one action
        self.token("tick");

        self.u16(1); // weak fairness
        self.u16(0) // no fair actions
    }
}

/// A ranking certificate for the countdown.
fn countdown_ranking() -> Vec<u8> {
    let mut out = Bytes::default();
    out.header(1);
    out.countdown_graph();

    out.u32(0); // row for n = 0: no transitions, and it is the goal
    out.u32(1); // row for n = 1
    out.u16(0);
    out.i64(0);
    out.u32(1); // row for n = 2
    out.u16(0);
    out.i64(1);

    out.u64(0); // rank(0)
    out.u64(1); // rank(1)
    out.u64(2); // rank(2)

    out.0
}

/// The same graph with a `stall` self-loop at `n = 2`, as an exclusion certificate.
///
/// `taken` selects whether the fair action list names `tick`, which is what decides
/// whether the self-loop is a legal execution.
fn countdown_with_stall(fair: bool) -> Vec<u8> {
    let mut out = Bytes::default();
    out.header(2);

    out.u16(1);
    out.token("n");
    out.i64(0);
    out.i64(2);

    out.u32(3);
    out.i64(0);
    out.i64(1);
    out.i64(2);

    out.u16(1);

    out.u32(1); // goal: n = 0
    out.i64(0);

    out.u32(1); // initial: n = 2
    out.i64(2);

    out.u16(2); // two actions, strictly ascending
    out.token("stall");
    out.token("tick");

    out.u16(1); // weak fairness
    if fair {
        out.u16(1);
        out.u16(1); // `tick` is weakly fair
    } else {
        out.u16(0);
    }

    out.u32(0); // n = 0: goal, no transitions
    out.u32(1); // n = 1
    out.u16(1); // tick
    out.i64(0);
    out.u32(2); // n = 2: stall to itself, or tick down
    out.u16(0); // stall
    out.i64(2);
    out.u16(1); // tick
    out.i64(1);

    out.0
}

#[test]
fn a_ranking_certificate_is_checked_from_bytes_a_consumer_can_write() {
    let bytes = countdown_ranking();
    let Verdict::Verified(claim) = check_certificate(&bytes) else {
        panic!("the hand-encoded countdown ranking must check green");
    };
    assert_eq!(claim.kind(), CertificateKind::Ranking);
    assert_eq!(claim.property(), PropertyClass::EventuallyStateSet);
    assert_eq!(claim.fairness(), FairnessClass::Weak);
    assert_eq!(claim.states(), 3);
    assert_eq!(claim.goal_states(), 1);
    assert_eq!(claim.initial_states(), 1);
    assert_eq!(claim.transitions(), 2);
    assert_eq!(claim.fair_actions(), 0);
    assert_eq!(claim.progress_bound(), Some(2));
    assert_eq!(
        claim.trusted_components(),
        [
            "certificate-model-correspondence",
            "envelope-digest-binding"
        ]
    );
}

#[test]
fn weak_fairness_is_what_separates_a_stall_from_a_counterexample() {
    // The same graph twice. Without a fairness assumption the `stall` self-loop is a
    // legal execution that never reaches `n = 0`, and the certificate is refuted by its
    // own carried graph. With `tick` declared weakly fair, the self-loop is not a legal
    // execution — `tick` is enabled throughout it and never taken — and the exclusion
    // holds.
    assert_eq!(
        check_certificate(&countdown_with_stall(false)),
        Verdict::Rejected(Rejection::FairCycleExists { state: 2 })
    );

    let Verdict::Verified(claim) = check_certificate(&countdown_with_stall(true)) else {
        panic!("with weak fairness for `tick` there is no fair cycle");
    };
    assert_eq!(claim.kind(), CertificateKind::FairSccExclusion);
    assert_eq!(claim.reachable_states(), 3);
    assert_eq!(claim.progress_bound(), None);
    assert_eq!(
        claim.trusted_components(),
        [
            "certificate-model-correspondence",
            "envelope-digest-binding",
            "fairness-assumption-correspondence",
        ],
        "an exclusion claim that used a fairness assumption must name it"
    );
}

#[test]
fn the_decoder_and_the_checker_agree_on_the_same_bytes() {
    let bytes = countdown_ranking();
    let certificate = decode(&bytes).expect("the green certificate decodes");
    assert_eq!(certificate.kind(), CertificateKind::Ranking);
    assert_eq!(certificate.envelope().schema_epoch(), WIRE_EPOCH);
    assert_eq!(certificate.body().ranks(), [0, 1, 2]);
    assert!(check_certificate(&bytes).is_verified());
}

#[test]
fn a_public_consumer_sees_typed_rejections_not_panics() {
    let mut bytes = countdown_ranking();
    bytes.push(0x00);
    assert_eq!(
        check_certificate(&bytes),
        Verdict::Rejected(Rejection::TrailingBytes { extra: 1 })
    );

    for green in [countdown_ranking(), countdown_with_stall(true)] {
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
}
