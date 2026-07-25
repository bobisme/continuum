# ADR-0018: Version semantics independently from APIs and artifact formats

**Status:** Accepted  
**Date:** 2026-07-24  
**Decision owners:** Continuum architecture group

## Context

Semver of crates does not identify the meaning of an old model or trace. Replay can silently change under new pack/runtime semantics.

## Decision

Pin distinct model-language, semantic epoch, CIR, certificate, crashpack, and pack versions. Every evidence artifact binds all digests. Breaking semantics cause refusal or explicit migration.

## Consequences

Long-lived replay and evidence become possible. Version management and migration tooling are mandatory.

## Alternatives considered

1. Use crate semver only: insufficient.
2. Best-effort backward compatibility: dangerous for evidence.
3. Freeze semantics permanently: unrealistic before maturity.

## Validation and rollback

G0 schemas include all version fields. Release tests replay a corpus from prior supported epochs.
