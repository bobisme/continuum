//! PR-20 / IMPL-02 evidence: patch identity (bn-195b).
//!
//! RFC 0032 fixes what a patch identity is by what gate 2 compares: "the sealed candidate
//! snapshot's digest equals the normalized base+changes digest", and `apply` "seals a
//! candidate snapshot from base + changes". So the candidate is derived, never named:
//! `SealedCandidate::seal` builds it from the base content and the declared edits through
//! `continuum-workspace`'s Merkle snapshot, and each `changes[].digest` is the digest of
//! the change's canonical record.
//!
//! # Canonical order
//!
//! The changes of one proposal must be path-disjoint, so they commute. The candidate is a
//! Merkle root, a function of the file *set* under the snapshot ordering contract
//! (segment-wise path order), and the recorded `changes` are written in ascending order
//! of each change's least path under that same order. Reordering declared changes
//! therefore moves no byte of the candidate, the changes, or the `rt_` identity.
//!
//! # Retained artifacts
//!
//! | Artifact id | What it is |
//! |---|---|
//! | `pr20-impl02-ack-after-sync-patch-identity` | the base, the candidate, and the recorded |
//! | | change of the move-ack-after-sync repair under BLAKE3, in |
//! | | `tests/fixtures/pr20-impl02-ack-after-sync-patch-identity.json` |
//!
//! Set `CONTINUUM_REPAIR_BLESS=1` to rewrite it from the library.

mod common;

use std::collections::BTreeSet;

use continuum_intent::canonical_json::Json;
use continuum_intent::contract::{IntentContract, IntentId};
use continuum_repair::handle::{CrashpackId, SnapshotId};
use continuum_repair::hypothesis::{ChangeKind, Hypothesis, Proposal};
use continuum_repair::patch::{
    DeclaredChange, FileEdit, MalformedChange, PatchRefusal, SealedCandidate,
};
use continuum_repair::transaction::{
    ApplyRefusal, FailureBinding, GateProfile, RepairTransaction, Resolution,
};
use continuum_value::identity::{Blake3Hasher, ContentHasher, Fnv1aPlaceholder};
use continuum_workspace::snapshot::WorkspaceContent;

use common::{
    MANIFEST, MANIFEST_PATH, MODEL, MODEL_PATH, REPLICA, REPLICA_AFTER, REPLICA_BEFORE,
    ack_after_sync_change, base_content, base_id, content, leaf, path, repaired_content, replace,
    snapshot_id,
};

type Sealed = SealedCandidate<Blake3Hasher>;
type Tx = RepairTransaction<Blake3Hasher>;

const FIXTURE: &str = "tests/fixtures/pr20-impl02-ack-after-sync-patch-identity.json";
const CRASH: &str = "crash_pr20_impl02_ack_before_sync";
const REGISTER_CONTRACT: &[u8] =
    include_bytes!("../../continuum-intent/tests/fixtures/replicated-register-contract.json");

/// The model edit that accompanies the repair in the commuting tests: the model records
/// that an ack follows durability.
const MODEL_AFTER: &str = "model ReplicatedRegister {\n  var stored: Nat\n  var acked: Set[Nat]\n  invariant acked <= durable\n}\n";

struct Binding;

impl FailureBinding for Binding {
    fn failure_base(&self, failure: &CrashpackId) -> Resolution<SnapshotId> {
        if failure.as_str() == CRASH {
            Resolution::Found(base_id())
        } else {
            Resolution::Unknown
        }
    }

    fn snapshot_intent(&self, snapshot: &SnapshotId) -> Resolution<IntentId> {
        if *snapshot == base_id() {
            Resolution::Found(
                IntentContract::decode(REGISTER_CONTRACT)
                    .expect("the register contract decodes")
                    .intent_id()
                    .clone(),
            )
        } else {
            Resolution::Unknown
        }
    }
}

fn draft() -> Tx {
    Tx::begin(
        CrashpackId::new(CRASH).unwrap(),
        GateProfile::PhaseB,
        &Binding,
    )
    .expect("the crashpack resolves")
}

