//! Deterministic breadth-first exploration of a [`Model`] (PR 8, IMPL-02).
//!
//! # What this module computes
//!
//! [`explore`] turns a declared transition system into its **reachable set**: every
//! state a finite sequence of transitions can reach from an enumerated initial state,
//! each carrying the length of the shortest such sequence and the labelled transition
//! that first reached it. That set, in canonical order, is the object docs/03 §6.1
//! calls a "canonical state table"; the depth column beside it and the
//! [`Discovery`] column beside that are what [`crate::witness`] reads.
//!
//! It consumes exactly the two primitives the model layer's seam table promised it
//! ([`Model::initial_states`] and [`Model::successors`],
//! `crates/continuum-engine-reference/src/model.rs:27-32`) and adds nothing to that
//! layer. Nothing here evaluates a guard, an update, or a predicate; a state's
//! successor row is whatever [`Model::successors`] says it is, in the order it says
//! it in.
//!
//! # The algorithm, and why each part of it is the deterministic choice
//!
//! An explicit FIFO queue of `(state, depth)`, a `BTreeMap` from state to depth as the
//! visited set, and one state expanded per iteration:
//!
//! - **The queue is explicit, not the call stack.** Recursion would make the maximum
//!   model size a function of the thread's stack size, which is an ambient property of
//!   the process (INV-005, `notes/plan/plan.md:323-325`) rather than a declared bound.
//!   The same argument [`crate::expr::IntExpr::depth`] makes for measuring an
//!   expression iteratively.
//! - **The queue carries each state's depth alongside it** rather than looking the
//!   depth up when the state is dequeued. A lookup has a "cannot happen" arm, and this
//!   crate's no-panic covenant makes every arm somebody's problem; carrying the depth
//!   deletes the arm instead of handling it.
//! - **The visited set is a `BTreeMap`, never a `HashMap`.** Iterating it yields the
//!   states in ascending state-vector order, which is the order the certificate wire
//!   form requires of a state table
//!   (`crates/continuum-kernel-core/src/wire.rs:89-92, 616-620`) — so [`Reachable`]
//!   comes out of the walk already canonical, with no sorting pass whose stability
//!   would then have to be argued. `GOV-1-03` scans for the hash-ordered spelling.
//! - **A state is expanded atomically.** Its whole successor row is computed, then the
//!   bounds are consulted, then every new target is admitted — or none of them is and
//!   the exploration stops with that state back at the head of the queue. Admitting
//!   part of a row would leave an explored set that is *not* a prefix of the canonical
//!   breadth-first walk, and a frontier that could not be resumed from.
//! - **New targets are admitted in ascending state order** (a `BTreeMap` keyed by the
//!   row's new targets), not in the row's own `(action, target)` order. Both are
//!   deterministic; ascending is the stronger choice, because it makes queue order —
//!   and therefore the frontier a bounded run reports — independent of how the actions
//!   happen to be *named*. Two models with the same transition relation and different
//!   action spellings explore identically.
//!
//! Layers are implicit and exact: a state's depth is written once, the first time it
//! is discovered, and breadth-first order makes that first write the shortest distance
//! (see [`Reachable::layer`]).
//!
//! # Where each state came from
//!
//! Beside the depth, and written at the same moment by the same rule, is a
//! [`Discovery`]: either the state is one of the model's own initial states, or it
//! names the state whose successor row first contained it and the action that did the
//! containing. A depth says a `d`-step path exists; a chain of `Discovery::Step`s *is*
//! that path, which is why [`crate::witness`] can hand back a labelled sequence
//! without a second search.
//!
//! Two determinism questions have to be answered for that chain to be a function of
//! the model alone, and both are answered by the order that was already there:
//!
//! - **Which predecessor?** The one whose expansion discovered the state, which is
//!   fixed by queue order and therefore by the canonical walk. A state at depth `d`
//!   may have many depth-`d-1` predecessors; the recorded one is the first to be
//!   dequeued among them.
//! - **Which action?** A successor row ascends by `(action index, target)`, so a
//!   target reached under several actions appears first under the least of them, and
//!   the least is what is recorded. The label is a function of the row, not of the
//!   order the row happens to be scanned in.
//!
//! Recording this changes nothing else. The set of new targets an expansion admits is
//! still a set keyed by [`State`], still iterated in ascending state order, and still
//! the same size — a `BTreeMap<State, usize>` replaces a `BTreeSet<State>` and the
//! keys, their order, and their count are unchanged, so the sequence of admissions,
//! the queue, the depths, the frontier, the counters and the bound checks are the
//! sequences and values they were before. `tests/bfs_origins.rs` asserts that against
//! an independent hand walk and against the frozen queue-ordered frontier, which is
//! the one observable an admission-order change would have to disturb.
//!
//! # Determinism (INV-005)
//!
//! There is no input to an exploration but the model and the [`Bounds`]. There is no
//! iteration order in it that is not a total order on the data: the queue is FIFO over
//! a sequence built from ascending sets, and the visited map is a `BTreeMap`. So two
//! explorations of one model under one bound are *equal*, not merely equivalent —
//! `tests/bfs_contract.rs` asserts byte-identical repeat runs, and the corroborating
//! walk in `tests/diehard_evidence.rs` (written before this module existed, with its
//! own `VecDeque`) reaches the same 16 states and the same depths.
//!
//! # Bounds are declared, and exhaustion is a result
//!
//! [`explore`] takes a [`Bounds`] because it has no default to fall back on that would
//! not be a cap nobody wrote down. Reaching one is not an error and not a smaller
//! answer: it is [`Exploration::Exhausted`], carrying the states explored so far, the
//! **frontier** — the discovered-but-unexpanded states, in queue order — and which
//! bound tripped. That is the engine-grain shape of the daemon's contract:
//!
//! > `BudgetExhausted` … task-budget spend was exhausted; carries `continuation` …
//! >
//! > — `notes/plan/rfcs/0026-continuumd-native-protocol.md:321`
//!
//! and of what a continuation holds — "frontier/search state"
//! (`notes/plan/docs/35_CONTINUUMD_WORKBENCH_DAEMON.md:112-118`). A campaign that ran
//! out of budget has not refuted anything (RFC 0026:360), and it has not *concluded*
//! anything either: only [`Exploration::Complete`] carries a closed set, and only a
//! closed set can be certified. [`Exploration::closed`] is the seam that says so in
//! the type rather than in prose.
//!
//! Silent truncation is what the shape exists to prevent (INV-009,
//! `notes/plan/docs/35_CONTINUUMD_WORKBENCH_DAEMON.md:130`).
//!
//! # No progress callback
//!
//! There is deliberately no observer, no callback, and no channel. A caller-supplied
//! progress hook is ambient effect by another name — it would let something outside
//! the model run during exploration, and it is exactly the construct [`crate::expr`]
//! refuses for guards, for the same reason. Progress is reported the way a pure
//! function reports it: the result carries [`Reachable::len`],
//! [`Reachable::expanded`], [`Reachable::transitions`] and [`Reachable::max_depth`],
//! and a partial result additionally carries the frontier. Streaming those counts as
//! task milestones is the daemon's job and PR 5+'s scope
//! (`notes/plan/docs/35_CONTINUUMD_WORKBENCH_DAEMON.md:99-110`; RFC 0026
//! `task.status`), not this crate's.
//!
//! # What is *not* here
//!
//! Whether an explored state violates an invariant, whether an empty successor row is
//! a defect or a legitimate terminal state, and how any of it is written to the wire
//! are other PR 8 bones. This module surfaces the raw material for each and encodes
//! none of the policy:
//!
//! | Sibling | What it reads here |
//! |---|---|
//! | invariant/deadlock checking (IMPL-03) | [`Reachable::states`], plus `Model::successors` being empty at a state — surfaced, never judged |
//! | shortest witness (IMPL-04) | [`Reachable::depth_of`] and [`Reachable::origin_of`]: a state at depth `d` has a `d`-step path, and the [`Discovery`] chain from it *is* that path. [`crate::witness`] walks it |
//! | finite closure certificate (IMPL-05) | [`Exploration::closed`] — the ascending [`Reachable::states`] table, and `Post(S) ⊆ S` guaranteed by the arm rather than re-checked |

