//! The operation register: research/25's semantic action grammar, as data.
//!
//! > Ship a machine-readable workflow grammar describing legal operation sequences and
//! > state transitions. An agent can validate a plan before calls; orchestrators can
//! > synthesize workflows; invalid operations become low-cost local feedback.
//! >
//! > — `notes/plan/research/25-agent-computer-interfaces-for-formal-systems.md`
//!
//! # The interface as a labelled transition system
//!
//! research/25 asks the ACI to be evaluated as `AgentState × ToolOperation → AgentState ×
//! Observation`. [`AgentContext`] is that `AgentState`, [`OPERATIONS`] is the transition
//! table, and [`admits`] is the guard. Three properties of the encoding are load-bearing:
//!
//! - **The state is the caller's, not the client's.** Plan §20 forbids an adapter from
//!   owning semantic state, so [`AgentContext`] is a value the caller holds and passes in
//!   per call. Two subagents sharing one workspace hold two contexts naming the same
//!   handles, with no session coupling (plan §10.5).
//! - **Preconditions are *necessary*, never merely typical.** research/25's own kill list
//!   includes "task grammar constrains legitimate strategies without reducing errors", so a
//!   requirement is stated here only where the daemon would certainly refuse without it —
//!   an operation naming a handle the caller does not have. Everything else is admitted and
//!   left to the daemon, which is the authority. `workspace.create` therefore has no
//!   precondition at all, because creating a second snapshot while holding a first is a
//!   legitimate strategy this grammar has no business blocking.
//! - **The transition is read off the *typed answer*, never guessed.**
//!   [`AgentContext::observe`] moves the state using `TaskRecord.status`, the envelope's
//!   `continuation`, and `WorkspaceCreateResponse.sealed` — values, not renderings. This is
//!   the whole of the ACI's claimed advantage over scraping, stated as one function.
//!
//! # Why a locally refused call still costs an attempt
//!
//! [`admits`] returning [`Err`] means no frame is written: no idempotency key is spent, no
//! audit record is made, and the interface-byte cost is zero. That is the "low-cost local
//! feedback" research/25 proposes. It is *not* a way to make an invalid-operation metric
//! look better, and [`Answer::attempted`](crate::Answer::attempted) is true on that arm for
//! exactly that reason: the agent still chose an operation whose preconditions did not
//! hold, and a benchmark that stopped counting those would be grading the interface's
//! tact.

use continuumd::daemon::family::Payload;
use continuumd::protocol::vocabulary::{ResultStatus, TaskStatus};

use crate::answer::Admitted;

/// What the caller holds a snapshot-shaped handle for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SnapshotState {
    /// The caller holds no snapshot handle.
    Absent,
    /// The caller holds an unsealed snapshot.
    Draft,
    /// The caller holds a sealed snapshot.
    Sealed,
}

/// What the caller holds a task-shaped handle for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CampaignState {
    /// The caller holds no task handle.
    Absent,
    /// A task exists and has not reported a terminal status.
    Live,
    /// A task suspended and the caller holds its continuation.
    Suspended,
    /// A task reached a terminal status: completed, failed, or cancelled.
    Settled,
}

/// The agent-side state the grammar is defined over.
///
/// Two fields, because the Phase A benchmark lane — snapshot, then campaign — is the whole
/// of what PR 10 drives. Debug, proof, repair and Forge handles are the later PRs' lanes and
/// are deliberately absent rather than stubbed: a grammar that named states no operation
/// here can reach would be describing an interface that does not exist.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AgentContext {
    /// What snapshot handle the caller holds.
    pub snapshot: SnapshotState,
    /// What task handle the caller holds.
    pub campaign: CampaignState,
}

impl AgentContext {
    /// The state before anything has been created.
    pub const EMPTY: Self = Self {
        snapshot: SnapshotState::Absent,
        campaign: CampaignState::Absent,
    };

    /// Move the state using the typed answer to `operation`.
    ///
    /// Every branch reads a typed field. Nothing here parses, and nothing here infers a
    /// transition from the *absence* of information: an answer that does not say what
    /// happened leaves the state where it was, which is the fail-closed direction for a
    /// caller that will next ask the grammar what it may do.
    pub fn observe(&mut self, operation: &str, admitted: &Admitted) {
        match operation {
            "workspace.create" => {
                self.snapshot = match &admitted.payload {
                    Payload::WorkspaceCreate(response) if response.sealed => SnapshotState::Sealed,
                    Payload::WorkspaceCreate(_) => SnapshotState::Draft,
                    _ => self.snapshot,
                };
            }
            "workspace.fork" => {
                if matches!(admitted.payload, Payload::WorkspaceFork(_)) {
                    // A fork is a new unsealed snapshot: `workspace.fork`'s response
                    // carries no `sealed` flag because a fork is never sealed on creation.
                    self.snapshot = SnapshotState::Draft;
                }
            }
            "workspace.seal" => {
                if matches!(admitted.payload, Payload::WorkspaceSeal(_)) {
                    self.snapshot = SnapshotState::Sealed;
                }
            }
            "verification.start" => {
                if admitted.task.is_some() {
                    self.campaign = if admitted.status == ResultStatus::TaskSuspended {
                        CampaignState::Suspended
                    } else {
                        CampaignState::Live
                    };
                }
            }
            "task.status" => {
                if let Payload::TaskStatus(record) = &admitted.payload {
                    self.campaign = match record.status {
                        TaskStatus::Created | TaskStatus::Running => CampaignState::Live,
                        TaskStatus::Suspended => CampaignState::Suspended,
                        TaskStatus::Completed | TaskStatus::Failed | TaskStatus::Cancelled => {
                            CampaignState::Settled
                        }
                    };
                }
            }
            "task.resume" => {
                if let Payload::TaskResume(response) = &admitted.payload {
                    self.campaign = match response.status {
                        TaskStatus::Created | TaskStatus::Running => CampaignState::Live,
                        TaskStatus::Suspended => CampaignState::Suspended,
                        TaskStatus::Completed | TaskStatus::Failed | TaskStatus::Cancelled => {
                            CampaignState::Settled
                        }
                    };
                }
            }
            "task.cancel" => {
                if matches!(admitted.payload, Payload::TaskCancel(_)) {
                    self.campaign = CampaignState::Settled;
                }
            }
            // `verification.result` reports; it moves nothing.
            _ => {}
        }
    }
}

