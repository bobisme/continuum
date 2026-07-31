//! The claim-status lattice (plan §11.4, PR-1 / IMPL-06).
//!
//! # The status set
//!
//! > ```text
//! > Proposed
//! > Observed
//! > Sampled
//! > Bounded
//! > Validated
//! > Proved
//! > Refuted
//! > Inconclusive
//! > Superseded
//! > ```
//! >
//! > Only trusted services can promote into `Validated` or `Proved`. Agent votes or
//! > confidence cannot.
//! >
//! > — `notes/plan/plan.md` §11.4 "Status lattice"
//!
//! > The status lattice is exactly plan §11.4 (`Proposed, Observed, Sampled, Bounded,
//! > Validated, Proved, Refuted, Inconclusive, Superseded` — there is no `draft`
//! > status).
//! >
//! > — `notes/plan/rfcs/0038-multi-agent-evidence-graph.md`, "Authority"
//!
//! Nine statuses, no more and no fewer. The lower-case tokens of
//! [`ClaimStatus::as_str`] are the `status` enum of
//! `notes/plan/schemas/evidence-graph-node.schema.json`, which RFC 0038 names as the
//! normative encoding ("This RFC summarizes; the schemas decide").
//!
//! # The order
//!
//! docs/44 is the dossier's only statement of which status may follow which:
//!
//! > | Status transition | Authority |
//! > |---|---|
//! > | (creation) → proposed | any authorized human/agent — there is no `draft` status (plan §11.4) |
//! > | proposed → observed | execution service |
//! > | proposed → sampled | execution/verification service |
//! > | proposed → bounded | verification service |
//! > | proposed/bounded → validated | independent checker |
//! > | proposed → proved | Lean proof service |
//! > | any → refuted | valid counterexample/checker |
//! > | any → inconclusive | producing service, with a typed INV-008 reason |
//! > | any → superseded | policy/owner with explicit edge |
//! >
//! > — `notes/plan/docs/44_MULTI_AGENT_EVIDENCE_GRAPH.md`, "Status authority"
//!
//! That table fixes the two ends: `Proposed` is the bottom (creation lands there and
//! there is no `draft` beneath it) and `Superseded` is the top (`any → superseded`, and
//! docs/44's "Garbage collection" retires superseded work rather than moving it on).
//! It also fixes `Refuted` above every assurance status — see below.
//!
//! The order *inside* the assurance band is fixed by RFC 0031, which owns the direction
//! words the intent policy verbs are written against:
//!
//! > **Assurance:** total order `observed < sampled < bounded < validated < proved`;
//! > movement down is `downgraded` and is what `no-downgrade` blocks.
//! >
//! > — `notes/plan/rfcs/0031-semantic-and-intent-diff.md`, "Classification lattice"
//!
//! plan §5.1 lists "lowering assurance from exhaustive to sampled" among the moves that
//! make verification green dishonestly, and docs/33 lists "downgrade exhaustive evidence
//! to simulation". Both sentences presuppose that these classes are comparable: a
//! *lowering* is meaningless between incomparable elements.
//!
//! The resulting Hasse diagram — [`ClaimStatus::COVERS`], one cover per dossier row:
//!
//! ```text
//!   Superseded    ⊤   retired by policy/owner with an explicit SUPERSEDES edge
//!       │
//!   Refuted           a valid counterexample or proof of inconsistency
//!       │
//!   Proved            the Lean kernel accepted the theorem under named axioms
//!       │
//!   Validated         an independent certificate/translation checker passed
//!       │
//!   Bounded           exhaustive/symbolic within a stated envelope
//!       │
//!   Sampled           a probabilistic/randomized campaign
//!       │
//!   Observed          one or more concrete executions
//!       │
//!   Inconclusive      an attempt was recorded and could not decide the claim
//!       │
//!   Proposed      ⊥   any authorized human/agent; there is no `draft` status
//! ```
//!
//! ## Why `Inconclusive` sits between `Proposed` and `Observed`
//!
//! docs/44's `any → inconclusive` row states *who* may write the status (the producing
//! service, with a typed INV-008 reason); the lattice states *when* that write is not a
//! regression. If `Inconclusive` sat above the assurance band, a producing service could
//! retire a `Proved` claim to `Inconclusive(ResourceExhausted)` — exactly what plan §11.4
//! forbids ("Budget exhaustion is never a verdict") and what INV-009 forbids ("Resuming a
//! task may add evidence or refine an unknown; it may not silently replace prior
//! artifacts under the same identity").
//!
//! Below `Observed` it reads correctly in both directions: an inconclusive claim carries
//! strictly more than a bare proposal (an attempt happened and its reason is typed) and
//! strictly less than one concrete execution, and the transition an engine actually needs
//! — `inconclusive → bounded` once the budget is raised — is a promotion, which is
//! INV-009's "refine an unknown".
//!
//! ## Why `Refuted` is above `Proved`
//!
//! `any → refuted` must always be able to land, or a claimed proof would outrank a
//! checked counterexample. The consequence is that the reverse transitions are
//! regressions, which is the point: a valid counterexample cannot be proved away
//! (`refuted → proved`) nor buried under budget exhaustion (`refuted → inconclusive`).
//! The only way past a refutation is `Superseded` — a successor claim under a revised
//! intent, linked by a `SUPERSEDES` edge (plan §4.6, §11.3) — and a confirmed
//! false-positive success verdict is a soundness incident with its own policy
//! (`notes/plan/docs/52_RELEASE_GATES_REV3.md`), not a status edit.
//!
//! ## Why the result is a chain, and what the "not totally ordered" sentences mean
//!
//! > These states are not totally ordered. A production observation and a bounded proof
//! > answer different questions. The assurance envelope records every relevant dimension.
//! >
//! > — `notes/plan/docs/33_REVISION_3_ARCHITECTURE.md`, "Evidence plane"
//!
//! > Evidence forms a partially ordered set, not a total ladder […] A production
//! > observation is not "higher" or "lower" than an inductive proof; they establish
//! > different facts.
//! >
//! > — `notes/plan/rfcs/0010-assurance-results-and-claims.md`, "Evidence classes"
//!
//! The covers above happen to form a chain, so this module's order is total today. That
//! does not collapse either sentence; it is forced by the write model.
//!
//! plan §5.1 requires that `bounded → sampled` be a downgrade. Ordinary work requires that
//! `sampled → bounded` be a promotion. A partial order cannot express both: incomparability
//! is symmetric, so a compare-and-set that permits a sideways move permits the downgrade,
//! and one that forbids sideways moves forbids the promotion and freezes the claim at
//! whichever evidence class reached it first. Only a comparable pair distinguishes the two
//! directions, and that is the whole content of "racing promotions cannot regress the
//! lattice" (plan §11.7). The same argument runs down the band: `bounded → validated`
//! (docs/44's own row) and `validated → proved` are the ordinary accumulation path.
//!
//! What the two sentences deny is an epistemic ranking of *evidence*, and the dossier keeps
//! that denial where it belongs — in the assurance envelope, not in the scalar status:
//!
//! - RFC 0010's thirteen evidence classes are a poset and stay one; the plan §11.4 statuses
//!   are not those classes (RFC 0010 is also the Revision 2 scheme, "not citable without
//!   translation to the docs/52 Revision 3 gates");
//! - the envelope's exploration, proof-evidence and implementation-linkage dimensions
//!   "form a product lattice; they are not collapsed into a single 'level 5'"
//!   (`notes/plan/docs/03_ASSURANCE_AND_TCB.md` §3);
//! - plan §11.4 keeps `CHECKED_CERTIFICATE` versus `TRUSTED_SOLVER` and `checked(N=k)` /
//!   `cutoff_checked(N≤k)` / `proved(∀N)` structurally distinct *beside* the status rather
//!   than as more statuses — a status is a claim's disposition, not a summary of what its
//!   evidence means.
//!
//! The five assurance statuses are therefore order-isomorphic to RFC 0031's requirement
//! ladder (`AssuranceLevel` in `continuum-value`, PR-1 / IMPL-03) — an intent demands a
//! level and a claim reaches a status — while the classes an envelope records stay
//! unordered on both sides of the seam.
//!
//! If PR 0's RFC 0038 expansion states an explicit non-total order, [`ClaimStatus::COVERS`]
//! is the single place to change. The order and lattice laws tested below hold for any
//! finite poset; `order_is_a_chain` is the tripwire that fails first, together with
//! [`ClaimStatus::join`] and [`ClaimStatus::meet`], which take the chain as given.
//!
//! # Regression, and the compare-and-set
//!
//! > status promotion is a compare-and-set against the claim's current status, so racing
//! > promotions cannot regress the lattice
//! >
//! > — plan §11.7 "Write model"
//!
//! > `StatusConflict` — an evidence-graph status promotion lost its compare-and-set
//! > against the claim's current status (plan §11.7, RFC 0038); re-read and retry, the
//! > lattice never regresses.
//! >
//! > — `notes/plan/rfcs/0026-continuumd-native-protocol.md`, "Errors"
//!
//! A transition is a [`StatusTransition::Regression`] exactly when it lowers the status,
//! and [`compare_and_set`] rejects it. Equal statuses are a
//! [`StatusTransition::Reassertion`] and are accepted, because a replayed idempotency key
//! must return the original node identity rather than an error (plan §11.7, RFC 0026).
//! [`verify_promotion_history`] is the acceptance predicate for the plan §24.5 frontier
//! lane's "independent linearizability checker against the §11.4 lattice"
//! (`notes/plan/research/08-agentic-verification.md`,
//! quote-id=evidence-graph-enforcement).
//!
//! ```
//! use continuum_evidence::claim_status::{ClaimStatus, StatusTransition, compare_and_set};
//!
//! let current = ClaimStatus::Bounded;
//! assert_eq!(
//!     current.transition_to(ClaimStatus::Validated),
//!     StatusTransition::Promotion
//! );
//! assert_eq!(
//!     current.transition_to(ClaimStatus::Sampled),
//!     StatusTransition::Regression
//! );
//!
//! // A writer that read `Proposed` loses its compare-and-set against `Bounded`.
//! assert!(compare_and_set(current, ClaimStatus::Proposed, ClaimStatus::Validated).is_err());
//! assert_eq!(
//!     compare_and_set(current, ClaimStatus::Bounded, ClaimStatus::Validated),
//!     Ok(ClaimStatus::Validated)
//! );
//! ```
//!
//! # `Conflict` is a node kind, not a status
//!
//! > Concurrent contradictory claims materialize a `Conflict` node rather than resolving
//! > by write order.
//! >
//! > — plan §11.7
//!
//! `Conflict` is one of plan §11.2's node kinds and `CONFLICTS_WITH` one of §11.3's edge
//! types; it is deliberately absent from the status set, and this module never invents it
//! as one. Two consequences are load-bearing here:
//!
//! - contradiction is never resolved by the order. [`ClaimStatus::join`] is the lattice's
//!   least upper bound, *not* a conflict-resolution rule: a writer holding a contradictory
//!   claim must materialize a `Conflict` node, and "a coordinator cannot resolve conflict
//!   by selecting the most confident agent answer" (docs/44, "Conflict handling");
//! - a lost compare-and-set is a [`StatusConflict`] — a transient, retryable, typed error
//!   (plan §10.3) — and is a different object from a `Conflict` node, which is durable
//!   evidence that two claims cannot both hold.
//!
//! # Deliberately not owned here
//!
//! - **Typed payloads.** `Inconclusive` MUST carry an INV-008 reason and `Validated` a
//!   `validation_basis`; [`ClaimStatus::schema_obligations`] states those obligations
//!   without carrying the payloads, and deliberately does not restate their variants. The
//!   payload types are `InconclusiveReason` and `ValidationBasis` in `continuum-value`'s
//!   assurance vocabulary (PR-1 / IMPL-03), whose own seam note asks this lattice to
//!   reference them rather than duplicate them. Wiring them in is the PR-1 exit's job;
//!   this crate takes no dependency on that crate now, and plan §20 declares no edge for
//!   one yet.
//! - **Authority.** Who may write which status (docs/44's right-hand column, INV-015
//!   "Agents cannot alter evidence status", the `service_identity` the node schema
//!   requires for `sampled`/`bounded`/`validated`/`proved`) is an authorization question,
//!   not a lattice question. The lattice answers only "would this write lower the claim?".
//! - **The write protocol.** Linearization per claim identity, idempotency keys,
//!   persistence, and `Conflict` materialization belong to `continuumd` and the evidence
//!   graph itself (plan §11.7, PR 7). [`compare_and_set`] is the pure decision those
//!   components make, not the transport or the store.

