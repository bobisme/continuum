namespace Continuum

universe u

/-- A deliberately small mathematical transition-system kernel. -/
structure TransitionSystem (State : Type u) where
  init : State → Prop
  step : State → State → Prop

namespace TransitionSystem

variable {State : Type u} (sys : TransitionSystem State)

/-- Finite-path reachability, including the initial state. -/
inductive Reachable : State → Prop
  | init {s} : sys.init s → Reachable s
  | step {s t} : Reachable s → sys.step s t → Reachable t

/-- A state predicate is inductive when it contains initial states and is
closed under transitions. -/
structure InductiveInvariant (inv : State → Prop) : Prop where
  init : ∀ s, sys.init s → inv s
  step : ∀ s t, inv s → sys.step s t → inv t

 theorem reachable_satisfies
    {inv : State → Prop}
    (h : sys.InductiveInvariant inv) :
    ∀ {s}, sys.Reachable s → inv s := by
  intro s hs
  induction hs with
  | init hi => exact h.init _ hi
  | step _ hst ih => exact h.step _ _ ih hst

end TransitionSystem
end Continuum
