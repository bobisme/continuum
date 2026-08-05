//! `continuum-mcp` — the minimal typed agent client (plan §10.4, §17.4; PR 10, PR 27).
//!
//! # Responsibility
//!
//! A curated projection of native operations for agents: explicit handles, deterministic
//! schemas and ordering, result bounds, and capability checks.
//!
//! Two subagents may share one workspace and intent while using isolated debugger and
//! proof handles, with no session coupling.
//!
//! # What PR 10 landed here, and why here
//!
//! START_HERE's PR 10 stem is "implement a minimal client exposing only typed operations".
//! Plan §20's crate list names exactly one agent-facing client surface — this one — and
//! the Phase A exit sentence names it as a third surface beside the native API and the
//! CLI: "Die Hard and Dining Philosophers can be checked through native API, CLI, and an
//! **agent client** with identical artifacts" (plan §21, Phase A). Plan §24.5's ACI row
//! then names "MCP-only surface" as the redesign this lane would fall back to, so the
//! surface G0-DX-10 grades is the surface in this crate.
//!
//! Two other §20 crates were considered and rejected on their own documentation.
//! `continuum-proof-client` is the **Lean** proof-service client (plan §15, PR 28) — its
//! receipts, axiom manifests and toolchain identity are a different subsystem, and putting
//! an ACI client there would have made its module doc untrue. `continuumd` owns the
//! protocol, and a client living inside the server would have had no byte boundary to
//! prove anything across.
//!
//! # What "only typed operations" means, concretely
//!
//! - **Every argument is a typed struct.** [`AgentClient`] never accepts a command string,
//!   a shell fragment, or free-form prose. Its lower edge, [`AgentClient::invoke`], takes
//!   `continuumd`'s own closed [`Arguments`] enum; its eight named operations take the
//!   semantic parameters those structs are built from.
//! - **Every answer is a typed value.** A success is an [`Admitted`] carrying
//!   `continuumd`'s closed [`Payload`] enum plus the envelope facts an agent needs — the
//!   task handle, the continuation, the verdict, the cost, and the INV-007 omission
//!   manifest. Nothing is parsed out of a rendered line.
//! - **Every refusal is a typed value.** A daemon refusal is a [`Refusal`] carrying the
//!   closed [`ErrorCode`], the `retryable` flag, the typed `recovery` operations the
//!   protocol offers, the continuation on a resumable failure, and the code's declared
//!   `Error.data` shape where one exists ([`RefusalData`], RFC 0026 F19) — a kernel's
//!   `CertificateRejection` reaches the caller with the kernel's own tokens intact, and
//!   `data` under a code this version declares no shape for is preserved verbatim rather
//!   than guessed at or dropped. An agent branches on an enum, never on a message.
//! - **Omission manifests surface** (INV-007). `ResultEnvelope.omissions` is `required` on
//!   the wire and is carried through to [`Admitted::omissions`] unabridged: a bounded
//!   answer names what it left out, inline, without a second call.
//! - **A precondition failure costs no bytes.** [`register`] is research/25's "semantic
//!   action grammar" as data: a labelled transition system over the operations this client
//!   exposes, with each operation's preconditions stated as typed
//!   [`Requirement`](register::Requirement)s. `invoke` consults it before the wire, and an
//!   operation whose preconditions do not hold is answered [`Outcome::Unmet`] locally —
//!   "invalid operations become low-cost local feedback" (research/25). It is still an
//!   *attempt*: a caller measuring validity counts it as one, which is why
//!   [`Answer::attempted`] is true on every arm including this one.
//!
//! # Dependency-boundary contract
//!
//! - **Adapters do not own semantic state** (`START_HERE_IMPLEMENTATION.md`, plan §20);
//!   MCP exposes `continuumd` operations and adds no authority of its own. This client
//!   holds *no* snapshot, task, or continuation handle of its own: every handle it uses is
//!   one the caller passed in on the call, and the grammar it validates against is
//!   [`AgentContext`](register::AgentContext) — the **caller's** state, supplied per call.
//!   What it does hold is interface accounting: the negotiated connection settings, the
//!   principal it speaks as, a monotone call counter, and a [`ByteLedger`]. None of those
//!   is a fact about a workspace, an intent, or a verification.
//! - INV-015 — agent least authority: the adapter may only narrow capabilities, never
//!   widen them. The principal travels in the envelope and is checked by the daemon below
//!   this layer; nothing here can grant what a capability does not carry.
//! - INV-005 — no ambient nondeterminism: no clock, no entropy, no thread. Request
//!   identifiers and idempotency keys are minted from the call counter, so the same script
//!   produces the same bytes on every run.
//! - Adapters are sinks in the workspace graph: no Continuum crate may depend on this one.
//!   `tools/check_crate_boundaries.py` enforces it mechanically, and the benchmark harness
//!   that grades this surface therefore links it as a **dev-dependency** — the one edge
//!   kind that checker reports rather than enforces, by its own documented design.
//!
//! # What this crate is not, yet
//!
//! It is not the MCP *transport*: there is no JSON-RPC framing, no tool manifest, and no
//! resource mapping here. Those are PR 27's, and inventing them now would have shipped a
//! second protocol beside the one RFC 0026 fixes. What PR 10 needs — and what G0-DX-10
//! grades — is the typed operation surface an agent calls, over the real wire, and that is
//! what is here.
//!
//! [`Arguments`]: continuumd::daemon::family::Arguments
//! [`Payload`]: continuumd::daemon::family::Payload
//! [`ErrorCode`]: continuumd::protocol::vocabulary::ErrorCode

pub mod answer;
pub mod client;
pub mod link;
pub mod register;

pub use answer::{
    Admitted, Answer, ByteLedger, CallBytes, ClientError, Outcome, Refusal, RefusalData,
};
pub use client::AgentClient;
pub use link::{LinkError, LocalLink, Transport};
pub use register::{AgentContext, CampaignState, Requirement, SnapshotState, Unmet};
