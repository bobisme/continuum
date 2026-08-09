//! The ten-stage pipeline's vocabulary and its **auditable intermediates** (RFC 0028,
//! "Compiler pipeline").
//!
//! # Scope: bn-21vno — the compiler's stage group 1 (stages 1–2)
//!
//! > Ten stages. […] Each stage MUST produce an auditable intermediate, and the
//! > intermediates MUST be retrievable for the parity audit's context-slice comparison
//! > (RFC 0030, compared artifact 5).
//! >
//! > — RFC 0028, "Compiler pipeline"
//!
//! Two MUSTs, and they are different obligations. *Produce* is per stage: a stage that
//! narrows or grows the working set without recording what it left cannot be audited at all.
//! *Retrievable* is about the record as a whole: the audit compares a pack's slice against a
//! clean recomputation's, and a comparison needs to be able to ask "what did stage 3 have?"
//! and get an answer. [`StageIntermediate`] is the first; [`StageTrail`] is the second.
//!
//! This bone owns the spine and fills the first three of its entries (the redaction pre-pass,
//! stage 1, stage 2). Stage groups 2, 3 and 4 append to it rather than inventing their own
//! record, which is why [`Stage`] is declared with all ten members from the start — the
//! precedent [`crate::selection::SelectionKind`] set in this crate for the same reason.
//!
//! # Why the pre-pass is a phase and not a stage
//!
//! > **Redaction runs before stage 1.** Plan §18.4 says context compilation enforces field
//! > policy "before slicing"; this RFC makes it exact — policy is applied before *root
//! > selection*, because a redacted root is not a root and slicing from one leaks the shape
//! > of what it dropped.
//! >
//! > — RFC 0028, "Compiler pipeline" (correction 12)
//!
//! The pre-pass is normatively ordered against stage 1 but is not one of the ten. Numbering
//! it 1 would renumber the RFC's own table; leaving it out of the record would make the one
//! ordering constraint the RFC states about it unauditable. [`Phase`] holds both: ordinal 0
//! for the pre-pass, 1–10 for the stages, and [`StageTrail::record`] enforces that a trail's
//! phases strictly increase — so an intermediate claiming redaction ran *after* root
//! selection cannot be recorded, and correction 12 is checked rather than remembered.
//!
//! # What an intermediate is, exactly
//!
//! The working set the phase left behind, in canonical order, plus the phase that produced
//! it. Nothing else — in particular no timestamp and no duration, because INV-005 forbids
//! ambient time and an intermediate that changed between two runs of one compile would make
//! the parity audit's comparison meaningless. [`StageIntermediate::digest`] is the content
//! identity of the canonical rendering, so two builds of one pipeline produce equal digests
//! and the audit can compare trails without comparing every set.
//!
//! # Clause → test
//!
//! | Clause | Source | Test |
//! |---|---|---|
//! | ten stages, root selection is stage 1 | RFC 0028 correction 13 | `the_stages_are_the_rfcs_ten_with_root_selection_first` |
//! | redaction precedes root selection | RFC 0028 correction 12 | `redaction_cannot_be_recorded_after_root_selection` |
//! | every stage produces an intermediate | RFC 0028, "Compiler pipeline" | `a_trail_reports_the_phases_that_recorded` |
//! | intermediates are retrievable per phase | RFC 0030, compared artifact 5 | `an_intermediate_is_retrievable_by_phase` |
//! | a phase records once | this module's discipline | `a_phase_cannot_record_twice` |
//! | a working set is inside the universe | this module's discipline | `an_intermediate_outside_the_universe_is_refused` |
//! | the digest is a function of content alone (INV-005) | ADR-0013, INV-005 | `two_builds_of_one_trail_agree` |

use core::fmt;
use std::collections::{BTreeMap, BTreeSet};

use continuum_intent::canonical_json::Json;
use continuum_value::identity::ContentHasher;
use continuum_value::value::Name;

