//! Dedicated exit evidence for `PR-16-EXIT` (`notes/plan/notes/PLAN_REQUIREMENTS.json`,
//! id `PR-16-EXIT`; `notes/plan/notes/START_HERE_IMPLEMENTATION.md`, PR 16's Exit line).
//!
//! > **Exit:** each mutant has an expected intent/property and deterministic campaign.
//! >
//! > — `notes/plan/notes/START_HERE_IMPLEMENTATION.md`, PR 16
//!
//! An independent exit layer over the replicated register's mutant set, the way
//! `pr15_exit_evidence.rs` sits over PR 15: it touches none of the IMPL suites
//! (`pr16_impl0{3,4,5}_*.rs`) and none of `src/`. It runs every campaign again through
//! the shared support files and holds each to an expectation written **here**, not read
//! from `tests/support/register_mutants.rs`; a test then shows the two agree.
//!
//! # The mutant set and where each campaign runs
//!
//! The set is `notes/plan/examples/replicated_register.md`'s table, M01 to M10.
//!
//! - M01 to M08 change the register program. Their campaigns are IMPL-05's (bn-28oa),
//!   derived from `pr16-correct-baseline`, with the fail-stop reruns (bn-20d8u) and the
//!   carried reruns (bn-2faf1), executed again here.
//! - M09, "view maps `Submitted` storage to abstract `Chosen`", changes the refinement
//!   view, not the program. PR 16's view is the durable register's refinement map
//!   (`durable_register.ctm`, "Refinement": the abstract `chosen` is `acks` read as a
//!   map), which `support/replicated_register.rs`'s `abstract_step` applies to every
//!   projected runtime step. Its campaign is here ([`view_campaign`]): the baseline's
//!   runs, each journal projected, judged under the correct view, under M09's view, in
//!   which an epoch is chosen once a majority holds a value in its log, synced or not,
//!   and under a durable-majority control view, which shows which failures are the
//!   Submitted bytes' doing.
//!   PR 17's CIR projection (bn-2et) and the check that a mutant fails at its mapped
//!   transition (bn-1mm) are not this campaign.
//! - M10, "independence rule says two writes to same epoch commute", changes the
//!   independence relation a reduction trusts. Its campaign runs beside the reducer,
//!   in `crates/continuum-engine-dpor/tests/pr16_exit_m10_independence.rs`, whose
//!   retained artifact `crates/continuum-engine-dpor/tests/evidence/pr16-exit-m10.json`
//!   this file reads and holds to its own pin, failing closed if it is missing, not met,
//!   pinned differently, or its tests are gone or ignored.
//!
//! # Expected intent, deterministic campaign, verdict
//!
//! Each campaign has one pinned [`Expect`] ([`specs`]):
//!
//! - `Kill`: the exact set of scenario properties the campaign refutes, the symptom of
//!   the shallowest finding of the primary property, a second property and symptom when
//!   one is pinned, and the one typed binding refusal some runs may end in instead. A
//!   refused or otherwise inconclusive run is never counted as a detection, every finding
//!   must come from a plan the mutant changed, and the shallowest witness is replayed
//!   from its plan and log alone and must show the pinned symptom again.
//! - `ExcludeRefusal`: every run of every changed plan is the named typed refusal, and no
//!   run refutes a property. A typed exclusion, never a kill.
//! - `ExcludeTimer`: no finding, and the timer accounting shows the stale timer never
//!   fires in the new process. Never a kill.
//! - `Clean`: the correct program; no finding and no refusal at all.
//!
//! A mutant is killed only when one of its `Kill` campaigns is met; M03 and M08 are
//! excluded by task identity in their plain and fail-stop campaigns and killed by their
//! carried reruns. Every campaign is run at the scenario seed 104729, where its identity
//! (BLAKE3 over every plan, log and journal) must equal the one IMPL-05's golden retains,
//! and at [`EXTRA_SEEDS`], where the verdict must be reached again and the identity is
//! pinned in `tests/evidence/pr16-exit.json`; the plain mutant campaigns and M09's are
//! run twice at the first extra seed and must give identical records. A seed draws the
//! sampled plans' logs only; the plans, the two-epoch sample included, are fixed by the
//! scenario seed.
//!
//! # Fail closed
//!
//! Every claimed check is one predicate, and the machine verdict in `pr16-exit.json` is
//! met only when every required predicate ([`required`]) is present exactly once and
//! passes (`bnd-03`). A mis-pinned expectation is detected: for every campaign, every
//! alternative pin (another kind, property set, symptom, second property or refusal) is
//! not met (`bnd-01`); synthetic outcomes with only refusals, an unbuildable plan, or a
//! finding on an unchanged plan are not met; and a missing, not-met or differently
//! pinned M10 artifact is not met (`bnd-02`).
//!
//! # Evidence map (stable artifact IDs)
//!
//! | ID | Claim | Test |
//! |---|---|---|
//! | `pr16-exit-hon-01` | the mutant set is the spec's, and this file's pins agree with IMPL-05's typed expectations | [`hon_01_the_mutant_set_and_its_pins`] |
//! | `pr16-exit-pos-01` | the correct program satisfies every property: every baseline campaign is clean at every seed, and the correct view has no failure | [`pos_01_the_correct_implementation_satisfies_the_properties`] |
//! | `pr16-exit-pos-02` | each program mutant's campaigns reach their pinned verdicts at every seed, and each mutant is killed | [`pos_02_each_program_mutant_reaches_its_pinned_verdict`] |
//! | `pr16-exit-pos-03` | M09's view campaign refutes RuntimeToAbstract at every seed with a replayed witness | [`pos_03_the_view_mutant_is_killed`] |
//! | `pr16-exit-pos-04` | M10's retained artifact is present, met and pinned as here | [`pos_04_the_independence_mutant_is_killed`] |
//! | `pr16-exit-det-01` | same seed, same bytes: retained identities reproduce, repeated runs are byte-identical, extra-seed identities are pinned | [`det_01_every_campaign_is_deterministic`] |
//! | `pr16-exit-bnd-01` | every alternative pin of every campaign is not met | [`bnd_01_a_mis_pinned_expectation_is_detected`] |
//! | `pr16-exit-bnd-02` | inconclusive or unsupported results are never a kill | [`bnd_02_inconclusive_results_are_never_a_kill`] |
//! | `pr16-exit-bnd-03` | the machine verdict is the conjunction of every claimed check | [`bnd_03_the_machine_verdict_is_the_conjunction_of_every_claimed_check`] |
//! | summary | `tests/evidence/pr16-exit.json` is byte-stable | [`the_exit_summary_is_byte_stable`] |
//!
//! Regenerate with `PR16_EXIT_BLESS=1 cargo test -p continuum-asupersync --test
//! pr16_exit_evidence`; the run then fails, so new bytes are reviewed.
//!
//! # Scope
//!
//! As the campaigns': the explored runs only, most logs sampled, not DPOR; witnesses are
//! the shallowest failing runs, replayed, not minimized (PR 18). M09 is run against PR
//! 16's refinement map on the baseline's runtime runs; M10 against the dossier's
//! abstract and one-epoch durable register models (its artifact states its own scope).

#![allow(
    missing_docs,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::too_many_lines
)]

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Mutex, OnceLock};

#[path = "support/primitive_conformance_model.rs"]
#[allow(dead_code)]
mod model;

#[path = "support/replicated_register.rs"]
#[allow(dead_code)]
mod register;

#[path = "support/register_baseline.rs"]
#[allow(dead_code)]
mod baseline;

#[path = "support/register_mutants.rs"]
#[allow(dead_code)]
mod mutants;

use baseline::{Campaign, Finding, Outcome, Property};
use continuum_asupersync::binding::run as bind;
use continuum_value::identity::{Blake3Hasher, ContentHasher};
use mutants::{Id, Mutant, Refusal};
use register::{Carrier, CrashMode, Expect as Step, Raw};

/// The scenario seed, at which IMPL-05's golden retains every campaign's identity.
const SEED: u64 = baseline::SCENARIO_SEED;

/// The further seeds every campaign is run at.
const EXTRA_SEEDS: [u64; 2] = [7, 65_537];

const IMPL05_GOLDEN: &str =
    "crates/continuum-asupersync/tests/golden/pr16_impl05_mutants.evidence.txt";
const M10_ARTIFACT: &str = "crates/continuum-engine-dpor/tests/evidence/pr16-exit-m10.json";
const M10_SUITE: &str = "crates/continuum-engine-dpor/tests/pr16_exit_m10_independence.rs";
const M10_TESTS: [&str; 4] = [
    "m10_same_epoch_commute_is_refuted_and_the_reducer_never_claims_it",
    "m10_the_campaign_is_deterministic_across_seeds",
    "m10_a_mis_pinned_expectation_or_an_inconclusive_campaign_is_not_met",
    "m10_the_artifact_is_byte_stable",
];

/// M10's expectation, as pinned here; the artifact must carry the same line.
const M10_EXPECTED: &str = "refuted; property RFC 0004 dependence evidence: a pair declared independent commutes wherever both are enabled (the DPOR witness checker's DoesNotCommute premise; C005 verdict preservation rests on it); abstract-register: refuted (disables); durable-register-one-epoch: refuted (disables); durable-register-claim-a: refuted (disables)";

fn repo(rel: &str) -> String {
    let path = format!("{}/../../{rel}", env!("CARGO_MANIFEST_DIR"));
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"))
}

// ---------------------------------------------------------------------------
// the pins
// ---------------------------------------------------------------------------

/// What a finding shows, as one token: the property's own symptom.
fn symptom_of(f: &Finding) -> String {
    let head = |s: String| {
        s.split(['(', ' ', '{'])
            .next()
            .unwrap_or_default()
            .to_owned()
    };
    match f {
        Finding::AckedNotDurable(_) => "AckedNotDurable".to_owned(),
        Finding::Agreement(_) => "Agreement".to_owned(),
        Finding::Refinement(m, _) => format!("Refinement/{m:?}"),
        Finding::Quiescence(u) => format!("Quiescence/{}", head(format!("{u:?}"))),
        Finding::Conservation(u) => format!("Conservation/{}", head(format!("{u:?}"))),
        Finding::Refused(r) => {
            for k in [
                Refusal::ReservationDropped,
                Refusal::TaskEnded,
                Refusal::TaskCrashed,
            ] {
                if k.matches(r) {
                    return format!("Refused/{k:?}");
                }
            }
            "Refused/other".to_owned()
        }
        Finding::Unbuildable(_) => "Unbuildable".to_owned(),
        Finding::LiftNot(_) => "LiftNot".to_owned(),
        Finding::A7Rejected => "A7Rejected".to_owned(),
        Finding::ProjectionRefused(_) => "ProjectionRefused".to_owned(),
    }
}

/// How a stale timer is shown never to fire in the new process.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TimerFate {
    /// Graceful: the crash cancels it or it fired before.
    Cancelled,
    /// Fail-stop: the crash fences it or it fired before.
    Fenced,
}

