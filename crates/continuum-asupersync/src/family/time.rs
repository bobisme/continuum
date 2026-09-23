//! The virtual time family — **not instrumented yet** (PR-14-IMPL-05).
//!
//! This file is the whole of that bullet's landing site: see [`crate::family`] for the
//! four steps. Until it lands, [`TimeEvent`] and [`TimeReport`] are uninhabited, so no
//! journal can hold one of these events, and a byte string that claims one is refused
//! with the typed [`DecodeError::FamilyNotInstrumented`].

use crate::encoding::{DecodeError, Decoder, EncodeError, Encoder};
use crate::family::Family;
use crate::lift::{LiftContext, LiftStop};
use crate::source::{RecordContext, RecordRefusal};

/// Whether this family's events exist yet.
pub const INSTRUMENTED: bool = false;

/// A virtual time event. Uninhabited until PR-14-IMPL-05.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TimeEvent {}

/// A virtual time primitive, as a scripted source reports it. Uninhabited until PR-14-IMPL-05.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TimeReport {}

/// Recorder state this family keeps. Empty until PR-14-IMPL-05.
#[derive(Debug, Default)]
pub struct RecordState;

/// Lift state this family keeps. Empty until PR-14-IMPL-05.
#[derive(Debug, Default)]
pub struct LiftState;

pub(crate) fn encode(event: &TimeEvent, _out: &mut Encoder) -> Result<(), EncodeError> {
    match *event {}
}

pub(crate) fn decode(_input: &mut Decoder<'_>, seq: u64) -> Result<TimeEvent, DecodeError> {
    Err(DecodeError::FamilyNotInstrumented {
        family: Family::Time.token(),
        seq,
    })
}

pub(crate) fn render(event: &TimeEvent) -> String {
    match *event {}
}

pub(crate) fn lift(event: &TimeEvent, _cx: &mut LiftContext) -> Result<(), LiftStop> {
    match *event {}
}

pub(crate) fn record(report: &TimeReport, _cx: &mut RecordContext) -> Result<(), RecordRefusal> {
    match *report {}
}
