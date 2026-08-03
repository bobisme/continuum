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
//! # What this bone (bn-3tz60) delivers
//!
//! The PR-13 START_HERE line names three command groups; this is the third: `context
//! expand`, and `task status`/`task resume`/`task cancel`. The other two groups —
//! `snapshot`/`check`/`explain` (bn-3rqvm) and `debug`/`repair`/`evidence show` (bn-1g7e4)
//! — and the cross-cutting output contract (bn-ybh1z) are separate bones and had not landed
//! when this one did; [`cli`] therefore knows only the `context` and `task` verb groups, and
//! a sibling group is expected to add its own arm to the same dispatcher rather than replace
//! it.
//!
//! - [`context`] — `context expand` follows a PR-11 expansion handle
//!   (`continuumd::protocol::operations::context::ContextExpandRequest`) and renders the
//!   INV-007 omission manifest alongside the expanded slice, non-suppressibly. The wire
//!   request/response shapes and the `context.expand` registry entry exist
//!   (`continuumd::protocol::registry`), but no `OperationFamily` serves the `context`
//!   namespace yet (PR-11/IMPL-04, bn-28jj, is open) and `context.expand` has no
//!   [`continuumd::daemon::family::Arguments`] variant — so every call this module makes
//!   reaches a real daemon over a real frame and is refused
//!   `UnsupportedSemanticFeature` (`continuumd::protocol::vocabulary::ErrorCode`), honestly,
//!   rather than a fabricated success. The success-path renderer is implemented and
//!   unit-tested against the real wire types directly; it cannot be exercised over a live
//!   round trip until bn-28jj lands.
//! - [`task`] — `task status`, `task resume`, `task cancel` drive the PR-6 lifecycle
//!   through `continuumd`'s real `task.status`/`task.resume`/`task.cancel` operations,
//!   which are fully implemented and wire-tested here against an in-process daemon.
//!
//! # Dependency-boundary contract
//!
//! - **Adapters do not own semantic state** (`START_HERE_IMPLEMENTATION.md`, plan §20).
//!   Every fact is fetched from `continuumd` and every mutation is a protocol request;
//!   the CLI holds no cache that could disagree with the authority. [`wire::Connection`]
//!   holds only the negotiated version and the principal it speaks as — the same two
//!   facts `continuum_mcp::AgentClient` holds, and for the same reason.
//! - UI crates cannot mutate evidence directly (plan §20).
//! - Adapters are sinks in the workspace graph: no Continuum crate may depend on this
//!   one. `tools/check_crate_boundaries.py` enforces the forbidden edges mechanically.
//!
//! # What "over the wire" means here, honestly
//!
//! `continuumd` ships no socket or process transport yet — `transport::LocalPair` is
//! documented as "in-process, synchronous, and has no socket, no runtime, and no thread […]
//! not a stand-in for a real transport, it is the grain the rest of the daemon is built
//! at". There is therefore no running `continuumd` process this binary can dial today, and
//! `bin/continuum.rs` says so rather than pretending: it wires [`wire::Connection`] against
//! whatever [`wire::Transport`] its caller supplies, and no transport that reaches a real
//! deployment exists in this workspace yet. What is real and tested is everything above
//! that seam — argument parsing, the typed request a command builds, the real
//! `Daemon`/`Server`/`LocalPair` round trip an encoded frame makes, and the rendering of the
//! real typed answer — exactly as `continuum-mcp` and `continuum-benchmark` exercise the
//! same daemon in-process today (`continuum-mcp/tests/typed_surface.rs`,
//! `crates/continuum-benchmark/src/rig.rs`).

pub mod cli;
pub mod context;
pub mod error;
pub mod format;
pub mod render;
pub mod task;
pub mod wire;
