//! The declared fidelity profile: exactly which network semantics this pack models,
//! and which it does not.
//!
//! RFC 0002 "Required profiles" names four fidelity classes, and docs/09 T06 names the
//! threat this module answers: a pack that claims host semantics it does not provide.
//! The controls T06 lists are a versioned fidelity profile, explicit axioms, and "pack
//! can only raise assurance for declared configurations". So the profile here is data,
//! not prose: every network semantic RFC 0002 "Standard packs / Network" and docs/17
//! §7 name has one row in [`Semantic::ALL`], and each row is either
//! [`Support::Modelled`] or [`Support::Unsupported`] with its reason. A step that asks
//! for an unsupported semantic is refused with
//! [`crate::Refusal::Unsupported`], whose INV-008 reading is `Unsupported`; it is
//! never approximated by a modelled one.
//!
//! The profile claims [`FidelityClass::AdversarialEnvelope`]: it permits every
//! behaviour its own rows do not forbid, and it makes **no host claim**
//! ([`HostQualification::None`]). Nothing here is measured against a real network
//! stack, so nothing here may be cited as `platform-qualified`.

use alloc::vec::Vec;
use core::fmt;

/// The profile's stable name, as `replicated_register.scenario.toml` `[network]
/// profile` and the replicated-register Intent Contract's `fault_model.profiles` spell
/// it.
pub const PROFILE_NAME: &str = "network/adversarial-v0";

/// The profile's semantic version (docs/17 §12: patch is implementation only, minor
/// is additive, major is changed semantics or fidelity).
pub const PROFILE_VERSION: ProfileVersion = ProfileVersion {
    major: 0,
    minor: 1,
    patch: 0,
};

/// A `major.minor.patch` profile version.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ProfileVersion {
    /// Changed semantics or fidelity.
    pub major: u16,
    /// An additive operation or fault; old behaviour unchanged.
    pub minor: u16,
    /// Implementation only; same semantics.
    pub patch: u16,
}

impl fmt::Display for ProfileVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

/// RFC 0002's four fidelity classes, with the spelling of
/// `intent-contract.schema.json`'s `fidelity_profile` enum.
///
/// The classes are not ordered: "a refinement proof or conformance campaign
/// establishes relationships" (RFC 0002), so this type deliberately has no `Ord`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FidelityClass {
    /// A simple mathematical abstraction.
    Ideal,
    /// Matches a documented service or API contract.
    Contractual,
    /// Backed by measurements and conformance tests for one deployment.
    PlatformQualified,
    /// Permits every behaviour not forbidden by declared constraints.
    AdversarialEnvelope,
}

impl FidelityClass {
    /// The schema spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Ideal => "ideal",
            Self::Contractual => "contractual",
            Self::PlatformQualified => "platform-qualified",
            Self::AdversarialEnvelope => "adversarial-envelope",
        }
    }
}

/// What the profile claims about a real host network. T06: a pack "can only raise
/// assurance for declared configurations", and this one declares none.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HostQualification {
    /// No host configuration is declared, measured, or conformance-tested. The pack has
    /// a Lab handler only; no production handler exists in this crate.
    None,
}

/// What the profile supplies for docs/17 §9's independence contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum IndependenceClaim {
    /// No independence relation is supplied, so every pair of network events is
    /// dependent ("`Unknown` means dependent", docs/17 §9). A reducer that uses this
    /// pack reduces nothing through it, so docs/09 T07 has no rule to subvert.
    AllDependent,
}

/// Whether the profile models a semantic.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Support {
    /// The Lab handler implements it, as [`Semantic::statement`] says.
    Modelled,
    /// The Lab handler does not implement it. A step that needs it is refused with
    /// [`crate::Refusal::Unsupported`].
    Unsupported,
}

