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
  lowercase hexadecimal digits. The `ssh-ed25519:` prefix the schema example files carried
  was a placeholder, not an SSH signature format, and this ADR does not adopt one. bn-3glnv
  replaced those placeholders with `ed25519:` tokens.

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
  `crates/continuum-evidence/tests/signing_identities.rs`. bn-1hape put the signing half
  on a production path: `continuum-security::entropy::OsEntropy` is the production
  `KeyEntropy`, `continuum-security::keystore::LocalKeystore` persists the solo-developer
  key minted on first use, and `continuumd`'s `evidence.link` signs each receipt's
  canonical bytes before publishing it. **No production path verifies yet**, and no
  intent-bundle or domain-pack producer exists on trunk. So TEST-9-07 stays `partial` and
  docs/09 T04's signing control stays a typed absence, whose check binds the receipt path
  and fails when a production verifier or a bundle or pack producer appears (review
  cr-3e3t1j). Production verification is bn-3glnv.
- `continuum-evidence` gains its first external edge. The crate documentation and
  `dependency-rationale.toml` record it, classed `trusted-checking-base`.
- Nothing in the wire changed at this ADR; the protocol stayed at 3.6, and `intent.accept`
  refused every named bundle. Protocol 3.8 (bn-3glnv) is the wire change: see "Revision:
  the signing wire".

### Follow-ups (named, not done here)

1. **Wire and daemon exposure — done (bn-3glnv, protocol 3.8).** See "Revision:
   the signing wire" below.
2. **An operating-system `KeyEntropy` source and an on-disk keystore — done (bn-1hape).**
   `OsEntropy` and `LocalKeystore` in `continuum-security`. Persisting later rotations and
   revocations to the keystore — done (bn-18w74): see "Revision: custody across restart".
3. **Signing at the producers — receipts done (bn-1hape).** `evidence.link`, the daemon's
   receipt producer, signs through `SigningRegistry::sign`. The kernel crates never sign
   (INV-004); their receipts are signed where the daemon publishes them. Promotion
   receipts (`repair.promote` is not served), intent-bundle export, and domain-pack
   publication have no producer on trunk, and sign when they land. The deployment
   launcher that builds a daemon from the keystore is `Builder::launch_signing`
   (bn-18w74).
4. **Distributed revocation — done (bn-3glnv).** The intent bundle carries the exporter's
   key-attested signer links inside its signed body: a rotation signed by both keys, a
   compromise revocation signed by the revoked key. An importer adopts a standing change
   only on the word of the keys it concerns, after the bundle's own signature and every
   link verify, and commits it only when the bundle verifies with an active signer (RFC
   0037 correction 20).

## Revision: the signing wire (bn-3glnv, protocol 3.8)

Follow-ups 1 and 4 landed as protocol 3.8 on IDL 1.16 (RFC 0026 correction 54, RFC 0027
correction 34, RFC 0037 corrections 19 and 20). What changed, and what it does to the
decisions above:

- **The daemon holds the authority.** `continuumd`'s `SigningAuthority` holds one
  `SigningRegistry`, at most one held `LocalSigner`, the local `AllowedSigners` policy,
  the key-entropy capability the deployment supplies, and the held intent bundles.
  `signing.mint`, `signing.rotate`, and `signing.revoke` are the audited daemon operations
  plan §18.6 names; `signing.registry` distributes the audit log and the head's BLAKE3
  digest (`RegistryHead::digest`); `signing.verify` returns typed provenance;
  `signing.sign_pack` signs a domain pack; `evidence.get` returns a receipt's signature.
  Every signing operation needs an unscoped, and for the writes privileged, grant.
