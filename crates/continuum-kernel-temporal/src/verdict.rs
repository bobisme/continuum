//! The closed verdict vocabulary of the temporal checking base (INV-008, PR 9).
//!
//! # Why this is not a `bool`
//!
//! > Timeout, unsupported semantics, insufficient telemetry, abstraction ambiguity,
//! > and incomplete proof search are distinct outcomes.
//! >
//! > — `notes/plan/plan.md` §2, INV-008 "Typed inconclusiveness"
//!
//! Liveness is where the distinction bites hardest. "This system does not always reach
//! its goal", "this certificate is malformed", and "this build does not implement
//! strong fairness" are three different facts about three different things, and only the
//! first is about the model. [`Verdict`] keeps them apart, and its success arm carries a
//! [`CheckedClaim`] naming the obligations discharged, the envelope they are relative
//! to, and the components still trusted (docs/03 §2).
//!
//! # The three arms
//!
//! - [`Verdict::Verified`] — every obligation of the family was re-derived from the
//!   certificate's own carried data.
//! - [`Verdict::Rejected`] — the bytes are not a valid certificate of the family they
//!   claim. For [`Rejection::FairCycleExists`] this is stronger than "malformed": the
//!   carried graph *contains* the fair cycle the certificate claims to exclude, so the
//!   rejection is a witness.
//! - [`Verdict::Unsupported`] — the bytes name a wire epoch, family, property class, or
//!   fairness class this checker does not implement.

use crate::wire::Envelope;

/// The outcome of checking one temporal certificate from its wire form.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verdict {
    /// Every obligation of the certificate's family was re-derived from its bytes.
    Verified(CheckedClaim),
    /// The bytes are not a well-formed, internally consistent certificate of the
    /// family they claim to belong to.
    Rejected(Rejection),
    /// The bytes are well-formed enough to name a feature this checker does not
    /// implement. Not a claim that the certificate is invalid.
    Unsupported(Feature),
}

impl Verdict {
    /// Whether this verdict is [`Verdict::Verified`].
    ///
    /// Never a substitute for matching: collapsing [`Verdict::Rejected`] and
    /// [`Verdict::Unsupported`] into one `false` throws away the distinction INV-008
    /// exists to preserve.
    #[must_use]
    pub const fn is_verified(&self) -> bool {
        matches!(self, Self::Verified(_))
    }

    /// The claim established by a [`Verdict::Verified`], or `None`.
    #[must_use]
    pub const fn claim(&self) -> Option<&CheckedClaim> {
        match self {
            Self::Verified(claim) => Some(claim),
            Self::Rejected(_) | Self::Unsupported(_) => None,
        }
    }
}

/// The certificate families `continuum-kernel-temporal` implements.
///
/// RFC 0005's "Kernel layering" gives this crate "graph/SCC/ranking certificates", and
/// `notes/plan/schemas/proof-receipt.schema.json` fixes both receipt spellings. The
/// remaining liveness shapes RFC 0005 lists — transition-invariant decomposition and
/// liveness-to-safety reduction — are later slices, and a certificate naming one is
/// [`Feature::CertificateKind`] rather than a rejection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CertificateKind {
    /// A well-founded ranking: unconditional progress to the goal set (docs/03 §6.5
    /// "Ranking"; docs/16 PO-LIV-002 and PO-LIV-003).
    Ranking,
    /// An absence-of-accepting-SCC certificate: no reachable weakly fair cycle avoids
    /// the goal set (docs/03 §6.5 "Finite graph"; docs/16 PO-LIV-004).
    FairSccExclusion,
}

impl CertificateKind {
    /// Every family this crate implements, in wire-code order.
    pub const ALL: [Self; 2] = [Self::Ranking, Self::FairSccExclusion];

    /// The `kind` code this family occupies in the wire header.
    #[must_use]
    pub const fn code(self) -> u16 {
        match self {
            Self::Ranking => 1,
            Self::FairSccExclusion => 2,
        }
    }

    /// The family a wire `kind` code names, or `None`.
    #[must_use]
    pub const fn from_code(code: u16) -> Option<Self> {
        match code {
            1 => Some(Self::Ranking),
            2 => Some(Self::FairSccExclusion),
            _ => None,
        }
    }

