//! The cross-engine differential harness over the engines on trunk (bn-34mw).
//!
//! - **Corpus run.** A seeded corpus (generator seeds `0..64`, the Die Hard
//!   transcription, and 16 fixtures with definedness predicates, bn-24a5c) goes through every lane whose engines are plugged in: the closure
//!   oracle, the kernel (through certificates) and the tiny exhaustive oracle, each
//!   against the reference path. The per-lane counts are pinned, and no lane may
//!   disagree: every claim a disagreement would halt must be in the committed
//!   register, and no registered claim may be asserted positive.
//! - **Planted disagreements.** Mutant engines in the explicit-engine slot: a lost
//!   transition, a forged counterexample, and a budget stop laundered into `Holds`.
//!   Each is detected, quarantines C006, fails the gate against the real claims
//!   baseline (C006 is OBSERVED), emits a deterministic `defect_*` report, and is
//!   minimized to a 1-minimal fixture that the test re-verifies on its own.
//! - **Boundaries.** A budget stop is inconclusive, never a disagreement; semantics an
//!   engine does not handle is `Unsupported`, never a disagreement.

mod support {
    pub mod engines;
    pub mod json;
}

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use continuum_corpus::differential::engine::{
    CLOSURE, DPOR, EXPLICIT, KERNEL, REFERENCE, SEMANTIC_ORACLE,
};
use continuum_corpus::differential::generate::{generate, generate_with_definedness};
use continuum_corpus::differential::{
    Budget, ClaimState, ClosureOracle, ContractFault, Decided, Disagreement, Element, Engine,
    Epochs, Fields, Fixture, GateFinding, Harness, Inconclusive, InvariantVerdict, LANES,
    LaneStatus, Minimality, MinimizeBudget, Normalized, Projection, QuarantineLedger, RunReport,
    Side, UndefinedKind, compare, compare_fields, ddmin, gate, halts,
};
use continuum_engine_reference::diehard;
use continuum_value::assurance::InconclusiveReason;

use support::engines::{
    DporEngine, DporLostState, KernelEngine, Mutant, Mutation, ReferenceEngine,
    SemanticOracleEngine,
};
use support::json;

const SEEDS: std::ops::Range<u64> = 0..64;

const BUDGET: Budget = Budget {
    states: 4096,
    depth: 4096,
    transitions: 1 << 20,
};

const MINIMIZE: MinimizeBudget = MinimizeBudget { tests: 512 };

fn epochs() -> Epochs {
    Epochs {
        semantic: "continuum-semantics-1".to_owned(),
        proof: None,
    }
}

/// Seeds of the fixtures that carry definedness predicates (bn-24a5c).
const DEFINEDNESS_SEEDS: std::ops::Range<u64> = 0..16;

/// Generator seeds, Die Hard, and the definedness fixtures.
const CORPUS_LEN: usize = 64 + 1 + 16;

fn corpus() -> Vec<Fixture> {
    let mut out: Vec<Fixture> = SEEDS.map(generate).collect();
    let die_hard = diehard::model().expect("the Die Hard transcription is a valid model");
    out.push(Fixture::from_model("die-hard", &die_hard));
    out.extend(DEFINEDNESS_SEEDS.map(generate_with_definedness));
    out
}

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// docs/18 claim states, from the pinned baseline `check_t09_evidence.py` keeps equal
/// to the live rows.
fn claim_states() -> BTreeMap<String, ClaimState> {
    let text = std::fs::read_to_string(root().join("tools/governance/claims-baseline.json"))
        .expect("the claims baseline is readable");
    let document = json::parse(&text).expect("the claims baseline is JSON");
    let entries = document
        .get("entries")
        .and_then(json::Json::as_array)
        .expect("the baseline has entries");
    let mut out = BTreeMap::new();
    for entry in entries {
        let id = entry
            .get("id")
            .and_then(json::Json::as_str)
            .expect("an entry has an id");
        let state = entry
            .get("state")
            .and_then(json::Json::as_str)
            .expect("an entry has a state");
        let state =
            ClaimState::parse(state).unwrap_or_else(|| panic!("{id}: unknown state {state}"));
        assert!(
            out.insert(id.to_owned(), state).is_none(),
            "{id} pinned twice"
        );
    }
    assert!(out.len() >= 35, "the baseline pins every docs/18 row");
    out
}

fn register() -> QuarantineLedger {
    let text = std::fs::read_to_string(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("differential/quarantine.txt"),
    )
    .expect("the quarantine register is readable");
    QuarantineLedger::parse(&text).expect("the quarantine register parses")
}

/// Plug honest test engines into the slots they name. (A configuration names slots
/// itself; the contract tests below do that for engines that cannot.)
fn slots<'a>(engines: &[&'a dyn Engine]) -> Vec<(String, &'a dyn Engine)> {
    engines.iter().map(|e| (e.identity().slot, *e)).collect()
}

fn run(engines: &[&dyn Engine], budget: Budget) -> RunReport {
    Harness::new(&slots(engines), &LANES, budget, epochs(), MINIMIZE)
        .expect("one engine per slot")
        .run(&corpus())
}

fn trunk_engines() -> [&'static dyn Engine; 4] {
    [
        &ReferenceEngine,
        &ClosureOracle,
        &KernelEngine,
        &SemanticOracleEngine,
    ]
}

fn lane<'r>(report: &'r RunReport, subject: &str, oracle: &str) -> &'r LaneStatus {
    &report
        .lanes
        .iter()
        .find(|(lane, _)| lane.subject == subject && lane.oracle == oracle)
        .expect("the lane is in the table")
        .1
}

// ---------------------------------------------------------------------------
// the corpus run
// ---------------------------------------------------------------------------

/// The pinned per-lane counts of the seeded corpus run. A change here is a change in
/// an engine's answers or in the corpus, and must be explained, not re-pinned blindly.
const PINNED_SUMMARY: &str = "\
fixtures=81 unbuildable=0
lane continuum-corpus::closure vs continuum-engine-reference fixtures=81 no-invariants=0 agreed=384 unjudged=0 witnesses-replayed=124 undefined-checked=78 undecided=[deadlocks-under-undefined-action:8] disagreeing-fixtures=0 disagreements=0
lane continuum-kernel-core vs continuum-engine-reference fixtures=81 no-invariants=0 agreed=264 unjudged=0 witnesses-replayed=62 undefined-checked=39 undecided=[Unsupported/-:8,undefined-unproven:39] disagreeing-fixtures=0 disagreements=0
lane continuum-engine-reference::semantic vs continuum-engine-reference fixtures=81 no-invariants=0 agreed=80 unjudged=157 witnesses-replayed=62 undefined-checked=39 undecided=[Unsupported/-:38,deadlocks-under-undefined-action:6] disagreeing-fixtures=0 disagreements=0
lane continuum-engine-explicit vs continuum-engine-reference absent missing=continuum-engine-explicit
lane continuum-engine-dpor vs continuum-engine-reference::semantic absent missing=continuum-engine-dpor
faulted-slots=[]
";

#[test]
fn the_seeded_corpus_agrees_across_every_engine_pair() {
    let report = run(&trunk_engines(), BUDGET);
    let summary = report.summary();
    println!("{summary}");
    assert!(report.unbuildable.is_empty(), "{:?}", report.unbuildable);
    for finding in &report.findings {
        println!("{}", finding.report.encode());
    }
    // The halting rule: a finding passes only when its whole quarantine entry is in
    // the committed register and its claims are not asserted positive. With the
    // register empty, that means no lane may disagree.
    assert_eq!(
        halts(&report.ledger, &register(), &claim_states()),
        Vec::new(),
        "the trunk engines disagree:\n{summary}"
    );

    // Anti-vacuity: each lane that ran compared something, and the verdict and replay
    // fields were exercised.
    for (subject, oracle) in [
        (CLOSURE, REFERENCE),
        (KERNEL, REFERENCE),
        (SEMANTIC_ORACLE, REFERENCE),
    ] {
        let LaneStatus::Ran(counts) = lane(&report, subject, oracle) else {
            panic!("{subject} vs {oracle} did not run");
        };
        assert_eq!(counts.fixtures, CORPUS_LEN, "{subject}");
        // The projection-only tiny oracle decides fewer fields (no verdicts).
        let floor = if subject == SEMANTIC_ORACLE {
            CORPUS_LEN / 2
        } else {
            CORPUS_LEN
        };
        assert!(counts.agreed > floor, "{subject}: {counts:?}");
    }
    let LaneStatus::Ran(closure) = lane(&report, CLOSURE, REFERENCE) else {
        unreachable!()
    };
    assert!(closure.witnesses_replayed > 10, "{closure:?}");
    // The absent slots are reported absent, never passed.
    for (subject, oracle, missing) in [
        (EXPLICIT, REFERENCE, EXPLICIT),
        (DPOR, SEMANTIC_ORACLE, DPOR),
    ] {
        assert_eq!(
            lane(&report, subject, oracle),
            &LaneStatus::Absent {
                missing: vec![missing.to_owned()]
            }
        );
    }
    assert_eq!(summary, PINNED_SUMMARY);
}

#[test]
fn the_corpus_exercises_every_compared_field() {
    let mut holds = 0;
    let mut violated = 0;
    let mut deadlocked = 0;
    let mut nondeterministic = 0;
    let mut undefined_action_verdicts = 0;
    let mut undefined_invariant_verdicts = 0;
    let mut undefined_models = 0;
    let mut defined_despite_guards = 0;
    for fixture in corpus() {
        let model = fixture.build().expect("every corpus fixture builds");
        let answer = ReferenceEngine.evaluate(&model, BUDGET);
        for verdict in answer.invariants.values() {
            match verdict {
                InvariantVerdict::Holds => holds += 1,
                InvariantVerdict::Violated { witness: Some(_) } => violated += 1,
                InvariantVerdict::Undefined(read) => match read.kind {
                    UndefinedKind::Action => undefined_action_verdicts += 1,
                    UndefinedKind::Invariant => undefined_invariant_verdicts += 1,
                },
                other => panic!("{}: {other:?}", fixture.label),
            }
        }
        undefined_models += usize::from(answer.undefined_action.is_some());
        let guarded = fixture
            .predicates
            .iter()
            .any(|(n, _)| n.ends_with("#defined"));
        defined_despite_guards += usize::from(
            guarded
                && answer
                    .invariants
                    .values()
                    .all(|v| !matches!(v, InvariantVerdict::Undefined(_))),
        );
        if let Projection::Exact { deadlocks, .. } = &answer.projection {
            deadlocked += usize::from(!deadlocks.is_empty());
        }
        nondeterministic += usize::from(model.actions().iter().any(|a| !a.is_deterministic()));
    }
    assert!(
        holds > 10 && violated > 10,
        "holds={holds} violated={violated}"
    );
    assert!(deadlocked > 5, "deadlocked={deadlocked}");
    assert!(nondeterministic > 5, "nondeterministic={nondeterministic}");
    // The undefined-read category is exercised in both kinds, at model level, and
    // with guards that hold (so its absence is compared too).
    assert!(
        undefined_action_verdicts > 0
            && undefined_invariant_verdicts > 0
            && undefined_models > 0
            && defined_despite_guards > 0,
        "action={undefined_action_verdicts} invariant={undefined_invariant_verdicts} models={undefined_models} defined={defined_despite_guards}"
    );
}

#[test]
fn the_run_is_deterministic() {
    let first = run(&trunk_engines(), BUDGET);
    let second = run(&trunk_engines(), BUDGET);
    assert_eq!(first.summary(), second.summary());
    assert_eq!(first, second);
}

// ---------------------------------------------------------------------------
// the DPOR lane (bn-3vhzw, C005): continuum-engine-dpor is now on trunk
// ---------------------------------------------------------------------------

fn dpor_engines() -> [&'static dyn Engine; 2] {
    [&SemanticOracleEngine, &DporEngine]
}

/// The pinned per-lane counts of the seeded corpus run against the reduced engine
/// alone: only the DPOR lane can run (its oracle needs no other slot), so every
/// other lane in the table is absent. A change here is a change in the reducer's
/// answers or in the corpus, and must be explained, not re-pinned blindly.
const DPOR_PINNED_SUMMARY: &str = "\
fixtures=81 unbuildable=0
lane continuum-corpus::closure vs continuum-engine-reference absent missing=continuum-corpus::closure,continuum-engine-reference
lane continuum-kernel-core vs continuum-engine-reference absent missing=continuum-kernel-core,continuum-engine-reference
lane continuum-engine-reference::semantic vs continuum-engine-reference absent missing=continuum-engine-reference
lane continuum-engine-explicit vs continuum-engine-reference absent missing=continuum-engine-explicit,continuum-engine-reference
lane continuum-engine-dpor vs continuum-engine-reference::semantic fixtures=81 no-invariants=0 agreed=53 unjudged=157 witnesses-replayed=62 undefined-checked=39 undecided=[-/Unsupported:22,Unsupported/-:14,Unsupported/Unsupported:16,deadlocks-under-undefined-action:5] disagreeing-fixtures=0 disagreements=0
faulted-slots=[]
";

