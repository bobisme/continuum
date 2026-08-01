import Continuum.Semantics
import Continuum.Temporal

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

/-- A stuttering simulation whose step obligation is only required at *reachable*
concrete states. Real refinement proofs take this form: the step condition is
discharged under the concrete system's inductive invariant. -/
structure ReachableSimulation
    {Concrete : Type u} {Abstract : Type v}
    (concrete : TransitionSystem Concrete)
    (abstract : TransitionSystem Abstract)
    (view : Concrete → Abstract) : Prop where
  init : ∀ c, concrete.init c → abstract.init (view c)
  step : ∀ c c', concrete.Reachable c → concrete.step c c' →
    view c = view c' ∨ abstract.step (view c) (view c')

namespace StutteringSimulation

variable {Concrete : Type u} {Abstract : Type v}
variable {concrete : TransitionSystem Concrete}
variable {abstract : TransitionSystem Abstract}
variable {view : Concrete → Abstract}

/-- Reachable concrete states map to reachable abstract states when concrete
steps either stutter or perform one abstract step.

RFC 0012 rung T1 (stuttering simulation preserves reachable safety predicates):
this is the reachability half of that rung. -/
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

/-- RFC 0012 rung T1 (stuttering simulation preserves reachable safety
predicates). Any safety predicate established for all reachable abstract states
holds, through the view map, at every reachable concrete state. -/
theorem transfer_safety
    (sim : StutteringSimulation concrete abstract view)
    (safe : Abstract → Prop)
    (habs : ∀ a, abstract.Reachable a → safe a) :
    ∀ c, concrete.Reachable c → safe (view c) :=
  fun _ hc => habs _ (sim.maps_reachable hc)

/-- RFC 0012 rung T1 (composition of simulations). Identity is a stuttering
simulation, so refinement chains have a unit. -/
theorem refl (sys : TransitionSystem Concrete) :
    StutteringSimulation sys sys (fun c => c) where
  init := fun _ hi => hi
  step := fun _ _ hstep => Or.inr hstep

/-- Every stuttering simulation is in particular a reachable simulation. -/
theorem toReachable
    (sim : StutteringSimulation concrete abstract view) :
    ReachableSimulation concrete abstract view where
  init := sim.init
  step := fun c c' _ hstep => sim.step c c' hstep

/-- RFC 0012 rung T1 (conditions for liveness preservation). The projection of a
concrete run along a stuttering simulation is an abstract *stuttering* run; it is
not, in general, an abstract run. -/
theorem projects_run
    (sim : StutteringSimulation concrete abstract view)
    {b : Behavior Concrete} (hb : concrete.IsRun b) :
    abstract.IsStutterRun (fun n => view (b n)) :=
  ⟨sim.init _ hb.1, fun n => sim.step _ _ (hb.2 n)⟩

/-- RFC 0012 rung T1 (conditions for liveness preservation), first condition. An
abstract eventuality proved *stutter-robustly* — for every abstract stuttering
run, as an LTL-X property must be — transfers to every concrete run. -/
theorem transfer_eventually
    (sim : StutteringSimulation concrete abstract view)
    (p : Abstract → Prop)
    (habs : ∀ a : Behavior Abstract, abstract.IsStutterRun a → Eventually p a)
    {b : Behavior Concrete} (hb : concrete.IsRun b) :
    Eventually (fun c => p (view c)) b :=
  habs _ (sim.projects_run hb)

/-- RFC 0012 rung T1 (conditions for liveness preservation), recurrence form. -/
theorem transfer_infinitelyOften
    (sim : StutteringSimulation concrete abstract view)
    (p : Abstract → Prop)
    (habs : ∀ a : Behavior Abstract, abstract.IsStutterRun a → InfinitelyOften p a)
    {b : Behavior Concrete} (hb : concrete.IsRun b) :
    InfinitelyOften (fun c => p (view c)) b :=
  habs _ (sim.projects_run hb)

