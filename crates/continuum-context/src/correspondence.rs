//! **Stage 7**, abstraction/refinement correspondence mapping — and the refusal it is in this
//! workspace (RFC 0028, "Compiler pipeline"; plan §16).
//!
//! # Scope: bn-imhw2 — the compiler's stage group 3 (stages 5–7)
//!
//! > | 7 | abstraction/refinement correspondence mapping | the §16 correspondence graph | the
//! > concrete-to-abstract links selected items carry |
//! >
//! > — RFC 0028, "Compiler pipeline"
//!
//! # There is no §16 correspondence graph here, and that is a checked fact
//!
//! Plan §16.1 describes the input: typed links `Rust type/field/event/effect ↔ operational
//! model component ↔ abstract model component ↔ property observer ↔ Lean definition/theorem`,
//! with §16.2's proof-oriented lenses on top — `get`, an optional `put`, a complement, a
//! consistency relation, ambiguity conditions, generated proof obligations. Plan §20 homes all
//! of it in `continuum-refinement` ("The correspondence graph, proof-oriented lenses,
//! stuttering classification, bounded refinement checking, and drift detection"), which is a
//! PR-1 / IMPL-01 scaffold with no public item. `correspondence.bind` is a registered
//! operation in `continuumd`'s IDL with no producing subsystem behind it — the exact shape
//! `rule errors.unsupported_surface` legislates for.
//!
//! `tests/pr11_compiler_stages_5_7.rs` reads that crate's own bytes and fails the moment it
//! grows a public surface.
//!
//! # This is *not* stage 4's source correspondence
//!
//! The distinction is worth stating once, loudly, because the two share a word.
//! [`crate::dependence::SourceCorrespondence`] is stage 4's input: **untrusted** claims that a
//! source span or a model action corresponds to some execution anchors, which stage 4 admits
//! only where a trusted [`crate::dependence::ExecutionDependence`] attests them (INV-016).
//! Plan §16's correspondence graph is a different artifact with a different trust story: a
//! *typed lens* between a concrete component and an abstract one, carrying a consistency
//! relation and its own generated proof obligations, whose status and evidence differ by
//! whether a link was generated, inferred, or handwritten. Stage 4 landing does not give
//! stage 7 its input, and this module exists partly so that nothing downstream reads it as
//! though it did.
//!
//! # What stage 7 costs, and the manifest cell it deliberately does not invent
//!
//! Stage 7 licenses no member of [`crate::guarantee::Guarantee`] at all — its Licenses column
//! is "the concrete-to-abstract links selected items carry", and at `schema_epoch` 1 a
//! `selected[]` item has no field for such a link (RFC 0028 F2 is the neighbouring gap: items
//! carry no per-item guarantee either). So what the refusal costs is:
//!
//! - **every link**: [`CorrespondenceMapping::subjects`] are the candidates the question asked
//!   for a concrete-to-abstract link for, and with no graph *none of them* is mapped —
//!   [`crate::compile::Compilation::unmapped_correspondence`] is the whole subject set, and no
//!   pack this crate can build carries a link, because there is no constructor for one;
//! - **the verdict**: `InconclusiveReason::AbstractionAmbiguity`, RFC 0028's named reason "for
//!   an unresolved correspondence", so an invariant or refinement pack cannot default to
//!   `satisfied`.
//!
//! What it does **not** cost is a manifest record, and that omission is deliberate rather than
//! an oversight. The manifest partitions **candidates**: "the size of the candidate set equals
//! the size of the selection plus the sum of the manifest counts, and the partition of the
//! unselected candidates by kind and reason is exact". Stage 7 drops no candidate — a link is
//! not a candidate, and a subject the pack does publish is published all the same, merely
//! without its abstract counterpart. Adding a cell for the missing links would make the
//! counting equation false in the direction that looks like diligence, and would put a fact in
//! the artifact contract that RFC 0028's own required-fields table has no home for. The
//! refusal is recorded in the trail, in [`crate::compile::Compilation::refusals`], and in the
//! verdict's reason — three places, all of which a consumer reads, and none of which is the
//! candidate partition.
//!
//! # Stage 7 does not narrow, either
//!
//! Same reason as stage 5: acting on behalf of a subsystem that does not exist is the guessing
//! `rule errors.unsupported_surface` prohibits, and a drop nothing decided cannot be
//! attributed to a deciding stage.
//!
//! # Clause → test
//!
//! | Clause | Source | Test |
//! |---|---|---|
//! | stage 7 refuses rather than degrading | RFC 0026 `rule errors.unsupported_surface` | `stage_seven_refuses_in_this_deployment` |
//! | the refusal names the absent §16 graph | plan §16; plan §20 | `the_refusal_names_the_absent_correspondence_graph` |
//! | no subject named is its own absence | INV-008 | `a_mapping_with_no_subject_is_a_different_absence` |
//! | an unresolved correspondence is `AbstractionAmbiguity` | RFC 0028, "Verdict and assurance" | `stage_seven_refuses_in_this_deployment` |
//! | every subject is unmapped | RFC 0028, "Compiler pipeline" | `every_subject_is_unmapped_because_nothing_maps` |

use std::collections::BTreeSet;

use continuum_value::value::Name;

use crate::unsupported::{AbsentProducer, StageRefusal, SurfaceAbsence, UnsupportedSurface};

