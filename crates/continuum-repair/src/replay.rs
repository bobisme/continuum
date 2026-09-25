//! Exact replay (PR-20 / IMPL-03): gate 1, `base_replay`, and gate 4, `exact_regression`,
//! evaluated from the transaction's crashpack and recorded on its gate list.
//!
//! # What the RFC makes the two gates
//!
//! > | 1 | `base_replay` | the original failure replays on the base snapshot | a replay run
//! > reproducing the crashpack's failure identity | evaluating against a base that never
//! > failed |
//! > | 4 | `exact_regression` | the exact failure no longer occurs | a replay run on the
//! > candidate under the original choices | — |
//! >
//! > **Gate 1 distinguishes non-reproduction from engine divergence.** The gate is
//! > `failed` when the failure does not reproduce on the base snapshot. A
//! > `ReplayDiverged` condition is `inconclusive` and MUST additionally publish a
//! > `defect_*` artifact.
//! >
//! > **Gate 4 handles eliminated events.** Where the patch removes an event the original
//! > replay depended on, the gate uses the §16 correspondence to report where replay
//! > diverges and continues under the transaction's declared `evaluation_policy`. It MUST
//! > NOT pass by declaring the divergence uninteresting.
//! >
//! > — RFC 0032, "The twelve gates"
//!
//! # Clause → mechanism
//!
//! | Clause | Mechanism |
//! |---|---|
//! | the crashpack is the one the transaction names, unaltered | [`evaluate`] re-derives `crash_` from the supplied bytes and refuses any other identity, [`ExactReplayRefusal::CrashpackTampered`], before it decodes a byte |
//! | the crashpack was recorded on the transaction's base | the record binds its base snapshot; another base is [`ExactReplayRefusal::ForeignCrashpack`] |
//! | the replayed programs are the base and the candidate | both contents are re-derived to their `ws_` identities and compared with the frozen `base_snapshot` and the sealed `candidate_snapshot` |
//! | gate 1 passes only on the exact recorded failure | [`BaseReplay::Reproduced`] needs the byte-identical journal, the byte-identical failure identity, and every recorded step run in order |
//! | gate 1 is `failed` when the failure does not reproduce on the base | a base run without the recorded failure identity is [`BaseReplay::NotReproduced`] |
//! | a replay divergence on the base is an engine condition, `inconclusive`, with a `defect_*` (RFC 0032 correction 6) | [`BaseReplay::Diverged`], and [`BaseReplay::Inconsistent`] when the failure reproduces but the run does not (a different journal, a missing or reordered step): the base, the crashpack and the schedule are verified, so only the recorder or the replayer can differ. Both carry a `defect_` record; INV-008 reason `EngineError` |
//! | gate 4 replays the candidate under the original choices | the same recorded schedule, handed to the same [`Replayer`] |
//! | a divergence is not a pass | [`ExactRegression::Diverged`] is `inconclusive` (`AbstractionAmbiguity`: the recorded choices do not determine the candidate's run) |
//! | the failure no longer occurs ⇒ passed; it recurs ⇒ failed | [`ExactRegression::NoLongerFails`], [`ExactRegression::Recurs`]; a run that fails another way is [`ExactRegression::FailsDifferently`], `failed` |
//! | gate 4 continues past eliminated events only "under the transaction's declared `evaluation_policy`"; it "MUST NOT pass by declaring the divergence uninteresting" | [`evaluate`] runs under [`EvaluationPolicy::Exact`] unless it is handed a [`PolicyAuthorization`]; the caller cannot choose the policy. Under `Exact` a clean candidate run with any eliminated or reordered step is [`ExactRegression::NotExact`], `inconclusive`, never `passed`. Only an authorization bound to this version's `rt_` identity, its base and candidate snapshots and its base intent, and checked against them, selects [`EvaluationPolicy::ContinueAndDisclose`]; a mismatch is a typed refusal, [`ExactReplayRefusal::Authorization`]. The recorded `evaluation_policy`, part of the `rt_` identity, names the policy and, for an authorized one, the authorization's digest |
//! | eliminated events are reported, with where | a run's `ev_` record lists the positions in the recorded schedule of the steps the program lacks, and counts the steps it ran out of the recorded order; the outcome carries both counts |
//! | a gate cannot pass on absent evidence; an `inconclusive` gate carries its typed reason | every recorded outcome cites the crashpack and one `ev_` or `defect_` record, `Unsupported` included. Each record binds the gate, the crashpack, the program, the replayer ([`Replayer::identity`]), the evaluation policy, the gate status and the typed INV-008 reason. The records are returned only with the version ([`ExactReplay::into_parts`]) |
//! | a gate not evaluated stays `pending` | when gate 1 does not pass, the candidate is not run and gate 4 is not recorded ([`ExactRegression::NotRun`]) |
//! | statuses do not move twice in one evaluation | both gates must be `pending`; the outcome is a new version ([`RepairTransaction::record_outcomes`]) |
//! | no ambient nondeterminism | the result is a function of the transaction, the bytes, the two contents and the replayer's answers |
//!
//! # The recorded run: what a crashpack carries for these gates
//!
//! `crashpack.schema.json` is a manifest whose `replay.choice_log`,
//! `artifacts.linear_replay` and `replay.expected_verdict` are strings that *name* the
//! schedule, the journal and the verdict. [`RecordedRun`] is that named content, bound
//! under one identity: the base snapshot it was captured on, the schedule, the journal
//! and the failure identity, in one length-prefixed record. Its `crash_` handle is the
//! digest of those bytes under the lineage's hasher, so any altered byte of any part
//! names another crashpack. No crashpack producer exists in the workspace yet (the
//! daemon's `VerificationResult.crashpack` has none), and no text fixes how a manifest's
//! `id` is derived; this record is the replay payload a producer must bind, and the
//! manifest wrapping is the producer's (open question below).
//!
//! The schedule, the journal and the failure identity are opaque here: the
//! [`Replayer`] that produced them reads the schedule, and this module compares the
//! other two byte for byte and never parses them.
//!
//! # Which replayer
//!
//! [`Replayer`] is the seam to the program half (the asupersync binding). It is given a
//! program's content and the recorded schedule and answers one of three things: the run
//! ([`ReplayOutcome::Ran`]), the point at which the program could not follow the schedule
//! ([`ReplayOutcome::Diverged`]), or that it cannot run the program at all
//! ([`ReplayOutcome::Unsupported`]). What "the original choices" are for a changed
//! program is the replayer's correspondence, and it must report the recorded steps the
//! program lacks (`eliminated`) rather than hide them. The instantiation on the
//! replicated register, `crates/continuum-asupersync/tests/pr20_impl03_exact_replay.rs`,
//! states its correspondence.
//!
//! # Not here
//!
//! - Daemon wiring: `repair.evaluate` is not served (bn-23pwm), so no production path
//!   calls [`evaluate`] yet, and publishing the `ev_` and `defect_` records it returns is
//!   the daemon's. The crashpack bytes and both contents come from the daemon's stores.
//! - The plan §16 correspondence (PR 17, CIR): gate 4 uses the replayer's own
//!   correspondence.
//! - **No production path issues a [`PolicyAuthorization`].** Its constructor is private
//!   to this crate, and the public seam [`request_continue_and_disclose`] returns the
//!   typed absence [`AuthorizationAbsence::NoAuthority`] today, so every production
//!   evaluation runs under [`EvaluationPolicy::Exact`]. Who may authorize
//!   `ContinueAndDisclose`, and on what evidence, is owned by bn-2vanm (the RFC 0032
//!   correction) and bn-b6u4 (the authority that issues it).
//! - Cost: the replay runs are not charged to `cost_ledger` (IMPL-05 with the daemon's
//!   budget); the ledger carries over unchanged.
//! - A `ReplayDiverged` gate-1 outcome makes the `defect_` record; publishing it as an
//!   engine-defect report under plan §4.7 is the daemon's.
//! - The typed INV-008 reason of an `inconclusive` gate is in the gate's cited evidence
//!   record, not in the gate entry: the schema's gate entry has no field for it.
//! - The gates trust the [`Replayer`]: the records state its answers by digest and name
//!   it, and the journals themselves are not in them.
//!
//! # Open questions raised, not resolved here
//!
//! - **The crashpack manifest's identity.** `crashpack.schema.json` has an `id` and a
//!   `created_at` and no derivation rule; here `crash_` names the replay payload. A
//!   producer must either name the manifest by this payload's identity or bind the payload
//!   identity inside the manifest.
//! - **Which status a persisted partial campaign has.** RFC 0032's rule 4 derives
//!   `evaluating` from "a running or budget-suspended gate campaign", and its field table
//!   makes `semantic_diff` non-null "from `evaluating` onward". A version with gates 1
//!   and 4 recorded and every other gate pending is neither running nor diffed yet
//!   (IMPL-04). This crate derives `evaluating` for it and emits no `semantic_diff`,
//!   which the schema admits and the table does not; rule 6 would read `blocked` for a
//!   failed gate 1. The RFC needs a correction for a partial campaign's status.
//! - **A candidate that fails another way.** RFC 0032's gate-4 claim is that "the exact
//!   failure no longer occurs". A candidate run under the original choices that fails with
//!   a different failure identity does not refute that claim, but a pass would present a
//!   failing run as gate evidence. This module fails it, [`ExactRegression::FailsDifferently`];
//!   the RFC should say which.