/-- RFC 0012 rung T1 (conditions for liveness preservation), second condition. A
*visible* simulation — one that never stutters — projects runs to runs, so
liveness proved over ordinary abstract runs transfers without a stutter-closure
side condition. -/
theorem projects_run_of_visible
    (sim : StutteringSimulation concrete abstract view)
    (hvis : ∀ c c', concrete.step c c' → view c ≠ view c')
    {b : Behavior Concrete} (hb : concrete.IsRun b) :
    abstract.IsRun (fun n => view (b n)) := by
  refine ⟨sim.init _ hb.1, fun n => ?_⟩
  rcases sim.step _ _ (hb.2 n) with heq | hstep
  · exact absurd heq (hvis _ _ (hb.2 n))
  · exact hstep

end StutteringSimulation

namespace ReachableSimulation

variable {Concrete : Type u} {Abstract : Type v}
variable {concrete : TransitionSystem Concrete}
variable {abstract : TransitionSystem Abstract}
variable {view : Concrete → Abstract}

/-- RFC 0012 rung T1 (stuttering simulation preserves reachable safety
predicates), invariant-relative form. The step obligation is only needed at
reachable concrete states. -/
theorem maps_reachable
    (sim : ReachableSimulation concrete abstract view) :
    ∀ {c}, concrete.Reachable c → abstract.Reachable (view c) := by
  intro c hc
  induction hc with
  | init hi => exact .init (sim.init _ hi)
  | @step s t hr hstep ih =>
      rcases sim.step _ _ hr hstep with hstutter | habs
      · exact hstutter ▸ ih
      · exact .step ih habs

/-- RFC 0012 rung T1. Safety transfer for the invariant-relative form. -/
theorem transfer_safety
    (sim : ReachableSimulation concrete abstract view)
    (safe : Abstract → Prop)
    (habs : ∀ a, abstract.Reachable a → safe a) :
    ∀ c, concrete.Reachable c → safe (view c) :=
  fun _ hc => habs _ (sim.maps_reachable hc)

/-- RFC 0012 rung T1. How invariants enter refinement proofs: a step condition
discharged under any inductive invariant of the concrete system yields a
reachable simulation. -/
theorem of_inductiveInvariant
    {inv : Concrete → Prop}
    (hinv : concrete.InductiveInvariant inv)
    (hinit : ∀ c, concrete.init c → abstract.init (view c))
    (hstep : ∀ c c', inv c → concrete.step c c' →
      view c = view c' ∨ abstract.step (view c) (view c')) :
    ReachableSimulation concrete abstract view where
  init := hinit
  step := fun c c' hr hs => hstep c c' (concrete.reachable_satisfies hinv hr) hs

end ReachableSimulation
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
refinement-graph receipt composition rule.

RFC 0012 rung T1 (composition of simulations). -/
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

namespace Continuum.ReachableSimulation

universe u v w

variable {Concrete : Type u} {Middle : Type v} {Abstract : Type w}
variable {concrete : TransitionSystem Concrete}
variable {middle : TransitionSystem Middle}
variable {abstract : TransitionSystem Abstract}
variable {toMiddle : Concrete → Middle}
variable {toAbstract : Middle → Abstract}

/-- RFC 0012 rung T1 (composition of simulations), invariant-relative form. The
middle system's reachability obligation is discharged by the lower simulation, so
refinement chains compose without re-proving invariants. -/
theorem compose
    (lower : ReachableSimulation concrete middle toMiddle)
    (upper : ReachableSimulation middle abstract toAbstract) :
    ReachableSimulation concrete abstract (fun c => toAbstract (toMiddle c)) := by
  constructor
  · intro c hc
    exact upper.init _ (lower.init _ hc)
  · intro c c' hreach hstep
    rcases lower.step _ _ hreach hstep with hlower | hmiddle
    · left
      exact congrArg toAbstract hlower
    · rcases upper.step _ _ (lower.maps_reachable hreach) hmiddle with hupper | habstract
      · left
        exact hupper
      · right
        exact habstract

end Continuum.ReachableSimulation

namespace Continuum

universe u v

namespace TransitionSystem

/-- The observed quotient of `sys` under an observation map. Only *visible*
transitions — those that change the observation — appear, and they are witnessed
from reachable states only. Hidden (observation-preserving) steps are closed away
by stuttering.

RFC 0012 rung T1 (observer projection and hidden-event closure). -/
def observe {State : Type u} {Obs : Type v}
    (sys : TransitionSystem State) (obs : State → Obs) : TransitionSystem Obs where
  init o := ∃ s, sys.init s ∧ obs s = o
  step o o' := ∃ s t, sys.Reachable s ∧ sys.step s t ∧ obs s ≠ obs t ∧
    obs s = o ∧ obs t = o'

variable {State : Type u} {Obs : Type v}

