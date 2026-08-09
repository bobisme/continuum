//! The whiteboard compiler as a client sees it, and the one place its artifacts are checked
//! against the schemas rather than against this suite's own opinion (plan §11.5, docs/44,
//! RFC 0038 "Whiteboard compiler").
//!
//! # Why the fixtures exist
//!
//! INV-003 makes `notes/plan/schemas/` normative for artifact shape, so a Rust assertion
//! that a record "looks right" establishes nothing: it is the parser checking itself. The
//! chain this file builds instead has two links and no hand-written middle:
//!
//! 1. every JSON document below is validated against its schema by the dossier gate with a
//!    real JSON-Schema validator — `whiteboard-note.schema.json` for the note (through
//!    `SCHEMA_PAIRS`, which already validates the dossier's own example) and
//!    `evidence-graph-{node,edge}.schema.json` for what the compiler emits (through
//!    `EXTERNAL_SCHEMA_PAIRS`, the device `continuum-intent`'s contract fixtures already
//!    use);
//! 2. the tests here assert that the library's records render to exactly those bytes.
//!
//! So "the compiler emits schema-conforming artifacts" is checked by the validator, not
//! asserted here, and "these are the bytes the compiler emits" is checked here. Neither half
//! can pass on its own.
//!
//! The note fixture is `notes/plan/schemas/examples/whiteboard-note.example.json` itself —
//! the dossier's normative example *is* what the library produces, which is stronger than a
//! second copy that could drift from it.
//!
//! # The renderer
//!
//! [`json`] is test scaffolding, not a codec. The library's records are
//! `continuum_value::value::Value`, the schemas govern JSON, and something has to put the
//! first in front of the second; the wire's own JSON spelling is `continuumd`'s codec and
//! this is deliberately not a second one — it is never linked into the library, handles only
//! the five kinds these records use, and panics rather than guessing on anything else.

use continuum_evidence::actor::ActorId;
use continuum_evidence::claim_status::ClaimStatus;
use continuum_evidence::edge::EdgeKind;
use continuum_evidence::graph::EvidenceGraph;
use continuum_evidence::identity::{DefaultNaming, EvidenceHandle, EvidenceNaming};
use continuum_evidence::node::{ClaimId, EvidenceNode, IdempotencyKey, NodeKind};
use continuum_evidence::provenance::{ArtifactRef, Provenance, Timestamp, Tool};
use continuum_evidence::whiteboard::{
    Compilation, Decision, Entry, Experiment, NoteText, Section, WhiteboardNote,
};
use continuum_value::epoch::EpochSet;

/// A canonical-enough JSON writer: object keys ascending by code point, two-space indent,
/// one trailing newline.
mod json {
    use continuum_value::value::Value;

    /// Render a value as the JSON a schema validates, with a trailing newline.
    pub fn document(value: &Value) -> String {
        let mut out = String::new();
        write(value, 0, &mut out);
        out.push('\n');
        out
    }

    fn write(value: &Value, indent: usize, out: &mut String) {
        match value {
            Value::Text(text) => string(text, out),
            Value::Nat(n) => out.push_str(&n.to_string()),
            Value::Int(n) => out.push_str(&n.to_string()),
            Value::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
            Value::Null => out.push_str("null"),
            Value::Seq(items) => {
                if items.is_empty() {
                    out.push_str("[]");
                    return;
                }
                out.push_str("[\n");
                for (position, item) in items.iter().enumerate() {
                    pad(indent + 1, out);
                    write(item, indent + 1, out);
                    out.push_str(if position + 1 == items.len() {
                        "\n"
                    } else {
                        ",\n"
                    });
                }
                pad(indent, out);
                out.push(']');
            }
            Value::Record(fields) => {
                if fields.is_empty() {
                    out.push_str("{}");
                    return;
                }
                // `Name`'s own order is CVNF's shortlex; JSON's canonical order is by code
                // point, so the keys are re-sorted here rather than taken as they come.
                let mut members: Vec<(&str, &Value)> = fields
                    .iter()
                    .map(|(name, value)| (name.as_str(), value))
                    .collect();
                members.sort_by_key(|(name, _)| *name);
                out.push_str("{\n");
                for (position, (name, member)) in members.iter().enumerate() {
                    pad(indent + 1, out);
                    string(name, out);
                    out.push_str(": ");
                    write(member, indent + 1, out);
                    out.push_str(if position + 1 == members.len() {
                        "\n"
                    } else {
                        ",\n"
                    });
                }
                pad(indent, out);
                out.push('}');
            }
            other => panic!("no JSON spelling for {other:?}: the evidence records use five kinds"),
        }
    }

    fn pad(indent: usize, out: &mut String) {
        for _ in 0..indent {
            out.push_str("  ");
        }
    }

