//! Assurance vocabulary and typed inconclusiveness (PR-1 / IMPL-03).
//!
//! # What this module is for
//!
//! > **B11 — Assurance is an envelope, not a badge**
//! >
//! > Every result describes dimensions such as bounds, faults, fairness, values,
//! > schedules, weak-memory model, observer, proof status, and unknowns. "Verified" alone
//! > is prohibited in machine output.
//! >
//! > Every envelope dimension names its producing engine or carries a typed
//! > `Unsupported` value; a dimension is never silently omitted.
//! >
//! > — `notes/plan/plan.md` §3 B11
//!
//! > **INV-008 — Typed inconclusiveness**
//! >
//! > Timeout, unsupported semantics, insufficient telemetry, abstraction ambiguity, and
//! > incomplete proof search are distinct outcomes.
//! >
//! > — `notes/plan/plan.md` §3
//!
//! This module supplies the closed vocabularies those two rules are stated over. It holds
//! no policy: which level a task needs, which engine produced a dimension, and when a
//! claim may be promoted belong to the crates that own those decisions.
//!
//! # What the dossier orders, and what it refuses to order
//!
//! Exactly one assurance order is fixed, and it is fixed normatively:
//!
//! > **Assurance:** total order `observed < sampled < bounded < validated < proved`;
//! > movement down is `downgraded` and is what `no-downgrade` blocks. Checker requirements
//! > (`independent_checker`, `clean_recompute`) turning off is also `downgraded`.
//! >
//! > — `notes/plan/rfcs/0031-semantic-and-intent-diff.md`, "Classification lattice"
//!
//! That order is over the *declared assurance requirement* — the Intent Contract field
//! `assurance.minimum` (`notes/plan/schemas/intent-contract.schema.json`) that CI locks
//! with `assurance = "no-downgrade"` (plan §5.4). It is [`AssuranceLevel`], and it is the
//! only type here that implements [`Ord`]. RFC 0031 rejects generalizing it:
//!
//! > **One global strength order across fields.** Rejected: bounds are partial, assurance
//! > is total, completion policies are unordered — collapsing them loses the distinctions
//! > the locks need.
//!
//! Everything else in the assurance vocabulary is deliberately unordered, and the dossier
//! says so three times:
//!
//! > These dimensions form a product lattice; they are not collapsed into a single
//! > "level 5."
//! >
//! > — `notes/plan/docs/03_ASSURANCE_AND_TCB.md` §3
//!
//! > Evidence forms a partially ordered set, not a total ladder […] A production
//! > observation is not "higher" or "lower" than an inductive proof; they establish
//! > different facts.
//! >
//! > — `notes/plan/rfcs/0010-assurance-results-and-claims.md`, "Evidence classes"
//!
//! > These states are not totally ordered. A production observation and a bounded proof
//! > answer different questions. The assurance envelope records every relevant dimension.
//! >
//! > — `notes/plan/docs/33_REVISION_3_ARCHITECTURE.md`, "Evidence plane"
//!
//! The two statements are consistent because they order different things: RFC 0031 orders
//! *what an intent demands*, so that a policy change has a direction; docs/03, docs/33 and
//! RFC 0010 deny an epistemic order over *what evidence means*. [`EvidenceClass`],
//! [`SemanticCoverage`], [`ExplorationClass`], [`ProofClass`] and
//! [`ImplementationLinkage`] therefore carry equality but no comparison, so that no caller
//! can quietly rank a production observation against an inductive proof.
//!
//! # No success flag
//!
//! Nothing here returns a boolean verdict. An [`AssuranceEnvelope`] cannot be constructed
//! empty: every constructor names all nine dimensions, and a dimension either names its
//! producing engine or carries a typed [`UnsupportedReason`]. The honest summary of a task
//! that established nothing is [`AssuranceEnvelope::unsupported_dimensions`] — a list of
//! what is missing — not a flag that says it passed. This is the assurance half of the
//! PR 1 exit condition, whose epoch half lives in [`crate::epoch`]:
//!
//! > **Exit:** an unsupported empty task returns a valid machine result naming every epoch
//! > and no misleading success flag.
//! >
//! > — `notes/plan/notes/START_HERE_IMPLEMENTATION.md`, PR 1
//!
//! # Seams left open on purpose
//!
//! - **The claim-status lattice (plan §11.4: `Proposed, Observed, Sampled, Bounded,
//!   Validated, Proved, Refuted, Inconclusive, Superseded`) is not defined here.** It is
//!   PR-1 / IMPL-06 and lives with the evidence graph. Two of its statuses reach into this
//!   module: `Inconclusive` "carries a typed reason per INV-008" — that reason is
//!   [`InconclusiveReason`] — and `Validated` "records whether solver evidence is
//!   `CHECKED_CERTIFICATE` or `TRUSTED_SOLVER`" — that discriminator is
//!   [`ValidationBasis`]. The lattice should reference these types rather than restate
//!   their variants; the wiring is the PR-1 exit's job.
//! - **The `established | refuted | inconclusive` verdict is not defined here** for the
//!   same reason: it is the lattice's shape, not this module's.
//! - **`Unsupported` and `EngineError` are reasons, never verdicts.** Per
//!   `notes/plan/schemas/assurance-result.schema.json`: "Unsupported and engine-error
//!   outcomes are not verdicts either: they are inconclusive with `inconclusive_reason`
//!   Unsupported/EngineError (INV-008)." Likewise "Budget exhaustion is never a verdict
//!   (plan §11.4); it is the `BudgetExhausted` error with a continuation" — that error and
//!   its continuation belong to `continuumd` (PR 5).
//! - **`docs/03`'s fifth claim dimension, `ObservationCoverage`, is absent.** docs/03's
//!   conceptual `AssuranceClaim` struct named it, but §3 tabulated no levels for it and no
//!   other dossier source enumerated them; inventing them here would have been exactly the
//!   guessing INV-008 exists to prevent, so docs/03 has since struck the field instead
//!   (bn-3usqn). This module still defines no such type, and should add one only if a
//!   source later fixes the field's levels.

use core::fmt;

// --- the one ordered ladder ------------------------------------------------------------

