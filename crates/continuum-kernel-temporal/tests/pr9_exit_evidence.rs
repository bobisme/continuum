//! Dedicated exit evidence for `PR-9-EXIT` (`notes/plan/notes/START_HERE_IMPLEMENTATION.md`,
//! PR 9's Exit line; delivered across bn-2i8 and bn-2he).
//!
//! > **Exit:** every single-field certificate mutation in the test suite is rejected,
//! > checking wire-form input only.
//! >
//! > — `notes/plan/notes/START_HERE_IMPLEMENTATION.md`, PR 9
//!
//! See `continuum-kernel-core/tests/pr9_exit_evidence.rs` for the shared rationale this
//! file repeats per crate. PR 9's sentence already has a mechanical, cross-crate index
//! — `tools/kernel-covenant/mutation-matrix.toml` (bn-2he) and `tools/
//! check_kernel_covenant.py`'s fail-closed KCOV-08 — and this crate's own
//! `src/check.rs` and `tests/wire_form_boundary.rs` already carry most of the
//! sentence's weight as exhaustive field sweeps. This file adds the residual:
//! single-field mutations this crate owns in the matrix but has so far only exercised
//! internally (via `Plan`), now exercised from wire-form bytes alone, and a mechanical
//! check that "checking wire-form input only" still holds against this crate's own
//! sources.
//!
//! # Evidence map
//!
//! - **"every single-field certificate mutation … is rejected"** — carried
//!   exhaustively by `check::tests::removing_any_transition_target_from_the_table_breaks_the_certificate`,
//!   `check::tests::the_carried_ranking_is_minimal_so_no_rank_can_shrink`,
//!   `check::tests::a_multi_state_component_is_found_and_judged_as_one`,
//!   `check::tests::no_two_byte_strings_decode_to_the_same_certificate`, and
//!   `check::tests::garbage_bytes_never_panic` (`src/check.rs`), and from outside the
//!   crate by `weak_fairness_is_what_separates_a_stall_from_a_counterexample` (`tests/
//!   wire_form_boundary.rs`) — already the `body.reference-retarget` class
//!   (`Rejection::FairCycleExists`), from wire-form bytes alone. `envelope.digest-
//!   relabelling` is `uncovered` for this crate in the matrix, a tracked gap bn-1q4r7
//!   owns and out of scope here. Three classes are added here that
//!   `wire_form_boundary.rs` does not yet touch:
//!   [`an_envelope_wire_epoch_this_build_does_not_implement_is_unsupported_from_bytes_alone`]
//!   (`framing.wire-epoch-substitution`, corroborated internally by
//!   `check::tests::unimplemented_contracts_are_unsupported_not_rejected`),
//!   [`a_domain_pack_count_beyond_the_declared_ceiling_is_rejected_from_bytes_alone`]
//!   (`encoding.count-out-of-range`, corroborated internally by
//!   `check::tests::counts_outside_the_declared_range_are_rejected`), and
//!   [`a_fair_action_beyond_the_declared_action_count_is_rejected_from_bytes_alone`],
//!   which repeats `check::tests::a_fair_action_outside_the_action_table_is_rejected`'s
//!   `body.dangling-reference` claim (`Rejection::FairActionOutOfRange`) from wire-form
//!   bytes alone — a different field, and a different certificate family
//!   (fair-SCC-exclusion rather than ranking), from the reference-retarget mutation
//!   `wire_form_boundary.rs` already shows.
//! - **"checking wire-form input only"** — `src/lib.rs`'s own module documentation:
//!   "`check_certificate` takes `&[u8]`. There is no other entry point, and no
//!   constructor exists for `wire::Certificate` outside `wire::decode`." Checked
//!   mechanically here, against `include_str!`-loaded copies of this crate's own
//!   sources, the same idiom `continuum_intent::tests::pr4_exit_evidence`'s
//!   `no_type_in_this_crate_has_a_mut_self_method` uses.
//!
//! # House rules
//!
//! `src/` is not touched; no existing test is touched; the matrix and covenant checker
//! are cited, not re-implemented; the encoder is duplicated locally from `tests/
//! wire_form_boundary.rs`; `envelope.digest-relabelling` is not touched anywhere in
//! this file (bn-1q4r7's fence).

use continuum_kernel_temporal::wire::{MAGIC, WIRE_EPOCH, decode};
use continuum_kernel_temporal::{
    CertificateKind, FairnessClass, Feature, Rejection, Verdict, check_certificate,
};

