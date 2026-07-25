import Continuum.Semantics

namespace Continuum

universe u

/-- Logical meaning of a finite closed-set certificate. The executable checker
uses a serialized finite set and edge witnesses; this theorem is its semantic
contract. -/
structure ClosureCertificate
    {State : Type u}
    (sys : TransitionSystem State)
    (member : State → Prop)
    (safe : State → Prop) : Prop where
  containsInit : ∀ s, sys.init s → member s
  closed : ∀ s t, member s → sys.step s t → member t
  safeOnMember : ∀ s, member s → safe s

namespace ClosureCertificate

variable {State : Type u}
variable {sys : TransitionSystem State}
variable {member safe : State → Prop}

 theorem provesSafety
    (cert : ClosureCertificate sys member safe) :
    ∀ {s}, sys.Reachable s → safe s := by
  intro s hs
  have hind : sys.InductiveInvariant member := {
    init := cert.containsInit
    step := cert.closed
  }
  exact cert.safeOnMember _ (sys.reachable_satisfies hind hs)

end ClosureCertificate
end Continuum