/// The assurance level an Intent Contract requires, ordered.
///
/// > **Assurance:** total order `observed < sampled < bounded < validated < proved`
/// >
/// > — RFC 0031, "Classification lattice"
///
/// The declaration order below *is* that order, and [`Ord`] is derived from it. The
/// spellings are the closed `assurance.minimum` enum of
/// `notes/plan/schemas/intent-contract.schema.json`.
///
/// The five names coincide with five of the nine plan §11.4 claim statuses. That is
/// deliberate — an intent demands a level, and a claim reaches a status — but the two are
/// different types: the lattice also has `Proposed`, `Refuted`, `Inconclusive` and
/// `Superseded`, which are not levels of anything and are not ordered against these.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum AssuranceLevel {
    /// One or more concrete executions were seen (docs/33, "Evidence plane").
    Observed,
    /// A probabilistic or randomized campaign ran; not exhaustive.
    Sampled,
    /// Exhaustive or symbolic within a stated envelope.
    Bounded,
    /// An independent certificate or translation checker passed. What kind of solver
    /// evidence backs it is [`ValidationBasis`], which is never elided (plan §11.4).
    Validated,
    /// The Lean kernel accepted the theorem under named axioms.
    Proved,
}

impl AssuranceLevel {
    /// Every level, weakest first — RFC 0031's total order.
    pub const ALL: [Self; 5] = [
        Self::Observed,
        Self::Sampled,
        Self::Bounded,
        Self::Validated,
        Self::Proved,
    ];

    /// The stable machine name, per `intent-contract.schema.json`.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Observed => "observed",
            Self::Sampled => "sampled",
            Self::Bounded => "bounded",
            Self::Validated => "validated",
            Self::Proved => "proved",
        }
    }
}

impl fmt::Display for AssuranceLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// The `assurance` block of an Intent Contract: a minimum level plus checker requirements.
///
/// The three fields are `notes/plan/schemas/intent-contract.schema.json`'s `assurance`
/// object (`minimum`, `independent_checker`, `clean_recompute`); RFC 0031 makes all three
/// load-bearing for the `no-downgrade` lock, because a requirement "turning off is also
/// `downgraded`".
///
/// The contract's fourth field, `accepted_evidence_classes`, is a *set* of
/// [`EvidenceClass`] values with no direction, so it takes no part in
/// [`change_to`](Self::change_to); set membership is classified `added`/`removed` per item
/// by the diff engine (RFC 0031), not ranked here.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AssuranceRequirement {
    minimum: AssuranceLevel,
    independent_checker: bool,
    clean_recompute: bool,
}

impl AssuranceRequirement {
    /// Declare a requirement.
    #[must_use]
    pub const fn new(
        minimum: AssuranceLevel,
        independent_checker: bool,
        clean_recompute: bool,
    ) -> Self {
        Self {
            minimum,
            independent_checker,
            clean_recompute,
        }
    }

    /// The minimum acceptable level.
    #[must_use]
    pub const fn minimum(self) -> AssuranceLevel {
        self.minimum
    }

    /// Whether an independent checker is required.
    #[must_use]
    pub const fn independent_checker(self) -> bool {
        self.independent_checker
    }

    /// Whether clean recomputation is required.
    #[must_use]
    pub const fn clean_recompute(self) -> bool {
        self.clean_recompute
    }

    /// Classify a change from this requirement to `after`, per RFC 0031.
    ///
    /// Movement down the [`AssuranceLevel`] order is a downgrade, and so is switching off
    /// either checker requirement. A change that both strengthens and weakens is reported
    /// as [`AssuranceChange::Downgraded`]: RFC 0031's fail-closed rule and plan §5.3 ("all
    /// non-affirmative classifications fail closed") forbid reporting a net direction that
    /// would let `no-downgrade` pass a contract that dropped its independent checker in
    /// exchange for a nominally higher minimum.
    ///
    /// An incomparable outcome never arises here (and [`AssuranceChange`] has no such
    /// variant), because RFC 0031 makes this one field totally ordered — unlike
    /// bounds, which are componentwise partial.
    #[must_use]
    pub const fn change_to(self, after: Self) -> AssuranceChange {
        let weakened = (after.minimum as u8) < (self.minimum as u8)
            || (self.independent_checker && !after.independent_checker)
            || (self.clean_recompute && !after.clean_recompute);
        let strengthened = (after.minimum as u8) > (self.minimum as u8)
            || (!self.independent_checker && after.independent_checker)
            || (!self.clean_recompute && after.clean_recompute);

        if weakened {
            AssuranceChange::Downgraded
        } else if strengthened {
            AssuranceChange::Upgraded
        } else {
            AssuranceChange::Unchanged
        }
    }
}

/// How an `assurance` field changed between two Intent Contracts.
///
/// These are the relations RFC 0031's closed classification set assigns to this field:
/// `unchanged`, and the `upgraded`/`downgraded` pair marked "(assurance / proof policy)".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AssuranceChange {
    /// The requirement is identical.
    Unchanged,
    /// The minimum rose, or a checker requirement was switched on, and nothing weakened.
    Upgraded,
    /// The minimum fell, or a checker requirement was switched off. This is what the
    /// `assurance = "no-downgrade"` intent lock blocks (plan §5.4) and what the plan §5.3
    /// completeness guarantee requires be classified as a privileged intent change.
    Downgraded,
}

impl AssuranceChange {
    /// The stable machine name, per RFC 0031's relation set.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Unchanged => "unchanged",
            Self::Upgraded => "upgraded",
            Self::Downgraded => "downgraded",
        }
    }

    /// Whether the `assurance = "no-downgrade"` lock blocks this change.
    #[must_use]
    pub const fn blocked_by_no_downgrade(self) -> bool {
        matches!(self, Self::Downgraded)
    }
}

impl fmt::Display for AssuranceChange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

// --- solver trust ----------------------------------------------------------------------

/// What backs a `Validated` claim: a checked certificate, or a trusted solver verdict.
///
/// > `Validated` records whether solver evidence is `CHECKED_CERTIFICATE` or
/// > `TRUSTED_SOLVER`; the two never render identically.
/// >
/// > — plan §11.4
///
/// "Never render identically" is why there is no `Display` fallback that could collapse
/// them and no `Default`: every surface that shows one must have chosen which it is. The
/// spellings are `validation_basis` in
/// `notes/plan/schemas/evidence-graph-node.schema.json`.
///
/// Seam: plan §11.4's `Validated` status carries this value. The status lattice is
/// PR-1 / IMPL-06 and should reference this type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ValidationBasis {
    /// A small kernel checked the evidence (docs/03 `CHECKED_CERTIFICATE`).
    CheckedCertificate,
    /// The solver's verdict is trusted; no independently checked certificate exists
    /// (RFC 0005: unsupported solver steps "leave the claim solver-trusted").
    TrustedSolver,
}

impl ValidationBasis {
    /// Both bases.
    pub const ALL: [Self; 2] = [Self::CheckedCertificate, Self::TrustedSolver];

