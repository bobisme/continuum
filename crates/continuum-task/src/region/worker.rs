//! What a region owns: workers, the evidence they hold, and the typed outcomes
//! cancellation leaves behind.
//!
//! The state vocabulary here is not this module's invention. It is RFC 0026's
//! `TaskStatus`, spelled once:
//!
//! > `TaskStatus` (`created`, `running`, `suspended`, `completed`, `failed`,
//! > `cancelled`) is the task-lifecycle vocabulary.
//! >
//! > — `notes/plan/rfcs/0026-continuumd-native-protocol.md`, "Task lifecycle"
//!
//! [`WorkerState::status_token`] reports those six spellings, so the region layer and
//! `continuumd`'s wire vocabulary cannot drift into two names for one state. That method
//! is the whole of the seam: this crate declares no edge to `continuumd` (plan §20 gives
//! the daemon the protocol and gives this crate the lifecycle), and a shared token is
//! how two crates agree without one importing the other.

use core::fmt;

/// A worker's identity inside one [`RegionTree`](super::RegionTree).
///
/// Identities are dense ordinals assigned in spawn order from a counter the tree owns.
/// They are not drawn, hashed, or timed: a schedule that spawns the same workers in the
/// same order names the same identities, which is what makes a run replayable as data
/// (INV-005).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct WorkerId(u32);

impl WorkerId {
    /// The identity at `ordinal` in spawn order.
    #[must_use]
    pub const fn at(ordinal: u32) -> Self {
        Self(ordinal)
    }

    /// This worker's position in spawn order.
    #[must_use]
    pub const fn ordinal(self) -> u32 {
        self.0
    }
}

impl fmt::Display for WorkerId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "w{}", self.0)
    }
}

/// Where a worker is in the RFC 0026 lifecycle.
///
/// ```text
/// Created → Running → Suspended | Completed | Failed | Cancelled
///                        │
///                        └── continuation + committed partial evidence
/// ```
///
/// The six members split three ways, and the split is what every property in this module
/// is stated over:
///
/// - **active** — [`Self::Created`] and [`Self::Running`]. Work is owed. A region holding
///   one of these is not quiescent and cannot be finalized.
/// - **parked** — [`Self::Suspended`]. Nothing is running; a continuation and committed
///   partial evidence exist (RFC 0026: "a suspended task is resumable by definition").
/// - **terminal** — [`Self::Completed`], [`Self::Failed`], [`Self::Cancelled`]. RFC 0026's
///   `task.status` monotonicity: "a terminal status never changes".
///
/// *Quiescent* is parked ∪ terminal: the set in which nothing is running.
/// [`drain`](super::RegionTree::drain) and [`finalize`](super::RegionTree::finalize)
/// deliberately require the **stronger** predicate, [`Self::is_terminal`], and the reason
/// is worth stating once: a parked worker is quiescent, so finalizing over it would not
/// leave anything *running* — but its status can still change, and a finalized region
/// that still owns a resumable worker is a continuation pointing into a scope that no
/// longer exists. Requiring terminal makes the post-condition one sentence with no
/// carve-out, which is what "provably nothing is running" has to mean if a proof is
/// going to be short enough to read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkerState {
    /// Admitted into an open region; not yet stepped.
    ///
    /// This is *not* a synonym for "doing nothing harmlessly". A `Created` worker owes a
    /// terminal state exactly as a `Running` one does, so finalizing over it would leave
    /// an obligation nobody discharged — which is what an orphan is.
    Created,
    /// Stepping.
    ///
    /// Observable here, unlike in `continuumd`'s task table, where
    /// `Daemon::dispatch` holds `&mut DaemonState` for a whole call and no second
    /// dispatch can see it. The difference is deliberate: this model makes the
    /// interleaving explicit data, so the state a schedule can sit in is the state a
    /// cancellation can arrive in.
    Running,
    /// Parked with committed partial evidence and a continuation (B18).
    Suspended,
    /// Finished its work.
    Completed,
    /// Finished badly, naming why.
    ///
    /// > `TaskRecord.failed_reason` (an `ErrorCode`) is REQUIRED when `status = failed`:
    /// > **a `Failed` task is never silent**.
    /// >
    /// > — `notes/plan/rfcs/0026-continuumd-native-protocol.md`, "Task lifecycle"
    ///
    /// The payload is required rather than optional, so a silent failure does not
    /// compile.
    Failed(FailureReason),
    /// Terminated by a cancellation that reached it.
    Cancelled,
}

