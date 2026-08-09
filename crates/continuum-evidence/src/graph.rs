//! The append-only evidence graph: what it stores, what it refuses, and what it can be
//! asked (RFC 0038 "Write and concurrency model" and "Queries", plan §11.7).
//!
//! # Append-only is the whole write model
//!
//! > The graph is append-only; nothing is edited in place. […] Idempotency keys (RFC 0026)
//! > make agent retries safe: a replayed write returns the original node identity.
//! >
//! > — RFC 0038, "Write and concurrency model"
//!
//! [`EvidenceGraph`] has no removal, no replacement, and no mutable accessor. Adding is
//! **put-if-absent** keyed by content identity, so a replay of an identical write returns
//! [`Insertion::Converged`] carrying the identity the graph already holds, and the stored
//! record — including any status a service has since promoted it to — is untouched. That is
//! the same behaviour `DaemonState::append_evidence` implements one layer up, arrived at from
//! the same sentence.
//!
//! # Three refusals, each from a different sentence
//!
//! | Refusal | Source |
//! |---|---|
//! | an edge endpoint the graph does not hold | RFC 0038's whiteboard compiler "rejects nonexistent references"; docs/44 "references must resolve" |
//! | a `CHECKED_BY` to-handle outside the configured rule | RFC 0038 D1, carried as the graph's [`CheckTargetRule`] — see below |
//! | an edge relating an artifact to itself | [`crate::edge::EvidenceEdge::new`], INV-004 |
//!
//! The second was a placeholder and is now a rule. RFC 0038 decided (D1, bn-3sypm) that a
//! `CHECKED_BY` edge's to-handle names a `receipt` and nothing else, so
//! [`CheckTargetRule`]'s default is that decision and a graph nobody configured enforces
//! it. It stays a *value* rather than a branch —
//! [`EvidenceGraph::with_check_target_rule`] still takes any set — because a deployment
//! ahead of or behind the RFC must be able to say so in the type; what changed is which
//! answer a caller gets for free.
//!
//! # Provenance is not checked, it is required
//!
//! There is no "reject a node with no provenance" branch below, and its absence is the
//! point: [`EvidenceNode::propose`] and [`EvidenceEdge::new`] take a
//! [`crate::provenance::Provenance`] by value, so the graph never sees one without a
//! producer, an input set, and a time. PR-7 / IMPL-02's "the graph refuses provenance-free
//! additions structurally" is that sentence read literally.
//!
//! # Queries
//!
//! > Missing obligations, conflicting candidates, proof frontier, repair frontier, semantic
//! > duplicates, provenance, and task generation.
//! >
//! > — RFC 0038, "Queries"
//!
//! What this module ships is the **traversal and selection layer** those seven are written
//! in — by kind, by status, by claim, by producer, and the incident edges of a node — plus
//! [`EvidenceGraph::contradictions`], which is "conflicting candidates" and is
//! [`crate::conflict`]'s input.
//!
//! The previous revision of this paragraph said the other six "need artifact content this
//! crate does not hold". That was true when it was written and is now too strong, and RFC
//! 0038's "Queries" section carries the corrected reading (bn-xsuz2). Re-verified against
//! the graph as it stands, the six split three ways:
//!
//! - **still content-blocked, four.** *Missing obligations* needs to decide that a claim
//!   requires an obligation nobody wrote, which means reading the claim, and a node holds a
//!   commitment rather than a formula. A *proof frontier* needs the judgement that a
//!   receipt discharges an obligation, which RFC 0038 D3 explicitly withholds from a
//!   `CHECKED_BY` edge — "an edge is not a promotion […] what that evidence licenses is a
//!   later, separate decision". A *repair frontier* needs patch and failure content, and
//!   nothing appends a `REPAIRS` edge. *Semantic duplicates* needs semantic equality, which
//!   is the one this crate can never have: ADR-0013 makes two records the same artifact
//!   exactly when they encode identically, and that is syntactic by construction.
//! - **now answerable from held content, one.** *Provenance* in docs/44's derivation sense
//!   — "source retrievals, and derivation" — is a walk over `provenance.inputs`, which every
//!   node carries and which `observe.ingest`, `evidence.link`, and `whiteboard.compile` all
//!   write. [`EvidenceGraph::nodes_by_producer`] answers the who-produced-what grain; the
//!   derivation closure is a legitimate slice and is carried, not shipped here.
//! - **blocked on a different thing than was recorded, one.** *Task generation*'s seven
//!   docs/44 forms are predicates over edge kinds the graph almost never holds: of the
//!   thirteen, `whiteboard.compile` appends `SUPPORTS`, `evidence.link` appends
//!   `CHECKED_BY`, and [`crate::conflict`] appends `CONFLICTS_WITH` and `SUPERSEDES` — no
//!   operation appends `DEPENDS_ON`, `REPAIRS`, `REFUTES`, or `COUNTEREXAMPLE_TO`. So "find
//!   counterexample to candidate" would answer *every* candidate in every graph, which is
//!   the empty-success shape rather than a query. What unblocks it is an operation that
//!   records a refutation, not more content.
//!
//! None of the six is approximated here. Naming which of them is blocked on *what* is the
//! difference between a deferral and a shrug.
//!
//! Every accessor iterates a [`std::collections::BTreeMap`] keyed by content identity, so
//! iteration order is a function of the content and of nothing else (GOV-1-03, INV-005).
//!
//! # Clause → test
//!
//! | Clause | Source | Test |
//! |---|---|---|
//! | append-only, nothing edited in place | RFC 0038 | `an_identical_write_converges_and_changes_nothing`, `the_graph_exposes_no_removal` |
//! | a replayed write returns the original identity | RFC 0038 | `an_identical_write_converges_and_changes_nothing` |
//! | references must resolve | docs/44, RFC 0038 | `an_edge_to_an_unheld_node_is_refused` |
//! | provenance is structural | PR-7 / IMPL-02 | `nothing_enters_the_graph_without_a_producer` |
//! | a check edge points at a receipt | RFC 0038 D1 (bn-3sypm) | `a_check_edge_points_at_a_receipt_by_default`, `a_decided_check_target_rule_is_enforced` |
//! | deterministic iteration | GOV-1-03, INV-005 | `iteration_order_is_content_order` |
//! | promotion is a compare-and-set | plan §11.7 | `a_lost_compare_and_set_is_a_status_conflict`, `a_downgrade_is_refused` |
//! | a producer may not promote its own claim | INV-004 | `a_service_cannot_promote_its_own_production` |
//! | promotion keeps the identity and appends | RFC 0038 | `a_promotion_appends_a_version_and_keeps_the_identity` |
//! | contradictions materialize a conflict node | plan §11.7 | `a_contradiction_materializes_a_conflict_node` |
//! | resolution is a transition, not a deletion | plan §4.6, docs/44 | `resolving_a_conflict_deletes_nothing` |
//! | only a policy owner retires a claim | docs/44 | `only_a_superseding_promotion_resolves` |
//! | unresolved contradictions remain visible | docs/44 | `an_unresolved_conflict_stays_visible` |

use core::fmt;
use std::collections::BTreeMap;

