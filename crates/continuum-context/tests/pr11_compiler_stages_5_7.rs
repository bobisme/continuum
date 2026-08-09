//! PR-11 / the Context Pack compiler, **stage group 3** (bn-imhw2): stage 5 (proof-dependency
//! slicing), stage 6 (observer projection) and stage 7 (abstraction/refinement correspondence
//! mapping), exercised from outside the crate at the grain the G0-DX-01 row's experiment uses.
//!
//! # What this group is, stated before any number
//!
//! Two of its three stages **refuse**, and that is the delivery rather than a shortfall.
//!
//! | Stage | Input RFC 0028 names | Producer plan §20 names | Its state in this workspace |
//! |---|---|---|---|
//! | 5 | the named obligations' proof dependency slice | `continuum-proof-client` (RFC 0035, RFC 0012) | PR-1 / IMPL-01 scaffold, no public item |
//! | 6 | the intent's observers | — the intent's own field group | **landed**: `continuum_intent::observers` |
//! | 7 | the plan §16 correspondence graph | `continuum-refinement` | PR-1 / IMPL-01 scaffold, no public item |
//!
//! RFC 0026's `rule errors.unsupported_surface` decides what stages 5 and 7 do about their
//! absent producers — "MUST fail with the typed `UnsupportedSemanticFeature` rather than
//! degrading, guessing, or returning an empty success" — and INV-008 decides what the record
//! must keep apart. So this file's load-bearing tests are:
//!
//! 1. the refusals happen, are typed, name the absent producer, cost their guarantee, record an
//!    `unsupported` irretrievable omission where they decide a candidate kind, and hand the pack
//!    a typed `inconclusive` reason so it cannot default to `satisfied`;
//! 2. the **anti-vacuity mutant** RFC 0028's own guarantee-violation list points at this stage —
//!    "a stage that quietly licenses `ProofRelevant` with no proof slice behind it" — is not
//!    merely absent but *unreachable*: every `License::issue` call site in the crate's
//!    production source is held to a recorded inventory, and the same scan over a mutant with
//!    the line spliced in fails;
//! 3. **freshness tripwires** that go red the moment either producer lands, so a refusal cannot
//!    outlive the workspace fact that justifies it. This is the INV-015 device
//!    (`crates/continuumd/tests/inv015_agent_least_authority_evidence.rs`,
//!    `boundary_the_host_effect_surface_has_not_arrived_and_its_crates_are_scaffolds`) applied
//!    to the compiler's two unbuilt inputs.
//!
//! Stage 6 is different and is implemented for real, because RFC 0028 names its input as "the
//! intent's observers" and those are landed: `continuum_intent::observers::{ObserverSet,
//! Observer, ObserverId, ProjectionKind}`, in a crate `continuum-context` already depends on.
//! This file builds stage 6's premise out of exactly those types, so the evidence that the
//! input exists is that the file compiles.
//!
//! # Honest scope
//!
//! Nothing here claims `ReplayPreserving` (the replay checker is not this crate's),
//! `ProofRelevant` (stage 5's — refused, and unreachable), or any minimality class (stage 8's).
//! The two guarantees this pipeline issues are still `CausallyClosed` and `PropertyPreserving`,
//! and **stage 6 adds no third**: RFC 0028's Licenses column reads "scoping under INV-013;
//! never a guarantee by itself", and `stage_six_licenses_nothing_even_when_it_narrows` pins it
//! by comparing the guarantee set of a compile with and without the stage while the stage
//! visibly narrows the pack. Reductions reported below are at *candidate count*, not bytes.
//!
//! # House rules
//!
//! Deterministic (INV-005): no clock, no entropy, no float; every set is a `BTree` and the whole
//! fixture is a pure function of its own constants.

use std::collections::{BTreeMap, BTreeSet};

use continuum_context::causal::{CausalError, CausalOrder};
use continuum_context::compile::{CausalCompile, CompileError, RedactionPolicy, RedactionReason};
use continuum_context::correspondence::CorrespondenceMapping;
use continuum_context::dependence::{
    CorrespondenceClaim, CorrespondenceRef, DependenceJoin, ExecutionDependence, Inadmissible,
    SourceCorrespondence,
};
use continuum_context::expansion::{ExpansionQuery, ExpansionRelation};
use continuum_context::guarantee::{Guarantee, RequestedGuarantees};
use continuum_context::model::ModelActionRef;
use continuum_context::monitor::PropertyMonitor;
use continuum_context::observer::{
    Observation, ObserverFilter, ObserverProjection, ProjectionApplicability, ProjectionError,
    ProjectionInapplicable,
};
use continuum_context::omission::OmissionReason;
use continuum_context::proof::ProofSlicing;
use continuum_context::property::{AutomatonState, Coverage, PropertyAutomaton};
use continuum_context::scope::{ScopeAudit, ScopeReduction, ScopeVerdict, ScopeViolation};
use continuum_context::selection::SelectionKind;
use continuum_context::source::{SourceRef, SourceSpan};
use continuum_context::stage::{Phase, Stage};
use continuum_context::unsupported::{
    AbsentProducer, StageRefusal, SurfaceAbsence, UnsupportedSurface,
};
use continuum_intent::observers::{Observer, ObserverId, ObserverSet, ProjectionKind};
use continuum_value::assurance::InconclusiveReason;
use continuum_value::value::Name;
use continuum_workspace::snapshot::WorkspacePath;

/// 214 noise candidates around the eleven named ones: 225, the dx01 campaign's own trace length.
const NOISE: usize = 214;

/// The failure's causal chain.
const CORE: [&str; 4] = ["e_begin", "e_submit", "e_ack", "e_loss"];

/// The property-directed core: the chain plus the hidden event the abstraction depends on and
/// no observer publishes.
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

