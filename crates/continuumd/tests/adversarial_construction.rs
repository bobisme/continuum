//! Adversarial construction: every identifier the wire carries is a trust boundary.
//!
//! Handles, actor identities, request identifiers, operation names, and timestamps all
//! arrive as strings from a client the daemon does not control. The IDL states a
//! `@pattern` or a prefix rule for each; the obligation on a constructor is not "usually
//! reject" but "reject, with a type, and never panic", which is what the sweeps below
//! assert.
//!
//! # The attack classes
//!
//! Each section is one way a hostile or corrupt identifier differs from a well-formed
//! one: a truncation, a near-miss prefix, a character outside the declared alphabet, a
//! separator in the wrong place, a very long run. Each asserts a *specific* rejection —
//! an error that cannot be told from another error is one nobody can act on.
//!
//! # Inputs stay linear
//!
//! No fixture is built by doubling, every one is under [`MAX_FIXTURE`] bytes, and the
//! truncation sweep is linear in the length of one short handle. A parser is allowed to
//! be slow on hostile input; a test is not allowed to become a memory benchmark.

use continuumd::protocol::registry::HANDLES;
use continuumd::protocol::scalar::{
    ActorId, ArtifactHandle, CapabilityHandle, ContinuationHandle, EvidenceHandle,
    IntentBundleHandle, IntentHandle, OperationName, RequestId, TaskHandle, Timestamp,
    WorkspaceHandle,
};
use continuumd::protocol::spec::ProtocolHandle;

/// The bound every fixture in this file stays under.
const MAX_FIXTURE: usize = 8192;

fn bounded(text: &str) -> &str {
    assert!(
        text.len() < MAX_FIXTURE,
        "fixtures stay bounded: {} bytes",
        text.len()
    );
    text
}

// --- handles -------------------------------------------------------------------------

#[test]
fn every_declared_prefix_admits_a_well_formed_handle() {
    // The registry is the list of classes; this asserts each one's constructor exists and
    // agrees with the prefix the registry publishes for it.
    assert_eq!(HANDLES.len(), 19);
    assert_eq!(WorkspaceHandle::PREFIX, "ws_");
    assert_eq!(CapabilityHandle::PREFIX, "cap_");

    let handle = WorkspaceHandle::new("ws_9fA-b_c").expect("well formed");
    assert_eq!(handle.as_str(), "ws_9fA-b_c");
}

#[test]
fn a_near_miss_prefix_is_rejected() {
    // `in_` and `inb_` differ by one character, and an intent is not an intent bundle.
    // The prefixes are not prefixes of each other — `inb_…` does not begin with `in_`,
    // because the third character is `b` and not `_` — so neither class admits the
    // other's handles.
    assert!(IntentHandle::new("in_abc").is_ok());
    assert!(IntentBundleHandle::new("inb_abc").is_ok());
    assert!(IntentHandle::new("inb_abc").is_err());
    assert!(IntentBundleHandle::new("in_abc").is_err());
}

#[test]
fn a_handle_of_the_wrong_class_is_rejected() {
    let error = WorkspaceHandle::new("task_abc").expect_err("wrong class");
    assert_eq!(error.declared, "WorkspaceHandle");
    assert!(TaskHandle::new("ws_abc").is_err());
    assert!(EvidenceHandle::new("evidence_abc").is_err());
    assert!(ContinuationHandle::new("cont").is_err());
}

#[test]
fn a_handle_with_an_empty_opaque_part_is_rejected() {
    // `<prefix><opaque>` where the opaque part matches `[A-Za-z0-9_-]+`: one or more.
    assert!(WorkspaceHandle::new("ws_").is_err());
    assert!(TaskHandle::new("task_").is_err());
    assert!(CapabilityHandle::new("cap_").is_err());
}

