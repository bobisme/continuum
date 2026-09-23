# ADR-0054: Signing identities use Ed25519 through `ed25519-dalek`, over canonical bytes, with content-addressed signers

**Status:** Accepted  
**Date:** 2026-09-23  
**Decision owners:** Continuum lead (bn-2ee4c dispatch: one new signature dependency, pinned and audited, as an explicit exception to the Phase B no-new-crates rule); drafted by `continuum-dev`  
**Implements:** plan §18.6, docs/09 §10, docs/09 T04 control "optional signing/attestation" and T07 control "pack signature/provenance", docs/19 §9 TEST-9-07

## Context

Plan §18.6 says receipts, intent bundles, and domain packs are signed, and docs/09 §10
gives the lifecycle: identities are minted through audited operations, and the
solo-developer default is a local keypair minted on first use. Organizational deployments
pin an allowed-signers set inside the intent bundle. Rotation and revocation are audited. A
lost key is not recovered, but superseded. An unverifiable signature downgrades to typed
unverified provenance. The one exception is the plan §4.2.1 CI acceptance check, which fails
closed.

Before this ADR no crate signed or verified anything. `ArtifactClass::SignedIntentBundle`
was a name only. RFC 0037 A1–A4 and RFC 0032 "Signing" assume a signature primitive that
did not exist. The schemas carry signatures as plain strings (`intent-registry-record`,
`promotion-receipt`), and the example files use `ssh-ed25519:…-placeholder` values.

The constraints that decide the choice:

- **INV-005, ADR-0003: no ambient nondeterminism.** Key generation may take entropy only
  through an explicit capability. A signature should be a function of key and message, so
  that a signed artifact is reproducible and a known-answer test can pin it.
- **`unsafe_code = "forbid"` workspace-wide**, and plan §20's reproducible-build covenant
  for the trust base. A C or assembly toolchain in the build graph of a trust-base crate
  makes the build a function of the host's `cc`.
- **INV-004: no self-certification.** The certificate checker and the `continuum-kernel-*`
  crates check certificates from wire form. A signature binds authorship, never proof
  validity (RFC 0032, ADR-0035), so the checker has no reason to link a signature crate.
- **ADR-0013: identity is canonical bytes.** What a signature covers, and what a signer
  *is*, must be the canonical encoding and not a digest of it.
- **The audit gate** (`tools/governance/dependency-audits.toml`) requires an exact-version,
  exact-checksum audit of every package in `Cargo.lock`.

## Decision

### D1 — the scheme is Ed25519 (RFC 8032), verified strictly

Ed25519 signatures are deterministic: the nonce derives from the secret key and the message.
So the same key signs the same artifact to the same bytes, and a pinned known-answer test
holds. Verification is `VerifyingKey::verify_strict`: it rejects a non-canonical `s`, a
small-order `R`, and a small-order public key, so signature malleability and weak-key
forgeries do not verify. A public key that is not a curve point, or is small-order, is
refused when a `SignerIdentity` is constructed, before any verification runs.

### D2 — the crate is `ed25519-dalek` `=2.2.0`, default features off, `zeroize` on

```toml
ed25519-dalek = { version = "=2.2.0", default-features = false, features = ["zeroize"] }
```

- **Exact pin.** 2.2.0 is the version that asupersync's `nkeys` edge already resolved, and
  bn-lf4i already audited it. This edge adds **no package** to `Cargo.lock`. The lock diff is
  two feature-unification lines (`zeroize` under `ed25519-dalek` and `curve25519-dalek`) and
  the new `continuum-evidence` edge. Every package in the signing path (`ed25519-dalek`,
  `ed25519`, `curve25519-dalek`, `curve25519-dalek-derive`, `sha2 0.10.9`, `signature`,
  `subtle`, `zeroize`, `fiat-crypto`) already has an exact-checksum audit. bn-2ee4c appends a
  re-read of the call paths to the `ed25519-dalek` audit entry.
