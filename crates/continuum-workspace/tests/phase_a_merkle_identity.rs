//! **Merkle identity** — the property suite for the workspace snapshot's content
//! addressing (`bn-221j`).
//!
//! # The law
//!
//! > Snapshots form a Merkle DAG. A tool call never means "whatever is currently on disk".
//! >
//! > — `notes/plan/plan.md` §4.2, quoted by `crates/continuum-workspace/src/snapshot.rs`
//!
//! > Canonical structural encodings define identity. Hashes index and partition;
//! > collisions resolve by exact comparison.
//! >
//! > — `notes/plan/adr/0013-exact-state-identity.md`
//!
//! Content addressing is a *biconditional*, and a suite that checks one direction has
//! checked the easy one:
//!
//! - **same content → same name.** [`Snapshot::build`] is a pure function of the content and
//!   the seam, so two builds of one workspace — in either insertion order, in either
//!   process — produce one root. This is the direction a cache depends on.
//! - **different content → different name.** Two workspaces that differ anywhere differ at
//!   the root. This is the direction *assurance* depends on, and it is the direction a
//!   truncated or field-forgetting digest silently breaks.
//!
//! and the property that makes it a *Merkle* identity rather than a flat digest:
//!
//! - **locality.** Editing one file changes exactly the identities on that file's path to
//!   the root and no others. That is the structural-sharing claim in observable form, and
//!   it is what makes an incremental store possible at all.
//!
//! # The identity seam, and why it is `HexIdentity` and not BLAKE3
//!
//! This suite says "two workspaces have two identities" as a *fact*, not as a probability.
//! Under [`HexIdentity`] a node's name is its own record bytes, so injectivity is
//! structural and a counterexample means the *tree construction* lost information — which
//! is the thing under test. Under a cryptographic hash the same test would be a statement
//! about BLAKE3, which ADR-0013 explicitly declines to depend on ("a system promising
//! stronger assurance should not depend on collision improbability"). The same reasoning,
//! and the same seam, is already in `tests/deterministic_ordering.rs` and
//! `tests/merkle_snapshot.rs`.
//!
//! [`Fnv1aTestIdentity`] — **not** a release hash — appears only where an identity has to
//! stay fixed-width: at the depth and width boundaries, where a `HexIdentity` would grow
//! with the subtree it names.
//!
//! # Evidence map
//!
//! | Claim | Test |
//! |---|---|
//! | building is a pure function: same content, same root, same records | [`positive_building_a_snapshot_is_a_pure_function_of_its_content`] |
//! | the root does not depend on the order files were inserted | [`positive_the_root_does_not_depend_on_insertion_order`] |
//! | different content is a different root, adjacent cases included | [`positive_different_content_is_a_different_root`] |
//! | editing one file moves exactly the identities on its path | [`positive_editing_one_file_moves_exactly_its_path_to_the_root`] |
//! | a tree encoding round trips and re-derives every identity | [`positive_a_tree_encoding_round_trips_and_re_derives_its_identities`] |
//! | equal subtrees in different workspaces share one identity | [`positive_equal_subtrees_share_one_identity_across_workspaces`] |
//! | the declared depth, width, and segment-length limits hold | [`boundary_the_declared_depth_width_and_segment_limits_hold`] |
//! | **the suite can fail**: a digest over a truncated preimage | [`falsification_a_digest_that_forgets_the_last_byte_is_caught`] |
//! | the shrunk counterexample is retained and replays on its own | [`falsification_the_retained_counterexample_replays_deterministically`] |
//!
//! # Determinism
//!
//! Every corpus is a function of a seed written into a [`Plan`] here (INV-005). The
//! boundary cases are built by counting to the bound — one segment, one child, one byte at
//! a time — never by doubling.

#[path = "../../continuum-value/tests/support/property.rs"]
mod property;

use std::collections::BTreeSet;

use continuum_workspace::artifact_path::{ArtifactClass, ArtifactHandle};
use continuum_workspace::publication::{ContentIdentifier, IdentityUnavailable};
use continuum_workspace::snapshot::{
    MAX_DEPTH, MAX_SEGMENT_BYTES, Snapshot, SnapshotNode, WorkspaceContent, WorkspacePath,
};

