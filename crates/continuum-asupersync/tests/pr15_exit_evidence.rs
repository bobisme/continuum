//! Dedicated exit evidence for `PR-15-EXIT` (`notes/plan/notes/PLAN_REQUIREMENTS.json`,
//! id `PR-15-EXIT`; `notes/plan/notes/START_HERE_IMPLEMENTATION.md`, PR 15's Exit line).
//!
//! > **Exit:** crash windows and cancellation points replay exactly.
//! >
//! > — `notes/plan/notes/START_HERE_IMPLEMENTATION.md`, PR 15
//!
//! This file is an independent exit layer over the four PR-15 packs and the register
//! campaigns that realize a crash, the way `pr14_exit_evidence.rs` sits over the PR-14
//! families: one named witness that walks the whole sentence in one place, touching none
//! of the per-pack suites (`pr15_impl01_network.rs`, `pr15_impl02_process.rs`,
//! `pr15_impl03_storage.rs`, `pr15_impl04_*.rs`), none of the register suites, and none
//! of `src/`. It lives in `continuum-asupersync`'s tests because the packs may have no
//! dependency and no dev-dependency (their `no_std` lane and INV-015 judge their
//! manifests), and because this crate's tests already hold the register program, the
//! binding and its crash.
//!
//! # Which windows, and who declares them
//!
//! Which window classes a pack has is read from its own `CANCELLATION_CONTRACT`, the
//! only place the packs state it (text in each profile's canonical bytes, not a typed
//! field); `hon_01` pins that reading, so a change to the text shows. The windows
//! themselves are then enumerated from the handler, never listed by hand
//! ([`hon_01_each_pack_declares_its_windows_in_its_own_contract`]):
//!
//! - `network/adversarial-v{0,1}`: `CANCELLATION_CONTRACT` `replay-choice` says
//!   "cancellation points are exactly the boundaries between steps", and the pack has no
//!   crash (endpoint crash is a declared unsupported case owned by the process profile);
//! - `process/crash-restart-v{0,1}`: "crash windows are exactly the boundaries between
//!   steps", and "graceful cancellation is not modelled here, so its cancellation points
//!   are the runtime's (PR 14)";
//! - `storage/append-log-v{0,1}`: "crash windows are exactly the boundaries between
//!   steps";
//! - the network and storage `early-return` rows say a cancellation "lands before it or
//!   after it", so their cancellation points are step boundaries. The process pack
//!   routes its cancellation points to the runtime, so none is counted at that pack:
//!   they are the binding's region `Cancel` (`pos-05`) and the PR-14 exit evidence's;
//! - `time/unmodelled-v0` declares `Operations::None`: no step, so no window. It is
//!   reported as declared-none, never counted as a replayed window.
//!
//! A crash window is a step boundary of a seeded choice log together with **every**
//! crash outcome the Lab handler enables there (`enabled_choices`, filtered to `Crash`:
//! the process pack's `Crash(n)` and the storage pack's `Crash { keep, torn }` for every
//! `keep` and tear), alone and followed by the recovery the handler then enables
//! (`Restart`, and `Truncate` when a torn record remains). A crash the handler does not
//! enable at a boundary is recorded with its typed refusal, which must not be
//! inconclusive. A cancellation point is a step boundary after which the program issues
//! no more steps while the scheduler and adversary go on (the contracts'
//! `late-completion` rows). After the splice, the rest of the log is continued: a step
//! the new state refuses as `NotEnabled` or `Malformed` is not taken (that is an invalid
//! choice, not an outcome), a storage `ProgramFault` ends the window there (it is a
//! conclusive verdict against the program, and the fault itself must replay at the same
//! index with the same refusal), and an inconclusive refusal ends the window as a typed
//! [`Gap`], so no window is dropped silently. Each pack runs under two configurations:
//! the scenario-shaped one, and one with every fault switch off, no restart and a crash
//! budget of one, so windows also meet a spent budget.
//!
//! At the runtime, a crash window is where the register program's crash lands: each
//! crash fate of `register_baseline::one_epoch_options` (a crash with the permit held,
//! after submit, after sync, after the reply), realized as the binding's graceful
//! `Cancel` of the incarnation's region (the PR-14 cancellation points) and as its
//! fail-stop `Crash` (bn-20d8u, bn-fxxf2, bn-1id0n), over seeded interleavings.
//!
//! # What "replay exactly" is checked as
//!
//! Every realized window is recorded — its canonical journal bytes, its choice log and
//! its outcome (the terminal state for a pack, the four-property run report for the
//! register) — and then replayed three ways, each compared byte for byte and outcome for
//! outcome with the record ([`exact_replay`]):
//!
//! 1. from the **recorded bytes**: an independent decoder written here from the pinned
//!    wire forms turns the journal back into its choice log and configuration, and the
//!    Lab handler runs it again, twice (repeated runs);
//! 2. from the **recorded plan**: the realized choice log under the recorded
//!    configuration;
//! 3. by the pack's own `Journal::replay`.
//!
//! The binding's windows replay from the recorded plan and log under seven lab seeds and
//! round-trip through `Journal::decode`. Every correct-baseline campaign the PR-16
//! golden retains (read from the golden, not listed here: the graceful and fail-stop
//! baselines and the message, timer and recovery carriers under each) is executed again
//! from its recorded plans and must reproduce the retained campaign identity, a digest
//! over every plan, log and journal, with no finding, and every count the retained line
//! carries (fences, payloads, timers).
//!
//! # Negative: a perturbed replay is a typed divergence
//!
//! Each crash window is replayed with its crash moved one step later, one step earlier,
//! and dropped. Each cancellation point is replayed with its cancellation dropped, moved
//! past the next program step, and moved one step later. Every perturbation of every
//! window gets exactly one disposition, and the dispositions add up to the windows:
//! a typed [`Divergence`]; verified equivalent, where the move provably changes nothing
//! (a crash swapped with an identical step, a cancellation dropped with no program step
//! after it, a cancellation moved across a scheduler or adversary step) and the replay is
//! checked to be the record; or absent, where there is no step on that side (the last
//! boundary, no program step remaining, the first or last operation of the register's
//! script). A perturbation classed equivalent that replays differently is a gap. At the
//! binding, the crash is also dropped with the recorded log kept aligned
//! (the crash operation replaced by a one-nanosecond clock advance, which commands no
//! task) and replayed under the other crash semantics (a `Cancel` as a `Crash`, and the
//! reverse). No perturbation is ever an accepted replay. A length-only checker would miss many of them: those are counted, so the
//! detection rests on content. A doctored recorded journal is a typed divergence too, and
//! the byte and outcome comparisons are each shown load-bearing: the same log under a
//! configuration that differs only in its retained-bytes budget differs in bytes alone,
//! and a doctored outcome differs in outcome alone (`neg-03`).
//!
//! What this proves, and no more: a pack event maps one to one to its step, so a moved
//! or dropped crash that is a valid run would also show in the choice logs alone; the
//! evidence is that the recorded journal fixes the run, not that the handler hides
//! state. A dropped cancellation compares two different logs, so it shows the checker
//! tells them apart. At the binding, a crash moved one step earlier is always refused
//! by the substrate (a command to an ended or crashed task), never a valid run.
//!
//! In a composed run the recorded journals hold each pack's own order, so a composed
//! crash moved "one step" moves past the nearest process or storage step. Moving it past
//! network steps commutes with them (no pack reads another's state); each such position
//! is replayed and verified identical, and a window at a boundary that differs from an
//! earlier one only by network steps is the same window: its bytes are verified equal
//! and it is not counted twice.
//!
//! # Fail closed
//!
//! Every check this file claims is one predicate with its own result (`compute_predicates`),
//! each recorded in `pr15-exit.json`. The machine verdict is `met` only when every
//! required predicate is present exactly once and passes (`aggregate`, `bnd-05`).
//! Per artifact, [`ExitVerdict`] is `Met` only when every declared window was realized and replayed
//! and at least one window exists; an unsupported, inconclusive, unrealized or diverging
//! window makes it `NotMet` with the typed [`Gap`]
//! ([`bnd_03_the_exit_fails_closed_on_an_unreplayable_window`]). Each requestable
//! declared unsupported case (bn-1oj6) is spliced into every window where it applies and
//! must be the pack's typed `Unsupported` refusal, never a recorded window
//! ([`bnd_01_every_requestable_unsupported_case_is_a_typed_refusal_never_replayed`]).
//!
//! # Evidence map (stable artifact IDs)
//!
//! | ID | Claim | Test |
//! |---|---|---|
//! | `pr15-exit-hon-01` | the window classes each profile declares (`-v0` and `-v1` share them), and the time pack's none | [`hon_01_each_pack_declares_its_windows_in_its_own_contract`] |
//! | `pr15-exit-pos-01` | network cancellation points replay exactly | [`pos_01_network_cancellation_points_replay_exactly`] |
//! | `pr15-exit-pos-02` | process crash windows replay exactly; its cancellation points are routed to the runtime | [`pos_02_process_crash_windows_replay_exactly`] |
//! | `pr15-exit-pos-03` | storage crash windows and cancellation points replay exactly | [`pos_03_storage_crash_windows_and_cancellation_points_replay_exactly`] |
//! | `pr15-exit-pos-04` | composed crash windows (process and storage at one boundary) replay exactly and leave the network journal unchanged | [`pos_04_composed_crash_windows_replay_exactly_and_leave_the_network_untouched`] |
//! | `pr15-exit-pos-05` | the register's graceful crash windows (binding `Cancel`) replay exactly across seeds | [`pos_05_register_graceful_crash_windows_replay_exactly`] |
//! | `pr15-exit-pos-06` | the register's fail-stop crash windows (binding `Crash`) replay exactly across seeds | [`pos_06_register_fail_stop_crash_windows_replay_exactly`] |
//! | `pr15-exit-pos-07` | the retained register campaigns replay to their recorded identities | [`pos_07_the_retained_register_campaigns_replay_to_their_recorded_identities`] |
//! | `pr15-exit-neg-01` | every crash window's moved or dropped crash is a typed divergence, verified equivalent, or absent, with exact accounting | [`neg_01_a_moved_or_dropped_crash_is_a_typed_divergence`] |
//! | `pr15-exit-neg-02` | every network and storage cancellation point's dropped or moved cancellation is a typed divergence, verified equivalent, or absent, with exact accounting | [`neg_02_a_dropped_or_moved_cancellation_is_a_typed_divergence`] |
//! | `pr15-exit-neg-03` | a doctored recorded journal is a typed divergence | [`neg_03_a_doctored_recorded_journal_is_a_typed_divergence`] |
//! | `pr15-exit-bnd-01` | every requestable declared unsupported case is a typed refusal, never replayed | [`bnd_01_every_requestable_unsupported_case_is_a_typed_refusal_never_replayed`] |
//! | `pr15-exit-bnd-02` | a crash a boundary does not enable is a typed, conclusive refusal | [`bnd_02_a_crash_a_boundary_does_not_enable_is_typed_and_conclusive`] |
//! | `pr15-exit-bnd-03` | the exit fails closed on an unreplayable window | [`bnd_03_the_exit_fails_closed_on_an_unreplayable_window`] |
//! | `pr15-exit-bnd-04` | the time pack declares no operation, so no window | [`bnd_04_the_time_pack_declares_no_operation_and_so_no_window`] |
//! | `pr15-exit-bnd-05` | the machine verdict is the conjunction of every claimed check | [`bnd_05_the_machine_verdict_is_the_conjunction_of_every_claimed_check`] |
//! | golden | the retained artifacts `tests/golden/pr15_exit_evidence.txt` and `tests/evidence/pr15-exit.json` are byte-stable | [`the_exit_evidence_artifacts_are_byte_stable`] |
//!
//! Regenerate both artifacts with `PR15_EXIT_BLESS=1 cargo test -p continuum-asupersync
//! --test pr15_exit_evidence`; the run always fails after it writes, so new bytes are
//! reviewed, never self-certified.
//!
//! # Scope, stated so no one takes more from it
//!
//! - The packs have no host semantics (`HostQualification::None`): this is exact replay of
//!   the Lab handlers, not of a host.
//! - The register program does not drive the packs: no substrate binding maps pack steps
//!   onto asupersync (the packs' own narrowing, PR 16's work). The runtime crash windows
//!   are the binding's `Cancel` and `Crash` of an incarnation's region, whose semantics
//!   are the process profile's rows `graceful-cancellation` and `fail-stop-crash`.
//! - Pack windows are every boundary of seeded logs (7 seeds, two configurations per
//!   pack); register windows are every crash fate over a seeded sample of interleavings;
//!   the campaigns are the retained ones, re-executed in full.
//! - `-v0` is covered through the rows and contract it shares with `-v1`: the one Lab
//!   handler journals under the `-v1` header, so no `-v0` journal exists to replay.
//! - Composed runs have crash windows only; cancellation points are per pack.
//! - A crash a boundary does not enable is seen as `NodeDown` and `CrashBudgetSpent`;
//!   other not-enabled reasons are not reached by this corpus.

#![allow(clippy::too_many_lines)]

#[path = "support/primitive_conformance_model.rs"]
#[allow(dead_code)]
mod model;

#[path = "support/replicated_register.rs"]
#[allow(dead_code)]
mod register;

#[path = "support/register_baseline.rs"]
#[allow(dead_code)]
mod baseline;

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::{Debug, Write as _};
use std::sync::OnceLock;

use continuum_asupersync::binding::{
    BindingConfig, BindingRefusal, SubstrateOp, run as run_binding,
};
use continuum_asupersync::choice::ChoiceLog;
use continuum_asupersync::family::Family;
use continuum_asupersync::family::lifecycle::{RegionLabel, TaskLabel};
use continuum_asupersync::journal::Journal as BJournal;
use continuum_task::region::worker::Resumability;
use continuum_value::assurance::InconclusiveReason;
use continuum_value::identity::{Blake3Hasher, ContentHasher};

use continuum_effects_network as net;
use continuum_effects_process as prc;
use continuum_effects_storage as sto;
use continuum_effects_time as tim;

use baseline::{Fate, Scope};
use register::{Built, CrashMode, Plan};

// ---------------------------------------------------------------------------
// the corpus parameters
// ---------------------------------------------------------------------------

/// Log-generator seeds: the pack corpora are a function of these alone (INV-005).
const SEEDS: [u64; 7] = [1, 2, 7, 42, 99, 0x9e37_79b9_7f4a_7c15, u64::MAX];
/// Logs per seed per pack.
const LOGS_PER_SEED: usize = 3;
/// Steps per generated log.
const LOG_LEN: usize = 18;
/// Replays from the recorded bytes, per window: the repeated runs.
const REPEATS: usize = 2;
/// Lab seeds of the binding: the journal must not depend on them.
const LAB_SEEDS: [u64; 7] = [0, 1, 7, 42, 99, 0x9e37_79b9_7f4a_7c15, u64::MAX];
/// Interleavings per register crash window plan and crash mode.
const REGISTER_LOGS: usize = 6;
/// The explicit seed of the register interleavings.
const REGISTER_SEED: u64 = 0x1515;
/// A retained-bytes budget no corpus log approaches: no window is inconclusive for
/// want of budget (a budget that binds is `bnd-03`'s case, not the corpus's).
const RETAINED: u64 = 1 << 24;

/// The PR-16 goldens that retain the register campaigns' identities.
const IMPL05_GOLDEN: &str = include_str!("golden/pr16_impl05_mutants.evidence.txt");

// ---------------------------------------------------------------------------
// typed replay results
// ---------------------------------------------------------------------------

/// Why a replay is not the record. Every variant is a divergence; none is a success.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
enum Divergence {
    /// The recorded bytes are not a journal of this pack.
    Undecodable(String),
    /// The decoded configuration is not the recorded one.
    HeaderDiffers,
    /// The journal's own choice log is not the recorded log.
    LogDiffers,
    /// The replayed plan is not a valid run: the handler refused step `index`.
    Refused {
        /// The refused step (0 for a binding refusal, which names no step).
        index: usize,
        /// The refusal's class.
        class: String,
    },
    /// The first event at which the replay and the record differ.
    EventDiffers {
        /// Its index.
        index: usize,
    },
    /// One journal is a strict prefix of the other.
    LengthDiffers {
        /// Recorded events.
        recorded: usize,
        /// Replayed events.
        replayed: usize,
    },
    /// The same events, different bytes: the canonical encoding broke.
    BytesDiffer,
    /// The same journal, a different outcome.
    OutcomeDiffers,
}

impl Divergence {
    fn kind(&self) -> &'static str {
        match self {
            Self::Undecodable(_) => "undecodable",
            Self::HeaderDiffers => "header-differs",
            Self::LogDiffers => "log-differs",
            Self::Refused { .. } => "refused",
            Self::EventDiffers { .. } => "event-differs",
            Self::LengthDiffers { .. } => "length-differs",
            Self::BytesDiffer => "bytes-differ",
            Self::OutcomeDiffers => "outcome-differs",
        }
    }
}

/// Why a declared window does not count as replayed. The exit fails closed on any.
#[derive(Debug, Clone, PartialEq, Eq)]
enum GapWhy {
    /// The window's replay diverged from its record.
    Diverged(Divergence),
    /// The window asks for something the profile does not model (INV-008 `Unsupported`).
    Unsupported(String),
    /// An exploration bound or another inconclusive outcome ended the window.
    Inconclusive(String),
    /// The window could not be realized as a run at all.
    NotRealized(String),
    /// A crash rewrote the history before it.
    PrefixRewritten,
    /// The artifact enumerated no window, so nothing was replayed.
    NoWindows,
}

/// One gap, located.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Gap {
    artifact: &'static str,
    window: String,
    why: GapWhy,
}

/// The exit's verdict over one artifact's windows.
#[derive(Debug, Clone, PartialEq, Eq)]
enum ExitVerdict {
    /// Every declared window was realized and replayed exactly, and there was one.
    Met {
        /// Windows replayed.
        windows: u64,
    },
    /// Some window was not: the typed gaps.
    NotMet(Vec<Gap>),
}

fn exit_verdict(artifact: &'static str, windows: u64, gaps: &[Gap]) -> ExitVerdict {
    let mut gaps: Vec<Gap> = gaps.to_vec();
    if windows == 0 {
        gaps.push(Gap {
            artifact,
            window: "-".to_owned(),
            why: GapWhy::NoWindows,
        });
    }
    if gaps.is_empty() {
        ExitVerdict::Met { windows }
    } else {
        ExitVerdict::NotMet(gaps)
    }
}

/// A pack refusal, reduced to what the exit reads: class, INV-008 reason, detail.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Refused {
    class: String,
    reason: Option<&'static str>,
    detail: String,
}

impl Refused {
    /// The gap a refusal of a window's own splice is.
    fn gap(&self, artifact: &'static str, window: &str) -> Gap {
        let why = match self.reason {
            Some("Unsupported") => GapWhy::Unsupported(self.detail.clone()),
            Some(other) => GapWhy::Inconclusive(format!("{other}: {}", self.detail)),
            None => GapWhy::NotRealized(self.detail.clone()),
        };
        Gap {
            artifact,
            window: window.to_owned(),
            why,
        }
    }

    /// The not-enabled reason's name, `NodeDown` for `NotEnabled(NodeDown(NodeId(0)))`.
    fn reason_name(&self) -> String {
        let inner = self
            .detail
            .split_once('(')
            .map_or(self.detail.as_str(), |(_, rest)| rest);
        inner
            .split(|c: char| !c.is_alphanumeric())
            .next()
            .unwrap_or("")
            .to_owned()
    }
}

// ---------------------------------------------------------------------------
// seeded choice
// ---------------------------------------------------------------------------

/// SplitMix64: the explicit, seeded source of every generated log.
struct SplitMix(u64);

impl SplitMix {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }

    fn below(&mut self, n: usize) -> usize {
        usize::try_from(self.next() % u64::try_from(n).expect("small")).expect("small")
    }

    fn node(&mut self, nodes: u8) -> u8 {
        u8::try_from(self.below(usize::from(nodes))).expect("a node")
    }
}

// ---------------------------------------------------------------------------
// an independent decoder of the pinned journal wire forms
// ---------------------------------------------------------------------------

