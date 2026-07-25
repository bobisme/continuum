# Proof Obligation Catalog

This catalog defines stable obligation classes. Diagnostics and certificates refer to these IDs.

## Model well-formedness

### PO-MOD-001 — Initial satisfiability

There exists at least one state satisfying `Init`, unless emptiness is explicitly expected.

### PO-MOD-002 — Action total evaluation

For all well-typed inputs and states, action guard/postcondition evaluation terminates without semantic error.

### PO-MOD-003 — State closure

Every enabled action produces a state in the declared state domain.

### PO-MOD-004 — Deterministic operator semantics

Pure operator evaluation is independent of implementation iteration order.

## Event/CIR

### PO-CIR-001 — Acyclic causality

\(\leq\) is acyclic.

### PO-CIR-002 — Conflict heredity

If \(e \# f\) and \(f \leq g\), then \(e \# g\).

### PO-CIR-003 — Configuration validity

Accepted configurations are downward closed and conflict-free.

### PO-CIR-004 — Independent diamond

Declared independent enabled events commute in state and relevant observation.

### PO-CIR-005 — Canonical encoding

Semantically equal artifacts encode identically for a semantic epoch.

## Effects and cancellation

### PO-EFF-001 — Phase legality

Effect lifecycle transitions follow the pack state machine.

### PO-EFF-002 — Commit uniqueness

A reservation commits at most once.

### PO-EFF-003 — Abort invisibility

An aborted effect does not emit a committed observation.

### PO-CAN-001 — Obligation ownership

Every live obligation has exactly one owner.

### PO-CAN-002 — Transfer causality

Obligation transfer is causally ordered and preserves identity.

### PO-CAN-003 — Region close

Closed region implies no live child tasks, finalizers, or obligations.

### PO-CAN-004 — Cancellation progress

Under declared responsiveness/fairness, cancellation reaches terminal lifecycle state.

## Domain packs

### PO-PACK-001 — Choice completeness

Pack `choices` includes every behavior claimed by its fidelity profile.

### PO-PACK-002 — Independence conservatism

Events declared independent satisfy the diamond obligation for the applicable property class.

### PO-PACK-003 — Handler refinement

Production and Lab handlers refine the pack semantics under declared host assumptions.

### PO-PACK-004 — Fault closure

Fault events preserve pack state well-formedness.

## Safety

### PO-SAF-001 — Initiation

\(Init \Rightarrow Inv\).

### PO-SAF-002 — Consecution

\(Inv \land Step \Rightarrow Inv'\).

### PO-SAF-003 — Strength

\(Inv \Rightarrow Property\).

## Refinement

### PO-REF-001 — Initial mapping

Concrete initial states map to abstract initial states.

### PO-REF-002 — Visible-step simulation

Every visible concrete step maps to one or more allowed abstract steps.

### PO-REF-003 — Stuttering classification

Invisible concrete steps preserve abstract state/observation.

### PO-REF-004 — Fault mapping

Concrete faults map to allowed abstract environment steps.

### PO-REF-005 — Progress

Infinite concrete invisible behavior is excluded or matched according to the refinement class.

### PO-REF-006 — Hyperproperty preservation

The chosen simulation/refinement rule preserves the declared hyperproperty class.

## POR/unfolding

### PO-POR-001 — Backtracking coverage

Every unexplored dependent reorder has a covered backtracking/source-set representative.

### PO-POR-002 — Sleep-set soundness

Sleeping an event does not remove the sole representative of a relevant trace class.

### PO-UNF-001 — Prefix completeness

Every reachable marking/configuration has a represented equivalent in the finite prefix.

### PO-UNF-002 — Cutoff adequacy

Cutoff order satisfies completeness conditions.

## Liveness

### PO-LIV-001 — Fairness encoding

Acceptance condition exactly represents named fairness assumptions.

### PO-LIV-002 — Ranking well-foundedness

Ranking domain has no infinite descending chain.

### PO-LIV-003 — Ranking decrease

Relevant fair progress steps decrease ranking or discharge eventuality.

### PO-LIV-004 — No accepting SCC

Finite graph contains no reachable SCC satisfying liveness violation acceptance.

## Timed/probabilistic

### PO-TIME-001 — Zone soundness

Zone abstraction contains all concrete clock valuations.

### PO-TIME-002 — Time-elapse closure

Successor computation accounts for allowed delay.

### PO-PROB-001 — Distribution normalization

Probability distributions are nonnegative and sum to one.

### PO-PROB-002 — Scheduler quantification

Min/max probability claim uses the declared adversary/scheduler class.

### PO-PROB-003 — Fixed-point certificate

Provided value bounds satisfy Bellman inequalities and error bounds.

## Production trace

### PO-OBS-001 — Trace integrity

Local sequence/integrity constraints hold.

### PO-OBS-002 — Completion consistency

Inserted/unordered events respect causality and time intervals.

### PO-OBS-003 — Observation sufficiency

The property is monitorable under the instrumentation schema or result is inconclusive.

### PO-OBS-004 — Build/model identity

Trace, implementation, packs, and model digests match the claim.

## Certificate kernel

### PO-KER-001 — Decoder totality

All inputs yield a verdict/error without panic.

### PO-KER-002 — Resource bounds

Declared limits are enforced before allocation/recursion.

### PO-KER-003 — Evidence binding

Certificate is cryptographically/structurally bound to model, property, and assumptions.
