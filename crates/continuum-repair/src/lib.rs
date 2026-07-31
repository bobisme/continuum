//! `continuum-repair` — repair transactions (plan §8, PR 20).
//!
//! # Responsibility
//!
//! begin / apply / evaluate / promote over a hypothesis, a patch identity, exact
//! replay, semantic and intent diff, accumulated evidence, and a policy verdict.
//!
//! A repair is a transaction with gates, not an edit: a property-weakening patch is
//! reclassified and blocked rather than merged.
//!
//! # Dependency-boundary contract
//!
//! - INV-011 — intent-preserving repair.
//! - May not import adapters or `continuum-forge`; Forge consumes repair and verifier
//!   interfaces, never the reverse.
//!
//! PR-1 / IMPL-01 scaffold: this crate declares its responsibility and its dependency
//! boundary. The types and behavior land in the PR named above.
//! `tools/check_crate_boundaries.py` enforces the forbidden edges mechanically.
