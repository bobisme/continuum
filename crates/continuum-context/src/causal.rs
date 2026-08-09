//! The compiler's causal input, and the closure checker that licenses `CausallyClosed`
//! (RFC 0028, "Compiler pipeline" stage 2; "Validation").
//!
//! # Scope: bn-21vno — the compiler's stage group 1 (stages 1–2)
//!
//! > | 2 | backward causal slicing | CIR causal order | `CausallyClosed`, and the
//! > precondition of `ReplayPreserving` |
//! >
//! > — RFC 0028, "Compiler pipeline"
//!
//! # Why the order is declared here and not imported
//!
//! Stage 2's declared input is "CIR causal order". `continuum-cir` is a documented stub in
//! this workspace, and `continuum-context` may not import an engine
//! (`tools/check_crate_boundaries.py`; this crate's dependency-boundary contract in
//! [`crate`]'s own documentation). So the compiler takes its causal order as a **value it is
//! handed**, not as an engine it calls: [`CausalOrder`] is a finite, immediate-predecessor
//! graph over named candidates, and any producer — a CIR, an explicit-state engine's step
//! relation, a replayed trace's dynamic dependence — projects into it.
//!
//! That is not a weakening. RFC 0028's Validation section is explicit that "the checkers are
//! independent of the compiler that made the claim", and the same separation applies to the
//! input: a compiler that computed its own causal order and then checked closure against
//! that order would be checking its own arithmetic. Here the order is the *premise*, stated
//! before any slicing, and the closure check reads it and nothing else.
//!
//! # Completeness is declared, because non-membership only proves something in a complete
//! order
//!
//! RFC 0028 draws a sharp line in the manifest:
//!
//! > `slice-irrelevant` is reserved for items *provably* outside the property-directed
//! > slice; an item dropped because the compiler could not decide is `heuristic-cutoff`,
//! > never `slice-irrelevant`.
//! >
//! > — RFC 0028, "Omission manifest"
//!
//! Whether a stage-2 drop is a proof turns entirely on whether the order it was decided
//! against is complete. In a [`Completeness::Complete`] order — one whose producer declares
//! it names every causal edge among the candidates — "not an ancestor of any root" is a
//! decision by set membership over the order's own transitive closure, and the drop is
//! `slice-irrelevant`. In a [`Completeness::Partial`] order the same computation proves
//! nothing, because an unnamed edge could have made the candidate an ancestor, and the drop
//! is `heuristic-cutoff`. [`Completeness::omission_reason`] makes the choice a consequence of
//! the input's own declaration rather than a judgement a compiler author makes per call.
//!
//! # Clause → test
//!
//! | Clause | Source | Test |
//! |---|---|---|
//! | an edge endpoint must be a node | this module's premise | `an_edge_to_nowhere_is_refused` |
//! | a causal order is acyclic | RFC 0001; "downward-closed under the causal order" | `a_cycle_is_not_a_causal_order` |
//! | closure is decided by the order's own edges | RFC 0028 "Validation" | `a_selection_missing_a_predecessor_is_not_closed` |
//! | the checker is independent of the slicer | RFC 0028 "Validation" | `the_checker_rejects_hand_built_selections_the_slicer_cannot_produce` |
//! | a complete order proves irrelevance; a partial one does not | RFC 0028, "Omission manifest" | `completeness_decides_the_omission_reason` |
//! | ancestry and topological order are deterministic | `rule ordering.deterministic` | `ancestors_and_topological_order_are_canonical` |

use core::fmt;
use std::collections::{BTreeMap, BTreeSet};

use continuum_value::value::Name;

use crate::omission::OmissionReason;
use crate::selection::SelectionKind;

