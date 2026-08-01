//! Invariant and deadlock checking over an explored state set (PR 8, IMPL-03).
//!
//! # What this module answers
//!
//! docs/03 §6.1 lists "evaluate property for every state" as step 5 of what a
//! closed-finite-state-space checker does. This is the producer side of the same
//! sentence: given a [`Model`], an [`Exploration`] of it, and a declaration of which
//! of the model's own predicates are to be *upheld*, it evaluates each one at every
//! discovered state and reports a typed outcome per predicate — together with what
//! the caller's declared completion policy makes of the states that have no enabled
//! action.
//!
//! It adds no evaluation of its own. Every answer here comes from the two primitives
//! the model layer's seam table promised this bone
//! (`crates/continuum-engine-reference/src/model.rs:27-32`):
//! [`Model::evaluate_predicate`] for an invariant, and an empty [`Model::successors`]
//! for a terminal state — "docs/03 §6.1's *disabled witness*"
//! (`crates/continuum-engine-reference/src/model.rs:444-452`). Every path this module
//! hands back comes from [`crate::witness::shortest`]. Nothing here walks a graph,
//! evaluates an expression, or decides what a shortest path is.
//!
//! # An outcome is not a `bool`
//!
//! The vocabulary is the assurance result's, not a flag:
//!
//! > | `verdict` | `enum`: `established`, `refuted`, `inconclusive` |
//! >
//! > "Budget exhaustion is never a verdict (plan §11.4); it is the `BudgetExhausted`
//! > error with a continuation. Unsupported and engine-error outcomes are not
//! > verdicts either: they are inconclusive with `inconclusive_reason`
//! > Unsupported/EngineError (INV-008)."
//! >
//! > — `notes/plan/schemas/assurance-result.schema.json:203-213`
//!
//! [`CheckOutcome`] is that three-way vocabulary at the grain one predicate over one
//! exploration can support, and it is closed. The shape is the trusted checking
//! base's, restated on the untrusted side: `continuum-kernel-core`'s `Verdict` is
//! "a closed three-way vocabulary" whose success arm "carries a `CheckedClaim` naming
//! the obligations that were discharged" and which distinguishes *this artifact is
//! invalid* from *this checker does not implement that*
//! (`crates/continuum-kernel-core/src/verdict.rs:1-36`). The distinction an engine
//! has to preserve is a different one — *no state falsifies this* from *no state I
//! looked at falsifies this* — and it is preserved the same way: by being an arm.
//!
//! # The asymmetry, which is the whole point
//!
//! A bounded exploration is a depth-preserving *prefix* of the complete one
//! (`crates/continuum-engine-reference/src/bfs.rs:39-49`), and the two directions of
//! a safety property behave differently across a prefix. `crate::bfs` states the rule
//! and assigns it here:
//!
//! > Useful precisely because a violation found in a partial set is a real violation:
//! > the states here were genuinely reached. The converse does not hold, and that
//! > asymmetry is the checking bone's to encode — *not finding* a violation in a
//! > partial set is `Inconclusive`/`ResourceExhausted` (INV-008; RFC 0026's
//! > `InconclusiveReason`), never a pass.
//! >
//! > — `crates/continuum-engine-reference/src/bfs.rs:542-546`
//!
//! So:
//!
//! | Exploration | Some explored state falsifies it | None does |
//! |---|---|---|
//! | [`Exploration::Complete`] | [`CheckOutcome::Violated`] with a shortest witness | [`CheckOutcome::Holds`] |
//! | [`Exploration::Exhausted`] | [`CheckOutcome::Violated`], **unwitnessed** | [`CheckOutcome::Inconclusive`] |
//!
//! Three of those four cells are forced. The fourth — a violation found inside a
//! truncated run — is a genuine refutation and is reported as one, because the state
//! was genuinely reached by a genuine sequence of transitions and no larger budget
//! can un-reach it. What a larger budget *can* change is the shortest path to it, so
//! that cell carries no path: [`crate::witness::shortest`] answers only for a closed
//! exploration, deliberately and "even when the endpoint was in fact discovered at
//! its true depth" (`crates/continuum-engine-reference/src/witness.rs:45-64`), and
//! this module does not work around its own sibling's refusal. The absence is typed
//! rather than empty — [`Evidence::Unwitnessed`] carries the
//! [`NoWitness`] the extractor returned.
//!
//! Nothing in this module can produce [`CheckOutcome::Holds`] from a truncated
//! exploration. That is the load-bearing INV-008 case: reaching a declared bound is
//! spend, not an answer, and an engine that let a budget become a pass would report a
//! property as holding because it ran out of room (`notes/plan/plan.md:335-337`;
//! `notes/plan/rfcs/0026-continuumd-native-protocol.md:360`).
//!
//! # Why the deadlock question is the caller's
//!
//! An empty successor row is a *fact*; whether it is a defect is a *declaration*. The
//! intent contract carries the declaration, as a closed four-member vocabulary that
//! no engine may quietly reinterpret:
//!
//! > `completion_policy` — "Behavior-completion policy for terminal states (RFC
//! > 0015). No engine may silently change this." `enum`: `stutter-forever`,
//! > `deadlock-violation`, `finite-trace-only`, `closed`.
//! >
//! > — `notes/plan/schemas/intent-contract.schema.json:772-780`; RFC 0015 "Core
//! >   behavior model"
//!
//! [`DeadlockPolicy`] is the engine-grain projection of that field, and it is an
//! argument rather than a default because a default would be a policy nobody wrote
//! down — the same reason [`crate::bfs::Bounds`] has no [`Default`]. docs/27 records
//! why the question is separate from the invariant one at all: "Deadlock is a
//! semantic property distinct from invariant failure"
//! (`notes/plan/docs/27_SPIKE_FINDINGS_REV2.md:47`), observed on Dining Philosophers,
//! whose canonical deadlock is the corpus's second frozen example.
//!
//! The projection is two-valued and the mapping is stated rather than assumed;
//! [`DeadlockPolicy`] documents which intent values reach which arm, and which one
//! has no reading at this layer.
//!
//! # Determinism (INV-005)
//!
//! There is no input but the model, the exploration and the [`Obligations`]. The
//! invariants are a `BTreeSet<usize>`, so they are answered in ascending index order
//! whatever order they were declared in and asking for one twice is asking for it
//! once. Both scans run over [`crate::bfs::Reachable::states`], which is ascending,
//! and the shallowest falsifying state is chosen by the rule
//! [`crate::witness::shortest`] uses for its endpoint — first strict improvement over
//! an ascending scan, so ties go to the canonically least state. Two checks of one
//! model under one exploration are *equal*, and their [`Display`](fmt::Display)
//! renderings are byte-identical; `tests/checking_contract.rs` asserts it.

