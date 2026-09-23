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
//!   and this layer takes an already-decoded [`Arguments`] and emits a typed [`Payload`]
//!   beside the envelope. That was forced until protocol 3.2, when the IDL fixed neither
//!   the canonical field order of its two encodings nor their union tagging nor those
//!   payloads' shape; it is now a *choice*, and the better one. A dispatch that parsed
//!   bytes would be a dispatch whose failures are two kinds — "I cannot read this" and "I
//!   will not do this" — mixed in one function. [`crate::codec`] does the first and
//!   [`crate::transport`] joins them, so this layer stays a pure function of typed values.
//!   The one seam back is [`Daemon::refuse`], which builds the answer a caller is owed for
//!   a request the codec could not read.
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
//! # The step boundaries are instrumented
//!
//! Each of those eight steps is preceded by a [`CrashPoint`], and one more follows the
//! handler: nine boundaries at which a deployment's [`CrashInjector`] may kill the daemon,
//! the way [`region`] instruments a worker's phases and the store instruments its
//! publication's. [`Daemon::dispatch_or_die`] is the entry point that consults them and
//! [`Daemon::dispatch`] is the same pipeline under [`NoCrash`]. What that buys is a
//! *falsifiable* statement about this pipeline rather than a decorated one: steps 1–7 are
//! volatile-side work and step 8 is the only step that reaches the store, so the durable
//! image after a kill is a step function of the boundary, and a step that grew a durable
//! side effect would show up as two boundaries disagreeing. See [`recovery`].
//!
//! # What the families inherit
//!
//! A family writes none of: version negotiation, the admission predicate, the audit
//! correlation, the annotation obligations, the idempotency ledger, the epoch set, the cost
//! report, or the error-code union check. It writes a namespace, a pure scope claim, and a
//! handler. See [`family`] for the four-step seam.

pub mod admission;
pub mod budget;
pub mod capability;
pub mod context;
pub mod continuation;
pub mod errors;
pub mod evidence;
pub mod family;
pub mod identity;
pub mod intent;
pub mod obligation;
pub mod observe;
pub mod output;
pub mod provisioning;
pub mod recovery;
pub mod region;
pub mod result;
pub mod state;
pub mod task;
pub mod terminal;
pub mod verification;
pub mod whiteboard;
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
use crate::protocol::vocabulary::{ErrorCode, ResultStatus};

use family::{Arguments, Call, Fault, OperationFamily, Payload, ScopeClaim};
use identity::{AuditCorrelator, HashedCorrelation};
use provisioning::ProvisioningRefusal;
use recovery::{CrashInjector, CrashPoint, Killed, NoCrash};
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
    receipt_signer: Option<ReceiptSigner>,
}

/// The daemon's receipt-signing identity (plan §18.6, ADR-0054, bn-1hape).
///
/// A deployment that supplies one ([`Builder::receipt_signer`]) has every receipt
/// `evidence.link` publishes signed over its canonical bytes through
/// [`SigningRegistry::sign`](continuum_evidence::signing::SigningRegistry::sign), before the
/// receipt is published. The signature is held in [`DaemonState::receipt_signature`]; it has
/// no wire spelling at protocol 3.6 (bn-3glnv). The keypair normally comes from
/// `continuum_security::keystore::LocalKeystore`, the solo-developer key minted on first use.
/// The daemon holds the key; agents never do (INV-015, RFC 0032 "Signing").
pub struct ReceiptSigner {
    registry: continuum_evidence::signing::SigningRegistry,
    signer: continuum_evidence::signing::LocalSigner,
}

impl ReceiptSigner {
    /// A receipt signer from a registry and a signer it holds standing for.
    #[must_use]
    pub const fn new(
        registry: continuum_evidence::signing::SigningRegistry,
        signer: continuum_evidence::signing::LocalSigner,
    ) -> Self {
        Self { registry, signer }
    }

    /// The registry whose standing governs this signer.
    #[must_use]
    pub const fn registry(&self) -> &continuum_evidence::signing::SigningRegistry {
        &self.registry
    }

    /// The identity receipts are signed as.
    #[must_use]
    pub const fn identity(&self) -> &continuum_evidence::signing::SignerIdentity {
        self.signer.identity()
    }