/// Whether the causal order names every causal edge among its candidates.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum Completeness {
    /// The producer declares the edge set exhaustive: an edge absent from the order is an
    /// edge absent from the execution. Non-ancestry is then a proof.
    Complete,
    /// The producer declares the edge set partial, and names why. Non-ancestry proves
    /// nothing, so stage 2's drops are `heuristic-cutoff`.
    Partial {
        /// Why the order is incomplete — recorded so a reader of the manifest can tell an
        /// undecidable drop from a decided one without re-deriving it.
        reason: PartialReason,
    },
}

impl Completeness {
    /// The manifest reason a stage-2 drop is recorded under, given this declaration.
    ///
    /// See the module documentation: the choice is a consequence of the input, not a
    /// judgement made per call.
    #[must_use]
    pub const fn omission_reason(&self) -> OmissionReason {
        match self {
            Self::Complete => OmissionReason::SliceIrrelevant,
            Self::Partial { .. } => OmissionReason::HeuristicCutoff,
        }
    }

    /// Whether non-ancestry under this order is a proof of irrelevance.
    #[must_use]
    pub const fn is_complete(&self) -> bool {
        matches!(self, Self::Complete)
    }
}

/// Why a causal order is only partial.
///
/// A closed set: an untyped free-text reason here would be the place an undecidable drop
/// could be dressed up as a decided one (INV-008's discipline, applied to the input).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PartialReason {
    /// The producing engine does not compute the causal order for this semantics.
    UnsupportedSemantics,
    /// The order was truncated by a bound the producer declared.
    Truncated,
    /// The telemetry the order is derived from is incomplete (INV-008's
    /// `InsufficientTelemetry`, at the input).
    InsufficientTelemetry,
}

impl PartialReason {
    /// All three members, in declaration order.
    pub const ALL: [Self; 3] = [
        Self::UnsupportedSemantics,
        Self::Truncated,
        Self::InsufficientTelemetry,
    ];

    /// A stable token for the auditable intermediate's rendering.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::UnsupportedSemantics => "unsupported-semantics",
            Self::Truncated => "truncated",
            Self::InsufficientTelemetry => "insufficient-telemetry",
        }
    }
}

impl fmt::Display for PartialReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A finite causal order over named, kinded candidates: the premise stage 2 slices against.
///
/// Edges are **immediate** causal predecessors. Ancestry is the transitive closure, computed
/// here rather than stored, so the two cannot disagree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CausalOrder {
    nodes: BTreeMap<Name, SelectionKind>,
    predecessors: BTreeMap<Name, BTreeSet<Name>>,
    completeness: Completeness,
}

impl CausalOrder {
    /// Build a complete causal order: `nodes` and their immediate-predecessor edges.
    ///
    /// Each edge is `(event, predecessor)` — the second happened before the first.
    ///
    /// # Errors
    ///
    /// [`CausalError::RepeatedNode`] for one identity declared twice;
    /// [`CausalError::UnknownEndpoint`] for an edge naming something that is not a node — an
    /// order over phantom events cannot be a premise for anything;
    /// [`CausalError::SelfEdge`] for an event that precedes itself; and
    /// [`CausalError::Cyclic`] for an edge set that is not a strict partial order, since
    /// "downward-closed under the causal order" is not a statement about a cyclic relation.
    pub fn new(
        nodes: impl IntoIterator<Item = (Name, SelectionKind)>,
        edges: impl IntoIterator<Item = (Name, Name)>,
    ) -> Result<Self, CausalError> {
        Self::declared(nodes, edges, Completeness::Complete)
    }

    /// Build a causal order the producer declares partial, naming why.
    ///
    /// # Errors
    ///
    /// As [`CausalOrder::new`].
    pub fn partial(
        nodes: impl IntoIterator<Item = (Name, SelectionKind)>,
        edges: impl IntoIterator<Item = (Name, Name)>,
        reason: PartialReason,
    ) -> Result<Self, CausalError> {
        Self::declared(nodes, edges, Completeness::Partial { reason })
    }

