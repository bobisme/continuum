"""Additional executable spikes for Continuum revision 2.

These experiments are intentionally tiny. Their job is to falsify semantic
ideas before they become architecture: symmetry quotienting, fair-lasso
checking, environment-assumption synthesis, nominal canonicalization, and
semiring-valued graph analysis.
"""
from __future__ import annotations

from collections import defaultdict, deque
from dataclasses import dataclass
from math import inf
from typing import Callable, Hashable, Iterable, TypeVar

from models import DieHard, DiningPhilosophers, DiningState
from reference_kernel import Step, explore

S = TypeVar("S", bound=Hashable)


# ---------------------------------------------------------------------------
# Symmetry: cyclic group action on dining philosophers
# ---------------------------------------------------------------------------

def rotate_dining(state: DiningState, k: int) -> DiningState:
    n = len(state.phase)
    k %= n
    phases = [None] * n
    owners = [-1] * n
    for old_p, phase in enumerate(state.phase):
        phases[(old_p + k) % n] = phase
    for old_fork, owner in enumerate(state.fork_owner):
        new_fork = (old_fork + k) % n
        owners[new_fork] = -1 if owner == -1 else (owner + k) % n
    return DiningState(tuple(phases), tuple(owners))


def canonical_dining(state: DiningState) -> DiningState:
    return min((rotate_dining(state, k) for k in range(len(state.phase))), key=repr)


def symmetry_experiment(n: int = 5) -> dict:
    model = DiningPhilosophers(n)
    result = explore(model)
    quotient = {canonical_dining(s) for s in result.states}

    # Exhaustively validate that each rotation is an automorphism of the
    # labeled transition relation modulo rotated philosopher identifiers.
    errors: list[str] = []
    for state in result.states:
        succ_targets = {step.target for step in model.successors(state)}
        for k in range(n):
            rotated_state = rotate_dining(state, k)
            rotated_targets = {step.target for step in model.successors(rotated_state)}
            expected_targets = {rotate_dining(t, k) for t in succ_targets}
            if rotated_targets != expected_targets:
                errors.append(f"rotation {k} does not preserve successors for {state!r}")
                break

    orbit_sizes = defaultdict(int)
    for state in result.states:
        orbit_sizes[canonical_dining(state)] += 1

    return {
        "group": f"C{n}",
        "raw_states": len(result.states),
        "quotient_states": len(quotient),
        "reduction_factor": len(result.states) / len(quotient),
        "automorphism_check_valid": not errors,
        "automorphism_errors": errors[:10],
        "orbit_size_histogram": {
            str(size): sum(1 for count in orbit_sizes.values() if count == size)
            for size in sorted(set(orbit_sizes.values()))
        },
    }


# ---------------------------------------------------------------------------
# Temporal semantics: fair-lasso certificates
# ---------------------------------------------------------------------------

@dataclass(frozen=True, order=True, slots=True)
class ProgressState:
    done: bool
    broken: bool = False


class ProgressModel:
    def __init__(self, broken: bool = False):
        self.broken = broken
        self.name = "BrokenProgress" if broken else "FairProgress"

    def initial_states(self) -> Iterable[ProgressState]:
        yield ProgressState(False, self.broken)

    def successors(self, s: ProgressState) -> Iterable[Step[ProgressState]]:
        if s.done:
            yield Step("StayDone", s)
            return
        yield Step("Wait", s)
        if not s.broken:
            yield Step("Complete", ProgressState(True, self.broken))


@dataclass(frozen=True, slots=True)
class Lasso:
    prefix_states: tuple[ProgressState, ...]
    prefix_actions: tuple[str, ...]
    cycle_states: tuple[ProgressState, ...]
    cycle_actions: tuple[str, ...]


def _enabled_actions(model: ProgressModel, state: ProgressState) -> set[str]:
    return {step.action for step in model.successors(state)}


