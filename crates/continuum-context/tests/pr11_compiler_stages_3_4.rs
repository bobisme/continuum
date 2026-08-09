//! PR-11 / the Context Pack compiler, **stage group 2** (bn-1kj2n): stage 3
//! (property-automaton relevance filtering) and stage 4 (the static/dynamic dependence join),
//! exercised from outside the crate at the grain the G0-DX-01 row's experiment uses.
//!
//! # Why this file exists beside the unit tests
//!
//! The same two reasons `pr11_compiler_stages_1_2.rs` states, at this group's own surface:
//!
//! 1. **The public surface is the whole surface.** Everything below is reachable through
//!    `continuum_context::{causal, compile, dependence, guarantee, monitor, property, stage}`
//!    with no `pub(crate)` help — and, in the other direction, `guarantee::License::issue` is
//!    still not reachable, so `PropertyPreserving` cannot be minted from out here any more than
//!    `CausallyClosed` could.
//! 2. **The stage group behaves at the row's grain.** The 225-candidate order below is the
//!    stage-1–2 fixture widened by exactly what stages 3 and 4 need in order to be
//!    falsifiable: an abstraction-relevant hidden event no observer publishes, a
//!    property-irrelevant diagnostic chain, and three `source`/`model` candidates whose
//!    dependence is corroborated, uncorroborated, and corroborated-but-unpublished.
//!
//! # Honest scope, stated before any number
//!
//! This is **stage group 2 only**. The reduction reported below is at *candidate count*, not
//! at bytes — bytes are `content_budget.bytes` on an assembled root pack (RFC 0028 correction
//! 17) and root assembly is the last child of bn-8g9uj. Nothing here claims `ReplayPreserving`
//! (the replay checker is not this crate's), `ProofRelevant` (stage 5's, and refused there for
//! want of a proof-service artifact) or any minimality class (stage 8's). The two guarantees
//! this pipeline can now issue are `CausallyClosed` and `PropertyPreserving`, and every
//! assertion about the second below is decided by `monitor::PropertyMonitor`, which runs the
//! automaton over the whole causal order and over the proposed selection and compares — it
//! never reads the filter's output as authority for anything (RFC 0028, "Validation": "an
//! implementation that reuses the compiler's own automaton has checked nothing").
//!
//! Stage 4 licenses no guarantee at all. It licenses *admissibility*, and the one claim made
//! about it here is INV-016's: a dependence asserted only by the untrusted source
//! correspondence does not become a selected item.
//!
//! # House rules
//!
//! Deterministic (INV-005): no clock, no entropy, no float; every set is a `BTree` and the
//! whole fixture is a pure function of its own constants.

use std::collections::{BTreeMap, BTreeSet};

use continuum_context::causal::{CausalOrder, ClosureViolation};
use continuum_context::compile::{
    CausalCompile, Compilation, CompileError, RedactionPolicy, RedactionReason,
};
use continuum_context::dependence::{
    CorrespondenceClaim, CorrespondenceRef, DependenceJoin, ExecutionDependence, Inadmissible,
    SourceCorrespondence,
};
use continuum_context::expansion::{ExpansionQuery, ExpansionRelation};
use continuum_context::guarantee::{Guarantee, GuaranteeSet, RequestedGuarantees};
use continuum_context::model::ModelActionRef;
use continuum_context::monitor::{
    Inapplicable, MonitorDisagreement, MonitorVerdict, PropertyMonitor,
};
use continuum_context::omission::OmissionReason;
use continuum_context::property::{
    AutomatonState, Coverage, CoverageGap, PropertyAutomaton, PropertyFilter,
};
use continuum_context::selection::SelectionKind;
use continuum_context::source::{SourceRef, SourceSpan};
use continuum_context::stage::{Phase, Stage};
use continuum_value::identity::Blake3Hasher;
use continuum_value::value::Name;
use continuum_workspace::snapshot::WorkspacePath;

/// 215 noise candidates around the ten named ones: 225, the dx01 campaign's own trace length.
const NOISE: usize = 215;

/// The failure's causal chain.
const CORE: [&str; 4] = ["e_begin", "e_submit", "e_ack", "e_loss"];

/// The property-directed core the pipeline should reach: the chain plus the hidden event the
/// abstraction depends on and no observer publishes.
const PROPERTY_CORE: [&str; 5] = ["e_begin", "e_submit", "e_ack", "e_loss", "e_flush"];

fn name(text: &str) -> Name {
    Name::new(text).expect("a canonical identifier")
}

fn noise_id(index: usize) -> Name {
    name(&format!("n_{index:04}"))
}

fn ids(names: &[&str]) -> BTreeSet<Name> {
    names.iter().map(|id| name(id)).collect()
}

