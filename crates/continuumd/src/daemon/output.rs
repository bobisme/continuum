//! `OutputPolicy.max_bytes`, enforced — the ceiling, the trim, and the two typed outcomes.
//!
//! > **`output_policy` bounds the payload, never the truth.** `max_bytes` is the enforced
//! > contract; token counts are advisory and tokenizer-relative (RFC 0027), and `max_nodes`
//! > is an `OutputPolicy` member, not a `Budget` dimension. Trimming to a ceiling MUST
//! > produce an `omissions` entry (INV-007) and MUST NOT drop an assurance dimension, a
//! > warning, an epoch, or a redaction stub. Hiding uncertainty to save bytes is prohibited.
//! >
//! > — RFC 0026, "Request envelope"
//!
//! Until this module the field was **declared enforced and enforced by nothing**. `continuumd`
//! accepted `output_policy`, keyed it into the idempotency state
//! ([`ReplayKey`](super::state::ReplayKey)), and never read it again: a caller that stated
//! `max_bytes` received whatever the handler produced, and the sentence above was a promise
//! no code kept. `notes/plan/notes/DX10_BYTE_LEDGER.md` §2.7b found it and bn-6fuu5 is where
//! it is paid.
//!
//! # The two typed outcomes, and why there are exactly two
//!
//! A ceiling is a bound on the answer, so a daemon under one has three things it could do and
//! only two of them are admissible.
//!
//! | | disposition | admissible? |
//! |---|---|---|
//! | 1 | the payload fits — answer it whole | yes; and no omission is invented for a trim that did not happen |
//! | 2 | the payload does not fit — elide a declared prefix and record what was elided | yes; this is the sentence above, and RFC 0028's packer precedent |
//! | 3 | nothing conforming fits — answer the floor anyway | **no**: that is the field being unenforced again |
//!
//! So disposition 3 is a **typed refusal**, which is what INV-008 asks for wherever a bound
//! and an answer disagree: "never a bare boolean, and never a success flag that outruns the
//! evidence". The precedent is exact and is already landed one crate over —
//! `continuum_context::budget::BudgetPacker` answers a byte ceiling with a *smaller pack* and
//! a fuller manifest while a conforming child exists, and refuses when none does
//! (`a_budget_below_the_minimal_child_refuses_rather_than_truncating`, bn-38p2).
//!
//! # Which code the refusal carries
//!
//! `task.status` declares `errors []`, so the codes available to it are `rule errors.common`'s
//! six and no more — [`MalformedRequest`](ErrorCode::MalformedRequest),
//! `ProtocolVersionUnsupported`, `CapabilityDenied`, `QuotaExhausted`, `EpochUnsupported`,
//! `UnsupportedSemanticFeature`. `BudgetExhausted` — the code `context.expand` refuses a
//! too-small ceiling with — is **not** in that union and `task.status` may not return it
//! (`rule errors.common`: "a daemon MUST NOT return a code outside that union").
//!
//! [`ErrorCode::MalformedRequest`] is the one that fits, and the fit is by precedent rather
//! than by elimination: RFC 0026's malformed-input list already maps *envelope obligations* to
//! it — "a `@mutation` request with no `idempotency_key`, and a `@readonly` request carrying
//! one ⇒ `MalformedRequest`; a `@task_starting` request with no `budget` ⇒ `MalformedRequest`"
//! — and each of those requests validates against its struct and is refused for what the
//! envelope *asks the daemon to do*. A `max_bytes` below the smallest conforming answer is the
//! same species: the request is well-formed and states a bound no conforming answer to this
//! operation can meet. `retryable` is false, because the identical request gets the identical
//! refusal and what must change is the caller's ceiling.
//!
//! **Flag raised, not silently closed.** RFC 0026 declares `max_bytes` "the enforced contract"
//! and names no code for a ceiling nothing conforming can meet. This module serves
//! `MalformedRequest` on the reasoning above and records the gap for the RFC's owner; a
//! declared code (or a declared "answer the floor and say so" disposition) would supersede it
//! without changing the shape of anything here.
//!
//! # What a ceiling may take from a `TaskRecord`, and what it may not
//!
//! The elision set is **fixed by the declaration, not chosen for bytes**. `rule
//! versioning.breaking_change` makes "changing a field's type or presence marker" a *major*
//! change, so a `required` member of `TaskRecord` is present on every conforming answer
//! whatever a ceiling says, and a `nullable` one is present-and-possibly-null — where null
//! means *no such thing*, never *withheld* (RFC 0026: "absent and null are distinct and MUST
//! NOT be conflated"). Writing `null` over a snapshot the task really has would be the
//! conflation that rule forbids, and would be a lie besides.
//!
//! What is left is exactly three places where a smaller *value* is still a conforming value:
//!
//! | # | member | why it is elidable | why it is in this position |
//! |---|---|---|---|
//! | 1 | `milestones` | `list<Milestone> required`; the empty list is a legal value | a progress narrative — the furthest of the three from what a poll asks, and the one that grows monotonically and is re-sent whole on every poll |
//! | 2 | `committed_evidence` | `list<EvidenceHandle> required`; likewise | evidence identities, which `evidence.query` and the task handle both reach |
//! | 3 | `budget` | `Budget required`, and all nine of its members are `optional` | last, because it is the frame `cost` is read in: a spend with no ceiling beside it is a number without its bound |
//!
//! `epochs` is **not** in the table and could not be: the sentence at the top of this module
//! forbids a trim from dropping an epoch, and `EpochSet.protocol` is `required` besides.
//! `task`, `status`, `cost` and `continuation` are not in it either — they are what a poll is
//! *for* (`DX10_BYTE_LEDGER.md` §3 C7), and eliding them would answer a different question.
//!
//! # The trim is a prefix, so a larger ceiling never elides more
//!
//! [`fit_task_record`] takes the **shortest prefix** of [`TASK_RECORD_ELISION_ORDER`] whose
//! record fits. That is RFC 0028's frontier order, and it is the property that makes two
//! answers about one task comparable rather than merely different: a ceiling of *n* elides a
//! subset of what a ceiling of *n − 1* elides, so re-reading with more room yields a superset
//! of what the tighter read carried, and the INV-009 monotone-evidence reading holds by
//! construction rather than by convention. Reading with **no** ceiling is the top of that
//! order and yields the whole record — which is why the omission's `recoverable_by` is the
//! task handle: the retrieval route is the same question asked with more room, exactly as
//! bn-38p2 recorded for a shortfall record's `recoverable_by` in the pack family.
//!
//! # An elision that changed nothing is not an omission
//!
//! A prefix may cover a member that was already empty. Nothing was left out there, so nothing
//! is recorded: the manifest would otherwise claim a ceiling withheld evidence that never
//! existed, and [`TaskEntry::omissions`](super::task::TaskEntry::omissions) is *already*
//! saying the true thing about that member — `unsupported`, "this deployment produced none".
//! The two reasons stay apart, which is the distinction `OmissionReason` exists to draw:
//! `unsupported` is a statement about the deployment, `budget` is a statement about this
//! answer.
//!
//! # Cost on a wire that states no ceiling: zero
//!
//! [`Ceiling::of`] reads the envelope, and an absent `output_policy` — or an
//! `output_policy` with no `max_bytes` — returns [`Ceiling::UNBOUNDED`], on which
//! [`fit_task_record`] returns the record it was given **without encoding anything**. A
//! deployment nobody sends a ceiling to pays no measurement, and the landed benchmark matrix
//! is byte-identical to what it was before this module existed.
//!
//! # Measured in the encoding the caller will receive
//!
//! A byte ceiling that counted JSON while the connection negotiated CBOR would be a ceiling on
//! a frame nobody receives, so the measurement uses [`Negotiated::encoding`] and the very
//! codec [`crate::transport::Server::answer`] will encode the payload with. That is the one
//! place the operation layer touches [`crate::codec`], and it is not a softening of that
//! layer's "no codec" rule: this module emits no frame and parses none. It *counts*, because
//! "enforced byte ceiling" is not a statement anything that cannot count bytes is able to
//! keep.
//!
//! # What this module deliberately does not decide
//!
//! - **The other 72 operations.** `task.status` is the operation whose record the ledger
//!   measured and whose declaration leaves something a ceiling can legally take. The one
//!   other reader is `context.compile` (bn-2ga1c), which uses [`Ceiling`], [`measure`] and
//!   [`max_nodes`] and answers an over-ceiling compile with the `BudgetExhausted` its own
//!   `errors` clause declares — see `daemon::context`. Every other operation still ignores
//!   `max_bytes`. The mechanism here is per-response-shape by construction — the elidable
//!   set is a property of the *declaration*, and there is no shape-agnostic answer to "what
//!   may a ceiling take from this struct".
//! - **What a `@mutation` does under a ceiling its answer cannot meet.** Refusing after the
//!   handler ran would trade a rendering bound for a lost mutation — the work committed and
//!   the caller told it failed — which is the shape INV-017 and `PublicationAborted`'s
//!   per-artifact "not published, not truncated" exist to prevent. `task.status` is `@readonly`, so
//!   this bone does not have to answer it, and it does not.
//!
//! [`Negotiated::encoding`]: crate::protocol::handshake::Negotiated::encoding

