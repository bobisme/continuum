//! `continuum-effects-storage` — the storage effect pack (plan §15, docs/17, PR 15).
//!
//! # Responsibility
//!
//! Declared storage profiles: submit, stable, sync, and ack, with explicitly declared
//! unsupported cases.
//!
//! Durability semantics are the substrate for the ack-before-sync failure that drives
//! the Phase B loop.
//!
//! Only the profile the replicated-register slice needs is in scope; breadth is
//! deliberately deferred.
//!
//! # What is here (PR-15 / IMPL-03, bn-2fk3)
//!
//! One fidelity profile, [`profile::APPEND_LOG_V0`] — `storage/append-log-v0` at
//! version 0.1.0, class `adversarial-envelope`, the name the replicated-register
//! scenario and Intent Contract already cite — and one Lab handler,
//! [`lab::Storage`], that implements exactly the profile's modelled rows:
//!
//! | Modelled | Step | Event |
//! |---|---|---|
//! | append a record to the node's log: visible at once, volatile | `Submit` | `Submitted` |
//! | open a sync ticket over the log's current length, a barrier | `Sync` | `SyncBegun` |
//! | the flush finishes: every record the ticket covers is stable | `Persist` | `Stable` |
//! | the completion reaches the incarnation that began it, only after `Stable` | `Ack` | `Acked` |
//! | a completion that arrives after that incarnation crashed, fenced | `Ack` | `Fenced` |
//! | a ticket held back for one more step, with no duration | `Delay` | `Delayed` |
//! | fail-stop crash: stable records kept, a prefix of the volatile suffix kept, the rest lost, the first lost record possibly torn | `Crash` | `Crashed` |
//! | restart in a new incarnation whose epoch is one more than the last | `Restart` | `Restarted` |
//! | recovery drops a torn tail | `Truncate` | `Truncated` |
//!
//! Every other storage semantic RFC 0002 and docs/17 §6 name has a row in
//! [`profile::Semantic`] marked [`profile::Support::Unsupported`] with its reason.
//! Three of them have a step a caller can ask for — sector corruption, write
//! reordering and a dishonest flush — and that step is refused with
//! [`Refusal::Unsupported`], never approximated. The other three — directory
//! durability, a device profile and an undetected tear — have no step at all; the
//! profile row is their declaration. A write over a torn tail is the program's fault,
//! the replicated register's M07, and is refused as [`Refusal::ProgramFault`], a
//! verdict against the program rather than an invalid choice log.
//! The profile states six assumptions ([`profile::ASSUMPTIONS`]).
//!
//! # Composition with the process pack
//!
//! The process pack deferred what stable storage keeps across a crash to this pack,
//! and [`profile::COMPOSITION`] states it. A composed run forwards each process-pack
//! `Crash` and `Restart` of a node to this pack at the same step boundary, and this
//! pack keeps the same epochs and fences a sync ticket by the same (node, epoch) rule.
//! Across a crash the node's log keeps every stable record intact and a prefix of its
//! volatile suffix, possibly ending in one torn record, and loses the rest.
//!
//! # What is not here
//!
//! - **No host semantics** (docs/09 T06). There is no production handler, no file, no
//!   device and no measured host configuration: [`profile::HostQualification::None`].
//!   Nothing this crate produces may be cited as `platform-qualified`.
//! - **No independence relation** (docs/09 T07): every pair of storage events is
//!   dependent ([`profile::IndependenceClaim::AllDependent`]).
//! - **No refinement relation** (RFC 0002 "Refinement obligation"). The profile states
//!   no concrete-to-abstract history relation; like the network and process packs, it
//!   is deferred to the replicated-register refinement work (PR 16), which relates
//!   `Acked` ranges to stable records through its own view.
//! - **No substrate binding.** `continuum-asupersync` does not bind a storage family,
//!   and this crate does not depend on it. Driving this pack from asupersync regions
//!   is the replicated-register implementation's work (PR 16).
//!
//! # Determinism
//!
//! Every choice, each crash's outcome included, is an explicit [`Step`] in a choice
//! log, and every accepted step is one [`Event`]. [`run`] is a pure function of a
//! configuration and a log, a [`lab::Journal`] is its own choice log
//! ([`lab::Journal::choice_log`]), and [`lab::Journal::encode`] is canonical, so replay
//! is exact to the byte, and so is every crash window.
//!
//! # Dependency-boundary contract
//!
//! - A domain pack is declared data plus a normalized adapter; it owns no semantic
//!   state and may not import engines or `continuum-forge`. The Lab handler's state is
//!   the pack's own declared storage state, not the program's.
//! - INV-008 — typed inconclusiveness: an unsupported case is declared, never silently
//!   approximated.
//!
//! This crate has no dependencies. `tools/check_crate_boundaries.py` enforces the
//! forbidden edges mechanically.
//!
//! # No host effects, by construction
//!
//! The crate is `#![no_std]`: it links only `core` and `alloc`, which have no
//! filesystem, network, process, environment or clock, and `unsafe_code` is forbidden
//! workspace-wide, so no FFI route exists either. `tests/pr15_no_std_lane.rs` runs the
//! compiler on the dev and release host builds: a copy of this crate with a planted
//! `std::net` use fails with the unresolved-`std` error. Structure rules, which the
//! lane, the INV-015 audit and the GOV-4-13 checker enforce, make those the only builds:
//! `#![no_std]` first, no `cfg`, no macro definitions, no `include!`, one `extern
//! crate`, which is `alloc`, no build script, no dependencies, no features. The rules
//! themselves are lexical; the claim rests on the compiler run they keep meaningful.
//! A std-less target is not in the pinned toolchain, so there is no second,
//! independent compiler witness.
//!
//! Nor does the crate read its build environment at compile time (cr-35ujnx): the
//! structure rules refuse `env!`, `option_env!`, `include!`, `include_str!`,
//! `include_bytes!`, `file!` and `cfg`, and the lane checks rustc's own dep-info for
//! the dev and release builds, which must name no `env-dep` variable and no file
//! outside `src/`. That there is no build script, `links` value, dependency or
//! feature is judged by Cargo's resolved view, `cargo metadata`, never by the
//! manifest's text.

#![no_std]

extern crate alloc;

pub mod lab;
pub mod profile;
pub mod refusal;
pub mod step;

pub use lab::{Journal, Storage, run};
pub use profile::{APPEND_LOG_V0, FidelityClass, FidelityProfile, Semantic, Support};
pub use refusal::{Refusal, RefusalClass, RunRefusal};
pub use step::{Entry, Epoch, Event, NodeId, NodeSet, Step, StorageConfig, TicketId, Value};
