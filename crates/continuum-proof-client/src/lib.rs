//! `continuum-proof-client` — the Lean proof service client (plan §15, PR 28).
//!
//! # Responsibility
//!
//! Client for the proof service: per-request isolation and cancellation, goals,
//! diagnostics, relevant-context extraction, and receipts carrying theorem and axiom
//! manifests.
//!
//! The Lean toolchain and the proof service itself are foreign, isolated processes;
//! this crate speaks to them and validates what comes back.
//!
//! # Dependency-boundary contract
//!
//! - INV-014 — version-explicit proof: every receipt records the Lean epoch, toolchain
//!   identity, and axiom manifest, and zero unapproved axioms is a checked property,
//!   not a convention.
//! - Lean does not trust generated facts without a checker theorem (docs/01 §13); this
//!   crate never upgrades a claim on its own authority.
//!
//! PR-1 / IMPL-01 scaffold: this crate declares its responsibility and its dependency
//! boundary. The types and behavior land in the PR named above.
//! `tools/check_crate_boundaries.py` enforces the forbidden edges mechanically.
