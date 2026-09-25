//! Mechanism preservation: the validator every reduction's result goes through (START_HERE
//! PR 18, bn-5kmuf).
//!
//! # Why a verdict is not enough
//!
//! A reduction keeps a candidate when its replay still shows the failure. That is RFC
//! 0028's `ReplayPreserving`: "the selected core replays to the same verdict". A core can
//! keep the verdict and lose the reason for it. A replay that checks the substrate's
//! semantics and not the program's accepts an acknowledgement with no write behind it; a
//! deletion can reach the same violated property through another pair of operations.
//! research/26 asks for "small Context Packs without omitting mechanism", and PR 18's exit
//! asks that the core "does not delete the actual causal mechanism". This module makes
//! that a checked property of every core, not an audit of some of them.
//!
//! # The definition
//!
//! A **mechanism** ([`Mechanism`]) is a finite set of **steps**, each a label from the
//! instantiation's step alphabet (the register's durable-register steps, for example
//! `Submit(n=a,epoch=0,value=v0)`), with two kinds of constraint:
//!
//! - **edges** ([`Edge`]): step `from` comes before step `to`, either in the replay's
//!   order ([`Order::Precedes`]) or in its happens-before order
//!   ([`Order::HappensBefore`]). The order is RFC 0001's causal order, as the replay's
//!   own [`Story`] declares it; it is not recomputed here;
//! - **guards** ([`Guard`]): no step whose label is in `forbidden` occurs strictly
//!   between step `after` and step `before`, up to the first step whose label is in
//!   `until`, if one occurs there. A guard is how a mechanism states an absence, such as
//!   "no `Sync` of that replica before the acknowledgement, until its bytes are lost"
//!   (after the loss, a `Sync` is another write's). Every guard spans an edge, so its
//!   interval is defined.
//!
//! The instantiation **derives** the mechanism from the original failure: from its
//! witnesses (the events the violated property observes, [`crate::reduce::Replayed`]),
//! the operations in their causal past that the violation rests on, and the order between
//! them. An edge is `HappensBefore` exactly when the original run orders the two steps
//! by happens-before; otherwise it is `Precedes`. So a mechanism is a function of the
//! original run, and the same derivation gives the same mechanism.
//!
//! A replay **preserves** the mechanism when it is a run, it shows the failure, and its
//! story has an **embedding** of the mechanism: an injective map from steps to story
//! positions that keeps each label and satisfies every edge and every guard
//! ([`embed`]). The embedding is up to the choice among equally labelled positions, and
//! nothing else: the same operations, in the same causal order, with the same
//! absences. Labels match only when equal, so an instantiation binds each label to what
//! identifies the operation's state: the register's labels carry their epoch, and a
//! derived mechanism names only operations of the acknowledgement's epoch. A reduction that renames the configuration (RFC 0028's `ValueMinimal`
//! renames values; bn-25z9o) renames the mechanism the same way before the embedding
//! ([`Validator::rename`]).
//!
//! # Every reduction goes through it
//!
//! [`Validated`] has no public constructor: only this module's check makes one. Every
//! public entry point of [`crate::reduce`] and [`crate::scenario`] takes a
//! [`Validator`], validates the input before any pass and the output of every pass that
//! changed its input, and returns its result as a [`Validated`] core or configuration.
//! A result whose check rejects it is returned as a typed rejection ([`Rejection`]),
//! never as a core; a check that cannot decide is an INV-008 inconclusive with its reason
//! ([`Checked::Undecided`]), never a pass. The check is charged before its work, from the
//! budget's validation allowance ([`crate::reduce::Budget::with_validation`]), which is
//! apart from the passes' replays, so the passes' trajectories do not depend on it.
//!
//! # The trust boundary
//!
//! The check is only as good as the validator's replay and derivation. The engine
//! enforces that no core leaves unchecked, that the mechanism is not empty and is
//! well formed, that the story's order is a causal order, and the embedding itself. A
//! validator that returns a story the program did not produce, or derives a mechanism
//! that does not describe the failure, defeats it, as a lying oracle defeats the
//! reduction. Two more limits of the check: a comparison of two labels is charged one
//! unit, so an instantiation with long labels must bound or intern them (the register's
//! are short, fixed-form strings); and a [`Validated`] value is not bound to the
//! validator that made it, so a caller that moves one between results defeats the
//! record, not the check. The register instantiation's validator replays through the program, apart
//! from the reduction's oracle, and checks the failure with the campaign's own checker
//! (`continuum-asupersync`'s `tests/support/pr18_program.rs`).

