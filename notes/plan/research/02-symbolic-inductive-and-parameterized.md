# Research Note 02: Symbolic, Inductive, and Parameterized Verification

**Claim class:** implementation and research program  
**Relevant sources:** [S18]–[S20], [S25]–[S29], [S61], [S71A]

## Thesis

Explicit exploration is essential but not sufficient. Continuum should expose semantic structure once and support several infinite-state and parameterized reasoning lanes:

- decision diagrams and saturation;
- SMT bounded checking;
- CHC/PDR/IC3;
- symmetry-aware quantified induction;
- cutoff discovery;
- abstract interpretation and CEGAR;
- well-structured transition systems;
- compositional recomposition.

The frontier is not one magic solver. It is a portfolio with shared obligations, common counterexamples, and certificates.

## Partitioned transition interface

LTSmin's PINS architecture demonstrates the leverage of separating a language frontend from transition-group structure. Continuum should expose:

```text
state variables / symbolic fields
transition groups
read dependencies
may-write dependencies
must-write dependencies
guards
action labels
symmetry actions
```

CIR footprints give much of this information natively. This supports symbolic relational products, saturation, static dependency matrices, and compositional decomposition.

## Decision diagrams

For finite but huge structured state spaces:

- BDDs suit Boolean structure;
- MDDs suit finite-domain variables;
- saturation exploits locality by applying transition groups near their topmost affected variable;
- parallel decision-diagram packages provide multicore execution.

The variable-order problem is central. Continuum can derive candidate orders from causal/resource graphs, view dependencies, and separator decompositions. A learned selector may choose among explicit, MDD, SAT-BMC, and PDR lanes, but learned choice only affects performance, never soundness.

## SMT bounded model checking

Compile model/CIR transitions to formulas:

\[
Init(s_0)\land\bigwedge_{i<k}T(s_i,s_{i+1})
\land\neg P(s_k).
\]

Use incremental solving, symmetry-breaking, partial-order constraints, and unsat-core-guided bound refinement. For richer values, use arrays, algebraic datatypes, bitvectors, and finite sets conservatively.

An `unsat` at bound \(k\) is bounded evidence only unless transformed into induction.

## PDR/IC3 and CHCs

PDR incrementally constructs inductive clauses blocking bad states. For quantified distributed protocols, symmetry-aware generalization can lift finite-instance facts into first-order invariants. Continuum should:

1. extract a relational transition system from `.ctm`;
2. preserve sorts and symmetry groups;
3. run finite-instance PDR;
4. generalize clauses through orbit representatives and quantified templates;
5. validate the candidate invariant on larger instances and with an independent checker;
6. emit an inductive-invariant certificate.

Spacer/GSpacer-style global guidance can help avoid locally attractive but globally useless generalizations.

## Novel proposal: Orbit-Lifted Reachability Grammar

Recent cutoff work derives quantified reachability formulas from symmetry-aware finite exploration. Continuum can generalize this into a grammar:

- enumerate small instances modulo symmetry;
- synthesize a minimum description of reachable orbit patterns;
- infer quantification shapes and cardinality thresholds;
- test stabilization on the next sizes;
- use the formula as both an invariant candidate and a cutoff hypothesis;
- attempt induction over domain extension.

The output is never called a cutoff proof until induction/checking establishes it. Failed stabilization still yields useful predicates for CEGAR.

## WSTS lane

Some unbounded systems are monotone under a well-quasi-order:

- lossy channels;
- coverability abstractions;
- multisets of indistinguishable processes;
- monotone resource counts.

A domain/model can declare an order \(\preceq\), and Continuum checks monotonicity obligations. Upward-closed sets are represented by finite bases/ideals. This can prove coverability for unbounded populations where finite model checking cannot.

Rust implementations rarely satisfy monotonicity directly. The technique belongs primarily at an abstract view, with refinement carrying the result downward.

## Abstract interpretation

Every view can define a Galois connection or sound abstraction:

\[
\alpha : C \to A,\qquad \gamma : A \to \mathcal P(C).
\]

Continuum should support abstract domains for:

- intervals/congruences;
- cardinalities;
- set membership summaries;
- queue/channel shapes;
- ownership/obligation counts;
- epochs and monotone logs;
- topology/partition summaries.

The abstract interpreter can overapproximate reachability. Spurious counterexamples drive refinement. Proof obligations state sound transfer, not merely test agreement.

## Recomposition

Traditional component decomposition can lose cross-component relations. Recomposition-style analysis suggests dynamically grouping variables/components around the current property or counterexample. CIR resource hypergraphs offer a natural source for candidate partitions and separators.

Novel hypothesis: choose decomposition using a weighted hypergraph whose edges combine transition footprints, obligation transfer, and observer coupling. Recompose only the causal cone needed to refute/prove a property.

## Portfolio scheduling

A verification task emits static/dynamic features:

- domain sizes and symmetry;
- transition locality;
- estimated branching;
- queue bounds;
- formula theories;
- causal width;
- observed explicit-state growth;
- invariant vocabulary.

An algorithm selector allocates budget across engines. The selector's decision and fallback policy are recorded. Sound engines retain independent verdicts; a timeout is not a vote.

## Required experiments

- Paxos/Raft-style parameterized safety.
- Mutual exclusion/token protocols.
- Sharded key-value protocols with symmetry.
- Lossy-channel examples for WSTS.
- Storage recovery with unbounded log abstraction.
- Benchmarks where each engine is known to fail.
- Cross-validation against Apalache, TLC, Ivy/IC3PO-related artifacts, LTSmin, and standalone SMT solvers where practical.

## Kill criteria

- Quantified generalization that cannot be independently validated.
- Learned engine selection that does not beat a simple schedule on held-out tasks.
- WSTS support that requires users to encode more mathematics than a direct external tool.
- Symbolic encodings whose counterexamples cannot replay in the reference semantics.
