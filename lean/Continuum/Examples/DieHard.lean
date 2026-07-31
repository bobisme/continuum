namespace Continuum.Examples.DieHard

structure State where
  big : Fin 6
  small : Fin 4
  deriving DecidableEq, Repr

/-- The shortest witness found by breadth-first exploration, represented as the
initial state followed by the state after each action. -/
def solution : List State := [
  ⟨⟨0, by decide⟩, ⟨0, by decide⟩⟩,
  ⟨⟨5, by decide⟩, ⟨0, by decide⟩⟩,
  ⟨⟨2, by decide⟩, ⟨3, by decide⟩⟩,
  ⟨⟨2, by decide⟩, ⟨0, by decide⟩⟩,
  ⟨⟨0, by decide⟩, ⟨2, by decide⟩⟩,
  ⟨⟨5, by decide⟩, ⟨2, by decide⟩⟩,
  ⟨⟨4, by decide⟩, ⟨3, by decide⟩⟩
]

theorem solution_ends_with_four_gallons :
    solution[6]? = some ⟨⟨4, by decide⟩, ⟨3, by decide⟩⟩ := by
  decide

end Continuum.Examples.DieHard
