//! INV-004 pinning evidence for `continuum-certificate` (plan §2, plan.md:319; plan
//! §20; `AGENTS.md`, "No self-certification").
//!
//! > Search code does not check its own strongest claims. Certificates cross an
//! > independent checker; foundational theorems cross Lean.
//! >
//! > — `notes/plan/plan.md:319`
//!
//! # Where the rest of INV-004's enforcement already lives
//!
//! This crate is the certificate-checking side of the invariant (plan §20:
//! "`continuum-certificate` and the `continuum-kernel-*` trusted checking base may not
//! depend on `continuum-engine-*`, `continuum-forge`, or `continuum-asupersync`"), and
//! the transitive, whole-workspace version of that rule is already enforced —
//! mechanically, with a self-test that replays a mutation and asserts it is caught —
//! by three independent tools, none of which this file re-implements:
//!
//! - `tools/check_crate_boundaries.py`'s `certificate-checker-not-search` rule reads
//!   `cargo metadata` and walks the real transitive closure of every crate in
//!   `CHECKER` (the four kernel crates plus this one); its `--self-test` includes the
//!   case `("certificate-checker-not-search", "continuum-certificate",
//!   "continuum-engine-explicit", "direct")` and a transitive variant reached
//!   `via:continuum-value`, both asserted caught.
//! - `tools/check_kernel_covenant.py`'s KCOV-06 (`dependency-freedom`) enforces that
//!   the four `continuum-kernel-*` manifests declare only what plan §20 admits —
//!   today, nothing — with fixture `kcov-06-async-runtime-dependency` proving an
//!   injected dependency is caught. That emptiness is also what fixes the *direction*
//!   of this crate's composition: a kernel crate cannot depend on this one, so the
//!   edge can only run the other way.
//! - `tools/governance/check_code_policy.py`'s GOV-1-06 (`no-network-in-checker`)
//!   walks the same `CHECKER` set (kernel crates plus this one) against
//!   `NETWORK_CAPABLE_MEMBERS` (which includes `continuum-asupersync`); fixture
//!   `gov-1-06-transitive-network-member` overlays *this crate's own manifest* to add
//!   an edge to `continuum-effects-network` through an innocent hop and asserts it is
//!   caught. GOV-1-07 (`dependency-rationale`) additionally requires that any
//!   dependency the checker set links be classified `trusted-checking-base` in
//!   `tools/governance/dependency-rationale.toml`, with fixture
//!   `gov-1-07-checker-dependency-not-tcb` proving a misclassification is caught.
//!
//! All three read `cargo metadata` / the manifests at `just check` time, so they see
//! this crate's *actual* declared dependencies on every run. What none of them is is a
//! test *inside this crate*, independent of those external tools being invoked at all —
//! every sibling crate in the trusted checking base has one (`continuum-kernel-core`,
//! `-sat`, `-smt`, `-temporal` each carry `tests/wire_form_boundary.rs` and
//! `tests/pr9_exit_evidence.rs`). The checks below close exactly that gap, at exactly
//! this crate's own scope, by reading its own tracked manifest and its own tracked
//! sources — the same `include_str!`-of-the-crate's-own-sources idiom
//! `continuum_kernel_core`'s
//! `certificate_has_no_public_constructor_no_public_field_and_no_manual_trait_impl`
//! (`tests/pr9_exit_evidence.rs`) uses for its own type-level claim. They do not
//! reimplement the transitive closure walk, the covenant's line budget, or the
//! rationale/TCB bookkeeping above — those stay owned by the three tools cited, and
//! this file cites rather than duplicates them.
//!
//! # What moved when the crate stopped being a scaffold
//!
//! The first check below used to read "the manifest declares no dependencies of any
//! kind today", and its own comment scoped it: "while it remains a PR-9 follow-on
//! scaffold". Delivering the crate's plan §20 responsibility is exactly the event that
//! assertion existed to make visible, and it did: adding the four kernel edges turned it
//! red. It is replaced here by the stronger form of the same claim — an *allow-list* of
//! exactly the four `continuum-kernel-*` crates, so any fifth dependency, forbidden or
//! not, is still caught in this crate's own suite before it can reach the
//! boundary/covenant/governance walk. The second check is unchanged.
//!
//! The behavioural half of INV-004 — the composition really is checked from wire bytes
//! by an independent kernel — is `tests/family_routing.rs`.

/// This crate's own manifest, read at compile time so the assertions below are about
/// the tracked `Cargo.toml`, not a value this file could drift from.
const MANIFEST: &str = include_str!("../Cargo.toml");

