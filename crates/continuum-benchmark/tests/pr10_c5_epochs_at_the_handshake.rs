//! **C5 re-measured, and gated.** What suppressing the epoch set from every answer is worth,
//! and the protocol major it needs before anyone may collect it.
//!
//! > | 2 | **C5** `epochs` pinned at the handshake | `bn-2in7i` | 1,344 | declaration-moving,
//! > bundled **3.6** |
//! >
//! > — `notes/plan/notes/DX10_BYTE_LEDGER.md` §6
//!
//! # The number is right. The version is not.
//!
//! Two independent claims live in that row, and this file separates them because the campaign
//! has now been wrong about the second one twice.
//!
//! **The size is confirmed.** [`the_prize_is_real_and_slightly_larger_than_the_ledger_said`]
//! re-measures the recorded answers with `continuumd`'s own codec and finds exactly the
//! 27,864 B §2.6 projected, one
//! distinct epoch encoding across all 172 answers, and a key skeleton of 5,332 B rather than
//! 5,160 B. C5 is the largest item in the ledger and the measurement backs it: **1,383 B per
//! solved task**, more than C1, C2, C3, C4 and C6 combined.
//!
//! **The wire-visibility class is falsified.** `ResultEnvelope` declares all three members
//! `required`:
//!
//! ```text
//!   cost: Cost required;
//!   epochs: EpochSet required;
//!   next_operations: list<NextOperation> required;
//! ```
//!
//! and `rule versioning.breaking_change` opens with
//!
//! > Removing or renaming any declaration or field; **changing a field's type or presence
//! > marker**; … each is a breaking change and MUST advance the protocol major. A breaking
//! > change MUST publish the typed per-artifact-class compatibility statement
//! > `Preserved | Revalidate | Incompatible` required by plan §4.6 before it is applied.
//!
//! C5's mechanism is *stated in those words* — "three `required` markers become `optional`" —
//! so it is a **major**, not the 3.6 minor, and it cannot be made dormant-at-3.5
//! either: the suppression a 3.6 client would receive is a frame the declaration does not
//! admit at any minor of major 3. [`a_conforming_reader_rejects_an_answer_with_these_members_absent`]
//! is that verdict taken mechanically rather than read off the rule — the decoder this repo
//! generates from that declaration refuses the frame.
//!
//! This is the same shape as `bn-6fuu5`'s C7 finding one item earlier, and the second time the
//! ledger's wire-visibility column has been wrong in the same direction. The difference is
//! worth stating: C7's *bytes* were falsified and its mechanism partly landed; C5's bytes are
//! confirmed and **none** of its mechanism may land.
//!
//! # What is available at 3.5: nothing
//!
//! [`nothing_of_the_prize_is_available_without_moving_a_presence_marker`] measures the
//! remainder rather than asserting it, and the remainder is **0 B**. In particular the 8,084 B
//! the landed instrument credits this line is measured here, confirmed as the reduction to
//! `protocol` plus six explicit nulls — and found unavailable too, for a reason the ledger did
//! not name: those six nulls would be the daemon denying it can pin five epochs it can pin,
//! and `rule envelope.epochs_named` spends its second sentence on exactly that.
//!
//! # For whoever takes the major
//!
//! Two facts this file establishes that the mechanism will need, neither of them about bytes:
//! [`the_epoch_set_every_answer_carries_is_the_set_the_welcome_pinned`] (the premise holds on
//! this deployment — 172 of 172) and
//! [`an_epoch_advance_reaches_a_live_connection_through_this_member_and_no_other`] (there is no
//! mid-connection announcement channel, so suppression must never elide a *changed* set).
//! `bn-2in7i`'s bone comment carries the activation checklist.

mod support;

use continuum_benchmark::report::Report;
use continuum_benchmark::surface::Arm;
use continuum_benchmark::variants::{self, EpochProjection};
use continuumd::codec::{CodecError, from_bytes, to_bytes};
use continuumd::protocol::envelope::{EpochSet, ResultEnvelope};
use continuumd::protocol::handshake::ServerWelcome;
use continuumd::protocol::registry;
use continuumd::protocol::spec::{Presence, ProtocolStruct};

/// The three members C5 would suppress.
const SUPPRESSED: [&str; 3] = ["cost", "epochs", "next_operations"];

/// The projection, measured once for the file.
fn projection() -> EpochProjection {
    variants::epoch_projection().expect("the recording sweep runs")
}

