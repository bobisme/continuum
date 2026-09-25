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
//! | 2 | the operation is one of the registry's 83 | plan §10.2 registry | `MalformedRequest` |
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

pub mod acceptance;
pub mod admission;
pub mod budget;
pub mod bundle;
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
pub mod signing;
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
    /// The identity already names byte-different content (ADR-0013): refused, never
    /// overwritten.
    Collision,
}

impl fmt::Display for ServiceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Identity => "no content identity could be derived",
            Self::Handle => "a handle is not well-formed for its artifact class",
            Self::Collision => "the identity already names byte-different content",
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

/// The daemon's signing identity as a deployment supplies it (plan §18.6, ADR-0054,
/// bn-1hape, bn-3glnv).
///
/// A deployment that supplies one ([`Builder::receipt_signer`]) installs it as the held key
/// of the daemon's [`SigningAuthority`](signing::SigningAuthority): every receipt
/// `evidence.link` publishes is signed over its canonical bytes through
/// [`SigningRegistry::sign`](continuum_evidence::signing::SigningRegistry::sign), before the
/// receipt is published, and the same key signs the bundles `intent.export_bundle` writes
/// and the packs `signing.sign_pack` signs. `signing.rotate` and `signing.revoke` change it
/// in place. The keypair normally comes from
/// `continuum_security::keystore::LocalKeystore`, the solo-developer key minted on first use.
/// The daemon holds the key; agents never do (INV-015, RFC 0032 "Signing").
pub struct ReceiptSigner {
    registry: continuum_evidence::signing::SigningRegistry,
    signer: continuum_evidence::signing::LocalSigner,
    restored: Option<continuum_evidence::signing::SigningCustodyState>,
}

/// Why [`Builder::launch_signing`] refused to build a daemon from a custody (bn-18w74).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LaunchRefusal<E> {
    /// The custody did not load, or could not remove a secret it no longer holds.
    Custody(E),
    /// The loaded state does not hold against its registry or the daemon's invariants.
    Install(InstallRefusal),
}

impl<E: fmt::Display> fmt::Display for LaunchRefusal<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Custody(error) => write!(f, "the signing custody refused: {error}"),
            Self::Install(refusal) => {
                write!(f, "the signing custody's state does not hold: {refusal:?}")
            }
        }
    }
}

impl<E: fmt::Debug + fmt::Display> core::error::Error for LaunchRefusal<E> {}

/// Why a registry and key cannot be installed as the deployment's signing identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallRefusal {
    /// The registry holds no standing for the key.
    UnknownSigner,
    /// The registry holds more records than the daemon's bound.
    RecordBound,
    /// The key is active and the registry lacks the records to recover from its
    /// compromise: its revocation, a replacement mint, and that replacement's own
    /// revocation (bn-3glnv, cr-2unxyh).
    NoRoomToRecover,
    /// A restored custody state does not hold against its registry (bn-18w74).
    Custody(signing::CustodyRefusal),
}

impl ReceiptSigner {
    /// A receipt signer from a registry and a signer it holds standing for.
    ///
    /// # Errors
    ///
    /// [`InstallRefusal`] when the registry does not know the key, holds more than
    /// [`MAX_AUDIT_RECORDS`](bundle::MAX_AUDIT_RECORDS) records, or holds the key active
    /// with fewer than [`RECOVERY_RECORDS`](signing::RECOVERY_RECORDS) records left — the
    /// reserve every daemon operation keeps while the held key is active, checked here for
    /// a registry built outside the daemon. So every installable registry can revoke its
    /// held key and mint a revocable replacement.
    pub fn new(
        registry: continuum_evidence::signing::SigningRegistry,
        signer: continuum_evidence::signing::LocalSigner,
    ) -> Result<Self, InstallRefusal> {
        let records = registry.audit_log().len();
        let standing = registry
            .standing(signer.identity())
            .ok_or(InstallRefusal::UnknownSigner)?;
        if records > bundle::MAX_AUDIT_RECORDS {
            return Err(InstallRefusal::RecordBound);
        }
        if *standing == continuum_evidence::signing::SignerStanding::Active
            && records.saturating_add(signing::RECOVERY_RECORDS) > bundle::MAX_AUDIT_RECORDS
        {
            return Err(InstallRefusal::NoRoomToRecover);
        }
        Ok(Self {
            registry,
            signer,
            restored: None,
        })
    }

