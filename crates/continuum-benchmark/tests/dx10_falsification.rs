//! **G0-DX-10 falsification.** An adversarial campaign against the *instrument*, run from
//! both directions.
//!
//! > | G0-DX-10 | Can agents use the protocol more effectively than CLI scraping? | same
//! > benchmark with native API vs shell output | higher success, lower tokens, fewer invalid
//! > actions | simplify/rework ACI |
//! >
//! > — `notes/plan/notes/G0_SPIKE_MATRIX.md`
//!
//! `pr10_impl01..05` are the *constructive* measurement: five metrics under one accounting
//! rule, fixed before any number was taken. This file attacks that rule and the instrument
//! around it. Its house rule is the one bn-21dd's DX-13 campaign set and bn-1kp6 and bn-3p32
//! kept: **the system under test is frozen.** The daemon and the protocol —
//! `crates/continuumd` and everything below it — are untouched. What is *under test here* is
//! the instrument, so this campaign may add variants beside the landed metrics; it may not
//! weaken one, and
//! [`control_every_landed_number_is_unchanged_by_this_campaign`] is the test that says so.
//!
//! # Both directions, because one direction is advocacy
//!
//! | | The question |
//! |---|---|
//! | **pro-shell-bias** (S1–S7) | is the measured native *loss* an artefact of choices that favour the baseline? |
//! | **pro-native-bias** (N1–N5) | are the claimed typed *advantages* artefacts of choices that favour the typed arm? |
//!
//! # Campaign map — attack → harness choice → test → verdict
//!
//! | # | Harness choice attacked | Test | Verdict |
//! |---|---|---|---|
//! | S1 | the baseline is not charged for the CLI's own protocol frames | [`s1_the_baseline_is_not_charged_for_the_frames_its_cli_actually_exchanged`] | **LANDED** — worth 10,162 bytes/solved task; margin −152% → +27%, still short of +30% |
//! | S2 | nor for the handshake each CLI process would perform | [`s2_the_baseline_is_not_charged_a_handshake_per_process_invocation`] | **LANDED** — margin → +49%, which *clears* the floor |
//! | S3 | the expansion command is answered from an envelope the harness already holds | [`s3_the_expansion_command_never_leaves_the_harness`] | **LANDED** — margin → +55% |
//! | S4 | the adapter renders from the very answer the daemon just produced | [`s4_the_adapter_is_never_a_beat_behind_the_daemon`] | **LANDED** — one stale rendering costs the baseline 4/24 tasks and 502‰ of invalid-action rate |
//! | S5 | one shared policy, so only mistakes *both* surfaces can express are counted | [`s5_the_shared_policy_can_only_make_mistakes_both_surfaces_can_express`] | **LANDED** — break-even is 0.6 shell-only invalid attempts per run |
//! | S6 | the envelope's bytes are treated as one undifferentiated cost | [`s6_where_the_typed_arms_bytes_go_and_how_far_a_redesign_reaches`] | **LANDED** on the *explanation*, **HELD** on the conclusion |
//! | S7 | `ceil(bytes/4)` treats dense JSON and prose alike | [`s7_the_declared_token_rule_cannot_move_the_margin_and_the_one_that_could_moves_it_the_other_way`] | **HELD** |
//! | N1 | a locally refused call costs zero | [`n1_a_call_refused_before_the_wire_is_free_on_the_wire_and_not_free_to_compose`] | **HELD** — worth 69 bytes/solved task |
//! | N2 | every injected fault lands on both arms | [`n2_a_renamed_projection_key_is_a_fault_class_only_the_text_arm_can_suffer`] | **LANDED** — one rename takes the baseline from 24/24 to 0/24 |
//! | N3 | the baseline is *disciplined* | [`n3_the_baselines_whole_result_rests_on_one_of_its_four_disciplines`] | **LANDED** — discipline 1 carries it; 2 and 4 are inert here |
//! | N4 | four tasks over two families | [`n4_every_conclusion_survives_dropping_any_one_task`] | **HELD** |
//! | N5 | completion requires the one datum the baseline pays extra for | [`n5_the_completion_criterion_contains_the_datum_only_one_arm_gets_free`] | **HELD** — worth 119 bytes/solved task, and it widens the loss |
//! | X1 | *whose* interface "interface bytes per solved task" names | recorded in the artifact by [`the_falsification_artifact_carries_a_verdict_for_every_attack_and_a_row_for_every_variant`] | **INCONCLUSIVE** — RFC 0027 fixes the metric and never fixes the interface; the margin is −152% under one reading and +55% under the other, and both are defensible from the text as written |
//!
//! X1 is the attack this campaign could not decide, and it is the one that decides the byte
//! metric. It is recorded as [`Outcome::Inconclusive`] with its reason typed, per INV-008's
//! spirit, rather than resolved by picking the reading that suits an answer.
//!
//! # Controls
//!
//! - [`control_every_landed_number_is_unchanged_by_this_campaign`] — the landed artifact
//!   still reads 10,384 / 4,118 / −152% / 69‰ / 69‰ / 24 / 24. Nothing here weakened it.
//!
//! **The landed figures moved once since this campaign ran, and every number above is
//! the value after that move.** bn-3of5h landed `workspace.create_by_reference` at
//! protocol 3.6 and the typed arm takes it, which took the native arm from 10,859 to
//! **10,384** B per solved task and the byte margin from −163% to −152%. The baseline is
//! byte-for-byte unmoved at 4,118, which is the anti-gaming half of that bone and is
//! asserted in `tests/pr10_c4b_port_by_reference.rs`. Every attack below was re-run
//! against the new matrix and every verdict is unchanged: eight LANDED, four HELD, one
//! INCONCLUSIVE. What moved is the size of the loss, not any conclusion about it.
//! - [`control_the_hidden_ledger_never_enters_an_interface_byte_total`] — the instrumentation
//!   this campaign added is beside the measurement, not inside it.
//! - [`control_a_decomposed_answer_re_encodes_to_the_frame_that_carried_it`] — the byte
//!   decomposition is a measurement of real frames and not of a re-derivation.
//! - [`the_falsification_report_renders_byte_identically_from_two_independent_campaigns`] —
//!   IMPL-05's device, applied to this artifact.

mod support;

use std::collections::BTreeMap;

use continuum_benchmark::falsification::{Attack, Direction, FalsificationReport, Outcome};
use continuum_benchmark::policy::{Call, Step};
use continuum_benchmark::report::{ArmTotals, RATIFIED, Report};
use continuum_benchmark::run::{ArmRun, BYTES_PER_APPROX_TOKEN};
use continuum_benchmark::separation;
use continuum_benchmark::shell::{self, Disciplines, Renderer};
use continuum_benchmark::surface::Arm;
use continuum_benchmark::task::SUBSET;
use continuum_benchmark::variants::{self, Cell, Sensitivity};
use continuumd::protocol::scalar::{ContinuationHandle, TaskHandle, WorkspaceHandle};

// --- the campaign's shared measurements -------------------------------------------------

/// Everything the campaign measures once, so thirteen attacks do not drive thirteen sweeps
/// each.
struct Campaign {
    native: Vec<ArmRun>,
    shell: Vec<Cell>,
    handshake: u64,
    composition: u64,
    decomposition: variants::Decomposition,
}