fn state(text: &str) -> AutomatonState {
    AutomatonState::new(name(text))
}

/// The fixture. Ten named candidates and 215 causally isolated noise candidates:
///
/// - `e_begin -> e_submit -> e_ack -> e_loss` — the durability failure's chain;
/// - `e_flush` — causally isolated, **not** in the automaton's alphabet, and declared an
///   abstraction-relevant hidden dependency. A filter written against the alphabet alone drops
///   it and stays causally closed, which is why it is the mutant this fixture exists for;
/// - `e_probe -> e_report` — a diagnostic chain the question names as a root and the property
///   is not directed at, so stage 3 has something real to narrow;
/// - `s_writer` (source), `m_commit` (model), `s_ghost` (source) — stage 4's three cases.
fn durability_order() -> CausalOrder {
    let mut nodes: Vec<(Name, SelectionKind)> = CORE
        .iter()
        .chain(["e_flush", "e_probe", "e_report"].iter())
        .map(|id| (name(id), SelectionKind::Event))
        .collect();
    nodes.push((name("s_writer"), SelectionKind::Source));
    nodes.push((name("s_ghost"), SelectionKind::Source));
    nodes.push((name("m_commit"), SelectionKind::Model));
    for index in 0..NOISE {
        // Every fourth noise candidate is a `state_delta`, so the manifest partitions over
        // more than one kind and the counting equation is checked across cells.
        let kind = if index % 4 == 0 {
            SelectionKind::StateDelta
        } else {
            SelectionKind::Event
        };
        nodes.push((noise_id(index), kind));
    }
    CausalOrder::new(
        nodes,
        [
            (name("e_submit"), name("e_begin")),
            (name("e_ack"), name("e_submit")),
            (name("e_loss"), name("e_ack")),
            (name("e_report"), name("e_probe")),
        ],
    )
    .expect("a well-formed causal order")
}

/// `q_0 -e_submit-> q_1 -e_ack-> q_2 -e_loss-> q_bad`, with `e_flush` declared hidden.
fn automaton(coverage: Coverage) -> PropertyAutomaton {
    PropertyAutomaton::new(
        state("q_0"),
        [
            (state("q_0"), name("e_submit"), state("q_1")),
            (state("q_1"), name("e_ack"), state("q_2")),
            (state("q_2"), name("e_loss"), state("q_bad")),
        ],
        [state("q_bad")],
        [name("e_flush")],
        coverage,
    )
    .expect("a well-formed property automaton")
}

fn source_ref(file: &str, line: u32) -> CorrespondenceRef {
    CorrespondenceRef::Source(SourceRef::new(
        SourceSpan::new(
            WorkspacePath::new(file).expect("a well-formed workspace path"),
            line,
            1,
            line,
            24,
        )
        .expect("a well-ordered span"),
    ))
}

/// Stage 4's three cases: corroborated and published, claimed-only-by-source, and
/// corroborated but anchored at something stage 3 dropped.
fn join() -> DependenceJoin {
    DependenceJoin::new(
        SourceCorrespondence::of([
            (
                name("s_writer"),
                CorrespondenceClaim::new(
                    source_ref("crates/store/src/writer.rs", 118),
                    [name("e_ack")],
                ),
            ),
            (
                name("m_commit"),
                CorrespondenceClaim::new(
                    CorrespondenceRef::Model(ModelActionRef::action_only(name("Commit"))),
                    [name("e_submit")],
                ),
            ),
            (
                name("s_ghost"),
                CorrespondenceClaim::new(
                    source_ref("crates/store/src/probe.rs", 7),
                    [name("e_probe")],
                ),
            ),
        ])
        .expect("a well-formed source correspondence"),
        ExecutionDependence::attesting([
            (name("s_writer"), ids(&["e_ack"])),
            (name("s_ghost"), ids(&["e_probe"])),
        ])
        .expect("a well-formed execution record"),
    )
}

fn residual() -> ExpansionQuery {
    ExpansionQuery::new(ExpansionRelation::CausalPredecessors, name("e_loss"))
}

fn roots() -> BTreeSet<Name> {
    ids(&["e_loss", "e_flush", "e_report"])
}

/// The whole pipeline: stages 1–4, a total automaton coverage, a complete causal order, and a
/// monitor over the same property the filter is directed by.
fn pipeline() -> CausalCompile {
    CausalCompile::new(durability_order(), RedactionPolicy::permitting_everything())
        .with_property(
            automaton(Coverage::Total),
            PropertyMonitor::over(automaton(Coverage::Total)),
        )
        .with_dependence(join())
}

