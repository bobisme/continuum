//! Where the daemon's budget accounting lives: one [`BudgetLedger`] per task, the wire
//! projections off it, and the names its committed publications carry (PR 6 / IMPL-02).
//!
//! > The daemon enforces per-principal and per-transaction cost ceilings; exceeding one
//! > yields `BudgetExhausted` with a continuation — never a silently smaller campaign (the
//! > cost-domain form of INV-007).
//! >
//! > — `notes/plan/plan.md` §8.6
//!
//! [`continuum_task::budget`] is the calculus that sentence names — a ledger over the nine
//! SD-12 dimensions, a `MeterSet` separating *declared* from *enforced*, and
//! `rule task.update_budget`'s legality table as a five-arm value. bn-1gc landed it with
//! the daemon seam written down and deliberately not wired; this module is that wiring,
//! and it is small on purpose: the accounting lives one crate down, and what happens here
//! is that the daemon's budget answers become *projections* of it rather than second
//! readings of the same fields.
//!
//! # Four projections, and one thing that is not a projection
//!
//! | Wire answer | Was | Is |
//! |---|---|---|
//! | `TaskRecord.budget` | a `Budget` field beside the ledger | [`wire_budget`] off [`BudgetLedger::budget`] |
//! | the engine's `Bounds` | `verification::bounds_of(&Budget)`, a second read of `budget.states` | [`bounds_of`], the ledger's `states` ceiling |
//! | `TaskRecord.cost` | `campaign.states()` read straight off the campaign | [`cost_of`], the ledger's [`Spend`] |
//! | the INV-007 omission manifest | `verification::unenforced`, an eight-name array | [`omissions_of`], derived from [`MeterSet::STATES_ONLY`] |
//!
//! The last row is the one worth stating twice. bn-18z's `unenforced` walked eight
//! hand-written `("budget.wall_ms", …)` pairs, so a daemon that gained a clock would keep
//! reporting `budget.wall_ms` as unenforced until somebody remembered to delete a line.
//! [`omissions_of`] asks the ledger, the ledger asks the meter set, and the meter set is
//! [`METERS`] — one `const` naming what this daemon can actually measure. Adding a meter
//! there removes an omission everywhere, by derivation.
//!
//! What is *not* a projection is [`Publications`]: the region layer and the budget layer
//! both **count** commitments and neither can name one, because `continuum-task` declares
//! no `continuum-workspace` edge and plan §20 states none. Artifact identity is therefore
//! the daemon's to add, and this module is where it is added.
//!
//! # Charging the delta, not the run
//!
//! `bfs::explore` has no partial-state entry point, so a resumed campaign re-explores from
//! the start and its state count *includes* the parked one — breadth-first admission makes
//! the parked explored set a prefix of the resumed one. A daemon that charged
//! `campaign.states()` on every run would therefore report 3 + 16 for a Die Hard campaign
//! that parked at 3 and closed at 16, which is not what it cost.
//!
//! [`charge_states`] charges the **difference** instead, so recorded spend tracks the
//! high-water mark of the walk: monotone, never double-counted, and equal to
//! `campaign.states()` at every point a caller can read it. That is what keeps
//! `TaskRecord.cost.states` the number it has always been while making the ledger, rather
//! than the campaign, the thing it is read off.
//!
//! # A ceiling below committed spend is recorded and is not a bound
//!
//! [`UpdateOutcome::Suspend`](continuum_task::budget::UpdateOutcome::Suspend) is
//! `rule task.update_budget`'s second sentence: lowering a dimension below committed spend
//! parks the task with committed partial evidence plus a continuation (B18), and "silent
//! truncation of a campaign is prohibited (INV-009)". The ceiling is still *recorded* —
//! `TaskRecord.budget` reports the budget in force, and a suspension that left the old
//! ceiling in place would report a bound the caller has withdrawn — but it is not an
//! enforceable bound on the current run, so [`bounds_of`] floors the engine's state bound
//! at the spend already recorded.
//!
//! Without that floor the lowering is a truncation with extra steps: a task parked at 3
//! states whose ceiling is lowered to 2 would resume under `Bounds::states = 2`, come back
//! with a *smaller* campaign, and overwrite a monotone `cost.states` with a lower number.
//! The floor is the difference between "the new ceiling admits no further progress" and
//! "the new ceiling un-explores what was already explored".

use continuum_engine_reference::bfs::Bounds;
use continuum_engine_reference::model::State;
use continuum_task::budget::dimension::{Ceiling, CostDimension, MeterSet};
use continuum_task::budget::partial::Checkpoint;
use continuum_task::budget::{Budget as LedgerBudget, BudgetLedger, ChargeOutcome, UpdateOutcome};
use continuum_workspace::artifact_path::ArtifactClass;
use continuum_workspace::publication::{ContentIdentifier, Published};

