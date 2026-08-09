//! **Stage 5**, proof-dependency slicing — and the refusal it is in this workspace
//! (RFC 0028, "Compiler pipeline"; RFC 0035; RFC 0012).
//!
//! # Scope: bn-imhw2 — the compiler's stage group 3 (stages 5–7)
//!
//! > | 5 | proof-dependency slicing | the named obligations | `ProofRelevant` |
//! >
//! > — RFC 0028, "Compiler pipeline"
//!
//! > | `ProofRelevant` | every selected item lies on the proof dependency slice of the named
//! > obligations | proof-slice check (RFC 0035, RFC 0012) | an item off the slice is a false
//! > relevance claim |
//! >
//! > — RFC 0028, "Guarantee classes"
//!
//! # This deployment has no proof service, and that is a checked fact
//!
//! Plan §20 homes the proof lane in `continuum-proof-client`, whose own module documentation
//! reads "PR-1 / IMPL-01 scaffold: this crate declares its responsibility and its dependency
//! boundary. The types and behavior land in the PR named above." It has no public item. There
//! is therefore nothing in this workspace that can compute a proof dependency slice, and
//! nothing that can run the proof-slice *check* RFC 0028's Validation section requires before
//! `ProofRelevant` may be published.
//!
//! `tests/pr11_compiler_stages_5_7.rs` asserts that from the crate's own bytes, and fails the
//! moment the surface arrives — the INV-015 tripwire device. This module's refusal is
//! therefore pinned to the workspace state that justifies it, not to a comment.
//!
//! # What stage 5 does instead of degrading
//!
//! > The proof-target pack profile […] MUST claim `ProofRelevant` or state why it cannot.
//! >
//! > — RFC 0028, "Pack profiles"
//!
//! This module is the second branch, made typed. [`ProofSlicing::refusal`] returns a
//! [`StageRefusal`] carrying the [`UnsupportedSurface::ProofDependencySlice`] surface and the
//! [`SurfaceAbsence`] that explains it; [`crate::compile`] records it in the trail, declines
//! the guarantee, records every `proof` candidate as an `unsupported`, irretrievable omission,
//! and carries `InconclusiveReason::IncompleteProofSearch` so the pack cannot default to
//! `satisfied`.
//!
//! It does **not** narrow the working set. A stage that cannot run must not act: dropping
//! items on the grounds that a slicer that does not exist would have dropped them is the
//! "guessing" `rule errors.unsupported_surface` prohibits, and it would also make the drop
//! unattributable to any stage that decided it.
//!
//! # `ProofRelevant` is unreachable by construction, not merely unclaimed
//!
//! [`crate::guarantee::License::issue`] is `pub(crate)`, so the only code that can mint
//! permission to claim `ProofRelevant` is this crate — and no line of this crate does.
//! That is a property of the source, so it is checked against the source: the integration
//! suite reads every `License::issue(` call site under `crates/continuum-context/src/` and
//! holds the licensed set to a recorded inventory of exactly `CausallyClosed` and
//! `PropertyPreserving`. A new licence anywhere — `ProofRelevant` above all — fails that test
//! until the audit is redone, and the same scan over a mutant with the line spliced in fails
//! too, so the check is not vacuous.
//!
//! This is the anti-vacuity mutant RFC 0028's own list asks for, pointed at this stage: "a
//! stage that quietly licenses `ProofRelevant` with no proof slice behind it" has no spelling.
//!
//! # The obligations are not candidates, and are not checked against the order
//!
//! Stage 4's join claims are checked against the causal order because a source correspondence
//! claims things *about candidates* ([`crate::compile::CompileError::NotACorrespondenceKind`]).
//! An obligation is not a candidate: it is a proof-side identity produced by a subsystem this
//! workspace does not have, so there is nothing here to check it against and a check invented
//! for it would be a second, unchecked authority over a vocabulary this crate does not own.
//! The obligations are carried, counted, and reported — and nothing more is claimed about
//! them.
//!
//! # Clause → test
//!
//! | Clause | Source | Test |
//! |---|---|---|
//! | stage 5 refuses rather than degrading | RFC 0026 `rule errors.unsupported_surface` | `stage_five_refuses_in_this_deployment` |
//! | "claim `ProofRelevant` or state why it cannot" | RFC 0028, "Pack profiles" | `the_refusal_names_the_absent_proof_service` |
//! | no obligation named is its own absence | INV-008 | `a_question_with_no_obligation_named_is_a_different_absence` |
//! | stage 5's column is the `proof` kind | RFC 0028, "Selection and the causal core" | `stage_five_decides_the_proof_kind_and_nothing_else` |
//! | the refusal is deterministic (INV-005) | INV-005 | `two_readings_of_one_slicing_agree` |

use std::collections::BTreeSet;

use continuum_value::value::Name;

use crate::selection::SelectionKind;
use crate::unsupported::{AbsentProducer, StageRefusal, SurfaceAbsence, UnsupportedSurface};

/// **Stage 5's premise**: the obligations the question names.
///
/// A value the compiler is handed, for exactly [`crate::causal::CausalOrder`]'s reason — the
/// producing subsystem is not this crate's and, here, is not this workspace's either.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ProofSlicing {
    obligations: BTreeSet<Name>,
}

impl ProofSlicing {
    /// Ask stage 5 to slice over the named obligations.
    #[must_use]
    pub fn over(obligations: impl IntoIterator<Item = Name>) -> Self {
        Self {
            obligations: obligations.into_iter().collect(),
        }
    }

