//! PR-11 / IMPL-05 evidence: the replay reference — `continuum-context`'s typed
//! constructor for the pack's top-level `replay` field (RFC 0028, "Required fields,
//! reconciled with plan §6.2", the "exact replay and debugger handles" row;
//! `context-pack.schema.json` `properties.replay`).
//!
//! Linked as a separate crate (`continuum_context::*`, not `crate::*`), the same
//! discipline `continuum-evidence`'s own PR 7 suite states for itself and PR-11 / IMPL-03's
//! own suite follows: this is evidence about the client's view of the crate, not about its
//! interior.
//!
//! | Clause | Test |
//! |---|---|
//! | a replay reference accepts a `crash_*` handle and renders exactly the schema's bare pattern, nothing more | `a_replay_reference_wraps_a_crashpack_handle_and_renders_its_bare_wire_string` |
//! | every artifact class other than `Crashpack` is refused, not silently accepted or silently dropped | `a_replay_reference_refuses_every_class_but_crashpack` |
//! | two distinct references never collide into one canonical encoding (anti-vacuity: the encoding is not constant) | `distinct_references_have_distinct_canonical_encodings` |
//! | a fixed reference is byte-identical across independent construction (docs/19 §7 determinism) | `identical_references_are_byte_identical_across_independent_builds` |
//! | the emitted value round-trips through the shared canonical-JSON reader (`continuum_intent::canonical_json`) with no drift | `the_emitted_value_round_trips_through_the_canonical_json_reader` |
//! | `replay` is not one of `selected[].kind`'s eleven members | `replay_is_not_a_selection_kind` |

use continuum_context::replay::{ReplayRef, ReplayRefError};
use continuum_context::selection::SelectionKind;
use continuum_intent::canonical_json::Json;
use continuum_workspace::artifact_path::{ArtifactClass, ArtifactHandle};

fn crashpack(identity: &str) -> ArtifactHandle {
    ArtifactHandle::new(ArtifactClass::Crashpack, identity).expect("well-formed test handle")
}

#[test]
fn a_replay_reference_wraps_a_crashpack_handle_and_renders_its_bare_wire_string() {
    let handle = crashpack("cp7m3x9");
    let reference = ReplayRef::new(handle.clone()).expect("a crashpack handle is accepted");
    assert_eq!(reference.crashpack(), &handle);
    // Exactly the schema's `^crash_[A-Za-z0-9_-]+$` pattern, with no wrapping object and
    // no second field — the minimal shape the schema and RFC 0028 name for `replay`.
    assert_eq!(
        reference.to_json(),
        Json::String("crash_cp7m3x9".to_owned())
    );
}

#[test]
fn a_replay_reference_refuses_every_class_but_crashpack() {
    for class in ArtifactClass::ALL {
        let handle = ArtifactHandle::new(class, "x1").expect("well-formed test handle");
        let outcome = ReplayRef::new(handle);
        if class == ArtifactClass::Crashpack {
            assert!(outcome.is_ok(), "the one accepted class must be accepted");
        } else {
            assert_eq!(
                outcome,
                Err(ReplayRefError::WrongArtifactClass(class)),
                "`{class}` must be refused, not silently accepted or silently dropped"
            );
        }
    }
}

#[test]
fn distinct_references_have_distinct_canonical_encodings() {
    let a = ReplayRef::new(crashpack("cp7m3x9")).expect("accepted");
    let b = ReplayRef::new(crashpack("cp7m3x8")).expect("accepted");
    assert_ne!(a, b);
    assert_ne!(a.to_canonical_bytes(), b.to_canonical_bytes());
}

#[test]
fn identical_references_are_byte_identical_across_independent_builds() {
    // Two independently constructed values, sharing no state — the same discipline
    // docs/19 §7's determinism matrix applies to a compiled artifact.
    let build = || ReplayRef::new(crashpack("cp7m3x9")).expect("accepted");
    assert_eq!(build(), build());
    assert_eq!(build().to_canonical_bytes(), build().to_canonical_bytes());
}

#[test]
fn the_emitted_value_round_trips_through_the_canonical_json_reader() {
    let reference = ReplayRef::new(crashpack("cp7m3x9")).expect("accepted");
    let bytes = reference.to_canonical_bytes();
    let parsed = Json::parse(&bytes).expect("canonical JSON parses");
    assert_eq!(parsed.as_str(), Some("crash_cp7m3x9"));
    // Re-writing the parsed document is a fixpoint, matching
    // `continuum_intent::canonical_json`'s own idempotency claim.
    assert_eq!(parsed.to_canonical_bytes(), bytes);
}

#[test]
fn replay_is_not_a_selection_kind() {
    // The task-adjacent phrase "replay selection kind" is not RFC 0028's own: the schema's
    // `selected[].kind` enum is eleven members and `replay` is not one of them (RFC 0028,
    // "Selection and the causal core"). `replay` is a top-level pack field instead
    // (`context-pack.schema.json` `properties.replay`), which is why `ReplayRef` has no
    // `into_selected_item` the way `SourceRef`/`ModelActionRef` do.
    assert_eq!(SelectionKind::from_wire_str("replay"), None);
    assert!(
        !SelectionKind::ALL
            .iter()
            .any(|kind| kind.as_wire_str() == "replay")
    );
}