impl Campaign {
    fn run() -> Self {
        let native = support::native_sweep();
        let shell = variants::metered_shell_sweep().expect("the baseline sweeps");
        let decomposition = variants::decompose().expect("the answers decompose");
        Self {
            handshake: variants::handshake_bytes(),
            composition: decomposition.compose_price("verification.start"),
            native,
            shell,
            decomposition,
        }
    }

    fn shell_runs(&self) -> Vec<ArmRun> {
        self.shell.iter().map(|cell| cell.run.clone()).collect()
    }

    fn under(&self, rule: variants::Accounting) -> Sensitivity {
        variants::under(
            rule,
            &self.native,
            &self.shell,
            self.composition,
            self.handshake,
        )
    }

    fn hidden_total(&self, of: impl Fn(&shell::HiddenCost) -> u64) -> u64 {
        self.shell.iter().map(|cell| of(&cell.hidden)).sum()
    }
}

/// A baseline sweep under a named renderer and discipline set, as plain runs.
fn baseline(renderer: Renderer, disciplines: Disciplines) -> Vec<ArmRun> {
    variants::shell_sweep(renderer, disciplines, false)
        .expect("the baseline sweeps")
        .into_iter()
        .map(|cell| cell.run)
        .collect()
}

/// Interface bytes per solved task, or zero when nothing solved.
fn per_solved(row: &Sensitivity, arm: Arm) -> u64 {
    match arm {
        Arm::Native => row.native_bytes_per_solved,
        Arm::Shell => row.shell_bytes_per_solved,
    }
    .unwrap_or_default()
}

// --- S1: the CLI's own frames ------------------------------------------------------------

/// **S1 — the baseline is not charged for the frames its CLI actually exchanged.**
///
/// `crate::shell` names this handicap in its own module documentation: "the shell arm's
/// interface bytes are the command line it wrote plus the output it read, and the frames
/// underneath are free". The choice is defensible — the agent's interface to a CLI is stdin
/// and stdout — but the sentence being graded is RFC 0027's "interface bytes per solved
/// task", and RFC 0027 fixes the metric without fixing *whose* interface. So the handicap is
/// a number, and this is that number.
///
/// **LANDED.** The CLI exchanged 243,902 bytes of protocol traffic underneath 172 command
/// invocations that the agent was never charged for — two and a half times everything the
/// agent read. Charging it moves the byte margin from −152% to +27%: the sign flips.
/// It still does not reach the ratified +30% floor.
#[test]
fn s1_the_baseline_is_not_charged_for_the_frames_its_cli_actually_exchanged() {
    let campaign = Campaign::run();
    let landed = campaign.under(variants::LANDED);
    let charged = campaign.under(variants::CLI_FRAMES);

    let frames = campaign.hidden_total(|hidden| hidden.frame_bytes());
    let agent = campaign.hidden_total(|_| 0) + landed.shell_bytes_per_solved.unwrap_or_default();
    let _ = agent;
    assert!(
        frames > 2 * 98_836,
        "the CLI's own traffic is more than twice what the agent read: {frames}"
    );

    assert_eq!(landed.byte_saving_percent, Some(-152), "the landed margin");
    assert_eq!(
        charged.byte_saving_percent,
        Some(27),
        "charging the CLI's own frames flips the sign of the byte margin"
    );
    assert!(
        !charged.clears.1,
        "and it still does not reach the ratified +30% floor"
    );
    assert_eq!(
        per_solved(&charged, Arm::Shell) - per_solved(&landed, Arm::Shell),
        10_162,
        "what the handicap is worth, per solved task"
    );
    // The one thing the handicap cannot touch: it is a cost, not a capability.
    assert_eq!(charged.success_points_permille, 0);
    assert_eq!(charged.invalid_reduction_percent, Some(0));
}

// --- S2: the handshake a process performs ------------------------------------------------

/// **S2 — nor for the handshake each CLI process would perform.**
///
/// The typed arm pays one handshake per *run*, because it holds one connection. A
/// `continuum` binary is one process per command and RFC 0026 gives the protocol no session
/// state to reuse, so every invocation negotiates again. The landed rule charges the
/// baseline zero of them — `ShellSurface::handshake_bytes` returns 0 and says why.
///
/// **LANDED, and this one flips a ratified margin.** 172 invocations at 875 bytes each is
/// 150,500 bytes the baseline did not pay; charging them takes the byte margin to +49%,
/// which *clears* the +30% floor.
#[test]
fn s2_the_baseline_is_not_charged_a_handshake_per_process_invocation() {
    let campaign = Campaign::run();
    let landed = campaign.under(variants::LANDED);
    let charged = campaign.under(variants::CLI_FRAMES_AND_HANDSHAKES);

    assert_eq!(campaign.handshake, 875, "hello frame plus welcome frame");
    assert_eq!(
        campaign.hidden_total(|hidden| u64::from(hidden.invocations)),
        172,
        "one process per command line written, re-reads included"
    );
    assert!(!landed.clears.1, "the landed byte margin does not clear");
    assert_eq!(charged.byte_saving_percent, Some(49));
    assert!(
        charged.clears.1,
        "under symmetric connection accounting the byte margin clears"
    );
    // The other two margins are untouched: an accounting rule cannot change what an arm did.
    assert!(!charged.clears.0, "success is still a tie");
    assert!(!charged.clears.2, "invalid-action rate is still a tie");
}

// --- S3: the expansion that never leaves the harness -------------------------------------

/// **S3 — the expansion command is answered from an envelope the harness already holds.**
///
/// Discipline 3 issues `continuum result omissions --request <id>` and the harness renders
/// the answer from the `ResultEnvelope` it is still holding: no process, no frame, no
/// handshake. That is the sharpest form of the "zero-cost freshness" subsidy — the baseline
/// gets a second read of a past answer for the price of the text.
///
/// **LANDED, small.** 28 expansions across the matrix; charging each a real invocation and
/// round trip moves the byte margin from +49% to +55%.
#[test]
fn s3_the_expansion_command_never_leaves_the_harness() {
    let campaign = Campaign::run();
    let without = campaign.under(variants::CLI_FRAMES_AND_HANDSHAKES);
    let with = campaign.under(variants::SYMMETRIC);

    assert_eq!(campaign.hidden_total(|h| u64::from(h.expansions)), 28);
    assert_eq!(
        campaign.hidden_total(|h| h.expansion_bytes),
        2_856,
        "what the agent paid for them"
    );
    assert_eq!(without.byte_saving_percent, Some(49));
    assert_eq!(with.byte_saving_percent, Some(55));
    assert!(with.clears.1);
    // The subsidy is real but it is not what decides the metric: S2 already cleared it.
    assert!(without.clears.1);
}

// --- S4: the adapter is never stale ------------------------------------------------------

