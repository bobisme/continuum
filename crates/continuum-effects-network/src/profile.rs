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
//! Each unsupported row also has one declared case in [`UNSUPPORTED`] (PR-15 /
//! IMPL-04, bn-1oj6): the host behaviour it leaves out, the [`Request`] by which a
//! caller could ask for it — a step, a step in a named state, or no operation at all —
//! and the [`Reliance`] that says what a verdict gives a program that depends on it: a
//! stated assumption, a modelled row that already covers it, the sibling profile that
//! owns it, or nothing, because the pack has no way to express it.
//!
//! The profile claims [`FidelityClass::AdversarialEnvelope`]: it permits every
//! behaviour its own rows do not forbid, and it makes **no host claim**
//! ([`HostQualification::None`]). Nothing here is measured against a real network
//! stack, so nothing here may be cited as `platform-qualified`.

use alloc::vec::Vec;
use core::fmt;

use crate::refusal::Refusal;
use crate::step::StepKind;

/// The current profile's name, `-v1`. The replicated-register scenario and Intent
/// Contract name the frozen `-v0`, [`PROFILE_NAME_V0`]; a contract adopts `-v1` by an
/// intent revision.
pub const PROFILE_NAME: &str = "network/adversarial-v1";

/// The frozen predecessor's name, `network/adversarial-v0`: the profile the replicated-register
/// scenario and Intent Contract named before bn-1oj6. A profile's name identifies its
/// canonical bytes (RFC 0002 correction 1), so the IMPL-04 content could not be
/// published under it; `-v0` stays declared, byte for byte as it was, in [`ADVERSARIAL_V0`].
pub const PROFILE_NAME_V0: &str = "network/adversarial-v0";

/// The profile's semantic version (docs/17 §12: patch is implementation only, minor
/// is additive, major is changed semantics or fidelity).
///
/// 1.0.0 (bn-1oj6, PR-15 / IMPL-04, cr-37bshu) is `network/adversarial-v1`: the `-v0` rows with
/// the declared unsupported cases, [`UNSUPPORTED`], and the `connectionless-send`, `atomic-payload` and `fixed-addressing`
/// assumptions they rest on. No modelled row, step or refusal differs from `-v0`; the
/// declared assumption set does, so it is a new profile under a new name and version,
/// never an edit of the published `-v0` (plan §4.6, ADR-0018).
pub const PROFILE_VERSION: ProfileVersion = ProfileVersion {
    major: 1,
    minor: 0,
    patch: 0,
};

/// The frozen predecessor's version, `-v0` at 0.1.0.
pub const PROFILE_VERSION_V0: ProfileVersion = ProfileVersion {
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

    /// Whether the profiles model this semantic. `-v0` and `-v1` share every row, so
    /// the answer is the same under both.
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
pub const ASSUMPTIONS: [(&str, &str); 5] = [
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
    (
        "connectionless-send",
        "Send to a node address succeeds at once: there is no connection, so the program \
         never sees a blocked or failed send, a reset, a half-open connection or a \
         connection epoch, and program code that handles those host signals is not \
         explored under this profile",
    ),
    (
        "atomic-payload",
        "a payload is delivered whole or not at all: a partial write, a short read, \
         fragmentation and reassembly below the payload are the transport's, outside \
         the profile",
    ),
    (
        "fixed-addressing",
        "node addresses are fixed and each names the node the program means: a Send to a \
         node reaches that node or no node, resolution never fails or blocks a Send, and \
         name resolution, service discovery, and a resolution that is late, stale, wrong \
         or missing are outside the profile",
    ),
];

/// The assumptions `network/adversarial-v0` states: the first 2 of [`ASSUMPTIONS`], taken by
/// construction so the frozen profile cannot drift from them. The entries after them
/// are `-v1`'s additions.
pub const ASSUMPTIONS_V0: &[(&str, &str)] = ASSUMPTIONS.as_slice().split_at(2).0;

/// How a caller could ask this pack for a host behaviour it does not model.
///
/// One exception holds for every request: a network whose journal already holds
/// [`crate::step::MAX_STEPS`] events refuses every step as `BoundReached(Steps)`, an
/// exploration bound, before it looks at the step. That refusal is inconclusive too
/// (INV-008 `ResourceExhausted`), never a success.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Request {
    /// A step of this kind asks for it in every state below the step bound, and is
    /// refused there.
    Step(StepKind),
    /// A step of this kind asks for it only in the state `when` names; in any other
    /// state the step is a modelled one, or refused as the modelled row refuses it.
    StepWhen {
        /// The step.
        step: StepKind,
        /// The state in which the step asks for the unsupported case.
        when: When,
    },
    /// No step, configuration value or other public operation of the pack can ask for
    /// it.
    NoOperation,
}