#[test]
fn the_dpor_lane_agrees_with_the_tiny_oracle_over_the_seeded_corpus() {
    let report = run(&dpor_engines(), BUDGET);
    let summary = report.summary();
    println!("{summary}");
    assert!(report.unbuildable.is_empty(), "{:?}", report.unbuildable);
    // The halting rule: no lane may disagree.
    assert_eq!(
        halts(&report.ledger, &register(), &claim_states()),
        Vec::new(),
        "DPOR disagrees with the tiny oracle:\n{summary}"
    );

    let LaneStatus::Ran(counts) = lane(&report, DPOR, SEMANTIC_ORACLE) else {
        panic!("the DPOR lane did not run");
    };
    assert_eq!(counts.fixtures, CORPUS_LEN);
    assert_eq!(counts.disagreements, 0);
    assert_eq!(counts.disagreeing_fixtures, 0);
    // Anti-vacuity: the lane genuinely compared something (a fixture whose visible
    // mask covers every declared variable, so the reduced search's answer is a
    // full, exact reachable set the tiny oracle's own exact set is checked
    // against), and models the reduction cannot decide at that grain are counted,
    // not silently dropped as agreement.
    assert!(counts.agreed > 0, "{counts:?}");
    assert!(!counts.undecided.is_empty(), "{counts:?}");
    assert!(counts.witnesses_replayed > 0, "{counts:?}");
    assert_eq!(summary, DPOR_PINNED_SUMMARY);
}

/// A full visible mask is provably the one case where the reduction is a no-op: with
/// every variable visible, every progressing label writes a visible one, so no
/// stubborn set without an enabled visible label can ever exist
/// (`footprint::Footprints::is_visible`; every seed's closure covers every enabled
/// label or contains a visible one) and every state expands in full
/// (`witness::FullReason::Exhaustive`) — never a persistent-set decline, and, because
/// a visit's sleep set is only consulted inside a `Reduced` expansion, never a
/// sleep-set decline either. This checks it directly over the reducer's own `Stats`,
/// so the claim in `DporEngine`'s doc cannot go stale: the DPOR lane's exact-set
/// agreement (`agreed` in `the_dpor_lane_agrees_with_the_tiny_oracle_over_the_seeded_corpus`)
/// is evidence that DPOR's search machinery agrees with a second, differently-coded
/// exhaustive explorer on these fixtures, never evidence about the persistent-set or
/// sleep-set reduction specifically — that reduction is exercised and checked
/// exhaustively by `continuum-engine-dpor`'s own `tests/c005_differential.rs`
/// (bn-voq4), including on masked models this lane cannot decide the reachable-state
/// field for at all (it reports them `Unsupported`, `undecided` in the corpus run).
#[test]
fn the_dpor_lane_s_exact_answers_are_provably_unreduced_full_mask_fixtures() {
    let mut checked = 0_usize;
    for fixture in corpus() {
        let model = fixture.build().expect("every corpus fixture builds");
        let obligations = continuum_engine_dpor::Obligations::every_predicate(
            &model,
            continuum_engine_dpor::DeadlockPolicy::Defect,
        );
        let bounds = continuum_engine_dpor::Bounds::new(
            BUDGET.states,
            BUDGET.transitions,
            BUDGET.depth,
            1 << 40,
        );
        let Ok(report) = continuum_engine_dpor::check(&model, &obligations, bounds) else {
            continue;
        };
        let full = if model.variables().len() >= 64 {
            u64::MAX
        } else {
            (1_u64 << model.variables().len()) - 1
        };
        if *report.completeness() == continuum_engine_dpor::Completeness::Complete
            && report.witness().visible() == full
        {
            let stats = report.stats();
            assert_eq!(
                stats.declined_persistent, 0,
                "{}: a full-mask fixture declined a persistent-set label: {stats:?}",
                fixture.label
            );
            assert_eq!(
                stats.declined_sleep, 0,
                "{}: a full-mask fixture declined a sleeping label: {stats:?}",
                fixture.label
            );
            checked = checked.saturating_add(1);
        }
    }
    assert!(
        checked > 0,
        "no corpus fixture reached a full visible mask; this test checks nothing"
    );
}

#[test]
fn a_planted_dpor_fault_is_detected_quarantined_and_minimized() {
    let engines: [&dyn Engine; 2] = [&SemanticOracleEngine, &DporLostState];
    let report = run(&engines, BUDGET);
    println!("{}", report.summary());
    assert!(
        !report.findings.is_empty(),
        "the planted DPOR fault was not detected"
    );
    // Pinned so a change is explained, not silently absorbed as "still non-empty".
    // `DporLostState` perturbs every fixture whose honest answer is `Exact` (51 of
    // the corpus's 81): the 29 the tiny oracle also decides show `ReachableStates`
    // directly; of the other 22 (`Unsupported`), only the 2 whose dropped state is
    // also the state one of DPOR's own undefined-read claims names show
    // `InvalidUndefined` (the drop corrupts the claim's own reachable-set proof, even
    // though its path still replays); the remaining 20 firings are real but silent on
    // this lane — nothing here re-derives the reachable set independently of DPOR's
    // own answer to check a merely-smaller one against, on a fixture the oracle
    // cannot decide.
    let LaneStatus::Ran(mutant_counts) = lane(&report, DPOR, SEMANTIC_ORACLE) else {
        panic!("the DPOR lane did not run");
    };
    assert_eq!(mutant_counts.disagreeing_fixtures, 31, "{mutant_counts:?}");
    for finding in &report.findings {
        assert_eq!(finding.lane.subject, DPOR);
        assert_eq!(finding.lane.oracle, SEMANTIC_ORACLE);
        // Most fixtures let the tiny oracle decide the reachable-state field, and the
        // dropped state shows there directly; on a fixture the oracle itself cannot
        // decide (a nondeterministic action, `Unsupported`), the same drop still
        // shows because DPOR's own undefined-read claim about the dropped state
        // no longer holds against DPOR's own (now smaller) reachable set.
        assert!(
            finding.disagreements.iter().any(|d| matches!(
                d,
                Disagreement::ReachableStates { .. }
                    | Disagreement::InvalidUndefined {
                        side: Side::Subject,
                        ..
                    }
            )),
            "{:?}",
            finding.disagreements
        );
        assert!(finding.minimized.size() <= finding.report.original.size);
    }
    assert!(
        report
            .findings
            .iter()
            .any(|f| f.minimized.size() < f.report.original.size),
        "minimization never shrank a fixture"
    );

    // Halting: C005 is quarantined, is OBSERVED in docs/18, and is not registered.
    assert_eq!(report.ledger.claims(), BTreeSet::from(["C005"]));
    let states = claim_states();
    assert!(
        gate(&report.ledger, &states).contains(&GateFinding::AssertedPositive {
            claim: "C005".to_owned(),
            state: ClaimState::Observed,
        })
    );
    let halted = halts(&report.ledger, &register(), &states);
    assert!(halted.contains(&GateFinding::AssertedPositive {
        claim: "C005".to_owned(),
        state: ClaimState::Observed,
    }));
    assert!(
        halted
            .iter()
            .any(|f| matches!(f, GateFinding::Unregistered { claim, .. } if claim == "C005"))
    );
}

// ---------------------------------------------------------------------------
// planted disagreements
// ---------------------------------------------------------------------------

/// Everything a finding must carry, checked on its own terms.
fn assert_handled(report: &RunReport, expected_key: fn(&Disagreement) -> bool) {
    assert!(
        !report.findings.is_empty(),
        "the planted mutant was not detected"
    );
    let states = claim_states();
    for finding in &report.findings {
        assert_eq!(finding.lane.subject, EXPLICIT);
        assert_eq!(finding.lane.oracle, REFERENCE);
        assert!(
            finding.disagreements.iter().any(expected_key),
            "{:?}",
            finding.disagreements
        );

        // The defect report pins what plan §4.7 requires, and its handle is its hash.
        let encoded = finding.report.encode();
        assert!(encoded.starts_with("format continuum-corpus-defect/2\n"));
        assert!(encoded.contains("\nsubject.slot continuum-engine-explicit\n"));
        assert!(encoded.contains("\nsubject.build planted-mutant/"));
        assert!(encoded.contains("\noracle.slot continuum-engine-reference\n"));
        assert!(encoded.contains("\nclaims.count 1\nclaim C006\n"));
        assert!(encoded.contains("\nepoch.semantic continuum-semantics-1\n"));
        assert!(encoded.contains("\nepoch.proof.absent -\n"));
        assert!(encoded.contains("\noriginal.identity blake3-256:"));
        assert!(encoded.contains("\nminimized.encoding "));
        let identity = finding
            .handle
            .strip_prefix("defect_")
            .expect("a defect_ handle");
        assert_eq!(identity.len(), 64);
        assert!(
            identity
                .bytes()
                .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
        );
        assert_eq!(finding.handle, finding.report.handle());

        // Minimized: no larger, reproduces, and (when claimed) 1-minimal, re-checked
        // here rather than taken from the minimizer.
        let minimized = &finding.minimized;
        assert!(minimized.size() <= finding.report.original.size);
        // The harness minimizes against the first (sorted) disagreement's key.
        let key = finding.disagreements.first().expect("a disagreement").key();
        assert_eq!(
            finding.report.disagreement,
            finding.disagreements[0].to_string()
        );
        let reproduces = |fixture: &Fixture| -> bool {
            let Ok(model) = fixture.build() else {
                return false;
            };
            let subject =
                Mutant(mutation_of(&finding.report.subject.build)).evaluate(&model, BUDGET);
            let oracle = ReferenceEngine.evaluate(&model, BUDGET);
            compare(&model, &subject, &oracle)
                .disagreements
                .iter()
                .any(|d| d.key() == key)
        };
        assert!(
            reproduces(minimized),
            "{}: the minimized fixture does not reproduce",
            finding.fixture
        );
        if finding.report.reproduction.minimality == Minimality::OneMinimal {
            for element in minimized.elements() {
                let keep: BTreeSet<Element> = minimized
                    .elements()
                    .into_iter()
                    .filter(|e| *e != element)
                    .collect();
                assert!(
                    !reproduces(&minimized.restrict(&keep)),
                    "{}: removing {element:?} still reproduces",
                    finding.fixture
                );
            }
        }
    }
    assert!(
        report
            .findings
            .iter()
            .any(|f| f.minimized.size() < f.report.original.size),
        "minimization never shrank a fixture"
    );

    // Halting: C006 is quarantined, is OBSERVED in docs/18, and is not registered.
    assert_eq!(report.ledger.claims(), BTreeSet::from(["C006"]));
    let findings = gate(&report.ledger, &states);
    assert_eq!(
        findings,
        vec![GateFinding::AssertedPositive {
            claim: "C006".to_owned(),
            state: ClaimState::Observed,
        }]
    );
    let halted = halts(&report.ledger, &register(), &states);
    assert!(halted.contains(&GateFinding::AssertedPositive {
        claim: "C006".to_owned(),
        state: ClaimState::Observed,
    }));
    assert!(
        halted
            .iter()
            .any(|f| matches!(f, GateFinding::Unregistered { claim, .. } if claim == "C006"))
    );
    // Recording the quarantine clears the registration half only; the positive
    // assertion still fails until docs/18 moves C006 off OBSERVED.
    // A register line for C006 with another defect's handle covers nothing.
    let stale = QuarantineLedger::parse(&format!(
        "C006 {EXPLICIT} {REFERENCE} defect_{}\n",
        "0".repeat(64)
    ))
    .expect("parses");
    assert!(
        halts(&report.ledger, &stale, &states)
            .iter()
            .any(|f| matches!(f, GateFinding::Unregistered { claim, .. } if claim == "C006"))
    );
    let registered = halts(&report.ledger, &report.ledger, &states);
    assert_eq!(registered, findings);
    let mut demoted = states.clone();
    demoted.insert("C006".to_owned(), ClaimState::Blocked);
    assert_eq!(halts(&report.ledger, &report.ledger, &demoted), Vec::new());
}

