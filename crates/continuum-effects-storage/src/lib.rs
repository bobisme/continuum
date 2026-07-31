//! `continuum-effects-storage` — the storage effect pack (plan §15, docs/17, PR 15).
//!
//! # Responsibility
//!
//! Declared storage profiles: submit, stable, sync, and ack, with explicitly declared
//! unsupported cases.
//!
//! Durability semantics are the substrate for the ack-before-sync failure that drives
//! the Phase B loop.
//!
//! # Dependency-boundary contract
//!
//! - A domain pack is declared data plus a normalized adapter; it owns no semantic
//!   state and may not import engines or `continuum-forge`.
//! - INV-008 — typed inconclusiveness: an unsupported case is declared, never silently
//!   approximated.
//!
//! PR-1 / IMPL-01 scaffold: this crate declares its responsibility and its dependency
//! boundary. The types and behavior land in the PR named above.
//! `tools/check_crate_boundaries.py` enforces the forbidden edges mechanically.
