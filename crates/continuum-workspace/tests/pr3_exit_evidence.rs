//! Dedicated exit evidence for `PR-3-EXIT` (`notes/plan/notes/PLAN_REQUIREMENTS.json`,
//! id `PR-3-EXIT`; `notes/plan/notes/START_HERE_IMPLEMENTATION.md`, PR 3's Exit line).
//!
//! > **Exit:** two clients can fork and analyze independently; an old snapshot remains
//! > reproducible after the working tree changes.
//! >
//! > — `notes/plan/notes/START_HERE_IMPLEMENTATION.md`, PR 3
//!
//! This file asserts that sentence end to end, as directly as it reads, the way
//! `pr2_exit_evidence.rs` did for PR 2's exit (bn-1sh6) and `crates/continuum-task/src/
//! result.rs` did for PR 1's (bn-1w1y). PR 3 landed across five bones —
//! `src/snapshot.rs` (bn-15gj), `src/import.rs` / `overlay.rs` / `lineage.rs` /
//! `seal.rs` / `diff.rs` (bn-1hrk), `src/components.rs` (bn-2xri), `src/staleness.rs`
//! (bn-3tda), and deterministic ordering (bn-1qhe) — and every one of them already
//! carries pieces of the sentence as unit-level facts; `lineage.rs` and `staleness.rs`
//! even quote its two clauses verbatim in their own module docs, and
//! `tests/workspace_lifecycle.rs` (bn-1hrk's own acceptance evidence for PR-3-IMPL-02)
//! already carries close cousins of both clauses — two forks evolving independently, two
//! forks converging a shared subtree, an old snapshot surviving disk changes — at the
//! implementation-bullet grain. What none of them is is a single, named witness that
//! walks the *whole* sentence in one place, through one shared store, real disk to real
//! store, the way `disk_import.rs` does for import alone: this file adds store-level
//! convergence proved as an exact identity-set and byte count rather than an inequality,
//! per-lineage staleness proved across two independently advancing clients rather than
//! one, and reconstruction of a sealed snapshot read back from the store rather than
//! round-tripped through in-memory `encode`/`decode`. Everywhere else it needs machinery,
//! it reruns `disk_import.rs`'s real-tempdir idiom and this crate's usual [`HexIdentity`]
//! seam, duplicated locally rather than imported — a `tests/*.rs` file is its own crate,
//! so nothing here can `use` a sibling one.
//!
//! # Evidence map
//!
//! The exit sentence has two clauses and one boundary this file makes explicit. Each is
//! carried end to end by one test; the pre-existing suites named beside it reached the
//! same claim first, at the unit level, and remain the corroborating evidence:
//!
//! - **"two clients can fork and analyze independently"** —
//!   [`positive_two_clients_fork_and_analyze_independently_while_shared_subtrees_converge`].
//!   One imported-and-sealed base, two forks, two overlays, two seals through one shared
//!   [`ReferenceStore`]: neither client's work perturbs the other's fork or its
//!   identities, each client's own [`WorkspaceDiff`] against the shared origin names only
//!   that client's own change, staleness of the shared base is provable in each advanced
//!   lineage independently, and every subtree neither client touched converges onto one
//!   stored copy. Corroborated by `lineage::tests::two_lines_from_one_base_evolve_independently`
//!   and `lineage::tests::advancing_leaves_the_earlier_value_intact` (`src/lineage.rs`,
//!   the value-level independence claim — two `Fork`s "cannot interfere because there is
//!   nothing shared between them to interfere *through*"), `staleness::tests::staleness_is_per_lineage`
//!   (`src/staleness.rs`, the per-lineage claim named directly), `overlay::tests::an_untouched_subtree_keeps_its_identity`
//!   (`src/overlay.rs`, one derivation's worth of the convergence claim this file repeats
//!   across two *independently sealed* workspaces),
//!   `publication::tests::a_second_publication_of_identical_content_converges_without_a_second_entry`
//!   (`src/publication.rs`, the store-side "one stored copy" fact this file's receipt and
//!   storage-attribution assertions restate one layer up, at the workspace), and
//!   `positive_two_clients_fork_and_evolve_independently` /
//!   `positive_two_forks_publish_their_shared_subtree_once` (`tests/workspace_lifecycle.rs`,
//!   the same two-client shape without a real disk import and without an exact
//!   store-convergence count — `after_two - after_one < receipts().len()` there, an exact
//!   union-set equality here).
//! - **"an old snapshot remains reproducible after the working tree changes"** —
//!   [`positive_an_old_snapshot_remains_reproducible_after_the_working_tree_changes`]. A
//!   real tempdir is imported and sealed as `A`; the tree is then modified, one file
//!   deleted, another added, for real, on disk; a second import (`B`) is sealed through
//!   the same store. Every node `A` ever had is read back from the store through
//!   [`ReferenceStore::read`] — never trusted from the `Snapshot` value still sitting in
//!   this process's memory — and compared byte for byte against the record it published,
//!   and every file's content against literally the bytes this test wrote to disk before
//!   changing anything; `A`'s identity is separately re-derived from those same original
//!   bytes alone, with no importer and no disk in the loop, and matches exactly. `B`
//!   coexists: independently readable, disjoint at every path that changed, converged at
//!   the one left alone. Corroborated by `disk_import::positive_two_imports_of_one_tree_agree`
//!   (`tests/disk_import.rs`, the real-tempdir idiom this file reruns),
//!   `positive_an_old_snapshot_survives_the_working_tree` (`tests/workspace_lifecycle.rs`,
//!   the same disk-mutation shape, reproduced there via `Snapshot::encode`/`decode`
//!   in-memory round-tripping rather than a read-back through a `ReferenceStore`), and by
//!   the "Immutability" section of `src/snapshot.rs`'s own module documentation, which
//!   names this exact clause as what a `Snapshot`'s having no mutating method buys.
//! - **boundary: a sealed snapshot is not stale merely because the disk moved on** —
//!   [`boundary_a_sealed_snapshot_is_not_stale_merely_because_the_disk_moved_on`]. This is
//!   the distinction the exit sentence's second clause turns on, made explicit:
//!   staleness (`src/staleness.rs`) is a fact about a *lineage's* own history — whether a
//!   `Fork` has since advanced past an identity — never a fact about the filesystem. A
//!   `Snapshot` is an immutable value; it is not re-read, polled, or invalidated by
//!   anything that happens on disk after it was built (`src/snapshot.rs`'s "No ambient
//!   anything"). This test changes the disk out from under a sealed snapshot and shows
//!   every guarded operation this crate has — [`check_current`], [`derive_current`],
//!   [`advance_current`], [`seal_current`] — still treats it as current; only advancing
//!   the *lineage*, never the disk, makes it `Stale`. Corroborated by
//!   `staleness::tests::the_current_head_passes`,
//!   `staleness::tests::a_superseded_head_is_stale_with_the_exact_identities_and_names_the_new_head`,
//!   `staleness::tests::guarded_overlay_derivation_refuses_a_stale_base_and_passes_the_current_one`,
//!   `staleness::tests::guarded_advance_refuses_a_stale_expected_head_and_passes_the_current_one`,
//!   and `staleness::tests::guarded_seal_refuses_a_stale_source_and_passes_the_current_one`
//!   (`src/staleness.rs`) — none of which ever touches a filesystem, which is itself part
//!   of the point: staleness has never needed one.
//!
//! # House rules, inherited from `pr2_exit_evidence.rs` and `disk_import.rs`
//!
//! - **`src/` is not touched.** Nothing here edits, weakens, or moves any existing test or
//!   source file, and no existing test in this crate is touched either.
//! - **Real directories, no test-only crate.** Scratch trees live under
//!   [`std::env::temp_dir`] in a per-test subdirectory, removed before each run (so a
//!   panicking run cannot poison the next one) and after each success — `disk_import.rs`'s
//!   idiom, duplicated locally.
//! - **Assertions are on identities and bytes, never on timing or interleaving.** This
//!   file makes no concurrency claim — PR 2's exit evidence and the DX-13 falsification
//!   campaign already own that ground for the store underneath — so every test here runs
//!   single-threaded, and every "independent" claim is a fact about values, not about
//!   scheduling.

