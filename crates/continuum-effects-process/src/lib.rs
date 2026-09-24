//! `continuum-effects-process` — the process effect pack (plan §15, docs/17, PR 15).
//!
//! # Responsibility
//!
//! Declared process profiles: crash, restart, and epoch, with explicitly declared
//! unsupported cases.
//!
//! Crash windows and cancellation points must replay exactly.
//!
//! Only the profile the replicated-register slice needs is in scope; breadth is
//! deliberately deferred.
//!
//! # What is here (PR-15 / IMPL-02, bn-3mmf)
//!
//! Two fidelity profiles over the same rows, class `adversarial-envelope`: the frozen
//! [`profile::CRASH_RESTART_V0`] — `process/crash-restart-v0` at version 0.1.0, byte for
//! byte as published — and the current [`profile::CRASH_RESTART_V1`] —
//! `process/crash-restart-v1` at 1.0.0, which adds the declared unsupported cases and
//! their assumptions (bn-1oj6; a profile's name identifies its content, RFC 0002
//! correction 1) — and one Lab handler,
//! [`lab::Process`], that implements exactly the profile's modelled rows:
//!
//! | Modelled | Step | Event |
//! |---|---|---|
//! | fail-stop crash of one node's incarnation, under a crash budget | `Crash` | `Crashed` |
//! | restart in a new incarnation whose epoch is one more than the last | `Restart` | `Restarted` |
//! | an operation an incarnation starts and waits for | `Begin` | `Begun` |
//! | its completion, delivered only to that incarnation while it runs | `Complete` | `Completed` |
//! | a completion that arrives after that incarnation crashed, fenced | `Complete` | `Fenced` |
//! | a completion held back for one more step, with no duration | `Delay` | `Delayed` |
//!
//! Every other process semantic RFC 0002 names has a row in [`profile::Semantic`]
//! marked [`profile::Support::Unsupported`] with its reason. Three of them have a step
//! a caller can ask for — graceful cancellation, panic and power loss — and that step
//! is refused with [`Refusal::Unsupported`], never approximated. The other two —
//! partial cleanup and supervisor decisions — have no step at all; the profile row is
//! their declaration. The profile states five assumptions ([`profile::ASSUMPTIONS`]):
//! fail-stop, no eventual restart, no failure detection, power loss as crashes, and a
//! node-level lifecycle.
//!
//! # What a crash does to in-flight state
//!
//! The network pack deferred this to the process pack, and [`profile::COMPOSITION`]
//! states it. The crashed incarnation runs nothing more, not even a finalizer. An
//! operation it started is not undone: its ticket stays pending, and a late completion
//! is `Fenced` — journalled, never delivered to a later incarnation. Network envelopes
//! are untouched and stay in flight. What stable storage keeps is the storage pack's.
//!
//! # Declared unsupported cases (PR-15 / IMPL-04, bn-1oj6)
//!
//! [`profile::UNSUPPORTED`] is the machine-readable, versioned enumeration of what the
//! profile does not model: one [`profile::UnsupportedCase`] per unsupported row, part of
//! the profile's canonical bytes. Each names the host behaviour, the
//! [`profile::Request`] by which a caller could ask for it — a step or no operation —
//! and the [`profile::Reliance`] that says what a verdict gives a program that depends
//! on it: the `node-level-lifecycle` assumption, for graceful cancellation and panic;
//! the `power-loss-as-crashes` assumption; or a modelled row that already covers it
//! (a fail-stop crash at a step boundary covers the program's own partial cleanup, and
//! the adversary's restart choice covers node-level supervisor decisions). At the step
//! bound every step is refused as `BoundReached(Steps)` first. [`profile::UnsupportedCase::refusal`]
//! is the refusal a caller gets, and `tests/pr15_impl04_unsupported.rs` holds the Lab
//! handler to it.
//!
//! # What is not here
//!
//! - **No host semantics** (docs/09 T06). There is no production handler, no operating
//!   system process, and no measured host configuration:
//!   [`profile::HostQualification::None`]. Nothing this crate produces may be cited as
//!   `platform-qualified`.
//! - **No independence relation** (docs/09 T07): every pair of process events is
//!   dependent ([`profile::IndependenceClaim::AllDependent`]).
//! - **No substrate binding.** `continuum-asupersync` does not bind a process family,
//!   and this crate does not depend on it. Driving this pack from asupersync regions is
//!   the replicated-register implementation's work (PR 16).
//! - **No storage semantics.** What survives a crash on stable storage belongs to the
//!   storage pack.
//!
//! # Determinism
//!
//! Every choice, each crash and restart included, is an explicit [`Step`] in a choice
//! log, and every accepted step is one [`Event`]. [`run`] is a pure function of a
//! configuration and a log, a [`lab::Journal`] is its own choice log
//! ([`lab::Journal::choice_log`]), and [`lab::Journal::encode`] is canonical, so replay
//! is exact to the byte, and so is every crash window. Graceful cancellation is not
//! modelled here, so this pack's own evidence is crash windows and step-boundary
//! truncation; cancellation points proper are the runtime's (PR 14).
//!
//! # Dependency-boundary contract
//!
//! - A domain pack is declared data plus a normalized adapter; it owns no semantic
//!   state and may not import engines or `continuum-forge`. The Lab handler's state is
//!   the pack's own declared lifecycle state, not the program's.
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

pub use lab::{Journal, Process, run};
pub use profile::{
    CRASH_RESTART_V0, CRASH_RESTART_V1, FidelityClass, FidelityProfile, PROFILES, Reliance,
    Request, Semantic, Support, UNSUPPORTED, UnsupportedCase,
};
pub use refusal::{Refusal, RefusalClass, RunRefusal};
pub use step::{Epoch, Event, NodeId, NodeSet, ProcessConfig, Step, StepKind, TicketId};
