//! Acceptance evidence for the overlay, fork, seal, and diff halves of `PR-3-IMPL-02`
//! (`notes/plan/notes/START_HERE_IMPLEMENTATION.md`, PR 3's second Implement bullet;
//! `notes/plan/notes/PLAN_REQUIREMENTS.json`, id `PR-3-IMPL-02`).
//!
//! > Clients may create a snapshot from a working tree, overlay an in-memory editor
//! > buffer, or fork an existing snapshot.
//! >
//! > — `notes/plan/plan.md` §4.2, "Workspace snapshots"
//!
//! The unit tests in `src/overlay.rs`, `src/lineage.rs`, `src/seal.rs`, and `src/diff.rs`
//! state each property in isolation. This file is the standalone witness: public API only,
//! from outside the crate, and it adds the two things a unit test cannot — the composition
//! with [`ReferenceStore`], and the whole path from a directory on disk to a published,
//! addressable artifact.
//!
//! # Evidence map
//!
//! | Claim | Test |
//! |---|---|
//! | deriving from a base leaves the base bit-for-bit unchanged | [`positive_an_overlay_never_touches_its_base`] |
//! | an overlay snapshot is an ordinary snapshot, provenance aside | [`positive_an_overlay_snapshot_is_an_ordinary_snapshot`] |
//! | two clients fork one base and evolve independently | [`positive_two_clients_fork_and_evolve_independently`] |
//! | a fork records where it diverged and what it superseded | [`positive_a_fork_carries_its_provenance`] |
//! | sealing publishes children before parents and reads back | [`positive_sealing_publishes_children_before_parents`] |
//! | two seals of identical trees converge on one artifact | [`positive_two_seals_of_identical_trees_converge`] |
//! | forks that share a subtree publish it once | [`positive_two_forks_publish_their_shared_subtree_once`] |
//! | a diff names both sides of every change | [`positive_a_diff_names_both_sides`] |
//! | a colliding seam is surfaced by the diff and refused by the store | [`adversarial_a_colliding_seam_is_surfaced_and_refused`] |
//! | disk to published artifact, end to end | [`positive_import_fork_overlay_seal_end_to_end`] |
//! | an old snapshot stays reproducible after the tree changes | [`positive_an_old_snapshot_survives_the_working_tree`] |
//! | a seal without the authority to publish is refused | [`negative_a_seal_without_authority_is_refused`] |
//!
//! # House rules
//!
//! - **Nothing in `src/` is touched.** Every seam used here is public API.
//! - **Two identity seams, for two jobs.** [`HexIdentity`] is injective, so every negative
//!   claim under it is a fact rather than a probability; [`ConstantIdentity`] names
//!   everything alike, which is how collision handling is observed rather than waited for
//!   (ADR-0013).
//! - **Real directories where the claim is about disk**, under [`std::env::temp_dir`], one
//!   per test, removed before and after.

use std::fs;
use std::path::{Path, PathBuf};

use continuum_workspace::artifact_path::{ArtifactClass, ArtifactHandle};
use continuum_workspace::components::WorkspaceDescriptor;
use continuum_workspace::diff::WorkspaceDiff;
use continuum_workspace::import::DiskImporter;
use continuum_workspace::lineage::{Fork, ForkName};
use continuum_workspace::overlay::Overlay;
use continuum_workspace::publication::{
    ActorId, AuditLog, AuthorityLevel, CapabilityDescriptor, CapabilityToken, ContentIdentifier,
    IdentityUnavailable, PublishRefusal, ReferenceStore,
};
use continuum_workspace::seal::{SealError, SealedWorkspace, rebind_components};
use continuum_workspace::snapshot::{ChangeKind, Snapshot, WorkspaceContent, WorkspacePath};

// --- identity seams -------------------------------------------------------------------------

/// The record's own bytes, in hex: injective, exactly ADR-0013's certified discipline.
#[derive(Clone)]
struct HexIdentity;