impl WorkerState {
    /// RFC 0026's own spelling of this state.
    ///
    /// The six tokens are `TaskStatus`'s six members. A caller projecting a worker onto
    /// the wire uses this rather than restating the vocabulary, which is the rule
    /// `continuum-value` states for the epoch and assurance vocabularies and the reason
    /// this crate has no `continuumd` edge.
    #[must_use]
    pub const fn status_token(&self) -> &'static str {
        match self {
            Self::Created => "created",
            Self::Running => "running",
            Self::Suspended => "suspended",
            Self::Completed => "completed",
            Self::Failed(_) => "failed",
            Self::Cancelled => "cancelled",
        }
    }

    /// Whether work is still owed: the worker has neither parked nor terminated.
    #[must_use]
    pub const fn is_active(&self) -> bool {
        matches!(self, Self::Created | Self::Running)
    }

    /// Whether the status can never change again (RFC 0026's monotonicity).
    #[must_use]
    pub const fn is_terminal(&self) -> bool {
        matches!(self, Self::Completed | Self::Failed(_) | Self::Cancelled)
    }

    /// Whether nothing is running: parked or terminal.
    ///
    /// Weaker than [`Self::is_terminal`], which is what
    /// [`finalize`](super::RegionTree::finalize) actually requires. Both are reported
    /// because they are genuinely different questions — "is anything running" and "can
    /// this status still change" — and collapsing them is how a parked worker gets
    /// finalized over.
    #[must_use]
    pub const fn is_quiescent(&self) -> bool {
        !self.is_active()
    }

    /// Whether a worker in this state may take part in an adapter obligation — open,
    /// discharge, hand on, or receive one: it has begun and has not terminated, so
    /// [`Self::Running`] or [`Self::Suspended`] (RFC 0026 correction 51).
    ///
    /// Not [`Self::is_active`]. A [`Self::Created`] worker has taken no step and holds
    /// nothing. A [`Self::Suspended`] worker still acts for the substrate: the substrate
    /// applies an open after the poll that made it returns, hands obligations between
    /// parked tasks, and discharges them during a cancellation before the holder's
    /// lifecycle is terminal. The calculus has no separate cancelling state — cancellation
    /// is its region's [`Draining`](super::RegionState::Draining) — so a cancelling worker
    /// is one of these two.
    #[must_use]
    pub const fn holds_substrate(&self) -> bool {
        matches!(self, Self::Running | Self::Suspended)
    }

    /// The typed failure reason, when this state is [`Self::Failed`].
    #[must_use]
    pub const fn failure_reason(&self) -> Option<&FailureReason> {
        match self {
            Self::Failed(reason) => Some(reason),
            _ => None,
        }
    }
}

impl fmt::Display for WorkerState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Failed(reason) => write!(f, "failed({reason})"),
            other => f.write_str(other.status_token()),
        }
    }
}

