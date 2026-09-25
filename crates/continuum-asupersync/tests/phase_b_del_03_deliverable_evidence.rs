//! Deliverable-evidence layer for `PHASE-B-DEL-03` — storage/network/process packs
//! (`notes/plan/plan.md`, Phase B deliverable list, the "storage/network/process packs"
//! bullet; owned by bn-1e40).
//!
//! bn-1e40 is the phase-level integration boundary over PR 15 (bn-1nn, done), which is
//! not represented by any single PR-grain bone. It does not re-implement or
//! re-simulate anything the PR-15 campaign already proved: `pr15_impl01_network.rs`,
//! `pr15_impl02_process.rs`, `pr15_impl03_storage.rs`, `pr15_impl04_*.rs` and
//! `pr15_exit_evidence.rs` (bn-jw7q, `PR-15-EXIT`, reviewed at cr-1j398p) stay
//! untouched. This file instead answers four questions mechanically, once, in one
//! place, and retains its own machine-readable and golden artifacts:
//!
//! 1. **Presence and declaration** (`phase-b-del-03-reg-{01,02,03}`): is each of the
//!    network, process and storage packs actually present as a real dependency edge of
//!    this crate's tests, does it declare exactly its two frozen fidelity profiles, does
//!    its own `CANCELLATION_CONTRACT` text declare its window rows, and does every
//!    profile it declares claim no host semantics and no independence relation (the
//!    self-declared half of T06/T07, read from the pack's own canonical bytes, not
//!    asserted by this file).
//! 2. **PR-15 exit evidence integrity** (`phase-b-del-03-evi-{01,02}`): do the retained
//!    `pr15_exit_evidence.rs`, `golden/pr15_exit_evidence.txt` and
//!    `evidence/pr15-exit.json` exist, is the suite free of `#[ignore]` (so none of its
//!    17 tests are compiled out of the default run), does the retained summary say
//!    `met`/`fail_closed`, and is this file's own retained record digest-bound to the
//!    *live* bytes of the golden and the JSON (BLAKE3, `continuum_value::identity`, the
//!    workspace's own digest seam) — so an edit to either retained artifact that is not
//!    matched by a re-bless of *this* file's golden is a byte-level mismatch here, not a
//!    silent pass.
//! 3. **Cross-component acceptance** (`phase-b-del-03-xc-{01,02,03}`): reusing the exit
//!    suite's own binary as a subprocess (the same pattern `pr15_no_std_lane.rs` uses to
//!    shell out to `cargo`), this file re-runs, live, the five PR-15-EXIT tests that
//!    exercise the packs *together*: `pos_04` (process and storage crash windows
//!    composed at one boundary, network journal proved unchanged, moves across network
//!    steps proved to commute — network, process and storage in one run), `pos_05` and
//!    `pos_06` (the register's real binding, both crash semantics, driving the packs
//!    through the actual runtime rather than the Lab handlers alone), and `neg_01` /
//!    `neg_02` (every perturbation of those same windows is a typed divergence, verified
//!    equivalent, or absent — never a silently accepted replay). A live `ok` for those
//!    five is the acceptance evidence; it is not read off the frozen JSON.
//! 4. **The pack independence rule, referenced honestly** (`phase-b-del-03-t07-01`): T06
//!    (bn-22sa), T07 (bn-3uva) and R10 (bn-2a0v) are **open**, not delivered — `bn show`
//!    on all three returns `state: open` as of 2026-09-25. Their required controls
//!    (sandboxing, dynamic footprint validation, baseline cross-check, first-party
//!    review for certified mode, pack signature/provenance for T07; the parallel T06
//!    list) are not implemented anywhere in this workspace, and this file does not claim
//!    they are. What *is* true today, and is checked here mechanically rather than
//!    quoted from prose, is narrower: every profile every pack declares carries
//!    `host: HostQualification::None` (no host claim exists to falsify) and
//!    `independence: IndependenceClaim::AllDependent` (no independence relation exists
//!    to subvert) — the network profile module's own doc comment says plainly "docs/09
//!    T07 has no rule to subvert" on that basis. That is a necessary precondition the
//!    packs' own data states, not an enforcement mechanism, and this file's `t07-01`
//!    predicate says exactly that, both ways.
//!
//! # What this file is not
//!
//! It is not a second exit layer over PR 15 — bn-jw7q already is that, and this file
//! cites it rather than re-deriving its window classification, its 114 predicates, or
//! its 8,280-plan-scale corpora. It runs a five-test slice of that suite as a live
//! subprocess for leg 3, and it reads (never re-computes) the frozen counts in
//! `pr15-exit.json` nowhere at all — the digest binding in leg 2 is enough to know
//! those bytes have not moved since bn-jw7q's own review. It does not touch `src/` of
//! any crate.
//!
//! # Retained artifacts
//!
//! `golden/phase_b_del_03_deliverable_evidence.txt` (human-readable) and
//! `evidence/phase-b-del-03.json` (machine-readable) are rendered by [`render_golden`]
//! and [`render_json`] from [`predicates`] and checked byte-identical against the committed files
//! ([`the_deliverable_evidence_is_byte_stable`]); regenerate deliberately with
//! `PHASE_B_DEL_03_BLESS=1 cargo test -p continuum-asupersync --test
//! phase_b_del_03_deliverable_evidence` and review the diff, exactly as
//! `PR15_EXIT_BLESS` works for bn-jw7q's own file.
//!
//! # Fail closed
//!
//! [`aggregate`] is the conjunction of every predicate in [`required`]; it is `Ok` only
//! when every required `(artifact, id)` pair is present exactly once and passing.
//! [`bnd_01_the_verdict_is_the_conjunction_of_every_required_predicate`] proves that by
//! construction: flipping, deleting or duplicating any one live predicate, or adding an
//! unrequired one, makes the aggregate `Err`.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::process::Command;
use std::sync::OnceLock;

use continuum_effects_network as net;
use continuum_effects_process as prc;
use continuum_effects_storage as sto;
use continuum_value::identity::{Blake3Hasher, ContentHasher};

// ---------------------------------------------------------------------------
// predicates
// ---------------------------------------------------------------------------

/// One checked claim: which artifact it belongs to, a stable id, whether it holds, and
/// a detail string for the failure message (never load-bearing for `pass` itself).
#[derive(Debug, Clone)]
struct Pred {
    artifact: &'static str,
    id: &'static str,
    pass: bool,
    detail: String,
}

fn pred(artifact: &'static str, id: &'static str, pass: bool, detail: impl Into<String>) -> Pred {
    Pred {
        artifact,
        id,
        pass,
        detail: detail.into(),
    }
}