    /// The stable lower-case token naming this family in receipts and diagnostics.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Ranking => "ranking",
            Self::FairSccExclusion => "fair-scc-exclusion",
        }
    }
}

/// The temporal property classes this crate can evaluate.
///
/// One class, and both families establish it: *eventually the goal set*. The goal set
/// is carried explicitly as state vectors rather than as a formula, which is the same
/// choice `continuum-kernel-core` makes for its state table and for the same reason —
/// evaluating a temporal formula would need the expression evaluator RFC 0005 places in
/// `continuum-kernel-core`, and a certificate that carries its own goal set needs none.
///
/// Read at every state of a closed table, `◇goal` is what a `leads-to` obligation
/// reduces to: `□(p → ◇goal)` for any `p` whatsoever.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PropertyClass {
    /// `◇ goal`, with the goal set carried as an explicit set of state vectors.
    EventuallyStateSet,
}

impl PropertyClass {
    /// Every property class this crate evaluates, in wire-code order.
    pub const ALL: [Self; 1] = [Self::EventuallyStateSet];

    /// The `property_class` code this class occupies in the wire body.
    #[must_use]
    pub const fn code(self) -> u16 {
        match self {
            Self::EventuallyStateSet => 1,
        }
    }

    /// The class a wire code names, or `None`.
    #[must_use]
    pub const fn from_code(code: u16) -> Option<Self> {
        match code {
            1 => Some(Self::EventuallyStateSet),
            _ => None,
        }
    }

    /// The stable lower-case token naming this class in receipts and diagnostics.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::EventuallyStateSet => "eventually-state-set",
        }
    }
}

/// The fairness classes this crate can interpret.
///
/// RFC 0008 and RFC 0015 name weak and strong fairness; only weak fairness is
/// implemented, and a certificate declaring another class is
/// [`Feature::FairnessClass`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FairnessClass {
    /// Weak fairness (justice): an action continuously enabled from some point on must
    /// eventually be taken. On a cycle this reads: for every fair action, either the
    /// cycle takes it, or some state of the cycle disables it.
    Weak,
}

impl FairnessClass {
    /// Every fairness class this crate interprets.
    pub const ALL: [Self; 1] = [Self::Weak];

    /// The `fairness_class` code this class occupies in the wire body.
    #[must_use]
    pub const fn code(self) -> u16 {
        match self {
            Self::Weak => 1,
        }
    }

    /// The class a wire code names, or `None`.
    #[must_use]
    pub const fn from_code(code: u16) -> Option<Self> {
        match code {
            1 => Some(Self::Weak),
            _ => None,
        }
    }

    /// The stable lower-case token naming this class in diagnostics.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Weak => "weak-fairness",
        }
    }
}

/// What a [`Verdict::Verified`] actually asserts.
///
/// The claim depends on the family:
///
/// - [`CertificateKind::Ranking`] — from every state of the carried table, every
///   execution of the carried transition relation reaches the goal set within
///   [`Self::progress_bound`] steps. No fairness assumption is involved.
/// - [`CertificateKind::FairSccExclusion`] — no execution starting in a declared
///   initial state, satisfying weak fairness for the declared fair actions, avoids the
///   goal set forever.
///
/// Both are statements about the *carried* transition relation. That it is the model's
/// is named by [`Self::trusted_components`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedClaim {
    kind: CertificateKind,
    property: PropertyClass,
    fairness: FairnessClass,
    envelope: Envelope,
    states: u32,
    initial_states: u32,
    goal_states: u32,
    reachable_states: u32,
    transitions: u64,
    fair_actions: u16,
    progress_bound: Option<u64>,
}