use core::fmt;
use core::marker::PhantomData;

use continuum_value::assurance::InconclusiveReason;
use continuum_value::identity::ContentHasher;

use crate::handle::{CrashpackId, EvidenceRef, MalformedHandle, RepairId, SnapshotId};

// Re-exported so that a [`Replayer`] or a caller outside this crate can name the
// program's content, its paths and tree, and a base intent without an edge of its own
// to `continuum-workspace` or `continuum-intent`: these types are already part of this
// crate's public API.
use crate::patch::HashedIdentifier;
use crate::transaction::{GateName, GateStatus, RecordRefusal, RepairTransaction};
pub use continuum_intent::contract::IntentId;
pub use continuum_workspace::snapshot::{Snapshot, SnapshotError, WorkspaceContent, WorkspacePath};

/// Domain tag of the recorded-run record.
const RECORDED_RUN_TAG: &[u8] = b"continuum-repair/recorded-run";
/// Domain tag of a replay-run evidence record (`ev_`).
const REPLAY_RUN_TAG: &[u8] = b"continuum-repair/replay-run";
/// Domain tag of a replay-divergence defect record (`defect_`).
const DEFECT_TAG: &[u8] = b"continuum-repair/replay-defect";

/// Version of the three record grammars. Changing it changes every identity they name.
pub const REPLAY_ENCODING_VERSION: u8 = 1;

// --- the recorded run --------------------------------------------------------------

/// The failing run a crashpack records, as far as gates 1 and 4 read it.
///
/// Built by [`Self::new`] (a producer) or [`Self::decode`] (a reader); both hold the
/// same invariants, and [`Self::canonical_bytes`] is the one spelling, so
/// `decode(canonical_bytes())` is the identity and so is the converse.
#[derive(Clone, PartialEq, Eq)]
pub struct RecordedRun {
    base_snapshot: SnapshotId,
    schedule: Vec<u8>,
    journal: Vec<u8>,
    failure: Vec<u8>,
}

impl fmt::Debug for RecordedRun {
    /// Lengths only: the parts are opaque and may be large.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RecordedRun")
            .field("base_snapshot", &self.base_snapshot)
            .field("schedule", &format_args!("<{} bytes>", self.schedule.len()))
            .field("journal", &format_args!("<{} bytes>", self.journal.len()))
            .field("failure", &format_args!("<{} bytes>", self.failure.len()))
            .finish()
    }
}

/// Why bytes are not a recorded run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MalformedRecord {
    /// The bytes do not start with the record's domain tag.
    Tag,
    /// The record is written in another encoding version.
    Version(u8),
    /// A field's length prefix runs past the end of the bytes.
    Truncated,
    /// Bytes remain after the last field.
    TrailingBytes,
    /// The base snapshot field is not a `ws_` handle.
    BaseSnapshot(MalformedHandle),
    /// The failure identity is empty: a crashpack records a failure.
    NoFailure,
}

impl fmt::Display for MalformedRecord {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Tag => f.write_str("not a recorded-run record"),
            Self::Version(version) => write!(f, "recorded-run encoding version {version}"),
            Self::Truncated => f.write_str("a field runs past the end of the record"),
            Self::TrailingBytes => f.write_str("bytes follow the last field"),
            Self::BaseSnapshot(error) => write!(f, "the base snapshot field: {error}"),
            Self::NoFailure => f.write_str("the record names no failure"),
        }
    }
}

impl std::error::Error for MalformedRecord {}

impl RecordedRun {
    /// A recorded run of the program at `base_snapshot` under `schedule`, whose journal
    /// was `journal` and whose failure identity was `failure`.
    ///
    /// # Errors
    ///
    /// [`MalformedRecord::NoFailure`] when `failure` is empty.
    pub fn new(
        base_snapshot: SnapshotId,
        schedule: Vec<u8>,
        journal: Vec<u8>,
        failure: Vec<u8>,
    ) -> Result<Self, MalformedRecord> {
        if failure.is_empty() {
            return Err(MalformedRecord::NoFailure);
        }
        Ok(Self {
            base_snapshot,
            schedule,
            journal,
            failure,
        })
    }

    /// Read a record from its canonical bytes. Strict: the tag, the version, every
    /// length prefix within the input, and nothing after the last field. Each length is
    /// checked against the bytes that remain before anything is copied, so the work and
    /// the memory are linear in the input.
    ///
    /// # Errors
    ///
    /// [`MalformedRecord`].
    pub fn decode(bytes: &[u8]) -> Result<Self, MalformedRecord> {
        let Some(rest) = bytes.strip_prefix(RECORDED_RUN_TAG) else {
            return Err(MalformedRecord::Tag);
        };
        let mut reader = Reader(rest);
        let version = reader.take(1)?[0];
        if version != REPLAY_ENCODING_VERSION {
            return Err(MalformedRecord::Version(version));
        }
        let base = reader.field()?;
        let schedule = reader.field()?;
        let journal = reader.field()?;
        let failure = reader.field()?;
        if !reader.0.is_empty() {
            return Err(MalformedRecord::TrailingBytes);
        }
        let base = core::str::from_utf8(base).map_err(|_| {
            MalformedRecord::BaseSnapshot(MalformedHandle {
                pattern: SnapshotId::PATTERN,
            })
        })?;
        let base_snapshot = SnapshotId::new(base).map_err(MalformedRecord::BaseSnapshot)?;
        Self::new(
            base_snapshot,
            schedule.to_vec(),
            journal.to_vec(),
            failure.to_vec(),
        )
    }

    /// The canonical bytes: tag, version, then the base snapshot, the schedule, the
    /// journal and the failure identity, each with a big-endian `u64` length prefix.
    #[must_use]
    pub fn canonical_bytes(&self) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(RECORDED_RUN_TAG);
        out.push(REPLAY_ENCODING_VERSION);
        field(&mut out, self.base_snapshot.as_str().as_bytes());
        field(&mut out, &self.schedule);
        field(&mut out, &self.journal);
        field(&mut out, &self.failure);
        out
    }

    /// The `crash_` handle naming this record under `H`: the digest of its canonical
    /// bytes. The digest indexes; the bytes are the identity.
    #[must_use]
    pub fn identity<H: ContentHasher>(&self) -> CrashpackId {
        crashpack_id::<H>(&self.canonical_bytes())
    }

    /// The snapshot the run was recorded on.
    #[must_use]
    pub const fn base_snapshot(&self) -> &SnapshotId {
        &self.base_snapshot
    }

    /// The recorded schedule, which only a [`Replayer`] reads.
    #[must_use]
    pub fn schedule(&self) -> &[u8] {
        &self.schedule
    }

    /// The recorded journal.
    #[must_use]
    pub fn journal(&self) -> &[u8] {
        &self.journal
    }

    /// The recorded failure identity.
    #[must_use]
    pub fn failure(&self) -> &[u8] {
        &self.failure
    }
}

fn crashpack_id<H: ContentHasher>(bytes: &[u8]) -> CrashpackId {
    let handle = format!("crash_{}", H::hash(bytes).to_token());
    CrashpackId::new(&handle).unwrap_or_else(|_| unreachable!("a hex token is a handle suffix"))
}

fn field(out: &mut Vec<u8>, bytes: &[u8]) {
    let length = u64::try_from(bytes.len()).unwrap_or_else(|_| unreachable!("a slice fits u64"));
    out.extend_from_slice(&length.to_be_bytes());
    out.extend_from_slice(bytes);
}

struct Reader<'a>(&'a [u8]);

impl<'a> Reader<'a> {
    fn take(&mut self, count: usize) -> Result<&'a [u8], MalformedRecord> {
        if count > self.0.len() {
            return Err(MalformedRecord::Truncated);
        }
        let (head, tail) = self.0.split_at(count);
        self.0 = tail;
        Ok(head)
    }

    fn field(&mut self) -> Result<&'a [u8], MalformedRecord> {
        let prefix: [u8; 8] = self
            .take(8)?
            .try_into()
            .unwrap_or_else(|_| unreachable!("eight bytes"));
        // A length that does not fit `usize` cannot fit the remaining input either.
        let count =
            usize::try_from(u64::from_be_bytes(prefix)).map_err(|_| MalformedRecord::Truncated)?;
        self.take(count)
    }
}

