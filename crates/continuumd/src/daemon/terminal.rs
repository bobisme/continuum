//! The durable terminal task record: what a task that reached `Completed` or `Failed`
//! commits to the store, so a restart answers `task.status` on it from durable data
//! (plan §4.5, RFC 0026 "Task lifecycle", bn-2g3ei).
//!
//! > `task.status` is monotonic. Reported milestones and `committed_evidence` only grow, and
//! > a terminal status never changes.
//! >
//! > — RFC 0026, "Task lifecycle"
//!
//! # What was missing
//!
//! bn-20142 made a *parked* task durable: its continuation record ([`continuation`]) carries
//! every field `task.status` projects, and a restart restores the task live. A task that
//! closed published no such record. The startup pass resolved it to `Settled` from its
//! `closed` campaign record, and `task.status` on its handle answered `CapabilityDenied`,
//! although the pre-crash daemon had answered `Completed`. A first run that failed
//! (`BudgetExhausted`, the state budget cannot hold the initial states) published nothing at
//! all, so a restart did not know the task.
//!
//! A [`TerminalRecord`] is the terminal task's `TaskRecord` inputs, written into the store
//! when the task reaches `Completed` or `Failed`. The startup pass restores the task from it
//! as a live, terminal `TaskEntry`, so `task.status` answers byte for byte what it answered
//! before the crash. A terminal status never changes, so the record is written once per task
//! and has no revisions.
//!
//! # Why a sibling record, and not a terminal revision of the continuation record
//!
//! A continuation record is keyed by a `cont_*` handle, and the handle is the content
//! identity of what a continuation *pins* (plan §4.4: "resumable task continuation"). A task
//! that closed on its first run never parked, so it has no `cont_*` handle, and minting one
//! for it would name a continuation that does not exist. A task that failed on its first run
//! has none either. So a terminal park state would cover only a task that parked and was
//! then resumed to completion. `Cancelled` is the one terminal status that is reached only
//! from a park (`task.cancel` of a `Suspended` task), and it stays a continuation revision
//! (bn-20142). The two remaining terminal statuses get this record.
//!
//! The field set is the continuation record's task half — operation, target, portfolio,
//! priority class, epochs, model source, ceilings, spend, checkpoints, publications,
//! milestones, committed evidence — plus what a terminal `TaskRecord` adds: the status, the
//! snapshot and intent as the task names them, `failed_reason`, `non_resumable_reason` and
//! the `continuation` field. The ledger replay and the part encoders are the continuation
//! record's own ([`continuation::replay`]), so the two records restore one ledger one way.
//!
//! # When it is written, and the commit point
//!
//! Inside [`verification::advance`](super::verification::advance), the one writer of a
//! task's outcome, before the in-memory status moves:
//!
//! - **`Completed`**: the terminal record is published *before* the closing campaign record.
//!   The campaign record's index commit is the commit point, as it is for a park. A crash or
//!   an abort between the two leaves a terminal record whose publication list names a
//!   campaign record the store does not hold. The startup pass reports it as unclaimed and
//!   does not use it, and the task resolves from its previous durable head. The reverse order
//!   would leave a `closed` head with no terminal record, which is the `Settled` resolution
//!   this record exists to remove.
//! - **`Failed`**: the run commits no campaign record, so the terminal record is the only
//!   durable write, and its index commit is the commit point. An abort answers
//!   `PublicationAborted` and the task does not become `Failed`.
//!
//! The record is built from a *projection* of the task after the change, and the writer
//! checks (`debug_assert`) that the projection equals the record of the task it then holds,
//! as the continuation record's writers do.
//!
//! # Why this is not a schema change
//!
//! `task_` is an existing class in plan §4.4 and in `continuum-workspace`'s `ArtifactClass`,
//! and the store already holds an internal record format under it: the campaign record
//! (bn-3dr, [`budget::publication_record`](super::budget::publication_record)), whose format
//! is also internal. `schemas/verification-task.schema.json` is the *wire* `TaskRecord`'s
//! artifact form, and no `task_` store record has ever been bound to it: the campaign record
//! does not conform to it and is not required to. This record reaches no wire field either.
//! `task.status` answers the `TaskRecord` it always declared, and no response carries the
//! record's identity. The format tag [`FORMAT`] is its own version and is what the startup
//! pass tells the two `task_` formats apart by: a campaign record starts with a task handle,
//! which never equals the tag. A record with an unknown tag is a typed [`RecordDefect`].
//! No IDL, protocol or JSON-schema document changes.
//!
//! # What stays volatile, declared
//!
//! - `TaskEntry::events` — hints (`rule subscription.hints_only`), as for a restored parked
//!   task.
//! - `TaskEntry::campaign` — the engine's `CheckReport`. The model, the target and the
//!   bounds are durable, and the engine is deterministic, so `verification.result` on a
//!   restored `Completed` task re-derives the report from them and checks it against the
//!   durable record (`verification::verification_result`). Nothing about the task changes.
//! - the continuations a completed task held before it closed. The task's
//!   `TaskRecord.continuation` is absent once it closed, so `task.status` does not read them.

