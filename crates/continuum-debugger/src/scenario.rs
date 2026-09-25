//! Scenario reduction: the owner, fault and value-domain passes of causal minimization
//! (START_HERE PR 18, bn-25z9o).
//!
//! # What it reduces
//!
//! [`crate::reduce`] shrinks one failing **run**: it deletes events and keeps the
//! happens-before past of the failure. It cannot remove a participant, a fault or a
//! value the run was configured with, because every event of the run belongs to that
//! configuration: the setup that spawns an unused replica is still a prefix of the run.
//! RFC 0028 names three more minimality classes, each over a dimension of the
//! **configuration**: `OwnerMinimal` (tasks and nodes reduced), `FaultMinimal` (no
//! unnecessary injected fault) and `ValueMinimal` (domains and names reduced). This
//! module reduces the configuration along those three [`Dimension`]s.
//!
//! A candidate here is a smaller configuration, never a subset of events. The
//! [`Scenario`] runs it: it builds the program under the candidate configuration,
//! executes it, and checks the preserved failure. So every kept configuration is a real
//! run of the program, not an edited trace. The event passes of [`crate::reduce`] then
//! reduce that run; the composition is the caller's (the register instantiation in
//! `continuum-asupersync`'s tests composes them and records both transcripts).
//!
//! # The passes
//!
//! Each pass is greedy first-improvement to a fixpoint along its dimension: generate the
//! candidates one step smaller than the current configuration along the dimension, run
//! them in the scenario's order, keep the first that fails, and start again from it.
//! The pass ends when no candidate fails. A **round** runs the owner, fault and value
//! passes in that order. Rounds repeat until a round keeps nothing, since a dropped fault
//! can make an owner unnecessary. In that last round every pass generated its
//! candidates from the final configuration, so its verdicts are the minimality evidence.
//!
//! Termination does not rest on the scenario's good will. The scenario gives each
//! configuration a [`Scenario::measure`], one number per dimension. A candidate of a
//! pass must be strictly smaller along the pass's dimension and no larger along the
//! other two. A candidate that is not is a contract breach: the reduction stops,
//! inconclusive with [`InconclusiveReason::EngineError`]. The engine compares each
//! candidate with the measure it observed when it kept the current version, so the sum of
//! the observed measures strictly decreases with every kept candidate; a memo hit that
//! says a candidate fails is a breach too, since it would equal a kept version. Every
//! generation and every candidate is charged, so the budget bounds the rest.
//!
//! # Scope: what a pass preserves (INV-013)
//!
//! A reduction may shrink how the failure is configured. It may not change what is
//! checked. Each pass declares its scope ([`Scenario::declare`]): the [`Preserved`]
//! facts every candidate keeps. The declaration must name [`Preserved::Property`], the
//! checked property over the same observer. A scenario that cannot keep the property
//! along a dimension (a value renaming of a property that names one value, for example)
//! returns an error instead. That pass is then refused: it runs nothing, keeps nothing,
//! and its minimality class is not claimed ([`DimensionEnd::ScopeRefused`]).
//!
//! Each candidate is also held to the scope before it runs ([`Scenario::admits`]). A
//! candidate outside it (a value domain too small for the property to be stated, or a
//! bound above the campaign's) is recorded as [`ConfigVerdict::OutOfScope`] and never
//! run or kept. A minimality class is a claim over the in-scope candidates only.
//!
//! # Verdicts
//!
//! [`Ran`] is what running a configuration showed: the failure reproduced, in the run
//! the scenario proposes ([`Ran::Fails`]); a decided non-failure ([`Ran::Holds`], with
//! how it was decided); a configuration that is no run of the program
//! ([`Ran::NotARun`]); or an INV-008 [`Ran::Inconclusive`], for example a bounded
//! schedule search that found no failure. An inconclusive candidate is not a
//! non-failure: a pass whose last candidates include one claims no minimality.
//!
//! # Mechanism preservation (bn-5kmuf)
//!
//! `Fails` is not enough to keep a candidate. The scenario is its own
//! [`crate::mechanism::Validator`], and the engine checks the proposed run against the
//! original failure's mechanism, renamed to the candidate's names. Only a run the check
//! preserves is kept. A run that reproduces the failure through another mechanism is
//! recorded as [`ConfigVerdict::MechanismLost`] with its typed reason and never kept; it
//! decides the candidate only when the scenario tried every run of it
//! ([`Scenario::search_exhausted`]), and otherwise withholds minimality. A check that
//! cannot decide is an INV-008 inconclusive. The input is checked before any pass: an
//! input whose run does not show the mechanism is refused
//! ([`ScenarioRefusal::MechanismNotInInput`]). So the reduced configuration is returned
//! only as a [`crate::mechanism::Validated`] one.
//!
//! # Budget, charged before work (INV-008)
//!
//! The reduction spends [`Budget`]'s replays and work units, charged before the work.
//! Each run is one replay plus the scenario's predicted cost ([`Scenario::run_cost`]).
//! Each generation of candidates is charged [`Scenario::candidates_cost`] before it is
//! built, each candidate at least one unit more before its measure and scope check, and
//! each memo lookup and insert its comparisons. The measure the engine compares against
//! is the one it observed when it kept the version, never a fresh one. A budget that runs out gives
//! [`ScenarioReduction::Inconclusive`] with the last configuration that failed.
//!
//! # The transcript (RFC 0028)
//!
//! RFC 0028 checks `OwnerMinimal`, `FaultMinimal` and `ValueMinimal` by a minimizer
//! transcript over the owners, the fault set and the value domain. Every reduction
//! returns one ([`ConfigAttempts`]): each candidate in the order it was tried, with its
//! dimension, round, the version of the configuration it was tried against, the
//! scenario's label of what it changed, its measure, the verdict and the scenario's
//! reason. A candidate judged from the memo of an identical earlier candidate is an entry
//! too ([`ConfigAttempt::memo_of`]). The versions themselves are returned, so a consumer
//! can re-run each kept configuration. The transcript is bounded like
//! [`crate::reduce`]'s ([`crate::reduce::TranscriptBound`], checked before an entry is
//! built, labels and reasons cut to [`REASON_CAP`] bytes with their lengths kept). A
//! minimality class is claimed only with a complete transcript.
//!
//! # Determinism (INV-006)
//!
//! The reduction reads no clock, random source, address or hash order. It is a pure
//! function of the scenario's answers and the budget. A scenario that searches schedules
//! must seed its search from the configuration, as the register instantiation does.