/// One network semantic from RFC 0002 "Standard packs / Network" or docs/17 §7.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Semantic {
    /// Message loss (`Drop`).
    Loss,
    /// Message duplication (`Duplicate`).
    Duplication,
    /// Delivery out of send order on one link.
    Reordering,
    /// Delay with no bound and no duration (`Delay`).
    UnboundedDelay,
    /// One symmetric two-sided partition (`Partition`, `Heal`).
    SymmetricPartition,
    /// The in-flight exploration bound.
    InFlightBound,
    /// A bound on delay, or any timing assumption.
    BoundedDelay,
    /// A one-way cut.
    AsymmetricPartition,
    /// Two partitions active at once.
    OverlappingPartitions,
    /// Connection epochs and connection identity.
    ConnectionEpochs,
    /// Half-open connections.
    HalfOpen,
    /// Sender backpressure.
    Backpressure,
    /// Payload corruption.
    Corruption,
    /// Injection of an envelope nobody sent (Byzantine forgery).
    Forgery,
    /// DNS and service discovery.
    ServiceDiscovery,
    /// MTU, framing, and fragmentation.
    Framing,
    /// Connection reset.
    ConnectionReset,
    /// Endpoint crash as a network event.
    EndpointCrash,
    /// Recalling an envelope already in flight, for example on sender cancellation.
    RecallInFlight,
}

impl Semantic {
    /// Every row of the profile, in declaration order.
    pub const ALL: [Self; 19] = [
        Self::Loss,
        Self::Duplication,
        Self::Reordering,
        Self::UnboundedDelay,
        Self::SymmetricPartition,
        Self::InFlightBound,
        Self::BoundedDelay,
        Self::AsymmetricPartition,
        Self::OverlappingPartitions,
        Self::ConnectionEpochs,
        Self::HalfOpen,
        Self::Backpressure,
        Self::Corruption,
        Self::Forgery,
        Self::ServiceDiscovery,
        Self::Framing,
        Self::ConnectionReset,
        Self::EndpointCrash,
        Self::RecallInFlight,
    ];

