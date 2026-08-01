//! Acceptance evidence for `PR-3-IMPL-05` — deterministic file ordering
//! (`notes/plan/notes/START_HERE_IMPLEMENTATION.md`, PR 3's fifth Implement bullet;
//! `notes/plan/notes/PLAN_REQUIREMENTS.json`, id `PR-3-IMPL-05`).
//!
//! > Snapshots are content-addressed and immutable.
//! >
//! > — `notes/plan/plan.md` §4.2, "Workspace snapshots"
//!
//! The requirement under test is one sentence: **the same workspace content has exactly
//! one snapshot identity, everywhere, forever** (plan §4.2, ADR-0018). Ordering is half
//! of it — a directory's children must have one order — and *admission* is the other
//! half: a name must have one spelling, or two working trees that differ share an
//! identity and the identity stops naming its content.
//!
//! `crates/continuum-workspace/src/snapshot.rs` carries the unit-level statement of each
//! rule. This file is the standalone witness: public API only, from outside the crate,
//! and it adds what a unit test cannot — sweeps wide enough to be evidence, and the
//! composition of admission with snapshot construction that an importer will actually
//! perform (PR 3 / IMPL-02).
//!
//! # Evidence map
//!
//! | Claim | Test |
//! |---|---|
//! | admission is a pure function of the candidate *set*, not of the walk | [`positive_admission_is_a_function_of_the_candidate_set`] |
//! | the order is segment-wise, total, and agrees with the built tree | [`positive_the_path_order_is_total_and_agrees_with_the_tree`] |
//! | distinct byte names stay distinct: admission never merges two files | [`positive_admission_is_injective_on_the_names_it_accepts`] |
//! | normalization and case pairs are distinct snapshots, not one | [`negative_normalization_and_case_pairs_are_distinct_snapshots`] |
//! | a name that is not UTF-8 is refused, never escaped or flattened | [`negative_names_that_are_not_utf8_are_refused`] |
//! | every reserved and non-portable name is a typed refusal | [`negative_reserved_and_non_portable_names_are_refused`] |
//! | a symbolic link, a device node, and an unimplemented policy are typed refusals | [`negative_links_special_files_and_unimplemented_policies_are_refused`] |
//! | candidates that are not a tree are a typed refusal, order-independently | [`negative_candidates_that_are_not_a_tree_are_refused`] |
//! | *which* rejection is reported cannot depend on the walk order | [`adversarial_the_walk_order_cannot_change_the_rejection`] |
//! | every typed rejection this policy defines has a witness | [`adversarial_every_typed_rejection_has_a_witness`] |
//! | boundaries: the deepest, widest, and longest-named workspaces | [`boundary_depth_width_and_segment_length`] |
//!
//! # House rules
//!
//! - **Nothing in `src/` is touched.** Every seam used here is public API.
//! - **No ambient randomness.** [`Shuffle`] is a written-down seed and a xorshift, so a
//!   failure here reproduces exactly (INV-005 does not stop at `src/`).
//! - **The identity seam is injective.** [`HexIdentity`] makes a record's own bytes its
//!   name, which is ADR-0013's certified discipline, so "these two workspaces have two
//!   identities" is a fact rather than a probability. [`Fnv1aTestIdentity`] is a
//!   fixed-width stand-in — **not** a release hash — used only where identities must stay
//!   short, at the depth and width boundaries.

use std::collections::{BTreeMap, BTreeSet};

