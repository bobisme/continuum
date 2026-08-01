//! Evidence map and gap-closing pins for `INV-014` (`notes/plan/plan.md:359-361`), for
//! bone bn-dw81.
//!
//! > `INV-014` — Version-explicit proof
//! >
//! > Lean version, library closure, theorem hashes, axioms, certificate schema, and
//! > checker identity are recorded.
//! >
//! > — `notes/plan/plan.md`, INV-014's contract sentence
//!
//! bn-dw81 is an evidence-mapping bone, in the shape bn-2res (T02) established: map the
//! invariant's controls to the mechanical evidence that already exists, and add only what
//! is genuinely missing. Six controls, each with its own citation:
//!
//! 1. **Lean version** — `lean/lean-toolchain` pins `leanprover/lean4:v4.32.1`
//!    (`notes/plan/docs/23_LEAN4_FORMALIZATION_PROGRAM.md` "The dossier pins `v4.32.1`");
//!    `just check`'s `lean:` recipe runs `lake build` under that pin, so an unpinned or
//!    wrong toolchain fails the gate. `lean/artifacts/axiom-manifest-t0-t1.{json,txt}`
//!    (ADR-0035) separately *records* the version alongside the axiom output.
//! 2. **library closure** — RFC 0035's own worker-key term for the Lean proof service
//!    ("Lean version, library closure, source snapshot, options, resource policy").
//!    `lean/lake-manifest.json` is the checked-in package-dependency closure; it is
//!    empty (`"packages": []`) by design — `lean/AxiomManifest.lean`'s own doc comment:
//!    "nothing in the metatheory may depend on it" — and `lean/lakefile.toml` requires no
//!    package.
//! 3. **theorem hashes / axioms** — `lean/AxiomManifest.lean` walks the RFC 0012 T0/T1
//!    ladder and records, per theorem, the axioms `Lean.collectAxioms` reports (the same
//!    source `#print axioms` uses); `lean/scripts/axiom-manifest.sh --check` (wired into
//!    `just check`) fails if the checked-in `lean/artifacts/axiom-manifest-t0-t1.{json,txt}`
//!    is stale. PR-4A's exit criterion is "T0/T1 compile with no `sorry` and empty axiom
//!    manifests", which the manifest's `theoremsWithAxioms: 0` records directly.
//! 4. **certificate schema** — `notes/plan/schemas/proof-receipt.schema.json` (RFC 0024),
//!    validated by `tools/validate_dossier.py` (`just check`'s `dossier` recipe) against
//!    `notes/plan/schemas/examples/finite-closure.proof-receipt.json`, and independently
//!    re-validated at the Rust layer by `tools/check_kernel_covenant.py`'s KCOV-09
//!    (`rule_receipt_conformance`), which validates every kernel crate's golden receipt
//!    against the same schema and is named for INV-014 in its own summary line
//!    ("Kernel receipts validate against proof-receipt.schema.json").
//! 5. **checker identity** — `crates/continuum-kernel-{core,sat,smt,temporal}/src/receipt.rs`
//!    each declare a qualified [`WIRE_EPOCH_ID`]-shaped constant
//!    (`<crate>/<magic>/<epoch>`), carried in the receipt's `checker.version` as semver
//!    build metadata (RFC 0026 correction 17: "There is no checker epoch \[...\] checker
//!    identity is \[...\] engine identity for native checkers"); `crates/*/src/receipt.rs`'s
//!    module documentation states the rationale ("a bare `1` would name none of them").
//!    KCOV-09 cross-checks every crate's declared `WIRE_EPOCH_ID` against its own
//!    `wire.rs` constants and flags a shared magic across crates as a violation.
//!
//! [`WIRE_EPOCH_ID`]: continuum_kernel_core::receipt::WIRE_EPOCH_ID
//!
//! # The architectural guard and its mutants (already landed, cited not re-implemented)
//!
//! The bone's delivery checklist asks for "an executable architectural guard and
//! permanent regression" plus "a mutant that would violate it and prove the guard
//! detects it". For the native-checker half of INV-014 this already exists and is
//! wired into `just check`:
//!
//! - `tools/check_kernel_covenant.py` KCOV-09, self-tested (`--self-test`) against four
//!   violating fixtures under `tools/kernel-covenant/fixtures/kcov-09-*/`
//!   (unqualified `WIRE_EPOCH_ID`, magic collision between two crates, the checker epoch
//!   written into `epochs` instead of `checker.version`, and an unknown field on a
//!   receipt);
//! - each kernel crate's own `receipt.rs` unit tests, including
//!   `every_seam_field_is_shape_checked` and
//!   `a_receipt_without_a_reproduction_command_is_refused` — the exact "does a receipt
//!   refuse emission when a seam field is absent or malformed" candidate this bone's
//!   brief named — present, byte-identically in structure, in **all four** kernel
//!   crates' `receipt.rs` (`ReceiptError::MalformedSeamField` / `ReceiptIdNotWellFormed`).
//!   No gap there: this file does not duplicate it.
//!
//! # What was genuinely missing, and what this file adds
//!
//! Every existing check above is scoped to *one* crate, *one* tool invocation, or *one*
//! artifact pair. Nothing previously cross-checked the pins against each other in one
//! place, and one drift was live and undetected:
//!
//! **Finding.** `lean/AxiomManifest.lean` writes the Lean version into both checked-in
//! artifacts as a hand-typed string literal (`"leanprover/lean4:v4.32.1"`, twice — the
//! JSON's `leanToolchain` field and the transcript's `# toolchain:` header) rather than
//! reading `lean/lean-toolchain`, the file elan/lake actually use to select a Lean
//! version. `axiom-manifest.sh --check` only diffs a *regeneration* of that same
//! generator against the checked-in files, so it would still pass if `lean-toolchain`
//! were bumped and the two literals were not — the "Lean version \[...\] recorded" claim
//! would keep passing every gate while naming last epoch's toolchain. This is a real,
//! previously-uncaught gap in "Lean version \[...\] recorded", not a defect in the
//! generator's logic; nothing here edits `lean/AxiomManifest.lean` (`src`-equivalent
//! tooling is out of scope for this bone; a fix belongs with whoever next touches the
//! generator).
//!
//! The tests below (all `positive_*`) close that gap with a direct byte comparison
//! between `lean/lean-toolchain` and both recorded copies, and extend the same
//! belt-and-suspenders treatment to the other five controls: the empty library closure,
//! the axiom manifest's JSON/transcript agreement, the certificate schema's `checker`
//! and `lean` block shapes (including the previously-unexercised conditional
//! `lean-theorem` branch), and cross-crate `WIRE_EPOCH_ID`/magic distinctness read live
//! from all four crates' own sources in one place rather than asserted per-crate. Two
//! `negative_*` tests are mutants proving the comparison functions this file relies on
//! (`pins_agree`, `BTreeSet` insertion) can actually fail, per the delivery checklist.
//!
//! # Deferred, not a gap in this bone's scope
//!
//! `proof-receipt.schema.json`'s `lean` block (`version`, `theorems`, `imports`,
//! `axioms`, optional `environment_hash`) is exactly the schema-side home for "Lean
//! version, library closure, theorem hashes, axioms" *as a receipt*, but no code
//! anywhere in the repository emits or checks a `lean-theorem`-kind receipt today —
//! there is no `finite-closure.proof-receipt.json`-style example for it, and no Rust
//! crate produces one. This is not a silent omission: it is `bn-3ie` ("Proof receipts
//! with theorem and axiom manifests", Phase D, `req:pr-28`), which depends on PR 28 (the
//! isolated Lean proof service, RFC 0035) opening in Phase D. bn-dw81 depends only on
//! `bn-3af` (PR 4a, Lean environment and seed theorems) and `bn-6tv` (PR 9, trusted
//! kernel crates), both already delivered; PR 28 has not started. Per the T02/bn-2res
//! precedent, a control that is a documented, not-yet-reached deferral is reported, not
//! patched, and the invariant's plan.md heading is not annotated `(delivered: ...)`
//! until that successor lands too.
//!
//! # House rules, inherited from the PR-9/T02 evidence files
//!
//! - `src/` is not touched (Lean or Rust), and no existing test in any crate is touched.
//! - Existing guards (KCOV-09, the four crates' `receipt.rs` unit tests, the dossier
//!   schema validation) are cited, not re-implemented.
//! - This file reads real repository files at compile time via `include_str!`
//!   (`crates/continuum-intent/tests/*.rs` already reaches three directories up into
//!   `notes/plan/schemas/` the same way); nothing here shells out, reads a clock, or
//!   touches the network.

