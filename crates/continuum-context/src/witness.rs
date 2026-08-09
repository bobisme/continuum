//! The **independent transcript check** that licenses the minimality classes (RFC 0028,
//! "Validation"; rule C3).
//!
//! # Scope: bn-3ub6i — the compiler's stage group 4 (stage 8)
//!
//! > - **Single-item removal** on `OneMinimal`; the corresponding transcripts on the other
//! >   minimality classes; preregistered study evidence on `ExplanationMinimal`.
//! > […]
//! > - **Guarantee-violation mutants** are rejected: […] **a minimality class with no transcript**.
//! >
//! > — RFC 0028, "Validation"
//!
//! # The independence argument
//!
//! The same three separations [`crate::monitor`] and [`crate::scope`] state, at this stage:
//!
//! 1. **It is a different computation, not the same one run twice.**
//!    [`crate::minimality::MinimalitySearch`] *searches*: it walks a removal family and records
//!    what it found. [`MinimalityAudit::verdict`] **never searches**. It is handed a transcript and
//!    asks three questions the search does not ask — did this transcript examine *exactly* the
//!    removals the class requires; is every recorded outcome the one the premise actually produces;
//!    and is the core it is about the set the pack publishes. A search bug shows up here as a
//!    named coverage gap or a named forgery, not as a shared assumption.
//! 2. **It reads only what a checker may read.** [`MinimalityAudit::verdict`] takes a
//!    [`crate::causal::CausalOrder`], the selection about to be published, and a transcript. It is
//!    not handed the search, the trail, the accounting, or stage 8's decision, and every transcript
//!    it can be asked about — including ones the search cannot produce — gets an answer decided
//!    from its own premise. That is what makes
//!    `the_audit_rejects_transcripts_the_search_cannot_produce` falsifiable rather than a
//!    restatement.
//! 3. **It is a different module, and this one names nothing of the search's.** `witness.rs`
//!    imports [`crate::minimality::MinimalityClass`] and the transcript vocabulary — the shared
//!    *premise*, exactly as `monitor.rs` imports `PropertyAutomaton` — and imports neither
//!    `MinimalitySearch` nor [`crate::compile`]. It computes the automaton run over a reduced set
//!    itself rather than calling the search's helper, which is why that duplication is deliberate.
//!    `the_witness_module_names_nothing_of_the_searchs` reads this file's own bytes and pins it.
//!
//! And, as there, what the separation does **not** buy: the audit and the search are given the same
//! *property*. [`crate::compile::CausalCompile::with_minimality`] takes the question and the audit
//! as separate arguments so a caller may audit against a stricter automaton, or a different
//! dimension attribution, than the search was given; where they disagree the audit wins and the
//! class is refused. And C6 is unchanged: the artifact records no checker identity, so "a
//! promotion-relevant consumer MUST re-run the checker rather than read the field".
//!
//! # What the audit checks, and why in this order
//!
//! 1. The transcript's core is empty → [`MinimalityRefusal::EmptyCore`]. A minimality claim about
//!    nothing is vacuous.
//! 2. The core is not the set the pack publishes → [`TranscriptDefect::CoreMismatch`]. A transcript
//!    about a different set is evidence about a different pack, however honest it is.
//! 3. The core is not downward closed → [`TranscriptDefect::CoreNotCausallyClosed`]. This is the
//!    finite shadow of RFC 0028's **Lean obligation 2**, "minimality soundness — a 1-minimal slice
//!    under the removal check is still a slice, that is, removal-minimization preserves downward
//!    closure". It is checked before anything about units because a minimality claim about a set
//!    that is not a slice is not an instance of that theorem at all — and because such a core would
//!    also break the `CausallyClosed` stage 2 licensed.
//! 4. The core does not witness the failure → [`MinimalityRefusal::NoWitness`]. The anti-vacuity
//!    boundary: with no witness to destroy every removal "destroys" one trivially, and a verdict of
//!    verified there would license a claim no check established.
//!    ([`crate::monitor::Inapplicable::NoObservedEvent`] is the same refusal in its own place.)
//! 5. A bound is declared on a class that takes none → [`TranscriptDefect::BoundNotApplicable`].
//! 6. The class's removal family is not computable from the premise → the refusal that says why,
//!    including [`MinimalityRefusal::UnboundedSearch`] for a `CardinalityMinimal` transcript with
//!    no declared bound. **This is where "an unbounded search cannot claim it" is enforced by the
//!    checker rather than by the producer**, so a transcript that arrived from anywhere is held to
//!    it.
//! 7. A required removal is missing → [`TranscriptDefect::UnitNotExamined`]; an examined removal is
//!    not required → [`TranscriptDefect::UnitNotRequired`]. Together these are RFC 0028's named
//!    mutant "a minimality class with no transcript" and its neighbours: a class whose transcript
//!    covers *part* of its family has established the class over part of it, which is not the
//!    class.
//! 8. A recorded outcome is not the one the premise produces → [`TranscriptDefect::ForgedOutcome`],
//!    **in both directions**. A transcript that under-claims is as wrong about the run as one that
//!    over-claims, and only a checker that re-derives can tell either from the truth.
//! 9. A removal the premise confirms the witness *survives* → [`TranscriptVerdict::Refuted`]. This
//!    is RFC 0028's own failure mode for `OneMinimal` — "a survivable removal refutes it" — and it
//!    is a different fact from a defective transcript: the evidence is sound and the claim is
//!    false. Reported after the forgery pass so an honest refutation is never used to mask a lie
//!    elsewhere in the same transcript.
//! 10. Otherwise → [`TranscriptVerdict::Verified`], the only arm that licenses anything.
//!
//! # One transcript licenses one class
//!
//! [`TranscriptVerdict::Verified`] carries the class it verified, and that class is the transcript's
//! own ([`crate::minimality::MinimalityTranscript::class`]). There is no path by which checking one
//! class produces a verdict about another, which is rule C3 made structural rather than reviewed:
//! "a consumer MUST NOT infer one class from another on the wire", and neither may the compiler
//! that writes the wire.
//!
//! # Clause → test
//!
//! | Clause | Source | Test |
//! |---|---|---|
//! | the check is independent of the search | RFC 0028, "Validation" | `the_audit_rejects_transcripts_the_search_cannot_produce` |
//! | a class with no transcript is rejected | RFC 0028, "Validation" | `a_transcript_missing_a_required_removal_is_defective` |
//! | a forged outcome is caught in both directions | RFC 0028, "Validation" | `a_forged_outcome_is_named_in_both_directions` |
//! | a survivable removal refutes the class | RFC 0028, "Guarantee classes" | `a_survivable_removal_refutes_the_class` |
//! | removal-minimization preserves downward closure | RFC 0028, Lean obligation 2 | `a_core_that_is_not_a_slice_is_defective` |
//! | an unbounded search cannot claim cardinality | RFC 0028, "Guarantee classes" | `an_unbounded_cardinality_transcript_is_refused` |
//! | one transcript licenses one class | RFC 0028 C3 | `a_verified_transcript_names_its_own_class_and_no_other` |
//! | the honest transcript passes | anti-vacuity | `the_searchs_own_transcript_verifies` |

