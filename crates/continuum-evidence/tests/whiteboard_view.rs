//! The whiteboard view as a client sees it, and the one place its relationship to
//! `whiteboard-note.schema.json` is checked against the dossier rather than against this
//! suite's own opinion (docs/44 "Whiteboard", RFC 0038 V1–V6).
//!
//! # The two dossier facts this file rests on, and how each is checked
//!
//! INV-003 makes `notes/plan/schemas/` normative, so a Rust assertion that the view "lines
//! up with the note format" would be the parser checking itself. The chain here has the same
//! two links `whiteboard_compiler.rs` builds, and no hand-written middle:
//!
//! 1. `notes/plan/schemas/examples/whiteboard-note.example.json` is validated against
//!    `whiteboard-note.schema.json` by the dossier gate with a real JSON-Schema validator.
//!    It is therefore a *conforming* note, and its top-level member set is the note format's
//!    own — not this file's reading of one. [`the_views_seven_sections_are_the_notes_own`]
//!    compares [`Section::schema_key`] against it.
//! 2. The schema document itself is read verbatim for the one fact RFC 0038 V6 turns on:
//!    it declares no `status` member, anywhere, at any nesting. That is a whole-document
//!    property, so it is checked as one — no parser to disagree with.
//!
//! The second is the load-bearing one, and it is written as a test rather than as a comment
//! for the reason this codebase writes anti-drift tests everywhere else: if the note format
//! ever gains a status member, V6's argument changes and someone has to notice. This test is
//! how they notice.
//!
//! # No byte fixtures, deliberately
//!
//! `whiteboard_compiler.rs` pins its records byte-for-byte, because what it emits is
//! evidence-graph nodes and edges — plan §4.4 artifact classes with normative schemas. A
//! view is neither: it publishes nothing, mints no identity, and RFC 0038 V6 declines to
//! give it a schema document. Pinning its bytes here would be inventing a normative shape no
//! source declared, which is the act INV-003 reserves for `notes/plan/schemas/`. What is
//! pinned instead is what the view *says* about the graph, which is the claim it makes.
//!
//! # Clause → test
//!
//! | Clause | Source | Test |
//! |---|---|---|
//! | the view's seven keys are the note's seven | V2, note schema | [`the_views_seven_sections_are_the_notes_own`] |
//! | the note format carries no status | V5, V6 | [`the_note_format_has_no_status_which_is_why_a_view_is_not_a_note`] |
//! | a view cannot be replayed as a note | V6 | [`a_view_record_is_not_a_note_and_could_not_be_compiled_as_one`] |
//! | a compiled note renders back into its own sections | V2, W2 | [`a_compiled_note_renders_back_into_the_sections_it_came_from`] |
//! | a view never upgrades a status | V5 | [`a_view_reports_a_promotion_and_never_performs_one`] |
//! | the unmapped kinds are counted, never placed | V2 | [`the_fourteen_unsectioned_kinds_are_counted_and_placed_nowhere`] |
//! | an empty scope is a view of nothing | boundary | [`a_view_of_nothing_is_seven_empty_sections`] |

use continuum_evidence::actor::ActorId;
use continuum_evidence::authority::{ServiceRole, TrustedService};
use continuum_evidence::claim_status::ClaimStatus;
use continuum_evidence::graph::EvidenceGraph;
use continuum_evidence::identity::DefaultNaming;
use continuum_evidence::node::{ClaimId, EvidenceNode, IdempotencyKey, Label, NodeKind};
use continuum_evidence::provenance::{ArtifactRef, Provenance, Timestamp, Tool};
use continuum_evidence::view::WhiteboardView;
use continuum_evidence::whiteboard::{
    Decision, Entry, Experiment, NoteText, Section, WhiteboardNote,
};
use continuum_value::epoch::EpochSet;
use continuum_value::value::Value;

/// The dossier's own note schema.
const NOTE_SCHEMA: &str = include_str!("../../../notes/plan/schemas/whiteboard-note.schema.json");

/// The validator-blessed conforming note.
const NOTE_EXAMPLE: &str =
    include_str!("../../../notes/plan/schemas/examples/whiteboard-note.example.json");

/// The schema for what a note compiles *to*, used as the positive control: it declares the
/// `status` member the note format deliberately does not.
const NODE_SCHEMA: &str =
    include_str!("../../../notes/plan/schemas/evidence-graph-node.schema.json");

