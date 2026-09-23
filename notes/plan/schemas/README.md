# Schema conventions

All schemas in this directory are JSON Schema draft 2020-12 with
`additionalProperties: false` at the top level.

This file is the normative home of the dossier's schema identity and
versioning convention (plan §25, SD-08). The key words MUST, MUST NOT,
SHOULD, and MAY are to be interpreted as in RFC 2119. One convention
governs every schema here; there is no per-schema dialect and no
grandfathered form.

## Identity

Three identities are in play, and they are not interchangeable.

1. **Document identity** — the `$id` of the schema file. Every schema
   document MUST declare

   ```text
   https://continuum.dev/schema/v<epoch>/<name>.json
   ```

   where `<epoch>` is the document's schema epoch as a decimal integer
   without leading zeros, and `<name>` is the file name with the
   `.schema.json` suffix removed (`context-pack.schema.json` →
   `https://continuum.dev/schema/v1/context-pack.json`). Exactly one
   document exists per (artifact class, schema epoch) pair.

2. **Class identity** — the same URI with the `v<epoch>/` segment
   removed (`https://continuum.dev/schema/context-pack.json`). It names
   the artifact class across every epoch, never changes, and is what
   prose, RFCs, and `continuumd-native-protocol.idl` cite. It MUST NOT
   be used as a `$id`.

3. **Instance identity** — the `schema_id`/`schema_epoch` pair every
   artifact carries (below). The governing document is recovered
   mechanically by inserting `v<schema_epoch>/` before the last segment
   of `schema_id`, so the two-field header and the document `$id`
   determine each other.

The retired `$id` forms are
`https://continuum.dev/schemas/<name>.schema.json` (pre-R3) and the
epoch-free `https://continuum.dev/schema/<name>.json` used until SD-08
was paid. Neither is accepted as a document identity.

## Document-level declarations

Beside `$id`, every schema document MUST carry two document-level
keywords. They are annotations under draft 2020-12 — validators ignore
them, tools and reviewers do not:

- `"schema_epoch": <integer>` — MUST equal the `v<epoch>` segment of the
  document's own `$id`.
- `"schema_kind": "artifact" | "value"` —
  - `artifact`: instances are standalone, content-addressed artifacts
    (plan §4.4). Such a schema MUST require the instance header below.
  - `value`: the schema defines a value that only ever appears embedded
    inside another artifact — today only `redacted.schema.json`. Such a
    schema MUST NOT require the header: the enclosing artifact has
    already declared it, and a nested second declaration could disagree
    with the first.

This mirrors the native protocol rather than inventing a second style.
`continuumd-native-protocol.idl` declares `idl_version` for the document
and `protocol.version` for the contract it defines, while every envelope
declares the `protocol_version` it was written under; `schema_epoch` is
the schema-side analogue of `protocol_version`. There is deliberately no
schema-side analogue of `idl_version`: a schema document's revision is
its content digest (plan §4.4), so an editorial change needs no version
bump and MUST NOT be given one.

## Instance header

Every instance of an `artifact` schema MUST carry exactly two identity
properties, both listed in `required` and both pinned with `const` in
the schema:

```json
{
  "schema_id": "https://continuum.dev/schema/context-pack.json",
  "schema_epoch": 1
}
```

- `schema_id` — the class identity. An artifact is therefore
  self-describing: a reader holding only the bytes can name the schema
  that governs them with no out-of-band context. This is the identity
  the native protocol's `Opaque` payloads are to name in place of the
  prose citations they carry today
  (`continuumd-native-protocol.idl`, open item 3).
- `schema_epoch` — the epoch of the document the instance was written
  against. A consumer that does not implement that epoch MUST reject the
  artifact with a typed error; evidence is never best-effort decoded
  across a breaking epoch (RFC 0026, docs/09 T13).

A schema MUST NOT introduce any other document-version field. The
retired forms are `format` + `version` (pre-R3), `schema_version` (R3),
and `encoding_version` (CIR). Two same-looking fields survive because
they version something else and are not schema identity:
`repair-transaction.version` is the monotonic transaction-lineage
counter of RFC 0032, and `cir.semantics_version` names the semantic
epoch its events are interpreted under.

## What advances a schema epoch

`schema_epoch` is a breaking-change counter, not a release counter.

A change to a schema document MUST advance `schema_epoch` — minting a
new `$id` and a new document — when it can invalidate an instance that
was valid before, or change what a valid instance means: removing or
renaming a property; adding a required property; narrowing a type,
`enum`, `pattern`, or bound; adding a conditional that rejects
previously valid instances; or redefining an existing property's
meaning.

A change MAY be made in place, without advancing, when every instance
valid under the document remains valid and keeps its meaning: adding an
optional property, relaxing a constraint, adding a member to an enum the
schema documents as open, or editing descriptions, `$comment`s, and key
order.