use crate::actor::ActorId;
use crate::authority::Promotion;
use crate::claim_status::{ClaimStatus, PromotionRejected, compare_and_set};
use crate::conflict::{MaterializedConflict, Resolution};
use crate::edge::{CheckTargetRule, EdgeKind, EvidenceEdge};
use crate::identity::EvidenceIdentity;
use crate::node::{ClaimId, EvidenceNode, NodeKind, NodeRef};

/// What an append did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Insertion {
    /// The record was not held and is now.
    Fresh(EvidenceIdentity),
    /// The graph already held this identity. Nothing was written and the held record —
    /// including its status history — is unchanged (RFC 0038: "a replayed write returns the
    /// original node identity").
    Converged(EvidenceIdentity),
}

impl Insertion {
    /// The identity the record is filed under, whichever arm this is.
    #[must_use]
    pub const fn identity(&self) -> &EvidenceIdentity {
        match self {
            Self::Fresh(identity) | Self::Converged(identity) => identity,
        }
    }

    /// Whether this append wrote something.
    #[must_use]
    pub const fn is_fresh(&self) -> bool {
        matches!(self, Self::Fresh(_))
    }
}

/// Why the graph refused to record a resolution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolutionRefusal {
    /// The identity does not name a held `conflict` node.
    NotAConflict,
    /// The promotion installs something other than `superseded`.
    NotARetirement {
        /// The status the promotion would have installed.
        status: ClaimStatus,
    },
    /// The explicit `SUPERSEDES` edge could not be built.
    Edge(crate::edge::EdgeRefusal),
    /// The explicit `SUPERSEDES` edge could not be appended.
    Graph(GraphRefusal),
    /// The transition itself was refused.
    Promotion(PromotionRefusal),
}

impl fmt::Display for ResolutionRefusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotAConflict => {
                f.write_str("that identity does not name a conflict node this graph holds")
            }
            Self::NotARetirement { status } => write!(
                f,
                "a conflict is resolved by retiring it: a promotion into {status} is a choice, \
                 not a resolution"
            ),
            Self::Edge(refusal) => fmt::Display::fmt(refusal, f),
            Self::Graph(refusal) => fmt::Display::fmt(refusal, f),
            Self::Promotion(refusal) => fmt::Display::fmt(refusal, f),
        }
    }
}

impl core::error::Error for ResolutionRefusal {}

/// Why the graph refused a promotion.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PromotionRefusal {
    /// The graph holds no node under that identity.
    UnknownNode,
    /// The promoting service is the node's own producer.
    ///
    /// > INV-004 — no self-certification: a producer may not promote its own claim.
    ///
    /// The three type-level guards in [`crate::authority`] stop a *client* from promoting.
    /// They do not stop a deployment that produces under the same identity it verifies
    /// under, which is self-certification with every rule obeyed at every step. This is that
    /// guard, and it is the only one that has to be a check rather than a type, because it
    /// compares two values neither of which is known when the witness is minted.
    SelfCertification,
    /// The compare-and-set was refused by the plan §11.4 lattice.
    Rejected(PromotionRejected),
}

impl fmt::Display for PromotionRefusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownNode => f.write_str("the graph holds no node under that identity"),
            Self::SelfCertification => f.write_str(
                "a producer may not promote its own claim: the promoting service is this \
                 node's producer",
            ),
            Self::Rejected(rejected) => fmt::Display::fmt(rejected, f),
        }
    }
}

impl core::error::Error for PromotionRefusal {}

/// Why the graph refused an append.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GraphRefusal {
    /// An edge names an endpoint the graph does not hold.
    ///
    /// RFC 0038's whiteboard compiler "rejects nonexistent references" and docs/44's
    /// compilation rules require that "references must resolve"; this is that rule at the
    /// layer where the graph is written rather than at the layer where prose is compiled.
    UnknownEndpoint {
        /// Whether the unheld endpoint was the edge's source.
        from: bool,
    },
    /// A `CHECKED_BY` edge's to-handle names a node kind the configured
    /// [`CheckTargetRule`] does not admit.
    ///
    /// Under the default rule — RFC 0038 D1 — that is every kind but `receipt`.
    CheckTargetRefused {
        /// The kind the to-handle named.
        kind: NodeKind,
    },
}

impl fmt::Display for GraphRefusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownEndpoint { from } => {
                let side = if *from { "source" } else { "target" };
                write!(f, "the edge's {side} names a node the graph does not hold")
            }
            Self::CheckTargetRefused { kind } => write!(
                f,
                "the configured CHECKED_BY target rule does not admit a `{kind}` to-handle"
            ),
        }
    }
}

impl core::error::Error for GraphRefusal {}

/// A pair of edges that cannot both hold.
///
/// One relation asserts what the other denies over the same ordered endpoints. Reported, not
/// resolved: plan §11.7 requires contradictory claims to "materialize a `Conflict` node
/// rather than resolving by write order", and this graph has no API that picks a winner.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Contradiction {
    /// The identity of the asserting edge (the `SUPPORTS`-side member).
    pub asserted: EvidenceIdentity,
    /// The identity of the denying edge (the `REFUTES`/`INVALIDATES`-side member of
    /// [`CONTRADICTORY_KINDS`]).
    pub denied: EvidenceIdentity,
}

/// The pairs of edge kinds that cannot both hold between one ordered pair of nodes.
///
/// Two rows, each read off the dossier rather than invented:
///
/// - `SUPPORTS` / `REFUTES`: docs/44's integrator computes "supporting/refuting evidence"
///   and "conflicts" as separate outputs, and an artifact that both supports and refutes the
///   same claim is the contradiction plan §11.7 names.
/// - `SUPPORTS` / `INVALIDATES`: docs/44's own example — "one patch repairs one failure but
///   invalidates a proof" — is a tension exactly when the same artifact is also offered as
///   support for the thing it invalidates.
///
/// Deliberately short. A longer table would be this crate inventing semantics for relations
/// the dossier defines only by name, and a wrong contradiction is worse than a missing one:
/// it would manufacture conflict nodes nobody can resolve.
pub const CONTRADICTORY_KINDS: [(EdgeKind, EdgeKind); 2] = [
    (EdgeKind::Supports, EdgeKind::Refutes),
    (EdgeKind::Supports, EdgeKind::Invalidates),
];

/// The append-only evidence graph.
///
/// Keyed by content identity throughout, so two records are the same record exactly when
/// they say the same thing, and iteration is deterministic.
#[derive(Debug, Clone, Default)]
pub struct EvidenceGraph {
    nodes: BTreeMap<EvidenceIdentity, EvidenceNode>,
    edges: BTreeMap<EvidenceIdentity, EvidenceEdge>,
    check_target_rule: CheckTargetRule,
}

impl EvidenceGraph {
    /// An empty graph, enforcing RFC 0038 D1: a `CHECKED_BY` edge points at a `receipt`.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The same graph under a different [`CheckTargetRule`].
    ///
    /// The RFC's answer is the default; this is for a deployment that must run ahead of it
    /// or behind it, and it exists so that saying so is a value rather than a patch.
    #[must_use]
    pub fn with_check_target_rule(mut self, rule: CheckTargetRule) -> Self {
        self.check_target_rule = rule;
        self
    }

    /// The rule in force.
    #[must_use]
    pub const fn check_target_rule(&self) -> &CheckTargetRule {
        &self.check_target_rule
    }

