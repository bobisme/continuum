# RFC 0029: Causal Verification Debugger

## Status
Draft.

## Model
Debugger handle identifies immutable execution branch and selected causally closed configuration.

## API
`open`, `state`, `enabled`, `step_event`, `step_abstract`, `reverse_causal`, `branch`, `compare`, `why_enabled`, `why_blocked`, `export`.

## Frontier
Each enabled event includes label, owner, effect, guard derivation, observer impact, conflicts, and estimated branch novelty. Selection must be replay-checked.

## Reverse
Reverse removes one or a set of maximal events while preserving causal closure. Ambiguity returns choices.

## DAP
Define mapping and custom namespaced requests; native API remains normative.

## Acceptance
Known race branch, liveness cycle, abstract stuttering, fault insertion, and round-trip export/replay.
