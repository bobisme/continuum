//! `continuum-forge` — Continuum Forge — intent-constrained synthesis (plan §14, PR
//! 29).
//!
//! # Responsibility
//!
//! Typed holes, finite grammar enumeration, exact counterexample feedback, safety plus
//! progress and non-vacuity checks, co-synthesis, and the quality-diversity candidate
//! archive.
//!
//! Forge is an untrusted search procedure whose output is verified like any other
//! candidate (INV-012 — non-vacuous synthesis).
//!
//! # Dependency-boundary contract
//!
//! - **Forge may not be imported by the verifier** (`START_HERE_IMPLEMENTATION.md`;
//!   plan §20 "Forge depends on verifier interfaces, never vice versa"). Forge sits at
//!   the bottom of the dependency islands: it consumes verification, context, debugger,
//!   and repair interfaces and is consumed only by `continuumd`, `continuum-cli`, and
//!   `continuum-benchmark`.
//! - The Forge/LLM candidate generator and the verifier and proof pipeline that judge
//!   it must not share decision logic (docs/33).
//!
//! `tools/check_crate_boundaries.py` enforces the forbidden edges mechanically.
//!
//! # What has landed, and what has not
//!
//! One lane: [`task`], the task-assembly lane. It is the caller with standing RFC 0037
//! AO2 names — "a lane that assembles a synthesis task against a contract MUST require
//! `optimization.non_vacuity` to be non-empty and MUST refuse the task otherwise, with
//! a typed refusal naming INV-012" — and landing it is what discharges that RFC's flag
//! F12, which recorded that the obligation was implemented in `continuum-intent` and
//! had no production caller (bn-1dsih).
//!
//! Everything else this crate's responsibility names is still unlanded: there is no
//! grammar, no enumeration, no counterexample loop, no candidate, no archive, and no
//! engine adapter. [`task`] decides whether a contract may *become* a synthesis task;
//! it runs no search, and this crate declares no dependency on any engine, solver,
//! effect crate, or protocol surface. PR 29 (`notes/plan/notes/START_HERE_IMPLEMENTATION.md`,
//! "Forge finite CEGIS v0") is where the search itself lands.
//!
//! # No wire surface, and no host authority
//!
//! This crate has no wire verb: `continuumd`'s operation registry declares no `forge`
//! namespace and no `continuumd` source names this crate, so nothing here is reachable
//! from an agent connection. That is a property of the workspace rather than a promise,
//! and `continuumd/tests/inv015_agent_least_authority_evidence.rs` sweeps it live —
//! together with this crate's whole public surface and its freedom from host effects —
//! so the first daemon caller, the first effect, or an unrecorded public item turns
//! that evidence red rather than widening INV-015's perimeter quietly.

pub mod task;
