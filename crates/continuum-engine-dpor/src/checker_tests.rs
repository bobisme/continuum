//! Anti-vacuity for the checker: a sound witness is accepted, and each kind of
//! tampering with it is rejected with the defect that names it.
//!
//! Tests of the RFC 0004 reduction-witness checker (correction 1).

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects
)]

use crate::test_support::{corpus, handmade};

use continuum_model_core::model::Model;

use super::{WitnessDefect, check_witness};
use crate::report::{Bounds, DeadlockPolicy, Obligations};
use crate::witness::{Expansion, ReductionWitness};

const WORK: u64 = 1 << 40;

fn run(model: &Model) -> (Obligations, ReductionWitness) {
    let obligations = Obligations::every_predicate(model, DeadlockPolicy::Defect);
    let report = crate::check(
        model,
        &obligations,
        Bounds::new(1 << 16, 1 << 20, 1 << 16, WORK),
    )
    .unwrap();
    (obligations, report.witness().clone())
}

fn handmade(id: &str) -> Model {
    handmade::all()
        .into_iter()
        .find(|(name, _)| *name == id)
        .unwrap()
        .1
}

#[test]
fn a_sound_witness_is_accepted_and_an_incomplete_one_is_not() {
    let model = handmade("ignoring");
    let (obligations, mut witness) = run(&model);
    assert!(check_witness(&model, &obligations, &witness, WORK).is_ok());
    assert_eq!(
        check_witness(&model, &obligations, &witness, 10),
        Err(WitnessDefect::WorkExhausted)
    );
    witness.complete = false;
    assert_eq!(
        check_witness(&model, &obligations, &witness, WORK),
        Err(WitnessDefect::Incomplete)
    );
}

#[test]
fn a_wrong_visible_set_is_rejected() {
    let model = handmade("ignoring");
    let (obligations, mut witness) = run(&model);
    witness.visible = 0;
    assert!(matches!(
        check_witness(&model, &obligations, &witness, WORK),
        Err(WitnessDefect::VisibleSet { .. })
    ));
}

#[test]
fn a_stubborn_set_that_is_not_closed_is_rejected() {
    // The first generated model with a reduced state whose stubborn set has a
    // member besides its seed: cut the set down to the seed.
    for seed in 0..200 {
        let model = corpus::generate(seed).model;
        let (obligations, mut witness) = run(&model);
        let Some(node) = witness.nodes.iter_mut().find(
            |node| matches!(&node.expansion, Expansion::Reduced { stubborn, .. } if stubborn.len() > 1),
        ) else {
            continue;
        };
        if let Expansion::Reduced { seed, stubborn } = &mut node.expansion {
            *stubborn = vec![*seed];
        }
        assert!(matches!(
            check_witness(&model, &obligations, &witness, WORK),
            Err(WitnessDefect::NotClosed { .. })
        ));
        return;
    }
    panic!("no generated model has a multi-member stubborn set");
}

#[test]
fn a_dropped_edge_is_uncovered() {
    let model = handmade("philosophers");
    let (obligations, mut witness) = run(&model);
    let visit = witness
        .visits
        .iter()
        .position(|visit| visit.edges.len() > 1)
        .unwrap();
    witness.visits[visit].edges.pop();
    assert!(matches!(
        check_witness(&model, &obligations, &witness, WORK),
        Err(WitnessDefect::Uncovered { .. }
            | WitnessDefect::Unreached { .. }
            | WitnessDefect::NodeWithoutVisit { .. })
    ));
}

#[test]
fn a_retargeted_edge_is_rejected() {
    let model = handmade("philosophers");
    let (obligations, mut witness) = run(&model);
    let edge = &mut witness.visits[0].edges[0];
    edge.target = if edge.target == 0 { 1 } else { 0 };
    assert!(matches!(
        check_witness(&model, &obligations, &witness, WORK),
        Err(WitnessDefect::Edge { .. } | WitnessDefect::Sleep { .. })
    ));
}

#[test]
fn an_unjustified_sleep_member_is_rejected() {
    // Put an enabled label to sleep on a visit that was entered awake.
    let model = handmade("wakeup");
    let (obligations, mut witness) = run(&model);
    let (index, label) = witness
        .visits
        .iter()
        .enumerate()
        .find_map(|(index, visit)| {
            (index != witness.roots[0] && visit.entry_sleep.is_empty())
                .then(|| visit.edges.first().map(|edge| (index, edge.label)))
                .flatten()
        })
        .unwrap();
    witness.visits[index].entry_sleep = vec![label];
    assert!(matches!(
        check_witness(&model, &obligations, &witness, WORK),
        Err(WitnessDefect::Sleep { .. })
    ));
}