/// The note-level members of a note: everything the example declares that is not a section.
const NOTE_LEVEL_MEMBERS: [&str; 6] = [
    "author",
    "created_at",
    "note_id",
    "schema_epoch",
    "schema_id",
    "tool",
];

fn artifact(handle: &str) -> ArtifactRef {
    ArtifactRef::new(handle).expect("well formed")
}

fn node(kind: NodeKind, handle: &str, claim: &str) -> EvidenceNode {
    EvidenceNode::propose(
        kind,
        artifact(handle),
        ClaimId::new(claim).expect("non-empty"),
        IdempotencyKey::new("key-1").expect("non-empty"),
        Provenance::new(
            ActorId::new("agent:swarm-1").expect("well formed"),
            Timestamp::new("2026-08-01T12:00:00.000Z").expect("canonical"),
            [],
        ),
    )
}

/// The top-level member names of a two-space pretty-printed JSON object, in file order.
///
/// Scaffolding, not a codec: it reads one indentation level of one file whose exact
/// formatting the dossier gate already enforces, and it is never linked into the library.
fn top_level_members(document: &str) -> Vec<String> {
    document
        .lines()
        .filter_map(|line| {
            let rest = line.strip_prefix("  \"")?;
            let (name, tail) = rest.split_once('"')?;
            tail.starts_with(':').then(|| name.to_owned())
        })
        .collect()
}

// --- the dossier facts ---------------------------------------------------------------

#[test]
fn the_views_seven_sections_are_the_notes_own() {
    // Link 1: the example is validated against the schema by the dossier gate, so its
    // member set is the note format's own. The view's seven keys are exactly the example's
    // members minus the six note-level ones — so the two directions of the whiteboard name
    // the same seven sections, and neither can rename one without this failing.
    let members = top_level_members(NOTE_EXAMPLE);
    assert_eq!(
        members.len(),
        13,
        "the example declares twelve required members plus `tool`"
    );
    let sections: Vec<String> = Section::ALL
        .iter()
        .map(|it| it.schema_key().to_owned())
        .collect();
    let mut remaining: Vec<&String> = members
        .iter()
        .filter(|member| !sections.contains(member))
        .collect();
    remaining.sort();
    assert_eq!(
        remaining,
        NOTE_LEVEL_MEMBERS.iter().collect::<Vec<_>>(),
        "every example member that is not a section is a note-level member"
    );
    for section in Section::ALL {
        assert!(
            members.iter().any(|it| it == section.schema_key()),
            "the note declares no `{}` section",
            section.schema_key()
        );
    }
}

#[test]
fn the_note_format_has_no_status_which_is_why_a_view_is_not_a_note() {
    // RFC 0038 V6's premise, checked as the whole-document property it is. W5 makes the
    // absence deliberate — "the format has no status member" — and that absence is a guard
    // on the INPUT side only. A view that rendered held state into this format would erase
    // every status the graph holds, which is why the view carries one and the note may not.
    // The quoted token, which is how a member name and a `required` entry are both spelled
    // in these documents — and which the schema's *prose* about status words does not match,
    // so the check is about the format rather than about the wording.
    assert!(
        !NOTE_SCHEMA.contains("\"status\""),
        "the note schema gained a status member; RFC 0038 V6's argument must be revisited"
    );
    assert!(!NOTE_EXAMPLE.contains("\"status\""));
    // Armed: the sibling schema for what a note compiles *to* does declare one, so the
    // absence above is a fact about the note format and not about the scan.
    assert!(NODE_SCHEMA.contains("\"status\""));

    // …and the view does carry one, on every row, so the two formats are not interchangeable
    // in either direction.
    let mut graph = EvidenceGraph::new();
    graph.add_node(node(NodeKind::Property, "prop_9f", "claim-1"));
    let view = WhiteboardView::of_graph(&graph);
    let Value::Record(record) = view.to_record(&DefaultNaming::new()) else {
        panic!("a view renders as a record");
    };
    let Some(Value::Seq(rows)) = record
        .iter()
        .find_map(|(name, value)| (name.as_str() == Section::Goal.schema_key()).then_some(value))
    else {
        panic!("the goal section is a list");
    };
    assert_eq!(rows.len(), 1);
    let Value::Record(row) = &rows[0] else {
        panic!("a row is a record");
    };
    assert!(row.iter().any(|(name, _)| name.as_str() == "status"));
}

