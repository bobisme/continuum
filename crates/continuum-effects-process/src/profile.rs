//! The declared fidelity profile: exactly which process-lifecycle semantics this pack
//! models, and which it does not.
//!
//! RFC 0002 "Required profiles" names four fidelity classes, and docs/09 T06 names the
//! threat this module answers: a pack that claims host semantics it does not provide.
//! The controls T06 lists are a versioned fidelity profile, explicit axioms, and "pack
//! can only raise assurance for declared configurations". So the profile here is data,
//! not prose: every process semantic RFC 0002 "Standard packs / Process lifecycle"
//! names, and every process constraint of docs/17 §10, has one row in
//! [`Semantic::ALL`], and each row is either [`Support::Modelled`] or
//! [`Support::Unsupported`] with its reason. A step that asks for an unsupported
//! semantic is refused with [`crate::Refusal::Unsupported`], whose INV-008 reading is
//! `Unsupported`; it is never approximated by a modelled one.
//!
//! The profile claims [`FidelityClass::AdversarialEnvelope`]: it permits every
//! behaviour its own rows do not forbid, and it makes **no host claim**
//! ([`HostQualification::None`]). Nothing here is measured against a real operating
//! system, supervisor or machine, so nothing here may be cited as
//! `platform-qualified`.
//!
//! # What a crash does to in-flight state
//!
//! The network pack deliberately made no claim here and deferred it to this pack
//! (`network/adversarial-v0` rows `endpoint-crash` and `connection-epochs`). This
//! profile states it, row by row and in [`COMPOSITION`]:
//!
//! - the crashed incarnation runs nothing more, not even a finalizer, and none of its
//!   volatile state reaches a later incarnation;
//! - an operation the incarnation started is not undone: its ticket stays pending, it
//!   may still complete any number of steps later, and that completion is fenced;
//! - envelopes in the network pack are untouched: sent envelopes stay in flight;
//! - what stable storage keeps is the storage pack's to state.

use alloc::vec::Vec;
use core::fmt;

/// The profile's stable name. The replicated-register Intent Contract's
/// `fault_model` enables `crash` and `recovery` but names no process profile yet;
/// this is the name a contract cites to bind them to this pack.
pub const PROFILE_NAME: &str = "process/crash-restart-v0";

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

/// What the profile claims about a real host. T06: a pack "can only raise assurance
/// for declared configurations", and this one declares none.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HostQualification {
    /// No host configuration is declared, measured, or conformance-tested. The pack has
    /// a Lab handler only; no production handler exists in this crate.
    None,
}

/// What the profile supplies for docs/17 §9's independence contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum IndependenceClaim {
    /// No independence relation is supplied, so every pair of process events is
    /// dependent ("`Unknown` means dependent", docs/17 §9; "crash event depends on all
    /// volatile operations"). A reducer that uses this pack reduces nothing through it,
    /// so docs/09 T07 has no rule to subvert.
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

/// One process semantic from RFC 0002 "Standard packs / Process lifecycle" or docs/17
/// §10.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Semantic {
    /// Fail-stop crash of one node's incarnation (`Crash`).
    FailStopCrash,
    /// A crash clears the incarnation's volatile state.
    VolatileStateLoss,
    /// Restart in a new incarnation with a fresh epoch (`Restart`).
    RestartNewEpoch,
    /// A completion that arrives after its incarnation crashed (`Complete`, `Fenced`).
    LateCompletion,
    /// External effects an incarnation started that outlive it.
    LeakedExternalEffects,
    /// The crash budget and the exploration bounds.
    ExplorationBounds,
    /// Graceful cancellation of an incarnation.
    GracefulCancellation,
    /// A task panic and its unwinding.
    Panic,
    /// Power loss: several nodes stop at once.
    PowerLoss,
    /// Cleanup left part done.
    PartialCleanup,
    /// Supervisor restart policy.
    SupervisorDecisions,
}

impl Semantic {
    /// Every row of the profile, in declaration order.
    pub const ALL: [Self; 11] = [
        Self::FailStopCrash,
        Self::VolatileStateLoss,
        Self::RestartNewEpoch,
        Self::LateCompletion,
        Self::LeakedExternalEffects,
        Self::ExplorationBounds,
        Self::GracefulCancellation,
        Self::Panic,
        Self::PowerLoss,
        Self::PartialCleanup,
        Self::SupervisorDecisions,
    ];