/// A campaign's pinned expectation.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Expect {
    /// The correct program: no finding, no refusal.
    Clean,
    /// Refutes exactly `properties`; the shallowest finding of `primary.0` has symptom
    /// `primary.1`; `also` is a second property with a symptom some run shows; the only
    /// run-level finding allowed is `refusal`, and then some run must end in it.
    Kill {
        properties: Vec<Property>,
        primary: (Property, &'static str),
        also: Option<(Property, &'static str)>,
        refusal: Option<&'static str>,
    },
    /// Every run of every changed plan is this refusal, and nothing else is found.
    ExcludeRefusal(&'static str),
    /// No finding, and no stale timer fires.
    ExcludeTimer(TimerFate),
}

impl Expect {
    fn render(&self) -> String {
        match self {
            Self::Clean => "clean: the correct program, no plan changed; no finding and no refusal".to_owned(),
            Self::Kill {
                properties,
                primary,
                also,
                refusal,
            } => {
                let props: Vec<String> = properties.iter().map(|p| prop_name(*p)).collect();
                let mut s = format!(
                    "kill: refutes exactly {{{}}}; shallowest {} finding is {}",
                    props.join(", "),
                    prop_name(primary.0),
                    primary.1
                );
                if let Some((p, sym)) = also {
                    let _ = write!(s, "; also {} with {}", prop_name(*p), sym);
                }
                match refusal {
                    Some(r) => {
                        let _ = write!(s, "; some runs are the typed refusal {r}, never counted");
                    }
                    None => s.push_str("; no refused or inconclusive run"),
                }
                s
            }
            Self::ExcludeRefusal(r) => format!(
                "excluded, not a kill: every run of every changed plan is the typed refusal {r}; no property refuted"
            ),
            Self::ExcludeTimer(TimerFate::Cancelled) => "excluded, not a kill: no finding; every stale timer is cancelled by the crash or fired before it; none fires after its region is cancelled".to_owned(),
            Self::ExcludeTimer(TimerFate::Fenced) => "excluded, not a kill: no finding; every stale timer is fenced by the crash or fired before it; none fires after its incarnation crashed".to_owned(),
        }
    }
}

fn prop_name(p: Property) -> String {
    p.scenario_name().unwrap_or("run").to_owned()
}

/// Which campaign.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Kind {
    Baseline(Mode),
    CarrierBaseline(Carrier, Mode),
    Plain(Id),
    FailStop(Id),
    Carried(Id, Mode),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Mode {
    Graceful,
    FailStop,
}

impl Mode {
    const fn crash(self) -> CrashMode {
        match self {
            Self::Graceful => CrashMode::Graceful,
            Self::FailStop => CrashMode::FailStop,
        }
    }
}

const CARRIERS: [Carrier; 3] = [Carrier::Message, Carrier::Timer, Carrier::Recovery];
const MODES: [Mode; 2] = [Mode::Graceful, Mode::FailStop];

fn rta() -> Property {
    Property::RuntimeToAbstract
}

fn kill(
    properties: &[Property],
    primary: (Property, &'static str),
    also: Option<(Property, &'static str)>,
    refusal: Option<&'static str>,
) -> Expect {
    Expect::Kill {
        properties: properties.to_vec(),
        primary,
        also,
        refusal,
    }
}

/// Every runtime campaign and its pin, written here.
fn specs() -> Vec<(Kind, Expect)> {
    let mut out = vec![
        (Kind::Baseline(Mode::Graceful), Expect::Clean),
        (Kind::Baseline(Mode::FailStop), Expect::Clean),
    ];
    for c in CARRIERS {
        for m in MODES {
            out.push((Kind::CarrierBaseline(c, m), Expect::Clean));
        }
    }
    let agreement = Property::Agreement;
    out.extend([
        (
            Kind::Plain(Id::M01),
            kill(
                &[agreement, rta()],
                (rta(), "AckedNotDurable"),
                Some((agreement, "Agreement")),
                None,
            ),
        ),
        (
            Kind::Plain(Id::M02),
            kill(
                &[rta()],
                (rta(), "Refinement/Unprojectable"),
                None,
                Some("Refused/ReservationDropped"),
            ),
        ),
        (
            Kind::FailStop(Id::M02),
            kill(
                &[rta()],
                (rta(), "Refinement/Unprojectable"),
                None,
                Some("Refused/ReservationDropped"),
            ),
        ),
        (
            Kind::Plain(Id::M03),
            Expect::ExcludeRefusal("Refused/TaskEnded"),
        ),
        (
            Kind::FailStop(Id::M03),
            Expect::ExcludeRefusal("Refused/TaskCrashed"),
        ),
        (
            Kind::Plain(Id::M04),
            kill(&[rta()], (rta(), "Refinement/Unprojectable"), None, None),
        ),
        (
            Kind::Plain(Id::M05),
            kill(
                &[agreement, rta()],
                (rta(), "AckedNotDurable"),
                Some((agreement, "Agreement")),
                None,
            ),
        ),
        (
            Kind::Plain(Id::M06),
            kill(
                &[Property::Quiescence],
                (Property::Quiescence, "Quiescence/TaskLive"),
                None,
                None,
            ),
        ),
        (
            Kind::Plain(Id::M07),
            kill(&[rta()], (rta(), "AckedNotDurable"), None, None),
        ),
        (
            Kind::FailStop(Id::M07),
            kill(&[rta()], (rta(), "AckedNotDurable"), None, None),
        ),
        (
            Kind::Plain(Id::M08),
            Expect::ExcludeTimer(TimerFate::Cancelled),
        ),
        (
            Kind::FailStop(Id::M08),
            Expect::ExcludeTimer(TimerFate::Fenced),
        ),
    ]);
    for id in [Id::M03, Id::M08] {
        for m in MODES {
            out.push((
                Kind::Carried(id, m),
                kill(
                    &[agreement, rta()],
                    (rta(), "AckedNotDurable"),
                    Some((agreement, "Agreement")),
                    None,
                ),
            ));
        }
    }
    out
}

/// M09's view campaign pin: RuntimeToAbstract refuted first by a `Choose` off the
/// stable-quorum step (acceptance claim C), and also by a withdrawn `Choose`.
const M09_EXPECTED: &str = "kill: refutes exactly {RuntimeToAbstract}, failing only at {View/ChooseOffQuorum at Submit/volatile, View/Withdrawn at Lose/volatile}; the shallowest is the ChooseOffQuorum (a Choose at a Submit whose majority rests on volatile bytes, not at the durable register's stable-quorum Ack: acceptance claim C); the Withdrawn is a Lose of those volatile bytes taking the abstract chosen back (the abstract register's Stability); some run's abstract history gives one epoch two values (abstract_register::Agreement read through the view); the durable-majority control view fails only at {View/ChooseOffQuorum at Sync/durable}, with no withdrawal and no two-value history, so those are the Submitted bytes' doing; the correct view has no failure on the same runs";

// ---------------------------------------------------------------------------
// running the campaigns
// ---------------------------------------------------------------------------

fn correct_graceful(plan: &register::Plan) -> Result<register::Built, String> {
    Ok(register::build_with_shutdown(plan))
}

fn correct_fail_stop(plan: &register::Plan) -> Result<register::Built, String> {
    Ok(register::build_with_shutdown_in(plan, CrashMode::FailStop))
}

fn base_at(seed: u64) -> Campaign {
    let mut b = baseline::baseline();
    b.seed = seed;
    b
}

/// The campaign of `kind` at `seed`, as a `Mutant` (a baseline's `id` is unused).
fn job(kind: Kind, base: &Campaign) -> Mutant {
    match kind {
        Kind::Baseline(Mode::Graceful) => Mutant {
            id: Id::M01,
            campaign: base.clone(),
            builder: correct_graceful,
            plan_mutant: false,
        },
        // IMPL-05 runs it as `execute_in(&base, FailStop)`, under the baseline's own
        // name, and its identity is over that name; the golden labels it
        // `pr16-correct-baseline-fail-stop` (see `name_of`).
        Kind::Baseline(Mode::FailStop) => Mutant {
            id: Id::M01,
            campaign: base.clone(),
            builder: correct_fail_stop,
            plan_mutant: false,
        },
        Kind::CarrierBaseline(c, m) => mutants::carrier_baseline(c, m.crash(), base),
        Kind::Plain(id) => mutants::mutant(id, base).expect("a program mutant"),
        Kind::FailStop(id) => mutants::mutant_fail_stop(id, base).expect("a fail-stop rerun"),
        Kind::Carried(id, m) => {
            mutants::mutant_carried(id, m.crash(), base).expect("a carried rerun")
        }
    }
}

/// Whether `kind`'s mutation changed plan `(gi, pi)`; nothing for a baseline.
fn changed(kind: Kind, m: &Mutant, base: &Campaign, gi: usize, pi: usize) -> bool {
    match kind {
        Kind::Baseline(_) | Kind::CarrierBaseline(..) => false,
        Kind::Plain(_) => mutants::changed(m, base, gi, pi),
        Kind::FailStop(_) => mutants::changed_fail_stop(m, base, gi, pi),
        Kind::Carried(_, mode) => mutants::changed_carried(m, mode.crash(), base, gi, pi),
    }
}

/// What one campaign showed, read independently of IMPL-05's summaries.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Shown {
    name: String,
    seed: u64,
    identity: String,
    plans: usize,
    runs: usize,
    changed_plans: usize,
    changed_runs: usize,
    /// Scenario properties refuted, by findings.
    properties: BTreeMap<Property, usize>,
    /// Runs showing each symptom token, run-level findings included.
    symptoms: BTreeMap<String, usize>,
    /// Runs refuting a scenario property: the detections.
    detecting_runs: usize,
    /// Runs whose only findings are run-level (refused, unbuildable, not lifted...).
    inconclusive_runs: usize,
    /// Findings on plans the mutant did not change.
    unchanged_findings: usize,
    /// Runs of changed plans, and how many of them are each run-level token.
    changed_run_tokens: BTreeMap<String, usize>,
    /// The symptom of the shallowest finding of each property.
    shallowest: BTreeMap<Property, String>,
    /// The shallowest witness of the pinned primary symptom, replayed: whether the replay
    /// shows it again.
    witness: Option<(String, bool)>,
    timers: [usize; 4],
    fenced_timers: usize,
}

fn show(kind: Kind, expect: &Expect, m: &Mutant, base: &Campaign, o: &Outcome) -> Shown {
    let mut s = Shown {
        name: name_of(kind),
        seed: m.campaign.seed,
        identity: o.digest.clone(),
        plans: o.plans.len(),
        runs: o.runs,
        changed_plans: 0,
        changed_runs: 0,
        properties: BTreeMap::new(),
        symptoms: BTreeMap::new(),
        detecting_runs: 0,
        inconclusive_runs: 0,
        unchanged_findings: 0,
        changed_run_tokens: BTreeMap::new(),
        shallowest: BTreeMap::new(),
        witness: None,
        timers: o.tally.timers,
        fenced_timers: o.tally.fenced[3],
    };
    type Depth = (u64, usize, usize, usize);
    let mut best: BTreeMap<Property, (Depth, String)> = BTreeMap::new();
    for (k, p) in o.plans.iter().enumerate() {
        let gi = m
            .campaign
            .groups
            .iter()
            .position(|g| g.id == p.group)
            .expect("a group");
        let ch = changed(kind, m, base, gi, p.index);
        if ch {
            s.changed_plans += 1;
            s.changed_runs += p.runs;
        }
        for (li, fs) in &p.findings {
            if !ch {
                s.unchanged_findings += fs.len();
            }
            let tokens: BTreeSet<String> = fs.iter().map(symptom_of).collect();
            for t in &tokens {
                *s.symptoms.entry(t.clone()).or_default() += 1;
            }
            if fs.iter().any(|f| f.property() != Property::Run) {
                s.detecting_runs += 1;
            } else {
                s.inconclusive_runs += 1;
                if ch {
                    for t in &tokens {
                        *s.changed_run_tokens.entry(t.clone()).or_default() += 1;
                    }
                }
            }
            for f in fs {
                let prop = f.property();
                if prop == Property::Run {
                    continue;
                }
                *s.properties.entry(prop).or_default() += 1;
                let key = (mutants::seq_of(f).unwrap_or(u64::MAX), k, *li, 0);
                if best.get(&prop).is_none_or(|(b, _)| key < *b) {
                    best.insert(prop, (key, symptom_of(f)));
                }
            }
        }
    }
    s.shallowest = best.into_iter().map(|(p, (_, t))| (p, t)).collect();
    if let Expect::Kill { primary, .. } = expect {
        let want = primary.1;
        s.witness = mutants::witness(m, o, |f| symptom_of(f) == want, |_, _| true).map(|w| {
            let (report, _) = mutants::replay(m, &w);
            // The replay must reproduce the recorded run's findings exactly.
            let again = report.findings == w.findings
                && report.findings.iter().any(|f| symptom_of(f) == want);
            (
                format!(
                    "group {} plan {} log {} ({} steps): {}",
                    w.group.1,
                    w.plan,
                    w.index,
                    w.log.len(),
                    want
                ),
                again,
            )
        });
    }
    s
}

/// Why `s` does not meet `e`. Empty is met.
fn judge(e: &Expect, s: &Shown) -> Vec<String> {
    let mut why = Vec::new();
    let mut need = |ok: bool, msg: String| {
        if !ok {
            why.push(format!("{}@{}: {msg}", s.name, s.seed));
        }
    };
    need(s.runs > 0, "no run".to_owned());
    let refuted: BTreeSet<Property> = s.properties.keys().copied().collect();
    match e {
        Expect::Clean => {
            // Structural for a baseline, whose kind changes no plan by definition. It is
            // what makes a Clean pin on a mutant campaign that shows nothing fail: with
            // no typed exclusion that mutant is a survivor.
            need(
                s.changed_plans == 0,
                format!("{} changed plans", s.changed_plans),
            );
            need(refuted.is_empty(), format!("refutes {refuted:?}"));
            need(s.symptoms.is_empty(), format!("findings {:?}", s.symptoms));
        }
        Expect::Kill {
            properties,
            primary,
            also,
            refusal,
        } => {
            let want: BTreeSet<Property> = properties.iter().copied().collect();
            need(
                refuted == want,
                format!("refutes {refuted:?}, pinned {want:?}"),
            );
            need(
                s.shallowest.get(&primary.0).map(String::as_str) == Some(primary.1),
                format!(
                    "shallowest {:?} finding is {:?}, pinned {}",
                    primary.0,
                    s.shallowest.get(&primary.0),
                    primary.1
                ),
            );
            match &s.witness {
                Some((_, true)) => {}
                other => need(false, format!("witness {other:?} does not replay")),
            }
            if let Some((p, sym)) = also {
                need(
                    want.contains(p),
                    format!("also {p:?} is not among the pinned properties"),
                );
                need(
                    s.symptoms.get(*sym).copied().unwrap_or(0) > 0,
                    format!("no run shows {sym}"),
                );
            }
            need(s.detecting_runs > 0, "no detecting run".to_owned());
            need(
                s.unchanged_findings == 0,
                format!("{} findings on unchanged plans", s.unchanged_findings),
            );
            let run_level: BTreeSet<&str> = s
                .symptoms
                .keys()
                .map(String::as_str)
                .filter(|t| !t.contains('/') || t.starts_with("Refused/"))
                .filter(|t| !matches!(*t, "AckedNotDurable" | "Agreement"))
                .collect();
            let allowed: BTreeSet<&str> = refusal.iter().copied().collect();
            need(
                run_level == allowed,
                format!(
                    "run-level findings {run_level:?}, pinned {allowed:?}; an inconclusive run is never a kill"
                ),
            );
        }
        Expect::ExcludeRefusal(r) => {
            need(refuted.is_empty(), format!("refutes {refuted:?}"));
            need(s.changed_runs > 0, "no changed run".to_owned());
            let only: BTreeMap<String, usize> = [((*r).to_owned(), s.changed_runs)].into();
            need(
                s.changed_run_tokens == only && s.symptoms == only,
                format!(
                    "run-level findings {:?}, pinned every one of {} changed runs {r}",
                    s.symptoms, s.changed_runs
                ),
            );
        }
        Expect::ExcludeTimer(fate) => {
            need(s.symptoms.is_empty(), format!("findings {:?}", s.symptoms));
            need(s.changed_plans > 0, "no changed plan".to_owned());
            let [scheduled, fired, cancelled, stale] = s.timers;
            need(stale == 0, format!("{stale} stale timers fired"));
            need(scheduled > 0, "no timer scheduled".to_owned());
            match fate {
                TimerFate::Cancelled => need(
                    fired + cancelled == scheduled && cancelled > 0 && s.fenced_timers == 0,
                    format!("timers {:?}, fenced {}", s.timers, s.fenced_timers),
                ),
                TimerFate::Fenced => need(
                    fired + s.fenced_timers == scheduled && cancelled == 0 && s.fenced_timers > 0,
                    format!("timers {:?}, fenced {}", s.timers, s.fenced_timers),
                ),
            }
        }
    }
    why
}

// ---------------------------------------------------------------------------
// M09: the view campaign
// ---------------------------------------------------------------------------

type ViewFn = fn(&Raw) -> Option<BTreeMap<u8, u8>>;

/// PR 16's refinement map: the abstract `chosen` is `acks` read as a map
/// (`durable_register.ctm`, "Refinement"). `None` when an epoch has two values.
fn view_correct(raw: &Raw) -> Option<BTreeMap<u8, u8>> {
    let (_, _, acks) = register::parts_of(raw);
    let mut out = BTreeMap::new();
    for (e, v) in acks {
        if out.insert(e, v).is_some() {
            return None;
        }
    }
    Some(out)
}

/// A majority view over `records`: an epoch is chosen once two replicas hold a value.
fn majority(records: impl IntoIterator<Item = (u8, u8, u8)>) -> Option<BTreeMap<u8, u8>> {
    let mut count: BTreeMap<(u8, u8), usize> = BTreeMap::new();
    for (_, e, v) in records {
        *count.entry((e, v)).or_default() += 1;
    }
    let mut out = BTreeMap::new();
    for ((e, v), n) in count {
        if n >= 2 && out.insert(e, v).is_some() {
            return None;
        }
    }
    Some(out)
}

/// M09's view: an epoch is chosen once a majority of replicas hold a value for it in
/// their logs, `Submitted` (volatile) bytes included. `None` when two values each have a
/// majority.
fn view_m09(raw: &Raw) -> Option<BTreeMap<u8, u8>> {
    majority(register::parts_of(raw).0)
}

/// The control view, without M09's defect: a majority of *durable* records. It maps a
/// `Sync` rather than the `Ack` to `Choose`, so it breaks claim C too, but it never
/// withdraws a value and never chooses two: those are the Submitted bytes' doing.
fn view_durable(raw: &Raw) -> Option<BTreeMap<u8, u8>> {
    majority(register::durable_of(raw))
}

/// One step judged through a view.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum ViewStep {
    Stutter,
    Choose,
    /// The view is undefined: two values chosen for one epoch at once.
    Unviewable,
    /// The abstract `chosen` loses or changes a value.
    Withdrawn,
    /// More than one epoch chosen by one step.
    ChooseMany,
    /// A `Choose` at a step that is not the stable-quorum `Ack` (claim C).
    ChooseOffQuorum,
}

fn judge_step(view: ViewFn, s: &register::Observed) -> ViewStep {
    let (Some(pre), Some(post)) = (&s.pre, &s.post) else {
        return ViewStep::Unviewable;
    };
    let (Some(a), Some(b)) = (view(pre), view(post)) else {
        return ViewStep::Unviewable;
    };
    if a == b {
        return ViewStep::Stutter;
    }
    if !a.iter().all(|(e, v)| b.get(e) == Some(v)) {
        return ViewStep::Withdrawn;
    }
    if b.len() != a.len() + 1 {
        return ViewStep::ChooseMany;
    }
    if matches!(&s.expect, Step::Step(l) if l.starts_with("Ack(")) {
        ViewStep::Choose
    } else {
        ViewStep::ChooseOffQuorum
    }
}

/// Where a failing step is: the durable-register action it is, and whether the value
/// it chooses or withdraws rests on a durable majority or on volatile bytes.
fn where_of(view: ViewFn, s: &register::Observed, kind: ViewStep) -> String {
    let action = match &s.expect {
        Step::Step(l) | Step::StepPrefix(l) => l.split('(').next().unwrap_or_default().to_owned(),
        Step::Stutter => "stutter".to_owned(),
    };
    let durable_majority = |raw: &Raw, e: u8, v: u8| {
        register::durable_of(raw)
            .iter()
            .filter(|&&(_, f, w)| (f, w) == (e, v))
            .count()
            >= 2
    };
    let (Some(pre), Some(post)) = (&s.pre, &s.post) else {
        return format!("{action}/unprojected");
    };
    let (a, b) = (
        view(pre).unwrap_or_default(),
        view(post).unwrap_or_default(),
    );
    let basis = match kind {
        ViewStep::ChooseOffQuorum | ViewStep::ChooseMany => b
            .iter()
            .filter(|(e, _)| !a.contains_key(e))
            .all(|(e, v)| durable_majority(post, *e, *v)),
        ViewStep::Withdrawn => a
            .iter()
            .filter(|(e, v)| b.get(e) != Some(v))
            .all(|(e, v)| durable_majority(pre, *e, *v)),
        _ => true,
    };
    format!("{action}/{}", if basis { "durable" } else { "volatile" })
}

/// What the view campaign showed, under one view.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct ViewShown {
    steps: usize,
    chooses: usize,
    /// Failing steps, by kind.
    failures: BTreeMap<String, usize>,
    /// Failing steps, by kind, action and basis (`where_of`).
    at: BTreeMap<String, usize>,
    /// Runs with a failure, and runs whose abstract history gives one epoch two values.
    failing_runs: usize,
    agreement_runs: usize,
    /// The shallowest failure: its kind and where, and its run.
    shallowest: Option<(String, usize, usize, usize, u64)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ViewCampaign {
    seed: u64,
    identity: String,
    /// Digests of the plans and of the logs alone, without the seed.
    plans_digest: String,
    logs_digest: String,
    sampled_plans: usize,
    runs: usize,
    /// Runs the binding or the projection refused: inconclusive, never a kill.
    inconclusive: usize,
    correct: ViewShown,
    mutant: ViewShown,
    control: ViewShown,
    /// Whether the shallowest M09 failure replays from its plan and log.
    replayed: bool,
}

fn view_run(
    view: ViewFn,
    steps: &[register::Observed],
    out: &mut ViewShown,
    at: (usize, usize, usize),
) -> Vec<String> {
    let mut failed = false;
    let mut history: BTreeSet<(u8, u8)> = BTreeSet::new();
    let mut verdicts = Vec::new();
    for s in steps {
        out.steps += 1;
        let v = judge_step(view, s);
        verdicts.push(format!("{}:{v:?}", s.seq));
        match v {
            ViewStep::Stutter => {}
            ViewStep::Choose => out.chooses += 1,
            other => {
                failed = true;
                let kind = format!("View/{other:?}");
                *out.failures.entry(kind.clone()).or_default() += 1;
                *out.at
                    .entry(format!("{kind} at {}", where_of(view, s, other)))
                    .or_default() += 1;
                let key = (kind, at.0, at.1, at.2, s.seq);
                if out
                    .shallowest
                    .as_ref()
                    .is_none_or(|b| (key.4, key.1, key.2, key.3) < (b.4, b.1, b.2, b.3))
                {
                    out.shallowest = Some(key);
                }
            }
        }
        if let Some(post) = s.post.as_ref().and_then(view) {
            history.extend(post);
        }
    }
    let agreement = history
        .iter()
        .any(|(e, v)| history.iter().any(|(f, w)| e == f && v != w));
    out.failing_runs += usize::from(failed);
    out.agreement_runs += usize::from(agreement);
    verdicts
}

/// The baseline's runs at `seed`, each journal projected and judged under the correct
/// view, under M09's, and under the durable-majority control.
fn view_campaign(seed: u64) -> ViewCampaign {
    let c = base_at(seed);
    let name = "pr16-exit-mut-09-submitted-as-chosen";
    let mut identity = format!("campaign {name} seed {:#x} bounds {:?}\n", c.seed, c.bounds);
    let (mut plans_text, mut logs_text) = (String::new(), String::new());
    let mut vc = ViewCampaign {
        seed,
        identity: String::new(),
        plans_digest: String::new(),
        logs_digest: String::new(),
        sampled_plans: 0,
        runs: 0,
        inconclusive: 0,
        correct: ViewShown::default(),
        mutant: ViewShown::default(),
        control: ViewShown::default(),
        replayed: false,
    };
    for (gi, group) in c.groups.iter().enumerate() {
        let _ = writeln!(identity, "group {} logs {}", group.id, group.logs);
        for (pi, plan) in group.plans.iter().enumerate() {
            let _ = writeln!(identity, "plan {pi} {}", baseline::render_plan(plan));
            let _ = writeln!(plans_text, "{}", baseline::render_plan(plan));
            let built = register::build_with_shutdown(plan);
            let (scope, logs) = baseline::logs_for(&built, group.logs, c.plan_seed(gi, pi));
            vc.sampled_plans += usize::from(scope != baseline::Scope::Exhaustive);
            for (li, log) in logs.iter().enumerate() {
                vc.runs += 1;
                let _ = writeln!(logs_text, "{log}");
                let steps = bind(&built.programs, log, &baseline::config())
                    .ok()
                    .and_then(|j| {
                        let d = j.digest().expect("digests").to_string();
                        register::observe(&built.roles, &j).ok().map(|s| (d, s))
                    });
                let Some((digest, steps)) = steps else {
                    vc.inconclusive += 1;
                    let _ = writeln!(identity, "{log} inconclusive");
                    continue;
                };
                let at = (gi, pi, li);
                let correct = view_run(view_correct, &steps, &mut vc.correct, at);
                let mutant = view_run(view_m09, &steps, &mut vc.mutant, at);
                let control = view_run(view_durable, &steps, &mut vc.control, at);
                let _ = writeln!(
                    identity,
                    "{log} {digest} {} | {} | {}",
                    correct.join(","),
                    mutant.join(","),
                    control.join(",")
                );
            }
        }
    }
    vc.identity = Blake3Hasher::hash(identity.as_bytes()).to_string();
    vc.plans_digest = Blake3Hasher::hash(plans_text.as_bytes()).to_string();
    vc.logs_digest = Blake3Hasher::hash(logs_text.as_bytes()).to_string();
    if let Some((kind, gi, pi, li, seq)) = vc.mutant.shallowest.clone() {
        let plan = &c.groups[gi].plans[pi];
        let built = register::build_with_shutdown(plan);
        let (_, logs) = baseline::logs_for(&built, c.groups[gi].logs, c.plan_seed(gi, pi));
        let j = bind(&built.programs, &logs[li], &baseline::config()).expect("a journal");
        let steps = register::observe(&built.roles, &j).expect("projects");
        vc.replayed = steps
            .iter()
            .any(|s| s.seq == seq && format!("View/{:?}", judge_step(view_m09, s)) == kind);
    }
    vc
}

/// Why the correct view is not clean on the view campaign. Empty is met.
fn judge_correct_view(
    v: &ViewCampaign,
    baseline_steps: usize,
    baseline_chooses: usize,
) -> Vec<String> {
    let mut why = Vec::new();
    let tag = format!("correct view@{}", v.seed);
    if v.inconclusive != 0 || v.runs == 0 {
        why.push(format!(
            "{tag}: {} inconclusive of {} runs",
            v.inconclusive, v.runs
        ));
    }
    if !v.correct.failures.is_empty() || v.correct.agreement_runs != 0 {
        why.push(format!("{tag}: fails: {:?}", v.correct.failures));
    }
    if v.correct.steps != baseline_steps || v.correct.chooses != baseline_chooses {
        why.push(format!(
            "{tag}: steps and chooses {}/{} are not the baseline correspondence's {baseline_steps}/{baseline_chooses}",
            v.correct.steps, v.correct.chooses
        ));
    }
    why
}

/// M09's pinned failures, by kind, action and basis: every off-quorum `Choose` is a
/// `Submit` whose majority rests on volatile bytes, and every withdrawal a `Lose` of
/// volatile bytes.
const M09_AT: [&str; 2] = [
    "View/ChooseOffQuorum at Submit/volatile",
    "View/Withdrawn at Lose/volatile",
];

/// The control view's pinned failures: every off-quorum `Choose` is a `Sync` that
/// completes a durable majority, and nothing is withdrawn.
const CONTROL_AT: [&str; 1] = ["View/ChooseOffQuorum at Sync/durable"];

/// Why the view campaign does not meet M09's pin. Empty is met.
fn judge_mutant_view(v: &ViewCampaign) -> Vec<String> {
    let mut why = Vec::new();
    let tag = format!("pr16-exit-mut-09-submitted-as-chosen@{}", v.seed);
    if v.inconclusive != 0 || v.runs == 0 {
        why.push(format!(
            "{tag}: {} inconclusive of {} runs; never a kill",
            v.inconclusive, v.runs
        ));
    }
    if v.mutant.shallowest.as_ref().map(|s| s.0.as_str()) != Some("View/ChooseOffQuorum") {
        why.push(format!(
            "{tag}: shallowest failure {:?}, pinned View/ChooseOffQuorum",
            v.mutant.shallowest
        ));
    }
    let at: BTreeSet<&str> = v.mutant.at.keys().map(String::as_str).collect();
    if at != BTreeSet::from(M09_AT) {
        why.push(format!("{tag}: failures at {at:?}, pinned {M09_AT:?}"));
    }
    if v.mutant.agreement_runs == 0 {
        why.push(format!(
            "{tag}: no run's abstract history gives an epoch two values"
        ));
    }
    if !v.replayed {
        why.push(format!("{tag}: the witness does not replay"));
    }
    let control: BTreeSet<&str> = v.control.at.keys().map(String::as_str).collect();
    if control != BTreeSet::from(CONTROL_AT) || v.control.agreement_runs != 0 {
        why.push(format!(
            "{tag}: the durable-majority control fails at {control:?} with {} two-value runs, pinned {CONTROL_AT:?} and none",
            v.control.agreement_runs
        ));
    }
    why
}

/// Crafted steps through `judge_step` and `where_of`: each kind, under each view.
fn crafted_steps() -> Vec<(String, bool)> {
    let step = |label: &str, pre: Raw, post: Raw| register::Observed {
        seq: 1,
        event: label.to_owned(),
        expect: if label.is_empty() {
            Step::Stutter
        } else {
            Step::Step(label.to_owned())
        },
        pre: Some(pre),
        post: Some(post),
    };
    let raw = register::raw_of;
    // a durable v0 at epoch 0; b submits v0 (volatile): a majority of log records.
    let one = raw(&[(0, 0, 0)], &[(1, 0)], &[]);
    let two = raw(&[(0, 0, 0), (1, 0, 0)], &[(1, 0)], &[]);
    let synced = raw(&[(0, 0, 0), (1, 0, 0)], &[], &[]);
    let acked = raw(&[(0, 0, 0), (1, 0, 0)], &[], &[(0, 0)]);
    let both = raw(&[(0, 0, 0), (1, 0, 0), (0, 1, 1), (1, 1, 1)], &[], &[]);
    let none = raw(&[(0, 0, 0)], &[], &[]);
    let submit = step("Submit(n=b,epoch=0,value=v0)", one.clone(), two.clone());
    let lose = step("Lose(n=b,epoch=0,value=v0)", two.clone(), one.clone());
    let sync = step("Sync(n=b,epoch=0)", two.clone(), synced.clone());
    let ack = step("Ack(epoch=0,value=v0)", synced.clone(), acked.clone());
    let many = step("", none, both);
    let cases: Vec<(&str, ViewFn, &register::Observed, ViewStep, Option<&str>)> = vec![
        (
            "M09 view, Submit",
            view_m09,
            &submit,
            ViewStep::ChooseOffQuorum,
            Some("Submit/volatile"),
        ),
        (
            "M09 view, Lose",
            view_m09,
            &lose,
            ViewStep::Withdrawn,
            Some("Lose/volatile"),
        ),
        ("M09 view, Sync", view_m09, &sync, ViewStep::Stutter, None),
        ("M09 view, Ack", view_m09, &ack, ViewStep::Stutter, None),
        (
            "correct view, Submit",
            view_correct,
            &submit,
            ViewStep::Stutter,
            None,
        ),
        (
            "correct view, Lose",
            view_correct,
            &lose,
            ViewStep::Stutter,
            None,
        ),
        (
            "correct view, Ack",
            view_correct,
            &ack,
            ViewStep::Choose,
            None,
        ),
        (
            "control view, Submit",
            view_durable,
            &submit,
            ViewStep::Stutter,
            None,
        ),
        (
            "control view, Sync",
            view_durable,
            &sync,
            ViewStep::ChooseOffQuorum,
            Some("Sync/durable"),
        ),
        (
            "M09 view, two epochs at once",
            view_m09,
            &many,
            ViewStep::ChooseMany,
            Some("stutter/durable"),
        ),
    ];
    cases
        .into_iter()
        .map(|(what, view, s, want, at)| {
            let got = judge_step(view, s);
            let ok = got == want && at.is_none_or(|w| where_of(view, s, got) == w);
            (format!("{what}: {got:?}"), ok)
        })
        .collect()
}

// ---------------------------------------------------------------------------
// all of it, once
// ---------------------------------------------------------------------------

struct All {
    /// `(kind, seed)` → what the campaign showed.
    shown: BTreeMap<(Kind, u64), Shown>,
    /// Repeats at the first extra seed, whole.
    repeats: BTreeMap<Kind, Shown>,
    /// The graceful baseline's correspondence at each seed: steps, chooses.
    correspondence: BTreeMap<u64, (usize, usize)>,
    views: BTreeMap<u64, ViewCampaign>,
    view_repeat: Option<ViewCampaign>,
}

enum Work {
    Campaign(Kind, u64, bool),
    View(u64, bool),
}

enum Done {
    Campaign(Kind, u64, bool, Box<Shown>, (usize, usize)),
    View(u64, bool, Box<ViewCampaign>),
}

fn seeds() -> Vec<u64> {
    let mut v = vec![SEED];
    v.extend(EXTRA_SEEDS);
    v
}

fn repeated(kind: Kind) -> bool {
    matches!(kind, Kind::Plain(_))
}

fn all() -> &'static All {
    static CELL: OnceLock<All> = OnceLock::new();
    CELL.get_or_init(|| {
        let specs = specs();
        let mut work = Vec::new();
        for seed in seeds() {
            for (kind, _) in &specs {
                work.push(Work::Campaign(*kind, seed, false));
            }
            work.push(Work::View(seed, false));
        }
        for (kind, _) in &specs {
            if repeated(*kind) {
                work.push(Work::Campaign(*kind, EXTRA_SEEDS[0], true));
            }
        }
        work.push(Work::View(EXTRA_SEEDS[0], true));
        let next = AtomicUsize::new(0);
        let done: Mutex<Vec<Done>> = Mutex::new(Vec::new());
        let workers = std::thread::available_parallelism().map_or(4, |n| n.get().min(12));
        std::thread::scope(|s| {
            for _ in 0..workers {
                s.spawn(|| {
                    loop {
                        let i = next.fetch_add(1, Ordering::SeqCst);
                        let Some(w) = work.get(i) else { break };
                        let d = match w {
                            Work::Campaign(kind, seed, rep) => {
                                let base = base_at(*seed);
                                let m = job(*kind, &base);
                                let o = mutants::execute(&m);
                                let expect =
                                    &specs.iter().find(|(k, _)| k == kind).expect("a spec").1;
                                let corr = (
                                    o.correspondence.steps,
                                    o.correspondence.chooses.values().sum(),
                                );
                                Done::Campaign(
                                    *kind,
                                    *seed,
                                    *rep,
                                    Box::new(show(*kind, expect, &m, &base, &o)),
                                    corr,
                                )
                            }
                            Work::View(seed, rep) => {
                                Done::View(*seed, *rep, Box::new(view_campaign(*seed)))
                            }
                        };
                        done.lock().expect("no worker panicked").push(d);
                    }
                });
            }
        });
        let mut a = All {
            shown: BTreeMap::new(),
            repeats: BTreeMap::new(),
            correspondence: BTreeMap::new(),
            views: BTreeMap::new(),
            view_repeat: None,
        };
        for d in done.into_inner().expect("no worker panicked") {
            match d {
                Done::Campaign(kind, seed, true, s, _) => {
                    assert_eq!(seed, EXTRA_SEEDS[0]);
                    a.repeats.insert(kind, *s);
                }
                Done::Campaign(kind, seed, false, s, corr) => {
                    if kind == Kind::Baseline(Mode::Graceful) {
                        a.correspondence.insert(seed, corr);
                    }
                    a.shown.insert((kind, seed), *s);
                }
                Done::View(_, true, v) => a.view_repeat = Some(*v),
                Done::View(seed, false, v) => {
                    a.views.insert(seed, *v);
                }
            }
        }
        a
    })
}

/// IMPL-05's retained identity of each campaign, by name, at the scenario seed.
fn retained() -> BTreeMap<String, String> {
    let golden = repo(IMPL05_GOLDEN);
    let mut out = BTreeMap::new();
    for line in golden.lines() {
        let Some(at) = line.find("campaign ") else {
            continue;
        };
        let rest = &line[at + "campaign ".len()..];
        let name = rest.split(' ').next().unwrap_or_default();
        let Some(i) = rest.find("identity ") else {
            continue;
        };
        let id: String = rest[i + "identity ".len()..].chars().take(64).collect();
        if id.len() == 64 && id.chars().all(|c| c.is_ascii_hexdigit()) {
            out.insert(name.to_owned(), id);
        }
    }
    out
}

fn name_of(kind: Kind) -> String {
    match kind {
        Kind::Baseline(Mode::FailStop) => mutants::FAIL_STOP_BASELINE.to_owned(),
        _ => job(kind, &base_at(SEED)).campaign.name.to_owned(),
    }
}

fn mutant_of(kind: Kind) -> Option<Id> {
    match kind {
        Kind::Plain(id) | Kind::FailStop(id) | Kind::Carried(id, _) => Some(id),
        _ => None,
    }
}

/// Fields of the M10 artifact, read without a JSON library: `"key":"value"` pairs.
fn json_field(text: &str, key: &str) -> Option<String> {
    // Top-level keys only: every one precedes the nested `subjects`.
    let text = &text[..text.find("\"subjects\":")?];
    let pat = format!("\"{key}\":\"");
    let at = text.find(&pat)? + pat.len();
    let mut out = String::new();
    let mut chars = text[at..].chars();
    while let Some(c) = chars.next() {
        match c {
            '"' => return Some(out),
            '\\' => match chars.next()? {
                'n' => out.push('\n'),
                c => out.push(c),
            },
            c => out.push(c),
        }
    }
    None
}

/// Why M10's retained artifact does not meet the pin here. Empty is met.
fn judge_m10(artifact: Option<&str>, suite: Option<&str>) -> Vec<String> {
    let mut why = Vec::new();
    let Some(a) = artifact else {
        return vec![format!("{M10_ARTIFACT}: missing")];
    };
    for (k, v) in [
        ("mutant", "M10"),
        ("verdict", "met"),
        ("requirement", "PR-16-EXIT"),
        ("artifact", "pr16-exit-m10-same-epoch-commute"),
        ("expected", M10_EXPECTED),
    ] {
        let got = json_field(a, k);
        if got.as_deref() != Some(v) {
            why.push(format!("M10 artifact {k} is {got:?}, pinned {v:?}"));
        }
    }
    if !a.contains("\"seeds_byte_identical\":true") || !a.contains("\"not_met_because\":[]") {
        why.push("M10 artifact is not byte-identical across seeds or has a gap".to_owned());
    }
    match suite {
        None => why.push(format!("{M10_SUITE}: missing")),
        Some(src) if src.contains("#![cfg") => {
            why.push(format!("{M10_SUITE}: compiled only under a condition"));
        }
        Some(src) => {
            for t in M10_TESTS {
                let decl = format!("fn {t}()");
                let Some(at) = src.find(&decl) else {
                    why.push(format!("M10 test {t} is missing"));
                    continue;
                };
                let head = &src[..at];
                let attrs = head.rsplit("\n}\n").next().unwrap_or(head);
                let live = attrs.lines().any(|l| l.trim() == "#[test]");
                if !live || attrs.contains("#[ignore") || attrs.contains("cfg") {
                    why.push(format!("M10 test {t} is not a live test"));
                }
            }
        }
    }
    why
}

// ---------------------------------------------------------------------------
// predicates
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
struct Pred {
    artifact: &'static str,
    id: String,
    pass: bool,
    detail: String,
}

/// The predicates the exit requires: derived from the static tables, not the results.
fn required() -> Vec<(&'static str, String)> {
    let mut r = vec![
        (
            "pr16-exit-hon-01",
            "the mutant table is replicated_register.md's ten".to_owned(),
        ),
        (
            "pr16-exit-hon-01",
            "every pin agrees with IMPL-05's typed expectation".to_owned(),
        ),
        (
            "pr16-exit-hon-01",
            "every mutant has a pinned expectation and a campaign".to_owned(),
        ),
        (
            "pr16-exit-hon-01",
            "every campaign IMPL-05's golden retains is pinned here".to_owned(),
        ),
    ];
    for seed in seeds() {
        for (kind, e) in specs() {
            let art = if matches!(e, Expect::Clean) {
                "pr16-exit-pos-01"
            } else {
                "pr16-exit-pos-02"
            };
            r.push((art, format!("{}@{seed}: pinned verdict met", name_of(kind))));
        }
        r.push((
            "pr16-exit-pos-01",
            format!(
                "correct view@{seed}: no failure, and it counts the baseline's steps and chooses"
            ),
        ));
        r.push((
            "pr16-exit-pos-03",
            format!("M09@{seed}: pinned verdict met"),
        ));
    }
    for id in &Id::ALL[..8] {
        r.push(("pr16-exit-pos-02", format!("{id:?}: killed at every seed")));
    }
    r.push((
        "pr16-exit-pos-04",
        "M10: artifact present, met and pinned as here".to_owned(),
    ));
    for (kind, _) in specs() {
        r.push((
            "pr16-exit-det-01",
            format!(
                "{}: identity equals IMPL-05's retained identity",
                name_of(kind)
            ),
        ));
        if repeated(kind) {
            r.push((
                "pr16-exit-det-01",
                format!(
                    "{}@{}: a repeated run is byte-identical",
                    name_of(kind),
                    EXTRA_SEEDS[0]
                ),
            ));
        }
    }
    r.push((
        "pr16-exit-det-01",
        format!("M09@{}: a repeated run is byte-identical", EXTRA_SEEDS[0]),
    ));
    r.push((
        "pr16-exit-det-01",
        "the extra seeds re-sample the logs and keep the plans".to_owned(),
    ));
    r.push((
        "pr16-exit-bnd-01",
        "every alternative pin of every campaign is not met".to_owned(),
    ));
    r.push((
        "pr16-exit-bnd-01",
        "every alternative M09 pin is not met".to_owned(),
    ));
    r.push((
        "pr16-exit-bnd-01",
        "the step judge classifies crafted steps under each view".to_owned(),
    ));
    r.push((
        "pr16-exit-bnd-02",
        "refusal-only, unbuildable and unchanged-plan outcomes are not met".to_owned(),
    ));
    r.push((
        "pr16-exit-bnd-02",
        "a missing, not-met or differently pinned M10 artifact is not met".to_owned(),
    ));
    r.push((
        "pr16-exit-bnd-02",
        "a run with a refusal beside a finding is classified, and not met".to_owned(),
    ));
    r
}

/// Every alternative of `e`: other kinds, other property sets, symptoms, seconds and
/// refusals.
fn alternatives(e: &Expect) -> Vec<Expect> {
    let props = [
        Property::Agreement,
        Property::RuntimeToAbstract,
        Property::Quiescence,
        Property::ObligationConservation,
    ];
    let syms = [
        "AckedNotDurable",
        "Agreement",
        "Refinement/Unprojectable",
        "Refinement/NotAnEnabledChoose",
        "Quiescence/TaskLive",
    ];
    let refusals = [
        "Refused/ReservationDropped",
        "Refused/TaskEnded",
        "Refused/TaskCrashed",
    ];
    let mut out = vec![
        Expect::Clean,
        Expect::ExcludeTimer(TimerFate::Cancelled),
        Expect::ExcludeTimer(TimerFate::Fenced),
    ];
    out.extend(refusals.iter().map(|r| Expect::ExcludeRefusal(r)));
    for p in props {
        for s in syms {
            out.push(kill(&[p], (p, s), None, None));
        }
    }
    if let Expect::Kill {
        properties,
        primary,
        also,
        refusal,
    } = e
    {
        for s in syms {
            out.push(kill(properties, (primary.0, s), *also, *refusal));
        }
        for p in props {
            let mut more = properties.clone();
            if !more.contains(&p) {
                more.push(p);
                out.push(kill(&more, *primary, *also, *refusal));
            }
            let fewer: Vec<Property> = properties.iter().copied().filter(|q| *q != p).collect();
            if fewer.len() < properties.len() && !fewer.is_empty() {
                out.push(kill(&fewer, *primary, *also, *refusal));
            }
        }
        for r in refusals {
            if Some(r) != *refusal {
                out.push(kill(properties, *primary, *also, Some(r)));
            }
        }
        if refusal.is_some() {
            out.push(kill(properties, *primary, *also, None));
        }
        if also.is_some() {
            out.push(kill(
                properties,
                *primary,
                Some((Property::ObligationConservation, "Conservation/x")),
                *refusal,
            ));
        }
    }
    out.retain(|a| a != e);
    out
}

fn compute_predicates() -> Vec<Pred> {
    let a = all();
    let mut p: Vec<Pred> = Vec::new();
    let mut add = |artifact: &'static str, id: String, pass: bool, detail: String| {
        p.push(Pred {
            artifact,
            id,
            pass,
            detail,
        });
    };
    let specs = specs();

    // --- hon-01
    let md = repo("notes/plan/examples/replicated_register.md");
    let rows: Vec<(String, String)> = md
        .lines()
        .filter_map(|l| {
            let c: Vec<&str> = l.split('|').map(str::trim).collect();
            (c.len() == 4 && c[1].starts_with('M') && c[1].len() == 3)
                .then(|| (c[1].to_owned(), c[2].to_owned()))
        })
        .collect();
    let ours: Vec<(String, String)> = Id::ALL
        .iter()
        .map(|id| (format!("{id:?}"), id.defect().to_owned()))
        .collect();
    add(
        "pr16-exit-hon-01",
        "the mutant table is replicated_register.md's ten".to_owned(),
        rows == ours && rows.len() == 10,
        format!("{} rows", rows.len()),
    );
    let mut disagree = Vec::new();
    for (kind, e) in &specs {
        let theirs = match kind {
            Kind::Plain(id) => Some(mutants::expected(*id)),
            Kind::FailStop(id) => mutants::expected_fail_stop(*id),
            Kind::Carried(id, _) => mutants::expected_carried(*id),
            _ => None,
        };
        let agrees = match (e, theirs) {
            (Expect::Clean, None) => true,
            (
                Expect::Kill {
                    primary,
                    also,
                    refusal,
                    ..
                },
                Some(mutants::Expected::Detected {
                    symptom,
                    also: their_also,
                    refusal: their_refusal,
                    ..
                }),
            ) => {
                *primary == (symptom.property(), their_token(symptom))
                    && *also == their_also.map(|s| (s.property(), their_token(s)))
                    && refusal.map(str::to_owned) == their_refusal.map(|r| format!("Refused/{r:?}"))
            }
            (Expect::ExcludeRefusal(r), Some(mutants::Expected::Excluded { exclusion, .. })) => {
                matches!(
                    (*r, exclusion),
                    ("Refused/TaskEnded", mutants::Exclusion::EpochFenced)
                        | (
                            "Refused/TaskCrashed",
                            mutants::Exclusion::EpochFencedByCrash
                        )
                )
            }
            (Expect::ExcludeTimer(f), Some(mutants::Expected::Excluded { exclusion, .. })) => {
                matches!(
                    (f, exclusion),
                    (TimerFate::Cancelled, mutants::Exclusion::TimerDropped)
                        | (TimerFate::Fenced, mutants::Exclusion::TimerFenced)
                )
            }
            _ => false,
        };
        if !agrees {
            disagree.push(name_of(*kind));
        }
    }
    add(
        "pr16-exit-hon-01",
        "every pin agrees with IMPL-05's typed expectation".to_owned(),
        disagree.is_empty(),
        format!("{} pins; disagreeing {disagree:?}", specs.len()),
    );
    let req = required();
    let mut uncovered = Vec::new();
    for id in Id::ALL {
        let covered = match id {
            Id::M09 => req
                .iter()
                .any(|(a, i)| *a == "pr16-exit-pos-03" && i.starts_with("M09@")),
            Id::M10 => req
                .iter()
                .any(|(a, i)| *a == "pr16-exit-pos-04" && i.starts_with("M10:")),
            _ => specs
                .iter()
                .any(|(k, e)| mutant_of(*k) == Some(id) && matches!(e, Expect::Kill { .. })),
        };
        if !covered {
            uncovered.push(id);
        }
    }
    add(
        "pr16-exit-hon-01",
        "every mutant has a pinned expectation and a campaign".to_owned(),
        uncovered.is_empty(),
        format!(
            "M01-M08: a kill pin on a runtime campaign here; M09: the view campaign here; M10: the reducer's campaign, read from its artifact; uncovered {uncovered:?}"
        ),
    );
    let pinned: BTreeSet<String> = specs.iter().map(|(k, _)| name_of(*k)).collect();
    let golden: BTreeSet<String> = retained().into_keys().collect();
    add(
        "pr16-exit-hon-01",
        "every campaign IMPL-05's golden retains is pinned here".to_owned(),
        pinned == golden,
        format!(
            "{} pinned, {} retained; only pinned {:?}; only retained {:?}",
            pinned.len(),
            golden.len(),
            pinned.difference(&golden).collect::<Vec<_>>(),
            golden.difference(&pinned).collect::<Vec<_>>()
        ),
    );

    // --- pos-01, pos-02, pos-03 per seed
    for seed in seeds() {
        for (kind, e) in &specs {
            let s = &a.shown[&(*kind, seed)];
            let why = judge(e, s);
            let art = if matches!(e, Expect::Clean) {
                "pr16-exit-pos-01"
            } else {
                "pr16-exit-pos-02"
            };
            add(
                art,
                format!("{}@{seed}: pinned verdict met", name_of(*kind)),
                why.is_empty(),
                if why.is_empty() {
                    format!(
                        "{} runs, {} changed, {} detecting, {} inconclusive (never counted); properties {:?}; symptoms {:?}; witness {:?}",
                        s.runs,
                        s.changed_runs,
                        s.detecting_runs,
                        s.inconclusive_runs,
                        s.properties,
                        s.symptoms,
                        s.witness
                    )
                } else {
                    why.join("; ")
                },
            );
        }
        let v = &a.views[&seed];
        let (steps, chooses) = a.correspondence[&seed];
        let correct = judge_correct_view(v, steps, chooses);
        add(
            "pr16-exit-pos-01",
            format!(
                "correct view@{seed}: no failure, and it counts the baseline's steps and chooses"
            ),
            correct.is_empty(),
            if correct.is_empty() {
                format!("{} steps, {} chooses", v.correct.steps, v.correct.chooses)
            } else {
                correct.join("; ")
            },
        );
        let why = judge_mutant_view(v);
        add(
            "pr16-exit-pos-03",
            format!("M09@{seed}: pinned verdict met"),
            why.is_empty(),
            if why.is_empty() {
                format!(
                    "{} runs; failures {:?}; failing runs {}; runs whose abstract history gives an epoch two values {}; shallowest {:?}; control {:?}",
                    v.runs,
                    v.mutant.at,
                    v.mutant.failing_runs,
                    v.mutant.agreement_runs,
                    v.mutant.shallowest.as_ref().map(|s| &s.0),
                    v.control.at
                )
            } else {
                why.join("; ")
            },
        );
    }
    for id in &Id::ALL[..8] {
        let mut killed_at = Vec::new();
        for seed in seeds() {
            let k = specs.iter().any(|(kind, e)| {
                mutant_of(*kind) == Some(*id)
                    && matches!(e, Expect::Kill { .. })
                    && judge(e, &a.shown[&(*kind, seed)]).is_empty()
            });
            let all_met = specs
                .iter()
                .filter(|(kind, _)| mutant_of(*kind) == Some(*id))
                .all(|(kind, e)| judge(e, &a.shown[&(*kind, seed)]).is_empty());
            if k && all_met {
                killed_at.push(seed);
            }
        }
        add(
            "pr16-exit-pos-02",
            format!("{id:?}: killed at every seed"),
            killed_at == seeds(),
            format!("killed at {killed_at:?}"),
        );
    }
    let m10 = judge_m10(
        std::fs::read_to_string(format!(
            "{}/../../{M10_ARTIFACT}",
            env!("CARGO_MANIFEST_DIR")
        ))
        .ok()
        .as_deref(),
        std::fs::read_to_string(format!("{}/../../{M10_SUITE}", env!("CARGO_MANIFEST_DIR")))
            .ok()
            .as_deref(),
    );
    add(
        "pr16-exit-pos-04",
        "M10: artifact present, met and pinned as here".to_owned(),
        m10.is_empty(),
        if m10.is_empty() {
            format!("{M10_ARTIFACT}: met; tests {M10_TESTS:?} live")
        } else {
            m10.join("; ")
        },
    );

    // --- det-01
    let ret = retained();
    for (kind, _) in &specs {
        let name = name_of(*kind);
        let got = &a.shown[&(*kind, SEED)].identity;
        add(
            "pr16-exit-det-01",
            format!("{name}: identity equals IMPL-05's retained identity"),
            ret.get(&name) == Some(got),
            got.to_string(),
        );
        if repeated(*kind) {
            // The whole record, verdict inputs included, not only the journal digest.
            let first = &a.shown[&(*kind, EXTRA_SEEDS[0])];
            add(
                "pr16-exit-det-01",
                format!(
                    "{name}@{}: a repeated run is byte-identical",
                    EXTRA_SEEDS[0]
                ),
                a.repeats.get(kind) == Some(first),
                first.identity.clone(),
            );
        }
    }
    add(
        "pr16-exit-det-01",
        format!("M09@{}: a repeated run is byte-identical", EXTRA_SEEDS[0]),
        a.view_repeat.as_ref() == Some(&a.views[&EXTRA_SEEDS[0]]),
        a.views[&EXTRA_SEEDS[0]].identity.clone(),
    );
    // A seed draws the sampled plans' logs; the plans, the two-epoch sample included,
    // are fixed by the scenario seed. Compared on the logs alone, without the seed.
    let logs: BTreeSet<&String> = a.views.values().map(|v| &v.logs_digest).collect();
    let plans: BTreeSet<&String> = a.views.values().map(|v| &v.plans_digest).collect();
    let sampled: BTreeSet<usize> = a.views.values().map(|v| v.sampled_plans).collect();
    add(
        "pr16-exit-det-01",
        "the extra seeds re-sample the logs and keep the plans".to_owned(),
        logs.len() == seeds().len() && plans.len() == 1 && sampled.iter().all(|n| *n > 0),
        format!(
            "seeds {:?}: {} distinct log corpora, {} plan set, sampled plans {sampled:?}; the other plans run every admissible log at every seed",
            seeds(),
            logs.len(),
            plans.len()
        ),
    );

    // --- bnd-01
    let mut kept = Vec::new();
    let mut tried = 0;
    for (kind, e) in &specs {
        let s = &a.shown[&(*kind, SEED)];
        for alt in alternatives(e) {
            tried += 1;
            if judge(&alt, s).is_empty() {
                kept.push(format!("{}: {}", name_of(*kind), alt.render()));
            }
        }
    }
    add(
        "pr16-exit-bnd-01",
        "every alternative pin of every campaign is not met".to_owned(),
        kept.is_empty() && tried > 0,
        format!("{tried} alternatives tried; met: {kept:?}"),
    );
    let v = &a.views[&SEED];
    let (steps, chooses) = a.correspondence[&SEED];
    let mut alts = 0;
    let mut kept_v = Vec::new();
    // Mis-pins of the view campaign: another shallowest kind; a pinned failure missing
    // or another added; the durable-majority control's failures, or the correct view's
    // none, in place of M09's; the control withdrawing; a correct view that fails or is
    // off the baseline's counts; an inconclusive run; no two-value history; a witness
    // that does not replay.
    let mut doctored: Vec<ViewCampaign> = Vec::new();
    let mut d = v.clone();
    if let Some(sh) = d.mutant.shallowest.as_mut() {
        sh.0 = "View/Withdrawn".to_owned();
    }
    doctored.push(d);
    let mut d = v.clone();
    d.mutant.at.remove(M09_AT[1]);
    doctored.push(d);
    let mut d = v.clone();
    d.mutant
        .at
        .insert("View/ChooseOffQuorum at Sync/durable".to_owned(), 1);
    doctored.push(d);
    let mut d = v.clone();
    d.mutant = d.control.clone();
    doctored.push(d);
    let mut d = v.clone();
    d.mutant = d.correct.clone();
    doctored.push(d);
    let mut d = v.clone();
    d.control.at.insert(M09_AT[1].to_owned(), 1);
    doctored.push(d);
    let mut d = v.clone();
    d.correct.failures.insert("View/Withdrawn".to_owned(), 1);
    doctored.push(d);
    let mut d = v.clone();
    d.correct.chooses += 1;
    doctored.push(d);
    let mut d = v.clone();
    d.inconclusive = 1;
    doctored.push(d);
    let mut d = v.clone();
    d.replayed = false;
    doctored.push(d);
    let mut d = v.clone();
    d.mutant.agreement_runs = 0;
    doctored.push(d);
    for d in &doctored {
        alts += 1;
        if judge_mutant_view(d).is_empty() && judge_correct_view(d, steps, chooses).is_empty() {
            kept_v.push(alts);
        }
    }
    add(
        "pr16-exit-bnd-01",
        "every alternative M09 pin is not met".to_owned(),
        kept_v.is_empty(),
        format!("{alts} doctored view results; met: {kept_v:?}"),
    );
    let crafted = crafted_steps();
    let wrong: Vec<&String> = crafted
        .iter()
        .filter(|(_, ok)| !ok)
        .map(|(w, _)| w)
        .collect();
    add(
        "pr16-exit-bnd-01",
        "the step judge classifies crafted steps under each view".to_owned(),
        wrong.is_empty() && !crafted.is_empty(),
        format!("{} crafted steps; misclassified {wrong:?}", crafted.len()),
    );

    // --- bnd-02
    let (kind, e) = specs
        .iter()
        .find(|(k, _)| *k == Kind::Plain(Id::M01))
        .expect("M01")
        .clone();
    let s = &a.shown[&(kind, SEED)];
    let mut bad = Vec::new();
    // Only refusals: every detecting run turned into a refused run.
    let mut r = s.clone();
    r.properties.clear();
    r.symptoms = [("Refused/TaskEnded".to_owned(), r.detecting_runs)].into();
    r.inconclusive_runs += r.detecting_runs;
    r.detecting_runs = 0;
    r.shallowest.clear();
    r.witness = None;
    if judge(&e, &r).is_empty() {
        bad.push("refusal-only");
    }
    let mut r = s.clone();
    *r.symptoms.entry("Unbuildable".to_owned()).or_default() += 1;
    if judge(&e, &r).is_empty() {
        bad.push("unbuildable");
    }
    let mut r = s.clone();
    *r.symptoms.entry("LiftNot".to_owned()).or_default() += 1;
    if judge(&e, &r).is_empty() {
        bad.push("not lifted");
    }
    let mut r = s.clone();
    r.unchanged_findings = 1;
    if judge(&e, &r).is_empty() {
        bad.push("unchanged-plan finding");
    }
    let mut r = s.clone();
    r.runs = 0;
    if judge(&e, &r).is_empty() {
        bad.push("no run");
    }
    add(
        "pr16-exit-bnd-02",
        "refusal-only, unbuildable and unchanged-plan outcomes are not met".to_owned(),
        bad.is_empty(),
        format!("met anyway: {bad:?}"),
    );
    let art = std::fs::read_to_string(format!(
        "{}/../../{M10_ARTIFACT}",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap_or_default();
    let suite =
        std::fs::read_to_string(format!("{}/../../{M10_SUITE}", env!("CARGO_MANIFEST_DIR")))
            .unwrap_or_default();
    let mut m10_kept = Vec::new();
    let cases: Vec<(&str, Option<String>, Option<String>)> = vec![
        ("missing", None, Some(suite.clone())),
        (
            "not met",
            Some(art.replace("\"verdict\":\"met\"", "\"verdict\":\"not-met\"")),
            Some(suite.clone()),
        ),
        (
            "re-pinned",
            Some(art.replace("refuted (disables)", "refuted (different-states)")),
            Some(suite.clone()),
        ),
        (
            "a gap",
            Some(art.replace("\"not_met_because\":[]", "\"not_met_because\":[\"x\"]")),
            Some(suite.clone()),
        ),
        (
            "seeds differ",
            Some(art.replace(
                "\"seeds_byte_identical\":true",
                "\"seeds_byte_identical\":false",
            )),
            Some(suite.clone()),
        ),
        ("suite missing", Some(art.clone()), None),
        (
            "a test ignored",
            Some(art.clone()),
            Some(suite.replacen(
                "#[test]\nfn m10_the_artifact_is_byte_stable",
                "#[test]\n#[ignore]\nfn m10_the_artifact_is_byte_stable",
                1,
            )),
        ),
        (
            "a test compiled out",
            Some(art.clone()),
            Some(suite.replacen(
                "#[test]\nfn m10_the_artifact_is_byte_stable",
                "#[cfg(any())]\n#[test]\nfn m10_the_artifact_is_byte_stable",
                1,
            )),
        ),
        (
            "a test commented out",
            Some(art.clone()),
            Some(suite.replacen(
                "#[test]\nfn m10_the_artifact_is_byte_stable",
                "// #[test]\nfn m10_the_artifact_is_byte_stable",
                1,
            )),
        ),
        (
            "a test renamed",
            Some(art.clone()),
            Some(suite.replace(
                "fn m10_the_campaign_is_deterministic_across_seeds",
                "fn renamed",
            )),
        ),
    ];
    for (what, a_, s_) in &cases {
        if judge_m10(a_.as_deref(), s_.as_deref()).is_empty() {
            m10_kept.push(*what);
        }
    }
    add(
        "pr16-exit-bnd-02",
        "a missing, not-met or differently pinned M10 artifact is not met".to_owned(),
        m10_kept.is_empty() && judge_m10(Some(&art), Some(&suite)).is_empty(),
        format!("{} doctored cases; met anyway: {m10_kept:?}", cases.len()),
    );
    // A run with a refusal beside a property finding: `show` counts it as detecting and
    // records the refusal, and the kill pin, which allows no refusal, is not met.
    let base = base_at(SEED);
    let m = job(kind, &base);
    let mut o = Outcome {
        runs: 1,
        ..Outcome::default()
    };
    o.plans.push(baseline::PlanOutcome {
        group: "scenario",
        index: 0,
        scope: baseline::Scope::Exhaustive,
        runs: 1,
        findings: vec![(
            0,
            vec![
                Finding::Refused(
                    continuum_asupersync::binding::BindingRefusal::TaskEnded { task: 3 }
                        .to_string(),
                ),
                Finding::AckedNotDurable(9),
            ],
        )],
    });
    let mixed = show(kind, &e, &m, &base, &o);
    add(
        "pr16-exit-bnd-02",
        "a run with a refusal beside a finding is classified, and not met".to_owned(),
        mixed.detecting_runs == 1
            && mixed.inconclusive_runs == 0
            && mixed.symptoms.contains_key("Refused/TaskEnded")
            && !judge(&e, &mixed).is_empty(),
        format!("{:?}; verdict {:?}", mixed.symptoms, judge(&e, &mixed)),
    );
    p
}

/// IMPL-05's symptom, as this file's token.
fn their_token(s: mutants::Symptom) -> &'static str {
    match s {
        mutants::Symptom::AckedNotDurable => "AckedNotDurable",
        mutants::Symptom::Unprojectable => "Refinement/Unprojectable",
        mutants::Symptom::CoordinatorLive => "Quiescence/TaskLive",
        mutants::Symptom::Agreement => "Agreement",
    }
}

fn predicates() -> &'static [Pred] {
    static CELL: OnceLock<Vec<Pred>> = OnceLock::new();
    CELL.get_or_init(compute_predicates)
}

