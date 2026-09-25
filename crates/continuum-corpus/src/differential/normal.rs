//! Normalized semantics: the one shape every engine's answer is projected onto.
//!
//! Engines differ in what they compute and how they spell it. The harness compares
//! only what the model's semantics fixes, so an answer is reduced to:
//!
//! - a **verdict per invariant** — [`InvariantVerdict`]: holds, violated (with a
//!   counterexample when the engine offers one), not established (a checker that
//!   refused the claim but offers no path), undefined (an undefined read, RFC 0003,
//!   which is not a verdict), or typed inconclusive;
//! - a **model-level undefined action read** — [`UndefinedRead`], which invalidates
//!   every verdict over the model;
//! - a **reachable-state projection** — [`Projection`]: the exact reachable set and
//!   its deadlocked (no enabled action) states, or only their number, or the checker's
//!   refusal of the producer's set, or typed inconclusive.
//!
//! Inconclusiveness is typed with `continuum-value`'s INV-008 reasons
//! ([`InconclusiveReason`]), never a bare flag. Nothing here is engine-specific:
//! search order, discovery depth, the particular counterexample path, and certificate
//! bytes are not normalized fields, because two correct engines may differ in them. A
//! counterexample is instead checked by replay against the model ([`super::replay`]).
//!
//! Decision records: RFC 0010 (the `established | refuted | inconclusive` result shape)
//! and RFC 0026 (`InconclusiveReason`).

use std::collections::{BTreeMap, BTreeSet};

use continuum_value::assurance::InconclusiveReason;

/// Why an answer is undecided, typed (INV-008), with a detail for the report.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Inconclusive {
    /// The INV-008 reason.
    pub reason: InconclusiveReason,
    /// What the engine said, for the report. Not compared.
    pub detail: String,
}

impl Inconclusive {
    /// An inconclusive answer for `reason`.
    #[must_use]
    pub fn new(reason: InconclusiveReason, detail: impl Into<String>) -> Self {
        Self {
            reason,
            detail: detail.into(),
        }
    }
}

/// A counterexample path: a start state and labelled steps, as plain values.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Trace {
    /// The start state's values, in the model's variable order.
    pub start: Vec<i64>,
    /// Each step: the action's name and the state it reached.
    pub steps: Vec<(String, Vec<i64>)>,
}

impl Trace {
    /// The last state of the path.
    #[must_use]
    pub fn end(&self) -> &[i64] {
        self.steps
            .last()
            .map_or(self.start.as_slice(), |(_, state)| state.as_slice())
    }
}

/// Whose read is undefined (RFC 0003, "Definedness"; `continuum_model_core::definedness`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum UndefinedKind {
    /// An action's read: a member of an action's definedness chain is false. It
    /// invalidates every verdict over the model.
    Action,
    /// A declared predicate's read: a member of `I#defined`'s chain is false, or the
    /// predicate is itself a definedness predicate and is false.
    Invariant,
}

impl UndefinedKind {
    /// `action` or `invariant`.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Action => "action",
            Self::Invariant => "invariant",
        }
    }
}

/// The typed outcome "undefined read in `X`": never a verdict (RFC 0003), an error of
/// the model at a state it reaches (RFC 0013). Compared across engines by kind and
/// checked against the model at the reported state; the subject and state are the
/// engine's choice of *which* undefined read to show, so two correct engines may pick
/// different ones, and each is validated rather than required to be equal.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct UndefinedRead {
    /// Action or invariant.
    pub kind: UndefinedKind,
    /// `X`: the base of the false definedness predicate's chain.
    pub subject: String,
    /// The state the read is undefined at, as values, when the engine names one.
    pub state: Option<Vec<i64>>,
    /// A path from an initial state to [`Self::state`], when the engine offers one: the
    /// proof that the state is reached, replayed by the harness.
    pub path: Option<Trace>,
}

/// What an engine established about one invariant.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InvariantVerdict {
    /// Every reachable state satisfies it.
    Holds,
    /// A reachable state falsifies it. The witness, when offered, is replayed.
    Violated {
        /// The counterexample, or `None` when the engine offers no path.
        witness: Option<Trace>,
    },
    /// A checker refused to establish it and offers no path (a certificate the kernel
    /// rejected). Agrees with [`Self::Violated`], disagrees with [`Self::Holds`].
    NotEstablished {
        /// The checker's reason, for the report.
        detail: String,
    },
    /// The invariant has no verdict: a read in it, or an action's read, is undefined at
    /// a reached state (RFC 0003). Not a verdict, not undecided: its own category.
    Undefined(UndefinedRead),
    /// Undecided.
    Inconclusive(Inconclusive),
}

/// What an engine established about the reachable states.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Projection {
    /// The exact reachable set and its deadlocked states, as value vectors.
    Exact {
        /// Every reachable state.
        states: BTreeSet<Vec<i64>>,
        /// The reachable states with no enabled action.
        deadlocks: BTreeSet<Vec<i64>>,
    },
    /// Only the number of reachable states.
    Cardinality {
        /// How many.
        states: usize,
    },
    /// A checker rejected the producer's closed set.
    CheckerRejected {
        /// The checker's reason.
        detail: String,
    },
    /// The engine does not report reachable states.
    NotProvided,
    /// Undecided.
    Inconclusive(Inconclusive),
}

/// One engine's whole answer about one model.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Normalized {
    /// The reachable-state projection.
    pub projection: Projection,
    /// A verdict per invariant, by predicate name. An absent name is an invariant the
    /// engine does not judge.
    pub invariants: BTreeMap<String, InvariantVerdict>,
    /// An undefined action read at a reached state, which invalidates the model; `None`
    /// when the engine found none. Reported by every engine that judges invariants.
    pub undefined_action: Option<UndefinedRead>,
}

impl Normalized {
    /// An answer that is inconclusive everywhere, for `predicates`.
    #[must_use]
    pub fn inconclusive<'a>(
        why: &Inconclusive,
        predicates: impl IntoIterator<Item = &'a str>,
    ) -> Self {
        Self {
            projection: Projection::Inconclusive(why.clone()),
            invariants: predicates
                .into_iter()
                .map(|name| (name.to_owned(), InvariantVerdict::Inconclusive(why.clone())))
                .collect(),
            undefined_action: None,
        }
    }
}