    /// A stable kebab-case token.
    #[must_use]
    pub const fn token(self) -> &'static str {
        match self {
            Self::Loss => "loss",
            Self::Duplication => "duplication",
            Self::Reordering => "reordering",
            Self::UnboundedDelay => "unbounded-delay",
            Self::SymmetricPartition => "symmetric-partition",
            Self::InFlightBound => "in-flight-bound",
            Self::BoundedDelay => "bounded-delay",
            Self::AsymmetricPartition => "asymmetric-partition",
            Self::OverlappingPartitions => "overlapping-partitions",
            Self::ConnectionEpochs => "connection-epochs",
            Self::HalfOpen => "half-open",
            Self::Backpressure => "backpressure",
            Self::Corruption => "corruption",
            Self::Forgery => "forgery",
            Self::ServiceDiscovery => "service-discovery",
            Self::Framing => "framing",
            Self::ConnectionReset => "connection-reset",
            Self::EndpointCrash => "endpoint-crash",
            Self::RecallInFlight => "recall-in-flight",
        }
    }

    /// Whether [`PROFILE_NAME`] at [`PROFILE_VERSION`] models this semantic.
    #[must_use]
    pub const fn support(self) -> Support {
        match self {
            Self::Loss
            | Self::Duplication
            | Self::Reordering
            | Self::UnboundedDelay
            | Self::SymmetricPartition
            | Self::InFlightBound => Support::Modelled,
            Self::BoundedDelay
            | Self::AsymmetricPartition
            | Self::OverlappingPartitions
            | Self::ConnectionEpochs
            | Self::HalfOpen
            | Self::Backpressure
            | Self::Corruption
            | Self::Forgery
            | Self::ServiceDiscovery
            | Self::Framing
            | Self::ConnectionReset
            | Self::EndpointCrash
            | Self::RecallInFlight => Support::Unsupported,
        }
    }

    /// What the profile models for this row, or why it does not.
    #[must_use]
    pub const fn statement(self) -> &'static str {
        match self {
            Self::Loss => {
                "Drop removes one in-flight copy of an envelope; enabled only when the \
                 configuration allows loss"
            }
            Self::Duplication => {
                "Duplicate adds one in-flight copy of an envelope, under the in-flight \
                 bound; enabled only when the configuration allows duplication; a copy \
                 keeps its envelope's send-order position, so on a FIFO link a later \
                 envelope waits until every copy of an earlier one is consumed"
            }
            Self::Reordering => {
                "with reordering allowed any in-flight envelope may be delivered; without \
                 it each directed link is FIFO by send order, and a duplicated copy is not \
                 re-queued behind later envelopes, so a FIFO link never delivers e0, e1, e0"
            }
            Self::UnboundedDelay => {
                "an envelope may stay in flight across any number of steps; Delay is an \
                 explicit, journalled stutter with no duration, and nothing guarantees \
                 eventual delivery"
            }
            Self::SymmetricPartition => {
                "Partition splits the nodes into two non-empty sides and blocks delivery \
                 across them in both directions; in-flight envelopes stay in flight and \
                 Heal restores delivery; the configuration bounds the number of \
                 partitions"
            }
            Self::InFlightBound => {
                "the configured bounds on in-flight copies, payload bytes, retained bytes \
                 (the size of the journal's canonical encoding, which holds every payload \
                 once) and journal length are exploration bounds, not host backpressure: a step \
                 past one is refused as ResourceExhausted, never silently dropped"
            }
            Self::BoundedDelay => {
                "the profile has no time model, so no delay bound, synchrony, or \
                 timeout assumption can be stated"
            }
            Self::AsymmetricPartition => "only symmetric two-sided partitions are modelled",
            Self::OverlappingPartitions => {
                "at most one partition is active at a time; a second Partition before \
                 Heal is refused"
            }
            Self::ConnectionEpochs => {
                "delivery is to a node address, not to a connection or a process \
                 incarnation; the process pack owns incarnation epochs"
            }
            Self::HalfOpen => "there are no connections, so there is no half-open state",
            Self::Backpressure => {
                "Send never blocks; flow control and sender backpressure are not modelled"
            }
            Self::Corruption => {
                "payloads are delivered bit-exact; corruption and checksum failure are not \
                 modelled"
            }
            Self::Forgery => {
                "every delivered envelope was sent: Byzantine injection is outside the \
                 profile, which relies on the network-no-forgery assumption"
            }
            Self::ServiceDiscovery => {
                "node addresses are fixed; DNS and discovery are not modelled"
            }
            Self::Framing => {
                "a payload is one atomic unit; MTU, framing and fragmentation are not \
                 modelled, and no step asks for them; the configured payload byte bound is \
                 an exploration bound, and a larger payload is refused as ResourceExhausted"
            }
            Self::ConnectionReset => "there are no connections, so there is no reset",
            Self::EndpointCrash => {
                "the network pack has no crash event; what a crash does to envelopes in \
                 flight to or from a node is the process pack's to state"
            }
            Self::RecallInFlight => {
                "a sent envelope cannot be recalled; Send is one atomic commit point"
            }
        }
    }
}

impl fmt::Display for Semantic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.token())
    }
}

/// The assumptions this profile relies on and states explicitly (T06: "explicit
/// axioms in assurance result"), as `(id, statement)`.
pub const ASSUMPTIONS: [(&str, &str); 2] = [
    (
        "network-no-forgery",
        "every delivered envelope was sent, from its sender to its receiver, with its \
         payload unchanged",
    ),
    (
        "no-eventual-delivery",
        "no fairness is assumed: an envelope may be delayed forever, so no liveness claim \
         may rest on this profile alone",
    ),
];

