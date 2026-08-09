//! Provenance as a derivation closure: what an artifact was made from, transitively
//! (docs/44 "Credit and provenance", RFC 0038 "Queries" P1–P7).
//!
//! # The query this answers
//!
//! > Every node records actor/tool/model version, prompts/tool inputs as policy permits,
//! > source retrievals, and derivation. This supports scientific credit, debugging,
//! > benchmark analysis, and reproduction without granting authority based on identity.
//! >
//! > — docs/44, "Credit and provenance"
//!
//! RFC 0038 lists `provenance` among seven queries and bn-xsuz2's re-verification found it
//! the one of the six that is answerable from content the graph already holds: every node
//! carries [`crate::provenance::Provenance::inputs`], and `observe.ingest`, `evidence.link`
//! and `whiteboard.compile` all write it. [`crate::graph::EvidenceGraph::nodes_by_producer`]
//! answers the *who-produced-what* grain. This module answers the *derivation* grain, which
//! is the other half of the sentence and the half "reproduction" needs.
//!
//! # P1 — the surface is crate-level, because the wire cannot spell the answer
//!
//! The declared surface was searched before this was concluded — the shape RFC 0026's IDL
//! revisions 1.5, 1.7 and 1.8 each took, and the search RFC 0038 W10 ran for the compiler.
//! Serving already-declared vocabulary is the cheaper payment, and it is not available here
//! for three reasons, the second decisive.
//!
//! - **`EvidenceQuery`'s traversal is over edges, and a derivation input is not an edge.**
//!   `roots` and `max_depth` are `rule evidence.traversal`, which says in clause 1 that "an
//!   edge is an adjacency" and in clause 2 that a node "is at depth `d + 1` when an *edge*
//!   incident to a depth-`d` node names it". `provenance.inputs` is not an edge; of the
//!   thirteen edge kinds `DEPENDS_ON` is the one that would carry a derivation, and nothing
//!   appends it. Reading provenance adjacency into `roots` would move which handles an
//!   already-declared scope selects for every existing client — `rule
//!   versioning.compatible_change`'s "relaxing a server-side constraint" — and it would
//!   *contradict* a stated clause rather than fill a silence. That is what separates it from
//!   the 1.8 revision, which was legitimate precisely because nothing had been said.
//! - **The response shape cannot carry the typed absence.** `evidence.query` answers
//!   `nodes: list<EvidenceHandle>` and `edges: list<EvidenceHandle>`. An input that names an
//!   artifact the graph holds no node about has **no** `ev_` handle, by construction — that
//!   is what "the graph does not hold it" means. Listing it would name a node the graph does
//!   not have; omitting it is the silent skip. [`AbsentInput`] is inexpressible there, the
//!   same way a section assignment and a status column were inexpressible for
//!   [`crate::view`].
//! - **Not even one step of the relation is spellable.** `EvidenceQuery` filters by node
//!   kind, edge kind, status, claim, roots and depth. None of them names an artifact handle,
//!   so a client cannot ask "which nodes are about `trace_9f`" — and therefore cannot walk
//!   the closure for itself out of `evidence.get`'s `Opaque` either, even though that
//!   `Opaque` does carry `provenance.inputs`. The content is on the wire; the *relation* has
//!   no vocabulary.
//!
//! `evidence.subscribe` inherits `evidence.query`'s predicate under `rule
//! subscription.delivery`, so it inherits the same limit. `context.compile` answers a
//! `context-pack.schema.json` pack — a different artifact class with a `question` and
//! declared `guarantees`. So this is a **crate-level typed surface and not a wire
//! operation**: no operation is added, no `EvidenceQuery` member is added, no rule is
//! stated, and the protocol version does not move. That is [`crate::view`]'s order and
//! `continuum-forge`'s before it — the crate-level surface lands first, and the wire
//! spelling lands when its RFC decides what it would cost.
//!
//! # P2 — the closure runs backward, along `provenance.inputs`, and only that direction
//!
//! A derivation closure is the transitive *ancestry*: what this artifact was made from. RFC
//! 0038 W4 gives the relation its own sentence — "the references become the compiled node's
//! `provenance.inputs` — derivation, never assertion" — and docs/44 names reproduction,
//! which is what an ancestry is for.
//!
//! `rule evidence.traversal` clause 1 walks an *edge* in either direction, because "an
//! edge's direction is what it ASSERTS, not which way relevance runs". That argument does
//! **not** carry over, and copying it would be the mistake it warns against, taken from the
//! other side: a derivation input is not an assertion about relevance, it *is* the relation.
//! "Made from" and "used by" are two different questions.
//!
//! The forward direction — what was derived *from* this — is deliberately not delivered, and
//! the reason is not effort. The backward closure can state the sense in which it is
//! complete: every place it could not continue is a named input, so [`AbsentInput`] lists
//! its own frontier. The forward relation cannot. Nothing enumerates the artifacts nobody
//! published, so a `dependents` answering the empty set would read "nothing derives from
//! this" when all it can mean is "this graph holds nothing that does" — the empty-success
//! shape `rule errors.unsupported_surface` forbids, reached from the reading side. A
//! *closure* is a claim of completeness, and only one of the two directions can say in what
//! sense it has it.
//!
//! # P3 — an input resolves to every held node about that artifact
//!
//! The resolves relation is [`crate::whiteboard`]'s own — a reference names an artifact, and
//! the graph holds zero or more nodes *about* that artifact — and W6's reason applies
//! verbatim: "where the graph holds several nodes about one cited artifact, one edge is
//! drawn from each: choosing between them is a semantic judgement the compiler cannot make".
//! A closure that kept one node per artifact would be making exactly that judgement, so all
//! of them are ancestors.
//!
//! # P4 — depth counts derivation steps, a seed is 0, and the least depth wins
//!
//! Deliberately `rule evidence.traversal` clause 2 with "edge" replaced by "derivation
//! step", so a reader who knows the wire's depth discipline knows this one: a seed is at
//! depth 0, a node an input names from depth `d` is at depth `d + 1`, and a node reachable
//! by two routes takes the shorter — which is what makes a diamond one answer rather than
//! two. Clause 3's reading is kept too: an absent bound is unbounded and `0` selects the
//! seeds alone, so nothing a caller could ask becomes unaskable.
//!
//! # P5 — the caller chooses the seeds; the graph resolves them
//!
//! [`crate::view`]'s V1, unchanged: a closure that chose its own scope would be choosing for
//! its caller. The seeds are nodes the caller selected — through
//! [`crate::graph::EvidenceGraph`]'s selection layer or otherwise — and the graph is
//! consulted only to resolve inputs. A seed the graph does not hold is therefore not a
//! refusal: it is a node the caller handed over, it sits at depth 0, and its own inputs
//! still resolve.
//!
//! # P6 — an absent input is a typed absence, never a skip and never a refusal
//!
//! > Comments, logs, docs, model strings, and production payloads cannot issue instructions
//! > to the workbench or proof service.
//! >
//! > — INV-016
//!
//! `provenance.inputs` is written by operations acting for admitted actors, so an entry is
//! not hostile — but it is still a *name*, and a name may denote an artifact this graph does
//! not hold: a `trace_` that never became an evidence node, a receipt held by another
//! deployment, an artifact in a class that has no node kind at all. W4's "references must
//! resolve" is a rule on the **write** path, where the graph is the authority over what a
//! note may cite; a read that refused every graph with an outside reference would make
//! partiality an error.
//!
//! So an unresolvable input is neither dropped nor refused. It is an [`AbsentInput`]: the
//! artifact named, the depth it was reached at, and every held node that named it. INV-008's
//! discipline is then satisfied at the level of the whole answer —
//! [`DerivationClosure::incompleteness`] returns *distinct typed reasons*, because "the
//! caller's own bound stopped me" and "the graph does not hold what was named" are two
//! different facts about the same empty space and a bare boolean would conflate them.
//!
//! # P7 — a cycle terminates the walk and is not an inconclusive outcome
//!
//! An [`crate::provenance::ArtifactRef`] is a name a producer supplies rather than a digest
//! this crate computes, so a cycle is constructible: a node about `a` may name `b` while a
//! node about `b` names `a`. Each node is expanded exactly once, keyed by content identity,
//! so the walk terminates and the closure is the same *set* whichever way it was entered.
//! That set is exactly right, so there is nothing inconclusive to type — a cycle earns a
//! test, not a member. (INV-008 types outcomes that fall short of an answer; this one does
//! not.)
//!
//! # This is a computed value, not an artifact class
//!
//! A closure mints no identity, publishes nothing, and is not one of plan §4.4's
//! content-addressed classes, so it needs no schema document and this module pins no bytes —
//! exactly as [`crate::graph::Contradiction`], [`crate::whiteboard::Compilation`] and
//! [`crate::view::WhiteboardView`] need none. [`DerivationClosure::to_record`] renders it for
//! a reader, and its determinism *is* asserted on encoded bytes; what is not asserted is a
//! normative shape, because inventing one is the act INV-003 reserves for
//! `notes/plan/schemas/`.
//!
//! # Clause → test
//!
//! | Clause | Source | Test |
//! |---|---|---|
//! | the walk is over inputs, not edges | P1 | `the_walk_follows_inputs_and_never_edges` |
//! | the closure runs backward only | P2 | `derivation_runs_backward_and_omits_a_consumer` |
//! | an input resolves to every node about it | P3 | `an_input_naming_two_nodes_reaches_both` |
//! | depth counts derivation steps, least wins | P4 | `a_diamond_reaches_its_apex_once_at_the_shorter_depth` |
//! | a bound of 0 is the seeds alone | P4 | `a_bound_of_zero_is_the_seeds_alone` |
//! | the caller chooses the seeds | P5 | `a_seed_the_graph_does_not_hold_still_names_its_inputs` |
//! | an absent input is named, never skipped | P6, INV-016 | `a_chain_with_a_dangling_input_names_the_absence` |
//! | incompleteness is typed and distinct | INV-008 | `the_two_reasons_a_closure_is_incomplete_are_distinct` |
//! | a cycle terminates | P7 | `a_cycle_terminates_and_is_one_set` |
//! | deterministic | GOV-1-03, INV-005 | `two_insertion_orders_close_identically` |

