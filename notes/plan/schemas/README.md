# Schema conventions

All schemas in this directory are JSON Schema draft 2020-12 with
`additionalProperties: false` at the top level.

## `$id` convention

Every schema's `$id` is:

```
https://continuum.dev/schema/<name>.json
```

where `<name>` is the schema's filename with the `.schema.json` suffix
replaced by `.json` (e.g. `intent-contract.schema.json` →
`https://continuum.dev/schema/intent-contract.json`). The older
`https://continuum.dev/schemas/<name>.schema.json` form is retired; the
six pre-R3 schemas (cir, crashpack, assurance-result, corpus-port,
proof-receipt, domain-pack) have been normalized to the convention
above, as has `benchmark-task.schema.json` (formerly
`continuumbench-task`).

## Versioning

Two mechanisms are currently in use:

1. **`schema_version` const** (R3 convention, canonical for new
   schemas): a top-level `"schema_version": { "const": "1.0" }`
   property, required. Instances declare exactly the schema version
   they were written against.
2. **`format` + `version`** (legacy, pre-R3): a `format` const naming
   the artifact class (e.g. `continuum-proof-receipt`) plus a free
   `version` string. Retained in the six older schemas until their next
   breaking revision; do not use for new schemas.

New schemas MUST use the `schema_version` const form.

## Schema epochs and compatibility

Schema versions are epochs in the sense of plan §4.6: advancing one
never mutates existing artifacts, and each advance publishes a typed
per-artifact-class compatibility statement —
`Preserved | Revalidate | Incompatible` — before it is applied.

Artifact readability is decoupled from protocol majors (plan §4.3):
evidence and receipt schemas remain readable by every future verifier
for their declared schema epoch, covered by the kernel crates'
reproducible-build covenant (plan §20). A protocol-major bump never, by
itself, invalidates or reinterprets a published artifact.

## Shared definitions

`redacted.schema.json` defines the plan §4.5
`Redacted(reason, commitment)` value. Consumers (proof-receipt,
context-pack, crashpack) inline an identical `$defs` entry named
`redacted` so each file stays self-contained for validators that do not
resolve cross-file `$ref`; the inlined copies mirror
`redacted.schema.json` and must be kept in sync with it.
