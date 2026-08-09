//! **PR-11 / bn-3ub6i — the Context Pack compiler's stage group 4 (RFC 0028 stage 8).**
//!
//! Minimal unsatisfied core / correction-set analysis: the six transcript-licensed minimality
//! classes, each licensed **only by its own transcript**, with an independent audit that verifies
//! the transcript rather than trusting the analyzer that wrote it.
//!
//! # Honest scope, stated before any number
//!
//! This suite exercises stage 8 on top of the landed stages 1–3, and nothing else:
//!
//! - **What it claims.** The six classes RFC 0028's guarantee table calls transcript-checkable —
//!   `OneMinimal`, `CardinalityMinimal`, `CausallyMinimal`, `ValueMinimal`, `OwnerMinimal`,
//!   `FaultMinimal` — are licensed here, each from its own transcript and each only after an
//!   independent audit re-derived every recorded outcome.
//! - **What it does not claim.** `ExplanationMinimal` is **not emittable** and is pinned as
//!   unreachable rather than merely unclaimed (RFC 0028 correction 5: it "requires the docs/48
//!   preregistered study […] and MUST NOT be emitted from a minimizer transcript"). Nothing here
//!   claims `ReplayPreserving` (stage 2 supplies only its precondition), `ProofRelevant` (stage 5
//!   refuses; there is no proof service in this workspace), `HeuristicRelevant` (stage 9 is
//!   deliberately unbuilt) or `CounterfactualUnderNamedModel`.
//! - **What "minimal" means here.** The *witness* is the property automaton's `Violated` verdict
//!   over the published selection — RFC 0028's failure-pack profile, "a question about a `refuted`
//!   or `deadlock` verdict". Minimality is therefore relative to a property, and stage 8 without
//!   stage 3 refuses rather than inventing one.
//! - **What stage 8 does not do.** It does not minimise the pack. RFC 0028 states each class about
//!   the *selected* set, so stage 8 analyses what stages 1–7 published; the omission manifest and
//!   INV-007's counting equation are untouched, and no manifest cell is invented for a redundant
//!   item (`crate::minimality`'s "What is declined here").
//!
//! The reduction reported below is at **candidate count** (205 → 5), not bytes: bytes are
//! `content_budget.bytes` on an assembled root pack (RFC 0028 correction 17), and root assembly is
//! a later bone's.
//!
//! # RFC 0028's named mutants, at this stage
//!
//! > **Guarantee-violation mutants** are rejected: […] **a minimality class with no transcript**.
//! >
//! > — RFC 0028, "Validation"
//!
//! That one, and the neighbours this stage can produce: a survivable single-item removal refuting
//! `OneMinimal`; a removal-minimized slice that is no longer downward closed (the finite shadow of
//! Lean obligation 2, "minimality soundness"); a forged outcome; a truncated family; a padded one;
//! a transcript about some other core; and `CardinalityMinimal` claimed by an unbounded search.

use std::collections::{BTreeMap, BTreeSet};

use continuum_context::causal::{CausalOrder, ClosureViolation};
use continuum_context::compile::{CausalCompile, Compilation, RedactionPolicy};
use continuum_context::expansion::{ExpansionQuery, ExpansionRelation};
use continuum_context::guarantee::{Guarantee, RequestedGuarantees};
use continuum_context::minimality::{
    DimensionAttribution, DimensionSet, MINIMALITY_GUARANTEES, MinimalityClass, MinimalityOutcome,
    MinimalityQuestion, MinimalityRefusal, MinimalityTranscript, MinimizedDimension,
    MinimizerArtifact, SearchBound, WitnessOutcome,
};
use continuum_context::monitor::PropertyMonitor;
use continuum_context::omission::OmissionReason;
use continuum_context::property::{AutomatonState, Coverage, PropertyAutomaton};
use continuum_context::selection::SelectionKind;
use continuum_context::stage::{Phase, Stage};
use continuum_context::witness::{MinimalityAudit, TranscriptDefect, TranscriptVerdict};
use continuum_value::identity::Blake3Hasher;
use continuum_value::value::Name;

// --- the fixture ---------------------------------------------------------------------------

/// The G0-DX-01 shape: one five-event causal chain, and two hundred causally isolated noise
/// events the property never steps on.
const CHAIN: [&str; 5] = ["e_begin", "e_submit", "e_ack", "e_retry", "e_loss"];
const NOISE: usize = 200;

fn name(text: &str) -> Name {
    Name::new(text).expect("well formed")
}

fn ids(names: &[&str]) -> BTreeSet<Name> {
    names.iter().map(|id| name(id)).collect()
}

fn chain() -> BTreeSet<Name> {
    CHAIN.iter().map(|id| name(id)).collect()
}

