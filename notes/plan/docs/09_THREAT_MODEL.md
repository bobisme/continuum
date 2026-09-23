# Threat Model

## 1. Security objective

Continuum must not create false confidence. Its primary security property is **sound communication of evidence**:

> An adversary, malformed artifact, engine bug, pack bug, incomplete trace, or resource limit must not cause a stronger assurance claim than the available evidence justifies.

Availability and confidentiality matter, but an unsound “proved” verdict is the catastrophic failure.

## 2. Assets

- semantic definitions;
- model/property digests;
- replay integrity;
- certificate validity;
- domain-pack fidelity profiles;
- production traces;
- implementation build identity;
- claim ledger;
- solver/proof artifacts;
- private payloads contained in traces;
- user trust.

## 3. Adversaries and failure sources

### Accidental

- verifier bugs;
- unsound optimization;
- ambiguous semantics;
- hidden nondeterminism;
- incorrect model;
- stale pack;
- incomplete instrumentation;
- integer overflow;
- noncanonical serialization;
- compiler/runtime behavior change.

### Malicious

- crafted model/certificate causing panic or resource exhaustion;
- forged production trace;
- malicious third-party pack lying about independence;
- solver returning malformed evidence;
- supply-chain dependency compromise;
- agent modifying properties to make verification pass;
- artifact substitution;
- hash collision attack;
- path traversal in crashpack extraction.

## 4. Trust boundaries

```text
untrusted:
  models
  imported TLA+/Quint artifacts
  third-party packs
  production trace streams
  solver output
  certificates before checking
  agent output
  crashpacks from others

semi-trusted:
  first-party optimized engines
  asupersync adapter
  first-party packs
  instrumentation runtime

trusted for certified claim:
  semantic epoch definition
  canonical decoder
  property evaluator
  continuum-kernel
  explicitly listed external axioms
```

## 5. Threats and controls

### T01 — Parser/decoder memory exhaustion (delivered: bn-3nfy)

Controls:

- length/depth limits;
- streaming decoding;
- checked arithmetic;
- allocation budgets;
- no recursive descent on unbounded attacker structures;
- fuzzing and corpus tests.

### T02 — Hash collision changes reachability (delivered: bn-277, bn-2res, bn-30eym)

Controls:

- fingerprints are indexes, not identity;
- exact canonical comparison on collision;
- cryptographic digests for artifact identity;
- adversarial collision corpus.

### T03 — Unsound POR

Controls:

- conservative dependence;
- no-reduction reference;
- pack commutation tests;
- observer/property class in result;
- certificate/fallback for strong claims.

### T04 — Forged trace or artifact substitution (delivered: bn-1ptb — four of six controls bound to live, re-run enforcement in `tools/governance/check_t04_evidence.py` / `evidence/t04.json`: content-addressed manifests via ADR-0013 and `continuum-value`'s canonical hashing plus workspace snapshot manifests; build identity via the INV-014 receipt `Seam` and KCOV-09; reject digest mismatch via publication's `IdentityCollision` abort and the certificate kernel's independent wire-form-only re-verification of a wrong-family or mutated certificate; and preserve redaction commitments via `Redacted.commitment` surviving redaction and re-verification unmodified — plus two typed-absence gaps recorded rather than fabricated, each with its own mechanical absence-check: hash chain/Merkle root over events, since no production-trace ingestion producer exists yet in this Phase-A tree and bn-15gj's Merkle root is over workspace content, a different asset; and signing/attestation, since docs/09 marks this one optional, nothing in the tree falsely claims it, and the other four controls do not depend on it)

Controls:

- content-addressed manifests;
- hash chain/Merkle root over events;
- build identity;
- optional signing/attestation;
- reject digest mismatch;
- preserve redaction commitments.

### T05 — Incomplete production observation presented as validity

Controls:

- instrumentation schema declares observability;
- sequence gaps and dropped buffers are events;
- checker has `INCONCLUSIVE_OBSERVATION`;
- property monitorability analysis;
- no default closed-world assumption.

### T06 — Pack claims host semantics it does not provide

Controls:

- versioned fidelity profile;
- host conformance tests;
- platform matrix;
- explicit axioms in assurance result;
- pack can only raise assurance for declared configurations.

### T07 — Malicious pack independence rule

Controls:

- sandbox untrusted packs;
- dynamic footprint validation;
- baseline cross-check;
- first-party review for certified mode;
- pack signature/provenance.

### T08 — Solver lies or proof checker disagrees

Controls:

- SAT witness replay;
- proof checking;
- solver-trusted label;
- multiple solver diversity;
- preserve raw proof and command.