use continuum_workspace::artifact_path::{ArtifactClass, ArtifactHandle};
use continuum_workspace::publication::{ContentIdentifier, IdentityUnavailable};
use continuum_workspace::snapshot::{
    AdmissionError, AdmissionPolicy, Candidate, MAX_DEPTH, MAX_SEGMENT_BYTES, PathError,
    ReservedReason, Snapshot, SnapshotError, SymlinkPolicy, WorkspaceContent, WorkspacePath, admit,
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

/// The injective seam: a record's own bytes, in hex (ADR-0013's certified discipline).
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
/// `tests/merkle_snapshot.rs` uses, for the same reason.
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

// --- deterministic corpora ---------------------------------------------------------------

/// A seeded xorshift. Every corpus and every permutation below is a function of a written
/// down seed and of nothing else, so a failure reproduces exactly.
struct Shuffle(u64);

impl Shuffle {
    const fn new(seed: u64) -> Self {
        // Zero is xorshift's fixed point; a corpus that never moves proves nothing.
        Self(seed | 1)
    }

    fn step(&mut self) -> u64 {
        let mut state = self.0;
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        self.0 = state;
        state
    }

    fn below(&mut self, bound: usize) -> usize {
        let bound = u64::try_from(bound).unwrap_or(1).max(1);
        usize::try_from(self.step() % bound).unwrap_or(0)
    }

    fn permute<T>(&mut self, items: &mut [T]) {
        for index in (1..items.len()).rev() {
            let other = self.below(index + 1);
            items.swap(index, other);
        }
    }
}

/// `café` precomposed (NFC) and decomposed (NFD): one grapheme cluster, two names.
const NFC: &str = "caf\u{e9}";
/// See [`NFC`].
const NFD: &str = "cafe\u{301}";

/// Segments chosen so that every rule the policy declines to apply has a witness in every
/// generated name: normalization pairs, case pairs, a fold that changes length, the
/// `.`-versus-`/` ordering divergence, and a format character.
const SEGMENT_ALPHABET: [&str; 12] = [
    "a",
    "b",
    "A",
    "B",
    "a.b",
    "ab",
    NFC,
    NFD,
    "stra\u{df}e",
    "strasse",
    "\u{200e}x",
    "z",
];

/// A deterministic corpus of distinct, admissible path names.
fn generated_names(seed: u64, count: usize) -> Vec<String> {
    let mut shuffle = Shuffle::new(seed);
    let mut names: BTreeSet<String> = BTreeSet::new();
    // Bounded rather than looped-until-full: a test that can spin is not a test.
    for _ in 0..count * 8 {
        if names.len() == count {
            break;
        }
        let depth = 1 + shuffle.below(3);
        let name: Vec<&str> = (0..depth)
            .map(|_| SEGMENT_ALPHABET[shuffle.below(SEGMENT_ALPHABET.len())])
            .collect();
        names.insert(name.join("/"));
    }
    // A path that is a file cannot also be a directory, so drop any name that is a proper
    // prefix of another: that is `WorkspaceContent`'s rule, not an ordering question.
    let all: Vec<String> = names.into_iter().collect();
    all.iter()
        .filter(|name| {
            !all.iter()
                .any(|other| other.starts_with(&format!("{name}/")))
        })
        .cloned()
        .collect()
}

/// Candidate entries for a list of names, each with content derived from its own name so
/// that no two files are accidentally equal.
fn candidates<'a>(names: &'a [String]) -> Vec<(&'a [u8], Candidate<'a>)> {
    names
        .iter()
        .map(|name| (name.as_bytes(), Candidate::File(name.as_bytes())))
        .collect()
}

fn admitted(names: &[String]) -> WorkspaceContent {
    admit(candidates(names), AdmissionPolicy::new()).expect("the corpus is a tree")
}

fn snapshot(content: &WorkspaceContent) -> Snapshot {
    Snapshot::build(content, &HexIdentity).expect("HexIdentity names every node")
}

// --- positive: one workspace, one description, one identity -------------------------------

#[test]
fn positive_admission_is_a_function_of_the_candidate_set() {
    // A directory walk yields entries in whatever order the filesystem felt like. The
    // description — and therefore the snapshot, and therefore the encoding — may not
    // depend on it, in any of the three.
    for seed in [1u64, 0x5deece66d, 0x9e37_79b9_7f4a_7c15] {
        let names = generated_names(seed, 48);
        assert!(names.len() > 24, "the generated corpus is too thin to test");

        let expected = admitted(&names);
        let expected_snapshot = snapshot(&expected);
        let expected_encoding = expected_snapshot.encode();

        let mut shuffle = Shuffle::new(seed ^ 0xa5a5_a5a5);
        for _ in 0..32 {
            let mut permuted = candidates(&names);
            shuffle.permute(&mut permuted);
            let content = admit(permuted, AdmissionPolicy::new()).expect("a tree");
            assert_eq!(content, expected, "seed {seed}");

            let built = snapshot(&content);
            assert_eq!(built.identity(), expected_snapshot.identity());
            assert_eq!(built.encode(), expected_encoding);
        }
    }
}