/// **Stage 7's premise**: the candidates the question asks for a concrete-to-abstract link for.
///
/// A value the compiler is handed, for exactly [`crate::causal::CausalOrder`]'s reason.
/// [`crate::compile::CausalCompile::run`] checks each subject *is* a candidate of the order —
/// structure is checked even where content cannot be — and refuses one from nowhere with
/// [`crate::compile::CompileError::Causal`], the discipline stage 4's join already follows.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CorrespondenceMapping {
    subjects: BTreeSet<Name>,
}

impl CorrespondenceMapping {
    /// Ask stage 7 to map these candidates onto their abstract counterparts.
    #[must_use]
    pub fn of(subjects: impl IntoIterator<Item = Name>) -> Self {
        Self {
            subjects: subjects.into_iter().collect(),
        }
    }

    /// Ask stage 7 to run with no subject named.
    #[must_use]
    pub fn nothing_named() -> Self {
        Self {
            subjects: BTreeSet::new(),
        }
    }

    /// The subjects, in canonical order.
    #[must_use]
    pub const fn subjects(&self) -> &BTreeSet<Name> {
        &self.subjects
    }

    /// How many subjects were named.
    #[must_use]
    pub fn len(&self) -> usize {
        self.subjects.len()
    }

    /// Whether the question named no subject at all.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.subjects.is_empty()
    }

    /// The subjects this stage could not map — **all of them**, because nothing maps.
    ///
    /// A method rather than a field so that the day `continuum-refinement` lands, the
    /// difference between "asked for" and "mapped" has a place to appear, and the tripwire in
    /// `tests/pr11_compiler_stages_5_7.rs` has already forced this file to be revisited by
    /// then.
    #[must_use]
    pub fn unmapped(&self) -> BTreeSet<Name> {
        self.subjects.clone()
    }

    /// Stage 7's outcome in this deployment: a typed refusal, always.
    ///
    /// The question's own defect is decided first, as in [`crate::proof::ProofSlicing::refusal`].
    #[must_use]
    pub fn refusal(&self) -> StageRefusal {
        let absence = if self.subjects.is_empty() {
            SurfaceAbsence::NothingNamed
        } else {
            SurfaceAbsence::ProducerNotDeployed(AbsentProducer::CorrespondenceGraph)
        };
        StageRefusal::of(UnsupportedSurface::CorrespondenceGraph, absence)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use continuum_value::assurance::InconclusiveReason;

    fn name(text: &str) -> Name {
        Name::new(text).expect("well formed")
    }

    #[test]
    fn stage_seven_refuses_in_this_deployment() {
        let refusal = CorrespondenceMapping::of([name("e_ack")]).refusal();
        assert_eq!(refusal.surface(), UnsupportedSurface::CorrespondenceGraph);
        assert_eq!(refusal.stage().number(), 7);
        assert_eq!(
            refusal.inconclusive_reason(),
            InconclusiveReason::AbstractionAmbiguity
        );
    }

    #[test]
    fn the_refusal_names_the_absent_correspondence_graph() {
        let refusal = CorrespondenceMapping::of([name("e_ack")]).refusal();
        assert_eq!(
            refusal.absence(),
            SurfaceAbsence::ProducerNotDeployed(AbsentProducer::CorrespondenceGraph)
        );
        assert_eq!(
            refusal
                .absence()
                .producer()
                .expect("a named producer")
                .crate_name(),
            "continuum-refinement"
        );
        assert_eq!(refusal.note(), "correspondence-no-producer");
    }

    #[test]
    fn a_mapping_with_no_subject_is_a_different_absence() {
        let refusal = CorrespondenceMapping::nothing_named().refusal();
        assert_eq!(refusal.absence(), SurfaceAbsence::NothingNamed);
        assert_eq!(refusal.note(), "correspondence-no-subject");
        assert_ne!(
            refusal,
            CorrespondenceMapping::of([name("e_ack")]).refusal()
        );
        // The reason a pack cannot default to `satisfied` is the same either way.
        assert_eq!(
            refusal.inconclusive_reason(),
            InconclusiveReason::AbstractionAmbiguity
        );
    }

    #[test]
    fn every_subject_is_unmapped_because_nothing_maps() {
        let mapping = CorrespondenceMapping::of([name("e_ack"), name("m_commit")]);
        assert_eq!(mapping.unmapped(), *mapping.subjects());
        assert_eq!(mapping.len(), 2);
        assert!(!mapping.is_empty());
        // The empty case is the empty set rather than "nothing to say".
        assert!(CorrespondenceMapping::nothing_named().unmapped().is_empty());
    }

    #[test]
    fn the_subjects_are_carried_in_canonical_order() {
        let mapping = CorrespondenceMapping::of([name("m_commit"), name("e_ack")]);
        assert_eq!(
            mapping.subjects().iter().cloned().collect::<Vec<_>>(),
            [name("e_ack"), name("m_commit")]
        );
    }

    #[test]
    fn two_readings_of_one_mapping_agree() {
        let mapping = CorrespondenceMapping::of([name("e_ack")]);
        assert_eq!(mapping.refusal(), mapping.refusal());
        assert_eq!(mapping, CorrespondenceMapping::of([name("e_ack")]));
    }
}