    /// The stable machine name, per `evidence-graph-node.schema.json`.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::CheckedCertificate => "checked-certificate",
            Self::TrustedSolver => "trusted-solver",
        }
    }
}

impl fmt::Display for ValidationBasis {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

// --- the evidence poset ----------------------------------------------------------------

/// A class of evidence offered toward a claim.
///
/// The thirteen classes are RFC 0010's evidence list, spelled as the closed enum shared by
/// `notes/plan/schemas/assurance-result.schema.json` and the Intent Contract's
/// `assurance.accepted_evidence_classes`.
///
/// **Unordered on purpose.** RFC 0010: "Evidence forms a partially ordered set, not a
/// total ladder […] A production observation is not 'higher' or 'lower' than an inductive
/// proof; they establish different facts." No [`Ord`] is derived, so a caller cannot rank
/// two classes without stating a comparison the dossier has not licensed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EvidenceClass {
    /// One deterministic example execution.
    Example,
    /// A randomized or generated campaign.
    Sampled,
    /// Bounded schedule exploration.
    BoundedSchedules,
    /// Equivalence-class-complete partial-order reduction.
    DporComplete,
    /// Exact finite reachability.
    FiniteExact,
    /// Symbolic bounded proof.
    SymbolicBounded,
    /// Inductive invariant proof.
    Inductive,
    /// Parameterized proof.
    Parameterized,
    /// Liveness proof.
    LivenessProof,
    /// Refinement proof.
    RefinementProof,
    /// Evidence emitted by a real execution.
    ProductionObservation,
    /// External or differential corroboration.
    Differential,
    /// An independently checked certificate.
    Certificate,
}

impl EvidenceClass {
    /// Every evidence class, in RFC 0010's listing order.
    ///
    /// The order is the dossier's presentation order and carries no strength claim.
    pub const ALL: [Self; 13] = [
        Self::Example,
        Self::Sampled,
        Self::BoundedSchedules,
        Self::DporComplete,
        Self::FiniteExact,
        Self::SymbolicBounded,
        Self::Inductive,
        Self::Parameterized,
        Self::LivenessProof,
        Self::RefinementProof,
        Self::ProductionObservation,
        Self::Differential,
        Self::Certificate,
    ];

    /// The stable machine name, per `assurance-result.schema.json`.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Example => "example",
            Self::Sampled => "sampled",
            Self::BoundedSchedules => "bounded-schedules",
            Self::DporComplete => "dpor-complete",
            Self::FiniteExact => "finite-exact",
            Self::SymbolicBounded => "symbolic-bounded",
            Self::Inductive => "inductive",
            Self::Parameterized => "parameterized",
            Self::LivenessProof => "liveness-proof",
            Self::RefinementProof => "refinement-proof",
            Self::ProductionObservation => "production-observation",
            Self::Differential => "differential",
            Self::Certificate => "certificate",
        }
    }
}