impl ContentIdentifier for HexIdentity {
    fn identify(
        &self,
        class: ArtifactClass,
        content: &[u8],
    ) -> Result<ArtifactHandle, IdentityUnavailable> {
        let mut token = String::with_capacity(content.len() * 2);
        for byte in content {
            token.push(char::from(b"0123456789abcdef"[usize::from(byte >> 4)]));
            token.push(char::from(b"0123456789abcdef"[usize::from(byte & 0x0f)]));
        }
        ArtifactHandle::new(class, &token).map_err(|_| IdentityUnavailable)
    }
}

/// Names every record `ws_same`. The worst identity function that exists, and the
/// instrument for observing what a collision does rather than hoping one never happens.
#[derive(Clone)]
struct ConstantIdentity;

impl ContentIdentifier for ConstantIdentity {
    fn identify(
        &self,
        class: ArtifactClass,
        _content: &[u8],
    ) -> Result<ArtifactHandle, IdentityUnavailable> {
        ArtifactHandle::new(class, "same").map_err(|_| IdentityUnavailable)
    }
}

// --- fixtures ---------------------------------------------------------------------------------

fn path(text: &str) -> WorkspacePath {
    WorkspacePath::new(text).expect("a test path is valid")
}

fn name(text: &str) -> ForkName {
    ForkName::new(text).expect("a test fork name is valid")
}

fn snapshot_of<I: ContentIdentifier>(files: &[(&str, &[u8])], identifier: &I) -> Snapshot {
    let mut content = WorkspaceContent::new();
    for (text, bytes) in files {
        content
            .insert(path(text), (*bytes).to_vec())
            .expect("insert");
    }
    Snapshot::build(&content, identifier).expect("build")
}

fn descriptor_of(files: &[(&str, &[u8])]) -> WorkspaceDescriptor {
    WorkspaceDescriptor::builder()
        .dependencies(b"[[package]]".to_vec())
        .build(snapshot_of(files, &HexIdentity), &HexIdentity)
        .expect("descriptor")
}

/// A store with a publish/read capability and a separate audit capability.
///
/// Two tokens rather than one: auditing sits at `promote` because everything it exposes is
/// an existence oracle, and a test that published with an audit-capable token would not be
/// exercising the authority a client actually holds.
fn store<I: ContentIdentifier + 'static>(
    identifier: I,
) -> (ReferenceStore, CapabilityToken, CapabilityToken) {
    let publisher = CapabilityToken::mint("publisher").expect("token");
    let operator = CapabilityToken::mint("operator").expect("token");
    let store = ReferenceStore::builder(identifier, AuditLog::new())
        .capability(CapabilityDescriptor::new(
            publisher.clone(),
            ActorId::new("editor"),
            AuthorityLevel::Propose,
        ))
        .capability(CapabilityDescriptor::new(
            operator.clone(),
            ActorId::new("operator"),
            AuthorityLevel::Promote,
        ))
        .build();
    (store, publisher, operator)
}

fn scratch(test: &str) -> PathBuf {
    let root = std::env::temp_dir()
        .join("continuum-workspace-lifecycle")
        .join(test);
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).expect("a scratch directory is creatable");
    root
}

fn cleanup(root: &Path) {
    let _ = fs::remove_dir_all(root);
}

fn write(root: &Path, relative: &str, content: &[u8]) {
    let file = root.join(relative);
    if let Some(parent) = file.parent() {
        fs::create_dir_all(parent).expect("a parent directory is creatable");
    }
    fs::write(&file, content).expect("a scratch file is writable");
}

// --- overlay ------------------------------------------------------------------------------------

