//! `continuum-benchmark` — ContinuumBench task families, metrics, and datasets (plan
//! §19, PR 30).
//!
//! # Responsibility
//!
//! Task families, metrics, dataset construction, and the reward-hacking suite used to
//! measure whether agents genuinely verify or merely appear to.
//!
//! Benchmarks test governance, not just bug fixing: a patch that games the intent must
//! score as a failure.
//!
//! # Dependency-boundary contract
//!
//! - May depend on Forge and on verifier interfaces — it is a harness, not part of the
//!   verifier.
//! - Owns no semantic state and publishes no trusted evidence of its own.
//!
//! PR-1 / IMPL-01 scaffold: this crate declares its responsibility and its dependency
//! boundary. The types and behavior land in the PR named above.
//! `tools/check_crate_boundaries.py` enforces the forbidden edges mechanically.
