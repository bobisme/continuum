# Security of Agentic Verification Workbenches

**Claim class:** research hypothesis

## Research question

How does a verifier remain trustworthy when untrusted agents control queries, patches, generated models/proofs, and large compute budgets?

## Attack surfaces

- protocol/adapters;
- source/log prompt injection;
- artifact handles/CAS;
- intent policy;
- incremental cache;
- proof/solver workers;
- generated code execution;
- production telemetry;
- benchmark graders;
- multi-agent coordination.

## Novel proposal: capability-typed semantic operations

Associate each protocol operation with a capability effect:

```text
ReadArtifact(A)
ProposePatch(W)
RunVerification(I, B)
RequestProof(E)
ReviseIntent(F)
PromoteEvidence(P)
```

A static/generated client can prevent impossible calls; the daemon enforces dynamically. Lean can model non-escalation for a simplified protocol state machine.

## Novel proposal: evidence noninterference

Sensitive production fields may be hidden from an agent while a trusted checker uses them. Define a receipt proving a verdict over hidden evidence without exposing content. Research selective disclosure, commitment schemes, or zero-knowledge techniques only where needed; simpler trusted local checking is baseline.

## Novel proposal: semantic prompt-injection firewall

Instead of trying to detect malicious language, architecturally separate:

- instruction channel: fixed tool schemas/policies;
- data channel: source/log/model strings;
- authority channel: capabilities/checkers.

Even a fully compromised model cannot promote evidence without authority.

## Verification targets

- handle authorization state machine;
- task/publication atomicity;
- intent-policy enforcement;
- proof-worker isolation;
- audit-log append consistency;
- cache-poison resistance;
- replay artifact integrity.

## Red-team corpus

- comments asking agent to weaken tests/property;
- forged receipt JSON;
- predictable handles;
- stale snapshot substitution;
- solver output bombs;
- generated code attempting host access;
- hidden benchmark exfiltration;
- malicious domain pack;
- production trace secret leakage;
- resource-exhaustion synthesis grammar.

## Kill criteria

No project kill for individual vulnerabilities, but autonomous promotion remains disabled until no known unprivileged path can alter intent/evidence status or escape isolation.
