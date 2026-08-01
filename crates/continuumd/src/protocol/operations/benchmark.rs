//! The `benchmark` namespace: request and response structs for its 1 operation.
//!
//! Each struct is the IDL's anonymous body under the generated-name rule of the
//! IDL header: the operation name in PascalCase with the `Request`/`Response`
//! suffix. A named body (`response VerificationResult;`) has no struct here; the
//! registry entry points at the shared type instead.

use super::super::prelude::*;
use crate::protocol_struct;

protocol_struct! {
    /// The `request` body of `benchmark.run`.
    struct BenchmarkRunRequest {
        /// Benchmark task (`schemas/benchmark-task.schema.json`).
        task_id: String required;
        /// Grader identities to apply; the daemon MUST reject any grader
        /// not registered for the task.
        graders: list<String> required;
    }
}

protocol_struct! {
    /// The `response` body of `benchmark.run`.
    struct BenchmarkRunResponse {
        /// IDL `task: TaskHandle optional`.
        task: TaskHandle optional;
        /// IDL `evidence: list<EvidenceHandle> required`.
        evidence: list<EvidenceHandle> required;
    }
}
