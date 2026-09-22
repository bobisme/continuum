//! The durable continuation record: what a parked task commits to the store, so a restart
//! can resume it (plan §4.5 O2, bn-20142).
//!
//! > On restart, `Running` tasks resume from their last committed continuation or transition
//! > to `Failed` with a typed reason — never to a silently reconstructed state.
//! >
//! > — `notes/plan/plan.md` §4.5
//!
//! # What was missing, and what this record adds
//!
//! bn-1z09m built the startup pass over the durable `task_*` campaign records. A campaign
//! record names its task, its sequence, its snapshot, its state count and its frontier. It
//! does not name the continuation's *pins* — the epochs, the bounds, the intent, the model
//! source — or the task fields `task.resume` reads to admit and run a resume: the ledger's
//! ceilings and spend, the committed publications, the operation, the target. So a parked
//! head resolved to `Failed(ContinuationNotDurable)`, and RFC 0026's "a terminal status never
//! changes" made that identity final.
//!
//! A [`ContinuationRecord`] is those fields, written into the store as an artifact of the
//! existing class [`ArtifactClass::Continuation`] (`cont_`, plan §4.4). Every field is read
//! back from the committed bytes. Nothing is taken from the successor's configuration, so
//! restoring from it is reading a committed continuation, not inference.
//!
//! # What the record holds
//!
//! | part | why `task.resume` or `task.status` needs it |
//! |---|---|
//! | the `cont_*` handle and its **pin preimage** | the pin preimage is the exact byte string the handle is the content identity of (snapshot, state count, frontier, the five compatibility epochs and engine identity). The startup pass re-derives the handle from it through the declared seam, so the handle claim is checked, not trusted |
//! | `revision` and park state | the order of this continuation's durable revisions, and whether the task was cancelled |
//! | intent and bounds | RFC 0026's remaining creation-time pins |
//! | operation, target, portfolio, priority class, the task's `EpochSet`, the model source | the `TaskRecord` fields and the model-availability guard |
//! | ceilings, spend, checkpoints | the ledger `task.resume` computes the next bound from (`budget::bounds_of`), restored by replay |
//! | publications | the committed campaign records, by commitment. The startup pass matches them against the `task_*` records the store holds |
//! | milestones, committed evidence | the monotone lists of `TaskRecord` (`rule task.status_monotonic`) |
//!
//! # Why the record is not the handle's preimage
//!
//! The `cont_*` handle is the identity of what the continuation *pins*, and it is unchanged
//! by this bone (the CLI goldens carry it). The record also carries facts that change while
//! the continuation stays the same: `task.update_budget` records a new ceiling and a
//! milestone, and `task.cancel` makes the task terminal. Each such change publishes a new
//! record for the **same** handle at the next `revision`. The record's own store identity is
//! therefore the content identity of the record's bytes, and the handle is carried inside it
//! with its preimage. The startup pass takes, per task, the highest revision whose
//! publication list is exactly the task's durable campaign records.
//!
//! # When it is written, and why that order
//!
//! - **At a park**, inside [`verification::advance`](super::verification::advance): the
//!   continuation record is published *before* the campaign record. The campaign record's
//!   index commit is the commit point. A crash or an abort between the two leaves a
//!   continuation record whose publication list names a campaign record the store does not
//!   hold. The startup pass reports it as unclaimed and does not use it, and the task
//!   resolves from its previous durable head. The reverse order would leave a parked head
//!   with no continuation, which is `Failed(ContinuationNotDurable)` for a task that parked
//!   successfully.
//! - **At `task.update_budget` and `task.cancel`** on a parked task: the next revision is
//!   published before the in-memory change. An abort answers `PublicationAborted` (admitted
//!   for both `@mutation` operations) and changes nothing.
//!
//! In each case the record is built from a *projection* of the task after the change, and
//! the handler checks (`debug_assert`) that the projection equals the record of the task it
//! then holds. So the durable record and the live task cannot drift apart silently.
//!
//! # Why this is not a schema change
//!
//! `cont_` is an existing class in plan §4.4 and in `continuum-workspace`'s
//! `ArtifactClass`. No `notes/plan/schemas/*.schema.json` document is bound to it (the
//! `inv003_no_prose_only_evidence` table lists the schema-bound classes, and `cont` is not
//! one). The record reaches no wire field: a client sees only the `cont_*` handle it already
//! saw, and `task.status`, `task.resume` and the others answer with their declared shapes.
//! It is an internal store record in the same position as the `task_*` campaign record
//! (bn-3dr), whose format is also internal. The format tag [`FORMAT`] is its own version;
//! changing the format means a new tag, and a record with an unknown tag is a typed
//! [`RecordDefect`], never a best-effort decode.
//!
//! # What stays volatile, declared
//!
//! - `TaskEntry::events` — `task.subscribe` events are hints (`rule subscription.hints_only`)
//!   and a client recovers the state by re-reading `task.status`. A restored task starts
//!   with no events.
//! - `TaskEntry::campaign` — the engine's `CheckReport` is not in the record. A restored task
//!   answers `verification.result` with `BudgetExhausted` resumable from its continuation.
//!   The next `task.resume` re-derives the report.
//! - a `task.resume` budget write whose run then fails — the ceiling and milestone that
//!   write recorded are durable only when the run commits.
//!
//! No clock, no entropy, no address: every part is a function of the task's own values, and
//! the encoding is [`Preimage`]'s length-prefixed concatenation.