// --- the replayer seam -------------------------------------------------------------

/// The program half: replays a recorded schedule on a program's content.
///
/// It must be deterministic (INV-005): the same content and schedule give the same
/// answer. What the recorded schedule means for a program that differs from the one it
/// was recorded on is this seam's correspondence; it must report the recorded steps the
/// program lacks ([`ReplayRun::eliminated`]) and the ones it ran out of the recorded
/// order ([`ReplayRun::reordered`]), and must answer [`ReplayOutcome::Diverged`], never
/// a run, when the schedule does not determine the program's run.
///
/// **The gates trust the replayer.** Gates 1 and 4 are as good as its runs: the
/// evidence records name the replayer by [`Self::identity`] and state its answer by
/// digest, so a replayer that answers without running is caught only by whoever
/// re-runs it from the records. Which replayers the daemon admits is the daemon's
/// (bn-23pwm).
pub trait Replayer {
    /// The replayer and its correspondence, with a version: bound into every evidence
    /// record, so two correspondences never share an `ev_` for one claim.
    fn identity(&self) -> &str;

    /// Replay `schedule` on the program `program` holds.
    fn replay(&self, program: &WorkspaceContent, schedule: &[u8]) -> ReplayOutcome;
}

/// What one replay did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReplayOutcome {
    /// The program ran to the end under the replayer's correspondence with the schedule,
    /// with every step it took placed by the schedule.
    Ran(ReplayRun),
    /// The program could not follow the schedule.
    Diverged(Divergence),
    /// The replayer cannot run this program at all (INV-008 `Unsupported`).
    Unsupported,
}

/// A replay that ran to the end.
#[derive(Clone, PartialEq, Eq)]
pub struct ReplayRun {
    journal: Vec<u8>,
    failure: Option<Vec<u8>>,
    eliminated: Vec<u64>,
    reordered: u64,
}

impl fmt::Debug for ReplayRun {
    /// Lengths and counts only.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ReplayRun")
            .field("journal", &format_args!("<{} bytes>", self.journal.len()))
            .field(
                "failure",
                &self
                    .failure
                    .as_ref()
                    .map(|failure| format!("<{} bytes>", failure.len())),
            )
            .field("eliminated", &self.eliminated.len())
            .field("reordered", &self.reordered)
            .finish()
    }
}

impl ReplayRun {
    /// A run whose journal was `journal`, which failed with the identity `failure` or did
    /// not fail (`None`); `eliminated` are the positions in the recorded schedule of the
    /// steps the program did not have, and `reordered` counts the steps it ran out of the
    /// recorded order. `Some` failure is a failure whatever its bytes: an empty identity
    /// never reads as no failure.
    ///
    /// The positions are kept sorted and without duplicates, so the record states a set.
    #[must_use]
    pub fn new(
        journal: Vec<u8>,
        failure: Option<Vec<u8>>,
        mut eliminated: Vec<u64>,
        reordered: u64,
    ) -> Self {
        eliminated.sort_unstable();
        eliminated.dedup();
        Self {
            journal,
            failure,
            eliminated,
            reordered,
        }
    }

    /// The run's journal.
    #[must_use]
    pub fn journal(&self) -> &[u8] {
        &self.journal
    }

    /// The run's failure identity, if it failed.
    #[must_use]
    pub fn failure(&self) -> Option<&[u8]> {
        self.failure.as_deref()
    }

    /// The positions in the recorded schedule of the steps the program did not have,
    /// ascending.
    #[must_use]
    pub fn eliminated(&self) -> &[u64] {
        &self.eliminated
    }

    /// How many steps the program ran out of the recorded order.
    #[must_use]
    pub const fn reordered(&self) -> u64 {
        self.reordered
    }

    fn follows_exactly(&self) -> bool {
        self.eliminated.is_empty() && self.reordered == 0
    }
}

/// Where a program stopped following a schedule: the number of recorded steps it had
/// followed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Divergence {
    /// Recorded steps followed before the divergence.
    pub at: u64,
}

// --- the gate outcomes ---------------------------------------------------------------

/// How the recorded failure compares with a replay's.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FailureMatch {
    /// The byte-identical failure identity.
    Same,
    /// The run did not fail.
    Absent,
    /// The run failed with another identity.
    Different,
}

/// Gate 1's outcome.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BaseReplay {
    /// The base replays the exact recorded failure: the journal and the failure identity
    /// are byte-identical, and every recorded step ran, in order. `passed`.
    Reproduced {
        /// The run's `ev_` record.
        run: EvidenceRef,
    },
    /// The base ran the schedule and the recorded failure did not occur: it did not fail,
    /// or failed another way. `failed` (RFC 0032: "the failure does not reproduce on the
    /// base snapshot").
    NotReproduced {
        /// The run's `ev_` record.
        run: EvidenceRef,
        /// Whether the journal differs from the recorded one.
        journal_differs: bool,
        /// How the failure compares: [`FailureMatch::Absent`] or
        /// [`FailureMatch::Different`].
        failure: FailureMatch,
    },
    /// The base reproduced the recorded failure identity, but not the recorded run: the
    /// journal differs, or a recorded step was missing or out of order. The base, the
    /// crashpack and the schedule are all verified, so a deterministic replay cannot
    /// differ: the recorder or the replayer is inconsistent, an engine condition.
    /// `inconclusive`, INV-008 `EngineError`, with a `defect_` record.
    Inconsistent {
        /// The `defect_` record, which states the run.
        defect: EvidenceRef,
        /// Whether the journal differs from the recorded one.
        journal_differs: bool,
        /// Recorded steps the base did not have.
        eliminated: u64,
        /// Steps the base ran out of the recorded order.
        reordered: u64,
    },
    /// The base could not follow its own recorded schedule: an engine condition
    /// (`ReplayDiverged`), never a statement about the patch. `inconclusive`, INV-008
    /// `EngineError`, with a `defect_` record.
    Diverged {
        /// Recorded steps followed.
        at: u64,
        /// The `defect_` record.
        defect: EvidenceRef,
    },
    /// The replayer cannot run the base. `inconclusive`, INV-008 `Unsupported`, with an
    /// `ev_` record that names the replayer and the reason.
    Unsupported {
        /// The `ev_` record.
        record: EvidenceRef,
    },
}

impl BaseReplay {
    /// The gate status this outcome records.
    #[must_use]
    pub const fn status(&self) -> GateStatus {
        match self {
            Self::Reproduced { .. } => GateStatus::Passed,
            Self::NotReproduced { .. } => GateStatus::Failed,
            Self::Inconsistent { .. } | Self::Diverged { .. } | Self::Unsupported { .. } => {
                GateStatus::Inconclusive
            }
        }
    }

    /// The typed INV-008 reason of an `inconclusive` outcome.
    #[must_use]
    pub const fn inconclusive_reason(&self) -> Option<InconclusiveReason> {
        match self {
            Self::Reproduced { .. } | Self::NotReproduced { .. } => None,
            Self::Inconsistent { .. } | Self::Diverged { .. } => {
                Some(InconclusiveReason::EngineError)
            }
            Self::Unsupported { .. } => Some(InconclusiveReason::Unsupported),
        }
    }

    /// The evidence record this outcome cites, besides the crashpack.
    #[must_use]
    pub const fn record(&self) -> &EvidenceRef {
        match self {
            Self::Reproduced { run } | Self::NotReproduced { run, .. } => run,
            Self::Inconsistent { defect, .. } | Self::Diverged { defect, .. } => defect,
            Self::Unsupported { record } => record,
        }
    }
}