/// One of RFC 0028's ten compiler stages.
///
/// Declared in the RFC's own table order, which is also the ordinal: root selection is
/// stage 1 (correction 13 — "Ten stages, and root selection is one of them"; plan §6.3
/// leaves it implicit and lists the remaining nine).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Stage {
    /// 1 — root selection. Licenses "nothing on its own; a bad root is a wrong pack, not an
    /// unsound one".
    RootSelection,
    /// 2 — backward causal slicing. Licenses `CausallyClosed`, and the precondition of
    /// `ReplayPreserving`.
    CausalSlicing,
    /// 3 — property-automaton relevance filtering. Licenses `PropertyPreserving`, with
    /// stage 2.
    PropertyRelevance,
    /// 4 — static/dynamic dependence join. Licenses the admissibility of `source` and
    /// `model` items.
    DependenceJoin,
    /// 5 — proof-dependency slicing. Licenses `ProofRelevant`.
    ProofSlicing,
    /// 6 — observer projection. Scoping under INV-013; "never a guarantee by itself".
    ObserverProjection,
    /// 7 — abstraction/refinement correspondence mapping.
    CorrespondenceMapping,
    /// 8 — minimal unsatisfied core / correction-set analysis. Licenses the minimality
    /// classes, each with its own transcript.
    MinimalCore,
    /// 9 — heuristic ranking of optional context. Licenses `HeuristicRelevant` only, and is
    /// the lane's kill surface: RFC 0028 requires that a deployment be able to run with this
    /// stage disabled.
    HeuristicRanking,
    /// 10 — budget packing. Licenses nothing: "packing MUST NOT create or destroy a
    /// guarantee silently". [`crate::budget::BudgetPacker`] is this stage in substance.
    BudgetPacking,
}

impl Stage {
    /// All ten stages, in pipeline order.
    pub const ALL: [Self; 10] = [
        Self::RootSelection,
        Self::CausalSlicing,
        Self::PropertyRelevance,
        Self::DependenceJoin,
        Self::ProofSlicing,
        Self::ObserverProjection,
        Self::CorrespondenceMapping,
        Self::MinimalCore,
        Self::HeuristicRanking,
        Self::BudgetPacking,
    ];

    /// The RFC's stage number, 1 through 10.
    #[must_use]
    pub const fn number(self) -> u8 {
        match self {
            Self::RootSelection => 1,
            Self::CausalSlicing => 2,
            Self::PropertyRelevance => 3,
            Self::DependenceJoin => 4,
            Self::ProofSlicing => 5,
            Self::ObserverProjection => 6,
            Self::CorrespondenceMapping => 7,
            Self::MinimalCore => 8,
            Self::HeuristicRanking => 9,
            Self::BudgetPacking => 10,
        }
    }

    /// A stable token for the intermediate's rendering.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::RootSelection => "root-selection",
            Self::CausalSlicing => "backward-causal-slicing",
            Self::PropertyRelevance => "property-automaton-relevance",
            Self::DependenceJoin => "dependence-join",
            Self::ProofSlicing => "proof-dependency-slicing",
            Self::ObserverProjection => "observer-projection",
            Self::CorrespondenceMapping => "correspondence-mapping",
            Self::MinimalCore => "minimal-unsatisfied-core",
            Self::HeuristicRanking => "heuristic-ranking",
            Self::BudgetPacking => "budget-packing",
        }
    }
}

impl fmt::Display for Stage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A point in the pipeline that produces an auditable intermediate: the pre-pass, or one of
/// the ten stages.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Phase {
    /// The redaction pre-pass, ordinal 0 — normatively *before* stage 1 (RFC 0028
    /// correction 12).
    Redaction,
    /// One of the ten numbered stages.
    Stage(Stage),
}

impl Phase {
    /// The pre-pass at 0, the stages at 1 through 10.
    #[must_use]
    pub const fn ordinal(self) -> u8 {
        match self {
            Self::Redaction => 0,
            Self::Stage(stage) => stage.number(),
        }
    }

    /// A stable token for the intermediate's rendering.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Redaction => "redaction-policy",
            Self::Stage(stage) => stage.as_str(),
        }
    }
}

