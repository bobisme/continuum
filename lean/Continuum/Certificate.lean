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

/-- RFC 0012 rung T0 (finite closure certificate soundness). A closure
certificate proves the safety property at every reachable state. -/
theorem provesSafety
    (cert : ClosureCertificate sys member safe) :
    ∀ {s}, sys.Reachable s → safe s := by
  intro s hs
  have hind : sys.InductiveInvariant member := {
    init := cert.containsInit
    step := cert.closed
  }
  exact cert.safeOnMember _ (sys.reachable_satisfies hind hs)

/-- RFC 0012 rung T0 (finite closure certificate soundness). The member predicate
of a closure certificate is an inductive invariant, which is how closure
certificates enter the invariant algebra of `Semantics`. -/
theorem isInductiveInvariant
    (cert : ClosureCertificate sys member safe) :
    sys.InductiveInvariant member where
  init := cert.containsInit
  step := cert.closed

/-- RFC 0012 rung T0 (finite closure certificate soundness), relative
completeness. A safety property holds of all reachable states exactly when some
closure certificate witnesses it, so the certificate *format* excludes no
provable safety claim. -/
theorem exists_iff_safety (safe : State → Prop) :
    (∃ member, ClosureCertificate sys member safe) ↔
      (∀ s, sys.Reachable s → safe s) := by
  constructor
  · intro h s hs
    obtain ⟨member, cert⟩ := h
    exact cert.provesSafety hs
  · intro h
    exact ⟨sys.Reachable, {
      containsInit := fun _ hi => .init hi
      closed := fun _ _ hr hst => hr.step hst
      safeOnMember := h }⟩

end ClosureCertificate

/-! ## Axiom-free Boolean plumbing for reflective checkers

The reflective checkers below use hand-rolled Boolean list operations rather than
`List.all` and `decide (· ∈ l)`. The core versions are correct, but their
correctness lemmas are proved through `propext`/`Quot.sound`, which would appear
in every certificate receipt. The definitions here are structurally identical and
their soundness and completeness lemmas depend on no axioms at all, which is what
ADR-0035 manifests are meant to record. -/

namespace Reflect

variable {α : Type u}

theorem andLeft {a b : Bool} (h : (a && b) = true) : a = true := by
  cases a with
  | true => rfl
  | false => exact Bool.noConfusion h

theorem andRight {a b : Bool} (h : (a && b) = true) : b = true := by
  cases a with
  | true => exact h
  | false => exact Bool.noConfusion h

theorem andIntro {a b : Bool} (ha : a = true) (hb : b = true) : (a && b) = true := by
  subst ha
  subst hb
  rfl

/-- Boolean membership test on a list. -/
def mem [DecidableEq α] (a : α) : List α → Bool
  | [] => false
  | b :: l => decide (a = b) || mem a l