fn order() -> CausalOrder {
    let mut nodes: Vec<(Name, SelectionKind)> = CHAIN
        .iter()
        .map(|id| (name(id), SelectionKind::Event))
        .collect();
    for index in 0..NOISE {
        nodes.push((name(&format!("n_beat_{index:03}")), SelectionKind::Event));
    }
    let edges: Vec<(Name, Name)> = CHAIN
        .windows(2)
        .map(|pair| (name(pair[1]), name(pair[0])))
        .collect();
    CausalOrder::new(nodes, edges).expect("a well-formed order")
}

fn state(text: &str) -> AutomatonState {
    AutomatonState::new(name(text))
}

/// Steps on every member of the chain in sequence and violates only at the end, so the chain's
/// closure really is its own minimal witness. `hidden` names abstraction-relevant events the
/// property depends on without observing.
fn automaton(hidden: &[&str]) -> PropertyAutomaton {
    let mut transitions = Vec::new();
    for (index, event) in CHAIN.iter().enumerate() {
        let to = if index + 1 == CHAIN.len() {
            state("q_bad")
        } else {
            state(&format!("q_{}", index + 1))
        };
        transitions.push((state(&format!("q_{index}")), name(event), to));
    }
    PropertyAutomaton::new(
        state("q_0"),
        transitions,
        [state("q_bad")],
        hidden.iter().map(|id| name(id)),
        Coverage::Total,
    )
    .expect("a well-formed automaton")
}

/// Three different partitions of the chain, so the three dimension classes really do have three
/// different removal families.
fn dimensions() -> DimensionSet {
    let attribute = |dimension, pairs: &[(&str, &str)]| {
        DimensionAttribution::new(
            dimension,
            pairs
                .iter()
                .map(|(candidate, element)| (name(candidate), name(element))),
        )
    };
    DimensionSet::of([
        attribute(
            MinimizedDimension::Value,
            &[
                ("e_begin", "v_id"),
                ("e_submit", "v_id"),
                ("e_ack", "v_seq"),
                ("e_retry", "v_seq"),
                ("e_loss", "v_seq"),
            ],
        ),
        attribute(
            MinimizedDimension::Owner,
            &[
                ("e_begin", "o_client"),
                ("e_submit", "o_client"),
                ("e_ack", "o_client"),
                ("e_retry", "o_server"),
                ("e_loss", "o_server"),
            ],
        ),
        attribute(
            MinimizedDimension::Fault,
            &[
                ("e_begin", "f_none"),
                ("e_submit", "f_none"),
                ("e_ack", "f_none"),
                ("e_retry", "f_none"),
                ("e_loss", "f_drop"),
            ],
        ),
    ])
}

fn residual() -> ExpansionQuery {
    ExpansionQuery::new(ExpansionRelation::CausalPredecessors, name("e_loss"))
}

fn all_classes() -> MinimalityQuestion {
    MinimalityQuestion::asking(MinimalityClass::ALL)
        .with_dimensions(dimensions())
        .with_bound(SearchBound::of(2).expect("a positive bound"))
}

/// The audit, built over its *own* automaton and its own attribution — the second authority.
fn audit() -> MinimalityAudit {
    MinimalityAudit::of(automaton(&[]), dimensions())
}

/// Stages 1–3 and 8, over the fixture.
fn compile(question: MinimalityQuestion) -> CausalCompile {
    CausalCompile::new(order(), RedactionPolicy::permitting_everything())
        .with_property(automaton(&[]), PropertyMonitor::over(automaton(&[])))
        .with_minimality(question, audit())
}

fn compiled(question: MinimalityQuestion) -> Compilation {
    compile(question)
        .run(&ids(&["e_loss"]), &residual())
        .expect("compiles")
}

fn selection(compiled: &Compilation) -> BTreeSet<Name> {
    compiled.selected().iter().cloned().collect()
}

/// A copy of `transcript` with one removal's recorded outcome replaced.
fn with_step(
    transcript: &MinimalityTranscript,
    removed: BTreeSet<Name>,
    outcome: WitnessOutcome,
) -> MinimalityTranscript {
    let mut steps: BTreeMap<BTreeSet<Name>, WitnessOutcome> = transcript
        .steps()
        .map(|(unit, recorded)| (unit.clone(), recorded))
        .collect();
    steps.insert(removed, outcome);
    MinimalityTranscript::new(
        transcript.class(),
        transcript.core().iter().cloned(),
        transcript.bound(),
        steps,
    )
}

/// A copy of `transcript` with one removal dropped entirely.
fn without_step(
    transcript: &MinimalityTranscript,
    removed: &BTreeSet<Name>,
) -> MinimalityTranscript {
    let steps: BTreeMap<BTreeSet<Name>, WitnessOutcome> = transcript
        .steps()
        .filter(|(unit, _)| *unit != removed)
        .map(|(unit, recorded)| (unit.clone(), recorded))
        .collect();
    MinimalityTranscript::new(
        transcript.class(),
        transcript.core().iter().cloned(),
        transcript.bound(),
        steps,
    )
}