/// The exit's machine verdict: every required predicate present exactly once and
/// passing, and no predicate the requirement list does not name.
fn aggregate(preds: &[Pred]) -> Result<(), Vec<String>> {
    let mut why = Vec::new();
    let required = required();
    for (artifact, id) in &required {
        let found: Vec<&Pred> = preds
            .iter()
            .filter(|p| p.artifact == *artifact && &p.id == id)
            .collect();
        match found.as_slice() {
            [] => why.push(format!("missing: {artifact} {id}")),
            [one] if one.pass => {}
            [one] => why.push(format!("failing: {artifact} {id}: {}", one.detail)),
            _ => why.push(format!("duplicated: {artifact} {id}")),
        }
    }
    for p in preds {
        if !required
            .iter()
            .any(|(a, id)| *a == p.artifact && *id == p.id)
        {
            why.push(format!("not required: {} {}", p.artifact, p.id));
        }
    }
    if why.is_empty() { Ok(()) } else { Err(why) }
}

fn assert_artifact(artifact: &str) {
    let preds = predicates();
    let required = required();
    assert!(
        required.iter().any(|(a, _)| *a == artifact),
        "{artifact} requires nothing"
    );
    for (a, id) in required.iter().filter(|(a, _)| *a == artifact) {
        let p = preds
            .iter()
            .find(|p| p.artifact == *a && &p.id == id)
            .unwrap_or_else(|| panic!("{a}: missing {id}"));
        assert!(p.pass, "{a}: {id}: {}", p.detail);
    }
}

