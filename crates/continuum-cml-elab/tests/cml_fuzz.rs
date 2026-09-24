//! Persistent fuzz harness for the CML front end: parser, elaborator, and lowering
//! (bn-1nmq).
//!
//! # What this is
//!
//! A deterministic, seed-driven, structure-aware fuzz harness over the four public entry
//! points that take untrusted CML text (INV-016) — `continuum_cml_syntax::parse`,
//! `continuum_cml_elab::elaborate_source_with`, `lower_with`, and `lower_configured`
//! under the committed run configurations — with a persisted corpus, explicit budgets per
//! probe, a typed outcome for budget exhaustion, a deterministic minimizer, and one
//! deliberately defective mutant per oracle so a green run is evidence and not a
//! hypothesis. It follows the wire-fuzz lane's design (`crates/continuumd/tests/
//! wire_fuzz.rs`, bn-3tz3) and runs in `just check`; `just fuzz` runs the extended
//! campaign.
//!
//! # Budgets
//!
//! Every probe runs on a fresh thread with a 1 MiB stack (half the default),
//! under elaboration and lowering [`Limits`](continuum_cml_elab::Limits) far below the
//! production defaults, and behind a wall-clock backstop. The oracles check the usage
//! each stage reports against the limits it ran under. `cml_fuzz/engine.rs` says which
//! budget is the real bound (the counted work) and which is only a backstop (the clock).
//!
//! # Why not `cargo fuzz`
//!
//! The same three constraints the wire-fuzz lane records: the workspace forbids
//! `unsafe_code` with no crate override and `libfuzzer-sys`'s `fuzz_target!` does not
//! satisfy it; `cargo fuzz` needs nightly and the gate runs on the pinned stable
//! toolchain (INV-014); and a gate's inputs must come from committed seeds (INV-005).
//! A libFuzzer lane would also add `libfuzzer-sys` and `arbitrary`, which need human
//! audits in `tools/governance/dependency-audits.toml`. So **there is no
//! coverage-guided lane**: this harness explores what its mutator constructs from the
//! corpus, and it will not find a defect that only branch feedback reaches. The corpus
//! is plain `.ctm` text, so a later libFuzzer lane can start from it unchanged.
//!
//! # How a finding becomes a committed regression
//!
//! 1. `the_campaign_finds_no_defect` fails and prints the target, the outcome, and the
//!    source.
//! 2. `cml_fuzz::minimize::minimize` shrinks it while preserving the oracle and the
//!    landing, deterministically.
//! 3. Commit the result as `tests/cml-fuzz-corpus/cases/regression-<name>.ctm`, fix the
//!    defect, and bless the manifest. From then on
//!    `every_corpus_entry_lands_where_the_manifest_declares` replays it on every run, and
//!    the mutator uses it as a host.
//!
//! [`a_crash_becomes_a_minimized_regression`] runs that round trip end to end on every
//! run, with the panicking mutant standing in for the crash.
//!
//! # What this does not cover
//!
//! - **Non-UTF-8 input.** Every entry point takes `&str`; a byte string that is not
//!   UTF-8 cannot reach them. The decoding belongs to whoever reads the file.
//! - **Configurations as the fuzzed input.** The configured target varies the model
//!   against two fixed, committed configurations. `RunConfig::parse` itself is the strict
//!   canonical-JSON reader, which the wire-fuzz lane covers as `canonical.json`.
//! - **Proof.** Fuzzing is testing evidence, never proof (docs/29).

#![allow(
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    reason = "test bodies assert on fixtures of known shape; the no-panic covenant is a \
              property of the pipeline under test, which is what the harness checks"
)]

mod cml_fuzz {
    pub mod corpus;
    pub mod depth;
    pub mod engine;
    pub mod generate;
    pub mod minimize;
    pub mod mutant;
    pub mod target;
}

use std::collections::{BTreeMap, BTreeSet};
use std::sync::atomic::Ordering;