/// The state in which a [`Request::StepWhen`] step asks for its unsupported case.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum When {
    /// A partition is active, and the `Partition` step is otherwise enabled: its side
    /// names only configured nodes and cuts something, and the partition budget is not
    /// spent. With the budget spent the step is `NotEnabled(PartitionBudgetSpent)`, so
    /// under the replicated-register scenario's budget of one the case is not reachable.
    PartitionActiveWithBudget,
}

impl When {
    /// A stable kebab-case token.
    #[must_use]
    pub const fn token(self) -> &'static str {
        match self {
            Self::PartitionActiveWithBudget => "partition-active-with-budget",
        }
    }
}

/// What a verdict over this profile says about a program that depends on the host
/// behaviour (docs/08 R10, docs/09 T06: a pack must not exclude a relevant behaviour
/// silently).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Reliance {
    /// The stated assumption with this id in [`ASSUMPTIONS`] decides it: a verdict holds
    /// only under that assumption, which an assurance result carries (T06: "explicit
    /// axioms in assurance result").
    Assumed(&'static str),
    /// Every delivery pattern the host behaviour produces is one the named modelled row
    /// already lets the scheduler or the adversary choose, so a verdict already covers
    /// it; only the step that names it is refused. This holds up to the exploration
    /// bounds: an envelope the model holds keeps its in-flight slot, so a later step may
    /// be refused as `ResourceExhausted`, which is inconclusive, where the host would go
    /// on.
    Subsumed(Semantic),
    /// The named sibling profile states it.
    Owner(&'static str),
    /// The pack has no state for it and no operation that asks for it, so a program
    /// cannot depend on it through this pack. A program that reaches it by another
    /// route performs an ambient host effect, which INV-005 refuses and RFC 0002
    /// reports as an `UnmodeledEffect` that downgrades assurance.
    OutsidePack,
}

/// One declared unsupported case: a host behaviour the profile does not model, how a
/// caller could ask for it, and what a program that depends on it gets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct UnsupportedCase {
    /// The profile row, which [`Semantic::support`] marks unsupported.
    pub semantic: Semantic,
    /// The host behaviour, in RFC 0002's and docs/17's terms.
    pub host_behaviour: &'static str,
    /// How a caller could ask for it.
    pub request: Request,
    /// What a verdict says about a program that depends on it.
    pub reliance: Reliance,
}

impl UnsupportedCase {
    /// The refusal a caller that asks for this case gets at the pack boundary:
    /// [`Refusal::Unsupported`] of the row, whose INV-008 reading is `Unsupported`, or
    /// `None` when no operation can ask for it. It is never a success and never a
    /// modelled event in its place. At the step bound the step is refused as
    /// `BoundReached(Steps)` first, as [`Request`] states.
    #[must_use]
    pub const fn refusal(&self) -> Option<Refusal> {
        match self.request {
            Request::Step(_) | Request::StepWhen { .. } => {
                Some(Refusal::Unsupported(self.semantic))
            }
            Request::NoOperation => None,
        }
    }
}

