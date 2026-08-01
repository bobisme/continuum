//! `continuum-asupersync` — the asupersync adapter and semantic journal (docs/01 §6, PR
//! 14).
//!
//! # Responsibility
//!
//! Instrumentation of narrow asupersync primitives — task and region lifecycle,
//! reserve/commit/abort, cancellation phases, obligations, virtual time, and channel
//! communication — emitted as a canonical semantic journal.
//!
//! Identical controlled choice logs must produce byte-identical semantic events
//! (INV-005, INV-006).
//!
//! # Dependency-boundary contract
//!
//! - **`continuum-model-core` may not depend on this crate** (plan §20, docs/01 §13).
//!   The dependency runs one way: the journal is observed and lifted into model terms,
//!   never linked into the definition of meaning.
//! - Kernel crates may not depend on this crate either — the kernel is synchronous by
//!   covenant.
//!
//! # What this crate will be held to
//!
//! The region and task-lifecycle semantics this adapter's journal has to agree with are
//! already written down, in `continuum_task::region` (bn-2gk, PR 6): the one-way region
//! lifecycle, RFC 0026's request → drain → finalize teardown, the six `TaskStatus`
//! states, the both-or-neither cancellation outcome, and an obligation ledger carrying
//! RFC 0001's own named `child-region quiescence` and `cancellation finalization`
//! resources. That module is a deterministic model with no runtime in it, which is what
//! ADR-0001's "Continuum remains capable of model-only execution without asupersync"
//! requires, and plan §21 is what puts it in Phase A while this crate waits for Phase B.
//!
//! The consequence for this crate is the useful one: when the substrate arrives, whether
//! its regions behave is a **conformance** question against an existing specification —
//! lift the journal into those states and check the same properties — rather than a
//! design question answered by whatever the runtime happens to do.
//!
//! PR-1 / IMPL-01 scaffold: this crate declares its responsibility and its dependency
//! boundary. The types and behavior land in the PR named above.
//! `tools/check_crate_boundaries.py` enforces the forbidden edges mechanically.
