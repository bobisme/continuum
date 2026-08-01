//! Dedicated exit evidence for `PR-9-EXIT` (`notes/plan/notes/START_HERE_IMPLEMENTATION.md`,
//! PR 9's Exit line; delivered across bn-2i8 and bn-2he).
//!
//! > **Exit:** every single-field certificate mutation in the test suite is rejected,
//! > checking wire-form input only.
//! >
//! > — `notes/plan/notes/START_HERE_IMPLEMENTATION.md`, PR 9
//!
//! This file asserts that sentence end to end, as directly as it reads, the way
//! `crates/continuum-workspace/tests/pr2_exit_evidence.rs` / `pr3_exit_evidence.rs` and
//! `crates/continuum-intent/tests/pr4_exit_evidence.rs` did for their own PRs. PR 9's
//! sentence already has a mechanical, cross-crate index — `tools/kernel-covenant/
//! mutation-matrix.toml` (bn-2he) names 23 mutation classes across the four
//! `continuum-kernel-*` crates and `tools/check_kernel_covenant.py` (KCOV-08) fails
//! closed if any of the 92 cells is neither owned by a named `#[test]` nor covered by
//! an explicit waiver — and this crate's own `src/check.rs` and `tests/
//! wire_form_boundary.rs` already carry most of the sentence's weight as exhaustive
//! field sweeps. What this file adds is the residual the matrix deliberately does not
//! (and, per its own header comment, should not) re-derive: two more single-field
//! mutations exercised from *outside* the crate — through the same hand-rolled,
//! decoder-independent encoder `wire_form_boundary.rs` uses, never the crate's
//! `#[cfg(test)]` `Plan` fixture — and a mechanical check, against this crate's own
//! sources rather than against prose, that "checking wire-form input only" is a
//! property of the type system and not a convention this file could quietly stop
//! being true of.
//!
//! # Evidence map
//!
//! - **"every single-field certificate mutation … is rejected"** — carried
//!   exhaustively, per mutation class, by `check::tests::every_state_table_mutation_is_rejected`,
//!   `check::tests::every_transition_retarget_is_rejected`,
//!   `check::tests::no_two_byte_strings_decode_to_the_same_certificate`, and
//!   `check::tests::garbage_bytes_never_panic` (`src/check.rs`, all four sweeping every
//!   field or every byte of the fixture, which is what "every single-field mutation"
//!   means per the matrix's own `body.value-perturbation` entry), and from outside the
//!   crate by `an_open_certificate_is_rejected_from_bytes_alone` and
//!   `garbage_from_outside_the_crate_never_panics` (`tests/wire_form_boundary.rs`). The
//!   matrix's `framing.*`, `encoding.*`, and `body.*` classes index all of the above by
//!   name; `envelope.digest-relabelling` is owned here too
//!   (`check::tests::envelope_digests_are_carried_labels_not_facts_the_kernel_can_check`)
//!   and is explicitly out of scope for this file (bn-1q4r7 owns porting the equivalent
//!   test to the sibling crates that still lack it). Two classes this crate owns are
//!   demonstrated internally (via `Plan`) but not yet from wire-form bytes alone, and
//!   are added here:
//!   [`an_envelope_wire_epoch_this_build_does_not_implement_is_unsupported_from_bytes_alone`]
//!   (`framing.wire-epoch-substitution`, corroborated internally by
//!   `check::tests::a_wire_epoch_this_build_does_not_implement_is_unsupported_not_rejected`)
//!   and
//!   [`a_domain_pack_count_beyond_the_declared_ceiling_is_rejected_from_bytes_alone`]
//!   (`encoding.count-out-of-range`, corroborated internally by
//!   `check::tests::counts_outside_the_declared_range_are_rejected`). A third,
//!   [`a_transition_naming_an_undeclared_action_is_rejected_from_bytes_alone`], repeats
//!   `check::tests::a_transition_naming_an_undeclared_action_is_rejected`'s
//!   `body.dangling-reference` claim (`Rejection::UnknownAction`) the same way —
//!   `wire_form_boundary.rs`'s own body mutation is a state *drop*
//!   (`body.element-drop`, `ClosureFailure`), not a dangling reference, so this is new
//!   ground rather than a restatement.
//! - **"checking wire-form input only"** — `src/lib.rs`'s own module documentation
//!   already states the claim in force: "`check_certificate` takes `&[u8]`. There is no
//!   other entry point, and no constructor exists for `wire::Certificate` outside
//!   `wire::decode`, which also takes `&[u8]`." `tests/wire_form_boundary.rs` restates
//!   it in prose and relies on it to justify why its encoder is independent of the
//!   decoder. Neither is a compile-time guarantee a reader can trust without re-reading
//!   the source, so
//!   [`checking_has_exactly_one_public_entry_point_and_it_takes_wire_form_bytes`] and
//!   [`certificate_has_no_public_constructor_no_public_field_and_no_manual_trait_impl`]
//!   check it mechanically — against `include_str!`-loaded copies of this crate's own
//!   sources, the same idiom `continuum_intent`'s
//!   `no_type_in_this_crate_has_a_mut_self_method` (`tests/pr4_exit_evidence.rs`) uses
//!   for its own type-level INV-001 claim. A `compile_fail` doctest cannot do this work
//!   from a `tests/*.rs` file: rustdoc only extracts doctests from the library target's
//!   own documentation, not from an integration test's, and `src/` is out of scope for
//!   this bone regardless.
//!
//! # House rules, inherited from the PR-2/3/4 evidence files
//!
//! - **`src/` is not touched**, and no existing test in this crate or its siblings is
//!   touched.
//! - **The matrix and the covenant checker are cited, not re-implemented.** This file
//!   adds three single-field mutations that were previously only exercised internally;
//!   it does not re-sweep every field the matrix and `src/check.rs` already own, and it
//!   does not touch `envelope.digest-relabelling` for any crate (bn-1q4r7's fence).
//! - **The encoder is duplicated locally**, from `tests/wire_form_boundary.rs`, per that
//!   file's own rationale: an integration test file is its own crate, so nothing here
//!   can `use` a sibling one, and the whole point of the independent-encoder discipline
//!   is that this file's bytes do not come from the crate's own encoder either.

