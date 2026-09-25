//! Semantic neighborhood generation around a causal core (START_HERE PR 21, RFC 0032
//! gate 5, bn-4ykgg).
//!
//! # What a neighbor is
//!
//! A repair that fixes only the exact failing run is overfit. Gate 5 runs the candidate
//! on a **neighborhood**: runs near the failure's causal core that differ from it along a
//! semantic dimension. A neighbor is never an edit of trace bytes. It is one of:
//!
//! - a new **schedule** of the core's events ([`Edit::Schedule`]): a permutation that
//!   the core's causal order admits;
//! - a **message fault** on one of the core's sends ([`Edit::Message`]): its
//!   duplication or loss;
//! - a **scenario edit** the substrate states in its own terms ([`Edit::Scenario`]):
//!   where a fault or a cancellation falls, which value or name each party carries.
//!
//! Every neighbor carries a [`Profile`]: the fault events it injects by
//! [`FaultClass`], its cancellations, the largest value index it names and the nodes it
//! uses. That profile is what the Intent Contract types it against ([`Envelope`]).
//!
//! # The strategies
//!
//! [`Strategy`] is RFC 0032's closed set of eight, one for one with plan §8.3, spelled as
//! `promotion-receipt.schema.json`'s `neighborhood_strategy` enum. The four semantic
//! dimensions map to them as follows:
//!
//! | dimension | strategy | generator |
//! |---|---|---|
//! | interleaving, inside the trace class | `schedule-perturbation` | generic: adjacent swaps, then seeded walks of independent swaps |
//! | interleaving, across a causal decision | `alternate-enabled-events` | generic: hoist a dependent event of another actor before a decision |
//! | timing | `message-duplication-loss-delay` | generic: delay a send (a reordering typed as a `delay` fault; a delay by one can be the same run as an adjacent swap), duplicate it, lose it |
//! | fault placement | `fault-window` | substrate: insert, remove or move a fault |
//! | fault placement | `cancellation-checkpoints` | substrate: move a cancellation to an adjacent checkpoint |
//! | value domain | `value-name-permutation` | substrate: symmetric value and name permutations, and value-domain steps |
//! | — | `abstraction-map-variants` | substrate, when it has an abstraction map |
//! | — | `hidden-corpus-mutations` | substrate, when it has a held-out corpus |
//!
//! The generic generators read only the core's [`CausalCore`]: its hard order (edges no
//! schedule can reverse), its dependence relation, its core membership and each event's
//! [`EventClass`]. They are property-directed: every generated neighbor moves at least
//! one event of the core.
//!
//! # Validity: typed against the intent, realizable by the program
//!
//! A generated candidate becomes a neighbor only when it is valid. The checks run in a
//! fixed order, and the first failure is the candidate's typed [`Rejection`]:
//!
//! 1. a schedule is a permutation of the core's events ([`Rejection::MalformedSchedule`]);
//! 2. it respects every hard edge ([`Rejection::ViolatesCausalOrder`]);
//! 3. a `schedule-perturbation` schedule stays in the core's trace class: it reverses no
//!    dependent pair ([`Rejection::LeavesTraceClass`]);
//! 4. a message fault names a send of the core ([`Rejection::NotAMessage`]);
//! 5. its profile is well typed against the [`Envelope`]: every fault class it injects
//!    is enabled in the contract's fault model, every count is within its cap and within
//!    the contract's `faults` bound, every value is inside `values` and every node inside
//!    `nodes`; an undeclared bound that the profile needs refuses the candidate (fail
//!    closed);
//! 6. it is not a duplicate: a schedule equal to the core's order or to an earlier
//!    schedule of its strategy, or a non-schedule edit whose content address (a digest
//!    of its canonical bytes and profile) an earlier edit of its strategy had. Coverage
//!    counts distinct content. A substrate identity is bound to the content it first
//!    came with: the same identity with other content is an
//!    [`Rejection::IdentityCollision`], never a duplicate, and makes the campaign an
//!    engine error.
//!
//! A realized run is executed as the neighbor only when its [`Witness`] passes the
//! library's causal-neighbor check: the definition of "this run is that neighbor" is
//! the library's, stated over the core's own dependence relation, never a substrate's.
//!
//! A rejected candidate is counted and never realized or executed. A valid one is then
//! handed to the substrate, which must realize it as a run the program and its binding
//! can produce ([`Realized`]): a candidate the program cannot produce is
//! `NotRealizable`, and one the substrate has no semantics for (a network fault on a
//! substrate with no network) is `Unsupported`, INV-008's unsupported semantics. Only a
//! realized run is executed.
//!
//! # Budget, charged before work (INV-008)
//!
//! [`Budget`] bounds candidates, runs and work units. A work unit is one constant-time
//! step up to a logarithmic factor (a dependence query, a set lookup, a byte of a
//! substrate-supplied string). Each candidate is charged before it is built, at its
//! predicted cost (its construction plus its validation walks); each generator step is
//! charged before it is examined; each run is charged before the substrate realizes it.
//!
//! A charge that fails suspends the campaign, RFC 0032's `BudgetExhausted` with a
//! continuation (never a smaller campaign): [`explore`] returns
//! [`Exploration::Suspended`], whose record is [`Verdict::Pending`] with each truncated
//! strategy's frontier disclosed (silent caps are prohibited), and whose
//! [`Continuation`] serializes the committed state. [`resume`] continues it under a
//! larger cumulative budget, re-running no committed neighbor, and a campaign resumed to
//! its end has the same [`NeighborhoodRecord::result_bytes`] as an uninterrupted one. A
//! budget that cannot pay to check the core is the terminal [`CoreError::CoreCheckExhausted`].
//!
//! # Determinism (INV-005, INV-006)
//!
//! Nothing here reads a clock, a random source, an address or a hash map's order. The
//! only randomness is [`Config::seed`], recorded in the output, from which each
//! strategy's walks draw through SplitMix64. The same core, substrate answers, envelope
//! and configuration give the same record, byte for byte ([`NeighborhoodRecord::canonical_bytes`]).
//!
//! # Disclosure
//!
//! # Trust: what makes a record gate-5 evidence
//!
//! A record is never evidence on its own say-so.
//!
//! - It has private fields and no public constructor: it exists only as the output of
//!   [`explore`] or [`resume`] in this process, so no caller can edit a verdict or drop
//!   a failing neighbor.
//! - It is bound to its [`Inputs`]: the generator, the `rt_*` transaction, the candidate
//!   (the substrate's subject identity), the engine identity with its epochs, the core
//!   (identity, a commitment to its whole relation, its profile), the envelope and the
//!   configuration. Each neighbor carries a content commitment to its edit and profile.
//!   [`NeighborhoodRecord::receipt_coverage`] derives the current inputs itself from
//!   the live substrate, envelope and configuration ([`Inputs::of`]); no caller can
//!   hand it a binding. A stale record is refused.
//! - A selected strategy that produced no candidate is a coverage gap, and the
//!   projection's `explored` counts distinct runs, so neighbors a program realizes as
//!   one run are one exploration.
//! - The engine identity is the substrate's word: it must change whenever what
//!   realizes or judges a neighbor changes (the register derives it from a digest of
//!   its own source). A substrate that changes its semantics under one identity is
//!   outside this contract.
//! - Every count reconciles with the dispositions, and the verdict with both, checked
//!   when the record is made and again when it is projected.
//! - Every value the substrate supplies enters once, through one admission path:
//!   the core is read once into a snapshot (charged, checked, and the only core used
//!   afterwards); its profile and the three identity strings are read once; every other
//!   string or byte vector (scenario identities and edit bytes, not-run reasons,
//!   not-realizable and unsupported reasons, witness echoes, refuted properties, run
//!   handles) goes through `admit`, which reads the length, refuses a value past its
//!   cap without copying it (a typed marker, or a typed refusal for an identity), and
//!   charges the byte length times the copies kept before keeping it.
//! - Within a strategy, non-schedule candidates' identities and content addresses are
//!   a bijection: any second binding in either direction is an identity collision (an
//!   engine error), whatever the arrival order, and the record's whole-set check holds
//!   `Complete` to the bijection.
//! - Bytes from outside the process are never trusted for an outcome: a continuation
//!   read back is only checked against a fresh derivation ([`resume_untrusted`]), and
//!   no record is ever decoded into a projectable value. Keeping a campaign across a
//!   restart without re-running it needs the daemon's authenticated `cont_*` store,
//!   which is not here.
//!
//! [`NeighborhoodRecord`] is the typed record: per strategy, what was generated,
//! rejected (by reason), found unsupported or unrealizable, executed, and how each
//! execution came out, plus every neighbor's disposition and every failing neighbor with
//! its run handle. Its canonical bytes are RFC 0037 ID5 canonical JSON. No schema
//! document defines the whole record, so its shape is crate-local
//! (`continuum.repair.neighborhood/v0`). [`NeighborhoodRecord::receipt_coverage`]
//! projects it onto the one shape a schema does define: `promotion-receipt.schema.json`'s
//! `coverage.neighborhood` (per-strategy `explored`, `failing`, `truncated`, `frontier`,
//! and the `failing_neighbors` array the ratified counting rule grades). No adequacy is
//! claimed: RFC 0032 makes none until the docs/50 gaming corpus lands.
//!
//! # What this is not
//!
//! It is not CIR-based. CIR (RFC 0001, PR 17) is not built, so a core is anything that
//! implements [`CausalCore`]; the replicated register's op-level order instantiates it
//! today, and a CIR causal core instantiates it when PR 17 lands. It does not reuse gate
//! results across a patch (RFC 0030 reuse classes): every neighbor is re-run, so no
//! strategy reports a `reused` count.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::sync::Arc;

use continuum_intent::bounds::{Bounds, DeclaredBound, ExplorationBound};
use continuum_intent::canonical_json::Json;
use continuum_intent::faults::{FaultClass, FaultModel};
use continuum_value::assurance::InconclusiveReason;
use continuum_value::identity::{Blake3Hasher, ContentHasher};

/// The Intent Contract vocabulary an [`Envelope`] is built from, re-exported so a
/// substrate can build one without its own edge to `continuum-intent`.
pub mod intent {
    pub use continuum_intent::bounds::{Bounds, DeclaredBound, ExplorationBound};
    pub use continuum_intent::canonical_json::Json;
    pub use continuum_intent::faults::{FaultClass, FaultModel};
}

/// The record's kind token.
pub const RECORD_KIND: &str = "continuum.repair.neighborhood/v0";

/// The largest core the generic generators accept. Several checks are quadratic in the
/// core's length; a larger core is refused before any work ([`CoreError::TooLarge`]).
pub const MAX_CORE_EVENTS: usize = 4_096;

// ---------------------------------------------------------------------------
// strategies
// ---------------------------------------------------------------------------

/// RFC 0032's eight closed neighborhood strategies, in its table's order. The token is
/// also the neighbor's class.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Strategy {
    /// Alternate enabled events at each causal decision.
    AlternateEnabledEvents,
    /// Fault insertion or removal around the repaired window.
    FaultWindow,
    /// Cancellation at adjacent checkpoints.
    CancellationCheckpoints,
    /// Equivalent value and name permutations.
    ValueNamePermutation,
    /// Changed message duplication, loss or delay.
    MessageDuplicationLossDelay,
    /// Schedule perturbations inside the same trace-class boundary.
    SchedulePerturbation,
    /// Generated variants from the abstraction map.
    AbstractionMapVariants,
    /// Hidden corpus-style mutations.
    HiddenCorpusMutations,
}

impl Strategy {
    /// Every strategy, in RFC 0032's order.
    pub const ALL: [Self; 8] = [
        Self::AlternateEnabledEvents,
        Self::FaultWindow,
        Self::CancellationCheckpoints,
        Self::ValueNamePermutation,
        Self::MessageDuplicationLossDelay,
        Self::SchedulePerturbation,
        Self::AbstractionMapVariants,
        Self::HiddenCorpusMutations,
    ];

    /// The wire token (`promotion-receipt.schema.json`, `neighborhood_strategy`).
    #[must_use]
    pub const fn token(self) -> &'static str {
        match self {
            Self::AlternateEnabledEvents => "alternate-enabled-events",
            Self::FaultWindow => "fault-window",
            Self::CancellationCheckpoints => "cancellation-checkpoints",
            Self::ValueNamePermutation => "value-name-permutation",
            Self::MessageDuplicationLossDelay => "message-duplication-loss-delay",
            Self::SchedulePerturbation => "schedule-perturbation",
            Self::AbstractionMapVariants => "abstraction-map-variants",
            Self::HiddenCorpusMutations => "hidden-corpus-mutations",
        }
    }

    /// The strategy a token names; `None` for any other string.
    #[must_use]
    pub fn from_token(token: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|s| s.token() == token)
    }

    /// Its position in [`Self::ALL`].
    #[must_use]
    pub const fn index(self) -> u64 {
        match self {
            Self::AlternateEnabledEvents => 0,
            Self::FaultWindow => 1,
            Self::CancellationCheckpoints => 2,
            Self::ValueNamePermutation => 3,
            Self::MessageDuplicationLossDelay => 4,
            Self::SchedulePerturbation => 5,
            Self::AbstractionMapVariants => 6,
            Self::HiddenCorpusMutations => 7,
        }
    }

    /// Whether the generic generators build its candidates from the core's order; the
    /// others come from the substrate ([`Substrate::scenario`]).
    #[must_use]
    pub const fn is_generic(self) -> bool {
        matches!(
            self,
            Self::AlternateEnabledEvents
                | Self::MessageDuplicationLossDelay
                | Self::SchedulePerturbation
        )
    }
}

// ---------------------------------------------------------------------------
// the core
// ---------------------------------------------------------------------------

/// What one event of a core is, for the generators.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventClass {
    /// An ordinary step.
    Step,
    /// A message send on `channel`: a site for delay, duplication and loss.
    Send {
        /// The channel, in the substrate's numbering.
        channel: u64,
    },
    /// An injected fault of this class.
    Fault(FaultClass),
}

/// A causal core: a failing run's events, numbered `0..len` in the order they were
/// observed, with the relations the generators read.
///
/// - [`Self::hard_predecessors`]: the edges no schedule of the program can reverse
///   (program order, a message before the step that consumes it). Each predecessor is
///   earlier than its event. A substrate may over-approximate them: an extra hard edge
///   only rejects some schedules the program could run (conservative, never unsound),
///   and it must say so. A missing hard edge admits a schedule the program cannot run;
///   the substrate's realization is then the check that catches it.
/// - [`Self::dependent`]: whether two events conflict, answered in constant or
///   logarithmic time (the budget charges one work unit per query) (their footprints share state one
///   of them writes). Two events joined by a hard edge are treated as dependent whatever
///   this returns. Swapping two adjacent independent events keeps the run in its trace
///   class (Mazurkiewicz equivalence); reversing a dependent pair leaves it.
/// - [`Self::in_core`]: whether an event is in the causal core proper (the reduced core
///   of PR 18, or a causal closure of the failure's witnesses); the rest of the run is
///   context. Every generated neighbor moves at least one core event.
///
/// The PR-14 journal and the replicated register instantiate it until CIR (PR 17) lands;
/// a CIR core instantiates it from its events' `causes`.
pub trait CausalCore {
    /// The number of events.
    fn len(&self) -> usize;
    /// Whether there are no events.
    fn is_empty(&self) -> bool {
        self.len() == 0
    }
    /// The event's hard predecessors.
    fn hard_predecessors(&self, event: usize) -> &[usize];
    /// Whether two events conflict.
    fn dependent(&self, a: usize, b: usize) -> bool;
    /// Whether the event is in the causal core proper.
    fn in_core(&self, event: usize) -> bool;
    /// The event's class.
    fn class(&self, event: usize) -> EventClass;
}

/// A [`CausalCore`] held as plain lists: the shape a substrate usually builds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoreOrder {
    hard: Vec<Vec<usize>>,
    dependent: Vec<BTreeSet<usize>>,
    core: Vec<bool>,
    class: Vec<EventClass>,
}

impl CoreOrder {
    /// A core of `hard.len()` events. `conflicts` lists dependent pairs in either
    /// order; `core` the core events; `class` each event's class (missing entries are
    /// [`EventClass::Step`]). Lists are sorted and deduplicated. An entry naming an
    /// event outside `0..hard.len()` is refused, never dropped: a lost conflict would
    /// let the trace-class check pass a reversal. Whether each hard predecessor is
    /// earlier than its event is [`explore`]'s check.
    ///
    /// # Errors
    ///
    /// [`CoreError::EntryOutOfRange`] for a conflict, core member or class entry out of
    /// range, or a conflict of an event with itself.
    pub fn new(
        hard: Vec<Vec<usize>>,
        conflicts: &[(usize, usize)],
        core: &[usize],
        class: &BTreeMap<usize, EventClass>,
    ) -> Result<Self, CoreError> {
        let n = hard.len();
        if n > MAX_CORE_EVENTS {
            return Err(CoreError::TooLarge { len: n });
        }
        let mut dependent = vec![BTreeSet::new(); n];
        for &(a, b) in conflicts {
            if a >= n || b >= n || a == b {
                return Err(CoreError::EntryOutOfRange { index: a.max(b) });
            }
            dependent[a].insert(b);
            dependent[b].insert(a);
        }
        let mut member = vec![false; n];
        for &e in core {
            *member
                .get_mut(e)
                .ok_or(CoreError::EntryOutOfRange { index: e })? = true;
        }
        if let Some((&e, _)) = class.range(n..).next() {
            return Err(CoreError::EntryOutOfRange { index: e });
        }
        let hard = hard
            .into_iter()
            .map(|mut l| {
                l.sort_unstable();
                l.dedup();
                l
            })
            .collect();
        let class = (0..n)
            .map(|e| class.get(&e).copied().unwrap_or(EventClass::Step))
            .collect();
        Ok(Self {
            hard,
            dependent,
            core: member,
            class,
        })
    }
}

impl CausalCore for CoreOrder {
    fn len(&self) -> usize {
        self.hard.len()
    }
    fn hard_predecessors(&self, event: usize) -> &[usize] {
        self.hard.get(event).map_or(&[], Vec::as_slice)
    }
    fn dependent(&self, a: usize, b: usize) -> bool {
        self.dependent.get(a).is_some_and(|s| s.contains(&b))
    }
    fn in_core(&self, event: usize) -> bool {
        self.core.get(event).copied().unwrap_or(false)
    }
    fn class(&self, event: usize) -> EventClass {
        self.class.get(event).copied().unwrap_or(EventClass::Step)
    }
}

/// Why a campaign was refused before any neighbor was generated.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CoreError {
    /// The core has more events than [`MAX_CORE_EVENTS`].
    TooLarge {
        /// Its length.
        len: usize,
    },
    /// The core has no event in the causal core proper, so no neighbor is
    /// property-directed.
    NoCoreEvent,
    /// A hard predecessor is not earlier than its event (or out of range).
    PredecessorNotEarlier {
        /// The event.
        event: usize,
        /// The offending predecessor.
        predecessor: usize,
    },
    /// A budget component does not fit the record's integer range.
    BudgetTooLarge,
    /// The core itself is not well typed against the envelope: every neighbor would
    /// inherit the defect.
    CoreOutsideEnvelope(Rejection),
    /// The budget cannot pay for checking the core.
    CoreCheckExhausted,
    /// [`Config::transaction`] is not an `rt_*` handle.
    NotATransaction,
    /// A substrate identity string (core, subject or engine) is longer than
    /// [`MAX_IDENTITY_BYTES`]: its length.
    IdentityTooLong {
        /// The length.
        len: usize,
    },
    /// A [`CoreOrder`] entry names an event outside the core.
    EntryOutOfRange {
        /// The offending index.
        index: usize,
    },
}

// ---------------------------------------------------------------------------
// the envelope
// ---------------------------------------------------------------------------

/// Per-class caps a scenario declares beside the contract's `bounds` (the scenario's
/// `[faults]` table): `None` is no cap beyond the contract's `faults` bound.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Caps {
    /// Crash faults.
    pub crashes: Option<u32>,
    /// Cancellations, crashes included.
    pub cancellations: Option<u32>,
    /// Partitions.
    pub partitions: Option<u32>,
}

/// What an Intent Contract admits of a neighbor: its fault model, its bounds, and the
/// scenario's per-class caps.
#[derive(Debug, Clone)]
pub struct Envelope {
    model: FaultModel,
    bounds: Bounds,
    caps: Caps,
}

/// Which bound a profile exceeded.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum BoundKind {
    /// [`Caps::crashes`].
    Crashes,
    /// [`Caps::cancellations`].
    Cancellations,
    /// [`Caps::partitions`].
    Partitions,
    /// Recoveries: at most the crashes they complete.
    Recoveries,
    /// The contract's `bounds.faults`: injected fault events of every class but
    /// recovery (a recovery completes a counted crash). A program's own cancellations
    /// are not injected faults; only [`Caps::cancellations`] bounds them, and the same
    /// rule holds whether `faults` is declared or not.
    Faults,
}

