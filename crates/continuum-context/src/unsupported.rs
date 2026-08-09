//! The typed refusal a stage records when its producing subsystem is **not in this
//! deployment** (RFC 0026 `rule errors.unsupported_surface`; INV-008; RFC 0028 C1).
//!
//! # Scope: bn-imhw2 — the compiler's stage group 3 (stages 5–7)
//!
//! > An operation registered ahead of its producing subsystem MUST fail with the typed
//! > `UnsupportedSemanticFeature` rather than degrading, guessing, or returning an empty
//! > success.
//! >
//! > — RFC 0026, `rule errors.unsupported_surface`
//!
//! Two of the three stages in this group have no producer in this workspace, and the
//! sentence above decides what they do about it: **refuse**. Not narrow-by-default, not
//! license-anyway, not silently skip. This module is the vocabulary that refusal is recorded
//! in, shared by [`crate::proof`] (stage 5) and [`crate::correspondence`] (stage 7) so the two
//! cannot spell the same fact two ways.
//!
//! # The evidence, recorded here rather than remembered
//!
//! | Surface | Stage | Producer plan §20 names | Its state in this workspace |
//! |---|---|---|---|
//! | proof dependency slice | 5 | `continuum-proof-client` (RFC 0035, RFC 0012) | a PR-1 / IMPL-01 scaffold: no public item at all |
//! | §16 correspondence graph | 7 | `continuum-refinement` (plan §16) | a PR-1 / IMPL-01 scaffold: no public item at all |
//!
//! Both rows are *live* assertions rather than prose: the integration suite
//! `tests/pr11_compiler_stages_5_7.rs` reads those two crates' sources and fails the moment
//! either grows a public surface, which is the INV-015 device applied here. A refusal that
//! outlived its reason would be as dishonest as a claim that outran its evidence, so the
//! refusal is pinned to go red when the producer lands.
//!
//! # Why the absence is typed, and not one word
//!
//! > Typed inconclusiveness (INV-008) — timeout, unsupported semantics, insufficient
//! > telemetry, abstraction ambiguity, and incomplete proof search are distinct outcomes.
//! > Never a bare boolean, and never a success flag that outruns the evidence.
//! >
//! > — `AGENTS.md`, "Key invariants"
//!
//! [`SurfaceAbsence`] keeps two facts apart that a single `unsupported` token would collapse:
//!
//! - [`SurfaceAbsence::ProducerNotDeployed`] — *this deployment has no proof service* (or no
//!   correspondence graph). A property of the workspace, which no caller can change and no
//!   question can avoid.
//! - [`SurfaceAbsence::NothingNamed`] — the question named nothing for the stage to work on.
//!   A property of the *question*, which is decidable **without** the absent producer, and is
//!   therefore decided first: a proof slice "of the named obligations" over no obligations
//!   would be vacuous whether or not a service existed, and
//!   [`crate::monitor::Inapplicable::EmptyAlphabet`] is this crate's precedent for refusing to
//!   dress a vacuous answer as a checked one.
//!
//! Both absences have the *same* consequences — the guarantee is declined, the omission is
//! `unsupported` and irretrievable, and the verdict is owed a typed reason — and that is the
//! point: INV-008 is about the outcomes being distinguishable in the record, not about them
//! being treated differently.
//!
//! # What a refusal costs, exactly
//!
//! 1. **The guarantee.** A refused stage licenses nothing. For stage 5 that is
//!    `ProofRelevant`, which is additionally unreachable by construction — see
//!    [`crate::proof`].
//! 2. **An `unsupported`, irretrievable omission** for every candidate the stage was the only
//!    stage that could have decided ([`StageRefusal::omission_reason`],
//!    [`StageRefusal::irretrievable_reason`]). RFC 0028: "when `expandable` is false the reason
//!    MUST be one that explains irretrievability (`redaction` or `unsupported`)".
//! 3. **A typed `inconclusive` reason** ([`StageRefusal::inconclusive_reason`]). RFC 0028: a
//!    proof-target or invariant pack "MUST NOT default to `satisfied`"; correction 4 settles
//!    that the outcome is "`verdict: inconclusive` with one of the six typed reasons plus an
//!    omission record". `AbstractionAmbiguity` and `IncompleteProofSearch` get their first
//!    producers here.
//! 4. **An auditable intermediate that records the stage RAN.** A stage that refused is not a
//!    stage that did not run, and [`crate::stage::StageTrail`] keeps the two apart already —
//!    the refusing stage records its working set unchanged, with the typed note
//!    [`StageRefusal::note`].
//!
//! What a refusal does **not** cost is the rest of the pack. The pipeline still runs
//! end-to-end and still publishes what the stages that did run established; this is stage 2's
//! own precedent ("a slicer bug costs the guarantee instead of producing a false one") applied
//! to an absent subsystem.
//!
//! # Clause → test
//!
//! | Clause | Source | Test |
//! |---|---|---|
//! | an unbuilt surface refuses rather than degrades | RFC 0026 `rule errors.unsupported_surface` | `every_refusal_is_unsupported_and_irretrievable` |
//! | the two absences are distinguishable | INV-008 | `the_two_absences_have_distinct_tokens` |
//! | the surface decides the inconclusive reason | RFC 0028, "Verdict and assurance" | `each_surface_names_its_own_inconclusive_reason` |
//! | a refusal names the stage it belongs to | RFC 0028, "Compiler pipeline" | `each_surface_names_its_stage` |
//! | notes are a closed vocabulary, never prose | INV-016 | `every_refusal_note_is_distinct` |

