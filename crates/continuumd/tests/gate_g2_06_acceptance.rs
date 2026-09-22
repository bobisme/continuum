//! G2-06 acceptance — the disjunction's discharge, audited from the pinned artifacts and
//! from the live wire.
//!
//! # The criterion, and why this file is not a re-run of the ablation
//!
//! > native ACI beats the disciplined shell baseline on success and cost, **or** the
//! > protocol is redesigned before freeze (G0-DX-10)
//! >
//! > — `notes/plan/docs/52_RELEASE_GATES_REV3.md`, G2
//!
//! The sentence is a **disjunction**, and it has already been adjudicated once: the first
//! disjunct failed as measured, and the criterion is carried by the second — the redesign
//! that `bn-762i` ratified on 2026-08-11 and that landed as protocol 3.6's
//! `workspace.create_by_reference` and as `OutputPolicy.max_bytes` enforcement. Re-running
//! the delivering sweep would only reproduce a number that is already pinned. What an
//! acceptance layer owes instead is the question the record cannot answer about itself:
//! **is the discharge sound?** Three legs, and each one can fail on its own.
//!
//! | Leg | Question | Where it is answered |
//! |---|---|---|
//! | 1 | Does the failure arithmetic re-derive from the raw pinned data? | the `leg1_*` tests — every headline figure recomputed from `DX10_BYTE_LEDGER.md`'s own tables |
//! | 2 | Did the redesign actually deliver what it promised, on the wire? | the `leg2_*` tests — `workspace.create_by_reference` and `max_bytes` driven live, bytes measured by this file |
//! | 3 | Would either leg notice if it were lied to? | the `leg3_*` tests — a doctored ledger and a by-reference frame that secretly ships its components |
//!
//! # What "independently" means here, mechanically
//!
//! This file is in `continuumd`, which `continuum-benchmark` depends on and not the other
//! way round. It therefore **cannot** call `continuum_benchmark::report`, `variants`, or any
//! other function that produced the figures under audit — the arithmetic below is written
//! fresh, in this file, over numbers parsed out of the ledger's Markdown at run time. A
//! figure that agrees is agreeing across two independent computations; a figure that
//! disagrees fails a test here.
//!
//! The same discipline governs Leg 2. The wire measurement is `frame.len()` on the bytes
//! this file encoded and the bytes the daemon answered with. It never asks the operation how
//! many bytes it saved, because an operation that lied about its own saving is exactly the
//! failure this leg exists to exclude.
//!
//! # What this file does NOT decide
//!
//! It does not adjudicate G0-DX-10; that is `bn-762i`'s, and the decision was the user's. It
//! does not re-open the accounting question X1, which RFC 0027 correction 31 pinned. It does
//! not measure whether a live model would use either surface better — no artifact in this
//! lane does, and the exit package records that absence as N3. And it states one caveat of
//! its own, in [`leg1_the_lossless_floor_is_conditional_on_charging_the_handshake`]: the
//! unreachability proof is load-bearing on the pinned per-task handshake charge, which the
//! ledger's own §5 route 1 names and this file measures the size of.
//!

use continuum_engine_reference::diehard;
use continuum_intent::canonical_json::Json;
use continuum_intent::contract::IntentContract;
use continuum_value::epoch::ProtocolWindow;
use continuum_workspace::snapshot::WorkspacePath;
use continuumd::daemon::family::{Arguments, Payload};
use continuumd::daemon::identity::{self, Blake3Identity};
use continuumd::daemon::intent::IntentFamily;
use continuumd::daemon::state::{DaemonState, IntentRecord, RegistryStatus};
use continuumd::daemon::task::TaskFamily;
use continuumd::daemon::verification::{VerificationFamily, model_source};
use continuumd::daemon::workspace::WorkspaceFamily;
use continuumd::daemon::{Daemon, OperationRequest};
use continuumd::protocol::envelope::{Budget, EpochSet, OutputPolicy, RequestEnvelope};
use continuumd::protocol::handshake::{
    CapabilityDescriptor, CapabilityProfile, ClientHello, Negotiated, VersionRange, negotiate,
};
use continuumd::protocol::operations::intent::IntentAcceptRequest;
use continuumd::protocol::operations::task::TaskStatusRequest;
use continuumd::protocol::operations::verification::VerificationStartRequest;
use continuumd::protocol::operations::workspace::{
    WorkspaceCreateByReferenceRequest, WorkspaceCreateRequest,
};
use continuumd::protocol::registry::{ENCODINGS, OPERATIONS, PROTOCOL_VERSION};
use continuumd::protocol::scalar::{
    ActorId, ByteCount, CapabilityHandle, Commitment, EpochIdentity, IntentHandle, Opaque,
    OperationName, ProtocolVersion, RequestId, TaskHandle, Timestamp,
};
use continuumd::protocol::shared::{SnapshotComponents, SnapshotEpochs, Target};
use continuumd::protocol::spec::{Nullable, Optional};
use continuumd::protocol::vocabulary::{
    AuthorityLevel, Encoding, ErrorCode, OmissionReason, Portfolio, ResultStatus, TargetKind,
};
use continuumd::transport::{Server, decode_result, encode_request};

// ================================================================================================
// The pinned artifacts, as text.
//
// Read rather than restated. A constant transcribed into this file would be a second spelling
// that could drift from the first; a parse of the artifact's own bytes cannot.
// ================================================================================================

/// The byte ledger: the raw measured decomposition every headline figure descends from.
const LEDGER: &str = include_str!("../../../notes/plan/notes/DX10_BYTE_LEDGER.md");

/// The exit package: the headline figures, as claimed.
const EXIT_PACKAGE: &str = include_str!("../../../notes/plan/notes/PHASE_A_EXIT_PACKAGE.md");

/// The G0 matrix: the row's Status and Decision cells.
const MATRIX: &str = include_str!("../../../notes/plan/notes/G0_SPIKE_MATRIX.md");

/// The release gates: the criterion's own sentence.
const GATES: &str = include_str!("../../../notes/plan/docs/52_RELEASE_GATES_REV3.md");

/// The kill-08 checkpoint: the fallible-family taxonomy the invalid-action pass rests on.
const KILL08: &str = include_str!("../../../notes/plan/notes/KILL08_CHECKPOINT.md");

/// The protocol declaration: the redesign's substance, as a schema rather than as a promise.
const IDL: &str = include_str!("../../../notes/plan/schemas/continuumd-native-protocol.idl");

/// The dossier validator's own output: the counts the freeze precondition is stated in.
const VALIDATION: &str = include_str!("../../../notes/plan/validation-results.json");

// --- Markdown, parsed just far enough ---------------------------------------------------------

/// The cells of one Markdown table row, stripped of emphasis and code decoration.
fn cells(line: &str) -> Vec<String> {
    line.trim()
        .trim_start_matches('|')
        .trim_end_matches('|')
        .split('|')
        .map(|cell| cell.replace(['`', '*'], "").trim().to_owned())
        .collect()
}

/// One `## ` section of a document, heading to next heading.
fn section<'a>(text: &'a str, heading: &str) -> &'a str {
    let start = text
        .find(heading)
        .unwrap_or_else(|| panic!("the document carries the section {heading:?}"));
    let rest = &text[start + heading.len()..];
    let end = rest.find("\n## ").map_or(rest.len(), |offset| offset + 1);
    &rest[..end]
}

/// The first row of `arity` cells whose first cell is `label`.
///
/// Arity discriminates, because one document holds several tables and a label can repeat
/// across them with a different meaning — `workspace.create` names a row in the ledger's
/// per-operation table and another in its per-artifact table, and the two are not the same
/// number.
fn row(text: &str, label: &str, arity: usize) -> Vec<String> {
    text.lines()
        .filter(|line| line.trim_start().starts_with('|'))
        .map(cells)
        .find(|row| row.len() == arity && row.first().is_some_and(|first| first == label))
        .unwrap_or_else(|| panic!("no {arity}-cell row labelled {label:?}"))
}

/// The first decimal count in a cell, with the ledger's thousands separators removed.
///
/// The *first* run rather than every digit in the cell, because a cell reads
/// `10,384 B native vs 4,118 B shell` and concatenating its digits would invent a number
/// nobody wrote.
fn count(cell: &str) -> u64 {
    let text = cell.replace(',', "");
    let digits: String = text
        .chars()
        .skip_while(|character| !character.is_ascii_digit())
        .take_while(char::is_ascii_digit)
        .collect();
    digits
        .parse()
        .unwrap_or_else(|_| panic!("{cell:?} carries no count"))
}

/// The number a sentence carries between two fixed phrases.
fn phrase(text: &str, before: &str, after: &str) -> u64 {
    let start = text
        .find(before)
        .unwrap_or_else(|| panic!("the artifact carries the phrase {before:?}"))
        + before.len();
    let rest = &text[start..];
    let end = rest
        .find(after)
        .unwrap_or_else(|| panic!("the phrase {before:?} is not followed by {after:?}"));
    count(&rest[..end])
}