#[test]
fn positive_an_overlay_never_touches_its_base() {
    let base = snapshot_of(
        &[("src/lib.rs", b"fn main() {}"), ("README.md", b"hello")],
        &HexIdentity,
    );
    // The witness is a full copy taken before: equality on `Snapshot` is recursive content
    // equality, so this compares every byte of every file and every child of every
    // directory, not just the root name.
    let witness = base.clone();

    let mut overlay = Overlay::new();
    overlay
        .write(path("src/lib.rs"), b"fn main() { todo!() }".to_vec())
        .expect("write");
    overlay.remove(path("README.md")).expect("remove");
    let derived = overlay.derive(&base, &HexIdentity).expect("derive");

    assert_eq!(base, witness);
    assert_eq!(base.identity(), witness.identity());
    assert_eq!(base.encode(), witness.encode());
    assert_eq!(base.file_count(), 2);

    // Deriving twice from the same base is the same answer, which it could not be if the
    // first derivation had consumed anything.
    let again = overlay.derive(&base, &HexIdentity).expect("derive");
    assert_eq!(again.snapshot().identity(), derived.snapshot().identity());
    assert_eq!(derived.base(), base.identity());
}

#[test]
fn positive_an_overlay_snapshot_is_an_ordinary_snapshot() {
    let base = snapshot_of(&[("src/lib.rs", b"fn main() {}")], &HexIdentity);
    let mut overlay = Overlay::new();
    overlay
        .write(path("src/lib.rs"), b"edited".to_vec())
        .expect("write");
    overlay
        .write(path("docs/new.md"), b"fresh".to_vec())
        .expect("write");
    let derived = overlay.derive(&base, &HexIdentity).expect("derive");

    // Provenance says where the bytes came from; identity says what they are. The derived
    // snapshot is byte-identical to the same content imported directly.
    let direct = snapshot_of(
        &[("src/lib.rs", b"edited"), ("docs/new.md", b"fresh")],
        &HexIdentity,
    );
    assert_eq!(derived.snapshot(), &direct);
    assert_eq!(derived.snapshot().identity(), direct.identity());
    assert_eq!(derived.snapshot().encode(), direct.encode());

    assert!(derived.is_overlaid(&path("src/lib.rs")));
    assert!(derived.is_overlaid(&path("docs/new.md")));
    assert_eq!(
        derived.overlaid_paths().collect::<Vec<_>>(),
        [&path("docs/new.md"), &path("src/lib.rs")],
    );

    // It publishes like any other snapshot, and a derived snapshot can be derived from.
    let (store, publisher, _) = store(HexIdentity);
    for (identity, record) in derived.snapshot().records() {
        let receipt = store
            .publish(
                ArtifactClass::WorkspaceSnapshot,
                record.to_vec(),
                &publisher,
            )
            .expect("publish");
        assert_eq!(receipt.handle(), identity);
    }
    assert!(store.read(direct.identity(), &publisher).is_ok());
}

// --- fork ------------------------------------------------------------------------------------------

#[test]
fn positive_two_clients_fork_and_evolve_independently() {
    let base = snapshot_of(
        &[
            ("src/lib.rs", b"fn main() {}"),
            ("shared/util.rs", b"shared"),
        ],
        &HexIdentity,
    );

    let alice = Fork::diverge(name("alice"), &base);
    let bob = Fork::diverge(name("bob"), &base);
    assert_eq!(alice.identity(), bob.identity());

    let mut alices = Overlay::new();
    alices
        .write(path("src/lib.rs"), b"alice was here".to_vec())
        .expect("write");
    let mut bobs = Overlay::new();
    bobs.write(path("src/lib.rs"), b"bob was here".to_vec())
        .expect("write");

    let alice = alice.advance(&alices, &HexIdentity).expect("advance");
    let bob = bob.advance(&bobs, &HexIdentity).expect("advance");

    // Three distinct workspaces, and the base is still one of them.
    assert_ne!(alice.identity(), bob.identity());
    assert_ne!(alice.identity(), base.identity());
    assert_ne!(bob.identity(), base.identity());
    assert_eq!(base.file_count(), 2);

    // Neither line's edit is visible in the other, and both still hold the untouched file
    // under exactly the identity the base gave it.
    assert_eq!(
        alice
            .head()
            .node(&path("shared/util.rs"))
            .map(|node| node.identity()),
        base.node(&path("shared/util.rs"))
            .map(|node| node.identity()),
    );
    assert_eq!(
        bob.head()
            .node(&path("shared/util.rs"))
            .map(|node| node.identity()),
        base.node(&path("shared/util.rs"))
            .map(|node| node.identity()),
    );

    // Analyzing one against the other is a path-level diff of exactly one file.
    let diff = WorkspaceDiff::between(alice.head(), bob.head());
    assert_eq!(diff.len(), 1);
    assert_eq!(
        diff.changes()
            .next()
            .map(|change| change.path().to_string()),
        Some("src/lib.rs".to_owned()),
    );
}

