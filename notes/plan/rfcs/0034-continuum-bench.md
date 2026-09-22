# RFC 0034: ContinuumBench Task and Grader Contract

## Status
Draft.

## Bundle
Public snapshot/intent/task/capabilities/budget; hidden variants/mutations/grader; required evidence class.

## Submission
Final snapshot/artifacts, evidence graph root, operation trace, resource report, and optional explanation. Natural-language-only submissions are invalid for artifact tasks.

## Grader order
Intent integrity, security, semantic correctness, evidence validity, hidden generalization, cost, explanation.

On the wire, `benchmark.run` names graders by these stages, as the tokens `intent_integrity`, `security`, `semantic_correctness`, `evidence_validity`, `hidden_generalization`, `cost`, and `explanation`, distinct and in this order (RFC 0026 IDL, `rule benchmark.graders`). A grader is never named by its implementation, which is hidden. `benchmark.run` names the task by the task document's `task_id`, not by a handle: a content handle over the bundle would be a function of its hidden half (`rule benchmark.task_identity`).

## Isolation
Hidden grader is inaccessible to agents. Source hashes and semantic clones are screened across splits.

## Reporting
Vector metrics and Pareto fronts; no single leaderboard hides cost/assurance.

## Acceptance
Reproducible grader, mutation sensitivity, leakage audit, baseline agents/humans.
