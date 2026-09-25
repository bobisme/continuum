//! PR-16 exit evidence, mutant M10 (bn-2cgl): "independence rule says two writes to
//! same epoch commute" (`notes/plan/examples/replicated_register.md`, "Required
//! mutants").
//!
//! M10 is not a mutant of the register program. It is a mutant of the independence
//! relation a partial-order reduction trusts, so its campaign runs here, beside the
//! reducer that has one (bn-voq4, C005), and not in `continuum-asupersync`, whose
//! campaigns use no reduction. The PR-16 exit summary
//! (`crates/continuum-asupersync/tests/evidence/pr16-exit.json`,
//! `crates/continuum-asupersync/tests/pr16_exit_evidence.rs`) reads the retained artifact
//! this file pins, `tests/evidence/pr16-exit-m10.json`, and fails closed unless it is
//! present, met, and pinned to the same expectation.
//!
//! # The mutant, the property, the campaign
//!
//! - **Mutant**: the declared rule "two writes to the same epoch commute": every pair of
//!   distinct actions whose ground names carry the same `epoch=` argument is declared
//!   independent. Every action of both register models writes, so the rule covers every
//!   same-epoch pair.
//! - **Expected violated property**: RFC 0004 "Dependence evidence": a pair declared
//!   independent must commute wherever both are enabled: firing them in either order
//!   reaches the same states, and neither disables the other. This is the premise the
//!   reducer's witness checker checks per stored state as `WitnessDefect::DoesNotCommute`
//!   (`src/checker.rs`). A reduction that trusts a rule that fails it can prune every
//!   interleaving that reaches a state, which is how C005's verdict preservation is lost.
//! - **Campaign**: the reference engine's complete exploration of each subject model, and
//!   at every reachable state every declared pair that is enabled there is fired in both
//!   orders through `Model::action_successors`. The campaign is exhaustive, so it has no
//!   log seed. It is run in a seeded order of states for each of [`SEEDS`], and its
//!   canonical result must be the same bytes for every seed and for a repeated run. The
//!   result is independent of the visit order by construction (counts, and an explicit
//!   tie-break for the witness), so the seeds and the repeat catch process
//!   nondeterminism only; the retained artifact, compared byte for byte, catches it
//!   across processes.
//! - **The correct implementation**: the reducer's own relation, `continuum_engine_dpor::
//!   dependence` (derived from footprints, never declared), must mark every refuted pair
//!   `DefinitelyDependent`. On these subjects it marks every pair dependent, so this leg
//!   shows only that the reducer is maximally conservative here, not that it
//!   discriminates. Every pair it marks `DefinitelyIndependent` must commute at
//!   every reachable state where both are enabled. `continuum_engine_dpor::check` on the
//!   subject must be complete and establish every invariant, as the unreduced
//!   evaluation of the invariants over the reference engine's reachable set does, and
//!   `check_witness` must accept its witness.
//!
//! # Fail closed
//!
//! The verdict ([`judge`]) is `met` only when every subject's exploration is complete,
//! the refutation carries a replayed witness of the pinned symptom, the real relation
//! answers every refuted pair with `DefinitelyDependent` (an `Unknown` from a short work
//! budget is a gap, not a pass), the real relation's independent pairs all commute, and
//! the reduced check is complete, established and its witness accepted. An exploration
//! stopped by a bound is a typed gap, never a kill. [`judge`] is itself mutation-checked:
//! each mis-pinned expectation and each doctored result is `not met`
//! ([`m10_a_mis_pinned_expectation_or_an_inconclusive_campaign_is_not_met`]).
//!
//! # Scope
//!
//! The subjects are the dossier's two register models: the abstract register at its
//! committed configuration (two epochs, two values), the durable register at claim A's
//! scope (three nodes, two values, two epochs), where the rule declares only the
//! same-epoch pairs, and the durable register at one epoch, where every pair is a
//! same-epoch pair. The reducer declares no pair of any subject independent (every
//! action writes the lowered set variables), so the leg "its independent pairs commute"
//! is vacuous on these subjects; C005's differential is that relation's non-vacuous
//! check. The campaign refutes the declared rule; it does not run a reduction under the mutated rule, since the reducer's
//! relation is derived and has no declared-rule input (a reduction over a declared rule
//! is the seeded `c005-mut-01` fault in `src/mutation.rs`, caught by C005's
//! differential).
//!
//! Regenerate the artifact with `PR16_EXIT_BLESS=1 cargo test -p continuum-engine-dpor
//! --test pr16_exit_m10_independence`; the run then fails, so new bytes are reviewed.