    fn string(text: &str, out: &mut String) {
        out.push('"');
        for character in text.chars() {
            match character {
                '"' => out.push_str("\\\""),
                '\\' => out.push_str("\\\\"),
                '\u{8}' => out.push_str("\\b"),
                '\u{c}' => out.push_str("\\f"),
                '\n' => out.push_str("\\n"),
                '\r' => out.push_str("\\r"),
                '\t' => out.push_str("\\t"),
                control if control < ' ' => {
                    out.push_str(&format!("\\u{:04x}", control as u32));
                }
                other => out.push(other),
            }
        }
        out.push('"');
    }
}

const NOTE_EXAMPLE: &str =
    include_str!("../../../notes/plan/schemas/examples/whiteboard-note.example.json");

fn artifact(handle: &str) -> ArtifactRef {
    ArtifactRef::new(handle).expect("well formed")
}

fn text(line: &str) -> NoteText {
    NoteText::new(line).expect("non-empty")
}

fn entry(claim: &str, subject: &str, line: &str, references: &[&str]) -> Entry {
    Entry::new(
        ClaimId::new(claim).expect("non-empty"),
        artifact(subject),
        text(line),
        references.iter().map(|it| artifact(it)),
    )
}

/// The note of `notes/plan/schemas/examples/whiteboard-note.example.json`, built through the
/// library's own constructors.
fn note() -> WhiteboardNote {
    WhiteboardNote::new(
        artifact("wb_ack_ordering_1"),
        ActorId::new("agent:swarm-invariant-1").expect("well formed"),
        Timestamp::new("2026-08-01T12:00:00.000Z").expect("canonical"),
        entry(
            "claim-ack-after-durable",
            "prop_ack_after_durable",
            "Establish that a reply is published only after the write it acknowledges is durable.",
            &["in_ack_v1"],
        ),
    )
    .expect("an unreserved class")
    .with_tool(Tool::new("continuum-forge").expect("non-empty"))
    .with_known_facts([entry(
        "claim-storage-fsync-ordering",
        "model_storage_v3",
        "The storage domain pack orders fsync before the completion event.",
        &["in_ack_v1"],
    )])
    .with_candidate_invariants([entry(
        "claim-ack-after-durable",
        "inv_ack_implies_durable",
        "For every ack, the corresponding write is already in the durable set.",
        &["model_storage_v3"],
    )])
    .with_counterexamples([entry(
        "claim-ack-after-durable",
        "crash_ack_before_fsync",
        "A schedule that publishes the reply between the buffer write and the fsync.",
        &["model_storage_v3"],
    )])
    .with_unresolved_obligations([entry(
        "claim-ack-after-durable",
        "goal_ack_durable_induction",
        "The induction step for the multi-writer case is not discharged.",
        &["model_storage_v3"],
    )])
    .with_experiments([Experiment::new(
        text("Re-run the DPOR exploration with the observer that distinguishes buffer-visible from durable."),
        [artifact("crash_ack_before_fsync")],
    )])
    .with_decisions([Decision::new(entry(
        "claim-observer-contract",
        "dec_observer_durable",
        "Adopt the durable-visibility observer contract for this property.",
        &["crash_ack_before_fsync", "model_storage_v3"],
    ))
    .expect("supported")])
}

/// A node a *service* published earlier, so nothing the note references was produced by the
/// note's own author.
fn published(kind: NodeKind, subject: &str, claim: &str) -> EvidenceNode {
    EvidenceNode::propose(
        kind,
        artifact(subject),
        ClaimId::new(claim).expect("non-empty"),
        IdempotencyKey::new(&format!("seed:{subject}")).expect("non-empty"),
        Provenance::new(
            ActorId::new("service:engine-dpor").expect("well formed"),
            Timestamp::new("2026-07-30T09:00:00.000Z").expect("canonical"),
            [],
        ),
    )
}

/// The graph the note is compiled against: one published node per referenced artifact.
fn seeded() -> EvidenceGraph {
    let mut graph = EvidenceGraph::new();
    for node in [
        published(NodeKind::IntentClaim, "in_ack_v1", "claim-intent-ack"),
        published(NodeKind::Model, "model_storage_v3", "claim-storage-model"),
        published(
            NodeKind::Counterexample,
            "crash_ack_before_fsync",
            "claim-observed-crash",
        ),
    ] {
        graph.add_node(node);
    }
    graph
}

fn compiled() -> (EvidenceGraph, Compilation) {
    let graph = seeded();
    let compilation = note()
        .compile(&graph, &EpochSet::unpinned())
        .expect("every reference resolves");
    (graph, compilation)
}

fn handle(node: &EvidenceNode) -> EvidenceHandle {
    DefaultNaming::new().name(&node.identity())
}

#[test]
fn the_note_renders_to_the_dossiers_own_example() {
    // The strongest form of "this note conforms": the bytes the library produces are the
    // bytes the dossier gate validates against `whiteboard-note.schema.json`.
    assert_eq!(json::document(&note().to_record()), NOTE_EXAMPLE);
}

