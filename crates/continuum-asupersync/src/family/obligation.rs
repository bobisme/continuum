//! The obligations family (PR-14-IMPL-04, bn-6nm8).
//!
//! # The vocabulary, and where it comes from
//!
//! docs/01 §6 maps the substrate's *obligation* to "linear resource delta", and docs/02
//! §7 states the invariants a ledger of them must keep:
//!
//! > every obligation has one owner or is discharged; ownership transfer is causal;
//! > region close implies no descendant tasks or obligations.
//!
//! asupersync 0.5.0 keeps that ledger in its runtime: every obligation, of any kind, has
//! a holder task and an owning region, and is `Reserved`, then `Committed`, `Aborted`
//! or `Leaked`. Its trace records each step (`ObligationReserve`, `ObligationCommit`,
//! `ObligationAbort`, `ObligationLeak`) and each ownership transfer (the runtime's
//! `obligation_handoff_v1` trace message). This family journals that ledger:
//!
//! | event | ledger delta | asupersync 0.5.0 |
//! |---|---|---|
//! | [`ObligationEvent::Opened`] | +1 in the region, owned by the holder | `ObligationReserve`, any kind |
//! | [`ObligationEvent::Discharged`] | −1, committed or aborted | `ObligationCommit`, `ObligationAbort` |
//! | [`ObligationEvent::Transferred`] | moves to a new holder and region | the runtime's handoff trace message |
//! | [`ObligationEvent::Leaked`] | −1 as a failure: the holder ended with it open | `ObligationLeak` |
//! | [`ObligationEvent::RegionSettled`] | the region's balance at close | the region's `RegionCloseComplete` |
//!
//! The reserve/commit/abort family (PR-14-IMPL-02) is the *effect phase* of the
//! `Transaction` kind: which staged publication a worker holds. This family is the
//! *ledger*: every kind, who owns each obligation and in which region, transfers, leaks,
//! and each region's balance when it closes. Neither contains the other.
//!
//! # How the events relate to the region calculus
//!
//! `continuum_task::region` has a ledger of its own (`obligation::Ledger`), and its
//! teardown's post-condition `Finalization::is_total` is "no orphan worker, and the
//! ledger balanced". The calculus's ledger holds only the calculus's own obligation
//! kinds and cannot be opened from outside, so the lift keeps the substrate's ledger
//! beside it under the calculus's linear rules and strengthens the post-condition with
//! it (`lift`, `finish`):
//!
//! 1. an obligation is opened once, by a live holder, in the region that owns the
//!    holder, while that region still accepts work;
//! 2. it is discharged or leaked at most once, and only while open; a transfer moves an
//!    open obligation to a different live holder, in the region that owns that holder;
//! 3. a region settles only after the model finalized it, and its reported balance is
//!    the ledger's own: the obligations still open in it and the ones leaked in it;
//! 4. a settled region with an open or leaked obligation violates "region close implies
//!    no obligations": the calculus's `is_total` holds, but the substrate's ledger does
//!    not balance;
//! 5. every region the model finalized has settled.
//!
//! # Identity
//!
//! Obligations are named by dense ordinals in the order the journal allocated them,
//! over every kind. Tasks and regions are the lifecycle family's ordinals.

use std::collections::BTreeMap;
use std::fmt;

use continuum_task::region::worker::WorkerId;
use continuum_task::region::{RegionId, RegionState};

use crate::encoding::{DecodeError, Decoder, EncodeError, Encoder};
use crate::family::EventBody;
use crate::family::lifecycle::{RegionOrdinal, TaskOrdinal};
use crate::lift::{LiftContext, LiftStop, Nonconformance};
use crate::source::{RecordContext, RecordRefusal};

/// Whether this family's events exist yet.
pub const INSTRUMENTED: bool = true;

/// An obligation, by its position in the journal's allocation order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ObligationOrdinal(pub u32);

/// The substrate's obligation kinds, as the journal names them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ObligationKind {
    /// A channel send permit.
    SendPermit,
    /// An acknowledgement owed for a received message.
    Ack,
    /// A lease on a resource.
    Lease,
    /// A pending I/O operation.
    IoOp,
    /// A semaphore permit.
    SemaphorePermit,
    /// An open transaction: the reserve/commit/abort family's effect.
    Transaction,
}