use continuum_workspace::publication::{ContentIdentifier, StoreAudit};

use super::budget;
use super::continuation::{
    self, Checkpointed, count, decode_budget, decode_epochs, decode_spend, spend_preimage,
    text_or_dash,
};
use super::family::Fault;
use super::recovery::{Parts, RecordDefect};
use super::task::{Preimage, TaskEntry, budget_preimage, epochs_preimage};
use crate::protocol::envelope::{Budget, EpochSet};
use crate::protocol::scalar::{
    Commitment, ContinuationHandle, EvidenceHandle, IntentHandle, OperationName, ProtocolVersion,
    TaskHandle, Timestamp, WorkspaceHandle,
};
use crate::protocol::shared::Target;
use crate::protocol::spec::{Nullable, ProtocolEnum};
use crate::protocol::task::Milestone;
use crate::protocol::vocabulary::{ErrorCode, Portfolio, PriorityClass, TargetKind, TaskStatus};

/// The format tag every terminal record starts with. It versions the record format.
pub const FORMAT: &str = "continuum.task-terminal-record/1";

/// The terminal status a [`TerminalRecord`] carries.
///
/// `Cancelled` is not here: it is reached only from a park, and it is the terminal revision
/// of the task's continuation record (bn-20142).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TerminalState {
    /// The exploration closed: `TaskStatus::Completed`.
    Completed,
    /// The run failed with a typed reason: `TaskStatus::Failed`.
    Failed,
}

impl TerminalState {
    /// A stable token.
    #[must_use]
    pub const fn token(self) -> &'static str {
        match self {
            Self::Completed => "completed",
            Self::Failed => "failed",
        }
    }

    /// The task status this terminal state restores to.
    #[must_use]
    pub const fn status(self) -> TaskStatus {
        match self {
            Self::Completed => TaskStatus::Completed,
            Self::Failed => TaskStatus::Failed,
        }
    }

    /// The terminal state of `status`, or [`None`] when `status` is not one a terminal
    /// record carries.
    #[must_use]
    pub const fn of_status(status: TaskStatus) -> Option<Self> {
        match status {
            TaskStatus::Completed => Some(Self::Completed),
            TaskStatus::Failed => Some(Self::Failed),
            _ => None,
        }
    }

    fn of(token: &str) -> Result<Self, RecordDefect> {
        match token {
            "completed" => Ok(Self::Completed),
            "failed" => Ok(Self::Failed),
            _ => Err(RecordDefect::Vocabulary),
        }
    }
}

/// A terminal task's durable state, as one `task_*`-class store artifact. See the module
/// documentation for the field set and the write order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalRecord {
    /// The task.
    pub task: TaskHandle,
    /// `Completed` or `Failed`.
    pub state: TerminalState,
    /// The `@task_starting` operation that created the task.
    pub operation: OperationName,
    /// The snapshot the campaign is over.
    pub snapshot: Nullable<WorkspaceHandle>,
    /// The governing intent.
    pub intent: Nullable<IntentHandle>,
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
    /// `TaskRecord.failed_reason`: REQUIRED when the state is `Failed`, absent otherwise.
    pub failed_reason: Option<ErrorCode>,
    /// `TaskRecord.non_resumable_reason`.
    pub non_resumable_reason: Option<String>,
    /// `TaskRecord.continuation`.
    pub continuation: Option<ContinuationHandle>,
}