impl CheckedClaim {
    /// Build a claim record. Crate-internal: only a completed check may mint one.
    #[allow(
        clippy::too_many_arguments,
        reason = "a claim is a record of everything the check re-derived; collapsing \
                  the fields into a struct literal here would only move the list"
    )]
    pub(crate) const fn new(
        kind: CertificateKind,
        property: PropertyClass,
        fairness: FairnessClass,
        envelope: Envelope,
        states: u32,
        initial_states: u32,
        goal_states: u32,
        reachable_states: u32,
        transitions: u64,
        fair_actions: u16,
        progress_bound: Option<u64>,
    ) -> Self {
        Self {
            kind,
            property,
            fairness,
            envelope,
            states,
            initial_states,
            goal_states,
            reachable_states,
            transitions,
            fair_actions,
            progress_bound,
        }
    }

    /// The certificate family that was checked.
    #[must_use]
    pub const fn kind(&self) -> CertificateKind {
        self.kind
    }

    /// The property class that was established.
    #[must_use]
    pub const fn property(&self) -> PropertyClass {
        self.property
    }

    /// The fairness class the certificate declared.
    #[must_use]
    pub const fn fairness(&self) -> FairnessClass {
        self.fairness
    }

    /// The claim envelope the certificate carried (RFC 0005 "Claim envelope").
    #[must_use]
    pub const fn envelope(&self) -> &Envelope {
        &self.envelope
    }

    /// How many distinct states the certificate's canonical table carried.
    #[must_use]
    pub const fn states(&self) -> u32 {
        self.states
    }

    /// How many initial states were located in that table.
    #[must_use]
    pub const fn initial_states(&self) -> u32 {
        self.initial_states
    }

    /// How many goal states were located in that table.
    #[must_use]
    pub const fn goal_states(&self) -> u32 {
        self.goal_states
    }

    /// How many table states the kernel found reachable from the initial states.
    ///
    /// Recomputed here, never taken from the certificate. Equal to
    /// [`Self::states`] for [`CertificateKind::Ranking`], whose obligation is
    /// discharged at every table state whether it is reachable or not — a strictly
    /// stronger claim, and the reason the reachability search is not run for it.
    #[must_use]
    pub const fn reachable_states(&self) -> u32 {
        self.reachable_states
    }

    /// How many labelled transitions were traversed.
    #[must_use]
    pub const fn transitions(&self) -> u64 {
        self.transitions
    }

    /// How many actions the certificate declared weakly fair.
    #[must_use]
    pub const fn fair_actions(&self) -> u16 {
        self.fair_actions
    }

    /// The largest rank in the certificate, which bounds the number of steps to the
    /// goal set — or `None` for a family that carries no ranking.
    #[must_use]
    pub const fn progress_bound(&self) -> Option<u64> {
        self.progress_bound
    }

    /// The components this verdict still trusts, in the sense of docs/03 §2's
    /// `trusted: Vec<TrustedComponent>`.
    ///
    /// Always present:
    ///
    /// - `certificate-model-correspondence` — that the carried transition relation and
    ///   goal set are the model's. Recomputing them would need a transition-relation
    ///   evaluator, which plan §20 forbids the kernel from sharing with an engine;
    ///   RFC 0024's receipt is where the binding is recorded.
    /// - `envelope-digest-binding` — that the envelope's digests were computed over the
    ///   artifacts they name (ADR-0013 canonical identity is `continuum-value`'s
    ///   obligation).
    ///
    /// Present only for [`CertificateKind::FairSccExclusion`] with a non-empty fair
    /// action set:
    ///
    /// - `fairness-assumption-correspondence` — that the declared weakly fair actions
    ///   are the model's fairness assumptions. This one is load-bearing in the
    ///   dangerous direction: declaring *more* actions fair excludes *more* cycles, so
    ///   an overstated fairness set weakens the claim silently. docs/16 PO-LIV-001
    ///   ("Acceptance condition exactly represents named fairness assumptions") is
    ///   exactly this obligation, and it is discharged by the producer, not here.
    ///
    /// A ranking certificate never carries it: its obligation holds on every schedule,
    /// so no fairness assumption enters the argument.
    #[must_use]
    pub const fn trusted_components(&self) -> &'static [&'static str] {
        match (self.kind, self.fair_actions) {
            (CertificateKind::FairSccExclusion, 1..=u16::MAX) => &[
                "certificate-model-correspondence",
                "envelope-digest-binding",
                "fairness-assumption-correspondence",
            ],
            _ => &[
                "certificate-model-correspondence",
                "envelope-digest-binding",
            ],
        }
    }
}