/// `crates/continuum-asupersync/Cargo.toml`, read live so a dropped dependency edge
/// shows here rather than only breaking the build.
const CARGO_TOML: &str = include_str!("../Cargo.toml");

/// Whether `continuum-effects-<pack>` is a real `[dev-dependencies]` path edge of this
/// crate — the same edge `pr15_exit_evidence.rs` uses, read from the manifest text
/// rather than assumed.
fn is_dev_dependency(pack: &str) -> bool {
    let Some((_, dev_section)) = CARGO_TOML.split_once("[dev-dependencies]") else {
        return false;
    };
    let needle = format!("continuum-effects-{pack} = {{ path = \"../continuum-effects-{pack}\" }}");
    dev_section.contains(&needle)
}

/// Every named row of `contract` is present with non-empty text.
fn contract_declares(contract: &[(&str, &str)], rows: &[&str]) -> bool {
    rows.iter().all(|row| {
        contract
            .iter()
            .any(|(k, v)| k == row && !v.trim().is_empty())
    })
}

// --- leg 1: presence and declaration ----------------------------------------------

fn network_predicates() -> Vec<Pred> {
    const ARTIFACT: &str = "phase-b-del-03-reg-01";
    let profiles = net::profile::PROFILES;
    let names: Vec<&str> = profiles.iter().map(|p| p.name).collect();
    let host_none = profiles
        .iter()
        .all(|p| p.host == net::profile::HostQualification::None);
    let all_dependent = profiles
        .iter()
        .all(|p| p.independence == net::profile::IndependenceClaim::AllDependent);
    vec![
        pred(
            ARTIFACT,
            "the network pack is a real dev-dependency edge of continuum-asupersync",
            is_dev_dependency("network"),
            "crates/continuum-asupersync/Cargo.toml [dev-dependencies]",
        ),
        pred(
            ARTIFACT,
            "the network pack declares exactly its two frozen profiles, -v0 and -v1",
            profiles.len() == 2 && names[0].ends_with("-v0") && names[1].ends_with("-v1"),
            format!("{names:?}"),
        ),
        pred(
            ARTIFACT,
            "the network pack's CANCELLATION_CONTRACT declares its replay-choice and early-return rows",
            contract_declares(
                &net::profile::CANCELLATION_CONTRACT,
                &["replay-choice", "early-return"],
            ),
            format!("{:?}", net::profile::CANCELLATION_CONTRACT),
        ),
        pred(
            ARTIFACT,
            "every declared network profile claims no host semantics and no independence relation",
            host_none && all_dependent,
            format!("host_none={host_none} all_dependent={all_dependent} over {names:?}"),
        ),
    ]
}

fn process_predicates() -> Vec<Pred> {
    const ARTIFACT: &str = "phase-b-del-03-reg-02";
    let profiles = prc::profile::PROFILES;
    let names: Vec<&str> = profiles.iter().map(|p| p.name).collect();
    let host_none = profiles
        .iter()
        .all(|p| p.host == prc::profile::HostQualification::None);
    let all_dependent = profiles
        .iter()
        .all(|p| p.independence == prc::profile::IndependenceClaim::AllDependent);
    vec![
        pred(
            ARTIFACT,
            "the process pack is a real dev-dependency edge of continuum-asupersync",
            is_dev_dependency("process"),
            "crates/continuum-asupersync/Cargo.toml [dev-dependencies]",
        ),
        pred(
            ARTIFACT,
            "the process pack declares exactly its two frozen profiles, -v0 and -v1",
            profiles.len() == 2 && names[0].ends_with("-v0") && names[1].ends_with("-v1"),
            format!("{names:?}"),
        ),
        pred(
            ARTIFACT,
            "the process pack's CANCELLATION_CONTRACT declares its replay-choice and early-return rows",
            contract_declares(
                &prc::profile::CANCELLATION_CONTRACT,
                &["replay-choice", "early-return"],
            ),
            format!("{:?}", prc::profile::CANCELLATION_CONTRACT),
        ),
        pred(
            ARTIFACT,
            "every declared process profile claims no host semantics and no independence relation",
            host_none && all_dependent,
            format!("host_none={host_none} all_dependent={all_dependent} over {names:?}"),
        ),
    ]
}

fn storage_predicates() -> Vec<Pred> {
    const ARTIFACT: &str = "phase-b-del-03-reg-03";
    let profiles = sto::profile::PROFILES;
    let names: Vec<&str> = profiles.iter().map(|p| p.name).collect();
    let host_none = profiles
        .iter()
        .all(|p| p.host == sto::profile::HostQualification::None);
    let all_dependent = profiles
        .iter()
        .all(|p| p.independence == sto::profile::IndependenceClaim::AllDependent);
    vec![
        pred(
            ARTIFACT,
            "the storage pack is a real dev-dependency edge of continuum-asupersync",
            is_dev_dependency("storage"),
            "crates/continuum-asupersync/Cargo.toml [dev-dependencies]",
        ),
        pred(
            ARTIFACT,
            "the storage pack declares exactly its two frozen profiles, -v0 and -v1",
            profiles.len() == 2 && names[0].ends_with("-v0") && names[1].ends_with("-v1"),
            format!("{names:?}"),
        ),
        pred(
            ARTIFACT,
            "the storage pack's CANCELLATION_CONTRACT declares its replay-choice and early-return rows",
            contract_declares(
                &sto::profile::CANCELLATION_CONTRACT,
                &["replay-choice", "early-return"],
            ),
            format!("{:?}", sto::profile::CANCELLATION_CONTRACT),
        ),
        pred(
            ARTIFACT,
            "every declared storage profile claims no host semantics and no independence relation",
            host_none && all_dependent,
            format!("host_none={host_none} all_dependent={all_dependent} over {names:?}"),
        ),
    ]
}

// --- leg 2: PR-15 exit evidence integrity -------------------------------------------

const PR15_GOLDEN: &str = include_str!("golden/pr15_exit_evidence.txt");
const PR15_JSON: &str = include_str!("evidence/pr15-exit.json");
const PR15_SUITE_SRC: &str = include_str!("pr15_exit_evidence.rs");

