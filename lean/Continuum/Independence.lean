namespace Continuum

universe u v w

/-- Two deterministic events commute as observed by `observe` from a state. -/
def CommutesUnder
    {State : Type u} {Obs : Type v}
    (observe : State → Obs)
    (left right : State → State)
    (s : State) : Prop :=
  observe (right (left s)) = observe (left (right s))

/-- `fine` refines `coarse` when equality of fine observations implies
 equality of coarse observations. -/
def ObserverRefines
    {State : Type u} {Fine : Type v} {Coarse : Type w}
    (fine : State → Fine) (coarse : State → Coarse) : Prop :=
  ∀ a b, fine a = fine b → coarse a = coarse b

/-- Independence is monotone down the observer lattice: events commuting for a
finer observer commute for every coarser observer. -/
theorem commutes_mono
    {State : Type u} {Fine : Type v} {Coarse : Type w}
    {fine : State → Fine} {coarse : State → Coarse}
    {left right : State → State} {s : State}
    (href : ObserverRefines fine coarse)
    (hcomm : CommutesUnder fine left right s) :
    CommutesUnder coarse left right s := by
  exact href _ _ hcomm

end Continuum
