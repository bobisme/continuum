//! PR 18 (bn-3km4z): the generic reduction passes of `continuum_debugger::reduce`, over
//! synthetic traces whose cores are known in advance. The instantiation on real failing
//! journals (the replicated register's M01) is `continuum-asupersync`'s
//! `tests/pr18_impl01_reduction.rs`.

use std::collections::BTreeSet;

use continuum_debugger::reduce::{
    self, Budget, CausalOrder, Deletion, Guarantee, NotReplayable, OrderError, Pass, PassEnd,
    Reduction, Refusal, Replayed,
};
use continuum_value::assurance::InconclusiveReason;

const BUDGET: Budget = Budget::new(100_000, 1 << 30);

/// A deterministic generator (SplitMix64): no ambient randomness (INV-005).
struct SplitMix(u64);

impl SplitMix {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }
    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
}

/// A random order over `n` events: each event gets up to three earlier predecessors.
fn random_order(rng: &mut SplitMix, n: usize) -> Vec<Vec<usize>> {
    (0..n)
        .map(|i| {
            if i == 0 {
                return Vec::new();
            }
            let k = rng.below(4);
            (0..k).map(|_| rng.below(i as u64) as usize).collect()
        })
        .collect()
}

/// An oracle that fails exactly when the replayed set holds every event of `needed`,
/// and reports them as witnesses; a set that is not a configuration of `order` is not
/// replayable, as a real semantics would refuse it.
fn needs<'a>(
    order: &'a CausalOrder,
    needed: &'a [usize],
    calls: &'a mut u64,
) -> impl FnMut(&[usize]) -> Replayed + 'a {
    move |kept: &[usize]| {
        *calls += 1;
        if !order.is_down_closed(kept) {
            return Replayed::NotReplayable(NotReplayable::Nonconforming("not a run".into()));
        }
        if needed.iter().all(|n| kept.binary_search(n).is_ok()) {
            Replayed::Fails {
                witnesses: needed.to_vec(),
            }
        } else {
            Replayed::Holds
        }
    }
}

/// The reference: every event with a path to a seed, by brute force over all pairs
/// (Warshall).
#[allow(clippy::needless_range_loop)]
fn brute_downset(preds: &[Vec<usize>], seeds: &[usize]) -> Vec<usize> {
    let n = preds.len();
    let mut reach = vec![vec![false; n]; n];
    for (b, ps) in preds.iter().enumerate() {
        for &a in ps {
            reach[a][b] = true;
        }
    }
    for k in 0..n {
        for i in 0..n {
            if reach[i][k] {
                for j in 0..n {
                    if reach[k][j] {
                        reach[i][j] = true;
                    }
                }
            }
        }
    }
    (0..n)
        .filter(|&e| seeds.contains(&e) || seeds.iter().any(|&s| reach[e][s]))
        .collect()
}

/// Closure keeps exactly the happens-before past of the witnesses: checked against a
/// brute-force transitive closure over 300 seeded random orders.
#[test]
fn closure_keeps_exactly_the_witnesses_past_on_random_orders() {
    let mut rng = SplitMix(18);
    for _ in 0..300 {
        let n = 1 + rng.below(40) as usize;
        let preds = random_order(&mut rng, n);
        let order = CausalOrder::from_predecessors(preds.clone()).expect("an order");
        let needed: Vec<usize> = {
            let mut v: Vec<usize> = (0..1 + rng.below(3))
                .map(|_| rng.below(n as u64) as usize)
                .collect();
            v.sort_unstable();
            v.dedup();
            v
        };
        let mut calls = 0;
        let mut oracle = needs(&order, &needed, &mut calls);
        let r = reduce::closure_pass(&order, &mut oracle, BUDGET);
        let core = r.core().expect("reduced");
        assert_eq!(core.events, brute_downset(&preds, &needed));
        assert!(order.is_down_closed(&core.events));
        assert!(core.guarantees.contains(&Guarantee::CausallyClosed));
    }
}