use core::fmt;
use std::collections::BTreeSet;

use crate::bfs::{Bound, Exploration, Reachable};
use crate::ident::Ident;
use crate::model::{EvaluationError, Model, State};
use crate::witness::{self, NoWitness, Target, Witness};

// ---------------------------------------------------------------------------
// what a check is asked to uphold
// ---------------------------------------------------------------------------

/// What the caller's completion policy makes of a state with no enabled action.
///
/// The engine-grain projection of the intent contract's `completion_policy`
/// (`notes/plan/schemas/intent-contract.schema.json:772-780`, RFC 0015), whose four
/// members map here as follows:
///
/// | `completion_policy` | Here |
/// |---|---|
/// | `deadlock-violation` | [`DeadlockPolicy::Defect`] |
/// | `stutter-forever` | [`DeadlockPolicy::Allowed`] — a terminal state stutters, so the behaviour continues and terminality is not a failure |
/// | `finite-trace-only` | [`DeadlockPolicy::Allowed`] — the trace ends there and the property is stated over finite traces |
/// | `closed` | **no reading at this layer** |
///
/// `closed` is environment-closed completion: the behaviour is completed by the
/// environment rather than by the system, and [`Model`] declares no environment for
/// this layer to consult. Translating it to either arm would be inventing the
/// missing half, so it is not translated — a caller holding `closed` has a question
/// this engine cannot answer yet, which is a statement worth making rather than a
/// guess worth hiding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DeadlockPolicy {
    /// A reachable state with no enabled action is a defect, to be reported with a
    /// path to it.
    Defect,
    /// A reachable state with no enabled action is legitimate. Terminal states are
    /// still counted, because their number is a fact about the model either way, but
    /// none is judged and none is witnessed.
    Allowed,
}

impl DeadlockPolicy {
    /// Every policy this layer distinguishes.
    pub const ALL: [Self; 2] = [Self::Defect, Self::Allowed];

    /// The stable machine name.
    ///
    /// Deliberately *not* an intent-contract spelling: `Defect` is exactly
    /// `deadlock-violation`, but `Allowed` covers two of that vocabulary's members,
    /// so reusing one of their names here would claim a correspondence that does not
    /// hold.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Defect => "defect",
            Self::Allowed => "allowed",
        }
    }
}

impl fmt::Display for DeadlockPolicy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// What one check upholds: which of the model's declared predicates are invariants,
/// and what a terminal state means.
///
/// **Data, never closures.** An invariant is named by its index in
/// [`Model::predicates`], which is the same discipline [`Target`] holds for a
/// witness's endpoint and [`crate::expr`] holds for a guard: a `Fn(&State) -> bool`
/// would be ambient effect reaching into the check, it would put the property outside
/// the model the result is a claim about, and it could not be digested into the
/// `property_digest` an RFC 0005 claim envelope carries.
///
/// A [`Model`] deliberately does not know which of its predicates are invariants —
/// "Whether `TypeOK` is an invariant to be upheld and `NotSolved` a goal whose
/// refutation is the answer is checking policy … so that one model can be checked
/// under several policies without being re-declared"
/// (`crates/continuum-engine-reference/src/model.rs:34-38`). This type is where that
/// policy is finally written down, per check.
///
/// The invariants are held as a set, so declaring one twice declares it once and the
/// report's order is ascending index order rather than the order someone typed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Obligations {
    invariants: BTreeSet<usize>,
    deadlock: DeadlockPolicy,
}