#[test]
fn a_view_record_is_not_a_note_and_could_not_be_compiled_as_one() {
    // V6 stated as a shape: a view carries none of the five members that make a document a
    // note, so nothing can hand a view back to `whiteboard.compile` as a fresh proposal set
    // under a new author. The laundering channel is closed by construction, not by a check.
    let mut graph = EvidenceGraph::new();
    graph.add_node(node(NodeKind::Property, "prop_9f", "claim-1"));
    let Value::Record(record) = WhiteboardView::of_graph(&graph).to_record(&DefaultNaming::new())
    else {
        panic!("a view renders as a record");
    };
    let members: Vec<&str> = record.keys().map(|it| it.as_str()).collect();
    for absent in NOTE_LEVEL_MEMBERS {
        assert!(
            !members.contains(&absent),
            "a view names `{absent}`, which only a note may"
        );
    }
    // …and it names one member no note may: what it did not render (V2).
    assert!(members.contains(&"unrendered"));
    // The example, by contrast, names all six. Anti-vacuity for the sweep above.
    let note_members = top_level_members(NOTE_EXAMPLE);
    for present in NOTE_LEVEL_MEMBERS {
        assert!(note_members.iter().any(|it| it == present));
    }
}

// --- the round trip ------------------------------------------------------------------

/// A note exercising all six proposing sections, an experiment, and a decision.
fn full_note() -> WhiteboardNote {
    let entry = |claim: &str, subject: &str, references: &[&str]| {
        Entry::new(
            ClaimId::new(claim).expect("non-empty"),
            artifact(subject),
            NoteText::new("a line the compiler never reads").expect("non-empty"),
            references.iter().map(|it| artifact(it)),
        )
    };
    WhiteboardNote::new(
        artifact("wb_note1"),
        ActorId::new("agent:swarm-1").expect("well formed"),
        Timestamp::new("2026-08-01T12:00:00.000Z").expect("canonical"),
        entry("claim-goal", "prop_9f", &["trace_9f"]),
    )
    .expect("an unreserved class")
    .with_tool(Tool::new("continuum-forge").expect("non-empty"))
    .with_known_facts([entry("claim-fact", "model_9f", &[])])
    .with_candidate_invariants([entry("claim-inv", "inv_9f", &[])])
    .with_counterexamples([entry("claim-cex", "crash_9f", &[])])
    .with_unresolved_obligations([entry("claim-obl", "goal_9f", &[])])
    .with_experiments([Experiment::new(
        NoteText::new("re-run under the durable-visibility observer").expect("non-empty"),
        [artifact("trace_9f")],
    )])
    .with_decisions([
        Decision::new(entry("claim-dec", "dec_9f", &["trace_9f"])).expect("supported")
    ])
}

#[test]
fn a_compiled_note_renders_back_into_the_sections_it_came_from() {
    // The two directions, joined. W2 says the heading decides the kind; V2 says the kind
    // decides the section; so a note that went in comes back out under its own headings —
    // and this is the test that would fail if either mapping moved on its own.
    let mut graph = EvidenceGraph::new();
    graph.add_node(node(NodeKind::Run, "trace_9f", "claim-seed"));
    let note = full_note();
    note.compile(&graph, &EpochSet::unpinned())
        .expect("every reference resolves")
        .apply(&mut graph)
        .expect("appended");

    let view = WhiteboardView::of_graph(&graph);
    let subjects = |section: Section| -> Vec<&str> {
        view.section(section)
            .iter()
            .map(|row| row.subject().as_str())
            .collect()
    };
    assert_eq!(subjects(Section::Goal), ["prop_9f"]);
    assert_eq!(subjects(Section::KnownFact), ["model_9f"]);
    assert_eq!(subjects(Section::CandidateInvariant), ["inv_9f"]);
    assert_eq!(subjects(Section::Counterexample), ["crash_9f"]);
    assert_eq!(subjects(Section::UnresolvedObligation), ["goal_9f"]);
    assert_eq!(subjects(Section::Decision), ["dec_9f"]);
    // V4: the experiment became a task proposal and never a node, so nothing can be here.
    assert!(view.section(Section::Experiment).is_empty());
    assert_eq!(view.row_count(), 6);

    // Every proposal is at the bottom of the lattice, and the view says so rather than
    // inferring anything from the section it landed in (V5, W5).
    for (_, rows) in view.sections() {
        for row in rows {
            assert_eq!(row.status(), ClaimStatus::Proposed);
        }
    }

    // The seed `run` node has no heading, so it is counted and placed nowhere (V2).
    let unrendered: Vec<(NodeKind, usize)> = view.unrendered().collect();
    assert_eq!(unrendered, [(NodeKind::Run, 1)]);

    // Each row carries the label the compiler wrote for its own section, so a reader can
    // still tell a whiteboard-origin node from any other (W9) — even though the label is
    // not what placed it.
    for (section, rows) in view.sections() {
        for row in rows {
            let labels: Vec<&str> = row.labels().map(Label::as_str).collect();
            assert_eq!(labels, [section.label().as_str()], "{section}");
        }
    }
}