impl BoundKind {
    /// A stable token.
    #[must_use]
    pub const fn token(self) -> &'static str {
        match self {
            Self::Crashes => "crashes",
            Self::Cancellations => "cancellations",
            Self::Partitions => "partitions",
            Self::Faults => "faults",
            Self::Recoveries => "recoveries",
        }
    }
}

/// What a neighbor injects and names, for the envelope check.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Profile {
    /// Fault events by class.
    pub faults: BTreeMap<FaultClass, u32>,
    /// Cancellations, crashes included.
    pub cancellations: u32,
    /// The largest value index named, `None` when no value is named.
    pub max_value: Option<u64>,
    /// The nodes the run uses.
    pub nodes: u32,
}

impl Profile {
    /// This profile with `count` more fault events of `class`, or `None` when the sum
    /// does not fit a `u32`. Never a clamp: every `u32` count, `u32::MAX` included, is
    /// an exact count, checked against its bound like any other.
    #[must_use]
    pub fn with_fault(mut self, class: FaultClass, count: u32) -> Option<Self> {
        let slot = self.faults.entry(class).or_default();
        *slot = slot.checked_add(count)?;
        Some(self)
    }

    /// One fault event of `class`.
    fn single(class: FaultClass) -> Self {
        Self {
            faults: BTreeMap::from([(class, 1)]),
            ..Self::default()
        }
    }

    /// The profile's counts, widened.
    fn wide(&self) -> Wide {
        Wide {
            faults: self
                .faults
                .iter()
                .map(|(&c, &n)| (c, u64::from(n)))
                .collect(),
            cancellations: u64::from(self.cancellations),
            max_value: self.max_value,
            nodes: u64::from(self.nodes),
        }
    }
}

/// A profile's counts in `u64`: the sum of two `u32` counts always fits, so composing
/// a generic neighbor's profile with the core's cannot overflow and is never clamped.
/// An exact bound (`u32::MAX` included) is admitted; only a count past its bound is
/// refused.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Wide {
    faults: BTreeMap<FaultClass, u64>,
    cancellations: u64,
    max_value: Option<u64>,
    nodes: u64,
}

impl Wide {
    fn count(&self, class: FaultClass) -> u64 {
        self.faults.get(&class).copied().unwrap_or(0)
    }
}

impl Envelope {
    /// The envelope of a contract's fault model and bounds, with a scenario's caps.
    #[must_use]
    pub const fn new(model: FaultModel, bounds: Bounds, caps: Caps) -> Self {
        Self {
            model,
            bounds,
            caps,
        }
    }

    /// Whether `profile` is well typed against the envelope; the first failure, in the
    /// module documentation's order, otherwise.
    ///
    /// # Errors
    ///
    /// The typed [`Rejection`].
    pub fn check(&self, profile: &Profile) -> Result<(), Rejection> {
        self.check_wide(&profile.wide())
    }

    /// Every bound is inclusive: a count equal to its bound is admitted (`<=`), and
    /// the `values` bound is a count of values, so the largest admitted index is one
    /// less than it.
    fn check_wide(&self, profile: &Wide) -> Result<(), Rejection> {
        for (&class, &count) in &profile.faults {
            if count > 0 && !self.model.enabled().contains(&class) {
                return Err(Rejection::OutsideFaultModel { class });
            }
        }
        let caps = [
            (
                BoundKind::Crashes,
                self.caps.crashes,
                profile.count(FaultClass::Crash),
            ),
            (
                BoundKind::Cancellations,
                self.caps.cancellations,
                // Crashes are cancellations too: a profile that under-reports them is
                // read at its crash count.
                profile.cancellations.max(profile.count(FaultClass::Crash)),
            ),
            (
                BoundKind::Partitions,
                self.caps.partitions,
                profile.count(FaultClass::Partition),
            ),
        ];
        for (what, cap, count) in caps {
            if let Some(bound) = cap
                && count > u64::from(bound)
            {
                return Err(Rejection::ExceedsFaultBound {
                    what,
                    count,
                    bound: u64::from(bound),
                });
            }
        }
        // A recovery completes a counted crash: never more recoveries than crashes.
        let (recoveries, crashes) = (
            profile.count(FaultClass::Recovery),
            profile.count(FaultClass::Crash),
        );
        if recoveries > crashes {
            return Err(Rejection::ExceedsFaultBound {
                what: BoundKind::Recoveries,
                count: recoveries,
                bound: crashes,
            });
        }
        // Summed wide: a handful of classes of at most 2 * u32::MAX each.
        let injected: u128 = profile
            .faults
            .iter()
            .filter(|(c, _)| **c != FaultClass::Recovery)
            .map(|(_, n)| u128::from(*n))
            .sum();
        let injected = u64::try_from(injected).unwrap_or(u64::MAX);
        match self.bounds.faults() {
            DeclaredBound::Declared(bound) => {
                let bound = u64::try_from(bound).unwrap_or(0);
                if injected > bound {
                    return Err(Rejection::ExceedsFaultBound {
                        what: BoundKind::Faults,
                        count: injected,
                        bound,
                    });
                }
            }
            DeclaredBound::Undeclared => {
                if injected > 0 {
                    return Err(Rejection::BoundUndeclared {
                        component: "faults",
                    });
                }
            }
        }
        if let (Some(value), ExplorationBound::Bounded(bound)) =
            (profile.max_value, self.bounds.values())
        {
            let bound = u64::try_from(bound).unwrap_or(0);
            if value >= bound {
                return Err(Rejection::OutsideValueDomain { value, bound });
            }
        }
        match self.bounds.nodes() {
            DeclaredBound::Declared(bound) => {
                let bound = u64::try_from(bound).unwrap_or(0);
                if profile.nodes > bound {
                    return Err(Rejection::OutsideNodeDomain {
                        nodes: profile.nodes,
                        bound,
                    });
                }
            }
            DeclaredBound::Undeclared => {
                if profile.nodes > 0 {
                    return Err(Rejection::BoundUndeclared { component: "nodes" });
                }
            }
        }
        Ok(())
    }

    fn to_json(&self) -> Json {
        let cap = |c: Option<u32>| c.map_or(Json::Null, |n| Json::Integer(i64::from(n)));
        obj([
            ("bounds", self.bounds.artifact_json()),
            (
                "caps",
                obj([
                    ("cancellations", cap(self.caps.cancellations)),
                    ("crashes", cap(self.caps.crashes)),
                    ("partitions", cap(self.caps.partitions)),
                ]),
            ),
            ("fault_model", self.model.artifact_json()),
        ])
    }
}

// ---------------------------------------------------------------------------
// candidates, rejections, the substrate
// ---------------------------------------------------------------------------

/// A neighbor, as the generators state it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Edit<S> {
    /// A schedule of the core's events: `order[k]` is the event run `k`-th.
    Schedule(Vec<usize>),
    /// A message fault on a send of the core: [`FaultClass::Duplication`] or
    /// [`FaultClass::Loss`].
    Message {
        /// The send.
        event: usize,
        /// The fault.
        fault: FaultClass,
    },
    /// A scenario edit in the substrate's own terms.
    Scenario(S),
}

/// Why a candidate is not a neighbor. It is counted and never realized or executed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Rejection {
    /// A schedule that is not a permutation of the core's events.
    MalformedSchedule,
    /// A schedule that runs `event` before its hard predecessor `predecessor`.
    ViolatesCausalOrder {
        /// The event.
        event: usize,
        /// The predecessor it overtakes.
        predecessor: usize,
    },
    /// A `schedule-perturbation` schedule that reverses the dependent pair `(first,
    /// second)`: it left the core's trace class.
    LeavesTraceClass {
        /// The earlier event of the pair in the core.
        first: usize,
        /// The later one.
        second: usize,
    },
    /// A message fault that is not the duplication or loss of a send of the core.
    NotAMessage {
        /// The event.
        event: usize,
    },
    /// A fault class the contract's fault model does not enable.
    OutsideFaultModel {
        /// The class.
        class: FaultClass,
    },
    /// A count past its bound.
    ExceedsFaultBound {
        /// Which bound.
        what: BoundKind,
        /// The profile's count.
        count: u64,
        /// The bound.
        bound: u64,
    },
    /// A bound the profile needs is not declared: fail closed.
    BoundUndeclared {
        /// `faults` or `nodes`.
        component: &'static str,
    },
    /// A value index outside the contract's `values` bound.
    OutsideValueDomain {
        /// The value index.
        value: u64,
        /// The bound.
        bound: u64,
    },
    /// More nodes than the contract's `nodes` bound.
    OutsideNodeDomain {
        /// The nodes.
        nodes: u64,
        /// The bound.
        bound: u64,
    },
    /// The same neighbor as the core, or as an earlier neighbor of its strategy (the
    /// same schedule, or the same content commitment).
    Duplicate,
    /// A second binding in either direction of the strategy's identity-content
    /// bijection: one identity naming two contents, or one content under two
    /// identities. Never deduplicated: an engine error.
    IdentityCollision,
    /// A substrate string of the candidate (its identity or its edit spelling) was past
    /// its cap: nothing of it was kept. An engine error.
    Oversized,
    /// The candidate's content digest equals an earlier one's while the canonical bytes
    /// differ (ADR-0013: collisions resolve by exact comparison). An engine error.
    DigestCollision,
    /// The core names a hard predecessor out of range or not earlier than its event.
    MalformedCore {
        /// The event.
        event: usize,
    },
}

impl Rejection {
    /// Whether this refusal is the engine's, not the candidate's: the candidate may be
    /// a neighbor the intent permits, dropped for a reason other than being outside the
    /// envelope or not a neighbor. Such a refusal is a coverage gap and makes the
    /// campaign an engine error, never Complete. Exhaustive, so a new rejection must be
    /// classified here.
    #[must_use]
    pub const fn is_engine_error(&self) -> bool {
        match self {
            // Not a neighbor of the core, or outside the Intent Contract's envelope.
            Self::MalformedSchedule
            | Self::ViolatesCausalOrder { .. }
            | Self::LeavesTraceClass { .. }
            | Self::NotAMessage { .. }
            | Self::OutsideFaultModel { .. }
            | Self::ExceedsFaultBound { .. }
            | Self::BoundUndeclared { .. }
            | Self::OutsideValueDomain { .. }
            | Self::OutsideNodeDomain { .. }
            // Covered already: the core itself or an earlier neighbor.
            | Self::Duplicate => false,
            // The engine could not decide the candidate.
            Self::IdentityCollision
            | Self::Oversized
            | Self::DigestCollision
            | Self::MalformedCore { .. } => true,
        }
    }

    /// A stable token for counting.
    #[must_use]
    pub const fn token(&self) -> &'static str {
        match self {
            Self::MalformedSchedule => "malformed-schedule",
            Self::ViolatesCausalOrder { .. } => "violates-causal-order",
            Self::LeavesTraceClass { .. } => "leaves-trace-class",
            Self::NotAMessage { .. } => "not-a-message",
            Self::OutsideFaultModel { .. } => "outside-fault-model",
            Self::ExceedsFaultBound { .. } => "exceeds-fault-bound",
            Self::BoundUndeclared { .. } => "bound-undeclared",
            Self::OutsideValueDomain { .. } => "outside-value-domain",
            Self::OutsideNodeDomain { .. } => "outside-node-domain",
            Self::Duplicate => "duplicate",
            Self::IdentityCollision => "identity-collision",
            Self::Oversized => "oversized",
            Self::DigestCollision => "digest-collision",
            Self::MalformedCore { .. } => "malformed-core",
        }
    }

    /// A rendering with the operands.
    #[must_use]
    pub fn render(&self) -> String {
        match self {
            Self::ViolatesCausalOrder { event, predecessor } => {
                format!("violates-causal-order({event} before {predecessor})")
            }
            Self::LeavesTraceClass { first, second } => {
                format!("leaves-trace-class({first},{second})")
            }
            Self::NotAMessage { event } => format!("not-a-message({event})"),
            Self::OutsideFaultModel { class } => format!("outside-fault-model({class})"),
            Self::ExceedsFaultBound { what, count, bound } => {
                format!("exceeds-fault-bound({} {count} > {bound})", what.token())
            }
            Self::BoundUndeclared { component } => format!("bound-undeclared({component})"),
            Self::OutsideValueDomain { value, bound } => {
                format!("outside-value-domain({value} >= {bound})")
            }
            Self::OutsideNodeDomain { nodes, bound } => {
                format!("outside-node-domain({nodes} > {bound})")
            }
            other => other.token().to_owned(),
        }
    }
}

/// A substrate's answer to a valid neighbor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Realized<R> {
    /// A run the program and its binding produce, with the witness the library checks
    /// before it executes the run as this neighbor.
    Run {
        /// The run.
        run: R,
        /// What the run did, in the core's terms.
        witness: Witness,
    },
    /// The program cannot produce it: why, as a stable token.
    NotRealizable(String),
    /// The substrate has no semantics for it (INV-008 unsupported): why.
    Unsupported(String),
}

/// What a realized run did, in the terms the library checks it by: the causal-neighbor
/// definition is the library's, not the substrate's.
///
/// - A schedule neighbor is realized when the run is a causal neighbor of the requested
///   schedule: [`Witness::Schedule`] accounts for every event of the core, taken or
///   absent from the program, and the library requires that every pair of events the
///   core's dependence relation ([`CausalCore::dependent`]) relates, both taken, keeps
///   the requested order (Mazurkiewicz equivalence under the core's dependence,
///   restricted to what the program has), and that the causal decision the candidate
///   was generated around (its moved pair) was taken, in the requested order. A program
///   that removes or reorders the decision does not realize the neighbor.
/// - A scenario or message neighbor is realized when the run is of that edit:
///   [`Witness::Edit`] echoes the edit's canonical bytes, and the library compares them
///   with the committed ones. Its schedule is not part of the neighbor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Witness {
    /// The core events the run took, in order, and those the program does not have.
    Schedule {
        /// Taken core events, in the order the run took them.
        taken: Vec<usize>,
        /// Core events the program does not have at all.
        absent: Vec<usize>,
        /// Whether the run stopped before its end (a deadlock). A halted run need not
        /// take every event, and it counts as the neighbor only when it fails.
        halted: bool,
    },
    /// The canonical bytes of the edit the run is of.
    Edit(Vec<u8>),
}

/// How one executed neighbor came out.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Execution {
    /// Every checked property holds on the run.
    Pass {
        /// The run's handle.
        run: String,
    },
    /// A checked property fails on the run.
    Fail {
        /// The first property refuted.
        property: String,
        /// The run's handle, disclosed in the receipt.
        run: String,
    },
    /// The run could not be decided.
    Inconclusive {
        /// Why (INV-008).
        reason: InconclusiveReason,
    },
}

/// Why a strategy did not run at all.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NotRun(pub String);

/// The program under evaluation and its runs. One substrate is one program: to compare
/// a candidate repair with the failing program, run a campaign on each.
pub trait Substrate {
    /// The core's order.
    type Core: CausalCore;
    /// A scenario edit in the substrate's terms.
    type Scenario: Clone;
    /// A realized run.
    type Run;

    /// The failing run's causal core.
    fn core(&self) -> &Self::Core;
    /// The core's content identity: which failing run and reduction it is. The record
    /// also commits to the core's whole relation, so this names it and does not stand
    /// for it.
    fn core_identity(&self) -> String;
    /// The content identity of the program under evaluation: the candidate (for a
    /// repair transaction, its sealed candidate snapshot). Two programs with one subject
    /// are one candidate to the record's binding, so it must change whenever the
    /// program does.
    fn subject(&self) -> String;
    /// The identity of what realizes and judges a neighbor: the engine or harness, its
    /// function version and every semantic epoch it reads (RFC 0030's pins). A record
    /// or continuation made under another engine identity is stale.
    fn engine_identity(&self) -> String;
    /// A scenario edit's canonical bytes, committed per neighbor: a changed edit under
    /// an unchanged identity is detected, never reused.
    fn scenario_bytes(&self, scenario: &Self::Scenario) -> Vec<u8>;
    /// The core's own profile: every schedule neighbor inherits it.
    fn core_profile(&self) -> Profile;
    /// `Some` when the substrate has no semantics for `strategy`.
    fn not_run(&self, strategy: Strategy) -> Option<NotRun>;
    /// How many scenario edits `strategy` has (only asked for non-generic strategies).
    fn scenario_count(&self, strategy: Strategy) -> usize;
    /// Scenario edit `index` of `strategy`: its identity within the strategy, the edit
    /// and its profile. `None` ends the strategy early.
    fn scenario(
        &self,
        strategy: Strategy,
        index: usize,
    ) -> Option<(String, Self::Scenario, Profile)>;
    /// Realize a valid neighbor as a run of the program.
    fn realize(&mut self, edit: &Edit<Self::Scenario>) -> Realized<Self::Run>;
    /// Check the run's properties.
    fn execute(&mut self, run: &Self::Run) -> Execution;
}

// ---------------------------------------------------------------------------
// configuration and budget
// ---------------------------------------------------------------------------

/// What a campaign may spend. Each component is charged before the work it pays for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Budget {
    /// Candidates generated.
    pub candidates: u64,
    /// Runs realized and executed.
    pub runs: u64,
    /// Work units: generator steps, candidate construction and validation walks.
    pub work: u64,
}

/// What a campaign spent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Spent {
    /// Candidates generated.
    pub candidates: u64,
    /// Runs realized.
    pub runs: u64,
    /// Work units.
    pub work: u64,
}

/// A campaign's parameters. Every one of them is recorded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    /// The seed every sampled part draws from.
    pub seed: u64,
    /// The strategies to run; the rest are recorded as not selected.
    pub strategies: Vec<Strategy>,
    /// Seeded walks of `schedule-perturbation`.
    pub walks: u32,
    /// Independent swaps per walk.
    pub walk_length: u32,
    /// The largest delay of a send, in positions.
    pub max_delay: u32,
    /// The budget.
    pub budget: Budget,
    /// The repair transaction this campaign evaluates a candidate for: its `rt_*`
    /// handle (plan §4.4). The record binds it, so gate-5 evidence names the transaction
    /// it was computed for.
    pub transaction: String,
}

#[derive(Debug, Clone, Copy)]
struct Exhausted;

/// Why a generator stopped early.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Stop {
    /// The budget cannot pay for the next step: suspend with a continuation.
    Exhausted,
    /// A resumed campaign regenerated a committed candidate that differs from the one
    /// its continuation records.
    Mismatch,
}

impl From<Exhausted> for Stop {
    fn from(_: Exhausted) -> Self {
        Self::Exhausted
    }
}

/// Work charges are computed with saturating arithmetic on purpose, and only there: a
/// charge is an upper estimate of work, and a saturated one (`u64::MAX`) exceeds every
/// admissible budget (at most `i64::MAX`, refused otherwise by `check_budget`), so the
/// meter refuses it as exhaustion, a typed outcome. Saturation can only over-charge,
/// never admit. Bounded semantic quantities (fault counts, cancellations, values,
/// coverage counts) never saturate: they use checked arithmetic and fail typed.
struct Meter {
    budget: Budget,
    spent: Spent,
}

impl Meter {
    /// Take back the candidate charge of a candidate that was not built (its work stays
    /// charged), so the candidate count matches the committed candidates. It follows
    /// its own charge, so the count is at least one; were it zero, nothing is taken
    /// back and the spend is overstated, never understated.
    fn return_candidate(&mut self) {
        if let Some(n) = self.spent.candidates.checked_sub(1) {
            self.spent.candidates = n;
        }
    }

    fn work(&mut self, units: u64) -> Result<(), Exhausted> {
        let next = self.spent.work.checked_add(units).ok_or(Exhausted)?;
        if next > self.budget.work {
            return Err(Exhausted);
        }
        self.spent.work = next;
        Ok(())
    }

    fn candidate(&mut self, units: u64) -> Result<(), Exhausted> {
        if self.spent.candidates >= self.budget.candidates {
            return Err(Exhausted);
        }
        self.work(units)?;
        self.spent.candidates += 1;
        Ok(())
    }

    fn run(&mut self) -> Result<(), Exhausted> {
        if self.spent.runs >= self.budget.runs {
            return Err(Exhausted);
        }
        self.spent.runs += 1;
        Ok(())
    }
}

/// SplitMix64: the explicit, seeded source of the walks (INV-005).
struct SplitMix(u64);

impl SplitMix {
    fn next(&mut self) -> u64 {
        // Wrapping is the generator's definition (SplitMix64 is arithmetic mod 2^64),
        // not a bounded quantity.
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }

    fn below(&mut self, n: usize) -> usize {
        let n64 = u64::try_from(n).unwrap_or(u64::MAX).max(1);
        usize::try_from(self.next() % n64).unwrap_or(0)
    }
}

// ---------------------------------------------------------------------------
// the record
// ---------------------------------------------------------------------------