/// **S4 — the adapter renders from the very answer the daemon just produced.**
///
/// A real CLI process re-derives state: it reads a file, a cache, or a replica, and can be a
/// beat behind. This adapter cannot be, by construction. The subsidy is invisible in the
/// landed numbers because the landed numbers have no way to express it.
///
/// **LANDED.** One stale rendering, injected at the second command of every run, costs the
/// baseline four of its twenty-four tasks and takes its invalid-action rate from 69‰ to
/// 571‰. Under that variant the typed arm clears *two* of the three ratified margins —
/// success by +167‰ and invalid-action rate by 87% — and still loses bytes.
///
/// The position matters and the campaign reports it rather than picking the worst: a stale
/// read at the third or fourth command costs the baseline nothing at all, because the
/// datum it corrupted is one the policy re-reads anyway. **What this campaign cannot say is
/// how often a real CLI is stale** — the instrument declares the rate to be zero and has no
/// way to measure it. That is stated in the artifact's limits.
#[test]
fn s4_the_adapter_is_never_a_beat_behind_the_daemon() {
    let campaign = Campaign::run();
    let landed = variants::plain("landed", &campaign.native, &campaign.shell_runs());

    let stale = baseline(Renderer::Stale { at: 2 }, Disciplines::ALL);
    let row = variants::plain("stale-at-2", &campaign.native, &stale);
    assert_eq!(row.shell_solved, 20, "four tasks lost to one stale read");
    assert_eq!(landed.shell_solved, 24);
    assert_eq!(row.shell_invalid_permille, 571);
    assert_eq!(landed.shell_invalid_permille, 69);
    assert_eq!(row.success_points_permille, 167);
    assert!(
        row.clears.0,
        "the success margin clears under one stale read"
    );
    assert!(row.clears.2, "so does the invalid-action margin");
    assert!(!row.clears.1, "the byte margin does not");
    assert_eq!(
        stale.iter().filter(|run| run.stuck.is_some()).count(),
        4,
        "and the four failures are honest: the agent gave up rather than reporting a \
         number it could not read"
    );

    // Position-dependence, reported rather than cherry-picked.
    for at in [3, 4] {
        let later = baseline(Renderer::Stale { at }, Disciplines::ALL);
        let row = variants::plain("stale-later", &campaign.native, &later);
        assert_eq!(
            row.shell_solved, 24,
            "a stale read at command {at} costs the baseline nothing: the datum is re-read"
        );
    }
}

// --- S5: the mistake class no shared policy can hold -------------------------------------

/// **S5 — one shared policy, so only mistakes *both* surfaces can express are counted.**
///
/// `crate::policy` states the load-bearing fairness rule: one `Policy` drives both arms, so
/// "the number of mistakes is fixed by construction and only the interface varies". That
/// rule is what makes the comparison paired — and it is also why the instrument cannot see
/// a mistake only one surface admits. A CLI takes its arguments by *name*, and a name can be
/// wrong. The typed client takes them by struct field, and there is no call through which a
/// caller can offer a different one: the mistake is not made, it is unrepresentable.
///
/// **LANDED.** Every command line in the matrix carries misnameable flags, and one misnamed
/// argument costs the baseline an attempt, an invalid operation and ~265 bytes, against zero
/// on the typed arm. The break-even is **0.6 shell-only invalid attempts per run** — six
/// misnamed arguments in ten runs — at which the ratified 50% relative reduction in
/// invalid-action rate clears. The measured tie at 69‰/69‰ is therefore a property of a
/// policy restricted to the mistakes both surfaces can express, not of the two surfaces.
///
/// What the campaign will not do is declare the rate. A scripted policy cannot have one.
#[test]
fn s5_the_shared_policy_can_only_make_mistakes_both_surfaces_can_express() {
    let campaign = Campaign::run();
    let snapshot = WorkspaceHandle::new("ws_0123456789abcdef").expect("a handle");
    let task = TaskHandle::new("task_0123456789abcdef").expect("a handle");
    let continuation = ContinuationHandle::new("cont_0123456789abcdef").expect("a handle");
    let calls = [
        Call::CreateWorkspace { seal: false },
        Call::ForkModule {
            base: snapshot.clone(),
        },
        Call::SealWorkspace {
            snapshot: snapshot.clone(),
        },
        Call::StartVerification {
            snapshot,
            states: 64,
        },
        Call::PollTask { task: task.clone() },
        Call::FetchResult { task: task.clone() },
        Call::Resume {
            continuation,
            states: 64,
        },
        Call::Cancel { task },
    ];

    let mut flags = 0;
    let mut cost = 0;
    for call in calls {
        let step = Step::faithful(call);
        let command = shell::command_line(&SUBSET[0], &step);
        let named = variants::flag_names(&command);
        assert!(
            !named.is_empty(),
            "every command names its arguments: {command}"
        );
        flags += named.len();
        cost += variants::misnamed_argument_cost(&command);
    }
    assert_eq!(
        flags, 25,
        "misnameable argument positions across the surface"
    );
    let unit = cost / 8;
    assert_eq!(unit, 265, "mean cost of one misnamed argument, in bytes");

    // The typed client's count is structurally zero: `register::OPERATIONS` names eight
    // operations, and every one of them is reached through a method whose parameters are
    // typed values. There is no string-keyed argument channel to misname.
    assert_eq!(continuum_mcp::register::OPERATIONS.len(), 8);

    let native_totals = arm_totals(&campaign.native);
    let shell_totals = arm_totals(&campaign.shell_runs());
    assert_eq!(native_totals.invalid_permille(), 69);
    assert_eq!(shell_totals.invalid_permille(), 69);
    assert_eq!(
        variants::invalid_reduction_breakeven_tenths(&native_totals, &shell_totals),
        Some(6),
        "0.6 shell-only invalid attempts per run clears the ratified 50% reduction"
    );

    // And at one per run — one misnamed flag in a seven-command script — it clears wide.
    let perturbed = variants::with_shell_only_mistakes(&campaign.shell_runs(), 1, unit);
    let row = variants::plain(
        "shell-only-mistakes-1-per-run",
        &campaign.native,
        &perturbed,
    );
    assert!(
        row.clears.2,
        "at one misnamed argument per run the invalid-action margin clears: {}",
        row.render()
    );
    assert!(
        !row.clears.1,
        "and the byte margin still does not: the class is cheap in bytes"
    );
}

fn arm_totals(runs: &[ArmRun]) -> ArmTotals {
    let mut totals = ArmTotals::default();
    for run in runs {
        totals.runs += 1;
        totals.attempted += run.attempted;
        totals.admitted += run.admitted;
        totals.invalid += run.invalid;
        totals.zero_cost_invalid += run.zero_cost_invalid;
        totals.bytes_total += run.bytes;
        if run.solved {
            totals.solved += 1;
            totals.bytes_on_solved += run.bytes;
        }
    }
    totals
}

// --- S6: envelope separability -------------------------------------------------------------