/// **The size, confirmed.** §2.6's 27,864 B is exact, and the bundled key skeleton is 172 B
/// larger than §3 C5 said.
#[test]
fn the_prize_is_real_and_slightly_larger_than_the_ledger_said() {
    let projection = projection();

    // The landed anchors, reproduced first, as the ledger's method requires.
    assert_eq!(projection.answers, 172, "the matrix's answers");
    assert_eq!(projection.frame_bytes, 171_220, "their result frames");

    assert_eq!(
        projection.distinct_epoch_sets, 1,
        "one distinct epoch encoding across every answer — §2.6's finding"
    );
    assert_eq!(
        projection.epochs_absent_bytes, 27_864,
        "and suppressing it is worth exactly what §2.6 projected"
    );

    // The bundle. Both members are provably empty on every answer, so the whole of what they
    // cost is key skeleton.
    assert_eq!(projection.empty_cost, 172);
    assert_eq!(projection.empty_next_operations, 172);
    assert_eq!(
        projection.cost_absent_bytes, 1_720,
        "172 answers x 10 B of `cost` key, colon, empty object and separator"
    );
    assert_eq!(
        projection.next_operations_absent_bytes, 3_612,
        "172 answers x 21 B of `next_operations` key, colon, empty list and separator"
    );
    assert_eq!(
        projection.cost_absent_bytes + projection.next_operations_absent_bytes,
        5_332,
        "against §3 C5's 5,160 — the skeleton is 31 B per answer, not 30"
    );

    assert_eq!(
        projection.per_solved(),
        1_383,
        "the largest single item in the ledger, and the measurement backs it"
    );
}

/// **The gate, taken mechanically.** A conforming 3.5 reader does not accept an answer with any
/// of these three members absent — so the suppression is not something a 3.6 client could be
/// served while a 3.5 client is served today's bytes. It is a frame major 3 does not have.
///
/// The reader is not a model of a client: it is the decoder this repo generates from the
/// declaration in `notes/plan/schemas/continuumd-native-protocol.idl`, which is the same
/// decoder every conforming implementation is required to be equivalent to.
#[test]
fn a_conforming_reader_rejects_an_answer_with_these_members_absent() {
    let landed = an_answer();
    let text = String::from_utf8(to_bytes(&landed).expect("the answer encodes"))
        .expect("canonical JSON is text");

    // The frame as landed decodes, so the surgery below is the only difference.
    let read: ResultEnvelope = from_bytes(text.as_bytes()).expect("the landed frame decodes");
    assert_eq!(read, landed);

    for member in SUPPRESSED {
        let suppressed =
            variants::without_member(&text, member).expect("the member is on the wire to remove");
        assert!(
            suppressed.len() < text.len(),
            "removing `{member}` removes bytes"
        );
        match from_bytes::<ResultEnvelope>(suppressed.as_bytes()) {
            Err(CodecError::MissingField { declared_by, field }) => {
                assert_eq!(declared_by, "ResultEnvelope");
                assert_eq!(field, member, "and it is refused by name");
            }
            other => panic!(
                "a 3.5 reader must refuse an answer without `{member}`, got {:?}",
                other.map(|_| "an accepted envelope")
            ),
        }
    }
}

/// **The declaration, structurally.** All three are `required`, and `EpochSet` has no member a
/// daemon may omit either — so there is no smaller *shape* of this member at major 3, only a
/// smaller value.
///
/// `rule versioning.compatible_change` names the five changes a minor may carry: adding an
/// operation, adding an `optional` field, adding a member to an `@open` enum, relaxing a
/// server-side constraint, adding an error code. Turning a `required` field into an `optional`
/// one is none of them, and is the first thing `rule versioning.breaking_change` names.
#[test]
fn the_three_members_are_required_and_no_minor_may_move_that() {
    for member in SUPPRESSED {
        let field = ResultEnvelope::FIELDS
            .iter()
            .find(|field| field.name == member)
            .unwrap_or_else(|| panic!("`{member}` is declared"));
        assert_eq!(
            field.presence,
            Presence::Required,
            "`ResultEnvelope.{member}: {} required` — suppressing it changes a presence marker",
            field.ty
        );
    }

    // Nor is there room inside the set: `protocol` is required and the other six are nullable,
    // and a nullable field omitted is malformed (`rule envelope.epochs_named`: "an epoch the
    // result cannot pin reads null; it is never an absent field").
    assert_eq!(EpochSet::FIELDS.len(), 7);
    for field in EpochSet::FIELDS {
        assert_ne!(
            field.presence,
            Presence::Optional,
            "`EpochSet.{}` may not be absent either",
            field.name
        );
    }

    // The counterpart the mechanism reads from is `required` too, which is the one piece of
    // good news for the major: the welcome is obliged to carry the set it would pin.
    let welcome = ServerWelcome::FIELDS
        .iter()
        .find(|field| field.name == "epochs")
        .expect("the welcome names the epochs it serves");
    assert_eq!(welcome.presence, Presence::Required);
}

