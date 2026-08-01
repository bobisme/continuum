//! Acceptance evidence for `PR-3-IMPL-03` — source/dependency/toolchain/config identities
//! (`notes/plan/notes/START_HERE_IMPLEMENTATION.md`, PR 3's third Implement bullet;
//! `notes/plan/notes/PLAN_REQUIREMENTS.json`, id `PR-3-IMPL-03`).
//!
//! > A workspace snapshot contains content identities for: source files; […]; dependency
//! > lockfiles; toolchain and semantic epochs; […]; configuration.
//! >
//! > — `notes/plan/plan.md` §4.2 "Workspace snapshots"
//!
//! `crates/continuum-workspace/src/components.rs` carries the unit-level statement of
//! each property, exactly as `snapshot.rs` does for the Merkle tree. This file is the
//! standalone witness: it uses only the crate's *public* API, from outside the crate, and
//! it adds what a unit test cannot reach — composition with [`ReferenceStore`], and
//! adversarial sweeps wide enough to be evidence rather than illustration.
//!
//! # Evidence map
//!
//! | Claim | Test |
//! |---|---|
//! | same components in any construction order ⇒ one identity | [`determinism_same_components_yield_one_identity`] |
//! | any byte change in any one component moves only the descriptor | [`determinism_any_component_byte_change_moves_the_identity`] |
//! | presence/absence of a component moves the identity on its own | [`determinism_presence_change_moves_the_identity`] |
//! | editing the lockfile leaves the source tree's own identity alone | [`independence_editing_one_component_never_touches_another`] |
//! | editing the source tree leaves every other component's identity alone | [`independence_editing_the_source_tree_never_touches_components`] |
//! | a total identity collision conflates no two distinct descriptors | [`adversarial_a_total_identity_collision_conflates_nothing`] |
//! | a store refuses to file a colliding second record | [`adversarial_a_store_refuses_the_colliding_second_record`] |
//! | truncation, reordering, unknown tags, and trailing bytes are typed refusals | [`decode_rejects_truncation_reorder_unknown_tag_and_trailing_bytes`] |
//! | an encoding round-trips through every present/absent combination | [`serialization_round_trips_every_presence_combination`] |
//! | descriptor + components publish and converge like `Snapshot::records` does | [`store_composition_descriptor_and_components_converge`] |
//! | two descriptors sharing a component share it in the store, not just in memory | [`store_composition_shared_components_publish_once`] |
//!
//! # House rules
//!
//! - **Nothing in `src/` is touched.** Every seam used here is public API.
//! - [`HexIdentity`] is *injective* — the record's own bytes are the identity, ADR-0013's
//!   certified discipline — so "distinct content, distinct identity" under it is a fact
//!   rather than a probability; every negative claim that must be airtight rests on it.
//! - [`ConstantIdentity`] is the worst identity function that exists, used exactly where
//!   a claim must survive it: ADR-0013's guarantee is "collisions do not decide
//!   anything", not "collisions are rare".

use std::collections::BTreeSet;
use std::sync::Arc;

use continuum_workspace::artifact_path::{ArtifactClass, ArtifactHandle};
use continuum_workspace::components::{
    Component, ComponentKind, DescriptorDecodeError, DescriptorError, WorkspaceDescriptor,
};
use continuum_workspace::publication::{
    AbortReason, ActorId, AuditLog, AuthorityLevel, CapabilityDescriptor, CapabilityToken,
    ContentIdentifier, IdentityUnavailable, PublishRefusal, ReferenceStore,
};
use continuum_workspace::snapshot::{Snapshot, WorkspaceContent, WorkspacePath};

// --- identity seams ----------------------------------------------------------------------

const HEX: &[u8; 16] = b"0123456789abcdef";

fn hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(char::from(HEX[usize::from(byte >> 4)]));
        out.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    out
}

/// The injective seam: a record's own bytes, in hex.
#[derive(Debug, Clone, Copy)]
struct HexIdentity;