- **The authoritative head (D6).** The daemon's own registry is authoritative for its own
  deployment: it only grows, and the daemon refreshes its head on every change and builds
  every verifier against that head. A bundle carries no audit log. A standing change
  travels only as a `SignerLink`: a rotation signed by the retiring key and its successor,
  or a compromise revocation signed by the revoked key, each over `{domain:
  "continuum.signer-link.v1", event, signer}` — a domain distinct from D4's, so a link
  signature is never an artifact signature. The importer checks every link and the
  bundle's own signature (`ArtifactSignature::authenticates`) before it applies any fact,
  and then applies a fact only on the word of the keys it concerns, known or unseen alike:
  never on another signer's assertion, never about one of the daemon's own keys, and never
  a loss recovery, which a lost key cannot sign — a peer learns of a loss only from its own
  operator. At rotation the retiring key signs its own compromise revocation and is then
  wiped, so "rotate, then revoke the old key" publishes a revocation the old key attested.
  A rotation's successor is introduced only if local policy pins it, so adoption is bounded
  by the pinned set. Adopted revocations stop four records short of the bound; only the
  held key may use those, and every operation that makes an active held key leaves one for
  its revocation, so an active held key can always be revoked; a registry built outside the
  daemon is installed only with three records free while its key is active, so every
  installed deployment can revoke its key and mint a revocable replacement. So
  `Verified` still needs a standing this daemon recorded, an unknown signer stays
  `StandingUnknown`, and a revocation learned once is never unlearned.