/// Gate 4's outcome.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExactRegression {
    /// The candidate ran under the original choices and did not fail, and the
    /// evaluation policy admits the run: it followed the recording exactly, or a checked
    /// [`PolicyAuthorization`] selected [`EvaluationPolicy::ContinueAndDisclose`] and the
    /// counts are disclosed. `passed`.
    NoLongerFails {
        /// The run's `ev_` record, which lists the eliminated positions.
        run: EvidenceRef,
        /// Recorded steps the candidate does not have, continued past and disclosed.
        eliminated: u64,
        /// Steps the candidate ran out of the recorded order, disclosed.
        reordered: u64,
    },
    /// The candidate ran under the original choices and failed with the recorded failure
    /// identity. `failed`.
    Recurs {
        /// The run's `ev_` record.
        run: EvidenceRef,
    },
    /// The candidate ran under the original choices and failed with another failure
    /// identity. `failed` (see the module's open questions).
    FailsDifferently {
        /// The run's `ev_` record.
        run: EvidenceRef,
    },
    /// The candidate could not follow the original choices. A divergence is not a pass.
    /// `inconclusive`, INV-008 `AbstractionAmbiguity`: the recorded choices do not
    /// determine the candidate's run.
    Diverged {
        /// Recorded steps followed.
        at: u64,
        /// The divergence's `ev_` record.
        run: EvidenceRef,
    },
    /// The candidate did not fail, but it did not follow the recording exactly: recorded
    /// steps were eliminated or reordered, and the evaluation policy is
    /// [`EvaluationPolicy::Exact`], which admits no such run. A repair may have moved or
    /// removed the causal events until the failure disappeared, so the run is not
    /// evidence that it is gone. `inconclusive`, INV-008 `AbstractionAmbiguity`.
    NotExact {
        /// The run's `ev_` record, which lists the eliminated positions.
        run: EvidenceRef,
        /// Recorded steps the candidate does not have.
        eliminated: u64,
        /// Steps the candidate ran out of the recorded order.
        reordered: u64,
    },
    /// The replayer cannot run the candidate. `inconclusive`, INV-008 `Unsupported`, with
    /// an `ev_` record that names the replayer and the reason.
    Unsupported {
        /// The `ev_` record.
        record: EvidenceRef,
    },
    /// Gate 1 did not pass, so there is no reproduced failure to regress against and the
    /// candidate is not run. Nothing is recorded: gate 4 stays `pending`, because it was
    /// not evaluated.
    NotRun,
}

impl ExactRegression {
    /// The gate status this outcome leaves on the version.
    #[must_use]
    pub const fn status(&self) -> GateStatus {
        match self {
            Self::NoLongerFails { .. } => GateStatus::Passed,
            Self::Recurs { .. } | Self::FailsDifferently { .. } => GateStatus::Failed,
            Self::Diverged { .. } | Self::NotExact { .. } | Self::Unsupported { .. } => {
                GateStatus::Inconclusive
            }
            Self::NotRun => GateStatus::Pending,
        }
    }

    /// The typed INV-008 reason of an `inconclusive` outcome.
    #[must_use]
    pub const fn inconclusive_reason(&self) -> Option<InconclusiveReason> {
        match self {
            Self::NoLongerFails { .. }
            | Self::Recurs { .. }
            | Self::FailsDifferently { .. }
            | Self::NotRun => None,
            Self::Diverged { .. } | Self::NotExact { .. } => {
                Some(InconclusiveReason::AbstractionAmbiguity)
            }
            Self::Unsupported { .. } => Some(InconclusiveReason::Unsupported),
        }
    }

    /// The evidence record this outcome cites, besides the crashpack; none when gate 4
    /// did not run.
    #[must_use]
    pub const fn record(&self) -> Option<&EvidenceRef> {
        match self {
            Self::NoLongerFails { run, .. }
            | Self::Recurs { run }
            | Self::FailsDifferently { run }
            | Self::NotExact { run, .. }
            | Self::Diverged { run, .. } => Some(run),
            Self::Unsupported { record } => Some(record),
            Self::NotRun => None,
        }
    }
}

/// The evaluation policy the gate campaign runs under (RFC 0032: gate 4 "continues
/// under the transaction's declared `evaluation_policy`"). It is recorded as the new
/// version's `evaluation_policy`, so it is part of the version's identity, and in every
/// evidence record the evaluation makes. A caller does not choose it: [`evaluate`] runs
/// under [`Self::Exact`] unless a checked [`PolicyAuthorization`] selects
/// [`Self::ContinueAndDisclose`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EvaluationPolicy {
    /// The candidate must follow the recording exactly for gate 4 to pass: a clean run
    /// with any eliminated or reordered step is [`ExactRegression::NotExact`]. The
    /// fail-closed default.
    Exact,
    /// A clean run with eliminated or reordered steps passes gate 4, and the positions
    /// and counts are disclosed in the gate's `ev_` record: the policy RFC 0032's "report
    /// where replay diverges and continue" describes. Reachable only through a
    /// [`PolicyAuthorization`], which nothing in production issues yet (bn-2vanm,
    /// bn-b6u4).
    ContinueAndDisclose,
}

impl EvaluationPolicy {
    /// The token recorded in `evaluation_policy` and in the evidence records.
    #[must_use]
    pub const fn token(self) -> &'static str {
        match self {
            Self::Exact => "exact-replay/exact/1",
            Self::ContinueAndDisclose => "exact-replay/continue-and-disclose/1",
        }
    }
}

/// Domain tag of an authorization's bound record.
const AUTHORIZATION_TAG: &[u8] = b"continuum-repair/policy-authorization";

/// An authorization to evaluate one transaction version under
/// [`EvaluationPolicy::ContinueAndDisclose`].
///
/// It is bound to the version it authorizes: the version's `rt_` identity (so it is
/// stale once the version is superseded), its base and candidate snapshots, and its base
/// intent, the `in_` content identity of the protected Intent Contract. Its digest is
/// taken over those fields and the policy, and [`evaluate`] re-derives and compares
/// every one of them before it uses the authorization ([`AuthorizationMismatch`]).
///
/// **Nothing in production issues one.** The constructor is private to this crate, and
/// the public seam, [`request_continue_and_disclose`], answers
/// [`AuthorizationAbsence::NoAuthority`] today. Who may authorize, and on what evidence,
/// is owned by bn-2vanm and bn-b6u4. A value cannot be built outside the crate:
///
/// ```compile_fail
/// use continuum_repair::replay::PolicyAuthorization;
/// let forged = PolicyAuthorization { digest: String::new() };
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PolicyAuthorization {
    policy: EvaluationPolicy,
    repair_id: RepairId,
    base_snapshot: SnapshotId,
    candidate_snapshot: SnapshotId,
    base_intent: IntentId,
    digest: String,
}

/// Why no authorization was issued.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthorizationAbsence {
    /// No authority exists that may authorize a relaxed evaluation policy (bn-2vanm,
    /// bn-b6u4). Every evaluation runs under [`EvaluationPolicy::Exact`].
    NoAuthority,
}

/// The policy-verdict seam that would issue a [`PolicyAuthorization`] for
/// `transaction`. There is no authority yet, so it answers
/// [`AuthorizationAbsence::NoAuthority`], never an authorization.
///
/// # Errors
///
/// Always [`AuthorizationAbsence::NoAuthority`] today.
pub const fn request_continue_and_disclose<H: ContentHasher>(
    transaction: &RepairTransaction<H>,
) -> Result<PolicyAuthorization, AuthorizationAbsence> {
    // The policy layer is the only place an authorization may be issued, and it issues
    // one only on a protected policy grant, which no RFC defines yet (bn-b6u4, bn-2vanm).
    crate::policy::authorize_continue_and_disclose(transaction, None)
}

/// Which bound field of an authorization does not match the transaction evaluated.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthorizationMismatch {
    /// It authorizes another policy than [`EvaluationPolicy::ContinueAndDisclose`].
    Policy,
    /// It authorizes another transaction version: a different or superseded `rt_`.
    Transaction,
    /// It is bound to another base snapshot.
    BaseSnapshot,
    /// It is bound to another candidate snapshot, or the version has none.
    CandidateSnapshot,
    /// It is bound to another base intent.
    Intent,
    /// Its digest is not the digest of its bound fields under the lineage's hasher.
    Digest,
}

impl PolicyAuthorization {
    /// Issue an authorization for `transaction` under `ContinueAndDisclose`. Crate-private
    /// and test-only: no production path issues one. `None` when the version has no
    /// candidate.
    #[cfg(test)]
    pub(crate) fn issue<H: ContentHasher>(transaction: &RepairTransaction<H>) -> Option<Self> {
        let mut authorization = Self {
            policy: EvaluationPolicy::ContinueAndDisclose,
            repair_id: transaction.repair_id().clone(),
            base_snapshot: transaction.base_snapshot().clone(),
            candidate_snapshot: transaction.candidate_snapshot()?.clone(),
            base_intent: transaction.base_intent().clone(),
            digest: String::new(),
        };
        authorization.digest = authorization.derive_digest::<H>();
        Some(authorization)
    }