    fn declared(
        nodes: impl IntoIterator<Item = (Name, SelectionKind)>,
        edges: impl IntoIterator<Item = (Name, Name)>,
        completeness: Completeness,
    ) -> Result<Self, CausalError> {
        let mut members: BTreeMap<Name, SelectionKind> = BTreeMap::new();
        for (id, kind) in nodes {
            if members.insert(id.clone(), kind).is_some() {
                return Err(CausalError::RepeatedNode { id });
            }
        }
        let mut predecessors: BTreeMap<Name, BTreeSet<Name>> = members
            .keys()
            .map(|id| (id.clone(), BTreeSet::new()))
            .collect();
        for (event, predecessor) in edges {
            if !members.contains_key(&event) {
                return Err(CausalError::UnknownEndpoint { id: event });
            }
            if !members.contains_key(&predecessor) {
                return Err(CausalError::UnknownEndpoint { id: predecessor });
            }
            if event == predecessor {
                return Err(CausalError::SelfEdge { id: event });
            }
            predecessors
                .get_mut(&event)
                .expect("every node has an entry")
                .insert(predecessor);
        }
        let order = Self {
            nodes: members,
            predecessors,
            completeness,
        };
        order.acyclic()?;
        Ok(order)
    }

    /// Kahn's algorithm over the `BTree`-ordered node set: a deterministic topological order
    /// when one exists, and the proof of acyclicity when it does not.
    fn acyclic(&self) -> Result<Vec<Name>, CausalError> {
        let mut remaining: BTreeMap<Name, usize> = self
            .predecessors
            .iter()
            .map(|(id, preds)| (id.clone(), preds.len()))
            .collect();
        let mut ordered: Vec<Name> = Vec::with_capacity(remaining.len());
        loop {
            let ready: Option<Name> = remaining
                .iter()
                .find(|(_, count)| **count == 0)
                .map(|(id, _)| id.clone());
            let Some(id) = ready else { break };
            remaining.remove(&id);
            for (other, count) in &mut remaining {
                if self
                    .predecessors
                    .get(other)
                    .is_some_and(|preds| preds.contains(&id))
                {
                    *count -= 1;
                }
            }
            ordered.push(id);
        }
        if !remaining.is_empty() {
            let mut inside: Vec<Name> = remaining.into_keys().collect();
            inside.sort();
            return Err(CausalError::Cyclic {
                first: inside.swap_remove(0),
            });
        }
        Ok(ordered)
    }

    /// How many candidates the order holds.
    #[must_use]
    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    /// Whether the order is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    /// The completeness the producer declared.
    #[must_use]
    pub const fn completeness(&self) -> &Completeness {
        &self.completeness
    }

    /// Whether `id` is a node of this order.
    #[must_use]
    pub fn contains(&self, id: &Name) -> bool {
        self.nodes.contains_key(id)
    }

    /// The kind of one node.
    #[must_use]
    pub fn kind(&self, id: &Name) -> Option<SelectionKind> {
        self.nodes.get(id).copied()
    }

    /// Every node, with its kind, in canonical [`Name`] order.
    pub fn nodes(&self) -> impl Iterator<Item = (&Name, SelectionKind)> {
        self.nodes.iter().map(|(id, kind)| (id, *kind))
    }

    /// The immediate causal predecessors of one node, in canonical order.
    #[must_use]
    pub fn immediate_predecessors(&self, id: &Name) -> Option<&BTreeSet<Name>> {
        self.predecessors.get(id)
    }

    /// A deterministic topological order: every node after all of its predecessors.
    ///
    /// Ties are broken by [`Name`] order, so two processes produce one sequence.
    #[must_use]
    pub fn topological_order(&self) -> Vec<Name> {
        self.acyclic()
            .expect("acyclicity was established by the constructor")
    }