/// A strict reader over recorded bytes: every read is bounds-checked, and a decoder
/// must consume the input exactly.
struct Reader<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl<'a> Reader<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, at: 0 }
    }

    fn take(&mut self, n: usize) -> Result<&'a [u8], String> {
        let end = self
            .at
            .checked_add(n)
            .filter(|end| *end <= self.bytes.len())
            .ok_or_else(|| format!("truncated at byte {}", self.at))?;
        let out = &self.bytes[self.at..end];
        self.at = end;
        Ok(out)
    }

    fn u8(&mut self) -> Result<u8, String> {
        Ok(self.take(1)?[0])
    }

    fn flag(&mut self) -> Result<bool, String> {
        match self.u8()? {
            0 => Ok(false),
            1 => Ok(true),
            other => Err(format!("flag byte {other}")),
        }
    }

    fn u32(&mut self) -> Result<u32, String> {
        Ok(u32::from_be_bytes(
            self.take(4)?.try_into().expect("4 bytes"),
        ))
    }

    fn u64(&mut self) -> Result<u64, String> {
        Ok(u64::from_be_bytes(
            self.take(8)?.try_into().expect("8 bytes"),
        ))
    }

    fn expect(&mut self, want: &[u8], what: &str) -> Result<(), String> {
        if self.take(want.len())? == want {
            Ok(())
        } else {
            Err(format!("{what} differs"))
        }
    }

    fn done(&self) -> Result<(), String> {
        if self.at == self.bytes.len() {
            Ok(())
        } else {
            Err(format!("{} trailing bytes", self.bytes.len() - self.at))
        }
    }
}

/// The header every pack journal opens with: magic, the length-prefixed profile name,
/// and the version.
fn header(magic: &[u8], name: &str, version: [u16; 3]) -> Vec<u8> {
    let mut out = magic.to_vec();
    out.extend_from_slice(&u32::try_from(name.len()).expect("short").to_be_bytes());
    out.extend_from_slice(name.as_bytes());
    for part in version {
        out.extend_from_slice(&part.to_be_bytes());
    }
    out
}

fn decode_network(bytes: &[u8]) -> Result<(net::NetworkConfig, Vec<net::Step>), String> {
    let v = net::ADVERSARIAL_V1.version;
    let mut r = Reader::new(bytes);
    r.expect(
        &header(
            b"continuum-network-journal\0",
            net::ADVERSARIAL_V1.name,
            [v.major, v.minor, v.patch],
        ),
        "header",
    )?;
    let nodes = r.u8()?;
    let in_flight = r.u32()?;
    let payload = r.u32()?;
    let bits = r.u8()?;
    if bits & !0b111 != 0 {
        return Err(format!("fault bits {bits:#b}"));
    }
    let faults = net::FaultSwitches {
        loss: bits & 1 != 0,
        duplication: bits & 2 != 0,
        reordering: bits & 4 != 0,
    };
    let partitions = r.u32()?;
    let retained = r.u64()?;
    let config = net::NetworkConfig::new(nodes, in_flight, payload, faults, partitions, retained)
        .map_err(|e| e.to_string())?;
    let count = r.u32()?;
    let mut steps = Vec::new();
    for _ in 0..count {
        let step = match r.u8()? {
            1 => {
                let _envelope = r.u32()?;
                let src = net::NodeId(r.u8()?);
                let dst = net::NodeId(r.u8()?);
                let len = usize::try_from(r.u32()?).map_err(|e| e.to_string())?;
                let payload = net::Payload(r.take(len)?.to_vec());
                net::Step::Send { src, dst, payload }
            }
            2 => {
                let envelope = net::EnvelopeId(r.u32()?);
                let _ = (r.u8()?, r.u8()?);
                net::Step::Deliver(envelope)
            }
            3 => net::Step::Drop(net::EnvelopeId(r.u32()?)),
            4 => net::Step::Duplicate(net::EnvelopeId(r.u32()?)),
            5 => net::Step::Delay(net::EnvelopeId(r.u32()?)),
            6 => net::Step::Partition(net::NodeSet(r.u64()?)),
            7 => net::Step::Heal,
            tag => return Err(format!("event tag {tag}")),
        };
        steps.push(step);
    }
    r.done()?;
    Ok((config, steps))
}

fn decode_process(bytes: &[u8]) -> Result<(prc::ProcessConfig, Vec<prc::Step>), String> {
    let v = prc::CRASH_RESTART_V1.version;
    let mut r = Reader::new(bytes);
    r.expect(
        &header(
            b"continuum-process-journal\0",
            prc::CRASH_RESTART_V1.name,
            [v.major, v.minor, v.patch],
        ),
        "header",
    )?;
    let nodes = r.u8()?;
    let crashes = r.u32()?;
    let restart = r.flag()?;
    let pending = r.u32()?;
    let retained = r.u64()?;
    let config = prc::ProcessConfig::new(nodes, crashes, restart, pending, retained)
        .map_err(|e| e.to_string())?;
    let count = r.u32()?;
    let mut steps = Vec::new();
    for _ in 0..count {
        let tag = r.u8()?;
        let ticket = match tag {
            1 | 2 | 3 | 6 => Some(prc::TicketId(r.u32()?)),
            4 | 5 => None,
            other => return Err(format!("event tag {other}")),
        };
        let node = prc::NodeId(r.u8()?);
        let _epoch = r.u32()?;
        steps.push(match (tag, ticket) {
            (1, _) => prc::Step::Begin(node),
            (2 | 3, Some(t)) => prc::Step::Complete(t),
            (6, Some(t)) => prc::Step::Delay(t),
            (4, _) => prc::Step::Crash(node),
            _ => prc::Step::Restart(node),
        });
    }
    r.done()?;
    Ok((config, steps))
}

fn decode_storage(bytes: &[u8]) -> Result<(sto::StorageConfig, Vec<sto::Step>), String> {
    let v = sto::APPEND_LOG_V1.version;
    let mut r = Reader::new(bytes);
    r.expect(
        &header(
            b"continuum-storage-journal\0",
            sto::APPEND_LOG_V1.name,
            [v.major, v.minor, v.patch],
        ),
        "header",
    )?;
    let nodes = r.u8()?;
    let crashes = r.u32()?;
    let restart = r.flag()?;
    let suffix_loss = r.flag()?;
    let torn = r.flag()?;
    let pending = r.u32()?;
    let max_steps = r.u32()?;
    let retained = r.u64()?;
    let config = sto::StorageConfig::new(
        nodes,
        crashes,
        restart,
        suffix_loss,
        torn,
        pending,
        retained,
    )
    .and_then(|c| c.with_max_steps(max_steps))
    .map_err(|e| e.to_string())?;
    let count = r.u32()?;
    let mut steps = Vec::new();
    for _ in 0..count {
        let step = match r.u8()? {
            1 => {
                let node = sto::NodeId(r.u8()?);
                let _epoch = r.u32()?;
                let _index = r.u32()?;
                let value = sto::Value(r.u32()?);
                sto::Step::Submit { node, value }
            }
            tag @ 2..=6 => {
                let ticket = sto::TicketId(r.u32()?);
                let node = sto::NodeId(r.u8()?);
                let _epoch = r.u32()?;
                let _upto = r.u32()?;
                match tag {
                    2 => sto::Step::Sync(node),
                    3 => sto::Step::Persist(ticket),
                    4 | 5 => sto::Step::Ack(ticket),
                    _ => sto::Step::Delay(ticket),
                }
            }
            7 => {
                let node = sto::NodeId(r.u8()?);
                let _epoch = r.u32()?;
                let keep = r.u32()?;
                let torn = r.flag()?;
                let _lost = r.u32()?;
                sto::Step::Crash { node, keep, torn }
            }
            8 => {
                let node = sto::NodeId(r.u8()?);
                let _epoch = r.u32()?;
                sto::Step::Restart(node)
            }
            9 => {
                let node = sto::NodeId(r.u8()?);
                let _epoch = r.u32()?;
                let _index = r.u32()?;
                sto::Step::Truncate(node)
            }
            tag => return Err(format!("event tag {tag}")),
        };
        steps.push(step);
    }
    r.done()?;
    Ok((config, steps))
}

// ---------------------------------------------------------------------------
// the three packs, behind one interface
// ---------------------------------------------------------------------------

/// What the exit needs of a pack: its Lab handler, its enabled choices, its crash
/// outcomes, its wire form, and its terminal state.
trait Pack {
    /// The pack's name in the evidence.
    const ID: &'static str;
    type Config: Copy + PartialEq + Debug;
    type Step: Clone + PartialEq + Debug;
    type Event: Clone + PartialEq + Debug;
    type Lab: Clone;

    fn lab(config: Self::Config) -> Self::Lab;
    fn apply(lab: &mut Self::Lab, step: &Self::Step) -> Result<(), Refused>;
    fn check(lab: &Self::Lab, step: &Self::Step) -> Result<(), Refused>;
    /// The scheduler's and adversary's enabled choices (the handler's own list).
    fn enabled(lab: &Self::Lab) -> Vec<Self::Step>;
    /// Program steps the log generator may offer in this state.
    fn program_options(lab: &Self::Lab, rng: &mut SplitMix) -> Vec<Self::Step>;
    fn is_program(step: &Self::Step) -> bool;
    fn is_crash(step: &Self::Step) -> bool;
    /// The recovery the handler enables right after `crash`: `Restart`, then `Truncate`
    /// while a torn record remains.
    fn recovery(after: &Self::Lab, crash: &Self::Step) -> Vec<Self::Step>;
    /// Each node's crash the handler does not enable in this state, with its refusal.
    fn crashes_not_enabled(lab: &Self::Lab) -> Vec<(u8, Refused)>;
    fn is_fence(event: &Self::Event) -> bool;
    fn events(lab: &Self::Lab) -> Vec<Self::Event>;
    fn encode(lab: &Self::Lab) -> Vec<u8>;
    fn decode(bytes: &[u8]) -> Result<(Self::Config, Vec<Self::Step>), String>;
    /// The pack's own `Journal::choice_log`: the log the journal records, each step as
    /// the handler normalized it.
    fn choice_log(lab: &Self::Lab) -> Vec<Self::Step>;
    /// The pack's own `Journal::replay`, encoded.
    fn own_replay(lab: &Self::Lab) -> Result<Vec<u8>, String>;
    /// The pack's own `run`: the encoded journal, or the refused index and refusal.
    fn own_run(config: Self::Config, steps: &[Self::Step]) -> Result<Vec<u8>, (usize, String)>;
    /// The terminal state, rendered: the outcome a replay must reproduce.
    fn outcome(lab: &Self::Lab) -> String;
}

fn refused_net(r: &net::Refusal) -> Refused {
    Refused {
        class: format!("{:?}", r.class()),
        reason: r.inconclusive_reason(),
        detail: format!("{r:?}"),
    }
}

fn refused_prc(r: &prc::Refusal) -> Refused {
    Refused {
        class: format!("{:?}", r.class()),
        reason: r.inconclusive_reason(),
        detail: format!("{r:?}"),
    }
}

fn refused_sto(r: &sto::Refusal) -> Refused {
    Refused {
        class: format!("{:?}", r.class()),
        reason: r.inconclusive_reason(),
        detail: format!("{r:?}"),
    }
}

struct Net;

impl Pack for Net {
    const ID: &'static str = "network";
    type Config = net::NetworkConfig;
    type Step = net::Step;
    type Event = net::Event;
    type Lab = net::Network;

    fn lab(config: Self::Config) -> Self::Lab {
        net::Network::new(config)
    }
    fn apply(lab: &mut Self::Lab, step: &Self::Step) -> Result<(), Refused> {
        lab.apply(step).map(|_| ()).map_err(|r| refused_net(&r))
    }
    fn check(lab: &Self::Lab, step: &Self::Step) -> Result<(), Refused> {
        lab.check(step).map_err(|r| refused_net(&r))
    }
    fn enabled(lab: &Self::Lab) -> Vec<Self::Step> {
        lab.enabled_choices()
    }
    fn program_options(lab: &Self::Lab, rng: &mut SplitMix) -> Vec<Self::Step> {
        let n = lab.config().nodes();
        let payload = net::Payload(vec![u8::try_from(lab.sent_count() % 251).expect("small")]);
        vec![
            net::Step::Send {
                src: net::NodeId(rng.node(n)),
                dst: net::NodeId(rng.node(n)),
                payload: payload.clone(),
            },
            net::Step::Send {
                src: net::NodeId(rng.node(n)),
                dst: net::NodeId(rng.node(n)),
                payload,
            },
            net::Step::Partition(net::NodeSet(1 << rng.node(n))),
        ]
    }
    fn is_program(step: &Self::Step) -> bool {
        step.chooser() == net::step::Chooser::Program
    }
    fn is_crash(_: &Self::Step) -> bool {
        false
    }
    fn recovery(_: &Self::Lab, _: &Self::Step) -> Vec<Self::Step> {
        Vec::new()
    }
    fn crashes_not_enabled(_: &Self::Lab) -> Vec<(u8, Refused)> {
        Vec::new()
    }
    fn is_fence(_: &Self::Event) -> bool {
        false
    }
    fn events(lab: &Self::Lab) -> Vec<Self::Event> {
        lab.events().to_vec()
    }
    fn encode(lab: &Self::Lab) -> Vec<u8> {
        lab.encode()
    }
    fn decode(bytes: &[u8]) -> Result<(Self::Config, Vec<Self::Step>), String> {
        decode_network(bytes)
    }
    fn choice_log(lab: &Self::Lab) -> Vec<Self::Step> {
        lab.clone().into_journal().choice_log().collect()
    }
    fn own_replay(lab: &Self::Lab) -> Result<Vec<u8>, String> {
        lab.clone()
            .into_journal()
            .replay()
            .map(|j| j.encode())
            .map_err(|r| r.to_string())
    }
    fn own_run(config: Self::Config, steps: &[Self::Step]) -> Result<Vec<u8>, (usize, String)> {
        net::run(config, steps)
            .map(|j| j.encode())
            .map_err(|r| (r.index, format!("{:?}", r.refusal)))
    }
    fn outcome(lab: &Self::Lab) -> String {
        format!(
            "in-flight {:?}; partition {:?}; partitions used {}; sent {}",
            lab.in_flight().collect::<Vec<_>>(),
            lab.partition(),
            lab.partitions_used(),
            lab.sent_count()
        )
    }
}

struct Proc;

impl Pack for Proc {
    const ID: &'static str = "process";
    type Config = prc::ProcessConfig;
    type Step = prc::Step;
    type Event = prc::Event;
    type Lab = prc::Process;

    fn lab(config: Self::Config) -> Self::Lab {
        prc::Process::new(config)
    }
    fn apply(lab: &mut Self::Lab, step: &Self::Step) -> Result<(), Refused> {
        lab.apply(step).map(|_| ()).map_err(|r| refused_prc(&r))
    }
    fn check(lab: &Self::Lab, step: &Self::Step) -> Result<(), Refused> {
        lab.check(step).map_err(|r| refused_prc(&r))
    }
    fn enabled(lab: &Self::Lab) -> Vec<Self::Step> {
        lab.enabled_choices()
    }
    fn program_options(lab: &Self::Lab, rng: &mut SplitMix) -> Vec<Self::Step> {
        let n = lab.config().nodes();
        vec![
            prc::Step::Begin(prc::NodeId(rng.node(n))),
            prc::Step::Begin(prc::NodeId(rng.node(n))),
        ]
    }
    fn is_program(step: &Self::Step) -> bool {
        step.chooser() == prc::step::Chooser::Program
    }
    fn is_crash(step: &Self::Step) -> bool {
        matches!(step, prc::Step::Crash(_))
    }
    fn recovery(after: &Self::Lab, crash: &Self::Step) -> Vec<Self::Step> {
        let prc::Step::Crash(node) = crash else {
            return Vec::new();
        };
        after
            .enabled_choices()
            .into_iter()
            .filter(|s| *s == prc::Step::Restart(*node))
            .collect()
    }
    fn crashes_not_enabled(lab: &Self::Lab) -> Vec<(u8, Refused)> {
        (0..lab.config().nodes())
            .filter_map(|n| {
                lab.check(&prc::Step::Crash(prc::NodeId(n)))
                    .err()
                    .map(|r| (n, refused_prc(&r)))
            })
            .collect()
    }
    fn is_fence(event: &Self::Event) -> bool {
        matches!(event, prc::Event::Fenced { .. })
    }
    fn events(lab: &Self::Lab) -> Vec<Self::Event> {
        lab.events().to_vec()
    }
    fn encode(lab: &Self::Lab) -> Vec<u8> {
        lab.encode()
    }
    fn decode(bytes: &[u8]) -> Result<(Self::Config, Vec<Self::Step>), String> {
        decode_process(bytes)
    }
    fn choice_log(lab: &Self::Lab) -> Vec<Self::Step> {
        lab.clone().into_journal().choice_log().collect()
    }
    fn own_replay(lab: &Self::Lab) -> Result<Vec<u8>, String> {
        lab.clone()
            .into_journal()
            .replay()
            .map(|j| j.encode())
            .map_err(|r| r.to_string())
    }
    fn own_run(config: Self::Config, steps: &[Self::Step]) -> Result<Vec<u8>, (usize, String)> {
        prc::run(config, steps)
            .map(|j| j.encode())
            .map_err(|r| (r.index, format!("{:?}", r.refusal)))
    }
    fn outcome(lab: &Self::Lab) -> String {
        let n = lab.config().nodes();
        format!(
            "up {:?}; epochs {:?}; crashes {}; pending {:?}; tickets {}",
            lab.up(),
            (0..n)
                .map(|i| lab.epoch(prc::NodeId(i)))
                .collect::<Vec<_>>(),
            lab.crashes(),
            lab.pending().collect::<Vec<_>>(),
            lab.ticket_count()
        )
    }
}

struct Stor;

impl Pack for Stor {
    const ID: &'static str = "storage";
    type Config = sto::StorageConfig;
    type Step = sto::Step;
    type Event = sto::Event;
    type Lab = sto::Storage;

    fn lab(config: Self::Config) -> Self::Lab {
        sto::Storage::new(config)
    }
    fn apply(lab: &mut Self::Lab, step: &Self::Step) -> Result<(), Refused> {
        lab.apply(step).map(|_| ()).map_err(|r| refused_sto(&r))
    }
    fn check(lab: &Self::Lab, step: &Self::Step) -> Result<(), Refused> {
        lab.check(step).map_err(|r| refused_sto(&r))
    }
    fn enabled(lab: &Self::Lab) -> Vec<Self::Step> {
        lab.enabled_choices()
    }
    fn program_options(lab: &Self::Lab, rng: &mut SplitMix) -> Vec<Self::Step> {
        let n = lab.config().nodes();
        let value = sto::Value(u32::try_from(lab.events().len()).expect("short"));
        vec![
            sto::Step::Submit {
                node: sto::NodeId(rng.node(n)),
                value,
            },
            sto::Step::Submit {
                node: sto::NodeId(rng.node(n)),
                value,
            },
            sto::Step::Sync(sto::NodeId(rng.node(n))),
            sto::Step::Truncate(sto::NodeId(rng.node(n))),
        ]
    }
    fn is_program(step: &Self::Step) -> bool {
        step.chooser() == sto::step::Chooser::Program
    }
    fn is_crash(step: &Self::Step) -> bool {
        matches!(step, sto::Step::Crash { .. })
    }
    fn recovery(after: &Self::Lab, crash: &Self::Step) -> Vec<Self::Step> {
        let sto::Step::Crash { node, .. } = crash else {
            return Vec::new();
        };
        let restart = sto::Step::Restart(*node);
        if !after.enabled_choices().contains(&restart) {
            return Vec::new();
        }
        let mut out = vec![restart];
        let mut restarted = after.clone();
        restarted.apply(&restart).expect("an enabled restart");
        if restarted.check(&sto::Step::Truncate(*node)).is_ok() {
            out.push(sto::Step::Truncate(*node));
        }
        out
    }
    fn crashes_not_enabled(lab: &Self::Lab) -> Vec<(u8, Refused)> {
        let enabled = lab.enabled_choices();
        (0..lab.config().nodes())
            .filter(|n| {
                !enabled
                    .iter()
                    .any(|s| matches!(s, sto::Step::Crash { node, .. } if node.0 == *n))
            })
            .map(|n| {
                let probe = sto::Step::Crash {
                    node: sto::NodeId(n),
                    keep: 0,
                    torn: false,
                };
                let refusal = lab
                    .check(&probe)
                    .expect_err("a node with no enabled crash outcome refuses keep 0");
                (n, refused_sto(&refusal))
            })
            .collect()
    }
    fn is_fence(event: &Self::Event) -> bool {
        matches!(event, sto::Event::Fenced { .. })
    }
    fn events(lab: &Self::Lab) -> Vec<Self::Event> {
        lab.events().to_vec()
    }
    fn encode(lab: &Self::Lab) -> Vec<u8> {
        lab.encode()
    }
    fn decode(bytes: &[u8]) -> Result<(Self::Config, Vec<Self::Step>), String> {
        decode_storage(bytes)
    }
    fn choice_log(lab: &Self::Lab) -> Vec<Self::Step> {
        lab.clone().into_journal().choice_log().collect()
    }
    fn own_replay(lab: &Self::Lab) -> Result<Vec<u8>, String> {
        lab.clone()
            .into_journal()
            .replay()
            .map(|j| j.encode())
            .map_err(|r| r.to_string())
    }
    fn own_run(config: Self::Config, steps: &[Self::Step]) -> Result<Vec<u8>, (usize, String)> {
        sto::run(config, steps)
            .map(|j| j.encode())
            .map_err(|r| (r.index, format!("{:?}", r.refusal)))
    }
    fn outcome(lab: &Self::Lab) -> String {
        let n = lab.config().nodes();
        let nodes: Vec<String> = (0..n)
            .map(|i| {
                let node = sto::NodeId(i);
                format!(
                    "{node}: epoch {:?} log {:?} stable {:?}",
                    lab.epoch(node),
                    lab.log(node),
                    lab.stable_len(node)
                )
            })
            .collect();
        format!(
            "up {:?}; {}; crashes {}; pending {:?}; tickets {}",
            lab.up(),
            nodes.join("; "),
            lab.crashes(),
            lab.pending().collect::<Vec<_>>(),
            lab.ticket_count()
        )
    }
}

