//! Cross-crate evidence map for `INV-008` (`notes/plan/plan.md:335-337`), kernel half, for
//! bone `bn-n9a1`.
//!
//! > `INV-008` — Typed inconclusiveness
//! >
//! > Timeout, unsupported semantics, insufficient telemetry, abstraction ambiguity, and
//! > incomplete proof search are distinct outcomes.
//! >
//! > — `notes/plan/plan.md:335-337`
//!
//! `bn-n9a1` is an evidence-mapping bone in the T02/bn-2res shape: map the invariant's
//! controls to the mechanical evidence that already exists, and add only what is genuinely
//! missing. This file closes the kernel's control — "unsupported semantics" — which is one
//! of the invariant's five named kinds and the one the trusted checking base owns outright;
//! the engine/task/daemon controls (timeout, insufficient telemetry, abstraction ambiguity,
//! incomplete proof search) are `crates/continuumd/tests/inv008_typed_inconclusiveness_evidence.rs`'s.
//!
//! # What already closes this control, and where
//!
//! Each of the four `continuum-kernel-{core,sat,smt,temporal}` crates carries the same
//! closed, three-arm `Verdict` — `Verified(CheckedClaim)`, `Rejected(Rejection)`,
//! `Unsupported(Feature)` — and each crate's own `src/verdict.rs` module doc states the
//! reason in the same words: rejecting an unrecognized wire epoch or certificate family
//! "would report a possibly valid certificate as invalid, which is exactly the ambiguity
//! INV-008 forbids." `crates/continuum-kernel-core/src/verdict.rs:30-33` is the canonical
//! statement; `sat`, `smt`, and `temporal` each restate it for their own families.
//!
//! Distinctness is proved mechanically, twice over, in every one of the four crates:
//!
//! - **per-crate unit tests.** Each crate's `src/check.rs` has
//!   `a_wire_epoch_this_build_does_not_implement_is_unsupported_not_rejected` (or, in
//!   `temporal`, the equivalently-scoped `unimplemented_contracts_are_unsupported_not_rejected`)
//!   and `an_unimplemented_certificate_family_is_unsupported` (`core`/`sat`/`smt`), driving a
//!   real decode through [`continuum_kernel_core::check`]'s sibling function in each crate
//!   and asserting the result is `Verdict::Unsupported(Feature::...)`, never
//!   `Verdict::Rejected(_)`. Each crate's `src/verdict.rs` additionally carries
//!   `a_non_verified_verdict_carries_no_claim`, which constructs one `Rejected` and one
//!   `Unsupported` value and proves both collapse `is_verified()` to `false` and `claim()`
//!   to `None` — i.e. the *only* boolean-shaped projection this vocabulary offers cannot
//!   distinguish the two, so a caller is never handed a shortcut that would let "rejected"
//!   and "unsupported" quietly become the same `false`.
//! - **the mutation matrix.** `tools/kernel-covenant/mutation-matrix.toml`'s
//!   `framing.wire-epoch-substitution` and `framing.certificate-family-substitution` classes
//!   name exactly this distinction as their `expected` outcome ("Unsupported(WireEpoch) —
//!   never Rejected … (INV-008)" and "Unsupported(CertificateKind / PropertyClass /
//!   FairnessClass), never Rejected.") and bind each class to the per-crate tests above by
//!   name. `tools/check_kernel_covenant.py`'s KCOV-08 (`just check`'s `covenant` recipe,
//!   self-tested before every run) fails closed if a named test is renamed or deleted, or if
//!   a (class, crate) cell is neither owned nor waived.
//!
//! Nothing above is re-implemented here. What no existing check does is look at all four
//! crates *at once* and confirm they still agree with each other: a per-crate unit test
//! proves its own crate's `Verdict` behaves correctly, but nothing previously compared the
//! four `Verdict` declarations against each other, and the covenant tool's KCOV-08 checks
//! that named tests exist and that the matrix accounts for every cell — it does not read
//! the `Verdict` enum's own variant list at all, in any crate. A fifth kernel crate (or an
//! edit to one of the existing four) that quietly reordered the three arms, renamed
//! `Unsupported` to something else, or added a second boolean predicate narrower than
//! `is_verified` would pass every test above and go unnoticed. That is the gap this file
//! closes, by reading all four crates' real `src/verdict.rs` at compile time (`include_str!`,
//! the `inv014_version_explicit_evidence.rs` precedent in this same directory) and comparing
//! them in one place — with no new dependency edge: `continuum-kernel-core`'s `Cargo.toml`
//! gains no `[dev-dependencies]` entry, exactly as that precedent's own comment records
//! ("nothing here shells out, reads a clock, or touches the network").
//!
//! # House rules, inherited from the PR-9/T02 evidence files
//!
//! - `src/` is not touched, in this crate or any sibling kernel crate, and no existing test
//!   is touched.
//! - Existing guards (the per-crate unit tests, the mutation matrix, KCOV-08) are cited, not
//!   re-implemented.
//! - This file reads real repository files at compile time via `include_str!`; nothing here
//!   shells out, reads a clock, opens a socket, or adds a workspace dependency edge.

