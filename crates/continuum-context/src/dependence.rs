//! **Stage 4**: the static/dynamic dependence join, and the admissibility of `source` and
//! `model` items (RFC 0028, "Compiler pipeline" stage 4; INV-016).
//!
//! # Scope: bn-1kj2n — the compiler's stage group 2 (stages 3–4)
//!
//! > | 4 | static/dynamic dependence join | source correspondence | admissibility of `source`
//! > and `model` items |
//! >
//! > — RFC 0028, "Compiler pipeline"
//!
//! Stage 4 is the one stage in the table whose "Licenses" column is not a guarantee. It
//! decides **admissibility**: whether a `source` span or a `model` action may enter a pack's
//! `selected[]` at all. Nothing here claims a member of the closed thirteen-member
//! [`crate::guarantee::Guarantee`] set, and nothing here issues a
//! [`crate::guarantee::License`].
//!
//! # INV-016 decides the shape, and it is a type here rather than a review note
//!
//! > relevant source spans and model actions | `selected[].kind` in `source`, `model` | array
//! > items | Source is untrusted data and MUST NOT be interpolated into any description
//! > (INV-016)
//! >
//! > — RFC 0028, "Required fields, reconciled with plan §6.2"
//!
//! INV-016 has two halves, and this module pays both.
//!
//! **No interpolation.** The only way an item of kind `source` or `model` can be built in this
//! crate is [`crate::source::SourceRef::into_selected_item`] or
//! [`crate::model::ModelActionRef::into_selected_item`], whose summaries are pure functions of
//! a typed span or a typed action name — neither takes a string parameter, and
//! `crate::selection::SelectedItem::new` is `pub(crate)`. [`CorrespondenceRef`] is a two-member
//! sum of exactly those two constructors and adds no third path, so
//! [`Admissible::into_selected_item`] cannot carry file content into a pack even if a caller
//! wanted it to.
//!
//! **No silent upgrade.** A dependence *claimed* by a source correspondence is untrusted input:
//! it is derived from comments, annotations, generated maps and analyses over files this
//! workbench did not write. A dependence *attested* by the execution record is not. The two
//! are therefore two different types — [`SourceCorrespondence`] and [`ExecutionDependence`] —
//! and there is no conversion between them, so a caller cannot pass the untrusted side where
//! the trusted side is expected. Corroboration is an explicit act of attestation, never an
//! implicit promotion, and [`DependenceJoin::admissibility`] refuses a claim the execution
//! never attested with [`Inadmissible::UncorroboratedSource`].
//!
//! # Structure is checked; assertions are corroborated
//!
//! The line this module draws, stated once because it decides several behaviours:
//!
//! - The *structure* of the join — which candidates it makes claims about — is the daemon's,
//!   and a claim naming a candidate this compile never held, or naming one whose kind is not
//!   `source` or `model`, is a structural mismatch that [`crate::compile::CausalCompile::run`]
//!   refuses with a typed error.
//! - The *content* of a claim — the anchors it asserts a dependence on — is untrusted, so it
//!   is never obeyed and never fatal. An anchor naming something that does not exist simply
//!   fails to corroborate; it does not fail the compile, and it cannot add a candidate to the
//!   pack. Untrusted input has no path to control flow and no path to the candidate set.
//!
//! # Every decline is `heuristic-cutoff`, and never `slice-irrelevant`
//!
//! > `slice-irrelevant` is reserved for items *provably* outside the property-directed slice;
//! > an item dropped because the compiler could not decide is `heuristic-cutoff`, never
//! > `slice-irrelevant`.
//! >
//! > — RFC 0028, "Omission manifest"
//!
//! [`Inadmissible::omission_reason`] is `heuristic-cutoff` for all three members, and that is
//! this module's own reading where RFC 0028 is silent, stated rather than assumed:
//!
//! - Stage 4 licenses admissibility, not irrelevance. Proving an item outside the
//!   property-directed slice is stage 3's decision ([`crate::property::Coverage`]), and an
//!   item stage 4 declined is one stage 4 could not *admit* — which is the definition of "the
//!   compiler could not decide".
//! - For [`Inadmissible::UncorroboratedSource`] the reading is forced rather than merely
//!   conservative. An uncorroborated claim is *undecided*, never *disproved*: recording it as
//!   provably irrelevant would be the pack asserting a proof about untrusted input, which is
//!   the same silent upgrade INV-016 forbids, pointed the other way. So even a join whose
//!   execution side is exhaustive declines to `heuristic-cutoff`.
//!
//! The consequence is deliberate and is worth naming: a compile with a complete causal order
//! and a total automaton coverage still produces `heuristic-cutoff` records where stage 4
//! declined, so one manifest carries both reasons and each is traceable to the stage that
//! decided it.
//!
//! # What is declined here
//!
//! - **Assembling the `selected[]` array.** [`Admissible::into_selected_item`] produces the
//!   items; deciding their `id`s, their order in the published array, and the pack they land in
//!   is whole-pack assembly's, which is bn-1y4qc's bullet.
//! - **Producing a source correspondence.** Plan §16's correspondence graph and
//!   `correspondence.bind` are not in this workspace; the join is a value the compiler is
//!   handed, for the same reason [`crate::causal::CausalOrder`] is.
//! - **A `counterfactual` admissibility rule.** RFC 0028 requires
//!   `CounterfactualUnderNamedModel` and a named causality model for that kind; stage 4's
//!   column names `source` and `model` and nothing else, and widening it here would be
//!   vocabulary this bone was not asked to invent.
//!
//! # Clause → test
//!
//! | Clause | Source | Test |
//! |---|---|---|
//! | the untrusted side cannot stand in for the trusted one | INV-016 | the two types, and `an_untrusted_claim_alone_is_not_admissible` |
//! | an uncorroborated claim does not upgrade | INV-016 | `an_untrusted_claim_alone_is_not_admissible` |
//! | corroboration is not vacuous | anti-vacuity | `the_same_claim_attested_by_the_execution_is_admissible` |
//! | an anchor outside the slice is not admissible | RFC 0028 stage 4 | `a_corroborated_claim_whose_anchor_is_unpublished_is_not_admissible` |
//! | an unknown anchor is ignored, not obeyed | INV-016 | `an_anchor_naming_nothing_is_not_corroborated` |
//! | every decline is `heuristic-cutoff` | RFC 0028, "Omission manifest" | `every_decline_is_heuristic_cutoff` |
//! | the summary is a position, never a snippet | INV-016 | `an_admissible_item_summarises_its_span_and_nothing_else` |