#![allow(
    missing_docs,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects
)]

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::sync::OnceLock;

use continuum_cml_elab::config::RunConfig;
use continuum_cml_elab::lower::{InitPath, lower_configured_with_init};
use continuum_cml_elab::{Limits, elaborate_source};
use continuum_engine_dpor::{self as dpor, DeadlockPolicy, DependenceEvidence, Obligations};
use continuum_engine_reference::bfs::{self, Discovery, Exploration};
use continuum_model_core::{Model, State};

/// The artifact ID of this campaign, as the exit summary names it.
const ARTIFACT: &str = "pr16-exit-m10-same-epoch-commute";

/// The seeds the campaign's state order is drawn from: the scenario seed and two more.
const SEEDS: [u64; 3] = [104_729, 1, 2];

/// Work units for one `dependence` question: enough to derive every footprint of the
/// subjects, so an `Unknown` answer is a gap in the evidence, not a budget choice.
const DEPENDENCE_WORK: u64 = 1 << 32;

/// The declared bounds of the reduced check.
const DPOR_BOUNDS: dpor::Bounds = dpor::Bounds::new(1 << 20, 1 << 24, 1 << 16, 1 << 36);

fn dossier(rel: &str) -> String {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../notes/plan/").to_owned() + rel;
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"))
}

// ---------------------------------------------------------------------------
// the expectation, pinned
// ---------------------------------------------------------------------------

/// How a declared-independent pair fails to commute at a state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Symptom {
    /// Firing one disables the other.
    Disables,
    /// Both orders fire, and they reach different states.
    DifferentStates,
}

impl Symptom {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Disables => "disables",
            Self::DifferentStates => "different-states",
        }
    }
}

/// M10's expected result, per subject.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Expect {
    /// The declared rule is refuted: some same-epoch pair does not commute, with this
    /// symptom on the shallowest witness.
    Refuted { symptom: Symptom },
    /// The declared rule holds (a mis-pin, for the teeth).
    Holds,
}

/// The property M10 violates, verbatim in the artifact and in the exit summary's pin.
const PROPERTY: &str = "RFC 0004 dependence evidence: a pair declared independent commutes wherever both are enabled (the DPOR witness checker's DoesNotCommute premise; C005 verdict preservation rests on it)";

/// The subjects and their pinned expectations.
fn subjects() -> Vec<(&'static str, &'static str, Expect)> {
    vec![
        (
            "abstract-register",
            "notes/plan/examples/abstract_register.ctm with notes/plan/examples/abstract_register.run-config.json",
            // Choose(e, v0) and Choose(e, v1): after one, the other's guard
            // `chosen.get(epoch) in {None, Some(value)}` is false.
            Expect::Refuted {
                symptom: Symptom::Disables,
            },
        ),
        (
            "durable-register-one-epoch",
            "notes/plan/examples/durable_register.ctm with notes/plan/examples/durable_register.run-config.json, Nat bounded to 0",
            // At one epoch every pair is a same-epoch pair, so the rule covers all of
            // them; the shallowest witness is two writes to one slot's permit.
            Expect::Refuted {
                symptom: Symptom::Disables,
            },
        ),
        (
            "durable-register-claim-a",
            "notes/plan/examples/durable_register.ctm with notes/plan/examples/durable_register.run-config.json",
            // Two epochs: the rule declares only same-epoch pairs, and cross-epoch pairs
            // keep the reducer's relation.
            Expect::Refuted {
                symptom: Symptom::Disables,
            },
        ),
    ]
}

fn lower_subject(id: &str) -> Model {
    let (ctm, config, one_epoch) = match id {
        "abstract-register" => (
            "examples/abstract_register.ctm",
            "examples/abstract_register.run-config.json",
            false,
        ),
        "durable-register-one-epoch" => (
            "examples/durable_register.ctm",
            "examples/durable_register.run-config.json",
            true,
        ),
        "durable-register-claim-a" => (
            "examples/durable_register.ctm",
            "examples/durable_register.run-config.json",
            false,
        ),
        other => panic!("no subject {other}"),
    };
    let norm = elaborate_source(&dossier(ctm)).expect("the dossier model elaborates");
    let mut text = dossier(config);
    if one_epoch {
        assert_eq!(text.matches("\"max\": 1").count(), 1);
        text = text.replacen("\"max\": 1", "\"max\": 0", 1);
    }
    let config = RunConfig::parse(text.as_bytes()).expect("the configuration reads");
    lower_configured_with_init(&norm, &config, Limits::default(), InitPath::Pinned)
        .0
        .expect("the subject lowers")
        .model()
        .clone()
}

