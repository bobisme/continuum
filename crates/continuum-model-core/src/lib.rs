//! `continuum-model-core` — the typed model core (docs/01 §4.1, plan §20).
//!
//! # Responsibility
//!
//! The programmatic transition model: states, actions, enabling conditions, invariants,
//! and the semantic model identity that every front end must agree on.
//!
//! This is the stable semantic boundary the CML front end, the engines, and the
//! correspondence machinery all target.
//!
//! # Dependency-boundary contract
//!
//! - **The model core does not depend on `continuum-asupersync`** (plan §20, docs/01
//!   §13, `START_HERE_IMPLEMENTATION.md`). The model is a mathematical object; the
//!   concurrency runtime is an observed subject, never a dependency of the thing that
//!   defines meaning.
//! - No engine, adapter, or Forge dependency: search consumes the model core, not the
//!   other way round.
//!
//! PR-1 / IMPL-01 scaffold: this crate declares its responsibility and its dependency
//! boundary. The types and behavior land in the PR named above.
//! `tools/check_crate_boundaries.py` enforces the forbidden edges mechanically.