// ---------------------------------------------------------------------------
// tests
// ---------------------------------------------------------------------------

/// `pr16-exit-hon-01`.
#[test]
fn hon_01_the_mutant_set_and_its_pins() {
    assert_artifact("pr16-exit-hon-01");
}

/// `pr16-exit-pos-01`.
#[test]
fn pos_01_the_correct_implementation_satisfies_the_properties() {
    assert_artifact("pr16-exit-pos-01");
}

/// `pr16-exit-pos-02`.
#[test]
fn pos_02_each_program_mutant_reaches_its_pinned_verdict() {
    assert_artifact("pr16-exit-pos-02");
}

/// `pr16-exit-pos-03`.
#[test]
fn pos_03_the_view_mutant_is_killed() {
    assert_artifact("pr16-exit-pos-03");
}

/// `pr16-exit-pos-04`.
#[test]
fn pos_04_the_independence_mutant_is_killed() {
    assert_artifact("pr16-exit-pos-04");
}

/// `pr16-exit-det-01`.
#[test]
fn det_01_every_campaign_is_deterministic() {
    assert_artifact("pr16-exit-det-01");
}

/// `pr16-exit-bnd-01`.
#[test]
fn bnd_01_a_mis_pinned_expectation_is_detected() {
    assert_artifact("pr16-exit-bnd-01");
}

