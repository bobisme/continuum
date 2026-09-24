//! Dedicated exit evidence for `INV-005` (`notes/plan/plan.md:323`, the invariant bone
//! `bn-jme9`).
//!
//! > `INV-005` — No ambient nondeterminism
//! >
//! > Controlled code accesses scheduling, time, entropy, I/O, faults, and cancellation
//! > through explicit capabilities.
//! >
//! > — `notes/plan/plan.md:323-325`
//!
//! # This is a binding file, not a rebuild
//!
//! INV-005 is already the most heavily mechanically enforced invariant in the dossier:
//! `tools/governance/check_code_policy.py`'s `GOV-1-03`/`GOV-1-04`/`GOV-1-05` scan the
//! entire 27-crate semantic core for nondeterministic collections, ambient time/RNG/
//! environment reads, and platform-dependent hashing, with fixtures proving every check
//! actually catches what it claims to; `tools/check_crate_boundaries.py`'s
//! `kernel-is-synchronous` and `certificate-checker-not-search` rules hold the trusted
//! checking base free of async runtimes and search; and `continuum-engine-reference`'s
//! own module documentation and `continuum-task::region`'s own module documentation
//! already make the determinism argument for the two crates that carry it furthest. None
//! of that is rebuilt here. This file does four things none of the above does:
//!
//! 1. **audits** the six named ambients against their actual capability seam or
//!    enforcement point, once, in one place, so the mapping is checkable rather than
//!    scattered across a dozen module docs (below);
//! 2. **re-runs the governance checkers live** (not by reading a possibly-stale
//!    committed JSON) and pins their fixture-level negative evidence by name;
//! 3. **exercises three of the six seams behaviorally** — cancellation/scheduling
//!    (`continuum-task::region`), faults (`continuum-workspace::publication::
//!    StorageFaults`), and entropy (`continuum-workspace::publication::CapabilityToken`)
//!    — proving each is reached only as a value the caller supplies, which a source scan
//!    cannot show and a module doc's say-so is not evidence for;
//! 4. **adds one genuinely new cross-crate determinism regression**
//!    (`continuum-engine-reference/tests/certificate_kernel_differential.rs`'s
//!    `two_independent_pipelines_from_a_declared_model_to_a_checked_certificate_are_byte_identical`,
//!    cited below and not duplicated here) at the behavioral level GOV-1-04's static scan
//!    cannot reach: two independent runs of a real cross-crate computation, byte-identical.
//!
//! # Audit: the six named ambients
//!
//! | Ambient | Tier that may touch it | Declared seam | Enforcement |
//! |---|---|---|---|
//! | **scheduling** | boundary (`continuum-asupersync`, Phase B substrate); landed today in the semantic core as data | [`continuum_task::region::RegionTree`] + [`continuum_task::region::schedule::Schedule`] — an interleaving is a value, every operation is `&mut self`, no thread, no async runtime (region.rs's own "Determinism: concurrency is data") | `check_crate_boundaries.py` `kernel-is-synchronous` (no async-runtime edge, direct or transitive through `continuum-asupersync`, in the trusted checking base) + `GOV-1-06`'s network/async-runtime-crate scan over the same closure — [`positive_the_crate_boundary_checker_proves_the_trusted_checking_base_is_synchronous_and_search_free`], [`positive_cancellation_and_scheduling_reach_region_calculus_code_as_typed_values_never_a_panic`] |
//! | **time** | boundary (`continuum-effects-time` declares `time/unmodelled-v0` at bn-1oj6: every RFC 0002 clock semantic is declared unsupported and the pack has no operation, so no clock reading, timeout or timer reaches controlled code through it; virtual-time profiles have not landed) | none landed yet; the prohibition is enforced everywhere a capability could be bypassed | `GOV-1-04`'s `ambient-time` check (`SystemTime`/`Instant`/`UNIX_EPOCH`/`chrono`/`OffsetDateTime`) over the whole semantic core; fixtures `gov-1-04-systemtime-in-kernel`, `gov-1-04-clock-dependency` — [`positive_gov_1_04_self_test_still_catches_exactly_its_five_named_fixtures`], [`positive_the_real_run_of_gov_1_03_04_05_and_06_finds_zero_semantic_core_violations`] |
//! | **entropy** | boundary (search engines' future seed capability — `continuum-engine-{dpor,symbolic,liveness}` are all PR-1/IMPL-01 scaffolds, and `continuum-forge`'s one landed lane is a contract-admission decision that draws no entropy — bn-1dsih); landed today in `continuum-workspace`'s capability minting | [`continuum_workspace::publication::CapabilityToken::mint`] — "takes the identity as an argument rather than drawing one from a random source: entropy is a capability, not ambient (INV-005, ADR-0003)" (that type's own doc, quoting INV-005 by name) | `GOV-1-04`'s `ambient-rng` check; fixture `gov-1-04-thread-rng-in-engine` plants `thread_rng` specifically inside an engine crate — [`positive_entropy_reaches_a_capability_token_only_as_a_caller_supplied_identity_never_ambient_randomness`] |
//! | **I/O** | boundary (`continuum-effects-{storage,network}`, PR-1/IMPL-01 scaffolds); landed today behind the store | [`continuum_workspace::publication::ReferenceStore`] — the semantic core's only path to persisted content, reached exclusively through `ContentIdentifier`/`AuditSink`/capability tokens; no `SEMANTIC_CORE` crate opens a file or socket directly | `GOV-1-06` `no-network-in-checker` (the kernel + certificate closure contains no network-capable member and no network/async dependency, direct or transitive) — [`positive_the_real_run_of_gov_1_03_04_05_and_06_finds_zero_semantic_core_violations`] |
//! | **faults** | boundary + daemon integration | [`continuum_workspace::publication::StorageFaults`] — a trait the *caller* supplies (`NoFaults`, or a custom implementation), consulted at each [`continuum_workspace::publication::PublicationPhase`] and answering a typed [`continuum_workspace::publication::AbortReason`]; `continuumd::daemon::recovery::{CrashPoint, FaultInjector}` — the daemon's own typed crash-injection capability | no source-scan rule (this ambient is not a symbol or a dependency to grep for); enforced behaviorally — [`positive_faults_reach_the_store_only_as_a_caller_supplied_typed_value_not_an_ambient_disk_read`] here, and exhaustively by `g1_crash_recovery_evidence.rs` for the daemon side |
//! | **cancellation** | semantic core, landed directly | [`continuum_task::region::RegionTree::cancel`] — a typed request (`Result<(), RegionFault>`), subtree-wide and monotone; a refused subsequent operation is a named [`continuum_task::region::RegionFault`] variant, never a panic | no source-scan rule (same reason as faults); enforced behaviorally — [`positive_cancellation_and_scheduling_reach_region_calculus_code_as_typed_values_never_a_panic`] here, and exhaustively by `continuum-task`'s own `region_cancellation.rs`/`region_no_orphan.rs` and `continuumd`'s `daemon_task_regions.rs`/`dx14_cancellation_matrix.rs` |
//!
//! Two honest gaps the table above states rather than hides: **entropy** and **time**
//! have no landed *positive* capability type yet (the crates that would own one —
//! `continuum-effects-time`, the optimized search engines, `continuum-forge` — are all
//! still without a search that would need one: the time pack declares every clock
//! semantic unsupported with no operation, bn-1oj6, the three engines are
//! PR-1/IMPL-01 scaffolds per their own module docs, and `continuum-forge` holds only
//! the task-assembly lane bn-1dsih landed, which enumerates nothing). What is real today is the
//! *prohibition*: `GOV-1-04` already scans those very crates' `src/` for a
//! clock or an RNG, and its `gov-1-04-thread-rng-in-engine` fixture plants exactly a
//! `thread_rng` inside the engine tier — the tier most likely to reach for one when
//! search lands — and proves the scan catches it there specifically, not only in the
//! kernel. **faults** and **cancellation** have no *mechanical* (source-scan) enforcement
//! at all, by design: there is no forbidden symbol for "faults arrived ambiently", only a
//! behavior to demonstrate, which is why this file carries positive tests for those two
//! rather than citing a GOV rule.
//!
//! # What already closes most of this, cited rather than rebuilt
//!
//! - `GOV-1-01` toolchain-pinned / `--locked` gate discipline / `overflow-checks` — the
//!   artifact-determinism half of the story: a build is reproducible before a single
//!   ambient is even considered (`tools/governance/check_t12_evidence.py`'s "reproducible
//!   build metadata" composite already binds these three together; not restated here).
//! - `GOV-1-03` deterministic-collections and `GOV-1-05` no-platform-hashing — the two
//!   *representation* determinism rules that make "byte-identical" a meaningful claim for
//!   anything computed in the core at all; INV-005's ambients are what a computation
//!   *reaches for*, GOV-1-03/05 are what a computation's own output *is made of*, and a
//!   claim of byte-identical output down the road implicitly leans on both.
//! - The crate tier partition (`SEMANTIC_CORE`/`BOUNDARY`/`ADAPTERS` in
//!   `check_code_policy.py`) — "owning an ambient resource behind a declared capability
//!   is \[the boundary tier's\] job" is the structural half of INV-005; this file's audit
//!   table is that same partition, read ambient-by-ambient instead of crate-by-crate.
//! - `crates/continuum-engine-reference/src/lib.rs`'s own "Determinism (INV-005)" section
//!   and `crates/continuum-task/src/region.rs`'s own "Determinism: concurrency is data"
//!   section — both already argue, in the crate that carries the property, why their own
//!   design makes the relevant ambient a value rather than an ambient read. Neither
//!   argument is repeated here; both are exercised (this file, and the cross-crate
//!   addition in `continuum-engine-reference`'s own test suite).
//! - The byte-identity evidence already in the tree: `bfs_diehard.rs`'s
//!   `two_explorations_of_die_hard_are_byte_identical` and
//!   `two_bounded_explorations_of_die_hard_are_byte_identical`,
//!   `diehard_evidence.rs`'s `two_walks_of_die_hard_are_byte_identical`,
//!   `witness_diehard.rs`'s `two_extractions_of_the_solution_are_byte_identical`,
//!   `model_contract.rs`'s `two_enumerations_of_the_same_model_are_byte_identical` (all
//!   `continuum-engine-reference`, single-crate), and `inv006_replay_stability_evidence.rs`
//!   / `pr8_exit_evidence.rs`'s
//!   `positive_the_whole_frame_sequence_replayed_from_a_fresh_daemon_is_byte_identical`
//!   (two independently built fresh `Daemon`s, one scripted Die Hard campaign, byte-identical
//!   wire transcripts — already spans `continuumd`, `continuum-engine-reference`,
//!   `continuum-workspace`, `continuum-task` and `continuum-value` in one pipeline). None
//!   of these is re-run here. What none of them do is compare two *independently checked*
//!   certificates against each other across the engine/kernel boundary specifically —
//!   `certificate_kernel_differential.rs`'s new
//!   `two_independent_pipelines_from_a_declared_model_to_a_checked_certificate_are_byte_identical`
//!   closes exactly that gap.
//!
//! # Clause → test map
//!
//! - **"scheduling … through explicit capabilities"** —
//!   [`positive_the_crate_boundary_checker_proves_the_trusted_checking_base_is_synchronous_and_search_free`]
//!   (mechanical: `kernel-is-synchronous`),
//!   [`positive_cancellation_and_scheduling_reach_region_calculus_code_as_typed_values_never_a_panic`]
//!   (behavioral: a refused schedule step is a typed value).
//! - **"time … through explicit capabilities"** —
//!   [`positive_gov_1_04_self_test_still_catches_exactly_its_five_named_fixtures`],
//!   [`positive_the_real_run_of_gov_1_03_04_05_and_06_finds_zero_semantic_core_violations`].
//! - **"entropy … through explicit capabilities"** — the same two GOV-1-04 tests, plus
//!   [`positive_entropy_reaches_a_capability_token_only_as_a_caller_supplied_identity_never_ambient_randomness`]
//!   (behavioral).
//! - **"I/O … through explicit capabilities"** —
//!   [`positive_the_real_run_of_gov_1_03_04_05_and_06_finds_zero_semantic_core_violations`]
//!   (`GOV-1-06`).
//! - **"faults … through explicit capabilities"** —
//!   [`positive_faults_reach_the_store_only_as_a_caller_supplied_typed_value_not_an_ambient_disk_read`]
//!   (behavioral; no mechanical rule exists for this ambient, stated above).
//! - **"cancellation … through explicit capabilities"** —
//!   [`positive_cancellation_and_scheduling_reach_region_calculus_code_as_typed_values_never_a_panic`].
//! - **the audit table itself, against drift** —
//!   [`anti_drift_the_plan_md_heading_still_names_these_six_ambients_in_this_order`],
//!   [`the_check_is_not_vacuous`] (the anti-drift comparison is proven to have teeth: a
//!   mutant sentence that drops one ambient is not silently accepted).
//! - **the committed GOV-1 evidence artifact does not go stale next to a live run** —
//!   [`boundary_the_committed_gov_1_evidence_artifact_still_agrees_with_a_live_self_test_run`].
//!
//! # Home crate, and why
//!
//! `continuumd` — the same home `INV-002`, `INV-006`, and `INV-016`'s evidence files
//! already use, for the same reason: INV-005 is not one crate's property, it is a
//! constraint on the whole 27-crate semantic core plus the boundary tier that is allowed
//! to touch what the core may not, and `continuumd` is the one crate that already
//! (normal, not merely dev) depends on the two crates that carry the landed positive
//! seams this audit cites — `continuum-task` (region calculus: scheduling, cancellation)
//! and `continuum-workspace` (the store: I/O, faults, and — via `CapabilityToken` —
//! entropy) — plus `continuum-engine-reference` (the search tier's determinism story).
//! `continuumd`'s own `src/daemon/recovery.rs` already imports
//! `continuum_workspace::publication::{StorageFaults, PublicationPhase, …}` directly for
//! its G1 crash-recovery machinery, so this file is not reaching for a capability its
//! crate does not already use in earnest. The "governance-adjacent, reads a checker's
//! JSON" half of this file reuses `continuumd::codec::json::Json` — the daemon's own wire
//! JSON codec, already used the same way by `inv016_untrusted_source_evidence.rs` in this
//! same crate — rather than adding a JSON dependency for one test file.
//!
//! # OOM hygiene
//!
//! Four subprocess invocations total (`check_code_policy.py` self-test and real run,
//! `check_crate_boundaries.py` self-test and real run — the last shells out to
//! `cargo metadata --no-deps`, already offline and already run by `just check`'s own
//! `boundaries` recipe). No loop here multiplies a string or a collection; the region
//! tree spawns exactly one worker, and both stores publish one short, `const` byte
//! string each.

