//! Acceptance re-derivation for release gate **G1-01** (bone `bn-1k1s8`).
//!
//! > snapshots, intent contracts, handles, and artifacts are immutable and
//! > content-addressed
//! >
//! > — `notes/plan/docs/52_RELEASE_GATES_REV3.md:22`, G1 bullet 1
//!
//! # Why this file exists beside the evidence that already passes
//!
//! [`PHASE_A_EXIT_PACKAGE.md`] §9.4 A17 states its own limit in its own words:
//!
//! > Sixteen per-criterion gate acceptance bones are open […]. Their brief —
//! > "independently rerun this criterion against the integrated phase artifact" — is not
//! > discharged by this package, which re-ran the *delivering* suites, not an independent
//! > re-derivation.
//!
//! Re-running a delivering suite proves the suite still passes. It does not re-derive the
//! property, because every check in it is written against the same identity
//! *implementation* the daemon uses: `every_published_address_is_the_blake3_identity_of_its_own_bytes`
//! (`crates/continuum-benchmark/tests/phase_a_triple_surface_identity.rs`) recomputes each
//! address "through the daemon's own `ContentIdentifier`", and
//! [`StoreAudit::fsck`](continuum_workspace::publication::StoreAudit::fsck) — which now takes
//! the declared identifier as an explicit argument (bn-k99dt) rather than the store's own
//! configured seam, so it catches a *substituted* `ContentIdentifier` — is still handed
//! `continuumd::daemon::identity::Blake3Identity` itself as that declared identifier, the very
//! implementation under test here. Both answer "does this content hash to what
//! `Blake3Identity` says it should?", which is a strictly weaker question than "is the
//! identity a content hash a second, independent implementation of BLAKE3 can recompute?".
//!
//! This file asks the stronger question. Its instrument is [`spec_blake3`]: a second,
//! from-scratch BLAKE3 written here from the specification, sharing no line of code with
//! the vendored `blake3` crate, with `continuum_value::identity::Blake3Hasher`, or with
//! `continuumd::daemon::identity::Blake3Identity`. Every address this file checks is
//! recomputed with that implementation from bytes that came off the wire or out of the
//! store. A daemon whose `Blake3Identity`/`Blake3Hasher` implementation stopped being
//! genuine BLAKE3 — while every store still declared and filed under that same
//! implementation — would keep every existing suite green, including `fsck` (bn-k99dt:
//! `fsck` now checks against a declared identifier, but the declared identifier a real
//! daemon hands it *is* `Blake3Identity`, so a bug in `Blake3Identity` itself agrees with
//! itself there too) — and would fail here.
//!
//! # Method, stated so the difference from the delivering suites is checkable
//!
//! | Delivering evidence | This file |
//! |---|---|
//! | recomputes addresses through the daemon's own `ContentIdentifier` | recomputes them with an independent BLAKE3 written from the spec ([`spec_blake3`]) |
//! | anchors the hash to the published vectors *inside the crate that vendors it* (`continuum-value`'s `blake3_matches_the_published_test_vectors`) | anchors the *independent* implementation to the same published vectors, and shows a one-round-short variant fails them ([`control_a_round_short_variant_of_the_instrument_fails_the_published_vectors`]) |
//! | asserts equality of artifacts across three surfaces | attacks the artifacts: re-publication, convergent re-creation, acceptance, lock, and a colliding identity seam |
//! | reads identities out of the store's own audit view | reads them out of the store *and* off the wire, and checks the two agree with the independent digest |
//! | runs the campaign that produced the evidence | runs a fresh campaign and additionally runs the whole script against a daemon whose seam is deliberately not BLAKE3, asserting that store's `fsck` — checked against the declared `Blake3Identity` — now finds every artifact defective too, and that this file's independent predicate *fails* on every artifact it published ([`negative_control_a_non_blake3_seam_is_self_consistent_and_fails_this_files_predicate`]) |
//!
//! That last row is the one that decides whether this file was worth writing. The
//! counterfeit daemon is caught by the declared-seam `fsck` check (bn-k99dt) precisely
//! because its `ContentIdentifier` is a wholesale substitution, not `Blake3Identity` with a
//! flaw — the gap this file closes is the one measure the tree still cannot apply to
//! itself: whether `Blake3Identity`'s own output is genuine BLAKE3. This file rejects every
//! artifact the counterfeit publishes on that independent ground.
//!
//! # The three identity levels, and which this file covers
//!
//! `notes/plan/schemas/README.md` "Identity" separates three things, and this file keeps
//! them separate:
//!
//! 1. **Document identity** — a schema file's `$id`,
//!    `https://continuum.dev/schema/v<epoch>/<name>.json`;
//! 2. **Class identity** — the same URI without `v<epoch>/`, which artifacts carry as
//!    `schema_id` and which MUST NOT be a `$id`;
//! 3. **Instance identity** — the `<class>_<digest>` handle the store files an artifact
//!    under.
//!
//! Levels 1 and 2 are covered once, mechanically, by
//! [`the_three_identity_levels_stay_distinct_document_class_and_instance`], which reads a
//! real schema document out of `notes/plan/schemas/` and a real registry record off the
//! wire. **Every other test in this file is about level 3 only.** G1-01's own sentence is
//! a level-3 sentence — a schema document is not "immutable and content-addressed" in the
//! store's sense, it is a file in the dossier — and conflating the two would let a green
//! schema check stand in for an artifact check.
//!
//! # The verdict this file reaches
//!
//! **SATISFIED-AT-NARROWER-SCOPE.** The narrowing is stated in
//! [`scope_the_classes_this_daemon_actually_mints`] and
//! [`scope_the_two_content_addressing_shapes_are_not_one_shape`] as executable
//! assertions rather than prose, and repeated here so a reader of the module doc gets it
//! without running anything:
//!
//! - **Content-addressed over the artifact's own stored bytes** — `ws_*`
//!   (`WorkspaceSnapshot`) and `task_*`-class store records. For these, "the identity is
//!   the digest of the artifact" is literally true and this file recomputes it from bytes
//!   it read back, transcribing nothing.
//! - **Content-addressed over a derivation preimage, not over the artifact** — `in_*`
//!   (`IntentContract`, digest of the RFC 0037 ID1/ID2 preimage, which excludes
//!   `intent_id`, `name`, `source` and `normal_form`) and the `task_*` *campaign* handle
//!   (digest of the `verification.start` request preimage). Both are content addresses of a
//!   *function of* the artifact, so an independent check must transcribe that function; this
//!   file does so, in [`campaign_preimage`] and through
//!   `IntentContract::identity_preimage_bytes`, and says so at each call. `cont_*` and
//!   `model_*` are derived the same way and are **not probed here** — no test in this file
//!   parks a continuation or names a model handle on the wire.
//! - **Not content-addressed at all** — `cap_*` (`ArtifactClass::Capability`), by design
//!   and by an executable assertion here.
//! - **Not minted by this script at all**, so unprobed by anything in this file: sixteen of
//!   plan §4.4's nineteen classes. INV-007: this is an absence, not a pass, and
//!   [`scope_the_classes_this_daemon_actually_mints`] counts it rather than describing it.
//!
//! # Absences and findings this file states rather than papers over (INV-007, INV-008)
//!
//! 1. **Two of nineteen classes reach the publication store** in the whole script —
//!    `WorkspaceSnapshot` and `Task`. Sixteen are unprobed here because nothing in this
//!    daemon mints them; `IntentContract` is probed and is deliberately *not* in the store.
//! 2. **The `in_*` contract lives in the daemon's registry, not in the publication store.**
//!    Its immutability is therefore proved on the wire (`intent.get` before and after a
//!    governance write) and not by a store read. That is a different and slightly weaker
//!    instrument, and it is named as one.
//! 3. **No epoch advance is exercised**, matching [`PHASE_A_EXIT_PACKAGE.md`] §9.2 N7. What
//!    is exercised is the observable consequence — two daemons at different epoch pinnings —
//!    which is not the same as migrating a published artifact across an advance.
//! 4. **The `cap_*` publication refusal arrives as `CapabilityDenied`, not as the
//!    publication abort the store's own documentation names.** The `NotContentAddressed`
//!    abort arm exists and is unreachable from an authorized caller, because the
//!    authorization layer refuses first. Recorded at
//!    [`scope_the_capability_class_is_the_only_class_that_is_not_content_addressed`].
//! 5. **Single-process, in-memory, one request at a time.** No transport, no disk, no
//!    concurrency. Everything here is about what the identity *is*, not about what survives
//!    a restart — that is G1-08's question, and §6.4 of the exit package already answers it
//!    with a declared architectural decline.
//!
//! [`PHASE_A_EXIT_PACKAGE.md`]: ../../../notes/plan/notes/PHASE_A_EXIT_PACKAGE.md

use std::collections::{BTreeMap, BTreeSet};

use continuum_intent::canonical_json::Json;
use continuum_intent::change_policy::{PolicyField, PolicyVerb};
use continuum_intent::contract::IntentContract;
use continuum_value::epoch::ProtocolWindow;
use continuum_workspace::artifact_path::{ArtifactClass, ArtifactHandle};
use continuum_workspace::publication::{
    CapabilityToken, ContentIdentifier, IdentityUnavailable, PublishRefusal, StoreDefect,
};
use continuum_workspace::snapshot::WorkspacePath;
use continuumd::daemon::family::{Arguments, Payload};
use continuumd::daemon::identity::Blake3Identity;
use continuumd::daemon::intent::IntentFamily;
use continuumd::daemon::state::{IntentRecord, RegistryStatus};
use continuumd::daemon::task::{Preimage, TaskFamily, budget_preimage, epochs_preimage};
use continuumd::daemon::verification::{VerificationFamily, model_source};
use continuumd::daemon::workspace::WorkspaceFamily;
use continuumd::daemon::{Daemon, OperationOutcome, OperationRequest, identity};
use continuumd::protocol::envelope::{Budget, EpochSet, RequestEnvelope};
use continuumd::protocol::handshake::{
    CapabilityDescriptor, CapabilityProfile, ClientHello, Negotiated, VersionRange, negotiate,
};
use continuumd::protocol::operations::intent::{
    IntentAcceptRequest, IntentGetRequest, IntentLockRequest,
};
use continuumd::protocol::operations::verification::VerificationStartRequest;
use continuumd::protocol::operations::workspace::WorkspaceCreateRequest;
use continuumd::protocol::registry::ENCODINGS;
use continuumd::protocol::scalar::{
    ActorId, CapabilityHandle, Commitment, EpochIdentity, IntentHandle, Opaque, OperationName,
    ProtocolVersion, RequestId, Timestamp, WorkspaceHandle,
};
use continuumd::protocol::shared::{SnapshotComponents, SnapshotEpochs, Target};
use continuumd::protocol::spec::{Nullable, Optional, ProtocolEnum};
use continuumd::protocol::vocabulary::{
    AuthorityLevel, Encoding, Portfolio, PriorityClass, ResultStatus, TargetKind,
};