// ---------------------------------------------------------------------------
// the declared windows
// ---------------------------------------------------------------------------

/// What a pack's own contract declares about its windows. The packs state this only
/// in their `CANCELLATION_CONTRACT` text (part of each profile's canonical bytes), not in
/// a typed field, so it is read from that text; `hon-01` pins the reading, and a change
/// to the text changes it visibly.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Declared {
    /// "crash windows are exactly the boundaries between steps".
    crash_windows: bool,
    /// Cancellation points are the pack's step boundaries: the `replay-choice` row says
    /// so, or the `early-return` row says a cancellation "lands before it or after it"
    /// and the pack does not route its cancellation points to the runtime.
    cancellation_points: bool,
    /// "its cancellation points are the runtime's (PR 14)": the pack models no
    /// cancellation of its own, and the runtime's cancellation points are the binding's
    /// (`pos-05`, and the PR-14 exit evidence).
    cancellation_routed_to_runtime: bool,
}

const CRASH_WINDOWS: &str = "crash windows are exactly the boundaries between steps";
const CANCELLATION_POINTS: &str = "cancellation points are exactly the boundaries between steps";
const LANDS: &str = "lands before it or after it";
const ROUTED: &str = "its cancellation points are the runtime's (PR 14)";

fn row<'a>(contract: &'a [(&'a str, &'a str)], id: &str) -> &'a str {
    contract
        .iter()
        .find(|(k, _)| *k == id)
        .map(|(_, v)| *v)
        .unwrap_or_else(|| panic!("the contract has no {id} row"))
}

fn declared(contract: &[(&str, &str)]) -> Declared {
    let replay = row(contract, "replay-choice");
    let routed = replay.contains(ROUTED);
    Declared {
        crash_windows: replay.contains(CRASH_WINDOWS),
        cancellation_points: !routed
            && (replay.contains(CANCELLATION_POINTS)
                || row(contract, "early-return").contains(LANDS)),
        cancellation_routed_to_runtime: routed,
    }
}

fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    haystack.windows(needle.len()).any(|w| w == needle)
}

// ---------------------------------------------------------------------------
// the window harness, generic over a pack
// ---------------------------------------------------------------------------

/// One realized window: what was recorded.
struct Record<P: Pack> {
    window: String,
    config: P::Config,
    log: Vec<P::Step>,
    lab: P::Lab,
    bytes: Vec<u8>,
    outcome: String,
    /// Continuation steps the new state did not enable.
    skipped: u64,
    /// The continuation step the handler refused as a program fault (storage only), with
    /// its refusal: a conclusive verdict against the program, so the window ends there
    /// and the fault itself is part of what must replay.
    fault: Option<(P::Step, Refused)>,
}

/// A seeded random walk: at each step an enabled choice or a program step.
fn generate<P: Pack>(config: P::Config, rng: &mut SplitMix, len: usize) -> Vec<P::Step> {
    let mut lab = P::lab(config);
    let mut log = Vec::new();
    let mut attempts = 0;
    while log.len() < len && attempts < len * 50 {
        attempts += 1;
        let mut options = P::enabled(&lab);
        options.extend(P::program_options(&lab, rng));
        let pick = options[rng.below(options.len())].clone();
        if P::apply(&mut lab, &pick).is_ok() {
            log.push(pick);
        }
    }
    log
}

/// Realize a window: `prefix`, then `splice`, then `rest` continued in the new state.
fn realize<P: Pack>(
    artifact: &'static str,
    window: String,
    config: P::Config,
    prefix: &[P::Step],
    splice: &[P::Step],
    rest: &[P::Step],
) -> Result<Record<P>, Gap> {
    let mut lab = P::lab(config);
    let mut log = Vec::new();
    for step in prefix {
        P::apply(&mut lab, step).map_err(|r| Gap {
            artifact,
            window: window.clone(),
            why: GapWhy::NotRealized(format!("prefix: {}", r.detail)),
        })?;
        log.push(step.clone());
    }
    for step in splice {
        P::apply(&mut lab, step).map_err(|r| r.gap(artifact, &window))?;
        log.push(step.clone());
    }
    let mut skipped = 0;
    let mut fault = None;
    for step in rest {
        match P::apply(&mut lab, step) {
            Ok(()) => log.push(step.clone()),
            Err(r) if r.reason.is_some() => return Err(r.gap(artifact, &window)),
            Err(r) if r.class == "ProgramFault" => {
                fault = Some((step.clone(), r));
                break;
            }
            Err(_) => skipped += 1,
        }
    }
    Ok(Record {
        window,
        config,
        bytes: P::encode(&lab),
        outcome: P::outcome(&lab),
        log,
        lab,
        skipped,
        fault,
    })
}

/// Run `steps` from an empty handler under `config`.
fn replay_steps<P: Pack>(config: P::Config, steps: &[P::Step]) -> Result<P::Lab, Divergence> {
    let mut lab = P::lab(config);
    for (index, step) in steps.iter().enumerate() {
        P::apply(&mut lab, step).map_err(|r| Divergence::Refused {
            index,
            class: r.class,
        })?;
    }
    Ok(lab)
}

/// Run the choice log decoded from recorded bytes.
fn replay_bytes<P: Pack>(bytes: &[u8]) -> Result<P::Lab, Divergence> {
    let (config, steps) = P::decode(bytes).map_err(Divergence::Undecodable)?;
    replay_steps::<P>(config, &steps)
}

/// The first way a replay is not the record, or `None` when it is the record exactly.
fn diverges<P: Pack>(record: &Record<P>, got: Result<P::Lab, Divergence>) -> Option<Divergence> {
    let lab = match got {
        Err(d) => return Some(d),
        Ok(lab) => lab,
    };
    let (want, have) = (P::events(&record.lab), P::events(&lab));
    if let Some(index) = want.iter().zip(&have).position(|(a, b)| a != b) {
        return Some(Divergence::EventDiffers { index });
    }
    if want.len() != have.len() {
        return Some(Divergence::LengthDiffers {
            recorded: want.len(),
            replayed: have.len(),
        });
    }
    if P::encode(&lab) != record.bytes {
        return Some(Divergence::BytesDiffer);
    }
    if P::outcome(&lab) != record.outcome {
        return Some(Divergence::OutcomeDiffers);
    }
    None
}

/// Replay a record every way the exit names; the number of replays, or the first
/// divergence.
fn exact_replay<P: Pack>(record: &Record<P>) -> Result<u64, Divergence> {
    let (config, steps) = P::decode(&record.bytes).map_err(Divergence::Undecodable)?;
    if config != record.config {
        return Err(Divergence::HeaderDiffers);
    }
    // The journal is its own choice log: what the decoder reads from the bytes is the
    // log the pack says the journal records.
    if steps != P::choice_log(&record.lab) {
        return Err(Divergence::LogDiffers);
    }
    let mut replays = 0;
    for _ in 0..REPEATS {
        if let Some(d) = diverges(record, replay_bytes::<P>(&record.bytes)) {
            return Err(d);
        }
        replays += 1;
    }
    if let Some(d) = diverges(record, replay_steps::<P>(record.config, &record.log)) {
        return Err(d);
    }
    replays += 1;
    match P::own_replay(&record.lab) {
        Ok(bytes) if bytes == record.bytes => replays += 1,
        Ok(_) => return Err(Divergence::BytesDiffer),
        Err(why) => {
            return Err(Divergence::Refused {
                index: 0,
                class: why,
            });
        }
    }
    // A window that ended in a program fault: the fault replays too, at the same index
    // and with the same refusal, through the harness and through the pack's own `run`.
    if let Some((step, refusal)) = &record.fault {
        let mut faulting = record.log.clone();
        faulting.push(step.clone());
        let replayed =
            replay_steps::<P>(record.config, &record.log).map(|lab| P::check(&lab, step));
        if replayed != Ok(Err(refusal.clone())) {
            return Err(Divergence::OutcomeDiffers);
        }
        if P::own_run(record.config, &faulting) != Err((record.log.len(), refusal.detail.clone())) {
            return Err(Divergence::OutcomeDiffers);
        }
        replays += 2;
    }
    Ok(replays)
}

/// Counts and gaps for one artifact.
#[derive(Debug, Clone, Default)]
struct Tally {
    counts: BTreeMap<String, u64>,
    gaps: Vec<Gap>,
    /// Perturbations a replay accepted: the negative evidence requires none.
    undetected: Vec<String>,
    /// BLAKE3 input: every recorded journal, length-prefixed, in order.
    corpus: Vec<u8>,
}

impl Tally {
    fn add(&mut self, key: impl Into<String>, n: u64) {
        *self.counts.entry(key.into()).or_default() += n;
    }

    fn get(&self, key: &str) -> u64 {
        self.counts.get(key).copied().unwrap_or(0)
    }

    fn retain(&mut self, bytes: &[u8]) {
        self.corpus
            .extend_from_slice(&u64::try_from(bytes.len()).expect("small").to_be_bytes());
        self.corpus.extend_from_slice(bytes);
    }

    fn digest(&self) -> String {
        Blake3Hasher::hash(&self.corpus).to_string()
    }

    fn absorb(mut self, other: Self) -> Self {
        for (k, v) in other.counts {
            *self.counts.entry(k).or_default() += v;
        }
        self.gaps.extend(other.gaps);
        self.undetected.extend(other.undetected);
        self.corpus.extend(other.corpus);
        self
    }
}

/// Record a window, replay it exactly, and fold the result into `tally`. The record,
/// when there is one, for the caller's perturbations.
fn record_window<P: Pack>(
    tally: &mut Tally,
    realized: Result<Record<P>, Gap>,
    original: &[P::Event],
    at: usize,
) -> Option<Record<P>> {
    let record = match realized {
        Ok(record) => record,
        Err(gap) => {
            tally.gaps.push(gap);
            return None;
        }
    };
    tally.retain(&record.bytes);
    tally.add("continuation steps not enabled", record.skipped);
    tally.add(
        "windows ending in a program fault, the fault replayed",
        u64::from(record.fault.is_some()),
    );
    if P::events(&record.lab)[..at] != original[..at] {
        tally.gaps.push(Gap {
            artifact: P::ID,
            window: record.window.clone(),
            why: GapWhy::PrefixRewritten,
        });
        return None;
    }
    match exact_replay(&record) {
        Ok(replays) => tally.add(
            "replays, each byte-identical with an identical outcome",
            replays,
        ),
        Err(d) => {
            tally.gaps.push(Gap {
                artifact: P::ID,
                window: record.window.clone(),
                why: GapWhy::Diverged(d),
            });
            return None;
        }
    }
    Some(record)
}

/// Replay a perturbed plan against `record`: it must diverge, typed.
fn expect_divergence<P: Pack>(
    tally: &mut Tally,
    record: &Record<P>,
    what: &str,
    perturbed: &[P::Step],
) {
    match diverges(record, replay_steps::<P>(record.config, perturbed)) {
        None => tally
            .undetected
            .push(format!("{}: {what} at {}", P::ID, record.window)),
        Some(d) => {
            tally.add(format!("{what}: {}", d.kind()), 1);
            if !matches!(d, Divergence::Refused { .. }) && perturbed.len() == record.log.len() {
                // A valid run of the same length: a checker that compared lengths only
                // would have accepted it.
                tally.add(format!("{what}: same length, detected by content"), 1);
            }
        }
    }
}

/// Replay a plan the evidence classes as equivalent to `record`: it must replay as the
/// record exactly, or the equivalence is a gap.
fn expect_same<P: Pack>(tally: &mut Tally, record: &Record<P>, what: &str, same: &[P::Step]) {
    match diverges(record, replay_steps::<P>(record.config, same)) {
        None => tally.add(format!("equivalent, verified: {what}"), 1),
        Some(d) => tally.gaps.push(Gap {
            artifact: P::ID,
            window: format!("{} {what}", record.window),
            why: GapWhy::Diverged(d),
        }),
    }
}

/// Every crash window and cancellation point `declared` names, over one log.
fn windows_over_log<P: Pack>(
    tally: &mut Tally,
    declared: Declared,
    config: P::Config,
    log: &[P::Step],
    label: &str,
) {
    let original = match replay_steps::<P>(config, log) {
        Ok(lab) => P::events(&lab),
        Err(d) => {
            tally.gaps.push(Gap {
                artifact: P::ID,
                window: format!("{label}: the generated log"),
                why: GapWhy::Diverged(d),
            });
            return;
        }
    };
    let mut state = P::lab(config);
    for at in 0..=log.len() {
        tally.add("boundaries", 1);
        if declared.crash_windows {
            for (_, refusal) in P::crashes_not_enabled(&state) {
                if refusal.reason.is_some() {
                    tally
                        .gaps
                        .push(refusal.gap(P::ID, &format!("{label}@{at}")));
                } else {
                    tally.add(format!("crash not enabled: {}", refusal.reason_name()), 1);
                }
            }
            let crashes: Vec<P::Step> =
                P::enabled(&state).into_iter().filter(P::is_crash).collect();
            for crash in crashes {
                let mut after = state.clone();
                P::apply(&mut after, &crash).expect("an enabled crash");
                let recovery = P::recovery(&after, &crash);
                let mut splices = vec![vec![crash.clone()]];
                if !recovery.is_empty() {
                    let mut s = vec![crash.clone()];
                    s.extend(recovery);
                    splices.push(s);
                }
                for splice in splices {
                    let kind = if splice.len() == 1 {
                        "crash"
                    } else {
                        "crash with recovery"
                    };
                    let window = format!("{label}@{at} {kind} {splice:?}");
                    tally.add(format!("{kind} windows"), 1);
                    let realized =
                        realize::<P>(P::ID, window, config, &log[..at], &splice, &log[at..]);
                    let Some(record) = record_window(tally, realized, &original, at) else {
                        continue;
                    };
                    tally.add(
                        "fenced completions after the crash",
                        u64::try_from(
                            P::events(&record.lab)[at..]
                                .iter()
                                .filter(|e| P::is_fence(e))
                                .count(),
                        )
                        .expect("small"),
                    );
                    // The crash is record.log[at]. Move it one step each way, and drop it.
                    // Every window gets one disposition per perturbation: detected, or
                    // absent (no step on that side), or equivalent (the neighbour is the
                    // same step, so the move changes nothing), verified.
                    let r = &record.log;
                    for (what, neighbour) in [
                        (
                            "crash moved one step later",
                            (at + 1 < r.len()).then_some(at + 1),
                        ),
                        ("crash moved one step earlier", at.checked_sub(1)),
                    ] {
                        match neighbour {
                            None => tally.add(format!("absent: {what}, no step on that side"), 1),
                            Some(j) => {
                                let mut moved = r.clone();
                                moved.swap(at, j);
                                if r[j] == r[at] {
                                    expect_same(tally, &record, what, &moved);
                                } else {
                                    expect_divergence(tally, &record, what, &moved);
                                }
                            }
                        }
                    }
                    let mut dropped = r.clone();
                    dropped.remove(at);
                    expect_divergence(tally, &record, "crash dropped", &dropped);
                }
            }
        }
        if declared.cancellation_points {
            tally.add("cancellation points", 1);
            let cancel_at = |point: usize| -> Result<Record<P>, Gap> {
                let rest: Vec<P::Step> = log[point..]
                    .iter()
                    .filter(|s| !P::is_program(s))
                    .cloned()
                    .collect();
                realize::<P>(
                    P::ID,
                    format!("{label}@{point} cancel"),
                    config,
                    &log[..point],
                    &[],
                    &rest,
                )
            };
            if let Some(record) = record_window(tally, cancel_at(at), &original, at) {
                tally.add(
                    "events after a cancellation point (late completions)",
                    u64::try_from(P::events(&record.lab).len() - at).expect("small"),
                );
                // Each point gets one disposition per perturbation. The first program
                // step at or after the point is the work the cancellation withholds.
                let next_program = (at..log.len()).find(|j| P::is_program(&log[*j]));
                // Dropped: the uncancelled log. With no program step after the point it is
                // the same log, verified equivalent; otherwise it must diverge.
                match next_program {
                    Some(_) => expect_divergence(tally, &record, "cancellation dropped", log),
                    None => expect_same(tally, &record, "cancellation dropped", log),
                }
                // Moved past the next program step: the point right after it. Every point
                // that withholds any program step is exercised this way.
                match next_program {
                    Some(j) => match cancel_at(j + 1) {
                        Ok(moved) => expect_divergence(
                            tally,
                            &record,
                            "cancellation moved past the next program step",
                            &moved.log,
                        ),
                        Err(gap) => tally.gaps.push(gap),
                    },
                    None => tally.add(
                        "absent: cancellation moved past the next program step, none remains",
                        1,
                    ),
                }
                // Moved one step later: across the step at the point. A program step
                // there must diverge; any other step leaves the same withheld work, so
                // the two points are verified equivalent; the last boundary has no later
                // point.
                if at == log.len() {
                    tally.add(
                        "absent: cancellation moved one step later, the last boundary",
                        1,
                    );
                } else {
                    match cancel_at(at + 1) {
                        Ok(moved) if P::is_program(&log[at]) => expect_divergence(
                            tally,
                            &record,
                            "cancellation moved one step later",
                            &moved.log,
                        ),
                        Ok(moved) => expect_same(
                            tally,
                            &record,
                            "cancellation moved one step later",
                            &moved.log,
                        ),
                        Err(gap) => tally.gaps.push(gap),
                    }
                }
            }
        }
        if at < log.len() {
            P::apply(&mut state, &log[at]).expect("the generated log is valid");
        }
    }
}

/// The pack's corpus: `LOGS_PER_SEED` logs per seed, generated under `generator` and
/// run under `config` (a larger crash budget, so a crash window exists after the log's
/// own crashes too).
fn pack_campaign<P: Pack>(declared: Declared, generator: P::Config, config: P::Config) -> Tally {
    let mut tally = Tally::default();
    for seed in SEEDS {
        let mut rng = SplitMix(seed);
        for n in 0..LOGS_PER_SEED {
            let log = generate::<P>(generator, &mut rng, LOG_LEN);
            tally.add("logs", 1);
            tally.add("log steps", u64::try_from(log.len()).expect("small"));
            windows_over_log::<P>(
                &mut tally,
                declared,
                config,
                &log,
                &format!("seed {seed:#x} log {n}"),
            );
        }
    }
    tally
}

fn network_config(partitions: u32) -> net::NetworkConfig {
    net::NetworkConfig::new(
        3,
        16,
        8,
        net::FaultSwitches {
            loss: true,
            duplication: true,
            reordering: true,
        },
        partitions,
        RETAINED,
    )
    .expect("a network configuration")
}

/// The second configuration of each pack: every fault switch off and the tightest
/// budgets, so windows meet a spent crash budget, no restart and an undeclared fault.
fn network_tight() -> net::NetworkConfig {
    net::NetworkConfig::new(
        3,
        4,
        8,
        net::FaultSwitches {
            loss: false,
            duplication: false,
            reordering: false,
        },
        0,
        RETAINED,
    )
    .expect("a network configuration")
}

fn process_tight() -> prc::ProcessConfig {
    prc::ProcessConfig::new(3, 1, false, 8, RETAINED).expect("a process configuration")
}

fn storage_tight() -> sto::StorageConfig {
    sto::StorageConfig::new(2, 1, false, false, false, 8, RETAINED)
        .expect("a storage configuration")
}

fn process_config(crashes: u32) -> prc::ProcessConfig {
    prc::ProcessConfig::new(3, crashes, true, 8, RETAINED).expect("a process configuration")
}

fn storage_config(crashes: u32) -> sto::StorageConfig {
    sto::StorageConfig::new(2, crashes, true, true, true, 8, RETAINED)
        .expect("a storage configuration")
}

fn network_declared() -> Declared {
    declared(&net::profile::CANCELLATION_CONTRACT)
}

fn process_declared() -> Declared {
    declared(&prc::profile::CANCELLATION_CONTRACT)
}

fn storage_declared() -> Declared {
    declared(&sto::profile::CANCELLATION_CONTRACT)
}

