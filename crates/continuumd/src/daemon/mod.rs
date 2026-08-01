//! The operation layer: request in, result out, over explicit state.
//!
//! # What this module is, and what it deliberately is not
//!
//! This is the daemon's *behaviour*, separated from its transport. A dispatch is a pure
//! function of the request and the daemon's state:
//!
//! - **no `async`, no runtime, no task.** INV-005 and ADR-0003 put the daemon core inside
//!   the deterministic band, and plan §20 gives `continuumd` no async runtime edge. Nothing
//!   here awaits, spawns, or blocks;
//! - **no ambient anything.** No clock (a time reading arrives as [`Services::now`]), no
//!   entropy (capability identities are supplied, never drawn), no filesystem (content
//!   arrives through [`DaemonState::stage`]), no network;
//! - **no codec.** `RequestEnvelope.arguments` and `ResultEnvelope.payload` are `Opaque`,
//!   and RFC 0026 fixes neither the canonical field order of its two encodings nor their
//!   union tagging; the IDL's own open item 3 leaves those payloads without a declared
//!   shape. So this layer takes an already-decoded [`Arguments`] and emits a typed
//!   [`Payload`] beside the envelope, and the transport half of PR 5 supplies the codec.
//!
//! What remains is the half that decides things, and it is testable without I/O.
//!
//! # The dispatch order, and why it is that order
//!
//! [`Daemon::dispatch`] runs eight steps. Every one of them reads registry data — the
//! IDL's own `authority`, `annotations`, and `errors` clauses, transcribed in
//! [`registry::OPERATIONS`](crate::protocol::registry::OPERATIONS) — rather than
//! per-operation code, so a family that lands later inherits all eight without writing any
//! of them.
//!
//! | # | Step | Source of the rule | Failure |
//! |---|---|---|---|
//! | 1 | the request names the connection's negotiated version | RFC 0026, "Version window and negotiation" | `ProtocolVersionUnsupported` |
//! | 2 | the operation is one of the registry's 72 | plan §10.2 registry | `MalformedRequest` |
//! | 3 | the arguments are that operation's declared shape | the IDL's `request` body | `MalformedRequest` |
//! | 4 | the family declares the handles the arguments name | RFC 0027 T2 | — |
//! | 5 | admission T1–T4 | RFC 0027, "The admission predicate" | `CapabilityDenied` |
//! | 6 | the annotation obligations | `RequestEnvelope`'s field rules | `MalformedRequest` |
//! | 7 | the idempotency ledger | `rule idempotency.replay` | `IdempotencyKeyReused` |
//! | 8 | the family handler | the operation's own semantics | its declared codes |
//!
//! Three orderings in that table are decisions rather than conveniences:
//!
//! - **the version check precedes everything**, because a request on a version the
//!   connection did not negotiate is not a request this daemon can read at all;
//! - **shape agreement precedes admission**, because T2 needs the handles the arguments
//!   name and a family cannot report them from a body it has not been given. What this
//!   costs is bounded and stated: a caller learns that its *own* request was malformed
//!   before learning whether its capability admits it. It learns nothing about any
//!   artifact, which is what RFC 0027 X2's existence-oracle rule protects;
//! - **admission precedes the obligations, the ledger, and the handler**, because "denial
//!   precedes semantic work and precedes the index" (X3) and because the idempotency ledger
//!   is state — consulting it before deciding admission would let an unadmitted caller
//!   probe another actor's keys.
//!
//! # What the families inherit
//!
//! A family writes none of: version negotiation, the admission predicate, the audit
//! correlation, the annotation obligations, the idempotency ledger, the epoch set, the cost
//! report, or the error-code union check. It writes a namespace, a pure scope claim, and a
//! handler. See [`family`] for the four-step seam.

pub mod admission;
pub mod capability;
pub mod errors;
pub mod evidence;
pub mod family;
pub mod identity;
pub mod intent;
pub mod obligation;
pub mod observe;
pub mod result;
pub mod state;
pub mod task;
pub mod verification;
pub mod workspace;

use core::fmt;
use std::sync::Arc;

