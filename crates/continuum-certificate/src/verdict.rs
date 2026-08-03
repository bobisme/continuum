//! What this crate answers with: a kernel's verdict, carried, or the fact that no
//! kernel was reached.
//!
//! # The vocabulary is borrowed, not defined
//!
//! Every arm of [`KernelVerdict`] holds the *owning kernel's own* `Verdict` value. This
//! module declares no `Verified`, no `Rejected`, no `Unsupported` and no claim type of
//! its own, so there is nothing here for a kernel's vocabulary to drift away from: a
//! new arm in `continuum_kernel_temporal::Verdict` reaches a caller of this crate the
//! day it lands, with no edit here and no re-mapping table to forget.
//!
//! That is also why there is no unified claim accessor. The four `CheckedClaim` types
//! are deliberately different — `continuum-kernel-sat` reports propagations,
//! `continuum-kernel-smt` reports an assurance class and its trusted theory lemmas,
//! `continuum-kernel-core` reports states and transitions — and a lowest common
//! denominator over them would drop exactly the part docs/03 §2 and RFC 0005's
//! "Proof-producing solver policy" insist a reader see: what a `Verified` still trusts.
//! A caller matches the family it cares about and reads the claim the kernel actually
//! made.

use crate::family::{Family, RoutingFault};

/// A verdict, in the vocabulary of the kernel that produced it.
///
/// The payload of each arm is that crate's own `Verdict` — the same three-arm
/// `Verified` / `Rejected` / `Unsupported` closed vocabulary, moved rather than
/// translated.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KernelVerdict {
    /// `continuum-kernel-core`'s verdict on a `CONTCERT` artifact.
    Core(continuum_kernel_core::Verdict),
    /// `continuum-kernel-sat`'s verdict on a `CONTSATC` artifact.
    Sat(continuum_kernel_sat::Verdict),
    /// `continuum-kernel-smt`'s verdict on a `CONTSMTC` artifact.
    Smt(continuum_kernel_smt::Verdict),
    /// `continuum-kernel-temporal`'s verdict on a `CONTTMPC` artifact.
    Temporal(continuum_kernel_temporal::Verdict),
}

impl KernelVerdict {
    /// Which checker spoke.
    #[must_use]
    pub const fn family(&self) -> Family {
        match self {
            Self::Core(_) => Family::Core,
            Self::Sat(_) => Family::Sat,
            Self::Smt(_) => Family::Smt,
            Self::Temporal(_) => Family::Temporal,
        }
    }
}

/// What [`crate::check_certificate`] answers.
///
/// Four outcomes, not three and not two: the three a checker can reach, plus the one
/// this crate can reach on its own. Nothing collapses them — there is no `is_verified`,
/// no `-> bool` and no `Result<(), _>` anywhere on this surface, because a caller that
/// wants one bit out of four facts should have to write that loss down where a reviewer
/// can see it (INV-008; docs/03 §2).
// A relayed kernel verdict carries that kernel's `CheckedClaim` — its envelope, its
// counters, its trusted-component list — and is an order of magnitude larger than a
// routing fault, which is a discriminant and eight bytes of magic. Boxing the large arm
// would put an allocation on the arm that is *supposed* to be the common one and would
// make the type's shape argue that a checked certificate is the unusual case, which is
// the opposite of what this surface is for. It would also insert an indirection of this
// crate's own invention between the caller and the kernel's value, when the whole claim
// of the type is that nothing here comes between them. The same choice, for the same
// reason, as `continuum_mcp::answer::Outcome` and `continuumd::daemon::family::Arguments`
// — "the uniform shape is worth more than the moved bytes". Nothing is on a hot path:
// one value per certificate, moved once, after a check that has already read the whole
// artifact.
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// The leading magic named a family, and that family's kernel checked the whole
    /// byte string. The payload is the kernel's answer, unmodified.
    Checked(KernelVerdict),
    /// No kernel owns these bytes, so none of them ran and none of them has an opinion.
    ///
    /// Not a rejection and not an unsupported feature; see [`RoutingFault`].
    Unroutable(RoutingFault),
}

impl Outcome {
    /// The checker that answered, or `None` when routing failed.
    #[must_use]
    pub const fn family(&self) -> Option<Family> {
        match self {
            Self::Checked(verdict) => Some(verdict.family()),
            Self::Unroutable(_) => None,
        }
    }

    /// The relayed kernel verdict, or `None` when routing failed.
    #[must_use]
    pub const fn verdict(&self) -> Option<&KernelVerdict> {
        match self {
            Self::Checked(verdict) => Some(verdict),
            Self::Unroutable(_) => None,
        }
    }

    /// Why routing failed, or `None` when a checker did answer.
    #[must_use]
    pub const fn routing_fault(&self) -> Option<&RoutingFault> {
        match self {
            Self::Unroutable(fault) => Some(fault),
            Self::Checked(_) => None,
        }
    }
}