    /// Append a node, put-if-absent.
    ///
    /// Never replaces: a second write of an identical node returns
    /// [`Insertion::Converged`] and the held record — including a status a service has since
    /// promoted it to — is untouched.
    pub fn add_node(&mut self, node: EvidenceNode) -> Insertion {
        let identity = node.identity();
        if self.nodes.contains_key(&identity) {
            return Insertion::Converged(identity);
        }
        self.nodes.insert(identity.clone(), node);
        Insertion::Fresh(identity)
    }

    /// Append an edge, put-if-absent.
    ///
    /// # Errors
    ///
    /// - [`GraphRefusal::UnknownEndpoint`] when either endpoint is not held. Checked before
    ///   anything is written, so a refused edge leaves the graph exactly as it was.
    /// - [`GraphRefusal::CheckTargetRefused`] when a `CHECKED_BY` edge's to-handle names a
    ///   kind the configured rule does not admit.
    pub fn add_edge(&mut self, edge: EvidenceEdge) -> Result<Insertion, GraphRefusal> {
        if !self.nodes.contains_key(edge.from().identity()) {
            return Err(GraphRefusal::UnknownEndpoint { from: true });
        }
        if !self.nodes.contains_key(edge.to().identity()) {
            return Err(GraphRefusal::UnknownEndpoint { from: false });
        }
        if edge.kind() == EdgeKind::CheckedBy {
            let kind = edge.to().kind();
            if !self.check_target_rule.admits(kind) {
                return Err(GraphRefusal::CheckTargetRefused { kind });
            }
        }
        let identity = edge.identity();
        if self.edges.contains_key(&identity) {
            return Ok(Insertion::Converged(identity));
        }
        self.edges.insert(identity.clone(), edge);
        Ok(Insertion::Fresh(identity))
    }

    /// Advance one claim's status, under a witness only [`crate::authority`] can mint.
    ///
    /// The compare-and-set is plan §11.7's:
    ///
    /// > status promotion is a compare-and-set against the claim's current status, so racing
    /// > promotions cannot regress the lattice
    ///
    /// and it is [`crate::claim_status::compare_and_set`]'s decision, not a second copy of
    /// it, so this graph and any independent linearizability checker share one implementation
    /// of "would this write lower the claim?".
    ///
    /// Three guards, in this order, and the order matters:
    ///
    /// 1. the node must be held — a promotion of something the graph does not have is not a
    ///    promotion;
    /// 2. **INV-004**: the promoting service may not be the node's own producer. Checked
    ///    before the lattice, because a service verifying its own production has nothing to
    ///    establish whatever the statuses say — the same order `evidence.verify` uses, and
    ///    for the same reason;
    /// 3. the lattice: a stale `expected` loses the race and a lowering write is refused.
    ///
    /// Authority — may *this* service install *that* status — was decided when the
    /// [`Promotion`] was minted; it cannot be re-opened here, because there is no way to
    /// build the witness without passing it.
    ///
    /// On success the node gains one history entry and keeps its identity, and the settled
    /// status is returned.
    ///
    /// # Errors
    ///
    /// - [`PromotionRefusal::UnknownNode`] — the graph does not hold the identity.
    /// - [`PromotionRefusal::SelfCertification`] — INV-004.
    /// - [`PromotionRefusal::Rejected`] — the typed plan §10.3 `StatusConflict` (a lost
    ///   compare-and-set) or a lattice regression.
    pub fn promote(
        &mut self,
        identity: &EvidenceIdentity,
        expected: ClaimStatus,
        promotion: &Promotion,
    ) -> Result<ClaimStatus, PromotionRefusal> {
        let node = self
            .nodes
            .get(identity)
            .ok_or(PromotionRefusal::UnknownNode)?;
        if node.provenance().actor() == promotion.service().actor() {
            return Err(PromotionRefusal::SelfCertification);
        }
        let settled = compare_and_set(node.status(), expected, promotion.status())
            .map_err(PromotionRefusal::Rejected)?;
        let promoted = node.appending(promotion.write(settled));
        self.nodes.insert(identity.clone(), promoted);
        Ok(settled)
    }

    /// Append a materialized conflict: the `conflict` node first, then its edges.
    ///
    /// > Concurrent contradictory claims materialize a `Conflict` node rather than resolving
    /// > by write order.
    /// >
    /// > — plan §11.7
    ///
    /// The node is appended before the edges so no edge is offered against an endpoint the
    /// graph does not yet hold, and every edge goes through [`Self::add_edge`], so the same
    /// three refusals apply to a conflict's edges as to any other. The subjects themselves
    /// must already be held: a conflict about claims nobody published is a reference that
    /// does not resolve.
    ///
    /// Returns the conflict node's identity.
    ///
    /// # Errors
    ///
    /// [`GraphRefusal::UnknownEndpoint`] when a subject is not held. The conflict node is
    /// appended first and stays: a partially-linked conflict is still visible, which is what
    /// "unresolved contradictions remain visible" (docs/44) asks for, and the append-only
    /// model has nothing to roll back with.
    pub fn add_conflict(
        &mut self,
        conflict: &MaterializedConflict,
    ) -> Result<EvidenceIdentity, GraphRefusal> {
        let identity = self.add_node(conflict.node().clone()).identity().clone();
        for edge in conflict.edges() {
            self.add_edge(edge.clone())?;
        }
        Ok(identity)
    }

    /// Every `conflict` node the graph holds, in content order.
    pub fn conflicts(&self) -> impl Iterator<Item = (&EvidenceIdentity, &EvidenceNode)> {
        self.nodes_of_kind(NodeKind::Conflict)
    }

    /// Every conflict node that has not been retired, in content order.
    ///
    /// docs/44's whiteboard rule — "unresolved contradictions remain visible" — as a query.
    /// A conflict is retired only by [`Self::resolve_conflict`], which supersedes it; every
    /// other status leaves it here.
    pub fn unresolved_conflicts(&self) -> impl Iterator<Item = (&EvidenceIdentity, &EvidenceNode)> {
        self.conflicts()
            .filter(|(_, node)| node.status() != ClaimStatus::Superseded)
    }