// ---------------------------------------------------------------------------
// the campaign
// ---------------------------------------------------------------------------

/// The `epoch=` argument of a ground action name, if it has one.
fn epoch_of(name: &str) -> Option<u64> {
    let at = name.find("epoch=")? + "epoch=".len();
    let digits: String = name[at..]
        .chars()
        .take_while(char::is_ascii_digit)
        .collect();
    digits.parse().ok()
}

/// The mutant rule: two distinct actions writing the same epoch are independent.
fn declared_pairs(model: &Model) -> Vec<(usize, usize)> {
    let actions = model.actions();
    let mut out = Vec::new();
    for i in 0..actions.len() {
        for j in i + 1..actions.len() {
            let (a, b) = (
                epoch_of(actions[i].name().as_str()),
                epoch_of(actions[j].name().as_str()),
            );
            if a.is_some() && a == b {
                out.push((i, j));
            }
        }
    }
    out
}

fn succ(model: &Model, action: usize, state: &State) -> BTreeSet<State> {
    model
        .action_successors(action, state)
        .expect("the subject evaluates")
        .into_iter()
        .collect()
}

/// Fire `i` then `j`, and `j` then `i`, from `s`: `None` when they commute there.
fn diamond(model: &Model, i: usize, j: usize, s: &State) -> Option<Symptom> {
    let ti = succ(model, i, s);
    let tj = succ(model, j, s);
    let mut ij = BTreeSet::new();
    for t in &ti {
        let next = succ(model, j, t);
        if next.is_empty() {
            return Some(Symptom::Disables);
        }
        ij.extend(next);
    }
    let mut ji = BTreeSet::new();
    for t in &tj {
        let next = succ(model, i, t);
        if next.is_empty() {
            return Some(Symptom::Disables);
        }
        ji.extend(next);
    }
    (ij != ji).then_some(Symptom::DifferentStates)
}

/// [`diamond`] from the two first steps, already computed.
fn diamond_from(
    model: &Model,
    i: usize,
    j: usize,
    ti: &BTreeSet<State>,
    tj: &BTreeSet<State>,
) -> Option<Symptom> {
    let mut ij = BTreeSet::new();
    for t in ti {
        let next = succ(model, j, t);
        if next.is_empty() {
            return Some(Symptom::Disables);
        }
        ij.extend(next);
    }
    let mut ji = BTreeSet::new();
    for t in tj {
        let next = succ(model, i, t);
        if next.is_empty() {
            return Some(Symptom::Disables);
        }
        ji.extend(next);
    }
    (ij != ji).then_some(Symptom::DifferentStates)
}

/// A shortest labelled path to `s`, from the exploration's discovery chain.
fn path_to(model: &Model, reach: &bfs::Reachable, s: &State) -> (State, Vec<String>) {
    let mut labels = Vec::new();
    let mut at = s.clone();
    loop {
        match reach.origin_of(&at).expect("a discovered state") {
            Discovery::Initial => break,
            Discovery::Step {
                predecessor,
                action,
            } => {
                labels.push(model.actions()[*action].name().as_str().to_owned());
                at = predecessor.clone();
            }
        }
    }
    labels.reverse();
    (at, labels)
}

fn replay(model: &Model, root: &State, labels: &[String]) -> State {
    assert!(
        model.initial_states().contains(root),
        "the path starts at an initial state"
    );
    let mut s = root.clone();
    for l in labels {
        let a = model.action_index(l).expect("a label of the model");
        s = succ(model, a, &s)
            .into_iter()
            .next()
            .expect("enabled on replay");
    }
    s
}

/// The shallowest refuting diamond: path to the state, the two labels, the symptom.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Witness {
    path: Vec<String>,
    first: String,
    second: String,
    symptom: Symptom,
    /// Whether replaying the path from the initial state and firing the pair again
    /// reproduces the symptom.
    replayed: bool,
}

/// Everything the campaign found on one subject, in canonical form.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Found {
    subject: &'static str,
    /// `None` when the exploration stopped at a bound (a gap).
    states: Option<usize>,
    actions: usize,
    declared: usize,
    diamonds: usize,
    refuted_diamonds: BTreeMap<Symptom, usize>,
    refuted_pairs: usize,
    commuting_pairs: usize,
    never_co_enabled: usize,
    witness: Option<Witness>,
    /// The real relation on the refuted pairs, by answer.
    real_on_refuted: BTreeMap<&'static str, usize>,
    /// Pairs the real relation marks `DefinitelyIndependent`, their co-enabled
    /// diamonds, and how many of those fail to commute.
    real_independent: (usize, usize, usize),
    /// The reduced check: complete, established, witness accepted; and the unreduced
    /// evaluation of the invariants over the reachable set.
    dpor: (bool, String, bool),
    oracle_holds: bool,
}

