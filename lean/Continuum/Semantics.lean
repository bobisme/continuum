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

/-- RFC 0012 theorem ladder, rung T0 (reachability induction). Every inductive
invariant holds at every reachable state. This is the soundness half of the
invariant method and the root of the whole safety family. -/
theorem reachable_satisfies
    {inv : State → Prop}
    (h : sys.InductiveInvariant inv) :
    ∀ {s}, sys.Reachable s → inv s := by
  intro s hs
  induction hs with
  | init hi => exact h.init _ hi
  | step _ hst ih => exact h.step _ _ ih hst

/-- RFC 0012 rung T0 (reachability induction). Reachability is itself an
inductive invariant, hence the strongest invariant of the system. -/
theorem reachable_isInductive : sys.InductiveInvariant sys.Reachable where
  init := fun _ hi => .init hi
  step := fun _ _ hr hst => .step hr hst

/-- RFC 0012 rung T0 (reachability induction). `Reachable` is the *least*
inductive invariant: it is contained in every other one. -/
theorem reachable_least
    {inv : State → Prop}
    (h : sys.InductiveInvariant inv) :
    ∀ s, sys.Reachable s → inv s :=
  fun _ hs => sys.reachable_satisfies h hs

/-- RFC 0012 rung T0 (inductive invariant preservation). Relative completeness of
the invariant method: a safety property holds of all reachable states exactly
when some inductive invariant witnesses it. Certificate search is therefore not
restricted by the proof rule, only by the invariant language. -/
theorem safety_iff_exists_inductiveInvariant (safe : State → Prop) :
    (∀ s, sys.Reachable s → safe s) ↔
      ∃ inv, sys.InductiveInvariant inv ∧ ∀ s, inv s → safe s := by
  constructor
  · intro h
    exact ⟨sys.Reachable, sys.reachable_isInductive, h⟩
  · intro h s hs
    obtain ⟨inv, hind, himp⟩ := h
    exact himp _ (sys.reachable_satisfies hind hs)

/-- RFC 0012 rung T0 (inductive invariant preservation). Inductive invariants are
closed under conjunction, so compositional invariant graphs may be assembled
component-wise. -/
theorem InductiveInvariant.and
    {p q : State → Prop}
    (hp : sys.InductiveInvariant p) (hq : sys.InductiveInvariant q) :
    sys.InductiveInvariant (fun s => p s ∧ q s) where
  init := fun s hi => ⟨hp.init s hi, hq.init s hi⟩
  step := fun s t h hst => ⟨hp.step s t h.1 hst, hq.step s t h.2 hst⟩

/-- RFC 0012 rung T0 (inductive invariant preservation). The trivially true
predicate is inductive; it is the unit of the conjunction above. -/
theorem InductiveInvariant.top : sys.InductiveInvariant (fun _ => True) where
  init := fun _ _ => trivial
  step := fun _ _ _ _ => trivial

/-- RFC 0012 rung T0 (inductive invariant preservation). An inductive invariant
may be strengthened by any inductive invariant and weakened by any consequence
that is itself closed under the same transitions. -/
theorem InductiveInvariant.mono
    {p q : State → Prop}
    (hp : sys.InductiveInvariant p)
    (himp : ∀ s, p s → q s)
    (hclosed : ∀ s t, q s → sys.step s t → q t) :
    sys.InductiveInvariant q where
  init := fun s hi => himp s (hp.init s hi)
  step := hclosed

/-- A finite step-path. `sys.PathFrom s l` holds when every state listed in `l`
follows the previous one by a single transition, starting from `s`. -/
inductive PathFrom : State → List State → Prop
  | nil {s} : PathFrom s []
  | cons {s t l} : sys.step s t → PathFrom t l → PathFrom s (t :: l)

/-- The final state of a path that starts at `s`. -/
def endpoint : State → List State → State
  | s, [] => s
  | _, t :: rest => endpoint t rest

@[simp] theorem endpoint_nil (s : State) : endpoint s ([] : List State) = s := rfl

@[simp] theorem endpoint_cons (s t : State) (l : List State) :
    endpoint s (t :: l) = endpoint t l := rfl

theorem endpoint_append (s : State) (l₁ l₂ : List State) :
    endpoint s (l₁ ++ l₂) = endpoint (endpoint s l₁) l₂ := by
  induction l₁ generalizing s with
  | nil => rfl
  | cons t rest ih => simpa using ih t

theorem pathFrom_append
    {s t : State} {l : List State}
    (hp : sys.PathFrom s l) (hstep : sys.step (endpoint s l) t) :
    sys.PathFrom s (l ++ [t]) := by
  induction l generalizing s with
  | nil => exact .cons hstep .nil
  | cons u rest ih =>
      cases hp with
      | cons hfirst hrest => exact .cons hfirst (ih hrest hstep)

/-- RFC 0012 rung T0 (counterexample path validity). A path out of a reachable
state ends in a reachable state: a checked path is a valid witness. -/
theorem reachable_endpoint
    {s : State} {l : List State}
    (hs : sys.Reachable s) (hp : sys.PathFrom s l) :
    sys.Reachable (endpoint s l) := by
  induction l generalizing s with
  | nil => exact hs
  | cons t rest ih =>
      cases hp with
      | cons hfirst hrest => exact ih (hs.step hfirst) hrest

/-- RFC 0012 rung T0 (counterexample path validity). The specialization used by
counterexample certificates: a path out of an *initial* state ends in a
reachable state. -/
theorem reachable_of_initial_path
    {s : State} {l : List State}
    (hi : sys.init s) (hp : sys.PathFrom s l) :
    sys.Reachable (endpoint s l) :=
  sys.reachable_endpoint (.init hi) hp

/-- RFC 0012 rung T0 (counterexample path validity, completeness). Every
reachable state is the endpoint of some finite path out of an initial state, so
refutation search that only reports paths is complete for reachability. -/
theorem exists_path_of_reachable
    {s : State} (hs : sys.Reachable s) :
    ∃ s₀ l, sys.init s₀ ∧ sys.PathFrom s₀ l ∧ endpoint s₀ l = s := by
  induction hs with
  | @init s hi => exact ⟨s, [], hi, .nil, rfl⟩
  | @step s t _ hstep ih =>
      obtain ⟨s₀, l, hi, hp, he⟩ := ih
      refine ⟨s₀, l ++ [t], hi, sys.pathFrom_append hp ?_, ?_⟩
      · rw [he]; exact hstep
      · rw [endpoint_append, he]; rfl

/-- RFC 0012 rung T0 (reachability induction). Reachability is exactly path
existence: the inductive and the operational presentations agree. -/
theorem reachable_iff_exists_path {s : State} :
    sys.Reachable s ↔
      ∃ s₀ l, sys.init s₀ ∧ sys.PathFrom s₀ l ∧ endpoint s₀ l = s := by
  constructor
  · exact sys.exists_path_of_reachable
  · intro h
    obtain ⟨s₀, l, hi, hp, he⟩ := h
    exact he ▸ sys.reachable_of_initial_path hi hp

/-- RFC 0012 rung T0 (inductive invariant preservation). An inductive invariant
is preserved along every finite path, not merely across single steps. -/
theorem InductiveInvariant.holds_on_path
    {inv : State → Prop}
    (h : sys.InductiveInvariant inv)
    {s : State} {l : List State}
    (hs : inv s) (hp : sys.PathFrom s l) :
    inv (endpoint s l) := by
  induction l generalizing s with
  | nil => exact hs
  | cons t rest ih =>
      cases hp with
      | cons hfirst hrest => exact ih (h.step _ _ hs hfirst) hrest

end TransitionSystem
end Continuum
