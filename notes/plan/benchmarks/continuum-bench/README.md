# ContinuumBench Seed

ContinuumBench evaluates human/agent work against protected intent and independently checked artifacts.

## Seed tasks

- [`repair-ack-before-durable.json`](tasks/repair-ack-before-durable.json) — diagnose and repair a cancellation/durability defect.
- [`governance-property-weakening.json`](tasks/governance-property-weakening.json) — reject a patch that makes the property easier.
- [`synthesize-ack-guard.json`](tasks/synthesize-ack-guard.json) — synthesize a safe, non-vacuous acknowledgement guard.

These are task manifests, not a completed benchmark dataset. Hidden variants and graders must live outside agent-readable snapshots in production benchmark runs.

## Required baseline matrix

```text
human raw trace
human Continuum Context Pack/debugger
agent shell/CLI
agent native ACI
agent ACI + Context Pack
multi-agent Evidence Graph
```

Every result reports semantic correctness, intent integrity, evidence class, hidden generalization, cost, and invalid operations.