use std::collections::BTreeSet;

// --- fixtures: real files, read at compile time ----------------------------------------

const LEAN_TOOLCHAIN: &str = include_str!("../../../lean/lean-toolchain");
const LAKEFILE_TOML: &str = include_str!("../../../lean/lakefile.toml");
const LAKE_MANIFEST: &str = include_str!("../../../lean/lake-manifest.json");
const AXIOM_MANIFEST_JSON: &str = include_str!("../../../lean/artifacts/axiom-manifest-t0-t1.json");
const AXIOM_MANIFEST_TXT: &str = include_str!("../../../lean/artifacts/axiom-manifest-t0-t1.txt");
const PROOF_RECEIPT_SCHEMA: &str =
    include_str!("../../../notes/plan/schemas/proof-receipt.schema.json");

#[derive(Clone, Copy)]
struct KernelCrate {
    name: &'static str,
    wire_src: &'static str,
    receipt_src: &'static str,
    golden: &'static str,
}

const CORE: KernelCrate = KernelCrate {
    name: "continuum-kernel-core",
    wire_src: include_str!("../src/wire.rs"),
    receipt_src: include_str!("../src/receipt.rs"),
    golden: include_str!("../tests/receipt.golden.json"),
};

const SAT: KernelCrate = KernelCrate {
    name: "continuum-kernel-sat",
    wire_src: include_str!("../../continuum-kernel-sat/src/wire.rs"),
    receipt_src: include_str!("../../continuum-kernel-sat/src/receipt.rs"),
    golden: include_str!("../../continuum-kernel-sat/tests/receipt.golden.json"),
};

