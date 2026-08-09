//! The whiteboard view: held graph state, rendered into plan §11.5's seven sections
//! (docs/44 "Whiteboard", RFC 0038 V1–V6).
//!
//! # The direction this module runs
//!
//! > A human-friendly whiteboard view permits provisional notes.
//! >
//! > — docs/44, "Whiteboard"
//!
//! [`crate::whiteboard`] is the note → proposals door and is one-way; this is the other
//! direction, graph → sections. RFC 0038 recorded it as undelivered with three things
//! missing — "no schema, no surface, and no decided semantics for which held nodes belong
//! in which section" — and V1–V6 decide all three. Each answer is a property of what a
//! view *is* rather than a check it runs, the same shape W1–W9 have.
//!
//! # V1 — the view is a projection of a node set, and the caller chooses the set
//!
//! [`WhiteboardView::of`] takes nodes; [`WhiteboardView::of_graph`] is the whole-graph
//! convenience. Scope is not this module's to pick: [`crate::graph::EvidenceGraph`] already
//! ships the selection layer — by kind, by status, by claim, by producer, and the incident
//! edges of a node — and a view that chose its own would be choosing for its caller exactly
//! as a compiler that quietly pinned no epochs would (W9's own argument). A caller that
//! wants docs/44's "only relevant graph slice" composes the selection it means.
//!
//! # V2 — section membership is W2 inverted
//!
//! [`Section::of_kind`] is [`Section::node_kind`] read backwards, and it is a function
//! because W2's mapping is injective: six proposing sections name six distinct kinds. An
//! ambiguous inverse would mean a held node belonged in two sections and the view would
//! have to choose between them, which is the semantic judgement W7 says this vocabulary may
//! not make.
//!
//! The other fourteen of plan §11.2's twenty kinds belong in **no** section, and a view
//! neither places them nor drops them silently: [`WhiteboardView::unrendered`] counts them
//! by kind. A view that quietly omitted fourteen twentieths of the node vocabulary would be
//! reporting a partial graph as if it were the whole one — the empty-success shape
//! `rule errors.unsupported_surface` forbids, arrived at from the reading side.
//!
//! # V3 — every section is a list, `Goal` included
//!
//! W1 gives a *note* exactly one goal, because plan §11.5 opens "for complex invention
//! *tasks*" and spells `Goal` singular. That is a constraint on an authored note and not on
//! the graph, which holds as many `property` nodes as anyone published. A view that picked
//! one to be *the* goal would be choosing, and W8's "nothing is ever dropped" read in
//! reverse forbids it, so [`WhiteboardView::section`] answers a slice for all seven.
//!
//! # V4 — the `Experiment` section is present and empty by construction
//!
//! W3 makes an experiment a task proposal and never a node, and RFC 0038 records that a
//! task proposal "remains a value and not an artifact" — nothing publishes one, and plan
//! §11.2's twenty kinds have no task kind. So no held artifact can appear under
//! `Experiment`, and [`Section::of_kind`] can never answer it. It is present-and-empty
//! rather than absent for W1's own reason: an omitted section and an empty section would be
//! two spellings of one view.
//!
//! # V5 — a row reports the status the graph holds, and the view never promotes
//!
//! [`ViewRow::status`] is [`crate::node::EvidenceNode::status`], verbatim. Nothing here
//! reads a lattice, compares a version, or consults a [`crate::authority::Promotion`] —
//! there is no code path from this module to [`crate::graph::EvidenceGraph::promote`],
//! which is the same compile-time device the compiler gets its half of the rule from.
//!
//! # V6 — a view is not a note, and this is the decision, not an omission
//!
//! The natural-looking answer would be to render the graph *as* a
//! `whiteboard-note.schema.json` document, so the two directions shared one format. It is
//! the wrong one, for three reasons, and the first is decisive:
//!
//! - **A note has no `status` member, deliberately** (W5: "the format has no status member,
//!   and the compiler does not read prose"). That absence is a guard on the *input* side —
//!   a field a producer can fill is a field a producer can lie in. Rendering held state into
//!   that format would erase every status the graph holds and republish `validated`,
//!   `refuted` and `superseded` content in the one shape whose whole meaning is "proposed,
//!   asserting nothing". W5's device does not cover this direction, so the view must carry
//!   the status and therefore cannot be a note.
//! - **A note requires `text`, and the graph holds none.** INV-003 puts human commentary
//!   beside a machine result and never inside one, which is exactly why no node carries
//!   prose; a view that synthesized a sentence to satisfy `minLength: 1` would be inventing
//!   the one thing the format says an author supplies.
//! - **A note-shaped view would be replayable as an authored note.** `whiteboard.compile`
//!   accepts any conforming document, so a view that conformed could be handed straight back
//!   as a fresh proposal set under a new author — a laundering channel by construction, and
//!   the one INV-016 reading a read-only surface can still get wrong.
//!
//! So the view shares [`Section`] with the compiler — one decision about the seven headings,
//! not two — and nothing else. It is a computed value rather than an artifact class: it
//! mints no identity, publishes nothing, is not one of plan §4.4's content-addressed
//! classes, and therefore needs no schema document, exactly as
//! [`crate::graph::Contradiction`] and [`crate::whiteboard::Compilation`] need none.
//!
//! # Nodes, not edges — deliberately
//!
//! plan §11.5's seven headings are headings over *claims*, and a row is a claim. A
//! decision's supporting edges (W6) are not re-sectioned here: they are already answerable
//! from a row's identity through [`crate::graph::EvidenceGraph::edges_to`], and inventing a
//! second place to read them would be two answers to one question. What a view must not do
//! is show a decision as though it had no support, and it does not: a row names the node, and
//! the graph names the edges.
//!
//! # This is the library, not the wire
//!
//! There is no `whiteboard.view` operation, and the declared surface was searched before
//! that was concluded — the shape W10 required of the compiler. `evidence.query` answers
//! two handle lists, `evidence.get` answers one artifact, `evidence.subscribe` carries
//! deltas under `evidence.query`'s own predicate, and `context.compile` answers a
//! `context-pack.schema.json` pack, which is a different artifact class with a `question`
//! and declared `guarantees`. None of them can carry a sectioned, status-bearing report
//! without a response member that does not exist, and adding one is a wire change. The
//! crate-level typed surface therefore lands first and the wire spelling lands when its RFC
//! decides it — bn-2qa0u's own order, and `continuum-forge`'s before it.
//!
//! # Clause → test
//!
//! | Clause | Source | Test |
//! |---|---|---|
//! | the caller chooses the scope | V1 | `a_view_renders_the_nodes_it_was_given_and_no_others` |
//! | section membership is W2 inverted | V2 | `the_inverse_is_total_on_the_six_and_empty_on_the_fourteen` |
//! | an unmapped kind is counted, never placed | V2 | `an_unrendered_kind_is_counted_and_appears_in_no_section` |
//! | every section is a list | V3 | `every_section_is_a_list_including_goal` |
//! | `Experiment` is present and empty | V4 | `the_experiment_section_is_present_and_empty` |
//! | a status renders as the graph holds it | V5 | `a_view_reports_every_status_verbatim` |
//! | a view is not a note | V6 | `tests/whiteboard_view.rs` |
//! | deterministic | GOV-1-03, INV-005 | `two_insertion_orders_render_identically` |

