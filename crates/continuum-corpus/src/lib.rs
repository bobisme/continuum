//! `continuum-corpus` — the TLA+ corpus and Tribunal harness inputs (plan §9.5, PR
//! 27a), and the cross-engine differential harness (docs/19 §1 and §5, bn-34mw).
//!
//! # Responsibility
//!
//! Corpus port manifests, per-family parity levels, and the pinned upstream revision
//! for the Wave 0/1 families.
//!
//! **Tribunal** refers exclusively to the TLA+ corpus oracle harness (plan §9.5); it is
//! not the incremental audit, and it is not the cross-engine differential harness
//! either (plan.review.2 on docs/19:12 asked for that separate name).
//!
//! Since bn-34mw this crate also carries [`differential`], the "cross-engine
//! differential harness" layer of docs/19 §1: engines that consume
//! `continuum-model-core` models are compared on normalized semantics, and a
//! disagreement halts the claims it bears on, emits a `defect_*` report (plan §4.7)
//! and minimizes its fixture (docs/19 §5: "Disagreement halts the relevant claim and
//! creates a minimized fixture"). It lives here because it is oracle tooling of the
//! same kind ADR-0029 keeps out of release artifacts, and this crate is the one no
//! release binary links.
//!
//! # Dependency-boundary contract
//!
//! - INV-018 — corpus honesty: a family's declared parity level is evidence-backed or
//!   it is not declared.
//! - Foreign TLA tools are Tribunal-only (docs/01 §13) and never ship in release
//!   binaries (ADR-0029).
//! - Oracle tooling is test-only: every workspace edge onto this crate is a
//!   `[dev-dependencies]` edge, and no package with a binary target reaches it through
//!   normal or build edges. `tests/oracle_tooling_is_test_only.rs` checks that over
//!   `cargo metadata` dependency kinds.
//! - The library links the model plane and `continuum-value` only. It never links an
//!   engine or the checking base; engines plug in through [`differential::Engine`] in
//!   test code.
//!
//! The Tribunal half is still the PR-1 / IMPL-01 scaffold: its types land in PR 27a.
//! `tools/check_crate_boundaries.py` enforces the forbidden edges mechanically.

#![forbid(unsafe_code)]
// A harness that panics on a malformed engine answer cannot report the defect it
// exists to find, so malformed input is a value on every path (the covenant
// `continuum-engine-reference` holds itself to). Test modules opt back out locally.
#![deny(
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::todo,
    clippy::unimplemented,
    clippy::arithmetic_side_effects
)]

pub mod differential;