use cml_fuzz::corpus::{self, Origin};
use cml_fuzz::engine::{Budget, FUZZ_LIMITS, Oracle, Outcome, Target, evaluate, silenced};
use cml_fuzz::generate::{Rng, generate};
use cml_fuzz::minimize::{MAX_PROBES, minimize};
use cml_fuzz::mutant::{Defect, MUTANTS, RELEASE};
use cml_fuzz::target::{ACCEPTED, fingerprint};

/// The gate campaign's committed seeds.
const CAMPAIGN_SEEDS: [u64; 4] = [1, 0x5eed, 0x00c0_ffee, 0x9e37_79b9_7f4a_7c15];

/// Inputs per seed in the gate. Each input is judged by all four targets.
const CAMPAIGN_ITERATIONS: usize = 384;

/// The extended campaign `just fuzz` runs.
const EXTENDED_SEEDS: [u64; 8] = [
    1,
    2,
    3,
    0x5eed,
    0x00c0_ffee,
    0x9e37_79b9_7f4a_7c15,
    0xdead_beef,
    0xffff_ffff_ffff_ffff,
];

/// Inputs per seed in the extended campaign.
const EXTENDED_ITERATIONS: usize = 6000;

/// The hand-written case classes every corpus must seed (the file-name prefix).
///
/// `lowers-` cases are well-formed models that lower, so that the mutator's hosts reach
/// the lowering and the configured lowering often enough to explore them.
const REQUIRED_CLASSES: [&str; 6] = [
    "cyclic-",
    "oversized-",
    "duplicate-",
    "fragment-",
    "budget-",
    "lowers-",
];

// --- the manifest ------------------------------------------------------------------------

/// Where every target lands on `source`, as one manifest line body.
fn landings(source: &str) -> String {
    let mut out = String::new();
    for target in cml_fuzz::target::all() {
        let outcome = evaluate(target, source);
        let Outcome::Clean { landing } = &outcome else {
            panic!(
                "`{}` is not clean on a corpus entry: {outcome:?}",
                target.name()
            );
        };
        out.push('\t');
        out.push_str(target.name());
        out.push('=');
        out.push_str(landing);
        if landing == ACCEPTED {
            out.push('@');
            out.push_str(&target.probe(source).detail);
        }
    }
    out
}

/// Rewrite the derived cases from their declaration. Both blessing tests call it first,
/// so the manifest is never computed over stale derived files, whatever the test order.
fn bless_derived() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        let cases_dir = corpus::dir().join("cases");
        let declared = corpus::derived();
        for e in corpus::cases()
            .iter()
            .filter(|e| e.origin == Origin::Derived)
        {
            let file = e.name.trim_start_matches("cases/");
            std::fs::remove_file(cases_dir.join(file)).expect("remove a stale derived case");
        }
        for (file, source) in &declared {
            std::fs::write(cases_dir.join(file), source).expect("write a derived case");
        }
    });
}

fn manifest_now() -> String {
    corpus::all()
        .iter()
        .map(|e| format!("{}{}\n", e.name, landings(&e.source)))
        .collect()
}

/// Every corpus entry still lands, on every target, exactly where the manifest says, with
/// the same identity fingerprints: the regression replay and the cross-process half of
/// the determinism law in one check.
#[test]
fn every_corpus_entry_lands_where_the_manifest_declares() {
    if corpus::blessing() {
        bless_derived();
    }
    let now = manifest_now();
    if corpus::blessing() {
        std::fs::write(corpus::manifest_path(), &now).expect("write the manifest");
    }
    let committed = corpus::manifest();
    let committed: BTreeMap<&str, &str> = committed
        .lines()
        .map(|l| l.split_once('\t').expect("a manifest line has landings"))
        .collect();
    let now_lines: BTreeMap<&str, &str> = now
        .lines()
        .map(|l| l.split_once('\t').expect("a line"))
        .collect();
    for (name, got) in &now_lines {
        let want = committed.get(name).unwrap_or_else(|| {
            panic!("`{name}` is not in the manifest; bless it with CML_FUZZ_BLESS=1")
        });
        assert_eq!(
            got, want,
            "`{name}` lands elsewhere than the manifest declares (the identity or the \
             refusal changed); if that is intended, bless with CML_FUZZ_BLESS=1 and review"
        );
    }
    let stale: Vec<&&str> = committed
        .keys()
        .filter(|k| !now_lines.contains_key(*k))
        .collect();
    assert!(
        stale.is_empty(),
        "the manifest names entries that no longer exist: {stale:?}"
    );
}

