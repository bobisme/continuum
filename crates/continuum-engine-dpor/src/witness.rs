//! The reduction witness: the reducer's account of every class of interleavings it
//! declined to explore (RFC 0004; docs/33 "DPOR reducer | unreduced differential
//! oracle + reduction witness checker").
//!
//! This module is data only: the shared schema between the reducer ([`crate::engine`])
//! and the independent checker ([`crate::checker`]). It holds no decision logic, so
//! the two sides share the *shape* of a justification and nothing that decides one
//! (docs/33: "Shared schemas are allowed. Shared decision logic that causes both
//! sides to accept the same bug is not.").
//!
//! # What a witness says
//!
//! The search runs over **visits**: a visit is a stored state together with the
//! sleep set it was entered with. For every stored state (a [`NodeRecord`]) the
//! witness gives its **expansion**: either [`Expansion::Full`] or
//! [`Expansion::Reduced`] with a seed and a stubborn set `T`. For every visit (a
//! [`VisitRecord`]) it gives the entry sleep set, whether the stack proviso expanded
//! it in full, and its explored edges in order, each with the visit it reached and
//! whether the label was then **donated** to the sleep set of the edges after it.
//!
//! An enabled label with no edge from a visit is declined:
//!
//! - **by persistence** — the visit is reduced and the label is outside `T`; the claim
//!   is that `T` is closed under the conflict relation for its enabled members and
//!   under necessary enabling for its disabled ones and contains no enabled visible
//!   label, so `T ∩ enabled` is a persistent set that hides no visible step;
//! - **by sleep** — the label is in the visit's entry sleep set; the claim is that
//!   every edge into the visit justifies that set: each member was asleep in the
//!   source visit or donated there by an earlier edge, is independent of the edge's
//!   label, and commutes with it at the source state.
//!
//! Globally, the claims are that no donated edge lies inside a strongly connected
//! component of the visit graph (a label sleeps only once the region it leads to is
//! fully explored), and that every cycle of the visit graph passes through a fully
//! expanded visit (the cycle proviso).

use continuum_model_core::model::State;

/// Why a state is fully expanded. (A visit of a reduced state may still be expanded
/// in full by the stack proviso; [`VisitRecord::proviso`] records that.)
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FullReason {
    /// No label makes progress: the state is terminal, or its only enabled firings
    /// are self-loops.
    NoProgress,
    /// No proper stubborn set without an enabled visible label exists: every seed's
    /// closure either covers every enabled label or contains a visible one.
    Exhaustive,
}

impl FullReason {
    /// The reason's name.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NoProgress => "no-progress",
            Self::Exhaustive => "exhaustive",
        }
    }
}

/// How a state is expanded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Expansion {
    /// A persistent subset of the enabled labels.
    Reduced {
        /// The enabled label the closure started from.
        seed: usize,
        /// The stubborn set: ascending label indices, enabled and disabled.
        stubborn: Vec<usize>,
    },
    /// Every enabled label.
    Full(FullReason),
}

/// One stored state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NodeRecord {
    pub(crate) state: State,
    pub(crate) expansion: Expansion,
}

impl NodeRecord {
    /// The state.
    #[must_use]
    pub const fn state(&self) -> &State {
        &self.state
    }

    /// How it is expanded (a visit may still be expanded in full by the proviso).
    #[must_use]
    pub const fn expansion(&self) -> &Expansion {
        &self.expansion
    }
}

/// One explored edge of a visit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EdgeRecord {
    pub(crate) label: usize,
    pub(crate) target: usize,
    pub(crate) donated: bool,
}

impl EdgeRecord {
    /// The label fired.
    #[must_use]
    pub const fn label(&self) -> usize {
        self.label
    }

    /// The visit reached.
    #[must_use]
    pub const fn target(&self) -> usize {
        self.target
    }

    /// Whether the label then joined the sleep set of the visit's later edges.
    #[must_use]
    pub const fn donated(&self) -> bool {
        self.donated
    }
}

/// One visit: a stored state entered with a sleep set.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VisitRecord {
    pub(crate) node: usize,
    pub(crate) entry_sleep: Vec<usize>,
    pub(crate) proviso: bool,
    pub(crate) edges: Vec<EdgeRecord>,
}

impl VisitRecord {
    /// The stored state's node index.
    #[must_use]
    pub const fn node(&self) -> usize {
        self.node
    }

    /// The entry sleep set, ascending.
    #[must_use]
    pub fn entry_sleep(&self) -> &[usize] {
        &self.entry_sleep
    }

    /// Whether the stack proviso expanded this visit in full.
    #[must_use]
    pub const fn proviso(&self) -> bool {
        self.proviso
    }

    /// The explored edges, in exploration order.
    #[must_use]
    pub fn edges(&self) -> &[EdgeRecord] {
        &self.edges
    }
}

/// The whole witness of one reduced search.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReductionWitness {
    pub(crate) labels: Vec<(usize, usize)>,
    pub(crate) visible: u64,
    pub(crate) nodes: Vec<NodeRecord>,
    pub(crate) visits: Vec<VisitRecord>,
    pub(crate) roots: Vec<usize>,
    pub(crate) complete: bool,
}

impl ReductionWitness {
    /// The label table the witness speaks of, `(action, outcome)` per label.
    #[must_use]
    pub fn labels(&self) -> &[(usize, usize)] {
        &self.labels
    }

    /// The visible variables the reducer used, one bit per variable.
    #[must_use]
    pub const fn visible(&self) -> u64 {
        self.visible
    }

    /// The stored states, in discovery order.
    #[must_use]
    pub fn nodes(&self) -> &[NodeRecord] {
        &self.nodes
    }

    /// The visits, in creation order.
    #[must_use]
    pub fn visits(&self) -> &[VisitRecord] {
        &self.visits
    }

    /// For each initial state, in order, the visit the search started it from (entry
    /// sleep set empty).
    #[must_use]
    pub fn roots(&self) -> &[usize] {
        &self.roots
    }

    /// Whether the search that produced it was complete.
    #[must_use]
    pub const fn is_complete(&self) -> bool {
        self.complete
    }
}
