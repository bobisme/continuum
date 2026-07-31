namespace Continuum

inductive CancelPhase
  | running
  | requested
  | draining
  | finalizing
  | cancelled
  deriving DecidableEq, Repr

/-- The structural lifecycle accepted by the cancellation calculus. -/
def CancelStep : CancelPhase → CancelPhase → Prop
  | .running, .requested => True
  | .requested, .draining => True
  | .draining, .finalizing => True
  | .finalizing, .cancelled => True
  | _, _ => False

/-- A simple phase rank used as the seed for progress/ranking proofs. -/
def CancelPhase.rank : CancelPhase → Nat
  | .running => 4
  | .requested => 3
  | .draining => 2
  | .finalizing => 1
  | .cancelled => 0

theorem cancel_step_decreases_rank {a b : CancelPhase}
    (h : CancelStep a b) : b.rank < a.rank := by
  cases a <;> cases b <;> simp [CancelStep, CancelPhase.rank] at h ⊢

end Continuum
