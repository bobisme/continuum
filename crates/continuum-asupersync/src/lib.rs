//! `continuum-asupersync` — the asupersync adapter and semantic journal (docs/01 §6, PR
//! 14).
//!
//! # Responsibility
//!
//! Instrumentation of narrow asupersync primitives — task and region lifecycle,
//! reserve/commit/abort, cancellation phases, obligations, virtual time, and channel
//! communication — emitted as a canonical semantic journal.
//!
//! Identical controlled choice logs must produce byte-identical semantic events
//! (INV-005, INV-006).
//!
//! # Dependency-boundary contract
//!
//! - **`continuum-model-core` may not depend on this crate** (plan §20, docs/01 §13).
//!   The dependency runs one way: the journal is observed and lifted into model terms,
//!   never linked into the definition of meaning.
//! - Kernel crates may not depend on this crate either — the kernel is synchronous by
//!   covenant.
//!
//! PR-1 / IMPL-01 scaffold: this crate declares its responsibility and its dependency
//! boundary. The types and behavior land in the PR named above.
//! `tools/check_crate_boundaries.py` enforces the forbidden edges mechanically.