    pub(crate) fn sign_receipt(
        &self,
        artifact: &continuum_value::identity::ContentIdentity,
    ) -> Result<
        continuum_evidence::signing::ArtifactSignature,
        continuum_evidence::signing::SignRefusal,
    > {
        self.registry.sign(
            &self.signer,
            continuum_evidence::signing::SignedArtifactKind::Receipt,
            artifact,
        )
    }
}

impl fmt::Debug for ReceiptSigner {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ReceiptSigner")
            .field("identity", &self.identity().handle())
            .finish_non_exhaustive()
    }
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
            .field("receipt_signer", &self.receipt_signer)
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

    /// The deployment's receipt-signing identity, when it supplied one.
    #[must_use]
    pub const fn receipt_signer(&self) -> Option<&ReceiptSigner> {
        self.receipt_signer.as_ref()
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
    /// The typed `Error.data` specifics, or [`ErrorData::None`] on a success and on
    /// every failure whose code declares no shape. `Error.data` on the envelope reads
    /// absent at this layer for the same reason `payload` reads null: the wire field is
    /// `Opaque` — bytes of the *negotiated* encoding — and this layer is encoding-free,
    /// so [`crate::transport::Server::answer`] encodes it where the encoding is known
    /// (RFC 0026 F19, protocol 3.4).
    pub data: family::ErrorData,
    /// The typed `Error.recovery` offers, already filtered under RFC 0027 N2, or empty.
    ///
    /// `Error.recovery` on the envelope reads empty at this layer for the reason `data`
    /// reads absent: each entry's `arguments` is `Opaque` — bytes of the *negotiated*
    /// encoding — so [`crate::transport::Server::answer`] encodes these offers into it
    /// where the encoding is known. Read this field, not the envelope's list, in process.
    pub recovery: Vec<family::RecoveryOffer>,
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
    startup: recovery::Startup,
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
                receipt_signer: None,
            },
            store_identifier: Box::new(identifier),
            state: DaemonState::new(),
            families: Vec::new(),
            capabilities: Vec::new(),
            store_faults: None,
            durable: None,
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

    /// What this daemon did about tasks when it started (plan §4.5 O2).
    ///
    /// [`Startup::Cold`](recovery::Startup::Cold) for a cold start. On a restart, the
    /// typed output of [`recovery::resolve_tasks`], or the typed refusal when the connection
    /// capability could not audit the adopted store.
    #[must_use]
    pub const fn startup(&self) -> &recovery::Startup {
        &self.startup
    }

    /// The typed refusal for a request this daemon could not *read*.
    ///
    /// The codec sits outside the operation layer, so a request whose `arguments` do not
    /// decode never reaches [`dispatch`](Daemon::dispatch) — and a caller that sent a
    /// message this daemon cannot read is still owed an answer rather than a closed
    /// connection. This builds that answer through the same three seams every other
    /// result goes through: the epoch set (`rule envelope.epochs_named`), the
    /// audit-correlation identity derived from the request identity alone
    /// (`rule audit.correlation`), and the error-code union check.
    ///
    /// `code` is the caller's, and it is the codec's: [`CodecError::code`] names the wire
    /// code RFC 0026's malformed-input list fixes for each decode failure.
    ///
    /// [`CodecError::code`]: crate::codec::CodecError::code
    #[must_use]
    pub fn refuse(&self, envelope: &RequestEnvelope, code: ErrorCode) -> ResultEnvelope {
        let audit = self
            .services
            .correlator
            .correlate(&envelope.request_id, &envelope.actor);
        let audit_required = registry::operation(envelope.operation.as_str())
            .is_some_and(obligation::audit_required);
        raise(
            &self.services,
            envelope,
            Fault::new(
                code,
                "the request body is not the shape this operation declares",
            ),
            &audit,
            audit_required,
        )
        .envelope
    }

    /// Answer one operation call.
    ///
    /// See this module's documentation for the eight steps and the reasons for their order.
    /// Total: every path returns an envelope, and no input reaches a panic.
    ///
    /// # The one `expect`, and why no input reaches it
    ///
    /// This delegates to [`dispatch_or_die`](Daemon::dispatch_or_die) under
    /// [`NoCrash`], whose [`kills`](CrashInjector::kills) returns `false` for every boundary
    /// — a checked fact, not a comment
    /// ([`recovery::tests::no_crash_kills_at_no_boundary`]). So the `Err` arm is unreachable
    /// for *every* input, and the `expect` is a statement about this function's own argument
    /// rather than about anything a caller sends. The alternative would be to fabricate a
    /// wire answer for a daemon that is not there, and the protocol fixes no error code for
    /// one: see [`Killed`].
    ///
    /// [`recovery::tests::no_crash_kills_at_no_boundary`]: recovery
    #[must_use]
    pub fn dispatch(&mut self, request: &OperationRequest) -> OperationOutcome {
        self.dispatch_or_die(request, &NoCrash)
            .expect("`NoCrash` kills at no boundary, so this dispatch cannot have died")
    }

    /// Answer one operation call, under a crash injector — the harness entry point.
    ///
    /// [`Ok`] when the daemon survived the whole dispatch, and [`Err`] naming the boundary it
    /// died at when it did not. There is no envelope on the error path on purpose: a process
    /// that is gone sends no frame, and a killed daemon's caller is owed nothing this layer
    /// can build. What the harness does next is take the durable substrate
    /// ([`Daemon::crash`]) and restart over it.
    ///
    /// # Errors
    ///
    /// [`Killed`] when `injector` kills at one of the nine boundaries [`CrashPoint`]
    /// enumerates.
    pub fn dispatch_or_die(
        &mut self,
        request: &OperationRequest,
        injector: &dyn CrashInjector,
    ) -> Result<OperationOutcome, Killed> {
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

        kill(injector, CrashPoint::BeforeVersionCheck)?;

        // 1. The negotiated version. "Re-negotiation mid-connection is not a protocol
        //    feature", so the comparison is equality, not compatibility.
        if !services
            .negotiated
            .admits_request(envelope.protocol_version)
        {
            return Ok(raise(
                services,
                envelope,
                Fault::new(
                    ErrorCode::ProtocolVersionUnsupported,
                    "the request names a protocol version this connection did not negotiate",
                ),
                &audit,
                false,
            ));
        }

        kill(injector, CrashPoint::BeforeOperationLookup)?;

        // 2. The operation. A well-formed name that the registry does not declare is
        //    malformed at a different layer than a name that is not a name at all, and
        //    `OperationName` already drew that line.
        let Some(spec) = registry::operation(envelope.operation.as_str()) else {
            return Ok(raise(
                services,
                envelope,
                Fault::new(
                    ErrorCode::MalformedRequest,
                    "the request names an operation this protocol version does not declare",
                ),
                &audit,
                false,
            ));
        };
        let audit_required = obligation::audit_required(spec);

        kill(injector, CrashPoint::BeforeShapeCheck)?;

        // 3. Shape agreement. The envelope's `operation` and the decoded body must name one
        //    operation; a request whose two halves disagree does not validate against the
        //    IDL, whichever half is wrong.
        if request.arguments.operation() != spec.name {
            return Ok(raise(
                services,
                envelope,
                Fault::new(
                    ErrorCode::MalformedRequest,
                    "the request body is not the shape this operation declares",
                ),
                &audit,
                audit_required,
            ));
        }

        kill(injector, CrashPoint::BeforeScopeClaim)?;

        // 4. The scope claim, pure in the arguments and consulting no state.
        let family = families
            .iter()
            .find(|family| Some(family.namespace()) == namespace(spec.name));
        let claim = family.map_or_else(ScopeClaim::default, |family| {
            family.scope(&request.arguments)
        });

        kill(injector, CrashPoint::BeforeAdmission)?;

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
            return Ok(OperationOutcome {
                envelope: result::denial(&envelope.request_id, &services.epochs, &audit),
                payload: Payload::None,
                data: family::ErrorData::None,
                recovery: Vec::new(),
            });
        };

        kill(injector, CrashPoint::BeforeObligations)?;

        // 6. The annotation obligations, from the registry's own annotation list.
        if let Err(fault) = obligation::check_request(spec, envelope) {
            return Ok(raise(services, envelope, fault, &audit, audit_required));
        }

        kill(injector, CrashPoint::BeforeIdempotencyLedger)?;

        // 7. The idempotency ledger. Only `@mutation` operations reach it: step 6 has
        //    already refused a key on a `@readonly` one.
        let replay_key = ReplayKey::of(envelope, &request.arguments);
        let key = envelope.idempotency_key.value().cloned();
        if let Some(key) = key.as_deref() {
            if let Some(previous) = state.replay(envelope.actor.as_str(), key) {
                if previous.request == replay_key {
                    return Ok(replayed(&previous.outcome, envelope, &audit));
                }
                return Ok(raise(
                    services,
                    envelope,
                    Fault::new(
                        ErrorCode::IdempotencyKeyReused,
                        "this idempotency key was used for a different request",
                    ),
                    &audit,
                    audit_required,
                ));
            }
        }

        kill(injector, CrashPoint::BeforeHandler)?;

        // 8. The family. An operation registered ahead of its subsystem is a typed refusal,
        //    never a degraded answer (`rule errors.unsupported_surface`).
        let Some(family) = family else {
            return Ok(unsupported_surface(
                services,
                envelope,
                &audit,
                audit_required,
            ));
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
                debug_assert!(
                    effect.completion.status() == ResultStatus::Ok
                        || spec.has(Annotation::TaskStarting),
                    "only a `@task_starting` operation MAY return `task_started` or \
                     `task_suspended`"
                );
                OperationOutcome {
                    envelope: result::success(
                        &envelope.request_id,
                        &effect,
                        &services.epochs,
                        audit_required.then_some(&audit),
                    ),
                    payload: effect.payload,
                    data: family::ErrorData::None,
                    recovery: Vec::new(),
                }
            }
            Err(fault) => raise(
                services,
                envelope,
                offer_under_n2(fault, envelope, families, state, services),
                &audit,
                audit_required,
            ),
        };

        // The last boundary: the handler ran — store writes and all — and the replay record
        // has not been written. It is the only one of the nine at which the store can have
        // changed, which is what makes the durable image a step function of the boundary
        // (see [`CrashPoint`]).
        kill(injector, CrashPoint::AfterHandler)?;

        // The ledger keeps every outcome except a retryable failure.
        //
        // `rule idempotency.replay`: "A mutation replayed with the same `idempotency_key` and
        // a byte-identical canonical request MUST return the same task or artifact identity."
        // A success binds its key, because it is the identity a replay returns. A
        // non-retryable failure binds its key too: RFC 0026's taxonomy says an identical
        // retry cannot succeed, so the replay answers what a re-run would, without re-running
        // the handler's effects.
        //
        // A retryable failure ([`errors::retryable`]) does not bind its key. The taxonomy
        // says an identical retry *can* succeed, and for `PublicationAborted` it names the
        // route — "same idempotency key" — and "Atomicity of publication" says "the retry is
        // a fresh publication, not a resumption of a partial one" and that "a retry under the
        // same idempotency key derives the same records; a record that is already published
        // converges on its existing identity, and it is not duplicated". A recorded abort
        // would answer that retry with the abort, forever within the retention window. The
        // failed attempt named no identity, so leaving the key unbound breaks no replay the
        // daemon still remembers (`ServerLimits.idempotency_retention_ms`).
        let binds = outcome
            .envelope
            .error
            .value()
            .is_none_or(|error| !error.retryable);
        if let Some(key) = key.filter(|_| binds) {
            state.record_replay(
                envelope.actor.as_str(),
                &key,
                Replay {
                    request: replay_key,
                    outcome: outcome.clone(),
                },
            );
        }
        Ok(outcome)
    }

    /// Kill the process and hand back what the durable layer still holds.
    ///
    /// This is the crash, at the grain this daemon has one: the value is consumed, so every
    /// [`VolatileFact`](recovery::VolatileFact) — the whole of [`DaemonState`] and the region
    /// tree with it — is dropped, and what comes out is the substrate a restart is built
    /// over. There is no flush, no checkpoint and no drain, deliberately: a crash that got to
    /// tidy up is not one.
    ///
    /// Hand the result to [`Builder::over`] to restart, and to
    /// [`recovery::recover`] to reconcile.
    #[must_use]
    pub fn crash(self) -> DurableSubstrate {
        DurableSubstrate {
            store: self.store,
            audit: self.store_audit,
        }
    }
}