// --- the encoder, duplicated from `tests/wire_form_boundary.rs` -----------------------

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

    fn i64(&mut self, value: i64) -> &mut Self {
        self.0.extend_from_slice(&value.to_be_bytes());
        self
    }

    fn token(&mut self, value: &str) -> &mut Self {
        self.u16(u16::try_from(value.len()).expect("fixture tokens are short"));
        self.0.extend_from_slice(value.as_bytes());
        self
    }

    fn header(&mut self, kind: u16, domain_pack_count: u16) -> &mut Self {
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
        self.u16(domain_pack_count)
    }
}

/// The same graph as `wire_form_boundary.rs`'s `countdown_with_stall`, with a `stall`
/// self-loop at `n = 2` and `tick` declared weakly fair, parameterized over the three
/// fields this file mutates.
///
/// - `domain_pack_count` — the envelope's declared domain-pack count. `0` (green)
///   declares none.
/// - `fair_action` — the one weakly-fair action's index. `1` (green) names `tick`;
///   anything `>=` the two-action count is out of range.
fn countdown_with_fair_tick_shaped(domain_pack_count: u16, fair_action: u16) -> Vec<u8> {
    let mut out = Bytes::default();
    out.header(2, domain_pack_count);

    out.u16(1); // one variable
    out.token("n");
    out.i64(0);
    out.i64(2);

    out.u32(3); // three states
    out.i64(0);
    out.i64(1);
    out.i64(2);

    out.u16(1); // property class: eventually-state-set

    out.u32(1); // goal: n = 0
    out.i64(0);

    out.u32(1); // initial: n = 2
    out.i64(2);

    out.u16(2); // two actions, strictly ascending
    out.token("stall");
    out.token("tick");

    out.u16(1); // weak fairness
    out.u16(1); // one fair action
    out.u16(fair_action);

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

/// The green certificate: `wire_form_boundary.rs`'s `countdown_with_stall(true)`, byte
/// for byte.
fn countdown_with_fair_tick() -> Vec<u8> {
    countdown_with_fair_tick_shaped(0, 1)
}

// --- clause 2: "checking wire-form input only" ----------------------------------------

const LIB_SRC: &str = include_str!("../src/lib.rs");
const CHECK_SRC: &str = include_str!("../src/check.rs");
const WIRE_SRC: &str = include_str!("../src/wire.rs");
const VERDICT_SRC: &str = include_str!("../src/verdict.rs");
const RECEIPT_SRC: &str = include_str!("../src/receipt.rs");
const FIXTURE_SRC: &str = include_str!("../src/fixture.rs");

/// Every source file this crate's public surface could hide a second entry point in.
const CRATE_SOURCES: &[(&str, &str)] = &[
    ("lib.rs", LIB_SRC),
    ("check.rs", CHECK_SRC),
    ("wire.rs", WIRE_SRC),
    ("verdict.rs", VERDICT_SRC),
    ("receipt.rs", RECEIPT_SRC),
    ("fixture.rs", FIXTURE_SRC),
];

/// Whether `source` contains `-> {exact_type}` as a bare return type, not merely as a
/// prefix of a longer identifier.
fn contains_bare_return_type(source: &str, exact_type: &str) -> bool {
    let marker = format!("-> {exact_type}");
    let mut search_from = 0usize;
    while let Some(pos) = source.get(search_from..).and_then(|s| s.find(&marker)) {
        let hit = search_from + pos;
        let after = source.get(hit + marker.len()..).unwrap_or("");
        let is_bare = match after.chars().next() {
            Some(next) => !next.is_alphanumeric() && next != '_',
            None => true,
        };
        if is_bare {
            return true;
        }
        search_from = hit + marker.len();
    }
    false
}

#[test]
fn checking_has_exactly_one_public_entry_point_and_it_takes_wire_form_bytes() {
    let mut public_verdict_producers = Vec::new();
    for (name, source) in CRATE_SOURCES {
        for line in source.lines() {
            let trimmed = line.trim_start();
            if trimmed.starts_with("pub fn") && line.contains("-> Verdict") {
                public_verdict_producers.push(format!("{name}: {}", line.trim()));
            }
        }
    }
    assert_eq!(
        public_verdict_producers,
        vec!["check.rs: pub fn check_certificate(bytes: &[u8]) -> Verdict {".to_owned()],
        "the crate must expose exactly one public route to a Verdict, and it must take \
         wire-form bytes; found {public_verdict_producers:?}"
    );
}

#[test]
fn certificate_has_no_public_constructor_no_public_field_and_no_manual_trait_impl() {
    let struct_start = WIRE_SRC
        .find("pub struct Certificate {")
        .expect("Certificate is declared in wire.rs");
    let struct_tail = &WIRE_SRC[struct_start..];
    let struct_end = struct_tail
        .find("\n}\n")
        .expect("the struct body has a closing brace");
    let struct_block = &struct_tail[..struct_end];
    for line in struct_block.lines().skip(1) {
        let trimmed = line.trim_start();
        assert!(
            !trimmed.starts_with("pub "),
            "Certificate grew a public field ({trimmed:?}); an external caller could then \
             build one without going through decode"
        );
    }

    let impl_start = WIRE_SRC
        .find("impl Certificate {")
        .expect("Certificate has an inherent impl");
    let impl_tail = &WIRE_SRC[impl_start..];
    let impl_end = impl_tail
        .find("\n}\n")
        .expect("the impl body has a closing brace");
    let impl_block = &impl_tail[..impl_end];
    assert!(
        !impl_block.contains("fn new("),
        "Certificate's inherent impl grew a `new` constructor outside `decode`"
    );
    assert!(
        !impl_block.contains("-> Self"),
        "Certificate's inherent impl grew a method that can mint one from `&self`"
    );
    assert!(
        !contains_bare_return_type(impl_block, "Certificate"),
        "Certificate's inherent impl grew a method returning a fresh Certificate rather \
         than borrowing the one `decode` already built"
    );

    assert!(
        !WIRE_SRC.contains("for Certificate"),
        "an `impl … for Certificate` appeared (From/Default/etc. would each read `for \
         Certificate`); decode is meant to be the only way in"
    );
}

// --- clause 1: three more single-field mutations, exercised from wire-form bytes alone -

#[test]
fn an_envelope_wire_epoch_this_build_does_not_implement_is_unsupported_from_bytes_alone() {
    let mut bytes = countdown_with_fair_tick();
    let unimplemented_epoch = WIRE_EPOCH.saturating_add(1);
    let patch = unimplemented_epoch.to_be_bytes();
    bytes[8] = patch[0];
    bytes[9] = patch[1];

    assert_eq!(
        check_certificate(&bytes),
        Verdict::Unsupported(Feature::WireEpoch {
            found: unimplemented_epoch
        }),
        "a wire epoch this build does not implement must be Unsupported, never Rejected"
    );

    assert!(check_certificate(&countdown_with_fair_tick()).is_verified());
}

#[test]
fn a_domain_pack_count_beyond_the_declared_ceiling_is_rejected_from_bytes_alone() {
    const MAX_DOMAIN_PACKS: u16 = 64;
    let bytes = countdown_with_fair_tick_shaped(MAX_DOMAIN_PACKS.saturating_add(1), 1);

    let Verdict::Rejected(rejection) = check_certificate(&bytes) else {
        panic!("a domain-pack count above the declared ceiling must be rejected");
    };
    assert_eq!(
        rejection,
        Rejection::CountOutOfRange {
            field: continuum_kernel_temporal::Field::DomainPackCount,
            found: u64::from(MAX_DOMAIN_PACKS.saturating_add(1)),
            min: 0,
            max: u64::from(MAX_DOMAIN_PACKS),
        }
    );
}

#[test]
fn a_fair_action_beyond_the_declared_action_count_is_rejected_from_bytes_alone() {
    // Only two actions are declared (`stall` at 0, `tick` at 1); fair action index `5`
    // names an action the certificate never introduced.
    let bytes = countdown_with_fair_tick_shaped(0, 5);

    assert_eq!(
        check_certificate(&bytes),
        Verdict::Rejected(Rejection::FairActionOutOfRange {
            position: 0,
            action: 5,
            actions: 2,
        })
    );
}

// --- the shaped fixture still agrees with the original, unmutated ---------------------

#[test]
fn the_shaped_green_fixture_still_matches_the_original_countdown_with_stall() {
    let bytes = countdown_with_fair_tick();
    let Verdict::Verified(claim) = check_certificate(&bytes) else {
        panic!("the unmutated shaped fixture must still check green");
    };
    assert_eq!(claim.kind(), CertificateKind::FairSccExclusion);
    assert_eq!(claim.fairness(), FairnessClass::Weak);
    let redecoded = decode(&bytes).expect("the green fixture decodes");
    assert_eq!(redecoded.kind(), CertificateKind::FairSccExclusion);
}