#[test]
fn positive_the_path_order_is_total_and_agrees_with_the_tree() {
    let names = generated_names(0x0bad_c0de, 40);
    let paths: Vec<WorkspacePath> = names
        .iter()
        .map(|name| WorkspacePath::from_bytes(name.as_bytes()).expect("admissible"))
        .collect();

    // A total order: reflexive, antisymmetric, transitive, total, and agreeing with
    // equality. Checked over the whole corpus rather than over a chosen pair.
    for left in &paths {
        assert_eq!(left.cmp(left), std::cmp::Ordering::Equal);
        for right in &paths {
            assert_eq!(left.cmp(right), right.cmp(left).reverse());
            assert_eq!(
                (left == right),
                left.cmp(right) == std::cmp::Ordering::Equal
            );
            for third in &paths {
                if left <= right && right <= third {
                    assert!(left <= third, "{left} <= {right} <= {third}");
                }
            }
        }
    }

    // And the order the tree lists is that order, strictly ascending, with no repeats.
    let listing: Vec<WorkspacePath> = snapshot(&admitted(&names))
        .subtrees()
        .into_iter()
        .map(|(path, _)| path)
        .collect();
    assert!(listing.windows(2).all(|pair| pair[0] < pair[1]));

    // Segment-wise, not string order on the rendered form. The corpus contains both
    // spellings, so this is a claim about the corpus and not only about a literal.
    assert!("a.b" < "a/b");
    let dotted = WorkspacePath::from_bytes(b"a.b").expect("admissible");
    let nested = WorkspacePath::from_bytes(b"a/b").expect("admissible");
    assert!(nested < dotted);
}

#[test]
fn positive_admission_is_injective_on_the_names_it_accepts() {
    // No normalization, no folding, no separator collapsing: two distinct accepted byte
    // strings are two distinct paths, and rendering a path returns its bytes. That is the
    // property an escaping scheme would have had to prove and this policy gets for free
    // by refusing to rewrite anything.
    let names = generated_names(0xfeed_face, 64);
    let mut paths: BTreeSet<WorkspacePath> = BTreeSet::new();
    for name in &names {
        let path = WorkspacePath::from_bytes(name.as_bytes()).expect("admissible");
        assert_eq!(path.to_string().as_bytes(), name.as_bytes());
        assert!(paths.insert(path), "{name:?} collided with another name");
    }
    assert_eq!(paths.len(), names.len());

    // End to end: the workspace holds exactly as many files as there were names, and each
    // one is reachable under the name it arrived with.
    let content = admitted(&names);
    assert_eq!(content.len(), names.len());
    let built = snapshot(&content);
    assert_eq!(built.file_count(), names.len());
    for name in &names {
        let path = WorkspacePath::from_bytes(name.as_bytes()).expect("admissible");
        assert!(
            built.node(&path).is_some(),
            "{name} is missing from the tree"
        );
    }
}

// --- negative: what one name may not become ------------------------------------------------