use continuum_workspace::publication::{
    AuditLog, ContentIdentifier, ReferenceStore, ScopedCapabilityPolicy,
};

use crate::protocol::envelope::{EpochSet, RequestEnvelope, ResultEnvelope};
use crate::protocol::handshake::{CapabilityDescriptor, Negotiated};
use crate::protocol::registry;
use crate::protocol::scalar::{CapabilityHandle, Timestamp};
use crate::protocol::spec::Annotation;
use crate::protocol::vocabulary::ErrorCode;

use family::{Arguments, Call, Fault, OperationFamily, Payload, ScopeClaim};
use identity::{AuditCorrelator, HashedCorrelation};
use state::{AdmissionRecord, DaemonState, Replay, ReplayKey, store_level};

/// A failure of one of the daemon's seams, before any wire code is chosen.
///
/// Kept separate from [`Fault`] because these are not answers to a caller: they are the
/// conditions under which the daemon cannot proceed, and each maps to a wire code at the
/// point where a caller is owed one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServiceError {
    /// The identity seam could not name a record. Nothing is published under a guess.
    Identity,
    /// A handle did not cross between the wire spelling and the store's.
    Handle,
}

impl fmt::Display for ServiceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Identity => "no content identity could be derived",
            Self::Handle => "a handle is not well-formed for its artifact class",
        })
    }
}

impl core::error::Error for ServiceError {}

/// The read-only seams and settings a dispatch runs against.
pub struct Services {
    identifier: Box<dyn ContentIdentifier>,
    correlator: Box<dyn AuditCorrelator>,
    negotiated: Negotiated,
    connection: CapabilityHandle,
    epochs: EpochSet,
    now: Option<Timestamp>,
}

impl fmt::Debug for Services {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // The connection capability is a `cap_*`; `CapabilityHandle`'s own `Debug` elides
        // it, and it is named here through that `Debug` rather than through its text.
        f.debug_struct("Services")
            .field("negotiated", &self.negotiated)
            .field("connection", &self.connection)
            .field("epochs", &self.epochs)
            .field("now", &self.now)
            .finish_non_exhaustive()
    }
}

impl Services {
    /// The content-identity seam. Pure in its arguments (INV-005, INV-006).
    #[must_use]
    pub fn identifier(&self) -> &dyn ContentIdentifier {
        self.identifier.as_ref()
    }

    /// What the handshake settled.
    #[must_use]
    pub const fn negotiated(&self) -> Negotiated {
        self.negotiated
    }

    /// The capability the connection presented. A request may present this one or a
    /// descendant of it that narrows.
    #[must_use]
    pub const fn connection(&self) -> &CapabilityHandle {
        &self.connection
    }

    /// The epochs this daemon serves new work under. Copied onto every result, in full
    /// (`rule envelope.epochs_named`).
    #[must_use]
    pub const fn epochs(&self) -> &EpochSet {
        &self.epochs
    }

    /// The deployment's time reading, when it supplied one.
    ///
    /// Time is an explicit effect, never ambient (INV-005, ADR-0003). A daemon with no
    /// reading cannot judge an expiring capability and denies it, because "a daemon that
    /// cannot decide admission fails closed" (RFC 0027).
    #[must_use]
    pub const fn now(&self) -> Option<&Timestamp> {
        self.now.as_ref()
    }
}

/// One decoded operation call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OperationRequest {
    /// The request envelope, exactly as the wire carried it.
    pub envelope: RequestEnvelope,
    /// The operation's decoded request body.
    pub arguments: Arguments,
}

/// One operation's result: the envelope, and the typed response beside it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OperationOutcome {
    /// The result envelope. Its `payload` reads `null` at this layer — see [`result`].
    pub envelope: ResultEnvelope,
    /// The operation's typed response body, or [`Payload::None`] on an error.
    pub payload: Payload,
}

impl OperationOutcome {
    /// The wire error code, when the call failed.
    #[must_use]
    pub fn error_code(&self) -> Option<ErrorCode> {
        self.envelope
            .error
            .value()
            .map(|error: &crate::protocol::envelope::Error| error.code)
    }
}

