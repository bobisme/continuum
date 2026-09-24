//! C4b landed: the typed arm names a corpus port instead of transmitting it (bn-3of5h).
//!
//! # What this file records
//!
//! `notes/plan/notes/DX10_BYTE_LEDGER.md` §8 pre-checked seven redesign candidates against
//! `rule versioning.compatible_change` and left exactly one standing inside protocol major
//! 3: C4b, as a **new operation**. `workspace.create_by_reference` is that operation, and
//! this file is its measurement — projected against landed, with every delta named.
//!
//! | Claim | Test |
//! |---|---|
//! | the typed arm sends the by-reference verb, and it is the only frame that moved | [`the_typed_arm_names_the_port_and_only_that_frame_moved`] |
//! | **the shell arm did not move, at all** | [`the_shell_arm_is_unmoved_by_this_bone`] |
//! | the landed native figure, against §8's projection | [`the_landed_saving_against_the_ledgers_projection`] |
//! | the protocol bump itself costs zero bytes | [`the_protocol_bump_is_byte_neutral`] |
//! | both arms still attempt the identical act sequence | [`the_two_arms_still_attempt_one_sequence`] |
//! | the same snapshot comes back, so nothing was traded for the bytes | [`the_by_reference_arm_reaches_the_same_snapshot`] |
//! | the margin is still nowhere near the ratified floor | [`the_ratified_byte_margin_still_does_not_clear`] |
//!
//! # The anti-gaming argument, stated before the numbers
//!
//! The discipline this lane runs under is that a policy change the *disciplined baseline*
//! could mirror must be mirrored or argued. Here there is nothing to mirror, and the reason
//! is in `crate::shell::command_line`: the baseline has written
//! `continuum workspace create --port TV-009 …` since the instrument was built, and the CLI
//! resolves that port's components from the filesystem. The typed arm had no way to say the
//! same thing and transmitted `SnapshotComponents` in full — 717 B a request against the
//! baseline's port name — which is the asymmetry PR-10 IMPL-02 recorded in as many words:
//! "a local CLI names a corpus port and resolves its components from the filesystem while a
//! remote protocol client must transmit `SnapshotComponents` in full".
//!
//! So this is the typed arm being given what the baseline always had, not a subsidy. Three
//! things keep that honest and each is a test below: the baseline's command lines, output,
//! and byte totals are **identical**, not merely close; both arms still attempt the same
//! act in the same order; and the deployment fact the typed arm now names — the registered
//! component set — is established in `Rig::fresh` beside the staged content, out of band,
//! charged to neither arm, exactly as the content those components name already was. The
//! typed arm has *always* depended on that content being there: `workspace.create`'s `files`
//! are commitments the daemon must already hold. Naming the whole set by reference asks for
//! one more out-of-band fact of the same kind.

mod support;

use continuum_benchmark::policy::{Call, PolicyKind};
use continuum_benchmark::report::{ArmTotals, RATIFIED, Report};
use continuum_benchmark::rig::Rig;
use continuum_benchmark::run::{self, ArmRun};
use continuum_benchmark::surface::Arm;
use continuum_benchmark::task::{DIE_HARD_ALL, SUBSET};
use continuumd::daemon::family::Payload;
use continuumd::daemon::state::DaemonState;
use continuumd::protocol::registry::PROTOCOL_VERSION;

/// The landed figure this bone moved, and the one it did not.
///
/// 10,859 -> 10,384 on the typed arm; 4,118 -> 4,118 on the baseline.
const NATIVE_BEFORE: u64 = 10_859;
const NATIVE_AFTER: u64 = 10_384;
const SHELL_UNCHANGED: u64 = 4_118;

/// §8.5's projection: 12,144 B over the matrix, 494 B per solved task.
const PROJECTED_PER_SOLVED: u64 = 494;

fn matrix() -> Vec<ArmRun> {
    support::both_arms()
}

fn totals(runs: &[ArmRun], arm: Arm) -> ArmTotals {
    let report = Report::new(runs.to_vec());
    *report.totals.get(&arm).expect("the arm ran")
}

