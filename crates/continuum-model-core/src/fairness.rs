//! Fairness assumptions: the \(F\) of the model tuple (bn-1ln12).
//!
//! > A model \(M\) defines: \(M = (S, I, \mathcal{A}, T, O, F)\) … \(F\): fairness
//! > assumptions.
//! >
//! > — `notes/plan/docs/02_SEMANTICS.md` §2
//!
//! > Fairness objects are typed and scoped: `WeakFair(action, scope)`;
//! > `StrongFair(action, scope)`; … No global "fair scheduler" switch exists.
//! >
//! > — `notes/plan/docs/02_SEMANTICS.md` §8
//!
//! > `weak_fair(Action): continuously enabled ⇒ eventually taken`
//! > `strong_fair(Action): enabled infinitely often ⇒ taken infinitely often`
//! >
//! > — `notes/plan/rfcs/0008-liveness-fairness-and-progress.md`, "Fairness scopes"
//!
//! # What a fairness assumption is here
//!
//! A [`Fairness`] is a [`Strength`] and a non-empty *set* of the model's actions, its
//! *scope*. It is one assumption about the whole set, not one per member:
//!
//! - the scope is **enabled** at a state when at least one of its actions is enabled
//!   there ([`crate::Model::is_enabled`]);
//! - the scope is **taken** at a step when the step's action ([`crate::Step::action`])
//!   is one of its actions.
//!
//! Over an infinite labelled execution `s0 -a0-> s1 -a1-> …` of the model:
//!
//! - **weak** fairness holds when, if from some point on the scope is enabled at every
//!   state, then the scope is taken infinitely often (`◇□ enabled ⇒ □◇ taken`);
//! - **strong** fairness holds when, if the scope is enabled at infinitely many
//!   states, then it is taken infinitely often (`□◇ enabled ⇒ □◇ taken`).
//!
//! An execution is *fair* when it satisfies every declared assumption. A model with no
//! assumption declares none: there is no default (RFC 0008 "no hidden fairness
//! defaults").
//!
//! ## Why a scope is a set
//!
//! docs/02 §2 makes an action a *schema* whose relation ranges over its parameters,
//! \(T_a \subseteq S \times Params_a \times S\), and RFC 0008 and RFC 0015 attach
//! fairness to the schema. A front end that expands a schema into one action per
//! parameter tuple (CML does: `Recover(n=a)`, `Recover(n=b)`, …) must keep the
//! assumption on the schema, which is the set of its instances. A set scope is that
//! assumption exactly; one assumption per instance would be a strictly stronger one
//! (INV-011 makes strengthening an assumption a privileged revision, never a
//! lowering choice). A programmatic author who wants per-instance fairness declares
//! one assumption per action.
//!
//! ## What "taken" and "enabled" read
//!
//! The *label* of a step, not its effect. A step of an action in the scope is taken
//! whether or not it changes the state, and the scope is enabled wherever one of its
//! guards holds. That is the labelled reading docs/02 §2 gives an execution ("a
//! labeled event structure") and the one `continuum-kernel-temporal`'s fair-cycle
//! check makes ("an edge labelled with it"). A TLA+ `WF_v(A)` reads `<<A>>_v`, the
//! steps of `A` that change `v`; the two differ only on a step of `A` that leaves `v`
//! unchanged, and such a port states its view explicitly (docs/25's "named fairness
//! monitors"). A view other than the label is not implemented here.
//!
//! ## Finite executions
//!
//! A state with no enabled action ends an execution. Every scope is disabled there, so
//! no assumption constrains it. Whether such an execution is a behaviour at all is the
//! completion policy of RFC 0015 ("No engine silently changes this policy"), which is
//! checking policy and lives with the check, never here.
//!
//! # Canonical form
//!
//! [`crate::ModelBuilder::build`] resolves every name to its action's index, holds a
//! scope as the strictly ascending list of those indices (a set: a name given twice is
//! one member), and holds the assumptions sorted by `(strength, scope)` with
//! duplicates collapsed, because an assumption stated twice is stated once. So two
//! models that declare the same assumptions in any order, with any repetition, are one
//! model with one identity.

use core::fmt;

/// The largest number of fairness assumptions a model may declare.
///
/// Equal to [`crate::model::MAX_ACTIONS`]: a front end that declares at most one
/// assumption of each strength per action schema stays far inside it, and a model past
/// it is refused where it is declared rather than carried.
pub const MAX_FAIRNESS: usize = crate::model::MAX_ACTIONS;

/// Weak or strong fairness.
///
/// The order (`Weak` before `Strong`) is the canonical order of assumptions in a
/// model and in its identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Strength {
    /// Justice: continuously enabled from some point on implies taken infinitely often.
    Weak,
    /// Compassion: enabled infinitely often implies taken infinitely often.
    Strong,
}

impl Strength {
    /// Both strengths, in canonical order.
    pub const ALL: [Self; 2] = [Self::Weak, Self::Strong];

    /// The stable machine name, as the intent contract spells a fairness `kind`
    /// (`notes/plan/schemas/intent-contract.schema.json`, `fairness[].kind`).
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Weak => "weak",
            Self::Strong => "strong",
        }
    }
}

impl fmt::Display for Strength {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// One fairness assumption: a strength over a non-empty set of actions.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Fairness {
    strength: Strength,
    actions: Vec<usize>,
}

impl Fairness {
    /// Built only by [`crate::ModelBuilder::build`], from a non-empty, strictly
    /// ascending list of indices into the model's actions.
    pub(crate) const fn new(strength: Strength, actions: Vec<usize>) -> Self {
        Self { strength, actions }
    }

    /// Weak or strong.
    #[must_use]
    pub const fn strength(&self) -> Strength {
        self.strength
    }

    /// The scope: indices into [`crate::Model::actions`], strictly ascending and never
    /// empty.
    #[must_use]
    pub fn actions(&self) -> &[usize] {
        &self.actions
    }

    /// Whether the action at `index` is in the scope. A binary search, so a caller that
    /// asks once per transition pays a logarithm, not a scan.
    #[must_use]
    pub fn contains(&self, index: usize) -> bool {
        self.actions.binary_search(&index).is_ok()
    }
}
