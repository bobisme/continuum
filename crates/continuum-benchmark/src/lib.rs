//! `continuum-benchmark` — ContinuumBench task families, metrics, and datasets (plan §19,
//! PR 30), and the ACI benchmark harness G0-DX-10 is decided on (PR 10).
//!
//! # Responsibility
//!
//! Task families, metrics, dataset construction, and the reward-hacking suite used to measure
//! whether agents genuinely verify or merely appear to.
//!
//! Benchmarks test governance, not just bug fixing: a patch that games the intent must score
//! as a failure.
//!
//! # What PR 10 landed here
//!
//! The instrument for the plan's freeze-blocking question, G0-DX-10: *can agents use the
//! protocol more effectively than CLI scraping?* Two arms drive one daemon over the same
//! benchmark tasks — the typed client in `continuum-mcp`, and a disciplined shell-scraping
//! baseline in [`shell`] — and five metrics are measured over them:
//!
//! | Bullet | Where it is defined | Where it is asserted |
//! |---|---|---|
//! | IMPL-01 valid operation rate | [`run`], IMPL-01 section | `tests/pr10_impl01_valid_operation_rate.rs` |
//! | IMPL-02 tokens/bytes | [`run`], IMPL-02 section | `tests/pr10_impl02_interface_bytes.rs` |
//! | IMPL-03 task completion | [`run`], IMPL-03 section | `tests/pr10_impl03_task_completion.rs` |
//! | IMPL-04 recovery from errors | [`run`], IMPL-04 section; [`policy::Fault`] | `tests/pr10_impl04_error_recovery.rs` |
//! | IMPL-05 deterministic reproduction | [`run`], IMPL-05 section | `tests/pr10_impl05_reproduction.rs` |
//!
//! plus the G0 staging gate the goal bone's criterion 2 names — the plan §19.4
//! family/source-hash [`separation`] check, which [`report::Report`] refuses to be `accepted`
//! without.
//!
//! # The shape of the experiment
//!
//! ```text
//! task × seed × policy × arm
//!   4  ×  3   ×   2    ×  2   = 48 runs
//! ```
//!
//! - **task** — [`task::SUBSET`], four tasks over the two corpus ports plan §21's Phase A
//!   exit names, with the dossier's frozen answers attached;
//! - **seed** — [`policy::SCHEDULES`], three *declared* schedules rather than three draws
//!   from a generator (INV-005: no ambient nondeterminism, anywhere, including here);
//! - **policy** — faithful and fallible, the second making the schedule's mistakes against
//!   the daemon's own error surfaces, which is IMPL-04's injection point;
//! - **arm** — the typed client and the shell baseline, driven by the *same* policy so that a
//!   measured difference is a difference in interface and not in how well an arm was written.
//!
//! # What this harness will not do
//!
//! **Decide.** It computes plan §24.5's ratified margins and prints them; the DX-10 verdict,
//! the matrix row, and any redesign decision belong to the exit bone and the falsification
//! bone. A harness that graded its own experiment would be self-certification in the small,
//! and [`report::Report`]'s `limits` section says so inside the artifact.
//!
//! **Simulate an agent's judgement.** The policies are state machines. That buys reproducible
//! bytes and costs the question "would a live model *discover* better use of the typed
//! surface?", which no scripted arm can answer. It is named in the artifact and in this
//! crate's tests rather than left for a reader to notice.
//!
//! # Dependency-boundary contract
//!
//! - May depend on Forge and on verifier interfaces — it is a harness, not part of the
//!   verifier.
//! - Owns no semantic state and publishes no trusted evidence of its own. Every fact in a
//!   [`report::Report`] was answered by a daemon or read off a corpus file, and the report is
//!   an artifact of this crate, never an evidence-graph claim.
//! - It does **not** depend on `continuum-mcp`. That crate is an adapter and adapters are
//!   sinks in the workspace graph (plan §20; `tools/check_crate_boundaries.py`), so the arm
//!   that drives it is wired in `tests/`. [`surface`] states exactly what that costs.

pub mod corpus;
pub mod philosophers;
pub mod policy;
pub mod report;
pub mod rig;
pub mod run;
pub mod separation;
pub mod shell;
pub mod surface;
pub mod task;

use continuumd::protocol::envelope::Budget;
use continuumd::protocol::scalar::{ByteCount, DurationMs};
use continuumd::protocol::spec::Optional;

/// The wall-clock ceiling both arms declare.
///
/// Declared and *not enforced*: this daemon meters `states` and nothing else, because a clock
/// is exactly what INV-005 keeps out of the deterministic core
/// (`continuumd::daemon::verification`). Declaring it is deliberate — it is what makes the
/// INV-007 omission manifest non-empty, and the manifest is the datum the two arms pay
/// differently for. See [`shell`]'s "one lossy rule".
pub const DECLARED_WALL_MS: u64 = 30_000;

/// The interface-byte ceiling both arms declare. Declared and not enforced, as above.
pub const DECLARED_BYTES: u64 = 1_048_576;

/// The wall-clock ceiling, for a renderer that has to print it.
#[must_use]
pub const fn declared_wall_ms() -> u64 {
    DECLARED_WALL_MS
}

/// The byte ceiling, for a renderer that has to print it.
#[must_use]
pub const fn declared_bytes() -> u64 {
    DECLARED_BYTES
}

/// The budget every `verification.start` and `task.resume` in this benchmark declares.
///
/// Three dimensions, identical on both arms. `states` is the ceiling the task sets and the
/// one this daemon can meter; `wall_ms` and `bytes` are declared and unmetered, so the daemon
/// answers with two typed omissions naming them — which is the INV-007 behaviour the harness
/// measures the cost of obtaining.
///
/// Holding the declaration identical across arms is what makes the comparison paired: plan
/// §24.5's ratified margins require "an identical base model, task set, and per-task budget".
#[must_use]
pub fn declared_budget(states: u64) -> Budget {
    Budget {
        wall_ms: Optional::Present(DurationMs::new(DECLARED_WALL_MS)),
        cpu_ms: Optional::Absent,
        memory_bytes: Optional::Absent,
        states: Optional::Present(states),
        solver_ms: Optional::Absent,
        proof_ms: Optional::Absent,
        tokens: Optional::Absent,
        candidates: Optional::Absent,
        bytes: Optional::Present(ByteCount::new(DECLARED_BYTES)),
    }
}
