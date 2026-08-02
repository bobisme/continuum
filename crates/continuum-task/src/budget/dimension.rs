//! The nine cost dimensions, the ceilings declared over them, and the meters that decide
//! which of those ceilings can be enforced at all.
//!
//! # One list, and this is not a second copy of it
//!
//! > `Cost` reports the same nine dimensions as `Budget` (`wall_ms`, `cpu_ms`,
//! > `memory_bytes`, `states`, `solver_ms`, `proof_ms`, `tokens`, `candidates`, `bytes`)
//! > and nothing else (SD-12: one budget/cost dimension list, shared with
//! > `schemas/verification-task.schema.json` and the plan §8.6 cost ledger).
//! >
//! > — `notes/plan/rfcs/0026-continuumd-native-protocol.md`, "Cost and omissions"
//!
//! The list is normative in three places that SD-12 holds identical — the IDL's `struct
//! Budget` (`notes/plan/schemas/continuumd-native-protocol.idl`), the `budget` object of
//! `notes/plan/schemas/verification-task.schema.json` (which sets `additionalProperties:
//! false`), and plan §8.6's cost ledger. RFC 0026's F16 records what it costs to change:
//! "a tenth dimension moves four artifacts together, one of them rank 1", which is
//! exactly why nothing here invents, drops, renames or reorders a member.
//!
//! [`CostDimension`] is therefore a *spelling* of that list, not a fork of it, and it is
//! held to its source mechanically: `crates/continuum-task/tests/budget_dimensions.rs`
//! parses the IDL's `struct Budget` and RFC 0026's own prose enumeration and requires
//! [`CostDimension::ALL`] to agree with both, member for member and in order. The device
//! is the one [`WorkerState::status_token`](crate::region::worker::WorkerState::status_token)
//! already uses for `TaskStatus`: two crates agree through a shared token rather than
//! through an import, because plan §20 gives `continuumd` the wire and gives this crate
//! the lifecycle, and there is no edge between them.
//!
//! The wire *encoding* of the same list — `Budget`/`Cost` as `protocol_struct!`s with
//! `Optional` fields, `tokenizer_id` beside `tokens` — stays in
//! `crates/continuumd/src/protocol/envelope.rs`, where a wire form belongs.
//!
//! # Declared is not enforced
//!
//! A ceiling somebody wrote down and a ceiling this accounting can actually stop work at
//! are different facts, and collapsing them is how a budget silently becomes advisory.
//! [`Ceiling`] carries the first, [`MeterSet`] the second, and the four cases they make
//! are all named:
//!
//! | | metered | unmetered |
//! |---|---|---|
//! | **declared** | *enforced* — a charge can exhaust it | *omitted* — [`DimensionOmission`], and a charge is refused |
//! | **undeclared** | *measured* — a charge is admitted and can never exhaust | *inert* — a charge is refused, and nothing is owed |
//!
//! The top-right cell is the INV-007 discipline `continuumd` already practises: bn-18z's
//! `unenforced` in `crates/continuumd/src/daemon/verification.rs` walks the eight
//! dimensions its engine has no meter for and emits one typed omission per *declared*
//! one, because
//!
//! > `states` is the one dimension with an enforcement path; depth and transitions have no
//! > wire dimension […] `wall_ms` and `cpu_ms` need a clock.
//! >
//! > — `crates/continuumd/src/daemon/verification.rs`
//!
//! [`MeterSet::STATES_ONLY`] is that daemon's meter set written down as a value, so the
//! omission list stops being a hand-maintained array and becomes a consequence of what
//! the accounting can measure.

use core::fmt;

/// One of the nine budget/cost dimensions.
///
/// The members and their order are the IDL's `struct Budget`, field for field. Ordering
/// derives from the declaration order rather than from the token spelling, so every
/// canonical rendering in this module lists dimensions the way the wire declares them and
/// not the way ASCII happens to sort them (INV-005).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CostDimension {
    /// `wall_ms` — elapsed wall-clock milliseconds.
    WallMs,
    /// `cpu_ms` — consumed CPU milliseconds.
    CpuMs,
    /// `memory_bytes` — peak resident bytes.
    MemoryBytes,
    /// `states` — explored states. `continuum-engine-reference`'s own `Bounds::states` is
    /// the one dimension with an enforcement path anywhere in this workspace today.
    States,
    /// `solver_ms` — milliseconds spent inside a solver.
    SolverMs,
    /// `proof_ms` — milliseconds spent in proof search or checking.
    ProofMs,
    /// `tokens` — model tokens. Advisory and tokenizer-relative; RFC 0026 requires a
    /// `tokenizer_id` beside any reported count, and that identity is the wire's to carry.
    Tokens,
    /// `candidates` — synthesis or repair candidates considered.
    Candidates,
    /// `bytes` — returned context bytes. The enforced context contract (RFC 0027).
    Bytes,
}