    /// The canonical bytes of the bound fields.
    fn bound_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(AUTHORIZATION_TAG);
        bytes.push(REPLAY_ENCODING_VERSION);
        field(&mut bytes, self.policy.token().as_bytes());
        field(&mut bytes, self.repair_id.as_str().as_bytes());
        field(&mut bytes, self.base_snapshot.as_str().as_bytes());
        field(&mut bytes, self.candidate_snapshot.as_str().as_bytes());
        field(&mut bytes, self.base_intent.as_str().as_bytes());
        bytes
    }

    fn derive_digest<H: ContentHasher>(&self) -> String {
        digest::<H>(&self.bound_bytes())
    }

    /// The authorization's digest, `<algorithm>:<hex>`.
    #[must_use]
    pub fn digest(&self) -> &str {
        &self.digest
    }

    /// Check every bound field against `transaction`, the policy first and the digest
    /// last.
    fn check<H: ContentHasher>(
        &self,
        transaction: &RepairTransaction<H>,
    ) -> Result<(), AuthorizationMismatch> {
        if self.policy != EvaluationPolicy::ContinueAndDisclose {
            return Err(AuthorizationMismatch::Policy);
        }
        if &self.repair_id != transaction.repair_id() {
            return Err(AuthorizationMismatch::Transaction);
        }
        if &self.base_snapshot != transaction.base_snapshot() {
            return Err(AuthorizationMismatch::BaseSnapshot);
        }
        if transaction.candidate_snapshot() != Some(&self.candidate_snapshot) {
            return Err(AuthorizationMismatch::CandidateSnapshot);
        }
        if &self.base_intent != transaction.base_intent() {
            return Err(AuthorizationMismatch::Intent);
        }
        if self.digest != self.derive_digest::<H>() {
            return Err(AuthorizationMismatch::Digest);
        }
        Ok(())
    }
}

/// An evidence record this evaluation made: its handle and its canonical bytes, for the
/// daemon to publish. Its handle is the digest of its bytes under the lineage's hasher.
#[derive(Clone, PartialEq, Eq)]
pub struct EvidenceRecord {
    handle: EvidenceRef,
    bytes: Vec<u8>,
}

impl fmt::Debug for EvidenceRecord {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("EvidenceRecord")
            .field("handle", &self.handle)
            .field("bytes", &format_args!("<{} bytes>", self.bytes.len()))
            .finish()
    }
}

impl EvidenceRecord {
    /// The handle a gate cites.
    #[must_use]
    pub const fn handle(&self) -> &EvidenceRef {
        &self.handle
    }

    /// The record's canonical bytes.
    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// What the record states about the gate it supports: the gate, the status and the
    /// typed INV-008 reason. Read by the policy verdict ([`crate::policy`]).
    ///
    /// `None` unless the bytes re-derive to the handle under `H` first, so a record
    /// whose bytes were altered states nothing, and unless the prefix up to the reason
    /// parses under this module's grammar. Only that prefix is read.
    pub(crate) fn statement<H: ContentHasher>(
        &self,
    ) -> Option<(GateName, GateStatus, Option<InconclusiveReason>)> {
        let (class, tag) = [RUN, DEFECT]
            .into_iter()
            .find(|(_, tag)| self.bytes.starts_with(tag))?;
        if self.handle.as_str() != format!("{class}_{}", H::hash(&self.bytes).to_token()) {
            return None;
        }
        let mut reader = Reader(self.bytes.get(tag.len()..)?);
        if reader.take(1).ok()? != [REPLAY_ENCODING_VERSION] {
            return None;
        }
        let gate = reader.field().ok()?;
        // The crashpack, the program, the replayer and the policy.
        for _ in 0..4 {
            reader.field().ok()?;
        }
        let status = reader.field().ok()?;
        let reason = reader.field().ok()?;
        let gate = GateName::ALL
            .into_iter()
            .find(|name| name.token().as_bytes() == gate)?;
        let status = GateStatus::ALL
            .into_iter()
            .find(|candidate| candidate.token().as_bytes() == status)?;
        let reason = if reason.is_empty() {
            None
        } else {
            Some(
                InconclusiveReason::ALL
                    .into_iter()
                    .find(|candidate| candidate.as_str().as_bytes() == reason)?,
            )
        };
        Some((gate, status, reason))
    }
}

/// Why no replay was evaluated. Nothing was recorded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExactReplayRefusal {
    /// The version is a `draft`: there is no candidate (RFC 0032: `InsufficientEvidence`).
    NoCandidate,
    /// Gate 1 or gate 4 is not `pending` on this version: it already has an outcome.
    NotPending {
        /// The gate.
        gate: GateName,
    },
    /// The supplied crashpack bytes do not name the transaction's `failure`: they were
    /// altered, or are another crashpack's. The derived identity is not echoed.
    CrashpackTampered {
        /// The crashpack the transaction names.
        expected: CrashpackId,
    },
    /// The crashpack's bytes are not a recorded run.
    CrashpackMalformed(MalformedRecord),
    /// The crashpack was recorded on another snapshot than the transaction's base.
    ForeignCrashpack {
        /// The snapshot the crashpack names.
        recorded_on: SnapshotId,
    },
    /// The supplied base content is not the transaction's `base_snapshot`.
    BaseMismatch {
        /// The frozen base.
        expected: SnapshotId,
        /// What the content derives to.
        found: SnapshotId,
    },
    /// The supplied candidate content is not the transaction's `candidate_snapshot`.
    CandidateMismatch {
        /// The sealed candidate.
        expected: SnapshotId,
        /// What the content derives to.
        found: SnapshotId,
    },
    /// A supplied content is not a workspace tree, or its identity could not be derived.
    Tree(SnapshotError),
    /// The version was already evaluated under another evaluation policy.
    PolicyMismatch,
    /// The supplied [`PolicyAuthorization`] is not bound to this version: another or a
    /// superseded transaction, another snapshot or intent, or a digest that does not
    /// match its fields. Nothing falls back to a pass.
    Authorization(AuthorizationMismatch),
    /// The version cannot advance.
    VersionExhausted,
}

impl fmt::Display for ExactReplayRefusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoCandidate => f.write_str("a draft transaction has no candidate to evaluate"),
            Self::NotPending { gate } => {
                write!(f, "gate `{}` is not pending on this version", gate.token())
            }
            Self::CrashpackTampered { expected } => {
                write!(f, "the supplied crashpack bytes do not name {expected}")
            }
            Self::CrashpackMalformed(error) => write!(f, "the crashpack is malformed: {error}"),
            Self::ForeignCrashpack { recorded_on } => write!(
                f,
                "the crashpack was recorded on {recorded_on}, not on the transaction's base"
            ),
            Self::BaseMismatch { expected, found } => {
                write!(f, "the base content is {found}, not {expected}")
            }
            Self::CandidateMismatch { expected, found } => {
                write!(f, "the candidate content is {found}, not {expected}")
            }
            Self::Tree(error) => write!(f, "a supplied content is not a workspace tree: {error}"),
            Self::PolicyMismatch => {
                f.write_str("the version was evaluated under another evaluation policy")
            }
            Self::Authorization(mismatch) => write!(
                f,
                "the policy authorization is not bound to this transaction: {mismatch:?}"
            ),
            Self::VersionExhausted => f.write_str("the transaction version cannot advance"),
        }
    }
}

impl std::error::Error for ExactReplayRefusal {}

/// An exact-replay evaluation: the new version with gates 1 and 4 recorded, the two
/// outcomes, and the evidence records the gates cite.
pub struct ExactReplay<H: ContentHasher> {
    transaction: RepairTransaction<H>,
    base_replay: BaseReplay,
    exact_regression: ExactRegression,
    records: Vec<EvidenceRecord>,
}

impl<H: ContentHasher> fmt::Debug for ExactReplay<H> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ExactReplay")
            .field("transaction", &self.transaction)
            .field("base_replay", &self.base_replay)
            .field("exact_regression", &self.exact_regression)
            .field("records", &self.records)
            .finish()
    }
}

impl<H: ContentHasher> ExactReplay<H> {
    /// The new version: gates 1 and 4 moved from `pending` to their outcomes.
    #[must_use]
    pub const fn transaction(&self) -> &RepairTransaction<H> {
        &self.transaction
    }

    /// Gate 1's outcome.
    #[must_use]
    pub const fn base_replay(&self) -> &BaseReplay {
        &self.base_replay
    }

    /// Gate 4's outcome.
    #[must_use]
    pub const fn exact_regression(&self) -> &ExactRegression {
        &self.exact_regression
    }

    /// The `ev_` and `defect_` records the gates cite, in the order they were made.
    #[must_use]
    pub fn records(&self) -> &[EvidenceRecord] {
        &self.records
    }

    /// The new version and the records its gates cite. There is no way to keep the
    /// version and drop the records: a gate that cites an `ev_` must have it to publish.
    #[must_use]
    pub fn into_parts(self) -> (RepairTransaction<H>, Vec<EvidenceRecord>) {
        (self.transaction, self.records)
    }
}

// --- the evaluation ------------------------------------------------------------------

