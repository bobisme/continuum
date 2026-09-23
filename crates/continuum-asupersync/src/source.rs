//! The scripted source: a Continuum-defined stand-in for the substrate, so the journal's
//! canonicality can be held today.
//!
//! # What it is
//!
//! A [`Script`] is one actor's sequence of primitive [`Report`]s. [`record`] interleaves
//! a set of scripts under a [`ChoiceLog`] and writes one semantic event per report.
//! Nothing else feeds it: no clock, no thread, no entropy, no iteration over a hash
//! map. So the journal is a function of `(scripts, log)` and of nothing ambient
//! (INV-005), which is the PR-14 exit property at the grain this crate can reach while
//! the substrate binding is absent ([`crate::binding`]).
//!
//! # What it is not
//!
//! It is not the substrate and does not model it. It does not judge legality: a script
//! that spawns into a closed region is recorded faithfully, and the lift
//! ([`crate::lift`]) is what says the history does not conform. That split is docs/01
//! §6's rule that "the adapter cannot be the sole refinement checker for itself" applied
//! inside this crate: the producer of events and the judge of events are different code.
//!
//! # Refusals
//!
//! A run that cannot be recorded is a typed [`RecordRefusal`] and no journal. A partial
//! journal is never returned, because a prefix presented as a run is a success flag that
//! outruns the evidence (INV-008).

use core::fmt;

use continuum_value::assurance::InconclusiveReason;

use crate::choice::ChoiceLog;
use crate::family::{self, EventBody, Family, Report};
use crate::journal::Journal;

/// One actor's reports, in program order.
pub type Script = Vec<Report>;

/// Why a run could not be recorded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RecordRefusal {
    /// The script reports a primitive of a family whose instrumentation has not landed.
    /// INV-008's *unsupported semantics*: see [`Self::inconclusive_reason`].
    UnsupportedPrimitive {
        /// The family.
        family: Family,
        /// The primitive's name, as the script spells it.
        operation: String,
    },
    /// A report names a region label no earlier report bound.
    UnboundRegion(u32),
    /// A report names a task label no earlier report bound.
    UnboundTask(u32),
    /// A report binds a region label that is already bound.
    RegionLabelRebound(u32),
    /// A report binds a task label that is already bound.
    TaskLabelRebound(u32),
    /// A choice indexes past the enabled actors.
    ChoiceOutOfRange {
        /// Position in the log.
        position: usize,
        /// The choice.
        choice: u32,
        /// How many actors were enabled.
        enabled: usize,
    },
    /// The log ended while reports remained.
    ChoiceLogExhausted {
        /// Reports not yet recorded.
        remaining: usize,
    },
    /// Choices remained after every script finished.
    ChoiceLogOverrun {
        /// Position of the first unused choice.
        position: usize,
    },
    /// The journal could not be extended.
    JournalFull,
}

impl RecordRefusal {
    /// The INV-008 reason this refusal is an instance of, when it is one.
    ///
    /// Only [`Self::UnsupportedPrimitive`] is: the run is outside what the adapter
    /// instruments. Every other arm is a malformed input — a script or a log that does
    /// not describe a run — and has no inconclusive reading.
    #[must_use]
    pub const fn inconclusive_reason(&self) -> Option<InconclusiveReason> {
        match self {
            Self::UnsupportedPrimitive { .. } => Some(InconclusiveReason::Unsupported),
            _ => None,
        }
    }
}

impl fmt::Display for RecordRefusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedPrimitive { family, operation } => write!(
                f,
                "primitive {operation} of family {family} is not instrumented ({})",
                family.requirement()
            ),
            Self::UnboundRegion(label) => write!(f, "region label {label} is not bound"),
            Self::UnboundTask(label) => write!(f, "task label {label} is not bound"),
            Self::RegionLabelRebound(label) => write!(f, "region label {label} is already bound"),
            Self::TaskLabelRebound(label) => write!(f, "task label {label} is already bound"),
            Self::ChoiceOutOfRange {
                position,
                choice,
                enabled,
            } => write!(
                f,
                "choice {choice} at position {position} exceeds the {enabled} enabled actors"
            ),
            Self::ChoiceLogExhausted { remaining } => {
                write!(
                    f,
                    "the choice log ended with {remaining} reports unrecorded"
                )
            }
            Self::ChoiceLogOverrun { position } => {
                write!(
                    f,
                    "choice {position} was not used: every script had finished"
                )
            }
            Self::JournalFull => f.write_str("the journal cannot hold another event"),
        }
    }
}

impl core::error::Error for RecordRefusal {}

/// The recorder's state while a run is recorded: the journal so far, and each family's
/// own bookkeeping. A family that lands adds nothing here; its `RecordState` field
/// already exists.
#[derive(Debug, Default)]
pub struct RecordContext {
    journal: Journal,
    full: bool,
    pub(crate) lifecycle: family::lifecycle::RecordState,
    #[allow(dead_code)] // empty by design: the effect recorder judges nothing
    pub(crate) effect: family::effect::RecordState,
    #[allow(dead_code)] // empty by design: the cancellation recorder judges nothing
    pub(crate) cancellation: family::cancellation::RecordState,
    #[allow(dead_code)] // empty by design: the obligation recorder judges nothing
    pub(crate) obligation: family::obligation::RecordState,
    #[allow(dead_code)] // empty by design: the time recorder judges nothing
    pub(crate) time: family::time::RecordState,
    #[allow(dead_code)] // filled by PR-14-IMPL-06
    pub(crate) channel: family::channel::RecordState,
}

impl RecordContext {
    /// Append one event to the journal being recorded.
    pub(crate) fn append(&mut self, body: EventBody) {
        if self.journal.append(body).is_none() {
            self.full = true;
        }
    }

    /// Whether an append failed.
    pub(crate) const fn is_full(&self) -> bool {
        self.full
    }

    /// The recorded journal, or `None` when an append failed.
    pub(crate) fn into_journal(self) -> Option<Journal> {
        (!self.full).then_some(self.journal)
    }
}

/// Record `scripts` interleaved by `log`.
///
/// # Errors
///
/// A [`RecordRefusal`] naming the first report or choice that could not be recorded.
pub fn record(scripts: &[Script], log: &ChoiceLog) -> Result<Journal, RecordRefusal> {
    let mut cursors = vec![0_usize; scripts.len()];
    let mut cx = RecordContext::default();
    let mut choices = log.choices().iter().enumerate();
    loop {
        let enabled: Vec<usize> = (0..scripts.len())
            .filter(|actor| cursors[*actor] < scripts[*actor].len())
            .collect();
        let Some((position, choice)) = choices.next() else {
            if enabled.is_empty() {
                break;
            }
            let remaining = enabled
                .iter()
                .map(|a| scripts[*a].len() - cursors[*a])
                .sum();
            return Err(RecordRefusal::ChoiceLogExhausted { remaining });
        };
        if enabled.is_empty() {
            return Err(RecordRefusal::ChoiceLogOverrun { position });
        }
        let actor = *enabled
            .get(choice.0 as usize)
            .ok_or(RecordRefusal::ChoiceOutOfRange {
                position,
                choice: choice.0,
                enabled: enabled.len(),
            })?;
        scripts[actor][cursors[actor]].record(&mut cx)?;
        if cx.full {
            return Err(RecordRefusal::JournalFull);
        }
        cursors[actor] += 1;
    }
    Ok(cx.journal)
}