/// The G0 matrix's `G0-DX-10` row, as one line.
fn dx10_row() -> &'static str {
    MATRIX
        .lines()
        .find(|line| line.starts_with("| G0-DX-10 |"))
        .expect("the matrix carries the row")
}

// ================================================================================================
// LEG 1 — the failure arithmetic, re-derived.
// ================================================================================================

/// The seven operations plus the connection, exactly as the ledger's §2.1 table names them.
const LEDGER_ROWS: [&str; 8] = [
    "workspace.create",
    "workspace.fork",
    "workspace.seal",
    "verification.start",
    "task.status",
    "verification.result",
    "task.resume",
    "connection handshake",
];

/// Everything Leg 1 derives, as one value, from one text.
///
/// A struct rather than loose assertions, so the Leg 3 control can run the *same* derivation
/// over a doctored copy of the ledger and compare whole values. A negative control that
/// re-implements the arithmetic it is controlling is not a control.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Derived {
    /// Solved cells per arm.
    solved: u64,
    /// Wire calls the typed arm makes over the matrix.
    wire_calls: u64,
    /// Typed-arm agent-visible bytes over the matrix, summed from the per-operation table.
    native_total: u64,
    /// Baseline agent-visible bytes over the matrix, summed the same way.
    shell_total: u64,
    /// Typed-arm bytes per solved task, before the 3.6 item.
    native_before: u64,
    /// Typed-arm bytes per solved task, after it.
    native_after: u64,
    /// Baseline bytes per solved task.
    shell_per_solved: u64,
    /// Byte margin before the 3.6 item, in percent.
    margin_before: i64,
    /// Byte margin after it.
    margin_after: i64,
    /// The most a passing typed arm could spend per solved task.
    ceiling_per_solved: u64,
    /// What the ratified floor needs given back, per solved task. Signed, because a doctored
    /// ledger can put the ceiling above the measurement and a control that panicked there
    /// would be a control that could not be run.
    gap_per_solved: i64,
    /// What the 3.6 item gave back, per solved task.
    returned_per_solved: u64,
    /// That, as a percentage of the gap.
    returned_share_percent: i64,
    /// The connection handshake charged per solved task.
    handshake_per_solved: u64,
    /// Wire calls per solved task.
    calls_per_task: u64,
    /// Baseline bytes per call.
    shell_per_call: u64,
    /// The most a passing typed arm could spend per call, request and result together.
    max_native_per_call: u64,
    /// Mean result payload per answer — content no lossless encoding can decline to move.
    mean_payload_per_answer: u64,
    /// Handshake plus result payload plus request payload, over the matrix.
    lossless_floor_total: u64,
    /// The same, per solved task.
    lossless_floor_per_solved: u64,
    /// The margin a typed arm spending exactly that floor would post.
    lossless_floor_margin: i64,
}

/// The margin the ratified sentence grades: percent fewer interface bytes per solved task.
///
/// Written here, in this file, from the sentence rather than from the harness that computes
/// it. Integer arithmetic and truncation toward zero, because the pinned figures are integers
/// and a float's rendering is not a portable fact.
const fn margin(shell_per_solved: u64, native_per_solved: u64) -> i64 {
    ((shell_per_solved as i64 - native_per_solved as i64) * 100) / shell_per_solved as i64
}

/// Derive every Leg 1 figure from one ledger text.
fn derive(ledger: &str) -> Derived {
    let decomposition = section(ledger, "## 2. The measured decomposition");
    let method = section(ledger, "## 1. Method");
    let landing = section(ledger, "## 9. C4b landed");

    // The typed arm makes fewer wire calls than the baseline makes dispatches, because its
    // register refuses four before the wire. §1's scaling rule states the number.
    let wire_calls = phrase(method, "typed arm makes ", " wire calls");

    // §2.1's per-operation table, summed. This is the raw measurement; every headline below
    // is a quotient of it.
    let mut native_total = 0;
    let mut shell_total = 0;
    let mut handshake_total = 0;
    for label in LEDGER_ROWS {
        let cells = row(decomposition, label, 7);
        let native = count(&cells[2]);
        let shell = count(&cells[4]);
        native_total += native;
        shell_total += shell;
        if label == "connection handshake" {
            handshake_total = native;
        }
    }

    // Solved cells, from the exit package's own success row: "24/24 native, 24/24 shell".
    let solved = count(
        row(EXIT_PACKAGE, "Absolute task success", 4)[2]
            .split('/')
            .next()
            .expect("the cell reads solved/runs"),
    );

    // §9.1: the one row the 3.6 item moved, before and after, per call.
    let create = row(
        landing,
        "workspace.create, B per call (request + result)",
        4,
    );
    let create_calls = count(&row(decomposition, "workspace.create", 7)[1]);
    let create_after = count(&create[2]) * create_calls;
    let native_total_after =
        native_total - count(&row(decomposition, "workspace.create", 7)[2]) + create_after;

    let native_before = native_total / solved;
    let native_after = native_total_after / solved;
    let shell_per_solved = shell_total / solved;

    // The ratified floor: at least 30% fewer interface bytes per solved task.
    let floor_percent = phrase(
        EXIT_PACKAGE,
        "| Interface bytes per solved task | ≥ ",
        "% fewer",
    );
    let ceiling_per_solved = shell_per_solved * (100 - floor_percent) / 100;

    // The lossless floor. Three quantities no redesign of an encoding can decline to move:
    // the connection the pinned accounting charges, the answers' own payload, and the request
    // payload that is genuine file content rather than envelope.
    let payload_over_answers = count(&row(decomposition, "payload", 4)[2]);
    let answers = phrase(method, "| answers decomposed | ", " |");
    let fork_arguments = phrase(
        decomposition,
        "carries a further ",
        " B of arguments over 8 calls",
    );
    let payload_scaled = payload_over_answers * wire_calls / answers;
    let lossless_floor_total = handshake_total + payload_scaled + fork_arguments;

    Derived {
        solved,
        wire_calls,
        native_total,
        shell_total,
        native_before,
        native_after,
        shell_per_solved,
        margin_before: margin(shell_per_solved, native_before),
        margin_after: margin(shell_per_solved, native_after),
        ceiling_per_solved,
        gap_per_solved: native_before as i64 - ceiling_per_solved as i64,
        returned_per_solved: native_before - native_after,
        returned_share_percent: match native_before as i64 - ceiling_per_solved as i64 {
            0 => 0,
            gap => (native_before as i64 - native_after as i64) * 100 / gap,
        },
        handshake_per_solved: handshake_total / solved,
        calls_per_task: wire_calls / solved,
        shell_per_call: shell_total / answers,
        // `H + n·rn <= 0.7·n·rs`, solved for `rn`. The baseline's per-call figure is carried
        // as the exact rational `shell_total / answers` rather than as its own truncation, so
        // the bound is not the compound of two roundings.
        max_native_per_call: ((100 - floor_percent) * shell_total * (wire_calls / solved)
            / (100 * answers)
            - handshake_total / solved)
            / (wire_calls / solved),
        mean_payload_per_answer: payload_over_answers / answers,
        lossless_floor_total,
        lossless_floor_per_solved: lossless_floor_total / solved,
        lossless_floor_margin: margin(shell_per_solved, lossless_floor_total / solved),
    }
}

/// **The raw measurement, summed by this file.**
///
/// The ledger's §2.1 per-operation table is the only place in the dossier where the matrix's
/// bytes are broken out per operation, and every downstream figure is a quotient of its two
/// totals. Summing it here is the first thing an audit owes: if the column does not add up,
/// nothing below it means anything.
#[test]
fn leg1_the_per_operation_table_sums_to_the_totals_the_ledger_states() {
    let derived = derive(LEDGER);
    let decomposition = section(LEDGER, "## 2. The measured decomposition");
    let stated = row(decomposition, "total", 7);

    assert_eq!(
        derived.native_total,
        count(&stated[2]),
        "the typed arm's per-operation column sums to its stated total"
    );
    assert_eq!(
        derived.shell_total,
        count(&stated[4]),
        "and so does the baseline's"
    );
    assert_eq!(derived.native_total, 260_630);
    assert_eq!(derived.shell_total, 98_836);
    assert_eq!(derived.solved, 24, "24 solved cells per arm");
    assert_eq!(derived.wire_calls, 168);
}