impl fmt::Display for Phase {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// What one phase left behind: the working set after it ran, in canonical order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StageIntermediate {
    phase: Phase,
    working_set: BTreeSet<Name>,
    note: Option<String>,
}

impl StageIntermediate {
    /// Record a phase's working set.
    #[must_use]
    pub fn of(phase: Phase, working_set: impl IntoIterator<Item = Name>) -> Self {
        Self {
            phase,
            working_set: working_set.into_iter().collect(),
            note: None,
        }
    }

    /// Record a phase's working set with a typed token explaining the phase's own
    /// disposition — a closed vocabulary member's `as_str`, never caller-supplied prose
    /// (INV-016).
    #[must_use]
    pub fn noted(
        phase: Phase,
        working_set: impl IntoIterator<Item = Name>,
        note: &'static str,
    ) -> Self {
        Self {
            phase,
            working_set: working_set.into_iter().collect(),
            note: Some(note.to_owned()),
        }
    }

    /// Which phase produced this intermediate.
    #[must_use]
    pub const fn phase(&self) -> Phase {
        self.phase
    }

    /// The working set the phase left, in canonical order.
    #[must_use]
    pub const fn working_set(&self) -> &BTreeSet<Name> {
        &self.working_set
    }

    /// How many items the phase left.
    #[must_use]
    pub fn len(&self) -> usize {
        self.working_set.len()
    }

    /// Whether the phase left nothing.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.working_set.is_empty()
    }

    /// The typed note, where the phase recorded one.
    #[must_use]
    pub fn note(&self) -> Option<&str> {
        self.note.as_deref()
    }

    /// The auditable rendering: `{phase, ordinal, items, note?}` in canonical JSON.
    #[must_use]
    pub fn to_json(&self) -> Json {
        let mut fields: BTreeMap<String, Json> = BTreeMap::new();
        fields.insert(
            "items".to_owned(),
            Json::Array(
                self.working_set
                    .iter()
                    .map(|id| Json::String(id.as_str().to_owned()))
                    .collect(),
            ),
        );
        if let Some(note) = &self.note {
            fields.insert("note".to_owned(), Json::String(note.clone()));
        }
        fields.insert(
            "ordinal".to_owned(),
            Json::Integer(i64::from(self.phase.ordinal())),
        );
        fields.insert(
            "phase".to_owned(),
            Json::String(self.phase.as_str().to_owned()),
        );
        Json::Object(fields)
    }

    /// The content identity of this intermediate (ADR-0013), as a digest token.
    #[must_use]
    pub fn digest<H: ContentHasher>(&self) -> String {
        H::hash(&self.to_json().to_canonical_bytes()).to_token()
    }
}

/// The pipeline's record: one intermediate per phase that ran, in phase order.
///
/// The trail is built over the **candidate universe** the pre-pass left, and every recorded
/// working set must be a subset of it. That is the one structural invariant that holds
/// across all ten stages: stage 2 *grows* the set the roots started, stages 3–8 narrow it,
/// and none of them may conjure an item the compile never held.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StageTrail {
    universe: BTreeSet<Name>,
    entries: BTreeMap<u8, StageIntermediate>,
}

impl StageTrail {
    /// Begin a trail over a candidate universe.
    #[must_use]
    pub fn over(universe: impl IntoIterator<Item = Name>) -> Self {
        Self {
            universe: universe.into_iter().collect(),
            entries: BTreeMap::new(),
        }
    }

