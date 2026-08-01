//! The connection handshake and the protocol-major N/N−1 window (IDL §7).
//!
//! > The handshake is **connection-level, not an operation**: it precedes any
//! > `RequestEnvelope`, is therefore outside the plan §10.2 registry, and has no entry in
//! > RFC 0027's authority table. It fixes five things and nothing else.
//! >
//! > — RFC 0026, "Connection lifecycle"
//!
//! The five are version, encoding, authority, limits, and epoch position; [`ClientHello`]
//! and [`ServerWelcome`] carry them. [`negotiate`] decides the first two.
//!
//! # Where the version rules actually live
//!
//! > `ProtocolEpoch::negotiate` is the typed form of this rule and `ProtocolWindow` of
//! > the major window; the daemon and that module MUST agree.
//! >
//! > — RFC 0026, "Version window and negotiation"
//!
//! They agree by being the same code: [`ProtocolVersion`] *is*
//! `continuum_value::epoch::ProtocolEpoch`, and [`negotiate`] calls that module's
//! `negotiate` and `ProtocolWindow::serves` rather than restating either rule. What this
//! module adds is the separation the IDL draws between the versions a daemon *implements*
//! and the majors it *serves* (`protocol.version` against `protocol.majors_served`), and
//! the typed reason a rejected client gets.

use core::fmt;

use continuum_value::epoch::{ProtocolRange, ProtocolWindow};

use super::prelude::*;
use crate::protocol_struct;

protocol_struct! {
    /// Inclusive range of protocol versions a client accepts.
    struct VersionRange {
        /// IDL `low: ProtocolVersion required`.
        low: ProtocolVersion required;
        /// IDL `high: ProtocolVersion required`.
        high: ProtocolVersion required;
    }
}

protocol_struct! {
    /// First frame on a connection, sent by the client.
    struct ClientHello {
        /// The client's supported range. The daemon selects the highest common
        /// version.
        protocol_versions: VersionRange required;
        /// Encodings the client accepts, most preferred first.
        encodings: list<Encoding> required;
        /// Client software identity, for the audit log.
        client: String required;
        /// IDL `actor: ActorId required`.
        actor: ActorId required;
        /// The capability the connection presents. Every request on the
        /// connection is authorized against a capability; a connection MAY
        /// present a narrower one per request.
        capability: CapabilityHandle required;
        /// Optional feature identifiers the client understands. Unknown
        /// identifiers MUST be ignored by the daemon.
        features: list<String> optional;
    }
}

protocol_struct! {
    /// Second frame, sent by the daemon on success.
    struct ServerWelcome {
        /// The selected version: the highest one common to both sides.
        protocol_version: ProtocolVersion required;
        /// IDL `encoding: Encoding required`.
        encoding: Encoding required;
        /// The majors this daemon serves concurrently: N and N−1.
        majors_served: list<U32> required;
        /// Daemon software identity, for the audit log and defect reports.
        server: String required;
        /// The authority the presented capability actually confers, after
        /// scoping. A client MUST NOT infer authority from anything else.
        grant: CapabilityDescriptor required;
        /// IDL `limits: ServerLimits required`.
        limits: ServerLimits required;
        /// Feature identifiers the daemon supports, intersected with the
        /// client's.
        features: list<String> required;
        /// The epochs this daemon is currently serving new work under.
        epochs: EpochSet required;
        /// Epoch advances announced but not yet applied. Plan §4.6 requires
        /// each advance to publish its typed per-artifact-class compatibility
        /// statement before it is applied; this is where a client reads it.
        pending_advances: list<EpochAdvanceNotice> required;
    }
}

protocol_struct! {
    /// Second frame, sent by the daemon when it refuses the connection.
    ///
    /// The alternative to [`ServerWelcome`], never sent beside it. Without it a
    /// client cannot tell a typed refusal from a transport failure, which is
    /// the gap RFC 0027 raised as F4. The two frames are told apart by their
    /// required fields and neither validates as the other: `ServerWelcome`
    /// requires `protocol_version`, this frame requires `code`.
    struct ServerReject {
        /// Why the connection was refused. `rule handshake.rejection` fixes
        /// the admissible set.
        code: ErrorCode required;
        /// Stable, non-interpolated explanation of the code. Never a rendering
        /// of a capability, a token, or anything the client supplied
        /// (INV-016).
        detail: String required;
        /// Whether an identical reconnection can succeed without any change by
        /// the caller. False for every code this version admits; the field is
        /// declared because `ErrorCode` is `@open` and a later minor may add a
        /// connection-level refusal an identical retry can satisfy.
        retryable: Bool required;
        /// The majors this daemon serves: N and N−1. Present so a client
        /// refused for its version learns the window from the refusal itself;
        /// it never received a `ServerWelcome` and has no other channel for
        /// it.
        majors_served: list<U32> required;
    }
}