#[test]
fn positive_a_fork_carries_its_provenance() {
    let base = snapshot_of(&[("a.rs", b"a")], &HexIdentity);
    let fork = Fork::diverge(name("feature/x"), &base);
    assert_eq!(fork.origin(), base.identity());
    assert_eq!(fork.parent(), None);

    let mut first = Overlay::new();
    first.write(path("a.rs"), b"a1".to_vec()).expect("write");
    let one = fork.advance(&first, &HexIdentity).expect("advance");

    let mut second = Overlay::new();
    second.write(path("a.rs"), b"a2".to_vec()).expect("write");
    let two = one.advance(&second, &HexIdentity).expect("advance");

    // Origin is the divergence point and never moves; parent is one `SUPERSEDES` step.
    assert_eq!(two.origin(), base.identity());
    assert_eq!(two.parent(), Some(one.identity()));
    assert_eq!(one.parent(), Some(base.identity()));
    assert_eq!(two.name(), &name("feature/x"));

    // Every earlier value of the line is still exactly what it was.
    assert_eq!(fork.identity(), base.identity());
    assert_ne!(one.identity(), two.identity());
}

// --- seal -------------------------------------------------------------------------------------------

#[test]
fn positive_sealing_publishes_children_before_parents() {
    let descriptor = descriptor_of(&[("src/lib.rs", b"fn main() {}"), ("README.md", b"hi")]);
    let expected: Vec<ArtifactHandle> = descriptor
        .records()
        .into_iter()
        .map(|(identity, _)| identity.clone())
        .collect();
    let (store, publisher, operator) = store(HexIdentity);

    let sealed = SealedWorkspace::seal(descriptor, &store, &publisher).expect("seal");

    // Receipts are in records order, which is children before parents, and the last record
    // is the descriptor's own.
    let published: Vec<ArtifactHandle> = sealed
        .receipts()
        .iter()
        .map(|receipt| receipt.handle().clone())
        .collect();
    assert_eq!(published, expected);
    assert_eq!(published.last(), Some(sealed.descriptor_identity()),);

    // Everything the sealed workspace names is readable under that name — the whole point
    // of sealing.
    for identity in &published {
        assert!(store.read(identity, &publisher).is_ok());
    }
    assert!(store.read(sealed.snapshot_identity(), &publisher).is_ok());

    // And the store agrees it holds exactly those artifacts and no others.
    let audit = store.audit_view(&operator).expect("audit");
    assert_eq!(audit.published_count(), published.len());
    assert!(audit.fsck().is_empty());
}

#[test]
fn positive_two_seals_of_identical_trees_converge() {
    let (store, publisher, operator) = store(HexIdentity);

    let first = SealedWorkspace::seal(
        descriptor_of(&[("src/lib.rs", b"fn main() {}")]),
        &store,
        &publisher,
    )
    .expect("seal");
    let after_first = store
        .audit_view(&operator)
        .expect("audit")
        .published_count();

    // A second, independently assembled workspace with byte-identical content.
    let second = SealedWorkspace::seal(
        descriptor_of(&[("src/lib.rs", b"fn main() {}")]),
        &store,
        &publisher,
    )
    .expect("seal");

    assert_eq!(first.descriptor_identity(), second.descriptor_identity());
    assert_eq!(first.snapshot_identity(), second.snapshot_identity());

    // Convergence: not one new artifact, and every publication still receipted — a dedup
    // that swallowed receipts would be an existence oracle.
    let audit = store.audit_view(&operator).expect("audit");
    assert_eq!(audit.published_count(), after_first);
    assert_eq!(
        audit.receipts(first.descriptor_identity()).len(),
        2,
        "both publications are in the ledger",
    );
    assert_eq!(second.receipts().len(), first.receipts().len());
    assert!(audit.fsck().is_empty());
}