// ---------------------------------------------------------------------------
// the composed run: network, process and storage at one boundary
// ---------------------------------------------------------------------------

/// One composed step. A composed crash or restart is forwarded to the process and the
/// storage pack at the same boundary, and takes effect only if both accept it (the
/// packs' composition rows); the network pack is not told.
#[derive(Debug, Clone, PartialEq)]
enum C {
    N(net::Step),
    P(prc::Step),
    S(sto::Step),
    Crash { node: u8, keep: u32, torn: bool },
    Restart(u8),
}

#[derive(Debug, Clone)]
struct Trio {
    n: net::Network,
    p: prc::Process,
    s: sto::Storage,
}

const COMPOSED_NODES: u8 = 3;

fn trio(crashes: u32) -> Trio {
    Trio {
        n: net::Network::new(network_config(1)),
        p: prc::Process::new(process_config(crashes)),
        s: sto::Storage::new(
            sto::StorageConfig::new(COMPOSED_NODES, crashes, true, true, true, 8, RETAINED)
                .expect("a storage configuration"),
        ),
    }
}

fn composed_crash(node: u8, keep: u32, torn: bool) -> (prc::Step, sto::Step) {
    (
        prc::Step::Crash(prc::NodeId(node)),
        sto::Step::Crash {
            node: sto::NodeId(node),
            keep,
            torn,
        },
    )
}

fn capply(t: &mut Trio, c: &C) -> Result<(), Refused> {
    match c {
        C::N(s) => Net::apply(&mut t.n, s),
        C::P(s) => Proc::apply(&mut t.p, s),
        C::S(s) => Stor::apply(&mut t.s, s),
        C::Crash { node, keep, torn } => {
            let (p, s) = composed_crash(*node, *keep, *torn);
            Proc::check(&t.p, &p)?;
            Stor::check(&t.s, &s)?;
            Proc::apply(&mut t.p, &p).expect("checked");
            Stor::apply(&mut t.s, &s).expect("checked");
            Ok(())
        }
        C::Restart(node) => {
            let (p, s) = (
                prc::Step::Restart(prc::NodeId(*node)),
                sto::Step::Restart(sto::NodeId(*node)),
            );
            Proc::check(&t.p, &p)?;
            Stor::check(&t.s, &s)?;
            Proc::apply(&mut t.p, &p).expect("checked");
            Stor::apply(&mut t.s, &s).expect("checked");
            Ok(())
        }
    }
}

/// The composed crash outcomes enabled at a boundary: the storage pack's enabled crash
/// outcomes of each node the process pack also lets crash. `Err` names a node on which
/// the two packs disagree, which the composition rows exclude.
fn composed_crashes(t: &Trio) -> Result<Vec<C>, String> {
    let enabled = t.s.enabled_choices();
    let mut out = Vec::new();
    for node in 0..COMPOSED_NODES {
        let process = t.p.check(&prc::Step::Crash(prc::NodeId(node))).is_ok();
        let outcomes: Vec<C> = enabled
            .iter()
            .filter_map(|s| match s {
                sto::Step::Crash {
                    node: n,
                    keep,
                    torn,
                } if n.0 == node => Some(C::Crash {
                    node,
                    keep: *keep,
                    torn: *torn,
                }),
                _ => None,
            })
            .collect();
        if process != !outcomes.is_empty() {
            return Err(format!(
                "node {node}: the process pack {} a crash, the storage pack enables {}",
                if process { "enables" } else { "refuses" },
                outcomes.len()
            ));
        }
        out.extend(outcomes);
    }
    Ok(out)
}

fn cgenerate(rng: &mut SplitMix, len: usize) -> Vec<C> {
    let mut t = trio(1);
    let mut log = Vec::new();
    let mut attempts = 0;
    while log.len() < len && attempts < len * 50 {
        attempts += 1;
        let mut options: Vec<C> = Net::enabled(&t.n).into_iter().map(C::N).collect();
        options.extend(Net::program_options(&t.n, rng).into_iter().map(C::N));
        options.extend(
            Proc::enabled(&t.p)
                .into_iter()
                .filter(|s| matches!(s, prc::Step::Complete(_) | prc::Step::Delay(_)))
                .map(C::P),
        );
        options.extend(Proc::program_options(&t.p, rng).into_iter().map(C::P));
        options.extend(
            Stor::enabled(&t.s)
                .into_iter()
                .filter(|s| !matches!(s, sto::Step::Crash { .. } | sto::Step::Restart(_)))
                .map(C::S),
        );
        options.extend(Stor::program_options(&t.s, rng).into_iter().map(C::S));
        options.extend(composed_crashes(&t).expect("the packs agree"));
        options.extend((0..COMPOSED_NODES).map(C::Restart));
        let pick = options[rng.below(options.len())].clone();
        if capply(&mut t, &pick).is_ok() {
            log.push(pick);
        }
    }
    log
}

struct CRecord {
    window: String,
    log: Vec<C>,
    trio: Trio,
    bytes: [Vec<u8>; 3],
    outcome: String,
    /// The continuation step refused as a program fault, with its refusal: the window
    /// ends there, and the fault must replay.
    fault: Option<(C, Refused)>,
}

fn cbytes(t: &Trio) -> [Vec<u8>; 3] {
    [t.n.encode(), t.p.encode(), t.s.encode()]
}

fn coutcome(t: &Trio) -> String {
    format!(
        "{} | {} | {}",
        Net::outcome(&t.n),
        Proc::outcome(&t.p),
        Stor::outcome(&t.s)
    )
}

fn crun(steps: &[C]) -> Result<Trio, Divergence> {
    let mut t = trio(3);
    for (index, c) in steps.iter().enumerate() {
        capply(&mut t, c).map_err(|r| Divergence::Refused {
            index,
            class: r.class,
        })?;
    }
    Ok(t)
}

fn first_event_difference<E: PartialEq>(want: &[E], have: &[E]) -> Option<Divergence> {
    if let Some(index) = want.iter().zip(have).position(|(a, b)| a != b) {
        return Some(Divergence::EventDiffers { index });
    }
    (want.len() != have.len()).then_some(Divergence::LengthDiffers {
        recorded: want.len(),
        replayed: have.len(),
    })
}

fn cdiverges(record: &CRecord, got: Result<Trio, Divergence>) -> Option<Divergence> {
    let t = match got {
        Err(d) => return Some(d),
        Ok(t) => t,
    };
    first_event_difference(record.trio.n.events(), t.n.events())
        .or_else(|| first_event_difference(record.trio.p.events(), t.p.events()))
        .or_else(|| first_event_difference(record.trio.s.events(), t.s.events()))
        .or_else(|| (cbytes(&t) != record.bytes).then_some(Divergence::BytesDiffer))
        .or_else(|| (coutcome(&t) != record.outcome).then_some(Divergence::OutcomeDiffers))
}

/// Replay a composed record from its three recorded journals and the record's tag
/// sequence (which pack each step went to). Each step and each configuration come from
/// the bytes; the cross-pack interleaving is the recorded plan's, since no pack journal
/// holds it.
fn creplay_bytes(record: &CRecord) -> Result<Trio, Divergence> {
    let (nc, mut ns) = decode_network(&record.bytes[0]).map_err(Divergence::Undecodable)?;
    let (pc, mut ps) = decode_process(&record.bytes[1]).map_err(Divergence::Undecodable)?;
    let (sc, mut ss) = decode_storage(&record.bytes[2]).map_err(Divergence::Undecodable)?;
    let fresh = trio(3);
    if (nc, pc, sc) != (*fresh.n.config(), *fresh.p.config(), *fresh.s.config()) {
        return Err(Divergence::HeaderDiffers);
    }
    ns.reverse();
    ps.reverse();
    ss.reverse();
    let short = || Divergence::Undecodable("a journal is shorter than its tags".to_owned());
    let mut steps = Vec::new();
    for c in &record.log {
        steps.push(match c {
            C::N(_) => C::N(ns.pop().ok_or_else(short)?),
            C::P(_) => C::P(ps.pop().ok_or_else(short)?),
            C::S(_) => C::S(ss.pop().ok_or_else(short)?),
            C::Crash { .. } => match (ps.pop(), ss.pop()) {
                (Some(prc::Step::Crash(a)), Some(sto::Step::Crash { node, keep, torn }))
                    if a.0 == node.0 =>
                {
                    C::Crash {
                        node: node.0,
                        keep,
                        torn,
                    }
                }
                _ => return Err(Divergence::Undecodable("a composed crash".to_owned())),
            },
            C::Restart(_) => match (ps.pop(), ss.pop()) {
                (Some(prc::Step::Restart(a)), Some(sto::Step::Restart(b))) if a.0 == b.0 => {
                    C::Restart(a.0)
                }
                _ => return Err(Divergence::Undecodable("a composed restart".to_owned())),
            },
        });
    }
    if !(ns.is_empty() && ps.is_empty() && ss.is_empty()) {
        return Err(Divergence::Undecodable(
            "a journal is longer than its tags".to_owned(),
        ));
    }
    crun(&steps)
}

fn cexpect_divergence(tally: &mut Tally, record: &CRecord, what: &str, perturbed: &[C]) {
    match cdiverges(record, crun(perturbed)) {
        None => tally
            .undetected
            .push(format!("composed: {what} at {}", record.window)),
        Some(d) => {
            tally.add(format!("{what}: {}", d.kind()), 1);
            if !matches!(d, Divergence::Refused { .. }) && perturbed.len() == record.log.len() {
                tally.add(format!("{what}: same length, detected by content"), 1);
            }
        }
    }
}

/// Every composed crash window over seeded composed logs.
fn composed_campaign() -> Tally {
    let mut tally = Tally::default();
    for seed in SEEDS {
        let mut rng = SplitMix(seed ^ 0xc0);
        for n in 0..2 {
            let log = cgenerate(&mut rng, 24);
            tally.add("logs", 1);
            let original = crun(&log).expect("the generated composed log is valid");
            let mut state = trio(3);
            let mut seen: BTreeMap<(usize, usize, String), [Vec<u8>; 3]> = BTreeMap::new();
            for at in 0..=log.len() {
                tally.add("boundaries", 1);
                let crashes = match composed_crashes(&state) {
                    Ok(c) => {
                        tally.add("boundaries where the two packs agree on every node", 1);
                        c
                    }
                    Err(why) => {
                        tally.gaps.push(Gap {
                            artifact: "composed",
                            window: format!("seed {seed:#x} log {n}@{at}"),
                            why: GapWhy::NotRealized(why),
                        });
                        Vec::new()
                    }
                };
                for crash in crashes {
                    let C::Crash { node, .. } = crash else {
                        unreachable!()
                    };
                    let mut after = state.clone();
                    capply(&mut after, &crash).expect("an enabled composed crash");
                    let mut splices = vec![vec![crash.clone()]];
                    if capply(&mut after.clone(), &C::Restart(node)).is_ok() {
                        let mut s = vec![crash.clone(), C::Restart(node)];
                        let mut restarted = after.clone();
                        capply(&mut restarted, &C::Restart(node)).expect("checked");
                        let truncate = C::S(sto::Step::Truncate(sto::NodeId(node)));
                        if capply(&mut restarted, &truncate).is_ok() {
                            s.push(truncate);
                        }
                        splices.push(s);
                    }
                    for splice in splices {
                        let window = format!("seed {seed:#x} log {n}@{at} {splice:?}");
                        let kind = if splice.len() == 1 {
                            "crash windows"
                        } else {
                            "crash with recovery windows"
                        };
                        let mut t = state.clone();
                        let mut realized: Vec<C> = log[..at].to_vec();
                        let mut failed = None;
                        for c in &splice {
                            match capply(&mut t, c) {
                                Ok(()) => realized.push(c.clone()),
                                Err(r) => {
                                    failed = Some(r.gap("composed", &window));
                                    break;
                                }
                            }
                        }
                        if let Some(gap) = failed {
                            tally.gaps.push(gap);
                            continue;
                        }
                        let mut fault = None;
                        for c in &log[at..] {
                            match capply(&mut t, c) {
                                Ok(()) => realized.push(c.clone()),
                                Err(r) if r.reason.is_some() => {
                                    failed = Some(r.gap("composed", &window));
                                    break;
                                }
                                Err(r) if r.class == "ProgramFault" => {
                                    fault = Some((c.clone(), r));
                                    break;
                                }
                                Err(_) => tally.add("continuation steps not enabled", 1),
                            }
                        }
                        if let Some(gap) = failed {
                            tally.gaps.push(gap);
                            continue;
                        }
                        let record = CRecord {
                            window,
                            bytes: cbytes(&t),
                            outcome: coutcome(&t),
                            log: realized,
                            trio: t,
                            fault,
                        };
                        // Boundaries separated only by network steps give the crash the
                        // same place in every pack journal: one window, not two. The
                        // repeat must record the same bytes; it is verified and not
                        // counted again.
                        let key = (
                            state.p.events().len(),
                            state.s.events().len(),
                            format!("{splice:?}"),
                        );
                        match seen.get(&key) {
                            Some(bytes) if *bytes == record.bytes => {
                                tally.add(
                                    "the same window across network steps, bytes verified equal",
                                    1,
                                );
                                continue;
                            }
                            Some(_) => {
                                tally.gaps.push(Gap {
                                    artifact: "composed",
                                    window: record.window.clone(),
                                    why: GapWhy::Diverged(Divergence::BytesDiffer),
                                });
                                continue;
                            }
                            None => {
                                seen.insert(key, record.bytes.clone());
                            }
                        }
                        tally.add(kind, 1);
                        for b in &record.bytes {
                            tally.retain(b);
                        }
                        if let Some((step, refusal)) = &record.fault {
                            let replayed = crun(&record.log).map(|mut t| capply(&mut t, step));
                            if replayed == Ok(Err(refusal.clone())) {
                                tally.add(
                                    "windows ending in a program fault, the fault replayed",
                                    1,
                                );
                            } else {
                                tally.gaps.push(Gap {
                                    artifact: "composed",
                                    window: record.window.clone(),
                                    why: GapWhy::Diverged(Divergence::OutcomeDiffers),
                                });
                                continue;
                            }
                        }
                        // A crash does not touch the network pack: its journal is the
                        // uncrashed run's, or a prefix of it when a program fault ended
                        // the window early.
                        let (have, want) = (record.trio.n.events(), original.n.events());
                        if have == want || (record.fault.is_some() && want.starts_with(have)) {
                            tally.add("network journal unchanged by the crash", 1);
                        } else {
                            tally.gaps.push(Gap {
                                artifact: "composed",
                                window: record.window.clone(),
                                why: GapWhy::NotRealized(
                                    "the crash changed the network journal".to_owned(),
                                ),
                            });
                        }
                        tally.add(
                            "fenced completions after the crash",
                            u64::try_from(
                                record
                                    .trio
                                    .p
                                    .events()
                                    .iter()
                                    .filter(|e| Proc::is_fence(e))
                                    .count()
                                    + record
                                        .trio
                                        .s
                                        .events()
                                        .iter()
                                        .filter(|e| Stor::is_fence(e))
                                        .count(),
                            )
                            .expect("small"),
                        );
                        let mut ok = true;
                        for _ in 0..REPEATS {
                            if let Some(d) = cdiverges(&record, creplay_bytes(&record)) {
                                tally.gaps.push(Gap {
                                    artifact: "composed",
                                    window: record.window.clone(),
                                    why: GapWhy::Diverged(d),
                                });
                                ok = false;
                                break;
                            }
                            tally.add("replays, each byte-identical with an identical outcome", 1);
                        }
                        if !ok {
                            continue;
                        }
                        if let Some(d) = cdiverges(&record, crun(&record.log)) {
                            tally.gaps.push(Gap {
                                artifact: "composed",
                                window: record.window.clone(),
                                why: GapWhy::Diverged(d),
                            });
                            continue;
                        }
                        tally.add("replays, each byte-identical with an identical outcome", 1);
                        // The crash is record.log[at]. The recorded journals hold each
                        // pack's own order, so "one step" is one step of a pack the crash
                        // reaches: the nearest process or storage step. Moving it past
                        // network steps only commutes with them (the packs share no
                        // state), which no journal records; those are counted apart.
                        let r = &record.log;
                        let touches = |c: &C| !matches!(c, C::N(_));
                        let later = (at + 1..r.len()).find(|j| touches(&r[*j]));
                        let earlier = (0..at).rev().find(|j| touches(&r[*j]));
                        // Every place the crash can take by crossing network steps alone
                        // must replay as the record: verified, not assumed.
                        for j in earlier.map_or(0, |e| e + 1)..later.unwrap_or(r.len()) {
                            if j == at {
                                continue;
                            }
                            let mut commuted = r.clone();
                            let crash = commuted.remove(at);
                            commuted.insert(j, crash);
                            match cdiverges(&record, crun(&commuted)) {
                                None => tally
                                    .add("crash moved across network steps: commutes, verified", 1),
                                Some(d) => tally.gaps.push(Gap {
                                    artifact: "composed",
                                    window: format!("{} commuted to {j}", record.window),
                                    why: GapWhy::Diverged(d),
                                }),
                            }
                        }
                        for (what, neighbour) in [
                            ("crash moved one step later", later),
                            ("crash moved one step earlier", earlier),
                        ] {
                            match neighbour {
                                None => tally.add(
                                    format!(
                                        "absent: {what}, no process or storage step on that side"
                                    ),
                                    1,
                                ),
                                Some(j) => {
                                    let mut moved = r.clone();
                                    let crash = moved.remove(at);
                                    moved.insert(j, crash);
                                    if r[j] == r[at] {
                                        match cdiverges(&record, crun(&moved)) {
                                            None => tally
                                                .add(format!("equivalent, verified: {what}"), 1),
                                            Some(d) => tally.gaps.push(Gap {
                                                artifact: "composed",
                                                window: format!("{} {what}", record.window),
                                                why: GapWhy::Diverged(d),
                                            }),
                                        }
                                    } else {
                                        cexpect_divergence(&mut tally, &record, what, &moved);
                                    }
                                }
                            }
                        }
                        let mut dropped = r.clone();
                        dropped.remove(at);
                        cexpect_divergence(&mut tally, &record, "crash dropped", &dropped);
                    }
                }
                if at < log.len() {
                    capply(&mut state, &log[at]).expect("the generated composed log is valid");
                }
            }
        }
    }
    tally
}

// ---------------------------------------------------------------------------
// the register: the binding's crash windows
// ---------------------------------------------------------------------------

fn binding_config(seed: u64) -> BindingConfig {
    BindingConfig::new(seed)
        .observing(Family::Effect)
        .observing(Family::Cancellation)
        .observing(Family::Obligation)
        .observing(Family::Time)
        .observing(Family::Channel)
}

fn variant(refusal: &BindingRefusal) -> String {
    format!("{refusal:?}")
        .split(|c: char| !c.is_alphanumeric())
        .next()
        .unwrap_or("")
        .to_owned()
}

/// The run report `register_baseline::run_one` gives, as the outcome a replay must
/// reproduce: findings, correspondence, quiescence and conservation counts.
fn register_outcome(built: &Built, epochs: u8, log: &ChoiceLog) -> (String, bool) {
    static REACH: OnceLock<BTreeMap<u8, BTreeSet<register::Raw>>> = OnceLock::new();
    let reach = REACH.get_or_init(|| {
        [1, 2]
            .into_iter()
            .map(|e| (e, register::reachable(e)))
            .collect()
    });
    let report = baseline::run_one(built, epochs, log, &reach[&epochs]);
    let clean = report.findings.is_empty();
    (
        Blake3Hasher::hash(format!("{report:?}").as_bytes()).to_string(),
        clean,
    )
}

fn bdiverges(
    recorded: &BJournal,
    bytes: &[u8],
    got: Result<BJournal, BindingRefusal>,
) -> Option<Divergence> {
    let j = match got {
        Err(r) => {
            return Some(Divergence::Refused {
                index: 0,
                class: variant(&r),
            });
        }
        Ok(j) => j,
    };
    first_event_difference(recorded.events(), j.events())
        .or_else(|| (j.encode().expect("encodes") != bytes).then_some(Divergence::BytesDiffer))
}

/// The one crash operation of replica `a` (actor 1): `Cancel` when graceful, `Crash`
/// when fail-stop.
fn crash_op(built: &Built, mode: CrashMode) -> Result<usize, String> {
    let at: Vec<usize> = built.programs[1]
        .iter()
        .enumerate()
        .filter(|(_, op)| match mode {
            CrashMode::Graceful => matches!(op, SubstrateOp::Cancel { .. }),
            CrashMode::FailStop => matches!(op, SubstrateOp::Crash { .. }),
        })
        .map(|(i, _)| i)
        .collect();
    match at.as_slice() {
        [one] => Ok(*one),
        other => Err(format!("{} crash operations in replica a", other.len())),
    }
}