use core::fmt;

/// One element of the plan §11.4 claim-status lattice.
///
/// The variants are declared in the order plan §11.4 and the node schema list them, which
/// is the dossier's *listing* order and not the lattice order — `Inconclusive` is listed
/// after `Refuted` but sits below every assurance status. Deriving [`PartialOrd`] would
/// therefore encode the wrong relation; the order is derived from [`Self::COVERS`]
/// instead.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ClaimStatus {
    /// The claim exists and nothing has been checked (docs/44: "any authorized
    /// human/agent — there is no `draft` status"). The bottom of the lattice.
    Proposed,
    /// "one or more concrete executions" (docs/33, "Evidence plane").
    Observed,
    /// "probabilistic/randomized campaign" (docs/33).
    Sampled,
    /// "exhaustive/symbolic within a stated envelope" (docs/33).
    Bounded,
    /// "independent certificate/translation checker passed" (docs/33).
    ///
    /// Carries `validation_basis` (`CHECKED_CERTIFICATE` or `TRUSTED_SOLVER`); "the two
    /// never render identically" (plan §11.4).
    Validated,
    /// "Lean kernel accepted the theorem under named axioms" (docs/33).
    Proved,
    /// "valid counterexample or proof of inconsistency" (docs/33).
    Refuted,
    /// "available evidence cannot decide the claim" (docs/33), with a typed INV-008
    /// reason. Budget exhaustion is never a verdict (plan §11.4).
    Inconclusive,
    /// Retired in favour of a successor, by "policy/owner with explicit edge" (docs/44);
    /// the successor is linked by a `SUPERSEDES` edge (plan §4.6, §11.3). The top of the
    /// lattice: superseded work "may be compacted but remain[s] reachable from
    /// receipts/audits" (docs/44) rather than moving on.
    Superseded,
}