use core::fmt;
use std::collections::BTreeSet;

use continuum_value::value::Name;

use crate::causal::{CausalOrder, ClosureViolation};
use crate::minimality::{
    DimensionSet, MinimalityClass, MinimalityRefusal, MinimalityTranscript, WitnessOutcome,
};
use crate::property::{AutomatonVerdict, PropertyAutomaton};

/// The checker that licenses the minimality classes.
///
/// Built over an automaton and a dimension attribution the caller supplies *separately* from the
/// ones the search was given — see the module documentation's "The independence argument".
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MinimalityAudit {
    automaton: PropertyAutomaton,
    dimensions: DimensionSet,
}

impl MinimalityAudit {
    /// Audit against `automaton`, with no dimension attribution declared.
    ///
    /// The three dimension classes are then refused with
    /// [`MinimalityRefusal::DimensionNotDeclared`], which is the honest answer: an audit that
    /// borrowed the question's attribution would be checking a reduction against the very
    /// declaration that produced it.
    #[must_use]
    pub fn over(automaton: PropertyAutomaton) -> Self {
        Self {
            automaton,
            dimensions: DimensionSet::empty(),
        }
    }

    /// Audit against `automaton` and this attribution of candidates to dimension elements.
    #[must_use]
    pub const fn of(automaton: PropertyAutomaton, dimensions: DimensionSet) -> Self {
        Self {
            automaton,
            dimensions,
        }
    }

    /// The property this audit decides the witness against.
    #[must_use]
    pub const fn automaton(&self) -> &PropertyAutomaton {
        &self.automaton
    }