/// A copy of `transcript` with its first recorded outcome flipped — the forgery an analyzer that
/// wanted the class would produce.
fn forged(transcript: &MinimalityTranscript) -> MinimalityTranscript {
    let (removed, recorded) = transcript.steps().next().expect("at least one removal");
    let flipped = match recorded {
        WitnessOutcome::Destroyed => WitnessOutcome::Survived,
        WitnessOutcome::Survived => WitnessOutcome::Destroyed,
    };
    with_step(transcript, removed.clone(), flipped)
}

// --- positive ------------------------------------------------------------------------------

#[test]
fn the_eight_stage_pipeline_runs_end_to_end_and_licenses_six_minimality_classes() {
    let compiled = compiled(all_classes());

    // 205 candidates in, 5 published: the chain, and only the chain.
    assert_eq!(compiled.accounting().candidate_count(), 205);
    assert_eq!(selection(&compiled), chain());
    assert_eq!(compiled.accounting().manifest().total(), 200);

    assert_eq!(
        compiled.trail().phases(),
        [
            Phase::Redaction,
            Phase::Stage(Stage::RootSelection),
            Phase::Stage(Stage::CausalSlicing),
            Phase::Stage(Stage::PropertyRelevance),
            Phase::Stage(Stage::MinimalCore),
        ]
    );
    assert_eq!(
        compiled.minimality_outcome(),
        Some(MinimalityOutcome::Licensed)
    );

    // All six classes, each from its own transcript, and the guarantee set in schema order.
    assert_eq!(
        compiled.minimality_classes(),
        MinimalityClass::ALL.into_iter().collect()
    );
    assert_eq!(
        compiled.guarantees().members(),
        [
            Guarantee::PropertyPreserving,
            Guarantee::OneMinimal,
            Guarantee::CardinalityMinimal,
            Guarantee::CausallyMinimal,
            Guarantee::ValueMinimal,
            Guarantee::OwnerMinimal,
            Guarantee::FaultMinimal,
            Guarantee::CausallyClosed,
        ]
    );
    // The transcripts are kept as the evidence the audit checked, one per class, each over the
    // published selection and each with the family its own class requires.
    let sizes: Vec<usize> = MinimalityClass::ALL
        .into_iter()
        .map(|class| {
            compiled
                .minimality_transcript(class)
                .expect("a transcript per class")
                .len()
        })
        .collect();
    // 5 singletons; 5 + 10 subsets within the bound; 1 maximal element; and 2 elements in each
    // of the three declared dimensions.
    assert_eq!(sizes, [5, 15, 1, 2, 2, 2]);
}

#[test]
fn every_published_transcript_is_verified_by_an_audit_run_a_second_time_from_this_test() {
    // The licence is only worth what the checker is: this test re-runs the audit itself, over the
    // selection the pack actually publishes, and gets the same six verdicts.
    let compiled = compiled(all_classes());
    let selection = selection(&compiled);
    for class in MinimalityClass::ALL {
        let transcript = compiled
            .minimality_transcript(class)
            .expect("a transcript per class");
        assert_eq!(
            audit().verdict(&order(), &selection, transcript),
            TranscriptVerdict::Verified {
                class,
                examined: transcript.len(),
            },
            "{class}"
        );
    }
}

#[test]
fn each_class_is_licensed_only_by_its_own_transcript() {
    // Rule C3: "A pack MUST claim exactly the classes whose checks ran and passed, and a consumer
    // MUST NOT infer one class from another on the wire." Asking for one class licenses exactly
    // that one — including for `OneMinimal`, whose removals are also a `CardinalityMinimal`
    // family at bound 1 and, under a one-candidate-per-element attribution, a `ValueMinimal` one.
    for class in MinimalityClass::ALL {
        let question = MinimalityQuestion::asking([class])
            .with_dimensions(dimensions())
            .with_bound(SearchBound::of(2).expect("positive"));
        let compiled = compiled(question);
        assert_eq!(
            compiled.minimality_classes(),
            BTreeSet::from([class]),
            "{class}"
        );
        assert_eq!(
            compiled.guarantees().members(),
            [
                Guarantee::PropertyPreserving,
                class.guarantee(),
                Guarantee::CausallyClosed,
            ]
            .into_iter()
            .collect::<BTreeSet<Guarantee>>()
            .into_iter()
            .collect::<Vec<Guarantee>>(),
            "{class}"
        );
        // No other class is even decided: unattempted is not the same fact as refused.
        assert_eq!(compiled.minimality_verdicts().len(), 1);
    }
}