fn seal(changes: &[DeclaredChange]) -> Result<Sealed, PatchRefusal> {
    Sealed::seal(&base_id(), &base_content(), changes)
}

fn model_change() -> DeclaredChange {
    replace(ChangeKind::Model, MODEL_PATH, MODEL_AFTER.as_bytes())
}

fn string(value: &str) -> Json {
    Json::String(value.to_owned())
}

// --- positive ----------------------------------------------------------------------

/// `pr20-impl02-ack-after-sync-patch-identity`: the move-ack-after-sync change yields one
/// patch identity, every time — the candidate is exactly the repaired register, and the
/// recorded change is the digest of the declared edit.
#[test]
fn pr20_impl02_positive_ack_after_sync_yields_a_stable_patch_identity() {
    let first = seal(&[ack_after_sync_change()]).expect("the repair applies to its base");
    let second = seal(&[ack_after_sync_change()]).expect("the repair applies to its base");
    assert_eq!(first, second);
    assert_eq!(first.identity(), second.identity());
    assert_eq!(first.base(), &base_id());
    assert_eq!(first.identity(), &snapshot_id(&repaired_content()));
    assert_ne!(first.identity(), &base_id());
    assert_eq!(first.check(&repaired_content()), Ok(()));
    assert_eq!(
        first.changes(),
        [ack_after_sync_change().recorded::<Blake3Hasher>()]
    );
    assert!(first.changes()[0].digest().starts_with("blake3-256:"));

    // `apply` records exactly this candidate and these changes.
    let proposal = Proposal::new(
        Hypothesis::new("move the ack after the sync"),
        vec![ack_after_sync_change()],
    );
    let applied = draft()
        .apply(&proposal, &base_content())
        .expect("the proposal applies");
    assert_eq!(applied.candidate(), &first);
    assert_eq!(
        applied.transaction().candidate_snapshot(),
        Some(first.identity())
    );
    assert_eq!(applied.transaction().changes(), first.changes());
    // Two independent `apply` runs name one version.
    let again = draft().apply(&proposal, &base_content()).unwrap();
    assert_eq!(
        again.transaction().repair_id(),
        applied.transaction().repair_id()
    );

    let retained = Json::object([
        ("base_snapshot".to_owned(), string(first.base().as_str())),
        (
            "candidate_snapshot".to_owned(),
            string(first.identity().as_str()),
        ),
        (
            "changes".to_owned(),
            Json::Array(
                first
                    .changes()
                    .iter()
                    .map(|change| {
                        Json::object([
                            ("digest".to_owned(), string(change.digest())),
                            ("kind".to_owned(), string(change.kind().token())),
                        ])
                        .unwrap()
                    })
                    .collect(),
            ),
        ),
    ])
    .unwrap()
    .to_canonical_bytes();
    let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(FIXTURE);
    if std::env::var_os("CONTINUUM_REPAIR_BLESS").is_some() {
        std::fs::write(&fixture, &retained).expect("the fixture is writable");
    }
    let on_disk = std::fs::read(&fixture).expect("the retained fixture exists");
    assert!(
        on_disk == retained,
        "{FIXTURE} is stale; rerun with CONTINUUM_REPAIR_BLESS=1\nlibrary:  {}",
        String::from_utf8_lossy(&retained)
    );
}

/// Create, replace, and delete each seal to the content they describe, including a
/// delete that frees a name for a directory in the same proposal.
#[test]
fn pr20_impl02_positive_every_edit_kind_seals_to_the_content_it_describes() {
    let change = DeclaredChange::new(
        ChangeKind::Domain,
        [
            (
                path(MANIFEST_PATH),
                FileEdit::Delete {
                    before: leaf(&base_content(), MANIFEST_PATH),
                },
            ),
            (
                path("Cargo.toml/register.toml"),
                FileEdit::Create {
                    content: b"name = \"register\"\n".to_vec(),
                },
            ),
        ],
    )
    .unwrap();
    let sealed = seal(&[change, ack_after_sync_change()]).expect("the edits apply");
    let expected = content(&[
        ("Cargo.toml/register.toml", b"name = \"register\"\n"),
        (MODEL_PATH, MODEL.as_bytes()),
        (REPLICA, REPLICA_AFTER.as_bytes()),
    ]);
    assert_eq!(sealed.identity(), &snapshot_id(&expected));
    assert_eq!(sealed.check(&expected), Ok(()));

    // No changes at all: the candidate is the base itself.
    let unchanged = seal(&[]).unwrap();
    assert_eq!(unchanged.identity(), &base_id());
    assert!(unchanged.changes().is_empty());
}

