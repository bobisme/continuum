//! The `workspace` family: `create`, `fork`, `diff`, `seal`.
//!
//! # What each operation is wired to
//!
//! Nothing here re-implements the snapshot model. Every operation is a translation between
//! the wire's request body and `continuum-workspace`'s landed machinery, plus the mapping
//! from that crate's typed refusals to the IDL's error codes.
//!
//! | Operation | Landed machinery |
//! |---|---|
//! | `workspace.create` | [`WorkspaceContent`] + [`Snapshot::build`] + [`Overlay::derive`] + [`WorkspaceDescriptorBuilder`], then [`SealedWorkspace::seal`] when `seal` is requested |
//! | `workspace.fork` | [`staleness::advance_current`] — the compare-and-set advance — then [`rebind_components`] |
//! | `workspace.seal` | [`staleness::seal_current`] |
//! | `workspace.diff` | none: see "The diff lane" below |
//!
//! # Lineages, and where `StaleSnapshot` comes from
//!
//! `workspace.create` opens a lineage at the snapshot it created; `workspace.fork` advances
//! that lineage; `workspace.seal` publishes against it. Both of the latter go through the
//! *guarded* form, which is the one the landed crate wrote for this wire contract:
//!
//! > This is the compare-and-set spelling for a caller that computed `head` against a
//! > *remembered* identity, `expected_head`, and wants the step refused if another advance
//! > landed first — the crate-level shape of "the named snapshot is not current"
//! > (RFC 0026's `StaleSnapshot`, `rule errors.common`).
//! >
//! > — `continuum_workspace::staleness::advance_to_current`
//!
//! So forking twice from one base refuses the second with `StaleSnapshot`, and sealing a
//! superseded descriptor refuses with the same code. Neither refusal spends the work first.
//!
//! # The two lineage answers, and the one that must not be distinguishable
//!
//! [`staleness::check_current`] returns two distinct refusals, and INV-008 is why they are
//! distinct: `Stale` names an identity the lineage really held, and `Unknown` names one it
//! cannot place at all. They map to *different* wire codes, and deliberately:
//!
//! - `LineageError::Stale` → [`ErrorCode::StaleSnapshot`]. The caller is in scope for the
//!   snapshot, the protocol declares the code for exactly this, and the recovery is to
//!   re-derive against the current head.
//! - `LineageError::Unknown` → [`ErrorCode::CapabilityDenied`], byte-identical with every
//!   other denial. There is no `UnknownSnapshot` code on the wire, and inventing a
//!   distinguishable not-found is precisely the existence oracle RFC 0027 X2 forbids: "a
//!   read of an artifact that does not exist and a read of one that exists but is out of
//!   scope return **byte-identical** envelopes".
//!
//! The same reading governs every handle this family resolves: a workspace the daemon does
//! not hold is [`Fault::denied`], not a not-found.
//!
//! # The diff lane
//!
//! `workspace.diff` returns [`ErrorCode::UnsupportedSemanticFeature`] — the code its own
//! `errors` clause declares — and does not compute a partial answer. Its response requires
//! `summary`, "the diff artifact, inline, in the form of `schemas/semantic-diff.schema.json`",
//! and that schema requires `intent_changes`, `semantic_changes`, `impact`, and `policy`:
//! RFC 0031's classification, which `crates/continuum-semantic-diff` has not shipped.
//!
//! > Operations registered in plan §10.2 ahead of their producing subsystem […] MUST fail
//! > with the typed `UnsupportedSemanticFeature` rather than degrading, guessing, or
//! > returning an empty success.
//! >
//! > — `rule errors.unsupported_surface`
//!
//! A file-level answer dressed as a semantic diff would be the "degrading" that rule names.
//! The same reasoning applies to the components `SnapshotComponents` declares that
//! `continuum-workspace` states are out of its scope — `cml_modules`, `rust_extraction`,
//! `domain_packs`, `correspondence`, `proof_environment` — each of which is refused when
//! non-empty rather than silently dropped.
//!
//! [`WorkspaceContent`]: continuum_workspace::snapshot::WorkspaceContent
//! [`Snapshot::build`]: continuum_workspace::snapshot::Snapshot::build
//! [`Overlay::derive`]: continuum_workspace::overlay::Overlay::derive
//! [`WorkspaceDescriptorBuilder`]: continuum_workspace::components::WorkspaceDescriptorBuilder
//! [`SealedWorkspace::seal`]: continuum_workspace::seal::SealedWorkspace::seal
//! [`rebind_components`]: continuum_workspace::seal::rebind_components
//! [`staleness::check_current`]: continuum_workspace::staleness::check_current

