# Research Note 01: True Concurrency as the Native Verification Object

**Claim class:** design hypothesis  
**Relevant sources:** [S15]–[S24], [S17], [S21]

## Thesis

Most model checkers linearize concurrent activity immediately and spend the rest of their lives recovering the independence they discarded. Continuum should invert that architecture: a finite partial-order execution is primary, while an interleaving is one linear extension used for execution or presentation.

This is not merely representational taste. It affects:

- reduction power;
- production-trace ingestion;
- compositionality;
- counterexample stability;
- fairness reasoning;
- certificate size;
- semantic debugging.

## Mathematical candidates

### Mazurkiewicz traces

Given an alphabet \(\Sigma\) and independence relation \(I\), quotient words by adjacent swaps of independent letters. This is the foundation of DPOR and a practical v0 model. Its weakness is that independence can depend on state, observers, faults, and effect phases.

### Prime/stable event structures

Events carry causal predecessors and conflict. Configurations are conflict-free downward-closed sets. They naturally represent branching and concurrency, but rich data may make conflict and enabling intensional.

### Occurrence nets and Petri-net unfoldings

Unfoldings separate causal histories and can produce finite complete prefixes under cutoffs. They offer strong reduction for asynchronous systems and a possible proof-carrying closure artifact.

### Interval pomsets

Operations in real systems extend over time. Interval pomsets can retain overlap without forcing an arbitrary linearization and align with production telemetry. Higher-dimensional automata research increasingly uses interval pomsets as accepted languages.

### Higher-dimensional automata

Independent transitions span cubes: two commuting actions form a square, three form a cube, and so on. Directed paths correspond to executions; directed homotopies identify schedules that differ only by deformation through independent operations. Recent Kleene and Myhill–Nerode results suggest automata/language minimization principles exist beyond interleavings.

## Proposed semantic tower

```text
CIR event structure
   ├── linear extension → executable replay
   ├── configuration graph → explicit-state model checking
   ├── occurrence prefix → unfolding engine
   ├── interval pomset → production conformance
   └── cubical complex → experimental high-dimensional reduction
```

Each projection has a checkable preservation obligation.

## Novel proposal: Causal Cubical Reduction

For a property observer \(O\), define an observer-relative independence relation \(I_O\). Build cubical cells for jointly enabled sets of pairwise independent events, but retain faces that interact with:

- property observations;
- fairness enabling;
- obligation flow;
- cancellation phases;
- time constraints;
- durability barriers.

Compute a directed quotient that preserves \(O\)-relevant reachability and selected cycle classes. The intended win is to collapse entire families of schedules with high concurrency dimension rather than discover pairwise swaps incrementally.

### Why this might work

Distributed runtimes often exhibit bursts of independent activity across nodes, shards, or keys. Pairwise DPOR records many races/backtracking choices even when a larger commuting family exists. A cubical representation makes the \(k\)-way independence explicit.

### Why it might fail

- Building cells may cost more than exploring schedules.
- State-dependent independence can fracture cubes.
- Directed homotopy preservation may be too weak for liveness.
- Rich data and faults can make the complex enormous.
- Certificate checking may become harder than the saved exploration.

### Falsification experiment

Corpus:

- independent per-shard requests;
- actor systems with mailbox-local work;
- replicated protocols with message fan-out;
- cancellation trees;
- key-value transactions with disjoint footprints;
- adversarial cases with mostly dependent events.

Compare against source-DPOR, optimal DPOR, parsimonious ODPOR, and unfolding prefixes. Report:

- maximal executions/relevant classes explored;
- event/cell count;
- wall time;
- peak memory;
- counterexample latency;
- certificate bytes;
- false independence defects found by mutation.

Kill the lane unless it yields at least an order-of-magnitude reduction on a non-artificial subset without a serious regression on dependent workloads.

## Ratified exploration-reduction threshold (plan §24.5, FR-02)

This note owns the plan §24.5 row *Exploration reduction (§9, INV-013)*.
The row joins two sources: the kill above (at least an order-of-magnitude
reduction on a non-artificial subset without a serious regression on
dependent workloads) and the docs/31 observer-indexed pair (median ≥5× on
the observer-sensitive class, checker overhead <20%, zero mutation loss).
The numbers and the denominator are fixed here. Plan §24.5 quotes the
sentence verbatim under the same `quote-id`.

