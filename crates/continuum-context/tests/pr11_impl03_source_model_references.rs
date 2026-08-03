//! PR-11 / IMPL-03 evidence: source and model references — `continuum-context`'s
//! `selected[].kind` in `{source, model}` (RFC 0028, "Required fields, reconciled with
//! plan §6.2", the "relevant source spans and model actions" row).
//!
//! Linked as a separate crate (`continuum_context::*`, not `crate::*`), the same
//! discipline `continuum-evidence`'s own PR 7 suite states for itself: this is evidence
//! about the client's view of the crate, not about its interior.
//!
//! | Clause | Test |
//! |---|---|
//! | the closed eleven-member `kind` vocabulary matches the schema, and fails closed on an unrecognized token | `the_selection_kind_vocabulary_is_closed_and_fails_closed` |
//! | a source reference names a position, never source text, and projects to `kind = source` with `artifact: null` | `a_source_reference_projects_cleanly_and_carries_no_source_text` |
//! | a model action reference projects to `kind = model`, carrying the elaborated-model artifact only when one was supplied and only when it is the right class | `a_model_action_reference_projects_with_the_right_artifact_discipline` |
//! | two distinct references never collide into one canonical encoding (anti-vacuity: the encoding is not constant) | `distinct_references_have_distinct_canonical_encodings` |
//! | a fixed compile is byte-identical across independent construction (docs/19 §7 determinism) | `identical_references_are_byte_identical_across_independent_builds` |
//! | `selected[]`'s six list-valued-field determinism obligation is satisfiable: items sort into one stable, content-derived order | `items_sort_into_one_stable_content_derived_order` |
//! | every emitted item round-trips through the shared canonical-JSON reader (`continuum_intent::canonical_json`) with no drift | `every_emitted_item_round_trips_through_the_canonical_json_reader` |

use continuum_context::model::ModelActionRef;
use continuum_context::selection::SelectionKind;
use continuum_context::source::{SourceRef, SourceSpan};
use continuum_intent::canonical_json::Json;
use continuum_value::value::Name;
use continuum_workspace::artifact_path::{ArtifactClass, ArtifactHandle};
use continuum_workspace::snapshot::WorkspacePath;

fn name(text: &str) -> Name {
    Name::new(text).expect("well-formed test name")
}

fn path(text: &str) -> WorkspacePath {
    WorkspacePath::new(text).expect("well-formed test path")
}

fn span(
    file: &str,
    start_line: u32,
    start_column: u32,
    end_line: u32,
    end_column: u32,
) -> SourceSpan {
    SourceSpan::new(path(file), start_line, start_column, end_line, end_column)
        .expect("well-ordered test span")
}

#[test]
fn the_selection_kind_vocabulary_is_closed_and_fails_closed() {
    // Transcribed from `notes/plan/schemas/context-pack.schema.json`,
    // `properties.selected.items.properties.kind.enum` (schema order).
    const SCHEMA_ORDER: [&str; 11] = [
        "event",
        "state_delta",
        "source",
        "model",
        "proof",
        "assumption",
        "counterfactual",
        "obligation_flow",
        "order_constraint",
        "repair_surface",
        "unknown",
    ];
    let ours: Vec<&str> = SelectionKind::ALL.iter().map(|k| k.as_wire_str()).collect();
    assert_eq!(
        ours, SCHEMA_ORDER,
        "the vocabulary must be exactly the schema's, in order"
    );

    // Every declared token round-trips (anti-vacuity: the parser is not a constant).
    for token in SCHEMA_ORDER {
        assert!(
            SelectionKind::from_wire_str(token).is_some(),
            "{token} must parse"
        );
    }
    // A near-miss of every declared token is refused (fail closed, RFC 0028
    // "Versioning and revision": "Forward compatibility is achieved by rejecting,
    // never by ignoring").
    for bad in ["Source", "SOURCE", "model ", "modell", "", "kind"] {
        assert_eq!(
            SelectionKind::from_wire_str(bad),
            None,
            "{bad:?} must be refused"
        );
    }
}

#[test]
fn a_source_reference_projects_cleanly_and_carries_no_source_text() {
    let reference = SourceRef::new(span("crates/foo/src/lib.rs", 10, 1, 12, 6));
    let item = reference.into_selected_item(name("e_span"));

    assert_eq!(item.kind(), SelectionKind::Source);
    assert_eq!(item.id().as_str(), "e_span");
    assert_eq!(item.artifact(), None);
    // The summary is exactly the position, structurally incapable of carrying file
    // content: no field on `SourceRef` or `SourceSpan` holds text read from a file, and
    // this crate performs no file I/O.
    assert_eq!(item.summary(), "crates/foo/src/lib.rs:10:1-12:6");
}

