//! The `workspace` family: `create`, `create_by_reference`, `fork`, `diff`, `seal`.
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
//! | `workspace.create_by_reference` | [`DaemonState::components`] resolves the reference, then **`create` itself** |
//! | `workspace.fork` | [`staleness::advance_current`] — the compare-and-set advance — then [`rebind_components`] |
//! | `workspace.seal` | [`staleness::seal_current`] |
//! | `workspace.diff` | the held-and-sealed input check only: see "The diff lane" below |
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
//! # Creating a workspace this daemon already holds
//!
//! A `ws_*` handle is the *content identity* of the descriptor, and the lineage is named by
//! that handle, so two `workspace.create` requests naming the same components name the same
//! snapshot **and the same lineage** — by construction, not by coincidence. What the second
//! one does is therefore a design decision, and it is this one:
//!
//! > **An identical create converges. It opens nothing that exists, replaces nothing that
//! > exists, and un-seals nothing.**
//!
//! This is G0-DX-13's disposition for identical creation — "one semantic identity; no lost
//! receipts", the pass condition `continuum-workspace`'s publication layer already meets
//! under 48-thread contention — applied to the daemon's own maps. It is also
//! [`DaemonState::append_evidence`](super::state::DaemonState::append_evidence)'s rule, in
//! the same words that module uses: a second write under a held identity is a *convergence*,
//! not an overwrite and not an error, and the stored value is returned untouched "even when
//! the two appends disagree about everything else".
//!
//! ## What it was before, and what that cost (bn-n1xou, bn-1kp6 DEFECT 1)
//!
//! `create` derived the `ForkName` from the handle and then called `put_lineage`, an
//! unconditional insert, so a second create of identical components **replaced the live
//! `Fork` with a fresh `Fork::diverge` rooted at the original snapshot**. Three consequences
//! were pinned as falsifications, all from that one line:
//!
//! - a superseded snapshot — refused `StaleSnapshot` one request earlier — was served again
//!   by `verification.start` and `task.resume`, which is the DX-03 experiment's own sentence
//!   ("mutate workspace, reuse old handle") coming out the wrong way;
//! - the snapshot that *was* the head became an identity the rewound `Fork` could not place,
//!   so the caller's own mutation was stranded: `CapabilityDenied` on seal and on fork, with
//!   nothing in the refusal saying a lineage had been rewound;
//! - `put_workspace` replaced the record too, so a `seal: false` re-create flipped a sealed
//!   snapshot back to unsealed and *revoked a live continuation* pinned to it.
//!
//! The idempotency ledger was the only guard, and `rule idempotency.replay` honours a key
//! only "for at least the retention window the daemon declares" and scopes keys per actor —
//! so a retry after the window, or a second agent, reached the rewind with no rule broken.
//!
//! ## The `seal` divergence, stated explicitly
//!
//! Convergence leaves one field that two identical creates can legitimately disagree about,
//! because it is not part of the identity they converge on: `seal`. The rule is
//! **monotone** — `sealed` after the create is `held.sealed || request.seal` — and each
//! direction is chosen rather than fallen into:
//!
//! | held | `seal` | result | why |
//! |---|---|---|---|
//! | unsealed | `true` | **sealed**, `sealed: true` | exactly `workspace.seal`'s effect on the same snapshot, through the operation that also asked for it. Nothing is lost by doing it here |
//! | sealed | `true` | stays sealed, `sealed: true` | already true; the publication is convergent in the store (DX-13), so re-publishing names the same records |
//! | sealed | `false` | **stays sealed**, `sealed: true` | `seal: false` is "do not seal it", not "un-seal it". There is no un-seal operation in this protocol, and a seal is what a continuation's staleness predicate reads: letting a create revoke one would let any caller revoke any continuation by re-creating components it can already name |
//! | unsealed | `false` | stays unsealed, `sealed: false` | nothing to do; identical to the first create |
//!
//! Row three is the only one whose *response* differs from the pre-fix daemon's: it reports
//! `sealed: true` where the old code reported `sealed: false` and made it true by making the
//! record false. `sealed` is a `required` field of `WorkspaceCreateResponse` declared as
//! whether the snapshot is sealed, so reporting the state the daemon is actually in is
//! conformance to what the field already says, not a new wire fact. Every other row answers
//! byte-identically with what the same request produced before.
//!
//! The rest of the record — the descriptor, the governing intent, the lineage name — is the
//! held one, untouched. That is `append_evidence`'s "disagree about everything else" clause
//! and it has one concrete consequence worth naming: two creates whose `files` agree but
//! whose `components.intent` differs converge on the *first* intent, because the intent is
//! not in the snapshot's content identity. A re-create cannot re-govern a snapshot that
//! exists; a caller wanting a differently governed workspace is naming a different workspace
//! and has to say so in the content.
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
//! **`daemon::task::resume` and `daemon::verification::start` map `LineageError::Unknown` to
//! `StaleSnapshot` instead** (collapsed with `Stale`), for a snapshot their own
//! `state.workspace(..)` lookup has already resolved — bn-1kp6's DX-03 attack 7 recorded the
//! asymmetry, and bn-10wdo's disposition ratifies both readings, keyed to *why* `check_current`
//! is being asked: this family asks it of a caller-declared identity about to be spent on a
//! **write** (a compare-and-set guard, X2's existence-oracle reasoning applies); the other two
//! ask it of an identity already fully resolved, where the only open question is currency, not
//! existence. See RFC 0026, "An unplaceable lineage identity: one condition, two codes, and why
//! that is a decision", for the full disposition and the rule a future operation follows.
//!
//! # The diff lane
//!
//! `workspace.diff` first consults its two snapshot carriers — each must be held and sealed,
//! RFC 0031's input contract (see [`diff`]) — and then returns
//! [`ErrorCode::UnsupportedSemanticFeature`] — the code its own `errors` clause declares —
//! and does not compute a partial answer. Its response requires
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
use continuum_workspace::publication::{PublishRefusal, Published, ReferenceStore};
use continuum_workspace::seal::{SealError, SealedWorkspace, rebind_components};
use continuum_workspace::snapshot::{Snapshot, WorkspaceContent, WorkspacePath};
use continuum_workspace::staleness::{
    GuardedAdvanceError, GuardedSealError, LineageError, advance_current, seal_current,
};