impl TerminalRecord {
    /// The record of `entry`, carrying `state`.
    ///
    /// `state` is passed rather than read so the writer can build the record of the task
    /// *after* the change it is about to make: see [`verification::advance`](super::verification::advance).
    #[must_use]
    pub fn of(entry: &TaskEntry, state: TerminalState) -> Self {
        Self {
            task: entry.handle.clone(),
            state,
            operation: entry.operation.clone(),
            snapshot: entry.snapshot.clone(),
            intent: entry.intent.clone(),
            target: entry.target.clone(),
            portfolio: entry.portfolio,
            priority_class: entry.priority_class,
            epochs: entry.epochs.clone(),
            model: entry.model.clone(),
            ceilings: entry.budget(),
            spend: continuation::spend_of(&entry.ledger),
            checkpoints: continuation::checkpoints_of(&entry.ledger),
            publications: entry
                .evidence
                .committed()
                .iter()
                .map(|publication| publication.commitment().clone())
                .collect(),
            milestones: entry.milestones.clone(),
            committed_evidence: entry.committed_evidence.clone(),
            failed_reason: entry.failed_reason,
            non_resumable_reason: entry.non_resumable_reason.clone(),
            continuation: entry.continuation.clone(),
        }
    }

    /// The record of `entry` as it is now, or [`None`] when its status is not one a terminal
    /// record carries.
    #[must_use]
    pub fn current(entry: &TaskEntry) -> Option<Self> {
        TerminalState::of_status(entry.status).map(|state| Self::of(entry, state))
    }