#[test]
fn a_solver_artifact_files_each_transcript_under_its_own_class() {
    // RFC 0028's stage-8 input is "a solver artifact, where one exists". Whoever produced the
    // transcripts, the artifact's index must agree with their contents — an index that disagreed
    // would make rule C3's forbidden inference on the consumer's behalf.
    let compiled = compiled(all_classes());
    let artifact = MinimizerArtifact::of(MinimalityClass::ALL.into_iter().map(|class| {
        (
            class,
            compiled
                .minimality_transcript(class)
                .expect("transcribed")
                .clone(),
        )
    }))
    .expect("each filed under its own class");
    assert_eq!(artifact.len(), 6);
    assert_eq!(
        artifact.classes(),
        MinimalityClass::ALL.into_iter().collect()
    );
    assert!(
        MinimizerArtifact::of([(
            MinimalityClass::ValueMinimal,
            compiled
                .minimality_transcript(MinimalityClass::OneMinimal)
                .expect("transcribed")
                .clone(),
        )])
        .is_err()
    );
    assert!(MinimizerArtifact::none().is_empty());
}

// --- negative: the transcript is not believed ------------------------------------------------

#[test]
fn a_forged_transcript_licenses_nothing_for_any_class() {
    // The evidence-not-assertion idiom, made falsifiable once per class: the analyzer claims an
    // outcome the premise does not produce, and the audit names the forgery instead of trusting it.
    let compiled = compiled(all_classes());
    let selection = selection(&compiled);
    for class in MinimalityClass::ALL {
        let honest = compiled
            .minimality_transcript(class)
            .expect("a transcript per class");
        assert!(
            audit().verdict(&order(), &selection, honest).is_verified(),
            "{class}"
        );
        assert!(
            matches!(
                audit().verdict(&order(), &selection, &forged(honest)),
                TranscriptVerdict::Defective(TranscriptDefect::ForgedOutcome { .. })
            ),
            "{class}"
        );
    }
}

#[test]
fn a_minimality_class_with_no_transcript_is_rejected() {
    // RFC 0028's own named guarantee-violation mutant, once per class: the claim is made and the
    // evidence is absent. Reported as the first required removal nobody examined.
    let compiled = compiled(all_classes());
    let selection = selection(&compiled);
    for class in MinimalityClass::ALL {
        let empty = MinimalityTranscript::new(
            class,
            selection.iter().cloned(),
            compiled
                .minimality_transcript(class)
                .expect("transcribed")
                .bound(),
            [],
        );
        assert!(
            matches!(
                audit().verdict(&order(), &selection, &empty),
                TranscriptVerdict::Defective(TranscriptDefect::UnitNotExamined { .. })
            ),
            "{class}"
        );
    }
}

#[test]
fn a_truncated_or_padded_transcript_licenses_nothing_for_any_class() {
    let compiled = compiled(all_classes());
    let selection = selection(&compiled);
    for class in MinimalityClass::ALL {
        let honest = compiled
            .minimality_transcript(class)
            .expect("a transcript per class");
        let (first, _) = honest.steps().next().expect("at least one removal");
        // One required removal skipped: the class was established over part of its family, which
        // is not the class.
        assert!(
            matches!(
                audit().verdict(&order(), &selection, &without_step(honest, first)),
                TranscriptVerdict::Defective(TranscriptDefect::UnitNotExamined { .. })
            ),
            "{class}"
        );
        // One removal the class does not require, added: the coverage claim is about some other
        // family. `n_beat_000` is not even in the core.
        assert!(
            matches!(
                audit().verdict(
                    &order(),
                    &selection,
                    &with_step(honest, ids(&["n_beat_000"]), WitnessOutcome::Destroyed)
                ),
                TranscriptVerdict::Defective(TranscriptDefect::UnitNotRequired { .. })
            ),
            "{class}"
        );
    }
}

#[test]
fn a_transcript_about_another_core_licenses_nothing() {
    let compiled = compiled(all_classes());
    let honest = compiled
        .minimality_transcript(MinimalityClass::OneMinimal)
        .expect("transcribed");
    // Sound evidence, about a set this pack does not publish.
    assert_eq!(
        audit().verdict(&order(), &ids(&["e_begin", "e_submit", "e_ack"]), honest),
        TranscriptVerdict::Defective(TranscriptDefect::CoreMismatch { id: name("e_loss") })
    );
}

#[test]
fn a_removal_minimized_slice_that_is_no_longer_downward_closed_establishes_nothing() {
    // The finite shadow of RFC 0028's Lean obligation 2 — "minimality soundness: a 1-minimal
    // slice under the removal check is still a slice, that is, removal-minimization preserves
    // downward closure". A minimizer that dropped `e_begin` to get a smaller core produced
    // something that is not a slice, and no class is established about it — which is also exactly
    // how it would have broken stage 2's `CausallyClosed`.
    let unclosed: BTreeSet<Name> = ids(&["e_submit", "e_ack", "e_retry", "e_loss"]);
    assert!(order().closure_violation(&unclosed).is_some());
    for class in MinimalityClass::ALL {
        let transcript = MinimalityTranscript::new(
            class,
            unclosed.iter().cloned(),
            if class.takes_bound() {
                SearchBound::of(2)
            } else {
                None
            },
            unclosed
                .iter()
                .map(|id| (BTreeSet::from([id.clone()]), WitnessOutcome::Destroyed)),
        );
        assert_eq!(
            audit().verdict(&order(), &unclosed, &transcript),
            TranscriptVerdict::Defective(TranscriptDefect::CoreNotCausallyClosed(
                ClosureViolation::MissingPredecessor {
                    selected: name("e_submit"),
                    predecessor: name("e_begin"),
                }
            )),
            "{class}"
        );
    }
}