use core::fmt;
use std::collections::{BTreeMap, VecDeque};

use crate::ident::Ident;
use crate::model::{EvaluationError, Model, State};

/// The largest reachable set an exploration will hold by default.
///
/// `MAX_STATES` from `crates/continuum-kernel-core/src/wire.rs:131` — "the largest
/// number of states a certificate's canonical table may carry". A reachable set beyond
/// it could be explored but never certified, which is the same reason
/// [`crate::model::MAX_INITIAL_STATES`] records the constant on the declaration side.
///
/// `continuum-kernel-temporal` declares its own, smaller `MAX_STATES` of `1 << 16`
/// (`crates/continuum-kernel-temporal/src/wire.rs:136`) and it is deliberately *not*
/// the bound here: the certificate this engine's fifth bone emits is a `kernel-core`
/// finite-closure certificate (docs/03 §6.1), and adopting the temporal bound would
/// refuse models the core checker can accept.
pub const MAX_STATES: usize = 1 << 20;

/// The deepest breadth-first layer an exploration will reach by default.
///
/// No certificate format carries a depth at all — docs/03 §6.1's contents are a state
/// table, initial-state IDs, action schemas, successor rows and invariant data — so
/// unlike [`MAX_STATES`] and [`MAX_TRANSITIONS`] this bound has no wire counterpart
/// and is the engine's own. It is set to [`MAX_STATES`] because that is already the
/// ceiling implied by the others: a reachable set of `n` states has breadth-first
/// depth at most `n - 1`, reached only when the set is a single path.
///
/// It is a separate knob rather than a derived one because bounding *search* without
/// bounding *size* is a question someone actually asks — "is there a violation within
/// `k` steps?" is the shortest-witness bone's (IMPL-04) natural query, and it wants a
/// depth bound and the full state budget.
pub const MAX_DEPTH: usize = 1 << 20;