use std::process::Command;
use std::sync::Arc;

use continuumd::codec::json::Json;

use continuum_task::region::worker::Resumability;
use continuum_task::region::{DrainCause, RegionFault, RegionState, RegionTree};

use continuum_workspace::artifact_path::{ArtifactClass, ArtifactHandle};
use continuum_workspace::publication::{
    AbortReason, ActorId, AuditLog, AuthorityLevel, CapabilityDescriptor, CapabilityToken,
    ContentIdentifier, IdentityUnavailable, NoFaults, PublicationPhase, PublishRefusal,
    ReferenceStore, StorageFaults,
};

const CHECK_CODE_POLICY: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tools/governance/check_code_policy.py"
);
const CHECK_CRATE_BOUNDARIES: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tools/check_crate_boundaries.py"
);
const REPO_ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../..");
const COMMITTED_GOV_1_EVIDENCE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tools/governance/evidence/gov-1.json"
);

// =====================================================================================
// Governance checkers, re-run live, read through the daemon's own wire JSON codec
// =====================================================================================

/// Strip the whitespace `json.dumps(..., indent=2)` inserts between tokens, so
/// [`Json::parse`] — which accepts the wire protocol's strict canonical form and, by the
/// same rule, none of Python's pretty-printing — can read a governance checker's report
/// without this file taking on a second JSON parser. Whitespace *inside* a string (a
/// checker's own message text) is data and is left untouched; only whitespace outside a
/// string is insignificant here.
fn compact_json(pretty: &str) -> String {
    let mut out = String::with_capacity(pretty.len());
    let mut in_string = false;
    let mut escaped = false;
    for ch in pretty.chars() {
        if in_string {
            out.push(ch);
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                in_string = false;
            }
            continue;
        }
        if ch == '"' {
            in_string = true;
            out.push(ch);
        } else if !ch.is_whitespace() {
            out.push(ch);
        }
    }
    out
}