/// Ask the injector, and turn a kill into the typed absence of an answer.
fn kill(injector: &dyn CrashInjector, point: CrashPoint) -> Result<(), Killed> {
    if injector.kills(point) {
        return Err(Killed { point });
    }
    Ok(())
}

/// The answer to a true replay: the recorded outcome, re-addressed to *this* call.
///
/// `rule idempotency.replay` fixes what a replay returns — "the same task or artifact
/// identity" — so the payload, the status, the artifacts, the task, the verdict, and the
/// error are the recorded ones, unchanged. Two envelope fields are not part of that
/// identity. They name the *attempt*, and a retry is a new attempt:
///
/// - **`request_id`** — "client-unique; echoed in the result" (RFC 0026, `RequestEnvelope`
///   table; the `ResultEnvelope` row reads "echo"). A retry carries a fresh one by
///   construction ([`ReplayKey`] excludes it for that reason). A client matches answers to
///   requests by this field, so it echoes the retry's value, not the first attempt's.
/// - **`audit`** — "The identity MUST be a function of the request identity alone —
///   `request_id` and `actor`" (`rule audit.correlation`). The first attempt's value is
///   `f(first request_id, actor)`, so this call's `audit` is the one derived for *this*
///   request identity. Presence is not re-decided: it is a function of the operation and
///   the outcome's error code, and both are the recorded ones. A `CapabilityDenied` never
///   reaches the ledger, because admission is step 5.
///
/// # The record the new correlation names
///
/// `@audit_recorded` obliges "every call written to the audit log" (RFC 0026, annotation
/// table; plan §18.5), and the `audit` field "names the plan §18.5 audit record the call
/// produced" (RFC 0026, `ResultEnvelope` notes). A replay is a call. Its record is the
/// [`AdmissionRecord`] step 5 writes before the ledger is consulted, under this same
/// correlation value (RFC 0027 P5: every admission decision is recorded). So the value this
/// function puts in `audit` names a record that exists, and no second write is needed
/// here. Writing one would also break exactly-once: the replay must not re-run the
/// handler's own effects, which include any audit-bearing registry write such as
/// `intent.accept`'s `acceptance.audit_record`. That record keeps the first attempt's
/// correlation because it describes the first attempt's effect.
fn replayed(
    recorded: &OperationOutcome,
    envelope: &RequestEnvelope,
    audit: &crate::protocol::scalar::AuditCorrelationId,
) -> OperationOutcome {
    let mut outcome = recorded.clone();
    outcome.envelope.request_id = envelope.request_id.clone();
    if !outcome.envelope.audit.is_absent() {
        outcome.envelope.audit = crate::protocol::spec::Optional::Present(audit.clone());
    }
    outcome
}