use std::collections::BTreeMap;

use continuum_value::value::{Name, Value};

use crate::actor::{ActorId, field};
use crate::claim_status::ClaimStatus;
use crate::graph::EvidenceGraph;
use crate::identity::{EvidenceIdentity, EvidenceNaming};
use crate::node::{ClaimId, EvidenceNode, Label, NodeKind};
use crate::provenance::ArtifactRef;
use crate::whiteboard::Section;

/// One claim, as a whiteboard section shows it.
///
/// Everything on it is read off the held node. There is no derived field, no computed
/// verdict, and no summary: a view reports, and a row that said anything the graph does not
/// already hold would be the view asserting rather than reading.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ViewRow {
    identity: EvidenceIdentity,
    kind: NodeKind,
    status: ClaimStatus,
    claim: ClaimId,
    subject: ArtifactRef,
    actor: ActorId,
    labels: Vec<Label>,
}

impl ViewRow {
    /// The graph's key for this node, so a reader can go back to the record.
    #[must_use]
    pub const fn identity(&self) -> &EvidenceIdentity {
        &self.identity
    }

    /// The node kind, which is what put the row in its section (V2).
    #[must_use]
    pub const fn kind(&self) -> NodeKind {
        self.kind
    }

    /// The status the graph holds, verbatim (V5).
    #[must_use]
    pub const fn status(&self) -> ClaimStatus {
        self.status
    }

