//! `continuum-effects-time` — the time effect pack (plan §15, docs/17, PR 15).
//!
//! # Responsibility
//!
//! Declared time profiles: virtual time, timeouts, and clock observations, with
//! explicitly declared unsupported cases.
//!
//! INV-005 — no ambient nondeterminism: every timing choice is a declared, replayable
//! event.
//!
//! # What is here (PR-15 / IMPL-04, bn-1oj6)
//!
//! One fidelity profile, [`profile::UNMODELLED_V0`] — `time/unmodelled-v0` at version
//! 0.1.0 — declared as data, that models **no** clock semantic. PR 15 implements only
//! the profiles the replicated register needs, and the replicated register uses no
//! clock: the network, process and storage profiles hold every completion back with an
//! explicit `Delay` step that has no duration. Each of the seven clock semantics RFC
//! 0002 names has one row in [`profile::Semantic`], marked
//! [`profile::Support::Unsupported`] with its reason, and one declared case in
//! [`profile::UNSUPPORTED`]: the host behaviour — a monotonic reading, wall time, drift
//! and skew between nodes, leap seconds and a clock that goes backwards, uncertainty
//! intervals, leases, timers and late wakeups — how a caller could ask for it, and what
//! a program that depends on it gets.
//!
//! The pack has no operation, no step and no handler, so every case is
//! [`profile::Request::NoOperation`]: no caller can ask this pack for a clock. A program
//! that reads a clock by another route performs an ambient host effect, which INV-005
//! refuses and RFC 0002 reports as an `UnmodeledEffect`
//! ([`profile::Reliance::OutsidePack`]).
//!
//! # What is not here
//!
//! - **No host semantics** (docs/09 T06): [`profile::HostQualification::None`]. Nothing
//!   this crate produces may be cited as `platform-qualified`.
//! - **No virtual time, timeout or clock observation.** They land with the first slice
//!   that needs a clock, as a new profile version.
//!
//! # Dependency-boundary contract
//!
//! - A domain pack is declared data plus a normalized adapter; it owns no semantic
//!   state and may not import engines or `continuum-forge`.
//! - INV-008 — typed inconclusiveness: an unsupported case is declared, never silently
//!   approximated.
//!
//! This crate has no dependencies. `tools/check_crate_boundaries.py` enforces the
//! forbidden edges mechanically.
//!
//! # No host effects, by construction
//!
//! The crate is `#![no_std]`: it links only `core` and `alloc`, which have no clock,
//! filesystem, network, process or environment, and `unsafe_code` is forbidden
//! workspace-wide, so no FFI route exists either. `tests/pr15_no_std_lane.rs`, the
//! network pack's lane applied unchanged, runs the compiler on the dev and release host
//! builds: a copy of this crate with a planted `std::net` use fails with the
//! unresolved-`std` error. The same structure rules as the other packs make those the
//! only builds, and the lane checks rustc's dep-info and Cargo's resolved view,
//! `cargo metadata`, for no build-environment input, build script, dependency or
//! feature.

#![no_std]

extern crate alloc;

pub mod profile;

pub use profile::{
    FidelityClass, FidelityProfile, Reliance, Request, Semantic, Support, UNMODELLED_V0,
    UNSUPPORTED, UnsupportedCase,
};
