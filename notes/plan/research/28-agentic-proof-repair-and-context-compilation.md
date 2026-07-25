# Agentic Proof Repair and Context Compilation

## State of the art

Pantograph exposes machine-oriented Lean proof states. AXLE targets scalable isolated Lean utilities. OProver and LAMP report gains from compiler feedback, retrieved verified proofs, explicit domain context, and multi-agent decomposition. APOLLO and related proof-repair systems explore iterative repair after library/source evolution.

References:

- https://arxiv.org/abs/2410.16429
- https://arxiv.org/abs/2606.26442
- https://arxiv.org/abs/2605.17283
- https://arxiv.org/abs/2606.28841

## Continuum opportunity

Proof repair is coupled to model/program changes. Continuum can provide context unavailable to generic theorem provers:

- semantic diff;
- changed abstraction/refinement edge;
- finite countermodel to failed invariant;
- execution witness;
- proof dependency slice;
- corpus analogues;
- protected theorem statement/axioms.

## Proof Context Pack

```text
goal + local context
protected theorem statement
relevant definitions/theorems
proof dependency slice
changed semantic edges
failed attempts and Lean diagnostics
finite countermodels / induction failures
candidate auxiliary lemmas
allowed tactics/axioms
version and budget
```

## Novel proposal: proof blueprint graph

Represent a proof as semantic obligations rather than one tactic script:

```text
init establishes invariant
step action A preserves invariant
step action B preserves invariant
invariant implies property
ranking decreases on progress action
fairness discharges infinite stutter
```

Each node may have Lean theorem(s), solver certificate, or finite evidence. Agents repair the smallest invalid blueprint nodes. Presentation proof can be regenerated.

## Novel proposal: countermodel-directed lemma synthesis

When an induction attempt fails, extract model states/transitions satisfying current hypothesis but violating preservation. Synthesize predicates that separate them from reachable states, rank them by generalization/corpus analogues, and send as lemma candidates. Finite evidence is clearly heuristic until theorem proved.

## Novel proposal: version-parametric proof receipts

A proof receipt stores theorem statement/environment hashes and can be replayed across compatible toolchain epochs only after declaration-level semantic equality checks. This avoids both needless rebuild and unsafe “same source text” assumptions.

## Experiments

- changed invariant definition;
- action split requiring new case lemma;
- renamed/restructured source with same theorem;
- stronger property;
- changed abstraction;
- library upgrade;
- malicious theorem weakening;
- proof agent given full repo vs Context Pack.

## Metrics

kernel success, attempts, tokens, wall time, relevant lemma recall, invented theorem changes, axiom integrity, proof size, human review burden.

## Kill criteria

- Context Pack loses critical lemmas too often;
- proof-state API does not outperform source/LSP loop;
- automated repair frequently proposes theorem weakening;
- proof blueprint abstraction adds maintenance without improving localization/reuse.