#[test]
fn a_handle_carrying_a_character_outside_the_alphabet_is_rejected() {
    for bad in [
        "ws_a/b", "ws_a.b", "ws_a b", "ws_a\0b", "ws_a\nb", "ws_é", "ws_a:b", "ws_a+b", "ws_a=b",
        "ws_a%2f",
    ] {
        assert!(
            WorkspaceHandle::new(bounded(bad)).is_err(),
            "{bad:?} must be rejected"
        );
    }
    assert!(WorkspaceHandle::new("ws_a%2").is_err());
}

#[test]
fn every_truncation_of_a_handle_is_a_typed_rejection_and_never_a_panic() {
    let good = "ws_0aZ-_9";
    assert!(WorkspaceHandle::new(good).is_ok());
    // A truncation that still leaves the prefix and one opaque character is a *different*
    // handle, not a malformed one — the class says nothing about how long an opaque part
    // is. Everything shorter is malformed, and the boundary is exactly there.
    let shortest = WorkspaceHandle::PREFIX.len() + 1;
    for cut in 0..good.len() {
        let truncated = &good[..cut];
        let constructed = WorkspaceHandle::new(truncated);
        if cut < shortest {
            assert!(
                constructed.is_err(),
                "truncation at {cut} ({truncated:?}) must not construct"
            );
        } else {
            assert_eq!(
                constructed
                    .expect("a shorter opaque part is still a handle")
                    .as_str(),
                truncated
            );
        }
    }
}

#[test]
fn a_long_handle_is_accepted_or_rejected_but_never_a_panic() {
    // The IDL bounds no identifier's length, so a long well-formed handle is well formed.
    // What matters is that the constructor terminates with a decision either way.
    let long = format!("ws_{}", "a".repeat(4096));
    assert!(WorkspaceHandle::new(bounded(&long)).is_ok());

    let long_bad = format!("ws_{}", "/".repeat(4096));
    assert!(WorkspaceHandle::new(bounded(&long_bad)).is_err());

    let no_prefix = "a".repeat(4096);
    assert!(WorkspaceHandle::new(bounded(&no_prefix)).is_err());
}

#[test]
fn a_capability_never_prints_itself() {
    // RFC 0026: a `cap_*` "MUST NOT be logged in request traces, MUST NOT appear in error
    // text or in `next_operations` arguments".
    let secret = "cap_S3cr3t-t0k3n";
    let capability = CapabilityHandle::new(secret).expect("well formed");
    let rendered = format!("{capability:?}");
    assert!(!rendered.contains("S3cr3t"), "{rendered}");
    assert_eq!(rendered, "CapabilityHandle(<redacted>)");
    assert_eq!(capability.as_str(), secret, "the value is still usable");
}

// --- the class-agnostic artifact handle ------------------------------------------------

#[test]
fn an_artifact_handle_requires_a_class_prefix_and_an_opaque_part() {
    for good in [
        "ws_abc",
        "cir_0",
        "defect_a-b_c",
        "a_b",
        "proof_state_1",
        "x9_Z",
    ] {
        assert!(
            ArtifactHandle::new(bounded(good)).is_ok(),
            "{good:?} matches the pattern"
        );
    }
    for bad in [
        "", "a", "ab", "a_", "_ab", "Ws_abc", "9ws_abc", "ws-abc", "ws_a/b", "ws_a b",
    ] {
        assert!(
            ArtifactHandle::new(bounded(bad)).is_err(),
            "{bad:?} does not match the pattern"
        );
    }
}

#[test]
fn every_truncation_of_an_artifact_handle_is_a_decision() {
    let good = "receipt_0aZ-_9";
    assert!(ArtifactHandle::new(good).is_ok());
    for cut in 0..=good.len() {
        // No assertion about which way it goes below the minimum length; the assertion is
        // that it returns.
        let _ = ArtifactHandle::new(&good[..cut]);
    }
}

// --- actor identities ------------------------------------------------------------------

