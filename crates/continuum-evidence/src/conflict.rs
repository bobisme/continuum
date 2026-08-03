//! Conflicts as first-class nodes, and resolution as a recorded transition
//! (plan §11.7, docs/44 "Conflict handling", PR-7 / IMPL-04).
//!
//! # A conflict is a node, not a decision
//!
//! > Concurrent contradictory claims materialize a `Conflict` node rather than resolving by
//! > write order.
//! >
//! > — plan §11.7
//!
//! > Conflicts are first-class nodes:
//! >
//! > ```text
//! > Abstraction A maps reply publication to Ack
//! > Abstraction B maps storage stability to Ack
//! > Conflict: both cannot satisfy observed correspondence
//! > Required experiment/proof: distinguish observer contract
//! > ```
//! >
//! > A coordinator cannot resolve conflict by selecting the most confident agent answer.
//! >
//! > — docs/44, "Conflict handling"
//!
//! docs/44's block has four lines and three of them are structure: the subjects (two here,
//! at least two in general), the ground on which they cannot both hold, and the experiment
//! or proof that would distinguish them. [`Conflict`] carries all three, and **all three are
//! mandatory** — a conflict with one subject is not a conflict, and a conflict with no
//! required experiment is a dead end nobody can act on, which is the shape "unresolved
//! contradictions remain visible" (docs/44's whiteboard rules) exists to prevent.
//!
//! # Never silently choose
//!
//! Nothing in this module, and nothing in [`crate::graph`], ranks the subjects of a
//! conflict. There is no "most confident" field to read, no join over the parties'
//! statuses, and no automatic resolution: [`crate::claim_status::ClaimStatus::join`] already
//! records that it "is the lattice operation and *not* a conflict-resolution rule".
//!
//! [`Resolution`] is what a coordinator produces instead, and it is refused unless it names
//! **evidence** — an artifact, not a vote. That is docs/44's sentence turned into a
//! constructor: the resolution of a conflict is a claim about the world, and a claim about
//! the world has an artifact behind it (INV-018's and GOV-1-12's shared spirit: an ambiguity
//! is an error, and choosing silently is how it stops being visible).
//!
//! # Resolution is a transition, not a deletion
//!
//! docs/44's authority table gives exactly one row for retiring a claim —
//! `any → superseded | policy/owner with explicit edge` — and plan §4.6 says what the edge
//! is: "re-derived artifacts receive new identities linked to their predecessors by
//! `SUPERSEDES` edges; nothing is rewritten in place".
//!
//! [`crate::graph::EvidenceGraph::resolve_conflict`] is those two sentences and nothing
//! else. It appends a `SUPERSEDES` edge from a `decision` node to the conflict node, then
//! promotes the conflict node to `superseded` through the ordinary
//! [`crate::graph::EvidenceGraph::promote`] path — so INV-004 and the compare-and-set both
//! still apply — and it removes nothing. The conflicting subjects, the `CONFLICTS_WITH`
//! edges between them, and the conflict node itself are all still held afterwards, at the
//! versions they were; only the conflict's *status* moved, and its history says so.
//!
//! # What a conflict can name, and one thing it cannot link
//!
//! [`ConflictSubject`] is a node **or** an edge, because a contradiction is often between
//! two assertions rather than two artifacts (a `SUPPORTS` and a `REFUTES` over one claim is
//! [`crate::graph::EvidenceGraph::contradictions`]' whole output). Materialization draws
//! what the graph's shape allows and says what it does not:
//!
//! - between two **node** subjects, a `CONFLICTS_WITH` edge — the plan §11.3 relation, one
//!   per unordered pair, in canonical identity order, because the relation is symmetric and
//!   two spellings of one fact would be two content identities for it;
//! - from the conflict node to each **node** subject, a `DERIVED_FROM` edge — the conflict
//!   *is* derived from the claims it is about;
//! - an **edge** subject cannot be an endpoint of anything, because the graph's edges join
//!   nodes. It is named instead in the conflict node's `provenance.inputs`, which is exactly
//!   the "from-what-inputs" member, and [`Conflict::materialize`] puts every subject's
//!   handle there — nodes included — so the record of what a conflict is about is complete
//!   in one schema-conforming place whether or not an edge could be drawn to it.
//!
//! # Clause → test
//!
//! | Clause | Source | Test |
//! |---|---|---|
//! | a conflict has at least two subjects | docs/44's block | `a_conflict_needs_two_subjects` |
//! | ground and required experiment are mandatory | docs/44's block | `a_conflict_states_its_ground_and_its_experiment` |
//! | contradictions materialize a conflict node | plan §11.7 | `a_contradiction_materializes_a_conflict_node` |
//! | provenance names every subject | docs/44, node schema | `the_conflict_node_names_every_subject_as_an_input` |
//! | `CONFLICTS_WITH` is symmetric, written once | ADR-0013 | `conflicts_with_edges_are_written_once_per_pair` |
//! | resolution names evidence, not confidence | docs/44 | `a_resolution_without_evidence_is_refused` |
//! | resolution is a transition, not a deletion | plan §4.6, docs/44 | `crate::graph`'s `resolving_a_conflict_deletes_nothing` |
//! | only a policy owner retires a claim | docs/44 | `crate::graph`'s `only_a_superseding_promotion_resolves` |