    /// The claim identity this node's promotions are linearized against (plan §11.7).
    #[must_use]
    pub const fn claim(&self) -> &ClaimId {
        &self.claim
    }

    /// The artifact the node is *about* — a whiteboard entry's `subject`, in reverse.
    #[must_use]
    pub const fn subject(&self) -> &ArtifactRef {
        &self.subject
    }

    /// Who produced it (docs/44 "Credit and provenance").
    #[must_use]
    pub const fn actor(&self) -> &ActorId {
        &self.actor
    }

    /// The node's labels, in the node's own order.
    ///
    /// A node a whiteboard note proposed carries `whiteboard:<section key>` here. That
    /// label is *not* what placed the row — V2 places by kind, so a view shows graph state
    /// and not only what a whiteboard wrote — but it is carried through, because which note
    /// section proposed a node is recorded nowhere else once the note is gone (W9).
    pub fn labels(&self) -> impl ExactSizeIterator<Item = &Label> {
        self.labels.iter()
    }

    /// This row as a record: the note's own entry members, plus the status a note has no
    /// member for.
    fn to_record(&self, naming: &impl EvidenceNaming) -> Value {
        Value::record([
            (field("actor"), Value::text(self.actor.as_str())),
            (field("claim"), Value::text(self.claim.as_str())),
            (
                field("evidence"),
                Value::text(naming.name(&self.identity).as_str()),
            ),
            (field("kind"), Value::text(self.kind.as_str())),
            (
                field("labels"),
                Value::seq(self.labels.iter().map(|it| Value::text(it.as_str())))
                    .expect("a label list nests one level"),
            ),
            (field("status"), Value::text(self.status.as_str())),
            (field("subject"), self.subject.to_value()),
        ])
        .expect("the seven member names are distinct compile-time constants")
    }
}

/// Held graph state in plan §11.5's seven sections.
///
/// Built by [`WhiteboardView::of`]. Immutable, and a function of the nodes it was given:
/// rows arrive in the order the caller iterated, which for every
/// [`EvidenceGraph`] accessor is content order (GOV-1-03, INV-005).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WhiteboardView {
    /// One slot per [`Section::ALL`] member, in that order. An array rather than a map, so
    /// "every section is present" (V3, V4) is a property of the type.
    sections: [Vec<ViewRow>; Section::ALL.len()],
    unrendered: BTreeMap<NodeKind, usize>,
}