    /// The backward closure of `roots`: the roots plus every causal ancestor of any of them.
    ///
    /// This is stage 2's computation, stated as a function of the order alone.
    ///
    /// # Errors
    ///
    /// [`CausalError::UnknownEndpoint`] for a root that is not a node — stage 1's output is
    /// checked against the order it will be sliced over, so a root from nowhere is caught
    /// before it becomes a wrong pack.
    pub fn backward_closure(&self, roots: &BTreeSet<Name>) -> Result<BTreeSet<Name>, CausalError> {
        for root in roots {
            if !self.contains(root) {
                return Err(CausalError::UnknownEndpoint { id: root.clone() });
            }
        }
        let mut closed: BTreeSet<Name> = BTreeSet::new();
        let mut frontier: Vec<Name> = roots.iter().cloned().collect();
        while let Some(id) = frontier.pop() {
            if !closed.insert(id.clone()) {
                continue;
            }
            if let Some(preds) = self.predecessors.get(&id) {
                frontier.extend(preds.iter().cloned());
            }
        }
        Ok(closed)
    }

    /// The **independent** closure check: the first way `selection` fails to be
    /// downward-closed under this order, or `None` when it is closed.
    ///
    /// This function reads the order's edges and the selection it is handed. It knows
    /// nothing about how the selection was produced, which is what makes it a checker rather
    /// than an assertion by the slicer — RFC 0028, "Validation": "the checkers are
    /// independent of the compiler that made the claim — INV-004's discipline applied here".
    ///
    /// The first violation is reported in canonical order, so the diagnosis is deterministic.
    #[must_use]
    pub fn closure_violation(&self, selection: &BTreeSet<Name>) -> Option<ClosureViolation> {
        for selected in selection {
            let Some(preds) = self.predecessors.get(selected) else {
                return Some(ClosureViolation::NotInOrder {
                    selected: selected.clone(),
                });
            };
            for predecessor in preds {
                if !selection.contains(predecessor) {
                    return Some(ClosureViolation::MissingPredecessor {
                        selected: selected.clone(),
                        predecessor: predecessor.clone(),
                    });
                }
            }
        }
        None
    }

    /// Whether `selection` is downward-closed under this order.
    #[must_use]
    pub fn is_downward_closed(&self, selection: &BTreeSet<Name>) -> bool {
        self.closure_violation(selection).is_none()
    }
}

/// The first way a selection is not downward-closed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClosureViolation {
    /// A selected item is not a node of the order at all, so closure is not even a statement
    /// about it.
    NotInOrder {
        /// The selected item.
        selected: Name,
    },
    /// A selected item's immediate causal predecessor is not selected.
    MissingPredecessor {
        /// The selected item.
        selected: Name,
        /// The predecessor it brings in, and that the selection lacks.
        predecessor: Name,
    },
}

impl fmt::Display for ClosureViolation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotInOrder { selected } => write!(
                f,
                "`{selected}` is selected but is not a node of the causal order"
            ),
            Self::MissingPredecessor {
                selected,
                predecessor,
            } => write!(
                f,
                "`{selected}` is selected and its causal predecessor `{predecessor}` is not; \
                 an unselected causal predecessor breaks `CausallyClosed`"
            ),
        }
    }
}

/// A way a causal order fails to be one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CausalError {
    /// One identity is declared as a node twice.
    RepeatedNode {
        /// The repeated identity.
        id: Name,
    },
    /// An edge, or a root, names something that is not a node.
    UnknownEndpoint {
        /// The identity that is not in the order.
        id: Name,
    },
    /// An event declared as its own causal predecessor.
    SelfEdge {
        /// The identity on both ends.
        id: Name,
    },
    /// The edge set is not a strict partial order.
    Cyclic {
        /// The first member of a cycle, in canonical order.
        first: Name,
    },
}

impl fmt::Display for CausalError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RepeatedNode { id } => write!(f, "`{id}` is declared as a node twice"),
            Self::UnknownEndpoint { id } => {
                write!(f, "`{id}` is not a node of this causal order")
            }
            Self::SelfEdge { id } => write!(f, "`{id}` cannot causally precede itself"),
            Self::Cyclic { first } => write!(
                f,
                "the edge set has a cycle through `{first}`, so it is not a causal order"
            ),
        }
    }
}