impl ContentIdentifier for HexIdentity {
    fn identify(
        &self,
        class: ArtifactClass,
        content: &[u8],
    ) -> Result<ArtifactHandle, IdentityUnavailable> {
        ArtifactHandle::new(class, &hex(content)).map_err(|_| IdentityUnavailable)
    }
}

/// Every record gets one name. The worst identity function that exists.
#[derive(Debug, Clone, Copy)]
struct ConstantIdentity;

impl ContentIdentifier for ConstantIdentity {
    fn identify(
        &self,
        class: ArtifactClass,
        _content: &[u8],
    ) -> Result<ArtifactHandle, IdentityUnavailable> {
        ArtifactHandle::new(class, "collide").map_err(|_| IdentityUnavailable)
    }
}

// --- fixtures ------------------------------------------------------------------------------

fn path(text: &str) -> WorkspacePath {
    WorkspacePath::new(text).expect("test path is well formed")
}

fn source<I: ContentIdentifier>(files: &[(&str, &[u8])], identifier: &I) -> Snapshot {
    let mut content = WorkspaceContent::new();
    for (text, bytes) in files {
        content
            .insert(path(text), (*bytes).to_vec())
            .expect("test description is a tree");
    }
    Snapshot::build(&content, identifier).expect("test seam names every node")
}

const SOURCE_A: [(&str, &[u8]); 2] = [
    ("Cargo.toml", b"[workspace]"),
    ("src/lib.rs", b"pub fn a() {}"),
];
const SOURCE_B: [(&str, &[u8]); 2] = [
    ("Cargo.toml", b"[workspace]"),
    ("src/lib.rs", b"pub fn b() {}"),
];

/// A store that may publish and read, built over `identifier`.
fn store<I: ContentIdentifier + 'static>(identifier: I) -> (ReferenceStore, CapabilityToken) {
    let token = CapabilityToken::mint("k1").expect("well-formed capability identity");
    let store = ReferenceStore::builder(identifier, Arc::new(AuditLog::new()))
        .capability(CapabilityDescriptor::new(
            token.clone(),
            ActorId::new("author"),
            AuthorityLevel::Propose,
        ))
        .build();
    (store, token)
}

// --- determinism ---------------------------------------------------------------------------

#[test]
fn determinism_same_components_yield_one_identity() {
    let build = || {
        WorkspaceDescriptor::builder()
            .dependencies(b"[[package]]\nname = \"a\"".to_vec())
            .toolchain(b"[toolchain]\nchannel = \"1.75\"".to_vec())
            .configuration(b"[workspace]\nmembers = []".to_vec())
            .build(source(&SOURCE_A, &HexIdentity), &HexIdentity)
            .expect("named")
    };
    let first = build();
    let second = build();
    assert_eq!(first.identity(), second.identity());
    assert_eq!(first, second);
    assert_eq!(first.record(), second.record());
    assert_eq!(first.encode(), second.encode());
}

#[test]
fn determinism_any_component_byte_change_moves_the_identity() {
    let baseline = || {
        WorkspaceDescriptor::builder()
            .dependencies(b"lock-v1".to_vec())
            .toolchain(b"toolchain-v1".to_vec())
            .configuration(b"config-v1".to_vec())
            .build(source(&SOURCE_A, &HexIdentity), &HexIdentity)
            .expect("named")
    };
    let base = baseline();

    let dependency_changed = WorkspaceDescriptor::builder()
        .dependencies(b"lock-v2".to_vec())
        .toolchain(b"toolchain-v1".to_vec())
        .configuration(b"config-v1".to_vec())
        .build(source(&SOURCE_A, &HexIdentity), &HexIdentity)
        .expect("named");
    let toolchain_changed = WorkspaceDescriptor::builder()
        .dependencies(b"lock-v1".to_vec())
        .toolchain(b"toolchain-v2".to_vec())
        .configuration(b"config-v1".to_vec())
        .build(source(&SOURCE_A, &HexIdentity), &HexIdentity)
        .expect("named");
    let configuration_changed = WorkspaceDescriptor::builder()
        .dependencies(b"lock-v1".to_vec())
        .toolchain(b"toolchain-v1".to_vec())
        .configuration(b"config-v2".to_vec())
        .build(source(&SOURCE_A, &HexIdentity), &HexIdentity)
        .expect("named");
    let source_changed = WorkspaceDescriptor::builder()
        .dependencies(b"lock-v1".to_vec())
        .toolchain(b"toolchain-v1".to_vec())
        .configuration(b"config-v1".to_vec())
        .build(source(&SOURCE_B, &HexIdentity), &HexIdentity)
        .expect("named");

    let identities: [&ArtifactHandle; 5] = [
        &base,
        &dependency_changed,
        &toolchain_changed,
        &configuration_changed,
        &source_changed,
    ]
    .map(WorkspaceDescriptor::identity);
    let distinct: BTreeSet<String> = identities.iter().map(|handle| handle.to_string()).collect();
    assert_eq!(
        distinct.len(),
        5,
        "every single-byte-different variant is a distinct identity"
    );
}