/// `pr16-exit-bnd-02`.
#[test]
fn bnd_02_inconclusive_results_are_never_a_kill() {
    assert_artifact("pr16-exit-bnd-02");
}

/// `pr16-exit-bnd-03`. The machine verdict is the conjunction of every required
/// predicate: flipping, deleting or duplicating any one, or adding an unrequired one,
/// makes it not met.
#[test]
fn bnd_03_the_machine_verdict_is_the_conjunction_of_every_claimed_check() {
    let real = predicates();
    assert_eq!(aggregate(real), Ok(()));
    assert_eq!(real.len(), required().len());
    for i in 0..real.len() {
        let mut flipped = real.to_vec();
        flipped[i].pass = false;
        assert!(
            aggregate(&flipped).is_err(),
            "flipping {} kept the verdict",
            real[i].id
        );
        let mut deleted = real.to_vec();
        deleted.remove(i);
        assert!(
            aggregate(&deleted).is_err(),
            "deleting {} kept the verdict",
            real[i].id
        );
        let mut duplicated = real.to_vec();
        duplicated.push(real[i].clone());
        assert!(
            aggregate(&duplicated).is_err(),
            "duplicating {} kept the verdict",
            real[i].id
        );
    }
    let mut extra = real.to_vec();
    extra.push(Pred {
        artifact: "pr16-exit-pos-01",
        id: "unnamed".to_owned(),
        pass: true,
        detail: String::new(),
    });
    assert!(aggregate(&extra).is_err());
    let mut flipped = real.to_vec();
    flipped[0].pass = false;
    assert!(render_json(&flipped).contains("\"verdict\":\"not-met\""));
    assert!(render_json(real).contains("\"verdict\":\"met\""));
}

