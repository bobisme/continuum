//! The replicated register's repair workspace, shared by the PR-20 evidence suites.
//!
//! The base is a small workspace whose replica acknowledges a write before the storage
//! sync; the ack-after-sync repair is one `rust` change that moves the ack after it.
//! Every identity here is derived from content under BLAKE3; none is written by hand.

#![allow(dead_code)]

use continuum_repair::handle::SnapshotId;
use continuum_repair::hypothesis::ChangeKind;
use continuum_repair::patch::{DeclaredChange, FileEdit, HashedIdentifier};
use continuum_value::identity::Blake3Hasher;
use continuum_workspace::snapshot::{Snapshot, WorkspaceContent, WorkspacePath};

/// The replica before the repair: the ack precedes the sync.
pub const REPLICA_BEFORE: &str = "pub fn write(reply: &Reply, storage: &mut Storage, value: u64) -> Result<(), Error> {\n    storage.put(value);\n    reply.send(Ack);\n    storage.sync()?;\n    Ok(())\n}\n";

/// The replica after the repair: the ack follows the sync.
pub const REPLICA_AFTER: &str = "pub fn write(reply: &Reply, storage: &mut Storage, value: u64) -> Result<(), Error> {\n    storage.put(value);\n    storage.sync()?;\n    reply.send(Ack);\n    Ok(())\n}\n";

/// The register model, which the repair does not touch.
pub const MODEL: &str = "model ReplicatedRegister {\n  var stored: Nat\n  var acked: Set[Nat]\n}\n";

/// The manifest, which the repair does not touch.
pub const MANIFEST: &str = "[package]\nname = \"register\"\n";

pub const REPLICA: &str = "src/replica.rs";
pub const MODEL_PATH: &str = "model/register.cml";
pub const MANIFEST_PATH: &str = "Cargo.toml";

pub fn path(text: &str) -> WorkspacePath {
    WorkspacePath::new(text).expect("a workspace path")
}

pub fn content(files: &[(&str, &[u8])]) -> WorkspaceContent {
    let mut content = WorkspaceContent::new();
    for (name, bytes) in files {
        content
            .insert(path(name), bytes.to_vec())
            .expect("distinct workspace paths");
    }
    content
}

/// The register workspace the crashpack was captured on.
pub fn base_content() -> WorkspaceContent {
    content(&[
        (MANIFEST_PATH, MANIFEST.as_bytes()),
        (MODEL_PATH, MODEL.as_bytes()),
        (REPLICA, REPLICA_BEFORE.as_bytes()),
    ])
}

/// The base with the replica repaired: what the candidate must hold.
pub fn repaired_content() -> WorkspaceContent {
    content(&[
        (MANIFEST_PATH, MANIFEST.as_bytes()),
        (MODEL_PATH, MODEL.as_bytes()),
        (REPLICA, REPLICA_AFTER.as_bytes()),
    ])
}

pub fn tree(content: &WorkspaceContent) -> Snapshot {
    Snapshot::build(content, &HashedIdentifier::<Blake3Hasher>::new()).expect("a tree")
}

/// The content identity of `content` under BLAKE3.
pub fn snapshot_id(content: &WorkspaceContent) -> SnapshotId {
    SnapshotId::new(&tree(content).identity().to_string()).expect("a ws_ handle")
}

/// The base snapshot's identity.
pub fn base_id() -> SnapshotId {
    snapshot_id(&base_content())
}

/// The leaf identity of the file at `name` in `content`.
pub fn leaf(content: &WorkspaceContent, name: &str) -> SnapshotId {
    let tree = tree(content);
    let file = tree
        .node(&path(name))
        .and_then(|node| node.as_file())
        .expect("a file");
    SnapshotId::new(&file.identity().to_string()).expect("a ws_ handle")
}

/// Replace the file at `name` in the base with `after`.
pub fn replace(kind: ChangeKind, name: &str, after: &[u8]) -> DeclaredChange {
    DeclaredChange::new(
        kind,
        [(
            path(name),
            FileEdit::Replace {
                before: leaf(&base_content(), name),
                content: after.to_vec(),
            },
        )],
    )
    .expect("a well-formed change")
}

/// The ack-after-sync repair: one `rust` change to the replica.
pub fn ack_after_sync_change() -> DeclaredChange {
    replace(ChangeKind::Rust, REPLICA, REPLICA_AFTER.as_bytes())
}
