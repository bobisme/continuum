//! PR 18 (bn-3km4z): the generic reduction passes of `continuum_debugger::reduce`, over
//! synthetic traces whose cores are known in advance. The instantiation on real failing
//! journals (the replicated register's M01) is `continuum-asupersync`'s
//! `tests/pr18_impl01_reduction.rs`.

use std::collections::BTreeSet;

use continuum_debugger::reduce::{
    self, Attempts, Budget, CausalOrder, Deletion, Guarantee, NotReplayable, OrderError, Pass,
    PassEnd, Reduction, Refusal, Replayed, TranscriptBound, Trial, Verdict,
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

// ---------------------------------------------------------------------------
// the per-removal transcript (RFC 0028, bn-2z08o)
// ---------------------------------------------------------------------------

/// The transcript's own consistency: every attempt is recorded in order with dense
/// indices, the fresh ones are exactly the replays, each pass's verdict counts are its
/// record's, each memo entry names an earlier fresh attempt with the same verdict and
/// size, each entry's removed events and candidate size add up to the set it was tried
/// against, and the version advances exactly at each kept candidate.
fn check_consistent(r: &Reduction) -> &Attempts {
    let (transcript, attempts, spent) = match r {
        Reduction::Reduced {
            transcript,
            attempts,
            spent,
            ..
        }
        | Reduction::Inconclusive {
            transcript,
            attempts,
            spent,
            ..
        } => (transcript, attempts, spent),
        Reduction::Refused(_) => panic!("refused: {r:?}"),
    };
    assert_eq!(attempts.omitted, 0);
    let fresh = attempts
        .entries
        .iter()
        .filter(|a| a.memo_of.is_none() && a.verdict != Verdict::Vacuous)
        .count();
    assert_eq!(fresh as u64, spent.replays);
    let mut version = 0;
    for (k, a) in attempts.entries.iter().enumerate() {
        assert_eq!(a.index, k as u64);
        assert_eq!(a.version, version, "#{k}");
        assert_eq!(a.removed.len() + a.size, a.from, "#{k}");
        assert!(a.removed.windows(2).all(|w| w[0] < w[1]));
        if let Some(m) = a.memo_of {
            let orig = &attempts.entries[usize::try_from(m).expect("small")];
            assert!(m < a.index && orig.memo_of.is_none() && orig.verdict != Verdict::Vacuous);
            assert_eq!((orig.verdict, orig.size), (a.verdict, a.size));
            assert!(a.reason.is_empty());
        }
        if a.verdict == Verdict::Vacuous {
            assert_eq!(a.size, 0);
        }
        if a.verdict == Verdict::Fails && a.trial != Trial::Start {
            version += 1;
        }
    }
    assert_eq!(attempts.version, version);
    for p in transcript {
        let of = |v: fn(&Verdict) -> bool| {
            attempts
                .entries
                .iter()
                .filter(|a| a.pass == Some(p.pass) && a.memo_of.is_none() && v(&a.verdict))
                .count() as u64
        };
        assert_eq!(of(|v| *v == Verdict::Holds), p.held);
        assert_eq!(of(|v| *v == Verdict::Nonconforming), p.nonconforming);
        assert_eq!(
            of(|v| matches!(v, Verdict::Inconclusive(_))),
            p.inconclusive
        );
    }
    attempts
}

/// The transcript records every attempt, consistently, over seeded random orders, in
/// both deletion modes.
#[test]
fn the_transcript_records_every_attempt_in_order() {
    let mut rng = SplitMix(2808);
    for _ in 0..200 {
        let n = 1 + rng.below(30) as usize;
        let preds = random_order(&mut rng, n);
        let order = CausalOrder::from_predecessors(preds).expect("an order");
        let needed: Vec<usize> = {
            let mut v: Vec<usize> = (0..1 + rng.below(3))
                .map(|_| rng.below(n as u64) as usize)
                .collect();
            v.sort_unstable();
            v.dedup();
            v
        };
        for mode in [Deletion::Configurations, Deletion::Atoms] {
            let mut calls = 0;
            let mut oracle = needs(&order, &needed, &mut calls);
            let r = reduce::minimize(&order, mode, &mut oracle, BUDGET);
            let attempts = check_consistent(&r);
            assert_eq!(attempts.entries[0].trial, Trial::Start);
            assert_eq!(attempts.entries[0].pass, None);
        }
    }
}

/// RFC 0028's checker, apart from the reducer: rebuild every version's set from the
/// transcript's start and the removals of its kept candidates (checking each entry's
/// size against the set it names), check that the final set is the core and that each
/// memo entry's candidate is its original's, then return, for the final set, every
/// recorded removal that did not fail and was decided.
fn rebuild(attempts: &Attempts, core: &reduce::Core) -> BTreeSet<Vec<usize>> {
    let mut sets: Vec<Vec<usize>> = vec![attempts.start.clone()];
    let mut candidates: std::collections::BTreeMap<u64, Vec<usize>> =
        std::collections::BTreeMap::new();
    for a in &attempts.entries {
        let v = usize::try_from(a.version).expect("small");
        assert_eq!(
            v + 1,
            sets.len(),
            "#{}: tried against the latest set",
            a.index
        );
        let set = &sets[v];
        assert_eq!(a.from, set.len(), "#{}", a.index);
        assert!(a.removed.iter().all(|e| set.binary_search(e).is_ok()));
        let candidate: Vec<usize> = set
            .iter()
            .copied()
            .filter(|e| a.removed.binary_search(e).is_err())
            .collect();
        assert_eq!(candidate.len(), a.size, "#{}", a.index);
        if let Some(m) = a.memo_of {
            assert_eq!(candidates.get(&m), Some(&candidate), "#{} memo", a.index);
        }
        candidates.insert(a.index, candidate.clone());
        if a.verdict == Verdict::Fails && a.trial != Trial::Start {
            sets.push(candidate);
        }
    }
    assert_eq!(sets.len() as u64, attempts.version + 1);
    assert_eq!(sets.last(), Some(&core.events), "the final set is the core");
    attempts
        .entries
        .iter()
        .filter(|a| {
            a.version == attempts.version
                && a.trial == Trial::Remove
                && matches!(
                    a.verdict,
                    Verdict::Holds | Verdict::Nonconforming | Verdict::Vacuous
                )
        })
        .map(|a| a.removed.clone())
        .collect()
}

/// The transcript licenses the minimality claim on its own (RFC 0028's checkers): the
/// sets are rebuilt from the transcript, and for every kept event of a minimal core the
/// removal of exactly its unit (the event over atoms; it with its causal future over
/// configurations) against the core is recorded, decided, and did not fail. Checked over
/// seeded random orders, one-event cores and removals that empty the core included.
#[test]
fn the_transcript_licenses_the_minimality_claim() {
    let mut rng = SplitMix(2809);
    let mut checked = 0;
    for _ in 0..200 {
        let n = 1 + rng.below(30) as usize;
        let preds = random_order(&mut rng, n);
        let order = CausalOrder::from_predecessors(preds.clone()).expect("an order");
        let needed = vec![rng.below(n as u64) as usize, rng.below(n as u64) as usize];
        for mode in [Deletion::Configurations, Deletion::Atoms] {
            let mut calls = 0;
            let mut oracle = needs(&order, &needed, &mut calls);
            let r = reduce::minimize(&order, mode, &mut oracle, BUDGET);
            let attempts = check_consistent(&r);
            let core = r.core().expect("reduced");
            assert!(core.guarantees.iter().any(|g| matches!(
                g,
                Guarantee::CausallyMinimal | Guarantee::OneMinimal | Guarantee::AtomMinimal
            )));
            let removals = rebuild(attempts, core);
            for &e in &core.events {
                let unit: Vec<usize> = match mode {
                    Deletion::Atoms => vec![e],
                    Deletion::Configurations => {
                        let future = brute_future(&preds, e);
                        core.events
                            .iter()
                            .copied()
                            .filter(|x| future.contains(x))
                            .collect()
                    }
                };
                assert!(
                    removals.contains(&unit),
                    "{mode:?}: no recorded removal of {unit:?} from {:?}",
                    core.events
                );
                checked += 1;
            }
        }
    }
    assert!(checked > 100, "{checked}");
}

/// A one-event core's only removal empties it: recorded as vacuous, so the transcript
/// still licenses the claim.
#[test]
fn a_one_event_core_records_its_vacuous_removal() {
    let order = CausalOrder::from_predecessors(vec![Vec::new(); 5]).expect("an order");
    let mut oracle = |kept: &[usize]| {
        if kept.contains(&3) {
            Replayed::Fails { witnesses: vec![3] }
        } else {
            Replayed::Holds
        }
    };
    let r = reduce::deletion_pass(
        &order,
        (0..5).collect(),
        Deletion::Atoms,
        &mut oracle,
        BUDGET,
    );
    let core = r.core().expect("reduced");
    assert_eq!(core.events, vec![3]);
    assert!(core.guarantees.contains(&Guarantee::OneMinimal));
    let attempts = check_consistent(&r);
    assert!(rebuild(attempts, core).contains(&vec![3]));
    assert!(
        attempts
            .entries
            .iter()
            .any(|a| a.verdict == Verdict::Vacuous)
    );
}

/// An oracle's long rendering is cut to `REASON_CAP` bytes at a character boundary,
/// with its full length kept: one long reason does not stop the transcript.
#[test]
fn a_long_reason_is_cut_not_dropped() {
    let order = CausalOrder::from_predecessors(vec![Vec::new(); 6]).expect("an order");
    let long = "é".repeat(4 * reduce::REASON_CAP);
    let mut oracle = |kept: &[usize]| {
        if kept.contains(&5) {
            Replayed::Fails { witnesses: vec![5] }
        } else {
            Replayed::NotReplayable(NotReplayable::Nonconforming(long.clone()))
        }
    };
    let r = reduce::deletion_pass(
        &order,
        (0..6).collect(),
        Deletion::Atoms,
        &mut oracle,
        BUDGET,
    );
    let attempts = check_consistent(&r);
    let a = attempts
        .entries
        .iter()
        .find(|a| a.verdict == Verdict::Nonconforming && a.memo_of.is_none())
        .expect("a refusal");
    assert!(a.reason.len() <= reduce::REASON_CAP && !a.reason.is_empty());
    assert_eq!(a.reason_bytes, long.len());
    assert!(
        r.core()
            .expect("reduced")
            .guarantees
            .contains(&Guarantee::OneMinimal)
    );
}

/// The transcript is bounded, and a bound that runs out is counted, never silent: the
/// core is the same, but no minimality class is claimed without its transcript.
#[test]
fn a_full_transcript_is_counted_and_withholds_minimality() {
    let order = CausalOrder::from_predecessors(vec![Vec::new(); 40]).expect("an order");
    let needed = [3, 17, 31];
    let oracle = |kept: &[usize]| {
        if needed.iter().all(|n| kept.contains(n)) {
            Replayed::Fails {
                witnesses: vec![31],
            }
        } else {
            Replayed::Holds
        }
    };
    let full = reduce::deletion_pass(
        &order,
        (0..40).collect(),
        Deletion::Atoms,
        &mut oracle.clone(),
        BUDGET,
    );
    for bound in [
        TranscriptBound {
            entries: 3,
            units: 1 << 20,
        },
        TranscriptBound {
            entries: 1 << 20,
            units: 50,
        },
        TranscriptBound {
            entries: 0,
            units: 0,
        },
    ] {
        let r = reduce::deletion_pass(
            &order,
            (0..40).collect(),
            Deletion::Atoms,
            &mut oracle.clone(),
            BUDGET.with_transcript(bound),
        );
        let attempts = r.attempts().expect("a transcript");
        assert!(attempts.omitted > 0, "{bound:?}");
        assert!(attempts.entries.len() as u64 <= bound.entries);
        let units: u64 = attempts
            .entries
            .iter()
            .map(|a| 1 + a.removed.len() as u64 + a.reason.len() as u64)
            .sum();
        assert!(units <= bound.units);
        assert_eq!(
            attempts.entries.len() as u64 + attempts.omitted,
            full.attempts().expect("a transcript").entries.len() as u64
        );
        let core = r.core().expect("reduced");
        assert_eq!(core.events, full.core().expect("reduced").events);
        assert!(!core.guarantees.contains(&Guarantee::OneMinimal));
    }
    assert!(
        full.core()
            .expect("reduced")
            .guarantees
            .contains(&Guarantee::OneMinimal)
    );
}

/// The transcript keeps the oracle's verdict and reason: a refusal's rendering, an
/// inconclusive's reason, and a memo hit's link to the attempt that replayed it.
#[test]
fn the_transcript_keeps_verdicts_and_reasons() {
    let order = CausalOrder::from_predecessors(vec![Vec::new(); 8]).expect("an order");
    let mut oracle = |kept: &[usize]| {
        if kept.len() == 8 || (kept.contains(&2) && kept.contains(&6)) {
            Replayed::Fails { witnesses: vec![6] }
        } else if kept.contains(&6) {
            Replayed::NotReplayable(NotReplayable::Inconclusive(
                InconclusiveReason::InsufficientTelemetry,
                "no telemetry".into(),
            ))
        } else {
            Replayed::NotReplayable(NotReplayable::Nonconforming("not a run".into()))
        }
    };
    let r = reduce::deletion_pass(
        &order,
        (0..8).collect(),
        Deletion::Atoms,
        &mut oracle,
        BUDGET,
    );
    let attempts = check_consistent(&r);
    assert!(attempts.entries.iter().any(|a| a.verdict
        == Verdict::Inconclusive(InconclusiveReason::InsufficientTelemetry)
        && a.reason == "no telemetry"));
    assert!(
        attempts
            .entries
            .iter()
            .any(|a| a.verdict == Verdict::Nonconforming && a.reason == "not a run")
    );
    assert!(attempts.entries.iter().any(|a| a.memo_of.is_some()));
}
