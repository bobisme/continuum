//! The derivation-provenance query as a client sees it, over content a real writer produced
//! (docs/44 "Credit and provenance", RFC 0038 "Queries" P1–P7).
//!
//! # Why this file compiles notes instead of hand-building provenance
//!
//! The unit tests in `src/derivation.rs` construct `provenance.inputs` directly, which
//! proves the walk and nothing about whether anything ever writes that field the way the
//! walk assumes. RFC 0038 records three writers — `observe.ingest`, `evidence.link`, and
//! `whiteboard.compile` — and exactly one of them has a library twin this crate can drive:
//! [`WhiteboardNote::compile`]. So the graphs below are compiled from notes, and every input
//! the closure walks was written by the compiler under RFC 0038 W4 ("the references become
//! the compiled node's `provenance.inputs` — derivation, never assertion"), not by this
//! suite.
//!
//! # The case that could not have been contrived
//!
//! W9 says a whiteboard note "is not a new artifact class", mints no node, and is named in
//! every compiled node's `provenance.inputs` — the reason the class is required not to be
//! `ev_` or `cap_`. So **every** graph compiled from a note holds nodes whose inputs name an
//! artifact the graph does not hold, and it holds them however carefully the note's own
//! references resolved. That is the strongest available statement of P6: a dangling input is
//! not a malformed graph, it is the normal output of the one writer this crate can run, and
//! W4's "references must resolve" is a rule on the *write* path rather than a guarantee the
//! read path may assume.
//!
//! # No byte fixtures, deliberately
//!
//! `whiteboard_compiler.rs` pins its records byte-for-byte because what it emits is plan
//! §4.4 artifact classes with normative schemas. A closure is neither: it publishes nothing,
//! mints no identity, and — like the view — has no schema document, because it is not a wire
//! answer (P1: `evidence.query` cannot spell one). Pinning its bytes would invent a
//! normative shape no source declared, the act INV-003 reserves for `notes/plan/schemas/`.
//! What is pinned is what the closure *says* about the graph, and that its answer is a
//! function of the graph's content alone.
//!
//! # Clause → test
//!
//! | Clause | Source | Test |
//! |---|---|---|
//! | the compiler writes the inputs the walk follows | W4, P3 | [`a_compiled_notes_closure_reaches_the_artifacts_it_cited`] |
//! | a note is named and never held | W9, P6 | [`every_compiled_node_names_the_note_and_the_note_is_absent`] |
//! | two generations chain, and a shared ancestor is reached once | P4 | [`two_notes_in_sequence_are_two_derivation_steps`] |
//! | an outside class is named, not refused | INV-016, P6 | [`an_input_in_a_class_with_no_node_kind_is_named_not_refused`] |
//! | incompleteness is typed and distinguishes its reasons | INV-008 | [`the_closure_says_which_kind_of_incomplete_it_is`] |
//! | the query writes nothing | P1 | [`closing_over_a_graph_appends_nothing_and_promotes_nothing`] |
//! | a graph with no inputs closes completely | boundary | [`a_graph_that_cites_nothing_closes_to_itself`] |

use continuum_evidence::actor::ActorId;
use continuum_evidence::claim_status::ClaimStatus;
use continuum_evidence::derivation::{DerivationClosure, Incompleteness};
use continuum_evidence::graph::EvidenceGraph;
use continuum_evidence::node::{ClaimId, EvidenceNode, IdempotencyKey, NodeKind};
use continuum_evidence::provenance::{ArtifactRef, Provenance, Timestamp, Tool};
use continuum_evidence::whiteboard::{Decision, Entry, NoteText, WhiteboardNote};
use continuum_value::epoch::EpochSet;

fn artifact(handle: &str) -> ArtifactRef {
    ArtifactRef::new(handle).expect("well formed")
}

fn entry(claim: &str, subject: &str, references: &[&str]) -> Entry {
    Entry::new(
        ClaimId::new(claim).expect("non-empty"),
        artifact(subject),
        NoteText::new("a line the compiler never reads").expect("non-empty"),
        references.iter().map(|it| artifact(it)),
    )
}