/// The authority daemon's operation layer.
pub struct Daemon {
    services: Services,
    state: DaemonState,
    store: ReferenceStore,
    store_audit: Arc<AuditLog>,
    families: Vec<Box<dyn OperationFamily>>,
}

impl fmt::Debug for Daemon {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Daemon")
            .field("services", &self.services)
            .field(
                "families",
                &self
                    .families
                    .iter()
                    .map(|family| family.namespace())
                    .collect::<Vec<_>>(),
            )
            .finish_non_exhaustive()
    }
}

impl Daemon {
    /// Start building a daemon.
    ///
    /// `identifier` is the content-identity seam; it is cloned into the publication store
    /// so that a record the daemon names and a record the store publishes carry one
    /// identity, which is the disagreement
    /// [`SealError::IdentityDisagreement`](continuum_workspace::seal::SealError::IdentityDisagreement)
    /// exists to catch.
    pub fn builder<I>(
        identifier: I,
        negotiated: Negotiated,
        connection: CapabilityHandle,
    ) -> Builder
    where
        I: ContentIdentifier + Clone + 'static,
    {
        let protocol = negotiated.protocol_version();
        Builder {
            services: Services {
                identifier: Box::new(identifier.clone()),
                correlator: Box::new(HashedCorrelation),
                negotiated,
                connection,
                epochs: result::unpinned(protocol),
                now: None,
            },
            store_identifier: Box::new(identifier),
            state: DaemonState::new(),
            families: Vec::new(),
            capabilities: Vec::new(),
        }
    }

    /// The seams and settings this daemon runs against.
    #[must_use]
    pub const fn services(&self) -> &Services {
        &self.services
    }

    /// Everything the daemon knows.
    #[must_use]
    pub const fn state(&self) -> &DaemonState {
        &self.state
    }

    /// Everything the daemon knows, mutably.
    ///
    /// The out-of-band administration surface: content staging and capability
    /// provisioning are not operations in this protocol version (IDL §7), so a deployment
    /// reaches them here rather than through [`dispatch`](Daemon::dispatch).
    ///
    /// A capability registered through this handle after the daemon is built is admitted
    /// for every operation its descriptor allows, and cannot *publish*, because the
    /// publication store's own registry is provisioned once at build time. That is the
    /// fail-closed direction, and it costs nothing on the revocation path that matters:
    /// admission is decided before any store call (RFC 0027 A8, X3), so a capability
    /// revoked here never reaches the store at all.
    pub const fn state_mut(&mut self) -> &mut DaemonState {
        &mut self.state
    }

    /// The publication store this daemon publishes sealed workspaces into.
    #[must_use]
    pub const fn store(&self) -> &ReferenceStore {
        &self.store
    }

    /// The store's authorization audit log (plan §18.5).
    #[must_use]
    pub fn store_audit(&self) -> &AuditLog {
        &self.store_audit
    }

