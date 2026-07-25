# Human Workflows

## Workflow 1: design a protocol before code

1. `continuum model new` creates CML module and Intent Contract.
2. IDE shows state, action, invariant, fairness, and fragment types.
3. Bounded exploration runs incrementally.
4. Counterexample appears as state delta and causal/action trace.
5. User branches in debugger or edits model.
6. Stronger proof lanes produce obligations/receipts.
7. Later Rust implementation binds to model via correspondence graph.

The user never has to invent a separate test harness for model execution.

## Workflow 2: bring an existing Rust project under control

1. `cargo continuum init` audits ambient effects.
2. Report classifies time, entropy, tasks, synchronization, network, storage, FFI, and opaque dependencies.
3. User chooses verification boundary and domain profiles.
4. Asupersync adapter runs deterministic smoke scenarios.
5. Context Packs explain unsupported/uncontrolled effects.
6. Model can be authored manually or bootstrapped as a draft from observed behavior.

Continuum must not claim a generated operational model is the intended abstraction.

## Workflow 3: debug a failure

1. Open causal explanation.
2. Inspect abstract state mismatch.
3. Step backward to last relevant write/obligation transition.
4. View enabled frontier before divergence.
5. Branch to alternative schedule/fault.
6. Compare branches.
7. Save hypothesis into repair transaction.

## Workflow 4: review a pull request

The review view groups changes by:

- intent;
- model semantics;
- concrete effects;
- abstraction/refinement;
- proof/certificate impact;
- verification/benchmark envelope;
- performance.

Textual source diff remains available but is not the only unit of review.

## Workflow 5: reason about liveness

The user sees:

- candidate fair cycle;
- fairness obligations currently owed;
- continuously/intermittently enabled actions;
- ranking/progress measures;
- scheduler/environment assumptions;
- whether the cycle is unfair, genuine, or inconclusive.

This avoids the common experience of staring at a long lasso without knowing which fairness clause matters.

## Workflow 6: production incident

1. Import partial-order telemetry under a named instrumentation profile.
2. Continuum reports matched model execution, violation, or insufficient evidence.
3. Missing telemetry is listed as concrete instrumentation needs.
4. A compatible trace is replayed/simulated if possible.
5. Result links to model/intent version deployed at incident time.

## Workflow 7: learn Continuum

Tutorials progress from puzzles to real code. Every lesson asks the learner to predict, run, explain, and repair. Tooltips explain formal vocabulary in situ. Advanced formulae remain accessible.

## Design standards

- no modal dialog blocks long work;
- cancellation is always available;
- every progress view has semantic milestones, not just percent;
- path from summary to raw evidence is at most three interactions;
- keyboard/CLI parity for core workflows;
- artifacts are linkable/shareable by handle;
- failures retain user annotations without mutating evidence;
- uncertainty is visually prominent.

## User research plan

Recruit:

- Rust engineers without formal-methods background;
- experienced distributed-systems engineers;
- TLA+/model-checking users;
- formal verification experts.

Tasks measure diagnosis, repair, assurance comprehension, and transfer. Compare raw traces, conventional model-checker output, and Continuum explanations. Record incorrect confidence, not just speed.
