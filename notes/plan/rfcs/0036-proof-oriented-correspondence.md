# RFC 0036: Proof-Oriented Model/Program Correspondence

## Status
Draft.

## Structure
Forward projection, optional reverse proposal, consistency relation, complement/provenance, ambiguity predicate, effects, obligations, evidence status.

## Reverse update
Returns zero/one/many candidate changes. Zero gives conflict; many gives alternatives and distinguishing obligations. It never chooses by hidden heuristic.

## Drift
Extraction changes trigger impacted correspondence edges and refinement obligations.

## Proof
Lean seed laws for simplified lenses; production correspondence uses translation validation and bounded refinement evidence.

## Acceptance
Unambiguous round-trip, ambiguous field aggregation, action split/merge, observer leakage, and stale map cases.
