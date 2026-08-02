//! Budget accounting: what a task was allowed to spend, what it actually spent, and the
//! typed outcome when the two meet (PR-6 / IMPL-02, IMPL-03).
//!
//! > The daemon enforces per-principal and per-transaction cost ceilings; exceeding one
//! > yields `BudgetExhausted` with a continuation — never a silently smaller campaign (the
//! > cost-domain form of INV-007).
//! >
//! > — `notes/plan/plan.md` §8.6
//!
//! # What this module is
//!
//! A [`BudgetLedger`] over the nine SD-12 dimensions ([`dimension`]), with four
//! operations and one report:
//!
//! - **charge** — record spend that has happened. Monotone: nothing un-spends it.
//! - **reserve / settle / release** — hold headroom for work in flight, then consume it or
//!   give it back. This is the only refund path, and it refunds *headroom*, never spend.
//! - **checkpoint** — bind the spend so far to the number of publications the region layer
//!   has committed. A checkpoint is what makes "committed partial evidence" a quantity
//!   rather than a phrase ([`partial`]).
//! - **update** — RFC 0026's `task.update_budget`, mid-flight, with its legality table.
//!
//! Exhaustion is a *result*, not an error: [`ChargeOutcome::Exhausted`] carries an
//! [`Exhaustion`] naming the dimension, its ceiling, the spend, and what was asked for.
//! `continuum-engine-reference`'s `bfs` module made the same choice for the same reason —
//! "a bound that trips is a *result* … never an error, which is the whole reason a parked
//! continuation exists". A [`BudgetFault`] is reserved for things that are genuinely
//! illegal, like charging a dimension nothing measures.
//!
//! # Spend is monotone; only headroom is refundable
//!
//! This is the rule the whole ledger is arranged around, and it is not a style choice.
//! `Cost` is *actual spend* (RFC 0026), so a publication that is staged and then discarded
//! does not un-spend the compute that produced it — the artifact is absent, the cost is
//! real, and a ledger that refunded it would report a campaign as cheaper than it was.
//! What *is* refundable is headroom reserved against a ceiling and not consumed:
//! [`BudgetLedger::reserve`] takes it out of the available budget without recording it as
//! spend, [`BudgetLedger::settle`] charges what was actually used and hands the remainder
//! back, and [`BudgetLedger::release`] hands all of it back having charged nothing.
//!
//! One consequence is worth stating because it is load-bearing: **a settle can never
//! exhaust**. The reservation already held the headroom, so the spend it becomes was
//! admissible when it was reserved. That is what makes the `bytes` dimension — RFC 0027's
//! *enforced* context contract — safe to reserve before a payload's true size is known.
//!
//! # Declared, enforced, omitted
//!
//! [`dimension`]'s header has the 2×2 table; the consequence here is that
//! [`BudgetLedger::omissions`] is *computed*, never maintained. Every dimension the caller
//! declared a ceiling for and this accounting has no meter for is a
//! [`DimensionOmission`](dimension::DimensionOmission) on the answer, which is the INV-007
//! discipline bn-18z wrote by hand in `crates/continuumd/src/daemon/verification.rs` and
//! this type derives. A charge against an unmetered dimension is refused outright, because
//! recording a number for something nothing measured is inventing a measurement, and
//! RFC 0026 is explicit that "a dimension the engine does not measure is absent, never
//! zero".
//!
//! ```
//! use continuum_task::budget::dimension::{CostDimension, MeterSet};
//! use continuum_task::budget::{Budget, BudgetLedger, ChargeOutcome};
//!
//! // A caller declares two ceilings; this service meters only `states`.
//! let budget = Budget::unbounded()
//!     .with(CostDimension::States, 16)
//!     .with(CostDimension::WallMs, 5_000);
//! let mut ledger = BudgetLedger::new(budget, MeterSet::STATES_ONLY);
//!
//! // The metered, declared dimension is enforced.
//! assert!(matches!(
//!     ledger.charge(CostDimension::States, 10),
//!     Ok(ChargeOutcome::Admitted { .. })
//! ));
//! let outcome = ledger.charge(CostDimension::States, 7).expect("metered");
//! let exhaustion = outcome.exhaustion().expect("10 + 7 > 16");
//! assert_eq!(exhaustion.dimension(), CostDimension::States);
//! assert_eq!(exhaustion.error_code_token(), "BudgetExhausted");
//!
//! // The refused charge changed nothing: spend is still what was admitted.
//! assert_eq!(ledger.spend().measured(CostDimension::States), Some(10));
//!
//! // The declared ceiling nothing meters is a typed omission, never a silent pass.
//! assert_eq!(
//!     ledger.omissions().iter().map(|o| o.subject()).collect::<Vec<_>>(),
//!     vec!["budget.wall_ms".to_owned()]
//! );
//! ```
//!
//! # Where the daemon plugs in (the seam, not the rewiring)
//!
//! `continuumd` is read-only to this bone, exactly as it was to the region layer, and the
//! seam is designed rather than taken:
//!
//! - `TaskEntry` already holds a `budget: Budget` and a `bounds: Bounds` derived from it by
//!   `verification::bounds_of`. Under this module it holds a [`BudgetLedger`] built from
//!   [`MeterSet::STATES_ONLY`](dimension::MeterSet::STATES_ONLY), and `bounds_of` becomes a
//!   projection of the ledger's `states` ceiling rather than a second reading of the same
//!   field.
//! - `TaskEntry::omissions` gains [`BudgetLedger::omissions`] and `verification::unenforced`
//!   is deleted: the eight-name array becomes a consequence of the meter set, so adding a
//!   meter removes an omission without anyone remembering to.
//! - `TaskEntry::cost` becomes [`BudgetLedger::spend`] projected onto `Cost`. The
//!   absent-never-zero rule survives the projection because [`Spend`] draws the same
//!   distinction: a metered dimension reads a measurement, an unmetered one reads absent.
//! - `task.update_budget` calls [`BudgetLedger::update`] and reads [`UpdateOutcome`], which
//!   is `rule task.update_budget`'s legality table as a value — including the
//!   [`UpdateOutcome::Suspend`] arm the handler currently has no way to compute, because it
//!   has no committed-spend figure to compare a lowered ceiling against.
//! - the `BudgetExhausted` fault it raises reads its dimension, ceiling and spend off
//!   [`Exhaustion`], and pairs with the region layer's
//!   [`CancelOutcome`](crate::region::worker::CancelOutcome) to answer RFC 0026's
//!   both-or-neither: committed partial evidence plus a continuation, or nothing published.
//!
//! # What this module is not
//!
//! - **Not quota.** `QuotaExhausted` is "a capability's concurrency or resource quota"
//!   and `BudgetExhausted` is task spend; RFC 0026 says the daemon "MUST NOT substitute one
//!   for the other". This layer holds no capability and knows no principal, so it can only
//!   ever produce the second. A per-principal ceiling is the daemon's, over the same nine
//!   dimensions.
//! - **Not a verdict.** "A campaign that ran out of budget has not refuted anything"
//!   (RFC 0026). [`Exhaustion`] carries an `ErrorCode` token and no `Verdict`, and
//!   [`TaskOutcome`](crate::result::TaskOutcome) has no arm it could become.
//! - **Not a clock, and not a meter.** Nothing here reads time, counts bytes, or asks an
//!   engine anything: a charge is a number a caller supplies, exactly as a
//!   [`WorkerStep`](crate::region::worker::WorkerStep) is a move a caller writes down
//!   (INV-005). Which dimensions a *real* service can measure is what
//!   [`MeterSet`](dimension::MeterSet) records, and this crate ships no meters.
//! - **Not the wire.** `Budget`, `Cost`, `Omission` and `tokenizer_id` are
//!   `continuumd`'s structs; this is the accounting they report.