/// The committed derived cases are exactly the declared derivation, byte for byte, in
/// both directions.
#[test]
fn the_derived_cases_are_committed_byte_for_byte() {
    let declared = corpus::derived();
    if corpus::blessing() {
        bless_derived();
    }
    let committed: BTreeMap<String, String> = corpus::cases()
        .into_iter()
        .filter(|e| e.origin == Origin::Derived)
        .map(|e| (e.name.trim_start_matches("cases/").to_owned(), e.source))
        .collect();
    let declared: BTreeMap<String, String> = declared.into_iter().collect();
    assert_eq!(
        committed.keys().collect::<Vec<_>>(),
        declared.keys().collect::<Vec<_>>(),
        "the committed derived cases are not the declared set; bless with CML_FUZZ_BLESS=1"
    );
    for (file, source) in &declared {
        assert_eq!(
            &committed[file], source,
            "`{file}` differs from its derivation"
        );
    }
}

/// Every required adversarial class has at least one hand-written case.
#[test]
fn every_required_case_class_is_seeded() {
    let hand: Vec<String> = corpus::cases()
        .into_iter()
        .filter(|e| e.origin == Origin::Hand)
        .map(|e| e.name)
        .collect();
    for class in REQUIRED_CLASSES {
        assert!(
            hand.iter()
                .any(|n| n.starts_with(&format!("cases/{class}"))),
            "no hand-written `{class}` case in {hand:?}"
        );
    }
    for name in &hand {
        assert!(
            REQUIRED_CLASSES
                .iter()
                .any(|c| name.starts_with(&format!("cases/{c}"))),
            "`{name}` is in no declared class"
        );
    }
}

/// Every corpus entry fits every target's input ceiling, so no committed case is a
/// regression test that never runs.
#[test]
fn every_corpus_entry_fits_its_budget() {
    for e in corpus::all() {
        for target in cml_fuzz::target::all() {
            assert!(
                e.source.len() <= target.budget().input_bytes,
                "`{}` is {} bytes, over `{}`'s ceiling",
                e.name,
                e.source.len(),
                target.name()
            );
        }
    }
}

// --- the budgets bind ----------------------------------------------------------------------

/// The fuzz limits are real: the `budget-` cases are refused by them, typed, and the same
/// sources are accepted under the production defaults. So the limits are not set so high
/// that no input reaches them, and the refusal is the limit, not the source.
#[test]
fn the_fuzz_limits_bind_and_are_typed() {
    let mut checked = 0;
    for e in corpus::cases()
        .iter()
        .filter(|e| e.name.starts_with("cases/budget-"))
    {
        let (limited, spent) = continuum_cml_elab::elaborate_source_with(&e.source, FUZZ_LIMITS);
        let limited = match limited {
            Err(err) => err.code(),
            Ok(model) => {
                let (lowered, lower_spent) = continuum_cml_elab::lower_with(&model, FUZZ_LIMITS);
                assert!(
                    lower_spent.work <= FUZZ_LIMITS.work && lower_spent.nodes <= FUZZ_LIMITS.nodes
                );
                lowered
                    .expect_err("a budget case is refused under the fuzz limits")
                    .code()
            }
        };
        assert!(spent.work <= FUZZ_LIMITS.work && spent.nodes <= FUZZ_LIMITS.nodes);
        assert!(
            limited.contains("limit") || limited.contains("too_large"),
            "`{}` is refused by `{limited}`, not by a budget",
            e.name
        );
        let model = continuum_cml_elab::elaborate_source(&e.source)
            .unwrap_or_else(|err| panic!("`{}` elaborates under the defaults: {err}", e.name));
        if !limited.starts_with("cml.limit.") {
            continuum_cml_elab::lower(&model)
                .unwrap_or_else(|err| panic!("`{}` lowers under the defaults: {err}", e.name));
        }
        checked += 1;
    }
    assert!(checked >= 2, "only {checked} budget cases");
}

