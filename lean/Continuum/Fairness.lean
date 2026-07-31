import Continuum.Temporal

namespace Continuum

universe u v

abbrev ActionBehavior (State : Type u) (Label : Type v) := Nat → State × Label

def StateAt {State : Type u} {Label : Type v}
    (b : ActionBehavior State Label) (n : Nat) : State := (b n).1

def LabelAt {State : Type u} {Label : Type v}
    (b : ActionBehavior State Label) (n : Nat) : Label := (b n).2

/-- Weak fairness: if an action is continuously enabled from some point, then
it occurs at or after every point sufficiently far in the behavior. -/
def WeakFair
    {State : Type u} {Label : Type v}
    (enabled : Label → State → Prop)
    (action : Label)
    (b : ActionBehavior State Label) : Prop :=
  (∃ n, ∀ m, n ≤ m → enabled action (StateAt b m)) →
  ∀ n, ∃ m, n ≤ m ∧ LabelAt b m = action

/-- A behavior in which the action recurs infinitely often is weakly fair,
independently of its enabledness premise. -/
theorem infinitely_often_action_implies_weakFair
    {State : Type u} {Label : Type v}
    (enabled : Label → State → Prop)
    (action : Label)
    (b : ActionBehavior State Label)
    (h : ∀ n, ∃ m, n ≤ m ∧ LabelAt b m = action) :
    WeakFair enabled action b := by
  intro _ n
  exact h n

end Continuum