pub mod dimension;
pub mod partial;

use core::fmt;

use dimension::{Ceiling, CostDimension, DimensionOmission, MeterSet, Metering};
use partial::Checkpoint;

/// The ceilings a caller declared, one slot per dimension.
///
/// Every dimension is named — there are always nine slots — and an undeclared one reads
/// [`Ceiling::Unbounded`] rather than being absent, which is the same shape
/// [`EpochSet`](continuum_value::epoch::EpochSet) uses for an unpinned epoch: a named
/// absence, never a missing field (INV-007).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Budget([Ceiling; 9]);

impl Default for Budget {
    fn default() -> Self {
        Self::unbounded()
    }
}

impl Budget {
    /// A budget declaring no ceiling on any dimension.
    ///
    /// Not a permissive default in disguise: a task whose budget declares nothing is a
    /// task nobody bounded, and RFC 0026 requires a `budget` on every `@task_starting`
    /// request precisely so that no such task reaches an engine. This constructor exists
    /// to be built on with [`Self::with`].
    #[must_use]
    pub const fn unbounded() -> Self {
        Self([Ceiling::Unbounded; 9])
    }

    /// The same budget, declaring `limit` on `dimension`.
    #[must_use]
    pub fn with(mut self, dimension: CostDimension, limit: u64) -> Self {
        self.0[dimension.index()] = Ceiling::At(limit);
        self
    }

    /// The same budget with `dimension`'s ceiling removed.
    #[must_use]
    pub fn without(mut self, dimension: CostDimension) -> Self {
        self.0[dimension.index()] = Ceiling::Unbounded;
        self
    }

    /// What `dimension` declares.
    #[must_use]
    pub const fn ceiling(&self, dimension: CostDimension) -> Ceiling {
        self.0[dimension.index()]
    }

    /// The dimensions carrying a declared ceiling, in declaration order.
    #[must_use]
    pub fn declared(&self) -> Vec<CostDimension> {
        CostDimension::ALL
            .into_iter()
            .filter(|dimension| self.ceiling(*dimension).is_declared())
            .collect()
    }

    /// A canonical one-line rendering of the declared ceilings, in declaration order.
    #[must_use]
    pub fn render(&self) -> String {
        let mut out = String::from("budget:");
        for dimension in self.declared() {
            out.push_str(&format!(" {}={}", dimension, self.ceiling(dimension)));
        }
        out
    }
}

impl fmt::Display for Budget {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.render())
    }
}

/// Measured spend, one slot per dimension.
///
/// > **A dimension the engine does not measure is absent, never zero** — the presence
/// > distinction carries the meaning.
/// >
/// > — `notes/plan/rfcs/0026-continuumd-native-protocol.md`, "Cost and omissions"
///
/// So an unmetered dimension reads [`None`] for the whole life of a ledger, and a metered
/// one reads `Some(0)` from the moment the ledger exists. The two are different claims —
/// "we did not look" and "we looked and it was nothing" — and this is the type that keeps
/// them apart.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Spend([Option<u64>; 9]);

impl Default for Spend {
    fn default() -> Self {
        Self::unmeasured()
    }
}

impl Spend {
    /// Spend in which nothing was measured.
    #[must_use]
    pub const fn unmeasured() -> Self {
        Self([None; 9])
    }

    /// Zero spend on exactly the dimensions `meters` measures.
    #[must_use]
    pub fn measured_by(meters: MeterSet) -> Self {
        let mut spend = Self::unmeasured();
        for dimension in meters.metered() {
            spend.0[dimension.index()] = Some(0);
        }
        spend
    }

    /// What `dimension` measured, or [`None`] when nothing measured it.
    #[must_use]
    pub const fn measured(&self, dimension: CostDimension) -> Option<u64> {
        self.0[dimension.index()]
    }

    /// The measured dimensions and their values, in declaration order.
    #[must_use]
    pub fn measurements(&self) -> Vec<(CostDimension, u64)> {
        CostDimension::ALL
            .into_iter()
            .filter_map(|dimension| self.measured(dimension).map(|value| (dimension, value)))
            .collect()
    }

    /// This spend minus `earlier`, per dimension.
    ///
    /// Measured only where *both* are measured, because a difference between a
    /// measurement and a non-measurement is not a measurement. Spend is monotone and
    /// `earlier` is always a snapshot of this same ledger, so the subtraction cannot
    /// underflow; it saturates rather than panicking because this crate's arithmetic is
    /// never allowed to be a panic site.
    #[must_use]
    pub fn since(&self, earlier: &Self) -> Self {
        let mut out = Self::unmeasured();
        for dimension in CostDimension::ALL {
            if let (Some(now), Some(then)) = (self.measured(dimension), earlier.measured(dimension))
            {
                out.0[dimension.index()] = Some(now.saturating_sub(then));
            }
        }
        out
    }

    /// A canonical one-line rendering of the measured dimensions, in declaration order.
    #[must_use]
    pub fn render(&self) -> String {
        let mut out = String::from("spend:");
        for (dimension, value) in self.measurements() {
            out.push_str(&format!(" {dimension}={value}"));
        }
        out
    }
}

impl fmt::Display for Spend {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.render())
    }
}