#[test]
fn the_typed_arm_names_the_port_and_only_that_frame_moved() {
    // The per-operation breakdown is where a byte change has to show itself. Every row but
    // `workspace.create` is what it was; that one row is the whole of the delta, which is
    // what "one operation's argument" means when it is measured rather than asserted.
    let runs = matrix();
    let report = Report::new(runs);
    let create = report
        .per_operation
        .get(&(Arm::Native, "workspace.create"))
        .copied()
        .expect("the matrix creates workspaces");
    assert_eq!(create.calls, 24, "24 creates over the matrix, as measured");
    assert_eq!(
        create.bytes, 23_904,
        "request frame plus result frame, by reference"
    );
    assert_eq!(create.bytes / u64::from(create.calls), 996);

    // The other six rows are the 3.5 figures unchanged: nothing else about the typed arm's
    // traffic moved, which is the claim `rule snapshot.by_reference` makes about the rest
    // of the surface.
    let unchanged: Vec<(&'static str, u64, u64)> = vec![
        ("task.resume", 4, 5_656),
        ("task.status", 48, 78_144),
        ("verification.result", 24, 45_840),
        ("verification.start", 36, 37_248),
        ("workspace.fork", 8, 12_658),
        ("workspace.seal", 28, 24_780),
    ];
    for (operation, calls, bytes) in unchanged {
        let row = report
            .per_operation
            .get(&(Arm::Native, operation))
            .copied()
            .unwrap_or_else(|| panic!("{operation} is on the wire"));
        assert_eq!(u64::from(row.calls), calls, "{operation} calls");
        assert_eq!(row.bytes, bytes, "{operation} bytes");
    }
}

#[test]
fn the_shell_arm_is_unmoved_by_this_bone() {
    // The load-bearing negative. Not "close", not "within rounding": the baseline's totals,
    // its per-operation breakdown, and every one of its transcript lines are what they were
    // before this operation existed.
    let runs = matrix();
    let shell = totals(&runs, Arm::Shell);
    assert_eq!(shell.bytes_per_solved(), Some(SHELL_UNCHANGED));
    assert_eq!(shell.bytes_total, 98_836);
    assert_eq!(shell.invalid_permille(), 69);
    assert_eq!((shell.solved, shell.runs), (24, 24));

    let report = Report::new(runs);
    let create = report
        .per_operation
        .get(&(Arm::Shell, "workspace.create"))
        .copied()
        .expect("the baseline creates workspaces");
    assert_eq!(
        (u64::from(create.calls), create.bytes),
        (24, 9_000),
        "the baseline still writes `--port` and reads the same answer"
    );

    // And the command line itself, verbatim: the port has always been named here.
    let mut rig = Rig::fresh();
    let mut surface = continuum_benchmark::shell::ShellSurface::new();
    let policy = continuum_benchmark::policy::Policy::new(PolicyKind::Faithful, 1);
    let run = run::drive(&mut surface, &mut rig, &DIE_HARD_ALL, policy).expect("the arm runs");
    assert!(
        run.solved,
        "the baseline still solves the cell it always solved"
    );
    let command = continuum_benchmark::shell::command_line(
        &DIE_HARD_ALL,
        &continuum_benchmark::policy::Step {
            call: Call::CreateWorkspace { seal: false },
            principal: continuum_benchmark::rig::Principal::BUILDER,
            fault: None,
            mistake: None,
        },
    );
    assert_eq!(
        command, "continuum workspace create --port TV-009 --seal false --as agent:builder",
        "the baseline's create names a port and always did"
    );
}

#[test]
fn the_landed_saving_against_the_ledgers_projection() {
    let runs = matrix();
    let native = totals(&runs, Arm::Native);
    let report = Report::new(runs);

    assert_eq!(native.bytes_per_solved(), Some(NATIVE_AFTER));
    let saved = NATIVE_BEFORE - NATIVE_AFTER;
    assert_eq!(saved, 475, "the landed saving per solved task");
    assert_eq!(
        saved * 24,
        11_400,
        "and over the matrix, at the per-solved grain the metric is graded on"
    );
    assert_eq!(
        native.bytes_total, 249_230,
        "260,616 B before; 11,386 B of arguments went away"
    );

    // §8.5 projected 494 B per solved task from a 211 B by-reference request. The landed
    // request is larger than that model — the commitment is a `ws_`-prefixed 64-hex token
    // and the member keys are real — so the landed saving is 96% of the projection. The
    // *margin* lands exactly where §8.8's table said it would.
    assert!(
        saved < PROJECTED_PER_SOLVED,
        "the landed saving does not exceed the projection: {saved} vs {PROJECTED_PER_SOLVED}"
    );
    assert_eq!(
        (PROJECTED_PER_SOLVED - saved) * 100 / PROJECTED_PER_SOLVED,
        3,
        "the shortfall against the projection, in percent"
    );
    assert_eq!(report.margins.byte_saving_percent, Some(-152));
}

