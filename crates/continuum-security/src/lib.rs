//! `continuum-security` — capabilities, sandboxing, privacy, and audit (plan §18).
//!
//! # Responsibility
//!
//! Capability model, sandboxing of foreign tools and agent processes, context privacy,
//! audit trail, and signing identities.
//!
//! Security boundaries are semantic boundaries (bet B20): authority is separate from
//! handle possession, and least authority is the default.
//!
//! # Dependency-boundary contract
//!
//! - INV-015 — agent least authority.
//! - Policy and mechanism only: this crate owns no verification semantics and may not
//!   import engines, adapters, or `continuum-forge`.
//!
//! PR-1 / IMPL-01 scaffold: this crate declares its responsibility and its dependency
//! boundary. The types and behavior land in the PR named above.
//! `tools/check_crate_boundaries.py` enforces the forbidden edges mechanically.