use core::fmt;
use std::collections::{BTreeMap, BTreeSet};

use continuum_value::value::Name;

use crate::model::ModelActionRef;
use crate::omission::OmissionReason;
use crate::selection::{SelectedItem, SelectionKind};
use crate::source::SourceRef;

/// A typed reference to what a `source` or `model` candidate names.
///
/// Exactly two members, because [`crate::source::SourceRef`] and
/// [`crate::model::ModelActionRef`] are the crate's only typed constructors for those two
/// kinds, and this type adds no third path into a pack (INV-016).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum CorrespondenceRef {
    /// A source span: `selected[].kind = "source"`.
    Source(SourceRef),
    /// A model action: `selected[].kind = "model"`.
    Model(ModelActionRef),
}

impl CorrespondenceRef {
    /// The selection kind this reference projects to.
    #[must_use]
    pub const fn kind(&self) -> SelectionKind {
        match self {
            Self::Source(_) => SelectionKind::Source,
            Self::Model(_) => SelectionKind::Model,
        }
    }

    /// Project into the pack's generic `selected[]` shape, under `id`.
    ///
    /// Delegates to the landed constructor for the kind; there is no string parameter here
    /// and none in either delegate (INV-016).
    #[must_use]
    pub fn into_selected_item(self, id: Name) -> SelectedItem {
        match self {
            Self::Source(reference) => reference.into_selected_item(id),
            Self::Model(reference) => reference.into_selected_item(id),
        }
    }
}