    /// Record one phase's intermediate.
    ///
    /// # Errors
    ///
    /// [`TrailError::PhaseRepeated`] when a phase records twice — an audit that found two
    /// answers for "what did stage 3 have?" would have no answer at all;
    /// [`TrailError::OutOfOrder`] when a phase records after a later one, which is how
    /// correction 12's "redaction runs before stage 1" is enforced rather than assumed; and
    /// [`TrailError::OutsideUniverse`] when a working set names something the compile never
    /// held as a candidate.
    pub fn record(&mut self, intermediate: StageIntermediate) -> Result<(), TrailError> {
        let ordinal = intermediate.phase().ordinal();
        if self.entries.contains_key(&ordinal) {
            return Err(TrailError::PhaseRepeated {
                phase: intermediate.phase(),
            });
        }
        if let Some((last, _)) = self.entries.iter().next_back()
            && *last > ordinal
        {
            return Err(TrailError::OutOfOrder {
                phase: intermediate.phase(),
                after: self.entries[last].phase(),
            });
        }
        if let Some(stray) = intermediate
            .working_set()
            .difference(&self.universe)
            .next()
            .cloned()
        {
            return Err(TrailError::OutsideUniverse {
                phase: intermediate.phase(),
                id: stray,
            });
        }
        self.entries.insert(ordinal, intermediate);
        Ok(())
    }

    /// The candidate universe this trail is stated over.
    #[must_use]
    pub const fn universe(&self) -> &BTreeSet<Name> {
        &self.universe
    }

    /// The phases that recorded, in pipeline order.
    #[must_use]
    pub fn phases(&self) -> Vec<Phase> {
        self.entries
            .values()
            .map(StageIntermediate::phase)
            .collect()
    }

    /// One phase's intermediate — the retrieval RFC 0030's compared artifact 5 needs.
    #[must_use]
    pub fn intermediate(&self, phase: Phase) -> Option<&StageIntermediate> {
        self.entries
            .get(&phase.ordinal())
            .filter(|entry| entry.phase() == phase)
    }

    /// Every intermediate, in pipeline order.
    pub fn intermediates(&self) -> impl Iterator<Item = &StageIntermediate> {
        self.entries.values()
    }

    /// The working set the last recorded phase left, or the universe when nothing ran.
    #[must_use]
    pub fn current(&self) -> BTreeSet<Name> {
        self.entries.values().next_back().map_or_else(
            || self.universe.clone(),
            |entry| entry.working_set().clone(),
        )
    }

    /// The auditable rendering of the whole trail.
    #[must_use]
    pub fn to_json(&self) -> Json {
        let mut fields: BTreeMap<String, Json> = BTreeMap::new();
        fields.insert(
            "intermediates".to_owned(),
            Json::Array(
                self.entries
                    .values()
                    .map(StageIntermediate::to_json)
                    .collect(),
            ),
        );
        fields.insert(
            "universe".to_owned(),
            Json::Array(
                self.universe
                    .iter()
                    .map(|id| Json::String(id.as_str().to_owned()))
                    .collect(),
            ),
        );
        Json::Object(fields)
    }

    /// The content identity of the whole trail (ADR-0013), as a digest token.
    #[must_use]
    pub fn digest<H: ContentHasher>(&self) -> String {
        H::hash(&self.to_json().to_canonical_bytes()).to_token()
    }
}

/// A way an intermediate cannot be recorded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TrailError {
    /// A phase recorded twice.
    PhaseRepeated {
        /// The phase already recorded.
        phase: Phase,
    },
    /// A phase recorded after a later one.
    OutOfOrder {
        /// The phase being recorded.
        phase: Phase,
        /// The later phase already in the trail.
        after: Phase,
    },
    /// A working set names something outside the candidate universe.
    OutsideUniverse {
        /// The phase whose set is wrong.
        phase: Phase,
        /// The first stray identity, in canonical order.
        id: Name,
    },
}

impl fmt::Display for TrailError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PhaseRepeated { phase } => {
                write!(f, "`{phase}` has already recorded an intermediate")
            }
            Self::OutOfOrder { phase, after } => write!(
                f,
                "`{phase}` cannot record after `{after}`; the pipeline's phase order is \
                 normative (RFC 0028, \"Compiler pipeline\")"
            ),
            Self::OutsideUniverse { phase, id } => write!(
                f,
                "`{phase}` left `{id}`, which is not a candidate of this compile"
            ),
        }
    }
}

impl core::error::Error for TrailError {}