#[test]
fn positive_two_forks_publish_their_shared_subtree_once() {
    let base = snapshot_of(
        &[("shared/util.rs", b"shared"), ("src/lib.rs", b"base")],
        &HexIdentity,
    );
    let mut alices = Overlay::new();
    alices
        .write(path("src/lib.rs"), b"alice".to_vec())
        .expect("write");
    let mut bobs = Overlay::new();
    bobs.write(path("src/lib.rs"), b"bob".to_vec())
        .expect("write");

    let alice = Fork::diverge(name("alice"), &base)
        .advance(&alices, &HexIdentity)
        .expect("advance");
    let bob = Fork::diverge(name("bob"), &base)
        .advance(&bobs, &HexIdentity)
        .expect("advance");

    let (store, publisher, operator) = store(HexIdentity);
    let one = SealedWorkspace::seal(
        rebind_components(&descriptor_of(&[("x", b"x")]))
            .build(alice.into_head(), &HexIdentity)
            .expect("descriptor"),
        &store,
        &publisher,
    )
    .expect("seal");
    let after_one = store
        .audit_view(&operator)
        .expect("audit")
        .published_count();

    let two = SealedWorkspace::seal(
        rebind_components(&descriptor_of(&[("x", b"x")]))
            .build(bob.into_head(), &HexIdentity)
            .expect("descriptor"),
        &store,
        &publisher,
    )
    .expect("seal");
    let after_two = store
        .audit_view(&operator)
        .expect("audit")
        .published_count();

    assert_ne!(one.descriptor_identity(), two.descriptor_identity());
    // The shared subtree, the lockfile component, and the untouched file are already in the
    // store, so the second seal adds strictly fewer artifacts than it published records.
    assert!(after_two - after_one < two.receipts().len());
    let shared = one
        .snapshot()
        .node(&path("shared"))
        .expect("the shared subtree");
    assert_eq!(
        two.snapshot()
            .node(&path("shared"))
            .map(|node| node.identity()),
        Some(shared.identity()),
    );
}

#[test]
fn negative_a_seal_without_authority_is_refused() {
    let reader = CapabilityToken::mint("reader").expect("token");
    let store = ReferenceStore::builder(HexIdentity, AuditLog::new())
        .capability(CapabilityDescriptor::new(
            reader.clone(),
            ActorId::new("reader"),
            AuthorityLevel::Read,
        ))
        .build();

    let refusal = SealedWorkspace::seal(descriptor_of(&[("a.rs", b"a")]), &store, &reader);
    assert!(matches!(
        refusal,
        Err(SealError::Refused {
            refusal: PublishRefusal::CapabilityDenied(_),
            ..
        }),
    ));

    // A handle is not authorization: nothing was published, and the refusal says nothing
    // about what the store holds.
    let unregistered = CapabilityToken::mint("nobody").expect("token");
    assert!(store.audit_view(&unregistered).is_err());
}

// --- diff ---------------------------------------------------------------------------------------------