/// The manifest as `(kind, reason) -> count`; every expandable record in this fixture carries
/// the one residual query, so the pair is a key.
fn cells(compiled: &Compilation) -> BTreeMap<(SelectionKind, OmissionReason), u32> {
    compiled
        .accounting()
        .manifest()
        .records()
        .iter()
        .map(|record| ((record.kind(), record.reason()), record.count()))
        .collect()
}

// =====================================================================================
// positive: the row's experiment, run by stages 1–4
// =====================================================================================

#[test]
fn the_noisy_durability_case_compiles_to_a_six_item_property_directed_core() {
    let compiled = pipeline()
        .run(&roots(), &residual())
        .expect("stages 1-4 run");

    assert_eq!(compiled.accounting().candidate_count(), 225);
    // The five-event property core, plus the one source span whose dependence the execution
    // record corroborated. `Name` orders shortlex, so the canonical list is by length first.
    assert_eq!(
        compiled.selected(),
        [
            name("e_ack"),
            name("e_loss"),
            name("e_begin"),
            name("e_flush"),
            name("e_submit"),
            name("s_writer"),
        ]
    );
    // The reduction at candidate-count grain; bytes are root assembly's measurement.
    assert_eq!(225 / compiled.selected().len(), 37);
}

#[test]
fn the_core_is_licensed_property_preserving_by_the_independent_monitor() {
    let compiled = pipeline()
        .run(&roots(), &residual())
        .expect("stages 1-4 run");

    assert_eq!(
        compiled.monitor_verdict(),
        Some(&MonitorVerdict::Preserving)
    );
    assert!(compiled.is_property_preserving());
    assert!(compiled.is_causally_closed());
    assert_eq!(
        compiled.guarantees().members(),
        [Guarantee::PropertyPreserving, Guarantee::CausallyClosed]
    );

    // And the same verdict, reached a second way: hand the published selection back to a
    // monitor built out here, from this file's own automaton, with no compiler involved.
    let order = durability_order();
    let published: BTreeSet<Name> = compiled.selected().iter().cloned().collect();
    assert_eq!(
        PropertyMonitor::over(automaton(Coverage::Total)).verdict(&order, &published),
        MonitorVerdict::Preserving
    );
    assert_eq!(order.closure_violation(&published), None);
}

#[test]
fn every_phase_left_a_retrievable_intermediate_in_pipeline_order() {
    let compiled = pipeline()
        .run(&roots(), &residual())
        .expect("stages 1-4 run");
    let trail = compiled.trail();

    assert_eq!(
        trail.phases(),
        [
            Phase::Redaction,
            Phase::Stage(Stage::RootSelection),
            Phase::Stage(Stage::CausalSlicing),
            Phase::Stage(Stage::PropertyRelevance),
            Phase::Stage(Stage::DependenceJoin),
        ]
    );
    assert_eq!(trail.universe().len(), 225);

    let working = |stage: Stage| {
        trail
            .intermediate(Phase::Stage(stage))
            .unwrap_or_else(|| panic!("{stage} recorded"))
            .working_set()
            .clone()
    };
    assert_eq!(working(Stage::RootSelection), roots());
    assert_eq!(
        working(Stage::CausalSlicing),
        ids(&[
            "e_begin", "e_submit", "e_ack", "e_loss", "e_flush", "e_probe", "e_report",
        ])
    );
    assert_eq!(working(Stage::PropertyRelevance), ids(&PROPERTY_CORE));
    assert_eq!(
        working(Stage::DependenceJoin),
        ids(&[
            "e_begin", "e_submit", "e_ack", "e_loss", "e_flush", "s_writer",
        ])
    );

    // The pipeline is not monotone, and the trail is the place that shows it: 3 -> 7 -> 5 -> 6.
    assert_eq!(
        [
            working(Stage::RootSelection).len(),
            working(Stage::CausalSlicing).len(),
            working(Stage::PropertyRelevance).len(),
            working(Stage::DependenceJoin).len(),
        ],
        [3, 7, 5, 6]
    );
    // A stage that did not run has no intermediate — distinct from an empty one.
    for stage in [
        Stage::ProofSlicing,
        Stage::ObserverProjection,
        Stage::CorrespondenceMapping,
        Stage::MinimalCore,
        Stage::HeuristicRanking,
        Stage::BudgetPacking,
    ] {
        assert!(trail.intermediate(Phase::Stage(stage)).is_none());
    }
}