impl Obligations {
    /// A check that upholds no invariant, under the declared deadlock policy.
    ///
    /// There is no [`Default`]: the deadlock policy has no safe default value, only a
    /// popular one, and [`crate::bfs::Bounds`] records the same argument for a budget.
    #[must_use]
    pub const fn new(deadlock: DeadlockPolicy) -> Self {
        Self {
            invariants: BTreeSet::new(),
            deadlock,
        }
    }

    /// Also uphold the model's predicate at `index`.
    ///
    /// The index is not validated here — [`check`] validates it against the model it
    /// is checked over, because that is the only place both are in hand.
    #[must_use]
    pub fn invariant(mut self, index: usize) -> Self {
        self.invariants.insert(index);
        self
    }

    /// Uphold every predicate `model` declares, under the declared deadlock policy.
    ///
    /// Convenience, and honest about what it means: a model's predicates are named
    /// boolean evaluations, and upholding all of them is one policy among several.
    /// Die Hard's `NotSolved` is "intentionally false" (`DieHard.ctm:31`), so this
    /// constructor over that model asks for exactly one violation and gets it.
    #[must_use]
    pub fn every_predicate(model: &Model, deadlock: DeadlockPolicy) -> Self {
        Self {
            invariants: (0..model.predicates().len()).collect(),
            deadlock,
        }
    }

    /// The predicate indices to uphold, ascending and duplicate-free.
    #[must_use]
    pub const fn invariants(&self) -> &BTreeSet<usize> {
        &self.invariants
    }

    /// What a terminal state means for this check.
    #[must_use]
    pub const fn deadlock(&self) -> DeadlockPolicy {
        self.deadlock
    }
}

// ---------------------------------------------------------------------------
// what the answers are about
// ---------------------------------------------------------------------------

/// How much of the model a report's answers are about.
///
/// docs/03 §3's exploration lattice names both levels and keeps them apart:
/// `EXHAUSTIVE_FINITE` is "complete reachable finite state space" and
/// `BOUNDED_STATES` is "all states within declared model scope"
/// (`notes/plan/docs/03_ASSURANCE_AND_TCB.md` §3). A report carries which one it is
/// so that no reader has to infer it from the outcomes, and so that
/// [`CheckOutcome::Holds`] can be the arm that only the first level admits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    /// docs/03 §3 `EXHAUSTIVE_FINITE`: the exploration closed, so "every discovered
    /// state" and "every reachable state" are the same set.
    Complete {
        /// How many states that is.
        states: usize,
    },
    /// docs/03 §3 `BOUNDED_STATES`: a declared bound stopped the exploration, so the
    /// discovered states are a prefix of the reachable ones.
    Bounded {
        /// How many states were explored.
        states: usize,
        /// Which declared bound stopped it.
        tripped: Bound,
        /// How many discovered states were never expanded.
        frontier: usize,
    },
}

impl Scope {
    /// The scope of an exploration.
    #[must_use]
    pub fn of(exploration: &Exploration) -> Self {
        match exploration {
            Exploration::Complete(reachable) => Self::Complete {
                states: reachable.len(),
            },
            Exploration::Exhausted(partial) => Self::Bounded {
                states: partial.explored().len(),
                tripped: partial.tripped(),
                frontier: partial.frontier().len(),
            },
        }
    }

    /// How many states the answers were computed over, either way.
    #[must_use]
    pub const fn states(self) -> usize {
        match self {
            Self::Complete { states } | Self::Bounded { states, .. } => states,
        }
    }

    /// Whether the exploration closed.
    #[must_use]
    pub const fn is_complete(self) -> bool {
        matches!(self, Self::Complete { .. })
    }

    /// The reason a claim over this scope cannot be established, or `None` when it
    /// can.
    const fn unresolved(self) -> Option<Unresolved> {
        match self {
            Self::Complete { .. } => None,
            Self::Bounded {
                states,
                tripped,
                frontier,
            } => Some(Unresolved::ResourceExhausted {
                tripped,
                explored: states,
                frontier,
            }),
        }
    }
}

impl fmt::Display for Scope {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            Self::Complete { states } => write!(f, "complete states={states}"),
            Self::Bounded {
                states,
                tripped,
                frontier,
            } => write!(
                f,
                "bounded states={states} tripped={tripped} frontier={frontier}"
            ),
        }
    }
}

// ---------------------------------------------------------------------------
// why a claim was not decided
// ---------------------------------------------------------------------------