fn evidence_kind(e: &DependenceEvidence) -> &'static str {
    match e {
        DependenceEvidence::DefinitelyIndependent { .. } => "definitely-independent",
        DependenceEvidence::DefinitelyDependent(_) => "definitely-dependent",
        DependenceEvidence::Unknown => "unknown",
    }
}

/// A deterministic permutation of `0..n` from `seed`.
fn order(n: usize, seed: u64) -> Vec<usize> {
    let mut state = seed;
    let mut next = || {
        state = state.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = state;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    };
    let mut v: Vec<usize> = (0..n).collect();
    for i in (1..n).rev() {
        let j = usize::try_from(next() % (i as u64 + 1)).expect("fits");
        v.swap(i, j);
    }
    v
}

/// One seeded sweep of the declared pairs over the reachable states.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Sweep {
    diamonds: usize,
    refuted_diamonds: BTreeMap<Symptom, usize>,
    refuted: BTreeSet<(usize, usize)>,
    co_enabled: BTreeSet<(usize, usize)>,
    /// Shallowest, then least state, then least pair: independent of the visit order.
    best: Option<(usize, State, (usize, usize), Symptom)>,
}

/// Fire every declared pair enabled at each reachable state, visiting the states in
/// `seed`'s order. Each label's successors at a state are computed once.
fn sweep(model: &Model, reach: &bfs::Reachable, declared: &[(usize, usize)], seed: u64) -> Sweep {
    let states = reach.states();
    let labels: BTreeSet<usize> = declared.iter().flat_map(|&(i, j)| [i, j]).collect();
    let mut out = Sweep {
        diamonds: 0,
        refuted_diamonds: BTreeMap::new(),
        refuted: BTreeSet::new(),
        co_enabled: BTreeSet::new(),
        best: None,
    };
    for k in order(states.len(), seed) {
        let s = &states[k];
        let first: BTreeMap<usize, BTreeSet<State>> = labels
            .iter()
            .filter(|l| model.is_enabled(**l, s).expect("evaluates"))
            .map(|l| (*l, succ(model, *l, s)))
            .collect();
        for &(i, j) in declared {
            let (Some(ti), Some(tj)) = (first.get(&i), first.get(&j)) else {
                continue;
            };
            out.co_enabled.insert((i, j));
            out.diamonds += 1;
            if let Some(sym) = diamond_from(model, i, j, ti, tj) {
                *out.refuted_diamonds.entry(sym).or_default() += 1;
                out.refuted.insert((i, j));
                let depth = reach.depth_of(s).expect("discovered");
                let key = (depth, s.clone(), (i, j), sym);
                if out
                    .best
                    .as_ref()
                    .is_none_or(|b| (b.0, &b.1, b.2) > (key.0, &key.1, key.2))
                {
                    out.best = Some(key);
                }
            }
        }
    }
    out
}

/// The legs that do not depend on the visit order: the reducer's relation, its reduced
/// check and witness, and the unreduced invariants.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Legs {
    real_on_refuted: BTreeMap<&'static str, usize>,
    real_independent: (usize, usize, usize),
    dpor: (bool, String, bool),
    oracle_holds: bool,
}

fn legs(model: &Model, states: &[State], refuted: &BTreeSet<(usize, usize)>) -> Legs {
    let mut real_on_refuted: BTreeMap<&'static str, usize> = BTreeMap::new();
    for &(i, j) in refuted {
        *real_on_refuted
            .entry(evidence_kind(&dpor::dependence(
                model,
                i,
                j,
                DEPENDENCE_WORK,
            )))
            .or_default() += 1;
    }
    // The real relation's independent pairs, over every pair of actions.
    let n = model.actions().len();
    let (mut ind_pairs, mut ind_diamonds, mut ind_bad) = (0, 0, 0);
    for i in 0..n {
        for j in i + 1..n {
            if dpor::dependence(model, i, j, DEPENDENCE_WORK).is_dependent() {
                continue;
            }
            ind_pairs += 1;
            for s in states {
                if model.is_enabled(i, s).expect("evaluates")
                    && model.is_enabled(j, s).expect("evaluates")
                {
                    ind_diamonds += 1;
                    ind_bad += usize::from(diamond(model, i, j, s).is_some());
                }
            }
        }
    }
    let obligations = Obligations::every_predicate(model, DeadlockPolicy::Allowed);
    let report = dpor::check(model, &obligations, DPOR_BOUNDS).expect("obligations declared");
    let accepted =
        dpor::check_witness(model, &obligations, report.witness(), DPOR_BOUNDS.work()).is_ok();
    let oracle_holds = (0..model.predicates().len()).all(|p| {
        states
            .iter()
            .all(|s| model.evaluate_predicate(p, s).expect("evaluates"))
    });
    Legs {
        real_on_refuted,
        real_independent: (ind_pairs, ind_diamonds, ind_bad),
        dpor: (
            report.witness().is_complete(),
            report.verdict().as_str().to_owned(),
            accepted,
        ),
        oracle_holds,
    }
}