// =========================================================================================
// the instrument: BLAKE3, written here from the specification
// =========================================================================================

/// A second BLAKE3, implemented from the specification, for recomputing addresses.
///
/// # Why a second implementation rather than the vendored crate
///
/// The workspace's `Cargo.lock` may not move for this bone, so a *third-party* second
/// implementation is not available; and calling `continuum_value::identity::Blake3Hasher`
/// would recompute a daemon address with the daemon's own hash, which is the circularity
/// this whole file exists to break. What is left is the honest option: write BLAKE3 again,
/// from its specification, in code that shares nothing with the vendored crate.
///
/// # What it is and is not
///
/// It is the unkeyed `hash` mode, tree included, over a `&[u8]` held in memory: chunk
/// chaining with the 64-byte block compression, the binary tree over chunk chaining
/// values, and the `ROOT` flag on the final compression. It is not incremental, not
/// keyed, not `derive_key`, and takes no extended output — 32 bytes is what an artifact
/// handle carries. Every constant below is from the specification, not copied from
/// `continuum-value`, and [`control_the_instrument_reproduces_the_published_blake3_vectors`]
/// is what makes that claim checkable rather than asserted.
mod spec_blake3 {
    /// The eight initialization words (the SHA-256 IV).
    const IV: [u32; 8] = [
        0x6a09_e667,
        0xbb67_ae85,
        0x3c6e_f372,
        0xa54f_f53a,
        0x510e_527f,
        0x9b05_688c,
        0x1f83_d9ab,
        0x5be0_cd19,
    ];

    /// The message-word permutation applied between rounds.
    const MSG_PERMUTATION: [usize; 16] = [2, 6, 3, 10, 7, 0, 4, 13, 1, 11, 12, 5, 9, 14, 15, 8];

    /// Bytes per compression input block.
    const BLOCK_LEN: usize = 64;

    /// Bytes per chunk: sixteen blocks.
    const CHUNK_LEN: usize = 1024;

    /// The specification's domain-separation flags.
    const CHUNK_START: u32 = 1 << 0;
    /// See [`CHUNK_START`].
    const CHUNK_END: u32 = 1 << 1;
    /// See [`CHUNK_START`].
    const PARENT: u32 = 1 << 2;
    /// See [`CHUNK_START`].
    const ROOT: u32 = 1 << 3;

    /// The number of rounds the specification fixes.
    pub const ROUNDS: usize = 7;

    /// The mixing function `G`.
    fn g(state: &mut [u32; 16], a: usize, b: usize, c: usize, d: usize, mx: u32, my: u32) {
        state[a] = state[a].wrapping_add(state[b]).wrapping_add(mx);
        state[d] = (state[d] ^ state[a]).rotate_right(16);
        state[c] = state[c].wrapping_add(state[d]);
        state[b] = (state[b] ^ state[c]).rotate_right(12);
        state[a] = state[a].wrapping_add(state[b]).wrapping_add(my);
        state[d] = (state[d] ^ state[a]).rotate_right(8);
        state[c] = state[c].wrapping_add(state[d]);
        state[b] = (state[b] ^ state[c]).rotate_right(7);
    }

    /// One round: four column mixes, then four diagonal mixes.
    fn round(state: &mut [u32; 16], m: &[u32; 16]) {
        g(state, 0, 4, 8, 12, m[0], m[1]);
        g(state, 1, 5, 9, 13, m[2], m[3]);
        g(state, 2, 6, 10, 14, m[4], m[5]);
        g(state, 3, 7, 11, 15, m[6], m[7]);
        g(state, 0, 5, 10, 15, m[8], m[9]);
        g(state, 1, 6, 11, 12, m[10], m[11]);
        g(state, 2, 7, 8, 13, m[12], m[13]);
        g(state, 3, 4, 9, 14, m[14], m[15]);
    }

    /// The message schedule for the next round.
    fn permute(m: &mut [u32; 16]) {
        let mut permuted = [0_u32; 16];
        for (slot, source) in permuted.iter_mut().zip(MSG_PERMUTATION) {
            *slot = m[source];
        }
        *m = permuted;
    }

    /// The compression function, with the round count as a parameter.
    ///
    /// The parameter exists for exactly one caller — the anti-vacuity control that runs it
    /// at `ROUNDS - 1` and requires the published vectors to *fail*. Nothing else may pass
    /// anything but [`ROUNDS`].
    fn compress_rounds(
        chaining_value: &[u32; 8],
        block_words: &[u32; 16],
        counter: u64,
        block_len: u32,
        flags: u32,
        rounds: usize,
    ) -> [u32; 8] {
        let mut state = [
            chaining_value[0],
            chaining_value[1],
            chaining_value[2],
            chaining_value[3],
            chaining_value[4],
            chaining_value[5],
            chaining_value[6],
            chaining_value[7],
            IV[0],
            IV[1],
            IV[2],
            IV[3],
            counter as u32,
            (counter >> 32) as u32,
            block_len,
            flags,
        ];
        let mut block = *block_words;
        for index in 0..rounds {
            round(&mut state, &block);
            if index + 1 < rounds {
                permute(&mut block);
            }
        }
        let mut output = [0_u32; 8];
        for (index, word) in output.iter_mut().enumerate() {
            *word = state[index] ^ state[index + 8];
        }
        output
    }

    /// The sixteen little-endian words of a block, zero-padded to 64 bytes.
    fn block_words(block: &[u8]) -> [u32; 16] {
        let mut padded = [0_u8; BLOCK_LEN];
        padded[..block.len()].copy_from_slice(block);
        let mut words = [0_u32; 16];
        for (index, word) in words.iter_mut().enumerate() {
            let start = index * 4;
            *word = u32::from_le_bytes([
                padded[start],
                padded[start + 1],
                padded[start + 2],
                padded[start + 3],
            ]);
        }
        words
    }

    /// One chunk's chaining value, with `extra` folded into the final block's flags.
    fn chunk_chaining_value(chunk: &[u8], counter: u64, extra: u32, rounds: usize) -> [u32; 8] {
        let blocks: Vec<&[u8]> = if chunk.is_empty() {
            vec![&[]]
        } else {
            chunk.chunks(BLOCK_LEN).collect()
        };
        let last = blocks.len() - 1;
        let mut chaining_value = IV;
        for (index, block) in blocks.iter().enumerate() {
            let mut flags = 0;
            if index == 0 {
                flags |= CHUNK_START;
            }
            if index == last {
                flags |= CHUNK_END | extra;
            }
            chaining_value = compress_rounds(
                &chaining_value,
                &block_words(block),
                counter,
                block.len() as u32,
                flags,
                rounds,
            );
        }
        chaining_value
    }

    /// A parent node's chaining value, with `extra` folded into its flags.
    fn parent_chaining_value(
        left: &[u32; 8],
        right: &[u32; 8],
        extra: u32,
        rounds: usize,
    ) -> [u32; 8] {
        let mut words = [0_u32; 16];
        words[..8].copy_from_slice(left);
        words[8..].copy_from_slice(right);
        compress_rounds(&IV, &words, 0, BLOCK_LEN as u32, PARENT | extra, rounds)
    }

    /// The largest power-of-two multiple of [`CHUNK_LEN`] strictly below `len`.
    fn left_len(len: usize) -> usize {
        let mut split = CHUNK_LEN;
        while split * 2 < len {
            split *= 2;
        }
        split
    }

    /// A non-root subtree's chaining value.
    fn subtree_chaining_value(input: &[u8], counter: u64, rounds: usize) -> [u32; 8] {
        if input.len() <= CHUNK_LEN {
            return chunk_chaining_value(input, counter, 0, rounds);
        }
        let split = left_len(input.len());
        let left = subtree_chaining_value(&input[..split], counter, rounds);
        let right = subtree_chaining_value(
            &input[split..],
            counter + (split / CHUNK_LEN) as u64,
            rounds,
        );
        parent_chaining_value(&left, &right, 0, rounds)
    }

    /// The 32-byte digest, at an arbitrary round count.
    ///
    /// Public only so the anti-vacuity control can reach it; every other caller goes
    /// through [`digest`].
    pub fn digest_rounds(input: &[u8], rounds: usize) -> [u8; 32] {
        let root = if input.len() <= CHUNK_LEN {
            chunk_chaining_value(input, 0, ROOT, rounds)
        } else {
            let split = left_len(input.len());
            let left = subtree_chaining_value(&input[..split], 0, rounds);
            let right = subtree_chaining_value(&input[split..], (split / CHUNK_LEN) as u64, rounds);
            parent_chaining_value(&left, &right, ROOT, rounds)
        };
        let mut bytes = [0_u8; 32];
        for (index, word) in root.iter().enumerate() {
            bytes[index * 4..index * 4 + 4].copy_from_slice(&word.to_le_bytes());
        }
        bytes
    }

    /// The BLAKE3 digest of `input`.
    #[must_use]
    pub fn digest(input: &[u8]) -> [u8; 32] {
        digest_rounds(input, ROUNDS)
    }

    /// The digest as lowercase hex — the spelling an artifact handle's identity half uses.
    #[must_use]
    pub fn token(input: &[u8]) -> String {
        digest(input)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect()
    }
}

/// The identity half this file expects a handle for `content` to carry.
///
/// One function, so that no test can quietly reach for a different recomputation.
fn independent_identity(content: &[u8]) -> String {
    spec_blake3::token(content)
}

/// The full wire spelling this file expects for `class` over `content`.
fn independent_handle(class: ArtifactClass, content: &[u8]) -> String {
    format!("{}{}", class.prefix(), independent_identity(content))
}

// =========================================================================================
// fixtures
// =========================================================================================

/// The Die Hard Intent Contract, as `continuum-intent`'s own suites use it.
///
/// `include_str!` rather than a copy: there is one Die Hard contract in this repository,
/// and a second would be a second Die Hard that could drift from the first.
const DIE_HARD_CONTRACT: &str =
    include_str!("../../continuum-intent/tests/fixtures/die-hard-contract.json");

/// The TV-009 port's model, verbatim.
const DIE_HARD_MODEL: &str =
    include_str!("../../../notes/plan/corpus/tla-examples/ports/TV-009/DieHard.ctm");

/// The TV-009 port's default model configuration.
const DIE_HARD_CONFIG: &str =
    include_str!("../../../notes/plan/corpus/tla-examples/ports/TV-009/default.model.toml");

/// The intent-registry-record schema document, read for its `$id` alone.
const REGISTRY_RECORD_SCHEMA: &str =
    include_str!("../../../notes/plan/schemas/intent-registry-record.schema.json");

/// A second file's bytes, so a second `workspace.create` names different components.
const SECOND_FILE: &str = "# a second file, so this snapshot is not the first one\n";

/// Where the TV-009 module sits in the snapshot, and in the model-source preimage.
const MODULE_PATH: &str = "DieHard.ctm";

fn version() -> ProtocolVersion {
    ProtocolVersion::new(3, 1)
}

