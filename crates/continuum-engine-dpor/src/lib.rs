//! `continuum-engine-dpor` — the partial-order reduction engine (docs/01 §7.2).
//!
//! # Responsibility
//!
//! Dynamic partial-order reduction with an explicit reduction witness for every class
//! of interleavings it declines to explore, for **finite safety** obligations:
//! invariants over reachable states and the deadlock question
//! (`notes/plan/rfcs/0004-exploration-dpor-and-unfoldings.md`, "Baseline
//! exploration"; claim C005 in `notes/plan/docs/18_CLAIMS_MATRIX.md`).
//!
//! Audited by an unreduced differential oracle plus a reduction-witness checker
//! (docs/33): [`check_witness`] is the checker, in [`checker`], and shares no
//! decision logic with the reducer; the oracle is `continuum-engine-reference`,
//! linked only by this crate's tests (`tests/c005_differential.rs`).
//!
//! # Dependency-boundary contract
//!
//! - Engines are untrusted producers: results reach the trust base only as certificates
//!   checked from wire form by `continuum-kernel-*`.
//! - **No `continuum-certificate` or `continuum-kernel-*` crate may depend on this
//!   crate** — experimental engines cannot enter the kernel dependency closure (docs/01
//!   §13).
//! - The one normal dependency is `continuum-model-core`. The reducer links no other
//!   engine, so the unreduced oracle that audits it shares none of its code.
//!
//! `tools/check_crate_boundaries.py` enforces the forbidden edges mechanically.
//!
//! # The algorithm
//!
//! RFC 0004's baseline is "conservative source-DPOR … It explores one or more
//! representatives per Mazurkiewicz class, depending on sleep-set precision", with
//! "stateless versus stateful" both supported. The models this engine checks are
//! finite state graphs that are routinely **cyclic** (a register that crashes and
//! recovers, a counter that wraps), and stateless DPOR explores maximal executions,
//! which a cycle makes infinite. This engine is therefore the stateful form:
//!
//! 1. **Labels and the conservative conflict relation** ([`footprint`]). Each action
//!    outcome is a label; footprints are derived from the model's expressions (guards
//!    included), never declared; two labels are dependent when either writes what the
//!    other reads or writes. Self-loop firings are not explored (they add no state).
//! 2. **Persistent (source) sets.** At each new state the reducer computes the
//!    smallest stubborn closure over the invisible enabled seeds — an enabled member
//!    brings every label it is dependent with, a disabled member brings every label
//!    that writes a variable it reads — and explores its enabled part. Such a set is
//!    persistent (Godefroid, LNCS 1032, 1996, ch. 4; Valmari's stubborn sets): no execution
//!    from the state that avoids the set can contain a label dependent with it.
//! 3. **Visibility (INV-013).** The variables the invariants read are visible, and
//!    so are those of every definedness predicate the check consults (RFC 0003:
//!    each invariant's chain `I#defined…`, and every action's chain, as
//!    `continuum_model_core::definedness::Definedness` classifies them, used as is).
//!    A reduced expansion never contains an enabled label that writes one, so every
//!    step the reduction postpones is invisible to the obligations. The soundness
//!    point for definedness: whether a state reads a value undefined is a function of
//!    those predicates' variables, so it is a function of the visible projection,
//!    and the reduced search reaches every reachable visible projection (below); a
//!    visible set without them would let a reduction postpone, forever, the step
//!    that makes a definedness predicate false (the `hidden-undefined` adversarial
//!    model, and seeded mutant 6). The dependence relation needs nothing more: a
//!    lowered action's own definedness is already conjoined into its guard, so its
//!    footprint reads it.
//! 4. **Sleep sets over visits.** The search graph's nodes are *visits*: a stored
//!    state with the sleep set it was entered with. Entering a state reuses a visit
//!    whose sleep set is a subset of the new one (it explores at least as much);
//!    otherwise a new visit is made. A label explored from a visit sleeps in the
//!    visit's later edges, and wakes in a child when a dependent label fires. A label
//!    is donated to the sleep set only once the strongly connected component its edge
//!    leads to is complete (Tarjan's algorithm, run online): a label whose edge leads
//!    back into an open component has not had its future explored yet, and letting it
//!    sleep makes the sleep justification circular. The C005 corpus found exactly
//!    that fault in this engine's first version, which reused Godefroid's
//!    state-caching rule for sleep sets unchanged; the rule is kept as a seeded mutant
//!    (`src/mutation.rs`).
//! 5. **The stack proviso.** An explored edge onto a visit on the search stack
//!    expands its source visit in full. The visit graph is searched depth first and
//!    each visit is expanded once, so every cycle of it contains such an edge, and
//!    every cycle therefore passes through a fully expanded visit (the ignoring
//!    proviso, C3).
//!
//! Under 2–5, every reachable deadlock and every reachable valuation of the visible
//! variables is reached by the reduced search, and so is an evaluation fault whenever
//! one is reachable. Each state is judged in the reference engine's order and
//! precedence (evaluation error, undefined action read, undefined read in the
//! invariant, violation), so the typed outcome "undefined read" is reported whenever
//! a stored state carries one, and a complete run stores a representative of every
//! reachable visible class; a bounded or faulted run reports what it found, and an
//! evaluation fault in the same obligation (or in an action's chain) ranks above it.
//! Like a violation, it names the first stored state, not how many states carry
//! it (the reference's count, for INV-007, is not reproduced). The search halts at the first fault it meets, as the reference
//! engine does, so what a run shows is that a reachable fault exists, not which one.
//! The argument is the literature's for persistent sets with visibility and the
//! cycle proviso (the ample-set conditions C0–C3 of Clarke, Grumberg and Peled,
//! *Model Checking*) and for sleep sets combined with persistent sets (Godefroid,
//! LNCS 1032, chs. 5–6), with the donation rule added so that no sleep justification
//! depends on a region still being explored. It is not machine-checked here: the
//! witness checker checks each premise per run, and the C005 differential checks the
//! conclusion against an unreduced oracle. That is the whole content of "preserves
//! finite safety verdicts" here: an invariant reads only visible variables, so its
//! verdict is a function of the reachable visible valuations, and the deadlock
//! verdict is a function of the reachable deadlocks.
//!
//! Not in this engine, and stated so no one reads it in: vector-clock happens-before
//! tracking and stateless replay (the literal source-DPOR of RFC 0004), wakeup trees
//! (optimal DPOR), observer-indexed dependence (RFC 0014), liveness (RFC 0004
//! "Fairness and liveness interaction": a safety reduction is never reused for
//! liveness — this crate has no liveness entry point), unfoldings, and distribution.
//!
//! # Typed results (INV-008)
//!
//! Every outcome is three-way with a typed reason ([`report`]). A search stopped by a
//! declared bound ([`Bounds`]) is [`Unresolved::ResourceExhausted`], never a pass; a
//! model that cannot be evaluated at a reachable state is
//! [`Unresolved::EngineError`]; a violation found before either is genuine and is
//! reported with its replayable [`Trace`]. Every unit of work an untrusted model can
//! cause is charged before it is done.