def check_eventually_done_lasso(
    model: ProgressModel,
    lasso: Lasso,
    *,
    weak_fair_actions: frozenset[str] = frozenset(),
) -> tuple[bool, str]:
    """Check a lasso witness for violation of <>done under weak fairness.

    A weakly fair action may not be continuously enabled throughout the cycle
    while never occurring on the cycle.
    """
    if not lasso.cycle_states or len(lasso.cycle_states) != len(lasso.cycle_actions):
        return False, "malformed cycle"
    if len(lasso.prefix_actions) + 1 != len(lasso.prefix_states):
        return False, "malformed prefix"

    def has_edge(src: ProgressState, action: str, dst: ProgressState) -> bool:
        return any(step.action == action and step.target == dst for step in model.successors(src))

    for src, action, dst in zip(
        lasso.prefix_states, lasso.prefix_actions, lasso.prefix_states[1:]
    ):
        if not has_edge(src, action, dst):
            return False, f"missing prefix edge {src!r} --{action}--> {dst!r}"

    entry = lasso.prefix_states[-1]
    if entry != lasso.cycle_states[0]:
        return False, "prefix does not enter cycle"

    n = len(lasso.cycle_states)
    for i, (src, action) in enumerate(zip(lasso.cycle_states, lasso.cycle_actions)):
        dst = lasso.cycle_states[(i + 1) % n]
        if not has_edge(src, action, dst):
            return False, f"missing cycle edge {src!r} --{action}--> {dst!r}"

    if any(s.done for s in (*lasso.prefix_states, *lasso.cycle_states)):
        return False, "lasso does not violate eventually(done)"

    cycle_action_set = set(lasso.cycle_actions)
    for fair_action in weak_fair_actions:
        continuously_enabled = all(
            fair_action in _enabled_actions(model, state) for state in lasso.cycle_states
        )
        if continuously_enabled and fair_action not in cycle_action_set:
            return False, f"unfair cycle: {fair_action} continuously enabled but never taken"

    return True, "valid fair counterexample"


def temporal_experiment() -> dict:
    good = ProgressModel(False)
    unfair_wait = Lasso(
        prefix_states=(ProgressState(False, False),),
        prefix_actions=(),
        cycle_states=(ProgressState(False, False),),
        cycle_actions=("Wait",),
    )
    raw_ok, raw_msg = check_eventually_done_lasso(good, unfair_wait)
    fair_ok, fair_msg = check_eventually_done_lasso(
        good, unfair_wait, weak_fair_actions=frozenset({"Complete"})
    )

    broken = ProgressModel(True)
    fair_failure = Lasso(
        prefix_states=(ProgressState(False, True),),
        prefix_actions=(),
        cycle_states=(ProgressState(False, True),),
        cycle_actions=("Wait",),
    )
    broken_ok, broken_msg = check_eventually_done_lasso(
        broken, fair_failure, weak_fair_actions=frozenset({"Complete"})
    )

    return {
        "unfair_wait_is_raw_liveness_counterexample": raw_ok,
        "raw_verdict": raw_msg,
        "weak_fairness_rejects_wait_cycle": not fair_ok,
        "fairness_verdict": fair_msg,
        "broken_model_has_fair_counterexample": broken_ok,
        "broken_verdict": broken_msg,
    }


# ---------------------------------------------------------------------------
# Safety games: synthesize an environment contract
# ---------------------------------------------------------------------------

@dataclass(frozen=True, slots=True)
class GameEdge:
    source: str
    actor: str  # "system" or "environment"
    action: str
    target: str


def assumption_game_experiment() -> dict:
    """Synthesize a minimum environment restriction for a safety objective.

    The finite spike enumerates subsets of environment moves. Production code
    will use game algorithms and symbolic generalization, but this exact search
    makes the intended contract unambiguous.
    """
    from itertools import combinations

    states = {"Start", "Persisting", "Retry", "Durable", "Acked", "Unsafe"}
    unsafe = {"Unsafe"}
    owner = {
        "Start": "system",
        "Persisting": "environment",
        "Retry": "system",
        "Durable": "system",
        "Acked": "system",
        "Unsafe": "system",
    }
    edges = [
        GameEdge("Start", "system", "BeginWrite", "Persisting"),
        GameEdge("Persisting", "environment", "SyncCompletes", "Durable"),
        GameEdge("Persisting", "environment", "CrashBeforeAck", "Retry"),
        GameEdge("Persisting", "environment", "AckBeforeSync", "Unsafe"),
        GameEdge("Retry", "system", "RetryWrite", "Persisting"),
        GameEdge("Durable", "system", "PublishAck", "Acked"),
        GameEdge("Acked", "system", "Stay", "Acked"),
        GameEdge("Unsafe", "system", "StayUnsafe", "Unsafe"),
    ]
    environment_edges = [edge for edge in edges if edge.actor == "environment"]

    def winning_region(removed: frozenset[GameEdge]) -> set[str]:
        outgoing: dict[str, list[GameEdge]] = defaultdict(list)
        for edge in edges:
            if edge not in removed:
                outgoing[edge.source].append(edge)
        winning = states - unsafe
        changed = True
        while changed:
            changed = False
            for state in tuple(winning):
                moves = outgoing[state]
                if not moves:
                    continue
                keep = (
                    any(edge.target in winning for edge in moves)
                    if owner[state] == "system"
                    else all(edge.target in winning for edge in moves)
                )
                if not keep:
                    winning.remove(state)
                    changed = True
        return winning

    unrestricted = winning_region(frozenset())
    candidates: list[tuple[tuple[GameEdge, ...], set[str]]] = []
    for size in range(len(environment_edges) + 1):
        for removed_tuple in combinations(environment_edges, size):
            region = winning_region(frozenset(removed_tuple))
            if "Start" in region:
                candidates.append((removed_tuple, region))
        if candidates:
            break

    if not candidates:
        raise AssertionError("no environment contract makes Start winning")

    # Prefer the contract that removes the fewest moves; ties are deterministic.
    removed, contracted_region = min(
        candidates,
        key=lambda item: tuple((e.source, e.action, e.target) for e in item[0]),
    )

    return {
        "unrestricted_start_winning": "Start" in unrestricted,
        "winning_region_unrestricted": sorted(unrestricted),
        "minimum_removed_move_count": len(removed),
        "synthesized_forbidden_environment_moves": [
            {"source": e.source, "action": e.action, "target": e.target}
            for e in removed
        ],
        "start_winning_under_synthesized_contract": "Start" in contracted_region,
        "winning_region_under_contract": sorted(contracted_region),
    }


