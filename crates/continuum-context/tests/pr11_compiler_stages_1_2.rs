//! PR-11 / the Context Pack compiler, **stage group 1** (bn-21vno): the redaction pre-pass,
//! stage 1 (root selection), and stage 2 (backward causal slicing), exercised from outside
//! the crate at the grain the G0-DX-01 row's experiment uses.
//!
//! # Why this file exists beside the unit tests
//!
//! The unit tests in `src/` check each rule at the smallest scale that can express it. This
//! file checks two things they structurally cannot:
//!
//! 1. **The public surface is the whole surface.** Everything below is reachable through
//!    `continuum_context::{causal, compile, guarantee, stage}` with no `pub(crate)` help, so
//!    a daemon wiring `context.compile` in a later bone has the API it needs — and, in the
//!    other direction, `guarantee::License::issue` is *not* reachable, which is what makes
//!    "a guarantee is a checked claim" a property of the crate boundary rather than of this
//!    crate's own discipline.
//! 2. **The stage group behaves at the row's grain.** G0-DX-01 is "200+ event noisy
//!    durability failure → Context Pack" with pass condition "causal core replay-preserving;
//!    ≥10× context reduction; exact expansion handles"
//!    (`notes/plan/notes/G0_SPIKE_MATRIX.md`). The compiler owes the *core selection* half of
//!    that row — `crates/continuumd/tests/dx01_falsification.rs` declares stage 2 as harness
//!    code and names the compiler as the missing producer. Here the 225-candidate order is
//!    sliced by the production stage 2, and the four-event core, the exact 221-item manifest,
//!    and the closure licence are all read off the compiler's own output.
//!
//! # Honest scope, stated before any number
//!
//! This is **stage group 1 only**. The reduction reported below is at *candidate count*, not
//! at bytes: byte measurement is `content_budget.bytes` on an assembled root pack (RFC 0028
//! correction 17), and root assembly is the last child of bn-8g9uj, not this one. Nothing
//! here claims `ReplayPreserving` — stage 2 licenses only its precondition and the replay
//! checker is not this crate's — and nothing here claims `PropertyPreserving`, which is
//! stage 3's. The one guarantee this stage group can issue is `CausallyClosed`, and every
//! assertion about it below is decided by
//! `causal::CausalOrder::closure_violation`, which reads the order's edges and knows nothing
//! about how the selection was made (RFC 0028, "Validation").
//!
//! # House rules
//!
//! Deterministic (INV-005): no clock, no entropy, no float; every set is a `BTree` and the
//! whole fixture is built by a pure function of its own constants.

use std::collections::BTreeSet;

use continuum_context::causal::{CausalOrder, ClosureViolation, Completeness, PartialReason};
use continuum_context::compile::{CausalCompile, CompileError, RedactionPolicy, RedactionReason};
use continuum_context::expansion::{ExpansionQuery, ExpansionRelation};
use continuum_context::guarantee::{Guarantee, GuaranteeSet, RequestedGuarantees};
use continuum_context::omission::OmissionReason;
use continuum_context::selection::SelectionKind;
use continuum_context::stage::{Phase, Stage};
use continuum_value::identity::Blake3Hasher;
use continuum_value::value::Name;

/// How many noise events surround the core. 221 + 4 = 225, the dx01 campaign's own trace
/// length.
const NOISE: usize = 221;

/// The four causal events, in causal order: the durability failure's own chain.
const CORE: [&str; 4] = ["e_begin", "e_submit", "e_ack", "e_loss"];

fn name(text: &str) -> Name {
    Name::new(text).expect("a canonical identifier")
}

fn noise_id(index: usize) -> Name {
    name(&format!("n_{index:04}"))
}

fn ids(names: &[&str]) -> BTreeSet<Name> {
    names.iter().map(|id| name(id)).collect()
}

/// The fixture: a four-event causal chain plus 221 causally isolated noise events, several
/// of them carrying durability vocabulary as deliberate red herrings (the dx01 campaign's
/// A02 attack, in miniature — a textually assembled core admits these and misses `e_begin`).
fn durability_order() -> CausalOrder {
    let mut nodes: Vec<(Name, SelectionKind)> = CORE
        .iter()
        .map(|id| (name(id), SelectionKind::Event))
        .collect();
    for index in 0..NOISE {
        // Every fourth noise event is a `state_delta`, so the manifest partitions over more
        // than one kind and the counting equation is checked across cells rather than in one.
        let kind = if index % 4 == 0 {
            SelectionKind::StateDelta
        } else {
            SelectionKind::Event
        };
        nodes.push((noise_id(index), kind));
    }
    let edges = vec![
        (name("e_submit"), name("e_begin")),
        (name("e_ack"), name("e_submit")),
        (name("e_loss"), name("e_ack")),
    ];
    CausalOrder::new(nodes, edges).expect("a well-formed causal order")
}