#[test]
fn the_manifest_accounts_for_every_one_of_the_219_dropped_candidates() {
    let compiled = pipeline()
        .run(&roots(), &residual())
        .expect("stages 1-4 run");
    let manifest = compiled.accounting().manifest();

    assert_eq!(manifest.total(), 219);
    assert_eq!(
        cells(&compiled),
        [
            // 161 noise events, plus `e_probe` and `e_report` which stage 3 proved outside
            // the property-directed slice under a total coverage.
            ((SelectionKind::Event, OmissionReason::SliceIrrelevant), 163),
            (
                (SelectionKind::StateDelta, OmissionReason::SliceIrrelevant),
                54
            ),
            // Stage 4's two declines, each undecided rather than disproved.
            ((SelectionKind::Source, OmissionReason::HeuristicCutoff), 1),
            ((SelectionKind::Model, OmissionReason::HeuristicCutoff), 1),
        ]
        .into()
    );
    for record in manifest.records() {
        assert!(record.retrievability().is_expandable());
        assert_eq!(record.retrievability().query(), Some(&residual()));
    }

    // INV-007's counting equation, recomputed from the two published halves.
    compiled
        .accounting()
        .reconcile()
        .expect("the two halves agree");
    assert_eq!(
        u64::from(compiled.accounting().candidate_count()),
        compiled.accounting().selected().len() as u64 + manifest.total()
    );
}

// =====================================================================================
// the mutants: a filter the monitor must catch, and a claim that must not upgrade
// =====================================================================================

/// A **mutant stage-3 filter**, written here rather than in the crate: it keeps the closure of
/// the events the automaton *steps on*, and nothing else. That is precisely the compiler RFC
/// 0028 prohibits — "A compiler MUST NOT exclude an abstraction-relevant hidden event on the
/// grounds that no observer publishes it" — and its output is causally closed, so stage 2's
/// checker licenses it without complaint.
fn observed_only_filter(
    automaton: &PropertyAutomaton,
    order: &CausalOrder,
    slice: &BTreeSet<Name>,
) -> BTreeSet<Name> {
    let seeds: BTreeSet<Name> = slice
        .iter()
        .filter(|id| automaton.observes(id))
        .cloned()
        .collect();
    order
        .backward_closure(&seeds)
        .expect("the slice is inside the order")
        .intersection(slice)
        .cloned()
        .collect()
}

#[test]
fn a_filter_that_drops_an_abstraction_relevant_hidden_event_is_rejected_by_the_monitor() {
    let order = durability_order();
    let automaton = automaton(Coverage::Total);
    let stage_two = ids(&[
        "e_begin", "e_submit", "e_ack", "e_loss", "e_flush", "e_probe", "e_report",
    ]);

    let honest = PropertyFilter::retain(&automaton, &order, &stage_two).expect("inside the order");
    let mutant = observed_only_filter(&automaton, &order, &stage_two);

    // The two filters differ by exactly the hidden event.
    assert_eq!(honest, ids(&PROPERTY_CORE));
    assert_eq!(mutant, ids(&CORE));
    assert_eq!(
        honest.difference(&mutant).cloned().collect::<BTreeSet<_>>(),
        ids(&["e_flush"])
    );

    // The closure checker licenses the mutant — it is downward closed — and the property
    // monitor refuses it. Two guarantees, two checkers, and neither implies the other.
    assert!(order.is_downward_closed(&mutant));
    assert_eq!(order.closure_violation(&mutant), None);
    let monitor = PropertyMonitor::over(automaton);
    assert_eq!(
        monitor.verdict(&order, &mutant),
        MonitorVerdict::Divergent(MonitorDisagreement::AbstractionDependencyDropped {
            id: name("e_flush"),
        })
    );
    // Anti-vacuity: the honest filter's output passes the same monitor.
    assert_eq!(monitor.verdict(&order, &honest), MonitorVerdict::Preserving);
}

#[test]
fn a_filter_that_drops_an_observed_event_is_rejected_by_the_monitor() {
    let order = durability_order();
    let monitor = PropertyMonitor::over(automaton(Coverage::Total));

    // A closed *prefix* of the property core: dropping `e_ack` and everything after it leaves
    // a set the closure checker licenses and the property no longer decides.
    let truncated = ids(&["e_begin", "e_submit", "e_flush"]);
    assert_eq!(order.closure_violation(&truncated), None);
    assert_eq!(
        monitor.verdict(&order, &truncated),
        MonitorVerdict::Divergent(MonitorDisagreement::ObservedEventDropped { id: name("e_ack") })
    );

    // And each single-event drop from the property core is caught at its own arm.
    for (dropped, expected) in [
        (
            "e_flush",
            MonitorDisagreement::AbstractionDependencyDropped {
                id: name("e_flush"),
            },
        ),
        (
            "e_begin",
            MonitorDisagreement::NotCausallyClosed(ClosureViolation::MissingPredecessor {
                selected: name("e_submit"),
                predecessor: name("e_begin"),
            }),
        ),
        (
            "e_loss",
            MonitorDisagreement::ObservedEventDropped { id: name("e_loss") },
        ),
    ] {
        let mut mutant = ids(&PROPERTY_CORE);
        mutant.remove(&name(dropped));
        assert_eq!(
            monitor.verdict(&order, &mutant),
            MonitorVerdict::Divergent(expected),
            "dropping `{dropped}` was not caught"
        );
    }
}

