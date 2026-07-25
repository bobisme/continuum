namespace Continuum

universe u

/-- A small prime-event-structure seed. The production development will add
labels, observers, intervals, and effect footprints in separate layers. -/
structure EventStructure (Event : Type u) where
  causal : Event → Event → Prop
  conflict : Event → Event → Prop
  causalRefl : ∀ e, causal e e
  causalTrans : ∀ a b c, causal a b → causal b c → causal a c
  conflictSymm : ∀ a b, conflict a b → conflict b a
  conflictHereditary : ∀ a b c, conflict a b → causal b c → conflict a c

namespace EventStructure

variable {Event : Type u} (es : EventStructure Event)

/-- A configuration is conflict-free and downward closed. -/
structure IsConfiguration (member : Event → Prop) : Prop where
  conflictFree : ∀ a b, member a → member b → ¬ es.conflict a b
  downwardClosed : ∀ a b, member b → es.causal a b → member a

theorem empty_isConfiguration : es.IsConfiguration (fun _ => False) := by
  constructor
  · intro a b ha
    contradiction
  · intro a b hb
    contradiction

end EventStructure
end Continuum