use continuum_workspace::artifact_path::ArtifactClass;
use continuum_workspace::components::{WorkspaceDescriptor, WorkspaceDescriptorBuilder};
use continuum_workspace::lineage::{Fork, ForkName};
use continuum_workspace::overlay::Overlay;
use continuum_workspace::publication::{PublishRefusal, ReferenceStore};
use continuum_workspace::seal::{SealError, SealedWorkspace, rebind_components};
use continuum_workspace::snapshot::{Snapshot, WorkspaceContent, WorkspacePath};
use continuum_workspace::staleness::{
    GuardedAdvanceError, GuardedSealError, LineageError, advance_current, seal_current,
};

use super::family::{Arguments, Call, Effect, Fault, OperationFamily, Payload, ScopeClaim};
use super::state::{DaemonState, RegistryStatus, WorkspaceRecord};
use super::{Services, identity};
use crate::protocol::envelope::{ArtifactRef, StructuralVerdictValue, Verdict};
use crate::protocol::operations::workspace::{
    WorkspaceCreateRequest, WorkspaceCreateResponse, WorkspaceForkRequest, WorkspaceForkResponse,
    WorkspaceSealRequest, WorkspaceSealResponse,
};
use crate::protocol::scalar::{ArtifactHandle, Commitment, WorkspaceHandle};
use crate::protocol::shared::SnapshotComponents;
use crate::protocol::spec::{Nullable, Optional};
use crate::protocol::vocabulary::{ErrorCode, StructuralOutcome};

/// The `workspace` namespace's four operations.
#[derive(Debug, Clone, Copy, Default)]
pub struct WorkspaceFamily;

/// Every `(operation, code)` pair this family can answer with.
///
/// A data table rather than a comment, so `tests/daemon_operations.rs` can hold every one of
/// them to `rule errors.common` ∪ the operation's `errors` clause instead of trusting that
/// the handlers stayed inside it.
pub const FAULTS: &[(&str, ErrorCode)] = &[
    ("workspace.create", ErrorCode::CapabilityDenied),
    ("workspace.create", ErrorCode::AcceptanceChainInvalid),
    ("workspace.create", ErrorCode::EpochUnsupported),
    ("workspace.create", ErrorCode::UnsupportedSemanticFeature),
    ("workspace.create", ErrorCode::MalformedRequest),
    ("workspace.create", ErrorCode::PublicationAborted),
    ("workspace.fork", ErrorCode::CapabilityDenied),
    ("workspace.fork", ErrorCode::StaleSnapshot),
    ("workspace.fork", ErrorCode::UnsupportedSemanticFeature),
    ("workspace.fork", ErrorCode::MalformedRequest),
    ("workspace.fork", ErrorCode::PublicationAborted),
    ("workspace.diff", ErrorCode::UnsupportedSemanticFeature),
    ("workspace.seal", ErrorCode::CapabilityDenied),
    ("workspace.seal", ErrorCode::StaleSnapshot),
    ("workspace.seal", ErrorCode::PublicationAborted),
];

impl OperationFamily for WorkspaceFamily {
    fn namespace(&self) -> &'static str {
        "workspace"
    }

    fn scope(&self, arguments: &Arguments) -> ScopeClaim {
        let workspace = ArtifactClass::WorkspaceSnapshot.token();
        match arguments {
            Arguments::WorkspaceCreate(request) => ScopeClaim {
                snapshots: Vec::new(),
                intents: vec![request.components.intent.clone()],
                classes: vec![workspace, ArtifactClass::IntentContract.token()],
            },
            Arguments::WorkspaceFork(request) => ScopeClaim {
                snapshots: vec![request.base.clone()],
                intents: Vec::new(),
                classes: vec![workspace],
            },
            Arguments::WorkspaceDiff(request) => ScopeClaim {
                snapshots: vec![request.before.clone(), request.after.clone()],
                intents: Vec::new(),
                classes: vec![workspace, ArtifactClass::Diff.token()],
            },
            Arguments::WorkspaceSeal(request) => ScopeClaim {
                snapshots: vec![request.snapshot.clone()],
                intents: Vec::new(),
                classes: vec![workspace],
            },
            _ => ScopeClaim::default(),
        }
    }

    fn handle(
        &self,
        call: &Call<'_>,
        state: &mut DaemonState,
        services: &Services,
        store: &ReferenceStore,
    ) -> Result<Effect, Fault> {
        match call.arguments {
            Arguments::WorkspaceCreate(request) => create(call, request, state, services, store),
            Arguments::WorkspaceFork(request) => fork(request, state, services),
            Arguments::WorkspaceDiff(_) => Err(Fault::new(
                ErrorCode::UnsupportedSemanticFeature,
                "the RFC 0031 diff lane this operation's `summary` artifact comes from has \
                 not shipped",
            )),
            Arguments::WorkspaceSeal(request) => seal(call, request, state, store),
            // Unreachable: the dispatcher checked shape agreement against the registry
            // before routing. A typed refusal rather than an `unreachable!`, because a
            // daemon does not abort on its own invariant.
            _ => Err(Fault::new(
                ErrorCode::MalformedRequest,
                "the request body is not the shape this operation declares",
            )),
        }
    }
}