/// Run a governance checker exactly as `Justfile`'s `governance`/`boundaries` recipes
/// invoke it, and parse its JSON report through the daemon's own wire codec.
fn run_json(program: &str, args: &[&str]) -> Json {
    let output = Command::new("python3")
        .arg(program)
        .args(args)
        .current_dir(REPO_ROOT)
        .output()
        .unwrap_or_else(|err| panic!("failed to spawn `python3 {program}`: {err}"));
    let stdout = String::from_utf8(output.stdout).expect("checker stdout is UTF-8");
    Json::parse(compact_json(&stdout).as_bytes()).unwrap_or_else(|err| {
        panic!("`python3 {program} {args:?}` output did not parse as JSON ({err:?}):\n{stdout}")
    })
}

fn obj(value: &Json) -> &std::collections::BTreeMap<String, Json> {
    value.as_object().expect("expected a JSON object")
}

fn strs(value: &Json) -> Vec<&str> {
    value
        .as_array()
        .expect("expected a JSON array")
        .iter()
        .map(|item| item.as_str().expect("expected a JSON string"))
        .collect()
}

/// `GOV-1-04`'s self-test replays five violating fixtures — a clock dependency, an
/// ambient-crate dependency, an environment read, a bare `SystemTime` inside the kernel,
/// and a `thread_rng` specifically inside the engine tier — through the real rule and
/// fails if any goes uncaught. This test re-runs that self-test live and pins the exact
/// fixture set by name, so a fixture quietly dropped (which would silently narrow
/// INV-005's negative evidence) is a failure here even though `check_code_policy.py`
/// itself would still report "pass".
#[test]
fn positive_gov_1_04_self_test_still_catches_exactly_its_five_named_fixtures() {
    let report = run_json(CHECK_CODE_POLICY, &["--self-test"]);
    let top = obj(&report);
    assert_eq!(
        top.get("status").and_then(Json::as_str),
        Some("pass"),
        "check_code_policy.py --self-test does not pass cleanly: {report:?}"
    );

    let by_requirement = obj(top
        .get("fixtures_by_requirement")
        .expect("self-test report names fixtures_by_requirement"));
    let gov_1_04 = strs(
        by_requirement
            .get("GOV-1-04")
            .expect("GOV-1-04 has a fixtures_by_requirement entry"),
    );
    assert_eq!(
        gov_1_04,
        [
            "gov-1-04-ambient-dependency",
            "gov-1-04-clock-dependency",
            "gov-1-04-environment-read-in-core",
            "gov-1-04-systemtime-in-kernel",
            "gov-1-04-thread-rng-in-engine",
        ],
        "GOV-1-04's caught-fixture set changed; INV-005's negative evidence is pinned to \
         exactly these five planted violations"
    );
}