fn assemble(
    subject: &'static str,
    model: &Model,
    reach: &bfs::Reachable,
    complete: bool,
    declared: usize,
    sw: &Sweep,
    legs: &Legs,
) -> Found {
    let witness = sw.best.as_ref().map(|(_, s, (i, j), symptom)| {
        let (root, path) = path_to(model, reach, s);
        let again = replay(model, &root, &path);
        Witness {
            replayed: &again == s && diamond(model, *i, *j, &again) == Some(*symptom),
            path,
            first: model.actions()[*i].name().as_str().to_owned(),
            second: model.actions()[*j].name().as_str().to_owned(),
            symptom: *symptom,
        }
    });
    Found {
        subject,
        states: complete.then_some(reach.states().len()),
        actions: model.actions().len(),
        declared,
        diamonds: sw.diamonds,
        refuted_diamonds: sw.refuted_diamonds.clone(),
        refuted_pairs: sw.refuted.len(),
        commuting_pairs: sw.co_enabled.len() - sw.refuted.len(),
        never_co_enabled: declared - sw.co_enabled.len(),
        witness,
        real_on_refuted: legs.real_on_refuted.clone(),
        real_independent: legs.real_independent,
        dpor: legs.dpor.clone(),
        oracle_holds: legs.oracle_holds,
    }
}

/// Run the whole campaign on `model` under `bounds`, visiting states in `seed`'s order.
fn campaign(subject: &'static str, model: &Model, bounds: bfs::Bounds, seed: u64) -> Found {
    let exploration = bfs::explore(model, bounds).expect("the subject evaluates");
    let complete = matches!(exploration, Exploration::Complete(_));
    let reach = exploration.reachable();
    let declared = declared_pairs(model);
    let sw = sweep(model, reach, &declared, seed);
    let l = legs(model, reach.states(), &sw.refuted);
    assemble(subject, model, reach, complete, declared.len(), &sw, &l)
}

// ---------------------------------------------------------------------------
// the verdict
// ---------------------------------------------------------------------------

/// Why a subject's result does not meet its expectation. Empty is met.
fn judge(expect: Expect, f: &Found) -> Vec<String> {
    let mut why = Vec::new();
    if f.states.is_none() {
        why.push(format!(
            "{}: the exploration stopped at a bound: inconclusive, never a kill",
            f.subject
        ));
        return why;
    }
    match expect {
        Expect::Holds => {
            if f.refuted_pairs > 0 {
                why.push(format!(
                    "{}: expected the rule to hold, it is refuted",
                    f.subject
                ));
            }
        }
        Expect::Refuted { symptom } => {
            match &f.witness {
                None => why.push(format!("{}: no refuting diamond", f.subject)),
                Some(w) => {
                    if w.symptom != symptom {
                        why.push(format!(
                            "{}: witness symptom {} is not the pinned {}",
                            f.subject,
                            w.symptom.as_str(),
                            symptom.as_str()
                        ));
                    }
                    if !w.replayed {
                        why.push(format!("{}: the witness does not replay", f.subject));
                    }
                }
            }
            if f.refuted_pairs == 0 || f.refuted_diamonds.values().sum::<usize>() == 0 {
                why.push(format!("{}: no refuted pair", f.subject));
            }
        }
    }
    // The correct implementation: the reducer's derived relation never claims a refuted
    // pair independent, answers each conclusively, and is sound where it does claim.
    let real_dependent = f
        .real_on_refuted
        .get("definitely-dependent")
        .copied()
        .unwrap_or(0);
    if real_dependent != f.refuted_pairs {
        why.push(format!(
            "{}: the reducer's relation on the refuted pairs is {:?}, not all definitely-dependent",
            f.subject, f.real_on_refuted
        ));
    }
    if f.real_independent.2 != 0 {
        why.push(format!(
            "{}: {} diamonds of the reducer's independent pairs do not commute",
            f.subject, f.real_independent.2
        ));
    }
    if f.dpor != (true, "established".to_owned(), true) || !f.oracle_holds {
        why.push(format!(
            "{}: the reduced check (complete, verdict, witness accepted) is {:?} and the unreduced invariants hold: {}",
            f.subject, f.dpor, f.oracle_holds
        ));
    }
    why
}