fn create(
    call: &Call<'_>,
    request: &WorkspaceCreateRequest,
    state: &mut DaemonState,
    services: &Services,
    store: &ReferenceStore,
) -> Result<Effect, Fault> {
    // The governing Intent Contract, by identity only (plan §4.2, INV-001). A snapshot
    // bound to a proposal would be a snapshot whose intent can still change, which is what
    // INV-001 exists to prevent, so only an accepted contract governs. The absent case is a
    // denial rather than a not-found (X2).
    let intent = state
        .intent(&request.components.intent)
        .ok_or_else(Fault::denied)?;
    if intent.status != RegistryStatus::Accepted {
        return Err(Fault::new(
            ErrorCode::AcceptanceChainInvalid,
            "the governing intent is not an accepted registry record",
        ));
    }

    check_epochs(&request.components, services)?;
    check_unsupported_components(&request.components)?;

    // Resolve the staged content before touching any state, so a request naming content the
    // daemon does not hold leaves nothing behind.
    let mut content = WorkspaceContent::new();
    for commitment in &request.components.files {
        let staged = state.staged(commitment).ok_or_else(Fault::denied)?;
        content
            .insert(staged.path.clone(), staged.content.clone())
            .map_err(|_| {
                Fault::new(
                    ErrorCode::MalformedRequest,
                    "the named file components do not describe a tree",
                )
            })?;
    }
    let dependencies = single_component(&request.components.dependencies, state)?;
    let configuration = single_component(&request.components.configuration, state)?;

    let mut snapshot = Snapshot::build(&content, services.identifier()).map_err(|_| {
        Fault::new(
            ErrorCode::PublicationAborted,
            "no content identity could be derived for the snapshot",
        )
    })?;
    if let Optional::Present(overlays) = &request.overlay {
        let overlay = build_overlay(overlays)?;
        snapshot = overlay
            .derive(&snapshot, services.identifier())
            .map_err(|_| {
                Fault::new(
                    ErrorCode::MalformedRequest,
                    "the overlay does not apply to the named components",
                )
            })?
            .into_snapshot();
    }

    let mut builder = WorkspaceDescriptorBuilder::default();
    if let Some(bytes) = dependencies {
        builder = builder.dependencies(bytes);
    }
    if let Some(bytes) = configuration {
        builder = builder.configuration(bytes);
    }
    let descriptor = builder
        .build(snapshot, services.identifier())
        .map_err(|_| {
            Fault::new(
                ErrorCode::PublicationAborted,
                "no content identity could be derived for a workspace component",
            )
        })?;

    let handle = wire_handle(&descriptor)?;
    let lineage = lineage_name(&handle)?;
    let sealed = if matches!(request.seal, Optional::Present(true)) {
        publish(call, &descriptor, store)?;
        true
    } else {
        false
    };

    state.put_lineage(Fork::diverge(lineage.clone(), descriptor.source()));
    state.put_workspace(
        handle.clone(),
        WorkspaceRecord {
            descriptor,
            intent: request.components.intent.clone(),
            lineage,
            sealed,
        },
    );

    Ok(Effect::new(
        Payload::WorkspaceCreate(WorkspaceCreateResponse {
            snapshot: handle.clone(),
            sealed,
            diagnostics: Vec::new(),
        }),
        structural(StructuralOutcome::Created),
    )
    .with_artifacts(vec![artifact(&handle)?]))
}