fn bexpect_divergence(
    tally: &mut Tally,
    what: &str,
    recorded: &BJournal,
    bytes: &[u8],
    perturbed: &Built,
    log: &ChoiceLog,
    window: &str,
) {
    match bdiverges(
        recorded,
        bytes,
        run_binding(&perturbed.programs, log, &binding_config(0)),
    ) {
        None => tally
            .undetected
            .push(format!("register: {what} at {window}")),
        Some(Divergence::Refused { class, .. }) => {
            tally.add(format!("{what}: refused {class}"), 1);
        }
        Some(d) => {
            tally.add(format!("{what}: {}", d.kind()), 1);
            if !matches!(d, Divergence::Refused { .. }) {
                tally.add(format!("{what}: a valid run, detected by content"), 1);
            }
        }
    }
}

/// Every crash fate the register program has, as a window plan: replica `a` takes the
/// fate, `b` writes the same value cleanly and `c` the other value.
fn register_window_plans() -> Vec<(String, Plan)> {
    baseline::one_epoch_options()
        .into_iter()
        .filter(|(_, fate)| fate.crashes())
        .map(|(v, fate)| {
            let plan = baseline::plan_of(
                "pr15-exit-crash-window",
                1,
                [
                    baseline::one_epoch_replica((v, fate)),
                    baseline::one_epoch_replica((v, Fate::Clean)),
                    baseline::one_epoch_replica((1 - v, Fate::Clean)),
                ],
            );
            (
                format!("a={}:{}", register::VALUES[usize::from(v)], fate.token()),
                plan,
            )
        })
        .collect()
}

fn register_windows(mode: CrashMode) -> Tally {
    let artifact = match mode {
        CrashMode::Graceful => "pr15-exit-pos-05",
        CrashMode::FailStop => "pr15-exit-pos-06",
    };
    let mut tally = Tally::default();
    for (pi, (name, plan)) in register_window_plans().into_iter().enumerate() {
        if let Err(v) = baseline::discipline(&plan, baseline::SCENARIO) {
            tally.gaps.push(Gap {
                artifact,
                window: name.clone(),
                why: GapWhy::NotRealized(format!("the plan breaks the protocol: {v:?}")),
            });
            continue;
        }
        tally.add("crash fates", 1);
        let built = register::build_with_shutdown_in(&plan, mode);
        let op = match crash_op(&built, mode) {
            Ok(op) => op,
            Err(why) => {
                tally.gaps.push(Gap {
                    artifact,
                    window: name.clone(),
                    why: GapWhy::NotRealized(why),
                });
                continue;
            }
        };
        let seed = REGISTER_SEED ^ u64::try_from(pi).expect("few");
        let (scope, logs) = baseline::logs_for(&built, REGISTER_LOGS, seed);
        if scope == Scope::Exhaustive {
            tally.add("plans run exhaustively", 1);
        }
        for (li, log) in logs.iter().enumerate() {
            let window = format!("{name} log {li}");
            tally.add("windows", 1);
            let recorded = match run_binding(&built.programs, log, &binding_config(0)) {
                Ok(j) => j,
                Err(r) => {
                    let why = match r.inconclusive_reason() {
                        Some(InconclusiveReason::Unsupported) => GapWhy::Unsupported(r.to_string()),
                        Some(other) => GapWhy::Inconclusive(format!("{other:?}: {r}")),
                        None => GapWhy::NotRealized(r.to_string()),
                    };
                    tally.gaps.push(Gap {
                        artifact,
                        window,
                        why,
                    });
                    continue;
                }
            };
            let bytes = recorded.encode().expect("encodes");
            tally.retain(&bytes);
            let (outcome, clean) = register_outcome(&built, plan.epochs, log);
            if !clean {
                tally.gaps.push(Gap {
                    artifact,
                    window: window.clone(),
                    why: GapWhy::Inconclusive("the run report has a finding".to_owned()),
                });
                continue;
            }
            // The recorded bytes are a journal: decode and re-encode, byte for byte.
            match BJournal::decode(&bytes) {
                Ok(j) if j.encode().expect("encodes") == bytes && j == recorded => {}
                other => {
                    tally.gaps.push(Gap {
                        artifact,
                        window: window.clone(),
                        why: GapWhy::Diverged(Divergence::Undecodable(format!(
                            "{:?}",
                            other.err()
                        ))),
                    });
                    continue;
                }
            }
            // Replay from the recorded plan and log: twice under the recorded seed, and
            // under every lab seed for the first log of each plan.
            let seeds: Vec<u64> = if li == 0 {
                LAB_SEEDS.to_vec()
            } else {
                vec![0, 0]
            };
            let mut diverged = None;
            for s in seeds {
                if let Some(d) = bdiverges(
                    &recorded,
                    &bytes,
                    run_binding(&built.programs, log, &binding_config(s)),
                ) {
                    diverged = Some(d);
                    break;
                }
                tally.add("replays, each byte-identical", 1);
            }
            if diverged.is_none() && register_outcome(&built, plan.epochs, log).0 != outcome {
                diverged = Some(Divergence::OutcomeDiffers);
            }
            if let Some(d) = diverged {
                tally.gaps.push(Gap {
                    artifact,
                    window,
                    why: GapWhy::Diverged(d),
                });
                continue;
            }
            tally.add("outcomes replayed identical", 1);
            // Perturbations of the program under the same log: the crash moved one
            // operation later and earlier in replica a's script, and dropped.
            let crash = built.programs[1][op].clone();
            let len = built.programs[1].len();
            if op + 1 < len {
                let moved = register::with_op(
                    &register::without_op(&built, 1, op),
                    1,
                    op + 1,
                    crash.clone(),
                );
                bexpect_divergence(
                    &mut tally,
                    "crash moved one step later",
                    &recorded,
                    &bytes,
                    &moved,
                    log,
                    &window,
                );
            } else {
                tally.add(
                    "absent: crash moved one step later, the last operation of the script",
                    1,
                );
            }
            if op > 0 {
                let moved = register::with_op(
                    &register::without_op(&built, 1, op),
                    1,
                    op - 1,
                    crash.clone(),
                );
                bexpect_divergence(
                    &mut tally,
                    "crash moved one step earlier",
                    &recorded,
                    &bytes,
                    &moved,
                    log,
                    &window,
                );
            } else {
                tally.add(
                    "absent: crash moved one step earlier, the first operation of the script",
                    1,
                );
            }
            let dropped = register::without_op(&built, 1, op);
            bexpect_divergence(
                &mut tally,
                "crash dropped",
                &recorded,
                &bytes,
                &dropped,
                log,
                &window,
            );
            // Dropped with the log kept aligned: the crash replaced by an operation that
            // touches no task, so the recorded log still schedules every operation.
            let blank = register::with_op(&dropped, 1, op, SubstrateOp::Advance { nanos: 1 });
            bexpect_divergence(
                &mut tally,
                "crash dropped, the log kept aligned",
                &recorded,
                &bytes,
                &blank,
                log,
                &window,
            );
            // The other crash semantics at the same point: a graceful cancellation
            // replayed as a fail-stop crash, or the reverse.
            let other = match crash {
                SubstrateOp::Cancel { region } => SubstrateOp::Crash { region },
                SubstrateOp::Crash { region } => SubstrateOp::Cancel { region },
                ref op => unreachable!("{op:?} is not a crash"),
            };
            let swapped = register::with_op(&dropped, 1, op, other);
            bexpect_divergence(
                &mut tally,
                "crash semantics swapped",
                &recorded,
                &bytes,
                &swapped,
                log,
                &window,
            );
        }
    }
    tally
}

// ---------------------------------------------------------------------------
// the retained register campaigns
// ---------------------------------------------------------------------------

/// A campaign line the PR-16 golden retains: its name, identity and run count, whether
/// it has no finding, and each other count the line retains, by name.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Retained {
    name: String,
    identity: String,
    runs: usize,
    findings_none: bool,
    counts: BTreeMap<&'static str, usize>,
}

/// The `name value` pairs of the segment of `line` that opens with `open` and ends at
/// `close`: `"fenced tasks 4652, obligations 4712"` gives `tasks 4652`, `obligations
/// 4712`.
fn pairs<'a>(line: &'a str, open: &str, close: char) -> Vec<(&'a str, usize)> {
    let Some((_, rest)) = line.split_once(open) else {
        return Vec::new();
    };
    rest.split(close)
        .next()
        .unwrap_or("")
        .split(", ")
        .filter_map(|part| {
            let (name, value) = part.trim().rsplit_once(' ')?;
            Some((name, value.parse().ok()?))
        })
        .collect()
}

fn parse_retained(line: &str) -> Option<Retained> {
    let rest = line.split_once("campaign ")?.1;
    let name = rest.split(' ').next()?;
    if !name.starts_with("pr16-correct-baseline") || !rest.contains(" seed ") {
        return None;
    }
    let field = |key: &str, stop: &[char]| -> Option<String> {
        let at = line.find(key)? + key.len();
        line[at..].split(stop).next().map(str::to_owned)
    };
    let runs = line
        .split(" runs;")
        .next()?
        .rsplit(' ')
        .next()?
        .parse()
        .ok()?;
    // The counts the line carries, each under the name `replayed_counts` gives it.
    let mut counts = BTreeMap::new();
    for (name, v) in pairs(line, "; fenced ", ';') {
        let key = match name {
            "tasks" => "fenced tasks",
            "obligations" => "fenced obligations",
            "reservations" => "fenced reservations",
            "timers" => "fenced timers",
            _ => continue,
        };
        counts.insert(key, v);
    }
    for (name, v) in pairs(line, "payloads sent ", '(') {
        let key = match name {
            "received" => "payloads received",
            "confirmed by the receiver" => "payloads confirmed",
            _ => continue,
        };
        counts.insert(key, v);
    }
    for (name, v) in pairs(line, "; timers ", ';') {
        let key = match name {
            "scheduled" => "timers scheduled",
            "fired" => "timers fired",
            "cancelled" => "timers cancelled",
            "fenced" => "timers fenced",
            _ => continue,
        };
        counts.insert(key, v);
    }
    Some(Retained {
        name: name.to_owned(),
        identity: field("identity ", &[';', ' '])?,
        runs,
        findings_none: field("findings ", &[';'])?.trim_end() == "none",
        counts,
    })
}

/// Every correct-baseline campaign the PR-16 golden retains, in its order.
fn retained_campaigns() -> Vec<Retained> {
    let mut out: Vec<Retained> = IMPL05_GOLDEN.lines().filter_map(parse_retained).collect();
    out.dedup_by(|a, b| a.name == b.name);
    out
}

/// What a retained campaign name means: its crash mode and carrier, or why the name is
/// not one this layer can run (a typed gap, never a silent skip).
fn campaign_kind(name: &str) -> Result<(CrashMode, Option<register::Carrier>), String> {
    let rest = name
        .strip_prefix("pr16-correct-baseline")
        .ok_or_else(|| format!("{name} is not a correct-baseline campaign"))?;
    let (rest, mode) = match rest.strip_suffix("-fail-stop") {
        Some(r) => (r, CrashMode::FailStop),
        None => (rest, CrashMode::Graceful),
    };
    let carrier = match rest {
        "" => None,
        "-carried-message" => Some(register::Carrier::Message),
        "-carried-timer" => Some(register::Carrier::Timer),
        "-carried-recovery" => Some(register::Carrier::Recovery),
        other => return Err(format!("{name}: unknown campaign suffix {other}")),
    };
    Ok((mode, carrier))
}

/// Execute a retained campaign again from its plans; the outcome and the campaign run.
fn execute_campaign(
    name: &str,
    mode: CrashMode,
    carrier: Option<register::Carrier>,
) -> (baseline::Outcome, baseline::Campaign) {
    let base = baseline::baseline();
    match carrier {
        // The fail-stop baseline keeps the base campaign's name in its identity.
        None => (baseline::execute_in(&base, mode), base),
        Some(c) => {
            let campaign = base.mutated(Box::leak(name.to_owned().into_boxed_str()), Plan::clone);
            let o = baseline::execute_with(&campaign, move |p| {
                Ok(register::build_carried(p, mode, c, register::CORRECT_FENCE))
            });
            (o, campaign)
        }
    }
}

/// The replayed value of each count a retained line may carry.
fn replayed_counts(o: &baseline::Outcome) -> BTreeMap<&'static str, usize> {
    let t = &o.tally;
    BTreeMap::from([
        ("fenced tasks", t.fenced[0]),
        ("fenced obligations", t.fenced[1]),
        ("fenced reservations", t.fenced[2]),
        ("fenced timers", t.fenced[3]),
        ("payloads received", o.carried[0]),
        ("payloads confirmed", o.carried[1]),
        ("timers scheduled", t.timers[0]),
        ("timers fired", t.timers[1]),
        ("timers cancelled", t.timers[2]),
        ("timers fenced", t.fenced[3]),
    ])
}

// ---------------------------------------------------------------------------
// the declared unsupported cases, as probes
// ---------------------------------------------------------------------------

/// A requestable declared unsupported case: the step that asks for it, when it asks,
/// and the refusal the profile declares for it.
struct Probe<P: Pack> {
    token: &'static str,
    step: P::Step,
    applies: fn(&P::Lab) -> bool,
    refusal: String,
}

fn always<L>(_: &L) -> bool {
    true
}

fn net_request(kind: net::StepKind) -> net::Step {
    use net::{EnvelopeId, NodeId, NodeSet, Payload, Step as S, StepKind as K};
    match kind {
        K::Send => S::Send {
            src: NodeId(0),
            dst: NodeId(1),
            payload: Payload(vec![0]),
        },
        K::Deliver => S::Deliver(EnvelopeId(0)),
        K::Drop => S::Drop(EnvelopeId(0)),
        K::Duplicate => S::Duplicate(EnvelopeId(0)),
        K::Delay => S::Delay(EnvelopeId(0)),
        K::Partition => S::Partition(NodeSet(0b010)),
        K::Heal => S::Heal,
        K::OneWayPartition => S::OneWayPartition {
            from: NodeSet(0b001),
            to: NodeSet(0b010),
        },
        K::Corrupt => S::Corrupt(EnvelopeId(0)),
        K::Forge => S::Forge {
            src: NodeId(0),
            dst: NodeId(1),
            payload: Payload(vec![0]),
        },
        K::ConnectionReset => S::ConnectionReset(NodeId(0), NodeId(1)),
        K::CrashEndpoint => S::CrashEndpoint(NodeId(0)),
        K::Recall => S::Recall(EnvelopeId(0)),
    }
}

fn prc_request(kind: prc::StepKind) -> prc::Step {
    use prc::{NodeId, NodeSet, Step as S, StepKind as K, TicketId};
    match kind {
        K::Begin => S::Begin(NodeId(0)),
        K::Complete => S::Complete(TicketId(0)),
        K::Delay => S::Delay(TicketId(0)),
        K::Crash => S::Crash(NodeId(0)),
        K::Restart => S::Restart(NodeId(0)),
        K::CancelGracefully => S::CancelGracefully(NodeId(0)),
        K::Panic => S::Panic(NodeId(0)),
        K::PowerLoss => S::PowerLoss(NodeSet(0b001)),
    }
}

fn sto_request(kind: sto::StepKind) -> sto::Step {
    use sto::{NodeId, Step as S, StepKind as K, TicketId, Value};
    match kind {
        K::Submit => S::Submit {
            node: NodeId(0),
            value: Value(0),
        },
        K::Sync => S::Sync(NodeId(0)),
        K::Persist => S::Persist(TicketId(0)),
        K::Ack => S::Ack(TicketId(0)),
        K::Delay => S::Delay(TicketId(0)),
        K::Crash => S::Crash {
            node: NodeId(0),
            keep: 0,
            torn: false,
        },
        K::Restart => S::Restart(NodeId(0)),
        K::Truncate => S::Truncate(NodeId(0)),
        K::Corrupt => S::Corrupt {
            node: NodeId(0),
            index: 0,
        },
        K::Reorder => S::Reorder(NodeId(0)),
        K::FalseFlush => S::FalseFlush(TicketId(0)),
    }
}

fn partition_active_with_budget(lab: &net::Network) -> bool {
    lab.partition().is_some() && lab.partitions_used() < lab.config().max_partitions()
}

fn net_probes() -> (Vec<Probe<Net>>, Vec<&'static str>) {
    let mut probes = Vec::new();
    let mut none = Vec::new();
    for case in net::UNSUPPORTED {
        let refusal = case.refusal().map(|r| format!("{r:?}"));
        match (case.request, refusal) {
            (net::Request::Step(kind), Some(refusal)) => probes.push(Probe {
                token: case.semantic.token(),
                step: net_request(kind),
                applies: always,
                refusal,
            }),
            (net::Request::StepWhen { step, when }, Some(refusal)) => probes.push(Probe {
                token: case.semantic.token(),
                step: net_request(step),
                applies: match when {
                    net::When::PartitionActiveWithBudget => partition_active_with_budget,
                },
                refusal,
            }),
            (net::Request::NoOperation, None) => none.push(case.semantic.token()),
            (request, refusal) => panic!("{request:?} declares refusal {refusal:?}"),
        }
    }
    (probes, none)
}

fn prc_probes() -> (Vec<Probe<Proc>>, Vec<&'static str>) {
    let mut probes = Vec::new();
    let mut none = Vec::new();
    for case in prc::UNSUPPORTED {
        match (case.request, case.refusal()) {
            (prc::Request::Step(kind), Some(refusal)) => probes.push(Probe {
                token: case.semantic.token(),
                step: prc_request(kind),
                applies: always,
                refusal: format!("{refusal:?}"),
            }),
            (prc::Request::NoOperation, None) => none.push(case.semantic.token()),
            (request, refusal) => panic!("{request:?} declares refusal {refusal:?}"),
        }
    }
    (probes, none)
}

fn sto_probes() -> (Vec<Probe<Stor>>, Vec<&'static str>) {
    let mut probes = Vec::new();
    let mut none = Vec::new();
    for case in sto::UNSUPPORTED {
        match (case.request, case.refusal()) {
            (sto::Request::Step(kind), Some(refusal)) => probes.push(Probe {
                token: case.semantic.token(),
                step: sto_request(kind),
                applies: always,
                refusal: format!("{refusal:?}"),
            }),
            (sto::Request::NoOperation, None) => none.push(case.semantic.token()),
            (request, refusal) => panic!("{request:?} declares refusal {refusal:?}"),
        }
    }
    (probes, none)
}

/// Splice every probe into every boundary of the seeded logs where it applies: the
/// harness must answer the pack's typed `Unsupported` refusal as a gap, never a record,
/// and the pack's own `run` must refuse the spliced log at the splice.
fn probe_campaign<P: Pack>(probes: &[Probe<P>], generator: P::Config, config: P::Config) -> Tally {
    let mut tally = Tally::default();
    for probe in probes {
        tally.add(format!("requestable {}", probe.token), 0);
    }
    for seed in SEEDS {
        let mut rng = SplitMix(seed ^ 0x0b);
        let log = generate::<P>(generator, &mut rng, LOG_LEN);
        let mut state = P::lab(config);
        for at in 0..=log.len() {
            for probe in probes {
                if !(probe.applies)(&state) {
                    continue;
                }
                let window = format!("seed {seed:#x}@{at} {}", probe.token);
                let splice = [probe.step.clone()];
                match realize::<P>(
                    P::ID,
                    window.clone(),
                    config,
                    &log[..at],
                    &splice,
                    &log[at..],
                ) {
                    Ok(_) => tally
                        .undetected
                        .push(format!("{}: {window} was recorded", P::ID)),
                    Err(Gap {
                        why: GapWhy::Unsupported(detail),
                        ..
                    }) if detail == probe.refusal => {
                        tally.add(format!("requestable {}", probe.token), 1);
                    }
                    Err(gap) => tally
                        .undetected
                        .push(format!("{}: {window}: {gap:?}", P::ID)),
                }
                let spliced: Vec<P::Step> = log[..at]
                    .iter()
                    .chain(&splice)
                    .chain(&log[at..])
                    .cloned()
                    .collect();
                match P::own_run(config, &spliced) {
                    Err((index, refusal)) if index == at && refusal == probe.refusal => {
                        tally.add("the pack's own run refuses at the splice", 1);
                    }
                    other => tally.undetected.push(format!(
                        "{}: {window}: own run {:?}",
                        P::ID,
                        other.map(|b| b.len())
                    )),
                }
            }
            if at < log.len() {
                P::apply(&mut state, &log[at]).expect("the generated log is valid");
            }
        }
    }
    tally
}

// ---------------------------------------------------------------------------
// the evidence, computed once
// ---------------------------------------------------------------------------