/// Why one claim could not be decided (INV-008).
///
/// > Timeout, unsupported semantics, insufficient telemetry, abstraction ambiguity,
/// > and incomplete proof search are distinct outcomes.
/// >
/// > — `notes/plan/plan.md:335-337`, INV-008
///
/// The dossier's closed six-member vocabulary is `InconclusiveReason`
/// (`notes/plan/schemas/assurance-result.schema.json:212-219`, mirrored in
/// `crates/continuum-value/src/assurance.rs:581-611`). Exactly two of its members can
/// be produced at this grain, and each arm here records which one it *is*, spelled
/// the way that vocabulary spells it, so a daemon lifting an engine outcome into an
/// assurance result reads the reason off rather than deciding it:
///
/// | Here | `InconclusiveReason` |
/// |---|---|
/// | [`Unresolved::ResourceExhausted`] | `ResourceExhausted` — INV-008's "timeout" |
/// | [`Unresolved::EngineError`] | `EngineError` — the engine failed rather than answered |
///
/// The other four are not reachable from a finite reference engine over a declared
/// model: `Unsupported` is a fragment question the declaration layer already refuses
/// (`ModelError`), `InsufficientTelemetry` needs telemetry this path has none of,
/// `AbstractionAmbiguity` needs a correspondence map, and `IncompleteProofSearch`
/// needs a proof search. This type is therefore a *named subset*, not a copy: it
/// carries the evidence for its reason, which the dossier enum does not, and it does
/// not offer four arms nothing here can construct.
///
/// `continuum-value` is not imported for the enum, for the reason
/// `crates/continuum-engine-reference/src/ident.rs` records against importing it at
/// all: its canonical name order is shortlex while this engine's certificate wire
/// form orders names by bytes, and one crate holding two orders for one concept is
/// the ambiguity `GOV-1-12` exists to forbid. The mapping is asserted as strings by
/// `tests/checking_contract.rs` instead, which is a test the two crates can share
/// without a dependency edge.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Unresolved {
    /// The exploration stopped at a declared bound and nothing in the explored prefix
    /// settled the claim. Raise the bound named here and ask again.
    ///
    /// Never a pass: budget spend is not a verdict
    /// (`notes/plan/rfcs/0026-continuumd-native-protocol.md:360`).
    ResourceExhausted {
        /// Which bound stopped the exploration.
        tripped: Bound,
        /// How many states it had explored.
        explored: usize,
        /// How many of those were never expanded.
        frontier: usize,
    },
    /// The model layer could not evaluate the claim at a reachable state.
    ///
    /// A defect in the declaration, not a budget question, and not a semantic answer
    /// either: "An engine defect is never a semantic verdict"
    /// (`notes/plan/rfcs/0026-continuumd-native-protocol.md:363`). Boxed for the
    /// reason [`crate::bfs::ExplorationError`] boxes its source — a well-formed model
    /// never produces the variant, so the success path should not be widened by a
    /// failure it does not have.
    EngineError {
        /// The state it was being evaluated at.
        state: State,
        /// What the model layer reported.
        source: Box<EvaluationError>,
    },
}

impl Unresolved {
    /// The `InconclusiveReason` member this is, spelled as the schemas spell it.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match *self {
            Self::ResourceExhausted { .. } => "ResourceExhausted",
            Self::EngineError { .. } => "EngineError",
        }
    }
}

impl fmt::Display for Unresolved {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ResourceExhausted {
                tripped,
                explored,
                frontier,
            } => write!(
                f,
                "reason=ResourceExhausted tripped={tripped} explored={explored} frontier={frontier}"
            ),
            Self::EngineError { state, source } => {
                write!(f, "reason=EngineError state={state} detail={source}")
            }
        }
    }
}

impl core::error::Error for Unresolved {
    fn source(&self) -> Option<&(dyn core::error::Error + 'static)> {
        match self {
            Self::EngineError { source, .. } => Some(&**source),
            Self::ResourceExhausted { .. } => None,
        }
    }
}

// ---------------------------------------------------------------------------
// the path to a defect
// ---------------------------------------------------------------------------

/// The path that accompanies a reported defect.
///
/// docs/03 §3 gives an assurance result a `witness` beside its `certificate` because
/// a positive claim survives an independent checker as a certificate and a negative
/// one survives as a replayable counterexample. This is the engine-grain form of that
/// field — and of its absence, which is typed rather than empty.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Evidence {
    /// A shortest labelled path from an initial state, from
    /// [`crate::witness::shortest`].
    Shortest(Witness),
    /// No path is offered, and this is why.
    ///
    /// The reachable case is [`NoWitness::Truncated`]: the defect is real — the state
    /// was genuinely reached — but the exploration is bounded, so no path from it is
    /// known to be shortest and the extractor refuses to hand one over rather than
    /// hand over a possibly longer one under the word *shortest*
    /// (`crates/continuum-engine-reference/src/witness.rs:45-56`). The remaining
    /// arms of [`NoWitness`] are structurally unreachable from a report this module
    /// builds — the endpoint came out of the exploration's own table, and its
    /// predicate was already evaluated there — and are carried rather than collapsed
    /// so that a mismatched model and exploration is reported instead of silently
    /// rendering as truncation.
    Unwitnessed(NoWitness),
}

impl Evidence {
    /// Wrap what [`crate::witness::shortest`] returned.
    fn of(extracted: Result<Witness, NoWitness>) -> Self {
        match extracted {
            Ok(found) => Self::Shortest(found),
            Err(reason) => Self::Unwitnessed(reason),
        }
    }

    /// The path, when there is one.
    #[must_use]
    pub const fn witness(&self) -> Option<&Witness> {
        match self {
            Self::Shortest(found) => Some(found),
            Self::Unwitnessed(_) => None,
        }
    }
}