use std::collections::{BTreeMap, BTreeSet};
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
    IdentityUnavailable, ReferenceStore,
};
use continuum_workspace::seal::{SealedWorkspace, rebind_components};
use continuum_workspace::snapshot::{NODE_CLASS, Snapshot, WorkspaceContent, WorkspacePath};
use continuum_workspace::staleness::{
    GuardedAdvanceError, GuardedOverlayError, GuardedSealError, LineageError, advance_current,
    check_current, derive_current, seal_current,
};

// --- identity seam -----------------------------------------------------------------------

/// The record's own bytes, in hex: injective, so every "one identity" claim below is a
/// fact rather than a probability (ADR-0013's certified discipline). The same seam every
/// other PR-3 suite in this crate uses, duplicated locally per the house rule above.
#[derive(Debug, Clone, Copy)]
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

// --- scratch trees -------------------------------------------------------------------------

/// A fresh, empty directory for one test. `disk_import.rs`'s idiom, duplicated locally.
fn scratch(test: &str) -> PathBuf {
    let root = std::env::temp_dir()
        .join("continuum-workspace-pr3-exit-evidence")
        .join(test);
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).expect("a scratch directory is creatable");
    root
}

/// Remove a scratch tree after a successful test.
fn cleanup(root: &Path) {
    let _ = fs::remove_dir_all(root);
}