/// One advance of one worker — the only thing that moves a [`WorkerState`].
///
/// A step is data, never a closure. That is the same choice
/// `continuum-engine-reference`'s model layer made for actions and for the same reason:
/// a run is only replayable if the steps that produced it can be written down, compared,
/// and re-fed (INV-005). It is also what makes [`Schedule`](super::schedule::Schedule)
/// possible at all.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkerStep {
    /// `Created → Running`.
    Begin,
    /// Stage a provisional publication. Nothing a reader can observe exists yet.
    ///
    /// The counterpart in the real store is
    /// `continuum_workspace::publication::StagedPublication`, whose
    /// `PublicationPhase::Staging` doc says outright: "Authorized, content in hand,
    /// identity being derived. Nothing is stored."
    Reserve,
    /// Turn the staged publication into a committed one: an index entry a reader can
    /// observe, monotonic thereafter (INV-009).
    ///
    /// The counterpart is `CommittedContent::commit_index`, which is where the store
    /// stops being able to abandon without leaving a receipt.
    Commit,
    /// `Running → Suspended`: park with a continuation (B18).
    Suspend,
    /// `Suspended → Running`: take the continuation back up.
    Resume,
    /// `Running → Completed`.
    Complete,
    /// `Created | Running → Failed`, naming why.
    Fail(FailureReason),
    /// Drop the one staged publication: `Running → Running`, publishing nothing.
    ///
    /// The explicit-abort arm of the effect protocol — docs/02 §7's
    /// `Reserved(token) → Aborted(reason)` and RFC 0001's `Reserved ↘ Aborted` — taken by
    /// the worker itself rather than forced on it by a drain or a failure. It is legal only
    /// from [`WorkerState::Running`] and only while a publication is staged; with nothing
    /// staged it is refused as
    /// [`RegionFault::AbortWithoutReserve`](super::RegionFault::AbortWithoutReserve), and
    /// with more than one staged it is refused as ambiguous, like [`Self::Commit`]. The
    /// store-side counterpart is `StagedPublication::abandon`: no index entry, no receipt,
    /// nothing a reader can observe. Committed evidence is untouched (INV-009). RFC 0026
    /// correction 51 records the step.
    Abort,
    /// Stage a publication in a named slot, alongside any already staged in other slots.
    ///
    /// The multi-staging discipline (RFC 0026 correction 51): a worker MAY hold several
    /// staged publications at once, one per slot, and each is its own linear resource —
    /// its own `provisional-publication` obligation, resolved exactly once by
    /// [`Self::CommitSlot`], [`Self::AbortSlot`], or the drain or failure that discards it.
    /// Staging a slot that is already staged is refused. [`Self::Reserve`] is
    /// `ReserveSlot(PublicationSlot::PRIMARY)` under the stricter one-in-flight guard.
    ReserveSlot(PublicationSlot),
    /// Commit the publication staged in this slot.
    CommitSlot(PublicationSlot),
    /// Drop the publication staged in this slot, publishing nothing.
    AbortSlot(PublicationSlot),
    /// Request this one worker's cancellation, outside any region's cancellation: a
    /// deadline passing, for example (RFC 0026 correction 53).
    ///
    /// The step records the request and leaves the worker's state unchanged. It is legal
    /// for a worker that has not terminated and whose cancellation is not already
    /// requested, by its own request or by its region's. The worker then acts as before
    /// until it acknowledges the request.
    RequestCancel,
    /// The worker observes its requested cancellation and starts its cleanup: docs/02 §7
    /// `→ Cancelling`, the substrate's checkpoint that returns the cancellation error.
    ///
    /// Legal for a worker that has not terminated and whose cancellation is requested,
    /// by its own [`Self::RequestCancel`] or by its region's cancellation, and not yet
    /// acknowledged. The state does not change. After it the worker may only drain:
    /// abort what it staged, abort its adapter obligations, and complete as cancelled.
    AcknowledgeCancel,
    /// The worker finishes its cleanup and completes as cancelled, on its own, before any
    /// region drain: `Cancelling ─ … → Cancelled` for one worker (RFC 0026 correction 53).
    ///
    /// Legal only after [`Self::AcknowledgeCancel`]. It does what a cancelling drain does
    /// for this one worker: it discards what is still staged, records the
    /// [`CancelOutcome`], and settles the worker as [`WorkerState::Cancelled`].
    CompleteCancelled,
    /// A fail-stop crash stops the worker where it is, naming why: `Created | Running |
    /// Suspended → Failed`, in any cancellation phase (RFC 0026 correction 59; bn-fxxf2).
    ///
    /// No code of the worker runs at the step or after it: not its cancellation handler,
    /// not a cleanup, not a resume. That is what sets it apart from [`Self::Fail`], which
    /// is the worker's own failure and so leaves only `Created | Running`. A parked worker
    /// crashes from its park; a worker whose cancellation is requested or acknowledged
    /// crashes from there, because a crash is not a step of its cleanup. The process
    /// profile's row `fail-stop-crash` is the source: no finalizer, cancellation handler
    /// or cleanup runs at the crash.
    ///
    /// What the worker held is fenced (correction 58's row `late-completion`):
    ///
    /// - each staged publication is discarded, as a failure's is: nothing a reader can
    ///   observe exists, and it can never be committed. The discard discharges its
    ///   `provisional-publication` obligation, so nothing staged survives the step;
    /// - each adapter obligation the worker holds stays open in the ledger. It is owed,
    ///   and no one can discharge it or hand it on, because its holder is terminal. So a
    ///   region that finalizes over it is not total, and reports it as outstanding;
    /// - no [`CancelOutcome`] is recorded: the worker did not complete as cancelled.
    ///
    /// A binding may refuse a crash the calculus admits, when it gives that crash no
    /// fail-stop semantics (correction 58 item 1). The calculus states what a crash is; a
    /// binding states which crashes it realizes.
    Crash(FailureReason),
}

