import Continuum.Semantics

namespace Continuum

universe u

/-- A transition-system automorphism packaged without depending on Mathlib's
algebra hierarchy. -/
structure Automorphism {State : Type u} (sys : TransitionSystem State) where
  map : State → State
  inv : State → State
  leftInv : ∀ s, inv (map s) = s
  rightInv : ∀ s, map (inv s) = s
  initIff : ∀ s, sys.init (map s) ↔ sys.init s
  stepIff : ∀ s t, sys.step (map s) (map t) ↔ sys.step s t

namespace Automorphism

variable {State : Type u} {sys : TransitionSystem State}

/-- Automorphisms preserve finite-path reachability. -/
theorem maps_reachable (a : Automorphism sys) :
    ∀ {s}, sys.Reachable s → sys.Reachable (a.map s) := by
  intro s hs
  induction hs with
  | init hi =>
      exact .init ((a.initIff _).2 hi)
  | step _ hstep ih =>
      exact .step ih ((a.stepIff _ _).2 hstep)

end Automorphism
end Continuum