use core::fmt;
use std::collections::BTreeSet;

use crate::edge::{EdgeRef, EdgeRelation, EvidenceEdge};
use crate::identity::{EvidenceHandle, EvidenceNaming};
use crate::node::{ClaimId, EvidenceNode, IdempotencyKey, NodeKind, NodeRef};
use crate::provenance::{ArtifactRef, Provenance};

/// One party to a conflict.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum ConflictSubject {
    /// A node — a candidate, an abstraction map, a claim.
    Node(NodeRef),
    /// An edge — an assertion, such as the `SUPPORTS` half of a support/refute pair.
    Edge(EdgeRef),
}

impl ConflictSubject {
    /// The `ev_` handle this subject is named by under `naming`.
    #[must_use]
    pub fn handle<N: EvidenceNaming + ?Sized>(&self, naming: &N) -> EvidenceHandle {
        match self {
            Self::Node(reference) => naming.name(reference.identity()),
            Self::Edge(reference) => naming.name(reference.identity()),
        }
    }

    /// The node reference, when this subject is a node.
    #[must_use]
    pub const fn as_node(&self) -> Option<&NodeRef> {
        match self {
            Self::Node(reference) => Some(reference),
            Self::Edge(_) => None,
        }
    }
}

/// Why the parties cannot both hold — docs/44's `Conflict:` line.
///
/// Prose, deliberately, because docs/44's own example is prose ("both cannot satisfy
/// observed correspondence") and no vocabulary for it exists anywhere in the dossier.
/// Inventing a closed enum here would be this crate deciding a taxonomy the dossier has not
/// written; refusing the empty string is the whole of what can honestly be enforced.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ConflictGround(String);

impl ConflictGround {
    /// State the ground.
    ///
    /// # Errors
    ///
    /// [`ConflictRefusal::EmptyGround`] for the empty string.
    pub fn new(text: &str) -> Result<Self, ConflictRefusal> {
        if text.is_empty() {
            return Err(ConflictRefusal::EmptyGround);
        }
        Ok(Self(text.to_owned()))
    }

    /// The stated ground.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ConflictGround {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// What would distinguish the parties — docs/44's `Required experiment/proof:` line.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RequiredExperiment(String);

impl RequiredExperiment {
    /// State the experiment or proof.
    ///
    /// # Errors
    ///
    /// [`ConflictRefusal::EmptyExperiment`] for the empty string. A conflict nobody can act
    /// on is a conflict that stops being visible.
    pub fn new(text: &str) -> Result<Self, ConflictRefusal> {
        if text.is_empty() {
            return Err(ConflictRefusal::EmptyExperiment);
        }
        Ok(Self(text.to_owned()))
    }

    /// The stated experiment or proof.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for RequiredExperiment {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Two or more claims in tension, before it is written into a graph.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Conflict {
    subjects: BTreeSet<ConflictSubject>,
    ground: ConflictGround,
    required_experiment: RequiredExperiment,
}

/// A conflict written out as the records a graph can hold.
///
/// Produced by [`Conflict::materialize`] and consumed by
/// [`crate::graph::EvidenceGraph::add_conflict`], which appends the node before the edges so
/// no edge is ever offered against an endpoint the graph does not yet hold.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MaterializedConflict {
    node: EvidenceNode,
    edges: Vec<EvidenceEdge>,
}