impl ObligationKind {
    /// Every kind, in tag order.
    pub const ALL: [Self; 6] = [
        Self::SendPermit,
        Self::Ack,
        Self::Lease,
        Self::IoOp,
        Self::SemaphorePermit,
        Self::Transaction,
    ];

    const fn tag(self) -> u8 {
        match self {
            Self::SendPermit => 1,
            Self::Ack => 2,
            Self::Lease => 3,
            Self::IoOp => 4,
            Self::SemaphorePermit => 5,
            Self::Transaction => 6,
        }
    }

    const fn from_tag(tag: u8) -> Option<Self> {
        match tag {
            1 => Some(Self::SendPermit),
            2 => Some(Self::Ack),
            3 => Some(Self::Lease),
            4 => Some(Self::IoOp),
            5 => Some(Self::SemaphorePermit),
            6 => Some(Self::Transaction),
            _ => None,
        }
    }

    /// A stable token.
    #[must_use]
    pub const fn token(self) -> &'static str {
        match self {
            Self::SendPermit => "send-permit",
            Self::Ack => "ack",
            Self::Lease => "lease",
            Self::IoOp => "io-op",
            Self::SemaphorePermit => "semaphore-permit",
            Self::Transaction => "transaction",
        }
    }
}

impl fmt::Display for ObligationKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.token())
    }
}

/// How an obligation was discharged.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Discharge {
    /// Committed.
    Committed,
    /// Aborted, for any reason.
    Aborted,
}

impl Discharge {
    const fn tag(self) -> u8 {
        match self {
            Self::Committed => 1,
            Self::Aborted => 2,
        }
    }

    const fn from_tag(tag: u8) -> Option<Self> {
        match tag {
            1 => Some(Self::Committed),
            2 => Some(Self::Aborted),
            _ => None,
        }
    }

    /// A stable token.
    #[must_use]
    pub const fn token(self) -> &'static str {
        match self {
            Self::Committed => "committed",
            Self::Aborted => "aborted",
        }
    }
}

/// A set of obligations, held strictly ascending so it has one spelling.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ObligationSet(Vec<ObligationOrdinal>);

impl ObligationSet {
    /// The set of these obligations, in canonical order.
    #[must_use]
    pub fn new(members: impl IntoIterator<Item = ObligationOrdinal>) -> Self {
        let mut members: Vec<ObligationOrdinal> = members.into_iter().collect();
        members.sort_unstable();
        members.dedup();
        Self(members)
    }

    /// The members, ascending.
    #[must_use]
    pub fn as_slice(&self) -> &[ObligationOrdinal] {
        &self.0
    }

    /// Whether the set is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

/// One obligation-ledger event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ObligationEvent {
    /// A new obligation, owned by `holder`, in `region`.
    Opened {
        /// The new obligation.
        obligation: ObligationOrdinal,
        /// Its kind.
        kind: ObligationKind,
        /// The task that holds it.
        holder: TaskOrdinal,
        /// The region that owns it.
        region: RegionOrdinal,
    },
    /// The obligation was discharged.
    Discharged {
        /// The obligation.
        obligation: ObligationOrdinal,
        /// How.
        how: Discharge,
    },
    /// The obligation moved to a new holder and region.
    Transferred {
        /// The obligation.
        obligation: ObligationOrdinal,
        /// The new holder.
        holder: TaskOrdinal,
        /// The region that now owns it.
        region: RegionOrdinal,
    },
    /// The holder ended with the obligation open: the substrate recorded a leak.
    Leaked {
        /// The obligation.
        obligation: ObligationOrdinal,
    },
    /// A region closed, with this balance.
    RegionSettled {
        /// The region.
        region: RegionOrdinal,
        /// Obligations still open in it.
        open: ObligationSet,
        /// Obligations leaked in it.
        leaked: ObligationSet,
    },
}

impl ObligationEvent {
    const fn tag(&self) -> u8 {
        match self {
            Self::Opened { .. } => 1,
            Self::Discharged { .. } => 2,
            Self::Transferred { .. } => 3,
            Self::Leaked { .. } => 4,
            Self::RegionSettled { .. } => 5,
        }
    }