/// An over-ceiling input is a typed third outcome, not a pass and not a finding.
#[test]
fn an_over_ceiling_input_is_a_typed_budget_outcome() {
    for target in cml_fuzz::target::all() {
        let ceiling = target.budget().input_bytes;
        let oversized = "x".repeat(ceiling + 1);
        let outcome = evaluate(target, &oversized);
        assert_eq!(
            outcome,
            Outcome::BudgetExhausted {
                limit: Budget::INPUT_BYTES,
                needed: ceiling + 1,
                ceiling,
            }
        );
        assert!(!outcome.is_defect());
    }
}

/// The probe thread's stack really is small, and an overflow on it is fatal rather than
/// silently passed.
///
/// Three child processes run a recursion whose frame size the child measures (the
/// address distance between two levels' frames, so the check holds in any build
/// profile): one needing about 1.5 MiB on the harness's own probe runner, which must die;
/// the same depth on a plain 3 MiB thread, which must pass (so the recursion itself is
/// sound, and what kills the first child is the runner's stack); and one needing about
/// 256 KiB on the probe runner, which must pass. So the runner's stack is below 1.5 MiB
/// (`cml_fuzz::engine::SMALL_STACK` is 1 MiB), and a front-end regression that recursed
/// that deep would take the gate down with it rather than pass unnoticed.
#[test]
fn the_probe_stack_is_small_and_an_overflow_is_fatal() {
    let run = |mode: &str| {
        std::process::Command::new(std::env::current_exe().expect("the test binary"))
            .args(["--exact", "stack_probe_child", "--ignored", "--nocapture"])
            .env("CML_FUZZ_STACK_MODE", mode)
            .output()
            .expect("run the child")
    };
    for mode in ["probe-shallow", "plain-deep"] {
        let out = run(mode);
        assert!(
            out.status.success(),
            "the `{mode}` control must pass: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }
    let deep = run("probe-deep");
    assert!(
        !deep.status.success(),
        "a 1.5 MiB recursion passed on the probe runner, so its stack is not below 1.5 MiB"
    );
}

/// About one kilobyte of stack per level, not optimized away. Records the address of
/// the first two levels' frames in `marks`, so the frame size can be measured.
#[inline(never)]
fn recurse(depth: usize, marks: &mut Vec<usize>) -> u64 {
    let frame = std::hint::black_box([depth as u8; 1024]);
    if marks.len() < 2 {
        marks.push(std::ptr::from_ref(&frame) as usize);
    }
    if depth == 0 {
        return u64::from(frame[0]);
    }
    recurse(depth - 1, marks).wrapping_add(u64::from(frame[1023]))
}

/// The stack bytes one level of [`recurse`] uses in this build.
fn frame_bytes() -> usize {
    let mut marks = Vec::new();
    let _ = std::hint::black_box(recurse(2, &mut marks));
    marks[0].abs_diff(marks[1]).max(1)
}

struct StackTarget;

impl Target for StackTarget {
    fn name(&self) -> &'static str {
        "stack-probe"
    }
    fn vocabulary(&self) -> &'static [&'static str] {
        &[ACCEPTED]
    }
    fn probe(&self, src: &str) -> cml_fuzz::engine::Probe {
        let depth: usize = src.trim().parse().expect("a depth");
        let _ = std::hint::black_box(recurse(depth, &mut Vec::new()));
        cml_fuzz::engine::Probe {
            landing: ACCEPTED.to_owned(),
            detail: String::new(),
            span: None,
            usage: Vec::new(),
            round_trip: cml_fuzz::engine::RoundTrip::NotApplicable,
            depth: 0,
        }
    }
}