const LIB_SRC: &str = include_str!("../src/lib.rs");
const CHECK_SRC: &str = include_str!("../src/check.rs");
const FAMILY_SRC: &str = include_str!("../src/family.rs");
const VERDICT_SRC: &str = include_str!("../src/verdict.rs");

/// Every source file this crate's public surface could hide something in. A module
/// added to `src/` and not to this list is a module these checks cannot see — the same
/// self-consistency worry `continuum-kernel-core`'s `pr9_exit_evidence.rs` names about
/// its own `CRATE_SOURCES`.
const CRATE_SOURCES: &[(&str, &str)] = &[
    ("lib.rs", LIB_SRC),
    ("check.rs", CHECK_SRC),
    ("family.rs", FAMILY_SRC),
    ("verdict.rs", VERDICT_SRC),
];

/// The four crates plan §20 admits into this crate's dependency set: the trusted
/// checking base it composes, and nothing else.
const ADMITTED_DEPENDENCIES: &[&str] = &[
    "continuum-kernel-core",
    "continuum-kernel-sat",
    "continuum-kernel-smt",
    "continuum-kernel-temporal",
];

/// The three families plan §20 forbids in the certificate checker's dependency
/// closure: search engines, Forge, and asupersync. Matched by name prefix/substring
/// directly against the manifest text, deliberately independent of TOML parsing so
/// this check has no shared code with `tools/check_crate_boundaries.py`'s own
/// `cargo_metadata`/`build_graph` (docs/03 §8: "diversity against common-mode bugs").
const FORBIDDEN_DEPENDENCY_SUBSTRINGS: &[&str] = &[
    "continuum-engine-",
    "continuum-forge",
    "continuum-asupersync",
];

/// The dependency names declared under `[dependencies]`, read as text.
fn declared_dependencies(manifest: &str) -> Vec<String> {
    let mut names = Vec::new();
    let mut inside = false;
    for line in manifest.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            inside = trimmed == "[dependencies]";
            continue;
        }
        if !inside || trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let (name, _) = trimmed
            .split_once('=')
            .expect("a dependency line is `name = …`");
        names.push(name.trim().to_owned());
    }
    names
}

/// A source file's lines with comment-only lines removed, so a claim quoted in
/// documentation is never mistaken for the construct it describes.
///
/// The crate uses line comments exclusively; the assertion below keeps that true, since
/// a block comment would let a construct hide from every scan in this file.
fn code_lines(name: &str, source: &str) -> Vec<String> {
    assert!(
        !source.contains("/*"),
        "{name} grew a block comment; the scans in this file strip line comments only"
    );
    source
        .lines()
        .filter(|line| !line.trim_start().starts_with("//"))
        .map(str::to_owned)
        .collect()
}

#[test]
fn the_manifest_declares_only_the_four_kernel_crates_and_nothing_else() {
    // Plan §20 places `continuum-certificate` beside the trusted checking base and
    // gives it one dependency rule; this crate composes the four kernel checkers and
    // links nothing else — no fifth workspace crate and no external crate. A manifest
    // edit that adds ANY other dependency becomes visible here, in this crate's own
    // test suite, before it can reach the boundary/covenant/governance walk.
    assert_eq!(
        declared_dependencies(MANIFEST),
        ADMITTED_DEPENDENCIES,
        "continuum-certificate's dependency set is exactly the four continuum-kernel-* \
         crates it composes; anything else is a new entry in the trusted checking base \
         and needs plan §20 to say so first"
    );
    assert!(
        !MANIFEST.contains("[build-dependencies]"),
        "continuum-certificate declared a [build-dependencies] table"
    );
    assert!(
        !MANIFEST.contains("[target."),
        "continuum-certificate declared a target-conditional dependency table"
    );
}

#[test]
fn the_manifest_names_no_forbidden_dependency_even_if_a_table_is_ever_added() {
    // A second, narrower check in case the crate ever legitimately grows a
    // [dependencies] table for something plan §20 admits (a schema/serde crate, say):
    // even then, none of the three forbidden families above may appear anywhere in
    // this file's text. This is INV-004's actual boundary, stated directly rather than
    // inferred from "no dependencies at all".
    for forbidden in FORBIDDEN_DEPENDENCY_SUBSTRINGS {
        assert!(
            !MANIFEST.contains(forbidden),
            "continuum-certificate's manifest names a forbidden dependency: {forbidden:?} \
             (plan §20: the certificate checker may not depend on search)"
        );
    }
}