    /// A stable kebab-case token.
    #[must_use]
    pub const fn token(self) -> &'static str {
        match self {
            Self::FailStopCrash => "fail-stop-crash",
            Self::VolatileStateLoss => "volatile-state-loss",
            Self::RestartNewEpoch => "restart-new-epoch",
            Self::LateCompletion => "late-completion",
            Self::LeakedExternalEffects => "leaked-external-effects",
            Self::ExplorationBounds => "exploration-bounds",
            Self::GracefulCancellation => "graceful-cancellation",
            Self::Panic => "panic",
            Self::PowerLoss => "power-loss",
            Self::PartialCleanup => "partial-cleanup",
            Self::SupervisorDecisions => "supervisor-decisions",
        }
    }

    /// Whether [`PROFILE_NAME`] at [`PROFILE_VERSION`] models this semantic.
    #[must_use]
    pub const fn support(self) -> Support {
        match self {
            Self::FailStopCrash
            | Self::VolatileStateLoss
            | Self::RestartNewEpoch
            | Self::LateCompletion
            | Self::LeakedExternalEffects
            | Self::ExplorationBounds => Support::Modelled,
            Self::GracefulCancellation
            | Self::Panic
            | Self::PowerLoss
            | Self::PartialCleanup
            | Self::SupervisorDecisions => Support::Unsupported,
        }
    }

    /// What the profile models for this row, or why it does not.
    #[must_use]
    pub const fn statement(self) -> &'static str {
        match self {
            Self::FailStopCrash => {
                "Crash stops the node's running incarnation at a step boundary: no code \
                 of that incarnation runs after it, and no finalizer, cancellation \
                 handler or cleanup runs at it; enabled only while the node is up, only \
                 when the configuration declares crashes, and at most the configured \
                 crash budget of times per run"
            }
            Self::VolatileStateLoss => {
                "a crash ends the incarnation and all of its volatile state with it; the \
                 pack holds no program state, so what it enforces is limited to its own \
                 tickets: every ticket stays bound to the incarnation that began it, and \
                 its completion never reaches another; data the old incarnation put in a \
                 network envelope or on storage is those packs' to state, see the \
                 composition table"
            }
            Self::RestartNewEpoch => {
                "Restart starts a new incarnation of a down node whose epoch is one more \
                 than the last, so epochs strictly increase and are never reused; the \
                 epoch advances at Restart, the first moment a new incarnation exists; \
                 the new incarnation starts with no ticket of its own, though the \
                 pending bound still counts the unresolved tickets of dead incarnations; \
                 recovery is the program's own code in it; enabled only when the \
                 configuration declares restart, and nothing guarantees it ever happens"
            }
            Self::LateCompletion => {
                "Complete delivers a pending ticket's completion only to the incarnation \
                 that began it, while that incarnation runs; a completion that arrives \
                 after that incarnation crashed, whether the node is down or restarted, \
                 is Fenced: journalled and discarded, never delivered; Delay is the \
                 adversary's explicit, journalled stutter that holds a pending \
                 completion back one step with no duration, so a completion may be \
                 delayed any number of steps, across a crash and a restart, with no \
                 bound"
            }
            Self::LeakedExternalEffects => {
                "a crash undoes no external effect: an operation begun before it keeps \
                 its ticket pending and may still take effect and complete afterwards, \
                 fenced; what the effect itself does is the owning pack's to state, see \
                 the composition table"
            }
            Self::ExplorationBounds => {
                "the configured bounds on pending tickets, retained bytes (the size of \
                 the journal's canonical encoding) and journal length are exploration \
                 bounds: a step past one is refused as ResourceExhausted, never dropped \
                 silently; the crash budget is part of the declared fault envelope, so a \
                 crash past it is not enabled"
            }
            Self::GracefulCancellation => {
                "cancellation is not process crash: graceful cancellation and its \
                 cleanup belong to the runtime's cancellation protocol (PR 14), and this \
                 pack neither models it nor approximates it by a crash"
            }
            Self::Panic => {
                "a task panic and its unwinding are not modelled, and a panic is not \
                 approximated by a crash of the whole incarnation"
            }
            Self::PowerLoss => {
                "several nodes stopping in one step, and what the power loss does to \
                 storage, are not modelled; crashes of distinct nodes are distinct steps"
            }
            Self::PartialCleanup => {
                "the pack has no cleanup phase: a crash runs no cleanup at all, and \
                 which of the program's own cleanup steps ran before a crash is program \
                 state the pack does not track; no step asks for it"
            }
            Self::SupervisorDecisions => {
                "Restart is an adversary choice with no supervisor policy: restart \
                 intensity, backoff, escalation and giving up are not modelled; no step \
                 asks for them"
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
pub const ASSUMPTIONS: [(&str, &str); 3] = [
    (
        "fail-stop",
        "a crashed incarnation executes nothing further and emits nothing further; no \
         Byzantine or arbitrary behaviour after a crash",
    ),
    (
        "no-eventual-restart",
        "no fairness is assumed: a down node may never restart and a pending ticket may \
         never complete, so no liveness claim may rest on this profile alone; a model \
         that needs restarts states its own fairness",
    ),
    (
        "no-failure-detection",
        "the pack tells no node that another crashed; a node learns of a crash only \
         through the program's own messages",
    ),
];

/// What a crash does to state other packs own, as `(pack, statement)`. The network
/// pack's `endpoint-crash` row defers this to the process pack.
pub const COMPOSITION: [(&str, &str); 3] = [
    (
        "network/adversarial-v0",
        "a crash does not touch the network pack: envelopes sent by or to the node stay \
         in flight and may still be delivered, dropped or duplicated; delivery is to a \
         node address, so an envelope sent before a crash may reach a later \
         incarnation, a self-addressed one included, which carries the old \
         incarnation's data across the crash; a receiver that must tell incarnations \
         apart carries the epoch in the payload and checks it; an envelope delivered \
         while its receiver is down reaches no incarnation and is lost, although the \
         network journal records it as Delivered, not Dropped",
    ),
    (
        "storage",
        "what stable storage keeps across a crash is the storage pack's to state; this \
         profile claims only that no ticket of the crashed incarnation completes into a \
         later one",
    ),
    (
        "runtime",
        "the pack is not bound to the runtime: mapping Crash onto runtime tasks is the \
         replicated-register implementation's work, and a mapping that runs \
         cancellation handlers at a crash realizes graceful cancellation, not a \
         fail-stop crash",
    ),
];

/// docs/17 §8's cancellation contract for the pack's operation, a ticket from `Begin`
/// to `Complete`, as `(aspect, statement)`.
pub const CANCELLATION_CONTRACT: [(&str, &str); 8] = [
    (
        "reservation-point",
        "none in this pack: Begin records a ticket and reserves no resource",
    ),
    (
        "commit-point",
        "Begin; after it the ticket is pending, before it nothing is",
    ),
    (
        "abort-behaviour",
        "a pending ticket cannot be withdrawn; a crash does not abort it: it stays \
         pending and its completion is fenced",
    ),
    ("finalizer-obligations", "none; a crash runs no finalizer"),
    (
        "early-return",
        "not applicable: every step completes in one step, so a crash or cancellation \
         lands before it or after it",
    ),
    (
        "late-completion",
        "a completion of a ticket whose incarnation crashed is Fenced: journalled, \
         never delivered to any incarnation",
    ),
    (
        "idempotence-key",
        "the ticket ordinal; a ticket resolves, Completed or Fenced, at most once",
    ),
    (
        "replay-choice",
        "every step, crash, restart and delay included, is an explicit choice-log \
         entry; crash windows are exactly the boundaries between steps; graceful \
         cancellation is not modelled here, so its cancellation points are the \
         runtime's (PR 14)",
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
pub const CRASH_RESTART_V0: FidelityProfile = FidelityProfile {
    name: PROFILE_NAME,
    version: PROFILE_VERSION,
    class: FidelityClass::AdversarialEnvelope,
    host: HostQualification::None,
    independence: IndependenceClaim::AllDependent,
};

impl FidelityProfile {
    /// The profile's canonical bytes: name, version, class, host claim, independence
    /// claim, every [`Semantic`] row with its support and statement, the assumptions,
    /// the composition table and the cancellation contract. Any change to what the
    /// profile says changes these bytes, so they can be pinned (docs/17 §12: "every
    /// crashpack pins exact pack digest").
    #[must_use]
    pub fn canonical_bytes(&self) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(b"continuum-process-profile\0");
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
        for table in [
            &ASSUMPTIONS[..],
            &COMPOSITION[..],
            &CANCELLATION_CONTRACT[..],
        ] {
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
/// static table, so saturation is unreachable; it saturates rather than wrapping so an
/// impossible overflow could never alias a shorter length.
pub(crate) fn u32_len(len: usize) -> u32 {
    u32::try_from(len).unwrap_or(u32::MAX)
}