static STACK_TARGET: StackTarget = StackTarget;

/// The child half of [`the_probe_stack_is_small_and_an_overflow_is_fatal`].
#[test]
#[ignore = "run only as a child process by the_probe_stack_is_small_and_an_overflow_is_fatal"]
fn stack_probe_child() {
    let mode = std::env::var("CML_FUZZ_STACK_MODE").unwrap_or_else(|_| "probe-shallow".to_owned());
    let frame = frame_bytes();
    let deep = (3 << 19) / frame; // 1.5 MiB
    let shallow = (1 << 18) / frame; // 256 KiB
    match mode.as_str() {
        "probe-shallow" | "probe-deep" => {
            let depth = if mode == "probe-shallow" {
                shallow
            } else {
                deep
            };
            assert_eq!(
                evaluate(&STACK_TARGET, &depth.to_string()),
                Outcome::Clean {
                    landing: ACCEPTED.to_owned()
                }
            );
        }
        "plain-deep" => {
            let worker = std::thread::Builder::new()
                .stack_size(3 << 20)
                .spawn(move || std::hint::black_box(recurse(deep, &mut Vec::new())))
                .expect("spawn");
            let _ = worker.join().expect("the recursion itself is sound");
        }
        other => panic!("unknown mode {other}"),
    }
}

// --- determinism across processes ------------------------------------------------------------

/// Two independent child processes compute every corpus identity, and they agree with
/// each other and with this process.
///
/// Each process has its own hash seeds (`std`'s `RandomState` draws per process), its own
/// address layout, and its own thread schedule, so an identity that leaked any of them
/// would differ here. The committed manifest is the same check against a process that
/// ran at bless time.
#[test]
fn identities_are_reproduced_by_independent_processes() {
    let run = || {
        let out = std::process::Command::new(std::env::current_exe().expect("the test binary"))
            .args([
                "--exact",
                "identity_child",
                "--ignored",
                "--nocapture",
                "--test-threads=1",
            ])
            .output()
            .expect("run the child");
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8(out.stdout)
            .expect("UTF-8")
            .lines()
            .filter_map(|l| l.split_once("CMLFP\t").map(|(_, line)| line.to_owned()))
            .collect::<Vec<_>>()
    };
    let here: Vec<String> = manifest_now().lines().map(str::to_owned).collect();
    let first = run();
    let second = run();
    assert_eq!(
        first.len(),
        here.len(),
        "the child printed a different corpus"
    );
    assert_eq!(first, second, "two processes disagree on an identity");
    assert_eq!(first, here, "a child process disagrees with this one");
    assert!(
        here.iter().filter(|l| l.contains("=accepted@")).count() >= 10,
        "too few accepted entries for the check to mean anything"
    );
}

/// The child half of [`identities_are_reproduced_by_independent_processes`].
#[test]
#[ignore = "run only as a child process by identities_are_reproduced_by_independent_processes"]
fn identity_child() {
    for line in manifest_now().lines() {
        println!("CMLFP\t{line}");
    }
}

// --- anti-vacuity -----------------------------------------------------------------------------

/// Every oracle catches a target that violates exactly the property it checks, and the
/// real parse target is clean on the same source.
#[test]
fn every_oracle_is_armed() {
    assert_eq!(
        MUTANTS.iter().map(|m| m.0).collect::<Vec<_>>(),
        Defect::ALL.to_vec(),
        "one mutant per declared defect"
    );
    let mut fired = BTreeSet::new();
    for mutant in &MUTANTS {
        let defect = mutant.0;
        let src = defect.trigger();
        RELEASE.store(false, Ordering::SeqCst);
        let outcome = silenced(|| evaluate(mutant, src));
        RELEASE.store(true, Ordering::SeqCst);
        match outcome {
            Outcome::Defect { oracle, .. } => {
                assert_eq!(oracle, defect.oracle(), "`{defect:?}` tripped `{oracle:?}`");
                fired.insert(oracle);
            }
            other => panic!("the `{defect:?}` mutant was not caught: {other:?}"),
        }
        let real = evaluate(&cml_fuzz::target::ParseTarget, src);
        assert!(
            !real.is_defect(),
            "the real parser is not clean on `{defect:?}`'s trigger"
        );
    }
    assert_eq!(
        fired,
        Oracle::ALL.into_iter().collect::<BTreeSet<_>>(),
        "an oracle has no mutant, so nothing shows it can fire"
    );
}