use std::collections::{BTreeMap, BTreeSet};

use continuum_value::value::Value;

use crate::actor::{ActorId, field};
use crate::claim_status::ClaimStatus;
use crate::graph::EvidenceGraph;
use crate::identity::{EvidenceIdentity, EvidenceNaming};
use crate::node::{ClaimId, EvidenceNode, NodeKind};
use crate::provenance::ArtifactRef;

/// One held node the closure reached, and how far from a seed it was.
///
/// Everything on it is read off the held node. There is no derived field and no verdict: a
/// closure reports what the graph holds, and [`Self::status`] is [`EvidenceNode::status`]
/// verbatim for the reason [`crate::view`]'s V5 gives — nothing here reads the lattice,
/// compares a version, or takes a [`crate::authority::Promotion`], and there is no code path
/// from this module to [`EvidenceGraph::promote`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DerivationRow {
    depth: u32,
    kind: NodeKind,
    artifact: ArtifactRef,
    claim: ClaimId,
    status: ClaimStatus,
    actor: ActorId,
}

impl DerivationRow {
    /// How many derivation steps from the nearest seed (P4). A seed is `0`.
    #[must_use]
    pub const fn depth(&self) -> u32 {
        self.depth
    }

    /// The node kind.
    #[must_use]
    pub const fn kind(&self) -> NodeKind {
        self.kind
    }

