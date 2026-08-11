//! Persistent fuzz infrastructure for the Phase A wire and canonical inputs (bn-3tz3).
//!
//! # What this is
//!
//! A deterministic, seed-driven, structure-aware fuzz harness over the ten decoders Phase
//! A exposes to untrusted bytes, with a committed corpus, an explicit resource budget per
//! target, a typed outcome for budget exhaustion, a deterministic minimizer, and a
//! deliberately defective mutant per oracle so that a green run is evidence rather than a
//! hypothesis.
//!
//! It runs in `just check`, on the pinned stable toolchain, from committed seeds.
//!
//! # Why not `cargo fuzz`
//!
//! Three of this repository's own constraints rule it out as *the gate*, and each one is
//! mechanical rather than a preference:
//!
//! 1. `Cargo.toml`'s `[workspace.lints.rust]` sets `unsafe_code = "forbid"` for every
//!    workspace crate with no crate-level override. `libfuzzer-sys`'s `fuzz_target!`
//!    expands to code that does not satisfy it.
//! 2. `rust-toolchain.toml` pins `1.97.0` and `just check` must stay on it (INV-014).
//!    `cargo fuzz` requires nightly, so a libFuzzer lane cannot be what the pinned gate
//!    runs — the same reasoning the `just sanitizers` recipe already writes down for
//!    `-Zsanitizer`.
//! 3. INV-005. A gate's inputs have to be reproducible from a committed seed rather than
//!    from ambient entropy, and libFuzzer's scheduler is neither seeded nor reproducible
//!    across runs by default.
//!
//! The alternative shape — a `fuzz/` directory excluded from the workspace, carrying its
//! own lockfile — was considered and rejected on a fourth ground:
//! `tools/governance/check_code_policy.py` reads `Cargo.toml` and `crates/*/Cargo.toml`,
//! so an excluded directory's dependencies would sit outside GOV-1-07's rationale
//! requirement and outside `dependency-audits.toml`'s fail-closed audit. Adding
//! `libfuzzer-sys`, `arbitrary`, and their trees to a workspace that deliberately has
//! exactly one external dependency (`blake3`) needs full audits somebody can honestly
//! attest, and this bone is not the place to manufacture them.
//!
//! **So: what this does NOT cover.** There is no coverage-guided lane here. This harness
//! explores what its generator constructs and what its corpus seeds; it does not follow
//! branch feedback, and it will not find a defect that only a coverage-guided search
//! reaches. Standing one up is a separate, opt-in, nightly-toolchain change with its own
//! governance entries — the corpus this bone commits is the input a later lane would
//! start from, and the case format is deliberately encoding-independent so it can be.
//!
//! # How a finding becomes a committed regression
//!
//! The round trip is wired end to end and is exercised on every run by
//! [`a_crash_becomes_a_minimized_regression`], so the procedure below is a description of
//! working code rather than a plan:
//!
//! 1. `the_campaign_finds_no_defect` fails and prints the target, the outcome, and the
//!    input bytes.
//! 2. `wire_fuzz::minimize::minimize` shrinks the input while preserving the outcome
//!    *signature* — the oracle that failed and what the target reported — under a
//!    [`MAX_PROBES`](wire_fuzz::minimize::MAX_PROBES) ceiling, deterministically, so two
//!    runs produce the same bytes.
//! 3. The result is written through `wire_fuzz::corpus::write` as a `.case` file with
//!    `class = regression`, `origin = minimized`, and the landing token it must reach.
//! 4. The file is committed under `crates/continuumd/tests/wire-fuzz-corpus/<target>/`.
//!    From then on `every_committed_case_lands_where_it_declares` replays it by name on
//!    every run, and the generator uses it as a mutation base — so the corpus gets denser
//!    around whatever the campaign has already found.
//!
//! The seeded half of the corpus is regenerated rather than transcribed:
//! `the_declared_seed_matrix_is_committed_byte_for_byte` writes what
//! `wire_fuzz::seed::rows` declares into `CARGO_TARGET_TMPDIR` and names the directory in
//! its failure message, so a seed change is a copy.
//!
//! # The rest of what this does not cover
//!
//! - **Stateful surfaces.** `continuumd::transport::Server::answer` — bytes in, a whole
//!   daemon dispatch, bytes out — is not a target here. It is not a pure function of its
//!   input (it carries daemon state across calls), so the determinism and round-trip
//!   oracles do not apply to it unmodified. The envelope decode that stands in front of it
//!   *is* covered, in both encodings.
//! - **The nesting composition gap.** `RequestEnvelope.arguments` is an `Opaque` whose
//!   bytes are parsed by a second document parse with a fresh depth budget, so two layers
//!   admit roughly twice `MAX_DEPTH` frames of recursion in total. That is bounded, and
//!   this harness bounds each layer, but it does not assert a *cumulative* budget, because
//!   the protocol does not declare one.
//! - **The kernels' non-core families.** `continuum-kernel-sat`, `-smt` and `-temporal`
//!   are routed to and probed, but the seeded cases target the `CONTCERT` family, whose
//!   wire layout is the one this file constructs from.
//! - **Proof.** Fuzzing is testing evidence, never proof (docs/29). A clean campaign is a
//!   statement about the inputs it ran, and the corpus is what makes that statement
//!   repeatable.