/// The 17 `#[test]` functions `pr15_exit_evidence.rs` names in its own evidence map,
/// plus its retained-artifact stability test. Read here, not off the file's own count,
/// so a renamed or removed test shows as a missing name rather than a smaller total.
const PR15_REQUIRED_TESTS: &[&str] = &[
    "hon_01_each_pack_declares_its_windows_in_its_own_contract",
    "pos_01_network_cancellation_points_replay_exactly",
    "pos_02_process_crash_windows_replay_exactly",
    "pos_03_storage_crash_windows_and_cancellation_points_replay_exactly",
    "pos_04_composed_crash_windows_replay_exactly_and_leave_the_network_untouched",
    "pos_05_register_graceful_crash_windows_replay_exactly",
    "pos_06_register_fail_stop_crash_windows_replay_exactly",
    "pos_07_the_retained_register_campaigns_replay_to_their_recorded_identities",
    "neg_01_a_moved_or_dropped_crash_is_a_typed_divergence",
    "neg_02_a_dropped_or_moved_cancellation_is_a_typed_divergence",
    "neg_03_a_doctored_recorded_journal_is_a_typed_divergence",
    "bnd_01_every_requestable_unsupported_case_is_a_typed_refusal_never_replayed",
    "bnd_02_a_crash_a_boundary_does_not_enable_is_typed_and_conclusive",
    "bnd_03_the_exit_fails_closed_on_an_unreplayable_window",
    "bnd_04_the_time_pack_declares_no_operation_and_so_no_window",
    "bnd_05_the_machine_verdict_is_the_conjunction_of_every_claimed_check",
    "the_exit_evidence_artifacts_are_byte_stable",
];

fn exit_evidence_predicates() -> Vec<Pred> {
    let mut v = Vec::new();
    let golden_digest = Blake3Hasher::hash(PR15_GOLDEN.as_bytes()).to_token();
    let json_digest = Blake3Hasher::hash(PR15_JSON.as_bytes()).to_token();

    v.push(pred(
        "phase-b-del-03-evi-01",
        "the retained PR-15 exit golden and machine summary exist and are non-empty",
        !PR15_GOLDEN.trim().is_empty() && !PR15_JSON.trim().is_empty(),
        format!(
            "golden {} bytes, json {} bytes",
            PR15_GOLDEN.len(),
            PR15_JSON.len()
        ),
    ));

    let ignore_count = PR15_SUITE_SRC.matches("#[ignore").count();
    v.push(pred(
        "phase-b-del-03-evi-01",
        "the PR-15 exit suite carries no #[ignore]: none of its tests are compiled out of the default run",
        ignore_count == 0,
        format!("{ignore_count} #[ignore] attribute(s) found in pr15_exit_evidence.rs"),
    ));

    let missing: Vec<&str> = PR15_REQUIRED_TESTS
        .iter()
        .filter(|name| !PR15_SUITE_SRC.contains(&format!("fn {name}(")))
        .copied()
        .collect();
    let extra_test_fns = PR15_SUITE_SRC.matches("\n#[test]\n").count();
    v.push(pred(
        "phase-b-del-03-evi-01",
        "every named PR-15 exit test function is present in the suite source, and the count matches",
        missing.is_empty() && extra_test_fns == PR15_REQUIRED_TESTS.len(),
        format!(
            "missing {missing:?}; #[test] count {extra_test_fns}, named {}",
            PR15_REQUIRED_TESTS.len()
        ),
    ));

    v.push(pred(
        "phase-b-del-03-evi-01",
        "this file's retained record is digest-bound to the live bytes of the PR-15 golden and JSON (BLAKE3)",
        // The binding is structural: `golden_digest`/`json_digest` are recomputed from
        // `include_str!` (the live committed bytes) on every run and rendered into this
        // file's own golden/JSON below; `the_deliverable_evidence_is_byte_stable`
        // compares that render against the committed copy, so a changed PR-15 artifact
        // changes this file's own golden and fails until re-blessed and reviewed.
        !golden_digest.is_empty() && !json_digest.is_empty(),
        format!("pr15 golden blake3 {golden_digest}; pr15 json blake3 {json_digest}"),
    ));

    v.push(pred(
        "phase-b-del-03-evi-02",
        "the retained PR-15 exit summary says verdict met, fail_closed true, and names bn-jw7q / PR-15-EXIT",
        PR15_JSON.contains("\"verdict\":\"met\"")
            && PR15_JSON.contains("\"fail_closed\":true")
            && PR15_JSON.contains("\"bone\":\"bn-jw7q\"")
            && PR15_JSON.contains("\"requirement\":\"PR-15-EXIT\""),
        "substring check against the live include_str! of evidence/pr15-exit.json",
    ));

    v.push(pred(
        "phase-b-del-03-evi-02",
        "the retained PR-15 exit summary's own predicate count is the conjunction it claims (114)",
        PR15_JSON.matches("\"pass\":true").count() == 114 && !PR15_JSON.contains("\"pass\":false"),
        format!(
            "pass:true count {}, pass:false present {}",
            PR15_JSON.matches("\"pass\":true").count(),
            PR15_JSON.contains("\"pass\":false")
        ),
    ));

    v
}

fn golden_digest_fact() -> String {
    Blake3Hasher::hash(PR15_GOLDEN.as_bytes()).to_token()
}

fn json_digest_fact() -> String {
    Blake3Hasher::hash(PR15_JSON.as_bytes()).to_token()
}

// --- leg 3: cross-component acceptance, live -----------------------------------------

/// The five PR-15-EXIT tests that exercise the network, process and storage packs
/// together through the real harness: `pos_04` composes process and storage crash
/// windows at one boundary while proving the network journal untouched and every move
/// across network steps commutes; `pos_05`/`pos_06` drive the register's real binding
/// (not the Lab handlers alone) through both crash semantics; `neg_01`/`neg_02` are
/// exact replay's negative half, every perturbation a typed divergence, verified
/// equivalent, or absent.
const CROSS_COMPONENT_TESTS: &[&str] = &[
    "pos_04_composed_crash_windows_replay_exactly_and_leave_the_network_untouched",
    "pos_05_register_graceful_crash_windows_replay_exactly",
    "pos_06_register_fail_stop_crash_windows_replay_exactly",
    "neg_01_a_moved_or_dropped_crash_is_a_typed_divergence",
    "neg_02_a_dropped_or_moved_cancellation_is_a_typed_divergence",
];

struct SubprocessRun {
    ran: bool,
    stdout: String,
    stderr: String,
}

