"""Finite models used by the Continuum revision-2 spikes."""
from __future__ import annotations

from dataclasses import dataclass
from enum import IntEnum
from itertools import product
from typing import Iterable

from reference_kernel import Step


@dataclass(frozen=True, order=True, slots=True)
class DieHardState:
    big: int
    small: int


class DieHard:
    """Semantic equivalent of tlaplus/Examples DieHard.tla."""

    name = "TLAExamples/DieHard"

    def initial_states(self) -> Iterable[DieHardState]:
        yield DieHardState(0, 0)

    def successors(self, s: DieHardState) -> Iterable[Step[DieHardState]]:
        def emit(action: str, big: int, small: int) -> Step[DieHardState]:
            assert 0 <= big <= 5
            assert 0 <= small <= 3
            return Step(action, DieHardState(big, small))

        yield emit("FillSmallJug", s.big, 3)
        yield emit("FillBigJug", 5, s.small)
        yield emit("EmptySmallJug", s.big, 0)
        yield emit("EmptyBigJug", 0, s.small)

        new_big = min(s.big + s.small, 5)
        yield emit("SmallToBig", new_big, s.small - (new_big - s.big))

        new_small = min(s.big + s.small, 3)
        yield emit("BigToSmall", s.big - (new_small - s.small), new_small)

    @staticmethod
    def type_ok(s: DieHardState) -> bool:
        return 0 <= s.big <= 5 and 0 <= s.small <= 3

    @staticmethod
    def not_solved(s: DieHardState) -> bool:
        return s.big != 4


class Phase(IntEnum):
    THINKING = 0
    WANTS = 1
    HAS_LEFT = 2
    EATING = 3


@dataclass(frozen=True, order=True, slots=True)
class DiningState:
    phase: tuple[Phase, ...]
    fork_owner: tuple[int, ...]  # -1 means free


class DiningPhilosophers:
    """A deliberately deadlock-prone left-then-right acquisition protocol."""

    def __init__(self, n: int = 5):
        if n < 2:
            raise ValueError("need at least two philosophers")
        self.n = n
        self.name = f"DiningPhilosophers({n})"

    def initial_states(self) -> Iterable[DiningState]:
        yield DiningState((Phase.THINKING,) * self.n, (-1,) * self.n)

    def successors(self, s: DiningState) -> Iterable[Step[DiningState]]:
        for i in range(self.n):
            left = i
            right = (i + 1) % self.n
            phase = list(s.phase)
            owners = list(s.fork_owner)

            if phase[i] == Phase.THINKING:
                phase[i] = Phase.WANTS
                yield Step(f"BecomeHungry({i})", DiningState(tuple(phase), s.fork_owner))
                phase[i] = Phase.THINKING

            if phase[i] == Phase.WANTS and owners[left] == -1:
                owners[left] = i
                phase[i] = Phase.HAS_LEFT
                yield Step(f"TakeLeft({i})", DiningState(tuple(phase), tuple(owners)))
                owners[left] = -1
                phase[i] = Phase.WANTS

            if phase[i] == Phase.HAS_LEFT and owners[right] == -1:
                owners[right] = i
                phase[i] = Phase.EATING
                yield Step(f"TakeRight({i})", DiningState(tuple(phase), tuple(owners)))
                owners[right] = -1
                phase[i] = Phase.HAS_LEFT

            if phase[i] == Phase.EATING:
                assert owners[left] == i and owners[right] == i
                owners[left] = -1
                owners[right] = -1
                phase[i] = Phase.THINKING
                yield Step(f"Release({i})", DiningState(tuple(phase), tuple(owners)))

    def is_deadlock(self, s: DiningState) -> bool:
        return not any(self.successors(s)) and any(p != Phase.THINKING for p in s.phase)

    def type_ok(self, s: DiningState) -> bool:
        if len(s.phase) != self.n or len(s.fork_owner) != self.n:
            return False
        for fork, owner in enumerate(s.fork_owner):
            if owner == -1:
                continue
            if not 0 <= owner < self.n:
                return False
            if s.phase[owner] not in (Phase.HAS_LEFT, Phase.EATING):
                return False
            left, right = owner, (owner + 1) % self.n
            if fork not in (left, right):
                return False
        return True


@dataclass(frozen=True, order=True, slots=True)
class AbstractRegister:
    value: int


@dataclass(frozen=True, order=True, slots=True)
class ConcreteRegister:
    value: int
    pending: int | None


class AbstractAtomicRegister:
    name = "AbstractAtomicRegister"

    def initial_states(self) -> Iterable[AbstractRegister]:
        yield AbstractRegister(0)

    def successors(self, s: AbstractRegister) -> Iterable[Step[AbstractRegister]]:
        for v in (0, 1, 2):
            yield Step(f"Write({v})", AbstractRegister(v))


class ConcreteTwoPhaseRegister:
    name = "ConcreteTwoPhaseRegister"

    def initial_states(self) -> Iterable[ConcreteRegister]:
        yield ConcreteRegister(0, None)

    def successors(self, s: ConcreteRegister) -> Iterable[Step[ConcreteRegister]]:
        if s.pending is None:
            for v in (0, 1, 2):
                yield Step(f"Reserve({v})", ConcreteRegister(s.value, v))
        else:
            yield Step("Commit", ConcreteRegister(s.pending, None))
            yield Step("Abort", ConcreteRegister(s.value, None))

    @staticmethod
    def abstraction(s: ConcreteRegister) -> AbstractRegister:
        return AbstractRegister(s.value)


def check_stuttering_refinement() -> tuple[bool, list[str]]:
    abstract = AbstractAtomicRegister()
    concrete = ConcreteTwoPhaseRegister()
    errors: list[str] = []

    abs_steps = {
        (src, dst)
        for src in (AbstractRegister(0), AbstractRegister(1), AbstractRegister(2))
        for step in abstract.successors(src)
        for dst in (step.target,)
    }

    concrete_states = [ConcreteRegister(v, p) for v, p in product((0, 1, 2), (None, 0, 1, 2))]
    for src in concrete_states:
        for step in concrete.successors(src):
            a0 = concrete.abstraction(src)
            a1 = concrete.abstraction(step.target)
            if a0 == a1:
                continue
            if (a0, a1) not in abs_steps:
                errors.append(f"non-refining step: {src!r} --{step.action}--> {step.target!r}")
    return not errors, errors