Until PR 5 freezes the interface (plan §25), epoch 1 is a draft: the
documents in this directory are edited in place and no compatibility
statement is owed for those edits. From the freeze onward a published
document is immutable — a breaking change mints `v<n+1>` and leaves
`v<n>` byte-for-byte intact. That immutability is what makes the Phase A
deliverable "immutable workspace and intent schemas" a property of the
artifacts rather than a promise in prose.

## Schema epochs and compatibility

A `schema_epoch` advance is an epoch advance in the sense of plan §4.6:
it never mutates an existing artifact, and it MUST publish the typed
per-artifact-class compatibility statement
`Preserved | Revalidate | Incompatible`, with the estimated invalidation
blast radius, before it is applied. At most two schema epochs are held
concurrently during a migration; new work defaults to the newest;
continuations resume only under their pinned epoch and are forked, never
migrated in place (plan §4.6, §9.6).

`schema_epoch` is not a seventh epoch. The six independently versioned
epochs — protocol, semantic, intent, evidence, proof, corpus — MUST NOT
be conflated with one another or with it (RFC 0026,
`rule versioning.epoch_independence`;
`crates/continuum-value/src/epoch.rs`). Concretely:

- `schema_epoch` versions one artifact class's encoding contract and
  pins no semantics. Two instances sharing a `schema_epoch` may record
  evaluations under different semantic epochs; an artifact's `epochs`
  block, never its `schema_epoch`, records what it is pinned to.
- For the classes plan §4.3 calls evidence — receipts, evidence-graph
  nodes and edges, assurance results — the evidence epoch is defined as
  the schemas an artifact declares itself under
  (`crates/continuum-value/src/epoch.rs`, `EvidenceEpoch`). Advancing
  `schema_epoch` for such a class is therefore one of the events that
  MAY advance the evidence epoch. The converse does not hold: the
  evidence epoch may advance while every schema document stays
  unchanged, and an evidence epoch MUST NOT be inferred from a
  `schema_epoch` value.
- A `schema_epoch` advance MUST NOT, by itself, advance any other epoch,
  and MUST NOT invalidate, reinterpret, or orphan a published artifact.

Artifact readability is decoupled from protocol majors (plan §4.3):
evidence and receipt schemas remain readable by every future verifier
for their declared schema epoch, covered by the kernel crates'
reproducible-build covenant (plan §20). A protocol-major bump never, by
itself, invalidates or reinterprets a published artifact, and retiring a
protocol major never orphans one.

## Shared definitions

`redacted.schema.json` defines the plan §4.5
`Redacted(reason, commitment)` value. Every consumer schema plan §4.5
obligates inlines a structurally identical `$defs` entry named
`redacted` (marked with a mirrored `$comment`) so each file stays
self-contained for validators that do not resolve cross-file `$ref`; the
inlined copies are identity-checked against `redacted.schema.json` by
the validator, so they cannot drift. Being a `value` schema, it carries
no instance header — the artifact holding the stub declares the epoch
for both.

## Run configuration

`run-config.schema.json` is the RFC 0003 "Configurations and bounds"
artifact (correction 3): the finite instantiation of a CML model's
sorts and the values of its constants, given to lowering explicitly.
Two of its rules are value-level and cannot be stated in JSON Schema,
so the schema's description states them and the reader
(`continuum_cml_elab::config`) enforces them: a `set` has no duplicate
member and a `map` no duplicate key, by value. Its identity is the
canonical ID5 encoding with `set` members and `map` entries sorted by
their own canonical encoding, so member order does not change it.

The reader and the schema are held to one verdict by a differential
(`notes/plan/tools/run_config_parity.py`, run by the dossier validator,
and `continuum-cml-elab`'s `the_reader_agrees_with_the_schema_on_every_parity_case`)
over the committed fixtures and generated edge names and shapes for every
constrained field. The schema is read under its document profile: the
strict JSON lexicon (no duplicate key, no floating-point number, no
non-finite number, no lone surrogate) and the two value-level rules above.
The reader's 16 MiB document cap and JSON depth bound of 64 are resource
bounds outside both.

## Mechanical enforcement

The convention is derived from the files, not asserted here.
`tools/validate_dossier.py` re-derives every document `$id` from its
file name and declared `schema_epoch`; checks the `schema_epoch` and
`schema_kind` keywords; checks that every `artifact` schema requires the
`schema_id`/`schema_epoch` header with the consts the identity rules
imply; checks that no retired version field survives; and validates
every example instance against its schema. A convention violation
therefore fails the dossier (`check_spec_debt`, predicate `sd08`; check
`schemas`) rather than waiting on review.
