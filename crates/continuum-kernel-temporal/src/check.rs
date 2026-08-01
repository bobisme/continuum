//! The temporal checkers: a ranking replayed, and a fair cycle searched for (PR 9).
//!
//! # What is being re-derived
//!
//! > Finite graph: SCC decomposition; reachable-component witnesses; fairness
//! > acceptance labels; absence of accepting SCCs or explicit fair-cycle witness.
//! >
//! > Ranking: well-founded domain; decrease obligations; fairness-to-progress linkage;
//! > safety side conditions.
//! >
//! > — `notes/plan/docs/03_ASSURANCE_AND_TCB.md` §6.5, "Liveness"
//!
//! Both families are checked over the same carried structure: a canonical state table,
//! a total successor relation, an explicit goal set, and — for the exclusion family — a
//! set of weakly fair actions. Four obligations are common to both, and they are the
//! "safety side conditions" of the sentence above:
//!
//! 1. every table state lies in the declared state domain (docs/16 PO-MOD-003);
//! 2. every declared initial state is in the table;
//! 3. every declared goal state is in the table;
//! 4. every transition names a declared action and lands in the table.
//!
//! The fourth is the one that makes the temporal claims mean anything: an execution of
//! the carried system stays inside the table, so a statement quantified over table
//! states is a statement about every execution. Successors are carried as state
//! *vectors*, so a producer that stopped exploring cannot hide it.
//!
//! # The ranking family
//!
//! Then, for [`CertificateKind::Ranking`]:
//!
//! - every goal state has rank `0`;
//! - every non-goal state has at least one successor;
//! - every transition out of a non-goal state strictly decreases the rank.
//!
//! The conclusion is unconditional: ranks are natural numbers, `<` on `ℕ` is
//! well-founded (docs/16 PO-LIV-002 — "Ranking domain has no infinite descending
//! chain" — discharged by the choice of domain rather than by a carried argument), so
//! no execution can take more than `rank(s)` non-goal steps, and totality means it
//! never stops before it gets there. From every state `s` of the table, every execution
//! reaches the goal set within `rank(s)` steps. docs/16 PO-LIV-003's "decrease" is
//! checked in its strongest form — *every* step decreases, not merely the fair ones —
//! which is why no fairness assumption enters the claim and none appears in
//! [`CheckedClaim::trusted_components`].
//!
//! The obligation is discharged at every table state, reachable or not. That is
//! deliberately stronger than PO-LIV-004's "reachable SCC" phrasing, and it costs
//! nothing: padding the table with unreachable states can only make a ranking harder to
//! satisfy.
//!
//! # The fair-cycle-exclusion family
//!
//! For [`CertificateKind::FairSccExclusion`], the kernel recomputes two things the
//! certificate does *not* carry, so that neither can be asserted:
//!
//! - **reachability** from the declared initial states, by breadth-first search over
//!   the carried successor relation;
//! - **the strongly connected components** of the reachable non-goal subgraph, by an
//!   iterative Tarjan search.
//!
//! A component is *nontrivial* if it has two or more states, or one state with a
//! self-loop — that is, if it carries a cycle. A nontrivial component is *fair* when
//! every declared fair action `a` is either taken by an edge inside the component, or
//! disabled at some state of the component. That is weak fairness read on a cycle: an
//! action continuously enabled throughout the cycle and never taken would be a justice
//! violation, so such a cycle is not a legal execution and does not refute anything.
//!
//! The certificate is rejected exactly when a fair nontrivial component exists, and the
//! rejection names a state on it ([`Rejection::FairCycleExists`]).
//!
//! Two things make that exact rather than approximate. If a fair execution avoided the
//! goal set forever, the set of states it visits infinitely often would be a strongly
//! connected set of reachable non-goal states, contained in some maximal component `D`;
//! every fair action it takes inside that set is an edge inside `D`, and every fair
//! action disabled somewhere in it is disabled somewhere in `D` — so `D` would be fair,
//! and the search would have found it. Conversely, a fair component admits an execution
//! that cycles through all of it forever, taking every internal fair edge infinitely
//! often and visiting every disabling state infinitely often; that execution satisfies
//! weak fairness and never reaches the goal. So the search neither over- nor
//! under-approximates, and a rejection is a genuine fair lasso.
//!
//! That last construction is the Revision 2 fair-lasso spike's rule, lifted from a
//! single lasso to a component: `check_eventually_done_lasso` in
//! `notes/plan/spikes/advanced_spikes.py` rejects a candidate counterexample when a
//! weakly fair action is "continuously enabled but never taken" on the cycle, and its
//! frozen results (`spikes/results/spike-results.json`, `advanced.temporal_fair_lasso`)
//! are reproduced from wire form by this crate's fixture tests.
//!
//! # Why the kernel recomputes the components instead of checking a carried one
//!
//! docs/03 §6.5 lists "SCC decomposition" among the certificate's contents, and a
//! producer-supplied decomposition *could* be validated. Recomputing is the better
//! trade here: validating a claimed partition means checking both strong connectivity
//! and maximality, which is as much code as computing it and has a subtler failure
//! mode, while Tarjan's search is linear in the graph the certificate already carries.
//! The certificate stays smaller and the kernel stays honest — nothing about the
//! component structure is taken on trust.
//!
//! # Determinism and recursion
//!
//! The search carries its own stack on the heap, so a long path is an allocation rather
//! than a stack overflow — RFC 0005's "cycle/recursion bounds" as a structural property.
//! Components are reported by their smallest table index, and every loop iterates over
//! sequences whose order is a decode-time invariant, so two runs over the same bytes
//! produce byte-identical verdicts on any platform (INV-005; ADR-0003; docs/12 §1).

