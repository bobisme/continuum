"""Executable sketch of observer-indexed independence.

The spike is not a proof. It checks the design's required monotonicity and
commutation conditions over a finite example, which will later be formalized
in Lean.
"""
from __future__ import annotations

from dataclasses import dataclass
from itertools import permutations
from typing import Callable, FrozenSet


@dataclass(frozen=True, slots=True)
class Store:
    x: int = 0
    y: int = 0
    audit: tuple[str, ...] = ()


@dataclass(frozen=True, slots=True)
class Event:
    name: str
    writes: FrozenSet[str]
    apply: Callable[[Store], Store]


def run(state: Store, order: tuple[Event, ...]) -> Store:
    for event in order:
        state = event.apply(state)
    return state


def commute_under(
    state: Store,
    a: Event,
    b: Event,
    observer: Callable[[Store], object],
) -> bool:
    return observer(run(state, (a, b))) == observer(run(state, (b, a)))


def experiment() -> dict[str, object]:
    write_x = Event(
        "write_x",
        frozenset({"x"}),
        lambda s: Store(x=1, y=s.y, audit=s.audit + ("x",)),
    )
    write_y = Event(
        "write_y",
        frozenset({"y"}),
        lambda s: Store(x=s.x, y=1, audit=s.audit + ("y",)),
    )

    observers = {
        "state_xy": lambda s: (s.x, s.y),
        "state_x": lambda s: s.x,
        "audit_order": lambda s: s.audit,
    }
    results = {
        name: commute_under(Store(), write_x, write_y, observer)
        for name, observer in observers.items()
    }

    # Observer refinement order: audit_order is more discriminating than
    # state_xy for this event pair; state_xy is more discriminating than x.
    monotone = (
        (not results["audit_order"] or results["state_xy"])
        and (not results["state_xy"] or results["state_x"])
    )

    return {
        "events": [write_x.name, write_y.name],
        "commutes_under": results,
        "observer_monotonicity_holds": monotone,
        "interpretation": (
            "The same events are independent for state observers but dependent "
            "for an order-sensitive audit observer. Reduction must therefore be "
            "indexed by the active observation/property contract."
        ),
    }
