//! bn-j1a50: the obligations family's ledger lives in the region calculus's own ledger.
//!
//! RFC 0026 correction 51 item 3 gives the calculus one typed entry point for adapter
//! obligations (`open_substrate`, `discharge_substrate`, `transfer_substrate`). The lift
//! now opens, discharges and transfers every journaled obligation through it, so the
//! ledger that `Finalization::is_total` reads counts them. These tests hold the two
//! accounts to each other on real substrate journals.
//!
//! # Evidence map
//!
//! | Claim | Test |
//! |---|---|
//! | after every conforming run, the calculus holds exactly the obligations the journal left open, with the journal's holder | [`the_calculus_ledger_holds_exactly_the_journals_open_obligations`] |
//! | a region whose tasks open, hand over and discharge substrate obligations finalizes totally | [`a_region_with_substrate_obligations_finalizes_totally`] |

use std::collections::BTreeMap;

use continuum_asupersync::binding::{BindingConfig, Program, SubstrateOp, run};
use continuum_asupersync::choice::ChoiceLog;
use continuum_asupersync::family::effect::ReservationLabel;
use continuum_asupersync::family::lifecycle::{RegionLabel, TaskLabel};
use continuum_asupersync::family::obligation::{ObligationEvent, ObligationKind};
use continuum_asupersync::family::{EventBody, Family};
use continuum_asupersync::journal::Journal;
use continuum_asupersync::lift::{LiftVerdict, lift};
use continuum_task::region::obligation::SubstrateId;
use continuum_task::region::worker::{Resumability, WorkerId};

fn config() -> BindingConfig {
    BindingConfig::new(0)
        .observing(Family::Effect)
        .observing(Family::Cancellation)
        .observing(Family::Obligation)
        .observing(Family::Time)
}

fn spawn(region: RegionLabel, task: TaskLabel) -> SubstrateOp {
    SubstrateOp::Spawn {
        region,
        task,
        resumability: Resumability::Resumable,
    }
}

const fn acquire(task: TaskLabel, label: u32, kind: ObligationKind) -> SubstrateOp {
    SubstrateOp::Acquire {
        task,
        reservation: ReservationLabel(label),
        kind,
    }
}

const fn commit(label: u32) -> SubstrateOp {
    SubstrateOp::Commit {
        reservation: ReservationLabel(label),
    }
}

/// Two actors. A root task that holds a lease it never discharges and commits a send
/// permit. A region whose tasks hold an ack and an I/O obligation, hand the I/O
/// obligation over, and are cancelled: the cancellation aborts what they hold.
fn programs() -> Vec<Program> {
    let (a, b, c) = (TaskLabel(1), TaskLabel(2), TaskLabel(3));
    let r1 = RegionLabel(1);
    vec![
        vec![
            spawn(RegionLabel::ROOT, a),
            SubstrateOp::Begin { task: a },
            acquire(a, 1, ObligationKind::Lease),
            acquire(a, 2, ObligationKind::SendPermit),
            commit(2),
        ],
        vec![
            SubstrateOp::OpenRegion {
                parent: RegionLabel::ROOT,
                child: r1,
            },
            spawn(r1, b),
            spawn(r1, c),
            SubstrateOp::Begin { task: b },
            SubstrateOp::Begin { task: c },
            acquire(b, 3, ObligationKind::Ack),
            acquire(c, 4, ObligationKind::IoOp),
            SubstrateOp::Transfer {
                reservation: ReservationLabel(4),
                to: b,
            },
            SubstrateOp::Cancel { region: r1 },
        ],
    ]
}