fn render(f: &Found) -> String {
    let mut s = String::new();
    let _ = write!(
        s,
        "{}: states {} actions {} declared-independent pairs {} co-enabled diamonds {} refuted diamonds {:?} refuted pairs {} commuting pairs {} never co-enabled {}; reducer on refuted pairs {:?}; reducer independent pairs {} diamonds {} not commuting {}; reduced check complete {} verdict {} witness accepted {}; unreduced invariants hold {}",
        f.subject,
        f.states
            .map_or("bound-stopped".to_owned(), |n| n.to_string()),
        f.actions,
        f.declared,
        f.diamonds,
        f.refuted_diamonds
            .iter()
            .map(|(k, v)| (k.as_str(), *v))
            .collect::<Vec<_>>(),
        f.refuted_pairs,
        f.commuting_pairs,
        f.never_co_enabled,
        f.real_on_refuted,
        f.real_independent.0,
        f.real_independent.1,
        f.real_independent.2,
        f.dpor.0,
        f.dpor.1,
        f.dpor.2,
        f.oracle_holds,
    );
    if let Some(w) = &f.witness {
        let _ = write!(
            s,
            "; witness: after [{}], {} and {} {} (replayed {})",
            w.path.join(", "),
            w.first,
            w.second,
            w.symptom.as_str(),
            w.replayed
        );
    }
    s
}

struct Run {
    found: Vec<(Expect, &'static str, Found)>,
    /// The canonical rendering per seed, and a repeat at the first seed.
    per_seed: Vec<(u64, String)>,
    repeat: String,
}

fn run() -> &'static Run {
    static CELL: OnceLock<Run> = OnceLock::new();
    CELL.get_or_init(|| {
        struct Subject {
            id: &'static str,
            src: &'static str,
            expect: Expect,
            model: Model,
            exploration: Exploration,
            declared: Vec<(usize, usize)>,
        }
        let subjects: Vec<Subject> = subjects()
            .into_iter()
            .map(|(id, src, expect)| {
                let model = lower_subject(id);
                let exploration =
                    bfs::explore(&model, bfs::Bounds::CERTIFIABLE).expect("the subject evaluates");
                let declared = declared_pairs(&model);
                Subject {
                    id,
                    src,
                    expect,
                    model,
                    exploration,
                    declared,
                }
            })
            .collect();
        // Every (subject, seed) sweep, and a repeat of the first seed, in parallel.
        let mut jobs: Vec<(usize, u64)> = Vec::new();
        for k in 0..subjects.len() {
            for seed in SEEDS {
                jobs.push((k, seed));
            }
            jobs.push((k, SEEDS[0]));
        }
        let sweeps: Vec<Sweep> = std::thread::scope(|scope| {
            let handles: Vec<_> = jobs
                .iter()
                .map(|&(k, seed)| {
                    let sub = &subjects[k];
                    scope.spawn(move || {
                        sweep(&sub.model, sub.exploration.reachable(), &sub.declared, seed)
                    })
                })
                .collect();
            handles
                .into_iter()
                .map(|h| h.join().expect("a sweep"))
                .collect()
        });
        let per_subject = SEEDS.len() + 1;
        let legs: Vec<Legs> = std::thread::scope(|scope| {
            let handles: Vec<_> = subjects
                .iter()
                .enumerate()
                .map(|(k, sub)| {
                    let refuted = &sweeps[k * per_subject].refuted;
                    scope.spawn(move || {
                        legs(&sub.model, sub.exploration.reachable().states(), refuted)
                    })
                })
                .collect();
            handles
                .into_iter()
                .map(|h| h.join().expect("the legs"))
                .collect()
        });
        let found_at = |k: usize, n: usize| -> Found {
            let sub = &subjects[k];
            assemble(
                sub.id,
                &sub.model,
                sub.exploration.reachable(),
                sub.exploration.is_complete(),
                sub.declared.len(),
                &sweeps[k * per_subject + n],
                &legs[k],
            )
        };
        let canon = |n: usize| -> String {
            (0..subjects.len())
                .map(|k| render(&found_at(k, n)))
                .collect::<Vec<_>>()
                .join("\n")
        };
        let per_seed: Vec<(u64, String)> = SEEDS
            .iter()
            .enumerate()
            .map(|(n, seed)| (*seed, canon(n)))
            .collect();
        let repeat = canon(SEEDS.len());
        Run {
            found: (0..subjects.len())
                .map(|k| (subjects[k].expect, subjects[k].src, found_at(k, 0)))
                .collect(),
            per_seed,
            repeat,
        }
    })
}