/// **S6 — the envelope's bytes are treated as one undifferentiated cost.**
///
/// A single ratio says which arm is cheaper; a redesign needs to know which bytes it could
/// have back. Every answer the matrix produced is decomposed by re-encoding it with each
/// field reduced to the smallest value its declaration admits.
///
/// **LANDED on the explanation.** bn-23gm's landing comment names the cause as "six epochs,
/// nine cost dimensions, an assurance envelope, an omission manifest and a `next_operations`
/// list on *every* answer". Two of those five cost **zero bytes**: this daemon populates no
/// `next_operations` (the same producer gap `Error.recovery` has, recorded by bn-1udt), and
/// the envelope-level `Cost` is empty because the spend an agent reads travels in
/// `TaskRecord.cost` inside the payload. The measured overhead is `artifacts` 20,732,
/// `omissions` 16,656, `assurance` 15,720 and `epochs` 8,084 over 172 answers.
///
/// **HELD on the conclusion, and this is the redesign-actionable number.** Of the four,
/// only `epochs` is both required and *constant across a connection*, so it is the only
/// conformance cost a redesign could amortize without changing a guarantee: with `epochs`
/// pinned at the handshake and `SnapshotComponents` resolved by reference, the typed arm
/// spends 9,355 bytes per solved task against the baseline's 4,118 — a -138% margin. Even
/// reducing the whole envelope to its floor — every list empty, every optional absent, the
/// six epochs gone — leaves 7,193 against 4,118, a -74% margin. **The byte loss is not an
/// envelope-overhead artefact. It survives a maximal redesign of the encoding.**
#[test]
fn s6_where_the_typed_arms_bytes_go_and_how_far_a_redesign_reaches() {
    let campaign = Campaign::run();
    let decomposition = &campaign.decomposition;
    let field = |name: &str| {
        decomposition
            .fields
            .iter()
            .find(|cost| cost.field == name)
            .map(|cost| cost.bytes)
            .expect("a decomposed field")
    };

    assert_eq!(decomposition.answers, 172);
    assert_eq!(decomposition.result_bytes, 170_584);
    assert_eq!(decomposition.request_bytes, 73_318);

    // The correction: two of the five named causes cost nothing.
    assert_eq!(
        field("next_operations"),
        0,
        "this daemon populates no next_operations, so they cost no bytes"
    );
    assert_eq!(
        field("cost"),
        0,
        "the envelope-level Cost is empty; the spend an agent reads is in the payload"
    );
    assert_eq!(field("artifacts"), 20_732);
    assert_eq!(field("omissions"), 16_656);
    assert_eq!(field("assurance"), 15_720);
    assert_eq!(field("epochs"), 8_084);
    assert_eq!(decomposition.snapshot_component_bytes, 17_208);
    assert_eq!(decomposition.snapshot_component_calls, 24);

    // The two floors, scaled to the typed arm's own wire calls. The decomposition is
    // measured over the baseline's 172 dispatches; the typed arm makes 168, because its
    // register refuses four calls before the wire.
    let native = arm_totals(&campaign.native);
    let wire_calls = u64::from(native.attempted - native.zero_cost_invalid);
    assert_eq!(wire_calls, 168);
    let per_answer = |total: u64| total * wire_calls / u64::from(decomposition.answers);

    let redesign = native
        .bytes_on_solved
        .saturating_sub(per_answer(decomposition.redesign_reducible()));
    let maximal = native
        .bytes_on_solved
        .saturating_sub(per_answer(decomposition.maximal_reducible()));
    let shell = arm_totals(&campaign.shell_runs())
        .bytes_per_solved()
        .expect("the baseline solved");

    let margin = |bytes: u64| ((shell as i64 - (bytes / 24) as i64) * 100) / shell as i64;
    assert_eq!(
        redesign / 24,
        9_355,
        "epochs pinned, components by reference — and `components` is now a *landed* \
         reduction rather than a projected one (bn-3of5h), which is why this row moved \
         with the arm it decomposes"
    );
    assert_eq!(margin(redesign), -127);
    assert_eq!(
        maximal / 24,
        7_193,
        "the whole envelope reduced to its floor"
    );
    assert_eq!(margin(maximal), -74);
    assert!(
        margin(maximal) < RATIFIED.byte_saving_percent,
        "the byte loss survives a maximal redesign of the encoding"
    );
}

// --- S7: the counting rule ------------------------------------------------------------------

/// **S7 — `ceil(bytes/4)` treats dense canonical JSON and sparse prose alike.**
///
/// research/25 declares no alternative counting rule, and plan §24.5 grades bytes rather
/// than tokens precisely so that no rule has to be chosen. The attack is still worth
/// running, in two parts.
///
/// **HELD.** Any `bytes/k` rule is affine in bytes with the *same* divisor on both arms, so
/// the relative margin is identical for every k — asserted here for 3, 4, 5 and 8, all
/// −152%. The declared rule cannot be hiding anything. And the one rule tried that is *not*
/// affine — words and punctuation counted separately, under which canonical JSON costs 309
/// tokens per thousand bytes against the projection's 145 — moves the margin from −152% to
/// **-437%**, further against the typed arm. No counting rule available to this campaign
/// improves the typed arm's position.
#[test]
fn s7_the_declared_token_rule_cannot_move_the_margin_and_the_one_that_could_moves_it_the_other_way()
{
    let campaign = Campaign::run();
    let native = arm_totals(&campaign.native)
        .bytes_per_solved()
        .expect("solved");
    let shell = arm_totals(&campaign.shell_runs())
        .bytes_per_solved()
        .expect("solved");

    assert_eq!(BYTES_PER_APPROX_TOKEN, 4);
    for divisor in [3_u64, 4, 5, 8] {
        let margin = ((shell.div_ceil(divisor) as i64 - native.div_ceil(divisor) as i64) * 100)
            / shell.div_ceil(divisor) as i64;
        assert_eq!(
            margin, -152,
            "every affine rule gives the same margin; bytes/{divisor} included"
        );
    }

    let frames = campaign.hidden_total(|hidden| hidden.frame_bytes());
    let frame_tokens = campaign.hidden_total(|hidden| hidden.frame_tokens);
    let agent_tokens = campaign.hidden_total(|hidden| hidden.agent_tokens);
    let agent_bytes: u64 = campaign.shell.iter().map(|cell| cell.run.bytes).sum();
    let json_density = frame_tokens * 1000 / frames;
    let prose_density = agent_tokens * 1000 / agent_bytes;
    assert_eq!(
        json_density, 309,
        "structural tokens per 1000 bytes of wire"
    );
    assert_eq!(prose_density, 145, "and per 1000 bytes of projection");

    let native_tokens = native * json_density / 1000;
    let shell_tokens = shell * prose_density / 1000;
    let margin = ((shell_tokens as i64 - native_tokens as i64) * 100) / shell_tokens as i64;
    assert_eq!(margin, -437);
    assert!(
        margin < -152,
        "the only non-affine rule tried moves the margin further against the typed arm"
    );
}

// --- N1: the free local refusal ---------------------------------------------------------------

/// **N1 — a call the register refuses before the wire costs zero.**
///
/// `crate::run`'s IMPL-01 section is careful about this: a locally refused call counts as an
/// attempt and as an invalid one, and the saving "is bytes, not validity". The attack is
/// whether even the byte saving is real: an agent that decided to make a call *composed* it,
/// and the tokens it spent composing it are gone whether or not a frame was written.
///
/// **HELD.** Priced at the request frame the client would have written — 411 bytes,
/// measured on the frame the operation actually produces — the four locally refused calls
/// are worth 69 bytes per solved task and move the margin from −152% to −153%. The claim in
/// bn-134i's landing comment is real, correctly scoped, and worth 0.6% of the typed arm's
/// cost. Nothing an adjudicator would rely on turns on it.
#[test]
fn n1_a_call_refused_before_the_wire_is_free_on_the_wire_and_not_free_to_compose() {
    let campaign = Campaign::run();
    let landed = campaign.under(variants::LANDED);
    let priced = campaign.under(variants::COMPOSED_REFUSALS_PRICED);

    assert_eq!(campaign.composition, 411, "the frame it would have written");
    assert_eq!(arm_totals(&campaign.native).zero_cost_invalid, 4);
    assert_eq!(landed.byte_saving_percent, Some(-152));
    assert_eq!(priced.byte_saving_percent, Some(-153));
    assert_eq!(
        per_solved(&priced, Arm::Native) - per_solved(&landed, Arm::Native),
        69,
        "what the free refusal is worth, per solved task"
    );
    // The rate is unchanged, which is the anti-cheating rule holding: a locally refused
    // call was already counted as invalid.
    assert_eq!(priced.native_invalid_permille, 69);
    assert_eq!(landed.native_invalid_permille, 69);
}