/// The `TooDeep` oracle's measure on the parse tree is exact, so its bound is the
/// parser's own and not an approximation: known trees measure what they are, in every
/// declaration position the walk visits, and the deepest tree the parser accepts in a
/// block measures exactly what the bound leaves it.
#[test]
fn the_tree_depth_measure_is_exact() {
    use cml_fuzz::depth::file_tree_depth;
    let bound = continuum_cml_syntax::MAX_NESTING as usize;
    let cases: [(&str, usize); 8] = [
        ("invariant I { x }", 1),
        ("invariant I { x + x + x }", 3),
        ("invariant I { x''' }", 4),
        ("invariant I { ((x)) }", 1),
        ("invariant I { f(x + 1, [y]) }", 3),
        ("action A { next x = x.m(y[0]) }", 3),
        ("state { x: Nat where x <= 1 + 2 }", 3),
        ("def d(n: Nat): Nat = if n == 0 then 1 else -n", 3),
    ];
    for (decl, want) in cases {
        let src = format!("module M\n{decl}\n");
        let file = continuum_cml_syntax::parse(&src).unwrap_or_else(|e| panic!("{decl}: {e}"));
        assert_eq!(file_tree_depth(&file), want, "{decl}");
    }
    // In a block the block itself is one level, so its deepest statement is one short of
    // the bound: `x` and 62 primes (63 deep) parses, one more prime is refused.
    let deepest = format!("module M\ninvariant I {{ x{} }}\n", "'".repeat(bound - 2));
    let file = continuum_cml_syntax::parse(&deepest).expect("the deepest chain parses");
    assert_eq!(file_tree_depth(&file), bound - 1);
    assert_eq!(
        evaluate(&cml_fuzz::target::ParseTarget, &deepest),
        Outcome::Clean {
            landing: ACCEPTED.to_owned()
        }
    );
    let past = format!("module M\ninvariant I {{ x{} }}\n", "'".repeat(bound - 1));
    assert!(continuum_cml_syntax::parse(&past).is_err());
}