impl fmt::Display for EvidenceClass {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

// --- the docs/03 claim dimensions ------------------------------------------------------

/// Declare one docs/03 §3 claim dimension.
///
/// Each dimension is a closed set of levels with a fixed `SCREAMING_SNAKE` spelling. None
/// of them derives [`Ord`]: docs/03 §3 closes with "These dimensions form a product
/// lattice; they are not collapsed into a single 'level 5.'", and no dossier source states
/// the comparison relation *within* a dimension either — the tables list levels, they do
/// not assert that each row dominates the one above it. `EXHAUSTIVE_FINITE` versus
/// `STATISTICAL` is exactly the pair RFC 0010 refuses to rank.
macro_rules! claim_dimension {
    (
        $(#[$meta:meta])*
        $name:ident, $count:literal, [ $( $(#[$variant_meta:meta])* $variant:ident => $token:literal ),+ $(,)? ]
    ) => {
        $(#[$meta])*
        ///
        /// Levels are a closed set with no ordering — see [`ExplorationClass`] and the
        /// module documentation for why.
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        pub enum $name {
            $( $(#[$variant_meta])* $variant, )+
        }

        impl $name {
            /// Every level, in the order docs/03 §3 tabulates them.
            ///
            /// The order is the table's row order and carries no strength claim.
            pub const ALL: [Self; $count] = [ $( Self::$variant, )+ ];

            /// The stable machine name, per docs/03 §3.
            #[must_use]
            pub const fn as_str(self) -> &'static str {
                match self {
                    $( Self::$variant => $token, )+
                }
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(self.as_str())
            }
        }
    };
}

claim_dimension! {
    /// How much of the real system's semantics the claim covers
    /// (`notes/plan/docs/03_ASSURANCE_AND_TCB.md` §3, "Semantic coverage").
    SemanticCoverage, 5, [
        /// Abstract semantics only.
        ModelOnly => "MODEL_ONLY",
        /// The implementation core uses modeled capabilities.
        ControlledCore => "CONTROLLED_CORE",
        /// The relevant dependency closure is audited.
        ControlledClosure => "CONTROLLED_CLOSURE",
        /// Real execution emitted semantic evidence.
        ProductionObserved => "PRODUCTION_OBSERVED",
        /// Host boundary coverage is justified for the declared property.
        HostComplete => "HOST_COMPLETE",
    ]
}

claim_dimension! {
    /// How the behaviour space was explored
    /// (`notes/plan/docs/03_ASSURANCE_AND_TCB.md` §3, "Exploration").
    ExplorationClass, 8, [
        /// One deterministic execution.
        OneRun => "ONE_RUN",
        /// Multiple generated executions.
        Sampled => "SAMPLED",
        /// All executions within schedule/fault bounds.
        BoundedChoices => "BOUNDED_CHOICES",
        /// All states within the declared model scope.
        BoundedStates => "BOUNDED_STATES",
        /// The complete reachable finite state space.
        ExhaustiveFinite => "EXHAUSTIVE_FINITE",
        /// Unbounded executions within a symbolic domain.
        Inductive => "INDUCTIVE",
        /// A quantified or cutoff argument for instance sizes.
        Parameterized => "PARAMETERIZED",
        /// A confidence/precision statement, not proof of impossibility — which is why
        /// this dimension is not a ladder.
        Statistical => "STATISTICAL",
    ]
}

claim_dimension! {
    /// What kind of proof evidence stands behind the claim
    /// (`notes/plan/docs/03_ASSURANCE_AND_TCB.md` §3, "Proof evidence").
    ///
    /// [`ProofClass::CheckedCertificate`] is the same fact as
    /// [`ValidationBasis::CheckedCertificate`], seen from the claim rather than from the
    /// evidence-graph node.
    ProofClass, 5, [
        /// The engine reports the result.
        AssertionOnly => "ASSERTION_ONLY",
        /// A counterexample was independently replayed.
        ReplayedWitness => "REPLAYED_WITNESS",
        /// Independent engines agree.
        CrossEngine => "CROSS_ENGINE",
        /// A small kernel checked the evidence.
        CheckedCertificate => "CHECKED_CERTIFICATE",
        /// Checker correctness is linked to a proof-assistant artifact.
        MechanizedKernel => "MECHANIZED_KERNEL",
    ]
}

claim_dimension! {
    /// How the claim is tied to running code
    /// (`notes/plan/docs/03_ASSURANCE_AND_TCB.md` §3, "Implementation linkage").
    ImplementationLinkage, 6, [
        /// Abstract model only.
        NoLink => "NO_LINK",
        /// Model traces are run against the code.
        TestGeneration => "TEST_GENERATION",
        /// Code traces are accepted by the model.
        TraceConformance => "TRACE_CONFORMANCE",
        /// Concrete transitions refine abstract transitions.
        StepRefinement => "STEP_REFINEMENT",
        /// Infinite stuttering is excluded.
        ProgressiveRefinement => "PROGRESSIVE_REFINEMENT",
        /// Selected hyperproperties are preserved.
        StrongObservational => "STRONG_OBSERVATIONAL",
    ]
}

// --- typed inconclusiveness ------------------------------------------------------------

/// Why a claim could not be decided (INV-008).
///
/// > `Inconclusive` carries a typed reason per INV-008 (`Unsupported`,
/// > `ResourceExhausted`, `EngineError`, `InsufficientTelemetry`, `AbstractionAmbiguity`,
/// > `IncompleteProofSearch`).
/// >
/// > — plan §11.4
///
/// The set is closed. Both normative schemas —
/// `notes/plan/schemas/assurance-result.schema.json` and
/// `notes/plan/schemas/evidence-graph-node.schema.json` — spell it as a JSON `enum` with
/// exactly these six members and no extension point, so there is no catch-all variant and
/// no free-text reason: an outcome that is none of these is not yet expressible, and must
/// be added to the dossier before it can be reported.
///
/// **These are reasons, not verdicts.** `assurance-result.schema.json`: "Unsupported and
/// engine-error outcomes are not verdicts either: they are inconclusive with
/// `inconclusive_reason` Unsupported/EngineError (INV-008)"; and "Budget exhaustion is
/// never a verdict (plan §11.4); it is the `BudgetExhausted` error with a continuation."
///
/// Seam: plan §11.4's `Inconclusive` status must carry one of these. The pairing is
/// enforced where the status lattice lives (PR-1 / IMPL-06).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum InconclusiveReason {
    /// The task is outside the declared semantic fragments — INV-008's "unsupported
    /// semantics".
    ///
    /// Distinct from *unknown*: RFC 0031 keeps `unknown` (undecidable or unattempted) and
    /// `unsupported` (outside declared fragments) apart, "these are distinct (INV-008)".
    /// Unknown-ness inside a supported fragment is [`Self::IncompleteProofSearch`], not
    /// this.
    Unsupported,
    /// A resource ran out before the claim could be decided — INV-008's "timeout".
    ///
    /// There is no `resource-exhausted` *verdict*: a run that stops on its budget is the
    /// typed `BudgetExhausted` error carrying its continuation (RFC 0026), never a
    /// semantic answer.
    ResourceExhausted,
    /// The engine failed rather than answered (plan §4.7's engine-defect lifecycle). Also
    /// where a failure advertised as replayable but not reproducible is downgraded to
    /// (INV-006).
    EngineError,
    /// The available telemetry cannot decide the claim — INV-008's "insufficient
    /// telemetry".
    ///
    /// RFC 0010's rendering: `INCONCLUSIVE (production telemetry missing StorageStable)`.
    InsufficientTelemetry,
    /// The abstraction or correspondence map admits several readings — INV-008's
    /// "abstraction ambiguity"; the wire-error surface of the same condition is
    /// `AmbiguousCorrespondence` (plan §10.3).
    AbstractionAmbiguity,
    /// Proof search did not close the obligation — INV-008's "incomplete proof search".
    IncompleteProofSearch,
}

impl InconclusiveReason {
    /// Every reason INV-008 distinguishes, in plan §11.4's listing order.
    ///
    /// The order is presentational: these are alternatives, not degrees.
    pub const ALL: [Self; 6] = [
        Self::Unsupported,
        Self::ResourceExhausted,
        Self::EngineError,
        Self::InsufficientTelemetry,
        Self::AbstractionAmbiguity,
        Self::IncompleteProofSearch,
    ];

    /// The stable machine name.
    ///
    /// The spelling is the schemas' — `"Unsupported"`, `"ResourceExhausted"`, … — which is
    /// deliberately unlike the kebab-case used for verdicts and statuses.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Unsupported => "Unsupported",
            Self::ResourceExhausted => "ResourceExhausted",
            Self::EngineError => "EngineError",
            Self::InsufficientTelemetry => "InsufficientTelemetry",
            Self::AbstractionAmbiguity => "AbstractionAmbiguity",
            Self::IncompleteProofSearch => "IncompleteProofSearch",
        }
    }
}

impl fmt::Display for InconclusiveReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

// --- the B11 assurance envelope --------------------------------------------------------

/// The typed reason an envelope dimension is unsupported.
///
/// `assurance-result.schema.json` types this as an object with a `reason` string rather
/// than a closed enum — that is the dossier's own extension point, because the set of
/// things a lane cannot yet do grows with the lanes. The dossier fixes one value today:
///
/// > Until the weak-memory lane ships (ADR-0032), every envelope's memory dimension reads
/// > `Unsupported(sequential-consistency-only)`.
/// >
/// > — plan §3 B11, restated in plan §24.5's frontier-lane register
///
/// Construction restricts the reason to a non-empty printable-ASCII token with no spaces,
/// so that one reason has one spelling in machine output and diffing two envelopes cannot
/// turn a whitespace difference into a semantic one.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct UnsupportedReason(String);

impl UnsupportedReason {
    /// The reason every `memory_model` dimension carries until the ADR-0032 weak-memory
    /// lane ships.
    pub const SEQUENTIAL_CONSISTENCY_ONLY: &'static str = "sequential-consistency-only";

    /// Build a reason from its token.
    ///
    /// # Errors
    ///
    /// Returns [`UnsupportedReasonError`] when the token is empty or is not printable
    /// ASCII.
    pub fn new(token: &str) -> Result<Self, UnsupportedReasonError> {
        if token.is_empty() {
            return Err(UnsupportedReasonError::Empty);
        }
        if let Some((index, character)) = token.char_indices().find(|(_, c)| !c.is_ascii_graphic())
        {
            return Err(UnsupportedReasonError::NonCanonical { index, character });
        }
        Ok(Self(token.to_owned()))
    }

    /// The standing weak-memory reason (ADR-0032).
    ///
    /// # Panics
    ///
    /// Never: [`Self::SEQUENTIAL_CONSISTENCY_ONLY`] is a printable-ASCII token.
    #[must_use]
    pub fn sequential_consistency_only() -> Self {
        Self::new(Self::SEQUENTIAL_CONSISTENCY_ONLY)
            .expect("the ADR-0032 reason is a canonical token")
    }

    /// The reason token.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for UnsupportedReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Why a string is not a usable unsupported-reason token.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnsupportedReasonError {
    /// The token was empty. `Unsupported` must be typed with a reason; an unexplained
    /// `Unsupported` is the silent omission B11 forbids.
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

impl fmt::Display for UnsupportedReasonError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => f.write_str("unsupported reason is empty"),
            Self::NonCanonical { index, character } => write!(
                f,
                "unsupported reason has a non-canonical character {character:?} at byte {index}"
            ),
        }
    }
}

impl core::error::Error for UnsupportedReasonError {}

/// One of the nine dimensions every assurance envelope reports.
///
/// > Every result describes dimensions such as bounds, faults, fairness, values,
/// > schedules, weak-memory model, observer, proof status, and unknowns.
/// >
/// > — plan §3 B11
///
/// The nine are the `required` list of `assurance-result.schema.json`'s
/// `assurance_envelope`, and the machine names below are its property names.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AssuranceDimension {
    /// Domain sizes and depth the result holds within.
    Bounds,
    /// The fault envelope explored.
    Faults,
    /// The fairness assumptions in force.
    Fairness,
    /// The value domains covered.
    Values,
    /// The schedules explored.
    Schedules,
    /// The memory model assumed. Reads `Unsupported(sequential-consistency-only)` until
    /// the ADR-0032 lane ships.
    MemoryModel,
    /// The observer projections the result is stated over.
    Observer,
    /// The proof status backing the result.
    ProofStatus,
    /// What remains unknown (INV-007's omission transparency, on the envelope).
    Unknowns,
}

impl AssuranceDimension {
    /// Every dimension, in the schema's required order.
    pub const ALL: [Self; 9] = [
        Self::Bounds,
        Self::Faults,
        Self::Fairness,
        Self::Values,
        Self::Schedules,
        Self::MemoryModel,
        Self::Observer,
        Self::ProofStatus,
        Self::Unknowns,
    ];

