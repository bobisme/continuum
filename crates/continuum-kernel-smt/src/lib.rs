//! `continuum-kernel-smt` — trusted checking base: SMT certificate checking (plan §20,
//! PR 9).
//!
//! # Responsibility
//!
//! Independent checking of SMT proof certificates over the theories Continuum admits,
//! from wire form only.
//!
//! Foreign solvers remain isolated behind normalized artifacts and never ship in
//! release binaries (ADR-0029). What crosses the boundary is the proof, and what this
//! crate does with it is replay the part of it that is replayable — and *name* the
//! part that is not.
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
//! Stronger than required, and deliberately: this crate depends on *nothing*. Not on
//! `continuum-kernel-sat`, whose propagation engine it re-implements rather than
//! imports. plan §20's crate table names four kernel crates and no `kernel-common`,
//! docs/03 §8 lists "certificate decoding" among the axes independent paths must
//! differ along, and RFC 0005's "Independence" section requires independent
//! implementations of semantics-sensitive functions. A propagation bug shared by the
//! SAT and SMT checkers would be a common-mode bug across the trusted base.
//! `tools/check_crate_boundaries.py` enforces the forbidden edges mechanically.
//!
//! # What is checked, and what is trusted
//!
//! RFC 0005's "Kernel layering" assigns this crate the "Alethe checker subset". The
//! subset implemented here is the one the dossier already sanctions, stated without
//! euphemism:
//!
//! > Backend proofs can be Alethe/LRAT/solver-specific plus checked theory lemmas.
//! > Until theory-proof support is mature, the result is `TRUSTED_SOLVER`, not
//! > `CHECKED_CERTIFICATE`.
//! >
//! > — `notes/plan/docs/03_ASSURANCE_AND_TCB.md` §6.2
//!
//! A certificate carries an **atom table** (theory atoms as opaque tokens), the
//! **propositional skeleton** of the asserted formulas as clauses over those atoms, a
//! set of **theory lemmas** — clauses each labelled with the theory that makes it valid
//! — and a **resolution proof** whose steps are checked exactly as an LRAT chain is:
//! unit propagation over named antecedents, ending in the empty clause.
//!
//! So the kernel establishes, in full and from the bytes alone:
//!
//! > the asserted skeleton *together with* the carried theory lemmas is
//! > propositionally unsatisfiable.
//!
//! It does **not** establish that a theory lemma is valid in its theory. There is no
//! arithmetic, no congruence closure, and no decision procedure of any kind in this
//! crate, and adding a fake one would be worse than having none: it would move the
//! trust boundary without moving the evidence. Every theory whose lemmas the proof
//! actually used is therefore named by [`CheckedClaim::trusted_components`], and
//! [`CheckedClaim::assurance_class`] reports docs/03 §7's
//! [`AssuranceClass::TrustedSolver`] whenever the set is non-empty. A certificate with
//! no theory lemmas — a pure Boolean conflict among the assertions — is
//! [`AssuranceClass::CheckedCertificate`], because a propositionally unsatisfiable
//! skeleton is unsatisfiable in every theory.
//!
//! That split is ADR-0017 exactly: "UNSAT results are `CHECKED_CERTIFICATE` only when
//! proof artifacts are independently checked; otherwise they are `TRUSTED_SOLVER`",
//! with "Unsupported theory proofs remain explicitly solver-trusted". Checked theory
//! lemmas — an Alethe theory-step checker, or a proof-producing decision procedure —
//! are the next slice, and the wire form reserves the step-kind code space for the
//! Alethe rule vocabulary they will need.
//!
//! # The serialization boundary
//!
//! [`check_certificate`] takes `&[u8]`. There is no other entry point, and no
//! constructor exists for [`wire::Certificate`] outside [`wire::decode`], which also
//! takes `&[u8]`. A solver adapter cannot hand the checker a structure it built.
//!
//! # Verdicts are typed, and inconclusiveness is a verdict shape
//!
//! [`check_certificate`] returns [`Verdict`] — `Verified(claim)`, `Rejected(reason)`,
//! `Unsupported(feature)`. There is no `bool` anywhere on the surface, and `Verified`
//! cannot be constructed without a [`CheckedClaim`] naming what was established and
//! what it still trusts (INV-008; docs/03 §2; RFC 0005's "Proof-producing solver
//! policy": no bare "verified" without its trusted components).
//!
//! # Malformed input
//!
//! docs/12 §11 classes "malformed artifact panic in kernel" as a release blocker and
//! docs/16 PO-KER-001 states decoder totality as a proof obligation. Every failure mode
//! of this crate is a value: no indexing, no slicing, no `unwrap`, no `expect`, no
//! `panic!`, and no wire-derived allocation reservation appears in the shipped code,
//! and the crate-level lints below make a regression a compile error.

#![forbid(unsafe_code)]
// The no-panic covenant, as lints rather than as review notes (docs/12 §11; docs/16
// PO-KER-001). Test modules opt back out locally.
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
    AssuranceClass, CertificateKind, CheckedClaim, Feature, Field, Rejection, TokenFault, Verdict,
};
