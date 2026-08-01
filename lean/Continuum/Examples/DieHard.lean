import Continuum.Certificate

namespace Continuum.Examples.DieHard

/-- Jug contents. The capacities are *not* encoded in the types, so `TypeOK`
below is a genuine invariant proved by a certificate rather than a type fact. -/
structure State where
  big : Nat
  small : Nat
  deriving DecidableEq, Repr

/-- The shortest witness found by breadth-first exploration, represented as the
initial state followed by the state after each action. -/
def solution : List State := [
  ⟨0, 0⟩,
  ⟨5, 0⟩,
  ⟨2, 3⟩,
  ⟨2, 0⟩,
  ⟨0, 2⟩,
  ⟨5, 2⟩,
  ⟨4, 3⟩
]

theorem solution_ends_with_four_gallons :
    solution[6]? = some ⟨4, 3⟩ := by
  decide

/-! ## The jug puzzle as an executable finite transition system

These definitions turn the puzzle into a `Continuum.FiniteSystem`: the shape a
Rust explorer emits and the reflective checkers of `Continuum.Certificate`
consume. The theorems at the end of this file are RFC 0012 rung T0 instantiated
on real data — a finite closure certificate with its negative cases, and a
counterexample path certificate. -/

/-- `min` without the order-instance detour, so that the executable model depends
on no lemmas at all and its receipts stay axiom-free. -/
def natMin (a b : Nat) : Nat := cond (Nat.ble a b) a b

def fillBig (s : State) : State := { s with big := 5 }

def fillSmall (s : State) : State := { s with small := 3 }

def emptyBig (s : State) : State := { s with big := 0 }

def emptySmall (s : State) : State := { s with small := 0 }

/-- Pour the small jug into the big one until one of them changes state. -/
def pourSmallToBig (s : State) : State :=
  ⟨s.big + natMin s.small (5 - s.big), s.small - natMin s.small (5 - s.big)⟩

/-- Pour the big jug into the small one until one of them changes state. -/
def pourBigToSmall (s : State) : State :=
  ⟨s.big - natMin s.big (3 - s.small), s.small + natMin s.big (3 - s.small)⟩

def actions : List (State → State) :=
  [fillBig, fillSmall, emptyBig, emptySmall, pourSmallToBig, pourBigToSmall]

def successors (s : State) : List State := actions.map (fun act => act s)

/-- The Die Hard jug puzzle as an executable finite system. -/
def system : Continuum.FiniteSystem State where
  initStates := [⟨0, 0⟩]
  succ := successors

/-- The reachable-state set an exploration engine emits as a finite closure
certificate: sixteen states. -/
def reachableClosure : List State :=
  [⟨0, 0⟩, ⟨0, 1⟩, ⟨0, 2⟩, ⟨0, 3⟩,
   ⟨1, 0⟩, ⟨1, 3⟩,
   ⟨2, 0⟩, ⟨2, 3⟩,
   ⟨3, 0⟩, ⟨3, 3⟩,
   ⟨4, 0⟩, ⟨4, 3⟩,
   ⟨5, 0⟩, ⟨5, 1⟩, ⟨5, 2⟩, ⟨5, 3⟩]

/-- Jug capacities are respected: the `TypeOK` of the TLA+ original. -/
def typeOK (s : State) : Bool := Nat.ble s.big 5 && Nat.ble s.small 3

/-- `TypeOK` together with the measurement invariant "one jug is always empty or
full". The second conjunct fails at eight capacity-respecting states, so it is
not a consequence of typing. -/
def jugInvariant (s : State) : Bool :=
  typeOK s && (s.small == 0 || s.small == 3 || s.big == 0 || s.big == 5)

/-- Positive evidence: the emitted closure certificate is accepted. -/
theorem closure_accepted :
    system.checkClosure jugInvariant reachableClosure = true := by decide

/-- RFC 0012 rung T0 (finite closure certificate soundness), instantiated: the
accepted certificate proves the invariant at every reachable state. This is the
Die Hard `TypeOK` milestone of the RFC's acceptance section. -/
theorem jugInvariant_of_reachable :
    ∀ {s : State}, system.toTransitionSystem.Reachable s → jugInvariant s = true :=
  Continuum.FiniteSystem.checkClosure_provesSafety closure_accepted

/-- The same statement at the level of ordinary arithmetic. -/
theorem typeOK_of_reachable {s : State}
    (h : system.toTransitionSystem.Reachable s) : s.big ≤ 5 ∧ s.small ≤ 3 := by
  have hinv : (typeOK s && (s.small == 0 || s.small == 3 || s.big == 0 || s.big == 5)) = true :=
    jugInvariant_of_reachable h
  have htype : (Nat.ble s.big 5 && Nat.ble s.small 3) = true := Reflect.andLeft hinv
  exact ⟨Nat.le_of_ble_eq_true (Reflect.andLeft htype),
    Nat.le_of_ble_eq_true (Reflect.andRight htype)⟩

/-- Negative evidence: a certificate missing the initial state is rejected. -/
theorem closure_without_init_rejected :
    system.checkClosure jugInvariant reachableClosure.tail = false := by decide

/-- Negative evidence: a certificate missing a successor of one of its own
members is rejected. -/
theorem closure_not_closed_rejected :
    system.checkClosure jugInvariant reachableClosure.dropLast = false := by decide

/-- Negative evidence: a *closed* certificate whose members violate the property
is rejected. Adding the unreachable state `(1,1)` keeps the set closed but breaks
the measurement invariant. -/
theorem closure_unsafe_member_rejected :
    system.checkClosure jugInvariant (reachableClosure ++ [⟨1, 1⟩]) = false := by decide

/-- Positive evidence: the recorded `solution` is accepted as a counterexample
path certificate for "the big jug never holds four gallons". -/
theorem four_gallons_witness :
    system.checkCounterexample (fun s => decide (s.big = 4)) solution = true := by decide

/-- RFC 0012 rung T0 (counterexample path validity), instantiated: the accepted
witness proves that a state with four gallons in the big jug is reachable. -/
theorem four_gallons_reachable :
    ∃ s : State, system.toTransitionSystem.Reachable s ∧ s.big = 4 := by
  obtain ⟨s, hr, hb⟩ :=
    Continuum.FiniteSystem.checkCounterexample_sound four_gallons_witness
  exact ⟨s, hr, of_decide_eq_true hb⟩

/-- Negative evidence: a path that skips intermediate states is rejected. -/
theorem skipping_path_rejected :
    system.checkCounterexample (fun s => decide (s.big = 4)) [⟨0, 0⟩, ⟨4, 3⟩] = false := by
  decide

end Continuum.Examples.DieHard