fn fork(
    request: &WorkspaceForkRequest,
    state: &mut DaemonState,
    services: &Services,
) -> Result<Effect, Fault> {
    if request
        .patches
        .value()
        .is_some_and(|patches| !patches.is_empty())
    {
        return Err(Fault::new(
            ErrorCode::UnsupportedSemanticFeature,
            "applying patch content identities to a base is not served by this daemon",
        ));
    }
    let base = state.workspace(&request.base).ok_or_else(Fault::denied)?;
    let expected_head = base.descriptor.source().identity().clone();
    let intent = base.intent.clone();
    let lineage_name = base.lineage.clone();
    let rebind = rebind_components(&base.descriptor);
    let lineage = state
        .lineage(&lineage_name)
        .ok_or_else(Fault::denied)?
        .clone();

    let overlay = match &request.overlay {
        Optional::Present(overlays) => build_overlay(overlays)?,
        Optional::Absent => Overlay::new(),
    };
    let advanced = advance_current(&lineage, &expected_head, &overlay, services.identifier())
        .map_err(|error| match error {
            GuardedAdvanceError::Lineage(lineage) => lineage_fault(&lineage),
            GuardedAdvanceError::Overlay(_) => Fault::new(
                ErrorCode::MalformedRequest,
                "the overlay does not apply to the base snapshot",
            ),
        })?;

    let descriptor = rebind
        .build(advanced.head().clone(), services.identifier())
        .map_err(|_| {
            Fault::new(
                ErrorCode::PublicationAborted,
                "no content identity could be derived for a workspace component",
            )
        })?;
    let handle = wire_handle(&descriptor)?;

    state.put_lineage(advanced);
    state.put_workspace(
        handle.clone(),
        WorkspaceRecord {
            descriptor,
            intent: intent.clone(),
            lineage: lineage_name,
            sealed: false,
        },
    );

    Ok(Effect::new(
        Payload::WorkspaceFork(WorkspaceForkResponse {
            snapshot: handle.clone(),
            intent,
            // "Pre-computed diff against the base, when available without evaluation" —
            // and it is not: a `diff_*` handle names an RFC 0031 artifact this daemon
            // cannot build. `optional` is the field's declared presence, and absent is the
            // truthful reading of "not available".
            pre_diff: Optional::Absent,
            diagnostics: Vec::new(),
        }),
        structural(StructuralOutcome::Created),
    )
    .with_artifacts(vec![artifact(&handle)?]))
}

fn seal(
    call: &Call<'_>,
    request: &WorkspaceSealRequest,
    state: &mut DaemonState,
    store: &ReferenceStore,
) -> Result<Effect, Fault> {
    let record = state
        .workspace(&request.snapshot)
        .ok_or_else(Fault::denied)?;
    let descriptor = record.descriptor.clone();
    let lineage = state
        .lineage(&record.lineage)
        .ok_or_else(Fault::denied)?
        .clone();
    let token =
        identity::capability_to_store(&call.envelope.capability).map_err(|_| Fault::denied())?;

    let sealed =
        seal_current(descriptor, &lineage, store, &token).map_err(|error| match error {
            GuardedSealError::Lineage(lineage) => lineage_fault(&lineage),
            GuardedSealError::Seal(seal) => seal_fault(&seal),
        })?;

    if let Some(record) = state.workspace_mut(&request.snapshot) {
        record.sealed = true;
    }
    let root = Commitment::new(&sealed.snapshot_identity().to_string());
    Ok(Effect::new(
        Payload::WorkspaceSeal(WorkspaceSealResponse {
            snapshot: request.snapshot.clone(),
            root_digest: root,
        }),
        structural(StructuralOutcome::Sealed),
    )
    .with_artifacts(vec![artifact(&request.snapshot)?]))
}

/// Publish every record a descriptor is made of, mapping the store's refusals to the wire.
fn publish(
    call: &Call<'_>,
    descriptor: &WorkspaceDescriptor,
    store: &ReferenceStore,
) -> Result<(), Fault> {
    let token =
        identity::capability_to_store(&call.envelope.capability).map_err(|_| Fault::denied())?;
    SealedWorkspace::seal(descriptor.clone(), store, &token)
        .map(|_| ())
        .map_err(|error| seal_fault(&error))
}

/// `SnapshotComponents.epochs` against the epochs the daemon serves.
///
/// > A daemon MUST NOT best-effort decode an artifact whose declared epoch it does not
/// > know: it returns `EpochUnsupported`.
/// >
/// > — `rule envelope.unknown_fields`
///
/// A daemon that pins no epoch of its own accepts any declaration, because it has nothing
/// to disagree with; one that pins an epoch refuses a snapshot declaring a different one.
fn check_epochs(components: &SnapshotComponents, services: &Services) -> Result<(), Fault> {
    let declared = [
        (
            services.epochs().semantic.value(),
            &components.epochs.semantic,
        ),
        (services.epochs().proof.value(), &components.epochs.proof),
    ];
    for (served, named) in declared {
        if served.is_some_and(|served| served != named) {
            return Err(Fault::new(
                ErrorCode::EpochUnsupported,
                "the named components declare an epoch this daemon does not serve",
            ));
        }
    }
    Ok(())
}