/// A named position in the wire form, so a rejection says *where*.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Field {
    /// The certificate byte string as a whole.
    Certificate,
    /// The eight-byte format magic.
    Magic,
    /// The header's wire-epoch field.
    WireEpoch,
    /// The header's certificate-family field.
    Kind,
    /// Envelope: the model digest (RFC 0005 "model hash").
    ModelDigest,
    /// Envelope: the semantic epoch (RFC 0005 "CIR semantics version").
    SemanticEpoch,
    /// Envelope: the property digest (RFC 0005 "property hash").
    PropertyDigest,
    /// Envelope: the scope digest (RFC 0005 "bounds").
    ScopeDigest,
    /// Envelope: the assumptions digest (RFC 0005 "assumptions").
    AssumptionsDigest,
    /// Envelope: the producing engine's identity (RFC 0005 "engine version").
    Producer,
    /// Envelope: the certificate schema epoch (RFC 0005 "certificate schema version").
    SchemaEpoch,
    /// Envelope: the number of domain-pack profile digests.
    DomainPackCount,
    /// Envelope: one domain-pack profile digest.
    DomainPack,
    /// State domain: the number of declared state variables.
    VariableCount,
    /// State domain: one variable's name.
    VariableName,
    /// State domain: one variable's inclusive range.
    VariableRange,
    /// State table: the number of states.
    StateCount,
    /// State table: one state vector.
    StateVector,
    /// The property-class code.
    PropertyClassCode,
    /// Goal set: the number of goal states.
    GoalCount,
    /// Goal set: one goal state vector.
    GoalState,
    /// The number of initial states.
    InitialCount,
    /// One initial state vector.
    InitialState,
    /// The number of declared actions.
    ActionCount,
    /// One action name.
    ActionName,
    /// The fairness-class code.
    FairnessClassCode,
    /// The number of weakly fair actions.
    FairActionCount,
    /// One weakly fair action index.
    FairAction,
    /// The number of transitions in one successor row.
    TransitionCount,
    /// One transition's action index.
    TransitionAction,
    /// One transition's target state vector.
    TransitionTarget,
    /// One state's rank.
    Rank,
}

impl Field {
    /// The stable lower-case token naming this field in diagnostics.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Certificate => "certificate",
            Self::Magic => "magic",
            Self::WireEpoch => "wire-epoch",
            Self::Kind => "kind",
            Self::ModelDigest => "model-digest",
            Self::SemanticEpoch => "semantic-epoch",
            Self::PropertyDigest => "property-digest",
            Self::ScopeDigest => "scope-digest",
            Self::AssumptionsDigest => "assumptions-digest",
            Self::Producer => "producer",
            Self::SchemaEpoch => "schema-epoch",
            Self::DomainPackCount => "domain-pack-count",
            Self::DomainPack => "domain-pack",
            Self::VariableCount => "variable-count",
            Self::VariableName => "variable-name",
            Self::VariableRange => "variable-range",
            Self::StateCount => "state-count",
            Self::StateVector => "state-vector",
            Self::PropertyClassCode => "property-class",
            Self::GoalCount => "goal-count",
            Self::GoalState => "goal-state",
            Self::InitialCount => "initial-count",
            Self::InitialState => "initial-state",
            Self::ActionCount => "action-count",
            Self::ActionName => "action-name",
            Self::FairnessClassCode => "fairness-class",
            Self::FairActionCount => "fair-action-count",
            Self::FairAction => "fair-action",
            Self::TransitionCount => "transition-count",
            Self::TransitionAction => "transition-action",
            Self::TransitionTarget => "transition-target",
            Self::Rank => "rank",
        }
    }
}

/// Why a length-prefixed token is not a legal token.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TokenFault {
    /// The token declared length zero.
    Empty,
    /// The token declared more bytes than [`crate::wire::MAX_TOKEN_BYTES`].
    TooLong {
        /// The declared length.
        found: usize,
    },
    /// The token contains a byte outside printable ASCII `0x21..=0x7E`.
    NonPrintable {
        /// Offset of the offending byte within the token.
        offset: usize,
        /// The offending byte.
        byte: u8,
    },
}