#[test]
fn every_emitted_node_renders_to_its_registered_fixture() {
    let (_, compilation) = compiled();
    let rendered: Vec<(Section, String)> = compilation
        .nodes()
        .map(|compiled| {
            (
                compiled.section(),
                json::document(&compiled.node().to_record(&handle(compiled.node()))),
            )
        })
        .collect();
    let expected: [(Section, &str); 6] = [
        (
            Section::Goal,
            include_str!("fixtures/whiteboard/node-goal.json"),
        ),
        (
            Section::KnownFact,
            include_str!("fixtures/whiteboard/node-known-fact.json"),
        ),
        (
            Section::CandidateInvariant,
            include_str!("fixtures/whiteboard/node-candidate-invariant.json"),
        ),
        (
            Section::Counterexample,
            include_str!("fixtures/whiteboard/node-counterexample.json"),
        ),
        (
            Section::UnresolvedObligation,
            include_str!("fixtures/whiteboard/node-unresolved-obligation.json"),
        ),
        (
            Section::Decision,
            include_str!("fixtures/whiteboard/node-decision.json"),
        ),
    ];
    assert_eq!(rendered.len(), expected.len());
    for ((section, actual), (want_section, want)) in rendered.iter().zip(expected) {
        assert_eq!(*section, want_section);
        assert_eq!(actual.as_str(), want, "section `{section}`");
    }
}

#[test]
fn every_emitted_edge_renders_to_its_registered_fixture() {
    let (graph, compilation) = compiled();
    let named = |identity| {
        let node = graph
            .node(identity)
            .or_else(|| {
                compilation
                    .nodes()
                    .map(|compiled| compiled.node())
                    .find(|node| &node.identity() == identity)
            })
            .expect("an endpoint is either held or proposed");
        handle(node)
    };
    let rendered: Vec<String> = compilation
        .edges()
        .map(|edge| {
            let record = edge.to_record(
                &DefaultNaming::new().name(&edge.identity()),
                &named(edge.from().identity()),
                &named(edge.to().identity()),
            );
            json::document(&record)
        })
        .collect();
    let expected = [
        include_str!("fixtures/whiteboard/edge-supports-crash.json"),
        include_str!("fixtures/whiteboard/edge-supports-model.json"),
    ];
    assert_eq!(rendered.len(), expected.len());
    for (actual, want) in rendered.iter().zip(expected) {
        assert_eq!(actual.as_str(), want);
    }
}

#[test]
fn the_fixtures_are_a_whole_compilation_and_not_a_selection() {
    // Anti-vacuity for the two fixture tests: they would still pass over a compiler that
    // emitted a subset, so the counts and the task lane are pinned here.
    let (_, compilation) = compiled();
    assert_eq!(compilation.nodes().len(), 6);
    assert_eq!(compilation.edges().len(), 2);
    assert_eq!(compilation.tasks().len(), 1);
    let sections: Vec<Section> = compilation.nodes().map(|it| it.section()).collect();
    assert_eq!(
        sections,
        [
            Section::Goal,
            Section::KnownFact,
            Section::CandidateInvariant,
            Section::Counterexample,
            Section::UnresolvedObligation,
            Section::Decision,
        ]
    );
    // The one section that proposes no node proposed a task instead.
    let task = compilation.tasks().next().expect("the experiment");
    assert!(task.text().as_str().starts_with("Re-run the DPOR"));
}

#[test]
fn a_client_can_apply_a_compilation_and_the_graph_holds_exactly_it() {
    let (mut graph, compilation) = compiled();
    let before = graph.node_count();
    let applied = compilation
        .apply(&mut graph)
        .expect("every endpoint is held");
    assert_eq!(applied.fresh_count(), 8);
    assert_eq!(graph.node_count(), before + 6);
    assert_eq!(graph.edge_count(), 2);
    for (_, node) in graph.nodes() {
        assert_eq!(node.status(), ClaimStatus::Proposed);
    }
    // Reapplying converges: the graph is put-if-absent and the compiler is a function.
    let again = compilation
        .apply(&mut graph)
        .expect("every endpoint is held");
    assert_eq!(again.fresh_count(), 0);
    assert_eq!(graph.node_count(), before + 6);
    assert_eq!(graph.edge_count(), 2);
}

#[test]
fn the_compiler_writes_no_status_and_records_no_check() {
    // RFC 0038: "rejects […] status claims without evidence", and D3's check edge is the
    // checker's own artifact. Both are absences, so both are checked over the whole output.
    let (mut graph, compilation) = compiled();
    compilation.apply(&mut graph).expect("appended");
    for compiled in compilation.nodes() {
        assert_eq!(compiled.node().status(), ClaimStatus::Proposed);
        assert!(
            compiled
                .node()
                .provenance()
                .actor()
                .as_str()
                .starts_with("agent:")
        );
    }
    for edge in compilation.edges() {
        assert_eq!(edge.kind(), EdgeKind::Supports);
        assert!(edge.checker().is_none());
    }
    // …and nothing the compiler wrote can be read as a contradiction it manufactured.
    assert!(graph.contradictions().is_empty());
}