/// What happened to one candidate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Disposition {
    /// Not a neighbor.
    Rejected(Rejection),
    /// The program cannot produce it.
    NotRealizable(String),
    /// The substrate has no semantics for it.
    Unsupported(String),
    /// Executed.
    Executed(Execution),
}

impl Disposition {
    fn token(&self) -> &'static str {
        match self {
            Self::Rejected(_) => "rejected",
            Self::NotRealizable(_) => "not-realizable",
            Self::Unsupported(_) => "unsupported",
            Self::Executed(Execution::Pass { .. }) => "pass",
            Self::Executed(Execution::Fail { .. }) => "fail",
            Self::Executed(Execution::Inconclusive { .. }) => "inconclusive",
        }
    }
}

/// One candidate and its disposition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Neighbor {
    /// Its strategy.
    pub strategy: Strategy,
    /// Its identity within the strategy's enumeration.
    pub id: String,
    /// What happened to it.
    pub disposition: Disposition,
    /// The candidate's content commitment: a digest over its strategy, identity, edit
    /// bytes and profile. A resume regenerates every committed candidate and compares
    /// it, so a candidate whose semantics changed under the same identity is refused.
    pub commitment: String,
    /// For an executed neighbor, the checked realization witness as canonical text:
    /// what the run actually took. Empty otherwise.
    pub witness: String,
    /// For a non-schedule candidate, the substrate identity it came with (bounded);
    /// empty for a schedule. With [`Self::content`] it is what the record's bijection
    /// check reads.
    pub identity: String,
    /// For a non-schedule candidate, its content address; empty for a schedule.
    pub content: String,
}

/// One strategy's coverage.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Coverage {
    /// Candidates generated (and charged).
    pub generated: u64,
    /// Candidates rejected, by [`Rejection::token`].
    pub rejected: BTreeMap<&'static str, u64>,
    /// Valid neighbors: generated and not rejected.
    pub valid: u64,
    /// Valid neighbors the substrate has no semantics for.
    pub unsupported: u64,
    /// Valid neighbors the program cannot produce, by reason.
    pub not_realizable: BTreeMap<String, u64>,
    /// Neighbors realized and executed.
    pub executed: u64,
    /// Executions on which every property holds.
    pub passed: u64,
    /// Executions on which a property fails.
    pub failed: u64,
    /// Executions that could not be decided.
    pub inconclusive: u64,
    /// Distinct runs among the executions, by run handle. A substrate may realize two
    /// neighbors as one run (a candidate program with fewer interleavings than the
    /// core's, say); this count says how many runs the executions really were.
    pub distinct_runs: u64,
    /// The committed frontier, when the budget truncated the strategy.
    pub frontier: Option<String>,
}

/// One strategy's line in the record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StrategyStatus {
    /// It ran, to its end or to its frontier.
    Ran(Coverage),
    /// It did not run: why.
    NotRun(NotRun),
}

/// How a campaign ended. Not a success flag: the failing neighbors are data.
///
/// `Complete` is the only verdict whose coverage a receipt may carry, and it is earned,
/// never defaulted: the selection is not empty, every selected strategy ran to its end,
/// at least one neighbor was executed, and every valid neighbor was executed and
/// decided. Anything else is disclosed as what it is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    /// Every selected strategy ran to its end and every valid neighbor was executed
    /// and decided.
    Complete,
    /// The budget ran out: gate 5 stays pending, and the campaign resumes from its
    /// [`Continuation`] (RFC 0032, "Cost governance": `BudgetExhausted` with a
    /// continuation, never a smaller campaign). Each truncated strategy discloses its
    /// frontier.
    Pending,
    /// The campaign finished and cannot vouch for its coverage (INV-008):
    /// [`InconclusiveReason::Unsupported`] when a selected strategy did not run, when a
    /// valid neighbor was unsupported or not realizable, or when nothing was executed;
    /// the run's own reason when an executed neighbor was undecided;
    /// [`InconclusiveReason::EngineError`] when a run could not be named or a substrate
    /// capped a strategy.
    Inconclusive {
        /// Why (INV-008).
        reason: InconclusiveReason,
    },
}

/// Why a record cannot be projected onto a receipt's `coverage.neighborhood`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProjectionRefusal {
    /// A failing neighbor whose run handle is not a plan §4.4 handle.
    UndisclosableRun {
        /// The neighbor.
        neighbor: String,
    },
    /// Executed neighbors that could not be decided.
    InconclusiveRuns {
        /// How many.
        count: u64,
    },
    /// The campaign is inconclusive.
    Inconclusive {
        /// Why.
        reason: InconclusiveReason,
    },
    /// The campaign is pending on its budget: resume it first.
    Pending,
    /// The record was computed for other inputs: the first field that differs.
    Stale {
        /// The field.
        field: String,
    },
    /// The record's accounting does not reconcile: what disagrees.
    Inconsistent(String),
    /// The verifier's own inputs cannot be derived.
    Core(CoreError),
}

/// A failing neighbor, disclosed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FailingNeighbor {
    /// Its strategy, which is its class.
    pub strategy: Strategy,
    /// Its identity within the strategy's enumeration.
    pub neighbor: String,
    /// The property refuted.
    pub property: String,
    /// The run's handle.
    pub run: String,
}

/// A campaign's record: what a receipt discloses about gate 5.
///
/// Its fields are private and it has no public constructor: a record exists only as the
/// output of [`explore`] or [`resume`] in this process, so a caller cannot edit a
/// verdict, drop a failing neighbor or retarget it. It is bound to its [`Inputs`], and
/// [`Self::receipt_coverage`] refuses it against any other inputs (stale), reconciles
/// every count with the dispositions, and projects only a complete campaign. A record
/// read back from bytes is never trusted: [`resume_untrusted`] re-derives it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NeighborhoodRecord {
    inputs: Inputs,
    core: String,
    core_events: u64,
    core_members: Vec<u64>,
    subject: String,
    config: Config,
    strategies: Vec<(Strategy, StrategyStatus)>,
    neighbors: Vec<Neighbor>,
    failing: Vec<FailingNeighbor>,
    spent: Spent,
    verdict: Verdict,
    envelope: Json,
}

impl NeighborhoodRecord {
    /// The inputs the record is bound to.
    #[must_use]
    pub const fn inputs(&self) -> &Inputs {
        &self.inputs
    }
    /// The core's identity.
    #[must_use]
    pub fn core(&self) -> &str {
        &self.core
    }
    /// The core's length.
    #[must_use]
    pub const fn core_events(&self) -> u64 {
        self.core_events
    }
    /// The core events proper, ascending.
    #[must_use]
    pub fn core_members(&self) -> &[u64] {
        &self.core_members
    }
    /// The program evaluated.
    #[must_use]
    pub fn subject(&self) -> &str {
        &self.subject
    }
    /// The configuration.
    #[must_use]
    pub const fn config(&self) -> &Config {
        &self.config
    }
    /// Per strategy, in [`Strategy::ALL`] order.
    #[must_use]
    pub fn strategies(&self) -> &[(Strategy, StrategyStatus)] {
        &self.strategies
    }
    /// Every candidate, in generation order.
    #[must_use]
    pub fn neighbors(&self) -> &[Neighbor] {
        &self.neighbors
    }
    /// Every failing neighbor, in generation order.
    #[must_use]
    pub fn failing(&self) -> &[FailingNeighbor] {
        &self.failing
    }
    /// What was spent.
    #[must_use]
    pub const fn spent(&self) -> Spent {
        self.spent
    }
    /// How it ended.
    #[must_use]
    pub const fn verdict(&self) -> Verdict {
        self.verdict
    }
}

/// The generator's own identity, bound into every record: a change to how neighbors
/// are generated or validated is a new generator.
pub const GENERATOR: &str = "continuum-repair/neighborhood@2";

/// What a record was computed from, as canonical JSON: the generator, the transaction,
/// the candidate (subject), the engine identity, the core (its identity, a commitment
/// to its whole relation, its profile), the envelope, and every configuration field but
/// the budget. [`Inputs::of`] derives it from the live inputs, so a verifier compares a
/// record's binding with what the transaction holds now.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Inputs(Json);

impl Inputs {
    /// The inputs `substrate`, `envelope` and `config` give now. Checking the core and
    /// committing to its relation are charged against `config.budget.work`.
    ///
    /// # Errors
    ///
    /// [`CoreError`], as for [`explore`].
    pub fn of<S: Substrate>(
        substrate: &S,
        envelope: &Envelope,
        config: &Config,
    ) -> Result<Self, CoreError> {
        check_budget(config)?;
        let mut meter = Meter {
            budget: config.budget,
            spent: Spent::default(),
        };
        let facts = check_core(substrate.core(), &mut meter)?;
        let ids = Identities::of(substrate, &mut meter)?;
        // Read once, and held to the envelope as a campaign holds it.
        let core_profile = substrate.core_profile();
        envelope
            .check(&core_profile)
            .map_err(CoreError::CoreOutsideEnvelope)?;
        Ok(Self::build(
            envelope,
            config,
            &facts,
            &ids,
            &core_profile,
            BLAKE3_INDEX,
        ))
    }

    fn build(
        envelope: &Envelope,
        config: &Config,
        facts: &Facts,
        ids: &Identities,
        core_profile: &Profile,
        content_index: &str,
    ) -> Self {
        let members = (0..facts.n)
            .filter(|&e| facts.frozen.core[e])
            .map(|e| int(u64::try_from(e).unwrap_or(u64::MAX)))
            .collect();
        Self(obj([
            (
                "config",
                obj([
                    ("max_delay", int(u64::from(config.max_delay))),
                    ("seed", text(&format!("{:#018x}", config.seed))),
                    (
                        "selected",
                        Json::Array(
                            selected_of(config)
                                .into_iter()
                                .map(|s| text(s.token()))
                                .collect(),
                        ),
                    ),
                    ("walk_length", int(u64::from(config.walk_length))),
                    ("walks", int(u64::from(config.walks))),
                ]),
            ),
            (
                "core",
                obj([
                    ("commitment", text(&facts.commitment)),
                    ("relation", text(&facts.relation)),
                    ("events", int(u64::try_from(facts.n).unwrap_or(u64::MAX))),
                    ("identity", text(&ids.core)),
                    ("members", Json::Array(members)),
                    ("profile", core_profile.to_json()),
                ]),
            ),
            ("engine", text(&ids.engine)),
            ("envelope", envelope.to_json()),
            ("generator", text(GENERATOR)),
            // The content index the record's addresses were made with: a verifier's
            // own derivation always says BLAKE3, so a record made with any other index
            // is stale to it and never projects.
            ("content_index", text(content_index)),
            ("subject", text(&ids.subject)),
            ("transaction", text(&config.transaction)),
        ]))
    }

    /// The canonical bytes.
    #[must_use]
    pub fn canonical_bytes(&self) -> Vec<u8> {
        self.0.to_canonical_bytes()
    }

    /// A BLAKE3 digest of the canonical bytes, for indexing only (ADR-0013).
    #[must_use]
    pub fn digest(&self) -> String {
        Blake3Hasher::hash(&self.canonical_bytes()).to_string()
    }

    /// The first top-level field in which `self` and `other` differ.
    fn first_difference(&self, other: &Self) -> Option<String> {
        let (Some(a), Some(b)) = (self.0.as_object(), other.0.as_object()) else {
            return Some("inputs".to_owned());
        };
        a.keys()
            .chain(b.keys())
            .find(|k| a.get(*k) != b.get(*k))
            .cloned()
    }
}

impl Profile {
    /// The profile as canonical JSON.
    #[must_use]
    pub fn to_json(&self) -> Json {
        obj([
            ("cancellations", int(u64::from(self.cancellations))),
            (
                "faults",
                // A zero count means the class is absent: one spelling for one profile.
                Json::Object(
                    self.faults
                        .iter()
                        .filter(|(_, n)| **n > 0)
                        .map(|(c, n)| (c.wire().to_owned(), int(u64::from(*n))))
                        .collect(),
                ),
            ),
            // Exact at every value: a decimal string, never a clamped integer.
            (
                "max_value",
                self.max_value
                    .map_or(Json::Null, |v| Json::String(v.to_string())),
            ),
            ("nodes", int(u64::from(self.nodes))),
        ])
    }
}

/// Whether refusing a candidate of `strategy` for `r` is the engine's failure: an
/// engine rejection, or a schedule-validity rejection in a strategy whose schedules are
/// valid by construction (the hoist of a causal decision, the delay of a send), where
/// it means the generator dropped a neighbor it should have built. The one rule the
/// campaign and [`taints`] both apply.
fn engine_refusal(strategy: Strategy, r: &Rejection) -> bool {
    r.is_engine_error()
        || (matches!(
            strategy,
            Strategy::AlternateEnabledEvents | Strategy::MessageDuplicationLossDelay
        ) && matches!(
            r,
            Rejection::MalformedSchedule
                | Rejection::ViolatesCausalOrder { .. }
                | Rejection::LeavesTraceClass { .. }
        ))
}

/// Whether a committed neighbor makes the campaign an engine error: a run it cannot
/// name, an outcome string replaced for its size, an oversized candidate, or an
/// identity collision.
fn taints(n: &Neighbor) -> bool {
    if let Disposition::Rejected(r) = &n.disposition
        && engine_refusal(n.strategy, r)
    {
        return true;
    }
    let oversized = |s: &str| s.starts_with(OVERSIZED);
    match &n.disposition {
        Disposition::Executed(Execution::Pass { run }) => !is_handle(run),
        Disposition::Executed(Execution::Fail { run, property }) => {
            !is_handle(run) || oversized(property)
        }
        Disposition::NotRealizable(w) | Disposition::Unsupported(w) => oversized(w),
        Disposition::Rejected(_) => false,
        Disposition::Executed(Execution::Inconclusive { .. }) => false,
    }
}

/// The witness as the record keeps it: its canonical text, not a digest (ADR-0013),
/// bounded by the core's length. An edit echo has already been checked equal, byte for
/// byte, to the neighbor's own content, so it is recorded as that fact.
fn witness_text(w: &Witness) -> String {
    match w {
        Witness::Schedule {
            taken,
            absent,
            halted,
        } => format!("schedule taken{taken:?} absent{absent:?} halted:{halted}"),
        Witness::Edit(_) => "edit echo equal to the neighbor's content".to_owned(),
    }
}

/// The canonical bytes of a message edit, which a substrate echoes in its witness.
#[must_use]
pub fn message_bytes(event: usize, fault: FaultClass) -> Vec<u8> {
    let mut b = b"message".to_vec();
    b.extend_from_slice(&u64::try_from(event).unwrap_or(u64::MAX).to_be_bytes());
    b.extend_from_slice(fault.wire().as_bytes());
    b
}

/// The length an [`admit`] refusal reported, 0 otherwise.
const fn oversized_len<T>(a: &Admitted<T>) -> usize {
    match a {
        Admitted::Oversized(l) => *l,
        _ => 0,
    }
}

/// The binding's name for the production content index.
const BLAKE3_INDEX: &str = "blake3";

/// The binding's name for any other content index (the test hook): never current.
const TEST_INDEX: &str = "test-hook (not blake3)";

/// The content index: BLAKE3, as ADR-0013's cryptographic hash. An index only.
fn blake3_index(bytes: &[u8]) -> String {
    Blake3Hasher::hash(bytes).to_string()
}

/// [`explore`] with another content index in place of BLAKE3: a test hook, so a test
/// can force a digest collision and see it decided on the bytes (ADR-0013). The index
/// decides neither equality nor inequality (both are decided on the canonical bytes),
/// and the record binds the index's name: a record made here is stale to every
/// verifier, whose derivation says BLAKE3, and never projects.
///
/// # Errors
///
/// As [`explore`].
#[doc(hidden)]
pub fn explore_with_content_index<S: Substrate>(
    substrate: &mut S,
    envelope: &Envelope,
    config: &Config,
    index: fn(&[u8]) -> String,
) -> Result<Exploration, CoreError> {
    run_campaign(
        substrate,
        envelope,
        config,
        None,
        index,
        TEST_INDEX,
        &mut Spent::default(),
    )
    .map_err(|e| match e {
        ResumeError::Core(c) => c,
        _ => CoreError::CoreCheckExhausted,
    })
}

/// The bytes a committed neighbor keeps, plus a constant for its fixed fields.
fn neighbor_weight(n: &Neighbor) -> u64 {
    let strings = n.id.len()
        + n.commitment.len()
        + n.witness.len()
        + n.identity.len()
        + n.content.len()
        + match &n.disposition {
            Disposition::Rejected(_) => 0,
            Disposition::NotRealizable(w) | Disposition::Unsupported(w) => w.len(),
            Disposition::Executed(Execution::Pass { run }) => run.len(),
            Disposition::Executed(Execution::Fail { property, run }) => property.len() + run.len(),
            Disposition::Executed(Execution::Inconclusive { .. }) => 0,
        };
    u64::try_from(strings)
        .unwrap_or(u64::MAX)
        .saturating_add(64)
}

/// The pending outcome of a resume that ran out of budget before its prior frontier:
/// nothing new is committed, but the work is. The record is rebuilt from the
/// campaign's own copies of the committed state (charged when they were copied) with
/// the cumulative spend, so a retry never repeats uncharged work and the ledger only
/// advances (RFC 0032, cumulative cost); the frontier stays the prior one.
#[allow(clippy::too_many_arguments)]
fn rebuild_pending<S: Substrate>(
    c: &Continuation,
    inputs: Inputs,
    [core, subject]: [String; 2],
    core_events: u64,
    core_members: Vec<u64>,
    config: &Config,
    campaign: &mut Campaign<'_, S>,
    envelope: Json,
) -> Exploration {
    let record = Arc::new(NeighborhoodRecord {
        inputs,
        core,
        core_events,
        core_members,
        subject,
        config: config.clone(),
        strategies: c.record.strategies.clone(),
        neighbors: std::mem::take(&mut campaign.neighbors),
        failing: std::mem::take(&mut campaign.failing),
        spent: campaign.meter.spent,
        verdict: Verdict::Pending,
        envelope,
    });
    let continuation = Continuation {
        record: Arc::clone(&record),
        strategy: c.strategy,
        index: c.index,
        contents: std::mem::take(&mut campaign.contents),
    };
    Exploration::Suspended {
        record,
        continuation: Box::new(continuation),
    }
}

fn check_budget(config: &Config) -> Result<(), CoreError> {
    let limit = u64::try_from(i64::MAX).unwrap_or(u64::MAX);
    let b = config.budget;
    if b.candidates > limit || b.runs > limit || b.work > limit {
        return Err(CoreError::BudgetTooLarge);
    }
    if !config.transaction.starts_with("rt_") || !is_handle(&config.transaction) {
        return Err(CoreError::NotATransaction);
    }
    Ok(())
}

fn obj<const N: usize>(fields: [(&str, Json); N]) -> Json {
    Json::Object(fields.into_iter().map(|(k, v)| (k.to_owned(), v)).collect())
}

fn int(n: u64) -> Json {
    // Every count is bounded by a budget component, and `explore` refuses a budget past
    // `i64::MAX`, so this conversion cannot saturate on a record it produced.
    Json::Integer(i64::try_from(n).unwrap_or(i64::MAX))
}

fn text(s: &str) -> Json {
    Json::String(s.to_owned())
}

fn reason_token(reason: InconclusiveReason) -> &'static str {
    reason.as_str()
}

/// The longest substrate identity string (core identity, subject, engine identity) the
/// binding keeps. A longer one refuses the campaign ([`CoreError::IdentityTooLong`]).
pub const MAX_IDENTITY_BYTES: u64 = 4_096;

/// A bound on a profile's canonical spelling: charged with every candidate, whose
/// retained content includes it.
const PROFILE_BYTES: u64 = 512;

/// The longest scenario identity a record keeps.
pub const MAX_SCENARIO_ID_BYTES: u64 = 256;

/// The longest scenario edit spelling (and its witness echo) a campaign reads.
pub const MAX_EDIT_BYTES: u64 = 65_536;

/// What [`admit`] made of one substrate-supplied string or byte vector.
enum Admitted<T> {
    /// Within its cap, and paid for: it may be kept.
    Kept(T),
    /// Past its cap: its length, and nothing of it is kept.
    Oversized(usize),
    /// The budget cannot pay to keep it.
    Exhausted,
}

/// The one way a substrate-supplied string or byte vector enters a record or a binding.
/// The length is read first (no copy); a value past `cap` is refused with its length.
/// A value within the cap is charged its byte length before it is kept, unless the
/// caller already reserved `cap` for it (`prepaid`, the run reserve).
fn admit<T: AsRef<[u8]>>(meter: &mut Meter, value: T, cap: u64, prepaid: bool) -> Admitted<T> {
    admit_copies(meter, value, cap, if prepaid { 0 } else { 1 })
}