/// The live (non-self-test) run: `GOV-1-03` (deterministic collections — the
/// representation half of "byte-identical" being meaningful), `GOV-1-04` (no ambient
/// time/RNG/environment — the **time** and **entropy** ambients), and `GOV-1-06` (no
/// network in the certificate checker — the **I/O** ambient) all report zero violations
/// against the real tree, not a fixture.
#[test]
fn positive_the_real_run_of_gov_1_03_04_05_and_06_finds_zero_semantic_core_violations() {
    let report = run_json(CHECK_CODE_POLICY, &[]);
    let top = obj(&report);
    let passing = strs(top.get("passing").expect("real run reports a passing list"));
    for rid in ["GOV-1-03", "GOV-1-04", "GOV-1-05", "GOV-1-06"] {
        assert!(
            passing.contains(&rid),
            "{rid} is not in the live check_code_policy.py run's `passing` list: {report:?}"
        );
    }
    let failing = obj(top.get("failing").expect("real run reports a failing map"));
    assert!(
        failing.is_empty(),
        "check_code_policy.py reports live violations: {failing:?}"
    );
}

/// `tools/governance/evidence/gov-1.json` is a committed artifact, not regenerated by
/// `just check`'s own `governance` recipe (which runs the checker without `--evidence`).
/// It can therefore go stale relative to a live rule or fixture change. This test compares
/// the committed artifact's `GOV-1-04` record against a live self-test run rather than
/// against a hardcoded literal, so drift between the two is caught here rather than
/// silently shipped.
#[test]
fn boundary_the_committed_gov_1_evidence_artifact_still_agrees_with_a_live_self_test_run() {
    let live = run_json(CHECK_CODE_POLICY, &["--self-test"]);
    let live_fixtures = strs(
        obj(obj(&live)
            .get("fixtures_by_requirement")
            .expect("live self-test names fixtures_by_requirement"))
        .get("GOV-1-04")
        .expect("live self-test names GOV-1-04's fixtures"),
    );

    let committed_text = std::fs::read_to_string(COMMITTED_GOV_1_EVIDENCE)
        .expect("tools/governance/evidence/gov-1.json is a committed, readable artifact");
    let committed = Json::parse(compact_json(&committed_text).as_bytes())
        .expect("the committed GOV-1 evidence artifact is valid JSON");
    let committed_gov_1_04 = obj(obj(obj(&committed)
        .get("requirements")
        .expect("committed evidence names requirements"))
    .get("GOV-1-04")
    .expect("committed evidence names GOV-1-04"));

    assert_eq!(
        committed_gov_1_04.get("result").and_then(Json::as_str),
        Some("pass"),
        "the committed GOV-1-04 record does not say pass: {committed_gov_1_04:?}"
    );
    let committed_fixtures = strs(
        committed_gov_1_04
            .get("fixtures_proven_caught")
            .expect("committed GOV-1-04 record names fixtures_proven_caught"),
    );
    assert_eq!(
        committed_fixtures, live_fixtures,
        "tools/governance/evidence/gov-1.json is stale relative to a live self-test run; \
         regenerate it with `python3 tools/governance/check_code_policy.py --evidence \
         tools/governance/evidence/gov-1.json`"
    );
}