use crate::verdict::{
    CertificateKind, CheckedClaim, FairnessClass, Feature, PropertyClass, Rejection, Verdict,
};
use crate::wire::{DecodeFailure, Envelope, StateDomain, StateTable, TemporalBody, decode};

/// Check one temporal certificate from its wire form.
///
/// The crate's whole public checking surface. It takes bytes — never a structure an
/// engine could hand over — and returns the closed [`Verdict`] vocabulary. It cannot
/// panic on any input: see the module documentation of [`crate::wire`] and the
/// `garbage_bytes_never_panic` test.
#[must_use]
pub fn check_certificate(bytes: &[u8]) -> Verdict {
    let certificate = match decode(bytes) {
        Ok(certificate) => certificate,
        Err(DecodeFailure::Rejected(rejection)) => return Verdict::Rejected(rejection),
        Err(DecodeFailure::Unsupported(feature)) => return Verdict::Unsupported(feature),
    };
    check_temporal(
        certificate.envelope(),
        certificate.kind(),
        certificate.body(),
    )
}

/// Everything both families establish before they diverge.
struct Common {
    goal: Vec<bool>,
    initial: Vec<u32>,
    successors: Vec<Vec<u32>>,
    transitions: u64,
}

/// Run the four shared obligations.
fn check_common(body: &TemporalBody) -> Result<Common, Rejection> {
    if let Some(rejection) = state_outside_domain(body.domain(), body.table()) {
        return Err(rejection);
    }

    let action_count = narrow_u16(body.actions().len());
    for (position, action) in body.fair_actions().iter().enumerate() {
        if *action >= action_count {
            return Err(Rejection::FairActionOutOfRange {
                position: narrow_u16(position),
                action: *action,
                actions: action_count,
            });
        }
    }

    let states = body.table().len() as usize;
    let mut goal = vec![false; states];
    for (position, state) in body.goal_states().iter().enumerate() {
        let Some(index) = body.table().position(state) else {
            return Err(Rejection::GoalStateNotInTable {
                position: narrow_u32(position),
            });
        };
        raise(&mut goal, index as usize);
    }

    let mut initial: Vec<u32> = Vec::new();
    for (position, state) in body.initial_states().iter().enumerate() {
        let Some(index) = body.table().position(state) else {
            return Err(Rejection::InitialStateNotInTable {
                position: narrow_u32(position),
            });
        };
        initial.push(index);
    }

    let mut successors: Vec<Vec<u32>> = Vec::new();
    let mut transitions: u64 = 0;
    for (state_index, row) in body.rows().iter().enumerate() {
        let state = narrow_u32(state_index);
        let mut targets: Vec<u32> = Vec::new();
        for (entry_index, transition) in row.iter().enumerate() {
            let entry = narrow_u32(entry_index);
            if transition.action() >= action_count {
                return Err(Rejection::UnknownAction {
                    state,
                    entry,
                    action: transition.action(),
                });
            }
            let Some(target) = body.table().position(transition.target()) else {
                return Err(Rejection::ClosureFailure { state, entry });
            };
            targets.push(target);
            transitions = transitions.saturating_add(1);
        }
        successors.push(targets);
    }

    Ok(Common {
        goal,
        initial,
        successors,
        transitions,
    })
}

/// Dispatch to the family's own obligations.
fn check_temporal(envelope: &Envelope, kind: CertificateKind, body: &TemporalBody) -> Verdict {
    let class_code = body.property_class_code();
    let Some(property) = PropertyClass::from_code(class_code) else {
        return Verdict::Unsupported(Feature::PropertyClass { found: class_code });
    };
    let fairness_code = body.fairness_class_code();
    let Some(fairness) = FairnessClass::from_code(fairness_code) else {
        return Verdict::Unsupported(Feature::FairnessClass {
            found: fairness_code,
        });
    };

    let common = match check_common(body) {
        Ok(common) => common,
        Err(rejection) => return Verdict::Rejected(rejection),
    };

    let states = body.table().len();
    let fair_actions = narrow_u16(body.fair_actions().len());
    let claim = |reachable: u32, bound: Option<u64>| {
        CheckedClaim::new(
            kind,
            property,
            fairness,
            envelope.clone(),
            states,
            narrow_u32(common.initial.len()),
            narrow_u32(body.goal_states().len()),
            reachable,
            common.transitions,
            fair_actions,
            bound,
        )
    };

    match kind {
        CertificateKind::Ranking => {
            if fair_actions != 0 {
                return Verdict::Unsupported(Feature::FairnessDischarge {
                    declared: fair_actions,
                });
            }
            match check_ranking(body, &common) {
                Ok(bound) => Verdict::Verified(claim(states, Some(bound))),
                Err(rejection) => Verdict::Rejected(rejection),
            }
        }
        CertificateKind::FairSccExclusion => {
            let reachable = reachable_states(&common);
            let in_scope = scope(&reachable, &common.goal);
            if let Some(state) = fair_cycle(body, &common.successors, &in_scope) {
                return Verdict::Rejected(Rejection::FairCycleExists { state });
            }
            let count = narrow_u32(reachable.iter().filter(|flag| **flag).count());
            Verdict::Verified(claim(count, None))
        }
    }
}

// ---------------------------------------------------------------------------
// the ranking family
// ---------------------------------------------------------------------------