    /// Record a conflict's resolution: an explicit edge and a status transition, never a
    /// deletion.
    ///
    /// > | any → superseded | policy/owner with explicit edge |
    /// >
    /// > — docs/44, "Status authority"
    ///
    /// Both halves of that row, in order: the `SUPERSEDES` edge from the resolution's
    /// `decision` node to the conflict node is appended first, then the conflict node is
    /// promoted to `superseded` through [`Self::promote`] — so INV-004's
    /// producer-is-not-the-checker guard and plan §11.7's compare-and-set both still apply,
    /// and the promotion must come from a service holding
    /// [`crate::authority::ServiceRole::PolicyOwner`], which is the only role docs/44
    /// authorizes for that row.
    ///
    /// Nothing is removed. The parties, the `CONFLICTS_WITH` edges between them and the
    /// conflict node itself are all still held afterwards; only the conflict's status moved,
    /// and its history records that it did.
    ///
    /// # Errors
    ///
    /// - [`ResolutionRefusal::NotAConflict`] when the identity does not name a `conflict`
    ///   node the graph holds.
    /// - [`ResolutionRefusal::NotARetirement`] when the promotion does not install
    ///   `superseded`. A resolution that promoted a conflict to anything else would be a
    ///   coordinator choosing an answer.
    /// - [`ResolutionRefusal::Edge`] when the explicit edge cannot be appended — most often
    ///   because the `decision` node is not held.
    /// - [`ResolutionRefusal::Promotion`] when the transition itself is refused.
    pub fn resolve_conflict(
        &mut self,
        conflict: &EvidenceIdentity,
        resolution: &Resolution,
        promotion: &Promotion,
    ) -> Result<ClaimStatus, ResolutionRefusal> {
        let node = self
            .nodes
            .get(conflict)
            .ok_or(ResolutionRefusal::NotAConflict)?;
        if node.kind() != NodeKind::Conflict {
            return Err(ResolutionRefusal::NotAConflict);
        }
        if promotion.status() != ClaimStatus::Superseded {
            return Err(ResolutionRefusal::NotARetirement {
                status: promotion.status(),
            });
        }
        let expected = node.status();
        let edge = resolution
            .supersedes_edge(&NodeRef::of(node))
            .map_err(ResolutionRefusal::Edge)?;
        self.add_edge(edge).map_err(ResolutionRefusal::Graph)?;
        self.promote(conflict, expected, promotion)
            .map_err(ResolutionRefusal::Promotion)
    }

    /// The node filed under an identity.
    #[must_use]
    pub fn node(&self, identity: &EvidenceIdentity) -> Option<&EvidenceNode> {
        self.nodes.get(identity)
    }

    /// The edge filed under an identity.
    #[must_use]
    pub fn edge(&self, identity: &EvidenceIdentity) -> Option<&EvidenceEdge> {
        self.edges.get(identity)
    }

    /// Every node, in content order.
    pub fn nodes(&self) -> impl ExactSizeIterator<Item = (&EvidenceIdentity, &EvidenceNode)> {
        self.nodes.iter()
    }

    /// Every edge, in content order.
    pub fn edges(&self) -> impl ExactSizeIterator<Item = (&EvidenceIdentity, &EvidenceEdge)> {
        self.edges.iter()
    }

    /// How many nodes are held.
    #[must_use]
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    /// How many edges are held.
    #[must_use]
    pub fn edge_count(&self) -> usize {
        self.edges.len()
    }

    /// Whether the graph holds nothing at all.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty() && self.edges.is_empty()
    }

    /// Every node of one kind, in content order.
    pub fn nodes_of_kind(
        &self,
        kind: NodeKind,
    ) -> impl Iterator<Item = (&EvidenceIdentity, &EvidenceNode)> {
        self.nodes
            .iter()
            .filter(move |(_, node)| node.kind() == kind)
    }

    /// Every node currently at one status, in content order.
    pub fn nodes_with_status(
        &self,
        status: ClaimStatus,
    ) -> impl Iterator<Item = (&EvidenceIdentity, &EvidenceNode)> {
        self.nodes
            .iter()
            .filter(move |(_, node)| node.status() == status)
    }

    /// Every node about one claim, in content order (plan §11.7's linearization unit).
    pub fn nodes_for_claim<'a>(
        &'a self,
        claim: &'a ClaimId,
    ) -> impl Iterator<Item = (&'a EvidenceIdentity, &'a EvidenceNode)> {
        self.nodes
            .iter()
            .filter(move |(_, node)| node.claim_id() == claim)
    }

    /// Every node one actor produced, in content order (RFC 0038's `provenance` query, at
    /// the grain this crate can answer: who produced what).
    pub fn nodes_by_producer<'a>(
        &'a self,
        actor: &'a ActorId,
    ) -> impl Iterator<Item = (&'a EvidenceIdentity, &'a EvidenceNode)> {
        self.nodes
            .iter()
            .filter(move |(_, node)| node.provenance().actor() == actor)
    }

    /// Every edge of one kind, in content order.
    pub fn edges_of_kind(
        &self,
        kind: EdgeKind,
    ) -> impl Iterator<Item = (&EvidenceIdentity, &EvidenceEdge)> {
        self.edges
            .iter()
            .filter(move |(_, edge)| edge.kind() == kind)
    }

    /// Every edge leaving a node, in content order.
    pub fn edges_from<'a>(
        &'a self,
        node: &'a NodeRef,
    ) -> impl Iterator<Item = (&'a EvidenceIdentity, &'a EvidenceEdge)> {
        self.edges
            .iter()
            .filter(move |(_, edge)| edge.from() == node)
    }

    /// Every edge arriving at a node, in content order.
    pub fn edges_to<'a>(
        &'a self,
        node: &'a NodeRef,
    ) -> impl Iterator<Item = (&'a EvidenceIdentity, &'a EvidenceEdge)> {
        self.edges.iter().filter(move |(_, edge)| edge.to() == node)
    }

    /// Every pair of held edges that cannot both hold, in content order.
    ///
    /// The pairing table is [`CONTRADICTORY_KINDS`]; the endpoints must match exactly,
    /// including direction, because two edges about different subjects are not in tension.
    /// This *reports* — it never picks a winner, never deletes, and never promotes. What to
    /// do about a contradiction is [`crate::conflict`]'s, and what it may not be is
    /// "resolved by write order" (plan §11.7).
    #[must_use]
    pub fn contradictions(&self) -> Vec<Contradiction> {
        let mut found = Vec::new();
        for (asserted_id, asserted) in &self.edges {
            for (denied_id, denied) in &self.edges {
                let same_subject = asserted.from() == denied.from() && asserted.to() == denied.to();
                if same_subject && CONTRADICTORY_KINDS.contains(&(asserted.kind(), denied.kind())) {
                    found.push(Contradiction {
                        asserted: asserted_id.clone(),
                        denied: denied_id.clone(),
                    });
                }
            }
        }
        found
    }
}

#[cfg(test)]
mod tests {
    use continuum_value::assurance::ValidationBasis;

    use super::*;
    use crate::authority::ServiceRole;
    use crate::authority::tests::service;
    use crate::conflict::tests::{experiment, ground};
    use crate::conflict::{ConflictSubject, Resolution};
    use crate::edge::EdgeRef;
    use crate::edge::tests::{checker, edge};
    use crate::edge::{EdgeRelation, EvidenceEdge};
    use crate::identity::DefaultNaming;
    use crate::node::tests::{node, provenance};
    use crate::provenance::ArtifactRef;

    fn seeded() -> (EvidenceGraph, EvidenceNode, EvidenceNode) {
        let claim = node(NodeKind::Property, "prop_9f", "claim-1");
        let run = node(NodeKind::Run, "trace_9f", "claim-1");
        let mut graph = EvidenceGraph::new();
        graph.add_node(claim.clone());
        graph.add_node(run.clone());
        (graph, run, claim)
    }