impl fmt::Display for Evidence {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Shortest(found) => write!(f, "witness {found}"),
            Self::Unwitnessed(reason) => write!(f, "witness none ({reason})"),
        }
    }
}

// ---------------------------------------------------------------------------
// one invariant
// ---------------------------------------------------------------------------

/// What checking one invariant over one exploration established.
///
/// Closed by construction: a caller matching on this has enumerated every outcome
/// this engine can produce for one predicate (INV-008). The arms are the assurance
/// result's `established` / `refuted` / `inconclusive`, in that order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckOutcome {
    /// No reachable state falsifies the invariant, over a *closed* exploration.
    ///
    /// The only arm that needs the exploration to have finished, and the only one a
    /// bound can therefore take away. `Post(S) ⊆ S` came with the
    /// [`Exploration::Complete`] arm (`crates/continuum-engine-reference/src/bfs.rs:529-534`),
    /// so "every discovered state" and "every reachable state" are the same set here
    /// and the invariant holds of the model, not merely of a prefix of it.
    Holds {
        /// How many reachable states it was evaluated at.
        states: usize,
    },
    /// A reachable state falsifies the invariant.
    ///
    /// A refutation, whether or not the exploration closed: the state was genuinely
    /// reached by a genuine sequence of transitions, and a larger budget cannot
    /// un-reach it. What a larger budget can change is the *path*, which is why
    /// `evidence` is [`Evidence`] and not a [`Witness`].
    Violated {
        /// The shallowest falsifying state; ties go to the canonically least, which
        /// is the rule [`crate::witness::shortest`] uses to pick its endpoint.
        state: State,
        /// That state's breadth-first depth.
        depth: usize,
        /// How many explored states falsify the invariant in total — INV-007's
        /// obligation on a report that shows one of them ("names what was omitted").
        violations: usize,
        /// A shortest path to `state`, or the typed reason there is none.
        evidence: Evidence,
    },
    /// The claim was not decided, and this is why (INV-008).
    Inconclusive(Unresolved),
}

impl CheckOutcome {
    /// The assurance-result verdict this outcome is.
    #[must_use]
    pub const fn verdict(&self) -> Verdict {
        match self {
            Self::Holds { .. } => Verdict::Established,
            Self::Violated { .. } => Verdict::Refuted,
            Self::Inconclusive(_) => Verdict::Inconclusive,
        }
    }
}

impl fmt::Display for CheckOutcome {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Holds { states } => write!(f, "established states={states}"),
            Self::Violated {
                state,
                depth,
                violations,
                ..
            } => write!(
                f,
                "refuted state={state} depth={depth} violations={violations}"
            ),
            Self::Inconclusive(reason) => write!(f, "inconclusive {reason}"),
        }
    }
}

/// One upheld invariant and what became of it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvariantResult {
    index: usize,
    name: Ident,
    outcome: CheckOutcome,
}

impl InvariantResult {
    /// The predicate's index in [`Model::predicates`].
    #[must_use]
    pub const fn index(&self) -> usize {
        self.index
    }

    /// The predicate's declared name.
    ///
    /// Carried beside the index for the reason [`crate::witness::Step`] carries an
    /// action's: a result is read by people and machines both, and neither should
    /// have to hold the model to make sense of it.
    #[must_use]
    pub const fn name(&self) -> &Ident {
        &self.name
    }

    /// What checking it established.
    #[must_use]
    pub const fn outcome(&self) -> &CheckOutcome {
        &self.outcome
    }
}

impl fmt::Display for InvariantResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invariant {} {} {}", self.index, self.name, self.outcome)?;
        if let CheckOutcome::Violated { evidence, .. } = &self.outcome {
            write!(f, "\n  {evidence}")?;
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// deadlock
// ---------------------------------------------------------------------------

/// One reachable state with no enabled action, under a policy that calls that a
/// defect.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Deadlock {
    state: State,
    depth: usize,
    evidence: Evidence,
}

impl Deadlock {
    /// The terminal state.
    #[must_use]
    pub const fn state(&self) -> &State {
        &self.state
    }

    /// Its breadth-first depth.
    #[must_use]
    pub const fn depth(&self) -> usize {
        self.depth
    }

    /// A shortest path to it, or the typed reason there is none.
    #[must_use]
    pub const fn evidence(&self) -> &Evidence {
        &self.evidence
    }
}

impl fmt::Display for Deadlock {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "deadlock state={} depth={}\n    {}",
            self.state, self.depth, self.evidence
        )
    }
}