### T09 — Agent weakens property (delivered: bn-20co, bn-hakt3 — four of five controls bound to live, re-run enforcement in `tools/governance/check_t09_evidence.py` / `evidence/t09.json`: semantic property diff via PR-12's `properties` classifier, which classifies an agent-authored complementary-disjunct weakening of a real corpus claim `weakened` and blocks it under `locked`, with disguise and blind-classifier mutants caught; review gates via the six INV-001 verbs forbidding `allow` on every acceptance path, privileged accept/reject/lock, and GOV §4 review records; and the immutable baseline property digest, new here: `tools/governance/t09-property-baseline.json` pins identity, claims, and policy BLAKE3 digests of every committed Intent Contract, computed by the real decoder in `crates/continuum-intent/tests/t09_property_baseline.rs`, and a merge-base rule demands a fresh named revision for every moved, new, or retired pin — plus agent cannot modify claim ledger or certificate bound at the daemon and certificate grain and, new in bn-hakt3, at the repository claims registry too: `tools/governance/claims-baseline.json` pins every docs/18 claim row's state and a digest of its claim and required-evidence wording, and a merge-base rule demands a fresh named revision whose bone carries that claim's own req: label for every promoted, reworded, or retired row; and mutation score a typed gap, since no producer computes a per-property mutation score and Forge's plan §14.5 mutation challenges are not landed, its own mechanical absence-check)

Controls:

- semantic property diff;
- review gates;
- immutable baseline property digest in CI;
- mutation score;
- agent cannot modify claim ledger or certificate.

### T10 — Replay executes hostile code

Controls:

- explicit user action;
- container/sandbox mode;
- network disabled by default;
- filesystem capability restrictions;
- resource/time limits;
- no automatic replay of downloaded artifacts.

### T11 — Secret leakage through traces

Controls:

- typed redaction at event schema;
- salted commitments;
- local abstraction;
- field-level retention;
- access control and encryption;
- crashpack scrubber;
- property declared against redacted observations where possible.

### T12 — Dependency/unsafe compromise (delivered: bn-nio3 — dependency vet/advisory-scanning gap closed, six controls bound in tools/governance/evidence/t12.json)

Controls:

- pinned lockfile;
- dependency allowlist;
- cargo-vet/advisory scanning;
- unsafe boundary audit;
- reproducible build metadata;
- minimal kernel dependencies.

### T13 — Semantic downgrade

A newer tool silently interprets an old artifact with weaker semantics.

Controls:

- explicit semantic epoch;
- exact pack digest;
- migration report;
- reject unknown breaking epoch;
- no “best effort” decode for evidence.

### T14 — Resource exhaustion mistaken for proof

Controls:

- `INCONCLUSIVE_RESOURCE_LIMIT`;
- partial exploration artifacts clearly marked;
- no success from frontier exhaustion unless closure is verified;
- certificate checker separately confirms closure.

### T15 — Common-mode compiler bug

Controls:

- foreign oracle corpus;
- independent kernel implementation path;
- potentially compile kernel with diverse toolchain;
- mechanized reference;
- serialization-level conformance vectors.

## 6. Production journal design

Requirements:

- append-only;
- monotonic local sequence number;
- causal predecessor IDs;
- clock uncertainty;
- explicit loss marker;
- schema digest;
- implementation build digest;
- field redaction map;
- integrity chain.

A central collector is not trusted to invent causal edges. It aggregates signed/local evidence.

## 7. Certificate decoder constraints

- bounded integer widths or arbitrary precision with allocation limits;
- canonical representation rejection;
- duplicate ID rejection;
- cycle/conflict validation;
- no path/URL dereference;
- no executable payload;
- stable error codes;
- checker never panics on input.

## 8. Security testing

- cargo-fuzz targets for every decoder;
- property tests for canonical encodings;
- malicious graph/certificate generator;
- pack honesty mutants;
- trace loss/reordering/tampering;
- solver proof corruption;
- replay sandbox escape tests;
- supply-chain bill of materials.

## 9. Incident policy

Any confirmed false-positive success verdict:

1. blocks release;
2. revokes affected claim IDs;
3. publishes affected semantic epochs/packs/engines;
4. supplies artifact scanner;
5. adds permanent mutation/regression;
6. reevaluates whether the optimizing engine remains eligible for certified mode.

Soundness incidents are treated more seriously than crashes or performance regressions.

## 10. Signing identities

Receipts, intent bundles, and domain packs are signed; plan §18.6 delegates the signing-identity lifecycle here.

- **Minting.** Identities are minted through audited daemon operations. The solo-developer default is a local keypair minted on first use and recorded in the audit log.
- **Trust-root distribution.** Organizational deployments pin an allowed-signers set distributed inside the intent bundle (plan §4.2.1).
- **Rotation and revocation.** Both are audited daemon operations.
- **Loss recovery.** A lost key is not recovered; recovery re-mints under a new identity with an audit-linked supersession record.
- **Verification failure.** A signature that cannot be verified downgrades the artifact to typed unverified provenance rather than failing open — except the plan §4.2.1 CI acceptance check, which fails closed by policy.