/// Replay the decrease obligation, returning the largest rank.
///
/// The two `unwrap_or` fallbacks below are unreachable: the decoder reads exactly one
/// rank per table state, so both lookups are in range. They are written to fail closed
/// anyway — a missing source rank reads as `0`, which forces a non-goal state to fail,
/// and a missing target rank reads as `u64::MAX`, which no source rank can exceed.
fn check_ranking(body: &TemporalBody, common: &Common) -> Result<u64, Rejection> {
    let mut bound: u64 = 0;
    for index in 0..body.table().len() {
        let slot = index as usize;
        let rank = body.ranks().get(slot).copied().unwrap_or(0);
        bound = bound.max(rank);

        if common.goal.get(slot).copied().unwrap_or(false) {
            if rank != 0 {
                return Err(Rejection::GoalRankNotZero { state: index, rank });
            }
            continue;
        }

        let targets = common.successors.get(slot).map_or(&[][..], Vec::as_slice);
        if targets.is_empty() {
            return Err(Rejection::ProgressDeadlock { state: index });
        }
        for (entry, target) in targets.iter().enumerate() {
            let target_rank = body
                .ranks()
                .get(*target as usize)
                .copied()
                .unwrap_or(u64::MAX);
            if target_rank >= rank {
                return Err(Rejection::RankDoesNotDecrease {
                    state: index,
                    entry: narrow_u32(entry),
                    from: rank,
                    to: target_rank,
                });
            }
        }
    }
    Ok(bound)
}

// ---------------------------------------------------------------------------
// the fair-cycle-exclusion family
// ---------------------------------------------------------------------------

/// Breadth-first reachability from the declared initial states.
///
/// Recomputed, never carried: PO-LIV-004 speaks of a *reachable* component, and a
/// certificate that got to say which states are reachable could exclude the interesting
/// ones by omission.
fn reachable_states(common: &Common) -> Vec<bool> {
    let mut reachable = vec![false; common.successors.len()];
    let mut frontier: Vec<u32> = Vec::new();
    for state in &common.initial {
        if !reachable.get(*state as usize).copied().unwrap_or(true) {
            raise(&mut reachable, *state as usize);
            frontier.push(*state);
        }
    }
    while let Some(state) = frontier.pop() {
        let targets = common
            .successors
            .get(state as usize)
            .map_or(&[][..], Vec::as_slice);
        for target in targets {
            if !reachable.get(*target as usize).copied().unwrap_or(true) {
                raise(&mut reachable, *target as usize);
                frontier.push(*target);
            }
        }
    }
    reachable
}

/// The subgraph the component search runs over: reachable and not yet at the goal.
fn scope(reachable: &[bool], goal: &[bool]) -> Vec<bool> {
    let mut scope = vec![false; reachable.len()];
    for (index, flag) in scope.iter_mut().enumerate() {
        *flag = reachable.get(index).copied().unwrap_or(false)
            && !goal.get(index).copied().unwrap_or(false);
    }
    scope
}

/// The smallest index of a state on a fair nontrivial component, or `None`.
///
/// Tarjan's algorithm, iteratively: `call` is the explicit recursion stack, holding a
/// state and how far its successor list has been consumed, and `stack` is Tarjan's own.
/// Indices count from one so that zero can mean "not yet visited".
fn fair_cycle(body: &TemporalBody, successors: &[Vec<u32>], in_scope: &[bool]) -> Option<u32> {
    let states = successors.len();
    let mut discovery = vec![0_u32; states];
    let mut low = vec![0_u32; states];
    let mut on_stack = vec![false; states];
    let mut membership = vec![false; states];
    let mut stack: Vec<u32> = Vec::new();
    let mut call: Vec<(u32, u32)> = Vec::new();
    let mut next: u32 = 1;

    for root in 0..states {
        if !in_scope.get(root).copied().unwrap_or(false) {
            continue;
        }
        if discovery.get(root).copied().unwrap_or(0) != 0 {
            continue;
        }
        let root_index = narrow_u32(root);
        write(&mut discovery, root, next);
        write(&mut low, root, next);
        next = next.saturating_add(1);
        stack.push(root_index);
        raise(&mut on_stack, root);
        call.push((root_index, 0));

        while let Some((state, cursor)) = call.last().copied() {
            let slot = state as usize;
            let targets = successors.get(slot).map_or(&[][..], Vec::as_slice);
            let mut position = cursor as usize;
            let mut descended = false;
            while let Some(target) = targets.get(position) {
                position = position.saturating_add(1);
                let target_slot = *target as usize;
                if !in_scope.get(target_slot).copied().unwrap_or(false) {
                    continue;
                }
                if let Some(entry) = call.last_mut() {
                    entry.1 = narrow_u32(position);
                }
                if discovery.get(target_slot).copied().unwrap_or(0) == 0 {
                    write(&mut discovery, target_slot, next);
                    write(&mut low, target_slot, next);
                    next = next.saturating_add(1);
                    stack.push(*target);
                    raise(&mut on_stack, target_slot);
                    call.push((*target, 0));
                } else if on_stack.get(target_slot).copied().unwrap_or(false) {
                    let seen = discovery.get(target_slot).copied().unwrap_or(0);
                    let current = low.get(slot).copied().unwrap_or(0);
                    write(&mut low, slot, current.min(seen));
                }
                descended = true;
                break;
            }
            if descended {
                continue;
            }

            call.pop();
            let finished = low.get(slot).copied().unwrap_or(0);
            if let Some((parent, _)) = call.last().copied() {
                let parent_slot = parent as usize;
                let current = low.get(parent_slot).copied().unwrap_or(0);
                write(&mut low, parent_slot, current.min(finished));
            }
            if finished != discovery.get(slot).copied().unwrap_or(0) {
                continue;
            }

            let mut component: Vec<u32> = Vec::new();
            while let Some(member) = stack.pop() {
                lower(&mut on_stack, member as usize);
                component.push(member);
                if member == state {
                    break;
                }
            }
            for member in &component {
                raise(&mut membership, *member as usize);
            }
            let fair = carries_cycle(successors, in_scope, &component)
                && is_fair(body, successors, &membership, &component);
            for member in &component {
                lower(&mut membership, *member as usize);
            }
            if fair {
                return component.iter().copied().min();
            }
        }
    }
    None
}