use super::family::Fault;
use super::task::{Preimage, unsupported};
use crate::protocol::envelope::{ArtifactRef, Budget, Cost, Omission};
use crate::protocol::scalar::{ArtifactHandle, ByteCount, Commitment, DurationMs, TaskHandle};
use crate::protocol::spec::Optional;
use crate::protocol::vocabulary::ErrorCode;

/// What this daemon can measure.
///
/// > `states` is the one dimension with an enforcement path; depth and transitions have no
/// > wire dimension […] `wall_ms` and `cpu_ms` need a clock.
/// >
/// > — this daemon's own [`verification`](super::verification) module
///
/// One `const`, and every omission this daemon reports about a budget is a consequence of
/// it. A deployment that grows a clock changes this line and the manifest follows.
pub const METERS: MeterSet = MeterSet::STATES_ONLY;

/// The ledger a wire budget opens.
///
/// Both halves are stated rather than defaulted, which is what
/// [`BudgetLedger::new`] takes them as parameters for: the budget is what the caller asked
/// for, and [`METERS`] is what this daemon can check.
#[must_use]
pub fn ledger_of(budget: &Budget) -> BudgetLedger {
    BudgetLedger::new(ceilings_of(budget), METERS)
}

/// The nine ceilings a wire budget declares.
fn ceilings_of(budget: &Budget) -> LedgerBudget {
    let mut declared = LedgerBudget::unbounded();
    for (dimension, limit) in dimensions_of(budget) {
        if let Some(limit) = limit {
            declared = declared.with(dimension, limit);
        }
    }
    declared
}

/// The wire budget's nine dimensions, in SD-12 declaration order.
///
/// One list, read once. The IDL's `struct Budget` is nine `optional` fields of three
/// different scalar types, and every function here that walks them walks this.
fn dimensions_of(budget: &Budget) -> [(CostDimension, Option<u64>); 9] {
    [
        (
            CostDimension::WallMs,
            budget.wall_ms.value().map(|value| value.millis()),
        ),
        (
            CostDimension::CpuMs,
            budget.cpu_ms.value().map(|value| value.millis()),
        ),
        (
            CostDimension::MemoryBytes,
            budget.memory_bytes.value().map(|value| value.bytes()),
        ),
        (CostDimension::States, budget.states.value().copied()),
        (
            CostDimension::SolverMs,
            budget.solver_ms.value().map(|value| value.millis()),
        ),
        (
            CostDimension::ProofMs,
            budget.proof_ms.value().map(|value| value.millis()),
        ),
        (CostDimension::Tokens, budget.tokens.value().copied()),
        (
            CostDimension::Candidates,
            budget.candidates.value().copied(),
        ),
        (
            CostDimension::Bytes,
            budget.bytes.value().map(|value| value.bytes()),
        ),
    ]
}

/// The wire spelling of the ceilings a ledger holds.
///
/// The inverse of [`ceilings_of`], and lossless in both directions: every wire dimension is
/// an `optional` `u64` under one of three newtypes, and [`Ceiling`] is the same information
/// with the absent case named. That is what lets a [`TaskEntry`](super::task::TaskEntry)
/// hold the ledger *instead of* a budget rather than beside one — two fields carrying one
/// fact are two answers to what a task may spend.
#[must_use]
pub fn wire_budget(ledger: &BudgetLedger) -> Budget {
    let declared = ledger.budget();
    let limit = |dimension: CostDimension| declared.ceiling(dimension).limit();
    Budget {
        wall_ms: optional(limit(CostDimension::WallMs).map(DurationMs::new)),
        cpu_ms: optional(limit(CostDimension::CpuMs).map(DurationMs::new)),
        memory_bytes: optional(limit(CostDimension::MemoryBytes).map(ByteCount::new)),
        states: optional(limit(CostDimension::States)),
        solver_ms: optional(limit(CostDimension::SolverMs).map(DurationMs::new)),
        proof_ms: optional(limit(CostDimension::ProofMs).map(DurationMs::new)),
        tokens: optional(limit(CostDimension::Tokens)),
        candidates: optional(limit(CostDimension::Candidates)),
        bytes: optional(limit(CostDimension::Bytes).map(ByteCount::new)),
    }
}

/// The engine bounds a ledger declares.
///
/// `states` is the one dimension with an enforcement path; depth and transitions have no
/// wire dimension and take the kernel wire form's own ceilings through `Bounds::CERTIFIABLE`.
/// A budget declaring no `states` therefore runs at the certifiable ceiling rather than at a
/// default nobody reviewed — the ceiling is *declared*, in `bfs`'s own constants, which is
/// what that module's "no silent caps" rule asks for.
///
/// The floor at recorded spend is this module's header: a ceiling below committed spend is
/// [`UpdateOutcome::Suspend`], and a suspension parks a campaign rather than un-exploring
/// it.
#[must_use]
pub fn bounds_of(ledger: &BudgetLedger) -> Bounds {
    bounds_after(ledger, None)
}