fn cap(handle: &str) -> CapabilityHandle {
    CapabilityHandle::new(handle).expect("a well-formed capability handle")
}

fn who(actor: &str) -> ActorId {
    ActorId::new(actor).expect("a well-formed actor identity")
}

fn name(operation: &str) -> OperationName {
    OperationName::new(operation).expect("a well-formed operation name")
}

fn epoch(token: &str) -> EpochIdentity {
    EpochIdentity::new(token).expect("a well-formed epoch identity")
}

fn now() -> Timestamp {
    Timestamp::new("2026-08-01T00:00:00.000Z").expect("a well-formed timestamp")
}

fn profile(privileged: &[&str]) -> CapabilityProfile {
    CapabilityProfile {
        privileged_operations: privileged.iter().map(|entry| name(entry)).collect(),
        denied_operations: Vec::new(),
        data_grants: Vec::new(),
        cross_principal_sharing: true,
    }
}

fn grant(
    handle: &str,
    actor: &str,
    level: AuthorityLevel,
    depth: u32,
    profile: Optional<CapabilityProfile>,
) -> CapabilityDescriptor {
    CapabilityDescriptor {
        capability: cap(handle),
        actor: who(actor),
        level,
        snapshots: Vec::new(),
        intents: Vec::new(),
        artifact_classes: Vec::new(),
        expires_at: Nullable::Null,
        delegation_depth: depth,
        profile,
    }
}

fn negotiated() -> Negotiated {
    let hello = ClientHello {
        protocol_versions: VersionRange {
            low: version(),
            high: version(),
        },
        encodings: vec![Encoding::CanonicalJson],
        client: "continuumd-gate-g1-01-acceptance".to_owned(),
        actor: who("service:continuumd"),
        capability: cap("cap_root"),
        features: Optional::Absent,
    };
    negotiate(&[version()], ProtocolWindow::new(3), ENCODINGS, &hello).expect("3.1 is served")
}

/// The five compatibility epochs this file's primary deployment pins.
///
/// `evidence` and `corpus` are left `Null` deliberately, matching the exit package's own
/// deployment (§8.2) so that a divergence between this file and that one is a real
/// divergence rather than a configuration difference.
fn epochs() -> EpochSet {
    EpochSet {
        protocol: version(),
        semantic: Nullable::Value(epoch("semantic-1")),
        intent: Nullable::Value(epoch("intent-1")),
        evidence: Nullable::Null,
        proof: Nullable::Value(epoch("proof-1")),
        corpus: Nullable::Null,
        engine: Nullable::Value(epoch("engine-reference-1")),
    }
}

/// A daemon over an arbitrary identity seam.
///
/// Generic in the seam so that the falsification control can install a deliberately broken
/// one and drive the *same* script through it. Every non-control caller passes
/// [`Blake3Identity`].
fn daemon_over<I>(identifier: I, pinned: bool) -> Daemon
where
    I: ContentIdentifier + Clone + 'static,
{
    let root = Some(cap("cap_root"));
    let mut builder = Daemon::builder(identifier, negotiated(), cap("cap_root")).now(now());
    if pinned {
        builder = builder.epochs(epochs());
    }
    builder
        .capability(
            grant(
                "cap_root",
                "service:continuumd",
                AuthorityLevel::Promote,
                4,
                Optional::Present(profile(&["intent.accept", "intent.reject", "intent.lock"])),
            ),
            None,
        )
        .capability(
            grant(
                "cap_builder",
                "agent:builder",
                AuthorityLevel::Propose,
                3,
                Optional::Absent,
            ),
            root.clone(),
        )
        .capability(
            grant(
                "cap_runner",
                "agent:runner",
                AuthorityLevel::Execute,
                3,
                Optional::Absent,
            ),
            root.clone(),
        )
        .capability(
            grant(
                "cap_steward",
                "human:steward",
                AuthorityLevel::ReviseIntent,
                3,
                Optional::Present(profile(&["intent.accept", "intent.reject", "intent.lock"])),
            ),
            root,
        )
        .family(WorkspaceFamily)
        .family(IntentFamily)
        .family(TaskFamily)
        .family(VerificationFamily)
        .build()
}

fn envelope(operation: &str, actor: &str, capability: &str, request: &str) -> RequestEnvelope {
    RequestEnvelope {
        protocol_version: version(),
        request_id: RequestId::new(request).expect("a well-formed request id"),
        idempotency_key: Optional::Absent,
        actor: who(actor),
        capability: cap(capability),
        operation: name(operation),
        snapshot: Nullable::Null,
        intent: Nullable::Null,
        arguments: Opaque::from_bytes(Vec::new()),
        budget: Optional::Absent,
        output_policy: Optional::Absent,
        trace: Optional::Absent,
        page: Optional::Absent,
    }
}

fn keyed(mut envelope: RequestEnvelope, key: &str) -> RequestEnvelope {
    envelope.idempotency_key = Optional::Present(key.to_owned());
    envelope
}

/// The `states` ceiling used by every `verification.start` here.
const STATE_CEILING: u64 = 64;

fn budgeted(mut envelope: RequestEnvelope, states: u64) -> RequestEnvelope {
    envelope.budget = Optional::Present(budget(states));
    envelope
}

fn budget(states: u64) -> Budget {
    Budget {
        wall_ms: Optional::Absent,
        cpu_ms: Optional::Absent,
        memory_bytes: Optional::Absent,
        states: Optional::Present(states),
        solver_ms: Optional::Absent,
        proof_ms: Optional::Absent,
        tokens: Optional::Absent,
        candidates: Optional::Absent,
        bytes: Optional::Absent,
    }
}

fn on(mut envelope: RequestEnvelope, snapshot: &WorkspaceHandle) -> RequestEnvelope {
    envelope.snapshot = Nullable::Value(snapshot.clone());
    envelope
}

fn die_hard_contract() -> IntentContract {
    IntentContract::decode(DIE_HARD_CONTRACT.trim_end().as_bytes()).expect("the fixture decodes")
}

/// The `in_*` handle a contract's ID1/ID2 preimage names, **recomputed independently**.
///
/// This is the fixture's own registration key, and it is derived with [`spec_blake3`]
/// rather than with the daemon's seam on purpose: the contract is put into the registry
/// under a name this file computed, so every later wire answer that repeats that name is
/// already agreeing with the independent digest.
fn intent_handle(contract: &IntentContract) -> IntentHandle {
    IntentHandle::new(&independent_handle(
        ArtifactClass::IntentContract,
        &contract.identity_preimage_bytes(),
    ))
    .expect("a lowercase-hex digest is a well-formed `in_` identity")
}

fn acceptance_bytes(signature: &str) -> Opaque {
    let mut fields: BTreeMap<String, Json> = BTreeMap::new();
    for (key, value) in [
        ("accepted_by", "human:steward"),
        ("capability", "revise-intent"),
        ("signature", signature),
        ("audit_record", "supplied-by-the-caller-and-overwritten"),
        ("timestamp", "2026-08-01T00:00:00.000Z"),
    ] {
        fields.insert(key.to_owned(), Json::String(value.to_owned()));
    }
    Opaque::from_bytes(Json::Object(fields).to_canonical_bytes())
}

/// How far [`rig_over`] drives the script before handing the rig back.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Stage {
    /// The contract is registered as `proposed`, content is staged, the model is in the
    /// catalog — and nothing has been accepted or created. `workspace.create` refuses an
    /// unaccepted intent (`daemon/workspace.rs`: "only an accepted contract governs"), so
    /// this stage has no snapshot and its [`Rig::snapshot`] is a placeholder.
    Registered,
    /// [`Stage::Registered`], then `intent.accept` and a sealing `workspace.create`, both
    /// through [`Daemon::dispatch`].
    Sealed,
}

/// The workspace under test: an accepted contract, staged content, and a sealed snapshot.
struct Rig {
    daemon: Daemon,
    intent: IntentHandle,
    /// The staged Die Hard files, in `workspace.create` order.
    files: Vec<Commitment>,
    /// A staged second file, for the "different components" attack.
    extra: Commitment,
    configuration: Commitment,
    snapshot: WorkspaceHandle,
}

impl Rig {
    /// The store token `cap_root` names — audit and read authority.
    fn root_token() -> CapabilityToken {
        identity::capability_to_store(&cap("cap_root")).expect("`cap_root` is a store token")
    }

    /// Every published identity, with the bytes the store holds for it.
    ///
    /// Read through the store's own capability-checked surface, so nothing here reaches
    /// past an authorization the daemon would enforce.
    fn published(&self) -> Vec<(ArtifactHandle, Vec<u8>)> {
        let token = Self::root_token();
        let view = self
            .daemon
            .store()
            .audit_view(&token)
            .expect("`cap_root` confers audit");
        view.identities()
            .into_iter()
            .map(|handle| {
                let content = self
                    .daemon
                    .store()
                    .read(&handle, &token)
                    .expect("`cap_root` confers read on a published artifact");
                (handle, content)
            })
            .collect()
    }

    fn components(&self, files: Vec<Commitment>) -> SnapshotComponents {
        SnapshotComponents {
            files,
            cml_modules: Vec::new(),
            rust_extraction: Vec::new(),
            domain_packs: Vec::new(),
            dependencies: Vec::new(),
            epochs: SnapshotEpochs {
                semantic: epoch("semantic-1"),
                proof: epoch("proof-1"),
                toolchain: Optional::Absent,
            },
            intent: self.intent.clone(),
            correspondence: Vec::new(),
            proof_environment: Vec::new(),
            configuration: vec![self.configuration.clone()],
            file_components: Optional::Absent,
        }
    }

    fn create(&mut self, request: &str, key: &str, files: Vec<Commitment>) -> OperationOutcome {
        let components = self.components(files);
        self.daemon.dispatch(&OperationRequest {
            envelope: keyed(
                envelope("workspace.create", "agent:builder", "cap_builder", request),
                key,
            ),
            arguments: Arguments::WorkspaceCreate(WorkspaceCreateRequest {
                components,
                overlay: Optional::Absent,
                seal: Optional::Present(true),
            }),
        })
    }

    fn get_intent(&mut self, intent: &IntentHandle, request: &str) -> OperationOutcome {
        self.daemon.dispatch(&OperationRequest {
            envelope: envelope("intent.get", "agent:builder", "cap_builder", request),
            arguments: Arguments::IntentGet(IntentGetRequest {
                intent: intent.clone(),
            }),
        })
    }

    /// The contract document `intent.get` answers with, as bytes.
    fn contract_bytes(&mut self, intent: &IntentHandle, request: &str) -> Vec<u8> {
        let got = self.get_intent(intent, request);
        ok(&got, "intent.get");
        match &got.payload {
            Payload::IntentGet(response) => response.contract.as_bytes().to_vec(),
            other => panic!("expected an intent.get payload, got {other:?}"),
        }
    }