impl MaterializedConflict {
    /// The `conflict` node (plan §11.2).
    #[must_use]
    pub const fn node(&self) -> &EvidenceNode {
        &self.node
    }

    /// The edges that link it to what it is about, in the order they must be appended.
    pub fn edges(&self) -> impl ExactSizeIterator<Item = &EvidenceEdge> {
        self.edges.iter()
    }
}

impl Conflict {
    /// Declare a conflict between two or more subjects.
    ///
    /// # Errors
    ///
    /// [`ConflictRefusal::TooFewSubjects`] when fewer than two *distinct* subjects are
    /// given. One claim is not in tension with itself, and a set collapses a repeated
    /// subject rather than counting it twice.
    pub fn between(
        subjects: impl IntoIterator<Item = ConflictSubject>,
        ground: ConflictGround,
        required_experiment: RequiredExperiment,
    ) -> Result<Self, ConflictRefusal> {
        let subjects: BTreeSet<ConflictSubject> = subjects.into_iter().collect();
        if subjects.len() < 2 {
            return Err(ConflictRefusal::TooFewSubjects {
                count: subjects.len(),
            });
        }
        Ok(Self {
            subjects,
            ground,
            required_experiment,
        })
    }

    /// The parties, in canonical order.
    pub fn subjects(&self) -> impl ExactSizeIterator<Item = &ConflictSubject> {
        self.subjects.iter()
    }

    /// Why they cannot both hold.
    #[must_use]
    pub const fn ground(&self) -> &ConflictGround {
        &self.ground
    }

    /// What would distinguish them.
    #[must_use]
    pub const fn required_experiment(&self) -> &RequiredExperiment {
        &self.required_experiment
    }

    /// Write this conflict out as a `conflict` node and the edges that link it.
    ///
    /// The node's provenance is the caller's, with every subject's handle added to
    /// `inputs` — so what the conflict is about is recorded in a schema-conforming member
    /// even for a subject no edge can reach. `naming` is the [`EvidenceNaming`] seam, so the
    /// handles written here follow whatever derivation the deployment (and eventually
    /// bn-3sypm) uses, and this function commits to none.
    #[must_use]
    pub fn materialize<N: EvidenceNaming + ?Sized>(
        &self,
        naming: &N,
        artifact: ArtifactRef,
        claim_id: ClaimId,
        idempotency_key: IdempotencyKey,
        provenance: Provenance,
    ) -> MaterializedConflict {
        let inputs: Vec<ArtifactRef> = self
            .subjects
            .iter()
            .filter_map(|subject| ArtifactRef::new(subject.handle(naming).as_str()).ok())
            .collect();
        let mut merged = Provenance::new(
            provenance.actor().clone(),
            provenance.created_at().clone(),
            provenance.inputs().cloned().chain(inputs),
        )
        .under_epochs(provenance.epochs().clone());
        if let Some(tool) = provenance.tool() {
            merged = merged.with_tool(tool.clone());
        }

        let node = EvidenceNode::propose(
            NodeKind::Conflict,
            artifact,
            claim_id,
            idempotency_key,
            merged,
        );
        let conflict_ref = NodeRef::of(&node);
        let parties: Vec<&NodeRef> = self
            .subjects
            .iter()
            .filter_map(ConflictSubject::as_node)
            .collect();

        let mut edges = Vec::new();
        // The conflict is derived from each claim it is about.
        for party in &parties {
            if let Ok(edge) = EvidenceEdge::new(
                EdgeRelation::DerivedFrom,
                conflict_ref.clone(),
                (*party).clone(),
                [],
                node.provenance().clone(),
            ) {
                edges.push(edge);
            }
        }
        // …and the claims are in tension with each other: one edge per unordered pair, in
        // canonical identity order, because the relation is symmetric and the other
        // direction would be a second content identity for one fact.
        for (index, left) in parties.iter().enumerate() {
            for right in &parties[index + 1..] {
                if let Ok(edge) = EvidenceEdge::new(
                    EdgeRelation::ConflictsWith,
                    (*left).clone(),
                    (*right).clone(),
                    [],
                    node.provenance().clone(),
                ) {
                    edges.push(edge);
                }
            }
        }
        MaterializedConflict { node, edges }
    }
}

