# ADR 0035: Strong Claims Emit Proof Receipts and Axiom Manifests

## Status

Accepted.

## Context

A certificate file or Lean theorem name alone is insufficient to reproduce the trust chain. Tool versions, encodings, transformations, assumptions, and axioms matter.

## Decision

Every strongest-assurance result emits a proof receipt containing semantic/proof epochs, closure hashes, transformation chain, certificate and checker hashes, theorem names, imports, and machine-captured axiom output. Receipts are content-addressed and independently checkable.

## Consequences

- proof provenance becomes a stable API;
- claims survive engine replacement when semantics and receipts remain valid;
- undocumented axioms fail release gates;
- receipts may be signed but signatures do not replace proof checking.
