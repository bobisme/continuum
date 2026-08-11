//! What one probe of one target answers, and what the harness decides about it.
//!
//! Three outcomes, never two and never a boolean (INV-008). "The target ran and every
//! oracle held", "an oracle failed", and "this case did not fit the declared budget, so
//! nothing was decided about it" are three different facts, and a harness that folded the
//! third into either of the others would report a case it never ran as evidence.

use core::fmt;

/// The resource ceilings one target is run under.
///
/// Explicit, per target, and checked *before* the decoder is called. Several of the
/// surfaces this harness drives have no total-size cap of their own —
/// `continuum_value::Value::decode` and `continuum_workspace::snapshot::Snapshot::decode`
/// bound depth and per-field lengths but not the byte string — and one has a documented
/// amplification: `continuumd::codec::json::Json::parse` collects its input into a
/// `Vec<char>` before the grammar runs, so the parser's peak footprint is four times the
/// input. A ceiling that lived inside the decoders would therefore not exist for every
/// target; this one does, and it is the harness's own.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Budget {
    /// The largest input, in bytes, this target will be handed.
    pub input_bytes: usize,
    /// The deepest nesting a *generated* input is allowed to carry.
    ///
    /// Generation stops nesting here. Seeds that probe a production depth bound are
    /// written by hand at exactly `production_max + 1`, nesting one side only, and are
    /// exempt from this bound but never from [`Budget::input_bytes`].
    pub generate_depth: usize,
    /// The largest number of nodes a *generated* structured input may carry.
    ///
    /// A node count and not only a byte count, because an input can be small and still
    /// describe an enormous structure. Bounding the constructed node count makes "the
    /// generator cannot build a bomb" a property of the generator rather than a property
    /// of how the bytes happened to land.
    pub generate_nodes: usize,
}

impl Budget {
    /// The default ceiling: 4 KiB of input, 24 levels, 512 nodes.
    ///
    /// 4 KiB is three orders of magnitude below `continuumd::transport::MAX_FRAME_BYTES`,
    /// and deliberately so: the oversized class is expressed by a *declared* length,
    /// which costs four bytes, never by materializing one. Nothing in this harness
    /// allocates in proportion to a number an input declares.
    pub const DEFAULT: Self = Self {
        input_bytes: 4096,
        generate_depth: 24,
        generate_nodes: 512,
    };

    /// The name of the ceiling [`Budget::input_bytes`] is.
    pub const INPUT_BYTES: &'static str = "input-bytes";
}

/// Which property failed.
///
/// A closed vocabulary. Each arm is a property this harness asserts of every target on
/// every input, and each one is demonstrated to be armed by a deliberately defective
/// mutant in `mutant.rs` — an oracle that has never fired is a hypothesis, not evidence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Oracle {
    /// The decoder unwound. docs/12 §11 classes "malformed artifact panic in kernel" as a
    /// release blocker, and RFC 0026 requires malformed input to produce "a typed error,
    /// never a panic".
    Panicked,
    /// Two probes of the same bytes disagreed. INV-005: a decoder is a pure function of
    /// its input, so a second, different answer is either hidden state or ambient input.
    Nondeterministic,
    /// An accepted input did not re-encode to itself, or its normal form was not a
    /// fixpoint. A canonical decoder that accepts two spellings of one value has made the
    /// encoding unusable as an identity (ADR-0013).
    RoundTrip,
    /// The decoder's answer was outside the closed landing vocabulary its target
    /// declares. The mechanical guard against a typed outcome quietly becoming an untyped
    /// one (INV-008).
    Untyped,
}

impl Oracle {
    /// The stable machine token.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Panicked => "panicked",
            Self::Nondeterministic => "nondeterministic",
            Self::RoundTrip => "round-trip",
            Self::Untyped => "untyped",
        }
    }

    /// Every arm, for the anti-vacuity matrix.
    pub const ALL: [Self; 4] = [
        Self::Panicked,
        Self::Nondeterministic,
        Self::RoundTrip,
        Self::Untyped,
    ];
}