/// **The headline bytes figures, recomputed.**
///
/// `10,384 B native vs 4,118 B shell = −152%` is the exit package's claim. Here it is a
/// quotient of the table above and the one row the 3.6 item moved — arrived at by a division
/// this file performs, and only then compared with what the package says.
#[test]
fn leg1_the_bytes_per_solved_task_and_the_minus_152_percent_margin_re_derive() {
    let derived = derive(LEDGER);

    // Pre-redesign, from the sum alone.
    assert_eq!(derived.native_before, 10_859, "260,630 / 24");
    assert_eq!(derived.shell_per_solved, 4_118, "98,836 / 24");
    assert_eq!(derived.margin_before, -163);

    // Post-redesign: one row of the table replaced by its landed value, nothing else touched.
    assert_eq!(derived.native_after, 10_384);
    assert_eq!(derived.margin_after, -152);
    assert_eq!(derived.returned_per_solved, 475);

    // And the claim, read out of the exit package rather than remembered.
    let claimed = row(EXIT_PACKAGE, "Interface bytes per solved task", 4);
    assert!(
        claimed[2].contains("10,384 B native vs 4,118 B shell"),
        "the package's own measured cell: {}",
        claimed[2]
    );
    assert!(claimed[2].contains("−152%"), "{}", claimed[2]);
    assert!(claimed[3].starts_with("FAIL"), "{}", claimed[3]);
}

/// **The floor, the gap, and what the 3.6 item was worth against it.**
///
/// A passing row needs the typed arm at or below 70% of the baseline. The distance from there
/// is the number that decides whether "redesign" was ever going to be enough, and the item
/// that landed closed 5% of it.
#[test]
fn leg1_the_ratified_floor_and_the_share_the_landed_item_returned() {
    let derived = derive(LEDGER);

    assert_eq!(derived.ceiling_per_solved, 2_882, "4,118 × 0.70");
    assert_eq!(derived.gap_per_solved, 7_977, "10,859 − 2,882");
    assert_eq!(
        derived.returned_share_percent, 5,
        "475 of 7,977 — the ledger §9.3 reads 5.9%, and this is its integer floor"
    );

    // The post-item shortfall, so the residue is stated and not implied.
    assert_eq!(
        derived.native_after - derived.ceiling_per_solved,
        7_502,
        "what is still owed after the only item major 3 admitted"
    );
}

/// **The success margin is a tie, and a tie is not a partial pass.**
///
/// The floor is +10 percentage points with a one-sided 95% lower bound above zero. Both arms
/// solve every cell, so the margin is exactly zero and the lower bound cannot be above it.
#[test]
fn leg1_the_success_margin_is_a_measured_tie_against_a_plus_10_point_floor() {
    let claimed = row(EXIT_PACKAGE, "Absolute task success", 4);
    assert!(claimed[1].contains("≥ +10 pp"), "{}", claimed[1]);
    assert!(
        claimed[2].contains("24/24 native, 24/24 shell"),
        "{}",
        claimed[2]
    );

    // Recomputed: success in per-mille, and the margin between the two arms.
    let solved = 24_i64;
    let runs = 24_i64;
    let native_permille = solved * 1000 / runs;
    let shell_permille = solved * 1000 / runs;
    assert_eq!(native_permille, 1000);
    assert_eq!(shell_permille, 1000);
    assert_eq!(native_permille - shell_permille, 0, "the margin is zero");
    assert!(
        (native_permille - shell_permille) < 10 * 10,
        "zero does not reach a +10 point floor"
    );
    assert!(claimed[3].starts_with("TIE"), "{}", claimed[3]);
}

/// **The break-even the invalid-action pass is measured against, recomputed from scratch.**
///
/// `bn-2c0a` fixed the target before the shot: how many invalid attempts the baseline must
/// make that the typed arm does not, before the ratified 50% relative reduction clears. The
/// answer it computed is 0.6 per run, and the exit package quotes it. The loop below is this
/// file's own, over the landed totals, and it lands on the same tenth.
///
/// The landed totals are themselves derived rather than transcribed: 172 dispatches per arm
/// is the ledger's answer count, and 69 per mille pins the invalid count to exactly 12 —
/// `11 × 1000 / 172` is 63 and `13 × 1000 / 172` is 75, so no other integer produces it.
#[test]
fn leg1_the_zero_point_six_break_even_recomputes_from_the_landed_totals() {
    let attempted = phrase(
        section(LEDGER, "## 1. Method"),
        "| answers decomposed | ",
        " |",
    );
    let permille = phrase(dx10_row(), "invalid-action rate ", " per mille both arms");
    assert_eq!(attempted, 172);
    assert_eq!(permille, 69);

    // The invalid count the rate pins, found rather than assumed.
    let candidates: Vec<u64> = (0..=attempted)
        .filter(|invalid| invalid * 1000 / attempted == permille)
        .collect();
    assert_eq!(
        candidates,
        vec![12],
        "69 per mille of 172 is 12 and nothing else"
    );
    let invalid = candidates[0];

    // The break-even: the smallest tenth of a shell-only invalid attempt per run at which the
    // ratified 50% relative reduction clears.
    let runs = 24_u64;
    let native_permille = invalid * 1000 / attempted;
    let break_even = (0..=1000_u64)
        .find(|tenths| {
            let extra = runs * tenths / 10;
            let shell_permille = (invalid + extra) * 1000 / (attempted + extra);
            shell_permille > 0
                && ((shell_permille as i64 - native_permille as i64) * 100) / shell_permille as i64
                    >= 50
        })
        .expect("some rate clears");
    assert_eq!(break_even, 6, "0.6 shell-only invalid attempts per run");

    // And the same number, as the exit package states it.
    let claimed = row(EXIT_PACKAGE, "Invalid-action rate", 4);
    assert!(claimed[2].contains("0.60 break-even"), "{}", claimed[2]);
    assert!(claimed[2].contains("0.95 shell-only"), "{}", claimed[2]);
}

/// **The 61% clearance, recomputed from the family taxonomy and the landed totals.**
///
/// Two of the five mistake families are unrepresentable on the typed surface; the other three
/// measure zero because both surfaces can express them. The measured 0.95 shell-only invalid
/// attempts per run over those two families' 48 cells fixes the extra count at 46, and 46
/// extra invalid attempts on the baseline against none on the typed arm is a 61% relative
/// reduction. Nothing here is transcribed from the delivering test.
#[test]
fn leg1_the_61_percent_invalid_action_clearance_re_derives() {
    // The taxonomy: five families, and the ones the typed surface prevents.
    let families = ["mf-01", "mf-02", "mf-03", "mf-04", "mf-05"];
    let prevented: Vec<&str> = families
        .into_iter()
        .filter(|family| row(KILL08, family, 6)[5] != "0%")
        .collect();
    assert_eq!(prevented, vec!["mf-03", "mf-05"], "two of five");
    for family in &prevented {
        assert_eq!(
            row(KILL08, family, 6)[3],
            "unrepresentable",
            "{family} is prevented because the typed surface cannot spell it"
        );
    }

    let runs_per_family = 24_u64;
    let attempted_per_family = 172_u64;
    let invalid_per_family = 12_u64;
    let cells = prevented.len() as u64 * runs_per_family;

    // 0.95 shell-only invalid attempts per run, in hundredths, over those cells. The integer
    // count is found, not assumed: 45 gives 93 hundredths and 47 gives 97.
    let hundredths = 95_u64;
    let extra: Vec<u64> = (0..=cells)
        .filter(|extra| extra * 100 / cells == hundredths)
        .collect();
    assert_eq!(extra, vec![46], "46 of 48 cells landed their mistake");
    let extra = extra[0];

    let native_attempted = prevented.len() as u64 * attempted_per_family;
    let native_invalid = prevented.len() as u64 * invalid_per_family;
    let shell_attempted = native_attempted + extra;
    let shell_invalid = native_invalid + extra;

    let native_permille = native_invalid * 1000 / native_attempted;
    let shell_permille = shell_invalid * 1000 / shell_attempted;
    assert_eq!(native_permille, 69);
    assert_eq!(shell_permille, 179);

    let reduction =
        ((shell_permille as i64 - native_permille as i64) * 100) / shell_permille as i64;
    assert_eq!(reduction, 61, "the ratified 50% floor clears at 61%");
    assert!(reduction >= 50);

    let claimed = row(EXIT_PACKAGE, "Invalid-action rate", 4);
    assert!(claimed[2].contains("61%"), "{}", claimed[2]);
    assert!(claimed[3].starts_with("PASS"), "{}", claimed[3]);
}

/// **The composite rule: two of three margins missed, so the first disjunct fails.**
///
/// The ratified sentence is a conjunction of three margins and says what happens when any one
/// is missed. This is that sentence evaluated, rather than the verdict quoted.
#[test]
fn leg1_the_composite_rule_fires_and_the_first_disjunct_fails() {
    let derived = derive(LEDGER);

    let success_clears = 0_i64 >= 10;
    let bytes_clear = derived.margin_after >= 30;
    let invalid_clears = 61_i64 >= 50;

    assert!(!success_clears, "a tie does not clear +10 points");
    assert!(!bytes_clear, "−152% does not clear +30%");
    assert!(invalid_clears, "61% does clear 50%");
    assert!(
        !(success_clears && bytes_clear && invalid_clears),
        "missing any one of the three fails G0-DX-10"
    );

    // And the matrix row records exactly that, with the consequence in force.
    assert!(
        dx10_row().contains(
            "Closed (failed as measured — consequence discharged by user-ratified redesign, \
             bn-762i)"
        ),
        "the Status cell states the failure and its discharge"
    );
    assert!(
        dx10_row().contains("this row closes as **failed**, not as passed"),
        "and refuses to read the discharge as a pass"
    );
}