/// One dimension's budget was exhausted, and here is every number that says so.
///
/// > `BudgetExhausted` | resource | task-budget spend was exhausted; carries
/// > `continuation`, or a typed `non_resumable_reason` | no — resume with a larger budget
/// >
/// > — `notes/plan/rfcs/0026-continuumd-native-protocol.md`, §10.3 taxonomy
///
/// It names the dimension for the reason `bfs::Bound` names which bound tripped: "a caller
/// that hit the state bound raises the state bound; one that hit the depth bound learns its
/// property may live deeper, which is a different next move". An exhaustion that said only
/// "out of budget" would leave the caller to guess which of nine numbers to change.
///
/// It is never a verdict. There is no `Verdict`, no `AssuranceLevel` and no boolean here,
/// and [`Self::error_code_token`] reports an `ErrorCode` spelling rather than a semantic
/// one, because "a campaign that ran out of budget has not refuted anything" (RFC 0026).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Exhaustion {
    dimension: CostDimension,
    limit: u64,
    spent: u64,
    requested: u64,
}

impl Exhaustion {
    /// The exhaustion of `dimension` at `limit`, with `spent` already recorded, when
    /// `requested` more was asked for.
    #[must_use]
    pub const fn new(dimension: CostDimension, limit: u64, spent: u64, requested: u64) -> Self {
        Self {
            dimension,
            limit,
            spent,
            requested,
        }
    }

    /// Which of the nine dimensions ran out.
    #[must_use]
    pub const fn dimension(&self) -> CostDimension {
        self.dimension
    }

    /// The ceiling that was declared.
    #[must_use]
    pub const fn limit(&self) -> u64 {
        self.limit
    }

    /// The spend recorded when the charge arrived. Unchanged by the refusal.
    #[must_use]
    pub const fn spent(&self) -> u64 {
        self.spent
    }

    /// What the refused charge asked for.
    #[must_use]
    pub const fn requested(&self) -> u64 {
        self.requested
    }

    /// How much was still available. Always less than [`Self::requested`].
    #[must_use]
    pub const fn headroom(&self) -> u64 {
        self.limit.saturating_sub(self.spent)
    }

    /// The RFC 0026 `ErrorCode` spelling of this outcome.
    ///
    /// The daemon's taxonomy is `continuumd`'s and this crate has no edge to it, so the
    /// agreement is by shared token — the device
    /// [`WorkerState::status_token`](crate::region::worker::WorkerState::status_token)
    /// uses for `TaskStatus`.
    #[must_use]
    pub const fn error_code_token(&self) -> &'static str {
        "BudgetExhausted"
    }

    /// A canonical one-line rendering.
    #[must_use]
    pub fn render(&self) -> String {
        format!(
            "{} dimension={} limit={} spent={} requested={}",
            self.error_code_token(),
            self.dimension,
            self.limit,
            self.spent,
            self.requested
        )
    }
}

impl fmt::Display for Exhaustion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.render())
    }
}

/// What a charge or a reservation did.
///
/// Two arms, and the second is a *result*: a ceiling that trips is the answer, not a
/// failure of the machinery. Compare `Exploration::Exhausted` in
/// `continuum-engine-reference`, which carries the partial walk rather than an error, for
/// the same reason a parked continuation exists at all.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ChargeOutcome {
    /// The spend was admitted.
    Admitted {
        /// Which dimension.
        dimension: CostDimension,
        /// The recorded spend after the charge.
        spent: u64,
        /// The ceiling it is measured against.
        ceiling: Ceiling,
    },
    /// The spend would have passed the ceiling, so it was not recorded.
    Exhausted(Exhaustion),
}

impl ChargeOutcome {
    /// The exhaustion, when the charge tripped a ceiling.
    #[must_use]
    pub const fn exhaustion(&self) -> Option<&Exhaustion> {
        match self {
            Self::Admitted { .. } => None,
            Self::Exhausted(exhaustion) => Some(exhaustion),
        }
    }

    /// Whether the spend was recorded.
    #[must_use]
    pub const fn is_admitted(&self) -> bool {
        matches!(self, Self::Admitted { .. })
    }

    /// A canonical one-line rendering.
    #[must_use]
    pub fn render(&self) -> String {
        match self {
            Self::Admitted {
                dimension,
                spent,
                ceiling,
            } => format!("admitted {dimension} spent={spent} ceiling={ceiling}"),
            Self::Exhausted(exhaustion) => format!("exhausted {exhaustion}"),
        }
    }
}

impl fmt::Display for ChargeOutcome {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.render())
    }
}

/// A lowered ceiling met committed spend, so the task parks instead of being truncated.
///
/// > Lowering a dimension below committed spend triggers suspension-with-continuation
/// > (B18): the task transitions to `Suspended` with committed partial evidence plus a
/// > valid continuation. Silent truncation of a campaign is prohibited (INV-009).
/// >
/// > — `notes/plan/schemas/continuumd-native-protocol.idl`, `rule task.update_budget`
///
/// **"Committed spend" is read as recorded, non-refundable spend** — this ledger's
/// [`BudgetLedger::spend`] — and not as the spend at the last checkpoint. The alternative
/// reading admits a ceiling the run has already passed, which is exactly the retroactive
/// truncation INV-009 forbids; under this reading the new ceiling is refused *as an
/// enforceable bound on the current run* and the run parks, which is what the rule says
/// happens.
///
/// The checkpoint it parks from is carried rather than inferred: it is the committed
/// partial evidence the continuation resumes from, and [`None`] — nothing committed yet —
/// is a representable, named outcome, the same way
/// [`CancelOutcome::NothingPublished`](crate::region::worker::CancelOutcome::NothingPublished)
/// is on the cancellation side.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Suspension {
    dimension: CostDimension,
    ceiling: u64,
    spent: u64,
    at: Option<Checkpoint>,
}

impl Suspension {
    /// The suspension `dimension`'s new `ceiling` forces, given `spent` and the
    /// checkpoint it parks from.
    #[must_use]
    pub const fn new(
        dimension: CostDimension,
        ceiling: u64,
        spent: u64,
        at: Option<Checkpoint>,
    ) -> Self {
        Self {
            dimension,
            ceiling,
            spent,
            at,
        }
    }

    /// The dimension whose ceiling was lowered.
    #[must_use]
    pub const fn dimension(&self) -> CostDimension {
        self.dimension
    }

    /// The new, lower ceiling.
    #[must_use]
    pub const fn ceiling(&self) -> u64 {
        self.ceiling
    }

    /// The committed spend it is below.
    #[must_use]
    pub const fn spent(&self) -> u64 {
        self.spent
    }

    /// The checkpoint the continuation resumes from, when anything was committed.
    #[must_use]
    pub const fn checkpoint(&self) -> Option<&Checkpoint> {
        self.at.as_ref()
    }

    /// How many publications the resumed work starts from.
    #[must_use]
    pub fn committed(&self) -> u32 {
        self.at.as_ref().map_or(0, Checkpoint::committed)
    }

    /// A canonical one-line rendering.
    #[must_use]
    pub fn render(&self) -> String {
        format!(
            "suspend dimension={} ceiling={} spent={} committed={}",
            self.dimension,
            self.ceiling,
            self.spent,
            self.committed()
        )
    }
}