#[test]
fn a_survivable_single_item_removal_refutes_one_minimal() {
    // RFC 0028's named failure mode for `OneMinimal`: "a survivable removal refutes it".
    // `n_beat_000` is declared an abstraction-relevant hidden event, so stage 3 MUST keep it
    // ("a compiler MUST NOT exclude an abstraction-relevant hidden event on the grounds that no
    // observer publishes it") and the automaton never steps on it. The pack is causally closed and
    // property-preserving and is *not* one-minimal, and the honest answer is to say so.
    let compiled = CausalCompile::new(order(), RedactionPolicy::permitting_everything())
        .with_property(
            automaton(&["n_beat_000"]),
            PropertyMonitor::over(automaton(&["n_beat_000"])),
        )
        .with_minimality(
            MinimalityQuestion::asking([MinimalityClass::OneMinimal]),
            MinimalityAudit::over(automaton(&["n_beat_000"])),
        )
        .run(&ids(&["e_loss", "n_beat_000"]), &residual())
        .expect("compiles");

    assert!(selection(&compiled).contains(&name("n_beat_000")));
    assert_eq!(
        compiled.minimality_verdict(MinimalityClass::OneMinimal),
        Some(&TranscriptVerdict::Refuted {
            removed: ids(&["n_beat_000"])
        })
    );
    assert!(compiled.minimality_classes().is_empty());
    assert_eq!(
        compiled.minimality_outcome(),
        Some(MinimalityOutcome::Unlicensed)
    );
    // A refutation costs the class and nothing else: the earlier stages' claims stand, and the
    // transcript that refuted it is kept as evidence rather than discarded.
    assert_eq!(
        compiled.guarantees().members(),
        [Guarantee::PropertyPreserving, Guarantee::CausallyClosed]
    );
    assert_eq!(
        compiled
            .minimality_transcript(MinimalityClass::OneMinimal)
            .expect("transcribed")
            .outcome(&ids(&["n_beat_000"])),
        Some(WitnessOutcome::Survived)
    );
}

#[test]
fn cardinality_minimality_is_refused_without_declared_bounds() {
    // RFC 0028: "an unbounded search cannot claim it." The refusal is the *checker's*, so a
    // transcript from any producer is held to it.
    let refused = compiled(MinimalityQuestion::asking([
        MinimalityClass::CardinalityMinimal,
    ]));
    assert_eq!(
        refused.minimality_verdict(MinimalityClass::CardinalityMinimal),
        Some(&TranscriptVerdict::Refused(
            MinimalityRefusal::UnboundedSearch
        ))
    );
    assert!(refused.minimality_classes().is_empty());
    assert!(
        !refused
            .guarantees()
            .members()
            .contains(&Guarantee::CardinalityMinimal)
    );

    // Declared, and the class is licensed. The bound is the whole difference.
    let bounded = compiled(
        MinimalityQuestion::asking([MinimalityClass::CardinalityMinimal])
            .with_bound(SearchBound::of(1).expect("positive")),
    );
    assert_eq!(
        bounded.minimality_classes(),
        BTreeSet::from([MinimalityClass::CardinalityMinimal])
    );
    assert_eq!(
        bounded
            .minimality_transcript(MinimalityClass::CardinalityMinimal)
            .expect("transcribed")
            .bound(),
        SearchBound::of(1)
    );
}

#[test]
fn a_dimension_class_without_a_declared_attribution_is_refused() {
    // INV-013's discipline at stage 8: a reduction along a dimension nobody described is justified
    // by nothing. The two absences are distinguished (INV-008) — nothing declared at all, and a
    // declaration that omits a selected item.
    let undeclared = compiled(MinimalityQuestion::asking([MinimalityClass::OwnerMinimal]));
    assert_eq!(
        undeclared.minimality_verdict(MinimalityClass::OwnerMinimal),
        Some(&TranscriptVerdict::Refused(
            MinimalityRefusal::DimensionNotDeclared {
                dimension: MinimizedDimension::Owner
            }
        ))
    );

    let partial = DimensionSet::of([DimensionAttribution::new(
        MinimizedDimension::Owner,
        [(name("e_begin"), name("o_client"))],
    )]);
    let incomplete = CausalCompile::new(order(), RedactionPolicy::permitting_everything())
        .with_property(automaton(&[]), PropertyMonitor::over(automaton(&[])))
        .with_minimality(
            MinimalityQuestion::asking([MinimalityClass::OwnerMinimal]).with_dimensions(partial),
            MinimalityAudit::over(automaton(&[])),
        )
        .run(&ids(&["e_loss"]), &residual())
        .expect("compiles");
    assert!(matches!(
        incomplete.minimality_verdict(MinimalityClass::OwnerMinimal),
        Some(&TranscriptVerdict::Refused(
            MinimalityRefusal::UndeclaredAttribution { .. }
        ))
    ));
    assert!(incomplete.minimality_classes().is_empty());
}

