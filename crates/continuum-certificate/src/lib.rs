//! `continuum-certificate` — the caller-facing certificate surface: wire bytes in, a
//! kernel's own verdict out (plan §20).
//!
//! # Responsibility
//!
//! Finite-closure certificates, type certificates, SAT and SMT refutations, and
//! temporal witnesses reach a caller as one thing: a byte string an untrusted producer
//! wrote. This crate is the single entry point that turns such a byte string into the
//! verdict of the trusted checker that owns its family — and nothing else.
//!
//! [`check_certificate`] reads the artifact's leading eight-byte magic, routes to the
//! `continuum-kernel-*` crate whose wire contract that magic names, and returns that
//! kernel's [`Verdict`](continuum_kernel_core::Verdict) verbatim.
//!
//! # What plan §20 assigns, and what it does not
//!
//! §20's crate list places `continuum-certificate` immediately above the four
//! `continuum-kernel-*` crates, and its dependency rules say one thing about it:
//!
//! > proof/certificate checker does not depend on search engines;
//! >
//! > — `notes/plan/plan.md` §20, "Dependency rules"
//!
//! The direction of the remaining edge is fixed, not chosen. `KCOV-06`
//! (`tools/check_kernel_covenant.py`) admits *no* dependency for a kernel manifest —
//! `ADMITTED_KERNEL_DEPENDENCIES` is the empty set, because "their decoder, arithmetic
//! and state representation are their own (docs/03 §8 diversity)". A kernel crate
//! therefore cannot depend on this one, and the composition can only run this way:
//! this crate links the four kernels.
//!
//! That settles what is left for this crate to own. The *formats* are already owned,
//! one per family, by the kernel that checks them: `CONTCERT` by
//! `continuum-kernel-core`, `CONTSATC` by `continuum-kernel-sat`, `CONTSMTC` by
//! `continuum-kernel-smt`, `CONTTMPC` by `continuum-kernel-temporal`, each with its own
//! decoder that shares no code with any producer. Restating any of those grammars here
//! would create a second authority over a byte layout the kernel already owns, and
//! docs/03 §8 makes the duplicate worse than useless — a checker that decodes through
//! someone else's reader has given up the independence the reader exists to provide.
//!
//! So this crate is deliberately thin, and the thinness is the design: it owns
//! **dispatch, not semantics**. It answers "which trusted checker owns these bytes?"
//! and then gets out of the way. No obligation is evaluated here, no byte is
//! reinterpreted here, and no verdict is minted here.
//!
//! # INV-004, structurally
//!
//! > Search code does not check its own strongest claims. Certificates cross an
//! > independent checker; foundational theorems cross Lean.
//! >
//! > — `notes/plan/plan.md:319`, INV-004 "No self-certification"
//!
//! Two mechanical halves, both visible from this file:
//!
//! - **This crate cannot link search.** No `continuum-engine-*`, no `continuum-forge`,
//!   no `continuum-asupersync`, directly or transitively.
//!   `tools/check_crate_boundaries.py`'s `certificate-checker-not-search` rule walks the
//!   real `cargo metadata` closure of this crate and the four kernels, and its
//!   self-test injects `continuum-certificate -> continuum-engine-explicit` and a
//!   transitive variant through `continuum-value` and asserts both are caught.
//!   `tests/inv004_no_self_certification.rs` re-states the same boundary against this
//!   crate's own tracked manifest, without sharing any code with that tool.
//! - **This crate cannot see a producer's structure.** [`check_certificate`] takes
//!   `&[u8]`, and it is the only public free function that returns an [`Outcome`].
//!   The checks that pin this cover `fn` and method signatures and public values, not
//!   trait-associated items or other forms (bn-2npu, Codex cr-3i3rst). Plan §20 puts "a serialization boundary
//!   between every engine and the kernel — a certificate is checked from its wire form,
//!   never from shared memory", and that boundary is preserved *through* this
//!   composition rather than merely at its far end: the full byte string is handed to
//!   the kernel unmodified, magic included, so the kernel's own decoder re-reads the
//!   artifact from byte zero and remains the only thing that ever interprets it.
//!   Nothing in `src/` names a decoded kernel type — no `wire::Certificate`, no
//!   `wire::decode` — and `tests/inv004_no_self_certification.rs` checks that
//!   mechanically against these sources.
//!
//! # Verdicts are relayed, never restated
//!
//! Each kernel returns a closed three-arm vocabulary — `Verified(claim)`,
//! `Rejected(reason)`, `Unsupported(feature)` — and [`KernelVerdict`] carries that
//! value itself, in the kernel's own type, not a re-encoding of it. The composition is
//! transparent by construction: for every input,
//! `check_certificate(bytes)` relays exactly what the routed kernel's own
//! `check_certificate(bytes)` returned, and `tests/family_routing.rs` asserts that
//! equality directly for green, rejected and unsupported artifacts of all four
//! families.
//!
//! Three consequences are deliberate:
//!
//! - **No boolean.** This surface exposes no `is_verified`, no `-> bool`, and no
//!   `Result<(), _>`. The kernels each offer `is_verified` "for assertions and for
//!   rendering, never as a substitute for matching"; at this layer there are *four*
//!   outcomes, not three, and one bit is even less able to carry them. A caller that
//!   wants a boolean has to write the collapse itself, where a reader can see it.
//! - **`Verified` cannot originate here.** `CheckedClaim::new` is `pub(crate)` in each
//!   kernel, so no code outside a kernel can construct a `Verified` arm. This crate
//!   could not fabricate one if it tried; it can only pass one along.
//! - **A routing failure is not a verdict.** When the leading bytes name no family,
//!   the answer is [`Outcome::Unroutable`] carrying a [`RoutingFault`] — never
//!   `Rejected` and never `Unsupported`. Deciding between those two is exactly what a
//!   decoder does, and this crate has no decoder; asserting either would be the
//!   collapse INV-008 exists to forbid. See [`RoutingFault`].
//!
//! # Malformed input
//!
//! docs/12 §11 classes "malformed artifact panic in kernel" as a release blocker, and
//! a dispatcher in front of the kernel is inside that blast radius. The lint wall below
//! is the kernel's own, so the no-panic covenant is a compile error here too: no
//! indexing, no slicing, no `unwrap`, no `expect`, no `panic!`, no wire-derived
//! allocation. Routing reads at most the first eight bytes and copies them into a fixed
//! array through a fallible conversion, so a zero-byte input and a 64 MiB input take
//! the same path.
//!
//! # Scope of this slice
//!
//! Receipt composition is not here. Each kernel's `receipt` module takes that crate's
//! own `Seam` of caller-supplied trusted inputs (build digest, toolchain identity,
//! input digest), and those four types are deliberately distinct; unifying them is a
//! receipt-format decision (RFC 0024, ADR-0035, INV-014), not a dispatch decision, and
//! inventing the union here would invent vocabulary. A caller that has a
//! [`KernelVerdict`] already holds the kernel's own claim and can call that kernel's
//! `receipt` directly.

#![forbid(unsafe_code)]
// The kernel crates' no-panic covenant, as lints rather than as review notes. This
// crate sits in front of the trusted checking base and is inside docs/12 §11's
// "malformed artifact panic in kernel" release-blocker class, so it carries the same
// wall; test modules opt back out locally.
#![deny(
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::todo,
    clippy::unimplemented,
    clippy::arithmetic_side_effects
)]

pub mod check;
pub mod family;
pub mod verdict;

pub use check::check_certificate;
pub use family::{Family, MAGIC_BYTES, RoutingFault};
pub use verdict::{KernelVerdict, Outcome};

// The four trusted checkers, re-exported so that this crate really is *one*
// caller-facing surface: matching a relayed [`KernelVerdict`] needs the kernel's own
// `Verdict`, `Rejection` and `Feature` types, and a caller should not have to declare
// four more dependencies — nor be tempted to, since a caller that names a kernel
// directly can also reach past this dispatcher.
pub use {
    continuum_kernel_core, continuum_kernel_sat, continuum_kernel_smt, continuum_kernel_temporal,
};