use property::{Case, Domain, NearPairs, Pair, Plan, Prng, Sexp, check, expect_replay_refutes};

/// Cases per law.
const CASES: usize = 256;

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

/// FNV-1a offset basis, 64-bit.
const FNV_OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
/// FNV-1a prime, 64-bit.
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

/// **Not a release hash.** A fixed-width, deliberately non-cryptographic stand-in, used
/// only where an identity has to stay short — a 64-segment path or a 255-byte name would
/// otherwise carry a [`HexIdentity`] that grows with the subtree. Mirrors the seam
/// `tests/deterministic_ordering.rs` and `tests/merkle_snapshot.rs` use, for the same
/// reason.
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

// --- the case ------------------------------------------------------------------------------

/// A workspace: file paths and their bytes, normalized to a description that *is* a tree.
#[derive(Debug, Clone, PartialEq, Eq)]
struct TreeCase {
    files: Vec<(String, Vec<u8>)>,
}

impl TreeCase {
    /// Keep exactly the files a [`WorkspaceContent`] accepts, in path order.
    ///
    /// Normalization rather than rejection, so that every generated *and* every shrunk case
    /// is a legal workspace by construction: a path that is not admissible, a repeated
    /// path, and a path that another path needs as a directory are all dropped. The suite
    /// is about identity, and "is this description a tree" is `WorkspaceContent`'s own
    /// question, answered exhaustively by `tests/deterministic_ordering.rs`.
    fn normalized(files: Vec<(String, Vec<u8>)>) -> Self {
        let mut sorted = files;
        sorted.sort();
        let mut content = WorkspaceContent::new();
        let mut kept = Vec::with_capacity(sorted.len());
        for (path, bytes) in sorted {
            let Ok(parsed) = WorkspacePath::new(&path) else {
                continue;
            };
            if content.insert(parsed, bytes.clone()).is_ok() {
                kept.push((path, bytes));
            }
        }
        Self { files: kept }
    }

    /// This workspace's content.
    fn content(&self) -> WorkspaceContent {
        let mut content = WorkspaceContent::new();
        for (path, bytes) in &self.files {
            let path = WorkspacePath::new(path).expect("a normalized case holds legal paths");
            content
                .insert(path, bytes.clone())
                .expect("a normalized case is a tree");
        }
        content
    }

    /// This workspace's snapshot under `identifier`.
    fn snapshot<I: ContentIdentifier>(&self, identifier: &I) -> Snapshot {
        Snapshot::build(&self.content(), identifier).expect("an injective seam names every node")
    }
}

impl Case for TreeCase {
    fn to_sexp(&self) -> Sexp {
        Sexp::list(
            self.files
                .iter()
                .map(|(path, bytes)| Sexp::list([Sexp::text(path), Sexp::atom(bytes.clone())])),
        )
    }

    fn from_sexp(sexp: &Sexp) -> Option<Self> {
        let mut files = Vec::new();
        for item in sexp.as_list()? {
            let [path, bytes] = item.as_tuple::<2>()?;
            files.push((path.as_text()?.to_owned(), bytes.as_atom()?.to_vec()));
        }
        Some(Self::normalized(files))
    }
}

// --- the domain ------------------------------------------------------------------------------

/// Path segments: a shortlex pair (`z` before `aa`), a shared directory name so subtree
/// sharing is reachable, and a name that is a prefix of another.
const SEGMENTS: [&str; 5] = ["a", "b", "z", "aa", "dir"];

/// File bytes: the empty file, two one-byte files that differ, and two two-byte files that
/// differ only in their *last* byte — which is what a truncated preimage cannot see.
const BLOBS: [&[u8]; 6] = [b"", b"\x00", b"\x01", b"\x00\x00", b"\x00\x01", b"ab"];

/// Generated workspaces.
#[derive(Debug, Clone, Copy)]
struct Trees {
    max_files: usize,
    max_depth: usize,
}

impl Trees {
    const fn small() -> Self {
        Self {
            max_files: 4,
            max_depth: 3,
        }
    }
}

impl Domain for Trees {
    type Item = TreeCase;