/// What the declared policy made of the terminal states an exploration found.
///
/// Four arms rather than three, because "no state is a deadlock" and "no state was
/// asked to be" are different sentences and only one of them is a claim.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeadlockOutcome {
    /// The policy is [`DeadlockPolicy::Allowed`]: no claim was made, so a bound
    /// cannot weaken it and there is nothing to witness. The count is a fact about
    /// the explored states, offered because INV-007 asks a bounded report to say what
    /// it left out — not a verdict about any of them.
    NotJudged {
        /// How many explored states have no enabled action.
        terminal: usize,
    },
    /// The policy is [`DeadlockPolicy::Defect`], the exploration closed, and no
    /// reachable state is terminal.
    Free {
        /// How many reachable states were examined.
        states: usize,
    },
    /// The policy is [`DeadlockPolicy::Defect`] and terminal states were found.
    ///
    /// **All of them**, in ascending state order, each with a path — unlike
    /// [`CheckOutcome::Violated`], which reports the shallowest falsifying state and
    /// counts the rest. The difference is what the two answers are for: an invariant
    /// violation is answered by *the shortest counterexample*, which is one path by
    /// definition, while a deadlock report is a report about a *set* — docs/27's
    /// Dining Philosophers result names the deadlock by which state it is ("every
    /// philosopher holding its left fork",
    /// `notes/plan/docs/27_SPIKE_FINDINGS_REV2.md:41`), and a model may have several
    /// that are not variations of one.
    Deadlocked {
        /// The terminal states, ascending.
        states: Vec<Deadlock>,
    },
    /// The policy is [`DeadlockPolicy::Defect`], the exploration stopped at a bound,
    /// and no *explored* state is terminal — which is not deadlock freedom (INV-008).
    Inconclusive(Unresolved),
}

impl DeadlockOutcome {
    /// The assurance-result verdict this outcome is.
    ///
    /// [`DeadlockOutcome::NotJudged`] is [`Verdict::Established`] the way an empty
    /// conjunction is true: the caller declared that terminal states are legitimate,
    /// and every one that was found is. It establishes nothing about deadlock, which
    /// is why the arm exists rather than reusing [`DeadlockOutcome::Free`].
    #[must_use]
    pub const fn verdict(&self) -> Verdict {
        match self {
            Self::NotJudged { .. } | Self::Free { .. } => Verdict::Established,
            Self::Deadlocked { .. } => Verdict::Refuted,
            Self::Inconclusive(_) => Verdict::Inconclusive,
        }
    }

    /// The reported deadlocks, which is empty for every arm but one.
    #[must_use]
    pub fn deadlocks(&self) -> &[Deadlock] {
        match self {
            Self::Deadlocked { states } => states,
            Self::NotJudged { .. } | Self::Free { .. } | Self::Inconclusive(_) => &[],
        }
    }
}

impl fmt::Display for DeadlockOutcome {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotJudged { terminal } => write!(f, "deadlock not-judged terminal={terminal}"),
            Self::Free { states } => write!(f, "deadlock established states={states}"),
            Self::Deadlocked { states } => {
                write!(f, "deadlock refuted count={}", states.len())?;
                for found in states {
                    write!(f, "\n  {found}")?;
                }
                Ok(())
            }
            Self::Inconclusive(reason) => write!(f, "deadlock inconclusive {reason}"),
        }
    }
}

// ---------------------------------------------------------------------------
// the report
// ---------------------------------------------------------------------------

/// The assurance-result verdict vocabulary
/// (`notes/plan/schemas/assurance-result.schema.json:203-210`).
///
/// Three members, closed, and deliberately *not* `continuum-kernel-core`'s
/// `Verdict`: that one is about a certificate's bytes and its third arm is
/// `Unsupported`, while this one is about a claim over a model and its third arm is
/// `Inconclusive`. The two are different questions with the same-shaped answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Verdict {
    /// The claim was decided in its favour, over the scope the report names.
    Established,
    /// The claim was decided against: a reachable state refutes it.
    Refuted,
    /// The claim was not decided; the report carries the typed reason.
    Inconclusive,
}

impl Verdict {
    /// Every verdict, in the schema's listing order.
    pub const ALL: [Self; 3] = [Self::Established, Self::Refuted, Self::Inconclusive];

    /// The stable machine name, spelled as `assurance-result.schema.json` spells it.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Established => "established",
            Self::Refuted => "refuted",
            Self::Inconclusive => "inconclusive",
        }
    }
}

impl fmt::Display for Verdict {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Everything one check established, over one exploration.
///
/// A value: it borrows nothing, compares by content, and renders the same bytes every
/// time, because this is what a daemon task result carries and an artifact that
/// depends on when it was rendered is not an artifact (INV-005; docs/34's agent
/// output principles — "typed enums rather than prose classifications; canonical
/// ordering; stable IDs").
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckReport {
    scope: Scope,
    policy: DeadlockPolicy,
    invariants: Vec<InvariantResult>,
    deadlock: DeadlockOutcome,
}

impl CheckReport {
    /// How much of the model the answers are about.
    #[must_use]
    pub const fn scope(&self) -> Scope {
        self.scope
    }

    /// The deadlock policy the caller declared.
    #[must_use]
    pub const fn policy(&self) -> DeadlockPolicy {
        self.policy
    }

    /// One result per upheld invariant, ascending by predicate index.
    #[must_use]
    pub fn invariants(&self) -> &[InvariantResult] {
        &self.invariants
    }

    /// What the declared policy made of the terminal states.
    #[must_use]
    pub const fn deadlock(&self) -> &DeadlockOutcome {
        &self.deadlock
    }