    /// The stable machine name, per `assurance-result.schema.json`.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Bounds => "bounds",
            Self::Faults => "faults",
            Self::Fairness => "fairness",
            Self::Values => "values",
            Self::Schedules => "schedules",
            Self::MemoryModel => "memory_model",
            Self::Observer => "observer",
            Self::ProofStatus => "proof_status",
            Self::Unknowns => "unknowns",
        }
    }

    /// This dimension's position in [`Self::ALL`].
    #[must_use]
    pub const fn index(self) -> usize {
        match self {
            Self::Bounds => 0,
            Self::Faults => 1,
            Self::Fairness => 2,
            Self::Values => 3,
            Self::Schedules => 4,
            Self::MemoryModel => 5,
            Self::Observer => 6,
            Self::ProofStatus => 7,
            Self::Unknowns => 8,
        }
    }
}

impl fmt::Display for AssuranceDimension {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A dimension covered by a named engine.
///
/// Both fields are required and non-empty: an engine name is what makes the dimension
/// attributable, and "every dimension names its producer" is unsatisfiable if either is
/// blank.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ProducedBy {
    engine: String,
    summary: String,
}

impl ProducedBy {
    /// Record the engine that covered a dimension and what it established.
    ///
    /// # Errors
    ///
    /// Returns [`ProducerError`] when the engine name or the summary is empty.
    pub fn new(engine: &str, summary: &str) -> Result<Self, ProducerError> {
        if engine.is_empty() {
            return Err(ProducerError::EngineUnnamed);
        }
        if summary.is_empty() {
            return Err(ProducerError::SummaryMissing);
        }
        Ok(Self {
            engine: engine.to_owned(),
            summary: summary.to_owned(),
        })
    }

    /// The producing engine's identity.
    #[must_use]
    pub fn engine(&self) -> &str {
        &self.engine
    }

    /// What the engine established for this dimension.
    #[must_use]
    pub fn summary(&self) -> &str {
        &self.summary
    }
}

/// Why a dimension cannot claim a producer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProducerError {
    /// The engine name was empty, so the dimension would name no producer.
    EngineUnnamed,
    /// The summary was empty, so the dimension would state nothing.
    SummaryMissing,
}

impl fmt::Display for ProducerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EngineUnnamed => f.write_str("envelope dimension names no producing engine"),
            Self::SummaryMissing => f.write_str("envelope dimension carries no summary"),
        }
    }
}

impl core::error::Error for ProducerError {}

/// What an envelope says about one dimension.
///
/// There are exactly two possibilities, per `assurance-result.schema.json`'s
/// `envelope_dimension`: the dimension "names its producing engine or carries a typed
/// `Unsupported(reason)`. A dimension is never silently omitted." There is no third,
/// absent, or defaulted case.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum DimensionEvidence {
    /// A named engine covered this dimension.
    Produced(ProducedBy),
    /// Nothing covered this dimension, and here is why.
    Unsupported(UnsupportedReason),
}

impl DimensionEvidence {
    /// Convenience constructor for a covered dimension.
    ///
    /// # Errors
    ///
    /// Returns [`ProducerError`] when the engine name or the summary is empty.
    pub fn produced(engine: &str, summary: &str) -> Result<Self, ProducerError> {
        ProducedBy::new(engine, summary).map(Self::Produced)
    }

    /// The producing engine, or [`None`] when the dimension is unsupported.
    #[must_use]
    pub fn engine(&self) -> Option<&str> {
        match self {
            Self::Produced(produced) => Some(produced.engine()),
            Self::Unsupported(_) => None,
        }
    }

    /// The typed reason, or [`None`] when the dimension is covered.
    #[must_use]
    pub const fn unsupported_reason(&self) -> Option<&UnsupportedReason> {
        match self {
            Self::Produced(_) => None,
            Self::Unsupported(reason) => Some(reason),
        }
    }
}

