# Executable Plan Traceability Policy

The implementation graph represents the complete executable plan only when
`tools/validate_dossier.py` reports `plan_bones_traceability` as passing.

`tools/generate_traceability.py` derives stable requirements from the
authoritative implementation program, PR sequence, release gates, G0
experiments, invariants, proof obligations, claims, threats, risks, frontier
register, specification-debt ledger, corpus inventory, test strategy,
engineering constitution, success metrics, and kill criteria. Its generated
registry is `notes/PLAN_REQUIREMENTS.json`.

Every active requirement must be named by a `req:<ID>` label on an active leaf
Bone unless the registry explicitly classifies it as a structural requirement
that may map to a goal. Every active Bone must carry at least one valid
requirement label; narrow graph-maintenance exceptions use `trace:meta`.
A leaf Bone may map at most eight requirements. This limit is a guard against
hiding a subsystem, gate, corpus wave, or evidence campaign inside one
apparently actionable task.

PR and phase entry dependencies live on their goal Bones. Bones propagates a
blocked or punted goal's status to its descendants for `bn next`; leaf-specific
dependencies remain explicit edges. The generated graph report applies the same
ancestor-propagation rule when counting dispatch-ready leaves.

Traceability alone does not make a Bone dispatch-ready. The graph contract also
pins semantic prerequisites on the initial frontier: workspace and toolchain
scaffolds precede their consumers, and invariant, threat, and kill assays wait
for the implementation or harness they exercise. Work labeled `security`,
`threat`, or `invariant` carries `risk:high` (or a stricter risk label) so Edict
routes it through security review and the failure-mode checklist.

Each risk and kill-signal assay is assigned to the earliest phase where its
evidence can be complete. It inherits the predecessor-phase barrier and blocks
that phase's integrated exit package. Agents prepare the evidence; the
continue/narrow/defer/kill decision remains privileged and manual.

Each active frontier lane has one leaf ratification Bone. Lanes due in Phase C
or later become dispatchable during the immediately preceding phase and block
their first consumer. Distinct mathematical or experimental lanes may not share
one leaf merely because they are listed in the same register paragraph.

Deferred and already-satisfied requirements remain in the registry with their
status, so omission is explicit rather than silent. A source edit, unknown
label, uncovered active requirement, non-leaf-only implementation mapping,
untraced Bone, or over-bundled leaf makes validation fail.

ADRs, RFCs, architecture chapters, schemas, examples, and research notes are
normative or evidentiary sources, but are not separately counted merely because
they are files. Their executable obligations enter the registry through the
identified contracts above. This prevents file-count inflation while retaining
requirement-level accountability.