use continuum_engine_reference::bfs::Bounds;
use continuum_engine_reference::model::{Model, State};
use continuum_task::budget::BudgetLedger;
use continuum_task::budget::dimension::CostDimension;
use continuum_workspace::artifact_path::{ArtifactClass, ArtifactHandle};
use continuum_workspace::publication::{
    CapabilityToken, ContentIdentifier, PublishRefusal, ReferenceStore,
};

use super::budget::{self, Publications};
use super::family::Fault;
use super::recovery::{Parts, RecordDefect};
use super::task::{Continuation, PinnedEpochs, Preimage, TaskEntry, TaskTable, budget_preimage};
use crate::protocol::envelope::{Budget, EpochSet};
use crate::protocol::scalar::{
    ByteCount, Commitment, ContinuationHandle, DurationMs, EpochIdentity, EvidenceHandle,
    IntentHandle, OperationName, ProtocolVersion, TaskHandle, Timestamp, WorkspaceHandle,
};
use crate::protocol::shared::Target;
use crate::protocol::spec::{Nullable, Optional, ProtocolEnum};
use crate::protocol::task::Milestone;
use crate::protocol::vocabulary::{ErrorCode, Portfolio, PriorityClass, TargetKind, TaskStatus};

/// The format tag every continuation record starts with. It versions the record format.
pub const FORMAT: &str = "continuum.continuation-record/1";

/// What the task holding the continuation was, at this revision.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ParkState {
    /// Parked and resumable: `TaskStatus::Suspended`.
    Suspended,
    /// Cancelled after it parked: `TaskStatus::Cancelled`. Terminal. The continuation is kept,
    /// as the live table keeps it (see `TaskTable`), and a resume answers the terminal status.
    Cancelled,
}

impl ParkState {
    /// A stable token.
    #[must_use]
    pub const fn token(self) -> &'static str {
        match self {
            Self::Suspended => "suspended",
            Self::Cancelled => "cancelled",
        }
    }

    /// The task status this park state restores to.
    #[must_use]
    pub const fn status(self) -> TaskStatus {
        match self {
            Self::Suspended => TaskStatus::Suspended,
            Self::Cancelled => TaskStatus::Cancelled,
        }
    }

    fn of(token: &str) -> Result<Self, RecordDefect> {
        match token {
            "suspended" => Ok(Self::Suspended),
            "cancelled" => Ok(Self::Cancelled),
            _ => Err(RecordDefect::Vocabulary),
        }
    }
}

/// One budget checkpoint: the publication count it bound and the spend at that moment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Checkpointed {
    /// How many publications were committed at this checkpoint.
    pub committed: u32,
    /// The spend per dimension, in `CostDimension::ALL` order. [`None`] is unmetered.
    pub spend: [Option<u64>; 9],
}

/// A parked task's durable state, as one `cont_*`-class store artifact. See the module
/// documentation for the field table and the write order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContinuationRecord {
    /// The continuation handle this record restores.
    pub handle: ContinuationHandle,
    /// This record's position in the handle's durable revisions, from zero.
    pub revision: u32,
    /// Suspended or cancelled.
    pub state: ParkState,
    /// The task the continuation resumes.
    pub task: TaskHandle,
    /// The snapshot the parked run was over.
    pub snapshot: WorkspaceHandle,
    /// How many states the parked run explored.
    pub states: u64,
    /// The queue-ordered frontier, as state vectors.
    pub frontier: Vec<Vec<i64>>,
    /// The compatibility epochs and engine identity the parked run consumed.
    pub pinned: PinnedEpochs,
    /// The governing intent.
    pub intent: Nullable<IntentHandle>,
    /// The bounds the parked run ran under.
    pub bounds: Bounds,
    /// The `@task_starting` operation that created the task.
    pub operation: OperationName,
    /// What the campaign is aimed at.
    pub target: Target,
    /// The declared portfolio profile.
    pub portfolio: Portfolio,
    /// The declared priority class.
    pub priority_class: PriorityClass,
    /// The epochs the task was created under.
    pub epochs: EpochSet,
    /// The content identity of the model source.
    pub model: Commitment,
    /// The ceilings in force, in the wire's spelling.
    pub ceilings: Budget,
    /// The recorded spend, in `CostDimension::ALL` order.
    pub spend: [Option<u64>; 9],
    /// Every budget checkpoint, in order.
    pub checkpoints: Vec<Checkpointed>,
    /// Every committed publication, by commitment, in commit order.
    pub publications: Vec<Commitment>,
    /// Every milestone reached, in order.
    pub milestones: Vec<Milestone>,
    /// Every evidence handle committed, in order.
    pub committed_evidence: Vec<EvidenceHandle>,
}