/// The largest number of labelled transitions an exploration will count by default.
///
/// `MAX_TRANSITIONS` from `crates/continuum-kernel-core/src/wire.rs:139` — "the
/// largest number of transitions a finite-closure certificate may carry in total,
/// across every successor row", which is exactly what [`Reachable::transitions`]
/// counts. The kernel accumulates the same quantity in the same type while decoding
/// (`crates/continuum-kernel-core/src/wire.rs:770-780`), so the two agree on overflow
/// as well as on the limit.
pub const MAX_TRANSITIONS: u64 = 1 << 22;

// ---------------------------------------------------------------------------
// bounds
// ---------------------------------------------------------------------------

/// What one exploration may spend, declared before it starts.
///
/// There is no [`Default`] and [`explore`] takes this by value, so an exploration
/// cannot begin without someone naming its limits. That is the mechanical half of the
/// no-silent-caps rule: a default budget is a truncation policy that nobody reviewed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Bounds {
    states: usize,
    depth: usize,
    transitions: u64,
}

impl Bounds {
    /// The largest exploration whose result a `kernel-core` finite-closure certificate
    /// could carry: [`MAX_STATES`], [`MAX_DEPTH`], [`MAX_TRANSITIONS`].
    ///
    /// The name describes the *bound*, not the outcome. An exploration that completes
    /// within it has a result no wire limit rejects; whether a certificate for that
    /// result exists, and whether an independent checker accepts it, is the fifth
    /// bone's claim and not this constant's.
    pub const CERTIFIABLE: Self = Self {
        states: MAX_STATES,
        depth: MAX_DEPTH,
        transitions: MAX_TRANSITIONS,
    };

    /// Bounds with every limit named explicitly.
    #[must_use]
    pub const fn new(states: usize, depth: usize, transitions: u64) -> Self {
        Self {
            states,
            depth,
            transitions,
        }
    }

    /// The same bounds with a different state limit.
    #[must_use]
    pub const fn with_states(self, states: usize) -> Self {
        Self {
            states,
            depth: self.depth,
            transitions: self.transitions,
        }
    }

    /// The same bounds with a different depth limit.
    #[must_use]
    pub const fn with_depth(self, depth: usize) -> Self {
        Self {
            states: self.states,
            depth,
            transitions: self.transitions,
        }
    }

    /// The same bounds with a different transition limit.
    #[must_use]
    pub const fn with_transitions(self, transitions: u64) -> Self {
        Self {
            states: self.states,
            depth: self.depth,
            transitions,
        }
    }