/// The bounds [`bounds_of`] would read after [`update`] with `budget` — computed without the
/// write, so a caller can decide what the next run would do before it changes anything.
///
/// [`update`] sets every dimension's ceiling to the budget's, an absent one to unbounded, so
/// the state ceiling after it is exactly `budget.states`; with no budget it is the ledger's.
#[must_use]
pub fn bounds_after(ledger: &BudgetLedger, budget: Option<&Budget>) -> Bounds {
    let ceiling = match budget {
        Some(budget) => budget.states.value().copied(),
        None => ledger.budget().ceiling(CostDimension::States).limit(),
    };
    match ceiling {
        Some(limit) => {
            let committed = ledger.spend().measured(CostDimension::States).unwrap_or(0);
            Bounds::CERTIFIABLE
                .with_states(usize::try_from(limit.max(committed)).unwrap_or(usize::MAX))
        }
        None => Bounds::CERTIFIABLE,
    }
}

/// The budget dimensions a caller declared and this daemon does not enforce (INV-007).
///
/// Derived, never maintained: [`BudgetLedger::omissions`] is the declared-and-unmetered
/// cell of [`dimension`](continuum_task::budget::dimension)'s 2×2, and the subject and
/// reason are the ledger's own — `budget.<token>` and `unsupported`, the spellings bn-18z's
/// hand-written array already emitted. Reason `unsupported` rather than `budget`: reason
/// `budget` means a ceiling caused something to be left out of the answer, and this says
/// the daemon has no meter for the dimension at all.
#[must_use]
pub fn omissions_of(ledger: &BudgetLedger) -> Vec<Omission> {
    ledger
        .omissions()
        .into_iter()
        .map(|omission| {
            debug_assert_eq!(
                omission.reason_token(),
                "unsupported",
                "the ledger's omission reason is the wire's `unsupported`"
            );
            unsupported(&omission.subject())
        })
        .collect()
}

/// What a task spent, projected off its ledger.
///
/// > A dimension the engine does not measure is absent, never zero.
/// >
/// > — `Cost`, IDL §6
///
/// `measured` is the projection: an unmetered dimension reads [`None`] on [`Spend`] and
/// absent here, so the presence distinction survives the crossing rather than being
/// re-decided on this side.
///
/// The `reading` gate is the other half of the same rule. A metered dimension reads
/// `Some(0)` from the moment a ledger exists — "we looked and it was nothing" — and this
/// daemon's one meter is the engine's own state count, which reads when a campaign comes
/// back and not before. A task that never produced one therefore reports *absent*: the
/// missing measurement, not a measurement of nothing.
#[must_use]
pub fn cost_of(ledger: &BudgetLedger, reading: bool) -> Cost {
    let mut cost = super::result::unmeasured();
    if !reading {
        return cost;
    }
    let spend = ledger.spend();
    let measured = |dimension: CostDimension| spend.measured(dimension);
    cost.wall_ms = optional(measured(CostDimension::WallMs).map(DurationMs::new));
    cost.cpu_ms = optional(measured(CostDimension::CpuMs).map(DurationMs::new));
    cost.memory_bytes = optional(measured(CostDimension::MemoryBytes).map(ByteCount::new));
    cost.states = optional(measured(CostDimension::States));
    cost.solver_ms = optional(measured(CostDimension::SolverMs).map(DurationMs::new));
    cost.proof_ms = optional(measured(CostDimension::ProofMs).map(DurationMs::new));
    cost.tokens = optional(measured(CostDimension::Tokens));
    cost.candidates = optional(measured(CostDimension::Candidates));
    cost.bytes = optional(measured(CostDimension::Bytes).map(ByteCount::new));
    // `tokenizer_id` is REQUIRED whenever `tokens` is reported, and this daemon meters no
    // tokens, so it stays absent — never a placeholder beside an absent count.
    debug_assert!(
        cost.tokens.is_absent(),
        "a reported token count owes a tokenizer identity"
    );
    cost
}

/// Record a campaign's state count against `ledger`, charging only what is new.
///
/// See this module's header: a resumed walk's count includes the parked one, so the charge
/// is the difference and the recorded spend is the high-water mark. Returns the outcome so
/// a caller can see an exhaustion rather than infer one; a charge that cannot land is
/// [`None`], which is unreachable because `bfs` refuses the expansion that would carry the
/// explored set past `Bounds::states` and [`bounds_of`] never sets that below the ceiling.
pub fn charge_states(ledger: &mut BudgetLedger, states: u64) -> Option<ChargeOutcome> {
    let recorded = ledger
        .spend()
        .measured(CostDimension::States)
        .unwrap_or_default();
    ledger
        .charge(CostDimension::States, states.saturating_sub(recorded))
        .ok()
}