/// **The remainder, measured.** Once the presence markers are excluded, C5 is worth nothing at
/// 3.5 — including the 8,084 B the landed instrument credits it, which is unavailable for a
/// second reason the ledger did not name.
#[test]
fn nothing_of_the_prize_is_available_without_moving_a_presence_marker() {
    let projection = projection();

    assert_eq!(
        projection.available_without_a_bump, 0,
        "no byte of C5 is collectable at 3.5"
    );

    // The credited figure, priced. §2.6 reads it as "the reduction to protocol-only, which
    // still ships six explicit nulls" and calls it a 3.4x understatement of 27,864. Both
    // readings are right about the arithmetic and neither is a saving anyone may take.
    assert_eq!(
        projection.protocol_only_bytes, 8_084,
        "the landed instrument's credit, reproduced exactly"
    );
    assert_eq!(
        projection.epochs_absent_bytes / projection.protocol_only_bytes,
        3,
        "hence §2.6's 3.4x"
    );

    // Why it is not available: five of the six nulls would be false. The set the daemon serves
    // pins `semantic`, `intent`, `proof`, `corpus` and `engine`; only `evidence` is genuinely
    // unpinned. `rule envelope.epochs_named` gives null one meaning — "the result cannot pin
    // this" — so writing it over a pinned epoch is a claim about the daemon, not an encoding.
    let served = continuum_benchmark::rig::epochs();
    let pinned = [
        served.semantic.value().is_some(),
        served.intent.value().is_some(),
        served.proof.value().is_some(),
        served.corpus.value().is_some(),
        served.engine.value().is_some(),
    ];
    assert_eq!(
        pinned.iter().filter(|is| **is).count(),
        5,
        "five epochs this daemon can pin and would have to deny"
    );
    assert!(
        served.evidence.is_null(),
        "and one it truthfully cannot, which is already null and already saving its bytes"
    );
}

/// **The premise, for the major.** Absence can mean "the set the welcome pinned" only if the
/// welcome pinned the set the answers carry. On this deployment it does, on every answer.
///
/// Recorded as a property of *this deployment*, not of the protocol: a daemon holding two
/// epochs during a migration (`EpochAdvanceNotice`, "at most two epochs concurrently") would
/// answer some calls under the older set, and that is the case the mechanism must state
/// explicitly rather than elide.
#[test]
fn the_epoch_set_every_answer_carries_is_the_set_the_welcome_pinned() {
    let projection = projection();
    assert_eq!(
        projection.answers_unpinned_by_the_welcome, 0,
        "172 of 172 answers carry the welcome's own set"
    );
    assert_eq!(projection.distinct_epoch_sets, 1);
}

/// **The correctness constraint, for the major.** There is no event frame announcing an epoch
/// advance, so on a live connection `ResultEnvelope.epochs` is the *only* member through which
/// a client learns the set changed. A suppression keyed on the negotiated version must
/// therefore elide repetition and never a change — and this test fixes what "the already
/// declared path" is, because there is exactly one.
///
/// `ServerWelcome.pending_advances` announces advances *before* they apply and is a handshake
/// frame: a client that connected before an announcement never sees it on that connection.
#[test]
fn an_epoch_advance_reaches_a_live_connection_through_this_member_and_no_other() {
    // `rule subscription.delivery`: "an event frame IS the declared event struct", and the
    // registry declares exactly two.
    let events: Vec<&str> = registry::OPERATIONS
        .iter()
        .filter_map(|operation| operation.events)
        .collect();
    assert_eq!(
        events,
        ["TaskEvent", "EvidenceEvent"],
        "the only frames a daemon may send unprompted"
    );
    assert!(
        !events.contains(&"EpochAdvanceNotice"),
        "and an epoch advance is not among them"
    );

    // The notice's one home is the handshake.
    let advances = ServerWelcome::FIELDS
        .iter()
        .find(|field| field.ty == "list<EpochAdvanceNotice>")
        .expect("the welcome carries the pending advances");
    assert_eq!(advances.name, "pending_advances");
    assert!(
        !ResultEnvelope::FIELDS
            .iter()
            .any(|field| field.ty.contains("EpochAdvanceNotice")),
        "a result never carries one"
    );
}