use std::collections::BTreeMap;

use continuum_value::assurance::InconclusiveReason;

use crate::mechanism::{
    self, Allowance, CheckEntry, Checked, Mechanism, Outcome, Rejection, Stage, Undecided,
    Validated, Validator,
};
use crate::reduce::{Budget, Exhausted, REASON_CAP, Spent, TranscriptBound};

/// A dimension of the configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Dimension {
    /// Owners: nodes, incarnations, tasks and the regions that hold them.
    Owner,
    /// The injected faults: crashes and cancellations, their number and where they
    /// fall.
    Fault,
    /// The value domain: values, epochs and names.
    Value,
}

impl Dimension {
    /// The dimensions in the order a round runs their passes.
    pub const ALL: [Self; 3] = [Self::Owner, Self::Fault, Self::Value];

    const fn index(self) -> usize {
        match self {
            Self::Owner => 0,
            Self::Fault => 1,
            Self::Value => 2,
        }
    }

    /// RFC 0028's minimality class for this dimension.
    #[must_use]
    pub const fn minimality(self) -> ScenarioGuarantee {
        match self {
            Self::Owner => ScenarioGuarantee::OwnerMinimal,
            Self::Fault => ScenarioGuarantee::FaultMinimal,
            Self::Value => ScenarioGuarantee::ValueMinimal,
        }
    }
}

/// A fact every candidate of a pass keeps: the pass's declared scope (INV-013).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Preserved {
    /// The checked property, over the same observer. Every declaration names it.
    Property,
    /// The defect under diagnosis: the program's code. A candidate changes the
    /// configuration it runs under, never the code.
    Defect,
    /// The semantics the run is judged under: the substrate, the binding, the crash
    /// semantics and the model.
    Semantics,
    /// The declared bounds: no candidate exceeds a bound the input's campaign declared.
    Bounds,
}

/// A smaller configuration, with the scenario's label of what it changed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Candidate<C> {
    /// The configuration.
    pub config: C,
    /// What it changed, as the transcript renders it. Rendering only.
    pub label: String,
}

/// What running a configuration showed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Ran {
    /// A run of the program under the configuration shows the preserved failure, with
    /// its mechanism.
    Fails,
    /// Decided: no run of the program under the configuration shows the preserved
    /// failure. The string says how it was decided.
    Holds(String),
    /// The configuration is no run of the program: the builder or the substrate
    /// refuses it.
    NotARun(String),
    /// Undecided (INV-008), with the reason and the scenario's rendering.
    Inconclusive(InconclusiveReason, String),
}

/// A failing run's configuration space: what the passes reduce.
pub trait Scenario {
    /// A configuration. The order is only for the memo; it carries no meaning.
    type Config: Clone + Ord;

    /// The scope of the pass over `dimension`: what every candidate keeps. It must name
    /// [`Preserved::Property`]. An error refuses the pass: the scenario cannot reduce
    /// this dimension without changing the checked property.
    ///
    /// # Errors
    ///
    /// Why the pass would change the checked property.
    fn declare(&self, dimension: Dimension) -> Result<Vec<Preserved>, String>;

    /// The size of `config` along each dimension, indexed owner, fault, value.
    fn measure(&self, config: &Self::Config) -> [u64; 3];

    /// The candidates one step smaller than `config` along `dimension`, in the order to
    /// try them.
    fn candidates(
        &self,
        config: &Self::Config,
        dimension: Dimension,
    ) -> Vec<Candidate<Self::Config>>;

    /// An upper bound on the work of [`Self::candidates`] from `config`, with the
    /// measure and the scope check of each candidate: charged before they are built.
    fn candidates_cost(&self, config: &Self::Config) -> u64;

    /// Whether `config` is inside the declared scope of the pass over `dimension`.
    ///
    /// # Errors
    ///
    /// Why it is not: the candidate is recorded out of scope and never run.
    fn admits(&self, dimension: Dimension, config: &Self::Config) -> Result<(), String>;

