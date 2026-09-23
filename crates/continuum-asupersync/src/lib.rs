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
//! # What has landed (PR-14-IMPL-03, bn-bx7i)
//!
//! - [`family::cancellation`] — the cancellation phases family: each cancelled task's
//!   `requested → acknowledged → cancelled`, with its cause, lifted as a refinement of
//!   the region calculus's cancel → drain step;
//! - [`binding`] observes it when [`binding::BindingConfig::families`] names it, from
//!   the substrate's own trace, in a canonical order the lab seed does not reach.
//!
//! # What has landed (PR-14-IMPL-02, bn-gzy1)
//!
//! - [`family::effect`] — the reserve / commit / abort family: each reservation's
//!   `reserved → committed | aborted`, lifted into the region calculus's `Reserve` and
//!   `Commit` steps and its drain's discard, with every reservation resolved exactly
//!   once and none leaked at region close;
//! - [`binding`] observes it from asupersync's own obligation trace, for reservations a
//!   task makes through its `Cx`.
//!
//! # What has landed (PR-14-IMPL-04, bn-6nm8)
//!
//! - [`family::obligation`] — the obligations family: the substrate's ledger of every
//!   obligation kind — holder and region, discharge, transfer, leak, and each region's
//!   balance at close — lifted beside the region calculus's own ledger, so a region
//!   that closes with an open or leaked obligation is a violation even when the
//!   calculus's `is_total` holds;
//! - [`binding`] observes it from the substrate's trace and holds it to the substrate's
//!   own obligation records and obligation-leak oracle.
//!
//! # What has landed (PR-14-IMPL-05, bn-3m1d)
//!
//! - [`family::time`] — the virtual time family: timers scheduled, fired and cancelled,
//!   and the virtual clock's advances, from the lab's own clock and trace, lifted into a
//!   parallel checked clock-and-timer model tied to the region calculus's tasks;
//! - [`binding`] observes it with tied timers in canonical order, so neither the host
//!   clock nor the lab seed reaches the journal.
//!
//! # What has landed (PR-14-IMPL-06, bn-3xx9)
//!
//! - [`family::channel`] — the channel communication family: bounded `mpsc` channels'
//!   sends, receives, blocks, closes and drops, with message identity the delivered
//!   payload, lifted into a parallel checked FIFO model tied to the region calculus's
//!   tasks;
//! - [`binding`] observes it with woken tasks in canonical order. Every family is now
//!   bound.
//!
//! # What is not here
//!
//! A family a later PR adds starts unbound: [`binding::substrate_binding`] answers it
//! with the typed absence [`binding::BindingAbsence::FamilyNotBound`].
//! `tools/check_crate_boundaries.py` enforces the forbidden edges mechanically.

pub mod binding;
pub mod choice;
pub mod encoding;
pub mod family;
pub mod journal;
pub mod lift;
pub mod source;