    /// The most states that may be *discovered*, initial states included.
    #[must_use]
    pub const fn states(self) -> usize {
        self.states
    }

    /// The deepest layer that may be discovered. Depth 0 is the initial states, so a
    /// bound of `0` admits them and refuses every successor of a new state.
    #[must_use]
    pub const fn depth(self) -> usize {
        self.depth
    }

    /// The most labelled transitions that may be counted, across every expanded row.
    #[must_use]
    pub const fn transitions(self) -> u64 {
        self.transitions
    }
}

/// Which declared limit stopped an exploration.
///
/// Distinct members rather than one `Exhausted` flag, for INV-008's reason: "timeout,
/// unsupported semantics, insufficient telemetry, abstraction ambiguity, and
/// incomplete proof search are distinct outcomes" (`notes/plan/plan.md:335-337`). A
/// caller that hit the state bound raises the state bound; one that hit the depth
/// bound learns its property may live deeper, which is a different next move.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Bound {
    /// [`Bounds::states`] — the next expansion would discover more states than
    /// declared.
    States,
    /// [`Bounds::depth`] — the next expansion would discover a state in a layer
    /// deeper than declared.
    Depth,
    /// [`Bounds::transitions`] — the next expansion's successor row would take the
    /// running total past the declared limit.
    Transitions,
}

impl fmt::Display for Bound {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match *self {
            Self::States => "state",
            Self::Depth => "depth",
            Self::Transitions => "transition",
        })
    }
}

// ---------------------------------------------------------------------------
// the reachable set
// ---------------------------------------------------------------------------

/// How one state was **first** reached.
///
/// Written once, at the moment the state is discovered, by the same rule that writes
/// its depth — so a `Discovery` is a fact about the canonical breadth-first walk, not
/// a preference among the paths that happen to exist. Following the chain from any
/// discovered state reaches an initial state in exactly [`Reachable::depth_of`] hops,
/// and the labelled sequence that comes back is a shortest path; [`crate::witness`]
/// is that walk with the endpoint-selection policy attached.
///
/// The predecessor is carried by value rather than as an index into
/// [`Reachable::states`]. An index would be smaller and is what a wire form would
/// write, but resolving one during the walk means a lookup with an arm that cannot
/// happen, and this crate's no-panic covenant makes every arm somebody's problem. A
/// reference engine spends the memory and keeps the arm deleted; the index form is a
/// projection the certificate bone can compute from the ascending table it already
/// emits.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Discovery {
    /// The state is one of [`Model::initial_states`]. It was not reached — it is where
    /// reaching starts, and it is the only [`Discovery`] a depth-0 state can have.
    Initial,
    /// The state was first seen in the successor row of `predecessor`, under the
    /// action at index `action`.
    Step {
        /// The state whose expansion discovered this one. Its depth is one less.
        predecessor: State,
        /// The index into [`Model::actions`] of the action that reached it — the
        /// **least** such index, because a successor row ascends by
        /// `(action index, target)` and a target reached under several actions
        /// appears first under the smallest of them.
        action: usize,
    },
}

/// The states an exploration discovered, in canonical order, with their depths and
/// their discoveries.
///
/// `states` is strictly ascending, which is the certificate wire form's rule for a
/// state table (`crates/continuum-kernel-core/src/wire.rs:89-92`), and the other two
/// columns are parallel to it: `depths[i]` is the length of a shortest path from an
/// initial state to `states[i]`, and `origins[i]` is the labelled transition that
/// first reached it ([`Discovery`]). Emitting a state table from this is therefore a
/// copy, not a sort.
///
/// **Discovered is not expanded.** Every state here was reached; only
/// [`Reachable::expanded`] of them had a successor row computed. For an
/// [`Exploration::Complete`] the two are equal and the set is closed under the
/// transition relation. For an [`Exploration::Exhausted`] they are not, and the
/// difference is exactly the frontier.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reachable {
    states: Vec<State>,
    depths: Vec<usize>,
    origins: Vec<Discovery>,
    expanded: usize,
    transitions: u64,
}

impl Reachable {
    /// The discovered states, strictly ascending.
    #[must_use]
    pub fn states(&self) -> &[State] {
        &self.states
    }