/// What a coordinator recorded about a conflict.
///
/// Not a choice between the parties: an outcome plus the **evidence** it rests on. docs/44
/// forbids resolving "by selecting the most confident agent answer", so this type has no
/// confidence and refuses an empty evidence set.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Resolution {
    decision: NodeRef,
    outcome: ResolutionOutcome,
    evidence: BTreeSet<ArtifactRef>,
    provenance: Provenance,
}

/// What the recorded decision says happened.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum ResolutionOutcome {
    /// One party stands; the others are retired by their own `SUPERSEDES` edges, which are
    /// separate writes and separate promotions.
    Retained(ConflictSubject),
    /// No party stands: the conflict is retired in favour of a successor claim.
    AllSuperseded,
}

impl Resolution {
    /// Record a resolution.
    ///
    /// # Errors
    ///
    /// - [`ConflictRefusal::NotADecisionNode`] when `decision` is not a
    ///   [`NodeKind::Decision`]. docs/44's retirement row demands "policy/owner with explicit
    ///   edge", and the thing at the other end of that edge is plan §11.2's `ReviewDecision`.
    /// - [`ConflictRefusal::NoEvidence`] when no artifact is named.
    pub fn new(
        decision: NodeRef,
        outcome: ResolutionOutcome,
        evidence: impl IntoIterator<Item = ArtifactRef>,
        provenance: Provenance,
    ) -> Result<Self, ConflictRefusal> {
        if decision.kind() != NodeKind::Decision {
            return Err(ConflictRefusal::NotADecisionNode {
                kind: decision.kind(),
            });
        }
        let evidence: BTreeSet<ArtifactRef> = evidence.into_iter().collect();
        if evidence.is_empty() {
            return Err(ConflictRefusal::NoEvidence);
        }
        Ok(Self {
            decision,
            outcome,
            evidence,
            provenance,
        })
    }

    /// The `decision` node recording it.
    #[must_use]
    pub const fn decision(&self) -> &NodeRef {
        &self.decision
    }

    /// What it says happened.
    #[must_use]
    pub const fn outcome(&self) -> &ResolutionOutcome {
        &self.outcome
    }

    /// The artifacts it rests on, in canonical order.
    pub fn evidence(&self) -> impl ExactSizeIterator<Item = &ArtifactRef> {
        self.evidence.iter()
    }

    /// Who recorded it.
    #[must_use]
    pub const fn provenance(&self) -> &Provenance {
        &self.provenance
    }

    /// The explicit `SUPERSEDES` edge docs/44's retirement row requires, from the decision
    /// to the conflict.
    ///
    /// # Errors
    ///
    /// [`crate::edge::EdgeRefusal::SelfEdge`] when the decision *is* the conflict node.
    pub fn supersedes_edge(
        &self,
        conflict: &NodeRef,
    ) -> Result<EvidenceEdge, crate::edge::EdgeRefusal> {
        EvidenceEdge::new(
            EdgeRelation::Supersedes,
            self.decision.clone(),
            conflict.clone(),
            self.evidence.iter().cloned(),
            self.provenance.clone(),
        )
    }
}

/// Why a conflict or a resolution was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ConflictRefusal {
    /// Fewer than two distinct parties.
    TooFewSubjects {
        /// How many distinct subjects were given.
        count: usize,
    },
    /// The ground was the empty string.
    EmptyGround,
    /// The required experiment was the empty string.
    EmptyExperiment,
    /// The resolution's decision node is not a `decision`.
    NotADecisionNode {
        /// The kind that was offered.
        kind: NodeKind,
    },
    /// The resolution names no evidence.
    NoEvidence,
}

impl fmt::Display for ConflictRefusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooFewSubjects { count } => write!(
                f,
                "a conflict is between at least two claims; {count} distinct subject(s) were given"
            ),
            Self::EmptyGround => {
                f.write_str("a conflict states the ground on which its parties cannot both hold")
            }
            Self::EmptyExperiment => {
                f.write_str("a conflict states the experiment or proof that would distinguish it")
            }
            Self::NotADecisionNode { kind } => write!(
                f,
                "a resolution is recorded by a `decision` node, not by a `{kind}`"
            ),
            Self::NoEvidence => f.write_str(
                "a conflict is not resolved by selecting the most confident answer: name evidence",
            ),
        }
    }
}