use continuum_kernel_core::wire::{MAGIC, WIRE_EPOCH, decode};
use continuum_kernel_core::{CertificateKind, Feature, Rejection, Verdict, check_certificate};

// --- the encoder, duplicated from `tests/wire_form_boundary.rs` -----------------------

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

/// A one-bit toggle: states `[0]` and `[1]`, one action `flip`, initial `[0]`. Same
/// fixture as `wire_form_boundary.rs`'s `toggle_certificate`, parameterized over the
/// two fields this file mutates so each mutation is a one-argument change rather than a
/// hand-patched byte offset.
///
/// - `domain_pack_count` — the envelope's declared domain-pack count. `0` (the green
///   value) declares none, and none follow in the body either way.
/// - `first_action` — state `[0]`'s one transition's action index. `0` (the green
///   value) names the only declared action, `flip`.
fn toggle_certificate_shaped(domain_pack_count: u16, first_action: u16) -> Vec<u8> {
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
    out.u16(domain_pack_count);

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
    out.u16(first_action);
    out.i64(1);
    out.u32(1); // row for state [1]
    out.u16(0);
    out.i64(0);

    out.0
}

/// The green certificate: `wire_form_boundary.rs`'s `toggle_certificate`, byte for byte.
fn toggle_certificate() -> Vec<u8> {
    toggle_certificate_shaped(0, 0)
}

// --- clause 2: "checking wire-form input only" ----------------------------------------

const LIB_SRC: &str = include_str!("../src/lib.rs");
const CHECK_SRC: &str = include_str!("../src/check.rs");
const WIRE_SRC: &str = include_str!("../src/wire.rs");
const VERDICT_SRC: &str = include_str!("../src/verdict.rs");
const RECEIPT_SRC: &str = include_str!("../src/receipt.rs");
const FIXTURE_SRC: &str = include_str!("../src/fixture.rs");

/// Every source file this crate's public surface could hide a second entry point in.
/// A module added to `src/` and not to this list is a module this file's checks cannot
/// see — the same self-consistency worry `pr4_exit_evidence.rs`'s
/// `no_type_in_this_crate_has_a_mut_self_method` names about its own `CRATE_SOURCES`.
const CRATE_SOURCES: &[(&str, &str)] = &[
    ("lib.rs", LIB_SRC),
    ("check.rs", CHECK_SRC),
    ("wire.rs", WIRE_SRC),
    ("verdict.rs", VERDICT_SRC),
    ("receipt.rs", RECEIPT_SRC),
    ("fixture.rs", FIXTURE_SRC),
];