#[test]
fn a_monitor_stricter_than_the_filter_refuses_the_licence() {
    // The filter is directed by one automaton and the licence is issued by a monitor over
    // another. Here the monitor also observes the diagnostic chain stage 3 dropped, so it
    // disagrees with a filter that did exactly what it was told.
    let stricter = PropertyAutomaton::new(
        state("q_0"),
        [
            (state("q_0"), name("e_submit"), state("q_1")),
            (state("q_1"), name("e_ack"), state("q_2")),
            (state("q_2"), name("e_loss"), state("q_bad")),
            (state("q_bad"), name("e_report"), state("q_bad")),
        ],
        [state("q_bad")],
        [name("e_flush")],
        Coverage::Total,
    )
    .expect("a well-formed property automaton");

    let compiled = CausalCompile::new(durability_order(), RedactionPolicy::permitting_everything())
        .with_property(automaton(Coverage::Total), PropertyMonitor::over(stricter))
        .with_dependence(join())
        .run(&roots(), &residual())
        .expect("stages 1-4 run");

    assert!(!compiled.is_property_preserving());
    assert!(
        !compiled
            .guarantees()
            .members()
            .contains(&Guarantee::PropertyPreserving)
    );
    assert_eq!(
        compiled.monitor_verdict(),
        Some(&MonitorVerdict::Divergent(
            MonitorDisagreement::ObservedEventDropped {
                id: name("e_report"),
            }
        ))
    );
    // The guarantee is dropped; nothing else is. The pack is the same honest pack.
    assert!(compiled.is_causally_closed());
    assert_eq!(compiled.accounting().manifest().total(), 219);
    compiled
        .accounting()
        .reconcile()
        .expect("the two halves agree");
}

/// The INV-016 mutant: a dependence asserted only by the untrusted source correspondence.
#[test]
fn an_untrusted_source_only_dependence_cannot_upgrade() {
    let compiled = pipeline()
        .run(&roots(), &residual())
        .expect("stages 1-4 run");

    // `m_commit`'s claimed anchor `e_submit` *is* published, so nothing about the slice saved
    // it: the refusal is entirely about who attested the dependence.
    assert!(compiled.selected().contains(&name("e_submit")));
    assert!(!compiled.selected().contains(&name("m_commit")));
    assert_eq!(
        compiled.declined().get(&name("m_commit")),
        Some(&Inadmissible::UncorroboratedSource)
    );
    assert!(!compiled.admissible().contains_key(&name("m_commit")));
    // And it is named and counted rather than hidden — undecided, never disproved.
    assert_eq!(
        cells(&compiled)
            .get(&(SelectionKind::Model, OmissionReason::HeuristicCutoff))
            .copied(),
        Some(1)
    );

    // Anti-vacuity, and the shape of the rule: the only way to admit it is to construct an
    // `ExecutionDependence`, an explicit act of attestation with no conversion from the
    // untrusted side. With the *same* claim so attested, the item is admitted.
    let attested = DependenceJoin::new(
        SourceCorrespondence::of([(
            name("m_commit"),
            CorrespondenceClaim::new(
                CorrespondenceRef::Model(ModelActionRef::action_only(name("Commit"))),
                [name("e_submit")],
            ),
        )])
        .expect("a well-formed source correspondence"),
        ExecutionDependence::attesting([(name("m_commit"), ids(&["e_submit"]))])
            .expect("a well-formed execution record"),
    );
    let upgraded = CausalCompile::new(durability_order(), RedactionPolicy::permitting_everything())
        .with_property(
            automaton(Coverage::Total),
            PropertyMonitor::over(automaton(Coverage::Total)),
        )
        .with_dependence(attested)
        .run(&roots(), &residual())
        .expect("stages 1-4 run");
    assert!(upgraded.selected().contains(&name("m_commit")));
    assert!(upgraded.declined().is_empty());
    assert_eq!(
        upgraded
            .admissible()
            .get(&name("m_commit"))
            .map(continuum_context::dependence::Admissible::kind),
        Some(SelectionKind::Model)
    );
}

