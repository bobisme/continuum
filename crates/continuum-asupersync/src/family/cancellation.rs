//! The cancellation phases family — **not instrumented yet** (PR-14-IMPL-03).
//!
//! This file is the whole of that bullet's landing site: see [`crate::family`] for the
//! four steps. Until it lands, [`CancellationEvent`] and [`CancellationReport`] are uninhabited, so no
//! journal can hold one of these events, and a byte string that claims one is refused
//! with the typed [`DecodeError::FamilyNotInstrumented`].

use crate::encoding::{DecodeError, Decoder, EncodeError, Encoder};
use crate::family::Family;
use crate::lift::{LiftContext, LiftStop};
use crate::source::{RecordContext, RecordRefusal};

/// Whether this family's events exist yet.
pub const INSTRUMENTED: bool = false;

/// A cancellation phases event. Uninhabited until PR-14-IMPL-03.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CancellationEvent {}

/// A cancellation phases primitive, as a scripted source reports it. Uninhabited until PR-14-IMPL-03.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CancellationReport {}

/// Recorder state this family keeps. Empty until PR-14-IMPL-03.
#[derive(Debug, Default)]
pub struct RecordState;

/// Lift state this family keeps. Empty until PR-14-IMPL-03.
#[derive(Debug, Default)]
pub struct LiftState;

pub(crate) fn encode(event: &CancellationEvent, _out: &mut Encoder) -> Result<(), EncodeError> {
    match *event {}
}

pub(crate) fn decode(_input: &mut Decoder<'_>, seq: u64) -> Result<CancellationEvent, DecodeError> {
    Err(DecodeError::FamilyNotInstrumented {
        family: Family::Cancellation.token(),
        seq,
    })
}

pub(crate) fn render(event: &CancellationEvent) -> String {
    match *event {}
}

pub(crate) fn lift(event: &CancellationEvent, _cx: &mut LiftContext) -> Result<(), LiftStop> {
    match *event {}
}

pub(crate) fn record(
    report: &CancellationReport,
    _cx: &mut RecordContext,
) -> Result<(), RecordRefusal> {
    match *report {}
}