fn mutation_of(build: &str) -> Mutation {
    match build {
        "planted-mutant/drop-successor" => Mutation::DropSuccessor,
        "planted-mutant/forge-witness" => Mutation::ForgeWitness,
        "planted-mutant/launder-budget" => Mutation::LaunderBudget,
        "planted-mutant/engine-error-on-wide" => Mutation::EngineErrorOnWide,
        "planted-mutant/drop-all-verdicts" => Mutation::DropAllVerdicts,
        "planted-mutant/ignore-definedness" => Mutation::IgnoreDefinedness,
        "planted-mutant/swap-undefined-kind" => Mutation::SwapUndefinedKind,
        "planted-mutant/drop-undefined-action" => Mutation::DropUndefinedAction,
        "planted-mutant/forge-undefined-state" => Mutation::ForgeUndefinedState,
        other => panic!("not a planted mutant: {other}"),
    }
}

fn with_mutant(mutation: Mutation) -> RunReport {
    let mutant = Mutant(mutation);
    let engines: [&dyn Engine; 5] = [
        &ReferenceEngine,
        &ClosureOracle,
        &KernelEngine,
        &SemanticOracleEngine,
        &mutant,
    ];
    run(&engines, BUDGET)
}

#[test]
fn a_planted_lost_transition_is_detected_quarantined_reported_and_minimized() {
    let report = with_mutant(Mutation::DropSuccessor);
    println!("{}", report.summary());
    if let Some(finding) = report.findings.first() {
        println!("{}", finding.report.encode());
    }
    assert_handled(&report, |d| {
        matches!(
            d,
            Disagreement::ReachableStates { .. }
                | Disagreement::Deadlocks { .. }
                | Disagreement::Verdict { .. }
        )
    });
    // Only the explicit lane saw it: the trunk lanes are unchanged.
    for finding in &report.findings {
        assert_eq!(finding.lane.subject, EXPLICIT);
    }
    // The report is a function of its inputs.
    let again = with_mutant(Mutation::DropSuccessor);
    let handles = |r: &RunReport| {
        r.findings
            .iter()
            .map(|f| f.handle.clone())
            .collect::<Vec<_>>()
    };
    assert_eq!(handles(&report), handles(&again));
}

#[test]
fn a_forged_counterexample_is_caught_by_replay() {
    let report = with_mutant(Mutation::ForgeWitness);
    assert_handled(&report, |d| {
        matches!(
            d,
            Disagreement::InvalidWitness {
                side: Side::Subject,
                ..
            }
        )
    });
}

// ---------------------------------------------------------------------------
// boundaries
// ---------------------------------------------------------------------------

const TIGHT: Budget = Budget {
    states: 3,
    depth: 2,
    transitions: 8,
};

#[test]
fn budget_exhaustion_is_inconclusive_not_disagreement() {
    let report = run(&trunk_engines(), TIGHT);
    println!("{}", report.summary());
    assert!(report.findings.is_empty(), "{}", report.summary());
    assert!(report.ledger.is_empty());
    let LaneStatus::Ran(closure) = lane(&report, CLOSURE, REFERENCE) else {
        panic!("the closure lane ran")
    };
    let exhausted: usize = closure
        .undecided
        .iter()
        .filter(|(reasons, _)| reasons.contains("ResourceExhausted"))
        .map(|(_, count)| *count)
        .sum();
    assert!(exhausted > 20, "{closure:?}");

    // One field, directly: a budget stop against a decisive answer records reasons.
    let model = generate(1).build().expect("builds");
    let stopped = Normalized::inconclusive(
        &Inconclusive::new(InconclusiveReason::ResourceExhausted, "tight"),
        model.predicates().iter().map(|p| p.name().as_str()),
    );
    let decided = ReferenceEngine.evaluate(&model, BUDGET);
    let comparison = compare(&model, &stopped, &decided);
    assert!(comparison.disagreements.is_empty());
    assert!(comparison
        .undecided
        .iter()
        .all(|u| u.subject == Some(InconclusiveReason::ResourceExhausted) && u.oracle.is_none()));
    assert!(!comparison.undecided.is_empty());
}

/// An engine that turns a budget stop into `Holds` is not protected by the rule
/// above: its decisive answer meets the oracle's decisive refutation.
#[test]
fn a_budget_stop_laundered_into_holds_is_a_disagreement() {
    struct Launder;
    impl Engine for Launder {
        fn identity(&self) -> continuum_corpus::differential::EngineIdentity {
            Mutant(Mutation::LaunderBudget).identity()
        }
        fn fields(&self) -> Fields {
            Fields::ALL
        }
        fn evaluate(
            &self,
            model: &continuum_engine_reference::model::Model,
            _: Budget,
        ) -> Normalized {
            Mutant(Mutation::LaunderBudget).evaluate(model, TIGHT)
        }
    }
    let engines: [&dyn Engine; 2] = [&ReferenceEngine, &Launder];
    let report = run(&engines, BUDGET);
    assert!(!report.findings.is_empty());
    assert!(report.findings.iter().all(|f| {
        f.disagreements.iter().any(|d| {
            matches!(
                d,
                Disagreement::Verdict { .. }
                    | Disagreement::UnexploredHolds {
                        side: Side::Subject,
                        ..
                    }
            )
        })
    }));
    assert!(report.findings.iter().any(|f| {
        f.disagreements
            .iter()
            .any(|d| matches!(d, Disagreement::Verdict { .. }))
    }));
    assert_eq!(report.ledger.claims(), BTreeSet::from(["C006"]));
}

/// The same laundering under one shared budget: the oracle is stopped too, so no
/// decisive answer contradicts the `Holds`. It is still caught, because the engine's
/// own reachable-state projection says it never finished exploring.
#[test]
fn a_laundered_holds_is_caught_when_both_sides_are_stopped() {
    let mutant = Mutant(Mutation::LaunderBudget);
    let engines: [&dyn Engine; 2] = [&ReferenceEngine, &mutant];
    let report = run(&engines, TIGHT);
    assert!(!report.findings.is_empty(), "{}", report.summary());
    assert!(
        report
            .findings
            .iter()
            .all(|f| f.disagreements.iter().any(|d| matches!(
                d,
                Disagreement::UnexploredHolds {
                    side: Side::Subject,
                    ..
                }
            )))
    );
    // The honest reference under the same budget is never flagged.
    assert!(
        report
            .findings
            .iter()
            .all(|f| !f.disagreements.iter().any(|d| matches!(
                d,
                Disagreement::UnexploredHolds {
                    side: Side::Oracle,
                    ..
                }
            )))
    );
}

/// An engine that judges invariants but leaves one out cannot agree vacuously.
#[test]
fn an_omitted_invariant_is_a_coverage_disagreement() {
    let model = diehard::model().expect("valid");
    let honest = ReferenceEngine.evaluate(&model, BUDGET);
    let mut partial = honest.clone();
    let dropped = partial
        .invariants
        .keys()
        .next()
        .cloned()
        .expect("die hard has predicates");
    partial.invariants.remove(&dropped);
    let comparison = compare(&model, &partial, &honest);
    assert!(
        comparison
            .disagreements
            .contains(&Disagreement::InvariantCoverage {
                side: Side::Subject,
                name: dropped,
            })
    );
    let mut invented = honest.clone();
    invented
        .invariants
        .insert("NotDeclared".to_owned(), InvariantVerdict::Holds);
    assert!(
        compare(&model, &invented, &honest)
            .disagreements
            .iter()
            .any(|d| matches!(
                d,
                Disagreement::InvariantCoverage { name, .. } if name == "NotDeclared"
            ))
    );
    assert!(compare(&model, &honest, &honest).disagreements.is_empty());
}

#[test]
fn a_defect_encoding_cannot_be_forged_by_a_field_value() {
    let report = with_mutant(Mutation::DropSuccessor);
    let original = report.findings.first().expect("a finding").report.clone();
    let mut forged = original.clone();
    forged.subject.build = format!("{}\noracle.slot x", original.subject.build);
    assert_ne!(forged.encode(), original.encode());
    assert_eq!(
        forged.encode().lines().count(),
        original.encode().lines().count()
    );
    let mut forged_fault = original.clone();
    forged_fault.faults.push((
        "x".to_owned(),
        "e\nfault.slot continuum-engine-reference".to_owned(),
    ));
    assert!(!encoded_fault_slots(&forged_fault.encode()).contains(REFERENCE));
    let mut split = original.clone();
    split.claims = vec!["C006".to_owned(), "C018".to_owned()];
    let mut joined = original.clone();
    joined.claims = vec!["C006,C018".to_owned()];
    assert_ne!(split.handle(), joined.handle());
    let mut absent = original.clone();
    absent.epochs.proof = Some("absent".to_owned());
    assert_ne!(absent.handle(), original.handle());
}

#[test]
fn unsupported_semantics_is_inconclusive_not_disagreement() {
    let fixture = corpus()
        .into_iter()
        .find(|f| f.actions.iter().any(|a| a.outcomes.len() > 1))
        .expect("the corpus has a nondeterministic action");
    let model = fixture.build().expect("builds");
    let answer = SemanticOracleEngine.evaluate(&model, BUDGET);
    let Projection::Inconclusive(why) = &answer.projection else {
        panic!("{answer:?}")
    };
    assert_eq!(why.reason, InconclusiveReason::Unsupported);
    let comparison = compare_fields(
        &model,
        &answer,
        Fields::PROJECTION_ONLY,
        &ReferenceEngine.evaluate(&model, BUDGET),
        Fields::ALL,
    );
    assert!(comparison.disagreements.is_empty());
    assert_eq!(comparison.undecided.len(), 1);
    assert_eq!(
        comparison.undecided[0].subject,
        Some(InconclusiveReason::Unsupported)
    );
}

// ---------------------------------------------------------------------------
// the claim table, the register, and the gate
// ---------------------------------------------------------------------------

#[test]
fn every_lane_names_registered_claims() {
    let states = claim_states();
    for lane in LANES {
        assert!(
            !lane.claims.is_empty(),
            "{} vs {}",
            lane.subject,
            lane.oracle
        );
        for claim in lane.claims {
            assert!(
                states.contains_key(*claim),
                "{claim} is not a docs/18 claim"
            );
        }
    }
}

#[test]
fn the_committed_register_holds_the_gate() {
    let register = register();
    assert_eq!(gate(&register, &claim_states()), Vec::new());
    for entry in register.entries() {
        assert!(
            LANES.iter().any(|l| l.subject == entry.subject
                && l.oracle == entry.oracle
                && l.claims.contains(&entry.claim.as_str())),
            "{entry:?} names no lane"
        );
    }
    assert_eq!(QuarantineLedger::parse(&register.render()), Ok(register));
}

#[test]
fn the_register_parser_refuses_malformed_lines() {
    let good = "C006 continuum-engine-explicit continuum-engine-reference defect_0123abcd\n";
    let parsed = QuarantineLedger::parse(good).expect("a well-formed line parses");
    assert_eq!(parsed.claims(), BTreeSet::from(["C006"]));
    for bad in [
        "C006 continuum-engine-explicit continuum-engine-reference\n",
        "C6 a b defect_x\n",
        "C006 a b notadefect\n",
        "C006 a b defect_\n",
        "C006 a b defect_x/y\n",
        "C006  a b defect_x\n",
    ] {
        assert!(QuarantineLedger::parse(bad).is_err(), "{bad:?} parsed");
    }
}

#[test]
fn the_gate_fails_closed_on_an_unknown_claim() {
    let ledger = QuarantineLedger::parse("C999 a b defect_x\n").expect("parses");
    assert_eq!(
        gate(&ledger, &claim_states()),
        vec![GateFinding::UnknownClaim {
            claim: "C999".to_owned()
        }]
    );
}

// ---------------------------------------------------------------------------
// the pieces
// ---------------------------------------------------------------------------

#[test]
fn ddmin_is_deterministic_and_one_minimal() {
    let items: Vec<u32> = (0..40).collect();
    let needs = |set: &[u32]| set.contains(&3) && set.contains(&31) && set.contains(&17);
    let first = ddmin(items.clone(), MinimizeBudget { tests: 10_000 }, needs);
    assert_eq!(first.kept, vec![3, 17, 31]);
    assert_eq!(first.minimality, Minimality::OneMinimal);
    let second = ddmin(items.clone(), MinimizeBudget { tests: 10_000 }, needs);
    assert_eq!(first, second);

    let starved = ddmin(items, MinimizeBudget { tests: 3 }, needs);
    assert_eq!(starved.minimality, Minimality::BudgetExhausted);
    assert!(needs(&starved.kept), "a starved result still reproduces");
    assert!(starved.tests <= 3);
}

/// Metamorphic relation "serialization round trip" (docs/19 §3): a model read back
/// into declarations and rebuilt has the model identity it started with, which is what
/// lets a minimized fixture stand for the model it was cut from.
#[test]
fn a_fixture_read_back_from_a_model_has_the_same_identity() {
    let model = diehard::model().expect("valid");
    let rebuilt = Fixture::from_model("die-hard", &model)
        .build()
        .expect("rebuilds");
    assert_eq!(rebuilt.identity(), model.identity());
    for seed in SEEDS {
        let model = generate(seed)
            .build()
            .expect("every generated fixture builds");
        let rebuilt = Fixture::from_model("again", &model)
            .build()
            .expect("rebuilds");
        assert_eq!(rebuilt.identity(), model.identity(), "seed {seed}");
    }
}

