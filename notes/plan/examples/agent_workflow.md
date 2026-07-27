# Agent Workflow Example: Durable Acknowledgement Repair

This walkthrough shows the intended API shape. Handles are illustrative.

```json
{"operation":"workspace.create","arguments":{"root":"."}}
```

```json
{"snapshot":"ws_4f","intent":"in_91"}
```

```json
{
  "operation":"verification.start",
  "idempotency_key":"b1946ac9",
  "snapshot":"ws_4f",
  "intent":"in_91",
  "arguments":{"target":{"kind":"property","id":"AckImpliesDurable"}},
  "budget":{"states":100000,"wall_ms":30000},
  "output_policy":{"context_pack":true,"max_tokens":6000}
}
```

Result:

```json
{
  "verdict":"refuted",
  "task_id":"task_82",
  "artifacts":[{"kind":"crashpack","handle":"crash_7m"},{"kind":"context_pack","handle":"ctx_2a"}]
}
```

The agent expands only source correspondence around `ReplyPublished`, begins a Repair Transaction, applies a patch with a hypothesis, and evaluates. Promotion succeeds only after intent diff, exact replay, neighboring schedules, mutations, refinement/proof impact, and clean comparison pass.

At no point does the agent parse terminal text or declare the patch proven.