use core::fmt;

use continuum_value::assurance::InconclusiveReason;

use crate::omission::{IrretrievableReason, OmissionReason};
use crate::stage::Stage;

/// A compiler input surface RFC 0028 names and this deployment does not have.
///
/// Declared in pipeline-stage order, which is also this type's [`Ord`] — so a map keyed by a
/// surface yields the *earliest refusing stage* first, which is what
/// [`crate::compile::Compilation::inconclusive_reason`] needs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum UnsupportedSurface {
    /// Stage 5's input: the proof dependency slice of the named obligations (RFC 0035,
    /// RFC 0012).
    ProofDependencySlice,
    /// Stage 7's input: the plan §16 correspondence graph and its proof-oriented lenses.
    CorrespondenceGraph,
}

impl UnsupportedSurface {
    /// Both surfaces, in pipeline-stage order.
    pub const ALL: [Self; 2] = [Self::ProofDependencySlice, Self::CorrespondenceGraph];

    /// The RFC 0028 stage this surface is the input of.
    #[must_use]
    pub const fn stage(self) -> Stage {
        match self {
            Self::ProofDependencySlice => Stage::ProofSlicing,
            Self::CorrespondenceGraph => Stage::CorrespondenceMapping,
        }
    }

    /// The subsystem plan §20 names as this surface's producer.
    #[must_use]
    pub const fn producer(self) -> AbsentProducer {
        match self {
            Self::ProofDependencySlice => AbsentProducer::ProofService,
            Self::CorrespondenceGraph => AbsentProducer::CorrespondenceGraph,
        }
    }

    /// The typed `inconclusive_reason` this surface's absence owes the pack.
    ///
    /// > A pack whose question is not about an evaluation outcome […] MUST carry
    /// > `inconclusive` with the reason naming why no verdict is available:
    /// > `IncompleteProofSearch` for an undischarged obligation, `AbstractionAmbiguity` for an
    /// > unresolved correspondence […]. It MUST NOT default to `satisfied`.
    /// >
    /// > — RFC 0028, "Verdict and assurance"
    #[must_use]
    pub const fn inconclusive_reason(self) -> InconclusiveReason {
        match self {
            Self::ProofDependencySlice => InconclusiveReason::IncompleteProofSearch,
            Self::CorrespondenceGraph => InconclusiveReason::AbstractionAmbiguity,
        }
    }