#[test]
fn stage_eight_without_stage_three_refuses_every_class() {
    // Minimality is relative to the property whose violation the core witnesses. With no
    // automaton there is nothing here that can decide whether a reduced set still witnesses
    // anything, so every asked-for class is refused by name — never skipped, never assumed.
    let compiled = CausalCompile::new(order(), RedactionPolicy::permitting_everything())
        .with_minimality(all_classes(), audit())
        .run(&ids(&["e_loss"]), &residual())
        .expect("compiles");

    assert_eq!(
        compiled.minimality_outcome(),
        Some(MinimalityOutcome::NoProperty)
    );
    for class in MinimalityClass::ALL {
        assert_eq!(
            compiled.minimality_verdict(class),
            Some(&TranscriptVerdict::Refused(MinimalityRefusal::NoProperty)),
            "{class}"
        );
    }
    assert!(compiled.minimality_transcripts().is_empty());
    assert_eq!(compiled.guarantees().members(), [Guarantee::CausallyClosed]);
}

// --- the two pins ----------------------------------------------------------------------------

#[test]
fn explanation_minimal_is_not_emittable_from_any_transcript() {
    // RFC 0028 correction 5: `ExplanationMinimal` "requires the docs/48 preregistered study (plan
    // §21.1, G8) and MUST NOT be emitted from a minimizer transcript". The pin is structural, in
    // four places at once.
    //
    // 1. The RFC's minimality table has seven members and the transcript-checkable class
    //    vocabulary has six; the difference is exactly this one.
    let classed: BTreeSet<Guarantee> = MinimalityClass::ALL
        .into_iter()
        .map(MinimalityClass::guarantee)
        .collect();
    let tabled: BTreeSet<Guarantee> = MINIMALITY_GUARANTEES.into_iter().collect();
    assert_eq!(
        tabled.difference(&classed).copied().collect::<Vec<_>>(),
        [Guarantee::ExplanationMinimal]
    );
    // 2. There is no class value for it, so no transcript can be recorded for it and no artifact
    //    can carry one.
    assert_eq!(MinimalityClass::of(Guarantee::ExplanationMinimal), None);
    // 3. The strongest configuration this crate can build — every class asked for, all six
    //    verified — does not emit it.
    let compiled = compiled(all_classes());
    assert_eq!(compiled.minimality_classes().len(), 6);
    assert!(
        !compiled
            .guarantees()
            .members()
            .contains(&Guarantee::ExplanationMinimal)
    );
    // 4. A caller who requests it by name gets it back as unachieved, never echoed (rule C1).
    let requested = RequestedGuarantees::of([
        Guarantee::ExplanationMinimal,
        Guarantee::OneMinimal,
        Guarantee::CausallyClosed,
    ]);
    assert_eq!(requested.members().len(), 3);
    assert!(requested.members().contains(&Guarantee::ExplanationMinimal));
    // The wire token exists — the schema's enum is thirteen members — and nothing in this crate
    // can put it in a pack. That is the whole distinction between a vocabulary and a claim.
    assert_eq!(
        Guarantee::from_wire_str("ExplanationMinimal"),
        Some(Guarantee::ExplanationMinimal)
    );
}

#[test]
fn minimal_without_a_class_is_not_a_spelling_in_any_rendering() {
    // Rule C3's rendering half: "`minimal` without a class is prohibited output in every
    // rendering" (plan §6.4). It is not a rule anybody has to remember here, because the bare
    // word is not a token: every minimality guarantee's wire spelling carries its class, and no
    // classless spelling parses.
    for candidate in ["minimal", "Minimal", "minimality", "1-minimal", "Minimal "] {
        assert_eq!(Guarantee::from_wire_str(candidate), None, "{candidate}");
    }
    for guarantee in MINIMALITY_GUARANTEES {
        let token = guarantee.as_wire_str();
        assert!(token.ends_with("Minimal"), "{token}");
        assert!(token.len() > "Minimal".len(), "{token}");
    }
    // And the compiler's own intermediate tokens are the same shape.
    for class in MinimalityClass::ALL {
        assert!(class.as_str().ends_with("-minimal"), "{class}");
        assert_ne!(class.as_str(), "minimal");
    }
}

// --- boundary --------------------------------------------------------------------------------

#[test]
fn a_pack_that_does_not_witness_the_failure_refuses_every_class() {
    // Anti-vacuity: rooted at `e_retry` the pack runs the automaton to a non-violating state, so
    // there is no witness for a removal to destroy — and "every removal destroyed it" would be
    // trivially true. Refused, not verified.
    let compiled = compile(all_classes())
        .run(
            &ids(&["e_retry"]),
            &ExpansionQuery::new(ExpansionRelation::CausalPredecessors, name("e_retry")),
        )
        .expect("compiles");
    assert_eq!(selection(&compiled).len(), 4);
    for class in MinimalityClass::ALL {
        assert_eq!(
            compiled.minimality_verdict(class),
            Some(&TranscriptVerdict::Refused(MinimalityRefusal::NoWitness)),
            "{class}"
        );
    }
    assert!(compiled.minimality_classes().is_empty());
}