// --- N2: the fault class only the text arm can suffer -------------------------------------------

/// **N2 — every fault the instrument injects lands on both arms.**
///
/// bn-1udt's four faults are all injected at the daemon's own error surfaces, which is what
/// the bullet asked for and is symmetric by construction. There is a fault class that is not
/// symmetric and that the instrument never injects: the projection's own format moving. A
/// CLI's output is its own; a typed field name is the schema's, and a schema change is a
/// protocol version.
///
/// **LANDED.** Renaming exactly one projected key — `cost.states`, `verdict`, or `task` —
/// takes the baseline from 24/24 to **0/24** while the typed arm is untouched, and the
/// success margin goes from a tie to +1000‰. The disciplined baseline fails *honestly*: on
/// the `task` rename all 24 runs stop at `Stuck::UnreadableAnswer` rather than reporting a
/// number they could not read, which is discipline 1 and 2 doing exactly their job. The
/// undisciplined one fails dishonestly, at 923‰ invalid operations.
///
/// This is the mirror of S5 and it points the same way: the landed tie on completion and on
/// recovery is conditional on a projection that never drifts, and the instrument declares
/// that condition rather than measuring it.
#[test]
fn n2_a_renamed_projection_key_is_a_fault_class_only_the_text_arm_can_suffer() {
    let campaign = Campaign::run();
    for (from, to) in [
        ("cost.states", "cost.state_count"),
        ("verdict", "result.verdict"),
        ("task", "task.handle"),
    ] {
        let drifted = baseline(Renderer::Drifting { from, to }, Disciplines::ALL);
        let row = variants::plain("drift", &campaign.native, &drifted);
        assert_eq!(
            row.shell_solved, 0,
            "one renamed key takes the baseline to zero: {from} -> {to}"
        );
        assert_eq!(row.native_solved, 24, "the typed arm has no analogue");
        assert_eq!(row.success_points_permille, 1000);
        assert!(row.clears.0);
    }

    // The disciplined failure is an honest one.
    let drifted = baseline(
        Renderer::Drifting {
            from: "task",
            to: "task.handle",
        },
        Disciplines::ALL,
    );
    assert_eq!(
        drifted.iter().filter(|run| run.stuck.is_some()).count(),
        24,
        "the disciplined baseline stops rather than guessing"
    );
    let undisciplined = baseline(
        Renderer::Drifting {
            from: "task",
            to: "task.handle",
        },
        Disciplines::NONE,
    );
    let row = variants::plain("drift-undisciplined", &campaign.native, &undisciplined);
    assert_eq!(row.shell_invalid_permille, 923);
    assert!(
        row.clears.2,
        "against an undisciplined scraper reading a drifted projection, the ratified \
         invalid-action margin clears"
    );
}

// --- N3: the disciplines --------------------------------------------------------------------------

/// **N3 — the baseline is *disciplined*, and the disciplines are the harness's own.**
///
/// The goal bone names a "disciplined-shell baseline"; RFC 0027's ablation ladder names its
/// first rung a **raw shell agent**. The instrument implements the first. This attack asks
/// how much of the baseline's result the disciplines are carrying, one discipline at a time.
///
/// **LANDED.** Discipline 1 carries all of it: without anchored key lookup the baseline
/// solves **0/24** at 923‰ invalid operations, because a lookup for `task` finds the
/// `task.status:` line. Disciplines 2 and 4 are **inert on this instrument** — dropping
/// either changes not one byte of the artifact — because the standard projection never emits
/// an unknown token and never omits a required key; they are load-bearing only under the
/// injected renderers (`Renderer::Flaky` for 4, `Renderer::Drifting` for 2), which is what
/// `crate::shell` already says about discipline 4 and does not yet say about 2. Discipline 3
/// is a pure cost of 119 bytes per solved task.
///
/// The number this gives an adjudicator is RFC 0027's missing rung: against a *raw* shell
/// agent the typed arm wins success by +1000‰ and invalid-action rate by 92%, and still
/// cannot be compared on bytes because the raw arm solves nothing to divide by.
#[test]
fn n3_the_baselines_whole_result_rests_on_one_of_its_four_disciplines() {
    let campaign = Campaign::run();
    let landed = variants::plain("landed", &campaign.native, &campaign.shell_runs());

    let without = |disciplines| {
        variants::plain(
            "d",
            &campaign.native,
            &baseline(Renderer::Standard, disciplines),
        )
    };

    let no_anchor = without(Disciplines {
        anchored_lookup: false,
        ..Disciplines::ALL
    });
    assert_eq!(no_anchor.shell_solved, 0);
    assert_eq!(no_anchor.shell_invalid_permille, 923);
    assert_eq!(no_anchor.byte_saving_percent, None, "nothing to divide by");

    for inert in [
        Disciplines {
            closed_vocabulary: false,
            ..Disciplines::ALL
        },
        Disciplines {
            bounded_reread: false,
            ..Disciplines::ALL
        },
    ] {
        let row = without(inert);
        assert_eq!(
            (
                row.shell_solved,
                row.shell_bytes_per_solved,
                row.shell_invalid_permille
            ),
            (
                landed.shell_solved,
                landed.shell_bytes_per_solved,
                landed.shell_invalid_permille
            ),
            "inert on the standard projection: it never emits an unknown token and never \
             omits a required key"
        );
    }

    let no_expansion = without(Disciplines {
        explicit_expansion: false,
        ..Disciplines::ALL
    });
    assert_eq!(no_expansion.shell_solved, 24);
    assert_eq!(
        per_solved(&landed, Arm::Shell) - per_solved(&no_expansion, Arm::Shell),
        119,
        "discipline 3 is a pure cost on this subset"
    );

    // RFC 0027's first rung, which the instrument does not run.
    let raw = variants::plain(
        "raw-shell",
        &campaign.native,
        &baseline(Renderer::Standard, Disciplines::NONE),
    );
    assert_eq!(raw.shell_solved, 0);
    assert_eq!(raw.success_points_permille, 1000);
    assert_eq!(raw.invalid_reduction_percent, Some(92));
    assert!(raw.clears.0 && raw.clears.2 && !raw.clears.1);
}

// --- N4: the task set -----------------------------------------------------------------------------

/// **N4 — four tasks, two semantic families, and every conclusion drawn over all four.**
///
/// **HELD.** Every leave-one-out subset the corpus admits reproduces every landed
/// conclusion: the byte margin stays at −152% or −151%, the success margin stays at zero,
/// the invalid-action reduction stays at zero, and the §19.4 separation check still passes
/// on every three-task subset — so each one is a subset a result could legitimately be
/// reported over. Nothing in the landed result depends on which task is in it.
///
/// What this does *not* establish is generalization beyond two families, which is DX-15's at
/// G9 and is already named in the landed artifact's limits.
#[test]
fn n4_every_conclusion_survives_dropping_any_one_task() {
    let campaign = Campaign::run();
    let shell = campaign.shell_runs();
    for (dropped, subset) in variants::leave_one_out() {
        assert!(
            separation::check(&subset).passes(),
            "the {dropped}-free subset is still a reportable one"
        );
        let row = variants::plain(
            format!("without-{dropped}"),
            &variants::restrict(&campaign.native, &subset),
            &variants::restrict(&shell, &subset),
        );
        assert_eq!(row.native_solved, 18);
        assert_eq!(row.shell_solved, 18);
        assert_eq!(row.success_points_permille, 0);
        assert_eq!(row.invalid_reduction_percent, Some(0));
        assert!(
            matches!(row.byte_saving_percent, Some(-153..=-151)),
            "the byte margin is stable without {dropped}: {}",
            row.render()
        );
        assert!(!row.clears_all());
    }
}