use std::collections::BTreeSet;

use continuum_value::assurance::InconclusiveReason;

use crate::reduce::{CausalOrder, Spent};

/// The most steps a mechanism may have.
pub const MAX_STEPS: usize = 64;
/// The most edges, and the most guards, a mechanism may have.
pub const MAX_CONSTRAINTS: usize = 1024;
/// The most forbidden labels, and the most ending labels, one guard may name.
pub const MAX_FORBIDDEN: usize = 4096;

/// How an edge orders its two steps.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Order {
    /// `from`'s position in the replay is before `to`'s.
    Precedes,
    /// `from` happens before `to` in the replay's causal order.
    HappensBefore,
}

/// Step `from` comes before step `to`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Edge {
    /// The earlier step.
    pub from: usize,
    /// The later step.
    pub to: usize,
    /// In which order.
    pub order: Order,
}

/// No step labelled with a member of `forbidden` occurs strictly between step `after`
/// and step `before`, before the first step there labelled with a member of `until`.
/// At each position `forbidden` is read first, so a label in both breaks the guard.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Guard<L> {
    /// The step the interval starts after.
    pub after: usize,
    /// The step the interval ends before.
    pub before: usize,
    /// The labels that may not occur in the interval.
    pub forbidden: Vec<L>,
    /// The labels that end the interval early.
    pub until: Vec<L>,
}

/// Why steps, edges and guards are not a mechanism.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MechanismError {
    /// No step: an empty mechanism embeds in every replay and says nothing.
    Empty,
    /// More than [`MAX_STEPS`] steps, [`MAX_CONSTRAINTS`] edges or guards, or
    /// [`MAX_FORBIDDEN`] forbidden or ending labels in one guard.
    TooLarge,
    /// An edge or guard names a step that does not exist, or an edge joins a step to
    /// itself.
    BadStep {
        /// The edge's or guard's index, edges first then guards.
        constraint: usize,
    },
    /// The edges have a cycle: no replay can satisfy them.
    Cycle,
    /// A guard spans no edge, so its interval is undefined.
    GuardWithoutEdge {
        /// The guard's index.
        guard: usize,
    },
}

/// A failure's causal mechanism. See the module documentation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mechanism<L> {
    steps: Vec<L>,
    edges: Vec<Edge>,
    guards: Vec<Guard<L>>,
}

