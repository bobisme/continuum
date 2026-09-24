//! `continuum-engine-reference` — the reference exploration path (docs/01 §7.1, PR 8).
//!
//! # Responsibility
//!
//! Deterministic breadth-first exploration with invariant and deadlock checking,
//! shortest witnesses, and finite closure certificates.
//!
//! The reference path is the differential oracle every optimized path is measured
//! against; it is written for obvious correctness, not speed.
//!
//! # Dependency-boundary contract
//!
//! - Engines are untrusted producers: results reach the trust base only as certificates
//!   checked from wire form by `continuum-kernel-*`.
//! - **No `continuum-certificate` or `continuum-kernel-*` crate may depend on this
//!   crate** — experimental engines cannot enter the kernel dependency closure (docs/01
//!   §13).
//! - One workspace dependency, `continuum-model-core`, and no external crate. The
//!   programmatic model ([`model`], [`ident`], [`domain`], [`expr`]) was written here
//!   for PR 8 and moved into the model core by bn-ybq (PR 15a), so that the CML
//!   elaborator can build the *same* model type without importing an engine. This
//!   crate re-exports the four modules under their old paths, so `crate::model::Model`
//!   and `continuum_engine_reference::model::Model` name the model core's type.
//!   Search consumes the model core; the model core imports nothing (plan §20).
//!   `continuum-value` was the one serious candidate for a further edge and was
//!   declined for a specific, recorded reason — see [`ident`]: its canonical name
//!   order is shortlex, while the certificate wire form this engine must satisfy
//!   orders names by bytes.
//!
//! `tools/check_crate_boundaries.py` enforces the forbidden edges mechanically.
//!
//! # What is here
//!
//! PR 8 has five deliverables
//! (`notes/plan/notes/START_HERE_IMPLEMENTATION.md:205-215`) and all five are here:
//!
//! - the **programmatic transition model** ([`model`], with [`ident`], [`domain`],
//!   [`expr`], [`diehard`]) — how a finite transition system is declared, what a state
//!   is, and how guards, updates, and named predicates are evaluated;
//! - **deterministic breadth-first exploration** ([`bfs`]) — the reachable set of a
//!   declared model, in canonical order, with each state's depth and the labelled
//!   transition that first reached it, under explicitly declared bounds;
//! - **invariant and deadlock checking** ([`checking`]) — a typed outcome per upheld
//!   invariant over an explored set, and what the caller's declared completion policy
//!   makes of the states with no enabled action;
//! - the **shortest witness** ([`witness`]) — the labelled path to a chosen state,
//!   read off the exploration's own discovery chain;
//! - the **finite closure certificate** ([`certificate`]) — a closed reachable set
//!   written as `CONTCERT` wire bytes, the only form in which anything this crate
//!   computes reaches the trusted checking base.
//!
//! Since bn-1ln12 it also answers **liveness under fairness** ([`liveness`]): `◇ P`
//! and `□◇ P` over a closed exploration, under the model's own weak and strong
//! fairness assumptions ([`fairness`]), by an Emerson–Lei fair-cycle search whose
//! counterexample is a lasso. It is the reference the liveness engine
//! (`continuum-engine-liveness`, Phase D) will be measured against; it emits no
//! certificate.
//!
//! It also carries the **tiny exhaustive oracle** of docs/19 §2 ([`semantic`], bn-1zgs):
//! a seeded generator of finite semantic systems (a [`model::Model`] plus footprints,
//! conflicts, obligations, cancellation phases and fairness), an oracle that enumerates
//! every configuration and bounded interleaving into a canonical artifact, and seeded
//! defects with a shrinker (RFC 0013, "Generated finite universes"). It is the
//! reference later reduction engines are checked against, and it uses only the
//! modules above.
//!
//! Each module was written to attach to the ones before it without changing them, and
//! did: the seam tables in [`model`] and [`bfs`] are the contracts that made that
//! possible, and they are kept because they record which primitive each answer comes
//! from.
//!
//! The division of labour between the last three is deliberate and is the crate's
//! central design decision. [`bfs`] surfaces an empty successor row and never calls it
//! a deadlock; [`witness`] produces a path to a chosen state and never says whether
//! reaching it is a defect; [`checking`] holds all of the policy and computes none of
//! the search. So one model can be checked under several policies without being
//! re-declared, and a witness to a violation, a witness to a deadlock, and a witness
//! to an ordinary state are one object produced by one code path.
//!
//! [`bfs`] searches; [`model`] does not, and the split is load-bearing. Exploration
//! reads the model layer through [`model::Model::initial_states`] and
//! [`model::Model::successors`] and through nothing else, so it has no private path to
//! an answer the model layer cannot also give. `tests/diehard_evidence.rs` walks the
//! model with its own nine-line closure and was written before [`bfs`] existed; it is
//! retained as the corroborating layer, and the two must agree on the frozen Die Hard
//! facts or one of them is wrong.
//!
//! # Determinism (INV-005)
//!
//! > Controlled code accesses scheduling, time, entropy, I/O, faults, and cancellation
//! > through explicit capabilities.
//! >
//! > — `notes/plan/plan.md:323-325`
//!
//! The model is data: named variables with declared finite domains, named actions whose
//! guards and updates are written in a closed expression language, and an explicit
//! enumeration of initial states. No part of it is a caller-supplied function, so "this
//! model reads nothing but its own state" is a property of the types rather than a
//! promise — the argument is spelled out in [`expr`]. Every observable sequence is
//! ordered by a total order on its own contents (`BTreeSet`, sorted vectors; no
//! hash-ordered collection appears in this crate), so two enumerations of one model on
//! one state are byte-identical.