/// Apply `budget` to `ledger`, one dimension at a time, and report what each did.
///
/// Nine outcomes in SD-12 declaration order, because `rule task.update_budget`'s legality
/// table is per *dimension*: one call can raise `states`, tighten `bytes` and record a
/// ceiling on `wall_ms` that nothing meters, and an answer that collapsed those to a single
/// verdict would have to pick one of them to report.
#[must_use]
pub fn update(ledger: &mut BudgetLedger, budget: &Budget) -> Vec<UpdateOutcome> {
    dimensions_of(budget)
        .into_iter()
        .map(|(dimension, limit)| {
            let ceiling = limit.map_or(Ceiling::Unbounded, Ceiling::At);
            ledger.update(dimension, ceiling)
        })
        .collect()
}

/// Whether [`update`] with `budget` would land any dimension on the `Suspend` arm.
///
/// The projection a durable write needs *before* the ledger changes (bn-20142): the
/// continuation record of a parked task is published first, and it carries the milestone
/// the suspension records. It restates [`BudgetLedger::update`]'s arm order — an equal
/// ceiling is `Unchanged` even below spend, and an unmetered dimension is `Unenforced` —
/// and `task.update_budget` checks it against the outcomes the update then returns.
#[must_use]
pub fn would_suspend(ledger: &BudgetLedger, budget: &Budget) -> bool {
    dimensions_of(budget).into_iter().any(|(dimension, limit)| {
        let Some(after) = limit else { return false };
        let Some(spent) = ledger.spend().measured(dimension) else {
            return false;
        };
        ledger.budget().ceiling(dimension).limit() != Some(after) && after < spent
    })
}

// ---------------------------------------------------------------------------
// artifact identity
// ---------------------------------------------------------------------------

/// One publication a task durably committed, **named**.
///
/// The gap bn-1gc recorded and could not close: its `Checkpoint` pairs a commitment
/// *count* with the spend recorded when it happened, and `CommittedPartialEvidence` reads
/// four numbers off that pair — but a count is not an identity, and "committed partial
/// evidence survives daemon restart" is not a claim a counter can be held to. A count that
/// came back as 2 after a restart says nothing about *which* two.
///
/// So a publication carries its content identity. Two consequences a test can hold:
///
/// - **the same campaign names the same publication in any process**, because the identity
///   is derived from the campaign's own canonical bytes through
///   [`ContentIdentifier`] — no counter, no clock, no address (INV-005, INV-006);
/// - **a resumed run's publication is a different name**, because its bytes differ, which
///   is RFC 0026's "re-derived artifacts receive new identities" at this grain rather than
///   an overwrite INV-009 forbids.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Publication {
    sequence: u32,
    /// The content identity, tied to the receipt the store issued for it (bn-283p6). A
    /// publication the task names is one the store has committed (INV-017).
    commitment: Published<Commitment>,
    checkpoint: Checkpoint,
}

impl Publication {
    /// This publication's position in commit order.
    ///
    /// A dense ordinal, like [`RegionId`](continuum_task::region::RegionId): counted, never
    /// drawn or timed.
    #[must_use]
    pub const fn sequence(&self) -> u32 {
        self.sequence
    }

    /// The content identity of what was committed.
    #[must_use]
    pub const fn commitment(&self) -> &Commitment {
        self.commitment.handle()
    }

    /// The budget checkpoint this commit was priced at.
    ///
    /// The join bn-1gc built: the region layer's commitment count paired with the spend
    /// recorded when it happened. Naming the publication is what makes the pair a statement
    /// about an artifact rather than about a number.
    #[must_use]
    pub const fn checkpoint(&self) -> &Checkpoint {
        &self.checkpoint
    }

    /// The wire reference naming this publication.
    ///
    /// `kind` is the artifact class prefix without its underscore, `handle` is the task the
    /// publication belongs to, and `commitment` is *this* publication's content identity —
    /// which is exactly what `ArtifactRef` is for: "typed refs (`kind`, `handle`,
    /// `commitment?`, `redacted?`), not bare handles". Two publications of one task are two
    /// refs with one handle and two commitments, which is a version list rather than an
    /// overwrite.
    ///
    /// # Errors
    ///
    /// [`Fault`] carrying [`ErrorCode::PublicationAborted`] when the task handle is not a
    /// well-formed artifact handle. Unreachable — a `task_*` handle was minted by the
    /// identity seam — and typed rather than unwrapped, because a daemon does not abort on
    /// its own invariant.
    pub fn artifact(&self, task: &TaskHandle) -> Result<ArtifactRef, Fault> {
        Ok(ArtifactRef {
            kind: ArtifactClass::Task.token().to_owned(),
            handle: ArtifactHandle::new(task.as_str()).map_err(|_| {
                Fault::new(
                    ErrorCode::PublicationAborted,
                    "the task identity is not a well-formed artifact handle",
                )
                .not_retryable()
            })?,
            commitment: Optional::Present(self.commitment.handle().clone()),
            redacted: Optional::Absent,
        })
    }
}