/// Every reason the kernel refuses a byte string as a temporal certificate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Rejection {
    /// The certificate is larger than [`crate::wire::MAX_CERTIFICATE_BYTES`]
    /// (RFC 0005 "Resource bounds"; docs/16 PO-KER-002).
    Oversized {
        /// The length of the supplied byte string.
        found: usize,
    },
    /// The reader needed more bytes than the certificate contains.
    Truncated {
        /// The field being read when the bytes ran out.
        field: Field,
        /// Offset at which the read began.
        offset: usize,
        /// Number of bytes the field needed.
        needed: usize,
        /// Number of bytes actually remaining.
        available: usize,
    },
    /// The leading eight bytes are not [`crate::wire::MAGIC`].
    BadMagic,
    /// Bytes remain after the certificate body ends.
    TrailingBytes {
        /// How many bytes followed the body.
        extra: usize,
    },
    /// A length-prefixed token is not a legal token.
    MalformedToken {
        /// Which token.
        field: Field,
        /// What is wrong with it.
        fault: TokenFault,
    },
    /// A declared count is outside the range the format admits.
    CountOutOfRange {
        /// Which count.
        field: Field,
        /// The declared value.
        found: u64,
        /// The smallest admissible value.
        min: u64,
        /// The largest admissible value.
        max: u64,
    },
    /// A sequence required to be strictly ascending is not.
    ///
    /// Canonical ordering is what makes the encoding unique: it forbids duplicates,
    /// fixes the identity of a state as its position in the table, and denies a
    /// producer any freedom two certificates could differ by.
    NotStrictlyAscending {
        /// Which sequence.
        field: Field,
        /// Index of the first element that did not exceed its predecessor.
        index: u32,
    },
    /// A declared state variable has `lo > hi`, so its range admits no value.
    InvertedVariableRange {
        /// Index of the variable in the declared state domain.
        variable: u16,
    },
    /// The envelope's schema epoch disagrees with the wire header's epoch.
    SchemaEpochMismatch {
        /// The epoch declared in the envelope.
        declared: u16,
        /// The epoch declared in the header.
        header: u16,
    },
    /// The certificate declared no initial states.
    ///
    /// A temporal claim quantifies over executions, and an execution starts somewhere.
    NoInitialStates,
    /// A declared initial state does not appear in the certificate's state table.
    InitialStateNotInTable {
        /// Position of the state in the declared initial sequence.
        position: u32,
    },
    /// A declared goal state does not appear in the certificate's state table.
    ///
    /// A goal outside the table can never be reached by an execution that stays in the
    /// table, so the certificate would be claiming progress towards nothing.
    GoalStateNotInTable {
        /// Position of the state in the declared goal sequence.
        position: u32,
    },
    /// A declared fair action index is not in the action table.
    FairActionOutOfRange {
        /// Position within the fair-action list.
        position: u16,
        /// The out-of-range action index.
        action: u16,
        /// The declared action count it leaves.
        actions: u16,
    },
    /// A state in the table violates the certificate's declared state domain.
    ///
    /// docs/03 §6.5's "safety side conditions" for a liveness argument: a ranking or
    /// exclusion claim over a table containing an ill-typed state is a claim about a
    /// system that is not the declared one (docs/16 PO-MOD-003).
    StateOutsideDomain {
        /// Index of the state in the state table.
        state: u32,
        /// Index of the variable whose declared range the state leaves.
        variable: u16,
        /// The value the state carried for that variable.
        value: i64,
    },
    /// A transition names an action index that is not in the action table.
    UnknownAction {
        /// Index of the source state in the state table.
        state: u32,
        /// Index of the transition within that state's successor row.
        entry: u32,
        /// The out-of-range action index.
        action: u16,
    },
    /// A transition leaves the certificate's state table.
    ///
    /// Closure is a precondition of every temporal claim here: an execution that could
    /// leave the table is an execution the certificate says nothing about. Successors
    /// are carried as state *vectors* rather than as indices for exactly this reason —
    /// an index would necessarily be in range and would make the obligation vacuous.
    ClosureFailure {
        /// Index of the source state in the state table.
        state: u32,
        /// Index of the transition within that state's successor row.
        entry: u32,
    },
    /// A goal state carries a non-zero rank.
    ///
    /// Rank zero is what "the obligation is discharged here" means; a goal state with a
    /// positive rank would make the ranking's meaning depend on something outside the
    /// certificate.
    GoalRankNotZero {
        /// Index of the state in the state table.
        state: u32,
        /// The rank it carried.
        rank: u64,
    },
    /// A transition out of a non-goal state does not strictly decrease the rank.
    ///
    /// docs/16 PO-LIV-003 ("Relevant fair progress steps decrease ranking or discharge
    /// eventuality"), in its unconditional form: *every* step decreases, so the
    /// argument needs no fairness assumption.
    RankDoesNotDecrease {
        /// Index of the source state in the state table.
        state: u32,
        /// Index of the transition within that state's successor row.
        entry: u32,
        /// The source state's rank.
        from: u64,
        /// The target state's rank.
        to: u64,
    },
    /// A non-goal state has no successors, so an execution can stop short of the goal.
    ///
    /// docs/03 §6.5's "safety side conditions": a well-founded ranking bounds how long
    /// progress takes, but only a total transition relation guarantees that progress
    /// happens at all. `continuum-kernel-core`'s empty row is a declared deadlock, which
    /// is a legitimate safety fact and a fatal liveness one.
    ProgressDeadlock {
        /// Index of the state in the state table.
        state: u32,
    },
    /// The carried graph contains a reachable weakly fair cycle that avoids the goal
    /// set, which is exactly what the certificate claims it does not.
    ///
    /// docs/16 PO-LIV-004 ("Finite graph contains no reachable SCC satisfying liveness
    /// violation acceptance"), refuted. This rejection is a *witness*: the named state
    /// lies on a strongly connected set of reachable non-goal states in which every
    /// declared fair action is either taken or disabled somewhere, so an execution that
    /// cycles through it forever satisfies weak fairness and never reaches the goal —
    /// a fair lasso in the sense of RFC 0008 and of the Revision 2 fair-lasso spike
    /// (`notes/plan/spikes/advanced_spikes.py`, `check_eventually_done_lasso`).
    FairCycleExists {
        /// The smallest table index of a state on the fair cycle.
        state: u32,
    },
}