    #[test]
    fn an_identical_write_converges_and_changes_nothing() {
        // RFC 0038: "a replayed write returns the original node identity".
        let mut graph = EvidenceGraph::new();
        let node = node(NodeKind::Run, "trace_9f", "claim-1");
        let first = graph.add_node(node.clone());
        assert!(first.is_fresh());
        let second = graph.add_node(node.clone());
        assert!(!second.is_fresh());
        assert_eq!(first.identity(), second.identity());
        assert_eq!(graph.node_count(), 1);
        assert_eq!(graph.node(first.identity()), Some(&node));

        let (mut graph, run, claim) = seeded();
        let edge = edge(EdgeRelation::Supports, &run, &claim);
        let first = graph.add_edge(edge.clone()).expect("both endpoints held");
        let second = graph.add_edge(edge).expect("both endpoints held");
        assert!(first.is_fresh());
        assert!(!second.is_fresh());
        assert_eq!(graph.edge_count(), 1);
    }

    #[test]
    fn the_graph_exposes_no_removal() {
        // Append-only stated as an API shape: there is nothing to call. The assertion that
        // can be written is that every held record stays held across further appends.
        let (mut graph, run, claim) = seeded();
        let held: Vec<EvidenceIdentity> = graph.nodes().map(|(id, _)| id.clone()).collect();
        graph
            .add_edge(edge(EdgeRelation::Supports, &run, &claim))
            .expect("both endpoints held");
        graph.add_node(node(NodeKind::Patch, "patch_9f", "claim-2"));
        for identity in held {
            assert!(graph.node(&identity).is_some());
        }
        assert_eq!(graph.node_count(), 3);
    }

    #[test]
    fn an_edge_to_an_unheld_node_is_refused() {
        // docs/44: "references must resolve".
        let claim = node(NodeKind::Property, "prop_9f", "claim-1");
        let run = node(NodeKind::Run, "trace_9f", "claim-1");
        let mut graph = EvidenceGraph::new();

        // Neither endpoint held: the source is named first.
        assert_eq!(
            graph.add_edge(edge(EdgeRelation::Supports, &run, &claim)),
            Err(GraphRefusal::UnknownEndpoint { from: true })
        );
        graph.add_node(run.clone());
        assert_eq!(
            graph.add_edge(edge(EdgeRelation::Supports, &run, &claim)),
            Err(GraphRefusal::UnknownEndpoint { from: false })
        );
        // A refusal writes nothing.
        assert_eq!(graph.edge_count(), 0);
        graph.add_node(claim.clone());
        assert!(
            graph
                .add_edge(edge(EdgeRelation::Supports, &run, &claim))
                .is_ok()
        );
        assert_eq!(
            GraphRefusal::UnknownEndpoint { from: false }.to_string(),
            "the edge's target names a node the graph does not hold"
        );
    }

    #[test]
    fn nothing_enters_the_graph_without_a_producer() {
        // Structural rather than checked: every held record has a provenance because the
        // constructors take one. The assertion sweeps what is held.
        let (mut graph, run, claim) = seeded();
        graph
            .add_edge(edge(EdgeRelation::Supports, &run, &claim))
            .expect("both endpoints held");
        assert_eq!(graph.node_count(), 2);
        assert_eq!(graph.edge_count(), 1);
        for (_, node) in graph.nodes() {
            assert!(!node.provenance().actor().as_str().is_empty());
            assert!(!node.provenance().created_at().as_str().is_empty());
        }
        for (_, edge) in graph.edges() {
            assert!(!edge.provenance().actor().as_str().is_empty());
        }
    }

    #[test]
    fn a_check_edge_points_at_a_receipt_by_default() {
        // RFC 0038 D1, at the layer that enforces it: a graph nobody configured admits a
        // check edge to a `receipt` and refuses one to each of the other nineteen kinds —
        // including the two candidates the RFC weighed and rejected.
        for kind in NodeKind::ALL {
            let target = node(kind, "prop_9f", "claim-1");
            let subject = node(NodeKind::Certificate, "cert_9f", "claim-1");
            let mut graph = EvidenceGraph::new();
            graph.add_node(target.clone());
            graph.add_node(subject.clone());
            let asserted = graph.add_edge(edge(
                EdgeRelation::CheckedBy(checker("service:kernel-core")),
                &subject,
                &target,
            ));
            if kind == NodeKind::Receipt {
                assert!(asserted.is_ok(), "{kind}");
            } else {
                assert_eq!(asserted, Err(GraphRefusal::CheckTargetRefused { kind }));
            }
        }
    }

    #[test]
    fn a_decided_check_target_rule_is_enforced() {
        // …and the enforcement is not vacuous: under a decided rule the admitted kind lands
        // and a refused kind does not, while every other edge kind is unaffected.
        let run = node(NodeKind::Run, "trace_9f", "claim-1");
        let certificate = node(NodeKind::Certificate, "cert_9f", "claim-1");
        let patch = node(NodeKind::Patch, "patch_9f", "claim-1");
        let mut graph = EvidenceGraph::new()
            .with_check_target_rule(CheckTargetRule::one_of([NodeKind::Certificate]));
        graph.add_node(run.clone());
        graph.add_node(certificate.clone());
        graph.add_node(patch.clone());

        assert!(
            graph
                .add_edge(edge(
                    EdgeRelation::CheckedBy(checker("service:kernel-core")),
                    &run,
                    &certificate,
                ))
                .is_ok()
        );
        assert_eq!(
            graph.add_edge(edge(
                EdgeRelation::CheckedBy(checker("service:kernel-core")),
                &run,
                &patch,
            )),
            Err(GraphRefusal::CheckTargetRefused {
                kind: NodeKind::Patch
            })
        );
        // The rule is about CHECKED_BY and nothing else.
        assert!(
            graph
                .add_edge(edge(EdgeRelation::Supports, &run, &patch))
                .is_ok()
        );
        assert_eq!(
            GraphRefusal::CheckTargetRefused {
                kind: NodeKind::Patch
            }
            .to_string(),
            "the configured CHECKED_BY target rule does not admit a `patch` to-handle"
        );
    }

    #[test]
    fn iteration_order_is_content_order() {
        // GOV-1-03/INV-005: the order is a function of the content, so two graphs built by
        // two insertion orders iterate identically.
        let members = [
            node(NodeKind::Run, "trace_a", "claim-1"),
            node(NodeKind::Patch, "patch_b", "claim-1"),
            node(NodeKind::Property, "prop_c", "claim-2"),
        ];
        let mut forward = EvidenceGraph::new();
        for member in &members {
            forward.add_node(member.clone());
        }
        let mut backward = EvidenceGraph::new();
        for member in members.iter().rev() {
            backward.add_node(member.clone());
        }
        let listed = |graph: &EvidenceGraph| -> Vec<Vec<u8>> {
            graph
                .nodes()
                .map(|(identity, _)| identity.canonical_bytes().to_vec())
                .collect()
        };
        assert_eq!(listed(&forward), listed(&backward));
        let mut sorted = listed(&forward);
        sorted.sort();
        assert_eq!(listed(&forward), sorted);
    }

