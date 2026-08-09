//! The task-assembly lane: turning an Intent Contract into a synthesis task, or
//! refusing to (plan §14.2, §14.5; RFC 0037 AO2; INV-012).
//!
//! # The obligation this lane exists to discharge
//!
//! > **AO2 — The obligation belongs to the caller with standing, and that caller is
//! > named.** […] A lane that assembles a synthesis task against a contract MUST
//! > require `optimization.non_vacuity` to be non-empty and MUST refuse the task
//! > otherwise, with a typed refusal naming INV-012. A lane that assembles no synthesis
//! > task MUST NOT impose it […]
//! >
//! > — RFC 0037, "Acceptance obligations that are not well-formedness"
//!
//! `continuum-intent` implements the obligation as
//! [`Optimization::require_non_vacuity`]; before this lane landed, nothing outside
//! tests called it, which RFC 0037 recorded as flag F12. [`assemble`] is that caller.
//!
//! # Two calls, and why both are made here
//!
//! RFC 0037 correction 17 ratified a *two-call* contract rather than a sixth
//! well-formedness rule:
//!
//! > **AO3 — A well-formedness verdict is not an acceptance decision and MUST NOT be
//! > read as one.** The verdict names the rules it decided; a caller that needs both
//! > answers makes both calls.
//!
//! [`assemble`] needs both answers, so it makes both calls:
//!
//! 1. [`IntentContract::check`] — the document's five cross-field rules (W2, W3, W7,
//!    W8, W9). Answers "can this be read as a contract at all".
//! 2. [`Optimization::require_non_vacuity`] — INV-012's obligation. Answers "may *this*
//!    lane act on it".
//!
//! Both run before either is allowed to decide. The second call is deliberately not
//! behind an early return on the first: when a document fails both, the refusal carries
//! both grounds ([`NotWellFormed::vacuous`]), so "the lane made the second call" is an
//! observable property of the returned value rather than a claim about the source. A
//! lane that dropped the second call would still refuse every ill-formed document and
//! would silently accept the one document INV-012 is about — a contract that is clean
//! on all five rules and requires no behavior to remain possible.
//!
//! # Why refusing is the whole job here
//!
//! Nothing in this module judges *what* a declared behavior says. The three
//! `optimization` sets hold opaque strings classified by set membership (RFC 0031), so
//! the obligation is a presence requirement and a one-character behavior discharges it
//! exactly as a real progress scenario does — RFC 0037 AO3 says so outright, and states
//! that judging a declared behavior's content is Forge's own job at synthesis time
//! (plan §14.5's "property mutation and hidden semantic variants detect overfitting").
//! That defense is unlanded; this lane does not pretend otherwise, and
//! [`SynthesisTask::positive_behaviors`] hands the declared behaviors on verbatim for
//! the search that will eventually mutate them.
//!
//! # The refusal's shape, and what this crate does not decide
//!
//! RFC 0033 (Continuum Forge) owns the surface that must call the obligation and the
//! refusal that surface returns; today it is a research-implementation sketch that
//! names the loop's "non-vacuity screen" and fixes no wire vocabulary. So this module
//! delivers the refusal as a *typed value* — [`TaskRefusal`], never a bare boolean and
//! never a bare message (INV-008) — with the invariant it names available as data
//! ([`VacuousIntent::invariant`]) rather than only as prose inside a `Display`.
//!
//! What it deliberately does not do is invent a wire spelling.
//! `schemas/continuumd-native-protocol.idl` declares no error code for a refused
//! synthesis task and no `forge` namespace, this crate takes no protocol dependency,
//! and picking an `ErrorCode` for a verb that does not exist would put a second
//! authority over a vocabulary RFC 0026 and RFC 0033 own. When the Forge verb lands,
//! [`TaskRefusal`] is the value it renders.
//!
//! [`Optimization::require_non_vacuity`]: continuum_intent::optimization::Optimization::require_non_vacuity
//! [`IntentContract::check`]: continuum_intent::contract::IntentContract::check

use core::fmt;

use continuum_intent::contract::{CheckEnvironment, ContractVerdict, IntentContract, IntentId};
use continuum_intent::optimization::{NonVacuityObligation, Objective};

/// The invariant a [`VacuousIntent`] refusal names.
///
/// Exposed as data, and as the exact token `plan.md` spells, so "the refusal names
/// INV-012" is checkable by a caller and by a test rather than being a substring of an
/// error message that a rewording could drop.
const NON_VACUITY_INVARIANT: &str = "INV-012";