/// The pin preimage: the byte string a `cont_*` handle is the content identity of.
///
/// One function for both the park that mints the handle and the startup pass that checks
/// it. The part order is the one the handle has always had: task, snapshot, state count and
/// frontier length as decimal text, every frontier component, then the six identities of
/// [`PinnedEpochs`] (`-` for null).
#[must_use]
pub fn pin_preimage(
    task: &TaskHandle,
    snapshot: &WorkspaceHandle,
    states: u64,
    frontier: &[Vec<i64>],
    pinned: &PinnedEpochs,
) -> Vec<u8> {
    let mut preimage = Preimage::new();
    preimage.text(task.as_str());
    preimage.text(snapshot.as_str());
    preimage.text(&states.to_string());
    preimage.text(&frontier.len().to_string());
    for state in frontier {
        for component in state {
            preimage.push(&component.to_be_bytes());
        }
    }
    for identity in pinned_identities(pinned) {
        text_or_dash(&mut preimage, identity.value().map(EpochIdentity::as_str));
    }
    preimage.bytes().to_vec()
}

/// A frontier as state vectors.
#[must_use]
pub fn vectors(frontier: &[State]) -> Vec<Vec<i64>> {
    frontier
        .iter()
        .map(|state| state.as_slice().to_vec())
        .collect()
}

/// Read state vectors back into `model`'s states.
///
/// # Errors
///
/// [`None`] when a vector is not a state of `model`.
#[must_use]
pub fn states_of(model: &Model, frontier: &[Vec<i64>]) -> Option<Vec<State>> {
    frontier
        .iter()
        .map(|vector| model.state(vector).ok())
        .collect()
}

fn pinned_identities(pinned: &PinnedEpochs) -> [&Nullable<EpochIdentity>; 6] {
    [
        &pinned.semantic,
        &pinned.intent,
        &pinned.evidence,
        &pinned.proof,
        &pinned.corpus,
        &pinned.engine,
    ]
}

fn text_or_dash(preimage: &mut Preimage, text: Option<&str>) {
    preimage.text(text.unwrap_or("-"));
}

fn spend_of(ledger: &BudgetLedger) -> [Option<u64>; 9] {
    CostDimension::ALL.map(|dimension| ledger.spend().measured(dimension))
}

fn spend_preimage(preimage: &mut Preimage, spend: &[Option<u64>; 9]) {
    for value in spend {
        match value {
            Some(value) => preimage.push(&value.to_be_bytes()),
            None => preimage.text("-"),
        }
    }
}

fn count(preimage: &mut Preimage, length: usize) {
    preimage.push(&u32::try_from(length).unwrap_or(u32::MAX).to_be_bytes());
}

impl ContinuationRecord {
    /// The record of `entry` holding `continuation`, at `revision` and `state`.
    ///
    /// `frontier` is passed as vectors because a continuation restored at startup holds its
    /// frontier as vectors until a resume reads it back through the model
    /// (`TaskTable::frontier_of`).
    #[must_use]
    pub fn of(
        entry: &TaskEntry,
        continuation: &Continuation,
        frontier: Vec<Vec<i64>>,
        states: u64,
        revision: u32,
        state: ParkState,
    ) -> Self {
        Self {
            handle: continuation.handle.clone(),
            revision,
            state,
            task: entry.handle.clone(),
            snapshot: continuation.snapshot.clone(),
            states,
            frontier,
            pinned: continuation.pinned.clone(),
            intent: continuation.intent.clone(),
            bounds: continuation.bounds,
            operation: entry.operation.clone(),
            target: entry.target.clone(),
            portfolio: entry.portfolio,
            priority_class: entry.priority_class,
            epochs: entry.epochs.clone(),
            model: entry.model.clone(),
            ceilings: entry.budget(),
            spend: spend_of(&entry.ledger),
            checkpoints: entry
                .ledger
                .checkpoints()
                .iter()
                .map(|checkpoint| Checkpointed {
                    committed: checkpoint.committed(),
                    spend: CostDimension::ALL
                        .map(|dimension| checkpoint.spend().measured(dimension)),
                })
                .collect(),
            publications: entry
                .evidence
                .committed()
                .iter()
                .map(|publication| publication.commitment().clone())
                .collect(),
            milestones: entry.milestones.clone(),
            committed_evidence: entry.committed_evidence.clone(),
        }
    }

    /// This record's pin preimage. See [`pin_preimage`].
    #[must_use]
    pub fn pins(&self) -> Vec<u8> {
        pin_preimage(
            &self.task,
            &self.snapshot,
            self.states,
            &self.frontier,
            &self.pinned,
        )
    }