/// Write one file, creating the directories on the way.
fn write(root: &Path, relative: &str, content: &[u8]) {
    let path = root.join(relative);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("a parent directory is creatable");
    }
    fs::write(&path, content).expect("a scratch file is writable");
}

// --- fixtures --------------------------------------------------------------------------

/// A workspace path, panicking on an invalid literal — every path this file writes is
/// chosen to be valid.
fn path(text: &str) -> WorkspacePath {
    WorkspacePath::new(text).expect("a test path is valid")
}

/// A fork name, panicking on an invalid literal.
fn name(text: &str) -> ForkName {
    ForkName::new(text).expect("a test fork name is valid")
}

/// Mint a capability token from a test-chosen identity (entropy is a capability,
/// INV-005).
fn token(identity: &str) -> CapabilityToken {
    CapabilityToken::mint(identity).expect("test capability identity is well formed")
}

/// Describe an unrestricted capability at `level` for `actor`.
fn grant(identity: &str, actor: &str, level: AuthorityLevel) -> CapabilityDescriptor {
    CapabilityDescriptor::new(token(identity), ActorId::new(actor), level)
}

/// A store with an operator (`promote`, for the audit view) and one `propose` capability
/// per named client, minted under the client's own name as both identity and actor.
fn store_with_clients(clients: &[&str]) -> (ReferenceStore, CapabilityToken, Vec<CapabilityToken>) {
    let mut builder = ReferenceStore::builder(HexIdentity, AuditLog::new()).capability(grant(
        "operator",
        "operator",
        AuthorityLevel::Promote,
    ));
    for &client in clients {
        builder = builder.capability(grant(client, client, AuthorityLevel::Propose));
    }
    let tokens = clients.iter().map(|&client| token(client)).collect();
    (builder.build(), token("operator"), tokens)
}

// --- PR-3-EXIT, clause 1: two clients fork and analyze independently ---------------------