# ---------------------------------------------------------------------------
# Nominal canonicalization: alpha-equivalence of fresh names
# ---------------------------------------------------------------------------

def canonicalize_nominal_trace(trace: list[tuple]) -> tuple[tuple, ...]:
    mapping: dict[object, int] = {}
    next_id = 0

    def atom(x: object) -> object:
        nonlocal next_id
        if isinstance(x, tuple):
            return tuple(atom(v) for v in x)
        if isinstance(x, list):
            return tuple(atom(v) for v in x)
        # Event names are kept as strings in position zero; all other string or
        # integer payloads are treated as atoms for this spike.
        if x not in mapping:
            mapping[x] = next_id
            next_id += 1
        return mapping[x]

    out: list[tuple] = []
    for event in trace:
        if not event:
            raise ValueError("empty event")
        out.append((event[0], *(atom(v) for v in event[1:])))
    return tuple(out)


def nominal_experiment() -> dict:
    a = [
        ("Alloc", 42),
        ("Send", 42),
        ("Alloc", 7),
        ("Link", 42, 7),
    ]
    b = [
        ("Alloc", "request-a"),
        ("Send", "request-a"),
        ("Alloc", "request-b"),
        ("Link", "request-a", "request-b"),
    ]
    c = [
        ("Alloc", "request-a"),
        ("Alloc", "request-b"),
        ("Send", "request-a"),
        ("Link", "request-a", "request-b"),
    ]
    ca, cb, cc = map(canonicalize_nominal_trace, (a, b, c))
    return {
        "renaming_equivalent": ca == cb,
        "causally_distinct_trace_not_collapsed": ca != cc,
        "canonical_trace": [list(event) for event in ca],
    }


# ---------------------------------------------------------------------------
# Semiring-valued traversal: shortest distance + number of shortest witnesses
# ---------------------------------------------------------------------------

def shortest_distance_and_count(
    initial: S,
    successors: Callable[[S], Iterable[Step[S]]],
    goal: Callable[[S], bool],
) -> tuple[int | None, int]:
    distance: dict[S, int] = {initial: 0}
    count: dict[S, int] = {initial: 1}
    queue: deque[S] = deque([initial])
    while queue:
        source = queue.popleft()
        d = distance[source]
        for step in successors(source):
            target = step.target
            nd = d + 1
            if target not in distance:
                distance[target] = nd
                count[target] = count[source]
                queue.append(target)
            elif distance[target] == nd:
                count[target] += count[source]
    goals = [s for s in distance if goal(s)]
    if not goals:
        return None, 0
    best = min(distance[s] for s in goals)
    return best, sum(count[s] for s in goals if distance[s] == best)


def semiring_experiment() -> dict:
    model = DieHard()
    initial = next(iter(model.initial_states()))
    distance, count = shortest_distance_and_count(
        initial, model.successors, lambda s: not model.not_solved(s)
    )
    return {
        "analysis": "tropical distance paired with natural-number multiplicity",
        "shortest_solution_distance": distance,
        "number_of_shortest_labeled_witnesses": count,
        "interpretation": "one graph traversal computes both optimization and witness multiplicity",
    }


def experiment() -> dict:
    return {
        "cyclic_symmetry": symmetry_experiment(5),
        "temporal_fair_lasso": temporal_experiment(),
        "assumption_safety_game": assumption_game_experiment(),
        "nominal_canonicalization": nominal_experiment(),
        "semiring_valued_search": semiring_experiment(),
    }