/// One claim from a source correspondence: a typed reference, and the candidates it asserts a
/// dependence on.
///
/// The anchors are the untrusted half. They are corroborated against
/// [`ExecutionDependence`], never believed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CorrespondenceClaim {
    reference: CorrespondenceRef,
    anchors: BTreeSet<Name>,
}

impl CorrespondenceClaim {
    /// A claim that `reference` bears on `anchors`.
    #[must_use]
    pub fn new(reference: CorrespondenceRef, anchors: impl IntoIterator<Item = Name>) -> Self {
        Self {
            reference,
            anchors: anchors.into_iter().collect(),
        }
    }

    /// What the claim refers to.
    #[must_use]
    pub const fn reference(&self) -> &CorrespondenceRef {
        &self.reference
    }

    /// The candidates the claim asserts a dependence on, in canonical order.
    #[must_use]
    pub const fn anchors(&self) -> &BTreeSet<Name> {
        &self.anchors
    }
}

/// The **static** side of the join: what a source correspondence claims.
///
/// Untrusted (INV-016). There is deliberately no conversion from this type to
/// [`ExecutionDependence`].
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SourceCorrespondence {
    claims: BTreeMap<Name, CorrespondenceClaim>,
}

impl SourceCorrespondence {
    /// A correspondence that claims nothing.
    #[must_use]
    pub fn none() -> Self {
        Self {
            claims: BTreeMap::new(),
        }
    }

    /// A correspondence over the given claims, one per candidate.
    ///
    /// # Errors
    ///
    /// [`DependenceError::RepeatedClaim`] when one candidate is claimed twice — two claims
    /// about one candidate would make "the dependence the correspondence asserts" ambiguous,
    /// and an ambiguity in untrusted input is resolved by refusing it, never by picking one;
    /// and [`DependenceError::AnchorlessClaim`] for a claim with no anchors, which asserts no
    /// dependence and so cannot be corroborated or refuted.
    pub fn of(
        claims: impl IntoIterator<Item = (Name, CorrespondenceClaim)>,
    ) -> Result<Self, DependenceError> {
        let mut members: BTreeMap<Name, CorrespondenceClaim> = BTreeMap::new();
        for (id, claim) in claims {
            if claim.anchors().is_empty() {
                return Err(DependenceError::AnchorlessClaim { id });
            }
            if members.insert(id.clone(), claim).is_some() {
                return Err(DependenceError::RepeatedClaim { id });
            }
        }
        Ok(Self { claims: members })
    }

    /// The claim about one candidate, when there is one.
    #[must_use]
    pub fn claim(&self, id: &Name) -> Option<&CorrespondenceClaim> {
        self.claims.get(id)
    }

    /// Every claimed candidate, in canonical order.
    pub fn claimed(&self) -> impl Iterator<Item = &Name> {
        self.claims.keys()
    }

    /// How many candidates are claimed.
    #[must_use]
    pub fn len(&self) -> usize {
        self.claims.len()
    }

    /// Whether nothing is claimed.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.claims.is_empty()
    }
}

/// The **dynamic** side of the join: the dependences the execution record attests.
///
/// Trusted, and constructed only by naming what actually ran. This is the type a
/// [`SourceCorrespondence`] cannot become.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ExecutionDependence {
    attested: BTreeMap<Name, BTreeSet<Name>>,
}

impl ExecutionDependence {
    /// An execution record that attests nothing.
    #[must_use]
    pub fn none() -> Self {
        Self {
            attested: BTreeMap::new(),
        }
    }