impl ClaimStatus {
    /// Every claim status, in the order plan §11.4 and the node schema list them.
    pub const ALL: [Self; 9] = [
        Self::Proposed,
        Self::Observed,
        Self::Sampled,
        Self::Bounded,
        Self::Validated,
        Self::Proved,
        Self::Refuted,
        Self::Inconclusive,
        Self::Superseded,
    ];

    /// The bottom of the lattice: where creation lands (docs/44).
    pub const BOTTOM: Self = Self::Proposed;

    /// The top of the lattice: retirement (docs/44).
    pub const TOP: Self = Self::Superseded;

    /// The cover relation — the Hasse edges of the lattice, `(lower, upper)`.
    ///
    /// The order is the reflexive-transitive closure of this table and nothing else, so
    /// every edge is a dossier citation:
    ///
    /// | Cover | Source |
    /// |---|---|
    /// | `Proposed ⋖ Inconclusive` | docs/44 `any → inconclusive`, read as the module documents |
    /// | `Inconclusive ⋖ Observed` | RFC 0031 makes `observed` the floor of the assurance order; INV-009 "refine an unknown" |
    /// | `Observed ⋖ Sampled` | RFC 0031 `observed < sampled` |
    /// | `Sampled ⋖ Bounded` | RFC 0031 `sampled < bounded`; plan §5.1 "lowering assurance from exhaustive to sampled" |
    /// | `Bounded ⋖ Validated` | docs/44 `proposed/bounded → validated`; RFC 0031 `bounded < validated` |
    /// | `Validated ⋖ Proved` | RFC 0031 `validated < proved` |
    /// | `Proved ⋖ Refuted` | docs/44 `any → refuted` |
    /// | `Refuted ⋖ Superseded` | docs/44 `any → superseded` |
    pub const COVERS: [(Self, Self); 8] = [
        (Self::Proposed, Self::Inconclusive),
        (Self::Inconclusive, Self::Observed),
        (Self::Observed, Self::Sampled),
        (Self::Sampled, Self::Bounded),
        (Self::Bounded, Self::Validated),
        (Self::Validated, Self::Proved),
        (Self::Proved, Self::Refuted),
        (Self::Refuted, Self::Superseded),
    ];