// ---------------------------------------------------------------------------
// the summary
// ---------------------------------------------------------------------------

enum Json {
    Str(String),
    Num(u64),
    Bool(bool),
    Arr(Vec<Json>),
    Obj(BTreeMap<String, Json>),
}

impl Json {
    fn render(&self, out: &mut String) {
        match self {
            Self::Str(s) => {
                out.push('"');
                for c in s.chars() {
                    match c {
                        '"' => out.push_str("\\\""),
                        '\\' => out.push_str("\\\\"),
                        '\n' => out.push_str("\\n"),
                        c if u32::from(c) < 0x20 => {
                            let _ = write!(out, "\\u{:04x}", u32::from(c));
                        }
                        c => out.push(c),
                    }
                }
                out.push('"');
            }
            Self::Num(n) => {
                let _ = write!(out, "{n}");
            }
            Self::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
            Self::Arr(items) => {
                out.push('[');
                for (i, item) in items.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    item.render(out);
                }
                out.push(']');
            }
            Self::Obj(members) => {
                out.push('{');
                for (i, (k, v)) in members.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    Self::Str(k.clone()).render(out);
                    out.push(':');
                    v.render(out);
                }
                out.push('}');
            }
        }
    }
}

fn obj(members: Vec<(&str, Json)>) -> Json {
    Json::Obj(
        members
            .into_iter()
            .map(|(k, v)| (k.to_owned(), v))
            .collect(),
    )
}