    fn generate(&self, rng: &mut Prng) -> TreeCase {
        let count = rng.between(0, self.max_files);
        let mut files = Vec::with_capacity(count);
        for _ in 0..count {
            let depth = rng.between(1, self.max_depth);
            let mut segments = Vec::with_capacity(depth);
            for _ in 0..depth {
                segments.push(*rng.pick(&SEGMENTS));
            }
            files.push((segments.join("/"), rng.pick(&BLOBS).to_vec()));
        }
        TreeCase::normalized(files)
    }

    fn shrink(&self, item: &TreeCase) -> Vec<TreeCase> {
        let mut out = Vec::new();
        if !item.files.is_empty() {
            out.push(TreeCase { files: Vec::new() });
        }
        // Drop one file.
        for index in 0..item.files.len() {
            let mut files = item.files.clone();
            files.remove(index);
            out.push(TreeCase::normalized(files));
        }
        // Shorten one file's content.
        for index in 0..item.files.len() {
            if item.files[index].1.is_empty() {
                continue;
            }
            let mut files = item.files.clone();
            let bytes = &mut files[index].1;
            bytes.pop();
            out.push(TreeCase::normalized(files));
        }
        // Zero one file's last content byte. This one does not make the case *smaller*, so
        // the shrinker will never accept it as a step — it is here for `NearPairs`, which
        // draws its neighbours from this list. Two files of equal length differing in one
        // byte is precisely the pair a digest over a truncated preimage cannot tell apart,
        // and no reduction that changes a *length* can produce it.
        for index in 0..item.files.len() {
            let Some(last) = item.files[index].1.last().copied() else {
                continue;
            };
            if last == 0 {
                continue;
            }
            let mut files = item.files.clone();
            let bytes = &mut files[index].1;
            let at = bytes.len() - 1;
            bytes[at] = 0;
            out.push(TreeCase::normalized(files));
        }
        // Flatten one file's path to its last segment.
        for index in 0..item.files.len() {
            let path = &item.files[index].0;
            let Some(last) = path.rsplit('/').next() else {
                continue;
            };
            if last == path {
                continue;
            }
            let mut files = item.files.clone();
            files[index].0 = last.to_owned();
            out.push(TreeCase::normalized(files));
        }
        out
    }

    fn size(&self, item: &TreeCase) -> usize {
        item.files
            .iter()
            .map(|(path, bytes)| path.len() + bytes.len() + 1)
            .sum()
    }
}

// --- the laws --------------------------------------------------------------------------------

fn building_is_pure(case: &TreeCase) -> Result<(), String> {
    let first = case.snapshot(&HexIdentity);
    let second = case.snapshot(&HexIdentity);
    if first.identity() != second.identity() {
        return Err("two builds of one workspace produced two roots".to_owned());
    }
    if first != second {
        return Err("two builds of one workspace produced two trees".to_owned());
    }
    if first.encode() != second.encode() {
        return Err("two builds of one workspace produced two encodings".to_owned());
    }
    let left: Vec<(String, Vec<u8>)> = first
        .records()
        .into_iter()
        .map(|(handle, record)| (handle.to_string(), record.to_vec()))
        .collect();
    let right: Vec<(String, Vec<u8>)> = second
        .records()
        .into_iter()
        .map(|(handle, record)| (handle.to_string(), record.to_vec()))
        .collect();
    if left != right {
        return Err("two builds of one workspace produced two publication listings".to_owned());
    }
    Ok(())
}

fn the_root_ignores_insertion_order(case: &TreeCase) -> Result<(), String> {
    let expected = case.snapshot(&HexIdentity);
    // Every rotation, plus the reverse: linear in the file count, deterministic, and
    // enough to separate "the order does not matter" from "sorted input happens to work".
    let count = case.files.len();
    for rotation in 0..count {
        let mut files = case.files.clone();
        files.rotate_left(rotation);
        let mut reversed = files.clone();
        reversed.reverse();
        for order in [files, reversed] {
            let mut content = WorkspaceContent::new();
            for (path, bytes) in order {
                let path = WorkspacePath::new(&path).expect("a normalized case holds legal paths");
                content
                    .insert(path, bytes)
                    .expect("a normalized case is a tree");
            }
            let built = Snapshot::build(&content, &HexIdentity).expect("names every node");
            if built.identity() != expected.identity() {
                return Err(format!(
                    "insertion order changed the root (rotation {rotation})"
                ));
            }
        }
    }
    Ok(())
}

