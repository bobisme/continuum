# Corpus Porting Playbook

## 1. Pin and inventory

Record source commit, modules, configurations, imports, proof files, expected tool modes/results, and notable features.

## 2. State the semantic intent

Before translating syntax, write:

- state variables and mathematical domains;
- initial predicate;
- action relation;
- stuttering policy;
- safety/liveness properties;
- fairness assumptions;
- expected deliberate failures;
- abstraction/refinement structure.

## 3. Choose the CML fragment

Classify each operator/action/property as finite, symbolic, temporal, probabilistic, theorem, or runtime. Any unsupported combination becomes a tracked language issue.

## 4. Create value/state correspondence

Define canonical translation. Identify auxiliary variables, model atoms, function values, sequence indexing conventions, and symmetry.

## 5. Obtain upstream oracle facts

At minimum:

- verdict;
- generated/distinct state counts where meaningful;
- depth;
- counterexample trace;
- liveness loop;
- deadlock behavior.

For P2+, export graph/transition facts or enough traces for bisimulation/language comparison.

## 6. Build native model

Prefer semantic clarity over literal structure. Preserve named action boundaries when they matter for fairness, traces or proof correspondence.

## 7. Differential check

Run the native reference evaluator and upstream oracle. Minimize differences before optimizing anything.

## 8. Add metamorphisms and mutations

Prove the port is semantic and the properties are non-vacuous.

## 9. Add Lean theorem parity

For P4:

- state theorem correspondence;
- formalize bridge to executable model;
- prove or import certificate;
- record axioms.

## 10. Add runtime exemplar when selected

Implement using asupersync and domain packs, instrument semantic events, define view/refinement, explore schedules/faults, and replay failures.

## Review questions

- Did the port accidentally make an action atomic that was not?
- Did typing remove behaviors the TLA+ model allowed?
- Did finite bounds become source semantics?
- Are fairness clauses attached to equivalent actions?
- Does `CHOOSE`/selection have the same determinism/underspecification?
- Are symmetry and model values preserved?
- Is a shorter/different counterexample merely search order or semantic divergence?
- Did the Lean theorem prove the intended property or an encoding artifact?
- Can a mutation make the property fail?