#[test]
fn a_harness_refuses_two_engines_in_one_slot() {
    let engines: [&dyn Engine; 2] = [&ReferenceEngine, &ReferenceEngine];
    assert!(Harness::new(&slots(&engines), &LANES, BUDGET, epochs(), MINIMIZE).is_err());
    let lone: [&dyn Engine; 1] = [&ReferenceEngine];
    let mut lanes = LANES.to_vec();
    lanes.push(continuum_corpus::differential::Lane {
        subject: REFERENCE,
        oracle: REFERENCE,
        subject_fields: Fields::ALL,
        oracle_fields: Fields::ALL,
        claims: &["C006"],
        basis: "self",
    });
    assert!(Harness::new(&slots(&lone), &lanes, BUDGET, epochs(), MINIMIZE).is_err());
    let spaced = [continuum_corpus::differential::Lane {
        subject: "a b",
        oracle: REFERENCE,
        subject_fields: Fields::ALL,
        oracle_fields: Fields::ALL,
        claims: &["C006"],
        basis: "space",
    }];
    assert!(Harness::new(&slots(&lone), &spaced, BUDGET, epochs(), MINIMIZE).is_err());
    let unclaimed = [continuum_corpus::differential::Lane {
        subject: CLOSURE,
        oracle: REFERENCE,
        subject_fields: Fields::ALL,
        oracle_fields: Fields::ALL,
        claims: &[],
        basis: "none",
    }];
    assert!(Harness::new(&slots(&lone), &unclaimed, BUDGET, epochs(), MINIMIZE).is_err());
}

/// Metamorphic relation "stable reordering of declarations" (docs/19 §3): reversing
/// the order of every declaration list of every fixture changes no model identity and
/// no lane's counts, so the harness's answers are functions of the models and not of
/// how they were written down.
#[test]
fn stable_reordering_of_declarations_preserves_every_lane() {
    let reversed: Vec<Fixture> = corpus()
        .into_iter()
        .map(|mut fixture| {
            let before = fixture.build().expect("builds").identity();
            fixture.variables.reverse();
            fixture.actions.reverse();
            fixture.initial_states.reverse();
            fixture.predicates.reverse();
            for initial in &mut fixture.initial_states {
                initial.reverse();
            }
            assert_eq!(fixture.build().expect("rebuilds").identity(), before);
            fixture
        })
        .collect();
    let harness = Harness::new(&slots(&trunk_engines()), &LANES, BUDGET, epochs(), MINIMIZE)
        .expect("harness");
    assert_eq!(
        harness.run(&reversed).summary(),
        harness.run(&corpus()).summary()
    );
}

// ---------------------------------------------------------------------------
// cr-2r0m24: engine failures and coverage enter the defect lifecycle
// ---------------------------------------------------------------------------

/// A typed `EngineError` on one side of a lane is an engine defect: a finding with a
/// `defect_*` report, a minimized fixture, and the lane's claims halted.
#[test]
fn a_one_sided_engine_error_is_reported_and_quarantined() {
    let report = with_mutant(Mutation::EngineErrorOnWide);
    assert_handled(&report, |d| {
        matches!(
            d,
            Disagreement::EngineFailure { side: Side::Subject, field, .. } if field == "reachable-states"
        )
    });
}

/// An `EngineError` is a fault, not a typed inconclusive: on both sides of a field it
/// is still a defect (`EngineFaultBoth`), never undecided (cr-2r0m24 round 2).
#[test]
fn the_same_engine_error_on_both_sides_is_a_defect() {
    let model = generate(1).build().expect("builds");
    let why = Inconclusive::new(InconclusiveReason::EngineError, "model does not evaluate");
    let names = || model.predicates().iter().map(|p| p.name().as_str());
    let left = Normalized::inconclusive(&why, names());
    let right = Normalized::inconclusive(&why, names());
    let comparison = compare(&model, &left, &right);
    assert!(
        comparison
            .disagreements
            .contains(&Disagreement::EngineFaultBoth {
                field: "reachable-states".to_owned(),
                subject: "model does not evaluate".to_owned(),
                oracle: "model does not evaluate".to_owned(),
            })
    );
    for predicate in model.predicates() {
        let field = format!("verdict:{}", predicate.name().as_str());
        assert!(
            comparison.disagreements.iter().any(
                |d| matches!(d, Disagreement::EngineFaultBoth { field: f, .. } if *f == field)
            )
        );
    }
    // One-sided, on one invariant only: a defect.
    let mut honest = ReferenceEngine.evaluate(&model, BUDGET);
    let first = honest
        .invariants
        .keys()
        .next()
        .cloned()
        .expect("a predicate");
    honest
        .invariants
        .insert(first.clone(), InvariantVerdict::Inconclusive(why.clone()));
    let comparison = compare(&model, &honest, &ReferenceEngine.evaluate(&model, BUDGET));
    assert!(
        comparison
            .disagreements
            .contains(&Disagreement::EngineFailure {
                side: Side::Subject,
                field: format!("verdict:{first}"),
                detail: "model does not evaluate".to_owned(),
            })
    );
}

/// A panicking adapter does not abort the run: the panic is caught, typed
/// `EngineError`, reported with a `defect_*` handle, minimized, and halts C006.
#[test]
fn a_panicking_engine_is_caught_reported_and_quarantined() {
    let report = with_mutant(Mutation::PanicOnNondeterministic);
    assert!(!report.findings.is_empty(), "the panic was not reported");
    for finding in &report.findings {
        assert_eq!(finding.lane.subject, EXPLICIT);
        assert!(finding.disagreements.iter().any(|d| matches!(
            d,
            Disagreement::EnginePanicked { side: Side::Subject, detail } if detail.contains("planted panic")
        )));
        // The panic is what is reported and minimized, not an echo of it.
        assert!(
            finding
                .report
                .disagreement
                .starts_with("engine-panicked side=subject"),
            "{}",
            finding.report.disagreement
        );
        assert!(
            finding
                .report
                .reproduction
                .disagreement
                .starts_with("engine-panicked")
        );
        assert_eq!(
            finding.disagreements.len(),
            1,
            "{:?}",
            finding.disagreements
        );
        assert_eq!(finding.handle, finding.report.handle());
        assert!(
            finding
                .report
                .encode()
                .contains("\nsubject.build planted-mutant/panic-on-nondeterministic\n")
        );
        // Minimized to a fixture that still has a nondeterministic action, and no larger.
        assert!(finding.minimized.size() <= finding.report.original.size);
        assert!(
            finding
                .minimized
                .actions
                .iter()
                .any(|a| a.outcomes.len() > 1)
        );
    }
    assert!(
        report
            .findings
            .iter()
            .any(|f| f.minimized.size() < f.report.original.size)
    );
    assert_eq!(report.ledger.claims(), BTreeSet::from(["C006"]));
    assert!(
        gate(&report.ledger, &claim_states()).contains(&GateFinding::AssertedPositive {
            claim: "C006".to_owned(),
            state: ClaimState::Observed,
        })
    );
}

/// The invariants an engine must judge come from the model: an engine that declares
/// it judges invariants and answers none is a coverage defect, not a clean lane.
#[test]
fn an_engine_answering_no_invariants_is_a_coverage_defect() {
    let report = with_mutant(Mutation::DropAllVerdicts);
    assert_handled(&report, |d| {
        matches!(
            d,
            Disagreement::InvariantCoverage {
                side: Side::Subject,
                ..
            }
        )
    });
    // Directly: an empty map under Fields::ALL, with the model declaring predicates.
    let model = diehard::model().expect("valid");
    let mut empty = ReferenceEngine.evaluate(&model, BUDGET);
    empty.invariants.clear();
    let honest = ReferenceEngine.evaluate(&model, BUDGET);
    assert!(
        compare(&model, &empty, &honest)
            .disagreements
            .iter()
            .any(|d| matches!(
                d,
                Disagreement::InvariantCoverage {
                    side: Side::Subject,
                    ..
                }
            ))
    );
    // A projection-only engine is held to its own declaration: answering an invariant
    // is a coverage defect too, and answering none is clean.
    assert!(
        compare_fields(
            &model,
            &honest,
            Fields::PROJECTION_ONLY,
            &honest,
            Fields::ALL
        )
        .disagreements
        .iter()
        .any(|d| matches!(
            d,
            Disagreement::InvariantCoverage {
                side: Side::Subject,
                ..
            }
        ))
    );
    assert!(
        compare_fields(
            &model,
            &empty,
            Fields::PROJECTION_ONLY,
            &honest,
            Fields::ALL
        )
        .disagreements
        .is_empty()
    );
    // A declared projection that is not provided is a coverage defect.
    let mut blind = honest.clone();
    blind.projection = Projection::NotProvided;
    assert!(compare(&model, &blind, &honest).disagreements.contains(
        &Disagreement::ProjectionCoverage {
            side: Side::Subject
        }
    ));
}

/// A fixture whose model declares no predicate is its own counted category: its
/// verdict fields are empty by the model's shape, and the lane says so.
#[test]
fn a_fixture_without_invariants_is_counted_not_read_as_agreement() {
    let mut fixture = generate(3);
    fixture.predicates.clear();
    fixture.label = "no-invariants".to_owned();
    let engines: [&dyn Engine; 2] = [&ReferenceEngine, &ClosureOracle];
    let report = Harness::new(&slots(&engines), &LANES, BUDGET, epochs(), MINIMIZE)
        .expect("harness")
        .run(&[fixture, generate(4)]);
    assert!(report.findings.is_empty(), "{}", report.summary());
    let LaneStatus::Ran(counts) = lane(&report, CLOSURE, REFERENCE) else {
        panic!("the closure lane ran")
    };
    assert_eq!(counts.fixtures, 2);
    assert_eq!(counts.no_invariant_fixtures, 1);
}

/// A panic in a projection-only engine is reported as the panic, and an engine error
/// on the oracle side is attributed to the oracle.
#[test]
fn a_panic_in_a_projection_only_engine_is_reported_as_the_panic() {
    struct PanickingOracle;
    impl Engine for PanickingOracle {
        fn identity(&self) -> continuum_corpus::differential::EngineIdentity {
            SemanticOracleEngine.identity()
        }
        fn fields(&self) -> Fields {
            Fields::PROJECTION_ONLY
        }
        fn evaluate(
            &self,
            model: &continuum_engine_reference::model::Model,
            budget: Budget,
        ) -> Normalized {
            assert!(model.variables().len() < 3, "planted oracle panic");
            SemanticOracleEngine.evaluate(model, budget)
        }
    }
    let engines: [&dyn Engine; 2] = [&ReferenceEngine, &PanickingOracle];
    let report = run(&engines, BUDGET);
    assert!(!report.findings.is_empty());
    for finding in &report.findings {
        assert_eq!(finding.lane.subject, SEMANTIC_ORACLE);
        assert!(
            finding
                .report
                .disagreement
                .starts_with("engine-panicked side=subject"),
            "{}",
            finding.report.disagreement
        );
        assert_eq!(
            finding.disagreements.len(),
            1,
            "{:?}",
            finding.disagreements
        );
    }
    assert_eq!(report.ledger.claims(), BTreeSet::from(["C005"]));

    let model = generate(1).build().expect("builds");
    let honest = ReferenceEngine.evaluate(&model, BUDGET);
    let mut broken = honest.clone();
    broken.projection = Projection::Inconclusive(Inconclusive::new(
        InconclusiveReason::EngineError,
        "oracle broke",
    ));
    let comparison = compare(&model, &honest, &broken);
    assert!(comparison.disagreements.iter().any(|d| matches!(
        d,
        Disagreement::EngineFailure { side: Side::Oracle, field, .. } if field == "reachable-states"
    )));
}

/// A `Holds` beside the same engine's errored exploration is unexplored. When the
/// other side errored the same way, the projection field is a two-sided fault too.
#[test]
fn a_holds_beside_an_errored_exploration_is_unexplored() {
    let model = generate(1).build().expect("builds");
    let mut answer = ReferenceEngine.evaluate(&model, BUDGET);
    answer.projection =
        Projection::Inconclusive(Inconclusive::new(InconclusiveReason::EngineError, "broke"));
    for verdict in answer.invariants.values_mut() {
        *verdict = InvariantVerdict::Holds;
    }
    let comparison = compare(&model, &answer, &answer);
    assert!(comparison.disagreements.iter().any(|d| matches!(
        d,
        Disagreement::UnexploredHolds {
            side: Side::Subject,
            ..
        }
    )));
    assert!(comparison.disagreements.iter().any(|d| matches!(
        d,
        Disagreement::EngineFaultBoth { field, .. } if field == "reachable-states"
    )));
    assert!(
        comparison.undecided.is_empty(),
        "{:?}",
        comparison.undecided
    );
}