impl<L: Ord + Clone> Mechanism<L> {
    /// The mechanism with these steps, edges and guards. A guard's `forbidden` and
    /// `until` are sets: each is kept sorted and without duplicates, so two mechanisms
    /// that differ only in how a guard lists its labels are equal.
    ///
    /// # Errors
    ///
    /// [`MechanismError`] when they are not a mechanism.
    pub fn new(
        steps: Vec<L>,
        edges: Vec<Edge>,
        mut guards: Vec<Guard<L>>,
    ) -> Result<Self, MechanismError> {
        if steps.is_empty() {
            return Err(MechanismError::Empty);
        }
        if steps.len() > MAX_STEPS
            || edges.len() > MAX_CONSTRAINTS
            || guards.len() > MAX_CONSTRAINTS
            || guards
                .iter()
                .any(|g| g.forbidden.len() > MAX_FORBIDDEN || g.until.len() > MAX_FORBIDDEN)
        {
            return Err(MechanismError::TooLarge);
        }
        let n = steps.len();
        for (k, e) in edges.iter().enumerate() {
            if e.from >= n || e.to >= n || e.from == e.to {
                return Err(MechanismError::BadStep { constraint: k });
            }
        }
        for (k, g) in guards.iter().enumerate() {
            if g.after >= n || g.before >= n || g.after == g.before {
                return Err(MechanismError::BadStep {
                    constraint: edges.len() + k,
                });
            }
            if !edges.iter().any(|e| e.from == g.after && e.to == g.before) {
                return Err(MechanismError::GuardWithoutEdge { guard: k });
            }
        }
        // Kahn's algorithm: every step leaves the queue exactly when the edges are acyclic.
        let mut indegree = vec![0_usize; n];
        let mut out: Vec<Vec<usize>> = vec![Vec::new(); n];
        for e in &edges {
            indegree[e.to] += 1;
            out[e.from].push(e.to);
        }
        let mut queue: Vec<usize> = (0..n).filter(|&s| indegree[s] == 0).collect();
        let mut seen = 0;
        while let Some(s) = queue.pop() {
            seen += 1;
            for &t in &out[s] {
                indegree[t] -= 1;
                if indegree[t] == 0 {
                    queue.push(t);
                }
            }
        }
        if seen != n {
            return Err(MechanismError::Cycle);
        }
        for g in &mut guards {
            g.forbidden.sort();
            g.forbidden.dedup();
            g.until.sort();
            g.until.dedup();
        }
        Ok(Self {
            steps,
            edges,
            guards,
        })
    }

    /// Its steps' labels.
    #[must_use]
    pub fn steps(&self) -> &[L] {
        &self.steps
    }

    /// Its edges.
    #[must_use]
    pub fn edges(&self) -> &[Edge] {
        &self.edges
    }

    /// Its guards.
    #[must_use]
    pub fn guards(&self) -> &[Guard<L>] {
        &self.guards
    }

    /// Every label it names, steps and the guards' labels: what renaming it costs.
    #[must_use]
    pub fn labels(&self) -> usize {
        self.steps.len()
            + self
                .guards
                .iter()
                .map(|g| g.forbidden.len() + g.until.len())
                .sum::<usize>()
    }

    /// The same mechanism with every label, the guards' included, renamed by `f`.
    /// The constraints are unchanged, so the result is a mechanism again; the guards'
    /// sets are sorted again.
    #[must_use]
    pub fn renamed(&self, mut f: impl FnMut(&L) -> L) -> Self {
        let set = |labels: &[L], f: &mut dyn FnMut(&L) -> L| {
            let mut out: Vec<L> = labels.iter().map(f).collect();
            out.sort();
            out.dedup();
            out
        };
        Self {
            steps: self.steps.iter().map(&mut f).collect(),
            edges: self.edges.clone(),
            guards: self
                .guards
                .iter()
                .map(|g| Guard {
                    after: g.after,
                    before: g.before,
                    forbidden: set(&g.forbidden, &mut f),
                    until: set(&g.until, &mut f),
                })
                .collect(),
        }
    }
}

/// What a replay shows, as the mechanism is read from it: its steps in the replay's
/// order, each with its label, and the causal order over them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Story<L> {
    labels: Vec<L>,
    order: CausalOrder,
}

impl<L> Story<L> {
    /// The story whose step `i` has label `labels[i]` and immediate happens-before
    /// predecessors `preds[i]`, each earlier than `i`.
    ///
    /// # Errors
    ///
    /// The lengths differ, or `preds` is not a causal order over the positions
    /// ([`crate::reduce::OrderError`]).
    pub fn new(labels: Vec<L>, preds: Vec<Vec<usize>>) -> Result<Self, String> {
        if labels.len() != preds.len() {
            return Err(format!(
                "{} labels and {} predecessor lists",
                labels.len(),
                preds.len()
            ));
        }
        let order = CausalOrder::from_predecessors(preds).map_err(|e| format!("{e:?}"))?;
        Ok(Self { labels, order })
    }