#[test]
fn a_donation_inside_a_component_is_rejected() {
    let model = handmade("ignoring");
    let (obligations, mut witness) = run(&model);
    // The loop L0/L1 is a two-visit component; one of its edges is not donated.
    let (visit, edge) = witness
        .visits
        .iter()
        .enumerate()
        .find_map(|(index, visit)| {
            visit
                .edges
                .iter()
                .position(|edge| {
                    !edge.donated
                        && witness.visits[edge.target]
                            .edges
                            .iter()
                            .any(|back| back.target == index)
                })
                .map(|edge| (index, edge))
        })
        .unwrap();
    witness.visits[visit].edges[edge].donated = true;
    assert!(matches!(
        check_witness(&model, &obligations, &witness, WORK),
        Err(WitnessDefect::DonatedInsideComponent { .. })
    ));
}

#[test]
fn a_cycle_without_a_full_visit_is_rejected() {
    let model = handmade("ignoring");
    let (obligations, mut witness) = run(&model);
    let visit = witness
        .visits
        .iter()
        .position(|visit| visit.proviso)
        .expect("the loop needs the proviso");
    witness.visits[visit].proviso = false;
    // The visit keeps the edges the proviso added, so coverage still holds; only
    // the cycle condition can reject it.
    assert_eq!(
        check_witness(&model, &obligations, &witness, WORK),
        Err(WitnessDefect::ReducedCycle)
    );
}

#[test]
fn a_missing_root_is_rejected() {
    let model = handmade("ignoring");
    let (obligations, mut witness) = run(&model);
    witness.roots.clear();
    assert!(matches!(
        check_witness(&model, &obligations, &witness, WORK),
        Err(WitnessDefect::Root { .. })
    ));
}

// ---------------------------------------------------------------------------
// cr-1wuzry: charge-before-work against a hostile model or witness
// ---------------------------------------------------------------------------

use super::check_witness_counted;
use continuum_model_core::expr::BoolExpr;
use continuum_model_core::model::{ActionDecl, ModelBuilder};

/// One variable and one always-true action with `outcomes` empty outcomes: the
/// highest label fan-out per unit of expression the model language allows.
fn fanout(outcomes: usize) -> Model {
    ModelBuilder::new()
        .variable("x", 0, 1)
        .action(ActionDecl::enumerated(
            "Wide",
            BoolExpr::constant(true),
            vec![Vec::new(); outcomes],
        ))
        .initial_state(&[("x", 0)])
        .build()
        .unwrap()
}

/// The least work that lets `check_witness` accept `witness`, by bisection; every
/// probe below it must be a typed `WorkExhausted` that touched no more items than it
/// paid for.
fn least_sufficient_work(
    model: &Model,
    obligations: &Obligations,
    witness: &ReductionWitness,
) -> u64 {
    let (mut low, mut high) = (0_u64, WORK);
    assert!(check_witness(model, obligations, witness, high).is_ok());
    while low < high {
        let mid = low + (high - low) / 2;
        let (result, touched) = check_witness_counted(model, obligations, witness, mid);
        assert!(touched <= mid, "work {mid}: touched {touched} items unpaid");
        match result {
            Ok(_) => high = mid,
            Err(WitnessDefect::WorkExhausted) => low = mid + 1,
            Err(other) => panic!("work {mid}: {other:?}"),
        }
    }
    low
}

#[test]
fn a_huge_outcome_fanout_is_refused_before_any_proportional_work() {
    // 200 000 empty outcomes of one constant-guarded action (the cr-1wuzry model):
    // under a budget far below the label count the checker refuses, typed, having
    // touched at most as many items as it paid for — never the outcome list.
    let model = fanout(200_000);
    let obligations = Obligations::new(DeadlockPolicy::Allowed);
    let (_, witness) = run(&fanout(1));
    for work in [0_u64, 1, 2, 10, 1_000, 100_000, 799_999] {
        let (result, touched) = check_witness_counted(&model, &obligations, &witness, work);
        assert_eq!(
            result.err(),
            Some(WitnessDefect::WorkExhausted),
            "work {work}"
        );
        assert!(
            touched <= work,
            "work {work}: touched {touched} items unpaid"
        );
        assert!(
            touched < 200_000,
            "work {work}: the outcome list was walked"
        );
    }
}

#[test]
fn the_exact_work_boundary_grows_with_the_outcome_count() {
    // Sweep the exact boundary across increasing fan-out: one unit less than the
    // least sufficient work is `WorkExhausted`, and the boundary grows at least
    // linearly with the outcome count, so no outcome is processed unpaid.
    let obligations = Obligations::new(DeadlockPolicy::Allowed);
    let mut previous = 0_u64;
    for outcomes in [1_usize, 4, 16, 64, 256, 1024, 20_000] {
        let model = fanout(outcomes);
        let report = crate::check(
            &model,
            &obligations,
            Bounds::new(1 << 16, 1 << 20, 1 << 16, WORK),
        )
        .unwrap();
        let least = least_sufficient_work(&model, &obligations, report.witness());
        let (below, touched) =
            check_witness_counted(&model, &obligations, report.witness(), least - 1);
        assert_eq!(
            below.err(),
            Some(WitnessDefect::WorkExhausted),
            "{outcomes} outcomes"
        );
        assert!(touched < least);
        assert!(
            least >= 4 * outcomes as u64,
            "{outcomes} outcomes: boundary {least}"
        );
        assert!(least > previous);
        previous = least;
    }
}