    /// The dimension attribution this audit checks the dimension classes against.
    #[must_use]
    pub const fn dimensions(&self) -> &DimensionSet {
        &self.dimensions
    }

    /// Decide whether `transcript` establishes its class about `selection`.
    ///
    /// The check order is the module documentation's, and each step is stated there with the reason
    /// it comes where it does.
    #[must_use]
    pub fn verdict(
        &self,
        order: &CausalOrder,
        selection: &BTreeSet<Name>,
        transcript: &MinimalityTranscript,
    ) -> TranscriptVerdict {
        let class = transcript.class();
        let core = transcript.core();
        // Computed once here rather than per removal: a property of the causal order, not of any
        // one reduction, and every run below sees the sequence it would have seen recomputed.
        let topological = order.topological_order();

        if core.is_empty() {
            return TranscriptVerdict::Refused(MinimalityRefusal::EmptyCore);
        }
        if let Some(id) = core.symmetric_difference(selection).next().cloned() {
            return TranscriptVerdict::Defective(TranscriptDefect::CoreMismatch { id });
        }
        if let Some(violation) = order.closure_violation(core) {
            return TranscriptVerdict::Defective(TranscriptDefect::CoreNotCausallyClosed(
                violation,
            ));
        }
        if self.witness(&topological, core) != AutomatonVerdict::Violated {
            return TranscriptVerdict::Refused(MinimalityRefusal::NoWitness);
        }
        if transcript.bound().is_some() && !class.takes_bound() {
            return TranscriptVerdict::Defective(TranscriptDefect::BoundNotApplicable { class });
        }

        let required = match class.removal_family(order, core, &self.dimensions, transcript.bound())
        {
            Ok(family) => family,
            Err(refusal) => return TranscriptVerdict::Refused(refusal),
        };
        let examined = transcript.examined();
        if let Some(removed) = required.difference(&examined).next().cloned() {
            return TranscriptVerdict::Defective(TranscriptDefect::UnitNotExamined { removed });
        }
        if let Some(removed) = examined.difference(&required).next().cloned() {
            return TranscriptVerdict::Defective(TranscriptDefect::UnitNotRequired { removed });
        }

        // Every recorded outcome is re-derived before any of them is believed. Both passes walk
        // `required`, which is a `BTreeSet` in `Name`'s canonical order, so the first defect and
        // the first counterexample are the same on every platform (INV-005).
        for removed in &required {
            let recorded = transcript
                .outcome(removed)
                .expect("coverage is exact by the checks above");
            let derived = self.derive(&topological, core, removed);
            if recorded != derived {
                return TranscriptVerdict::Defective(TranscriptDefect::ForgedOutcome {
                    removed: removed.clone(),
                    recorded,
                    derived,
                });
            }
        }
        for removed in &required {
            if self.derive(&topological, core, removed) == WitnessOutcome::Survived {
                return TranscriptVerdict::Refuted {
                    removed: removed.clone(),
                };
            }
        }

        TranscriptVerdict::Verified {
            class,
            examined: required.len(),
        }
    }

    /// What removing `removed` from `core` actually does to the witness, decided here.
    fn derive(
        &self,
        topological: &[Name],
        core: &BTreeSet<Name>,
        removed: &BTreeSet<Name>,
    ) -> WitnessOutcome {
        let remaining: BTreeSet<Name> = core.difference(removed).cloned().collect();
        WitnessOutcome::of(self.witness(topological, &remaining))
    }

    /// Run this audit's automaton over `set`, in the causal order's own topological order.
    ///
    /// Written here rather than reached for in [`crate::minimality`] on purpose: the shared thing
    /// is the *premise* (the automaton, the order), not an implementation of the run.
    fn witness(&self, topological: &[Name], set: &BTreeSet<Name>) -> AutomatonVerdict {
        let trace: Vec<Name> = topological
            .iter()
            .filter(|id| set.contains(*id))
            .cloned()
            .collect();
        self.automaton.run(&trace).verdict()
    }
}