#[test]
fn a_corroborated_dependence_anchored_outside_the_core_is_not_admitted() {
    let compiled = pipeline()
        .run(&roots(), &residual())
        .expect("stages 1-4 run");

    // `s_ghost` is attested by the execution record, and its anchor `e_probe` is exactly what
    // stage 3 dropped, so it has nothing published to attach to.
    assert!(!compiled.selected().contains(&name("e_probe")));
    assert_eq!(
        compiled.declined().get(&name("s_ghost")),
        Some(&Inadmissible::AnchorNotSelected)
    );
    assert_eq!(
        cells(&compiled)
            .get(&(SelectionKind::Source, OmissionReason::HeuristicCutoff))
            .copied(),
        Some(1)
    );
}

#[test]
fn a_fabricated_anchor_is_ignored_rather_than_obeyed() {
    // Untrusted input naming a candidate that does not exist is data, not a command: it fails
    // to corroborate, it does not fail the compile, and it cannot add a candidate to the pack.
    let fabricating = DependenceJoin::new(
        SourceCorrespondence::of([(
            name("s_writer"),
            CorrespondenceClaim::new(
                source_ref("crates/store/src/writer.rs", 118),
                [name("e_nowhere"), name("n_9999")],
            ),
        )])
        .expect("a well-formed source correspondence"),
        ExecutionDependence::attesting([(name("s_writer"), ids(&["e_nowhere"]))])
            .expect("a well-formed execution record"),
    );
    let compiled = CausalCompile::new(durability_order(), RedactionPolicy::permitting_everything())
        .with_property(
            automaton(Coverage::Total),
            PropertyMonitor::over(automaton(Coverage::Total)),
        )
        .with_dependence(fabricating)
        .run(&roots(), &residual())
        .expect("stages 1-4 run");

    assert_eq!(compiled.accounting().candidate_count(), 225);
    assert!(!compiled.selected().contains(&name("s_writer")));
    assert_eq!(
        compiled.declined().get(&name("s_writer")),
        Some(&Inadmissible::AnchorNotSelected)
    );
    compiled
        .accounting()
        .reconcile()
        .expect("the two halves agree");
}

// =====================================================================================
// negative: each refusal, and its own arm
// =====================================================================================

#[test]
fn a_join_claim_about_a_candidate_of_another_kind_is_refused() {
    let miscast = DependenceJoin::new(
        SourceCorrespondence::of([(
            name("e_ack"),
            CorrespondenceClaim::new(
                source_ref("crates/store/src/writer.rs", 1),
                [name("e_loss")],
            ),
        )])
        .expect("a well-formed source correspondence"),
        ExecutionDependence::none(),
    );
    assert_eq!(
        CausalCompile::new(durability_order(), RedactionPolicy::permitting_everything())
            .with_dependence(miscast)
            .run(&roots(), &residual()),
        Err(CompileError::NotACorrespondenceKind {
            id: name("e_ack"),
            kind: SelectionKind::Event,
        })
    );
}

#[test]
fn a_redaction_hole_costs_both_guarantees_and_neither_the_honesty() {
    let compiled = CausalCompile::new(
        durability_order(),
        RedactionPolicy::withholding([(name("e_submit"), RedactionReason::Purged)]),
    )
    .with_property(
        automaton(Coverage::Total),
        PropertyMonitor::over(automaton(Coverage::Total)),
    )
    .with_dependence(join())
    .run(&roots(), &residual())
    .expect("stages 1-4 run");

    assert_eq!(compiled.redaction_gap(), &ids(&["e_submit"]));
    assert!(!compiled.is_causally_closed());
    assert!(!compiled.is_property_preserving());
    assert!(compiled.guarantees().members().is_empty());
    assert_eq!(
        compiled.monitor_verdict(),
        Some(&MonitorVerdict::Divergent(
            MonitorDisagreement::ObservedEventDropped {
                id: name("e_submit"),
            }
        ))
    );
    // The hole is named, counted, and marked irretrievable.
    let redacted: Vec<_> = compiled
        .accounting()
        .manifest()
        .records()
        .iter()
        .filter(|record| record.reason() == OmissionReason::Redaction)
        .collect();
    assert_eq!(redacted.len(), 1);
    assert_eq!(redacted[0].count(), 1);
    assert!(!redacted[0].retrievability().is_expandable());
    compiled
        .accounting()
        .reconcile()
        .expect("the two halves still agree");
}

