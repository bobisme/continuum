# RFC 0008: Liveness, Fairness, and Progress

**Status:** Proposed  
**Target gate:** G4 (Revision 2 scheme, docs/26 — not citable without translation to the docs/52 Revision 3 gates per plan §22)

## Summary

Continuum SHALL treat liveness assumptions as explicit semantic objects. Finite randomized testing is never labeled a liveness proof. Cancellation progress, recovery, delivery, scheduler fairness, and timing assumptions are composed and reported separately.

## Property model

Core temporal properties include:

- `eventually P`;
- `always P`;
- `P leads_to Q`;
- recurrence/persistence;
- response and stabilization;
- weak/strong fairness of named actions;
- bounded progress under time assumptions.

The internal representation may compile LTL-like syntax to automata, ranking obligations, or liveness-to-safety transformations.

## Fairness scopes

Fairness is attached to action schemas and enabling predicates:

```text
weak_fair(Action): continuously enabled ⇒ eventually taken
strong_fair(Action): enabled infinitely often ⇒ taken infinitely often
```

Distributed assumptions are separate:

- eventual message delivery;
- eventual partition healing;
- eventual stable leader;
- bounded clock drift after GST;
- finite crashes;
- fair task polling;
- cooperative cancellation checkpoints.

The result prints them as assumptions, never as implicit defaults.

## Cancellation progress

Asupersync's request → drain → finalize lifecycle becomes a progress protocol. Candidate rankings include:

\[
R = (
  \#\text{live descendants},
  \#\text{unresolved obligations},
  \text{remaining cleanup budget},
  \#\text{pending effect phases}
)
\]

with a lexicographic or multiset order. A proof must justify decreases under fair scheduling and identify non-cooperative foreign calls as assumptions or unsupported paths.

## Verification lanes

### Finite graph

Build the product with a Büchi/parity monitor, find accepting SCCs, and account for fairness. Counterexamples are lassos or partial-order fair cycles.

### Liveness to safety

Use prophecy/history/ranking instrumentation and safety engines where sound. The certificate records the transformation.

### Ranking synthesis

Use templates, ICE/CEGIS, Horn clauses, and human hints to synthesize ranking functions or transition invariants.

### Compositional progress

Components expose progress measures and rely/guarantee conditions. Composition checks that circular waiting assumptions are discharged rather than mutually assumed.

## Partial-order reduction

A liveness reduction must preserve relevant cycles and fairness. Property-observer footprints contribute to dependence. The safety DPOR configuration is never reused by default.

## Time

Bounded response is a timed property, not ordinary liveness. Virtual time and production clocks map to a common constraint semantics, with uncertainty and fairness explicitly separated.

## Diagnostics

A liveness failure should classify:

- genuine fair cycle;
- unfair scheduler artifact;
- violated environment assumption;
- cancellation futurelock/obligation leak;
- insufficient bound;
- unknown due to abstraction.

The explanation includes the recurring causal core and the smallest set of fairness assumptions needed to eliminate it.

## Success criteria

G4 requires:

- safety/liveness distinction in result schemas;
- weak/strong fairness semantics tested against TLC/another oracle on a corpus;
- at least one distributed progress proof;
- at least one cancellation-drain ranking certificate;
- liveness counterexample replay;
- no hidden fairness defaults.

## Rejected alternatives

- “No failure after N simulated hours” as proof.
- Global scheduler fairness with no action-level semantics.
- Timeouts as a generic substitute for progress.
- Assuming every cancellation-aware task eventually cooperates.
