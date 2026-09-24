//! The declared fidelity profile: exactly which storage semantics this pack models, and
//! which it does not.
//!
//! RFC 0002 "Required profiles" names four fidelity classes, and docs/09 T06 names the
//! threat this module answers: a pack that claims host semantics it does not provide.
//! The controls T06 lists are a versioned fidelity profile, explicit axioms, and "pack
//! can only raise assurance for declared configurations". So the profile here is data,
//! not prose: every storage semantic RFC 0002 "Standard packs / Storage" names, every
//! claim, fault and exclusion of docs/17 §6's durable-storage example, and every
//! storage constraint of docs/17 §10 maps to a row of [`Semantic::ALL`], and each row
//! is either [`Support::Modelled`] or [`Support::Unsupported`] with its reason. A step
//! that asks for an unsupported semantic is refused with
//! [`crate::Refusal::Unsupported`], whose INV-008 reading is `Unsupported`; it is never
//! approximated by a modelled one.
//!
//! The profile claims [`FidelityClass::AdversarialEnvelope`]: it permits every
//! behaviour its own rows and assumptions do not forbid, and it makes **no host claim**
//! ([`HostQualification::None`]). Nothing here is measured against a real device, file
//! system or machine, so nothing here may be cited as `platform-qualified`.
//!
//! # Submit, stable, sync, ack
//!
//! - **Submit** appends a record to the node's log. It is visible at once and volatile.
//! - **Sync** opens a ticket over the log's current length: a barrier over every record
//!   the node submitted before it.
//! - **Stable**: `Persist` finishes the ticket's flush, and every record it covers is
//!   stable. A stable record is never lost, torn or changed.
//! - **Ack** delivers the ticket's completion to the incarnation that began it, only
//!   after `Persist`. An `Acked` event vouches that every record the ticket covers is
//!   stable. A completion that arrives after that incarnation crashed is `Fenced`.
//!
//! # What a crash keeps
//!
//! The process pack deferred this to this pack (`process/crash-restart-v0`,
//! composition row `storage`). A crash keeps every stable record, intact. Of the
//! volatile suffix it keeps a prefix of the adversary's choosing and loses the rest,
//! and the first lost record may survive torn. Every surviving intact record is stable
//! afterwards. [`COMPOSITION`] states how this composes with the process pack's crash
//! and fence.

use alloc::vec::Vec;
use core::fmt;

/// The profile's stable name: the name `replicated_register.scenario.toml`'s
/// `[storage] profile`, the replicated-register Intent Contract's `fault_model` and
/// `trust_boundaries`, and `schemas/examples/storage-pack.manifest.json` already use.
pub const PROFILE_NAME: &str = "storage/append-log-v0";

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
    /// No independence relation is supplied, so every pair of storage events is
    /// dependent ("`Unknown` means dependent", docs/17 §9). A reducer that uses this
    /// pack reduces nothing through it, so docs/09 T07 has no rule to subvert. This is
    /// more conservative than the `footprint` default of the schema example
    /// `storage-pack.manifest.json`, which this crate does not claim.
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

/// One storage semantic from RFC 0002 "Standard packs / Storage" or docs/17 §6 and §10.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Semantic {
    /// Volatile process memory.
    VolatileProcessMemory,
    /// Page cache and submitted writes (`Submit`).
    SubmittedWrites,
    /// Durable stable storage (`Persist`).
    StableStorage,
    /// The atomic unit of a write.
    AtomicityGranularity,
    /// Write order and the sync barrier.
    OrderingAndBarriers,
    /// A crash loses a suffix of the volatile records (`Crash`, `keep`).
    VolatileSuffixLoss,
    /// A crash leaves the last surviving record torn (`Crash`, `torn`).
    TornWrites,
    /// Crash and restart (`Crash`, `Restart`).
    CrashRestart,
    /// What a new incarnation reads, and dropping a torn tail (`Truncate`).
    RecoveryProcedure,
    /// The sync ticket's completion and its fence (`Ack`, `Delay`).
    SyncAcknowledgement,
    /// The exploration bounds.
    ExplorationBounds,
    /// Sector or block corruption.
    SectorCorruption,
    /// Writes reaching the medium out of submit order.
    WriteReordering,
    /// A device that reports a flush it did not perform.
    FlushDishonesty,
    /// Rename, link and directory durability.
    DirectoryDurability,
    /// A device and file-system profile.
    DeviceProfile,
    /// A tear below the record that reads as a different intact record.
    UndetectedTear,
}