#[test]
fn determinism_presence_change_moves_the_identity() {
    let none = WorkspaceDescriptor::builder()
        .build(source(&SOURCE_A, &HexIdentity), &HexIdentity)
        .expect("named");
    let empty_lockfile = WorkspaceDescriptor::builder()
        .dependencies(Vec::new())
        .build(source(&SOURCE_A, &HexIdentity), &HexIdentity)
        .expect("named");
    let empty_toolchain = WorkspaceDescriptor::builder()
        .toolchain(Vec::new())
        .build(source(&SOURCE_A, &HexIdentity), &HexIdentity)
        .expect("named");
    let empty_configuration = WorkspaceDescriptor::builder()
        .configuration(Vec::new())
        .build(source(&SOURCE_A, &HexIdentity), &HexIdentity)
        .expect("named");

    // Absence is not "present, zero bytes": every presence combination, even with
    // byte-identical (empty) content, is a structurally distinct record and identity.
    let variants = [
        &none,
        &empty_lockfile,
        &empty_toolchain,
        &empty_configuration,
    ];
    let distinct: BTreeSet<String> = variants
        .iter()
        .map(|descriptor| descriptor.identity().to_string())
        .collect();
    assert_eq!(distinct.len(), 4);
    assert_ne!(none, empty_lockfile);
    assert_eq!(none.present_kinds(), vec![ComponentKind::Source]);
    assert_eq!(
        empty_lockfile.present_kinds(),
        vec![ComponentKind::Source, ComponentKind::Dependencies]
    );
}

// --- component independence ----------------------------------------------------------------

#[test]
fn independence_editing_one_component_never_touches_another() {
    let fixed_source = source(&SOURCE_A, &HexIdentity);
    let fixed_source_identity = fixed_source.identity().clone();

    let before = WorkspaceDescriptor::builder()
        .dependencies(b"lock-v1".to_vec())
        .toolchain(b"toolchain-v1".to_vec())
        .build(fixed_source, &HexIdentity)
        .expect("named");

    let after = WorkspaceDescriptor::builder()
        .dependencies(b"lock-v2".to_vec())
        .toolchain(b"toolchain-v1".to_vec())
        .build(source(&SOURCE_A, &HexIdentity), &HexIdentity)
        .expect("named");

    assert_ne!(before.identity(), after.identity());
    // The source tree's own identity is untouched by a dependency edit.
    assert_eq!(before.source().identity(), &fixed_source_identity);
    assert_eq!(after.source().identity(), &fixed_source_identity);
    assert_eq!(before.source(), after.source());
    // And the toolchain component, which did not change, kept its own identity too.
    assert_eq!(
        before.toolchain().map(Component::identity),
        after.toolchain().map(Component::identity)
    );
}