fn different_content_is_a_different_root(pair: &Pair<TreeCase>) -> Result<(), String> {
    root_injectivity_under(pair, &HexIdentity)
}

/// The injectivity law, over whichever identity seam is supplied.
fn root_injectivity_under<I: ContentIdentifier>(
    pair: &Pair<TreeCase>,
    identifier: &I,
) -> Result<(), String> {
    let left = pair.left.snapshot(identifier);
    let right = pair.right.snapshot(identifier);
    let same_root = left.identity() == right.identity();
    let same_workspace = pair.left.files == pair.right.files;
    match (same_root, same_workspace) {
        (true, true) | (false, false) => Ok(()),
        (true, false) => Err(
            "two different workspaces share one Merkle root: the identity is not a \
             function of the content"
                .to_owned(),
        ),
        (false, true) => Err("one workspace has two Merkle roots".to_owned()),
    }
}

/// Editing one file moves exactly the identities on its path to the root.
fn editing_one_file_moves_exactly_its_path(case: &TreeCase) -> Result<(), String> {
    let Some((edited_path, _)) = case.files.first().cloned() else {
        return Ok(());
    };
    let before = case.snapshot(&HexIdentity);

    let mut files = case.files.clone();
    files[0].1.push(0xff);
    let after = TreeCase::normalized(files).snapshot(&HexIdentity);

    if before.identity() == after.identity() {
        return Err(format!("editing {edited_path} did not change the root"));
    }

    let edited = WorkspacePath::new(&edited_path).expect("a normalized case holds legal paths");
    let before_nodes = named_identities(&before);
    let after_nodes = named_identities(&after);

    for (path, identity) in &before_nodes {
        let on_the_path = path == &edited || path.is_ancestor_of(&edited);
        match after_nodes.iter().find(|(other, _)| other == path) {
            None => return Err(format!("the node at {path} disappeared")),
            Some((_, other)) => {
                if on_the_path && identity == other {
                    return Err(format!(
                        "{path} is on the edited file's path but kept its identity"
                    ));
                }
                if !on_the_path && identity != other {
                    return Err(format!(
                        "{path} is not on the edited file's path but changed identity"
                    ));
                }
            }
        }
    }
    Ok(())
}

fn a_tree_encoding_round_trips(case: &TreeCase) -> Result<(), String> {
    let snapshot = case.snapshot(&HexIdentity);
    let bytes = snapshot.encode();
    let decoded = Snapshot::decode(&bytes, &HexIdentity)
        .map_err(|error| format!("a snapshot's own encoding did not decode: {error}"))?;
    if decoded != snapshot {
        return Err("decoding an encoding produced a different tree".to_owned());
    }
    if decoded.identity() != snapshot.identity() {
        return Err("the re-derived root disagrees with the built one".to_owned());
    }
    if decoded.encode() != bytes {
        return Err("re-encoding a decoded tree changed the bytes".to_owned());
    }
    Ok(())
}

/// Every non-root node's path and identity.
fn named_identities(snapshot: &Snapshot) -> Vec<(WorkspacePath, ArtifactHandle)> {
    snapshot
        .subtrees()
        .into_iter()
        .map(|(path, node)| (path, node.identity().clone()))
        .collect()
}

// --- the suite --------------------------------------------------------------------------------

#[test]
fn positive_building_a_snapshot_is_a_pure_function_of_its_content() {
    let plan = Plan::new("build is pure", 0x2021_0221_0003_0001, CASES);
    check(&plan, &Trees::small(), building_is_pure).expect_held(&plan);
}

#[test]
fn positive_the_root_does_not_depend_on_insertion_order() {
    let plan = Plan::new("order independence", 0x2021_0221_0003_0002, CASES);
    check(&plan, &Trees::small(), the_root_ignores_insertion_order).expect_held(&plan);
}

#[test]
fn positive_different_content_is_a_different_root() {
    // Adjacent workspaces first — one file dropped, one byte removed, one path flattened —
    // because that is where a lossy identity hides. Then independent pairs.
    let plan = Plan::new("root injectivity, adjacent", 0x2021_0221_0003_0003, CASES);
    check(
        &plan,
        &NearPairs(Trees::small()),
        different_content_is_a_different_root,
    )
    .expect_held(&plan);

    let plan = Plan::new(
        "root injectivity, independent",
        0x2021_0221_0003_0013,
        CASES,
    );
    check(
        &plan,
        &property::Pairs(Trees::small()),
        different_content_is_a_different_root,
    )
    .expect_held(&plan);
}