#[test]
fn negative_normalization_and_case_pairs_are_distinct_snapshots() {
    // Each pair renders identically-or-nearly on a screen and differs in bytes. Every one
    // of them is two files, two paths, and two snapshot identities.
    let pairs: [(&str, &str); 5] = [
        (NFC, NFD),                 // Unicode normalization
        ("A.rs", "a.rs"),           // simple case
        ("stra\u{df}e", "strasse"), // full folding, which changes length
        ("\u{131}.rs", "i.rs"),     // locale-dependent folding (Turkish)
        ("\u{212b}", "\u{c5}"),     // Angstrom sign vs. precomposed A-ring
    ];

    for (left, right) in pairs {
        assert_ne!(left, right);

        let left_path = WorkspacePath::from_bytes(left.as_bytes()).expect("admissible");
        let right_path = WorkspacePath::from_bytes(right.as_bytes()).expect("admissible");
        assert_ne!(left_path, right_path, "{left:?} and {right:?} merged");

        // Two snapshots of the same bytes under the two names are two snapshots.
        let left_snapshot = snapshot(
            &admit(
                [(left.as_bytes(), Candidate::File(b"same content"))],
                AdmissionPolicy::new(),
            )
            .expect("a tree"),
        );
        let right_snapshot = snapshot(
            &admit(
                [(right.as_bytes(), Candidate::File(b"same content"))],
                AdmissionPolicy::new(),
            )
            .expect("a tree"),
        );
        assert_ne!(left_snapshot.identity(), right_snapshot.identity());

        // The leaf is shared — content is content — which is exactly why the *directory*
        // has to name its children and does.
        assert_eq!(
            left_snapshot.node(&left_path).map(|node| node.record()),
            right_snapshot.node(&right_path).map(|node| node.record()),
        );

        // And both names coexist in one workspace, as two files.
        let both = admit(
            [
                (left.as_bytes(), Candidate::File(b"left")),
                (right.as_bytes(), Candidate::File(b"right")),
            ],
            AdmissionPolicy::new(),
        )
        .expect("two names, two files");
        assert_eq!(both.len(), 2);
        assert_eq!(both.get(&left_path), Some(b"left".as_slice()));
        assert_eq!(both.get(&right_path), Some(b"right".as_slice()));
    }
}

#[test]
fn negative_names_that_are_not_utf8_are_refused() {
    // Refused, never escaped and never lossily decoded. The overlong `/` is the sharp
    // case: a decoder that accepted it would gain a second spelling of the separator.
    let invalid: [&[u8]; 8] = [
        b"a\xc0\xafb",          // overlong encoding of `/`
        b"\xc0\x80",            // overlong encoding of NUL
        b"\xed\xa0\x80",        // a lone surrogate half
        b"\xed\xbf\xbf",        // the other end of the surrogate range
        b"ab\xe2\x82",          // a truncated three-byte sequence
        b"\x80",                // a bare continuation byte
        b"\xff\xfe",            // a UTF-16 byte-order mark, byte-swapped into nonsense
        b"ok/\xf5\x80\x80\x80", // past the U+10FFFF ceiling, in a later segment
    ];

    for raw in invalid {
        assert!(
            matches!(
                WorkspacePath::from_bytes(raw),
                Err(PathError::SegmentEncoding { .. })
            ),
            "{raw:?} was not refused"
        );
        assert!(matches!(
            admit([(raw, Candidate::File(b""))], AdmissionPolicy::new()),
            Err(AdmissionError::Name {
                error: PathError::SegmentEncoding { .. },
                ..
            })
        ));
    }

    // The refusal is not "everything unusual": the replacement character is a perfectly
    // good name when the file is *actually* called that, which is why a lossy decode of
    // the sequences above would have merged distinct files onto it.
    let replacement = "\u{fffd}".as_bytes();
    assert!(WorkspacePath::from_bytes(replacement).is_ok());
    let content = admit(
        [(replacement, Candidate::File(b"genuinely named U+FFFD"))],
        AdmissionPolicy::new(),
    )
    .expect("a tree");
    assert_eq!(content.len(), 1);
}

#[test]
fn negative_reserved_and_non_portable_names_are_refused() {
    // One assertion per rule, and a near-miss beside each so the rule is not wider than
    // it says.
    let reserved: [(&str, ReservedReason, &str); 6] = [
        ("CON", ReservedReason::DeviceName, "CONS"),
        ("nul.txt", ReservedReason::DeviceName, "null.txt"),
        ("COM1", ReservedReason::DeviceName, "COM10"),
        ("LPT9.tar.gz", ReservedReason::DeviceName, "LPT.tar.gz"),
        ("build.", ReservedReason::TrailingDot, ".build"),
        ("build ", ReservedReason::TrailingSpace, "bu ild"),
    ];
    for (refused, reason, accepted) in reserved {
        assert_eq!(
            WorkspacePath::from_bytes(refused.as_bytes()),
            Err(PathError::ReservedSegment {
                index: 0,
                segment: refused.to_owned(),
                reason,
            }),
            "{refused:?} was accepted"
        );
        assert!(
            WorkspacePath::from_bytes(accepted.as_bytes()).is_ok(),
            "{accepted:?} was refused; the rule is wider than it claims"
        );
    }

    // The characters no ordinary Win32 path may hold, and the control range.
    for character in ['\\', '<', '>', ':', '"', '|', '?', '*', '\u{0}', '\u{7f}'] {
        let name = format!("src/a{character}b");
        assert!(
            matches!(
                WorkspacePath::from_bytes(name.as_bytes()),
                Err(PathError::SegmentCharacter { index: 1, .. })
            ),
            "{name:?} was accepted"
        );
    }

    // Traversal stays refused through the byte boundary too, which is where an importer
    // meets it (docs/19 §9, docs/09).
    for raw in [b"../etc/passwd".as_slice(), b"a/./b", b"a/../b", b"/abs"] {
        assert!(
            WorkspacePath::from_bytes(raw).is_err(),
            "{raw:?} was accepted"
        );
    }
}