#[test]
fn a_huge_witness_list_is_refused_before_it_is_read() {
    // A valid witness, then one list inflated to a million entries: a stubborn set,
    // a visit's entry sleep set, and a visit's edge list. Under the work that
    // sufficed for the valid witness, each is refused typed, with no more items
    // touched than paid for.
    let model = handmade("independent");
    let (obligations, witness) = run(&model);
    let least = least_sufficient_work(&model, &obligations, &witness);
    let huge = 1_000_000;

    let mut stubborn = witness.clone();
    let node = stubborn
        .nodes
        .iter()
        .position(|node| matches!(node.expansion, Expansion::Reduced { .. }))
        .expect("the independent counters have a reduced state");
    if let Expansion::Reduced {
        seed,
        stubborn: set,
    } = &mut stubborn.nodes[node].expansion
    {
        *set = vec![*seed; huge];
    }
    let mut sleep = witness.clone();
    let label = sleep.visits[0].edges[0].label;
    sleep.visits[0].entry_sleep = vec![label; huge];
    let mut edges = witness.clone();
    let edge = edges.visits[0].edges[0];
    edges.visits[0].edges = vec![edge; huge];

    for (name, tampered) in [("stubborn", stubborn), ("sleep", sleep), ("edges", edges)] {
        let (result, touched) = check_witness_counted(&model, &obligations, &tampered, least);
        assert_eq!(result.err(), Some(WitnessDefect::WorkExhausted), "{name}");
        assert!(touched <= least, "{name}: touched {touched} items unpaid");
        assert!(touched < huge as u64, "{name}: the inflated list was read");
    }
}

#[test]
fn no_accepted_witness_touches_more_than_it_paid() {
    for (id, model) in handmade::all() {
        let (obligations, witness) = run(&model);
        if !witness.is_complete() {
            continue;
        }
        let (result, touched) = check_witness_counted(&model, &obligations, &witness, WORK);
        assert!(result.is_ok(), "{id}");
        let least = least_sufficient_work(&model, &obligations, &witness);
        assert!(touched <= least, "{id}: touched {touched}, paid {least}");
    }
}

#[test]
fn an_out_of_range_edge_label_is_dangling_under_any_budget() {
    // A structural defect is typed as one, not as budget exhaustion: the edge label
    // is range-checked before anything is charged for firing it.
    let model = handmade("independent");
    let (obligations, mut witness) = run(&model);
    let least = least_sufficient_work(&model, &obligations, &witness);
    witness.visits[0].edges[0].label = witness.labels.len() + 5;
    for work in [least, WORK, u64::MAX] {
        assert_eq!(
            check_witness(&model, &obligations, &witness, work).err(),
            Some(WitnessDefect::Dangling { what: "edge label" }),
            "work {work}"
        );
    }
}

// ---------------------------------------------------------------------------
// cr-1wuzry round 2: the two checker collections, paid per site
// ---------------------------------------------------------------------------

use super::check_witness_omitting;

fn panics_unpaid(run: impl FnOnce()) -> bool {
    let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(run));
    match caught {
        Ok(()) => false,
        Err(payload) => payload
            .downcast_ref::<String>()
            .is_some_and(|message| message.contains("unpaid")),
    }
}

#[test]
fn the_stubborn_table_is_paid_before_it_is_built_even_for_all_full_witnesses() {
    // The philosophers' witness has full expansions (the uncharged arm before round 2).
    // With the site's charge left out — the 4c281661 code — the per-site credit check
    // fires; with it, the witness is accepted, and one unit short is refused typed.
    let model = handmade("philosophers");
    let (obligations, witness) = run(&model);
    assert!(
        witness
            .nodes
            .iter()
            .any(|node| matches!(node.expansion, Expansion::Full(_)))
    );
    assert!(panics_unpaid(|| {
        let _ = check_witness_omitting(&model, &obligations, &witness, WORK, "stubborn-slots");
    }));
    assert!(check_witness_omitting(&model, &obligations, &witness, WORK, "").is_ok());
    let least = least_sufficient_work(&model, &obligations, &witness);
    assert_eq!(
        check_witness(&model, &obligations, &witness, least - 1).err(),
        Some(WitnessDefect::WorkExhausted)
    );
}

#[test]
fn every_donation_search_is_paid_before_the_insert() {
    // The first generated model whose witness donates at least twice on one visit.
    let (model, obligations, witness) = (0..500)
        .map(|seed| {
            let model = corpus::generate(seed).model;
            let (obligations, witness) = run(&model);
            (model, obligations, witness)
        })
        .find(|(_, _, witness)| {
            witness.is_complete()
                && witness
                    .visits
                    .iter()
                    .any(|visit| visit.edges.iter().filter(|edge| edge.donated).count() >= 2)
        })
        .expect("some generated witness donates twice on one visit");
    assert!(panics_unpaid(|| {
        let _ = check_witness_omitting(&model, &obligations, &witness, WORK, "donation-search");
    }));
    assert!(check_witness_omitting(&model, &obligations, &witness, WORK, "").is_ok());
}