    /// The canonical bytes: the store holds exactly these.
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        let mut preimage = Preimage::new();
        preimage.text(FORMAT);
        preimage.text(self.task.as_str());
        preimage.text(self.state.token());
        preimage.text(self.operation.as_str());
        text_or_dash(
            &mut preimage,
            self.snapshot.value().map(WorkspaceHandle::as_str),
        );
        text_or_dash(&mut preimage, self.intent.value().map(IntentHandle::as_str));
        preimage.text(self.target.kind.as_wire());
        preimage.text(&self.target.id);
        preimage.text(self.portfolio.as_wire());
        preimage.text(self.priority_class.as_wire());
        preimage.push(&self.epochs.protocol.major().to_be_bytes());
        preimage.push(&self.epochs.protocol.minor().to_be_bytes());
        epochs_preimage(&mut preimage, &self.epochs);
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
        text_or_dash(&mut preimage, self.failed_reason.map(ErrorCode::as_wire));
        // Free text, which may itself be `-`, so its presence is a count rather than a
        // marker.
        count(
            &mut preimage,
            usize::from(self.non_resumable_reason.is_some()),
        );
        if let Some(reason) = &self.non_resumable_reason {
            preimage.text(reason);
        }
        text_or_dash(
            &mut preimage,
            self.continuation.as_ref().map(ContinuationHandle::as_str),
        );
        preimage.bytes().to_vec()
    }

    /// Whether `bytes` carry this record's format tag, so the startup pass reads them as a
    /// terminal record rather than as a campaign record.
    #[must_use]
    pub fn is_terminal_record(bytes: &[u8]) -> bool {
        Parts::new(bytes).text().is_ok_and(|tag| tag == FORMAT)
    }

    /// Decode the bytes a `task_*` index entry names.
    ///
    /// Strict, like the continuation record's decoder: every part at its declared width, no
    /// byte left over, every token in its vocabulary, the bytes the canonical encoding of what
    /// they decode to, and the ledger replays. A `Failed` record names its failure reason and
    /// a `Completed` one names none (RFC 0026: "a `Failed` task is never silent").
    ///
    /// # Errors
    ///
    /// The [`RecordDefect`] that names the first part that did not decode.
    pub fn decode(bytes: &[u8]) -> Result<Self, RecordDefect> {
        let mut parts = Parts::new(bytes);
        if parts.text()? != FORMAT {
            return Err(RecordDefect::Format);
        }
        let task = TaskHandle::new(parts.text()?).map_err(|_| RecordDefect::TaskHandle)?;
        let state = TerminalState::of(parts.text()?)?;
        let operation = OperationName::new(parts.text()?).map_err(|_| RecordDefect::Vocabulary)?;
        let snapshot = match parts.text()? {
            "-" => Nullable::Null,
            text => Nullable::Value(
                WorkspaceHandle::new(text).map_err(|_| RecordDefect::SnapshotHandle)?,
            ),
        };
        let intent = match parts.text()? {
            "-" => Nullable::Null,
            text => Nullable::Value(IntentHandle::new(text).map_err(|_| RecordDefect::Vocabulary)?),
        };
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
        let failed_reason = match parts.text()? {
            "-" => None,
            text => Some(ErrorCode::from_wire(text).map_err(|_| RecordDefect::Vocabulary)?),
        };
        let non_resumable_reason = match parts.u32()? {
            0 => None,
            1 => Some(parts.text()?.to_owned()),
            _ => return Err(RecordDefect::Vocabulary),
        };
        let continuation = match parts.text()? {
            "-" => None,
            text => Some(ContinuationHandle::new(text).map_err(|_| RecordDefect::Vocabulary)?),
        };
        if !parts.is_empty() {
            return Err(RecordDefect::TrailingBytes);
        }
        let record = Self {
            task,
            state,
            operation,
            snapshot,
            intent,
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
            failed_reason,
            non_resumable_reason,
            continuation,
        };
        if record.encode() != bytes {
            return Err(RecordDefect::NonCanonical);
        }
        // RFC 0026: `failed_reason` is REQUIRED when `status = failed`, and a completed task
        // carries none.
        if (record.state == TerminalState::Failed) != record.failed_reason.is_some() {
            return Err(RecordDefect::Vocabulary);
        }
        record.ledger()?;
        Ok(record)
    }

    fn ledger(&self) -> Result<continuum_task::budget::BudgetLedger, RecordDefect> {
        continuation::replay(
            &self.checkpoints,
            &self.spend,
            &self.ceilings,
            self.publications.len(),
        )
    }

    /// The live, terminal task this record restores.
    ///
    /// # Errors
    ///
    /// [`RecordDefect::Ledger`] when the ledger does not replay. Unreachable for a record
    /// [`Self::decode`] returned, which runs the same replay. [`RecordDefect::Unreceipted`]
    /// when `audit`'s ledger holds no receipt for a campaign record this record names
    /// (bn-283p6).
    pub fn restore(&self, audit: &StoreAudit<'_>) -> Result<TaskEntry, RecordDefect> {
        let ledger = self.ledger()?;
        let evidence = continuation::publications_of(&self.publications, &ledger, audit)?;
        Ok(TaskEntry {
            handle: self.task.clone(),
            operation: self.operation.clone(),
            status: self.state.status(),
            snapshot: self.snapshot.clone(),
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
            continuation: self.continuation.clone(),
            failed_reason: self.failed_reason,
            non_resumable_reason: self.non_resumable_reason.clone(),
            campaign: None,
            region: None,
            worker: None,
            evidence,
        })
    }

    /// The store identity this record is published under, through `identifier`.
    ///
    /// # Errors
    ///
    /// As [`budget::commitment_of`].
    pub fn commitment(&self, identifier: &dyn ContentIdentifier) -> Result<Commitment, Fault> {
        budget::commitment_of(identifier, &self.encode())
    }
}