/// docs/17 §8's cancellation contract for the pack's one operation, `Send`, as
/// `(aspect, statement)`.
pub const CANCELLATION_CONTRACT: [(&str, &str); 8] = [
    (
        "reservation-point",
        "none in this pack: a sender's reservation is the caller's own obligation",
    ),
    (
        "commit-point",
        "Send; after it the envelope is in flight, before it nothing is",
    ),
    (
        "abort-behaviour",
        "nothing to abort: Send has no partial state",
    ),
    ("finalizer-obligations", "none"),
    (
        "early-return",
        "not applicable: Send completes in one step, so cancellation lands before it or \
         after it",
    ),
    (
        "late-completion",
        "an envelope sent before cancellation stays in flight and may still be delivered",
    ),
    (
        "idempotence-key",
        "the envelope ordinal; duplicated copies share it",
    ),
    (
        "replay-choice",
        "every step is an explicit choice-log entry; cancellation points are exactly the \
         boundaries between steps",
    ),
];

/// The declared profile, as one value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FidelityProfile {
    /// [`PROFILE_NAME`].
    pub name: &'static str,
    /// [`PROFILE_VERSION`].
    pub version: ProfileVersion,
    /// The RFC 0002 class the profile claims.
    pub class: FidelityClass,
    /// What it claims about a real host.
    pub host: HostQualification,
    /// What it supplies for independence.
    pub independence: IndependenceClaim,
}

/// The one profile this crate declares.
pub const ADVERSARIAL_V0: FidelityProfile = FidelityProfile {
    name: PROFILE_NAME,
    version: PROFILE_VERSION,
    class: FidelityClass::AdversarialEnvelope,
    host: HostQualification::None,
    independence: IndependenceClaim::AllDependent,
};

impl FidelityProfile {
    /// The profile's canonical bytes: name, version, class, host claim, independence
    /// claim, every [`Semantic`] row with its support and statement, the assumptions,
    /// and the cancellation contract. Any change to what the profile says changes these
    /// bytes, so they can be pinned (docs/17 §12: "every crashpack pins exact pack
    /// digest").
    #[must_use]
    pub fn canonical_bytes(&self) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(b"continuum-network-profile\0");
        put_str(&mut out, self.name);
        out.extend_from_slice(&self.version.major.to_be_bytes());
        out.extend_from_slice(&self.version.minor.to_be_bytes());
        out.extend_from_slice(&self.version.patch.to_be_bytes());
        put_str(&mut out, self.class.as_str());
        put_str(
            &mut out,
            match self.host {
                HostQualification::None => "host-none",
            },
        );
        put_str(
            &mut out,
            match self.independence {
                IndependenceClaim::AllDependent => "all-dependent",
            },
        );
        out.extend_from_slice(&u32_len(Semantic::ALL.len()).to_be_bytes());
        for semantic in Semantic::ALL {
            put_str(&mut out, semantic.token());
            put_str(
                &mut out,
                match semantic.support() {
                    Support::Modelled => "modelled",
                    Support::Unsupported => "unsupported",
                },
            );
            put_str(&mut out, semantic.statement());
        }
        for table in [&ASSUMPTIONS[..], &CANCELLATION_CONTRACT[..]] {
            out.extend_from_slice(&u32_len(table.len()).to_be_bytes());
            for (id, statement) in table {
                put_str(&mut out, id);
                put_str(&mut out, statement);
            }
        }
        out
    }
}

pub(crate) fn put_str(out: &mut Vec<u8>, text: &str) {
    out.extend_from_slice(&u32_len(text.len()).to_be_bytes());
    out.extend_from_slice(text.as_bytes());
}

/// A length as a `u32`. Every length encoded here is bounded far below `u32::MAX` by a
/// configuration cap or a static table, so saturation is unreachable; it saturates
/// rather than wrapping so an impossible overflow could never alias a shorter length.
pub(crate) fn u32_len(len: usize) -> u32 {
    u32::try_from(len).unwrap_or(u32::MAX)
}