#[test]
fn an_empty_pack_refuses_every_class() {
    let compiled = CausalCompile::new(
        CausalOrder::new([], []).expect("an empty order is well formed"),
        RedactionPolicy::permitting_everything(),
    )
    .with_property(automaton(&[]), PropertyMonitor::over(automaton(&[])))
    .with_minimality(all_classes(), audit())
    .run(&BTreeSet::new(), &residual())
    .expect("compiles");

    assert!(compiled.selected().is_empty());
    for class in MinimalityClass::ALL {
        assert_eq!(
            compiled.minimality_verdict(class),
            Some(&TranscriptVerdict::Refused(MinimalityRefusal::EmptyCore)),
            "{class}"
        );
    }
}

#[test]
fn a_bound_at_the_core_size_examines_every_proper_reduction_including_the_empty_one() {
    // Boundary: with the bound at the core's size the family is every non-empty subset, so the
    // transcript covers removing the whole core and leaving nothing.
    let compiled = compiled(
        MinimalityQuestion::asking([MinimalityClass::CardinalityMinimal])
            .with_bound(SearchBound::of(CHAIN.len()).expect("positive")),
    );
    let transcript = compiled
        .minimality_transcript(MinimalityClass::CardinalityMinimal)
        .expect("transcribed");
    assert_eq!(transcript.len(), (1 << CHAIN.len()) - 1);
    assert_eq!(
        transcript.outcome(&chain()),
        Some(WitnessOutcome::Destroyed)
    );
    assert_eq!(
        compiled.minimality_classes(),
        BTreeSet::from([MinimalityClass::CardinalityMinimal])
    );
}

#[test]
fn a_dimension_with_one_element_verifies_and_says_nothing_more_than_it_checked() {
    // Boundary, recorded rather than hidden: a value domain with a single element has a single
    // removal unit — the whole core — so the class verifies on one run. That is a true and weak
    // claim, and the transcript's own size is what says how weak: `examined: 1`. Nothing here
    // upgrades it, and rule C3 keeps a consumer from reading it as any other class.
    let single = DimensionSet::of([DimensionAttribution::new(
        MinimizedDimension::Value,
        CHAIN.iter().map(|id| (name(id), name("v_only"))),
    )]);
    let compiled = CausalCompile::new(order(), RedactionPolicy::permitting_everything())
        .with_property(automaton(&[]), PropertyMonitor::over(automaton(&[])))
        .with_minimality(
            MinimalityQuestion::asking([MinimalityClass::ValueMinimal])
                .with_dimensions(single.clone()),
            MinimalityAudit::of(automaton(&[]), single),
        )
        .run(&ids(&["e_loss"]), &residual())
        .expect("compiles");

    assert_eq!(
        compiled.minimality_verdict(MinimalityClass::ValueMinimal),
        Some(&TranscriptVerdict::Verified {
            class: MinimalityClass::ValueMinimal,
            examined: 1,
        })
    );
    assert_eq!(
        compiled.minimality_classes(),
        BTreeSet::from([MinimalityClass::ValueMinimal])
    );
}

// --- stage 8 changes nothing else --------------------------------------------------------------

#[test]
fn stage_eight_narrows_nothing_and_leaves_the_counting_equation_alone() {
    let with = compiled(all_classes());
    let without = CausalCompile::new(order(), RedactionPolicy::permitting_everything())
        .with_property(automaton(&[]), PropertyMonitor::over(automaton(&[])))
        .run(&ids(&["e_loss"]), &residual())
        .expect("compiles");

    assert_eq!(with.selected(), without.selected());
    assert_eq!(
        with.accounting().manifest(),
        without.accounting().manifest()
    );
    with.accounting().reconcile().expect("the two halves agree");
    assert_eq!(
        u64::from(with.accounting().candidate_count()),
        with.accounting().manifest().total() + with.accounting().selected().len() as u64
    );
    // Every drop is stage 2's, under a complete order, and none of them is stage 8's — stage 8
    // invents no manifest cell (`crate::correspondence`'s discipline, restated).
    for record in with.accounting().manifest().records() {
        assert_eq!(record.reason(), OmissionReason::SliceIrrelevant);
        assert!(record.retrievability().is_expandable());
    }
    // Stage 8's intermediate is stage 3's working set, unchanged.
    assert_eq!(
        with.trail()
            .intermediate(Phase::Stage(Stage::MinimalCore))
            .expect("stage 8 recorded")
            .working_set(),
        with.trail()
            .intermediate(Phase::Stage(Stage::PropertyRelevance))
            .expect("stage 3 recorded")
            .working_set()
    );
    // And an unclaimed class owes the verdict no reason: RFC 0028's failure profile says a pack
    // SHOULD claim a minimality class, not MUST.
    assert_eq!(with.inconclusive_reason(), None);
}