// --- metamorphic -------------------------------------------------------------------

/// Metamorphic relation: set/map insertion order. The declared changes are a set, and so
/// are the edits inside one change. Reordering commutative changes is invisible: the candidate, the recorded changes, and
/// the transaction's `rt_` identity are one value for every order. The recorded order is
/// ascending least path under the snapshot ordering contract — `model/register.cml`
/// before `src/replica.rs` — whatever order the changes were declared in.
#[test]
fn pr20_impl02_metamorphic_reordering_commutative_changes_follows_the_canonical_order() {
    let forward = seal(&[ack_after_sync_change(), model_change()]).unwrap();
    let backward = seal(&[model_change(), ack_after_sync_change()]).unwrap();
    assert_eq!(forward, backward);
    assert_eq!(forward.identity(), backward.identity());
    assert_eq!(
        forward.changes(),
        [
            model_change().recorded::<Blake3Hasher>(),
            ack_after_sync_change().recorded::<Blake3Hasher>(),
        ]
    );

    let apply = |changes: Vec<DeclaredChange>| {
        draft()
            .apply(
                &Proposal::new(Hypothesis::new("h"), changes),
                &base_content(),
            )
            .unwrap()
            .into_parts()
            .0
    };
    let one = apply(vec![ack_after_sync_change(), model_change()]);
    let other = apply(vec![model_change(), ack_after_sync_change()]);
    assert_eq!(one.repair_id(), other.repair_id());
    assert_eq!(one.to_artifact_bytes(), other.to_artifact_bytes());

    // Inside one change, the order its edits are listed in does not reach its record.
    let both = |order: [usize; 2]| {
        let edits = [
            (
                path(MODEL_PATH),
                FileEdit::Replace {
                    before: leaf(&base_content(), MODEL_PATH),
                    content: MODEL_AFTER.as_bytes().to_vec(),
                },
            ),
            (
                path(REPLICA),
                FileEdit::Replace {
                    before: leaf(&base_content(), REPLICA),
                    content: REPLICA_AFTER.as_bytes().to_vec(),
                },
            ),
        ];
        DeclaredChange::new(ChangeKind::Rust, order.map(|index| edits[index].clone())).unwrap()
    };
    assert_eq!(
        both([0, 1]).canonical_bytes(),
        both([1, 0]).canonical_bytes()
    );
    // Splitting one change into two moves the recorded changes, not the candidate.
    let joined = seal(&[both([1, 0])]).unwrap();
    assert_eq!(joined.identity(), forward.identity());
    assert_ne!(joined.changes(), forward.changes());
}

// --- negative ----------------------------------------------------------------------