    /// Answer one operation call.
    ///
    /// See this module's documentation for the eight steps and the reasons for their order.
    /// Total: every path returns an envelope, and no input reaches a panic.
    #[must_use]
    pub fn dispatch(&mut self, request: &OperationRequest) -> OperationOutcome {
        let Self {
            services,
            state,
            store,
            families,
            ..
        } = self;
        let envelope = &request.envelope;
        let audit = services
            .correlator
            .correlate(&envelope.request_id, &envelope.actor);

        // 1. The negotiated version. "Re-negotiation mid-connection is not a protocol
        //    feature", so the comparison is equality, not compatibility.
        if !services
            .negotiated
            .admits_request(envelope.protocol_version)
        {
            return raise(
                services,
                envelope,
                Fault::new(
                    ErrorCode::ProtocolVersionUnsupported,
                    "the request names a protocol version this connection did not negotiate",
                ),
                &audit,
                false,
            );
        }

        // 2. The operation. A well-formed name that the registry does not declare is
        //    malformed at a different layer than a name that is not a name at all, and
        //    `OperationName` already drew that line.
        let Some(spec) = registry::operation(envelope.operation.as_str()) else {
            return raise(
                services,
                envelope,
                Fault::new(
                    ErrorCode::MalformedRequest,
                    "the request names an operation this protocol version does not declare",
                ),
                &audit,
                false,
            );
        };
        let audit_required = obligation::audit_required(spec);

        // 3. Shape agreement. The envelope's `operation` and the decoded body must name one
        //    operation; a request whose two halves disagree does not validate against the
        //    IDL, whichever half is wrong.
        if request.arguments.operation() != spec.name {
            return raise(
                services,
                envelope,
                Fault::new(
                    ErrorCode::MalformedRequest,
                    "the request body is not the shape this operation declares",
                ),
                &audit,
                audit_required,
            );
        }

        // 4. The scope claim, pure in the arguments and consulting no state.
        let family = families
            .iter()
            .find(|family| Some(family.namespace()) == namespace(spec.name));
        let claim = family.map_or_else(ScopeClaim::default, |family| {
            family.scope(&request.arguments)
        });

        // 5. Admission. The single answer, and the audit record that must exist whatever
        //    the answer was (RFC 0027 P5).
        let admitted = admission::admit(
            spec,
            envelope,
            &claim,
            state,
            &services.connection,
            services.now.as_ref(),
        )
        .cloned();
        state.record_admission(AdmissionRecord {
            operation: spec.name.to_owned(),
            actor: envelope.actor.as_str().to_owned(),
            capability: envelope.capability.clone(),
            admitted: admitted.is_ok(),
            audit: audit.as_str().to_owned(),
        });
        let Ok(grant) = admitted else {
            return OperationOutcome {
                envelope: result::denial(&envelope.request_id, &services.epochs, &audit),
                payload: Payload::None,
            };
        };

        // 6. The annotation obligations, from the registry's own annotation list.
        if let Err(fault) = obligation::check_request(spec, envelope) {
            return raise(services, envelope, fault, &audit, audit_required);
        }

        // 7. The idempotency ledger. Only `@mutation` operations reach it: step 6 has
        //    already refused a key on a `@readonly` one.
        let replay_key = ReplayKey::of(envelope, &request.arguments);
        let key = envelope.idempotency_key.value().cloned();
        if let Some(key) = key.as_deref() {
            if let Some(previous) = state.replay(envelope.actor.as_str(), key) {
                if previous.request == replay_key {
                    return previous.outcome.clone();
                }
                return raise(
                    services,
                    envelope,
                    Fault::new(
                        ErrorCode::IdempotencyKeyReused,
                        "this idempotency key was used for a different request",
                    ),
                    &audit,
                    audit_required,
                );
            }
        }

        // 8. The family. An operation registered ahead of its subsystem is a typed refusal,
        //    never a degraded answer (`rule errors.unsupported_surface`).
        let Some(family) = family else {
            return unsupported_surface(services, envelope, &audit, audit_required);
        };
        let call = Call {
            spec,
            envelope,
            arguments: &request.arguments,
            grant: &grant,
            audit: &audit,
        };
        let outcome = match family.handle(&call, state, services, store) {
            Ok(effect) => {
                debug_assert!(
                    effect.verdict.is_null() == spec.verdict.is_none(),
                    "an operation's result carries a verdict exactly when its `verdict` \
                     clause declares one"
                );
                debug_assert_eq!(
                    effect.payload.operation(),
                    Some(spec.name),
                    "an operation's payload is its own `response` body"
                );
                OperationOutcome {
                    envelope: result::success(
                        &envelope.request_id,
                        &effect,
                        &services.epochs,
                        audit_required.then_some(&audit),
                    ),
                    payload: effect.payload,
                }
            }
            Err(fault) => raise(services, envelope, fault, &audit, audit_required),
        };

        if let Some(key) = key {
            state.record_replay(
                envelope.actor.as_str(),
                &key,
                Replay {
                    request: replay_key,
                    outcome: outcome.clone(),
                },
            );
        }
        outcome
    }
}

