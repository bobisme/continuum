//! The C005 differential: the reduced engine against the unreduced oracle.
//!
//! The oracle is `continuum-engine-reference` — breadth-first exploration of every
//! reachable state, then its own invariant and deadlock check — and nothing on this
//! side shares its decision code. Shared by `tests/c005_differential.rs` and the
//! mutation campaign (`src/mutation.rs`), each of which names the reduced engine
//! `crate::dpor`.
//!
//! One model is compared on four counts:
//!
//! 1. **verdicts** — every invariant's three-way verdict (with the inconclusive
//!    reason) and the deadlock verdict, or both sides report an engine error;
//! 2. **visible reachable states** — the set of reachable states projected onto the
//!    variables the invariants read equals the reduced search's stored set projected
//!    the same way, and the set of terminal states is equal outright;
//! 3. **counterexamples** — every reduced counterexample replays: it starts at an
//!    initial state, each step is a labelled successor in `Model::successors`, every
//!    state on it is in the oracle's reachable set, and it ends at a state that
//!    falsifies its invariant (or is terminal, for a deadlock);
//! 4. **the witness** — the independent checker accepts the reduction witness, and
//!    its own verdicts, terminal states and projections equal the reducer's.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    dead_code
)]

use std::collections::{BTreeMap, BTreeSet};

use continuum_engine_reference as reference;
use continuum_model_core::definedness::{Definedness, Guarded};
use continuum_model_core::model::{Model, State};

use crate::dpor;

/// The corpus bound: at most `2^18` reachable states and the reference engine's own
/// certifiable transition bound (`bfs::MAX_TRANSITIONS`, `2^22`). Every generated and
/// adversarial model completes well inside it; a dossier model that does not is
/// recorded as outside the corpus.
pub fn reference_bounds() -> reference::Bounds {
    reference::Bounds::new(1 << 18, 1 << 18, reference::bfs::MAX_TRANSITIONS)
}

/// Whether the unreduced oracle completes `model` inside the corpus bound; `Err`
/// names the bound it trips.
pub fn fits(model: &Model) -> Result<(), String> {
    match reference::explore(model, reference_bounds()) {
        Ok(exploration) => match exploration.exhausted() {
            None => Ok(()),
            Some(partial) => Err(format!(
                "the unreduced oracle exceeds the corpus bound ({:?} after {} states)",
                partial.tripped(),
                partial.explored().len()
            )),
        },
        Err(_) => Ok(()),
    }
}

pub fn dpor_bounds() -> dpor::Bounds {
    dpor::Bounds::new(1 << 18, 1 << 24, 1 << 20, 1 << 40)
}

/// Which obligations one comparison asks about: the invariant predicate indices and
/// whether a terminal state is a defect.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Scope {
    pub invariants: Vec<usize>,
    pub defect: bool,
}

impl Scope {
    /// Every predicate, and deadlock a defect: the corpus default.
    pub fn every(model: &Model) -> Self {
        Self {
            invariants: (0..model.predicates().len()).collect(),
            defect: true,
        }
    }

    pub fn dpor(&self) -> dpor::Obligations {
        let policy = if self.defect {
            dpor::DeadlockPolicy::Defect
        } else {
            dpor::DeadlockPolicy::Allowed
        };
        self.invariants
            .iter()
            .fold(dpor::Obligations::new(policy), |o, i| o.invariant(*i))
    }

    fn reference(&self) -> reference::Obligations {
        let policy = if self.defect {
            reference::DeadlockPolicy::Defect
        } else {
            reference::DeadlockPolicy::Allowed
        };
        self.invariants
            .iter()
            .fold(reference::Obligations::new(policy), |o, i| o.invariant(*i))
    }
}

/// The variables every predicate reads, one bit per variable.
pub fn visible_mask(model: &Model) -> u64 {
    visible_mask_of(model, &Scope::every(model))
}

/// The variables the check reads under `scope`, one bit per variable: the scope's
/// invariants, their definedness chains, and every action's definedness chain
/// (RFC 0003; model-core's `Definedness`). The reduced engine keeps exactly these
/// visible, so the reachable states are compared projected onto them.
pub fn visible_mask_of(model: &Model, scope: &Scope) -> u64 {
    let definedness = Definedness::of(model);
    let mut read: BTreeSet<usize> = definedness.action_chains().flatten().copied().collect();
    for index in &scope.invariants {
        read.extend(definedness.guards_of(*index).iter().copied());
        read.insert(*index);
    }
    let mut mask = 0_u64;
    for predicate in read.iter().map(|i| &model.predicates()[*i]) {
        let mut names = Vec::new();
        predicate.body().variables(&mut names);
        for name in names {
            mask |= 1_u64 << model.variable_index(&name).unwrap();
        }
    }
    mask
}