theorem mem_sound [DecidableEq α] {a : α} :
    ∀ {l : List α}, mem a l = true → a ∈ l := by
  intro l
  induction l with
  | nil => intro h; exact Bool.noConfusion h
  | cons b rest ih =>
      intro h
      have h' : (decide (a = b) || mem a rest) = true := h
      cases hb : decide (a = b) with
      | true =>
          have hab : a = b := of_decide_eq_true hb
          subst hab
          exact List.Mem.head rest
      | false =>
          rw [hb] at h'
          exact List.Mem.tail b (ih h')

theorem mem_complete [DecidableEq α] {a : α} :
    ∀ {l : List α}, a ∈ l → mem a l = true := by
  intro l
  induction l with
  | nil => intro h; cases h
  | cons b rest ih =>
      intro h
      cases h with
      | head =>
          show (decide (a = a) || mem a rest) = true
          rw [decide_eq_true (rfl : a = a)]
          rfl
      | tail _ hrest =>
          show (decide (a = b) || mem a rest) = true
          cases hb : decide (a = b) with
          | true => rfl
          | false => exact ih hrest

/-- Boolean universal quantifier over a list. -/
def all (p : α → Bool) : List α → Bool
  | [] => true
  | a :: l => p a && all p l

theorem all_sound {p : α → Bool} :
    ∀ {l : List α} {a : α}, all p l = true → a ∈ l → p a = true := by
  intro l
  induction l with
  | nil => intro a _ hmem; cases hmem
  | cons b rest ih =>
      intro a h hmem
      have h' : (p b && all p rest) = true := h
      cases hmem with
      | head => exact andLeft h'
      | tail _ hrest => exact ih (andRight h') hrest

theorem all_complete {p : α → Bool} :
    ∀ {l : List α}, (∀ a ∈ l, p a = true) → all p l = true := by
  intro l
  induction l with
  | nil => intro _; rfl
  | cons b rest ih =>
      intro h
      show (p b && all p rest) = true
      exact andIntro (h b (List.Mem.head rest))
        (ih (fun a ha => h a (List.Mem.tail b ha)))

end Reflect

/-- An executable presentation of a finite-branching transition system: the Rust
explorer emits `initStates` and `succ`, and the Lean checkers below consume the
same data. Certificate checking is therefore reflection on this structure rather
than a giant generated proof term (RFC 0012, "Reflection"). -/
structure FiniteSystem (State : Type u) where
  initStates : List State
  succ : State → List State

namespace FiniteSystem

variable {State : Type u}

/-- The mathematical meaning of a `FiniteSystem`. -/
def toTransitionSystem (fs : FiniteSystem State) : TransitionSystem State where
  init s := s ∈ fs.initStates
  step s t := t ∈ fs.succ s

@[simp] theorem toTransitionSystem_init (fs : FiniteSystem State) (s : State) :
    fs.toTransitionSystem.init s = (s ∈ fs.initStates) := rfl

@[simp] theorem toTransitionSystem_step (fs : FiniteSystem State) (s t : State) :
    fs.toTransitionSystem.step s t = (t ∈ fs.succ s) := rfl

/-- Boolean check that `cert` contains every initial state and is closed under
successors. -/
def closureOk [DecidableEq State] (fs : FiniteSystem State) (cert : List State) : Bool :=
  Reflect.all (fun s => Reflect.mem s cert) fs.initStates &&
    Reflect.all (fun s => Reflect.all (fun t => Reflect.mem t cert) (fs.succ s)) cert

/-- The reflective finite-closure checker: `cert` is closed and every member is
safe. -/
def checkClosure [DecidableEq State] (fs : FiniteSystem State)
    (safe : State → Bool) (cert : List State) : Bool :=
  fs.closureOk cert && Reflect.all safe cert

/-- The checker is *exactly* the closure specification: no unclosed certificate is
accepted and no closed certificate is rejected. -/
theorem checkClosure_eq_true_iff [DecidableEq State] (fs : FiniteSystem State)
    (safe : State → Bool) (cert : List State) :
    fs.checkClosure safe cert = true ↔
      ((∀ s ∈ fs.initStates, s ∈ cert) ∧
        (∀ s ∈ cert, ∀ t ∈ fs.succ s, t ∈ cert) ∧
        (∀ s ∈ cert, safe s = true)) := by
  constructor
  · intro h
    have hpair : (fs.closureOk cert && Reflect.all safe cert) = true := h
    have hclosure : (Reflect.all (fun s => Reflect.mem s cert) fs.initStates &&
        Reflect.all (fun s => Reflect.all (fun t => Reflect.mem t cert) (fs.succ s)) cert) = true :=
      Reflect.andLeft hpair
    have hsafe := Reflect.andRight hpair
    have hinit := Reflect.andLeft hclosure
    have hclosed := Reflect.andRight hclosure
    refine ⟨fun s hs => ?_, ?_, fun s hs => Reflect.all_sound hsafe hs⟩
    · exact Reflect.mem_sound
        (Reflect.all_sound (p := fun s => Reflect.mem s cert) hinit hs)
    · intro s hs t ht
      have hrow := Reflect.all_sound
        (p := fun s => Reflect.all (fun t => Reflect.mem t cert) (fs.succ s)) hclosed hs
      exact Reflect.mem_sound
        (Reflect.all_sound (p := fun t => Reflect.mem t cert) hrow ht)
  · intro h
    obtain ⟨hinit, hclosed, hsafe⟩ := h
    refine Reflect.andIntro (Reflect.andIntro ?_ ?_) (Reflect.all_complete hsafe)
    · exact Reflect.all_complete (fun s hs => Reflect.mem_complete (hinit s hs))
    · exact Reflect.all_complete
        (fun s hs => Reflect.all_complete (fun t ht => Reflect.mem_complete (hclosed s hs t ht)))

/-- RFC 0012 rung T0 (finite closure certificate soundness), reflective form:
`check input cert = true → SemanticallyValid input`. An accepted certificate
yields the semantic `ClosureCertificate`. -/
theorem checkClosure_sound [DecidableEq State] {fs : FiniteSystem State}
    {safe : State → Bool} {cert : List State}
    (h : fs.checkClosure safe cert = true) :
    ClosureCertificate fs.toTransitionSystem (fun s => s ∈ cert) (fun s => safe s = true) := by
  obtain ⟨hinit, hclosed, hsafe⟩ := (fs.checkClosure_eq_true_iff safe cert).1 h
  exact {
    containsInit := fun s hs => hinit s hs
    closed := fun s t hs hst => hclosed s hs t hst
    safeOnMember := hsafe }

/-- RFC 0012 rung T0 (finite closure certificate soundness). The end-to-end
statement consumed by receipts: an accepted closure certificate proves the safety
property at every reachable state of the finite system. -/
theorem checkClosure_provesSafety [DecidableEq State] {fs : FiniteSystem State}
    {safe : State → Bool} {cert : List State}
    (h : fs.checkClosure safe cert = true) :
    ∀ {s : State}, fs.toTransitionSystem.Reachable s → safe s = true :=
  fun hs => (checkClosure_sound h).provesSafety hs

/-- Negative direction: a certificate that omits a successor of one of its own
members is rejected. Malformed closures cannot be laundered into theorems. -/
theorem checkClosure_eq_false_of_unclosed [DecidableEq State] (fs : FiniteSystem State)
    (safe : State → Bool) {cert : List State} {s t : State}
    (hs : s ∈ cert) (hst : t ∈ fs.succ s) (hnot : t ∉ cert) :
    fs.checkClosure safe cert = false := by
  cases hcheck : fs.checkClosure safe cert with
  | false => rfl
  | true =>
      obtain ⟨_, hclosed, _⟩ := (fs.checkClosure_eq_true_iff safe cert).1 hcheck
      exact absurd (hclosed s hs t hst) hnot

/-- Negative direction: a certificate that misses an initial state is rejected. -/
theorem checkClosure_eq_false_of_missing_init [DecidableEq State]
    (fs : FiniteSystem State) (safe : State → Bool) {cert : List State} {s : State}
    (hs : s ∈ fs.initStates) (hnot : s ∉ cert) :
    fs.checkClosure safe cert = false := by
  cases hcheck : fs.checkClosure safe cert with
  | false => rfl
  | true =>
      obtain ⟨hinit, _, _⟩ := (fs.checkClosure_eq_true_iff safe cert).1 hcheck
      exact absurd (hinit s hs) hnot

/-- Boolean check that `l` is a sequence of successors starting at `s`. -/
def checkTrace [DecidableEq State] (fs : FiniteSystem State) :
    State → List State → Bool
  | _, [] => true
  | s, t :: rest => Reflect.mem t (fs.succ s) && checkTrace fs t rest

/-- The reflective counterexample checker: the head must be initial, the tail must
be a valid trace, and the final state must violate the property. -/
def checkCounterexample [DecidableEq State] (fs : FiniteSystem State)
    (bad : State → Bool) : List State → Bool
  | [] => false
  | s :: rest =>
      Reflect.mem s fs.initStates && fs.checkTrace s rest &&
        bad (TransitionSystem.endpoint s rest)

/-- An accepted trace is a genuine path of the underlying transition system. -/
theorem checkTrace_sound [DecidableEq State] (fs : FiniteSystem State) :
    ∀ {s : State} {l : List State},
      fs.checkTrace s l = true → fs.toTransitionSystem.PathFrom s l := by
  intro s l
  induction l generalizing s with
  | nil => intro _; exact .nil
  | cons t rest ih =>
      intro h
      have h' : (Reflect.mem t (fs.succ s) && fs.checkTrace t rest) = true := h
      exact .cons (Reflect.mem_sound (Reflect.andLeft h')) (ih (Reflect.andRight h'))

/-- RFC 0012 rung T0 (counterexample path validity), reflective form. An accepted
counterexample certificate proves that some reachable state violates the
property. -/
theorem checkCounterexample_sound [DecidableEq State] {fs : FiniteSystem State}
    {bad : State → Bool} {p : List State}
    (h : fs.checkCounterexample bad p = true) :
    ∃ s : State, fs.toTransitionSystem.Reachable s ∧ bad s = true := by
  cases p with
  | nil => exact Bool.noConfusion h
  | cons s rest =>
      have h' : (Reflect.mem s fs.initStates && fs.checkTrace s rest &&
          bad (TransitionSystem.endpoint s rest)) = true := h
      have hinit := Reflect.mem_sound (Reflect.andLeft (Reflect.andLeft h'))
      have htrace := Reflect.andRight (Reflect.andLeft h')
      have hbad := Reflect.andRight h'
      exact ⟨_, fs.toTransitionSystem.reachable_of_initial_path hinit
        (fs.checkTrace_sound htrace), hbad⟩

/-- RFC 0012 rung T0. The two T0 certificate families are mutually exclusive: an
accepted counterexample refutes every closure certificate for the negated
property, so a receipt cannot carry both. -/
theorem no_closure_of_counterexample [DecidableEq State] {fs : FiniteSystem State}
    {bad : State → Bool} {p cert : List State}
    (hcl : fs.checkClosure (fun s => !bad s) cert = true)
    (hce : fs.checkCounterexample bad p = true) : False := by
  obtain ⟨s, hr, hbad⟩ := checkCounterexample_sound hce
  have hsafe : (!bad s) = true := checkClosure_provesSafety hcl hr
  rw [hbad] at hsafe
  exact Bool.noConfusion hsafe

end FiniteSystem
end Continuum