impl fmt::Display for Suspension {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.render())
    }
}

/// What a mid-flight `task.update_budget` did to one dimension — the legality table as a
/// value.
///
/// The rule has two sentences and this enum has five arms, because three of the cases the
/// rule leaves implicit are genuinely different and a caller has to be able to tell them
/// apart:
///
/// | new ceiling vs. old | metered | outcome |
/// |---|---|---|
/// | higher, or removed | yes | [`Self::Raised`] — "raising a dimension extends the current run" |
/// | equal | yes | [`Self::Unchanged`] — a re-sent update changes nothing (INV-002) |
/// | lower, still at or above committed spend | yes | [`Self::Tightened`] — binds from now on; nothing already spent is invalidated |
/// | lower, below committed spend | yes | [`Self::Suspend`] — B18, never truncation |
/// | any | no | [`Self::Unenforced`] — the ceiling is recorded and the dimension stays omitted |
///
/// **Lowering is admitted**, which is worth saying plainly because the opposite is a
/// tempting simplification: RFC 0026 does not forbid it, it *defines* it, and the defined
/// behaviour is a suspension rather than a refusal. A ledger that refused every lowering
/// would make `task.update_budget` unable to express the one case the rule spends its
/// second sentence on.
///
/// The last arm has no raise/lower distinction on purpose. Without a meter there is no
/// spend to compare a ceiling against, so reporting one would be a claim the accounting
/// cannot support; the ceiling is still recorded, so a later meter enforces it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UpdateOutcome {
    /// The ceiling rose, or was removed entirely. The current run continues under it.
    Raised {
        /// Which dimension.
        dimension: CostDimension,
        /// What it declared before.
        from: Ceiling,
        /// What it declares now.
        to: Ceiling,
    },
    /// The ceiling is what it already was.
    Unchanged {
        /// Which dimension.
        dimension: CostDimension,
        /// The ceiling, unchanged.
        ceiling: Ceiling,
    },
    /// The ceiling fell but still admits the spend already recorded.
    Tightened {
        /// Which dimension.
        dimension: CostDimension,
        /// What it declared before.
        from: Ceiling,
        /// What it declares now.
        to: Ceiling,
        /// The committed spend the new ceiling still admits.
        spent: u64,
    },
    /// The ceiling fell below committed spend: park with a continuation (B18).
    Suspend(Suspension),
    /// The dimension has no meter, so the recorded ceiling is unenforceable and stays a
    /// typed omission.
    Unenforced(DimensionOmission),
}

impl UpdateOutcome {
    /// The dimension this outcome is about.
    #[must_use]
    pub const fn dimension(&self) -> CostDimension {
        match self {
            Self::Raised { dimension, .. }
            | Self::Unchanged { dimension, .. }
            | Self::Tightened { dimension, .. } => *dimension,
            Self::Suspend(suspension) => suspension.dimension,
            Self::Unenforced(omission) => omission.dimension(),
        }
    }

    /// A stable token for canonical rendering.
    #[must_use]
    pub const fn token(&self) -> &'static str {
        match self {
            Self::Raised { .. } => "raised",
            Self::Unchanged { .. } => "unchanged",
            Self::Tightened { .. } => "tightened",
            Self::Suspend(_) => "suspend",
            Self::Unenforced(_) => "unenforced",
        }
    }

    /// The suspension, when the update forced one.
    #[must_use]
    pub const fn suspension(&self) -> Option<&Suspension> {
        match self {
            Self::Suspend(suspension) => Some(suspension),
            _ => None,
        }
    }

    /// A canonical one-line rendering.
    #[must_use]
    pub fn render(&self) -> String {
        match self {
            Self::Raised {
                dimension,
                from,
                to,
            } => format!("raised {dimension} {from}->{to}"),
            Self::Unchanged { dimension, ceiling } => format!("unchanged {dimension} {ceiling}"),
            Self::Tightened {
                dimension,
                from,
                to,
                spent,
            } => format!("tightened {dimension} {from}->{to} spent={spent}"),
            Self::Suspend(suspension) => suspension.render(),
            Self::Unenforced(omission) => format!("unenforced {omission}"),
        }
    }
}

impl fmt::Display for UpdateOutcome {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.render())
    }
}

/// Every way a budget operation can be refused, named.
///
/// INV-008 at this layer, and the same discipline
/// [`RegionFault`](crate::region::RegionFault) applies: an illegal operation is a distinct
/// typed value carrying both halves of why, never a panic, never a silent no-op, never a
/// bare `false`. Exhaustion is deliberately *not* here — it is an outcome, not a fault.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BudgetFault {
    /// Nothing measures this dimension, so spend on it cannot be recorded.
    ///
    /// The alternative — record it anyway — would put a number where RFC 0026 requires an
    /// absence: "a dimension the engine does not measure is absent, never zero". A caller
    /// that wants this charge to land declares the meter.
    UnmeteredDimension {
        /// The dimension the call named.
        dimension: CostDimension,
    },
    /// Recording this spend would overflow the accounting itself.
    ///
    /// Reachable only on an unbounded dimension, where no ceiling stops the sum first. It
    /// is a fault rather than an exhaustion because no *declared* ceiling was reached:
    /// reporting `BudgetExhausted` for it would name a limit nobody set.
    SpendOverflow {
        /// The dimension the call named.
        dimension: CostDimension,
        /// The spend already recorded.
        spent: u64,
        /// What the charge asked for.
        requested: u64,
    },
    /// A reservation is already outstanding on this dimension.
    ///
    /// At most one is in flight at a time, which is the shape the publication store
    /// already has: `StagedPublication` and `CommittedContent` are linear typestates, each
    /// consumed by the call that advances it.
    ReserveOverReservation {
        /// The dimension the call named.
        dimension: CostDimension,
    },
    /// Nothing is reserved on this dimension, so there is nothing to settle or release.
    NoReservation {
        /// The dimension the call named.
        dimension: CostDimension,
    },
    /// A settle asked to charge more than was reserved.
    ///
    /// Refused rather than truncated: the excess was never held against the ceiling, so
    /// charging it here would let a settle spend headroom that was never checked — which
    /// is how "a settle never exhausts" would stop being true.
    SettleExceedsReservation {
        /// The dimension the call named.
        dimension: CostDimension,
        /// What was reserved.
        reserved: u64,
        /// What the settle asked to charge.
        actual: u64,
    },
    /// A checkpoint was taken while a publication's headroom is still reserved.
    ///
    /// The budget-side form of
    /// [`RegionFault::SuspendWithProvisionalEvidence`](crate::region::RegionFault::SuspendWithProvisionalEvidence):
    /// a checkpoint binds committed evidence to a spend figure, and an outstanding
    /// reservation means that figure is not settled yet. Binding to it would name a cost
    /// that is still going to change.
    CheckpointWithReservation {
        /// The dimension still holding a reservation.
        dimension: CostDimension,
    },
    /// A checkpoint reported fewer committed publications than the one before it.
    ///
    /// > `task.status` is monotonic. Reported milestones and `committed_evidence` only
    /// > grow, and a terminal status never changes.
    /// >
    /// > — `notes/plan/rfcs/0026-continuumd-native-protocol.md`, "Task lifecycle"
    CheckpointRegressesCommitted {
        /// What the call reported.
        committed: u32,
        /// What the previous checkpoint reported.
        previous: u32,
    },
}

