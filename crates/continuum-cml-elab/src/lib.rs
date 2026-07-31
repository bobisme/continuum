//! `continuum-cml-elab` — CML elaboration to the normalized semantic AST (PR 15a,
//! docs/11 §14).
//!
//! # Responsibility
//!
//! Elaborates parsed CML into the normalized semantic AST and, from there, into the
//! same semantic model identity a programmatic model produces.
//!
//! The normalized AST is frozen before surface syntax, so the deterministic formatter
//! and the migration tool (PR 15b) are defined over this crate's output, not over the
//! token stream.
//!
//! # Dependency-boundary contract
//!
//! - Depends on `continuum-cml-syntax` — the one workspace edge PR 15a states outright
//!   ("parser and elaborator").
//! - A front end, not an authority: it may not own semantic state, publish evidence, or
//!   import engines, adapters, or Forge.
//!
//! PR-1 / IMPL-01 scaffold: this crate declares its responsibility and its dependency
//! boundary. The types and behavior land in the PR named above.
//! `tools/check_crate_boundaries.py` enforces the forbidden edges mechanically.