#[test]
fn positive_a_diff_names_both_sides() {
    let base = snapshot_of(
        &[
            ("keep.rs", b"keep"),
            ("gone.rs", b"gone"),
            ("edit.rs", b"before"),
        ],
        &HexIdentity,
    );

    let mut overlay = Overlay::new();
    overlay.remove(path("gone.rs")).expect("remove");
    overlay
        .write(path("edit.rs"), b"after".to_vec())
        .expect("write");
    overlay
        .write(path("added.rs"), b"added".to_vec())
        .expect("write");
    let derived = overlay.derive(&base, &HexIdentity).expect("derive");

    let diff = WorkspaceDiff::of_overlay(&base, &derived).expect("diff");
    assert_eq!(
        diff.changes()
            .map(|change| (change.path().to_string(), change.kind()))
            .collect::<Vec<_>>(),
        [
            ("added.rs".to_owned(), ChangeKind::Added),
            ("edit.rs".to_owned(), ChangeKind::Modified),
            ("gone.rs".to_owned(), ChangeKind::Removed),
        ],
    );

    let added = diff.added().next().expect("an addition");
    assert!(added.before().is_none());
    assert_eq!(
        added.after(),
        derived
            .snapshot()
            .node(&path("added.rs"))
            .map(|node| node.identity()),
    );

    let removed = diff.removed().next().expect("a removal");
    assert!(removed.after().is_none());
    assert_eq!(
        removed.before(),
        base.node(&path("gone.rs")).map(|node| node.identity()),
    );

    let modified = diff.modified().next().expect("a modification");
    assert_eq!(
        modified.before(),
        base.node(&path("edit.rs")).map(|node| node.identity()),
    );
    assert_eq!(
        modified.after(),
        derived
            .snapshot()
            .node(&path("edit.rs"))
            .map(|node| node.identity()),
    );
    assert_ne!(modified.before(), modified.after());
    assert_eq!(diff.identity_collisions(), 0);

    // Diffing against a base the derivation never had is a typed refusal, not a wrong
    // answer.
    let stranger = snapshot_of(&[("other.rs", b"other")], &HexIdentity);
    let mismatch = WorkspaceDiff::of_overlay(&stranger, &derived).expect_err("a mismatch");
    assert_eq!(mismatch.expected(), base.identity());
    assert_eq!(mismatch.found(), stranger.identity());
}

#[test]
fn adversarial_a_colliding_seam_is_surfaced_and_refused() {
    // Every record is named `ws_same`, so identity decides nothing at all.
    let before = snapshot_of(&[("a.rs", b"before")], &ConstantIdentity);
    let after = snapshot_of(&[("a.rs", b"after")], &ConstantIdentity);

    let diff = WorkspaceDiff::between(&before, &after);
    // The change is still found, by exact comparison, and the collisions are reported as
    // data rather than absorbed.
    assert_eq!(diff.modified().count(), 1);
    assert!(diff.identity_collisions() > 0);
    assert_eq!(
        diff.modified().next().and_then(|change| change.before()),
        diff.modified().next().and_then(|change| change.after()),
    );

    // And the store refuses to file two different records under one name rather than
    // conflating them: sealing the second workspace aborts, it does not silently converge.
    let (store, publisher, _) = store(ConstantIdentity);
    let first = WorkspaceDescriptor::builder()
        .build(before, &ConstantIdentity)
        .expect("descriptor");
    let second = WorkspaceDescriptor::builder()
        .build(after, &ConstantIdentity)
        .expect("descriptor");

    assert!(SealedWorkspace::seal(first, &store, &publisher).is_err());
    assert!(matches!(
        SealedWorkspace::seal(second, &store, &publisher),
        Err(SealError::Refused {
            refusal: PublishRefusal::Aborted(_),
            ..
        }),
    ));
}

// --- end to end --------------------------------------------------------------------------------------------

