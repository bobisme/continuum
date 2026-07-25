# Competitive Matrix

The matrix is directional, not a marketing score. Capabilities vary by version and model.

| Capability | TLA+/TLC | Quint/Apalache | P | Stateright | Loom/Shuttle | Verus/Creusot/Kani | FoundationDB-style DST | Continuum target |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| arbitrary abstract state | excellent | strong | constrained | Rust-shaped | poor | code-shaped | poor | excellent |
| model before code | yes | yes | yes | yes | no | no | no | yes |
| explicit safety | yes | yes/symbolic | yes | yes | schedules | local/code | sampled | yes |
| liveness/fairness | strong | partial/varies | monitors/proof | limited | limited | limited | sampled | strong target |
| parameterized proof | TLAPS/manual | inductive support | verifier | no | no | local proofs | no | restricted automated lanes |
| production code execution | no | Connect bridge | generated/observed | model code | near-real local | actual code | yes | yes |
| cancellation semantics | ad hoc model | ad hoc model | machine events | ad hoc | drop/schedule | code-level | project-specific | structural via asupersync |
| crash/durability semantics | modeled manually | modeled manually | modeled | modeled | poor | local | project-specific | standard packs |
| deterministic replay | TLC traces | traces | systematic tests | yes | yes | counterexamples | yes | compatibility surface |
| model/code refinement | manual/TLAPS | Connect/testing | integrated modes | manual | no | contracts | no | first-class |
| production partial-order validation | external | external | PObserve | no | no | no | logs | first-class target |
| proof certificates | TLAPS proofs | solver-dependent | verifier-dependent | no | no/solver | proof systems | no | core target |
| one native Rust toolchain | no | mixed | mixed | yes | yes | mixed | project-specific | yes baseline |
| typed assurance claims | no common schema | no common schema | no common schema | no | no | proof-specific | no | yes |
| domain-pack reuse | modules | modules/connect | libraries | Rust models | primitive wrappers | libraries | bespoke | explicit semantic packs |

## Where Continuum should not compete

- replacing interactive theorem provers;
- proving arbitrary mathematical theorems;
- full weak-memory semantics in the first releases;
- being the easiest first formal-methods tool before the core is stable;
- matching every TLA+ language feature;
- outperforming specialized timed/probabilistic tools on every workload.

## Defensible wedge

1. asupersync production and Lab integration;
2. cancellation/obligation semantics;
3. reusable network/storage/process packs;
4. causal counterexamples and replay;
5. standalone abstract models;
6. direct refinement into real Rust;
7. typed, certificate-backed assurance.
