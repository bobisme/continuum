//! Acceptance evidence for `PR-3-IMPL-01` — Merkle workspace snapshots
//! (`notes/plan/notes/START_HERE_IMPLEMENTATION.md`, PR 3's first Implement bullet;
//! `notes/plan/notes/PLAN_REQUIREMENTS.json`, id `PR-3-IMPL-01`).
//!
//! > Snapshots form a Merkle DAG. A tool call never means "whatever is currently on
//! > disk"; it means a named snapshot.
//! >
//! > — `notes/plan/plan.md` §4.2, "Workspace snapshots"
//!
//! `crates/continuum-workspace/src/snapshot.rs` carries the unit-level statement of each
//! property. This file is the standalone witness: it uses only the crate's *public* API,
//! from outside the crate, and it adds the two things a unit test cannot reach — the
//! composition with [`ReferenceStore`], and adversarial sweeps wide enough to be evidence
//! rather than illustration.
//!
//! # Evidence map
//!
//! | Claim | Test |
//! |---|---|
//! | identical content ⇒ identical identity, whatever the input order | [`positive_identical_content_yields_one_identity_in_any_input_order`] |
//! | shared subtrees are shared *in the store*, not merely equal in memory | [`positive_two_snapshots_sharing_a_subtree_publish_it_once`] |
//! | a parent is never published before its children | [`positive_records_publish_children_before_parents`] |
//! | one byte re-derives the path to the root and nothing else | [`negative_one_byte_re_derives_only_the_path_to_the_root`] |
//! | a rename is a new snapshot | [`negative_a_rename_is_a_new_snapshot`] |
//! | a description that is not a tree is a typed refusal | [`negative_a_description_that_is_not_a_tree_is_refused`] |
//! | a totally colliding identity seam conflates nothing | [`adversarial_a_total_identity_collision_conflates_nothing`] |
//! | a partially colliding seam conflates nothing either | [`adversarial_a_length_only_identity_conflates_nothing`] |
//! | the store refuses to file two records under one colliding name | [`adversarial_a_store_refuses_the_colliding_second_record`] |
//! | encodings round-trip, and only one byte string spells a snapshot | [`serialization_round_trips_and_admits_one_spelling`] |
//! | no proper prefix of an encoding decodes | [`serialization_rejects_every_truncation`] |
//! | boundaries: empty, deepest, widest | [`boundary_empty_deepest_and_widest_workspaces`] |
//!
//! # House rules
//!
//! - **Nothing in `src/` is touched.** Every seam used here is public API.
//! - **Two identity seams, for two different jobs.** [`HexIdentity`] is *injective* — the
//!   record's own bytes are the identity, which is ADR-0013's certified discipline — so
//!   "distinct content, distinct identity" under it is a fact rather than a probability;
//!   it is used wherever a negative claim has to be airtight. [`Fnv1aTestIdentity`] is a
//!   fixed-width stand-in used where identities must stay short (deep and wide trees); it
//!   is **not** a release hash and says so.
//! - **Adversarial seams are the instrument, not the environment.** ADR-0013's guarantee
//!   is not "collisions are rare", it is "collisions do not decide anything", so the
//!   collisions here are made certain rather than waited for.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use continuum_workspace::artifact_path::{ArtifactClass, ArtifactHandle};
use continuum_workspace::publication::{
    AbortReason, ActorId, AuditLog, AuthorityLevel, CapabilityDescriptor, CapabilityToken,
    ContentIdentifier, IdentityUnavailable, PublishRefusal, ReferenceStore,
};
use continuum_workspace::snapshot::{
    ChangeKind, MAX_DEPTH, PathError, Snapshot, SnapshotDecodeError, SnapshotError, SnapshotNode,
    WorkspaceContent, WorkspacePath,
};

// --- identity seams ---------------------------------------------------------------------

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
///
/// ADR-0013's first sentence as a [`ContentIdentifier`] — "canonical structural encodings
/// define identity" — so two records have one name exactly when they are one record. Every
/// negative claim in this file that must not rest on a probability rests on this.
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

/// FNV-1a offset basis, 64-bit.
const FNV_OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
/// FNV-1a prime, 64-bit.
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

