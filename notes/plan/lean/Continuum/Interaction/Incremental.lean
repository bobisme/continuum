namespace Continuum.Interaction

universe u v

/-- A minimal exact-reuse witness: the normalized semantic inputs are equal. -/
structure ExactReuseWitness {Input : Type u} (before after : Input) : Prop where
  inputsEqual : before = after

/-- Exact reuse of a pure query preserves its output. -/
theorem exactReuse_sound {Input : Type u} {Output : Type v}
    (query : Input → Output) {before after : Input}
    (witness : ExactReuseWitness before after) :
    query before = query after := by
  simpa [witness.inputsEqual]

/-- Conservative invalidation may include extra changed inputs but must include
all truly changed dependencies. -/
def ConservativeClosure {Node : Type u}
    (trueDependency reportedDependency : Node → Node → Prop) : Prop :=
  ∀ a b, trueDependency a b → reportedDependency a b

 theorem conservative_includes_true_edge {Node : Type u}
    {trueDependency reportedDependency : Node → Node → Prop}
    (h : ConservativeClosure trueDependency reportedDependency)
    {a b : Node} (edge : trueDependency a b) :
    reportedDependency a b := by
  exact h a b edge

end Continuum.Interaction
