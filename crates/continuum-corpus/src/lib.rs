//! `continuum-corpus` — the TLA+ corpus and Tribunal harness inputs (plan §9.5, PR
//! 27a).
//!
//! # Responsibility
//!
//! Corpus port manifests, per-family parity levels, and the pinned upstream revision
//! for the Wave 0/1 families.
//!
//! **Tribunal** refers exclusively to the TLA+ corpus oracle harness (plan §9.5); it is
//! not the incremental audit.
//!
//! # Dependency-boundary contract
//!
//! - INV-018 — corpus honesty: a family's declared parity level is evidence-backed or
//!   it is not declared.
//! - Foreign TLA tools are Tribunal-only (docs/01 §13) and never ship in release
//!   binaries (ADR-0029).
//!
//! PR-1 / IMPL-01 scaffold: this crate declares its responsibility and its dependency
//! boundary. The types and behavior land in the PR named above.
//! `tools/check_crate_boundaries.py` enforces the forbidden edges mechanically.
