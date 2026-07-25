# Revision 2 Changelog

**Date:** 2026-07-24  
**Base:** original Continuum dossier  
**Revision thesis:** the project is governed by an external semantic corpus and an independent Lean proof plane, not by internal architectural elegance.

## Structural changes

- Replaced the CIR-centric architecture with the **Model / Program / Proof semantic triptych**.
- Made the pinned `tlaplus/Examples` CI-validated corpus a Continuum 1.0 release contract.
- Added explicit parity levels P0–P5 and six feature-driven porting waves.
- Added a stratified model language with finite, symbolic, temporal, probabilistic, theorem, and runtime fragments.
- Elevated Lean 4 from a future audit tool to the metatheory and certificate-authority plane.
- Required independent semantic paths: reference evaluator, optimized engines, runtime adapter, foreign oracles, and Lean checkers may not certify themselves by shared implementation agreement.
- Added proof receipts with semantic epoch, proof epoch, assumptions, certificate digest, theorem names, and axiom manifest.
- Added a separate weak-memory execution-graph lane below protocol semantics.
- Added checked global-to-local projection and generated Rust interfaces as an optional implementation discipline.

## New corpus program

- Inventoried 80 CI-validated TLA+ example families and 39 extended/external examples.
- Added category, proof, PlusCal, TLC, Apalache, parity, porting-wave, and runtime-refinement metadata.
- Added the corpus policy, feature census, parity contract, porting plan, oracle protocol, and port manifest schema.
- Added design ports for Die Hard and Dining Philosophers.
- Added a corpus inventory checker and local-clone ingestion scaffold.

## New executable evidence

- Exact finite-state reference kernel with closure certificates and an independent checker.
- Die Hard: 16 states, 96 labeled transitions, shortest solution depth 6.
- Dining Philosophers(5): 573 states, 2,365 transitions, shortest deadlock depth 10.
- Cyclic symmetry quotient: 573 to 117 representatives with exhaustive automorphism validation.
- Stuttering-refinement spike for reserve/commit/abort semantics.
- Observer-indexed independence and observer-monotonicity spike.
- Fair-lasso spike separating unfair stuttering from genuine liveness failure.
- Safety-game spike synthesizing the minimal missing environment contract: forbid `AckBeforeSync`.
- Nominal canonicalization and semiring-valued traversal spikes.

## New Lean seed

- Transition systems, reachability, and inductive invariants.
- Finite closure-certificate soundness.
- Stuttering simulations and reachability preservation.
- Event-structure configurations.
- Observer-indexed commutation monotonicity.
- Symmetry automorphisms preserving reachability.
- Cancellation lifecycle rank decrease.
- Weak fairness primitives and proof-receipt identity policy.
- Concrete Die Hard solution witness.

The Lean source is intentionally `sorry`-free but was not kernel-checked in the artifact environment because no Lean toolchain was available. CI compilation and `#print axioms` capture remain release gates.

## New normative material

- ADRs 0021–0035.
- RFCs 0011–0025.
- Research programs 11–24.
- Documents 22–32.
- Corpus-port and proof-receipt JSON Schemas.
- Revised roadmap, G0 falsification matrix, first 25 pull requests, claims matrix, threat model additions, and kill criteria.
