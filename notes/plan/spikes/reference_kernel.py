"""Small executable reference kernel for Continuum architectural spikes.

This is intentionally simple, exact, deterministic, and independent of any
future optimized Rust engine. It is not production code. Its purpose is to
make semantic claims falsifiable while the architecture is still fluid.
"""
from __future__ import annotations

from collections import deque
from dataclasses import dataclass
from typing import Callable, Generic, Hashable, Iterable, Mapping, Protocol, TypeVar

S = TypeVar("S", bound=Hashable)


@dataclass(frozen=True, slots=True)
class Step(Generic[S]):
    action: str
    target: S


class FiniteModel(Protocol[S]):
    name: str

    def initial_states(self) -> Iterable[S]: ...

    def successors(self, state: S) -> Iterable[Step[S]]: ...


@dataclass(frozen=True, slots=True)
class TraceStep(Generic[S]):
    source: S
    action: str
    target: S


@dataclass(slots=True)
class ExplorationResult(Generic[S]):
    model: str
    states: tuple[S, ...]
    edges: tuple[TraceStep[S], ...]
    depth: Mapping[S, int]
    predecessor: Mapping[S, tuple[S, str] | None]

    def trace_to(self, state: S) -> tuple[TraceStep[S], ...]:
        if state not in self.predecessor:
            raise KeyError(f"state not reached: {state!r}")
        rev: list[TraceStep[S]] = []
        cur = state
        while (pred := self.predecessor[cur]) is not None:
            src, action = pred
            rev.append(TraceStep(src, action, cur))
            cur = src
        rev.reverse()
        return tuple(rev)

    def first_state(self, predicate: Callable[[S], bool]) -> S | None:
        candidates = (s for s in self.states if predicate(s))
        return min(candidates, key=lambda s: self.depth[s], default=None)


@dataclass(frozen=True, slots=True)
class ClosureCertificate(Generic[S]):
    model: str
    states: tuple[S, ...]
    initial_states: tuple[S, ...]
    edges: tuple[TraceStep[S], ...]


@dataclass(frozen=True, slots=True)
class CertificateVerdict:
    valid: bool
    errors: tuple[str, ...]


def explore(model: FiniteModel[S], *, max_states: int | None = None) -> ExplorationResult[S]:
    """Deterministic exact BFS over a finite model."""
    initials = tuple(sorted(set(model.initial_states()), key=repr))
    if not initials:
        raise ValueError("model has no initial states")

    queue: deque[S] = deque(initials)
    seen: set[S] = set(initials)
    ordered: list[S] = list(initials)
    depth: dict[S, int] = {s: 0 for s in initials}
    predecessor: dict[S, tuple[S, str] | None] = {s: None for s in initials}
    edges: list[TraceStep[S]] = []

    while queue:
        source = queue.popleft()
        steps = sorted(model.successors(source), key=lambda st: (st.action, repr(st.target)))
        for step in steps:
            edge = TraceStep(source, step.action, step.target)
            edges.append(edge)
            if step.target in seen:
                continue
            seen.add(step.target)
            ordered.append(step.target)
            depth[step.target] = depth[source] + 1
            predecessor[step.target] = (source, step.action)
            queue.append(step.target)
            if max_states is not None and len(seen) > max_states:
                raise RuntimeError(f"state budget exceeded: {max_states}")

    return ExplorationResult(
        model=model.name,
        states=tuple(ordered),
        edges=tuple(edges),
        depth=depth,
        predecessor=predecessor,
    )


def make_closure_certificate(model: FiniteModel[S], result: ExplorationResult[S]) -> ClosureCertificate[S]:
    return ClosureCertificate(
        model=model.name,
        states=tuple(sorted(result.states, key=repr)),
        initial_states=tuple(sorted(set(model.initial_states()), key=repr)),
        edges=tuple(sorted(result.edges, key=lambda e: (repr(e.source), e.action, repr(e.target)))),
    )


def check_closure_certificate(
    model: FiniteModel[S],
    certificate: ClosureCertificate[S],
    *,
    invariant: Callable[[S], bool] | None = None,
) -> CertificateVerdict:
    """Check finite reachability closure without trusting the explorer.

    For a safety proof, the certificate must contain every initial state, be
    closed under the model's successor relation, and satisfy the invariant.
    Extra unreachable states are permitted but make the certificate larger.
    """
    errors: list[str] = []
    states = set(certificate.states)
    initials = set(model.initial_states())

    if certificate.model != model.name:
        errors.append(f"model mismatch: {certificate.model!r} != {model.name!r}")
    if not initials.issubset(states):
        errors.append(f"missing initial states: {sorted(initials - states, key=repr)!r}")

    listed_edges = {(e.source, e.action, e.target) for e in certificate.edges}
    for state in sorted(states, key=repr):
        if invariant is not None and not invariant(state):
            errors.append(f"invariant false at {state!r}")
        for step in model.successors(state):
            if step.target not in states:
                errors.append(
                    f"closure failure: {state!r} --{step.action}--> {step.target!r} not in certificate"
                )
            if (state, step.action, step.target) not in listed_edges:
                errors.append(
                    f"missing edge witness: {state!r} --{step.action}--> {step.target!r}"
                )

    return CertificateVerdict(not errors, tuple(errors))