pub fn project(state: &State, mask: u64) -> Vec<i64> {
    state
        .as_slice()
        .iter()
        .enumerate()
        .filter(|(index, _)| mask & (1_u64 << index) != 0)
        .map(|(_, value)| *value)
        .collect()
}

/// The oracle's answer.
pub struct Oracle {
    /// `None` when exploration hit an evaluation error.
    pub verdicts: Option<Vec<String>>,
    pub states: usize,
    pub transitions: u64,
    pub reachable: BTreeSet<State>,
    pub projections: BTreeSet<Vec<i64>>,
    /// The terminal states the reference engine reports under `Defect` (empty under
    /// `Allowed`, where it reports only a count).
    pub deadlocks: BTreeSet<State>,
    /// Every reachable terminal state, from the raw exploration, whatever the policy.
    pub terminals: BTreeSet<State>,
    /// On a faulted model: every reachable state whose successor row fails to
    /// evaluate, from an unreduced walk that does not expand such states.
    pub faults: BTreeSet<State>,
}

/// An unreduced walk that records every reachable state whose successor row fails
/// to evaluate and does not expand it: the set of faults a reachable fault can be.
/// Model-layer primitives only; it shares nothing with the reducer.
pub fn fault_walk(model: &Model) -> BTreeSet<State> {
    let mut seen: BTreeSet<State> = model.initial_states().iter().cloned().collect();
    let mut queue: Vec<State> = seen.iter().cloned().collect();
    let mut faults = BTreeSet::new();
    while let Some(state) = queue.pop() {
        assert!(
            seen.len() <= 1 << 18,
            "the fault walk stays inside the corpus bound"
        );
        match model.successors(&state) {
            Err(_) => {
                faults.insert(state);
            }
            Ok(row) => {
                for step in row {
                    if seen.insert(step.target().clone()) {
                        queue.push(step.target().clone());
                    }
                }
            }
        }
    }
    faults
}

fn reference_reason(reason: &reference::Unresolved) -> String {
    reason.as_str().to_owned()
}

pub fn oracle(model: &Model) -> Oracle {
    oracle_of(model, &Scope::every(model))
}

pub fn oracle_of(model: &Model, scope: &Scope) -> Oracle {
    let mask = visible_mask_of(model, scope);
    let exploration = match reference::explore(model, reference_bounds()) {
        Ok(exploration) => exploration,
        Err(reference::ExplorationError::Evaluation { .. }) => {
            return Oracle {
                verdicts: None,
                states: 0,
                transitions: 0,
                reachable: BTreeSet::new(),
                projections: BTreeSet::new(),
                deadlocks: BTreeSet::new(),
                terminals: BTreeSet::new(),
                faults: fault_walk(model),
            };
        }
        Err(other) => panic!("oracle refused the corpus bounds: {other}"),
    };
    if let Some(partial) = exploration.exhausted() {
        panic!(
            "the oracle completes every corpus model: {:?} after {} states",
            partial.tripped(),
            partial.explored().len()
        );
    }
    let obligations = scope.reference();
    let report =
        reference::check(model, &exploration, &obligations).expect("obligations are declared");
    let mut verdicts: Vec<String> = report
        .invariants()
        .iter()
        .map(|result| match result.outcome() {
            reference::CheckOutcome::Holds { .. } => "established".to_owned(),
            reference::CheckOutcome::Violated { .. } => "refuted".to_owned(),
            reference::CheckOutcome::Undefined(undefined) => {
                spell_undefined(undefined.read(), undefined.subject())
            }
            reference::CheckOutcome::Inconclusive(reason) => {
                format!("inconclusive:{}", reference_reason(reason))
            }
        })
        .collect();
    let deadlocks: BTreeSet<State> = report
        .deadlock()
        .deadlocks()
        .iter()
        .map(|deadlock| deadlock.state().clone())
        .collect();
    verdicts.push(match report.deadlock() {
        reference::DeadlockOutcome::Free { .. } | reference::DeadlockOutcome::NotJudged { .. } => {
            "established".to_owned()
        }
        reference::DeadlockOutcome::Deadlocked { .. } => "refuted".to_owned(),
        reference::DeadlockOutcome::Undefined(undefined) => {
            spell_undefined(undefined.read(), undefined.subject())
        }
        reference::DeadlockOutcome::Inconclusive(reason) => {
            format!("inconclusive:{}", reference_reason(reason))
        }
    });
    let reachable: BTreeSet<State> = exploration.reachable().states().iter().cloned().collect();
    // State-changing (action, target) steps: the reference's own transition count
    // also counts self-loops, which the reduced engine never fires.
    let progress: usize = reachable
        .iter()
        .map(|state| {
            model
                .successors(state)
                .expect("the oracle evaluated every row")
                .iter()
                .filter(|step| step.target() != state)
                .count()
        })
        .sum();
    let terminals: BTreeSet<State> = reachable
        .iter()
        .filter(|state| model.successors(state).is_ok_and(|row| row.is_empty()))
        .cloned()
        .collect();
    if scope.defect
        && matches!(
            report.deadlock(),
            reference::DeadlockOutcome::Deadlocked { .. } | reference::DeadlockOutcome::Free { .. }
        )
    {
        // The harness's reading of "terminal" is the reference engine's own (where the
        // reference lists terminal states: not under an undefined action read).
        assert_eq!(
            deadlocks, terminals,
            "raw terminal set against the reference's list"
        );
    }
    Oracle {
        terminals,
        faults: BTreeSet::new(),
        verdicts: Some(verdicts),
        states: reachable.len(),
        transitions: progress as u64,
        projections: reachable.iter().map(|state| project(state, mask)).collect(),
        reachable,
        deadlocks,
    }
}