/// Two faulty engines on one lane: every fixture is a defect with a `defect_*` report,
/// a minimized fixture, and the lane's claim halted, even though both sides report the
/// same typed `EngineError`.
#[test]
fn engine_faults_on_both_sides_are_reported_and_quarantined() {
    struct Faulty(&'static str);
    impl Engine for Faulty {
        fn identity(&self) -> continuum_corpus::differential::EngineIdentity {
            continuum_corpus::differential::EngineIdentity {
                slot: self.0.to_owned(),
                build: "planted-fault/both".to_owned(),
            }
        }
        fn fields(&self) -> Fields {
            Fields::ALL
        }
        fn evaluate(
            &self,
            model: &continuum_engine_reference::model::Model,
            _: Budget,
        ) -> Normalized {
            Normalized::inconclusive(
                &Inconclusive::new(InconclusiveReason::EngineError, "planted fault"),
                model.predicates().iter().map(|p| p.name().as_str()),
            )
        }
    }
    let (subject, oracle) = (Faulty(EXPLICIT), Faulty(REFERENCE));
    let engines: [&dyn Engine; 2] = [&subject, &oracle];
    let report = run(&engines, BUDGET);
    // One finding per corpus fixture; faults on other fields met while minimizing are
    // reported as their own findings, marked as such.
    assert_eq!(
        report
            .findings
            .iter()
            .filter(|f| f.found_while_minimizing.is_none())
            .count(),
        CORPUS_LEN,
        "{}",
        report.summary()
    );
    for finding in &report.findings {
        assert_eq!(finding.lane.subject, EXPLICIT);
        assert!(
            finding.report.disagreement.starts_with("engine-fault-both"),
            "{}",
            finding.report.disagreement
        );
        assert_eq!(finding.handle, finding.report.handle());
        assert!(finding.minimized.size() <= finding.report.original.size);
    }
    // A fault is the engine's: both faulting slots halt every lane they serve. The
    // explicit slot serves C006; the reference slot serves every trunk lane.
    let mut expected = reference_lane_claims();
    expected.insert("C006");
    assert_eq!(report.ledger.claims(), expected);
    assert!(
        gate(&report.ledger, &claim_states()).contains(&GateFinding::AssertedPositive {
            claim: "C006".to_owned(),
            state: ClaimState::Observed,
        })
    );
}

/// The typed inconclusives stay undecided on both sides: two engines that both report
/// `Unsupported` (or both `ResourceExhausted`) produce no finding, and the lane counts
/// the field.
#[test]
fn a_typed_inconclusive_on_both_sides_is_undecided() {
    let model = generate(1).build().expect("builds");
    for reason in [
        InconclusiveReason::Unsupported,
        InconclusiveReason::ResourceExhausted,
    ] {
        let why = Inconclusive::new(reason, "typed");
        let names = || model.predicates().iter().map(|p| p.name().as_str());
        let comparison = compare(
            &model,
            &Normalized::inconclusive(&why, names()),
            &Normalized::inconclusive(&why, names()),
        );
        assert!(
            comparison.disagreements.is_empty(),
            "{reason}: {:?}",
            comparison.disagreements
        );
        // The projection, the undefined-action field, and every verdict.
        assert_eq!(comparison.undecided.len(), 2 + model.predicates().len());
        assert!(
            comparison
                .undecided
                .iter()
                .all(|u| u.subject == Some(reason) && u.oracle == Some(reason))
        );
    }
    struct Unsupporting(&'static str);
    impl Engine for Unsupporting {
        fn identity(&self) -> continuum_corpus::differential::EngineIdentity {
            continuum_corpus::differential::EngineIdentity {
                slot: self.0.to_owned(),
                build: "planted/unsupported".to_owned(),
            }
        }
        fn fields(&self) -> Fields {
            Fields::ALL
        }
        fn evaluate(
            &self,
            model: &continuum_engine_reference::model::Model,
            _: Budget,
        ) -> Normalized {
            Normalized::inconclusive(
                &Inconclusive::new(InconclusiveReason::Unsupported, "outside the fragment"),
                model.predicates().iter().map(|p| p.name().as_str()),
            )
        }
    }
    let (subject, oracle) = (Unsupporting(EXPLICIT), Unsupporting(REFERENCE));
    let engines: [&dyn Engine; 2] = [&subject, &oracle];
    let report = run(&engines, BUDGET);
    assert!(report.findings.is_empty(), "{}", report.summary());
    assert!(report.ledger.is_empty());
    let LaneStatus::Ran(counts) = lane(&report, EXPLICIT, REFERENCE) else {
        panic!("the explicit lane ran")
    };
    // One projection field per fixture, plus every predicate's verdict field.
    assert!(
        counts
            .undecided
            .get("Unsupported/Unsupported")
            .copied()
            .unwrap_or(0)
            > 65,
        "{counts:?}"
    );
    assert_eq!(counts.undecided.len(), 1, "{counts:?}");
}

/// A panic on both sides is two panics and nothing else; a panic beside the other
/// side's own `EngineError` reports that fault against the other side.
#[test]
fn panic_attribution_holds_with_a_faulty_or_panicking_other_side() {
    struct Panics(&'static str);
    impl Engine for Panics {
        fn identity(&self) -> continuum_corpus::differential::EngineIdentity {
            continuum_corpus::differential::EngineIdentity {
                slot: self.0.to_owned(),
                build: "planted/panics".to_owned(),
            }
        }
        fn fields(&self) -> Fields {
            Fields::ALL
        }
        fn evaluate(&self, _: &continuum_engine_reference::model::Model, _: Budget) -> Normalized {
            panic!("planted panic on {}", self.0)
        }
    }
    struct Faults(&'static str);
    impl Engine for Faults {
        fn identity(&self) -> continuum_corpus::differential::EngineIdentity {
            continuum_corpus::differential::EngineIdentity {
                slot: self.0.to_owned(),
                build: "planted/faults".to_owned(),
            }
        }
        fn fields(&self) -> Fields {
            Fields::ALL
        }
        fn evaluate(
            &self,
            model: &continuum_engine_reference::model::Model,
            _: Budget,
        ) -> Normalized {
            Normalized::inconclusive(
                &Inconclusive::new(InconclusiveReason::EngineError, "own fault"),
                model.predicates().iter().map(|p| p.name().as_str()),
            )
        }
    }
    let corpus = [generate(1)];
    let run_on = |engines: &[&dyn Engine]| {
        Harness::new(&slots(engines), &LANES, BUDGET, epochs(), MINIMIZE)
            .expect("harness")
            .run(&corpus)
    };

    let (a, b) = (Panics(EXPLICIT), Panics(REFERENCE));
    let report = run_on(&[&a, &b]);
    let finding = report.findings.first().expect("a finding");
    assert_eq!(
        finding.disagreements.len(),
        2,
        "{:?}",
        finding.disagreements
    );
    assert!(
        finding
            .disagreements
            .iter()
            .all(|d| matches!(d, Disagreement::EnginePanicked { .. }))
    );

    let (a, b) = (Panics(EXPLICIT), Faults(REFERENCE));
    let report = run_on(&[&a, &b]);
    let finding = report.findings.first().expect("a finding");
    assert!(
        finding
            .report
            .disagreement
            .starts_with("engine-panicked side=subject")
    );
    assert!(
        !finding
            .disagreements
            .iter()
            .any(|d| matches!(d, Disagreement::EngineFaultBoth { .. }))
    );
    let faults = finding
        .disagreements
        .iter()
        .filter(|d| matches!(d, Disagreement::EngineFailure { side: Side::Oracle, detail, .. } if detail == "own fault"))
        .count();
    let model = generate(1).build().expect("builds");
    assert_eq!(
        faults,
        1 + model.predicates().len(),
        "{:?}",
        finding.disagreements
    );
    assert_eq!(finding.disagreements.len(), 2 + model.predicates().len());
}

// ---------------------------------------------------------------------------
// cr-2r0m24 round 3: contract faults at assembly enter the defect lifecycle
// ---------------------------------------------------------------------------

/// An engine whose every method panics.
struct PanicsEverywhere {
    identity_panics: bool,
}

impl Engine for PanicsEverywhere {
    fn identity(&self) -> continuum_corpus::differential::EngineIdentity {
        if self.identity_panics {
            panic!("planted identity panic");
        }
        ReferenceEngine.identity()
    }
    fn fields(&self) -> Fields {
        panic!("planted fields panic")
    }
    fn evaluate(&self, _: &continuum_engine_reference::model::Model, _: Budget) -> Normalized {
        panic!("never evaluated")
    }
}

fn reference_lane_claims() -> BTreeSet<&'static str> {
    LANES
        .iter()
        .filter(|l| l.subject == REFERENCE || l.oracle == REFERENCE)
        .flat_map(|l| l.claims.iter().copied())
        .collect()
}

/// A panic in `identity()` or `fields()` is a contract fault attributed to the slot the
/// configuration plugged the engine into: a `defect_*` report, and every claim of every
/// lane that slot serves quarantined. The engine is never evaluated.
#[test]
fn a_panic_in_identity_or_fields_is_a_defect_quarantining_every_lane_of_its_slot() {
    for (identity_panics, fault) in [
        (true, ContractFault::IdentityPanicked),
        (false, ContractFault::FieldsPanicked),
    ] {
        let broken = PanicsEverywhere { identity_panics };
        let engines: Vec<(&str, &dyn Engine)> = vec![
            (REFERENCE, &broken),
            (CLOSURE, &ClosureOracle),
            (KERNEL, &KernelEngine),
            (SEMANTIC_ORACLE, &SemanticOracleEngine),
        ];
        let report = Harness::new(&engines, &LANES, BUDGET, epochs(), MINIMIZE)
            .expect("an engine fault is not a configuration error")
            .run(&corpus());
        assert_eq!(report.contract_findings.len(), 1, "{}", report.summary());
        let finding = &report.contract_findings[0];
        assert_eq!(finding.report.slot, REFERENCE);
        assert_eq!(finding.report.fault, fault);
        assert!(finding.report.detail.contains("planted"));
        assert_eq!(finding.handle, finding.report.handle());
        assert!(finding.handle.starts_with("defect_"));
        assert!(
            finding
                .report
                .encode()
                .starts_with("format continuum-corpus-contract-defect/1\n")
        );
        // Every lane the reference serves is halted under this handle and did not run.
        assert_eq!(report.ledger.claims(), reference_lane_claims());
        assert!(report.ledger.entries().all(|q| q.defect == finding.handle));
        for lane in LANES
            .iter()
            .filter(|l| l.subject == REFERENCE || l.oracle == REFERENCE)
        {
            assert!(finding.lanes.contains(lane));
            let (_, status) = report
                .lanes
                .iter()
                .find(|(l, _)| l == lane)
                .expect("in table");
            assert_eq!(
                status,
                &LaneStatus::Faulted {
                    slots: vec![REFERENCE.to_owned()]
                }
            );
            for claim in lane.claims {
                assert!(report.ledger.entries().any(|q| q.claim == *claim
                    && q.subject == lane.subject
                    && q.oracle == lane.oracle));
            }
        }
        // The halting rule fails: C006, C018 and C023 are OBSERVED.
        let halted = halts(&report.ledger, &register(), &claim_states());
        assert!(
            halted
                .iter()
                .any(|f| matches!(f, GateFinding::Unregistered { .. }))
        );
        assert!(halted.contains(&GateFinding::AssertedPositive {
            claim: "C006".to_owned(),
            state: ClaimState::Observed
        }));
    }
}

/// An engine that names another slot, or declares fields a lane it serves does not
/// allow, is a contract fault of the slot it was plugged into, not a refusal.
#[test]
fn a_contract_mismatch_is_a_defect_not_a_refusal() {
    struct VerdictBlind;
    impl Engine for VerdictBlind {
        fn identity(&self) -> continuum_corpus::differential::EngineIdentity {
            Mutant(Mutation::DropAllVerdicts).identity()
        }
        fn fields(&self) -> Fields {
            Fields::PROJECTION_ONLY
        }
        fn evaluate(
            &self,
            model: &continuum_engine_reference::model::Model,
            budget: Budget,
        ) -> Normalized {
            Mutant(Mutation::DropAllVerdicts).evaluate(model, budget)
        }
    }
    let engines: Vec<(&str, &dyn Engine)> =
        vec![(REFERENCE, &ReferenceEngine), (EXPLICIT, &VerdictBlind)];
    let report = Harness::new(&engines, &LANES, BUDGET, epochs(), MINIMIZE)
        .expect("harness")
        .run(&corpus());
    assert_eq!(report.contract_findings.len(), 1);
    assert_eq!(
        report.contract_findings[0].report.fault,
        ContractFault::FieldsMismatch
    );
    let finding = &report.contract_findings[0];
    assert_eq!(finding.report.slot, EXPLICIT);
    assert_eq!(finding.handle, finding.report.handle());
    assert!(
        finding
            .report
            .encode()
            .contains("\nbuild.present planted-mutant/drop-all-verdicts\n")
    );
    assert_eq!(
        lane(&report, EXPLICIT, REFERENCE),
        &LaneStatus::Faulted {
            slots: vec![EXPLICIT.to_owned()]
        }
    );
    assert_eq!(report.ledger.claims(), BTreeSet::from(["C006"]));

    // Plugged under a slot it does not name.
    let engines: Vec<(&str, &dyn Engine)> =
        vec![(REFERENCE, &ReferenceEngine), (EXPLICIT, &ClosureOracle)];
    let report = Harness::new(&engines, &LANES, BUDGET, epochs(), MINIMIZE)
        .expect("harness")
        .run(&corpus());
    assert_eq!(report.contract_findings.len(), 1);
    assert_eq!(
        report.contract_findings[0].report.fault,
        ContractFault::SlotMismatch
    );
    assert_eq!(report.ledger.claims(), BTreeSet::from(["C006"]));
}

/// Every `HarnessError` is a configuration error decided before any engine code runs.
/// The match is exhaustive, so a new variant must be added here; each variant is
/// produced with engines that panic on every call, and none of them is called.
#[test]
fn every_harness_error_is_engine_free() {
    use continuum_corpus::differential::HarnessError;
    use std::cell::Cell;
    struct Counting<'c>(&'c Cell<usize>);
    impl Engine for Counting<'_> {
        fn identity(&self) -> continuum_corpus::differential::EngineIdentity {
            self.0.set(self.0.get() + 1);
            panic!("engine code ran")
        }
        fn fields(&self) -> Fields {
            self.0.set(self.0.get() + 1);
            panic!("engine code ran")
        }
        fn evaluate(&self, _: &continuum_engine_reference::model::Model, _: Budget) -> Normalized {
            self.0.set(self.0.get() + 1);
            panic!("engine code ran")
        }
    }
    let calls = Cell::new(0);
    let engine = Counting(&calls);
    let lane = |subject: &'static str, oracle: &'static str, claims: &'static [&'static str]| {
        continuum_corpus::differential::Lane {
            subject,
            oracle,
            subject_fields: Fields::ALL,
            oracle_fields: Fields::ALL,
            claims,
            basis: "test",
        }
    };
    type Case<'e> = (
        Vec<(&'e str, &'e dyn Engine)>,
        Vec<continuum_corpus::differential::Lane>,
    );
    let cases: Vec<Case<'_>> = vec![
        (
            vec![(REFERENCE, &engine), (REFERENCE, &engine)],
            LANES.to_vec(),
        ),
        (vec![("a b", &engine)], LANES.to_vec()),
        (
            vec![(REFERENCE, &engine)],
            vec![lane(REFERENCE, REFERENCE, &["C006"])],
        ),
        (
            vec![(REFERENCE, &engine)],
            vec![lane(CLOSURE, REFERENCE, &[])],
        ),
        (
            vec![(REFERENCE, &engine)],
            vec![
                lane(CLOSURE, REFERENCE, &["C006"]),
                lane(CLOSURE, REFERENCE, &["C005"]),
            ],
        ),
    ];
    let mut seen = BTreeSet::new();
    for (engines, lanes) in cases {
        let error = match Harness::new(&engines, &lanes, BUDGET, epochs(), MINIMIZE) {
            Err(error) => error,
            Ok(_) => panic!("expected a configuration error"),
        };
        seen.insert(match error {
            HarnessError::DuplicateSlot(_) => "duplicate-slot",
            HarnessError::BadSlot(_) => "bad-slot",
            HarnessError::SelfLane(_) => "self-lane",
            HarnessError::BadClaims(_) => "bad-claims",
            HarnessError::DuplicateLane(_) => "duplicate-lane",
        });
    }
    assert_eq!(seen.len(), 5, "every variant is produced: {seen:?}");
    assert_eq!(
        calls.get(),
        0,
        "no engine code ran while deciding a HarnessError"
    );
}