    /// Each state's shortest distance from an initial state, parallel to
    /// [`Reachable::states`].
    #[must_use]
    pub fn depths(&self) -> &[usize] {
        &self.depths
    }

    /// How many states were discovered.
    #[must_use]
    pub fn len(&self) -> usize {
        self.states.len()
    }

    /// Whether nothing was discovered.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.states.is_empty()
    }

    /// Whether a state was discovered.
    #[must_use]
    pub fn contains(&self, state: &State) -> bool {
        self.states.binary_search(state).is_ok()
    }

    /// A state's shortest distance from an initial state, or `None` when it was not
    /// discovered.
    ///
    /// A binary search, which is sound here for the reason it is *not* sound in
    /// [`crate::expr::Environment`]: this slice is ascending by construction rather
    /// than by a precondition someone has to remember.
    #[must_use]
    pub fn depth_of(&self, state: &State) -> Option<usize> {
        let index = self.states.binary_search(state).ok()?;
        self.depths.get(index).copied()
    }

    /// How each discovered state was first reached, parallel to
    /// [`Reachable::states`].
    #[must_use]
    pub fn origins(&self) -> &[Discovery] {
        &self.origins
    }

    /// How one state was first reached, or `None` when it was not discovered.
    ///
    /// The same binary search [`Reachable::depth_of`] makes, sound for the same
    /// reason: the slice is ascending by construction rather than by a precondition
    /// someone has to remember.
    #[must_use]
    pub fn origin_of(&self, state: &State) -> Option<&Discovery> {
        let index = self.states.binary_search(state).ok()?;
        self.origins.get(index)
    }

    /// The states at one breadth-first layer, ascending.
    #[must_use]
    pub fn layer(&self, depth: usize) -> Vec<&State> {
        self.states
            .iter()
            .zip(self.depths.iter())
            .filter_map(|(state, at)| (*at == depth).then_some(state))
            .collect()
    }

    /// The deepest layer discovered, or `None` when nothing was.
    #[must_use]
    pub fn max_depth(&self) -> Option<usize> {
        self.depths.iter().copied().max()
    }

    /// How many discovered states had their successor row computed.
    #[must_use]
    pub const fn expanded(&self) -> usize {
        self.expanded
    }

    /// The labelled transitions leaving the expanded states, counted the way the wire
    /// form counts them: one per `(action, target)` pair in a successor row, so two
    /// actions reaching one target are two transitions
    /// (`crates/continuum-kernel-core/src/wire.rs:786-794`).
    #[must_use]
    pub const fn transitions(&self) -> u64 {
        self.transitions
    }
}

// ---------------------------------------------------------------------------
// results
// ---------------------------------------------------------------------------

/// An exploration stopped by a declared bound: what it saw, and where it stopped.
///
/// This is a *result*, not a failure. It is the engine-grain analogue of a suspended
/// task's committed partial evidence plus its continuation
/// (`notes/plan/docs/35_CONTINUUMD_WORKBENCH_DAEMON.md:112-118`), with the one part an
/// engine can supply: the frontier.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Partial {
    explored: Reachable,
    frontier: Vec<State>,
    tripped: Bound,
    bounds: Bounds,
}

impl Partial {
    /// What was discovered before the bound tripped. Not closed: some state here has
    /// a successor that was never computed.
    #[must_use]
    pub const fn explored(&self) -> &Reachable {
        &self.explored
    }

    /// The discovered-but-unexpanded states, **in queue order** — the state whose
    /// expansion tripped the bound first, then the rest of the queue behind it.
    ///
    /// Queue order, not ascending order, and the difference is the whole point: this
    /// sequence is a resume point, and sorting it would discard the breadth-first
    /// position that makes a resumed walk agree with an unbounded one. It is still
    /// deterministic — it is a function of the model and the bounds alone — it is just
    /// not sorted.
    ///
    /// Every state here is also in [`Partial::explored`], with its depth.
    #[must_use]
    pub fn frontier(&self) -> &[State] {
        &self.frontier
    }

    /// Which bound stopped the exploration.
    ///
    /// One bound, even when raising it would immediately trip another: the checks run
    /// in a fixed order — transitions, then depth, then states — so the answer is a
    /// function of the model and the bounds rather than of anything else.
    #[must_use]
    pub const fn tripped(&self) -> Bound {
        self.tripped
    }