/// An undefined read in both engines' spelling: an action read by its kind alone
/// (which action's chain a search meets first depends on its order; any one
/// invalidates the model), a predicate read by its kind and its subject.
pub fn spell_undefined(read: Guarded, subject: &str) -> String {
    match read {
        Guarded::Action => "undefined:action".to_owned(),
        Guarded::Predicate(_) => format!("undefined:predicate:{subject}"),
    }
}

/// The states marked by an undefined read, from the oracle's reachable set, with
/// model-core's `Definedness` and the reference engine's order: `action` holds every
/// state where some action's chain is false; `subject[i]` every state (with every
/// action defined) where invariant `invariants[i]`'s own chain, or the invariant
/// when it is itself a definedness predicate, is false.
pub struct Marked {
    pub action: BTreeSet<State>,
    pub subject: Vec<BTreeSet<State>>,
}

pub fn marked(model: &Model, reachable: &BTreeSet<State>, invariants: &[usize]) -> Marked {
    let definedness = Definedness::of(model);
    let holds = |index: usize, state: &State| model.evaluate_predicate(index, state) != Ok(false);
    let mut out = Marked {
        action: BTreeSet::new(),
        subject: vec![BTreeSet::new(); invariants.len()],
    };
    for state in reachable {
        let action = definedness
            .action_chains()
            .any(|chain| chain.iter().any(|&guard| !holds(guard, state)));
        if action {
            out.action.insert(state.clone());
            continue;
        }
        for (slot, index) in invariants.iter().enumerate() {
            let undefined = definedness
                .guards_of(*index)
                .iter()
                .any(|&g| !holds(g, state))
                || (definedness.guards(*index).is_some() && !holds(*index, state));
            if undefined {
                out.subject[slot].insert(state.clone());
            }
        }
    }
    out
}

/// The reduced engine's verdicts in the oracle's spelling, or `None` for an engine
/// error.
pub fn dpor_verdicts(report: &dpor::Report) -> Option<Vec<String>> {
    if let dpor::Completeness::Faulted(_) = report.completeness() {
        return None;
    }
    let spell = |verdict: dpor::Verdict, reason: Option<&dpor::Unresolved>| match (verdict, reason)
    {
        (dpor::Verdict::Inconclusive, Some(reason)) => format!("inconclusive:{}", reason.as_str()),
        (verdict, _) => verdict.as_str().to_owned(),
    };
    let mut verdicts: Vec<String> = report
        .invariants()
        .iter()
        .map(|result| match result.outcome() {
            dpor::InvariantOutcome::Undefined(read) => spell_undefined(read.read(), read.subject()),
            dpor::InvariantOutcome::Inconclusive(reason) => {
                spell(dpor::Verdict::Inconclusive, Some(reason))
            }
            outcome => spell(outcome.verdict(), None),
        })
        .collect();
    verdicts.push(match report.deadlock() {
        dpor::DeadlockOutcome::Undefined(read) => spell_undefined(read.read(), read.subject()),
        dpor::DeadlockOutcome::Inconclusive(reason) => {
            spell(dpor::Verdict::Inconclusive, Some(reason))
        }
        outcome => spell(outcome.verdict(), None),
    });
    Some(verdicts)
}