/// A fault met only while minimizing another disagreement is not dropped: it gets its
/// own report (marked as found while minimizing), and its slot's lanes are halted
/// under that report.
#[test]
fn a_fault_met_while_minimizing_is_reported() {
    let report = with_mutant(Mutation::DropSuccessorPanicOnSingleAction);
    let side: Vec<_> = report
        .findings
        .iter()
        .filter(|f| f.found_while_minimizing.is_some())
        .collect();
    assert!(!side.is_empty(), "{}", report.summary());
    for finding in side {
        assert!(
            finding
                .report
                .disagreement
                .starts_with("engine-panicked side=subject"),
            "{}",
            finding.report.disagreement
        );
        let parent = finding.found_while_minimizing.as_ref().expect("marked");
        assert!(
            report
                .findings
                .iter()
                .any(|f| &f.handle == parent && f.found_while_minimizing.is_none())
        );
        assert!(
            report
                .ledger
                .entries()
                .any(|q| q.defect == finding.handle && q.claim == "C006")
        );
        assert_eq!(finding.handle, finding.report.handle());
    }
}

/// When an engine fault and a semantic disagreement meet on one fixture, the fault is
/// what the report names and minimizes, and the report lists every claim the fault
/// halts: every lane of the faulting slot.
#[test]
fn a_fault_is_reported_before_a_semantic_disagreement() {
    struct FaultAndLoss;
    impl Engine for FaultAndLoss {
        fn identity(&self) -> continuum_corpus::differential::EngineIdentity {
            continuum_corpus::differential::EngineIdentity {
                slot: REFERENCE.to_owned(),
                build: "planted/fault-and-loss".to_owned(),
            }
        }
        fn fields(&self) -> Fields {
            Fields::ALL
        }
        fn evaluate(
            &self,
            model: &continuum_engine_reference::model::Model,
            budget: Budget,
        ) -> Normalized {
            let mut answer = Mutant(Mutation::DropSuccessor).evaluate(model, budget);
            if let Some(verdict) = answer.invariants.values_mut().next() {
                *verdict = InvariantVerdict::Inconclusive(Inconclusive::new(
                    InconclusiveReason::EngineError,
                    "fault",
                ));
            }
            answer
        }
    }
    // The faulty engine sits in the reference slot, against the honest closure oracle.
    let engines: Vec<(&str, &dyn Engine)> =
        vec![(REFERENCE, &FaultAndLoss), (CLOSURE, &ClosureOracle)];
    let report = Harness::new(&engines, &LANES, BUDGET, epochs(), MINIMIZE)
        .expect("harness")
        .run(&corpus());
    let mixed: Vec<_> = report
        .findings
        .iter()
        .filter(|f| {
            f.found_while_minimizing.is_none()
                && f.disagreements
                    .iter()
                    .any(|d| matches!(d, Disagreement::ReachableStates { .. }))
        })
        .collect();
    assert!(!mixed.is_empty(), "{}", report.summary());
    for finding in mixed {
        assert!(
            finding
                .report
                .disagreement
                .starts_with("engine-failure side=oracle"),
            "{}",
            finding.report.disagreement
        );
        let listed: BTreeSet<&str> = finding.report.claims.iter().map(String::as_str).collect();
        assert_eq!(listed, reference_lane_claims());
        for entry in report
            .ledger
            .entries()
            .filter(|q| q.defect == finding.handle)
        {
            assert!(listed.contains(entry.claim.as_str()));
        }
    }
}

/// cr-2r0m24 round 4: quarantine follows the first observation of a fault. The
/// reference slot holds an engine that faults exactly once, on the first minimization
/// candidate it ever sees, and is the honest reference otherwise. The explicit lane's
/// lost-transition mutant makes the harness minimize, the reference faults once there,
/// and its recheck is clean. The fault is still reported (labelled not-reproduced),
/// and every lane the reference slot serves (C005, C006, C018, C023) is quarantined.
#[test]
fn a_transient_fault_quarantines_its_slot_from_the_first_observation() {
    use std::cell::Cell;
    struct Transient {
        corpus: BTreeSet<Vec<u8>>,
        fired: Cell<bool>,
    }
    impl Engine for Transient {
        fn identity(&self) -> continuum_corpus::differential::EngineIdentity {
            ReferenceEngine.identity()
        }
        fn fields(&self) -> Fields {
            Fields::ALL
        }
        fn evaluate(
            &self,
            model: &continuum_engine_reference::model::Model,
            budget: Budget,
        ) -> Normalized {
            if !self.corpus.contains(model.identity().as_bytes()) && !self.fired.get() {
                self.fired.set(true);
                panic!("planted transient fault");
            }
            ReferenceEngine.evaluate(model, budget)
        }
    }
    let transient = Transient {
        corpus: corpus()
            .iter()
            .map(|f| f.build().expect("builds").identity().as_bytes().to_vec())
            .collect(),
        fired: Cell::new(false),
    };
    let mutant = Mutant(Mutation::DropSuccessor);
    let engines: Vec<(&str, &dyn Engine)> = vec![(REFERENCE, &transient), (EXPLICIT, &mutant)];
    let report = Harness::new(&engines, &LANES, BUDGET, epochs(), MINIMIZE)
        .expect("harness")
        .run(&corpus());
    assert!(transient.fired.get(), "the fault fired");
    let transient_findings: Vec<_> = report
        .findings
        .iter()
        .filter(|f| {
            f.report
                .disagreement
                .starts_with("engine-panicked side=oracle")
        })
        .collect();
    assert_eq!(transient_findings.len(), 1, "{}", report.summary());
    let finding = transient_findings[0];
    assert!(finding.found_while_minimizing.is_some());
    assert_eq!(
        finding.report.reproduction.minimality,
        Minimality::NotReproduced,
        "the recheck is clean, and the report says so"
    );
    assert!(finding.faulted_slots.contains(REFERENCE));
    assert!(report.faulted_slots.contains(REFERENCE));
    // Every lane the reference slot serves is quarantined under the transient report,
    // and the report lists every one of those claims.
    let lanes: Vec<_> = LANES
        .iter()
        .filter(|l| l.subject == REFERENCE || l.oracle == REFERENCE)
        .collect();
    assert_eq!(lanes.len(), 4);
    for lane in lanes {
        for claim in lane.claims {
            assert!(
                report.ledger.entries().any(|q| q.defect == finding.handle
                    && q.claim == *claim
                    && q.subject == lane.subject
                    && q.oracle == lane.oracle),
                "{claim} on {} vs {}",
                lane.subject,
                lane.oracle
            );
            assert!(finding.report.claims.iter().any(|c| c == claim));
        }
    }
    assert_eq!(report.ledger.claims(), reference_lane_claims());
    // Attribution: the lost-transition report that was being minimized does not claim
    // the reference's fault; that fault's own report does.
    let parent = report
        .findings
        .iter()
        .find(|f| Some(&f.handle) == finding.found_while_minimizing.as_ref())
        .expect("the parent finding");
    assert!(
        parent.faulted_slots.is_empty(),
        "{:?}",
        parent.faulted_slots
    );
    assert_faults_are_evidenced(&report);
}

/// Every slot the run holds faulted is named by a report that records a fault of that
/// slot: a contract defect, or a finding whose own evidence shows it.
fn assert_faults_are_evidenced(report: &RunReport) {
    for slot in &report.faulted_slots {
        let contract = report
            .contract_findings
            .iter()
            .any(|f| &f.report.slot == slot);
        let evaluation = report.findings.iter().any(|f| {
            f.faulted_slots.contains(slot)
                && ["engine-panicked", "engine-failure", "engine-fault-both"]
                    .iter()
                    .any(|p| f.report.disagreement.starts_with(p))
        });
        assert!(
            contract || evaluation,
            "{slot} is held faulted with no report of its fault"
        );
    }
}

// ---------------------------------------------------------------------------
// cr-2r0m24 round 5: every quarantine rests on a report that encodes its evidence
// ---------------------------------------------------------------------------

/// The slots whose fault a report's **encoding** carries, read from the encoded text.
fn encoded_fault_slots(encoded: &str) -> BTreeSet<String> {
    encoded
        .lines()
        .filter_map(|line| line.strip_prefix("fault.slot "))
        .map(str::to_owned)
        .collect()
}