/// The journal's own account: each obligation's holder while open, `None` once
/// discharged or leaked.
fn journal_ledger(journal: &Journal) -> BTreeMap<u32, Option<u32>> {
    let mut out = BTreeMap::new();
    for event in journal.events() {
        if let EventBody::Obligation(event) = event.body() {
            match event {
                ObligationEvent::Opened {
                    obligation, holder, ..
                }
                | ObligationEvent::Transferred {
                    obligation, holder, ..
                } => {
                    out.insert(obligation.0, Some(holder.0));
                }
                ObligationEvent::Discharged { obligation, .. }
                | ObligationEvent::Leaked { obligation } => {
                    out.insert(obligation.0, None);
                }
                // A fence stays owed, as the calculus's ledger keeps it (bn-20d8u).
                ObligationEvent::RegionSettled { .. } | ObligationEvent::Fenced { .. } => {}
            }
        }
    }
    out
}

#[test]
fn the_calculus_ledger_holds_exactly_the_journals_open_obligations() {
    let programs = programs();
    let lengths: Vec<usize> = programs.iter().map(Vec::len).collect();
    let logs = ChoiceLog::enumerate(&lengths);
    assert_eq!(logs.len(), 2_002, "14! / (5!·9!)");
    let mut still_open = 0;
    for log in &logs {
        let journal = run(&programs, log, &config()).unwrap();
        let LiftVerdict::Conforms(lifted) = lift(&journal) else {
            panic!("log {log}: {:?}\n{}", lift(&journal), journal.render());
        };
        let ledger = journal_ledger(&journal);
        assert_eq!(ledger.len(), 4, "log {log}");
        for (obligation, holder) in ledger {
            let calculus = lifted
                .tree()
                .substrate_holder(SubstrateId::at(u64::from(obligation)));
            assert_eq!(
                calculus,
                holder.map(WorkerId::at),
                "log {log}: o{obligation}\n{}",
                journal.render()
            );
            still_open += usize::from(holder.is_some());
        }
    }
    // Non-vacuity: the root task's lease is open at the end of every run.
    assert_eq!(still_open, logs.len());
}

#[test]
fn a_region_with_substrate_obligations_finalizes_totally() {
    let (a, b) = (TaskLabel(1), TaskLabel(2));
    let r1 = RegionLabel(1);
    let programs = vec![vec![
        SubstrateOp::OpenRegion {
            parent: RegionLabel::ROOT,
            child: r1,
        },
        spawn(r1, a),
        spawn(r1, b),
        SubstrateOp::Begin { task: a },
        SubstrateOp::Begin { task: b },
        acquire(a, 1, ObligationKind::Lease),
        SubstrateOp::Transfer {
            reservation: ReservationLabel(1),
            to: b,
        },
        commit(1),
        SubstrateOp::Finish { task: a },
        SubstrateOp::Finish { task: b },
        SubstrateOp::Close { region: r1 },
    ]];
    let journal = run(&programs, &ChoiceLog::new([0; 11]), &config()).unwrap();
    let LiftVerdict::Conforms(lifted) = lift(&journal) else {
        panic!("{:?}\n{}", lift(&journal), journal.render());
    };
    let [(_, finalization)] = lifted.finalizations() else {
        panic!("one finalization expected\n{}", journal.render());
    };
    // The region with all the work in it finalized totally.
    assert!(finalization.is_total(), "{}", finalization.ledger());
    assert_eq!(lifted.tree().substrate_holder(SubstrateId::at(0)), None);
    // The lease was opened and discharged in the calculus's own ledger: the same run,
    // observed without the obligations family, opens exactly one ledger entry fewer.
    let without = BindingConfig::new(0)
        .observing(Family::Effect)
        .observing(Family::Cancellation)
        .observing(Family::Time);
    let projection = run(&programs, &ChoiceLog::new([0; 11]), &without).unwrap();
    let LiftVerdict::Conforms(projected) = lift(&projection) else {
        panic!("{:?}", lift(&projection));
    };
    let [(_, bare)] = projected.finalizations() else {
        panic!("one finalization expected");
    };
    assert!(bare.is_total());
    assert_eq!(finalization.ledger().opened(), bare.ledger().opened() + 1);
    assert_eq!(
        finalization.ledger().discharged(),
        bare.ledger().discharged() + 1
    );
}