    /// A stable token for an auditable intermediate's rendering.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ProofDependencySlice => "proof-dependency-slice",
            Self::CorrespondenceGraph => "correspondence-graph",
        }
    }
}

impl fmt::Display for UnsupportedSurface {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A subsystem plan §20 names, that this workspace has not built.
///
/// A closed two-member set rather than a free string: the crate name is the thing the
/// integration suite's tripwire reads, and a refusal that named its missing producer in prose
/// could not be checked against the workspace at all.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum AbsentProducer {
    /// RFC 0035's isolated Lean proof service, reached through `continuum-proof-client`.
    ProofService,
    /// Plan §16's model/program correspondence graph, homed in `continuum-refinement`.
    CorrespondenceGraph,
}

impl AbsentProducer {
    /// Both producers, in [`UnsupportedSurface::ALL`] order.
    pub const ALL: [Self; 2] = [Self::ProofService, Self::CorrespondenceGraph];

    /// The plan §20 crate that will hold this producer.
    ///
    /// The integration suite reads exactly this crate's source and fails when it grows a
    /// public surface, so the refusal cannot outlive its reason.
    #[must_use]
    pub const fn crate_name(self) -> &'static str {
        match self {
            Self::ProofService => "continuum-proof-client",
            Self::CorrespondenceGraph => "continuum-refinement",
        }
    }

    /// A stable token.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ProofService => "proof-service",
            Self::CorrespondenceGraph => "correspondence-graph",
        }
    }
}

impl fmt::Display for AbsentProducer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Why a stage's input surface is not available — the INV-008 distinction the refusal keeps.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SurfaceAbsence {
    /// The subsystem that would produce this surface is not built in this deployment.
    ProducerNotDeployed(
        /// The subsystem plan §20 names.
        AbsentProducer,
    ),
    /// The question named nothing for this stage to work on.
    ///
    /// Decided *before* the deployment fact, because it is decidable without consulting the
    /// absent producer: a slice over nothing is vacuous whether or not a slicer exists.
    NothingNamed,
}

impl SurfaceAbsence {
    /// A stable token.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ProducerNotDeployed(_) => "producer-not-deployed",
            Self::NothingNamed => "nothing-named",
        }
    }

    /// The producer this absence names, when it names one.
    #[must_use]
    pub const fn producer(self) -> Option<AbsentProducer> {
        match self {
            Self::ProducerNotDeployed(producer) => Some(producer),
            Self::NothingNamed => None,
        }
    }
}

impl fmt::Display for SurfaceAbsence {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ProducerNotDeployed(producer) => write!(
                f,
                "no `{producer}` is deployed here; plan §20 homes it in `{}`, which is a \
                 PR-1 / IMPL-01 scaffold in this workspace",
                producer.crate_name()
            ),
            Self::NothingNamed => {
                f.write_str("this question names nothing for the stage to work on")
            }
        }
    }
}

/// One stage's refusal to run: which surface is missing, and why.
///
/// Constructed by the stage that refused ([`crate::proof::ProofSlicing::refusal`],
/// [`crate::correspondence::CorrespondenceMapping::refusal`]) and read off
/// [`crate::compile::Compilation::refusals`]. It is deliberately *not* guarded the way
/// [`crate::guarantee::License`] is: a licence is permission to claim something and must be
/// unforgeable, whereas a refusal only ever costs, and a type nobody can build would make the
/// refusal untestable from outside the crate for no gain.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StageRefusal {
    surface: UnsupportedSurface,
    absence: SurfaceAbsence,
}

impl StageRefusal {
    /// Record that `surface` is unavailable for `absence`.
    #[must_use]
    pub const fn of(surface: UnsupportedSurface, absence: SurfaceAbsence) -> Self {
        Self { surface, absence }
    }