// --- N5: the completion criterion ------------------------------------------------------------------

/// **N5 — completion requires the one datum only the typed arm gets for free.**
///
/// `AgentView::complete` requires `ceiling_enforced` — read off the INV-007 omission
/// manifest, which the typed answer carries inline and the projection summarizes. It is the
/// instrument's single deliberate concession to the typed arm, and `crate::shell` names it
/// as "the one lossy rule". The attack: is the completion tie an artefact of a criterion
/// built around the typed arm's strength?
///
/// **HELD, and it points the other way.** Dropping the clause entirely — grading a run
/// solved when it read the frozen state count and the frozen verdict, whatever it knew about
/// metering — leaves **both** arms at 24/24. The criterion changes no completion outcome.
/// Its only effect is the 119 bytes per solved task the baseline pays for the expansion, and
/// removing that makes the typed arm's byte loss *worse*, from −152% to −159%. The
/// concession is real, is small, and is the only one; it is not holding the result up.
#[test]
fn n5_the_completion_criterion_contains_the_datum_only_one_arm_gets_free() {
    let campaign = Campaign::run();
    let ceiling_free = |runs: &[ArmRun]| {
        runs.iter()
            .filter(|run| {
                let task = SUBSET
                    .iter()
                    .find(|task| task.id == run.task)
                    .expect("a subset task");
                variants::ceiling_free_completion(run, task)
            })
            .count()
    };
    assert_eq!(ceiling_free(&campaign.native), 24);
    assert_eq!(ceiling_free(&campaign.shell_runs()), 24);

    let landed = variants::plain("landed", &campaign.native, &campaign.shell_runs());
    let without = variants::plain(
        "no-expansion",
        &campaign.native,
        &baseline(
            Renderer::Standard,
            Disciplines {
                explicit_expansion: false,
                ..Disciplines::ALL
            },
        ),
    );
    assert_eq!(landed.byte_saving_percent, Some(-152));
    assert_eq!(
        without.byte_saving_percent,
        Some(-159),
        "removing the concession widens the typed arm's loss"
    );
}

// --- controls ------------------------------------------------------------------------------------

/// **Control — nothing this campaign added moved a landed number.**
///
/// The house rule of a falsification campaign is that the system under test is frozen. Here
/// the instrument *is* the system under test, so the rule is narrowed rather than dropped:
/// variants are added beside the landed metrics and the landed metrics are asserted
/// unchanged, headline for headline, against bn-134i's landing comment.
#[test]
fn control_every_landed_number_is_unchanged_by_this_campaign() {
    let report = Report::new(support::both_arms());
    let native = *report.totals.get(&Arm::Native).expect("the typed arm");
    let shell = *report.totals.get(&Arm::Shell).expect("the baseline");

    assert!(report.accepted, "the §19.4 gate still passes");
    assert_eq!(native.bytes_per_solved(), Some(10_384));
    assert_eq!(shell.bytes_per_solved(), Some(4_118));
    assert_eq!(report.margins.byte_saving_percent, Some(-152));
    assert_eq!(native.invalid_permille(), 69);
    assert_eq!(shell.invalid_permille(), 69);
    assert_eq!(report.margins.invalid_reduction_percent, Some(0));
    assert_eq!((native.solved, native.runs), (24, 24));
    assert_eq!((shell.solved, shell.runs), (24, 24));
    assert_eq!(report.margins.success_points_permille, 0);
    assert_eq!(native.recovery_permille(), 1000);
    assert_eq!(shell.recovery_permille(), 1000);
    assert!(
        !report.margins.success_clears
            && !report.margins.bytes_clear
            && !report.margins.invalid_clears,
        "none of the three ratified margins clears under the landed rule"
    );
}

/// **Control — the instrumentation is beside the measurement, not inside it.**
///
/// [`shell::HiddenCost`] exists so this campaign can price what the baseline was not charged
/// for. If any of it leaked into an interface-byte total, every variant would be measuring
/// the leak. The landed baseline's own byte total is recomputed here from the run's
/// transcript-independent parts and compared.
#[test]
fn control_the_hidden_ledger_never_enters_an_interface_byte_total() {
    let metered = variants::metered_shell_sweep().expect("the baseline sweeps");
    let plain = continuum_benchmark::run::sweep_shell().expect("the baseline sweeps");
    assert_eq!(metered.len(), plain.len());
    for (cell, reference) in metered.iter().zip(&plain) {
        assert_eq!(
            cell.run, *reference,
            "a metered run is the landed run, byte for byte"
        );
        assert!(
            cell.hidden.frame_bytes() > 0,
            "and the hidden ledger is not empty, so the equality is not vacuous"
        );
        assert!(cell.hidden.frame_bytes() > cell.run.bytes);
    }
}

/// **Control — the decomposition measures real frames.**
///
/// A decomposition computed by re-encoding a decoded envelope is only a measurement of the
/// wire if the re-encoding is the wire. Every recorded answer is re-encoded and compared to
/// the frame length the CLI process actually read.
#[test]
fn control_a_decomposed_answer_re_encodes_to_the_frame_that_carried_it() {
    let cells = variants::shell_sweep(Renderer::Standard, Disciplines::ALL, true)
        .expect("the baseline sweeps");
    let mut checked = 0;
    for cell in &cells {
        for recorded in &cell.recorded {
            let bytes = continuumd::codec::to_bytes(&recorded.envelope).expect("it re-encodes");
            assert_eq!(
                bytes.len() as u64,
                recorded.received,
                "the re-encoded answer is the frame that carried it ({})",
                recorded.operation
            );
            checked += 1;
        }
    }
    assert_eq!(checked, 172, "every answer in the matrix");
}

// --- the artifact ------------------------------------------------------------------------------------