/// **The floor argument, re-derived: the ratified budget is smaller than the content.**
///
/// The ledger's §5 states the condition algebraically — `H + n·rn ≤ 0.7·n·rs` — and concludes
/// that no lossless protocol reaches it. This re-derives the bound two ways, and both are
/// arithmetic over the pinned tables rather than restatements of the conclusion.
///
/// **Per call.** With the handshake charged per solved task, a passing typed arm has at most
/// 277 B per call for request and result together. The mean *result payload alone* — the
/// answer's own content, with every envelope member, every key and the whole request frame
/// already at zero — is 293 B. The budget is smaller than the content.
///
/// **Over the matrix.** Sum the three quantities no encoding redesign can decline to move —
/// the connection the pinned accounting charges, the result payloads, and the request-side
/// file content of `workspace.fork` — and the total is above the ratified ceiling. A typed arm
/// spending exactly that floor and nothing else posts +22%, short of +30%.
#[test]
fn leg1_no_lossless_protocol_reaches_the_ratified_floor_on_this_instrument() {
    let derived = derive(LEDGER);

    // The algebraic bound, solved in this file.
    assert_eq!(derived.handshake_per_solved, 875);
    assert_eq!(derived.calls_per_task, 7);
    assert_eq!(
        derived.shell_per_call, 574,
        "98,836 / 172, truncated — the ledger rounds the same quotient to 575"
    );
    assert_eq!(
        derived.max_native_per_call, 277,
        "the whole per-call budget a passing row leaves, both directions"
    );
    assert_eq!(
        derived.mean_payload_per_answer, 293,
        "the answers' own content, per answer"
    );
    assert!(
        derived.mean_payload_per_answer > derived.max_native_per_call,
        "the payload alone is over the budget for payload plus everything else"
    );

    // The aggregate bound.
    assert_eq!(derived.lossless_floor_total, 76_439);
    assert_eq!(derived.lossless_floor_per_solved, 3_184);
    assert!(
        derived.lossless_floor_per_solved > derived.ceiling_per_solved,
        "{} B is over the {} B a passing row allows",
        derived.lossless_floor_per_solved,
        derived.ceiling_per_solved
    );
    assert_eq!(derived.lossless_floor_margin, 22);
    assert!(
        derived.lossless_floor_margin < 30,
        "a protocol that sent content and nothing else still misses the floor"
    );

    // The exit package makes the same claim in words. Both agree, and both are stated as
    // conditional on the pinned accounting rather than as absolute.
    assert!(
        EXIT_PACKAGE.contains("unreachable under the pinned accounting by any lossless protocol"),
        "the package qualifies the claim by the accounting it holds under"
    );
}

/// **The caveat, measured rather than waved at: the proof leans on the handshake charge.**
///
/// INV-007 says an absence is named. The unreachability result above is not unconditional, and
/// the condition is one number: the 875 B connection the pinned accounting charges the typed
/// arm per solved task and charges the baseline not at all. Drop that single term and the same
/// floor clears the ratified margin with room to spare. The ledger's §5 route 1 names this and
/// this test measures how much it is worth, so the conditionality is a figure rather than a
/// footnote.
///
/// This is not an argument that the accounting is wrong. It is `bn-2c0a`'s attack S2, already
/// landed and already recorded in the matrix, and RFC 0027 correction 31 pinned the reading
/// afterwards. It is here because an acceptance layer that reported the proof without its
/// hypothesis would be reporting something stronger than what was proved.
#[test]
fn leg1_the_lossless_floor_is_conditional_on_charging_the_handshake() {
    let derived = derive(LEDGER);

    let without_handshake =
        derived.lossless_floor_total - derived.handshake_per_solved * derived.solved;
    let per_solved = without_handshake / derived.solved;
    let unhandshaken_margin = margin(derived.shell_per_solved, per_solved);

    assert_eq!(per_solved, 2_309);
    assert_eq!(unhandshaken_margin, 43);
    assert!(
        unhandshaken_margin >= 30,
        "with the connection uncharged the same content floor clears the ratified margin"
    );
    assert!(
        derived.lossless_floor_margin < 30,
        "with it charged it does not — so the proof's hypothesis is load-bearing"
    );

    // The instrument's own two-sided record of this: charging the *baseline* a handshake per
    // process moves the margin the other way, and that variant is landed too.
    assert!(
        dx10_row().contains("charging it reaches +47%, which *clears*"),
        "the matrix records the symmetric variant at full strength"
    );
}

/// **The ledger's §9.1 landing table agrees with the §2.1 table it descends from.**
///
/// An audit that reproduces every number and reports no discrepancy has probably not
/// reproduced them — which is exactly what caught this document's prior state: two cells of
/// the landing table did not agree with the table they descend from. §2.1 puts
/// `workspace.create` at 35,304 B over 24 calls, which is 1,471 B per call, and its totals row
/// reads 260,630 B; §9.1's "before" cells used to read 1,470/−474 and 260,616/−11,386 — off by
/// one byte per call and by the resulting 14 B over the matrix. Both are now the exact figures,
/// 1,471/−475 and 260,630/−11,400, and this test is the regression guard: it fails again if
/// either before/delta pair drifts from §2.1's own arithmetic, and it fails if the stale
/// figures reappear anywhere in the document.
///
/// The landed after-figure, 249,230, was never in question — it is exactly 260,630 − 11,400,
/// the true delta of the create row — and every headline figure is unchanged by the
/// correction: 10,859, 10,384, −475 per solved task, −163% and −152%, because integer division
/// by 24 absorbs a discrepancy this small either way.
#[test]
fn leg1_the_ledger_landing_table_agrees_with_the_table_it_descends_from() {
    let decomposition = section(LEDGER, "## 2. The measured decomposition");
    let landing = section(LEDGER, "## 9. C4b landed");

    let table_row = row(decomposition, "workspace.create", 7);
    let calls = count(&table_row[1]);
    let bytes = count(&table_row[2]);
    assert_eq!((calls, bytes), (24, 35_304));
    assert_eq!(bytes / calls, 1_471, "the exact per-call figure");

    let landing_row = row(
        landing,
        "workspace.create, B per call (request + result)",
        4,
    );
    assert_eq!(
        count(&landing_row[1]),
        bytes / calls,
        "§9.1's before-cell equals §2.1's own row exactly"
    );
    assert_eq!(count(&landing_row[1]), 1_471, "§9.1 states 1,471");
    assert_eq!(count(&landing_row[3]), 475, "and a delta of 475");

    let over_the_matrix = row(landing, "native, B over the matrix", 4);
    let before = count(&over_the_matrix[1]);
    let after = count(&over_the_matrix[2]);
    let delta = count(&over_the_matrix[3]);
    assert_eq!((before, after, delta), (260_630, 249_230, 11_400));
    assert_eq!(before - delta, after, "§9.1 is internally consistent");

    let summed = derive(LEDGER).native_total;
    assert_eq!(summed, 260_630);
    assert_eq!(
        summed, before,
        "§9.1's before-cell now agrees with §2.1's own sum exactly"
    );
    assert_eq!(
        summed - (bytes - count(&landing_row[2]) * calls),
        after,
        "and §2.1's sum, with the create row replaced by its landed value, still lands on \
         249,230 exactly"
    );

    // The headline quotients, unmoved by the correction.
    assert_eq!(summed / 24, 10_859);
    assert_eq!(before / 24, 10_859);
    assert_eq!(after / 24, 10_384);

    // The stale transcription is gone from the document, not merely superseded.
    assert!(
        !LEDGER.contains("260,616"),
        "the stale before-cell must not reappear"
    );
    assert!(
        !LEDGER.contains("11,386"),
        "the stale delta must not reappear"
    );
    assert!(
        !landing.contains("1,470 | 996"),
        "the stale per-call before-cell must not reappear"
    );
}

// ================================================================================================
// LEG 2 — the redesign disjunct's discharge, as artifacts and as bytes.
// ================================================================================================