- **`default-features = false`** drops `std`, `fast` (precomputed tables: speed only), and
  everything that reaches entropy. `rand_core` stays off, so `SigningKey::generate` does not
  exist in this build. A key is only `SigningKey::from_bytes` over a 32-byte seed.
- **`zeroize`** wipes a dropped secret key.
- The crate is `#![cfg_attr(not(test), forbid(unsafe_code))]`. Its `curve25519-dalek`
  backend holds the unsafe SIMD code, and bn-lf4i's audit reads it. Our own code stays
  under the workspace `forbid`.

### D3 — the crate that holds it is `continuum-evidence`; the checker does not

Signing lives in `continuum_evidence::signing`. That crate already owns provenance and
already depends on `continuum-value` for ADR-0013 identity. Its reverse dependencies are
`continuum-task` and `continuumd`, neither of which is in the certificate-checking base.
`continuum-certificate` and the `continuum-kernel-*` crates do **not** gain the edge.
`tools/check_crate_boundaries.py` and GOV-1-06 keep that true mechanically.
`continuum-workspace` was the other candidate. It is stdlib-only by design, and its module
documentation makes that a boundary contract, so it is not changed.

### D4 — canonical wire bytes

- **The signed message** is the canonical encoding (`Value::encode`) of the record
  `{ artifact: Bytes(<artifact canonical bytes>), domain: "continuum.signature.v1",
  kind: "receipt" | "intent-bundle" | "domain-pack", signer: <signer record> }`.
  The artifact half is the artifact's ADR-0013 `ContentIdentity`, which is its exact
  canonical bytes, not a digest. The kind is signed, so a receipt signature does not verify
  as a domain-pack signature. The signer is signed, so a signature cannot be re-attributed.
  The domain string versions the envelope.
- **A signature record** (`ArtifactSignature`) is the canonical encoding of
  `{ kind, signature: Bytes(64), signer: <signer record> }`. A verifier decodes it and
  checks it from those bytes alone. Malformed bytes are a typed outcome.
- **A signature token**, for the schemas' string fields, is `ed25519:` followed by 128
  lowercase hexadecimal digits. The `ssh-ed25519:` prefix in the schema example files is a
  placeholder. It is not an SSH signature format, and this ADR does not adopt one.

### D5 — a signer identity is a content-addressed value

A signer is the canonical record `{ public_key: Bytes(32), scheme: "ed25519" }`. Its exact
identity is that record's canonical bytes (ADR-0013). Two signers are the same exactly when
the records encode identically. The text handle is `signer_<64 hex>`: BLAKE3 over the
canonical record, under the ADR-0013 non-certified label `signer-identity-handle`. The
handle names a signer and never decides sameness. The allowed-signers set, standing, and
revocation are all keyed by the exact identity.

### D6 — lifecycle and policy

- **Entropy is a capability.** `KeyEntropy::seed` is the only way entropy reaches key
  generation. Minting fails with `EntropyUnavailable` if the source fails, and never falls
  back to another source.
- **Mint on first use.** `LocalKeyring::signer_or_mint` mints and records the local key on
  first call, and returns the same key afterwards without drawing entropy.
- **Standing and audit.** `SigningRegistry` holds each signer's standing (`Active`,
  `Rotated`, `Revoked`) and an append-only audit log of `Minted`, `Rotated`, `Revoked`, and
  `Superseded` records with the actor. It holds no secret, so a verifier can hold it.
  `SigningRegistry::sign` is the only signing path, and it refuses a signer that is not
  `Active`.
- **Rotation is not revocation.** Plan §4.6 keeps a published receipt verifiable
  indefinitely. So a rotated signer's signatures still verify, with the standing
  `Rotated { successor }` reported beside the result, and the registry refuses new
  signatures from it. Revocation is the compromise answer: a revoked signer's signatures
  downgrade to `SignerRevoked`.
- **Loss recovery** revokes the lost identity as `KeyLost`, mints a successor, and links the
  two with a `Superseded` record (docs/09 §10).
- **Allowed signers.** `AllowedSigners` maps a signer to the kinds it may sign. It has a
  canonical value, so an intent bundle can pin it by content identity. Local policy, not the
  signature, decides which signers count (RFC 0037 A3).