#[test]
fn a_view_reports_a_promotion_and_never_performs_one() {
    // V5. The same graph, viewed before and after a promotion: exactly one row moves, and
    // it moves to what the graph holds. Nothing in the view can reach `promote`.
    let mut graph = EvidenceGraph::new();
    graph.add_node(node(NodeKind::Run, "trace_9f", "claim-seed"));
    full_note()
        .compile(&graph, &EpochSet::unpinned())
        .expect("every reference resolves")
        .apply(&mut graph)
        .expect("appended");

    let before = WhiteboardView::of_graph(&graph);
    let goal = before.section(Section::Goal)[0].identity().clone();
    let checker = TrustedService::new(
        continuum_evidence::actor::ServiceIdentity::parse("service:checker").expect("a service"),
        [ServiceRole::IndependentChecker],
    )
    .expect("one role");
    graph
        .promote(
            &goal,
            ClaimStatus::Proposed,
            &checker
                .validate(continuum_value::assurance::ValidationBasis::CheckedCertificate)
                .expect("authorized"),
        )
        .expect("an independent checker promotes another actor's proposal");

    let after = WhiteboardView::of_graph(&graph);
    assert_eq!(
        after.section(Section::Goal)[0].status(),
        ClaimStatus::Validated
    );
    assert_eq!(after.section(Section::Goal)[0].identity(), &goal);
    // …and nothing else moved.
    for section in Section::ALL {
        if section == Section::Goal {
            continue;
        }
        assert_eq!(before.section(section), after.section(section), "{section}");
    }
    assert_eq!(before.row_count(), after.row_count());
}

// --- boundaries ----------------------------------------------------------------------

#[test]
fn the_fourteen_unsectioned_kinds_are_counted_and_placed_nowhere() {
    // V2 at the widest boundary: one node of each of plan §11.2's twenty kinds. Six are
    // rows, fourteen are counts, and the two sets partition what is held — a view is a
    // projection that says how much it projected away.
    let mut graph = EvidenceGraph::new();
    for kind in NodeKind::ALL {
        graph.add_node(node(kind, &format!("art_{}", kind.as_str()), "claim-1"));
    }
    let view = WhiteboardView::of_graph(&graph);
    assert_eq!(view.row_count(), 6);
    assert_eq!(view.unrendered_count(), 14);
    assert_eq!(
        view.row_count() + view.unrendered_count(),
        graph.node_count()
    );
    for (kind, count) in view.unrendered() {
        assert_eq!(count, 1, "{kind}");
        for (_, rows) in view.sections() {
            assert!(rows.iter().all(|row| row.kind() != kind), "{kind}");
        }
    }
}

#[test]
fn a_view_of_nothing_is_seven_empty_sections() {
    // Boundary: an empty scope renders a view, not an absence of one — W1's reason for
    // present-and-empty sections, read on the reading side.
    let view = WhiteboardView::of_graph(&EvidenceGraph::new());
    assert_eq!(view.sections().len(), 7);
    assert_eq!(view.row_count(), 0);
    assert_eq!(view.unrendered_count(), 0);
    let Value::Record(record) = view.to_record(&DefaultNaming::new()) else {
        panic!("a view renders as a record");
    };
    assert_eq!(record.len(), 8, "seven sections plus `unrendered`");
    for value in record.values() {
        assert_eq!(value, &Value::seq([]).expect("an empty list"));
    }
}