/// **The decision trail is a recorded act, in the artifacts a reader can check.**
///
/// The bones themselves are outside a test's reach — `bn-762i` and `bn-3861i` are quoted in the
/// bone comment and the final report. What is checkable here is that the dossier the bones
/// commit to says what the bones say, in three independent documents, and that the matrix row's
/// Decision cell records the disjunct rather than a pass.
#[test]
fn leg2_the_decision_trail_agrees_across_the_matrix_the_plan_and_the_package() {
    // The criterion, as the gates state it — a disjunction, and this is the text under audit.
    assert!(
        GATES.contains(
            "native ACI beats the disciplined shell baseline on success and cost, or the \
             protocol is redesigned before freeze (G0-DX-10)"
        ),
        "the gate sentence is the disjunction this file audits"
    );

    // The package's verdict on this clause, and its refusal to call it a pass.
    assert!(EXIT_PACKAGE.contains("**Verdict: SATISFIED-BY-DISJUNCT.**"));
    assert!(EXIT_PACKAGE.contains("**The first disjunct FAILED as measured.**"));
    assert!(
        EXIT_PACKAGE
            .contains("second disjunct — protocol redesign before freeze — on a decision the user\nratified on 2026-08-11"),
        "the ratification is dated and attributed to the user"
    );
    let clause = row(EXIT_PACKAGE, "G2-06", 3);
    assert!(
        clause[2].starts_with("satisfied-by-disjunct"),
        "the package's own G2-06 row: {}",
        clause[2]
    );

    // The matrix row: the two executed items, named.
    assert!(dx10_row().contains("`OutputPolicy.max_bytes` enforced for the first time (bn-6fuu5)"));
    assert!(dx10_row().contains("`workspace.create_by_reference` live at 3.6 (bn-3of5h)"));
    assert!(dx10_row().contains("Ratified as the standing 4.0 plan (bn-3861i)"));
    assert!(dx10_row().contains("**Phase A's protocol freezes at 3.6.**"));
}

/// **"Before freeze" is temporally coherent: the redesign landed at the version the freeze
/// freezes at, and the freeze itself is a gate that has not closed.**
///
/// Two halves. The version half is mechanical: the redesign's operation is declared `@since
/// 3.6`, the registry serves 3.6, and 3.6 is the version the ratified decision freezes Phase A
/// at — so the redesign is inside the frozen surface rather than after it. The act half is
/// documentary and is stated in the bone comment: `bn-1grk`, the Phase A exit gate that closes
/// the freeze, is `goal:manual` and still `open`, and this bone is one of its dependencies.
#[test]
fn leg2_the_redesign_landed_at_the_version_the_freeze_freezes_at() {
    assert_eq!(PROTOCOL_VERSION, "3.6", "the registry serves 3.6");
    assert!(
        IDL.contains("  version = \"3.6\";"),
        "and the IDL declares the same version"
    );

    // The operation, declared at that version and nowhere earlier. Line-based rather than a
    // byte slice, because the IDL's doc comments carry multi-byte punctuation and a fixed
    // window would eventually land inside a character.
    let annotated: Vec<&str> = IDL
        .lines()
        .skip_while(|line| line.trim() != "@since(\"3.6\")")
        .take(2)
        .collect();
    assert_eq!(
        annotated,
        [
            "@since(\"3.6\")",
            "operation workspace.create_by_reference {"
        ],
        "the `@since 3.6` annotation sits on the redesign's own operation"
    );
    assert_eq!(
        IDL.matches("@since(\"3.6\")").count(),
        1,
        "and 3.6 introduced exactly one declaration"
    );

    assert!(
        MATRIX.contains("**Phase A's protocol freezes at 3.6.**"),
        "and the freeze is recorded at that version, not before it"
    );
    assert!(
        GATES.contains("A failed or unexecuted freeze-blocking item blocks interface freeze."),
        "the freeze is a gated act, so `before freeze` names an ordering that exists"
    );

    // The freeze *precondition* is now met, and G0-DX-10 was the last item holding it: no
    // matrix row's Status cell still opens with `Open`, and the dossier validator's own G0
    // count reads zero. The freeze *act* is a separate, later, manual gate — `bn-1grk`, still
    // open and carrying this bone among its dependencies — which is the documentary half of
    // this leg and is recorded on the bone rather than asserted here.
    for line in MATRIX.lines().filter(|line| line.starts_with("| G0-DX-")) {
        let status = &cells(line)[5];
        assert!(
            !status.starts_with("Open"),
            "a row still reads freeze-blocking: {status}"
        );
    }
    assert!(
        VALIDATION.contains("\"open_freeze_blocking\": 0"),
        "the validator's own count of open freeze-blocking G0 items"
    );
}

/// **The operation exists in the served registry, at the authority and shape the IDL declares.**
#[test]
fn leg2_create_by_reference_is_served_and_matches_its_declaration() {
    let spec = OPERATIONS
        .iter()
        .find(|spec| spec.name == "workspace.create_by_reference")
        .expect("the daemon's registry serves the operation");
    assert_eq!(spec.authority, AuthorityLevel::Propose);

    let names: Vec<&str> = spec.request.fields.iter().map(|field| field.name).collect();
    assert_eq!(
        names,
        ["components", "epochs", "intent", "seal"],
        "the request names a component set by commitment, and carries no components"
    );
    let responses: Vec<&str> = spec
        .response
        .fields
        .iter()
        .map(|field| field.name)
        .collect();
    assert_eq!(responses, ["snapshot", "sealed", "diagnostics"]);

    // The declaration the registry mirrors, quoted from the IDL itself.
    assert!(
        IDL.contains("`workspace.create_by_reference.components` is the content identity of a"),
        "`rule snapshot.by_reference` names the mechanism"
    );
}

/// **The promised reduction is real, and it is measured on the wire by this file.**
///
/// The redesign's substance: naming a component set by its content identity instead of
/// enumerating it. The two spellings are driven live over the same daemon with the same
/// components, the same epochs, the same intent and the same seal, and the only thing that
/// differs is the argument. The bytes are `frame.len()` on the frames this test encoded and
/// the daemon answered with — the operation is never asked what it saved.
///
/// Three things are asserted and all three are needed. The reduction is **strictly positive**
/// and in the promised direction. The result frames are **the same size**, so the saving is on
/// the request as the mechanism claims and not an answer that got smaller by saying less. And
/// the two spellings **publish the same `ws_` handle**, which is a content identity — so
/// nothing was traded for the bytes.
#[test]
fn leg2_the_by_reference_form_moves_strictly_fewer_wire_bytes_than_the_by_value_form() {
    let mut fixture = fixture();
    let set = components(&fixture);
    let reference = fixture
        .daemon
        .state_mut()
        .register_components(&Blake3Identity, set.clone())
        .expect("blake3 names every input");

    let by_value = encode_request(
        &keyed(
            envelope("workspace.create", "agent:builder", "cap_builder", "req_v"),
            "idem-value",
        ),
        &Arguments::WorkspaceCreate(WorkspaceCreateRequest {
            components: set.clone(),
            overlay: Optional::Absent,
            seal: Optional::Present(true),
        }),
    )
    .expect("the by-value request encodes");

    let by_reference = encode_request(
        &keyed(
            envelope(
                "workspace.create_by_reference",
                "agent:builder",
                "cap_builder",
                "req_r",
            ),
            "idem-reference",
        ),
        &Arguments::WorkspaceCreateByReference(WorkspaceCreateByReferenceRequest {
            components: reference.clone(),
            epochs: snapshot_epochs(),
            intent: fixture.intent.clone(),
            seal: Optional::Present(true),
        }),
    )
    .expect("the by-reference request encodes");

    let mut server = Server::new(fixture.daemon, negotiated());
    let value_answer = server.answer(&by_value).expect("the daemon answers");
    let reference_answer = server.answer(&by_reference).expect("the daemon answers");

    let (value_result, value_payload) =
        decode_result("workspace.create", &value_answer).expect("the client decodes");
    let (reference_result, reference_payload) =
        decode_result("workspace.create_by_reference", &reference_answer)
            .expect("the client decodes");
    assert_eq!(
        value_result.status,
        ResultStatus::Ok,
        "{:?}",
        value_result.error
    );
    assert_eq!(
        reference_result.status,
        ResultStatus::Ok,
        "{:?}",
        reference_result.error
    );

    // Same act, same content identity. `ws_` is a Merkle root, so this is the strongest
    // available statement that nothing was traded away.
    let Payload::WorkspaceCreate(value_body) = value_payload else {
        panic!("expected a workspace.create payload");
    };
    let Payload::WorkspaceCreateByReference(reference_body) = reference_payload else {
        panic!("expected a workspace.create_by_reference payload");
    };
    assert_eq!(
        value_body.snapshot, reference_body.snapshot,
        "two spellings, one snapshot"
    );
    assert_eq!(value_body.sealed, reference_body.sealed);

    // The measurement. Requests shrink; answers do not move.
    assert!(
        by_reference.len() < by_value.len(),
        "the by-reference request is {} B against the by-value {} B",
        by_reference.len(),
        by_value.len()
    );
    assert_eq!(
        value_answer.len(),
        reference_answer.len(),
        "the saving is on the request, not on a smaller answer"
    );

    // The measured figures, pinned. A change that moves them should be loud, which is the same
    // reason `daemon_workspace_reference.rs` pins its frames as byte strings.
    //
    // The by-value request measures **718 B**, and the ledger's §2.5 measured a Die Hard
    // `workspace.create` request at 718 B on an instrument built by other people for another
    // purpose. Two fixtures, one number: the ledger's request-side accounting is reproducible
    // from outside the crate that produced it.
    let saved = by_value.len() - by_reference.len();
    assert_eq!(by_value.len(), 718, "the by-value request");
    assert_eq!(by_reference.len(), 448, "the by-reference request");
    assert_eq!(
        saved, 270,
        "measured on the wire, not reported by the daemon"
    );
    assert_eq!(value_answer.len(), 524);
    assert!(
        section(LEDGER, "## 2. The measured decomposition").contains("mean 717 B (Die Hard 718"),
        "the ledger's own Die Hard request measurement"
    );

    // And it is a saving worth the bump, not a rounding difference. The ledger's landed figure
    // is 474 B per call on its own fixture and its own component set; this one is 270 B on a
    // smaller set, so the claim held here is the proportion rather than the ledger's byte.
    assert_eq!(
        saved * 100 / by_value.len(),
        37,
        "37% of the by-value request, against the ledger's 32% of its own request-plus-result"
    );
}