/// Deletion over configurations finds the unique smallest failing configuration when
/// failure is monotone, and its core is 1-minimal: removing any kept event with its
/// future no longer fails, checked exhaustively.
#[test]
fn deletion_over_configurations_is_one_minimal_on_random_orders() {
    let mut rng = SplitMix(1818);
    for _ in 0..200 {
        let n = 1 + rng.below(30) as usize;
        let preds = random_order(&mut rng, n);
        let order = CausalOrder::from_predecessors(preds.clone()).expect("an order");
        let needed = vec![rng.below(n as u64) as usize];
        let mut calls = 0;
        let whole: Vec<usize> = (0..n).collect();
        let r = {
            let mut oracle = needs(&order, &needed, &mut calls);
            reduce::deletion_pass(&order, whole, Deletion::Configurations, &mut oracle, BUDGET)
        };
        let core = r.core().expect("reduced").clone();
        assert_eq!(core.events, brute_downset(&preds, &needed));
        assert!(core.guarantees.contains(&Guarantee::CausallyMinimal));
        for &e in &core.events {
            let future: BTreeSet<usize> = brute_future(&preds, e);
            let smaller: Vec<usize> = core
                .events
                .iter()
                .copied()
                .filter(|x| !future.contains(x))
                .collect();
            let mut probe = needs(&order, &needed, &mut calls);
            assert!(!matches!(probe(&smaller), Replayed::Fails { .. }));
        }
    }
}

fn brute_future(preds: &[Vec<usize>], e: usize) -> BTreeSet<usize> {
    let mut out = BTreeSet::from([e]);
    for (i, ps) in preds.iter().enumerate() {
        if ps.iter().any(|p| out.contains(p)) {
            out.insert(i);
        }
    }
    out
}

/// Deletion over atoms is classic `ddmin`: with no order, a failure that needs three
/// events among forty is reduced to exactly those three.
#[test]
fn deletion_over_atoms_finds_the_needed_events() {
    let order = CausalOrder::from_predecessors(vec![Vec::new(); 40]).expect("an order");
    let needed = [3, 17, 31];
    let mut oracle = |kept: &[usize]| {
        if needed.iter().all(|n| kept.contains(n)) {
            Replayed::Fails {
                witnesses: vec![31],
            }
        } else {
            Replayed::Holds
        }
    };
    let r = reduce::deletion_pass(
        &order,
        (0..40).collect(),
        Deletion::Atoms,
        &mut oracle,
        BUDGET,
    );
    let core = r.core().expect("reduced");
    assert_eq!(core.events, needed);
    assert!(core.guarantees.contains(&Guarantee::OneMinimal));
    assert!(!core.guarantees.contains(&Guarantee::CausallyMinimal));
}

/// Deletion over atoms whose core keeps a multi-event atom claims only
/// `AtomMinimal`: single events inside the atom were never removed alone.
#[test]
fn deletion_over_multi_event_atoms_claims_only_atom_minimality() {
    let order =
        CausalOrder::with_atoms(vec![Vec::new(); 10], vec![vec![4, 5, 6]]).expect("an order");
    let mut oracle = |kept: &[usize]| {
        if kept.contains(&5) {
            Replayed::Fails { witnesses: vec![5] }
        } else {
            Replayed::Holds
        }
    };
    let r = reduce::deletion_pass(
        &order,
        (0..10).collect(),
        Deletion::Atoms,
        &mut oracle,
        BUDGET,
    );
    let core = r.core().expect("reduced");
    assert_eq!(core.events, vec![4, 5, 6]);
    assert!(core.guarantees.contains(&Guarantee::AtomMinimal));
    assert!(!core.guarantees.contains(&Guarantee::OneMinimal));
}

