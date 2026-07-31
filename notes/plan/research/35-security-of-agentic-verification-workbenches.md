# Security of Agentic Verification Workbenches

**Claim class:** research hypothesis

## Lane status (plan §24.5)

This note owns the **workbench security (§18)** register row, and the row is
**ratified** with the promotion gate below. Plan §24.5 quotes the sentence
verbatim under the same `quote-id`. The deadline is the opening of Phase B,
which is where agent repair with promotion begins; the registered fallback,
human-approved promotion only, is unchanged and is what the workbench runs on
until the gate is cleared.

**Ratified promotion gate.**
quote-id=workbench-security-promotion-gate "Autonomous promotion stays disabled until a single red-team corpus run against the current build and dependency epoch clears every case, where the corpus contains at least three cases for each of the ten red-team classes of research/35, one per prohibited outcome named in its kill criterion (unprivileged intent-status alteration, unprivileged evidence-status alteration, isolation escape), plus one case for each of the seven intent-policy blocks and one escape attempt against each of the eight worker-isolation controls of docs/49, for at least 45 cases in total; the run clears only if every case is refused by a trusted authority check and recorded in the append-only audit log, with zero unprivileged intent-status or evidence-status alterations and zero isolation escapes, and any later case that succeeds re-locks autonomous promotion until the corpus, enlarged with that case and its regression test, clears again in full, leaving human-approved promotion as the standing fallback."

Rationale. The draft row stated the right condition (no known unprivileged
path may alter intent or evidence status or escape isolation) but nothing
falsifiable: *no known path* is discharged by not looking. The gate above
turns it into a corpus with a fixed size and a pass bar, and every number is
counted from structure this dossier already commits to rather than invented.
The ten classes are the ten bullets of the Red-team corpus list below, taken
one-for-one, so the corpus cannot silently drop a class; the three cases per
class are the three prohibited outcomes named in this note's own Kill criteria
sentence (alter intent status, alter evidence status, escape isolation), which
forces each attack class to be pushed toward each end state it could reach
instead of being retired after one representative probe. The seven additional
intent cases are the seven policy blocks of docs/49 (bound contraction,
property weakening, assumption strengthening, fault removal, observer
coarsening, assurance downgrade, opaque-boundary expansion) and the eight
escape attempts are the eight worker-isolation controls of docs/49 (pinned
image/toolchain, read-only input mount, no ambient credentials, resource
limits, disabled network, output size/schema limits, hard kill, audit trace);
those two lists are the enumerations the architecture claims to enforce, so a
corpus that does not touch each entry has not tested the claim. Thirty plus
seven plus eight is the floor of 45, and *at least* is deliberate: the corpus
grows and never shrinks.

Refused *and* logged is a conjunction because docs/49 requires the adapter to
log attempted privileged operations and plan §18.5 requires every privileged
operation to record actor, capability, and policy decision; a block that
leaves no audit record is indistinguishable from an attack that was never
tried, so it fails the case. Binding the run to the current build and
dependency epoch follows docs/49's supply-chain rule that a dependency epoch
change invalidates evidence: a corpus result from an older toolchain is stale
evidence, not a gate. Re-locking on any later success is what keeps the gate
honest between runs, and because the enlarged corpus must clear in full, a
regression cannot be discharged by re-running only the case that failed.

The gate is falsifiable in the direction that matters: one successful
unprivileged status alteration or isolation escape, at any time, fails it and
returns the workbench to human-approved promotion. Consistent with the Kill
criteria below, no individual vulnerability kills the project or the lane; it
only re-locks autonomy.

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