    #[test]
    fn selection_queries_partition_what_is_held() {
        let (mut graph, run, claim) = seeded();
        let patch = node(NodeKind::Patch, "patch_9f", "claim-2");
        graph.add_node(patch.clone());
        graph
            .add_edge(edge(EdgeRelation::Supports, &run, &claim))
            .expect("both endpoints held");
        graph
            .add_edge(edge(EdgeRelation::Repairs, &patch, &claim))
            .expect("both endpoints held");

        assert_eq!(graph.nodes_of_kind(NodeKind::Run).count(), 1);
        assert_eq!(graph.nodes_of_kind(NodeKind::Decision).count(), 0);
        assert_eq!(
            graph.nodes_with_status(ClaimStatus::Proposed).count(),
            graph.node_count()
        );
        assert_eq!(graph.nodes_with_status(ClaimStatus::Proved).count(), 0);
        let claim_id = ClaimId::new("claim-1").expect("non-empty");
        assert_eq!(graph.nodes_for_claim(&claim_id).count(), 2);
        let producer = crate::actor::ActorId::new("agent:swarm-1").expect("well formed");
        assert_eq!(graph.nodes_by_producer(&producer).count(), 3);
        let stranger = crate::actor::ActorId::new("agent:nobody").expect("well formed");
        assert_eq!(graph.nodes_by_producer(&stranger).count(), 0);

        assert_eq!(graph.edges_of_kind(EdgeKind::Supports).count(), 1);
        assert_eq!(graph.edges_of_kind(EdgeKind::Supersedes).count(), 0);
        assert_eq!(graph.edges_from(&NodeRef::of(&run)).count(), 1);
        assert_eq!(graph.edges_to(&NodeRef::of(&claim)).count(), 2);
        assert_eq!(graph.edges_from(&NodeRef::of(&claim)).count(), 0);
    }