/// [`admit`] for a value the crate keeps `copies` times: charged `copies` times its
/// length (0 when prepaid), so what is charged covers what is stored.
fn admit_copies<T: AsRef<[u8]>>(meter: &mut Meter, value: T, cap: u64, copies: u64) -> Admitted<T> {
    let len = value.as_ref().len();
    let len64 = u64::try_from(len).unwrap_or(u64::MAX);
    if len64 > cap {
        return Admitted::Oversized(len);
    }
    if copies > 0 && meter.work(len64.saturating_mul(copies)).is_err() {
        return Admitted::Exhausted;
    }
    Admitted::Kept(value)
}

/// The marker an oversized value is replaced by.
fn oversized_marker(len: usize) -> String {
    format!("{OVERSIZED}{len}>")
}

/// The substrate's three identity strings, read once and admitted, so what is charged
/// is exactly what is bound.
struct Identities {
    core: String,
    subject: String,
    engine: String,
}

impl Identities {
    fn of<S: Substrate>(substrate: &S, meter: &mut Meter) -> Result<Self, CoreError> {
        // Each identity is kept twice (the binding and the record's own field).
        let mut take = |s: String| match admit_copies(meter, s, MAX_IDENTITY_BYTES, 2) {
            Admitted::Kept(s) => Ok(s),
            Admitted::Oversized(len) => Err(CoreError::IdentityTooLong { len }),
            Admitted::Exhausted => Err(CoreError::CoreCheckExhausted),
        };
        Ok(Self {
            core: take(substrate.core_identity())?,
            subject: take(substrate.subject())?,
            engine: take(substrate.engine_identity())?,
        })
    }
}

/// The longest substrate outcome string a record keeps: a not-realizable or unsupported
/// reason, a refuted property. The run reserve pays for one of each and a handle; a
/// longer string is never copied: it is replaced by [`OVERSIZED`] and its length, and
/// the campaign is an engine error.
pub const MAX_OUTCOME_BYTES: u64 = 256;

/// The marker an oversized outcome string is replaced by, followed by its length.
pub const OVERSIZED: &str = "<oversized outcome string of bytes: ";

/// The longest run handle a record accepts. The run charge reserves this much work for
/// storing it; a longer handle is not a handle here.
pub const MAX_HANDLE_BYTES: u64 = 256;

/// Whether `handle` is a plan §4.4 handle as `promotion-receipt.schema.json` spells it,
/// `^[a-z][a-z0-9_]*_[A-Za-z0-9_-]+$`, of at most [`MAX_HANDLE_BYTES`].
#[must_use]
pub fn is_handle(handle: &str) -> bool {
    let b = handle.as_bytes();
    if b.len() > usize::try_from(MAX_HANDLE_BYTES).unwrap_or(usize::MAX) {
        return false;
    }
    if !b.first().is_some_and(u8::is_ascii_lowercase) {
        return false;
    }
    let prefix_ok = |c: &u8| c.is_ascii_lowercase() || c.is_ascii_digit() || *c == b'_';
    let tail_ok = |c: &u8| c.is_ascii_alphanumeric() || *c == b'_' || *c == b'-';
    // The tail after the separating underscore must be all tail-class: it starts after
    // the last byte outside that class. The separator must lie inside the prefix-class
    // run after the first byte. One pass each: linear in the handle.
    let tail_from = b.iter().rposition(|c| !tail_ok(c)).map_or(0, |p| p + 1);
    let prefix_end = b
        .iter()
        .skip(1)
        .position(|c| !prefix_ok(c))
        .map_or(b.len(), |p| p + 1);
    // A separator `u` with 1 <= u < prefix_end, u + 1 >= tail_from, u + 1 < len.
    let lo = tail_from.saturating_sub(1).max(1);
    let hi = prefix_end.min(b.len().saturating_sub(1));
    (lo..hi).any(|u| b[u] == b'_')
}

impl NeighborhoodRecord {
    /// The record as canonical JSON.
    #[must_use]
    pub fn to_json(&self) -> Json {
        let strategies = self
            .strategies
            .iter()
            .map(|(s, status)| match status {
                StrategyStatus::NotRun(NotRun(why)) => obj([
                    ("reason", text(why)),
                    ("status", text("not-run")),
                    ("strategy", text(s.token())),
                ]),
                StrategyStatus::Ran(c) => {
                    let mut fields = BTreeMap::new();
                    let mut put = |k: &str, v: Json| {
                        fields.insert(k.to_owned(), v);
                    };
                    put("status", text("ran"));
                    put("strategy", text(s.token()));
                    put("generated", int(c.generated));
                    put(
                        "rejected",
                        Json::Object(
                            c.rejected
                                .iter()
                                .map(|(k, v)| ((*k).to_owned(), int(*v)))
                                .collect(),
                        ),
                    );
                    put("valid", int(c.valid));
                    put("unsupported", int(c.unsupported));
                    put(
                        "not_realizable",
                        Json::Object(
                            c.not_realizable
                                .iter()
                                .map(|(k, v)| (k.clone(), int(*v)))
                                .collect(),
                        ),
                    );
                    put("executed", int(c.executed));
                    put("passed", int(c.passed));
                    put("failed", int(c.failed));
                    put("inconclusive", int(c.inconclusive));
                    put("distinct_runs", int(c.distinct_runs));
                    put("truncated", Json::Bool(c.frontier.is_some()));
                    if let Some(f) = &c.frontier {
                        put("frontier", text(f));
                    }
                    Json::Object(fields)
                }
            })
            .collect();
        let neighbors = self
            .neighbors
            .iter()
            .map(|n| {
                let mut fields = BTreeMap::new();
                fields.insert("strategy".to_owned(), text(n.strategy.token()));
                fields.insert("neighbor".to_owned(), text(&n.id));
                fields.insert("commitment".to_owned(), text(&n.commitment));
                if !n.witness.is_empty() {
                    fields.insert("witness".to_owned(), text(&n.witness));
                }
                if !n.identity.is_empty() || !n.content.is_empty() {
                    fields.insert("identity".to_owned(), text(&n.identity));
                    fields.insert("content".to_owned(), text(&n.content));
                }
                fields.insert("disposition".to_owned(), text(n.disposition.token()));
                match &n.disposition {
                    Disposition::Rejected(r) => {
                        fields.insert("detail".to_owned(), text(&r.render()));
                        fields.insert("rejection".to_owned(), r.to_json());
                    }
                    Disposition::NotRealizable(why) | Disposition::Unsupported(why) => {
                        fields.insert("detail".to_owned(), text(why));
                    }
                    Disposition::Executed(Execution::Pass { run }) => {
                        fields.insert("run".to_owned(), text(run));
                    }
                    Disposition::Executed(Execution::Fail { property, run }) => {
                        fields.insert("detail".to_owned(), text(property));
                        fields.insert("run".to_owned(), text(run));
                    }
                    Disposition::Executed(Execution::Inconclusive { reason }) => {
                        fields.insert("detail".to_owned(), text(reason_token(*reason)));
                    }
                }
                Json::Object(fields)
            })
            .collect();
        let verdict = match self.verdict {
            Verdict::Complete => obj([("status", text("complete"))]),
            Verdict::Pending => obj([("status", text("pending"))]),
            Verdict::Inconclusive { reason } => obj([
                ("reason", text(reason_token(reason))),
                ("status", text("inconclusive")),
            ]),
        };
        obj([
            ("kind", text(RECORD_KIND)),
            ("inputs", self.inputs.0.clone()),
            (
                "core",
                obj([
                    ("events", int(self.core_events)),
                    ("identity", text(&self.core)),
                    (
                        "members",
                        Json::Array(self.core_members.iter().map(|m| int(*m)).collect()),
                    ),
                ]),
            ),
            ("subject", text(&self.subject)),
            ("seed", text(&format!("{:#018x}", self.config.seed))),
            (
                "config",
                obj([
                    (
                        "budget",
                        obj([
                            ("candidates", int(self.config.budget.candidates)),
                            ("runs", int(self.config.budget.runs)),
                            ("work", int(self.config.budget.work)),
                        ]),
                    ),
                    ("max_delay", int(u64::from(self.config.max_delay))),
                    ("transaction", text(&self.config.transaction)),
                    (
                        "selected",
                        Json::Array(
                            selected_of(&self.config)
                                .into_iter()
                                .map(|s| text(s.token()))
                                .collect(),
                        ),
                    ),
                    ("walk_length", int(u64::from(self.config.walk_length))),
                    ("walks", int(u64::from(self.config.walks))),
                ]),
            ),
            ("envelope", self.envelope.clone()),
            ("strategies", Json::Array(strategies)),
            ("neighbors", Json::Array(neighbors)),
            (
                "failing_neighbors",
                Json::Array(self.failing.iter().map(failing_json).collect()),
            ),
            (
                "spent",
                obj([
                    ("candidates", int(self.spent.candidates)),
                    ("runs", int(self.spent.runs)),
                    ("work", int(self.spent.work)),
                ]),
            ),
            ("verdict", verdict),
        ])
    }

    /// The record's canonical bytes (RFC 0037 ID5 canonical JSON).
    #[must_use]
    pub fn canonical_bytes(&self) -> Vec<u8> {
        self.to_json().to_canonical_bytes()
    }

    /// A BLAKE3 digest of [`Self::canonical_bytes`], for indexing only (ADR-0013).
    #[must_use]
    pub fn digest(&self) -> String {
        Blake3Hasher::hash(&self.canonical_bytes()).to_string()
    }

    /// The per-strategy coverage of the strategies that ran.
    #[must_use]
    pub fn coverage(&self, strategy: Strategy) -> Option<&Coverage> {
        self.strategies.iter().find_map(|(s, st)| match st {
            StrategyStatus::Ran(c) if *s == strategy => Some(c),
            _ => None,
        })
    }

    /// The projection onto `promotion-receipt.schema.json`'s `coverage.neighborhood`:
    /// one entry per strategy that ran (`explored` counts executed neighbors, `failing`
    /// equals that strategy's `failing_neighbors` entries), and every failing neighbor
    /// with its run handle. Strategies that did not run are omitted, as the schema
    /// says. No `reused` count appears: nothing here is served from a cache.
    ///
    /// Only a [`Verdict::Complete`] record projects. The schema's shape has no place for
    /// an unsupported, unrealizable, undecided or unnamed neighbor, for a selected
    /// strategy that did not run, or for a pending frontier, so a projection that
    /// would fold any of them into `explored` or drop them is refused instead (INV-007,
    /// INV-008, RFC 0032's "failing neighbors are disclosed"). The record keeps them.
    ///
    /// # Errors
    ///
    /// [`ProjectionRefusal`].
    pub fn receipt_coverage<S: Substrate>(
        &self,
        substrate: &S,
        envelope: &Envelope,
        config: &Config,
    ) -> Result<Json, ProjectionRefusal> {
        // Bound to what it was computed for: the verifier's inputs are derived here,
        // from the live substrate, envelope and configuration, never taken from the
        // caller or the record. Any changed input makes the record stale.
        let current = Inputs::of(substrate, envelope, config).map_err(ProjectionRefusal::Core)?;
        if let Some(field) = self.inputs.first_difference(&current) {
            return Err(ProjectionRefusal::Stale { field });
        }
        // Every count reconciles with the dispositions, and the verdict with both.
        self.reconcile().map_err(ProjectionRefusal::Inconsistent)?;
        // A pending campaign is resumable, never terminal: checked before anything
        // that would type it terminal.
        if self.verdict == Verdict::Pending {
            return Err(ProjectionRefusal::Pending);
        }
        if let Some(f) = self.failing.iter().find(|f| !is_handle(&f.run)) {
            return Err(ProjectionRefusal::UndisclosableRun {
                neighbor: f.neighbor.clone(),
            });
        }
        let undecided: u64 = self
            .strategies
            .iter()
            .filter_map(|(_, st)| match st {
                StrategyStatus::Ran(c) => Some(c.inconclusive),
                StrategyStatus::NotRun(_) => None,
            })
            .try_fold(0_u64, u64::checked_add)
            .ok_or_else(|| ProjectionRefusal::Inconsistent("undecided overflows".to_owned()))?;
        if undecided > 0 {
            return Err(ProjectionRefusal::InconclusiveRuns { count: undecided });
        }
        match self.verdict {
            Verdict::Complete => Ok(self.coverage_json()),
            Verdict::Pending => Err(ProjectionRefusal::Pending),
            Verdict::Inconclusive { reason } => Err(ProjectionRefusal::Inconclusive { reason }),
        }
    }

    /// Whether the record's accounting reconciles: per strategy, every count is the
    /// count of its neighbors' dispositions (generated = rejected + valid, valid =
    /// unsupported + not realizable + executed, executed = passed + failed +
    /// inconclusive, distinct runs = distinct handles); the failing list is exactly the
    /// failing neighbors, in order; the candidate spend is the generated total; and the
    /// verdict is the one the data supports (`Complete` only with a non-empty selection,
    /// every selected strategy run to its end, something executed and nothing
    /// unsupported, unrealizable, undecided or unnamed; `Pending` only with a frontier).
    /// Every sum is checked.
    fn reconcile(&self) -> Result<(), String> {
        let sum = |it: &mut dyn Iterator<Item = u64>| -> Result<u64, String> {
            let mut total = 0_u64;
            for n in it {
                total = total
                    .checked_add(n)
                    .ok_or_else(|| "a count overflows".to_owned())?;
            }
            Ok(total)
        };
        let selected: BTreeSet<Strategy> = self.config.strategies.iter().copied().collect();
        let mut generated_total = 0_u64;
        let mut gap = selected.is_empty();
        let mut any_frontier = false;
        let mut executed_total = 0_u64;
        let mut undecided = 0_u64;
        let mut valid_total = 0_u64;
        let mut not_bijective = false;
        let mut position = 0_usize;
        for (s, st) in &self.strategies {
            let mine: Vec<&Neighbor> = self.neighbors.iter().filter(|n| n.strategy == *s).collect();
            // Neighbors are grouped by strategy, in strategy order.
            for n in &mine {
                if self
                    .neighbors
                    .get(position)
                    .map(|x| (x.strategy, x.id.as_str()))
                    != Some((n.strategy, n.id.as_str()))
                {
                    return Err(format!("{}: neighbors out of order", s.token()));
                }
                position += 1;
            }
            let c = match st {
                StrategyStatus::NotRun(_) => {
                    if !mine.is_empty() {
                        return Err(format!("{}: neighbors of a strategy not run", s.token()));
                    }
                    gap |= selected.contains(s);
                    continue;
                }
                StrategyStatus::Ran(c) => c,
            };
            if !selected.contains(s) {
                return Err(format!("{}: ran without being selected", s.token()));
            }
            let count = |f: &dyn Fn(&Disposition) -> bool| {
                u64::try_from(mine.iter().filter(|n| f(&n.disposition)).count()).unwrap_or(u64::MAX)
            };
            let rejected = count(&|d| matches!(d, Disposition::Rejected(_)));
            let checks = [
                (
                    "generated",
                    u64::try_from(mine.len()).unwrap_or(u64::MAX) == c.generated,
                ),
                (
                    "rejected",
                    sum(&mut c.rejected.values().copied())? == rejected,
                ),
                ("valid", c.generated.checked_sub(rejected) == Some(c.valid)),
                (
                    "unsupported",
                    count(&|d| matches!(d, Disposition::Unsupported(_))) == c.unsupported,
                ),
                (
                    "not-realizable",
                    count(&|d| matches!(d, Disposition::NotRealizable(_)))
                        == sum(&mut c.not_realizable.values().copied())?,
                ),
                (
                    "executed",
                    count(&|d| matches!(d, Disposition::Executed(_))) == c.executed,
                ),
                (
                    "passed",
                    count(&|d| matches!(d, Disposition::Executed(Execution::Pass { .. })))
                        == c.passed,
                ),
                (
                    "failed",
                    count(&|d| matches!(d, Disposition::Executed(Execution::Fail { .. })))
                        == c.failed,
                ),
                (
                    "inconclusive",
                    count(&|d| matches!(d, Disposition::Executed(Execution::Inconclusive { .. })))
                        == c.inconclusive,
                ),
                (
                    "valid-split",
                    sum(&mut [
                        c.unsupported,
                        sum(&mut c.not_realizable.values().copied())?,
                        c.executed,
                    ]
                    .into_iter())?
                        == c.valid,
                ),
                (
                    "distinct-runs",
                    u64::try_from(
                        mine.iter()
                            .filter_map(|n| match &n.disposition {
                                Disposition::Executed(
                                    Execution::Pass { run } | Execution::Fail { run, .. },
                                ) if is_handle(run) => Some(run.as_str()),
                                _ => None,
                            })
                            .collect::<BTreeSet<_>>()
                            .len(),
                    )
                    .unwrap_or(u64::MAX)
                        == c.distinct_runs,
                ),
            ];
            if let Some((what, _)) = checks.iter().find(|(_, ok)| !ok) {
                return Err(format!(
                    "{}: {what} disagrees with the neighbors",
                    s.token()
                ));
            }
            // The rejected map is keyed by the rejections' own tokens.
            for (token, n) in &c.rejected {
                if count(&|d| matches!(d, Disposition::Rejected(r) if r.token() == *token)) != *n {
                    return Err(format!("{}: rejected[{token}] disagrees", s.token()));
                }
            }
            generated_total = generated_total
                .checked_add(c.generated)
                .ok_or_else(|| "generated overflows".to_owned())?;
            executed_total = executed_total
                .checked_add(c.executed)
                .ok_or_else(|| "executed overflows".to_owned())?;
            undecided = undecided
                .checked_add(c.inconclusive)
                .ok_or_else(|| "undecided overflows".to_owned())?;
            any_frontier |= c.frontier.is_some();
            // A selected strategy that produced nothing covered nothing: a gap, not a
            // vacuous completion.
            gap |= c.unsupported > 0 || !c.not_realizable.is_empty() || c.executed == 0;
            // The whole-set bijection: over every non-schedule candidate of the strategy
            // (duplicates included, oversized ones excepted), each identity names one
            // content and each content has one identity; a strategy whose pairs are not
            // a bijection must carry an identity-collision rejection, and cannot be part
            // of a Complete campaign.
            let mut by_id: BTreeMap<&str, &str> = BTreeMap::new();
            let mut by_content: BTreeMap<&str, &str> = BTreeMap::new();
            let mut bijective = true;
            for n in &mine {
                // Non-schedule candidates are the ones with a content address (an
                // identity may be empty; a content address never is).
                if n.content.is_empty()
                    || matches!(
                        n.disposition,
                        Disposition::Rejected(Rejection::Oversized | Rejection::DigestCollision)
                    )
                {
                    continue;
                }
                if *by_id.entry(&n.identity).or_insert(&n.content) != n.content.as_str()
                    || *by_content.entry(&n.content).or_insert(&n.identity) != n.identity.as_str()
                {
                    bijective = false;
                }
            }
            let collided = mine.iter().any(|n| {
                matches!(
                    n.disposition,
                    Disposition::Rejected(Rejection::IdentityCollision)
                )
            });
            if bijective == collided {
                return Err(format!(
                    "{}: the identity-content bijection disagrees with its rejections",
                    s.token()
                ));
            }
            not_bijective |= !bijective;
            // A witness digest exactly on the executed neighbors.
            if mine
                .iter()
                .any(|n| matches!(n.disposition, Disposition::Executed(_)) == n.witness.is_empty())
            {
                return Err(format!(
                    "{}: a witness disagrees with the dispositions",
                    s.token()
                ));
            }
            valid_total = valid_total
                .checked_add(c.valid)
                .ok_or_else(|| "valid overflows".to_owned())?;
            for (why, n) in &c.not_realizable {
                if count(&|d| matches!(d, Disposition::NotRealizable(w) if w == why)) != *n {
                    return Err(format!("{}: not_realizable[{why}] disagrees", s.token()));
                }
            }
        }
        if position != self.neighbors.len() {
            return Err("a neighbor of no strategy".to_owned());
        }
        let fails: Vec<(Strategy, &str, &str, &str)> = self
            .neighbors
            .iter()
            .filter_map(|n| match &n.disposition {
                Disposition::Executed(Execution::Fail { property, run }) => {
                    Some((n.strategy, n.id.as_str(), property.as_str(), run.as_str()))
                }
                _ => None,
            })
            .collect();
        let listed: Vec<(Strategy, &str, &str, &str)> = self
            .failing
            .iter()
            .map(|f| {
                (
                    f.strategy,
                    f.neighbor.as_str(),
                    f.property.as_str(),
                    f.run.as_str(),
                )
            })
            .collect();
        if fails != listed {
            return Err("the failing list disagrees with the neighbors".to_owned());
        }
        if self.spent.candidates != generated_total {
            return Err("the candidate spend disagrees with the generated total".to_owned());
        }
        // Every valid candidate was charged exactly one run.
        if self.spent.runs != valid_total {
            return Err("the run spend disagrees with the valid total".to_owned());
        }
        // The descriptive fields repeat the binding; they must agree with it.
        let bound = |path: &[&str]| -> Option<&Json> {
            let mut j = &self.inputs.0;
            for k in path {
                j = j.as_object()?.get(*k)?;
            }
            Some(j)
        };
        let agree = bound(&["core", "identity"]).and_then(Json::as_str) == Some(self.core.as_str())
            && bound(&["subject"]).and_then(Json::as_str) == Some(self.subject.as_str())
            && bound(&["transaction"]).and_then(Json::as_str)
                == Some(self.config.transaction.as_str())
            && bound(&["envelope"]) == Some(&self.envelope)
            && bound(&["core", "events"]) == Some(&int(self.core_events));
        if !agree {
            return Err("the record disagrees with its own binding".to_owned());
        }
        let unnamed = self.neighbors.iter().any(taints);
        let consistent = match self.verdict {
            Verdict::Complete => {
                !gap && !any_frontier
                    && executed_total > 0
                    && undecided == 0
                    && !unnamed
                    && !not_bijective
            }
            Verdict::Pending => any_frontier,
            Verdict::Inconclusive { .. } => true,
        };
        if !consistent {
            return Err(format!(
                "the verdict {:?} is not the one the data supports",
                self.verdict
            ));
        }
        Ok(())
    }

