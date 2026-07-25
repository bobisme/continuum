# ADR 0031: Corpus-Derived Language Governance

## Status

Accepted for revision 2.

## Context

A new formal language can accumulate elegant features without proving practical completeness. Continuum claims it can replace ordinary TLA+ usage in concurrent and distributed Rust projects.

## Decision

The pinned 80-family TLA+ Examples validated corpus governs CML and engine priorities. Every language feature proposal identifies the corpus families and real-system use cases it unlocks. Continuum 1.0 requires all validated families at their declared parity level.

Corpus pressure does not prohibit features absent from TLA+. It prevents the project from declaring victory while missing known modeling patterns.

## Consequences

- corpus manifests and dashboards are release artifacts;
- semantic epochs include corpus compatibility changes;
- port-specific hacks are rejected;
- language ergonomics remain free to improve on TLA+ syntax;
- the extended 39-family set remains tracked but is not silently counted as validated.