struct CampaignReplay {
    retained: Retained,
    /// Why the campaign could not be run, when it could not: a gap.
    unrunnable: Option<String>,
    identity: String,
    runs: usize,
    findings: usize,
    /// Each count the retained line carries: `(name, replayed, retained)`.
    compared: Vec<(&'static str, usize, usize)>,
    crash_plans: usize,
}

struct Evidence {
    network: Tally,
    process: Tally,
    storage: Tally,
    composed: Tally,
    graceful: Tally,
    fail_stop: Tally,
    campaigns: Vec<CampaignReplay>,
    probes: [(Tally, Vec<&'static str>); 3],
}

fn evidence() -> &'static Evidence {
    static CELL: OnceLock<Evidence> = OnceLock::new();
    CELL.get_or_init(|| {
        std::thread::scope(|s| {
            let campaigns: Vec<_> = retained_campaigns()
                .into_iter()
                .map(|retained| {
                    s.spawn(move || {
                        let (mode, carrier) = match campaign_kind(&retained.name) {
                            Ok(kind) => kind,
                            Err(why) => {
                                return CampaignReplay {
                                    retained,
                                    unrunnable: Some(why),
                                    identity: String::new(),
                                    runs: 0,
                                    findings: 0,
                                    compared: Vec::new(),
                                    crash_plans: 0,
                                };
                            }
                        };
                        let (o, campaign) = execute_campaign(&retained.name, mode, carrier);
                        let crash_plans = campaign
                            .groups
                            .iter()
                            .flat_map(|g| &g.plans)
                            .filter(|p| {
                                p.replicas.iter().flatten().any(|a| {
                                    matches!(
                                        a,
                                        register::Act::Crash | register::Act::CrashRepropose(..)
                                    )
                                })
                            })
                            .count();
                        let replayed = replayed_counts(&o);
                        let compared = retained
                            .counts
                            .iter()
                            .map(|(k, v)| (*k, replayed[k], *v))
                            .collect();
                        CampaignReplay {
                            retained,
                            unrunnable: None,
                            identity: o.digest.clone(),
                            runs: o.runs,
                            findings: o.by_property.values().sum(),
                            compared,
                            crash_plans,
                        }
                    })
                })
                .collect();
            let graceful = s.spawn(|| register_windows(CrashMode::Graceful));
            let fail_stop = s.spawn(|| register_windows(CrashMode::FailStop));
            let network = s.spawn(|| {
                pack_campaign::<Net>(network_declared(), network_config(1), network_config(1))
                    .absorb(pack_campaign::<Net>(
                        network_declared(),
                        network_tight(),
                        network_tight(),
                    ))
            });
            let process = s.spawn(|| {
                pack_campaign::<Proc>(process_declared(), process_config(1), process_config(3))
                    .absorb(pack_campaign::<Proc>(
                        process_declared(),
                        process_tight(),
                        process_tight(),
                    ))
            });
            let storage = s.spawn(|| {
                pack_campaign::<Stor>(storage_declared(), storage_config(1), storage_config(3))
                    .absorb(pack_campaign::<Stor>(
                        storage_declared(),
                        storage_tight(),
                        storage_tight(),
                    ))
            });
            let composed = s.spawn(composed_campaign);
            let probes = s.spawn(|| {
                let (np, nn) = net_probes();
                let (pp, pn) = prc_probes();
                let (sp, sn) = sto_probes();
                [
                    (
                        probe_campaign::<Net>(&np, network_config(2), network_config(2)),
                        nn,
                    ),
                    (
                        probe_campaign::<Proc>(&pp, process_config(1), process_config(3)),
                        pn,
                    ),
                    (
                        probe_campaign::<Stor>(&sp, storage_config(1), storage_config(3)),
                        sn,
                    ),
                ]
            });
            Evidence {
                network: network.join().expect("network"),
                process: process.join().expect("process"),
                storage: storage.join().expect("storage"),
                composed: composed.join().expect("composed"),
                graceful: graceful.join().expect("graceful"),
                fail_stop: fail_stop.join().expect("fail-stop"),
                campaigns: campaigns
                    .into_iter()
                    .map(|h| h.join().expect("a campaign"))
                    .collect(),
                probes: probes.join().expect("probes"),
            }
        })
    })
}

/// The replayed windows of a tally: every window kind it counts.
fn windows(t: &Tally) -> u64 {
    t.get("crash windows")
        + t.get("crash with recovery windows")
        + t.get("cancellation points")
        + t.get("windows")
}

/// Detections of the perturbation `what`: every typed-divergence count recorded for it.
fn detected(t: &Tally, what: &str) -> u64 {
    let prefix = format!("{what}: ");
    t.counts
        .iter()
        .filter(|(k, _)| {
            k.starts_with(&prefix) && !k.contains("same length") && !k.contains("valid run")
        })
        .map(|(_, v)| *v)
        .sum()
}

/// Windows where `what` was verified to change nothing.
fn equivalent(t: &Tally, what: &str) -> u64 {
    t.get(&format!("equivalent, verified: {what}"))
}

/// Windows where `what` has no place to go.
fn absent(t: &Tally, what: &str) -> u64 {
    let prefix = format!("absent: {what}");
    t.counts
        .iter()
        .filter(|(k, _)| k.starts_with(&prefix))
        .map(|(_, v)| *v)
        .sum()
}

/// Detections a length-only checker would have missed.
fn by_content(t: &Tally) -> u64 {
    t.counts
        .iter()
        .filter(|(k, _)| k.contains("same length") || k.contains("valid run"))
        .map(|(_, v)| *v)
        .sum()
}

// ---------------------------------------------------------------------------
// the predicates: every claim the evidence makes, each with its own result
// ---------------------------------------------------------------------------

/// One claimed check and its result.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Pred {
    artifact: &'static str,
    id: String,
    pass: bool,
    detail: String,
}

struct Preds(Vec<Pred>);

impl Preds {
    fn add(
        &mut self,
        artifact: &'static str,
        id: impl Into<String>,
        pass: bool,
        detail: impl Into<String>,
    ) {
        self.0.push(Pred {
            artifact,
            id: id.into(),
            pass,
            detail: detail.into(),
        });
    }
}

/// The per-pack artifact of a tally.
const TALLIES: [(&str, &str); 6] = [
    ("pr15-exit-pos-01", "network"),
    ("pr15-exit-pos-02", "process"),
    ("pr15-exit-pos-03", "storage"),
    ("pr15-exit-pos-04", "composed"),
    ("pr15-exit-pos-05", "graceful"),
    ("pr15-exit-pos-06", "fail-stop"),
];

fn tally_of<'a>(e: &'a Evidence, name: &str) -> &'a Tally {
    match name {
        "network" => &e.network,
        "process" => &e.process,
        "storage" => &e.storage,
        "composed" => &e.composed,
        "graceful" => &e.graceful,
        _ => &e.fail_stop,
    }
}

/// The crash windows a tally recorded.
fn crash_windows(t: &Tally) -> u64 {
    t.get("crash windows") + t.get("crash with recovery windows") + t.get("windows")
}

/// The byte-flip and load-bearing-comparison checks of one pack (`neg-03`).
fn doctor<P: Pack>(
    config: P::Config,
    generator: P::Config,
    other_budget: P::Config,
) -> Vec<(String, bool, String)> {
    let log = generate::<P>(generator, &mut SplitMix(0xd0c), LOG_LEN);
    let fresh =
        || realize::<P>(P::ID, "doctored".to_owned(), config, &log, &[], &[]).expect("a valid log");
    let record = fresh();
    let mut out = vec![(
        format!("{}: the undoctored record replays exactly", P::ID),
        exact_replay(&record).is_ok(),
        String::new(),
    )];
    let budget = diverges(&record, replay_steps::<P>(other_budget, &record.log));
    out.push((
        format!(
            "{}: the same log under another retained-bytes budget differs in bytes alone",
            P::ID
        ),
        budget == Some(Divergence::BytesDiffer),
        format!("{budget:?}"),
    ));
    let mut outcome = fresh();
    outcome.outcome.push('!');
    let got = exact_replay(&outcome).err();
    out.push((
        format!("{}: a doctored outcome differs in outcome alone", P::ID),
        got == Some(Divergence::OutcomeDiffers),
        format!("{got:?}"),
    ));
    let mut kinds: BTreeMap<&'static str, usize> = BTreeMap::new();
    let mut accepted = Vec::new();
    for i in 0..record.bytes.len() {
        let mut doctored = fresh();
        doctored.bytes[i] ^= 0x01;
        match exact_replay(&doctored) {
            Ok(_) => accepted.push(i),
            Err(d) => *kinds.entry(d.kind()).or_default() += 1,
        }
    }
    out.push((
        format!(
            "{}: every single-bit flip of the recorded journal is a typed divergence",
            P::ID
        ),
        accepted.is_empty()
            && kinds.contains_key("undecodable")
            && kinds.keys().any(|k| *k != "undecodable"),
        format!(
            "{} bytes; kinds {kinds:?}; accepted {accepted:?}",
            record.bytes.len()
        ),
    ));
    out
}

fn compute_predicates() -> Vec<Pred> {
    let e = evidence();
    let mut p = Preds(Vec::new());

    // --- hon-01
    let h = "pr15-exit-hon-01";
    p.add(
        h,
        "network declares cancellation points and no crash window",
        network_declared()
            == Declared {
                crash_windows: false,
                cancellation_points: true,
                cancellation_routed_to_runtime: false,
            },
        format!("{:?}", network_declared()),
    );
    p.add(
        h,
        "process declares crash windows and routes cancellation to the runtime",
        process_declared()
            == Declared {
                crash_windows: true,
                cancellation_points: false,
                cancellation_routed_to_runtime: true,
            },
        format!("{:?}", process_declared()),
    );
    p.add(
        h,
        "storage declares crash windows and cancellation points",
        storage_declared()
            == Declared {
                crash_windows: true,
                cancellation_points: true,
                cancellation_routed_to_runtime: false,
            },
        format!("{:?}", storage_declared()),
    );
    p.add(
        h,
        "network endpoint crash is owned by the process profile",
        net::Semantic::EndpointCrash
            .unsupported_case()
            .map(|c| c.reliance)
            == Some(net::Reliance::Owner("process/crash-restart-v0")),
        "",
    );
    let mut rows = true;
    for (profiles, contract) in [
        (
            net::PROFILES.map(|p| p.canonical_bytes()),
            &net::profile::CANCELLATION_CONTRACT,
        ),
        (
            prc::PROFILES.map(|p| p.canonical_bytes()),
            &prc::profile::CANCELLATION_CONTRACT,
        ),
        (
            sto::PROFILES.map(|p| p.canonical_bytes()),
            &sto::profile::CANCELLATION_CONTRACT,
        ),
    ] {
        for bytes in profiles {
            for id in ["replay-choice", "early-return"] {
                rows &= contains(&bytes, row(contract, id).as_bytes());
            }
        }
    }
    p.add(
        h,
        "-v0 and -v1 of each pack carry the replay-choice and early-return rows",
        rows,
        "",
    );
    p.add(
        h,
        "-v0 declares no unsupported case and -v1 declares them, in each pack",
        net::PROFILES[0].unsupported.is_none()
            && net::PROFILES[1].unsupported.is_some()
            && prc::PROFILES[0].unsupported.is_none()
            && prc::PROFILES[1].unsupported.is_some()
            && sto::PROFILES[0].unsupported.is_none()
            && sto::PROFILES[1].unsupported.is_some(),
        "",
    );

    // --- pos-01..06: every window replayed, no gap, no accepted perturbation
    for (artifact, name) in TALLIES {
        let t = tally_of(e, name);
        let verdict = exit_verdict(artifact, windows(t), &t.gaps);
        p.add(
            artifact,
            "every window realized and replayed exactly, and there is one",
            matches!(verdict, ExitVerdict::Met { windows } if windows > 0),
            match verdict {
                ExitVerdict::Met { windows } => format!("{windows} windows"),
                ExitVerdict::NotMet(g) => format!("{} gaps: {:?}", g.len(), g.first()),
            },
        );
        p.add(
            artifact,
            "no perturbation was accepted as the record",
            t.undetected.is_empty(),
            format!("{:?}", t.undetected.first()),
        );
    }
    let n = &e.network;
    p.add(
        "pr15-exit-pos-01",
        "no crash window: the pack has no crash",
        n.get("crash windows") == 0,
        "",
    );
    p.add(
        "pr15-exit-pos-01",
        "more than 100 cancellation points",
        n.get("cancellation points") > 100,
        n.get("cancellation points").to_string(),
    );
    p.add(
        "pr15-exit-pos-01",
        "late completions after a cancellation point",
        n.get("events after a cancellation point (late completions)") > 0,
        "",
    );
    for (artifact, t) in [
        ("pr15-exit-pos-02", &e.process),
        ("pr15-exit-pos-03", &e.storage),
    ] {
        p.add(
            artifact,
            "more than 100 crash windows alone and with recovery",
            t.get("crash windows") > 100 && t.get("crash with recovery windows") > 100,
            crash_windows(t).to_string(),
        );
        p.add(
            artifact,
            "late completions of a crashed incarnation fenced",
            t.get("fenced completions after the crash") > 0,
            t.get("fenced completions after the crash").to_string(),
        );
    }
    p.add(
        "pr15-exit-pos-02",
        "no pack cancellation point: routed to the runtime",
        e.process.get("cancellation points") == 0
            && process_declared().cancellation_routed_to_runtime,
        "",
    );
    p.add(
        "pr15-exit-pos-03",
        "more than 100 cancellation points",
        e.storage.get("cancellation points") > 100,
        e.storage.get("cancellation points").to_string(),
    );
    let c = &e.composed;
    let composed = c.get("crash windows") + c.get("crash with recovery windows");
    p.add(
        "pr15-exit-pos-04",
        "more than 100 composed crash windows",
        composed > 100,
        composed.to_string(),
    );
    p.add(
        "pr15-exit-pos-04",
        "the network journal is unchanged by every crash",
        c.get("network journal unchanged by the crash") == composed,
        "",
    );
    p.add(
        "pr15-exit-pos-04",
        "the two packs agree on every node at every boundary",
        c.get("boundaries where the two packs agree on every node") == c.get("boundaries")
            && c.get("boundaries") > 0,
        "",
    );
    p.add(
        "pr15-exit-pos-04",
        "every move across network steps verified to commute",
        c.get("crash moved across network steps: commutes, verified") > 0,
        "",
    );
    for (artifact, t) in [
        ("pr15-exit-pos-05", &e.graceful),
        ("pr15-exit-pos-06", &e.fail_stop),
    ] {
        p.add(
            artifact,
            "every crash fate, every value and retry: 12",
            t.get("crash fates") == 12,
            t.get("crash fates").to_string(),
        );
        p.add(
            artifact,
            "every window's run report replayed identical",
            t.get("outcomes replayed identical") == t.get("windows") && t.get("windows") > 0,
            "",
        );
    }

    // --- pos-07: every retained correct-baseline campaign
    let a = "pr15-exit-pos-07";
    p.add(
        a,
        "every retained correct-baseline campaign is run: 8",
        e.campaigns.len() == 8,
        e.campaigns.len().to_string(),
    );
    for c in &e.campaigns {
        let name = &c.retained.name;
        let carried = name.contains("carried");
        let fenced = c
            .compared
            .iter()
            .any(|(k, v, _)| *k == "fenced tasks" && *v > 0);
        p.add(a, format!("{name}: replays to its retained identity, runs and counts with no finding"),
            c.unrunnable.is_none()
                && c.retained.findings_none
                && c.identity == c.retained.identity
                && c.runs == c.retained.runs
                && c.findings == 0
                && c.compared.len() == c.retained.counts.len()
                && c.compared.iter().all(|(_, x, y)| x == y)
                && (!carried || c.compared.len() >= 6)
                && (name != "pr16-correct-baseline-fail-stop" || fenced)
                && c.crash_plans > 0,
            format!("identity {} (retained {}); runs {}; findings {}; compared {:?}; crash plans {}; {:?}",
                c.identity, c.retained.identity, c.runs, c.findings, c.compared, c.crash_plans, c.unrunnable));
    }

    // --- neg-01: every crash window gets a disposition for each perturbation
    for (artifact, name) in [
        ("pr15-exit-neg-01", "process"),
        ("pr15-exit-neg-01", "storage"),
        ("pr15-exit-neg-01", "composed"),
        ("pr15-exit-neg-01", "graceful"),
        ("pr15-exit-neg-01", "fail-stop"),
    ] {
        let t = tally_of(e, name);
        let windows = crash_windows(t);
        for what in [
            "crash moved one step later",
            "crash moved one step earlier",
            "crash dropped",
        ] {
            let (d, q, s) = (detected(t, what), equivalent(t, what), absent(t, what));
            p.add(artifact, format!("{name}: {what}: detected + verified-equivalent + absent = windows, some detected"),
                d + q + s == windows && d > 0, format!("{d} + {q} + {s} of {windows}"));
        }
        p.add(
            artifact,
            format!("{name}: some detections are by content at equal length"),
            by_content(t) > 0,
            by_content(t).to_string(),
        );
    }
    for (name, what) in [
        ("graceful", "crash dropped, the log kept aligned"),
        ("graceful", "crash semantics swapped"),
        ("fail-stop", "crash dropped, the log kept aligned"),
        ("fail-stop", "crash semantics swapped"),
    ] {
        let t = tally_of(e, name);
        p.add(
            "pr15-exit-neg-01",
            format!("{name}: {what}: detected in every window"),
            detected(t, what) == t.get("windows") && t.get("windows") > 0,
            detected(t, what).to_string(),
        );
    }

    // --- neg-02: every cancellation point gets a disposition for each perturbation
    for name in ["network", "storage"] {
        let t = tally_of(e, name);
        let points = t.get("cancellation points");
        for (what, may_be_equivalent, may_be_absent) in [
            ("cancellation dropped", true, false),
            ("cancellation moved past the next program step", false, true),
            ("cancellation moved one step later", true, true),
        ] {
            let (d, q, s) = (detected(t, what), equivalent(t, what), absent(t, what));
            p.add("pr15-exit-neg-02", format!("{name}: {what}: detected + verified-equivalent + absent = points, some detected"),
                d + q + s == points && d > 0 && (may_be_equivalent || q == 0) && (may_be_absent || s == 0),
                format!("{d} + {q} + {s} of {points}"));
        }
    }

    // --- neg-03
    let half = RETAINED / 2;
    let net_half = net::NetworkConfig::new(3, 16, 8, network_config(1).faults(), 1, half)
        .expect("a configuration");
    let prc_half = prc::ProcessConfig::new(3, 3, true, 8, half).expect("a configuration");
    let sto_half =
        sto::StorageConfig::new(2, 3, true, true, true, 8, half).expect("a configuration");
    for (id, pass, detail) in doctor::<Net>(network_config(1), network_config(1), net_half)
        .into_iter()
        .chain(doctor::<Proc>(
            process_config(3),
            process_config(1),
            prc_half,
        ))
        .chain(doctor::<Stor>(
            storage_config(3),
            storage_config(1),
            sto_half,
        ))
    {
        p.add("pr15-exit-neg-03", id, pass, detail);
    }
    let plan = &register_window_plans()[0].1;
    let built = register::build_with_shutdown_in(plan, CrashMode::FailStop);
    let log = &baseline::logs_for(&built, 1, REGISTER_SEED).1[0];
    let recorded = run_binding(&built.programs, log, &binding_config(0)).expect("runs");
    let bytes = recorded.encode().expect("encodes");
    let (mut rejected, mut decoded_as_record) = (0, 0);
    for i in (0..bytes.len()).step_by(7) {
        let mut doctored = bytes.clone();
        doctored[i] ^= 0x01;
        match BJournal::decode(&doctored) {
            Ok(j) if j == recorded => decoded_as_record += 1,
            Ok(_) => {}
            Err(_) => rejected += 1,
        }
    }
    p.add(
        "pr15-exit-neg-03",
        "binding: no flipped byte decodes to the record",
        decoded_as_record == 0 && rejected > 0,
        format!("{rejected} rejected, {decoded_as_record} decoded as the record"),
    );

    // --- bnd-01
    let b = "pr15-exit-bnd-01";
    let totals = [
        net::UNSUPPORTED.len(),
        prc::UNSUPPORTED.len(),
        sto::UNSUPPORTED.len(),
    ];
    for (((tally, none), total), pack) in e
        .probes
        .iter()
        .zip(totals)
        .zip(["network", "process", "storage"])
    {
        let requestable: Vec<(&String, &u64)> = tally
            .counts
            .iter()
            .filter(|(k, _)| k.starts_with("requestable "))
            .collect();
        p.add(b, format!("{pack}: every requestable case is refused Unsupported in every window where it applies, never recorded"),
            tally.undetected.is_empty() && !requestable.is_empty() && requestable.iter().all(|(_, v)| **v > 0),
            format!("{requestable:?}; {:?}", tally.undetected.first()));
        p.add(
            b,
            format!("{pack}: requestable and no-operation cases partition the declared table"),
            requestable.len() + none.len() == total && !none.is_empty(),
            format!("{} + {} of {total}", requestable.len(), none.len()),
        );
        p.add(
            b,
            format!("{pack}: the pack's own run refuses the spliced log at the splice"),
            tally.get("the pack's own run refuses at the splice") > 0,
            "",
        );
    }
    let (_, nn) = net_probes();
    let (_, pn) = prc_probes();
    let (_, sn) = sto_probes();
    let no_step = nn.iter().all(|t| {
        net::StepKind::ALL.iter().all(|k| {
            net_request(*k)
                .unsupported_semantic()
                .map(net::Semantic::token)
                != Some(t)
        })
    }) && pn.iter().all(|t| {
        prc::StepKind::ALL.iter().all(|k| {
            prc_request(*k)
                .unsupported_semantic()
                .map(prc::Semantic::token)
                != Some(t)
        })
    }) && sn.iter().all(|t| {
        sto::StepKind::ALL.iter().all(|k| {
            sto_request(*k)
                .unsupported_semantic()
                .map(sto::Semantic::token)
                != Some(t)
        })
    });
    p.add(b, "no step kind asks for a no-operation case", no_step, "");
    p.add(
        b,
        "the request constructors build each step kind as itself",
        net::StepKind::ALL
            .iter()
            .all(|k| net_request(*k).kind() == *k)
            && prc::StepKind::ALL
                .iter()
                .all(|k| prc_request(*k).kind() == *k)
            && sto::StepKind::ALL
                .iter()
                .all(|k| sto_request(*k).kind() == *k),
        "",
    );
    let ops = vec![
        SubstrateOp::OpenRegion {
            parent: RegionLabel::ROOT,
            child: RegionLabel(1),
        },
        SubstrateOp::SpawnWithDeadline {
            region: RegionLabel(1),
            task: TaskLabel(1),
            resumability: Resumability::Resumable,
            deadline: 100,
        },
        SubstrateOp::Begin { task: TaskLabel(1) },
        SubstrateOp::Crash {
            region: RegionLabel(1),
        },
    ];
    let clog = ChoiceLog::new(vec![0; ops.len()]);
    let refusal = run_binding(&[ops], &clog, &binding_config(0));
    p.add(b, "binding: a crash without fail-stop semantics is CrashUnsupported, INV-008 Unsupported",
        matches!(&refusal, Err(r @ BindingRefusal::CrashUnsupported { .. }) if r.inconclusive_reason() == Some(InconclusiveReason::Unsupported)),
        format!("{:?}", refusal.as_ref().err()));

    // --- bnd-02
    for (name, t) in [("process", &e.process), ("storage", &e.storage)] {
        for reason in ["NodeDown", "CrashBudgetSpent"] {
            let n = t.get(&format!("crash not enabled: {reason}"));
            p.add(
                "pr15-exit-bnd-02",
                format!("{name}: a crash is refused {reason}"),
                n > 0,
                n.to_string(),
            );
        }
        p.add(
            "pr15-exit-bnd-02",
            format!("{name}: no crash candidate is inconclusive or unsupported"),
            t.gaps
                .iter()
                .all(|g| !matches!(g.why, GapWhy::Inconclusive(_) | GapWhy::Unsupported(_))),
            "",
        );
    }

    // --- bnd-03
    let f = "pr15-exit-bnd-03";
    p.add(
        f,
        "a clean artifact with windows is met",
        exit_verdict("x", 3, &[]) == ExitVerdict::Met { windows: 3 },
        "",
    );
    p.add(f, "an artifact with no window is not met", matches!(exit_verdict("x", 0, &[]), ExitVerdict::NotMet(g) if g.len() == 1 && g[0].why == GapWhy::NoWindows), "");
    let plog = generate::<Proc>(process_config(1), &mut SplitMix(5), LOG_LEN);
    let gap = realize::<Proc>(
        "bnd",
        "cancel".to_owned(),
        process_config(3),
        &plog[..3],
        &[prc::Step::CancelGracefully(prc::NodeId(0))],
        &plog[3..],
    )
    .err();
    p.add(f, "an unsupported window is a typed gap and not met",
        matches!(&gap, Some(g) if matches!(g.why, GapWhy::Unsupported(_)) && matches!(exit_verdict("bnd", 1, std::slice::from_ref(g)), ExitVerdict::NotMet(_))),
        format!("{gap:?}"));
    let slog = generate::<Stor>(storage_config(1), &mut SplitMix(9), 8);
    let bounded = storage_config(3)
        .with_max_steps(u32::try_from(slog.len()).expect("short"))
        .expect("a step bound");
    let mut bt = Tally::default();
    windows_over_log::<Stor>(&mut bt, storage_declared(), bounded, &slog, "bounded");
    p.add(
        f,
        "a window a step bound cuts is a ResourceExhausted gap and not met",
        bt.gaps.iter().any(
            |g| matches!(&g.why, GapWhy::Inconclusive(r) if r.starts_with("ResourceExhausted")),
        ) && matches!(
            exit_verdict("bnd", windows(&bt), &bt.gaps),
            ExitVerdict::NotMet(_)
        ),
        "",
    );
    let mut record =
        realize::<Proc>("bnd", "w".to_owned(), process_config(3), &plog, &[], &[]).expect("valid");
    let last = record.bytes.len() - 1;
    record.bytes[last] ^= 0x01;
    p.add(
        f,
        "a record doctored after recording does not replay",
        exact_replay(&record).is_err(),
        "",
    );
    let gaps: usize = TALLIES
        .iter()
        .map(|(_, name)| tally_of(e, name).gaps.len())
        .sum();
    p.add(
        f,
        "the corpus has no gap of any kind",
        gaps == 0,
        gaps.to_string(),
    );

    // --- bnd-04
    let t4 = "pr15-exit-bnd-04";
    p.add(
        t4,
        "time/unmodelled-v0 declares no operation",
        tim::UNMODELLED_V0.operations == tim::profile::Operations::None,
        "",
    );
    p.add(
        t4,
        "every time case is NoOperation and OutsidePack, one per row",
        tim::UNSUPPORTED.len() == tim::Semantic::ALL.len()
            && tim::UNSUPPORTED.iter().all(|c| {
                c.request == tim::Request::NoOperation && c.reliance == tim::Reliance::OutsidePack
            }),
        "",
    );
    p.add(
        t4,
        "zero windows are never met",
        matches!(exit_verdict("time", 0, &[]), ExitVerdict::NotMet(_)),
        "",
    );
    p.0
}