#![forbid(unsafe_code)]
// The no-panic covenant of the reference engine
// (`crates/continuum-engine-reference/src/lib.rs`), for the same reason: a reducer
// that panics on a malformed model cannot report the defect. Test modules opt out
// locally.
#![deny(
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::todo,
    clippy::unimplemented,
    clippy::arithmetic_side_effects
)]

mod bits;
mod budget;
pub mod checker;
mod engine;
pub mod footprint;
pub mod report;
pub mod witness;

#[cfg(test)]
mod mutation;
#[cfg(test)]
mod test_support;
// The test support shared with `tests/` names this crate `crate::dpor` in both
// places (`tests/support/differential.rs`).
#[cfg(test)]
use crate as dpor;

pub use checker::{CheckedInvariant, CheckedWitness, WitnessDefect, check_witness};
pub use footprint::{Conflict, DependenceEvidence, Footprint, Label};
pub use report::{
    Bound, Bounds, CheckError, Completeness, Deadlock, DeadlockOutcome, DeadlockPolicy,
    EngineFault, InvariantOutcome, InvariantResult, Obligations, Report, Stats, Trace, TraceStep,
    UndefinedRead, Unresolved, Verdict,
};
pub use witness::{EdgeRecord, Expansion, FullReason, NodeRecord, ReductionWitness, VisitRecord};

use continuum_model_core::model::Model;

/// The reduction algorithm and its version, recorded in every report (RFC 0004
/// "Outputs": "reduction algorithm/version").
pub const ALGORITHM: &str = "stateful-persistent-sleep-dpor/v0";

/// The work units [`check`] reserves per invariant before it reads the obligations:
/// validating and collecting the index, the per-invariant slots of the search, and
/// the invariant's result in the report (its name is copied: at most
/// `continuum_model_core::ident::MAX_IDENT_BYTES` bytes).
pub const OBLIGATION_UNITS: u64 = 32 + 128;

/// The dependence oracle and its version (RFC 0004 "Outputs": "dependence
/// oracle/version").
pub const DEPENDENCE_ORACLE: &str = "syntactic-footprint/v0";

/// Check `obligations` over `model` with the reduced search, within `bounds`.
///
/// Deterministic: the report is a function of the three arguments alone (INV-005).
///
/// # Errors
///
/// [`CheckError::UnknownPredicate`] when an obligation names a predicate the model
/// does not declare, and [`CheckError::WorkBelowObligations`] when `bounds` cannot
/// pay [`OBLIGATION_UNITS`] per invariant (checked first). Every other bound, and every
/// evaluation failure, is a typed outcome inside the [`Report`].
pub fn check(
    model: &Model,
    obligations: &Obligations,
    bounds: Bounds,
) -> Result<Report, CheckError> {
    engine::run(model, obligations, bounds, engine::Knobs::default())
}

/// RFC 0004's `dependence` question for two labels of `model`, from the syntactic
/// footprint oracle, within `work` units. `Unknown` is returned for a label outside
/// the model's table and when `work` does not cover deriving the footprints; either
/// way a consumer treats the pair as dependent.
#[must_use]
pub fn dependence(model: &Model, first: usize, second: usize, work: u64) -> DependenceEvidence {
    let mut meter = budget::Meter::new(work);
    match footprint::Footprints::derive(model, &[], &mut meter, true, true) {
        Ok(footprints) => footprints.dependence(first, second),
        Err(_) => DependenceEvidence::Unknown,
    }
}