impl CostDimension {
    /// The nine dimensions, in the IDL's declaration order.
    ///
    /// Nine is not a coincidence to be re-derived: it is SD-12's list, and a tenth member
    /// here without the four artifacts RFC 0026's F16 names is precisely the drift SD-12
    /// exists to prevent.
    pub const ALL: [Self; 9] = [
        Self::WallMs,
        Self::CpuMs,
        Self::MemoryBytes,
        Self::States,
        Self::SolverMs,
        Self::ProofMs,
        Self::Tokens,
        Self::Candidates,
        Self::Bytes,
    ];

    /// This dimension's wire spelling — the IDL field name, exactly.
    #[must_use]
    pub const fn token(self) -> &'static str {
        match self {
            Self::WallMs => "wall_ms",
            Self::CpuMs => "cpu_ms",
            Self::MemoryBytes => "memory_bytes",
            Self::States => "states",
            Self::SolverMs => "solver_ms",
            Self::ProofMs => "proof_ms",
            Self::Tokens => "tokens",
            Self::Candidates => "candidates",
            Self::Bytes => "bytes",
        }
    }

    /// This dimension's position in [`Self::ALL`].
    ///
    /// The index into every nine-slot array in this module. Written as a match rather
    /// than as a search so it is total and constant-time, and so adding a member without
    /// widening the arrays does not compile.
    #[must_use]
    pub const fn index(self) -> usize {
        match self {
            Self::WallMs => 0,
            Self::CpuMs => 1,
            Self::MemoryBytes => 2,
            Self::States => 3,
            Self::SolverMs => 4,
            Self::ProofMs => 5,
            Self::Tokens => 6,
            Self::Candidates => 7,
            Self::Bytes => 8,
        }
    }

    /// The typed subject naming this dimension's declared ceiling, for an INV-007
    /// omission.
    ///
    /// `budget.wall_ms`, and the prefix is not decoration: it is the spelling bn-18z's
    /// `unenforced` already emits into `continuumd`'s omission manifest, so a daemon
    /// projecting [`DimensionOmission`] onto the wire writes the same subject it writes
    /// today rather than a second one.
    ///
    /// > The subject is stated in typed terms, never as free-form prose about the user's
    /// > source.
    /// >
    /// > — `notes/plan/rfcs/0026-continuumd-native-protocol.md`, "Cost and omissions"
    #[must_use]
    pub fn subject(self) -> String {
        format!("budget.{}", self.token())
    }
}

impl fmt::Display for CostDimension {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.token())
    }
}

/// What one dimension of a [`Budget`](super::Budget) declares.
///
/// Two members rather than an `Option<u64>` because the absent case has a meaning worth
/// naming: RFC 0026's `Budget` fields are all `optional`, and an absent one is *no
/// ceiling declared*, never a ceiling of zero. A zero ceiling is a real and different
/// thing — it admits no spend at all — and the two must not be spelled the same way.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Ceiling {
    /// No ceiling was declared for this dimension. Spend on it can never exhaust.
    Unbounded,
    /// A ceiling was declared. Spend may reach it and may not pass it.
    At(u64),
}

impl Ceiling {
    /// The declared limit, when there is one.
    #[must_use]
    pub const fn limit(self) -> Option<u64> {
        match self {
            Self::Unbounded => None,
            Self::At(limit) => Some(limit),
        }
    }

    /// Whether a ceiling was declared at all.
    #[must_use]
    pub const fn is_declared(self) -> bool {
        matches!(self, Self::At(_))
    }

    /// How much may still be spent under this ceiling given `committed` spend.
    ///
    /// [`None`] means *unbounded*, which is deliberately not `Some(u64::MAX)`: a caller
    /// that has to distinguish "no ceiling" from "a very large ceiling" — the update
    /// legality table does — cannot do it once the two are the same number.
    #[must_use]
    pub const fn headroom(self, committed: u64) -> Option<u64> {
        match self {
            Self::Unbounded => None,
            Self::At(limit) => Some(limit.saturating_sub(committed)),
        }
    }
}

impl fmt::Display for Ceiling {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unbounded => f.write_str("unbounded"),
            Self::At(limit) => write!(f, "{limit}"),
        }
    }
}

