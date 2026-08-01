//! Capability negotiation: the daemon half of the connection handshake.
//!
//! # What this surface is, read off the IDL rather than imagined
//!
//! The bone that opened this module was written as "handshake → mint/delegate/revoke". The
//! wire has one of those four, and the other three are not omissions to be filled in:
//!
//! > Minting, scoping, delegating, and revoking capabilities is a separate administrative
//! > surface (plan §4.5) that is out of scope for this IDL version.
//! >
//! > — the IDL, §7
//!
//! > Capability administration is **not** part of the native protocol's operation registry
//! > at major 3. […] A mint operation on the agent-facing connection is a
//! > privilege-escalation surface. […] Keeping administration out-of-band gives the native
//! > protocol a property worth stating outright: **no operation in this protocol can widen
//! > the authority of the connection that invokes it** (INV-015, agent least authority).
//! >
//! > — RFC 0026, "Capability administration" (correction 20)
//!
//! [`registry::OPERATIONS`](crate::protocol::registry::OPERATIONS) agrees: there is no
//! `capability` namespace among the 72, and `continuum-workspace`'s store-side
//! [`ReferenceStore::mint`] and [`ReferenceStore::revoke`] have **no wire caller** and are
//! deliberately not given one here. Adding a wire mint would not be filling a gap; it would
//! be deleting the property RFC 0026 names. So what this module implements is exactly the
//! three of the handshake's five things that need daemon state — authority, limits, and
//! epoch position — beside the two [`negotiate`] already decides.
//!
//! | The five things the handshake fixes | Where it is decided |
//! |---|---|
//! | version | [`handshake::negotiate`], from RFC 0026's window rule |
//! | encoding | [`handshake::negotiate`], client preference order |
//! | authority | here: the descriptor the daemon *holds*, never the one the client claims |
//! | limits | here: [`ConnectionPolicy::limits`] |
//! | epoch position | here: the daemon's epoch set and its announced advances |
//!
//! # Downgrading cleanly, and never granting by silence
//!
//! Three separate downgrades happen in [`welcome`], and each has a rule behind it:
//!
//! - **version.** "The daemon selects the **highest version common** to that range and its
//!   served set, or rejects the connection. The daemon MUST NOT select a version outside
//!   the client's range" — a client offering `3.0..=3.1` against a `3.1` daemon is served
//!   3.1, one offering `3.0..=3.0` is served 3.0, and one offering only a retired major is
//!   refused rather than quietly upgraded.
//! - **authority.** `ServerWelcome.grant` reports what the presented capability *confers*,
//!   which is the registered descriptor. A client that presents `cap_x` and believes it
//!   confers `promote` is told what it actually confers, and "a client MUST NOT infer
//!   authority from anything else" (RFC 0026). No path in this module copies a field from
//!   [`ClientHello`] into the grant.
//! - **features.** "Feature identifiers the daemon supports, intersected with the client's"
//!   and "Unknown identifiers MUST be ignored by the daemon". The intersection is computed
//!   in the daemon's own order and an identifier the daemon does not implement is dropped —
//!   never echoed back, which would read to a client as a grant.
//!
//! The last one is the acceptance criterion "unknown capabilities are never silently
//! granted", and it is a set intersection rather than a filter over the client's list on
//! purpose: echoing is what a filter over the *client's* list would do if the daemon's own
//! set were ever misspelled.
//!
//! # What is deliberately *not* cached
//!
//! > E1 — expiry is evaluated **per request**, not at handshake. A daemon MUST NOT serve a
//! > connection from a grant cached at handshake time; the first request after expiry fails
//! > `CapabilityDenied`.
//! >
//! > — RFC 0027, and R1 says the same for revocation
//!
//! [`welcome`] returns a [`ServerWelcome`] and stores nothing. The authority a *request*
//! runs under is re-decided from [`DaemonState`] by [`admission`](super::admission) on
//! every dispatch, so revoking a capability between two requests denies the second, and an
//! expiry that falls between them denies the second. `tests/daemon_evidence.rs` holds both.
//!
//! A capability failure at the handshake is still the one indistinguishable answer:
//!
//! > A capability refusal MUST NOT distinguish an unregistered token from an expired, a
//! > revoked, or an unauthorized one: every admission failure is one answer (RFC 0027 X1)
//! > […] `detail` MUST NOT vary with which of them occurred.
//! >
//! > — `rule handshake.rejection`
//!
//! [`Refusal`] carries no discriminant for those cases, and the frame's `detail` is
//! [`DENIAL_DETAIL`](super::result::DENIAL_DETAIL) — the same constant a per-request denial
//! carries.
//!
//! # Delegation
//!
//! There is no wire `delegate`, so a delegated capability arrives through
//! [`DaemonState::register_capability`] like every other one, and D1–D9 are enforced where
//! they are *decidable*: [`admission`](super::admission) walks the delegation chain on
//! every request and requires each hop to narrow on level, depth, expiry, all three scope
//! lists, and the profile. A child that exceeds its parent is therefore refused **before**
//! any family runs — the `evidence` family never sees it — which is what
//! `tests/daemon_evidence.rs` asserts through `dispatch` rather than by calling the
//! predicate directly.
//!
//! [`ReferenceStore::mint`]: continuum_workspace::publication::ReferenceStore::mint
//! [`ReferenceStore::revoke`]: continuum_workspace::publication::ReferenceStore::revoke
//! [`handshake::negotiate`]: crate::protocol::handshake::negotiate

