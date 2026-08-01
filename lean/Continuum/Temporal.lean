import Continuum.Semantics

namespace Continuum

universe u

abbrev Behavior (State : Type u) := Nat → State

def Always {State : Type u} (p : State → Prop) (b : Behavior State) : Prop :=
  ∀ n, p (b n)

def Eventually {State : Type u} (p : State → Prop) (b : Behavior State) : Prop :=
  ∃ n, p (b n)

def InfinitelyOften {State : Type u} (p : State → Prop) (b : Behavior State) : Prop :=
  ∀ n, ∃ m, n ≤ m ∧ p (b m)

def ContinuouslyEnabled {State : Type u}
    (enabled : State → Prop) (b : Behavior State) : Prop :=
  ∃ n, ∀ m, n ≤ m → enabled (b m)

/-- Extensional stuttering at a particular step. -/
def StuttersAt {State : Type u} (b : Behavior State) (n : Nat) : Prop :=
  b n = b (n + 1)

namespace TransitionSystem

variable {State : Type u} (sys : TransitionSystem State)

/-- `b` is a run of `sys`: it starts in an initial state and every consecutive
pair is a transition. -/
def IsRun (b : Behavior State) : Prop :=
  sys.init (b 0) ∧ ∀ n, sys.step (b n) (b (n + 1))

/-- `b` is a stuttering run of `sys`: every consecutive pair either repeats the
state or is a transition. Stuttering runs are the behaviors that survive
refinement, which is why the liveness conditions of RFC 0012 rung T1 are stated
over them. -/
def IsStutterRun (b : Behavior State) : Prop :=
  sys.init (b 0) ∧ ∀ n, b n = b (n + 1) ∨ sys.step (b n) (b (n + 1))

theorem isRun_toStutterRun {b : Behavior State} (h : sys.IsRun b) :
    sys.IsStutterRun b :=
  ⟨h.1, fun n => Or.inr (h.2 n)⟩

/-- RFC 0012 rung T0 (reachability induction), behavioral form. Every state
visited by a stuttering run is reachable. -/
theorem isStutterRun_reachable
    {b : Behavior State} (h : sys.IsStutterRun b) :
    ∀ n, sys.Reachable (b n) := by
  intro n
  induction n with
  | zero => exact .init h.1
  | succ k ih =>
      rcases h.2 k with heq | hstep
      · exact heq ▸ ih
      · exact ih.step hstep

/-- RFC 0012 rung T0 (reachability induction), behavioral form for runs. -/
theorem isRun_reachable
    {b : Behavior State} (h : sys.IsRun b) :
    ∀ n, sys.Reachable (b n) :=
  sys.isStutterRun_reachable (sys.isRun_toStutterRun h)

/-- RFC 0012 rung T0 (inductive invariant preservation), stated temporally: an
inductive invariant holds *always* along every stuttering run. -/
theorem always_of_inductiveInvariant
    {inv : State → Prop} (hinv : sys.InductiveInvariant inv)
    {b : Behavior State} (hb : sys.IsStutterRun b) :
    Always inv b :=
  fun n => sys.reachable_satisfies hinv (sys.isStutterRun_reachable hb n)

end TransitionSystem

/-! Fairness, Büchi/Streett acceptance, and lasso certificates are added under
RFC 0015 and RFC 0012 rung T2. -/
end Continuum
