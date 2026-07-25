# Security for Autonomous Agents

## Security posture

Agents are useful untrusted principals operating on valuable source, production traces, proof infrastructure, and compute resources.

## Capability matrix

| Capability | Modeler | Repairer | Prover | Reviewer | Trusted promoter |
|---|---:|---:|---:|---:|---:|
| read granted snapshots | yes | yes | scoped | yes | yes |
| propose model/patch/proof | yes | yes | yes | no | yes |
| start bounded task | yes | yes | yes | yes | yes |
| revise intent | proposal only | no | no | proposal only | policy-dependent |
| promote evidence | no | no | no | no | yes/checker |
| access production secrets | no/default | no | no | scoped | scoped |
| arbitrary network/host exec | no | no | no | no | no/default |

## Prompt injection boundary

Source code, comments, documentation, logs, model labels, counterexample payloads, and production messages are untrusted content. The agent adapter:

- labels data fields;
- never concatenates them into system/tool instructions;
- escapes rendering;
- enforces capabilities independently of model output;
- logs attempted privileged operations;
- can redact or summarize sensitive fields through trusted code.

## Artifact integrity

- content hashes cover canonical bytes and schema version;
- receipts cover referenced artifact identities;
- signed organizational attestations optional;
- CAS publication is transactional;
- handles are opaque and unguessable where confidentiality matters;
- authorization is checked on every dereference.

## Worker isolation

Lean, solvers, corpus oracles, generated Rust, and third-party analyzers run with:

- pinned image/toolchain;
- read-only input mount;
- no ambient credentials;
- resource limits;
- network disabled unless required and allowlisted;
- output size/schema limits;
- cancellation and hard-kill fallback;
- audit trace.

## Denial of service

Verification naturally permits explosive workloads. Controls:

- preflight complexity estimates;
- per-actor/project budgets;
- state/solver/proof/token quotas;
- resumable tasks;
- fair scheduling;
- duplicate task coalescing;
- artifact/result size limits;
- Forge grammar restrictions;
- cancellation.

Budget exhaustion is not a negative/positive verdict.

## Evidence forgery

Only checker services can create validated/proved edges. Clients cannot submit arbitrary status. Every promotion re-fetches and verifies referenced artifacts.

## Intent attacks

Policy blocks:

- bound contraction;
- property weakening;
- assumption strengthening;
- fault removal;
- observer coarsening;
- assurance downgrade;
- opaque-boundary expansion.

Allowed redesign creates a new intent version and explicit review.

## Benchmark integrity

Hidden tasks/graders reside outside agent-readable snapshots. Tool outputs cannot reveal expected patches. Semantic clone detection reduces training leakage.

## Supply chain

Pin Rust/Lean/solver/domain-pack dependencies. Receipts name closure hashes. A dependency epoch change invalidates evidence according to policy.

## Incident response

Security events produce separate audit incidents, not model counterexamples. Relevant semantic tasks may be quarantined. Reproducible evidence helps determine whether a malicious patch exploited verifier, adapter, policy, or proof service.