    const fn token(&self) -> &'static str {
        match self {
            Self::Opened { .. } => "opened",
            Self::Discharged { .. } => "discharged",
            Self::Transferred { .. } => "transferred",
            Self::Leaked { .. } => "leaked",
            Self::RegionSettled { .. } => "region-settled",
        }
    }
}

fn encode_set(set: &ObligationSet, out: &mut Encoder) -> Result<(), EncodeError> {
    let count =
        u32::try_from(set.0.len()).map_err(|_| EncodeError::FieldTooLong { len: set.0.len() })?;
    out.u32(count);
    for member in &set.0 {
        out.u32(member.0);
    }
    Ok(())
}

pub(crate) fn encode(event: &ObligationEvent, out: &mut Encoder) -> Result<(), EncodeError> {
    out.tag(event.tag());
    match event {
        ObligationEvent::Opened {
            obligation,
            kind,
            holder,
            region,
        } => {
            out.u32(obligation.0);
            out.tag(kind.tag());
            out.u32(holder.0);
            out.u32(region.0);
        }
        ObligationEvent::Discharged { obligation, how } => {
            out.u32(obligation.0);
            out.tag(how.tag());
        }
        ObligationEvent::Transferred {
            obligation,
            holder,
            region,
        } => {
            out.u32(obligation.0);
            out.u32(holder.0);
            out.u32(region.0);
        }
        ObligationEvent::Leaked { obligation } => out.u32(obligation.0),
        ObligationEvent::RegionSettled {
            region,
            open,
            leaked,
        } => {
            out.u32(region.0);
            encode_set(open, out)?;
            encode_set(leaked, out)?;
        }
    }
    Ok(())
}

fn decode_set(input: &mut Decoder<'_>) -> Result<ObligationSet, DecodeError> {
    let at = input.offset();
    let count = input.u32()?;
    let mut members: Vec<ObligationOrdinal> = Vec::new();
    for _ in 0..count {
        let member = ObligationOrdinal(input.u32()?);
        if members.last().is_some_and(|last| *last >= member) {
            return Err(DecodeError::UnsortedSet { at });
        }
        members.push(member);
    }
    Ok(ObligationSet(members))
}

fn decode_tag<T>(
    input: &mut Decoder<'_>,
    table: &'static str,
    from: fn(u8) -> Option<T>,
) -> Result<T, DecodeError> {
    let at = input.offset();
    let tag = input.tag()?;
    from(tag).ok_or(DecodeError::UnknownTag { table, tag, at })
}

pub(crate) fn decode(input: &mut Decoder<'_>, _seq: u64) -> Result<ObligationEvent, DecodeError> {
    let at = input.offset();
    let tag = input.tag()?;
    Ok(match tag {
        1 => ObligationEvent::Opened {
            obligation: ObligationOrdinal(input.u32()?),
            kind: decode_tag(input, "obligation kind", ObligationKind::from_tag)?,
            holder: TaskOrdinal(input.u32()?),
            region: RegionOrdinal(input.u32()?),
        },
        2 => ObligationEvent::Discharged {
            obligation: ObligationOrdinal(input.u32()?),
            how: decode_tag(input, "discharge", Discharge::from_tag)?,
        },
        3 => ObligationEvent::Transferred {
            obligation: ObligationOrdinal(input.u32()?),
            holder: TaskOrdinal(input.u32()?),
            region: RegionOrdinal(input.u32()?),
        },
        4 => ObligationEvent::Leaked {
            obligation: ObligationOrdinal(input.u32()?),
        },
        5 => ObligationEvent::RegionSettled {
            region: RegionOrdinal(input.u32()?),
            open: decode_set(input)?,
            leaked: decode_set(input)?,
        },
        other => {
            return Err(DecodeError::UnknownTag {
                table: "obligation event",
                tag: other,
                at,
            });
        }
    })
}

fn render_set(set: &ObligationSet) -> String {
    let members: Vec<String> = set.0.iter().map(|o| format!("o{}", o.0)).collect();
    format!("[{}]", members.join(","))
}