/// The fixed predicates the exit requires, one per claimed check. Adding a claim means
/// adding its predicate here and in [`compute_predicates`]; [`aggregate`] refuses a
/// verdict when either side lacks the other.
const REQUIRED: [(&str, &str); 106] = [
    (
        "pr15-exit-hon-01",
        "network declares cancellation points and no crash window",
    ),
    (
        "pr15-exit-hon-01",
        "process declares crash windows and routes cancellation to the runtime",
    ),
    (
        "pr15-exit-hon-01",
        "storage declares crash windows and cancellation points",
    ),
    (
        "pr15-exit-hon-01",
        "network endpoint crash is owned by the process profile",
    ),
    (
        "pr15-exit-hon-01",
        "-v0 and -v1 of each pack carry the replay-choice and early-return rows",
    ),
    (
        "pr15-exit-hon-01",
        "-v0 declares no unsupported case and -v1 declares them, in each pack",
    ),
    (
        "pr15-exit-pos-01",
        "every window realized and replayed exactly, and there is one",
    ),
    (
        "pr15-exit-pos-01",
        "no perturbation was accepted as the record",
    ),
    (
        "pr15-exit-pos-02",
        "every window realized and replayed exactly, and there is one",
    ),
    (
        "pr15-exit-pos-02",
        "no perturbation was accepted as the record",
    ),
    (
        "pr15-exit-pos-03",
        "every window realized and replayed exactly, and there is one",
    ),
    (
        "pr15-exit-pos-03",
        "no perturbation was accepted as the record",
    ),
    (
        "pr15-exit-pos-04",
        "every window realized and replayed exactly, and there is one",
    ),
    (
        "pr15-exit-pos-04",
        "no perturbation was accepted as the record",
    ),
    (
        "pr15-exit-pos-05",
        "every window realized and replayed exactly, and there is one",
    ),
    (
        "pr15-exit-pos-05",
        "no perturbation was accepted as the record",
    ),
    (
        "pr15-exit-pos-06",
        "every window realized and replayed exactly, and there is one",
    ),
    (
        "pr15-exit-pos-06",
        "no perturbation was accepted as the record",
    ),
    ("pr15-exit-pos-01", "no crash window: the pack has no crash"),
    ("pr15-exit-pos-01", "more than 100 cancellation points"),
    (
        "pr15-exit-pos-01",
        "late completions after a cancellation point",
    ),
    (
        "pr15-exit-pos-02",
        "more than 100 crash windows alone and with recovery",
    ),
    (
        "pr15-exit-pos-02",
        "late completions of a crashed incarnation fenced",
    ),
    (
        "pr15-exit-pos-03",
        "more than 100 crash windows alone and with recovery",
    ),
    (
        "pr15-exit-pos-03",
        "late completions of a crashed incarnation fenced",
    ),
    (
        "pr15-exit-pos-02",
        "no pack cancellation point: routed to the runtime",
    ),
    ("pr15-exit-pos-03", "more than 100 cancellation points"),
    ("pr15-exit-pos-04", "more than 100 composed crash windows"),
    (
        "pr15-exit-pos-04",
        "the network journal is unchanged by every crash",
    ),
    (
        "pr15-exit-pos-04",
        "the two packs agree on every node at every boundary",
    ),
    (
        "pr15-exit-pos-04",
        "every move across network steps verified to commute",
    ),
    (
        "pr15-exit-pos-05",
        "every crash fate, every value and retry: 12",
    ),
    (
        "pr15-exit-pos-05",
        "every window's run report replayed identical",
    ),
    (
        "pr15-exit-pos-06",
        "every crash fate, every value and retry: 12",
    ),
    (
        "pr15-exit-pos-06",
        "every window's run report replayed identical",
    ),
    (
        "pr15-exit-pos-07",
        "every retained correct-baseline campaign is run: 8",
    ),
    (
        "pr15-exit-neg-01",
        "process: crash moved one step later: detected + verified-equivalent + absent = windows, some detected",
    ),
    (
        "pr15-exit-neg-01",
        "process: crash moved one step earlier: detected + verified-equivalent + absent = windows, some detected",
    ),
    (
        "pr15-exit-neg-01",
        "process: crash dropped: detected + verified-equivalent + absent = windows, some detected",
    ),
    (
        "pr15-exit-neg-01",
        "process: some detections are by content at equal length",
    ),
    (
        "pr15-exit-neg-01",
        "storage: crash moved one step later: detected + verified-equivalent + absent = windows, some detected",
    ),
    (
        "pr15-exit-neg-01",
        "storage: crash moved one step earlier: detected + verified-equivalent + absent = windows, some detected",
    ),
    (
        "pr15-exit-neg-01",
        "storage: crash dropped: detected + verified-equivalent + absent = windows, some detected",
    ),
    (
        "pr15-exit-neg-01",
        "storage: some detections are by content at equal length",
    ),
    (
        "pr15-exit-neg-01",
        "composed: crash moved one step later: detected + verified-equivalent + absent = windows, some detected",
    ),
    (
        "pr15-exit-neg-01",
        "composed: crash moved one step earlier: detected + verified-equivalent + absent = windows, some detected",
    ),
    (
        "pr15-exit-neg-01",
        "composed: crash dropped: detected + verified-equivalent + absent = windows, some detected",
    ),
    (
        "pr15-exit-neg-01",
        "composed: some detections are by content at equal length",
    ),
    (
        "pr15-exit-neg-01",
        "graceful: crash moved one step later: detected + verified-equivalent + absent = windows, some detected",
    ),
    (
        "pr15-exit-neg-01",
        "graceful: crash moved one step earlier: detected + verified-equivalent + absent = windows, some detected",
    ),
    (
        "pr15-exit-neg-01",
        "graceful: crash dropped: detected + verified-equivalent + absent = windows, some detected",
    ),
    (
        "pr15-exit-neg-01",
        "graceful: some detections are by content at equal length",
    ),
    (
        "pr15-exit-neg-01",
        "fail-stop: crash moved one step later: detected + verified-equivalent + absent = windows, some detected",
    ),
    (
        "pr15-exit-neg-01",
        "fail-stop: crash moved one step earlier: detected + verified-equivalent + absent = windows, some detected",
    ),
    (
        "pr15-exit-neg-01",
        "fail-stop: crash dropped: detected + verified-equivalent + absent = windows, some detected",
    ),
    (
        "pr15-exit-neg-01",
        "fail-stop: some detections are by content at equal length",
    ),
    (
        "pr15-exit-neg-01",
        "graceful: crash dropped, the log kept aligned: detected in every window",
    ),
    (
        "pr15-exit-neg-01",
        "graceful: crash semantics swapped: detected in every window",
    ),
    (
        "pr15-exit-neg-01",
        "fail-stop: crash dropped, the log kept aligned: detected in every window",
    ),
    (
        "pr15-exit-neg-01",
        "fail-stop: crash semantics swapped: detected in every window",
    ),
    (
        "pr15-exit-neg-02",
        "network: cancellation dropped: detected + verified-equivalent + absent = points, some detected",
    ),
    (
        "pr15-exit-neg-02",
        "network: cancellation moved past the next program step: detected + verified-equivalent + absent = points, some detected",
    ),
    (
        "pr15-exit-neg-02",
        "network: cancellation moved one step later: detected + verified-equivalent + absent = points, some detected",
    ),
    (
        "pr15-exit-neg-02",
        "storage: cancellation dropped: detected + verified-equivalent + absent = points, some detected",
    ),
    (
        "pr15-exit-neg-02",
        "storage: cancellation moved past the next program step: detected + verified-equivalent + absent = points, some detected",
    ),
    (
        "pr15-exit-neg-02",
        "storage: cancellation moved one step later: detected + verified-equivalent + absent = points, some detected",
    ),
    (
        "pr15-exit-neg-03",
        "network: the undoctored record replays exactly",
    ),
    (
        "pr15-exit-neg-03",
        "network: the same log under another retained-bytes budget differs in bytes alone",
    ),
    (
        "pr15-exit-neg-03",
        "network: a doctored outcome differs in outcome alone",
    ),
    (
        "pr15-exit-neg-03",
        "network: every single-bit flip of the recorded journal is a typed divergence",
    ),
    (
        "pr15-exit-neg-03",
        "process: the undoctored record replays exactly",
    ),
    (
        "pr15-exit-neg-03",
        "process: the same log under another retained-bytes budget differs in bytes alone",
    ),
    (
        "pr15-exit-neg-03",
        "process: a doctored outcome differs in outcome alone",
    ),
    (
        "pr15-exit-neg-03",
        "process: every single-bit flip of the recorded journal is a typed divergence",
    ),
    (
        "pr15-exit-neg-03",
        "storage: the undoctored record replays exactly",
    ),
    (
        "pr15-exit-neg-03",
        "storage: the same log under another retained-bytes budget differs in bytes alone",
    ),
    (
        "pr15-exit-neg-03",
        "storage: a doctored outcome differs in outcome alone",
    ),
    (
        "pr15-exit-neg-03",
        "storage: every single-bit flip of the recorded journal is a typed divergence",
    ),
    (
        "pr15-exit-neg-03",
        "binding: no flipped byte decodes to the record",
    ),
    (
        "pr15-exit-bnd-01",
        "network: every requestable case is refused Unsupported in every window where it applies, never recorded",
    ),
    (
        "pr15-exit-bnd-01",
        "network: requestable and no-operation cases partition the declared table",
    ),
    (
        "pr15-exit-bnd-01",
        "network: the pack's own run refuses the spliced log at the splice",
    ),
    (
        "pr15-exit-bnd-01",
        "process: every requestable case is refused Unsupported in every window where it applies, never recorded",
    ),
    (
        "pr15-exit-bnd-01",
        "process: requestable and no-operation cases partition the declared table",
    ),
    (
        "pr15-exit-bnd-01",
        "process: the pack's own run refuses the spliced log at the splice",
    ),
    (
        "pr15-exit-bnd-01",
        "storage: every requestable case is refused Unsupported in every window where it applies, never recorded",
    ),
    (
        "pr15-exit-bnd-01",
        "storage: requestable and no-operation cases partition the declared table",
    ),
    (
        "pr15-exit-bnd-01",
        "storage: the pack's own run refuses the spliced log at the splice",
    ),
    (
        "pr15-exit-bnd-01",
        "no step kind asks for a no-operation case",
    ),
    (
        "pr15-exit-bnd-01",
        "the request constructors build each step kind as itself",
    ),
    (
        "pr15-exit-bnd-01",
        "binding: a crash without fail-stop semantics is CrashUnsupported, INV-008 Unsupported",
    ),
    ("pr15-exit-bnd-02", "process: a crash is refused NodeDown"),
    (
        "pr15-exit-bnd-02",
        "process: a crash is refused CrashBudgetSpent",
    ),
    (
        "pr15-exit-bnd-02",
        "process: no crash candidate is inconclusive or unsupported",
    ),
    ("pr15-exit-bnd-02", "storage: a crash is refused NodeDown"),
    (
        "pr15-exit-bnd-02",
        "storage: a crash is refused CrashBudgetSpent",
    ),
    (
        "pr15-exit-bnd-02",
        "storage: no crash candidate is inconclusive or unsupported",
    ),
    ("pr15-exit-bnd-03", "a clean artifact with windows is met"),
    ("pr15-exit-bnd-03", "an artifact with no window is not met"),
    (
        "pr15-exit-bnd-03",
        "an unsupported window is a typed gap and not met",
    ),
    (
        "pr15-exit-bnd-03",
        "a window a step bound cuts is a ResourceExhausted gap and not met",
    ),
    (
        "pr15-exit-bnd-03",
        "a record doctored after recording does not replay",
    ),
    ("pr15-exit-bnd-03", "the corpus has no gap of any kind"),
    (
        "pr15-exit-bnd-04",
        "time/unmodelled-v0 declares no operation",
    ),
    (
        "pr15-exit-bnd-04",
        "every time case is NoOperation and OutsidePack, one per row",
    ),
    ("pr15-exit-bnd-04", "zero windows are never met"),
];

fn predicates() -> &'static [Pred] {
    static CELL: OnceLock<Vec<Pred>> = OnceLock::new();
    CELL.get_or_init(compute_predicates)
}