impl WorkerStep {
    /// A stable token for canonical rendering.
    #[must_use]
    pub const fn token(&self) -> &'static str {
        match self {
            Self::Begin => "begin",
            Self::Reserve => "reserve",
            Self::Commit => "commit",
            Self::Suspend => "suspend",
            Self::Resume => "resume",
            Self::Complete => "complete",
            Self::Fail(_) => "fail",
            Self::Abort => "abort",
            Self::ReserveSlot(_) => "reserve-slot",
            Self::CommitSlot(_) => "commit-slot",
            Self::AbortSlot(_) => "abort-slot",
            Self::RequestCancel => "request-cancel",
            Self::AcknowledgeCancel => "acknowledge-cancel",
            Self::CompleteCancelled => "complete-cancelled",
            Self::Crash(_) => "crash",
        }
    }

    /// The slot this step names, when it names one.
    #[must_use]
    pub const fn slot(&self) -> Option<PublicationSlot> {
        match self {
            Self::ReserveSlot(slot) | Self::CommitSlot(slot) | Self::AbortSlot(slot) => Some(*slot),
            _ => None,
        }
    }
}

impl fmt::Display for WorkerStep {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Fail(reason) => write!(f, "fail({reason})"),
            Self::Crash(reason) => write!(f, "crash({reason})"),
            Self::ReserveSlot(slot) | Self::CommitSlot(slot) | Self::AbortSlot(slot) => {
                write!(f, "{}({slot})", self.token())
            }
            other => f.write_str(other.token()),
        }
    }
}

/// Where one worker is in its own cancellation (RFC 0026 correction 53).
///
/// The per-task half of docs/02 §7's lifecycle, `Active ─ request → Cancelling`. A
/// worker's cancellation is requested by its own [`WorkerStep::RequestCancel`] or by
/// its region's cancellation. It is acknowledged by the worker's own
/// [`WorkerStep::AcknowledgeCancel`]. Only acknowledgement changes what the worker may
/// do: before it, the worker acts as it did before the request.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CancelPhase {
    /// No cancellation is requested.
    Active,
    /// A cancellation is requested and the worker has not observed it.
    Requested,
    /// The worker observed its cancellation and is cleaning up.
    Acknowledged,
}

impl CancelPhase {
    /// A stable token for canonical rendering.
    #[must_use]
    pub const fn token(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Requested => "requested",
            Self::Acknowledged => "acknowledged",
        }
    }
}

impl fmt::Display for CancelPhase {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.token())
    }
}

/// Which of a worker's staged publications a step names.
///
/// Slots are chosen by the caller and are scoped to one worker: `w0#1` and `w1#1` are two
/// different publications. They are data, never drawn or timed, so one schedule names one
/// set of slots on every run (INV-005). An adapter lifting a substrate that holds many
/// reservations per task gives each reservation its own slot — its reservation ordinal is
/// the natural choice. [`Self::PRIMARY`] is the slot the one-in-flight steps
/// ([`WorkerStep::Reserve`]) use, and its obligation is keyed by the worker alone, so a
/// ledger that never stages a second slot renders exactly as it did before slots existed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PublicationSlot(u32);

impl PublicationSlot {
    /// The slot [`WorkerStep::Reserve`] stages into.
    pub const PRIMARY: Self = Self(0);

    /// The slot at `ordinal`.
    #[must_use]
    pub const fn at(ordinal: u32) -> Self {
        Self(ordinal)
    }

    /// This slot's ordinal.
    #[must_use]
    pub const fn ordinal(self) -> u32 {
        self.0
    }
}

impl fmt::Display for PublicationSlot {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "#{}", self.0)
    }
}

