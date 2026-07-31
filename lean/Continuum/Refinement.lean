import Continuum.Semantics

namespace Continuum

universe u v

structure StutteringSimulation
    {Concrete : Type u} {Abstract : Type v}
    (concrete : TransitionSystem Concrete)
    (abstract : TransitionSystem Abstract)
    (view : Concrete → Abstract) : Prop where
  init : ∀ c, concrete.init c → abstract.init (view c)
  step : ∀ c c', concrete.step c c' →
    view c = view c' ∨ abstract.step (view c) (view c')

namespace StutteringSimulation

variable {Concrete : Type u} {Abstract : Type v}
variable {concrete : TransitionSystem Concrete}
variable {abstract : TransitionSystem Abstract}
variable {view : Concrete → Abstract}

/-- Reachable concrete states map to reachable abstract states when concrete
steps either stutter or perform one abstract step. -/
theorem maps_reachable
    (sim : StutteringSimulation concrete abstract view) :
    ∀ {c}, concrete.Reachable c → abstract.Reachable (view c) := by
  intro c hc
  induction hc with
  | init hi => exact .init (sim.init _ hi)
  | step _ hstep ih =>
      rcases sim.step _ _ hstep with hstutter | habs
      · simpa [hstutter] using ih
      · exact .step ih habs

end StutteringSimulation
end Continuum

namespace Continuum.StutteringSimulation

universe u v w

variable {Concrete : Type u} {Middle : Type v} {Abstract : Type w}
variable {concrete : TransitionSystem Concrete}
variable {middle : TransitionSystem Middle}
variable {abstract : TransitionSystem Abstract}
variable {toMiddle : Concrete → Middle}
variable {toAbstract : Middle → Abstract}

/-- Stuttering simulations compose. This theorem is the seed of Continuum's
refinement-graph receipt composition rule. -/
theorem compose
    (lower : StutteringSimulation concrete middle toMiddle)
    (upper : StutteringSimulation middle abstract toAbstract) :
    StutteringSimulation concrete abstract (fun c => toAbstract (toMiddle c)) := by
  constructor
  · intro c hc
    exact upper.init _ (lower.init _ hc)
  · intro c c' hstep
    rcases lower.step _ _ hstep with hlower | hmiddle
    · left
      exact congrArg toAbstract hlower
    · rcases upper.step _ _ hmiddle with hupper | habstract
      · left
        exact hupper
      · right
        exact habstract

/-- Any property proved for all reachable abstract states transfers to the
concrete system through a stuttering simulation and its abstraction map. -/
theorem transfers_reachable_property
    (sim : StutteringSimulation concrete abstract (fun c => toAbstract (toMiddle c)))
    (property : Abstract → Prop)
    (holds : ∀ {a}, abstract.Reachable a → property a) :
    ∀ {c}, concrete.Reachable c → property (toAbstract (toMiddle c)) := by
  intro c hc
  exact holds (sim.maps_reachable hc)

end Continuum.StutteringSimulation
