namespace Continuum

/-- Semantic identity fields carried by a strongest-assurance result. Strings
stand in for canonical digests at this seed stage. -/
structure ProofReceipt where
  schemaVersion : Nat
  semanticEpoch : String
  proofEpoch : String
  modelDigest : String
  propertyDigest : String
  assumptionsDigest : String
  certificateDigest : String
  theoremNames : List String
  axioms : List String
  deriving DecidableEq, Repr

/-- Release receipts are axiom-clean when the declared axiom manifest is empty.
This is a policy predicate, not a claim that Lean itself has no trusted base. -/
def ProofReceipt.AxiomClean (receipt : ProofReceipt) : Prop :=
  receipt.axioms = []

end Continuum