    /// The bounds the exploration was given.
    #[must_use]
    pub const fn bounds(&self) -> Bounds {
        self.bounds
    }
}

/// What an exploration produced.
///
/// Two arms, because "the reachable set" and "part of the reachable set" are different
/// claims and only one of them can be certified. A caller that wants the states either
/// way asks for [`Exploration::reachable`]; a caller that needs closure asks for
/// [`Exploration::closed`] and gets `None` when there is none.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Exploration {
    /// The queue emptied: every discovered state was expanded, and every successor of
    /// every discovered state is a discovered state. This is `Post(S) ⊆ S` — the
    /// closure conjunct of docs/03 §6.1 — holding by construction rather than by a
    /// later check.
    Complete(Reachable),
    /// A declared bound was reached first.
    Exhausted(Partial),
}

impl Exploration {
    /// The states discovered, whether or not the exploration finished.
    ///
    /// Useful precisely because a violation found in a partial set is a real
    /// violation: the states here were genuinely reached. The converse does not hold,
    /// and that asymmetry is the checking bone's to encode — *not finding* a violation
    /// in a partial set is `Inconclusive`/`ResourceExhausted` (INV-008; RFC 0026's
    /// `InconclusiveReason`), never a pass.
    #[must_use]
    pub const fn reachable(&self) -> &Reachable {
        match self {
            Self::Complete(reachable) => reachable,
            Self::Exhausted(partial) => &partial.explored,
        }
    }

    /// The reachable set when it is closed, and `None` when the exploration stopped
    /// early.
    ///
    /// The certificate bone's entry point: a finite-closure certificate built from a
    /// truncated set would fail the checker's closure step
    /// (docs/03 §6.1, "verify successor closure"), and this signature is why it cannot
    /// be built by accident.
    #[must_use]
    pub const fn closed(&self) -> Option<&Reachable> {
        match self {
            Self::Complete(reachable) => Some(reachable),
            Self::Exhausted(_) => None,
        }
    }

    /// The partial result when a bound tripped, and `None` when the exploration
    /// finished.
    #[must_use]
    pub const fn exhausted(&self) -> Option<&Partial> {
        match self {
            Self::Complete(_) => None,
            Self::Exhausted(partial) => Some(partial),
        }
    }

    /// Whether the exploration finished within its bounds.
    #[must_use]
    pub const fn is_complete(&self) -> bool {
        matches!(*self, Self::Complete(_))
    }
}

/// Why an exploration produced no result at all.
///
/// Distinct from [`Exploration::Exhausted`], which is a result. Both arms here say the
/// same thing in different words: the exploration was asked for something that is not
/// a question about this model.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExplorationError {
    /// A state's successor row could not be computed.
    ///
    /// The model's transition relation is undefined at a state the model itself can
    /// reach — docs/16 PO-MOD-003 failing
    /// (`notes/plan/docs/16_PROOF_OBLIGATIONS.md:17`) — which is a defect in the
    /// declaration, not a budget question, and resuming would fail identically because
    /// the model is deterministic.
    ///
    /// The states explored before the failure are deliberately not carried: they are a
    /// prefix of a walk over a relation that does not exist everywhere, and they are
    /// re-derivable exactly, by exploring under a bound that stops short of `state` —
    /// which returns an ordinary [`Exploration::Exhausted`]. Nothing is lost by
    /// dropping evidence a bound can reproduce; something *would* be lost by reporting
    /// a partial reachable set for a model that has none.
    Evaluation {
        /// The state whose row failed. `model.successors(&state)` reproduces it.
        state: State,
        /// That state's breadth-first depth, so the failure can be located in the
        /// walk and not only in the model.
        depth: usize,
        /// The action being evaluated, when one can be named. `None` only when the
        /// failure was not attributable to an action — a malformed state rather than
        /// a malformed transition.
        action: Option<Ident>,
        /// What the model layer reported.
        ///
        /// Boxed, and the reason is the success path rather than this one: every
        /// caller of [`explore`] carries the `Result`, and a well-formed model never
        /// produces this variant. Widening the returned value by the size of a defect
        /// report charges every correct exploration for a failure it does not have.
        source: Box<EvaluationError>,
    },
    /// The state bound cannot hold the model's own initial states.
    ///
    /// Not a partial result, because there is no prefix of the walk to report: the
    /// initial states are not something exploration *discovers*, they are what it
    /// starts from ([`Model::initial_states`] is never empty). A budget that cannot
    /// hold them describes no exploration of this model.
    InitialStatesExceedBound {
        /// How many initial states the model enumerates.
        initial: usize,
        /// The declared [`Bounds::states`].
        limit: usize,
    },
}