    fn coverage_json(&self) -> Json {
        let strategies = self
            .strategies
            .iter()
            .filter_map(|(s, st)| match st {
                StrategyStatus::NotRun(_) => None,
                StrategyStatus::Ran(c) => {
                    let failing = self.failing.iter().filter(|f| f.strategy == *s).count();
                    let mut fields = BTreeMap::new();
                    fields.insert("strategy".to_owned(), text(s.token()));
                    // Explored counts distinct runs: neighbors a program realizes as one
                    // run are one exploration, never several.
                    fields.insert("explored".to_owned(), int(c.distinct_runs));
                    fields.insert(
                        "failing".to_owned(),
                        int(u64::try_from(failing).unwrap_or(u64::MAX)),
                    );
                    fields.insert("truncated".to_owned(), Json::Bool(c.frontier.is_some()));
                    if let Some(f) = &c.frontier {
                        fields.insert("frontier".to_owned(), text(f));
                    }
                    Some(Json::Object(fields))
                }
            })
            .collect();
        obj([
            (
                "failing_neighbors",
                Json::Array(
                    self.failing
                        .iter()
                        .map(|f| {
                            obj([
                                ("neighbor", text(&f.neighbor)),
                                ("run", text(&f.run)),
                                ("strategy", text(f.strategy.token())),
                                ("summary", text(&f.property)),
                            ])
                        })
                        .collect(),
                ),
            ),
            ("strategies", Json::Array(strategies)),
        ])
    }
}

fn failing_json(f: &FailingNeighbor) -> Json {
    obj([
        ("neighbor", text(&f.neighbor)),
        ("property", text(&f.property)),
        ("run", text(&f.run)),
        ("strategy", text(f.strategy.token())),
    ])
}

// ---------------------------------------------------------------------------
// the campaign
// ---------------------------------------------------------------------------

/// The facts about the core every charge and check reads, computed once.
struct Facts {
    n: usize,
    edges: u64,
    /// `n * (n - 1) / 2`: one dependence query per pair.
    pairs: u64,
    /// Every hard edge `(predecessor, event)`, for logarithmic lookup.
    hard: BTreeSet<(usize, usize)>,
    /// A digest over the whole relation: hard predecessors, conflicts, membership and
    /// classes. A core with the same identity string and another relation is another
    /// core.
    commitment: String,
    /// The canonical text the commitment is a digest of: the binding compares this,
    /// exactly (ADR-0013).
    relation: String,
    /// The core as it was read and checked.
    frozen: Frozen,
}

impl Facts {
    fn hard_edge(&self, a: usize, b: usize) -> bool {
        self.hard.contains(&(a.min(b), a.max(b)))
    }
}

/// The core, read once: every relation the campaign uses after the core check comes
/// from this snapshot, never from the substrate's core again, so what was charged and
/// checked is what is used and committed to.
struct Frozen {
    preds: Vec<Vec<usize>>,
    /// Each event's hard ancestors (the transitive closure of the hard edges), as a
    /// bitset: hard order is transitive, so a witness is held to the closure, not only
    /// to the direct edges (an absent middle event does not license an inversion).
    ancestors: Vec<Vec<u64>>,
    dependent: BTreeSet<(usize, usize)>,
    core: Vec<bool>,
    class: Vec<EventClass>,
}

impl Frozen {
    /// Whether `a` is a hard ancestor of `e` (the transitive closure).
    fn is_ancestor(&self, a: usize, e: usize) -> bool {
        self.ancestors
            .get(e)
            .and_then(|bits| bits.get(a / 64))
            .is_some_and(|w| w & (1 << (a % 64)) != 0)
    }
}

impl CausalCore for Frozen {
    fn len(&self) -> usize {
        self.preds.len()
    }
    fn hard_predecessors(&self, event: usize) -> &[usize] {
        self.preds.get(event).map_or(&[], Vec::as_slice)
    }
    fn dependent(&self, a: usize, b: usize) -> bool {
        self.dependent.contains(&(a.min(b), a.max(b)))
    }
    fn in_core(&self, event: usize) -> bool {
        self.core.get(event).copied().unwrap_or(false)
    }
    fn class(&self, event: usize) -> EventClass {
        self.class.get(event).copied().unwrap_or(EventClass::Step)
    }
}

fn check_core<C: CausalCore>(core: &C, meter: &mut Meter) -> Result<Facts, CoreError> {
    let n = core.len();
    if n > MAX_CORE_EVENTS {
        return Err(CoreError::TooLarge { len: n });
    }
    let n64 = u64::try_from(n).unwrap_or(u64::MAX);
    meter.work(n64).map_err(|_| CoreError::CoreCheckExhausted)?;
    let mut edges = 0_u64;
    let mut hard = BTreeSet::new();
    let mut preds_all = Vec::with_capacity(n);
    let mut members = Vec::with_capacity(n);
    let mut classes = Vec::with_capacity(n);
    for e in 0..n {
        // Each event is read exactly once.
        let preds = core.hard_predecessors(e);
        let len = u64::try_from(preds.len()).unwrap_or(u64::MAX);
        // Charged before the list is walked, copied and indexed.
        meter.work(len).map_err(|_| CoreError::CoreCheckExhausted)?;
        for &p in preds {
            if p >= e {
                return Err(CoreError::PredecessorNotEarlier {
                    event: e,
                    predecessor: p,
                });
            }
            hard.insert((p, e));
        }
        let mut own = preds.to_vec();
        own.sort_unstable();
        own.dedup();
        preds_all.push(own);
        edges = edges.saturating_add(len);
        members.push(core.in_core(e));
        classes.push(core.class(e));
    }
    if !members.iter().any(|m| *m) {
        return Err(CoreError::NoCoreEvent);
    }
    // Every pair is read once, both ways: charged before the walk.
    let pairs = n64.saturating_mul(n64.saturating_sub(1)) / 2;
    meter
        .work(
            pairs
                .saturating_mul(2)
                .saturating_add(n64)
                .saturating_add(edges),
        )
        .map_err(|_| CoreError::CoreCheckExhausted)?;
    let mut dependent = BTreeSet::new();
    for a in 0..n {
        for b in a + 1..n {
            if core.dependent(a, b) || core.dependent(b, a) {
                dependent.insert((a, b));
            }
        }
    }
    // The closure: one word-wise union per hard edge, charged before it is built.
    let words = n.div_ceil(64);
    meter
        .work(
            edges
                .saturating_add(n64)
                .saturating_mul(u64::try_from(words).unwrap_or(u64::MAX).max(1)),
        )
        .map_err(|_| CoreError::CoreCheckExhausted)?;
    let mut ancestors: Vec<Vec<u64>> = Vec::with_capacity(n);
    for (e, preds) in preds_all.iter().enumerate() {
        let mut bits = vec![0_u64; words];
        for &p in preds {
            bits[p / 64] |= 1 << (p % 64);
            for (w, a) in bits.iter_mut().zip(&ancestors[p]) {
                *w |= *a;
            }
        }
        debug_assert_eq!(ancestors.len(), e);
        ancestors.push(bits);
    }
    let frozen = Frozen {
        ancestors,
        preds: preds_all,
        dependent,
        core: members,
        class: classes,
    };
    // The commitment is over the snapshot: bounded by what was charged.
    let mut text_form = String::new();
    for e in 0..n {
        let class = match frozen.class[e] {
            EventClass::Step => "step".to_owned(),
            EventClass::Send { channel } => format!("send:{channel}"),
            EventClass::Fault(c) => format!("fault:{c}"),
        };
        let conflicts: Vec<usize> = frozen
            .dependent
            .range((e, e + 1)..(e + 1, 0))
            .map(|&(_, b)| b)
            .collect();
        let _ = writeln!(
            text_form,
            "{e} {} {class} hard{:?} conflicts{conflicts:?}",
            u8::from(frozen.core[e]),
            frozen.preds[e]
        );
    }
    // The text is kept in the binding: charged by length before it is.
    meter
        .work(u64::try_from(text_form.len()).unwrap_or(u64::MAX))
        .map_err(|_| CoreError::CoreCheckExhausted)?;
    Ok(Facts {
        n,
        edges,
        pairs,
        hard,
        commitment: Blake3Hasher::hash(text_form.as_bytes()).to_string(),
        relation: text_form,
        frozen,
    })
}

/// Each event's position in `order`, or the first reason `order` is not a schedule the
/// hard order admits.
fn check_schedule<C: CausalCore>(core: &C, order: &[usize]) -> Result<Vec<usize>, Rejection> {
    let n = core.len();
    if order.len() != n {
        return Err(Rejection::MalformedSchedule);
    }
    let mut pos = vec![usize::MAX; n];
    for (k, &e) in order.iter().enumerate() {
        match pos.get_mut(e) {
            Some(slot) if *slot == usize::MAX => *slot = k,
            _ => return Err(Rejection::MalformedSchedule),
        }
    }
    for &e in order {
        for &p in core.hard_predecessors(e) {
            // A predecessor outside the core is a malformed core, never a panic:
            // `check_candidate` does not run the core check first.
            let Some(&pp) = pos.get(p) else {
                return Err(Rejection::MalformedCore { event: e });
            };
            if p >= e {
                return Err(Rejection::MalformedCore { event: e });
            }
            if pp > pos[e] {
                return Err(Rejection::ViolatesCausalOrder {
                    event: e,
                    predecessor: p,
                });
            }
        }
    }
    Ok(pos)
}

/// Checks 1, 2, 4 and 5 of the module documentation on one candidate: a schedule is a
/// permutation of the core's events that respects every hard edge, a message fault
/// names a send of the core, and the profile is well typed against `envelope`. A
/// generic edit's `profile` is what it adds to `core_profile`; a scenario edit's is its
/// whole profile. The trace-class check (3) and the duplicate check (6) need the
/// strategy and the campaign, and are [`explore`]'s. The cost is linear in the core
/// plus its hard edges; [`explore`] charges it before calling this.
///
/// This is the check every generated candidate passes before it can be realized, so a
/// caller can put a hand-built candidate through the same gate.
///
/// # Errors
///
/// The first [`Rejection`].
pub fn check_candidate<C: CausalCore, S>(
    core: &C,
    envelope: &Envelope,
    core_profile: &Profile,
    edit: &Edit<S>,
    profile: &Profile,
) -> Result<(), Rejection> {
    check_edit(core, envelope, core_profile, edit, profile).map(|_| ())
}

fn check_edit<C: CausalCore, S>(
    core: &C,
    envelope: &Envelope,
    core_profile: &Profile,
    edit: &Edit<S>,
    profile: &Profile,
) -> Result<Option<Vec<usize>>, Rejection> {
    let mut pos = None;
    match edit {
        Edit::Schedule(order) => pos = Some(check_schedule(core, order)?),
        Edit::Message { event, fault } => {
            if *event >= core.len() || !matches!(core.class(*event), EventClass::Send { .. }) {
                return Err(Rejection::NotAMessage { event: *event });
            }
            if !matches!(fault, FaultClass::Duplication | FaultClass::Loss) {
                return Err(Rejection::NotAMessage { event: *event });
            }
        }
        Edit::Scenario(_) => {}
    }
    match edit {
        Edit::Scenario(_) => envelope.check(profile)?,
        _ => envelope.check_wide(&merged(core_profile, profile))?,
    }
    Ok(pos)
}

/// The schedule that takes the decision `(j, i)` (`i < j` in the core order, `i` not a
/// hard ancestor of `j`): `j` runs before `i` together with the hard ancestors of `j`
/// between them, in order, and everything else keeps its place. Valid by construction:
/// every hard ancestor of a hoisted event is before `i` or hoisted with it (the closure
/// is transitive), and `i` is no ancestor of a hoisted event (it would then be one of
/// `j`).
fn hoisted(core: &Frozen, i: usize, j: usize) -> Vec<usize> {
    let n = core.len();
    let lifted: Vec<usize> = (i + 1..=j)
        .filter(|&k| k == j || core.is_ancestor(k, j))
        .collect();
    let mut out: Vec<usize> = (0..i).collect();
    out.extend(&lifted);
    out.extend((i..n).filter(|k| !lifted.contains(k)));
    out
}

/// The send `s` delayed past the next `d` events that are not its hard descendants,
/// or `None` when fewer than `d` such events follow it. Valid by construction: a
/// moved event has no hard ancestor at or after `s` other than an earlier moved one (an
/// ancestor that descends from `s` would make it a descendant too). The decision taken
/// is the last moved event before the send.
fn delayed(core: &Frozen, s: usize, d: usize) -> Option<(Vec<usize>, usize)> {
    let n = core.len();
    let past: Vec<usize> = (s + 1..n)
        .filter(|&e| !core.is_ancestor(s, e))
        .take(d)
        .collect();
    let last = *past.last()?;
    if past.len() < d {
        return None;
    }
    let mut out: Vec<usize> = (0..s).collect();
    out.extend(&past);
    out.extend((s..n).filter(|e| !past.contains(e)));
    Some((out, last))
}

fn moved(n: usize, from: usize, to: usize) -> Vec<usize> {
    let mut out: Vec<usize> = (0..n).collect();
    let e = out.remove(from);
    out.insert(to, e);
    out
}

/// One candidate as a generator states it.
struct Cand<S> {
    id: String,
    /// The duplicate-check key of a non-schedule edit: the substrate's own identity of a
    /// scenario edit, or the fault and the send of a message edit.
    key: String,
    edit: Edit<S>,
    /// For a generic candidate, what it adds to the core's profile; for a scenario edit,
    /// its whole profile.
    profile: Profile,
    /// A scenario edit's canonical bytes from the substrate (charged by length before
    /// they are hashed); `None` for a generic edit, whose bytes the engine spells.
    edit_bytes: Option<Vec<u8>>,
    /// The causal decision a schedule candidate was generated around: the pair it
    /// moves, in the order it requests. The realization must take both, in that order.
    decision: Option<(usize, usize)>,
    /// Whether a substrate string of it was past its cap (and replaced by a marker).
    oversized: bool,
}

/// A committed candidate a resume regenerates and compares.
struct Committed {
    id: String,
    commitment: String,
    rejection: Option<Rejection>,
    /// The committed canonical content bytes (shared, never copied): the regenerated
    /// candidate must have exactly these.
    content: Arc<[u8]>,
}

/// A strategy's generation state.
struct StrategyRun {
    strategy: Strategy,
    coverage: Coverage,
    seen_schedules: BTreeSet<Vec<usize>>,
    /// Each non-schedule candidate's substrate identity, bound to the content
    /// commitment it first came with.
    id_to_content: BTreeMap<String, Arc<[u8]>>,
    /// Every content commitment of a non-schedule candidate: the duplicate check is on
    /// content, so two identities for one edit are one neighbor.
    content_to_id: BTreeMap<Arc<[u8]>, String>,
    /// Each content digest with the canonical bytes it first indexed: the exact check.
    by_digest: BTreeMap<String, Arc<[u8]>>,
    seen_runs: BTreeSet<String>,
    /// The enumeration index of the next candidate: committed, replayed or new. Ids
    /// are numbered by it, so a resumed strategy numbers its candidates as the
    /// uninterrupted one did.
    next: u64,
}

struct Campaign<'a, S: Substrate> {
    substrate: &'a mut S,
    envelope: &'a Envelope,
    core_profile: Profile,
    meter: &'a mut Meter,
    facts: Facts,
    neighbors: Vec<Neighbor>,
    failing: Vec<FailingNeighbor>,
    engine_error: bool,
    first_inconclusive: Option<InconclusiveReason>,
    /// The committed candidates of the strategy a resume is regenerating, in order:
    /// each is rebuilt and validated (so the duplicate sets are the ones the
    /// uninterrupted campaign had), compared by identity, commitment and rejection, and
    /// never counted, realized or executed again.
    replay: std::collections::VecDeque<Committed>,
    /// Every committed neighbor's canonical content bytes, in order: one shared
    /// allocation per content, charged once when the candidate was.
    contents: Vec<Arc<[u8]>>,
    /// The function that indexes content (BLAKE3, or a test hook): an index only;
    /// equality is decided on the bytes.
    digest: fn(&[u8]) -> String,
    /// Whether a substrate capped a strategy silently (fewer edits than it declared).
    /// That is terminal: no later strategy runs, and the campaign never suspends, so a
    /// cap is never carried into a continuation.
    capped: bool,
    /// Whether the strategy being regenerated finished before the suspension: a
    /// candidate past its committed ones is a divergence, not new work.
    replay_only: bool,
}

