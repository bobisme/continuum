namespace Continuum.Interaction

universe u

/-- Hard requirements for a finite synthesis candidate. -/
structure CandidateRequirements (Candidate : Type u) where
  safety : Candidate → Prop
  progress : Candidate → Prop
  implementable : Candidate → Prop

/-- Forge acceptance deliberately includes progress/non-vacuity rather than
safety alone. -/
def CandidateAccepted {Candidate : Type u}
    (requirements : CandidateRequirements Candidate)
    (candidate : Candidate) : Prop :=
  requirements.safety candidate ∧
  requirements.progress candidate ∧
  requirements.implementable candidate

 theorem accepted_is_safe {Candidate : Type u}
    (requirements : CandidateRequirements Candidate) (candidate : Candidate)
    (h : CandidateAccepted requirements candidate) :
    requirements.safety candidate := by
  exact h.1

 theorem accepted_makes_progress {Candidate : Type u}
    (requirements : CandidateRequirements Candidate) (candidate : Candidate)
    (h : CandidateAccepted requirements candidate) :
    requirements.progress candidate := by
  exact h.2.1

 theorem accepted_is_implementable {Candidate : Type u}
    (requirements : CandidateRequirements Candidate) (candidate : Candidate)
    (h : CandidateAccepted requirements candidate) :
    requirements.implementable candidate := by
  exact h.2.2

end Continuum.Interaction