impl WhiteboardView {
    /// Render a set of nodes.
    ///
    /// The caller supplies the scope (V1). Each node lands in the section its kind names
    /// (V2); a node whose kind names none is counted in [`Self::unrendered`] and placed
    /// nowhere.
    pub fn of<'a>(nodes: impl IntoIterator<Item = &'a EvidenceNode>) -> Self {
        let mut view = Self {
            sections: core::array::from_fn(|_| Vec::new()),
            unrendered: BTreeMap::new(),
        };
        for node in nodes {
            let Some(section) = Section::of_kind(node.kind()) else {
                *view.unrendered.entry(node.kind()).or_insert(0) += 1;
                continue;
            };
            view.sections[slot(section)].push(ViewRow {
                identity: node.identity(),
                kind: node.kind(),
                status: node.status(),
                claim: node.claim_id().clone(),
                subject: node.artifact().clone(),
                actor: node.provenance().actor().clone(),
                labels: node.labels().cloned().collect(),
            });
        }
        view
    }

    /// Render every node a graph holds.
    ///
    /// The whole-graph reading of V1, and the only scope this module names for itself —
    /// because it is the one scope that chooses nothing.
    #[must_use]
    pub fn of_graph(graph: &EvidenceGraph) -> Self {
        Self::of(graph.nodes().map(|(_, node)| node))
    }

    /// The rows of one section, in the order they were rendered.
    ///
    /// Never absent: [`Section::Experiment`] answers an empty slice by construction (V4),
    /// and so does any section the scope reached nothing for.
    #[must_use]
    pub fn section(&self, section: Section) -> &[ViewRow] {
        &self.sections[slot(section)]
    }

    /// Every section, in plan §11.5's order, whether or not it has rows.
    pub fn sections(&self) -> impl ExactSizeIterator<Item = (Section, &[ViewRow])> {
        Section::ALL
            .into_iter()
            .enumerate()
            .map(|(slot, section)| (section, self.sections[slot].as_slice()))
    }

    /// How many nodes of each kind no section names, in kind order (V2).
    ///
    /// Empty exactly when every node the scope reached was placed. This is the view's
    /// statement that it is a projection and not a listing: fourteen of plan §11.2's twenty
    /// kinds have no heading in plan §11.5, and saying so is the difference between a
    /// partial answer and a wrong one.
    pub fn unrendered(&self) -> impl ExactSizeIterator<Item = (NodeKind, usize)> + '_ {
        self.unrendered.iter().map(|(kind, count)| (*kind, *count))
    }

    /// How many nodes in total no section named.
    #[must_use]
    pub fn unrendered_count(&self) -> usize {
        self.unrendered.values().sum()
    }

    /// How many rows the view placed.
    #[must_use]
    pub fn row_count(&self) -> usize {
        self.sections.iter().map(Vec::len).sum()
    }

    /// This view as a record: seven section keys, plus what it did not render.
    ///
    /// The section keys are [`Section::schema_key`] — the note schema's own spelling, so a
    /// reader of either document finds the same seven names. What the record deliberately
    /// does **not** carry is `schema_id`, `schema_epoch`, `author`, `created_at`, `note_id`
    /// or `text`, and what it does carry that a note may not is `status` on every row: a
    /// view is not a note (V6), and the record says so in both directions rather than
    /// relying on a comment. `naming` is a parameter for the reason
    /// [`crate::conflict::Conflict::materialize`]'s is — RFC 0038 D2 puts naming in the
    /// deployment's seam, and a view that named artifacts for its caller would be choosing
    /// an identity kernel.
    pub fn to_record(&self, naming: &impl EvidenceNaming) -> Value {
        let mut fields: Vec<(Name, Value)> = Section::ALL
            .into_iter()
            .enumerate()
            .map(|(slot, section)| {
                (
                    field(section.schema_key()),
                    Value::seq(self.sections[slot].iter().map(|row| row.to_record(naming)))
                        .expect("a row list nests"),
                )
            })
            .collect();
        fields.push((
            field("unrendered"),
            Value::seq(self.unrendered.iter().map(|(kind, count)| {
                Value::record([
                    (field("count"), Value::nat(*count as u128)),
                    (field("kind"), Value::text(kind.as_str())),
                ])
                .expect("the two member names are distinct compile-time constants")
            }))
            .expect("an unrendered list nests"),
        ));
        Value::record(fields).expect("the member names are distinct compile-time constants")
    }
}