    /// The canonical bytes: the store holds exactly these.
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        let mut preimage = Preimage::new();
        preimage.text(FORMAT);
        preimage.text(self.handle.as_str());
        preimage.push(&self.revision.to_be_bytes());
        preimage.text(self.state.token());
        preimage.push(&self.pins());
        text_or_dash(&mut preimage, self.intent.value().map(IntentHandle::as_str));
        preimage.push(&(self.bounds.states() as u64).to_be_bytes());
        preimage.push(&(self.bounds.depth() as u64).to_be_bytes());
        preimage.push(&self.bounds.transitions().to_be_bytes());
        preimage.text(self.operation.as_str());
        preimage.text(self.target.kind.as_wire());
        preimage.text(&self.target.id);
        preimage.text(self.portfolio.as_wire());
        preimage.text(self.priority_class.as_wire());
        preimage.push(&self.epochs.protocol.major().to_be_bytes());
        preimage.push(&self.epochs.protocol.minor().to_be_bytes());
        super::task::epochs_preimage(&mut preimage, &self.epochs);
        preimage.text(self.model.as_str());
        budget_preimage(&mut preimage, &self.ceilings);
        spend_preimage(&mut preimage, &self.spend);
        count(&mut preimage, self.checkpoints.len());
        for checkpoint in &self.checkpoints {
            preimage.push(&checkpoint.committed.to_be_bytes());
            spend_preimage(&mut preimage, &checkpoint.spend);
        }
        count(&mut preimage, self.publications.len());
        for publication in &self.publications {
            preimage.text(publication.as_str());
        }
        count(&mut preimage, self.milestones.len());
        for milestone in &self.milestones {
            preimage.text(&milestone.name);
            preimage.text(milestone.at.as_str());
        }
        count(&mut preimage, self.committed_evidence.len());
        for evidence in &self.committed_evidence {
            preimage.text(evidence.as_str());
        }
        preimage.bytes().to_vec()
    }

    /// Decode the bytes a `cont_*` index entry names.
    ///
    /// Strict, like the campaign-record decoder: every part at its declared width, no byte
    /// left over, every token in its vocabulary, and the bytes are the canonical encoding of
    /// what they decode to. A record that passes also restores: the ledger replay is run here,
    /// so a record whose checkpoints and spend disagree is a defect now rather than a failure
    /// at startup.
    ///
    /// The handle is *not* checked against the pins here, because that needs the identity
    /// seam. [`Self::derives_its_handle`] is that check, and the startup pass runs it.
    ///
    /// # Errors
    ///
    /// The [`RecordDefect`] that names the first part that did not decode.
    pub fn decode(bytes: &[u8]) -> Result<Self, RecordDefect> {
        let mut parts = Parts::new(bytes);
        if parts.text()? != FORMAT {
            return Err(RecordDefect::Format);
        }
        let handle =
            ContinuationHandle::new(parts.text()?).map_err(|_| RecordDefect::Vocabulary)?;
        let revision = parts.u32()?;
        let state = ParkState::of(parts.text()?)?;
        let (task, snapshot, states, frontier, pinned) = decode_pins(parts.next()?)?;
        let intent = match parts.text()? {
            "-" => Nullable::Null,
            text => Nullable::Value(IntentHandle::new(text).map_err(|_| RecordDefect::Vocabulary)?),
        };
        let bounds = Bounds::new(
            usize::try_from(parts.u64()?).map_err(|_| RecordDefect::FieldWidth)?,
            usize::try_from(parts.u64()?).map_err(|_| RecordDefect::FieldWidth)?,
            parts.u64()?,
        );
        let operation = OperationName::new(parts.text()?).map_err(|_| RecordDefect::Vocabulary)?;
        let target = Target {
            kind: TargetKind::from_wire(parts.text()?).map_err(|_| RecordDefect::Vocabulary)?,
            id: parts.text()?.to_owned(),
        };
        let portfolio =
            Portfolio::from_wire(parts.text()?).map_err(|_| RecordDefect::Vocabulary)?;
        let priority_class =
            PriorityClass::from_wire(parts.text()?).map_err(|_| RecordDefect::Vocabulary)?;
        let protocol = ProtocolVersion::new(parts.u32()?, parts.u32()?);
        let task_epochs = decode_epochs(&mut parts)?;
        let epochs = EpochSet {
            protocol,
            semantic: task_epochs.semantic,
            intent: task_epochs.intent,
            evidence: task_epochs.evidence,
            proof: task_epochs.proof,
            corpus: task_epochs.corpus,
            engine: task_epochs.engine,
        };
        let model = Commitment::new(parts.text()?);
        let ceilings = decode_budget(&mut parts)?;
        let spend = decode_spend(&mut parts)?;
        let mut checkpoints = Vec::new();
        for _ in 0..parts.u32()? {
            checkpoints.push(Checkpointed {
                committed: parts.u32()?,
                spend: decode_spend(&mut parts)?,
            });
        }
        let mut publications = Vec::new();
        for _ in 0..parts.u32()? {
            publications.push(Commitment::new(parts.text()?));
        }
        let mut milestones = Vec::new();
        for _ in 0..parts.u32()? {
            let name = parts.text()?.to_owned();
            let at = Timestamp::new(parts.text()?).map_err(|_| RecordDefect::Vocabulary)?;
            milestones.push(Milestone { name, at });
        }
        let mut committed_evidence = Vec::new();
        for _ in 0..parts.u32()? {
            committed_evidence
                .push(EvidenceHandle::new(parts.text()?).map_err(|_| RecordDefect::Vocabulary)?);
        }
        if !parts.is_empty() {
            return Err(RecordDefect::TrailingBytes);
        }
        let record = Self {
            handle,
            revision,
            state,
            task,
            snapshot,
            states,
            frontier,
            pinned,
            intent,
            bounds,
            operation,
            target,
            portfolio,
            priority_class,
            epochs,
            model,
            ceilings,
            spend,
            checkpoints,
            publications,
            milestones,
            committed_evidence,
        };
        if record.encode() != bytes {
            return Err(RecordDefect::NonCanonical);
        }
        record.ledger()?;
        Ok(record)
    }

    /// Whether this record's pins derive its handle through `identifier`.
    #[must_use]
    pub fn derives_its_handle(&self, identifier: &dyn ContentIdentifier) -> bool {
        identifier
            .identify(ArtifactClass::Continuation, &self.pins())
            .is_ok_and(|derived| derived.to_string() == self.handle.as_str())
    }

    /// Replay the ledger this record describes.
    ///
    /// `BudgetLedger` has no constructor from parts, by design (one task, one accounting).
    /// So the ledger is rebuilt through its own operations, in the order the live task made
    /// them: charges up to each checkpoint's spend and the checkpoint, charges up to the
    /// final spend, then the ceilings in force. The replay runs unbounded, so no charge can
    /// exhaust, and the ceilings are recorded last through `budget::update`, which records a
    /// ceiling on every arm. The result is checked against the record, part by part.
    fn ledger(&self) -> Result<BudgetLedger, RecordDefect> {
        let mut ledger = budget::ledger_of(&unbounded());
        for checkpoint in &self.checkpoints {
            charge_to(&mut ledger, &checkpoint.spend)?;
            ledger
                .checkpoint(checkpoint.committed)
                .map_err(|_| RecordDefect::Ledger)?;
        }
        charge_to(&mut ledger, &self.spend)?;
        let _ = budget::update(&mut ledger, &self.ceilings);
        let replayed_checkpoints: Vec<(u32, [Option<u64>; 9])> = ledger
            .checkpoints()
            .iter()
            .map(|checkpoint| {
                (
                    checkpoint.committed(),
                    CostDimension::ALL.map(|dimension| checkpoint.spend().measured(dimension)),
                )
            })
            .collect();
        let recorded: Vec<(u32, [Option<u64>; 9])> = self
            .checkpoints
            .iter()
            .map(|checkpoint| (checkpoint.committed, checkpoint.spend))
            .collect();
        if spend_of(&ledger) != self.spend
            || replayed_checkpoints != recorded
            || budget::wire_budget(&ledger) != self.ceilings
            || ledger.exhaustion().is_some()
            || self.checkpoints.len() != self.publications.len()
        {
            return Err(RecordDefect::Ledger);
        }
        Ok(ledger)
    }

    /// The live task and continuation this record restores.
    ///
    /// The continuation's `frontier` is empty: its states can be read back only through the
    /// model, and the model catalog is provisioned after startup (`VolatileFact::ModelCatalog`
    /// is `Reprovisioned`). The vectors ride beside it in [`Restored::frontier`], and
    /// `task.resume` reads them back before it runs.
    ///
    /// # Errors
    ///
    /// [`RecordDefect::Ledger`] when the ledger does not replay. Unreachable for a record
    /// [`Self::decode`] returned, which runs the same replay.
    pub fn restore(&self) -> Result<Restored, RecordDefect> {
        let ledger = self.ledger()?;
        let mut evidence = Publications::new();
        for (publication, checkpoint) in self.publications.iter().zip(ledger.checkpoints()) {
            evidence.stage(publication.clone());
            if evidence.commit(*checkpoint).is_none() {
                return Err(RecordDefect::Ledger);
            }
        }
        let entry = TaskEntry {
            handle: self.task.clone(),
            operation: self.operation.clone(),
            status: self.state.status(),
            snapshot: Nullable::Value(self.snapshot.clone()),
            intent: self.intent.clone(),
            target: self.target.clone(),
            portfolio: self.portfolio,
            priority_class: self.priority_class,
            ledger,
            epochs: self.epochs.clone(),
            model: self.model.clone(),
            milestones: self.milestones.clone(),
            committed_evidence: self.committed_evidence.clone(),
            events: Vec::new(),
            continuation: Some(self.handle.clone()),
            failed_reason: None,
            non_resumable_reason: None,
            campaign: None,
            region: None,
            worker: None,
            evidence,
        };
        let continuation = Continuation {
            handle: self.handle.clone(),
            task: self.task.clone(),
            snapshot: self.snapshot.clone(),
            intent: self.intent.clone(),
            pinned: self.pinned.clone(),
            bounds: self.bounds,
            frontier: Vec::new(),
        };
        Ok(Restored {
            entry,
            continuation,
            frontier: self.frontier.clone(),
            revision: self.revision,
            states: self.states,
        })
    }
}