/// Gate 2's comparison: a candidate holding an undeclared file change — an extra file, a
/// changed byte in a file no change touches, a missing file — is refused, and the
/// refusal names every smuggled path.
#[test]
fn pr20_impl02_negative_an_undeclared_file_change_in_the_candidate_is_refused() {
    let sealed = seal(&[ack_after_sync_change()]).unwrap();

    let extra = content(&[
        (MANIFEST_PATH, MANIFEST.as_bytes()),
        (MODEL_PATH, MODEL.as_bytes()),
        (REPLICA, REPLICA_AFTER.as_bytes()),
        ("src/backdoor.rs", b"pub fn skip_sync() {}"),
    ]);
    assert_eq!(
        sealed.check(&extra),
        Err(PatchRefusal::UndeclaredEdits {
            paths: vec![path("src/backdoor.rs")],
        })
    );

    let mut tweaked_model = MODEL.as_bytes().to_vec();
    tweaked_model[0] ^= 0x20;
    let tweaked = content(&[
        (MANIFEST_PATH, MANIFEST.as_bytes()),
        (MODEL_PATH, &tweaked_model),
        (REPLICA, REPLICA_AFTER.as_bytes()),
        ("src/backdoor.rs", b"pub fn skip_sync() {}"),
    ]);
    assert_eq!(
        sealed.check(&tweaked),
        Err(PatchRefusal::UndeclaredEdits {
            paths: vec![path(MODEL_PATH), path("src/backdoor.rs")],
        })
    );

    let missing = content(&[
        (MODEL_PATH, MODEL.as_bytes()),
        (REPLICA, REPLICA_AFTER.as_bytes()),
    ]);
    assert_eq!(
        sealed.check(&missing),
        Err(PatchRefusal::UndeclaredEdits {
            paths: vec![path(MANIFEST_PATH)],
        })
    );

    // The unrepaired base is not the candidate either: the declared edit is missing.
    assert_eq!(
        sealed.check(&base_content()),
        Err(PatchRefusal::UndeclaredEdits {
            paths: vec![path(REPLICA)],
        })
    );
    assert_eq!(sealed.check(&repaired_content()), Ok(()));
}

/// A change against a different base is refused, typed, in both of its forms: base
/// content that is not the transaction's frozen base, and a change whose edits were
/// authored against another snapshot.
#[test]
fn pr20_impl02_negative_a_change_against_a_different_base_is_refused() {
    // The base content does not have the transaction's base identity.
    let other = content(&[
        (MANIFEST_PATH, MANIFEST.as_bytes()),
        (MODEL_PATH, MODEL.as_bytes()),
        (REPLICA, REPLICA_BEFORE.as_bytes()),
        ("README.md", b"register"),
    ]);
    let proposal = Proposal::new(Hypothesis::new("h"), vec![ack_after_sync_change()]);
    assert_eq!(
        draft().apply(&proposal, &other).map(|_| ()),
        Err(ApplyRefusal::Patch(PatchRefusal::BaseMismatch {
            expected: base_id(),
            found: snapshot_id(&other),
        }))
    );

    // A replace authored against a replica the base does not hold.
    let foreign = content(&[(REPLICA, b"pub fn write() {}")]);
    let stale = DeclaredChange::new(
        ChangeKind::Rust,
        [(
            path(REPLICA),
            FileEdit::Replace {
                before: leaf(&foreign, REPLICA),
                content: REPLICA_AFTER.as_bytes().to_vec(),
            },
        )],
    )
    .unwrap();
    assert_eq!(
        seal(&[stale]),
        Err(PatchRefusal::StaleBefore {
            path: path(REPLICA)
        })
    );
    let stale_delete = DeclaredChange::new(
        ChangeKind::Rust,
        [(
            path(REPLICA),
            FileEdit::Delete {
                before: leaf(&foreign, REPLICA),
            },
        )],
    )
    .unwrap();
    assert_eq!(
        seal(&[stale_delete]),
        Err(PatchRefusal::StaleBefore {
            path: path(REPLICA)
        })
    );

    // Edits that presume files the base does or does not hold.
    let absent = DeclaredChange::new(
        ChangeKind::Rust,
        [(
            path("src/absent.rs"),
            FileEdit::Replace {
                before: leaf(&base_content(), REPLICA),
                content: Vec::new(),
            },
        )],
    )
    .unwrap();
    assert_eq!(
        seal(&[absent]),
        Err(PatchRefusal::MissingFile {
            path: path("src/absent.rs")
        })
    );
    let over = DeclaredChange::new(
        ChangeKind::Rust,
        [(
            path(REPLICA),
            FileEdit::Create {
                content: REPLICA_AFTER.as_bytes().to_vec(),
            },
        )],
    )
    .unwrap();
    assert_eq!(
        seal(&[over]),
        Err(PatchRefusal::CreateOverExisting {
            path: path(REPLICA)
        })
    );
    // A delete of a directory is not a delete of a file.
    let directory = DeclaredChange::new(
        ChangeKind::Rust,
        [(
            path("src"),
            FileEdit::Delete {
                before: leaf(&base_content(), REPLICA),
            },
        )],
    )
    .unwrap();
    assert_eq!(
        seal(&[directory]),
        Err(PatchRefusal::NotAFile { path: path("src") })
    );
    // A directory where the base holds a file is not a tree.
    let under_file = DeclaredChange::new(
        ChangeKind::Rust,
        [(
            path("src/replica.rs/inner.rs"),
            FileEdit::Create {
                content: Vec::new(),
            },
        )],
    )
    .unwrap();
    assert!(matches!(seal(&[under_file]), Err(PatchRefusal::Tree(_))));

    // An intent change is refused before any sealing work, even against a wrong base.
    let intent = Proposal::new(
        Hypothesis::new("h"),
        vec![replace(ChangeKind::Intent, MODEL_PATH, b"relaxed")],
    );
    assert_eq!(
        draft().apply(&intent, &other).map(|_| ()),
        Err(ApplyRefusal::IntentMutationDenied { index: 0 })
    );
}