/// What a worker has published, and what it still holds provisionally.
///
/// Two numbers rather than one enum, because a worker publishes more than once and the
/// two facts answer different questions: `committed` is monotone evidence (INV-009 —
/// "resuming a task may add evidence … it may not silently replace prior artifacts"),
/// and `provisional` is a publication *in progress*, which is the thing cancellation is
/// forbidden to truncate:
///
/// > Cancellation MUST NOT truncate a publication in progress: `task.cancel` finalizes
/// > or discards, never both halves.
/// >
/// > — `notes/plan/rfcs/0026-continuumd-native-protocol.md`, "Atomicity of publication"
///
/// Each staged publication is one of the store's linear typestates — `StagedPublication`
/// and `CommittedContent` are each consumed by the call that advances it — and a worker
/// may hold several of them at once, one per [`PublicationSlot`] (RFC 0026 correction
/// 51). The one-in-flight steps ([`WorkerStep::Reserve`], [`WorkerStep::Commit`],
/// [`WorkerStep::Abort`]) keep the stricter discipline in which at most one is staged.
/// This ledger counts them; which slots are staged is the region tree's record, and the
/// obligation ledger keys one obligation per slot, so the two accountings are independent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct EvidenceLedger {
    staged: u32,
    committed: u32,
}

impl EvidenceLedger {
    /// A worker that has published nothing and staged nothing.
    #[must_use]
    pub const fn empty() -> Self {
        Self {
            staged: 0,
            committed: 0,
        }
    }

    /// Whether a publication is staged and not yet resolved.
    #[must_use]
    pub const fn is_provisional(&self) -> bool {
        self.staged > 0
    }

    /// How many publications are staged and not yet resolved.
    #[must_use]
    pub const fn staged(&self) -> u32 {
        self.staged
    }

    /// How many publications this worker has committed.
    #[must_use]
    pub const fn committed(&self) -> u32 {
        self.committed
    }

    /// Whether this worker holds committed partial evidence.
    #[must_use]
    pub const fn has_committed_evidence(&self) -> bool {
        self.committed > 0
    }

    pub(crate) const fn reserve(&mut self) {
        self.staged += 1;
    }

    /// Resolve one staged publication by committing it. The caller has checked that one
    /// is staged.
    pub(crate) const fn commit(&mut self) {
        self.staged = self.staged.saturating_sub(1);
        self.committed += 1;
    }

    /// Resolve one staged publication by dropping it: nothing observable is left behind,
    /// and `committed` does not move. The caller has checked that one is staged.
    pub(crate) const fn abort(&mut self) {
        self.staged = self.staged.saturating_sub(1);
    }

    /// Drop every staged publication: nothing observable is left behind.
    ///
    /// The store's counterpart is `StagedPublication::abandon` /
    /// `CommittedContent::abandon`, which report `AbortReason::Abandoned` and — as
    /// `PublicationAborted`'s doc puts it — leave "no index entry, no receipt, nothing a
    /// reader can observe".
    pub(crate) const fn discard(&mut self) -> bool {
        let had = self.staged > 0;
        self.staged = 0;
        had
    }
}

/// Whether a worker's work can be picked up again, declared at spawn.
///
/// Declared rather than defaulted, because INV-005 makes explicitness the rule and
/// because the dossier has both cases: RFC 0026 requires `non_resumable_reason` "when
/// `failed_reason = BudgetExhausted` and no continuation exists", so a task that cannot
/// be resumed is a named outcome and not an accident. A caller that does not know is
/// making a claim either way; this type makes it write the claim down.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Resumability {
    /// The work can mint a continuation, so cancelling it after it committed evidence
    /// leaves [`CancelOutcome::CommittedWithContinuation`].
    Resumable,
    /// The work cannot be picked up again, and here is the typed reason RFC 0026's
    /// `non_resumable_reason` requires.
    NonResumable(NonResumableReason),
}

/// A continuation the region layer minted for a worker that held committed evidence when
/// cancellation reached it.
///
/// **This is not a `cont_*` handle.** RFC 0026 requires a real continuation to pin the
/// snapshot, the intent, the committed frontier and search state, all six compatibility
/// epochs and engine identity — none of which a region tree can see, and none of which it
/// will invent. What this type carries is the region-layer fact the daemon needs in order
/// to mint one: *which worker* is resumable and *how much* committed evidence it is
/// resumable from. Binding it to a `ContinuationHandle` is `continuumd`'s, and pinning a
/// continuation that omits an epoch its task consumed is malformed there, not here.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Continuation {
    worker: WorkerId,
    committed: u32,
}

impl Continuation {
    /// The continuation for `worker`, resuming from `committed` publications.
    ///
    /// Public because the value has to be comparable from outside the crate: a daemon
    /// translating a [`CancelOutcome`] into RFC 0026's nullable `continuation` field
    /// needs to state what it received, and a test asserting a cancellation table needs
    /// to write down what it expects. It carries no authority — minting one here does
    /// not make a region resumable, and the region tree mints its own.
    #[must_use]
    pub const fn new(worker: WorkerId, committed: u32) -> Self {
        Self { worker, committed }
    }