/// Whether this accounting can measure one dimension.
///
/// The whole of the INV-007 discipline turns on this being a separate question from
/// [`Ceiling`]. A daemon that reports a ceiling it cannot measure against is reporting a
/// promise; naming the metering makes the promise a checkable claim instead.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Metering {
    /// There is a meter: spend on this dimension is measured, and a declared ceiling on
    /// it is enforced.
    Metered,
    /// There is no meter. Spend is not measured, so a declared ceiling on this dimension
    /// is a typed [`DimensionOmission`] and never a silently satisfied one.
    Unmetered,
}

impl Metering {
    /// A stable token for canonical rendering.
    #[must_use]
    pub const fn token(self) -> &'static str {
        match self {
            Self::Metered => "metered",
            Self::Unmetered => "unmetered",
        }
    }

    /// Whether a meter exists.
    #[must_use]
    pub const fn is_metered(self) -> bool {
        matches!(self, Self::Metered)
    }
}

impl fmt::Display for Metering {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.token())
    }
}

/// Which of the nine dimensions an accounting has a meter for.
///
/// Declared at construction and never changed afterwards: what a service can measure is a
/// property of the service, not of the request, and a meter that appeared mid-run would
/// make an omission manifest depend on when it was asked for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MeterSet([bool; 9]);

impl MeterSet {
    /// An accounting that measures nothing.
    ///
    /// Every declared ceiling under it is an omission. That is the honest starting point:
    /// a meter is something a service has to *add*, and defaulting to "measures
    /// everything" is the silent-enforcement claim this type exists to refuse.
    #[must_use]
    pub const fn none() -> Self {
        Self([false; 9])
    }

    /// An accounting that measures every dimension.
    ///
    /// Present for tests and for a service that genuinely meters all nine; no
    /// implementation in this workspace does yet.
    #[must_use]
    pub const fn all() -> Self {
        Self([true; 9])
    }

    /// The meter set of a service whose only enforcement path is the engine's state
    /// bound.
    ///
    /// This is `continuumd` as bn-18z left it: `verification::bounds_of` maps
    /// `budget.states` onto `bfs::Bounds` and nothing else, and `unenforced` names the
    /// other eight. Naming it here is what lets that hand-written omission list be
    /// replaced by [`Self::omissions`] over a declared budget rather than kept in sync by
    /// hand.
    pub const STATES_ONLY: Self =
        Self([false, false, false, true, false, false, false, false, false]);

    /// The same set, with a meter for `dimension`.
    #[must_use]
    pub fn with(mut self, dimension: CostDimension) -> Self {
        self.0[dimension.index()] = true;
        self
    }

    /// Whether `dimension` is measured.
    #[must_use]
    pub const fn metering(self, dimension: CostDimension) -> Metering {
        if self.0[dimension.index()] {
            Metering::Metered
        } else {
            Metering::Unmetered
        }
    }

    /// The dimensions with a meter, in declaration order.
    #[must_use]
    pub fn metered(self) -> Vec<CostDimension> {
        CostDimension::ALL
            .into_iter()
            .filter(|dimension| self.metering(*dimension).is_metered())
            .collect()
    }

    /// The dimensions without one, in declaration order.
    #[must_use]
    pub fn unmetered(self) -> Vec<CostDimension> {
        CostDimension::ALL
            .into_iter()
            .filter(|dimension| !self.metering(*dimension).is_metered())
            .collect()
    }

    /// A canonical one-line rendering: the metered dimensions, in declaration order.
    #[must_use]
    pub fn render(self) -> String {
        let mut out = String::from("meters:");
        for dimension in self.metered() {
            out.push(' ');
            out.push_str(dimension.token());
        }
        out
    }
}

impl fmt::Display for MeterSet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.render())
    }
}

/// A ceiling the caller declared and this accounting cannot enforce.
///
/// > Trimming to a ceiling MUST produce an `omissions` entry (INV-007) and MUST NOT drop
/// > an assurance dimension, a warning, an epoch, or a redaction stub.
/// >
/// > — `notes/plan/rfcs/0026-continuumd-native-protocol.md`, request envelope
///
/// The cost-domain form of the same rule is plan §8.6's: exceeding a ceiling yields
/// `BudgetExhausted` with a continuation, "never a silently smaller campaign (the
/// cost-domain form of INV-007)". A ceiling with no meter behind it cannot produce that
/// outcome at all, so the honest answer is this: the ceiling is recorded, the dimension is
/// named as unenforced, and nothing pretends the budget was respected.
///
/// It carries one dimension and no free-form text on purpose. The `reason` vocabulary is
/// RFC 0026's closed five (`budget`, `redaction`, `unsupported`, `heuristic-cutoff`,
/// `slice-irrelevant`) and lives on the wire, in `continuumd`; this type reports the token
/// its producer already uses rather than minting a second closed enum of reasons — the
/// same trade [`FailureReason`](crate::region::worker::FailureReason) makes for
/// `ErrorCode`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DimensionOmission {
    dimension: CostDimension,
}