const SMT: KernelCrate = KernelCrate {
    name: "continuum-kernel-smt",
    wire_src: include_str!("../../continuum-kernel-smt/src/wire.rs"),
    receipt_src: include_str!("../../continuum-kernel-smt/src/receipt.rs"),
    golden: include_str!("../../continuum-kernel-smt/tests/receipt.golden.json"),
};

const TEMPORAL: KernelCrate = KernelCrate {
    name: "continuum-kernel-temporal",
    wire_src: include_str!("../../continuum-kernel-temporal/src/wire.rs"),
    receipt_src: include_str!("../../continuum-kernel-temporal/src/receipt.rs"),
    golden: include_str!("../../continuum-kernel-temporal/tests/receipt.golden.json"),
};

const KERNEL_CRATES: [KernelCrate; 4] = [CORE, SAT, SMT, TEMPORAL];

// --- tiny anchor-based extraction, matching the style of check_kernel_covenant.py's own
//     regex reads and receipt.rs's own `.contains()` assertions; no parser dependency ---

/// The slice of `source` starting right after the first occurrence of `anchor`.
fn after<'a>(source: &'a str, anchor: &str) -> &'a str {
    let start = source.find(anchor).expect(anchor);
    &source[start + anchor.len()..]
}

/// The quoted string value immediately following `anchor` (which must already include
/// the opening quote).
fn quoted<'a>(source: &'a str, anchor: &str) -> &'a str {
    let rest = after(source, anchor);
    let end = rest.find('"').expect("closing quote");
    &rest[..end]
}

