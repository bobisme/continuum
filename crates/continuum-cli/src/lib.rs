//! `continuum-cli` — the human command-line adapter (plan §13.4, PR 13).
//!
//! # Responsibility
//!
//! Stable commands and output for `snapshot`, `check`, `explain`, `debug`, `repair`,
//! `evidence show`, `context expand`, and `task status/resume/cancel`.
//!
//! `--json` is the contract; prose is a projection of it (INV-003 — no prose-only
//! machine interfaces).
//!
//! # Dependency-boundary contract
//!
//! - **Adapters do not own semantic state** (`START_HERE_IMPLEMENTATION.md`, plan §20).
//!   Every fact is fetched from `continuumd` and every mutation is a protocol request;
//!   the CLI holds no cache that could disagree with the authority.
//! - UI crates cannot mutate evidence directly (plan §20).
//! - Adapters are sinks in the workspace graph: no Continuum crate may depend on this
//!   one.
//!
//! PR-1 / IMPL-01 scaffold: this crate declares its responsibility and its dependency
//! boundary. The types and behavior land in the PR named above.
//! `tools/check_crate_boundaries.py` enforces the forbidden edges mechanically.