- **Verification never fails open.** `SignatureVerifier::verify` is total. It returns
  `Provenance::Verified` or `Provenance::Unverified` with a typed reason: `Unsigned`,
  `Malformed`, `KindMismatch`, `SignatureMismatch`, `StandingStale`, `StandingUnknown`,
  `SignerRevoked`, or `SignerNotAllowed` (INV-008). Checks run in a fixed order: presence,
  wire form, kind, cryptography, registry freshness, known standing, revocation, allowed
  set. A statement about a signer's standing is made only about an authentic signature.
- **Verified needs an authoritative standing** (review cr-3e3t1j). A verifier is built
  with the authoritative `RegistryHead`, the content identity of the registry's whole audit
  log. A registry whose head differs may be missing a revocation, so it is
  `StandingStale`. A current registry with no entry for the signer is `StandingUnknown`.
  Neither is ever read as `Active`. Until the head is distributed (follow-up 1), only a
  caller that holds the authority's registry can reach `Verified`.
- **Bounded parsing.** A signature token must be exactly 128 lowercase hex digits,
  checked before decoding into a fixed `[u8; 64]`. An encoded signature record longer than
  `MAX_SIGNATURE_RECORD_LEN` (512 bytes) is refused before the canonical decoder runs.
- **Seed hygiene.** `KeyEntropy::seed` returns `Zeroizing<[u8; 32]>`, and key derivation
  takes it by value, so the seed is wiped on every success and error path. `zeroize`
  `=1.9.0` (`default-features = false`) is the second direct edge. It is the version
  `ed25519-dalek` already resolves, and bn-lf4i audited it, so it adds no package.
- **CI fails closed.** `SignatureVerifier::verify_for_ci_acceptance` runs the same checks
  and turns every unverified outcome, including an absent signature, into
  `AcceptanceChainInvalid`, the wire code RFC 0026 and RFC 0037 I3 name.

## Consequences

- TEST-9-07's signature half has a library and real tests, in
  `crates/continuum-evidence/src/signing.rs` and
  `crates/continuum-evidence/tests/signing_identities.rs`. **No production path calls
  them yet**: no real receipt, intent bundle, or domain pack is signed or verified. So
  TEST-9-07 stays `partial` and docs/09 T04's signing control stays a typed absence,
  whose check requires the library present and no caller outside `continuum-evidence`
  (review cr-3e3t1j). Production use is bn-3glnv (wiring) and bn-1hape (producers and
  entropy).
- `continuum-evidence` gains its first external edge. The crate documentation and
  `dependency-rationale.toml` record it, classed `trusted-checking-base`.
- Nothing in the wire changes. The protocol stays at 3.6. `continuumd`'s `intent.accept`
  still refuses every named bundle with `AcceptanceChainInvalid`, because the daemon holds
  no bundles.

### Follow-ups (named, not done here)

1. **Wire and daemon exposure (bn-3glnv).** Audited daemon operations for mint, rotate,
   revoke, and supersede; the wire spelling of signature records, allowed-signers sets, and
   the authoritative `RegistryHead`; and
   `intent.accept` verifying a held bundle's chain through `verify_for_ci_acceptance`. This
   needs an RFC 0026/0037 revision and a protocol version, which the 3.6 freeze excludes.
2. **An operating-system `KeyEntropy` source** in a boundary crate, and an on-disk keystore
   for `LocalKeyring` (bn-1hape).
3. **Signing at the producers (bn-1hape).** Kernel and promotion receipts, intent-bundle export, and
   domain-pack publication call `SigningRegistry::sign` once those producers exist.
4. **Distributed revocation.** Revocation and rotation records are local audit records
   today. Distributing them inside the intent bundle, signed, is RFC 0037's open question on
   revocation and expiry.

## Alternatives considered

1. **`ring` or `aws-lc-rs`.** Rejected: C and assembly in the build graph of a trust-base
   crate break plan §20's reproducible-build covenant, and `ring` adds packages the lock does
   not have.