fn note(note_id: &str, goal: Entry) -> WhiteboardNote {
    WhiteboardNote::new(
        artifact(note_id),
        ActorId::new("agent:swarm-1").expect("well formed"),
        Timestamp::new("2026-08-01T12:00:00.000Z").expect("canonical"),
        goal,
    )
    .expect("an unreserved class")
    .with_tool(Tool::new("continuum-forge").expect("non-empty"))
}

/// A seed node about `handle`, deriving from `inputs`.
fn seed(handle: &str, inputs: &[&str]) -> EvidenceNode {
    EvidenceNode::propose(
        NodeKind::Run,
        artifact(handle),
        ClaimId::new("claim-seed").expect("non-empty"),
        IdempotencyKey::new("key-seed").expect("non-empty"),
        Provenance::new(
            ActorId::new("service:observer").expect("well formed"),
            Timestamp::new("2026-08-01T12:00:00.000Z").expect("canonical"),
            inputs.iter().map(|it| artifact(it)),
        ),
    )
}

/// The artifacts a closure reached, each with its depth, sorted for comparison.
fn subjects(closure: &DerivationClosure) -> Vec<String> {
    let mut listed: Vec<String> = closure
        .reached()
        .map(|(_, row)| format!("{}@{}", row.artifact(), row.depth()))
        .collect();
    listed.sort();
    listed
}

/// The artifacts a closure named and could not reach.
fn absences(closure: &DerivationClosure) -> Vec<String> {
    closure
        .absent()
        .map(|it| format!("{}@{}", it.artifact(), it.depth()))
        .collect()
}

#[test]
fn a_compiled_notes_closure_reaches_the_artifacts_it_cited() {
    // W4 → P3, end to end: the note's references become the compiled node's inputs, and the
    // closure resolves each input to every held node about that artifact.
    let mut graph = EvidenceGraph::new();
    graph.add_node(seed("trace_9f", &[]));
    let compiled = note("wb_note1", entry("claim-goal", "prop_9f", &["trace_9f"]))
        .with_decisions([
            Decision::new(entry("claim-dec", "dec_9f", &["trace_9f"])).expect("supported")
        ])
        .compile(&graph, &EpochSet::unpinned())
        .expect("every reference resolves");
    let decision = compiled
        .nodes()
        .find(|it| it.node().kind() == NodeKind::Decision)
        .expect("one decision")
        .node()
        .clone();
    compiled.apply(&mut graph).expect("appended");

    let closure = DerivationClosure::of(&graph, [&decision]);
    // The decision at 0; the cited run node at 1. Nothing else: `prop_9f` cited the same
    // run node but is not itself an input of the decision, so it is not an ancestor.
    assert_eq!(subjects(&closure), ["dec_9f@0", "trace_9f@1"]);
    assert!(closure.holds(&decision.identity()));

    // A SUPPORTS edge was drawn between exactly these two, and the closure did not use it —
    // it used the inputs. (P1: the two relations coincide here and diverge below.)
    assert_eq!(
        graph
            .edges_of_kind(continuum_evidence::edge::EdgeKind::Supports)
            .count(),
        1
    );
}

#[test]
fn every_compiled_node_names_the_note_and_the_note_is_absent() {
    // W9 → P6. The note handle is in every compiled node's inputs and the graph holds no
    // node about it, because a note is not an artifact class. So a perfectly ordinary
    // compilation produces a graph with a named-but-absent input, and the closure says so
    // rather than dropping it.
    let mut graph = EvidenceGraph::new();
    graph.add_node(seed("trace_9f", &[]));
    let compiled = note("wb_note1", entry("claim-goal", "prop_9f", &["trace_9f"]))
        .with_known_facts([entry("claim-fact", "model_9f", &[])])
        .compile(&graph, &EpochSet::unpinned())
        .expect("every reference resolves");
    // Anti-vacuity: the note really is named by every compiled node.
    for node in compiled.nodes() {
        assert!(
            node.node().provenance().has_input(&artifact("wb_note1")),
            "{}",
            node.node().artifact()
        );
    }
    compiled.apply(&mut graph).expect("appended");

    let closure = DerivationClosure::of_graph(&graph);
    assert_eq!(absences(&closure), ["wb_note1@1"]);
    assert_eq!(
        closure.incompleteness(),
        [Incompleteness::AbsentInputs { count: 1 }]
    );
    // …and every compiled node is recorded as having named it, so credit is not lost.
    let named_by: usize = closure
        .absent()
        .next()
        .expect("one absence")
        .named_by()
        .len();
    assert_eq!(named_by, 2);
    // The graph itself is not malformed: the note's own references all resolved.
    assert_eq!(closure.reached_count(), graph.node_count());
}