impl Rejection {
    /// The stable lower-case token naming this rejection reason.
    ///
    /// This crate owns its rejection vocabulary, so this crate spells it: a consumer
    /// that relays a rejection carries this token verbatim rather than transcribing
    /// the enum into a vocabulary of its own (the second-authority mistake bn-dtg61
    /// declined; RFC 0026 F19, paid at protocol 3.4 by bn-3jrtz). One token per
    /// variant, in the same diagnostic register as [`Field::as_str`]; the variant's
    /// numeric payload does not travel with it — those specifics are recoverable by
    /// re-running this kernel over the same bytes, which any holder of the artifact
    /// can do.
    #[must_use]
    pub const fn reason(&self) -> &'static str {
        match self {
            Self::Oversized { .. } => "oversized",
            Self::Truncated { .. } => "truncated",
            Self::BadMagic => "bad-magic",
            Self::TrailingBytes { .. } => "trailing-bytes",
            Self::MalformedToken { .. } => "malformed-token",
            Self::CountOutOfRange { .. } => "count-out-of-range",
            Self::NotStrictlyAscending { .. } => "not-strictly-ascending",
            Self::InvertedVariableRange { .. } => "inverted-variable-range",
            Self::SchemaEpochMismatch { .. } => "schema-epoch-mismatch",
            Self::NoInitialStates => "no-initial-states",
            Self::InitialStateNotInTable { .. } => "initial-state-not-in-table",
            Self::GoalStateNotInTable { .. } => "goal-state-not-in-table",
            Self::FairActionOutOfRange { .. } => "fair-action-out-of-range",
            Self::StateOutsideDomain { .. } => "state-outside-domain",
            Self::UnknownAction { .. } => "unknown-action",
            Self::ClosureFailure { .. } => "closure-failure",
            Self::GoalRankNotZero { .. } => "goal-rank-not-zero",
            Self::RankDoesNotDecrease { .. } => "rank-does-not-decrease",
            Self::ProgressDeadlock { .. } => "progress-deadlock",
            Self::FairCycleExists { .. } => "fair-cycle-exists",
        }
    }

    /// The wire position this rejection names, when it names one.
    ///
    /// Exactly the variants that carry a [`Field`] member answer [`Some`]; the rest
    /// answer [`None`] rather than guessing a position the rejection never recorded.
    #[must_use]
    pub const fn field(&self) -> Option<Field> {
        match self {
            Self::Truncated { field, .. }
            | Self::MalformedToken { field, .. }
            | Self::CountOutOfRange { field, .. }
            | Self::NotStrictlyAscending { field, .. } => Some(*field),
            _ => None,
        }
    }
}