/// Evaluate gates 1 and 4 of `transaction` and record them as its next version.
///
/// `crashpack` is the stored content of the transaction's `failure`; `base` and
/// `candidate` are the contents of its `base_snapshot` and `candidate_snapshot`. None is
/// trusted: each is re-derived to its identity and compared, in that order, before any
/// replay runs. Then the recorded schedule is replayed on the base (gate 1) and, only
/// when the base reproduces the exact failure, on the candidate (gate 4).
///
/// The policy is [`EvaluationPolicy::Exact`] when `authorization` is `None`. An
/// authorization is checked against the transaction before any replay runs, and only a
/// matching one selects [`EvaluationPolicy::ContinueAndDisclose`].
///
/// # Errors
///
/// [`ExactReplayRefusal`]; a refusal records nothing.
pub fn evaluate<H: ContentHasher>(
    transaction: &RepairTransaction<H>,
    crashpack: &[u8],
    base: &WorkspaceContent,
    candidate: &WorkspaceContent,
    replayer: &impl Replayer,
    authorization: Option<&PolicyAuthorization>,
) -> Result<ExactReplay<H>, ExactReplayRefusal> {
    let Some(candidate_snapshot) = transaction.candidate_snapshot() else {
        return Err(ExactReplayRefusal::NoCandidate);
    };
    for name in [GateName::BaseReplay, GateName::ExactRegression] {
        let pending = transaction
            .gates()
            .iter()
            .any(|gate| gate.name() == name && gate.status() == GateStatus::Pending);
        if !pending {
            return Err(ExactReplayRefusal::NotPending { gate: name });
        }
    }
    // The identity first: one linear pass over the bytes, and nothing is decoded from
    // bytes that do not name the transaction's crashpack.
    if &crashpack_id::<H>(crashpack) != transaction.failure() {
        return Err(ExactReplayRefusal::CrashpackTampered {
            expected: transaction.failure().clone(),
        });
    }
    let recorded =
        RecordedRun::decode(crashpack).map_err(ExactReplayRefusal::CrashpackMalformed)?;
    if recorded.base_snapshot() != transaction.base_snapshot() {
        return Err(ExactReplayRefusal::ForeignCrashpack {
            recorded_on: recorded.base_snapshot().clone(),
        });
    }
    let found = content_id::<H>(base)?;
    if &found != transaction.base_snapshot() {
        return Err(ExactReplayRefusal::BaseMismatch {
            expected: transaction.base_snapshot().clone(),
            found,
        });
    }
    let found = content_id::<H>(candidate)?;
    if &found != candidate_snapshot {
        return Err(ExactReplayRefusal::CandidateMismatch {
            expected: candidate_snapshot.clone(),
            found,
        });
    }

    let (policy, recorded_policy) = match authorization {
        None => (
            EvaluationPolicy::Exact,
            EvaluationPolicy::Exact.token().to_owned(),
        ),
        Some(authorization) => {
            authorization
                .check(transaction)
                .map_err(ExactReplayRefusal::Authorization)?;
            (
                authorization.policy,
                format!(
                    "{};authorization={}",
                    authorization.policy.token(),
                    authorization.digest()
                ),
            )
        }
    };

    let crash = EvidenceRef::new(transaction.failure().as_str())
        .unwrap_or_else(|_| unreachable!("a crash_ handle is an evidence handle"));
    let mut records = Vec::new();
    let mut made = Records::<H> {
        crash: transaction.failure(),
        replayer: replayer.identity(),
        policy: &recorded_policy,
        out: &mut records,
        hasher: PhantomData,
    };

    let base_snapshot = transaction.base_snapshot();
    let base_replay = match replayer.replay(base, recorded.schedule()) {
        ReplayOutcome::Ran(run) => {
            let journal_differs = run.journal() != recorded.journal();
            let answer = Answer::Ran(&run);
            match compare(run.failure(), recorded.failure()) {
                FailureMatch::Same if !journal_differs && run.follows_exactly() => {
                    BaseReplay::Reproduced {
                        run: made.record(
                            RUN,
                            GateName::BaseReplay,
                            base_snapshot,
                            &answer,
                            GateStatus::Passed,
                            None,
                        ),
                    }
                }
                FailureMatch::Same => BaseReplay::Inconsistent {
                    defect: made.record(
                        DEFECT,
                        GateName::BaseReplay,
                        base_snapshot,
                        &answer,
                        GateStatus::Inconclusive,
                        Some(InconclusiveReason::EngineError),
                    ),
                    journal_differs,
                    eliminated: count(run.eliminated().len()),
                    reordered: run.reordered(),
                },
                failure => BaseReplay::NotReproduced {
                    run: made.record(
                        RUN,
                        GateName::BaseReplay,
                        base_snapshot,
                        &answer,
                        GateStatus::Failed,
                        None,
                    ),
                    journal_differs,
                    failure,
                },
            }
        }
        ReplayOutcome::Diverged(divergence) => BaseReplay::Diverged {
            at: divergence.at,
            defect: made.record(
                DEFECT,
                GateName::BaseReplay,
                base_snapshot,
                &Answer::Diverged(divergence),
                GateStatus::Inconclusive,
                Some(InconclusiveReason::EngineError),
            ),
        },
        ReplayOutcome::Unsupported => BaseReplay::Unsupported {
            record: made.record(
                RUN,
                GateName::BaseReplay,
                base_snapshot,
                &Answer::Unsupported,
                GateStatus::Inconclusive,
                Some(InconclusiveReason::Unsupported),
            ),
        },
    };

    let gate = GateName::ExactRegression;
    let exact_regression = if matches!(base_replay, BaseReplay::Reproduced { .. }) {
        match replayer.replay(candidate, recorded.schedule()) {
            ReplayOutcome::Ran(run) => {
                let answer = Answer::Ran(&run);
                let eliminated = count(run.eliminated().len());
                let reordered = run.reordered();
                match compare(run.failure(), recorded.failure()) {
                    FailureMatch::Absent
                        if run.follows_exactly()
                            || policy == EvaluationPolicy::ContinueAndDisclose =>
                    {
                        ExactRegression::NoLongerFails {
                            run: made.record(
                                RUN,
                                gate,
                                candidate_snapshot,
                                &answer,
                                GateStatus::Passed,
                                None,
                            ),
                            eliminated,
                            reordered,
                        }
                    }
                    FailureMatch::Absent => ExactRegression::NotExact {
                        run: made.record(
                            RUN,
                            gate,
                            candidate_snapshot,
                            &answer,
                            GateStatus::Inconclusive,
                            Some(InconclusiveReason::AbstractionAmbiguity),
                        ),
                        eliminated,
                        reordered,
                    },
                    FailureMatch::Same => ExactRegression::Recurs {
                        run: made.record(
                            RUN,
                            gate,
                            candidate_snapshot,
                            &answer,
                            GateStatus::Failed,
                            None,
                        ),
                    },
                    FailureMatch::Different => ExactRegression::FailsDifferently {
                        run: made.record(
                            RUN,
                            gate,
                            candidate_snapshot,
                            &answer,
                            GateStatus::Failed,
                            None,
                        ),
                    },
                }
            }
            ReplayOutcome::Diverged(divergence) => ExactRegression::Diverged {
                at: divergence.at,
                run: made.record(
                    RUN,
                    gate,
                    candidate_snapshot,
                    &Answer::Diverged(divergence),
                    GateStatus::Inconclusive,
                    Some(InconclusiveReason::AbstractionAmbiguity),
                ),
            },
            ReplayOutcome::Unsupported => ExactRegression::Unsupported {
                record: made.record(
                    RUN,
                    gate,
                    candidate_snapshot,
                    &Answer::Unsupported,
                    GateStatus::Inconclusive,
                    Some(InconclusiveReason::Unsupported),
                ),
            },
        }
    } else {
        ExactRegression::NotRun
    };

    let cite = |record: &EvidenceRef| vec![crash.clone(), record.clone()];
    let mut outcomes = vec![(
        GateName::BaseReplay,
        base_replay.status(),
        cite(base_replay.record()),
    )];
    if let Some(record) = exact_regression.record() {
        outcomes.push((gate, exact_regression.status(), cite(record)));
    }
    let transaction = transaction
        .record_outcomes(outcomes, &recorded_policy)
        .map_err(|refusal| match refusal {
            RecordRefusal::NoCandidate => ExactReplayRefusal::NoCandidate,
            RecordRefusal::NotPending { gate } | RecordRefusal::NotAnOutcome { gate } => {
                ExactReplayRefusal::NotPending { gate }
            }
            RecordRefusal::PolicyMismatch => ExactReplayRefusal::PolicyMismatch,
            RecordRefusal::VersionExhausted => ExactReplayRefusal::VersionExhausted,
        })?;
    Ok(ExactReplay {
        transaction,
        base_replay,
        exact_regression,
        records,
    })
}

fn count(n: usize) -> u64 {
    u64::try_from(n).unwrap_or(u64::MAX)
}