// --- fixtures: real files, read at compile time ------------------------------------------

const CORE_VERDICT_SRC: &str = include_str!("../src/verdict.rs");
const SAT_VERDICT_SRC: &str = include_str!("../../continuum-kernel-sat/src/verdict.rs");
const SMT_VERDICT_SRC: &str = include_str!("../../continuum-kernel-smt/src/verdict.rs");
const TEMPORAL_VERDICT_SRC: &str = include_str!("../../continuum-kernel-temporal/src/verdict.rs");

const MUTATION_MATRIX: &str = include_str!("../../../tools/kernel-covenant/mutation-matrix.toml");

#[derive(Clone, Copy)]
struct KernelCrate {
    name: &'static str,
    verdict_src: &'static str,
}

const CORE: KernelCrate = KernelCrate {
    name: "continuum-kernel-core",
    verdict_src: CORE_VERDICT_SRC,
};
const SAT: KernelCrate = KernelCrate {
    name: "continuum-kernel-sat",
    verdict_src: SAT_VERDICT_SRC,
};
const SMT: KernelCrate = KernelCrate {
    name: "continuum-kernel-smt",
    verdict_src: SMT_VERDICT_SRC,
};
const TEMPORAL: KernelCrate = KernelCrate {
    name: "continuum-kernel-temporal",
    verdict_src: TEMPORAL_VERDICT_SRC,
};

const KERNEL_CRATES: [KernelCrate; 4] = [CORE, SAT, SMT, TEMPORAL];

/// The closed vocabulary every kernel crate's `Verdict` must declare, in order.
///
/// Declaration order is not cosmetic: `continuum-kernel-core/src/verdict.rs`'s own doc
/// calls the enum "closed by construction: a caller matching on this enum has enumerated
/// every outcome the kernel can produce (INV-008)", and a caller relying on `match`
/// exhaustiveness for that guarantee is relying on there being exactly these three arms —
/// not on some other set that merely happens to have three members.
const EXPECTED_VOCABULARY: [&str; 3] = ["Verified", "Rejected", "Unsupported"];

// --- tiny anchor-based extraction, matching inv014_version_explicit_evidence.rs's style ---

/// The body of `pub enum $name { ... }` in `source`, from just after its opening brace to
/// just before the closing brace that starts a line (rustfmt's own style for a top-level
/// item, and true of all four `Verdict` declarations, which nest no braces of their own).
fn enum_body<'a>(source: &'a str, name: &str) -> &'a str {
    let anchor = format!("pub enum {name} {{");
    let start = source
        .find(&anchor)
        .unwrap_or_else(|| panic!("no `{anchor}` found"));
    let rest = &source[start + anchor.len()..];
    let end = rest
        .find("\n}")
        .unwrap_or_else(|| panic!("no closing brace for enum {name}"));
    &rest[..end]
}