/// Changes must be disjoint so that they commute, and a change must name something.
#[test]
fn pr20_impl02_negative_overlapping_and_malformed_changes_are_refused() {
    assert_eq!(
        seal(&[ack_after_sync_change(), ack_after_sync_change()]),
        Err(PatchRefusal::OverlappingChanges {
            path: path(REPLICA)
        })
    );
    let rival = replace(ChangeKind::Rust, REPLICA, b"pub fn write() {}");
    assert_eq!(
        seal(&[ack_after_sync_change(), rival]),
        Err(PatchRefusal::OverlappingChanges {
            path: path(REPLICA)
        })
    );
    assert_eq!(
        DeclaredChange::new(ChangeKind::Rust, []),
        Err(MalformedChange::Empty)
    );
    assert_eq!(
        DeclaredChange::new(
            ChangeKind::Rust,
            [
                (
                    path(REPLICA),
                    FileEdit::Create {
                        content: Vec::new()
                    }
                ),
                (path(REPLICA), FileEdit::Create { content: vec![1] }),
            ],
        ),
        Err(MalformedChange::DuplicatePath {
            path: path(REPLICA)
        })
    );
}

/// No refusal and no Debug form renders file content.
#[test]
fn pr20_impl02_negative_refusals_do_not_render_file_content() {
    let secret = b"SECRET-CONTENT-7f3a";
    let change = DeclaredChange::new(
        ChangeKind::Rust,
        [(
            path(REPLICA),
            FileEdit::Create {
                content: secret.to_vec(),
            },
        )],
    )
    .unwrap();
    let refusal = seal(std::slice::from_ref(&change)).unwrap_err();
    let sealed = seal(&[ack_after_sync_change()]).unwrap();
    for rendering in [
        format!("{refusal}"),
        format!("{refusal:?}"),
        format!("{change:?}"),
        format!("{sealed:?}"),
    ] {
        assert!(!rendering.contains("SECRET-CONTENT"), "{rendering}");
        assert!(!rendering.contains("storage.sync"), "{rendering}");
    }
}

// --- boundary ----------------------------------------------------------------------

