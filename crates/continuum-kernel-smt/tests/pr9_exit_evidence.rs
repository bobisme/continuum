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
//! `src/check.rs` already carries most of the sentence's weight as exhaustive field
//! sweeps. This file adds the residual: single-field mutations this crate owns in the
//! matrix but has so far only exercised internally (via `Plan`), now exercised from
//! wire-form bytes alone, and a mechanical check that "checking wire-form input only"
//! still holds against this crate's own sources.
//!
//! Unlike its siblings, this crate's own `tests/wire_form_boundary.rs` has no body
//! mutation at all yet — only the two verified-certificate assurance-class tests plus
//! truncation and trailing bytes — so the gap this file closes is proportionally
//! larger here: it is the first place this crate rejects a mutated body from wire-form
//! bytes alone rather than through the internal `Plan` fixture.
//!
//! # Evidence map
//!
//! - **"every single-field certificate mutation … is rejected"** — carried
//!   exhaustively by `check::tests::dropping_any_antecedent_breaks_its_step`,
//!   `check::tests::dropping_a_lemma_the_proof_uses_breaks_the_proof`,
//!   `check::tests::every_antecedent_retarget_is_rejected`,
//!   `check::tests::no_two_byte_strings_decode_to_the_same_certificate`, and
//!   `check::tests::garbage_bytes_never_panic` (`src/check.rs`), and from outside the
//!   crate by `a_public_consumer_sees_typed_rejections_not_panics` (`tests/
//!   wire_form_boundary.rs`, truncation and trailing bytes only).
//!   `envelope.digest-relabelling` is `uncovered` for this crate in the matrix — a
//!   tracked gap bn-1q4r7 owns, out of scope here per this bone's fence. Three classes
//!   are added here, all new ground for this crate's *external* suite:
//!   [`an_envelope_wire_epoch_this_build_does_not_implement_is_unsupported_from_bytes_alone`]
//!   (`framing.wire-epoch-substitution`, corroborated internally by
//!   `check::tests::a_wire_epoch_this_build_does_not_implement_is_unsupported_not_rejected`),
//!   [`a_domain_pack_count_beyond_the_declared_ceiling_is_rejected_from_bytes_alone`]
//!   (`encoding.count-out-of-range`, corroborated internally by
//!   `check::tests::counts_outside_the_declared_range_are_rejected`), and
//!   [`a_clause_literal_beyond_the_declared_atom_count_is_rejected_from_bytes_alone`],
//!   which repeats `check::tests::a_literal_outside_the_atom_table_is_rejected`'s
//!   `body.dangling-reference` claim (`Rejection::AtomOutOfRange`) from wire-form bytes
//!   alone, caught inside `decode` itself.
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

use continuum_kernel_smt::wire::{MAGIC, WIRE_EPOCH, decode};
use continuum_kernel_smt::{CertificateKind, Feature, Rejection, Verdict, check_certificate};

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

    fn envelope(&mut self, domain_pack_count: u16) -> &mut Self {
        self.token("blake3:boundary-model");
        self.token("continuum-semantics-1");
        self.token("blake3:boundary-property");
        self.token("blake3:boundary-scope");
        self.token("blake3:empty-assumptions");
        self.token("continuum-solver-adapter/0.0.0");
        self.u16(WIRE_EPOCH);
        self.u16(domain_pack_count)
    }
}

/// `a < b` and `b < a`, refuted by one arithmetic lemma. Same fixture as
/// `wire_form_boundary.rs`'s `antisymmetry_certificate(true)`, parameterized over the
/// two fields this file mutates.
///
/// - `domain_pack_count` — the envelope's declared domain-pack count. `0` (green)
///   declares none.
/// - `first_assertion_literal` — assertion clause `1`'s one literal. `1` (green) names
///   atom `1` (`a-lt-b`) positively; anything with `unsigned_abs() > atoms` (`2` here)
///   is out of range.
fn antisymmetry_certificate_shaped(
    domain_pack_count: u16,
    first_assertion_literal: i32,
) -> Vec<u8> {
    let mut out = Bytes::default();
    out.0.extend_from_slice(&MAGIC);
    out.u16(WIRE_EPOCH);
    out.u16(1); // smt-proof
    out.envelope(domain_pack_count);

    out.u32(2); // two atoms
    out.token("a-lt-b");
    out.token("b-lt-a");

    out.u32(2); // assertions: ids 1, 2
    out.clause(&[first_assertion_literal]);
    out.clause(&[2]);
    out.u32(1); // lemmas: id 3
    out.token("LIA");
    out.clause(&[-1, -2]);

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

/// The green certificate: `wire_form_boundary.rs`'s `antisymmetry_certificate(true)`,
/// byte for byte.
fn antisymmetry_certificate() -> Vec<u8> {
    antisymmetry_certificate_shaped(0, 1)
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
    let mut bytes = antisymmetry_certificate();
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

    assert!(check_certificate(&antisymmetry_certificate()).is_verified());
}

#[test]
fn a_domain_pack_count_beyond_the_declared_ceiling_is_rejected_from_bytes_alone() {
    const MAX_DOMAIN_PACKS: u16 = 64;
    let bytes = antisymmetry_certificate_shaped(MAX_DOMAIN_PACKS.saturating_add(1), 1);

    let Verdict::Rejected(rejection) = check_certificate(&bytes) else {
        panic!("a domain-pack count above the declared ceiling must be rejected");
    };
    assert_eq!(
        rejection,
        Rejection::CountOutOfRange {
            field: continuum_kernel_smt::Field::DomainPackCount,
            found: u64::from(MAX_DOMAIN_PACKS.saturating_add(1)),
            min: 0,
            max: u64::from(MAX_DOMAIN_PACKS),
        }
    );
}

#[test]
fn a_clause_literal_beyond_the_declared_atom_count_is_rejected_from_bytes_alone() {
    // Only two atoms are declared; literal `9` names an atom the certificate never
    // introduced. Caught inside `decode` itself, before any proof step is even read.
    let bytes = antisymmetry_certificate_shaped(0, 9);

    assert_eq!(
        check_certificate(&bytes),
        Verdict::Rejected(Rejection::AtomOutOfRange {
            clause: 1,
            position: 0,
            literal: 9,
            atoms: 2,
        })
    );
}

// --- the shaped fixture still agrees with the original, unmutated ---------------------

#[test]
fn the_shaped_green_fixture_still_matches_the_original_antisymmetry_certificate() {
    let bytes = antisymmetry_certificate();
    let Verdict::Verified(claim) = check_certificate(&bytes) else {
        panic!("the unmutated shaped fixture must still check green");
    };
    assert_eq!(claim.kind(), CertificateKind::SmtProof);
    assert_eq!(claim.lemmas_used(), 1);
    let redecoded = decode(&bytes).expect("the green fixture decodes");
    assert_eq!(redecoded.kind(), CertificateKind::SmtProof);
}