/// Re-run [`CROSS_COMPONENT_TESTS`] live, as a subprocess against this crate's own
/// `pr15_exit_evidence` test binary — the same "shell out to `cargo`" pattern
/// `pr15_no_std_lane.rs` uses, applied to a real target instead of a scratch copy, so
/// this is the harness itself running, not a description of it. Memoized: every
/// predicate that needs it shares the one run.
fn cross_component_run() -> &'static SubprocessRun {
    static RUN: OnceLock<SubprocessRun> = OnceLock::new();
    RUN.get_or_init(|| {
        let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let workspace_root = manifest_dir
            .join("../..")
            .canonicalize()
            .expect("the workspace root exists");
        let mut command = Command::new(env!("CARGO"));
        command
            .arg("test")
            .arg("--offline")
            .arg("--locked")
            .arg("-p")
            .arg("continuum-asupersync")
            .arg("--test")
            .arg("pr15_exit_evidence")
            .arg("--")
            .arg("--exact");
        for name in CROSS_COMPONENT_TESTS {
            command.arg(name);
        }
        command.current_dir(&workspace_root);
        match command.output() {
            Ok(output) => SubprocessRun {
                ran: output.status.success(),
                stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
                stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
            },
            Err(err) => SubprocessRun {
                ran: false,
                stdout: String::new(),
                stderr: format!("failed to spawn cargo: {err}"),
            },
        }
    })
}

fn test_ran_ok(run: &SubprocessRun, name: &str) -> bool {
    run.ran && run.stdout.contains(&format!("test {name} ... ok"))
}

fn cross_component_predicates() -> Vec<Pred> {
    let run = cross_component_run();
    vec![
        pred(
            "phase-b-del-03-xc-01",
            "the composed pack test (network+process+storage together, real Lab handlers) replays exactly, live",
            test_ran_ok(
                run,
                "pos_04_composed_crash_windows_replay_exactly_and_leave_the_network_untouched",
            ),
            subprocess_detail(run),
        ),
        pred(
            "phase-b-del-03-xc-02",
            "the register's real binding (both crash semantics) drives the packs and replays exactly, live",
            test_ran_ok(run, "pos_05_register_graceful_crash_windows_replay_exactly")
                && test_ran_ok(
                    run,
                    "pos_06_register_fail_stop_crash_windows_replay_exactly",
                ),
            subprocess_detail(run),
        ),
        pred(
            "phase-b-del-03-xc-03",
            "a perturbation of a composed or binding window is a typed divergence, live, never a silent accept",
            test_ran_ok(run, "neg_01_a_moved_or_dropped_crash_is_a_typed_divergence")
                && test_ran_ok(
                    run,
                    "neg_02_a_dropped_or_moved_cancellation_is_a_typed_divergence",
                ),
            subprocess_detail(run),
        ),
        pred(
            "phase-b-del-03-xc-01",
            "the subprocess itself exited 0 (cargo ran, and every named test in the slice passed)",
            run.ran,
            subprocess_detail(run),
        ),
    ]
}

/// The subprocess's own "test result: ok. N passed; ..." line, with the trailing
/// "; finished in <seconds>" clause dropped: wall-clock time is not part of the claim,
/// varies run to run, and would otherwise make the retained golden non-reproducible.
fn subprocess_detail(run: &SubprocessRun) -> String {
    if run.ran {
        let line = run
            .stdout
            .lines()
            .rev()
            .find(|line| line.starts_with("test result:"))
            .unwrap_or("test result: (not found)");
        line.split("; finished in")
            .next()
            .unwrap_or(line)
            .to_owned()
    } else {
        format!(
            "subprocess did not succeed; stderr tail: {}",
            run.stderr
                .lines()
                .rev()
                .take(3)
                .collect::<Vec<_>>()
                .join(" | ")
        )
    }
}

// --- leg 4: the pack independence rule, referenced honestly -------------------------

fn independence_predicates() -> Vec<Pred> {
    let network = net::profile::PROFILES
        .iter()
        .all(|p| p.host == net::profile::HostQualification::None);
    let process = prc::profile::PROFILES
        .iter()
        .all(|p| p.host == prc::profile::HostQualification::None);
    let storage = sto::profile::PROFILES
        .iter()
        .all(|p| p.host == sto::profile::HostQualification::None);
    vec![pred(
        "phase-b-del-03-t07-01",
        "T06/T07/R10 are open (not enforced) and this record does not claim otherwise; \
         every pack's profiles self-declare HostQualification::None, the narrower fact \
         that is mechanically true today",
        network && process && storage,
        "T06 bn-22sa: open. T07 bn-3uva: open (controls not implemented: sandbox \
         untrusted packs, dynamic footprint validation, baseline cross-check, \
         first-party review for certified mode, pack signature/provenance). \
         R10 bn-2a0v: open. States read via `bn show` on 2026-09-25; owned by those \
         bones, not by bn-1e40. HostQualification::None holds for every profile in \
         network, process and storage's PROFILES.",
    )]
}

// ---------------------------------------------------------------------------
// aggregation: fail closed
// ---------------------------------------------------------------------------

fn predicates() -> Vec<Pred> {
    let mut all = Vec::new();
    all.extend(network_predicates());
    all.extend(process_predicates());
    all.extend(storage_predicates());
    all.extend(exit_evidence_predicates());
    all.extend(cross_component_predicates());
    all.extend(independence_predicates());
    all
}

