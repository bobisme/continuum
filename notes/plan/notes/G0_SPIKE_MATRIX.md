# G0 Falsification Matrix — Revision 3

G0 exists to kill seductive architecture before it ossifies. A failed load-bearing spike blocks interface freeze.

| ID | Question | Required experiment | Pass condition | Failure consequence |
|---|---|---|---|---|
| G0-DX-01 | Can an agent diagnose from a bounded artifact rather than raw logs? | 200+ event noisy durability failure → Context Pack | causal core replay-preserving; ≥10× context reduction; exact expansion handles | redesign evidence model |
| G0-DX-02 | Can Continuum detect formal reward hacking? | patches that weaken property, strengthen assumptions, reduce bounds, hide observer events, remove faults | all classified as intent changes; ordinary code guard repair is not | block autonomous repair |
| G0-DX-03 | Is all state explicit and resumable? | create snapshot, start bounded task, resume continuation, mutate workspace, reuse old handle | idempotence, stale snapshot rejection, deterministic resume | reject daemon protocol |
| G0-DX-04 | Can an exact counterexample be debugged causally? | branch before `Ack`/`Sync` race | enabled frontier and alternate branch reproduce expected safe/failing outcomes | DAP design remains experimental |
| G0-DX-05 | Does incrementality preserve clean semantics? | mutate property, model action, source function, proof lemma, docs | invalidation cone matches declared dependency classes; incremental result equals clean result | disable affected reuse class |
| G0-DX-06 | Can a repair transaction resist overfitting? | patch exact trace only, then run neighborhood/mutation campaign | narrow repair rejected; semantic repair promoted | no autonomous promotion |
| G0-DX-07 | Can Forge synthesize under non-vacuity? | grammar of acknowledgement guards with safety + progress | returns `synced`, rejects `false`/never-ack | redesign objectives/CEGIS loop |
| G0-DX-08 | Can model/program sync admit ambiguity? | concrete field maps to multiple abstract summaries | returns typed conflict/obligations; never silently chooses | remove bidirectional editing |
| G0-DX-09 | Can humans understand explanations better than raw traces? | task-based study with experienced Rust engineers | higher diagnosis accuracy and lower time; no assurance miscalibration | redesign explanation UX |
| G0-DX-10 | Can agents use the protocol more effectively than CLI scraping? | same benchmark with native API vs shell output | higher success, lower tokens, fewer invalid actions | simplify/rework ACI |
| G0-DX-11 | Can proof service isolate versions and requests? | concurrent Lean epochs, malicious inputs, cancellation | deterministic isolated result and resource bounds | no remote proof lane |
| G0-DX-12 | Can intent remain stable across swarm work? | planner, repairer, prover, reviewer with adversarial patch | evidence graph catches every unauthorized intent edge | block multi-agent autonomy |
| G0-DX-13 | Does content addressing survive concurrency? | concurrent identical task creation and artifact publication | one semantic identity; no lost receipts; deterministic result | redesign CAS/task transaction |
| G0-DX-14 | Can cancellation of verification work close correctly? | cancel DPOR, solver, proof, and synthesis tasks at every phase | no leaked obligations; resumable artifacts either committed or absent | integrate deeper with asupersync |
| G0-DX-15 | Can the corpus generate agent tasks without leakage? | hidden mutation splits and source-hash isolation | no answer leakage; deterministic grading; held-out behavior variants | benchmark invalid |

## Promotion rule

A spike may become architecture only when it has:

1. a precise semantic claim;
2. a reference implementation or checker;
3. adversarial mutations;
4. deterministic artifacts;
5. a baseline;
6. a failure policy;
7. a path to independent evidence.

A visually impressive demo is not a pass condition.