    /// An execution record attesting, for each candidate, the anchors it was observed to bear
    /// on.
    ///
    /// # Errors
    ///
    /// [`DependenceError::RepeatedAttestation`] when one candidate is attested twice.
    pub fn attesting(
        entries: impl IntoIterator<Item = (Name, BTreeSet<Name>)>,
    ) -> Result<Self, DependenceError> {
        let mut attested: BTreeMap<Name, BTreeSet<Name>> = BTreeMap::new();
        for (id, anchors) in entries {
            if attested.insert(id.clone(), anchors).is_some() {
                return Err(DependenceError::RepeatedAttestation { id });
            }
        }
        Ok(Self { attested })
    }

    /// The anchors the execution attests for one candidate.
    #[must_use]
    pub fn anchors(&self, id: &Name) -> Option<&BTreeSet<Name>> {
        self.attested.get(id)
    }

    /// Whether the execution attests this one dependence.
    #[must_use]
    pub fn attests(&self, id: &Name, anchor: &Name) -> bool {
        self.attested
            .get(id)
            .is_some_and(|anchors| anchors.contains(anchor))
    }
}

/// Stage 4's computation: the join of an untrusted static correspondence with a trusted
/// dynamic one.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DependenceJoin {
    correspondence: SourceCorrespondence,
    execution: ExecutionDependence,
}

impl DependenceJoin {
    /// Join a correspondence with an execution record.
    #[must_use]
    pub const fn new(correspondence: SourceCorrespondence, execution: ExecutionDependence) -> Self {
        Self {
            correspondence,
            execution,
        }
    }

    /// The untrusted static side.
    #[must_use]
    pub const fn correspondence(&self) -> &SourceCorrespondence {
        &self.correspondence
    }

    /// The trusted dynamic side.
    #[must_use]
    pub const fn execution(&self) -> &ExecutionDependence {
        &self.execution
    }

    /// Every candidate the correspondence claims, in canonical order.
    ///
    /// These are the candidates stage 4 decides about; a candidate nothing claims is not
    /// stage 4's to disposition.
    pub fn claimed(&self) -> impl Iterator<Item = &Name> {
        self.correspondence.claimed()
    }

    /// The typed reference for a claimed candidate.
    #[must_use]
    pub fn reference(&self, id: &Name) -> Option<&CorrespondenceRef> {
        self.correspondence
            .claim(id)
            .map(CorrespondenceClaim::reference)
    }

    /// Decide whether `candidate` may enter a pack whose published core is `selection`.
    ///
    /// The three refusals, in the order they are decided:
    ///
    /// 1. Nothing claims the candidate → [`Inadmissible::NoCorrespondence`].
    /// 2. The execution attests none of the claimed anchors →
    ///    [`Inadmissible::UncorroboratedSource`]. Decided *before* the slice is consulted, so
    ///    an uncorroborated claim never gets partial credit for pointing at a published item.
    /// 3. Every corroborated anchor is outside the published core →
    ///    [`Inadmissible::AnchorNotSelected`]. The item has nothing in the pack to attach to,
    ///    and "expansion is navigation over a published pack" (RFC 0028).
    #[must_use]
    pub fn admissibility(&self, candidate: &Name, selection: &BTreeSet<Name>) -> Admissibility {
        let Some(claim) = self.correspondence.claim(candidate) else {
            return Admissibility::Inadmissible(Inadmissible::NoCorrespondence);
        };
        let corroborated: BTreeSet<Name> = claim
            .anchors()
            .iter()
            .filter(|anchor| self.execution.attests(candidate, anchor))
            .cloned()
            .collect();
        if corroborated.is_empty() {
            return Admissibility::Inadmissible(Inadmissible::UncorroboratedSource);
        }
        let published: BTreeSet<Name> = corroborated.intersection(selection).cloned().collect();
        if published.is_empty() {
            return Admissibility::Inadmissible(Inadmissible::AnchorNotSelected);
        }
        Admissibility::Admissible(Admissible {
            reference: claim.reference().clone(),
            anchors: published,
        })
    }
}