/// Everything that survives a daemon crash.
///
/// The publication store and the authorization audit log it writes through — the durable
/// column of [`recovery`]'s table, as one value. It is deliberately not a `Clone`: two copies
/// of a store are two stores, and a restart that ran against a copy would be reconciling
/// something no daemon ever wrote to.
#[derive(Debug)]
pub struct DurableSubstrate {
    store: ReferenceStore,
    audit: Arc<AuditLog>,
}

impl DurableSubstrate {
    /// The store, for a [`recovery::recover`] pass before anything is restarted over it.
    #[must_use]
    pub const fn store(&self) -> &ReferenceStore {
        &self.store
    }

    /// The authorization audit log (plan §18.5), which the crash did not truncate either.
    #[must_use]
    pub fn audit(&self) -> &AuditLog {
        &self.audit
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
    // The typed `Error.data` value travels beside the envelope, exactly as the typed
    // payload does, because the wire field is `Opaque` — bytes of the negotiated
    // encoding — and this layer is encoding-free (RFC 0026 F19; see
    // `OperationOutcome::data`).
    let data = fault.data.clone();
    let recovery = fault.recovery.clone();
    OperationOutcome {
        envelope: result::failure(&envelope.request_id, fault, &services.epochs, attached),
        payload: Payload::None,
        data,
        recovery,
    }
}

/// Drop every recovery offer the presenting capability would be denied.
///
/// > **N2 — a daemon MUST NOT offer an operation the presenting capability would deny.**
/// > `next_operations` and `Error.recovery` are computed against the request's capability,
/// > so a recovery surface never widens authority (A7) and never advertises the existence of
/// > a privileged path to a principal that does not hold it.
/// >
/// > — RFC 0027, "`next_operations` and recovery are capability-relative"
///
/// Each offer is put through the same [`admission::admit`] a real call would meet: the
/// offered operation's registry entry, the scope claim its own family computes from the
/// offered arguments, and the request's envelope with its `snapshot` cleared — an offer
/// names its snapshot in its body, and the envelope a client re-issues it under is the
/// client's. An offer for an operation no family here serves is dropped too: it would be
/// answered `UnsupportedSemanticFeature`, which is not a recovery.
///
/// Pure: admission reads state and writes nothing, and no admission record is written for
/// an offer, because an offer is not a request.
fn offer_under_n2(
    mut fault: Fault,
    envelope: &RequestEnvelope,
    families: &[Box<dyn OperationFamily>],
    state: &DaemonState,
    services: &Services,
) -> Fault {
    fault.recovery.retain(|offer| {
        let Some(spec) = registry::operation(offer.arguments.operation()) else {
            return false;
        };
        let Some(family) = families
            .iter()
            .find(|family| Some(family.namespace()) == namespace(spec.name))
        else {
            return false;
        };
        let claim = family.scope(&offer.arguments);
        let mut offered = envelope.clone();
        offered.snapshot = crate::protocol::spec::Nullable::Null;
        admission::admit(
            spec,
            &offered,
            &claim,
            state,
            &services.connection,
            services.now.as_ref(),
        )
        .is_ok()
    });
    fault
}

/// The refusal an operation registered ahead of its subsystem gets.
///
/// > Operations registered in plan §10.2 ahead of their producing subsystem […] MUST fail
/// > with the typed `UnsupportedSemanticFeature` rather than degrading, guessing, or
/// > returning an empty success.
/// >
/// > — `rule errors.unsupported_surface`
///
/// # The rule conflict this used to work around, and what closed it
///
/// Until protocol 3.2 this was the one construction path that did **not** assert
/// `rule errors.common`, because the two rules contradicted each other: the union was the
/// five common codes plus the operation's own `errors` clause, and twenty-five of the
/// seventy-two operations — the three declaring `errors []` and the twenty-two that never
/// named the code — could not return `UnsupportedSemanticFeature` while
/// `rule errors.unsupported_surface` required exactly that of an unshipped lane.
/// bn-3gi resolved it in favour of `errors.unsupported_surface` here and recorded the
/// defect; bn-i4aem paid it in the IDL, where it belonged.
///
/// `UnsupportedSemanticFeature` is now the sixth always-admissible code
/// (`rule errors.common`, protocol 3.2), so this path goes through [`raise`] like every
/// other and the bypass is gone. Nothing about the *answer* changed; what changed is that
/// the daemon no longer has a documented exception to one of its own rules.
fn unsupported_surface(
    services: &Services,
    envelope: &RequestEnvelope,
    audit: &crate::protocol::scalar::AuditCorrelationId,
    required: bool,
) -> OperationOutcome {
    raise(
        services,
        envelope,
        Fault::new(
            ErrorCode::UnsupportedSemanticFeature,
            "this operation's subsystem is not served by this daemon",
        ),
        audit,
        required,
    )
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
    store_faults: Option<Box<dyn continuum_workspace::publication::StorageFaults>>,
    durable: Option<DurableSubstrate>,
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

    /// Supply the receipt-signing identity (plan §18.6, bn-1hape). Every receipt
    /// `evidence.link` publishes is then signed before it is published, and a signer the
    /// registry no longer holds active refuses the publication rather than publishing it
    /// unsigned. A deployment that omits it publishes receipts unsigned, which a verifier
    /// reads as `UnverifiedReason::Unsigned`.
    #[must_use]
    pub fn receipt_signer(mut self, signer: ReceiptSigner) -> Self {
        self.services.receipt_signer = Some(signer);
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

    /// Supply the storage seam the publication store publishes through.
    ///
    /// > docs/35's acceptance list requires "a crash injected between the content commit and
    /// > the index commit of every publication phase, with fsck run on the survivor". This is
    /// > where that injection enters.
    /// >
    /// > — [`StorageFaults`]
    ///
    /// It enters *there*, one crate down, and this is the daemon-side door to it: without
    /// this, a deployment could not reach the INV-017 instant at all, and the crash-recovery
    /// evidence would have to injure a store it built itself rather than the one a daemon
    /// actually publishes into. Ignored by [`over`](Builder::over), which adopts a store that
    /// already has its own.
    ///
    /// [`StorageFaults`]: continuum_workspace::publication::StorageFaults
    #[must_use]
    pub fn store_faults(
        mut self,
        faults: impl continuum_workspace::publication::StorageFaults + 'static,
    ) -> Self {
        self.store_faults = Some(Box::new(faults));
        self
    }

    /// Restart over the substrate a crashed daemon left behind.
    ///
    /// The other half of [`Daemon::crash`]. The store is adopted whole — its content, index,
    /// receipt ledger, abort log and *its own capability registry* — because that registry is
    /// durable state administered through
    /// [`ReferenceStore::mint`](continuum_workspace::publication::ReferenceStore::mint) and
    /// [`revoke`](continuum_workspace::publication::ReferenceStore::revoke), not something a
    /// restart re-derives. So [`capability`](Builder::capability) still provisions the *wire*
    /// registry that admission reads — [`VolatileFact::WireCapabilityRegistry`], the
    /// out-of-band surface a deployment restores at startup — and no longer writes the
    /// store's.
    ///
    /// [`VolatileFact::WireCapabilityRegistry`]: recovery::VolatileFact::WireCapabilityRegistry
    ///
    /// The direction that omission runs in is the safe one: a capability the deployment stops
    /// provisioning is denied at admission, before any store call (RFC 0027 A8, X3), so it
    /// never reaches the store's registry at all.
    #[must_use]
    pub fn over(mut self, durable: DurableSubstrate) -> Self {
        self.durable = Some(durable);
        self
    }

    /// Assemble the daemon.
    ///
    /// [`try_build`](Builder::try_build) with the refusal turned into a panic, for a
    /// deployment whose capabilities are literals it controls.
    ///
    /// # Panics
    ///
    /// When a provisioned capability is refused (see [`try_build`](Builder::try_build)).
    /// The panic message is the refusal's [`Display`](core::fmt::Display), which names the
    /// capability and the spelling.
    #[must_use]
    pub fn build(self) -> Daemon {
        self.try_build()
            .unwrap_or_else(|refusal| panic!("daemon provisioning refused: {refusal}"))
    }

    /// Assemble the daemon, or refuse a mis-provisioned capability, typed.
    ///
    /// A capability whose handle is not a well-formed `cap_*` store token is registered in
    /// the wire registry and not in the store's: it can be admitted and it cannot publish,
    /// which is fail-closed in the only direction that matters.
    ///
    /// # Errors
    ///
    /// [`ProvisioningRefusal::ArtifactClass`] when a descriptor's `artifact_classes` names a
    /// string that is not a class token (`rule artifact_class.spelling`) — the plan §4.4
    /// prefix spelling `ws_` included. Every descriptor is checked before any is
    /// registered, so a refused build registers nothing.
    pub fn try_build(mut self) -> Result<Daemon, ProvisioningRefusal> {
        let mut scopes = Vec::with_capacity(self.capabilities.len());
        for (descriptor, _) in &self.capabilities {
            scopes.push(provisioning::scoped_classes(descriptor)?);
        }

        // A restart adopts the surviving store and provisions the wire registry only; a cold
        // start builds a store and provisions both. The two paths differ in exactly that,
        // which is the durable/volatile split spelled as control flow.
        if let Some(durable) = self.durable {
            for (descriptor, parent) in self.capabilities {
                self.state.register_capability(descriptor, parent)?;
            }
            // The startup task-resolution pass (plan §4.5 O2), before any dispatch can run.
            // It reads the adopted store under the connection capability and writes only the
            // task table: a `Restored` task as a live entry with its continuation (the resume
            // branch, bn-20142), a `Terminal` task as a live terminal entry (bn-2g3ei), every
            // other task into the resolved set.
            //
            // A restored record is tied to the receipt the store's ledger kept for it
            // (bn-283p6), through the one audit view the pass reads under. A record with no
            // receipt is not restored: the task is filed, and reported, as
            // `Failed(Unreceipted)`, never as a `Restored` or `Terminal` it is not.
            let audit = identity::capability_to_store(&self.services.connection)
                .map_err(|_| continuum_workspace::publication::CapabilityDenied)
                .and_then(|operator| {
                    durable
                        .store
                        .audit_view(&operator)
                        .map(|audit| (audit, operator))
                });
            let startup = match audit {
                Ok((audit, operator)) => {
                    let mut resolution =
                        recovery::resolve_tasks_in(&durable.store, &audit, &operator);
                    let mut withdrawn = Vec::new();
                    for resolved in resolution.tasks() {
                        // `resolve_tasks_in` tied every identity to its receipt and withdrew
                        // every resolution that rested on one with none
                        // (`Failed(Unreceipted)`, bn-283p6), so each restore below succeeds.
                        // It restores the record under its receipt-tied index identity, never
                        // one this build re-derives. If a restore fails anyway, the task is
                        // withdrawn the same way here: a `Restored` or `Terminal` resolution
                        // never stands without the live task it names.
                        let loaded = match &resolved.resolution {
                            recovery::Resolution::Restored(_) => {
                                match (&resolved.continuation, &resolved.continuation_identity) {
                                    (Some(record), Some(identity)) => record
                                        .restore(&audit, identity.handle())
                                        .ok()
                                        .map(|restored| {
                                            self.state.tasks_mut().restore(restored);
                                        }),
                                    _ => None,
                                }
                            }
                            recovery::Resolution::Terminal(_) => match &resolved.terminal {
                                Some(record) => record.restore(&audit).ok().map(|entry| {
                                    self.state.tasks_mut().put(entry);
                                }),
                                None => None,
                            },
                            recovery::Resolution::Settled | recovery::Resolution::Failed(_) => {
                                self.state.tasks_mut().resolve(resolved.clone());
                                Some(())
                            }
                        };
                        if loaded.is_none() {
                            self.state.tasks_mut().resolve(resolved.unreceipted());
                            withdrawn.push(resolved.task.clone());
                        }
                    }
                    resolution.withdraw(&withdrawn);
                    recovery::Startup::Resolved(resolution)
                }
                Err(denied) => recovery::Startup::Refused(denied),
            };
            return Ok(Daemon {
                services: self.services,
                state: self.state,
                store: durable.store,
                store_audit: durable.audit,
                families: self.families,
                startup,
            });
        }

        let store_audit = Arc::new(AuditLog::new());
        let mut store = ReferenceStore::builder(
            BoxedIdentifier(self.store_identifier),
            Arc::clone(&store_audit),
        )
        .policy(ScopedCapabilityPolicy);
        if let Some(faults) = self.store_faults {
            store = store.faults(BoxedFaults(faults));
        }
        for ((descriptor, parent), classes) in self.capabilities.into_iter().zip(scopes) {
            if let Ok(token) = identity::capability_to_store(&descriptor.capability) {
                store = store.capability(
                    continuum_workspace::publication::CapabilityDescriptor::new(
                        token,
                        identity::actor_to_store(&descriptor.actor),
                        store_level(descriptor.level),
                    )
                    .scoped_to(classes),
                );
            }
            self.state.register_capability(descriptor, parent)?;
        }
        Ok(Daemon {
            services: self.services,
            state: self.state,
            store: store.build(),
            store_audit,
            families: self.families,
            startup: recovery::Startup::Cold,
        })
    }
}

/// Adapts an owned `Box<dyn StorageFaults>` back into the by-value seam the store's builder
/// takes. The sibling of [`BoxedIdentifier`], for the same reason.
struct BoxedFaults(Box<dyn continuum_workspace::publication::StorageFaults>);

impl continuum_workspace::publication::StorageFaults for BoxedFaults {
    fn check(
        &self,
        phase: continuum_workspace::publication::PublicationPhase,
    ) -> Result<(), continuum_workspace::publication::AbortReason> {
        self.0.check(phase)
    }
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