/// The exact 23 `(artifact, id)` pairs the deliverable's verdict requires — a hand-typed
/// static list, not derived from [`predicates`] (cr-1qktn9: mirrors `pr16_exit_evidence
/// .rs`'s `required`, "derived from the static tables, not the results"). This is what
/// makes [`aggregate`] a real pin rather than a tautology: `required().iter().map(|p|
/// (p.artifact, p.id))` would always equal `predicates()`'s own ids by construction, so
/// a predicate silently dropped from a `*_predicates()` function — or a whole function
/// dropped from [`predicates`] — would shrink both sides together and `aggregate` would
/// never notice. Checked against the live set in
/// [`bnd_01_the_verdict_is_the_conjunction_of_every_required_predicate`]: every id here
/// must appear in [`predicates`] exactly once and pass, and [`predicates`] must name
/// nothing this list does not.
const REQUIRED: [(&str, &str); 23] = [
    (
        "phase-b-del-03-reg-01",
        "the network pack is a real dev-dependency edge of continuum-asupersync",
    ),
    (
        "phase-b-del-03-reg-01",
        "the network pack declares exactly its two frozen profiles, -v0 and -v1",
    ),
    (
        "phase-b-del-03-reg-01",
        "the network pack's CANCELLATION_CONTRACT declares its replay-choice and early-return rows",
    ),
    (
        "phase-b-del-03-reg-01",
        "every declared network profile claims no host semantics and no independence relation",
    ),
    (
        "phase-b-del-03-reg-02",
        "the process pack is a real dev-dependency edge of continuum-asupersync",
    ),
    (
        "phase-b-del-03-reg-02",
        "the process pack declares exactly its two frozen profiles, -v0 and -v1",
    ),
    (
        "phase-b-del-03-reg-02",
        "the process pack's CANCELLATION_CONTRACT declares its replay-choice and early-return rows",
    ),
    (
        "phase-b-del-03-reg-02",
        "every declared process profile claims no host semantics and no independence relation",
    ),
    (
        "phase-b-del-03-reg-03",
        "the storage pack is a real dev-dependency edge of continuum-asupersync",
    ),
    (
        "phase-b-del-03-reg-03",
        "the storage pack declares exactly its two frozen profiles, -v0 and -v1",
    ),
    (
        "phase-b-del-03-reg-03",
        "the storage pack's CANCELLATION_CONTRACT declares its replay-choice and early-return rows",
    ),
    (
        "phase-b-del-03-reg-03",
        "every declared storage profile claims no host semantics and no independence relation",
    ),
    (
        "phase-b-del-03-evi-01",
        "the retained PR-15 exit golden and machine summary exist and are non-empty",
    ),
    (
        "phase-b-del-03-evi-01",
        "the PR-15 exit suite carries no #[ignore]: none of its tests are compiled out of the default run",
    ),
    (
        "phase-b-del-03-evi-01",
        "every named PR-15 exit test function is present in the suite source, and the count matches",
    ),
    (
        "phase-b-del-03-evi-01",
        "this file's retained record is digest-bound to the live bytes of the PR-15 golden and JSON (BLAKE3)",
    ),
    (
        "phase-b-del-03-evi-02",
        "the retained PR-15 exit summary says verdict met, fail_closed true, and names bn-jw7q / PR-15-EXIT",
    ),
    (
        "phase-b-del-03-evi-02",
        "the retained PR-15 exit summary's own predicate count is the conjunction it claims (114)",
    ),
    (
        "phase-b-del-03-xc-01",
        "the composed pack test (network+process+storage together, real Lab handlers) replays exactly, live",
    ),
    (
        "phase-b-del-03-xc-02",
        "the register's real binding (both crash semantics) drives the packs and replays exactly, live",
    ),
    (
        "phase-b-del-03-xc-03",
        "a perturbation of a composed or binding window is a typed divergence, live, never a silent accept",
    ),
    (
        "phase-b-del-03-xc-01",
        "the subprocess itself exited 0 (cargo ran, and every named test in the slice passed)",
    ),
    (
        "phase-b-del-03-t07-01",
        "T06/T07/R10 are open (not enforced) and this record does not claim otherwise; every pack's profiles self-declare HostQualification::None, the narrower fact that is mechanically true today",
    ),
];

fn required() -> Vec<(&'static str, &'static str)> {
    REQUIRED.to_vec()
}

fn aggregate(preds: &[Pred]) -> Result<(), Vec<String>> {
    let req = required();
    let mut why = Vec::new();
    for (artifact, id) in &req {
        let matches: Vec<&Pred> = preds
            .iter()
            .filter(|p| p.artifact == *artifact && p.id == *id)
            .collect();
        match matches.as_slice() {
            [] => why.push(format!("missing: {artifact} {id}")),
            [one] if one.pass => {}
            [one] => why.push(format!("failing: {artifact} {id}: {}", one.detail)),
            _ => why.push(format!("duplicated: {artifact} {id}")),
        }
    }
    for p in preds {
        if !req.iter().any(|(a, id)| *a == p.artifact && *id == p.id) {
            why.push(format!("not required: {} {}", p.artifact, p.id));
        }
    }
    if why.is_empty() { Ok(()) } else { Err(why) }
}

// ---------------------------------------------------------------------------
// tests
// ---------------------------------------------------------------------------

#[test]
fn reg_01_the_network_pack_is_present_registered_and_declares_its_windows() {
    let preds = network_predicates();
    for p in &preds {
        assert!(p.pass, "{}: {}", p.id, p.detail);
    }
}

#[test]
fn reg_02_the_process_pack_is_present_registered_and_declares_its_windows() {
    let preds = process_predicates();
    for p in &preds {
        assert!(p.pass, "{}: {}", p.id, p.detail);
    }
}

#[test]
fn reg_03_the_storage_pack_is_present_registered_and_declares_its_windows() {
    let preds = storage_predicates();
    for p in &preds {
        assert!(p.pass, "{}: {}", p.id, p.detail);
    }
}

#[test]
fn evi_01_the_pr15_exit_evidence_exists_is_not_compiled_out_and_is_digest_bound() {
    let preds = exit_evidence_predicates();
    for p in &preds {
        if p.artifact == "phase-b-del-03-evi-01" {
            assert!(p.pass, "{}: {}", p.id, p.detail);
        }
    }
}

#[test]
fn evi_02_the_pr15_exit_evidence_verdict_is_met_and_fail_closed() {
    let preds = exit_evidence_predicates();
    for p in &preds {
        if p.artifact == "phase-b-del-03-evi-02" {
            assert!(p.pass, "{}: {}", p.id, p.detail);
        }
    }
}

#[test]
fn xc_01_the_composed_pack_scenario_replays_exactly_live() {
    let preds = cross_component_predicates();
    for p in &preds {
        if p.artifact == "phase-b-del-03-xc-01" {
            assert!(p.pass, "{}: {}", p.id, p.detail);
        }
    }
}

#[test]
fn xc_02_the_registers_real_binding_drives_the_packs_and_replays_exactly_live() {
    let preds = cross_component_predicates();
    for p in &preds {
        if p.artifact == "phase-b-del-03-xc-02" {
            assert!(p.pass, "{}: {}", p.id, p.detail);
        }
    }
}

#[test]
fn xc_03_a_perturbation_of_a_cross_component_window_is_a_typed_divergence_live() {
    let preds = cross_component_predicates();
    for p in &preds {
        if p.artifact == "phase-b-del-03-xc-03" {
            assert!(p.pass, "{}: {}", p.id, p.detail);
        }
    }
}

#[test]
fn t07_01_the_pack_independence_rule_is_referenced_honestly_not_claimed() {
    let preds = independence_predicates();
    for p in &preds {
        assert!(p.pass, "{}: {}", p.id, p.detail);
    }
}