impl fmt::Display for Oracle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// What a target says about an input it accepted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Reencoding {
    /// The target refused the input. There is nothing to re-encode.
    Rejected,
    /// The target accepted, and this encoding MUST equal the input byte for byte.
    ///
    /// The contract of every strictly canonical decoder here: `continuumd`'s JSON and
    /// CBOR readers, CVNF-1, and the snapshot tree.
    Canonical(Vec<u8>),
    /// The target accepted, and this encoding is the input's normal form.
    ///
    /// Weaker than [`Reencoding::Canonical`], and used only where the decoder's own
    /// contract is weaker: `continuum_intent::canonical_json` accepts "any legal JSON
    /// spelling of an admissible document", so byte identity is not owed — a fixpoint is.
    NormalForm(Vec<u8>),
}

/// One target's answer about one input.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Probe {
    /// Where the input landed, as a token from the target's closed vocabulary.
    pub landing: String,
    /// The re-encoding, for the canonicity oracle.
    pub reencoding: Reencoding,
}

impl Probe {
    /// A rejection landing at `landing`.
    pub fn rejected(landing: impl Into<String>) -> Self {
        Self {
            landing: landing.into(),
            reencoding: Reencoding::Rejected,
        }
    }

    /// An acceptance whose re-encoding must equal the input byte for byte.
    #[must_use]
    pub fn canonical(bytes: Vec<u8>) -> Self {
        Self {
            landing: super::target::ACCEPTED.to_owned(),
            reencoding: Reencoding::Canonical(bytes),
        }
    }

    /// An acceptance whose re-encoding is a normal form, not a byte identity.
    #[must_use]
    pub fn normal_form(bytes: Vec<u8>) -> Self {
        Self {
            landing: super::target::ACCEPTED.to_owned(),
            reencoding: Reencoding::NormalForm(bytes),
        }
    }
}

/// What the harness decided about one (target, input) pair.
///
/// The three arms are disjoint by construction and none is a boolean.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CaseOutcome {
    /// Every oracle held. The input landed at `landing`.
    Clean {
        /// The closed-vocabulary landing token.
        landing: String,
    },
    /// An oracle failed. This is what becomes a minimized regression.
    Defect {
        /// Which property failed.
        oracle: Oracle,
        /// What the target reported before the failure, when it reported anything.
        ///
        /// `None` for [`Oracle::Panicked`], where there is no answer to name.
        landing: Option<String>,
    },
    /// The case did not fit the declared budget. Nothing ran and nothing was decided.
    ///
    /// Deliberately *not* a pass: a harness that silently skipped an over-budget case and
    /// reported green would be reporting coverage it does not have.
    BudgetExhausted {
        /// Which ceiling was hit.
        limit: &'static str,
        /// What the case needed.
        needed: usize,
        /// What the budget allows.
        ceiling: usize,
    },
}

impl CaseOutcome {
    /// The stable machine token for this outcome's *class*.
    ///
    /// This is the minimizer's signature: a reduction preserves the outcome exactly when
    /// it preserves this token, so the minimizer cannot turn one defect into a different
    /// one and report the result as the same finding.
    #[must_use]
    pub fn signature(&self) -> String {
        match self {
            Self::Clean { landing } => format!("clean:{landing}"),
            Self::Defect { oracle, landing } => format!(
                "defect:{}:{}",
                oracle.as_str(),
                landing.as_deref().unwrap_or("-")
            ),
            Self::BudgetExhausted { limit, .. } => format!("budget-exhausted:{limit}"),
        }
    }

    /// Whether this outcome is a defect the corpus must record.
    #[must_use]
    pub const fn is_defect(&self) -> bool {
        matches!(self, Self::Defect { .. })
    }

    /// The landing this outcome reports, when it reports one.
    #[must_use]
    pub fn landing(&self) -> Option<&str> {
        match self {
            Self::Clean { landing } => Some(landing),
            Self::Defect { landing, .. } => landing.as_deref(),
            Self::BudgetExhausted { .. } => None,
        }
    }
}

impl fmt::Display for CaseOutcome {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Clean { landing } => write!(f, "clean, landing at `{landing}`"),
            Self::Defect { oracle, landing } => match landing {
                Some(landing) => write!(f, "defect: {oracle} (reported `{landing}`)"),
                None => write!(f, "defect: {oracle}"),
            },
            Self::BudgetExhausted {
                limit,
                needed,
                ceiling,
            } => write!(
                f,
                "budget exhausted: {limit} needs {needed}, ceiling is {ceiling}"
            ),
        }
    }
}