/// **`max_bytes` is enforced live, and the enforcement is visible in the answer frame.**
///
/// The second executed redesign item. `OutputPolicy.max_bytes` was declared "the enforced
/// contract" and read by no handler until `bn-6fuu5`. Driven here over the real byte boundary:
/// a ceiling sweep on `task.status`, with the frame length measured by this file at every step.
///
/// Four claims, and each is a property of the measured frames rather than of a flag.
///
/// 1. A caller that states no ceiling is answered whole, and carries no trim record.
/// 2. **Every** answer under a ceiling has a payload at or below it — the declaration's own
///    sentence, checked against the bytes the daemon put on the wire rather than against a
///    flag it set.
/// 3. A ceiling below the record's own size produces a **strictly smaller payload**, and the
///    trim is recorded as an INV-007 omission whose `reason` is `budget` and whose
///    `recoverable_by` names the task — the retrieval route, populated.
/// 4. A ceiling nothing conforming can meet is a **typed refusal**, not an answer over the
///    bound. That is the disposition that makes "enforced" mean something.
///
/// The measurement is the payload rather than the frame, because the payload is what the
/// declaration bounds — and the difference is not pedantry. A trim *adds* an omission record
/// carrying a 69-byte task handle, so a bounded answer's **frame** can be larger than the
/// unbounded one it replaced. That is `DX10_BYTE_LEDGER.md` §3's C7 falsification observed
/// live, and it is asserted below rather than smoothed over: this mechanism is a conformance
/// fix, and it was never a byte saving.
#[test]
fn leg2_max_bytes_is_enforced_on_the_wire_and_the_payload_actually_shrinks() {
    let (fixture, task) = fixture_with_task();
    let mut server = Server::new(fixture.daemon, negotiated());

    // Every `request_id` below is the same length, because the result envelope echoes it and a
    // shorter identifier would move the frame for a reason that has nothing to do with a
    // ceiling. A byte measurement that did not control for that would read its own naming
    // convention as a saving.
    let unbounded = status_frame(&mut server, &task, "req_c99", None);
    let (open, _) = decode_result("task.status", &unbounded).expect("the client decodes");
    assert_eq!(open.status, ResultStatus::Ok, "{:?}", open.error);
    assert!(
        open.omissions
            .iter()
            .all(|omission| omission.reason != OmissionReason::Budget),
        "an unbounded read carries no trim"
    );
    let whole = payload_bytes(&open);

    // A sweep from the answer's own size down to a twentieth of it, so the two crossings —
    // whole to trimmed, and trimmed to refused — are found rather than guessed.
    let span = unbounded.len() as u64;
    let mut trimmed: Option<(u64, usize)> = None;
    let mut refused: Option<u64> = None;
    let mut grew: Option<(u64, usize)> = None;
    let mut previous = usize::MAX;
    for index in 0..20_u64 {
        let ceiling = span * (20 - index) / 20;
        let frame = status_frame(
            &mut server,
            &task,
            &format!("req_c{index:02}"),
            Some(ceiling),
        );
        let (result, _) = decode_result("task.status", &frame).expect("the client decodes");
        match result.status {
            ResultStatus::Ok => {
                let payload = payload_bytes(&result);
                assert!(
                    payload as u64 <= ceiling,
                    "an answer of {payload} B under a {ceiling} B ceiling is the field \
                     unenforced again"
                );
                assert!(
                    payload <= previous,
                    "a tighter ceiling never produced a larger payload: {ceiling}"
                );
                previous = payload;
                if frame.len() >= unbounded.len() && payload < whole && grew.is_none() {
                    grew = Some((ceiling, frame.len()));
                }
                if payload < whole && trimmed.is_none() {
                    let budget: Vec<_> = result
                        .omissions
                        .iter()
                        .filter(|omission| omission.reason == OmissionReason::Budget)
                        .collect();
                    assert!(
                        !budget.is_empty(),
                        "a trim at {ceiling} B produced no INV-007 record"
                    );
                    for omission in budget {
                        assert_eq!(
                            omission
                                .recoverable_by
                                .value()
                                .map(|handle| handle.as_str()),
                            Some(task.as_str()),
                            "INV-007's retrieval half names the task"
                        );
                    }
                    trimmed = Some((ceiling, payload));
                }
            }
            ResultStatus::Error => {
                assert_eq!(
                    result.error.value().map(|error| error.code),
                    Some(ErrorCode::MalformedRequest),
                    "a ceiling below the floor is refused with a declared code"
                );
                if refused.is_none() {
                    refused = Some(ceiling);
                }
            }
            other => panic!("unexpected status {other:?}"),
        }
    }

    let (ceiling, length) = trimmed.expect("some ceiling in the sweep bound the record");
    assert!(
        length < whole,
        "the payload shrank from {whole} B to {length} B at a {ceiling} B ceiling"
    );
    let floor = refused.expect("some ceiling in the sweep was below the floor");
    assert!(
        floor < ceiling,
        "the refusal is below the trim, so the two dispositions are ordered"
    );

    // The measured figures, pinned.
    assert_eq!((unbounded.len(), whole), (1_290, 681), "the whole answer");
    assert_eq!((ceiling, length), (645, 559), "the trim, and what it cost");
    assert_eq!(floor, 516, "the tightest ceiling nothing conforming meets");

    // C7, live: the mechanism bounds the payload and does not reduce the wire.
    let (ceiling, frame) = grew.expect("some bounded answer's frame is not smaller");
    assert!(
        frame >= unbounded.len(),
        "at a {ceiling} B ceiling the frame is {frame} B against {} B unbounded — the INV-007 \
         record costs more than the elision saves, which is why this item is conformance and \
         not a byte saving",
        unbounded.len()
    );
    assert_eq!(
        (ceiling, frame),
        (645, 1_305),
        "122 B off the payload, 15 B onto the frame"
    );
}

/// The bytes of one result envelope's payload, read off the wire.
fn payload_bytes(envelope: &continuumd::protocol::envelope::ResultEnvelope) -> usize {
    envelope
        .payload
        .value()
        .map_or(0, |opaque| opaque.as_bytes().len())
}

// ================================================================================================
// LEG 3 — negative controls. Would either leg notice if it were lied to?
// ================================================================================================

/// A copy of the ledger with one cell replaced.
fn doctored(from: &str, to: &str) -> String {
    assert_eq!(
        LEDGER.matches(from).count(),
        1,
        "the control edits exactly one place: {from:?}"
    );
    LEDGER.replace(from, to)
}

/// **A doctored ledger flips the arithmetic, in the direction the doctoring pushes.**
///
/// The recomputation is only evidence if it is a function of the data. Three perturbations,
/// each a plausible shape of dishonesty, and each caught by a different figure.
#[test]
fn leg3_a_doctored_ledger_moves_every_figure_that_descends_from_it() {
    let honest = derive(LEDGER);

    // (a) Shrink the typed arm's largest row. A ledger that understated `task.status` by an
    // order of magnitude would report a margin that nearly clears.
    let flattered = derive(&doctored(
        "| `task.status` | 48 | 78,144 | 1,628 | 25,704 | 535 | 3.0x |",
        "| `task.status` | 48 | 7,814 | 163 | 25,704 | 535 | 3.0x |",
    ));
    assert_ne!(flattered.native_before, honest.native_before);
    assert_eq!(flattered.native_before, 7_929);
    assert_eq!(
        flattered.margin_before, -92,
        "the doctored margin is not −163%"
    );
    assert!(
        flattered.margin_before > honest.margin_before,
        "and it flatters the typed arm, which is the point of doctoring it"
    );

    // (b) Inflate the baseline. The other way to manufacture a pass is to make the thing being
    // beaten more expensive. Here it is enough to flip the sign of the whole verdict.
    let inflated = derive(&doctored(
        "| `task.status` | 48 | 78,144 | 1,628 | 25,704 | 535 | 3.0x |",
        "| `task.status` | 48 | 78,144 | 1,628 | 2,570,400 | 53,550 | 3.0x |",
    ));
    assert!(
        inflated.margin_before > 30,
        "a doctored baseline reads as a pass at {}%, which the honest data never does",
        inflated.margin_before
    );
    assert!(honest.margin_before < 0);

    // (c) Erase the payload. The lossless-floor argument rests on content no encoding can
    // decline to move; a ledger that under-reported it would make the floor look reachable.
    let hollow = derive(&doctored(
        "| `payload` | — | 50,484 | — |",
        "| `payload` | — | 5,048 | — |",
    ));
    assert!(
        hollow.lossless_floor_margin >= 30,
        "with the payload hollowed out the floor clears at {}%",
        hollow.lossless_floor_margin
    );
    assert!(
        honest.lossless_floor_margin < 30,
        "and on the real data it does not, at {}%",
        honest.lossless_floor_margin
    );
}