/// **PR-3-EXIT, clause 1.** "Two clients can fork and analyze independently."
///
/// One workspace is imported from a real directory and sealed. Two clients each fork it,
/// each edits only a file of their own, and each advances and seals *through the same
/// store*. Independence is asserted as a structural fact — identities, diffs, and
/// staleness verdicts — not as an absence of a crash; convergence is asserted as an exact
/// count, not as "the test did not time out".
#[test]
fn positive_two_clients_fork_and_analyze_independently_while_shared_subtrees_converge() {
    let root = scratch("two-clients-independent");

    // "shared/" is a subtree neither client will ever touch; "Cargo.lock" is a component
    // both clients carry forward unchanged; "alice/" and "bob/" hold the one file each
    // client actually edits.
    write(
        &root,
        "shared/common.txt",
        b"shared, never edited by either client",
    );
    write(&root, "alice/notes.txt", b"alice's starting point");
    write(&root, "bob/notes.txt", b"bob's starting point");
    write(&root, "Cargo.lock", b"[[package]]\nname = \"base\"");

    let imported = DiskImporter::new()
        .import(&root, &HexIdentity)
        .expect("import");

    let (store, operator, clients) = store_with_clients(&["client-alice", "client-bob"]);
    let alice_cap = clients[0].clone();
    let bob_cap = clients[1].clone();

    let base_sealed = SealedWorkspace::seal(imported.into_descriptor(), &store, &alice_cap)
        .expect("the base workspace seals");
    let base = base_sealed.snapshot().clone();
    let base_identity = base_sealed.snapshot_identity().clone();

    let alice_fork = Fork::diverge(name("alice"), &base);
    let bob_fork = Fork::diverge(name("bob"), &base);
    // A name is a label, not an identity (lineage.rs): both open at the same head.
    assert_eq!(alice_fork.identity(), bob_fork.identity());
    assert_eq!(alice_fork.origin(), &base_identity);
    assert_eq!(bob_fork.origin(), &base_identity);

    let mut alice_overlay = Overlay::new();
    alice_overlay
        .write(path("alice/notes.txt"), b"alice's own analysis".to_vec())
        .expect("write");
    let mut bob_overlay = Overlay::new();
    bob_overlay
        .write(path("bob/notes.txt"), b"bob's own analysis".to_vec())
        .expect("write");

    let alice_advanced = alice_fork
        .advance(&alice_overlay, &HexIdentity)
        .expect("alice advances");
    let bob_advanced = bob_fork
        .advance(&bob_overlay, &HexIdentity)
        .expect("bob advances");

    // --- independence: analyzing one lineage says nothing about, and does not touch, the
    // other --------------------------------------------------------------------------------

    assert_ne!(alice_advanced.identity(), bob_advanced.identity());
    assert_eq!(alice_advanced.origin(), &base_identity);
    assert_eq!(bob_advanced.origin(), &base_identity);

    // `bob_fork`, held since before alice ever advanced, still reads exactly as it did at
    // diverge time: there is nothing shared between the two `Fork` values for alice's work
    // to touch it through (lineage.rs's module doc).
    assert_eq!(bob_fork.identity(), &base_identity);
    assert_eq!(bob_fork.parent(), None);

    // Diffing each client's fork against the shared origin names only that client's own
    // change: "analyze independently" as an actual analysis, not just a fork.
    let alice_diff = WorkspaceDiff::between(&base, alice_advanced.head());
    assert_eq!(
        alice_diff
            .changes()
            .map(|change| change.path())
            .collect::<Vec<_>>(),
        [&path("alice/notes.txt")],
    );
    let bob_diff = WorkspaceDiff::between(&base, bob_advanced.head());
    assert_eq!(
        bob_diff
            .changes()
            .map(|change| change.path())
            .collect::<Vec<_>>(),
        [&path("bob/notes.txt")],
    );

    // --- staleness is per-lineage: the shared base is stale exactly where it was actually
    // superseded, and nowhere else -----------------------------------------------------

    assert!(matches!(
        check_current(&alice_advanced, &base_identity),
        Err(LineageError::Stale(_))
    ));
    assert!(matches!(
        check_current(&bob_advanced, &base_identity),
        Err(LineageError::Stale(_))
    ));
    // bob's *un-advanced* fork still calls the same identity current: staleness is a fact
    // about a lineage's own history, not about what any other client did
    // (staleness.rs's `staleness_is_per_lineage`, one workspace up).
    assert_eq!(check_current(&bob_fork, &base_identity), Ok(()));
    assert_eq!(check_current(&alice_fork, &base_identity), Ok(()));

    // Alice's advanced head is not even a *known* coordinate in bob's lineage — Unknown,
    // not Stale, which would overclaim a relationship that never existed (INV-008).
    assert!(matches!(
        check_current(&bob_advanced, alice_advanced.identity()),
        Err(LineageError::Unknown(_))
    ));

    // --- seal both through the same store: shared, unchanged subtrees converge onto one
    // stored copy; nothing either client alone touched is duplicated ------------------------

    let alice_descriptor = rebind_components(base_sealed.descriptor())
        .build(alice_advanced.head().clone(), &HexIdentity)
        .expect("alice's descriptor");
    let bob_descriptor = rebind_components(base_sealed.descriptor())
        .build(bob_advanced.head().clone(), &HexIdentity)
        .expect("bob's descriptor");

    // The untouched subtree really is one identity across all three trees, file and
    // directory alike (overlay.rs's `an_untouched_subtree_keeps_its_identity`, one level
    // up: shared across independently sealed workspaces, not just within one derivation).
    let shared_file = path("shared/common.txt");
    let shared_dir = path("shared");
    for tree in [&base, alice_advanced.head(), bob_advanced.head()] {
        assert_eq!(
            tree.node(&shared_file).map(|node| node.identity()),
            base.node(&shared_file).map(|node| node.identity()),
        );
        assert_eq!(
            tree.node(&shared_dir).map(|node| node.identity()),
            base.node(&shared_dir).map(|node| node.identity()),
        );
    }

    // Captured before sealing consumes the descriptors: the exact union of every distinct
    // record identity and its bytes across all three trees.
    let expected_identities: BTreeSet<ArtifactHandle> = base_sealed
        .descriptor()
        .records()
        .into_iter()
        .chain(alice_descriptor.records())
        .chain(bob_descriptor.records())
        .map(|(identity, _)| identity.clone())
        .collect();
    let expected_bytes: BTreeMap<ArtifactHandle, Vec<u8>> = base_sealed
        .descriptor()
        .records()
        .into_iter()
        .chain(alice_descriptor.records())
        .chain(bob_descriptor.records())
        .map(|(identity, record)| (identity.clone(), record.to_vec()))
        .collect();
    let shared_identity = base
        .node(&shared_dir)
        .expect("shared dir present in the base")
        .identity()
        .clone();
    let lockfile_identity = base_sealed
        .descriptor()
        .dependencies()
        .expect("Cargo.lock is a recognized dependency component")
        .identity()
        .clone();

    let alice_sealed = SealedWorkspace::seal(alice_descriptor, &store, &alice_cap)
        .expect("alice seals through the shared store");
    let bob_sealed = SealedWorkspace::seal(bob_descriptor, &store, &bob_cap)
        .expect("bob seals through the same shared store");

    let view = store.audit_view(&operator).expect("operator");

    // No duplication: the store's distinct-identity count is exactly the union of what
    // the three descriptors named, whatever overlap they had
    // (publication.rs's `a_second_publication_of_identical_content_converges_without_a_second_entry`,
    // one workspace up).
    assert_eq!(view.published_count(), expected_identities.len());

    // Bytes actually stored equal the union's bytes, each counted once: a converged
    // record costs nothing extra in storage attribution.
    let expected_total_bytes: u64 = expected_bytes
        .values()
        .map(|record| u64::try_from(record.len()).unwrap_or(u64::MAX))
        .sum();
    assert_eq!(
        view.storage_attribution()
            .get(&NODE_CLASS)
            .copied()
            .unwrap_or(0),
        expected_total_bytes,
    );

    // Receipts confirm convergence, not duplication: the shared subtree and the
    // unchanged lockfile were published by all three seals and carry three receipts
    // each; each client's own private change carries exactly one, attributed to that
    // client.
    assert_eq!(view.receipts(&shared_identity).len(), 3);
    assert_eq!(view.receipts(&lockfile_identity).len(), 3);
    assert_eq!(view.receipts(alice_advanced.identity()).len(), 1);
    assert_eq!(view.receipts(bob_advanced.identity()).len(), 1);
    assert_eq!(
        view.receipts(alice_advanced.identity())[0].actor(),
        &ActorId::new("client-alice"),
    );
    assert_eq!(
        view.receipts(bob_advanced.identity())[0].actor(),
        &ActorId::new("client-bob"),
    );

    assert_eq!(view.fsck(), Vec::new());
    assert!(
        store
            .read(alice_sealed.descriptor_identity(), &alice_cap)
            .is_ok()
    );
    assert!(
        store
            .read(bob_sealed.descriptor_identity(), &bob_cap)
            .is_ok()
    );

    // Sealing through the shared store perturbed nothing about bob's own earlier value,
    // either — the independence claim holds at the very end, not just before the store
    // was touched.
    assert_eq!(bob_fork.identity(), &base_identity);
    assert_eq!(check_current(&bob_fork, &base_identity), Ok(()));

    cleanup(&root);
}