    /// The registry record `intent.get` answers with, as bytes.
    fn registry_record(&mut self, intent: &IntentHandle, request: &str) -> Vec<u8> {
        let got = self.get_intent(intent, request);
        ok(&got, "intent.get");
        match &got.payload {
            Payload::IntentGet(response) => response.record.as_bytes().to_vec(),
            other => panic!("expected an intent.get payload, got {other:?}"),
        }
    }

    fn accept(&mut self, request: &str, key: &str, signature: &str) -> OperationOutcome {
        let intent = self.intent.clone();
        self.daemon.dispatch(&OperationRequest {
            envelope: keyed(
                envelope("intent.accept", "human:steward", "cap_steward", request),
                key,
            ),
            arguments: Arguments::IntentAccept(IntentAcceptRequest {
                proposal: intent,
                acceptance: acceptance_bytes(signature),
                bundle: Optional::Absent,
            }),
        })
    }

    /// Tighten one policy field through `intent.lock`, which RFC 0037 ID3 makes mint a
    /// successor contract under a new identity.
    fn lock(
        &mut self,
        request: &str,
        key: &str,
        field: PolicyField,
        verb: PolicyVerb,
    ) -> IntentHandle {
        let intent = self.intent.clone();
        let mut policy = BTreeMap::new();
        policy.insert(field.wire().to_owned(), verb.wire().to_owned());
        let locked = self.daemon.dispatch(&OperationRequest {
            envelope: keyed(
                envelope("intent.lock", "human:steward", "cap_steward", request),
                key,
            ),
            arguments: Arguments::IntentLock(IntentLockRequest { intent, policy }),
        });
        ok(&locked, "intent.lock");
        match &locked.payload {
            Payload::IntentLock(response) => response.intent.clone(),
            other => panic!("expected an intent.lock payload, got {other:?}"),
        }
    }

    fn start(&mut self, request: &str, key: &str) -> OperationOutcome {
        let snapshot = self.snapshot.clone();
        self.daemon.dispatch(&OperationRequest {
            envelope: on(
                budgeted(
                    keyed(
                        envelope("verification.start", "agent:runner", "cap_runner", request),
                        key,
                    ),
                    STATE_CEILING,
                ),
                &snapshot,
            ),
            arguments: Arguments::VerificationStart(VerificationStartRequest {
                target: Target {
                    kind: TargetKind::Property,
                    id: "NotSolved".to_owned(),
                },
                portfolio: Portfolio::Interactive,
                context_policy: Optional::Absent,
                priority_class: Optional::Absent,
            }),
        })
    }
}

fn ok(outcome: &OperationOutcome, what: &str) {
    assert!(
        matches!(
            outcome.envelope.status,
            ResultStatus::Ok | ResultStatus::TaskStarted
        ),
        "{what} did not succeed: {:?} {:?}",
        outcome.envelope.status,
        outcome.envelope.error
    );
}

fn snapshot_of(outcome: &OperationOutcome) -> WorkspaceHandle {
    match &outcome.payload {
        Payload::WorkspaceCreate(response) => response.snapshot.clone(),
        other => panic!("expected a workspace.create payload, got {other:?}"),
    }
}

/// The rig, over the given identity seam and epoch pinning.
///
/// Every step runs through [`Daemon::dispatch`]: the intent is accepted on the wire, the
/// workspace is created and sealed on the wire, and the model is registered in the catalog
/// out of band because this daemon has no CML front end (`daemon/verification.rs`).
fn rig_over<I>(identifier: I, pinned: bool, stage: Stage) -> Rig
where
    I: ContentIdentifier + Clone + 'static,
{
    let mut daemon = daemon_over(identifier, pinned);
    let contract = die_hard_contract();
    let intent = intent_handle(&contract);
    daemon.state_mut().put_intent(
        intent.clone(),
        IntentRecord {
            contract,
            status: RegistryStatus::Proposed,
            supersedes: None,
            superseded_by: None,
            acceptance: None,
        },
    );

    let mut files = Vec::new();
    for (path, content) in [("DieHard.ctm", DIE_HARD_MODEL), ("README.md", "# TV-009\n")] {
        files.push(
            daemon
                .state_mut()
                .stage(
                    &Blake3Identity,
                    WorkspacePath::new(path).expect("a workspace path"),
                    content.as_bytes().to_vec(),
                )
                .expect("staging names its content"),
        );
    }
    let extra = daemon
        .state_mut()
        .stage(
            &Blake3Identity,
            WorkspacePath::new("SECOND.md").expect("a workspace path"),
            SECOND_FILE.as_bytes().to_vec(),
        )
        .expect("staging names its content");
    let configuration = daemon
        .state_mut()
        .stage(
            &Blake3Identity,
            WorkspacePath::new("default.model.toml").expect("a workspace path"),
            DIE_HARD_CONFIG.as_bytes().to_vec(),
        )
        .expect("staging names its content");

    daemon.state_mut().models_mut().register(
        model_source(&Blake3Identity, [(MODULE_PATH, DIE_HARD_MODEL.as_bytes())])
            .expect("the module set is nameable"),
        continuum_engine_reference::diehard::model().expect("the TV-009 port builds"),
    );

    let mut rig = Rig {
        daemon,
        intent,
        files: files.clone(),
        extra,
        configuration,
        snapshot: WorkspaceHandle::new("ws_placeholder").expect("a placeholder handle"),
    };
    if stage == Stage::Registered {
        return rig;
    }

    let accepted = rig.accept("req_accept", "idem-accept", "sig-die-hard-v1");
    ok(&accepted, "intent.accept");
    let created = rig.create("req_create", "idem-create", files);
    ok(&created, "workspace.create");
    rig.snapshot = snapshot_of(&created);
    rig
}

/// The rig this file's probes run against: BLAKE3, epochs pinned, snapshot sealed.
fn rig() -> Rig {
    rig_over(Blake3Identity, true, Stage::Sealed)
}

// =========================================================================================
// A — the instrument, controlled before it is trusted
// =========================================================================================

/// The specification's own vector inputs: `i % 251`, repeating.
fn vector_input(len: usize) -> Vec<u8> {
    (0..len).map(|index| (index % 251) as u8).collect()
}

/// The published BLAKE3 vectors, at lengths straddling the 1024-byte chunk boundary in
/// both directions so the single-chunk path and the tree path are both anchored.
const PUBLISHED_VECTORS: [(usize, &str); 8] = [
    (
        0,
        "af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262",
    ),
    (
        1,
        "2d3adedff11b61f14c886e35afa036736dcd87a74d27b5c1510225d0f592e213",
    ),
    (
        2,
        "7b7015bb92cf0b318037702a6cdd81dee41224f734684c2c122cd6359cb1ee63",
    ),
    (
        3,
        "e1be4d7a8ab5560aa4199eea339849ba8e293d55ca0a81006726d184519e647f",
    ),
    (
        1023,
        "10108970eeda3eb932baac1428c7a2163b0e924c9a9e25b35bba72b28f70bd11",
    ),
    (
        1024,
        "42214739f095a406f3fc83deb889744ac00df831c10daa55189b5d121c855af7",
    ),
    (
        1025,
        "d00278ae47eb27b34faecf67b4fe263f82d5412916c1ffd97c8cb7fb814b8444",
    ),
    (
        2048,
        "e776b6028c7cd22a4d0ba182a8bf62205d2ef576467e838ed6f2529b85fba24a",
    ),
];

/// **Control (positive).** The instrument is BLAKE3.
///
/// The literals are the published specification vectors — the same eight
/// `continuum-value`'s own `blake3_matches_the_published_test_vectors` pins its vendored
/// crate against, plus the widely published `blake3("abc")`. Anchoring both
/// implementations to the *same external* constants is what makes their later agreement
/// evidence rather than coincidence: two implementations that agreed only with each other
/// could be wrong together, and two that each match the published vectors cannot be.
///
/// This test is the reason every other test in this file is allowed to say
/// "independently recomputed".
#[test]
fn control_the_instrument_reproduces_the_published_blake3_vectors() {
    for (len, expected) in PUBLISHED_VECTORS {
        assert_eq!(
            independent_identity(&vector_input(len)),
            expected,
            "the instrument disagrees with the published vector at input length {len}"
        );
    }
    assert_eq!(
        independent_identity(b"abc"),
        "6437b3ac38465133ffb63b75273a8db548c558465d79db03fd359c6cd5bd9d85"
    );
}

/// **Control (negative).** The vectors above are load-bearing on the *algorithm*, not on
/// the shape of the output.
///
/// A 256-bit-wide function that is not BLAKE3 would still produce sixty-four hex
/// characters and would still round-trip through `ArtifactHandle`, so a check that only
/// asserted "the identity is a 64-character token" would pass against any hash at all.
/// Running the instrument's own compression one round short is the cheapest way to build
/// a function that is exactly that: same width, same spelling, same determinism, wrong
/// answer. Every published vector must reject it.
///
/// If this test ever passes vacuously — because `digest_rounds` stopped consulting its
/// round count, say — the assertion below fails on the very first vector.
#[test]
fn control_a_round_short_variant_of_the_instrument_fails_the_published_vectors() {
    for (len, expected) in PUBLISHED_VECTORS {
        let short: String = spec_blake3::digest_rounds(&vector_input(len), spec_blake3::ROUNDS - 1)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();
        assert_eq!(short.len(), 64, "the mutant is still 256 bits wide");
        assert_ne!(
            short,
            expected,
            "a {}-round compression reproduced the published BLAKE3 vector at length {len}; \
             the vector is not pinning the algorithm",
            spec_blake3::ROUNDS - 1
        );
    }
}

/// **Control (negative).** Distinct contents never receive one identity, and equal
/// contents always do.
///
/// The pair of assertions is deliberate. Injectivity alone is satisfied by a counter, and
/// convergence alone by a constant; only both together describe a content address. The
/// corpus is every artifact the rig's full script publishes plus their one-bit
/// perturbations, so this is run over real bytes rather than over invented ones.
#[test]
fn control_distinct_contents_never_share_an_identity_and_equal_contents_always_do() {
    let mut rig = rig();
    ok(&rig.start("req_start", "idem-start"), "verification.start");

    let published = rig.published();
    assert!(
        published.len() >= 2,
        "the script published {} artifacts; this control needs at least two",
        published.len()
    );

    let mut corpus: Vec<Vec<u8>> = Vec::new();
    for (_, content) in &published {
        corpus.push(content.clone());
        let mut flipped = content.clone();
        if let Some(first) = flipped.first_mut() {
            *first ^= 0x01;
        } else {
            flipped.push(0);
        }
        corpus.push(flipped);
    }

    let mut seen: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    for content in corpus {
        let identity = independent_identity(&content);
        match seen.get(&identity) {
            Some(previous) => assert_eq!(
                previous, &content,
                "two different byte strings received one identity {identity}"
            ),
            None => {
                seen.insert(identity, content);
            }
        }
    }

    // Convergence, stated as its own assertion rather than inferred from the absence of a
    // collision above: the same bytes hashed twice are the same identity.
    for (_, content) in &published {
        assert_eq!(
            independent_identity(content),
            independent_identity(&content.clone()),
            "the instrument is not a function of the bytes alone"
        );
    }
}