/// The record of `task` as `tasks` holds it now, at `revision`, with the pinned state
/// count `states`, or [`None`] when the task holds no continuation.
fn held(
    tasks: &TaskTable,
    task: &TaskHandle,
    states: u64,
    revision: u32,
    state: ParkState,
) -> Option<ContinuationRecord> {
    let entry = tasks.get(task)?;
    let handle = entry.continuation.as_ref()?;
    let continuation = tasks.continuation(handle)?;
    let frontier = tasks.frontier_of(handle)?;
    Some(ContinuationRecord::of(
        entry,
        continuation,
        frontier,
        states,
        revision,
        state,
    ))
}

/// The record of `task` as `tasks` holds it now, at the next revision of its continuation.
///
/// [`None`] when the task holds no continuation, or holds one with no durable record. The
/// second is unreachable: every park publishes one. The caller then publishes nothing,
/// which is the behaviour before bn-20142.
#[must_use]
pub fn current(
    tasks: &TaskTable,
    task: &TaskHandle,
    state: ParkState,
) -> Option<ContinuationRecord> {
    let handle = tasks.get(task)?.continuation.clone()?;
    held(
        tasks,
        task,
        tasks.pinned_states(&handle)?,
        tasks.next_revision(&handle),
        state,
    )
}

/// Record that `projected` is durable, after the live change it projected has been made.
///
/// The check that keeps the durable record and the live task from drifting apart: the
/// record of the task as it is now must equal the projection that was published before the
/// change. A disagreement is a defect in this wiring, so it is a `debug_assert`, and the
/// suites run every path that reaches it.
pub fn settle(tasks: &mut TaskTable, projected: &ContinuationRecord) {
    debug_assert_eq!(
        held(
            tasks,
            &projected.task,
            projected.states,
            projected.revision,
            projected.state
        )
        .as_ref(),
        Some(projected),
        "the published continuation record is not the record of the task the daemon holds"
    );
    tasks.record_revision(&projected.handle, projected.revision, projected.states);
}