/// Which slot of [`WhiteboardView::sections`] a section occupies.
///
/// Read off [`Section::ALL`] rather than matched, so the array's order cannot drift from the
/// vocabulary's.
///
/// # Panics
///
/// Never: [`Section::ALL`] is total over [`Section`].
fn slot(section: Section) -> usize {
    Section::ALL
        .into_iter()
        .position(|it| it == section)
        .expect("Section::ALL is total over Section")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::authority::ServiceRole;
    use crate::authority::tests::service;
    use crate::identity::DefaultNaming;
    use crate::node::tests::node;

    /// A graph holding one node of every kind, about one artifact each.
    fn every_kind() -> EvidenceGraph {
        let mut graph = EvidenceGraph::new();
        for kind in NodeKind::ALL {
            graph.add_node(node(kind, &format!("art_{}", kind.as_str()), "claim-1"));
        }
        graph
    }

    #[test]
    fn the_inverse_is_total_on_the_six_and_empty_on_the_fourteen() {
        // V2. `Section::of_kind` is `Section::node_kind` read backwards, and the round trip
        // holds in both directions — which is what makes the inverse a function.
        let mapped: Vec<NodeKind> = NodeKind::ALL
            .into_iter()
            .filter(|kind| Section::of_kind(*kind).is_some())
            .collect();
        assert_eq!(
            mapped,
            [
                NodeKind::Assumption,
                NodeKind::Property,
                NodeKind::ProofGoal,
                NodeKind::InvariantCandidate,
                NodeKind::Counterexample,
                NodeKind::Decision,
            ]
        );
        for section in Section::ALL {
            match section.node_kind() {
                Some(kind) => assert_eq!(Section::of_kind(kind), Some(section), "{section}"),
                // V4: no held node can name `Experiment`.
                None => assert_eq!(section, Section::Experiment),
            }
        }
    }

    #[test]
    fn a_view_renders_the_nodes_it_was_given_and_no_others() {
        // V1: the caller chooses the scope, and the view adds nothing to it. The same graph
        // seen through two selections gives two views.
        let graph = every_kind();
        let whole = WhiteboardView::of_graph(&graph);
        assert_eq!(whole.row_count(), 6);
        assert_eq!(whole.unrendered_count(), 14);

        let one_kind = WhiteboardView::of(
            graph
                .nodes_of_kind(NodeKind::Property)
                .map(|(_, node)| node),
        );
        assert_eq!(one_kind.row_count(), 1);
        assert_eq!(one_kind.unrendered_count(), 0);
        assert_eq!(one_kind.section(Section::Goal).len(), 1);
        assert!(one_kind.section(Section::KnownFact).is_empty());
    }

    #[test]
    fn an_unrendered_kind_is_counted_and_appears_in_no_section() {
        // V2: a view is a projection and says so. The fourteen unmapped kinds are counted by
        // kind and placed nowhere — never silently dropped.
        let view = WhiteboardView::of_graph(&every_kind());
        let counted: Vec<NodeKind> = view.unrendered().map(|(kind, _)| kind).collect();
        assert_eq!(counted.len(), 14);
        for (kind, count) in view.unrendered() {
            assert_eq!(count, 1, "{kind}");
            assert!(Section::of_kind(kind).is_none(), "{kind}");
        }
        for (_, rows) in view.sections() {
            for row in rows {
                assert!(Section::of_kind(row.kind()).is_some());
            }
        }
        // Anti-vacuity: nothing is lost. Every held node is either a row or a count.
        assert_eq!(
            view.row_count() + view.unrendered_count(),
            every_kind().node_count()
        );
    }

    #[test]
    fn every_section_is_a_list_including_goal() {
        // V3: W1's "exactly one goal" constrains an authored note, not the graph. Two
        // `property` nodes are two goal rows, and picking one would be choosing.
        let mut graph = EvidenceGraph::new();
        graph.add_node(node(NodeKind::Property, "prop_a", "claim-1"));
        graph.add_node(node(NodeKind::Property, "prop_b", "claim-2"));
        let view = WhiteboardView::of_graph(&graph);
        assert_eq!(view.section(Section::Goal).len(), 2);
        assert_eq!(view.sections().len(), 7);
    }

    #[test]
    fn the_experiment_section_is_present_and_empty() {
        // V4: an experiment is a task proposal and never a node (W3), so no held artifact
        // can appear here — but the section is present, because an omitted section and an
        // empty one would be two spellings of one view (W1's reason).
        let view = WhiteboardView::of_graph(&every_kind());
        assert!(view.section(Section::Experiment).is_empty());
        let present: Vec<Section> = view.sections().map(|(section, _)| section).collect();
        assert_eq!(present, Section::ALL);
    }

    #[test]
    fn a_view_reports_every_status_verbatim() {
        // V5: a row's status is the node's, for every member of the plan §11.4 lattice that
        // a promotion can reach. Nothing here upgrades, defaults, or summarizes.
        for status in ClaimStatus::ALL {
            let mut graph = EvidenceGraph::new();
            let identity = graph
                .add_node(node(NodeKind::Property, "prop_9f", "claim-1"))
                .identity()
                .clone();
            // Every role, so each reachable status is installed by one authorized to.
            let declared = service(
                "service:promoter",
                &[
                    ServiceRole::Execution,
                    ServiceRole::Verification,
                    ServiceRole::IndependentChecker,
                    ServiceRole::ProofService,
                    ServiceRole::PolicyOwner,
                ],
            );
            if let Ok(promotion) = declared.promote_to(status) {
                graph
                    .promote(&identity, ClaimStatus::Proposed, &promotion)
                    .expect("an authorized promotion from the bottom");
            }
            let view = WhiteboardView::of_graph(&graph);
            let row = &view.section(Section::Goal)[0];
            let held = graph.node(&identity).expect("held").status();
            assert_eq!(row.status(), held, "{status}");
        }
    }

    #[test]
    fn a_row_carries_the_section_label_but_is_not_placed_by_it() {
        // V2: placement is by kind, so a view shows graph state and not only what a
        // whiteboard wrote. A node with a *contradicting* label still lands by its kind.
        let mut graph = EvidenceGraph::new();
        graph.add_node(
            node(NodeKind::Property, "prop_9f", "claim-1")
                .with_labels([Section::Counterexample.label()]),
        );
        let view = WhiteboardView::of_graph(&graph);
        assert!(view.section(Section::Counterexample).is_empty());
        let row = &view.section(Section::Goal)[0];
        let labels: Vec<&str> = row.labels().map(Label::as_str).collect();
        assert_eq!(labels, ["whiteboard:counterexamples"]);
    }

    #[test]
    fn two_insertion_orders_render_identically() {
        // GOV-1-03, INV-005: the graph iterates in content order, so a view of it is a
        // function of what is held and not of when it arrived.
        let members = [
            node(NodeKind::Property, "prop_c", "claim-1"),
            node(NodeKind::Assumption, "fact_a", "claim-2"),
            node(NodeKind::Decision, "dec_b", "claim-3"),
        ];
        let mut forward = EvidenceGraph::new();
        for member in &members {
            forward.add_node(member.clone());
        }
        let mut backward = EvidenceGraph::new();
        for member in members.iter().rev() {
            backward.add_node(member.clone());
        }
        assert_eq!(
            WhiteboardView::of_graph(&forward),
            WhiteboardView::of_graph(&backward)
        );
        let naming = DefaultNaming::new();
        assert_eq!(
            WhiteboardView::of_graph(&forward).to_record(&naming),
            WhiteboardView::of_graph(&backward).to_record(&naming)
        );
    }

    #[test]
    fn the_record_names_the_schemas_seven_sections_and_what_was_not_rendered() {
        let view = WhiteboardView::of_graph(&every_kind());
        let Value::Record(record) = view.to_record(&DefaultNaming::new()) else {
            panic!("a view renders as a record");
        };
        let mut members: Vec<String> = record.keys().map(|it| it.as_str().to_owned()).collect();
        members.sort();
        assert_eq!(
            members,
            [
                "candidate_invariants",
                "counterexamples",
                "decisions",
                "experiments",
                "goal",
                "known_facts",
                "unrendered",
                "unresolved_obligations",
            ]
        );
    }

    #[test]
    fn an_empty_graph_renders_seven_empty_sections() {
        // Boundary: nothing held is a view of nothing, not an absence of a view.
        let view = WhiteboardView::of_graph(&EvidenceGraph::new());
        assert_eq!(view.row_count(), 0);
        assert_eq!(view.unrendered_count(), 0);
        assert_eq!(view.sections().len(), 7);
        for (_, rows) in view.sections() {
            assert!(rows.is_empty());
        }
    }
}