impl fmt::Display for BudgetFault {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnmeteredDimension { dimension } => write!(
                f,
                "nothing measures {dimension}, so spend on it cannot be recorded"
            ),
            Self::SpendOverflow {
                dimension,
                spent,
                requested,
            } => write!(
                f,
                "charging {requested} to {dimension} would overflow the {spent} already recorded"
            ),
            Self::ReserveOverReservation { dimension } => {
                write!(f, "{dimension} already holds a reservation")
            }
            Self::NoReservation { dimension } => {
                write!(f, "{dimension} holds no reservation")
            }
            Self::SettleExceedsReservation {
                dimension,
                reserved,
                actual,
            } => write!(
                f,
                "{dimension} reserved {reserved}, so a settle of {actual} is not covered"
            ),
            Self::CheckpointWithReservation { dimension } => write!(
                f,
                "cannot checkpoint while {dimension} holds an unsettled reservation"
            ),
            Self::CheckpointRegressesCommitted {
                committed,
                previous,
            } => write!(
                f,
                "committed evidence is monotone: {committed} is fewer than the {previous} \
                 already checkpointed"
            ),
        }
    }
}

impl core::error::Error for BudgetFault {}

/// One task's budget accounting: what it may spend, what it has spent, what is held in
/// flight, and every checkpoint it has bound committed evidence to.
///
/// Deliberately not [`Clone`]: a ledger is one task's single accounting, and a second copy
/// of it is a second answer to "what did this cost" — the reason
/// [`Ledger`](crate::region::obligation::Ledger) is not `Clone` either. Deliberately
/// without interior mutability: every operation takes `&mut self`, so a run is a sequence
/// of moves a caller wrote down (INV-005).
#[derive(Debug)]
pub struct BudgetLedger {
    budget: Budget,
    meters: MeterSet,
    spend: Spend,
    reserved: [Option<u64>; 9],
    checkpoints: Vec<Checkpoint>,
    exhaustion: Option<Exhaustion>,
}

impl BudgetLedger {
    /// A ledger for `budget`, measuring what `meters` measures.
    ///
    /// Both are parameters rather than defaults because INV-005 makes explicitness the
    /// rule and because each is a claim: the budget is what the caller asked for, and the
    /// meter set is what the service can actually check.
    #[must_use]
    pub fn new(budget: Budget, meters: MeterSet) -> Self {
        Self {
            budget,
            meters,
            spend: Spend::measured_by(meters),
            reserved: [None; 9],
            checkpoints: Vec::new(),
            exhaustion: None,
        }
    }

    /// The ceilings in force. [`Self::update`] is the only thing that changes them.
    #[must_use]
    pub const fn budget(&self) -> &Budget {
        &self.budget
    }

    /// What this accounting measures. Fixed for the ledger's life.
    #[must_use]
    pub const fn meters(&self) -> MeterSet {
        self.meters
    }

    /// The recorded spend — the task's `Cost`.
    #[must_use]
    pub const fn spend(&self) -> &Spend {
        &self.spend
    }

    /// The headroom currently held in flight on `dimension`, if any.
    #[must_use]
    pub const fn reservation(&self, dimension: CostDimension) -> Option<u64> {
        self.reserved[dimension.index()]
    }

    /// The dimensions holding a reservation, in declaration order.
    ///
    /// Empty is the resting state: a reservation is a publication in flight, and the
    /// region layer's teardown leaves none outstanding. [`partial::EvidenceBook`] makes
    /// that a cross-check rather than an assumption.
    #[must_use]
    pub fn reservations(&self) -> Vec<CostDimension> {
        CostDimension::ALL
            .into_iter()
            .filter(|dimension| self.reservation(*dimension).is_some())
            .collect()
    }

    /// Every checkpoint taken, in order.
    #[must_use]
    pub fn checkpoints(&self) -> &[Checkpoint] {
        &self.checkpoints
    }

    /// The most recent checkpoint, when anything has been committed.
    #[must_use]
    pub fn last_checkpoint(&self) -> Option<&Checkpoint> {
        self.checkpoints.last()
    }

    /// The first ceiling this ledger ran out of, if any.
    ///
    /// *First*, not latest: a later charge against a different dimension does not change
    /// which one stopped the run, exactly as `Partial::tripped` reports the single bound
    /// that ended an exploration. Recording it makes an exhausted ledger self-describing,
    /// so a task's `failed_reason` need not be reconstructed from the charge that produced
    /// it.
    #[must_use]
    pub const fn exhaustion(&self) -> Option<&Exhaustion> {
        self.exhaustion.as_ref()
    }

    /// Whether `dimension` has a declared ceiling *and* a meter behind it.
    #[must_use]
    pub const fn is_enforced(&self, dimension: CostDimension) -> bool {
        self.budget.ceiling(dimension).is_declared()
            && matches!(self.meters.metering(dimension), Metering::Metered)
    }

    /// The dimensions this ledger actually enforces, in declaration order.
    #[must_use]
    pub fn enforced(&self) -> Vec<CostDimension> {
        CostDimension::ALL
            .into_iter()
            .filter(|dimension| self.is_enforced(*dimension))
            .collect()
    }

    /// The INV-007 manifest: every declared ceiling with no meter behind it.
    ///
    /// Computed from the budget and the meter set rather than maintained, so a service
    /// that gains a meter loses the omission automatically. An empty list means every
    /// declared ceiling is enforced — never that the question was not asked.
    #[must_use]
    pub fn omissions(&self) -> Vec<DimensionOmission> {
        CostDimension::ALL
            .into_iter()
            .filter(|dimension| {
                self.budget.ceiling(*dimension).is_declared()
                    && !self.meters.metering(*dimension).is_metered()
            })
            .map(DimensionOmission::new)
            .collect()
    }

    /// How much may still be charged to `dimension`, counting any reservation against it.
    ///
    /// [`None`] is *unbounded*. Reservations are subtracted because that is what a
    /// reservation is for: headroom held aside so a later charge cannot take it.
    #[must_use]
    pub fn headroom(&self, dimension: CostDimension) -> Option<u64> {
        let spent = self.spend.measured(dimension).unwrap_or(0);
        let reserved = self.reservation(dimension).unwrap_or(0);
        self.budget
            .ceiling(dimension)
            .headroom(spent.saturating_add(reserved))
    }