    /// An upper bound on the work of [`Self::run`] on `config`: charged before it runs.
    fn run_cost(&self, config: &Self::Config) -> u64;

    /// Run the program under `config` and check the preserved failure. A `Fails` names
    /// the run the scenario proposes; the engine then checks that run against the
    /// original failure's mechanism ([`crate::mechanism`]) and keeps the candidate only
    /// when the check preserves it.
    fn run(&mut self, config: &Self::Config) -> Ran;

    /// Whether the run [`Self::run`] proposed for `config` was chosen after every run of
    /// `config` was tried: a mechanism rejection of it then decides the candidate. The
    /// default, `false`, makes such a rejection undecided, which withholds minimality.
    fn search_exhausted(&self, _config: &Self::Config) -> bool {
        false
    }
}

/// What the reduction concluded of one candidate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigVerdict {
    /// The failure reproduced: the candidate was kept.
    Fails,
    /// A decided non-failure.
    Holds,
    /// No run of the program.
    NotARun,
    /// Undecided (INV-008).
    Inconclusive(InconclusiveReason),
    /// Outside the pass's declared scope: not run, not kept.
    OutOfScope,
    /// Not smaller along its dimension, or larger along another: the scenario broke its
    /// contract, and the reduction stopped.
    NotSmaller,
    /// The failure reproduces, but the validator rejects the run: the right verdict
    /// through the wrong mechanism (bn-5kmuf). Not kept. `decided` when the scenario tried
    /// every run of the candidate ([`Scenario::search_exhausted`]).
    MechanismLost {
        /// Why the validator rejects it.
        why: LostKind,
        /// Whether the rejection decides the candidate.
        decided: bool,
    },
}

/// A [`Rejection`] without its rendering, for the transcript's verdict; the rendering is
/// the entry's reason.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LostKind {
    /// The validator's replay says the run is no run of the program.
    NotARun,
    /// The run does not reproduce the failure on the validator's replay.
    FailureLost,
    /// A different mechanism.
    Mechanism(mechanism::Mismatch),
}

impl LostKind {
    fn of(r: &Rejection) -> Self {
        match r {
            Rejection::NotARun(_) => Self::NotARun,
            Rejection::FailureLost(_) => Self::FailureLost,
            Rejection::Mechanism(m) => Self::Mechanism(*m),
        }
    }
}

impl ConfigVerdict {
    /// Whether the verdict decides that the candidate does not reproduce the failure
    /// within the scope: what a minimality claim rests on.
    #[must_use]
    pub const fn decided_non_failure(self) -> bool {
        matches!(
            self,
            Self::Holds
                | Self::NotARun
                | Self::OutOfScope
                | Self::MechanismLost { decided: true, .. }
        )
    }
}

/// One entry of the scenario transcript.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigAttempt {
    /// Its position among every attempt, from zero.
    pub index: u64,
    /// The pass; `None` for the start.
    pub dimension: Option<Dimension>,
    /// The round, from zero.
    pub round: u64,
    /// The version of the configuration it was tried against: `0` for the start, one
    /// more after each kept candidate.
    pub version: u64,
    /// The scenario's label, cut to at most [`REASON_CAP`] bytes.
    pub label: String,
    /// The label's full length in bytes.
    pub label_bytes: usize,
    /// The candidate's measure, owner, fault, value.
    pub measure: [u64; 3],
    /// The verdict.
    pub verdict: ConfigVerdict,
    /// The scenario's reason, cut to at most [`REASON_CAP`] bytes; empty for a failure
    /// and a memo hit.
    pub reason: String,
    /// The reason's full length in bytes.
    pub reason_bytes: usize,
    /// For a candidate judged from the memo, the attempt that ran it.
    pub memo_of: Option<u64>,
}

/// The scenario transcript, bounded like [`crate::reduce::Attempts`].
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ConfigAttempts {
    /// The recorded attempts, in order.
    pub entries: Vec<ConfigAttempt>,
    /// Attempts made after the bound ran out: counted, not recorded.
    pub omitted: u64,
}

/// A property of the reduced configuration that the reduction checked.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ScenarioGuarantee {
    /// The configuration itself was run and the failure reproduced. This is not RFC
    /// 0028's `ReplayPreserving`, which a pack claims of a causal core with a replay and
    /// which implies `CausallyClosed` (C2): it says only that the configuration reruns
    /// to the failure.
    Reproduces,
    /// RFC 0028's `OwnerMinimal`, over the scenario's owner candidates: no in-scope
    /// candidate of the owner pass still fails through the original failure's mechanism
    /// (bn-5kmuf). A candidate that fails only through another mechanism
    /// ([`ConfigVerdict::MechanismLost`], decided) does not refute it: the class is
    /// minimality among mechanism-preserving configurations. What an owner candidate removes (a task,
    /// an incarnation, a node) is the scenario's to say, and its evidence must say it.
    OwnerMinimal,
    /// RFC 0028's `FaultMinimal`, over the scenario's fault candidates: no in-scope
    /// candidate of the fault pass (a fault dropped or placed earlier) still fails
    /// through the original failure's mechanism.
    FaultMinimal,
    /// RFC 0028's `ValueMinimal`, over the scenario's value candidates: no in-scope
    /// one-step reduction of the value domain still fails through the original failure's
    /// mechanism, renamed.
    ValueMinimal,
}

