//! What an arm is, what it reports, and where each arm's code lives.
//!
//! # The seam
//!
//! A [`Surface`] performs one [`Step`] and reports an [`Observation`]: was the operation
//! admitted, what typed code refused it, what did the arm manage to *read*, and what did the
//! exchange cost in interface bytes. Everything above this seam — the policy, the metric
//! definitions, the completion criterion, the byte accounting, the transcript — is shared,
//! so no arm can be advantaged by how its driver was written.
//!
//! # Where the two arms live, and why they are not in the same file
//!
//! The shell baseline is [`crate::shell::ShellSurface`], in this library.
//!
//! The native arm is wired in `tests/`, and that is a plan §20 consequence rather than a
//! preference. The typed client is `continuum-mcp`; `continuum-mcp` is an **adapter**;
//! adapters are sinks in the workspace graph, so no crate may take a normal dependency on
//! one (`tools/check_crate_boundaries.py`, RULE adapters-are-sinks, the mechanical shadow of
//! §20's "adapters do not own semantic state"). A `tests/` file is its own crate and links
//! the client as a dev-dependency, which that checker reports rather than enforces, by its
//! own documented design.
//!
//! The cost of that is one `match` per test binary and it is worth naming precisely: the
//! **delegation** is test-side, the **measurement** is not. Attempt counting, admission
//! classification, byte accounting, view folding, completion, and transcript rendering all
//! happen in [`crate::run`], identically for both arms, over this trait. An arm's own code
//! decides only which client method to call and what it could read back.
//!
//! # What an arm may not do
//!
//! Reach around the interface. The shell arm may not read a typed field, and the native arm
//! may not read a rendered line. Both are given the same [`Rig`] because both drive the same
//! daemon, and `Rig::daemon` — the one-layer-down accessor — is used by *evidence tests* to
//! corroborate numbers that have no wire home, never by an arm during a run.

use continuumd::protocol::scalar::{ContinuationHandle, TaskHandle, WorkspaceHandle};
use continuumd::protocol::vocabulary::{ErrorCode, SemanticVerdict, TaskStatus};

use crate::policy::Step;
use crate::rig::Rig;
use crate::task::BenchmarkTask;

/// Which arm of the comparison.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Arm {
    /// The typed client: `continuum-mcp` over `continuumd`'s wire.
    Native,
    /// The disciplined shell baseline: a CLI-style text projection, scraped.
    Shell,
}

impl Arm {
    /// Both arms, in report order.
    pub const ALL: [Self; 2] = [Self::Native, Self::Shell];

    /// The stable token a report writes this arm as.
    #[must_use]
    pub const fn token(self) -> &'static str {
        match self {
            Self::Native => "native",
            Self::Shell => "shell",
        }
    }
}

/// What an arm recovered from one answer.
///
/// Every field is [`Option`], and that is the measurement: the native arm fills a field
/// because the wire declares it; the shell arm fills it because it found it in a rendering.
/// A `None` is not an error — it is the interface failing to hand something over, and the
/// policy responds to it the way an agent does.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Reading {
    /// A snapshot handle the answer named.
    pub snapshot: Option<WorkspaceHandle>,
    /// Whether that snapshot is sealed.
    pub sealed: Option<bool>,
    /// A task handle the answer named.
    pub task: Option<TaskHandle>,
    /// A task's status.
    pub status: Option<TaskStatus>,
    /// A continuation the answer named.
    pub continuation: Option<ContinuationHandle>,
    /// Whether the answer *decided* the continuation question at all.
    ///
    /// Three-valued, because "no continuation was parked" and "this answer does not say" are
    /// different facts and an agent that conflated them would drop a resumable campaign. The
    /// wire distinguishes them structurally — a task record's `continuation` is `optional`,
    /// and its absence on a `task.status` answer is a statement. A rendering has to say so in
    /// words, and [`crate::shell`] does.
    pub continuation_known: bool,
    /// The reachable-state count.
    pub states: Option<u64>,
    /// The campaign's verdict.
    pub verdict: Option<SemanticVerdict>,
    /// How many omissions the answer declared (INV-007).
    pub omissions: Option<usize>,
    /// Whether the state ceiling the agent declared was actually metered.
    ///
    /// Read off the INV-007 omission manifest: the daemon reports a declared-and-unmetered
    /// dimension as an omission whose subject is `budget.<token>`
    /// (`continuumd::daemon::budget::omissions_of`), so "no omission names `budget.states`"
    /// *is* "the ceiling was enforced". The typed arm has the manifest inline in the answer
    /// it already received; the shell arm has a count and an expansion command. This one
    /// field is where that difference is paid for, and [`crate::shell`]'s module
    /// documentation says why it is the honest place to pay it.
    pub ceiling_enforced: Option<bool>,
}

/// One arm's account of one attempted operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Observation {
    /// Whether the daemon admitted the operation.
    pub admitted: bool,
    /// The typed refusal code, when the arm could determine one.
    pub code: Option<ErrorCode>,
    /// Whether the *interface* refused before the daemon saw the call.
    ///
    /// Only the typed surface can do this — it is research/25's "invalid operations become
    /// low-cost local feedback". It is still a failed attempt and is counted as one; what it
    /// is not is a *cost*, and [`Observation::bytes`] is zero on it.
    pub local_refusal: bool,
    /// Interface bytes this attempt spent, both directions.
    pub bytes: u64,
    /// What the arm read back.
    pub reading: Reading,
    /// The canonical transcript line for this attempt.
    ///
    /// Deterministic and byte-stable: `crate::run`'s reproduction check compares two runs'
    /// transcripts byte for byte, so nothing here may carry an address, an iteration order,
    /// or a length that depends on anything but the exchange.
    pub line: String,
}

/// Why an arm could not attempt an operation at all.
///
/// Not a refusal and not an unreadable answer: a [`SurfaceError`] means this crate is wrong —
/// a request that would not encode, a wire that broke, a self-contradicting envelope. Runs do
/// not continue past one, because a benchmark that scored around its own defects would be
/// measuring them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SurfaceError {
    /// Which arm failed.
    pub arm: Arm,
    /// The operation it was attempting.
    pub operation: &'static str,
    /// What went wrong, in this crate's own words.
    pub detail: String,
}

impl core::fmt::Display for SurfaceError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "{} arm could not attempt {}: {}",
            self.arm.token(),
            self.operation,
            self.detail
        )
    }
}

impl core::error::Error for SurfaceError {}

/// One arm of the comparison.
pub trait Surface {
    /// Which arm this is.
    fn arm(&self) -> Arm;

    /// Attempt one operation and report what happened.
    ///
    /// # Errors
    ///
    /// [`SurfaceError`] when the arm could not attempt the operation at all.
    fn perform(
        &mut self,
        rig: &mut Rig,
        task: &BenchmarkTask,
        step: &Step,
    ) -> Result<Observation, SurfaceError>;

    /// Interface bytes this arm has spent opening its connection.
    ///
    /// Counted once per run and added to the run's total, because a surface that needed a
    /// large negotiation to start would be paying a cost a per-operation total would hide.
    fn handshake_bytes(&self) -> u64;
}
