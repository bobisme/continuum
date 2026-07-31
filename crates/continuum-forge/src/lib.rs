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
//! PR-1 / IMPL-01 scaffold: this crate declares its responsibility and its dependency
//! boundary. The types and behavior land in the PR named above.
//! `tools/check_crate_boundaries.py` enforces the forbidden edges mechanically.