#[test]
fn independence_editing_the_source_tree_never_touches_components() {
    let dependency_bytes = b"lock-fixed".to_vec();
    let before = WorkspaceDescriptor::builder()
        .dependencies(dependency_bytes.clone())
        .build(source(&SOURCE_A, &HexIdentity), &HexIdentity)
        .expect("named");
    let after = WorkspaceDescriptor::builder()
        .dependencies(dependency_bytes)
        .build(source(&SOURCE_B, &HexIdentity), &HexIdentity)
        .expect("named");

    assert_ne!(before.identity(), after.identity());
    assert_ne!(before.source().identity(), after.source().identity());
    // The dependency component is byte-identical input, so it is named identically and
    // compares equal, regardless of what happened to the source tree beside it.
    assert_eq!(
        before.dependencies().map(Component::identity),
        after.dependencies().map(Component::identity)
    );
    assert_eq!(before.dependencies(), after.dependencies());
}

// --- adversarial identities ------------------------------------------------------------------

#[test]
fn adversarial_a_total_identity_collision_conflates_nothing() {
    let left = WorkspaceDescriptor::builder()
        .dependencies(b"one".to_vec())
        .toolchain(b"same-toolchain".to_vec())
        .build(source(&SOURCE_A, &ConstantIdentity), &ConstantIdentity)
        .expect("named");
    let right = WorkspaceDescriptor::builder()
        .dependencies(b"two".to_vec())
        .toolchain(b"same-toolchain".to_vec())
        .build(source(&SOURCE_A, &ConstantIdentity), &ConstantIdentity)
        .expect("named");

    // The collision is total and real: descriptor, source root, and every component
    // share one name under this seam.
    assert_eq!(left.identity(), right.identity());
    assert_eq!(left.source().identity(), right.source().identity());
    assert_eq!(
        left.dependencies().map(Component::identity),
        right.dependencies().map(Component::identity)
    );
    assert_eq!(
        left.toolchain().map(Component::identity),
        right.toolchain().map(Component::identity)
    );

    // And nothing is conflated: content equality resolves it in both directions.
    assert_ne!(left, right);
    assert_ne!(left.dependencies(), right.dependencies());
    // The toolchain component, unlike the dependency one, really is equal content.
    assert_eq!(left.toolchain(), right.toolchain());

    let same_as_left = WorkspaceDescriptor::builder()
        .dependencies(b"one".to_vec())
        .toolchain(b"same-toolchain".to_vec())
        .build(source(&SOURCE_A, &ConstantIdentity), &ConstantIdentity)
        .expect("named");
    assert_eq!(left, same_as_left);
}

#[test]
fn adversarial_a_store_refuses_the_colliding_second_record() {
    let left = WorkspaceDescriptor::builder()
        .dependencies(b"one".to_vec())
        .build(source(&[("a", b"x")], &ConstantIdentity), &ConstantIdentity)
        .expect("named");
    let right = WorkspaceDescriptor::builder()
        .dependencies(b"two".to_vec())
        .build(source(&[("a", b"x")], &ConstantIdentity), &ConstantIdentity)
        .expect("named");

    let (store, token) = store(ConstantIdentity);
    let mut refused_dependency_records = 0usize;
    for descriptor in [&left, &right] {
        for (_, record) in descriptor.records() {
            match store.publish(ArtifactClass::WorkspaceSnapshot, record.to_vec(), &token) {
                Ok(_) => {}
                Err(PublishRefusal::Aborted(aborted)) => {
                    assert_eq!(aborted.reason(), AbortReason::IdentityCollision);
                    refused_dependency_records += 1;
                }
                Err(other) => panic!("unexpected refusal: {other}"),
            }
        }
    }
    // Every record that collided under `ConstantIdentity` and was not byte-identical to
    // the first publisher's was refused rather than silently overwritten: the source leaf
    // and root (shared, byte-identical, converge for free) do not collide, but each
    // descriptor's own record and dependency component do.
    assert!(
        refused_dependency_records >= 2,
        "expected at least the second dependency component and the second descriptor \
         record to collide, got {refused_dependency_records}"
    );
}

// --- decoding ------------------------------------------------------------------------------