fn s(text: &str) -> Json {
    Json::Str(text.to_owned())
}

fn n(v: usize) -> Json {
    Json::Num(u64::try_from(v).expect("fits"))
}

fn campaign_json(kind: Kind, e: &Expect) -> Json {
    let a = all();
    let per_seed: Vec<Json> = seeds()
        .iter()
        .map(|seed| {
            let sh = &a.shown[&(kind, *seed)];
            obj(vec![
                ("seed", Json::Num(*seed)),
                ("identity", s(&sh.identity)),
                ("plans", n(sh.plans)),
                ("runs", n(sh.runs)),
                ("changed_plans", n(sh.changed_plans)),
                ("changed_runs", n(sh.changed_runs)),
                ("detecting_runs", n(sh.detecting_runs)),
                ("inconclusive_runs_never_counted", n(sh.inconclusive_runs)),
                (
                    "properties",
                    Json::Obj(
                        sh.properties
                            .iter()
                            .map(|(p, c)| (prop_name(*p), n(*c)))
                            .collect(),
                    ),
                ),
                (
                    "runs_by_symptom",
                    Json::Obj(
                        sh.symptoms
                            .iter()
                            .map(|(k, c)| (k.clone(), n(*c)))
                            .collect(),
                    ),
                ),
                (
                    "witness",
                    sh.witness
                        .as_ref()
                        .map_or(s("none"), |(w, r)| s(&format!("{w}; replayed {r}"))),
                ),
                ("met", Json::Bool(judge(e, sh).is_empty())),
            ])
        })
        .collect();
    obj(vec![
        ("campaign", s(&name_of(kind))),
        ("expected", s(&e.render())),
        ("seeds", Json::Arr(per_seed)),
    ])
}