#[test]
fn bnd_01_the_verdict_is_the_conjunction_of_every_required_predicate() {
    let real = predicates();
    assert_eq!(aggregate(&real), Ok(()));
    let req = required();
    assert_eq!(real.len(), req.len());
    for i in 0..real.len() {
        let mut flipped = real.clone();
        flipped[i].pass = false;
        assert!(
            aggregate(&flipped).is_err(),
            "flipping {} kept the verdict",
            real[i].id
        );
        let mut deleted = real.clone();
        deleted.remove(i);
        assert!(
            aggregate(&deleted).is_err(),
            "deleting {} kept the verdict",
            real[i].id
        );
        let mut duplicated = real.clone();
        duplicated.push(real[i].clone());
        assert!(
            aggregate(&duplicated).is_err(),
            "duplicating {} kept the verdict",
            real[i].id
        );
    }
    let mut extra = real.clone();
    extra.push(pred(
        "phase-b-del-03-reg-01",
        "unnamed",
        true,
        String::new(),
    ));
    assert!(aggregate(&extra).is_err());
    // The verdict text itself is rendered from `aggregate`, not a fixed string: flipping
    // one real predicate must flip both the golden and the JSON's rendered `verdict`
    // line, and the untouched original must still say `met`.
    let mut flipped = real.clone();
    flipped[0].pass = false;
    assert!(render_json(&flipped).contains("\"verdict\":\"not-met\""));
    assert!(render_golden(&flipped).contains("\nverdict: not-met\n"));
    assert!(render_json(&real).contains("\"verdict\":\"met\""));
    assert!(render_golden(&real).contains("\nverdict: met\n"));
}

/// Every low-level check a predicate is built from is a real two-valued function of its
/// input, not a constant dressed up as a computation: each can be driven to both `true`
/// and `false` with data this test controls, independent of what the live workspace
/// happens to contain today.
#[test]
fn bnd_02_the_predicate_ingredients_are_computed_not_fixed() {
    let complete: [(&str, &str); 2] = [
        ("replay-choice", "every step is a choice"),
        ("early-return", "lands before or after"),
    ];
    assert!(contract_declares(
        &complete,
        &["replay-choice", "early-return"]
    ));
    let missing_row: [(&str, &str); 1] = [("replay-choice", "every step is a choice")];
    assert!(!contract_declares(
        &missing_row,
        &["replay-choice", "early-return"]
    ));
    let blank_row: [(&str, &str); 2] = [
        ("replay-choice", ""),
        ("early-return", "lands before or after"),
    ];
    assert!(
        !contract_declares(&blank_row, &["replay-choice", "early-return"]),
        "a row with empty text must not count as declared"
    );

    // `is_dev_dependency` reads the real, live `CARGO_TOML` constant (it is not
    // parameterized over a synthetic manifest), so its `false` arm is driven by a pack
    // name the manifest cannot possibly declare, and its `true` arm by the three packs
    // this deliverable actually names.
    assert!(is_dev_dependency("network"));
    assert!(is_dev_dependency("process"));
    assert!(is_dev_dependency("storage"));
    assert!(!is_dev_dependency(
        "this-pack-does-not-exist-in-any-manifest"
    ));

    let ok = SubprocessRun {
        ran: true,
        stdout: "test foo ... ok\ntest result: ok. 1 passed\n".to_owned(),
        stderr: String::new(),
    };
    assert!(test_ran_ok(&ok, "foo"));
    assert!(
        !test_ran_ok(&ok, "bar"),
        "a test name absent from stdout must never read as ok"
    );
    let did_not_run = SubprocessRun {
        ran: false,
        stdout: "test foo ... ok\n".to_owned(),
        stderr: "cargo: could not compile".to_owned(),
    };
    assert!(
        !test_ran_ok(&did_not_run, "foo"),
        "a matching stdout line must not count when the subprocess itself did not exit 0"
    );
}

/// cr-1qktn9 [medium]: the retained JSON's declared `predicate_count` must equal the
/// live predicate list's own length, the number of predicate objects actually rendered
/// into its `predicates` array, and the sum of every artifact's own `predicate_count` —
/// parsed back out of the rendered text itself rather than assumed, so a renderer bug
/// that drifts the declared number from what it actually wrote is caught here rather
/// than sitting unchecked in prose (the plan.md annotation and earlier bone comments
/// said 24 for what is, and has always been, 23 real predicates — this is what would
/// have caught that by construction instead of by a human recount).
#[test]
fn bnd_03_the_declared_predicate_count_matches_the_predicates_array_and_the_grouped_totals() {
    let preds = predicates();
    let json = render_json(&preds);

    let predicate_objects = json.matches("\"pass\":").count();
    assert_eq!(
        predicate_objects,
        preds.len(),
        "one \"pass\" field per rendered predicate object"
    );

    let counts = find_all_u64_after(&json, "\"predicate_count\":");
    let (declared, per_artifact) = counts
        .split_first()
        .expect("at least the top-level predicate_count field is rendered");
    assert_eq!(
        *declared,
        preds.len() as u64,
        "the declared predicate_count must equal predicates.len()"
    );
    let grouped_total: u64 = per_artifact.iter().sum();
    assert_eq!(
        grouped_total,
        preds.len() as u64,
        "the sum of every artifact's own predicate_count must equal the declared total"
    );

    // The golden's human-readable line must say the same number.
    let golden = render_golden(&preds);
    assert_eq!(
        find_u64_after(&golden, "predicate count: "),
        preds.len() as u64
    );
}

// ---------------------------------------------------------------------------
// retained artifacts: golden (human) and JSON (machine)
// ---------------------------------------------------------------------------

fn render_golden(preds: &[Pred]) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "PHASE-B-DEL-03 deliverable evidence (bn-1e40)");
    let _ = writeln!(
        out,
        "requirement: PHASE-B-DEL-03 (notes/plan/plan.md, Phase B deliverable list)"
    );
    let _ = writeln!(
        out,
        "constituent bones: bn-3ohe (network), bn-3mmf (process), bn-2fk3 (storage),"
    );
    let _ = writeln!(
        out,
        "  bn-1oj6 (IMPL-04 unsupported/time), bn-jw7q (PR-15-EXIT); PR 15 itself bn-1nn done"
    );
    let _ = writeln!(
        out,
        "not enforced, referenced honestly: T06 bn-22sa, T07 bn-3uva, R10 bn-2a0v (all open)"
    );
    let _ = writeln!(out, "pr15 golden blake3: {}", golden_digest_fact());
    let _ = writeln!(out, "pr15 json blake3: {}", json_digest_fact());
    let _ = writeln!(out);
    let verdict = if aggregate(preds).is_ok() {
        "met"
    } else {
        "not-met"
    };
    let _ = writeln!(out, "verdict: {verdict}");
    let _ = writeln!(out, "fail_closed: true");
    let _ = writeln!(out, "predicate count: {}", preds.len());
    let _ = writeln!(out);
    for p in preds {
        let _ = writeln!(
            out,
            "[{}] {} :: {} :: {} :: {}",
            if p.pass { "pass" } else { "FAIL" },
            p.artifact,
            p.id,
            p.pass,
            p.detail
        );
    }
    out
}