    // --- charging ------------------------------------------------------------------------

    /// Record `amount` of spend on `dimension`.
    ///
    /// Monotone and irrevocable: a charge that lands is part of the task's `Cost` forever.
    /// A charge that would pass the ceiling records nothing and returns
    /// [`ChargeOutcome::Exhausted`] — all-or-nothing, the way `bfs::explore` refuses *the
    /// next expansion* rather than admitting part of one, so a bounded run is a prefix of
    /// an unbounded one rather than a differently-shaped answer.
    ///
    /// # Errors
    ///
    /// [`BudgetFault::UnmeteredDimension`] when nothing measures `dimension`, or
    /// [`BudgetFault::SpendOverflow`] when no ceiling stops the sum and the accounting
    /// itself would wrap.
    pub fn charge(
        &mut self,
        dimension: CostDimension,
        amount: u64,
    ) -> Result<ChargeOutcome, BudgetFault> {
        self.require_meter(dimension)?;
        let spent = self.spend.measured(dimension).unwrap_or(0);
        if let Some(headroom) = self.headroom(dimension)
            && amount > headroom
        {
            let limit = self.budget.ceiling(dimension).limit().unwrap_or(u64::MAX);
            let exhaustion = Exhaustion::new(dimension, limit, spent, amount);
            self.exhaustion.get_or_insert(exhaustion);
            return Ok(ChargeOutcome::Exhausted(exhaustion));
        }
        let landed = spent
            .checked_add(amount)
            .ok_or(BudgetFault::SpendOverflow {
                dimension,
                spent,
                requested: amount,
            })?;
        self.spend.0[dimension.index()] = Some(landed);
        Ok(ChargeOutcome::Admitted {
            dimension,
            spent: landed,
            ceiling: self.budget.ceiling(dimension),
        })
    }

    /// Hold `amount` of headroom on `dimension` for work in flight, without recording it
    /// as spend.
    ///
    /// This is the budget-side half of
    /// [`WorkerStep::Reserve`](crate::region::worker::WorkerStep::Reserve): a publication
    /// that has been staged will cost something, that cost has to be unavailable to
    /// anything else, and it is not spend until the publication commits. The `bytes`
    /// dimension is the case that makes it necessary — RFC 0027's byte budget is *the
    /// enforced context contract*, and a payload's true size is known only after it is
    /// built.
    ///
    /// # Errors
    ///
    /// [`BudgetFault::UnmeteredDimension`], or [`BudgetFault::ReserveOverReservation`]
    /// when one is already outstanding on this dimension.
    pub fn reserve(
        &mut self,
        dimension: CostDimension,
        amount: u64,
    ) -> Result<ChargeOutcome, BudgetFault> {
        self.require_meter(dimension)?;
        if self.reservation(dimension).is_some() {
            return Err(BudgetFault::ReserveOverReservation { dimension });
        }
        let spent = self.spend.measured(dimension).unwrap_or(0);
        if let Some(headroom) = self.headroom(dimension)
            && amount > headroom
        {
            let limit = self.budget.ceiling(dimension).limit().unwrap_or(u64::MAX);
            let exhaustion = Exhaustion::new(dimension, limit, spent, amount);
            self.exhaustion.get_or_insert(exhaustion);
            return Ok(ChargeOutcome::Exhausted(exhaustion));
        }
        self.reserved[dimension.index()] = Some(amount);
        Ok(ChargeOutcome::Admitted {
            dimension,
            spent,
            ceiling: self.budget.ceiling(dimension),
        })
    }

    /// Consume `dimension`'s reservation, charging `actual` and refunding the rest.
    ///
    /// Returns the refunded headroom. **A settle can never exhaust**: the reservation
    /// already held the headroom against the ceiling, so the spend it becomes was
    /// admissible when it was reserved. That is why this returns a number rather than a
    /// [`ChargeOutcome`] — an arm that cannot happen is worse than an arm that does not
    /// exist.
    ///
    /// # Errors
    ///
    /// [`BudgetFault::UnmeteredDimension`], [`BudgetFault::NoReservation`],
    /// [`BudgetFault::SettleExceedsReservation`] when `actual` is more than was held, or
    /// [`BudgetFault::SpendOverflow`].
    pub fn settle(&mut self, dimension: CostDimension, actual: u64) -> Result<u64, BudgetFault> {
        self.require_meter(dimension)?;
        let reserved = self
            .reservation(dimension)
            .ok_or(BudgetFault::NoReservation { dimension })?;
        if actual > reserved {
            return Err(BudgetFault::SettleExceedsReservation {
                dimension,
                reserved,
                actual,
            });
        }
        let spent = self.spend.measured(dimension).unwrap_or(0);
        let landed = spent
            .checked_add(actual)
            .ok_or(BudgetFault::SpendOverflow {
                dimension,
                spent,
                requested: actual,
            })?;
        self.reserved[dimension.index()] = None;
        self.spend.0[dimension.index()] = Some(landed);
        Ok(reserved - actual)
    }

    /// Give `dimension`'s whole reservation back, charging nothing.
    ///
    /// Returns the refunded headroom. This is the budget-side half of the discard the
    /// region layer performs when cancellation reaches a staged publication: the artifact
    /// leaves "no index entry, no receipt, nothing a reader can observe", so its bytes were
    /// never written and its headroom was never spent. Compute already charged while the
    /// publication was staged stays charged, because it happened.
    ///
    /// # Errors
    ///
    /// [`BudgetFault::UnmeteredDimension`] or [`BudgetFault::NoReservation`].
    pub fn release(&mut self, dimension: CostDimension) -> Result<u64, BudgetFault> {
        self.require_meter(dimension)?;
        let reserved = self
            .reservation(dimension)
            .ok_or(BudgetFault::NoReservation { dimension })?;
        self.reserved[dimension.index()] = None;
        Ok(reserved)
    }

    // --- checkpoints ---------------------------------------------------------------------