/// The milestone `name` at `now`, appended to `record`, when a time reading was supplied.
///
/// The projection of [`TaskEntry::reach`], which records nothing without a reading.
pub fn reach(record: &mut ContinuationRecord, name: &str, now: Option<&Timestamp>) {
    if let Some(at) = now {
        record.milestones.push(Milestone {
            name: name.to_owned(),
            at: at.clone(),
        });
    }
}

/// A task and its continuation, read back from a [`ContinuationRecord`].
#[derive(Debug)]
pub struct Restored {
    /// The live task.
    pub entry: TaskEntry,
    /// The continuation, with its frontier not yet read back through the model.
    pub continuation: Continuation,
    /// The frontier, as state vectors.
    pub frontier: Vec<Vec<i64>>,
    /// The revision the record was at.
    pub revision: u32,
    /// The state count the continuation's pins name.
    pub states: u64,
}

fn unbounded() -> Budget {
    Budget {
        wall_ms: Optional::Absent,
        cpu_ms: Optional::Absent,
        memory_bytes: Optional::Absent,
        states: Optional::Absent,
        solver_ms: Optional::Absent,
        proof_ms: Optional::Absent,
        tokens: Optional::Absent,
        candidates: Optional::Absent,
        bytes: Optional::Absent,
    }
}

/// Charge `ledger` up to `target`, dimension by dimension.
fn charge_to(ledger: &mut BudgetLedger, target: &[Option<u64>; 9]) -> Result<(), RecordDefect> {
    for (dimension, target) in CostDimension::ALL.into_iter().zip(target) {
        let current = ledger.spend().measured(dimension);
        match (current, target) {
            (None, None) => {}
            (Some(current), Some(target)) if *target >= current => {
                if *target > current {
                    let outcome = ledger
                        .charge(dimension, target - current)
                        .map_err(|_| RecordDefect::Ledger)?;
                    if !outcome.is_admitted() {
                        return Err(RecordDefect::Ledger);
                    }
                }
            }
            _ => return Err(RecordDefect::Ledger),
        }
    }
    Ok(())
}

type Pins = (
    TaskHandle,
    WorkspaceHandle,
    u64,
    Vec<Vec<i64>>,
    PinnedEpochs,
);

fn decode_pins(bytes: &[u8]) -> Result<Pins, RecordDefect> {
    let mut parts = Parts::new(bytes);
    let task = TaskHandle::new(parts.text()?).map_err(|_| RecordDefect::TaskHandle)?;
    let snapshot = WorkspaceHandle::new(parts.text()?).map_err(|_| RecordDefect::SnapshotHandle)?;
    let states: u64 = parts
        .text()?
        .parse()
        .map_err(|_| RecordDefect::FieldWidth)?;
    let length: usize = parts
        .text()?
        .parse()
        .map_err(|_| RecordDefect::FieldWidth)?;
    let mut rest = Vec::new();
    while !parts.is_empty() {
        rest.push(parts.next()?);
    }
    if rest.len() < 6 {
        return Err(RecordDefect::Truncated);
    }
    let epochs = rest.split_off(rest.len() - 6);
    let components = rest;
    let shaped = match length {
        0 => components.is_empty(),
        n => !components.is_empty() && components.len() % n == 0,
    };
    if !shaped {
        return Err(RecordDefect::FrontierShape);
    }
    let mut values = Vec::with_capacity(components.len());
    for component in components {
        let bytes: [u8; 8] = component.try_into().map_err(|_| RecordDefect::FieldWidth)?;
        values.push(i64::from_be_bytes(bytes));
    }
    let frontier = match length {
        0 => Vec::new(),
        n => values
            .chunks(values.len() / n)
            .map(<[i64]>::to_vec)
            .collect(),
    };
    let mut identities = Vec::with_capacity(6);
    for part in epochs {
        let text = std::str::from_utf8(part).map_err(|_| RecordDefect::Truncated)?;
        identities.push(epoch(text)?);
    }
    let [semantic, intent, evidence, proof, corpus, engine]: [Nullable<EpochIdentity>; 6] =
        identities.try_into().map_err(|_| RecordDefect::Truncated)?;
    Ok((
        task,
        snapshot,
        states,
        frontier,
        PinnedEpochs {
            semantic,
            intent,
            evidence,
            proof,
            corpus,
            engine,
        },
    ))
}