#![allow(
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::arithmetic_side_effects,
    reason = "test bodies assert on fixtures of known shape; the no-panic covenant is a \
              property of the shipped decoders, which is what the harness itself checks"
)]

mod wire_fuzz {
    pub mod corpus;
    pub mod engine;
    pub mod generate;
    pub mod minimize;
    pub mod mutant;
    pub mod outcome;
    pub mod seed;
    pub mod target;
}

use std::collections::BTreeSet;

use wire_fuzz::corpus::{Case, Class, Origin};
use wire_fuzz::engine::{evaluate, run_campaign, silenced};
use wire_fuzz::minimize::minimize;
use wire_fuzz::mutant::{Defect, MutantJson};
use wire_fuzz::outcome::{CaseOutcome, Oracle};
use wire_fuzz::seed::Row;
use wire_fuzz::target::by_name;

/// The campaign's committed seeds. Changing them changes what the gate explores, which is
/// a deliberate, reviewable edit rather than a run-to-run difference.
const CAMPAIGN_SEEDS: [u64; 4] = [1, 0x5eed, 0x00c0_ffee, 0x9e37_79b9_7f4a_7c15];

/// Inputs per seed, per target, in the gate. Ten targets times four seeds times this is
/// the campaign's whole probe count.
const CAMPAIGN_ITERATIONS: usize = 128;

/// The extended campaign `just fuzz` runs. Same generator, same determinism, more ground.
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

/// Inputs per seed, per target, in the extended campaign.
const EXTENDED_ITERATIONS: usize = 2048;

fn corpus() -> Vec<Case> {
    wire_fuzz::corpus::load()
}

// --- the corpus is the evidence -------------------------------------------------------

/// Every committed case still lands exactly where it says it does.
///
/// This is the assertion that keeps the corpus honest. A seed whose landing drifts — one
/// that starts being refused a layer earlier, so that the property it was written for is
/// never reached — fails here rather than continuing to look like coverage.
#[test]
fn every_committed_case_lands_where_it_declares() {
    let cases = corpus();
    assert!(
        !cases.is_empty(),
        "the corpus is empty; `wire-fuzz-corpus/` is the evidence this bone commits"
    );
    for case in &cases {
        let target = by_name(&case.target)
            .unwrap_or_else(|| panic!("case `{}` names no target `{}`", case.name, case.target));
        let outcome = evaluate(target, &case.bytes);
        match &outcome {
            CaseOutcome::Clean { landing } => assert_eq!(
                landing, &case.expect,
                "case `{}` ({}) landed at `{landing}`, not at the `{}` it declares",
                case.name, case.target, case.expect
            ),
            other => panic!(
                "case `{}` ({}) is not clean: {other}. Every committed case is a \
                 regression test: either the decoder changed, or the case did.",
                case.name, case.target
            ),
        }
    }
}