#[cfg(test)]
mod tests {
    use super::*;
    use continuum_value::identity::Blake3Hasher;

    fn name(text: &str) -> Name {
        Name::new(text).expect("well formed")
    }

    fn universe() -> Vec<Name> {
        ["e_1", "e_2", "e_3", "s_1"].into_iter().map(name).collect()
    }

    #[test]
    fn the_stages_are_the_rfcs_ten_with_root_selection_first() {
        let numbers: Vec<u8> = Stage::ALL.into_iter().map(Stage::number).collect();
        assert_eq!(numbers, (1..=10).collect::<Vec<u8>>());
        assert_eq!(Stage::RootSelection.number(), 1);
        assert_eq!(Stage::BudgetPacking.number(), 10);
        // Every token is distinct, so the rendering identifies the stage.
        let tokens: BTreeSet<&str> = Stage::ALL.into_iter().map(Stage::as_str).collect();
        assert_eq!(tokens.len(), 10);
    }

    #[test]
    fn the_pre_pass_sorts_before_every_stage() {
        assert_eq!(Phase::Redaction.ordinal(), 0);
        for stage in Stage::ALL {
            assert!(Phase::Redaction < Phase::Stage(stage));
            assert!(Phase::Redaction.ordinal() < Phase::Stage(stage).ordinal());
        }
    }

    #[test]
    fn redaction_cannot_be_recorded_after_root_selection() {
        let mut trail = StageTrail::over(universe());
        trail
            .record(StageIntermediate::of(
                Phase::Stage(Stage::RootSelection),
                [name("e_3")],
            ))
            .expect("the first entry");
        assert_eq!(
            trail.record(StageIntermediate::of(Phase::Redaction, universe())),
            Err(TrailError::OutOfOrder {
                phase: Phase::Redaction,
                after: Phase::Stage(Stage::RootSelection),
            })
        );
    }

    #[test]
    fn a_later_stage_cannot_record_before_an_earlier_one() {
        let mut trail = StageTrail::over(universe());
        trail
            .record(StageIntermediate::of(
                Phase::Stage(Stage::MinimalCore),
                [name("e_1")],
            ))
            .expect("the first entry");
        assert!(matches!(
            trail.record(StageIntermediate::of(
                Phase::Stage(Stage::CausalSlicing),
                [name("e_1")],
            )),
            Err(TrailError::OutOfOrder { .. })
        ));
    }

    #[test]
    fn a_phase_cannot_record_twice() {
        let mut trail = StageTrail::over(universe());
        trail
            .record(StageIntermediate::of(
                Phase::Stage(Stage::CausalSlicing),
                [name("e_1")],
            ))
            .expect("the first entry");
        assert_eq!(
            trail.record(StageIntermediate::of(
                Phase::Stage(Stage::CausalSlicing),
                [name("e_2")],
            )),
            Err(TrailError::PhaseRepeated {
                phase: Phase::Stage(Stage::CausalSlicing),
            })
        );
    }

    #[test]
    fn an_intermediate_outside_the_universe_is_refused() {
        let mut trail = StageTrail::over(universe());
        assert_eq!(
            trail.record(StageIntermediate::of(
                Phase::Stage(Stage::RootSelection),
                [name("e_1"), name("z_9")],
            )),
            Err(TrailError::OutsideUniverse {
                phase: Phase::Stage(Stage::RootSelection),
                id: name("z_9"),
            })
        );
    }

    #[test]
    fn a_trail_reports_the_phases_that_recorded() {
        let mut trail = StageTrail::over(universe());
        assert!(trail.phases().is_empty());
        assert_eq!(trail.current(), universe().into_iter().collect());

        trail
            .record(StageIntermediate::of(Phase::Redaction, universe()))
            .expect("the pre-pass");
        trail
            .record(StageIntermediate::of(
                Phase::Stage(Stage::RootSelection),
                [name("e_3")],
            ))
            .expect("stage 1");
        trail
            .record(StageIntermediate::of(
                Phase::Stage(Stage::CausalSlicing),
                [name("e_1"), name("e_2"), name("e_3")],
            ))
            .expect("stage 2");

        assert_eq!(
            trail.phases(),
            [
                Phase::Redaction,
                Phase::Stage(Stage::RootSelection),
                Phase::Stage(Stage::CausalSlicing),
            ]
        );
        // Stage 2 grows what stage 1 selected: the trail's invariant is the universe, not
        // monotone narrowing.
        assert_eq!(trail.current().len(), 3);
    }