    /// Its labels, in the replay's order.
    #[must_use]
    pub fn labels(&self) -> &[L] {
        &self.labels
    }

    /// Its length.
    #[must_use]
    pub fn len(&self) -> usize {
        self.labels.len()
    }

    /// Whether it is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.labels.is_empty()
    }
}

/// What the validator's replay of a subject showed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StoryReplay<L> {
    /// The subject is a run and shows the preserved failure; its story.
    Fails(Story<L>),
    /// The subject is a run and the failure does not reproduce.
    Holds(String),
    /// The subject is no run of the program.
    NotARun(String),
    /// The replay could not decide (INV-008).
    Inconclusive(InconclusiveReason, String),
}

/// A mechanism that cannot be derived or checked: an INV-008 reason, with a rendering.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Undecided {
    /// Why.
    pub reason: InconclusiveReason,
    /// The rendering.
    pub detail: String,
}

/// The replay that judges a reduction's result, apart from the reduction's own oracle.
/// `T` is what a reduction returns: a set of trace indices for [`crate::reduce`], a
/// configuration for [`crate::scenario`].
pub trait Validator<T: ?Sized> {
    /// The step alphabet.
    type Label: Ord + Clone + std::fmt::Debug;

    /// The mechanism of the original failure, derived from it. Asked once per
    /// reduction.
    ///
    /// # Errors
    ///
    /// The mechanism cannot be derived: the reduction is then inconclusive.
    fn mechanism(&mut self) -> Result<Mechanism<Self::Label>, Undecided>;

    /// An upper bound on the work of [`Self::story`] on `subject`: charged before it
    /// runs.
    fn story_cost(&self, subject: &T) -> u64;

    /// Replay `subject` apart from the reduction's oracle, check the failure, and read
    /// the story.
    fn story(&mut self, subject: &T) -> StoryReplay<Self::Label>;

    /// `label` of the original failure's mechanism, as `subject` names it. The identity
    /// unless the reduction renames the configuration.
    fn rename(&self, _subject: &T, label: &Self::Label) -> Self::Label {
        label.clone()
    }
}

/// How the story fails to embed the mechanism.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mismatch {
    /// Too few positions have this step's label: none, or fewer than the steps with
    /// that label up to this one. The operation is gone.
    MissingStep {
        /// The step.
        step: usize,
    },
    /// Every step has a position, but no embedding satisfies the edges up to and
    /// including this one.
    OrderBroken {
        /// The first edge no embedding can add.
        edge: usize,
    },
    /// Some embedding satisfies every edge, but none satisfies the guards up to and
    /// including this one.
    GuardBroken {
        /// The first guard no embedding can add.
        guard: usize,
    },
}

/// Why a reduction's result was rejected: the right verdict, or not even that, but not
/// the mechanism.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Rejection {
    /// The validator's replay says the result is no run of the program.
    NotARun(String),
    /// The result is a run, and the failure does not reproduce.
    FailureLost(String),
    /// The failure reproduces, through a different mechanism.
    Mechanism(Mismatch),
}

/// What the check spent, and what it found, of a result it preserved.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckRecord {
    /// The mechanism's size: steps, edges, guards.
    pub mechanism: [usize; 3],
    /// The story's length.
    pub story: usize,
    /// The embedding: each step's position in the story.
    pub embedding: Vec<usize>,
    /// What the check spent: one replay, and work units.
    pub spent: Spent,
}

/// A result that the check found preserves the mechanism. Only this module constructs
/// one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Validated<T> {
    subject: T,
    record: CheckRecord,
}

impl<T> Validated<T> {
    /// The validated result.
    #[must_use]
    pub const fn subject(&self) -> &T {
        &self.subject
    }

    /// What the check found.
    #[must_use]
    pub const fn record(&self) -> &CheckRecord {
        &self.record
    }