use super::admission::Derived;
use super::family::{
    Arguments, Call, Effect, Fault, OperationFamily, Payload, RATIONALE_RESEAL_LINEAGE_HEAD,
    RecoveryOffer, ScopeClaim,
};
use super::state::{DaemonState, WorkspaceRecord};
use super::{Services, identity};
use crate::protocol::envelope::{ArtifactRef, StructuralVerdictValue, Verdict};
use crate::protocol::operations::workspace::{
    WorkspaceCreateByReferenceRequest, WorkspaceCreateByReferenceResponse, WorkspaceCreateRequest,
    WorkspaceCreateResponse, WorkspaceDiffRequest, WorkspaceForkRequest, WorkspaceForkResponse,
    WorkspaceSealRequest, WorkspaceSealResponse,
};
use crate::protocol::scalar::{ArtifactHandle, Commitment, WorkspaceHandle};
use crate::protocol::shared::SnapshotComponents;
use crate::protocol::spec::{Nullable, Optional};
use crate::protocol::vocabulary::{ErrorCode, StructuralOutcome};

/// The `workspace` namespace's five operations.
///
/// # The by-reference lane (protocol 3.6, `rule snapshot.by_reference`)
///
/// `workspace.create_by_reference` is not a second creator. It resolves its `components`
/// commitment to the `SnapshotComponents` value this daemon holds, substitutes the
/// request's own `intent` and `epochs` into it, and then calls [`create`] — the same
/// function, on a value of the same type. So the two spellings cannot drift: there is one
/// implementation of what a create *is*, and equal components produce an equal `ws_`
/// handle and a member-for-member equal response whichever spelling named them. The rule
/// requires exactly that ("two spellings of one argument MUST NOT become two
/// behaviours"), and `tests/daemon_operations.rs` holds it as a byte comparison rather
/// than as a claim.
///
/// **Why the two members travel.** `intent` cannot come from the reference because
/// [`OperationFamily::scope`] is computed from the *arguments alone*, before any handler
/// runs and before the state is reachable: admission decides T2 and T3 from that claim, so
/// a governing intent hidden behind a commitment is a scope claim the admission predicate
/// cannot see, and INV-015 is not enforceable against a claim it cannot read. That is a
/// property of this daemon's own architecture and not a preference. `epochs` cannot come
/// from the reference because [`check_epochs`] checks them against the epochs the
/// deployment serves; reading them out of the store would make the daemon check itself,
/// and the same components at two semantic epochs are two snapshots.
///
/// **Where a reference comes from.** [`DaemonState::register_components`], and there is no
/// verb for it: a deployment registers the sets it holds out of band, exactly as it stages
/// the content those sets name, and every accepted inline `workspace.create` registers its
/// own components through the same method. The inline form is the registration. A
/// commitment this daemon does not hold is [`Fault::denied`] — byte identical with every
/// other denial, because there is no `UnknownComponents` code and a distinguishable
/// not-found is the existence oracle RFC 0027 X2 forbids. It is not an oracle in the other
/// direction either, and for the reason [`create`] already gives about `file_components`:
/// a caller reaches this line by naming a commitment, and that commitment's preimage is
/// the component set it would have had to hold to derive it.
///
/// [`create`]: create()
/// [`DaemonState::components`]: super::state::DaemonState::components
/// [`DaemonState::register_components`]: super::state::DaemonState::register_components
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
    ("workspace.create_by_reference", ErrorCode::CapabilityDenied),
    (
        "workspace.create_by_reference",
        ErrorCode::AcceptanceChainInvalid,
    ),
    ("workspace.create_by_reference", ErrorCode::EpochUnsupported),
    (
        "workspace.create_by_reference",
        ErrorCode::UnsupportedSemanticFeature,
    ),
    ("workspace.create_by_reference", ErrorCode::MalformedRequest),
    (
        "workspace.create_by_reference",
        ErrorCode::PublicationAborted,
    ),
    ("workspace.fork", ErrorCode::CapabilityDenied),
    ("workspace.fork", ErrorCode::StaleSnapshot),
    ("workspace.fork", ErrorCode::UnsupportedSemanticFeature),
    ("workspace.fork", ErrorCode::MalformedRequest),
    ("workspace.fork", ErrorCode::PublicationAborted),
    ("workspace.diff", ErrorCode::CapabilityDenied),
    ("workspace.diff", ErrorCode::StaleSnapshot),
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
                instances: Vec::new(),
            },
            Arguments::WorkspaceCreateByReference(request) => ScopeClaim {
                // The intent is read from the *request*, which is why the operation
                // declares it rather than resolving it: this function has no state, and a
                // scope claim admission cannot read is a claim it cannot enforce.
                snapshots: Vec::new(),
                intents: vec![request.intent.clone()],
                classes: vec![workspace, ArtifactClass::IntentContract.token()],
                instances: Vec::new(),
            },
            Arguments::WorkspaceFork(request) => ScopeClaim {
                snapshots: vec![request.base.clone()],
                intents: Vec::new(),
                classes: vec![workspace],
                instances: Vec::new(),
            },
            Arguments::WorkspaceDiff(request) => ScopeClaim {
                snapshots: vec![request.before.clone(), request.after.clone()],
                intents: Vec::new(),
                classes: vec![workspace, ArtifactClass::Diff.token()],
                instances: Vec::new(),
            },
            Arguments::WorkspaceSeal(request) => ScopeClaim {
                snapshots: vec![request.snapshot.clone()],
                intents: Vec::new(),
                classes: vec![workspace],
                instances: Vec::new(),
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
            Arguments::WorkspaceCreateByReference(request) => {
                create_by_reference(call, request, state, services, store)
            }
            Arguments::WorkspaceFork(request) => fork(call, request, state, services),
            Arguments::WorkspaceDiff(request) => diff(call, request, state),
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
    // INV-001 exists to prevent, so only an accepted contract governs (RFC 0037 R2). It
    // must also be the accepted head of its lineage, holding the acceptance block
    // `intent.accept` or `intent.lock` recorded, which is the test `intent.lock` and A2
    // apply (bn-1mgcv). A superseded contract no longer governs: a new snapshot bound to it
    // would do new work under the intent a later acceptance or lock replaced — around the
    // lock that tightened it (plan §5.4) — and `accepted` without its block is partial
    // state, which fails closed. `workspace.fork` is different: RFC 0037 requires it to
    // keep its base's binding by identity. The absent case is a denial rather than a
    // not-found (X2).
    let intent = state
        .intent(&request.components.intent)
        .ok_or_else(Fault::denied)?;
    if !super::intent::accepted_head(intent) {
        return Err(Fault::new(
            ErrorCode::AcceptanceChainInvalid,
            "the governing intent is not the accepted head of its lineage",
        ));
    }

    check_epochs(&request.components, services)?;
    check_unsupported_components(&request.components)?;

    let placement = declared_placement(&request.components)?;

    // Resolve the staged content before touching any state, so a request naming content the
    // daemon does not hold leaves nothing behind.
    let mut content = WorkspaceContent::new();
    for (index, commitment) in request.components.files.iter().enumerate() {
        let staged = state.staged(commitment).ok_or_else(Fault::denied)?;
        if let Some(declared) = placement.and_then(|list| list.get(index)) {
            // `rule snapshot.file_components` — the two lists are one statement, so a
            // disagreement is refused rather than resolved in either list's favour. It is
            // not an existence oracle: the caller reached this line by naming a commitment
            // this daemon holds, and the commitment's preimage is the (path, content)
            // record it would have had to build to name it.
            if declared.path.as_str() != staged.path.to_string() {
                return Err(Fault::new(
                    ErrorCode::MalformedRequest,
                    "a declared file component names a path other than the one its \
                     commitment was derived over",
                ));
            }
        }
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
        .not_retryable()
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
            .not_retryable()
        })?;

    let handle = wire_handle(&descriptor)?;
    // The snapshot handle is a function of the components, so a create may land on a
    // snapshot this daemon already holds and seal it (convergence, below). It is decided by
    // the grant's `snapshots` list first, whether or not it is held, so the answer does not
    // say which (X2), and a snapshot-scoped grant writes no snapshot it does not list
    // (`rule capability.instance_scope`, the derived-handle clause; cr-3hcpn4).
    call.derived(Derived::Snapshot(&handle))?;
    // A create converges on a snapshot this daemon already holds under the same content,
    // and the snapshot identity does not include the intent. A held record governed by
    // another intent is not this caller's to seal or to learn the state of: the create is
    // refused before anything is published (cr-3hcpn4). The residue is stated: the refusal
    // says that identical content is held under another intent, which the caller could
    // only learn by holding that content.
    if let Some(held) = state.workspace(&handle) {
        if held.intent != request.components.intent {
            return Err(Fault::denied());
        }
        call.derived(Derived::Intent(&held.intent))?;
    }
    let lineage = lineage_name(&handle)?;
    let requested_seal = matches!(request.seal, Optional::Present(true));
    let seal = if requested_seal {
        Some(publish(call, &descriptor, &handle, store)?)
    } else {
        None
    };

    // A create is convergent, not destructive. See "Creating a workspace this daemon
    // already holds" in this module's documentation for the whole reading; the three lines
    // are: the lineage is *opened* if absent and otherwise left exactly as it is
    // (`open_lineage` is put-if-absent), the held record is not replaced, and `sealed` is
    // monotone — a create can seal an unsealed snapshot, and no create un-seals one.
    state.open_lineage(Fork::diverge(lineage.clone(), descriptor.source()));
    let sealed = match state.workspace(&handle).map(WorkspaceRecord::sealed) {
        Some(already) => {
            if !already && let Some(seal) = seal {
                if let Some(held) = state.workspace_mut(&handle) {
                    held.seal = Some(seal);
                }
            }
            already || requested_seal
        }
        None => {
            state.put_workspace(
                handle.clone(),
                WorkspaceRecord {
                    descriptor,
                    intent: request.components.intent.clone(),
                    lineage,
                    seal,
                },
            );
            requested_seal
        }
    };

    // The inline form is what registers what the by-reference form may later name
    // (`rule snapshot.by_reference`, protocol 3.6), so there is no registration verb and
    // no second surface. It is deliberately unable to change what this operation answers:
    // the value is recorded after every check has passed, and a deployment whose identity
    // seam cannot name it simply holds no reference to it — the by-reference lane then
    // denies, which is the typed outcome for a set the daemon does not hold. Every byte
    // `workspace.create` sends and receives is what 3.5 sent and received.
    let _ = state.register_components(services.identifier(), request.components.clone());

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