    /// The stable machine token, byte-identical to the node schema's `status` enum.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Proposed => "proposed",
            Self::Observed => "observed",
            Self::Sampled => "sampled",
            Self::Bounded => "bounded",
            Self::Validated => "validated",
            Self::Proved => "proved",
            Self::Refuted => "refuted",
            Self::Inconclusive => "inconclusive",
            Self::Superseded => "superseded",
        }
    }

    /// Parse a node-schema `status` token.
    ///
    /// Returns [`None`] for anything outside the nine — including `draft`, which RFC 0038
    /// and docs/44 both single out as not being a status, and `conflict`, which is a node
    /// kind (plan §11.2).
    #[must_use]
    pub fn from_token(token: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|s| s.as_str() == token)
    }

    /// Whether this status is at least `other` in the lattice — `self ≥ other`.
    ///
    /// Reflexive: a status is at least itself, which is what makes an idempotent replay a
    /// permitted write.
    #[must_use]
    pub const fn is_at_least(self, other: Self) -> bool {
        other.up_set() & self.bit() != 0
    }

    /// Classify a status transition, which is what the §11.7 compare-and-set decides on.
    #[must_use]
    pub const fn transition_to(self, next: Self) -> StatusTransition {
        if self.eq_status(next) {
            StatusTransition::Reassertion
        } else if next.is_at_least(self) {
            StatusTransition::Promotion
        } else {
            StatusTransition::Regression
        }
    }

    /// The least upper bound of two statuses.
    ///
    /// This is the lattice operation and *not* a conflict-resolution rule: plan §11.7
    /// requires contradictory claims to materialize a `Conflict` node rather than
    /// resolving by write order, and joining two statuses would resolve by order. Its
    /// legitimate use is reasoning about what a claim's status must be once two
    /// non-contradictory promotions have both landed.
    ///
    /// The order is a chain (`order_is_a_chain`), so the least upper bound is the higher
    /// of the two; a dossier amendment that makes the order partial must rewrite this to
    /// search for the minimum common upper bound.
    #[must_use]
    pub const fn join(self, other: Self) -> Self {
        if self.is_at_least(other) { self } else { other }
    }

    /// The greatest lower bound of two statuses — the dual of [`Self::join`], and the
    /// strongest status both claims are known to have reached.
    #[must_use]
    pub const fn meet(self, other: Self) -> Self {
        if self.is_at_least(other) { other } else { self }
    }

    /// The obligations the node schema attaches to a node carrying this status.
    ///
    /// These are stated by `notes/plan/schemas/evidence-graph-node.schema.json`'s
    /// conditional blocks and by plan §11.4. This module reports them; the payload types
    /// (the INV-008 reason enum, `validation_basis`) land with PR-1 / IMPL-03 in
    /// `continuum-value` and are joined here at the PR-1 exit.
    #[must_use]
    pub const fn schema_obligations(self) -> StatusObligations {
        StatusObligations {
            typed_inconclusive_reason: matches!(self, Self::Inconclusive),
            validation_basis: matches!(self, Self::Validated),
            service_identity: matches!(
                self,
                Self::Sampled | Self::Bounded | Self::Validated | Self::Proved
            ),
        }
    }

    /// Position in [`Self::ALL`]; the bit index used to derive the order.
    ///
    /// This is the dossier's listing order, which is not the lattice order.
    const fn index(self) -> u32 {
        match self {
            Self::Proposed => 0,
            Self::Observed => 1,
            Self::Sampled => 2,
            Self::Bounded => 3,
            Self::Validated => 4,
            Self::Proved => 5,
            Self::Refuted => 6,
            Self::Inconclusive => 7,
            Self::Superseded => 8,
        }
    }

    /// This status as a one-hot bit.
    const fn bit(self) -> u16 {
        1 << self.index()
    }

    /// Every status at or above `self`, as a bit set: the reflexive-transitive closure of
    /// [`Self::COVERS`] from `self` upward.
    const fn up_set(self) -> u16 {
        let mut set = self.bit();
        loop {
            let mut next = set;
            let mut edge = 0;
            while edge < Self::COVERS.len() {
                let (lower, upper) = Self::COVERS[edge];
                if set & lower.bit() != 0 {
                    next |= upper.bit();
                }
                edge += 1;
            }
            if next == set {
                return set;
            }
            set = next;
        }
    }

    /// `const`-callable equality; `PartialEq::eq` is not `const`.
    const fn eq_status(self, other: Self) -> bool {
        self.index() == other.index()
    }
}

impl PartialOrd for ClaimStatus {
    /// The lattice order.
    ///
    /// [`Ord`] is deliberately not implemented. The dossier calls this a *lattice*, and
    /// the module documentation explains why today's covers happen to form a chain; a
    /// later amendment that makes the order partial must be able to answer "incomparable"
    /// without every call site silently misreading a rank.
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        if self == other {
            Some(core::cmp::Ordering::Equal)
        } else if self.is_at_least(*other) {
            Some(core::cmp::Ordering::Greater)
        } else if other.is_at_least(*self) {
            Some(core::cmp::Ordering::Less)
        } else {
            None
        }
    }
}

impl fmt::Display for ClaimStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// What a write would do to a claim's status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StatusTransition {
    /// The status is unchanged. A replayed idempotency key must land here and succeed,
    /// returning the original node identity (plan §11.7, RFC 0026).
    Reassertion,
    /// The status moves up the lattice.
    Promotion,
    /// The status would move down the lattice. The §11.7 compare-and-set rejects it:
    /// "racing promotions cannot regress the lattice".
    Regression,
}

impl StatusTransition {
    /// Whether the §11.7 write model permits this transition.
    #[must_use]
    pub const fn is_permitted(self) -> bool {
        matches!(self, Self::Reassertion | Self::Promotion)
    }

    /// Whether this transition lowers the claim's status.
    #[must_use]
    pub const fn is_regression(self) -> bool {
        matches!(self, Self::Regression)
    }

    /// The stable machine name of this classification.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Reassertion => "reassertion",
            Self::Promotion => "promotion",
            Self::Regression => "regression",
        }
    }
}

impl fmt::Display for StatusTransition {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// The schema-level obligations a status carries (plan §11.4 and the node schema).
///
/// The payload types themselves are not here — see the module's "Deliberately not owned"
/// section.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct StatusObligations {
    /// The node MUST carry an INV-008 `inconclusive_reason` (plan §11.4: "`Inconclusive`
    /// carries a typed reason per INV-008").
    pub typed_inconclusive_reason: bool,
    /// The node MUST record whether solver evidence is `CHECKED_CERTIFICATE` or
    /// `TRUSTED_SOLVER` (plan §11.4).
    pub validation_basis: bool,
    /// The node MUST name the daemon service identity that performed the promotion
    /// (plan §11.7; node schema, `sampled`/`bounded`/`validated`/`proved`).
    pub service_identity: bool,
}