fn render_json(preds: &[Pred]) -> String {
    let a = all();
    let verdict = aggregate(preds);
    let specs = specs();
    let mut mutants_json = Vec::new();
    for id in Id::ALL {
        let mut members = vec![("id", s(&format!("{id:?}"))), ("defect", s(id.defect()))];
        match id {
            Id::M09 => {
                members.push(("expected", s(M09_EXPECTED)));
                members.push((
                    "campaign",
                    s("pr16-exit-mut-09-submitted-as-chosen: pr16-correct-baseline's runs, each journal projected, judged under PR 16's refinement map and under M09's view"),
                ));
                members.push((
                    "seeds",
                    Json::Arr(
                        seeds()
                            .iter()
                            .map(|seed| {
                                let v = &a.views[seed];
                                obj(vec![
                                    ("seed", Json::Num(*seed)),
                                    ("identity", s(&v.identity)),
                                    ("runs", n(v.runs)),
                                    ("inconclusive_runs_never_counted", n(v.inconclusive)),
                                    ("correct_view_steps", n(v.correct.steps)),
                                    ("correct_view_chooses", n(v.correct.chooses)),
                                    ("correct_view_failures", n(v.correct.failures.values().sum())),
                                    (
                                        "mutant_view_failures",
                                        Json::Obj(v.mutant.failures.iter().map(|(k, c)| (k.clone(), n(*c))).collect()),
                                    ),
                                    (
                                        "mutant_view_failures_at",
                                        Json::Obj(
                                            v.mutant
                                                .at
                                                .iter()
                                                .map(|(k, c)| (k.clone(), n(*c)))
                                                .collect(),
                                        ),
                                    ),
                                    (
                                        "control_view_failures_at",
                                        Json::Obj(
                                            v.control
                                                .at
                                                .iter()
                                                .map(|(k, c)| (k.clone(), n(*c)))
                                                .collect(),
                                        ),
                                    ),
                                    ("control_view_runs_with_two_values_in_one_epoch_of_history", n(v.control.agreement_runs)),
                                    ("sampled_plans", n(v.sampled_plans)),
                                    ("logs_digest", s(&v.logs_digest)),
                                    ("mutant_view_failing_runs", n(v.mutant.failing_runs)),
                                    ("mutant_view_runs_with_two_values_in_one_epoch_of_history", n(v.mutant.agreement_runs)),
                                    (
                                        "witness",
                                        s(&v.mutant.shallowest.as_ref().map_or("none".to_owned(), |(k, g, p, l, q)| {
                                            format!("{k} at group {g} plan {p} log {l} event {q}; replayed {}", v.replayed)
                                        })),
                                    ),
                                ])
                            })
                            .collect(),
                    ),
                ));
            }
            Id::M10 => {
                members.push(("expected", s(M10_EXPECTED)));
                members.push(("campaign", s(M10_SUITE)));
                members.push(("artifact", s(M10_ARTIFACT)));
                let art = std::fs::read(format!(
                    "{}/../../{M10_ARTIFACT}",
                    env!("CARGO_MANIFEST_DIR")
                ))
                .unwrap_or_default();
                members.push(("artifact_blake3", s(&Blake3Hasher::hash(&art).to_string())));
            }
            _ => {
                members.push((
                    "campaigns",
                    Json::Arr(
                        specs
                            .iter()
                            .filter(|(k, _)| mutant_of(*k) == Some(id))
                            .map(|(k, e)| campaign_json(*k, e))
                            .collect(),
                    ),
                ));
            }
        }
        mutants_json.push(obj(members));
    }
    let correct: Vec<Json> = specs
        .iter()
        .filter(|(_, e)| matches!(e, Expect::Clean))
        .map(|(k, e)| campaign_json(*k, e))
        .collect();
    let doc = obj(vec![
        ("requirement", s("PR-16-EXIT")),
        (
            "exit",
            s("each mutant has an expected intent/property and deterministic campaign"),
        ),
        ("bone", s("bn-2cgl")),
        (
            "suite",
            s("crates/continuum-asupersync/tests/pr16_exit_evidence.rs"),
        ),
        (
            "verdict",
            s(if verdict.is_ok() { "met" } else { "not-met" }),
        ),
        (
            "verdict_rule",
            s(
                "met only when every required predicate is present exactly once and passes, and no unrequired predicate appears; a refused, unbuildable, unlifted or unprojected run is inconclusive and never counted as a kill; an exclusion is typed and never a kill",
            ),
        ),
        (
            "not_met_because",
            Json::Arr(
                verdict
                    .err()
                    .unwrap_or_default()
                    .iter()
                    .map(|w| s(w))
                    .collect(),
            ),
        ),
        (
            "seeds",
            Json::Arr(seeds().iter().map(|x| Json::Num(*x)).collect()),
        ),
        ("mutants", Json::Arr(mutants_json)),
        ("correct", Json::Arr(correct)),
        (
            "predicates",
            Json::Arr(
                preds
                    .iter()
                    .map(|p| {
                        obj(vec![
                            ("artifact", s(p.artifact)),
                            ("id", s(&p.id)),
                            ("pass", Json::Bool(p.pass)),
                            ("detail", s(&p.detail)),
                        ])
                    })
                    .collect(),
            ),
        ),
        (
            "narrowing",
            Json::Arr(vec![
                s(
                    "the explored runs only, most logs sampled from each seed; not DPOR; witnesses are the shallowest failing runs, replayed, not minimized (PR 18)",
                ),
                s(
                    "M03 and M08 are excluded by task identity in their plain and fail-stop campaigns and killed only against a process epoch carried in a message or a timer payload (bn-2faf1)",
                ),
                s(
                    "M09 is run against PR 16's refinement map on the baseline's runtime runs; PR 17's CIR projection (bn-2et) and the check that each mutant fails at its mapped transition (bn-1mm) are not this campaign",
                ),
                s(
                    "M10 is run against the dossier's abstract register and the one-epoch durable register; no reduction is run under the mutated rule, and the reducer declares no pair of either subject independent",
                ),
                s(
                    "crash semantics as each campaign states: graceful region cancellation unless named fail-stop",
                ),
            ]),
        ),
    ]);
    let mut out = String::new();
    doc.render(&mut out);
    out.push('\n');
    out
}

/// The retained summary. It pins every campaign's identity at every seed, so a second
/// process that reproduces it reproduces every campaign byte for byte.
#[test]
fn the_exit_summary_is_byte_stable() {
    const JSON: &str = include_str!("evidence/pr16-exit.json");
    let json = render_json(predicates());
    if std::env::var_os("PR16_EXIT_BLESS").is_some() {
        let root = env!("CARGO_MANIFEST_DIR");
        std::fs::write(format!("{root}/tests/evidence/pr16-exit.json"), &json)
            .expect("the summary is writable");
        panic!(
            "PR16_EXIT_BLESS rewrote the summary; rerun without it and review the diff as a contract change"
        );
    }
    assert!(
        json.contains("\"verdict\":\"met\""),
        "the exit is not met:\n{json}"
    );
    assert_eq!(
        json, JSON,
        "the exit summary drifted; regenerate with PR16_EXIT_BLESS=1 and review the diff"
    );
}