impl fmt::Display for ExplorationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Evaluation {
                state,
                depth,
                action,
                source,
            } => match action {
                Some(action) => write!(
                    f,
                    "action `{action}` at state {state} (depth {depth}): {source}"
                ),
                None => write!(f, "state {state} (depth {depth}): {source}"),
            },
            Self::InitialStatesExceedBound { initial, limit } => write!(
                f,
                "the model enumerates {initial} initial states; the state bound is {limit}"
            ),
        }
    }
}

impl core::error::Error for ExplorationError {
    fn source(&self) -> Option<&(dyn core::error::Error + 'static)> {
        match self {
            Self::Evaluation { source, .. } => Some(&**source),
            Self::InitialStatesExceedBound { .. } => None,
        }
    }
}

// ---------------------------------------------------------------------------
// the walk
// ---------------------------------------------------------------------------

/// Explore `model` breadth-first within `bounds`.
///
/// Deterministic: the returned value is a function of `model` and `bounds` and of
/// nothing else.
///
/// # Errors
///
/// [`ExplorationError::InitialStatesExceedBound`] when the state bound cannot hold the
/// model's initial states, and [`ExplorationError::Evaluation`] when a reachable
/// state's successor row cannot be computed. Reaching a bound is *not* an error; it is
/// [`Exploration::Exhausted`].
pub fn explore(model: &Model, bounds: Bounds) -> Result<Exploration, ExplorationError> {
    let initial = model.initial_states();
    if initial.len() > bounds.states {
        return Err(ExplorationError::InitialStatesExceedBound {
            initial: initial.len(),
            limit: bounds.states,
        });
    }

    let mut visited: BTreeMap<State, Visit> = BTreeMap::new();
    let mut queue: VecDeque<(State, usize)> = VecDeque::new();
    for state in initial {
        // `Model::initial_states` is strictly ascending and therefore duplicate free
        // (`crates/continuum-kernel-core/src/wire.rs:89-92`). Queueing only on a fresh
        // insert says so locally, so this loop does not silently double-queue if that
        // ever stops being true.
        let visit = Visit {
            depth: 0,
            origin: Discovery::Initial,
        };
        if visited.insert(state.clone(), visit).is_none() {
            queue.push_back((state.clone(), 0));
        }
    }

    let mut expanded: usize = 0;
    let mut transitions: u64 = 0;

    while let Some((here, depth)) = queue.pop_front() {
        let row = match model.successors(&here) {
            Ok(row) => row,
            Err(source) => {
                return Err(ExplorationError::Evaluation {
                    action: culprit(model, &here),
                    state: here,
                    depth,
                    source: Box::new(source),
                });
            }
        };

        // Ascending and deduplicated: the row is ordered by `(action, target)`, so one
        // target can appear under several actions and the targets are not themselves
        // ordered. The value is the action that reached the target *first* in that
        // order — the least action index — which is the label the state's `Discovery`
        // carries. Keying by `State` keeps this the same set, in the same order, with
        // the same length as the `BTreeSet` it replaced.
        let mut discovered: BTreeMap<State, usize> = BTreeMap::new();
        for step in &row {
            if !visited.contains_key(step.target()) && !discovered.contains_key(step.target()) {
                discovered.insert(step.target().clone(), step.action());
            }
        }

        let commit = match admit(
            bounds,
            Spend {
                transitions,
                visited: visited.len(),
                row: row.len(),
                discovered: discovered.len(),
                depth,
            },
        ) {
            Ok(commit) => commit,
            Err(tripped) => {
                // The expansion is abandoned whole, so `here` goes back where it was
                // and the explored set stays a prefix of the canonical walk.
                queue.push_front((here, depth));
                return Ok(Exploration::Exhausted(Partial {
                    explored: collect(visited, expanded, transitions),
                    frontier: queue.into_iter().map(|(state, _)| state).collect(),
                    tripped,
                    bounds,
                }));
            }
        };

        for (target, action) in discovered {
            let visit = Visit {
                depth: commit.next,
                origin: Discovery::Step {
                    predecessor: here.clone(),
                    action,
                },
            };
            visited.insert(target.clone(), visit);
            queue.push_back((target, commit.next));
        }
        transitions = commit.transitions;
        // Bounded by `visited.len()`, which is bounded by `bounds.states`, so this
        // cannot saturate; saturating rather than checked keeps a dead error arm out
        // of a function whose every arm has to mean something.
        expanded = expanded.saturating_add(1);
    }

    Ok(Exploration::Complete(collect(
        visited,
        expanded,
        transitions,
    )))
}