/// **Not a release hash.** A fixed-width, deliberately non-cryptographic stand-in, used
/// only where an identity has to stay short — a 64-segment path or a 256-child directory
/// would otherwise carry a [`HexIdentity`] that grows with the subtree.
///
/// Four domain-separated FNV-1a lanes concatenated big-endian, rendered as 64 hex
/// characters. The lanes are strongly correlated and the construction has nothing
/// resembling the collision resistance its width suggests; it mirrors
/// `continuum-value`'s `Fnv1aPlaceholder` and is here for the same reason — being
/// obviously inadequate is safer than being plausibly inadequate.
#[derive(Debug, Clone, Copy)]
struct Fnv1aTestIdentity;

impl Fnv1aTestIdentity {
    fn lane(index: u8, bytes: &[u8]) -> u64 {
        let mut state = FNV_OFFSET_BASIS ^ u64::from(index);
        state = state.wrapping_mul(FNV_PRIME);
        for byte in bytes {
            state ^= u64::from(*byte);
            state = state.wrapping_mul(FNV_PRIME);
        }
        state
    }
}

impl ContentIdentifier for Fnv1aTestIdentity {
    fn identify(
        &self,
        class: ArtifactClass,
        content: &[u8],
    ) -> Result<ArtifactHandle, IdentityUnavailable> {
        let mut digest = [0u8; 32];
        for lane in 0..4u8 {
            let value = Self::lane(lane, content).to_be_bytes();
            let at = usize::from(lane) * 8;
            digest[at..at + 8].copy_from_slice(&value);
        }
        ArtifactHandle::new(class, &hex(&digest)).map_err(|_| IdentityUnavailable)
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

/// Names a record by its length, so records collide in several buckets rather than one.
///
/// Nastier than [`ConstantIdentity`] in one specific way: a bug that happens to work when
/// everything shares a single name still fails here.
#[derive(Debug, Clone, Copy)]
struct LengthIdentity;

impl ContentIdentifier for LengthIdentity {
    fn identify(
        &self,
        class: ArtifactClass,
        content: &[u8],
    ) -> Result<ArtifactHandle, IdentityUnavailable> {
        ArtifactHandle::new(class, &format!("len{}", content.len()))
            .map_err(|_| IdentityUnavailable)
    }
}

// --- fixtures ----------------------------------------------------------------------------

fn path(text: &str) -> WorkspacePath {
    WorkspacePath::new(text).expect("test path is well formed")
}

fn describe(files: &[(&str, &[u8])]) -> WorkspaceContent {
    let mut content = WorkspaceContent::new();
    for (text, bytes) in files {
        content
            .insert(path(text), (*bytes).to_vec())
            .expect("test description is a tree");
    }
    content
}

fn build<I: ContentIdentifier>(files: &[(&str, &[u8])], identifier: &I) -> Snapshot {
    Snapshot::build(&describe(files), identifier).expect("test seam names every node")
}

/// A corpus wide enough to have siblings at every level and deep enough to have interior
/// nodes that are neither the root nor a leaf's parent.
const CORPUS: [(&str, &[u8]); 12] = [
    ("Cargo.toml", b"[workspace]"),
    ("README.md", b"# continuum"),
    ("crates/continuum-value/Cargo.toml", b"name = value"),
    ("crates/continuum-value/src/lib.rs", b"pub mod value;"),
    ("crates/continuum-value/src/value.rs", b"pub enum Value {}"),
    ("crates/continuum-workspace/Cargo.toml", b"name = workspace"),
    (
        "crates/continuum-workspace/src/lib.rs",
        b"pub mod snapshot;",
    ),
    (
        "crates/continuum-workspace/src/snapshot.rs",
        b"pub struct Snapshot;",
    ),
    ("notes/plan/plan.md", b"## 4.2 Workspace snapshots"),
    ("notes/plan/adr/0013.md", b"canonical encodings"),
    ("tools/check.py", b"import sys"),
    ("tools/governance/policy.py", b"import re"),
];

/// Every node of a snapshot, root included, keyed by its record bytes.
fn nodes_by_record(snapshot: &Snapshot) -> BTreeMap<&[u8], &SnapshotNode> {
    let mut out: BTreeMap<&[u8], &SnapshotNode> = BTreeMap::new();
    out.insert(snapshot.root().record(), snapshot.root());
    for (_, node) in snapshot.subtrees() {
        out.insert(node.record(), node);
    }
    out
}

/// The identity of every non-root node, by path.
fn subtree_identities(snapshot: &Snapshot) -> BTreeMap<WorkspacePath, String> {
    snapshot
        .subtrees()
        .into_iter()
        .map(|(path, node)| (path, node.identity().to_string()))
        .collect()
}

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

// --- positive: determinism and structural sharing -------------------------------------------

#[test]
fn positive_identical_content_yields_one_identity_in_any_input_order() {
    let forward = build(&CORPUS, &HexIdentity);

    let mut reversed: Vec<(&str, &[u8])> = CORPUS.to_vec();
    reversed.reverse();
    let mut rotated: Vec<(&str, &[u8])> = CORPUS.to_vec();
    rotated.rotate_left(5);
    // Interleaved: deep files first, then shallow ones.
    let mut interleaved: Vec<(&str, &[u8])> = CORPUS.to_vec();
    interleaved.sort_by_key(|(text, _)| std::cmp::Reverse(text.matches('/').count()));

    for order in [reversed, rotated, interleaved] {
        let other = build(&order, &HexIdentity);
        assert_eq!(other.identity(), forward.identity());
        assert_eq!(other, forward);
        assert_eq!(subtree_identities(&other), subtree_identities(&forward));
        assert_eq!(other.encode(), forward.encode());
    }

    // And the listing is in path order, strictly, with no duplicates.
    let paths: Vec<WorkspacePath> = forward
        .subtrees()
        .into_iter()
        .map(|(path, _)| path)
        .collect();
    assert!(paths.windows(2).all(|pair| pair[0] < pair[1]));
    assert_eq!(forward.file_count(), CORPUS.len());
}

#[test]
fn positive_two_snapshots_sharing_a_subtree_publish_it_once() {
    let base = build(&CORPUS, &HexIdentity);
    let mut edited_files: Vec<(&str, &[u8])> = CORPUS.to_vec();
    edited_files[10] = ("tools/check.py", b"import os");
    let edited = build(&edited_files, &HexIdentity);
    assert_ne!(base.identity(), edited.identity());

    let (store, token) = store(HexIdentity);

    // Publishing derives the identity from the bytes; the snapshot never asserts one.
    let mut published: BTreeSet<String> = BTreeSet::new();
    for snapshot in [&base, &edited] {
        for (identity, record) in snapshot.records() {
            let receipt = store
                .publish(ArtifactClass::WorkspaceSnapshot, record.to_vec(), &token)
                .expect("the store names the record exactly as the snapshot did");
            assert_eq!(receipt.handle(), identity);
            published.insert(receipt.handle().to_string());
        }
    }

    // The union of the two record sets is what the store holds: the shared subtrees
    // converged rather than being stored twice. Ordinary publication convergence *is*
    // structural dedup; nothing here needed a second store or a second index.
    let distinct: BTreeSet<String> = base
        .records()
        .into_iter()
        .chain(edited.records())
        .map(|(_, record)| hex(record))
        .collect();
    assert_eq!(published.len(), distinct.len());
    // Fewer than the sum: the two snapshots really do share.
    assert!(published.len() < base.records().len() + edited.records().len());

    // Every published record reads back byte-identical, under both roots.
    for snapshot in [&base, &edited] {
        assert_eq!(
            store
                .read(snapshot.identity(), &token)
                .expect("the root is published"),
            snapshot.root().record()
        );
    }
}

#[test]
fn positive_records_publish_children_before_parents() {
    let snapshot = build(&CORPUS, &HexIdentity);
    let by_record = nodes_by_record(&snapshot);
    let (store, token) = store(HexIdentity);

    let mut published: BTreeSet<String> = BTreeSet::new();
    for (identity, record) in snapshot.records() {
        // Before this record is written, every child it names is already readable.
        if let Some(directory) = by_record.get(record).and_then(|node| node.as_directory()) {
            for (_, child) in directory.children() {
                assert!(
                    published.contains(&child.identity().to_string()),
                    "a parent record was published before its child"
                );
                assert!(store.read(child.identity(), &token).is_ok());
            }
        }
        let receipt = store
            .publish(ArtifactClass::WorkspaceSnapshot, record.to_vec(), &token)
            .expect("published");
        assert_eq!(receipt.handle(), identity);
        published.insert(receipt.handle().to_string());
    }

    // The root is last, and it is the whole snapshot's name.
    assert_eq!(
        snapshot.records().last().map(|(identity, _)| *identity),
        Some(snapshot.identity())
    );
}

// --- negative: sensitivity ------------------------------------------------------------------

#[test]
fn negative_one_byte_re_derives_only_the_path_to_the_root() {
    let base = build(&CORPUS, &HexIdentity);
    let mut edited_files: Vec<(&str, &[u8])> = CORPUS.to_vec();
    edited_files[7] = (
        "crates/continuum-workspace/src/snapshot.rs",
        b"pub struct Snapshot!",
    );
    let edited = build(&edited_files, &HexIdentity);

    assert_ne!(base.identity(), edited.identity());

    let touched: BTreeSet<WorkspacePath> = [
        path("crates"),
        path("crates/continuum-workspace"),
        path("crates/continuum-workspace/src"),
        path("crates/continuum-workspace/src/snapshot.rs"),
    ]
    .into_iter()
    .collect();

    let before = subtree_identities(&base);
    let after = subtree_identities(&edited);
    assert_eq!(
        before.keys().collect::<Vec<_>>(),
        after.keys().collect::<Vec<_>>(),
        "the shape did not change, only one file's bytes"
    );
    for (path, identity) in &before {
        let other = after.get(path).expect("same shape");
        if touched.contains(path) {
            assert_ne!(identity, other, "{path} is on the path to the root");
        } else {
            assert_eq!(identity, other, "{path} is off the path to the root");
        }
    }

    let diff = base.diff(&edited);
    assert_eq!(diff.identity_collisions(), 0);
    assert_eq!(
        diff.changes()
            .map(|change| (change.path().to_string(), change.kind()))
            .collect::<Vec<_>>(),
        [(
            "crates/continuum-workspace/src/snapshot.rs".to_owned(),
            ChangeKind::Modified
        )]
    );
}

#[test]
fn negative_a_rename_is_a_new_snapshot() {
    let before = build(&[("src/a.rs", b"same bytes")], &HexIdentity);
    let after = build(&[("src/b.rs", b"same bytes")], &HexIdentity);

    assert_ne!(before.identity(), after.identity());
    // The *content* did not move, which is precisely why the name has to be in the
    // directory's record rather than only in the leaf's.
    assert_eq!(
        before.node(&path("src/a.rs")).map(SnapshotNode::identity),
        after.node(&path("src/b.rs")).map(SnapshotNode::identity),
    );
    assert_ne!(
        before.node(&path("src")).map(SnapshotNode::identity),
        after.node(&path("src")).map(SnapshotNode::identity),
    );

    // At the identity level a rename is a removal and an addition; richer diff
    // (rename detection) is PR 3 / IMPL-02's.
    assert_eq!(
        before
            .diff(&after)
            .changes()
            .map(|change| (change.path().to_string(), change.kind()))
            .collect::<Vec<_>>(),
        [
            ("src/a.rs".to_owned(), ChangeKind::Removed),
            ("src/b.rs".to_owned(), ChangeKind::Added),
        ]
    );
}

#[test]
fn negative_a_description_that_is_not_a_tree_is_refused() {
    let mut content = WorkspaceContent::new();
    content.insert(path("a/b"), b"1".to_vec()).expect("first");

    assert_eq!(
        content.insert(path("a/b"), b"2".to_vec()),
        Err(SnapshotError::DuplicatePath { path: path("a/b") })
    );
    assert_eq!(
        content.insert(path("a/b/c"), b"3".to_vec()),
        Err(SnapshotError::PathConflict {
            file: path("a/b"),
            descendant: path("a/b/c"),
        })
    );
    // A refusal changes nothing, so the snapshot of the surviving description is the
    // snapshot of a description that never saw the bad inserts.
    assert_eq!(
        Snapshot::build(&content, &HexIdentity).expect("named"),
        build(&[("a/b", b"1")], &HexIdentity)
    );

    // And no path that could escape the workspace is representable at all.
    for text in [
        "",
        "/etc/passwd",
        "../..",
        "a/./b",
        "a/../b",
        "a\0b",
        "a\\b",
    ] {
        assert!(
            WorkspacePath::new(text).is_err(),
            "path {text:?} was accepted"
        );
    }
}

// --- adversarial: colliding identity seams ---------------------------------------------------

#[test]
fn adversarial_a_total_identity_collision_conflates_nothing() {
    let left = build(&[("a", b"one"), ("d/e", b"three")], &ConstantIdentity);
    let right = build(&[("a", b"two"), ("d/e", b"three")], &ConstantIdentity);

    // The collision is total and real.
    assert_eq!(left.identity(), right.identity());
    let names: BTreeSet<String> = left
        .subtrees()
        .into_iter()
        .map(|(_, node)| node.identity().to_string())
        .collect();
    assert_eq!(names.len(), 1);

    // And nothing is conflated: the snapshots are not equal, and the diff is exact.
    assert_ne!(left, right);
    let diff = left.diff(&right);
    assert_eq!(
        diff.changes()
            .map(|change| (change.path().to_string(), change.kind()))
            .collect::<Vec<_>>(),
        [("a".to_owned(), ChangeKind::Modified)]
    );
    // The collisions are counted rather than absorbed: root and the changed leaf.
    assert_eq!(diff.identity_collisions(), 2);

    // Equal content under the same hostile seam still diffs to nothing — the exact
    // comparison decides in both directions, so it cannot be a blanket "always differs".
    let same = build(&[("a", b"one"), ("d/e", b"three")], &ConstantIdentity);
    assert!(left.diff(&same).is_empty());
    assert_eq!(left.diff(&same).identity_collisions(), 0);
}

#[test]
fn adversarial_a_length_only_identity_conflates_nothing() {
    // Same lengths everywhere, so every corresponding node collides, in several buckets
    // rather than one.
    let left = build(&[("a/x", b"1111"), ("a/y", b"2222")], &LengthIdentity);
    let right = build(&[("a/x", b"1111"), ("a/y", b"3333")], &LengthIdentity);

    assert_eq!(left.identity(), right.identity());
    assert!(
        left.subtrees()
            .into_iter()
            .map(|(_, node)| node.identity().to_string())
            .collect::<BTreeSet<_>>()
            .len()
            > 1,
        "the seam should produce several buckets"
    );

    assert_ne!(left, right);
    let diff = left.diff(&right);
    assert_eq!(
        diff.paths().map(ToString::to_string).collect::<Vec<_>>(),
        ["a/y".to_owned()]
    );
    // Root, `a`, and the leaf: three colliding pairs over unequal content.
    assert_eq!(diff.identity_collisions(), 3);
}

#[test]
fn adversarial_a_store_refuses_the_colliding_second_record() {
    // The listing keeps distinct records distinct even when they share a name, which is
    // what lets the store's ADR-0013 exact comparison fire instead of silently
    // overwriting one artifact with another.
    let snapshot = build(&[("a", b"one"), ("b", b"two")], &ConstantIdentity);
    let records = snapshot.records();
    assert_eq!(records.len(), 3, "three distinct records");
    assert_eq!(
        records
            .iter()
            .map(|(identity, _)| identity.to_string())
            .collect::<BTreeSet<_>>()
            .len(),
        1,
        "under one name"
    );

    let (store, token) = store(ConstantIdentity);
    let mut refusals = 0usize;
    for (_, record) in &records {
        match store.publish(ArtifactClass::WorkspaceSnapshot, record.to_vec(), &token) {
            Ok(_) => {}
            Err(PublishRefusal::Aborted(aborted)) => {
                assert_eq!(aborted.reason(), AbortReason::IdentityCollision);
                refusals += 1;
            }
            Err(other) => panic!("unexpected refusal: {other}"),
        }
    }
    assert_eq!(
        refusals, 2,
        "the first record is filed; the other two collide"
    );
}

// --- serialization -----------------------------------------------------------------------------

#[test]
fn serialization_round_trips_and_admits_one_spelling() {
    for snapshot in [
        build(&CORPUS, &HexIdentity),
        build(&[], &HexIdentity),
        build(&[("a", b"")], &HexIdentity),
        build(&CORPUS, &Fnv1aTestIdentity),
    ] {
        let bytes = snapshot.encode();
        let decoded = Snapshot::decode(&bytes, &HexIdentity).expect("round trip");
        assert_eq!(decoded, snapshot);
        assert_eq!(decoded.encode(), bytes);
    }

    // One snapshot, one spelling: mutating any byte either breaks the encoding outright
    // or produces the canonical encoding of a *different* snapshot. There is no second
    // byte string that decodes to the same snapshot.
    let original = build(&[("a/b", b"xy"), ("c", b"z")], &HexIdentity);
    let bytes = original.encode();
    let mut accepted = 0usize;
    for index in 0..bytes.len() {
        for delta in [1u8, 0x7f, 0x80, 0xff] {
            let mut mutated = bytes.clone();
            mutated[index] = mutated[index].wrapping_add(delta);
            if mutated == bytes {
                continue;
            }
            match Snapshot::decode(&mutated, &HexIdentity) {
                Err(_) => {}
                Ok(other) => {
                    accepted += 1;
                    assert_ne!(other, original, "two byte strings spelled one snapshot");
                    assert_eq!(other.encode(), mutated, "an accepted encoding is canonical");
                }
            }
        }
    }
    // The sweep has to actually reach the accepting branch, or it proves nothing about
    // canonicity — only about rejection.
    assert!(
        accepted > 0,
        "no mutation was accepted; the sweep is vacuous"
    );
}

#[test]
fn serialization_rejects_every_truncation() {
    let bytes = build(&CORPUS, &Fnv1aTestIdentity).encode();
    for len in 0..bytes.len() {
        assert!(
            Snapshot::decode(&bytes[..len], &Fnv1aTestIdentity).is_err(),
            "a proper prefix of an encoding decoded at length {len}"
        );
    }
    assert!(Snapshot::decode(&bytes, &Fnv1aTestIdentity).is_ok());

    // Trailing bytes are equally a refusal: an encoding is exactly its own length.
    let mut extended = bytes;
    extended.push(0);
    assert!(matches!(
        Snapshot::decode(&extended, &Fnv1aTestIdentity),
        Err(SnapshotDecodeError::TrailingBytes { .. })
    ));
}

// --- boundaries -----------------------------------------------------------------------------------

#[test]
fn boundary_empty_deepest_and_widest_workspaces() {
    // Empty: a name of its own, stable, and distinguishable from a workspace with one
    // empty file in it.
    let empty = build(&[], &Fnv1aTestIdentity);
    assert!(empty.is_empty());
    assert_eq!(empty.identity(), build(&[], &Fnv1aTestIdentity).identity());
    assert_ne!(
        empty.identity(),
        build(&[("a", b"")], &Fnv1aTestIdentity).identity()
    );
    assert!(empty.diff(&empty).is_empty());

    // Deepest: MAX_DEPTH segments is accepted, MAX_DEPTH + 1 is a typed refusal.
    let deepest: Vec<String> = (0..MAX_DEPTH).map(|index| format!("d{index}")).collect();
    let deepest = WorkspacePath::from_segments(&deepest).expect("at the bound");
    assert_eq!(deepest.depth(), MAX_DEPTH);
    let mut content = WorkspaceContent::new();
    content
        .insert(deepest.clone(), b"deep".to_vec())
        .expect("a tree");
    let deep_snapshot = Snapshot::build(&content, &Fnv1aTestIdentity).expect("named");
    assert_eq!(deep_snapshot.subtrees().len(), MAX_DEPTH);
    assert!(deep_snapshot.node(&deepest).is_some());
    assert!(matches!(
        WorkspacePath::from_segments(vec!["x".to_owned(); MAX_DEPTH + 1]),
        Err(PathError::TooDeep { .. })
    ));
    // It round-trips at the bound too, which is where a decoder's depth check has to be
    // an off-by-one in the right direction.
    let bytes = deep_snapshot.encode();
    assert_eq!(
        Snapshot::decode(&bytes, &Fnv1aTestIdentity).expect("round trip"),
        deep_snapshot
    );

    // Widest: 256 siblings, listed in strict segment order and independent of insertion
    // order.
    let names: Vec<String> = (0..256).map(|index| format!("f{index:03}")).collect();
    let mut wide = WorkspaceContent::new();
    let mut reversed = WorkspaceContent::new();
    for name in &names {
        wide.insert(path(name), name.clone().into_bytes())
            .expect("a tree");
    }
    for name in names.iter().rev() {
        reversed
            .insert(path(name), name.clone().into_bytes())
            .expect("a tree");
    }
    let wide = Snapshot::build(&wide, &Fnv1aTestIdentity).expect("named");
    let reversed = Snapshot::build(&reversed, &Fnv1aTestIdentity).expect("named");
    assert_eq!(wide.identity(), reversed.identity());
    assert_eq!(wide.file_count(), 256);
    let listed: Vec<String> = wide
        .subtrees()
        .into_iter()
        .map(|(path, _)| path.to_string())
        .collect();
    let mut sorted = listed.clone();
    sorted.sort();
    assert_eq!(listed, sorted);
    assert_eq!(listed.first().map(String::as_str), Some("f000"));
    assert_eq!(listed.last().map(String::as_str), Some("f255"));
}