/// The function-type boundary, on the 1 MiB probe stack (cr-3rqxh8). `T0 = Nat` and
/// `Tn = Set[Tn-1] -> Nat` written out in full is a type tree `2n + 1` deep, but the
/// parser used to count only `n + 1` of it: it parsed an arrow's left side at the
/// arrow's own level and then put it below the new `Function` node, so `T63` (a tree 127
/// deep) was accepted at `MAX_NESTING`. Now every tree the parser accepts is at most
/// `MAX_NESTING` deep by the exact, non-recursive measure, `T62`, `T63`, and `T64` are
/// refused typed, and the largest accepted `Tn` is exactly the one the bound admits. Every
/// target is judged by every oracle, so an acceptance past the bound is a `TooDeep`
/// finding and an overflow aborts the gate.
#[test]
fn the_function_type_boundary_holds_on_the_probe_stack() {
    use cml_fuzz::depth::file_tree_depth;
    let bound = continuum_cml_syntax::MAX_NESTING as usize;
    let t = |n: usize| {
        let mut ty = String::from("Nat");
        for _ in 0..n {
            ty = format!("Set[{ty}] -> Nat");
        }
        ty
    };
    let admitted = (bound - 1) / 2; // 2n + 1 <= bound
    for n in [1, admitted, admitted + 1, 62, 63, 64] {
        for decl in [format!("const C: {}", t(n)), format!("type A = {}", t(n))] {
            let src = format!("module M\n{decl}\n");
            for target in cml_fuzz::target::all() {
                let outcome = evaluate(target, &src);
                assert!(
                    !outcome.is_defect(),
                    "`{}` on T{n}: {outcome:?}",
                    target.name()
                );
            }
            match continuum_cml_syntax::parse(&src) {
                Ok(file) => {
                    assert!(n <= admitted, "T{n} was accepted past the bound");
                    let depth = file_tree_depth(&file);
                    assert_eq!(depth, 2 * n + 1, "T{n}");
                    assert!(depth <= bound);
                }
                Err(e) => {
                    assert!(n > admitted, "T{n}, within the bound, was refused: {e}");
                    assert_eq!(e.kind, continuum_cml_syntax::ParseErrorKind::NestingTooDeep);
                }
            }
        }
    }
    // A right-nested arrow chain re-attaches nothing: `n` arrows are a tree `n + 1` deep,
    // accepted exactly up to the bound.
    for n in [bound - 2, bound - 1, bound] {
        let src = format!("module M\nconst C: {}Nat\n", "Nat -> ".repeat(n));
        match continuum_cml_syntax::parse(&src) {
            Ok(file) => {
                assert!(n < bound, "{n} arrows accepted");
                assert_eq!(file_tree_depth(&file), n + 1);
            }
            Err(e) => {
                assert_eq!(n, bound, "{n} arrows refused: {e}");
                assert_eq!(e.kind, continuum_cml_syntax::ParseErrorKind::NestingTooDeep);
            }
        }
        assert!(!evaluate(&cml_fuzz::target::ParseTarget, &src).is_defect());
    }
}

/// A crash becomes a minimized, replayable regression.
///
/// The panicking mutant stands in for the crash: a padded trigger trips it, the
/// minimizer shrinks it deterministically while keeping the signature, and the minimized
/// source replays the finding on the mutant and is clean on the real parser.
#[test]
fn a_crash_becomes_a_minimized_regression() {
    let mutant = &MUTANTS[0];
    assert_eq!(mutant.0, Defect::PanicOnDeepNesting);
    let padded = format!(
        "module Padding\nstate {{ y: Nat where y <= 2 }}\n{}action Tail {{ unchanged y }}\n",
        mutant.0.trigger().trim_start_matches("module M\n")
    );
    let found = silenced(|| evaluate(mutant, &padded));
    assert!(
        found.is_defect(),
        "the padded source must trip the defect: {found:?}"
    );

    let first = silenced(|| minimize(mutant, &padded));
    let second = silenced(|| minimize(mutant, &padded));
    assert_eq!(
        first.source, second.source,
        "minimization is not deterministic"
    );
    assert!(
        first.source.len() < padded.len(),
        "minimization did not shrink"
    );
    assert_eq!(
        first.source, "((((((((",
        "the minimum of a nesting panic is the nesting"
    );
    assert_eq!(first.outcome.signature(), found.signature());
    assert!(
        !first.exhausted,
        "stopped at the {MAX_PROBES}-probe ceiling"
    );
    assert!(first.probes > 0, "minimization ran no probe");

    let replayed = silenced(|| evaluate(mutant, &first.source));
    assert_eq!(replayed.signature(), found.signature());
    let real = evaluate(&cml_fuzz::target::ParseTarget, &first.source);
    assert!(
        !real.is_defect(),
        "the real parser must be clean on the regression: {real:?}"
    );
}

// --- the campaign -------------------------------------------------------------------------------

#[derive(Debug, Default)]
struct Report {
    probes: usize,
    over_budget: usize,
    findings: Vec<(String, Outcome)>,
    landings: BTreeMap<String, usize>,
}