/// A synthesis task whose fixed part is an Intent Contract that cleared both calls.
///
/// Constructible only by [`assemble`]: the fields are private and there is no public
/// constructor, so a `SynthesisTask` value *is* the evidence that the non-vacuity
/// obligation was required and met. [`positive_behaviors`](Self::positive_behaviors) is
/// non-empty for every value of this type — plan §14.5's "Every synthesis task includes
/// positive behaviors or progress scenarios", held by construction rather than by
/// convention.
///
/// This is plan §14.2's `fixed` and `objectives` blocks as far as a contract can supply
/// them, and no further: the `holes` block — the typed grammar or sketch locations a
/// search enumerates — is not a fact the contract carries, and the `archive` is PR 29's.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SynthesisTask {
    intent_id: IntentId,
    positive_behaviors: Vec<NonVacuityObligation>,
    hard: Vec<Objective>,
    soft: Vec<Objective>,
}

impl SynthesisTask {
    /// The handle of the contract this task is assembled against.
    #[must_use]
    pub const fn intent_id(&self) -> &IntentId {
        &self.intent_id
    }

    /// The declared non-vacuity behaviors, in the contract's canonical order.
    ///
    /// Never empty. Verbatim, and unjudged: see the module documentation on why content
    /// is the search's question and not this lane's.
    #[must_use]
    pub fn positive_behaviors(&self) -> &[NonVacuityObligation] {
        &self.positive_behaviors
    }

    /// `optimization.hard` — the constraints a candidate must satisfy.
    #[must_use]
    pub fn hard_objectives(&self) -> &[Objective] {
        &self.hard
    }

    /// `optimization.soft` — the objectives a candidate is ranked by.
    #[must_use]
    pub fn soft_objectives(&self) -> &[Objective] {
        &self.soft
    }
}

/// INV-012's refusal: the contract requires no behavior to remain possible, so no
/// synthesis task may be assembled against it.
///
/// Not a defect in the document. An empty `optimization.non_vacuity` is well formed
/// (RFC 0037 AO1) and a contract that governs only verification may carry one
/// legitimately; what this value says is narrower and exact — *this* lane may not act
/// on it. [`invariant`](Self::invariant) names the invariant that denies it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VacuousIntent {
    intent_id: IntentId,
}

impl VacuousIntent {
    /// The handle of the contract that was refused.
    #[must_use]
    pub const fn intent_id(&self) -> &IntentId {
        &self.intent_id
    }

    /// The invariant this refusal names: `INV-012`.
    #[must_use]
    pub const fn invariant(&self) -> &'static str {
        NON_VACUITY_INVARIANT
    }
}

impl fmt::Display for VacuousIntent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "no synthesis task may be assembled against `{}`: it declares no \
             optimization.non_vacuity behavior, and a system that admits no behavior \
             satisfies every safety claim ({NON_VACUITY_INVARIANT}, plan §14.5) — declare \
             a required progress or availability behavior, or assemble no task",
            self.intent_id.as_str(),
        )
    }
}

impl core::error::Error for VacuousIntent {}

/// The first call's refusal: the document does not satisfy the five cross-field rules,
/// so it cannot be read as a contract.
///
/// Distinct from [`VacuousIntent`] on purpose (INV-008): "this is not a contract" and
/// "this is a contract this lane may not act on" are different outcomes with different
/// remedies, and neither is a degenerate case of the other.
///
/// [`vacuous`](Self::vacuous) records the *second* call's answer for the same document.
/// It is `Some` when the contract is both ill formed and vacuous, and its presence is
/// what makes "the lane made both calls" observable: a lane that short-circuited on the
/// verdict could never populate it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NotWellFormed {
    intent_id: IntentId,
    verdict: ContractVerdict,
    vacuous: Option<VacuousIntent>,
}

impl NotWellFormed {
    /// The handle the document declares.
    #[must_use]
    pub const fn intent_id(&self) -> &IntentId {
        &self.intent_id
    }

    /// The checker's own verdict, relayed rather than re-spelled: `continuum-intent`
    /// owns the W-rule vocabulary and this crate does not enumerate it.
    #[must_use]
    pub const fn verdict(&self) -> &ContractVerdict {
        &self.verdict
    }

    /// The non-vacuity refusal for the same document, when that call also failed.
    #[must_use]
    pub const fn vacuous(&self) -> Option<&VacuousIntent> {
        self.vacuous.as_ref()
    }
}

impl fmt::Display for NotWellFormed {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "no synthesis task may be assembled against `{}`: {}",
            self.intent_id.as_str(),
            self.verdict,
        )?;
        if let Some(vacuous) = &self.vacuous {
            write!(
                f,
                "; and separately, it declares no optimization.non_vacuity behavior \
                 ({})",
                vacuous.invariant(),
            )?;
        }
        Ok(())
    }
}