/// RFC 0028's own distinction, decidable here for the first time: two compiles over one order,
/// differing only in what the automaton *declares* about its own completeness.
#[test]
fn an_undecidable_drop_is_heuristic_cutoff_and_never_slice_irrelevant() {
    let total = pipeline()
        .run(&roots(), &residual())
        .expect("stages 1-4 run");
    let partial = CausalCompile::new(durability_order(), RedactionPolicy::permitting_everything())
        .with_property(
            automaton(Coverage::Partial {
                reason: CoverageGap::UnenumeratedAbstraction,
            }),
            PropertyMonitor::over(automaton(Coverage::Total)),
        )
        .with_dependence(join())
        .run(&roots(), &residual())
        .expect("stages 1-4 run");

    // Same selection, same guarantees: coverage says what a *drop* proves, not what is kept.
    assert_eq!(total.selected(), partial.selected());
    assert_eq!(total.guarantees().members(), partial.guarantees().members());

    // The two stage-3 drops move out of the proved cell and into the undecided one; the 215
    // stage-2 drops keep their proof, because the *causal order* is still complete.
    assert_eq!(
        cells(&partial),
        [
            ((SelectionKind::Event, OmissionReason::HeuristicCutoff), 2),
            ((SelectionKind::Event, OmissionReason::SliceIrrelevant), 161),
            (
                (SelectionKind::StateDelta, OmissionReason::SliceIrrelevant),
                54
            ),
            ((SelectionKind::Source, OmissionReason::HeuristicCutoff), 1),
            ((SelectionKind::Model, OmissionReason::HeuristicCutoff), 1),
        ]
        .into()
    );
    assert_eq!(partial.accounting().manifest().total(), 219);
    partial
        .accounting()
        .reconcile()
        .expect("the two halves agree");
}

// =====================================================================================
// boundary
// =====================================================================================

#[test]
fn an_execution_the_automaton_does_not_observe_licenses_nothing_and_says_why() {
    // The bone's boundary: a slice containing no automaton-observed event at all.
    // `PropertyPreserving` is not claimed, and the reason is typed rather than absent.
    let elsewhere = PropertyAutomaton::new(
        state("q_0"),
        [(state("q_0"), name("e_elsewhere"), state("q_1"))],
        [state("q_1")],
        [],
        Coverage::Total,
    )
    .expect("a well-formed property automaton");
    let compiled = CausalCompile::new(durability_order(), RedactionPolicy::permitting_everything())
        .with_property(automaton(Coverage::Total), PropertyMonitor::over(elsewhere))
        .with_dependence(join())
        .run(&roots(), &residual())
        .expect("stages 1-4 run");

    assert!(!compiled.is_property_preserving());
    assert_eq!(
        compiled.monitor_verdict(),
        Some(&MonitorVerdict::Inapplicable(Inapplicable::NoObservedEvent))
    );
    // Distinct from a monitor that observes nothing anywhere.
    let silent = PropertyAutomaton::new(state("q_0"), [], [], [], Coverage::Total)
        .expect("a well-formed property automaton");
    assert_eq!(
        PropertyMonitor::over(silent).verdict(&durability_order(), &ids(&PROPERTY_CORE)),
        MonitorVerdict::Inapplicable(Inapplicable::EmptyAlphabet)
    );
}

#[test]
fn a_property_filter_that_empties_the_slice_cannot_name_where_to_get_anything_back() {
    // Roots the property is not directed at: stage 3 keeps nothing, and a pack that published
    // nothing cannot anchor the query that retrieves what it dropped.
    assert_eq!(
        pipeline().run(&ids(&["e_report"]), &residual()),
        Err(CompileError::AnchorNotSelected {
            anchor: name("e_loss"),
        })
    );
}

#[test]
fn a_slice_that_is_entirely_property_relevant_is_not_narrowed() {
    let compiled = pipeline()
        .run(&ids(&["e_loss", "e_flush"]), &residual())
        .expect("stages 1-4 run");
    let trail = compiled.trail();
    assert_eq!(
        trail
            .intermediate(Phase::Stage(Stage::CausalSlicing))
            .expect("stage 2")
            .working_set(),
        trail
            .intermediate(Phase::Stage(Stage::PropertyRelevance))
            .expect("stage 3")
            .working_set()
    );
    assert_eq!(
        trail
            .intermediate(Phase::Stage(Stage::PropertyRelevance))
            .expect("stage 3")
            .note(),
        Some("property-retained")
    );
    assert!(compiled.is_property_preserving());
}

#[test]
fn a_join_that_admits_nothing_records_that_it_ran() {
    let compiled = CausalCompile::new(durability_order(), RedactionPolicy::permitting_everything())
        .with_property(
            automaton(Coverage::Total),
            PropertyMonitor::over(automaton(Coverage::Total)),
        )
        .with_dependence(DependenceJoin::default())
        .run(&roots(), &residual())
        .expect("stages 1-4 run");

    let stage_four = compiled
        .trail()
        .intermediate(Phase::Stage(Stage::DependenceJoin))
        .expect("stage 4 recorded");
    assert_eq!(stage_four.note(), Some("no-admissible-correspondence"));
    assert_eq!(stage_four.working_set(), &ids(&PROPERTY_CORE));
    assert!(compiled.admissible().is_empty());
    assert!(compiled.declined().is_empty());
    // A stage that ran and admitted nothing is not the same fact as a stage that did not run.
    assert_eq!(compiled.selected().len(), 5);
}