impl<S: Substrate> Campaign<'_, S> {
    fn conflicting(&self, a: usize, b: usize) -> bool {
        let core = &self.facts.frozen;
        core.dependent(a, b) || core.dependent(b, a) || self.facts.hard_edge(a, b)
    }

    /// Predicted work of a candidate's construction and validation, charged before it
    /// is built.
    fn cost(&self, strategy: Strategy, schedule: bool) -> u64 {
        let n = u64::try_from(self.facts.n).unwrap_or(u64::MAX);
        // The candidate's retained canonical content: its profile (bounded) and, for a
        // schedule, its order (charged below with the schedule's walks).
        let mut units = n.saturating_add(1).saturating_add(PROFILE_BYTES);
        if schedule {
            // Position table, the hard-edge walk, and the duplicate set's comparisons.
            units = units
                .saturating_add(n)
                .saturating_add(self.facts.edges)
                .saturating_add(n.saturating_mul(64));
            if strategy == Strategy::SchedulePerturbation {
                // The trace-class check: one conflict query per pair.
                units = units.saturating_add(self.facts.pairs);
            }
        }
        units
    }

    /// The first dependent pair `pos` reverses.
    fn check_class(&self, pos: &[usize]) -> Result<(), Rejection> {
        let n = self.facts.n;
        for a in 0..n {
            for b in a + 1..n {
                if pos[a] > pos[b] && self.conflicting(a, b) {
                    return Err(Rejection::LeavesTraceClass {
                        first: a,
                        second: b,
                    });
                }
            }
        }
        Ok(())
    }

    fn validate(
        &self,
        run: &mut StrategyRun,
        cand: &Cand<S::Scenario>,
        content: &str,
        content_bytes: &Arc<[u8]>,
    ) -> Result<(), Rejection> {
        if cand.oversized {
            return Err(Rejection::Oversized);
        }
        // ADR-0013: the digest indexes, the bytes decide. One digest for two different
        // contents is a typed collision, never an equality.
        if !matches!(cand.edit, Edit::Schedule(_))
            && let Some(b) = run.by_digest.get(content)
            && **b != **content_bytes
        {
            return Err(Rejection::DigestCollision);
        }
        // Identity and content are a bijection within a strategy, and every declared
        // pair takes part: the check runs before any other, so a candidate the envelope
        // later rejects is bound too, exactly as the record's whole-set check
        // (`reconcile`) reads it. The exact pair seen again is a duplicate; any second
        // binding in either direction (one identity naming two contents, or one content
        // under two identities) is a collision, never a duplicate. Whatever the arrival
        // order, a pair set that is not a bijection yields a collision.
        if !matches!(cand.edit, Edit::Schedule(_)) {
            // Both maps are keyed and compared on the canonical bytes: the digest
            // never decides equality or inequality (ADR-0013).
            match (
                run.id_to_content.get(&cand.key),
                run.content_to_id.get(&**content_bytes),
            ) {
                (None, None) => {
                    // One shared allocation: each map holds a pointer, not a copy.
                    run.id_to_content
                        .insert(cand.key.clone(), Arc::clone(content_bytes));
                    run.content_to_id
                        .insert(Arc::clone(content_bytes), cand.key.clone());
                    run.by_digest
                        .entry(content.to_owned())
                        .or_insert_with(|| Arc::clone(content_bytes));
                }
                (Some(c), Some(i)) if **c == **content_bytes && *i == cand.key => {
                    return Err(Rejection::Duplicate);
                }
                _ => return Err(Rejection::IdentityCollision),
            }
        }
        let core = &self.facts.frozen;
        let pos = check_edit(
            core,
            self.envelope,
            &self.core_profile,
            &cand.edit,
            &cand.profile,
        )?;
        if run.strategy == Strategy::SchedulePerturbation
            && let Some(pos) = pos
        {
            self.check_class(&pos)?;
        }
        if let Edit::Schedule(order) = &cand.edit
            && (order.iter().enumerate().all(|(k, &e)| k == e)
                || !run.seen_schedules.insert(order.clone()))
        {
            return Err(Rejection::Duplicate);
        }
        Ok(())
    }

    /// Charge, validate, realize and execute one candidate. `Err` when the budget cannot
    /// pay for the candidate or its run; that candidate is then not counted, so the
    /// strategy's frontier is its index and a resume offers it again.
    fn offer(
        &mut self,
        run: &mut StrategyRun,
        build: impl FnOnce() -> Cand<S::Scenario>,
        schedule: bool,
    ) -> Result<(), Stop> {
        let cost = self.cost(run.strategy, schedule);
        if self.replay.is_empty() && self.replay_only {
            return Err(Stop::Mismatch);
        }
        if self.replay.is_empty() {
            self.meter.candidate(cost)?;
        } else {
            // A committed candidate was counted when it was first generated; its
            // regeneration is work only.
            self.meter.work(cost)?;
        }
        let cand = build();
        self.process(run, cand)
    }

    fn process(&mut self, run: &mut StrategyRun, cand: Cand<S::Scenario>) -> Result<(), Stop> {
        // Built once and shared by every holder; its length was charged with the
        // candidate (`cost`, and the scenario edit's admission).
        let content_bytes: Arc<[u8]> = Arc::from(self.content_bytes(&cand));
        let content = (self.digest)(&content_bytes);
        let commitment = self.commit_with(run.strategy, &cand, &content);
        if let Some(expected) = self.replay.pop_front() {
            // Rebuild the duplicate sets exactly as the first pass left them, and hold
            // the regenerated candidate to the committed one: same identity, the same
            // content byte for byte (the commitment only indexes), same validity.
            let validity = self.validate(run, &cand, &content, &content_bytes).err();
            let same = cand.id == expected.id
                && *content_bytes == *expected.content
                && commitment == expected.commitment
                && validity == expected.rejection;
            return if same {
                run.next += 1;
                Ok(())
            } else {
                Err(Stop::Mismatch)
            };
        }
        let (disposition, witness) = match self.validate(run, &cand, &content, &content_bytes) {
            Err(r) => {
                if engine_refusal(run.strategy, &r) {
                    // The engine could not decide a candidate that may be a neighbor
                    // the intent permits: a coverage gap, terminal for the verdict and
                    // disclosed in the record.
                    self.engine_error = true;
                }
                *run.coverage.rejected.entry(r.token()).or_default() += 1;
                (Disposition::Rejected(r), String::new())
            }
            Ok(()) => {
                // The run and the storage of its handle (at most `MAX_HANDLE_BYTES`)
                // are charged together, before the substrate is asked.
                // The witness check (one dependence query per pair, plus the core) is
                // charged here too.
                let verify = self
                    .facts
                    .pairs
                    .saturating_mul(2)
                    .saturating_add(self.facts.edges)
                    .saturating_add(
                        u64::try_from(self.facts.n)
                            .unwrap_or(u64::MAX)
                            .saturating_mul(3),
                    )
                    // The witness text kept: at most 8 bytes per core event, plus its frame.
                    .saturating_add(
                        u64::try_from(self.facts.n)
                            .unwrap_or(u64::MAX)
                            .saturating_mul(8)
                            .saturating_add(64),
                    );
                if let Err(e) = self
                    .meter
                    .work(
                        // Every copy kept: the handle in the disposition, the failing
                        // list and the distinct-run set; the property or reason in the
                        // disposition and the failing list or the reason map.
                        MAX_HANDLE_BYTES
                            .saturating_mul(3)
                            .saturating_add(MAX_OUTCOME_BYTES.saturating_mul(2))
                            .saturating_add(verify),
                    )
                    .and_then(|()| self.meter.run())
                {
                    // Valid, but no run is left to pay for it: take it back out of the
                    // counts and the duplicate sets. The work spent building and
                    // checking it stays spent.
                    match &cand.edit {
                        Edit::Schedule(order) => {
                            run.seen_schedules.remove(order);
                        }
                        _ => {
                            // Both bindings, and the digest's bytes, were made by this
                            // candidate.
                            run.id_to_content.remove(&cand.key);
                            run.content_to_id.remove(&*content_bytes);
                            if run
                                .by_digest
                                .get(&content)
                                .is_some_and(|b| **b == *content_bytes)
                            {
                                run.by_digest.remove(&content);
                            }
                        }
                    }
                    self.meter.return_candidate();
                    return Err(e.into());
                }
                run.coverage.valid += 1;
                self.dispose(run, &cand)
            }
        };
        run.coverage.generated += 1;
        run.next += 1;
        self.neighbors.push(Neighbor {
            strategy: run.strategy,
            id: cand.id,
            disposition,
            commitment,
            witness,
            // The neighbor's content bytes are kept (in memory) for an exact replay.
            identity: if matches!(cand.edit, Edit::Schedule(_)) {
                String::new()
            } else {
                cand.key
            },
            content: if matches!(cand.edit, Edit::Schedule(_)) {
                String::new()
            } else {
                content
            },
        });
        self.contents.push(content_bytes);
        Ok(())
    }

    /// The candidate's content commitment. A generic edit's bytes are the engine's own
    /// spelling (bounded by the core, already charged with the candidate); a scenario
    /// edit's are the substrate's, charged by length in [`Self::scenarios`].
    fn commit_with(&self, strategy: Strategy, cand: &Cand<S::Scenario>, content: &str) -> String {
        let mut bytes = Vec::new();
        for part in [
            strategy.token().as_bytes(),
            cand.id.as_bytes(),
            content.as_bytes(),
        ] {
            bytes.extend_from_slice(part);
            bytes.push(0);
        }
        Blake3Hasher::hash(&bytes).to_string()
    }

    /// The candidate's canonical content: its edit's canonical bytes and its profile.
    /// Equality of content is decided on these bytes (ADR-0013); its digest only
    /// indexes.
    fn content_bytes(&self, cand: &Cand<S::Scenario>) -> Vec<u8> {
        let mut bytes = Vec::new();
        match (&cand.edit, &cand.edit_bytes) {
            (Edit::Schedule(order), _) => {
                bytes.extend_from_slice(b"schedule");
                for e in order {
                    bytes.extend_from_slice(&u64::try_from(*e).unwrap_or(u64::MAX).to_be_bytes());
                }
            }
            (Edit::Message { event, fault }, _) => {
                bytes.extend_from_slice(b"message");
                bytes.extend_from_slice(&u64::try_from(*event).unwrap_or(u64::MAX).to_be_bytes());
                bytes.extend_from_slice(fault.wire().as_bytes());
            }
            (Edit::Scenario(_), Some(b)) => {
                bytes.extend_from_slice(b"scenario");
                bytes.extend_from_slice(&u64::try_from(b.len()).unwrap_or(u64::MAX).to_be_bytes());
                bytes.extend_from_slice(b);
            }
            (Edit::Scenario(_), None) => bytes.extend_from_slice(b"scenario?"),
        }
        bytes.push(0);
        bytes.extend_from_slice(&cand.profile.to_json().to_canonical_bytes());
        bytes
    }

    fn dispose(
        &mut self,
        run: &mut StrategyRun,
        cand: &Cand<S::Scenario>,
    ) -> (Disposition, String) {
        match self.substrate.realize(&cand.edit) {
            Realized::NotRealizable(why) => {
                let why = self.bounded(why);
                *run.coverage.not_realizable.entry(why.clone()).or_default() += 1;
                (Disposition::NotRealizable(why), String::new())
            }
            Realized::Unsupported(why) => {
                let why = self.bounded(why);
                run.coverage.unsupported += 1;
                (Disposition::Unsupported(why), String::new())
            }
            Realized::Run { run: r, witness } => {
                // The library's causal-neighbor check, before the run is executed as
                // this neighbor (its cost was charged with the run).
                if let Err(why) = self.check_witness(cand, &witness) {
                    *run.coverage.not_realizable.entry(why.clone()).or_default() += 1;
                    return (Disposition::NotRealizable(why), String::new());
                }
                let witness_digest = witness_text(&witness);
                let ex = self.substrate.execute(&r);
                if matches!(witness, Witness::Schedule { halted: true, .. })
                    && !matches!(ex, Execution::Fail { .. })
                {
                    // A halted run is the neighbor only as a failure: a run that stopped
                    // early and did not fail proved nothing about the neighbor. Decided
                    // before any outcome string is admitted: none of them is kept.
                    let why = "witness: halted run did not fail".to_owned();
                    *run.coverage.not_realizable.entry(why.clone()).or_default() += 1;
                    return (Disposition::NotRealizable(why), String::new());
                }
                run.coverage.executed += 1;
                // Outcome strings are bounded before they are kept: an oversized one is
                // replaced by a typed marker, and the outcome keeps its kind.
                let ex = match ex {
                    Execution::Pass { run: h } => Execution::Pass {
                        run: self.bounded_handle(h),
                    },
                    Execution::Fail { property, run: h } => Execution::Fail {
                        property: self.bounded(property),
                        run: self.bounded_handle(h),
                    },
                    other @ Execution::Inconclusive { .. } => other,
                };
                if let Execution::Pass { run: h } | Execution::Fail { run: h, .. } = &ex {
                    if is_handle(h) {
                        // Paid for by the handle reserve charged with the run.
                        if run.seen_runs.insert(h.clone()) {
                            run.coverage.distinct_runs += 1;
                        }
                    } else {
                        // A run the receipt cannot name by handle cannot be disclosed:
                        // it stays in the record as it came out, the campaign is
                        // inconclusive, and the receipt projection is refused.
                        self.engine_error = true;
                    }
                }
                match &ex {
                    Execution::Pass { .. } => run.coverage.passed += 1,
                    Execution::Fail { property, run: h } => {
                        run.coverage.failed += 1;
                        self.failing.push(FailingNeighbor {
                            strategy: run.strategy,
                            neighbor: cand.id.clone(),
                            property: property.clone(),
                            run: h.clone(),
                        });
                    }
                    Execution::Inconclusive { reason } => {
                        run.coverage.inconclusive += 1;
                        self.first_inconclusive.get_or_insert(*reason);
                    }
                }
                (Disposition::Executed(ex), witness_digest)
            }
        }
    }

    /// `s` if it fits [`MAX_OUTCOME_BYTES`]; otherwise the typed marker, and the
    /// campaign becomes an engine error. The length is read, the bytes never copied.
    fn bounded(&mut self, s: String) -> String {
        self.admit_outcome(s, MAX_OUTCOME_BYTES)
    }

    /// An outcome string through [`admit`], prepaid by the run reserve: kept, or
    /// replaced by the marker with the campaign an engine error.
    fn admit_outcome(&mut self, s: String, cap: u64) -> String {
        match admit(self.meter, s, cap, true) {
            Admitted::Kept(s) => s,
            Admitted::Oversized(len) => {
                self.engine_error = true;
                oversized_marker(len)
            }
            // Prepaid admission never charges; kept for totality.
            Admitted::Exhausted => {
                self.engine_error = true;
                oversized_marker(0)
            }
        }
    }

    /// A run handle, bounded as [`Self::bounded`] with [`MAX_HANDLE_BYTES`]. An
    /// oversized handle is not a handle; the marker keeps it one that is never valid.
    fn bounded_handle(&mut self, h: String) -> String {
        self.admit_outcome(h, MAX_HANDLE_BYTES)
    }

    /// The causal-neighbor definition (see [`Witness`]), checked on what the substrate
    /// says the run did. Linear in the core plus one dependence query per pair of taken
    /// events, charged with the run.
    fn check_witness(&self, cand: &Cand<S::Scenario>, witness: &Witness) -> Result<(), String> {
        let bad = |why: &str| Err(format!("witness: {why}"));
        match (&cand.edit, witness) {
            (
                Edit::Schedule(order),
                Witness::Schedule {
                    taken,
                    absent,
                    halted,
                },
            ) => {
                let n = self.facts.n;
                if taken.len().checked_add(absent.len()).is_none_or(|t| t > n) {
                    return bad("more events than the core has");
                }
                let mut seen = vec![false; n];
                for &e in taken.iter().chain(absent) {
                    match seen.get_mut(e) {
                        Some(slot) if !*slot => *slot = true,
                        _ => return bad("an event twice or out of range"),
                    }
                }
                if !halted && seen.iter().any(|x| !x) {
                    return bad("an event neither taken nor absent");
                }
                let mut want = vec![usize::MAX; n];
                for (k, &e) in order.iter().enumerate() {
                    want[e] = k;
                }
                // No hard edge of the core may be inverted by what the run took: a
                // program-order or message edge no schedule of the core can reverse.
                // (The requested order respects every one: the generator refuses any
                // candidate that does not.)
                // Held to the transitive closure: every hard ancestor of a taken event
                // was taken before it or is absent from the program. That refuses an
                // event taken after one of its hard descendants, and (in a halted run)
                // an event taken while a hard ancestor the program has is still pending.
                let frozen = &self.facts.frozen;
                let words = n.div_ceil(64);
                let mut done = vec![0_u64; words];
                for &e in absent {
                    done[e / 64] |= 1 << (e % 64);
                }
                for &e in taken {
                    if frozen.ancestors[e]
                        .iter()
                        .zip(&done)
                        .any(|(anc, ok)| anc & !ok != 0)
                    {
                        return bad("a hard edge inverted");
                    }
                    done[e / 64] |= 1 << (e % 64);
                }
                let core = &self.facts.frozen;
                for (i, &a) in taken.iter().enumerate() {
                    for &b in &taken[i + 1..] {
                        if (core.dependent(a, b) || core.dependent(b, a)) && want[a] > want[b] {
                            return bad("a dependent pair in another order");
                        }
                    }
                }
                if let Some((first, second)) = cand.decision {
                    let at = |e: usize| taken.iter().position(|x| *x == e);
                    match (at(first), at(second)) {
                        (Some(x), Some(y)) if x < y => {}
                        _ => return bad("the causal decision was not taken as requested"),
                    }
                }
                Ok(())
            }
            (_, Witness::Edit(bytes))
                if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > MAX_EDIT_BYTES =>
            {
                bad("an oversized edit echo")
            }
            (Edit::Scenario(_), Witness::Edit(bytes)) => {
                if cand.edit_bytes.as_deref() == Some(bytes.as_slice()) {
                    Ok(())
                } else {
                    bad("another scenario ran")
                }
            }
            (Edit::Message { event, fault }, Witness::Edit(bytes)) => {
                if *bytes == message_bytes(*event, *fault) {
                    Ok(())
                } else {
                    bad("another message fault ran")
                }
            }
            _ => bad("of the wrong kind"),
        }
    }

    fn schedule_perturbation(
        &mut self,
        run: &mut StrategyRun,
        config: &Config,
    ) -> Result<(), Stop> {
        let n = self.facts.n;
        let token = run.strategy.token();
        // Adjacent transpositions touching the core.
        for i in 0..n.saturating_sub(1) {
            self.meter.work(1)?;
            let core = &self.facts.frozen;
            if !(core.in_core(i) || core.in_core(i + 1)) {
                continue;
            }
            let k = run.next;
            self.offer(
                run,
                || Cand {
                    id: format!("{token}#{k}:swap({i},{})", i + 1),
                    key: String::new(),
                    edit: Edit::Schedule(moved(n, i, i + 1)),
                    profile: Profile::default(),
                    edit_bytes: None,
                    decision: Some((i + 1, i)),
                    oversized: false,
                },
                true,
            )?;
        }
        // Seeded walks of independent adjacent swaps that touch the core.
        let step = u64::try_from(n).unwrap_or(u64::MAX).saturating_mul(2);
        for w in 0..config.walks {
            let mut rng = SplitMix(
                config.seed ^ (run.strategy.index() << 56) ^ (u64::from(w) << 24) ^ 0x5eed,
            );
            let mut order: Vec<usize> = (0..n).collect();
            for _ in 0..config.walk_length {
                self.meter.work(step)?;
                let core = &self.facts.frozen;
                let options: Vec<usize> = (0..n.saturating_sub(1))
                    .filter(|&p| {
                        let (a, b) = (order[p], order[p + 1]);
                        (core.in_core(a) || core.in_core(b)) && !self.conflicting(a, b)
                    })
                    .collect();
                if options.is_empty() {
                    break;
                }
                let p = options[rng.below(options.len())];
                order.swap(p, p + 1);
            }
            let k = run.next;
            self.offer(
                run,
                || Cand {
                    id: format!("{token}#{k}:walk({w})"),
                    key: String::new(),
                    edit: Edit::Schedule(order),
                    profile: Profile::default(),
                    edit_bytes: None,
                    decision: None,
                    oversized: false,
                },
                true,
            )?;
        }
        Ok(())
    }

    fn alternate_enabled(&mut self, run: &mut StrategyRun) -> Result<(), Stop> {
        let n = self.facts.n;
        let token = run.strategy.token();
        for i in 0..n {
            for j in i + 1..n {
                self.meter.work(1)?;
                let core = &self.facts.frozen;
                // A causal decision: `i` and `j` conflict and `i` is no hard ancestor of
                // `j` (directly or through others), so another schedule could have run
                // `j` first: the one that hoists `j` with its own ancestors.
                if !(core.in_core(i) || core.in_core(j))
                    || !(core.dependent(i, j) || core.dependent(j, i))
                    || core.is_ancestor(i, j)
                {
                    continue;
                }
                let schedule = hoisted(core, i, j);
                let k = run.next;
                self.offer(
                    run,
                    || Cand {
                        id: format!("{token}#{k}:hoist({j} before {i})"),
                        key: String::new(),
                        edit: Edit::Schedule(schedule),
                        profile: Profile::default(),
                        edit_bytes: None,
                        decision: Some((j, i)),
                        oversized: false,
                    },
                    true,
                )?;
            }
        }
        Ok(())
    }

    fn messages(&mut self, run: &mut StrategyRun, config: &Config) -> Result<(), Stop> {
        let n = self.facts.n;
        let token = run.strategy.token();
        let max_delay = usize::try_from(config.max_delay).unwrap_or(usize::MAX);
        for s in 0..n {
            self.meter.work(1)?;
            let core = &self.facts.frozen;
            if !core.in_core(s) || !matches!(core.class(s), EventClass::Send { .. }) {
                continue;
            }
            for d in 1..=max_delay.min(n - 1 - s) {
                // Past the next `d` events that do not descend from the send; none left
                // means no longer delay exists.
                let Some((schedule, last)) = delayed(&self.facts.frozen, s, d) else {
                    break;
                };
                let k = run.next;
                self.offer(
                    run,
                    || Cand {
                        id: format!("{token}#{k}:delay({s},+{d})"),
                        key: String::new(),
                        edit: Edit::Schedule(schedule),
                        profile: Profile::single(FaultClass::Delay),
                        edit_bytes: None,
                        decision: Some((last, s)),
                        oversized: false,
                    },
                    true,
                )?;
            }
            for (fault, verb) in [
                (FaultClass::Duplication, "duplicate"),
                (FaultClass::Loss, "lose"),
            ] {
                let k = run.next;
                self.offer(
                    run,
                    || Cand {
                        id: format!("{token}#{k}:{verb}({s})"),
                        key: format!("{fault}:{s}"),
                        edit: Edit::Message { event: s, fault },
                        profile: Profile::single(fault),
                        edit_bytes: None,
                        decision: None,
                        oversized: false,
                    },
                    false,
                )?;
            }
        }
        Ok(())
    }

    fn scenarios(&mut self, run: &mut StrategyRun) -> Result<(), Stop> {
        let count = self.substrate.scenario_count(run.strategy);
        let token = run.strategy.token();
        for index in 0..count {
            if self.replay.is_empty() && self.replay_only {
                return Err(Stop::Mismatch);
            }
            // Charged before the substrate builds the edit.
            let cost = self.cost(run.strategy, false);
            let replaying = !self.replay.is_empty();
            if replaying {
                self.meter.work(cost)?;
            } else {
                self.meter.candidate(cost)?;
            }
            let Some((id, edit, profile)) = self.substrate.scenario(run.strategy, index) else {
                // The substrate declared more edits than it gave: a silent cap is not
                // allowed (INV-007). No candidate was built; the strategy is disclosed
                // at this frontier and the campaign is inconclusive.
                if !replaying {
                    self.meter.return_candidate();
                }
                self.engine_error = true;
                self.capped = true;
                run.coverage.frontier = Some(format!("{token}#{index}"));
                break;
            };
            // The substrate's identity and edit spelling are admitted (capped, then
            // charged by length) before they are kept or hashed. If the budget cannot
            // pay, no candidate was built: the candidate charge is taken back, so the
            // spend matches the committed candidates.
            // The identity is kept in the neighbor's id, its identity field, the
            // failing list and the two binding maps.
            let id = admit_copies(self.meter, id, MAX_SCENARIO_ID_BYTES, 5);
            let edit_bytes = match id {
                Admitted::Exhausted => Admitted::Exhausted,
                // Kept twice: in the candidate, and in its shared canonical content.
                _ => admit_copies(
                    self.meter,
                    self.substrate.scenario_bytes(&edit),
                    MAX_EDIT_BYTES,
                    2,
                ),
            };
            let (id, edit_bytes, oversized) = match (id, edit_bytes) {
                (Admitted::Exhausted, _) | (_, Admitted::Exhausted) => {
                    if !replaying {
                        self.meter.return_candidate();
                    }
                    return Err(Stop::Exhausted);
                }
                (Admitted::Kept(id), Admitted::Kept(b)) => (id, b, false),
                (id, b) => {
                    // Past a cap: nothing of it is kept. The candidate is recorded as an
                    // oversized rejection under a marker, and the campaign is an engine
                    // error.
                    let marker = oversized_marker(oversized_len(&id).max(oversized_len(&b)));
                    (marker.clone(), marker.into_bytes(), true)
                }
            };
            let cand = Cand {
                id: format!("{token}#{index}:{id}"),
                key: id,
                edit: Edit::Scenario(edit),
                profile,
                edit_bytes: Some(edit_bytes),
                decision: None,
                oversized,
            };
            self.process(run, cand)?;
        }
        Ok(())
    }
}

