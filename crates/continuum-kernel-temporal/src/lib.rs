//! `continuum-kernel-temporal` — trusted checking base: temporal and liveness
//! certificate checking (plan §20, PR 9).
//!
//! # Responsibility
//!
//! Independent checking of temporal-property and liveness evidence — fairness
//! obligations, lasso witnesses, and their closure conditions — from wire form only.
//!
//! The liveness engine searches; this crate only checks what the search claims.
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
//! external crate and no workspace crate, not even `continuum-kernel-core`, whose state
//! table and envelope decoder it re-implements rather than imports. docs/03 §8 lists
//! "state representation", "traversal", "parser" and "certificate decoding" among the
//! axes independent paths must differ along; plan §20's crate table names four kernel
//! crates and no shared one. `tools/check_crate_boundaries.py` enforces the forbidden
//! edges mechanically.
//!
//! The independence matters most here. `continuum-engine-liveness` will compute SCCs to
//! *find* fair cycles; this crate computes them to *refuse* a certificate that claims
//! there are none. If both used the same SCC implementation, a bug in it would make the
//! engine miss a counterexample and the kernel bless the omission.
//!
//! # What is checked
//!
//! RFC 0005's "Kernel layering" assigns this crate "graph/SCC/ranking certificates",
//! and docs/03 §6.5 splits liveness evidence into exactly those two shapes. Both are
//! implemented, and `notes/plan/schemas/proof-receipt.schema.json` fixes both receipt
//! spellings — `ranking` and `fair-scc-exclusion`:
//!
//! - [`CertificateKind::Ranking`] — docs/03 §6.5's "Ranking: well-founded domain;
//!   decrease obligations; … safety side conditions" (docs/16 PO-LIV-002, PO-LIV-003).
//!   Every non-goal state of the carried system has a successor, and every transition
//!   out of a non-goal state strictly decreases a natural-number rank. The conclusion
//!   is unconditional progress: from *every* state of the table, *every* execution
//!   reaches the goal set within `rank(s)` steps. No fairness assumption is used, so
//!   none is trusted.
//! - [`CertificateKind::FairSccExclusion`] — docs/03 §6.5's "SCC decomposition;
//!   reachable-component witnesses; fairness acceptance labels; absence of accepting
//!   SCCs" (docs/16 PO-LIV-004: "Finite graph contains no reachable SCC satisfying
//!   liveness violation acceptance"). The kernel recomputes reachability from the
//!   declared initial states, recomputes the strongly connected components of the
//!   reachable non-goal subgraph, and refuses the certificate if any of them admits a
//!   weakly fair cycle.
//!
//! Both families establish the same property class: `◇ goal` from the states named,
//! where the goal set is carried explicitly. Read state-wise over a closed table that
//! is what a `leads-to` obligation reduces to (RFC 0003 names `leads-to`;
//! `corpus/tla-examples/FEATURE_CENSUS.md` lists "eventually/always, leads-to, response
//! and recurrence" as the temporal shapes the corpus needs).
//!
//! # Two families because they trust different things
//!
//! A ranking certificate is the stronger artifact and the harder one to produce: it
//! proves progress on *every* schedule, so a system whose progress depends on a fairness
//! assumption cannot have one. An SCC-exclusion certificate is producible for those
//! systems, and pays for it by trusting that the declared weakly fair action set is the
//! model's — a trusted component the ranking family does not have. Neither is a
//! degraded version of the other, and [`CheckedClaim::trusted_components`] is where the
//! difference is visible.
//!
//! # What is deliberately not checked, stated loudly
//!
//! *Strong* fairness (compassion: an action enabled infinitely often must be taken
//! infinitely often) is not implemented. RFC 0008 and RFC 0015 both name it, and a
//! certificate that declares it is [`Feature::FairnessClass`] — Unsupported, never
//! Rejected, because such a certificate may be perfectly valid under a contract this
//! build does not implement (INV-008). A ranking certificate that declares fair actions
//! is [`Feature::FairnessDischarge`] for the same reason: docs/03 §6.5's
//! "fairness-to-progress linkage" is a real obligation and this build does not check
//! it, so it declines rather than ignoring the declaration.
//!
//! # Verdicts are typed
//!
//! [`check_certificate`] returns [`Verdict`] — `Verified(claim)`, `Rejected(reason)`,
//! `Unsupported(feature)` — and `Verified` cannot be constructed without a
//! [`CheckedClaim`] naming what was established and what it still trusts (INV-008;
//! docs/03 §2; RFC 0005's "Proof-producing solver policy").
//!
//! # Malformed input
//!
//! docs/12 §11 classes "malformed artifact panic in kernel" as a release blocker and
//! docs/16 PO-KER-001 states decoder totality as a proof obligation. Every failure mode
//! of this crate is a value: no indexing, no slicing, no `unwrap`, no `expect`, no
//! `panic!`, and no wire-derived allocation reservation appears in the shipped code.
//! The component search is iterative and carries its own stack, so a deep graph is a
//! heap allocation rather than a stack overflow — RFC 0005's "cycle/recursion bounds"
//! read as a structural property rather than a limit.

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
    CertificateKind, CheckedClaim, FairnessClass, Feature, Field, PropertyClass, Rejection,
    TokenFault, Verdict,
};
