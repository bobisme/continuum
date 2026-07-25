# ADR-0013: Use exact canonical state identity in exhaustive and certified modes

**Status:** Accepted  
**Date:** 2026-07-24  
**Decision owners:** Continuum architecture group

## Context

Hash compaction and fingerprints are fast but collisions can suppress states. A system promising stronger assurance should not depend on collision improbability.

## Decision

Canonical structural encodings define identity. Hashes index and partition; collisions resolve by exact comparison. Artifact digests use cryptographic hashes, but proof validity still derives from decoded structure.

## Consequences

Memory/time may increase relative to fingerprint-only systems. Delta encoding, interning, compression, and batch processing recover performance.

## Alternatives considered

1. 64-bit fingerprints: rejected for certified modes.
2. 256-bit hashes as identity: extremely safe but still an assumption; acceptable only for non-certified modes if labeled.
3. Full object comparison without canonicalization: nondeterministic and slow.

## Validation and rollback

Benchmark exact overhead. If too high, support clearly labeled probabilistic-identity mode while retaining exact certified mode.
