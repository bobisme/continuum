//! The obligations family — **not instrumented yet** (PR-14-IMPL-04).
//!
//! This file is the whole of that bullet's landing site: see [`crate::family`] for the
//! four steps. Until it lands, [`ObligationEvent`] and [`ObligationReport`] are uninhabited, so no
//! journal can hold one of these events, and a byte string that claims one is refused
//! with the typed [`DecodeError::FamilyNotInstrumented`].

use crate::encoding::{DecodeError, Decoder, EncodeError, Encoder};
use crate::family::Family;
use crate::lift::{LiftContext, LiftStop};
use crate::source::{RecordContext, RecordRefusal};

/// Whether this family's events exist yet.
pub const INSTRUMENTED: bool = false;

/// A obligations event. Uninhabited until PR-14-IMPL-04.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ObligationEvent {}

/// A obligations primitive, as a scripted source reports it. Uninhabited until PR-14-IMPL-04.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ObligationReport {}

/// Recorder state this family keeps. Empty until PR-14-IMPL-04.
#[derive(Debug, Default)]
pub struct RecordState;

/// Lift state this family keeps. Empty until PR-14-IMPL-04.
#[derive(Debug, Default)]
pub struct LiftState;

pub(crate) fn encode(event: &ObligationEvent, _out: &mut Encoder) -> Result<(), EncodeError> {
    match *event {}
}

pub(crate) fn decode(_input: &mut Decoder<'_>, seq: u64) -> Result<ObligationEvent, DecodeError> {
    Err(DecodeError::FamilyNotInstrumented {
        family: Family::Obligation.token(),
        seq,
    })
}

pub(crate) fn render(event: &ObligationEvent) -> String {
    match *event {}
}

pub(crate) fn lift(event: &ObligationEvent, _cx: &mut LiftContext) -> Result<(), LiftStop> {
    match *event {}
}

pub(crate) fn record(
    report: &ObligationReport,
    _cx: &mut RecordContext,
) -> Result<(), RecordRefusal> {
    match *report {}
}