// =========================================================================================
// the two counterfeit identity seams the controls install
// =========================================================================================

/// A 256-bit-wide, deterministic, non-cryptographic identity seam that is **not** BLAKE3.
///
/// Four FNV-1a lanes over the same bytes, each with a different offset basis, rendered as
/// one 64-character lowercase-hex token. Everything the store asks of a
/// [`ContentIdentifier`] holds: it is pure in its arguments, injective in practice over the
/// inputs this file feeds it, and its tokens are inside the identity character class. A
/// daemon built over it therefore behaves exactly like the production one at every seam a
/// *self*-check can see — its publications converge and its handles round-trip. A check
/// against the *declared* identifier —
/// [`fsck`](continuum_workspace::publication::StoreAudit::fsck), since bn-k99dt — does catch
/// this particular substitution, because it is a wholesale swap of `ContentIdentifier` and
/// not a flaw inside `Blake3Identity` itself.
///
/// It exists so this file can state what it is really claiming. The claim is not "the
/// store agrees with the declared seam"; it is "the identity is the BLAKE3 digest of the
/// content", which survives even a declared-seam check that happens to be `Blake3Identity`
/// itself, and
/// [`negative_control_a_non_blake3_seam_is_self_consistent_and_fails_this_files_predicate`]
/// is what separates the two.
#[derive(Debug, Clone, Copy, Default)]
struct Fnv1a256Identity;

impl Fnv1a256Identity {
    /// One FNV-1a 64-bit lane.
    fn lane(content: &[u8], basis: u64) -> u64 {
        let mut hash = basis;
        for byte in content {
            hash ^= u64::from(*byte);
            hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
        }
        hash
    }

    /// The 64-character token.
    fn token(content: &[u8]) -> String {
        [
            0xcbf2_9ce4_8422_2325,
            0x9e37_79b9_7f4a_7c15,
            0xc2b2_ae3d_27d4_eb4f,
            0x1656_67b1_9e37_79f9,
        ]
        .iter()
        .map(|basis| format!("{:016x}", Self::lane(content, *basis)))
        .collect()
    }
}

impl ContentIdentifier for Fnv1a256Identity {
    fn identify(
        &self,
        class: ArtifactClass,
        content: &[u8],
    ) -> Result<ArtifactHandle, IdentityUnavailable> {
        ArtifactHandle::new(class, &Self::token(content)).map_err(|_| IdentityUnavailable)
    }
}

/// A seam that names every equal-length content identically.
///
/// The only way to reach the store's same-identity-different-content branch at all: BLAKE3
/// cannot be made to collide on demand, so the refusal that branch implements would be
/// dead, unexercised code without a seam that collides by construction. Used by exactly one
/// test, [`immutability_the_store_refuses_new_content_under_a_held_identity`], and never to
/// drive the workspace script.
#[derive(Debug, Clone, Copy, Default)]
struct EqualLengthIdentity;

impl ContentIdentifier for EqualLengthIdentity {
    fn identify(
        &self,
        class: ArtifactClass,
        content: &[u8],
    ) -> Result<ArtifactHandle, IdentityUnavailable> {
        ArtifactHandle::new(class, &format!("len{:016x}", content.len()))
            .map_err(|_| IdentityUnavailable)
    }
}

// =========================================================================================
// B — content-addressing round trip, class by class
// =========================================================================================

/// **Snapshots.** Every `ws_*` record the store holds is the independent BLAKE3 digest of
/// the bytes it holds under it, and the handle `workspace.create` answered with is one of
/// them.
///
/// The second half is what makes the first half about G1-01 rather than about the store in
/// isolation: a client that received `ws_X` and a store that files bytes under `ws_X` must
/// be naming one thing, or "content-addressed" would be an internal property no caller
/// could use.
///
/// Nothing here is transcribed. The bytes are read back through
/// [`ReferenceStore::read`](continuum_workspace::publication::ReferenceStore::read) under a
/// real capability, and the expected identity is [`spec_blake3`]'s digest of exactly those
/// bytes.
#[test]
fn snapshots_are_the_independent_digest_of_the_bytes_the_store_holds() {
    let rig = rig();
    let published = rig.published();
    let snapshots: Vec<_> = published
        .iter()
        .filter(|(handle, _)| handle.class() == ArtifactClass::WorkspaceSnapshot)
        .collect();
    assert!(
        !snapshots.is_empty(),
        "the sealing `workspace.create` published no `ws_*` record, so this check would \
         hold vacuously"
    );

    for (handle, content) in &snapshots {
        assert_eq!(
            handle.identity(),
            independent_identity(content),
            "the store files {handle} under an identity that is not the independent BLAKE3 \
             digest of the {} bytes it holds there",
            content.len()
        );
    }

    let wire = rig.snapshot.as_str();
    assert!(
        snapshots
            .iter()
            .any(|(handle, _)| handle.to_string() == wire),
        "`workspace.create` answered {wire}, which is not one of the {} `ws_*` records the \
         store holds: the wire name and the store name are not one name",
        snapshots.len()
    );
}

/// **Intent contracts.** The `in_*` identity the daemon *itself derives* for a successor
/// contract is the independent BLAKE3 digest of the ID1/ID2 preimage of the contract
/// document that same daemon returns on the wire.
///
/// `intent.lock` is the operation that makes this a real derivation rather than an echo.
/// The predecessor's handle is a name this file chose when it registered the contract, so a
/// daemon that only ever repeated its argument would pass a check against it. RFC 0037 ID3
/// makes `intent.lock` mint a **successor**: the daemon computes that name with no input
/// from the caller. This test then fetches the successor with `intent.get`, decodes the
/// returned document with `IntentContract::decode`, recomputes the preimage, and requires
/// the daemon's name to be the digest of it.
///
/// The scope narrowing this file reports lives here, as an assertion rather than a note:
/// the digest is over `identity_preimage_bytes()`, **not** over the artifact document, and
/// the last assertion below shows the two differ. `in_*` is a content address of a
/// specified *function of* the contract, so an independent check must transcribe that
/// function — which is exactly what `identity_preimage_bytes` is public for
/// (`continuum-intent`: "Public so that the exclusion list is checkable").
#[test]
fn intent_contracts_are_the_independent_digest_of_the_wire_contracts_preimage() {
    let mut rig = rig();

    // The predecessor: registered under a name this file computed, so agreement here says
    // only that the daemon did not rename it.
    let predecessor = rig.intent.clone();
    let before = rig.contract_bytes(&predecessor, "req_get_before");
    let decoded = IntentContract::decode(&before).expect("the wire document decodes");
    assert_eq!(
        predecessor.as_str(),
        independent_handle(
            ArtifactClass::IntentContract,
            &decoded.identity_preimage_bytes()
        ),
        "the registered contract's own wire document does not re-derive its handle"
    );

    // The successor: a name the daemon derived, with nothing from this file in it.
    let successor = rig.lock(
        "req_lock",
        "idem-lock",
        PolicyField::AbstractionMaps,
        PolicyVerb::Locked,
    );
    assert_ne!(
        successor, predecessor,
        "`intent.lock` is a governance edit inside the ID2 preimage, so it must mint a new \
         identity (RFC 0037 ID3)"
    );

    let document = rig.contract_bytes(&successor, "req_get_successor");
    let successor_contract = IntentContract::decode(&document).expect("the wire document decodes");
    assert_eq!(
        successor.as_str(),
        independent_handle(
            ArtifactClass::IntentContract,
            &successor_contract.identity_preimage_bytes()
        ),
        "the daemon-derived successor identity is not the independent digest of the \
         successor document's ID1/ID2 preimage"
    );

    // The scope statement, executable: the address is over the preimage, not the document.
    assert_ne!(
        successor_contract.identity_preimage_bytes(),
        document,
        "the ID2 preimage and the artifact document are byte-identical here, so this file \
         cannot claim the two shapes are distinguishable"
    );
    assert_ne!(
        successor.as_str(),
        independent_handle(ArtifactClass::IntentContract, &document),
        "the `in_*` identity happens to equal the digest of the whole document, which \
         would make the preimage rule unobservable"
    );
}

/// **Artifacts.** The `task_*` record the campaign publishes is the independent BLAKE3
/// digest of the bytes the store holds for it, and the `ArtifactRef` the wire returns names
/// that identity in its `commitment`.
///
/// This is the artifact-class half of G1-01 driven through a real campaign:
/// `verification.start` explores the TV-009 port, publishes its campaign record through the
/// store's three-phase protocol, and answers with an `ArtifactRef`. Both names on that ref
/// are checked, and the check that they are *different* names is [`scope_the_two_content_addressing_shapes_are_not_one_shape`].
#[test]
fn task_artifacts_are_the_independent_digest_of_their_stored_record() {
    let mut rig = rig();
    let start = rig.start("req_start", "idem-start");
    ok(&start, "verification.start");

    let published = rig.published();
    let records: Vec<_> = published
        .iter()
        .filter(|(handle, _)| handle.class() == ArtifactClass::Task)
        .collect();
    assert_eq!(
        records.len(),
        1,
        "one campaign publishes one `task_*` record; found {}",
        records.len()
    );
    let (handle, content) = records[0];
    assert_eq!(
        handle.identity(),
        independent_identity(content),
        "the campaign record at {handle} is not the independent digest of its {} stored \
         bytes",
        content.len()
    );

    let refs = &start.envelope.artifacts;
    assert_eq!(refs.len(), 1, "expected one artifact ref, got {refs:?}");
    let commitment = refs[0]
        .commitment
        .value()
        .expect("the campaign's artifact ref names its record by commitment");
    assert_eq!(
        commitment.as_str(),
        handle.to_string(),
        "the wire commitment does not name the identity the store filed the record under"
    );
}

