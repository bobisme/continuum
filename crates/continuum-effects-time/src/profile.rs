//! The declared fidelity profile: every clock semantic RFC 0002 names, and the fact that
//! this pack models none of them.
//!
//! RFC 0002 "Standard packs / Clock" says a clock pack must distinguish seven
//! semantics, and docs/09 T06 names the threat this module answers: a pack that claims
//! host semantics it does not provide. PR 15 implements only the profiles the
//! replicated register needs, and the replicated register uses no clock: the network
//! profile `network/adversarial-v0` has no time model (its `bounded-delay` row), and
//! the process and storage profiles hold every completion back with no duration. So
//! this pack declares, as data, that it models no clock semantic at all. Every row of
//! [`Semantic::ALL`] is [`Support::Unsupported`] with its reason, and every row has one
//! declared case in [`UNSUPPORTED`] (PR-15 / IMPL-04, bn-1oj6).
//!
//! The pack has no operation, no step and no handler, so no caller can ask it for a
//! clock reading, a timer or a lease: every case's [`Request`] is
//! [`Request::NoOperation`]. A program that depends on a clock cannot get one through
//! this pack, and a program that reads one by another route performs an ambient host
//! effect, which INV-005 refuses and RFC 0002 reports as an `UnmodeledEffect` that
//! downgrades assurance ([`Reliance::OutsidePack`]).
//!
//! The profile claims [`FidelityClass::AdversarialEnvelope`] only in the trivial sense
//! that it forbids nothing, because it declares no operation, and the profile says so
//! as data: [`FidelityProfile::operations`] is [`Operations::None`], in the canonical
//! bytes, so no consumer can read the class as covering any clock behaviour. It makes
//! **no host claim** ([`HostQualification::None`]). Nothing here may be cited as
//! `platform-qualified`.

use alloc::vec::Vec;
use core::fmt;

/// The profile's stable name.
pub const PROFILE_NAME: &str = "time/unmodelled-v0";

/// The profile's semantic version (docs/17 §12: patch is implementation only, minor
/// is additive, major is changed semantics or fidelity).
///
/// 0.1.0 (bn-1oj6, PR-15 / IMPL-04) is the first version: the crate goes from a
/// scaffold with no contract to a declared profile with every clock semantic
/// unsupported.
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

/// What the profile claims about a real host clock. T06: a pack "can only raise
/// assurance for declared configurations", and this one declares none.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HostQualification {
    /// No host clock is declared, measured, or conformance-tested. The pack has no
    /// handler of any kind.
    None,
}

/// Whether the profile models a semantic.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Support {
    /// The pack implements it, as [`Semantic::statement`] says. No row of this profile
    /// is modelled.
    Modelled,
    /// The pack does not implement it, and no operation can ask for it.
    Unsupported,
}

/// One clock semantic from RFC 0002 "Standard packs / Clock".
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Semantic {
    /// Monotonic local time.
    MonotonicLocalTime,
    /// Wall time.
    WallTime,
    /// Drift and skew.
    DriftAndSkew,
    /// Discontinuities.
    Discontinuities,
    /// Uncertainty intervals.
    UncertaintyIntervals,
    /// Lease assumptions.
    LeaseAssumptions,
    /// Timer coalescing and delayed wakeup.
    TimerCoalescingAndDelayedWakeup,
}

impl Semantic {
    /// Every row of the profile, in RFC 0002's order.
    pub const ALL: [Self; 7] = [
        Self::MonotonicLocalTime,
        Self::WallTime,
        Self::DriftAndSkew,
        Self::Discontinuities,
        Self::UncertaintyIntervals,
        Self::LeaseAssumptions,
        Self::TimerCoalescingAndDelayedWakeup,
    ];

    /// A stable kebab-case token.
    #[must_use]
    pub const fn token(self) -> &'static str {
        match self {
            Self::MonotonicLocalTime => "monotonic-local-time",
            Self::WallTime => "wall-time",
            Self::DriftAndSkew => "drift-and-skew",
            Self::Discontinuities => "discontinuities",
            Self::UncertaintyIntervals => "uncertainty-intervals",
            Self::LeaseAssumptions => "lease-assumptions",
            Self::TimerCoalescingAndDelayedWakeup => "timer-coalescing-and-delayed-wakeup",
        }
    }

    /// Whether [`PROFILE_NAME`] at [`PROFILE_VERSION`] models this semantic. It models
    /// none.
    #[must_use]
    pub const fn support(self) -> Support {
        match self {
            Self::MonotonicLocalTime
            | Self::WallTime
            | Self::DriftAndSkew
            | Self::Discontinuities
            | Self::UncertaintyIntervals
            | Self::LeaseAssumptions
            | Self::TimerCoalescingAndDelayedWakeup => Support::Unsupported,
        }
    }

    /// Why the profile does not model this row.
    #[must_use]
    pub const fn statement(self) -> &'static str {
        match self {
            Self::MonotonicLocalTime => {
                "the pack has no clock: no monotonic reading, no duration and no \
                 ordering by time is modelled, and no operation asks for one"
            }
            Self::WallTime => {
                "the pack has no wall clock: no calendar time, time zone or mapping from \
                 monotonic to wall time is modelled, and no operation asks for one"
            }
            Self::DriftAndSkew => {
                "with no clock there is no rate: drift of one clock and skew between the \
                 clocks of distinct nodes are not modelled"
            }
            Self::Discontinuities => {
                "with no clock there is nothing to jump: leap seconds, wall-clock steps and \
                 a monotonic clock that goes backwards are not modelled, and docs/17 \
                 §10's constraint that a clock jump changes the wall mapping and not \
                 monotonic logical time has no state to constrain"
            }
            Self::UncertaintyIntervals => {
                "no bounded-uncertainty reading, such as an earliest and latest time, is \
                 modelled"
            }
            Self::LeaseAssumptions => {
                "no lease, lease expiry or safety argument that rests on bounded clock \
                 error is modelled; the network profile states no delay bound either"
            }
            Self::TimerCoalescingAndDelayedWakeup => {
                "no timer, timeout, sleep or wakeup is modelled, so neither coalescing nor \
                 a late wakeup is; the network, process and storage profiles hold every \
                 completion back with an explicit Delay step that has no duration"
            }
        }
    }
}