use crate::codec::cbor::Cbor;
use crate::codec::json::Json;
use crate::codec::{Document, ProtocolValue, write_in};
use crate::protocol::envelope::{Budget, Omission, RequestEnvelope};
use crate::protocol::scalar::{ArtifactHandle, TaskHandle};
use crate::protocol::spec::Optional;
use crate::protocol::task::TaskRecord;
use crate::protocol::vocabulary::{Encoding, ErrorCode, OmissionReason};

use super::family::Fault;

/// The order a ceiling takes `TaskRecord`'s elidable members in.
///
/// Declared here as wire subjects — the strings an [`Omission`] names — because that is what
/// a caller reads them as. See this module's table for why the order is this order.
pub const TASK_RECORD_ELISION_ORDER: [&str; 3] =
    ["task.milestones", "task.committed_evidence", "task.budget"];

/// The detail of the refusal a ceiling below the floor earns.
pub const CEILING_BELOW_FLOOR: &str = "the stated output_policy.max_bytes is below the smallest conforming payload for this \
     operation";

/// The detail of the refusal an unencodable payload earns.
///
/// Unreachable for a `TaskRecord`, whose members are all encodable protocol values; typed
/// rather than unwrapped, because a daemon does not abort on its own invariant.
pub const UNMEASURABLE: &str = "the answer could not be measured against the stated ceiling";