#[test]
fn two_notes_in_sequence_are_two_derivation_steps() {
    // P4 over a real two-generation chain, with a shared ancestor reached once at the
    // shorter depth. Note 1 proposes `prop_9f` from `trace_9f`; note 2 proposes `dec_9f`
    // from both `prop_9f` and `trace_9f`, so `trace_9f` is reachable at 2 and at 1.
    let mut graph = EvidenceGraph::new();
    graph.add_node(seed("trace_9f", &[]));
    note("wb_note1", entry("claim-goal", "prop_9f", &["trace_9f"]))
        .compile(&graph, &EpochSet::unpinned())
        .expect("every reference resolves")
        .apply(&mut graph)
        .expect("appended");

    let second = note("wb_note2", entry("claim-goal2", "goal_9f", &["prop_9f"]))
        .with_decisions([
            Decision::new(entry("claim-dec", "dec_9f", &["prop_9f", "trace_9f"]))
                .expect("supported"),
        ])
        .compile(&graph, &EpochSet::unpinned())
        .expect("every reference resolves");
    let decision = second
        .nodes()
        .find(|it| it.node().kind() == NodeKind::Decision)
        .expect("one decision")
        .node()
        .clone();
    second.apply(&mut graph).expect("appended");

    let closure = DerivationClosure::of(&graph, [&decision]);
    assert_eq!(
        subjects(&closure),
        ["dec_9f@0", "prop_9f@1", "trace_9f@1"],
        "the shorter of two routes wins"
    );
    assert_eq!(closure.depth(), 1);
    // Both notes are named along the way, and neither is held.
    assert_eq!(absences(&closure), ["wb_note1@2", "wb_note2@1"]);

    // Bounding at one step keeps the first generation and says the bound stopped it.
    let bounded = DerivationClosure::bounded(&graph, [&decision], Some(1));
    assert_eq!(subjects(&bounded), ["dec_9f@0", "prop_9f@1", "trace_9f@1"]);
    assert_eq!(absences(&bounded), ["wb_note2@1"]);
    assert_eq!(
        bounded.incompleteness(),
        [
            Incompleteness::Bound { max_depth: 1 },
            Incompleteness::AbsentInputs { count: 1 }
        ]
    );
}

#[test]
fn an_input_in_a_class_with_no_node_kind_is_named_not_refused() {
    // INV-016 → P6. `provenance.inputs` is written for admitted actors, but an entry is a
    // *name*: a capability grant, a raw trace, an artifact another deployment holds. None of
    // them has to be an evidence node, and a read that refused such a graph would make
    // partiality an error.
    let mut graph = EvidenceGraph::new();
    let observed = seed("trace_9f", &["cap_grant1", "crash_c0"]);
    graph.add_node(observed.clone());

    let closure = DerivationClosure::of(&graph, [&observed]);
    assert_eq!(subjects(&closure), ["trace_9f@0"]);
    assert_eq!(absences(&closure), ["cap_grant1@1", "crash_c0@1"]);
    assert_eq!(
        closure.incompleteness(),
        [Incompleteness::AbsentInputs { count: 2 }]
    );
    // Each absence names the node that reached for it, so a caller can act on it.
    for absent in closure.absent() {
        assert_eq!(
            absent.named_by().collect::<Vec<_>>(),
            [&observed.identity()]
        );
    }

    // Anti-vacuity: hold a node about one of them and it moves from absent to reached.
    graph.add_node(seed("crash_c0", &[]));
    let closure = DerivationClosure::of(&graph, [&observed]);
    assert_eq!(subjects(&closure), ["crash_c0@1", "trace_9f@0"]);
    assert_eq!(absences(&closure), ["cap_grant1@1"]);
}