/-- RFC 0012 rung T1 (hidden-event closure). The quotient has no hidden
transitions: every observed step genuinely changes the observation. -/
theorem observe_step_visible
    (sys : TransitionSystem State) (obs : State → Obs) {o o' : Obs}
    (h : (sys.observe obs).step o o') : o ≠ o' := by
  obtain ⟨s, t, _, _, hne, hs, ht⟩ := h
  intro heq
  exact hne (by rw [hs, ht, heq])

/-- RFC 0012 rung T1 (observer projection). The observation map is a reachable
stuttering simulation onto the observed quotient. -/
theorem observe_simulation [DecidableEq Obs]
    (sys : TransitionSystem State) (obs : State → Obs) :
    ReachableSimulation sys (sys.observe obs) obs where
  init := fun c hc => ⟨c, hc, rfl⟩
  step := fun c c' hreach hstep =>
    if h : obs c = obs c' then Or.inl h
    else Or.inr ⟨c, c', hreach, hstep, h, rfl, rfl⟩

/-- RFC 0012 rung T1 (observer projection and hidden-event closure). The reachable
observations of the quotient are exactly the observations of reachable concrete
states: dropping hidden steps loses nothing and invents nothing. -/
theorem observe_reachable_iff [DecidableEq Obs]
    (sys : TransitionSystem State) (obs : State → Obs) {o : Obs} :
    (sys.observe obs).Reachable o ↔ ∃ s, sys.Reachable s ∧ obs s = o := by
  constructor
  · intro h
    induction h with
    | @init o hi =>
        obtain ⟨s, hs, he⟩ := hi
        exact ⟨s, .init hs, he⟩
    | @step o o' _ hstep _ =>
        obtain ⟨s, t, hr, hst, _, _, he⟩ := hstep
        exact ⟨t, hr.step hst, he⟩
  · intro h
    obtain ⟨s, hs, he⟩ := h
    exact he ▸ (observe_simulation sys obs).maps_reachable hs

/-- RFC 0012 rung T1 (observer projection). Observation-only safety properties may
be discharged on the quotient without loss: the projection is sound *and*
complete for them. -/
theorem observe_safety_iff [DecidableEq Obs]
    (sys : TransitionSystem State) (obs : State → Obs) (p : Obs → Prop) :
    (∀ o, (sys.observe obs).Reachable o → p o) ↔ (∀ s, sys.Reachable s → p (obs s)) := by
  constructor
  · intro h s hs
    exact h _ ((observe_reachable_iff sys obs).2 ⟨s, hs, rfl⟩)
  · intro h o ho
    obtain ⟨s, hs, he⟩ := (observe_reachable_iff sys obs).1 ho
    exact he ▸ h s hs

end TransitionSystem

/-- A history (auxiliary) variable added to `sys`. The augmentation must be
*total*: every initial state admits an initial history value and every step
admits a history update. Totality is exactly what makes erasure sound.

RFC 0012 rung T1 (history/auxiliary variable erasure). -/
structure HistoryAugmentation
    {State : Type u} (sys : TransitionSystem State) (Hist : Type v) where
  init : State → Hist → Prop
  step : State → Hist → State → Hist → Prop
  initTotal : ∀ s, sys.init s → ∃ h, init s h
  stepTotal : ∀ s h t, sys.step s t → ∃ h', step s h t h'

namespace HistoryAugmentation

variable {State : Type u} {Hist : Type v} {sys : TransitionSystem State}

/-- The augmented system over `State × Hist`. The state component evolves exactly
as before; the history component only records. -/
def system (aug : HistoryAugmentation sys Hist) : TransitionSystem (State × Hist) where
  init p := sys.init p.1 ∧ aug.init p.1 p.2
  step p q := sys.step p.1 q.1 ∧ aug.step p.1 p.2 q.1 q.2

/-- RFC 0012 rung T1 (history/auxiliary variable erasure). Erasing the auxiliary
component is a stuttering simulation — in fact a step-for-step one. -/
theorem erasure_simulation (aug : HistoryAugmentation sys Hist) :
    StutteringSimulation aug.system sys Prod.fst where
  init := fun _ hi => hi.1
  step := fun _ _ hstep => Or.inr hstep.1

/-- RFC 0012 rung T1 (history/auxiliary variable erasure). Totality lifts every
reachable base state to a reachable augmented state, so the augmentation removes
no behavior. -/
theorem reachable_lift (aug : HistoryAugmentation sys Hist)
    {s : State} (hs : sys.Reachable s) :
    ∃ h : Hist, aug.system.Reachable (s, h) := by
  induction hs with
  | @init s hi =>
      obtain ⟨h, hh⟩ := aug.initTotal s hi
      exact ⟨h, .init ⟨hi, hh⟩⟩
  | @step s t _ hstep ih =>
      obtain ⟨h, hr⟩ := ih
      obtain ⟨h', hh'⟩ := aug.stepTotal s h t hstep
      exact ⟨h', hr.step ⟨hstep, hh'⟩⟩

/-- RFC 0012 rung T1 (history/auxiliary variable erasure). A safety property of
the base state is provable on the augmented system exactly when it holds of the
base system: auxiliary variables are a sound *and* complete proof device, and can
be erased from the receipt. -/
theorem erases_safety (aug : HistoryAugmentation sys Hist) (p : State → Prop) :
    (∀ q : State × Hist, aug.system.Reachable q → p q.1) ↔
      (∀ s, sys.Reachable s → p s) := by
  constructor
  · intro h s hs
    obtain ⟨hh, hr⟩ := aug.reachable_lift hs
    exact h (s, hh) hr
  · intro h q hq
    exact h _ ((aug.erasure_simulation).maps_reachable hq)

end HistoryAugmentation
end Continuum