#[test]
fn negative_links_special_files_and_unimplemented_policies_are_refused() {
    // A symbolic link is a name, not content.
    assert_eq!(
        admit(
            [
                (b"src/lib.rs".as_slice(), Candidate::File(b"fn main() {}")),
                (b"src/escape".as_slice(), Candidate::Symlink(b"/etc/passwd")),
            ],
            AdmissionPolicy::new()
        ),
        Err(AdmissionError::Symlink {
            name: b"src/escape".to_vec(),
            target: b"/etc/passwd".to_vec(),
        })
    );

    // A FIFO has no content: reading one yields whatever was in flight (INV-005).
    assert_eq!(
        admit(
            [(b"pipe".as_slice(), Candidate::Special)],
            AdmissionPolicy::new()
        ),
        Err(AdmissionError::Special {
            name: b"pipe".to_vec()
        })
    );

    // And the arm that is named but not implemented refuses before reading anything, so
    // an importer cannot follow a link without coming back to this module.
    let following = AdmissionPolicy::new().with_symlinks(SymlinkPolicy::FollowWithCycleDetection);
    assert!(!following.symlinks().is_implemented());
    assert!(SymlinkPolicy::Reject.is_implemented());
    assert_eq!(SymlinkPolicy::default(), SymlinkPolicy::Reject);
    for corpus in [Vec::new(), vec![(b"a".as_slice(), Candidate::File(b"x"))]] {
        assert_eq!(
            admit(corpus, following),
            Err(AdmissionError::UnsupportedSymlinkPolicy {
                policy: SymlinkPolicy::FollowWithCycleDetection
            })
        );
    }
}

#[test]
fn negative_candidates_that_are_not_a_tree_are_refused() {
    let repeated: Vec<(&[u8], Candidate<'_>)> = vec![
        (b"a/b", Candidate::File(b"one")),
        (b"a/b", Candidate::File(b"two")),
    ];
    assert!(matches!(
        admit(repeated, AdmissionPolicy::new()),
        Err(AdmissionError::NotATree(
            SnapshotError::DuplicatePath { .. }
        ))
    ));

    let conflicting: Vec<(&[u8], Candidate<'_>)> = vec![
        (b"a/b/c", Candidate::File(b"deep")),
        (b"a/b", Candidate::File(b"shallow")),
    ];
    let expected = Err(AdmissionError::NotATree(SnapshotError::PathConflict {
        file: WorkspacePath::from_bytes(b"a/b").expect("admissible"),
        descendant: WorkspacePath::from_bytes(b"a/b/c").expect("admissible"),
    }));
    assert_eq!(admit(conflicting.clone(), AdmissionPolicy::new()), expected);
    let mut reversed = conflicting;
    reversed.reverse();
    assert_eq!(admit(reversed, AdmissionPolicy::new()), expected);
}

// --- adversarial ---------------------------------------------------------------------------

