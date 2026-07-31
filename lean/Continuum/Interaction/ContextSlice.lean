import Continuum.EventStructure

namespace Continuum.Interaction

universe u

/-- A selected event predicate together with the causal-closure condition needed
for a replay-oriented Context Pack. -/
structure CausalSlice {Event : Type u}
    (es : Continuum.EventStructure Event) where
  selected : Event → Prop
  downwardClosed : ∀ a b, selected b → es.causal a b → selected a

/-- The full event set is always a causally closed slice. -/
def fullSlice {Event : Type u} (es : Continuum.EventStructure Event) :
    CausalSlice es where
  selected := fun _ => True
  downwardClosed := by
    intro _ _ _ _
    trivial

 theorem fullSlice_selects {Event : Type u}
    (es : Continuum.EventStructure Event) (e : Event) :
    (fullSlice es).selected e := by
  trivial

/-- Any selected event brings all of its causal predecessors into the slice. -/
theorem predecessor_selected {Event : Type u}
    {es : Continuum.EventStructure Event}
    (slice : CausalSlice es) {a b : Event}
    (hb : slice.selected b) (hab : es.causal a b) :
    slice.selected a := by
  exact slice.downwardClosed a b hb hab

end Continuum.Interaction