    /// Bind the spend so far to `committed` — the number of publications the region layer
    /// has committed for this task.
    ///
    /// This is the join. The region layer counts commitments
    /// ([`EvidenceLedger::committed`](crate::region::worker::EvidenceLedger::committed))
    /// and knows nothing about cost; this ledger measures cost and knows nothing about
    /// artifacts; a checkpoint is the pair, and it is what lets a partial result say what
    /// it spent *and* what it committed instead of only one of the two.
    ///
    /// # Errors
    ///
    /// [`BudgetFault::CheckpointWithReservation`] when a publication's headroom is still
    /// in flight, or [`BudgetFault::CheckpointRegressesCommitted`] when `committed` is
    /// below the previous checkpoint's — INV-009 and RFC 0026's `task.status`
    /// monotonicity.
    pub fn checkpoint(&mut self, committed: u32) -> Result<Checkpoint, BudgetFault> {
        if let Some(dimension) = self.reservations().first() {
            return Err(BudgetFault::CheckpointWithReservation {
                dimension: *dimension,
            });
        }
        if let Some(previous) = self.checkpoints.last()
            && committed < previous.committed()
        {
            return Err(BudgetFault::CheckpointRegressesCommitted {
                committed,
                previous: previous.committed(),
            });
        }
        let sequence = u32::try_from(self.checkpoints.len()).unwrap_or(u32::MAX);
        let checkpoint = Checkpoint::new(sequence, committed, self.spend);
        self.checkpoints.push(checkpoint);
        Ok(checkpoint)
    }

    // --- mid-flight update ---------------------------------------------------------------

    /// Apply RFC 0026's `task.update_budget` to one dimension, returning what it did.
    ///
    /// The ceiling is *always* recorded, on every arm including
    /// [`UpdateOutcome::Suspend`] and [`UpdateOutcome::Unenforced`]: the daemon's
    /// `TaskRecord.budget` reports the budget in force, and a suspension that left the old
    /// ceiling in place would report a bound the caller has withdrawn. What varies is what
    /// the update *means for the run*, and that is what the outcome names.
    ///
    /// This is infallible by construction — there is no illegal update — which is why it
    /// returns a bare [`UpdateOutcome`] rather than a [`Result`]. The one thing a caller
    /// might expect to be refused, lowering below spend, is a defined behaviour rather than
    /// a refusal.
    pub fn update(&mut self, dimension: CostDimension, ceiling: Ceiling) -> UpdateOutcome {
        let from = self.budget.ceiling(dimension);
        self.budget.0[dimension.index()] = ceiling;
        if !self.meters.metering(dimension).is_metered() {
            return UpdateOutcome::Unenforced(DimensionOmission::new(dimension));
        }
        let spent = self.spend.measured(dimension).unwrap_or(0);
        match (from.limit(), ceiling.limit()) {
            (Some(before), Some(after)) if after == before => {
                UpdateOutcome::Unchanged { dimension, ceiling }
            }
            (None, None) => UpdateOutcome::Unchanged { dimension, ceiling },
            (_, Some(after)) if after < spent => UpdateOutcome::Suspend(Suspension::new(
                dimension,
                after,
                spent,
                self.checkpoints.last().copied(),
            )),
            (Some(before), Some(after)) if after < before => UpdateOutcome::Tightened {
                dimension,
                from,
                to: ceiling,
                spent,
            },
            (None, Some(_)) => UpdateOutcome::Tightened {
                dimension,
                from,
                to: ceiling,
                spent,
            },
            _ => UpdateOutcome::Raised {
                dimension,
                from,
                to: ceiling,
            },
        }
    }

    /// A canonical multi-line rendering, for byte-identity comparison.
    ///
    /// Every line is a fact in a fixed order with no timestamp, address or iteration order
    /// in it, so two runs of one sequence of operations render byte-identically (INV-005) —
    /// the same device [`Finalization::render`](crate::region::Finalization::render) uses.
    #[must_use]
    pub fn render(&self) -> String {
        let mut out = String::new();
        out.push_str(&self.budget.render());
        out.push('\n');
        out.push_str(&self.meters.render());
        out.push('\n');
        out.push_str(&self.spend.render());
        out.push('\n');
        out.push_str("enforced:");
        for dimension in self.enforced() {
            out.push(' ');
            out.push_str(dimension.token());
        }
        out.push('\n');
        out.push_str("omissions:");
        for omission in self.omissions() {
            out.push(' ');
            out.push_str(&omission.render());
        }
        out.push('\n');
        for checkpoint in &self.checkpoints {
            out.push_str(&checkpoint.render());
            out.push('\n');
        }
        out.push_str(&match &self.exhaustion {
            Some(exhaustion) => format!("exhaustion: {exhaustion}\n"),
            None => "exhaustion: none\n".to_owned(),
        });
        out
    }

