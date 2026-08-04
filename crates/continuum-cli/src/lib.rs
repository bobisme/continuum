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
//! # What has landed here, bone by bone
//!
//! The PR-13 START_HERE line names three command groups. The third (bn-3tz60) landed first:
//! `context expand`, and `task status`/`task resume`/`task cancel`. The second (bn-1g7e4)
//! is the failure-investigation and promotion group: `debug open`/`debug state`,
//! `repair begin`/`repair review`, and `evidence show`. The first — the read/verify group
//! (bn-3rqvm) — is `snapshot create|fork|seal`, `check start|result|await`, and
//! `explain compile`. The cross-cutting output contract (bn-ybh1z) is a separate bone and had
//! not landed when this one did; [`cli`] therefore knows the `snapshot`, `check`, `explain`,
//! `context`, `task`, `debug`, `repair`, and `evidence` verb groups and reports every other
//! first word as a usage error.
//!
//! # Explicit handles everywhere (INV-002)
//!
//! No command in this crate has an implicit subject. There is no session file, no "current"
//! task, branch, transaction, or evidence node, and [`wire::Connection`] holds none: every
//! handle is a positional argument on every invocation. That is why `debug` is two verbs
//! rather than one — `debug open` prints the `dbg_*` branch in full and `debug state` takes
//! one back — instead of a single command that opens a branch and remembers it.
//!
//! # Honest depth (INV-008)
//!
//! Several of this crate's commands drive operations `continuumd` registers and does not yet
//! serve. They do not pretend otherwise and they do not fail opaquely: the frame is sent, the
//! daemon's own `UnsupportedSemanticFeature` comes back (`rule
//! errors.unsupported_surface`), and every format prints a typed [`render::Depth`] token
//! beside the exact operation name that is unsupported. The token is *derived* from the
//! daemon's answer, so no list in this crate has to be edited on the day a family lands —
//! see [`render::Depth`] for why that matters.
//!
//! - [`snapshot`] — `snapshot create`/`fork`/`seal` drive the PR-3 workspace family through
//!   `continuumd`'s real `workspace.create`/`workspace.fork`/`workspace.seal`, all three of
//!   which `continuumd::daemon::workspace::WorkspaceFamily` serves. Live end to end:
//!   `tests/snapshot_family.rs` creates a snapshot, seals it by the handle the create
//!   *printed*, and forks it, all through the command functions over real frames. The
//!   registry's fourth `workspace` operation, `workspace.diff`, is deliberately not a verb of
//!   this group — see [`snapshot`]'s module doc.
//! - [`check`] — `check start` submits a verification campaign (`verification.start`) and
//!   `check result`/`check await` read the verdict it earns (`verification.result`,
//!   `verification.await`). All three are served. `check start` performs exactly one call and
//!   prints the handle: there is no poll loop, because a client-side stopping rule would be a
//!   CLI-only behavior with no daemon counterpart, and a remembered task would be the ambient
//!   state INV-002 forbids. The verdict, its INV-008 `inconclusive_reason`, and the
//!   nine-dimension assurance envelope are read off the answer and re-derived nowhere.
//! - [`explain`] — `explain compile` asks for a Context Pack (`context.compile`) and renders
//!   the *projection* of the pack document: its schema-normative fields as `key  value` lines
//!   in the text formats, and the document itself embedded verbatim in machine output. The
//!   compiler is the one operation in a served namespace this daemon refuses, so the command
//!   reports a typed `unsupported` depth today (see above) and its projection is unit-tested
//!   against a conforming pack.
//! - [`context`] — `context expand` follows a PR-11 expansion handle
//!   (`continuumd::protocol::operations::context::ContextExpandRequest`) and renders the
//!   INV-007 omission manifest alongside the expanded slice, non-suppressibly. **Live as of
//!   bn-28jj** (PR-11/IMPL-04): `continuumd::daemon::context::ContextFamily` serves the
//!   `context` namespace and `context.expand` has an
//!   [`continuumd::daemon::family::Arguments`] variant, so the success path this crate
//!   shipped renderer-first now runs end to end against a real daemon over a real frame —
//!   `tests/context_expand.rs` drives it. What it renders is the child pack embedded
//!   verbatim and the child's own residual manifest, and a refusal is still a rendered
//!   answer rather than an error: `context.compile` remains typed-refused
//!   (`UnsupportedSemanticFeature`), because the pack *compiler* is PR-11's other bullets.
//! - [`task`] — `task status`, `task resume`, `task cancel` drive the PR-6 lifecycle
//!   through `continuumd`'s real `task.status`/`task.resume`/`task.cancel` operations,
//!   which are fully implemented and wire-tested here against an in-process daemon.
//! - [`evidence`] — `evidence show` reads one PR-7 evidence-graph node or edge by handle
//!   through the real `evidence.get`, which
//!   [`continuumd::daemon::evidence::EvidenceFamily`] serves. Live: `tests/evidence_show.rs`
//!   drives the success arm, the RFC 0027 X2 denial, the RFC 0026 `Redacted` stub, and the
//!   typed `unsupported` omission an `--inline` request earns, all against a real daemon.
//! - [`debug`] — `debug open` and `debug state` drive the PR-19 verification debugger.
//!   `continuumd` registers all eleven `debug` operations and serves none of them, so both
//!   report a typed `unsupported` depth today (see above) over a real round trip; their
//!   success renderers are unit-tested against the real wire types.
//! - [`repair`] — `repair begin` and `repair review` open and inspect a PR-20 repair
//!   transaction. Same standing as [`debug`]: registered, unserved, typed-unsupported, and
//!   renderer-proven ahead of the family.
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

pub mod check;
pub mod cli;
pub mod context;
pub mod debug;
pub mod error;
pub mod evidence;
pub mod explain;
pub mod format;
pub mod render;
pub mod repair;
pub mod snapshot;
pub mod task;
pub mod wire;