/// The committed corpus is byte-identical to the seed matrix this build declares.
///
/// Regeneration is mechanical: the failure message names a directory holding exactly the
/// files this test expected, so fixing a drift is a copy rather than a transcription.
#[test]
fn the_declared_seed_matrix_is_committed_byte_for_byte() {
    let committed = corpus();
    let declared = wire_fuzz::seed::cases();
    let scratch = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join("wire-fuzz-regen");
    let _ = std::fs::remove_dir_all(&scratch);

    let mut missing: Vec<String> = Vec::new();
    for case in &declared {
        wire_fuzz::corpus::write(&scratch, case);
        let found = committed
            .iter()
            .find(|other| other.target == case.target && other.name == case.name);
        match found {
            None => missing.push(format!("{}/{}", case.target, case.name)),
            Some(other) => assert_eq!(
                other,
                case,
                "the committed case {}/{} differs from the declared one; regenerated \
                 files are under {}",
                case.target,
                case.name,
                scratch.display()
            ),
        }
    }
    assert!(
        missing.is_empty(),
        "these declared seeds are not committed: {missing:?}; regenerated files are under \
         {}",
        scratch.display()
    );
}

/// Every corpus file is one of the declared seeds or a minimized regression.
///
/// The other direction of the check above: a stray file cannot sit in the corpus claiming
/// coverage nobody declared.
#[test]
fn every_committed_case_is_declared_or_minimized() {
    let declared: BTreeSet<(String, String)> = wire_fuzz::seed::cases()
        .into_iter()
        .map(|case| (case.target, case.name))
        .collect();
    for case in corpus() {
        if case.origin == Origin::Minimized {
            assert_eq!(
                case.class,
                Class::Regression,
                "a minimized case is a regression: {}/{}",
                case.target,
                case.name
            );
            continue;
        }
        assert!(
            declared.contains(&(case.target.clone(), case.name.clone())),
            "{}/{} is committed but not declared in seed.rs",
            case.target,
            case.name
        );
    }
}

/// Every (target, required class) pair is either seeded or carries a stated reason.
///
/// The mechanical form of "no silent hole". A class a decoder has no notion of — a cycle
/// in a format with no recursion, an identifier in a layer that carries none — is a
/// documented absence, never a gap someone has to notice.
#[test]
fn every_required_class_is_seeded_or_has_a_stated_reason() {
    let rows = wire_fuzz::seed::rows();
    for target in wire_fuzz::target::all() {
        for class in Class::REQUIRED {
            let covered = rows.iter().any(|row| match row {
                Row::Seeded(case) => case.target == target.name() && case.class == class,
                Row::NotApplicable {
                    target: named,
                    class: absent,
                    reason,
                } => {
                    *named == target.name() && *absent == class && {
                        assert!(
                            reason.len() > 40,
                            "`{}` / `{}` is declared not-applicable without a real reason",
                            target.name(),
                            class.as_str()
                        );
                        true
                    }
                }
            });
            assert!(
                covered,
                "`{}` declares neither a `{}` seed nor a reason it has none",
                target.name(),
                class.as_str()
            );
        }
    }
}

/// All four required classes are seeded somewhere, whatever any single target declares.
#[test]
fn all_four_case_classes_are_seeded() {
    let cases = wire_fuzz::seed::cases();
    for class in Class::REQUIRED {
        let seeded: Vec<&str> = cases
            .iter()
            .filter(|case| case.class == class)
            .map(|case| case.target.as_str())
            .collect();
        assert!(
            !seeded.is_empty(),
            "no target seeds the `{}` class",
            class.as_str()
        );
    }
}

// --- named regressions, one per required class -----------------------------------------

fn assert_class_lands(class: Class) {
    let cases = wire_fuzz::seed::cases();
    let mut checked = 0_usize;
    for case in cases.iter().filter(|case| case.class == class) {
        let target = by_name(&case.target).expect("a declared target");
        assert_eq!(
            evaluate(target, &case.bytes),
            CaseOutcome::Clean {
                landing: case.expect.clone()
            },
            "`{}` ({}) must land at `{}`",
            case.name,
            case.target,
            case.expect
        );
        checked += 1;
    }
    assert!(checked > 0, "the `{}` class is empty", class.as_str());
}