use continuum_value::epoch::ProtocolWindow;

use super::state::DaemonState;
use super::{Daemon, result};
use crate::protocol::envelope::EpochSet;
use crate::protocol::handshake::{
    CapabilityDescriptor, ClientHello, EpochAdvanceNotice, Negotiated, NegotiationError,
    ServerLimits, ServerReject, ServerWelcome, negotiate,
};
use crate::protocol::scalar::{ProtocolVersion, Timestamp};
use crate::protocol::vocabulary::{Encoding, ErrorCode};

/// The connection-level settings a daemon answers a [`ClientHello`] from.
///
/// Separate from [`Services`](super::Services) because these are properties of the *daemon
/// deployment* rather than of a connection: a connection is the *result* of applying them
/// to one hello. `protocol.version` and `protocol.majors_served` are two settings and the
/// IDL draws the line between them — the versions a daemon implements and the majors it
/// still serves — so both are here and neither is derived from the other.
#[derive(Debug, Clone)]
pub struct ConnectionPolicy {
    implemented: Vec<ProtocolVersion>,
    window: ProtocolWindow,
    encodings: Vec<Encoding>,
    features: Vec<String>,
    limits: ServerLimits,
    server: String,
    pending_advances: Vec<EpochAdvanceNotice>,
}

impl ConnectionPolicy {
    /// The policy of a daemon implementing `implemented`, serving `window`, and speaking
    /// `encodings`, with no optional features and no announced epoch advance.
    #[must_use]
    pub fn new(
        implemented: Vec<ProtocolVersion>,
        window: ProtocolWindow,
        encodings: Vec<Encoding>,
        limits: ServerLimits,
        server: String,
    ) -> Self {
        Self {
            implemented,
            window,
            encodings,
            features: Vec::new(),
            limits,
            server,
            pending_advances: Vec::new(),
        }
    }

    /// The feature identifiers this daemon implements.
    ///
    /// Only these can appear in a [`ServerWelcome`]: the welcome's list is this set
    /// intersected with the client's, so an identifier the daemon does not name here is
    /// never granted however the client spells it.
    #[must_use]
    pub fn features(mut self, features: Vec<String>) -> Self {
        self.features = features;
        self
    }

    /// Epoch advances announced but not yet applied (plan §4.6).
    #[must_use]
    pub fn pending_advances(mut self, advances: Vec<EpochAdvanceNotice>) -> Self {
        self.pending_advances = advances;
        self
    }

    /// The majors this daemon serves concurrently: N and N−1.
    ///
    /// > The daemon MUST serve protocol majors N and N−1 concurrently […] observable per
    /// > connection as `ServerWelcome.majors_served`.
    /// >
    /// > — RFC 0026, "Version window and negotiation"
    ///
    /// Derived from [`ProtocolWindow::current_major`] rather than configured beside it, so
    /// the list a client reads and the set [`ProtocolWindow::serves`] admits cannot
    /// disagree. Major 0 has no predecessor and reports itself alone.
    #[must_use]
    pub fn majors_served(&self) -> Vec<u32> {
        let current = self.window.current_major();
        match current.checked_sub(1) {
            Some(previous) => vec![current, previous],
            None => vec![current],
        }
    }
}

/// Why a connection was not established, and the frame the client is owed.
///
/// The frame is [`None`] in exactly the two situations `rule handshake.rejection` describes
/// — a client too old to parse a [`ServerReject`], and a failure for which the protocol
/// fixes no code — and collapsing them would lose a distinction the rule draws.
///
/// There is no variant, no field, and no method here that says *which* capability test
/// failed. RFC 0027 X1 makes every admission failure one answer, and a refusal type with
/// one shape is how that stops being a discipline.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Refusal {
    /// The typed refusal frame, or [`None`] where the connection must be closed without
    /// one.
    pub frame: Option<ServerReject>,
}