/// How one pass ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DimensionEnd {
    /// It ran to its fixpoint.
    Done,
    /// The scenario declared that it cannot reduce this dimension without changing the
    /// checked property, or its declaration does not name [`Preserved::Property`]. The
    /// pass ran nothing.
    ScopeRefused(String),
    /// The budget ran out inside the pass.
    Exhausted(Exhausted),
    /// A candidate was not smaller: the scenario broke its contract.
    Contract,
}

/// One pass's record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DimensionRecord {
    /// The round, from zero.
    pub round: u64,
    /// The pass.
    pub dimension: Dimension,
    /// The measure of its input.
    pub before: [u64; 3],
    /// The measure of its result.
    pub after: [u64; 3],
    /// Candidates generated.
    pub offered: u64,
    /// Candidates run.
    pub runs: u64,
    /// Candidates kept.
    pub kept: u64,
    /// Candidates that held.
    pub held: u64,
    /// Candidates that were no run.
    pub not_a_run: u64,
    /// Candidates outside the scope.
    pub out_of_scope: u64,
    /// Candidates the scenario could not decide, or whose mechanism check could not.
    pub inconclusive: u64,
    /// Candidates judged from the memo.
    pub memo: u64,
    /// Candidates whose failure reproduced through another mechanism: never kept.
    pub mechanism_lost: u64,
    /// How it ended.
    pub end: DimensionEnd,
}

/// Why a reduction did not start.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScenarioRefusal {
    /// The input configuration runs and does not fail.
    InputHolds(String),
    /// The input configuration is no run of the program.
    InputNotARun(String),
    /// The input configuration's run is inconclusive.
    InputInconclusive(InconclusiveReason, String),
    /// The validator rejects the input itself: its failing run does not show the
    /// mechanism derived from the original failure ([`crate::mechanism`]).
    MechanismNotInInput(Rejection),
}

/// A scenario reduction's result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScenarioReduction<C> {
    /// The reduction reached a round that kept nothing, and the reduced configuration's
    /// failing run preserves the original failure's mechanism ([`crate::mechanism`]):
    /// the only way a reduction returns a configuration.
    Reduced {
        /// The reduced configuration: the last version, validated.
        config: Validated<C>,
        /// Every version, from the input: each one ran and failed.
        versions: Vec<C>,
        /// What was checked of it, ascending.
        guarantees: Vec<ScenarioGuarantee>,
        /// Each pass, in order.
        passes: Vec<DimensionRecord>,
        /// Each attempt, in order.
        attempts: ConfigAttempts,
        /// What the passes spent. Each check's spending is in `checks`.
        spent: Spent,
        /// Each mechanism check, in order: the input, then each candidate whose run
        /// failed, kept or not.
        checks: Vec<CheckEntry>,
    },
    /// The reduction reached a round that kept nothing, but the validator rejects the
    /// reduced configuration's run: the right verdict through the wrong mechanism. Not a
    /// success.
    Rejected {
        /// The reduced configuration, which is not validated.
        config: C,
        /// Why the validator rejects it.
        why: Rejection,
        /// Every version, from the input.
        versions: Vec<C>,
        /// Each pass, in order.
        passes: Vec<DimensionRecord>,
        /// Each attempt, in order.
        attempts: ConfigAttempts,
        /// What the passes spent.
        spent: Spent,
        /// Each mechanism check, in order.
        checks: Vec<CheckEntry>,
    },
    /// The reduction stopped early, or the validator could not decide its result
    /// (INV-008). Not a success, and no minimality is claimed.
    Inconclusive {
        /// [`InconclusiveReason::ResourceExhausted`] when the budget or the validation
        /// allowance ran out; [`InconclusiveReason::EngineError`] when the scenario broke
        /// its contract; the validator's reason when it could not decide.
        reason: InconclusiveReason,
        /// The last configuration that ran and failed, if the input did, with its
        /// mechanism check.
        best: Option<Checked<C>>,
        /// Every version kept before the stop.
        versions: Vec<C>,
        /// Each pass, in order.
        passes: Vec<DimensionRecord>,
        /// Each attempt, in order.
        attempts: ConfigAttempts,
        /// What the passes spent.
        spent: Spent,
        /// Each mechanism check, in order.
        checks: Vec<CheckEntry>,
    },
    /// The reduction did not start: the input does not fail, or does not show the
    /// mechanism. The start's run is in the
    /// transcript and its cost in `spent`.
    Refused {
        /// Why.
        why: ScenarioRefusal,
        /// The start's attempt.
        attempts: ConfigAttempts,
        /// What it spent.
        spent: Spent,
    },
}

impl<C> ScenarioReduction<C> {
    /// The reduced, validated configuration, if the reduction finished with one.
    #[must_use]
    pub const fn config(&self) -> Option<&Validated<C>> {
        match self {
            Self::Reduced { config, .. } => Some(config),
            _ => None,
        }
    }

