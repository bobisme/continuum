# RFC 0019: Corpus Port Manifest and Evidence Schema

**Status:** Proposed  
**Target gate:** G0-Corpus

## Purpose

Make every semantic-equivalence claim inspectable, reproducible, and machine-checkable.

## Manifest shape

```toml
schema = "continuum-corpus-port/v1"
id = "TV-009"
title = "The Die Hard Problem"
source_epoch = "tla-examples@91c22ea..."
parity = "P2"
status = "passing"

[[source.module]]
path = "specifications/DieHard/DieHard.tla"
sha256 = "..."

[[source.model]]
config = "..."
mode = "exhaustive"
expected = "safety-failure"

[native]
model = "model.ctm"
config = "model.toml"
semantics_epoch = "cml/0.2"

[correspondence]
state = "state-map.cir-map"
actions = "action-map.toml"
stuttering = "collapse"

[[claim]]
id = "TypeOK"
kind = "invariant"
expected = "valid"
evidence = ["closure:sha256:..."]

[[claim]]
id = "NotSolved"
kind = "invariant"
expected = "invalid"
minimal_depth = 6
evidence = ["crashpack:sha256:..."]
```

## Required fields by parity

- P1: source, native model, correspondence, expected verdict.
- P2: finite comparison mode, normalized state/action map, oracle artifacts.
- P3: temporal/fairness map and liveness evidence.
- P4: Lean theorem names, axioms report, source theorem correspondence.
- P5: Rust package, binary hash, adapter coverage, refinement evidence.

## Status taxonomy

```text
untriaged
inventoried
porting
blocked-language
blocked-engine
blocked-proof
oracle-disagreement
passing
intentionally-divergent
unsupported
```

Only `passing` counts toward release parity. `intentionally-divergent` must explain why and does not satisfy the 1.0 contract without an explicit scope amendment.

## Generated dashboard

The corpus dashboard shows coverage by:

- semantic feature;
- parity level;
- backend;
- proof status;
- mutation score;
- runtime and memory;
- last successful semantic epoch;
- unresolved disagreement.
