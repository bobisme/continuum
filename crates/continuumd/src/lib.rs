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
//! `tools/check_crate_boundaries.py` enforces the forbidden edges mechanically.
//!
//! # What is here
//!
//! [`protocol`] — the native protocol's type layer: every plan §10.2 operation with its
//! request, response, verdict, and error declarations, the request and result envelopes,
//! the connection handshake and the protocol-major N/N−1 window, and the complete plan
//! §10.3 error taxonomy. The types are held to
//! `notes/plan/schemas/continuumd-native-protocol.idl` by `tests/idl_conformance.rs`,
//! which parses that file and fails closed on any disagreement.
//!
//! [`daemon`] — the operation layer over that type layer: a dispatch skeleton that enforces
//! the version window, RFC 0027's admission predicate, and the IDL's annotation obligations
//! *from registry data*, and the `workspace` and `intent` operation families wired to
//! `continuum-workspace` and `continuum-intent`. It is pure request-in/result-out over
//! explicit state — no runtime, no clock, no filesystem, no `async` — so it is testable
//! without I/O, which is what INV-005 and ADR-0003 ask of the daemon core.
//!
//! Transport and the byte-level canonical JSON/CBOR codec are not here yet; [`protocol`]'s
//! documentation states exactly where the type layer stops and why, and [`daemon`]'s states
//! how the operation layer is parameterized over the codec's absence.

pub mod daemon;
pub mod protocol;