/// What the audit concluded about one transcript.
///
/// Four arms, not a boolean. "Established", "the evidence is sound and the claim is false", "the
/// evidence does not check out", and "the class cannot be claimed in this shape" are four different
/// facts (INV-008's discipline, at a checker), and only the first licenses anything.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TranscriptVerdict {
    /// The transcript establishes its class over the published selection. **The only arm that
    /// licenses a guarantee**, and it licenses exactly [`TranscriptVerdict::Verified::class`].
    Verified {
        /// The class established — the transcript's own, never another (rule C3).
        class: MinimalityClass,
        /// How many removals the class required, all of them examined and all re-derived.
        examined: usize,
    },
    /// The transcript is sound and the class is false: this removal leaves the witness standing.
    Refuted {
        /// The removal the witness survives, first in canonical order.
        removed: BTreeSet<Name>,
    },
    /// The transcript does not check out, and the audit names how.
    Defective(TranscriptDefect),
    /// The class cannot be claimed in this shape at all, and the audit names why.
    Refused(MinimalityRefusal),
}

impl TranscriptVerdict {
    /// Whether this verdict licenses its class.
    #[must_use]
    pub const fn is_verified(&self) -> bool {
        matches!(self, Self::Verified { .. })
    }

    /// The class this verdict licenses, when it licenses one.
    #[must_use]
    pub const fn licensed(&self) -> Option<MinimalityClass> {
        match self {
            Self::Verified { class, .. } => Some(*class),
            _ => None,
        }
    }

    /// A stable token for an auditable intermediate's rendering — a closed vocabulary member's own
    /// spelling, never caller-supplied prose (INV-016).
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Verified { .. } => "minimality-verified",
            Self::Refuted { .. } => "minimality-refuted",
            Self::Defective(_) => "minimality-defective",
            Self::Refused(_) => "minimality-refused",
        }
    }
}

impl fmt::Display for TranscriptVerdict {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Verified { class, examined } => write!(
                f,
                "`{class}` is established: {examined} required removal(s) examined and re-derived"
            ),
            Self::Refuted { removed } => write!(
                f,
                "the witness survives the removal of {} selected item(s), which refutes the class \
                 (RFC 0028, \"Guarantee classes\")",
                removed.len()
            ),
            Self::Defective(defect) => write!(f, "{defect}"),
            Self::Refused(refusal) => write!(f, "{refusal}"),
        }
    }
}

/// How a transcript fails to establish the class it is evidence for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TranscriptDefect {
    /// The transcript is about a core that is not the selection the pack publishes.
    CoreMismatch {
        /// The first identity in the symmetric difference, in canonical order.
        id: Name,
    },
    /// The core is not downward closed under the causal order.
    ///
    /// > **Minimality soundness** — a 1-minimal slice under the removal check is still a slice,
    /// > that is, removal-minimization preserves downward closure.
    /// >
    /// > — RFC 0028, "Formal model", obligation 2
    CoreNotCausallyClosed(ClosureViolation),
    /// A search bound is declared on a class that declares none.
    BoundNotApplicable {
        /// The class that takes no bound.
        class: MinimalityClass,
    },
    /// The class requires this removal to be examined and the transcript does not examine it.
    UnitNotExamined {
        /// The first unexamined required removal, in canonical order.
        removed: BTreeSet<Name>,
    },
    /// The transcript examines a removal the class does not require, so it is not the class's
    /// family and the coverage claim is about something else.
    UnitNotRequired {
        /// The first surplus removal, in canonical order.
        removed: BTreeSet<Name>,
    },
    /// A recorded outcome is not the outcome the premise produces.
    ForgedOutcome {
        /// The removal whose outcome was misrecorded.
        removed: BTreeSet<Name>,
        /// What the transcript claims.
        recorded: WitnessOutcome,
        /// What the audit derives.
        derived: WitnessOutcome,
    },
}

impl TranscriptDefect {
    /// A stable token.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::CoreMismatch { .. } => "core-mismatch",
            Self::CoreNotCausallyClosed(_) => "core-not-causally-closed",
            Self::BoundNotApplicable { .. } => "bound-not-applicable",
            Self::UnitNotExamined { .. } => "unit-not-examined",
            Self::UnitNotRequired { .. } => "unit-not-required",
            Self::ForgedOutcome { .. } => "forged-outcome",
        }
    }
}