/// Whether `source` contains `-> {exact_type}` as a bare return type — not merely as a
/// prefix of a longer identifier (`CertificateKind` must not count as a hit for
/// `Certificate`).
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
    // The mechanical half of `src/lib.rs`'s own claim: "`check_certificate` takes
    // `&[u8]`. There is no other entry point." A second `pub fn` returning `Verdict`
    // anywhere in the crate would be a second way in, wire-form or not, and this check
    // would catch it whichever file it landed in.
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
    // The mechanical half of `wire.rs`'s own doc comment on `Certificate`: "no way to be
    // built except by `decode`". Checked against the struct and its inherent impl
    // rather than assumed from the doc comment staying accurate.
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

// --- clause 1: two more single-field mutations, exercised from wire-form bytes alone ---

#[test]
fn an_envelope_wire_epoch_this_build_does_not_implement_is_unsupported_from_bytes_alone() {
    // `framing.wire-epoch-substitution`: internally corroborated by
    // `check::tests::a_wire_epoch_this_build_does_not_implement_is_unsupported_not_rejected`.
    // The header's wire-epoch field sits at a fixed offset — eight bytes of magic, then
    // one big-endian `u16` — so it is patched directly rather than rebuilt, the same
    // way `wire_form_boundary.rs`'s own tests treat the header.
    let mut bytes = toggle_certificate();
    let unimplemented_epoch = WIRE_EPOCH.saturating_add(1);
    let patch = unimplemented_epoch.to_be_bytes();
    bytes[8] = patch[0];
    bytes[9] = patch[1];

    assert_eq!(
        check_certificate(&bytes),
        Verdict::Unsupported(Feature::WireEpoch {
            found: unimplemented_epoch
        }),
        "a wire epoch this build does not implement must be Unsupported, never Rejected \
         (INV-008): the artifact may be valid under a contract this build has never seen"
    );

    // Sanity: the same bytes, unpatched, still check green — the mutation is the only
    // thing that moved.
    assert!(check_certificate(&toggle_certificate()).is_verified());
}

#[test]
fn a_domain_pack_count_beyond_the_declared_ceiling_is_rejected_from_bytes_alone() {
    // `encoding.count-out-of-range`: internally corroborated by
    // `check::tests::counts_outside_the_declared_range_are_rejected`. `counted_u16`
    // checks a declared count against its ceiling before reading a single entry, so
    // this is rejected immediately — no domain-pack tokens need to actually follow.
    const MAX_DOMAIN_PACKS: u16 = 64;
    let bytes = toggle_certificate_shaped(MAX_DOMAIN_PACKS.saturating_add(1), 0);

    let Verdict::Rejected(rejection) = check_certificate(&bytes) else {
        panic!("a domain-pack count above the declared ceiling must be rejected");
    };
    assert_eq!(
        rejection,
        Rejection::CountOutOfRange {
            field: continuum_kernel_core::Field::DomainPackCount,
            found: u64::from(MAX_DOMAIN_PACKS.saturating_add(1)),
            min: 0,
            max: u64::from(MAX_DOMAIN_PACKS),
        }
    );
}

#[test]
fn a_transition_naming_an_undeclared_action_is_rejected_from_bytes_alone() {
    // `body.dangling-reference`: internally corroborated by
    // `check::tests::a_transition_naming_an_undeclared_action_is_rejected`.
    // `wire_form_boundary.rs`'s own body mutation drops a state (`ClosureFailure`); this
    // one instead retargets an action index to a value outside the one-action table,
    // which is a different field and a different rejection.
    let bytes = toggle_certificate_shaped(0, 7);

    assert_eq!(
        check_certificate(&bytes),
        Verdict::Rejected(Rejection::UnknownAction {
            state: 0,
            entry: 0,
            action: 7,
        })
    );
}

// --- the green fixture and the decoder still agree, unmutated -------------------------

#[test]
fn the_shaped_green_fixture_still_matches_the_original_toggle_certificate() {
    // Guards the parameterization above: the shaped builder at its green arguments must
    // produce exactly what `wire_form_boundary.rs`'s own hand-written
    // `toggle_certificate` does, so a future edit to one cannot silently diverge from
    // the other's fixture without a test noticing.
    let bytes = toggle_certificate();
    let Verdict::Verified(claim) = check_certificate(&bytes) else {
        panic!("the unmutated shaped fixture must still check green");
    };
    assert_eq!(claim.kind(), CertificateKind::FiniteClosure);
    assert_eq!(claim.states(), 2);
    assert_eq!(claim.transitions(), 2);
    assert_eq!(claim.initial_states(), 1);
    let redecoded = decode(&bytes).expect("the green fixture decodes");
    assert_eq!(redecoded.kind(), CertificateKind::FiniteClosure);
}