impl core::error::Error for CausalError {}

#[cfg(test)]
mod tests {
    use super::*;

    fn name(text: &str) -> Name {
        Name::new(text).expect("well formed")
    }

    fn set(ids: [&str; 0]) -> BTreeSet<Name> {
        ids.iter().map(|id| name(id)).collect()
    }

    fn selection(ids: &[&str]) -> BTreeSet<Name> {
        ids.iter().map(|id| name(id)).collect()
    }

    /// A chain `e_1 -> e_2 -> e_3` with a concurrent `n_1`, and a `s_1` source with no
    /// causal edges at all.
    fn order() -> CausalOrder {
        CausalOrder::new(
            [
                (name("e_1"), SelectionKind::Event),
                (name("e_2"), SelectionKind::Event),
                (name("e_3"), SelectionKind::Event),
                (name("n_1"), SelectionKind::Event),
                (name("s_1"), SelectionKind::Source),
            ],
            [
                (name("e_2"), name("e_1")),
                (name("e_3"), name("e_2")),
                (name("n_1"), name("e_1")),
            ],
        )
        .expect("a well-formed order")
    }

    #[test]
    fn an_edge_to_nowhere_is_refused() {
        assert_eq!(
            CausalOrder::new(
                [(name("e_1"), SelectionKind::Event)],
                [(name("e_1"), name("e_0"))],
            ),
            Err(CausalError::UnknownEndpoint { id: name("e_0") })
        );
        assert_eq!(
            CausalOrder::new(
                [(name("e_1"), SelectionKind::Event)],
                [(name("e_9"), name("e_1"))],
            ),
            Err(CausalError::UnknownEndpoint { id: name("e_9") })
        );
    }

    #[test]
    fn a_repeated_node_is_refused() {
        assert_eq!(
            CausalOrder::new(
                [
                    (name("e_1"), SelectionKind::Event),
                    (name("e_1"), SelectionKind::Source),
                ],
                [],
            ),
            Err(CausalError::RepeatedNode { id: name("e_1") })
        );
    }

    #[test]
    fn an_event_cannot_precede_itself() {
        assert_eq!(
            CausalOrder::new(
                [(name("e_1"), SelectionKind::Event)],
                [(name("e_1"), name("e_1"))],
            ),
            Err(CausalError::SelfEdge { id: name("e_1") })
        );
    }

    #[test]
    fn a_cycle_is_not_a_causal_order() {
        assert_eq!(
            CausalOrder::new(
                [
                    (name("e_1"), SelectionKind::Event),
                    (name("e_2"), SelectionKind::Event),
                    (name("e_3"), SelectionKind::Event),
                ],
                [
                    (name("e_2"), name("e_1")),
                    (name("e_3"), name("e_2")),
                    (name("e_1"), name("e_3")),
                ],
            ),
            Err(CausalError::Cyclic { first: name("e_1") })
        );
    }

    #[test]
    fn an_empty_order_is_a_causal_order() {
        let empty = CausalOrder::new([], []).expect("nothing to order");
        assert!(empty.is_empty());
        assert!(empty.topological_order().is_empty());
        assert!(empty.is_downward_closed(&set([])));
        assert_eq!(
            empty.backward_closure(&set([])).expect("no roots"),
            BTreeSet::new()
        );
    }

    #[test]
    fn ancestors_and_topological_order_are_canonical() {
        let built = order();
        // Kahn's algorithm taking the least ready node each round, under `Name`'s shortlex
        // order: one sequence, in every process.
        assert_eq!(
            built.topological_order(),
            [
                name("e_1"),
                name("e_2"),
                name("e_3"),
                name("n_1"),
                name("s_1"),
            ]
        );
        assert_eq!(
            built
                .backward_closure(&selection(&["e_3"]))
                .expect("a node"),
            selection(&["e_1", "e_2", "e_3"])
        );
        // Two identical builds agree, and so do two closures of one root set.
        assert_eq!(built.topological_order(), order().topological_order());
    }