/// Build the whole campaign artifact from measured numbers.
fn campaign_report() -> FalsificationReport {
    let campaign = Campaign::run();
    let shell_runs = campaign.shell_runs();
    let landed = campaign.under(variants::LANDED);

    let mut sensitivity: Vec<Sensitivity> = variants::ACCOUNTING_RULES
        .iter()
        .map(|rule| campaign.under(*rule))
        .collect();
    sensitivity.push(variants::plain(
        "stale-read-at-command-2",
        &campaign.native,
        &baseline(Renderer::Stale { at: 2 }, Disciplines::ALL),
    ));
    sensitivity.push(variants::plain(
        "projection-key-renamed",
        &campaign.native,
        &baseline(
            Renderer::Drifting {
                from: "cost.states",
                to: "cost.state_count",
            },
            Disciplines::ALL,
        ),
    ));
    sensitivity.push(variants::plain(
        "raw-shell-baseline",
        &campaign.native,
        &baseline(Renderer::Standard, Disciplines::NONE),
    ));
    sensitivity.push(variants::plain(
        "baseline-without-expansion",
        &campaign.native,
        &baseline(
            Renderer::Standard,
            Disciplines {
                explicit_expansion: false,
                ..Disciplines::ALL
            },
        ),
    ));
    sensitivity.push(variants::plain(
        "shell-only-mistakes-1-per-run",
        &campaign.native,
        &variants::with_shell_only_mistakes(&shell_runs, 1, 265),
    ));
    for (dropped, subset) in variants::leave_one_out() {
        sensitivity.push(variants::plain(
            format!("without-{dropped}"),
            &variants::restrict(&campaign.native, &subset),
            &variants::restrict(&shell_runs, &subset),
        ));
    }

    let native = arm_totals(&campaign.native);
    let shell = arm_totals(&shell_runs);
    let breakeven = variants::invalid_reduction_breakeven_tenths(&native, &shell)
        .map_or_else(|| "none".to_owned(), |tenths| format!("{tenths}/10"));
    let wire_calls = u64::from(native.attempted - native.zero_cost_invalid);
    let per_answer = |total: u64| total * wire_calls / u64::from(campaign.decomposition.answers);
    let maximal = native
        .bytes_on_solved
        .saturating_sub(per_answer(campaign.decomposition.maximal_reducible()))
        / 24;

    let attacks = vec![
        Attack {
            id: "S1",
            direction: Direction::ProShell,
            target: "the baseline is not charged for its CLI's own protocol frames",
            outcome: Outcome::Landed,
            evidence: format!(
                "hidden frames {} bytes over {} invocations; shell bytes/solved {} -> {}; \
                 byte margin {}% -> {}%; still short of the +{}% floor",
                campaign.hidden_total(|hidden| hidden.frame_bytes()),
                campaign.hidden_total(|hidden| u64::from(hidden.invocations)),
                per_solved(&landed, Arm::Shell),
                per_solved(&campaign.under(variants::CLI_FRAMES), Arm::Shell),
                landed.byte_saving_percent.unwrap_or_default(),
                campaign
                    .under(variants::CLI_FRAMES)
                    .byte_saving_percent
                    .unwrap_or_default(),
                RATIFIED.byte_saving_percent,
            ),
        },
        Attack {
            id: "S2",
            direction: Direction::ProShell,
            target: "nor a connection handshake per CLI process invocation",
            outcome: Outcome::Landed,
            evidence: format!(
                "{} bytes per handshake x {} invocations; byte margin -> {}%, which CLEARS \
                 the +{}% floor",
                campaign.handshake,
                campaign.hidden_total(|hidden| u64::from(hidden.invocations)),
                campaign
                    .under(variants::CLI_FRAMES_AND_HANDSHAKES)
                    .byte_saving_percent
                    .unwrap_or_default(),
                RATIFIED.byte_saving_percent,
            ),
        },
        Attack {
            id: "S3",
            direction: Direction::ProShell,
            target: "the expansion command is served from an envelope the harness holds",
            outcome: Outcome::Landed,
            evidence: format!(
                "{} expansions, {} agent bytes, no process and no round trip; byte margin \
                 -> {}%",
                campaign.hidden_total(|hidden| u64::from(hidden.expansions)),
                campaign.hidden_total(|hidden| hidden.expansion_bytes),
                campaign
                    .under(variants::SYMMETRIC)
                    .byte_saving_percent
                    .unwrap_or_default(),
            ),
        },
        Attack {
            id: "S4",
            direction: Direction::ProShell,
            target: "the adapter renders from the answer the daemon just produced",
            outcome: Outcome::Landed,
            evidence: {
                let stale = variants::plain(
                    "s",
                    &campaign.native,
                    &baseline(Renderer::Stale { at: 2 }, Disciplines::ALL),
                );
                format!(
                    "one stale rendering: shell solved {}/24, invalid {}permille (from {}); \
                     success and invalid-action margins both CLEAR; rate unmeasurable here",
                    stale.shell_solved, stale.shell_invalid_permille, landed.shell_invalid_permille,
                )
            },
        },
        Attack {
            id: "S5",
            direction: Direction::ProShell,
            target: "one shared policy counts only mistakes both surfaces can express",
            outcome: Outcome::Landed,
            evidence: format!(
                "25 misnameable argument positions on the text surface, 0 on the typed one; \
                 break-even {breakeven} shell-only invalid attempts per run clears the \
                 ratified {}% reduction",
                RATIFIED.invalid_reduction_percent,
            ),
        },
        Attack {
            id: "S6",
            direction: Direction::ProShell,
            target: "the envelope's bytes are one undifferentiated cost",
            outcome: Outcome::Landed,
            evidence: format!(
                "next_operations and envelope Cost cost 0 bytes, correcting the landed \
                 explanation; measured overhead artifacts {} omissions {} assurance {} \
                 epochs {}; a MAXIMAL envelope redesign leaves {} vs {} bytes/solved, still \
                 a loss",
                campaign
                    .decomposition
                    .fields
                    .iter()
                    .find(|f| f.field == "artifacts")
                    .map_or(0, |f| f.bytes),
                campaign
                    .decomposition
                    .fields
                    .iter()
                    .find(|f| f.field == "omissions")
                    .map_or(0, |f| f.bytes),
                campaign
                    .decomposition
                    .fields
                    .iter()
                    .find(|f| f.field == "assurance")
                    .map_or(0, |f| f.bytes),
                campaign
                    .decomposition
                    .fields
                    .iter()
                    .find(|f| f.field == "epochs")
                    .map_or(0, |f| f.bytes),
                maximal,
                shell.bytes_per_solved().unwrap_or_default(),
            ),
        },
        Attack {
            id: "S7",
            direction: Direction::ProShell,
            target: "ceil(bytes/4) treats dense JSON and prose alike",
            outcome: Outcome::Held,
            evidence: "research/25 declares no alternative rule; every bytes/k rule is \
                       affine and gives -152% for k in {3,4,5,8}; the one non-affine rule \
                       tried gives -437%, further against the typed arm"
                .to_owned(),
        },
        Attack {
            id: "N1",
            direction: Direction::ProNative,
            target: "a call the register refuses before the wire costs zero",
            outcome: Outcome::Held,
            evidence: format!(
                "4 refusals priced at the {}-byte frame they would have written = {} bytes \
                 per solved task; byte margin {}% -> {}%",
                campaign.composition,
                per_solved(
                    &campaign.under(variants::COMPOSED_REFUSALS_PRICED),
                    Arm::Native
                ) - per_solved(&landed, Arm::Native),
                landed.byte_saving_percent.unwrap_or_default(),
                campaign
                    .under(variants::COMPOSED_REFUSALS_PRICED)
                    .byte_saving_percent
                    .unwrap_or_default(),
            ),
        },
        Attack {
            id: "N2",
            direction: Direction::ProNative,
            target: "every injected fault lands on both arms",
            outcome: Outcome::Landed,
            evidence: "one renamed projection key takes the baseline from 24/24 to 0/24 \
                       with the typed arm untouched; success margin 0 -> +1000permille; the \
                       disciplined baseline fails honestly (24 stuck), the undisciplined one \
                       at 923permille invalid"
                .to_owned(),
        },
        Attack {
            id: "N3",
            direction: Direction::ProNative,
            target: "the baseline is disciplined, and the disciplines are the harness's",
            outcome: Outcome::Landed,
            evidence: "discipline 1 carries the whole result (0/24 and 923permille invalid \
                       without it); disciplines 2 and 4 are inert on the standard \
                       projection; discipline 3 is a pure 119 bytes/solved task cost"
                .to_owned(),
        },
        Attack {
            id: "N4",
            direction: Direction::ProNative,
            target: "four tasks over two semantic families",
            outcome: Outcome::Held,
            evidence: "every leave-one-out subset still passes the plan 19.4 check and \
                       reproduces every conclusion: byte margin -152/-151%, success 0, \
                       invalid reduction 0"
                .to_owned(),
        },
        Attack {
            id: "N5",
            direction: Direction::ProNative,
            target: "completion requires the datum only the typed arm receives inline",
            outcome: Outcome::Held,
            evidence: "dropping the clause leaves both arms at 24/24; its only effect is \
                       119 bytes/solved task on the baseline, and removing it widens the \
                       typed arm's loss to -159%"
                .to_owned(),
        },
        Attack {
            id: "X1",
            direction: Direction::ProShell,
            target: "whose interface 'interface bytes per solved task' names",
            outcome: Outcome::Inconclusive("the governing text does not decide it".to_owned()),
            evidence: "RFC 0027 fixes the metric and the denominator and never says whose \
                       interface; under the agent's stdin/stdout the margin is -152%, under \
                       total protocol traffic it is +55%. Both readings are defensible from \
                       the text as written."
                .to_owned(),
        },
    ];

    let surviving = vec![
        format!(
            "1. SUCCESS is a TIE and the tie is robust. 24/24 on both arms under every \
             accounting variant, every leave-one-out subset, and both policies. The +{}pp \
             floor is missed under every rule this campaign could defend. It is met only \
             where the baseline is degraded: a raw (undisciplined) scraper, a drifted \
             projection, or a stale read.",
            RATIFIED.success_points
        ),
        format!(
            "2. INVALID-ACTION RATE is a TIE at 69permille/69permille and the tie is an \
             artefact of the policy, not of the surfaces. It is stable under every \
             accounting variant and every task subset, but the shared policy can only make \
             mistakes both surfaces can express; {breakeven} shell-only invalid attempts per \
             run clears the ratified {}% reduction. The tie is a lower bound on the typed \
             arm's advantage, not a measurement of it.",
            RATIFIED.invalid_reduction_percent
        ),
        format!(
            "3. BYTES: the sign of the margin is decided by an accounting choice the \
             governing text leaves open. Agent-interface bytes: -152%. CLI frames charged: \
             +27%. Frames and per-invocation handshakes charged: +49%. Fully symmetric: \
             +55%. The +{}% floor is missed under the landed rule and cleared under \
             symmetric connection accounting.",
            RATIFIED.byte_saving_percent
        ),
        "4. BYTES, the part no accounting choice moves: under the landed rule the loss is \
         NOT an envelope-overhead artefact. Reducing the whole result envelope to its floor \
         and resolving SnapshotComponents by reference still leaves the typed arm at 7,193 \
         bytes per solved task against 4,118 - a -74% margin. A redesign of the ENCODING \
         cannot close it."
            .to_owned(),
        "5. RECOVERY parity (1000permille both arms) holds under every symmetric fault, and \
         is conditional on a projection that never drifts and is never stale. One renamed \
         key or one stale read breaks the baseline and not the typed arm; the instrument \
         declares both rates to be zero and cannot measure either."
            .to_owned(),
        "6. The instrument's own concessions are small and measured: the free local refusal \
         is worth 69 bytes/solved task to the typed arm, and the omission-manifest clause in \
         the completion criterion is worth 119 bytes/solved task to the baseline. Neither \
         changes any verdict."
            .to_owned(),
        "7. What no variant here can supply: a live model. Every conclusion above is about \
         what one scripted behaviour costs through two interfaces. research/25's own \
         mechanism for the invalid-action margin - validating a plan against the register \
         before acting - is unexercised, and this campaign did not build it."
            .to_owned(),
    ];

    FalsificationReport {
        attacks,
        sensitivity,
        decomposition: campaign.decomposition,
        surviving,
    }
}

