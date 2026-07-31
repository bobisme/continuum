//! `continuum-observer` — the observer lattice (docs/01 §8, plan §5.2).
//!
//! # Responsibility
//!
//! Declared observation levels and event slices: what an intent is allowed to see, at
//! what granularity, and how observations compose.
//!
//! Observers are part of the protected question, which is why they are declared data
//! rather than an implementation detail of an engine.
//!
//! # Dependency-boundary contract
//!
//! - INV-002 — no hidden semantic state. An observer that is not declared does not
//!   exist.
//! - No engine, adapter, or Forge dependency.
//!
//! PR-1 / IMPL-01 scaffold: this crate declares its responsibility and its dependency
//! boundary. The types and behavior land in the PR named above.
//! `tools/check_crate_boundaries.py` enforces the forbidden edges mechanically.