/// **Handles.** Every handle this file's whole script puts on the wire is a plan §4.4 class
/// prefix over a sixty-four-character lowercase-hex digest that this file recomputed.
///
/// The check is deliberately over the *transcript* rather than over the store, because
/// G1-01's "handles" clause is about what a caller receives. A handle that were a counter,
/// a UUID, or an opaque server-side token would satisfy every other test in this file that
/// looks only at stored bytes; none would survive the last assertion here, which requires
/// each wire handle to be a member of the set of identities this file independently
/// derived.
#[test]
fn every_handle_the_wire_returns_is_a_class_prefix_over_a_recomputed_digest() {
    let mut rig = rig();
    let start = rig.start("req_start", "idem-start");
    ok(&start, "verification.start");
    let campaign = match &start.payload {
        Payload::VerificationStart(response) => response
            .task
            .value()
            .cloned()
            .expect("a started campaign names its task"),
        other => panic!("expected a verification.start payload, got {other:?}"),
    };
    let successor = rig.lock(
        "req_lock",
        "idem-lock",
        PolicyField::AbstractionMaps,
        PolicyVerb::Locked,
    );
    let successor_document = rig.contract_bytes(&successor, "req_get_successor");
    let predecessor_document = rig.contract_bytes(&rig.intent.clone(), "req_get_predecessor");

    // Everything this file can independently derive an identity for, derived.
    let mut derived: BTreeSet<String> = BTreeSet::new();
    for (handle, content) in rig.published() {
        derived.insert(independent_handle(handle.class(), &content));
    }
    for document in [&predecessor_document, &successor_document] {
        let contract = IntentContract::decode(document).expect("the wire document decodes");
        derived.insert(independent_handle(
            ArtifactClass::IntentContract,
            &contract.identity_preimage_bytes(),
        ));
    }
    derived.insert(independent_handle(
        ArtifactClass::Task,
        campaign_preimage(&rig.snapshot, &rig.intent).bytes(),
    ));

    let transcript = [
        rig.snapshot.as_str().to_owned(),
        rig.intent.as_str().to_owned(),
        successor.as_str().to_owned(),
        campaign.as_str().to_owned(),
        start.envelope.artifacts[0].handle.as_str().to_owned(),
        start.envelope.artifacts[0]
            .commitment
            .value()
            .expect("a commitment")
            .as_str()
            .to_owned(),
    ];

    for handle in &transcript {
        let parsed: ArtifactHandle = handle
            .parse()
            .unwrap_or_else(|error| panic!("{handle} is not a plan §4.4 handle: {error:?}"));
        assert_eq!(
            parsed.identity().len(),
            64,
            "{handle} carries a {}-character identity, not a 256-bit digest",
            parsed.identity().len()
        );
        assert!(
            parsed
                .identity()
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)),
            "{handle} carries an identity outside lowercase hex"
        );
        assert!(
            derived.contains(handle),
            "{handle} crossed the wire and this file could not independently derive it; a \
             handle nobody but the daemon can recompute is an opaque identifier, not a \
             content address"
        );
    }
}

/// The `verification.start` campaign preimage, transcribed from
/// `daemon::verification::start_handle`.
///
/// **This function is a transcription and is labelled as one.** The `task_*` campaign
/// identity is a digest of the request that asked for the campaign, not of any artifact, so
/// an independent party can only recompute it by restating the preimage rule. It is written
/// here through the *public* `Preimage`, `budget_preimage` and `epochs_preimage` — the same
/// pieces the daemon exports precisely so a checker need not restate them — with the field
/// order taken from `start_handle`. A change to that order fails
/// [`every_handle_the_wire_returns_is_a_class_prefix_over_a_recomputed_digest`] and
/// [`scope_the_two_content_addressing_shapes_are_not_one_shape`], which is the intended
/// behaviour: this file's claim about `task_*` is weaker than its claim about `ws_*`, and
/// the weakness is that this transcription exists.
fn campaign_preimage(snapshot: &WorkspaceHandle, intent: &IntentHandle) -> Preimage {
    let mut preimage = Preimage::new();
    preimage.text("verification.start");
    preimage.text(snapshot.as_str());
    preimage.text(intent.as_str());
    preimage.text(TargetKind::Property.as_wire());
    preimage.text("NotSolved");
    preimage.text(Portfolio::Interactive.as_wire());
    preimage.text(PriorityClass::Interactive.as_wire());
    budget_preimage(&mut preimage, &budget(STATE_CEILING));
    epochs_preimage(&mut preimage, &epochs());
    preimage
}

/// **The sweep.** Every artifact of every class the store holds after the full script is
/// the independent BLAKE3 digest of its own bytes.
///
/// The three assertions before the loop are the anti-vacuity apparatus: an empty store, a
/// single-class store, or a store whose index is smaller than the script's publications
/// would each make the loop pass while proving nothing.
#[test]
fn every_artifact_the_store_holds_is_the_independent_digest_of_its_own_bytes() {
    let mut rig = rig();
    ok(&rig.start("req_start", "idem-start"), "verification.start");
    let published = rig.published();

    assert!(
        published.len() >= 2,
        "the store holds {} artifacts; the sweep would be near-vacuous",
        published.len()
    );
    let classes: BTreeSet<ArtifactClass> =
        published.iter().map(|(handle, _)| handle.class()).collect();
    assert!(
        classes.len() >= 2,
        "the store holds one artifact class ({classes:?}); the sweep would say nothing \
         about class independence"
    );
    assert!(
        published.iter().all(|(_, content)| !content.is_empty()),
        "an empty artifact would make its digest a constant this check cannot distinguish"
    );

    for (handle, content) in &published {
        assert_eq!(
            handle.identity(),
            independent_identity(content),
            "{handle} ({} bytes, class {}) is not the independent digest of its content",
            content.len(),
            handle.class()
        );
    }
}

// =========================================================================================
// C — immutability, attacked through every mutation avenue the API exposes
// =========================================================================================

/// **Attack: re-create with different content.** A second `workspace.create` naming
/// different components mints a *new* identity and leaves every byte of the first snapshot
/// where it was.
///
/// This is the shape G1-01's "immutable" clause has to have in a content-addressed store:
/// there is no in-place edit to refuse, because a different tree is a different name. The
/// test is written so that a store that *did* overwrite would fail on the second assertion
/// block, not merely on the handle comparison — every previously published `(handle, bytes)`
/// pair is compared bit for bit after the second create.
#[test]
fn immutability_a_create_with_different_components_mints_a_new_identity() {
    let mut rig = rig();
    let before: Vec<(ArtifactHandle, Vec<u8>)> = rig.published();
    let first = rig.snapshot.clone();

    let mut widened = rig.files.clone();
    widened.push(rig.extra.clone());
    let second = rig.create("req_create_2", "idem-create-2", widened);
    ok(&second, "the second workspace.create");
    let second_handle = snapshot_of(&second);

    assert_ne!(
        second_handle, first,
        "a snapshot over different components received the first snapshot's identity"
    );

    let after: BTreeMap<ArtifactHandle, Vec<u8>> = rig.published().into_iter().collect();
    for (handle, content) in &before {
        let now = after
            .get(handle)
            .unwrap_or_else(|| panic!("{handle} disappeared after the second create"));
        assert_eq!(
            now, content,
            "{handle} holds different bytes after a second, unrelated publication"
        );
    }
    assert!(
        after.len() > before.len(),
        "the second create published nothing new, so nothing about new identities was shown"
    );
}

/// **Attack: re-create with identical content, under a different request and key.** The
/// store converges on one identity and rewrites no byte; the receipt ledger grows, because
/// a receipt is a record of an event and the artifact is not.
///
/// The distinction the last block draws is the one that makes convergence safe: what is
/// append-only (the ledger) and what is immutable (the content) are different things, and a
/// store that "converged" by silently rewriting content would pass the handle comparison
/// and fail here.
#[test]
fn immutability_a_repeated_create_converges_and_rewrites_no_byte() {
    let mut rig = rig();
    let before: BTreeMap<ArtifactHandle, Vec<u8>> = rig.published().into_iter().collect();
    let first = rig.snapshot.clone();

    let token = Rig::root_token();
    let receipts_before: usize = {
        let view = rig
            .daemon
            .store()
            .audit_view(&token)
            .expect("`cap_root` confers audit");
        before
            .keys()
            .map(|handle| view.receipts(handle).len())
            .sum()
    };

    let files = rig.files.clone();
    let again = rig.create("req_create_again", "idem-create-again", files);
    ok(&again, "the repeated workspace.create");
    assert_eq!(
        snapshot_of(&again),
        first,
        "one component set named two snapshots; the address is not a function of content"
    );

    let after: BTreeMap<ArtifactHandle, Vec<u8>> = rig.published().into_iter().collect();
    assert_eq!(
        after.keys().collect::<Vec<_>>(),
        before.keys().collect::<Vec<_>>(),
        "a convergent re-publication changed the set of published identities"
    );
    for (handle, content) in &before {
        assert_eq!(
            after.get(handle),
            Some(content),
            "{handle} was rewritten by a convergent re-publication"
        );
    }

    let receipts_after: usize = {
        let view = rig
            .daemon
            .store()
            .audit_view(&token)
            .expect("`cap_root` confers audit");
        before
            .keys()
            .map(|handle| view.receipts(handle).len())
            .sum()
    };
    assert!(
        receipts_after > receipts_before,
        "the second publication left no receipt ({receipts_before} → {receipts_after}); \
         convergence must still be recorded, or a re-publication is unobservable"
    );
}

/// **Attack: a governance write.** `intent.accept` moves a contract's registry status and
/// moves nothing about the contract.
///
/// RFC 0037 ID4 keeps acceptance and supersession out of the `in_*` preimage, so this is
/// the case where a *mutating, privileged* operation is applied directly to an intent
/// contract and the artifact must come back byte-identical. The comparison is on the wire
/// document, not on an in-memory handle, so a daemon that re-rendered the contract
/// differently after acceptance would fail here even if its identity happened to survive.
#[test]
fn immutability_accepting_an_intent_moves_the_record_and_not_the_contract() {
    let mut rig = rig_over(Blake3Identity, true, Stage::Registered);
    let intent = rig.intent.clone();

    let contract_before = rig.contract_bytes(&intent, "req_get_before");
    let record_before = rig.registry_record(&intent, "req_record_before");

    let accepted = rig.accept("req_accept", "idem-accept", "sig-die-hard-v1");
    ok(&accepted, "intent.accept");

    let contract_after = rig.contract_bytes(&intent, "req_get_after");
    let record_after = rig.registry_record(&intent, "req_record_after");

    assert_eq!(
        contract_after, contract_before,
        "acceptance rewrote the contract document"
    );
    assert_eq!(
        intent.as_str(),
        independent_handle(
            ArtifactClass::IntentContract,
            &IntentContract::decode(&contract_after)
                .expect("the wire document decodes")
                .identity_preimage_bytes()
        ),
        "the contract's identity is no longer the digest of its own preimage after \
         acceptance"
    );
    assert_ne!(
        record_after, record_before,
        "acceptance changed no registry record, so this test attacked nothing"
    );
}