quote-id=exploration-reduction-lane-gate "Reduced exploration replaces conservative unreduced exploration as the claim-bearing default only if, on a lane corpus whose workload membership is fixed before any measurement and whose unit of reduction is the explored maximal execution — every complete run the explorer executes to a configuration with no enabled transition or to the declared bound, sleep-set-blocked and redundant runs included, counted under identical bounds on both sides — both of the following hold for RFC 0014 tier A finite safety properties: first, against unreduced enumeration of every maximal execution, the median reduction ratio is at least 10 over a non-artificial subset of at least six workloads drawn from at least three of the five non-adversarial research/01 families, where non-artificial means a model of or a program in an existing protocol or runtime and never a parameterized synthetic generator, the median wall time over that subset with dependence-witness checking included is below the unreduced baseline's, and every workload of the research/01 adversarial mostly-dependent family stays within 20 percent of the unreduced baseline in wall time and in peak memory with dependence-witness checking included; second, against source-DPOR with the conservative dependence relation of RFC 0014, observer-indexed reduction reaches a median reduction ratio of at least 5 over an observer-sensitive class that contains at least the four RFC 0014 acceptance workloads, with witness-checking time below 20 percent of the observer-indexed search time on every workload of that class; and on every workload of both parts the reduced run returns the baseline's typed verdict, covers every observer-relevant equivalence class the baseline covers, and detects every mutant of the lane's false-independence mutation corpus that the baseline detects, so that one changed verdict or one missed mutant fails the lane."

**Denominator.** The register proposed "explored maximal-execution
classes". This note fixes the unit as the explored maximal *execution*,
and uses the class only as the coverage side condition. The reason is
arithmetic. Every sound reduction covers the same equivalence classes as
the unreduced baseline, so a ratio of classes is 1 for every sound
reducer and 1 for an unsound reducer that misses nothing on the corpus.
It cannot measure reduction. A ratio of executions measures the work
that reduction saves. The class count then makes sure that the saved
work did not lose coverage: the reduced run must cover every
observer-relevant class that its baseline covers. Sleep-set-blocked and
redundant runs are counted, because the explorer paid for them. Both
sides of a ratio use identical bounds, so a bound hit is the same event
on both sides and is reported as the typed bound result of
`continuum-engine-reference`, never as a completed run.

**Two baselines, two parts.** The first part is the lane-level bar. Its
baseline is the register fallback itself, unreduced enumeration, because
the question is whether reduction pays for leaving the fallback. The
second part is the docs/31 bar. Its baseline is source-DPOR with
conservative dependencies, the RFC 0014 product baseline, because docs/31
states the hypothesis as reduction *beyond conservative resource
conflicts*. The ratio is the same unit in both parts. The plan §24.5
cubical row is a third, separate bar (≥3× against optimal DPOR and
unfoldings), and neither of these parts replaces it.

**Readings chosen, and the conflicts recorded.** Where a source is
ambiguous or two sources disagree, this note takes the strictest reading.

1. *Serious regression.* research/01 gives no number. docs/07 §7 gives
   25 percent (the cubical non-regression clause), and docs/31 gives 20
   percent (checker overhead). This note takes 20 percent, the tighter
   figure, and applies it to every dependent workload, not to a median.
2. *Checker overhead.* docs/31 attaches "<20%" to a median ratio and does
   not say whether the overhead is a median too. This note applies it to
   every workload of the observer-sensitive class.
3. *Zero mutation loss.* docs/31 states it for the observer-indexed part.
   RFC 0014 requires a zero missed-bug mutation score for the certified
   tier. This note applies it to both parts.
4. *Conjunction.* The register row carries both sources in one threshold
   cell. This note reads that cell as one gate: reduced exploration leaves
   the fallback only when both parts hold. A reading that lets the first
   part promote alone is weaker. It is a privileged intent revision
   (INV-001, INV-011), not a ratification choice. Consequence: if the
   observer-indexed part fails or is killed, the whole lane stays on the
   fallback. RFC 0004 and RFC 0014 name conservative source-DPOR as
   Version 0 and as the product baseline. Under this gate, that engine can
   run, but no claim can rely on its reduction until the gate clears.
5. *Non-artificial subset.* research/01 says "a non-artificial subset"
   and does not fix its size. This note requires the subset to be fixed
   before any measurement, with at least six workloads from at least
   three families. A subset that is selected after the results can make
   any reducer pass.