/// quarantine(slot, claim) ⇒ ∃ report R, R.handle = entry.defect, and R's encoding
/// shows either that lane's disagreement (its subject and oracle slots) or a fault of
/// a slot the entry's lane serves.
fn assert_ledger_evidenced(report: &RunReport) {
    /// A report's encoded lane (subject, oracle), if it has one, and its fault slots.
    type Evidence = (Option<(String, String)>, BTreeSet<String>);
    let mut evidence: BTreeMap<String, Evidence> = BTreeMap::new();
    for finding in &report.findings {
        let encoded = finding.report.encode();
        assert_eq!(
            finding.handle,
            finding.report.handle(),
            "handle is the encoding's hash"
        );
        let slot = |key: &str| {
            encoded
                .lines()
                .find_map(|l| l.strip_prefix(key))
                .expect("slot line")
                .to_owned()
        };
        // Each encoded fault names the slot of the side it is about.
        let (subject_slot, oracle_slot) = (slot("subject.slot "), slot("oracle.slot "));
        let lines: Vec<&str> = encoded.lines().collect();
        for pair in lines.windows(2) {
            let (Some(fault_slot), Some(evidence_text)) = (
                pair[0].strip_prefix("fault.slot "),
                pair[1].strip_prefix("fault.evidence "),
            ) else {
                continue;
            };
            let both = evidence_text.starts_with("engine-fault-both");
            let allowed_subject = both || evidence_text.contains("side=subject");
            let allowed_oracle = both || evidence_text.contains("side=oracle");
            assert!(
                (fault_slot == subject_slot && allowed_subject)
                    || (fault_slot == oracle_slot && allowed_oracle),
                "fault line names {fault_slot} for evidence {evidence_text}"
            );
        }
        evidence.insert(
            finding.handle.clone(),
            (
                Some((subject_slot, oracle_slot)),
                encoded_fault_slots(&encoded),
            ),
        );
    }
    for finding in &report.contract_findings {
        let encoded = finding.report.encode();
        assert_eq!(finding.handle, finding.report.handle());
        let slot = encoded
            .lines()
            .find_map(|l| l.strip_prefix("slot "))
            .expect("slot line")
            .to_owned();
        evidence.insert(finding.handle.clone(), (None, BTreeSet::from([slot])));
    }
    for entry in report.ledger.entries() {
        let (lane, faulted) = evidence
            .get(&entry.defect)
            .unwrap_or_else(|| panic!("{entry:?} names no report of this run"));
        let lane_evidence = lane
            .as_ref()
            .is_some_and(|(s, o)| *s == entry.subject && *o == entry.oracle);
        let fault_evidence = faulted.contains(&entry.subject) || faulted.contains(&entry.oracle);
        assert!(
            lane_evidence || fault_evidence,
            "{entry:?} rests on a report that encodes neither its lane's disagreement nor a fault of its slots"
        );
    }
    for slot in &report.faulted_slots {
        assert!(
            evidence.values().any(|(_, f)| f.contains(slot)),
            "{slot} is held faulted but no report encodes its fault"
        );
    }
}

/// The invariant, over the corpus with the trunk engines and with every planted
/// mutant, budget stop, fault and contract failure the suite plants.
#[test]
fn every_ledger_entry_names_a_report_encoding_its_evidence() {
    let mut runs = vec![run(&trunk_engines(), BUDGET), run(&trunk_engines(), TIGHT)];
    for mutation in [
        Mutation::DropSuccessor,
        Mutation::ForgeWitness,
        Mutation::LaunderBudget,
        Mutation::EngineErrorOnWide,
        Mutation::PanicOnNondeterministic,
        Mutation::DropAllVerdicts,
        Mutation::DropSuccessorPanicOnSingleAction,
        Mutation::IgnoreDefinedness,
        Mutation::SwapUndefinedKind,
        Mutation::DropUndefinedAction,
        Mutation::ForgeUndefinedState,
    ] {
        runs.push(with_mutant(mutation));
    }
    let broken = PanicsEverywhere {
        identity_panics: true,
    };
    let engines: Vec<(&str, &dyn Engine)> = vec![(REFERENCE, &broken), (CLOSURE, &ClosureOracle)];
    runs.push(
        Harness::new(&engines, &LANES, BUDGET, epochs(), MINIMIZE)
            .expect("harness")
            .run(&corpus()),
    );
    // Evaluation faults in the reference slot, which serves four lanes: most of its
    // entries rest on encoded fault evidence, not on the report's own lane.
    let (explicit, reference) = (FaultyIn(EXPLICIT), FaultyIn(REFERENCE));
    let engines: Vec<(&str, &dyn Engine)> = vec![(EXPLICIT, &explicit), (REFERENCE, &reference)];
    runs.push(
        Harness::new(&engines, &LANES, BUDGET, epochs(), MINIMIZE)
            .expect("harness")
            .run(&corpus()),
    );
    let mut entries = 0;
    let mut by_encoded_fault = 0;
    for report in &runs {
        assert_ledger_evidenced(report);
        entries += report.ledger.entries().count();
        for entry in report.ledger.entries() {
            if let Some(finding) = report.findings.iter().find(|f| f.handle == entry.defect) {
                let own_lane =
                    finding.lane.subject == entry.subject && finding.lane.oracle == entry.oracle;
                if !own_lane {
                    by_encoded_fault += 1;
                }
            }
        }
    }
    assert!(entries > 50, "the property is exercised: {entries} entries");
    assert!(
        by_encoded_fault > 0,
        "some defect-report entries rest on encoded fault evidence alone"
    );
}

/// Two slots fault around one candidate. The explicit engine faults once, on the
/// first minimization candidate it sees (candidate C). The reference is honest on C in
/// that same comparison and faults on C from then on, so the explicit fault's
/// **recheck** shows a reference fault. On 001ca3de the explicit fault's report took
/// that reference fault into its quarantine without encoding it. Now every quarantine
/// rests on a report that encodes that slot's own fault.
#[test]
fn two_transient_slots_each_rest_on_their_own_encoded_fault() {
    use std::cell::{Cell, RefCell};
    let corpus_ids: BTreeSet<Vec<u8>> = corpus()
        .iter()
        .map(|f| f.build().expect("builds").identity().as_bytes().to_vec())
        .collect();
    let candidate: RefCell<Option<Vec<u8>>> = RefCell::new(None);
    struct Subject<'c> {
        corpus: &'c BTreeSet<Vec<u8>>,
        candidate: &'c RefCell<Option<Vec<u8>>>,
    }
    impl Engine for Subject<'_> {
        fn identity(&self) -> continuum_corpus::differential::EngineIdentity {
            Mutant(Mutation::DropSuccessor).identity()
        }
        fn fields(&self) -> Fields {
            Fields::ALL
        }
        fn evaluate(
            &self,
            model: &continuum_engine_reference::model::Model,
            budget: Budget,
        ) -> Normalized {
            let id = model.identity().as_bytes().to_vec();
            if !self.corpus.contains(&id) && self.candidate.borrow().is_none() {
                *self.candidate.borrow_mut() = Some(id);
                panic!("planted subject transient");
            }
            Mutant(Mutation::DropSuccessor).evaluate(model, budget)
        }
    }
    struct Oracle<'c> {
        candidate: &'c RefCell<Option<Vec<u8>>>,
        sightings: Cell<usize>,
        fired: Cell<bool>,
    }
    impl Engine for Oracle<'_> {
        fn identity(&self) -> continuum_corpus::differential::EngineIdentity {
            ReferenceEngine.identity()
        }
        fn fields(&self) -> Fields {
            Fields::ALL
        }
        fn evaluate(
            &self,
            model: &continuum_engine_reference::model::Model,
            budget: Budget,
        ) -> Normalized {
            let is_c = self.candidate.borrow().as_deref() == Some(model.identity().as_bytes());
            if is_c {
                self.sightings.set(self.sightings.get() + 1);
                // Sighting 1 is the comparison where the subject faulted. From then on
                // the reference faults on C: on every retest and on the recheck.
                if self.sightings.get() >= 2 {
                    self.fired.set(true);
                    panic!("planted oracle fault on C after the subject's");
                }
            }
            ReferenceEngine.evaluate(model, budget)
        }
    }
    let subject = Subject {
        corpus: &corpus_ids,
        candidate: &candidate,
    };
    let oracle = Oracle {
        candidate: &candidate,
        sightings: Cell::new(0),
        fired: Cell::new(false),
    };
    let engines: Vec<(&str, &dyn Engine)> = vec![(EXPLICIT, &subject), (REFERENCE, &oracle)];
    let report = Harness::new(&engines, &LANES, BUDGET, epochs(), MINIMIZE)
        .expect("harness")
        .run(&corpus());
    assert!(
        candidate.borrow().is_some() && oracle.fired.get(),
        "both transients fired"
    );
    // The scenario is the one the round-5 finding describes: the explicit fault's
    // report is built on a recheck that shows the reference's fault, and it encodes
    // both.
    let both = report.findings.iter().find(|f| {
        f.report
            .disagreement
            .starts_with("engine-panicked side=subject")
            && f.report.faulted_slots().contains(REFERENCE)
    });
    assert!(
        both.is_some(),
        "the explicit report's recheck shows the reference fault"
    );
    assert!(report.faulted_slots.contains(EXPLICIT));
    assert!(report.faulted_slots.contains(REFERENCE));
    assert_ledger_evidenced(&report);
    // Every entry on a lane the explicit slot is not part of (a reference-only lane)
    // rests on a report that encodes a reference fault.
    for entry in report
        .ledger
        .entries()
        .filter(|q| q.subject != EXPLICIT && q.oracle != EXPLICIT)
    {
        let finding = report
            .findings
            .iter()
            .find(|f| f.handle == entry.defect)
            .expect("a finding");
        assert!(
            encoded_fault_slots(&finding.report.encode()).contains(REFERENCE),
            "{entry:?}"
        );
    }
    // All four reference lanes are halted.
    assert_eq!(report.ledger.claims(), reference_lane_claims());
}