/// The ceiling a request stated, or its absence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ceiling(Option<u64>);

impl Ceiling {
    /// No ceiling: the caller stated none, so nothing bounds the payload.
    pub const UNBOUNDED: Self = Self(None);

    /// A ceiling of `max_bytes`.
    #[must_use]
    pub const fn of_bytes(max_bytes: u64) -> Self {
        Self(Some(max_bytes))
    }

    /// The ceiling this request stated.
    ///
    /// Absent `output_policy` and a present `output_policy` with absent `max_bytes` are the
    /// same answer — "no byte ceiling" — and deliberately so: `OutputPolicy` carries four
    /// members and a caller that states `audience` alone has stated no bound on bytes.
    #[must_use]
    pub fn of(envelope: &RequestEnvelope) -> Self {
        Self(
            envelope
                .output_policy
                .value()
                .and_then(|policy| policy.max_bytes.value().copied())
                .map(crate::protocol::scalar::ByteCount::bytes),
        )
    }

    /// The ceiling this request's `budget.bytes` stated.
    ///
    /// A second reading of the envelope beside [`Ceiling::of`], and a different field:
    /// `Budget.bytes` is the task's byte budget (a `@task_starting` operation carries a
    /// `budget`, RFC 0026), `OutputPolicy.max_bytes` is the ceiling on the result payload
    /// (IDL `struct OutputPolicy`). An absent `budget`, or a `budget` with no `bytes`, states
    /// no ceiling — never a ceiling of zero. The `context` family reads this one against the
    /// pack document it publishes, the way `content_budget.bytes` measures it.
    #[must_use]
    pub fn of_budget(envelope: &RequestEnvelope) -> Self {
        Self(
            envelope
                .budget
                .value()
                .and_then(|budget| budget.bytes.value().copied())
                .map(crate::protocol::scalar::ByteCount::bytes),
        )
    }