impl fmt::Display for Semantic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.token())
    }
}

/// How a caller could ask this pack for a host behaviour it does not model. The pack
/// has no operation, so there is one answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Request {
    /// No step, configuration value or other public operation of the pack can ask for
    /// it: the pack has none.
    NoOperation,
}

/// What a verdict over this profile says about a program that depends on the host
/// behaviour (docs/08 R10, docs/09 T06: a pack must not exclude a relevant behaviour
/// silently).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Reliance {
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

/// The declared unsupported cases: one per row, in [`Semantic::ALL`] order. It is part
/// of the profile's canonical bytes, so a change to it changes the fingerprint and needs
/// a version bump.
pub const UNSUPPORTED: [UnsupportedCase; 7] = [
    UnsupportedCase {
        semantic: Semantic::MonotonicLocalTime,
        host_behaviour: "a monotonic local clock: a reading, an elapsed duration, and \
                         ordering events by it",
        request: Request::NoOperation,
        reliance: Reliance::OutsidePack,
    },
    UnsupportedCase {
        semantic: Semantic::WallTime,
        host_behaviour: "wall time: calendar time, time zones, and the mapping from \
                         monotonic to wall time",
        request: Request::NoOperation,
        reliance: Reliance::OutsidePack,
    },
    UnsupportedCase {
        semantic: Semantic::DriftAndSkew,
        host_behaviour: "clock drift on one node, and clock skew between the clocks of \
                         distinct nodes",
        request: Request::NoOperation,
        reliance: Reliance::OutsidePack,
    },
    UnsupportedCase {
        semantic: Semantic::Discontinuities,
        host_behaviour: "clock discontinuities: a leap second, a wall-clock step, and a \
                         monotonic clock that goes backwards",
        request: Request::NoOperation,
        reliance: Reliance::OutsidePack,
    },
    UnsupportedCase {
        semantic: Semantic::UncertaintyIntervals,
        host_behaviour: "a clock reading with bounded uncertainty, as an earliest and a \
                         latest possible time",
        request: Request::NoOperation,
        reliance: Reliance::OutsidePack,
    },
    UnsupportedCase {
        semantic: Semantic::LeaseAssumptions,
        host_behaviour: "a lease, its expiry, and a safety argument that rests on a bound \
                         on clock error",
        request: Request::NoOperation,
        reliance: Reliance::OutsidePack,
    },
    UnsupportedCase {
        semantic: Semantic::TimerCoalescingAndDelayedWakeup,
        host_behaviour: "a timer or a timeout: coalesced timers, and a wakeup that comes \
                         late or early",
        request: Request::NoOperation,
        reliance: Reliance::OutsidePack,
    },
];

impl Semantic {
    /// This row's declared unsupported case.
    #[must_use]
    pub fn unsupported_case(self) -> Option<UnsupportedCase> {
        UNSUPPORTED
            .iter()
            .copied()
            .find(|case| case.semantic == self)
    }
}

/// What the profile's operations are. The class alone would read as "adversarial clock
/// behaviour is covered"; this field says, in the profile's canonical bytes, that the
/// class covers nothing, because no operation exists to cover.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Operations {
    /// The profile declares no operation, step or handler: the class forbids nothing
    /// only because nothing can be asked, and no verdict covers any clock behaviour.
    None,
}

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
    /// Its operations: none.
    pub operations: Operations,
}

/// The one profile this crate declares.
pub const UNMODELLED_V0: FidelityProfile = FidelityProfile {
    name: PROFILE_NAME,
    version: PROFILE_VERSION,
    class: FidelityClass::AdversarialEnvelope,
    host: HostQualification::None,
    operations: Operations::None,
};

impl FidelityProfile {
    /// The profile's canonical bytes: name, version, class, host claim, operations, every
    /// [`Semantic`] row with its support and statement, and the declared unsupported
    /// cases. Any change to what the profile says changes these bytes, so they can be
    /// pinned (docs/17 §12: "every crashpack pins exact pack digest").
    #[must_use]
    pub fn canonical_bytes(&self) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(b"continuum-time-profile\0");
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
            match self.operations {
                Operations::None => "operations-none",
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
        out.extend_from_slice(&u32_len(UNSUPPORTED.len()).to_be_bytes());
        for case in &UNSUPPORTED {
            put_str(&mut out, case.semantic.token());
            put_str(&mut out, case.host_behaviour);
            put_str(
                &mut out,
                match case.request {
                    Request::NoOperation => "no-operation",
                },
            );
            put_str(
                &mut out,
                match case.reliance {
                    Reliance::OutsidePack => "outside-pack",
                },
            );
        }
        out
    }
}

fn put_str(out: &mut Vec<u8>, text: &str) {
    out.extend_from_slice(&u32_len(text.len()).to_be_bytes());
    out.extend_from_slice(text.as_bytes());
}

/// A length as a `u32`. Every length encoded here is bounded far below `u32::MAX` by a
/// static table, so saturation is unreachable; it saturates rather than wrapping so an
/// impossible overflow could never alias a shorter length.
fn u32_len(len: usize) -> u32 {
    u32::try_from(len).unwrap_or(u32::MAX)
}