/// The comparison categories, in ledger order.
pub const CATEGORIES: [&str; 7] = [
    "verdicts",
    "projections",
    "terminals",
    "traces",
    "witness",
    "fault",
    "undefined",
];

/// Every way one model's reduced answer differs from the oracle's, and how many
/// comparisons of each category were actually made. Empty disagreements with
/// non-zero comparisons is agreement; zero comparisons is not evidence.
#[derive(Debug, Default)]
pub struct Comparison {
    pub disagreements: Vec<String>,
    /// The categories that disagreed.
    pub failed: BTreeSet<&'static str>,
    /// Comparisons made, by category (non-vacuous ones: a terminal comparison
    /// counts only over a non-empty set, a projection one always holds a state).
    pub made: BTreeMap<&'static str, usize>,
    pub checker_rejected: Option<String>,
    pub engine_error: bool,
    pub traces_replayed: usize,
    /// Any verdict of either engine is inconclusive.
    pub inconclusive: bool,
}

impl Comparison {
    fn fail(&mut self, category: &'static str, message: String) {
        self.failed.insert(category);
        self.disagreements.push(message);
    }

    fn count(&mut self, category: &'static str) {
        *self.made.entry(category).or_default() += 1;
    }
}

impl Comparison {
    /// Whether the differential proper (verdicts, projections, deadlocks, traces)
    /// caught a difference.
    pub fn differential_detects(&self) -> bool {
        !self.disagreements.is_empty()
    }
}

fn replays(model: &Model, oracle: &Oracle, trace: &dpor::Trace) -> Result<(), String> {
    if model.initial_states().binary_search(trace.start()).is_err() {
        return Err("trace does not start at an initial state".to_owned());
    }
    let mut here = trace.start().clone();
    for step in trace.steps() {
        let row = model
            .successors(&here)
            .map_err(|e| format!("successor row: {e}"))?;
        if !row
            .iter()
            .any(|entry| entry.action() == step.action() && entry.target() == step.target())
        {
            return Err(format!(
                "step {} -> {} is not a successor",
                step.action(),
                step.target()
            ));
        }
        if !oracle.reachable.contains(step.target()) {
            return Err("trace leaves the oracle's reachable set".to_owned());
        }
        here = step.target().clone();
    }
    Ok(())
}

/// Compare one reduced report with the oracle.
pub fn compare(model: &Model, oracle: &Oracle, report: &dpor::Report) -> Comparison {
    compare_of(model, &Scope::every(model), oracle, report)
}