/// The patch identity moves with every byte of a change: each single-bit flip of the
/// new content, of a path segment, and of the `before` handle, and a different kind,
/// gives a distinct change digest; every content and path flip also gives a distinct
/// candidate. A kind does not reach the candidate's content, so only the digest moves
/// with it.
#[test]
fn pr20_impl02_boundary_identity_changes_with_any_change_byte() {
    let reference = ack_after_sync_change();
    let reference_sealed = seal(std::slice::from_ref(&reference)).unwrap();
    let mut digests = BTreeSet::from([reference.recorded::<Blake3Hasher>().digest().to_owned()]);
    let mut candidates = BTreeSet::from([reference_sealed.identity().clone()]);
    let before = leaf(&base_content(), REPLICA);

    // Every bit of the new content.
    let after = REPLICA_AFTER.as_bytes();
    for index in 0..after.len() {
        for bit in 0..8 {
            let mut bytes = after.to_vec();
            bytes[index] ^= 1 << bit;
            let change = DeclaredChange::new(
                ChangeKind::Rust,
                [(
                    path(REPLICA),
                    FileEdit::Replace {
                        before: before.clone(),
                        content: bytes,
                    },
                )],
            )
            .unwrap();
            assert!(
                digests.insert(change.recorded::<Blake3Hasher>().digest().to_owned()),
                "content byte {index} bit {bit}: digest did not move"
            );
            let sealed = seal(&[change]).unwrap();
            assert!(
                candidates.insert(sealed.identity().clone()),
                "content byte {index} bit {bit}: candidate did not move"
            );
        }
    }
    // One byte appended, one removed.
    for bytes in [[after, b"\n"].concat(), after[..after.len() - 1].to_vec()] {
        let change = DeclaredChange::new(
            ChangeKind::Rust,
            [(
                path(REPLICA),
                FileEdit::Replace {
                    before: before.clone(),
                    content: bytes,
                },
            )],
        )
        .unwrap();
        assert!(digests.insert(change.recorded::<Blake3Hasher>().digest().to_owned()));
        assert!(candidates.insert(seal(&[change]).unwrap().identity().clone()));
    }

    // Every byte of the path, as a create of a new file at the flipped name.
    let created = "src/replica2.rs";
    for index in 0..created.len() {
        let mut name = created.as_bytes().to_vec();
        // Toggle between two admissible letters so every flip is still a legal path.
        name[index] = if name[index] == b'q' { b'r' } else { b'q' };
        let name = String::from_utf8(name).unwrap();
        let change = DeclaredChange::new(
            ChangeKind::Rust,
            [(
                path(&name),
                FileEdit::Create {
                    content: REPLICA_AFTER.as_bytes().to_vec(),
                },
            )],
        )
        .unwrap();
        assert!(
            digests.insert(change.recorded::<Blake3Hasher>().digest().to_owned()),
            "path byte {index}: digest did not move"
        );
        let sealed = seal(&[change]).expect("a create of a new name applies");
        assert!(
            candidates.insert(sealed.identity().clone()),
            "path byte {index}: candidate did not move"
        );
    }

    // Every byte of the `before` handle's identity part: a digest move, and a refusal.
    let token = before.as_str();
    for index in "ws_".len()..token.len() {
        let mut flipped = token.as_bytes().to_vec();
        flipped[index] = if flipped[index] == b'a' { b'b' } else { b'a' };
        let flipped = SnapshotId::new(std::str::from_utf8(&flipped).unwrap()).unwrap();
        let change = DeclaredChange::new(
            ChangeKind::Rust,
            [(
                path(REPLICA),
                FileEdit::Replace {
                    before: flipped,
                    content: REPLICA_AFTER.as_bytes().to_vec(),
                },
            )],
        )
        .unwrap();
        assert!(
            digests.insert(change.recorded::<Blake3Hasher>().digest().to_owned()),
            "before byte {index}: digest did not move"
        );
        assert_eq!(
            seal(&[change]),
            Err(PatchRefusal::StaleBefore {
                path: path(REPLICA)
            })
        );
    }

    // Every other kind: the digest moves, the candidate does not.
    for kind in ChangeKind::ALL
        .into_iter()
        .filter(|kind| *kind != ChangeKind::Rust)
    {
        let change = replace(kind, REPLICA, REPLICA_AFTER.as_bytes());
        assert!(
            digests.insert(change.recorded::<Blake3Hasher>().digest().to_owned()),
            "{}: digest did not move",
            kind.token()
        );
        assert_eq!(
            seal(&[change]).unwrap().identity(),
            reference_sealed.identity()
        );
    }

    // Create, Replace and Delete of one path with one content are three records.
    let tags: BTreeSet<Vec<u8>> = [
        FileEdit::Create {
            content: Vec::new(),
        },
        FileEdit::Replace {
            before: before.clone(),
            content: Vec::new(),
        },
        FileEdit::Delete {
            before: before.clone(),
        },
    ]
    .into_iter()
    .map(|edit| {
        DeclaredChange::new(ChangeKind::Rust, [(path(REPLICA), edit)])
            .unwrap()
            .canonical_bytes()
    })
    .collect();
    assert_eq!(tags.len(), 3);

    // The hasher is part of the identity's name: one lineage, one algorithm.
    let placeholder = SealedCandidate::<Fnv1aPlaceholder>::seal(
        &SnapshotId::new(
            &continuum_workspace::snapshot::Snapshot::build(
                &base_content(),
                &continuum_repair::patch::HashedIdentifier::<Fnv1aPlaceholder>::new(),
            )
            .unwrap()
            .identity()
            .to_string(),
        )
        .unwrap(),
        &base_content(),
        &[DeclaredChange::new(
            ChangeKind::Rust,
            [(
                path("src/new.rs"),
                FileEdit::Create {
                    content: Vec::new(),
                },
            )],
        )
        .unwrap()],
    )
    .unwrap();
    assert!(
        placeholder.changes()[0]
            .digest()
            .starts_with("placeholder-fnv1a-256:")
    );
    // A BLAKE3 base handle is not the placeholder lineage's base.
    assert!(matches!(
        SealedCandidate::<Fnv1aPlaceholder>::seal(&base_id(), &base_content(), &[]),
        Err(PatchRefusal::BaseMismatch { .. })
    ));
}