/// A task's publications: the committed ones, named and in order, and the staged one, which
/// is nobody's.
///
/// > `task.cancel` […] MUST leave either committed partial evidence plus a valid
/// > continuation, or nothing published (INV-009, plan B19). Cancellation MUST NOT truncate
/// > a publication in progress (INV-017).
/// >
/// > — `rule task.cancel_correct`
///
/// The type is a linear typestate over one publication at a time, which is the shape
/// `continuum-workspace`'s own `StagedPublication` → `CommittedContent` pair already has:
/// [`stage`](Self::stage) opens one, [`commit`](Self::commit) consumes it into a
/// [`Publication`], and [`discard`](Self::discard) consumes it into nothing. There is no
/// third exit, so **a staged publication is never observable**: [`committed`](Self::committed)
/// cannot see it and [`artifacts`](Self::artifacts) cannot name it. That is G0-DX-14's
/// second conjunct — "artifacts either committed or absent" — as a property of what
/// compiles rather than of a check somebody runs.
///
/// [`committed`](Self::committed) is append-only. Nothing here removes a publication, which
/// is INV-009's "resuming a task may add evidence … it may not silently replace prior
/// artifacts" at the list.
#[derive(Debug, Default)]
pub struct Publications {
    committed: Vec<Publication>,
    staged: Option<Commitment>,
}

impl Publications {
    /// A task that has published nothing and staged nothing.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Every publication this task durably committed, in commit order.
    #[must_use]
    pub fn committed(&self) -> &[Publication] {
        &self.committed
    }

    /// How many publications are durable.
    ///
    /// The counter [`TaskEntry`](super::task::TaskEntry) used to carry, now a *consequence*
    /// of the named list rather than a number kept beside it. `continuation.is_some() ⇒
    /// count > 0` still holds by construction, for the same reason it did: the commit
    /// happens before the continuation is minted.
    #[must_use]
    pub fn count(&self) -> u32 {
        u32::try_from(self.committed.len()).unwrap_or(u32::MAX)
    }

    /// Whether a publication is in flight.
    ///
    /// True only *inside* the dispatch that staged it: every path out of
    /// [`verification::advance`](super::verification::advance) either commits or discards,
    /// and a dispatch holds `&mut DaemonState` for its whole extent, so no operation can
    /// observe this as true. Exposed because an instrumented answer to "is anything
    /// dangling" beats an argument that nothing can be.
    #[must_use]
    pub const fn is_staging(&self) -> bool {
        self.staged.is_some()
    }

    /// Stage a publication under `commitment`. Nothing a reader can observe changes.
    ///
    /// Replaces any publication already staged, which cannot happen: the one caller stages
    /// and then commits or discards within one dispatch.
    pub fn stage(&mut self, commitment: Commitment) {
        self.staged = Some(commitment);
    }

    /// Commit the staged publication at `checkpoint`, naming it by `published`.
    ///
    /// `published` is the staged name tied to the store's receipt (bn-283p6), so a task
    /// cannot name a publication the store has not committed (INV-017). The order "publish,
    /// then record" is what compiles, not a convention.
    ///
    /// [`None`] when nothing was staged — a commit with no publication in flight is the
    /// caller's bug, and it is reported rather than invented, because minting a name for a
    /// publication that was never staged is exactly the half-visible artifact this type
    /// exists to make unconstructable. [`None`] too when `published` names another
    /// publication than the staged one. The staged publication is then dropped, as by
    /// [`discard`](Self::discard): a commit is both-or-neither.
    pub fn commit(
        &mut self,
        checkpoint: Checkpoint,
        published: Published<Commitment>,
    ) -> Option<&Publication> {
        let staged = self.staged.take()?;
        if &staged != published.handle() {
            return None;
        }
        let sequence = self.count();
        self.committed.push(Publication {
            sequence,
            commitment: published,
            checkpoint,
        });
        self.committed.last()
    }

    /// Drop the staged publication. Its bytes were never written and no reader saw it.
    ///
    /// Returns whether there was one. The budget-side counterpart is
    /// [`BudgetLedger::release`](continuum_task::budget::BudgetLedger::release): the
    /// headroom comes back and the compute stays charged, because the compute happened.
    pub fn discard(&mut self) -> bool {
        self.staged.take().is_some()
    }

    /// The wire references naming every committed publication, in commit order.
    ///
    /// # Errors
    ///
    /// As [`Publication::artifact`].
    pub fn artifacts(&self, task: &TaskHandle) -> Result<Vec<ArtifactRef>, Fault> {
        self.committed
            .iter()
            .map(|publication| publication.artifact(task))
            .collect()
    }

