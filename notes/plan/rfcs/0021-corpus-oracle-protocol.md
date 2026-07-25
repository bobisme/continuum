# RFC 0021: Corpus Oracle Protocol

## Summary

Define the reproducible protocol by which native Continuum ports are compared with pinned TLC, Apalache, PlusCal, and TLAPS artifacts without making those tools runtime dependencies.

## Required artifacts

- source repository/commit/path hashes;
- toolchain container or lock manifest;
- model configuration and expected result;
- normalized initial/successor/reachable-state facts;
- ITF/CIR trace correspondence;
- semantic relation used for comparison;
- command, exit status, stdout/stderr hashes;
- timeout/resource classification.

## Comparison modes

1. exact finite graph;
2. bounded relational successor sampling;
3. shortest failure;
4. liveness/fair-lasso shape;
5. proof theorem intent;
6. mutation agreement.

## Security

Foreign tools execute in a sandbox with no network and bounded resources. Oracle output is untrusted data parsed by hardened adapters.