    /// The transcript.
    #[must_use]
    pub const fn attempts(&self) -> Option<&ConfigAttempts> {
        match self {
            Self::Reduced { attempts, .. }
            | Self::Rejected { attempts, .. }
            | Self::Inconclusive { attempts, .. }
            | Self::Refused { attempts, .. } => Some(attempts),
        }
    }

    /// The mechanism checks, unless the reduction was refused.
    #[must_use]
    pub fn checks(&self) -> Option<&[CheckEntry]> {
        match self {
            Self::Reduced { checks, .. }
            | Self::Rejected { checks, .. }
            | Self::Inconclusive { checks, .. } => Some(checks),
            Self::Refused { .. } => None,
        }
    }
}

/// The running state.
struct State<'a, S: Scenario + Validator<<S as Scenario>::Config>> {
    scenario: &'a mut S,
    /// The original failure's mechanism, derived once.
    mechanism: Result<Mechanism<<S as Validator<S::Config>>::Label>, Undecided>,
    /// The validation allowance left.
    allowance: Allowance,
    /// Each check, in order.
    checks: Vec<CheckEntry>,
    /// Each configuration checked, and its outcome: none is checked twice.
    checked: BTreeMap<S::Config, Outcome>,
    budget: Budget,
    spent: Spent,
    attempts: ConfigAttempts,
    tried: u64,
    room: TranscriptBound,
    full: bool,
    versions: Vec<S::Config>,
    /// Each version's measure, as the engine observed it when it kept the version: the
    /// `from` of every later candidate, so a scenario whose measure changes after a run
    /// cannot raise it.
    measures: Vec<[u64; 3]>,
    passes: Vec<DimensionRecord>,
    /// Each configuration run, with the attempt that ran it and its verdict.
    memo: BTreeMap<S::Config, (u64, ConfigVerdict)>,
}

/// Why the reduction stopped early.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Stop {
    Exhausted(Exhausted),
    Contract,
}

/// `text` cut to at most [`REASON_CAP`] bytes at a character boundary.
fn cut(text: &str) -> &str {
    let mut at = text.len().min(REASON_CAP);
    while !text.is_char_boundary(at) {
        at -= 1;
    }
    &text[..at]
}

/// What running a configuration says of it.
const fn verdict_of(ran: &Ran) -> ConfigVerdict {
    match ran {
        Ran::Fails => ConfigVerdict::Fails,
        Ran::Holds(_) => ConfigVerdict::Holds,
        Ran::NotARun(_) => ConfigVerdict::NotARun,
        Ran::Inconclusive(reason, _) => ConfigVerdict::Inconclusive(*reason),
    }
}

fn reason_of(ran: &Ran) -> &str {
    match ran {
        Ran::Fails => "",
        Ran::Holds(s) | Ran::NotARun(s) | Ran::Inconclusive(_, s) => s,
    }
}

/// Whether `m` is strictly smaller than `from` along `d` and no larger along the others.
fn smaller(m: [u64; 3], from: [u64; 3], d: Dimension) -> bool {
    let i = d.index();
    m[i] < from[i] && (0..3).all(|k| k == i || m[k] <= from[k])
}

/// A pass's running counts.
#[derive(Default)]
struct Counts {
    offered: u64,
    runs: u64,
    kept: u64,
    held: u64,
    not_a_run: u64,
    out_of_scope: u64,
    inconclusive: u64,
    memo: u64,
    mechanism_lost: u64,
}

/// What one transcript entry says, before the bound admits it.
struct Entry<'a> {
    label: &'a str,
    measure: [u64; 3],
    verdict: ConfigVerdict,
    reason: &'a str,
    memo_of: Option<u64>,
}

/// Where an attempt stands.
#[derive(Clone, Copy)]
struct At {
    dimension: Option<Dimension>,
    round: u64,
}