/// The by-reference lane: resolve, substitute the two caller-declared members, delegate.
///
/// Three lines of substance and no fourth, which is the design. Everything a create *is*
/// happens in [`create`], reached with a `SnapshotComponents` value of the same type the
/// inline lane builds from the wire, so `rule snapshot.by_reference`'s "two spellings of
/// one argument MUST NOT become two behaviours" is a property of the call graph rather
/// than a discipline anyone has to keep.
fn create_by_reference(
    call: &Call<'_>,
    request: &WorkspaceCreateByReferenceRequest,
    state: &mut DaemonState,
    services: &Services,
    store: &ReferenceStore,
) -> Result<Effect, Fault> {
    // A commitment this daemon does not hold is a denial and not a not-found (X2).
    let mut components = state
        .components(&request.components)
        .ok_or_else(Fault::denied)?
        .clone();
    // `rule snapshot.by_reference`: the stored value's own `intent` and `epochs` are NOT
    // read on this lane. The request's are what govern — the intent because admission has
    // already decided T2/T3 against exactly this handle (see `scope`), the epochs because
    // `check_epochs` is a check of the caller's declaration against the deployment.
    components.intent = request.intent.clone();
    components.epochs = request.epochs.clone();

    let inline = WorkspaceCreateRequest {
        components,
        // No overlay on this lane: an overlay is content inline again, and the operation
        // does not declare one. `workspace.fork` is where buffers go.
        overlay: Optional::Absent,
        seal: request.seal,
    };
    let effect = create(call, &inline, state, services, store)?;
    // The one thing that differs, and it differs only because the IDL names two anonymous
    // bodies: the response members are equal, field for field, by construction.
    let Payload::WorkspaceCreate(response) = effect.payload else {
        // Unreachable: `create` returns exactly that payload. A typed refusal rather than
        // an `unreachable!`, because a daemon does not abort on its own invariant.
        return Err(Fault::new(
            ErrorCode::PublicationAborted,
            "the create lane produced a body this operation does not declare",
        )
        .not_retryable());
    };
    Ok(Effect::new(
        Payload::WorkspaceCreateByReference(WorkspaceCreateByReferenceResponse {
            snapshot: response.snapshot,
            sealed: response.sealed,
            diagnostics: response.diagnostics,
        }),
        effect.verdict,
    )
    .with_artifacts(effect.artifacts))
}

