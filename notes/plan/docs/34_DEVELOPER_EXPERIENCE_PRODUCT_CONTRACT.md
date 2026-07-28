# Developer Experience Product Contract

## Product promise

A user should move from a concurrent-systems question to trustworthy evidence without becoming an expert in every engine Continuum employs.

The product contract is:

> Every operation is discoverable, reproducible, interruptible, explainable, and honest about its assurance envelope.

## Human contract

### Immediate comprehension

The first screen or terminal result must communicate:

1. verdict;
2. property and intent version;
3. one-sentence semantic explanation;
4. causal-core size;
5. assurance class;
6. unresolved unknowns;
7. next useful operations.

It must not lead with engine banners, JVM flags, solver statistics, or serialized states.

### Exactness on demand

Every simplified item links to exact evidence. Progressive disclosure is lossless navigation, not information destruction.

### Stable vocabulary

Use consistent terms:

- **failure** for a checked counterexample;
- **engine error** for tool failure;
- **unsupported** for unavailable semantics;
- **inconclusive** for insufficient evidence;
- **assumption** for environmental restriction;
- **bound** for finite scope;
- **proof** only for independently checked theorem/certificate classes defined by policy.

### Reproduction

Every failure has a single stable replay handle. Every success names the command/API and artifact needed for independent checking.

### No surprise cost

Before expensive work, Continuum reports the planned portfolio and budget. It can return useful partial evidence at budget boundaries.

## Agent contract

### No screen scraping

All semantic operations have stable versioned schemas. Human output is not an API.

### Bounded observations

Results fit declared byte/token budgets. Larger graphs are paginated by stable handle and semantically meaningful expansion operations.

### Actionable failures

An error includes:

- machine code;
- affected handle/input;
- whether retry is safe;
- valid recovery operations;
- whether any evidence was committed.

### Explicit state

Agents can save, share, fork, and resume tasks by handles. No operation depends on hidden chat history or connection affinity.

### Capability clarity

Tool descriptions distinguish read, propose, execute, and promote authority. Possession of an artifact handle does not grant mutation or promotion rights.

## Latency targets

This table is gate-normative for G5 (plan §21 Phase C exit): at G5 the
targets are enforced on the reference workload, measured with a
saturating background swarm present (plan §4.1). Before Phase C they
are design targets.

| Interaction | p50 target | p95 target |
|---|---:|---:|
| parse/type/intent diagnostics | 50 ms | 200 ms |
| semantic hover/correspondence | 50 ms | 150 ms |
| cached bounded check | 100 ms | 500 ms |
| local incremental exploration feedback | 250 ms | 2 s |
| explain: failure → rendered causal explanation | 200 ms | 1 s |
| Context Pack compilation from existing evidence (at ≥10^7 evidence nodes) | 100 ms | 1 s |
| evidence query over the evidence graph at ≥10^7 nodes | 100 ms | 500 ms |
| replay/minimized branch load | 250 ms | 1 s |
| debugger reverse-step (checkpoint re-execution) | 100 ms | 500 ms |
| debugger branch fork at a frontier | 250 ms | 1 s |
| task cancellation acknowledgment | 50 ms | 200 ms |

The ≥10^7-node scale qualifier on the evidence-query and Context Pack
rows is gate-normative for G5's evidence-query bullet (plan §22 G5).

The explain-interaction row (failure → rendered causal explanation, p50 200 ms / p95 1 s) is gate-normative for G5.

Long tasks stream monotonic progress and return continuations.

## Output principles

### Terminal

- one result per line/group;
- stable rule IDs;
- no color-only meaning;
- paths relative to workspace where possible;
- exact `--json` equivalent;
- no progress noise when noninteractive.

### IDE

- source-local diagnostics when mapping exists;
- evidence handle in diagnostic metadata;
- code lens for check/explain/debug;
- semantic diff preview before protected changes;
- no automatic model/code rewrite without a preview transaction.

### Agent

- typed enums rather than prose classifications;
- canonical ordering;
- stable IDs;
- content hashes and epochs;
- explicit omissions;
- no duplicate payload when a reference suffices.

## Workflow acceptance tests

These four workflows are the referent of the preregistered G8 study
(plan §21.1): the three human-executed workflows (new model; existing
Rust system; review) are covered by the two-cohort human study; the
agent-repair workflow is covered by the G2 ACI ablation and
ContinuumBench, not the human study.

### New model

A developer can create, check, inspect a shortest witness, and add an invariant in under ten minutes without reading implementation docs.

### Existing Rust system

A developer can identify uncontrolled effects, run one deterministic campaign, and replay a failure without replacing production logic.

### Agent repair

An agent can diagnose and propose a repair using only Context Pack and expansion operations. It cannot promote a patch that changes intent.

### Review

A reviewer can answer within one screen:

- what changed semantically;
- whether intent changed;
- what evidence was invalidated/rebuilt;
- what remains unknown.

## Anti-goals

- reproducing every engine's native flags in the primary UX;
- pretending bounded checking is proof;
- forcing users to maintain duplicate configurations;
- exposing raw internal IDs without semantic labels;
- generating unreviewable model/code changes;
- optimizing agent token count by hiding uncertainty;
- treating a successful tool invocation as a successful verification result.

## Measurement

DX is evaluated continuously through human and agent task suites. “It felt clear to the implementer” is not evidence.