/// Build one failure result, checking the code against `rule errors.common` on the way.
///
/// `spec`-less failures — an unknown operation, an unnegotiated version — are checked
/// against nothing because there is no operation to check against; every other code passes
/// through [`Fault::admissible_for`] at its call site in [`Daemon::dispatch`].
fn raise(
    services: &Services,
    envelope: &RequestEnvelope,
    fault: Fault,
    audit: &crate::protocol::scalar::AuditCorrelationId,
    required: bool,
) -> OperationOutcome {
    debug_assert!(
        registry::operation(envelope.operation.as_str())
            .is_none_or(|spec| fault.admissible_for(spec)),
        "a daemon MUST NOT return a code outside `rule errors.common` for the operation"
    );
    // `rule audit.correlation`: present on every `@audit_recorded` result *and* on every
    // result whose error is `CapabilityDenied`, "because a denial that leaves no audit
    // record is indistinguishable from an attack that was never tried".
    let attached = (required || fault.code == ErrorCode::CapabilityDenied).then_some(audit);
    OperationOutcome {
        envelope: result::failure(&envelope.request_id, fault, &services.epochs, attached),
        payload: Payload::None,
    }
}

/// The refusal an operation registered ahead of its subsystem gets.
///
/// > Operations registered in plan §10.2 ahead of their producing subsystem […] MUST fail
/// > with the typed `UnsupportedSemanticFeature` rather than degrading, guessing, or
/// > returning an empty success.
/// >
/// > — `rule errors.unsupported_surface`
///
/// # The one place two IDL rules disagree, and how it is resolved
///
/// `rule errors.common` says "A daemon MUST NOT return a code outside that union for the
/// operation", and the union is the five common codes plus the operation's own `errors`
/// clause. Eleven operations declare `errors []` — `workspace.seal`, `task.status`,
/// `task.subscribe` among them — so for those the two rules cannot both be obeyed while
/// the operation is unshipped: `errors.unsupported_surface` requires exactly the code
/// `errors.common` forbids.
///
/// `errors.unsupported_surface` wins, because it is the rule written *about this
/// situation* while `errors.common` is written about an operation that is being served,
/// and because the alternative — answering `MalformedRequest` for a well-formed request —
/// would tell the caller something false. This function is therefore the one construction
/// path that does not assert the union, and it is deliberately the only one: every other
/// code a family raises still goes through [`raise`]'s check. The defect is recorded
/// against the IDL rather than papered over here.
fn unsupported_surface(
    services: &Services,
    envelope: &RequestEnvelope,
    audit: &crate::protocol::scalar::AuditCorrelationId,
    required: bool,
) -> OperationOutcome {
    OperationOutcome {
        envelope: result::failure(
            &envelope.request_id,
            Fault::new(
                ErrorCode::UnsupportedSemanticFeature,
                "this operation's subsystem is not served by this daemon",
            ),
            &services.epochs,
            required.then_some(audit),
        ),
        payload: Payload::None,
    }
}

/// The namespace half of a `namespace.verb` operation name.
fn namespace(operation: &str) -> Option<&str> {
    operation.split_once('.').map(|(namespace, _)| namespace)
}

/// Builds a [`Daemon`].
pub struct Builder {
    services: Services,
    store_identifier: Box<dyn ContentIdentifier>,
    state: DaemonState,
    families: Vec<Box<dyn OperationFamily>>,
    capabilities: Vec<(CapabilityDescriptor, Option<CapabilityHandle>)>,
}

impl fmt::Debug for Builder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Builder")
            .field("services", &self.services)
            .finish_non_exhaustive()
    }
}

impl Builder {
    /// Supply the epochs this daemon serves new work under.
    #[must_use]
    pub fn epochs(mut self, epochs: EpochSet) -> Self {
        self.services.epochs = epochs;
        self
    }

    /// Supply a time reading, so capabilities with an `expires_at` can be judged.
    ///
    /// A deployment with no clock effect omits it, and every expiring capability is then
    /// denied — the fail-closed direction, and the reason there is no default here.
    #[must_use]
    pub fn now(mut self, now: Timestamp) -> Self {
        self.services.now = Some(now);
        self
    }

