//! `continuum-task` — cancel-correct verification tasks (plan §4.5, PR 6).
//!
//! # Responsibility
//!
//! Task lifecycle, committed partial evidence, budget accounting,
//! suspension/continuation, drain and finalize behavior, and the guarantee of no orphan
//! workers.
//!
//! Publication is two-phase: reserve identity, stream provisional evidence, validate
//! references, commit. Cancellation before commit leaves no authoritative artifact.
//!
//! The typed result a verification task returns lives here, in [`result`] — the PR-1
//! exit. A task's *result* is the one part of plan §4.5's responsibility that every
//! other PR-1 deliverable already constrains: it names the six epochs (PR-1 / IMPL-02),
//! it carries the nine-dimension assurance envelope and the typed INV-008 reasons
//! (PR-1 / IMPL-03), and its inconclusive case is the payload the claim-status lattice
//! (PR-1 / IMPL-06) says an `inconclusive` claim owes. Landing the result type before
//! the lifecycle is deliberate: it is what the exit condition of PR 1 is stated over —
//! "an unsupported empty task returns a valid machine result naming every epoch and no
//! misleading success flag" — and `continuumd` (PR 5) needs something to wrap in a
//! result envelope that is not a second definition of the same fields.
//!
//! Owning the *result* is not owning the *run*: the lifecycle states, cancellation,
//! drain and finalize, budget accounting and continuations remain PR 6, and the wire
//! envelope, its encodings and its golden traces remain PR 5.
//!
//! # Dependency-boundary contract
//!
//! - INV-009 — monotonic task evidence.
//! - Bet B18/B19 — interactive results carry budget semantics, and cancellation
//!   correctness applies to verification itself.
//! - Sits in the middle island with `continuum-evidence` and `continuumd`; may not
//!   import engines, adapters, or `continuum-forge`.
//! - Depends on `continuum-value` for the epoch and assurance vocabularies. That crate
//!   is the workspace's only declared leaf crate and holds the vocabulary precisely so
//!   that "the daemon, the CLI, the evidence graph, the context packs, the certificates
//!   and the corpus receipts all render the same nine dimensions, the same assurance
//!   ladder, and the same typed INV-008 inconclusive reasons"
//!   (`crates/continuum-value/src/lib.rs`). A result that restated them would be the
//!   second spelling that rule exists to prevent.
//! - Depends on `continuum-evidence` for the plan §11.4 claim-status lattice. The edge
//!   stays inside the middle island — START_HERE's "Dependency islands" places
//!   `continuum-task`, `continuum-evidence` and `continuumd` on one tier — and it runs
//!   one way: the evidence graph knows nothing of tasks. `claim_status.rs` asks for it
//!   outright, leaving the pairing of a status with its typed payload as a seam whose
//!   "payload types … land with PR-1 / IMPL-03 in `continuum-value` and are joined here
//!   at the PR-1 exit"; [`result::TaskOutcome::claim_status`] is that join.
//! - No edge to `continuum-workspace`: the result seed references no artifact handle,
//!   and plan §20 states no such edge. A missing edge is cheap to add later; a wrong
//!   edge is architectural debt.
//!
//! PR-1 / IMPL-01 scaffold: this crate declares its responsibility and its dependency
//! boundary. The lifecycle types and behavior land in the PR named above; [`result`]
//! landed with the PR-1 exit. `tools/check_crate_boundaries.py` enforces the forbidden
//! edges mechanically.

pub mod result;