/// One named dimension of an [`AssuranceEnvelope`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EnvelopeEntry<'a> {
    /// Which dimension this entry reports.
    pub dimension: AssuranceDimension,
    /// What the envelope says about it.
    pub evidence: &'a DimensionEvidence,
}

/// The B11 assurance envelope: all nine dimensions, always named.
///
/// > **Assurance is an envelope, not a badge.** […] "Verified" alone is prohibited in
/// > machine output.
/// >
/// > — plan §3 B11
///
/// There is no constructor that leaves a dimension out and no [`Default`]: the only way to
/// build one is to name a typed [`UnsupportedReason`] for every dimension you cannot cover
/// and then fill in the ones you can. [`entries`](Self::entries) always yields nine
/// entries in [`AssuranceDimension::ALL`] order, so a result cannot report eight
/// dimensions, and the honest summary of a task that established nothing is
/// [`unsupported_dimensions`](Self::unsupported_dimensions) rather than a success flag.
///
/// RFC 0026 makes this envelope "required on every semantic verdict"; wiring it into the
/// result envelope is `continuumd`'s job (PR 5).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssuranceEnvelope {
    dimensions: [DimensionEvidence; 9],
}

impl AssuranceEnvelope {
    /// An envelope in which every dimension is unsupported for the same typed reason.
    ///
    /// This is what an unsupported or empty task reports: nine named dimensions, nine
    /// typed reasons, and nothing that reads as success.
    #[must_use]
    pub fn all_unsupported(reason: &UnsupportedReason) -> Self {
        Self {
            dimensions: core::array::from_fn(|_| DimensionEvidence::Unsupported(reason.clone())),
        }
    }

    /// Replace what the envelope says about one dimension.
    #[must_use]
    pub fn with(mut self, dimension: AssuranceDimension, evidence: DimensionEvidence) -> Self {
        self.set(dimension, evidence);
        self
    }

    /// Replace what the envelope says about one dimension, in place.
    pub fn set(&mut self, dimension: AssuranceDimension, evidence: DimensionEvidence) {
        self.dimensions[dimension.index()] = evidence;
    }

    /// What the envelope says about one dimension.
    #[must_use]
    pub const fn dimension(&self, dimension: AssuranceDimension) -> &DimensionEvidence {
        &self.dimensions[dimension.index()]
    }

    /// Every dimension, named, in [`AssuranceDimension::ALL`] order.
    ///
    /// The array length is nine by construction: an envelope cannot report eight.
    #[must_use]
    pub fn entries(&self) -> [EnvelopeEntry<'_>; 9] {
        AssuranceDimension::ALL.map(|dimension| EnvelopeEntry {
            dimension,
            evidence: self.dimension(dimension),
        })
    }