/// **The wire measurement would catch a by-reference form that secretly shipped the value.**
///
/// The failure this leg exists to exclude: an operation that claims a reduction, reports one,
/// and puts the components on the wire anyway. The detector is a predicate over the *frame's
/// bytes* — does this request carry the component set's own content, member by member — and it
/// is shown to work by being run against a frame that really does carry them.
///
/// A predicate that only ever returned `false` would pass the by-reference half of this test
/// and prove nothing. The by-value half is what makes the by-reference half evidence.
#[test]
fn leg3_a_by_reference_frame_that_shipped_its_components_would_be_detected() {
    let mut fixture = fixture();
    let set = components(&fixture);
    let reference = fixture
        .daemon
        .state_mut()
        .register_components(&Blake3Identity, set.clone())
        .expect("blake3 names every input");

    let by_value = encode_request(
        &keyed(
            envelope("workspace.create", "agent:builder", "cap_builder", "req_v"),
            "idem-value",
        ),
        &Arguments::WorkspaceCreate(WorkspaceCreateRequest {
            components: set.clone(),
            overlay: Optional::Absent,
            seal: Optional::Present(true),
        }),
    )
    .expect("the by-value request encodes");

    let by_reference = encode_request(
        &keyed(
            envelope(
                "workspace.create_by_reference",
                "agent:builder",
                "cap_builder",
                "req_r",
            ),
            "idem-reference",
        ),
        &Arguments::WorkspaceCreateByReference(WorkspaceCreateByReferenceRequest {
            components: reference.clone(),
            epochs: snapshot_epochs(),
            intent: fixture.intent.clone(),
            seal: Optional::Present(true),
        }),
    )
    .expect("the by-reference request encodes");

    // The detector: every file commitment of the set, looked for in the frame's bytes.
    let ships = |frame: &[u8], set: &SnapshotComponents| -> usize {
        let text = String::from_utf8(frame.to_vec()).expect("canonical JSON is UTF-8");
        set.files
            .iter()
            .filter(|commitment| text.contains(commitment.as_str()))
            .count()
    };

    assert!(!set.files.is_empty(), "the set has content to ship");
    assert_eq!(
        ships(&by_value, &set),
        set.files.len(),
        "the detector fires on the form that really does ship the components"
    );
    assert_eq!(
        ships(&by_reference, &set),
        0,
        "and finds none of them in the by-reference frame"
    );

    // The structural half: the by-reference frame carries the commitment and none of the
    // member keys the value form spends its bytes on.
    let text = String::from_utf8(by_reference.clone()).expect("canonical JSON is UTF-8");
    assert!(
        text.contains(reference.as_str()),
        "the reference itself is on the wire"
    );
    for key in [
        "cml_modules",
        "domain_packs",
        "proof_environment",
        "rust_extraction",
    ] {
        assert!(
            !text.contains(key),
            "a by-reference request carries no `{key}` member"
        );
        assert!(
            String::from_utf8(by_value.clone())
                .expect("canonical JSON is UTF-8")
                .contains(key),
            "and the by-value request does, which is what makes the check discriminating"
        );
    }
}

/// **The equality assertions in Leg 2 are not vacuous.**
///
/// Leg 2 asserts that two spellings publish the same snapshot handle. A test that compared two
/// values it never checked were derivable independently would pass on a daemon that returned a
/// constant. Here the handle is checked against the identity a *client* derives for the
/// component set from its own bytes, through the declared seam and not through the answer.
#[test]
fn leg3_the_same_snapshot_claim_is_checked_against_a_client_derived_identity() {
    let mut fixture = fixture();
    let set = components(&fixture);
    let registered = fixture
        .daemon
        .state_mut()
        .register_components(&Blake3Identity, set.clone())
        .expect("blake3 names every input");
    let derived = DaemonState::components_identity(&Blake3Identity, &set)
        .expect("the identity seam names the value");

    assert_eq!(
        derived, registered,
        "the reference is a function of the components, not a token the daemon invented"
    );

    // And a set that differs by one member gets a different identity, so the seam discriminates.
    let mut other = set.clone();
    other
        .configuration
        .push(Commitment::new("cm_a-different-member"));
    let other_identity = DaemonState::components_identity(&Blake3Identity, &other)
        .expect("the identity seam names the value");
    assert_ne!(derived, other_identity);
}

// ================================================================================================
// The fixture. A Die Hard workspace, an accepted contract, a registered model, one campaign.
// ================================================================================================

/// The TV-009 port's model, verbatim.
const DIE_HARD_MODEL: &str =
    include_str!("../../../notes/plan/corpus/tla-examples/ports/TV-009/DieHard.ctm");

/// The TV-009 port's default model configuration.
const DIE_HARD_CONFIG: &str =
    include_str!("../../../notes/plan/corpus/tla-examples/ports/TV-009/default.model.toml");

/// The Die Hard Intent Contract, as `continuum-intent`'s own suites use it.
const DIE_HARD_CONTRACT: &str =
    include_str!("../../continuum-intent/tests/fixtures/die-hard-contract.json");

/// Where the model lives inside the workspace.
const MODULE_PATH: &str = "DieHard.ctm";

/// The daemon under test, plus the handles a caller needs to reach it.
struct Fixture {
    daemon: Daemon,
    intent: IntentHandle,
    files: Vec<Commitment>,
    configuration: Commitment,
}

/// The protocol version this connection negotiates.
///
/// 3.6, because the operation under audit is `@since("3.6")` and a connection below it does not
/// admit the request — which `daemon_workspace_reference.rs` holds as its own version-window
/// test and this file relies on rather than repeats.
fn version() -> ProtocolVersion {
    ProtocolVersion::new(3, 6)
}

fn cap(handle: &str) -> CapabilityHandle {
    CapabilityHandle::new(handle).expect("a well-formed capability handle")
}

fn who(actor: &str) -> ActorId {
    ActorId::new(actor).expect("a well-formed actor identity")
}

fn name(operation: &str) -> OperationName {
    OperationName::new(operation).expect("a well-formed operation name")
}

fn epoch(token: &str) -> EpochIdentity {
    EpochIdentity::new(token).expect("a well-formed epoch identity")
}

fn profile(privileged: &[&str]) -> CapabilityProfile {
    CapabilityProfile {
        privileged_operations: privileged.iter().map(|entry| name(entry)).collect(),
        denied_operations: Vec::new(),
        data_grants: Vec::new(),
        cross_principal_sharing: true,
    }
}

fn grant(
    handle: &str,
    actor: &str,
    level: AuthorityLevel,
    depth: u32,
    profile: Optional<CapabilityProfile>,
) -> CapabilityDescriptor {
    CapabilityDescriptor {
        capability: cap(handle),
        actor: who(actor),
        level,
        snapshots: Vec::new(),
        intents: Vec::new(),
        artifact_classes: Vec::new(),
        expires_at: Nullable::Null,
        delegation_depth: depth,
        profile,
    }
}

fn negotiated() -> Negotiated {
    let hello = ClientHello {
        protocol_versions: VersionRange {
            low: version(),
            high: version(),
        },
        encodings: vec![Encoding::CanonicalJson],
        client: "continuumd-gate-g2-06-acceptance".to_owned(),
        actor: who("service:continuumd"),
        capability: cap("cap_root"),
        features: Optional::Absent,
    };
    negotiate(&[version()], ProtocolWindow::new(3), ENCODINGS, &hello).expect("3.6 is served")
}

fn epochs() -> EpochSet {
    EpochSet {
        protocol: version(),
        semantic: Nullable::Value(epoch("semantic-1")),
        intent: Nullable::Value(epoch("intent-1")),
        evidence: Nullable::Null,
        proof: Nullable::Value(epoch("proof-1")),
        corpus: Nullable::Null,
        engine: Nullable::Value(epoch("engine-reference-1")),
    }
}

fn snapshot_epochs() -> SnapshotEpochs {
    SnapshotEpochs {
        semantic: epoch("semantic-1"),
        proof: epoch("proof-1"),
        toolchain: Optional::Absent,
    }
}