/// Whether a component contains a cycle: two or more states, or one with a self-loop.
fn carries_cycle(successors: &[Vec<u32>], in_scope: &[bool], component: &[u32]) -> bool {
    if component.len() > 1 {
        return true;
    }
    let Some(state) = component.first() else {
        return false;
    };
    let targets = successors
        .get(*state as usize)
        .map_or(&[][..], Vec::as_slice);
    targets
        .iter()
        .any(|target| target == state && in_scope.get(*target as usize).copied().unwrap_or(false))
}

/// Whether an execution that cycles through this component forever satisfies weak
/// fairness for every declared fair action.
///
/// For each fair action, either the component takes it — an edge labelled with it whose
/// target is inside the component — or some state of the component disables it, meaning
/// that state's row carries no transition with that label. An action that is enabled at
/// every state of the component and never taken inside it is exactly the "continuously
/// enabled but never taken" case weak fairness forbids, and its presence makes the
/// component unfair, hence not a counterexample.
fn is_fair(
    body: &TemporalBody,
    successors: &[Vec<u32>],
    membership: &[bool],
    component: &[u32],
) -> bool {
    for action in body.fair_actions() {
        let mut taken = false;
        let mut disabled = false;
        for state in component {
            let slot = *state as usize;
            let Some(row) = body.rows().get(slot) else {
                continue;
            };
            let targets = successors.get(slot).map_or(&[][..], Vec::as_slice);
            let mut enabled = false;
            for (entry, transition) in row.iter().enumerate() {
                if transition.action() != *action {
                    continue;
                }
                enabled = true;
                if let Some(target) = targets.get(entry)
                    && membership.get(*target as usize).copied().unwrap_or(false)
                {
                    taken = true;
                }
            }
            if !enabled {
                disabled = true;
            }
        }
        if !taken && !disabled {
            return false;
        }
    }
    true
}

// ---------------------------------------------------------------------------
// shared helpers
// ---------------------------------------------------------------------------

/// The first state that leaves the declared domain, as a rejection, or `None`.
///
/// A state vector shorter than the declared arity is unrepresentable: the decoder reads
/// exactly `arity` components per state. The `else` arm exists so that no future change
/// to the decoder can turn a shape mismatch into an index panic.
fn state_outside_domain(domain: &StateDomain, table: &StateTable) -> Option<Rejection> {
    for index in 0..table.len() {
        let state = table.state(index)?;
        for (position, variable) in domain.variables().iter().enumerate() {
            let Some(value) = state.get(position) else {
                return Some(Rejection::StateOutsideDomain {
                    state: index,
                    variable: narrow_u16(position),
                    value: 0,
                });
            };
            if !variable.admits(*value) {
                return Some(Rejection::StateOutsideDomain {
                    state: index,
                    variable: narrow_u16(position),
                    value: *value,
                });
            }
        }
    }
    None
}

/// Set a flag, ignoring an out-of-range index.
fn raise(flags: &mut [bool], index: usize) {
    if let Some(flag) = flags.get_mut(index) {
        *flag = true;
    }
}

/// Clear a flag, ignoring an out-of-range index.
fn lower(flags: &mut [bool], index: usize) {
    if let Some(flag) = flags.get_mut(index) {
        *flag = false;
    }
}

/// Write a value, ignoring an out-of-range index.
fn write(values: &mut [u32], index: usize, value: u32) {
    if let Some(slot) = values.get_mut(index) {
        *slot = value;
    }
}

/// Saturating `usize -> u32`, for diagnostic indices only.
const fn narrow_u32(value: usize) -> u32 {
    if value > u32::MAX as usize {
        u32::MAX
    } else {
        value as u32
    }
}