pub(crate) fn render(event: &ObligationEvent) -> String {
    match event {
        ObligationEvent::Opened {
            obligation,
            kind,
            holder,
            region,
        } => format!(
            "opened o{} {kind} holder=t{} region=r{}",
            obligation.0, holder.0, region.0
        ),
        ObligationEvent::Discharged { obligation, how } => {
            format!("discharged o{} {}", obligation.0, how.token())
        }
        ObligationEvent::Transferred {
            obligation,
            holder,
            region,
        } => format!(
            "transferred o{} holder=t{} region=r{}",
            obligation.0, holder.0, region.0
        ),
        ObligationEvent::Leaked { obligation } => format!("leaked o{}", obligation.0),
        ObligationEvent::RegionSettled {
            region,
            open,
            leaked,
        } => format!(
            "region-settled r{} open={} leaked={}",
            region.0,
            render_set(open),
            render_set(leaked)
        ),
    }
}

// --- lift ----------------------------------------------------------------------------

/// Where one obligation is, as the journal so far says.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum State {
    Open,
    Discharged,
    Leaked,
}

impl State {
    const fn token(self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::Discharged => "discharged",
            Self::Leaked => "leaked",
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct Entry {
    kind: ObligationKind,
    holder: u32,
    region: u32,
    state: State,
}

/// Why an obligation-ledger event does not conform.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LedgerFault {
    /// A new obligation is not named by the next ordinal.
    ObligationIdentity {
        /// The journal's ordinal.
        journal: u32,
        /// The next ordinal.
        expected: u32,
    },
    /// The event names an obligation no earlier event opened.
    UnknownObligation {
        /// The obligation.
        obligation: u32,
    },
    /// The obligation is not open, so it cannot be discharged, leaked or transferred.
    NotOpen {
        /// The obligation.
        obligation: u32,
        /// The event's token.
        event: &'static str,
        /// Its state.
        state: &'static str,
    },
    /// The holder is terminal in the model, or completed as cancelled, so it cannot own
    /// an obligation.
    HolderTerminal {
        /// The obligation.
        obligation: u32,
        /// The holder.
        holder: u32,
    },
    /// The journal places the obligation in a region that does not own its holder.
    HolderRegionMismatch {
        /// The obligation.
        obligation: u32,
        /// The region the journal names.
        journal: u32,
        /// The region that owns the holder in the model.
        model: u32,
    },
    /// An obligation opened in a region that no longer accepts work.
    RegionNotOpen {
        /// The obligation.
        obligation: u32,
        /// The region.
        region: u32,
        /// The model's token for its state.
        state: &'static str,
    },
    /// A transfer names the holder the obligation already has.
    SameHolder {
        /// The obligation.
        obligation: u32,
    },
    /// A region settled before the model finalized it.
    SettledBeforeFinalized {
        /// The region.
        region: u32,
        /// The model's token for its state.
        state: &'static str,
    },
    /// A region settled twice.
    SettledTwice {
        /// The region.
        region: u32,
    },
    /// A region's reported balance is not the ledger's.
    BalanceMismatch {
        /// The region.
        region: u32,
        /// Reported open and leaked obligations.
        reported: (Vec<u32>, Vec<u32>),
        /// The ledger's open and leaked obligations.
        ledger: (Vec<u32>, Vec<u32>),
    },
    /// A region closed while obligations in it were open or leaked: "region close
    /// implies no descendant tasks or obligations" does not hold.
    UnbalancedAtClose {
        /// The region.
        region: u32,
        /// Obligations still open.
        open: Vec<u32>,
        /// Obligations leaked.
        leaked: Vec<u32>,
    },
    /// The model finalized a region that never settled.
    Unsettled {
        /// The region.
        region: u32,
    },
}

impl fmt::Display for LedgerFault {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ObligationIdentity { journal, expected } => write!(
                f,
                "the journal opened o{journal} but the next obligation is o{expected}"
            ),
            Self::UnknownObligation { obligation } => {
                write!(f, "o{obligation} was never opened")
            }
            Self::NotOpen {
                obligation,
                event,
                state,
            } => write!(f, "o{obligation} is {state}, so it cannot be {event}"),
            Self::HolderTerminal { obligation, holder } => write!(
                f,
                "o{obligation}'s holder t{holder} is terminal in the model"
            ),
            Self::HolderRegionMismatch {
                obligation,
                journal,
                model,
            } => write!(
                f,
                "o{obligation} is placed in r{journal} but its holder belongs to r{model}"
            ),
            Self::RegionNotOpen {
                obligation,
                region,
                state,
            } => write!(f, "o{obligation} opened in r{region}, which is {state}"),
            Self::SameHolder { obligation } => {
                write!(
                    f,
                    "o{obligation} is transferred to the holder it already has"
                )
            }
            Self::SettledBeforeFinalized { region, state } => {
                write!(f, "r{region} settled while it is {state}, not finalized")
            }
            Self::SettledTwice { region } => write!(f, "r{region} settled twice"),
            Self::BalanceMismatch {
                region,
                reported,
                ledger,
            } => write!(
                f,
                "r{region} reports open/leaked {reported:?} but the ledger has {ledger:?}"
            ),
            Self::UnbalancedAtClose {
                region,
                open,
                leaked,
            } => write!(
                f,
                "r{region} closed with open {open:?} and leaked {leaked:?} obligations"
            ),
            Self::Unsettled { region } => {
                write!(f, "r{region} was finalized but never settled")
            }
        }
    }
}

