# RFC 0018: Agent-Native Proof and Repair Loop

**Status:** Proposed  
**Target gates:** G2 onward (Revision 2 scheme, docs/26; Rev-2 G2 ≈ Rev-3 G4 per plan §24 — not citable without translation to the docs/52 Revision 3 gates per plan §22)

## Principle

Agents are powerful search and explanation systems. They are not trusted oracles. Continuum exposes structured tasks and checks every result.

## Machine-facing artifact graph

```text
claim
 ├─ model/version
 ├─ assumptions
 ├─ property AST
 ├─ search result
 ├─ counterexample/certificate
 ├─ source correspondence
 ├─ proof obligations
 └─ reproduction commands
```

Every node has a stable schema and content hash.

## Agent operations

- translate prose/design into a draft model;
- port a TLA+ corpus case;
- propose invariants, lemmas, rankings, cutoffs, observers, and abstraction maps;
- classify counterexamples as model/implementation/assumption defects;
- minimize semantic changes;
- propose Rust fixes;
- generate Lean proof attempts;
- search the corpus for analogous proof patterns;
- explain evidence at multiple abstraction levels.

## Acceptance pipeline

A proposed repair passes only when:

1. original failure replays;
2. patch changes the intended semantic slice;
3. exact failure no longer occurs;
4. neighboring causal schedules/faults are explored;
5. mutation suite remains sensitive;
6. assumptions/properties did not weaken unexpectedly;
7. required certificate/Lean theorem checks;
8. production/runtime compatibility tests pass.

## Counterexample API

Agents receive a minimized causal explanation rather than megabytes of logs:

```json
{
  "violated": "AckImpliesDurable",
  "causal_core": ["reserve#18", "cancel#20", "ack#22", "crash#23"],
  "missing_happens_before": ["sync#21 -> ack#22"],
  "abstract_delta": {"acked": "+(n,e,v)", "durable": "unchanged"},
  "assumptions_used": ["A-NET-03"],
  "replay": "continuum replay cp:..."
}
```

## Proof search

Lean goals are emitted with:

- relevant definitions unfolded;
- proof slice/dependency graph;
- candidate lemmas from corpus/search;
- countermodels to failed induction;
- finite-instance evidence marked as heuristic;
- no hidden axioms.

Agent-generated proofs are accepted only by Lean's kernel.

## Security

Agent execution is sandboxed. Model/compiler/proof inputs are untrusted. Agents cannot mark claims `PROVEN`, rewrite evidence ledgers, or weaken policies without an explicit diff and review.