/// The integer immediately following the first occurrence of `key` in `text` (e.g.
/// `find_u64_after(json, "\"predicate_count\":")`) — the light hand-rolled JSON reading
/// this file already uses elsewhere (`PR15_JSON.matches(...)` above), rather than a new
/// `serde_json` dependency this crate does not already have.
fn find_u64_after(text: &str, key: &str) -> u64 {
    let idx = text
        .find(key)
        .unwrap_or_else(|| panic!("{key} not found in: {text}"));
    let rest = &text[idx + key.len()..];
    let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
    digits
        .parse()
        .unwrap_or_else(|_| panic!("no digits after {key}"))
}

/// Every integer following an occurrence of `key` in `text`, in order.
fn find_all_u64_after(text: &str, key: &str) -> Vec<u64> {
    let mut out = Vec::new();
    let mut start = 0usize;
    while let Some(pos) = text[start..].find(key) {
        let abs = start + pos;
        out.push(find_u64_after(&text[abs..], key));
        start = abs + key.len();
    }
    out
}

fn json_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            c if (c as u32) < 0x20 => {
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out
}

fn render_json(preds: &[Pred]) -> String {
    let verdict = if aggregate(preds).is_ok() {
        "met"
    } else {
        "not-met"
    };
    let mut by_artifact: BTreeMap<&'static str, Vec<&Pred>> = BTreeMap::new();
    for p in preds {
        by_artifact.entry(p.artifact).or_default().push(p);
    }
    let mut out = String::new();
    out.push('{');
    let _ = write!(out, "\"bone\":\"bn-1e40\",");
    let _ = write!(out, "\"requirement\":\"PHASE-B-DEL-03\",");
    let _ = write!(
        out,
        "\"constituent_bones\":[\"bn-1nn\",\"bn-3ohe\",\"bn-3mmf\",\"bn-2fk3\",\"bn-1oj6\",\"bn-jw7q\"],"
    );
    let _ = write!(
        out,
        "\"not_enforced\":[{{\"id\":\"T06\",\"bone\":\"bn-22sa\",\"state\":\"open\"}},\
         {{\"id\":\"T07\",\"bone\":\"bn-3uva\",\"state\":\"open\"}},\
         {{\"id\":\"R10\",\"bone\":\"bn-2a0v\",\"state\":\"open\"}}],"
    );
    let _ = write!(
        out,
        "\"pr15_golden_blake3\":\"{}\",\"pr15_json_blake3\":\"{}\",",
        golden_digest_fact(),
        json_digest_fact()
    );
    let _ = write!(
        out,
        "\"suite\":\"crates/continuum-asupersync/tests/phase_b_del_03_deliverable_evidence.rs\","
    );
    let _ = write!(out, "\"fail_closed\":true,");
    let _ = write!(out, "\"verdict\":\"{verdict}\",");
    // Declared, not just implicit in the array's own length: computed from `preds.len()`
    // on every render (cr-1qktn9), and checked against both the array length and the
    // per-artifact totals below in `bnd_03_the_declared_predicate_count_matches_the_
    // predicates_array_and_the_grouped_totals`, so a hand-edited or stale number here
    // fails the byte-stability test rather than sitting unchecked in prose.
    let _ = write!(out, "\"predicate_count\":{},", preds.len());
    out.push_str("\"predicates\":[");
    for (i, p) in preds.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        let _ = write!(
            out,
            "{{\"artifact\":\"{}\",\"id\":\"{}\",\"pass\":{},\"detail\":\"{}\"}}",
            p.artifact,
            json_escape(p.id),
            p.pass,
            json_escape(&p.detail)
        );
    }
    out.push_str("],");
    out.push_str("\"artifacts\":[");
    for (i, (artifact, ps)) in by_artifact.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        let met = ps.iter().all(|p| p.pass);
        let _ = write!(
            out,
            "{{\"id\":\"{artifact}\",\"verdict\":\"{}\",\"predicate_count\":{}}}",
            if met { "met" } else { "not-met" },
            ps.len()
        );
    }
    out.push_str("]}");
    out.push('\n');
    out
}

/// Write `contents` to `path`, refusing to follow a symlink planted at `path` rather
/// than writing through it (cr-1qktn9 [medium]).
///
/// The common case — nothing exists at `path` yet, true for every fresh scratch file
/// this suite creates — takes the atomic path: `create_new` opens with `O_CREAT|O_EXCL`
/// (Unix) / `CREATE_NEW` (Windows), so if *anything* is already there when the open
/// happens — a regular file, a directory, a symlink, dangling or not — the call fails
/// with `AlreadyExists` and nothing is written through it. There is no window between a
/// check and the write for a planted symlink to win.
///
/// The one case that legitimately re-uses an existing path is `PHASE_B_DEL_03_BLESS`
/// itself overwriting the already-committed golden/JSON on a second bless: there
/// `create_new` correctly reports `AlreadyExists`, and this falls back to
/// `symlink_metadata` (which, unlike `metadata`, does not follow a symlink) to confirm
/// the existing entry is an ordinary regular file before overwriting it; a symlink is
/// refused instead of followed.
fn write_no_symlink(path: &std::path::Path, contents: &str) -> std::io::Result<()> {
    use std::io::Write as _;
    match std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
    {
        Ok(mut file) => file.write_all(contents.as_bytes()),
        Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => {
            let existing = std::fs::symlink_metadata(path)?;
            if existing.file_type().is_symlink() {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::AlreadyExists,
                    format!("refusing to write through a symlink at {}", path.display()),
                ));
            }
            std::fs::write(path, contents)
        }
        Err(err) => Err(err),
    }
}