/// **The instrument that prices the inadmissible.** The counterfactual frames above exist only
/// as text, because the Rust types cannot hold them, so the text surgery is load-bearing and is
/// tested on its own before any number is read off it.
///
/// The cases that matter are the ones a regex would get wrong: a member whose *value* contains
/// the name of another member, and a string containing a closing brace.
#[test]
fn the_surgery_removes_one_member_and_exactly_one_separator() {
    let object = r#"{"a":1,"cost":{},"epochs":{"x":"cost"},"next_operations":[],"z":"}"}"#;

    assert_eq!(
        variants::without_member(object, "cost").as_deref(),
        Some(r#"{"a":1,"epochs":{"x":"cost"},"next_operations":[],"z":"}"}"#),
        "the member goes, the string that spells its name inside another value stays"
    );
    assert_eq!(
        variants::without_member(object, "epochs").as_deref(),
        Some(r#"{"a":1,"cost":{},"next_operations":[],"z":"}"}"#)
    );
    assert_eq!(
        variants::without_member(object, "next_operations").as_deref(),
        Some(r#"{"a":1,"cost":{},"epochs":{"x":"cost"},"z":"}"}"#)
    );
    assert_eq!(
        variants::without_member(object, "a").as_deref(),
        Some(r#"{"cost":{},"epochs":{"x":"cost"},"next_operations":[],"z":"}"}"#),
        "the first member takes the comma that follows it"
    );
    assert_eq!(
        variants::without_member(object, "z").as_deref(),
        Some(r#"{"a":1,"cost":{},"epochs":{"x":"cost"},"next_operations":[]}"#),
        "the last member takes the comma before it — a brace inside a string is not a brace"
    );
    assert_eq!(
        variants::without_member(object, "nope"),
        None,
        "and a member that is not there is not a saving"
    );

    // Every result is still a document the codec accepts, which is what makes the byte
    // difference a wire measurement rather than a string measurement.
    for member in SUPPRESSED {
        let landed = an_answer();
        let text = String::from_utf8(to_bytes(&landed).expect("encodes")).expect("text");
        let cut = variants::without_member(&text, member).expect("the member is there");
        assert_eq!(
            cut.matches(&format!("\"{member}\":")).count(),
            0,
            "`{member}` is gone from the frame"
        );
        assert!(cut.starts_with('{') && cut.ends_with('}'));
    }
}

/// **The control.** Nothing in this bone touched the wire, and the landed matrix says so.
#[test]
fn the_landed_matrix_is_unmoved_by_this_bone() {
    let report = Report::new(support::both_arms());
    let native = report.totals[&Arm::Native];
    let shell = report.totals[&Arm::Shell];

    // 10,384 as of bn-3of5h, not 10,859: `workspace.create_by_reference` landed at
    // protocol 3.6 and the typed arm takes it. Nothing in *this* bone moved it — the
    // control's claim is unchanged and is still checked against the matrix, at the value
    // the matrix now has.
    assert_eq!(native.bytes_per_solved(), Some(10_384));
    assert_eq!(shell.bytes_per_solved(), Some(4_118));
    assert_eq!((native.solved, shell.solved), (24, 24));
    assert_eq!(native.invalid_permille(), 69);
    assert_eq!(shell.invalid_permille(), 69);
}

// --- fixtures -------------------------------------------------------------------------------

/// One answer the daemon actually produced, taken from the matrix rather than built by hand.
fn an_answer() -> ResultEnvelope {
    variants::shell_sweep(
        continuum_benchmark::shell::Renderer::Standard,
        continuum_benchmark::shell::Disciplines::ALL,
        true,
    )
    .expect("the recording sweep runs")
    .into_iter()
    .find_map(|cell| cell.recorded.first().cloned())
    .expect("the matrix produced an answer")
    .envelope
}
