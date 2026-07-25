namespace Continuum.Interaction

/-- Canonical semantic identities for the protected portions of a Continuum
Intent Contract. The production representation is a typed AST; digests keep
this seed independent of the eventual serialization. -/
structure IntentSignature where
  properties : String
  assumptions : String
  observers : String
  bounds : String
  faults : String
  fairness : String
  assurance : String
  deriving DecidableEq, Repr

/-- A change preserves protected intent exactly when all protected semantic
identities remain equal. -/
def PreservesIntent (before after : IntentSignature) : Prop := before = after

/-- A repair proposal names the intent it began with and the intent attached to
its candidate workspace. -/
structure RepairIntentEdge where
  before : IntentSignature
  after : IntentSignature

/-- Ordinary repair policy accepts only exact preservation. Intent revisions
use a different policy and artifact class. -/
def RepairIntentEdge.Accepted (edge : RepairIntentEdge) : Prop :=
  PreservesIntent edge.before edge.after

 theorem preservesIntent_refl (intent : IntentSignature) :
    PreservesIntent intent intent := by
  rfl

 theorem accepted_implies_equal (edge : RepairIntentEdge)
    (h : edge.Accepted) : edge.before = edge.after := by
  exact h

 theorem preservesIntent_trans {a b c : IntentSignature}
    (hab : PreservesIntent a b) (hbc : PreservesIntent b c) :
    PreservesIntent a c := by
  exact Eq.trans hab hbc

end Continuum.Interaction