#![forbid(unsafe_code)]
// The same no-panic covenant the trusted checking base holds itself to
// (`crates/continuum-kernel-core/src/lib.rs:96-104`), for a different reason: a
// reference oracle that panics on a malformed model declaration cannot report the
// defect it exists to find. Malformed input has to be a value on every path; test
// modules opt back out locally.
#![deny(
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::todo,
    clippy::unimplemented,
    clippy::arithmetic_side_effects
)]

pub mod bfs;
pub mod certificate;
pub mod checking;
pub mod diehard;
pub mod liveness;
pub mod semantic;
pub mod witness;

// The programmatic model, re-exported from the model core under the paths PR 8 gave it
// (and its fairness assumptions, bn-1ln12).
pub use continuum_model_core::{domain, expr, fairness, ident, model};

pub use bfs::{
    Bound, Bounds, Discovery, Exploration, ExplorationError, Partial, Reachable, explore,
};
pub use certificate::{
    ClaimEnvelope, ClosedSet, EmissionError, EnvelopeError, Field, emit_finite_closure,
    emit_invariant_closure,
};
pub use checking::{
    CheckError, CheckOutcome, CheckReport, Deadlock, DeadlockOutcome, DeadlockPolicy, Evidence,
    InvariantResult, Obligations, Scope, Unresolved, Verdict, check,
};
pub use domain::{Domain, DomainError, Variable};
pub use expr::{ArithOp, BoolExpr, CmpOp, EvalError, IntExpr};
pub use fairness::{Fairness, Strength};
pub use ident::{Ident, IdentError};
pub use liveness::{
    Cycle, Goal, Lasso, LivenessError, LivenessOutcome, Stuttering, check_liveness,
};
pub use model::{
    Action, ActionDecl, Assignment, EvaluationError, Model, ModelBuilder, ModelError, Outcome,
    Predicate, Site, State, Step, Symbol,
};
// `witness::Step` is deliberately *not* re-exported: `model::Step` already holds that
// name at the root, and a witness step and a successor-row step are different shapes.
// Name it through its module — `witness::Step` — and the two never collide.
pub use witness::{NoWitness, Target, Witness, shortest};