    /// Replace the audit-correlation seam.
    #[must_use]
    pub fn correlator(mut self, correlator: impl AuditCorrelator + 'static) -> Self {
        self.services.correlator = Box::new(correlator);
        self
    }

    /// Provision a capability, optionally as a delegation of `parent`.
    ///
    /// Registered in both registries: the wire one, which admission reads, and the
    /// publication store's, which is the store-side *subset* RFC 0027's "Landed vocabulary"
    /// clause describes — the token, actor, level, and class scope the store decides on,
    /// and not the instance scopes, expiry, delegation depth, or profile, which are decided
    /// above it and are enforced here by [`admission`].
    #[must_use]
    pub fn capability(
        mut self,
        descriptor: CapabilityDescriptor,
        parent: Option<CapabilityHandle>,
    ) -> Self {
        self.capabilities.push((descriptor, parent));
        self
    }

    /// Register an operation family.
    #[must_use]
    pub fn family(mut self, family: impl OperationFamily + 'static) -> Self {
        self.families.push(Box::new(family));
        self
    }

    /// Assemble the daemon.
    ///
    /// A capability whose handle is not a well-formed `cap_*` store token is registered in
    /// the wire registry and not in the store's: it can be admitted and it cannot publish,
    /// which is fail-closed in the only direction that matters.
    #[must_use]
    pub fn build(mut self) -> Daemon {
        let store_audit = Arc::new(AuditLog::new());
        let mut store = ReferenceStore::builder(
            BoxedIdentifier(self.store_identifier),
            Arc::clone(&store_audit),
        )
        .policy(ScopedCapabilityPolicy);
        for (descriptor, parent) in self.capabilities {
            if let Ok(token) = identity::capability_to_store(&descriptor.capability) {
                store = store.capability(
                    continuum_workspace::publication::CapabilityDescriptor::new(
                        token,
                        identity::actor_to_store(&descriptor.actor),
                        store_level(descriptor.level),
                    )
                    .scoped_to(store_classes(&descriptor)),
                );
            }
            self.state.register_capability(descriptor, parent);
        }
        Daemon {
            services: self.services,
            state: self.state,
            store: store.build(),
            store_audit,
            families: self.families,
        }
    }
}

/// The artifact classes a wire descriptor scopes to, as the store names them.
///
/// A class the store does not know is dropped rather than approximated: the wire's
/// `artifact_classes` is "plan §4.4 prefixes", the store's [`ArtifactClass`] is the closed
/// set of those prefixes, and a prefix outside it names no class the store could scope to.
/// The wire-side test in [`admission`] still sees the full list, so nothing is widened by
/// the omission.
///
/// [`ArtifactClass`]: continuum_workspace::artifact_path::ArtifactClass
fn store_classes(
    descriptor: &CapabilityDescriptor,
) -> Vec<continuum_workspace::artifact_path::ArtifactClass> {
    descriptor
        .artifact_classes
        .iter()
        .filter_map(|token| continuum_workspace::artifact_path::ArtifactClass::from_token(token))
        .collect()
}

/// Adapts an owned `Box<dyn ContentIdentifier>` back into the by-value seam the store's
/// builder takes.
struct BoxedIdentifier(Box<dyn ContentIdentifier>);

impl ContentIdentifier for BoxedIdentifier {
    fn identify(
        &self,
        class: continuum_workspace::artifact_path::ArtifactClass,
        content: &[u8],
    ) -> Result<
        continuum_workspace::artifact_path::ArtifactHandle,
        continuum_workspace::publication::IdentityUnavailable,
    > {
        self.0.identify(class, content)
    }
}

/// Whether this operation's registry entry declares it a mutation.
///
/// Exposed for the sibling families: an operation's annotations are data, and reading them
/// is always a registry lookup rather than a list a handler keeps.
#[must_use]
pub fn is_mutation(operation: &str) -> bool {
    registry::operation(operation).is_some_and(|spec| spec.has(Annotation::Mutation))
}
