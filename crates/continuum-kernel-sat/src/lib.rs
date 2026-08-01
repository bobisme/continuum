//! `continuum-kernel-sat` — trusted checking base: SAT certificate checking (plan §20,
//! PR 9).
//!
//! # Responsibility
//!
//! Independent checking of propositional certificates (resolution/DRAT-class evidence)
//! produced by untrusted solvers, from wire form only.
//!
//! The solver is a search procedure and stays outside the trust base; only its
//! certificate crosses the boundary. ADR-0029 makes that structural: no external SAT
//! solver ships in a release artifact, so the only thing a release binary can do with
//! solver output is *check* it.
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
//! external crate and no workspace crate — not even its sibling
//! `continuum-kernel-core`, whose reader and envelope decoder this crate re-implements
//! rather than imports. docs/03 §8 lists "state representation", "arithmetic",
//! "parser", "certificate decoding" as the axes independent paths must differ along,
//! and RFC 0005's "Independence" section allows shared *schema types* but requires
//! independent implementations of semantics-sensitive functions. plan §20's crate
//! table names four kernel crates and no `kernel-common`, so the duplication is the
//! architecture rather than an oversight: a codec bug shared by two checkers is a
//! common-mode bug in the trusted base.
//! `tools/check_crate_boundaries.py` enforces the forbidden edges mechanically.
//!
//! # The serialization boundary
//!
//! > serialization boundary: certificates are checked from wire form, never from
//! > shared memory;
//! >
//! > — `notes/plan/plan.md` §20, "Dependency rules"
//!
//! [`check_certificate`] takes `&[u8]`. There is no other entry point, and no
//! constructor exists for [`wire::Certificate`] outside [`wire::decode`], which also
//! takes `&[u8]`. A solver adapter therefore cannot hand the checker a structure it
//! built — the type system, not a convention, forbids it.
//!
//! # What is checked
//!
//! One of RFC 0005's seven certificate families, in the shape RFC 0005's "Kernel
//! layering" assigns to this crate:
//!
//! ```text
//! continuum-kernel-sat
//!   LRAT checker
//! ```
//!
//! [`CertificateKind::Lrat`] is a clausal refutation: a CNF formula plus a sequence of
//! clause-addition steps, each carrying the antecedent chain that makes it *reverse
//! unit propagation* derivable, ending in the empty clause. The checker re-runs every
//! propagation over the certificate's own clauses; it never takes a producer's word
//! that a step follows. The receipt spelling of the family is `lrat`, fixed by
//! `notes/plan/schemas/proof-receipt.schema.json`'s `certificate.kind` enum.
//!
//! A verified certificate is docs/03 §7's top solver-trust row —
//!
//! ```text
//! UNSAT:
//!   Alethe/LRAT proof checked         → certificate checked
//! ```
//!
//! — ADR-0017's `CHECKED_CERTIFICATE` rather than `TRUSTED_SOLVER`, *for the formula
//! the certificate carries*. That the formula is the propositional encoding of the
//! caller's model and property is not a property of these bytes; it is named by
//! [`CheckedClaim::trusted_components`] and bound by the receipt (RFC 0024, ADR-0035).
//!
//! # What is deliberately not checked, stated loudly
//!
//! LRAT has two step families. *RUP* steps — a clause implied by unit propagation from
//! named antecedents — are implemented here in full. *RAT* steps — the resolution
//! asymmetric tautology extension that lets a solver record blocked-clause additions
//! and other satisfiability-preserving-but-not-implied inferences — are **not**. A
//! certificate that names one is [`Feature::ProofStep`], never a rejection: the
//! artifact may be a valid RAT proof under a contract this build does not implement,
//! and reporting it as invalid is exactly the ambiguity INV-008 forbids. The wire form
//! reserves the step-kind code space for it (see [`wire`]).
//!
//! # Verdicts are typed, and inconclusiveness is a verdict shape
//!
//! [`check_certificate`] returns [`Verdict`], a closed three-arm vocabulary —
//! `Verified(claim)`, `Rejected(reason)`, `Unsupported(feature)`. There is no `bool`
//! anywhere on the surface, and `Verified` cannot be constructed without a
//! [`CheckedClaim`] naming what was established, the envelope it is relative to, and
//! the components it still trusts (INV-008; docs/03 §2; RFC 0005's "Proof-producing
//! solver policy": no bare "verified" without its trusted components).
//!
//! # Malformed input
//!
//! docs/12 §11 classes "malformed artifact panic in kernel" as a release blocker, and
//! docs/16 PO-KER-001 ("Decoder totality") states it as a proof obligation. Every
//! failure mode of this crate is a value: no indexing, no slicing, no `unwrap`, no
//! `expect`, no `panic!`, and no wire-derived allocation reservation appears in the
//! shipped code, and the crate-level lints below make a regression a compile error.
//! The `garbage_bytes_never_panic` test feeds the checker a reproducible pseudo-random
//! corpus, every prefix of a valid certificate, and spliced corruptions.

#![forbid(unsafe_code)]
// The no-panic covenant, as lints rather than as review notes. Malformed input has
// to be a value on every path (docs/12 §11, release-blocker classes; docs/16
// PO-KER-001), so the constructs that turn bad data into an unwind are denied
// outright in shipped code; test modules opt back out locally.
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
pub use verdict::{CertificateKind, CheckedClaim, Feature, Field, Rejection, TokenFault, Verdict};
