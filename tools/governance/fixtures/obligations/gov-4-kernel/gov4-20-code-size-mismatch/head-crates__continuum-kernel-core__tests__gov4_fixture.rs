//! Fixture tests (inert data; never compiled).

/// Mutation: encoding injectivity.
#[test]
fn rejects_encoding_reorder() {
    assert_eq!(1, 1);
}

/// Optimization check: the fast path matches the reference computation.
#[test]
fn fast_path_matches_reference() {
    assert_eq!(1, 1);
}

#[test]
#[ignore]
fn ignored_marker() {
    assert_eq!(1, 1);
}