/// Stage 4's decision about one `source` or `model` candidate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Admissibility {
    /// The candidate may enter the pack, and the anchors that carry it are named.
    Admissible(Admissible),
    /// The candidate may not, and the reason is typed.
    Inadmissible(Inadmissible),
}

impl Admissibility {
    /// Whether the candidate may enter the pack.
    #[must_use]
    pub const fn is_admissible(&self) -> bool {
        matches!(self, Self::Admissible(_))
    }

    /// The typed refusal, when there is one.
    #[must_use]
    pub const fn inadmissible(&self) -> Option<Inadmissible> {
        match self {
            Self::Admissible(_) => None,
            Self::Inadmissible(reason) => Some(*reason),
        }
    }

    /// The admitted item, when there is one.
    #[must_use]
    pub const fn admissible(&self) -> Option<&Admissible> {
        match self {
            Self::Admissible(admitted) => Some(admitted),
            Self::Inadmissible(_) => None,
        }
    }
}

/// An admitted `source` or `model` candidate: its typed reference, and the published anchors
/// that corroborate it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Admissible {
    reference: CorrespondenceRef,
    anchors: BTreeSet<Name>,
}

impl Admissible {
    /// The typed reference.
    #[must_use]
    pub const fn reference(&self) -> &CorrespondenceRef {
        &self.reference
    }

    /// The published anchors that corroborate the item, in canonical order.
    #[must_use]
    pub const fn anchors(&self) -> &BTreeSet<Name> {
        &self.anchors
    }

    /// The selection kind this item projects to.
    #[must_use]
    pub const fn kind(&self) -> SelectionKind {
        self.reference.kind()
    }

    /// Project into the pack's generic `selected[]` shape, under `id`.
    #[must_use]
    pub fn into_selected_item(self, id: Name) -> SelectedItem {
        self.reference.into_selected_item(id)
    }
}

/// Why a `source` or `model` candidate is not admissible.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Inadmissible {
    /// No correspondence claims the candidate at all.
    NoCorrespondence,
    /// Only the untrusted source correspondence claims the dependence; the execution record
    /// attests none of it. INV-016's load-bearing case: the claim does not upgrade.
    UncorroboratedSource,
    /// The dependence is corroborated, and no anchor carrying it is in the published core.
    AnchorNotSelected,
}

impl Inadmissible {
    /// All three members, in declaration order.
    pub const ALL: [Self; 3] = [
        Self::NoCorrespondence,
        Self::UncorroboratedSource,
        Self::AnchorNotSelected,
    ];

    /// A stable token for the auditable intermediate's rendering.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NoCorrespondence => "no-correspondence",
            Self::UncorroboratedSource => "uncorroborated-source",
            Self::AnchorNotSelected => "anchor-not-selected",
        }
    }

    /// The manifest reason a stage-4 decline is recorded under.
    ///
    /// Always `heuristic-cutoff`; see the module documentation's "Every decline is
    /// `heuristic-cutoff`, and never `slice-irrelevant`".
    #[must_use]
    pub const fn omission_reason(self) -> OmissionReason {
        OmissionReason::HeuristicCutoff
    }
}

impl fmt::Display for Inadmissible {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoCorrespondence => f.write_str("no source correspondence claims this candidate"),
            Self::UncorroboratedSource => f.write_str(
                "the dependence is claimed only by the source correspondence, which is \
                 untrusted data; the execution record attests none of it (INV-016)",
            ),
            Self::AnchorNotSelected => f.write_str(
                "every corroborated anchor is outside the published core, so the item has \
                 nothing in the pack to attach to",
            ),
        }
    }
}

