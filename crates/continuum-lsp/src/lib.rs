//! `continuum-lsp` — the LSP authoring adapter (plan §17.1, PR 25).
//!
//! # Responsibility
//!
//! CML and source diagnostics, semantic hover, go-to-correspondence, code lenses, and
//! intent-change preview over snapshots.
//!
//! An unsaved buffer becomes an explicit snapshot; it never mutates daemon state
//! implicitly.
//!
//! # Dependency-boundary contract
//!
//! - **Adapters do not own semantic state** (`START_HERE_IMPLEMENTATION.md`, plan §20);
//!   overlays are snapshots, not hidden edits.
//! - UI crates cannot mutate evidence directly (plan §20).
//! - Adapters are sinks in the workspace graph: no Continuum crate may depend on this
//!   one.
//!
//! PR-1 / IMPL-01 scaffold: this crate declares its responsibility and its dependency
//! boundary. The types and behavior land in the PR named above.
//! `tools/check_crate_boundaries.py` enforces the forbidden edges mechanically.