/// The variant identifiers declared in an enum body, in declaration order.
///
/// A hand-rolled scan rather than a regex dependency (this crate adds none): a variant
/// line, after trimming, starts with an uppercase letter and is not a doc comment or
/// attribute; the identifier is everything up to the first character that is neither
/// alphanumeric nor `_`. Works for both tuple variants (`Verified(CheckedClaim),`) and bare
/// ones, which is all four `Verdict` declarations ever use.
fn variant_names(body: &str) -> Vec<&str> {
    body.lines()
        .map(str::trim)
        .filter(|line| {
            !line.is_empty()
                && !line.starts_with('/')
                && !line.starts_with('#')
                && line.chars().next().is_some_and(char::is_uppercase)
        })
        .map(|line| {
            let end = line
                .find(|c: char| !(c.is_alphanumeric() || c == '_'))
                .unwrap_or(line.len());
            &line[..end]
        })
        .collect()
}

/// `Ok` when `found` is exactly `expected`, `Err` naming the two when it is not. Every
/// positive test below calls this and expects `Ok`; the dedicated negative test calls it
/// with a value known to differ and expects `Err`, proving the comparison can actually fail
/// (delivery checklist: "a mutant that would violate it and prove the guard detects it").
fn vocabulary_matches(label: &str, found: &[&str], expected: &[&str]) -> Result<(), String> {
    if found == expected {
        Ok(())
    } else {
        Err(format!("{label}: found {found:?}, expected {expected:?}"))
    }
}

// --- control: the three-arm vocabulary, identical across all four kernel crates ----------

#[test]
fn positive_all_four_kernel_crates_declare_the_same_three_arm_verdict_vocabulary_in_order() {
    for kc in KERNEL_CRATES {
        let body = enum_body(kc.verdict_src, "Verdict");
        let found = variant_names(body);
        vocabulary_matches(kc.name, &found, &EXPECTED_VOCABULARY).unwrap_or_else(|error| {
            panic!(
                "{error} — a kernel crate whose `Verdict` vocabulary drifted from the other \
                 three would still pass every per-crate unit test and KCOV-08 (neither reads \
                 the enum's own variant list), so this cross-crate sweep is the only place \
                 the four are compared against each other"
            )
        });
    }
}

#[test]
fn negative_the_vocabulary_sweep_is_not_vacuous() {
    // A mutant on a copy of `core`'s own real enum body: `Unsupported` renamed to `Unknown`,
    // the exact drift INV-008 forbids (an `Unknown` arm collapses "unsupported semantics"
    // into an undifferentiated catch-all, which `verdict.rs`'s own module doc rules out:
    // "There is deliberately no `Unknown`").
    let body = enum_body(CORE_VERDICT_SRC, "Verdict");
    let mutated_source = body.replace("Unsupported(Feature)", "Unknown(Feature)");
    let mutated = variant_names(&mutated_source);
    let outcome = vocabulary_matches("mutant", &mutated, &EXPECTED_VOCABULARY);
    assert!(
        outcome.is_err(),
        "a renamed arm must be reported, never accepted: {mutated:?}"
    );

    // And a drop, the other direction a real regression could take: a crate that merged
    // `Rejected` and `Unsupported` into one arm.
    let dropped_source = body.replacen("    Rejected(Rejection),\n", "", 1);
    let dropped = variant_names(&dropped_source);
    assert!(
        vocabulary_matches("mutant-dropped", &dropped, &EXPECTED_VOCABULARY).is_err(),
        "a dropped arm must be reported, never accepted: {dropped:?}"
    );
}

// --- control: the only boolean projection is `is_verified`, and it is the only one --------