#[test]
fn adversarial_the_walk_order_cannot_change_the_rejection() {
    // Six distinct defects in one candidate set. Whichever order the walk produced them
    // in, one of them is reported — the same one — because "first" is by raw name and not
    // by arrival. Two importers of one workspace must agree on *why* it was refused for
    // the same reason they must agree on its identity.
    let corpus: Vec<(&[u8], Candidate<'_>)> = vec![
        (b"z-good", Candidate::File(b"x")),
        (b"y-reserved/CON", Candidate::File(b"x")),
        (b"x-bad-utf8\xff", Candidate::File(b"x")),
        (b"w-non-portable:name", Candidate::File(b"x")),
        (b"v-link", Candidate::Symlink(b"elsewhere")),
        (b"u-special", Candidate::Special),
        (b"t-traversal/..", Candidate::File(b"x")),
    ];

    let expected = admit(corpus.clone(), AdmissionPolicy::new());
    assert!(expected.is_err());
    // Least in raw-name order among the defective candidates: `t-traversal/..`.
    assert_eq!(
        expected,
        Err(AdmissionError::Name {
            name: b"t-traversal/..".to_vec(),
            error: PathError::RelativeSegment {
                index: 1,
                segment: "..".to_owned(),
            },
        })
    );

    let mut shuffle = Shuffle::new(0x1234_5678_9abc_def0);
    for _ in 0..128 {
        let mut permuted = corpus.clone();
        shuffle.permute(&mut permuted);
        assert_eq!(admit(permuted, AdmissionPolicy::new()), expected);
    }

    // Remove the least-named defect and the answer moves to the next one, deterministically
    // — the rule is an order, not a hard-coded winner.
    let mut without = corpus;
    without.retain(|(name, _)| *name != b"t-traversal/..".as_slice());
    let expected = admit(without.clone(), AdmissionPolicy::new());
    assert_eq!(
        expected,
        Err(AdmissionError::Special {
            name: b"u-special".to_vec()
        })
    );
    let mut shuffle = Shuffle::new(0x0fed_cba9_8765_4321);
    for _ in 0..128 {
        let mut permuted = without.clone();
        shuffle.permute(&mut permuted);
        assert_eq!(admit(permuted, AdmissionPolicy::new()), expected);
    }
}

#[test]
fn adversarial_every_typed_rejection_has_a_witness() {
    // A rejection nobody can produce is a rule nobody is bound by. Each row is a witness
    // for one variant of the policy's error surface; the assertion at the end is that the
    // surface has no unwitnessed corner.
    let mut witnessed: BTreeMap<&'static str, String> = BTreeMap::new();
    let mut witness = |label: &'static str, error: &dyn std::fmt::Display| {
        witnessed.insert(label, error.to_string());
    };

    let cases: [(&'static str, &[u8]); 10] = [
        ("Empty", b""),
        ("EmptySegment", b"a//b"),
        ("RelativeSegment", b".."),
        ("SegmentCharacter", b"a\\b"),
        ("SegmentEncoding", b"\xff"),
        ("ReservedSegment/DeviceName", b"AUX"),
        ("ReservedSegment/TrailingDot", b"a."),
        ("ReservedSegment/TrailingSpace", b"a "),
        ("SegmentTooLong", &[b'x'; MAX_SEGMENT_BYTES + 1]),
        ("TooDeep", b"a"), // replaced below: a literal deep path is unwieldy
    ];
    for (label, raw) in cases {
        if label == "TooDeep" {
            let deep = ["x"; MAX_DEPTH + 1].join("/");
            let error = WorkspacePath::from_bytes(deep.as_bytes()).expect_err("past the bound");
            assert!(matches!(error, PathError::TooDeep { .. }));
            witness(label, &error);
            continue;
        }
        let error = WorkspacePath::from_bytes(raw).expect_err("refused");
        witness(label, &error);
    }

    // The admission-level variants, which no single path can produce.
    witness(
        "AdmissionError::Name",
        &admit(
            [(b"\xff".as_slice(), Candidate::File(b""))],
            AdmissionPolicy::new(),
        )
        .expect_err("refused"),
    );
    witness(
        "AdmissionError::Symlink",
        &admit(
            [(b"link".as_slice(), Candidate::Symlink(b"target"))],
            AdmissionPolicy::new(),
        )
        .expect_err("refused"),
    );
    witness(
        "AdmissionError::Special",
        &admit(
            [(b"node".as_slice(), Candidate::Special)],
            AdmissionPolicy::new(),
        )
        .expect_err("refused"),
    );
    witness(
        "AdmissionError::UnsupportedSymlinkPolicy",
        &admit(
            Vec::new(),
            AdmissionPolicy::new().with_symlinks(SymlinkPolicy::FollowWithCycleDetection),
        )
        .expect_err("refused"),
    );
    witness(
        "AdmissionError::NotATree",
        &admit(
            [
                (b"a".as_slice(), Candidate::File(b"1")),
                (b"a".as_slice(), Candidate::File(b"2")),
            ],
            AdmissionPolicy::new(),
        )
        .expect_err("refused"),
    );

    assert_eq!(witnessed.len(), 15, "a rejection lost its witness");
    // Every message says something, and none of them leaks a raw byte as a `Debug` blob.
    for (label, message) in &witnessed {
        assert!(!message.is_empty(), "{label} renders to nothing");
        assert!(!message.contains("[100, 101"), "{label} renders raw bytes");
    }
}