/// The declared unsupported cases: one per [`Support::Unsupported`] row, in
/// [`Semantic::ALL`] order. It is part of the profile's canonical bytes, so a change
/// to it changes the fingerprint and needs a version bump.
pub const UNSUPPORTED: [UnsupportedCase; 13] = [
    UnsupportedCase {
        semantic: Semantic::BoundedDelay,
        host_behaviour: "a network that delivers within a bound: synchrony, or a delay \
                         bound; a timeout that fires when a message is late is the time \
                         pack's, and time/unmodelled-v0 models none",
        request: Request::NoOperation,
        reliance: Reliance::Subsumed(Semantic::UnboundedDelay),
    },
    UnsupportedCase {
        semantic: Semantic::AsymmetricPartition,
        host_behaviour: "a one-way cut: envelopes from one set of nodes to another are \
                         blocked while the reverse direction delivers",
        request: Request::Step(StepKind::OneWayPartition),
        reliance: Reliance::Subsumed(Semantic::UnboundedDelay),
    },
    UnsupportedCase {
        semantic: Semantic::OverlappingPartitions,
        host_behaviour: "two cuts active at once, for example a second partition \
                         before the first heals",
        request: Request::StepWhen {
            step: StepKind::Partition,
            when: When::PartitionActiveWithBudget,
        },
        reliance: Reliance::Subsumed(Semantic::UnboundedDelay),
    },
    UnsupportedCase {
        semantic: Semantic::ConnectionEpochs,
        host_behaviour: "connection identity: a reconnect that starts a new connection \
                         epoch, and data bound to the old one",
        request: Request::NoOperation,
        reliance: Reliance::Assumed("connectionless-send"),
    },
    UnsupportedCase {
        semantic: Semantic::HalfOpen,
        host_behaviour: "a half-open connection: one end believes the connection is \
                         open after the other end has closed or lost it",
        request: Request::NoOperation,
        reliance: Reliance::Assumed("connectionless-send"),
    },
    UnsupportedCase {
        semantic: Semantic::Backpressure,
        host_behaviour: "sender backpressure and capacity: a send that blocks, or \
                         fails, because a buffer or a window is full",
        request: Request::NoOperation,
        reliance: Reliance::Assumed("connectionless-send"),
    },
    UnsupportedCase {
        semantic: Semantic::Corruption,
        host_behaviour: "payload corruption in flight, detected by a checksum or not",
        request: Request::Step(StepKind::Corrupt),
        reliance: Reliance::Assumed("network-no-forgery"),
    },
    UnsupportedCase {
        semantic: Semantic::Forgery,
        host_behaviour: "an envelope nobody sent, or a sender identity that is not the \
                         real sender: no authentication is modelled",
        request: Request::Step(StepKind::Forge),
        reliance: Reliance::Assumed("network-no-forgery"),
    },
    UnsupportedCase {
        semantic: Semantic::ServiceDiscovery,
        host_behaviour: "DNS and service discovery: a name that resolves late, to a \
                         stale address, or not at all, so an envelope reaches the wrong \
                         node or none",
        request: Request::NoOperation,
        reliance: Reliance::Assumed("fixed-addressing"),
    },
    UnsupportedCase {
        semantic: Semantic::Framing,
        host_behaviour: "MTU, framing and fragmentation: a byte-level partial write, a \
                         short read, or a message split or merged by the transport",
        request: Request::NoOperation,
        reliance: Reliance::Assumed("atomic-payload"),
    },
    UnsupportedCase {
        semantic: Semantic::ConnectionReset,
        host_behaviour: "a connection reset: in-flight data is lost and an end is told \
                         so by an error",
        request: Request::Step(StepKind::ConnectionReset),
        reliance: Reliance::Assumed("connectionless-send"),
    },
    UnsupportedCase {
        semantic: Semantic::EndpointCrash,
        host_behaviour: "an endpoint that crashes, and what the crash does to envelopes \
                         in flight to or from it",
        request: Request::Step(StepKind::CrashEndpoint),
        reliance: Reliance::Owner("process/crash-restart-v0"),
    },
    UnsupportedCase {
        semantic: Semantic::RecallInFlight,
        host_behaviour: "a sender that withdraws an envelope already in flight, for \
                         example when its task is cancelled",
        request: Request::Step(StepKind::Recall),
        reliance: Reliance::Subsumed(Semantic::UnboundedDelay),
    },
];

