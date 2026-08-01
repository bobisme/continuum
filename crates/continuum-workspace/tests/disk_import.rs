//! Acceptance evidence for the disk-import half of `PR-3-IMPL-02`
//! (`notes/plan/notes/START_HERE_IMPLEMENTATION.md`, PR 3's second Implement bullet;
//! `notes/plan/notes/PLAN_REQUIREMENTS.json`, id `PR-3-IMPL-02`).
//!
//! > Clients may create a snapshot from a working tree […]
//! >
//! > — `notes/plan/plan.md` §4.2, "Workspace snapshots"
//!
//! `crates/continuum-workspace/src/import.rs` carries the unit-level statement of each
//! property over values. This file is the standalone witness, and it adds the one thing a
//! unit test cannot: **real directories on a real filesystem**, including the adversarial
//! entries — symbolic links, sockets, undecodable names, trees past the depth bound — that
//! only exist once something has actually been created on disk.
//!
//! # Evidence map
//!
//! | Claim | Test |
//! |---|---|
//! | one tree, one identity, whatever order it was written in | [`positive_two_imports_of_one_tree_agree`] |
//! | the root's recognized files become components | [`positive_the_roots_recognized_files_become_components`] |
//! | recognition is additive: the file stays in the tree | [`positive_a_recognized_file_is_still_a_file`] |
//! | a workspace without a lockfile is a different workspace | [`positive_a_missing_component_moves_the_descriptor_identity`] |
//! | two spellings of one component are refused | [`negative_two_toolchain_spellings_are_refused`] |
//! | a symbolic link refuses the import, naming it | [`negative_a_symbolic_link_is_refused`] |
//! | a socket refuses the import, naming it | [`negative_a_socket_is_refused`] |
//! | a name that is not UTF-8 is refused, typed | [`negative_a_name_that_is_not_utf8_is_refused`] |
//! | an empty directory leaves no trace, and is named | [`boundary_an_empty_directory_leaves_no_trace`] |
//! | the depth bound is a typed rejection, never a crash | [`boundary_the_depth_bound_is_typed_on_both_sides`] |
//! | the segment bound is the platform's own | [`boundary_the_segment_bound_is_the_platforms`] |
//! | names that differ only in Unicode form are two files | [`positive_unicode_distinct_names_stay_distinct`] |
//! | an empty root is a legal workspace | [`boundary_an_empty_root_is_a_workspace`] |
//! | a file is not a workspace root | [`negative_a_file_is_not_a_root`] |
//! | an unreadable directory is a typed refusal | [`negative_an_unreadable_directory_is_typed`] |
//!
//! # House rules
//!
//! - **Nothing in `src/` is touched.** Every seam used here is public API.
//! - **Real directories, no test-only crate.** Scratch trees live under
//!   [`std::env::temp_dir`] in a per-test subdirectory, are removed before each run (so a
//!   panicking run cannot poison the next one) and after each success.
//! - **Platform-specific entries are guarded, never faked.** A symbolic link, a socket, and
//!   an undecodable name are created for real under `#[cfg(unix)]`; where the platform
//!   refuses to create one, the test says so and stops rather than asserting something
//!   weaker while claiming the same thing.

use std::fs;
use std::path::{Path, PathBuf};