/// A feature the certificate names and this checker does not implement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Feature {
    /// The certificate declares a wire epoch this build does not implement
    /// (plan §4.6, ADR-0018).
    WireEpoch {
        /// The declared epoch.
        found: u16,
    },
    /// The certificate declares a family outside [`CertificateKind::ALL`].
    ///
    /// RFC 0005's liveness section also names transition-invariant decomposition and
    /// liveness-to-safety reduction; both are later slices.
    CertificateKind {
        /// The declared family code.
        found: u16,
    },
    /// The certificate declares a property class outside [`PropertyClass::ALL`].
    PropertyClass {
        /// The declared class code.
        found: u16,
    },
    /// The certificate declares a fairness class outside
    /// [`FairnessClass::ALL`] — strong fairness, in practice.
    ///
    /// RFC 0008 and RFC 0015 both name it; this build implements weak fairness only,
    /// and a strong-fairness certificate may be perfectly valid.
    FairnessClass {
        /// The declared class code.
        found: u16,
    },
    /// A ranking certificate declares fair actions.
    ///
    /// docs/03 §6.5's "fairness-to-progress linkage" is a real obligation and this
    /// build's ranking check does not discharge it — it checks the strictly stronger,
    /// fairness-free decrease condition. Rather than ignore the declaration and issue a
    /// claim the producer did not ask for, the checker declines.
    FairnessDischarge {
        /// How many fair actions were declared.
        declared: u16,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn certificate_kind_codes_round_trip() {
        for kind in CertificateKind::ALL {
            assert_eq!(CertificateKind::from_code(kind.code()), Some(kind));
        }
        assert_eq!(CertificateKind::from_code(0), None);
        assert_eq!(CertificateKind::from_code(3), None);
    }

    #[test]
    fn the_family_tokens_are_the_receipt_spellings() {
        // `notes/plan/schemas/proof-receipt.schema.json`, certificate.kind enum.
        assert_eq!(CertificateKind::Ranking.as_str(), "ranking");
        assert_eq!(
            CertificateKind::FairSccExclusion.as_str(),
            "fair-scc-exclusion"
        );
    }

    #[test]
    fn property_and_fairness_codes_round_trip() {
        for class in PropertyClass::ALL {
            assert_eq!(PropertyClass::from_code(class.code()), Some(class));
        }
        assert_eq!(PropertyClass::from_code(0), None);
        assert_eq!(PropertyClass::from_code(2), None);

        for class in FairnessClass::ALL {
            assert_eq!(FairnessClass::from_code(class.code()), Some(class));
        }
        // Strong fairness is code 2 by convention and is not implemented.
        assert_eq!(FairnessClass::from_code(2), None);
    }

    #[test]
    fn field_tokens_are_unique() {
        let fields = [
            Field::Certificate,
            Field::Magic,
            Field::WireEpoch,
            Field::Kind,
            Field::ModelDigest,
            Field::SemanticEpoch,
            Field::PropertyDigest,
            Field::ScopeDigest,
            Field::AssumptionsDigest,
            Field::Producer,
            Field::SchemaEpoch,
            Field::DomainPackCount,
            Field::DomainPack,
            Field::VariableCount,
            Field::VariableName,
            Field::VariableRange,
            Field::StateCount,
            Field::StateVector,
            Field::PropertyClassCode,
            Field::GoalCount,
            Field::GoalState,
            Field::InitialCount,
            Field::InitialState,
            Field::ActionCount,
            Field::ActionName,
            Field::FairnessClassCode,
            Field::FairActionCount,
            Field::FairAction,
            Field::TransitionCount,
            Field::TransitionAction,
            Field::TransitionTarget,
            Field::Rank,
        ];
        let tokens: std::collections::BTreeSet<&str> = fields.iter().map(|f| f.as_str()).collect();
        assert_eq!(tokens.len(), fields.len());
    }

    #[test]
    fn a_non_verified_verdict_carries_no_claim() {
        let rejected = Verdict::Rejected(Rejection::FairCycleExists { state: 0 });
        assert!(!rejected.is_verified());
        assert!(rejected.claim().is_none());

        let unsupported = Verdict::Unsupported(Feature::FairnessClass { found: 2 });
        assert!(!unsupported.is_verified());
        assert!(unsupported.claim().is_none());
    }
}