#[test]
fn positive_editing_one_file_moves_exactly_its_path_to_the_root() {
    let plan = Plan::new("Merkle locality", 0x2021_0221_0003_0004, CASES);
    check(
        &plan,
        &Trees::small(),
        editing_one_file_moves_exactly_its_path,
    )
    .expect_held(&plan);
}

#[test]
fn positive_a_tree_encoding_round_trips_and_re_derives_its_identities() {
    let plan = Plan::new("tree encoding round trip", 0x2021_0221_0003_0005, CASES);
    check(&plan, &Trees::small(), a_tree_encoding_round_trips).expect_held(&plan);
}

#[test]
fn positive_equal_subtrees_share_one_identity_across_workspaces() {
    // Structural sharing: the same directory content under two different parents is one
    // node with one identity, in both workspaces. This is what makes the structure a DAG
    // rather than a tree of independent digests.
    let plan = Plan::new("structural sharing", 0x2021_0221_0003_0006, CASES);
    check(
        &plan,
        &NearPairs(Trees::small()),
        |pair: &Pair<TreeCase>| {
            let left = pair.left.snapshot(&HexIdentity);
            let right = pair.right.snapshot(&HexIdentity);
            let left_nodes = named_identities(&left);
            let right_nodes = named_identities(&right);
            for (path, identity) in &left_nodes {
                let Some((_, other)) = right_nodes.iter().find(|(other, _)| other == path) else {
                    continue;
                };
                let left_record = left.node(path).map(SnapshotNode::record);
                let right_record = right.node(path).map(SnapshotNode::record);
                if (left_record == right_record) != (identity == other) {
                    return Err(format!(
                        "at {path}, equal records and equal identities disagree"
                    ));
                }
            }
            Ok(())
        },
    )
    .expect_held(&plan);
}

#[test]
fn boundary_the_declared_depth_width_and_segment_limits_hold() {
    // Depth: a path with exactly `MAX_DEPTH` segments, built by counting to it.
    let mut segments = Vec::with_capacity(MAX_DEPTH);
    for index in 0..MAX_DEPTH {
        segments.push(format!("s{index}"));
    }
    let deepest = TreeCase::normalized(vec![(segments.join("/"), b"leaf".to_vec())]);
    assert_eq!(deepest.files.len(), 1, "a MAX_DEPTH path is admissible");
    let snapshot = deepest.snapshot(&Fnv1aTestIdentity);
    assert_eq!(snapshot.file_count(), 1);
    assert_eq!(snapshot.subtrees().len(), MAX_DEPTH);
    assert_eq!(
        Snapshot::decode(&snapshot.encode(), &Fnv1aTestIdentity).ok(),
        Some(snapshot.clone())
    );

    // One past the bound is refused rather than silently truncated.
    let mut past = segments.clone();
    past.push("one-too-many".to_owned());
    let too_deep = TreeCase::normalized(vec![(past.join("/"), b"leaf".to_vec())]);
    assert!(
        too_deep.files.is_empty(),
        "a path one segment past MAX_DEPTH must not be admitted"
    );

    // Segment length: exactly `MAX_SEGMENT_BYTES`, built by counting to it.
    let mut segment = String::with_capacity(MAX_SEGMENT_BYTES);
    for _ in 0..MAX_SEGMENT_BYTES {
        segment.push('n');
    }
    let longest = TreeCase::normalized(vec![(segment.clone(), b"x".to_vec())]);
    assert_eq!(
        longest.files.len(),
        1,
        "a MAX_SEGMENT_BYTES name is admissible"
    );
    assert_eq!(building_is_pure(&longest), Ok(()));

    // Width: a directory with many children, built one child at a time.
    let mut wide = Vec::with_capacity(512);
    for index in 0..512 {
        wide.push((
            format!("f{index:04}"),
            vec![u8::try_from(index % 251).unwrap_or(0)],
        ));
    }
    let wide = TreeCase::normalized(wide);
    assert_eq!(wide.files.len(), 512);
    let snapshot = wide.snapshot(&Fnv1aTestIdentity);
    assert_eq!(snapshot.file_count(), 512);
    let roots: BTreeSet<String> = [
        wide.snapshot(&Fnv1aTestIdentity).identity().to_string(),
        snapshot.identity().to_string(),
    ]
    .into_iter()
    .collect();
    assert_eq!(roots.len(), 1, "a wide directory still has one root");
}