use continuum_workspace::artifact_path::{ArtifactClass, ArtifactHandle};
use continuum_workspace::components::ComponentKind;
use continuum_workspace::import::{
    COMPONENT_RECOGNITION_VERSION, DiskImporter, ImportError, IoOperation,
    RECOGNIZED_COMPONENT_FILES,
};
use continuum_workspace::publication::{ContentIdentifier, IdentityUnavailable};
use continuum_workspace::snapshot::{
    AdmissionError, AdmissionPolicy, MAX_DEPTH, MAX_SEGMENT_BYTES, PathError, WorkspacePath,
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

/// The record's own bytes, in hex: injective, so "two trees, one identity" is a fact rather
/// than a probability (ADR-0013's certified discipline).
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

/// **Not a release hash.** The same fixed-width stand-in
/// `crates/continuum-workspace/tests/merkle_snapshot.rs` uses, and here for the same
/// reason: a [`HexIdentity`] grows with the subtree it names, so a 64-segment path would
/// carry an identity that doubles at every level. Four domain-separated FNV-1a lanes,
/// concatenated big-endian; obviously inadequate rather than plausibly inadequate.
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

// --- scratch trees -------------------------------------------------------------------------

/// A fresh, empty directory for one test.
///
/// The name is the test's own, so two tests never share one and the path is a function of
/// the source rather than of a clock or a counter. Any residue from an earlier run is
/// removed first: a test that panicked must not change what the next run observes.
fn scratch(test: &str) -> PathBuf {
    let root = std::env::temp_dir()
        .join("continuum-workspace-disk-import")
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

/// Import a tree under the default policy.
fn import(root: &Path) -> Result<continuum_workspace::import::ImportedWorkspace, ImportError> {
    DiskImporter::new().import(root, &HexIdentity)
}

/// Every file path in an imported tree, in path order.
fn paths(imported: &continuum_workspace::import::ImportedWorkspace) -> Vec<String> {
    imported
        .snapshot()
        .files()
        .into_iter()
        .map(|(path, _)| path.to_string())
        .collect()
}

// --- determinism ------------------------------------------------------------------------------

#[test]
fn positive_two_imports_of_one_tree_agree() {
    let root = scratch("two-imports-agree");
    let forward = root.join("forward");
    let backward = root.join("backward");

    let files: [(&str, &[u8]); 5] = [
        ("README.md", b"hello"),
        ("src/lib.rs", b"fn main() {}"),
        ("src/nested/deep.rs", b"deep"),
        ("Cargo.lock", b"[[package]]"),
        ("z.txt", b"last"),
    ];
    for (name, content) in files {
        write(&forward, name, content);
    }
    for (name, content) in files.iter().rev() {
        write(&backward, name, content);
    }

    let one = import(&forward).expect("import");
    let two = import(&backward).expect("import");

    assert_eq!(one.snapshot().identity(), two.snapshot().identity());
    assert_eq!(one.descriptor().identity(), two.descriptor().identity());
    assert_eq!(paths(&one), paths(&two));
    assert_eq!(
        paths(&one),
        [
            "Cargo.lock",
            "README.md",
            "src/lib.rs",
            "src/nested/deep.rs",
            "z.txt"
        ],
    );

    // Importing the same directory twice is the same answer too.
    assert_eq!(
        import(&forward).expect("re-import").snapshot().identity(),
        one.snapshot().identity(),
    );

    cleanup(&root);
}

// --- component recognition ----------------------------------------------------------------------

#[test]
fn positive_the_roots_recognized_files_become_components() {
    let root = scratch("recognized-components");
    write(&root, "Cargo.lock", b"[[package]]\nname = \"a\"");
    write(
        &root,
        "rust-toolchain.toml",
        b"[toolchain]\nchannel = \"1.90.0\"",
    );
    write(&root, "continuum.toml", b"intent = \"in_7c2f91\"");
    write(&root, "src/lib.rs", b"fn main() {}");

    let imported = import(&root).expect("import");
    let recognized: Vec<(ComponentKind, String)> = imported
        .recognized_components()
        .iter()
        .map(|(kind, path)| (*kind, path.to_string()))
        .collect();
    assert_eq!(
        recognized,
        [
            (ComponentKind::Dependencies, "Cargo.lock".to_owned()),
            (ComponentKind::Toolchain, "rust-toolchain.toml".to_owned()),
            (ComponentKind::Configuration, "continuum.toml".to_owned()),
        ],
    );

    let descriptor = imported.descriptor();
    assert_eq!(
        descriptor
            .dependencies()
            .map(|component| component.content()),
        Some(b"[[package]]\nname = \"a\"".as_slice()),
    );
    assert_eq!(
        descriptor.toolchain().map(|component| component.content()),
        Some(b"[toolchain]\nchannel = \"1.90.0\"".as_slice()),
    );
    assert_eq!(
        descriptor
            .configuration()
            .map(|component| component.content()),
        Some(b"intent = \"in_7c2f91\"".as_slice()),
    );
    assert_eq!(
        imported.component_recognition_version(),
        COMPONENT_RECOGNITION_VERSION
    );
    assert_eq!(imported.admission_policy(), AdmissionPolicy::new());

    cleanup(&root);
}

#[test]
fn positive_a_recognized_file_is_still_a_file() {
    let root = scratch("recognition-is-additive");
    write(&root, "Cargo.lock", b"[[package]]");
    write(&root, "src/lib.rs", b"fn main() {}");

    let imported = import(&root).expect("import");
    // The lockfile is a component *and* an ordinary file: a snapshot describes the
    // directory, and the directory has a `Cargo.lock` in it.
    assert_eq!(paths(&imported), ["Cargo.lock", "src/lib.rs"]);

    // The two identities are different names for related bytes: the component record and
    // the file record are framed differently, so neither can be mistaken for the other.
    let lockfile = imported
        .snapshot()
        .node(&WorkspacePath::new("Cargo.lock").expect("path"))
        .expect("the lockfile is in the tree");
    let component = imported
        .descriptor()
        .dependencies()
        .expect("the lockfile is a component");
    assert_ne!(lockfile.identity(), component.identity());
    assert_eq!(
        lockfile.as_file().expect("a file").content(),
        component.content(),
    );

    cleanup(&root);
}

#[test]
fn positive_a_missing_component_moves_the_descriptor_identity() {
    let root = scratch("missing-component");
    let with = root.join("with");
    let without = root.join("without");
    write(&with, "src/lib.rs", b"fn main() {}");
    write(&with, "Cargo.lock", b"[[package]]");
    write(&without, "src/lib.rs", b"fn main() {}");

    let with = import(&with).expect("import");
    let without = import(&without).expect("import");

    // The source trees differ (one holds the lockfile) and so do the descriptors; the
    // component's absence is structural, not an empty buffer.
    assert!(without.recognized_components().is_empty());
    assert!(without.descriptor().dependencies().is_none());
    assert_ne!(
        with.descriptor().identity(),
        without.descriptor().identity()
    );

    cleanup(&root);
}

#[test]
fn negative_two_toolchain_spellings_are_refused() {
    let root = scratch("ambiguous-toolchain");
    write(&root, "rust-toolchain.toml", b"[toolchain]");
    write(&root, "rust-toolchain", b"1.90.0");
    write(&root, "src/lib.rs", b"fn main() {}");

    assert_eq!(
        import(&root),
        Err(ImportError::AmbiguousComponent {
            kind: ComponentKind::Toolchain,
            present: vec![
                "rust-toolchain.toml".to_owned(),
                "rust-toolchain".to_owned()
            ],
        }),
    );

    cleanup(&root);
}

#[test]
fn positive_the_recognized_table_is_closed_and_root_relative() {
    // The table is public and auditable in one glance: three optional kinds, four names,
    // every one of them a single path segment.
    let names: Vec<&str> = RECOGNIZED_COMPONENT_FILES
        .iter()
        .flat_map(|(_, names)| names.iter().copied())
        .collect();
    assert_eq!(
        names,
        [
            "Cargo.lock",
            "rust-toolchain.toml",
            "rust-toolchain",
            "continuum.toml"
        ],
    );
    for name in names {
        assert_eq!(
            WorkspacePath::new(name)
                .expect("a recognized name is a path")
                .depth(),
            1,
        );
    }
}

// --- entries that are not content ------------------------------------------------------------

#[cfg(unix)]
#[test]
fn negative_a_symbolic_link_is_refused() {
    let root = scratch("symlink-refused");
    write(&root, "src/lib.rs", b"fn main() {}");
    std::os::unix::fs::symlink("src/lib.rs", root.join("alias.rs"))
        .expect("a symbolic link is creatable");

    assert_eq!(
        import(&root),
        Err(ImportError::Admission(AdmissionError::Symlink {
            name: b"alias.rs".to_vec(),
            target: b"src/lib.rs".to_vec(),
        })),
    );

    // The other arm is named but not implemented, and selecting it is also typed — an
    // importer cannot follow a link by setting a flag.
    let following = DiskImporter::new().with_admission(
        AdmissionPolicy::new()
            .with_symlinks(continuum_workspace::snapshot::SymlinkPolicy::FollowWithCycleDetection),
    );
    assert!(matches!(
        following.import(&root, &HexIdentity),
        Err(ImportError::Admission(
            AdmissionError::UnsupportedSymlinkPolicy { .. }
        )),
    ));

    cleanup(&root);
}

#[cfg(unix)]
#[test]
fn negative_a_socket_is_refused() {
    let root = scratch("socket-refused");
    write(&root, "src/lib.rs", b"fn main() {}");
    let listener = std::os::unix::net::UnixListener::bind(root.join("daemon.sock"));
    let Ok(listener) = listener else {
        // A path too long for `sockaddr_un`, or a filesystem that refuses sockets. The
        // claim is not weakened silently: there is nothing to test here.
        cleanup(&root);
        return;
    };

    assert_eq!(
        import(&root),
        Err(ImportError::Admission(AdmissionError::Special {
            name: b"daemon.sock".to_vec(),
        })),
    );

    drop(listener);
    cleanup(&root);
}

#[cfg(unix)]
#[test]
fn negative_a_name_that_is_not_utf8_is_refused() {
    use std::ffi::OsStr;
    use std::os::unix::ffi::OsStrExt;

    let root = scratch("non-utf8-name");
    let name = OsStr::from_bytes(b"bad\xffname.rs");
    if fs::write(root.join(name), b"x").is_err() {
        // A filesystem that refuses non-UTF-8 names (some do). Nothing to observe.
        cleanup(&root);
        return;
    }

    assert_eq!(
        import(&root),
        Err(ImportError::Admission(AdmissionError::Name {
            name: b"bad\xffname.rs".to_vec(),
            error: PathError::SegmentEncoding {
                index: 0,
                offset: 3,
                byte: 0xff,
            },
        })),
    );

    cleanup(&root);
}

// --- empty directories ---------------------------------------------------------------------------

#[test]
fn boundary_an_empty_directory_leaves_no_trace() {
    let root = scratch("empty-directories");
    write(&root, "src/lib.rs", b"fn main() {}");
    fs::create_dir_all(root.join("target")).expect("directory");
    fs::create_dir_all(root.join("nested/deeper")).expect("directory");

    let imported = import(&root).expect("import");
    assert_eq!(paths(&imported), ["src/lib.rs"]);

    // Skipped, and *named*: the consequence is observable rather than merely documented.
    // A directory that leaves no trace has no workspace path, so it is named by its disk
    // path, and every level of a nested empty chain is reported.
    assert_eq!(
        imported.unrepresented_directories(),
        [
            root.join("nested"),
            root.join("nested/deeper"),
            root.join("target"),
        ],
    );

    // And the snapshot is exactly the snapshot of the tree without them.
    let without = root.join("without");
    write(&without, "src/lib.rs", b"fn main() {}");
    let without = import(&without).expect("import");
    assert_eq!(
        imported.snapshot().identity(),
        without.snapshot().identity()
    );
    assert!(without.unrepresented_directories().is_empty());

    cleanup(&root);
}

// --- adversarial trees ------------------------------------------------------------------------------

#[test]
fn boundary_the_depth_bound_is_typed_on_both_sides() {
    let root = scratch("depth-bound");

    // A fixed-width seam, because a `HexIdentity` doubles at every level of nesting and
    // this tree is 64 deep. Which seam is installed decides nothing here: the depth bound
    // is enforced before any identity is derived.
    let importer = DiskImporter::new();

    // A file at exactly the bound: `d/d/.../d/f.rs` with MAX_DEPTH segments.
    let inside = root.join("inside");
    let mut relative = String::new();
    for _ in 0..MAX_DEPTH - 1 {
        relative.push_str("d/");
    }
    relative.push_str("f.rs");
    write(&inside, &relative, b"deep");
    let imported = importer
        .import(&inside, &Fnv1aTestIdentity)
        .expect("a tree at the bound imports");
    assert_eq!(imported.snapshot().file_count(), 1);
    assert_eq!(imported.snapshot().files()[0].0.depth(), MAX_DEPTH);

    // One segment deeper: a typed rejection, and no stack overflow — the walk keeps its
    // own stack, so an adversarial tree costs time, never a crash.
    let outside = root.join("outside");
    let mut relative = String::new();
    for _ in 0..MAX_DEPTH {
        relative.push_str("d/");
    }
    relative.push_str("f.rs");
    write(&outside, &relative, b"deeper");
    assert!(matches!(
        importer.import(&outside, &Fnv1aTestIdentity),
        Err(ImportError::Admission(AdmissionError::Name {
            error: PathError::TooDeep { depth, max },
            ..
        })) if depth == MAX_DEPTH + 1 && max == MAX_DEPTH,
    ));

    cleanup(&root);
}

#[test]
fn boundary_the_segment_bound_is_the_platforms() {
    let root = scratch("segment-bound");
    let at_bound = "x".repeat(MAX_SEGMENT_BYTES);
    write(&root, &at_bound, b"fits");
    let imported = import(&root).expect("import");
    assert_eq!(paths(&imported), [at_bound]);

    // One byte more is `NAME_MAX + 1` on every filesystem in the supported matrix, which
    // is precisely why the bound is 255: the workspace this segment would name cannot be
    // written out at all. Where a filesystem does accept it, the import refuses it typed.
    let past_bound = "x".repeat(MAX_SEGMENT_BYTES + 1);
    if fs::write(root.join(&past_bound), b"too long").is_ok() {
        assert!(matches!(
            import(&root),
            Err(ImportError::Admission(AdmissionError::Name {
                error: PathError::SegmentTooLong { bytes, max, .. },
                ..
            })) if bytes == MAX_SEGMENT_BYTES + 1 && max == MAX_SEGMENT_BYTES,
        ));
    }

    cleanup(&root);
}

#[test]
fn positive_unicode_distinct_names_stay_distinct() {
    let root = scratch("unicode-distinct");
    // `café` in NFC and in NFD: two names an operating system distinguishes, and therefore
    // two files. No normalization happens anywhere between the disk and the Merkle record.
    let composed = "caf\u{e9}.rs";
    let decomposed = "cafe\u{301}.rs";
    write(&root, composed, b"composed");
    write(&root, decomposed, b"decomposed");

    let entries = fs::read_dir(&root).expect("read_dir").count();
    if entries != 2 {
        // A normalizing filesystem (some APFS configurations) merged them before this
        // crate saw anything. Nothing about the importer is observable here.
        cleanup(&root);
        return;
    }

    let imported = import(&root).expect("import");
    let mut found = paths(&imported);
    found.sort();
    let mut expected = vec![composed.to_owned(), decomposed.to_owned()];
    expected.sort();
    assert_eq!(found, expected);
    assert_eq!(imported.snapshot().file_count(), 2);

    cleanup(&root);
}

// --- roots --------------------------------------------------------------------------------------------

#[test]
fn boundary_an_empty_root_is_a_workspace() {
    let root = scratch("empty-root");
    let imported = import(&root).expect("an empty directory is a workspace");
    assert!(imported.snapshot().is_empty());
    assert!(imported.recognized_components().is_empty());
    assert!(imported.unrepresented_directories().is_empty());
    // It has a name, and it is not the name of any file.
    assert!(
        imported
            .snapshot()
            .identity()
            .to_string()
            .starts_with("ws_")
    );
    cleanup(&root);
}

#[test]
fn negative_a_file_is_not_a_root() {
    let root = scratch("file-root");
    write(&root, "lonely.rs", b"fn main() {}");
    let file = root.join("lonely.rs");
    assert_eq!(
        import(&file),
        Err(ImportError::RootIsNotADirectory { path: file.clone() }),
    );

    // An absent root is a filesystem refusal, named and typed.
    let absent = root.join("nowhere");
    assert_eq!(
        import(&absent),
        Err(ImportError::Io {
            path: absent,
            operation: IoOperation::ReadMetadata,
            kind: std::io::ErrorKind::NotFound,
        }),
    );

    cleanup(&root);
}

#[cfg(unix)]
#[test]
fn negative_an_unreadable_directory_is_typed() {
    use std::os::unix::fs::PermissionsExt;

    let root = scratch("unreadable-directory");
    write(&root, "src/lib.rs", b"fn main() {}");
    fs::create_dir_all(root.join("locked")).expect("directory");
    write(&root, "locked/secret.rs", b"secret");
    if fs::set_permissions(root.join("locked"), fs::Permissions::from_mode(0o000)).is_err() {
        cleanup(&root);
        return;
    }

    let result = import(&root);
    // Restore before asserting, so a failure does not leave an unremovable tree behind.
    let _ = fs::set_permissions(root.join("locked"), fs::Permissions::from_mode(0o755));

    match result {
        Err(ImportError::Io {
            path,
            operation: IoOperation::ReadDirectory,
            kind,
        }) => {
            assert_eq!(path, root.join("locked"));
            assert_eq!(kind, std::io::ErrorKind::PermissionDenied);
        }
        // Running as a user who can read anything (root in a container): the directory was
        // readable after all, and there is nothing to observe.
        Ok(imported) => assert_eq!(paths(&imported), ["locked/secret.rs", "src/lib.rs"]),
        other => panic!("expected a typed I/O refusal, got {other:?}"),
    }

    cleanup(&root);
}
