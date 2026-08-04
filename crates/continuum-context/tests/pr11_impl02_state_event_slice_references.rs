//! PR-11 / IMPL-02 evidence: state and event slice references —
//! `continuum-context`'s `selected[].kind` in `{event, state_delta}` (RFC 0028,
//! "Required fields, reconciled with plan §6.2", the "causal core and surrounding
//! frontier" and "concrete and abstract state deltas" rows).
//!
//! Linked as a separate crate (`continuum_context::*`, not `crate::*`), the same
//! discipline `continuum-evidence`'s own PR 7 suite states for itself, and the same
//! discipline `pr11_impl03_source_model_references.rs` already applies to this
//! crate's sibling kinds: this is evidence about the client's view of the crate, not
//! about its interior.
//!
//! | Clause | Test |
//! |---|---|
//! | an event reference projects to `kind = event`, carrying the causal-execution-graph artifact only when one was supplied and only when it is the right class | `an_event_reference_projects_with_the_right_artifact_discipline` |
//! | a state-delta reference projects to `kind = state_delta`, always with `artifact: null`, and refuses a delta naming no change | `a_state_delta_reference_projects_cleanly_and_refuses_a_no_op_delta` |
//! | two distinct references never collide into one canonical encoding (anti-vacuity: the encoding is not constant) | `distinct_references_have_distinct_canonical_encodings` |
//! | a fixed compile is byte-identical across independent construction (docs/19 §7 determinism) | `identical_references_are_byte_identical_across_independent_builds` |
//! | `selected[]`'s six list-valued-field determinism obligation is satisfiable: items sort into one stable, content-derived order | `items_sort_into_one_stable_content_derived_order` |
//! | every emitted item round-trips through the shared canonical-JSON reader (`continuum_intent::canonical_json`) with no drift | `every_emitted_item_round_trips_through_the_canonical_json_reader` |

use continuum_context::event::{EventRef, EventRole};
use continuum_context::selection::SelectionKind;
use continuum_context::state_delta::{StateDeltaClass, StateDeltaRef};
use continuum_intent::canonical_json::Json;
use continuum_value::value::{Name, Value};
use continuum_workspace::artifact_path::{ArtifactClass, ArtifactHandle};

fn name(text: &str) -> Name {
    Name::new(text).expect("well-formed test name")
}

#[test]
fn an_event_reference_projects_with_the_right_artifact_discipline() {
    let bare = EventRef::new(name("e7"), EventRole::Observed).into_selected_item(name("e_bare"));
    assert_eq!(bare.kind(), SelectionKind::Event);
    assert_eq!(bare.artifact(), None);

    let handle = ArtifactHandle::new(ArtifactClass::CausalExecutionGraph, "run1")
        .expect("well-formed causal-execution-graph handle");
    let with_graph =
        EventRef::with_execution_graph(name("e3"), EventRole::CausalPredecessor, handle.clone())
            .expect("a causal-execution-graph handle is accepted")
            .into_selected_item(name("e_graph"));
    assert_eq!(with_graph.kind(), SelectionKind::Event);
    assert_eq!(with_graph.artifact(), Some(&handle));

    // A handle of the wrong class is refused outright — never silently accepted and
    // never silently dropped to `None`.
    let wrong_class = ArtifactHandle::new(ArtifactClass::Crashpack, "cp1").expect("well-formed");
    assert!(EventRef::with_execution_graph(name("e7"), EventRole::Observed, wrong_class).is_err());
}

#[test]
fn a_state_delta_reference_projects_cleanly_and_refuses_a_no_op_delta() {
    let delta = StateDeltaRef::new(
        name("acknowledged"),
        StateDeltaClass::Abstract,
        Some(Value::Bool(false)),
        Value::Bool(true),
    )
    .expect("a real change")
    .into_selected_item(name("e_delta"));
    assert_eq!(delta.kind(), SelectionKind::StateDelta);
    // A state delta never carries a standalone artifact (see the module
    // documentation's "What is declined").
    assert_eq!(delta.artifact(), None);
    assert_eq!(
        delta.summary(),
        "abstract state delta: `acknowledged` false \u{2192} true"
    );

    let no_op = StateDeltaRef::new(
        name("acknowledged"),
        StateDeltaClass::Abstract,
        Some(Value::Bool(true)),
        Value::Bool(true),
    );
    assert!(no_op.is_err(), "a delta naming no change must be refused");
}

