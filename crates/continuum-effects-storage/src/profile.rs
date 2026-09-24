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
//! Each unsupported row also has one declared case in [`UNSUPPORTED`] (PR-15 /
//! IMPL-04, bn-1oj6): the host behaviour it leaves out, the [`Request`] by which a
//! caller could ask for it — a step, or no operation at all — and the [`Reliance`]
//! that says what a verdict gives a program that depends on it: a stated assumption, a
//! modelled row that already covers it, the composition row that owns it, or nothing,
//! because the pack has no way to express it.
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

use crate::refusal::Refusal;
use crate::step::StepKind;

/// The current profile's name, `-v1`. The replicated-register scenario, Intent Contract
/// and `schemas/examples/storage-pack.manifest.json` name the frozen `-v0`,
/// [`PROFILE_NAME_V0`]; a contract adopts `-v1` by an intent revision.
pub const PROFILE_NAME: &str = "storage/append-log-v1";

/// The frozen predecessor's name, `storage/append-log-v0`: the profile the replicated-register
/// scenario and Intent Contract named before bn-1oj6. A profile's name identifies its
/// canonical bytes (RFC 0002 correction 1), so the IMPL-04 content could not be
/// published under it; `-v0` stays declared, byte for byte as it was, in [`APPEND_LOG_V0`].
pub const PROFILE_NAME_V0: &str = "storage/append-log-v0";

/// The profile's semantic version (docs/17 §12: patch is implementation only, minor
/// is additive, major is changed semantics or fidelity).
///
/// 1.0.0 (bn-1oj6, PR-15 / IMPL-04, cr-37bshu) is `storage/append-log-v1`: the `-v0` rows with
/// the declared unsupported cases, [`UNSUPPORTED`], and the `no-medium-corruption`, `no-device-profile` and `durable-log-entry`
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

    /// Whether the profiles model this semantic. `-v0` and `-v1` share every row, so
    /// the answer is the same under both.
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
pub const ASSUMPTIONS: [(&str, &str); 9] = [
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
    (
        "no-medium-corruption",
        "a record on the medium keeps its bits: latent sector or block corruption, and \
         corruption at a crash other than the one torn last record, are outside the \
         profile",
    ),
    (
        "no-device-profile",
        "no device or file system is described: the program never sees a failed write or \
         flush, for example on a full device or an I/O error, and kernel and file-system \
         bugs, mount options and cache settings are outside the profile",
    ),
    (
        "durable-log-entry",
        "each node's log exists durably under a durable name before its first Submit: \
         a crash never loses the log itself, and creating, renaming, linking or \
         removing files and directory entries is outside the profile",
    ),
];

/// The assumptions `storage/append-log-v0` states: the first 6 of [`ASSUMPTIONS`], taken by
/// construction so the frozen profile cannot drift from them. The entries after them
/// are `-v1`'s additions.
pub const ASSUMPTIONS_V0: &[(&str, &str)] = ASSUMPTIONS.as_slice().split_at(6).0;

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

/// How a caller could ask this pack for a host behaviour it does not model.
///
/// One exception holds for every request: a pack whose journal already holds
/// the configuration's step bound, at most [`crate::step::MAX_STEPS`], in events refuses every step as `BoundReached(Steps)`, an exploration bound, before it
/// looks at the step. That refusal is inconclusive too (INV-008 `ResourceExhausted`),
/// never a success.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Request {
    /// A step of this kind asks for it in every state below the step bound, and is
    /// refused there.
    Step(StepKind),
    /// No step, configuration value or other public operation of the pack can ask for
    /// it.
    NoOperation,
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
    /// Every behaviour the host behaviour produces is one the named modelled row already
    /// lets the program, the scheduler or the adversary choose under the same
    /// configuration, so a verdict already covers it.
    Subsumed(Semantic),
    /// The [`COMPOSITION`] row with this id states it.
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
            Request::Step(_) => Some(Refusal::Unsupported(self.semantic)),
            Request::NoOperation => None,
        }
    }
}