2. **`ed25519-compact`.** Small and pure Rust, but not in the lock. It would add a package
   and an audit for no capability gain over a crate the substrate already ships.
3. **ECDSA P-256 (`p256`/`ecdsa`).** Rejected: a per-signature nonce makes a signature not
   a function of key and message unless RFC 6979 is used, and it adds a package tree the
   lock does not have.
4. **SSH signatures (`sshsig`), to match the example placeholders.** Rejected: it adds an
   SSH key and armor format with no consumer, and the placeholders are placeholders.
5. **HMAC or another shared-secret MAC.** Rejected: a verifier would hold the signing
   secret, which breaks INV-015 as RFC 0032 "Signing" applies it (the receipt service
   holds the keys, agents never do), and makes
   distributing an allowed-signers set meaningless.
6. **A hand-written Ed25519.** Rejected: a hand-rolled primitive would look like a
   signature and not be one.
7. **Put signing in `continuum-workspace`.** Rejected: that crate is stdlib-only by its own
   boundary contract, and its `publication` seam deliberately takes identity through a trait.

## Compatibility

No semantic epoch change, no schema change, no protocol change. Existing artifacts carry no
signature and continue to read as they did. The first artifact to carry a signature will
carry the D4 record, and the envelope's `domain` string versions it for any later change.

## Security

A signature binds authorship to an artifact's canonical bytes. It is not evidence that the
artifact's claim is true, and it never replaces certificate or proof checking (ADR-0035).
The threat-model controls it serves are docs/09 T04 ("optional signing/attestation") and T07
("pack signature/provenance"). Residual risks, stated rather than discounted:

- **Key custody.** The library holds a `LocalSigner` in memory and wipes it on drop. On-disk
  custody is follow-up 2. Until it lands, a restarted process mints a new identity.
- **Rotation cannot date a signature.** Without a trusted timestamp, a signature from a
  rotated key cannot be proven to predate the rotation. So compromise must be answered with
  revocation, not rotation, and the API names the two separately.
- **Revocation is local.** A verifier whose registry lacks a revocation reports
  `StandingStale` against the authoritative head, never `Active`. But until the head is
  distributed (follow-up 1), a remote verifier cannot reach `Verified` at all. Follow-ups 1
  and 4 close this.

## Performance hypothesis

One Ed25519 signature or strict verification costs tens of microseconds. That is negligible
beside the artifacts being signed. The `fast` feature is off, which costs throughput only.
No benchmark gate is proposed until a producer signs at volume.

## Validation and rollback

Validation: `ed25519_matches_the_rfc_8032_test_vectors` pins RFC 8032 §7.1 TEST 1 and TEST 2
as literals. `signatures_are_deterministic_and_pinned` pins this ADR's envelope over a fixed
seed and artifact, so a change to the envelope, the canonical encoding, or the primitive fails
the gate. The rest of `tests/signing_identities.rs` covers round trip per kind, tampered
payload, wrong key, revoked key, rotated key, a signer outside the allowed set or its kinds,
kind relabelling, downgrade-not-fail-open, CI fail-closed, mint-on-first-use, loss
recovery, unknown standing, a stale registry missing a revocation, and an empty registry.
`tools/test-policy/sections/s9_security_tests_07.py` binds TEST-9-07's library evidence to
them and keeps the ID `partial` while no production caller exists.

Rollback: remove the `signing` module and the `ed25519-dalek` and `zeroize` edges. No published artifact
carries a signature yet, so nothing needs migration. After signatures ship, a scheme change
is a new `domain` string and a new `SignatureScheme` member, and old signatures stay
verifiable under the old member (plan §4.6).

## References

- plan §18.6, §4.2.1, §4.6, §20; docs/09 §10, T04, T07; docs/19 §9
- RFC 0026 (`AcceptanceChainInvalid`), RFC 0032 "Signing", RFC 0037 A1–A4 and I2–I3
- ADR-0003, ADR-0013, ADR-0035, ADR-0053 D7
- RFC 8032 (Ed25519)