    #[test]
    fn a_contradiction_is_reported_and_never_resolved() {
        // plan §11.7: contradictory claims materialize a Conflict node "rather than
        // resolving by write order". Both orders report the same pair.
        let (mut graph, run, claim) = seeded();
        let supports = edge(EdgeRelation::Supports, &run, &claim);
        let refutes = edge(EdgeRelation::Refutes, &run, &claim);
        assert!(graph.contradictions().is_empty());
        graph.add_edge(supports.clone()).expect("endpoints held");
        assert!(graph.contradictions().is_empty());
        graph.add_edge(refutes.clone()).expect("endpoints held");

        let found = graph.contradictions();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].asserted, supports.identity());
        assert_eq!(found[0].denied, refutes.identity());
        // Both edges are still held; nothing was chosen.
        assert_eq!(graph.edge_count(), 2);
        assert!(graph.edge(&supports.identity()).is_some());
        assert!(graph.edge(&refutes.identity()).is_some());
    }

    #[test]
    fn a_contradiction_needs_the_same_subject() {
        // Anti-vacuity for the pairing table: the same two kinds over different endpoints,
        // or in the other direction, are not in tension.
        let claim = node(NodeKind::Property, "prop_9f", "claim-1");
        let other = node(NodeKind::Property, "prop_aa", "claim-1");
        let run = node(NodeKind::Run, "trace_9f", "claim-1");
        let mut graph = EvidenceGraph::new();
        for member in [&claim, &other, &run] {
            graph.add_node(member.clone());
        }
        graph
            .add_edge(edge(EdgeRelation::Supports, &run, &claim))
            .expect("endpoints held");
        graph
            .add_edge(edge(EdgeRelation::Refutes, &run, &other))
            .expect("endpoints held");
        assert!(graph.contradictions().is_empty());
        graph
            .add_edge(edge(EdgeRelation::Refutes, &claim, &run))
            .expect("endpoints held");
        assert!(graph.contradictions().is_empty());
        // …and the table's second row does fire.
        graph
            .add_edge(edge(EdgeRelation::Invalidates, &run, &claim))
            .expect("endpoints held");
        assert_eq!(graph.contradictions().len(), 1);
    }

    // --- promotion (PR-7 / IMPL-03) ------------------------------------------------------

    #[test]
    fn a_promotion_appends_a_version_and_keeps_the_identity() {
        let mut graph = EvidenceGraph::new();
        let node = node(NodeKind::Run, "trace_9f", "claim-1");
        let identity = graph.add_node(node.clone()).identity().clone();
        let checker = service("service:checker", &[ServiceRole::IndependentChecker]);
        let promotion = checker
            .validate(ValidationBasis::CheckedCertificate)
            .expect("authorized");

        let settled = graph
            .promote(&identity, ClaimStatus::Proposed, &promotion)
            .expect("a promotion from the expected status");
        assert_eq!(settled, ClaimStatus::Validated);

        let held = graph.node(&identity).expect("still held");
        assert_eq!(held.identity(), identity);
        assert_eq!(held.status(), ClaimStatus::Validated);
        assert_eq!(held.version(), 1);
        assert_eq!(
            held.status_history(),
            [ClaimStatus::Proposed, ClaimStatus::Validated]
        );
        assert_eq!(
            crate::claim_status::verify_promotion_history(&held.status_history()),
            Ok(())
        );
        let write = held.history().last().expect("the promotion");
        assert_eq!(write.service_identity(), Some("service:checker"));
        assert_eq!(
            write.validation_basis(),
            Some(ValidationBasis::CheckedCertificate)
        );
        // The graph still holds one node: a promotion is a new version, not a new record.
        assert_eq!(graph.node_count(), 1);
    }

    #[test]
    fn a_service_cannot_promote_its_own_production() {
        // INV-004, the half no type can carry: the producer and the checker are two values
        // that only meet here.
        let mut graph = EvidenceGraph::new();
        let produced = EvidenceNode::propose(
            NodeKind::Run,
            ArtifactRef::new("trace_9f").expect("well formed"),
            ClaimId::new("claim-1").expect("non-empty"),
            crate::node::IdempotencyKey::new("key-1").expect("non-empty"),
            provenance("service:checker"),
        );
        let identity = graph.add_node(produced).identity().clone();
        let itself = service("service:checker", &[ServiceRole::IndependentChecker]);
        let promotion = itself
            .validate(ValidationBasis::CheckedCertificate)
            .expect("authorized");
        assert_eq!(
            graph.promote(&identity, ClaimStatus::Proposed, &promotion),
            Err(PromotionRefusal::SelfCertification)
        );
        // Nothing moved.
        assert_eq!(
            graph.node(&identity).expect("still held").status(),
            ClaimStatus::Proposed
        );
        // …and a different service promotes the identical node, so the refusal is about the
        // identity match and not about the node.
        let independent = service("service:other", &[ServiceRole::IndependentChecker]);
        let promotion = independent
            .validate(ValidationBasis::CheckedCertificate)
            .expect("authorized");
        assert_eq!(
            graph.promote(&identity, ClaimStatus::Proposed, &promotion),
            Ok(ClaimStatus::Validated)
        );
    }

    #[test]
    fn a_lost_compare_and_set_is_a_status_conflict() {
        // plan §11.7: racing promotions cannot regress the lattice; the loser is told what
        // to re-read (plan §10.3 `StatusConflict`).
        let mut graph = EvidenceGraph::new();
        let identity = graph
            .add_node(node(NodeKind::Run, "trace_9f", "claim-1"))
            .identity()
            .clone();
        let runner = service("service:runner", &[ServiceRole::Execution]);
        let checker = service("service:checker", &[ServiceRole::IndependentChecker]);
        graph
            .promote(
                &identity,
                ClaimStatus::Proposed,
                &runner
                    .promote_to(ClaimStatus::Observed)
                    .expect("authorized"),
            )
            .expect("first writer wins");

        // The second writer still believes the claim is `proposed`.
        let refused = graph
            .promote(
                &identity,
                ClaimStatus::Proposed,
                &checker
                    .validate(ValidationBasis::CheckedCertificate)
                    .expect("authorized"),
            )
            .expect_err("a stale expectation");
        assert!(matches!(
            refused,
            PromotionRefusal::Rejected(PromotionRejected::Conflict(_))
        ));
        // Re-read and retry, and the lattice never regressed.
        assert_eq!(
            graph.promote(
                &identity,
                ClaimStatus::Observed,
                &checker
                    .validate(ValidationBasis::CheckedCertificate)
                    .expect("authorized"),
            ),
            Ok(ClaimStatus::Validated)
        );
    }

    #[test]
    fn a_downgrade_is_refused() {
        let mut graph = EvidenceGraph::new();
        let identity = graph
            .add_node(node(NodeKind::Run, "trace_9f", "claim-1"))
            .identity()
            .clone();
        let engine = service(
            "service:engine",
            &[ServiceRole::Verification, ServiceRole::Execution],
        );
        graph
            .promote(
                &identity,
                ClaimStatus::Proposed,
                &engine.promote_to(ClaimStatus::Bounded).expect("authorized"),
            )
            .expect("authorized and a promotion");
        // plan §5.1's "lowering assurance from exhaustive to sampled", refused by the
        // lattice even though the service is authorized for `sampled`.
        let refused = graph
            .promote(
                &identity,
                ClaimStatus::Bounded,
                &engine.promote_to(ClaimStatus::Sampled).expect("authorized"),
            )
            .expect_err("a downgrade");
        assert!(matches!(
            refused,
            PromotionRefusal::Rejected(PromotionRejected::Regression(_))
        ));
        assert_eq!(
            graph.node(&identity).expect("held").status(),
            ClaimStatus::Bounded
        );
    }

    #[test]
    fn authority_and_the_lattice_are_independent_guards() {
        // A proof service is authorized for `proved` and is still refused over a `refuted`
        // claim, because the lattice puts a checked counterexample above a proof.
        let mut graph = EvidenceGraph::new();
        let identity = graph
            .add_node(node(NodeKind::Run, "trace_9f", "claim-1"))
            .identity()
            .clone();
        let prover = service(
            "service:lean",
            &[ServiceRole::ProofService, ServiceRole::IndependentChecker],
        );
        graph
            .promote(
                &identity,
                ClaimStatus::Proposed,
                &prover.promote_to(ClaimStatus::Refuted).expect("authorized"),
            )
            .expect("any → refuted");
        let promotion = prover.promote_to(ClaimStatus::Proved).expect("authorized");
        assert_eq!(promotion.status(), ClaimStatus::Proved);
        assert!(matches!(
            graph.promote(&identity, ClaimStatus::Refuted, &promotion),
            Err(PromotionRefusal::Rejected(PromotionRejected::Regression(_)))
        ));
    }

    #[test]
    fn promoting_an_unheld_node_is_refused() {
        let mut graph = EvidenceGraph::new();
        let orphan = node(NodeKind::Run, "trace_9f", "claim-1").identity();
        let checker = service("service:checker", &[ServiceRole::IndependentChecker]);
        assert_eq!(
            graph.promote(
                &orphan,
                ClaimStatus::Proposed,
                &checker
                    .validate(ValidationBasis::CheckedCertificate)
                    .expect("authorized"),
            ),
            Err(PromotionRefusal::UnknownNode)
        );
        assert_eq!(
            PromotionRefusal::UnknownNode.to_string(),
            "the graph holds no node under that identity"
        );
    }

    #[test]
    fn the_exit_sentence_holds_over_every_role() {
        // PR 7's exit: "an untrusted client cannot promote a proposal to validated/proved."
        // The library-level reading, swept: of the five roles a deployment can declare, only
        // two reach those two statuses, and no non-service actor can hold any role at all.
        let mut graph = EvidenceGraph::new();
        let identity = graph
            .add_node(node(NodeKind::Run, "trace_9f", "claim-1"))
            .identity()
            .clone();
        for role in ServiceRole::ALL {
            let declared = service("service:s", &[role]);
            let validated = declared.validate(ValidationBasis::CheckedCertificate);
            let proved = declared.promote_to(ClaimStatus::Proved);
            assert_eq!(
                validated.is_ok(),
                role == ServiceRole::IndependentChecker,
                "{role}"
            );
            assert_eq!(proved.is_ok(), role == ServiceRole::ProofService, "{role}");
            if let Ok(promotion) = validated {
                assert_eq!(
                    graph.promote(&identity, ClaimStatus::Proposed, &promotion),
                    Ok(ClaimStatus::Validated)
                );
            }
        }
        // …and the graph did reach `validated` exactly once, so the sweep is not vacuous.
        assert_eq!(
            graph.node(&identity).expect("held").status(),
            ClaimStatus::Validated
        );
        for text in ["agent:swarm-1", "human:ada", "ci:nightly"] {
            let actor = crate::actor::ActorId::new(text).expect("well formed");
            assert!(crate::actor::ServiceIdentity::new(actor).is_err(), "{text}");
        }
    }

    // --- conflicts (PR-7 / IMPL-04) ------------------------------------------------------

    fn materialized(
        subjects: Vec<crate::conflict::ConflictSubject>,
    ) -> crate::conflict::MaterializedConflict {
        crate::conflict::Conflict::between(subjects, ground(), experiment())
            .expect("two distinct parties")
            .materialize(
                &DefaultNaming::new(),
                ArtifactRef::new("conflict_9f").expect("well formed"),
                ClaimId::new("claim-1").expect("non-empty"),
                crate::node::IdempotencyKey::new("key-c").expect("non-empty"),
                provenance("agent:integrator"),
            )
    }

    #[test]
    fn a_contradiction_materializes_a_conflict_node() {
        // plan §11.7: the contradiction is reported, a conflict node is written, and no
        // party is chosen.
        let (mut graph, run, claim) = seeded();
        let supports = edge(EdgeRelation::Supports, &run, &claim);
        let refutes = edge(EdgeRelation::Refutes, &run, &claim);
        graph.add_edge(supports.clone()).expect("endpoints held");
        graph.add_edge(refutes.clone()).expect("endpoints held");
        let found = graph.contradictions();
        assert_eq!(found.len(), 1);
        assert_eq!(graph.conflicts().count(), 0);

        let conflict = materialized(vec![
            ConflictSubject::Edge(EdgeRef::of(&supports)),
            ConflictSubject::Edge(EdgeRef::of(&refutes)),
            ConflictSubject::Node(NodeRef::of(&claim)),
        ]);
        let identity = graph.add_conflict(&conflict).expect("subjects held");
        assert_eq!(graph.conflicts().count(), 1);
        assert_eq!(graph.unresolved_conflicts().count(), 1);
        let node = graph.node(&identity).expect("held");
        assert_eq!(node.kind(), NodeKind::Conflict);
        assert_eq!(node.status(), ClaimStatus::Proposed);
        // Both contradicting edges are still held, and neither was chosen.
        assert!(graph.edge(&supports.identity()).is_some());
        assert!(graph.edge(&refutes.identity()).is_some());
        assert_eq!(graph.contradictions().len(), 1);
    }

    #[test]
    fn a_conflict_about_an_unheld_subject_is_refused() {
        // docs/44: "references must resolve", and a conflict's own edges are edges.
        let (mut graph, _run, claim) = seeded();
        let stranger = node(NodeKind::AbstractionMap, "map_z", "claim-9");
        let conflict = materialized(vec![
            ConflictSubject::Node(NodeRef::of(&claim)),
            ConflictSubject::Node(NodeRef::of(&stranger)),
        ]);
        assert!(graph.add_conflict(&conflict).is_err());
        // The conflict node itself is still visible: append-only has nothing to roll back
        // with, and a half-linked conflict is better than a silent one.
        assert_eq!(graph.conflicts().count(), 1);
    }

    #[test]
    fn an_unresolved_conflict_stays_visible() {
        let (mut graph, run, claim) = seeded();
        let conflict = materialized(vec![
            ConflictSubject::Node(NodeRef::of(&claim)),
            ConflictSubject::Node(NodeRef::of(&run)),
        ]);
        let identity = graph.add_conflict(&conflict).expect("subjects held");
        assert_eq!(graph.unresolved_conflicts().count(), 1);
        // Promoting it anywhere below the top leaves it unresolved.
        let engine = service("service:engine", &[ServiceRole::Verification]);
        graph
            .promote(
                &identity,
                ClaimStatus::Proposed,
                &engine.promote_to(ClaimStatus::Bounded).expect("authorized"),
            )
            .expect("authorized and a promotion");
        assert_eq!(graph.unresolved_conflicts().count(), 1);
    }

    #[test]
    fn only_a_superseding_promotion_resolves() {
        // docs/44's retirement row is the only one that retires a claim, and only the
        // policy owner holds it.
        let (mut graph, run, claim) = seeded();
        let decision = node(NodeKind::Decision, "decision_9f", "claim-1");
        graph.add_node(decision.clone());
        let conflict = materialized(vec![
            ConflictSubject::Node(NodeRef::of(&claim)),
            ConflictSubject::Node(NodeRef::of(&run)),
        ]);
        let identity = graph.add_conflict(&conflict).expect("subjects held");
        let resolution = Resolution::new(
            NodeRef::of(&decision),
            crate::conflict::ResolutionOutcome::Retained(ConflictSubject::Node(NodeRef::of(
                &claim,
            ))),
            [ArtifactRef::new("trace_9f").expect("well formed")],
            provenance("human:ada"),
        )
        .expect("named evidence");

        // No other role can even mint the promotion this needs.
        for role in ServiceRole::ALL {
            let declared = service("service:s", &[role]);
            assert_eq!(
                declared.promote_to(ClaimStatus::Superseded).is_ok(),
                role == ServiceRole::PolicyOwner,
                "{role}"
            );
        }
        // …and a promotion to anything else is refused as a choice rather than a resolution.
        let checker = service("service:checker", &[ServiceRole::IndependentChecker]);
        assert_eq!(
            graph.resolve_conflict(
                &identity,
                &resolution,
                &checker
                    .validate(ValidationBasis::CheckedCertificate)
                    .expect("authorized"),
            ),
            Err(ResolutionRefusal::NotARetirement {
                status: ClaimStatus::Validated
            })
        );
        // A node that is not a conflict cannot be resolved.
        let claim_identity = node(NodeKind::Property, "prop_9f", "claim-1").identity();
        let owner = service("service:owner", &[ServiceRole::PolicyOwner]);
        assert_eq!(
            graph.resolve_conflict(
                &claim_identity,
                &resolution,
                &owner
                    .promote_to(ClaimStatus::Superseded)
                    .expect("authorized"),
            ),
            Err(ResolutionRefusal::NotAConflict)
        );
    }

    #[test]
    fn resolving_a_conflict_deletes_nothing() {
        // plan §4.6: "nothing is rewritten in place"; docs/44: "policy/owner with explicit
        // edge". Both halves land, and the whole graph is still there afterwards.
        let (mut graph, run, claim) = seeded();
        let decision = node(NodeKind::Decision, "decision_9f", "claim-1");
        graph.add_node(decision.clone());
        let conflict = materialized(vec![
            ConflictSubject::Node(NodeRef::of(&claim)),
            ConflictSubject::Node(NodeRef::of(&run)),
        ]);
        let identity = graph.add_conflict(&conflict).expect("subjects held");
        let before_nodes = graph.node_count();
        let before_edges = graph.edge_count();
        let held: Vec<EvidenceIdentity> = graph.nodes().map(|(id, _)| id.clone()).collect();
        let held_edges: Vec<EvidenceIdentity> = graph.edges().map(|(id, _)| id.clone()).collect();

        let resolution = Resolution::new(
            NodeRef::of(&decision),
            crate::conflict::ResolutionOutcome::AllSuperseded,
            [ArtifactRef::new("trace_9f").expect("well formed")],
            provenance("human:ada"),
        )
        .expect("named evidence");
        let owner = service("service:owner", &[ServiceRole::PolicyOwner]);
        let settled = graph
            .resolve_conflict(
                &identity,
                &resolution,
                &owner
                    .promote_to(ClaimStatus::Superseded)
                    .expect("authorized"),
            )
            .expect("a policy owner retires a conflict");
        assert_eq!(settled, ClaimStatus::Superseded);

        // Nothing was removed, and the explicit edge was added.
        assert_eq!(graph.node_count(), before_nodes);
        assert_eq!(graph.edge_count(), before_edges + 1);
        for identity in held {
            assert!(graph.node(&identity).is_some());
        }
        for identity in held_edges {
            assert!(graph.edge(&identity).is_some());
        }
        assert_eq!(graph.edges_of_kind(EdgeKind::Supersedes).count(), 1);
        // The conflict node is still held, at a new version, and its history says what
        // happened rather than the record disappearing.
        let resolved = graph.node(&identity).expect("still held");
        assert_eq!(resolved.status(), ClaimStatus::Superseded);
        assert_eq!(resolved.version(), 1);
        assert_eq!(
            resolved.status_history(),
            [ClaimStatus::Proposed, ClaimStatus::Superseded]
        );
        assert_eq!(graph.conflicts().count(), 1);
        assert_eq!(graph.unresolved_conflicts().count(), 0);
        // The CONFLICTS_WITH link between the parties survives the resolution.
        assert_eq!(graph.edges_of_kind(EdgeKind::ConflictsWith).count(), 1);
    }

    #[test]
    fn a_second_producers_identical_assertion_is_a_second_edge() {
        // docs/44 keeps credit; two producers are two pieces of evidence, not one with a
        // winner.
        let (mut graph, run, claim) = seeded();
        let first = edge(EdgeRelation::Supports, &run, &claim);
        let second = EvidenceEdge::new(
            EdgeRelation::Supports,
            NodeRef::of(&run),
            NodeRef::of(&claim),
            [ArtifactRef::new("cert_9f").expect("well formed")],
            provenance("agent:swarm-2"),
        )
        .expect("distinct endpoints");
        graph.add_edge(first).expect("endpoints held");
        graph.add_edge(second).expect("endpoints held");
        assert_eq!(graph.edge_count(), 2);
    }
}