/// `Verdict::is_verified` is the one place any of the four crates renders this vocabulary as
/// a `bool`, and every crate's own doc comment on it warns why a caller must not use it as a
/// substitute for matching: "a caller that collapses `Rejected` and `Unsupported` into one
/// `false` has thrown away the distinction INV-008 exists to preserve." That warning is only
/// honest if no crate *also* offers a narrower boolean — an `is_rejected` or `is_unsupported`
/// — that a caller could reach for instead and get exactly the collapse the warning forbids.
#[test]
fn positive_is_verified_is_the_only_boolean_projection_in_every_kernel_crate() {
    for kc in KERNEL_CRATES {
        let is_verified_count = kc.verdict_src.matches("pub const fn is_verified").count();
        assert_eq!(
            is_verified_count, 1,
            "{}: exactly one `is_verified` projection",
            kc.name
        );
        for absent in [
            "fn is_rejected",
            "fn is_unsupported",
            "fn is_unsupported_or_rejected",
        ] {
            assert!(
                !kc.verdict_src.contains(absent),
                "{}: `{absent}` would be exactly the second, narrower boolean predicate that \
                 lets a caller re-collapse `Rejected` and `Unsupported` without a match — \
                 INV-008's \"no boolean-typed verdict\" reading of the closed vocabulary",
                kc.name
            );
        }
        // `claim()` is the other total projection every crate offers, and it must agree with
        // `is_verified` on which arms are "not established": both are `None`/`false` for
        // `Rejected` and `Unsupported` alike, so the *only* way to tell them apart is the
        // full match, never a query.
        assert!(
            kc.verdict_src
                .contains("Self::Rejected(_) | Self::Unsupported(_) => None"),
            "{}: `claim()` must fold `Rejected` and `Unsupported` to the same `None`, exactly \
             as `is_verified` folds them to the same `false` — the point being that neither \
             total projection can be used to distinguish them, only the match can",
            kc.name
        );
    }
}

// --- control: the covenant's own prose still says "never Rejected" ------------------------

/// The quoted string value of the `expected` key inside the `[[class]]` block starting at
/// `id = "<class_id>"` in `MUTATION_MATRIX`.
fn matrix_expected(class_id: &str) -> &'static str {
    let anchor = format!("id = \"{class_id}\"");
    let start = MUTATION_MATRIX
        .find(&anchor)
        .unwrap_or_else(|| panic!("mutation-matrix.toml has no class `{class_id}`"));
    let rest = &MUTATION_MATRIX[start..];
    let key = "expected = \"";
    let key_at = rest
        .find(key)
        .unwrap_or_else(|| panic!("class `{class_id}` has no `expected` key"));
    let after_key = &rest[key_at + key.len()..];
    let end = after_key
        .find('"')
        .expect("the expected value's closing quote");
    &after_key[..end]
}

#[test]
fn positive_the_covenant_still_names_unsupported_never_rejected_for_the_two_inv008_classes() {
    for class_id in [
        "framing.wire-epoch-substitution",
        "framing.certificate-family-substitution",
    ] {
        let expected = matrix_expected(class_id);
        assert!(
            expected.contains("Unsupported") && expected.contains("never Rejected"),
            "class `{class_id}`'s `expected` field must still state the INV-008 distinction \
             mechanically, not just historically: found {expected:?}"
        );
    }
    // And the crate list the matrix covers is still exactly the kernel it names in its own
    // header comment — `crates = ["core", "sat", "smt", "temporal"]` — so a fifth kernel
    // crate would have to be added to the matrix explicitly rather than silently uncovered.
    assert!(MUTATION_MATRIX.contains("crates = [\"core\", \"sat\", \"smt\", \"temporal\"]"));
}

#[test]
fn negative_the_matrix_prose_pin_is_not_vacuous() {
    let mutant = "Unsupported(WireEpoch) — treated the same as Rejected.";
    assert!(
        !(mutant.contains("Unsupported") && mutant.contains("never Rejected")),
        "a matrix entry that dropped the 'never Rejected' clause must fail this pin"
    );
}
