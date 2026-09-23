//! Fixture tests (inert data; never compiled). Lives outside every kernel crate.

#[test]
fn unrelated_marker() {
    assert_eq!(1, 1);
}