/// INV-008: a final removal the oracle cannot decide is not a removal that did not
/// fail. The pass ends `Undecided` and the core claims no minimality.
#[test]
fn an_undecided_removal_withholds_minimality() {
    let order = CausalOrder::from_predecessors(vec![Vec::new(); 8]).expect("an order");
    // Fails while 2 and 6 are both kept; removing 6 alone is inconclusive.
    let mut oracle = |kept: &[usize]| {
        if !kept.contains(&6) && kept.contains(&2) && kept.len() == 1 {
            return Replayed::NotReplayable(NotReplayable::Inconclusive(
                InconclusiveReason::InsufficientTelemetry,
                "cannot tell".into(),
            ));
        }
        if kept.contains(&2) && kept.contains(&6) {
            Replayed::Fails { witnesses: vec![6] }
        } else {
            Replayed::Holds
        }
    };
    let r = reduce::deletion_pass(
        &order,
        (0..8).collect(),
        Deletion::Atoms,
        &mut oracle,
        BUDGET,
    );
    let Reduction::Reduced {
        core, transcript, ..
    } = r
    else {
        panic!("reduced: {r:?}");
    };
    assert_eq!(core.events, vec![2, 6]);
    assert_eq!(
        transcript.last().map(|p| &p.end),
        Some(&PassEnd::Undecided { removals: 1 })
    );
    assert!(core.guarantees.iter().all(|g| !matches!(
        g,
        Guarantee::OneMinimal | Guarantee::AtomMinimal | Guarantee::CausallyMinimal
    )));
}

/// An oracle that says a candidate fails but names no witness inside it breaks its
/// contract mid-pass: the reduction stops, inconclusive with `EngineError`, and keeps
/// the best set validated before the breach.
#[test]
fn a_witness_contract_breach_mid_pass_stops_the_reduction() {
    let order = CausalOrder::from_predecessors(vec![Vec::new(); 8]).expect("an order");
    let mut oracle = |kept: &[usize]| {
        if kept.len() == 8 {
            Replayed::Fails { witnesses: vec![7] }
        } else {
            Replayed::Fails { witnesses: vec![] }
        }
    };
    let r = reduce::deletion_pass(
        &order,
        (0..8).collect(),
        Deletion::Atoms,
        &mut oracle,
        BUDGET,
    );
    let Reduction::Inconclusive {
        reason,
        best,
        transcript,
        ..
    } = r
    else {
        panic!("inconclusive: {r:?}");
    };
    assert_eq!(reason, InconclusiveReason::EngineError);
    assert_eq!(best.expect("the start").events, (0..8).collect::<Vec<_>>());
    assert_eq!(
        transcript.last().map(|p| &p.end),
        Some(&PassEnd::OracleContract)
    );
}

/// Atoms stand or fall together: the closure of one member holds the whole atom and its
/// past, and a configuration missing an atom-mate is not a configuration.
#[test]
fn atoms_stand_or_fall_together() {
    // 0 → 1, 2 → 3; atom {1, 4}.
    let preds = vec![vec![], vec![0], vec![], vec![2], vec![]];
    let order = CausalOrder::with_atoms(preds, vec![vec![4, 1]]).expect("an order");
    assert_eq!(order.down_closure(&[4]), vec![0, 1, 4]);
    assert_eq!(order.atom(1), vec![1, 4]);
    assert!(!order.is_down_closed(&[0, 1]));
    assert!(order.is_down_closed(&[0, 1, 4]));
    let mut calls = 0;
    let needed = [4];
    let mut oracle = needs(&order, &needed, &mut calls);
    let r = reduce::minimize(&order, Deletion::Configurations, &mut oracle, BUDGET);
    assert_eq!(r.core().expect("reduced").events, vec![0, 1, 4]);
    assert_eq!(
        CausalOrder::with_atoms(vec![vec![]; 3], vec![vec![0, 1], vec![1, 2]]),
        Err(OrderError::AtomsOverlap { event: 1 })
    );
    assert_eq!(
        CausalOrder::with_atoms(vec![vec![]; 3], vec![vec![0, 9]]),
        Err(OrderError::AtomOutOfRange { event: 9 })
    );
}