fn compare(run: Option<&[u8]>, recorded: &[u8]) -> FailureMatch {
    match run {
        None => FailureMatch::Absent,
        Some(failure) if failure == recorded => FailureMatch::Same,
        Some(_) => FailureMatch::Different,
    }
}

fn content_id<H: ContentHasher>(
    content: &WorkspaceContent,
) -> Result<SnapshotId, ExactReplayRefusal> {
    let tree = Snapshot::build(content, &HashedIdentifier::<H>::new())
        .map_err(ExactReplayRefusal::Tree)?;
    SnapshotId::new(&tree.identity().to_string())
        .map_err(|_| ExactReplayRefusal::Tree(SnapshotError::IdentityUnavailable { path: None }))
}

/// What a replay answered, as an evidence record states it.
enum Answer<'a> {
    Ran(&'a ReplayRun),
    Diverged(Divergence),
    Unsupported,
}

/// The two record classes an evaluation makes.
const RUN: (&str, &[u8]) = ("ev", REPLAY_RUN_TAG);
const DEFECT: (&str, &[u8]) = ("defect", DEFECT_TAG);

/// The records one evaluation makes, in order.
struct Records<'a, H: ContentHasher> {
    crash: &'a CrashpackId,
    replayer: &'a str,
    policy: &'a str,
    out: &'a mut Vec<EvidenceRecord>,
    hasher: PhantomData<fn() -> H>,
}

impl<H: ContentHasher> Records<'_, H> {
    /// An `ev_` replay-run record or a `defect_` engine-condition record: which gate,
    /// which crashpack, which program, which replayer under which evaluation policy, the
    /// gate status and typed INV-008 reason it supports, and what the replay answered. A
    /// run is stated by the digests of its journal and failure identity (spelled
    /// `<algorithm>:<hex>`), the positions of the recorded steps it eliminated, and the
    /// count of steps it reordered. Every variable field is length-prefixed and the two
    /// answers have distinct discriminants, so distinct statements have distinct bytes.
    fn record(
        &mut self,
        (class, tag): (&str, &[u8]),
        gate: GateName,
        program: &SnapshotId,
        answer: &Answer<'_>,
        status: GateStatus,
        reason: Option<InconclusiveReason>,
    ) -> EvidenceRef {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(tag);
        bytes.push(REPLAY_ENCODING_VERSION);
        field(&mut bytes, gate.token().as_bytes());
        field(&mut bytes, self.crash.as_str().as_bytes());
        field(&mut bytes, program.as_str().as_bytes());
        field(&mut bytes, self.replayer.as_bytes());
        field(&mut bytes, self.policy.as_bytes());
        field(&mut bytes, status.token().as_bytes());
        field(
            &mut bytes,
            reason.map_or("", InconclusiveReason::as_str).as_bytes(),
        );
        match answer {
            Answer::Ran(run) => {
                bytes.push(0);
                field(&mut bytes, digest::<H>(run.journal()).as_bytes());
                match run.failure() {
                    None => bytes.push(0),
                    Some(failure) => {
                        bytes.push(1);
                        field(&mut bytes, digest::<H>(failure).as_bytes());
                    }
                }
                bytes.extend_from_slice(&count(run.eliminated().len()).to_be_bytes());
                for position in run.eliminated() {
                    bytes.extend_from_slice(&position.to_be_bytes());
                }
                bytes.extend_from_slice(&run.reordered().to_be_bytes());
            }
            Answer::Diverged(divergence) => {
                bytes.push(1);
                bytes.extend_from_slice(&divergence.at.to_be_bytes());
            }
            Answer::Unsupported => bytes.push(2),
        }
        let handle = EvidenceRef::new(&format!("{class}_{}", H::hash(&bytes).to_token()))
            .unwrap_or_else(|_| unreachable!("a class and a hex token make an evidence handle"));
        self.out.push(EvidenceRecord {
            handle: handle.clone(),
            bytes,
        });
        handle
    }
}

fn digest<H: ContentHasher>(bytes: &[u8]) -> String {
    format!("{}:{}", H::ALGORITHM.token(), H::hash(bytes).to_token())
}

#[cfg(test)]
mod tests {
    //! The authorized path, [`EvaluationPolicy::ContinueAndDisclose`], which only this
    //! crate's tests can reach: [`PolicyAuthorization::issue`] is crate-private and
    //! test-only, and no production path issues an authorization (bn-2vanm, bn-b6u4).

    use super::*;
    use crate::hypothesis::{ChangeKind, Hypothesis, Proposal};
    use crate::patch::{DeclaredChange, FileEdit};
    use crate::transaction::{FailureBinding, GateProfile, Resolution};
    use continuum_value::identity::Blake3Hasher;

    type Tx = RepairTransaction<Blake3Hasher>;

    const REPLICA: &str = "src/replica.rs";
    const SCHEDULE: &[u8] = b"step-0\nstep-1\n";
    const JOURNAL: &[u8] = b"journal: ack v0; lose v0; ack v1";
    const FAILURE: &[u8] = b"Agreement(41)";

    fn content(replica: &str) -> WorkspaceContent {
        let mut content = WorkspaceContent::new();
        content
            .insert(
                WorkspacePath::new(REPLICA).unwrap(),
                replica.as_bytes().to_vec(),
            )
            .unwrap();
        content
    }

    fn id(content: &WorkspaceContent) -> SnapshotId {
        content_id::<Blake3Hasher>(content).unwrap()
    }

    fn crashpack() -> RecordedRun {
        RecordedRun::new(
            id(&content("ack; sync")),
            SCHEDULE.to_vec(),
            JOURNAL.to_vec(),
            FAILURE.to_vec(),
        )
        .unwrap()
    }

    struct Binding {
        failure: CrashpackId,
        intent: &'static str,
    }

    impl FailureBinding for Binding {
        fn failure_base(&self, failure: &CrashpackId) -> Resolution<SnapshotId> {
            if *failure == self.failure {
                Resolution::Found(id(&content("ack; sync")))
            } else {
                Resolution::Unknown
            }
        }

        fn snapshot_intent(&self, _: &SnapshotId) -> Resolution<IntentId> {
            Resolution::Found(IntentId::new(self.intent).unwrap())
        }
    }

    /// The transaction applied with the replica replaced by `after`, under `intent`.
    fn applied(after: &str, intent: &'static str) -> Tx {
        let failure = crashpack().identity::<Blake3Hasher>();
        let draft = Tx::begin(
            failure.clone(),
            GateProfile::PhaseB,
            &Binding { failure, intent },
        )
        .unwrap();
        let base = content("ack; sync");
        let tree = Snapshot::build(&base, &HashedIdentifier::<Blake3Hasher>::new()).unwrap();
        let path = WorkspacePath::new(REPLICA).unwrap();
        let before = SnapshotId::new(
            &tree
                .node(&path)
                .and_then(|node| node.as_file())
                .unwrap()
                .identity()
                .to_string(),
        )
        .unwrap();
        let change = DeclaredChange::new(
            ChangeKind::Rust,
            [(
                path,
                FileEdit::Replace {
                    before,
                    content: after.as_bytes().to_vec(),
                },
            )],
        )
        .unwrap();
        let proposal = Proposal::new(Hypothesis::new("move the ack"), vec![change]);
        draft.apply(&proposal, &base).unwrap().into_parts().0
    }

    /// Replays the base exactly and the candidate cleanly, with steps eliminated and
    /// reordered.
    struct Replay;

    impl Replayer for Replay {
        fn identity(&self) -> &str {
            "unit/1"
        }

        fn replay(&self, program: &WorkspaceContent, _: &[u8]) -> ReplayOutcome {
            if *program == content("ack; sync") {
                ReplayOutcome::Ran(ReplayRun::new(
                    JOURNAL.to_vec(),
                    Some(FAILURE.to_vec()),
                    Vec::new(),
                    0,
                ))
            } else {
                ReplayOutcome::Ran(ReplayRun::new(b"clean".to_vec(), None, vec![1], 2))
            }
        }
    }

    fn evaluate_with(
        tx: &Tx,
        after: &str,
        authorization: Option<&PolicyAuthorization>,
    ) -> Result<ExactReplay<Blake3Hasher>, ExactReplayRefusal> {
        evaluate(
            tx,
            &crashpack().canonical_bytes(),
            &content("ack; sync"),
            &content(after),
            &Replay,
            authorization,
        )
    }