/// What an operation needs the caller to already hold.
///
/// A closed vocabulary, and a small one: every member names a *handle* the request must
/// carry. Nothing here encodes policy, authority, or ordering preference — those are the
/// daemon's, and a client that second-guessed them would be the "authority encoded in
/// prose" research/25 asks an ACI not to have.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Requirement {
    /// Nothing: the operation is always legal.
    Nothing,
    /// Any snapshot handle, sealed or not.
    Snapshot,
    /// A sealed snapshot: verification runs over sealed state (plan §4.2).
    SealedSnapshot,
    /// A task handle.
    Task,
    /// A parked continuation.
    Continuation,
}

impl Requirement {
    /// Whether `context` satisfies this requirement.
    #[must_use]
    pub const fn holds(self, context: &AgentContext) -> bool {
        match self {
            Self::Nothing => true,
            Self::Snapshot => !matches!(context.snapshot, SnapshotState::Absent),
            Self::SealedSnapshot => matches!(context.snapshot, SnapshotState::Sealed),
            Self::Task => !matches!(context.campaign, CampaignState::Absent),
            Self::Continuation => matches!(context.campaign, CampaignState::Suspended),
        }
    }
}

/// One row of the grammar: an operation and what it requires.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OperationRule {
    /// The wire operation name.
    pub operation: &'static str,
    /// What the caller must already hold.
    pub requires: Requirement,
}

/// The grammar, in the order [`allowed`] reports it.
///
/// This is the machine-readable artifact research/25 asks for: eight rows, each an
/// operation and its precondition, readable by an orchestrator that wants to synthesize a
/// plan before spending a call. It is exactly the operations [`AgentClient`] exposes — a
/// grammar naming an operation the client cannot call would be a promise about a surface
/// that is not there.
///
/// [`AgentClient`]: crate::AgentClient
pub const OPERATIONS: &[OperationRule] = &[
    OperationRule {
        operation: "workspace.create",
        requires: Requirement::Nothing,
    },
    OperationRule {
        operation: "workspace.fork",
        requires: Requirement::Snapshot,
    },
    OperationRule {
        operation: "workspace.seal",
        requires: Requirement::Snapshot,
    },
    OperationRule {
        operation: "verification.start",
        requires: Requirement::SealedSnapshot,
    },
    OperationRule {
        operation: "verification.result",
        requires: Requirement::Task,
    },
    OperationRule {
        operation: "task.status",
        requires: Requirement::Task,
    },
    OperationRule {
        operation: "task.cancel",
        requires: Requirement::Task,
    },
    OperationRule {
        operation: "task.resume",
        requires: Requirement::Continuation,
    },
];

/// The rule for `operation`, when the register has one.
#[must_use]
pub fn rule(operation: &str) -> Option<&'static OperationRule> {
    OPERATIONS.iter().find(|rule| rule.operation == operation)
}

/// Whether `operation` may be called from `context`.
///
/// An operation the register does not name is admitted: this client's surface is a curated
/// subset of the protocol's seventy-three operations, and a grammar that refused everything
/// it had not been told about would be an authority rather than an aid.
///
/// # Errors
///
/// [`Unmet`] when the operation's precondition does not hold.
pub fn admits(context: &AgentContext, operation: &'static str) -> Result<(), Unmet> {
    match rule(operation) {
        Some(rule) if !rule.requires.holds(context) => Err(Unmet {
            operation,
            requires: rule.requires,
            context: *context,
        }),
        _ => Ok(()),
    }
}

/// Every operation legal from `context`, in [`OPERATIONS`] order.
///
/// Deterministic ordering is the point: plan §10.1 asks the ACI for "deterministic
/// ordering", and an orchestrator that planned against a set whose iteration order moved
/// would produce a different plan on every run.
#[must_use]
pub fn allowed(context: &AgentContext) -> Vec<&'static str> {
    OPERATIONS
        .iter()
        .filter(|rule| rule.requires.holds(context))
        .map(|rule| rule.operation)
        .collect()
}

/// The client's own refusal: an operation whose preconditions do not hold.
///
/// It carries the context it was judged against, so the feedback is diagnosable without a
/// second call — the caller can see both what was needed and what it had.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Unmet {
    /// The operation that was refused.
    pub operation: &'static str,
    /// What it needed.
    pub requires: Requirement,
    /// The state it was judged against.
    pub context: AgentContext,
}

impl core::fmt::Display for Unmet {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "{} requires {:?}, held {:?}/{:?}",
            self.operation, self.requires, self.context.snapshot, self.context.campaign
        )
    }
}