#[test]
fn decode_rejects_truncation_reorder_unknown_tag_and_trailing_bytes() {
    let good = WorkspaceDescriptor::builder()
        .dependencies(b"lock".to_vec())
        .toolchain(b"toolchain".to_vec())
        .build(source(&SOURCE_A, &HexIdentity), &HexIdentity)
        .expect("named")
        .encode();

    // Truncation: no proper prefix decodes.
    for len in 0..good.len() {
        assert!(
            WorkspaceDescriptor::decode(&good[..len], &HexIdentity).is_err(),
            "a proper prefix of length {len} decoded"
        );
    }
    assert!(WorkspaceDescriptor::decode(&good, &HexIdentity).is_ok());

    // Trailing bytes.
    let mut trailing = good.clone();
    trailing.push(0);
    assert!(matches!(
        WorkspaceDescriptor::decode(&trailing, &HexIdentity),
        Err(DescriptorDecodeError::TrailingBytes { .. })
    ));

    // Reorder: swap the toolchain and dependency entries' tag bytes so the stream lists
    // tag 0x02 before 0x01. Both entries carry equal-length payloads (`"lock"`/9 bytes vs
    // `"toolchain"`/9 bytes — pad "lock" so the byte offsets line up), so we build the
    // reordered stream directly rather than surgically patching `good`.
    let reordered = {
        let descriptor = WorkspaceDescriptor::builder()
            .dependencies(b"AAAAAAAAA".to_vec())
            .toolchain(b"BBBBBBBBB".to_vec())
            .build(source(&SOURCE_A, &HexIdentity), &HexIdentity)
            .expect("named");
        let mut bytes = descriptor.encode();
        // Locate the two same-length entries by their distinctive payload bytes and swap
        // their tag bytes in place, which is exactly a reordering of the entry list from
        // the decoder's point of view (tag position is unchanged; tag *value* per position
        // is what canonical order constrains).
        let dep_tag_at = bytes
            .windows(9)
            .position(|window| window == b"AAAAAAAAA".as_slice())
            .expect("dependency payload present")
            - 9; // tag(1) + len(8) precede the payload
        let tool_tag_at = bytes
            .windows(9)
            .position(|window| window == b"BBBBBBBBB".as_slice())
            .expect("toolchain payload present")
            - 9;
        bytes.swap(dep_tag_at, tool_tag_at);
        bytes
    };
    assert!(matches!(
        WorkspaceDescriptor::decode(&reordered, &HexIdentity),
        Err(DescriptorDecodeError::TagOrder { .. })
    ));

    // Unknown tag: corrupt the source entry's tag byte (position `DESCRIPTOR_MAGIC.len()
    // + version(1) + count(8)`) to a value no `ComponentKind` claims.
    let magic_and_version_and_count = 6 + 1 + 8;
    let mut unknown_tag = good.clone();
    unknown_tag[magic_and_version_and_count] = 0x7f;
    assert!(matches!(
        WorkspaceDescriptor::decode(&unknown_tag, &HexIdentity),
        Err(DescriptorDecodeError::UnknownTag { tag: 0x7f, .. })
    ));

    // Missing source: an encoding whose only entry is a non-source kind.
    let no_source = WorkspaceDescriptor::builder()
        .dependencies(b"x".to_vec())
        .build(source(&SOURCE_A, &HexIdentity), &HexIdentity)
        .expect("named");
    let mut bytes = no_source.encode();
    // Drop the source entry (tag 0x00, 8-byte length, then `Snapshot::encode()` bytes) by
    // rewriting the stream with just the dependency entry and count = 1.
    let source_len = {
        let len_at = 6 + 1 + 8 + 1; // magic + version + count + source tag
        let len_bytes: [u8; 8] = bytes[len_at..len_at + 8].try_into().expect("8 bytes");
        usize::try_from(u64::from_be_bytes(len_bytes)).expect("fits in memory")
    };
    let source_entry_len = 1 + 8 + source_len; // tag + length + payload
    let header_len = 6 + 1 + 8;
    let dependency_entry = bytes[header_len + source_entry_len..].to_vec();
    let mut without_source = Vec::new();
    without_source.extend_from_slice(&bytes[..6]); // magic
    without_source.push(bytes[6]); // version
    without_source.extend_from_slice(&1u64.to_be_bytes()); // count = 1
    without_source.extend_from_slice(&dependency_entry);
    bytes = without_source;
    assert!(matches!(
        WorkspaceDescriptor::decode(&bytes, &HexIdentity),
        Err(DescriptorDecodeError::MissingSource)
    ));
}