/// The cyclic class: one-sided nesting at `production_max + 1`, on every nesting decoder.
#[test]
fn regression_cyclic_cases_reach_their_depth_bounds() {
    assert_class_lands(Class::Cyclic);
}

/// The oversized class: a declared magnitude past a production bound, never a materialized
/// one.
#[test]
fn regression_oversized_cases_reach_their_size_bounds() {
    assert_class_lands(Class::Oversized);
}

/// The duplicate-id class: a repeated identity where the format requires uniqueness.
#[test]
fn regression_duplicate_id_cases_reach_their_uniqueness_rules() {
    assert_class_lands(Class::DuplicateId);
}

/// The noncanonical class: a second spelling of a value that already has one.
#[test]
fn regression_noncanonical_cases_reach_their_canonicity_rules() {
    assert_class_lands(Class::Noncanonical);
}

// --- resource limits --------------------------------------------------------------------

/// An over-ceiling input is a typed third outcome, not a pass and not a finding.
///
/// INV-008 in the harness's own shape: "ran out of budget" has to be distinguishable from
/// "clean" and from "found a defect", or a gate can report coverage for a case it never
/// ran. The input is built at `ceiling + 1` bytes — linear, and one byte over.
#[test]
fn an_over_ceiling_input_is_a_typed_budget_outcome() {
    for target in wire_fuzz::target::all() {
        let ceiling = target.budget().input_bytes;
        let oversized = vec![b'0'; ceiling + 1];
        let outcome = evaluate(target, &oversized);
        assert_eq!(
            outcome,
            CaseOutcome::BudgetExhausted {
                limit: wire_fuzz::outcome::Budget::INPUT_BYTES,
                needed: ceiling + 1,
                ceiling,
            },
            "`{}` must report budget exhaustion rather than running the decoder",
            target.name()
        );
        assert!(!outcome.is_defect(), "budget exhaustion is not a finding");
        assert!(
            !matches!(outcome, CaseOutcome::Clean { .. }),
            "budget exhaustion is not a pass"
        );
    }
}

/// Every committed case fits inside its target's ceiling.
///
/// A corpus case that could not be run would be a regression test that never runs.
#[test]
fn every_committed_case_fits_its_budget() {
    for case in corpus() {
        let target = by_name(&case.target).expect("a declared target");
        assert!(
            case.bytes.len() <= target.budget().input_bytes,
            "`{}` is {} bytes, over `{}`'s ceiling of {}",
            case.name,
            case.bytes.len(),
            case.target,
            target.budget().input_bytes
        );
    }
}

// --- anti-vacuity -------------------------------------------------------------------------

/// Every oracle catches a decoder that violates exactly the property it checks.
///
/// Without this the harness is a hypothesis. Each mutant differs from the real
/// `canonical.json` decoder in one behaviour; the real decoder is asserted clean on the
/// same bytes, so what fires is the injected defect and not the input.
#[test]
fn every_oracle_is_armed() {
    let real = by_name("canonical.json").expect("the real decoder");
    let mut fired: BTreeSet<Oracle> = BTreeSet::new();
    for defect in Defect::ALL {
        let bytes = MutantJson::trigger(defect);
        let mutant = MutantJson::new(defect);
        let outcome = silenced(|| evaluate(&mutant, &bytes));
        match outcome {
            CaseOutcome::Defect { oracle, .. } => {
                assert_eq!(
                    oracle,
                    defect.oracle(),
                    "the `{defect:?}` mutant tripped `{oracle}`, not the oracle it targets"
                );
                fired.insert(oracle);
            }
            other => panic!("the `{defect:?}` mutant was not caught: {other}"),
        }
        let honest = evaluate(real, &bytes);
        assert!(
            !honest.is_defect(),
            "the real decoder is not clean on the `{defect:?}` trigger: {honest}. The \
             mutant's finding would then be about the input, not the defect."
        );
    }
    assert_eq!(
        fired,
        Oracle::ALL.into_iter().collect::<BTreeSet<_>>(),
        "an oracle has no mutant, so nothing shows it can fire"
    );
}

// --- crash to minimized regression ---------------------------------------------------------