impl fmt::Display for TranscriptDefect {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CoreMismatch { id } => write!(
                f,
                "`{id}` is in the transcript's core or in the published selection but not in both; \
                 a minimality claim is about the set the pack publishes"
            ),
            Self::CoreNotCausallyClosed(violation) => write!(
                f,
                "{violation}; removal-minimization preserves downward closure, so a core that is \
                 not a slice establishes no minimality class (RFC 0028, \"Formal model\")"
            ),
            Self::BoundNotApplicable { class } => write!(
                f,
                "`{class}` declares no search bound, and this transcript carries one; the bound \
                 qualifies an exhaustive smaller-set search and nothing else"
            ),
            Self::UnitNotExamined { removed } => write!(
                f,
                "the class requires a removal of {} selected item(s) that this transcript never \
                 examined; a class with no transcript for part of its family is a class with no \
                 transcript (RFC 0028, \"Validation\")",
                removed.len()
            ),
            Self::UnitNotRequired { removed } => write!(
                f,
                "this transcript examines a removal of {} selected item(s) that the class does not \
                 require; its coverage is of some other family",
                removed.len()
            ),
            Self::ForgedOutcome {
                removed,
                recorded,
                derived,
            } => write!(
                f,
                "the transcript records `{recorded}` for a removal of {} selected item(s) and the \
                 premise produces `{derived}`; the transcript is not evidence of what it claims",
                removed.len()
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::minimality::{
        DimensionAttribution, MinimalitySearch, MinimizedDimension, SearchBound,
    };
    use crate::property::{AutomatonState, Coverage};
    use crate::selection::SelectionKind;
    use std::collections::BTreeMap;

    fn name(text: &str) -> Name {
        Name::new(text).expect("well formed")
    }

    fn ids(names: &[&str]) -> BTreeSet<Name> {
        names.iter().map(|id| name(id)).collect()
    }

    /// The chain `e_begin -> e_submit -> e_ack -> e_loss`, plus an unobserved isolated event.
    fn order() -> CausalOrder {
        CausalOrder::new(
            [
                (name("e_begin"), SelectionKind::Event),
                (name("e_submit"), SelectionKind::Event),
                (name("e_ack"), SelectionKind::Event),
                (name("e_loss"), SelectionKind::Event),
                (name("n_beat"), SelectionKind::Event),
            ],
            [
                (name("e_submit"), name("e_begin")),
                (name("e_ack"), name("e_submit")),
                (name("e_loss"), name("e_ack")),
            ],
        )
        .expect("a well-formed order")
    }

    fn automaton() -> PropertyAutomaton {
        let state = |text: &str| AutomatonState::new(name(text));
        PropertyAutomaton::new(
            state("q_0"),
            [
                (state("q_0"), name("e_begin"), state("q_1")),
                (state("q_1"), name("e_submit"), state("q_2")),
                (state("q_2"), name("e_ack"), state("q_3")),
                (state("q_3"), name("e_loss"), state("q_bad")),
            ],
            [state("q_bad")],
            [],
            Coverage::Total,
        )
        .expect("a well-formed automaton")
    }

    fn core() -> BTreeSet<Name> {
        ids(&["e_begin", "e_submit", "e_ack", "e_loss"])
    }

    fn audit() -> MinimalityAudit {
        MinimalityAudit::over(automaton())
    }

    fn transcribe(class: MinimalityClass, core: &BTreeSet<Name>) -> MinimalityTranscript {
        MinimalitySearch::transcribe(
            class,
            &order(),
            &automaton(),
            core,
            &DimensionSet::empty(),
            SearchBound::of(2),
        )
        .expect("the core witnesses the failure")
    }

    #[test]
    fn the_searchs_own_transcript_verifies() {
        let verdict = audit().verdict(
            &order(),
            &core(),
            &transcribe(MinimalityClass::OneMinimal, &core()),
        );
        assert_eq!(
            verdict,
            TranscriptVerdict::Verified {
                class: MinimalityClass::OneMinimal,
                examined: 4,
            }
        );
        assert!(verdict.is_verified());
        assert_eq!(verdict.licensed(), Some(MinimalityClass::OneMinimal));
        assert_eq!(verdict.as_str(), "minimality-verified");
    }

    #[test]
    fn a_verified_transcript_names_its_own_class_and_no_other() {
        // Rule C3, at the checker: each class is decided from its own transcript, and the verdict
        // names that class. The `CardinalityMinimal` family here is a superset of `OneMinimal`'s
        // and the two verdicts are still two.
        for class in [
            MinimalityClass::OneMinimal,
            MinimalityClass::CardinalityMinimal,
            MinimalityClass::CausallyMinimal,
        ] {
            let verdict = audit().verdict(&order(), &core(), &transcribe(class, &core()));
            assert_eq!(verdict.licensed(), Some(class), "{class}");
        }
    }

    /// The independence claim, made falsifiable — `monitor.rs`'s and `scope.rs`'s device at this
    /// stage.
    #[test]
    fn the_audit_rejects_transcripts_the_search_cannot_produce() {
        let order = order();
        // Claims a class over a core the pack does not publish.
        assert_eq!(
            audit().verdict(
                &order,
                &ids(&["e_begin", "e_submit", "e_ack"]),
                &transcribe(MinimalityClass::OneMinimal, &core())
            ),
            TranscriptVerdict::Defective(TranscriptDefect::CoreMismatch { id: name("e_loss") })
        );
        // Examines nothing at all: "a minimality class with no transcript".
        assert_eq!(
            audit().verdict(
                &order,
                &core(),
                &MinimalityTranscript::new(MinimalityClass::OneMinimal, core(), None, [])
            ),
            TranscriptVerdict::Defective(TranscriptDefect::UnitNotExamined {
                removed: ids(&["e_ack"])
            })
        );
        // And the honest one still passes, so the audit is not merely a rejector.
        assert!(
            audit()
                .verdict(
                    &order,
                    &core(),
                    &transcribe(MinimalityClass::OneMinimal, &core())
                )
                .is_verified()
        );
    }

    #[test]
    fn a_transcript_missing_a_required_removal_is_defective() {
        let honest = transcribe(MinimalityClass::OneMinimal, &core());
        let truncated: BTreeMap<BTreeSet<Name>, WitnessOutcome> = honest
            .steps()
            .filter(|(removed, _)| *removed != &ids(&["e_loss"]))
            .map(|(removed, outcome)| (removed.clone(), outcome))
            .collect();
        assert_eq!(
            audit().verdict(
                &order(),
                &core(),
                &MinimalityTranscript::new(MinimalityClass::OneMinimal, core(), None, truncated)
            ),
            TranscriptVerdict::Defective(TranscriptDefect::UnitNotExamined {
                removed: ids(&["e_loss"])
            })
        );
    }

    #[test]
    fn a_transcript_padded_with_a_removal_the_class_does_not_require_is_defective() {
        let honest = transcribe(MinimalityClass::OneMinimal, &core());
        let mut padded: BTreeMap<BTreeSet<Name>, WitnessOutcome> = honest
            .steps()
            .map(|(removed, outcome)| (removed.clone(), outcome))
            .collect();
        padded.insert(ids(&["e_ack", "e_loss"]), WitnessOutcome::Destroyed);
        assert_eq!(
            audit().verdict(
                &order(),
                &core(),
                &MinimalityTranscript::new(MinimalityClass::OneMinimal, core(), None, padded)
            ),
            TranscriptVerdict::Defective(TranscriptDefect::UnitNotRequired {
                removed: ids(&["e_ack", "e_loss"])
            })
        );
    }

    #[test]
    fn a_forged_outcome_is_named_in_both_directions() {
        // Over-claiming: `n_beat` survives its own removal and the transcript says it does not.
        let padded_core = ids(&["e_begin", "e_submit", "e_ack", "e_loss", "n_beat"]);
        let honest = transcribe(MinimalityClass::OneMinimal, &padded_core);
        assert_eq!(
            honest.outcome(&ids(&["n_beat"])),
            Some(WitnessOutcome::Survived)
        );
        let mut forged: BTreeMap<BTreeSet<Name>, WitnessOutcome> = honest
            .steps()
            .map(|(removed, outcome)| (removed.clone(), outcome))
            .collect();
        forged.insert(ids(&["n_beat"]), WitnessOutcome::Destroyed);
        assert_eq!(
            audit().verdict(
                &order(),
                &padded_core,
                &MinimalityTranscript::new(
                    MinimalityClass::OneMinimal,
                    padded_core.clone(),
                    None,
                    forged
                )
            ),
            TranscriptVerdict::Defective(TranscriptDefect::ForgedOutcome {
                removed: ids(&["n_beat"]),
                recorded: WitnessOutcome::Destroyed,
                derived: WitnessOutcome::Survived,
            })
        );
        // Under-claiming is a forgery too: the transcript is wrong about the run either way.
        let mut timid: BTreeMap<BTreeSet<Name>, WitnessOutcome> =
            transcribe(MinimalityClass::OneMinimal, &core())
                .steps()
                .map(|(removed, outcome)| (removed.clone(), outcome))
                .collect();
        timid.insert(ids(&["e_ack"]), WitnessOutcome::Survived);
        assert_eq!(
            audit().verdict(
                &order(),
                &core(),
                &MinimalityTranscript::new(MinimalityClass::OneMinimal, core(), None, timid)
            ),
            TranscriptVerdict::Defective(TranscriptDefect::ForgedOutcome {
                removed: ids(&["e_ack"]),
                recorded: WitnessOutcome::Survived,
                derived: WitnessOutcome::Destroyed,
            })
        );
    }

    #[test]
    fn a_survivable_removal_refutes_the_class() {
        // RFC 0028's own failure mode for `OneMinimal`, and the honest transcript that records it:
        // the evidence is sound, the class is false, and nothing is licensed.
        let padded_core = ids(&["e_begin", "e_submit", "e_ack", "e_loss", "n_beat"]);
        let verdict = audit().verdict(
            &order(),
            &padded_core,
            &transcribe(MinimalityClass::OneMinimal, &padded_core),
        );
        assert_eq!(
            verdict,
            TranscriptVerdict::Refuted {
                removed: ids(&["n_beat"])
            }
        );
        assert!(!verdict.is_verified());
        assert_eq!(verdict.licensed(), None);
        assert_eq!(verdict.as_str(), "minimality-refuted");
    }

    #[test]
    fn a_core_that_is_not_a_slice_is_defective() {
        // The finite shadow of Lean obligation 2: a removal-minimized core missing a causal
        // predecessor establishes no class, and would have broken stage 2's `CausallyClosed` too.
        let unclosed = ids(&["e_submit", "e_ack", "e_loss"]);
        let verdict = audit().verdict(
            &order(),
            &unclosed,
            &MinimalityTranscript::new(
                MinimalityClass::CausallyMinimal,
                unclosed.clone(),
                None,
                [(ids(&["e_loss"]), WitnessOutcome::Destroyed)],
            ),
        );
        assert_eq!(
            verdict,
            TranscriptVerdict::Defective(TranscriptDefect::CoreNotCausallyClosed(
                ClosureViolation::MissingPredecessor {
                    selected: name("e_submit"),
                    predecessor: name("e_begin"),
                }
            ))
        );
        assert_eq!(verdict.as_str(), "minimality-defective");
    }

    #[test]
    fn an_unbounded_cardinality_transcript_is_refused() {
        // The checker enforces "an unbounded search cannot claim it", so a transcript that arrived
        // from an external solver is held to it just as the in-crate search is.
        let unbounded = MinimalityTranscript::new(
            MinimalityClass::CardinalityMinimal,
            core(),
            None,
            core()
                .iter()
                .map(|id| (BTreeSet::from([id.clone()]), WitnessOutcome::Destroyed)),
        );
        assert_eq!(
            audit().verdict(&order(), &core(), &unbounded),
            TranscriptVerdict::Refused(MinimalityRefusal::UnboundedSearch)
        );
        // Declared, and the same transcript verifies — the bound is the whole difference.
        let bounded = MinimalityTranscript::new(
            MinimalityClass::CardinalityMinimal,
            core(),
            SearchBound::of(1),
            core()
                .iter()
                .map(|id| (BTreeSet::from([id.clone()]), WitnessOutcome::Destroyed)),
        );
        assert_eq!(
            audit().verdict(&order(), &core(), &bounded),
            TranscriptVerdict::Verified {
                class: MinimalityClass::CardinalityMinimal,
                examined: 4,
            }
        );
    }

    #[test]
    fn a_bound_on_a_class_that_takes_none_is_defective() {
        let bounded = MinimalityTranscript::new(
            MinimalityClass::OneMinimal,
            core(),
            SearchBound::of(1),
            core()
                .iter()
                .map(|id| (BTreeSet::from([id.clone()]), WitnessOutcome::Destroyed)),
        );
        assert_eq!(
            audit().verdict(&order(), &core(), &bounded),
            TranscriptVerdict::Defective(TranscriptDefect::BoundNotApplicable {
                class: MinimalityClass::OneMinimal
            })
        );
    }

    #[test]
    fn an_empty_core_and_a_witnessless_core_are_refused() {
        assert_eq!(
            audit().verdict(
                &order(),
                &BTreeSet::new(),
                &MinimalityTranscript::new(MinimalityClass::OneMinimal, [], None, [])
            ),
            TranscriptVerdict::Refused(MinimalityRefusal::EmptyCore)
        );
        // The chain's prefix satisfies the property, so there is no witness a removal could
        // destroy — and "every removal destroyed it" would be vacuously true.
        let prefix = ids(&["e_begin", "e_submit", "e_ack"]);
        let verdict = audit().verdict(
            &order(),
            &prefix,
            &MinimalityTranscript::new(
                MinimalityClass::OneMinimal,
                prefix.clone(),
                None,
                prefix
                    .iter()
                    .map(|id| (BTreeSet::from([id.clone()]), WitnessOutcome::Destroyed)),
            ),
        );
        assert_eq!(
            verdict,
            TranscriptVerdict::Refused(MinimalityRefusal::NoWitness)
        );
        assert_eq!(verdict.as_str(), "minimality-refused");
    }

    #[test]
    fn a_dimension_class_is_refused_without_the_audits_own_attribution() {
        let dimensions = DimensionSet::of([DimensionAttribution::new(
            MinimizedDimension::Owner,
            [
                (name("e_begin"), name("o_client")),
                (name("e_submit"), name("o_client")),
                (name("e_ack"), name("o_server")),
                (name("e_loss"), name("o_server")),
            ],
        )]);
        let transcript = MinimalitySearch::transcribe(
            MinimalityClass::OwnerMinimal,
            &order(),
            &automaton(),
            &core(),
            &dimensions,
            None,
        )
        .expect("transcribed");
        // The audit declared nothing, so it refuses rather than borrowing the question's.
        assert_eq!(
            audit().verdict(&order(), &core(), &transcript),
            TranscriptVerdict::Refused(MinimalityRefusal::DimensionNotDeclared {
                dimension: MinimizedDimension::Owner
            })
        );
        // Declared, and the same transcript verifies.
        assert_eq!(
            MinimalityAudit::of(automaton(), dimensions)
                .verdict(&order(), &core(), &transcript)
                .licensed(),
            Some(MinimalityClass::OwnerMinimal)
        );
    }

    #[test]
    fn a_stricter_audit_than_the_search_refuses_the_class() {
        // The two premises are supplied separately, so a caller may audit against a stricter
        // automaton than the search was given. The audit wins.
        let state = |text: &str| AutomatonState::new(name(text));
        let lenient = PropertyAutomaton::new(
            state("q_0"),
            [(state("q_0"), name("e_loss"), state("q_bad"))],
            [state("q_bad")],
            [],
            Coverage::Total,
        )
        .expect("a well-formed automaton");
        // Under the lenient property only `e_loss` matters, so the core is not one-minimal —
        // and the search over it says so, while the strict automaton's own transcript verifies.
        let transcript = MinimalitySearch::transcribe(
            MinimalityClass::OneMinimal,
            &order(),
            &automaton(),
            &core(),
            &DimensionSet::empty(),
            None,
        )
        .expect("transcribed");
        assert!(
            audit()
                .verdict(&order(), &core(), &transcript)
                .is_verified()
        );
        assert!(matches!(
            MinimalityAudit::over(lenient).verdict(&order(), &core(), &transcript),
            TranscriptVerdict::Defective(TranscriptDefect::ForgedOutcome { .. })
        ));
    }

    #[test]
    fn every_defect_token_is_a_distinct_token() {
        let defects = [
            TranscriptDefect::CoreMismatch { id: name("e_ack") },
            TranscriptDefect::CoreNotCausallyClosed(ClosureViolation::MissingPredecessor {
                selected: name("e_ack"),
                predecessor: name("e_submit"),
            }),
            TranscriptDefect::BoundNotApplicable {
                class: MinimalityClass::OneMinimal,
            },
            TranscriptDefect::UnitNotExamined {
                removed: ids(&["e_ack"]),
            },
            TranscriptDefect::UnitNotRequired {
                removed: ids(&["e_ack"]),
            },
            TranscriptDefect::ForgedOutcome {
                removed: ids(&["e_ack"]),
                recorded: WitnessOutcome::Destroyed,
                derived: WitnessOutcome::Survived,
            },
        ];
        let tokens: BTreeSet<&str> = defects.iter().map(TranscriptDefect::as_str).collect();
        assert_eq!(tokens.len(), 6);
        for defect in &defects {
            assert!(!defect.to_string().is_empty());
            assert!(!defect.as_str().contains(' '));
        }
    }

    #[test]
    fn two_audits_of_one_transcript_agree() {
        let transcript = transcribe(MinimalityClass::CausallyMinimal, &core());
        assert_eq!(
            audit().verdict(&order(), &core(), &transcript),
            audit().verdict(&order(), &core(), &transcript)
        );
    }
}