impl core::error::Error for NotWellFormed {}

/// Why [`assemble`] refused to build a task.
///
/// Two variants carrying two distinct payload types rather than one struct with a
/// reason tag, so a caller that matches an arm holds a value that could not have been
/// the other one (INV-008: distinct outcomes stay distinct in the type).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TaskRefusal {
    /// The first call refused: the document fails a cross-field rule.
    NotWellFormed(NotWellFormed),
    /// The second call refused: INV-012's obligation is not met.
    Vacuous(VacuousIntent),
}

impl TaskRefusal {
    /// The handle of the contract that was refused, whichever call refused it.
    #[must_use]
    pub const fn intent_id(&self) -> &IntentId {
        match self {
            Self::NotWellFormed(refusal) => refusal.intent_id(),
            Self::Vacuous(refusal) => refusal.intent_id(),
        }
    }

    /// The INV-012 refusal this refusal carries, if any.
    ///
    /// `Some` for [`TaskRefusal::Vacuous`], and also for a
    /// [`TaskRefusal::NotWellFormed`] whose second call failed too — so a caller asking
    /// "was the non-vacuity obligation met?" gets the same answer regardless of which
    /// call refused first, and never has to infer it from the absence of a variant.
    #[must_use]
    pub const fn vacuous(&self) -> Option<&VacuousIntent> {
        match self {
            Self::NotWellFormed(refusal) => refusal.vacuous(),
            Self::Vacuous(refusal) => Some(refusal),
        }
    }
}

impl fmt::Display for TaskRefusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotWellFormed(refusal) => refusal.fmt(f),
            Self::Vacuous(refusal) => refusal.fmt(f),
        }
    }
}

impl core::error::Error for TaskRefusal {}

impl From<NotWellFormed> for TaskRefusal {
    fn from(refusal: NotWellFormed) -> Self {
        Self::NotWellFormed(refusal)
    }
}

impl From<VacuousIntent> for TaskRefusal {
    fn from(refusal: VacuousIntent) -> Self {
        Self::Vacuous(refusal)
    }
}

/// Assemble a synthesis task against `contract`, or refuse with a typed refusal.
///
/// Makes both calls RFC 0037 AO3 requires of a caller that needs both answers, in this
/// order and both before either decides:
///
/// 1. [`IntentContract::check`] against `environment` — W2, W3, W7, W8, W9.
/// 2. [`Optimization::require_non_vacuity`] — INV-012.
///
/// Determinism: a pure function of `contract` and `environment`. It reads no clock, no
/// entropy, no file, and no network, and holds no state between calls, so two runs over
/// equal inputs return equal results (INV-005).
///
/// # Errors
///
/// [`TaskRefusal::NotWellFormed`] when the contract fails any cross-field rule — with
/// [`NotWellFormed::vacuous`] populated when INV-012's obligation also failed;
/// [`TaskRefusal::Vacuous`] when the contract is well formed and
/// `optimization.non_vacuity` is empty.
///
/// [`Optimization::require_non_vacuity`]: continuum_intent::optimization::Optimization::require_non_vacuity
/// [`IntentContract::check`]: continuum_intent::contract::IntentContract::check
pub fn assemble(
    contract: &IntentContract,
    environment: &CheckEnvironment,
) -> Result<SynthesisTask, TaskRefusal> {
    // Both calls, unconditionally, before either decides. See the module documentation:
    // making the obligation a separate statement rather than a `?`-chained step after
    // the verdict is what lets a refusal report both grounds, which is in turn what
    // makes the second call observable from the returned value.
    let verdict = contract.check(environment);
    let non_vacuity = contract.optimization().require_non_vacuity();

    let vacuous = non_vacuity.err().map(|_| VacuousIntent {
        intent_id: contract.intent_id().clone(),
    });

    if let Err(verdict) = verdict.into_result() {
        return Err(TaskRefusal::NotWellFormed(NotWellFormed {
            intent_id: contract.intent_id().clone(),
            verdict,
            vacuous,
        }));
    }
    if let Some(vacuous) = vacuous {
        return Err(TaskRefusal::Vacuous(vacuous));
    }

    let optimization = contract.optimization();
    Ok(SynthesisTask {
        intent_id: contract.intent_id().clone(),
        positive_behaviors: optimization.non_vacuity().cloned().collect(),
        hard: optimization.hard().cloned().collect(),
        soft: optimization.soft().cloned().collect(),
    })
}
