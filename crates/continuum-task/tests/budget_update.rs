//! `task.update_budget`, mid-flight: the legality table as a table.
//!
//! > `task.update_budget` adjusts the budget of a running task. Raising a dimension
//! > extends the current run. Lowering a dimension below committed spend triggers
//! > suspension-with-continuation (plan B18): the task transitions to `Suspended` with
//! > committed partial evidence plus a valid continuation. Silent truncation of a campaign
//! > is prohibited (INV-009).
//! >
//! > — `notes/plan/schemas/continuumd-native-protocol.idl`, `rule task.update_budget`
//!
//! # Lowering is admitted, and this file is where that is stated as behaviour
//!
//! The rule does not forbid lowering; it *defines* it, and the definition is a suspension
//! rather than a refusal. A ledger that refused every lowering would make the operation
//! unable to express the case the rule spends its second sentence on, and would push
//! callers toward the silent truncation INV-009 prohibits. So five outcomes, and
//! [`the_update_legality_table_holds_in_every_row`] walks all of them as data.
//!
//! The one reading this file fixes is what "committed spend" means. It is taken as
//! *recorded, non-refundable spend* — [`BudgetLedger::spend`] — rather than the spend at the
//! last checkpoint. Under the other reading a task that had spent 40 states and last
//! committed at 10 would accept a new ceiling of 20, which is a bound the run has already
//! passed; admitting it would make the next charge exhaust against a ceiling that was never
//! true, which is retroactive truncation by another name.
//! [`lowering_between_the_checkpoint_and_the_live_spend_still_parks`] pins the distinction.

use continuum_task::budget::dimension::{Ceiling, CostDimension, MeterSet};
use continuum_task::budget::{Budget, BudgetLedger, UpdateOutcome};

fn metered(limit: u64) -> BudgetLedger {
    BudgetLedger::new(
        Budget::unbounded().with(CostDimension::States, limit),
        MeterSet::STATES_ONLY,
    )
}

fn spent(limit: u64, spend: u64) -> BudgetLedger {
    let mut ledger = metered(limit);
    ledger
        .charge(CostDimension::States, spend)
        .expect("metered");
    ledger
}

#[test]
fn the_update_legality_table_holds_in_every_row() {
    // (row, ceiling before, spend, ceiling after, expected token)
    let rows: &[(&str, Ceiling, u64, Ceiling, &str)] = &[
        (
            "raise a declared ceiling",
            Ceiling::At(100),
            40,
            Ceiling::At(200),
            "raised",
        ),
        (
            "remove a declared ceiling",
            Ceiling::At(100),
            40,
            Ceiling::Unbounded,
            "raised",
        ),
        (
            "re-send the same ceiling",
            Ceiling::At(100),
            40,
            Ceiling::At(100),
            "unchanged",
        ),
        (
            "re-send no ceiling",
            Ceiling::Unbounded,
            0,
            Ceiling::Unbounded,
            "unchanged",
        ),
        (
            "lower to exactly the committed spend",
            Ceiling::At(100),
            40,
            Ceiling::At(40),
            "tightened",
        ),
        (
            "lower, still above the committed spend",
            Ceiling::At(100),
            40,
            Ceiling::At(60),
            "tightened",
        ),
        (
            "introduce a ceiling above the committed spend",
            Ceiling::Unbounded,
            40,
            Ceiling::At(60),
            "tightened",
        ),
        (
            "lower below the committed spend",
            Ceiling::At(100),
            40,
            Ceiling::At(39),
            "suspend",
        ),
        (
            "introduce a ceiling below the committed spend",
            Ceiling::Unbounded,
            40,
            Ceiling::At(1),
            "suspend",
        ),
    ];

    for (row, before, spend, after, expected) in rows {
        let budget = match before {
            Ceiling::Unbounded => Budget::unbounded(),
            Ceiling::At(limit) => Budget::unbounded().with(CostDimension::States, *limit),
        };
        let mut ledger = BudgetLedger::new(budget, MeterSet::STATES_ONLY);
        ledger
            .charge(CostDimension::States, *spend)
            .expect("metered");
        let outcome = ledger.update(CostDimension::States, *after);
        assert_eq!(
            outcome.token(),
            *expected,
            "row {row:?}: {}",
            outcome.render()
        );
        assert_eq!(
            ledger.budget().ceiling(CostDimension::States),
            *after,
            "row {row:?}: the new ceiling is recorded on every arm, because \
             TaskRecord.budget reports the budget in force"
        );
        assert_eq!(
            ledger.spend().measured(CostDimension::States),
            Some(*spend),
            "row {row:?}: an update never rewrites recorded spend"
        );
    }
}