/// A lost compare-and-set against a claim's current status.
///
/// This is the typed `StatusConflict` of plan §10.3: "re-read and retry, the lattice never
/// regresses" (RFC 0026). It is *not* a `Conflict` node — see the module documentation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct StatusConflict {
    /// The status the writer read and compared against.
    pub expected: ClaimStatus,
    /// The status the claim actually carries; the value to re-read and retry against.
    pub actual: ClaimStatus,
}

impl fmt::Display for StatusConflict {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "status conflict: expected {}, claim is {}",
            self.expected, self.actual
        )
    }
}

impl core::error::Error for StatusConflict {}

/// A write that would move a claim down the lattice.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LatticeRegression {
    /// The claim's status before the write.
    pub from: ClaimStatus,
    /// The status the write would have installed.
    pub to: ClaimStatus,
}

impl fmt::Display for LatticeRegression {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "lattice regression: {} would lower to {}",
            self.from, self.to
        )
    }
}

impl core::error::Error for LatticeRegression {}

/// Why a status promotion was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PromotionRejected {
    /// The writer's `expected` status is not the claim's current status. The caller
    /// re-reads and retries (plan §11.7, RFC 0026).
    Conflict(StatusConflict),
    /// The write would lower the claim's status. Re-reading will not help; the write is
    /// wrong.
    Regression(LatticeRegression),
}

impl fmt::Display for PromotionRejected {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Conflict(conflict) => fmt::Display::fmt(conflict, f),
            Self::Regression(regression) => fmt::Display::fmt(regression, f),
        }
    }
}

impl core::error::Error for PromotionRejected {}

/// The pure decision behind plan §11.7's status compare-and-set.
///
/// > status promotion is a compare-and-set against the claim's current status, so racing
/// > promotions cannot regress the lattice
/// >
/// > — plan §11.7
///
/// Linearization per claim identity, persistence, authority checks, and idempotency-key
/// replay belong to `continuumd` (PR 7); this function owns only the lattice half of the
/// decision, so that the daemon and any independent checker share one implementation of
/// "would this write lower the claim?".
///
/// # Errors
///
/// - [`PromotionRejected::Conflict`] when `expected` is not `current`: the caller lost the
///   race and must re-read and retry (plan §10.3 `StatusConflict`).
/// - [`PromotionRejected::Regression`] when `next` is below `current`.
pub fn compare_and_set(
    current: ClaimStatus,
    expected: ClaimStatus,
    next: ClaimStatus,
) -> Result<ClaimStatus, PromotionRejected> {
    if expected != current {
        return Err(PromotionRejected::Conflict(StatusConflict {
            expected,
            actual: current,
        }));
    }
    if current.transition_to(next).is_regression() {
        return Err(PromotionRejected::Regression(LatticeRegression {
            from: current,
            to: next,
        }));
    }
    Ok(next)
}

/// A regression found in a recorded per-claim status history.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct HistoryRegression {
    /// Index of the status the history regressed *to*.
    pub index: usize,
    /// The regressing step.
    pub regression: LatticeRegression,
}

impl fmt::Display for HistoryRegression {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "at step {}: {}", self.index, self.regression)
    }
}

impl core::error::Error for HistoryRegression {}