    /// The validated result, by value. The value no longer carries the check.
    #[must_use]
    pub fn into_inner(self) -> T {
        self.subject
    }
}

impl<T> std::ops::Deref for Validated<T> {
    type Target = T;

    fn deref(&self) -> &T {
        &self.subject
    }
}

/// A result, checked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Checked<T> {
    /// It preserves the mechanism.
    Preserved(Validated<T>),
    /// It does not.
    Rejected {
        /// The result.
        subject: T,
        /// Why.
        why: Rejection,
        /// What the check spent.
        spent: Spent,
    },
    /// The check could not decide (INV-008). Never a pass.
    Undecided {
        /// The result.
        subject: T,
        /// Why.
        reason: InconclusiveReason,
        /// The rendering.
        detail: String,
        /// What the check spent.
        spent: Spent,
    },
}

impl<T> Checked<T> {
    /// The result, whatever the check said.
    #[must_use]
    pub fn subject(&self) -> &T {
        match self {
            Self::Preserved(v) => v.subject(),
            Self::Rejected { subject, .. } | Self::Undecided { subject, .. } => subject,
        }
    }

    /// The outcome without the result.
    #[must_use]
    pub fn outcome(&self) -> Outcome {
        match self {
            Self::Preserved(v) => Outcome::Preserved(v.record.clone()),
            Self::Rejected { why, spent, .. } => Outcome::Rejected(why.clone(), *spent),
            Self::Undecided {
                reason,
                detail,
                spent,
                ..
            } => Outcome::Undecided(*reason, detail.clone(), *spent),
        }
    }
}

/// A check's outcome, without the result it checked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// Preserved, with what the check found.
    Preserved(CheckRecord),
    /// Rejected, with why and what the check spent.
    Rejected(Rejection, Spent),
    /// Undecided (INV-008), with why, its rendering and what the check spent.
    Undecided(InconclusiveReason, String, Spent),
}

impl Outcome {
    /// `subject`, checked with this outcome. Crate-only: this is how a result gets its
    /// [`Validated`] form, and only from an outcome [`check`] produced.
    pub(crate) fn attach<T>(self, subject: T) -> Checked<T> {
        match self {
            Self::Preserved(record) => Checked::Preserved(Validated { subject, record }),
            Self::Rejected(why, spent) => Checked::Rejected {
                subject,
                why,
                spent,
            },
            Self::Undecided(reason, detail, spent) => Checked::Undecided {
                subject,
                reason,
                detail,
                spent,
            },
        }
    }
}

/// The validation allowance: replays and work units, charged before the check's work,
/// apart from the passes' budget.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Allowance {
    /// Replays left.
    pub replays: u64,
    /// Work units left.
    pub work: u64,
}

impl Allowance {
    /// The allowance [`crate::reduce::Budget::new`] grants: 64 replays and 2^34 work
    /// units.
    pub const DEFAULT: Self = Self {
        replays: 64,
        work: 1 << 34,
    };

    fn charge(&mut self, units: u64, spent: &mut Spent) -> bool {
        if self.work < units {
            return false;
        }
        self.work -= units;
        spent.work = spent.work.saturating_add(units);
        true
    }
}

/// A meter of work units: charged before each step of the embedding search.
struct Meter<'a> {
    allowance: &'a mut Allowance,
    spent: &'a mut Spent,
}

impl Meter<'_> {
    fn charge(&mut self, units: u64) -> Result<(), ()> {
        if self.allowance.charge(units, self.spent) {
            Ok(())
        } else {
            Err(())
        }
    }
}

fn units(n: usize) -> u64 {
    u64::try_from(n).unwrap_or(u64::MAX)
}

/// The embedding search's shared state.
struct Search<'a, L> {
    mechanism: &'a Mechanism<L>,
    story: &'a Story<L>,
    /// Each step's candidate positions, ascending.
    candidates: Vec<Vec<usize>>,
    /// Each guard's forbidden labels, and its ending labels.
    forbidden: Vec<BTreeSet<&'a L>>,
    until: Vec<BTreeSet<&'a L>>,
    /// Each position's happens-before past, computed once when first asked.
    past: Vec<Option<Vec<bool>>>,
}