    /// The verdict over every declared obligation at once.
    ///
    /// The fold has one rule and it is not "worst wins": **a refutation dominates**.
    /// A found counterexample is a fact that a raised bound cannot take back, and
    /// letting an inconclusive sibling mask it would be budget exhaustion behaving as
    /// a semantic answer, which is the one thing INV-008 and RFC 0026 both forbid.
    /// Otherwise any inconclusive obligation makes the whole report inconclusive, and
    /// only an all-established report is [`Verdict::Established`].
    ///
    /// **Vacuity is possible and is not hidden.** A report with no invariants under
    /// [`DeadlockPolicy::Allowed`] establishes nothing and returns
    /// [`Verdict::Established`], the way an empty conjunction is true. Nothing here
    /// can detect that a caller asked for nothing; what this type does instead is
    /// refuse to let the rendering hide it — [`Display`](fmt::Display) always prints
    /// the scope, the policy, and a line per invariant, so a vacuous report is
    /// visibly vacuous.
    #[must_use]
    pub fn verdict(&self) -> Verdict {
        let mut inconclusive = false;
        for result in &self.invariants {
            match result.outcome.verdict() {
                Verdict::Refuted => return Verdict::Refuted,
                Verdict::Inconclusive => inconclusive = true,
                Verdict::Established => {}
            }
        }
        match self.deadlock.verdict() {
            Verdict::Refuted => return Verdict::Refuted,
            Verdict::Inconclusive => inconclusive = true,
            Verdict::Established => {}
        }
        if inconclusive {
            Verdict::Inconclusive
        } else {
            Verdict::Established
        }
    }

    /// The result for one predicate index, or `None` when it was not upheld.
    #[must_use]
    pub fn invariant(&self, index: usize) -> Option<&InvariantResult> {
        self.invariants.iter().find(|result| result.index == index)
    }
}

impl fmt::Display for CheckReport {
    /// One result per line or indented group, in a fixed order: scope, policy, each
    /// invariant ascending, the deadlock outcome, the folded verdict.
    ///
    /// docs/34's terminal contract — "one result per line/group; stable rule IDs; no
    /// color-only meaning" (`notes/plan/docs/34_DEVELOPER_EXPERIENCE_PRODUCT_CONTRACT.md`,
    /// "Output principles"). The `--json` equivalent that contract also requires is
    /// the daemon's: this crate declares no serialization dependency, and inventing a
    /// second encoding of the same value here would be the duplicate-payload the
    /// agent half of that contract forbids.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "scope {}", self.scope)?;
        writeln!(f, "policy deadlock={}", self.policy)?;
        for result in &self.invariants {
            writeln!(f, "{result}")?;
        }
        writeln!(f, "{}", self.deadlock)?;
        write!(f, "verdict {}", self.verdict())
    }
}

// ---------------------------------------------------------------------------
// why a check could not be run at all
// ---------------------------------------------------------------------------

/// Why a check was refused before it began.
///
/// Distinct from [`Unresolved`], and the distinction is `continuum-kernel-core`'s
/// between a `Rejected` verdict and an inconclusive result: this is a statement
/// about the *request*, never about the model. It is returned before any state is
/// examined, so a refused check leaves no partial report — the same discipline
/// `crate::certificate` applies to a refused emission, which never leaves a partial
/// artifact.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckError {
    /// An obligation names a predicate index the model does not declare.
    UnknownPredicate {
        /// The index that was asked for.
        index: usize,
        /// How many predicates the model declares.
        declared: usize,
    },
}

impl fmt::Display for CheckError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            Self::UnknownPredicate { index, declared } => write!(
                f,
                "invariant names predicate {index}; the model declares {declared}"
            ),
        }
    }
}

impl core::error::Error for CheckError {}

// ---------------------------------------------------------------------------
// the check
// ---------------------------------------------------------------------------

/// Check every declared invariant, and the deadlock question, over one exploration.
///
/// Deterministic: the returned value is a function of `model`, `exploration` and
/// `obligations` and of nothing else.
///
/// `exploration` must be an exploration of `model`. Nothing here can check that in
/// general — a [`Reachable`] carries no model identity — and the mismatches that
/// *can* be caught surface as data: a state of the wrong arity or outside a declared
/// domain is [`Unresolved::EngineError`], and a discovery chain naming an action the
/// model does not declare is [`NoWitness::ForeignExploration`] inside an
/// [`Evidence::Unwitnessed`].
///
/// # Errors
///
/// [`CheckError::UnknownPredicate`] when an obligation names a predicate the model
/// does not declare. Every index is validated before any state is examined, so this
/// is the only way a check produces no report.
pub fn check(
    model: &Model,
    exploration: &Exploration,
    obligations: &Obligations,
) -> Result<CheckReport, CheckError> {
    let declared = model.predicates().len();
    for index in &obligations.invariants {
        if model.predicates().get(*index).is_none() {
            return Err(CheckError::UnknownPredicate {
                index: *index,
                declared,
            });
        }
    }

    let scope = Scope::of(exploration);
    let reachable = exploration.reachable();

    let mut invariants: Vec<InvariantResult> = Vec::with_capacity(obligations.invariants.len());
    for index in &obligations.invariants {
        let Some(predicate) = model.predicates().get(*index) else {
            return Err(CheckError::UnknownPredicate {
                index: *index,
                declared,
            });
        };
        invariants.push(InvariantResult {
            index: *index,
            name: predicate.name().clone(),
            outcome: one_invariant(model, exploration, reachable, scope, *index),
        });
    }

    Ok(CheckReport {
        scope,
        policy: obligations.deadlock,
        invariants,
        deadlock: deadlock_outcome(model, exploration, reachable, scope, obligations.deadlock),
    })
}