    /// The surface that is missing.
    #[must_use]
    pub const fn surface(self) -> UnsupportedSurface {
        self.surface
    }

    /// Why it is missing.
    #[must_use]
    pub const fn absence(self) -> SurfaceAbsence {
        self.absence
    }

    /// The stage that refused.
    #[must_use]
    pub const fn stage(self) -> Stage {
        self.surface.stage()
    }

    /// The manifest reason a candidate only this stage could have decided is recorded under.
    ///
    /// Always `unsupported` — "outside the engine's semantics" is exactly what an unbuilt
    /// producing subsystem is, and it is the reason RFC 0028 C1 names for a guarantee that was
    /// requested and not achieved for want of the semantics.
    #[must_use]
    pub const fn omission_reason(self) -> OmissionReason {
        OmissionReason::Unsupported
    }

    /// The irretrievability this refusal carries.
    ///
    /// `expandable: false`, because there is nothing in this deployment for an expansion to
    /// reach: "the relation is defined for the anchor's kind and the engine cannot compute it"
    /// is `UnsupportedSemanticFeature` at expansion time too (RFC 0028, "Expansion protocol").
    #[must_use]
    pub const fn irretrievable_reason(self) -> IrretrievableReason {
        IrretrievableReason::Unsupported
    }

    /// The typed `inconclusive_reason` this refusal owes the pack.
    #[must_use]
    pub const fn inconclusive_reason(self) -> InconclusiveReason {
        self.surface.inconclusive_reason()
    }

    /// The typed note the refusing stage's auditable intermediate carries.
    ///
    /// A closed vocabulary member's own spelling, never caller-supplied prose (INV-016) — the
    /// discipline [`crate::monitor::MonitorVerdict::as_str`] set for the same slot.
    #[must_use]
    pub const fn note(self) -> &'static str {
        match (self.surface, self.absence) {
            (UnsupportedSurface::ProofDependencySlice, SurfaceAbsence::ProducerNotDeployed(_)) => {
                "proof-slicing-no-producer"
            }
            (UnsupportedSurface::ProofDependencySlice, SurfaceAbsence::NothingNamed) => {
                "proof-slicing-no-obligation"
            }
            (UnsupportedSurface::CorrespondenceGraph, SurfaceAbsence::ProducerNotDeployed(_)) => {
                "correspondence-no-producer"
            }
            (UnsupportedSurface::CorrespondenceGraph, SurfaceAbsence::NothingNamed) => {
                "correspondence-no-subject"
            }
        }
    }
}

