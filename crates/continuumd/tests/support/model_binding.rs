#![allow(dead_code)]

//! A sealed snapshot whose model the daemon holds, for the certificate lane's model
//! binding (bn-3hk4v).
//!
//! `evidence.verify` promotes a wire-epoch-2 finite-closure certificate only when the model
//! the certificate carries is the model the daemon holds for the snapshot the node derives
//! from. So every test that expects `validated` on the certificate lane needs a sealed
//! snapshot, a registered model, and a certificate node that names the snapshot in its
//! `provenance.inputs`. This module builds the first two through the daemon's own
//! operations: the snapshot is created and sealed by `workspace.create`, and the model is
//! registered in the catalog out of band, which is the only registration surface this
//! daemon has (`daemon::verification`, "What `verification.start` can honestly construct").
//!
//! The governing intent is filed directly as an accepted registry record. The acceptance
//! chain is not what these tests are about, and `daemon_task_operations.rs` covers it.

use continuum_engine_reference::model::Model;
use continuum_intent::contract::IntentContract;
use continuum_workspace::snapshot::WorkspacePath;
use continuumd::daemon::family::{Arguments, Payload};
use continuumd::daemon::identity::{Blake3Identity, intent_to_wire};
use continuumd::daemon::state::{Acceptance, IntentRecord, RegistryStatus};
use continuumd::daemon::verification::model_source;
use continuumd::daemon::{Daemon, OperationRequest};
use continuumd::protocol::envelope::RequestEnvelope;
use continuumd::protocol::operations::workspace::WorkspaceCreateRequest;
use continuumd::protocol::scalar::{EpochIdentity, IntentHandle, WorkspaceHandle};
use continuumd::protocol::shared::{SnapshotComponents, SnapshotEpochs};
use continuumd::protocol::spec::Optional;
use continuumd::protocol::vocabulary::ResultStatus;

/// The TV-009 port's model source, verbatim: the bytes a Die Hard snapshot carries.
pub const DIE_HARD_MODULE: &str =
    include_str!("../../../../notes/plan/corpus/tla-examples/ports/TV-009/DieHard.ctm");

/// Where the Die Hard module sits inside a snapshot.
pub const DIE_HARD_PATH: &str = "DieHard.ctm";

/// The Die Hard Intent Contract, as `continuum-intent`'s own suites use it.
const DIE_HARD_CONTRACT: &str =
    include_str!("../../../continuum-intent/tests/fixtures/die-hard-contract.json");

/// File the Die Hard contract as an accepted registry head, and return its handle.
fn accepted_intent(daemon: &mut Daemon) -> IntentHandle {
    let contract = IntentContract::decode(DIE_HARD_CONTRACT.trim_end().as_bytes())
        .expect("the fixture decodes");
    let stored = continuum_workspace::publication::ContentIdentifier::identify(
        &Blake3Identity,
        continuum_workspace::artifact_path::ArtifactClass::IntentContract,
        &contract.identity_preimage_bytes(),
    )
    .expect("blake3 names every input");
    let intent = intent_to_wire(&stored).expect("an `in_` handle");
    daemon.state_mut().put_intent(
        intent.clone(),
        IntentRecord {
            contract,
            status: RegistryStatus::Accepted,
            supersedes: None,
            superseded_by: None,
            acceptance: Some(Acceptance {
                accepted_by: "human:steward".to_owned(),
                signature: "sig-model-binding-fixture".to_owned(),
                timestamp: "2026-08-01T00:00:00.000Z".to_owned(),
                audit_record: "audit-model-binding-fixture".to_owned(),
                chain: Vec::new(),
            }),
        },
    );
    intent
}

/// Create and seal a snapshot that carries one CML module, `path` holding `module`, and
/// register `model` (when given) as that module set's elaboration.
///
/// `envelope` is the caller's `workspace.create` envelope: the operation name, a
/// capability that admits it, and an idempotency key. The snapshot handle comes back from
/// the operation, so the test names the snapshot the daemon named.
pub fn sealed_snapshot(
    daemon: &mut Daemon,
    envelope: RequestEnvelope,
    path: &str,
    module: &[u8],
    model: Option<Model>,
) -> WorkspaceHandle {
    let intent = accepted_intent(daemon);
    let file = daemon
        .state_mut()
        .stage(
            &Blake3Identity,
            WorkspacePath::new(path).expect("a workspace path"),
            module.to_vec(),
        )
        .expect("staging names its content");
    if let Some(model) = model {
        daemon.state_mut().models_mut().register(
            model_source(&Blake3Identity, [(path, module)]).expect("blake3 names the module set"),
            model,
        );
    }
    let epoch = |token: &str| EpochIdentity::new(token).expect("a well-formed epoch identity");
    let created = daemon.dispatch(&OperationRequest {
        envelope,
        arguments: Arguments::WorkspaceCreate(WorkspaceCreateRequest {
            components: SnapshotComponents {
                files: vec![file],
                cml_modules: Vec::new(),
                rust_extraction: Vec::new(),
                domain_packs: Vec::new(),
                dependencies: Vec::new(),
                epochs: SnapshotEpochs {
                    semantic: epoch("semantic-1"),
                    proof: epoch("proof-1"),
                    toolchain: Optional::Absent,
                },
                intent,
                correspondence: Vec::new(),
                proof_environment: Vec::new(),
                configuration: Vec::new(),
                file_components: Optional::Absent,
            },
            overlay: Optional::Absent,
            seal: Optional::Present(true),
        }),
    });
    assert_eq!(
        created.envelope.status,
        ResultStatus::Ok,
        "{:?}",
        created.envelope.error
    );
    match &created.payload {
        Payload::WorkspaceCreate(response) => response.snapshot.clone(),
        other => panic!("expected a workspace.create payload, got {other:?}"),
    }
}

/// A sealed Die Hard snapshot with the Die Hard model registered for it.
pub fn die_hard_snapshot(daemon: &mut Daemon, envelope: RequestEnvelope) -> WorkspaceHandle {
    sealed_snapshot(
        daemon,
        envelope,
        DIE_HARD_PATH,
        DIE_HARD_MODULE.as_bytes(),
        Some(continuum_engine_reference::diehard::model().expect("the port builds")),
    )
}