    /// The worker this continuation resumes.
    #[must_use]
    pub const fn worker(&self) -> WorkerId {
        self.worker
    }

    /// How many committed publications the resumed work starts from.
    #[must_use]
    pub const fn committed(&self) -> u32 {
        self.committed
    }
}

impl fmt::Display for Continuation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "cont({}, committed={})", self.worker, self.committed)
    }
}

/// What a cancellation left behind for one worker.
///
/// > **`task.cancel` triggers request → drain → finalize.** It MUST leave either
/// > committed partial evidence plus a valid continuation, or nothing published
/// > (INV-009, B19). Its response carries `continuation` as `nullable`, so "cancelled
/// > with nothing published" is a representable, named outcome rather than an inference
/// > from an absent field.
/// >
/// > — `notes/plan/rfcs/0026-continuumd-native-protocol.md`, "Task lifecycle"
///
/// The rule is a *both-or-neither*, and this enum is that rule as a type. There is no
/// constructor for "committed evidence and no continuation" and none for "a continuation
/// and nothing committed": the first is the leaked-obligation shape G0-DX-14 exists to
/// catch, and the second is a resume pointer into nothing. The third arm is the case
/// RFC 0026 names separately — committed evidence that genuinely cannot be resumed — and
/// it carries the reason rather than degrading to silence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CancelOutcome {
    /// Committed partial evidence plus a valid continuation.
    CommittedWithContinuation(Continuation),
    /// Committed partial evidence that cannot be resumed, naming why (RFC 0026's
    /// `non_resumable_reason`).
    CommittedNonResumable(NonResumableReason),
    /// Nothing published: no artifact a reader can observe, and no continuation.
    NothingPublished,
}

impl CancelOutcome {
    /// A stable token for canonical rendering.
    #[must_use]
    pub const fn token(&self) -> &'static str {
        match self {
            Self::CommittedWithContinuation(_) => "committed-with-continuation",
            Self::CommittedNonResumable(_) => "committed-non-resumable",
            Self::NothingPublished => "nothing-published",
        }
    }

    /// The continuation, when this outcome carries one.
    #[must_use]
    pub const fn continuation(&self) -> Option<Continuation> {
        match self {
            Self::CommittedWithContinuation(continuation) => Some(*continuation),
            _ => None,
        }
    }

    /// Whether anything a reader can observe was published.
    #[must_use]
    pub const fn published_anything(&self) -> bool {
        !matches!(self, Self::NothingPublished)
    }
}

impl fmt::Display for CancelOutcome {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CommittedWithContinuation(continuation) => {
                write!(f, "committed-with-continuation({continuation})")
            }
            Self::CommittedNonResumable(reason) => write!(f, "committed-non-resumable({reason})"),
            Self::NothingPublished => f.write_str("nothing-published"),
        }
    }
}

/// Why a worker failed.
///
/// A carrier, deliberately not a vocabulary. RFC 0026 requires `failed_reason` to be an
/// `ErrorCode` — the §10.3 taxonomy that lives in `continuumd`'s protocol module — and
/// plan §20 gives this crate no edge to `continuumd`. Minting a second closed enum of
/// failure codes here would be exactly the "second spelling" the workspace's
/// single-vocabulary rule exists to prevent, so this type carries the daemon's token and
/// checks only that it *is* a token: non-empty, printable ASCII, one spelling in machine
/// output. The validation mirrors
/// [`UnsupportedReason`](continuum_value::assurance::UnsupportedReason), which makes the
/// same trade for the same reason.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FailureReason(String);

impl FailureReason {
    /// Build a reason from its token.
    ///
    /// # Errors
    ///
    /// Returns [`ReasonError`] when the token is empty or is not printable ASCII.
    pub fn new(token: &str) -> Result<Self, ReasonError> {
        canonical_token(token).map(Self)
    }

    /// The reason token.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for FailureReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Why a worker's committed evidence cannot be resumed.
///
/// The region-layer carrier for RFC 0026's `non_resumable_reason`, held to the same
/// canonical-token shape as [`FailureReason`] and for the same reason.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NonResumableReason(String);

impl NonResumableReason {
    /// Build a reason from its token.
    ///
    /// # Errors
    ///
    /// Returns [`ReasonError`] when the token is empty or is not printable ASCII.
    pub fn new(token: &str) -> Result<Self, ReasonError> {
        canonical_token(token).map(Self)
    }

