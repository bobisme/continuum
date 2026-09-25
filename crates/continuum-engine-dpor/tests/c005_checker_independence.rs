//! docs/33: the DPOR reducer is audited by "unreduced differential oracle + reduction
//! witness checker", and "shared decision logic that causes both sides to accept the
//! same bug is not" allowed. This pins, from the source, that the witness checker
//! (`src/checker.rs`) reaches into the crate only for the shared schema — the witness
//! data types and the obligations — and never for the reducer's footprint
//! derivation, its label sets, its search, or its budget.
//!
//! A mutant that makes the checker import `crate::footprint` (to reuse the reducer's
//! conflict relation instead of re-deriving it) fails
//! `the_checker_imports_only_the_shared_schema`.

#![allow(missing_docs, clippy::unwrap_used, clippy::expect_used, clippy::panic)]

const CHECKER: &str = include_str!("../src/checker.rs");

/// The crate paths the checker may name: the witness schema and the obligations.
const ALLOWED: [&str; 2] = ["crate::witness::", "crate::report::Obligations"];

/// The reducer's modules, which the checker must not name at all.
const FORBIDDEN: [&str; 5] = ["footprint", "engine", "bits", "budget", "LabelSet"];

/// The checker's source with comments removed, so prose may mention the reducer.
fn code() -> String {
    CHECKER
        .lines()
        .filter(|line| !line.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn the_checker_imports_only_the_shared_schema() {
    let code = code();
    let mut named = 0;
    for (at, _) in code.match_indices("crate::") {
        let rest = &code[at..];
        assert!(
            ALLOWED.iter().any(|allowed| rest.starts_with(allowed)),
            "the checker names a reducer path: {}",
            rest.lines().next().unwrap()
        );
        named += 1;
    }
    assert!(named >= 2, "the checker names the shared schema");
    for forbidden in FORBIDDEN {
        assert!(!code.contains(forbidden), "the checker names `{forbidden}`");
    }
    // The checker's own tests live in a separate file, which may call the reducer to
    // produce witnesses; the checker file itself only declares that module.
    assert!(code.contains("#[path = \"checker_tests.rs\"]"));
}

#[test]
fn the_checker_derives_its_own_footprints_and_components() {
    let code = code();
    for own in [
        "fn int_reads",
        "fn bool_reads",
        "fn independent",
        "fn fire",
        "fn commute",
        "fn components",
    ] {
        assert!(code.contains(own), "the checker defines {own}");
    }
}