    /// The stated ceiling in bytes, when there is one.
    #[must_use]
    pub const fn stated(self) -> Option<u64> {
        self.0
    }

    /// Whether a payload of `bytes` bytes is inside this ceiling.
    ///
    /// A ceiling of *n* admits *n*: `max_bytes` is a maximum, so the bound is `<=` and the
    /// answer exactly at it is served whole.
    #[must_use]
    pub const fn admits(self, bytes: u64) -> bool {
        match self.0 {
            None => true,
            Some(ceiling) => bytes <= ceiling,
        }
    }
}

/// The graph-node ceiling this request stated — IDL `OutputPolicy.max_nodes`, "Ceiling on
/// returned graph nodes" — or [`None`].
///
/// A count of nodes, not of bytes, so it is not a [`Ceiling`]: nothing here measures it. The
/// operation that returns graph nodes decides what one is and enforces it; `context.compile`
/// counts `selected[]` items and records the ceiling at `content_budget.nodes` (RFC 0028,
/// "Budgets and packing").
#[must_use]
pub fn max_nodes(envelope: &RequestEnvelope) -> Option<u64> {
    envelope
        .output_policy
        .value()
        .and_then(|policy| policy.max_nodes.value().copied())
}

/// A record fitted to a ceiling, and the manifest of what the fitting took.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fitted {
    /// The record to answer with.
    pub record: TaskRecord,
    /// One INV-007 record per member the ceiling actually took, in elision order.
    pub omissions: Vec<Omission>,
    /// The measured size of [`Fitted::record`] in the negotiated encoding, when a ceiling
    /// was stated.
    ///
    /// [`None`] on an unbounded request, where nothing was measured because nothing had to
    /// be: see this module's "Cost on a wire that states no ceiling".
    pub bytes: Option<u64>,
}

/// Fit `record` under `ceiling`, measured in `encoding`.
///
/// Returns the whole record and an empty manifest when the ceiling admits it (and when there
/// is no ceiling, without measuring). Otherwise elides the shortest prefix of
/// [`TASK_RECORD_ELISION_ORDER`] that fits, recording an [`Omission`] — `reason = budget`,
/// `recoverable_by` = the task's own handle — for each elided member that carried something.
///
/// # Errors
///
/// [`Fault`] carrying [`ErrorCode::MalformedRequest`] when no conforming record fits the
/// ceiling, and when the record cannot be measured at all. See this module's documentation
/// for why that code and not another.
pub fn fit_task_record(
    record: TaskRecord,
    ceiling: Ceiling,
    encoding: Encoding,
) -> Result<Fitted, Fault> {
    if ceiling.stated().is_none() {
        return Ok(Fitted {
            record,
            omissions: Vec::new(),
            bytes: None,
        });
    }

    let mut bytes = measure(&record, encoding)?;
    if ceiling.admits(bytes) {
        return Ok(Fitted {
            record,
            omissions: Vec::new(),
            bytes: Some(bytes),
        });
    }

    // The retrieval route, resolved once and before anything is elided: an omission that
    // could not name it would be half an INV-007 record, and finding that out after the trim
    // would mean answering with a manifest the rule does not accept.
    let recoverable_by = expansion_handle(&record.task)?;
    let mut fitted = record;
    let mut omissions = Vec::new();
    for subject in TASK_RECORD_ELISION_ORDER {
        if elide(&mut fitted, subject) {
            omissions.push(Omission {
                reason: OmissionReason::Budget,
                subject: (*subject).to_owned(),
                recoverable_by: Optional::Present(recoverable_by.clone()),
            });
        }
        bytes = measure(&fitted, encoding)?;
        if ceiling.admits(bytes) {
            return Ok(Fitted {
                record: fitted,
                omissions,
                bytes: Some(bytes),
            });
        }
    }
    Err(Fault::new(ErrorCode::MalformedRequest, CEILING_BELOW_FLOOR))
}