/// Check that `projected` is the record of the task `entry` is now, after the change it
/// projected has been made.
///
/// The check that keeps the durable record and the live task from drifting apart, as
/// [`continuation::settle`] is for the continuation record. A disagreement is a defect in
/// this wiring, so it is a `debug_assert`, and the suites run every path that reaches it.
pub fn settle(entry: Option<&TaskEntry>, projected: &TerminalRecord) {
    debug_assert_eq!(
        entry.and_then(TerminalRecord::current).as_ref(),
        Some(projected),
        "the published terminal record is not the record of the task the daemon holds"
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::scalar::DurationMs;
    use crate::protocol::spec::Optional;
    use continuum_task::budget::dimension::CostDimension;

    fn states(value: u64) -> [Option<u64>; 9] {
        let mut spend = [None; 9];
        spend[CostDimension::States.index()] = Some(value);
        spend
    }

    fn record() -> TerminalRecord {
        TerminalRecord {
            task: TaskHandle::new("task_t").expect("a task handle"),
            state: TerminalState::Completed,
            operation: OperationName::new("verification.start").expect("an operation"),
            snapshot: Nullable::Value(WorkspaceHandle::new("ws_s").expect("a workspace handle")),
            intent: Nullable::Value(IntentHandle::new("in_i").expect("an intent handle")),
            target: Target {
                kind: TargetKind::AllClaims,
                id: "DieHard".to_owned(),
            },
            portfolio: Portfolio::Interactive,
            priority_class: PriorityClass::Interactive,
            epochs: EpochSet {
                protocol: ProtocolVersion::new(3, 1),
                semantic: Nullable::Null,
                intent: Nullable::Null,
                evidence: Nullable::Null,
                proof: Nullable::Null,
                corpus: Nullable::Null,
                engine: Nullable::Null,
            },
            model: Commitment::new("elab_m"),
            ceilings: Budget {
                states: Optional::Present(64),
                wall_ms: Optional::Present(DurationMs::new(5)),
                ..continuation::unbounded()
            },
            spend: states(16),
            checkpoints: vec![
                Checkpointed {
                    committed: 1,
                    spend: states(4),
                },
                Checkpointed {
                    committed: 2,
                    spend: states(16),
                },
            ],
            publications: vec![Commitment::new("task_p0"), Commitment::new("task_p1")],
            milestones: vec![Milestone {
                name: "exploration.closed".to_owned(),
                at: Timestamp::new("2026-08-01T00:00:00.000Z").expect("a timestamp"),
            }],
            committed_evidence: Vec::new(),
            failed_reason: None,
            non_resumable_reason: None,
            continuation: None,
        }
    }

    fn failed() -> TerminalRecord {
        TerminalRecord {
            state: TerminalState::Failed,
            spend: states(0),
            checkpoints: Vec::new(),
            publications: Vec::new(),
            milestones: Vec::new(),
            failed_reason: Some(ErrorCode::BudgetExhausted),
            // Free text that is also the absent marker of every other part.
            non_resumable_reason: Some("-".to_owned()),
            ceilings: Budget {
                states: Optional::Present(1),
                ..continuation::unbounded()
            },
            ..record()
        }
    }

    #[test]
    fn the_decoder_is_the_inverse_of_the_encoder_for_both_terminal_states() {
        let (store, operator) = crate::daemon::identity::testing::store();
        for spelling in ["task_p0", "task_p1"] {
            crate::daemon::identity::testing::publish(
                &store,
                &operator,
                spelling,
                Commitment::new(spelling),
            );
        }
        let audit = store.audit_view(&operator).expect("the operator audits");
        for record in [record(), failed()] {
            let bytes = record.encode();
            assert!(TerminalRecord::is_terminal_record(&bytes));
            assert_eq!(TerminalRecord::decode(&bytes), Ok(record.clone()));
            let entry = record.restore(&audit).expect("restores");
            assert_eq!(
                TerminalRecord::current(&entry),
                Some(record),
                "record → restore → record is the identity"
            );
        }
    }

    #[test]
    fn a_campaign_record_is_not_read_as_a_terminal_record() {
        let campaign = budget::publication_record(
            &TaskHandle::new("task_t").expect("a task handle"),
            0,
            Some("ws_s"),
            16,
            true,
            &[],
        );
        assert!(!TerminalRecord::is_terminal_record(&campaign));
        assert_eq!(TerminalRecord::decode(&campaign), Err(RecordDefect::Format));
    }

    #[test]
    fn a_record_that_does_not_decode_is_a_typed_defect_per_cause() {
        let good = record().encode();
        assert_eq!(
            TerminalRecord::decode(&good[..good.len() - 1]),
            Err(RecordDefect::Truncated)
        );
        let mut trailing = good.clone();
        trailing.extend_from_slice(&0_u64.to_be_bytes());
        assert_eq!(
            TerminalRecord::decode(&trailing),
            Err(RecordDefect::TrailingBytes)
        );
        let silent = TerminalRecord {
            failed_reason: None,
            ..failed()
        };
        assert_eq!(
            TerminalRecord::decode(&silent.encode()),
            Err(RecordDefect::Vocabulary),
            "a `Failed` record that names no reason is refused"
        );
        let unpaired = TerminalRecord {
            publications: vec![Commitment::new("task_p0")],
            ..record()
        };
        assert_eq!(
            TerminalRecord::decode(&unpaired.encode()),
            Err(RecordDefect::Ledger),
            "a checkpoint with no publication does not replay"
        );
    }
}