fn residual() -> ExpansionQuery {
    ExpansionQuery::new(ExpansionRelation::CausalPredecessors, name("e_loss"))
}

fn clean() -> CausalCompile {
    CausalCompile::new(durability_order(), RedactionPolicy::permitting_everything())
}

// =====================================================================================
// positive: the row's experiment, run by the compiler
// =====================================================================================

#[test]
fn the_noisy_durability_case_slices_to_its_four_event_core() {
    let compiled = clean()
        .run(&ids(&["e_loss"]), &residual())
        .expect("stages 1-2 run");

    assert_eq!(compiled.accounting().candidate_count(), 225);
    assert_eq!(compiled.selected().len(), 4);
    assert_eq!(
        compiled.selected().iter().cloned().collect::<BTreeSet<_>>(),
        ids(&CORE)
    );
    // The reduction at candidate-count grain. Bytes are root assembly's measurement, not
    // this stage group's — see this file's "Honest scope".
    assert_eq!(225 / compiled.selected().len(), 56);
}

#[test]
fn the_core_is_licensed_causally_closed_by_the_independent_checker() {
    let compiled = clean()
        .run(&ids(&["e_loss"]), &residual())
        .expect("stages 1-2 run");

    assert!(compiled.is_causally_closed());
    assert_eq!(compiled.closure_violation(), None);
    assert_eq!(compiled.guarantees().members(), [Guarantee::CausallyClosed]);

    // And the same verdict, reached a second way: hand the published selection back to the
    // checker directly, from outside the compiler entirely.
    let order = durability_order();
    let published: BTreeSet<Name> = compiled.selected().iter().cloned().collect();
    assert!(order.is_downward_closed(&published));
    assert_eq!(order.closure_violation(&published), None);
}