impl Semantic {
    /// Every row of the profile, in declaration order.
    pub const ALL: [Self; 17] = [
        Self::VolatileProcessMemory,
        Self::SubmittedWrites,
        Self::StableStorage,
        Self::AtomicityGranularity,
        Self::OrderingAndBarriers,
        Self::VolatileSuffixLoss,
        Self::TornWrites,
        Self::CrashRestart,
        Self::RecoveryProcedure,
        Self::SyncAcknowledgement,
        Self::ExplorationBounds,
        Self::SectorCorruption,
        Self::WriteReordering,
        Self::FlushDishonesty,
        Self::DirectoryDurability,
        Self::DeviceProfile,
        Self::UndetectedTear,
    ];

    /// A stable kebab-case token. `volatile-suffix-loss` and `torn-last-record` are the
    /// fault names of `storage-pack.manifest.json`'s `storage/append-log-v0` profile.
    #[must_use]
    pub const fn token(self) -> &'static str {
        match self {
            Self::VolatileProcessMemory => "volatile-process-memory",
            Self::SubmittedWrites => "submitted-writes",
            Self::StableStorage => "stable-storage",
            Self::AtomicityGranularity => "atomicity-granularity",
            Self::OrderingAndBarriers => "ordering-and-barriers",
            Self::VolatileSuffixLoss => "volatile-suffix-loss",
            Self::TornWrites => "torn-last-record",
            Self::CrashRestart => "crash-restart",
            Self::RecoveryProcedure => "recovery-procedure",
            Self::SyncAcknowledgement => "sync-acknowledgement",
            Self::ExplorationBounds => "exploration-bounds",
            Self::SectorCorruption => "sector-corruption",
            Self::WriteReordering => "write-reordering",
            Self::FlushDishonesty => "flush-dishonesty",
            Self::DirectoryDurability => "directory-durability",
            Self::DeviceProfile => "device-profile",
            Self::UndetectedTear => "undetected-tear",
        }
    }

    /// Whether [`PROFILE_NAME`] at [`PROFILE_VERSION`] models this semantic.
    #[must_use]
    pub const fn support(self) -> Support {
        match self {
            Self::VolatileProcessMemory
            | Self::SubmittedWrites
            | Self::StableStorage
            | Self::AtomicityGranularity
            | Self::OrderingAndBarriers
            | Self::VolatileSuffixLoss
            | Self::TornWrites
            | Self::CrashRestart
            | Self::RecoveryProcedure
            | Self::SyncAcknowledgement
            | Self::ExplorationBounds => Support::Modelled,
            Self::SectorCorruption
            | Self::WriteReordering
            | Self::FlushDishonesty
            | Self::DirectoryDurability
            | Self::DeviceProfile
            | Self::UndetectedTear => Support::Unsupported,
        }
    }

    /// What the profile models for this row, or why it does not.
    #[must_use]
    pub const fn statement(self) -> &'static str {
        match self {
            Self::VolatileProcessMemory => {
                "the pack holds no process memory: what an incarnation keeps in memory \
                 ends with it at a crash, as process/crash-restart-v0 volatile-state-loss \
                 states; the only volatile state this pack holds is the submitted suffix \
                 of each log"
            }
            Self::SubmittedWrites => {
                "Submit appends one record to the node's log, after every record before \
                 it, while the node is up and its log has no torn tail; the record is \
                 visible at once to every later read of the log while it survives, and it \
                 is volatile: it is not stable until a finished flush covers it or a \
                 crash keeps it, and a crash may lose it; a submitted record cannot be \
                 withdrawn"
            }
            Self::StableStorage => {
                "a record is stable once Persist finished a sync that covers it, or once \
                 it survived a crash intact; sync completion is the durability boundary: \
                 a stable record is never lost, never torn and never changed by any later \
                 step, and the stable records of a log are always a prefix of it"
            }
            Self::AtomicityGranularity => {
                "the record is the atomic unit: a crash keeps a record whole, loses it \
                 whole, or leaves it torn, and a torn record reads as Torn, never as \
                 another intact value; a tear below the record that reads as a \
                 different intact record is the undetected-tear row, unsupported"
            }
            Self::OrderingAndBarriers => {
                "the records of one log reach the medium in submit order, so what a crash \
                 leaves is a prefix of what was submitted; Sync is a barrier over its own \
                 node's log: its ticket covers every record that node submitted before \
                 it, and none after; logs of distinct nodes share no barrier and no order"
            }
            Self::VolatileSuffixLoss => {
                "Crash keeps every stable record and the first keep records of the \
                 volatile suffix, keep of the adversary's choosing, and loses the rest; \
                 every surviving intact record is stable afterwards; when the \
                 configuration does not declare the fault, a crash must keep every \
                 volatile record"
            }
            Self::TornWrites => {
                "a crash that loses at least one volatile record may leave the first lost \
                 record torn instead, as the log's last entry; only a record that was \
                 never stable can be torn, at most one per log, and only when the \
                 configuration declares the fault; a torn tail survives later crashes \
                 until Truncate drops it"
            }
            Self::CrashRestart => {
                "Crash and Restart are the process pack's Crash and Restart at the same \
                 step boundary, under its rules: a crash is enabled only while the node \
                 is up and under the crash budget, a restart only for a down node when \
                 restart is declared, with an epoch one more than the last; a crash runs \
                 no flush and undoes no finished one; see the composition table"
            }
            Self::RecoveryProcedure => {
                "a new incarnation reads the log it inherits: the stable records, intact, \
                 and at most one torn tail; a Submit or Sync over a torn tail is refused \
                 as a ProgramFault, a verdict against the program (the replicated \
                 register's M07), and Truncate, which is atomic and durable, drops the \
                 tail; \
                 what recovery does with the records is the program's own code"
            }
            Self::SyncAcknowledgement => {
                "Sync opens a ticket bound to the incarnation that began it and to the \
                 log length it covers; Persist finishes its flush while that incarnation \
                 runs; Ack delivers the completion only to that incarnation and only \
                 after Persist, as Acked, which vouches that every record the ticket \
                 covers is stable; a completion that arrives after that incarnation \
                 crashed is Fenced, journalled and never delivered, whether or not its \
                 flush finished; Delay holds a ticket back one step with no duration, so \
                 an Ack may be delayed with no bound"
            }
            Self::ExplorationBounds => {
                "the configured bounds on pending tickets, retained bytes (the size of \
                 the journal's canonical encoding) and journal length are exploration \
                 bounds: a step past one is refused as ResourceExhausted, never dropped \
                 silently; the crash budget is part of the declared fault envelope, so a \
                 crash past it is not enabled"
            }
            Self::SectorCorruption => {
                "a stored record's bits changing on the medium, latent or at a crash, \
                 other than the one torn record, is not modelled; Corrupt asks for it and \
                 is refused"
            }
            Self::WriteReordering => {
                "the medium taking a later volatile record before an earlier one, so that \
                 a crash leaves a gap, is not modelled: the profile assumes the surviving \
                 log is a prefix of the submitted one; Reorder asks for it and is refused"
            }
            Self::FlushDishonesty => {
                "a device or firmware that reports a flush it did not perform is not \
                 modelled: the profile assumes a finished flush is on the medium; \
                 FalseFlush asks for it and is refused"
            }
            Self::DirectoryDurability => {
                "the pack has one log per node and no namespace: file creation, rename, \
                 link and directory durability are not modelled, and no step asks for \
                 them"
            }
            Self::UndetectedTear => {
                "a power-loss tear below the record, the declared atomic unit, that \
                 still passes the record's checksum and so reads as a different intact \
                 value is not modelled: the detected-tear assumption excludes it; no \
                 step asks for it"
            }
            Self::DeviceProfile => {
                "no device, file system, mount option or cache configuration is declared, \
                 measured or tested, and kernel or file-system bugs are not modelled; the \
                 host qualification is None, so the profile may not be cited as \
                 platform-qualified; no step asks for it"
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
pub const ASSUMPTIONS: [(&str, &str); 6] = [
    (
        "sync-promotes-a-prefix",
        "sync promotes a prefix to stable: a finished flush makes every record its ticket \
         covers stable, and the stable records of a log are a prefix of it",
    ),
    (
        "durable-log-is-a-prefix",
        "the log a crash leaves is a prefix of the submitted log, with at most its last \
         entry torn; the replicated-register contract states this as its storage \
         assumption DurableLogIsAPrefix",
    ),
    (
        "honest-flush",
        "a finished flush is on the medium; a device that lies about a flush is outside \
         the profile",
    ),
    (
        "detected-tear",
        "a torn record always fails its checksum and reads as Torn",
    ),
    (
        "durable-truncation",
        "Truncate is atomic and durable in one step: no crash brings a dropped torn \
         record back",
    ),
    (
        "no-eventual-flush",
        "no fairness is assumed: a pending flush may never finish and a completion may \
         never arrive, so no liveness claim may rest on this profile alone",
    ),
];

/// How this pack composes with the other packs, as `(pack, statement)`. The process
/// pack's `storage` composition row defers what stable storage keeps to this pack.
pub const COMPOSITION: [(&str, &str); 3] = [
    (
        "process/crash-restart-v0",
        "a composed run forwards each process-pack Crash and Restart of a node to this \
         pack at the same step boundary, in the same order, with the same crash budget \
         and restart switch, and chooses keep and torn only here, from the outcomes \
         enabled here, of which keeping every volatile record always is while the node \
         is up and the budget allows; a composed crash or restart takes effect only if \
         both packs accept it, and if either refuses it, for example this pack's \
         retained-bytes budget, neither applies it; this pack then keeps \
         the same incarnation epochs by the same rule, 0 first and one more at each \
         restart, and fences a sync ticket by the same rule, by node and epoch: its \
         completion reaches only the incarnation that began it while it runs, and is \
         Fenced otherwise; across the crash the node's log keeps every stable record \
         intact and a prefix of its volatile suffix, possibly ending in one torn record, \
         and loses the rest, while the incarnation's memory is lost as volatile-state-loss \
         states; a process ticket that stands for a sync is this pack's ticket, and its \
         Fenced completion vouches for nothing",
    ),
    (
        "network/adversarial-v0",
        "the two packs share no state: an envelope carries no durability, and a record \
         is stable or not whatever the network does; an acknowledgement a program sends \
         over the network makes a durability claim that holds only if an Acked sync \
         covers the record, and sending it earlier is the replicated register's M01, \
         ack-before-sync",
    ),
    (
        "runtime",
        "the pack is not bound to the runtime: mapping Submit, Sync, Persist and Ack onto \
         runtime operations and their obligations is the replicated-register \
         implementation's work, and the runtime's Requested, Reserved and Aborted phases \
         of an append happen before Submit",
    ),
];

/// docs/17 §8's cancellation contract for the pack's operation, an append from
/// `Submit` through `Sync` and `Persist` to `Ack`, as `(aspect, statement)`.
pub const CANCELLATION_CONTRACT: [(&str, &str); 8] = [
    (
        "reservation-point",
        "none in this pack: the runtime's Requested and Reserved phases, a write permit, \
         come before Submit",
    ),
    (
        "commit-point",
        "Submit for the record and Sync for the ticket: after Submit the record is in \
         the log, after Sync the ticket is pending, and before them nothing is",
    ),
    (
        "abort-behaviour",
        "a submitted record cannot be withdrawn and a pending ticket cannot be \
         cancelled; a cancellation after Submitted leaves the record volatile, and a \
         crash may then lose it; the runtime's Aborted phase comes before Submit or not \
         at all",
    ),
    (
        "finalizer-obligations",
        "none; a crash runs no finalizer and no flush",
    ),
    (
        "early-return",
        "not applicable: every step completes in one step, so a cancellation lands \
         before it or after it; a waiter cancelled before Ack leaves the ticket pending, \
         and its Ack is still delivered to the incarnation while it runs",
    ),
    (
        "late-completion",
        "an Ack of a ticket whose incarnation crashed is Fenced: journalled, never \
         delivered to any incarnation, and it vouches for nothing",
    ),
    (
        "idempotence-key",
        "the ticket ordinal; a ticket resolves, Acked or Fenced, at most once; a record \
         is keyed by its node and log position",
    ),
    (
        "replay-choice",
        "every step, each crash outcome's keep and torn included, is an explicit \
         choice-log entry; crash windows are exactly the boundaries between steps",
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
pub const APPEND_LOG_V0: FidelityProfile = FidelityProfile {
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
        out.extend_from_slice(b"continuum-storage-profile\0");
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