// --- anti-vacuity: the suite can fail ------------------------------------------------------

/// A content address computed over a **truncated preimage**: the record's last byte is not
/// hashed.
///
/// The realistic shape of a broken Merkle node — a digest taken over a prefix, a field
/// appended after the digest was written, a length read one short. It is still a pure
/// function of (most of) the content, so every determinism law in this file passes under
/// it; only injectivity can see the loss. A file record ends with the file's own bytes, so
/// two files differing only in their last byte get one name, and the collision propagates
/// to the root.
#[derive(Debug, Clone, Copy)]
struct TruncatingIdentity;

impl ContentIdentifier for TruncatingIdentity {
    fn identify(
        &self,
        class: ArtifactClass,
        content: &[u8],
    ) -> Result<ArtifactHandle, IdentityUnavailable> {
        let seen = &content[..content.len().saturating_sub(1)];
        ArtifactHandle::new(class, &hex(seen)).map_err(|_| IdentityUnavailable)
    }
}

/// This suite's own injectivity law, over the truncating seam.
fn root_injectivity_under_a_truncating_digest(pair: &Pair<TreeCase>) -> Result<(), String> {
    root_injectivity_under(pair, &TruncatingIdentity)
}

/// Two workspaces each holding one file at the path `z`, whose two-byte contents differ
/// only in the last byte — `00 01` against `00 00`.
///
/// The smallest witness the truncating digest cannot see, and the shape is the argument: no
/// reduction that changes a *length* produces it, because a length difference is inside the
/// preimage the broken digest still reads. Only "same length, last byte differs" is
/// invisible to it.
const RETAINED_TRUNCATING_DIGEST: &str = "(((#7a #0001)) ((#7a #0000)))";

#[test]
fn falsification_a_digest_that_forgets_the_last_byte_is_caught() {
    let plan = Plan::new(
        "root injectivity, over a digest that forgets the last byte",
        0x2021_0221_0003_0003,
        CASES,
    );
    let refuted = check(
        &plan,
        &NearPairs(Trees::small()),
        root_injectivity_under_a_truncating_digest,
    )
    .expect_refuted(&plan);

    assert_eq!(refuted.minimal_repr, RETAINED_TRUNCATING_DIGEST);
    assert!(
        refuted.reason.contains("share one Merkle root"),
        "{}",
        refuted.reason
    );
    assert_ne!(refuted.minimal.left.files, refuted.minimal.right.files);

    // The real, injective seam separates exactly this pair.
    assert_eq!(
        different_content_is_a_different_root(&refuted.minimal),
        Ok(())
    );

    // Every *determinism* law still passes under the broken seam, which is why an
    // injectivity law is not optional.
    assert_eq!(building_is_pure(&refuted.minimal.left), Ok(()));
    assert_eq!(
        the_root_ignores_insertion_order(&refuted.minimal.left),
        Ok(())
    );

    let again = check(
        &plan,
        &NearPairs(Trees::small()),
        root_injectivity_under_a_truncating_digest,
    )
    .expect_refuted(&plan);
    assert_eq!(again.minimal_repr, refuted.minimal_repr);
    assert_eq!(again.shrink_steps, refuted.shrink_steps);
}

#[test]
fn falsification_the_retained_counterexample_replays_deterministically() {
    let reason = expect_replay_refutes::<Pair<TreeCase>, _>(
        RETAINED_TRUNCATING_DIGEST,
        root_injectivity_under_a_truncating_digest,
    );
    assert!(reason.contains("share one Merkle root"), "{reason}");

    let pair = Pair::<TreeCase>::from_repr(RETAINED_TRUNCATING_DIGEST).expect("parses");
    assert_eq!(pair.repr(), RETAINED_TRUNCATING_DIGEST);
    assert_eq!(different_content_is_a_different_root(&pair), Ok(()));
}