    /// The reason token.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for NonResumableReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Why a string is not a usable reason token.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ReasonError {
    /// The token was empty. A reason that says nothing is the silence RFC 0026's
    /// "a `Failed` task is never silent" forbids.
    Empty,
    /// The token contained a character outside printable ASCII, so it has more than one
    /// possible spelling in machine output.
    NonCanonical {
        /// Byte offset of the first offending character.
        index: usize,
        /// The first offending character.
        character: char,
    },
}

impl fmt::Display for ReasonError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => f.write_str("a reason token may not be empty"),
            Self::NonCanonical { index, character } => write!(
                f,
                "a reason token must be printable ASCII; byte {index} is {character:?}"
            ),
        }
    }
}

impl core::error::Error for ReasonError {}

fn canonical_token(token: &str) -> Result<String, ReasonError> {
    if token.is_empty() {
        return Err(ReasonError::Empty);
    }
    if let Some((index, character)) = token.char_indices().find(|(_, c)| !c.is_ascii_graphic()) {
        return Err(ReasonError::NonCanonical { index, character });
    }
    Ok(token.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reason(token: &str) -> FailureReason {
        FailureReason::new(token).expect("test token is canonical")
    }

    #[test]
    fn the_six_status_tokens_are_rfc_0026_task_status() {
        let states = [
            WorkerState::Created,
            WorkerState::Running,
            WorkerState::Suspended,
            WorkerState::Completed,
            WorkerState::Failed(reason("budget-exhausted")),
            WorkerState::Cancelled,
        ];
        let tokens: Vec<&str> = states.iter().map(WorkerState::status_token).collect();
        assert_eq!(
            tokens,
            vec![
                "created",
                "running",
                "suspended",
                "completed",
                "failed",
                "cancelled"
            ],
            "the region layer must spell TaskStatus exactly as RFC 0026 does"
        );
    }

    #[test]
    fn active_parked_and_terminal_partition_the_six_states() {
        let states = [
            WorkerState::Created,
            WorkerState::Running,
            WorkerState::Suspended,
            WorkerState::Completed,
            WorkerState::Failed(reason("engine-defect")),
            WorkerState::Cancelled,
        ];
        for state in &states {
            assert_ne!(
                state.is_active(),
                state.is_quiescent(),
                "{state} is both or neither, so the orphan predicate is ill-defined"
            );
        }
        assert!(WorkerState::Suspended.is_quiescent() && !WorkerState::Suspended.is_terminal());
    }

    #[test]
    fn an_empty_reason_token_is_refused() {
        assert_eq!(FailureReason::new(""), Err(ReasonError::Empty));
        assert_eq!(NonResumableReason::new(""), Err(ReasonError::Empty));
    }

    #[test]
    fn a_reason_token_with_a_space_is_refused_because_it_has_two_spellings() {
        assert_eq!(
            FailureReason::new("budget exhausted"),
            Err(ReasonError::NonCanonical {
                index: 6,
                character: ' '
            })
        );
    }

    #[test]
    fn discarding_a_staged_publication_leaves_nothing_observable() {
        let mut ledger = EvidenceLedger::empty();
        ledger.reserve();
        assert!(ledger.is_provisional());
        assert!(ledger.discard());
        assert!(!ledger.is_provisional());
        assert_eq!(ledger.committed(), 0, "a discard must publish nothing");
    }

    #[test]
    fn committing_is_monotone_and_clears_the_provisional_half() {
        let mut ledger = EvidenceLedger::empty();
        ledger.reserve();
        ledger.commit();
        ledger.reserve();
        ledger.commit();
        assert_eq!(ledger.committed(), 2);
        assert!(!ledger.is_provisional());
        assert!(ledger.has_committed_evidence());
    }

    #[test]
    fn a_cancel_outcome_cannot_pair_committed_evidence_with_a_missing_continuation() {
        let published =
            CancelOutcome::CommittedWithContinuation(Continuation::new(WorkerId::at(3), 2));
        assert!(published.published_anything());
        assert_eq!(published.continuation().map(|c| c.committed()), Some(2));

        let nothing = CancelOutcome::NothingPublished;
        assert!(!nothing.published_anything());
        assert_eq!(nothing.continuation(), None);
    }
}