    /// Ask stage 5 to run with no obligation named.
    ///
    /// A distinct fact from [`ProofSlicing::over`] with an empty iterator only in intent; both
    /// produce the same value, and both are refused with [`SurfaceAbsence::NothingNamed`].
    #[must_use]
    pub fn nothing_named() -> Self {
        Self {
            obligations: BTreeSet::new(),
        }
    }

    /// The obligations, in canonical order.
    #[must_use]
    pub const fn obligations(&self) -> &BTreeSet<Name> {
        &self.obligations
    }

    /// How many obligations were named.
    #[must_use]
    pub fn len(&self) -> usize {
        self.obligations.len()
    }

    /// Whether the question named no obligation at all.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.obligations.is_empty()
    }

    /// Stage 5's outcome in this deployment: a typed refusal, always.
    ///
    /// The question's own defect is decided first — see [`crate::unsupported`]'s "Why the
    /// absence is typed, and not one word": an empty obligation set makes the stage vacuous
    /// whether or not a proof service exists, and that is decidable without consulting the
    /// absent producer.
    #[must_use]
    pub fn refusal(&self) -> StageRefusal {
        let absence = if self.obligations.is_empty() {
            SurfaceAbsence::NothingNamed
        } else {
            SurfaceAbsence::ProducerNotDeployed(AbsentProducer::ProofService)
        };
        StageRefusal::of(UnsupportedSurface::ProofDependencySlice, absence)
    }

    /// Whether a candidate of this kind is stage 5's to decide.
    ///
    /// Exactly `proof`. RFC 0028's plan §6.2 mapping gives that kind one home — "proof/
    /// refinement slice | `selected[].kind = proof` | Licenses `ProofRelevant` **only when the
    /// proof-slice check ran**" — so a `proof` candidate that this compile publishes without
    /// stage 5 is published for some *other* stage's reason (its place in the causal order),
    /// and a `proof` candidate it does not publish while stage 5 refused is `unsupported`
    /// rather than provably irrelevant.
    #[must_use]
    pub const fn decides(kind: SelectionKind) -> bool {
        matches!(kind, SelectionKind::Proof)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn name(text: &str) -> Name {
        Name::new(text).expect("well formed")
    }

    #[test]
    fn stage_five_refuses_in_this_deployment() {
        let slicing = ProofSlicing::over([name("o_durable"), name("o_ack")]);
        let refusal = slicing.refusal();
        assert_eq!(refusal.surface(), UnsupportedSurface::ProofDependencySlice);
        assert_eq!(refusal.stage().number(), 5);
        assert_eq!(
            refusal.inconclusive_reason(),
            continuum_value::assurance::InconclusiveReason::IncompleteProofSearch
        );
    }

    #[test]
    fn the_refusal_names_the_absent_proof_service() {
        let refusal = ProofSlicing::over([name("o_durable")]).refusal();
        assert_eq!(
            refusal.absence(),
            SurfaceAbsence::ProducerNotDeployed(AbsentProducer::ProofService)
        );
        assert_eq!(
            refusal.absence().producer().expect("a named producer"),
            AbsentProducer::ProofService
        );
        assert_eq!(refusal.note(), "proof-slicing-no-producer");
    }

    #[test]
    fn a_question_with_no_obligation_named_is_a_different_absence() {
        let refusal = ProofSlicing::nothing_named().refusal();
        assert_eq!(refusal.absence(), SurfaceAbsence::NothingNamed);
        assert_eq!(refusal.note(), "proof-slicing-no-obligation");
        // Same consequences, different recorded fact — which is INV-008's point.
        assert_eq!(
            refusal.inconclusive_reason(),
            ProofSlicing::over([name("o_durable")])
                .refusal()
                .inconclusive_reason()
        );
        assert_ne!(refusal, ProofSlicing::over([name("o_durable")]).refusal());
        // And an explicitly empty list reads the same as naming none at all.
        assert_eq!(ProofSlicing::over([]), ProofSlicing::nothing_named());
    }

    #[test]
    fn the_obligations_are_carried_in_canonical_order() {
        // `Name` orders shortlex, so length decides before the alphabet does.
        let slicing = ProofSlicing::over([name("o_durability"), name("o_ack"), name("o_loss")]);
        assert_eq!(
            slicing.obligations().iter().cloned().collect::<Vec<_>>(),
            [name("o_ack"), name("o_loss"), name("o_durability")]
        );
        assert_eq!(slicing.len(), 3);
        assert!(!slicing.is_empty());
    }

    #[test]
    fn a_repeated_obligation_is_one_obligation() {
        let slicing = ProofSlicing::over([name("o_ack"), name("o_ack")]);
        assert_eq!(slicing.len(), 1);
    }

    #[test]
    fn stage_five_decides_the_proof_kind_and_nothing_else() {
        let decided: Vec<SelectionKind> = SelectionKind::ALL
            .into_iter()
            .filter(|kind| ProofSlicing::decides(*kind))
            .collect();
        assert_eq!(decided, [SelectionKind::Proof]);
    }

    #[test]
    fn two_readings_of_one_slicing_agree() {
        let slicing = ProofSlicing::over([name("o_durable")]);
        assert_eq!(slicing.refusal(), slicing.refusal());
        assert_eq!(slicing, ProofSlicing::over([name("o_durable")]));
    }
}