/// **IMPL-05's device, applied to this artifact.** Two whole campaigns, each driving its own
/// fresh rigs from scratch, render byte-identically; and the comparison is shown able to fail.
#[test]
fn the_falsification_report_renders_byte_identically_from_two_independent_campaigns() {
    let first = campaign_report();
    let second = campaign_report();
    assert_eq!(
        first.render(),
        second.render(),
        "two independent campaigns render one artifact"
    );

    let mut perturbed = second;
    perturbed.attacks[0].outcome = Outcome::Held;
    assert_ne!(
        first.render(),
        perturbed.render(),
        "and the comparison can tell two artifacts apart"
    );

    let rendered = first.render();
    assert!(rendered.is_ascii(), "no locale reaches the artifact");
    assert!(!rendered.contains("2026"), "no clock reaches the artifact");
    for marker in [
        "[attacks]",
        "[sensitivity]",
        "[decomposition]",
        "[surviving]",
        "[limits]",
    ] {
        assert!(rendered.contains(marker), "the artifact carries {marker}");
    }
}

/// **The artifact's shape, held to the deliverable.** Twelve attacks with a verdict each, one
/// sensitivity row per variant, and a surviving-conclusions statement that names which of
/// the row's three pass conditions the surviving numbers meet.
#[test]
fn the_falsification_artifact_carries_a_verdict_for_every_attack_and_a_row_for_every_variant() {
    let report = campaign_report();

    let mut by_direction: BTreeMap<&'static str, usize> = BTreeMap::new();
    for attack in &report.attacks {
        *by_direction.entry(attack.direction.token()).or_default() += 1;
    }
    assert_eq!(by_direction.get("pro-shell"), Some(&8));
    assert_eq!(by_direction.get("pro-native"), Some(&5));
    assert_eq!(report.attacks.len(), 13);

    let landed = report
        .attacks
        .iter()
        .filter(|attack| attack.outcome == Outcome::Landed)
        .count();
    let held = report
        .attacks
        .iter()
        .filter(|attack| attack.outcome == Outcome::Held)
        .count();
    let inconclusive = report
        .attacks
        .iter()
        .filter(|attack| matches!(attack.outcome, Outcome::Inconclusive(_)))
        .count();
    assert_eq!((landed, held, inconclusive), (8, 4, 1));

    // The first sensitivity row is the landed artifact, unchanged, so a reader who quotes
    // the table cannot quote a variant thinking it is the measurement.
    assert_eq!(report.sensitivity[0].rule, "landed");
    assert_eq!(report.sensitivity[0].native_bytes_per_solved, Some(10_384));
    assert_eq!(report.sensitivity[0].shell_bytes_per_solved, Some(4_118));
    assert_eq!(report.sensitivity[0].byte_saving_percent, Some(-152));
    assert!(!report.sensitivity[0].clears_all());
    assert_eq!(report.sensitivity.len(), 15);

    // No row of the table clears all three at once, which is the sentence the ratified
    // margin actually asks about.
    assert!(
        !report.sensitivity.iter().any(Sensitivity::clears_all),
        "no accounting or instrument variant makes all three ratified margins clear \
         simultaneously"
    );

    assert_eq!(report.surviving.len(), 7);
    assert!(report.surviving[0].contains("TIE"));
    assert!(report.surviving[2].contains("-152%"));
}