/// Take `subject` out of `record`, reporting whether it carried anything.
///
/// The report is what keeps the manifest honest: eliding an empty list changes no byte and
/// withholds nothing, so it earns no record. See "An elision that changed nothing is not an
/// omission".
fn elide(record: &mut TaskRecord, subject: &str) -> bool {
    match subject {
        "task.milestones" => {
            let carried = !record.milestones.is_empty();
            record.milestones = Vec::new();
            carried
        }
        "task.committed_evidence" => {
            let carried = !record.committed_evidence.is_empty();
            record.committed_evidence = Vec::new();
            carried
        }
        "task.budget" => {
            let carried = record.budget != empty_budget();
            record.budget = empty_budget();
            carried
        }
        // Unreachable: the loop iterates `TASK_RECORD_ELISION_ORDER` and this function is
        // private to it. An unknown subject elides nothing rather than panicking, so a future
        // order that gains a name without gaining an arm is a ceiling that refuses rather
        // than a daemon that aborts.
        _ => false,
    }
}

/// A `Budget` stating no dimension.
///
/// All nine members are `optional`, so this is a conforming `Budget` — the empty statement,
/// not a statement of zeros. "A dimension the engine does not measure is absent, never zero"
/// is `Cost`'s rule and the reading is the same here: absent is the absence of a ceiling, and
/// a nine-zero budget would be a claim the caller may spend nothing.
fn empty_budget() -> Budget {
    Budget {
        wall_ms: Optional::Absent,
        cpu_ms: Optional::Absent,
        memory_bytes: Optional::Absent,
        states: Optional::Absent,
        solver_ms: Optional::Absent,
        proof_ms: Optional::Absent,
        tokens: Optional::Absent,
        candidates: Optional::Absent,
        bytes: Optional::Absent,
    }
}

/// The handle whose re-reading recovers what a ceiling took.
///
/// The task's own handle, class-agnostic: "the same question asked with more room". The
/// conversion is fallible in the type and unreachable in fact — a `task_*` handle satisfies
/// `ArtifactHandle`'s pattern by construction — and is typed rather than unwrapped for the
/// reason [`super::budget::Publications::artifact`] gives for the identical conversion.
fn expansion_handle(task: &TaskHandle) -> Result<ArtifactHandle, Fault> {
    ArtifactHandle::new(task.as_str())
        .map_err(|_| Fault::new(ErrorCode::MalformedRequest, UNMEASURABLE))
}

/// The size of `value` in the encoding this connection negotiated.
///
/// # Errors
///
/// [`Fault`] carrying [`ErrorCode::MalformedRequest`] when the value cannot be encoded.
pub fn measure<T: ProtocolValue>(value: &T, encoding: Encoding) -> Result<u64, Fault> {
    match encoding {
        Encoding::CanonicalJson => measure_in::<Json, T>(value),
        Encoding::CanonicalCbor => measure_in::<Cbor, T>(value),
    }
}