#[test]
fn the_closure_says_which_kind_of_incomplete_it_is() {
    // INV-008: "never a bare boolean". Three graphs over one shape, and the answer names a
    // different reason each time — which a single flag could not do.
    let mut graph = EvidenceGraph::new();
    let deep = seed("trace_deep", &[]);
    graph.add_node(deep);
    note("wb_note1", entry("claim-goal", "prop_9f", &["trace_deep"]))
        .compile(&graph, &EpochSet::unpinned())
        .expect("every reference resolves")
        .apply(&mut graph)
        .expect("appended");
    let proposal = graph
        .nodes_of_kind(NodeKind::Property)
        .next()
        .expect("one property")
        .1
        .clone();

    // Unbounded: only the note is missing.
    let whole = DerivationClosure::of(&graph, [&proposal]);
    assert_eq!(
        whole.incompleteness(),
        [Incompleteness::AbsentInputs { count: 1 }]
    );
    // Bounded at zero: nothing was walked at all, so the bound is the only thing to say.
    let none = DerivationClosure::bounded(&graph, [&proposal], Some(0));
    assert_eq!(
        none.incompleteness(),
        [Incompleteness::Bound { max_depth: 0 }]
    );
    assert_eq!(none.absent_count(), 0);
    // A seed with nothing to derive from is complete under any bound.
    let alone = DerivationClosure::bounded(
        &graph,
        [graph
            .nodes_of_kind(NodeKind::Run)
            .next()
            .expect("the seed")
            .1],
        Some(0),
    );
    assert!(alone.is_complete());
    assert_eq!(alone.incompleteness(), []);
}

#[test]
fn closing_over_a_graph_appends_nothing_and_promotes_nothing() {
    // P1/V5: the query is a read. Nothing here can reach `promote`, and the graph is
    // byte-identical afterwards by the only measure this crate has — what it holds.
    let mut graph = EvidenceGraph::new();
    graph.add_node(seed("trace_9f", &[]));
    note("wb_note1", entry("claim-goal", "prop_9f", &["trace_9f"]))
        .with_decisions([
            Decision::new(entry("claim-dec", "dec_9f", &["trace_9f"])).expect("supported")
        ])
        .compile(&graph, &EpochSet::unpinned())
        .expect("every reference resolves")
        .apply(&mut graph)
        .expect("appended");

    let before: Vec<(String, ClaimStatus, usize)> = graph
        .nodes()
        .map(|(_, node)| (node.artifact().to_string(), node.status(), node.version()))
        .collect();
    let node_count = graph.node_count();
    let edge_count = graph.edge_count();

    let closure = DerivationClosure::of_graph(&graph);
    assert_eq!(closure.reached_count(), node_count);

    let after: Vec<(String, ClaimStatus, usize)> = graph
        .nodes()
        .map(|(_, node)| (node.artifact().to_string(), node.status(), node.version()))
        .collect();
    assert_eq!(before, after);
    assert_eq!(graph.node_count(), node_count);
    assert_eq!(graph.edge_count(), edge_count);
    // Every row reports the bottom of the lattice, because that is what a proposal holds.
    for (_, row) in closure.reached() {
        assert_eq!(row.status(), ClaimStatus::Proposed);
    }
}

#[test]
fn a_graph_that_cites_nothing_closes_to_itself() {
    // Boundary: a graph whose records name no input is its own ancestry, complete, at
    // depth 0 — an empty absence list is an answer and not a gap.
    let mut graph = EvidenceGraph::new();
    graph.add_node(seed("trace_9f", &[]));
    graph.add_node(seed("crash_c0", &[]));

    let closure = DerivationClosure::of_graph(&graph);
    assert_eq!(subjects(&closure), ["crash_c0@0", "trace_9f@0"]);
    assert_eq!(closure.absent_count(), 0);
    assert_eq!(closure.depth(), 0);
    assert!(closure.is_complete());
    assert_eq!(closure.max_depth(), None);
}