impl DimensionOmission {
    /// The omission for `dimension`.
    #[must_use]
    pub const fn new(dimension: CostDimension) -> Self {
        Self { dimension }
    }

    /// Which dimension went unenforced.
    #[must_use]
    pub const fn dimension(self) -> CostDimension {
        self.dimension
    }

    /// The typed subject: `budget.<token>`.
    #[must_use]
    pub fn subject(self) -> String {
        self.dimension.subject()
    }

    /// The RFC 0026 `Omission.reason` token this omission carries.
    ///
    /// `unsupported`, not `budget`. The distinction matters and bn-18z already made it:
    /// reason `budget` means *the budget caused something to be left out of the answer*,
    /// while this omission says *the daemon has no meter for that dimension at all*,
    /// which is a missing capability. Reporting it as `budget` would suggest a ceiling
    /// did work that no ceiling did.
    #[must_use]
    pub const fn reason_token(self) -> &'static str {
        "unsupported"
    }

    /// A canonical one-line rendering.
    #[must_use]
    pub fn render(self) -> String {
        format!("{}:{}", self.reason_token(), self.subject())
    }
}

impl fmt::Display for DimensionOmission {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.render())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_nine_tokens_are_the_idl_budget_fields_in_declaration_order() {
        let tokens: Vec<&str> = CostDimension::ALL
            .into_iter()
            .map(CostDimension::token)
            .collect();
        assert_eq!(
            tokens,
            vec![
                "wall_ms",
                "cpu_ms",
                "memory_bytes",
                "states",
                "solver_ms",
                "proof_ms",
                "tokens",
                "candidates",
                "bytes",
            ],
            "SD-12 holds one budget/cost dimension list across the IDL, \
             verification-task.schema.json and the §8.6 ledger"
        );
    }

    #[test]
    fn every_dimension_indexes_its_own_slot_exactly_once() {
        let mut seen = [false; 9];
        for dimension in CostDimension::ALL {
            let index = dimension.index();
            assert!(
                !seen[index],
                "{dimension} shares a slot with another dimension"
            );
            seen[index] = true;
            assert_eq!(CostDimension::ALL[index], dimension);
        }
        assert!(
            seen.into_iter().all(|slot| slot),
            "a slot belongs to nobody"
        );
    }

    #[test]
    fn an_undeclared_ceiling_is_not_a_ceiling_of_zero() {
        assert_eq!(Ceiling::Unbounded.limit(), None);
        assert_eq!(Ceiling::At(0).limit(), Some(0));
        assert!(!Ceiling::Unbounded.is_declared());
        assert!(Ceiling::At(0).is_declared());
        assert_eq!(Ceiling::Unbounded.headroom(1_000), None);
        assert_eq!(Ceiling::At(0).headroom(0), Some(0));
    }

    #[test]
    fn the_states_only_meter_set_is_the_daemons_single_enforcement_path() {
        assert_eq!(MeterSet::STATES_ONLY.metered(), vec![CostDimension::States]);
        assert_eq!(
            MeterSet::STATES_ONLY.unmetered().len(),
            8,
            "bn-18z's `unenforced` names exactly eight dimensions"
        );
        assert_eq!(MeterSet::STATES_ONLY.render(), "meters: states");
    }

    #[test]
    fn a_meter_set_measures_nothing_until_a_meter_is_added() {
        let empty = MeterSet::none();
        assert!(empty.metered().is_empty());
        assert_eq!(empty.unmetered().len(), 9);
        let one = empty.with(CostDimension::Bytes);
        assert_eq!(one.metering(CostDimension::Bytes), Metering::Metered);
        assert_eq!(one.metering(CostDimension::Tokens), Metering::Unmetered);
        assert_eq!(MeterSet::all().unmetered(), Vec::new());
    }

    #[test]
    fn an_omission_names_the_subject_the_daemon_already_writes() {
        let omission = DimensionOmission::new(CostDimension::WallMs);
        assert_eq!(omission.subject(), "budget.wall_ms");
        assert_eq!(omission.reason_token(), "unsupported");
        assert_eq!(omission.render(), "unsupported:budget.wall_ms");
    }
}