    /// The artifact this node is *about* — what an input names when it resolves here (P3).
    #[must_use]
    pub const fn artifact(&self) -> &ArtifactRef {
        &self.artifact
    }

    /// The claim its status is linearized against (plan §11.7).
    #[must_use]
    pub const fn claim(&self) -> &ClaimId {
        &self.claim
    }

    /// The status the graph holds, verbatim.
    #[must_use]
    pub const fn status(&self) -> ClaimStatus {
        self.status
    }

    /// Who produced it (docs/44 "Credit and provenance").
    #[must_use]
    pub const fn actor(&self) -> &ActorId {
        &self.actor
    }

    fn of(node: &EvidenceNode, depth: u32) -> Self {
        Self {
            depth,
            kind: node.kind(),
            artifact: node.artifact().clone(),
            claim: node.claim_id().clone(),
            status: node.status(),
            actor: node.provenance().actor().clone(),
        }
    }

    fn to_record(&self, identity: &EvidenceIdentity, naming: &impl EvidenceNaming) -> Value {
        Value::record([
            (field("actor"), Value::text(self.actor.as_str())),
            (field("artifact"), self.artifact.to_value()),
            (field("claim"), Value::text(self.claim.as_str())),
            (field("depth"), Value::nat(u128::from(self.depth))),
            (
                field("evidence"),
                Value::text(naming.name(identity).as_str()),
            ),
            (field("kind"), Value::text(self.kind.as_str())),
            (field("status"), Value::text(self.status.as_str())),
        ])
        .expect("the seven member names are distinct compile-time constants")
    }
}

/// An input that named an artifact the graph holds no node about (P6, INV-016).
///
/// A *typed absence*, never a silent skip: the artifact is named, the depth it was reached
/// at is recorded, and so is every held node that named it — so a caller can go and fetch
/// what is missing, or say why it never will.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AbsentInput {
    artifact: ArtifactRef,
    depth: u32,
    named_by: BTreeSet<EvidenceIdentity>,
}

impl AbsentInput {
    /// The artifact handle the input named.
    #[must_use]
    pub const fn artifact(&self) -> &ArtifactRef {
        &self.artifact
    }

    /// The depth it would have sat at: one step past the nearest node that named it (P4).
    #[must_use]
    pub const fn depth(&self) -> u32 {
        self.depth
    }

    /// Every held node in the closure whose `provenance.inputs` named it, in content order.
    pub fn named_by(&self) -> impl ExactSizeIterator<Item = &EvidenceIdentity> {
        self.named_by.iter()
    }

    fn to_record(&self, naming: &impl EvidenceNaming) -> Value {
        Value::record([
            (field("artifact"), self.artifact.to_value()),
            (field("depth"), Value::nat(u128::from(self.depth))),
            (
                field("named_by"),
                Value::seq(
                    self.named_by
                        .iter()
                        .map(|identity| Value::text(naming.name(identity).as_str())),
                )
                .expect("a handle list nests one level"),
            ),
        ])
        .expect("the three member names are distinct compile-time constants")
    }
}

/// Why a closure is not the whole ancestry (INV-008).
///
/// Two facts about the same empty space, and they are not the same fact: one is the
/// caller's own bound and the other is the graph's partiality. A bare "incomplete" flag
/// would conflate them, and a caller that could act on the first — raise the bound — cannot
/// act on the second.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Incompleteness {
    /// The caller's `max_depth` stopped the walk with derivation steps still unwalked.
    ///
    /// Raised only when continuing would have added something: a bounded walk whose last
    /// level named nothing new is *complete*, and says so.
    Bound {
        /// The bound that stopped it.
        max_depth: u32,
    },
    /// At least one input named an artifact the graph holds no node about (P6).
    AbsentInputs {
        /// How many distinct artifacts were named and not held.
        count: usize,
    },
}

impl Incompleteness {
    /// The reason's canonical token.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Bound { .. } => "bound",
            Self::AbsentInputs { .. } => "absent-inputs",
        }
    }
}

impl core::fmt::Display for Incompleteness {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Bound { max_depth } => {
                write!(
                    f,
                    "the walk stopped at max_depth {max_depth} with more to reach"
                )
            }
            Self::AbsentInputs { count } => write!(
                f,
                "{count} named input artifacts are not held by this graph"
            ),
        }
    }
}

/// The transitive ancestry of a set of seed nodes, over `provenance.inputs`.
///
/// Built by [`DerivationClosure::of`] or [`DerivationClosure::bounded`]. Immutable, and a
/// function of what the graph holds and which seeds it was given: both halves iterate a
/// [`BTreeMap`] keyed by content, so the answer is a function of the content and of nothing
/// else (GOV-1-03, INV-005).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DerivationClosure {
    reached: BTreeMap<EvidenceIdentity, DerivationRow>,
    absent: BTreeMap<ArtifactRef, AbsentInput>,
    max_depth: Option<u32>,
    truncated: bool,
}