/// A crash becomes a minimized, committed, replayable regression.
///
/// The whole round trip, end to end, driven by the panicking mutant because a panic is
/// literally the crash case:
///
/// 1. a padded input trips the defect;
/// 2. the deterministic minimizer shrinks it while preserving the outcome signature;
/// 3. two minimizations of the same finding agree byte for byte;
/// 4. the minimized case is written in the corpus's own format;
/// 5. it is read back and replays to the same defect against the mutant;
/// 6. and it is clean against the real decoder, which is what makes it a regression for
///    *that defect* rather than a case that fails for everyone.
#[test]
fn a_crash_becomes_a_minimized_regression() {
    let defect = Defect::PanicOnDeepNesting;
    let mutant = MutantJson::new(defect);
    let mut padded = b"----padding----".to_vec();
    padded.extend_from_slice(&MutantJson::trigger(defect));
    padded.extend_from_slice(b"----trailing----");

    let found = silenced(|| evaluate(&mutant, &padded));
    assert!(found.is_defect(), "the padded input must trip the defect");

    let first = silenced(|| minimize(&mutant, &padded));
    let second = silenced(|| minimize(&mutant, &padded));
    assert_eq!(
        first.bytes, second.bytes,
        "minimization is not deterministic, so a committed case would not be re-derivable"
    );
    assert!(
        first.bytes.len() < padded.len(),
        "minimization did not shrink {} bytes",
        padded.len()
    );
    assert_eq!(
        first.outcome.signature(),
        found.signature(),
        "minimization changed the finding"
    );
    assert!(first.probes > 0, "minimization ran no probe");
    assert!(
        !first.exhausted,
        "minimization stopped on its {} probe ceiling rather than at a fixpoint, so the \
         committed case is the smallest the budget reached and not the smallest there is",
        wire_fuzz::minimize::MAX_PROBES
    );

    let case = Case {
        name: "mutant-panic-on-deep-nesting".to_owned(),
        target: "canonical.json".to_owned(),
        class: Class::Regression,
        expect: "json::too-deep".to_owned(),
        origin: Origin::Minimized,
        note: "the minimized form of a deliberately unbounded-recursion mutant".to_owned(),
        bytes: first.bytes.clone(),
    };
    let directory = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join("wire-fuzz-crashes");
    let path = wire_fuzz::corpus::write(&directory, &case);
    let text = std::fs::read_to_string(&path).expect("the case was written");
    let round_tripped = Case::parse(&case.name, &text).expect("the case format round-trips");
    assert_eq!(round_tripped, case, "the case format is not byte-faithful");

    let replayed = silenced(|| evaluate(&mutant, &round_tripped.bytes));
    assert_eq!(
        replayed.signature(),
        found.signature(),
        "the committed case does not replay the finding"
    );

    let real = by_name("canonical.json").expect("the real decoder");
    assert_eq!(
        evaluate(real, &round_tripped.bytes),
        CaseOutcome::Clean {
            landing: round_tripped.expect.clone()
        },
        "the production decoder must be clean on the regression, landing at its declared \
         token"
    );
}

/// The corpus format survives a render/parse round trip for every committed case.
#[test]
fn the_case_format_round_trips() {
    for case in corpus() {
        let rendered = case.render();
        let parsed = Case::parse(&case.name, &rendered).expect("a rendered case parses");
        assert_eq!(parsed, case, "`{}` does not round-trip", case.name);
    }
}

// --- the campaign ---------------------------------------------------------------------------

