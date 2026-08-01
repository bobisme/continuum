//! The `task` namespace: request and response structs for its 5 operations.
//!
//! Each struct is the IDL's anonymous body under the generated-name rule of the
//! IDL header: the operation name in PascalCase with the `Request`/`Response`
//! suffix. A named body (`response VerificationResult;`) has no struct here; the
//! registry entry points at the shared type instead.

use super::super::prelude::*;
use crate::protocol_struct;

protocol_struct! {
    /// The `request` body of `task.status`.
    struct TaskStatusRequest {
        /// IDL `task: TaskHandle required`.
        task: TaskHandle required;
    }
}

protocol_struct! {
    /// The `request` body of `task.cancel`.
    struct TaskCancelRequest {
        /// IDL `task: TaskHandle required`.
        task: TaskHandle required;
    }
}

protocol_struct! {
    /// The `response` body of `task.cancel`.
    struct TaskCancelResponse {
        /// IDL `task: TaskHandle required`.
        task: TaskHandle required;
        /// IDL `status: TaskStatus required`.
        status: TaskStatus required;
        /// IDL `continuation: ContinuationHandle nullable`.
        continuation: ContinuationHandle nullable;
        /// IDL `committed_evidence: list<EvidenceHandle> required`.
        committed_evidence: list<EvidenceHandle> required;
    }
}

protocol_struct! {
    /// The `request` body of `task.resume`.
    struct TaskResumeRequest {
        /// IDL `continuation: ContinuationHandle required`.
        continuation: ContinuationHandle required;
        /// IDL `budget: Budget optional`.
        budget: Budget optional;
    }
}

protocol_struct! {
    /// The `response` body of `task.resume`.
    struct TaskResumeResponse {
        /// IDL `task: TaskHandle required`.
        task: TaskHandle required;
        /// IDL `status: TaskStatus required`.
        status: TaskStatus required;
    }
}

protocol_struct! {
    /// The `request` body of `task.subscribe`.
    struct TaskSubscribeRequest {
        /// IDL `task: TaskHandle required`.
        task: TaskHandle required;
    }
}

protocol_struct! {
    /// The `response` body of `task.subscribe`.
    struct TaskSubscribeResponse {
        /// Snapshot of the record at subscription time; events follow.
        record: TaskRecord required;
    }
}

protocol_struct! {
    /// The `request` body of `task.update_budget`.
    struct TaskUpdateBudgetRequest {
        /// IDL `task: TaskHandle required`.
        task: TaskHandle required;
        /// IDL `budget: Budget required`.
        budget: Budget required;
    }
}

protocol_struct! {
    /// The `response` body of `task.update_budget`.
    struct TaskUpdateBudgetResponse {
        /// IDL `task: TaskHandle required`.
        task: TaskHandle required;
        /// IDL `status: TaskStatus required`.
        status: TaskStatus required;
        /// IDL `budget: Budget required`.
        budget: Budget required;
        /// Present when lowering the budget suspended the task.
        continuation: ContinuationHandle optional;
    }
}