/// What the visited map records about one discovered state.
///
/// The two columns are written together because they are decided together: the first
/// expansion to reach a state fixes both its depth and where it came from.
#[derive(Debug, Clone)]
struct Visit {
    depth: usize,
    origin: Discovery,
}

/// What expanding one state would cost.
#[derive(Debug, Clone, Copy)]
struct Spend {
    transitions: u64,
    visited: usize,
    row: usize,
    discovered: usize,
    depth: usize,
}

/// What expanding one state would leave behind.
#[derive(Debug, Clone, Copy)]
struct Commit {
    transitions: u64,
    next: usize,
}

/// Whether one expansion fits, and if not, which bound it broke.
///
/// A pure function of the counts, checked in a fixed order — transitions, depth,
/// states — so an expansion that breaks two bounds at once always names the same one.
/// The depth and state checks are skipped when the row discovers nothing new: an
/// expansion that adds no state cannot deepen the walk or grow the set, and stopping
/// on it would report a bound that was never actually reached.
fn admit(bounds: Bounds, spend: Spend) -> Result<Commit, Bound> {
    let width = u64::try_from(spend.row).map_err(|_| Bound::Transitions)?;
    let transitions = spend
        .transitions
        .checked_add(width)
        .ok_or(Bound::Transitions)?;
    if transitions > bounds.transitions {
        return Err(Bound::Transitions);
    }
    let mut next = spend.depth;
    if spend.discovered > 0 {
        next = spend.depth.checked_add(1).ok_or(Bound::Depth)?;
        if next > bounds.depth {
            return Err(Bound::Depth);
        }
        let total = spend
            .visited
            .checked_add(spend.discovered)
            .ok_or(Bound::States)?;
        if total > bounds.states {
            return Err(Bound::States);
        }
    }
    Ok(Commit { transitions, next })
}

/// The visited map as a canonical state table with parallel depth and discovery
/// columns.
///
/// `BTreeMap` iteration is ascending by key, and [`State`]'s `Ord` is lexicographic on
/// the state vector (`crates/continuum-engine-reference/src/model.rs:116-117`), so the
/// table comes out in wire order with no sorting pass.
fn collect(visited: BTreeMap<State, Visit>, expanded: usize, transitions: u64) -> Reachable {
    let mut states: Vec<State> = Vec::with_capacity(visited.len());
    let mut depths: Vec<usize> = Vec::with_capacity(visited.len());
    let mut origins: Vec<Discovery> = Vec::with_capacity(visited.len());
    for (state, visit) in visited {
        states.push(state);
        depths.push(visit.depth);
        origins.push(visit.origin);
    }
    Reachable {
        states,
        depths,
        origins,
        expanded,
        transitions,
    }
}

/// Which action [`Model::successors`] was evaluating when it failed.
///
/// `Model::successors` checks the state and then scans actions in ascending index
/// order, returning the first failure
/// (`crates/continuum-engine-reference/src/model.rs:520-529`), so replaying that scan
/// with [`Model::action_successors`] reaches the same action. Evaluation is pure, so
/// the replay is free of consequences, and it runs only on the error path — the walk
/// itself pays nothing for it.
///
/// `None` means no single action failed, which is the case when the failure came from
/// the state-validity check ahead of the scan rather than from a transition.
fn culprit(model: &Model, state: &State) -> Option<Ident> {
    for (index, action) in model.actions().iter().enumerate() {
        if model.action_successors(index, state).is_err() {
            return Some(action.name().clone());
        }
    }
    None
}