#[test]
fn the_protocol_bump_is_byte_neutral() {
    // The measurement above is attributable to the argument and to nothing else, and this
    // is why: the version appears on the wire only as a `ProtocolVersion`, and "3.2" and
    // "3.6" are the same length. A bump that changed the frame size would have put an
    // unattributed byte into every row of the table. The registry has since moved to 3.7
    // (bn-28kv4, a capability field this instrument never reads) and to 3.8 (bn-3glnv,
    // the signing wire, whose operations this instrument never names); the instrument
    // stays at the version it measured.
    assert_eq!(PROTOCOL_VERSION, "3.8");
    assert_eq!(
        continuum_benchmark::rig::VERSION,
        (3, 6),
        "the instrument speaks the version the operation is `@since`"
    );
    assert_eq!("3.2".len(), "3.6".len());

    let rig = Rig::fresh();
    assert_eq!(
        rig.hello_frame().len(),
        rig.hello_frame().len(),
        "a tautology guarding the next two lines' intent"
    );
    assert_eq!(rig.version().to_string(), "3.6");
    assert_eq!(
        rig.hello_frame().len() + rig.welcome_frame().len(),
        875,
        "the handshake is the same 875 B it was at 3.2"
    );
}

#[test]
fn the_two_arms_still_attempt_one_sequence() {
    // The fairness property this instrument rests on, re-checked at the operation that
    // changed. `Call::operation()` reports the *act*, and the act is the same act: a
    // by-reference create is `workspace.create` spelled with a different argument
    // (`rule snapshot.by_reference`), so the shared policy decides one script and both arms
    // run it.
    let runs = matrix();
    for task in SUBSET {
        for seed in [1_u16, 2, 3] {
            for kind in PolicyKind::ALL {
                let pick = |arm: Arm| {
                    runs.iter()
                        .find(|run| {
                            run.arm == arm
                                && run.task == task.id
                                && run.seed == seed
                                && run.policy == kind
                        })
                        .expect("the cell ran")
                };
                let names = |run: &ArmRun| -> Vec<String> {
                    run.transcript
                        .iter()
                        .filter_map(|line| line.split(' ').next().map(str::to_owned))
                        .collect()
                };
                assert_eq!(names(pick(Arm::Native)), names(pick(Arm::Shell)));
                assert_eq!(pick(Arm::Native).attempted, pick(Arm::Shell).attempted);
            }
        }
    }
}

#[test]
fn the_by_reference_arm_reaches_the_same_snapshot() {
    // Nothing was traded for the bytes. The reference resolves to the value the inline form
    // would have carried, and a `ws_` handle is a content identity — so this is the
    // strongest statement available that the two arms did the same thing.
    let mut rig = Rig::fresh();
    let source = DIE_HARD_ALL.source;
    let components = rig.components(source);
    let derived =
        DaemonState::components_identity(continuum_benchmark::rig::identifier(), &components)
            .expect("the identity seam names the value");
    assert_eq!(
        derived,
        rig.components_reference(source),
        "the rig registered exactly the value `Rig::components` builds"
    );

    let mut surface = support::NativeSurface::new();
    let policy = continuum_benchmark::policy::Policy::new(PolicyKind::Faithful, 1);
    let run = run::drive(&mut surface, &mut rig, &DIE_HARD_ALL, policy).expect("the arm runs");
    assert!(run.solved, "the by-reference arm solves the cell");

    // And the same components named inline publish the same handle, on the same daemon.
    let inline = rig
        .daemon()
        .state()
        .components(&derived)
        .expect("the set is registered");
    assert_eq!(
        inline.files, components.files,
        "the registered value is the value, member for member"
    );
    let _ = Payload::None;
}

#[test]
fn the_ratified_byte_margin_still_does_not_clear() {
    // The finding this lane exists to report is unchanged, and this bone does not soften
    // it. §8.8's arithmetic: the floor needs 7,977 B per solved task given back and this
    // gives back 475, which is 5.9% of the gap. The redesign question stays open and is a
    // 4.0 question for the other six ledger items.
    let report = Report::new(matrix());
    assert!(!report.margins.bytes_clear);
    assert_eq!(RATIFIED.byte_saving_percent, 30);
    assert_eq!(report.margins.byte_saving_percent, Some(-152));

    let needed = NATIVE_BEFORE - (SHELL_UNCHANGED * 70 / 100);
    assert_eq!(needed, 7_977, "what the ratified floor needs given back");
    assert_eq!(
        (NATIVE_BEFORE - NATIVE_AFTER) * 100 / needed,
        5,
        "and what this bone gives back, as a percentage of it"
    );
}
