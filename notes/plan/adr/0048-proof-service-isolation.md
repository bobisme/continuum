# ADR 0048: Proof Service Isolation

## Status
Accepted.

## Context
Lean environments are version-sensitive and agent-generated proof inputs are untrusted. Large proof workflows need concurrency and cancellation.

## Decision
Run proof operations in isolated workers keyed by exact Lean/library/source closure. Workers expose typed goal/proof operations, resource bounds, deterministic checking, axiom manifests, and receipts. Only kernel-checked results reach Proved.

## Consequences
- local and remote worker implementations;
- proof context compilation matters;
- environment changes invalidate receipts;
- worker output is untrusted until validated.

## Evidence required
Multi-version isolation, malicious inputs, cancellation, and receipt replay.