/// A closure that does not replay to the failure is a typed pass outcome: the pass keeps
/// its input, and the deletion pass still runs from it.
#[test]
fn a_closure_that_does_not_replay_is_typed_and_kept_out() {
    // 0, 1, 2 independent; the failure is witnessed at 2 but needs 0 too, which the
    // declared order does not relate to 2: the order under-approximates.
    let order = CausalOrder::from_predecessors(vec![vec![], vec![], vec![]]).expect("an order");
    let mut oracle = |kept: &[usize]| {
        if kept.contains(&0) && kept.contains(&2) {
            Replayed::Fails { witnesses: vec![2] }
        } else {
            Replayed::Holds
        }
    };
    let r = reduce::minimize(&order, Deletion::Configurations, &mut oracle, BUDGET);
    let Reduction::Reduced {
        core, transcript, ..
    } = r
    else {
        panic!("reduced: {r:?}");
    };
    assert_eq!(transcript[0].pass, Pass::Closure);
    assert_eq!(
        transcript[0].end,
        PassEnd::ClosureNotReplayPreserving(Replayed::Holds)
    );
    assert_eq!(core.events, vec![0, 2]);
}

/// INV-008: a budget that runs out is inconclusive with the best replay-validated set,
/// the replay is charged before it runs, and no minimality is claimed.
#[test]
fn an_exhausted_budget_is_typed_and_charged_first() {
    let order = CausalOrder::from_predecessors(vec![Vec::new(); 64]).expect("an order");
    let mut calls = 0_u64;
    let mut oracle = |kept: &[usize]| {
        calls += 1;
        if kept.contains(&63) {
            Replayed::Fails {
                witnesses: vec![63],
            }
        } else {
            Replayed::Holds
        }
    };
    let r = reduce::deletion_pass(
        &order,
        (0..64).collect(),
        Deletion::Atoms,
        &mut oracle,
        Budget::new(3, 1 << 30),
    );
    let Reduction::Inconclusive {
        reason,
        best,
        spent,
        ..
    } = r
    else {
        panic!("inconclusive: {r:?}");
    };
    assert_eq!(reason, InconclusiveReason::ResourceExhausted);
    assert_eq!(spent.replays, 3);
    assert_eq!(calls, 3);
    let best = best.expect("a validated set");
    assert!(best.events.contains(&63));
    assert!(!best.guarantees.contains(&Guarantee::OneMinimal));
    // Work units run out too, before a candidate is built.
    let mut oracle = |_: &[usize]| Replayed::Fails { witnesses: vec![0] };
    let r = reduce::deletion_pass(
        &order,
        (0..64).collect(),
        Deletion::Atoms,
        &mut oracle,
        Budget::new(1_000, 10),
    );
    assert!(matches!(r, Reduction::Inconclusive { .. }), "{r:?}");
}

/// Refusals: an empty trace, an input that holds, one that cannot be replayed, a start
/// that is not a configuration, and an oracle that breaks the witness contract.
#[test]
fn refusals_are_typed() {
    let empty = CausalOrder::from_predecessors(Vec::new()).expect("an order");
    let mut fails = |_: &[usize]| Replayed::Fails { witnesses: vec![0] };
    assert_eq!(
        reduce::closure_pass(&empty, &mut fails, BUDGET),
        Reduction::Refused(Refusal::EmptyTrace)
    );
    let order = CausalOrder::from_predecessors(vec![vec![], vec![0]]).expect("an order");
    let mut holds = |_: &[usize]| Replayed::Holds;
    assert_eq!(
        reduce::closure_pass(&order, &mut holds, BUDGET),
        Reduction::Refused(Refusal::InputHolds)
    );
    let mut refuses =
        |_: &[usize]| Replayed::NotReplayable(NotReplayable::Nonconforming("no".into()));
    assert!(matches!(
        reduce::closure_pass(&order, &mut refuses, BUDGET),
        Reduction::Refused(Refusal::InputNotReplayable(_))
    ));
    assert_eq!(
        reduce::deletion_pass(
            &order,
            vec![1],
            Deletion::Configurations,
            &mut fails,
            BUDGET
        ),
        Reduction::Refused(Refusal::NotAConfiguration)
    );
    assert_eq!(
        reduce::deletion_pass(&order, Vec::new(), Deletion::Atoms, &mut fails, BUDGET),
        Reduction::Refused(Refusal::EmptyStart)
    );
    let atoms = CausalOrder::with_atoms(vec![vec![]; 3], vec![vec![0, 2]]).expect("an order");
    assert_eq!(
        reduce::deletion_pass(&atoms, vec![0, 1], Deletion::Atoms, &mut fails, BUDGET),
        Reduction::Refused(Refusal::NotAConfiguration)
    );
    let mut outside = |_: &[usize]| Replayed::Fails { witnesses: vec![7] };
    assert_eq!(
        reduce::closure_pass(&order, &mut outside, BUDGET),
        Reduction::Refused(Refusal::WitnessContract)
    );
    let mut none = |_: &[usize]| Replayed::Fails { witnesses: vec![] };
    assert_eq!(
        reduce::closure_pass(&order, &mut none, BUDGET),
        Reduction::Refused(Refusal::WitnessContract)
    );
    assert_eq!(
        CausalOrder::from_predecessors(vec![vec![], vec![1]]),
        Err(OrderError::PredecessorNotEarlier {
            event: 1,
            predecessor: 1
        })
    );
}

