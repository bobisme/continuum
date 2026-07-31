//! `continuumd` — the authority daemon and the native protocol (plan §4.1, §4.3, §10,
//! PR 5).
//!
//! # Responsibility
//!
//! The workbench authority: it owns task admission, artifact publication, evidence
//! writes, and capability negotiation, and it defines the native protocol —
//! request/response types, transport, typed errors, and idempotency keys — for the plan
//! §10.2 operations.
//!
//! Bet B3: `continuumd` is the authority. Bet B4: explicit handles dominate implicit
//! sessions.
//!
//! # Dependency-boundary contract
//!
//! - **There is no `continuum-protocol` crate — the native protocol lives here**
//!   (`START_HERE_IMPLEMENTATION.md`, "Dependency islands"). Any future crate by that
//!   name is a boundary violation, and the boundary check rejects it.
//! - The daemon is outside the smallest trust base (docs/33): it orchestrates engines
//!   and services but never substitutes its own judgement for a kernel check.
//! - Adapters depend on `continuumd`; `continuumd` does not depend on adapters.
//!
//! PR-1 / IMPL-01 scaffold: this crate declares its responsibility and its dependency
//! boundary. The types and behavior land in the PR named above.
//! `tools/check_crate_boundaries.py` enforces the forbidden edges mechanically.