/// The trimmed text immediately following `anchor`, up to (not including) `stop`.
fn until<'a>(source: &'a str, anchor: &str, stop: char) -> &'a str {
    let rest = after(source, anchor);
    let end = rest.find(stop).expect("stop character");
    rest[..end].trim()
}

/// `Err` naming the two values when a supposedly-shared pin has drifted, `Ok` when they
/// agree. Every positive check below calls this and expects `Ok`; a dedicated negative
/// test calls it with values known to differ and expects `Err`, so the comparison is
/// proven capable of failing rather than passing vacuously (delivery checklist: "a
/// mutant that would violate it and prove the guard detects it").
fn pins_agree(label: &str, a: &str, b: &str) -> Result<(), String> {
    if a == b {
        Ok(())
    } else {
        Err(format!("{label} disagree: {a:?} vs {b:?}"))
    }
}

// --- control 1: Lean version -------------------------------------------------------------

#[test]
fn positive_lean_version_in_the_axiom_manifest_matches_the_pinned_toolchain() {
    let pinned = LEAN_TOOLCHAIN.trim();
    let json_recorded = quoted(AXIOM_MANIFEST_JSON, "\"leanToolchain\": \"");
    let txt_recorded = until(AXIOM_MANIFEST_TXT, "# toolchain: ", '\n');

    pins_agree(
        "lean/lean-toolchain vs axiom-manifest-t0-t1.json's leanToolchain",
        pinned,
        json_recorded,
    )
    .expect(
        "bn-dw81 finding: the axiom manifest's recorded Lean version is a string \
         literal hand-written into lean/AxiomManifest.lean, not read from \
         lean/lean-toolchain (the file elan/lake actually use to select a Lean \
         version) — nothing before this test checked the two against each other",
    );
    pins_agree(
        "lean/lean-toolchain vs axiom-manifest-t0-t1.txt's toolchain header",
        pinned,
        txt_recorded,
    )
    .expect("same finding: the transcript carries its own separate copy of the literal");
}

#[test]
fn negative_a_lean_version_mismatch_is_not_silently_accepted() {
    let mutant = pins_agree(
        "mutant toolchain pin",
        "leanprover/lean4:v4.32.1",
        "leanprover/lean4:v4.33.0",
    );
    assert!(
        mutant.is_err(),
        "a drifted Lean version pin must be reported, never accepted"
    );
}

// --- control 2: library closure -----------------------------------------------------------

#[test]
fn positive_library_closure_is_recorded_as_empty_and_nothing_requires_a_package() {
    assert!(
        LAKE_MANIFEST.contains("\"packages\": []"),
        "lean/lake-manifest.json is the checked-in Lean package-dependency closure \
         (RFC 0035 worker key 'library closure'); the metatheory is self-contained by \
         design (lean/AxiomManifest.lean: 'nothing in the metatheory may depend on it')"
    );
    assert!(
        !LAKEFILE_TOML.contains("[[require]]"),
        "no lakefile package requirement exists to disagree with the empty manifest"
    );
}

// --- control 3: theorem hashes / axioms ----------------------------------------------------

#[test]
fn positive_axiom_manifest_json_and_transcript_agree_on_the_zero_axiom_claim() {
    let theorem_count: u32 = until(AXIOM_MANIFEST_JSON, "\"theoremCount\": ", ',')
        .parse()
        .expect("theoremCount is an integer");
    let with_axioms: u32 = until(AXIOM_MANIFEST_JSON, "\"theoremsWithAxioms\": ", ',')
        .parse()
        .expect("theoremsWithAxioms is an integer");
    assert!(
        theorem_count > 0,
        "the RFC 0012 T0/T1 ladder names at least one theorem"
    );
    assert_eq!(
        with_axioms, 0,
        "PR-4A's exit criterion is empty axiom manifests (RFC 0012 ladder; ADR-0035)"
    );

    let depends_lines = AXIOM_MANIFEST_TXT.matches("depends on axioms:").count();
    let clean_lines = AXIOM_MANIFEST_TXT
        .matches("does not depend on any axioms")
        .count();
    assert_eq!(
        depends_lines, with_axioms as usize,
        "the human-auditable transcript must agree with the JSON's axiom count"
    );
    assert_eq!(
        clean_lines as u32, theorem_count,
        "the human-auditable transcript must agree with the JSON's theorem count"
    );
}