fn envelope(operation: &str, actor: &str, capability: &str, request: &str) -> RequestEnvelope {
    RequestEnvelope {
        protocol_version: version(),
        request_id: RequestId::new(request).expect("a well-formed request id"),
        idempotency_key: Optional::Absent,
        actor: who(actor),
        capability: cap(capability),
        operation: name(operation),
        snapshot: Nullable::Null,
        intent: Nullable::Null,
        arguments: Opaque::from_bytes(Vec::new()),
        budget: Optional::Absent,
        output_policy: Optional::Absent,
        trace: Optional::Absent,
        page: Optional::Absent,
    }
}

fn keyed(mut envelope: RequestEnvelope, key: &str) -> RequestEnvelope {
    envelope.idempotency_key = Optional::Present(key.to_owned());
    envelope
}

fn acceptance_bytes() -> Opaque {
    let mut fields: std::collections::BTreeMap<String, Json> = std::collections::BTreeMap::new();
    for (key, value) in [
        ("accepted_by", "human:steward"),
        ("capability", "revise-intent"),
        ("signature", "sig-die-hard-v1"),
        ("audit_record", "supplied-by-the-caller-and-overwritten"),
        ("timestamp", "2026-08-01T00:00:00.000Z"),
    ] {
        fields.insert(key.to_owned(), Json::String(value.to_owned()));
    }
    Opaque::from_bytes(Json::Object(fields).to_canonical_bytes())
}

fn intent_handle(contract: &IntentContract) -> IntentHandle {
    let stored = continuum_workspace::publication::ContentIdentifier::identify(
        &Blake3Identity,
        continuum_workspace::artifact_path::ArtifactClass::IntentContract,
        &contract.identity_preimage_bytes(),
    )
    .expect("blake3 names every input");
    identity::intent_to_wire(&stored).expect("an `in_` handle")
}

/// A daemon with the four families the audit needs and a capability tree that reaches them.
fn fixture() -> Fixture {
    let root = Some(cap("cap_root"));
    let mut daemon = Daemon::builder(Blake3Identity, negotiated(), cap("cap_root"))
        .epochs(epochs())
        .now(Timestamp::new("2026-08-01T00:00:00.000Z").expect("a well-formed timestamp"))
        .capability(
            grant(
                "cap_root",
                "service:continuumd",
                AuthorityLevel::Promote,
                4,
                Optional::Present(profile(&["intent.accept", "intent.reject", "intent.lock"])),
            ),
            None,
        )
        .capability(
            grant(
                "cap_builder",
                "agent:builder",
                AuthorityLevel::Propose,
                3,
                Optional::Absent,
            ),
            root.clone(),
        )
        .capability(
            grant(
                "cap_runner",
                "agent:runner",
                AuthorityLevel::Execute,
                3,
                Optional::Absent,
            ),
            root.clone(),
        )
        .capability(
            grant(
                "cap_reader",
                "agent:reader",
                AuthorityLevel::Read,
                3,
                Optional::Absent,
            ),
            root.clone(),
        )
        .capability(
            grant(
                "cap_steward",
                "human:steward",
                AuthorityLevel::ReviseIntent,
                3,
                Optional::Present(profile(&["intent.accept", "intent.reject", "intent.lock"])),
            ),
            root,
        )
        .family(WorkspaceFamily)
        .family(IntentFamily)
        .family(TaskFamily)
        .family(VerificationFamily)
        .build();

    let contract = IntentContract::decode(DIE_HARD_CONTRACT.trim_end().as_bytes())
        .expect("the fixture decodes");
    let intent = intent_handle(&contract);
    daemon.state_mut().put_intent(
        intent.clone(),
        IntentRecord {
            contract,
            status: RegistryStatus::Proposed,
            supersedes: None,
            superseded_by: None,
            acceptance: None,
        },
    );

    let mut files = Vec::new();
    for (path, content) in [(MODULE_PATH, DIE_HARD_MODEL), ("README.md", "# TV-009\n")] {
        files.push(
            daemon
                .state_mut()
                .stage(
                    &Blake3Identity,
                    WorkspacePath::new(path).expect("a workspace path"),
                    content.as_bytes().to_vec(),
                )
                .expect("staging names its content"),
        );
    }
    let configuration = daemon
        .state_mut()
        .stage(
            &Blake3Identity,
            WorkspacePath::new("default.model.toml").expect("a workspace path"),
            DIE_HARD_CONFIG.as_bytes().to_vec(),
        )
        .expect("staging names its content");

    daemon.state_mut().models_mut().register(
        model_source(&Blake3Identity, [(MODULE_PATH, DIE_HARD_MODEL.as_bytes())])
            .expect("blake3 names the module set"),
        diehard::model().expect("the port builds"),
    );

    let accepted = daemon.dispatch(&OperationRequest {
        envelope: keyed(
            envelope(
                "intent.accept",
                "human:steward",
                "cap_steward",
                "req_accept",
            ),
            "idem-accept",
        ),
        arguments: Arguments::IntentAccept(IntentAcceptRequest {
            proposal: intent.clone(),
            acceptance: acceptance_bytes(),
            bundle: Optional::Absent,
        }),
    });
    assert_eq!(
        accepted.envelope.status,
        ResultStatus::Ok,
        "{:?}",
        accepted.envelope.error
    );

    Fixture {
        daemon,
        intent,
        files,
        configuration,
    }
}

/// The component set both spellings name.
fn components(fixture: &Fixture) -> SnapshotComponents {
    SnapshotComponents {
        files: fixture.files.clone(),
        cml_modules: Vec::new(),
        rust_extraction: Vec::new(),
        domain_packs: Vec::new(),
        dependencies: Vec::new(),
        epochs: snapshot_epochs(),
        intent: fixture.intent.clone(),
        correspondence: Vec::new(),
        proof_environment: Vec::new(),
        configuration: vec![fixture.configuration.clone()],
        file_components: Optional::Absent,
    }
}

/// The same fixture with a sealed snapshot and one started campaign, for the `max_bytes` leg.
fn fixture_with_task() -> (Fixture, TaskHandle) {
    let mut fixture = fixture();
    let set = components(&fixture);

    let created = fixture.daemon.dispatch(&OperationRequest {
        envelope: keyed(
            envelope(
                "workspace.create",
                "agent:builder",
                "cap_builder",
                "req_create",
            ),
            "idem-create",
        ),
        arguments: Arguments::WorkspaceCreate(WorkspaceCreateRequest {
            components: set,
            overlay: Optional::Absent,
            seal: Optional::Present(true),
        }),
    });
    assert_eq!(
        created.envelope.status,
        ResultStatus::Ok,
        "{:?}",
        created.envelope.error
    );
    let snapshot = match &created.payload {
        Payload::WorkspaceCreate(response) => response.snapshot.clone(),
        other => panic!("expected a workspace.create payload, got {other:?}"),
    };

    let mut start = keyed(
        envelope(
            "verification.start",
            "agent:runner",
            "cap_runner",
            "req_start",
        ),
        "idem-start",
    );
    start.snapshot = Nullable::Value(snapshot);
    start.budget = Optional::Present(Budget {
        wall_ms: Optional::Absent,
        cpu_ms: Optional::Absent,
        memory_bytes: Optional::Absent,
        states: Optional::Present(64),
        solver_ms: Optional::Absent,
        proof_ms: Optional::Absent,
        tokens: Optional::Absent,
        candidates: Optional::Absent,
        bytes: Optional::Absent,
    });
    let started = fixture.daemon.dispatch(&OperationRequest {
        envelope: start,
        arguments: Arguments::VerificationStart(VerificationStartRequest {
            target: Target {
                kind: TargetKind::AllClaims,
                id: "DieHard".to_owned(),
            },
            portfolio: Portfolio::Interactive,
            context_policy: Optional::Absent,
            priority_class: Optional::Absent,
        }),
    });
    assert_eq!(
        started.envelope.status,
        ResultStatus::TaskStarted,
        "{:?}",
        started.envelope.error
    );
    let task = match &started.payload {
        Payload::VerificationStart(response) => response
            .task
            .value()
            .cloned()
            .expect("a fresh start names a task"),
        other => panic!("expected a verification.start payload, got {other:?}"),
    };

    (fixture, task)
}

/// One `task.status` over the wire, optionally under a stated byte ceiling.
fn status_frame(
    server: &mut Server,
    task: &TaskHandle,
    request: &str,
    max_bytes: Option<u64>,
) -> Vec<u8> {
    let mut envelope = envelope("task.status", "agent:reader", "cap_reader", request);
    if let Some(max_bytes) = max_bytes {
        envelope.output_policy = Optional::Present(OutputPolicy {
            max_bytes: Optional::Present(ByteCount::new(max_bytes)),
            max_tokens: Optional::Absent,
            max_nodes: Optional::Absent,
            audience: Optional::Absent,
        });
    }
    let frame = encode_request(
        &envelope,
        &Arguments::TaskStatus(TaskStatusRequest { task: task.clone() }),
    )
    .expect("the request encodes");
    server.answer(&frame).expect("the daemon answers")
}