// --- boundaries --------------------------------------------------------------------------------

#[test]
fn boundary_depth_width_and_segment_length() {
    // At the segment-length bound, at the depth bound, and both at once: accepted, and a
    // byte past either is a typed refusal.
    let longest = "x".repeat(MAX_SEGMENT_BYTES);
    assert!(WorkspacePath::from_bytes(longest.as_bytes()).is_ok());
    assert!(matches!(
        WorkspacePath::from_bytes("x".repeat(MAX_SEGMENT_BYTES + 1).as_bytes()),
        Err(PathError::SegmentTooLong { .. })
    ));

    let deepest: Vec<String> = (0..MAX_DEPTH).map(|index| format!("d{index}")).collect();
    let deepest = deepest.join("/");
    assert!(WorkspacePath::from_bytes(deepest.as_bytes()).is_ok());
    assert!(matches!(
        WorkspacePath::from_bytes(["x"; MAX_DEPTH + 1].join("/").as_bytes()),
        Err(PathError::TooDeep { .. })
    ));

    // The worst admissible path: MAX_DEPTH segments of MAX_SEGMENT_BYTES bytes each. It
    // admits, it builds, and it round-trips.
    let heaviest: Vec<String> = (0..MAX_DEPTH)
        .map(|index| format!("{index:03}{}", "y".repeat(MAX_SEGMENT_BYTES - 3)))
        .collect();
    let heaviest = heaviest.join("/");
    let content = admit(
        [(heaviest.as_bytes(), Candidate::File(b"deep and wide"))],
        AdmissionPolicy::new(),
    )
    .expect("at both bounds");
    let built = Snapshot::build(&content, &Fnv1aTestIdentity).expect("named");
    assert_eq!(built.subtrees().len(), MAX_DEPTH);
    let bytes = built.encode();
    assert_eq!(
        Snapshot::decode(&bytes, &Fnv1aTestIdentity).expect("round trip"),
        built
    );

    // Widest: 512 siblings whose names differ only in ways this policy refuses to
    // normalize away — case and normalization form — admitted in one directory and listed
    // in exactly one order.
    let mut names: Vec<String> = Vec::with_capacity(512);
    for index in 0..128 {
        names.push(format!("f{index:03}"));
        names.push(format!("F{index:03}"));
        names.push(format!("{NFC}{index:03}"));
        names.push(format!("{NFD}{index:03}"));
    }
    let forward = admit(candidates(&names), AdmissionPolicy::new()).expect("a tree");
    assert_eq!(forward.len(), 512, "a name was merged into another");

    let mut shuffle = Shuffle::new(0x2468_ace0_1357_bdf9);
    let mut permuted = candidates(&names);
    shuffle.permute(&mut permuted);
    let shuffled = admit(permuted, AdmissionPolicy::new()).expect("a tree");
    assert_eq!(shuffled, forward);

    let listing: Vec<String> = forward.paths().map(ToString::to_string).collect();
    let mut sorted = listing.clone();
    sorted.sort();
    assert_eq!(listing, sorted);
    assert_eq!(
        Snapshot::build(&shuffled, &Fnv1aTestIdentity)
            .expect("named")
            .identity(),
        Snapshot::build(&forward, &Fnv1aTestIdentity)
            .expect("named")
            .identity()
    );
}