impl<S: Scenario + Validator<<S as Scenario>::Config>> State<'_, S> {
    /// Check `config` with the validator, as `stage`, and record the check. A
    /// configuration checked before keeps that outcome and is not checked again.
    fn check(&mut self, config: &S::Config, stage: Stage) -> Outcome {
        if let Some(outcome) = self.checked.get(config) {
            return outcome.clone();
        }
        let outcome = mechanism::check(
            &mut *self.scenario,
            &self.mechanism,
            config,
            &mut self.allowance,
        );
        self.checks.push(CheckEntry {
            stage,
            outcome: outcome.clone(),
        });
        self.checked.insert(config.clone(), outcome.clone());
        outcome
    }

    /// [`Self::check`] of the last version.
    fn check_last(&mut self, stage: Stage) -> Outcome {
        let config = self.versions.last().expect("a version").clone();
        self.check(&config, stage)
    }

    fn version(&self) -> u64 {
        u64::try_from(self.versions.len().saturating_sub(1)).unwrap_or(u64::MAX)
    }

    /// Record an attempt, if the bound has room for it; checked before it is built.
    fn note(&mut self, index: u64, at: At, e: Entry<'_>) {
        let Entry {
            label,
            measure,
            verdict,
            reason,
            memo_of,
        } = e;
        let (label_cut, reason_cut) = (cut(label), cut(reason));
        let units = 1_u64
            .saturating_add(u64::try_from(label_cut.len()).unwrap_or(u64::MAX))
            .saturating_add(u64::try_from(reason_cut.len()).unwrap_or(u64::MAX));
        if self.full || self.room.entries == 0 || self.room.units < units {
            self.full = true;
            self.attempts.omitted = self.attempts.omitted.saturating_add(1);
            return;
        }
        self.room.entries -= 1;
        self.room.units -= units;
        let version = self.version();
        self.attempts.entries.push(ConfigAttempt {
            index,
            dimension: at.dimension,
            round: at.round,
            version,
            label: label_cut.to_owned(),
            label_bytes: label.len(),
            measure,
            verdict,
            reason: reason_cut.to_owned(),
            reason_bytes: reason.len(),
            memo_of,
        });
    }

    fn next_index(&mut self) -> u64 {
        let i = self.tried;
        self.tried = self.tried.saturating_add(1);
        i
    }

    /// Run `config`, charged first: one replay and its predicted cost. The attempt is
    /// recorded; an attempt the budget cannot pay for is counted as omitted.
    fn run(
        &mut self,
        config: &S::Config,
        at: At,
        label: &str,
        measure: [u64; 3],
    ) -> Result<Ran, Exhausted> {
        // The run's predicted cost, and the memo insert that follows it.
        let cost = self
            .scenario
            .run_cost(config)
            .saturating_add(self.memo_cost());
        let charged = self
            .budget
            .charge_replay(&mut self.spent)
            .and_then(|()| self.budget.charge_work(cost, &mut self.spent));
        let index = self.next_index();
        if let Err(e) = charged {
            self.attempts.omitted = self.attempts.omitted.saturating_add(1);
            return Err(e);
        }
        let ran = self.scenario.run(config);
        let mut verdict = verdict_of(&ran);
        let mut rendering = reason_of(&ran).to_owned();
        // A candidate's failing run is kept only when it preserves the original
        // failure's mechanism (bn-5kmuf). The start is checked by the caller.
        if let (Ran::Fails, Some(dimension)) = (&ran, at.dimension) {
            let stage = Stage::Dimension {
                round: at.round,
                dimension,
            };
            match self.check(config, stage) {
                Outcome::Preserved(_) => {}
                Outcome::Rejected(why, _) => {
                    verdict = ConfigVerdict::MechanismLost {
                        why: LostKind::of(&why),
                        decided: self.scenario.search_exhausted(config),
                    };
                    rendering =
                        format!("the failure reproduces through another mechanism: {why:?}");
                }
                Outcome::Undecided(InconclusiveReason::ResourceExhausted, ..) => {
                    // The check could not be paid for: the reduction stops, as when the
                    // budget runs out, rather than spend runs it can never keep.
                    self.attempts.omitted = self.attempts.omitted.saturating_add(1);
                    return Err(Exhausted::Work);
                }
                Outcome::Undecided(reason, detail, _) => {
                    verdict = ConfigVerdict::Inconclusive(reason);
                    rendering = format!("the mechanism check could not decide: {detail}");
                }
            }
        }
        self.note(
            index,
            at,
            Entry {
                label,
                measure,
                verdict,
                reason: &rendering,
                memo_of: None,
            },
        );
        self.memo.insert(config.clone(), (index, verdict));
        // The scenario's own answer, unless the check changed it: the start's refusal
        // keeps its kind (holds, no run, inconclusive).
        Ok(match verdict {
            ConfigVerdict::MechanismLost { .. } => Ran::Holds(rendering),
            ConfigVerdict::Inconclusive(reason) if matches!(ran, Ran::Fails) => {
                Ran::Inconclusive(reason, rendering)
            }
            _ => ran,
        })
    }

    /// The work one memo lookup or insert costs: its comparisons.
    fn memo_cost(&self) -> u64 {
        let n = u64::try_from(self.memo.len()).unwrap_or(u64::MAX);
        u64::from(u64::BITS - n.leading_zeros()).saturating_add(1)
    }

    fn record(
        &mut self,
        round: u64,
        d: Dimension,
        before: [u64; 3],
        c: &Counts,
        end: DimensionEnd,
    ) {
        let after = *self.measures.last().expect("the input is a version");
        self.passes.push(DimensionRecord {
            round,
            dimension: d,
            before,
            after,
            offered: c.offered,
            runs: c.runs,
            kept: c.kept,
            held: c.held,
            not_a_run: c.not_a_run,
            out_of_scope: c.out_of_scope,
            inconclusive: c.inconclusive,
            memo: c.memo,
            mechanism_lost: c.mechanism_lost,
            end,
        });
    }

    /// One pass over `d` in `round`, from the last version to its fixpoint. Returns
    /// whether it kept a candidate and whether its last generation of candidates was
    /// all decided.
    fn pass(&mut self, round: u64, d: Dimension) -> Result<(bool, bool), Stop> {
        let at = At {
            dimension: Some(d),
            round,
        };
        let before = *self.measures.last().expect("a version");
        let mut c = Counts::default();
        let mut kept_any = false;
        loop {
            let from = *self.measures.last().expect("a version");
            // The copy of the current version is part of the generation's predicted cost.
            let current = self.versions.last().expect("a version").clone();
            let cost = self.scenario.candidates_cost(&current);
            if let Err(e) = self.budget.charge_work(cost, &mut self.spent) {
                self.record(round, d, before, &c, DimensionEnd::Exhausted(e));
                return Err(Stop::Exhausted(e));
            }
            let candidates = self.scenario.candidates(&current, d);
            let mut kept = None;
            let mut decided = true;
            for cand in candidates {
                c.offered += 1;
                // At least one unit per candidate, whatever the scenario predicted: a
                // scenario that predicts nothing still pays for each candidate it offers.
                if let Err(e) = self.budget.charge_work(1, &mut self.spent) {
                    self.record(round, d, before, &c, DimensionEnd::Exhausted(e));
                    return Err(Stop::Exhausted(e));
                }
                let m = self.scenario.measure(&cand.config);
                if !smaller(m, from, d) {
                    let index = self.next_index();
                    self.note(
                        index,
                        at,
                        Entry {
                            label: &cand.label,
                            measure: m,
                            verdict: ConfigVerdict::NotSmaller,
                            reason: "",
                            memo_of: None,
                        },
                    );
                    self.record(round, d, before, &c, DimensionEnd::Contract);
                    return Err(Stop::Contract);
                }
                if let Err(why) = self.scenario.admits(d, &cand.config) {
                    let index = self.next_index();
                    self.note(
                        index,
                        at,
                        Entry {
                            label: &cand.label,
                            measure: m,
                            verdict: ConfigVerdict::OutOfScope,
                            reason: &why,
                            memo_of: None,
                        },
                    );
                    c.out_of_scope += 1;
                    continue;
                }
                let lookup = self.memo_cost();
                if let Err(e) = self.budget.charge_work(lookup, &mut self.spent) {
                    self.record(round, d, before, &c, DimensionEnd::Exhausted(e));
                    return Err(Stop::Exhausted(e));
                }
                let verdict = if let Some(&(of, verdict)) = self.memo.get(&cand.config) {
                    let index = self.next_index();
                    if verdict == ConfigVerdict::Fails {
                        // Every configuration that failed was kept, and every kept version is
                        // strictly smaller than the last: a smaller candidate cannot equal
                        // one. A scenario whose configurations compare equal across
                        // measures broke its contract.
                        self.note(
                            index,
                            at,
                            Entry {
                                label: &cand.label,
                                measure: m,
                                verdict: ConfigVerdict::NotSmaller,
                                reason: "equal to a configuration already kept",
                                memo_of: Some(of),
                            },
                        );
                        self.record(round, d, before, &c, DimensionEnd::Contract);
                        return Err(Stop::Contract);
                    }
                    self.note(
                        index,
                        at,
                        Entry {
                            label: &cand.label,
                            measure: m,
                            verdict,
                            reason: "",
                            memo_of: Some(of),
                        },
                    );
                    c.memo += 1;
                    verdict
                } else {
                    match self.run(&cand.config, at, &cand.label, m) {
                        Ok(_) => {
                            c.runs += 1;
                            self.memo
                                .get(&cand.config)
                                .map_or(ConfigVerdict::Holds, |e| e.1)
                        }
                        Err(e) => {
                            self.record(round, d, before, &c, DimensionEnd::Exhausted(e));
                            return Err(Stop::Exhausted(e));
                        }
                    }
                };
                match verdict {
                    ConfigVerdict::Fails => {
                        kept = Some((cand.config, m));
                        break;
                    }
                    ConfigVerdict::Holds => c.held += 1,
                    ConfigVerdict::NotARun => c.not_a_run += 1,
                    ConfigVerdict::Inconclusive(_) => {
                        c.inconclusive += 1;
                        decided = false;
                    }
                    ConfigVerdict::MechanismLost { decided: d, .. } => {
                        c.mechanism_lost += 1;
                        decided &= d;
                    }
                    // Neither is ever memoized: an out-of-scope candidate is not run, and a
                    // candidate that is not smaller stops the reduction before its run.
                    ConfigVerdict::OutOfScope | ConfigVerdict::NotSmaller => c.out_of_scope += 1,
                }
            }
            match kept {
                Some((config, m)) => {
                    c.kept += 1;
                    kept_any = true;
                    self.versions.push(config);
                    self.measures.push(m);
                }
                None => {
                    self.record(round, d, before, &c, DimensionEnd::Done);
                    return Ok((kept_any, decided));
                }
            }
        }
    }
}