#[test]
fn lowering_below_committed_spend_parks_with_the_committed_evidence() {
    let mut ledger = metered(100);
    ledger.charge(CostDimension::States, 30).expect("metered");
    ledger.checkpoint(2).expect("nothing reserved");
    ledger.charge(CostDimension::States, 10).expect("metered");

    let outcome = ledger.update(CostDimension::States, Ceiling::At(5));
    let suspension = outcome
        .suspension()
        .expect("lowering below committed spend parks");
    assert_eq!(suspension.dimension(), CostDimension::States);
    assert_eq!(suspension.ceiling(), 5);
    assert_eq!(suspension.spent(), 40);
    assert_eq!(
        suspension.committed(),
        2,
        "it parks with committed partial evidence, not without it (B18)"
    );
    assert_eq!(
        suspension
            .checkpoint()
            .and_then(|checkpoint| checkpoint.spend().measured(CostDimension::States)),
        Some(30),
        "the continuation resumes from the frontier the checkpoint priced"
    );
}

#[test]
fn lowering_between_the_checkpoint_and_the_live_spend_still_parks() {
    let mut ledger = metered(100);
    ledger.charge(CostDimension::States, 10).expect("metered");
    ledger.checkpoint(1).expect("nothing reserved");
    ledger.charge(CostDimension::States, 30).expect("metered");

    // 20 is above the checkpoint's spend (10) and below the live spend (40).
    let outcome = ledger.update(CostDimension::States, Ceiling::At(20));
    assert_eq!(
        outcome.token(),
        "suspend",
        "\"committed spend\" is recorded spend; a ceiling the run has already passed \
         cannot be admitted as an enforceable bound"
    );
    assert_eq!(outcome.suspension().map(|s| s.spent()), Some(40));
}

#[test]
fn a_task_that_committed_nothing_still_parks_with_a_named_zero() {
    let mut ledger = spent(100, 40);
    let outcome = ledger.update(CostDimension::States, Ceiling::At(1));
    let suspension = outcome.suspension().expect("parks");
    assert_eq!(suspension.checkpoint(), None);
    assert_eq!(
        suspension.committed(),
        0,
        "nothing committed is a representable, named outcome — never an absent field"
    );
}

#[test]
fn an_unmetered_dimension_records_the_ceiling_and_stays_omitted() {
    let mut ledger = metered(100);
    for (before, after) in [
        (Ceiling::Unbounded, Ceiling::At(5_000)),
        (Ceiling::At(5_000), Ceiling::At(1)),
        (Ceiling::At(1), Ceiling::Unbounded),
    ] {
        assert_eq!(ledger.budget().ceiling(CostDimension::WallMs), before);
        let outcome = ledger.update(CostDimension::WallMs, after);
        assert_eq!(
            outcome.token(),
            "unenforced",
            "without a meter there is no spend to compare a ceiling against, so no \
             raise/lower claim is made"
        );
        assert_eq!(ledger.budget().ceiling(CostDimension::WallMs), after);
    }
    // The ceiling is back to `Unbounded`, so nothing is declared and nothing is omitted.
    assert!(ledger.omissions().is_empty());
    ledger.update(CostDimension::WallMs, Ceiling::At(7));
    assert_eq!(
        ledger
            .omissions()
            .iter()
            .map(|o| o.subject())
            .collect::<Vec<_>>(),
        vec!["budget.wall_ms".to_owned()],
        "the recorded ceiling is unenforceable, and says so"
    );
}

#[test]
fn a_raise_extends_the_run_that_a_ceiling_had_stopped() {
    let mut ledger = spent(40, 40);
    assert!(
        ledger
            .charge(CostDimension::States, 1)
            .expect("metered")
            .exhaustion()
            .is_some()
    );
    assert!(matches!(
        ledger.update(CostDimension::States, Ceiling::At(80)),
        UpdateOutcome::Raised { .. }
    ));
    assert!(
        ledger
            .charge(CostDimension::States, 40)
            .expect("metered")
            .is_admitted(),
        "raising a dimension extends the current run"
    );
    assert_eq!(ledger.spend().measured(CostDimension::States), Some(80));
    assert!(
        ledger.exhaustion().is_some(),
        "the ledger still remembers which ceiling stopped it first; a raise does not \
         rewrite the history that produced the continuation"
    );
}

#[test]
fn a_tightened_ceiling_binds_the_next_charge_and_not_the_last_one() {
    let mut ledger = spent(100, 40);
    assert!(matches!(
        ledger.update(CostDimension::States, Ceiling::At(50)),
        UpdateOutcome::Tightened { .. }
    ));
    assert_eq!(ledger.headroom(CostDimension::States), Some(10));
    assert!(
        ledger
            .charge(CostDimension::States, 11)
            .expect("metered")
            .exhaustion()
            .is_some()
    );
    assert_eq!(
        ledger.spend().measured(CostDimension::States),
        Some(40),
        "nothing already spent was invalidated by the tightening"
    );
}

#[test]
fn updating_every_dimension_is_legal_and_leaves_one_outcome_each() {
    let mut ledger = BudgetLedger::new(Budget::unbounded(), MeterSet::all());
    for dimension in CostDimension::ALL {
        let outcome = ledger.update(dimension, Ceiling::At(10));
        assert_eq!(outcome.dimension(), dimension);
        assert_eq!(
            outcome.token(),
            "tightened",
            "{dimension}: introducing a ceiling above zero spend tightens"
        );
    }
    assert_eq!(ledger.enforced().len(), 9);
    assert!(ledger.omissions().is_empty());
}