impl Semantic {
    /// This row's declared unsupported case, or `None` for a modelled row.
    #[must_use]
    pub fn unsupported_case(self) -> Option<UnsupportedCase> {
        UNSUPPORTED
            .iter()
            .copied()
            .find(|case| case.semantic == self)
    }
}

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
///
/// Only the assumptions and the declared unsupported cases vary between profiles. The
/// rows ([`Semantic::support`], [`Semantic::statement`]) and the composition and
/// cancellation tables are shared, so a future profile that changes a row needs those
/// to become per-profile first; until then the registry test fails such an edit, and
/// the fix it asks for is that refactor plus a new name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FidelityProfile {
    /// Its name, which identifies its canonical bytes (RFC 0002 correction 1).
    pub name: &'static str,
    /// Its version.
    pub version: ProfileVersion,
    /// The RFC 0002 class the profile claims.
    pub class: FidelityClass,
    /// What it claims about a real host.
    pub host: HostQualification,
    /// What it supplies for independence.
    pub independence: IndependenceClaim,
    /// The assumptions it states.
    pub assumptions: &'static [(&'static str, &'static str)],
    /// Its declared unsupported cases; `None` for `-v0`, which declared none, so its
    /// canonical bytes stay exactly what they were.
    pub unsupported: Option<&'static [UnsupportedCase]>,
}

/// The frozen `network/adversarial-v0` at 0.1.0, byte for byte the profile this crate published
/// before bn-1oj6: its rows, its 2 assumptions, and no declared unsupported cases.
pub const ADVERSARIAL_V0: FidelityProfile = FidelityProfile {
    name: PROFILE_NAME_V0,
    version: PROFILE_VERSION_V0,
    class: FidelityClass::AdversarialEnvelope,
    host: HostQualification::None,
    independence: IndependenceClaim::AllDependent,
    assumptions: ASSUMPTIONS_V0,
    unsupported: None,
};

/// The current `network/adversarial-v1` at 1.0.0: the same rows, every assumption, and the declared
/// unsupported cases. The Lab handler journals under this profile.
pub const ADVERSARIAL_V1: FidelityProfile = FidelityProfile {
    name: PROFILE_NAME,
    version: PROFILE_VERSION,
    class: FidelityClass::AdversarialEnvelope,
    host: HostQualification::None,
    independence: IndependenceClaim::AllDependent,
    assumptions: &ASSUMPTIONS,
    unsupported: Some(&UNSUPPORTED),
};

/// Every profile this crate declares, oldest first. The one Lab handler implements the
/// rows both share; they differ only in what they declare about the host.
pub const PROFILES: [FidelityProfile; 2] = [ADVERSARIAL_V0, ADVERSARIAL_V1];

impl FidelityProfile {
    /// The profile's canonical bytes: name, version, class, host claim, independence
    /// claim, every [`Semantic`] row with its support and statement, the assumptions,
    /// the cancellation contract, and the declared unsupported cases. Any change to what
    /// the profile says changes these bytes, so they can be pinned (docs/17 §12: "every
    /// crashpack pins exact pack digest").
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
        for table in [self.assumptions, &CANCELLATION_CONTRACT[..]] {
            out.extend_from_slice(&u32_len(table.len()).to_be_bytes());
            for (id, statement) in table {
                put_str(&mut out, id);
                put_str(&mut out, statement);
            }
        }
        if let Some(cases) = self.unsupported {
            Self::put_unsupported(&mut out, cases);
        }
        out
    }

    /// The declared unsupported cases' canonical bytes: a count, then each case.
    fn put_unsupported(out: &mut Vec<u8>, cases: &[UnsupportedCase]) {
        out.extend_from_slice(&u32_len(cases.len()).to_be_bytes());
        for case in cases {
            put_str(out, case.semantic.token());
            put_str(out, case.host_behaviour);
            match case.request {
                Request::Step(step) => {
                    put_str(out, "step");
                    put_str(out, step.as_str());
                }
                Request::StepWhen { step, when } => {
                    put_str(out, "step-when");
                    put_str(out, step.as_str());
                    put_str(out, when.token());
                }
                Request::NoOperation => put_str(out, "no-operation"),
            }
            match case.reliance {
                Reliance::Assumed(id) => {
                    put_str(out, "assumed");
                    put_str(out, id);
                }
                Reliance::Subsumed(row) => {
                    put_str(out, "subsumed");
                    put_str(out, row.token());
                }
                Reliance::Owner(profile) => {
                    put_str(out, "owner");
                    put_str(out, profile);
                }
                Reliance::OutsidePack => put_str(out, "outside-pack"),
            }
        }
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