protocol_struct! {
    /// An announced epoch advance (plan §4.6). Advancing never mutates an
    /// existing artifact: re-derived artifacts receive new identities linked
    /// to their predecessors by `SUPERSEDES` edges. The daemon may hold at
    /// most two epochs concurrently during migration; new work defaults to
    /// the newest, and continuations resume only under their pinned epoch.
    struct EpochAdvanceNotice {
        /// Which epoch advances: `protocol`, `semantic`, `intent`,
        /// `evidence`, `proof`, or `corpus`.
        epoch: String required;
        /// IDL `from: EpochIdentity required`.
        from: EpochIdentity required;
        /// IDL `to: EpochIdentity required`.
        to: EpochIdentity required;
        /// Compatibility statement per artifact class, keyed by the plan §4.4
        /// class prefix.
        compatibility: map<String,Compatibility> required;
        /// Estimated invalidation blast radius by artifact class.
        blast_radius: map<String,U64> required;
    }
}

protocol_struct! {
    /// What a `cap_*` capability confers (RFC 0027 "Authority levels and
    /// capabilities").
    struct CapabilityDescriptor {
        /// IDL `capability: CapabilityHandle required`.
        capability: CapabilityHandle required;
        /// IDL `actor: ActorId required`.
        actor: ActorId required;
        /// IDL `level: AuthorityLevel required`.
        level: AuthorityLevel required;
        /// Snapshots in scope; empty means unrestricted within `level`.
        snapshots: list<WorkspaceHandle> required;
        /// Intents in scope; empty means unrestricted within `level`.
        intents: list<IntentHandle> required;
        /// Artifact classes in scope, as plan §4.4 prefixes; empty means all.
        artifact_classes: list<String> required;
        /// IDL `expires_at: Timestamp nullable`.
        expires_at: Timestamp nullable;
        /// Whether the holder may delegate, and how deeply.
        delegation_depth: U32 required;
        /// The narrowing surface beyond level and scope. Absent is the
        /// fail-closed reading and is exactly equivalent to an empty profile:
        /// no privileged operation granted, no additional data grant held, no
        /// cross-principal sharing. See `rule capability.profile_narrowing`.
        profile: CapabilityProfile optional;
    }
}

protocol_struct! {
    /// What a capability grants and withholds beyond its level and scope
    /// (RFC 0027, "The admission predicate").
    ///
    /// This is the wire form of admission test T3 and of the per-operation
    /// restrictions a level cannot express: two capabilities with identical
    /// levels and scopes differ here in whether they admit `intent.accept`,
    /// which before 3.1 was deployment folklore rather than a wire fact.
    /// Every field narrows; none widens. A profile can only subtract from
    /// what `level` and the scope lists already permit.
    struct CapabilityProfile {
        /// The `@privileged` operations this capability may invoke (T3). An
        /// operation absent from this list is denied however high the level.
        /// Naming a non-`@privileged` operation here grants nothing.
        privileged_operations: list<OperationName> required;
        /// Operations this capability may not invoke whatever its level
        /// permits — docs/49's Reviewer cell, which a total order cannot
        /// express (RFC 0027 correction 22). An operation named both here and
        /// in `privileged_operations` is denied.
        denied_operations: list<OperationName> required;
        /// Additional grants this capability carries, for operations that
        /// require one beyond their authority level.
        data_grants: list<DataGrant> required;
        /// Whether this capability's publications may be deduplicated against
        /// content another principal already holds. RFC 0026 requires the
        /// sharing policy to be a capability property rather than a
        /// daemon-global flag, because a global flag cannot be scoped,
        /// delegated, or revoked. False does not weaken the indistinguishable-
        /// cost rule: a publication MUST report the same `cost` either way.
        cross_principal_sharing: Bool required;
    }
}