// --- control 4: certificate schema ---------------------------------------------------------

#[test]
fn positive_certificate_schema_pins_the_checker_and_lean_block_shapes() {
    assert!(
        PROOF_RECEIPT_SCHEMA.contains(
            "\"required\": [\"name\", \"version\", \"source_hash\", \"binary_hash\", \"toolchain\"]"
        ),
        "checker identity's five required fields (INV-014's 'checker identity')"
    );
    assert!(
        PROOF_RECEIPT_SCHEMA
            .contains("\"required\": [\"version\", \"theorems\", \"imports\", \"axioms\"]"),
        "the lean block's four required fields cover 'Lean version', 'theorem hashes' \
         (theorems), 'library closure' (imports) and 'axioms' in one place"
    );
    assert!(
        PROOF_RECEIPT_SCHEMA.contains("\"game-strategy\", \"lean-theorem\""),
        "lean-theorem is a certificate.kind the schema admits"
    );
    assert!(
        PROOF_RECEIPT_SCHEMA.contains("\"then\": { \"required\": [\"lean\"] }"),
        "a lean-theorem certificate's receipt is required to carry the lean block — the \
         conditional branch `tools/validate_dossier.py`'s SCHEMA_PAIRS never exercises, \
         since its one example instance is a closed-set certificate"
    );
}

// --- control 5: checker identity, cross-crate ------------------------------------------------

#[test]
fn positive_all_four_kernel_crates_have_distinct_qualified_wire_epoch_ids() {
    let mut magics = BTreeSet::new();
    let mut ids = BTreeSet::new();
    for kc in KERNEL_CRATES {
        let magic = quoted(kc.wire_src, "pub const MAGIC: [u8; 8] = *b\"");
        let epoch = until(kc.wire_src, "pub const WIRE_EPOCH: u16 = ", ';');
        let declared_id = quoted(kc.receipt_src, "pub const WIRE_EPOCH_ID: &str = \"");
        let expected_id = format!("{}/{magic}/{epoch}", kc.name);

        assert_eq!(
            declared_id, expected_id,
            "{}'s WIRE_EPOCH_ID must name its own crate, magic and epoch",
            kc.name
        );
        assert!(
            magics.insert(magic),
            "magic {magic:?} reused by more than one kernel crate"
        );
        assert!(
            ids.insert(declared_id),
            "WIRE_EPOCH_ID {declared_id:?} reused by more than one kernel crate"
        );
    }
    assert_eq!(magics.len(), KERNEL_CRATES.len());
    assert_eq!(ids.len(), KERNEL_CRATES.len());
}

#[test]
fn negative_a_duplicated_wire_epoch_id_is_not_silently_accepted() {
    let mut ids = BTreeSet::new();
    assert!(ids.insert("continuum-kernel-core/CONTCERT/1"));
    assert!(
        !ids.insert("continuum-kernel-core/CONTCERT/1"),
        "a repeated WIRE_EPOCH_ID must be rejected by the same distinctness check the \
         positive test relies on"
    );
}

#[test]
fn positive_golden_receipts_name_their_own_crate_and_carry_the_matching_wire_marker() {
    for kc in KERNEL_CRATES {
        let magic = quoted(kc.wire_src, "pub const MAGIC: [u8; 8] = *b\"");
        let epoch = until(kc.wire_src, "pub const WIRE_EPOCH: u16 = ", ';');
        assert!(
            kc.golden.contains(&format!("\"name\": \"{}\"", kc.name)),
            "{}'s golden receipt must name itself as checker",
            kc.name
        );
        assert!(
            kc.golden.contains(&format!("+wire.{magic}.{epoch}")),
            "{}'s golden receipt version must carry its own qualified wire epoch",
            kc.name
        );
    }
}