#[test]
fn distinct_references_have_distinct_canonical_encodings() {
    let a = EventRef::new(name("e7"), EventRole::Observed).into_selected_item(name("x"));
    let b = EventRef::new(name("e7"), EventRole::CausalPredecessor).into_selected_item(name("x"));
    assert_ne!(a.to_canonical_bytes(), b.to_canonical_bytes());

    let c = StateDeltaRef::new(
        name("acknowledged"),
        StateDeltaClass::Abstract,
        Some(Value::Bool(false)),
        Value::Bool(true),
    )
    .expect("a real change")
    .into_selected_item(name("x"));
    let d = StateDeltaRef::new(
        name("durable"),
        StateDeltaClass::Abstract,
        Some(Value::Bool(false)),
        Value::Bool(true),
    )
    .expect("a real change")
    .into_selected_item(name("x"));
    assert_ne!(c.to_canonical_bytes(), d.to_canonical_bytes());

    // An event item and a state-delta item sharing every other field still differ,
    // because `kind` is part of the canonical encoding.
    let e = EventRef::new(name("shared"), EventRole::Observed).into_selected_item(name("shared"));
    let f = StateDeltaRef::new(
        name("shared"),
        StateDeltaClass::Abstract,
        None,
        Value::Bool(true),
    )
    .expect("a real change")
    .into_selected_item(name("shared"));
    assert_ne!(e.to_canonical_bytes(), f.to_canonical_bytes());
}

#[test]
fn identical_references_are_byte_identical_across_independent_builds() {
    // Two independently constructed values, sharing no state — the same discipline
    // docs/19 §7's determinism matrix applies to a compiled artifact.
    let build_event =
        || EventRef::new(name("e7"), EventRole::Observed).into_selected_item(name("e_x"));
    assert_eq!(
        build_event().to_canonical_bytes(),
        build_event().to_canonical_bytes()
    );

    let build_delta = || {
        StateDeltaRef::new(
            name("acknowledged"),
            StateDeltaClass::Abstract,
            Some(Value::Bool(false)),
            Value::Bool(true),
        )
        .expect("a real change")
        .into_selected_item(name("e_y"))
    };
    assert_eq!(
        build_delta().to_canonical_bytes(),
        build_delta().to_canonical_bytes()
    );
}

#[test]
fn items_sort_into_one_stable_content_derived_order() {
    // RFC 0028, "Pack identity, lineage, and determinism": the six list-valued fields
    // (`selected` among them) "MUST be deterministically ordered by content identity
    // or by a declared sort key … List order is part of the canonical encoding". This
    // crate does not own the pack-level packing stage (stage 10, IMPL-06), but the
    // items it hands that stage already carry a total, content-derived order two
    // independent sorts agree on.
    let mut first = [
        StateDeltaRef::new(
            name("acknowledged"),
            StateDeltaClass::Abstract,
            Some(Value::Bool(false)),
            Value::Bool(true),
        )
        .expect("a real change")
        .into_selected_item(name("c")),
        EventRef::new(name("e1"), EventRole::Observed).into_selected_item(name("a")),
        EventRef::new(name("e2"), EventRole::Observed).into_selected_item(name("b")),
    ];
    let mut second = [
        EventRef::new(name("e2"), EventRole::Observed).into_selected_item(name("b")),
        StateDeltaRef::new(
            name("acknowledged"),
            StateDeltaClass::Abstract,
            Some(Value::Bool(false)),
            Value::Bool(true),
        )
        .expect("a real change")
        .into_selected_item(name("c")),
        EventRef::new(name("e1"), EventRole::Observed).into_selected_item(name("a")),
    ];
    first.sort();
    second.sort();
    let first_bytes: Vec<Vec<u8>> = first.iter().map(|item| item.to_canonical_bytes()).collect();
    let second_bytes: Vec<Vec<u8>> = second
        .iter()
        .map(|item| item.to_canonical_bytes())
        .collect();
    assert_eq!(
        first_bytes, second_bytes,
        "one content, one order, regardless of build order"
    );
}

#[test]
fn every_emitted_item_round_trips_through_the_canonical_json_reader() {
    let items = vec![
        EventRef::new(name("e7"), EventRole::Observed).into_selected_item(name("e_bare")),
        EventRef::with_execution_graph(
            name("e3"),
            EventRole::CausalPredecessor,
            ArtifactHandle::new(ArtifactClass::CausalExecutionGraph, "run1").expect("ok"),
        )
        .expect("accepted")
        .into_selected_item(name("e_graph")),
        StateDeltaRef::new(
            name("acknowledged"),
            StateDeltaClass::Abstract,
            Some(Value::Bool(false)),
            Value::Bool(true),
        )
        .expect("a real change")
        .into_selected_item(name("e_delta")),
    ];

    for item in items {
        let bytes = item.to_canonical_bytes();
        let parsed = Json::parse(&bytes).expect("canonical JSON parses");
        let fields = parsed.as_object().expect("object");
        assert_eq!(fields.len(), 4, "exactly {{artifact, id, kind, summary}}");
        assert_eq!(fields["id"].as_str(), Some(item.id().as_str()));
        assert_eq!(fields["kind"].as_str(), Some(item.kind().as_wire_str()));
        assert_eq!(fields["summary"].as_str(), Some(item.summary()));
        match item.artifact() {
            Some(handle) => assert_eq!(
                fields["artifact"].as_str(),
                Some(handle.to_string()).as_deref()
            ),
            None => assert!(fields["artifact"].is_null()),
        }
        // Re-writing the parsed document is a fixpoint, matching
        // `continuum_intent::canonical_json`'s own idempotency claim.
        assert_eq!(parsed.to_canonical_bytes(), bytes);
    }
}