    fn require_meter(&self, dimension: CostDimension) -> Result<(), BudgetFault> {
        if self.meters.metering(dimension).is_metered() {
            Ok(())
        } else {
            Err(BudgetFault::UnmeteredDimension { dimension })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn states_ledger(limit: u64) -> BudgetLedger {
        BudgetLedger::new(
            Budget::unbounded().with(CostDimension::States, limit),
            MeterSet::STATES_ONLY,
        )
    }

    #[test]
    fn a_metered_dimension_reads_zero_and_an_unmetered_one_reads_absent() {
        let ledger = states_ledger(8);
        assert_eq!(ledger.spend().measured(CostDimension::States), Some(0));
        assert_eq!(
            ledger.spend().measured(CostDimension::WallMs),
            None,
            "a dimension the engine does not measure is absent, never zero"
        );
    }

    #[test]
    fn a_charge_that_would_pass_the_ceiling_records_nothing() {
        let mut ledger = states_ledger(8);
        assert!(
            ledger
                .charge(CostDimension::States, 8)
                .expect("metered")
                .is_admitted()
        );
        let outcome = ledger.charge(CostDimension::States, 1).expect("metered");
        assert_eq!(
            outcome.exhaustion(),
            Some(&Exhaustion::new(CostDimension::States, 8, 8, 1))
        );
        assert_eq!(
            ledger.spend().measured(CostDimension::States),
            Some(8),
            "an exhausted charge is all-or-nothing"
        );
        assert_eq!(
            ledger.exhaustion().map(Exhaustion::dimension),
            Some(CostDimension::States)
        );
    }

    #[test]
    fn the_first_exhausted_dimension_is_the_one_the_ledger_keeps() {
        let mut ledger = BudgetLedger::new(
            Budget::unbounded()
                .with(CostDimension::States, 1)
                .with(CostDimension::Bytes, 1),
            MeterSet::all(),
        );
        ledger.charge(CostDimension::Bytes, 4).expect("metered");
        ledger.charge(CostDimension::States, 4).expect("metered");
        assert_eq!(
            ledger.exhaustion().map(Exhaustion::dimension),
            Some(CostDimension::Bytes),
            "the bound that tripped first is the one BudgetExhausted names"
        );
    }

    #[test]
    fn charging_an_unmetered_dimension_is_refused_rather_than_invented() {
        let mut ledger = states_ledger(8);
        assert_eq!(
            ledger.charge(CostDimension::WallMs, 1),
            Err(BudgetFault::UnmeteredDimension {
                dimension: CostDimension::WallMs
            })
        );
        assert_eq!(ledger.spend().measured(CostDimension::WallMs), None);
    }

    #[test]
    fn an_undeclared_metered_dimension_is_measured_and_never_exhausts() {
        let mut ledger = BudgetLedger::new(Budget::unbounded(), MeterSet::all());
        let outcome = ledger
            .charge(CostDimension::Tokens, u64::MAX / 2)
            .expect("metered");
        assert!(outcome.is_admitted());
        assert!(
            ledger.omissions().is_empty(),
            "nothing was declared, so nothing was omitted"
        );
        assert!(ledger.enforced().is_empty());
    }

    #[test]
    fn an_unbounded_dimension_reports_overflow_rather_than_a_ceiling_nobody_set() {
        let mut ledger = BudgetLedger::new(Budget::unbounded(), MeterSet::all());
        ledger
            .charge(CostDimension::Tokens, u64::MAX)
            .expect("metered");
        assert_eq!(
            ledger.charge(CostDimension::Tokens, 1),
            Err(BudgetFault::SpendOverflow {
                dimension: CostDimension::Tokens,
                spent: u64::MAX,
                requested: 1,
            })
        );
    }

    #[test]
    fn a_reservation_holds_headroom_a_later_charge_cannot_take() {
        let mut ledger = BudgetLedger::new(
            Budget::unbounded().with(CostDimension::Bytes, 100),
            MeterSet::all(),
        );
        ledger.reserve(CostDimension::Bytes, 60).expect("metered");
        assert_eq!(ledger.headroom(CostDimension::Bytes), Some(40));
        assert!(
            ledger
                .charge(CostDimension::Bytes, 41)
                .expect("metered")
                .exhaustion()
                .is_some(),
            "the reservation is not available to anything else"
        );
        assert_eq!(
            ledger.spend().measured(CostDimension::Bytes),
            Some(0),
            "a reservation is not spend"
        );
    }

    #[test]
    fn settling_charges_what_was_used_and_refunds_the_rest() {
        let mut ledger = BudgetLedger::new(
            Budget::unbounded().with(CostDimension::Bytes, 100),
            MeterSet::all(),
        );
        ledger.reserve(CostDimension::Bytes, 60).expect("metered");
        assert_eq!(ledger.settle(CostDimension::Bytes, 25), Ok(35));
        assert_eq!(ledger.spend().measured(CostDimension::Bytes), Some(25));
        assert_eq!(ledger.reservation(CostDimension::Bytes), None);
        assert_eq!(ledger.headroom(CostDimension::Bytes), Some(75));
    }

    #[test]
    fn releasing_refunds_the_headroom_and_charges_nothing() {
        let mut ledger = BudgetLedger::new(
            Budget::unbounded().with(CostDimension::Bytes, 100),
            MeterSet::all(),
        );
        ledger.reserve(CostDimension::Bytes, 60).expect("metered");
        assert_eq!(ledger.release(CostDimension::Bytes), Ok(60));
        assert_eq!(
            ledger.spend().measured(CostDimension::Bytes),
            Some(0),
            "a discarded publication wrote no bytes"
        );
        assert_eq!(
            ledger.release(CostDimension::Bytes),
            Err(BudgetFault::NoReservation {
                dimension: CostDimension::Bytes
            })
        );
    }

    #[test]
    fn a_settle_may_not_charge_more_than_was_reserved() {
        let mut ledger = BudgetLedger::new(
            Budget::unbounded().with(CostDimension::Bytes, 100),
            MeterSet::all(),
        );
        ledger.reserve(CostDimension::Bytes, 10).expect("metered");
        assert_eq!(
            ledger.settle(CostDimension::Bytes, 11),
            Err(BudgetFault::SettleExceedsReservation {
                dimension: CostDimension::Bytes,
                reserved: 10,
                actual: 11,
            })
        );
        assert_eq!(
            ledger.reserve(CostDimension::Bytes, 1),
            Err(BudgetFault::ReserveOverReservation {
                dimension: CostDimension::Bytes
            })
        );
    }

    #[test]
    fn a_checkpoint_may_not_be_taken_mid_publication() {
        let mut ledger = BudgetLedger::new(
            Budget::unbounded().with(CostDimension::Bytes, 100),
            MeterSet::all(),
        );
        ledger.reserve(CostDimension::Bytes, 10).expect("metered");
        assert_eq!(
            ledger.checkpoint(1),
            Err(BudgetFault::CheckpointWithReservation {
                dimension: CostDimension::Bytes
            })
        );
        ledger.settle(CostDimension::Bytes, 10).expect("reserved");
        assert_eq!(ledger.checkpoint(1).map(|c| c.committed()), Ok(1));
    }

    #[test]
    fn committed_evidence_is_monotone_across_checkpoints() {
        let mut ledger = states_ledger(64);
        ledger.charge(CostDimension::States, 4).expect("metered");
        ledger.checkpoint(1).expect("nothing reserved");
        ledger.charge(CostDimension::States, 4).expect("metered");
        ledger.checkpoint(2).expect("monotone");
        assert_eq!(
            ledger.checkpoint(1),
            Err(BudgetFault::CheckpointRegressesCommitted {
                committed: 1,
                previous: 2
            })
        );
        assert_eq!(ledger.checkpoints().len(), 2);
    }

    #[test]
    fn a_declared_ceiling_with_no_meter_is_named_rather_than_silently_passed() {
        let ledger = BudgetLedger::new(
            Budget::unbounded()
                .with(CostDimension::States, 8)
                .with(CostDimension::WallMs, 1)
                .with(CostDimension::Tokens, 2),
            MeterSet::STATES_ONLY,
        );
        assert_eq!(ledger.enforced(), vec![CostDimension::States]);
        let subjects: Vec<String> = ledger.omissions().iter().map(|o| o.subject()).collect();
        assert_eq!(
            subjects,
            vec!["budget.wall_ms".to_owned(), "budget.tokens".to_owned()],
            "declared but unmetered, in declaration order"
        );
    }

    #[test]
    fn the_render_of_one_ledger_is_byte_identical_across_two_runs() {
        let build = || {
            let mut ledger = BudgetLedger::new(
                Budget::unbounded()
                    .with(CostDimension::States, 32)
                    .with(CostDimension::CpuMs, 10),
                MeterSet::STATES_ONLY,
            );
            ledger.charge(CostDimension::States, 12).expect("metered");
            ledger.checkpoint(1).expect("nothing reserved");
            ledger.charge(CostDimension::States, 25).expect("metered");
            ledger.render()
        };
        let first = build();
        assert_eq!(first.as_bytes(), build().as_bytes());
        assert!(!first.is_empty(), "a vacuous render proves nothing");
    }
}