/// The outcome for one already-validated predicate index.
fn one_invariant(
    model: &Model,
    exploration: &Exploration,
    reachable: &Reachable,
    scope: Scope,
    index: usize,
) -> CheckOutcome {
    let falsifying = match falsifying(model, reachable, index) {
        Ok(found) => found,
        Err(reason) => return CheckOutcome::Inconclusive(reason),
    };
    match falsifying.shallowest {
        Some((state, depth)) => CheckOutcome::Violated {
            state,
            depth,
            violations: falsifying.count,
            evidence: Evidence::of(witness::shortest(model, exploration, &Target::Fails(index))),
        },
        None => match scope.unresolved() {
            Some(reason) => CheckOutcome::Inconclusive(reason),
            None => CheckOutcome::Holds {
                states: scope.states(),
            },
        },
    }
}

/// Where an invariant fails: the shallowest such state, and how many there are.
struct Falsifying {
    shallowest: Option<(State, usize)>,
    count: usize,
}

/// Evaluate one predicate at every discovered state.
///
/// The scan runs over the ascending state table and only a *strictly* shallower state
/// displaces the incumbent, so ties go to the canonically least — the same rule
/// `witness::select` applies, which is why the state reported here and the endpoint
/// of the witness extracted for it are the same state whenever both exist.
/// `tests/checking_diehard.rs` asserts that agreement rather than assuming it.
fn falsifying(
    model: &Model,
    reachable: &Reachable,
    index: usize,
) -> Result<Falsifying, Unresolved> {
    let mut shallowest: Option<(State, usize)> = None;
    let mut count: usize = 0;
    for (state, depth) in reachable.states().iter().zip(reachable.depths().iter()) {
        let holds =
            model
                .evaluate_predicate(index, state)
                .map_err(|source| Unresolved::EngineError {
                    state: state.clone(),
                    source: Box::new(source),
                })?;
        if holds {
            continue;
        }
        count = count.saturating_add(1);
        let improves = match shallowest {
            Some((_, incumbent)) => *depth < incumbent,
            None => true,
        };
        if improves {
            shallowest = Some((state.clone(), *depth));
        }
    }
    Ok(Falsifying { shallowest, count })
}

/// The deadlock half of a report.
fn deadlock_outcome(
    model: &Model,
    exploration: &Exploration,
    reachable: &Reachable,
    scope: Scope,
    policy: DeadlockPolicy,
) -> DeadlockOutcome {
    let terminal = match terminal_states(model, reachable) {
        Ok(found) => found,
        Err(reason) => return DeadlockOutcome::Inconclusive(reason),
    };
    match policy {
        DeadlockPolicy::Allowed => DeadlockOutcome::NotJudged {
            terminal: terminal.len(),
        },
        DeadlockPolicy::Defect => {
            if terminal.is_empty() {
                return match scope.unresolved() {
                    Some(reason) => DeadlockOutcome::Inconclusive(reason),
                    None => DeadlockOutcome::Free {
                        states: scope.states(),
                    },
                };
            }
            DeadlockOutcome::Deadlocked {
                states: terminal
                    .into_iter()
                    .map(|(state, depth)| Deadlock {
                        evidence: Evidence::of(witness::shortest(
                            model,
                            exploration,
                            &Target::State(state.clone()),
                        )),
                        state,
                        depth,
                    })
                    .collect(),
            }
        }
    }
}

/// Every discovered state with no enabled action, ascending.
///
/// Terminality is a fact about one state and the model, not about the exploration:
/// [`Model::successors`] is empty exactly when no action is enabled, and "an enabled
/// action always has at least one successor"
/// (`crates/continuum-engine-reference/src/model.rs:444-452`), so the row is recomputed
/// for every discovered state rather than only for the expanded ones. That is what
/// makes a deadlock inside a bounded run reportable — the frontier states were never
/// expanded by the walk, but their rows are still computable — and it is the same
/// move `crate::certificate` makes when it recomputes rows as it writes them.
fn terminal_states(
    model: &Model,
    reachable: &Reachable,
) -> Result<Vec<(State, usize)>, Unresolved> {
    let mut found: Vec<(State, usize)> = Vec::new();
    for (state, depth) in reachable.states().iter().zip(reachable.depths().iter()) {
        let row = model
            .successors(state)
            .map_err(|source| Unresolved::EngineError {
                state: state.clone(),
                source: Box::new(source),
            })?;
        if row.is_empty() {
            found.push((state.clone(), *depth));
        }
    }
    Ok(found)
}