/// Answer one [`ClientHello`].
///
/// # Errors
///
/// [`Refusal`], carrying the frame the client is owed and nothing that distinguishes one
/// capability failure from another.
pub fn welcome(
    policy: &ConnectionPolicy,
    state: &DaemonState,
    epochs: &EpochSet,
    now: Option<&Timestamp>,
    hello: &ClientHello,
) -> Result<(Negotiated, ServerWelcome), Refusal> {
    let majors = policy.majors_served();

    // 1. Version and encoding, decided by the module that owns the rule. A version failure
    //    precedes the capability check because a client the daemon cannot speak to cannot
    //    be told anything about its capability either.
    let negotiated = negotiate(&policy.implemented, policy.window, &policy.encodings, hello)
        .map_err(|error| Refusal {
            frame: ServerReject::for_negotiation(error, hello, &majors, &version_detail(error)),
        })?;

    // 2. Authority. Registered, unrevoked, bound to the actor that presented it, and
    //    unexpired — four tests, one answer. An unregistered token and a revoked one take
    //    the same path here for the same reason they do in `admission`: they are
    //    indistinguishable by construction rather than by care.
    let grant = state
        .grant(&hello.capability)
        .filter(|grant| grant.descriptor.actor == hello.actor)
        .filter(|grant| unexpired(&grant.descriptor, now))
        .ok_or_else(|| Refusal {
            frame: capability_reject(hello, &majors),
        })?;

    Ok((
        negotiated,
        ServerWelcome {
            protocol_version: negotiated.protocol_version(),
            encoding: negotiated.encoding(),
            majors_served: majors,
            server: policy.server.clone(),
            // The authority the presented capability *actually* confers. Cloned from the
            // daemon's own registry; no field of `hello` reaches it.
            grant: grant.descriptor.clone(),
            limits: policy.limits.clone(),
            features: offered_features(policy, hello),
            epochs: epochs.clone(),
            pending_advances: policy.pending_advances.clone(),
        },
    ))
}

impl Daemon {
    /// Answer one [`ClientHello`] from this daemon's capability registry and epochs.
    ///
    /// The connection this daemon was *built* for is not consulted: a welcome is a
    /// statement about the capability the hello presented, and a daemon serves more than
    /// one connection over its life. Nothing is cached — see this module's documentation on
    /// E1/R1 — so the grant reported here is re-decided by
    /// [`admission`](super::admission) on every subsequent request.
    ///
    /// # Errors
    ///
    /// [`Refusal`], as [`welcome`].
    pub fn welcome(
        &self,
        policy: &ConnectionPolicy,
        hello: &ClientHello,
    ) -> Result<(Negotiated, ServerWelcome), Refusal> {
        welcome(
            policy,
            &self.state,
            &self.services.epochs,
            self.services.now.as_ref(),
            hello,
        )
    }
}

/// The features a welcome may name: the daemon's set intersected with the client's.
///
/// Iterating the *daemon's* list is the load-bearing half. An identifier the daemon does
/// not implement cannot appear in the output whatever the client sent, so
/// "unknown identifiers MUST be ignored" holds by the shape of the loop rather than by a
/// filter that could be inverted. The order is the daemon's too, so two clients offering
/// the same set in different orders are told the same thing (`rule ordering.deterministic`).
fn offered_features(policy: &ConnectionPolicy, hello: &ClientHello) -> Vec<String> {
    let offered = hello.features.value();
    policy
        .features
        .iter()
        .filter(|feature| offered.is_some_and(|list| list.contains(feature)))
        .cloned()
        .collect()
}

/// Whether a descriptor is unexpired against the reading the deployment supplied.
///
/// A daemon with no clock effect cannot judge an expiring capability and refuses it: "a
/// daemon that cannot decide admission fails closed" (RFC 0027). A non-expiring descriptor
/// needs no reading, which is E2's "`expires_at: null` means non-expiring".
fn unexpired(descriptor: &CapabilityDescriptor, now: Option<&Timestamp>) -> bool {
    match descriptor.expires_at.value() {
        Some(expiry) => now.is_some_and(|reading| reading < expiry),
        None => true,
    }
}

/// The refusal frame a capability failure gets.
///
/// `rule handshake.rejection` admits exactly two codes and fixes `retryable` false for
/// both; the detail is the same constant a per-request denial carries, so a client cannot
/// tell a handshake refusal from a dispatch denial by reading it either.
fn capability_reject(hello: &ClientHello, majors: &[u32]) -> Option<ServerReject> {
    // The frame is defined from 3.1; a client whose offer does not reach it cannot parse
    // one, and "a frame the client cannot parse is not a typed refusal".
    if hello.protocol_versions.high < ProtocolVersion::new(3, 1) {
        return None;
    }
    Some(ServerReject {
        code: ErrorCode::CapabilityDenied,
        detail: result::DENIAL_DETAIL.to_owned(),
        retryable: false,
        majors_served: majors.to_vec(),
    })
}

/// The stable, non-interpolated text a version refusal carries (INV-016).
fn version_detail(error: NegotiationError) -> String {
    error.to_string()
}