/// The whole verdict: every subject met, and every seed's and the repeat's bytes equal.
fn verdict(run: &Run) -> Vec<String> {
    let mut why: Vec<String> = run
        .found
        .iter()
        .flat_map(|(e, _, f)| judge(*e, f))
        .collect();
    let first = &run.per_seed[0].1;
    for (seed, text) in &run.per_seed {
        if text != first {
            why.push(format!(
                "seed {seed}: the canonical result differs from seed {}",
                run.per_seed[0].0
            ));
        }
    }
    if &run.repeat != first {
        why.push("a repeated run differs".to_owned());
    }
    why
}

// ---------------------------------------------------------------------------
// the artifact
// ---------------------------------------------------------------------------

fn json_str(s: &str) -> String {
    let mut out = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            c if u32::from(c) < 0x20 => {
                let _ = write!(out, "\\u{:04x}", u32::from(c));
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// The pinned expectation, as the exit summary pins it too.
fn expected_line() -> String {
    let parts: Vec<String> = subjects()
        .iter()
        .map(|(id, _, e)| match e {
            Expect::Refuted { symptom } => format!("{id}: refuted ({})", symptom.as_str()),
            Expect::Holds => format!("{id}: holds"),
        })
        .collect();
    format!("refuted; property {PROPERTY}; {}", parts.join("; "))
}

fn artifact(run: &Run) -> String {
    let why = verdict(run);
    let mut s = String::from("{");
    let _ = write!(s, "\"artifact\":{}", json_str(ARTIFACT));
    let _ = write!(
        s,
        ",\"bone\":\"bn-2cgl\",\"requirement\":\"PR-16-EXIT\",\"mutant\":\"M10\""
    );
    let _ = write!(
        s,
        ",\"defect\":{}",
        json_str("independence rule says two writes to same epoch commute")
    );
    let _ = write!(
        s,
        ",\"mutation\":{}",
        json_str(
            "a declared independence rule: every pair of distinct actions whose ground names carry the same epoch= argument is independent"
        )
    );
    let _ = write!(s, ",\"expected\":{}", json_str(&expected_line()));
    let _ = write!(
        s,
        ",\"suite\":\"crates/continuum-engine-dpor/tests/pr16_exit_m10_independence.rs\""
    );
    let _ = write!(
        s,
        ",\"seeds\":[{}]",
        SEEDS
            .iter()
            .map(u64::to_string)
            .collect::<Vec<_>>()
            .join(",")
    );
    let _ = write!(
        s,
        ",\"seeds_byte_identical\":{}",
        run.per_seed.iter().all(|(_, t)| *t == run.per_seed[0].1)
            && run.repeat == run.per_seed[0].1
    );
    let _ = write!(
        s,
        ",\"verdict\":{}",
        json_str(if why.is_empty() { "met" } else { "not-met" })
    );
    let _ = write!(
        s,
        ",\"not_met_because\":[{}]",
        why.iter()
            .map(|w| json_str(w))
            .collect::<Vec<_>>()
            .join(",")
    );
    let _ = write!(
        s,
        ",\"narrowing\":{}",
        json_str(
            "the subjects are the abstract register at its committed configuration and the durable register at claim A's scope and at one epoch; no reduction is run under the mutated rule, since the reducer's relation is derived and takes no declared rule; the reducer marks every pair of every subject dependent (every action writes the lowered set variables), so its legs here show it is maximally conservative, not that it discriminates, and C005's differential is the non-vacuous check of that relation; the campaign is exhaustive and its result independent of the visit order by construction, so the seeds and the repeat catch process nondeterminism only"
        )
    );
    // The subjects last: every top-level string key precedes them, which is how the
    // exit summary reads this artifact.
    s.push_str(",\"subjects\":[");
    for (k, (e, src, f)) in run.found.iter().enumerate() {
        if k > 0 {
            s.push(',');
        }
        let _ = write!(
            s,
            "{{\"id\":{},\"source\":{},\"result\":{},\"met\":{}}}",
            json_str(f.subject),
            json_str(src),
            json_str(&render(f)),
            judge(*e, f).is_empty()
        );
    }
    s.push(']');
    s.push_str("}\n");
    s
}

// ---------------------------------------------------------------------------
// tests
// ---------------------------------------------------------------------------

/// M10 on every subject: the declared rule is refuted with a replayed witness of the
/// pinned symptom, and the reducer's own relation is correct on the same pairs.
#[test]
fn m10_same_epoch_commute_is_refuted_and_the_reducer_never_claims_it() {
    let r = run();
    for (e, _, f) in &r.found {
        assert_eq!(judge(*e, f), Vec::<String>::new(), "{}", render(f));
        assert!(f.refuted_pairs > 0 && f.commuting_pairs + f.refuted_pairs <= f.declared);
    }
}

/// The campaign's canonical result is the same bytes for every seed's visit order and
/// for a repeated run.
#[test]
fn m10_the_campaign_is_deterministic_across_seeds() {
    let r = run();
    assert_eq!(r.per_seed.len(), SEEDS.len());
    for (seed, text) in &r.per_seed {
        assert_eq!(text, &r.per_seed[0].1, "seed {seed}");
    }
    assert_eq!(r.repeat, r.per_seed[0].1);
    // The seeds do change the visit order.
    assert_ne!(order(64, SEEDS[0]), order(64, SEEDS[1]));
}

/// The teeth: each mis-pinned expectation and each doctored result is not met.
#[test]
fn m10_a_mis_pinned_expectation_or_an_inconclusive_campaign_is_not_met() {
    let r = run();
    for (e, _, f) in &r.found {
        assert!(judge(*e, f).is_empty());
        // Mis-pinned: the rule holds; the other symptom.
        assert!(!judge(Expect::Holds, f).is_empty(), "{}", f.subject);
        let Expect::Refuted { symptom } = *e else {
            panic!("pinned refuted")
        };
        let other = match symptom {
            Symptom::Disables => Symptom::DifferentStates,
            Symptom::DifferentStates => Symptom::Disables,
        };
        assert!(!judge(Expect::Refuted { symptom: other }, f).is_empty());
        // Doctored results: a bound-stopped exploration; no witness; a witness that does
        // not replay; an Unknown answer from the reducer; an unsound reducer pair; a
        // reduced check that is not established.
        let mut g = f.clone();
        g.states = None;
        assert!(!judge(*e, &g).is_empty());
        let mut g = f.clone();
        g.witness = None;
        assert!(!judge(*e, &g).is_empty());
        let mut g = f.clone();
        g.witness.as_mut().expect("a witness").replayed = false;
        assert!(!judge(*e, &g).is_empty());
        let mut g = f.clone();
        *g.real_on_refuted.entry("definitely-dependent").or_default() -= 1;
        *g.real_on_refuted.entry("unknown").or_default() += 1;
        assert!(!judge(*e, &g).is_empty());
        let mut g = f.clone();
        g.real_independent.2 = 1;
        assert!(!judge(*e, &g).is_empty());
        let mut g = f.clone();
        g.dpor.1 = "inconclusive".to_owned();
        assert!(!judge(*e, &g).is_empty());
    }
    // A real bound-stopped campaign is a gap, never a kill.
    let (id, _, e) = subjects().into_iter().nth(1).expect("the durable subject");
    let m = lower_subject(id);
    let cut = campaign(id, &m, bfs::Bounds::new(8, 64, 1 << 20), SEEDS[0]);
    assert_eq!(cut.states, None);
    assert!(!judge(e, &cut).is_empty());
    // A short work budget makes the reducer answer Unknown, which judge refuses.
    let (i, j) = declared_pairs(&m)[0];
    assert_eq!(dpor::dependence(&m, i, j, 1), DependenceEvidence::Unknown);
}

/// The retained artifact is byte-stable and met.
#[test]
fn m10_the_artifact_is_byte_stable() {
    const PINNED: &str = include_str!("evidence/pr16-exit-m10.json");
    let text = artifact(run());
    if std::env::var_os("PR16_EXIT_BLESS").is_some() {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/evidence/pr16-exit-m10.json"
        );
        std::fs::write(path, &text).expect("the artifact is writable");
        panic!("PR16_EXIT_BLESS rewrote the artifact; rerun without it and review the diff");
    }
    assert!(text.contains("\"verdict\":\"met\""), "{text}");
    assert_eq!(
        text, PINNED,
        "the M10 artifact drifted; regenerate with PR16_EXIT_BLESS=1"
    );
}
