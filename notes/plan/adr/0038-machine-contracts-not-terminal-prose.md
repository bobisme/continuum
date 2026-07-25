# ADR 0038: Machine Contracts, Not Terminal Prose

## Status
Accepted.

## Context
Agents scraping human output are brittle, expensive, and prone to misclassification. Human text also changes more frequently than semantic contracts.

## Decision
Every semantic operation has a versioned typed request/result schema. Terminal and UI prose are projections. Exit codes, JSON/CBOR, errors, omissions, epochs, and valid next actions are stable contracts.

## Consequences
- additional schema/versioning discipline;
- generated clients become feasible;
- UI copy may evolve independently;
- agents cannot rely on undocumented internal fields.

## Evidence required
Golden protocol traces, schema compatibility, fuzzing, and ACI-vs-shell benchmark.
