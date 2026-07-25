# RFC 0035: Isolated Lean Proof Service

## Status
Draft.

## Worker key
Lean version, library closure, source snapshot, options, resource policy.

## Operations
Strict check, goal extraction, proof-state step, metadata/axioms, lemma extraction, deterministic rewrite/repair, receipt.

## Isolation
No ambient network/credentials; bounded CPU/memory/output; per-request workspace; cancel/hard-kill; canonical diagnostics.

## Context Pack
The service accepts exact goal plus curated declarations/proof slice and can return missing-dependency requests.

## Receipt
Statement/environment/proof hashes, theorem, imports, axiom output, kernel/tool version, worker artifact.

## Acceptance
Multi-version concurrency, malicious source, cancellation, deterministic replay, and zero unapproved axioms for seed theorems.