fn fork(
    call: &Call<'_>,
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
    // The base's intent is not named by the request; the fork records it and reports it.
    // It is decided by the grant before it is read (cr-3hcpn4).
    call.derived(Derived::Intent(&base.intent))?;
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
            GuardedAdvanceError::Lineage(error) => lineage_fault(&error, state, &lineage_name),
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
            .not_retryable()
        })?;
    let handle = wire_handle(&descriptor)?;
    // The new snapshot, decided as a create decides its own, before anything is written.
    call.derived(Derived::Snapshot(&handle))?;

    state.put_lineage(advanced);
    state.put_workspace(
        handle.clone(),
        WorkspaceRecord {
            descriptor,
            intent: intent.clone(),
            lineage: lineage_name,
            seal: None,
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

/// `workspace.diff`: consult both staleness carriers, then answer that the lane has not
/// shipped.
///
/// > Both snapshots MUST be sealed. An unsealed snapshot is rejected (`StaleSnapshot`, RFC
/// > 0026); the diff MUST NOT classify against a mutable input.
/// >
/// > — RFC 0031, "Inputs"
///
/// The two carriers are checked for what RFC 0031 requires of them, and for nothing else:
///
/// - **held** — a handle this daemon does not hold is [`Fault::denied`], as everywhere in
///   this family (X2);
/// - **sealed** — an unsealed input is `StaleSnapshot`, with a [`reseal_current_head`]
///   offer for its lineage;
/// - **not current** — deliberately *not* checked. A diff compares two points in history,
///   and "what changed since the snapshot I hold" is its main use, so a superseded `before`
///   is the ordinary case rather than a stale one. `context.expand` reads a superseded pack
///   for the same reason (RFC 0028).
///
/// Only after both carriers pass does the operation answer `UnsupportedSemanticFeature`:
/// the classification is RFC 0031's, which `crates/continuum-semantic-diff` has not
/// shipped, and `rule errors.unsupported_surface` forbids a degraded answer. The checks
/// before it are not a degraded answer. They are the input contract the shipped lane will
/// have, and they are what makes this operation's staleness behaviour observable today
/// (`tests/gate_g2_04_acceptance.rs`, which found it consulted neither carrier).
fn diff(
    call: &Call<'_>,
    request: &WorkspaceDiffRequest,
    state: &DaemonState,
) -> Result<Effect, Fault> {
    for snapshot in [&request.before, &request.after] {
        let record = state.workspace(snapshot).ok_or_else(Fault::denied)?;
        // The snapshot's intent, decided before its lineage is read for a recovery offer
        // (cr-3hcpn4).
        call.derived(Derived::Intent(&record.intent))?;
        if !record.sealed() {
            return Err(Fault::new(
                ErrorCode::StaleSnapshot,
                "a diff classifies only sealed snapshots",
            )
            .with_recovery(reseal_current_head(state, &record.lineage)));
        }
    }
    Err(Fault::new(
        ErrorCode::UnsupportedSemanticFeature,
        "the RFC 0031 diff lane this operation's `summary` artifact comes from has not \
         shipped",
    ))
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
    // The snapshot's intent is not named by the request, and sealing writes this record and
    // its lineage's recovery offers name the head. It is decided first (cr-3hcpn4).
    call.derived(Derived::Intent(&record.intent))?;
    let descriptor = record.descriptor.clone();
    let lineage = state
        .lineage(&record.lineage)
        .ok_or_else(Fault::denied)?
        .clone();
    let token =
        identity::capability_to_store(&call.envelope.capability).map_err(|_| Fault::denied())?;

    let sealed =
        seal_current(descriptor, &lineage, store, &token).map_err(|error| match error {
            GuardedSealError::Lineage(error) => lineage_fault(&error, state, lineage.name()),
            GuardedSealError::Seal(seal) => seal_fault(&seal),
        })?;

    let seal = sealed
        .root_receipt()
        .and_then(|receipt| Published::attest(receipt, request.snapshot.clone()).ok())
        .ok_or_else(identity_disagreement)?;
    if let Some(record) = state.workspace_mut(&request.snapshot) {
        record.seal = Some(seal);
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
///
/// Returns `handle` tied to the descriptor record's receipt (bn-283p6), which is the only
/// value a [`WorkspaceRecord`] can read as sealed.
fn publish(
    call: &Call<'_>,
    descriptor: &WorkspaceDescriptor,
    handle: &WorkspaceHandle,
    store: &ReferenceStore,
) -> Result<Published<WorkspaceHandle>, Fault> {
    let token =
        identity::capability_to_store(&call.envelope.capability).map_err(|_| Fault::denied())?;
    let sealed = SealedWorkspace::seal(descriptor.clone(), store, &token)
        .map_err(|error| seal_fault(&error))?;
    sealed
        .root_receipt()
        .and_then(|receipt| Published::attest(receipt, handle.clone()).ok())
        .ok_or_else(identity_disagreement)
}

/// The daemon's name for a sealed workspace is not the handle its descriptor record was
/// published under.
///
/// Unreachable while `wire_handle` renders the descriptor identity: the two spellings are
/// one rule. A typed refusal rather than a panic, and deterministic, so not retryable. The
/// records are published and converge on a retry, as for a partial seal (RFC 0026's
/// per-artifact contract), but no workspace record reads as sealed.
fn identity_disagreement() -> Fault {
    Fault::new(
        ErrorCode::PublicationAborted,
        "the sealed descriptor's receipt does not name the workspace handle",
    )
    .not_retryable()
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
/// The declared placement of the `files` components, checked against `files` itself.
///
/// > When `SnapshotComponents.file_components` is present it MUST have the same length as
/// > `files` and its `commitment` values MUST equal `files` element for element, in
/// > order […]. A daemon MUST reject a disagreement with `MalformedRequest` rather than
/// > preferring either list.
/// >
/// > — `rule snapshot.file_components`
///
/// Returning [`None`] is the pre-3.2 lane: the request declares no placement, and the
/// daemon resolves it from what was staged out of band. That lane is not a workaround any
/// more — it is what a 3.0/3.1 client sends, and `rule versioning.compatible_change`
/// requires it to keep working.
fn declared_placement(
    components: &SnapshotComponents,
) -> Result<Option<&Vec<crate::protocol::shared::FileComponent>>, Fault> {
    let Optional::Present(declared) = &components.file_components else {
        return Ok(None);
    };
    let disagreement = Fault::new(
        ErrorCode::MalformedRequest,
        "`file_components` and `files` do not name the same file components in the same \
         order",
    );
    if declared.len() != components.files.len() {
        return Err(disagreement);
    }
    for (component, commitment) in declared.iter().zip(&components.files) {
        if &component.commitment != commitment {
            return Err(disagreement);
        }
    }
    Ok(Some(declared))
}

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
///
/// `LineageError::Stale` carries the head that superseded the named snapshot
/// ([`StaleSnapshot::current`](continuum_workspace::staleness::StaleSnapshot::current)).
/// That value is carried through to the refusal as a [`reseal_at_head`] offer rather than
/// dropped, which is RFC 0027 H8 (bn-27mx7). `Unknown` carries nothing: it is a denial,
/// and a denial that named a head would be the existence oracle X2 forbids.
fn lineage_fault(error: &LineageError, state: &DaemonState, lineage: &ForkName) -> Fault {
    match error {
        LineageError::Stale(stale) => superseded(state, lineage, stale.current()),
        LineageError::Unknown(_) => Fault::denied(),
    }
}

/// The `StaleSnapshot` a *currency* refusal answers: the named snapshot is not its lineage's
/// head, and the refusal names the head that is.
///
/// Shared by every family that asks the currency question — `workspace.fork`,
/// `workspace.seal`, `verification.start`, `task.resume` — so the refusal an agent reads is
/// one shape whichever operation it came from.
pub(super) fn superseded(
    state: &DaemonState,
    lineage: &ForkName,
    current: &continuum_workspace::artifact_path::ArtifactHandle,
) -> Fault {
    Fault::new(
        ErrorCode::StaleSnapshot,
        "the named snapshot has been superseded in its lineage",
    )
    .with_recovery(reseal_at_head(state, lineage, current))
}

/// The recovery a stale or unsealed snapshot has: `workspace.seal` of its lineage's head.
///
/// > **H8 — a stale-handle failure is recoverable and says so.** […] "re-seal", "re-base"
/// > […] is executable rather than described.
/// >
/// > — RFC 0027, "Stale handles and stale capabilities"
///
/// # Why `workspace.seal`, and not a re-issued fork or run
///
/// - It names the head as a `ws_*` handle, which is the one thing the refused caller lacks
///   (findings F1 and F3 of `tests/gate_g2_04_acceptance.rs`).
/// - It is executable as given. Sealing an unsealed head publishes it; sealing a sealed
///   head converges on the records that exist (G0-DX-13). Every snapshot-pinned run needs
///   the head sealed before it is re-issued against it.
/// - Its arguments are a handle and nothing else. A re-issued `workspace.fork` would have
///   to echo the caller's overlay content, and RFC 0027 S1 forbids source content in a
///   `recovery` argument. A re-issued `verification.start` or `task.resume` cannot carry
///   the head at all, because both name their snapshot on the envelope or in the
///   continuation, never in the request body `NextOperation.arguments` pre-fills.
///
/// An empty list when the head maps to no unique `ws_*` handle
/// ([`DaemonState::workspace_at`]): an empty `recovery` is the typed statement that no
/// recovery exists (RFC 0026), and a guessed handle would be worse than none.
pub(super) fn reseal_at_head(
    state: &DaemonState,
    lineage: &ForkName,
    current: &continuum_workspace::artifact_path::ArtifactHandle,
) -> Vec<RecoveryOffer> {
    state
        .workspace_at(lineage, current)
        .map(|head| RecoveryOffer {
            arguments: Arguments::WorkspaceSeal(WorkspaceSealRequest {
                snapshot: head.clone(),
            }),
            rationale: RATIONALE_RESEAL_LINEAGE_HEAD,
        })
        .into_iter()
        .collect()
}

/// [`reseal_at_head`] for the head `lineage` holds now, for a refusal that is about
/// sealedness rather than currency and so has no `LineageError::Stale` in hand.
pub(super) fn reseal_current_head(state: &DaemonState, lineage: &ForkName) -> Vec<RecoveryOffer> {
    state
        .lineage(lineage)
        .map(|fork| reseal_at_head(state, lineage, fork.identity()))
        .unwrap_or_default()
}

/// A seal publishes a composite one record at a time, children first and the root last, so
/// an abort at record *k* can leave the records before *k* published, each complete and
/// receipted. The detail says what is true at operation grain: the workspace root is not
/// published, and no record is partial (RFC 0026 "Atomicity of publication", correction 48;
/// INV-017 per artifact, RFC 0038).
/// The `detail` of every aborted composite publication, retryable or not.
const SEAL_ABORTED: &str = "the composite publication aborted; the workspace root was not \
                            published and nothing was truncated, and records published before \
                            the abort remain published";

fn seal_fault(error: &SealError) -> Fault {
    match error {
        SealError::Refused {
            refusal: PublishRefusal::CapabilityDenied(_),
            ..
        } => Fault::denied(),
        SealError::Refused { .. } => Fault::new(ErrorCode::PublicationAborted, SEAL_ABORTED),
        // The two identity seams disagree on the same bytes, so a retry derives the same
        // disagreement: deterministic, whatever the taxonomy default says.
        SealError::IdentityDisagreement { .. } => {
            Fault::new(ErrorCode::PublicationAborted, SEAL_ABORTED).not_retryable()
        }
    }
}

fn wire_handle(descriptor: &WorkspaceDescriptor) -> Result<WorkspaceHandle, Fault> {
    identity::workspace_to_wire(descriptor.identity()).map_err(|_| {
        Fault::new(
            ErrorCode::PublicationAborted,
            "the derived workspace identity is not a well-formed handle",
        )
        .not_retryable()
    })
}

fn lineage_name(handle: &WorkspaceHandle) -> Result<ForkName, Fault> {
    ForkName::new(handle.as_str()).map_err(|_| {
        Fault::new(
            ErrorCode::PublicationAborted,
            "the derived workspace identity is not a printable lineage label",
        )
        .not_retryable()
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
            .not_retryable()
        })?,
        commitment: Optional::Absent,
        redacted: Optional::Absent,
    })
}

fn structural(outcome: StructuralOutcome) -> Nullable<Verdict> {
    Nullable::Value(Verdict::Structural(StructuralVerdictValue { outcome }))
}