/// The plan §4.2 components `continuum-workspace` states are outside its scope.
fn check_unsupported_components(components: &SnapshotComponents) -> Result<(), Fault> {
    let out_of_scope = [
        &components.cml_modules,
        &components.rust_extraction,
        &components.domain_packs,
        &components.correspondence,
        &components.proof_environment,
    ];
    if out_of_scope.iter().any(|list| !list.is_empty()) {
        return Err(Fault::new(
            ErrorCode::UnsupportedSemanticFeature,
            "this daemon binds only the file, dependency, and configuration components of \
             a snapshot",
        ));
    }
    Ok(())
}

/// The single blob a one-entry component list names.
///
/// The landed descriptor carries *one* dependency and *one* configuration component, so a
/// list naming several is a typed refusal rather than a silent choice among them.
fn single_component(
    commitments: &[Commitment],
    state: &DaemonState,
) -> Result<Option<Vec<u8>>, Fault> {
    match commitments {
        [] => Ok(None),
        [one] => Ok(Some(
            state.staged(one).ok_or_else(Fault::denied)?.content.clone(),
        )),
        _ => Err(Fault::new(
            ErrorCode::UnsupportedSemanticFeature,
            "a workspace binds at most one dependency and one configuration component",
        )),
    }
}

fn build_overlay(overlays: &[crate::protocol::shared::FileOverlay]) -> Result<Overlay, Fault> {
    let mut overlay = Overlay::new();
    for buffer in overlays {
        let path = WorkspacePath::new(&buffer.path).map_err(|_| {
            Fault::new(
                ErrorCode::MalformedRequest,
                "an overlay names a path this workspace model does not admit",
            )
        })?;
        overlay.write(path, buffer.content.clone()).map_err(|_| {
            Fault::new(
                ErrorCode::MalformedRequest,
                "an overlay names one path twice",
            )
        })?;
    }
    Ok(overlay)
}

/// The two lineage answers, mapped. See this module's documentation for why they differ.
fn lineage_fault(error: &LineageError) -> Fault {
    match error {
        LineageError::Stale(_) => Fault::new(
            ErrorCode::StaleSnapshot,
            "the named snapshot has been superseded in its lineage",
        ),
        LineageError::Unknown(_) => Fault::denied(),
    }
}

fn seal_fault(error: &SealError) -> Fault {
    match error {
        SealError::Refused {
            refusal: PublishRefusal::CapabilityDenied(_),
            ..
        } => Fault::denied(),
        SealError::Refused { .. } | SealError::IdentityDisagreement { .. } => Fault::new(
            ErrorCode::PublicationAborted,
            "the publication aborted; nothing was published and nothing was truncated",
        ),
    }
}

fn wire_handle(descriptor: &WorkspaceDescriptor) -> Result<WorkspaceHandle, Fault> {
    identity::workspace_to_wire(descriptor.identity()).map_err(|_| {
        Fault::new(
            ErrorCode::PublicationAborted,
            "the derived workspace identity is not a well-formed handle",
        )
    })
}

fn lineage_name(handle: &WorkspaceHandle) -> Result<ForkName, Fault> {
    ForkName::new(handle.as_str()).map_err(|_| {
        Fault::new(
            ErrorCode::PublicationAborted,
            "the derived workspace identity is not a printable lineage label",
        )
    })
}

fn artifact(handle: &WorkspaceHandle) -> Result<ArtifactRef, Fault> {
    Ok(ArtifactRef {
        kind: ArtifactClass::WorkspaceSnapshot.token().to_owned(),
        handle: ArtifactHandle::new(handle.as_str()).map_err(|_| {
            Fault::new(
                ErrorCode::PublicationAborted,
                "the derived workspace identity is not a well-formed artifact handle",
            )
        })?,
        commitment: Optional::Absent,
        redacted: Optional::Absent,
    })
}

fn structural(outcome: StructuralOutcome) -> Nullable<Verdict> {
    Nullable::Value(Verdict::Structural(StructuralVerdictValue { outcome }))
}