/// Lift state: the substrate's ledger, and which regions settled.
#[derive(Debug, Default)]
pub struct LiftState {
    entries: BTreeMap<u32, Entry>,
    /// Send permits each task committed and has not yet sent with (bn-1i050).
    permits: BTreeMap<u32, u32>,
    settled: BTreeMap<u32, bool>,
    present: bool,
}

fn fault(fault: LedgerFault) -> LiftStop {
    LiftStop::Violation(Nonconformance::Obligation(fault))
}

/// A channel send is the commit of its sender's `SendPermit` (docs/02 §7 two-phase
/// effect): when the journal carries this family, consume one committed, unused send
/// permit of `task`. `false` when there is none.
pub(crate) fn take_send_permit(cx: &mut LiftContext, task: u32) -> bool {
    if !cx.obligation.present {
        return true;
    }
    match cx.obligation.permits.get_mut(&task) {
        Some(count) if *count > 0 => {
            *count -= 1;
            true
        }
        _ => false,
    }
}

/// A task that completes as cancelled holds no open obligation (docs/02 §7
/// `obligations == ∅ → Cancelled`); one it holds is [`LedgerFault::HolderTerminal`].
pub(crate) fn check_none_held(cx: &LiftContext, task: u32) -> Result<(), LiftStop> {
    match cx
        .obligation
        .entries
        .iter()
        .find(|(_, entry)| entry.holder == task && entry.state == State::Open)
    {
        Some((obligation, _)) => Err(fault(LedgerFault::HolderTerminal {
            obligation: *obligation,
            holder: task,
        })),
        None => Ok(()),
    }
}

/// The holder must be live in the model and owned by `region`.
fn check_holder(
    cx: &LiftContext,
    obligation: u32,
    holder: u32,
    region: u32,
) -> Result<(), LiftStop> {
    let worker = WorkerId::at(holder);
    if cx.tree.worker_state(worker)?.is_terminal() {
        return Err(fault(LedgerFault::HolderTerminal { obligation, holder }));
    }
    let owner = cx.tree.owner(worker)?.ordinal();
    if owner != region {
        return Err(fault(LedgerFault::HolderRegionMismatch {
            obligation,
            journal: region,
            model: owner,
        }));
    }
    Ok(())
}

fn open_entry(
    cx: &LiftContext,
    obligation: u32,
    event: &ObligationEvent,
) -> Result<Entry, LiftStop> {
    let Some(entry) = cx.obligation.entries.get(&obligation).copied() else {
        return Err(fault(LedgerFault::UnknownObligation { obligation }));
    };
    if entry.state != State::Open {
        return Err(fault(LedgerFault::NotOpen {
            obligation,
            event: event.token(),
            state: entry.state.token(),
        }));
    }
    Ok(entry)
}

fn members(set: &ObligationSet) -> Vec<u32> {
    set.0.iter().map(|o| o.0).collect()
}