/// Compare one reduced report with the oracle under `scope`.
pub fn compare_of(
    model: &Model,
    scope: &Scope,
    oracle: &Oracle,
    report: &dpor::Report,
) -> Comparison {
    let mut out = Comparison::default();
    let mine = dpor_verdicts(report);
    match (&oracle.verdicts, &mine) {
        (None, None) => {
            // Both engines stop at the first evaluation fault they reach, so what is
            // compared is the existence of a reachable fault: the reduced engine's
            // fault state must be one of the reachable faults an unreduced walk finds.
            out.engine_error = true;
            out.count("fault");
            match report.completeness() {
                dpor::Completeness::Faulted(dpor::EngineFault::Evaluation {
                    state,
                    action: Some(_),
                    ..
                }) if oracle.faults.contains(state) => {}
                other => out.fail(
                    "fault",
                    format!("reduced fault is not a reachable fault: {other:?}"),
                ),
            }
            return out;
        }
        (Some(theirs), Some(mine)) => {
            out.count("verdicts");
            out.inconclusive = theirs
                .iter()
                .chain(mine.iter())
                .any(|verdict| verdict.starts_with("inconclusive"));
            if theirs != mine {
                out.fail(
                    "verdicts",
                    format!("verdicts: oracle {theirs:?} reduced {mine:?}"),
                );
            }
        }
        (theirs, mine) => {
            out.fail(
                "fault",
                format!(
                    "engine error: oracle {:?} reduced {:?}",
                    theirs.is_none(),
                    mine.is_none()
                ),
            );
            return out;
        }
    }
    if *report.completeness() != dpor::Completeness::Complete {
        out.fail(
            "verdicts",
            format!("reduced search incomplete: {:?}", report.completeness()),
        );
    }
    // The reduced engine's stored states, projected onto the oracle's mask (not the
    // reducer's own, which a faulty reducer could shrink): a lost state shows here,
    // a changed mask does not.
    out.count("projections");
    let mask = visible_mask_of(model, scope);
    let stored: BTreeSet<Vec<i64>> = report
        .witness()
        .nodes()
        .iter()
        .map(|node| project(node.state(), mask))
        .collect();
    if oracle.projections != stored {
        out.fail(
            "projections",
            format!(
                "visible projections: oracle {} reduced {}",
                oracle.projections.len(),
                stored.len()
            ),
        );
    }
    // Terminal states, under either policy: the reduced report's list (`Defect`) or
    // count (`Allowed`) against the raw exploration's terminal set.
    if !oracle.terminals.is_empty() {
        out.count("terminals");
    }
    let mut reduced_deadlocks: BTreeSet<State> = BTreeSet::new();
    match report.deadlock() {
        dpor::DeadlockOutcome::Deadlocked { states } => {
            for deadlock in states {
                reduced_deadlocks.insert(deadlock.state().clone());
                let Some(trace) = deadlock.trace() else {
                    out.fail("traces", "deadlock unwitnessed".to_owned());
                    continue;
                };
                out.count("traces");
                match replays(model, oracle, trace) {
                    Ok(())
                        if model
                            .successors(deadlock.state())
                            .is_ok_and(|row| row.is_empty()) =>
                    {
                        out.traces_replayed += 1;
                    }
                    Ok(()) => out.fail("traces", "deadlock trace ends at a live state".to_owned()),
                    Err(why) => out.fail("traces", format!("deadlock trace: {why}")),
                }
            }
            if reduced_deadlocks != oracle.terminals {
                out.fail(
                    "terminals",
                    format!(
                        "terminal states: oracle {} reduced {}",
                        oracle.terminals.len(),
                        reduced_deadlocks.len()
                    ),
                );
            }
        }
        dpor::DeadlockOutcome::NotJudged { terminal } => {
            if *terminal != oracle.terminals.len() {
                out.fail(
                    "terminals",
                    format!(
                        "terminal count: oracle {} reduced {terminal}",
                        oracle.terminals.len()
                    ),
                );
            }
        }
        dpor::DeadlockOutcome::Free { .. } => {
            if !oracle.terminals.is_empty() {
                out.fail("terminals", "reduced reports no terminal state".to_owned());
            }
        }
        // The undefined read is compared above; the terminal set still is, by the checker.
        dpor::DeadlockOutcome::Undefined(_) | dpor::DeadlockOutcome::Inconclusive(_) => {}
    }
    // Undefined reads: each reduced one must be a marked state of its kind in the
    // oracle's reachable set, reached by a trace that replays.
    let marks = marked(model, &oracle.reachable, &scope.invariants);
    let mut reads: Vec<(Option<usize>, &dpor::UndefinedRead)> = report
        .invariants()
        .iter()
        .enumerate()
        .filter_map(|(slot, result)| match result.outcome() {
            dpor::InvariantOutcome::Undefined(read) => Some((Some(slot), &**read)),
            _ => None,
        })
        .collect();
    if let dpor::DeadlockOutcome::Undefined(read) = report.deadlock() {
        reads.push((None, &**read));
    }
    let definedness = Definedness::of(model);
    for (slot, read) in reads {
        out.count("undefined");
        // The read itself: its guard is false at its state, and is classified as
        // model-core classifies it.
        if model.evaluate_predicate(read.guard(), read.state()) != Ok(false)
            || definedness.guards(read.guard()).unwrap_or(Guarded::Action) != read.read()
        {
            out.fail(
                "undefined",
                format!(
                    "undefined read {} is not a false guard of its kind",
                    read.guard_name()
                ),
            );
        }
        let marked_here = match (read.read(), slot) {
            (Guarded::Action, _) => marks.action.contains(read.state()),
            (Guarded::Predicate(_), Some(slot)) => marks
                .subject
                .get(slot)
                .is_some_and(|set| set.contains(read.state())),
            (Guarded::Predicate(_), None) => false,
        };
        if !marked_here {
            out.fail(
                "undefined",
                format!("undefined read in {} at an unmarked state", read.subject()),
            );
        }
        match read
            .trace()
            .map(|trace| (trace, replays(model, oracle, trace)))
        {
            Some((trace, Ok(()))) if trace.end() == read.state() => out.traces_replayed += 1,
            other => out.fail(
                "undefined",
                format!(
                    "undefined read trace does not replay: {:?}",
                    other.map(|(_, r)| r)
                ),
            ),
        }
    }
    for result in report.invariants() {
        if let dpor::InvariantOutcome::Violated { state, trace } = result.outcome() {
            let Some(trace) = trace else {
                out.fail(
                    "traces",
                    format!("counterexample for {} unwitnessed", result.name()),
                );
                continue;
            };
            out.count("traces");
            if trace.end() != state {
                out.fail(
                    "traces",
                    format!("counterexample for {} ends elsewhere", result.name()),
                );
            }
            match replays(model, oracle, trace) {
                Ok(()) if model.evaluate_predicate(result.index(), trace.end()) == Ok(false) => {
                    out.traces_replayed += 1;
                }
                Ok(()) => out.fail(
                    "traces",
                    format!(
                        "counterexample for {} ends at a satisfying state",
                        result.name()
                    ),
                ),
                Err(why) => out.fail("traces", format!("counterexample: {why}")),
            }
        }
    }

    let obligations = scope.dpor();
    match dpor::check_witness(model, &obligations, report.witness(), 1 << 40) {
        Err(defect) => out.checker_rejected = Some(format!("{defect:?}")),
        Ok(checked) => {
            out.count("witness");
            // The checker lists every stored terminal state under either policy: it
            // must be the raw exploration's terminal set (a reduction that lost a
            // terminal state is caught here even when no verdict reads it).
            let checked_deadlocks: BTreeSet<State> = checked.deadlocks.iter().cloned().collect();
            if checked_deadlocks != oracle.terminals {
                out.fail(
                    "terminals",
                    format!(
                        "terminal states (checker, {}): oracle {} reduced {}",
                        if scope.defect { "defect" } else { "allowed" },
                        oracle.terminals.len(),
                        checked_deadlocks.len()
                    ),
                );
            }
            if checked.projections != *report.projections() {
                out.checker_rejected =
                    Some("checker's own account differs from the reducer's".to_owned());
            }
            // Exact agreement with the checker, which scans the stored states in node
            // order as the reducer judges them: the same guard at the same state.
            let reduced_action = match report.deadlock() {
                dpor::DeadlockOutcome::Undefined(read) => {
                    Some((read.guard(), read.state().clone()))
                }
                _ => None,
            };
            for ((_, verdict), result) in checked.invariants.iter().zip(report.invariants()) {
                if let (
                    dpor::CheckedInvariant::Undefined { guard, state },
                    dpor::InvariantOutcome::Undefined(read),
                ) = (verdict, result.outcome())
                    && (*guard != read.guard() || state != read.state())
                {
                    out.checker_rejected = Some(format!(
                        "checker and reducer name different undefined reads for {}",
                        result.name()
                    ));
                }
            }
            if checked.undefined_action != reduced_action {
                out.checker_rejected =
                    Some("checker and reducer disagree on an undefined action read".to_owned());
            }
            for ((index, verdict), result) in checked.invariants.iter().zip(report.invariants()) {
                let agrees = *index == result.index()
                    && matches!(
                        (verdict, result.outcome()),
                        (
                            dpor::CheckedInvariant::Undefined { .. },
                            dpor::InvariantOutcome::Undefined(_)
                        ) | (
                            dpor::CheckedInvariant::Holds,
                            dpor::InvariantOutcome::Holds { .. }
                        ) | (
                            dpor::CheckedInvariant::Violated(_),
                            dpor::InvariantOutcome::Violated { .. }
                        ) | (
                            dpor::CheckedInvariant::EvaluationFailed,
                            dpor::InvariantOutcome::Inconclusive(dpor::Unresolved::EngineError(_))
                        )
                    );
                if !agrees {
                    out.checker_rejected = Some(format!("checker verdict for {index} differs"));
                }
            }
        }
    }
    out
}
