//! `continuum-kernel-core` — trusted checking base: core certificate semantics (plan
//! §20, PR 9).
//!
//! # Responsibility
//!
//! Finite-closure and type-certificate checking, plus receipt generation carrying the
//! checker epoch, build digest, and input hashes (INV-014).
//!
//! One of the four `continuum-kernel-*` crates that constitute the trusted checking
//! base. Together they carry a <15,000 non-test-line covenant (docs/03) and must build
//! reproducibly from pinned sources.
//!
//! # Dependency-boundary contract
//!
//! - Kernel covenant (plan §20): no shared optimized evaluator code with any engine, no
//!   async, no unsafe, no plugins or dynamic loading, and a serialization boundary
//!   between every engine and the kernel.
//! - **May not depend on search**: no `continuum-engine-*`, no `continuum-forge`, and
//!   no `continuum-asupersync` — directly or transitively.
//!
//! Stronger than required, and deliberately: this crate depends on *nothing*. No
//! external crate and no workspace crate, so its decoder, its arithmetic and its
//! state representation are its own. docs/03 §8 lists exactly those — "state
//! representation", "arithmetic", "parser", "certificate decoding" — as the axes
//! independent paths must differ along, and RFC 0005's "Independence" section allows
//! shared *schema types* but requires independent implementations of
//! semantics-sensitive functions. A checker that shared a codec with its producers
//! would inherit their codec bugs invisibly. `tools/check_crate_boundaries.py`
//! enforces the forbidden edges mechanically.
//!
//! # The serialization boundary
//!
//! > serialization boundary: certificates are checked from wire form, never from
//! > shared memory;
//! >
//! > — `notes/plan/notes/START_HERE_IMPLEMENTATION.md`, PR 9
//!
//! [`check_certificate`] takes `&[u8]`. There is no other entry point, and no
//! constructor exists for [`wire::Certificate`] outside [`wire::decode`], which also
//! takes `&[u8]`. An engine therefore cannot hand the checker a structure it built —
//! the type system, not a convention, forbids it. This is the mechanical half of
//! INV-004 ("no self-certification"); the other half is that the crate cannot link an
//! engine at all.
//!
//! # What is checked
//!
//! Two of RFC 0005's seven certificate families:
//!
//! - [`verdict::CertificateKind::FiniteClosure`] — RFC 0005's "Closed reachable set",
//!   docs/03 §6.1's "Closed finite state space": `Init ⊆ S`, `Post(S) ⊆ S`,
//!   `S ⊆ P`. The formal statement, and the theorem that these three conjuncts imply
//!   safety at every reachable state, is `ClosureCertificate` in
//!   `lean/Continuum/Certificate.lean` (RFC 0012 rung T0).
//! - [`verdict::CertificateKind::StateType`] — every state of the certificate's table
//!   lies in the declared state domain (docs/16 PO-MOD-003); composed with a closure
//!   certificate over the same table this is the "finite closure safety for DieHard
//!   `TypeOK`" milestone of docs/23's theorem ladder.
//!
//! The other five families belong elsewhere: LRAT to `continuum-kernel-sat`, Alethe
//! to `continuum-kernel-smt`, SCC/ranking to `continuum-kernel-temporal`, and
//! counterexample/refinement witnesses to later slices. A certificate naming one of
//! them is [`verdict::Feature::CertificateKind`], never a rejection.
//!
//! # Verdicts are typed, and inconclusiveness is a verdict shape
//!
//! [`check_certificate`] returns [`Verdict`], a closed three-arm vocabulary —
//! `Verified(claim)`, `Rejected(reason)`, `Unsupported(feature)`. There is no `bool`
//! anywhere on the surface, and `Verified` cannot be constructed without a
//! [`CheckedClaim`] naming what was established, the envelope it is relative to, and
//! the components it still trusts (INV-008; docs/03 §2; RFC 0005's
//! "Proof-producing solver policy": no bare "verified" without its trusted
//! components).
//!
//! # Malformed input
//!
//! docs/12 §11 classes "malformed artifact panic in kernel" as a release blocker.
//! Every failure mode of this crate is a value: no indexing, no slicing, no `unwrap`,
//! no `expect`, no `panic!`, and no wire-derived allocation reservation appears in
//! the shipped code, and the crate-level lints below make a regression a compile
//! error. The `garbage_bytes_never_panic` test feeds the checker a reproducible
//! pseudo-random corpus, every prefix of a valid certificate, and spliced
//! corruptions.
//!
//! # Scope of this slice
//!
//! Receipt generation (INV-014: checker epoch, build digest, input hashes), the
//! <15,000-line covenant tooling, and the full mutation campaign are PR 9's covenant
//! slice, not this one. What lands here is the wire form, the two checkers, and the
//! verdict vocabulary they answer in.

#![forbid(unsafe_code)]
// The no-panic covenant, as lints rather than as review notes. Malformed input has
// to be a value on every path (docs/12 §11, release-blocker classes), so the
// constructs that turn bad data into an unwind are denied outright in shipped code;
// test modules opt back out locally.
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
pub mod verdict;
pub mod wire;

#[cfg(test)]
mod fixture;

pub use check::check_certificate;
pub use verdict::{
    CertificateKind, CheckedClaim, Feature, Field, PropertyClass, Rejection, TokenFault, Verdict,
};