protocol_struct! {
    /// Server-declared operational limits, negotiated once per connection.
    struct ServerLimits {
        /// Minimum time idempotency keys are honored. Default 86_400_000 ms.
        idempotency_retention_ms: DurationMs required;
        /// IDL `max_page_size: U32 required`.
        max_page_size: U32 required;
        /// IDL `max_result_bytes: ByteCount required`.
        max_result_bytes: ByteCount required;
        /// Concurrency quota this capability draws on; `QuotaExhausted` is
        /// returned when it is exceeded.
        max_concurrent_tasks: U32 required;
    }
}

// --- negotiation ------------------------------------------------------------------

impl VersionRange {
    /// The range as the value layer's [`ProtocolRange`], or [`None`] when `high`
    /// precedes `low`.
    ///
    /// The IDL calls the range "inclusive" and declares both endpoints `required`, but
    /// states no ordering constraint a decoder could enforce, so an inverted range is
    /// possible on the wire and is rejected here rather than silently normalized.
    #[must_use]
    pub fn to_range(&self) -> Option<ProtocolRange> {
        ProtocolRange::new(self.low, self.high)
    }
}

/// What the handshake settled: one version and one encoding, for the life of the
/// connection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Negotiated {
    protocol_version: ProtocolVersion,
    encoding: Encoding,
}

impl Negotiated {
    /// The selected version.
    #[must_use]
    pub const fn protocol_version(self) -> ProtocolVersion {
        self.protocol_version
    }

    /// The selected encoding.
    #[must_use]
    pub const fn encoding(self) -> Encoding {
        self.encoding
    }

    /// Whether a request envelope naming `version` may be served on this connection.
    ///
    /// > A `RequestEnvelope` naming a different `protocol_version` than the connection
    /// > negotiated MUST be rejected with `ProtocolVersionUnsupported`. Re-negotiation
    /// > mid-connection is not a protocol feature.
    /// >
    /// > — RFC 0026, "Version window and negotiation"
    ///
    /// The comparison is equality, not compatibility: a minor difference is a different
    /// version, and the connection negotiated one.
    #[must_use]
    pub fn admits_request(self, version: ProtocolVersion) -> bool {
        version == self.protocol_version
    }
}

/// Why a connection was not established.
///
/// Each variant reports the typed [`ErrorCode`] the protocol fixes for it through
/// [`error_code`](NegotiationError::error_code) — or [`None`] where the protocol fixes
/// none, which is a gap in the specification rather than a choice made here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NegotiationError {
    /// The client's `VersionRange` has `high` below `low`.
    MalformedRange,
    /// No version the daemon implements falls inside the client's range.
    NoCommonVersion,
    /// The daemon implements a version inside the client's range, but no longer serves
    /// that major: the client falls outside the N/N−1 window.
    OutsideServiceWindow,
    /// The client and the daemon share no encoding.
    NoCommonEncoding,
}

impl NegotiationError {
    /// The wire error code the protocol fixes for this rejection.
    ///
    /// [`ErrorCode::ProtocolVersionUnsupported`] is fixed for both version failures:
    /// "No protocol version is common to client and daemon, or the client's major falls
    /// outside the N / N−1 window" (IDL §6, `ErrorCode`).
    ///
    /// The other two return [`None`], and deliberately so. The IDL fixes no code for a
    /// handshake frame that is itself malformed, nor for an encoding mismatch:
    /// `MalformedRequest` is defined against a *request*, and the handshake precedes any
    /// request envelope (IDL §7). Choosing one here would be inventing wire semantics.
    /// Protocol 3.1 declares the [`ServerReject`] frame and still fixes no code for those
    /// two (`rule handshake.rejection` names the set explicitly), so both remain
    /// transport-level closes.
    #[must_use]
    pub const fn error_code(self) -> Option<ErrorCode> {
        match self {
            Self::NoCommonVersion | Self::OutsideServiceWindow => {
                Some(ErrorCode::ProtocolVersionUnsupported)
            }
            Self::MalformedRange | Self::NoCommonEncoding => None,
        }
    }
}