fn epoch(text: &str) -> Result<Nullable<EpochIdentity>, RecordDefect> {
    match text {
        "-" => Ok(Nullable::Null),
        text => Ok(Nullable::Value(
            EpochIdentity::new(text).map_err(|_| RecordDefect::Vocabulary)?,
        )),
    }
}

fn decode_epochs(parts: &mut Parts<'_>) -> Result<PinnedEpochs, RecordDefect> {
    Ok(PinnedEpochs {
        semantic: epoch(parts.text()?)?,
        intent: epoch(parts.text()?)?,
        evidence: epoch(parts.text()?)?,
        proof: epoch(parts.text()?)?,
        corpus: epoch(parts.text()?)?,
        engine: epoch(parts.text()?)?,
    })
}

/// One `u64`-or-`-` part: a value is eight bytes and the absent marker is one, so the width
/// decides.
fn optional_u64(parts: &mut Parts<'_>) -> Result<Option<u64>, RecordDefect> {
    let part = parts.next()?;
    if part == b"-" {
        return Ok(None);
    }
    let bytes: [u8; 8] = part.try_into().map_err(|_| RecordDefect::FieldWidth)?;
    Ok(Some(u64::from_be_bytes(bytes)))
}

fn decode_spend(parts: &mut Parts<'_>) -> Result<[Option<u64>; 9], RecordDefect> {
    let mut spend = [None; 9];
    for slot in &mut spend {
        *slot = optional_u64(parts)?;
    }
    Ok(spend)
}

fn decode_budget(parts: &mut Parts<'_>) -> Result<Budget, RecordDefect> {
    let [
        wall_ms,
        cpu_ms,
        memory_bytes,
        states,
        solver_ms,
        proof_ms,
        tokens,
        candidates,
        bytes,
    ] = decode_spend(parts)?;
    fn present<T>(value: Option<T>) -> Optional<T> {
        match value {
            Some(value) => Optional::Present(value),
            None => Optional::Absent,
        }
    }
    Ok(Budget {
        wall_ms: present(wall_ms.map(DurationMs::new)),
        cpu_ms: present(cpu_ms.map(DurationMs::new)),
        memory_bytes: present(memory_bytes.map(ByteCount::new)),
        states: present(states),
        solver_ms: present(solver_ms.map(DurationMs::new)),
        proof_ms: present(proof_ms.map(DurationMs::new)),
        tokens: present(tokens),
        candidates: present(candidates),
        bytes: present(bytes.map(ByteCount::new)),
    })
}