// --- differential ------------------------------------------------------------------

/// The change digest equals an independent construction of the record grammar written
/// field by field in this test and hashed with `continuum_value`'s BLAKE3, and the
/// candidate identity equals `continuum-workspace`'s own snapshot of the repaired content
/// built with no repair code at all.
#[test]
fn pr20_impl02_differential_identities_match_independent_constructions() {
    fn bytes(out: &mut Vec<u8>, value: &[u8]) {
        out.extend_from_slice(&(value.len() as u64).to_be_bytes());
        out.extend_from_slice(value);
    }
    let before = leaf(&base_content(), REPLICA);
    let mut record = Vec::new();
    bytes(&mut record, b"continuum-repair/change");
    record.push(1);
    bytes(&mut record, b"rust");
    record.extend_from_slice(&1u64.to_be_bytes());
    record.extend_from_slice(&2u64.to_be_bytes());
    bytes(&mut record, b"src");
    bytes(&mut record, b"replica.rs");
    record.push(0x01);
    bytes(&mut record, before.as_str().as_bytes());
    bytes(&mut record, REPLICA_AFTER.as_bytes());

    let change = ack_after_sync_change();
    assert_eq!(change.canonical_bytes(), record);
    assert_eq!(
        change.recorded::<Blake3Hasher>().digest(),
        format!("blake3-256:{}", Blake3Hasher::hash(&record).to_token())
    );

    // The candidate: the daemon's production identity composition, restated here as a
    // plain `ContentIdentifier`, over the repaired content.
    struct Blake3Identity;
    impl continuum_workspace::publication::ContentIdentifier for Blake3Identity {
        fn identify(
            &self,
            class: continuum_workspace::artifact_path::ArtifactClass,
            content: &[u8],
        ) -> Result<
            continuum_workspace::artifact_path::ArtifactHandle,
            continuum_workspace::publication::IdentityUnavailable,
        > {
            continuum_workspace::artifact_path::ArtifactHandle::new(
                class,
                &Blake3Hasher::hash(content).to_token(),
            )
            .map_err(|_| continuum_workspace::publication::IdentityUnavailable)
        }
    }
    let independent =
        continuum_workspace::snapshot::Snapshot::build(&repaired_content(), &Blake3Identity)
            .unwrap();
    let sealed = seal(&[change]).unwrap();
    assert_eq!(
        sealed.identity().as_str(),
        independent.identity().to_string()
    );
    assert_eq!(sealed.snapshot(), &independent);

    // Seal is a function of its inputs: the base content's insertion order is invisible.
    let mut shuffled = WorkspaceContent::new();
    for (name, text) in [
        (REPLICA, REPLICA_BEFORE),
        (MANIFEST_PATH, MANIFEST),
        (MODEL_PATH, MODEL),
    ] {
        shuffled
            .insert(path(name), text.as_bytes().to_vec())
            .unwrap();
    }
    assert_eq!(
        Sealed::seal(&base_id(), &shuffled, &[ack_after_sync_change()]).unwrap(),
        sealed
    );
}