    #[test]
    fn an_intermediate_is_retrievable_by_phase() {
        let mut trail = StageTrail::over(universe());
        trail
            .record(StageIntermediate::noted(
                Phase::Redaction,
                universe(),
                "no-policy",
            ))
            .expect("the pre-pass");
        trail
            .record(StageIntermediate::of(
                Phase::Stage(Stage::RootSelection),
                [name("e_3")],
            ))
            .expect("stage 1");

        let recorded = trail
            .intermediate(Phase::Stage(Stage::RootSelection))
            .expect("stage 1 recorded");
        assert_eq!(recorded.working_set(), &[name("e_3")].into_iter().collect());
        assert_eq!(recorded.note(), None);
        assert_eq!(
            trail
                .intermediate(Phase::Redaction)
                .expect("the pre-pass recorded")
                .note(),
            Some("no-policy")
        );
        // A phase that did not run has no intermediate, which is a different answer from an
        // empty one.
        assert!(
            trail
                .intermediate(Phase::Stage(Stage::ProofSlicing))
                .is_none()
        );
    }

    #[test]
    fn an_empty_working_set_is_recordable_and_is_not_absence() {
        let mut trail = StageTrail::over(universe());
        trail
            .record(StageIntermediate::of(
                Phase::Stage(Stage::CausalSlicing),
                [],
            ))
            .expect("a stage may leave nothing");
        let recorded = trail
            .intermediate(Phase::Stage(Stage::CausalSlicing))
            .expect("recorded");
        assert!(recorded.is_empty());
        assert_eq!(recorded.len(), 0);
    }

    #[test]
    fn the_rendering_carries_the_phase_its_ordinal_and_its_items() {
        let intermediate = StageIntermediate::of(
            Phase::Stage(Stage::CausalSlicing),
            [name("e_2"), name("e_1")],
        );
        assert_eq!(
            intermediate.to_json().to_canonical_bytes(),
            br#"{"items":["e_1","e_2"],"ordinal":2,"phase":"backward-causal-slicing"}"#
        );
    }

    #[test]
    fn two_builds_of_one_trail_agree() {
        let build = || {
            let mut trail = StageTrail::over(universe());
            trail
                .record(StageIntermediate::of(Phase::Redaction, universe()))
                .expect("the pre-pass");
            trail
                .record(StageIntermediate::of(
                    Phase::Stage(Stage::RootSelection),
                    [name("e_3")],
                ))
                .expect("stage 1");
            trail
        };
        assert_eq!(build(), build());
        assert_eq!(
            build().digest::<Blake3Hasher>(),
            build().digest::<Blake3Hasher>()
        );
        // And a different trail is a different digest, so the identity is not vacuous.
        let mut other = build();
        other
            .record(StageIntermediate::of(
                Phase::Stage(Stage::CausalSlicing),
                [name("e_1"), name("e_3")],
            ))
            .expect("stage 2");
        assert_ne!(
            build().digest::<Blake3Hasher>(),
            other.digest::<Blake3Hasher>()
        );
    }

    #[test]
    fn insertion_order_does_not_move_an_intermediates_digest() {
        let one = StageIntermediate::of(
            Phase::Stage(Stage::CausalSlicing),
            [name("e_1"), name("e_2")],
        );
        let other = StageIntermediate::of(
            Phase::Stage(Stage::CausalSlicing),
            [name("e_2"), name("e_1")],
        );
        assert_eq!(one, other);
        assert_eq!(one.digest::<Blake3Hasher>(), other.digest::<Blake3Hasher>());
    }
}