impl<L: Ord> Search<'_, L> {
    fn happens_before(&mut self, a: usize, b: usize, meter: &mut Meter<'_>) -> Result<bool, ()> {
        if self.past[b].is_none() {
            // The walk, its result and the mark: three passes over the order.
            meter.charge(self.story.order.walk_cost().saturating_mul(3))?;
            let mut mark = vec![false; self.story.len()];
            for e in self.story.order.down_closure(&[b]) {
                mark[e] = true;
            }
            self.past[b] = Some(mark);
        }
        Ok(a != b && self.past[b].as_ref().is_some_and(|m| m[a]))
    }

    fn edge_holds(&mut self, e: Edge, at: &[usize], meter: &mut Meter<'_>) -> Result<bool, ()> {
        meter.charge(1)?;
        let (a, b) = (at[e.from], at[e.to]);
        match e.order {
            Order::Precedes => Ok(a < b),
            Order::HappensBefore => self.happens_before(a, b, meter),
        }
    }

    fn guard_holds(&self, g: usize, at: &[usize], meter: &mut Meter<'_>) -> Result<bool, ()> {
        let guard = &self.mechanism.guards[g];
        let (a, b) = (at[guard.after], at[guard.before]);
        if a >= b {
            return Ok(false);
        }
        // Each position of the interval: two set lookups.
        let size = self.forbidden[g].len().max(self.until[g].len());
        let log = u64::from(usize::BITS - size.leading_zeros());
        meter.charge(units(b - a).saturating_mul(log.saturating_add(1).saturating_mul(2)))?;
        for l in &self.story.labels[a + 1..b] {
            if self.forbidden[g].contains(l) {
                return Ok(false);
            }
            if self.until[g].contains(l) {
                return Ok(true);
            }
        }
        Ok(true)
    }

    /// Whether some injective, label-keeping assignment satisfies the first `edges`
    /// edges and the first `guards` guards; the assignment when one does.
    fn find(
        &mut self,
        edges: usize,
        guards: usize,
        meter: &mut Meter<'_>,
    ) -> Result<Option<Vec<usize>>, ()> {
        let n = self.mechanism.steps.len();
        // The setup below: the constraint tables and the assignment, charged first.
        meter.charge(
            units(n)
                .saturating_add(units(edges))
                .saturating_add(units(guards))
                .saturating_add(units(self.story.len())),
        )?;
        // The constraints checked when step `s` is assigned: those whose later step is `s`.
        let mut edges_at: Vec<Vec<usize>> = vec![Vec::new(); n];
        for (k, e) in self.mechanism.edges[..edges].iter().enumerate() {
            edges_at[e.from.max(e.to)].push(k);
        }
        let mut guards_at: Vec<Vec<usize>> = vec![Vec::new(); n];
        for (k, g) in self.mechanism.guards[..guards].iter().enumerate() {
            guards_at[g.after.max(g.before)].push(k);
        }
        let mut at = vec![usize::MAX; n];
        let mut used = vec![false; self.story.len()];
        // Iterative backtracking: `next[s]` is the index into `candidates[s]` to try next.
        let mut next = vec![0_usize; n];
        let mut s = 0_usize;
        loop {
            if s == n {
                return Ok(Some(at));
            }
            let mut placed = false;
            while next[s] < self.candidates[s].len() {
                let p = self.candidates[s][next[s]];
                next[s] += 1;
                meter.charge(1)?;
                if used[p] {
                    continue;
                }
                at[s] = p;
                let mut ok = true;
                for &k in &edges_at[s] {
                    if !self.edge_holds(self.mechanism.edges[k], &at, meter)? {
                        ok = false;
                        break;
                    }
                }
                if ok {
                    for &k in &guards_at[s] {
                        if !self.guard_holds(k, &at, meter)? {
                            ok = false;
                            break;
                        }
                    }
                }
                if ok {
                    used[p] = true;
                    placed = true;
                    break;
                }
                at[s] = usize::MAX;
            }
            if placed {
                s += 1;
                if s < n {
                    next[s] = 0;
                }
                continue;
            }
            // No position for `s`: back up to the previous step and try its next one.
            if s == 0 {
                return Ok(None);
            }
            s -= 1;
            used[at[s]] = false;
            at[s] = usize::MAX;
        }
    }
}