impl fmt::Display for StageRefusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "stage {} ({}) cannot run: {}; the stage refuses rather than degrading, guessing, \
             or returning an empty success (RFC 0026 `rule errors.unsupported_surface`)",
            self.stage().number(),
            self.surface,
            self.absence
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn refusals() -> Vec<StageRefusal> {
        let mut all = Vec::new();
        for surface in UnsupportedSurface::ALL {
            all.push(StageRefusal::of(
                surface,
                SurfaceAbsence::ProducerNotDeployed(surface.producer()),
            ));
            all.push(StageRefusal::of(surface, SurfaceAbsence::NothingNamed));
        }
        all
    }

    #[test]
    fn every_refusal_is_unsupported_and_irretrievable() {
        for refusal in refusals() {
            assert_eq!(refusal.omission_reason(), OmissionReason::Unsupported);
            assert_eq!(
                refusal.irretrievable_reason(),
                IrretrievableReason::Unsupported
            );
            // The two spellings agree, so a record cannot claim `unsupported` while its
            // retrievability arm says something else.
            assert_eq!(
                refusal.irretrievable_reason().reason(),
                refusal.omission_reason()
            );
            assert!(refusal.omission_reason().admits_irretrievability());
        }
    }

    #[test]
    fn the_two_absences_have_distinct_tokens() {
        assert_ne!(
            SurfaceAbsence::ProducerNotDeployed(AbsentProducer::ProofService).as_str(),
            SurfaceAbsence::NothingNamed.as_str()
        );
        assert_eq!(
            SurfaceAbsence::ProducerNotDeployed(AbsentProducer::ProofService).producer(),
            Some(AbsentProducer::ProofService)
        );
        assert_eq!(SurfaceAbsence::NothingNamed.producer(), None);
        // And the two are not equal even where the surface is, which is INV-008's whole
        // point: one record, two distinguishable outcomes.
        assert_ne!(
            StageRefusal::of(
                UnsupportedSurface::ProofDependencySlice,
                SurfaceAbsence::ProducerNotDeployed(AbsentProducer::ProofService)
            ),
            StageRefusal::of(
                UnsupportedSurface::ProofDependencySlice,
                SurfaceAbsence::NothingNamed
            )
        );
    }

    #[test]
    fn each_surface_names_its_own_inconclusive_reason() {
        assert_eq!(
            UnsupportedSurface::ProofDependencySlice.inconclusive_reason(),
            InconclusiveReason::IncompleteProofSearch
        );
        assert_eq!(
            UnsupportedSurface::CorrespondenceGraph.inconclusive_reason(),
            InconclusiveReason::AbstractionAmbiguity
        );
        // Neither surface reaches for the generic `Unsupported` token: RFC 0028 names the
        // specific reason for each of these two absences, and a compiler that fell back to
        // the generic one would lose exactly the distinction INV-008 asks for.
        for surface in UnsupportedSurface::ALL {
            assert_ne!(
                surface.inconclusive_reason(),
                InconclusiveReason::Unsupported
            );
        }
    }

    #[test]
    fn each_surface_names_its_stage_and_its_producer() {
        assert_eq!(
            UnsupportedSurface::ProofDependencySlice.stage(),
            Stage::ProofSlicing
        );
        assert_eq!(UnsupportedSurface::ProofDependencySlice.stage().number(), 5);
        assert_eq!(
            UnsupportedSurface::CorrespondenceGraph.stage(),
            Stage::CorrespondenceMapping
        );
        assert_eq!(UnsupportedSurface::CorrespondenceGraph.stage().number(), 7);
        assert_eq!(
            AbsentProducer::ProofService.crate_name(),
            "continuum-proof-client"
        );
        assert_eq!(
            AbsentProducer::CorrespondenceGraph.crate_name(),
            "continuum-refinement"
        );
    }

    #[test]
    fn the_surfaces_sort_into_pipeline_order() {
        // `Compilation::inconclusive_reason` reads the first entry of a map keyed by this
        // type, so the derived `Ord` is load-bearing rather than incidental.
        assert!(UnsupportedSurface::ProofDependencySlice < UnsupportedSurface::CorrespondenceGraph);
        let numbers: Vec<u8> = UnsupportedSurface::ALL
            .into_iter()
            .map(|surface| surface.stage().number())
            .collect();
        assert_eq!(numbers, [5, 7]);
    }

    #[test]
    fn every_refusal_note_is_distinct() {
        let notes: std::collections::BTreeSet<&str> =
            refusals().iter().map(|refusal| refusal.note()).collect();
        assert_eq!(notes.len(), 4);
        // Every note is a token, not a sentence: an auditable intermediate carries no prose.
        for note in notes {
            assert!(!note.contains(' '), "{note:?} must be a token");
        }
    }

    #[test]
    fn a_refusal_says_which_workspace_fact_produced_it() {
        let refusal = StageRefusal::of(
            UnsupportedSurface::ProofDependencySlice,
            SurfaceAbsence::ProducerNotDeployed(AbsentProducer::ProofService),
        );
        let text = refusal.to_string();
        assert!(text.contains("continuum-proof-client"), "{text}");
        assert!(text.contains("stage 5"), "{text}");
    }
}