fn run_campaign(target: &'static dyn Target, seeds: &[u64], iterations: usize) -> Report {
    let hosts: Vec<String> = corpus::all().into_iter().map(|e| e.source).collect();
    let mut report = Report::default();
    for seed in seeds {
        let mut rng = Rng::new(*seed);
        for _ in 0..iterations {
            let src = generate(&mut rng, &hosts);
            let outcome = evaluate(target, &src);
            report.probes += 1;
            if let Some(landing) = outcome.landing() {
                *report.landings.entry(landing.to_owned()).or_default() += 1;
            }
            match outcome {
                Outcome::Clean { .. } => {}
                Outcome::BudgetExhausted { .. } => report.over_budget += 1,
                Outcome::Defect { .. } => report.findings.push((src, outcome)),
            }
        }
    }
    report
}

/// The least number of distinct landings each target's campaign must reach, and the
/// least number of inputs it must accept. A probe count says how many inputs ran; these
/// say how many rules they reached, which is the number that decays silently when the
/// mutator stops matching the language.
fn floors(name: &str) -> (usize, usize) {
    match name {
        "cml.parse" => (16, 250),
        "cml.elab" => (28, 50),
        "cml.lower" => (36, 20),
        _ => (30, 5),
    }
}

fn assert_campaign(seeds: &[u64], iterations: usize) {
    for target in cml_fuzz::target::all() {
        let report = run_campaign(target, seeds, iterations);
        if let Some((src, outcome)) = report.findings.first() {
            panic!(
                "`{}` produced {} finding(s); the first is {outcome:?} on:\n{src}\nMinimize \
                 it with `minimize::minimize` and commit it as a regression case.",
                target.name(),
                report.findings.len()
            );
        }
        assert_eq!(report.probes, seeds.len() * iterations);
        assert!(
            report.over_budget * 20 < report.probes,
            "`{}` spent {} of {} probes over budget",
            target.name(),
            report.over_budget,
            report.probes
        );
        let (distinct, accepted) = floors(target.name());
        eprintln!(
            "{}: {} distinct landings over {} probes: {:?}",
            target.name(),
            report.landings.len(),
            report.probes,
            report.landings
        );
        assert!(
            report.landings.len() >= distinct,
            "`{}` reached {} landings ({:?}); the floor is {distinct}",
            target.name(),
            report.landings.len(),
            report.landings
        );
        let got = report.landings.get(ACCEPTED).copied().unwrap_or(0);
        assert!(
            got >= accepted,
            "`{}` accepted {got} inputs; the floor is {accepted}, so the later stages are \
             barely reached",
            target.name()
        );
    }
}

/// The gate campaign: every target, every committed seed, no finding.
#[test]
fn the_campaign_finds_no_defect() {
    assert_campaign(&CAMPAIGN_SEEDS, CAMPAIGN_ITERATIONS);
}

/// Two runs of one campaign produce the same inputs and the same landings (INV-005).
#[test]
fn the_campaign_is_reproducible_from_its_seed() {
    let hosts: Vec<String> = corpus::all().into_iter().map(|e| e.source).collect();
    let draw = |seed| {
        let mut rng = Rng::new(seed);
        (0..32)
            .map(|_| generate(&mut rng, &hosts))
            .collect::<Vec<_>>()
    };
    let (a, b) = (draw(0x5eed), draw(0x5eed));
    assert_eq!(a, b, "the mutator is not a function of its seed");
    assert_ne!(a, draw(0x5eee), "two seeds drew the same inputs");
    let target = cml_fuzz::target::by_name("cml.elab").expect("a target");
    let first = run_campaign(target, &[0x5eed], 32);
    let second = run_campaign(target, &[0x5eed], 32);
    assert_eq!(first.landings, second.landings);
    let fingerprints: Vec<String> = a.iter().map(|s| fingerprint(s.as_bytes())).collect();
    assert_eq!(fingerprints.len(), 32);
}

/// The extended campaign, for `just fuzz`. Same generator, more ground.
#[test]
#[ignore = "the extended lane: run it with `just fuzz`"]
fn the_extended_campaign_finds_no_defect() {
    assert_campaign(&EXTENDED_SEEDS, EXTENDED_ITERATIONS);
}