/// Embed `mechanism` in `story`, charging `allowance` before each step of the work.
/// `Ok(Ok(positions))` is an embedding; `Ok(Err(mismatch))` says why there is none;
/// `Err(AllowanceExhausted)` is an allowance that ran out before the search decided.
///
/// The mismatch is the first failure in a fixed order: the lowest step with no
/// equally labelled position; else the first edge `k` such that no assignment satisfies
/// edges `0..=k`; else the first guard `k` such that none satisfies every edge and
/// guards `0..=k`. So the reason is a function of the mechanism and the story alone.
///
/// # Errors
///
/// [`AllowanceExhausted`] when the allowance runs out.
pub fn embed<L: Ord + Clone>(
    mechanism: &Mechanism<L>,
    story: &Story<L>,
    allowance: &mut Allowance,
    spent: &mut Spent,
) -> Result<Result<Vec<usize>, Mismatch>, AllowanceExhausted> {
    embed_metered(mechanism, story, allowance, spent).map_err(|()| AllowanceExhausted)
}

/// The validation allowance ran out before the embedding search decided.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AllowanceExhausted;

fn embed_metered<L: Ord + Clone>(
    mechanism: &Mechanism<L>,
    story: &Story<L>,
    allowance: &mut Allowance,
    spent: &mut Spent,
) -> Result<Result<Vec<usize>, Mismatch>, ()> {
    let mut meter = Meter { allowance, spent };
    let n = mechanism.steps.len();
    // The candidate lists: one comparison per step and position, charged first.
    meter.charge(units(n).saturating_mul(units(story.len()).saturating_add(1)))?;
    let candidates: Vec<Vec<usize>> = mechanism
        .steps
        .iter()
        .map(|l| {
            story
                .labels
                .iter()
                .enumerate()
                .filter(|(_, x)| *x == l)
                .map(|(p, _)| p)
                .collect()
        })
        .collect();
    // A step with no position, or with fewer positions than steps of its label up to it:
    // no injective assignment exists, whatever the constraints. One map operation per
    // step, charged first.
    meter.charge(units(n).saturating_mul(units(usize::BITS as usize)))?;
    let mut earlier: std::collections::BTreeMap<&L, usize> = std::collections::BTreeMap::new();
    for (step, l) in mechanism.steps.iter().enumerate() {
        let k = earlier.entry(l).or_default();
        *k += 1;
        if candidates[step].len() < *k {
            return Ok(Err(Mismatch::MissingStep { step }));
        }
    }
    let forbidden_labels: usize = mechanism
        .guards
        .iter()
        .map(|g| g.forbidden.len() + g.until.len())
        .sum();
    meter.charge(units(forbidden_labels).saturating_mul(units(usize::BITS as usize)))?;
    let mut search = Search {
        mechanism,
        story,
        candidates,
        forbidden: mechanism
            .guards
            .iter()
            .map(|g| g.forbidden.iter().collect())
            .collect(),
        until: mechanism
            .guards
            .iter()
            .map(|g| g.until.iter().collect())
            .collect(),
        past: vec![None; story.len()],
    };
    let (edges, guards) = (mechanism.edges.len(), mechanism.guards.len());
    if let Some(at) = search.find(edges, guards, &mut meter)? {
        return Ok(Ok(at));
    }
    for k in 0..edges {
        if search.find(k + 1, 0, &mut meter)?.is_none() {
            return Ok(Err(Mismatch::OrderBroken { edge: k }));
        }
    }
    for k in 0..guards {
        if search.find(edges, k + 1, &mut meter)?.is_none() {
            return Ok(Err(Mismatch::GuardBroken { guard: k }));
        }
    }
    // Every prefix is satisfiable and the whole is not: impossible, since the last
    // prefix is the whole. Kept total rather than asserted.
    Ok(Err(Mismatch::GuardBroken {
        guard: guards.saturating_sub(1),
    }))
}