/// `kernel-is-synchronous` (the **scheduling** ambient, at the trusted-checking-base
/// tier) and `certificate-checker-not-search` both hold against the live workspace graph,
/// re-derived from `cargo metadata` rather than read off a fixture.
#[test]
fn positive_the_crate_boundary_checker_proves_the_trusted_checking_base_is_synchronous_and_search_free()
 {
    let self_test = run_json(CHECK_CRATE_BOUNDARIES, &["--self-test"]);
    assert_eq!(
        obj(&self_test).get("self_test").and_then(Json::as_str),
        Some("pass"),
        "check_crate_boundaries.py --self-test did not pass: {self_test:?}"
    );

    let real = run_json(CHECK_CRATE_BOUNDARIES, &[]);
    let top = obj(&real);
    assert_eq!(
        top.get("status").and_then(Json::as_str),
        Some("pass"),
        "check_crate_boundaries.py's live run did not pass: {real:?}"
    );
    let rules = strs(
        top.get("rules")
            .expect("live run names the rules it enforced"),
    );
    for rule in ["kernel-is-synchronous", "certificate-checker-not-search"] {
        assert!(
            rules.contains(&rule),
            "{rule} was not among the rules this live run enforced: {rules:?}"
        );
    }
    let violations = strs(
        top.get("violations")
            .expect("live run names its violations"),
    );
    assert!(
        violations.is_empty(),
        "the crate-boundary checker found live violations: {violations:?}"
    );
}