fn assert_campaign(seeds: &[u64], iterations: usize) {
    let cases = corpus();
    for target in wire_fuzz::target::all() {
        let report = run_campaign(target, seeds, iterations, &cases);
        assert!(
            report.findings.is_empty(),
            "`{}` produced {} finding(s); the first is {} against `{}` on {} byte(s): \
             {:02x?}. Minimize it with `minimize::minimize` and commit the case.",
            target.name(),
            report.findings.len(),
            report.findings[0].outcome,
            report.findings[0].target,
            report.findings[0].bytes.len(),
            report.findings[0].bytes
        );
        assert_eq!(
            report.probes,
            seeds.len() * iterations,
            "`{}` did not run every input",
            target.name()
        );
        assert!(
            report.over_budget * 4 < report.probes,
            "`{}` spent {} of {} probes over budget; the generator is over-producing and \
             the campaign is exploring less than its probe count suggests",
            target.name(),
            report.over_budget,
            report.probes
        );
        // The campaign's own coverage floor: a quarter of the target's declared
        // vocabulary, never fewer than four distinct landings. A probe count says how
        // many inputs ran; this says how many *rules* they reached, which is the number
        // that decays silently when a generator stops matching a format.
        let floor = (target.vocabulary().len() / 4).max(4);
        assert!(
            report.landings.len() >= floor,
            "`{}` reached {} of its {} declared landings ({:?}); the floor is {floor}. A \
             campaign that lands in one place explores one rule.",
            target.name(),
            report.landings.len(),
            target.vocabulary().len(),
            report.landings
        );
    }
}

/// The gate campaign: every target, every committed seed, no findings.
#[test]
fn the_campaign_finds_no_defect() {
    assert_campaign(&CAMPAIGN_SEEDS, CAMPAIGN_ITERATIONS);
}

/// Two runs of the same campaign produce the same report.
///
/// The property that makes a finding reproducible from the seed alone (INV-005). Without
/// it a corpus is a record of what one machine happened to do.
#[test]
fn the_campaign_is_reproducible_from_its_seed() {
    let cases = corpus();
    let target = by_name("canonical.cbor").expect("a declared target");
    let first = run_campaign(target, &CAMPAIGN_SEEDS, 64, &cases);
    let second = run_campaign(target, &CAMPAIGN_SEEDS, 64, &cases);
    assert_eq!(first.probes, second.probes);
    assert_eq!(first.clean, second.clean);
    assert_eq!(first.over_budget, second.over_budget);
    assert_eq!(first.landings, second.landings);
}

/// The two encodings carry the same envelope, and each is canonical in its own spelling.
///
/// RFC 0026 requires malformed-input fuzzing on both encodings; this is the well-formed
/// control that makes the CBOR half meaningful without committing a second spelling of
/// one message to the corpus.
#[test]
fn the_two_encodings_agree_on_one_envelope() {
    let json = continuumd::codec::to_bytes(&wire_fuzz::seed::envelope_value(
        wire_fuzz::seed::json_arguments(),
    ))
    .expect("encodes");
    // The CBOR envelope's `arguments` is the CBOR spelling of the same empty document.
    // Not a workaround: `rule encoding.opaque_payloads` carries an `Opaque` "verbatim as a
    // canonical value of the negotiated encoding", so one message genuinely has different
    // `arguments` bytes in the two encodings.
    let cbor = continuumd::codec::to_cbor_bytes(&wire_fuzz::seed::envelope_value(
        wire_fuzz::seed::cbor_arguments(),
    ))
    .expect("encodes");
    assert_ne!(json, cbor, "two encodings, not one");
    for (target, bytes) in [
        ("frame.request-envelope.json", &json),
        ("frame.request-envelope.cbor", &cbor),
    ] {
        assert_eq!(
            evaluate(by_name(target).expect("a declared target"), bytes),
            CaseOutcome::Clean {
                landing: "accepted".to_owned()
            },
            "`{target}` must accept its own encoding of the envelope"
        );
    }
}

/// The Intent Contract fixture the seeds are built from still decodes.
#[test]
fn the_intent_fixture_still_decodes() {
    assert!(
        wire_fuzz::seed::contract_decodes(),
        "the Die Hard contract fixture no longer decodes, so every intent seed built from \
         it tests something other than what it says"
    );
}

/// The extended campaign, for `just fuzz`.
///
/// Ignored by default so `just check` stays quick; `just fuzz` runs it. Same generator and
/// the same determinism — a longer run with more committed seeds, never a different kind
/// of run.
#[test]
#[ignore = "the extended lane: run it with `just fuzz`"]
fn the_extended_campaign_finds_no_defect() {
    assert_campaign(&EXTENDED_SEEDS, EXTENDED_ITERATIONS);
}