#[test]
fn the_manifest_accounts_for_every_one_of_the_221_noise_candidates() {
    let compiled = clean()
        .run(&ids(&["e_loss"]), &residual())
        .expect("stages 1-2 run");
    let manifest = compiled.accounting().manifest();

    assert_eq!(manifest.total(), 221);
    // Two cells: `event` and `state_delta`, both `slice-irrelevant`, both expandable behind
    // the one residual query.
    assert_eq!(manifest.records().len(), 2);
    for record in manifest.records() {
        assert_eq!(record.reason(), OmissionReason::SliceIrrelevant);
        assert!(record.retrievability().is_expandable());
        assert_eq!(record.retrievability().query(), Some(&residual()));
    }
    let by_kind: Vec<(SelectionKind, u32)> = manifest
        .records()
        .iter()
        .map(|record| (record.kind(), record.count()))
        .collect();
    assert_eq!(
        by_kind,
        [(SelectionKind::Event, 165), (SelectionKind::StateDelta, 56),]
    );

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

#[test]
fn every_phase_left_a_retrievable_intermediate_in_pipeline_order() {
    let compiled = clean()
        .run(&ids(&["e_loss"]), &residual())
        .expect("stages 1-2 run");
    let trail = compiled.trail();

    assert_eq!(
        trail.phases(),
        [
            Phase::Redaction,
            Phase::Stage(Stage::RootSelection),
            Phase::Stage(Stage::CausalSlicing),
        ]
    );
    assert_eq!(trail.universe().len(), 225);
    assert_eq!(
        trail
            .intermediate(Phase::Redaction)
            .expect("the pre-pass recorded")
            .len(),
        225
    );
    assert_eq!(
        trail
            .intermediate(Phase::Stage(Stage::RootSelection))
            .expect("stage 1 recorded")
            .working_set(),
        &ids(&["e_loss"])
    );
    assert_eq!(
        trail
            .intermediate(Phase::Stage(Stage::CausalSlicing))
            .expect("stage 2 recorded")
            .working_set(),
        &ids(&CORE)
    );
    // Stage 2 *grew* what stage 1 selected. A trail that assumed narrowing would have been
    // wrong about the one stage that is not a filter.
    assert!(
        trail
            .intermediate(Phase::Stage(Stage::CausalSlicing))
            .expect("stage 2 recorded")
            .len()
            > trail
                .intermediate(Phase::Stage(Stage::RootSelection))
                .expect("stage 1 recorded")
                .len()
    );
    // A stage that did not run has no intermediate — distinct from an empty one.
    for stage in [
        Stage::PropertyRelevance,
        Stage::ProofSlicing,
        Stage::MinimalCore,
        Stage::HeuristicRanking,
        Stage::BudgetPacking,
    ] {
        assert!(trail.intermediate(Phase::Stage(stage)).is_none());
    }
}

// =====================================================================================
// negative: each refusal, and its own arm
// =====================================================================================

/// RFC 0028 C2's own example of what is not accepted: a set assembled from textual
/// relevance. Every id carrying durability vocabulary, plus the noise that imitates it —
/// and the checker refuses it, from the order's edges alone.
#[test]
fn a_textually_assembled_core_is_not_causally_closed() {
    let order = durability_order();
    let textual: BTreeSet<Name> = ids(&["e_submit", "e_ack", "e_loss"])
        .into_iter()
        .chain([noise_id(0), noise_id(7)])
        .collect();
    assert_eq!(
        order.closure_violation(&textual),
        Some(ClosureViolation::MissingPredecessor {
            selected: name("e_submit"),
            predecessor: name("e_begin"),
        })
    );
}

/// The anti-vacuity companion: dropping *any* one core event breaks closure, and each drop
/// fails at its own successor. A checker that passed everything would pass these too.
#[test]
fn dropping_any_single_core_event_breaks_the_closure() {
    let order = durability_order();
    for (dropped, successor) in [
        ("e_begin", "e_submit"),
        ("e_submit", "e_ack"),
        ("e_ack", "e_loss"),
    ] {
        let mut mutant = ids(&CORE);
        mutant.remove(&name(dropped));
        assert_eq!(
            order.closure_violation(&mutant),
            Some(ClosureViolation::MissingPredecessor {
                selected: name(successor),
                predecessor: name(dropped),
            }),
            "dropping `{dropped}` was not caught"
        );
    }
    // The fourth core event is the root: dropping it leaves a shorter *closed* prefix, which
    // is the honest answer — closure is not a claim of completeness.
    let mut without_root = ids(&CORE);
    without_root.remove(&name("e_loss"));
    assert_eq!(order.closure_violation(&without_root), None);
}

#[test]
fn a_redacted_root_is_refused_before_any_slicing() {
    let compile = CausalCompile::new(
        durability_order(),
        RedactionPolicy::withholding([(name("e_loss"), RedactionReason::Purged)]),
    );
    assert_eq!(
        compile.run(&ids(&["e_loss"]), &residual()),
        Err(CompileError::RedactedRoot {
            id: name("e_loss"),
            reason: RedactionReason::Purged,
        })
    );
}

#[test]
fn a_redacted_causal_ancestor_costs_the_guarantee_rather_than_the_honesty() {
    let compile = CausalCompile::new(
        durability_order(),
        RedactionPolicy::withholding([(name("e_begin"), RedactionReason::Purged)]),
    );
    let compiled = compile
        .run(&ids(&["e_loss"]), &residual())
        .expect("stages 1-2 run");

    assert_eq!(compiled.redaction_gap(), &ids(&["e_begin"]));
    assert!(!compiled.is_causally_closed());
    assert!(compiled.guarantees().members().is_empty());
    assert_eq!(
        compiled.closure_violation(),
        Some(&ClosureViolation::MissingPredecessor {
            selected: name("e_submit"),
            predecessor: name("e_begin"),
        })
    );
    // The withheld ancestor is still named and counted: `expandable: false`, reason
    // `redaction`. A pack that answered anyway would be the confident smaller answer plan
    // §18.4 prohibits.
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
    assert_eq!(compiled.accounting().manifest().total(), 222);
}

#[test]
fn a_residual_query_nothing_published_anchors_is_refused() {
    let query = ExpansionQuery::new(ExpansionRelation::CausalPredecessors, noise_id(3));
    assert_eq!(
        clean().run(&ids(&["e_loss"]), &query),
        Err(CompileError::AnchorNotSelected {
            anchor: noise_id(3)
        })
    );
}

#[test]
fn a_partial_causal_order_cannot_call_its_drops_provable() {
    let partial = CausalOrder::partial(
        durability_order()
            .nodes()
            .map(|(id, kind)| (id.clone(), kind))
            .collect::<Vec<_>>(),
        [
            (name("e_submit"), name("e_begin")),
            (name("e_ack"), name("e_submit")),
            (name("e_loss"), name("e_ack")),
        ],
        PartialReason::InsufficientTelemetry,
    )
    .expect("a well-formed causal order");
    assert_eq!(
        partial.completeness(),
        &Completeness::Partial {
            reason: PartialReason::InsufficientTelemetry
        }
    );

    let compiled = CausalCompile::new(partial, RedactionPolicy::permitting_everything())
        .run(&ids(&["e_loss"]), &residual())
        .expect("stages 1-2 run");

    // The same core, the same closure licence — and every drop recorded as
    // `heuristic-cutoff`, because non-ancestry in an incomplete order proves nothing.
    assert_eq!(compiled.selected().len(), 4);
    assert!(compiled.is_causally_closed());
    for record in compiled.accounting().manifest().records() {
        assert_eq!(record.reason(), OmissionReason::HeuristicCutoff);
    }
}

// =====================================================================================
// boundary
// =====================================================================================

#[test]
fn a_root_that_is_the_whole_order_omits_nothing() {
    let order = CausalOrder::new(
        [
            (name("e_1"), SelectionKind::Event),
            (name("e_2"), SelectionKind::Event),
        ],
        [(name("e_2"), name("e_1"))],
    )
    .expect("a well-formed causal order");
    let compiled = CausalCompile::new(order, RedactionPolicy::permitting_everything())
        .run(
            &ids(&["e_2"]),
            &ExpansionQuery::new(ExpansionRelation::SameOwner, name("e_2")),
        )
        .expect("stages 1-2 run");

    assert!(compiled.accounting().manifest().is_empty());
    assert!(compiled.is_causally_closed());
    assert_eq!(compiled.selected().len(), 2);
}

#[test]
fn an_empty_order_compiles_to_an_empty_pack() {
    let compiled = CausalCompile::new(
        CausalOrder::new([], []).expect("nothing to order"),
        RedactionPolicy::permitting_everything(),
    )
    .run(
        &BTreeSet::new(),
        &ExpansionQuery::new(ExpansionRelation::SameOwner, name("e_1")),
    )
    .expect("stages 1-2 run");

    assert_eq!(compiled.accounting().candidate_count(), 0);
    assert!(compiled.selected().is_empty());
    assert!(compiled.accounting().manifest().is_empty());
    // An empty selection is vacuously closed, and the licence says exactly that and no more.
    assert!(compiled.is_causally_closed());
}

#[test]
fn two_roots_on_one_chain_select_the_same_core_as_the_deeper_one_alone() {
    let one = clean()
        .run(&ids(&["e_loss"]), &residual())
        .expect("stages 1-2 run");
    let both = clean()
        .run(&ids(&["e_loss", "e_submit"]), &residual())
        .expect("stages 1-2 run");
    assert_eq!(one.selected(), both.selected());
}

// =====================================================================================
// the two disciplines the crate boundary carries
// =====================================================================================

#[test]
fn a_request_is_not_a_claim_across_the_crate_boundary() {
    let requested = RequestedGuarantees::of([
        Guarantee::ReplayPreserving,
        Guarantee::CausallyClosed,
        Guarantee::PropertyPreserving,
        Guarantee::OneMinimal,
    ]);
    let compiled = clean()
        .run(&ids(&["e_loss"]), &residual())
        .expect("stages 1-2 run");

    // Four asked for; one achieved; and none of the other three echoed into the published
    // set. `RequestedGuarantees` has no conversion into a claim, and `GuaranteeSet::claim`
    // needs a `License` whose constructor this crate does not export — so from out here the
    // published set can only be *read*, which is rule C1 enforced by the crate boundary.
    assert_eq!(compiled.guarantees().members(), [Guarantee::CausallyClosed]);
    assert_eq!(
        requested.unachieved(&GuaranteeSet::empty()),
        [
            Guarantee::ReplayPreserving,
            Guarantee::PropertyPreserving,
            Guarantee::OneMinimal,
            Guarantee::CausallyClosed,
        ]
    );
}

#[test]
fn the_whole_compile_is_deterministic() {
    let one = clean()
        .run(&ids(&["e_loss"]), &residual())
        .expect("stages 1-2 run");
    let other = clean()
        .run(&ids(&["e_loss"]), &residual())
        .expect("stages 1-2 run");

    assert_eq!(one, other);
    assert_eq!(
        one.trail().digest::<Blake3Hasher>(),
        other.trail().digest::<Blake3Hasher>()
    );
    assert_eq!(
        one.trail().to_json().to_canonical_bytes(),
        other.trail().to_json().to_canonical_bytes()
    );
    // Not vacuous: a different compile is a different trail.
    let narrower = clean()
        .run(
            &ids(&["e_ack"]),
            &ExpansionQuery::new(ExpansionRelation::CausalPredecessors, name("e_ack")),
        )
        .expect("stages 1-2 run");
    assert_ne!(
        one.trail().digest::<Blake3Hasher>(),
        narrower.trail().digest::<Blake3Hasher>()
    );
}