// =====================================================================================
// Behavioral seams: cancellation/scheduling, faults, entropy
// =====================================================================================

/// Scheduling and cancellation, together, at the seam that actually carries them
/// (`continuum-task::region`): a run is `&mut self`-driven data, not a thread or an
/// executor, and cancellation is a typed request whose refusal of a subsequent operation
/// is a named [`RegionFault`] variant — never a panic, never a silently-ignored call.
#[test]
fn positive_cancellation_and_scheduling_reach_region_calculus_code_as_typed_values_never_a_panic() {
    let mut tree = RegionTree::new();
    let root = tree.root();
    assert_eq!(tree.state(root), Ok(RegionState::Open));

    tree.spawn(root, Resumability::Resumable)
        .expect("the root region accepts work while it is open");

    tree.cancel(root)
        .expect("cancellation is a legal request against an open region");
    assert_eq!(
        tree.state(root),
        Ok(RegionState::Draining(DrainCause::Cancelled))
    );

    // There is no ambient scheduler to consult after cancellation — the call is
    // answered synchronously, by value, from the tree's own state alone.
    let refused = tree.spawn(root, Resumability::Resumable);
    assert_eq!(
        refused,
        Err(RegionFault::SpawnIntoClosedRegion {
            region: root,
            state: RegionState::Draining(DrainCause::Cancelled),
        }),
        "spawning into a cancelled region must be refused as a typed RegionFault, not a panic"
    );
}