/// Reduce the failing configuration `start` along the owner, fault and value
/// dimensions of `scenario`, within `budget`. See the module documentation. The scenario
/// is its own [`Validator`]: the input's failing run and the result of every pass that
/// kept a candidate are checked against the original failure's mechanism.
pub fn reduce_scenario<S: Scenario + Validator<<S as Scenario>::Config>>(
    scenario: &mut S,
    start: S::Config,
    budget: Budget,
) -> ScenarioReduction<S::Config> {
    let mechanism = scenario.mechanism();
    let mut st = State {
        scenario,
        mechanism,
        allowance: budget.validation(),
        checks: Vec::new(),
        checked: BTreeMap::new(),
        budget,
        spent: Spent::default(),
        attempts: ConfigAttempts::default(),
        tried: 0,
        room: budget.transcript_bound(),
        full: false,
        versions: vec![start.clone()],
        measures: Vec::new(),
        passes: Vec::new(),
        memo: BTreeMap::new(),
    };
    let at = At {
        dimension: None,
        round: 0,
    };
    let m = st.scenario.measure(&start);
    st.measures.push(m);
    let why = match st.run(&start, at, "start", m) {
        Err(e) => return stopped(st, Stop::Exhausted(e), false),
        Ok(Ran::Fails) => None,
        Ok(Ran::Holds(why)) => Some(ScenarioRefusal::InputHolds(why)),
        Ok(Ran::NotARun(why)) => Some(ScenarioRefusal::InputNotARun(why)),
        Ok(Ran::Inconclusive(reason, why)) => Some(ScenarioRefusal::InputInconclusive(reason, why)),
    };
    if let Some(why) = why {
        return ScenarioReduction::Refused {
            why,
            attempts: st.attempts,
            spent: st.spent,
        };
    }
    // The input's failing run must show the mechanism: otherwise no result can be
    // checked against it. A check that cannot decide stops the reduction (INV-008).
    match st.check_last(Stage::Input) {
        Outcome::Preserved(_) => {}
        Outcome::Rejected(why, _) => {
            return ScenarioReduction::Refused {
                why: ScenarioRefusal::MechanismNotInInput(why),
                attempts: st.attempts,
                spent: st.spent,
            };
        }
        Outcome::Undecided(reason, ..) => {
            return ScenarioReduction::Inconclusive {
                reason,
                best: None,
                versions: Vec::new(),
                passes: st.passes,
                attempts: st.attempts,
                spent: st.spent,
                checks: st.checks,
            };
        }
    }
    let scopes: Vec<Result<(), String>> = Dimension::ALL
        .iter()
        .map(|&d| match st.scenario.declare(d) {
            Ok(p) if p.contains(&Preserved::Property) => Ok(()),
            Ok(_) => Err("the declared scope does not name the checked property".to_owned()),
            Err(why) => Err(why),
        })
        .collect();
    let mut round = 0_u64;
    loop {
        let mut kept_in_round = false;
        let mut decided = [false; 3];
        for d in Dimension::ALL {
            if let Err(why) = &scopes[d.index()] {
                let before = *st.measures.last().expect("a version");
                st.record(
                    round,
                    d,
                    before,
                    &Counts::default(),
                    DimensionEnd::ScopeRefused(why.clone()),
                );
                continue;
            }
            match st.pass(round, d) {
                Ok((kept, all_decided)) => {
                    kept_in_round |= kept;
                    decided[d.index()] = all_decided;
                    if kept {
                        // The pass's result, checked.
                        st.check_last(Stage::Dimension {
                            round,
                            dimension: d,
                        });
                    }
                }
                Err(stop) => return stopped(st, stop, true),
            }
        }
        if !kept_in_round {
            let complete = st.attempts.omitted == 0;
            let mut guarantees = vec![ScenarioGuarantee::Reproduces];
            for d in Dimension::ALL {
                if complete && decided[d.index()] {
                    guarantees.push(d.minimality());
                }
            }
            guarantees.sort_unstable();
            // The last version was checked when its pass kept it, or as the input, so
            // this reuses that check.
            let outcome = st.check_last(Stage::Result);
            let config = st.versions.last().expect("a version").clone();
            return match outcome.attach(config) {
                Checked::Preserved(config) => ScenarioReduction::Reduced {
                    config,
                    versions: st.versions,
                    guarantees,
                    passes: st.passes,
                    attempts: st.attempts,
                    spent: st.spent,
                    checks: st.checks,
                },
                Checked::Rejected {
                    subject: config,
                    why,
                    ..
                } => ScenarioReduction::Rejected {
                    config,
                    why,
                    versions: st.versions,
                    passes: st.passes,
                    attempts: st.attempts,
                    spent: st.spent,
                    checks: st.checks,
                },
                Checked::Undecided {
                    subject,
                    reason,
                    detail,
                    spent,
                } => ScenarioReduction::Inconclusive {
                    reason,
                    best: Some(Checked::Undecided {
                        subject,
                        reason,
                        detail,
                        spent,
                    }),
                    versions: st.versions,
                    passes: st.passes,
                    attempts: st.attempts,
                    spent: st.spent,
                    checks: st.checks,
                },
            };
        }
        round = round.saturating_add(1);
    }
}

fn stopped<S: Scenario + Validator<<S as Scenario>::Config>>(
    mut st: State<'_, S>,
    stop: Stop,
    started: bool,
) -> ScenarioReduction<S::Config> {
    let best = started.then(|| {
        let outcome = st.check_last(Stage::Best);
        outcome.attach(st.versions.last().expect("a version").clone())
    });
    ScenarioReduction::Inconclusive {
        reason: match stop {
            Stop::Exhausted(_) => InconclusiveReason::ResourceExhausted,
            Stop::Contract => InconclusiveReason::EngineError,
        },
        best,
        versions: if started { st.versions } else { Vec::new() },
        passes: st.passes,
        attempts: st.attempts,
        spent: st.spent,
        checks: st.checks,
    }
}