/// An engine that reports a typed `EngineError` everywhere, in a given slot.
struct FaultyIn(&'static str);

impl Engine for FaultyIn {
    fn identity(&self) -> continuum_corpus::differential::EngineIdentity {
        continuum_corpus::differential::EngineIdentity {
            slot: self.0.to_owned(),
            build: "planted-fault/everywhere".to_owned(),
        }
    }
    fn fields(&self) -> Fields {
        Fields::ALL
    }
    fn evaluate(&self, model: &continuum_engine_reference::model::Model, _: Budget) -> Normalized {
        Normalized::inconclusive(
            &Inconclusive::new(InconclusiveReason::EngineError, "planted fault"),
            model.predicates().iter().map(|p| p.name().as_str()),
        )
    }
}

// ---------------------------------------------------------------------------
// the undefined-read category (bn-24a5c, RFC 0003 "Definedness")
// ---------------------------------------------------------------------------

/// An engine that ignores definedness disagrees with the reference on the undefined
/// category, and that is a defect like any other: reported, minimized, C006 halted.
#[test]
fn ignoring_definedness_is_a_disagreement() {
    let report = with_mutant(Mutation::IgnoreDefinedness);
    assert_handled(&report, |d| {
        matches!(
            d,
            Disagreement::UndefinedAction { .. }
                | Disagreement::Verdict {
                    subject: Decided::Holds,
                    oracle: Decided::Undefined(_),
                    ..
                }
        )
    });
    assert!(
        report
            .findings
            .iter()
            .all(|f| f.fixture.starts_with("def-"))
    );
}

/// The kind is compared: an action read reported as an invariant read (or the reverse)
/// is a disagreement.
#[test]
fn a_swapped_undefined_kind_is_a_disagreement() {
    let report = with_mutant(Mutation::SwapUndefinedKind);
    assert_handled(&report, |d| {
        matches!(
            d,
            Disagreement::Verdict {
                subject: Decided::Undefined(_),
                oracle: Decided::Undefined(_),
                ..
            }
        )
    });
}

/// The subject and state are checked against the model: an undefined read claimed at
/// a state where the chain holds is a defect even when the kinds agree.
#[test]
fn an_undefined_read_claimed_where_the_read_is_defined_is_caught() {
    let report = with_mutant(Mutation::ForgeUndefinedState);
    assert_handled(&report, |d| {
        matches!(
            d,
            Disagreement::InvalidUndefined {
                side: Side::Subject,
                ..
            }
        )
    });
}

/// Field-level rules: equal kinds agree whatever subject and state each side names
/// (each is checked against the model), different kinds or an undefined read against a
/// verdict disagree, and an undefined read against a budget stop is recorded undecided.
#[test]
fn undefined_reads_are_compared_by_kind_and_checked_against_the_model() {
    let fixture = corpus()
        .into_iter()
        .find(|f| {
            let model = f.build().expect("builds");
            ReferenceEngine
                .evaluate(&model, BUDGET)
                .invariants
                .values()
                .any(|v| matches!(v, InvariantVerdict::Undefined(r) if r.kind == UndefinedKind::Invariant))
        })
        .expect("a corpus fixture with an undefined invariant read");
    let model = fixture.build().expect("builds");
    let honest = ReferenceEngine.evaluate(&model, BUDGET);
    let (name, read) = honest
        .invariants
        .iter()
        .find_map(|(n, v)| match v {
            InvariantVerdict::Undefined(r) if r.kind == UndefinedKind::Invariant => {
                Some((n.clone(), r.clone()))
            }
            _ => None,
        })
        .expect("found");
    let reached = match &honest.projection {
        Projection::Exact { states, .. } => Some(states.clone()),
        _ => None,
    };
    assert_eq!(
        continuum_corpus::differential::check_undefined(
            &model,
            &read,
            Some(&name),
            reached.as_ref()
        ),
        Ok(())
    );
    // Same answer: agreed, and checked on both sides.
    let same = compare(&model, &honest, &honest);
    assert!(same.disagreements.is_empty(), "{:?}", same.disagreements);
    assert!(same.undefined_checked >= 2);
    // Against a verdict: a disagreement on that invariant.
    let mut holds = honest.clone();
    holds
        .invariants
        .insert(name.clone(), InvariantVerdict::Holds);
    assert!(
        compare(&model, &holds, &honest)
            .disagreements
            .contains(&Disagreement::Verdict {
                invariant: name.clone(),
                subject: Decided::Holds,
                oracle: Decided::Undefined(UndefinedKind::Invariant),
            })
    );
    // Against a budget stop: recorded undecided, not agreed, not dropped.
    let mut stopped = honest.clone();
    stopped.invariants.insert(
        name.clone(),
        InvariantVerdict::Inconclusive(Inconclusive::new(
            InconclusiveReason::ResourceExhausted,
            "stop",
        )),
    );
    let comparison = compare(&model, &stopped, &honest);
    assert!(comparison.disagreements.is_empty());
    assert!(
        comparison
            .undecided
            .iter()
            .any(|u| u.field == format!("verdict:{name}")
                && u.subject == Some(InconclusiveReason::ResourceExhausted)
                && u.oracle.is_none())
    );
}

/// Only the model-level undefined action read is dropped, every verdict kept: the
/// model-level field alone decides, and it is a disagreement.
#[test]
fn a_dropped_model_level_undefined_action_is_a_disagreement() {
    let report = with_mutant(Mutation::DropUndefinedAction);
    assert_handled(&report, |d| {
        matches!(
            d,
            Disagreement::UndefinedAction {
                subject: None,
                oracle: Some(_),
            }
        )
    });
}

/// Every fault of the undefined-read check is reachable, and a claim is tied to the
/// verdict it explains.
#[test]
fn undefined_claims_fail_closed_on_every_fault() {
    use continuum_corpus::differential::{UndefinedFault, UndefinedRead, check_undefined};
    let fixture = corpus()
        .into_iter()
        .find(|f| {
            f.predicates.iter().any(|(n, _)| n == "inv0#defined")
                && f.predicates.iter().any(|(n, _)| n == "inv1")
        })
        .or_else(|| {
            corpus()
                .into_iter()
                .find(|f| f.predicates.iter().any(|(n, _)| n == "inv0#defined"))
        })
        .expect("a fixture guarding inv0");
    let model = fixture.build().expect("builds");
    let state = model.initial_states()[0].as_slice().to_vec();
    let claim = |kind, subject: &str, state: Option<Vec<i64>>| UndefinedRead {
        kind,
        subject: subject.to_owned(),
        state,
        path: None,
    };
    assert_eq!(
        check_undefined(
            &model,
            &claim(UndefinedKind::Invariant, "inv0", None),
            Some("inv0"),
            None
        ),
        Err(UndefinedFault::NoState)
    );
    assert_eq!(
        check_undefined(
            &model,
            &claim(
                UndefinedKind::Invariant,
                "inv0",
                Some(vec![999; state.len()])
            ),
            Some("inv0"),
            None
        ),
        Err(UndefinedFault::NotAState)
    );
    assert_eq!(
        check_undefined(
            &model,
            &claim(UndefinedKind::Action, "no-such-action", Some(state.clone())),
            None,
            None
        ),
        Err(UndefinedFault::UnknownSubject)
    );
    // A claim naming inv0 cannot explain the verdict on another invariant.
    if model.predicate_index("inv1").is_some() {
        assert_eq!(
            check_undefined(
                &model,
                &claim(UndefinedKind::Invariant, "inv0", Some(state.clone())),
                Some("inv1"),
                None
            ),
            Err(UndefinedFault::WrongSubject)
        );
    }
    // A state outside the claiming side's reachable set is refused.
    let empty = BTreeSet::new();
    assert_eq!(
        check_undefined(
            &model,
            &claim(UndefinedKind::Invariant, "inv0", Some(state.clone())),
            Some("inv0"),
            Some(&empty)
        ),
        Err(UndefinedFault::Unreached)
    );
}

/// A budget stop does not turn a precedence difference into a defect: under a tight
/// budget, one side may reach an undefined read the other did not, over every
/// definedness seed the corpus generator offers up to 256.
#[test]
fn a_budget_stop_does_not_turn_definedness_precedence_into_a_defect() {
    let corpus: Vec<Fixture> = (0..256).map(generate_with_definedness).collect();
    let engines: [&dyn Engine; 2] = [&ReferenceEngine, &ClosureOracle];
    for budget in [
        TIGHT,
        Budget {
            states: 4,
            depth: 3,
            transitions: 16,
        },
        Budget {
            states: 6,
            depth: 4,
            transitions: 32,
        },
    ] {
        let report = Harness::new(&slots(&engines), &LANES, budget, epochs(), MINIMIZE)
            .expect("harness")
            .run(&corpus);
        assert!(
            report.findings.is_empty(),
            "{budget:?}: {}",
            report.summary()
        );
    }
}

// ---------------------------------------------------------------------------
// cr-2r0m24 (th-28049x, th-2r68ob): undefined-read claims fail closed
// ---------------------------------------------------------------------------

/// A model whose invariant guard is false at a state no action reaches: `x` starts at
/// 0 and never moves, and `inv0#defined` is `x != 2`, false only at the unreached 2.
fn unreached_guard_model() -> continuum_engine_reference::model::Model {
    use continuum_engine_reference::expr::{BoolExpr, CmpOp, IntExpr};
    use continuum_engine_reference::model::{ActionDecl, ModelBuilder};
    ModelBuilder::new()
        .variable("x", 0, 3)
        .initial_state(&[("x", 0)])
        .action(ActionDecl::deterministic(
            "stay",
            BoolExpr::constant(true),
            vec![("x", IntExpr::var("x"))],
        ))
        .predicate("inv0", BoolExpr::constant(true))
        .predicate(
            "inv0#defined",
            BoolExpr::compare(CmpOp::Ne, IntExpr::var("x"), IntExpr::constant(2)),
        )
        .build()
        .expect("valid")
}

/// th-28049x: two sides claim the same undefined read at a state neither proves
/// reached (no path, projection not exact). It is never agreement: the field is
/// `undefined-unproven`. A path that does not reach the state is a defect.
#[test]
fn an_undefined_read_at_an_unproven_state_is_never_agreement() {
    use continuum_corpus::differential::{UndefinedFault, UndefinedRead};
    let model = unreached_guard_model();
    let unreached = UndefinedRead {
        kind: UndefinedKind::Invariant,
        subject: "inv0".to_owned(),
        state: Some(vec![2]),
        path: None,
    };
    let stopped = Inconclusive::new(InconclusiveReason::ResourceExhausted, "partial");
    let answer = Normalized {
        projection: Projection::Inconclusive(stopped),
        invariants: [
            (
                "inv0".to_owned(),
                InvariantVerdict::Undefined(unreached.clone()),
            ),
            (
                "inv0#defined".to_owned(),
                InvariantVerdict::Undefined(unreached.clone()),
            ),
        ]
        .into_iter()
        .collect(),
        undefined_action: None,
    };
    let comparison = compare(&model, &answer, &answer);
    assert_eq!(comparison.undefined_checked, 0, "nothing was proven");
    assert_eq!(
        comparison
            .undecided
            .iter()
            .filter(|u| u.field.starts_with("undefined-unproven:verdict:"))
            .count(),
        2,
        "{:?}",
        comparison.undecided
    );
    assert!(
        comparison.disagreements.is_empty(),
        "{:?}",
        comparison.disagreements
    );
    // Neither verdict field is agreement: nothing is agreed (the projection and the
    // model-level field are undecided too).
    assert_eq!(comparison.agreed, 0, "{comparison:?}");

    // A path that replays but ends elsewhere, or does not replay, is a defect.
    let mut forged = answer.clone();
    let ends_elsewhere = UndefinedRead {
        path: Some(continuum_corpus::differential::Trace {
            start: vec![0],
            steps: vec![("stay".to_owned(), vec![0])],
        }),
        ..unreached.clone()
    };
    forged.invariants.insert(
        "inv0".to_owned(),
        InvariantVerdict::Undefined(ends_elsewhere),
    );
    assert!(compare(&model, &forged, &answer).disagreements.contains(
        &Disagreement::InvalidUndefined {
            side: Side::Subject,
            field: "verdict:inv0".to_owned(),
            fault: UndefinedFault::PathEndsElsewhere,
        }
    ));
    let not_a_path = UndefinedRead {
        path: Some(continuum_corpus::differential::Trace {
            start: vec![0],
            steps: vec![("stay".to_owned(), vec![2])],
        }),
        ..unreached
    };
    forged
        .invariants
        .insert("inv0".to_owned(), InvariantVerdict::Undefined(not_a_path));
    assert!(compare(&model, &forged, &answer).disagreements.contains(
        &Disagreement::InvalidUndefined {
            side: Side::Subject,
            field: "verdict:inv0".to_owned(),
            fault: UndefinedFault::PathDoesNotReplay,
        }
    ));
}

/// th-2r68ob: the model-level field is the undefined *action* read. An invariant read
/// there is a defect, and two of them never agree.
#[test]
fn the_model_level_field_accepts_only_action_reads() {
    use continuum_corpus::differential::{UndefinedFault, UndefinedRead};
    let fixture = corpus()
        .into_iter()
        .find(|f| {
            let model = f.build().expect("builds");
            ReferenceEngine
                .evaluate(&model, BUDGET)
                .invariants
                .values()
                .any(|v| matches!(v, InvariantVerdict::Undefined(r) if r.kind == UndefinedKind::Invariant))
        })
        .expect("a fixture with an undefined invariant read");
    let model = fixture.build().expect("builds");
    let honest = ReferenceEngine.evaluate(&model, BUDGET);
    let invariant_read: UndefinedRead = honest
        .invariants
        .values()
        .find_map(|v| match v {
            InvariantVerdict::Undefined(r) if r.kind == UndefinedKind::Invariant => Some(r.clone()),
            _ => None,
        })
        .expect("found");
    let mut smuggled = honest.clone();
    smuggled.undefined_action = Some(invariant_read);
    let comparison = compare(&model, &smuggled, &smuggled);
    for side in [Side::Subject, Side::Oracle] {
        assert!(
            comparison
                .disagreements
                .contains(&Disagreement::InvalidUndefined {
                    side,
                    field: "undefined-action".to_owned(),
                    fault: UndefinedFault::NotAnActionRead,
                })
        );
    }
    // The action field is not agreed, and the deadlock field is still compared (an
    // invariant read there does not suspend it): exactly one field fewer agrees.
    assert_eq!(
        compare(&model, &honest, &honest).agreed,
        comparison.agreed + 1
    );
    assert!(
        !comparison
            .undecided
            .iter()
            .any(|u| u.field == "deadlocks-under-undefined-action")
    );
}

/// A path that replays to its state where the read is in fact defined is a defect,
/// not a proof.
#[test]
fn a_replayed_path_to_a_defined_read_is_a_defect() {
    use continuum_corpus::differential::{UndefinedFault, UndefinedRead, check_undefined};
    let model = unreached_guard_model();
    let claim = UndefinedRead {
        kind: UndefinedKind::Invariant,
        subject: "inv0".to_owned(),
        state: Some(vec![0]),
        path: Some(continuum_corpus::differential::Trace {
            start: vec![0],
            steps: vec![("stay".to_owned(), vec![0])],
        }),
    };
    assert_eq!(
        check_undefined(&model, &claim, Some("inv0"), None),
        Err(UndefinedFault::Defined)
    );
}
