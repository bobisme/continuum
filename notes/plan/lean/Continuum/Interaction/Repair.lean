import Continuum.Interaction.Intent

namespace Continuum.Interaction

/-- The proof-relevant seed of a repair evaluation. Production evidence fields
are content-addressed artifacts checked by their respective services. -/
structure RepairEvidence where
  exactReplayPasses : Prop
  neighborhoodPasses : Prop
  mutationChallengePasses : Prop
  proofImpactClosed : Prop
  cleanParity : Prop

/-- Promotion requires intent preservation and every mandatory evidence gate. -/
def RepairAccepted (intent : RepairIntentEdge) (evidence : RepairEvidence) : Prop :=
  intent.Accepted ∧
  evidence.exactReplayPasses ∧
  evidence.neighborhoodPasses ∧
  evidence.mutationChallengePasses ∧
  evidence.proofImpactClosed ∧
  evidence.cleanParity

 theorem accepted_preserves_intent
    (intent : RepairIntentEdge) (evidence : RepairEvidence)
    (h : RepairAccepted intent evidence) :
    intent.before = intent.after := by
  exact accepted_implies_equal intent h.1

 theorem accepted_exact_replay
    (intent : RepairIntentEdge) (evidence : RepairEvidence)
    (h : RepairAccepted intent evidence) :
    evidence.exactReplayPasses := by
  exact h.2.1

 theorem accepted_clean_parity
    (intent : RepairIntentEdge) (evidence : RepairEvidence)
    (h : RepairAccepted intent evidence) :
    evidence.cleanParity := by
  exact h.2.2.2.2.2

end Continuum.Interaction
