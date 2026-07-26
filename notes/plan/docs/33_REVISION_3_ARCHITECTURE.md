# Revision 3 Architecture: Intent, Evidence, and Workbench

## Decision

Revision 3 changes Continuum from a verifier with agent features into an **intent-preserving verification workbench**. The semantic core remains independent of the interaction layer, but the interaction layer becomes a load-bearing correctness component.

```text
                     protected Intent Contract
                              │
             ┌────────────────┼────────────────┐
             ▼                ▼                ▼
          Model Plane     Program Plane      Proof Plane
             └────────────────┼────────────────┘
                              ▼
                         Evidence Plane
                              │
                              ▼
                        Workbench Plane
```

## Why the semantic triptych was insufficient

Model, program, and proof can agree for the wrong reason. An automated repair can:

- replace a strong property with a weak one;
- add a favorable fairness assumption;
- exclude the failing state through bounds or constraints;
- coarsen an observer until distinct bad states look equal;
- remove a fault from the environment;
- downgrade exhaustive evidence to simulation.

All three planes may then become “green.” The missing object is the user's intended claim and acceptable assurance.

## Protected intent

Intent is a typed immutable artifact. It names:

- claims and property formulas;
- observer scope;
- environment assumptions;
- fault and recovery envelope;
- timing/fairness conditions;
- bounds/cutoffs;
- trust boundaries;
- assurance requirements;
- hard/soft synthesis objectives;
- non-vacuity conditions;
- change policy.

Intent revisions are legitimate, but they are never hidden inside repairs. A semantic diff identifies the direction and impact of the change. Dependent evidence is invalidated.

## Evidence plane

The evidence plane normalizes results from heterogeneous engines into typed artifacts. It distinguishes:

```text
Observed         one or more concrete executions
Sampled          probabilistic/randomized campaign
Bounded          exhaustive/symbolic within a stated envelope
Validated        independent certificate/translation checker passed
Proved           Lean kernel accepted the theorem under named axioms
Refuted          valid counterexample or proof of inconsistency
Inconclusive     available evidence cannot decide the claim
```

These states are not totally ordered. A production observation and a bounded proof answer different questions. The assurance envelope records every relevant dimension.

## Workbench plane

The workbench is `continuumd` plus adapters. It provides:

- immutable snapshots and content addressing;
- incremental semantic queries;
- task and budget lifecycle;
- Context Packs;
- causal debugger;
- semantic/intent diff;
- repair transactions;
- synthesis/Forge;
- proof isolation;
- evidence graph;
- access control and audit.

## Architectural separation

The following separations are mandatory:

| Producer | Independent consumer/checker |
|---|---|
| optimized explicit explorer | closure/certificate checker |
| DPOR reducer | unreduced differential oracle + reduction witness checker |
| Rust extractor | translation validation/refinement checker |
| Forge/LLM candidate generator | verifier and proof pipeline |
| proof agent | Lean kernel |
| incremental query engine | Incremental Parity Audit |
| Context compiler | replay/property preservation checks |
| semantic synchronizer | correspondence-law checker |

Shared schemas are allowed. Shared decision logic that causes both sides to accept the same bug is not.

## Dataflow

```text
source/model/proof files
        │ snapshot
        ▼
immutable workspace DAG ─────── protected Intent Contract
        │                                  │
        ├── parse/elaborate/extract ───────┤
        │                                  ▼
        ├── model/program/proof graphs → verification portfolio
        │                                  │
        │                                  ▼
        └────────────────────────────── evidence graph
                                           │
          ┌─────────────┬──────────────────┼───────────────┐
          ▼             ▼                  ▼               ▼
      Context Pack   debugger       repair transaction    Forge
```

## Failure model

Continuum itself is a concurrent system. Workbench tasks use asupersync regions and obligations. Publication is two-phase:

```text
reserve artifact identity
  → compute/stream provisional evidence
  → validate references
  → commit artifact and ledger edge
```

Cancellation before commit leaves no authoritative artifact. Suspended search state is committed as a continuation with explicit partial-evidence status.

## Scaling model

The local daemon is the first architecture. Remote workers later consume immutable inputs and return candidate artifacts. The authority service validates and commits them. The design avoids shared mutable solver/session state and permits:

- local single-process development;
- isolated Lean/solver processes;
- parallel workers;
- remote proof farms;
- content-addressed caching;
- reproducible task handoff.

## Trust boundary

Trusted or independently checked components should converge toward:

- canonical value decoder;
- core transition/certificate semantics;
- proof-receipt verifier;
- Intent Contract parser/policy;
- content identity and authorization;
- Lean kernel/toolchain closure.

The daemon, search engines, UIs, agents, solvers, and optimizers remain outside the smallest trust base.

## Consequence

Revision 3 raises initial implementation cost. It also prevents the most dangerous failure mode: an autonomous system producing convincing formal evidence for a subtly altered question.