/// A generic candidate's own profile added to the core's, in `u64`: two `u32` counts
/// always sum exactly, so the composition is never clamped and never overflows.
fn merged(core: &Profile, own: &Profile) -> Wide {
    let mut out = core.wide();
    for (&class, &count) in &own.faults {
        // Cannot overflow: at most u32::MAX + u32::MAX.
        *out.faults.entry(class).or_default() += u64::from(count);
    }
    out.cancellations += u64::from(own.cancellations);
    out.max_value = out.max_value.max(own.max_value);
    out.nodes = out.nodes.max(u64::from(own.nodes));
    out
}

/// The selected strategies, deduplicated, in [`Strategy::ALL`] order.
fn selected_of(config: &Config) -> Vec<Strategy> {
    let set: BTreeSet<Strategy> = config.strategies.iter().copied().collect();
    Strategy::ALL
        .into_iter()
        .filter(|s| set.contains(s))
        .collect()
}

/// A campaign's outcome.
// The finished record is moved by value to its caller; boxing it would add an
// allocation to every campaign for no retained-memory gain.
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Exploration {
    /// The campaign ran every selected strategy to its end (whatever its verdict).
    Finished(NeighborhoodRecord),
    /// The budget ran out. The record's verdict is [`Verdict::Pending`], its truncated
    /// strategies disclose their frontiers, and it is never projected onto a receipt.
    /// [`resume`] continues from the continuation with a larger budget.
    Suspended {
        /// What was committed so far: shared with the continuation, never copied.
        record: Arc<NeighborhoodRecord>,
        /// Where to continue.
        continuation: Box<Continuation>,
    },
}

impl Exploration {
    /// The record, finished or pending.
    #[must_use]
    pub fn record(&self) -> &NeighborhoodRecord {
        match self {
            Self::Finished(r) => r,
            Self::Suspended { record, .. } => record,
        }
    }

    /// The record, finished or pending.
    #[must_use]
    pub fn into_record(self) -> NeighborhoodRecord {
        match self {
            Self::Finished(r) => r,
            Self::Suspended {
                record,
                continuation,
            } => {
                // The continuation's share is dropped first, so the record is moved out,
                // not copied.
                drop(continuation);
                Arc::unwrap_or_clone(record)
            }
        }
    }
}

/// A suspended campaign's committed state: its pending record and its frontier.
///
/// # Trust
///
/// A `Continuation` value has no public constructor: it exists only as
/// [`explore`]'s or [`resume`]'s output in this process, so [`resume`] can carry its
/// committed outcomes without re-running them. It is bound to its [`Inputs`], and
/// [`resume`] refuses it under any other inputs. Resuming regenerates every committed
/// candidate (work only) and holds it to its recorded identity, commitment and
/// validity, then continues at the frontier; a campaign resumed to its end has the same
/// [`NeighborhoodRecord::result_bytes`] as the uninterrupted one.
///
/// Its canonical bytes (`continuum.repair.neighborhood.continuation/v1`) are for
/// storage and disclosure. Bytes are never trusted for an outcome: [`resume_untrusted`]
/// reads them only to check them against a fresh derivation. Carrying a continuation
/// across a process restart without re-running it needs an authenticated store (plan
/// §4.4's `cont_*` artifact, held and signed by the daemon), which is not here.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Continuation {
    record: Arc<NeighborhoodRecord>,
    strategy: Strategy,
    index: u64,
    /// Each committed neighbor's canonical content bytes, in the record's order: what a
    /// resume holds a regenerated candidate to, exactly (ADR-0013). Shared with the
    /// campaign that made them (one allocation per content, charged once). In memory
    /// only; the serialized form carries digests, and bytes from outside are re-derived.
    contents: Vec<Arc<[u8]>>,
}

/// The continuation's kind token.
pub const CONTINUATION_KIND: &str = "continuum.repair.neighborhood.continuation/v1";

/// Why a continuation cannot be resumed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResumeError {
    /// The bytes are not a continuation: where the decode stopped.
    Malformed(String),
    /// The continuation belongs to other inputs (another transaction, candidate,
    /// engine, core, envelope or configuration): the field that differs.
    Mismatch(String),
    /// Regenerating the committed candidates did not reproduce them: an identity,
    /// commitment or validity differs, or a finished strategy has more candidates now.
    Diverged,
    /// Untrusted bytes claim an outcome a fresh derivation does not give: the first
    /// neighbor that differs.
    Forged {
        /// The neighbor.
        neighbor: String,
    },
    /// The budget cannot pay to read the continuation.
    Exhausted,
    /// The core or the budget is refused, as for [`explore`].
    Core(CoreError),
}

impl Continuation {
    /// The canonical content bytes the continuation keeps, counting each allocation
    /// once (contents are shared, never copied), and how many allocations that is.
    /// For tests of the retention bound.
    #[doc(hidden)]
    #[must_use]
    pub fn retained_content(&self) -> (u64, usize) {
        let mut seen = BTreeSet::new();
        // Summed wide: a u128 of at most usize::MAX lengths cannot overflow.
        let mut bytes = 0_u128;
        for c in &self.contents {
            if seen.insert(Arc::as_ptr(c).cast::<u8>() as usize) {
                bytes += c.len() as u128;
            }
        }
        (u64::try_from(bytes).unwrap_or(u64::MAX), seen.len())
    }

    /// The frontier: the strategy and the candidate index it resumes at.
    #[must_use]
    pub const fn frontier(&self) -> (Strategy, u64) {
        (self.strategy, self.index)
    }

    /// The canonical bytes.
    #[must_use]
    pub fn canonical_bytes(&self) -> Vec<u8> {
        obj([
            ("kind", text(CONTINUATION_KIND)),
            (
                "frontier",
                obj([
                    ("index", int(self.index)),
                    ("strategy", text(self.strategy.token())),
                ]),
            ),
            ("record", self.record.to_json()),
        ])
        .to_canonical_bytes()
    }

    /// Read a continuation back, for [`resume_untrusted`] only: the caller charges its
    /// length first. The result is never resumed from; it is compared with a fresh
    /// derivation.
    ///
    /// # Errors
    ///
    /// [`ResumeError::Malformed`] for anything that is not a continuation this module
    /// wrote: bad JSON, a missing or mistyped field, an unknown token, a frontier that
    /// is not a truncated strategy of the record, counts that do not reconcile.
    fn decode(bytes: &[u8]) -> Result<Self, ResumeError> {
        let json = Json::parse(bytes).map_err(|e| ResumeError::Malformed(format!("{e:?}")))?;
        let o = json_obj(&json, "continuation")?;
        if json_str(o, "kind")? != CONTINUATION_KIND {
            return Err(bad("kind"));
        }
        let f = json_obj(field(o, "frontier")?, "frontier")?;
        let strategy =
            Strategy::from_token(json_str(f, "strategy")?).ok_or_else(|| bad("strategy"))?;
        let index = json_u64(f, "index")?;
        let record = NeighborhoodRecord::from_json(field(o, "record")?)?;
        let frontier_ok = record.verdict == Verdict::Pending
            && record.strategies.iter().any(|(s, st)| {
                *s == strategy
                    && matches!(st, StrategyStatus::Ran(c)
                        if c.generated == index
                            && c.frontier.as_deref()
                                == Some(format!("{}#{index}", strategy.token()).as_str()))
            });
        if !frontier_ok {
            return Err(bad("frontier"));
        }
        record.check_consistent()?;
        Ok(Self {
            record: Arc::new(record),
            strategy,
            index,
            // Never used: bytes read from outside are only compared with a fresh
            // derivation, which has its own.
            contents: Vec::new(),
        })
    }
}

fn bad(what: &str) -> ResumeError {
    ResumeError::Malformed(what.to_owned())
}

fn field<'a>(o: &'a BTreeMap<String, Json>, k: &str) -> Result<&'a Json, ResumeError> {
    o.get(k).ok_or_else(|| bad(k))
}

fn json_obj<'a>(j: &'a Json, what: &str) -> Result<&'a BTreeMap<String, Json>, ResumeError> {
    j.as_object().ok_or_else(|| bad(what))
}

fn json_str<'a>(o: &'a BTreeMap<String, Json>, k: &str) -> Result<&'a str, ResumeError> {
    field(o, k)?.as_str().ok_or_else(|| bad(k))
}

fn json_u64(o: &BTreeMap<String, Json>, k: &str) -> Result<u64, ResumeError> {
    match field(o, k)? {
        Json::Integer(n) => u64::try_from(*n).map_err(|_| bad(k)),
        _ => Err(bad(k)),
    }
}

fn json_u32(o: &BTreeMap<String, Json>, k: &str) -> Result<u32, ResumeError> {
    u32::try_from(json_u64(o, k)?).map_err(|_| bad(k))
}

fn json_usize(o: &BTreeMap<String, Json>, k: &str) -> Result<usize, ResumeError> {
    usize::try_from(json_u64(o, k)?).map_err(|_| bad(k))
}

fn json_arr<'a>(o: &'a BTreeMap<String, Json>, k: &str) -> Result<&'a [Json], ResumeError> {
    field(o, k)?.as_array().ok_or_else(|| bad(k))
}

/// Every rejection token, for decoding the counts map.
const REJECTION_TOKENS: [&str; 14] = [
    "digest-collision",
    "oversized",
    "identity-collision",
    "malformed-schedule",
    "violates-causal-order",
    "leaves-trace-class",
    "not-a-message",
    "outside-fault-model",
    "exceeds-fault-bound",
    "bound-undeclared",
    "outside-value-domain",
    "outside-node-domain",
    "duplicate",
    "malformed-core",
];

impl Rejection {
    /// The rejection as a structured JSON object.
    #[must_use]
    pub fn to_json(&self) -> Json {
        let u = |n: usize| int(u64::try_from(n).unwrap_or(u64::MAX));
        let mut f = BTreeMap::new();
        f.insert("kind".to_owned(), text(self.token()));
        match self {
            Self::ViolatesCausalOrder { event, predecessor } => {
                f.insert("event".to_owned(), u(*event));
                f.insert("predecessor".to_owned(), u(*predecessor));
            }
            Self::LeavesTraceClass { first, second } => {
                f.insert("first".to_owned(), u(*first));
                f.insert("second".to_owned(), u(*second));
            }
            Self::NotAMessage { event } | Self::MalformedCore { event } => {
                f.insert("event".to_owned(), u(*event));
            }
            Self::OutsideFaultModel { class } => {
                f.insert("class".to_owned(), text(class.wire()));
            }
            Self::ExceedsFaultBound { what, count, bound } => {
                f.insert("what".to_owned(), text(what.token()));
                f.insert("count".to_owned(), int(*count));
                f.insert("bound".to_owned(), int(*bound));
            }
            Self::BoundUndeclared { component } => {
                f.insert("component".to_owned(), text(component));
            }
            Self::OutsideValueDomain { value, bound } => {
                f.insert("value".to_owned(), int(*value));
                f.insert("bound".to_owned(), int(*bound));
            }
            Self::OutsideNodeDomain { nodes, bound } => {
                f.insert("nodes".to_owned(), int(*nodes));
                f.insert("bound".to_owned(), int(*bound));
            }
            Self::MalformedSchedule
            | Self::Duplicate
            | Self::IdentityCollision
            | Self::Oversized
            | Self::DigestCollision => {}
        }
        Json::Object(f)
    }

    fn from_json(j: &Json) -> Result<Self, ResumeError> {
        let o = json_obj(j, "rejection")?;
        Ok(match json_str(o, "kind")? {
            "malformed-schedule" => Self::MalformedSchedule,
            "duplicate" => Self::Duplicate,
            "identity-collision" => Self::IdentityCollision,
            "oversized" => Self::Oversized,
            "digest-collision" => Self::DigestCollision,
            "violates-causal-order" => Self::ViolatesCausalOrder {
                event: json_usize(o, "event")?,
                predecessor: json_usize(o, "predecessor")?,
            },
            "leaves-trace-class" => Self::LeavesTraceClass {
                first: json_usize(o, "first")?,
                second: json_usize(o, "second")?,
            },
            "not-a-message" => Self::NotAMessage {
                event: json_usize(o, "event")?,
            },
            "malformed-core" => Self::MalformedCore {
                event: json_usize(o, "event")?,
            },
            "outside-fault-model" => Self::OutsideFaultModel {
                class: FaultClass::from_wire(json_str(o, "class")?).ok_or_else(|| bad("class"))?,
            },
            "exceeds-fault-bound" => Self::ExceedsFaultBound {
                what: match json_str(o, "what")? {
                    "crashes" => BoundKind::Crashes,
                    "cancellations" => BoundKind::Cancellations,
                    "partitions" => BoundKind::Partitions,
                    "faults" => BoundKind::Faults,
                    "recoveries" => BoundKind::Recoveries,
                    _ => return Err(bad("what")),
                },
                count: json_u64(o, "count")?,
                bound: json_u64(o, "bound")?,
            },
            "bound-undeclared" => Self::BoundUndeclared {
                component: match json_str(o, "component")? {
                    "faults" => "faults",
                    "nodes" => "nodes",
                    _ => return Err(bad("component")),
                },
            },
            "outside-value-domain" => Self::OutsideValueDomain {
                value: json_u64(o, "value")?,
                bound: json_u64(o, "bound")?,
            },
            "outside-node-domain" => Self::OutsideNodeDomain {
                nodes: json_u64(o, "nodes")?,
                bound: json_u64(o, "bound")?,
            },
            _ => return Err(bad("rejection kind")),
        })
    }
}

fn reason_from(token: &str) -> Result<InconclusiveReason, ResumeError> {
    InconclusiveReason::ALL
        .into_iter()
        .find(|r| r.as_str() == token)
        .ok_or_else(|| bad("reason"))
}

impl NeighborhoodRecord {
    /// The result's identity: the canonical bytes without the budget and the spend,
    /// which differ between an uninterrupted campaign and a suspended-and-resumed one
    /// that commits the same neighbors.
    #[must_use]
    pub fn result_bytes(&self) -> Vec<u8> {
        let mut j = self.to_json();
        if let Json::Object(o) = &mut j {
            o.remove("spent");
            if let Some(Json::Object(c)) = o.get_mut("config") {
                c.remove("budget");
            }
        }
        j.to_canonical_bytes()
    }

    fn check_consistent(&self) -> Result<(), ResumeError> {
        self.reconcile().map_err(ResumeError::Malformed)
    }