/// Publish `record` into the store under [`ArtifactClass::Continuation`].
///
/// The store's two-phase protocol, as for the campaign record: stage, commit the content,
/// commit the index. The daemon's name for the record and the store's are checked against
/// each other, and a disagreement is refused.
///
/// # Errors
///
/// [`Fault::denied`] when `publisher` may not publish a `cont_` artifact, and
/// [`ErrorCode::PublicationAborted`] when the publication did not complete atomically or the
/// two identity seams disagree. Nothing is published on an error path.
pub fn publish(
    store: &ReferenceStore,
    publisher: &CapabilityToken,
    identifier: &dyn ContentIdentifier,
    record: &ContinuationRecord,
) -> Result<ArtifactHandle, Fault> {
    let aborted = || {
        Fault::new(
            ErrorCode::PublicationAborted,
            "the continuation record's publication aborted; nothing was published and nothing \
             was changed",
        )
    };
    let bytes = record.encode();
    let named = identifier
        .identify(ArtifactClass::Continuation, &bytes)
        .map_err(|_| aborted().not_retryable())?;
    let receipt = store
        .stage(ArtifactClass::Continuation, bytes, publisher)
        .and_then(|staged| Ok(staged.commit_content()?.commit_index()?))
        .map_err(|refusal| match refusal {
            PublishRefusal::CapabilityDenied(_) => Fault::denied(),
            PublishRefusal::Aborted(_) => aborted(),
        })?;
    if receipt.handle() != &named {
        return Err(aborted().not_retryable());
    }
    Ok(named)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::daemon::identity::Blake3Identity;

    fn epoch(token: &str) -> Nullable<EpochIdentity> {
        Nullable::Value(EpochIdentity::new(token).expect("a well-formed epoch"))
    }

    fn record() -> ContinuationRecord {
        let task = TaskHandle::new("task_t").expect("a task handle");
        let snapshot = WorkspaceHandle::new("ws_s").expect("a workspace handle");
        let pinned = PinnedEpochs {
            semantic: epoch("semantic-1"),
            intent: epoch("intent-1"),
            evidence: Nullable::Null,
            proof: epoch("proof-1"),
            corpus: Nullable::Null,
            engine: epoch("engine-reference-1"),
        };
        let frontier = vec![vec![0, 3], vec![5, 0]];
        let pins = pin_preimage(&task, &snapshot, 4, &frontier, &pinned);
        let handle = Blake3Identity
            .identify(ArtifactClass::Continuation, &pins)
            .expect("blake3 names every input");
        ContinuationRecord {
            handle: ContinuationHandle::new(&handle.to_string()).expect("a cont handle"),
            revision: 1,
            state: ParkState::Suspended,
            task,
            snapshot,
            states: 4,
            frontier,
            pinned: pinned.clone(),
            intent: Nullable::Value(IntentHandle::new("in_i").expect("an intent handle")),
            bounds: Bounds::CERTIFIABLE.with_states(4),
            operation: OperationName::new("verification.start").expect("an operation"),
            target: Target {
                kind: TargetKind::AllClaims,
                id: "DieHard".to_owned(),
            },
            portfolio: Portfolio::Interactive,
            priority_class: PriorityClass::Interactive,
            epochs: EpochSet {
                protocol: ProtocolVersion::new(3, 1),
                semantic: pinned.semantic.clone(),
                intent: pinned.intent.clone(),
                evidence: Nullable::Null,
                proof: pinned.proof.clone(),
                corpus: Nullable::Null,
                engine: pinned.engine.clone(),
            },
            model: Commitment::new("elab_m"),
            ceilings: Budget {
                states: Optional::Present(8),
                wall_ms: Optional::Present(DurationMs::new(5)),
                ..unbounded()
            },
            spend: {
                let mut spend = [None; 9];
                spend[CostDimension::States.index()] = Some(4);
                spend
            },
            checkpoints: vec![Checkpointed {
                committed: 1,
                spend: {
                    let mut spend = [None; 9];
                    spend[CostDimension::States.index()] = Some(4);
                    spend
                },
            }],
            publications: vec![Commitment::new("task_p0")],
            milestones: vec![Milestone {
                name: "exploration.bounded".to_owned(),
                at: Timestamp::new("2026-08-01T00:00:00.000Z").expect("a timestamp"),
            }],
            committed_evidence: Vec::new(),
        }
    }

    #[test]
    fn the_decoder_is_the_inverse_of_the_encoder_and_the_pins_derive_the_handle() {
        let record = record();
        let decoded = ContinuationRecord::decode(&record.encode()).expect("decodes");
        assert_eq!(decoded, record);
        assert!(decoded.derives_its_handle(&Blake3Identity));

        let mut forged = record.clone();
        forged.handle = ContinuationHandle::new("cont_forged").expect("a cont handle");
        let forged = ContinuationRecord::decode(&forged.encode()).expect("still decodes");
        assert!(
            !forged.derives_its_handle(&Blake3Identity),
            "a handle the pins do not derive is caught"
        );
    }

    #[test]
    fn the_restored_ledger_is_the_recorded_one() {
        let restored = record().restore().expect("restores");
        let entry = &restored.entry;
        assert_eq!(entry.status, TaskStatus::Suspended);
        assert_eq!(entry.publications(), 1);
        assert_eq!(entry.budget(), record().ceilings);
        assert_eq!(
            entry.ledger.spend().measured(CostDimension::States),
            Some(4)
        );
        assert_eq!(entry.ledger.checkpoints().len(), 1);
        assert_eq!(restored.frontier, vec![vec![0, 3], vec![5, 0]]);
        assert!(restored.continuation.frontier.is_empty());
        let again = ContinuationRecord::of(
            entry,
            &restored.continuation,
            restored.frontier.clone(),
            4,
            1,
            ParkState::Suspended,
        );
        assert_eq!(again, record(), "record → restore → record is the identity");
    }

    #[test]
    fn a_record_that_does_not_decode_is_a_typed_defect_per_cause() {
        let good = record().encode();
        assert_eq!(
            ContinuationRecord::decode(&good[..good.len() - 1]),
            Err(RecordDefect::Truncated)
        );
        let mut trailing = good.clone();
        trailing.extend_from_slice(&0_u64.to_be_bytes());
        assert_eq!(
            ContinuationRecord::decode(&trailing),
            Err(RecordDefect::TrailingBytes)
        );
        let mut wrong_format = record();
        wrong_format.revision = 0;
        let mut bytes = wrong_format.encode();
        // The format tag's first byte after its eight-byte length prefix.
        bytes[8] = b'X';
        assert_eq!(
            ContinuationRecord::decode(&bytes),
            Err(RecordDefect::Format)
        );
        let mut inconsistent = record();
        inconsistent.spend[CostDimension::States.index()] = Some(2);
        assert_eq!(
            ContinuationRecord::decode(&inconsistent.encode()),
            Err(RecordDefect::Ledger),
            "a spend below the last checkpoint does not replay"
        );
        let mut unpaired = record();
        unpaired.publications.push(Commitment::new("task_p1"));
        assert_eq!(
            ContinuationRecord::decode(&unpaired.encode()),
            Err(RecordDefect::Ledger),
            "a publication with no checkpoint does not replay"
        );
    }
}