fn measure_in<D: Document, T: ProtocolValue>(value: &T) -> Result<u64, Fault> {
    write_in::<D, T>(value)
        .map(|bytes| bytes.len() as u64)
        .map_err(|_| Fault::new(ErrorCode::MalformedRequest, UNMEASURABLE))
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::protocol::envelope::{Cost, EpochSet, OutputPolicy};
    use crate::protocol::scalar::{
        ByteCount, EpochIdentity, EvidenceHandle, IntentHandle, OperationName, ProtocolVersion,
        RequestId, Timestamp, WorkspaceHandle,
    };
    use crate::protocol::spec::Nullable;
    use crate::protocol::task::Milestone;
    use crate::protocol::vocabulary::{PriorityClass, TaskStatus};

    fn handle(text: &str) -> TaskHandle {
        TaskHandle::new(text).expect("a well-formed task handle")
    }

    fn record(milestones: usize, evidence: usize) -> TaskRecord {
        TaskRecord {
            task: handle("task_00000000000000000000000000000001"),
            operation: OperationName::new("verification.start").expect("a name"),
            status: TaskStatus::Completed,
            snapshot: Nullable::Value(
                WorkspaceHandle::new("ws_00000000000000000000000000000002").expect("a handle"),
            ),
            intent: Nullable::Value(
                IntentHandle::new("in_00000000000000000000000000000003").expect("a handle"),
            ),
            failed_reason: Optional::Absent,
            continuation: Optional::Absent,
            non_resumable_reason: Optional::Absent,
            budget: Budget {
                states: Optional::Present(64),
                ..empty_budget()
            },
            cost: Cost {
                states: Optional::Present(16),
                ..crate::daemon::result::unmeasured()
            },
            epochs: EpochSet {
                protocol: ProtocolVersion::new(3, 4),
                semantic: Nullable::Value(EpochIdentity::new("semantic-1").expect("an epoch")),
                intent: Nullable::Value(EpochIdentity::new("intent-1").expect("an epoch")),
                evidence: Nullable::Null,
                proof: Nullable::Value(EpochIdentity::new("proof-1").expect("an epoch")),
                corpus: Nullable::Value(EpochIdentity::new("corpus-1").expect("an epoch")),
                engine: Nullable::Value(EpochIdentity::new("engine-1").expect("an epoch")),
            },
            priority_class: PriorityClass::Interactive,
            milestones: (0..milestones)
                .map(|index| Milestone {
                    name: format!("milestone.{index}"),
                    at: Timestamp::new("2026-08-01T00:00:00.000Z").expect("a timestamp"),
                })
                .collect(),
            committed_evidence: (0..evidence)
                .map(|index| {
                    EvidenceHandle::new(&format!("ev_0000000000000000000000000000000{index}"))
                        .expect("a handle")
                })
                .collect(),
        }
    }

    fn size(record: &TaskRecord) -> u64 {
        measure(record, Encoding::CanonicalJson).expect("a measurable record")
    }

    fn fit(record: &TaskRecord, ceiling: u64) -> Result<Fitted, Fault> {
        fit_task_record(
            record.clone(),
            Ceiling::of_bytes(ceiling),
            Encoding::CanonicalJson,
        )
    }

    fn envelope(policy: Optional<OutputPolicy>) -> RequestEnvelope {
        RequestEnvelope {
            protocol_version: ProtocolVersion::new(3, 4),
            request_id: RequestId::new("req_1").expect("a request id"),
            idempotency_key: Optional::Absent,
            actor: crate::protocol::scalar::ActorId::new("agent:reader").expect("an actor"),
            capability: crate::protocol::scalar::CapabilityHandle::new("cap_reader")
                .expect("a capability"),
            operation: OperationName::new("task.status").expect("a name"),
            snapshot: Nullable::Null,
            intent: Nullable::Null,
            arguments: crate::protocol::scalar::Opaque::from_bytes(Vec::new()),
            budget: Optional::Absent,
            output_policy: policy,
            trace: Optional::Absent,
            page: Optional::Absent,
        }
    }

    fn policy(max_bytes: Option<u64>) -> OutputPolicy {
        OutputPolicy {
            max_bytes: match max_bytes {
                Some(bytes) => Optional::Present(ByteCount::new(bytes)),
                None => Optional::Absent,
            },
            max_tokens: Optional::Absent,
            max_nodes: Optional::Absent,
            audience: Optional::Absent,
        }
    }

    /// An absent policy, and a policy that bounds something other than bytes, both state no
    /// byte ceiling. They are not the same field and they are the same answer.
    #[test]
    fn a_request_that_states_no_byte_ceiling_is_unbounded() {
        assert_eq!(Ceiling::of(&envelope(Optional::Absent)), Ceiling::UNBOUNDED);
        assert_eq!(
            Ceiling::of(&envelope(Optional::Present(policy(None)))),
            Ceiling::UNBOUNDED
        );
        assert_eq!(
            Ceiling::of(&envelope(Optional::Present(policy(Some(512))))).stated(),
            Some(512)
        );
    }

    /// The landed wire's cost: an unbounded request is answered with the record it was given,
    /// and nothing is measured to do it.
    #[test]
    fn an_unbounded_request_is_answered_whole_and_unmeasured() {
        let record = record(3, 2);
        let fitted = fit_task_record(record.clone(), Ceiling::UNBOUNDED, Encoding::CanonicalJson)
            .expect("an unbounded fit cannot fail");
        assert_eq!(
            fitted.record, record,
            "the record is what the handler built"
        );
        assert!(fitted.omissions.is_empty(), "nothing was left out");
        assert_eq!(fitted.bytes, None, "and nothing was encoded to find out");
    }

    /// The boundary, from both sides. `max_bytes` is a maximum, so the answer whose size is
    /// exactly the ceiling is served whole, and one byte less of room takes the first member
    /// of the order and no more.
    #[test]
    fn a_ceiling_at_the_records_own_size_serves_it_whole_and_one_below_elides_the_first_member() {
        let record = record(3, 2);
        let whole = size(&record);

        let at = fit(&record, whole).expect("the record fits its own size");
        assert_eq!(at.record, record);
        assert!(at.omissions.is_empty());
        assert_eq!(at.bytes, Some(whole));

        let below = fit(&record, whole - 1).expect("a smaller conforming record exists");
        assert!(below.record.milestones.is_empty(), "the first member goes");
        assert_eq!(
            below.record.committed_evidence, record.committed_evidence,
            "and the second does not, because the first was enough"
        );
        assert_eq!(below.record.budget, record.budget, "nor the third");
        assert_eq!(below.omissions.len(), 1);
        assert_eq!(below.omissions[0].subject, "task.milestones");
        assert_eq!(below.omissions[0].reason, OmissionReason::Budget);
        assert_eq!(
            below.omissions[0]
                .recoverable_by
                .value()
                .map(|h| h.as_str()),
            Some(record.task.as_str()),
            "INV-007's retrieval half names the task: the same question with more room"
        );
        assert!(below.bytes.expect("measured") < whole);
    }

    /// The floor is the whole order elided, and it is served rather than refused.
    #[test]
    fn a_ceiling_at_the_floor_is_answered_with_every_elidable_member_gone() {
        let record = record(3, 2);
        let mut floor_record = record.clone();
        for subject in TASK_RECORD_ELISION_ORDER {
            elide(&mut floor_record, subject);
        }
        let floor = size(&floor_record);

        let fitted = fit(&record, floor).expect("the floor is an answer");
        assert_eq!(fitted.record, floor_record);
        assert_eq!(fitted.bytes, Some(floor));
        assert_eq!(
            fitted
                .omissions
                .iter()
                .map(|omission| omission.subject.as_str())
                .collect::<Vec<_>>(),
            TASK_RECORD_ELISION_ORDER.to_vec(),
            "one record per member taken, in the declared order"
        );
        // The three the trim may never take are all still there.
        assert_eq!(fitted.record.epochs, record.epochs, "no epoch was dropped");
        assert_eq!(fitted.record.task, record.task);
        assert_eq!(fitted.record.status, record.status);
        assert_eq!(fitted.record.cost, record.cost);
    }

    /// One byte below the floor there is no conforming answer, and the daemon says so with a
    /// typed refusal rather than a payload over the ceiling it was given.
    #[test]
    fn a_ceiling_below_the_floor_refuses_rather_than_overrunning() {
        let record = record(3, 2);
        let mut floor_record = record.clone();
        for subject in TASK_RECORD_ELISION_ORDER {
            elide(&mut floor_record, subject);
        }
        let floor = size(&floor_record);

        let fault = fit(&record, floor - 1).expect_err("nothing conforming fits");
        assert_eq!(fault.code, ErrorCode::MalformedRequest);
        assert_eq!(fault.detail, CEILING_BELOW_FLOOR);
        assert!(!fault.retryable, "the same request gets the same answer");

        // And the same at the extreme, so the refusal is not a boundary artefact.
        let zero = fit(&record, 0).expect_err("a ceiling of zero admits nothing");
        assert_eq!(zero.code, ErrorCode::MalformedRequest);
    }

    /// The prefix property, checked over every ceiling from the floor to the whole record:
    /// more room never elides more. This is what makes the INV-009 superset reading hold —
    /// re-reading with a larger ceiling recovers everything a tighter read carried.
    #[test]
    fn a_larger_ceiling_elides_a_subset_of_what_a_smaller_one_elides() {
        let record = record(3, 2);
        let whole = size(&record);
        let mut floor_record = record.clone();
        for subject in TASK_RECORD_ELISION_ORDER {
            elide(&mut floor_record, subject);
        }
        let floor = size(&floor_record);

        let mut previous: Option<Fitted> = None;
        for ceiling in floor..=whole {
            let fitted = fit(&record, ceiling).expect("every ceiling from the floor up answers");
            assert!(
                fitted.bytes.expect("measured") <= ceiling,
                "the answer is inside the ceiling at {ceiling}"
            );
            if let Some(previous) = &previous {
                assert!(
                    fitted.omissions.len() <= previous.omissions.len(),
                    "more room, never more elided (at {ceiling})"
                );
                assert!(
                    fitted.record.milestones.len() >= previous.record.milestones.len()
                        && fitted.record.committed_evidence.len()
                            >= previous.record.committed_evidence.len(),
                    "and the answer is a superset of the tighter one (at {ceiling})"
                );
            }
            previous = Some(fitted);
        }
        let top = previous.expect("at least one ceiling was tried");
        assert_eq!(
            top.record, record,
            "the top of the order is the whole record"
        );
    }

    /// An elision that took nothing records nothing: a manifest that claimed a ceiling
    /// withheld milestones a task never reached would be false, and
    /// `TaskEntry::omissions`'s `unsupported` record is already the true statement about it.
    #[test]
    fn eliding_an_already_empty_member_records_no_omission() {
        let record = record(0, 0);
        let mut floor_record = record.clone();
        for subject in TASK_RECORD_ELISION_ORDER {
            elide(&mut floor_record, subject);
        }
        let fitted = fit(&record, size(&floor_record)).expect("the floor is an answer");
        assert_eq!(
            fitted
                .omissions
                .iter()
                .map(|omission| omission.subject.as_str())
                .collect::<Vec<_>>(),
            vec!["task.budget"],
            "only the member that actually carried something is named"
        );
    }

    /// The ceiling is measured in the encoding the caller will receive, not in whichever one
    /// the daemon finds convenient. CBOR is the smaller of the two, so a ceiling that forces
    /// a trim under JSON can leave the same record whole under CBOR — and that is the point:
    /// a bound on bytes nobody receives is not a bound.
    #[test]
    fn the_ceiling_is_measured_in_the_negotiated_encoding() {
        let record = record(3, 2);
        let json = size(&record);
        let cbor = measure(&record, Encoding::CanonicalCbor).expect("a measurable record");
        assert!(
            cbor < json,
            "CBOR is the smaller encoding here: {cbor} < {json}"
        );

        let under_cbor = fit_task_record(
            record.clone(),
            Ceiling::of_bytes(json - 1),
            Encoding::CanonicalCbor,
        )
        .expect("a fit");
        assert_eq!(
            under_cbor.record, record,
            "the same ceiling that trims under JSON leaves this record whole under CBOR"
        );
        assert!(under_cbor.omissions.is_empty());
    }

    /// A ceiling admits its own value: `max_bytes` is a maximum.
    #[test]
    fn the_bound_is_inclusive() {
        let ceiling = Ceiling::of_bytes(100);
        assert!(ceiling.admits(99));
        assert!(ceiling.admits(100));
        assert!(!ceiling.admits(101));
        assert!(Ceiling::UNBOUNDED.admits(u64::MAX));
    }
}