#[test]
fn checking_has_exactly_one_public_entry_point_and_it_takes_wire_form_bytes() {
    // The mechanical half of `src/lib.rs`'s own claim: `check_certificate` takes
    // `&[u8]` and there is no other way in. A second `pub fn` returning `Outcome`
    // anywhere in the crate would be a second way in, wire-form or not, and this check
    // would catch it whichever file it landed in.
    let mut producers = Vec::new();
    for (name, source) in CRATE_SOURCES {
        for line in code_lines(name, source) {
            let trimmed = line.trim_start();
            if trimmed.starts_with("pub fn") && line.contains("-> Outcome") {
                producers.push(format!("{name}: {}", line.trim()));
            }
        }
    }
    assert_eq!(
        producers,
        vec!["check.rs: pub fn check_certificate(bytes: &[u8]) -> Outcome {".to_owned()],
        "the crate must expose exactly one public route to an Outcome, and it must take \
         wire-form bytes; found {producers:?}"
    );
}

#[test]
fn no_source_in_this_crate_names_a_decoded_kernel_structure() {
    // > a certificate is checked from its wire form, never from shared memory
    // >   — notes/plan/plan.md §20, "Dependency rules"
    //
    // The serialization boundary is preserved *through* this composition, not merely at
    // its far end. This crate links the four kernels, so it could reach for a decoded
    // `wire::Certificate`, call `wire::decode`, or handle a `DecodeFailure` and decide
    // for itself — and then the artifact would have been interpreted twice, the second
    // time by a crate with no covenant over it. The only thing this crate may take from
    // a kernel's `wire` module is the family magic: a byte constant, not a structure.
    let mut wire_items: Vec<String> = Vec::new();
    for (name, source) in CRATE_SOURCES {
        for line in code_lines(name, source) {
            let mut rest = line.as_str();
            while let Some(position) = rest.find("wire::") {
                let tail = rest
                    .get(position.saturating_add("wire::".len())..)
                    .unwrap_or("");
                let item: String = tail
                    .chars()
                    .take_while(|c| c.is_alphanumeric() || *c == '_')
                    .collect();
                wire_items.push(item);
                rest = tail;
            }
        }
    }
    wire_items.sort_unstable();
    wire_items.dedup();
    assert_eq!(
        wire_items,
        vec!["MAGIC".to_owned()],
        "the only item this crate may take from a kernel's wire module is the family \
         magic; found {wire_items:?}"
    );

    for (name, source) in CRATE_SOURCES {
        for forbidden in [
            "decode(",
            "DecodeFailure",
            "CheckedClaim",
            "Certificate {",
            "Envelope",
        ] {
            for line in code_lines(name, source) {
                assert!(
                    !line.contains(forbidden),
                    "{name} names {forbidden:?}; this crate must never hold a decoded \
                     certificate — only the bytes and the kernel's verdict"
                );
            }
        }
    }
}

#[test]
fn the_crate_carries_the_kernel_no_panic_lint_wall() {
    // This crate sits in front of the trusted checking base, inside docs/12 §11's
    // "malformed artifact panic in kernel" release-blocker class, so it carries the same
    // seven denials plus the crate-level forbid that
    // `tools/check_kernel_covenant.py`'s KCOV-02 requires of the four kernel crates.
    // That checker's scope is the kernel crates only, by its own `KERNEL_CRATES`
    // constant, so this crate's wall is asserted here or nowhere.
    assert!(
        LIB_SRC.contains("#![forbid(unsafe_code)]"),
        "src/lib.rs dropped the crate-level `unsafe_code` forbid"
    );
    for lint in [
        "clippy::indexing_slicing",
        "clippy::unwrap_used",
        "clippy::expect_used",
        "clippy::panic",
        "clippy::todo",
        "clippy::unimplemented",
        "clippy::arithmetic_side_effects",
    ] {
        assert!(
            LIB_SRC.contains(lint),
            "src/lib.rs dropped `{lint}` from the no-panic covenant's lint wall"
        );
    }
}

#[test]
fn no_public_item_collapses_the_four_outcomes_into_a_bool() {
    // Each kernel offers `is_verified` "for assertions and for rendering, never as a
    // substitute for matching". At this layer there are four outcomes rather than three
    // — a routing fault is not a verdict — so one bit is even less able to carry them,
    // and the surface offers none. A caller that wants a boolean writes the collapse
    // itself, where a reviewer can see it; `tests/family_routing.rs` does exactly that,
    // in six visible lines.
    for (name, source) in CRATE_SOURCES {
        for line in code_lines(name, source) {
            assert!(
                !line.contains("-> bool"),
                "{name} grew a boolean-returning method ({}); the verdict vocabulary must \
                 reach the caller intact (INV-008)",
                line.trim()
            );
        }
    }
}