#[test]
fn a_stage_eight_that_was_asked_for_nothing_is_not_a_stage_eight_that_did_not_run() {
    let asked_nothing = compiled(MinimalityQuestion::nothing_asked());
    assert_eq!(
        asked_nothing.minimality_outcome(),
        Some(MinimalityOutcome::NothingAsked)
    );
    assert_eq!(
        asked_nothing
            .trail()
            .intermediate(Phase::Stage(Stage::MinimalCore))
            .expect("stage 8 recorded")
            .note(),
        Some("minimality-nothing-asked")
    );
    assert!(asked_nothing.minimality_verdicts().is_empty());

    let unconfigured = CausalCompile::new(order(), RedactionPolicy::permitting_everything())
        .with_property(automaton(&[]), PropertyMonitor::over(automaton(&[])))
        .run(&ids(&["e_loss"]), &residual())
        .expect("compiles");
    assert!(
        unconfigured
            .trail()
            .intermediate(Phase::Stage(Stage::MinimalCore))
            .is_none()
    );
    assert_eq!(unconfigured.minimality_outcome(), None);
    // Same pack either way: what differs is the record of what was attempted.
    assert_eq!(asked_nothing.selected(), unconfigured.selected());
    assert_eq!(asked_nothing.guarantees(), unconfigured.guarantees());
}

// --- determinism (INV-005) ---------------------------------------------------------------------

#[test]
fn two_runs_of_an_eight_stage_compile_agree() {
    let build = || compiled(all_classes());
    assert_eq!(build(), build());
    assert_eq!(
        build().trail().to_json().to_canonical_bytes(),
        build().trail().to_json().to_canonical_bytes()
    );
    for class in MinimalityClass::ALL {
        let digest = |compiled: &Compilation| {
            compiled
                .minimality_transcript(class)
                .expect("transcribed")
                .digest::<Blake3Hasher>()
        };
        assert_eq!(digest(&build()), digest(&build()), "{class}");
    }
    // And a different transcript is a different digest, so the pin is not vacuous: the same class
    // over a different core hashes differently.
    let narrower = compile(all_classes())
        .run(
            &ids(&["e_retry"]),
            &ExpansionQuery::new(ExpansionRelation::CausalPredecessors, name("e_retry")),
        )
        .expect("compiles");
    assert!(
        narrower
            .minimality_transcript(MinimalityClass::OneMinimal)
            .is_none()
    );
    let shifted = MinimalityTranscript::new(
        MinimalityClass::OneMinimal,
        ids(&["e_begin", "e_submit"]),
        None,
        [(ids(&["e_begin"]), WitnessOutcome::Destroyed)],
    );
    assert_ne!(
        shifted.digest::<Blake3Hasher>(),
        build()
            .minimality_transcript(MinimalityClass::OneMinimal)
            .expect("transcribed")
            .digest::<Blake3Hasher>()
    );
}

// --- the independence pin ------------------------------------------------------------------------

/// `witness.rs`'s own bytes, so the independence claim is checked rather than asserted.
const WITNESS_SOURCE: &str = include_str!("../src/witness.rs");

/// A module's production half, with comment lines removed — `pr11_compiler_stages_5_7.rs`'s device.
fn production_code(source: &str) -> String {
    source
        .split("#[cfg(test)]")
        .next()
        .unwrap_or(source)
        .lines()
        .map(str::trim)
        .filter(|line| !line.starts_with("//"))
        .collect::<Vec<&str>>()
        .join("\n")
}

#[test]
fn the_witness_module_names_nothing_of_the_searchs() {
    // The mechanical half of `crate::witness`'s independence argument, and the exact device
    // `the_monitor_module_names_nothing_of_the_filters` uses for stage 3. Comment lines are
    // stripped first: this module's documentation *names* the producer it must not call, in the
    // argument for why it does not.
    let production = production_code(WITNESS_SOURCE);
    let modules: BTreeSet<&str> = production
        .match_indices("use crate::")
        .filter_map(|(at, _)| {
            production[at + "use crate::".len()..]
                .split("::")
                .next()
                .map(str::trim)
        })
        .collect();
    assert_eq!(
        modules,
        BTreeSet::from(["causal", "minimality", "property"]),
        "the audit reads a premise it does not own"
    );
    // It never reaches the producer, the pipeline, or a licence.
    assert!(!production.contains("MinimalitySearch"));
    assert!(!production.contains("use crate::compile"));
    assert!(!production.contains("License::issue"));
    assert!(!production.contains("use crate::guarantee"));
    // And it runs the automaton itself rather than calling the search's helper — the duplication
    // that makes the two computations two.
    assert!(production.contains("topological_order()"));
}