    /// The dimensions nothing covered, in [`AssuranceDimension::ALL`] order.
    ///
    /// This is the envelope's honest summary — the list INV-007's omission transparency
    /// asks for — and it is deliberately not a boolean.
    #[must_use]
    pub fn unsupported_dimensions(&self) -> Vec<AssuranceDimension> {
        AssuranceDimension::ALL
            .into_iter()
            .filter(|dimension| self.dimension(*dimension).engine().is_none())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reason(token: &str) -> UnsupportedReason {
        UnsupportedReason::new(token).expect("test token is canonical")
    }

    fn produced(engine: &str) -> DimensionEvidence {
        DimensionEvidence::produced(engine, "summary").expect("named producer")
    }

    // --- the one ordered ladder (RFC 0031) --------------------------------------------

    #[test]
    fn assurance_levels_follow_the_rfc_0031_total_order() {
        // RFC 0031: total order `observed < sampled < bounded < validated < proved`.
        assert!(AssuranceLevel::Observed < AssuranceLevel::Sampled);
        assert!(AssuranceLevel::Sampled < AssuranceLevel::Bounded);
        assert!(AssuranceLevel::Bounded < AssuranceLevel::Validated);
        assert!(AssuranceLevel::Validated < AssuranceLevel::Proved);
    }

    #[test]
    fn every_pair_of_assurance_levels_is_comparable() {
        // "assurance is total" (RFC 0031, rejected alternatives) — unlike bounds, no two
        // levels are incomparable.
        for left in AssuranceLevel::ALL {
            for right in AssuranceLevel::ALL {
                assert!(left.partial_cmp(&right).is_some());
            }
        }
        let mut sorted = AssuranceLevel::ALL;
        sorted.sort_unstable();
        assert_eq!(sorted, AssuranceLevel::ALL);
    }

    #[test]
    fn assurance_level_names_match_the_intent_contract_schema() {
        let names: Vec<&str> = AssuranceLevel::ALL
            .iter()
            .map(|level| level.as_str())
            .collect();
        assert_eq!(
            names,
            ["observed", "sampled", "bounded", "validated", "proved"]
        );
        assert_eq!(AssuranceLevel::Proved.to_string(), "proved");
    }

    // --- downgrade classification -----------------------------------------------------

    #[test]
    fn an_identical_requirement_is_unchanged() {
        let requirement = AssuranceRequirement::new(AssuranceLevel::Bounded, true, true);
        assert_eq!(
            requirement.change_to(requirement),
            AssuranceChange::Unchanged
        );
        assert!(!AssuranceChange::Unchanged.blocked_by_no_downgrade());
    }

    #[test]
    fn lowering_the_minimum_is_a_downgrade() {
        // plan §5.1's gaming move: "lowering assurance from exhaustive to sampled".
        let before = AssuranceRequirement::new(AssuranceLevel::Proved, false, false);
        let after = AssuranceRequirement::new(AssuranceLevel::Sampled, false, false);
        assert_eq!(before.change_to(after), AssuranceChange::Downgraded);
        assert!(before.change_to(after).blocked_by_no_downgrade());
    }

    #[test]
    fn raising_the_minimum_is_an_upgrade() {
        let before = AssuranceRequirement::new(AssuranceLevel::Observed, false, false);
        let after = AssuranceRequirement::new(AssuranceLevel::Validated, false, false);
        assert_eq!(before.change_to(after), AssuranceChange::Upgraded);
        assert!(!before.change_to(after).blocked_by_no_downgrade());
    }

    #[test]
    fn switching_off_a_checker_requirement_is_a_downgrade() {
        // RFC 0031: "Checker requirements (`independent_checker`, `clean_recompute`)
        // turning off is also `downgraded`."
        let before = AssuranceRequirement::new(AssuranceLevel::Bounded, true, true);
        for after in [
            AssuranceRequirement::new(AssuranceLevel::Bounded, false, true),
            AssuranceRequirement::new(AssuranceLevel::Bounded, true, false),
            AssuranceRequirement::new(AssuranceLevel::Bounded, false, false),
        ] {
            assert_eq!(before.change_to(after), AssuranceChange::Downgraded);
        }
    }

    #[test]
    fn switching_on_a_checker_requirement_is_an_upgrade() {
        let before = AssuranceRequirement::new(AssuranceLevel::Bounded, false, false);
        let after = AssuranceRequirement::new(AssuranceLevel::Bounded, true, false);
        assert_eq!(before.change_to(after), AssuranceChange::Upgraded);
    }

    #[test]
    fn a_change_that_both_weakens_and_strengthens_fails_closed() {
        // Trading the independent checker for a nominally higher minimum must not read as
        // `upgraded`, or `no-downgrade` would let it through (RFC 0031 fail-closed rule,
        // plan §5.3).
        let before = AssuranceRequirement::new(AssuranceLevel::Bounded, true, true);
        let after = AssuranceRequirement::new(AssuranceLevel::Proved, false, true);
        assert_eq!(before.change_to(after), AssuranceChange::Downgraded);
        assert!(before.change_to(after).blocked_by_no_downgrade());
    }

    #[test]
    fn assurance_change_names_are_rfc_0031_relations() {
        assert_eq!(AssuranceChange::Unchanged.as_str(), "unchanged");
        assert_eq!(AssuranceChange::Upgraded.as_str(), "upgraded");
        assert_eq!(AssuranceChange::Downgraded.to_string(), "downgraded");
    }

    #[test]
    fn requirement_accessors_report_the_declared_contract() {
        let requirement = AssuranceRequirement::new(AssuranceLevel::Validated, true, false);
        assert_eq!(requirement.minimum(), AssuranceLevel::Validated);
        assert!(requirement.independent_checker());
        assert!(!requirement.clean_recompute());
    }

    // --- solver trust ------------------------------------------------------------------

    #[test]
    fn the_two_validation_bases_never_render_identically() {
        // plan §11.4: "the two never render identically".
        assert_ne!(
            ValidationBasis::CheckedCertificate.as_str(),
            ValidationBasis::TrustedSolver.as_str()
        );
        assert_eq!(
            ValidationBasis::CheckedCertificate.to_string(),
            "checked-certificate"
        );
        assert_eq!(ValidationBasis::TrustedSolver.as_str(), "trusted-solver");
    }

    // --- typed inconclusiveness --------------------------------------------------------

    #[test]
    fn every_inv_008_reason_is_a_typed_variant() {
        // plan §11.4 and both normative schemas' `inconclusive_reason` enum.
        let names: Vec<&str> = InconclusiveReason::ALL
            .iter()
            .map(|reason| reason.as_str())
            .collect();
        assert_eq!(
            names,
            [
                "Unsupported",
                "ResourceExhausted",
                "EngineError",
                "InsufficientTelemetry",
                "AbstractionAmbiguity",
                "IncompleteProofSearch",
            ]
        );
    }

    #[test]
    fn inconclusive_reasons_are_distinct_outcomes() {
        // INV-008: timeout, unsupported semantics, insufficient telemetry, abstraction
        // ambiguity, and incomplete proof search "are distinct outcomes".
        for (index, reason) in InconclusiveReason::ALL.iter().enumerate() {
            for other in &InconclusiveReason::ALL[index + 1..] {
                assert_ne!(reason, other);
                assert_ne!(reason.as_str(), other.as_str());
            }
        }
    }

    #[test]
    fn unsupported_and_unknown_do_not_share_a_variant() {
        // RFC 0031: `unknown` (undecidable/unattempted) and `unsupported` (outside
        // declared fragments) "are distinct (INV-008)". Undecidedness inside a supported
        // fragment is IncompleteProofSearch.
        assert_ne!(
            InconclusiveReason::Unsupported,
            InconclusiveReason::IncompleteProofSearch
        );
    }

    // --- the evidence poset ------------------------------------------------------------

    #[test]
    fn the_thirteen_evidence_classes_match_the_schema_vocabulary() {
        let names: Vec<&str> = EvidenceClass::ALL
            .iter()
            .map(|class| class.as_str())
            .collect();
        assert_eq!(
            names,
            [
                "example",
                "sampled",
                "bounded-schedules",
                "dpor-complete",
                "finite-exact",
                "symbolic-bounded",
                "inductive",
                "parameterized",
                "liveness-proof",
                "refinement-proof",
                "production-observation",
                "differential",
                "certificate",
            ]
        );
    }

    #[test]
    fn evidence_classes_carry_identity_but_no_rank() {
        // RFC 0010: "A production observation is not 'higher' or 'lower' than an inductive
        // proof." All this type supports is telling them apart.
        assert_ne!(
            EvidenceClass::ProductionObservation,
            EvidenceClass::Inductive
        );
        assert_eq!(EvidenceClass::Certificate, EvidenceClass::Certificate);
    }

    // --- docs/03 claim dimensions ------------------------------------------------------

    #[test]
    fn the_docs_03_dimension_levels_match_the_tables() {
        assert_eq!(
            SemanticCoverage::ALL.map(SemanticCoverage::as_str),
            [
                "MODEL_ONLY",
                "CONTROLLED_CORE",
                "CONTROLLED_CLOSURE",
                "PRODUCTION_OBSERVED",
                "HOST_COMPLETE",
            ]
        );
        assert_eq!(
            ExplorationClass::ALL.map(ExplorationClass::as_str),
            [
                "ONE_RUN",
                "SAMPLED",
                "BOUNDED_CHOICES",
                "BOUNDED_STATES",
                "EXHAUSTIVE_FINITE",
                "INDUCTIVE",
                "PARAMETERIZED",
                "STATISTICAL",
            ]
        );
        assert_eq!(
            ProofClass::ALL.map(ProofClass::as_str),
            [
                "ASSERTION_ONLY",
                "REPLAYED_WITNESS",
                "CROSS_ENGINE",
                "CHECKED_CERTIFICATE",
                "MECHANIZED_KERNEL",
            ]
        );
        assert_eq!(
            ImplementationLinkage::ALL.map(ImplementationLinkage::as_str),
            [
                "NO_LINK",
                "TEST_GENERATION",
                "TRACE_CONFORMANCE",
                "STEP_REFINEMENT",
                "PROGRESSIVE_REFINEMENT",
                "STRONG_OBSERVATIONAL",
            ]
        );
    }

    #[test]
    fn the_dimensions_are_not_collapsed_into_one_level() {
        // docs/03 §3: "These dimensions form a product lattice; they are not collapsed
        // into a single 'level 5.'" Nothing here converts between dimensions, and the
        // top row of one is not the top row of another.
        assert_ne!(
            ProofClass::MechanizedKernel.as_str(),
            ExplorationClass::Statistical.as_str()
        );
        assert_eq!(ExplorationClass::ALL.len(), 8);
        assert_eq!(SemanticCoverage::ALL.len(), 5);
        assert_eq!(ProofClass::ALL.len(), 5);
        assert_eq!(ImplementationLinkage::ALL.len(), 6);
        assert_eq!(SemanticCoverage::HostComplete.to_string(), "HOST_COMPLETE");
    }

    // --- unsupported reasons -----------------------------------------------------------

    #[test]
    fn the_weak_memory_reason_is_the_one_the_dossier_fixes() {
        // plan §3 B11 / §24.5: every memory dimension reads
        // `Unsupported(sequential-consistency-only)` until ADR-0032's lane ships.
        assert_eq!(
            UnsupportedReason::sequential_consistency_only().as_str(),
            "sequential-consistency-only"
        );
    }

    #[test]
    fn an_untyped_unsupported_is_rejected() {
        // A dimension that says only "unsupported" is the silent omission B11 forbids.
        assert_eq!(
            UnsupportedReason::new(""),
            Err(UnsupportedReasonError::Empty)
        );
    }

    #[test]
    fn a_non_canonical_reason_is_rejected() {
        assert_eq!(
            UnsupportedReason::new("sequential consistency only"),
            Err(UnsupportedReasonError::NonCanonical {
                index: 10,
                character: ' '
            })
        );
        assert!(matches!(
            UnsupportedReason::new("timed\n"),
            Err(UnsupportedReasonError::NonCanonical { .. })
        ));
    }

    // --- the envelope ------------------------------------------------------------------

    #[test]
    fn the_nine_dimensions_are_the_schema_required_set() {
        let names: Vec<&str> = AssuranceDimension::ALL
            .iter()
            .map(|dimension| dimension.as_str())
            .collect();
        assert_eq!(
            names,
            [
                "bounds",
                "faults",
                "fairness",
                "values",
                "schedules",
                "memory_model",
                "observer",
                "proof_status",
                "unknowns",
            ]
        );
        for (index, dimension) in AssuranceDimension::ALL.iter().enumerate() {
            assert_eq!(dimension.index(), index);
        }
    }

    #[test]
    fn an_unsupported_envelope_still_names_every_dimension() {
        // The assurance half of the PR 1 exit: an unsupported empty task reports a valid
        // machine result with no misleading success flag.
        let envelope = AssuranceEnvelope::all_unsupported(&reason("unattempted"));
        let entries = envelope.entries();

        assert_eq!(entries.len(), AssuranceDimension::ALL.len());
        for (entry, dimension) in entries.iter().zip(AssuranceDimension::ALL) {
            assert_eq!(entry.dimension, dimension);
            assert_eq!(
                entry
                    .evidence
                    .unsupported_reason()
                    .map(UnsupportedReason::as_str),
                Some("unattempted")
            );
            assert_eq!(entry.evidence.engine(), None);
        }
        assert_eq!(
            envelope.unsupported_dimensions(),
            AssuranceDimension::ALL.to_vec()
        );
    }

    #[test]
    fn a_partially_covered_envelope_still_names_every_dimension() {
        let envelope = AssuranceEnvelope::all_unsupported(&reason("unattempted"))
            .with(AssuranceDimension::Bounds, produced("engine-explicit"))
            .with(AssuranceDimension::Schedules, produced("engine-dpor"))
            .with(
                AssuranceDimension::MemoryModel,
                DimensionEvidence::Unsupported(UnsupportedReason::sequential_consistency_only()),
            );

        assert_eq!(envelope.entries().len(), 9);
        assert_eq!(
            envelope.dimension(AssuranceDimension::Bounds).engine(),
            Some("engine-explicit")
        );
        assert_eq!(
            envelope
                .dimension(AssuranceDimension::MemoryModel)
                .unsupported_reason()
                .map(UnsupportedReason::as_str),
            Some("sequential-consistency-only")
        );
        assert_eq!(
            envelope.unsupported_dimensions(),
            vec![
                AssuranceDimension::Faults,
                AssuranceDimension::Fairness,
                AssuranceDimension::Values,
                AssuranceDimension::MemoryModel,
                AssuranceDimension::Observer,
                AssuranceDimension::ProofStatus,
                AssuranceDimension::Unknowns,
            ]
        );
    }

    #[test]
    fn a_fully_covered_envelope_omits_nothing() {
        let mut envelope = AssuranceEnvelope::all_unsupported(&reason("unattempted"));
        for dimension in AssuranceDimension::ALL {
            envelope.set(dimension, produced("engine-reference"));
        }
        assert!(envelope.unsupported_dimensions().is_empty());
        assert!(
            envelope
                .entries()
                .iter()
                .all(|entry| entry.evidence.engine().is_some())
        );
    }

    #[test]
    fn a_dimension_that_names_no_producer_is_rejected() {
        // "Every dimension names its producing engine" is unsatisfiable with a blank name.
        assert_eq!(
            DimensionEvidence::produced("", "summary"),
            Err(ProducerError::EngineUnnamed)
        );
        assert_eq!(
            DimensionEvidence::produced("engine-explicit", ""),
            Err(ProducerError::SummaryMissing)
        );
    }

    #[test]
    fn a_produced_dimension_keeps_its_engine_and_summary() {
        let evidence = DimensionEvidence::produced("engine-dpor", "all schedules to depth 20")
            .expect("named producer");
        let DimensionEvidence::Produced(produced) = &evidence else {
            panic!("expected a produced dimension");
        };
        assert_eq!(produced.engine(), "engine-dpor");
        assert_eq!(produced.summary(), "all schedules to depth 20");
        assert_eq!(evidence.unsupported_reason(), None);
    }

    // --- errors -------------------------------------------------------------------------

    #[test]
    fn error_types_render_and_implement_error() {
        fn assert_error<E: core::error::Error>(error: &E) -> String {
            error.to_string()
        }
        assert!(!assert_error(&UnsupportedReasonError::Empty).is_empty());
        assert!(
            !assert_error(&UnsupportedReasonError::NonCanonical {
                index: 0,
                character: ' '
            })
            .is_empty()
        );
        assert!(!assert_error(&ProducerError::EngineUnnamed).is_empty());
        assert!(!assert_error(&ProducerError::SummaryMissing).is_empty());
    }
}