/// Check `subject` with `validator` against `mechanism` (the original failure's, before
/// the subject's renaming), from `allowance`. The only producer of a preserving
/// [`Outcome`], and so, through [`Outcome::attach`], of a [`Validated`] result.
pub(crate) fn check<T: ?Sized, V: Validator<T>>(
    validator: &mut V,
    mechanism: &Result<Mechanism<V::Label>, Undecided>,
    subject: &T,
    allowance: &mut Allowance,
) -> Outcome {
    let mut spent = Spent::default();
    let mechanism = match mechanism {
        Ok(m) => m,
        Err(u) => return Outcome::Undecided(u.reason, u.detail.clone(), spent),
    };
    let exhausted = |spent: Spent, what: &str| {
        Outcome::Undecided(
            InconclusiveReason::ResourceExhausted,
            format!("the validation allowance ran out before {what}"),
            spent,
        )
    };
    if allowance.replays == 0 {
        return exhausted(spent, "the validation replay");
    }
    if !allowance.charge(validator.story_cost(subject), &mut spent) {
        return exhausted(spent, "the validation replay");
    }
    allowance.replays -= 1;
    spent.replays = spent.replays.saturating_add(1);
    let story = match validator.story(subject) {
        StoryReplay::Fails(story) => story,
        StoryReplay::Holds(why) => return Outcome::Rejected(Rejection::FailureLost(why), spent),
        StoryReplay::NotARun(why) => return Outcome::Rejected(Rejection::NotARun(why), spent),
        StoryReplay::Inconclusive(reason, why) => return Outcome::Undecided(reason, why, spent),
    };
    // Each label renamed, and the guards' sets sorted again.
    let labels = units(mechanism.labels());
    if !allowance.charge(
        labels.saturating_mul(u64::from(u64::BITS - labels.leading_zeros()).saturating_add(1)),
        &mut spent,
    ) {
        return exhausted(spent, "the mechanism's renaming");
    }
    let renamed = mechanism.renamed(|l| validator.rename(subject, l));
    match embed(&renamed, &story, allowance, &mut spent) {
        Ok(Ok(embedding)) => Outcome::Preserved(CheckRecord {
            mechanism: [
                renamed.steps.len(),
                renamed.edges.len(),
                renamed.guards.len(),
            ],
            story: story.len(),
            embedding,
            spent,
        }),
        Ok(Err(m)) => Outcome::Rejected(Rejection::Mechanism(m), spent),
        Err(AllowanceExhausted) => exhausted(spent, "the embedding search decided"),
    }
}

/// Where in a reduction a check ran.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Stage {
    /// The input, before any pass: its failure must show the mechanism it was derived
    /// from.
    Input,
    /// The result of an event pass of [`crate::reduce`] that changed its input.
    Pass(crate::reduce::Pass),
    /// The result of a scenario pass of [`crate::scenario`] that kept a candidate.
    Dimension {
        /// The round.
        round: u64,
        /// The pass.
        dimension: crate::scenario::Dimension,
    },
    /// The best result of a reduction that stopped early.
    Best,
    /// A finished reduction's result that no earlier check covered. The engine checks
    /// each pass's result as the pass ends, so this stage is recorded only if that
    /// invariant breaks: the result is then still checked.
    Result,
}

/// One check of a reduction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckEntry {
    /// Where it ran.
    pub stage: Stage,
    /// The outcome.
    pub outcome: Outcome,
}