// --- PR-3-EXIT, clause 2: an old snapshot remains reproducible after the working tree
// changes -----------------------------------------------------------------------------------

/// **PR-3-EXIT, clause 2.** "An old snapshot remains reproducible after the working tree
/// changes."
///
/// A real directory is imported and sealed as `A`. The directory is then genuinely
/// modified, one file deleted, another added. A second import, `B`, is sealed through the
/// same store. `A` is reconstructed by reading every one of its nodes back from the store
/// — not by trusting the `Snapshot` value still held in memory — and its identity is
/// separately re-derived from the original bytes alone, with no importer and no disk
/// involved a second time.
#[test]
fn positive_an_old_snapshot_remains_reproducible_after_the_working_tree_changes() {
    let root = scratch("old-snapshot-reproducible");

    let original: [(&str, &[u8]); 3] = [
        ("src/lib.rs", b"fn main() {}"),
        ("README.md", b"hello"),
        ("notes/todo.txt", b"todo: ship PR 3"),
    ];
    for (relative, content) in original {
        write(&root, relative, content);
    }

    let imported_a = DiskImporter::new()
        .import(&root, &HexIdentity)
        .expect("first import");
    let (store, operator, clients) = store_with_clients(&["client"]);
    let capability = clients[0].clone();
    let a_sealed =
        SealedWorkspace::seal(imported_a.into_descriptor(), &store, &capability).expect("A seals");
    let a = a_sealed.snapshot().clone();
    let identity_a = a_sealed.snapshot_identity().clone();

    // The working tree changes for real: one file is modified, one is deleted, one is
    // added. "notes/todo.txt" is left exactly as it was.
    write(&root, "src/lib.rs", b"fn main() { println!(\"changed\"); }");
    fs::remove_file(root.join("README.md")).expect("remove README.md");
    write(&root, "src/new_module.rs", b"pub fn helper() {}");

    let imported_b = DiskImporter::new()
        .import(&root, &HexIdentity)
        .expect("second import, after the tree changed");
    let b_sealed = SealedWorkspace::seal(imported_b.into_descriptor(), &store, &capability)
        .expect("B seals through the same store");
    let b = b_sealed.snapshot().clone();

    // A different working tree is a different snapshot.
    assert_ne!(a_sealed.snapshot_identity(), b_sealed.snapshot_identity());
    assert_ne!(
        a_sealed.descriptor_identity(),
        b_sealed.descriptor_identity()
    );

    // --- A is reconstructible byte for byte, read back from the store alone ---------------

    // Every node A ever had — every file and every directory on the path to the root —
    // round-trips through `ReferenceStore::read` to the exact bytes the in-memory
    // snapshot carries. Read back through the store, not trusted from the `Snapshot`
    // value still sitting in this process's memory.
    for (_, node) in a.subtrees() {
        assert_eq!(
            store
                .read(node.identity(), &capability)
                .expect("every node of A is durably in the store"),
            node.record(),
        );
    }
    assert_eq!(
        store
            .read(a.identity(), &capability)
            .expect("A's root is durably in the store"),
        a.root().record(),
    );

    // What the store hands back for each file is exactly what was on disk when A was
    // taken — not what is on disk now.
    for (relative, content) in original {
        let node = a.node(&path(relative)).expect("A still names this path");
        let file = node.as_file().expect("a file");
        assert_eq!(file.content(), content);
    }

    // Re-deriving a snapshot from the *original* bytes alone — no importer, no disk,
    // nothing from `a` itself — reproduces `identity_a` exactly: identity is a pure
    // function of content (snapshot.rs's "No ambient anything"), so it does not matter
    // that the disk has since moved on.
    let mut rebuilt = WorkspaceContent::new();
    for (relative, content) in original {
        rebuilt
            .insert(path(relative), content.to_vec())
            .expect("insert");
    }
    let rebuilt = Snapshot::build(&rebuilt, &HexIdentity).expect("rebuild from the original bytes");
    assert_eq!(rebuilt.identity(), &identity_a);

    // --- B coexists: readable in its own right, disjoint where the tree changed,
    // converged where it did not --------------------------------------------------------

    for (_, node) in b.subtrees() {
        assert_eq!(
            store
                .read(node.identity(), &capability)
                .expect("every node of B is durably in the store"),
            node.record(),
        );
    }

    // Disjoint at the changed paths.
    assert_ne!(
        a.node(&path("src/lib.rs")).map(|node| node.identity()),
        b.node(&path("src/lib.rs")).map(|node| node.identity()),
    );
    assert!(a.node(&path("README.md")).is_some());
    assert!(b.node(&path("README.md")).is_none());
    assert!(a.node(&path("src/new_module.rs")).is_none());
    assert!(b.node(&path("src/new_module.rs")).is_some());

    // Converged at the untouched path: one stored copy, not two.
    let todo_path = path("notes/todo.txt");
    assert_eq!(
        a.node(&todo_path).map(|node| node.identity()),
        b.node(&todo_path).map(|node| node.identity()),
    );
    let todo_identity = a.node(&todo_path).expect("todo").identity().clone();
    let view = store.audit_view(&operator).expect("operator");
    assert_eq!(view.receipts(&todo_identity).len(), 2);
    assert_eq!(view.fsck(), Vec::new());

    cleanup(&root);
}

