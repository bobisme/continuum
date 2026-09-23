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
//! - This crate depends on `continuum-task` (the region calculus it is held to), on
//!   `continuum-value` (the ADR-0013 digest seam), and on `asupersync` itself (pinned
//!   `=0.5.0` with `deterministic-mode` only; `tools/governance/dependency-rationale.toml`
//!   gives the reasons and `dependency-audits.toml` records the audit of its tree).
//!
//! # What has landed (PR-14-IMPL-01, bn-lf4i)
//!
//! The substrate-independent half of the journal:
//!
//! - [`journal`] — the append-only [`journal::Journal`], its canonical encoding
//!   ([`encoding`]) with a decoder that refuses every second spelling, and its BLAKE3
//!   digest;
//! - [`family`] — the six PR-14 event families with stable tags, and **the extension
//!   point**: each sibling bullet (IMPL-02…06) lands by editing only its own
//!   `src/family/<name>.rs`, whose event and report types are uninhabited until then;
//! - [`family::lifecycle`] — the task/region lifecycle family, whose events are the
//!   region calculus's operations reported as facts;
//! - [`choice`] — the controlled choice log, a plain Continuum type;
//! - [`source`] — a scripted source that records per-actor scripts under a choice log,
//!   so "identical choice logs give identical events" is testable with no substrate;
//! - [`lift`] — the journal replayed into `continuum_task::region::RegionTree`, with a
//!   three-way verdict: conforms, violates at a sequence number, or inconclusive;
//! - [`binding`] — the substrate binding for the lifecycle family: asupersync's lab
//!   runtime driven under a [`choice::ChoiceLog`], with the journal observed from the
//!   substrate's own trace. The PR-14 exit sentence holds at this grain too, and the
//!   substrate's journal is byte-equal to the scripted source's at every interleaving
//!   the tests enumerate.
//!
//! # What is not here
//!
//! The other five families' bindings: [`binding::substrate_binding`] answers each of
//! them with the typed absence [`binding::BindingAbsence::FamilyNotBound`].
//! `tools/check_crate_boundaries.py` enforces the forbidden edges mechanically.

pub mod binding;
pub mod choice;
pub mod encoding;
pub mod family;
pub mod journal;
pub mod lift;
pub mod source;