    /// A canonical one-line-per-publication rendering, for byte-identity comparison.
    #[must_use]
    pub fn render(&self) -> String {
        let mut out = String::new();
        for publication in &self.committed {
            out.push_str(&format!(
                "publication {} {} {}\n",
                publication.sequence,
                publication.commitment().as_str(),
                publication.checkpoint.render()
            ));
        }
        out.push_str(match self.staged {
            Some(_) => "staged: one\n",
            None => "staged: none\n",
        });
        out
    }
}

/// The canonical record a campaign result *is* — and the bytes the store holds.
///
/// The record is the task it belongs to, its position in that task's commit order, the
/// snapshot it ran over, the number of states it explored, whether the exploration closed,
/// and the queue-ordered frontier it parked at — so two daemons that ran the same campaign
/// build the same record and a resumed run, whose walk went further, builds a different one.
/// Every part is length-prefixed by [`Preimage`], so no pair of inputs can produce another
/// pair's bytes by concatenation.
///
/// # Why this is a function and not a preimage buried in the identity derivation
///
/// It was the latter until bn-3dr. A publication was *named* by hashing these bytes and the
/// bytes were then thrown away, which made the name unfetchable: the store had never been
/// given the record the name is of, so "committed partial evidence survives daemon restart"
/// (IMPL-02) had nothing to survive in. Splitting the record out lets
/// [`verification::advance`](super::verification::advance) publish it through
/// `continuum-workspace`'s two-phase protocol under the identity
/// [`publication_commitment`] derives from exactly these bytes — so the daemon's name and
/// the store's are one value by construction rather than by agreement.
#[must_use]
pub fn publication_record(
    task: &TaskHandle,
    sequence: u32,
    snapshot: Option<&str>,
    states: u64,
    closed: bool,
    frontier: &[State],
) -> Vec<u8> {
    let mut preimage = Preimage::new();
    preimage.text(task.as_str());
    preimage.push(&sequence.to_be_bytes());
    preimage.text(snapshot.unwrap_or("-"));
    preimage.push(&states.to_be_bytes());
    preimage.text(if closed { "closed" } else { "bounded" });
    preimage.push(&(frontier.len() as u64).to_be_bytes());
    for state in frontier {
        for component in state.as_slice() {
            preimage.push(&component.to_be_bytes());
        }
    }
    preimage.bytes().to_vec()
}

/// The content identity a campaign record commits under.
///
/// # The epochs are deliberately not in this identity
///
/// A publication is *the campaign's content*, and the epochs ride the task record beside it.
/// bn-3dr revisited the question the routing comment on this bone raised — whether a durable
/// store binding needs epoch-scoped publication identities — and the answer is no, on the
/// store's own terms: [`ReferenceStore`](continuum_workspace::publication::ReferenceStore)
/// derives every identity from the bytes it is handed and from nothing else, so binding the
/// daemon's name to the store's requires only that the daemon publish the bytes it named,
/// which [`publication_record`] is. Widening the identity with epochs would make the two
/// daemons of `two_daemons_name_one_campaigns_publications_identically` disagree whenever
/// their epoch sets did, for no store-side gain. The function stays the single point where
/// that decision could be revisited.
///
/// # Errors
///
/// [`Fault`] carrying [`ErrorCode::PublicationAborted`] when the identity seam cannot name
/// the record. Nothing is published under a guessed identity.
pub fn publication_commitment(
    identifier: &dyn ContentIdentifier,
    task: &TaskHandle,
    sequence: u32,
    snapshot: Option<&str>,
    states: u64,
    closed: bool,
    frontier: &[State],
) -> Result<Commitment, Fault> {
    let record = publication_record(task, sequence, snapshot, states, closed, frontier);
    commitment_of(identifier, &record)
}

/// The commitment a published campaign record carries.
///
/// Factored out of [`publication_commitment`] rather than duplicated for the reason
/// [`DaemonState::commit_of`](super::state::DaemonState::commit_of) gives for the same
/// split: the value is derived twice — once to *name* a publication and once to check that
/// the store named the same record the same way — and a second spelling would let the two
/// agree with each other while both disagreed with the protocol.
///
/// # Errors
///
/// [`Fault`] carrying [`ErrorCode::PublicationAborted`] when the identity seam cannot name
/// the record.
pub fn commitment_of(
    identifier: &dyn ContentIdentifier,
    record: &[u8],
) -> Result<Commitment, Fault> {
    let handle = identifier
        .identify(ArtifactClass::Task, record)
        .map_err(|_| {
            Fault::new(
                ErrorCode::PublicationAborted,
                "no content identity could be derived for a committed publication",
            )
            .not_retryable()
        })?;
    Ok(Commitment::new(&handle.to_string()))
}