#[test]
fn a_model_action_reference_projects_with_the_right_artifact_discipline() {
    let bare = ModelActionRef::action_only(name("FillBig")).into_selected_item(name("e_act1"));
    assert_eq!(bare.kind(), SelectionKind::Model);
    assert_eq!(bare.artifact(), None);

    let handle = ArtifactHandle::new(ArtifactClass::ElaboratedModel, "diehard1")
        .expect("well-formed elaborated-model handle");
    let with_model = ModelActionRef::with_model(name("FillBig"), handle.clone())
        .expect("an elaborated-model handle is accepted")
        .into_selected_item(name("e_act2"));
    assert_eq!(with_model.kind(), SelectionKind::Model);
    assert_eq!(with_model.artifact(), Some(&handle));

    // A handle of the wrong class is refused outright — never silently accepted and
    // never silently dropped to `None`.
    let wrong_class = ArtifactHandle::new(ArtifactClass::Crashpack, "cp1").expect("well-formed");
    assert!(ModelActionRef::with_model(name("FillBig"), wrong_class).is_err());
}

#[test]
fn distinct_references_have_distinct_canonical_encodings() {
    let a = SourceRef::new(span("a.rs", 1, 1, 1, 5)).into_selected_item(name("x"));
    let b = SourceRef::new(span("a.rs", 1, 1, 1, 6)).into_selected_item(name("x"));
    assert_ne!(a.to_canonical_bytes(), b.to_canonical_bytes());

    let c = ModelActionRef::action_only(name("FillBig")).into_selected_item(name("x"));
    let d = ModelActionRef::action_only(name("EmptyBig")).into_selected_item(name("x"));
    assert_ne!(c.to_canonical_bytes(), d.to_canonical_bytes());

    // A source item and a model item sharing every other field still differ, because
    // `kind` is part of the canonical encoding.
    let e = SourceRef::new(span("a.rs", 1, 1, 1, 5)).into_selected_item(name("shared"));
    let f = ModelActionRef::action_only(name("shared-name")).into_selected_item(name("shared"));
    assert_ne!(e.to_canonical_bytes(), f.to_canonical_bytes());
}

#[test]
fn identical_references_are_byte_identical_across_independent_builds() {
    // Two independently constructed values, sharing no state — the same discipline
    // docs/19 §7's determinism matrix applies to a compiled artifact.
    let build = || {
        SourceRef::new(span("crates/foo/src/lib.rs", 3, 1, 3, 9)).into_selected_item(name("e_x"))
    };
    assert_eq!(build().to_canonical_bytes(), build().to_canonical_bytes());

    let build_model = || {
        let handle = ArtifactHandle::new(ArtifactClass::ElaboratedModel, "d1").expect("ok");
        ModelActionRef::with_model(name("FillBig"), handle)
            .expect("accepted")
            .into_selected_item(name("e_y"))
    };
    assert_eq!(
        build_model().to_canonical_bytes(),
        build_model().to_canonical_bytes()
    );
}

#[test]
fn items_sort_into_one_stable_content_derived_order() {
    // RFC 0028, "Pack identity, lineage, and determinism": the six list-valued fields
    // (`selected` among them) "MUST be deterministically ordered by content identity or
    // by a declared sort key … List order is part of the canonical encoding". This crate
    // does not own the pack-level packing stage (stage 10, IMPL-06), but the items it
    // hands that stage already carry a total, content-derived order two independent
    // sorts agree on.
    let model_handle = ArtifactHandle::new(ArtifactClass::ElaboratedModel, "d1").expect("ok");
    let mut first = [
        ModelActionRef::with_model(name("FillBig"), model_handle.clone())
            .expect("accepted")
            .into_selected_item(name("c")),
        SourceRef::new(span("a.rs", 1, 1, 1, 2)).into_selected_item(name("a")),
        SourceRef::new(span("a.rs", 2, 1, 2, 2)).into_selected_item(name("b")),
    ];
    let mut second = [
        SourceRef::new(span("a.rs", 2, 1, 2, 2)).into_selected_item(name("b")),
        ModelActionRef::with_model(name("FillBig"), model_handle)
            .expect("accepted")
            .into_selected_item(name("c")),
        SourceRef::new(span("a.rs", 1, 1, 1, 2)).into_selected_item(name("a")),
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
        SourceRef::new(span("crates/foo/src/lib.rs", 10, 1, 12, 6))
            .into_selected_item(name("e_span")),
        ModelActionRef::action_only(name("FillBig")).into_selected_item(name("e_act1")),
        ModelActionRef::with_model(
            name("FillBig"),
            ArtifactHandle::new(ArtifactClass::ElaboratedModel, "diehard1").expect("ok"),
        )
        .expect("accepted")
        .into_selected_item(name("e_act2")),
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