/// Saturating `usize -> u16`, for diagnostic indices only.
const fn narrow_u16(value: usize) -> u16 {
    if value > u16::MAX as usize {
        u16::MAX
    } else {
        value as u16
    }
}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::indexing_slicing,
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::arithmetic_side_effects,
        reason = "test bodies assert on known-shaped fixtures; the no-panic covenant \
                  is a property of the shipped checker, not of its test harness"
    )]

    use super::*;
    use crate::fixture::{Plan, Xorshift};
    use crate::verdict::{Field, TokenFault};
    use crate::wire::{MAGIC, MAX_TOKEN_BYTES, WIRE_EPOCH};

    fn verdict(plan: &Plan) -> Verdict {
        check_certificate(&plan.encode())
    }

    fn rejection(plan: &Plan) -> Rejection {
        match verdict(plan) {
            Verdict::Rejected(rejection) => rejection,
            other => panic!("expected a rejection, got {other:?}"),
        }
    }

    // --- green path: ranking ----------------------------------------------

    #[test]
    fn the_drain_ranking_certificate_is_verified() {
        let plan = Plan::drain_ranking();
        let Verdict::Verified(claim) = verdict(&plan) else {
            panic!("the drain ranking must check green");
        };
        assert_eq!(claim.kind(), CertificateKind::Ranking);
        assert_eq!(claim.property(), PropertyClass::EventuallyStateSet);
        assert_eq!(claim.states(), 9);
        assert_eq!(claim.goal_states(), 3);
        assert_eq!(claim.initial_states(), 1);
        assert_eq!(claim.transitions(), 12);
        assert_eq!(claim.fair_actions(), 0);
        assert_eq!(claim.progress_bound(), Some(4));
        assert_eq!(
            claim.reachable_states(),
            9,
            "a ranking obligation is discharged at every table state"
        );
        assert_eq!(
            claim.trusted_components(),
            [
                "certificate-model-correspondence",
                "envelope-digest-binding"
            ],
            "a ranking claim assumes no fairness, so it trusts no fairness declaration"
        );
    }

    // --- green path: fair-cycle exclusion, against the Revision 2 spike -----

    #[test]
    fn the_fair_progress_model_excludes_every_fair_cycle() {
        // `spikes/results/spike-results.json`, advanced.temporal_fair_lasso:
        //   "weak_fairness_rejects_wait_cycle": true
        // The Wait self-loop is the only non-goal cycle, and `Complete` is enabled
        // throughout it and never taken — so it is not a fair execution, and nothing
        // refutes eventually(done).
        let plan = Plan::fair_progress_exclusion();
        let Verdict::Verified(claim) = verdict(&plan) else {
            panic!("FairProgress must exclude every fair cycle");
        };
        assert_eq!(claim.kind(), CertificateKind::FairSccExclusion);
        assert_eq!(claim.states(), 2);
        assert_eq!(claim.reachable_states(), 2);
        assert_eq!(claim.goal_states(), 1);
        assert_eq!(claim.fair_actions(), 1);
        assert_eq!(claim.progress_bound(), None);
        assert_eq!(
            claim.trusted_components(),
            [
                "certificate-model-correspondence",
                "envelope-digest-binding",
                "fairness-assumption-correspondence",
            ],
            "an exclusion claim that used a fairness assumption must name it"
        );
    }

    #[test]
    fn the_broken_progress_model_has_a_fair_counterexample() {
        // `spikes/results/spike-results.json`, advanced.temporal_fair_lasso:
        //   "broken_model_has_fair_counterexample": true
        // `Complete` is declared but disabled everywhere, so the Wait self-loop
        // discharges weak fairness vacuously and is a genuine fair lasso.
        let plan = Plan::broken_progress_exclusion();
        assert_eq!(rejection(&plan), Rejection::FairCycleExists { state: 0 });
    }

    #[test]
    fn without_the_fairness_assumption_the_wait_cycle_refutes_progress() {
        // `spikes/results/spike-results.json`, advanced.temporal_fair_lasso:
        //   "unfair_wait_is_raw_liveness_counterexample": true
        // The same FairProgress model with no fairness declared: the Wait self-loop is
        // now a legal execution, and it never reaches `done`.
        let mut plan = Plan::fair_progress_exclusion();
        plan.fair_actions.clear();
        assert_eq!(rejection(&plan), Rejection::FairCycleExists { state: 0 });
    }

    #[test]
    fn a_fairness_free_ranking_cannot_prove_a_fairness_dependent_property() {
        // The two families are not stronger and weaker versions of one check. The
        // FairProgress model's progress genuinely depends on weak fairness, so no
        // ranking exists for it — the Wait self-loop cannot decrease anything.
        let mut plan = Plan::fair_progress_exclusion();
        plan.kind = CertificateKind::Ranking.code();
        plan.fair_actions.clear();
        plan.ranks = vec![1, 0];
        assert_eq!(
            rejection(&plan),
            Rejection::RankDoesNotDecrease {
                state: 0,
                entry: 1,
                from: 1,
                to: 1,
            }
        );
    }

    #[test]
    fn checking_is_deterministic_over_the_same_bytes() {
        for plan in [Plan::drain_ranking(), Plan::fair_progress_exclusion()] {
            let bytes = plan.encode();
            assert_eq!(check_certificate(&bytes), check_certificate(&bytes));
            assert!(check_certificate(&bytes).is_verified());
        }
    }

    // --- adversarial: framing ---------------------------------------------

    #[test]
    fn empty_input_is_a_truncation_not_a_verdict() {
        assert_eq!(
            check_certificate(&[]),
            Verdict::Rejected(Rejection::Truncated {
                field: Field::Magic,
                offset: 0,
                needed: 8,
                available: 0,
            })
        );
    }

    #[test]
    fn truncated_bytes_are_rejected_at_every_length() {
        for plan in [Plan::drain_ranking(), Plan::fair_progress_exclusion()] {
            let bytes = plan.encode();
            for cut in 0..bytes.len() {
                assert!(
                    !check_certificate(&bytes[..cut]).is_verified(),
                    "a {cut}-byte prefix was accepted as a certificate"
                );
            }
        }
    }

    #[test]
    fn a_sibling_kernels_magic_is_rejected() {
        let mut plan = Plan::drain_ranking();
        plan.magic = *b"CONTCERT";
        assert_eq!(rejection(&plan), Rejection::BadMagic);
    }

    #[test]
    fn unimplemented_contracts_are_unsupported_not_rejected() {
        let mut epoch = Plan::drain_ranking();
        epoch.wire_epoch = WIRE_EPOCH.saturating_add(1);
        epoch.schema_epoch = epoch.wire_epoch;
        assert_eq!(
            verdict(&epoch),
            Verdict::Unsupported(Feature::WireEpoch {
                found: WIRE_EPOCH + 1
            })
        );

        let mut family = Plan::drain_ranking();
        family.kind = 3;
        assert_eq!(
            verdict(&family),
            Verdict::Unsupported(Feature::CertificateKind { found: 3 })
        );

        let mut property = Plan::drain_ranking();
        property.property_class = 9;
        assert_eq!(
            verdict(&property),
            Verdict::Unsupported(Feature::PropertyClass { found: 9 })
        );

        // Strong fairness: named by RFC 0008 and RFC 0015, not implemented here.
        let mut fairness = Plan::fair_progress_exclusion();
        fairness.fairness_class = 2;
        assert_eq!(
            verdict(&fairness),
            Verdict::Unsupported(Feature::FairnessClass { found: 2 })
        );
    }

    #[test]
    fn a_ranking_that_declares_fair_actions_is_unsupported() {
        // docs/03 §6.5's "fairness-to-progress linkage" is not implemented, so a
        // certificate asking for it is declined rather than silently checked against
        // the stronger fairness-free condition it did not claim.
        let mut plan = Plan::drain_ranking();
        plan.fair_actions = vec![0];
        assert_eq!(
            verdict(&plan),
            Verdict::Unsupported(Feature::FairnessDischarge { declared: 1 })
        );
    }

    #[test]
    fn trailing_bytes_and_oversized_inputs_are_rejected() {
        let mut plan = Plan::drain_ranking();
        plan.trailing = vec![0x00];
        assert_eq!(rejection(&plan), Rejection::TrailingBytes { extra: 1 });

        let bytes = vec![0_u8; crate::wire::MAX_CERTIFICATE_BYTES + 1];
        assert_eq!(
            check_certificate(&bytes),
            Verdict::Rejected(Rejection::Oversized {
                found: crate::wire::MAX_CERTIFICATE_BYTES + 1
            })
        );
    }

    // --- adversarial: envelope and shape -----------------------------------

    #[test]
    fn an_envelope_that_names_a_different_schema_epoch_is_rejected() {
        let mut plan = Plan::drain_ranking();
        plan.schema_epoch = WIRE_EPOCH.saturating_add(4);
        assert_eq!(
            rejection(&plan),
            Rejection::SchemaEpochMismatch {
                declared: WIRE_EPOCH + 4,
                header: WIRE_EPOCH,
            }
        );
    }

    #[test]
    fn malformed_tokens_are_rejected_by_field() {
        let mut digest = Plan::drain_ranking();
        digest.model_digest = String::new();
        assert_eq!(
            rejection(&digest),
            Rejection::MalformedToken {
                field: Field::ModelDigest,
                fault: TokenFault::Empty,
            }
        );

        let mut action = Plan::drain_ranking();
        action.actions[0] = "dec a".to_owned();
        assert!(matches!(
            rejection(&action),
            Rejection::MalformedToken {
                field: Field::ActionName,
                ..
            }
        ));

        let mut long = Plan::drain_ranking();
        long.producer = "p".repeat(MAX_TOKEN_BYTES + 1);
        assert_eq!(
            rejection(&long),
            Rejection::MalformedToken {
                field: Field::Producer,
                fault: TokenFault::TooLong {
                    found: MAX_TOKEN_BYTES + 1
                },
            }
        );
    }

    #[test]
    fn unordered_sequences_are_rejected_as_ambiguous_encodings() {
        let mut variables = Plan::drain_ranking();
        variables.variables.reverse();
        assert_eq!(
            rejection(&variables),
            Rejection::NotStrictlyAscending {
                field: Field::VariableName,
                index: 1,
            }
        );

        let mut states = Plan::drain_ranking();
        states.states.swap(0, 1);
        assert_eq!(
            rejection(&states),
            Rejection::NotStrictlyAscending {
                field: Field::StateVector,
                index: 1,
            }
        );

        let mut goal = Plan::drain_ranking();
        goal.goal.swap(0, 1);
        assert_eq!(
            rejection(&goal),
            Rejection::NotStrictlyAscending {
                field: Field::GoalState,
                index: 1,
            }
        );

        let mut actions = Plan::drain_ranking();
        actions.actions.swap(0, 1);
        assert_eq!(
            rejection(&actions),
            Rejection::NotStrictlyAscending {
                field: Field::ActionName,
                index: 1,
            }
        );

        let mut fair = Plan::fair_progress_exclusion();
        fair.fair_actions = vec![2, 0];
        assert_eq!(
            rejection(&fair),
            Rejection::NotStrictlyAscending {
                field: Field::FairAction,
                index: 1,
            }
        );

        let mut row = Plan::drain_ranking();
        row.rows[4].reverse();
        assert_eq!(
            rejection(&row),
            Rejection::NotStrictlyAscending {
                field: Field::TransitionAction,
                index: 1,
            }
        );
    }

    #[test]
    fn an_inverted_variable_range_is_rejected() {
        let mut plan = Plan::drain_ranking();
        plan.variables[0].1 = 2;
        plan.variables[0].2 = 0;
        assert_eq!(
            rejection(&plan),
            Rejection::InvertedVariableRange { variable: 0 }
        );
    }

    #[test]
    fn counts_outside_the_declared_range_are_rejected() {
        let mut no_variables = Plan::drain_ranking();
        no_variables.variables.clear();
        assert_eq!(
            rejection(&no_variables),
            Rejection::CountOutOfRange {
                field: Field::VariableCount,
                found: 0,
                min: 1,
                max: u64::from(crate::wire::MAX_VARIABLES),
            }
        );

        let mut no_actions = Plan::drain_ranking();
        no_actions.actions.clear();
        no_actions.rows.iter_mut().for_each(Vec::clear);
        assert_eq!(
            rejection(&no_actions),
            Rejection::CountOutOfRange {
                field: Field::ActionCount,
                found: 0,
                min: 1,
                max: u64::from(crate::wire::MAX_ACTIONS),
            }
        );

        let mut huge = Plan::drain_ranking();
        huge.declared_state_count = Some(u32::MAX);
        assert_eq!(
            rejection(&huge),
            Rejection::CountOutOfRange {
                field: Field::StateCount,
                found: u64::from(u32::MAX),
                min: 1,
                max: u64::from(crate::wire::MAX_STATES),
            }
        );
    }

    #[test]
    fn a_certificate_with_no_initial_states_is_rejected() {
        let mut plan = Plan::drain_ranking();
        plan.initial.clear();
        assert_eq!(rejection(&plan), Rejection::NoInitialStates);
    }

    // --- adversarial: the shared obligations -------------------------------

    #[test]
    fn a_state_outside_the_declared_domain_is_rejected() {
        let mut plan = Plan::drain_ranking();
        plan.variables[0].2 = 1; // a <= 1, but the table reaches a = 2
        let Rejection::StateOutsideDomain {
            variable, value, ..
        } = rejection(&plan)
        else {
            panic!("shrinking the declared domain must violate the safety side condition");
        };
        assert_eq!(variable, 0);
        assert_eq!(value, 2);
    }

    #[test]
    fn an_initial_or_goal_state_outside_the_table_is_rejected() {
        let mut initial = Plan::drain_ranking();
        initial.initial = vec![vec![7, 7]];
        assert_eq!(
            rejection(&initial),
            Rejection::InitialStateNotInTable { position: 0 }
        );

        let mut goal = Plan::drain_ranking();
        goal.goal = vec![vec![7, 7]];
        assert_eq!(
            rejection(&goal),
            Rejection::GoalStateNotInTable { position: 0 }
        );
    }

    #[test]
    fn a_transition_leaving_the_table_breaks_closure() {
        let mut plan = Plan::drain_ranking();
        plan.rows[8][0].1 = vec![9, 9];
        plan.rows[8].sort();
        let entry = plan.rows[8]
            .iter()
            .position(|transition| transition.1 == vec![9, 9])
            .expect("the mutated transition is still present");
        assert_eq!(
            rejection(&plan),
            Rejection::ClosureFailure {
                state: 8,
                entry: u32::try_from(entry).expect("small index"),
            }
        );
    }

    #[test]
    fn a_transition_naming_an_undeclared_action_is_rejected() {
        let mut plan = Plan::drain_ranking();
        plan.rows[8][0].0 = 200;
        plan.rows[8].sort();
        let entry = plan.rows[8]
            .iter()
            .position(|transition| transition.0 == 200)
            .expect("the mutated transition is still present");
        assert_eq!(
            rejection(&plan),
            Rejection::UnknownAction {
                state: 8,
                entry: u32::try_from(entry).expect("small index"),
                action: 200,
            }
        );
    }

    #[test]
    fn a_fair_action_outside_the_action_table_is_rejected() {
        let mut plan = Plan::fair_progress_exclusion();
        plan.fair_actions = vec![9];
        assert_eq!(
            rejection(&plan),
            Rejection::FairActionOutOfRange {
                position: 0,
                action: 9,
                actions: 3,
            }
        );
    }

    // --- adversarial: the ranking obligation -------------------------------

    #[test]
    fn a_goal_state_with_a_non_zero_rank_is_rejected() {
        let mut plan = Plan::drain_ranking();
        plan.ranks[0] = 3;
        assert_eq!(
            rejection(&plan),
            Rejection::GoalRankNotZero { state: 0, rank: 3 }
        );
    }

    #[test]
    fn a_non_goal_state_with_no_successors_cannot_progress() {
        let mut plan = Plan::drain_ranking();
        plan.rows[3].clear(); // state [1,0] is not a goal
        assert_eq!(rejection(&plan), Rejection::ProgressDeadlock { state: 3 });
    }

    #[test]
    fn the_carried_ranking_is_minimal_so_no_rank_can_shrink() {
        // Each rank is the state's longest distance to the goal set, which is the
        // smallest ranking the decrease obligation admits. Lowering any of them puts
        // some successor's rank at or above its own; lowering a goal rank is
        // impossible, since they are already zero.
        let green = Plan::drain_ranking();
        for index in 0..green.ranks.len() {
            if green.ranks[index] == 0 {
                continue;
            }
            let mut plan = green.clone();
            plan.ranks[index] -= 1;
            assert!(
                !verdict(&plan).is_verified(),
                "rank {index} lowered by one was accepted"
            );
        }
    }

    #[test]
    fn raising_a_rank_is_rejected_wherever_a_predecessor_depends_on_it() {
        // The mirror direction. Every state except the initial one has a predecessor
        // whose own rank is exactly one greater, so raising it breaks that
        // predecessor's decrease obligation; a goal state's rank cannot be raised at
        // all.
        let green = Plan::drain_ranking();
        for index in 0..green.ranks.len() {
            if index == 8 {
                continue; // state [2,2]: the initial state, and nothing points at it
            }
            let mut plan = green.clone();
            plan.ranks[index] += 1;
            assert!(
                !verdict(&plan).is_verified(),
                "rank {index} raised by one was accepted"
            );
        }
    }

    #[test]
    fn raising_the_top_rank_only_loosens_the_bound_it_reports() {
        // Nothing points at the initial state, so its rank is free to be larger than
        // it needs to be. That is sound and it is not silently ignored: the claim's
        // progress bound is the number the certificate actually carried.
        let mut plan = Plan::drain_ranking();
        plan.ranks[8] = 40;
        let Verdict::Verified(claim) = verdict(&plan) else {
            panic!("a loose rank at a state with no predecessor is still a ranking");
        };
        assert_eq!(claim.progress_bound(), Some(40));
    }

    #[test]
    fn removing_any_transition_target_from_the_table_breaks_the_certificate() {
        let green = Plan::drain_ranking();
        for row in 0..green.rows.len() {
            for entry in 0..green.rows[row].len() {
                let mut plan = green.clone();
                plan.rows[row][entry].1 = vec![-1, -1];
                plan.rows[row].sort();
                assert!(
                    matches!(rejection(&plan), Rejection::ClosureFailure { state, .. }
                        if state == u32::try_from(row).expect("small index")),
                    "row {row} entry {entry} was not caught as a closure failure"
                );
            }
        }
    }

    // --- adversarial: the exclusion obligation -----------------------------

    #[test]
    fn a_certificate_that_omits_a_goal_state_is_refuted_by_its_own_graph() {
        // Dropping `done` from the goal set turns the whole reachable graph into
        // non-goal states, and the StayDone self-loop at `[0,1]` becomes a fair cycle:
        // `Complete` is disabled there, so weak fairness is discharged vacuously.
        let mut plan = Plan::fair_progress_exclusion();
        plan.goal.clear();
        assert_eq!(rejection(&plan), Rejection::FairCycleExists { state: 1 });
    }

    #[test]
    fn declaring_an_extra_action_fair_can_only_exclude_more_cycles() {
        // The direction that makes `fairness-assumption-correspondence` a trusted
        // component: declaring `Wait` fair as well leaves the verdict green, because
        // more fairness excludes more cycles. Overstating the model's fairness weakens
        // the claim without changing the verdict, which is why the producer owns
        // PO-LIV-001 and the claim names the assumption.
        let mut plan = Plan::fair_progress_exclusion();
        plan.fair_actions = vec![0, 2];
        let Verdict::Verified(claim) = verdict(&plan) else {
            panic!("a larger fairness assumption cannot introduce a fair cycle");
        };
        assert_eq!(claim.fair_actions(), 2);
    }

    #[test]
    fn a_multi_state_component_is_found_and_judged_as_one() {
        // The self-loop fixtures never put more than one state on the component
        // search's stack. Here the non-goal states `p = 0` and `p = 1` toggle into each
        // other, so the search has to recognise them as one component before it can
        // ask whether that component is fair.
        let plan = Plan::ping_pong_exclusion();
        let Verdict::Verified(claim) = verdict(&plan) else {
            panic!("`finish` is enabled throughout the toggle cycle and never taken");
        };
        assert_eq!(claim.states(), 3);
        assert_eq!(claim.reachable_states(), 3);

        // Drop the fairness assumption and the same cycle becomes a legal execution
        // that never finishes. The witness is the component's smallest index.
        let mut unfair = Plan::ping_pong_exclusion();
        unfair.fair_actions.clear();
        assert_eq!(rejection(&unfair), Rejection::FairCycleExists { state: 0 });
    }

    #[test]
    fn a_fair_action_taken_inside_a_component_discharges_its_own_obligation() {
        // Weak fairness for `toggle` is satisfied *by* the toggle cycle: the action is
        // taken infinitely often on it. So declaring only `toggle` fair excludes
        // nothing, and the cycle still refutes the certificate. This is the `taken`
        // branch of the fairness rule, which the self-loop fixtures never reach.
        let mut plan = Plan::ping_pong_exclusion();
        plan.fair_actions = vec![1];
        assert_eq!(rejection(&plan), Rejection::FairCycleExists { state: 0 });
    }

    #[test]
    fn an_unreachable_fair_cycle_does_not_refute_the_certificate() {
        // PO-LIV-004 speaks of a *reachable* SCC, and reachability is recomputed here
        // rather than carried, so a producer can neither hide a reachable cycle nor be
        // punished for an unreachable one.
        let plan = Plan::unreachable_trap_exclusion();
        let Verdict::Verified(claim) = verdict(&plan) else {
            panic!("an unreachable trap is not a counterexample");
        };
        assert_eq!(claim.states(), 4);
        assert_eq!(claim.reachable_states(), 2);
    }

    #[test]
    fn making_the_trap_reachable_refutes_the_certificate() {
        let mut plan = Plan::unreachable_trap_exclusion();
        plan.initial = vec![vec![0, 0], vec![1, 0]];
        assert!(matches!(
            rejection(&plan),
            Rejection::FairCycleExists { .. }
        ));
    }

    // --- adversarial: canonicity and garbage -------------------------------

    #[test]
    fn no_two_byte_strings_decode_to_the_same_certificate() {
        for plan in [Plan::drain_ranking(), Plan::fair_progress_exclusion()] {
            let green = plan.encode();
            let decoded = crate::wire::decode(&green).expect("the green certificate decodes");
            for position in 0..green.len() {
                for delta in [0x01_u8, 0x40, 0xff] {
                    let mut mutated = green.clone();
                    mutated[position] ^= delta;
                    if let Ok(other) = crate::wire::decode(&mutated) {
                        assert_ne!(
                            other, decoded,
                            "byte {position} xor {delta:#04x} decoded to the same certificate"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn garbage_bytes_never_panic() {
        let mut source = Xorshift::new(0x0f1e_2d3c_4b5a_6978);
        for length in 0..192_usize {
            for _ in 0..8 {
                let bytes = source.bytes(length);
                assert!(!check_certificate(&bytes).is_verified());
            }
        }

        for kind in [1_u16, 2] {
            for length in 0..192_usize {
                for _ in 0..8 {
                    let mut bytes = MAGIC.to_vec();
                    bytes.extend_from_slice(&WIRE_EPOCH.to_be_bytes());
                    bytes.extend_from_slice(&kind.to_be_bytes());
                    bytes.extend_from_slice(&source.bytes(length));
                    assert!(!check_certificate(&bytes).is_verified());
                }
            }
        }

        for plan in [Plan::drain_ranking(), Plan::fair_progress_exclusion()] {
            let green = plan.encode();
            assert!(
                green.len() < 8192,
                "the fixture stays small enough to sweep"
            );
            for _ in 0..512 {
                let mut bytes = green.clone();
                let at = source.next_u64() as usize % bytes.len();
                let len = source.next_u64() as usize % 24;
                let patch = source.bytes(len);
                for (offset, byte) in patch.iter().enumerate() {
                    if let Some(slot) = bytes.get_mut(at.saturating_add(offset)) {
                        *slot = *byte;
                    }
                }
                let _ = check_certificate(&bytes);
            }
        }
    }
}