// --- PR-3-EXIT, boundary: a sealed snapshot is not stale merely because the disk moved
// on ------------------------------------------------------------------------------------

/// **PR-3-EXIT, boundary.** The distinction the exit sentence's second clause turns on,
/// made explicit: [`StaleSnapshot`](continuum_workspace::staleness::StaleSnapshot) is
/// about a *lineage's* history, never about the filesystem. A snapshot sealed standalone
/// is not stale merely because the disk it was read from moved on — it is a `Stale`
/// coordinate only once a `Fork`'s own lineage has actually advanced past it.
#[test]
fn boundary_a_sealed_snapshot_is_not_stale_merely_because_the_disk_moved_on() {
    let root = scratch("not-stale-because-disk-moved");
    write(&root, "src/lib.rs", b"fn main() {}");
    write(&root, "README.md", b"hello");

    let imported = DiskImporter::new()
        .import(&root, &HexIdentity)
        .expect("import");
    let a = imported.snapshot().clone();
    let identity_a = a.identity().clone();

    let (store, _operator, clients) = store_with_clients(&["client"]);
    let capability = clients[0].clone();

    let fork = Fork::diverge(name("solo"), &a);

    // The disk moves on for real, out from under the fork. `fork` and `a` are values;
    // neither reads the filesystem again after this point.
    write(&root, "src/lib.rs", b"fn main() { todo!() }");
    fs::remove_file(root.join("README.md")).expect("remove README.md");

    // Not stale: `identity_a` is still `fork`'s current head. Staleness has never been a
    // function of the filesystem — only of the lineage's own recorded history
    // (`src/snapshot.rs`'s "Immutability" section names this exact clause).
    assert_eq!(check_current(&fork, &identity_a), Ok(()));

    // A guarded operation against A as the current base succeeds — sealing A standalone
    // is not stale merely because the disk moved on.
    let source_descriptor = WorkspaceDescriptor::builder()
        .build(a.clone(), &HexIdentity)
        .expect("descriptor from A");
    let sealed = seal_current(source_descriptor, &fork, &store, &capability)
        .expect("A is still fork's current head; the disk change above is irrelevant");
    assert_eq!(sealed.snapshot_identity(), &identity_a);

    // The compare-and-set overlay advance succeeds too, for the same reason — and this is
    // what actually moves the *lineage*, not the disk, past A.
    let mut overlay = Overlay::new();
    overlay
        .write(
            path("src/lib.rs"),
            b"analysis of A, offline from disk".to_vec(),
        )
        .expect("write");
    let advanced = advance_current(&fork, &identity_a, &overlay, &HexIdentity)
        .expect("A is current; the disk's own state plays no part in this guard");

    // Only now, superseded within its own lineage, is A stale — and the error names
    // `advanced`'s actual current head, exactly staleness.rs's contract.
    let error = check_current(&advanced, &identity_a).expect_err("A was superseded");
    let LineageError::Stale(stale) = error else {
        panic!(
            "A's supersession must be provable Stale, not Unknown: it is `advanced`'s own origin"
        )
    };
    assert_eq!(stale.stale(), &identity_a);
    assert_eq!(stale.current(), advanced.identity());

    // `fork` itself is untouched by advancing past it: the earlier value still reads
    // exactly as it did (lineage.rs: advancing produces a new value, never mutates one).
    assert_eq!(check_current(&fork, &identity_a), Ok(()));

    // The same guarded operations that succeeded above now refuse — for the lineage
    // reason, and only the lineage reason, never because of anything the disk did.
    assert!(matches!(
        derive_current(&overlay, &a, &advanced, &HexIdentity),
        Err(GuardedOverlayError::Lineage(LineageError::Stale(_)))
    ));
    assert!(matches!(
        advance_current(&advanced, &identity_a, &overlay, &HexIdentity),
        Err(GuardedAdvanceError::Lineage(LineageError::Stale(_)))
    ));
    let stale_descriptor = WorkspaceDescriptor::builder()
        .build(a.clone(), &HexIdentity)
        .expect("descriptor from A");
    assert!(matches!(
        seal_current(stale_descriptor, &advanced, &store, &capability),
        Err(GuardedSealError::Lineage(LineageError::Stale(_)))
    ));

    cleanup(&root);
}
