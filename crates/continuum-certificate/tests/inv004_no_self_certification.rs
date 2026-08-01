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
//!   injected dependency is caught.
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
//! `tests/pr9_exit_evidence.rs`), and this crate, still a PR-9-follow-on scaffold with
//! no types and no tests at all, does not. The two checks below close exactly that gap,
//! at exactly this crate's own scope: they assert the zero-dependency baseline this
//! crate ships today by reading its own tracked manifest, the same
//! `include_str!`-of-the-crate's-own-sources idiom
//! `continuum_kernel_core`'s
//! `certificate_has_no_public_constructor_no_public_field_and_no_manual_trait_impl`
//! (`tests/pr9_exit_evidence.rs`) uses for its own type-level claim. They do not
//! reimplement the transitive closure walk, the covenant's line budget, or the
//! rationale/TCB bookkeeping above — those stay owned by the three tools cited, and
//! this file cites rather than duplicates them.
//!
//! No `src/` file is touched or added by this file.

/// This crate's own manifest, read at compile time so the assertions below are about
/// the tracked `Cargo.toml`, not a value this file could drift from.
const MANIFEST: &str = include_str!("../Cargo.toml");

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

#[test]
fn the_manifest_declares_no_dependencies_of_any_kind_today() {
    // `continuum-certificate` is still a documented scaffold (`src/lib.rs`): no types,
    // no [dependencies] table, no [build-dependencies] table, and no
    // [target.*.dependencies] override. A manifest edit that adds ANY dependency —
    // forbidden or not — becomes visible here, in this crate's own test suite, before
    // it can even reach the boundary/covenant/governance walk.
    assert!(
        !MANIFEST.contains("[dependencies]"),
        "continuum-certificate must stay dependency-free while it remains a PR-9 \
         follow-on scaffold; a [dependencies] table appeared in its manifest"
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