    #[test]
    fn a_root_from_nowhere_is_refused() {
        assert_eq!(
            order().backward_closure(&selection(&["e_9"])),
            Err(CausalError::UnknownEndpoint { id: name("e_9") })
        );
    }

    #[test]
    fn a_backward_closure_is_downward_closed() {
        let order = order();
        for roots in [
            selection(&["e_3"]),
            selection(&["n_1"]),
            selection(&["e_3", "n_1"]),
            selection(&["s_1"]),
            selection(&["e_1"]),
        ] {
            let closed = order.backward_closure(&roots).expect("nodes");
            assert!(
                order.is_downward_closed(&closed),
                "the closure of {roots:?} is not closed"
            );
        }
    }

    #[test]
    fn a_selection_missing_a_predecessor_is_not_closed() {
        let order = order();
        assert_eq!(
            order.closure_violation(&selection(&["e_2", "e_3"])),
            Some(ClosureViolation::MissingPredecessor {
                selected: name("e_2"),
                predecessor: name("e_1"),
            })
        );
    }

    /// The independence claim, made falsifiable. Each of these selections is hand-built —
    /// [`CausalOrder::backward_closure`] cannot produce any of them — and the checker
    /// decides each one from the order's edges alone. A checker that trusted the slicer
    /// would pass all four.
    #[test]
    fn the_checker_rejects_hand_built_selections_the_slicer_cannot_produce() {
        let order = order();

        // A core with its middle removed.
        assert!(matches!(
            order.closure_violation(&selection(&["e_1", "e_3"])),
            Some(ClosureViolation::MissingPredecessor { .. })
        ));
        // A "textually relevant" set: everything whose name starts with `e_`, minus the
        // root cause. RFC 0028 C2's own example of what is not accepted.
        assert!(matches!(
            order.closure_violation(&selection(&["e_2", "e_3", "n_1"])),
            Some(ClosureViolation::MissingPredecessor { .. })
        ));
        // An item that is not in the order at all.
        assert_eq!(
            order.closure_violation(&selection(&["e_1", "e_2", "x_1"])),
            Some(ClosureViolation::NotInOrder {
                selected: name("x_1"),
            })
        );
        // And the honest one still passes, so the checker is not merely a rejector.
        assert_eq!(order.closure_violation(&selection(&["e_1", "e_2"])), None);
        assert_eq!(order.closure_violation(&set([])), None);
    }

    #[test]
    fn a_selection_of_causally_isolated_items_is_closed() {
        // Boundary: `s_1` has no predecessors, so `{s_1}` is trivially closed. Closure is
        // not a claim that anything was sliced.
        let order = order();
        assert!(order.is_downward_closed(&selection(&["s_1"])));
    }

    #[test]
    fn completeness_decides_the_omission_reason() {
        let complete = order();
        assert!(complete.completeness().is_complete());
        assert_eq!(
            complete.completeness().omission_reason(),
            OmissionReason::SliceIrrelevant
        );

        for reason in PartialReason::ALL {
            let partial = CausalOrder::partial([(name("e_1"), SelectionKind::Event)], [], reason)
                .expect("a well-formed order");
            assert!(!partial.completeness().is_complete());
            assert_eq!(
                partial.completeness().omission_reason(),
                OmissionReason::HeuristicCutoff,
                "a drop decided against a {reason} order is not a proof of irrelevance"
            );
        }
    }

    #[test]
    fn the_partial_reasons_have_distinct_tokens() {
        let tokens: BTreeSet<&str> = PartialReason::ALL
            .into_iter()
            .map(PartialReason::as_str)
            .collect();
        assert_eq!(tokens.len(), PartialReason::ALL.len());
    }
}