/// Accept or reject a recorded per-claim status history.
///
/// This is the lattice half of the plan §24.5 frontier lane's acceptance test — "every
/// recorded per-claim promotion history is accepted by an independent linearizability
/// checker against the §11.4 lattice"
/// (`notes/plan/research/08-agentic-verification.md`, quote-id=evidence-graph-enforcement).
/// The linearization itself (that some sequential order of the concurrent operations
/// explains the observed responses) is the checker's job; what it checks each candidate
/// order *against* is this predicate.
///
/// Empty and single-status histories are accepted: a claim that was only ever created has
/// not regressed.
///
/// # Errors
///
/// Returns the first [`HistoryRegression`] — a consecutive pair that lowers the status.
pub fn verify_promotion_history(history: &[ClaimStatus]) -> Result<(), HistoryRegression> {
    for (index, pair) in history.windows(2).enumerate() {
        let [from, to] = [pair[0], pair[1]];
        if from.transition_to(to).is_regression() {
            return Err(HistoryRegression {
                index: index + 1,
                regression: LatticeRegression { from, to },
            });
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// docs/44's "Status authority" table, with the `any` rows expanded as the module
    /// documents: every transition the dossier authorizes must be permitted.
    fn authorized_transitions() -> Vec<(ClaimStatus, ClaimStatus)> {
        let mut rows = vec![
            // proposed → observed / sampled / bounded / proved
            (ClaimStatus::Proposed, ClaimStatus::Observed),
            (ClaimStatus::Proposed, ClaimStatus::Sampled),
            (ClaimStatus::Proposed, ClaimStatus::Bounded),
            (ClaimStatus::Proposed, ClaimStatus::Proved),
            // proposed/bounded → validated
            (ClaimStatus::Proposed, ClaimStatus::Validated),
            (ClaimStatus::Bounded, ClaimStatus::Validated),
            // proposed → inconclusive
            (ClaimStatus::Proposed, ClaimStatus::Inconclusive),
        ];
        // any → refuted, for every status a refutation can still reach.
        for status in ClaimStatus::ALL {
            if status != ClaimStatus::Superseded {
                rows.push((status, ClaimStatus::Refuted));
            }
        }
        // any → superseded.
        for status in ClaimStatus::ALL {
            rows.push((status, ClaimStatus::Superseded));
        }
        rows
    }

    // --- the status set ----------------------------------------------------------------

    #[test]
    fn the_status_set_is_exactly_plan_11_4() {
        // plan §11.4, RFC 0038, and the node schema's `status` enum, in listing order.
        let tokens: Vec<&str> = ClaimStatus::ALL.iter().map(|s| s.as_str()).collect();
        assert_eq!(
            tokens,
            [
                "proposed",
                "observed",
                "sampled",
                "bounded",
                "validated",
                "proved",
                "refuted",
                "inconclusive",
                "superseded"
            ]
        );
    }

    #[test]
    fn there_is_no_draft_status() {
        // RFC 0038: "there is no `draft` status"; docs/44 repeats the guard.
        assert_eq!(ClaimStatus::from_token("draft"), None);
    }

    #[test]
    fn conflict_is_a_node_kind_not_a_status() {
        // plan §11.2 lists `Conflict` among the node kinds; §11.4 does not list it as a
        // status, and §11.7 resolves contradictions by materializing a node.
        assert_eq!(ClaimStatus::from_token("conflict"), None);
    }

    #[test]
    fn tokens_round_trip() {
        for status in ClaimStatus::ALL {
            assert_eq!(ClaimStatus::from_token(status.as_str()), Some(status));
            assert_eq!(status.to_string(), status.as_str());
        }
        assert_eq!(ClaimStatus::from_token("Proposed"), None);
        assert_eq!(ClaimStatus::from_token(""), None);
    }

    #[test]
    fn statuses_are_pairwise_distinct() {
        for (index, status) in ClaimStatus::ALL.iter().enumerate() {
            for other in &ClaimStatus::ALL[index + 1..] {
                assert_ne!(status, other);
                assert_ne!(status.as_str(), other.as_str());
            }
        }
    }

    // --- order laws, exhaustive over the nine statuses ----------------------------------

    #[test]
    fn order_is_reflexive() {
        for status in ClaimStatus::ALL {
            assert!(
                status.is_at_least(status),
                "{status} is not at least itself"
            );
        }
    }

    #[test]
    fn order_is_antisymmetric() {
        for left in ClaimStatus::ALL {
            for right in ClaimStatus::ALL {
                if left.is_at_least(right) && right.is_at_least(left) {
                    assert_eq!(left, right, "{left} and {right} dominate each other");
                }
            }
        }
    }

    #[test]
    fn order_is_transitive() {
        for low in ClaimStatus::ALL {
            for middle in ClaimStatus::ALL {
                for high in ClaimStatus::ALL {
                    if middle.is_at_least(low) && high.is_at_least(middle) {
                        assert!(
                            high.is_at_least(low),
                            "{low} ≤ {middle} ≤ {high} but not {low} ≤ {high}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn order_is_a_chain() {
        // The tripwire described in the module documentation: today's covers are a chain,
        // and `join`/`meet` take that as given.
        for left in ClaimStatus::ALL {
            for right in ClaimStatus::ALL {
                assert!(
                    left.is_at_least(right) || right.is_at_least(left),
                    "{left} and {right} are incomparable"
                );
                assert!(left.partial_cmp(&right).is_some());
            }
        }
    }

    #[test]
    fn proposed_is_the_bottom_and_superseded_the_top() {
        for status in ClaimStatus::ALL {
            assert!(
                status.is_at_least(ClaimStatus::BOTTOM),
                "{status} < proposed"
            );
            assert!(
                ClaimStatus::TOP.is_at_least(status),
                "superseded < {status}"
            );
        }
        assert_eq!(ClaimStatus::BOTTOM, ClaimStatus::Proposed);
        assert_eq!(ClaimStatus::TOP, ClaimStatus::Superseded);
    }

    #[test]
    fn partial_ord_agrees_with_is_at_least() {
        use core::cmp::Ordering;

        for left in ClaimStatus::ALL {
            for right in ClaimStatus::ALL {
                match left.partial_cmp(&right) {
                    Some(Ordering::Equal) => assert_eq!(left, right),
                    Some(Ordering::Greater) => {
                        assert!(left.is_at_least(right) && !right.is_at_least(left));
                    }
                    Some(Ordering::Less) => {
                        assert!(right.is_at_least(left) && !left.is_at_least(right));
                    }
                    None => panic!("{left} and {right} are incomparable"),
                }
            }
        }
    }

    #[test]
    fn covers_are_strict_and_minimal() {
        for (lower, upper) in ClaimStatus::COVERS {
            assert!(upper.is_at_least(lower));
            assert_ne!(lower, upper);
            // A Hasse edge admits nothing strictly between its endpoints.
            for middle in ClaimStatus::ALL {
                let between = middle != lower
                    && middle != upper
                    && middle.is_at_least(lower)
                    && upper.is_at_least(middle);
                assert!(!between, "{middle} sits inside the cover {lower} ⋖ {upper}");
            }
        }
    }

    #[test]
    fn the_order_is_generated_by_the_covers() {
        // Strict comparability holds exactly where the covers make it hold: every strict
        // pair is a chain of covers, so no relation was smuggled in past the citations.
        let mut reachable = vec![vec![false; ClaimStatus::ALL.len()]; ClaimStatus::ALL.len()];
        for (lower, upper) in ClaimStatus::COVERS {
            reachable[lower.index() as usize][upper.index() as usize] = true;
        }
        for middle in ClaimStatus::ALL {
            for low in ClaimStatus::ALL {
                for high in ClaimStatus::ALL {
                    if reachable[low.index() as usize][middle.index() as usize]
                        && reachable[middle.index() as usize][high.index() as usize]
                    {
                        reachable[low.index() as usize][high.index() as usize] = true;
                    }
                }
            }
        }
        for low in ClaimStatus::ALL {
            for high in ClaimStatus::ALL {
                let strictly_below = high.is_at_least(low) && low != high;
                assert_eq!(
                    strictly_below,
                    reachable[low.index() as usize][high.index() as usize],
                    "cover closure disagrees with the order on {low} → {high}"
                );
            }
        }
    }

    // --- lattice laws ------------------------------------------------------------------

    #[test]
    fn join_is_the_least_upper_bound() {
        for left in ClaimStatus::ALL {
            for right in ClaimStatus::ALL {
                let join = left.join(right);
                assert!(join.is_at_least(left) && join.is_at_least(right));
                for bound in ClaimStatus::ALL {
                    if bound.is_at_least(left) && bound.is_at_least(right) {
                        assert!(
                            bound.is_at_least(join),
                            "{bound} bounds {left},{right} below their join {join}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn meet_is_the_greatest_lower_bound() {
        for left in ClaimStatus::ALL {
            for right in ClaimStatus::ALL {
                let meet = left.meet(right);
                assert!(left.is_at_least(meet) && right.is_at_least(meet));
                for bound in ClaimStatus::ALL {
                    if left.is_at_least(bound) && right.is_at_least(bound) {
                        assert!(
                            meet.is_at_least(bound),
                            "{bound} bounds {left},{right} above their meet {meet}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn join_and_meet_are_idempotent_and_commutative() {
        for left in ClaimStatus::ALL {
            assert_eq!(left.join(left), left);
            assert_eq!(left.meet(left), left);
            for right in ClaimStatus::ALL {
                assert_eq!(left.join(right), right.join(left));
                assert_eq!(left.meet(right), right.meet(left));
            }
        }
    }

    #[test]
    fn join_and_meet_are_associative() {
        for a in ClaimStatus::ALL {
            for b in ClaimStatus::ALL {
                for c in ClaimStatus::ALL {
                    assert_eq!(a.join(b).join(c), a.join(b.join(c)));
                    assert_eq!(a.meet(b).meet(c), a.meet(b.meet(c)));
                }
            }
        }
    }

    #[test]
    fn absorption_laws_hold() {
        for left in ClaimStatus::ALL {
            for right in ClaimStatus::ALL {
                assert_eq!(left.join(left.meet(right)), left);
                assert_eq!(left.meet(left.join(right)), left);
            }
        }
    }

    #[test]
    fn join_and_meet_agree_with_the_order() {
        for left in ClaimStatus::ALL {
            for right in ClaimStatus::ALL {
                assert_eq!(right.is_at_least(left), left.join(right) == right);
                assert_eq!(right.is_at_least(left), left.meet(right) == left);
            }
        }
    }

    #[test]
    fn bottom_and_top_are_the_lattice_units() {
        for status in ClaimStatus::ALL {
            assert_eq!(status.join(ClaimStatus::BOTTOM), status);
            assert_eq!(status.meet(ClaimStatus::TOP), status);
            assert_eq!(status.join(ClaimStatus::TOP), ClaimStatus::TOP);
            assert_eq!(status.meet(ClaimStatus::BOTTOM), ClaimStatus::BOTTOM);
        }
    }

    // --- regression detection, exhaustive over all 81 ordered pairs ---------------------

    #[test]
    fn every_ordered_pair_is_classified_exactly_once() {
        for from in ClaimStatus::ALL {
            for to in ClaimStatus::ALL {
                let transition = from.transition_to(to);
                let expected = if from == to {
                    StatusTransition::Reassertion
                } else if to.is_at_least(from) {
                    StatusTransition::Promotion
                } else {
                    StatusTransition::Regression
                };
                assert_eq!(transition, expected, "{from} → {to}");
                assert_eq!(transition.is_regression(), !to.is_at_least(from));
                assert_eq!(transition.is_permitted(), to.is_at_least(from));
                assert_eq!(transition.is_permitted(), !transition.is_regression());
            }
        }
    }

    #[test]
    fn a_regression_is_exactly_a_lowering() {
        for from in ClaimStatus::ALL {
            for to in ClaimStatus::ALL {
                assert_eq!(
                    from.transition_to(to).is_regression(),
                    from.is_at_least(to) && from != to,
                    "{from} → {to}"
                );
            }
        }
    }

    #[test]
    fn every_authorized_transition_is_permitted() {
        for (from, to) in authorized_transitions() {
            assert!(
                from.transition_to(to).is_permitted(),
                "docs/44 authorizes {from} → {to} but the lattice rejects it"
            );
        }
    }

    #[test]
    fn the_plan_5_1_downgrades_are_regressions() {
        // plan §5.1 "lowering assurance from exhaustive to sampled"; docs/33 "downgrade
        // exhaustive evidence to simulation".
        for (from, to) in [
            (ClaimStatus::Bounded, ClaimStatus::Sampled),
            (ClaimStatus::Bounded, ClaimStatus::Observed),
            (ClaimStatus::Validated, ClaimStatus::Bounded),
            (ClaimStatus::Proved, ClaimStatus::Validated),
            (ClaimStatus::Sampled, ClaimStatus::Observed),
        ] {
            assert!(
                from.transition_to(to).is_regression(),
                "{from} → {to} must be a regression"
            );
        }
    }

    #[test]
    fn budget_exhaustion_cannot_retire_earned_assurance() {
        // plan §11.4 "Budget exhaustion is never a verdict"; INV-009 monotonic evidence.
        for from in [
            ClaimStatus::Observed,
            ClaimStatus::Sampled,
            ClaimStatus::Bounded,
            ClaimStatus::Validated,
            ClaimStatus::Proved,
            ClaimStatus::Refuted,
        ] {
            assert!(
                from.transition_to(ClaimStatus::Inconclusive)
                    .is_regression(),
                "{from} → inconclusive must be a regression"
            );
        }
        // …and the transition an engine actually needs is a promotion (INV-009's "refine
        // an unknown").
        assert_eq!(
            ClaimStatus::Inconclusive.transition_to(ClaimStatus::Bounded),
            StatusTransition::Promotion
        );
    }

    #[test]
    fn a_refutation_cannot_be_proved_away() {
        assert_eq!(
            ClaimStatus::Proved.transition_to(ClaimStatus::Refuted),
            StatusTransition::Promotion
        );
        assert_eq!(
            ClaimStatus::Refuted.transition_to(ClaimStatus::Proved),
            StatusTransition::Regression
        );
        // The only way past a refutation is a successor claim (plan §4.6 SUPERSEDES).
        assert_eq!(
            ClaimStatus::Refuted.transition_to(ClaimStatus::Superseded),
            StatusTransition::Promotion
        );
    }

    #[test]
    fn superseded_is_terminal() {
        for to in ClaimStatus::ALL {
            let transition = ClaimStatus::Superseded.transition_to(to);
            if to == ClaimStatus::Superseded {
                assert_eq!(transition, StatusTransition::Reassertion);
            } else {
                assert_eq!(
                    transition,
                    StatusTransition::Regression,
                    "superseded → {to}"
                );
            }
        }
    }

    #[test]
    fn transition_names_are_stable() {
        assert_eq!(StatusTransition::Reassertion.to_string(), "reassertion");
        assert_eq!(StatusTransition::Promotion.to_string(), "promotion");
        assert_eq!(StatusTransition::Regression.to_string(), "regression");
    }

    // --- compare-and-set ---------------------------------------------------------------

    #[test]
    fn a_promotion_from_the_expected_status_is_accepted() {
        assert_eq!(
            compare_and_set(
                ClaimStatus::Bounded,
                ClaimStatus::Bounded,
                ClaimStatus::Validated
            ),
            Ok(ClaimStatus::Validated)
        );
    }

    #[test]
    fn an_idempotent_replay_is_accepted() {
        // plan §11.7: a replayed write returns the original node identity, not an error.
        for status in ClaimStatus::ALL {
            assert_eq!(compare_and_set(status, status, status), Ok(status));
        }
    }

    #[test]
    fn a_lost_compare_and_set_returns_a_status_conflict() {
        // Two writers race from `proposed`; the loser must be told what to re-read.
        let rejected = compare_and_set(
            ClaimStatus::Bounded,
            ClaimStatus::Proposed,
            ClaimStatus::Observed,
        )
        .expect_err("the writer's expected status is stale");
        assert_eq!(
            rejected,
            PromotionRejected::Conflict(StatusConflict {
                expected: ClaimStatus::Proposed,
                actual: ClaimStatus::Bounded,
            })
        );
        assert_eq!(
            rejected.to_string(),
            "status conflict: expected proposed, claim is bounded"
        );
    }

    #[test]
    fn a_downgrade_is_rejected_as_a_lattice_regression() {
        let rejected = compare_and_set(
            ClaimStatus::Bounded,
            ClaimStatus::Bounded,
            ClaimStatus::Sampled,
        )
        .expect_err("a downgrade must not be installed");
        assert_eq!(
            rejected,
            PromotionRejected::Regression(LatticeRegression {
                from: ClaimStatus::Bounded,
                to: ClaimStatus::Sampled,
            })
        );
        assert_eq!(
            rejected.to_string(),
            "lattice regression: bounded would lower to sampled"
        );
    }

    #[test]
    fn a_stale_expectation_is_reported_before_the_regression() {
        // The loser of a race is told to re-read; only a writer that read correctly and
        // still asked to lower the status is told its write is wrong.
        let rejected = compare_and_set(
            ClaimStatus::Proved,
            ClaimStatus::Bounded,
            ClaimStatus::Observed,
        )
        .expect_err("stale expectation");
        assert!(matches!(rejected, PromotionRejected::Conflict(_)));
    }

    #[test]
    fn compare_and_set_never_installs_a_lower_status() {
        for current in ClaimStatus::ALL {
            for next in ClaimStatus::ALL {
                if let Ok(installed) = compare_and_set(current, current, next) {
                    assert!(
                        installed.is_at_least(current),
                        "{current} → {installed} lowered the claim"
                    );
                }
            }
        }
    }

    // --- history verification ----------------------------------------------------------

    #[test]
    fn empty_and_singleton_histories_are_accepted() {
        assert_eq!(verify_promotion_history(&[]), Ok(()));
        assert_eq!(verify_promotion_history(&[ClaimStatus::Proposed]), Ok(()));
    }

    #[test]
    fn a_monotone_history_is_accepted() {
        assert_eq!(
            verify_promotion_history(&[
                ClaimStatus::Proposed,
                ClaimStatus::Inconclusive,
                ClaimStatus::Bounded,
                ClaimStatus::Bounded,
                ClaimStatus::Validated,
                ClaimStatus::Refuted,
                ClaimStatus::Superseded,
            ]),
            Ok(())
        );
        // The full chain, bottom to top, is a legal life story.
        let chain: Vec<ClaimStatus> = {
            let mut sorted = ClaimStatus::ALL.to_vec();
            sorted.sort_by(|a, b| a.partial_cmp(b).expect("the order is a chain"));
            sorted
        };
        assert_eq!(verify_promotion_history(&chain), Ok(()));
    }

    #[test]
    fn a_regressing_history_names_the_offending_step() {
        let regression = verify_promotion_history(&[
            ClaimStatus::Proposed,
            ClaimStatus::Bounded,
            ClaimStatus::Sampled,
            ClaimStatus::Proved,
        ])
        .expect_err("the third step lowers the claim");
        assert_eq!(
            regression,
            HistoryRegression {
                index: 2,
                regression: LatticeRegression {
                    from: ClaimStatus::Bounded,
                    to: ClaimStatus::Sampled,
                },
            }
        );
        assert_eq!(
            regression.to_string(),
            "at step 2: lattice regression: bounded would lower to sampled"
        );
    }

    #[test]
    fn every_authorized_transition_forms_an_accepted_history() {
        for (from, to) in authorized_transitions() {
            assert_eq!(
                verify_promotion_history(&[from, to]),
                Ok(()),
                "{from} → {to}"
            );
        }
    }

    // --- schema obligations seam -------------------------------------------------------

    #[test]
    fn schema_obligations_match_the_node_schema() {
        for status in ClaimStatus::ALL {
            let obligations = status.schema_obligations();
            assert_eq!(
                obligations.typed_inconclusive_reason,
                status == ClaimStatus::Inconclusive,
                "{status}: INV-008 reason"
            );
            assert_eq!(
                obligations.validation_basis,
                status == ClaimStatus::Validated,
                "{status}: validation_basis"
            );
            assert_eq!(
                obligations.service_identity,
                matches!(
                    status,
                    ClaimStatus::Sampled
                        | ClaimStatus::Bounded
                        | ClaimStatus::Validated
                        | ClaimStatus::Proved
                ),
                "{status}: service_identity"
            );
        }
    }
}
