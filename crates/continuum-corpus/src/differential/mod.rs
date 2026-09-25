//! The cross-engine differential harness (docs/19 §1 and §5, bn-34mw).
//!
//! > Use independently implemented systems where semantics overlap: reference model
//! > evaluator versus optimized evaluator; exhaustive enumerator versus DPOR; …
//! > Disagreement halts the relevant claim and creates a minimized fixture.
//! >
//! > — `notes/plan/docs/19_TEST_STRATEGY.md` §5
//!
//! # Shape
//!
//! - A corpus of [`Fixture`]s: models held as declarations ([`fixture`]), from the
//!   seeded generator ([`generate`]) or read back from any [`Model`](continuum_model_core::model::Model).
//! - Engines behind one adapter trait, [`Engine`] ([`engine`]), each projecting its
//!   answer onto [`Normalized`] semantics ([`normal`]): a verdict per invariant and a
//!   reachable-state projection, with INV-008 typed inconclusive reasons.
//! - A lane table ([`claims::LANES`]) pairing a subject engine with an oracle and
//!   naming the docs/18 claims their agreement is evidence for.
//! - Comparison ([`compare`]) of normalized answers, with every counterexample checked
//!   by replay against the model ([`replay`]).
//! - On disagreement: delta-debugging minimization ([`minimize`]), a `defect_*`
//!   report ([`defect`]), and quarantine of the lane's claims (for an engine fault,
//!   of every lane the faulting slot serves), gated against the
//!   committed register and the claims registry ([`claims`]).
//! - The driver ([`run`]).
//!
//! # Engines on trunk
//!
//! Of the engine crates, only `continuum-engine-reference` has behaviour; the
//! explicit, DPOR, symbolic and liveness crates are documented scaffolds. The engines
//! compared today are the reference path, its tiny exhaustive oracle, the kernel's
//! independent evaluator (through wire-epoch-2 certificates), and this harness's own
//! closure oracle ([`closure`]). Their adapters are in `tests/support/engines.rs`,
//! over dev edges. The explicit and DPOR slots are in the lane table and report
//! `absent` until an adapter fills them.
//!
//! Foreign oracles are not adapters here: TLC and Apalache are not installed and are
//! Tribunal-only under ADR-0029. The asupersync Lab oracles judge program journals,
//! not models, and are compared with the obligation model by
//! `crates/continuum-asupersync/tests/a7_primitive_conformance.rs`; a Lab-report adapter
//! is a later lane, when a model-to-program projection exists. No SAT/SMT solver
//! answers a question about a `continuum-model-core` model today.
//!
//! This module is oracle tooling: it never ships (see the crate documentation).

pub mod claims;
pub mod closure;
pub mod compare;
pub mod defect;
pub mod engine;
pub mod fixture;
pub mod generate;
pub mod minimize;
pub mod normal;
pub mod replay;
pub mod run;

pub use claims::{ClaimState, GateFinding, LANES, Lane, Quarantine, QuarantineLedger, gate, halts};
pub use closure::ClosureOracle;
pub use compare::{Comparison, Decided, Disagreement, Side, Undecided, compare, compare_fields};
pub use defect::{ContractDefect, ContractFault, DefectReport, Epochs};
pub use engine::{Budget, Engine, EngineIdentity, Fields};
pub use fixture::{Element, Fixture, FixtureAction};
pub use minimize::{Minimality, MinimizeBudget, Minimized, ddmin};
pub use normal::{
    Inconclusive, InvariantVerdict, Normalized, Projection, Trace, UndefinedKind, UndefinedRead,
};
pub use replay::{ReplayFault, UndefinedFault, check_undefined, replay};
pub use run::{ContractFinding, Finding, Harness, HarnessError, LaneCounts, LaneStatus, RunReport};