/// A way a dependence join fails to be one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DependenceError {
    /// One candidate is claimed twice by the correspondence.
    RepeatedClaim {
        /// The candidate claimed twice.
        id: Name,
    },
    /// A claim asserts no dependence at all.
    AnchorlessClaim {
        /// The candidate whose claim has no anchors.
        id: Name,
    },
    /// One candidate is attested twice by the execution record.
    RepeatedAttestation {
        /// The candidate attested twice.
        id: Name,
    },
}

impl fmt::Display for DependenceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RepeatedClaim { id } => write!(
                f,
                "the source correspondence claims `{id}` twice; an ambiguity in untrusted \
                 input is refused, never resolved by picking one (INV-016)"
            ),
            Self::AnchorlessClaim { id } => write!(
                f,
                "the correspondence's claim about `{id}` names no anchor, so it asserts no \
                 dependence and can be neither corroborated nor refuted"
            ),
            Self::RepeatedAttestation { id } => {
                write!(f, "the execution record attests `{id}` twice")
            }
        }
    }
}

impl core::error::Error for DependenceError {}

#[cfg(test)]
mod tests {
    use super::*;
    use continuum_workspace::snapshot::WorkspacePath;

    fn name(text: &str) -> Name {
        Name::new(text).expect("well formed")
    }

    fn ids(names: &[&str]) -> BTreeSet<Name> {
        names.iter().map(|id| name(id)).collect()
    }

    fn span(file: &str, line: u32) -> SourceRef {
        SourceRef::new(
            crate::source::SourceSpan::new(
                WorkspacePath::new(file).expect("well-formed path"),
                line,
                1,
                line,
                9,
            )
            .expect("well-ordered span"),
        )
    }

    fn source_claim(file: &str, line: u32, anchors: &[&str]) -> CorrespondenceClaim {
        CorrespondenceClaim::new(
            CorrespondenceRef::Source(span(file, line)),
            anchors.iter().map(|id| name(id)),
        )
    }

    fn model_claim(action: &str, anchors: &[&str]) -> CorrespondenceClaim {
        CorrespondenceClaim::new(
            CorrespondenceRef::Model(ModelActionRef::action_only(name(action))),
            anchors.iter().map(|id| name(id)),
        )
    }

    /// `s_writer` is corroborated on `e_ack`; `m_commit` is claimed on `e_submit` and
    /// attested nowhere; `s_ghost` is corroborated on `e_probe`, which the core drops.
    fn join() -> DependenceJoin {
        DependenceJoin::new(
            SourceCorrespondence::of([
                (
                    name("s_writer"),
                    source_claim("crates/a/src/lib.rs", 12, &["e_ack"]),
                ),
                (name("m_commit"), model_claim("Commit", &["e_submit"])),
                (
                    name("s_ghost"),
                    source_claim("crates/a/src/old.rs", 3, &["e_probe"]),
                ),
            ])
            .expect("a well-formed correspondence"),
            ExecutionDependence::attesting([
                (name("s_writer"), ids(&["e_ack"])),
                (name("s_ghost"), ids(&["e_probe"])),
            ])
            .expect("a well-formed execution record"),
        )
    }

    fn core() -> BTreeSet<Name> {
        ids(&["e_begin", "e_submit", "e_ack", "e_loss"])
    }

    #[test]
    fn a_corroborated_claim_whose_anchor_is_published_is_admissible() {
        let admissibility = join().admissibility(&name("s_writer"), &core());
        assert!(admissibility.is_admissible());
        let admitted = admissibility.admissible().expect("admitted");
        assert_eq!(admitted.anchors(), &ids(&["e_ack"]));
        assert_eq!(admitted.kind(), SelectionKind::Source);
        assert_eq!(admissibility.inadmissible(), None);
    }