/// The declared unsupported cases: one per [`Support::Unsupported`] row, in
/// [`Semantic::ALL`] order. It is part of the profile's canonical bytes, so a change
/// to it changes the fingerprint and needs a version bump.
pub const UNSUPPORTED: [UnsupportedCase; 6] = [
    UnsupportedCase {
        semantic: Semantic::SectorCorruption,
        host_behaviour: "sector or block corruption: a stored record's bits change on \
                         the medium, latent or at a crash",
        request: Request::Step(StepKind::Corrupt),
        reliance: Reliance::Assumed("no-medium-corruption"),
    },
    UnsupportedCase {
        semantic: Semantic::WriteReordering,
        host_behaviour: "writes of one log reaching the medium out of submit order, so \
                         that a crash leaves a gap; across the logs of distinct nodes no \
                         order is assumed, which the ordering-and-barriers row models",
        request: Request::Step(StepKind::Reorder),
        reliance: Reliance::Assumed("durable-log-is-a-prefix"),
    },
    UnsupportedCase {
        semantic: Semantic::FlushDishonesty,
        host_behaviour: "fsync that lies: device firmware or a cache reports a flush \
                         it did not perform",
        request: Request::Step(StepKind::FalseFlush),
        reliance: Reliance::Assumed("honest-flush"),
    },
    UnsupportedCase {
        semantic: Semantic::DirectoryDurability,
        host_behaviour: "file creation, rename, link and directory durability: a new \
                         or renamed file, the log's own file included, whose directory \
                         entry a crash loses",
        request: Request::NoOperation,
        reliance: Reliance::Assumed("durable-log-entry"),
    },
    UnsupportedCase {
        semantic: Semantic::DeviceProfile,
        host_behaviour: "a device and file-system profile: a write or flush that fails, \
                         for example with no space left, and kernel or file-system bugs",
        request: Request::NoOperation,
        reliance: Reliance::Assumed("no-device-profile"),
    },
    UnsupportedCase {
        semantic: Semantic::UndetectedTear,
        host_behaviour: "a power-loss tear below the declared atomic unit that passes \
                         the record's checksum and reads as a different intact record",
        request: Request::NoOperation,
        reliance: Reliance::Assumed("detected-tear"),
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

/// The frozen `storage/append-log-v0` at 0.1.0, byte for byte the profile this crate published
/// before bn-1oj6: its rows, its 6 assumptions, and no declared unsupported cases.
pub const APPEND_LOG_V0: FidelityProfile = FidelityProfile {
    name: PROFILE_NAME_V0,
    version: PROFILE_VERSION_V0,
    class: FidelityClass::AdversarialEnvelope,
    host: HostQualification::None,
    independence: IndependenceClaim::AllDependent,
    assumptions: ASSUMPTIONS_V0,
    unsupported: None,
};

/// The current `storage/append-log-v1` at 1.0.0: the same rows, every assumption, and the declared
/// unsupported cases. The Lab handler journals under this profile.
pub const APPEND_LOG_V1: FidelityProfile = FidelityProfile {
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
pub const PROFILES: [FidelityProfile; 2] = [APPEND_LOG_V0, APPEND_LOG_V1];

impl FidelityProfile {
    /// The profile's canonical bytes: name, version, class, host claim, independence
    /// claim, every [`Semantic`] row with its support and statement, the assumptions,
    /// the composition table, the cancellation contract and the declared unsupported
    /// cases. Any change to what the profile says changes these bytes, so they can be
    /// pinned (docs/17 §12: "every crashpack pins exact pack digest").
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
            self.assumptions,
            &COMPOSITION[..],
            &CANCELLATION_CONTRACT[..],
        ] {
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
                Reliance::Owner(row) => {
                    put_str(out, "owner");
                    put_str(out, row);
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
/// static table, so saturation is unreachable; it saturates rather than wrapping so an
/// impossible overflow could never alias a shorter length.
pub(crate) fn u32_len(len: usize) -> u32 {
    u32::try_from(len).unwrap_or(u32::MAX)
}