#[test]
fn positive_import_fork_overlay_seal_end_to_end() {
    let root = scratch("end-to-end");
    write(&root, "Cargo.lock", b"[[package]]\nname = \"a\"");
    write(
        &root,
        "rust-toolchain.toml",
        b"[toolchain]\nchannel = \"1.90.0\"",
    );
    write(&root, "src/lib.rs", b"fn main() {}");
    write(&root, "src/util.rs", b"fn util() {}");

    let imported = DiskImporter::new()
        .import(&root, &HexIdentity)
        .expect("import");
    let descriptor = imported.descriptor().clone();

    // Fork the imported tree and edit one file in an editor buffer that never reaches the
    // disk.
    let fork = Fork::diverge(name("editor"), imported.snapshot());
    let mut overlay = Overlay::new();
    overlay
        .write(path("src/lib.rs"), b"fn main() { todo!() }".to_vec())
        .expect("write");
    let derived = overlay.derive(fork.head(), &HexIdentity).expect("derive");
    let diff = WorkspaceDiff::of_overlay(fork.head(), &derived).expect("diff");
    assert_eq!(diff.len(), 1);
    assert_eq!(diff.modified().count(), 1);
    assert!(derived.is_overlaid(&path("src/lib.rs")));

    let advanced = fork.advance_to(derived.into_snapshot());
    assert_eq!(advanced.parent(), Some(imported.snapshot().identity()));

    // Seal it with the components the import recognized: an editor buffer does not change
    // the lockfile, and the disk may have moved on.
    let sealed = {
        let (store, publisher, operator) = store(HexIdentity);
        let sealed = SealedWorkspace::seal(
            rebind_components(&descriptor)
                .build(advanced.into_head(), &HexIdentity)
                .expect("descriptor"),
            &store,
            &publisher,
        )
        .expect("seal");
        assert!(store.read(sealed.descriptor_identity(), &publisher).is_ok());
        assert!(store.read(sealed.snapshot_identity(), &publisher).is_ok());
        assert!(
            store
                .audit_view(&operator)
                .expect("audit")
                .fsck()
                .is_empty()
        );
        sealed
    };

    // The sealed value outlives the store it was published to: it is a name, not a handle
    // onto a live process.
    assert_eq!(
        sealed
            .descriptor()
            .dependencies()
            .map(|component| component.content()),
        Some(b"[[package]]\nname = \"a\"".as_slice()),
    );
    assert_eq!(
        sealed
            .snapshot()
            .node(&path("src/lib.rs"))
            .and_then(|node| node.as_file())
            .map(|file| file.content()),
        Some(b"fn main() { todo!() }".as_slice()),
    );
    // The imported tree is untouched by everything that happened after it.
    assert_eq!(
        imported
            .snapshot()
            .node(&path("src/lib.rs"))
            .and_then(|node| node.as_file())
            .map(|file| file.content()),
        Some(b"fn main() {}".as_slice()),
    );

    cleanup(&root);
}

#[test]
fn positive_an_old_snapshot_survives_the_working_tree() {
    let root = scratch("old-snapshot-survives");
    write(&root, "src/lib.rs", b"fn main() {}");
    write(&root, "README.md", b"hello");

    let before = DiskImporter::new()
        .import(&root, &HexIdentity)
        .expect("import");
    let old = before.snapshot().clone();
    let old_identity = old.identity().clone();
    let old_encoding = old.encode();

    // The working tree moves on: a file is edited, one is deleted, one is added.
    write(&root, "src/lib.rs", b"fn main() { todo!() }");
    fs::remove_file(root.join("README.md")).expect("remove");
    write(&root, "src/new.rs", b"fn new() {}");

    let after = DiskImporter::new()
        .import(&root, &HexIdentity)
        .expect("import");

    // The old snapshot is exactly what it was: same identity, same bytes, same content,
    // and it still decodes to itself.
    assert_eq!(old.identity(), &old_identity);
    assert_eq!(old.encode(), old_encoding);
    assert_eq!(
        Snapshot::decode(&old_encoding, &HexIdentity).expect("decode"),
        old,
    );
    assert_eq!(
        old.node(&path("README.md"))
            .and_then(|node| node.as_file())
            .map(|file| file.content()),
        Some(b"hello".as_slice()),
    );

    // And the change is describable as a diff between two named artifacts.
    let diff = WorkspaceDiff::between(&old, after.snapshot());
    assert_eq!(
        diff.changes()
            .map(|change| (change.path().to_string(), change.kind()))
            .collect::<Vec<_>>(),
        [
            ("README.md".to_owned(), ChangeKind::Removed),
            ("src/lib.rs".to_owned(), ChangeKind::Modified),
            ("src/new.rs".to_owned(), ChangeKind::Added),
        ],
    );

    cleanup(&root);
}