    /// The load-bearing INV-016 negative: a dependence nothing but the untrusted source
    /// correspondence asserts does not become a selected item.
    #[test]
    fn an_untrusted_claim_alone_is_not_admissible() {
        let admissibility = join().admissibility(&name("m_commit"), &core());
        assert_eq!(
            admissibility,
            Admissibility::Inadmissible(Inadmissible::UncorroboratedSource)
        );
        assert!(!admissibility.is_admissible());
        assert!(admissibility.admissible().is_none());
        // And the anchor it named *is* published, so nothing about the slice saved it: the
        // refusal is about who attested the dependence.
        assert!(core().contains(&name("e_submit")));
    }

    /// Anti-vacuity for the rule above: the same claim, attested by the execution record,
    /// *is* admissible. The check discriminates rather than refusing everything — and the
    /// only way to get here is to construct an `ExecutionDependence`, which is an explicit
    /// act of attestation with no conversion from the untrusted side.
    #[test]
    fn the_same_claim_attested_by_the_execution_is_admissible() {
        let upgraded = DependenceJoin::new(
            SourceCorrespondence::of([(name("m_commit"), model_claim("Commit", &["e_submit"]))])
                .expect("a well-formed correspondence"),
            ExecutionDependence::attesting([(name("m_commit"), ids(&["e_submit"]))])
                .expect("a well-formed execution record"),
        );
        let admissibility = upgraded.admissibility(&name("m_commit"), &core());
        assert!(admissibility.is_admissible());
        assert_eq!(
            admissibility.admissible().expect("admitted").kind(),
            SelectionKind::Model
        );
    }

    #[test]
    fn a_corroborated_claim_whose_anchor_is_unpublished_is_not_admissible() {
        assert_eq!(
            join().admissibility(&name("s_ghost"), &core()),
            Admissibility::Inadmissible(Inadmissible::AnchorNotSelected)
        );
    }

    #[test]
    fn an_unclaimed_candidate_has_no_correspondence() {
        assert_eq!(
            join().admissibility(&name("s_other"), &core()),
            Admissibility::Inadmissible(Inadmissible::NoCorrespondence)
        );
    }

    #[test]
    fn an_anchor_naming_nothing_is_not_corroborated() {
        // Untrusted input naming a candidate that does not exist is data, not a command: it
        // fails to corroborate and nothing else happens.
        let fabricated = DependenceJoin::new(
            SourceCorrespondence::of([(
                name("s_writer"),
                source_claim("crates/a/src/lib.rs", 1, &["e_nowhere"]),
            )])
            .expect("a well-formed correspondence"),
            ExecutionDependence::none(),
        );
        assert_eq!(
            fabricated.admissibility(&name("s_writer"), &core()),
            Admissibility::Inadmissible(Inadmissible::UncorroboratedSource)
        );
    }

    #[test]
    fn an_execution_attestation_the_correspondence_never_claimed_admits_nothing() {
        // The join is a conjunction, and the trusted side alone is not a claim either: there
        // is no typed reference to build an item from.
        let dynamic_only = DependenceJoin::new(
            SourceCorrespondence::none(),
            ExecutionDependence::attesting([(name("s_writer"), ids(&["e_ack"]))])
                .expect("well formed"),
        );
        assert_eq!(
            dynamic_only.admissibility(&name("s_writer"), &core()),
            Admissibility::Inadmissible(Inadmissible::NoCorrespondence)
        );
    }

    #[test]
    fn a_partly_corroborated_claim_is_admitted_on_its_corroborated_anchor_alone() {
        let mixed = DependenceJoin::new(
            SourceCorrespondence::of([(
                name("s_writer"),
                source_claim("crates/a/src/lib.rs", 12, &["e_ack", "e_nowhere"]),
            )])
            .expect("well formed"),
            ExecutionDependence::attesting([(name("s_writer"), ids(&["e_ack"]))])
                .expect("well formed"),
        );
        let admissibility = mixed.admissibility(&name("s_writer"), &core());
        assert_eq!(
            admissibility.admissible().expect("admitted").anchors(),
            &ids(&["e_ack"])
        );
    }

