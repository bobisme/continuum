# ADR 0040: Typed Context Packs

## Status
Accepted.

## Context
Raw traces and repository dumps overwhelm humans and agents. Unlabeled summaries may omit the actual cause.

## Decision
Continuum compiles bounded Context Packs from causal, proof, source, state, assumption, and correspondence graphs. Packs include guarantee class, omission manifest, expansion handles, replay/evidence references, and exact semantic identity.

## Consequences
- context compilation becomes a product subsystem;
- claims such as replay-preserving must be checked;
- compactness is optimized subject to faithfulness;
- packs support human and agent renderings.

## Evidence required
Replay-preservation checks, context ablations, human studies, and token/effectiveness benchmarks.
