//! Task records, continuations, and stream events (IDL §8).

use super::prelude::*;
use crate::protocol_struct;

protocol_struct! {
    /// A task record, as returned by `task.status`
    /// (`schemas/verification-task.schema.json` is the artifact form).
    struct TaskRecord {
        /// IDL `task: TaskHandle required`.
        task: TaskHandle required;
        /// IDL `operation: OperationName required`.
        operation: OperationName required;
        /// IDL `status: TaskStatus required`.
        status: TaskStatus required;
        /// IDL `snapshot: WorkspaceHandle nullable`.
        snapshot: WorkspaceHandle nullable;
        /// IDL `intent: IntentHandle nullable`.
        intent: IntentHandle nullable;
        /// Typed terminal reason; REQUIRED when `status = failed`. A `Failed`
        /// task is never silent (plan §4.5).
        failed_reason: ErrorCode optional;
        /// REQUIRED when `status = suspended`; a suspended task is resumable
        /// by definition (plan §11.4).
        continuation: ContinuationHandle optional;
        /// REQUIRED when `failed_reason = BudgetExhausted` and no continuation
        /// exists.
        non_resumable_reason: String optional;
        /// IDL `budget: Budget required`.
        budget: Budget required;
        /// IDL `cost: Cost required`.
        cost: Cost required;
        /// IDL `epochs: EpochSet required`.
        epochs: EpochSet required;
        /// IDL `priority_class: PriorityClass required`.
        priority_class: PriorityClass required;
        /// Semantic milestones reached so far, in order.
        milestones: list<Milestone> required;
        /// Evidence committed so far. Monotonic: entries are never removed.
        committed_evidence: list<EvidenceHandle> required;
    }
}

protocol_struct! {
    /// A named semantic milestone a task reached, and when.
    struct Milestone {
        /// IDL `name: String required`.
        name: String required;
        /// IDL `at: Timestamp required`.
        at: Timestamp required;
    }
}

protocol_struct! {
    /// A progress event on a task subscription. Events are hints; committed
    /// artifacts and `task.status` are authoritative.
    struct TaskEvent {
        /// IDL `task: TaskHandle required`.
        task: TaskHandle required;
        /// IDL `at: Timestamp required`.
        at: Timestamp required;
        /// IDL `kind: TaskEventKind required`.
        kind: TaskEventKind required;
        /// IDL `milestone: Milestone optional`.
        milestone: Milestone optional;
        /// IDL `cost: Cost optional`.
        cost: Cost optional;
        /// IDL `status: TaskStatus optional`.
        status: TaskStatus optional;
    }
}

protocol_struct! {
    /// A committed evidence-graph delta on an evidence subscription. Unlike
    /// task progress events, evidence deltas reference committed artifacts.
    struct EvidenceEvent {
        /// IDL `at: Timestamp required`.
        at: Timestamp required;
        /// IDL `kind: EvidenceEventKind required`.
        kind: EvidenceEventKind required;
        /// IDL `node: EvidenceHandle optional`.
        node: EvidenceHandle optional;
        /// IDL `edge: EvidenceHandle optional`.
        edge: EvidenceHandle optional;
        /// Status after the transition, for `status_transition` events.
        status: EvidenceStatus optional;
        /// The claim whose status changed.
        claim_id: String optional;
    }
}