impl core::error::Error for ConflictRefusal {}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::identity::DefaultNaming;
    use crate::node::tests::{node, provenance};

    pub(crate) fn ground() -> ConflictGround {
        ConflictGround::new("both cannot satisfy observed correspondence").expect("non-empty")
    }

    pub(crate) fn experiment() -> RequiredExperiment {
        RequiredExperiment::new("distinguish observer contract").expect("non-empty")
    }

    fn abstractions() -> (EvidenceNode, EvidenceNode) {
        (
            node(NodeKind::AbstractionMap, "map_a", "claim-1"),
            node(NodeKind::AbstractionMap, "map_b", "claim-1"),
        )
    }

    #[test]
    fn a_conflict_needs_two_subjects() {
        let (left, right) = abstractions();
        assert_eq!(
            Conflict::between([], ground(), experiment()),
            Err(ConflictRefusal::TooFewSubjects { count: 0 })
        );
        assert_eq!(
            Conflict::between(
                [ConflictSubject::Node(NodeRef::of(&left))],
                ground(),
                experiment()
            ),
            Err(ConflictRefusal::TooFewSubjects { count: 1 })
        );
        // A repeated subject collapses rather than counting twice.
        assert_eq!(
            Conflict::between(
                [
                    ConflictSubject::Node(NodeRef::of(&left)),
                    ConflictSubject::Node(NodeRef::of(&left)),
                ],
                ground(),
                experiment()
            ),
            Err(ConflictRefusal::TooFewSubjects { count: 1 })
        );
        let conflict = Conflict::between(
            [
                ConflictSubject::Node(NodeRef::of(&left)),
                ConflictSubject::Node(NodeRef::of(&right)),
            ],
            ground(),
            experiment(),
        )
        .expect("two distinct parties");
        assert_eq!(conflict.subjects().len(), 2);
    }

    #[test]
    fn a_conflict_states_its_ground_and_its_experiment() {
        // docs/44's block has both lines, and both are mandatory arguments here.
        assert_eq!(ConflictGround::new(""), Err(ConflictRefusal::EmptyGround));
        assert_eq!(
            RequiredExperiment::new(""),
            Err(ConflictRefusal::EmptyExperiment)
        );
        let (left, right) = abstractions();
        let conflict = Conflict::between(
            [
                ConflictSubject::Node(NodeRef::of(&left)),
                ConflictSubject::Node(NodeRef::of(&right)),
            ],
            ground(),
            experiment(),
        )
        .expect("two distinct parties");
        assert_eq!(
            conflict.ground().as_str(),
            "both cannot satisfy observed correspondence"
        );
        assert_eq!(
            conflict.required_experiment().as_str(),
            "distinguish observer contract"
        );
        assert_eq!(
            ConflictRefusal::TooFewSubjects { count: 1 }.to_string(),
            "a conflict is between at least two claims; 1 distinct subject(s) were given"
        );
    }

    #[test]
    fn the_conflict_node_names_every_subject_as_an_input() {
        let naming = DefaultNaming::new();
        let (left, right) = abstractions();
        let supports = crate::edge::tests::edge(EdgeRelation::Supports, &left, &right);
        let conflict = Conflict::between(
            [
                ConflictSubject::Node(NodeRef::of(&left)),
                ConflictSubject::Node(NodeRef::of(&right)),
                ConflictSubject::Edge(EdgeRef::of(&supports)),
            ],
            ground(),
            experiment(),
        )
        .expect("three distinct parties");
        let materialized = conflict.materialize(
            &naming,
            ArtifactRef::new("conflict_9f").expect("well formed"),
            ClaimId::new("claim-1").expect("non-empty"),
            IdempotencyKey::new("key-1").expect("non-empty"),
            provenance("agent:integrator"),
        );
        assert_eq!(materialized.node().kind(), NodeKind::Conflict);
        let inputs: Vec<&str> = materialized
            .node()
            .provenance()
            .inputs()
            .map(ArtifactRef::as_str)
            .collect();
        assert_eq!(inputs.len(), 3);
        for subject in conflict.subjects() {
            let handle = subject.handle(&naming);
            assert!(
                inputs.contains(&handle.as_str()),
                "{} is not among the inputs",
                handle
            );
        }
        // The edge subject is named even though no edge can point at it.
        assert!(
            inputs.contains(&naming.name(&supports.identity()).as_str()),
            "the edge subject must still be recorded"
        );
    }

    #[test]
    fn conflicts_with_edges_are_written_once_per_pair() {
        let naming = DefaultNaming::new();
        let (left, right) = abstractions();
        let third = node(NodeKind::AbstractionMap, "map_c", "claim-1");
        let conflict = Conflict::between(
            [
                ConflictSubject::Node(NodeRef::of(&left)),
                ConflictSubject::Node(NodeRef::of(&right)),
                ConflictSubject::Node(NodeRef::of(&third)),
            ],
            ground(),
            experiment(),
        )
        .expect("three distinct parties");
        let materialized = conflict.materialize(
            &naming,
            ArtifactRef::new("conflict_9f").expect("well formed"),
            ClaimId::new("claim-1").expect("non-empty"),
            IdempotencyKey::new("key-1").expect("non-empty"),
            provenance("agent:integrator"),
        );
        let conflicts_with = materialized
            .edges()
            .filter(|edge| edge.kind() == crate::edge::EdgeKind::ConflictsWith)
            .count();
        let derived_from = materialized
            .edges()
            .filter(|edge| edge.kind() == crate::edge::EdgeKind::DerivedFrom)
            .count();
        // Three parties: three unordered pairs, one edge each, and one derivation per party.
        assert_eq!(conflicts_with, 3);
        assert_eq!(derived_from, 3);
        // …and every drawn edge is between two distinct identities, in one direction only.
        let mut seen = BTreeSet::new();
        for edge in materialized.edges() {
            assert!(seen.insert(edge.identity()));
            assert_ne!(edge.from().identity(), edge.to().identity());
        }
    }

    #[test]
    fn a_resolution_without_evidence_is_refused() {
        // docs/44: "A coordinator cannot resolve conflict by selecting the most confident
        // agent answer." There is no confidence to select on, and no evidence is a refusal.
        let decision = node(NodeKind::Decision, "decision_9f", "claim-1");
        let (left, _) = abstractions();
        assert_eq!(
            Resolution::new(
                NodeRef::of(&decision),
                ResolutionOutcome::AllSuperseded,
                [],
                provenance("human:ada"),
            ),
            Err(ConflictRefusal::NoEvidence)
        );
        let resolution = Resolution::new(
            NodeRef::of(&decision),
            ResolutionOutcome::Retained(ConflictSubject::Node(NodeRef::of(&left))),
            [ArtifactRef::new("trace_9f").expect("well formed")],
            provenance("human:ada"),
        )
        .expect("named evidence");
        assert_eq!(resolution.evidence().len(), 1);
        assert_eq!(
            ConflictRefusal::NoEvidence.to_string(),
            "a conflict is not resolved by selecting the most confident answer: name evidence"
        );
    }

    #[test]
    fn a_resolution_is_recorded_by_a_decision_node() {
        // plan §11.2's `ReviewDecision`; docs/44's "policy/owner with explicit edge".
        let (left, _) = abstractions();
        for kind in NodeKind::ALL {
            let recorder = node(kind, "rec_9f", "claim-1");
            let built = Resolution::new(
                NodeRef::of(&recorder),
                ResolutionOutcome::Retained(ConflictSubject::Node(NodeRef::of(&left))),
                [ArtifactRef::new("trace_9f").expect("well formed")],
                provenance("human:ada"),
            );
            assert_eq!(built.is_ok(), kind == NodeKind::Decision, "{kind}");
            if kind != NodeKind::Decision {
                assert_eq!(built, Err(ConflictRefusal::NotADecisionNode { kind }));
            }
        }
    }

    #[test]
    fn the_supersedes_edge_carries_the_resolutions_evidence() {
        let decision = node(NodeKind::Decision, "decision_9f", "claim-1");
        let conflict_node = node(NodeKind::Conflict, "conflict_9f", "claim-1");
        let resolution = Resolution::new(
            NodeRef::of(&decision),
            ResolutionOutcome::AllSuperseded,
            [ArtifactRef::new("trace_9f").expect("well formed")],
            provenance("human:ada"),
        )
        .expect("named evidence");
        let edge = resolution
            .supersedes_edge(&NodeRef::of(&conflict_node))
            .expect("distinct endpoints");
        assert_eq!(edge.kind(), crate::edge::EdgeKind::Supersedes);
        let named: Vec<&str> = edge.evidence().map(ArtifactRef::as_str).collect();
        assert_eq!(named, ["trace_9f"]);
        // A decision cannot supersede itself.
        assert!(resolution.supersedes_edge(&NodeRef::of(&decision)).is_err());
    }
}