struct LenIdentifier;

impl ContentIdentifier for LenIdentifier {
    fn identify(
        &self,
        class: ArtifactClass,
        content: &[u8],
    ) -> Result<ArtifactHandle, IdentityUnavailable> {
        ArtifactHandle::new(class, &format!("inv005len{}", content.len()))
            .map_err(|_| IdentityUnavailable)
    }
}

/// A fault-injection value supplied from *outside* `continuum-workspace` — exactly the
/// seam INV-005 requires: this type is declared in this test file, never by the store
/// itself, and the store's behavior is entirely a function of which `StorageFaults` value
/// it was built with.
#[derive(Debug, Clone, Copy)]
struct AbortAtStaging;

impl StorageFaults for AbortAtStaging {
    fn check(&self, phase: PublicationPhase) -> Result<(), AbortReason> {
        if phase == PublicationPhase::Staging {
            Err(AbortReason::StorageExhausted)
        } else {
            Ok(())
        }
    }
}

fn store_with(faults: impl StorageFaults + 'static) -> (ReferenceStore, CapabilityToken) {
    let author = CapabilityToken::mint("inv-005-author")
        .expect("`inv-005-author` is a well-formed capability identity");
    let store = ReferenceStore::builder(LenIdentifier, Arc::new(AuditLog::new()))
        .faults(faults)
        .capability(CapabilityDescriptor::new(
            author.clone(),
            ActorId::new("author"),
            AuthorityLevel::Propose,
        ))
        .build();
    (store, author)
}

/// The **faults** ambient: the *same* store code path, the *same* content, and the
/// *only* thing that differs between the two runs below is which `StorageFaults` value
/// the caller injected. Faults reach the store as an explicit, typed capability — never
/// an ambient disk read this test would have to fake a filesystem to observe.
#[test]
fn positive_faults_reach_the_store_only_as_a_caller_supplied_typed_value_not_an_ambient_disk_read()
{
    let (clean, author) = store_with(NoFaults);
    let receipt = clean
        .publish(
            ArtifactClass::Evidence,
            b"inv-005-content".to_vec(),
            &author,
        )
        .expect("NoFaults never aborts a publication");
    assert_eq!(
        clean
            .read(receipt.handle(), &author)
            .expect("the publication succeeded and is readable"),
        b"inv-005-content"
    );

    let (faulty, author) = store_with(AbortAtStaging);
    let refusal = faulty
        .publish(
            ArtifactClass::Evidence,
            b"inv-005-content".to_vec(),
            &author,
        )
        .expect_err("AbortAtStaging must abort every publication at the staging phase");
    match refusal {
        PublishRefusal::Aborted(aborted) => {
            assert_eq!(aborted.phase(), PublicationPhase::Staging);
            assert_eq!(aborted.reason(), AbortReason::StorageExhausted);
        }
        other => panic!("expected PublishRefusal::Aborted(..), got {other:?}"),
    }
}