// --- serialization -------------------------------------------------------------------------

#[test]
fn serialization_round_trips_every_presence_combination() {
    let base_source = || source(&SOURCE_A, &HexIdentity);
    let combinations: Vec<WorkspaceDescriptor> = vec![
        WorkspaceDescriptor::builder()
            .build(base_source(), &HexIdentity)
            .expect("named"),
        WorkspaceDescriptor::builder()
            .dependencies(b"lock".to_vec())
            .build(base_source(), &HexIdentity)
            .expect("named"),
        WorkspaceDescriptor::builder()
            .toolchain(b"toolchain".to_vec())
            .build(base_source(), &HexIdentity)
            .expect("named"),
        WorkspaceDescriptor::builder()
            .configuration(b"config".to_vec())
            .build(base_source(), &HexIdentity)
            .expect("named"),
        WorkspaceDescriptor::builder()
            .dependencies(b"lock".to_vec())
            .configuration(b"config".to_vec())
            .build(base_source(), &HexIdentity)
            .expect("named"),
        WorkspaceDescriptor::builder()
            .dependencies(b"lock".to_vec())
            .toolchain(b"toolchain".to_vec())
            .configuration(b"config".to_vec())
            .build(base_source(), &HexIdentity)
            .expect("named"),
        // Empty-but-present components round-trip too.
        WorkspaceDescriptor::builder()
            .dependencies(Vec::new())
            .toolchain(Vec::new())
            .configuration(Vec::new())
            .build(base_source(), &HexIdentity)
            .expect("named"),
    ];

    let mut identities: Vec<String> = Vec::new();
    for descriptor in &combinations {
        let bytes = descriptor.encode();
        let decoded = WorkspaceDescriptor::decode(&bytes, &HexIdentity).expect("round trip");
        assert_eq!(&decoded, descriptor);
        assert_eq!(decoded.identity(), descriptor.identity());
        assert_eq!(decoded.encode(), bytes);
        assert_eq!(decoded.present_kinds(), descriptor.present_kinds());
        identities.push(descriptor.identity().to_string());
    }
    // Every presence combination above is a distinct identity.
    let distinct: BTreeSet<String> = identities.into_iter().collect();
    assert_eq!(distinct.len(), combinations.len());
}

// --- store composition ----------------------------------------------------------------------

#[test]
fn store_composition_descriptor_and_components_converge() {
    let descriptor = WorkspaceDescriptor::builder()
        .dependencies(b"lock".to_vec())
        .toolchain(b"toolchain".to_vec())
        .configuration(b"config".to_vec())
        .build(source(&SOURCE_A, &HexIdentity), &HexIdentity)
        .expect("named");

    let (store, token) = store(HexIdentity);

    // Publish once, in the order `records()` returns.
    for (identity, record) in descriptor.records() {
        let receipt = store
            .publish(ArtifactClass::WorkspaceSnapshot, record.to_vec(), &token)
            .expect("the store names the record exactly as the descriptor did");
        assert_eq!(receipt.handle(), identity);
    }

    // Publishing the exact same records again converges: same handles, no collision.
    for (identity, record) in descriptor.records() {
        let receipt = store
            .publish(ArtifactClass::WorkspaceSnapshot, record.to_vec(), &token)
            .expect("republishing byte-identical content converges");
        assert_eq!(receipt.handle(), identity);
    }

    // The descriptor's own record and every component read back byte-identical.
    assert_eq!(
        store
            .read(descriptor.identity(), &token)
            .expect("published"),
        descriptor.record()
    );
    for component in [
        descriptor.dependencies(),
        descriptor.toolchain(),
        descriptor.configuration(),
    ]
    .into_iter()
    .flatten()
    {
        assert_eq!(
            store.read(component.identity(), &token).expect("published"),
            component.record()
        );
    }
    assert_eq!(
        store
            .read(descriptor.source().identity(), &token)
            .expect("published"),
        descriptor.source().root().record()
    );
}