    #[test]
    fn an_authorization_bound_to_the_version_admits_continue_and_disclose() {
        let tx = applied("sync; ack", "in_register");
        let authorization = PolicyAuthorization::issue(&tx).unwrap();
        let result = evaluate_with(&tx, "sync; ack", Some(&authorization)).unwrap();
        assert!(matches!(
            result.exact_regression(),
            ExactRegression::NoLongerFails {
                eliminated: 1,
                reordered: 2,
                ..
            }
        ));
        let recorded = result.transaction().evaluation_policy().unwrap();
        assert_eq!(
            recorded,
            format!(
                "exact-replay/continue-and-disclose/1;authorization={}",
                authorization.digest()
            )
        );
        assert!(authorization.digest().starts_with("blake3-256:"));
        // Every record the evaluation made binds the authorized policy.
        for record in result.records() {
            assert!(
                record
                    .bytes()
                    .windows(recorded.len())
                    .any(|w| w == recorded.as_bytes())
            );
        }
    }

    #[test]
    fn without_an_authorization_the_policy_is_exact_and_the_run_is_not_passed() {
        let tx = applied("sync; ack", "in_register");
        let result = evaluate_with(&tx, "sync; ack", None).unwrap();
        assert!(matches!(
            result.exact_regression(),
            ExactRegression::NotExact { .. }
        ));
        assert_eq!(result.exact_regression().status(), GateStatus::Inconclusive);
        assert_eq!(
            result.transaction().evaluation_policy(),
            Some(EvaluationPolicy::Exact.token())
        );
        // The public seam issues nothing.
        assert_eq!(
            request_continue_and_disclose(&tx),
            Err(AuthorizationAbsence::NoAuthority)
        );
    }

    #[test]
    fn an_authorization_for_another_transaction_snapshot_or_intent_is_refused() {
        let tx = applied("sync; ack", "in_register");
        let refused = |authorization: &PolicyAuthorization| {
            evaluate_with(&tx, "sync; ack", Some(authorization)).map(|_| ())
        };
        // Another version: another candidate on the same draft.
        let other = applied("sync; ack; ack", "in_register");
        let foreign = PolicyAuthorization::issue(&other).unwrap();
        assert_eq!(
            refused(&foreign),
            Err(ExactReplayRefusal::Authorization(
                AuthorizationMismatch::Transaction
            ))
        );
        // Another intent: same content, another protected contract.
        let elsewhere = applied("sync; ack", "in_other_contract");
        assert_eq!(
            refused(&PolicyAuthorization::issue(&elsewhere).unwrap()),
            Err(ExactReplayRefusal::Authorization(
                AuthorizationMismatch::Transaction
            )),
            "the intent moves the rt_ identity, so the version differs first"
        );
        // Each bound field altered alone, the rest intact.
        let genuine = PolicyAuthorization::issue(&tx).unwrap();
        let mut base = genuine.clone();
        base.base_snapshot = id(&content("other"));
        assert_eq!(
            refused(&base),
            Err(ExactReplayRefusal::Authorization(
                AuthorizationMismatch::BaseSnapshot
            ))
        );
        let mut candidate = genuine.clone();
        candidate.candidate_snapshot = id(&content("other"));
        assert_eq!(
            refused(&candidate),
            Err(ExactReplayRefusal::Authorization(
                AuthorizationMismatch::CandidateSnapshot
            ))
        );
        let mut intent = genuine.clone();
        intent.base_intent = IntentId::new("in_other_contract").unwrap();
        assert_eq!(
            refused(&intent),
            Err(ExactReplayRefusal::Authorization(
                AuthorizationMismatch::Intent
            ))
        );
        let mut policy = genuine.clone();
        policy.policy = EvaluationPolicy::Exact;
        assert_eq!(
            refused(&policy),
            Err(ExactReplayRefusal::Authorization(
                AuthorizationMismatch::Policy
            ))
        );
        assert!(refused(&genuine).is_ok());
    }

    #[test]
    fn a_stale_or_tampered_authorization_is_refused() {
        let tx = applied("sync; ack", "in_register");
        let genuine = PolicyAuthorization::issue(&tx).unwrap();
        // Stale: the version it authorized has been superseded by a later version with
        // the same candidate and every gate pending again.
        let base = content("ack; sync");
        let tree = Snapshot::build(&base, &HashedIdentifier::<Blake3Hasher>::new()).unwrap();
        let path = WorkspacePath::new(REPLICA).unwrap();
        let before = SnapshotId::new(
            &tree
                .node(&path)
                .and_then(|node| node.as_file())
                .unwrap()
                .identity()
                .to_string(),
        )
        .unwrap();
        let change = DeclaredChange::new(
            ChangeKind::Rust,
            [(
                path,
                FileEdit::Replace {
                    before,
                    content: b"sync; ack".to_vec(),
                },
            )],
        )
        .unwrap();
        let proposal = Proposal::new(Hypothesis::new("move the ack"), vec![change]);
        let next = tx.apply(&proposal, &base).unwrap().into_parts().0;
        assert_eq!(next.candidate_snapshot(), tx.candidate_snapshot());
        assert_ne!(next.repair_id(), tx.repair_id());
        assert_eq!(
            evaluate_with(&next, "sync; ack", Some(&genuine)).map(|_| ()),
            Err(ExactReplayRefusal::Authorization(
                AuthorizationMismatch::Transaction
            ))
        );
        // Tampered: a digest that is not the digest of the bound fields.
        let mut forged = genuine.clone();
        forged.digest = format!("blake3-256:{}", "0".repeat(64));
        assert_eq!(
            evaluate_with(&tx, "sync; ack", Some(&forged)).map(|_| ()),
            Err(ExactReplayRefusal::Authorization(
                AuthorizationMismatch::Digest
            ))
        );
        let mut tampered = genuine;
        tampered.digest.push('0');
        assert_eq!(
            evaluate_with(&tx, "sync; ack", Some(&tampered)).map(|_| ()),
            Err(ExactReplayRefusal::Authorization(
                AuthorizationMismatch::Digest
            ))
        );
    }

    #[test]
    fn a_record_states_its_reason_only_when_its_bytes_name_it() {
        use crate::policy::{GateReasons, ReplayRecords};
        let crash = CrashpackId::new("crash_unit").unwrap();
        let program = SnapshotId::new("ws_unit").unwrap();
        let mut out = Vec::new();
        let (ambiguity, engine, passed) = {
            let mut made = Records::<Blake3Hasher> {
                crash: &crash,
                replayer: "unit/1",
                policy: EvaluationPolicy::Exact.token(),
                out: &mut out,
                hasher: PhantomData,
            };
            let gate = GateName::ExactRegression;
            (
                made.record(
                    RUN,
                    gate,
                    &program,
                    &Answer::Unsupported,
                    GateStatus::Inconclusive,
                    Some(InconclusiveReason::AbstractionAmbiguity),
                ),
                made.record(
                    DEFECT,
                    gate,
                    &program,
                    &Answer::Unsupported,
                    GateStatus::Inconclusive,
                    Some(InconclusiveReason::EngineError),
                ),
                made.record(
                    RUN,
                    gate,
                    &program,
                    &Answer::Unsupported,
                    GateStatus::Passed,
                    None,
                ),
            )
        };
        assert_eq!(
            out[0].statement::<Blake3Hasher>(),
            Some((
                GateName::ExactRegression,
                GateStatus::Inconclusive,
                Some(InconclusiveReason::AbstractionAmbiguity)
            ))
        );
        // One altered byte: the bytes no longer name the handle, so nothing is stated.
        let mut tampered = out[0].clone();
        let last = tampered.bytes.len() - 1;
        tampered.bytes[last] ^= 1;
        assert_eq!(tampered.statement::<Blake3Hasher>(), None);

        let read = |records: &[EvidenceRecord], cites: &[&EvidenceRef]| {
            let cites: Vec<EvidenceRef> = cites.iter().map(|cite| (*cite).clone()).collect();
            ReplayRecords::<Blake3Hasher>::new(records)
                .inconclusive_reason(GateName::ExactRegression, &cites)
        };
        let crash_cite = EvidenceRef::new(crash.as_str()).unwrap();
        assert_eq!(
            read(&out, &[&crash_cite, &ambiguity]),
            Resolution::Found(InconclusiveReason::AbstractionAmbiguity)
        );
        // Two cited records that disagree on the reason.
        assert_eq!(read(&out, &[&ambiguity, &engine]), Resolution::Unknown);
        // A cited record that states another status, alone or beside a good one.
        assert_eq!(read(&out, &[&passed]), Resolution::Unknown);
        assert_eq!(read(&out, &[&ambiguity, &passed]), Resolution::Unknown);
        // A cited record that was not supplied.
        assert_eq!(read(&out[..1], &[&ambiguity, &engine]), Resolution::Unknown);
        // A record whose bytes were altered.
        assert_eq!(read(&[tampered], &[&ambiguity]), Resolution::Unknown);
        // No record supplied at all.
        assert_eq!(
            read(&[], &[&crash_cite, &ambiguity]),
            Resolution::Unavailable
        );
    }
}
