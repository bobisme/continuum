# Corpus Governance Policy

## Pinning

Each corpus epoch pins:

- upstream repository and commit;
- all submodule commits;
- exact TLA+ toolchain/TLC version;
- exact Apalache and TLAPS versions where used;
- expected model configurations and resource budgets;
- generated oracle artifacts and their hashes.

Upstream movement creates a proposed epoch. It does not mutate historical evidence.

## Upstream is an oracle, not a runtime dependency

Java/TLC, Apalache, and TLAPS may execute in the compatibility Tribunal. Continuum release binaries do not invoke them for ordinary verification.

This gives the project two benefits without circularity:

1. differential testing against the mature ecosystem;
2. native sovereignty for users and CI.

## Source provenance and licensing

A port records the original authors, source path, license, and transformation notes. Original TLA+ source is not copied into a generated artifact unless its license permits redistribution. External examples remain links plus independently authored native models when necessary.

Per-family license verdicts for the pinned commit are in [`REDISTRIBUTION_AUDIT.md`](REDISTRIBUTION_AUDIT.md), which is re-run on every corpus epoch: a pin change can change the answer.

## Oracle disagreement

Disagreement is classified before any side is called wrong:

- parser/elaboration divergence;
- value-semantics divergence;
- model-configuration divergence;
- state-projection mismatch;
- search-order-only difference;
- symmetry or fingerprint artifact;
- temporal/fairness interpretation mismatch;
- upstream defect;
- Continuum defect;
- intentionally unsupported extension.

Every disagreement becomes a minimized fixture.

## Metamorphic corpus generation

For each accepted port, the Tribunal produces semantics-preserving variants:

- alpha-renaming and declaration reordering;
- action factoring and inlining;
- equivalent set/function/sequence expressions;
- explicit versus implicit stuttering;
- module instantiation and constant substitution;
- symmetry-preserving identifier permutation;
- auxiliary-variable introduction/removal with projection;
- procedural versus relational formulation.

A result that survives only the original spelling is not semantic parity.

## Mutation contract

Every property must be shown non-vacuous by a relevant mutation:

- negate or weaken one guard;
- alter one update;
- remove one fairness condition;
- change one quorum threshold;
- move one durability boundary;
- reorder one cancellation phase;
- corrupt one refinement map.

The expected engine must reject or find the resulting defect. “Passes the original” without mutation sensitivity is weak evidence.
