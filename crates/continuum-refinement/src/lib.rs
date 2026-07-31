//! `continuum-refinement` — model/program correspondence and refinement (plan §16, PR
//! 17).
//!
//! # Responsibility
//!
//! The correspondence graph, proof-oriented lenses, stuttering classification, bounded
//! refinement checking, and drift detection between a model and the code it claims to
//! describe.
//!
//! Correspondence laws are checked by a component independent of the semantic
//! synchronizer that proposes them (docs/33 "Architectural separation").
//!
//! # Dependency-boundary contract
//!
//! - Consumes `continuum-model-core` and `continuum-cir` interfaces when the
//!   implementation lands; never the reverse.
//! - No engine, adapter, or Forge dependency.
//!
//! PR-1 / IMPL-01 scaffold: this crate declares its responsibility and its dependency
//! boundary. The types and behavior land in the PR named above.
//! `tools/check_crate_boundaries.py` enforces the forbidden edges mechanically.