    #[test]
    fn a_repeated_claim_is_refused() {
        assert_eq!(
            SourceCorrespondence::of([
                (name("s_writer"), source_claim("a.rs", 1, &["e_ack"])),
                (name("s_writer"), source_claim("b.rs", 2, &["e_ack"])),
            ]),
            Err(DependenceError::RepeatedClaim {
                id: name("s_writer")
            })
        );
    }

    #[test]
    fn an_anchorless_claim_is_refused() {
        assert_eq!(
            SourceCorrespondence::of([(name("s_writer"), source_claim("a.rs", 1, &[]))]),
            Err(DependenceError::AnchorlessClaim {
                id: name("s_writer")
            })
        );
    }

    #[test]
    fn a_repeated_attestation_is_refused() {
        assert_eq!(
            ExecutionDependence::attesting([
                (name("s_writer"), ids(&["e_ack"])),
                (name("s_writer"), ids(&["e_loss"])),
            ]),
            Err(DependenceError::RepeatedAttestation {
                id: name("s_writer")
            })
        );
    }

    #[test]
    fn every_decline_is_heuristic_cutoff() {
        for reason in Inadmissible::ALL {
            assert_eq!(reason.omission_reason(), OmissionReason::HeuristicCutoff);
        }
        let tokens: BTreeSet<&str> = Inadmissible::ALL
            .into_iter()
            .map(Inadmissible::as_str)
            .collect();
        assert_eq!(tokens.len(), Inadmissible::ALL.len());
    }

    #[test]
    fn an_admissible_item_summarises_its_span_and_nothing_else() {
        let admitted = join()
            .admissibility(&name("s_writer"), &core())
            .admissible()
            .expect("admitted")
            .clone();
        let item = admitted.into_selected_item(name("s_writer"));
        assert_eq!(item.kind(), SelectionKind::Source);
        assert_eq!(item.summary(), "crates/a/src/lib.rs:12:1-12:9");
        assert_eq!(item.artifact(), None);
    }

    #[test]
    fn a_model_item_summarises_its_action_and_nothing_else() {
        let upgraded = DependenceJoin::new(
            SourceCorrespondence::of([(name("m_commit"), model_claim("Commit", &["e_submit"]))])
                .expect("well formed"),
            ExecutionDependence::attesting([(name("m_commit"), ids(&["e_submit"]))])
                .expect("well formed"),
        );
        let item = upgraded
            .admissibility(&name("m_commit"), &core())
            .admissible()
            .expect("admitted")
            .clone()
            .into_selected_item(name("m_commit"));
        assert_eq!(item.kind(), SelectionKind::Model);
        assert_eq!(item.summary(), "model action `Commit`");
    }

    #[test]
    fn the_join_reports_what_it_claims() {
        let join = join();
        let claimed: Vec<&Name> = join.claimed().collect();
        assert_eq!(
            claimed,
            [&name("s_ghost"), &name("m_commit"), &name("s_writer")]
        );
        assert_eq!(join.correspondence().len(), 3);
        assert!(!join.correspondence().is_empty());
        assert!(SourceCorrespondence::none().is_empty());
        assert!(join.execution().attests(&name("s_writer"), &name("e_ack")));
        assert!(
            !join
                .execution()
                .attests(&name("m_commit"), &name("e_submit"))
        );
        assert_eq!(
            join.execution().anchors(&name("s_writer")),
            Some(&ids(&["e_ack"]))
        );
        assert_eq!(
            join.reference(&name("m_commit"))
                .map(CorrespondenceRef::kind),
            Some(SelectionKind::Model)
        );
        assert_eq!(join.reference(&name("s_other")), None);
    }

    #[test]
    fn two_joins_of_one_input_agree() {
        assert_eq!(join(), join());
        assert_eq!(
            join().admissibility(&name("s_writer"), &core()),
            join().admissibility(&name("s_writer"), &core())
        );
    }
}