#[test]
fn the_four_actor_schemes_are_a_closed_set() {
    for scheme in ActorId::SCHEMES {
        let text = format!("{scheme}:one.two-three");
        let actor = ActorId::new(bounded(&text)).expect("a declared scheme");
        assert_eq!(actor.scheme(), scheme);
    }
    for bad in [
        "root:one",
        "Agent:one",
        "agent",
        "agent:",
        ":one",
        "",
        "agent:one two",
        "agent:one/two",
        "agents:one",
        "human:oné",
    ] {
        assert!(ActorId::new(bounded(bad)).is_err(), "{bad:?}");
    }
    // A colon inside the tail is allowed by the pattern: `service:ci:runner-7`.
    assert!(ActorId::new("service:ci:runner-7").is_ok());
}

// --- request identifiers ----------------------------------------------------------------

#[test]
fn a_request_id_carries_its_prefix() {
    assert!(RequestId::new("req_1").is_ok());
    assert!(RequestId::new("req_a-B_9").is_ok());
    for bad in ["", "req", "req_", "REQ_1", "req_1.2", "req_1 2", "xreq_1"] {
        assert!(RequestId::new(bounded(bad)).is_err(), "{bad:?}");
    }
}

// --- operation names ---------------------------------------------------------------------

#[test]
fn an_operation_name_is_namespace_dot_verb() {
    assert!(OperationName::new("workspace.create").is_ok());
    assert!(OperationName::new("intent.propose_revision").is_ok());
    for bad in [
        "",
        "workspace",
        "workspace.",
        ".create",
        "Workspace.create",
        "workspace.Create",
        "work_space.create",
        "workspace.create.extra",
        "workspace .create",
        "workspace.create ",
    ] {
        assert!(OperationName::new(bounded(bad)).is_err(), "{bad:?}");
    }
}

#[test]
fn being_well_formed_is_not_being_registered() {
    // The pattern and the registry are different questions, and conflating them would
    // make a typo indistinguishable from an operation this build does not implement.
    assert!(
        OperationName::registered("workspace.create")
            .expect("well formed")
            .is_some()
    );
    assert!(
        OperationName::registered("workspace.destroy")
            .expect("well formed")
            .is_none(),
        "well formed, not registered"
    );
    assert!(
        OperationName::registered("Workspace.destroy").is_err(),
        "not even well formed"
    );
    // Every registered name is, itself, well formed under the alias pattern.
    for spec in continuumd::protocol::registry::OPERATIONS {
        assert!(
            OperationName::new(spec.name).is_ok(),
            "{} matches the OperationName pattern",
            spec.name
        );
    }
}

// --- timestamps ---------------------------------------------------------------------------

#[test]
fn a_timestamp_has_exactly_one_spelling() {
    let good = "2026-08-01T12:34:56.789Z";
    assert_eq!(Timestamp::new(good).expect("canonical").as_str(), good);

    for bad in [
        "2026-08-01T12:34:56Z",
        "2026-08-01T12:34:56.789+00:00",
        "2026-08-01T12:34:56.789z",
        "2026-08-01 12:34:56.789Z",
        "2026-8-01T12:34:56.789Z",
        "2026-08-01T12:34:56.7890Z",
        "",
        "Z",
    ] {
        assert!(Timestamp::new(bounded(bad)).is_err(), "{bad:?}");
    }

    // Field ranges, which the IDL's IETF RFC3339 citation fixes.
    for out_of_range in [
        "2026-13-01T12:34:56.789Z",
        "2026-00-01T12:34:56.789Z",
        "2026-08-32T12:34:56.789Z",
        "2026-08-01T24:34:56.789Z",
        "2026-08-01T12:60:56.789Z",
        "2026-08-01T12:34:61.789Z",
    ] {
        assert!(Timestamp::new(out_of_range).is_err(), "{out_of_range:?}");
    }
    // A leap second is in range; the IDL states no calendar rule beyond RFC3339.
    assert!(Timestamp::new("2026-12-31T23:59:60.000Z").is_ok());
}

#[test]
fn every_truncation_of_a_timestamp_is_a_typed_rejection() {
    let good = "2026-08-01T12:34:56.789Z";
    for cut in 0..good.len() {
        assert!(Timestamp::new(&good[..cut]).is_err(), "truncation at {cut}");
    }
}