/// The **entropy** ambient: `CapabilityToken::mint` takes the identity as an argument.
/// Minting the same caller-supplied identity twice, independently, must agree — which
/// would not hold if the token type drew its own randomness — and two different
/// caller-supplied identities must disagree.
#[test]
fn positive_entropy_reaches_a_capability_token_only_as_a_caller_supplied_identity_never_ambient_randomness()
 {
    let a = CapabilityToken::mint("inv-005-fixed-identity")
        .expect("`inv-005-fixed-identity` is well-formed");
    let b = CapabilityToken::mint("inv-005-fixed-identity")
        .expect("`inv-005-fixed-identity` is well-formed");
    assert_eq!(
        a, b,
        "minting the same caller-supplied identity twice produced different tokens; \
         entropy must not be reaching this type ambiently"
    );

    let c = CapabilityToken::mint("inv-005-different-identity")
        .expect("`inv-005-different-identity` is well-formed");
    assert_ne!(
        a, c,
        "two different caller-supplied identities minted equal tokens"
    );
}

// =====================================================================================
// Anti-drift: the audit table above, held against plan.md's own sentence
// =====================================================================================

/// The six ambients this file's audit table names, in plan.md's own order.
const SIX_AMBIENTS: [&str; 6] = [
    "scheduling",
    "time",
    "entropy",
    "I/O",
    "faults",
    "cancellation",
];

/// The pinned INV-005 contract sentence, verbatim, per `notes/plan/plan.md:325`.
const CONTRACT_SENTENCE: &str = "Controlled code accesses scheduling, time, entropy, I/O, \
faults, and cancellation through explicit capabilities.";

/// Extract the comma/and-separated nouns between "accesses" and "through explicit
/// capabilities" from one INV-005 contract sentence. Returns `None` if the sentence's
/// shape no longer matches — a rewritten sentence is reported, not guessed at.
fn extract_ambients(sentence: &str) -> Option<Vec<String>> {
    let middle = sentence
        .strip_prefix("Controlled code accesses ")?
        .strip_suffix(" through explicit capabilities.")?;
    Some(
        middle
            .replace(", and ", ", ")
            .split(", ")
            .map(str::trim)
            .map(str::to_owned)
            .collect(),
    )
}

/// This file's audit table is written against the exact wording of plan.md's INV-005
/// sentence. If that sentence is ever reworded — an ambient added, removed, renamed, or
/// reordered — this test fails, forcing the audit table (and the rest of this file) to be
/// revisited rather than silently drifting out of sync with its own authority.
#[test]
fn anti_drift_the_plan_md_heading_still_names_these_six_ambients_in_this_order() {
    let plan_path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../notes/plan/plan.md");
    let plan = std::fs::read_to_string(plan_path).expect("notes/plan/plan.md is readable");
    assert!(
        plan.contains(CONTRACT_SENTENCE),
        "notes/plan/plan.md no longer contains the INV-005 contract sentence verbatim; \
         this file's audit table is written against exact wording that has since changed"
    );

    let ambients = extract_ambients(CONTRACT_SENTENCE).expect(
        "the pinned sentence has the expected \"accesses … through explicit capabilities\" shape",
    );
    assert_eq!(
        ambients, SIX_AMBIENTS,
        "the audit table above names {SIX_AMBIENTS:?}; plan.md's own sentence now names {ambients:?}"
    );
}

/// A mutant of this file's own anti-drift comparison, in the `idl_conformance.rs`
/// `the_check_is_not_vacuous` style: a sentence with `cancellation` quietly dropped must
/// not compare equal to [`SIX_AMBIENTS`]. If it did, the anti-drift test above would pass
/// against any sentence naming six-or-fewer ambients, which would make it worthless.
#[test]
fn the_check_is_not_vacuous() {
    let mutant = "Controlled code accesses scheduling, time, entropy, I/O, and faults \
through explicit capabilities.";
    let ambients =
        extract_ambients(mutant).expect("the mutant sentence keeps the same overall shape");
    assert_eq!(
        ambients.len(),
        5,
        "the mutant should read as five ambients, not silently pad or truncate to six"
    );
    assert_ne!(
        ambients, SIX_AMBIENTS,
        "a mutant sentence that drops `cancellation` was not caught by the comparison; \
         the anti-drift check above would be vacuous"
    );

    // And the shape guard itself is not vacuous either: a sentence that does not even
    // have the pinned prefix/suffix must report `None`, not silently misparse.
    assert_eq!(
        extract_ambients("An entirely different sentence."),
        None,
        "a sentence with none of the pinned shape was not reported as unparseable"
    );
}
