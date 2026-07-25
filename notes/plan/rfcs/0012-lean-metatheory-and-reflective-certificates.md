# RFC 0012: Lean Metatheory and Reflective Certificates

**Status:** Proposed  
**Target gates:** G0-Proof, G2, G3, G4

## Goals

1. Give precise mathematical meaning to Continuum claims.
2. Keep optimized search engines outside the trusted base.
3. Validate large certificates efficiently through reflection.
4. Connect solver-level evidence to original model semantics with verified encodings.
5. Provide reusable theorems for corpus ports and real systems.

## Package split

```text
continuum-metatheory-core     Lean core/Std only
continuum-metatheory-mathlib  algebra, order, probability, topology
continuum-certificate-ffi     byte format and parser correspondence
continuum-corpus-proofs       TLA+ example theorem parity
continuum-asupersync-laws     concrete effect/cancellation contracts
```

## Foundational definitions

The first frozen definitions are deliberately conventional:

```lean
structure TransitionSystem (S) where
  init : S → Prop
  step : S → S → Prop

abbrev Behavior S := Nat → S
inductive Reachable ...
structure StutteringSimulation ...
```

CIR true-concurrency semantics are related to this interleaving projection rather than replacing it immediately. This permits early proofs with a familiar foundation while keeping the richer event-structure model available.

## The theorem ladder

### T0 — transition-system safety

- reachability induction;
- inductive invariant preservation;
- finite closure certificate soundness;
- counterexample path validity.

### T1 — stuttering and refinement

- stuttering simulation preserves reachable safety predicates;
- composition of simulations;
- observer projection and hidden-event closure;
- history/auxiliary variable erasure;
- conditions for liveness preservation.

### T2 — temporal semantics

- infinite behaviors and stuttering closure;
- LTL without next;
- weak/strong fairness definitions;
- Büchi/Streett acceptance equivalence;
- fair lasso/SCC certificate soundness;
- rank/compassion progress certificates.

### T3 — partial-order reduction

- event dependence and commutation diamonds;
- observer-indexed independence monotonicity;
- trace equivalence;
- source-set/DPOR coverage theorem for the supported finite fragment;
- certificate checker soundness.

### T4 — symmetry and parameterization

- finite group actions and orbit representatives;
- canonicalization preserves transitions/properties;
- nominal support/equivariance for atom fragments;
- cutoff/quantified-invariant certificate schemas.

### T5 — concrete execution

- cancellation phase and obligation state machine;
- reserve/commit/abort laws;
- region closure/quiescence contracts;
- asupersync event adapter refinement into CIR;
- selected protocol implementations refine abstract models.

## Certificate architecture

A certificate has three layers:

```text
wire bytes
  ↓ parser theorem
well-formed certificate value
  ↓ checker soundness theorem
semantic proposition
```

The parser is not trusted by assertion. Either:

- a verified parser is generated/implemented in Lean and extracted; or
- the Rust parser produces a canonical digest and a second Lean parser consumes the same bytes for proof import.

### Certificate families

- path/counterexample;
- finite closure;
- inductive invariant;
- simulation/refinement;
- symmetry quotient;
- DPOR coverage;
- fair SCC/lasso;
- ranking function;
- SAT LRAT;
- pseudo-Boolean VeriPB;
- SMT/Alethe or solver-specific proof;
- MDP/value-iteration bounds with rational enclosures.

## Reflection

Large certificates are checked by proved Boolean functions executed as native Lean code. The theorem shape is:

```lean
 theorem check_sound (input : Input) (cert : Cert) :
   check input cert = true → SemanticallyValid input
```

A successful reflected check yields a theorem without materializing every low-level inference as a giant proof term.

## Verified encodings

Solver certificates prove facts about encodings. Continuum must also prove:

```text
model semantics
  ↔ bounded transition formula
  ↔ bit-blasted/PB/CNF encoding
```

Encoding versions are semantic epochs. A solver proof without an encoding theorem is assurance-capped.

## Axiom policy

Every public theorem reports `#print axioms`. Permitted foundations are documented. `sorry`, `admit`, unchecked native axioms, and opaque foreign proof imports are forbidden in release proof packages.

## Build strategy

Lean proof compilation is not on the ordinary Rust edit loop. CI uses:

- cached `.olean` artifacts;
- declaration-granular targets;
- proof package partitioning;
- nightly full rebuilds;
- deterministic theorem manifests.

## Acceptance

The first accepted milestone imports a Rust-generated finite closure certificate for DieHard, proves `TypeOK` for all reachable states, rejects a malformed closure, and reproduces the same theorem under the pure Lean checker.