    /// Read a record back from [`Self::to_json`].
    fn from_json(j: &Json) -> Result<Self, ResumeError> {
        let o = json_obj(j, "record")?;
        if json_str(o, "kind")? != RECORD_KIND {
            return Err(bad("record kind"));
        }
        let core = json_obj(field(o, "core")?, "core")?;
        let core_members = json_arr(core, "members")?
            .iter()
            .map(|m| match m {
                Json::Integer(n) => u64::try_from(*n).map_err(|_| bad("members")),
                _ => Err(bad("members")),
            })
            .collect::<Result<Vec<_>, _>>()?;
        let seed_text = json_str(o, "seed")?;
        let seed = seed_text
            .strip_prefix("0x")
            .and_then(|h| u64::from_str_radix(h, 16).ok())
            .ok_or_else(|| bad("seed"))?;
        let c = json_obj(field(o, "config")?, "config")?;
        let b = json_obj(field(c, "budget")?, "budget")?;
        let strategies_sel = json_arr(c, "selected")?
            .iter()
            .map(|t| {
                t.as_str()
                    .and_then(Strategy::from_token)
                    .ok_or_else(|| bad("selected"))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let config = Config {
            seed,
            strategies: strategies_sel,
            walks: json_u32(c, "walks")?,
            walk_length: json_u32(c, "walk_length")?,
            max_delay: json_u32(c, "max_delay")?,
            budget: Budget {
                candidates: json_u64(b, "candidates")?,
                runs: json_u64(b, "runs")?,
                work: json_u64(b, "work")?,
            },
            transaction: json_str(c, "transaction")?.to_owned(),
        };
        let mut strategies = Vec::new();
        for (st, want) in json_arr(o, "strategies")?.iter().zip(Strategy::ALL) {
            let st = json_obj(st, "strategy")?;
            if json_str(st, "strategy")? != want.token() {
                return Err(bad("strategy order"));
            }
            let status = match json_str(st, "status")? {
                "not-run" => StrategyStatus::NotRun(NotRun(json_str(st, "reason")?.to_owned())),
                "ran" => {
                    let mut rejected = BTreeMap::new();
                    for (k, v) in json_obj(field(st, "rejected")?, "rejected")? {
                        let token = REJECTION_TOKENS
                            .into_iter()
                            .find(|t| t == k)
                            .ok_or_else(|| bad("rejected"))?;
                        let Json::Integer(n) = v else {
                            return Err(bad("rejected"));
                        };
                        rejected.insert(token, u64::try_from(*n).map_err(|_| bad("rejected"))?);
                    }
                    let mut not_realizable = BTreeMap::new();
                    for (k, v) in json_obj(field(st, "not_realizable")?, "not_realizable")? {
                        let Json::Integer(n) = v else {
                            return Err(bad("not_realizable"));
                        };
                        not_realizable.insert(
                            k.clone(),
                            u64::try_from(*n).map_err(|_| bad("not_realizable"))?,
                        );
                    }
                    StrategyStatus::Ran(Coverage {
                        generated: json_u64(st, "generated")?,
                        rejected,
                        valid: json_u64(st, "valid")?,
                        unsupported: json_u64(st, "unsupported")?,
                        not_realizable,
                        executed: json_u64(st, "executed")?,
                        passed: json_u64(st, "passed")?,
                        failed: json_u64(st, "failed")?,
                        inconclusive: json_u64(st, "inconclusive")?,
                        distinct_runs: json_u64(st, "distinct_runs")?,
                        frontier: match st.get("frontier") {
                            None => None,
                            Some(f) => Some(f.as_str().ok_or_else(|| bad("frontier"))?.to_owned()),
                        },
                    })
                }
                _ => return Err(bad("status")),
            };
            strategies.push((want, status));
        }
        if strategies.len() != Strategy::ALL.len() {
            return Err(bad("strategies"));
        }
        let mut neighbors = Vec::new();
        for n in json_arr(o, "neighbors")? {
            let n = json_obj(n, "neighbor")?;
            let strategy =
                Strategy::from_token(json_str(n, "strategy")?).ok_or_else(|| bad("strategy"))?;
            let id = json_str(n, "neighbor")?.to_owned();
            let detail = || json_str(n, "detail").map(ToOwned::to_owned);
            let run = || json_str(n, "run").map(ToOwned::to_owned);
            let disposition = match json_str(n, "disposition")? {
                "rejected" => Disposition::Rejected(Rejection::from_json(field(n, "rejection")?)?),
                "not-realizable" => Disposition::NotRealizable(detail()?),
                "unsupported" => Disposition::Unsupported(detail()?),
                "pass" => Disposition::Executed(Execution::Pass { run: run()? }),
                "fail" => Disposition::Executed(Execution::Fail {
                    property: detail()?,
                    run: run()?,
                }),
                "inconclusive" => Disposition::Executed(Execution::Inconclusive {
                    reason: reason_from(json_str(n, "detail")?)?,
                }),
                _ => return Err(bad("disposition")),
            };
            neighbors.push(Neighbor {
                strategy,
                id,
                disposition,
                commitment: json_str(n, "commitment")?.to_owned(),
                witness: match n.get("witness") {
                    None => String::new(),
                    Some(w) => w.as_str().ok_or_else(|| bad("witness"))?.to_owned(),
                },
                identity: match n.get("identity") {
                    None => String::new(),
                    Some(w) => w.as_str().ok_or_else(|| bad("identity"))?.to_owned(),
                },
                content: match n.get("content") {
                    None => String::new(),
                    Some(w) => w.as_str().ok_or_else(|| bad("content"))?.to_owned(),
                },
            });
        }
        let mut failing = Vec::new();
        for f in json_arr(o, "failing_neighbors")? {
            let f = json_obj(f, "failing neighbor")?;
            failing.push(FailingNeighbor {
                strategy: Strategy::from_token(json_str(f, "strategy")?)
                    .ok_or_else(|| bad("strategy"))?,
                neighbor: json_str(f, "neighbor")?.to_owned(),
                property: json_str(f, "property")?.to_owned(),
                run: json_str(f, "run")?.to_owned(),
            });
        }
        let sp = json_obj(field(o, "spent")?, "spent")?;
        let v = json_obj(field(o, "verdict")?, "verdict")?;
        let verdict = match json_str(v, "status")? {
            "complete" => Verdict::Complete,
            "pending" => Verdict::Pending,
            "inconclusive" => Verdict::Inconclusive {
                reason: reason_from(json_str(v, "reason")?)?,
            },
            _ => return Err(bad("verdict")),
        };
        let record = Self {
            inputs: Inputs(field(o, "inputs")?.clone()),
            core: json_str(core, "identity")?.to_owned(),
            core_events: json_u64(core, "events")?,
            core_members,
            subject: json_str(o, "subject")?.to_owned(),
            config,
            strategies,
            neighbors,
            failing,
            spent: Spent {
                candidates: json_u64(sp, "candidates")?,
                runs: json_u64(sp, "runs")?,
                work: json_u64(sp, "work")?,
            },
            verdict,
            envelope: field(o, "envelope")?.clone(),
        };
        // The decode is exact: re-encoding gives the same document.
        if record.to_json() != *j {
            return Err(bad("not canonical"));
        }
        Ok(record)
    }
}

/// Generate, validate, realize and execute the neighborhood of `substrate`'s core under
/// `envelope` and `config`, and record it.
///
/// # Errors
///
/// [`CoreError`] when the core, the budget or the transaction is refused before any
/// neighbor is generated. Everything after that is in the outcome: a budget that runs
/// out is [`Exploration::Suspended`] with a [`Continuation`], never a smaller campaign.
pub fn explore<S: Substrate>(
    substrate: &mut S,
    envelope: &Envelope,
    config: &Config,
) -> Result<Exploration, CoreError> {
    run_campaign(
        substrate,
        envelope,
        config,
        None,
        blake3_index,
        BLAKE3_INDEX,
        &mut Spent::default(),
    )
    .map_err(|e| match e {
        ResumeError::Core(c) => c,
        // Unreachable: a fresh campaign has no continuation to mismatch, nothing to
        // replay, and no bytes to read. Mapped to a refusal rather than a panic.
        ResumeError::Mismatch(_)
        | ResumeError::Diverged
        | ResumeError::Forged { .. }
        | ResumeError::Malformed(_)
        | ResumeError::Exhausted => CoreError::CoreCheckExhausted,
    })
}

/// Continue a suspended campaign under `config`, whose budget is the new cumulative
/// ceiling (the spend already committed counts against it). The inputs now must be
/// the continuation's (see [`Inputs`]). A budget that runs out again suspends again at
/// a frontier no earlier than this one.
///
/// `ledger` is the transaction's cumulative spend, kept by the caller across calls
/// (RFC 0032, cumulative cost). Its work is the cumulative cost: the attempt starts
/// from the larger of it and the continuation's own, is refused before any work when
/// the budget has nothing left above that, and writes the advanced total back on every
/// exit (success, suspension, exhaustion or refusal), so the checks a resume repeats
/// (the core, the identities, the committed copies, the replay) are charged each time
/// and a retry with the same ledger never repeats work for free. Its candidate and run
/// counts are written back as the continuation's record's.
///
/// # Errors
///
/// [`ResumeError`]; `ledger` holds the spend either way.
pub fn resume<S: Substrate>(
    substrate: &mut S,
    envelope: &Envelope,
    config: &Config,
    continuation: &Continuation,
    ledger: &mut Spent,
) -> Result<Exploration, ResumeError> {
    run_campaign(
        substrate,
        envelope,
        config,
        Some(continuation),
        blake3_index,
        BLAKE3_INDEX,
        ledger,
    )
}

/// Check continuation bytes from outside this process against a fresh derivation, and
/// return that derivation. Their inputs must be the current ones, and every committed
/// neighbor they claim must be the one the fresh campaign produces at the same
/// position, disposition and handle included. Nothing the bytes claim is trusted, their
/// spend included: the outcome is the fresh campaign's.
///
/// `ledger` is the transaction's cumulative spend, kept by the caller (the bytes'
/// own spend is a claim and is never taken into it). The bytes are charged to its work
/// by length before they are read, the fresh derivation's work starts from it (its
/// re-executed runs are paid for there; its candidate and run counts are its own
/// record's), and the advanced total is written back on every exit, a malformed,
/// mismatched or forged document included. A retry with the same ledger is refused before any work once the budget
/// has nothing left above it.
///
/// # Errors
///
/// [`ResumeError::Exhausted`] when the budget cannot pay to read the bytes and still
/// work; [`ResumeError::Malformed`]; [`ResumeError::Mismatch`] for other inputs;
/// [`ResumeError::Forged`] for a claimed outcome the derivation does not give.
/// `ledger` holds the spend either way.
pub fn resume_untrusted<S: Substrate>(
    substrate: &mut S,
    envelope: &Envelope,
    config: &Config,
    bytes: &[u8],
    ledger: &mut Spent,
) -> Result<Exploration, ResumeError> {
    check_budget(config).map_err(ResumeError::Core)?;
    let len = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
    // Charged before the first byte is read, and only when work is left after it.
    ledger.work = ledger
        .work
        .checked_add(len)
        .filter(|w| *w < config.budget.work)
        .ok_or(ResumeError::Exhausted)?;
    let claimed = Continuation::decode(bytes)?;
    // The fresh derivation's meter starts from the ledger (the bytes read included),
    // and writes its total back to it on every exit.
    let fresh = run_campaign(
        substrate,
        envelope,
        config,
        None,
        blake3_index,
        BLAKE3_INDEX,
        ledger,
    )?;
    // The fresh campaign's own binding is the current inputs.
    if let Some(field) = claimed
        .record
        .inputs
        .first_difference(&fresh.record().inputs)
    {
        return Err(ResumeError::Mismatch(field));
    }
    let got = fresh.record().neighbors();
    let want = claimed.record.neighbors();
    for (i, w) in want.iter().enumerate() {
        match got.get(i) {
            Some(g) if g == w => {}
            // The fresh campaign stopped before this position: it cannot vouch for it,
            // and nothing past its own frontier is taken from the bytes.
            None if fresh.record().verdict() == Verdict::Pending => break,
            _ => {
                return Err(ResumeError::Forged {
                    neighbor: w.id.clone(),
                });
            }
        }
    }
    Ok(fresh)
}

/// A campaign under the cumulative `ledger`: its meter starts from the ledger (the
/// larger of it and a continuation's spend) and the meter's total is written back on
/// every exit, so no path (a refusal, an exhaustion before or after the campaign
/// exists, a suspension or a finish) loses or resets spend.
fn run_campaign<S: Substrate>(
    substrate: &mut S,
    envelope: &Envelope,
    config: &Config,
    prior: Option<&Continuation>,
    digest: fn(&[u8]) -> String,
    index_name: &'static str,
    ledger: &mut Spent,
) -> Result<Exploration, ResumeError> {
    check_budget(config).map_err(ResumeError::Core)?;
    // Work is the cumulative cost and carries from the ledger; candidates and runs
    // are the record's own counts (the reconciliation holds them to its neighbors), so
    // they start from the continuation's record, or from zero for a fresh derivation,
    // whose re-executed runs are paid for in work.
    let counts = prior.map_or_else(Spent::default, |c| c.record.spent);
    let start = Spent {
        work: ledger.work.max(counts.work),
        ..counts
    };
    ledger.work = start.work;
    let resuming = start != Spent::default();
    // A resume that cannot make progress is refused before any work: no work left, or
    // no candidate left (a frontier always needs one more). Runs may equal the spend:
    // rejected candidates need none.
    let b = config.budget;
    if resuming && (b.work <= start.work || b.candidates <= start.candidates || b.runs < start.runs)
    {
        return Err(ResumeError::Exhausted);
    }
    let mut meter = Meter {
        budget: config.budget,
        spent: start,
    };
    let out = campaign_body(
        substrate, envelope, config, prior, digest, index_name, &mut meter, resuming,
    );
    *ledger = meter.spent;
    out
}

#[allow(clippy::too_many_lines, clippy::too_many_arguments)]
fn campaign_body<S: Substrate>(
    substrate: &mut S,
    envelope: &Envelope,
    config: &Config,
    prior: Option<&Continuation>,
    digest: fn(&[u8]) -> String,
    index_name: &'static str,
    meter: &mut Meter,
    resuming: bool,
) -> Result<Exploration, ResumeError> {
    // Above a carried spend, a campaign that cannot even check the core has exhausted
    // its budget; that is not a refused core. The spend is in the ledger.
    let exhausted_or = |e: CoreError| match e {
        CoreError::CoreCheckExhausted if resuming => ResumeError::Exhausted,
        e => ResumeError::Core(e),
    };
    let facts = check_core(substrate.core(), meter).map_err(exhausted_or)?;
    let ids = Identities::of(substrate, meter).map_err(exhausted_or)?;
    // A resume copies the committed neighbors and failing list into the new campaign:
    // charged by their retained size before they are copied.
    if let Some(c) = prior {
        let weight = c
            .record
            .neighbors
            .iter()
            .map(neighbor_weight)
            .chain(c.record.failing.iter().map(|f| {
                u64::try_from(f.neighbor.len() + f.property.len() + f.run.len()).unwrap_or(u64::MAX)
            }))
            .fold(0_u64, u64::saturating_add);
        meter.work(weight).map_err(|_| ResumeError::Exhausted)?;
    }
    let core_profile = substrate.core_profile();
    envelope
        .check(&core_profile)
        .map_err(|r| ResumeError::Core(CoreError::CoreOutsideEnvelope(r)))?;
    let inputs = Inputs::build(envelope, config, &facts, &ids, &core_profile, index_name);
    if let Some(c) = prior
        && let Some(field) = c.record.inputs.first_difference(&inputs)
    {
        return Err(ResumeError::Mismatch(field));
    }
    let core_members: Vec<u64> = (0..facts.n)
        .filter(|&e| facts.frozen.core[e])
        .map(|e| u64::try_from(e).unwrap_or(u64::MAX))
        .collect();
    let core_events = u64::try_from(facts.n).unwrap_or(u64::MAX);
    let core_identity = ids.core.clone();
    let subject = ids.subject.clone();
    let envelope_json = envelope.to_json();
    let selected = selected_of(config);
    let mut campaign = Campaign {
        substrate,
        envelope,
        core_profile,
        meter,
        facts,
        neighbors: prior.map_or_else(Vec::new, |c| c.record.neighbors.clone()),
        failing: prior.map_or_else(Vec::new, |c| c.record.failing.clone()),
        // (The copy of the committed neighbors and failing list is charged above.)
        // An undisclosable run committed before a suspension stays an engine error.
        engine_error: prior.is_some_and(|c| c.record.neighbors.iter().any(taints)),
        capped: false,
        first_inconclusive: prior.and_then(|c| {
            c.record.neighbors.iter().find_map(|n| match n.disposition {
                Disposition::Executed(Execution::Inconclusive { reason }) => Some(reason),
                _ => None,
            })
        }),
        replay: std::collections::VecDeque::new(),
        replay_only: false,
        contents: prior.map_or_else(Vec::new, |c| c.contents.clone()),
        digest,
    };
    let committed = |s: Strategy| -> Option<Coverage> {
        prior.and_then(|c| {
            c.record.strategies.iter().find_map(|(x, st)| match st {
                StrategyStatus::Ran(cov) if *x == s => Some(cov.clone()),
                _ => None,
            })
        })
    };
    let frontier_rank = prior.map(|c| c.strategy.index());
    let mut strategies = Vec::new();
    let mut exhausted = false;
    let mut gaps = selected.is_empty();
    let mut frontier = None;
    for strategy in Strategy::ALL {
        if !selected.contains(&strategy) {
            strategies.push((
                strategy,
                StrategyStatus::NotRun(NotRun("not selected".to_owned())),
            ));
            continue;
        }
        if let Some(NotRun(why)) = campaign.substrate.not_run(strategy) {
            // A selected strategy the substrate cannot run is a coverage gap. Its reason
            // is admitted like every substrate string.
            gaps = true;
            let why = match admit(campaign.meter, why, MAX_OUTCOME_BYTES, false) {
                Admitted::Kept(w) => w,
                Admitted::Oversized(len) => {
                    campaign.engine_error = true;
                    oversized_marker(len)
                }
                Admitted::Exhausted
                    if !exhausted && frontier_rank.is_some_and(|f| strategy.index() <= f) =>
                {
                    // Out of budget before the prior frontier: nothing new is
                    // committed; the pending record is rebuilt with the cumulative
                    // spend, and the frontier stays where it was.
                    return Ok(rebuild_pending(
                        prior.ok_or(ResumeError::Diverged)?,
                        inputs,
                        [core_identity, subject],
                        core_events,
                        core_members,
                        config,
                        &mut campaign,
                        envelope_json,
                    ));
                }
                Admitted::Exhausted if !exhausted => {
                    // Suspend here: nothing of this strategy is committed.
                    exhausted = true;
                    frontier = Some((strategy, 0));
                    let coverage = Coverage {
                        frontier: Some(format!("{}#0", strategy.token())),
                        ..Coverage::default()
                    };
                    strategies.push((strategy, StrategyStatus::Ran(coverage)));
                    continue;
                }
                Admitted::Exhausted => "budget exhausted before the reason was read".to_owned(),
            };
            strategies.push((strategy, StrategyStatus::NotRun(NotRun(why))));
            continue;
        }
        let mut run = StrategyRun {
            strategy,
            coverage: Coverage::default(),
            seen_schedules: BTreeSet::new(),
            id_to_content: BTreeMap::new(),
            content_to_id: BTreeMap::new(),
            by_digest: BTreeMap::new(),
            seen_runs: BTreeSet::new(),
            next: 0,
        };
        let rank = strategy.index();
        if campaign.capped {
            // After a silent cap nothing more runs; disclosed, never hidden.
            gaps = true;
            strategies.push((
                strategy,
                StrategyStatus::NotRun(NotRun(
                    "the campaign stopped: a substrate capped an earlier strategy".to_owned(),
                )),
            ));
            continue;
        }
        if exhausted {
            run.coverage.frontier = Some(format!("{}#0", strategy.token()));
            strategies.push((strategy, StrategyStatus::Ran(run.coverage)));
            continue;
        }
        let regenerating = frontier_rank.is_some_and(|f| rank <= f);
        if regenerating {
            // Committed before the suspension: its counts carry, and every committed
            // candidate is regenerated (not re-run) and held to what was committed.
            run.coverage = committed(strategy)
                .ok_or_else(|| ResumeError::Mismatch("strategies".to_owned()))?;
            let before_frontier = frontier_rank.is_some_and(|f| rank < f);
            if before_frontier == run.coverage.frontier.is_some() {
                return Err(ResumeError::Mismatch("strategies".to_owned()));
            }
            run.coverage.frontier = None;
            for n in campaign.neighbors.iter().filter(|n| n.strategy == strategy) {
                if let Disposition::Executed(
                    Execution::Pass { run: h } | Execution::Fail { run: h, .. },
                ) = &n.disposition
                    && is_handle(h)
                {
                    run.seen_runs.insert(h.clone());
                }
            }
            if campaign.contents.len() != campaign.neighbors.len() {
                return Err(ResumeError::Diverged);
            }
            campaign.replay = campaign
                .neighbors
                .iter()
                .zip(&campaign.contents)
                .filter(|(n, _)| n.strategy == strategy)
                .map(|(n, content)| Committed {
                    id: n.id.clone(),
                    commitment: n.commitment.clone(),
                    rejection: match &n.disposition {
                        Disposition::Rejected(r) => Some(r.clone()),
                        _ => None,
                    },
                    content: content.clone(),
                })
                .collect();
            campaign.replay_only = before_frontier;
        }
        let result = match strategy {
            Strategy::SchedulePerturbation => campaign.schedule_perturbation(&mut run, config),
            Strategy::AlternateEnabledEvents => campaign.alternate_enabled(&mut run),
            Strategy::MessageDuplicationLossDelay => campaign.messages(&mut run, config),
            _ => campaign.scenarios(&mut run),
        };
        let replaying = campaign.replay_only || !campaign.replay.is_empty();
        match result {
            Err(Stop::Mismatch) => return Err(ResumeError::Diverged),
            Err(Stop::Exhausted) if regenerating && replaying => {
                // Out of budget while regenerating what was already committed.
                return Ok(rebuild_pending(
                    prior.ok_or(ResumeError::Diverged)?,
                    inputs,
                    [core_identity, subject],
                    core_events,
                    core_members,
                    config,
                    &mut campaign,
                    envelope_json,
                ));
            }
            Err(Stop::Exhausted) => {
                exhausted = true;
                let index = run.coverage.generated;
                run.coverage.frontier = Some(format!("{}#{index}", strategy.token()));
                frontier = Some((strategy, index));
            }
            Ok(()) => {
                if !campaign.replay.is_empty() {
                    return Err(ResumeError::Diverged);
                }
            }
        }
        campaign.replay.clear();
        campaign.replay_only = false;
        // An unsupported or unrealizable neighbor, or a selected strategy that produced
        // nothing, is a coverage gap.
        gaps |= run.coverage.unsupported > 0
            || !run.coverage.not_realizable.is_empty()
            || run.coverage.executed == 0;
        strategies.push((strategy, StrategyStatus::Ran(run.coverage)));
    }
    let executed = strategies
        .iter()
        .filter_map(|(_, st)| match st {
            StrategyStatus::Ran(c) => Some(c.executed),
            StrategyStatus::NotRun(_) => None,
        })
        // Counts are bounded by the run budget (at most i64::MAX), so this cannot
        // overflow; checked anyway, and an overflow is a gap, never a clamp.
        .try_fold(0_u64, u64::checked_add);
    gaps |= executed.is_none_or(|e| e == 0);
    // A budget-exhausted campaign always suspends with its continuation (RFC 0032):
    // an earlier undisclosable run is kept in the record and makes the finished
    // campaign an engine error when it resumes to its end. Then an engine error, an
    // undecided run, a coverage gap. A silent cap never suspends: it stops generation.
    let verdict = if exhausted && !campaign.capped {
        Verdict::Pending
    } else if campaign.engine_error {
        Verdict::Inconclusive {
            reason: InconclusiveReason::EngineError,
        }
    } else if let Some(reason) = campaign.first_inconclusive {
        Verdict::Inconclusive { reason }
    } else if gaps {
        Verdict::Inconclusive {
            reason: InconclusiveReason::Unsupported,
        }
    } else {
        Verdict::Complete
    };
    let campaign_contents = campaign.contents;
    let mut record = NeighborhoodRecord {
        inputs,
        core: core_identity,
        core_events,
        core_members,
        subject,
        config: config.clone(),
        strategies,
        neighbors: campaign.neighbors,
        failing: campaign.failing,
        spent: campaign.meter.spent,
        verdict,
        envelope: envelope_json,
    };
    // The engine's own accounting is held to the same reconciliation a verifier runs;
    // a record that fails it is an engine error, never evidence.
    if record.reconcile().is_err() {
        record.verdict = Verdict::Inconclusive {
            reason: InconclusiveReason::EngineError,
        };
        return Ok(Exploration::Finished(record));
    }
    Ok(match (verdict, frontier) {
        (Verdict::Pending, Some((strategy, index))) => {
            // One record, shared by the outcome and its continuation.
            let record = Arc::new(record);
            Exploration::Suspended {
                continuation: Box::new(Continuation {
                    record: Arc::clone(&record),
                    strategy,
                    index,
                    contents: campaign_contents,
                }),
                record,
            }
        }
        _ => Exploration::Finished(record),
    })
}
