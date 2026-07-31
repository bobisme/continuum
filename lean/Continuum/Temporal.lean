import Continuum.Semantics

namespace Continuum

universe u

abbrev Behavior (State : Type u) := Nat → State

def Always {State : Type u} (p : State → Prop) (b : Behavior State) : Prop :=
  ∀ n, p (b n)

def Eventually {State : Type u} (p : State → Prop) (b : Behavior State) : Prop :=
  ∃ n, p (b n)

def InfinitelyOften {State : Type u} (p : State → Prop) (b : Behavior State) : Prop :=
  ∀ n, ∃ m, n ≤ m ∧ p (b m)

def ContinuouslyEnabled {State : Type u}
    (enabled : State → Prop) (b : Behavior State) : Prop :=
  ∃ n, ∀ m, n ≤ m → enabled (b m)

/-- Extensional stuttering at a particular step. -/
def StuttersAt {State : Type u} (b : Behavior State) (n : Nat) : Prop :=
  b n = b (n + 1)

/-! The first Lean milestone formalizes safety and stuttering here. Fairness,
Büchi/Streett acceptance, and lasso certificates are added under RFC 0015. -/
end Continuum