- **What a bundle and a pack sign.** Receipts, bundle bodies, and domain packs are all
  signed over their bytes wrapped as one canonical `Bytes` value (D4's `artifact` half),
  through one function, `signed_bytes_identity`. The bundle's own layout is `rule
  intent.bundles`.
- **CI fails closed on a real path.** `intent.accept` naming a held bundle calls
  `SignatureVerifier::verify_for_ci_acceptance` and answers `AcceptanceChainInvalid` on
  every other outcome.
- **Library additions.** `AllowedSigners` gains `entry_value`, `entry_from_value`,
  `from_entries`, `iter`, `len`, `is_empty`, `kinds`, and `intersection`;
  `SigningRegistry` gains `signers`; `record_observed`, which appends a standing fact
  learned from outside as the next audit record, validated exactly as replay validates a
  record; and `attest_rotation`, `attest_revocation`, and `rotate_attested`, the only ways
  to make a link, each refusing a key that is not active; `RegistryHead` gains `digest`; `ArtifactSignature` gains `authenticates`, the
  cryptographic check alone; and `SignerLink`, `LinkEvent`, and `LINK_DOMAIN` are the
  key-attested transition. None changes an existing encoding or a decision above.
- **A fourth signed kind: `intent-acceptance`.** RFC 0037 A1 requires an acceptance
  signature over the accepted `in_*`, its base or a genesis marker, the capability, the
  principal, and the time. It is a D4 signature of kind `intent-acceptance` over that
  canonical record (which also names its own domain and the previous chain element), so a
  signature of any other kind is never an acceptance. The kind token is additive: no
  existing message changes. A key pinned only for `intent-bundle` no longer passes CI
  acceptance; a deployment pins its acceptance keys for `intent-acceptance` too (RFC 0037
  correction 21).
- **Acceptance needs an active signer.** `verify_for_ci_acceptance` still reports a rotated
  signer's signature as verified (plan §4.6). `intent.accept` additionally requires the
  bundle's signer to be active, because a retired key vouches for no new acceptance.
- **Still out of scope at 3.8.** The daemon is sans-IO, so keys minted or rotated over
  the wire lived in daemon memory; bn-18w74 persisted them (next revision). The residual
  risk under "Revocation is local" narrows but does not close: a verifier that never
  received a bundle carrying a revocation cannot know of it.

## Revision: custody across restart (bn-18w74)

One additive wire change, and no schema change: protocol 3.8 → 3.9 and IDL 1.16 → 1.17
add the error code `OutcomeUnknown` (RFC 0026 correction 60). `signing.mint`,
`signing.rotate`, `signing.revoke` and `intent.import_bundle` declare it, and
`rule signing.custody` states it. A daemon with durable custody refuses those four
operations below 3.9 with `UnsupportedSemanticFeature`. What changed:

- **What persists.** `continuum_evidence::signing::SigningCustodyState` is everything a
  restarted authority needs except the held key's secret: the registry, the held key's
  identity, the **own** keys with the kinds each was allowed, the own links (rotations
  and compromise revocations the own keys attested), the adopted links (relayed from
  verified bundles), and each retired own key's pre-signed revocation.
  `SigningCustody` is the capability a deployment supplies to load and record it; the
  daemon and the signing library perform no I/O (INV-005, ADR-0003).
- **Own keys are recorded apart from the registry's signers.** Before, `install` took
  every signer of the installed registry as the daemon's own, and a restart would have
  taken every adopted peer key as own. Now the own set is persisted, and an install
  without restored state owns its held key alone: every other signer of the registry is
  a peer whose standing an attested link may change. This closes the residual formerly
  stated under Security.
- **Every change is recorded before it takes effect.** `signing.mint`, `signing.rotate`,
  `signing.revoke`, and a bundle adoption build the change, record it through the
  custody, and only then hold the new key and keep the new state. A new key's seed is
  captured from the entropy capability into a zeroizing buffer for that one write. A
  custody write has one commit point (the keystore's rename), and its result says which
  side of it a failure fell on (`CustodyWrite`, review cr-33e464). Before it
  (`NotRecorded`): nothing a restart loads changed, so the change is undone in memory,
  the answer is `PublicationAborted`, and the authority refuses every later signing
  write and signature until a restart. At or after it (`Unconfirmed`, for example a
  failed directory sync): a crash may keep the change or lose it, so the answer is
  `OutcomeUnknown` — an error code added for this at protocol 3.9 (IDL 1.17, RFC 0026
  correction 60, `rule signing.custody`) — never success, and the authority is
  quarantined: no signature, no signing write. The keystore writes a pending marker,
  durably, before the commit point and removes it after a confirmed sync, so the
  ambiguity survives a crash; a restart loads whichever record is durable, validates it
  in full, removes what no durable record names, and clears the marker before it signs.
  So a success is only ever a confirmed record, and a `PublicationAborted` never becomes
  an applied state. While a record is unconfirmed, no trust-deciding read uses the live
  standing (review cr-1dc5ii round 2): verification is `standing-stale`, a bundle-backed
  acceptance fails closed, and an import, a local acceptance, a lock, and the registry
  are refused. A recorded `OutcomeUnknown` is never replayed to a connection below 3.9. A daemon with durable custody refuses these writes on a connection
  below 3.9 (`UnsupportedSemanticFeature`), because no older code is honest for an
  unknown outcome.
- **The keystore.** `LocalKeystore` implements `SigningCustody`. One state file holds
  the whole state and is replaced atomically (a new `0600` file created with `O_EXCL`
  and `O_NOFOLLOW`, synced, renamed over the old one, and the directory synced); each
  held key's seed is its own `0600` file, named by its public key, written and synced
  before the state that names it, and removed after the state that retires it. A key
  file no state names — what a crash leaves — is removed by the launcher's sweep, only
  after the loaded state validated. Reads keep bn-1hape's descriptor-bound checks
  (`lstat`, `O_NOFOLLOW`, `fstat` device, inode, owner, and mode; directory and ancestor
  trust), check the file length on the descriptor before reading, bound every section
  count and record length before it is read, and replay the audit log one record at a
  time. A daemon's custody holds an exclusive lock on the store (`signing.lock`,
  `File::try_lock`) for its whole life, and every other write takes it too, so two
  processes never write one store; a second launch is refused (`Locked`). The handle
  that holds it is its only owner (review cr-1dc5ii): `LocalKeystore` is not `Clone`,
  every write through it runs under one mutex that owns the lock, a second handle in the
  same process is refused like another process (flock locks belong to the open file
  description), and each write stages its state in a fresh `O_EXCL` temporary file.
  Every store descriptor is close-on-exec, so a spawned process inherits none, and the
  handle records the process that took its lifetime lock: a copy a `fork` left in a child
  shares that lock's open file description, and is refused (`InheritedHandle`) before it
  takes a lock or reads or writes a store file. The owning process is recorded outside
  the writer mutex and checked before any lock, so a child forked while another thread
  held that mutex is refused instead of blocking on it (review cr-1dc5ii round 3); a child
  dropping its copy only closes its descriptor and never releases the parent's flock.
  The owner is recorded when the handle is constructed, before any lock exists, and a
  handle whose process cannot be read is refused rather than treated as owned (round 4).
  Residual: a child another thread starts is forked before it execs and, until then,
  holds a copy of every descriptor, so a flock released in that window stays held and a
  concurrent lock attempt is answered `Locked` (typed, fail-closed). The process id comes from `/proc/self`,
  so a durable custody launches on Linux and Android only; elsewhere the launch is
  refused (`Unsupported`), a stated residual (the keystore names filesystem facilities
  only, which INV-015's audit holds it to). The store path is
  resolved one component at a time, following each symlink by hand, and every directory
  and every symlink it passes through must be owned by root or the store's owner, a
  directory not group- or world-writable unless sticky (review cr-2qu5zr, ssh
  `StrictModes` style): only root and the owner can then change what the path resolves
  to, so the later lock, opens, and rename reach the directory validated. The check is
  re-run immediately before each of them, and the directory must keep the device and
  inode first validated (`StoreMoved`). A traversed symlink must also sit in a directory
  only root and the owner can write, sticky or not, and have exactly one link, so a hard
  link to one of the owner's symlinks planted where `fs.protected_hardlinks` is off is
  refused (round 2). A store file with a second hard link is refused, and the store
  directory is private (no group or other bits), so only root and the owner can name or
  create entries in it. Stated limits: a FUSE mount can report any owner; and where
  `fs.protected_hardlinks` is off, another user who can search a directory holding one of
  the path's symlinks can hard-link it elsewhere, so the store is refused until the owner
  recreates the symlink — availability only, never an acceptance. The bn-1hape two-file layout could not be replaced atomically; a store that
  still holds either of its files is refused (`LegacyLayout`), never read or minted
  over. No production store of that layout existed: nothing on trunk launched a daemon
  from the keystore before this revision.
- **The launcher re-validates everything.** `Builder::launch_signing` loads or, on first
  use only, mints; then `ReceiptSigner::restored` checks the record bound and the reserve
  every daemon operation keeps — an active held key can still be revoked; a state the
  daemon wrote after a replacing mint near the bound may hold fewer than
  `RECOVERY_RECORDS` free, and must restart — and `validate_custody`: the held key is the
  restored one, is own, and is not retired; every own key has standing; every link verifies, is recorded
  in the audit log, appears once, and sits on the right side of the own/adopted line; the
  own links form disjoint chains; every retired own key keeps its rotation link and its
  pre-signed revocation; an adopted key retires and is revoked at most once; and
  every pre-signed revocation is a retired own key's own; every own key revoked as
  compromised keeps its revocation link, so export still relays it; no own key but the
  held one is active; and a lost own key's successor is own. The check is complete in
  both directions against the audit log (review cr-33e464 round 4): every own rotation
  and compromise revocation has its own link; every peer rotation has its adopted link;
  every peer compromise revocation has exactly one of an adopted link or a local entry;
  every peer loss revocation has a local entry; a supersession joins two own keys; and
  no link or local entry exists without its record. The state persists the peer keys the
  operator revoked locally (`local_revocations`), because such a revocation carries no
  link and the log alone cannot tell it from an adopted one. Any failure is a typed
  `LaunchRefusal`, and nothing is swept or minted. The custody is attached only by the
  launcher; installing another identity afterwards makes the authority refuse every
  write, so it is never recorded over the store. Residual: the audit log does not say
  which keys were minted locally, so a state edited to claim a peer key as own, with no
  adopted link about it, is not detected; the state file's owner-only permissions are the
  control.
- **Held bundles and import records persist (bn-3snfi).** No wire change: the
  custody state also carries each bundle an import verified, by its `inb_` handle with
  its signed bytes, and the import records — each contract an import entered, with the
  bundle it last entered from. They live in the keystore's one state file (version 2,
  `CTMSTA02`; a version-1 file reads as holding none), so an import's adopted facts, its
  bundle, and its records commit at the one rename, whole or not at all, under the
  custody's answer rules: a refused write is `PublicationAborted` and changes nothing,
  an unconfirmed one is `OutcomeUnknown` and quarantines, and an import with nothing
  new to record writes nothing. Reads are bounded before any bundle is copied (at most
  1024 bundles of at most 4 MiB, 64 MiB together, charged bundle by bundle; at most
  65 536 records; every handle at most 256 bytes). At launch, before any key is used or
  anything is swept, every held bundle must decode, recompute to the handle it is
  recorded under, be authenticated by its own signature, and carry only attested links;
  every record must name a held bundle exporting a canonical, well-formed contract of
  that `in_` identity; and every adopted link must be carried by a held bundle. Any
  failure is a typed `LaunchRefusal` (`HeldBundle`, `ImportRecord`, `UncarriedLink`).
  Then the bundles are held again and every recorded contract re-enters the registry at
  `proposed`, so `intent.accept` naming a bundle held before a restart is decided as it
  was before it. Signer standing and a restored revision's lineage are not re-checked at
  launch: `intent.accept` re-verifies the chain, re-checks the predecessor's protected
  fields, and requires the predecessor to be the accepted head, failing closed, and an
  imported proposal is never accepted locally. Stated limits: a bundle this daemon exported
  and nobody imported stays in memory only, because `intent.export_bundle` has no
  `OutcomeUnknown` to answer an unconfirmed write with (importing it records it); the
  intent registry itself is not persisted, and `intent.reject` records nothing (it has
  no custody failure to answer), so a restart re-enters a rejected import at
  `proposed` — exactly what importing its bundle again would do — and it is still
  accepted only through its bundle; every state write rewrites the held bundles, up
  to 64 MiB, and a launch re-hashes and re-checks all of them (contracts, W1–W10, and
  every carried link's signatures); held bundles are never evicted, so the bound of 1024
  bundles or 64 MiB, which a restart used to reset, now holds across restarts until an
  eviction path exists (after it, an import of a new bundle is `QuotaExhausted`); and a
  version-1 state that holds an adopted link is refused at launch (`UncarriedLink`),
  because version 1 recorded no bundle to carry it — no production store of that
  version existed (it merged with bn-18w74 the same day, and no shipped binary launched
  one).

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

No semantic epoch change and no schema change. The protocol changed twice after this ADR:
3.8 (bn-3glnv) added the signing operations, and 3.9 (bn-18w74) added `OutcomeUnknown`
and the pre-3.9 custody refusal boundary ("Revision: custody across restart"). Existing artifacts carry no
signature and continue to read as they did. The first artifact to carry a signature will
carry the D4 record, and the envelope's `domain` string versions it for any later change.

## Security

A signature binds authorship to an artifact's canonical bytes. It is not evidence that the
artifact's claim is true, and it never replaces certificate or proof checking (ADR-0035).
The threat-model controls it serves are docs/09 T04 ("optional signing/attestation") and T07
("pack signature/provenance"). Residual risks, stated rather than discounted:

- **Key custody.** The library holds a `LocalSigner` in memory and wipes it on drop. On disk
  (bn-1hape, bn-18w74) a seed is protected by owner-only permissions and the store's
  directory checks, not by encryption, and a removed key file's blocks may stay on the
  device until the filesystem reuses them.
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
