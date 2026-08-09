//! **research/25 fallible-policy families.** The invalid-action margin the scripted policy
//! floors, converted from an argument into a measurement.
//!
//! # What this file is evidence for
//!
//! bn-2c0a's campaign closed with the invalid-action metric a **tie at 69‰/69‰** and with the
//! tie explained rather than accepted:
//!
//! > the shared policy can only make mistakes both surfaces can express; 0.6 shell-only
//! > invalid attempts per run clears the ratified 50% reduction. The tie is a lower bound on
//! > the typed arm's advantage, not a measurement of it.
//! >
//! > — `dx10_falsification.rs`, surviving conclusion 2
//!
//! and named the mechanism nobody had run:
//!
//! > research/25's own mechanism for the invalid-action margin — validating a plan against
//! > the register before acting — is unexercised, and this campaign did not build it.
//! >
//! > — surviving conclusion 7
//!
//! `continuum_benchmark::families` builds it and this file measures it. Five declared,
//! seeded, deterministic mistake classes are injected into the **same** shared policy; both
//! arms are handed the identical [`Step`] carrying the identical mistake; and the arms differ
//! only in whether they have a channel through which the mistake can be made.
//!
//! # The result, in one line
//!
//! At **one declared argument mistake per run** — the smallest non-zero rate a per-run
//! injection can have — the baseline makes **0.95 shell-only invalid attempts per run**
//! against the landed instrument's **0.60** break-even, and the ratified 50% relative
//! reduction clears at **61%**, on every family-level split. The three classes both surfaces
//! can express measure **zero**. (0.95 rather than 1.00 because a family stands aside from a
//! step that already carries an IMPL-04 fault, which cost it 2 of 48 cells.)
//!
//! # What it does not do
//!
//! Close DX-10. The bytes margin failed independently and the redesign is in force
//! (bn-762i, X1 adjudicated 2026-08-09); this readies the invalid-action margin for the
//! post-redesign re-run. Nor does it declare a *rate*: a scripted policy cannot have one, and
//! the artifact's limits say so beside the number.
//!
//! # Campaign map — attack → what it attacks → verdict
//!
//! | # | Attacked | Verdict |
//! |---|---|---|
//! | F1 | the family set is chosen to favour the typed arm | **HELD** — three of five classes are typed-expressible, injected on the typed arm, and measure 0 |
//! | F2 | a typed-expressible mistake class was omitted | **HELD** — the containment theorem, checked over all five channels and all 25 positions |
//! | F3 | counting a prevented intent as no attempt is what makes the margin | **LANDED** — the reading is load-bearing, and the other reading is reported with its number |
//! | F4 | the out-of-grammar family banked an idempotency-key collapse | **LANDED on the hazard, declined as evidence** |
//! | F5 | the injection rate was tuned to clear the break-even | **HELD** — one per run is the floor, and it reproduces bn-2c0a's synthetic prediction |
//! | F6 | a real CLI's "did you mean" makes a misnamed argument free | **HELD** — the diagnostic is charged and the attempt is never refunded |
//!
//! [`Step`]: continuum_benchmark::policy::Step

mod support;

use continuum_benchmark::falsification::{Attack, Direction, Outcome};
use continuum_benchmark::families::{
    self, CELLS, COLLIDING, CONTROL, Channel, Expressibility, FAMILIES, FamilyMargin, FamilyReport,
    GUARANTEED, Headline, MistakeClass, MistakeFamily, SITES, Split,
};
use continuum_benchmark::policy::Step;
use continuum_benchmark::report::{ArmTotals, RATIFIED};
use continuum_benchmark::run::ArmRun;
use continuum_benchmark::shell;
use continuum_benchmark::surface::Arm;
use continuum_benchmark::task::SUBSET;
use continuum_benchmark::variants;

/// Every family's two arms, measured once so a dozen assertions do not drive a dozen sweeps.
struct Campaign {
    landed_native: Vec<ArmRun>,
    landed_shell: Vec<ArmRun>,
    arms: Vec<(MistakeFamily, Vec<ArmRun>, Vec<ArmRun>)>,
}

impl Campaign {
    fn run() -> Self {
        let arms = FAMILIES
            .iter()
            .map(|family| {
                let (native, shell) = support::family_arms(*family);
                (*family, native, shell)
            })
            .collect();
        Self {
            landed_native: support::native_sweep(),
            landed_shell: continuum_benchmark::run::sweep_shell().expect("the baseline sweeps"),
            arms,
        }
    }

    fn margins(&self) -> Vec<FamilyMargin> {
        self.arms
            .iter()
            .flat_map(|(family, native, shell)| families::margins(*family, native, shell))
            .collect()
    }

    fn whole(&self, family: MistakeFamily) -> FamilyMargin {
        let (_, native, shell) = self
            .arms
            .iter()
            .find(|(candidate, _, _)| *candidate == family)
            .expect("a family this campaign ran");
        FamilyMargin::measure(family, Split::All, native, shell)
    }

    /// The break-even the *landed* instrument computed, in hundredths.
    fn break_even_hundredths(&self) -> i64 {
        let native = totals(&self.landed_native);
        let shell = totals(&self.landed_shell);
        i64::from(
            variants::invalid_reduction_breakeven_tenths(&native, &shell)
                .expect("the landed break-even"),
        ) * 10
    }

