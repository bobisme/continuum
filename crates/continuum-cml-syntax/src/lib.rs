//! `continuum-cml-syntax` — CML surface syntax and parser (PR 15a, docs/11).
//!
//! # Responsibility
//!
//! Lexer, grammar, and concrete syntax tree for the CML Finite core fragment — enough
//! surface for the replicated register and the Wave 0 corpus ports.
//!
//! CML is a second front end, not a replacement: the programmatic model API stays
//! supported and both must elaborate to the same semantic model identity.
//!
//! # Dependency-boundary contract
//!
//! - INV-016 — source is untrusted data. Parsing may not carry authority, mutate
//!   semantic state, or trigger effects.
//! - Syntax only: elaboration lives in `continuum-cml-elab`, so this crate does not
//!   import the model core.
//!
//! PR-1 / IMPL-01 scaffold: this crate declares its responsibility and its dependency
//! boundary. The types and behavior land in the PR named above.
//! `tools/check_crate_boundaries.py` enforces the forbidden edges mechanically.