/// **Attack: an identity-moving governance write.** `intent.lock` mints a successor and
/// leaves the predecessor's document bit for bit.
///
/// The pair with the test above. `intent.lock` edits the policy table, which *is* in the
/// ID2 preimage, so RFC 0037 ID3 requires a successor rather than an edit. Both halves are
/// asserted: the new name is not the old name, and the old name still answers with exactly
/// the bytes it answered with before — which is what "an epoch advance never mutates a
/// published artifact; re-derived artifacts get new identities" (plan §4.6, ADR-0018) means
/// one level down.
#[test]
fn immutability_locking_an_intent_mints_a_successor_and_leaves_the_predecessor() {
    let mut rig = rig();
    let predecessor = rig.intent.clone();
    let before = rig.contract_bytes(&predecessor, "req_get_before");

    let successor = rig.lock(
        "req_lock",
        "idem-lock",
        PolicyField::AbstractionMaps,
        PolicyVerb::Locked,
    );
    assert_ne!(successor, predecessor, "the lock did not mint a successor");

    let after = rig.contract_bytes(&predecessor, "req_get_after");
    assert_eq!(
        after, before,
        "the predecessor's contract document changed when its successor was minted"
    );

    let successor_document = rig.contract_bytes(&successor, "req_get_successor");
    assert_ne!(
        successor_document, before,
        "the successor document is byte-identical to the predecessor's, so the lock \
         changed nothing and the new identity is unexplained"
    );

    // The predecessor's *record* moved — status and back-pointer — which is the half that
    // is allowed to move.
    let record = rig.registry_record(&predecessor, "req_record_after");
    let text = String::from_utf8(record).expect("a canonical JSON record is UTF-8");
    assert!(
        text.contains("\"superseded\""),
        "the predecessor's registry status did not move to `superseded`: {text}"
    );
    assert!(
        text.contains(successor.as_str()),
        "the predecessor's record does not name its successor: {text}"
    );
}

/// **Attack: publish different content under an identity the store already holds.**
///
/// The store's own refusal path, exercised end to end. BLAKE3 cannot be made to collide on
/// demand, so this is unreachable through the production seam — which is exactly why it is
/// worth exercising, because unexercised refusal code is a claim rather than a behaviour.
/// [`EqualLengthIdentity`] names every equal-length content identically, so two distinct
/// eight-byte payloads collide by construction.
///
/// Both halves are required: the second publication is **refused** (never partially
/// applied), and a read under the shared identity still returns the *first* bytes. A store
/// that answered "ok" and overwrote would fail the second; one that answered "ok" and
/// silently dropped the write would fail the first.
#[test]
fn immutability_the_store_refuses_new_content_under_a_held_identity() {
    let daemon = daemon_over(EqualLengthIdentity, true);
    let token = Rig::root_token();
    let store = daemon.store();

    let first = b"aaaaaaaa".to_vec();
    let second = b"bbbbbbbb".to_vec();
    assert_eq!(
        first.len(),
        second.len(),
        "the collision this test needs is a length collision"
    );

    let receipt = store
        .publish(ArtifactClass::Evidence, first.clone(), &token)
        .expect("the first publication is authorized and uncontested");
    let handle = receipt.handle().clone();

    let refusal = store
        .publish(ArtifactClass::Evidence, second.clone(), &token)
        .expect_err("a different content under a held identity must be refused");
    assert!(
        matches!(refusal, PublishRefusal::Aborted(_)),
        "the collision was refused as {refusal:?}, not as a publication abort"
    );

    let held = store
        .read(&handle, &token)
        .expect("the first artifact is still published");
    assert_eq!(
        held, first,
        "the refused publication overwrote the artifact it collided with"
    );
    assert_ne!(
        held, second,
        "the store holds the second publication's bytes under the first's identity"
    );
}

/// **Attack: change the daemon's epoch pinning.** Two daemons at different epoch pinnings
/// name one snapshot identically and one campaign differently.
///
/// [`PHASE_A_EXIT_PACKAGE.md`] §8.3 states both halves as design and §9.2 N7 records that
/// no epoch advance has ever been exercised. This test exercises the observable half of it
/// without one: run the identical script against a daemon that pins five compatibility
/// epochs and against one that pins none.
///
/// - The **snapshot** address must be equal. "The store derives identity from bytes and
///   nothing else […] the epochs ride the record, not the address" — so a daemon whose
///   epoch set differs cannot rename, and therefore cannot invalidate, a published
///   snapshot. That is ADR-0018's "an epoch advance never mutates an existing artifact",
///   observed rather than asserted.
/// - The **campaign** identity must differ, because `epochs_preimage` puts the six epoch
///   identities into `verification.start`'s preimage. That is the other half of the same
///   rule: a derivation that *is* epoch-sensitive gets a **new** identity rather than a
///   changed meaning under the old one.
///
/// A build that conflated the two — epochs in the snapshot address, or epochs out of the
/// campaign preimage — fails exactly one of the two assertions below.
#[test]
fn immutability_epoch_pinning_moves_a_derivation_and_never_a_published_snapshot() {
    let mut pinned = rig_over(Blake3Identity, true, Stage::Sealed);
    let mut unpinned = rig_over(Blake3Identity, false, Stage::Sealed);

    assert_eq!(
        pinned.snapshot, unpinned.snapshot,
        "two daemons at different epoch pinnings named one component set differently; the \
         epoch has entered the snapshot address, which SD-13 and ADR-0018 forbid"
    );

    let pinned_bytes: BTreeMap<ArtifactHandle, Vec<u8>> = pinned.published().into_iter().collect();
    let unpinned_bytes: BTreeMap<ArtifactHandle, Vec<u8>> =
        unpinned.published().into_iter().collect();
    assert_eq!(
        pinned_bytes, unpinned_bytes,
        "the two daemons published different bytes for one component set"
    );

    let pinned_start = pinned.start("req_start", "idem-start");
    ok(&pinned_start, "verification.start under pinned epochs");
    let unpinned_start = unpinned.start("req_start", "idem-start");
    ok(&unpinned_start, "verification.start under no pinned epochs");

    let campaign = |outcome: &OperationOutcome| match &outcome.payload {
        Payload::VerificationStart(response) => response
            .task
            .value()
            .cloned()
            .expect("a started campaign names its task"),
        other => panic!("expected a verification.start payload, got {other:?}"),
    };
    assert_ne!(
        campaign(&pinned_start),
        campaign(&unpinned_start),
        "two daemons at different epoch pinnings named one campaign identically; the epoch \
         set is not in the campaign preimage, so a resume could cross an epoch advance \
         under an unchanged identity"
    );
}

// =========================================================================================
// D — negative controls that show the probes can fail
// =========================================================================================

/// **Negative control.** A single flipped bit anywhere in any stored artifact is detected.
///
/// The corruption is applied to an in-memory copy — nothing in the store is touched — and
/// the recomputation is the same one every positive test above uses. Four perturbations per
/// artifact, chosen to cover the three positions a chunked hash treats differently (first
/// block, interior, final block) plus a length change, because a digest that ignored
/// trailing bytes or that hashed only a prefix would survive some of these and not others.
#[test]
fn negative_control_a_single_flipped_bit_in_any_stored_artifact_is_detected() {
    let mut rig = rig();
    ok(&rig.start("req_start", "idem-start"), "verification.start");
    let published = rig.published();
    assert!(!published.is_empty(), "nothing was published to corrupt");

    let mut corruptions = 0_usize;
    for (handle, content) in &published {
        let positions = [0, content.len() / 2, content.len() - 1];
        for position in positions {
            let mut corrupt = content.clone();
            corrupt[position] ^= 0x01;
            assert_ne!(
                independent_identity(&corrupt),
                handle.identity(),
                "a bit flipped at byte {position} of {handle} left the recomputed identity \
                 unchanged"
            );
            corruptions += 1;
        }
        // A length change, which a digest over a fixed prefix would miss.
        let mut truncated = content.clone();
        truncated.pop();
        assert_ne!(
            independent_identity(&truncated),
            handle.identity(),
            "dropping the last byte of {handle} left the recomputed identity unchanged"
        );
        corruptions += 1;
    }
    assert_eq!(
        corruptions,
        published.len() * 4,
        "the corruption sweep did not run the expected number of perturbations"
    );
}

/// **Negative control — the falsification of this file's own predicate.**
///
/// A daemon built over [`Fnv1a256Identity`] runs the entire script successfully and is
/// **perfectly self-consistent**: every self-referential check in the tree — one that
/// compares the store to its own configured seam — would pass against it. Checked against
/// the *declared* identifier instead, `fsck` (bn-k99dt) does catch it, because
/// `Fnv1a256Identity` is a wholesale substitution of `ContentIdentifier` and not a flaw
/// hiding inside `Blake3Identity` itself. Its addresses are nonetheless not BLAKE3 digests
/// of anything, and this file's independent predicate rejects every one of them too, on
/// different ground than `fsck`'s.
///
/// That contrast is the whole argument for this file existing, so it is asserted rather
/// than described:
///
/// 1. the script succeeds and publishes artifacts;
/// 2. `fsck`, checked against the declared `Blake3Identity`, reports every artifact as an
///    identity mismatch — the substituted seam does not survive a declared-seam audit;
/// 3. every published artifact also fails the independent-digest predicate;
/// 4. and the real daemon passes both, in the same test, over the same script.
///
/// If a future change made the independent predicate vacuous — comparing a value to itself,
/// or comparing nothing — step 3 fails immediately.
#[test]
fn negative_control_a_non_blake3_seam_is_self_consistent_and_fails_this_files_predicate() {
    let counterfeit = rig_over(Fnv1a256Identity, true, Stage::Sealed);
    let published = counterfeit.published();
    assert!(
        !published.is_empty(),
        "the counterfeit daemon published nothing, so it was never really tested"
    );

    let token = Rig::root_token();
    let defects = counterfeit
        .daemon
        .store()
        .audit_view(&token)
        .expect("`cap_root` confers audit")
        .fsck(&Blake3Identity);
    assert_eq!(
        defects.len(),
        published.len(),
        "fsck, checked against the declared seam, must flag every substituted entry: \
         {defects:?}"
    );
    assert!(
        defects
            .iter()
            .all(|defect| matches!(defect, StoreDefect::IdentityMismatch(_))),
        "and flag it as an identity mismatch, not any other class: {defects:?}"
    );

    for (handle, content) in &published {
        assert_ne!(
            handle.identity(),
            independent_identity(content),
            "the counterfeit seam produced the BLAKE3 digest for {handle}; this file's \
             predicate cannot distinguish it from the production seam"
        );
    }

    // And the production seam, over the same script, passes both checks.
    let real = rig();
    let real_defects = real
        .daemon
        .store()
        .audit_view(&Rig::root_token())
        .expect("`cap_root` confers audit")
        .fsck(&Blake3Identity);
    assert!(
        real_defects.is_empty(),
        "the production daemon's own declared seam failed its own audit: {real_defects:?}"
    );
    for (handle, content) in real.published() {
        assert_eq!(
            handle.identity(),
            independent_identity(&content),
            "the production daemon failed the same predicate the counterfeit was rejected \
             by"
        );
    }
}

// =========================================================================================
// E — the three identity levels, kept apart
// =========================================================================================

/// The string value of `field` in a canonical-JSON object.
fn json_string(document: &Json, field: &str) -> String {
    let Json::Object(fields) = document else {
        panic!("expected a JSON object, got {document:?}");
    };
    match fields.get(field) {
        Some(Json::String(value)) => value.clone(),
        other => panic!("expected a string at `{field}`, got {other:?}"),
    }
}