fn optional<T>(value: Option<T>) -> Optional<T> {
    match value {
        Some(value) => Optional::Present(value),
        None => Optional::Absent,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::daemon::identity::{Blake3Identity, testing};
    use crate::protocol::scalar::TaskHandle;

    fn budget() -> Budget {
        Budget {
            wall_ms: Optional::Present(DurationMs::new(5_000)),
            cpu_ms: Optional::Absent,
            memory_bytes: Optional::Absent,
            states: Optional::Present(64),
            solver_ms: Optional::Absent,
            proof_ms: Optional::Absent,
            tokens: Optional::Absent,
            candidates: Optional::Absent,
            bytes: Optional::Present(ByteCount::new(4_096)),
        }
    }

    fn task() -> TaskHandle {
        TaskHandle::new("task_publication").expect("a well-formed task handle")
    }

    #[test]
    fn the_wire_budget_round_trips_through_the_ledger_unchanged() {
        let ledger = ledger_of(&budget());
        assert_eq!(wire_budget(&ledger), budget());
    }

    #[test]
    fn the_omission_manifest_is_derived_from_the_meter_set() {
        let ledger = ledger_of(&budget());
        let subjects: Vec<String> = omissions_of(&ledger)
            .into_iter()
            .map(|omission| omission.subject)
            .collect();
        assert_eq!(
            subjects,
            vec!["budget.wall_ms".to_owned(), "budget.bytes".to_owned()],
            "only the declared ceilings this daemon cannot measure, in declaration order"
        );
        // The derivation is load-bearing rather than a re-spelling of a list: a meter set
        // that measured everything owes nothing, without anything here being edited.
        let metered = BudgetLedger::new(super::ceilings_of(&budget()), MeterSet::all());
        assert!(omissions_of(&metered).is_empty());
    }

    #[test]
    fn every_declared_ceiling_this_daemon_cannot_meter_is_named() {
        let all = Budget {
            wall_ms: Optional::Present(DurationMs::new(1)),
            cpu_ms: Optional::Present(DurationMs::new(1)),
            memory_bytes: Optional::Present(ByteCount::new(1)),
            states: Optional::Present(1),
            solver_ms: Optional::Present(DurationMs::new(1)),
            proof_ms: Optional::Present(DurationMs::new(1)),
            tokens: Optional::Present(1),
            candidates: Optional::Present(1),
            bytes: Optional::Present(ByteCount::new(1)),
        };
        let subjects: Vec<String> = omissions_of(&ledger_of(&all))
            .into_iter()
            .map(|omission| omission.subject)
            .collect();
        assert_eq!(
            subjects,
            vec![
                "budget.wall_ms",
                "budget.cpu_ms",
                "budget.memory_bytes",
                "budget.solver_ms",
                "budget.proof_ms",
                "budget.tokens",
                "budget.candidates",
                "budget.bytes",
            ],
            "the eight bn-18z's `unenforced` named by hand, derived"
        );
    }

    #[test]
    fn a_charge_records_the_high_water_mark_rather_than_the_sum() {
        let mut ledger = ledger_of(&budget());
        charge_states(&mut ledger, 3).expect("metered");
        assert_eq!(
            cost_of(&ledger, true).states,
            Optional::Present(3),
            "the parked run"
        );
        charge_states(&mut ledger, 16).expect("metered");
        assert_eq!(
            cost_of(&ledger, true).states,
            Optional::Present(16),
            "the resumed run re-explored the same prefix; it did not cost 19"
        );
        charge_states(&mut ledger, 16).expect("metered");
        assert_eq!(cost_of(&ledger, true).states, Optional::Present(16));
    }

    #[test]
    fn a_cost_with_no_reading_is_absent_rather_than_zero() {
        let ledger = ledger_of(&budget());
        assert!(
            cost_of(&ledger, false).states.is_absent(),
            "a task that ran nothing measured nothing"
        );
        assert_eq!(
            cost_of(&ledger, true).states,
            Optional::Present(0),
            "a campaign that explored nothing measured nothing, which is a different claim"
        );
        assert!(cost_of(&ledger, true).wall_ms.is_absent());
        assert!(cost_of(&ledger, true).tokenizer_id.is_absent());
    }

    #[test]
    fn a_ceiling_below_committed_spend_is_recorded_and_does_not_shrink_the_walk() {
        let mut ledger = ledger_of(&budget());
        charge_states(&mut ledger, 3).expect("metered");
        let lowered = Budget {
            states: Optional::Present(2),
            ..budget()
        };
        let outcomes = update(&mut ledger, &lowered);
        let suspension = outcomes
            .iter()
            .find_map(continuum_task::budget::UpdateOutcome::suspension)
            .expect("lowering below committed spend is B18");
        assert_eq!(suspension.dimension(), CostDimension::States);
        assert_eq!(suspension.ceiling(), 2);
        assert_eq!(suspension.spent(), 3);
        assert_eq!(
            wire_budget(&ledger).states,
            Optional::Present(2),
            "the ceiling is recorded: `TaskRecord.budget` reports the budget in force"
        );
        assert_eq!(
            bounds_of(&ledger).states(),
            3,
            "and it is not a bound that un-explores what the run already explored"
        );
    }

    #[test]
    fn a_raised_ceiling_is_the_bound_the_next_run_takes() {
        let mut ledger = ledger_of(&budget());
        charge_states(&mut ledger, 3).expect("metered");
        assert_eq!(bounds_of(&ledger).states(), 64);
        let unbounded = Budget {
            states: Optional::Absent,
            ..budget()
        };
        let _ = update(&mut ledger, &unbounded);
        assert_eq!(
            bounds_of(&ledger).states(),
            Bounds::CERTIFIABLE.states(),
            "a budget declaring no `states` runs at the certifiable ceiling"
        );
    }

    #[test]
    fn a_staged_publication_is_absent_until_it_commits_and_absent_after_a_discard() {
        let mut publications = Publications::new();
        assert_eq!(publications.count(), 0);
        publications.stage(Commitment::new("commit-1"));
        assert!(publications.is_staging());
        assert!(
            publications.committed().is_empty(),
            "nothing a reader can observe"
        );
        assert!(publications.artifacts(&task()).expect("named").is_empty());
        assert!(publications.discard());
        assert!(!publications.is_staging());
        assert_eq!(publications.count(), 0, "a discard publishes nothing");

        let (store, operator) = testing::store();
        let commit_2 = Commitment::new("task_commit-2");
        publications.stage(commit_2.clone());
        let spend = continuum_task::budget::Spend::measured_by(METERS);
        let named = publications
            .commit(
                Checkpoint::new(0, 1, spend),
                testing::publish(&store, &operator, "task_commit-2", commit_2),
            )
            .expect("a staged publication commits");
        assert_eq!(named.sequence(), 0);
        assert_eq!(named.commitment().as_str(), "task_commit-2");
        assert_eq!(publications.count(), 1);
        assert!(!publications.is_staging());
        assert_eq!(publications.artifacts(&task()).expect("named").len(), 1);
    }

    #[test]
    fn committing_nothing_names_nothing() {
        let (store, operator) = testing::store();
        let published = |spelling: &str| {
            testing::publish(&store, &operator, spelling, Commitment::new(spelling))
        };
        let mut publications = Publications::new();
        let spend = continuum_task::budget::Spend::measured_by(METERS);
        assert!(
            publications
                .commit(Checkpoint::new(0, 0, spend), published("task_c0"))
                .is_none()
        );
        assert_eq!(publications.count(), 0);
        assert!(!publications.discard());
    }

    /// bn-283p6: a receipt for one publication does not commit another. The staged one is
    /// dropped, both-or-neither, and nothing is named.
    #[test]
    fn a_receipt_for_another_publication_commits_nothing() {
        let (store, operator) = testing::store();
        let mut publications = Publications::new();
        publications.stage(Commitment::new("task_staged"));
        let other = testing::publish(
            &store,
            &operator,
            "task_other",
            Commitment::new("task_other"),
        );
        let spend = continuum_task::budget::Spend::measured_by(METERS);
        assert!(
            publications
                .commit(Checkpoint::new(0, 1, spend), other)
                .is_none()
        );
        assert_eq!(publications.count(), 0, "nothing is named");
        assert!(!publications.is_staging(), "and nothing is left in flight");
    }

    #[test]
    fn one_campaign_names_one_publication_in_any_process() {
        let identifier = Blake3Identity;
        let name = |sequence: u32, states: u64| {
            publication_commitment(
                &identifier,
                &task(),
                sequence,
                Some("ws_snapshot"),
                states,
                false,
                // `State` is constructible only through a `Model`, so the frontier's
                // contribution to this preimage is walked at the daemon grain by
                // `tests/pr6_impl02_budget_evidence.rs` against real campaigns.
                &[],
            )
            .expect("the identity seam names it")
        };
        assert_eq!(name(0, 3), name(0, 3), "pure in its arguments (INV-006)");
        assert_ne!(
            name(0, 3),
            name(1, 3),
            "a second publication of one task is a second name"
        );
        assert_ne!(
            name(0, 3),
            name(0, 16),
            "a resumed run that walked further is a different artifact"
        );
    }

    #[test]
    fn the_publication_render_is_byte_identical_across_two_runs() {
        let build = || {
            let (store, operator) = testing::store();
            let mut publications = Publications::new();
            let spend = continuum_task::budget::Spend::measured_by(METERS);
            for (sequence, spelling) in [(0, "task_commit-1"), (1, "task_commit-2")] {
                let name = Commitment::new(spelling);
                publications.stage(name.clone());
                publications.commit(
                    Checkpoint::new(sequence, sequence + 1, spend),
                    testing::publish(&store, &operator, spelling, name),
                );
            }
            publications.render()
        };
        assert_eq!(build().as_bytes(), build().as_bytes());
        assert!(!build().is_empty(), "a vacuous render proves nothing");
    }
}