    fn headline(&self) -> Headline {
        let whole: Vec<FamilyMargin> = FAMILIES.iter().map(|f| self.whole(*f)).collect();
        let prevented: Vec<&FamilyMargin> = whole
            .iter()
            .filter(|margin| margin.class.prevented_on_typed())
            .collect();
        let shared: Vec<&FamilyMargin> = whole
            .iter()
            .filter(|margin| !margin.class.prevented_on_typed())
            .collect();
        let every: Vec<&FamilyMargin> = whole.iter().collect();
        Headline {
            break_even_hundredths: self.break_even_hundredths(),
            prevented_hundredths: families::pooled_hundredths(&prevented),
            shared_hundredths: families::pooled_hundredths(&shared),
            summed_hundredths: families::summed_hundredths(&every),
            prevented_landed_cells: prevented.iter().map(|margin| margin.shell.mistakes).sum(),
            prevented_cells: prevented.iter().map(|margin| margin.shell.runs).sum(),
        }
    }
}

fn totals(runs: &[ArmRun]) -> ArmTotals {
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

// --- the surface the families are defined over -------------------------------------------------

/// **The 25 misnameable positions are the command surface, re-derived rather than restated.**
///
/// `families::SITES` is a table, and a table can drift from the thing it describes. This
/// re-derives it from `shell::command_line` — the one function that writes a command — and
/// compares operation, ordinal and flag, position by position. It also holds the two counts
/// the artifact quotes: 25 positions in total, 15 of them on operations every solved run
/// issues.
#[test]
fn the_site_table_is_the_command_surface_it_describes() {
    let mut derived: Vec<(&'static str, u32, String)> = Vec::new();
    for call in families::probe_calls() {
        let step = Step::faithful(call);
        let command = shell::command_line(&SUBSET[0], &step);
        let flags = variants::flag_names(&command);
        assert!(
            !flags.is_empty(),
            "every command names its arguments: {command}"
        );
        for (ordinal, flag) in flags.iter().enumerate() {
            derived.push((
                step.call.operation(),
                u32::try_from(ordinal).expect("a small ordinal"),
                (*flag).to_owned(),
            ));
        }
    }

    assert_eq!(
        derived.len(),
        25,
        "misnameable argument positions across the surface"
    );
    assert_eq!(SITES.len(), derived.len(), "the table has one row per flag");
    for (site, (operation, ordinal, flag)) in SITES.iter().zip(&derived) {
        assert_eq!(site.operation, *operation);
        assert_eq!(site.ordinal, *ordinal);
        assert_eq!(site.flag, flag, "{operation} argument {ordinal}");
        assert!(
            !site.typed.is_empty(),
            "every position names the compiler-resolved field it corresponds to"
        );
    }

    assert_eq!(families::guaranteed_sites().len(), GUARANTEED);
    assert_eq!(GUARANTEED, 15, "positions every solved run reaches");
    // "Guaranteed" is a claim about the five operations a completed run cannot avoid, and it
    // is checked against the landed runs rather than asserted.
    let issued: Vec<&'static str> = [
        "workspace.create",
        "workspace.seal",
        "verification.start",
        "task.status",
        "verification.result",
    ]
    .into_iter()
    .collect();
    for run in continuum_benchmark::run::sweep_shell().expect("the baseline sweeps") {
        for operation in &issued {
            assert!(
                run.per_operation.contains_key(operation),
                "{} issues {operation}",
                run.task
            );
        }
    }
    for site in SITES {
        assert_eq!(
            site.guaranteed,
            issued.contains(&site.operation),
            "{} argument {} is marked guaranteed exactly when its operation is",
            site.operation,
            site.ordinal
        );
    }
}

// --- the control ---------------------------------------------------------------------------------

/// **Control — the family machinery is inert when no family is selected.**
///
/// `Policy::new` selects no injection, so every landed run must decide exactly the steps it
/// decided before this module existed. The comparison is over whole `ArmRun` values — every
/// counter, every byte, every transcript line — on both arms, and the control family (which
/// runs the same matrix through the family sweep's own code path) reproduces them.
#[test]
fn the_control_family_is_the_landed_matrix_run_for_run() {
    let campaign = Campaign::run();
    let (native, shell) = support::family_arms(CONTROL);
    assert_eq!(
        native, campaign.landed_native,
        "the control family is the landed native matrix"
    );
    assert_eq!(
        shell, campaign.landed_shell,
        "the control family is the landed shell matrix"
    );

    // And the landed figures the falsification campaign's own control asserts are untouched.
    let native_totals = totals(&campaign.landed_native);
    let shell_totals = totals(&campaign.landed_shell);
    assert_eq!(native_totals.invalid_permille(), 69);
    assert_eq!(shell_totals.invalid_permille(), 69);
    assert_eq!(native_totals.bytes_per_solved(), Some(10_859));
    assert_eq!(shell_totals.bytes_per_solved(), Some(4_118));
    assert_eq!((native_totals.solved, shell_totals.solved), (24, 24));

    // Nothing landed carries a mistake or a prevented intent.
    for run in campaign.landed_native.iter().chain(&campaign.landed_shell) {
        assert!(run.mistakes.is_empty(), "no landed run makes a mistake");
        assert_eq!(run.prevented, 0, "and none has an intent prevented");
    }
}

// --- the anti-gaming half -------------------------------------------------------------------------

/// **No typed-expressible mistake class was omitted.**
///
/// The containment theorem, checked three ways: over the channels (every channel the typed
/// surface has, the text surface has), over the taxonomy (every channel is the channel of
/// exactly one class, so no channel is unrepresented), and over the arms (every class the
/// typed surface can express is in fact injected on the typed arm and measured there).
#[test]
fn no_typed_expressible_mistake_class_was_omitted() {
    assert!(
        families::typed_channels_are_contained_in_text_channels(),
        "the text surface's mistake channels contain the typed surface's, strictly"
    );

    // Every channel is exactly one class's channel: the taxonomy has no gap and no duplicate.
    let mut covered: Vec<Channel> = MistakeClass::ALL
        .iter()
        .map(|class| class.channel())
        .collect();
    covered.sort_unstable();
    covered.dedup();
    assert_eq!(
        covered.len(),
        Channel::ALL.len(),
        "every channel is covered"
    );
    assert_eq!(MistakeClass::ALL.len(), Channel::ALL.len());

    // The two the typed surface does not have, named.
    let text_only: Vec<&'static str> = Channel::ALL
        .iter()
        .filter(|channel| !channel.exists_on(Arm::Native))
        .map(|channel| channel.token())
        .collect();
    assert_eq!(text_only, ["argument-presence", "argument-name"]);

    // The two surfaces name the same eight operations, so the site table covers the whole
    // command surface and not a chosen part of it; and `out-of-grammar` really is out of the
    // typed surface's own grammar rather than a mistake this crate invented.
    let mut operations: Vec<&'static str> = SITES.iter().map(|site| site.operation).collect();
    operations.dedup();
    assert_eq!(operations.len(), continuum_mcp::register::OPERATIONS.len());
    for operation in &operations {
        assert!(
            continuum_mcp::register::rule(operation).is_some(),
            "{operation} is in the typed surface's register too"
        );
    }
    assert_eq!(
        continuum_mcp::register::rule("verification.start")
            .expect("the register names it")
            .requires,
        continuum_mcp::register::Requirement::SealedSnapshot,
        "the out-of-grammar family violates research/25's action grammar, as written"
    );

    // And every class the typed surface *can* express is injected there, not only on the
    // baseline. This is the rule that stops the family set from being a one-sided list.
    let campaign = Campaign::run();
    for family in FAMILIES {
        if family.class.expressibility(Arm::Native) == Expressibility::Unrepresentable {
            continue;
        }
        let margin = campaign.whole(*family);
        assert!(
            margin.native.invalid > totals(&campaign.landed_native).invalid,
            "{} injects on the typed arm too",
            family.id
        );
    }
}

/// **The declared expressibility table is what the two arms actually did.**
///
/// `MistakeClass::expressibility` is a claim about each surface, and a claim this crate can
/// be caught making wrongly. Every cell is checked against the runs: an unrepresentable class
/// must leave a prevented intent and spend nothing; a locally refused class must leave an
/// invalid attempt that spent nothing; a wire class must leave an invalid attempt that spent
/// something.
#[test]
fn the_declared_expressibility_table_is_what_the_two_arms_did() {
    let campaign = Campaign::run();
    for (family, native, shell) in &campaign.arms {
        for (arm, runs) in [(Arm::Native, native), (Arm::Shell, shell)] {
            let expected = family.class.expressibility(arm);
            let prevented: u32 = runs.iter().map(|run| run.prevented).sum();
            let decided: usize = runs
                .iter()
                .map(|run| {
                    run.mistakes
                        .iter()
                        .filter(|class| **class == family.class)
                        .count()
                })
                .sum();
            assert!(
                decided >= 20,
                "{} decides its mistake in nearly every cell",
                family.id
            );
            match expected {
                Expressibility::Unrepresentable => {
                    assert_eq!(
                        usize::try_from(prevented).expect("a small count"),
                        decided,
                        "{} on the {} arm prevents every intent it decides",
                        family.id,
                        arm.token()
                    );
                    // Prevented intents cost nothing anywhere: not a byte, not an attempt.
                    let landed = if arm == Arm::Native {
                        &campaign.landed_native
                    } else {
                        &campaign.landed_shell
                    };
                    assert_eq!(
                        totals(runs).attempted,
                        totals(landed).attempted,
                        "{} on the {} arm attempts nothing extra",
                        family.id,
                        arm.token()
                    );
                }
                Expressibility::Local => {
                    assert_eq!(prevented, 0, "{} is representable here", family.id);
                    assert!(
                        totals(runs).zero_cost_invalid
                            > totals(if arm == Arm::Native {
                                &campaign.landed_native
                            } else {
                                &campaign.landed_shell
                            })
                            .zero_cost_invalid,
                        "{} on the {} arm is refused before the wire",
                        family.id,
                        arm.token()
                    );
                }
                Expressibility::Wire => {
                    assert_eq!(prevented, 0, "{} is representable here", family.id);
                    let landed = if arm == Arm::Native {
                        &campaign.landed_native
                    } else {
                        &campaign.landed_shell
                    };
                    assert!(
                        totals(runs).attempted > totals(landed).attempted,
                        "{} on the {} arm attempts it",
                        family.id,
                        arm.token()
                    );
                    assert!(
                        totals(runs).bytes_total > totals(landed).bytes_total,
                        "{} on the {} arm pays for it",
                        family.id,
                        arm.token()
                    );
                }
            }
        }
    }
}

/// **The three classes both surfaces can express measure a margin of zero.**
///
/// The anti-gaming measurement, and the one that makes the other two numbers worth anything:
/// a family set that produced a margin on *every* class would be measuring the taxonomy.
/// These three produce none — identical attempts, identical invalid counts, identical rates,
/// on every family-level split.
#[test]
fn the_symmetric_families_measure_a_margin_of_zero() {
    let campaign = Campaign::run();
    let mut checked = 0;
    for (family, native, shell) in campaign
        .arms
        .iter()
        .filter(|(family, _, _)| !family.class.prevented_on_typed())
    {
        for margin in families::margins(*family, native, shell) {
            assert!(margin.shell.mistakes > 0, "{} injects", family.id);
            assert_eq!(
                margin.native.mistakes,
                margin.shell.mistakes,
                "{} {}: one policy decided both arms' mistakes",
                margin.family,
                margin.split.token()
            );
            assert_eq!(
                margin.native.attempted,
                margin.shell.attempted,
                "{} {}: both arms attempt the same operations",
                margin.family,
                margin.split.token()
            );
            assert_eq!(margin.native.invalid, margin.shell.invalid);
            assert_eq!(margin.shell_only_per_run_hundredths(), 0);
            assert_eq!(margin.reduction_percent(), Some(0));
            assert!(!margin.clears());
            checked += 1;
        }
    }
    assert_eq!(
        checked,
        3 * Split::all().len(),
        "three families, five splits"
    );
}

// --- the headline ----------------------------------------------------------------------------------

/// **One argument mistake per run clears the 0.6 break-even, and the ratified margin with it.**
///
/// The number the exit bone asked for. The break-even is not restated here — it is recomputed
/// from the *landed* totals by `variants::invalid_reduction_breakeven_tenths`, the function
/// bn-2c0a wrote before these families existed, so the target was fixed before the shot.
#[test]
fn one_argument_mistake_per_run_clears_the_break_even() {
    let campaign = Campaign::run();
    let headline = campaign.headline();

    assert_eq!(
        headline.break_even_hundredths, 60,
        "0.6 shell-only invalid attempts per run, from the landed instrument"
    );
    assert_eq!(
        headline.prevented_hundredths, 95,
        "one declared argument mistake per run, landed in 46 of 48 cells"
    );
    assert_eq!(
        (headline.prevented_landed_cells, headline.prevented_cells),
        (46, 48),
        "the two cells the families stood aside from carried an IMPL-04 fault on every \
         candidate step"
    );
    assert_eq!(headline.shared_hundredths, 0);
    assert_eq!(
        headline.summed_hundredths, 190,
        "one mistake of each of the five classes per run"
    );
    assert!(
        headline.clears(),
        "0.95 measured against a 0.60 break-even: {headline:?}"
    );

    // And the margin itself, family by family and split by split.
    for family in FAMILIES
        .iter()
        .filter(|family| family.class.prevented_on_typed())
    {
        for split in Split::all() {
            let (_, native, shell) = campaign
                .arms
                .iter()
                .find(|(candidate, _, _)| candidate == family)
                .expect("a family");
            let margin = FamilyMargin::measure(*family, split, native, shell);
            let hundredths = margin.shell_only_per_run_hundredths();
            assert!(
                (91..=100).contains(&hundredths),
                "{} {}: {hundredths}/100 shell-only invalid attempts per run",
                family.id,
                split.token()
            );
            assert!(
                hundredths > headline.break_even_hundredths,
                "{} {} clears the break-even on its own",
                family.id,
                split.token()
            );
            assert!(
                margin.reduction_percent().is_some_and(|value| value >= 60),
                "{} {}: {:?}",
                family.id,
                split.token(),
                margin.reduction_percent()
            );
            assert!(
                margin.clears(),
                "{} {} clears the ratified {}% reduction",
                family.id,
                split.token(),
                RATIFIED.invalid_reduction_percent
            );
            assert_eq!(
                (margin.native.solved, margin.shell.solved),
                (margin.native.runs, margin.shell.runs),
                "{} {}: both arms still solve everything, so the margin is not a failure",
                family.id,
                split.token()
            );
        }
    }
}

/// **Every family-level split reaches its family's verdict** (research/33, principle 6).
///
/// A margin carried by the development partition, or by one semantic family, would be an
/// overfit to the tasks this harness was written against. Every split of every family reaches
/// the same verdict as the whole, and every split of a prevented family clears the break-even
/// on its own — so the headline cannot be one partition's.
///
/// The splits are not required to produce the *same* number: a family stands aside from a
/// step that already carries an IMPL-04 fault, the faults are not spread evenly over the
/// tasks, and the spread is reported (91–100 hundredths) rather than averaged away.
#[test]
fn every_family_level_split_reaches_its_family_verdict() {
    let campaign = Campaign::run();
    let break_even = campaign.break_even_hundredths();
    for (family, native, shell) in &campaign.arms {
        let whole = FamilyMargin::measure(*family, Split::All, native, shell);
        for split in Split::all() {
            let margin = FamilyMargin::measure(*family, split, native, shell);
            assert_eq!(
                margin.clears(),
                whole.clears(),
                "{} {}",
                family.id,
                split.token()
            );
            if family.class.prevented_on_typed() {
                assert!(
                    margin.shell_only_per_run_hundredths() > break_even,
                    "{} {} clears the break-even on its own",
                    family.id,
                    split.token()
                );
            } else {
                assert_eq!(margin.shell_only_per_run_hundredths(), 0);
            }
            if !matches!(split, Split::All) {
                assert_eq!(margin.native.runs, CELLS / 2, "a split is half the matrix");
                assert_eq!(margin.shell.runs, CELLS / 2);
            }
        }
    }
}

/// **An independent grader recomputes every count from the transcripts alone** (research/33,
/// principles 3 and 4).
///
/// `families::regrade` reads the operation trace and nothing else — no counter, no arm, no
/// surface. Every attempted, invalid and prevented total in this file's margins is
/// re-derived from it and compared, so a counter that drifted from the trace is caught rather
/// than trusted. The grader is shown able to fail: a corrupted trace disagrees.
#[test]
fn an_independent_grader_recomputes_every_count_from_the_transcripts() {
    let campaign = Campaign::run();
    let mut checked = 0;
    for (family, native, shell) in &campaign.arms {
        for (arm, runs) in [(Arm::Native, native), (Arm::Shell, shell)] {
            let counted = totals(runs);
            let prevented: u32 = runs.iter().map(|run| run.prevented).sum();
            let (attempted, invalid, regraded_prevented) = families::regrade(runs);
            assert_eq!(
                (attempted, invalid, regraded_prevented),
                (counted.attempted, counted.invalid, prevented),
                "{} on the {} arm regrades to its counters",
                family.id,
                arm.token()
            );
            checked += 1;
        }
    }
    assert_eq!(checked, FAMILIES.len() * 2);

    // The grader can tell two traces apart: drop one line and the recount moves.
    let (_, native, _) = &campaign.arms[0];
    let mut corrupted = native.clone();
    corrupted[0].transcript.pop();
    assert_ne!(
        families::regrade(&corrupted),
        families::regrade(native),
        "the grader reads the trace rather than agreeing with it"
    );
}

/// **Every family reproduces byte-identically from two independent sweeps** (INV-005,
/// IMPL-05's device).
///
/// Seeds here are declared schedules and cell indices, not draws, so two sweeps built from
/// scratch — fresh rigs, fresh clients, fresh daemons — must produce identical runs. The
/// whole artifact is compared, and shown able to fail.
#[test]
fn every_family_reproduces_byte_identically_from_two_independent_sweeps() {
    for family in FAMILIES {
        let (first_native, first_shell) = support::family_arms(*family);
        let (second_native, second_shell) = support::family_arms(*family);
        assert_eq!(first_native, second_native, "{} native", family.id);
        assert_eq!(first_shell, second_shell, "{} shell", family.id);
    }
}

// --- the falsification arms ----------------------------------------------------------------------

/// **F3 — counting a prevented intent as no attempt is what makes the margin.**
///
/// The load-bearing accounting choice, attacked head on and reported with its number. Under
/// the other reading — every intent the agent formed is an attempt, whether or not any
/// surface could carry it — the typed arm gains one invalid attempt per run and the margin
/// goes to exactly zero.
///
/// **LANDED.** The reading decides the metric, so the reading is argued rather than assumed,
/// and the argument is in `crate::run`'s own words: *"an attempt is one operation the policy
/// decided to make."* The policy decided one operation; the typed arm made it in one call and
/// the baseline needed two. The second reading also has a consequence worth stating: a
/// mistake decided by one shared policy is decided identically on both arms, so under it a
/// shared-policy instrument can never produce an invalid-action margin **at all** — which
/// would make plan §24.5's ratified 50% reduction unmeasurable by the only kind of instrument
/// that can be fair.
#[test]
fn f3_the_other_reading_of_a_prevented_intent_takes_the_margin_to_zero() {
    let campaign = Campaign::run();
    for family in FAMILIES
        .iter()
        .filter(|family| family.class.prevented_on_typed())
    {
        let (_, native, shell) = campaign
            .arms
            .iter()
            .find(|(candidate, _, _)| candidate == family)
            .expect("a family");
        let native_totals = totals(native);
        let shell_totals = totals(shell);
        let prevented: i64 = native.iter().map(|run| i64::from(run.prevented)).sum();

        // Reading B: a prevented intent is an attempt, and an invalid one.
        let attempted = i64::from(native_totals.attempted) + prevented;
        let invalid = i64::from(native_totals.invalid) + prevented;
        let permille = (invalid * 1000) / attempted;
        assert_eq!(
            permille,
            shell_totals.invalid_permille(),
            "{}: under the other reading the two arms tie exactly",
            family.id
        );
        assert_eq!(
            attempted,
            i64::from(shell_totals.attempted),
            "{}: and they tie on attempts too, because one policy decided both",
            family.id
        );
    }
}

/// **F4 — the out-of-grammar family could have banked an idempotency-key collapse.**
///
/// A CLI process derives its request identifier and its idempotency key from the command it
/// is running (`crate::shell::dispatch`), so a mistaken command that is *byte-identical* to
/// the legitimate one that follows it spends the same key and is answered by the daemon's
/// replay of its own earlier refusal — forever.
///
/// **LANDED on the hazard, DECLINED as evidence.** Measured, the collision costs the baseline
/// 20 of its 24 tasks and takes its invalid-action rate to 931‰, which would clear the
/// ratified margin several times over. It is not reported as a family result, because it
/// prices *this harness's key derivation* rather than the interface: `OUT_OF_GRAMMAR_CEILING`
/// makes the mistaken command distinguishable and the family measures zero.
#[test]
fn f4_the_out_of_grammar_family_declines_an_idempotency_key_collapse() {
    let colliding = families::shell_sweep(COLLIDING).expect("the baseline sweeps");
    let totals = totals(&colliding);
    assert_eq!(
        totals.solved, 4,
        "the collision costs the baseline 20 tasks"
    );
    assert_eq!(totals.invalid_permille(), 931);
    assert_eq!(
        colliding.iter().filter(|run| run.stuck.is_some()).count(),
        20,
        "and the failures are honest: the agent gave up rather than reporting a number"
    );

    // The mechanism, shown directly: the colliding command is the legitimate one.
    let snapshot = {
        let mut rig = continuum_benchmark::rig::Rig::fresh();
        let _ = &mut rig;
        continuumd::protocol::scalar::WorkspaceHandle::new("ws_0123456789abcdef").expect("a handle")
    };
    let legitimate = shell::command_line(
        &SUBSET[0],
        &Step::faithful(continuum_benchmark::policy::Call::StartVerification {
            snapshot: snapshot.clone(),
            states: SUBSET[0].ceiling,
        }),
    );
    let declined = shell::command_line(
        &SUBSET[0],
        &Step::faithful(continuum_benchmark::policy::Call::StartVerification {
            snapshot,
            states: families::OUT_OF_GRAMMAR_CEILING,
        }),
    );
    assert_ne!(
        legitimate, declined,
        "the reported family's mistaken command is not the legitimate one"
    );

    // And the family that is reported measures nothing.
    let campaign = Campaign::run();
    let reported = campaign.whole(FAMILIES[0]);
    assert_eq!(reported.class, MistakeClass::OutOfGrammar);
    assert_eq!(reported.shell_only_per_run_hundredths(), 0);
    assert_eq!(reported.shell.solved, CELLS);
}

/// **F5 — the injection rate was tuned to clear the break-even.**
///
/// **HELD.** One mistake per run is the smallest non-zero rate a per-run injection can have,
/// it is declared in the family rather than fitted, and the number it produces reproduces the
/// *synthetic* prediction bn-2c0a made before these families existed: perturbing the landed
/// baseline with one shell-only invalid attempt per run at the measured 265-byte unit cost
/// gives the same reduction the executed family measures.
#[test]
fn f5_the_declared_rate_reproduces_the_synthetic_prediction_it_was_not_fitted_to() {
    let campaign = Campaign::run();
    let unit = families::position_costs()
        .iter()
        .map(|cost| cost.misnamed_bytes)
        .sum::<u64>()
        / 25;
    let synthetic = variants::with_shell_only_mistakes(&campaign.landed_shell, 1, unit);
    let predicted = variants::plain("synthetic", &campaign.landed_native, &synthetic);

    let measured = campaign.whole(FAMILIES[4]);
    assert_eq!(measured.class, MistakeClass::MisnamedArgument);
    let predicted_percent = predicted
        .invalid_reduction_percent
        .expect("the synthetic prediction");
    let measured_percent = measured
        .reduction_percent()
        .expect("the measured reduction");
    assert_eq!(predicted_percent, 62, "bn-2c0a's synthetic prediction");
    assert!(
        (predicted_percent - measured_percent).abs() <= 1,
        "the executed family reproduces the synthetic prediction to within the one cell it \
         stood aside from ({measured_percent}% measured against {predicted_percent}% \
         predicted): {}",
        predicted.render()
    );
    assert!(predicted.clears.2 && measured.clears());

    // The rate is declared, not fitted: no cell decides more than one mistake, and the cells
    // that decide none are the ones whose every candidate step already carried a fault.
    let (_, native, shell) = campaign
        .arms
        .iter()
        .find(|(family, _, _)| family.id == FAMILIES[4].id)
        .expect("a family");
    for runs in [native, shell] {
        let mut landed = 0;
        for run in runs {
            let made = run
                .mistakes
                .iter()
                .filter(|class| **class == MistakeClass::MisnamedArgument)
                .count();
            assert!(made <= 1, "at most one mistake per run");
            landed += made;
        }
        assert_eq!(landed, 23, "landed in 23 of the 24 cells");
    }
}

/// **F6 — a real CLI's "did you mean" makes a misnamed argument free.**
///
/// **HELD.** It makes it *recoverable*, which the family already models: the corrected
/// command is a separate, separately counted attempt, and the run still solves its task. What
/// no diagnostic can refund is the attempt itself, and the attempt is what the metric counts.
/// The diagnostic is also *charged* rather than assumed free — every one of the 25 positions
/// is priced at the command the agent wrote plus the usage the tool printed, and the family
/// pays that price on the arm that has the mistake to make.
#[test]
fn f6_the_cli_diagnostic_is_charged_and_the_attempt_is_never_refunded() {
    for cost in families::position_costs() {
        assert!(
            cost.misnamed_bytes > 0 && cost.omitted_bytes > 0,
            "{} argument {} is priced",
            cost.site.operation,
            cost.site.ordinal
        );
    }

    let campaign = Campaign::run();
    let margin = campaign.whole(FAMILIES[4]);
    // Recovered, not lost: the baseline solves every task it solved before.
    assert_eq!(margin.shell.solved, CELLS);
    assert_eq!(margin.shell.solved, margin.native.solved);
    // And it paid for the diagnostic: more attempts and more bytes than the landed matrix.
    let landed = totals(&campaign.landed_shell);
    assert!(margin.shell.attempted > landed.attempted);
    assert!(margin.shell.bytes_total > landed.bytes_total);
    assert_eq!(
        margin.shell.attempted - landed.attempted,
        margin.shell.mistakes,
        "one extra attempt per landed mistake, and it is never subtracted"
    );
    // The typed arm pays neither: no attempt, no byte.
    assert_eq!(
        margin.native.attempted,
        totals(&campaign.landed_native).attempted
    );
    assert_eq!(
        margin.native.bytes_total,
        totals(&campaign.landed_native).bytes_total
    );
}

// --- the artifact -----------------------------------------------------------------------------------

/// Build the whole family artifact from measured numbers.
fn family_report() -> FamilyReport {
    let campaign = Campaign::run();
    let headline = campaign.headline();
    let margins = campaign.margins();
    let positions = families::position_costs();
    let unit = positions
        .iter()
        .map(|cost| cost.misnamed_bytes)
        .sum::<u64>()
        / 25;
    let landed_shell = totals(&campaign.landed_shell);

    let shared: Vec<&str> = FAMILIES
        .iter()
        .filter(|family| !family.class.prevented_on_typed())
        .map(|family| family.id)
        .collect();
    let misnamed = campaign.whole(FAMILIES[4]);
    let omitted = campaign.whole(FAMILIES[2]);
    let grammar = campaign.whole(FAMILIES[0]);
    let colliding = totals(&families::shell_sweep(COLLIDING).expect("the baseline sweeps"));

    let attacks = vec![
        Attack {
            id: "F1",
            direction: Direction::ProNative,
            target: "the family set is chosen to favour the typed arm",
            outcome: Outcome::Held,
            evidence: format!(
                "{} of {} classes are typed-expressible and are injected on the typed arm \
                 ({}); each measures 0 shell-only invalid attempts per run and a 0% \
                 reduction, on all {} splits",
                shared.len(),
                MistakeClass::ALL.len(),
                shared.join(","),
                Split::all().len(),
            ),
        },
        Attack {
            id: "F2",
            direction: Direction::ProNative,
            target: "a typed-expressible mistake class was omitted",
            outcome: Outcome::Held,
            evidence: format!(
                "every one of the {} mistake channels is exactly one class's channel; every \
                 channel the typed surface has, the text surface has; the {} it lacks are \
                 argument-name and argument-presence, enumerated over {} positions each \
                 naming its compiler-resolved counterpart",
                Channel::ALL.len(),
                Channel::ALL.len() - 3,
                positions.len(),
            ),
        },
        Attack {
            id: "F3",
            direction: Direction::ProNative,
            target: "counting a prevented intent as no attempt is what makes the margin",
            outcome: Outcome::Landed,
            evidence: format!(
                "under the other reading the two arms tie exactly - {}permille on both, and \
                 the same attempt count - so the margin is 0%. The reading is argued from \
                 crate::run's own definition of an attempt as an operation, and the other \
                 reading would make the ratified {}% reduction unmeasurable by any \
                 shared-policy instrument",
                misnamed.shell.invalid_permille(),
                RATIFIED.invalid_reduction_percent,
            ),
        },
        Attack {
            id: "F4",
            direction: Direction::ProShell,
            target: "the out-of-grammar family could bank an idempotency-key collapse",
            outcome: Outcome::Landed,
            evidence: format!(
                "a mistaken command byte-identical to the legitimate one is an idempotency \
                 replay: baseline solves {}/{} at {}permille invalid. DECLINED as evidence - \
                 it prices this harness's command-derived key, not the interface; the \
                 reported family declares a distinct ceiling and measures 0",
                colliding.solved,
                colliding.runs,
                colliding.invalid_permille(),
            ),
        },
        Attack {
            id: "F5",
            direction: Direction::ProShell,
            target: "the injection rate was tuned to clear the break-even",
            outcome: Outcome::Held,
            evidence: format!(
                "one mistake per run is the smallest non-zero per-run rate and is declared, \
                 not fitted; the executed family measures a {}% reduction, which is what \
                 bn-2c0a's synthetic perturbation at the measured {}-byte unit cost predicted \
                 before these families existed",
                misnamed.reduction_percent().unwrap_or_default(),
                unit,
            ),
        },
        Attack {
            id: "F6",
            direction: Direction::ProNative,
            target: "a real CLI's 'did you mean' makes a misnamed argument free",
            outcome: Outcome::Held,
            evidence: format!(
                "the diagnostic is charged at every one of the {} positions (mean {} bytes \
                 misnamed, {} bytes omitted) and the corrected command is a separate \
                 attempt; the baseline still solves {}/{}, so the margin is in attempts and \
                 not in failures",
                positions.len(),
                unit,
                positions.iter().map(|cost| cost.omitted_bytes).sum::<u64>() / 25,
                misnamed.shell.solved,
                misnamed.shell.runs,
            ),
        },
    ];

    let surviving = vec![
        format!(
            "1. THE MARGIN IS MEASURED, not floored. At one argument mistake per run - the \
             smallest non-zero rate a per-run injection can have - the baseline makes \
             {}/100 shell-only invalid attempts per run against the landed instrument's \
             {}/100 break-even (the declared rate is one per run and it landed in {} of \
             {} cells), and the ratified {}% relative reduction clears at {}% on every \
             family-level split.",
            headline.prevented_hundredths,
            headline.break_even_hundredths,
            headline.prevented_landed_cells,
            headline.prevented_cells,
            RATIFIED.invalid_reduction_percent,
            misnamed.reduction_percent().unwrap_or_default(),
        ),
        format!(
            "2. IT IS NOT THE TAXONOMY. The three classes both surfaces can express - {} - \
             are injected on BOTH arms and measure exactly zero: identical attempts, \
             identical invalid counts, identical rates. Only the two argument channels the \
             typed surface does not have produce a margin.",
            shared.join(", "),
        ),
        format!(
            "3. THE TWO ASYMMETRIC CLASSES AGREE. Misnamed and omitted arguments measure the \
             same {}/100 shell-only attempts per run and the same {}% reduction, over \
             disjoint diagnostics and different costs ({} vs {} bytes per occurrence). The \
             number is a property of the channel, not of one mistake.",
            misnamed.shell_only_per_run_hundredths(),
            omitted.reduction_percent().unwrap_or_default(),
            unit,
            positions.iter().map(|cost| cost.omitted_bytes).sum::<u64>() / 25,
        ),
        "4. THE LOAD-BEARING CHOICE IS NAMED: a mistake with no channel is not an attempt. \
         Under the other reading the arms tie exactly and the margin is 0%. That reading \
         would also make the ratified reduction unmeasurable by any instrument that drives \
         both arms with one policy, which is the only kind that can be fair. F3 carries the \
         number either way."
            .to_owned(),
        format!(
            "5. BYTES MOVE THE OTHER WAY, and stay lost. A mistake the typed surface refuses \
             locally costs it nothing: the out-of-grammar family leaves the typed arm at {} \
             bytes per solved task - its landed figure, unchanged - while the baseline goes \
             from {} to {}. Every family still leaves the typed arm far above the baseline. \
             The bytes margin failed independently (bn-762i, X1) and nothing here revisits it.",
            grammar.native.bytes_per_solved().unwrap_or_default(),
            landed_shell.bytes_per_solved().unwrap_or_default(),
            grammar.shell.bytes_per_solved().unwrap_or_default(),
        ),
        format!(
            "6. WHAT IS STILL SCRIPTED. A family declares what the agent gets wrong; it does \
             not declare how often a real agent gets it wrong. The measurement is 'one \
             argument mistake per run costs the baseline one invalid attempt the typed \
             surface cannot make', and the break-even says an agent needs {}/100 of those \
             per run. Whether a live model exceeds that is the question no scripted \
             instrument can answer.",
            headline.break_even_hundredths,
        ),
        "7. THIS DOES NOT CLOSE DX-10. It readies the invalid-action margin for the \
         post-redesign re-run. The row's pass condition is bn-762i's, and the protocol \
         redesign forced by the bytes margin is still in force."
            .to_owned(),
    ];

    FamilyReport {
        headline,
        margins,
        positions,
        attacks,
        surviving,
    }
}

/// **The artifact, held to its shape and to IMPL-05's device.**
///
/// Two whole campaigns, each driving its own fresh rigs from scratch, render byte-identically;
/// the comparison is shown able to fail; and no clock, locale or path reaches the bytes.
#[test]
fn the_family_artifact_renders_byte_identically_from_two_independent_campaigns() {
    let first = family_report();
    let second = family_report();
    assert_eq!(
        first.render(),
        second.render(),
        "two independent campaigns render one artifact"
    );

    let mut perturbed = second;
    perturbed.attacks[0].outcome = Outcome::Landed;
    assert_ne!(
        first.render(),
        perturbed.render(),
        "and the comparison can tell two artifacts apart"
    );

    let rendered = first.render();
    assert!(rendered.is_ascii(), "no locale reaches the artifact");
    assert!(!rendered.contains("2026"), "no clock reaches the artifact");
    for marker in [
        "[classes]",
        "[headline]",
        "[margins]",
        "[positions]",
        "[attacks]",
        "[surviving]",
        "[limits]",
    ] {
        assert!(rendered.contains(marker), "the artifact carries {marker}");
    }
}

/// **The artifact's contents, held to the deliverable.**
///
/// Six attacks with a verdict each, one margin row per family per split, all 25 positions,
/// and a headline that states the measured number and the break-even it is compared against.
#[test]
fn the_family_artifact_carries_the_headline_the_exit_bone_asked_for() {
    let report = family_report();

    assert_eq!(report.attacks.len(), 6);
    assert_eq!(
        report
            .attacks
            .iter()
            .filter(|attack| attack.direction == Direction::ProNative)
            .count(),
        4
    );
    assert_eq!(
        report
            .attacks
            .iter()
            .filter(|attack| attack.outcome == Outcome::Landed)
            .count(),
        2
    );
    assert_eq!(report.margins.len(), FAMILIES.len() * Split::all().len());
    assert_eq!(report.positions.len(), 25);
    assert_eq!(report.surviving.len(), 7);

    let rendered = report.render();
    assert!(rendered.contains("break_even_hundredths: 60"));
    assert!(rendered.contains("prevented_families_hundredths: 95"));
    assert!(rendered.contains("shared_families_hundredths: 0"));
    assert!(rendered.contains("prevented_mistakes_landed: 46/48 cells"));
    assert!(rendered.contains("clears: true"));
    assert!(rendered.contains("containment: typed channels are contained in text channels = true"));
    // The artifact says what it is not.
    assert!(rendered.contains("DOES NOT CLOSE DX-10"));
    assert!(rendered.contains("verdict: none"));
}