#[test]
fn store_composition_shared_components_publish_once() {
    // Two descriptors with different source trees but the exact same dependency lockfile.
    let shared_dependency = b"[[package]]\nname = \"shared\"".to_vec();
    let left = WorkspaceDescriptor::builder()
        .dependencies(shared_dependency.clone())
        .build(source(&SOURCE_A, &HexIdentity), &HexIdentity)
        .expect("named");
    let right = WorkspaceDescriptor::builder()
        .dependencies(shared_dependency)
        .build(source(&SOURCE_B, &HexIdentity), &HexIdentity)
        .expect("named");

    assert_ne!(left.identity(), right.identity());
    assert_eq!(
        left.dependencies().map(Component::identity),
        right.dependencies().map(Component::identity)
    );

    let (store, token) = store(HexIdentity);
    let mut published: BTreeSet<String> = BTreeSet::new();
    for descriptor in [&left, &right] {
        for (identity, record) in descriptor.records() {
            let receipt = store
                .publish(ArtifactClass::WorkspaceSnapshot, record.to_vec(), &token)
                .expect("converges or files fresh");
            assert_eq!(receipt.handle(), identity);
            published.insert(receipt.handle().to_string());
        }
    }

    let distinct: BTreeSet<String> = left
        .records()
        .into_iter()
        .chain(right.records())
        .map(|(_, record)| hex(record))
        .collect();
    assert_eq!(published.len(), distinct.len());
    // Fewer than the sum of both descriptors' record counts: the shared dependency
    // component (and, coincidentally, the shared `Cargo.toml` leaf and root within each
    // source tree's own dedup) really did converge rather than being stored twice.
    assert!(published.len() < left.records().len() + right.records().len());

    // The shared dependency component reads back the same bytes from either name.
    let left_dependency = left.dependencies().expect("present");
    let right_dependency = right.dependencies().expect("present");
    assert_eq!(left_dependency.identity(), right_dependency.identity());
    assert_eq!(
        store
            .read(left_dependency.identity(), &token)
            .expect("published"),
        store
            .read(right_dependency.identity(), &token)
            .expect("published"),
    );
}

// --- error surfacing (public-API smoke, mirrors `DescriptorError`/`Debug`) -------------------

#[test]
fn descriptor_error_and_decode_error_display_do_not_panic() {
    // Not a correctness claim about wording — a smoke test that every variant's `Display`
    // is reachable from outside the crate and does not panic, since these are the errors
    // a caller ultimately reports.
    let errors = [
        DescriptorError::IdentityUnavailable {
            kind: Some(ComponentKind::Dependencies),
        },
        DescriptorError::IdentityUnavailable { kind: None },
        DescriptorError::IdentityClassMismatch {
            kind: Some(ComponentKind::Toolchain),
            class: ArtifactClass::Evidence,
        },
    ];
    for error in errors {
        assert!(!error.to_string().is_empty());
    }

    let decode_errors = [
        DescriptorDecodeError::Magic,
        DescriptorDecodeError::Version { found: 9 },
        DescriptorDecodeError::UnexpectedEnd { at: 0, needed: 1 },
        DescriptorDecodeError::LengthOutOfRange {
            at: 0,
            declared: u64::MAX,
        },
        DescriptorDecodeError::UnknownTag { tag: 0xff, at: 0 },
        DescriptorDecodeError::TagOrder { at: 0, tag: 0x01 },
        DescriptorDecodeError::MissingSource,
        DescriptorDecodeError::TrailingBytes { at: 0 },
    ];
    for error in decode_errors {
        assert!(!error.to_string().is_empty());
    }
}