/// The integer value of `field` in a canonical-JSON object.
fn json_integer(document: &Json, field: &str) -> i64 {
    let Json::Object(fields) = document else {
        panic!("expected a JSON object, got {document:?}");
    };
    match fields.get(field) {
        Some(Json::Integer(value)) => *value,
        other => panic!("expected an integer at `{field}`, got {other:?}"),
    }
}

/// **The three identities.** Schema-document identity, class identity, and instance
/// identity are three different values, and this file has not conflated them.
///
/// `notes/plan/schemas/README.md` is normative and its own words are the assertions here:
/// a document `$id` is `https://continuum.dev/schema/v<epoch>/<name>.json`; the class
/// identity is "the same URI with the `v<epoch>/` segment removed", "never changes", and
/// "MUST NOT be used as a `$id`"; and the instance identity is the `schema_id`/`schema_epoch`
/// header an artifact carries beside the content address the store files it under.
///
/// Three sources, no two of them the same source: the `$id` comes out of the real schema
/// file in `notes/plan/schemas/`, the `schema_id`/`schema_epoch` pair comes off the wire in
/// an `intent.get` registry record, and the instance content address is recomputed by
/// [`spec_blake3`]. The final assertion is the one that matters for G1-01: the content
/// address carries no schema epoch, so a schema-epoch advance cannot rename an artifact —
/// the exact confusion `schemas/README.md` warns against when it says `schema_epoch` "is
/// **not** a seventh epoch".
#[test]
fn the_three_identity_levels_stay_distinct_document_class_and_instance() {
    let mut rig = rig();
    let intent = rig.intent.clone();
    let record_bytes = rig.registry_record(&intent, "req_record");
    let record = Json::parse(&record_bytes).expect("the registry record is canonical JSON");

    // Level 1 — document identity, read out of the dossier.
    let schema = Json::parse(REGISTRY_RECORD_SCHEMA.as_bytes()).expect("the schema is JSON");
    let document_id = json_string(&schema, "$id");
    assert_eq!(
        document_id, "https://continuum.dev/schema/v1/intent-registry-record.json",
        "the schema document's `$id` is not the epoch-qualified form README.md requires"
    );

    // Level 2 — class identity, read off the wire.
    let class_id = json_string(&record, "schema_id");
    let epoch_value = json_integer(&record, "schema_epoch");
    assert_eq!(
        class_id, "https://continuum.dev/schema/intent-registry-record.json",
        "the artifact carries something other than the epoch-free class identity"
    );
    assert!(
        !class_id.contains("/v1/"),
        "the class identity carries an epoch segment: {class_id}"
    );

    // The README's mechanical relation between the two: insert `v<schema_epoch>/` before
    // the last segment of the class identity and the document `$id` comes back.
    let (prefix, last) = class_id
        .rsplit_once('/')
        .expect("a class identity has at least one `/`");
    assert_eq!(
        format!("{prefix}/v{epoch_value}/{last}"),
        document_id,
        "the two-field instance header and the document `$id` do not determine each other"
    );

    // Level 3 — instance identity, recomputed.
    let instance = intent.as_str();
    assert_eq!(
        instance,
        independent_handle(
            ArtifactClass::IntentContract,
            &IntentContract::decode(&rig.contract_bytes(&intent, "req_contract"))
                .expect("the wire document decodes")
                .identity_preimage_bytes()
        ),
        "the instance identity is not the independent digest of the contract's preimage"
    );

    // The three are pairwise distinct, and level 3 is independent of level 1.
    assert_ne!(document_id, class_id);
    assert_ne!(document_id, instance);
    assert_ne!(class_id, instance);
    assert!(
        !instance.contains(&format!("v{epoch_value}")) && !instance.contains("schema"),
        "the content address carries schema-document information ({instance}); a schema \
         epoch advance would rename the artifact"
    );
}

// =========================================================================================
// F — the scope this file's verdict is narrowed to, stated as assertions
// =========================================================================================

/// **Scope.** Exactly one of plan §4.4's nineteen artifact classes is not
/// content-addressed, it is `cap_*`, and the store refuses to publish it.
///
/// This is the one place G1-01's "artifacts are […] content-addressed" has a real
/// exception, and it is an exception the plan takes deliberately: capability tokens "are
/// minted randomly and do confer authority", so a capability with a content-derived name
/// would be a bearer token anyone holding its bytes could recompute. Recorded here as an
/// assertion so the verdict "18 of 19 classes" is mechanical and a nineteenth exception
/// added later fails this test.
///
/// # A finding, recorded rather than smoothed over
///
/// The refusal arrives as **`CapabilityDenied`, not as a publication abort**, and this test
/// was written expecting the latter. `ReferenceStore::stage`'s own documentation says
/// `PublishRefusal::Aborted` is returned "when the class is not content-addressed", and
/// that arm exists (`AbortReason::NotContentAddressed`) — but it is unreachable from any
/// authorized caller, because `ScopedCapabilityPolicy::decide` refuses
/// `Action::Publish` on a non-content-addressed class *first*, at the authorization layer,
/// and authorization runs before the class check. Both layers hold the property; only the
/// outer one is observable. The assertion below is written against what the daemon does,
/// with the inner layer named so a future change that made it reachable is a visible
/// change rather than a silent one.
#[test]
fn scope_the_capability_class_is_the_only_class_that_is_not_content_addressed() {
    let uncontent: Vec<ArtifactClass> = ArtifactClass::ALL
        .into_iter()
        .filter(|class| !class.is_content_addressed())
        .collect();
    assert_eq!(
        uncontent,
        vec![ArtifactClass::Capability],
        "plan §4.4's not-content-addressed set is no longer exactly {{cap_*}}"
    );

    let daemon = daemon_over(Blake3Identity, true);
    let token = Rig::root_token();
    let refusal = daemon
        .store()
        .publish(ArtifactClass::Capability, b"a token".to_vec(), &token)
        .expect_err("a capability may not be published");
    assert!(
        matches!(refusal, PublishRefusal::CapabilityDenied(_)),
        "publishing a capability was refused as {refusal:?}; the outer, authorization-layer \
         refusal has moved and this test no longer describes where the property is enforced"
    );

    // A content-addressed class publishes under the same capability, so the refusal above
    // is about the class and not about the authority.
    daemon
        .store()
        .publish(ArtifactClass::Evidence, b"a token".to_vec(), &token)
        .expect("the same capability publishes a content-addressed class");
}

/// **Scope.** The classes this daemon actually mints, and the classes nothing in this file
/// probes.
///
/// INV-007: an unmeasured class is an absence to be counted, not a pass to be implied. The
/// full script — `intent.accept`, `workspace.create` with `seal`, `verification.start` —
/// publishes exactly two of plan §4.4's nineteen classes into the store. This test pins
/// that set, so the verdict's scope cannot silently widen (a new class would fail the
/// equality) or narrow (a class that stopped being published would fail it too), and counts
/// the remaining sixteen in the same assertion rather than in a comment.
///
/// The intent contract is deliberately in neither list: it is `in_*`-addressed and reachable
/// on the wire, and it lives in the daemon's own registry rather than in the publication
/// store, which is itself a scope fact worth pinning.
#[test]
fn scope_the_classes_this_daemon_actually_mints() {
    let mut rig = rig();
    ok(&rig.start("req_start", "idem-start"), "verification.start");
    let minted: BTreeSet<ArtifactClass> = rig
        .published()
        .into_iter()
        .map(|(handle, _)| handle.class())
        .collect();

    let expected: BTreeSet<ArtifactClass> = [ArtifactClass::WorkspaceSnapshot, ArtifactClass::Task]
        .into_iter()
        .collect();
    assert_eq!(
        minted, expected,
        "the classes this script publishes have changed; the verdict's scope is stale"
    );

    let unprobed: Vec<ArtifactClass> = ArtifactClass::ALL
        .into_iter()
        .filter(|class| !minted.contains(class) && *class != ArtifactClass::IntentContract)
        .collect();
    assert_eq!(
        unprobed.len(),
        ArtifactClass::ALL.len() - minted.len() - 1,
        "expected sixteen classes no publication in this file reaches, found {}: {:?}",
        unprobed.len(),
        unprobed
    );
    assert_eq!(
        unprobed.len(),
        16,
        "plan §4.4's class count moved; the verdict's absence list is stale: {unprobed:?}"
    );

    // And the intent contract is content-addressed without being in the store at all.
    let intent = rig.intent.clone();
    let store_handle = ArtifactHandle::new(ArtifactClass::IntentContract, &intent.as_str()[3..])
        .expect("the identity half is a well-formed token");
    assert!(
        !rig.published()
            .into_iter()
            .any(|(handle, _)| handle == store_handle),
        "the intent contract is in the publication store; this file's account of where it \
         lives is wrong"
    );
}

/// **Scope.** The two content-addressing shapes are two shapes, and this file does not
/// report them as one.
///
/// `verification.start` answers with an `ArtifactRef` whose `handle` and `commitment` are
/// both `task_*` content addresses of different things: the handle is the digest of the
/// *request preimage* that named the campaign, the commitment is the digest of the
/// *record bytes* the store holds. Both are content addresses; only the second is a digest
/// of the artifact.
///
/// The distinction is why this file's verdict is `SATISFIED-AT-NARROWER-SCOPE` rather than
/// `SATISFIED`: an independent party can verify the second with nothing but the bytes, and
/// can verify the first only by transcribing [`campaign_preimage`]. Asserting the two are
/// different values is what stops a later reader from citing the strong claim for the weak
/// case.
#[test]
fn scope_the_two_content_addressing_shapes_are_not_one_shape() {
    let mut rig = rig();
    let start = rig.start("req_start", "idem-start");
    ok(&start, "verification.start");

    let reference = &start.envelope.artifacts[0];
    let campaign = reference.handle.as_str().to_owned();
    let commitment = reference
        .commitment
        .value()
        .expect("the campaign names its record")
        .as_str()
        .to_owned();

    assert_ne!(
        campaign, commitment,
        "the request-derived campaign identity and the record's content address are one \
         value here, so the two shapes cannot be told apart"
    );

    // The request-derived one: recomputable only through a transcription of the rule.
    assert_eq!(
        campaign,
        independent_handle(
            ArtifactClass::Task,
            campaign_preimage(&rig.snapshot, &rig.intent).bytes()
        ),
        "the campaign identity is not the digest of the transcribed request preimage"
    );

    // The record one: recomputable from the bytes alone.
    let record = rig
        .published()
        .into_iter()
        .find(|(handle, _)| handle.class() == ArtifactClass::Task)
        .expect("the campaign published its record");
    assert_eq!(
        commitment,
        independent_handle(ArtifactClass::Task, &record.1),
        "the commitment is not the digest of the record bytes the store holds"
    );

    // And the campaign identity is *not* the digest of any published artifact, which is
    // the fact that makes the two shapes genuinely different rather than two spellings.
    for (_, content) in rig.published() {
        assert_ne!(
            campaign,
            independent_handle(ArtifactClass::Task, &content),
            "the campaign identity turned out to be a stored artifact's digest after all"
        );
    }
}