/// Write `golden` to `golden_path` and `json` to `json_path` (never through a planted
/// symlink — see [`write_no_symlink`]), then panic — unconditionally, on every call,
/// whether or not the new bytes differ from whatever was there before. This is the
/// whole of what `PHASE_B_DEL_03_BLESS` does; it is factored out to its own function,
/// taking plain paths rather than reading the env var or `CARGO_MANIFEST_DIR` itself, so
/// [`bless_always_panics_after_writing_even_when_the_bytes_are_unchanged`] can drive it
/// against a scratch directory and observe the panic directly, rather than trusting a
/// reading of the `if` block below. INV-004: a blessing run must never go green in the
/// same run that wrote its own new bytes — self-certification is exactly what a
/// diff-conditioned panic would be, so this function has no branch that returns
/// normally, matching `pr13_exit_evidence.rs`'s `bless`.
fn bless(
    golden_path: &std::path::Path,
    json_path: &std::path::Path,
    golden: &str,
    json: &str,
) -> ! {
    write_no_symlink(golden_path, golden).expect("the golden is writable and not a symlink");
    write_no_symlink(json_path, json).expect("the summary is writable and not a symlink");
    panic!(
        "PHASE_B_DEL_03_BLESS rewrote the golden and the summary; rerun without the \
         variable and review the diff as a contract change"
    );
}

/// A fresh, private, unpredictable directory under this crate's own
/// `CARGO_TARGET_TMPDIR` (set by Cargo for test binaries; unlike `std::env::temp_dir()`
/// it is not a world-writable location shared with every other user and process on the
/// host) for a self-test's scratch files (cr-1qktn9 [medium]). Created with
/// `std::fs::create_dir`, which fails rather than following or accepting whatever is
/// already at the path — a pre-existing directory, file, or symlink a same-host
/// attacker planted there is refused, never reused; on a name collision (vanishingly
/// unlikely given the pid/thread/nanosecond/counter suffix, but checked rather than
/// assumed) the name is retried rather than falling back to `create_dir_all`, which
/// would silently accept a pre-existing attacker-controlled directory.
fn private_scratch_dir(label: &str) -> std::path::PathBuf {
    let base = std::path::Path::new(env!("CARGO_TARGET_TMPDIR"));
    std::fs::create_dir_all(base).expect("the crate's own target tmp dir is creatable");
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    for attempt in 0..64u32 {
        let candidate = base.join(format!(
            "{label}-{}-{:?}-{nanos}-{attempt}",
            std::process::id(),
            std::thread::current().id()
        ));
        match std::fs::create_dir(&candidate) {
            Ok(()) => return candidate,
            Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(err) => panic!(
                "could not create a private scratch dir at {}: {err}",
                candidate.display()
            ),
        }
    }
    panic!("could not find an unused private scratch dir name for {label} after 64 attempts");
}

#[test]
fn the_deliverable_evidence_is_byte_stable() {
    const GOLDEN: &str = include_str!("golden/phase_b_del_03_deliverable_evidence.txt");
    const JSON: &str = include_str!("evidence/phase-b-del-03.json");
    let preds = predicates();
    let golden = render_golden(&preds);
    let json = render_json(&preds);
    if std::env::var_os("PHASE_B_DEL_03_BLESS").is_some() {
        let root = env!("CARGO_MANIFEST_DIR");
        bless(
            std::path::Path::new(&format!(
                "{root}/tests/golden/phase_b_del_03_deliverable_evidence.txt"
            )),
            std::path::Path::new(&format!("{root}/tests/evidence/phase-b-del-03.json")),
            &golden,
            &json,
        );
    }
    assert!(
        json.contains("\"verdict\":\"met\""),
        "the deliverable evidence is not met:\n{json}"
    );
    assert_eq!(
        golden, GOLDEN,
        "the deliverable evidence drifted; to regenerate deliberately, run with \
         PHASE_B_DEL_03_BLESS=1 and review the diff.\nactual:\n{golden}"
    );
    assert_eq!(json, JSON, "the machine-readable summary drifted");
    assert_eq!(golden, render_golden(&predicates()));
}

/// `bless` (what `PHASE_B_DEL_03_BLESS` runs) must panic on every call, not only when
/// the new bytes differ from the old ones — a diff-conditioned panic would let a
/// blessing run go green in the very run that wrote its own new bytes, the
/// self-certification INV-004 forbids and the property Codex's bn-1bj4 finding named.
/// Driven twice against a scratch directory with byte-identical content both times, so
/// the second call has nothing to diff against and must still panic.
#[test]
fn bless_always_panics_after_writing_even_when_the_bytes_are_unchanged() {
    let dir = private_scratch_dir("phase-b-del-03-bless-selftest");
    let golden_path = dir.join("golden.txt");
    let json_path = dir.join("evidence.json");

    let first = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        bless(&golden_path, &json_path, "same content\n", "same json\n")
    }));
    assert!(first.is_err(), "bless must panic on its first write");
    assert_eq!(
        std::fs::read_to_string(&golden_path).expect("bless wrote the golden"),
        "same content\n"
    );
    assert_eq!(
        std::fs::read_to_string(&json_path).expect("bless wrote the summary"),
        "same json\n"
    );

    // Second call: identical bytes, nothing to diff. A correct `bless` still panics.
    let second = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        bless(&golden_path, &json_path, "same content\n", "same json\n")
    }));
    assert!(
        second.is_err(),
        "bless must panic even when the new bytes are byte-identical to what was already there"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

/// cr-1qktn9 [medium], the validation it names: a symlink planted at the golden or JSON
/// path is refused, never followed and written through. A dangling symlink pointed at a
/// `victim.txt` this test controls; `bless` must leave `victim.txt` byte-for-byte
/// unchanged and the symlink itself still a symlink, not a regular file `bless`
/// overwrote in place.
#[cfg(unix)]
#[test]
fn bless_refuses_to_follow_a_planted_symlink_rather_than_writing_through_it() {
    let dir = private_scratch_dir("phase-b-del-03-bless-symlink-selftest");
    let victim = dir.join("victim.txt");
    std::fs::write(&victim, "DO NOT OVERWRITE\n").expect("the victim file is writable");

    let golden_path = dir.join("golden.txt");
    std::os::unix::fs::symlink(&victim, &golden_path).expect("a symlink is creatable");
    let json_path = dir.join("evidence.json");

    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        bless(
            &golden_path,
            &json_path,
            "attacker content\n",
            "attacker json\n",
        )
    }));
    // `bless` panics either way (it always does — the property above); what matters here
    // is what it did *not* do: write through the symlink.
    assert!(result.is_err());
    assert_eq!(
        std::fs::read_to_string(&victim).expect("the victim file is still a readable regular file"),
        "DO NOT OVERWRITE\n",
        "a symlink at the golden path must be refused, not followed: the victim file it \
         points at must be untouched"
    );
    assert!(
        std::fs::symlink_metadata(&golden_path)
            .expect("the golden path still exists")
            .file_type()
            .is_symlink(),
        "the planted symlink must still be a symlink, not replaced by a regular file bless wrote"
    );

    let _ = std::fs::remove_dir_all(&dir);
}