pub(crate) fn lift(event: &ObligationEvent, cx: &mut LiftContext) -> Result<(), LiftStop> {
    cx.obligation.present = true;
    match event {
        ObligationEvent::Opened {
            obligation,
            kind,
            holder,
            region,
        } => {
            let expected = u32::try_from(cx.obligation.entries.len()).unwrap_or(u32::MAX);
            if obligation.0 != expected {
                return Err(fault(LedgerFault::ObligationIdentity {
                    journal: obligation.0,
                    expected,
                }));
            }
            check_holder(cx, obligation.0, holder.0, region.0)?;
            let state = cx.tree.state(RegionId::at(region.0))?;
            if !state.accepts_work() {
                return Err(fault(LedgerFault::RegionNotOpen {
                    obligation: obligation.0,
                    region: region.0,
                    state: state.token(),
                }));
            }
            cx.obligation.entries.insert(
                obligation.0,
                Entry {
                    kind: *kind,
                    holder: holder.0,
                    region: region.0,
                    state: State::Open,
                },
            );
        }
        ObligationEvent::Discharged { obligation, .. } | ObligationEvent::Leaked { obligation } => {
            let mut entry = open_entry(cx, obligation.0, event)?;
            if let ObligationEvent::Discharged { how, .. } = event {
                check_holder(cx, obligation.0, entry.holder, entry.region)?;
                entry.state = State::Discharged;
                if *how == Discharge::Committed && entry.kind == ObligationKind::SendPermit {
                    *cx.obligation.permits.entry(entry.holder).or_insert(0) += 1;
                }
            } else {
                entry.state = State::Leaked;
            }
            cx.obligation.entries.insert(obligation.0, entry);
        }
        ObligationEvent::Transferred {
            obligation,
            holder,
            region,
        } => {
            let mut entry = open_entry(cx, obligation.0, event)?;
            if entry.holder == holder.0 {
                return Err(fault(LedgerFault::SameHolder {
                    obligation: obligation.0,
                }));
            }
            check_holder(cx, obligation.0, holder.0, region.0)?;
            entry.holder = holder.0;
            entry.region = region.0;
            cx.obligation.entries.insert(obligation.0, entry);
        }
        ObligationEvent::RegionSettled {
            region,
            open,
            leaked,
        } => {
            let state = cx.tree.state(RegionId::at(region.0))?;
            if state != RegionState::Finalized {
                return Err(fault(LedgerFault::SettledBeforeFinalized {
                    region: region.0,
                    state: state.token(),
                }));
            }
            if cx.obligation.settled.insert(region.0, true).is_some() {
                return Err(fault(LedgerFault::SettledTwice { region: region.0 }));
            }
            let in_region = |wanted: State| -> Vec<u32> {
                cx.obligation
                    .entries
                    .iter()
                    .filter(|(_, e)| e.region == region.0 && e.state == wanted)
                    .map(|(o, _)| *o)
                    .collect()
            };
            let ledger = (in_region(State::Open), in_region(State::Leaked));
            let reported = (members(open), members(leaked));
            if reported != ledger {
                return Err(fault(LedgerFault::BalanceMismatch {
                    region: region.0,
                    reported,
                    ledger,
                }));
            }
            if !open.is_empty() || !leaked.is_empty() {
                return Err(fault(LedgerFault::UnbalancedAtClose {
                    region: region.0,
                    open: ledger.0,
                    leaked: ledger.1,
                }));
            }
        }
    }
    Ok(())
}

/// The whole-journal check, run after the last event: when the journal carries this
/// family, every region the model finalized has settled.
pub(crate) fn finish(cx: &LiftContext) -> Result<(), LiftStop> {
    if !cx.obligation.present {
        return Ok(());
    }
    let count = u32::try_from(cx.tree.region_count()).unwrap_or(u32::MAX);
    for region in 0..count {
        if cx.tree.state(RegionId::at(region))? == RegionState::Finalized
            && !cx.obligation.settled.contains_key(&region)
        {
            return Err(fault(LedgerFault::Unsettled { region }));
        }
    }
    Ok(())
}

// --- the scripted source -------------------------------------------------------------

/// An obligation-ledger step, as a scripted source reports it: the event itself, named
/// by journal ordinals.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObligationReport(pub ObligationEvent);

/// Recorder state this family keeps: none. The recorder judges nothing.
#[derive(Debug, Default)]
pub struct RecordState;

pub(crate) fn record(
    report: &ObligationReport,
    cx: &mut RecordContext,
) -> Result<(), RecordRefusal> {
    cx.append(EventBody::Obligation(report.0.clone()));
    Ok(())
}