/// INV-006: identical inputs give identical reductions, transcripts and spending.
#[test]
fn identical_inputs_give_identical_reductions() {
    let mut rng = SplitMix(6);
    for _ in 0..50 {
        let n = 1 + rng.below(30) as usize;
        let preds = random_order(&mut rng, n);
        let needed = vec![rng.below(n as u64) as usize];
        let run = || {
            let order = CausalOrder::from_predecessors(preds.clone()).expect("an order");
            let mut calls = 0;
            let mut oracle = needs(&order, &needed, &mut calls);
            reduce::minimize(&order, Deletion::Atoms, &mut oracle, BUDGET)
        };
        assert_eq!(run(), run());
    }
}

/// Metamorphic, independent-event swap: the same partial order written in another
/// linearization (a random linear extension: the events renumbered so that every
/// predecessor still comes first) reduces to the same core, mapped back. Under a
/// monotone failure the smallest failing configuration is unique, so neither the
/// closure nor deletion over configurations may depend on which independent events the
/// trace happened to list first.
#[test]
fn independent_event_swap_leaves_the_core_unchanged() {
    let mut rng = SplitMix(42);
    for _ in 0..200 {
        let n = 2 + rng.below(30) as usize;
        let preds = random_order(&mut rng, n);
        let needed = vec![rng.below(n as u64) as usize];
        // A random linear extension: repeatedly take a random event whose predecessors
        // are all placed.
        let mut placed = vec![false; n];
        let mut position = vec![0_usize; n];
        for slot in 0..n {
            let ready: Vec<usize> = (0..n)
                .filter(|&e| !placed[e] && preds[e].iter().all(|&p| placed[p]))
                .collect();
            let e = ready[rng.below(ready.len() as u64) as usize];
            placed[e] = true;
            position[e] = slot;
        }
        let mut swapped = vec![Vec::new(); n];
        for (e, ps) in preds.iter().enumerate() {
            swapped[position[e]] = ps.iter().map(|&p| position[p]).collect();
        }
        let needed_swapped: Vec<usize> = needed.iter().map(|&e| position[e]).collect();
        for mode in [None, Some(Deletion::Configurations)] {
            let core_of = |preds: &Vec<Vec<usize>>, needed: &[usize]| {
                let order = CausalOrder::from_predecessors(preds.clone()).expect("an order");
                let mut calls = 0;
                let mut oracle = needs(&order, needed, &mut calls);
                let r = match mode {
                    None => reduce::closure_pass(&order, &mut oracle, BUDGET),
                    Some(m) => reduce::minimize(&order, m, &mut oracle, BUDGET),
                };
                r.core().expect("reduced").events.clone()
            };
            let original = core_of(&preds, &needed);
            let mut mapped: Vec<usize> = core_of(&swapped, &needed_swapped);
            let back: Vec<usize> = {
                let mut inverse = vec![0_usize; n];
                for (e, &p) in position.iter().enumerate() {
                    inverse[p] = e;
                }
                mapped.iter_mut().for_each(|x| *x = inverse[*x]);
                mapped.sort_unstable();
                mapped
            };
            assert_eq!(original, back, "{mode:?}");
        }
    }
}
