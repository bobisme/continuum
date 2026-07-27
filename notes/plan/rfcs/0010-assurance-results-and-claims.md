# RFC 0010: Assurance Results and Claims

**Status:** Proposed  
**Target gate:** G0 (Revision 2 scheme, docs/26 — not citable without translation to the docs/52 Revision 3 gates per plan §22)

## Summary

Every Continuum command SHALL emit a typed, machine-readable assurance result. The CLI may summarize it, but cannot collapse distinct evidence classes into a green checkmark.

## Result shape

```rust
pub struct AssuranceResult {
    pub claim: Claim,
    pub verdict: Verdict,
    pub scope: Scope,
    pub assumptions: AssumptionSet,
    pub evidence: Vec<Evidence>,
    pub trusted_components: Vec<ComponentRef>,
    pub limitations: Vec<Limitation>,
    pub reproducer: Option<CrashpackRef>,
}
```

## Verdict

```text
Established
Refuted
Inconclusive
Unsupported
ResourceExhausted
EngineError
```

`Established` is meaningful only together with evidence and scope.

## Evidence classes

Evidence forms a partially ordered set, not a total ladder:

- deterministic example;
- randomized/sample campaign;
- bounded schedule exploration;
- equivalence-class-complete DPOR;
- finite exact reachability;
- symbolic bounded proof;
- inductive invariant proof;
- parameterized proof;
- liveness proof;
- refinement proof;
- production observation;
- external/differential corroboration;
- independently checked certificate.

A production observation is not “higher” or “lower” than an inductive proof; they establish different facts.

## Scope

Scope includes:

- model/program hash;
- semantic version;
- domain sizes;
- schedule/fault/time bounds;
- pack profiles;
- properties and observers;
- fairness/environment assumptions;
- instrumentation completeness;
- target architecture/compiler when code-level semantics matter.

## Claim examples

```text
For all configurations reachable in the finite domain
Node=3, Value=2, with ≤2 fail-stop crashes and the
storage/log-v1 adversarial profile, invariant Agreement holds.
Evidence: exact reachable-set certificate.
```

```text
Across 10^7 deterministic campaigns selected by PCT and
coverage guidance, no violation was observed.
Evidence: sampled; not exhaustive.
```

```text
This production trace is inconsistent with all behaviors of
Protocol v12 under telemetry profile complete-send-receive.
Evidence: checked conformance unsat core.
```

## Claim matrix

The repository maintains a claims matrix:

| ID | Public wording | Formal predicate | Scope | Required evidence | Current state |
|---|---|---|---|---|---|

Documentation CI rejects stronger wording than the evidence state permits.

## Reproducibility

Every result records:

- exact command and environment;
- random seeds/choice logs;
- pinned toolchain/dependencies;
- solver versions and queries;
- certificate hashes;
- source commit;
- artifact-retention policy.

## Human presentation

CLI examples:

```text
REFUTED — replayable causal counterexample
ESTABLISHED (finite exact; certificate checked)
NO BUG FOUND (sampled 10,000,000 executions)
INCONCLUSIVE (production telemetry missing StorageStable)
```

Colors and checkmarks may not erase the qualifier.

## Rejected alternatives

- One confidence score.
- “Passed” for both tests and proofs.
- Hiding solver trust behind a generic verified label.
- Treating resource exhaustion as no bug found.