/// Every predicate the exit requires, as `(artifact, id)`: the fixed ones, and one per
/// campaign the PR-16 golden retains. It is derived from the claims this file makes and
/// the golden, never from the results, so a predicate that goes missing is noticed.
fn required() -> Vec<(&'static str, String)> {
    let mut out: Vec<(&'static str, String)> = REQUIRED
        .iter()
        .map(|(a, id)| (*a, (*id).to_owned()))
        .collect();
    for c in retained_campaigns() {
        out.push((
            "pr15-exit-pos-07",
            format!(
                "{}: replays to its retained identity, runs and counts with no finding",
                c.name
            ),
        ));
    }
    out
}

/// The exit's machine verdict: every required predicate present exactly once and
/// passing, and no predicate the requirement list does not name. `Err` lists why not.
fn aggregate(preds: &[Pred]) -> Result<(), Vec<String>> {
    let mut why = Vec::new();
    let required = required();
    for (artifact, id) in &required {
        let found: Vec<&Pred> = preds
            .iter()
            .filter(|p| p.artifact == *artifact && &p.id == id)
            .collect();
        match found.as_slice() {
            [] => why.push(format!("missing: {artifact} {id}")),
            [one] if one.pass => {}
            [one] => why.push(format!("failing: {artifact} {id}: {}", one.detail)),
            _ => why.push(format!("duplicated: {artifact} {id}")),
        }
    }
    for p in preds {
        if !required
            .iter()
            .any(|(a, id)| *a == p.artifact && *id == p.id)
        {
            why.push(format!("not required: {} {}", p.artifact, p.id));
        }
    }
    if why.is_empty() { Ok(()) } else { Err(why) }
}

/// Assert every predicate of `artifact` passes and the whole set is complete.
fn assert_artifact(artifact: &str) {
    let preds = predicates();
    let required = required();
    assert!(
        required.iter().any(|(a, _)| *a == artifact),
        "{artifact} requires no predicate"
    );
    for (a, id) in required.iter().filter(|(a, _)| *a == artifact) {
        let p = preds
            .iter()
            .find(|p| p.artifact == *a && &p.id == id)
            .unwrap_or_else(|| panic!("{a}: missing {id}"));
        assert!(p.pass, "{a}: {id}: {}", p.detail);
    }
}

// ---------------------------------------------------------------------------
// tests
// ---------------------------------------------------------------------------

/// `pr15-exit-hon-01`. The window classes are the ones each pack's own contract
/// declares, for both of its frozen profiles, and the time pack declares none.
#[test]
fn hon_01_each_pack_declares_its_windows_in_its_own_contract() {
    assert_artifact("pr15-exit-hon-01");
}

/// `pr15-exit-pos-01`.
#[test]
fn pos_01_network_cancellation_points_replay_exactly() {
    assert_artifact("pr15-exit-pos-01");
}

/// `pr15-exit-pos-02`.
#[test]
fn pos_02_process_crash_windows_replay_exactly() {
    assert_artifact("pr15-exit-pos-02");
}

/// `pr15-exit-pos-03`.
#[test]
fn pos_03_storage_crash_windows_and_cancellation_points_replay_exactly() {
    assert_artifact("pr15-exit-pos-03");
}

/// `pr15-exit-pos-04`.
#[test]
fn pos_04_composed_crash_windows_replay_exactly_and_leave_the_network_untouched() {
    assert_artifact("pr15-exit-pos-04");
}

/// `pr15-exit-pos-05`.
#[test]
fn pos_05_register_graceful_crash_windows_replay_exactly() {
    assert_artifact("pr15-exit-pos-05");
}

/// `pr15-exit-pos-06`.
#[test]
fn pos_06_register_fail_stop_crash_windows_replay_exactly() {
    assert_artifact("pr15-exit-pos-06");
}

/// `pr15-exit-pos-07`.
#[test]
fn pos_07_the_retained_register_campaigns_replay_to_their_recorded_identities() {
    assert_artifact("pr15-exit-pos-07");
}

/// `pr15-exit-neg-01`. Every crash window, for each perturbation, is detected, verified
/// equivalent, or has no place to move to; the three add up to the windows.
#[test]
fn neg_01_a_moved_or_dropped_crash_is_a_typed_divergence() {
    assert_artifact("pr15-exit-neg-01");
}

/// `pr15-exit-neg-02`. Every cancellation point, for each perturbation, is detected,
/// verified equivalent, or has no place to move to; the three add up to the points.
#[test]
fn neg_02_a_dropped_or_moved_cancellation_is_a_typed_divergence() {
    assert_artifact("pr15-exit-neg-02");
}

/// `pr15-exit-neg-03`.
#[test]
fn neg_03_a_doctored_recorded_journal_is_a_typed_divergence() {
    assert_artifact("pr15-exit-neg-03");
}

/// `pr15-exit-bnd-01`.
#[test]
fn bnd_01_every_requestable_unsupported_case_is_a_typed_refusal_never_replayed() {
    assert_artifact("pr15-exit-bnd-01");
}

/// `pr15-exit-bnd-02`.
#[test]
fn bnd_02_a_crash_a_boundary_does_not_enable_is_typed_and_conclusive() {
    assert_artifact("pr15-exit-bnd-02");
}

/// `pr15-exit-bnd-03`.
#[test]
fn bnd_03_the_exit_fails_closed_on_an_unreplayable_window() {
    assert_artifact("pr15-exit-bnd-03");
}

/// `pr15-exit-bnd-04`.
#[test]
fn bnd_04_the_time_pack_declares_no_operation_and_so_no_window() {
    assert_artifact("pr15-exit-bnd-04");
}

/// `pr15-exit-bnd-05`. The machine verdict is the conjunction of every required
/// predicate: the real set is met, and flipping any one predicate to failing, deleting
/// it, duplicating it, or adding one the list does not name makes it not met, and the
/// rendered summary says so.
#[test]
fn bnd_05_the_machine_verdict_is_the_conjunction_of_every_claimed_check() {
    let real = predicates();
    assert_eq!(aggregate(real), Ok(()));
    assert_eq!(real.len(), required().len());
    for i in 0..real.len() {
        let mut flipped = real.to_vec();
        flipped[i].pass = false;
        assert!(
            aggregate(&flipped).is_err(),
            "flipping {} kept the verdict",
            real[i].id
        );
        let mut deleted = real.to_vec();
        deleted.remove(i);
        assert!(
            aggregate(&deleted).is_err(),
            "deleting {} kept the verdict",
            real[i].id
        );
        let mut duplicated = real.to_vec();
        duplicated.push(real[i].clone());
        assert!(
            aggregate(&duplicated).is_err(),
            "duplicating {} kept the verdict",
            real[i].id
        );
    }
    let mut extra = real.to_vec();
    extra.push(Pred {
        artifact: "pr15-exit-pos-01",
        id: "unnamed".to_owned(),
        pass: true,
        detail: String::new(),
    });
    assert!(aggregate(&extra).is_err());
    let artifacts = artifacts();
    let mut flipped = real.to_vec();
    flipped[0].pass = false;
    assert!(render_json(&artifacts, &flipped).contains("\"verdict\":\"not-met\""));
    assert!(render_json(&artifacts, real).contains("\"verdict\":\"met\""));
}

// ---------------------------------------------------------------------------
// the retained artifacts
// ---------------------------------------------------------------------------

/// A JSON value, rendered compact with sorted keys.
enum Json {
    Str(String),
    Num(u64),
    Bool(bool),
    Arr(Vec<Json>),
    Obj(BTreeMap<String, Json>),
}

impl Json {
    fn render(&self, out: &mut String) {
        match self {
            Self::Str(s) => {
                out.push('"');
                for c in s.chars() {
                    match c {
                        '"' => out.push_str("\\\""),
                        '\\' => out.push_str("\\\\"),
                        '\n' => out.push_str("\\n"),
                        c if u32::from(c) < 0x20 => {
                            let _ = write!(out, "\\u{:04x}", u32::from(c));
                        }
                        c => out.push(c),
                    }
                }
                out.push('"');
            }
            Self::Num(n) => {
                let _ = write!(out, "{n}");
            }
            Self::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
            Self::Arr(items) => {
                out.push('[');
                for (i, item) in items.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    item.render(out);
                }
                out.push(']');
            }
            Self::Obj(members) => {
                out.push('{');
                for (i, (k, v)) in members.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    Self::Str(k.clone()).render(out);
                    out.push(':');
                    v.render(out);
                }
                out.push('}');
            }
        }
    }
}

fn obj(members: Vec<(&str, Json)>) -> Json {
    Json::Obj(
        members
            .into_iter()
            .map(|(k, v)| (k.to_owned(), v))
            .collect(),
    )
}

fn s(text: &str) -> Json {
    Json::Str(text.to_owned())
}

/// One artifact: its ID, claim, test, and counts.
struct Artifact {
    id: &'static str,
    claim: &'static str,
    test: &'static str,
    counts: BTreeMap<String, u64>,
    facts: BTreeMap<String, String>,
}

fn tally_artifact(
    id: &'static str,
    claim: &'static str,
    test: &'static str,
    t: &Tally,
) -> Artifact {
    let mut facts = BTreeMap::new();
    facts.insert("journal corpus digest".to_owned(), t.digest());
    facts.insert(
        "verdict".to_owned(),
        match exit_verdict(id, windows(t), &t.gaps) {
            ExitVerdict::Met { .. } if t.undetected.is_empty() => "met".to_owned(),
            other => format!("NOT MET: {other:?} undetected {:?}", t.undetected),
        },
    );
    Artifact {
        id,
        claim,
        test,
        counts: t.counts.clone(),
        facts,
    }
}

fn artifacts() -> Vec<Artifact> {
    let e = evidence();
    let mut out = Vec::new();
    let mut hon = Artifact {
        id: "pr15-exit-hon-01",
        claim: "the windows each pack's own contract declares, for -v0 and -v1, and the time pack's none",
        test: "hon_01_each_pack_declares_its_windows_in_its_own_contract",
        counts: BTreeMap::new(),
        facts: BTreeMap::new(),
    };
    for (pack, d, profiles) in [
        ("network", network_declared(), net::PROFILES.map(|p| p.name)),
        ("process", process_declared(), prc::PROFILES.map(|p| p.name)),
        ("storage", storage_declared(), sto::PROFILES.map(|p| p.name)),
    ] {
        hon.facts.insert(
            format!("{pack} declares"),
            format!(
                "crash windows {}, cancellation points {} (profiles {})",
                d.crash_windows,
                d.cancellation_points,
                profiles.join(", ")
            ),
        );
    }
    hon.facts.insert(
        "time declares".to_owned(),
        format!(
            "{} operations {:?}: no window; {} unsupported cases, each NoOperation",
            tim::UNMODELLED_V0.name,
            tim::UNMODELLED_V0.operations,
            tim::UNSUPPORTED.len()
        ),
    );
    out.push(hon);
    out.push(tally_artifact(
        "pr15-exit-pos-01",
        "network cancellation points replay exactly",
        "pos_01_network_cancellation_points_replay_exactly",
        &e.network,
    ));
    out.push(tally_artifact(
        "pr15-exit-pos-02",
        "process crash windows replay exactly; its cancellation points are routed to the runtime",
        "pos_02_process_crash_windows_replay_exactly",
        &e.process,
    ));
    out.push(tally_artifact(
        "pr15-exit-pos-03",
        "storage crash windows and cancellation points replay exactly",
        "pos_03_storage_crash_windows_and_cancellation_points_replay_exactly",
        &e.storage,
    ));
    out.push(tally_artifact(
        "pr15-exit-pos-04",
        "composed crash windows replay exactly and leave the network journal unchanged",
        "pos_04_composed_crash_windows_replay_exactly_and_leave_the_network_untouched",
        &e.composed,
    ));
    out.push(tally_artifact(
        "pr15-exit-pos-05",
        "the register's graceful crash windows (binding Cancel) replay exactly across lab seeds",
        "pos_05_register_graceful_crash_windows_replay_exactly",
        &e.graceful,
    ));
    out.push(tally_artifact(
        "pr15-exit-pos-06",
        "the register's fail-stop crash windows (binding Crash) replay exactly across lab seeds",
        "pos_06_register_fail_stop_crash_windows_replay_exactly",
        &e.fail_stop,
    ));
    let mut campaigns = Artifact {
        id: "pr15-exit-pos-07",
        claim: "the retained register campaigns replay to their recorded identities",
        test: "pos_07_the_retained_register_campaigns_replay_to_their_recorded_identities",
        counts: BTreeMap::new(),
        facts: BTreeMap::new(),
    };
    for c in &e.campaigns {
        let name = &c.retained.name;
        campaigns.counts.insert(
            format!("{name} runs"),
            u64::try_from(c.runs).expect("small"),
        );
        campaigns.counts.insert(
            format!("{name} plans with a crash"),
            u64::try_from(c.crash_plans).expect("small"),
        );
        let compared: Vec<String> = c
            .compared
            .iter()
            .map(|(k, replayed, retained)| format!("{k} {replayed} (retained {retained})"))
            .collect();
        campaigns.facts.insert(
            name.clone(),
            match &c.unrunnable {
                Some(why) => format!("NOT RUN: {why}"),
                None => format!(
                    "replayed identity {}; retained identity {}; findings {}; {}",
                    c.identity,
                    c.retained.identity,
                    c.findings,
                    compared.join(", ")
                ),
            },
        );
    }
    out.push(campaigns);
    let mut unsupported = Artifact {
        id: "pr15-exit-bnd-01",
        claim: "every requestable declared unsupported case is a typed refusal, never replayed",
        test: "bnd_01_every_requestable_unsupported_case_is_a_typed_refusal_never_replayed",
        counts: BTreeMap::new(),
        facts: BTreeMap::new(),
    };
    for ((tally, none), pack) in e.probes.iter().zip(["network", "process", "storage"]) {
        for (k, v) in &tally.counts {
            unsupported.counts.insert(format!("{pack} {k}"), *v);
        }
        unsupported
            .facts
            .insert(format!("{pack} no-operation cases"), none.join(", "));
    }
    unsupported.facts.insert(
        "binding".to_owned(),
        "a crash of a region whose live task runs under a deadline is CrashUnsupported, INV-008 Unsupported".to_owned(),
    );
    out.push(unsupported);
    out
}

fn render_golden(artifacts: &[Artifact]) -> String {
    let mut out = String::new();
    let _ = writeln!(
        out,
        "# PR-15 exit evidence (bn-jw7q): crash windows and cancellation points replay exactly.\n\
         # crates/continuum-asupersync/tests/pr15_exit_evidence.rs; machine-readable twin tests/evidence/pr15-exit.json.\n\
         # Regenerate: PR15_EXIT_BLESS=1 cargo test -p continuum-asupersync --test pr15_exit_evidence\n\
         # corpus: seeds {SEEDS:?}, {LOGS_PER_SEED} logs of {LOG_LEN} steps per seed per pack; lab seeds {LAB_SEEDS:?}; \
         register interleavings {REGISTER_LOGS} per crash fate and mode from seed {REGISTER_SEED:#x}"
    );
    for a in artifacts {
        let _ = writeln!(out, "\n[{}]\nclaim: {}\ntest: {}", a.id, a.claim, a.test);
        for (k, v) in &a.counts {
            let _ = writeln!(out, "count {k} = {v}");
        }
        for (k, v) in &a.facts {
            let _ = writeln!(out, "fact {k} = {v}");
        }
    }
    let _ = writeln!(
        out,
        "\n[predicates]\nverdict: {}",
        match aggregate(predicates()) {
            Ok(()) => "met: every required predicate is present once and passes".to_owned(),
            Err(why) => format!("NOT MET: {why:?}"),
        }
    );
    for p in predicates() {
        let _ = writeln!(
            out,
            "{} {} {}: {}",
            if p.pass { "pass" } else { "FAIL" },
            p.artifact,
            p.id,
            p.detail
        );
    }
    out
}

fn render_json(artifacts: &[Artifact], preds: &[Pred]) -> String {
    let verdict = aggregate(preds);
    let doc = obj(vec![
        ("requirement", s("PR-15-EXIT")),
        (
            "exit",
            s("crash windows and cancellation points replay exactly"),
        ),
        ("bone", s("bn-jw7q")),
        (
            "suite",
            s("crates/continuum-asupersync/tests/pr15_exit_evidence.rs"),
        ),
        (
            "golden",
            s("crates/continuum-asupersync/tests/golden/pr15_exit_evidence.txt"),
        ),
        (
            "verdict",
            s(if verdict.is_ok() { "met" } else { "not-met" }),
        ),
        (
            "verdict_rule",
            s(
                "met only when every required predicate is present exactly once and passes, and no unrequired predicate appears",
            ),
        ),
        (
            "not_met_because",
            Json::Arr(
                verdict
                    .err()
                    .unwrap_or_default()
                    .iter()
                    .map(|w| s(w))
                    .collect(),
            ),
        ),
        (
            "predicates",
            Json::Arr(
                preds
                    .iter()
                    .map(|p| {
                        obj(vec![
                            ("artifact", s(p.artifact)),
                            ("id", s(&p.id)),
                            ("pass", Json::Bool(p.pass)),
                            ("detail", s(&p.detail)),
                        ])
                    })
                    .collect(),
            ),
        ),
        ("fail_closed", Json::Bool(true)),
        (
            "replay",
            s(
                "each realized window is recorded as canonical journal bytes, choice log and outcome, and replayed from the recorded bytes (an independent decoder, twice), from the recorded plan, and by the pack's own Journal::replay; binding windows replay from the recorded plan and log under 7 lab seeds and round-trip Journal::decode; the retained register campaigns are re-executed and must reproduce their retained identities",
            ),
        ),
        (
            "artifacts",
            Json::Arr(
                artifacts
                    .iter()
                    .map(|a| {
                        obj(vec![
                            ("id", s(a.id)),
                            ("claim", s(a.claim)),
                            ("test", s(a.test)),
                            (
                                "counts",
                                Json::Obj(
                                    a.counts
                                        .iter()
                                        .map(|(k, v)| (k.clone(), Json::Num(*v)))
                                        .collect(),
                                ),
                            ),
                            (
                                "facts",
                                Json::Obj(a.facts.iter().map(|(k, v)| (k.clone(), s(v))).collect()),
                            ),
                        ])
                    })
                    .collect(),
            ),
        ),
        (
            "negative",
            Json::Arr(vec![
                s(
                    "pr15-exit-neg-01: in the process, storage, composed and register crash windows, each window's crash moved one step later, moved one step earlier, and dropped gets exactly one disposition: a typed divergence, verified equivalent (the neighbour is the same step), or absent (no step on that side, or at the binding the first or last operation of the script); the three add up to the windows for each perturbation, dropped is detected in every window, and the per-window accounting is in the predicates; at the binding a crash dropped with the log kept aligned, and a crash replayed under the other semantics, is detected in every window; detections a length-only checker would miss are counted",
                ),
                s(
                    "pr15-exit-neg-02: at the network and storage cancellation points (the process pack routes its cancellation to the runtime), each point gets exactly one disposition per perturbation, and the dispositions add up to the points: dropped is a typed divergence when a program step follows the point and verified equivalent when none does; moved past the next program step is a typed divergence whenever a program step follows, absent otherwise; moved one step later is a typed divergence when the crossed step is the program's, verified equivalent when it is a scheduler or adversary step, and absent at the last boundary",
                ),
                s(
                    "pr15-exit-neg-03: each single-bit flip of a recorded pack journal replays as a typed divergence; a flipped binding journal never decodes to the record",
                ),
            ]),
        ),
        (
            "boundary",
            Json::Arr(vec![
                s(
                    "pr15-exit-bnd-02: a crash a boundary does not enable is refused NotEnabled for a named reason, never inconclusive",
                ),
                s(
                    "pr15-exit-bnd-03: an unsupported, bound-cut, diverging or empty artifact makes the verdict NotMet with a typed gap",
                ),
                s(
                    "pr15-exit-bnd-04: time/unmodelled-v0 declares Operations::None, so it has no window; that is reported, never counted as met",
                ),
                s(
                    "pr15-exit-bnd-05: the verdict is met only when every required predicate is present exactly once and passes; flipping, deleting or duplicating any one, or adding an unrequired one, makes it not-met",
                ),
            ]),
        ),
        (
            "narrowing",
            Json::Arr(vec![
                s(
                    "no host semantics: every pack is HostQualification::None; this is exact replay of the Lab handlers, not of a host",
                ),
                s(
                    "no substrate binding: the register program does not drive the packs; its crash windows are the binding's Cancel (process profile row graceful-cancellation) and Crash (row fail-stop-crash) of an incarnation's region",
                ),
                s(
                    "the time pack has no operation, so no window; virtual time across a crash is the binding's, exercised by the carried-timer fail-stop campaign",
                ),
                s(
                    "pack windows are every boundary of seeded logs under two configurations per pack; register windows are every crash fate over a seeded sample of interleavings; every retained correct-baseline campaign is re-executed in full",
                ),
                s(
                    "-v0 is covered through the rows and contract it shares with -v1; the one Lab handler journals under the -v1 header, so no -v0 journal exists",
                ),
                s(
                    "the process pack routes its cancellation points to the runtime; they are the binding's region Cancel (pos-05) and the PR-14 exit evidence's",
                ),
                s(
                    "a pack event maps one to one to its step, so perturbation detection shows the recorded journal fixes the run; neg-03 shows the byte and outcome comparisons are each load-bearing",
                ),
                s(
                    "composed runs have crash windows only; moves across network steps are verified to commute and repeated windows are verified equal, not counted twice",
                ),
            ]),
        ),
    ]);
    let mut out = String::new();
    doc.render(&mut out);
    out.push('\n');
    out
}

/// The retained artifacts. The golden pins every count and corpus digest, so any
/// change to the evidence is visible; the JSON is its machine-readable twin.
#[test]
fn the_exit_evidence_artifacts_are_byte_stable() {
    const GOLDEN: &str = include_str!("golden/pr15_exit_evidence.txt");
    const JSON: &str = include_str!("evidence/pr15-exit.json");
    let artifacts = artifacts();
    let golden = render_golden(&artifacts);
    let json = render_json(&artifacts, predicates());
    if std::env::var_os("PR15_EXIT_BLESS").is_some() {
        let root = env!("CARGO_MANIFEST_DIR");
        std::fs::write(
            format!("{root}/tests/golden/pr15_exit_evidence.txt"),
            &golden,
        )
        .expect("the golden is writable");
        std::fs::write(format!("{root}/tests/evidence/pr15-exit.json"), &json)
            .expect("the summary is writable");
        panic!(
            "PR15_EXIT_BLESS rewrote the golden and the summary; rerun without the variable \
             and review the diff as a contract change"
        );
    }
    assert!(
        json.contains("\"verdict\":\"met\""),
        "the exit is not met:\n{json}"
    );
    assert_eq!(
        golden, GOLDEN,
        "the exit evidence drifted; to regenerate deliberately, run with PR15_EXIT_BLESS=1 \
         and review the diff.\nactual:\n{golden}"
    );
    assert_eq!(json, JSON, "the machine-readable summary drifted");
    // Rendered twice, identical bytes.
    assert_eq!(golden, render_golden(&self::artifacts()));
}