// =====================================================================================
// the disciplines the crate boundary and the module boundary carry
// =====================================================================================

/// `monitor.rs`'s own bytes, so the independence claim is checked rather than asserted.
const MONITOR_SOURCE: &str = include_str!("../src/monitor.rs");

#[test]
fn the_monitor_module_imports_only_the_premise_the_filter_also_reads() {
    // RFC 0028: "an implementation that reuses the compiler's own automaton has checked
    // nothing". The checker may share the *property* — both sides must be talking about the
    // same one, exactly as stage 2's slicer and checker share a `CausalOrder` — and it may not
    // reach the filter or the compiler. This is the mechanical half of that claim: the
    // production half of `monitor.rs` names `causal` and `property`'s automaton, and nothing
    // else of this crate's. (Its own test module does use `PropertyFilter`, to build an honest
    // slice to check; a test is not the checker.)
    let production = MONITOR_SOURCE
        .split("#[cfg(test)]")
        .next()
        .expect("the module has a non-test half");
    let imports: Vec<&str> = production
        .lines()
        .map(str::trim)
        .filter(|line| line.starts_with("use crate::"))
        .collect();
    assert_eq!(
        imports,
        [
            "use crate::causal::{CausalOrder, ClosureViolation};",
            "use crate::property::{AutomatonVerdict, PropertyAutomaton};",
        ]
    );
    assert!(!production.contains("PropertyFilter::"));
    assert!(!production.contains("use crate::compile"));
}

#[test]
fn a_request_for_property_preservation_is_not_a_claim_across_the_crate_boundary() {
    let requested = RequestedGuarantees::of([
        Guarantee::PropertyPreserving,
        Guarantee::ProofRelevant,
        Guarantee::OneMinimal,
    ]);
    // A compile whose monitor disagrees: three requested, none of them this one achieved.
    let stricter = PropertyAutomaton::new(
        state("q_0"),
        [
            (state("q_0"), name("e_submit"), state("q_1")),
            (state("q_1"), name("e_report"), state("q_bad")),
        ],
        [state("q_bad")],
        [],
        Coverage::Total,
    )
    .expect("a well-formed property automaton");
    let compiled = CausalCompile::new(durability_order(), RedactionPolicy::permitting_everything())
        .with_property(automaton(Coverage::Total), PropertyMonitor::over(stricter))
        .with_dependence(join())
        .run(&roots(), &residual())
        .expect("stages 1-4 run");

    assert_eq!(compiled.guarantees().members(), [Guarantee::CausallyClosed]);
    assert_eq!(
        requested.unachieved(&GuaranteeSet::empty()),
        [
            Guarantee::PropertyPreserving,
            Guarantee::ProofRelevant,
            Guarantee::OneMinimal,
        ]
    );
}

#[test]
fn an_admitted_source_item_is_a_position_and_never_a_snippet() {
    // INV-016's other half. The item's summary is exactly the span's rendering, and the only
    // constructors that can produce it take no string parameter at all.
    let compiled = pipeline()
        .run(&roots(), &residual())
        .expect("stages 1-4 run");
    let items = compiled.admissible_items();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].id(), &name("s_writer"));
    assert_eq!(items[0].kind(), SelectionKind::Source);
    assert_eq!(
        items[0].summary(),
        "crates/store/src/writer.rs:118:1-118:24"
    );
    assert_eq!(items[0].artifact(), None);
}

#[test]
fn the_whole_four_stage_compile_is_deterministic() {
    let one = pipeline()
        .run(&roots(), &residual())
        .expect("stages 1-4 run");
    let other = pipeline()
        .run(&roots(), &residual())
        .expect("stages 1-4 run");

    assert_eq!(one, other);
    assert_eq!(
        one.trail().digest::<Blake3Hasher>(),
        other.trail().digest::<Blake3Hasher>()
    );
    assert_eq!(
        one.trail().to_json().to_canonical_bytes(),
        other.trail().to_json().to_canonical_bytes()
    );
    // Not vacuous: dropping stage 3 is a different trail, and a different digest.
    let without_stage_three =
        CausalCompile::new(durability_order(), RedactionPolicy::permitting_everything())
            .with_dependence(join())
            .run(&roots(), &residual())
            .expect("stages 1-2 and 4 run");
    assert_ne!(
        one.trail().digest::<Blake3Hasher>(),
        without_stage_three.trail().digest::<Blake3Hasher>()
    );
}