6. *Scope.* research/13 puts fairness and liveness out of scope and
   defers them to the liveness-preserving reduction lane. This gate
   licenses reduction for RFC 0014 tier A only. Tiers B to D keep the
   fallback until their own lane promotes.

**Rationale for the numbers.** The 10 is research/01's own "order of
magnitude". The 5 and the 20 percent are docs/31's own numbers. The
median, not the minimum, is the aggregate for both ratios, because the
families differ widely in their degree of independence and one
nearly-dependent real workload must not veto the lane. The per-workload
floors on dependent workloads, witness checking, verdicts and mutants
are the cost of that choice: a median can hide a cost regression or a
soundness defect, so those are graded on every workload. The median wall
time conjunct stops a win that exists only in execution counts while
the time per execution grows.

**Scope and dependency.** The lane corpus, the false-independence
mutation corpus, and the observer-indexed engine do not exist today.
`continuum-engine-dpor` and `continuum-observer` are PR-1 scaffolds. The
ratified threshold binds when they land. Nothing is measured against it
until then.

### Fallback: conservative unreduced exploration

The fallback is stated and reachable today. `continuum-engine-reference`
(PR 8) is exhaustive breadth-first exploration with no reduction. It
checks invariants and deadlocks under declared bounds, reports bound
exhaustion as a typed result, and writes a finite closure certificate
that `continuum-certificate` checks from its wire form. It is also the
unreduced differential oracle that `continuum-engine-dpor` is audited
against (docs/33). Every exploration claim in the product runs on this
fallback now. The fallback costs scale: unreduced exploration can be
intractable on large corpora. Plan §24 states the program-level
consequence for liveness. For safety, a claim on the fallback reports a
bound hit as `Inconclusive` with its typed reason, never as success
(INV-008).

### Kill criteria for this row

- **Lane kill (research/01).** The first part fails on the lane corpus:
  the median is below 10, or a dependent workload regresses by more than
  20 percent, or a verdict changes, or a mutant is missed. The lane is
  killed and the fallback stays.
- **Observer-indexed kill (docs/31, research/13).** Witness cost
  dominates: with witness checking included, observer-indexed reduction
  is slower than the conservative source-DPOR baseline at the median of
  the observer-sensitive class. Or observer and property changes
  invalidate cached independence so often that reuse never pays for its
  witness cost: over a recorded sequence of property edits, the witness
  cost of recomputation is more than the exploration that reuse saved.
  Observer-indexed reduction is then default-off, and by reading 4 above
  the lane stays on the fallback.
- The research/13 kill criteria apply unchanged to the observer-indexed
  part.

## Novel proposal: Observer-Sensitive Independence

Traditional dependence asks whether transitions commute in full concrete state. Continuum properties often observe only a projection. Define independence relative to:

\[
\mathsf{Obs}_P = \text{property state} \cup \text{fairness enabling}
\cup \text{assumption monitors}.
\]

Events may be equivalent for one property and dependent for another. This can produce property-directed reduction while remaining sound if the observation abstraction is complete.

The hard part is generating and checking the property footprint. A certificate should include a proof that swapping the events preserves the observer projection and future enabled observer-relevant behavior.

## Counterexample geometry

A causal counterexample should be a minimal bad configuration, not merely a short word. Minimization objectives can include:

- event count;
- number of owners/nodes;
- number of faults;
- causal width;
- obligation-flow complexity;
- observer-visible events;
- semantic edit distance to a passing execution.

Computing a minimal causal core can use delta debugging over downward-closed configurations plus SAT/SMT constraints. Geodesic linearization then produces a readable replay with few owner switches.

## Topological coverage

Betti numbers or persistent homology over a commuting-diamond/cubical complex may indicate unexplored concurrency structure. This is a heuristic, not assurance. It may help schedule selection by prioritizing traces that add new cycles or fill unobserved cells.

The benchmark must compare it against simpler novelty metrics: event-pair coverage, happens-before edge coverage, state novelty, and random/PCT scheduling. If topology does not improve bug yield per CPU, delete it.

## Deliverables

1. Formal CIR-to-event-structure semantics.
2. Reference enumerator for tiny event structures.
3. Source-DPOR over CIR.
4. Unfolding prototype.
5. Cubical prototype behind an experimental feature.
6. Cross-engine counterexample equivalence tests.
7. Certificate format for partial-order closure.