impl DerivationClosure {
    /// The unbounded closure of a set of seeds (P4: an absent bound is unbounded).
    pub fn of<'a>(
        graph: &'a EvidenceGraph,
        seeds: impl IntoIterator<Item = &'a EvidenceNode>,
    ) -> Self {
        Self::bounded(graph, seeds, None)
    }

    /// The closure of every node a graph holds.
    ///
    /// The one scope this module names for itself, because it is the one that chooses
    /// nothing (P5). Its [`Self::absent`] is exactly the set of artifacts the graph's own
    /// records name and do not hold — "what does this graph depend on that it has not got".
    #[must_use]
    pub fn of_graph(graph: &EvidenceGraph) -> Self {
        Self::of(graph, graph.nodes().map(|(_, node)| node))
    }

    /// The closure of a set of seeds, bounded in derivation steps.
    ///
    /// `max_depth` is `None` for unbounded and `Some(0)` for the seeds alone — `rule
    /// evidence.traversal` clause 3's reading, kept so that nothing a caller could ask
    /// becomes unaskable (P4).
    pub fn bounded<'a>(
        graph: &'a EvidenceGraph,
        seeds: impl IntoIterator<Item = &'a EvidenceNode>,
        max_depth: Option<u32>,
    ) -> Self {
        // The resolves relation, indexed once. A `roots` clause is a statement about the
        // graph and re-deriving it per candidate would re-walk the graph once per node —
        // the daemon's `Reach` discipline, at this layer.
        let mut about: BTreeMap<&ArtifactRef, Vec<(&EvidenceIdentity, &EvidenceNode)>> =
            BTreeMap::new();
        for (identity, node) in graph.nodes() {
            about
                .entry(node.artifact())
                .or_default()
                .push((identity, node));
        }

        let mut closure = Self {
            reached: BTreeMap::new(),
            absent: BTreeMap::new(),
            max_depth,
            truncated: false,
        };
        let mut frontier: Vec<&EvidenceNode> = Vec::new();
        for seed in seeds {
            let identity = seed.identity();
            if closure.reached.contains_key(&identity) {
                continue;
            }
            closure.reached.insert(identity, DerivationRow::of(seed, 0));
            frontier.push(seed);
        }

        let mut depth = 0_u32;
        while !frontier.is_empty() {
            let next_depth = depth + 1;
            // The bound truncates everything past it, absences included: reporting an
            // absence at `max_depth + 1` would be answering past the bound the caller set.
            let within = max_depth.is_none_or(|bound| next_depth <= bound);
            let mut next: Vec<&EvidenceNode> = Vec::new();
            for node in &frontier {
                for input in node.provenance().inputs() {
                    match about.get(input) {
                        Some(holders) => {
                            for (identity, held) in holders {
                                if closure.reached.contains_key(*identity) {
                                    continue;
                                }
                                if !within {
                                    closure.truncated = true;
                                    continue;
                                }
                                closure.reached.insert(
                                    (*identity).clone(),
                                    DerivationRow::of(held, next_depth),
                                );
                                next.push(held);
                            }
                        }
                        None => {
                            if let Some(recorded) = closure.absent.get_mut(input) {
                                recorded.named_by.insert(node.identity());
                                continue;
                            }
                            if !within {
                                closure.truncated = true;
                                continue;
                            }
                            closure.absent.insert(
                                input.clone(),
                                AbsentInput {
                                    artifact: input.clone(),
                                    depth: next_depth,
                                    named_by: [node.identity()].into_iter().collect(),
                                },
                            );
                        }
                    }
                }
            }
            frontier = next;
            depth = next_depth;
        }
        closure
    }

    /// Every held node the closure reached, in content order, with its depth.
    ///
    /// Includes the seeds, at depth 0 — a closure is reflexive, and `max_depth = 0` has to
    /// name something for clause 3's reading to hold.
    pub fn reached(&self) -> impl ExactSizeIterator<Item = (&EvidenceIdentity, &DerivationRow)> {
        self.reached.iter()
    }

    /// The row for one identity, when the closure reached it.
    #[must_use]
    pub fn row(&self, identity: &EvidenceIdentity) -> Option<&DerivationRow> {
        self.reached.get(identity)
    }

    /// Whether the closure reached an identity.
    #[must_use]
    pub fn holds(&self, identity: &EvidenceIdentity) -> bool {
        self.reached.contains_key(identity)
    }

    /// Every named-but-absent input, in artifact order (P6).
    pub fn absent(&self) -> impl ExactSizeIterator<Item = &AbsentInput> {
        self.absent.values()
    }

    /// How many held nodes were reached.
    #[must_use]
    pub fn reached_count(&self) -> usize {
        self.reached.len()
    }

    /// How many distinct artifacts were named and not held.
    #[must_use]
    pub fn absent_count(&self) -> usize {
        self.absent.len()
    }

    /// The bound the walk ran under, if any.
    #[must_use]
    pub const fn max_depth(&self) -> Option<u32> {
        self.max_depth
    }

    /// The greatest depth any reached node sits at.
    #[must_use]
    pub fn depth(&self) -> u32 {
        self.reached
            .values()
            .map(DerivationRow::depth)
            .max()
            .unwrap_or(0)
    }

    /// Every distinct reason this closure is not the whole ancestry (INV-008).
    ///
    /// Empty exactly when [`Self::is_complete`]. The order is the declaration order of
    /// [`Incompleteness`] and is therefore a function of nothing but which reasons hold.
    #[must_use]
    pub fn incompleteness(&self) -> Vec<Incompleteness> {
        let mut reasons = Vec::new();
        if let (true, Some(max_depth)) = (self.truncated, self.max_depth) {
            reasons.push(Incompleteness::Bound { max_depth });
        }
        if !self.absent.is_empty() {
            reasons.push(Incompleteness::AbsentInputs {
                count: self.absent.len(),
            });
        }
        reasons
    }

    /// Whether every input named by every reached node resolved, within the bound.
    ///
    /// True means the closure *is* the ancestry: there is no further derivation step to
    /// take and nothing was named that this graph does not hold.
    #[must_use]
    pub fn is_complete(&self) -> bool {
        self.incompleteness().is_empty()
    }

    /// This closure as a record: what was reached, what was not, and why it stopped.
    ///
    /// `naming` is a parameter for the reason [`crate::conflict::Conflict::materialize`]'s
    /// is — RFC 0038 D2 puts naming in the deployment's seam, and a closure that named
    /// artifacts for its caller would be choosing an identity kernel. The record is a
    /// rendering and not a schema (see the module docs): it is asserted deterministic, never
    /// normative.
    pub fn to_record(&self, naming: &impl EvidenceNaming) -> Value {
        Value::record([
            (
                field("absent"),
                Value::seq(self.absent.values().map(|it| it.to_record(naming)))
                    .expect("an absent list nests"),
            ),
            (
                field("incomplete"),
                Value::seq(
                    self.incompleteness()
                        .iter()
                        .map(|reason| Value::text(reason.as_str())),
                )
                .expect("a reason list nests one level"),
            ),
            (
                field("max_depth"),
                self.max_depth
                    .map_or(Value::Null, |bound| Value::nat(u128::from(bound))),
            ),
            (
                field("reached"),
                Value::seq(
                    self.reached
                        .iter()
                        .map(|(identity, row)| row.to_record(identity, naming)),
                )
                .expect("a row list nests"),
            ),
        ])
        .expect("the four member names are distinct compile-time constants")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::edge::EdgeRelation;
    use crate::edge::tests::edge;
    use crate::identity::DefaultNaming;
    use crate::node::{IdempotencyKey, NodeRef};
    use crate::provenance::{Provenance, Timestamp};

    /// A node about `artifact`, derived from `inputs`, asserting about `claim`.
    ///
    /// The claim is the parameter rather than the idempotency key because a key is
    /// deliberately *outside* a node's identity (plan §11.7): two records that say the same
    /// thing under two keys are one node, so varying the key cannot produce two nodes about
    /// one artifact and varying the claim can.
    fn derived(artifact: &str, inputs: &[&str], claim: &str) -> EvidenceNode {
        EvidenceNode::propose(
            NodeKind::Run,
            ArtifactRef::new(artifact).expect("well formed"),
            ClaimId::new(claim).expect("non-empty"),
            IdempotencyKey::new("key-1").expect("non-empty"),
            Provenance::new(
                ActorId::new("agent:swarm-1").expect("well formed"),
                Timestamp::new("2026-08-01T12:00:00.000Z").expect("well formed"),
                inputs
                    .iter()
                    .map(|it| ArtifactRef::new(it).expect("well formed")),
            ),
        )
    }

    fn artifact(text: &str) -> ArtifactRef {
        ArtifactRef::new(text).expect("well formed")
    }

    /// The artifacts a closure reached, in content order.
    fn subjects(closure: &DerivationClosure) -> Vec<String> {
        let mut listed: Vec<String> = closure
            .reached()
            .map(|(_, row)| format!("{}@{}", row.artifact(), row.depth()))
            .collect();
        listed.sort();
        listed
    }

    fn seeded(nodes: &[EvidenceNode]) -> EvidenceGraph {
        let mut graph = EvidenceGraph::new();
        for node in nodes {
            graph.add_node(node.clone());
        }
        graph
    }

    #[test]
    fn the_walk_follows_inputs_and_never_edges() {
        // P1: the relation is `provenance.inputs`, which is not the edge adjacency `rule
        // evidence.traversal` defines. Positive and negative in one graph: `trace_b` is an
        // input of `trace_a` and is reached; `trace_c` is joined to `trace_a` by a SUPPORTS
        // edge and by nothing else, and is not.
        let apex = derived("trace_a", &["trace_b"], "key-a");
        let input = derived("trace_b", &[], "key-b");
        let neighbour = derived("trace_c", &[], "key-c");
        let mut graph = seeded(&[apex.clone(), input.clone(), neighbour.clone()]);
        graph
            .add_edge(edge(EdgeRelation::Supports, &neighbour, &apex))
            .expect("both endpoints held");

        let closure = DerivationClosure::of(&graph, [&apex]);
        assert_eq!(subjects(&closure), ["trace_a@0", "trace_b@1"]);
        assert!(!closure.holds(&neighbour.identity()));
        // …and the edge really is there, so the negative is not vacuous.
        assert_eq!(graph.edges_to(&NodeRef::of(&apex)).count(), 1);
        assert!(closure.is_complete());
    }

    #[test]
    fn derivation_runs_backward_and_omits_a_consumer() {
        // P2: "made from" and "used by" are two questions, and only the first is answered.
        // `consumer` names `source`; the closure of the consumer reaches the source, and the
        // closure of the source does not reach the consumer.
        let source = derived("trace_src", &[], "key-s");
        let consumer = derived("trace_use", &["trace_src"], "key-u");
        let graph = seeded(&[source.clone(), consumer.clone()]);

        let backward = DerivationClosure::of(&graph, [&consumer]);
        assert_eq!(subjects(&backward), ["trace_src@1", "trace_use@0"]);

        let forward = DerivationClosure::of(&graph, [&source]);
        assert_eq!(subjects(&forward), ["trace_src@0"]);
        assert!(!forward.holds(&consumer.identity()));
        assert!(forward.is_complete());
    }

    #[test]
    fn an_input_naming_two_nodes_reaches_both() {
        // P3, W6's reason: where the graph holds several nodes about one artifact, choosing
        // between them is a semantic judgement this vocabulary may not make. Two claims
        // about one artifact is the ordinary way that happens.
        let apex = derived("trace_a", &["trace_b"], "claim-a");
        let first = derived("trace_b", &[], "claim-b1");
        let second = derived("trace_b", &[], "claim-b2");
        // Two distinct nodes, both about `trace_b`.
        assert_ne!(first.identity(), second.identity());
        let graph = seeded(&[apex.clone(), first.clone(), second.clone()]);

        let closure = DerivationClosure::of(&graph, [&apex]);
        assert_eq!(closure.reached_count(), 3);
        assert!(closure.holds(&first.identity()));
        assert!(closure.holds(&second.identity()));
        assert!(closure.is_complete());
    }

    #[test]
    fn a_diamond_reaches_its_apex_once_at_the_shorter_depth() {
        // P4, mandatory case. top ← {left, right} ← base, plus a second route top ← base,
        // so `base` is reachable at depth 2 and at depth 1 and must land at 1 — the least,
        // which is what makes a diamond one answer.
        let base = derived("trace_base", &[], "key-base");
        let left = derived("trace_left", &["trace_base"], "key-left");
        let right = derived("trace_right", &["trace_base"], "key-right");
        let top = derived("trace_top", &["trace_left", "trace_right"], "key-top");
        let graph = seeded(&[base.clone(), left.clone(), right.clone(), top.clone()]);

        let closure = DerivationClosure::of(&graph, [&top]);
        assert_eq!(
            subjects(&closure),
            [
                "trace_base@2",
                "trace_left@1",
                "trace_right@1",
                "trace_top@0"
            ]
        );
        assert_eq!(closure.reached_count(), 4);
        assert!(closure.is_complete());

        // The shorter of two routes wins: a direct input from the apex puts `base` at 1.
        let short = derived(
            "trace_top",
            &["trace_left", "trace_right", "trace_base"],
            "key-top",
        );
        let graph = seeded(&[base.clone(), left, right, short.clone()]);
        let closure = DerivationClosure::of(&graph, [&short]);
        assert_eq!(closure.row(&base.identity()).expect("reached").depth(), 1);
        assert_eq!(closure.depth(), 1);
    }

    #[test]
    fn a_chain_with_a_dangling_input_names_the_absence() {
        // P6, INV-016, mandatory case. head ← middle ← `trace_gone`, which the graph holds
        // no node about. The absence is named, at its depth, by the node that named it —
        // never dropped, never a refusal.
        let middle = derived("trace_mid", &["trace_gone"], "key-m");
        let head = derived("trace_head", &["trace_mid"], "key-h");
        let graph = seeded(&[middle.clone(), head.clone()]);

        let closure = DerivationClosure::of(&graph, [&head]);
        assert_eq!(subjects(&closure), ["trace_head@0", "trace_mid@1"]);
        assert_eq!(closure.absent_count(), 1);
        let absent = closure.absent().next().expect("one absence");
        assert_eq!(absent.artifact(), &artifact("trace_gone"));
        assert_eq!(absent.depth(), 2);
        assert_eq!(absent.named_by().collect::<Vec<_>>(), [&middle.identity()]);
        // INV-008: the answer says it is partial, and says which kind of partial.
        assert!(!closure.is_complete());
        assert_eq!(
            closure.incompleteness(),
            [Incompleteness::AbsentInputs { count: 1 }]
        );

        // Anti-vacuity: hold a node about `trace_gone` and the same walk is complete, with
        // the artifact among the reached rather than among the absent.
        let tail = derived("trace_gone", &[], "key-g");
        let graph = seeded(&[middle, head.clone(), tail.clone()]);
        let closure = DerivationClosure::of(&graph, [&head]);
        assert_eq!(closure.absent_count(), 0);
        assert!(closure.is_complete());
        assert_eq!(closure.row(&tail.identity()).expect("reached").depth(), 2);
    }

    #[test]
    fn one_absent_artifact_named_twice_is_one_absence_with_two_namers() {
        // P6: the absence is keyed by the artifact, because one missing artifact is one
        // missing artifact however many records reach for it — and every reacher is kept,
        // because dropping one would lose the credit docs/44 keeps.
        let left = derived("trace_left", &["trace_gone"], "key-l");
        let right = derived("trace_right", &["trace_gone"], "key-r");
        let top = derived("trace_top", &["trace_left", "trace_right"], "key-t");
        let graph = seeded(&[left.clone(), right.clone(), top.clone()]);

        let closure = DerivationClosure::of(&graph, [&top]);
        assert_eq!(closure.absent_count(), 1);
        let absent = closure.absent().next().expect("one absence");
        let namers: BTreeSet<EvidenceIdentity> = absent.named_by().cloned().collect();
        assert_eq!(
            namers,
            [left.identity(), right.identity()].into_iter().collect()
        );
        assert_eq!(absent.depth(), 2);
    }

    #[test]
    fn a_bound_of_zero_is_the_seeds_alone() {
        // P4 / clause 3: the bound reading a pre-rule caller could always express is still
        // expressible, and it says it truncated rather than reporting a complete answer.
        let base = derived("trace_base", &[], "key-base");
        let top = derived("trace_top", &["trace_base"], "key-top");
        let graph = seeded(&[base.clone(), top.clone()]);

        let closure = DerivationClosure::bounded(&graph, [&top], Some(0));
        assert_eq!(subjects(&closure), ["trace_top@0"]);
        assert_eq!(
            closure.incompleteness(),
            [Incompleteness::Bound { max_depth: 0 }]
        );
        assert_eq!(closure.max_depth(), Some(0));

        // One step is enough here, and then the bound stopped nothing.
        let closure = DerivationClosure::bounded(&graph, [&top], Some(1));
        assert_eq!(subjects(&closure), ["trace_base@1", "trace_top@0"]);
        assert!(closure.is_complete());
        // Unbounded agrees with a bound wider than the ancestry.
        assert_eq!(
            DerivationClosure::of(&graph, [&top]).reached_count(),
            closure.reached_count()
        );
    }

    #[test]
    fn a_bound_truncates_absences_too() {
        // The bound truncates everything past it: reporting an absence at `max_depth + 1`
        // would be answering past the bound the caller set. So the same graph reads
        // `Bound` at 0 and `AbsentInputs` at 1.
        let top = derived("trace_top", &["trace_gone"], "key-t");
        let graph = seeded(core::slice::from_ref(&top));

        let tight = DerivationClosure::bounded(&graph, [&top], Some(0));
        assert_eq!(tight.absent_count(), 0);
        assert_eq!(
            tight.incompleteness(),
            [Incompleteness::Bound { max_depth: 0 }]
        );

        let wide = DerivationClosure::bounded(&graph, [&top], Some(1));
        assert_eq!(wide.absent_count(), 1);
        assert_eq!(
            wide.incompleteness(),
            [Incompleteness::AbsentInputs { count: 1 }]
        );
    }

    #[test]
    fn the_two_reasons_a_closure_is_incomplete_are_distinct() {
        // INV-008: "never a bare boolean". One graph, one bound, and both reasons hold at
        // once — which a single flag could not tell apart.
        let deep = derived("trace_deep", &[], "key-d");
        let middle = derived("trace_mid", &["trace_deep"], "key-m");
        let top = derived("trace_top", &["trace_mid", "trace_gone"], "key-t");
        let graph = seeded(&[deep, middle, top.clone()]);

        let closure = DerivationClosure::bounded(&graph, [&top], Some(1));
        assert_eq!(
            closure.incompleteness(),
            [
                Incompleteness::Bound { max_depth: 1 },
                Incompleteness::AbsentInputs { count: 1 }
            ]
        );
        assert!(!closure.is_complete());
        assert_eq!(Incompleteness::Bound { max_depth: 1 }.as_str(), "bound");
        assert_eq!(
            Incompleteness::AbsentInputs { count: 1 }.as_str(),
            "absent-inputs"
        );
        assert_ne!(
            Incompleteness::Bound { max_depth: 1 }.to_string(),
            Incompleteness::AbsentInputs { count: 1 }.to_string()
        );

        // Anti-vacuity: unbounded leaves exactly one reason standing.
        let closure = DerivationClosure::of(&graph, [&top]);
        assert_eq!(
            closure.incompleteness(),
            [Incompleteness::AbsentInputs { count: 1 }]
        );
    }

    #[test]
    fn a_bounded_walk_that_reaches_a_known_node_is_not_truncated() {
        // The truncation flag is about lost information, not about the walk stopping: a
        // boundary node whose only input is already reached loses nothing.
        let base = derived("trace_base", &[], "key-base");
        // `left` is derived from `base`; `top` is derived from both, so at depth 1 the
        // boundary node `left` names `base` — already reached at depth 1.
        let left = derived("trace_left", &["trace_base"], "key-left");
        let top = derived("trace_top", &["trace_left", "trace_base"], "key-top");
        let graph = seeded(&[base, left, top.clone()]);

        let closure = DerivationClosure::bounded(&graph, [&top], Some(1));
        assert_eq!(closure.reached_count(), 3);
        assert!(closure.is_complete());
    }

    #[test]
    fn a_cycle_terminates_and_is_one_set() {
        // P7: an artifact handle is a name a producer supplies, so `a` derived from `b`
        // derived from `a` is constructible. Each node is expanded once, keyed by identity.
        let first = derived("trace_a", &["trace_b"], "key-a");
        let second = derived("trace_b", &["trace_a"], "key-b");
        let graph = seeded(&[first.clone(), second.clone()]);

        let from_first = DerivationClosure::of(&graph, [&first]);
        assert_eq!(subjects(&from_first), ["trace_a@0", "trace_b@1"]);
        assert!(from_first.is_complete());

        let from_second = DerivationClosure::of(&graph, [&second]);
        assert_eq!(subjects(&from_second), ["trace_a@1", "trace_b@0"]);
        // The same set either way; only the depths differ, because depth is from the seed.
        let members = |closure: &DerivationClosure| -> BTreeSet<EvidenceIdentity> {
            closure.reached().map(|(id, _)| id.clone()).collect()
        };
        assert_eq!(members(&from_first), members(&from_second));

        // A node naming its own artifact is a cycle of length one: reached once, at 0.
        let itself = derived("trace_self", &["trace_self"], "key-self");
        let graph = seeded(core::slice::from_ref(&itself));
        let closure = DerivationClosure::of(&graph, [&itself]);
        assert_eq!(closure.reached_count(), 1);
        assert_eq!(closure.row(&itself.identity()).expect("reached").depth(), 0);
        assert!(closure.is_complete());
    }

    #[test]
    fn a_seed_the_graph_does_not_hold_still_names_its_inputs() {
        // P5: the caller chooses the seeds and the graph resolves them. A seed the graph
        // does not hold is not a refusal — it is a node the caller handed over.
        let input = derived("trace_in", &[], "key-in");
        let stranger = derived("trace_out", &["trace_in"], "key-out");
        let graph = seeded(core::slice::from_ref(&input));
        assert!(graph.node(&stranger.identity()).is_none());

        let closure = DerivationClosure::of(&graph, [&stranger]);
        assert_eq!(subjects(&closure), ["trace_in@1", "trace_out@0"]);
        assert!(closure.is_complete());
    }

    #[test]
    fn a_closure_of_no_seeds_is_empty_and_complete() {
        // Boundary: nothing asked for is an empty answer, not an absent one.
        let graph = seeded(&[derived("trace_a", &["trace_b"], "key-a")]);
        let closure = DerivationClosure::of(&graph, []);
        assert_eq!(closure.reached_count(), 0);
        assert_eq!(closure.absent_count(), 0);
        assert_eq!(closure.depth(), 0);
        assert!(closure.is_complete());
        assert_eq!(closure.reached().len(), 0);

        // …and an empty graph closes over nothing at all.
        let closure = DerivationClosure::of_graph(&EvidenceGraph::new());
        assert!(closure.is_complete());
        assert_eq!(closure.reached_count(), 0);
    }

    #[test]
    fn a_whole_graph_closure_names_what_the_graph_depends_on_and_has_not_got() {
        // `of_graph` is the one scope that chooses nothing (P5): every held node is a seed,
        // so everything is at depth 0 and the absences are exactly the outside references.
        let graph = seeded(&[
            derived("trace_a", &["trace_b", "trace_gone"], "key-a"),
            derived("trace_b", &[], "key-b"),
        ]);
        let closure = DerivationClosure::of_graph(&graph);
        assert_eq!(subjects(&closure), ["trace_a@0", "trace_b@0"]);
        assert_eq!(closure.absent_count(), 1);
        assert_eq!(
            closure.absent().next().expect("one").artifact(),
            &artifact("trace_gone")
        );
        assert_eq!(closure.absent().next().expect("one").depth(), 1);
    }

    #[test]
    fn a_closure_reports_the_status_the_graph_holds() {
        // V5's discipline, unchanged: a row's status is the node's, and nothing here reads
        // the lattice or takes a promotion witness.
        use crate::authority::ServiceRole;
        use crate::authority::tests::service;

        for status in ClaimStatus::ALL {
            let base = derived("trace_base", &[], "key-base");
            let top = derived("trace_top", &["trace_base"], "key-top");
            let mut graph = seeded(&[base.clone(), top.clone()]);
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
                    .promote(&base.identity(), ClaimStatus::Proposed, &promotion)
                    .expect("an authorized promotion from the bottom");
            }
            let held = graph.node(&base.identity()).expect("held").status();
            // The promoted node keeps its identity, so the closure still names it.
            let closure = DerivationClosure::of(&graph, [&top]);
            let row = closure
                .reached()
                .find(|(_, row)| row.artifact() == &artifact("trace_base"))
                .expect("reached");
            assert_eq!(row.1.status(), held, "{status}");
        }
    }

    #[test]
    fn two_insertion_orders_close_identically() {
        // GOV-1-03, INV-005: the graph iterates in content order, so a closure over it is a
        // function of what is held and not of when it arrived — down to the encoded bytes.
        let members = [
            derived("trace_base", &["trace_gone"], "key-base"),
            derived("trace_left", &["trace_base"], "key-left"),
            derived("trace_right", &["trace_base"], "key-right"),
            derived("trace_top", &["trace_left", "trace_right"], "key-top"),
        ];
        let mut forward = EvidenceGraph::new();
        for member in &members {
            forward.add_node(member.clone());
        }
        let mut backward = EvidenceGraph::new();
        for member in members.iter().rev() {
            backward.add_node(member.clone());
        }
        let top = &members[3];
        let first = DerivationClosure::of(&forward, [top]);
        let second = DerivationClosure::of(&backward, [top]);
        assert_eq!(first, second);
        assert_eq!(first.reached_count(), 4);
        assert_eq!(first.absent_count(), 1);

        let naming = DefaultNaming::new();
        assert_eq!(
            first.to_record(&naming).encode(),
            second.to_record(&naming).encode()
        );
    }

    #[test]
    fn the_record_names_what_was_reached_what_was_not_and_why_it_stopped() {
        let base = derived("trace_base", &["trace_gone"], "key-base");
        let top = derived("trace_top", &["trace_base"], "key-top");
        let graph = seeded(&[base, top.clone()]);
        let closure = DerivationClosure::bounded(&graph, [&top], Some(4));
        let record = closure.to_record(&DefaultNaming::new());
        let Value::Record(fields) = &record else {
            panic!("a closure renders as a record");
        };
        let mut members: Vec<String> = fields.keys().map(|it| it.as_str().to_owned()).collect();
        members.sort();
        assert_eq!(members, ["absent", "incomplete", "max_depth", "reached"]);

        // The reasons render as their tokens, and the bound renders as a number.
        let incomplete = fields.get(&field("incomplete")).expect("a member");
        assert_eq!(
            incomplete,
            &Value::seq([Value::text("absent-inputs")]).expect("nests")
        );
        assert_eq!(
            fields.get(&field("max_depth")).expect("a member"),
            &Value::nat(4)
        );
        // …and an unbounded closure renders `max_depth` as an explicit absence rather than
        // dropping the member, the discipline `Provenance::epochs_value` uses.
        let unbounded = DerivationClosure::of(&graph, [&top]).to_record(&DefaultNaming::new());
        let Value::Record(fields) = &unbounded else {
            panic!("a closure renders as a record");
        };
        assert_eq!(
            fields.get(&field("max_depth")).expect("a member"),
            &Value::Null
        );
    }
}