/// The first protocol version that defines [`ServerReject`].
///
/// A refusal frame is sent *before* a version is negotiated, so it cannot be gated on
/// one. The gate is the client's own offer instead: a client whose range reaches this
/// version has told the daemon it can parse the frame.
const REJECT_FRAME_SINCE: ProtocolVersion = ProtocolVersion::new(3, 1);

impl ServerReject {
    /// The typed refusal for `error`, or [`None`] where the connection must be closed
    /// without a frame.
    ///
    /// > A daemon MUST send it only when the client's offered `VersionRange` reaches a
    /// > version that defines it — `high` at or above `"3.1"` — and MUST close the
    /// > connection without a frame otherwise, because a frame the client cannot parse is
    /// > not a typed refusal.
    /// >
    /// > — `rule handshake.rejection`
    ///
    /// [`None`] therefore covers two different situations, and collapsing them would lose
    /// the distinction the rule draws: a client too old to read the frame, and a failure
    /// for which the protocol fixes no code ([`error_code`](NegotiationError::error_code)
    /// returning [`None`]).
    ///
    /// `detail` is supplied by the caller and MUST be stable, non-interpolated text
    /// (INV-016); [`NegotiationError`]'s own [`Display`](fmt::Display) is such a text and
    /// is what a daemon is expected to pass. `retryable` is false for every code this
    /// version admits.
    #[must_use]
    pub fn for_negotiation(
        error: NegotiationError,
        hello: &ClientHello,
        majors_served: &[u32],
        detail: &str,
    ) -> Option<Self> {
        if hello.protocol_versions.high < REJECT_FRAME_SINCE {
            return None;
        }
        Some(Self {
            code: error.error_code()?,
            detail: detail.to_owned(),
            retryable: false,
            majors_served: majors_served.to_vec(),
        })
    }
}

impl fmt::Display for NegotiationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::MalformedRange => "the offered version range is inverted",
            Self::NoCommonVersion => "no protocol version is common to client and daemon",
            Self::OutsideServiceWindow => "the offered major is outside the served N/N−1 window",
            Self::NoCommonEncoding => "no wire encoding is common to client and daemon",
        })
    }
}

impl core::error::Error for NegotiationError {}

/// Settle version and encoding for a connection.
///
/// `implemented` is every version the daemon can speak and `window` is the major window
/// it currently serves — `protocol.version` and `protocol.majors_served` are two
/// settings, and this is the difference between them. `encodings` is the daemon's set;
/// the client's list is ordered most-preferred-first and that preference decides.
///
/// The version rule is RFC 0026's, unmodified: the highest version common to the
/// client's inclusive range and the daemon's *served* set, never a version outside the
/// client's range. When nothing is servable the two failures are told apart — a client
/// that offered only a retired or an unreleased major gets
/// [`OutsideServiceWindow`](NegotiationError::OutsideServiceWindow), a client that shares
/// nothing at all gets [`NoCommonVersion`](NegotiationError::NoCommonVersion) — because
/// the two mean different things to the client even though the wire code is the same.
///
/// # Errors
///
/// [`NegotiationError`], whose [`error_code`](NegotiationError::error_code) names the
/// wire rejection where the protocol fixes one.
pub fn negotiate(
    implemented: &[ProtocolVersion],
    window: ProtocolWindow,
    encodings: &[Encoding],
    hello: &ClientHello,
) -> Result<Negotiated, NegotiationError> {
    let offered = hello
        .protocol_versions
        .to_range()
        .ok_or(NegotiationError::MalformedRange)?;

    let servable: Vec<ProtocolVersion> = implemented
        .iter()
        .copied()
        .filter(|version| window.serves(*version))
        .collect();

    let protocol_version = match ProtocolVersion::negotiate(&servable, offered) {
        Some(version) => version,
        None if ProtocolVersion::negotiate(implemented, offered).is_some() => {
            return Err(NegotiationError::OutsideServiceWindow);
        }
        None => return Err(NegotiationError::NoCommonVersion),
    };

    let encoding = hello
        .encodings
        .iter()
        .copied()
        .find(|candidate| encodings.contains(candidate))
        .ok_or(NegotiationError::NoCommonEncoding)?;

    Ok(Negotiated {
        protocol_version,
        encoding,
    })
}