    /// A receipt signer restarted from its custody (bn-18w74): the held key and the
    /// persisted [`SigningCustodyState`](continuum_evidence::signing::SigningCustodyState)
    /// — own keys, links, and pre-signed revocations beside the registry.
    ///
    /// Everything is checked before the key can be used: the key is known, the record
    /// bound holds, an active key can still be revoked — the reserve every daemon operation
    /// keeps (a restored state was written by a daemon, which may leave fewer than
    /// [`RECOVERY_RECORDS`](signing::RECOVERY_RECORDS) free after a replacing mint) — then
    /// [`validate_custody`](signing::validate_custody) — the held key is the one restored,
    /// every own key has standing, every link is attested by the keys it concerns, is
    /// recorded in the audit log, and sits on the right side of the own/adopted line, the
    /// own links form disjoint chains, and every pre-signed revocation is a retired own
    /// key's own.
    ///
    /// # Errors
    ///
    /// [`InstallRefusal`], typed; a state that does not hold is never repaired.
    pub fn restored(
        signer: continuum_evidence::signing::LocalSigner,
        state: continuum_evidence::signing::SigningCustodyState,
    ) -> Result<Self, InstallRefusal> {
        // The reserve a restored state must keep is the one the running daemon keeps: an
        // active held key can still be revoked (one record). A replacing mint or a loss
        // recovery may legitimately leave fewer than `RECOVERY_RECORDS` free, and a state
        // the daemon wrote must restart (review of bn-18w74, B1). A registry built outside
        // the daemon still needs the full reserve (`new`).
        let registry = state.registry().clone();
        let records = registry.audit_log().len();
        let standing = registry
            .standing(signer.identity())
            .ok_or(InstallRefusal::UnknownSigner)?;
        if records > bundle::MAX_AUDIT_RECORDS {
            return Err(InstallRefusal::RecordBound);
        }
        if *standing == continuum_evidence::signing::SignerStanding::Active
            && records >= bundle::MAX_AUDIT_RECORDS
        {
            return Err(InstallRefusal::NoRoomToRecover);
        }
        signing::validate_custody(&state, signer.identity()).map_err(InstallRefusal::Custody)?;
        Ok(Self {
            registry,
            signer,
            restored: Some(state),
        })
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

    fn into_parts(
        self,
    ) -> (
        continuum_evidence::signing::SigningRegistry,
        continuum_evidence::signing::LocalSigner,
        Option<continuum_evidence::signing::SigningCustodyState>,
    ) {
        (self.registry, self.signer, self.restored)
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

    /// Supply a new time reading.
    ///
    /// Time is an explicit effect (INV-005, ADR-0003): the deployment reads its clock and
    /// hands the reading in, and the daemon never reads one itself. Admission judges expiry
    /// against the latest reading, and so does subscription delivery, which re-decides a
    /// subscriber's standing before every pass.
    pub fn set_now(&mut self, now: Timestamp) {
        self.services.now = Some(now);
    }

    /// Decide T1–T4 again for a request this daemon already admitted, against the current
    /// registry and time reading, and return the grant it stands under now.
    ///
    /// Everything `dispatch` decides at step 5 — level, snapshot, intent, and class scope,
    /// the instance scope of what the request names, the profile's denial and privilege
    /// tests, and the capability's standing through its whole chain — with the family's own
    /// scope claim. Nothing is recorded: this is not a request. A subscription outlives the
    /// request that opened it, so delivery calls this before every pass, and a re-registered
    /// or narrowed grant is decided exactly as a new request would be (cr-3hcpn4).
    #[must_use]
    pub fn readmit(
        &self,
        envelope: &RequestEnvelope,
        arguments: &Arguments,
    ) -> Option<CapabilityDescriptor> {
        let spec = registry::operation(envelope.operation.as_str())?;
        let family = self
            .families
            .iter()
            .find(|family| Some(family.namespace()) == namespace(spec.name))?;
        let claim = family.scope(arguments);
        admission::admit(
            spec,
            envelope,
            &claim,
            &self.state,
            &self.services.connection,
            self.services.now.as_ref(),
        )
        .ok()
        .cloned()
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
        let version = self.services.negotiated.protocol_version();
        let spec = registry::operation(envelope.operation.as_str());
        // An operation the negotiated version does not declare gets the one answer
        // `dispatch` gives it, whatever its body: the version gate is decided before the
        // body matters (`OperationSpec::defined_at`, bn-7xz8v), and it records no audit.
        let mut refused = if spec.is_some_and(|spec| !spec.defined_at(version)) {
            raise(&self.services, envelope, undeclared(), &audit, false)
        } else {
            let audit_required = spec.is_some_and(obligation::audit_required);
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
        };
        emitted_at(&mut refused, version);
        refused.envelope
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
        let version = self.services.negotiated.protocol_version();
        self.answer(request, injector).map(|mut outcome| {
            emitted_at(&mut outcome, version);
            outcome
        })
    }

    /// [`dispatch_or_die`](Daemon::dispatch_or_die) before the answer is held to the
    /// connection's version ([`emitted_at`]). The idempotency ledger records what this
    /// returns, so a recorded answer is held to the version of the connection that replays
    /// it, not of the one that recorded it.
    fn answer(
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
        //    An operation `@since` a version later than the negotiated one is not declared at
        //    that version, so it is refused exactly as an undeclared name is: here, before
        //    admission and before the idempotency ledger, so the answer does not depend on
        //    the grant and no key recorded on a newer connection replays on an older one
        //    (`rule versioning.compatible_change`, `rule signing.identities`). The date is
        //    the registry's own for every operation (`OperationSpec::since`, bn-7xz8v).
        if !spec.defined_at(services.negotiated.protocol_version()) {
            return Ok(raise(services, envelope, undeclared(), &audit, false));
        }
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
            denial: admitted.is_err().then_some(state::Denial::Admission),
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
                    // The ledger is keyed by actor and request, not by capability, so the
                    // presenting capability may not be the one the outcome was recorded
                    // under: a sibling token, a re-provisioned grant, a narrowed parent.
                    // Admission above already decided its standing (cr-3hcpn4).
                    //
                    // Recovery is capability-relative (RFC 0027 N2): a replayed offer is
                    // decided under the presenting capability, as a fresh one is, and an
                    // offer it would be denied is dropped. What remains is returned only
                    // when the presenting grant covers the recording one, or admits every
                    // handle the remaining outcome names; otherwise it is the one denial,
                    // and nothing of the outcome is returned.
                    let mut outcome = replayed(&previous.outcome, envelope, &audit);
                    outcome.recovery =
                        admissible_offers(outcome.recovery, envelope, families, state, services);
                    // The handles the first execution derived are re-decided too, whatever
                    // the outcome names: a replay does not run the handler, so its derived
                    // checks run here or not at all (cr-3lrkq3). Under a covering grant they
                    // pass by construction; they are decided anyway, so the rule does not
                    // rest on `covers` being exact.
                    let derived_admitted = previous
                        .derived
                        .iter()
                        .all(|handle| admission::admits_derived(&grant, handle.as_derived()));
                    if !derived_admitted
                        || (!admission::covers(&grant, &previous.grant)
                            && !replay_admitted(&outcome, &grant))
                    {
                        state.record_denial(audit.as_str(), state::Denial::ReplayAuthority);
                        return Ok(raise(
                            services,
                            envelope,
                            Fault::denied(),
                            &audit,
                            audit_required,
                        ));
                    }
                    // Only after the replay's authority is decided (so a presenter it does
                    // not cover learns nothing of the recorded answer): a recorded answer is
                    // re-emitted on this connection, so it is held to
                    // this connection's version: an error code the negotiated version does
                    // not define (a 3.9 `OutcomeUnknown` replayed to 3.8, for example after
                    // state moved to a new connection) is refused typed, and nothing runs
                    // (`rule versioning.enums`, review cr-1dc5ii).
                    if let Some(error) = previous.outcome.envelope.error.value() {
                        if !errors::defined_at(error.code, services.negotiated.protocol_version()) {
                            return Ok(raise(
                                services,
                                envelope,
                                version_undefined_outcome(),
                                &audit,
                                audit_required,
                            ));
                        }
                    }
                    return Ok(outcome);
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
        let consulted = std::cell::RefCell::default();
        let call = Call {
            spec,
            envelope,
            arguments: &request.arguments,
            grant: &grant,
            audit: &audit,
            consulted: &consulted,
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
            Err(fault)
                if !errors::defined_at(fault.code, services.negotiated.protocol_version()) =>
            {
                // Every family gates its version-dated codes itself, before anything changes
                // (`signing::require_custody_version`), so this arm is unreachable today. It
                // is the backstop at emission: were a gate missing, no code the negotiated
                // version does not define leaves, although the refusal could then follow a
                // change the handler made.
                raise(
                    services,
                    envelope,
                    version_undefined_outcome(),
                    &audit,
                    audit_required,
                )
            }
            Err(fault) => {
                if fault.derived_denial {
                    state.record_derived_denial(audit.as_str());
                }
                raise(
                    services,
                    envelope,
                    offer_under_n2(fault, envelope, families, state, services),
                    &audit,
                    audit_required,
                )
            }
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
                    grant: grant.clone(),
                    derived: consulted.take().into_iter().collect(),
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
/// Whether `grant` admits every handle a recorded outcome names, for a replay whose
/// presenting grant does not cover the recording one ([`admission::covers`]): its
/// envelope (the task, the continuation, the artifact references, the error), its payload,
/// its typed error data, and its recovery offers.
///
/// Each part is read in its canonical wire form, so this is exactly what a replay would
/// send, and every string in it that spells a handle of an artifact class is decided by
/// [`admission::admits_derived`]. An outcome that cannot be encoded is not replayed: the
/// check fails closed.
fn replay_admitted(outcome: &OperationOutcome, grant: &CapabilityDescriptor) -> bool {
    use crate::codec::json::Json as Wire;
    use continuum_intent::canonical_json::Json;

    fn admitted(value: &Json, grant: &CapabilityDescriptor) -> bool {
        let named = |text: &str| {
            provisioning::class_of(text).is_none()
                || admission::admits_derived(grant, admission::Derived::Instance(text))
        };
        match value {
            Json::String(text) => named(text),
            Json::Array(items) => items.iter().all(|item| admitted(item, grant)),
            // Two fields carry strings a class prefix can spell and that name no instance:
            // `commitment` (`ArtifactRef.commitment`, a content hash; the artifact is decided
            // by the `handle` beside it) and `status` (a vocabulary token such as
            // `task_suspended`). Every other string is decided. A string that only looks like
            // a handle is decided too, which can refuse a replay the grant would admit and
            // never the converse: the check fails closed.
            Json::Object(fields) => fields.iter().all(|(key, item)| {
                named(key) && (key == "commitment" || key == "status" || admitted(item, grant))
            }),
            _ => true,
        }
    }

    let mut documents: Vec<Vec<u8>> = Vec::new();
    let Ok(envelope) = crate::codec::write_in::<Wire, _>(&outcome.envelope) else {
        return false;
    };
    documents.push(envelope);
    match crate::codec::operations::encode_payload(&outcome.payload) {
        Ok(Some(payload)) => documents.push(payload.as_bytes().to_vec()),
        Ok(None) => {}
        Err(_) => return false,
    }
    match crate::codec::operations::encode_error_data_in::<Wire>(&outcome.data) {
        Ok(Some(data)) => documents.push(data.as_bytes().to_vec()),
        Ok(None) => {}
        Err(_) => return false,
    }
    for offer in &outcome.recovery {
        match crate::transport::encode_arguments(&offer.arguments) {
            Ok(arguments) => documents.push(arguments.as_bytes().to_vec()),
            Err(_) => return false,
        }
    }
    documents
        .iter()
        .all(|bytes| Json::parse(bytes).is_ok_and(|document| admitted(&document, grant)))
}

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
/// Hold an answer to the version of the connection it is emitted on.
///
/// A daemon "MUST NOT emit fields the negotiated version does not define" (`rule
/// versioning.compatible_change`), and that rule governs a dated field over any rule that
/// requires it (RFC 0026 correction 62, bn-7xz8v):
///
/// - `ResultEnvelope.audit` is `@since("3.1")` ([`crate::protocol::since::RESULT_AUDIT`]).
///   Below 3.1 it is not emitted. The audit record is still written daemon-side; `rule
///   audit.correlation` applies from 3.1, where the field exists.
/// - `CertificateRejection`, the `Error.data` shape of `CertificateRejected`, is
///   `@since("3.4")` ([`crate::protocol::since::CERTIFICATE_REJECTION`]). Below 3.4 no code
///   declares a shape, so `data` stays absent (`rule encoding.opaque_payloads`).
///
/// Every answer [`Daemon`] gives passes through here, fresh, replayed, or refused before
/// its body decoded.
fn emitted_at(outcome: &mut OperationOutcome, version: crate::protocol::scalar::ProtocolVersion) {
    use crate::protocol::since::{CERTIFICATE_REJECTION, RESULT_AUDIT, defines};
    if !defines(Some(RESULT_AUDIT), version) {
        outcome.envelope.audit = crate::protocol::spec::Optional::Absent;
    }
    if !defines(Some(CERTIFICATE_REJECTION), version) {
        if let family::ErrorData::CertificateRejection(_) = outcome.data {
            outcome.data = family::ErrorData::None;
        }
    }
}

/// The refusal for an operation this connection's version does not declare: the one an
/// undeclared name gets (`rule signing.identities` names it for the 3.8 operations).
fn undeclared() -> Fault {
    Fault::new(
        ErrorCode::MalformedRequest,
        "the request names an operation this protocol version does not declare",
    )
}

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
    fault.recovery = admissible_offers(fault.recovery, envelope, families, state, services);
    fault
}

/// The offers of `offers` the capability `envelope` presents would be admitted for
/// ([`offer_under_n2`]); shared by a fresh failure and a replayed one.
fn admissible_offers(
    mut offers: Vec<family::RecoveryOffer>,
    envelope: &RequestEnvelope,
    families: &[Box<dyn OperationFamily>],
    state: &DaemonState,
    services: &Services,
) -> Vec<family::RecoveryOffer> {
    offers.retain(|offer| {
        let Some(spec) = registry::operation(offer.arguments.operation()) else {
            return false;
        };
        // An operation the connection's version does not declare is not offered: the
        // client could only be refused it (`OperationSpec::defined_at`).
        if !spec.defined_at(services.negotiated.protocol_version()) {
            return false;
        }
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
    offers
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

/// The refusal an answer gets when its error code is not defined at the negotiated
/// protocol version: a typed, non-retryable `UnsupportedSemanticFeature`, which every
/// version defines and which claims nothing about what the refused answer said.
fn version_undefined_outcome() -> Fault {
    Fault::new(
        ErrorCode::UnsupportedSemanticFeature,
        "the outcome of this request is not defined at the negotiated protocol version",
    )
    .not_retryable()
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
        let (registry, key, restored) = signer.into_parts();
        self.state.signing_mut().install(registry, key, restored);
        self
    }

    /// Supply the signing authority's durable custody (bn-18w74). Every change the
    /// `signing` operations and a bundle adoption make is then recorded through it before
    /// it takes effect, and a change it refuses is undone and answered
    /// `PublicationAborted`, after which the daemon refuses every signing write and
    /// signature until it is restarted from what the custody holds. The daemon performs
    /// no I/O itself: the custody is the deployment's capability, normally
    /// `continuum_security`'s `LocalKeystore`. Reached only through
    /// [`launch_signing`](Self::launch_signing), so a custody is never attached to a
    /// signing identity it did not load; a later [`receipt_signer`](Self::receipt_signer)
    /// replacing that identity makes the authority refuse every write.
    #[must_use]
    fn signing_custody<C>(mut self, custody: C) -> Self
    where
        C: continuum_evidence::signing::SigningCustody + 'static,
    {
        self.state.signing_mut().set_custody(custody);
        self
    }

    /// The deployment launcher (bn-18w74): build the signing authority from its durable
    /// custody, so its identities survive a restart.
    ///
    /// 1. [`load_or_mint`](continuum_evidence::signing::SigningCustody::load_or_mint): the
    ///    recorded state and held key, or, on first use only, a key minted with `entropy`
    ///    and recorded as audit record 0 attributed to `actor`;
    /// 2. [`ReceiptSigner::restored`]: every part checked against the registry and the
    ///    daemon's invariants — the record bound, the recovery reserve, own keys, link
    ///    attestation and topology, pre-signed revocations — before the key is used;
    /// 3. the held intent bundles and import records (bn-3snfi) checked the same way: each
    ///    bundle bounded, decoded, its identity recomputed from its bytes, authenticated by
    ///    its own signature, its links attested; each record naming a held bundle that
    ///    exports a well-formed contract of that identity; each adopted link carried by a
    ///    held bundle ([`intent::restore_imports`]);
    /// 4. [`sweep`](continuum_evidence::signing::SigningCustody::sweep): any secret the
    ///    validated state does not hold (a crash between a change and its clean-up) is
    ///    removed;
    /// 5. the key is installed, the bundles are held again and every recorded contract
    ///    re-enters the registry at `proposed`, `entropy` becomes the daemon's key-entropy
    ///    capability, and the custody records every later change
    ///    ([`signing_custody`](Self::signing_custody)).
    ///
    /// The daemon performs no I/O here: every effect is the custody's.
    ///
    /// # Errors
    ///
    /// [`LaunchRefusal`], typed, carrying no key material. A custody that does not load
    /// or does not validate is refused, never repaired and never minted over.
    pub fn launch_signing<C>(
        self,
        mut custody: C,
        actor: &continuum_evidence::actor::ActorId,
        mut entropy: Box<dyn continuum_evidence::signing::KeyEntropy + Send + Sync>,
    ) -> Result<Self, LaunchRefusal<C::Error>>
    where
        C: continuum_evidence::signing::SigningCustody + 'static,
    {
        let (state, key) = custody
            .load_or_mint(actor, &mut *entropy)
            .map_err(LaunchRefusal::Custody)?;
        // The bundle parts are shared bytes: kept aside before the state is consumed.
        let bundles = state.held_bundles().clone();
        let imports = state.imports().clone();
        let adopted = state.adopted_links().to_vec();
        let signer = ReceiptSigner::restored(key, state).map_err(LaunchRefusal::Install)?;
        let restored = intent::restore_imports(&self.services, &bundles, &imports, &adopted)
            .map_err(|refusal| LaunchRefusal::Install(InstallRefusal::Custody(refusal)))?;
        drop((bundles, imports, adopted));
        custody
            .sweep(signer.identity())
            .map_err(LaunchRefusal::Custody)?;
        let mut builder = self.receipt_signer(signer).key_entropy(entropy);
        intent::reenter_imports(&mut builder.state, restored);
        Ok(builder.signing_custody(custody))
    }

    /// Add signers to the local allowed-signers policy (plan §18.6's organizational
    /// deployment, RFC 0037 A3). Local policy, not a bundle, decides which signers count;
    /// a bundle's pinned set only narrows it (bn-3glnv).
    #[must_use]
    pub fn allowed_signers(
        mut self,
        allowed: &continuum_evidence::signing::AllowedSigners,
    ) -> Self {
        self.state.signing_mut().allow(allowed);
        self
    }

    /// Supply the key-entropy capability `signing.mint`, `signing.rotate`, and a `key-lost`
    /// `signing.revoke` draw a new key from (INV-005, ADR-0003). A daemon without one
    /// refuses to mint with `UnsupportedSemanticFeature`; it never falls back to another
    /// source (bn-3glnv).
    #[must_use]
    pub fn key_entropy(
        mut self,
        entropy: Box<dyn continuum_evidence::signing::KeyEntropy + Send + Sync>,
    ) -> Self {
        self.state.signing_mut().set_entropy(entropy);
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
    /// prefix spelling `ws_` included, and [`ProvisioningRefusal::Instance`] when its
    /// `instances` names a handle that cannot be an instance scope
    /// (`rule capability.instance_scope`). Every descriptor is checked before any is
    /// registered, so a refused build registers nothing.
    pub fn try_build(mut self) -> Result<Daemon, ProvisioningRefusal> {
        let mut scopes = Vec::with_capacity(self.capabilities.len());
        for (descriptor, _) in &self.capabilities {
            scopes.push(provisioning::scoped_classes(descriptor)?);
            provisioning::scoped_instances(descriptor)?;
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