/// The fixture. Eleven named candidates and 214 causally isolated noise candidates:
///
/// - `e_begin -> e_submit -> e_ack -> e_loss` — the durability failure's chain;
/// - `e_flush` — causally isolated, **not** in the automaton's alphabet, declared an
///   abstraction-relevant hidden dependency, and published by no observer. It is the candidate
///   RFC 0028's "MUST NOT exclude an abstraction-relevant hidden event on the grounds that no
///   observer publishes it" is about, and stage 6 is the only stage that could commit that
///   error;
/// - `e_probe -> e_report` — a diagnostic chain the question names as a root, which no observer
///   publishes and the property is not directed at;
/// - `s_writer` (source) and `m_commit` (model) — stage 4's corroborated and uncorroborated
///   cases, carried forward so this group runs on top of a real stage 4;
/// - `p_durable`, `p_ack` (**proof**) — stage 5's column, and the only kind it decides.
fn durability_order() -> CausalOrder {
    let mut nodes: Vec<(Name, SelectionKind)> = CORE
        .iter()
        .chain(["e_flush", "e_probe", "e_report"].iter())
        .map(|id| (name(id), SelectionKind::Event))
        .collect();
    nodes.push((name("s_writer"), SelectionKind::Source));
    nodes.push((name("m_commit"), SelectionKind::Model));
    nodes.push((name("p_durable"), SelectionKind::Proof));
    nodes.push((name("p_ack"), SelectionKind::Proof));
    for index in 0..NOISE {
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

fn automaton() -> PropertyAutomaton {
    PropertyAutomaton::new(
        state("q_0"),
        [
            (state("q_0"), name("e_submit"), state("q_1")),
            (state("q_1"), name("e_ack"), state("q_2")),
            (state("q_2"), name("e_loss"), state("q_bad")),
        ],
        [state("q_bad")],
        [name("e_flush")],
        Coverage::Total,
    )
    .expect("a well-formed property automaton")
}

/// `s_writer` is corroborated on `e_ack`; `m_commit` is claimed on `e_submit` and attested
/// nowhere (INV-016: an untrusted claim alone admits nothing).
fn join() -> DependenceJoin {
    DependenceJoin::new(
        SourceCorrespondence::of([
            (
                name("s_writer"),
                CorrespondenceClaim::new(
                    CorrespondenceRef::Source(SourceRef::new(
                        SourceSpan::new(
                            WorkspacePath::new("crates/store/src/writer.rs").expect("well formed"),
                            42,
                            5,
                            42,
                            31,
                        )
                        .expect("well ordered"),
                    )),
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
        ])
        .expect("a well-formed source correspondence"),
        ExecutionDependence::attesting([(name("s_writer"), ids(&["e_ack"]))])
            .expect("a well-formed execution record"),
    )
}

fn roots() -> BTreeSet<Name> {
    ids(&["e_loss", "e_flush", "e_probe"])
}

fn residual() -> ExpansionQuery {
    ExpansionQuery::new(ExpansionRelation::CausalPredecessors, name("e_loss"))
}

// --- stage 6's premise, built out of the intent contract's own observer types ---------------

fn an_observer(id: &str, events: &[&str]) -> Observer {
    Observer::new(
        ObserverId::new(id).expect("a non-empty unit key"),
        [(
            ProjectionKind::Events,
            events.iter().map(|event| (*event).to_owned()).collect(),
        )],
    )
    .expect("a well-formed observer")
}

fn observer_set(members: impl IntoIterator<Item = Observer>) -> ObserverSet {
    ObserverSet::from_observers(members).expect("a well-formed observer set")
}

fn seen(element: &str) -> Observation {
    Observation::new(ProjectionKind::Events, element)
}

/// The deployment's declaration of what each named candidate publishes.
///
/// The chain's three tail events are published as the families `client` names; `e_begin`,
/// `e_flush`, `e_probe` and `e_report` are declared into families no observer names, so each is
/// individually droppable and the reduction has to be decided rather than assumed. The 214
/// noise candidates and the two `source`/`model` candidates are deliberately left
/// **unattributed**, which is how the "no justification, no reduction" rule gets exercised at
/// the row's grain.
fn attribution() -> Vec<(Name, Observation)> {
    vec![
        (name("e_begin"), seen("Begin")),
        (name("e_submit"), seen("Submitted")),
        (name("e_ack"), seen("Acked")),
        (name("e_loss"), seen("Lost")),
        (name("e_flush"), seen("Internal")),
        (name("e_probe"), seen("Internal")),
        (name("e_report"), seen("Internal")),
    ]
}

fn projection() -> ObserverProjection {
    ObserverProjection::new(
        observer_set([an_observer("client", &["Submitted", "Acked", "Lost"])]),
        attribution(),
    )
    .expect("a well-formed observer projection")
}

fn audit() -> ScopeAudit {
    ScopeAudit::over(projection())
}

/// The whole group, on top of stages 1–4.
fn staged() -> CausalCompile {
    CausalCompile::new(durability_order(), RedactionPolicy::permitting_everything())
        .with_property(automaton(), PropertyMonitor::over(automaton()))
        .with_dependence(join())
        .with_proof_slicing(ProofSlicing::over([name("o_durability")]))
        .with_observer_projection(projection(), audit())
        .with_correspondence_mapping(CorrespondenceMapping::of([name("m_commit"), name("e_ack")]))
}

// =====================================================================================
// the whole group, end to end
// =====================================================================================

#[test]
fn the_seven_stage_pipeline_runs_end_to_end_with_two_stages_refusing() {
    let compiled = staged().run(&roots(), &residual()).expect("stages 1-7 run");

    // Every phase that ran recorded, in the RFC's order, with the pre-pass before stage 1.
    assert_eq!(
        compiled.trail().phases(),
        [
            Phase::Redaction,
            Phase::Stage(Stage::RootSelection),
            Phase::Stage(Stage::CausalSlicing),
            Phase::Stage(Stage::PropertyRelevance),
            Phase::Stage(Stage::DependenceJoin),
            Phase::Stage(Stage::ProofSlicing),
            Phase::Stage(Stage::ObserverProjection),
            Phase::Stage(Stage::CorrespondenceMapping),
        ]
    );
    // Stage 8 was not configured, so it has no intermediate — a different fact from an empty
    // one, and the distinction two of the stages above depend on.
    assert!(
        compiled
            .trail()
            .intermediate(Phase::Stage(Stage::MinimalCore))
            .is_none()
    );

    // 225 candidates in; a six-item core out; the guarantees the stages that ran established.
    assert_eq!(compiled.accounting().candidate_count(), 225);
    let mut expected: Vec<Name> = PROPERTY_CORE.iter().map(|id| name(id)).collect();
    expected.push(name("s_writer"));
    expected.sort();
    assert_eq!(compiled.selected(), expected);
    assert_eq!(
        compiled.guarantees().members(),
        [Guarantee::PropertyPreserving, Guarantee::CausallyClosed]
    );
    // And the accounting still closes exactly.
    compiled.accounting().reconcile().expect("halves agree");
    assert_eq!(
        u64::from(compiled.accounting().candidate_count()),
        compiled.accounting().manifest().total() + compiled.selected().len() as u64
    );
}

#[test]
fn the_manifest_names_every_deciding_stage_including_the_one_that_refused() {
    let compiled = staged().run(&roots(), &residual()).expect("stages 1-7 run");
    let cells: BTreeMap<(SelectionKind, OmissionReason), u32> = compiled
        .accounting()
        .manifest()
        .records()
        .iter()
        .map(|record| ((record.kind(), record.reason()), record.count()))
        .collect();
    assert_eq!(
        cells,
        [
            // stage 2 under a complete order, and stage 3 under a total coverage: proofs.
            ((SelectionKind::Event, OmissionReason::SliceIrrelevant), 162),
            (
                (SelectionKind::StateDelta, OmissionReason::SliceIrrelevant),
                54
            ),
            // stage 4's uncorroborated claim: undecided, never disproved.
            ((SelectionKind::Model, OmissionReason::HeuristicCutoff), 1),
            // stage 5's column: the only stage that could have decided it, and it refused.
            ((SelectionKind::Proof, OmissionReason::Unsupported), 2),
        ]
        .into()
    );
    // The `unsupported` cell is the irretrievable one, and it is the only irretrievable cell
    // here: nothing was redacted.
    for record in compiled.accounting().manifest().records() {
        assert_eq!(
            record.retrievability().is_expandable(),
            record.reason() != OmissionReason::Unsupported,
            "{:?}/{:?}",
            record.kind(),
            record.reason()
        );
    }
    assert_eq!(compiled.accounting().manifest().total(), 219);
}

// =====================================================================================
// stage 5 — the refusal
// =====================================================================================

#[test]
fn stage_five_refuses_names_the_absent_proof_service_and_declines_its_guarantee() {
    let compiled = staged().run(&roots(), &residual()).expect("stages 1-7 run");

    let refusal = compiled
        .refusal(UnsupportedSurface::ProofDependencySlice)
        .expect("stage 5 ran and refused");
    assert_eq!(refusal.stage(), Stage::ProofSlicing);
    assert_eq!(
        refusal.absence(),
        SurfaceAbsence::ProducerNotDeployed(AbsentProducer::ProofService)
    );
    assert_eq!(
        refusal.absence().producer().expect("a named producer"),
        AbsentProducer::ProofService
    );
    assert_eq!(refusal.omission_reason(), OmissionReason::Unsupported);
    assert_eq!(
        refusal.inconclusive_reason(),
        InconclusiveReason::IncompleteProofSearch
    );

    // "It MUST claim `ProofRelevant` or state why it cannot" — this is the second branch, and
    // the guarantee is absent from the pack.
    assert!(
        !compiled
            .guarantees()
            .members()
            .contains(&Guarantee::ProofRelevant)
    );
    // The refusal is recorded in the trail with a typed token, never prose.
    assert_eq!(
        compiled
            .trail()
            .intermediate(Phase::Stage(Stage::ProofSlicing))
            .expect("stage 5 recorded")
            .note(),
        Some("proof-slicing-no-producer")
    );
}

#[test]
fn a_requested_proof_relevance_is_not_echoed_back_across_the_crate_boundary() {
    let compiled = staged().run(&roots(), &residual()).expect("stages 1-7 run");
    let requested = RequestedGuarantees::of([
        Guarantee::ProofRelevant,
        Guarantee::PropertyPreserving,
        Guarantee::CausallyClosed,
    ]);
    // RFC 0028 C1: "`context.compile`'s `guarantees` list is a *request*. A daemon MUST NOT
    // echo a requested guarantee it did not achieve." Three requested; two achieved; the third
    // is named as unachieved rather than appearing in the pack.
    let achieved = compiled.guarantees().members();
    let unachieved: Vec<Guarantee> = requested
        .members()
        .into_iter()
        .filter(|guarantee| !achieved.contains(guarantee))
        .collect();
    assert_eq!(unachieved, [Guarantee::ProofRelevant]);
    // The type-level half is `RequestedGuarantees`' own: it has no conversion into a
    // `GuaranteeSet`, and `GuaranteeSet::empty` is the only one an external caller can build —
    // so out here a request cannot become a claim even by mistake.
    assert!(
        requested
            .unachieved(&continuum_context::guarantee::GuaranteeSet::empty())
            .contains(&Guarantee::ProofRelevant)
    );
}

#[test]
fn a_proof_candidate_is_an_unsupported_irretrievable_omission() {
    let compiled = staged().run(&roots(), &residual()).expect("stages 1-7 run");
    let record = compiled
        .accounting()
        .manifest()
        .records()
        .iter()
        .find(|record| record.kind() == SelectionKind::Proof)
        .expect("the two proof candidates are accounted for");
    assert_eq!(record.reason(), OmissionReason::Unsupported);
    assert_eq!(record.count(), 2);
    // `expandable: false`, and the reason explains irretrievability — there is nothing in this
    // deployment for an expansion to reach.
    assert!(!record.retrievability().is_expandable());
    assert!(record.retrievability().query().is_none());

    // Without stage 5 the same two candidates carry the causal order's reason instead. The
    // `unsupported` cell exists *because* a stage ran and refused, not because the kind is
    // special.
    let without = CausalCompile::new(durability_order(), RedactionPolicy::permitting_everything())
        .with_property(automaton(), PropertyMonitor::over(automaton()))
        .with_dependence(join())
        .run(&roots(), &residual())
        .expect("stages 1-4 run");
    assert!(without.refusals().is_empty());
    assert_eq!(without.inconclusive_reason(), None);
    assert!(
        !without
            .accounting()
            .manifest()
            .records()
            .iter()
            .any(|record| record.reason() == OmissionReason::Unsupported)
    );
    // And the two compiles publish the same items: a refusal costs the guarantee, never the
    // pack.
    assert_eq!(compiled.selected(), without.selected());
}

#[test]
fn a_question_that_names_no_obligation_is_a_different_absence_with_the_same_consequences() {
    let compiled = CausalCompile::new(durability_order(), RedactionPolicy::permitting_everything())
        .with_proof_slicing(ProofSlicing::nothing_named())
        .run(&roots(), &residual())
        .expect("stages 1-5 run");
    let refusal = compiled
        .refusal(UnsupportedSurface::ProofDependencySlice)
        .expect("stage 5 ran and refused");

    // INV-008: two distinguishable outcomes, not one `unsupported` word.
    assert_eq!(refusal.absence(), SurfaceAbsence::NothingNamed);
    assert_eq!(refusal.absence().producer(), None);
    assert_eq!(refusal.note(), "proof-slicing-no-obligation");
    assert_ne!(
        refusal,
        StageRefusal::of(
            UnsupportedSurface::ProofDependencySlice,
            SurfaceAbsence::ProducerNotDeployed(AbsentProducer::ProofService)
        )
    );
    // Same consequences: no guarantee, an `unsupported` irretrievable cell, a typed verdict.
    assert!(
        !compiled
            .guarantees()
            .members()
            .contains(&Guarantee::ProofRelevant)
    );
    assert_eq!(
        compiled.inconclusive_reason(),
        Some(InconclusiveReason::IncompleteProofSearch)
    );
    assert!(
        compiled
            .accounting()
            .manifest()
            .records()
            .iter()
            .any(|record| record.kind() == SelectionKind::Proof
                && record.reason() == OmissionReason::Unsupported)
    );
}

// =====================================================================================
// stage 7 — the refusal
// =====================================================================================

#[test]
fn stage_seven_refuses_names_the_absent_correspondence_graph_and_maps_nothing() {
    let compiled = staged().run(&roots(), &residual()).expect("stages 1-7 run");
    let refusal = compiled
        .refusal(UnsupportedSurface::CorrespondenceGraph)
        .expect("stage 7 ran and refused");

    assert_eq!(refusal.stage(), Stage::CorrespondenceMapping);
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
    assert_eq!(
        refusal.inconclusive_reason(),
        InconclusiveReason::AbstractionAmbiguity
    );
    // Every subject asked for is unmapped, including one the pack does publish: an item can be
    // in the pack and still carry no concrete-to-abstract link.
    assert_eq!(
        compiled.unmapped_correspondence(),
        &ids(&["e_ack", "m_commit"])
    );
    assert!(compiled.selected().contains(&name("e_ack")));
    assert_eq!(
        compiled
            .trail()
            .intermediate(Phase::Stage(Stage::CorrespondenceMapping))
            .expect("stage 7 recorded")
            .note(),
        Some("correspondence-no-producer")
    );
}

#[test]
fn stage_seven_invents_no_manifest_cell_because_it_drops_no_candidate() {
    // The manifest partitions *candidates*; a link is not one. Inventing a cell for the missing
    // links would make the counting equation false in the direction that looks like diligence.
    let with = staged().run(&roots(), &residual()).expect("stages 1-7 run");
    let without = CausalCompile::new(durability_order(), RedactionPolicy::permitting_everything())
        .with_property(automaton(), PropertyMonitor::over(automaton()))
        .with_dependence(join())
        .with_proof_slicing(ProofSlicing::over([name("o_durability")]))
        .with_observer_projection(projection(), audit())
        .run(&roots(), &residual())
        .expect("stages 1-6 run");

    assert_eq!(
        with.accounting().manifest(),
        without.accounting().manifest()
    );
    assert_eq!(with.selected(), without.selected());
    assert_eq!(with.guarantees(), without.guarantees());
    // What differs is exactly the recorded refusal and the unmapped subjects.
    assert!(without.unmapped_correspondence().is_empty());
    assert_eq!(with.refusals().len(), 2);
    assert_eq!(without.refusals().len(), 1);
}

#[test]
fn the_earliest_refusing_stage_names_the_verdicts_reason_and_the_later_one_is_still_recorded() {
    let both = staged().run(&roots(), &residual()).expect("stages 1-7 run");
    assert_eq!(both.refusals().len(), 2);
    assert_eq!(
        both.inconclusive_reason(),
        Some(InconclusiveReason::IncompleteProofSearch)
    );
    // Stage 7 alone gives the other reason, so the precedence above is a rule rather than an
    // accident of which stage happens to be configured.
    let seven_only =
        CausalCompile::new(durability_order(), RedactionPolicy::permitting_everything())
            .with_correspondence_mapping(CorrespondenceMapping::of([name("m_commit")]))
            .run(&roots(), &residual())
            .expect("stages 1-2 and 7 run");
    assert_eq!(
        seven_only.inconclusive_reason(),
        Some(InconclusiveReason::AbstractionAmbiguity)
    );
    // Neither reason is the generic `Unsupported`: RFC 0028 names a specific one for each.
    for reason in [both.inconclusive_reason(), seven_only.inconclusive_reason()] {
        assert_ne!(reason, Some(InconclusiveReason::Unsupported));
    }
    // A compile that configured neither stage owes no reason — and that is *not* `satisfied`.
    let neither = CausalCompile::new(durability_order(), RedactionPolicy::permitting_everything())
        .run(&roots(), &residual())
        .expect("stages 1-2 run");
    assert_eq!(neither.inconclusive_reason(), None);
}

// =====================================================================================
// stage 6 — the one that is implemented
// =====================================================================================

#[test]
fn stage_six_projects_onto_the_named_observers_and_is_audited_independently() {
    // The reduced configuration RFC 0028 requires a deployment be able to run: a plain causal
    // slice, with the property automaton not configured, so observer scoping is the only
    // narrowing there is.
    let compiled = CausalCompile::new(durability_order(), RedactionPolicy::permitting_everything())
        .with_observer_projection(projection(), audit())
        .run(&roots(), &residual())
        .expect("stages 1-2 and 6 run");

    // `e_flush` and `e_probe` are declared into families no observer publishes, and nothing
    // protects them here, so scoping removes them. `e_begin` is *also* unpublished and is kept,
    // because the re-closure needs it for `e_submit`.
    assert_eq!(compiled.observer_dropped(), &ids(&["e_flush", "e_probe"]));
    assert_eq!(
        compiled.selected(),
        ids(&CORE).into_iter().collect::<Vec<_>>()
    );
    assert!(compiled.is_causally_closed());

    // The reduction is checked by a second, independent computation over the same premise.
    let verdict = compiled.scope_verdict().expect("stage 6 ran");
    assert!(verdict.is_justified());
    let ScopeVerdict::Justified { observers } = verdict else {
        panic!("the honest reduction is justified");
    };
    assert_eq!(
        observers
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>(),
        ["client"]
    );
    compiled.accounting().reconcile().expect("halves agree");
}

#[test]
fn stage_six_licenses_nothing_even_when_it_narrows() {
    // RFC 0028: "scoping under INV-013; never a guarantee by itself".
    let with = CausalCompile::new(durability_order(), RedactionPolicy::permitting_everything())
        .with_observer_projection(projection(), audit())
        .run(&roots(), &residual())
        .expect("stages 1-2 and 6 run");
    let without = CausalCompile::new(durability_order(), RedactionPolicy::permitting_everything())
        .run(&roots(), &residual())
        .expect("stages 1-2 run");

    assert_eq!(with.guarantees(), without.guarantees());
    assert_eq!(with.guarantees().members(), [Guarantee::CausallyClosed]);
    // Not vacuous: the stage really did narrow the pack, and the audit really did justify it.
    assert_ne!(with.selected(), without.selected());
    assert_eq!(with.selected().len() + 2, without.selected().len());
    assert!(with.scope_verdict().expect("stage 6 ran").is_justified());
}

#[test]
fn a_stage_six_drop_is_heuristic_cutoff_and_never_slice_irrelevant() {
    let compiled = CausalCompile::new(durability_order(), RedactionPolicy::permitting_everything())
        .with_observer_projection(projection(), audit())
        .run(&roots(), &residual())
        .expect("stages 1-2 and 6 run");
    let cells: BTreeMap<(SelectionKind, OmissionReason), u32> = compiled
        .accounting()
        .manifest()
        .records()
        .iter()
        .map(|record| ((record.kind(), record.reason()), record.count()))
        .collect();
    // Two events left the pack for stage 6's reason, and stage 6 licenses *scoping*, not
    // irrelevance: INV-013's third justification source — the fairness obligations — has no
    // producer in this workspace, so an item outside every named observer's scope is undecided
    // rather than disproved. Everything else keeps stage 2's proof.
    assert_eq!(
        cells.get(&(SelectionKind::Event, OmissionReason::HeuristicCutoff)),
        Some(&2)
    );
    assert!(cells.contains_key(&(SelectionKind::Event, OmissionReason::SliceIrrelevant)));
    compiled.accounting().reconcile().expect("halves agree");
}

/// The load-bearing pair for INV-013 and RFC 0028's "Selection and the causal core".
#[test]
fn observer_projection_may_not_exclude_an_abstraction_relevant_hidden_event() {
    // Without the property, scoping drops `e_flush`: no observer publishes it.
    let unprotected =
        CausalCompile::new(durability_order(), RedactionPolicy::permitting_everything())
            .with_observer_projection(projection(), audit())
            .run(&roots(), &residual())
            .expect("stages 1-2 and 6 run");
    assert!(!unprotected.selected().contains(&name("e_flush")));

    // With the property, the *same* projection may not remove it — "a compiler MUST NOT exclude
    // an abstraction-relevant hidden event on the grounds that no observer publishes it".
    let protected =
        CausalCompile::new(durability_order(), RedactionPolicy::permitting_everything())
            .with_property(automaton(), PropertyMonitor::over(automaton()))
            .with_observer_projection(projection(), audit())
            .run(&roots(), &residual())
            .expect("stages 1-3 and 6 run");
    assert!(protected.selected().contains(&name("e_flush")));
    assert!(protected.is_property_preserving());
    assert!(protected.observer_dropped().is_empty());

    // And the independent monitor agrees the protected pack preserves the property, while the
    // unprotected one — a perfectly good answer to a different question — is not claimed to.
    assert!(!unprotected.is_property_preserving());
}

#[test]
fn stage_six_cannot_undo_stage_three() {
    // The general form of the pair above, and a theorem rather than a coincidence: stage 3's
    // output is the backward closure of the relevance set, so every member is either protected
    // or an ancestor of something protected, and stage 6 restores both.
    let with_three = staged().run(&roots(), &residual()).expect("stages 1-7 run");
    let without_six =
        CausalCompile::new(durability_order(), RedactionPolicy::permitting_everything())
            .with_property(automaton(), PropertyMonitor::over(automaton()))
            .with_dependence(join())
            .run(&roots(), &residual())
            .expect("stages 1-4 run");
    assert_eq!(with_three.selected(), without_six.selected());
    assert!(with_three.observer_dropped().is_empty());
    // The projection is applicable — `client` publishes three of this order's candidates — so
    // the stage really ran and really found nothing it was allowed to remove.
    assert_eq!(
        projection().applicability(&durability_order()),
        ProjectionApplicability::Applicable
    );
    assert_eq!(
        with_three
            .trail()
            .intermediate(Phase::Stage(Stage::ObserverProjection))
            .expect("stage 6 recorded")
            .note(),
        Some("observer-retained")
    );
}

#[test]
fn an_unattributed_candidate_is_never_scoped_away() {
    // INV-013 justifies a reduction *relative to* the named observers. The 214 noise candidates
    // and `s_writer`/`m_commit` have no declared observability, so nothing justifies removing
    // them and stage 6 does not. They leave the pack — or stay in it — for the reasons stages 2
    // and 4 decided, which is what the manifest records.
    let compiled = CausalCompile::new(durability_order(), RedactionPolicy::permitting_everything())
        .with_dependence(join())
        .with_observer_projection(projection(), audit())
        .run(&roots(), &residual())
        .expect("stages 1-2, 4 and 6 run");
    assert!(compiled.selected().contains(&name("s_writer")));
    for dropped in compiled.observer_dropped() {
        assert!(
            projection().is_attributed(dropped),
            "{dropped} was dropped by stage 6 without a declared observability"
        );
    }
    // And the audit says so from its own side.
    let hand_built = ScopeReduction::new(
        ids(&["e_begin", "e_submit", "e_ack", "s_writer"]),
        ids(&["e_begin", "e_submit", "e_ack"]),
        [],
    );
    assert_eq!(
        audit().verdict(&durability_order(), &hand_built),
        ScopeVerdict::Unjustified(ScopeViolation::UnattributedCandidateDropped {
            id: name("s_writer")
        })
    );
}

#[test]
fn a_coarsened_observer_narrows_the_pack() {
    // RFC 0031's `observers` → `coarsened` move seen from the compiler, and this file's
    // anti-vacuity for the whole of stage 6: a projection that reduced the same way whatever
    // the observers said would make every assertion above meaningless.
    let coarse = ObserverProjection::new(
        observer_set([an_observer("client", &["Submitted"])]),
        attribution(),
    )
    .expect("a well-formed observer projection");
    let compiled = CausalCompile::new(durability_order(), RedactionPolicy::permitting_everything())
        .with_observer_projection(coarse.clone(), ScopeAudit::over(coarse))
        .run(
            &roots(),
            &ExpansionQuery::new(ExpansionRelation::CausalPredecessors, name("e_submit")),
        )
        .expect("stages 1-2 and 6 run");
    assert_eq!(
        compiled.selected(),
        ids(&["e_begin", "e_submit"])
            .into_iter()
            .collect::<Vec<_>>()
    );
    assert_eq!(
        compiled.observer_dropped(),
        &ids(&["e_ack", "e_loss", "e_flush", "e_probe"])
    );
    assert!(compiled.is_causally_closed());
    assert!(
        compiled
            .scope_verdict()
            .expect("stage 6 ran")
            .is_justified()
    );
}

#[test]
fn an_intent_that_binds_no_observer_scopes_nothing_and_says_why() {
    let unbound = ObserverProjection::new(observer_set([]), attribution())
        .expect("a well-formed observer projection");
    let compiled = CausalCompile::new(durability_order(), RedactionPolicy::permitting_everything())
        .with_observer_projection(unbound.clone(), ScopeAudit::over(unbound))
        .run(&roots(), &residual())
        .expect("stages 1-2 and 6 run");

    // The vacuous reduction — "no observer publishes it, so drop everything" — is exactly what
    // an empty observer set would license if the stage did not refuse to act on it.
    assert!(compiled.observer_dropped().is_empty());
    assert_eq!(
        compiled.scope_verdict(),
        Some(&ScopeVerdict::Inapplicable(
            ProjectionInapplicable::NoNamedObserver
        ))
    );
    assert_eq!(
        compiled
            .trail()
            .intermediate(Phase::Stage(Stage::ObserverProjection))
            .expect("stage 6 recorded")
            .note(),
        Some("observer-inapplicable")
    );
    // It still ran, which is not the same fact as not being configured.
    assert!(
        CausalCompile::new(durability_order(), RedactionPolicy::permitting_everything())
            .run(&roots(), &residual())
            .expect("stages 1-2 run")
            .trail()
            .intermediate(Phase::Stage(Stage::ObserverProjection))
            .is_none()
    );
}

#[test]
fn an_order_no_named_observer_publishes_is_inapplicable_and_not_justified() {
    let elsewhere = ObserverProjection::new(
        observer_set([an_observer("client", &["SomethingElse"])]),
        attribution(),
    )
    .expect("a well-formed observer projection");
    let compiled = CausalCompile::new(durability_order(), RedactionPolicy::permitting_everything())
        .with_observer_projection(elsewhere.clone(), ScopeAudit::over(elsewhere))
        .run(&roots(), &residual())
        .expect("stages 1-2 and 6 run");
    assert!(compiled.observer_dropped().is_empty());
    assert_eq!(
        compiled.scope_verdict(),
        Some(&ScopeVerdict::Inapplicable(
            ProjectionInapplicable::NoObservedCandidate
        ))
    );
}

/// The independence claim, made falsifiable — the precedent
/// `the_monitor_rejects_hand_built_selections_the_filter_cannot_produce` set for stage 3.
#[test]
fn the_audit_rejects_hand_built_reductions_the_filter_cannot_produce() {
    let order = durability_order();
    let slice = ids(&PROPERTY_CORE);

    // Drops a candidate the named observer publishes.
    assert_eq!(
        audit().verdict(
            &order,
            &ScopeReduction::new(slice.clone(), ids(&["e_begin", "e_submit", "e_flush"]), [])
        ),
        ScopeVerdict::Unjustified(ScopeViolation::ObservedCandidateDropped {
            id: name("e_ack"),
            observer: ObserverId::new("client").expect("a non-empty unit key"),
        })
    );
    // Drops a candidate the property protects.
    assert_eq!(
        audit().verdict(
            &order,
            &ScopeReduction::new(
                slice.clone(),
                ids(&CORE),
                automaton().relevant().into_iter().collect::<Vec<_>>()
            )
        ),
        ScopeVerdict::Unjustified(ScopeViolation::ProtectedCandidateDropped {
            id: name("e_flush")
        })
    );
    // Grows the set instead of narrowing it.
    assert_eq!(
        audit().verdict(
            &order,
            &ScopeReduction::new(ids(&["e_begin"]), ids(&["e_begin", "e_ack"]), [])
        ),
        ScopeVerdict::Unjustified(ScopeViolation::CandidateAdded { id: name("e_ack") })
    );
    // Breaks the closure stage 2 licensed.
    assert!(matches!(
        audit().verdict(
            &order,
            &ScopeReduction::new(slice.clone(), ids(&["e_submit", "e_ack", "e_loss"]), [])
        ),
        ScopeVerdict::Unjustified(ScopeViolation::NotCausallyClosed(_))
    ));
    // And the filter's own output still passes, so the audit is not merely a rejector.
    let honest = ObserverFilter::retain(&projection(), &order, &slice, &BTreeSet::new())
        .expect("inside the order");
    assert!(
        audit()
            .verdict(&order, &ScopeReduction::new(slice, honest, []))
            .is_justified()
    );
}

#[test]
fn a_stricter_audit_than_the_projection_refuses_the_reduction() {
    // The two premises are supplied separately, exactly as stage 3's automaton and monitor are,
    // so a caller may audit against a wider observer set than the filter was given.
    let strict = ObserverProjection::new(
        observer_set([an_observer(
            "auditor",
            &["Submitted", "Acked", "Lost", "Internal"],
        )]),
        attribution(),
    )
    .expect("a well-formed observer projection");
    let compiled = CausalCompile::new(durability_order(), RedactionPolicy::permitting_everything())
        .with_observer_projection(projection(), ScopeAudit::over(strict))
        .run(&roots(), &residual())
        .expect("stages 1-2 and 6 run");

    assert!(matches!(
        compiled.scope_verdict(),
        Some(ScopeVerdict::Unjustified(
            ScopeViolation::ObservedCandidateDropped { .. }
        ))
    ));
    // The pack is unchanged and still honest: no guarantee turned on the audit, so there is
    // none to withdraw, and the unjustified reduction is *recorded* rather than hidden.
    assert!(compiled.is_causally_closed());
    compiled.accounting().reconcile().expect("halves agree");
}

// =====================================================================================
// the crate boundary, the input structure, and the disciplines carried forward
// =====================================================================================

#[test]
fn an_attribution_or_a_subject_from_nowhere_is_refused() {
    // Structure is checked at every stage's input, even where the stage's content cannot be.
    let stray = ObserverProjection::new(
        observer_set([an_observer("client", &["Acked"])]),
        [(name("e_nowhere"), seen("Acked"))],
    )
    .expect("a well-formed observer projection");
    assert_eq!(
        CausalCompile::new(durability_order(), RedactionPolicy::permitting_everything())
            .with_observer_projection(stray.clone(), ScopeAudit::over(stray))
            .run(&roots(), &residual()),
        Err(CompileError::Causal(CausalError::UnknownEndpoint {
            id: name("e_nowhere"),
        }))
    );
    assert_eq!(
        CausalCompile::new(durability_order(), RedactionPolicy::permitting_everything())
            .with_correspondence_mapping(CorrespondenceMapping::of([name("x_nowhere")]))
            .run(&roots(), &residual()),
        Err(CompileError::Causal(CausalError::UnknownEndpoint {
            id: name("x_nowhere"),
        }))
    );
    // An obligation, by contrast, is *not* checked against the order: it is a proof-side
    // identity produced by a subsystem this workspace does not have, so there is nothing here
    // to check it against and a check invented for it would be an unchecked authority.
    assert!(
        CausalCompile::new(durability_order(), RedactionPolicy::permitting_everything())
            .with_proof_slicing(ProofSlicing::over([name("o_nowhere")]))
            .run(&roots(), &residual())
            .is_ok()
    );
}

#[test]
fn a_candidate_attributed_twice_has_no_single_reading() {
    assert_eq!(
        ObserverProjection::new(
            observer_set([an_observer("client", &["Acked"])]),
            [
                (name("e_ack"), seen("Acked")),
                (name("e_ack"), seen("Something")),
            ],
        ),
        Err(ProjectionError::RepeatedCandidate { id: name("e_ack") })
    );
}

#[test]
fn redaction_still_wins_over_every_stage_of_this_group() {
    // A withheld candidate is not stage 5's, 6's or 7's to decide: it keeps the irretrievable
    // `redaction` record the pre-pass earned it, and the guarantee it was needed for is gone.
    let compiled = CausalCompile::new(
        durability_order(),
        RedactionPolicy::withholding([(name("e_submit"), RedactionReason::Purged)]),
    )
    .with_property(automaton(), PropertyMonitor::over(automaton()))
    .with_proof_slicing(ProofSlicing::over([name("o_durability")]))
    .with_observer_projection(projection(), audit())
    .run(&roots(), &residual())
    .expect("stages 1-3, 5 and 6 run");

    assert!(!compiled.selected().contains(&name("e_submit")));
    assert!(!compiled.is_causally_closed());
    assert!(!compiled.is_property_preserving());
    assert!(!compiled.observer_dropped().contains(&name("e_submit")));
    let redacted = compiled
        .accounting()
        .manifest()
        .records()
        .iter()
        .find(|record| record.reason() == OmissionReason::Redaction)
        .expect("the withheld candidate is a redaction omission");
    assert_eq!(redacted.count(), 1);
    assert!(!redacted.retrievability().is_expandable());
    // Both irretrievable reasons now appear in one manifest, and they stay distinct.
    assert!(
        compiled
            .accounting()
            .manifest()
            .records()
            .iter()
            .any(|record| record.reason() == OmissionReason::Unsupported)
    );
    compiled.accounting().reconcile().expect("halves agree");
}

#[test]
fn stage_four_still_admits_only_a_corroborated_dependence_underneath_this_group() {
    let compiled = staged().run(&roots(), &residual()).expect("stages 1-7 run");
    assert_eq!(
        compiled.declined().get(&name("m_commit")),
        Some(&Inadmissible::UncorroboratedSource)
    );
    assert_eq!(
        compiled.admissible().keys().collect::<Vec<_>>(),
        [&name("s_writer")]
    );
}

#[test]
fn two_runs_of_a_seven_stage_compile_agree() {
    let one = staged().run(&roots(), &residual()).expect("stages 1-7 run");
    let other = staged().run(&roots(), &residual()).expect("stages 1-7 run");
    assert_eq!(one, other);
    assert_eq!(
        one.trail().to_json().to_canonical_bytes(),
        other.trail().to_json().to_canonical_bytes()
    );
    // And a different compile has a different trail, so the pin is not vacuous.
    let fewer = CausalCompile::new(durability_order(), RedactionPolicy::permitting_everything())
        .with_property(automaton(), PropertyMonitor::over(automaton()))
        .run(&roots(), &residual())
        .expect("stages 1-3 run");
    assert_ne!(
        one.trail().to_json().to_canonical_bytes(),
        fewer.trail().to_json().to_canonical_bytes()
    );
}

// =====================================================================================
// the tripwires: refusals pinned to the workspace facts that justify them
// =====================================================================================

/// The two crates plan §20 names as the producers of stages 5's and 7's inputs, held as their
/// own bytes so the claim "not deployed here" is checked rather than asserted.
const PROOF_CLIENT_LIB: &str = include_str!("../../continuum-proof-client/src/lib.rs");
const REFINEMENT_LIB: &str = include_str!("../../continuum-refinement/src/lib.rs");

/// Every `pub` item declaration in a source text, as a line.
fn pub_items(source: &str) -> Vec<&str> {
    source
        .lines()
        .map(str::trim)
        .filter(|line| line.starts_with("pub "))
        .collect()
}

/// **TRIPWIRE.** Stage 5's and stage 7's producing subsystems have not arrived: both crates are
/// zero-public-item scaffolds that say so themselves.
///
/// The moment either grows a public item this test fails, and the refusals in
/// `continuum_context::{proof, correspondence}` must be reconciled against a surface that now
/// exists rather than left standing on a stale reading of the workspace. That is the whole
/// point: a refusal is a claim about the deployment, and a claim needs an artifact that can go
/// red.
#[test]
fn tripwire_the_proof_service_and_the_correspondence_graph_have_no_producer() {
    for (crate_name, source) in [
        (AbsentProducer::ProofService, PROOF_CLIENT_LIB),
        (AbsentProducer::CorrespondenceGraph, REFINEMENT_LIB),
    ] {
        let items = pub_items(source);
        assert!(
            items.is_empty(),
            "`{}` grew public items {items:?}; stage {} no longer has an absent producer — \
             reconcile its refusal against the surface that now exists",
            crate_name.crate_name(),
            if crate_name == AbsentProducer::ProofService {
                5
            } else {
                7
            }
        );
        assert!(
            source.contains("PR-1 / IMPL-01 scaffold"),
            "`{}` no longer declares itself a scaffold",
            crate_name.crate_name()
        );
    }
    // The two crates are the ones the refusal vocabulary names, so the tripwire and the
    // production code cannot drift apart.
    assert!(PROOF_CLIENT_LIB.contains("continuum-proof-client"));
    assert!(REFINEMENT_LIB.contains("continuum-refinement"));
    // And each still declares the responsibility the refusal cites.
    assert!(REFINEMENT_LIB.contains("The correspondence graph, proof-oriented lenses"));
    assert!(REFINEMENT_LIB.contains("plan §16"));
    assert!(PROOF_CLIENT_LIB.contains("Client for the proof service"));
}

#[test]
fn negative_the_producer_tripwire_is_not_vacuous() {
    // `pub_items` finds items where there are items — this crate's own stage-5 module has them.
    assert!(!pub_items(include_str!("../src/proof.rs")).is_empty());
    // And the mutant: the scaffold with one public item spliced in is caught.
    let mutant = format!("{PROOF_CLIENT_LIB}\npub fn slice() {{}}\n");
    assert_eq!(pub_items(&mutant), ["pub fn slice() {}"]);
}

// --- the licence inventory: `ProofRelevant` is unreachable by construction ------------------

/// Every source file of `continuum-context`, recorded rather than globbed.
///
/// A recorded inventory is itself a tripwire: a new module has to be added here before the scan
/// below can pass, so a module cannot start licensing a guarantee without this audit being
/// redone. `every_source_file_of_the_crate_is_recorded` closes the loop against the directory.
const SOURCES: [(&str, &str); 27] = [
    ("accounting.rs", include_str!("../src/accounting.rs")),
    ("assurance.rs", include_str!("../src/assurance.rs")),
    ("budget.rs", include_str!("../src/budget.rs")),
    ("causal.rs", include_str!("../src/causal.rs")),
    ("compile.rs", include_str!("../src/compile.rs")),
    (
        "correspondence.rs",
        include_str!("../src/correspondence.rs"),
    ),
    ("dependence.rs", include_str!("../src/dependence.rs")),
    ("event.rs", include_str!("../src/event.rs")),
    ("expansion.rs", include_str!("../src/expansion.rs")),
    ("guarantee.rs", include_str!("../src/guarantee.rs")),
    ("lib.rs", include_str!("../src/lib.rs")),
    ("model.rs", include_str!("../src/model.rs")),
    ("monitor.rs", include_str!("../src/monitor.rs")),
    ("observer.rs", include_str!("../src/observer.rs")),
    ("omission.rs", include_str!("../src/omission.rs")),
    ("pack.rs", include_str!("../src/pack.rs")),
    ("proof.rs", include_str!("../src/proof.rs")),
    ("property.rs", include_str!("../src/property.rs")),
    ("replay.rs", include_str!("../src/replay.rs")),
    ("scope.rs", include_str!("../src/scope.rs")),
    ("selection.rs", include_str!("../src/selection.rs")),
    ("source.rs", include_str!("../src/source.rs")),
    ("stage.rs", include_str!("../src/stage.rs")),
    ("state_delta.rs", include_str!("../src/state_delta.rs")),
    ("target.rs", include_str!("../src/target.rs")),
    ("unsupported.rs", include_str!("../src/unsupported.rs")),
    ("verdict.rs", include_str!("../src/verdict.rs")),
];

/// A module's production half, with comment lines removed.
///
/// The `#[cfg(test)]` split is `monitor.rs`'s own precedent — a test is not the compiler — and
/// the comment filter matters because `guarantee.rs`'s module documentation carries a
/// `compile_fail` doctest whose whole point is a `License::issue` line that must never compile.
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

/// Every guarantee the crate's production code mints a licence for, in the order the sources
/// are recorded.
fn licensed_guarantees(sources: &[(&str, &str)]) -> Vec<String> {
    const CALL: &str = "License::issue(";
    let mut found = Vec::new();
    for (_, source) in sources {
        let code = production_code(source);
        let mut rest = code.as_str();
        while let Some(at) = rest.find(CALL) {
            let after = &rest[at + CALL.len()..];
            let close = after
                .find(')')
                .expect("a `License::issue` call site closes its argument list");
            found.push(after[..close].trim().to_owned());
            rest = &after[close..];
        }
    }
    found
}

/// **TRIPWIRE / anti-vacuity mutant.** The guarantee-licensing surface of the whole crate,
/// recorded exactly.
///
/// RFC 0028's guarantee-violation list, read at this stage group, names the mutant this test
/// exists to make unreachable: *a stage that quietly licenses `ProofRelevant` with no proof
/// slice behind it*. `License::issue` is `pub(crate)`, so the only code that could mint that
/// permission is this crate — and the inventory below is every call site there is. Two of them,
/// both in `compile.rs`, both from a checker's verdict, and neither for `ProofRelevant`.
///
/// A third call site, or a different guarantee at either of these two, fails this test until
/// the audit is redone.
#[test]
fn tripwire_the_crate_licenses_exactly_two_guarantees_and_proof_relevance_is_not_one() {
    let licensed = licensed_guarantees(&SOURCES);
    assert_eq!(
        licensed,
        [
            "Guarantee::CausallyClosed".to_owned(),
            "Guarantee::PropertyPreserving".to_owned(),
        ],
        "the crate's `License::issue` call sites have changed"
    );
    assert!(!licensed.iter().any(|g| g.contains("ProofRelevant")));
    // The compile-time half of the same claim: `ProofRelevant` is absent from every pack this
    // pipeline can build, including the one that ran all seven stages.
    assert!(
        !staged()
            .run(&roots(), &residual())
            .expect("stages 1-7 run")
            .guarantees()
            .members()
            .contains(&Guarantee::ProofRelevant)
    );
}

#[test]
fn negative_the_licence_inventory_is_not_vacuous() {
    // The mutant RFC 0028 names, spliced into the production half of a real module: the scan
    // finds it. Without this, an inventory that matched because the scanner found nothing at
    // all would prove nothing.
    let mutant = format!(
        "{}\nfn sneak(set: &mut GuaranteeSet) {{ set.claim(License::issue(Guarantee::ProofRelevant)); }}\n#[cfg(test)]\nmod tests {{}}",
        production_code(SOURCES[4].1)
    );
    let licensed = licensed_guarantees(&[("mutant.rs", &mutant)]);
    assert!(
        licensed.contains(&"Guarantee::ProofRelevant".to_owned()),
        "the scan must catch a spliced licence: {licensed:?}"
    );
    // And a source with no call site at all yields nothing, so the scanner is not matching on
    // the word `Guarantee`.
    assert!(licensed_guarantees(&[("proof.rs", SOURCES[16].1)]).is_empty());
    assert_eq!(SOURCES[16].0, "proof.rs");
    assert_eq!(SOURCES[4].0, "compile.rs");
}

#[test]
fn every_source_file_of_the_crate_is_recorded() {
    // Closes the loop on the inventory above: a new module fails this test until it is added,
    // so the licence scan cannot silently stop covering the crate.
    let mut on_disk: Vec<String> = std::fs::read_dir(concat!(env!("CARGO_MANIFEST_DIR"), "/src"))
        .expect("the crate's source directory")
        .map(|entry| {
            entry
                .expect("a readable directory entry")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .filter(|file| file.ends_with(".rs"))
        .collect();
    on_disk.sort();
    let recorded: Vec<String> = SOURCES.iter().map(|(file, _)| (*file).to_owned()).collect();
    assert_eq!(on_disk, recorded);
}

// --- stage 6's input really is landed, and the audit really is independent -----------------

/// `observers.rs`'s own bytes: the evidence that stage 6's input exists is that this file
/// compiles against those types, and this pin is the freshness half — if the field group is
/// renamed or narrowed, stage 6's "the input is here" reading is re-derived rather than assumed.
const INTENT_OBSERVERS: &str = include_str!("../../continuum-intent/src/observers.rs");

#[test]
fn tripwire_the_intents_observers_are_still_the_landed_field_group_stage_six_reads() {
    for declaration in [
        "pub struct ObserverSet {",
        "pub struct Observer {",
        "pub struct ObserverId(String);",
        "pub enum ProjectionKind {",
        "pub const ALL: [Self; 4] = [Self::Events, Self::State, Self::Knowledge, Self::Security];",
    ] {
        assert!(
            INTENT_OBSERVERS.contains(declaration),
            "`continuum-intent`'s observer group no longer declares {declaration:?}; stage 6's \
             input has moved and its reading must be re-derived"
        );
    }
    // The four projections are compared by kind, and stage 6 uses the same vocabulary rather
    // than a second copy of it.
    assert_eq!(ProjectionKind::ALL.len(), 4);
    assert_eq!(seen("Acked").kind(), ProjectionKind::Events);
}

/// `scope.rs`'s own bytes, so the independence claim is checked rather than asserted.
const SCOPE_SOURCE: &str = include_str!("../src/scope.rs");

#[test]
fn the_scope_audit_module_imports_only_the_premise_the_filter_also_reads() {
    // The mechanical half of `crate::scope`'s independence argument, and the exact device
    // `the_monitor_module_imports_only_the_premise_the_filter_also_reads` uses for stage 3.
    // Comment lines are stripped first: this module's documentation *names* the filter it must
    // not call, in the argument for why it does not, and a check that could not tell the two
    // apart would be unfalsifiable in the wrong direction.
    let production = production_code(SCOPE_SOURCE);
    let imports: Vec<&str> = production
        .lines()
        .filter(|line| line.starts_with("use crate::"))
        .collect();
    assert_eq!(
        imports,
        [
            "use crate::causal::{CausalOrder, ClosureViolation};",
            "use crate::observer::{ObserverProjection, ProjectionApplicability, ProjectionInapplicable};",
        ]
    );
    assert!(!production.contains("ObserverFilter"));
    assert!(!production.contains("use crate::compile"));
    // And, the point of this whole stage: the audit reaches no licence.
    assert!(!production.contains("License::issue"));
    assert!(!production.contains("use crate::guarantee"));
}

#[test]
fn a_guarantee_still_cannot_be_minted_from_outside_the_crate() {
    // The crate-boundary discipline stage groups 1 and 2 established, restated at this group's
    // surface: a request is not a claim, and there is no constructor out here for either
    // `ProofRelevant` or anything else.
    let requested = RequestedGuarantees::of([Guarantee::ProofRelevant, Guarantee::CausallyClosed]);
    assert_eq!(requested.members().len(), 2);
    let compiled = staged().run(&roots(), &residual()).expect("stages 1-7 run");
    assert!(
        !compiled
            .guarantees()
            .members()
            .contains(&Guarantee::ProofRelevant)
    );
    assert!(
        compiled
            .guarantees()
            .members()
            .contains(&Guarantee::CausallyClosed)
    );
}
