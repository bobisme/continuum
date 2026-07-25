# RFC 0015: Temporal, Fairness, and Hyperproperty Semantics

**Status:** Proposed  
**Target gates:** G3–G4

## Core behavior model

A behavior is an infinite state/event execution. Finite executions are completed according to an explicit policy:

- stutter forever at terminal state;
- deadlock violation;
- finite-trace property only;
- environment-closed completion.

No engine silently changes this policy.

## Temporal language

The initial temporal core supports:

- `always P`;
- `eventually P`;
- `P leads_to Q`;
- recurrence and persistence;
- action occurrence/enabledness;
- weak and strong fairness;
- bounded metric operators under the timed extension.

The theorem-preserving core is LTL without `next`, because stuttering refinement is central. `next` is allowed only under an observation granularity that makes each step semantically fixed.

## Fairness

Fairness is attached to named action schemas plus a view of variables/events. Each result lists:

- scheduler fairness;
- action weak/strong fairness;
- network delivery fairness;
- crash/recovery restrictions;
- cancellation checkpoint responsiveness;
- environment/client assumptions;
- timer/clock progress.

Fairness monitors become part of CIR and reduction dependence.

## Model checking

Finite models use automata-theoretic checking:

1. translate property to a Büchi/generalized Büchi/Streett monitor;
2. construct product on demand;
3. find accepting SCCs/fair lassos;
4. minimize stem and loop under semantic constraints;
5. emit SCC/lasso certificate.

## Ranking liveness

For parameterized or symbolic models, Continuum supports proof obligations built from:

- well-founded rankings;
- lexicographic/multiset orders;
- helpful transitions;
- justice/compassion premises;
- modular progress lemmas;
- proof graphs/slices.

Agents may propose rankings. Solvers may synthesize coefficients. Lean or a checked certificate validates the final argument.

## Hyperproperties

Some important claims compare executions:

- noninterference;
- observational determinism;
- linearizability/refinement;
- fault transparency;
- constant-time or secrecy observers.

Continuum represents these with explicit product/self-composition or automata over tuples of executions. Strong refinement is required where trace inclusion is insufficient.

## Runtime limitation

Incomplete asynchronous production observations can make properties undecidable. Runtime conformance therefore uses four-valued evidence:

```text
Valid | Invalid | Inconclusive | SemanticsMismatch
```

`Inconclusive` is not failure and never becomes `Valid` through timeout.

## Assumption provenance

Every liveness theorem contains an assumption object with stable IDs and a partial order of strength. Adding a fairness assumption changes the claim identity.

## Corpus requirements

P3 ports must include at least one accepted fair behavior, one unfair behavior excluded by assumptions, and a deliberate liveness mutation. EWD termination cases, mutual exclusion, consensus, and Streamlet-like external cases form the core suite.
