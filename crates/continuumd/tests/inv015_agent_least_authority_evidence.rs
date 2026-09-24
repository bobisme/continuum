//! Dedicated evidence map for `INV-015` — Agent least authority
//! (`notes/plan/plan.md:364`, the invariant bone `bn-3mjd`).
//!
//! > Agents cannot alter evidence status, sign receipts, access ungranted production
//! > traces, or execute unrestricted host effects.
//! >
//! > — `notes/plan/plan.md`, INV-015's contract sentence
//!
//! # The clause map: each clause → its live enforcement sites
//!
//! | INV-015 clause | live site | direct evidence |
//! |---|---|---|
//! | alter evidence status | `daemon/evidence.rs` — [`Promotion`] has private fields and no public constructor, so producer code (`daemon/observe.rs`, a *different module*) can name the type and never build one; `observe.ingest`'s wire shape declares no status field, so an append lands at the lattice's bottom as a property of the request's *type* | `daemon_evidence.rs`: `a_producers_append_lands_at_the_lattices_bottom`, `the_producers_request_body_has_no_field_that_could_name_a_status`, `the_status_written_is_what_the_checker_established_not_what_the_caller_named`, `no_operation_in_either_family_removes_or_edits_an_appended_node` — cited via [`status_authority`], plus this file's own registry-grain sweep [`privileged_perimeter::positive_no_mutation_request_admits_a_caller_supplied_status`] |
//! | sign receipts | `evidence.link` — the checker is the *admitted capability's* actor (there is no checker request field), the actor must be a `service:` scheme, and self-certification is refused before the receipt is read (RFC 0038 D3) | `daemon_evidence.rs`: `only_a_service_actor_may_append_a_check_edge`, `a_checker_may_not_record_a_check_of_its_own_production` — cited via [`receipt_authority`]; cryptographic signing (plan §18.6, bn-1hape): the daemon holds the key, installed only by the deployment, and signs only inside `evidence.link` after the service gate; no wire type carries key material — [`receipt_authority::positive_only_the_daemon_holds_the_receipt_key_and_only_a_service_check_signs`] |
//! | access ungranted production traces | `daemon/admission.rs` — R-4: `observe.ingest` requires `DataGrant::ProductionTrace` beyond its `execute` level, decided by [`required_grant`] *before* any family runs; the denial is the zero-bit [`Denied`] (X1) and precedes the index (X3), so a refused caller learns nothing (X2) | live here: [`trace_grant::positive_exactly_one_operation_requires_a_data_grant_and_it_is_the_production_trace`], [`trace_grant::positive_a_denial_carries_zero_bits`]; cited: `the_production_trace_grant_is_required_beyond_the_execute_level`, `every_admission_failure_is_one_byte_identical_answer`, `a_promotion_of_a_claim_that_does_not_exist_is_byte_identical_to_one_that_does` |
//! | execute unrestricted host effects | structurally: the 83-operation registry has **no host-execution verb and no `capability` namespace** (RFC 0026 correction 20: "no operation in this protocol can widen the authority of the connection that invokes it"); `ReferenceStore::mint`/`revoke` have no wire caller (swept live over every `continuumd` source); every remaining host-effect crate (`continuum-effects-time`, `continuum-proof-client`, `continuum-security`) is a zero-pub-item scaffold, pinned to go red when the substance arrives; `continuum-effects-network` grew its Lab pack at bn-3ohe, `continuum-effects-process` its Lab pack at bn-3mmf, and `continuum-effects-storage` its Lab pack at bn-2fk3, and each is audited like Forge — a recorded public inventory, no host effect by the compiler (`#![no_std]` and a live compiler lane), no dependencies, and no route from a connection into it; `continuum-forge` grew its first public surface at bn-1dsih and is audited rather than grandfathered — a recorded public inventory, zero host-effect facilities named in its code, one verifier-side dependency, and no route from a connection into it: the declared `forge.*` vocabulary has no registered family and answers `UnsupportedSemanticFeature`, the daemon does not link the crate, and no `continuumd` source names it | live here: [`privileged_perimeter::positive_the_namespace_set_is_closed_and_contains_no_capability_namespace`], [`no_widening`] |
//!
//! Cross-cutting, because "no ambient authority" is a property of the *admission
//! predicate* rather than of any one clause: the `@privileged` set is exactly the five
//! governance verbs and, from protocol 3.8, the six signing-wire writes, and nothing else
//! ([`privileged_perimeter`]), every one of them is
//! `@audit_recorded` (docs/49: "logs attempted privileged operations"), an absent
//! capability profile is the fail-closed reading ("grants nothing rather than
//! everything"), and delegation only narrows — all pinned against the sources that
//! enforce them, with the already-landed live tests cited by name.
//!
//! # docs/49's authority rows, sited
//!
//! `bn-34je`'s INV-001 suite already pinned the "revise intent: proposal only" row
//! verbatim; this file cites that pin ([`contract_pins`]) rather than re-deriving it,
//! and pins the three rows that *are* INV-015's: promote evidence (`yes/checker` only),
//! access production secrets, and arbitrary network/host exec (`no` across every agent
//! column).
//!
//! # The docs/49 worker-isolation audit: eight controls, zero worker-scoped producers
//!
//! docs/49 requires Lean, solvers, corpus oracles, generated Rust, and third-party
//! analyzers to run under eight controls. **No worker execution environment exists in
//! this workspace today** — nothing launches a foreign process on an agent's behalf, so
//! every control is declared-no-producer at worker scope. Four of the eight have a live
//! *daemon/task-grain cousin* enforcing the same idea one layer up; four have none.
//! [`isolation_controls`] holds the audit as data, anti-drift-checked against docs/49's
//! own bullet list, and pins the reason this absence does not falsify the contract
//! today: plan §24.5's workbench-security frontier row keeps **autonomous promotion
//! disabled** until a Phase B red-team corpus — including "one escape attempt against
//! each of the eight worker-isolation controls of docs/49" — clears in full. The
//! red-team corpus is a Phase B G3 deliverable and does not exist today; this file does
//! not pretend otherwise.
//!
//! # The adapter surface (`continuum-mcp`), cited not re-derived
//!
//! research/25's semantic action grammar (`register::OPERATIONS`) is a curated
//! eight-row subset of the 75, its preconditions are *necessary* handle requirements
//! and never authority (the daemon is the authority), and the client can narrow which
//! principal it speaks as but never widen what that principal may do —
//! [`mcp_grammar`] pins the rows, the narrowing sentences, and `typed_surface.rs`'s
//! own live INV-015 test, all via `include_str!` (the inv008/inv014 no-new-dependency
//! device).
//!
//! # Honest gaps, typed as absences (the INV-008 shape), each pinned to go red
//!
//! 1. **Worker isolation has no worker.** All eight docs/49 controls are
//!    declared-no-producer at worker scope ([`isolation_controls`]); the scaffold
//!    sweep ([`no_widening::boundary_the_host_effect_surface_has_not_arrived_and_its_crates_are_scaffolds`])
//!    goes red the moment any effects/proof-client/security crate grows a public item,
//!    forcing this audit to be redone against a surface that then exists. `continuum-forge`
//!    has already left that list: bn-1dsih landed its task-assembly lane — the caller
//!    RFC 0037 AO2 names for INV-012's non-vacuity obligation — and
//!    [`no_widening::boundary_forge_grew_a_task_assembly_lane_and_it_reaches_no_host_effect`]
//!    re-establishes for that real surface what emptiness used to assert. The audit's
//!    conclusion is unchanged and its basis is not: Forge's surface decides whether a
//!    contract may become a synthesis task, it names no host-effect facility, it
//!    declares one verifier-side dependency, and no route runs from an agent connection
//!    to it — the `forge.*` operations the registry has always declared still have no
//!    family answering them, and the daemon neither links nor names the crate. Nothing
//!    about worker isolation is discharged by that lane; the worker itself is still
//!    absent.
//! 2. **`continuum-security` is a PR-1/IMPL-01 scaffold.** The crate plan §18 names
//!    for "capability model, sandboxing … audit trail, and signing identities" has
//!    zero public items; the live capability model ships in `continuumd`
//!    (admission/capability) and `continuum-workspace` (publication) instead — pinned.
//! 3. **Signing identities: receipts are signed, nothing is verified yet** (plan §18.6,
//!    docs/09, bn-1hape). `evidence.link` signs each receipt it publishes with a key the
//!    deployment installs through `Builder::receipt_signer` and the daemon holds; the
//!    keystore and OS entropy live in `continuum-security`, which the daemon neither links
//!    nor names. An acceptance signature is the end of an RFC 0037 A1 signature chain
//!    the daemon verifies before a bundle acceptance and signs itself on a local one
//!    (bn-3glnv, cr-2unxyh); a caller's text is recorded verbatim only when no key
//!    signs — swept live.
//! 4. **The §18.4 capture-time contract has no producer**: salted payload commitments,
//!    per-artifact encryption, and the retention clock are unimplemented ("salted"
//!    appears in no `continuumd` source). The *redaction* half is live and cited
//!    (typed stubs + INV-007 omission manifests, `daemon_evidence.rs`).
//! 5. **`ServerLimits.max_result_bytes` is declared, not enforced**: the field exists
//!    on the handshake and no `daemon/` source reads it — swept live, so the sweep
//!    fails (and this row flips) when enforcement lands.
//!
//! # Mutant story (anti-vacuity)
//!
//! [`mutants`] re-runs every evaluating leg against a doctored input and requires the
//! defect to be reported: a docs/49 copy missing one isolation bullet, a privileged
//! set with a smuggled member and with a dropped member, each load-bearing source pin
//! stripped from a copy of its source, a planted `.mint(` caller, a scaffold grown a
//! public item, and — for the Forge audit — a planted host-effect call, a comment that
//! merely mentions one, an added public item, a widened dependency set, and a planted
//! daemon caller.
//!
//! House rules: `src/` untouched, no existing test edited, no new dependency edge
//! (every cross-crate citation is `include_str!` over tracked files), deterministic
//! (fixed inputs, no clock, no entropy, no network).
//!
//! [`Promotion`]: continuumd::daemon::evidence::Promotion
//! [`Denied`]: continuumd::daemon::admission::Denied
//! [`required_grant`]: continuumd::daemon::admission::required_grant

use std::fs;
use std::path::{Path, PathBuf};

use continuumd::daemon::admission::{Denied, required_grant};
use continuumd::daemon::evidence::DEFAULT_SERVICE;
use continuumd::protocol::registry::OPERATIONS;
use continuumd::protocol::spec::Annotation;
use continuumd::protocol::vocabulary::DataGrant;

// --- the cited texts, all tracked in this repository ---------------------------------------

const PLAN_MD: &str = include_str!("../../../notes/plan/plan.md");
const DOCS_49: &str = include_str!("../../../notes/plan/docs/49_SECURITY_FOR_AUTONOMOUS_AGENTS.md");
const IDL: &str = include_str!("../../../notes/plan/schemas/continuumd-native-protocol.idl");
const RUST_TOOLCHAIN: &str = include_str!("../../../rust-toolchain.toml");
const LEAN_TOOLCHAIN: &str = include_str!("../../../lean/lean-toolchain");

const ADMISSION: &str = include_str!("../src/daemon/admission.rs");
const CAPABILITY: &str = include_str!("../src/daemon/capability.rs");
const EVIDENCE: &str = include_str!("../src/daemon/evidence.rs");
const OBSERVE: &str = include_str!("../src/daemon/observe.rs");
const OBLIGATION: &str = include_str!("../src/daemon/obligation.rs");
const STATE: &str = include_str!("../src/daemon/state.rs");
const DAEMON_MOD: &str = include_str!("../src/daemon/mod.rs");
const HANDSHAKE: &str = include_str!("../src/protocol/handshake.rs");

const DAEMON_EVIDENCE_TESTS: &str = include_str!("daemon_evidence.rs");
const DAEMON_OPERATIONS_TESTS: &str = include_str!("daemon_operations.rs");
const TASK_OPERATIONS_TESTS: &str = include_str!("daemon_task_operations.rs");
const DX14_CANCELLATION: &str = include_str!("dx14_cancellation_matrix.rs");
const INV003_EVIDENCE: &str = include_str!("inv003_no_prose_only_evidence.rs");
const INV005_EVIDENCE: &str = include_str!("inv005_ambient_nondeterminism_evidence.rs");
const INV001_EVIDENCE: &str =
    include_str!("../../continuum-intent/tests/inv001_protected_intent_evidence.rs");

const MCP_LIB: &str = include_str!("../../continuum-mcp/src/lib.rs");
const MCP_CLIENT: &str = include_str!("../../continuum-mcp/src/client.rs");
const MCP_REGISTER: &str = include_str!("../../continuum-mcp/src/register.rs");
const MCP_TYPED_SURFACE: &str = include_str!("../../continuum-mcp/tests/typed_surface.rs");
const PUBLICATION: &str = include_str!("../../continuum-workspace/src/publication.rs");

const SECURITY_LIB: &str = include_str!("../../continuum-security/src/lib.rs");
const EFFECTS_NETWORK_LIB: &str = include_str!("../../continuum-effects-network/src/lib.rs");
const EFFECTS_PROCESS_LIB: &str = include_str!("../../continuum-effects-process/src/lib.rs");
const EFFECTS_STORAGE_LIB: &str = include_str!("../../continuum-effects-storage/src/lib.rs");
const EFFECTS_TIME_LIB: &str = include_str!("../../continuum-effects-time/src/lib.rs");
const PROOF_CLIENT_LIB: &str = include_str!("../../continuum-proof-client/src/lib.rs");

/// `continuum-forge`'s manifest — the crate is no longer a zero-pub-item scaffold
/// (bn-1dsih), so what is swept about it is its *declared dependency set* rather than
/// its emptiness. Read as text, deliberately independent of the `cargo metadata` walk
/// `tools/check_crate_boundaries.py` uses, per docs/03 §8's diversity rule.
const FORGE_MANIFEST: &str = include_str!("../../continuum-forge/Cargo.toml");

/// `continuum-security`'s manifest — the same posture as [`FORGE_MANIFEST`], and for the
/// same reason: bn-ymw landed the plan §18.1 prompt-injection corpus there, so the crate
/// stopped being a zero-pub-item scaffold and what is swept about it is its declared
/// dependency set rather than its emptiness.
const SECURITY_MANIFEST: &str = include_str!("../../continuum-security/Cargo.toml");

/// `continuum-effects-network`'s manifest — the same posture as [`FORGE_MANIFEST`]:
/// bn-3ohe landed the `network/adversarial-v0` Lab pack there, so what is swept about
/// the crate is its declared dependency set rather than its emptiness.
const EFFECTS_NETWORK_MANIFEST: &str = include_str!("../../continuum-effects-network/Cargo.toml");

/// `continuum-effects-process`'s manifest — the same posture: bn-3mmf landed the
/// `process/crash-restart-v0` Lab pack there.
const EFFECTS_PROCESS_MANIFEST: &str = include_str!("../../continuum-effects-process/Cargo.toml");

/// `continuum-effects-storage`'s manifest — the same posture: bn-2fk3 landed the
/// `storage/append-log-v0` Lab pack there.
const EFFECTS_STORAGE_MANIFEST: &str = include_str!("../../continuum-effects-storage/Cargo.toml");

/// `continuumd`'s own manifest — the other end of the same edge: whether the daemon
/// links Forge at all.
const CONTINUUMD_MANIFEST: &str = include_str!("../Cargo.toml");

// --- shared helpers ------------------------------------------------------------------------

/// The first needle `text` does not contain, if any.
///
/// A pin helper rather than a bare `assert!` per needle so [`mutants`] can re-run the
/// same predicate against a doctored copy and hold it to *fail*.
fn missing_pin<'needles>(text: &str, needles: &[&'needles str]) -> Option<&'needles str> {
    needles
        .iter()
        .find(|needle| !text.contains(**needle))
        .copied()
}

/// Assert every needle is present, naming the first that is not.
fn pin(source_name: &str, text: &str, needles: &[&str]) {
    if let Some(needle) = missing_pin(text, needles) {
        panic!("{source_name} no longer contains the load-bearing text {needle:?}");
    }
}

/// The eight worker-isolation bullets of docs/49, extracted from the section's own
/// list rather than hand-copied, so a docs edit moves this file or fails it.
fn isolation_bullets(docs: &str) -> Vec<String> {
    let section = docs
        .split("## Worker isolation")
        .nth(1)
        .expect("docs/49 has a Worker isolation section")
        .split("\n## ")
        .next()
        .expect("the section ends");
    section
        .lines()
        .filter_map(|line| line.strip_prefix("- "))
        .map(|bullet| bullet.trim_end_matches([';', '.']).to_owned())
        .collect()
}

/// Every `pub` item declared at the start of a line, trimmed — the scaffold detector.
fn pub_items(source: &str) -> Vec<&str> {
    source
        .lines()
        .map(str::trim_start)
        .filter(|line| {
            line.starts_with("pub ") || line.starts_with("pub(") || line.starts_with("pub<")
        })
        .collect()
}

/// Every line of `source` that calls the store-side administrative surface.
fn administrative_callers(source: &str) -> Vec<&str> {
    source
        .lines()
        .filter(|line| !line.trim_start().starts_with("//"))
        .filter(|line| line.contains(".mint(") || line.contains(".revoke("))
        .collect()
}

/// The exact `.mint(`/`.revoke(` call sites protocol 3.8's signer administration makes, in
/// `daemon/signing.rs` alone (bn-3glnv). Each is a call on the signing authority's own
/// `SigningRegistry` — the line above it is `.registry` — and a signer is not a capability,
/// so none is the capability-administration surface this sweep guards. Any other
/// `.mint(`/`.revoke(` in that file, and any at all elsewhere, still reports.
const SIGNER_ADMINISTRATION: [&str; 2] =
    [".mint(&actor, entropy)", ".revoke(&named, &actor, reason)"];

/// The administrative callers in `source` that are not the signer-administration sites,
/// for `daemon/signing.rs`; exactly [`administrative_callers`] for every other file.
fn unexempted_callers<'a>(path: &Path, source: &'a str) -> Vec<&'a str> {
    let callers = administrative_callers(source);
    if !path.ends_with("src/daemon/signing.rs") {
        return callers;
    }
    // Judged by each caller's own line index, so a second identical call elsewhere is not
    // exempted by the first one's `.registry` line.
    let lines: Vec<&str> = source.lines().collect();
    let unexempted: Vec<&str> = lines
        .iter()
        .enumerate()
        .filter(|(_, line)| callers.contains(line))
        .filter(|(at, line)| {
            let exempt = SIGNER_ADMINISTRATION.contains(&line.trim())
                && at
                    .checked_sub(1)
                    .is_some_and(|above| lines[above].trim() == ".registry");
            !exempt
        })
        .map(|(_, line)| *line)
        .collect();
    unexempted
}

/// All `.rs` sources under `dir`, recursively — a real directory walk (the inv003
/// device), so a *new* daemon module is swept without anyone remembering to list it.
fn rust_sources(dir: &Path, into: &mut Vec<(PathBuf, String)>) {
    let mut entries: Vec<PathBuf> = fs::read_dir(dir)
        .unwrap_or_else(|error| panic!("{} is readable: {error}", dir.display()))
        .map(|entry| entry.expect("directory entry").path())
        .collect();
    entries.sort();
    for path in entries {
        if path.is_dir() {
            rust_sources(&path, into);
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            let text = fs::read_to_string(&path)
                .unwrap_or_else(|error| panic!("{} is readable: {error}", path.display()));
            into.push((path, text));
        }
    }
}

/// Every source file of `continuum-forge`, walked rather than listed, so a module added
/// to that crate is swept without anyone remembering to name it here.
fn forge_sources() -> Vec<(PathBuf, String)> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../continuum-forge/src");
    let mut sources = Vec::new();
    rust_sources(&root, &mut sources);
    assert!(
        !sources.is_empty(),
        "the walk found no continuum-forge source; the crate moved and this sweep is \
         reporting nothing rather than checking something"
    );
    sources
}

/// Every source file of `continuum-security`, walked rather than listed — the same device
/// as [`forge_sources`], so a module added to the corpus crate is swept without anyone
/// remembering to name it here.
fn security_sources() -> Vec<(PathBuf, String)> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../continuum-security/src");
    let mut sources = Vec::new();
    rust_sources(&root, &mut sources);
    assert!(
        !sources.is_empty(),
        "the walk found no continuum-security source; the crate moved and this sweep is \
         reporting nothing rather than checking something"
    );
    sources
}

/// Every source file of `continuum-effects-network`, walked rather than listed — the
/// same device as [`forge_sources`].
fn effects_network_sources() -> Vec<(PathBuf, String)> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../continuum-effects-network/src");
    let mut sources = Vec::new();
    rust_sources(&root, &mut sources);
    assert!(
        !sources.is_empty(),
        "the walk found no continuum-effects-network source; the crate moved and this \
         sweep is reporting nothing rather than checking something"
    );
    sources
}

/// The network pack's compiler lane (bn-3ohe, cr-1dl1d7).
const EFFECTS_NETWORK_NO_STD_LANE: &str =
    include_str!("../../continuum-effects-network/tests/pr15_no_std_lane.rs");

/// The name of the network lane's test.
const NETWORK_NO_STD_LANE_TEST: &str =
    "fn the_compiler_refuses_a_host_facility_in_the_network_pack()";

/// Every source file of `continuum-effects-process`, walked rather than listed — the
/// same device as [`effects_network_sources`] (bn-3mmf).
fn effects_process_sources() -> Vec<(PathBuf, String)> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../continuum-effects-process/src");
    let mut sources = Vec::new();
    rust_sources(&root, &mut sources);
    assert!(
        !sources.is_empty(),
        "the walk found no continuum-effects-process source; the crate moved and this \
         sweep is reporting nothing rather than checking something"
    );
    sources
}

/// The process pack's compiler lane (bn-3mmf), the network lane applied to that crate.
const EFFECTS_PROCESS_NO_STD_LANE: &str =
    include_str!("../../continuum-effects-process/tests/pr15_no_std_lane.rs");

/// The name of the process lane's test.
const PROCESS_NO_STD_LANE_TEST: &str =
    "fn the_compiler_refuses_a_host_facility_in_the_process_pack()";

/// Every source file of `continuum-effects-storage`, walked rather than listed — the
/// same device as [`effects_network_sources`] (bn-2fk3).
fn effects_storage_sources() -> Vec<(PathBuf, String)> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../continuum-effects-storage/src");
    let mut sources = Vec::new();
    rust_sources(&root, &mut sources);
    assert!(
        !sources.is_empty(),
        "the walk found no continuum-effects-storage source; the crate moved and this \
         sweep is reporting nothing rather than checking something"
    );
    sources
}

/// The storage pack's compiler lane (bn-2fk3), the network lane applied to that crate.
const EFFECTS_STORAGE_NO_STD_LANE: &str =
    include_str!("../../continuum-effects-storage/tests/pr15_no_std_lane.rs");

/// The name of the storage lane's test.
const STORAGE_NO_STD_LANE_TEST: &str =
    "fn the_compiler_refuses_a_host_facility_in_the_storage_pack()";

// The shared token lexer and structure rules (cr-35ujnx): one Rust file for this
// audit and the three pack lanes, the twin of `tools/governance/rust_lexer.py`.
include!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tools/governance/rust_lexer.rs"
));

/// Why a pack's manifest lets a build escape its `src/`, or `Ok` (cr-35ujnx round
/// 5). No manifest text scan decides it:
///
/// - Cargo's resolved view does. The manifest, the sources and any `build.rs` beside
///   the real manifest are placed in a scratch copy of the real workspace (its root
///   manifest verbatim, so inherited keys resolve as they do for real), and
///   `cargo metadata` must show no `custom-build` or `proc-macro` target, no `links`,
///   no dependency, no feature, and a library at the pack's own `src/lib.rs`.
/// - Python's `tomllib` parses the `[lints]` table structurally, which must be exactly
///   `workspace = true`, so the pack keeps the workspace's `unsafe_code = "forbid"`.
fn manifest_problem(sources: &[(PathBuf, String)], manifest: &str) -> Result<(), String> {
    manifest_problem_with(sources, manifest, &[])
}

/// [`manifest_problem`], with extra files placed beside the scratch manifest, such as a
/// `build.rs` that Cargo finds with no manifest key.
fn manifest_problem_with(
    sources: &[(PathBuf, String)],
    manifest: &str,
    extra: &[(&str, &str)],
) -> Result<(), String> {
    static SCRATCH: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let (lib, _) = sources
        .iter()
        .find(|(path, _)| path.ends_with("lib.rs"))
        .ok_or("no lib.rs")?;
    let src = lib.parent().ok_or("lib.rs has no directory")?;
    let crate_dir = src.parent().ok_or("src has no crate directory")?;
    let package = crate_dir
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or("the crate directory has no name")?;
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let scratch = root.join("target/inv015-cargo-view").join(format!(
        "{}-{}",
        std::process::id(),
        SCRATCH.fetch_add(1, std::sync::atomic::Ordering::SeqCst)
    ));
    let pack = scratch.join("crates").join(package);
    let _ = std::fs::remove_dir_all(&scratch);
    std::fs::create_dir_all(pack.join("src")).map_err(|e| e.to_string())?;
    std::fs::copy(root.join("Cargo.toml"), scratch.join("Cargo.toml"))
        .map_err(|e| e.to_string())?;
    std::fs::write(pack.join("Cargo.toml"), manifest).map_err(|e| e.to_string())?;
    for (path, text) in sources {
        let rel = path.strip_prefix(src).map_err(|e| e.to_string())?;
        let to = pack.join("src").join(rel);
        if let Some(dir) = to.parent() {
            std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        }
        std::fs::write(to, text).map_err(|e| e.to_string())?;
    }
    for (file, text) in extra {
        std::fs::write(pack.join(file), text).map_err(|e| e.to_string())?;
    }
    if crate_dir.join("build.rs").exists() {
        std::fs::copy(crate_dir.join("build.rs"), pack.join("build.rs"))
            .map_err(|e| e.to_string())?;
    }
    let verdict = cargo_view::metadata(&pack.join("Cargo.toml"), false)
        .and_then(|meta| cargo_view::pack_problem(&meta, package, &pack));
    let _ = std::fs::remove_dir_all(&scratch);
    verdict?;
    let mut child = std::process::Command::new("python3")
        .args([
            "-c",
            "import json, sys, tomllib\n\
             try:\n    m = tomllib.loads(sys.stdin.read())\n\
             except Exception as e:\n    print(json.dumps({'error': str(e)})); sys.exit()\n\
             print(json.dumps({'lints': m.get('lints')}))",
        ])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| format!("python3 did not run: {e}"))?;
    std::io::Write::write_all(
        &mut child.stdin.take().ok_or("no stdin")?,
        manifest.as_bytes(),
    )
    .map_err(|e| e.to_string())?;
    let out = child.wait_with_output().map_err(|e| e.to_string())?;
    let lints = String::from_utf8_lossy(&out.stdout);
    if lints.trim_end() != "{\"lints\": {\"workspace\": true}}" {
        return Err(format!(
            "the manifest's [lints] is not exactly `workspace = true`: {lints}"
        ));
    }
    Ok(())
}

/// Cargo's resolved view of a pack in its real workspace, `--locked`.
fn real_pack_problem(package: &str) -> Result<(), String> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..");
    let dir = root.join(package);
    let meta = cargo_view::metadata(&dir.join("Cargo.toml"), true)?;
    cargo_view::pack_problem(&meta, package, &dir)
}

/// Whether a crate's sources, manifest and compiler lane make "links no `std`" a
/// compiler fact for every build (bn-3ohe, cr-1dl1d7). The structure rules close every
/// way a build other than the lane's could differ: `#![no_std]` is the crate root's
/// first item; `src/` has no block comment, no `cfg`, `cfg_attr`, macro definition,
/// `include!`, `#[path]`, `asm!` or raw token, and exactly one `extern crate`, which is
/// `alloc`; the manifest has only `[package]` (no `build`, no `links`) and
/// `[lints] workspace = true`. The lane then runs the compiler: it must be a live
/// `#[test]` (no `#[ignore]`, `should_panic` or `cfg`) that plants a `std` use and
/// demands the unresolved-`std` error. The structure is checked here; the lane is
/// what actually runs the compiler.
fn no_std_unconditional(
    sources: &[(PathBuf, String)],
    manifest: &str,
    lane: &str,
    lane_test: &str,
) -> Result<(), String> {
    let names: Vec<(String, &str)> = sources
        .iter()
        .map(|(path, text)| (path.display().to_string(), text.as_str()))
        .collect();
    rust_lexer::structure(names.iter().map(|(name, text)| (name.as_str(), *text)))?;
    manifest_problem(sources, manifest)?;
    let at = lane
        .find(lane_test)
        .ok_or("the compiler lane test is missing")?;
    let attrs = &lane[..at];
    let attrs = &attrs[attrs.rfind("\n\n").unwrap_or(0)..];
    if !attrs.contains("#[test]")
        || ["ignore", "should_panic", "cfg"]
            .iter()
            .any(|a| attrs.contains(a))
    {
        return Err("the compiler lane test is not a live `#[test]`".to_owned());
    }
    let body = &lane[at..];
    if !(body.contains("std::net::UdpSocket") && body.contains("E0433") && body.contains("release"))
    {
        return Err("the compiler lane no longer plants a std use in both profiles".to_owned());
    }
    if !(body.contains("cargo_problem(manifest_dir()") && lane.contains("cargo_view::metadata")) {
        return Err(
            "the compiler lane no longer checks Cargo's resolved view of the pack".to_owned(),
        );
    }
    if !(body.contains("ambient_inputs(&control") && lane.contains("# env-dep:")) {
        return Err(
            "the compiler lane no longer checks rustc's dep-info for ambient inputs".to_owned(),
        );
    }
    if !(lane.contains("extern\\ncrate std as s;") && body.contains("with_aliased_std")) {
        return Err(
            "the compiler lane no longer witnesses the split, aliased `extern crate std`"
                .to_owned(),
        );
    }
    Ok(())
}

/// The std surfaces through which a crate could reach the machine. A crate whose code
/// names none of them cannot execute a host effect on its own, whatever its callers do.
///
/// The one spelling that could reach a facility without naming it — a raw block — is
/// deliberately *not* listed here, and not because it is unimportant: the workspace
/// forbids it outright with no crate-level override (`Cargo.toml`'s
/// `[workspace.lints.rust]`, docs/03, docs/12), and `GOV-1-07`'s sibling rule
/// `unsafe-forbidden` scans every source in the tree for it on every `just check` run.
/// Naming the token here would plant the very literal that rule searches for and turn
/// this file into its own violation, so the check is cited rather than duplicated.
const HOST_EFFECT_FACILITIES: &[&str] = &[
    "std::fs",
    "std::io",
    "std::net",
    "std::os",
    "std::env",
    "std::process",
    "std::thread",
    "std::time",
    "SystemTime",
    "Instant",
    "Command",
    "TcpStream",
];

/// Every code line of `source` that names a host-effect facility, comments excluded so
/// that prose *about* an effect is never mistaken for the effect.
fn host_effect_lines(source: &str) -> Vec<&str> {
    source
        .lines()
        .map(str::trim_start)
        .filter(|line| !line.starts_with("//"))
        .filter(|line| {
            HOST_EFFECT_FACILITIES
                .iter()
                .any(|facility| line.contains(facility))
        })
        .collect()
}

/// The two `continuum-security` files that perform host effects on purpose: the signing
/// sources bn-1hape added (plan §18.6). Only filesystem facilities are allowed in them.
fn is_signing_source(path: &std::path::Path) -> bool {
    path.ends_with("entropy.rs") || path.ends_with("keystore.rs")
}

/// What the signing sources may name: the filesystem, its I/O, and the Unix permission
/// extensions.
const SIGNING_SOURCE_FACILITIES: &[&str] = &["std::fs", "std::io", "std::os"];

/// What they may never name, even inside an allowed path (`std::os::unix::process`).
const NON_FILESYSTEM_FACILITIES: &[&str] = &[
    "std::net",
    "std::process",
    "std::thread",
    "std::time",
    "std::env",
    "process::",
    "net::",
    "SystemTime",
    "Instant",
    "Command",
    "TcpStream",
];

/// The dependency names declared under `[dependencies]` of a manifest, read as text.
///
/// Deliberately not a TOML parse and deliberately not `cargo metadata`: this file shares
/// no code with `tools/check_crate_boundaries.py`, so the two can disagree (docs/03 §8,
/// "diversity against common-mode bugs") — the same posture
/// `continuum-certificate/tests/inv004_no_self_certification.rs` takes on the same
/// forbidden-edge family.
fn declared_dependencies(manifest: &str) -> Vec<String> {
    let mut names = Vec::new();
    let mut inside = false;
    for line in manifest.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            inside = trimmed == "[dependencies]";
            continue;
        }
        if !inside || trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let (name, _) = trimmed
            .split_once('=')
            .expect("a dependency line is `name = …`");
        names.push(name.trim().to_owned());
    }
    names
}

/// Every source file of this crate, from the manifest directory the harness fixes.
fn continuumd_sources() -> Vec<(PathBuf, String)> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut sources = Vec::new();
    rust_sources(&root, &mut sources);
    assert!(
        sources.len() >= 30,
        "the source walk found only {} files, which is not this crate",
        sources.len()
    );
    sources
}

/// The `(count, noun)` pairs of the IDL's own registry-summary sentence, so the closed
/// counts asserted here are the IDL's rather than this file's.
fn idl_registry_counts() -> (usize, usize) {
    let sentence = IDL
        .lines()
        .find(|line| line.contains(" operations in ") && line.contains("namespaces"))
        .expect("the IDL states its registry size");
    let (left, right) = sentence
        .split_once(" operations in ")
        .expect("the sentence splits on its own phrase");
    let operations = left
        .split(|character: char| !character.is_ascii_digit())
        .rfind(|token| !token.is_empty())
        .expect("an operation count")
        .parse::<usize>()
        .expect("an operation count parses");
    let namespaces = right
        .split_whitespace()
        .next()
        .expect("a namespace count")
        .parse::<usize>()
        .expect("a namespace count parses");
    (operations, namespaces)
}

// --- the contract and docs/49's authority rows ---------------------------------------------

mod contract_pins {
    use super::{DOCS_49, INV001_EVIDENCE, PLAN_MD, pin};

    /// The clause sentence this whole file maps, plus the three capability-matrix rows
    /// that are INV-015's own, plus §18.2's default-deny sentence.
    #[test]
    fn positive_the_plan_contract_and_the_docs49_authority_rows_are_pinned() {
        pin(
            "plan.md",
            PLAN_MD,
            &[
                "### INV-015 — Agent least authority",
                "Agents cannot alter evidence status, sign receipts, access ungranted \
                 production traces, or execute unrestricted host effects.",
                // §18.2: privilege is never acquired by default.
                "They do not receive evidence-signing, intent-policy mutation, arbitrary \
                 network, or host execution by default.",
            ],
        );
        pin(
            "docs/49",
            DOCS_49,
            &[
                "| promote evidence | no | no | no | no | yes/checker |",
                "| access production secrets | no/default | no | no | scoped | scoped |",
                "| arbitrary network/host exec | no | no | no | no | no/default |",
                "enforces capabilities independently of model output",
            ],
        );
    }

    /// The fourth authority row — "revise intent: proposal only" — is already pinned
    /// verbatim by the INV-001 evidence suite (`bn-34je`), together with the daemon
    /// module doc that realizes it. Cited, not re-derived; this tripwire fails if that
    /// pin is ever gutted underneath this citation.
    #[test]
    fn positive_the_revise_intent_row_is_cited_from_the_inv001_suite() {
        pin(
            "inv001_protected_intent_evidence.rs",
            INV001_EVIDENCE,
            &[
                "| revise intent | proposal only | no | no | proposal only | policy-dependent |",
                "revise intent: proposal only",
            ],
        );
    }
}

// --- the privileged perimeter, live off the registry ---------------------------------------

mod privileged_perimeter {
    use std::collections::BTreeSet;

    use super::{Annotation, OBLIGATION, OPERATIONS, idl_registry_counts, pin};

    /// The exact `@privileged` set — the five governance verbs and nothing else.
    ///
    /// This is the least-authority perimeter as data. Nothing previously asserted the
    /// set exhaustively: the conformance suite holds registry annotations equal to the
    /// IDL's, but a *sixth* privileged operation added consistently to both would have
    /// passed every existing gate. Here it fails until this map is re-audited.
    pub(super) fn privileged_names<'name>(
        names: impl Iterator<Item = &'name str>,
    ) -> BTreeSet<&'name str> {
        names.collect()
    }

    /// The operations plan §18.5 and RFC 0027 make privileged: the five governance verbs,
    /// and from protocol 3.8 (bn-3glnv) the six signing-wire writes. Re-audited for the
    /// latter: minting, rotating, and revoking a signer, signing a domain pack, and exporting
    /// or importing an intent bundle change which signatures the deployment trusts or speak
    /// for it, so each is `@privileged @audit_recorded` at `revise-intent`, and each also
    /// needs a grant with no scope list (`rule signing.identities`). None of them mints,
    /// widens, or revokes a *capability*: the signer registry is not the capability registry,
    /// and no wire operation reaches `ReferenceStore::mint` or `ReferenceStore::revoke`.
    pub(super) const PRIVILEGED: [&str; 11] = [
        "intent.accept",
        "intent.export_bundle",
        "intent.import_bundle",
        "intent.lock",
        "intent.reject",
        "repair.promote",
        "repair.reject",
        "signing.mint",
        "signing.revoke",
        "signing.rotate",
        "signing.sign_pack",
    ];

    #[test]
    fn positive_the_privileged_set_is_exactly_the_five_governance_operations() {
        let live = privileged_names(
            OPERATIONS
                .iter()
                .filter(|spec| spec.has(Annotation::Privileged))
                .map(|spec| spec.name),
        );
        let expected: BTreeSet<&str> = PRIVILEGED.into_iter().collect();
        assert_eq!(
            live, expected,
            "the @privileged perimeter moved; re-audit INV-015's map before accepting it"
        );
    }

    /// docs/49: "logs attempted privileged operations". Every `@privileged` operation
    /// is `@audit_recorded`, which is the registry claim `daemon/obligation.rs` banks
    /// on ("cheaper to *enforce* than to re-verify") — verified here anyway, because a
    /// claim about a table is one table-sweep away from being a fact.
    #[test]
    fn positive_every_privileged_operation_is_audit_recorded() {
        for spec in OPERATIONS
            .iter()
            .filter(|spec| spec.has(Annotation::Privileged))
        {
            assert!(
                spec.has(Annotation::AuditRecorded),
                "{} is @privileged but not @audit_recorded",
                spec.name
            );
        }
        pin(
            "daemon/obligation.rs",
            OBLIGATION,
            &["every `@privileged` operation carries"],
        );
    }

    /// RFC 0026 correction 20 keeps capability administration off the wire; the
    /// registry agrees: no `capability` namespace exists among the operations, and the
    /// namespace count is the IDL's own, so a namespace cannot be added quietly.
    #[test]
    fn positive_the_namespace_set_is_closed_and_contains_no_capability_namespace() {
        let namespaces: BTreeSet<&str> = OPERATIONS
            .iter()
            .map(|spec| spec.name.split('.').next().expect("namespace.verb"))
            .collect();
        assert!(
            !namespaces.contains("capability"),
            "a capability namespace appeared on the wire; correction 20 is breached"
        );
        let (operations, namespace_count) = idl_registry_counts();
        assert_eq!(
            OPERATIONS.len(),
            operations,
            "registry size vs the IDL's sentence"
        );
        assert_eq!(
            namespaces.len(),
            namespace_count,
            "namespace count vs the IDL's sentence"
        );
    }

    /// No `@mutation` request admits a caller-supplied status: the one status-named
    /// field on any mutating request is `evidence.verify`'s `expected_status`, which is
    /// the compare-and-set *guard* — what the caller believes, never what is written
    /// (the written status is what the daemon re-derived, `daemon_evidence.rs`).
    ///
    /// The guard's presence is asserted too, so this sweep is proven to be reading the
    /// real field lists rather than vacuously passing over empty ones.
    #[test]
    fn positive_no_mutation_request_admits_a_caller_supplied_status() {
        let mut status_fields = Vec::new();
        for spec in OPERATIONS
            .iter()
            .filter(|spec| spec.has(Annotation::Mutation))
        {
            for field in spec.request.fields {
                if field.name.contains("status") {
                    status_fields.push((spec.name, field.name));
                }
            }
        }
        assert_eq!(
            status_fields,
            vec![("evidence.verify", "expected_status")],
            "a mutating request grew a status-shaped field; INV-015 clause one is at risk"
        );
    }
}

// --- clause 1: alter evidence status -------------------------------------------------------

mod status_authority {
    use super::{DAEMON_EVIDENCE_TESTS, EVIDENCE, OBSERVE, pin};

    /// The compile-time device: the promotion witness has no public constructor and the
    /// producer family lives in a different module, so producer-side code physically
    /// cannot reach the status write (bn-24i's four-place INV-004 enforcement,
    /// re-cited here because INV-015's first clause *is* its authority half).
    #[test]
    fn positive_the_promotion_witness_cannot_be_built_outside_the_verification_service() {
        pin(
            "daemon/evidence.rs",
            EVIDENCE,
            &[
                "Its fields are private to this module and it has no public constructor",
                "*name* the type and can never build a value of it",
            ],
        );
        pin(
            "daemon/observe.rs",
            OBSERVE,
            &["`provenance.actor` — from the *admitted grant*, not from the request"],
        );
    }

    /// The live suite that dispatches the attacks: each cited test still exists under
    /// its cited name, so this map's rows cannot rot silently (the bn-1eqt device).
    #[test]
    fn positive_the_live_status_authority_tests_still_exist() {
        pin(
            "tests/daemon_evidence.rs",
            DAEMON_EVIDENCE_TESTS,
            &[
                "fn a_producers_append_lands_at_the_lattices_bottom",
                "fn the_producers_request_body_has_no_field_that_could_name_a_status",
                "fn the_status_written_is_what_the_checker_established_not_what_the_caller_named",
                "fn no_operation_in_either_family_removes_or_edits_an_appended_node",
                "fn a_verification_service_may_not_promote_a_claim_it_produced_itself",
            ],
        );
    }
}

// --- clause 2: sign receipts ---------------------------------------------------------------

mod receipt_authority {
    use super::{
        DAEMON_EVIDENCE_TESTS, DAEMON_MOD, DEFAULT_SERVICE, EVIDENCE, PLAN_MD, STATE,
        continuumd_sources, pin,
    };

    /// A check receipt is the checker's own artifact: the caller *is* the checker (no
    /// checker request field), the caller must be a `service:` actor, and a check of
    /// one's own production is refused. An `agent:` actor cannot append a receipt at
    /// any authority level — which is this clause, enforced.
    #[test]
    fn positive_receipt_appending_is_gated_to_service_actors_and_never_self_certified() {
        assert!(
            DEFAULT_SERVICE.starts_with("service:"),
            "the default checker identity is not a service: actor"
        );
        pin(
            "daemon/evidence.rs",
            EVIDENCE,
            &["the caller is not a `service:` actor"],
        );
        pin(
            "tests/daemon_evidence.rs",
            DAEMON_EVIDENCE_TESTS,
            &[
                "fn only_a_service_actor_may_append_a_check_edge",
                "fn a_checker_may_not_record_a_check_of_its_own_production",
                "fn a_checker_appends_a_receipt_and_the_check_edge_that_names_it",
            ],
        );
    }

    /// The cryptographic half (plan §18.6, bn-1hape). Until bn-1hape this was a gap
    /// pinned as a tripwire: nothing signed a receipt. Now `evidence.link` signs each
    /// receipt it publishes, and this guards that an *agent* still cannot:
    ///
    /// - the signing key is installed only by the deployment, through
    ///   `Builder::receipt_signer`, and held by the daemon ("agents never do");
    /// - signing happens inside `evidence.link` only after the caller is admitted as a
    ///   `service:` checker, so an `agent:` capability never reaches the signer;
    /// - no wire type carries a key: no `protocol/` source names a signer or key type.
    #[test]
    fn positive_only_the_daemon_holds_the_receipt_key_and_only_a_service_check_signs() {
        pin(
            "plan.md",
            PLAN_MD,
            &[
                "### 18.6 Signing identities",
                "Receipts, intent bundles, and domain packs are signed.",
            ],
        );
        pin(
            "daemon/state.rs",
            STATE,
            &[
                "the RFC 0037 A1 signature chain, oldest first",
                "fn record_receipt_signature(",
            ],
        );
        let service_gate = EVIDENCE
            .find("ServiceIdentity::parse(call.grant.actor.as_str())")
            .expect("evidence.link admits its checker as a service identity");
        let signs = EVIDENCE
            .find(".sign_receipt(")
            .expect("evidence.link signs the receipt");
        assert!(
            service_gate < signs,
            "evidence.link signs before it has refused a non-service caller"
        );
        let mut installers = 0;
        for (path, text) in continuumd_sources() {
            let rel = path.display().to_string();
            if rel.contains("/protocol/") {
                for token in ["ReceiptSigner", "LocalSigner", "SigningKey", "signing_key"] {
                    assert!(
                        !text.contains(token),
                        "{rel} names {token:?}: a wire type now carries signing material"
                    );
                }
            }
            installers += text.matches("pub fn receipt_signer(mut self").count();
            let reaches_keystore = text
                .lines()
                .map(str::trim_start)
                .filter(|line| !line.starts_with("//"))
                .any(|line| line.contains("continuum_security"));
            assert!(
                !reaches_keystore,
                "{rel} names continuum_security in code: the daemon would reach the keystore itself"
            );
        }
        assert_eq!(
            installers, 1,
            "the builder is the one place a receipt signer is installed"
        );
        pin(
            "daemon/mod.rs",
            DAEMON_MOD,
            &["The daemon holds the key; agents never do"],
        );
    }
}

// --- clause 3: access ungranted production traces ------------------------------------------

mod trace_grant {
    use super::{
        ADMISSION, DAEMON_EVIDENCE_TESTS, DAEMON_OPERATIONS_TESTS, DataGrant, Denied, OPERATIONS,
        PLAN_MD, continuumd_sources, pin, required_grant,
    };

    /// R-4, live off the production predicate: across the whole registry, exactly one
    /// operation requires a grant beyond its authority level, and it is the
    /// production-trace grant on the one verb that ingests a production trace.
    #[test]
    fn positive_exactly_one_operation_requires_a_data_grant_and_it_is_the_production_trace() {
        let granted: Vec<(&str, DataGrant)> = OPERATIONS
            .iter()
            .filter_map(|spec| required_grant(spec.name).map(|grant| (spec.name, grant)))
            .collect();
        assert_eq!(
            granted,
            vec![("observe.ingest", DataGrant::ProductionTrace)],
            "the data-grant table moved; re-audit the ungranted-traces row"
        );
    }

    /// X1 as a compile fact: the admission refusal is a unit struct, so level, scope,
    /// privilege, expiry, revocation, actor mismatch, and grant failures are one value
    /// with zero bits in it — a refused caller cannot learn *why*.
    #[test]
    fn positive_a_denial_carries_zero_bits() {
        assert_eq!(core::mem::size_of::<Denied>(), 0, "Denied grew a field");
        let denial: Denied = Default::default();
        assert_eq!(denial, Denied, "Denied has one value");
        pin(
            "daemon/admission.rs",
            ADMISSION,
            &[
                "A denial carries nothing.",
                "a client MUST NOT be able to tell them apart",
            ],
        );
    }

    /// X2/X3 — denial precedes semantic work and reveals no existence — held by live
    /// dispatch tests, cited by name; plus the R-4 dispatch test itself.
    #[test]
    fn positive_the_no_existence_oracle_discipline_is_held_by_live_tests() {
        pin(
            "daemon/admission.rs",
            ADMISSION,
            &["denial precedes semantic work and precedes the index"],
        );
        pin(
            "tests/daemon_operations.rs",
            DAEMON_OPERATIONS_TESTS,
            &[
                "fn every_admission_failure_is_one_byte_identical_answer",
                "fn a_snapshot_the_daemon_does_not_hold_is_denied_and_never_reported_missing",
            ],
        );
        pin(
            "tests/daemon_evidence.rs",
            DAEMON_EVIDENCE_TESTS,
            &[
                "fn a_promotion_of_a_claim_that_does_not_exist_is_byte_identical_to_one_that_does",
                "fn the_production_trace_grant_is_required_beyond_the_execute_level",
            ],
        );
    }

    /// The honest half: §18.4's capture-time contract — salted payload commitments,
    /// per-artifact encryption, a retention clock — has no producer. The *redaction*
    /// half is live (typed stubs beside the record, INV-007 omission manifests) and
    /// cited; the capture-time half is swept as an absence that flips this row when it
    /// lands.
    #[test]
    fn boundary_the_capture_time_contract_has_no_producer() {
        pin("plan.md", PLAN_MD, &["salted commitments by default"]);
        pin(
            "tests/daemon_evidence.rs",
            DAEMON_EVIDENCE_TESTS,
            &[
                "fn a_redacted_reference_reads_back_as_the_typed_stub_and_names_its_omission",
                "fn verifying_over_a_redacted_reference_returns_the_structural_result_and_the_redaction",
            ],
        );
        for (path, text) in continuumd_sources() {
            assert!(
                !text.contains("salted"),
                "{} now implements salted capture; re-audit the production-trace row",
                path.display()
            );
        }
    }
}

// --- clause 4: execute unrestricted host effects, and never-by-default ---------------------

mod no_widening {
    use continuumd::protocol::registry::OPERATIONS;
    use continuumd::protocol::vocabulary::ErrorCode;

    use super::{
        ADMISSION, CAPABILITY, CONTINUUMD_MANIFEST, DAEMON_OPERATIONS_TESTS, EFFECTS_NETWORK_LIB,
        EFFECTS_NETWORK_MANIFEST, EFFECTS_NETWORK_NO_STD_LANE, EFFECTS_PROCESS_LIB,
        EFFECTS_PROCESS_MANIFEST, EFFECTS_PROCESS_NO_STD_LANE, EFFECTS_STORAGE_LIB,
        EFFECTS_STORAGE_MANIFEST, EFFECTS_STORAGE_NO_STD_LANE, EFFECTS_TIME_LIB, FORGE_MANIFEST,
        NETWORK_NO_STD_LANE_TEST, NON_FILESYSTEM_FACILITIES, PROCESS_NO_STD_LANE_TEST,
        PROOF_CLIENT_LIB, PUBLICATION, SECURITY_LIB, SECURITY_MANIFEST, SIGNER_ADMINISTRATION,
        SIGNING_SOURCE_FACILITIES, STORAGE_NO_STD_LANE_TEST, administrative_callers,
        continuumd_sources, declared_dependencies, effects_network_sources,
        effects_process_sources, effects_storage_sources, forge_sources, host_effect_lines,
        is_signing_source, no_std_unconditional, pin, pub_items, real_pack_problem,
        security_sources, unexempted_callers,
    };

    /// Correction 20's property, stated and then swept: capability administration is
    /// out-of-band, `ReferenceStore::mint`/`revoke` have no wire caller, and no
    /// `continuumd` source calls either — a real walk over every source file, so the
    /// first wire caller anyone adds turns this red before it turns into a surface.
    #[test]
    fn positive_the_administrative_surface_has_no_wire_caller() {
        pin(
            "daemon/capability.rs",
            CAPABILITY,
            &[
                "no operation in this protocol can widen",
                "have **no wire caller**",
            ],
        );
        // The store-side mint itself is least-authority: administration is a distinct
        // action and an administrator may not mint above its own level.
        pin(
            "continuum-workspace/src/publication.rs",
            PUBLICATION,
            &["and it may not mint", "INV-015's least authority"],
        );
        let mut signer_sites = 0;
        for (path, text) in continuumd_sources() {
            let callers = unexempted_callers(&path, &text);
            assert!(
                callers.is_empty(),
                "{} calls the administrative surface: {callers:?}",
                path.display()
            );
            if path.ends_with("src/daemon/signing.rs") {
                signer_sites += administrative_callers(&text).len();
            }
        }
        assert_eq!(
            signer_sites,
            SIGNER_ADMINISTRATION.len(),
            "the signer-administration sites moved; re-audit the exemption"
        );
    }

    /// "Privilege is never acquired by default": an absent profile grants nothing, an
    /// empty grant list grants nothing, delegation only narrows — the fail-closed
    /// sentences pinned in the predicate that enforces them, and the dispatch tests
    /// that hold them, cited by name.
    #[test]
    fn positive_privilege_is_never_acquired_by_default() {
        pin(
            "daemon/admission.rs",
            ADMISSION,
            &[
                "An absent profile is the fail-closed reading",
                "grants nothing rather than everything",
                "nothing widens it",
            ],
        );
        pin(
            "tests/daemon_operations.rs",
            DAEMON_OPERATIONS_TESTS,
            &[
                "fn a_reviser_without_the_privilege_cannot_accept_an_intent",
                "fn a_delegation_that_does_not_narrow_its_parent_is_denied",
            ],
        );
    }

    /// The host-effect substance has not arrived: every crate that will hold it is a
    /// zero-pub-item scaffold that says so itself. This is the tripwire that makes the
    /// isolation gap rows honest — the moment any of these grows a public item, this
    /// file fails and the docs/49 audit below must be redone against a real surface.
    ///
    /// `continuum-forge` was the seventh row here until bn-1dsih landed its
    /// task-assembly lane. It is *not* silently dropped: it moves to
    /// [`boundary_forge_grew_a_task_assembly_lane_and_it_reaches_no_host_effect`], where
    /// the property this sweep actually guards — that no crate on this list can execute
    /// a host effect on an agent's behalf — is re-established for a surface that now
    /// exists, instead of being asserted by emptiness.
    ///
    /// `continuum-security` left on the same terms when bn-ymw landed the plan §18.1
    /// prompt-injection corpus in it, and is re-established the same way, in
    /// [`boundary_security_grew_an_injection_corpus_and_it_reaches_no_host_effect`].
    /// `continuum-effects-network` left when bn-3ohe landed its Lab pack, and is
    /// re-established in
    /// [`boundary_effects_network_grew_a_lab_pack_and_it_reaches_no_host_effect`].
    /// `continuum-effects-process` left when bn-3mmf landed its Lab pack, and is
    /// re-established in
    /// [`boundary_effects_process_grew_a_lab_pack_and_it_reaches_no_host_effect`].
    /// `continuum-effects-storage` left when bn-2fk3 landed its Lab pack, and is
    /// re-established in
    /// [`boundary_effects_storage_grew_a_lab_pack_and_it_reaches_no_host_effect`]. The
    /// two rows below keep the original, stricter form because their substance
    /// genuinely has not arrived.
    #[test]
    fn boundary_the_host_effect_surface_has_not_arrived_and_its_crates_are_scaffolds() {
        let scaffolds = [
            ("continuum-effects-time", EFFECTS_TIME_LIB),
            ("continuum-proof-client", PROOF_CLIENT_LIB),
        ];
        for (name, source) in scaffolds {
            let items = pub_items(source);
            assert!(
                items.is_empty(),
                "{name} grew public items {items:?}; the host-effect/isolation surface \
                 is arriving — re-audit INV-015's fourth clause and the docs/49 table"
            );
            assert!(
                source.contains("PR-1 / IMPL-01 scaffold"),
                "{name} no longer declares itself a scaffold"
            );
        }
    }

    /// `continuum-forge`'s recorded public surface, sorted — every `pub` item across
    /// every source file of the crate, as `pub_items` reads them.
    ///
    /// A recorded inventory rather than a zero-item assertion, because the crate is no
    /// longer empty. It is the same tripwire in a different shape: an unrecorded public
    /// item fails this test, so a surface cannot grow past what INV-015 has audited
    /// without the audit being redone.
    const FORGE_PUBLIC_SURFACE: [&str; 17] = [
        "pub const fn intent_id(&self) -> &IntentId {",
        "pub const fn intent_id(&self) -> &IntentId {",
        "pub const fn intent_id(&self) -> &IntentId {",
        "pub const fn intent_id(&self) -> &IntentId {",
        "pub const fn invariant(&self) -> &'static str {",
        "pub const fn vacuous(&self) -> Option<&VacuousIntent> {",
        "pub const fn vacuous(&self) -> Option<&VacuousIntent> {",
        "pub const fn verdict(&self) -> &ContractVerdict {",
        "pub enum TaskRefusal {",
        "pub fn assemble(",
        "pub fn hard_objectives(&self) -> &[Objective] {",
        "pub fn positive_behaviors(&self) -> &[NonVacuityObligation] {",
        "pub fn soft_objectives(&self) -> &[Objective] {",
        "pub mod task;",
        "pub struct NotWellFormed {",
        "pub struct SynthesisTask {",
        "pub struct VacuousIntent {",
    ];

    /// The first crate on the host-effect list to grow a real surface, audited rather
    /// than grandfathered (bn-1dsih; RFC 0037 flag F12).
    ///
    /// What arrived is a *contract-admission* lane: `assemble` decides whether an Intent
    /// Contract may become a synthesis task, by making the two calls RFC 0037 AO2/AO3
    /// require, and refusing with a typed value. What INV-015's fourth clause forbids is
    /// executing unrestricted host effects on an agent's behalf, so the question this
    /// test answers is not "is the crate empty" but "can any of this new surface be
    /// reached from the wire, and can any of it touch the machine". Four independent
    /// legs, each swept live:
    ///
    /// 1. the surface is exactly what has been audited — an unrecorded `pub` item fails;
    /// 2. no code line in the crate names a host-effect facility, so the lane cannot
    ///    open a file, a socket, a process, a thread, or a clock;
    /// 3. the crate's declared dependency set is one verifier-side crate, so it cannot
    ///    reach an effect through a neighbour either;
    /// 4. it is unreachable from an agent connection.
    ///
    /// Leg 4 is stated carefully, because the obvious version of it is false: the
    /// registry *does* declare a `forge` namespace — `forge.create`, `forge.step`,
    /// `forge.archive`, `forge.materialize` — and has since the protocol surface landed.
    /// Those four are declared vocabulary ahead of their subsystem, which `rule
    /// errors.unsupported_surface` provides for and which is why each carries
    /// `UnsupportedSemanticFeature` in its own `errors` clause. What makes the lane
    /// unreachable is one layer down: no [`OperationFamily`] answers for the `forge`
    /// namespace, `continuumd` does not link `continuum-forge`, and no `continuumd`
    /// source names it — so a `forge.*` request meets a typed refusal from the
    /// dispatcher and never reaches any code in that crate. All three are swept live.
    ///
    /// Any of the four legs turning red means Forge has acquired authority the docs/49
    /// audit below has never examined, and that audit must be redone before the change
    /// lands.
    ///
    /// [`OperationFamily`]: continuumd::daemon::family::OperationFamily
    #[test]
    fn boundary_forge_grew_a_task_assembly_lane_and_it_reaches_no_host_effect() {
        // 1. The audited surface, exactly.
        let mut items: Vec<String> = forge_sources()
            .iter()
            .flat_map(|(_, text)| {
                pub_items(text)
                    .into_iter()
                    .map(str::to_owned)
                    .collect::<Vec<String>>()
            })
            .collect();
        items.sort();
        let audited: Vec<String> = FORGE_PUBLIC_SURFACE
            .iter()
            .map(|it| (*it).to_owned())
            .collect();
        assert_eq!(
            items, audited,
            "continuum-forge's public surface is not the one INV-015 audited; record the \
             new items here and re-derive this test's four legs against them"
        );

        // 2. No host-effect facility is named anywhere in the crate's code.
        for (path, text) in forge_sources() {
            let effects = host_effect_lines(&text);
            assert!(
                effects.is_empty(),
                "{} names host-effect facilities {effects:?}; the lane is supposed to be \
                 a pure function of a contract and an environment",
                path.display()
            );
        }

        // 3. One declared dependency, and it is verifier-side.
        assert_eq!(
            declared_dependencies(FORGE_MANIFEST),
            vec!["continuum-intent".to_owned()],
            "continuum-forge's dependency set widened; an effect crate, an engine, or the \
             daemon among them would give the lane authority through a neighbour"
        );

        // 4. Unreachable from an agent connection. The `forge` namespace is declared
        // vocabulary — recorded here rather than wished away — and every one of its
        // operations answers `UnsupportedSemanticFeature`, which is the shape `rule
        // errors.unsupported_surface` fixes for an operation registered ahead of its
        // subsystem.
        let forge_operations: Vec<&str> = OPERATIONS
            .iter()
            .map(|spec| spec.name)
            .filter(|name| name.starts_with("forge."))
            .collect();
        assert_eq!(
            forge_operations,
            vec![
                "forge.create",
                "forge.step",
                "forge.archive",
                "forge.materialize"
            ],
            "the declared forge vocabulary changed; re-derive what a forge.* request can \
             now reach"
        );
        for name in &forge_operations {
            let spec = OPERATIONS
                .iter()
                .find(|spec| spec.name == *name)
                .expect("the operation was just enumerated");
            assert!(
                spec.errors.contains(&ErrorCode::UnsupportedSemanticFeature),
                "{name} no longer declares UnsupportedSemanticFeature; it may have grown a \
                 subsystem, and this audit must be redone against it"
            );
        }

        // No family answers for the namespace, so the dispatcher refuses before any
        // handler runs. Swept by looking for the namespace literal a family's
        // `OperationFamily::namespace` would have to return.
        for (path, text) in continuumd_sources() {
            let claims: Vec<&str> = text
                .lines()
                .map(str::trim_start)
                .filter(|line| !line.starts_with("//"))
                .filter(|line| *line == "\"forge\"")
                .collect();
            assert!(
                claims.is_empty(),
                "{} claims the forge namespace; a family now answers forge.* and INV-015's \
                 fourth clause must be re-audited against it",
                path.display()
            );
        }

        // And the crate itself is not linked and not named, so even the declared verbs
        // have no route into it.
        assert!(
            !declared_dependencies(CONTINUUMD_MANIFEST).contains(&"continuum-forge".to_owned()),
            "continuumd now links continuum-forge; the lane is one call away from the wire"
        );
        for (path, text) in continuumd_sources() {
            let callers: Vec<&str> = text
                .lines()
                .map(str::trim_start)
                .filter(|line| !line.starts_with("//"))
                .filter(|line| line.contains("continuum_forge"))
                .collect();
            assert!(
                callers.is_empty(),
                "{} names continuum_forge: {callers:?}",
                path.display()
            );
        }
    }

    /// `continuum-security`'s recorded public surface, sorted — every `pub` item across
    /// every source file of the crate, as [`pub_items`] reads them.
    ///
    /// The same shape as [`FORGE_PUBLIC_SURFACE`] and for the same reason: the crate is no
    /// longer empty, so the tripwire becomes an inventory. An unrecorded public item fails
    /// the audit below, so this surface cannot grow past what INV-015 has examined without
    /// the examination being redone.
    const SECURITY_PUBLIC_SURFACE: [&str; 64] = [
        "pub carrier: &'static str,",
        "pub const ALL: [Self; 11] = [",
        "pub const ALL: [Self; 3] = [",
        "pub const ALL: [Self; 7] = [",
        "pub const ALL: [Self; 8] = [",
        "pub const AUDIT_FILE: &str = \"signing-audit.log\";",
        "pub const AUDIT_MAGIC: [u8; 8] = *b\"CTMAUD01\";",
        "pub const CASES: &[Case] = &[",
        "pub const KEY_FILE: &str = \"signer.key\";",
        "pub const KEY_FILE_LEN: usize = KEY_MAGIC.len() + SEED_LEN + PUBLIC_KEY_LEN;",
        "pub const KEY_MAGIC: [u8; 8] = *b\"CTMKEY01\";",
        "pub const MAX_AUDIT_RECORDS: u32 = 4096;",
        "pub const MAX_AUDIT_RECORD_LEN: u32 = 1024;",
        "pub const MAX_PAYLOAD_BYTES: usize = 4096;",
        "pub const OS_ENTROPY_PATH: &str = \"/dev/urandom\";",
        "pub const fn bullet(self) -> &'static str {",
        "pub const fn bullet(self) -> &'static str {",
        "pub const fn bullet(self) -> &'static str {",
        "pub const fn bullet(self) -> &'static str {",
        "pub const fn expecting_owner(mut self, uid: u32) -> Self {",
        "pub const fn minted(&self) -> bool {",
        "pub const fn new() -> Self {",
        "pub const fn readability(class: ArtifactClass) -> Readability {",
        "pub const fn registry(&self) -> &SigningRegistry {",
        "pub const fn signer(&self) -> &LocalSigner {",
        "pub const fn token(self) -> &'static str {",
        "pub enum IntentPolicyBlock {",
        "pub enum IsolationControl {",
        "pub enum KeystoreError {",
        "pub enum ProhibitedOutcome {",
        "pub enum Readability {",
        "pub enum RedTeamClass {",
        "pub enum Vector {",
        "pub fn case(id: &str) -> Option<&'static Case> {",
        "pub fn dir(&self) -> &Path {",
        "pub fn encode_audit_log(registry: &SigningRegistry) -> Result<Vec<u8>, KeystoreError> {",
        "pub fn into_parts(self) -> (SigningRegistry, LocalSigner) {",
        "pub fn isolation_cases() -> impl Iterator<Item = &'static Case> {",
        "pub fn new(dir: impl Into<PathBuf>) -> Self {",
        "pub fn open(&self) -> Result<OpenedKeystore, KeystoreError> {",
        "pub fn open_or_mint(",
        "pub fn policy_block_cases() -> impl Iterator<Item = &'static Case> {",
        "pub fn red_team_cases() -> impl Iterator<Item = &'static Case> {",
        "pub id: &'static str,",
        "pub mod entropy;",
        "pub mod injection;",
        "pub mod keystore;",
        "pub operation: &'static str,",
        "pub outcome: ProhibitedOutcome,",
        "pub payload: &'static str,",
        "pub struct Case {",
        "pub struct LocalKeystore {",
        "pub struct OpenedKeystore {",
        "pub struct OsEntropy {",
        "pub surface: ArtifactClass,",
        "pub vector: Vector,",
        "pub(super) const O_NOFOLLOW: i32 = 0;",
        "pub(super) const O_NOFOLLOW: i32 = 0o100_000;",
        "pub(super) const O_NOFOLLOW: i32 = 0o400_000;",
        "pub(super) const O_NOFOLLOW: i32 = 0x0100;",
        "pub(super) fn mint(",
        "pub(super) fn open(store: &LocalKeystore) -> Result<OpenedKeystore, KeystoreError> {",
        "pub(super) fn open_bound(",
        "pub(super) fn read_audit(mut file: File) -> Result<SigningRegistry, KeystoreError> {",
    ];

    /// `continuum-security` stopped being a zero-pub-item scaffold when bn-ymw landed the
    /// plan §18.1 / docs/49 / research/35 prompt-injection corpus in it. It is *not*
    /// silently dropped from the sweep above: the property that sweep actually guards — no
    /// crate on that list can execute a host effect on an agent's behalf — is
    /// re-established here against the surface that now exists.
    ///
    /// The crate that plan §18 names for the capability model, sandboxing, privacy and the
    /// audit trail is exactly the one where a growing public surface would matter most, so
    /// the four legs are the Forge audit's, applied to what actually landed:
    ///
    /// 1. the audited surface is exactly [`SECURITY_PUBLIC_SURFACE`];
    /// 2. no host-effect facility is named anywhere in the crate's code — the corpus is
    ///    *data about* attacks, never a means of performing one;
    /// 3. one declared dependency, and it is the model-side one the corpus enumerates its
    ///    surfaces against, so the crate cannot reach an effect through a neighbour;
    /// 4. it is unreachable from an agent connection.
    ///
    /// Leg 4 differs from Forge's in exactly one way, recorded rather than wished away:
    /// `continuumd` *does* declare this crate, in `[dev-dependencies]`, because
    /// `tests/g2_injection_corpus_evidence.rs` drives every case through the wire boundary.
    /// What makes the lane unreachable is that the edge is dev-position only — nothing
    /// `continuumd` ships links it, and no `continuumd` `src/` source names it — so no
    /// request can reach any code in that crate. Both halves are swept live.
    ///
    /// Any of the four legs turning red means the corpus crate has acquired authority the
    /// docs/49 audit below has never examined, and that audit must be redone before the
    /// change lands.
    #[test]
    fn boundary_security_grew_an_injection_corpus_and_it_reaches_no_host_effect() {
        // 1. The audited surface, exactly.
        let mut items: Vec<String> = security_sources()
            .iter()
            .flat_map(|(_, text)| {
                pub_items(text)
                    .into_iter()
                    .map(str::to_owned)
                    .collect::<Vec<String>>()
            })
            .collect();
        items.sort();
        let audited: Vec<String> = SECURITY_PUBLIC_SURFACE
            .iter()
            .map(|it| (*it).to_owned())
            .collect();
        assert_eq!(
            items, audited,
            "continuum-security's public surface is not the one INV-015 audited; record \
             the new items here and re-derive this test's four legs against them"
        );

        // 2. No host-effect facility is named anywhere in the crate's code. This is the
        // load-bearing leg for a red-team corpus specifically: its payloads are prose
        // *describing* exfiltration and process spawning, and the crate must carry them as
        // inert bytes without ever naming a facility that could perform one.
        //
        // bn-1hape added the signing sources (plan §18.6): `entropy.rs` reads the kernel's
        // entropy device and `keystore.rs` reads and writes the key files. Those two are
        // the crate's only effectful files, and their effects are the filesystem alone —
        // no process, network, thread, clock, or environment facility. Every other file,
        // the corpus included, stays inert.
        for (path, text) in security_sources() {
            let effects = host_effect_lines(&text);
            if is_signing_source(&path) {
                let wider: Vec<&&str> = effects
                    .iter()
                    .filter(|line| {
                        !SIGNING_SOURCE_FACILITIES
                            .iter()
                            .any(|allowed| line.contains(allowed))
                            || NON_FILESYSTEM_FACILITIES
                                .iter()
                                .any(|facility| line.contains(facility))
                    })
                    .collect();
                assert!(
                    wider.is_empty(),
                    "{} names host-effect facilities beyond the filesystem: {wider:?}",
                    path.display()
                );
                continue;
            }
            assert!(
                effects.is_empty(),
                "{} names host-effect facilities {effects:?}; the corpus is supposed to be \
                 inert data about attacks, not a means of performing one",
                path.display()
            );
        }

        // 3. Three declared dependencies, all model-side. `continuum-workspace` owns the
        // closed plan §4.4 `ArtifactClass` the corpus classifies its surfaces against;
        // `continuum-evidence` and `continuum-value` are the signing library and canonical
        // encoding the signing sources supply (bn-1hape). An effect crate, an engine, or the
        // daemon among them would give the crate authority through a neighbour.
        assert_eq!(
            declared_dependencies(SECURITY_MANIFEST),
            vec![
                "continuum-evidence".to_owned(),
                "continuum-value".to_owned(),
                "continuum-workspace".to_owned()
            ],
            "continuum-security's dependency set widened; re-derive what the corpus can \
             now reach"
        );

        // 4. Unreachable from an agent connection. The edge into this crate is
        // dev-position only, so nothing the daemon ships links it.
        assert!(
            !declared_dependencies(CONTINUUMD_MANIFEST).contains(&"continuum-security".to_owned()),
            "continuumd now links continuum-security outside [dev-dependencies]; the corpus \
             is one call away from the wire"
        );
        for (path, text) in continuumd_sources() {
            let callers: Vec<&str> = text
                .lines()
                .map(str::trim_start)
                .filter(|line| !line.starts_with("//"))
                .filter(|line| line.contains("continuum_security"))
                .collect();
            assert!(
                callers.is_empty(),
                "{} names continuum_security: {callers:?}",
                path.display()
            );
        }

        // And the crate still declares the contract this file holds it to.
        pin(
            "continuum-security/src/lib.rs",
            SECURITY_LIB,
            &["INV-015 — agent least authority."],
        );
    }

    /// `continuum-effects-network`'s recorded public surface, sorted — every `pub` item
    /// across every source file of the crate, as `pub_items` reads them (bn-3ohe).
    const EFFECTS_NETWORK_PUBLIC_SURFACE: [&str; 105] = [
        "pub class: FidelityClass,",
        "pub const ADVERSARIAL_V0: FidelityProfile = FidelityProfile {",
        "pub const ALL: [Self; 19] = [",
        "pub const ASSUMPTIONS: [(&str, &str); 2] = [",
        "pub const CANCELLATION_CONTRACT: [(&str, &str); 8] = [",
        "pub const JOURNAL_HEADER_BYTES: u64 =",
        "pub const MAX_IN_FLIGHT_CAP: u32 = 1 << 16;",
        "pub const MAX_NODES: u8 = 64;",
        "pub const MAX_PAYLOAD_CAP: u32 = 1 << 16;",
        "pub const MAX_RETAINED_CAP: u64 = 1 << 28;",
        "pub const MAX_STEPS: usize = 1 << 20;",
        "pub const PROFILE_NAME: &str = \"network/adversarial-v0\";",
        "pub const PROFILE_VERSION: ProfileVersion = ProfileVersion {",
        "pub const fn all(n: u8) -> Self {",
        "pub const fn as_str(self) -> &'static str {",
        "pub const fn bits_valid(self, n: u8) -> bool {",
        "pub const fn chooser(&self) -> Chooser {",
        "pub const fn class(&self) -> RefusalClass {",
        "pub const fn config(&self) -> &NetworkConfig {",
        "pub const fn config(&self) -> &NetworkConfig {",
        "pub const fn contains(self, node: NodeId) -> bool {",
        "pub const fn faults(&self) -> FaultSwitches {",
        "pub const fn in_flight_total(&self) -> u32 {",
        "pub const fn inconclusive_reason(&self) -> Option<&'static str> {",
        "pub const fn max_in_flight(&self) -> u32 {",
        "pub const fn max_partitions(&self) -> u32 {",
        "pub const fn max_payload_bytes(&self) -> u32 {",
        "pub const fn max_retained_bytes(&self) -> u64 {",
        "pub const fn new(",
        "pub const fn new(config: NetworkConfig) -> Self {",
        "pub const fn nodes(&self) -> u8 {",
        "pub const fn partition(&self) -> Option<NodeSet> {",
        "pub const fn partitions_used(&self) -> u32 {",
        "pub const fn replicated_register_scenario(",
        "pub const fn retained_bytes(&self) -> u64 {",
        "pub const fn retained_bytes(&self) -> u64 {",
        "pub const fn statement(self) -> &'static str {",
        "pub const fn support(self) -> Support {",
        "pub const fn token(self) -> &'static str {",
        "pub const fn unsupported_semantic(&self) -> Option<Semantic> {",
        "pub duplication: bool,",
        "pub enum Bound {",
        "pub enum Chooser {",
        "pub enum ConfigRefusal {",
        "pub enum Event {",
        "pub enum FidelityClass {",
        "pub enum HostQualification {",
        "pub enum IndependenceClaim {",
        "pub enum Malformed {",
        "pub enum NotEnabled {",
        "pub enum Refusal {",
        "pub enum RefusalClass {",
        "pub enum Semantic {",
        "pub enum Step {",
        "pub enum Support {",
        "pub fn apply(&mut self, step: &Step) -> Result<&Event, Refusal> {",
        "pub fn canonical_bytes(&self) -> Vec<u8> {",
        "pub fn check(&self, step: &Step) -> Result<(), Refusal> {",
        "pub fn choice_log(&self) -> impl Iterator<Item = Step> + '_ {",
        "pub fn copies(&self, envelope: EnvelopeId) -> u32 {",
        "pub fn enabled_choices(&self) -> Vec<Step> {",
        "pub fn encode(&self) -> Vec<u8> {",
        "pub fn encode(&self) -> Vec<u8> {",
        "pub fn envelope(&self, envelope: EnvelopeId) -> Option<(NodeId, NodeId, &Payload)> {",
        "pub fn events(&self) -> &[Event] {",
        "pub fn events(&self) -> &[Event] {",
        "pub fn in_flight(&self) -> impl Iterator<Item = (EnvelopeId, u32)> + '_ {",
        "pub fn into_journal(self) -> Journal {",
        "pub fn of(nodes: &[u8]) -> Option<Self> {",
        "pub fn replay(&self) -> Result<Self, RunRefusal> {",
        "pub fn run(config: NetworkConfig, steps: &[Step]) -> Result<Journal, RunRefusal> {",
        "pub fn sent_count(&self) -> usize {",
        "pub fn step(&self) -> Step {",
        "pub host: HostQualification,",
        "pub independence: IndependenceClaim,",
        "pub index: usize,",
        "pub loss: bool,",
        "pub major: u16,",
        "pub minor: u16,",
        "pub mod lab;",
        "pub mod profile;",
        "pub mod refusal;",
        "pub mod step;",
        "pub name: &'static str,",
        "pub patch: u16,",
        "pub refusal: Refusal,",
        "pub reordering: bool,",
        "pub struct EnvelopeId(pub u32);",
        "pub struct FaultSwitches {",
        "pub struct FidelityProfile {",
        "pub struct Journal {",
        "pub struct Network {",
        "pub struct NetworkConfig {",
        "pub struct NodeId(pub u8);",
        "pub struct NodeSet(pub u64);",
        "pub struct Payload(pub Vec<u8>);",
        "pub struct ProfileVersion {",
        "pub struct RunRefusal {",
        "pub use lab::{Journal, Network, run};",
        "pub use profile::{ADVERSARIAL_V0, FidelityClass, FidelityProfile, Semantic, Support};",
        "pub use refusal::{Refusal, RefusalClass, RunRefusal};",
        "pub use step::{EnvelopeId, Event, FaultSwitches, NetworkConfig, NodeId, NodeSet, Payload, Step};",
        "pub version: ProfileVersion,",
        "pub(crate) fn put_str(out: &mut Vec<u8>, text: &str) {",
        "pub(crate) fn u32_len(len: usize) -> u32 {",
    ];

    /// `continuum-effects-process`'s recorded public surface, sorted — every `pub` item
    /// across every source file of the crate, as `pub_items` reads them (bn-3mmf).
    const EFFECTS_PROCESS_PUBLIC_SURFACE: [&str; 101] = [
        "pub class: FidelityClass,",
        "pub const ALL: [Self; 11] = [",
        "pub const ASSUMPTIONS: [(&str, &str); 3] = [",
        "pub const CANCELLATION_CONTRACT: [(&str, &str); 8] = [",
        "pub const COMPOSITION: [(&str, &str); 3] = [",
        "pub const CRASH_RESTART_V0: FidelityProfile = FidelityProfile {",
        "pub const JOURNAL_HEADER_BYTES: u64 =",
        "pub const MAX_CRASHES_CAP: u32 = 1 << 16;",
        "pub const MAX_NODES: u8 = 64;",
        "pub const MAX_PENDING_CAP: u32 = 1 << 16;",
        "pub const MAX_RETAINED_CAP: u64 = 1 << 28;",
        "pub const MAX_STEPS: usize = 1 << 20;",
        "pub const PROFILE_NAME: &str = \"process/crash-restart-v0\";",
        "pub const PROFILE_VERSION: ProfileVersion = ProfileVersion {",
        "pub const fn all(n: u8) -> Self {",
        "pub const fn as_str(self) -> &'static str {",
        "pub const fn chooser(&self) -> Chooser {",
        "pub const fn class(&self) -> RefusalClass {",
        "pub const fn config(&self) -> &ProcessConfig {",
        "pub const fn config(&self) -> &ProcessConfig {",
        "pub const fn contains(self, node: NodeId) -> bool {",
        "pub const fn crashes(&self) -> u32 {",
        "pub const fn inconclusive_reason(&self) -> Option<&'static str> {",
        "pub const fn is_up(&self, node: NodeId) -> bool {",
        "pub const fn max_crashes(&self) -> u32 {",
        "pub const fn max_pending(&self) -> u32 {",
        "pub const fn max_retained_bytes(&self) -> u64 {",
        "pub const fn new(",
        "pub const fn new(config: ProcessConfig) -> Self {",
        "pub const fn nodes(&self) -> u8 {",
        "pub const fn replicated_register_scenario(",
        "pub const fn restart(&self) -> bool {",
        "pub const fn retained_bytes(&self) -> u64 {",
        "pub const fn retained_bytes(&self) -> u64 {",
        "pub const fn statement(self) -> &'static str {",
        "pub const fn step(&self) -> Step {",
        "pub const fn support(self) -> Support {",
        "pub const fn token(self) -> &'static str {",
        "pub const fn unsupported_semantic(&self) -> Option<Semantic> {",
        "pub const fn up(&self) -> NodeSet {",
        "pub const fn with(self, node: NodeId) -> Self {",
        "pub const fn without(self, node: NodeId) -> Self {",
        "pub enum Bound {",
        "pub enum Chooser {",
        "pub enum ConfigRefusal {",
        "pub enum Event {",
        "pub enum FidelityClass {",
        "pub enum HostQualification {",
        "pub enum IndependenceClaim {",
        "pub enum Malformed {",
        "pub enum NotEnabled {",
        "pub enum Refusal {",
        "pub enum RefusalClass {",
        "pub enum Semantic {",
        "pub enum Step {",
        "pub enum Support {",
        "pub fn apply(&mut self, step: &Step) -> Result<&Event, Refusal> {",
        "pub fn canonical_bytes(&self) -> Vec<u8> {",
        "pub fn check(&self, step: &Step) -> Result<(), Refusal> {",
        "pub fn choice_log(&self) -> impl Iterator<Item = Step> + '_ {",
        "pub fn enabled_choices(&self) -> Vec<Step> {",
        "pub fn encode(&self) -> Vec<u8> {",
        "pub fn encode(&self) -> Vec<u8> {",
        "pub fn epoch(&self, node: NodeId) -> Option<Epoch> {",
        "pub fn events(&self) -> &[Event] {",
        "pub fn events(&self) -> &[Event] {",
        "pub fn into_journal(self) -> Journal {",
        "pub fn pending(&self) -> impl Iterator<Item = TicketId> + '_ {",
        "pub fn replay(&self) -> Result<Self, RunRefusal> {",
        "pub fn run(config: ProcessConfig, steps: &[Step]) -> Result<Journal, RunRefusal> {",
        "pub fn ticket(&self, ticket: TicketId) -> Option<(NodeId, Epoch)> {",
        "pub fn ticket_count(&self) -> usize {",
        "pub host: HostQualification,",
        "pub independence: IndependenceClaim,",
        "pub index: usize,",
        "pub major: u16,",
        "pub minor: u16,",
        "pub mod lab;",
        "pub mod profile;",
        "pub mod refusal;",
        "pub mod step;",
        "pub name: &'static str,",
        "pub patch: u16,",
        "pub refusal: Refusal,",
        "pub struct Epoch(pub u32);",
        "pub struct FidelityProfile {",
        "pub struct Journal {",
        "pub struct NodeId(pub u8);",
        "pub struct NodeSet(pub u64);",
        "pub struct Process {",
        "pub struct ProcessConfig {",
        "pub struct ProfileVersion {",
        "pub struct RunRefusal {",
        "pub struct TicketId(pub u32);",
        "pub use lab::{Journal, Process, run};",
        "pub use profile::{CRASH_RESTART_V0, FidelityClass, FidelityProfile, Semantic, Support};",
        "pub use refusal::{Refusal, RefusalClass, RunRefusal};",
        "pub use step::{Epoch, Event, NodeId, NodeSet, ProcessConfig, Step, TicketId};",
        "pub version: ProfileVersion,",
        "pub(crate) fn put_str(out: &mut Vec<u8>, text: &str) {",
        "pub(crate) fn u32_len(len: usize) -> u32 {",
    ];

    /// `continuum-effects-network` stopped being a zero-pub-item scaffold when bn-3ohe
    /// landed the `network/adversarial-v0` Lab pack (PR-15 / IMPL-01). The property the
    /// scaffold sweep guards — no crate on that list can execute a host effect on an
    /// agent's behalf — is re-established here against the surface that now exists,
    /// with the Forge audit's four legs:
    ///
    /// 1. the audited surface is exactly [`EFFECTS_NETWORK_PUBLIC_SURFACE`];
    /// 2. no host effect is possible, by the compiler: the crate is `#![no_std]`
    ///    unconditionally (no `cfg_attr`, no features, no `extern crate std`), and its
    ///    compiler lane `tests/pr15_no_std_lane.rs` — a copy with a planted
    ///    `use std::net::UdpSocket;` must not compile — exists and is not ignored;
    /// 3. no declared dependency, so it cannot reach an effect through a neighbour;
    /// 4. it is unreachable from an agent connection: `continuumd` does not declare it,
    ///    and no `continuumd` source names it.
    ///
    /// Any leg turning red means the network pack has acquired authority the docs/49
    /// audit below has never examined — a production handler with a real socket is
    /// exactly that — and the audit must be redone before the change lands.
    #[test]
    fn boundary_effects_network_grew_a_lab_pack_and_it_reaches_no_host_effect() {
        // 1. The audited surface, exactly.
        let mut items: Vec<String> = effects_network_sources()
            .iter()
            .flat_map(|(_, text)| {
                pub_items(text)
                    .into_iter()
                    .map(str::to_owned)
                    .collect::<Vec<String>>()
            })
            .collect();
        items.sort();
        let audited: Vec<String> = EFFECTS_NETWORK_PUBLIC_SURFACE
            .iter()
            .map(|it| (*it).to_owned())
            .collect();
        assert_eq!(
            items, audited,
            "continuum-effects-network's public surface is not the one INV-015 audited; \
             record the new items here and re-derive this test's four legs against them"
        );

        // 2. No host effect, by the compiler rather than by a source scan (bn-3ohe,
        // cr-1dl1d7: a substring scan misses `use std::{os::unix::net::UnixDatagram as
        // U}`). The crate is `#![no_std]` unconditionally, so it links only `core` and
        // `alloc`, and `unsafe_code` is forbidden, so there is no FFI route. The live
        // proof is the pack's own compiler lane, which must exist and not be ignored;
        // the structure below is what keeps that lane meaningful.
        if let Err(why) = no_std_unconditional(
            &effects_network_sources(),
            EFFECTS_NETWORK_MANIFEST,
            EFFECTS_NETWORK_NO_STD_LANE,
            NETWORK_NO_STD_LANE_TEST,
        ) {
            panic!("continuum-effects-network is not provably free of host effects: {why}");
        }

        // 3. No declared dependency.
        assert_eq!(
            real_pack_problem("continuum-effects-network"),
            Ok(()),
            "continuum-effects-network gained a dependency; re-derive what it can reach"
        );
        // No build script and no build dependency either: build-time code runs on the
        // host. Both are judged by Cargo's resolved view, `cargo metadata --locked`
        // (cr-35ujnx round 5), never by the manifest's text.
        assert_eq!(
            real_pack_problem("continuum-effects-network"),
            Ok(()),
            "continuum-effects-network declares a build script or build dependency"
        );
        assert!(
            !std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../continuum-effects-network/build.rs")
                .exists(),
            "continuum-effects-network grew a build.rs"
        );

        // 4. Unreachable from an agent connection.
        assert!(
            !CONTINUUMD_MANIFEST.contains("continuum-effects-network"),
            "continuumd now declares continuum-effects-network"
        );
        for (path, text) in continuumd_sources() {
            let callers: Vec<&str> = text
                .lines()
                .map(str::trim_start)
                .filter(|line| !line.starts_with("//"))
                .filter(|line| line.contains("continuum_effects_network"))
                .collect();
            assert!(
                callers.is_empty(),
                "{} names continuum_effects_network: {callers:?}",
                path.display()
            );
        }

        // And the crate still declares the contract this file holds it to.
        pin(
            "continuum-effects-network/src/lib.rs",
            EFFECTS_NETWORK_LIB,
            &[
                "**No host semantics** (docs/09 T06)",
                "This crate has no dependencies.",
            ],
        );
    }

    /// `continuum-effects-process` stopped being a zero-pub-item scaffold when bn-3mmf
    /// landed the `process/crash-restart-v0` Lab pack (PR-15 / IMPL-02). The property
    /// the scaffold sweep guards is re-established against the surface that now exists,
    /// with the same four legs as the network pack:
    ///
    /// 1. the audited surface is exactly [`EFFECTS_PROCESS_PUBLIC_SURFACE`];
    /// 2. no host effect is possible, by the compiler: the crate is `#![no_std]`
    ///    unconditionally, and its compiler lane `tests/pr15_no_std_lane.rs` — a copy with
    ///    a planted `std::net` use must not compile — exists and is not ignored;
    /// 3. no declared dependency and no build script;
    /// 4. it is unreachable from an agent connection: `continuumd` does not declare it,
    ///    and no `continuumd` source names it.
    ///
    /// A process pack is where "spawn a worker" or "kill a process" would land; any leg
    /// turning red means it has acquired authority the docs/49 audit has never examined.
    #[test]
    fn boundary_effects_process_grew_a_lab_pack_and_it_reaches_no_host_effect() {
        // 1. The audited surface, exactly.
        let mut items: Vec<String> = effects_process_sources()
            .iter()
            .flat_map(|(_, text)| {
                pub_items(text)
                    .into_iter()
                    .map(str::to_owned)
                    .collect::<Vec<String>>()
            })
            .collect();
        items.sort();
        let audited: Vec<String> = EFFECTS_PROCESS_PUBLIC_SURFACE
            .iter()
            .map(|it| (*it).to_owned())
            .collect();
        assert_eq!(
            items, audited,
            "continuum-effects-process's public surface is not the one INV-015 audited; \
             record the new items here and re-derive this test's four legs against them"
        );

        // 2. No host effect, by the compiler rather than by a source scan.
        if let Err(why) = no_std_unconditional(
            &effects_process_sources(),
            EFFECTS_PROCESS_MANIFEST,
            EFFECTS_PROCESS_NO_STD_LANE,
            PROCESS_NO_STD_LANE_TEST,
        ) {
            panic!("continuum-effects-process is not provably free of host effects: {why}");
        }

        // 3. No declared dependency, no build script, no build dependency.
        assert_eq!(
            real_pack_problem("continuum-effects-process"),
            Ok(()),
            "continuum-effects-process gained a dependency; re-derive what it can reach"
        );
        assert_eq!(
            real_pack_problem("continuum-effects-process"),
            Ok(()),
            "continuum-effects-process declares a build script or build dependency"
        );
        assert!(
            !std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../continuum-effects-process/build.rs")
                .exists(),
            "continuum-effects-process grew a build.rs"
        );

        // 4. Unreachable from an agent connection.
        assert!(
            !CONTINUUMD_MANIFEST.contains("continuum-effects-process"),
            "continuumd now declares continuum-effects-process"
        );
        for (path, text) in continuumd_sources() {
            let callers: Vec<&str> = text
                .lines()
                .map(str::trim_start)
                .filter(|line| !line.starts_with("//"))
                .filter(|line| line.contains("continuum_effects_process"))
                .collect();
            assert!(
                callers.is_empty(),
                "{} names continuum_effects_process: {callers:?}",
                path.display()
            );
        }

        // And the crate still declares the contract this file holds it to.
        pin(
            "continuum-effects-process/src/lib.rs",
            EFFECTS_PROCESS_LIB,
            &[
                "**No host semantics** (docs/09 T06)",
                "This crate has no dependencies.",
            ],
        );
    }

    /// `continuum-effects-storage`'s recorded public surface, sorted — every `pub` item
    /// across every source file of the crate, as `pub_items` reads them (bn-2fk3).
    const EFFECTS_STORAGE_PUBLIC_SURFACE: [&str; 110] = [
        "pub class: FidelityClass,",
        "pub const ALL: [Self; 17] = [",
        "pub const APPEND_LOG_V0: FidelityProfile = FidelityProfile {",
        "pub const ASSUMPTIONS: [(&str, &str); 6] = [",
        "pub const CANCELLATION_CONTRACT: [(&str, &str); 8] = [",
        "pub const COMPOSITION: [(&str, &str); 3] = [",
        "pub const JOURNAL_HEADER_BYTES: u64 =",
        "pub const MAX_CRASHES_CAP: u32 = 1 << 16;",
        "pub const MAX_NODES: u8 = 64;",
        "pub const MAX_PENDING_CAP: u32 = 1 << 16;",
        "pub const MAX_RETAINED_CAP: u64 = 1 << 28;",
        "pub const MAX_STEPS: usize = 1 << 20;",
        "pub const PROFILE_NAME: &str = \"storage/append-log-v0\";",
        "pub const PROFILE_VERSION: ProfileVersion = ProfileVersion {",
        "pub const fn all(n: u8) -> Self {",
        "pub const fn as_str(self) -> &'static str {",
        "pub const fn chooser(&self) -> Chooser {",
        "pub const fn class(&self) -> RefusalClass {",
        "pub const fn config(&self) -> &StorageConfig {",
        "pub const fn config(&self) -> &StorageConfig {",
        "pub const fn contains(self, node: NodeId) -> bool {",
        "pub const fn crashes(&self) -> u32 {",
        "pub const fn inconclusive_reason(&self) -> Option<&'static str> {",
        "pub const fn is_up(&self, node: NodeId) -> bool {",
        "pub const fn max_crashes(&self) -> u32 {",
        "pub const fn max_pending(&self) -> u32 {",
        "pub const fn max_retained_bytes(&self) -> u64 {",
        "pub const fn max_steps(&self) -> u32 {",
        "pub const fn new(",
        "pub const fn nodes(&self) -> u8 {",
        "pub const fn replicated_register_scenario(",
        "pub const fn restart(&self) -> bool {",
        "pub const fn retained_bytes(&self) -> u64 {",
        "pub const fn retained_bytes(&self) -> u64 {",
        "pub const fn statement(self) -> &'static str {",
        "pub const fn step(&self) -> Step {",
        "pub const fn suffix_loss(&self) -> bool {",
        "pub const fn support(self) -> Support {",
        "pub const fn token(self) -> &'static str {",
        "pub const fn torn(&self) -> bool {",
        "pub const fn unsupported_semantic(&self) -> Option<Semantic> {",
        "pub const fn up(&self) -> NodeSet {",
        "pub const fn with(self, node: NodeId) -> Self {",
        "pub const fn with_max_steps(self, max_steps: u32) -> Result<Self, ConfigRefusal> {",
        "pub const fn without(self, node: NodeId) -> Self {",
        "pub enum Bound {",
        "pub enum Chooser {",
        "pub enum ConfigRefusal {",
        "pub enum Entry {",
        "pub enum Event {",
        "pub enum FidelityClass {",
        "pub enum HostQualification {",
        "pub enum IndependenceClaim {",
        "pub enum Malformed {",
        "pub enum NotEnabled {",
        "pub enum ProgramFault {",
        "pub enum Refusal {",
        "pub enum RefusalClass {",
        "pub enum Semantic {",
        "pub enum Step {",
        "pub enum Support {",
        "pub fn apply(&mut self, step: &Step) -> Result<&Event, Refusal> {",
        "pub fn canonical_bytes(&self) -> Vec<u8> {",
        "pub fn check(&self, step: &Step) -> Result<(), Refusal> {",
        "pub fn choice_log(&self) -> impl Iterator<Item = Step> + '_ {",
        "pub fn enabled_choices(&self) -> Vec<Step> {",
        "pub fn encode(&self) -> Vec<u8> {",
        "pub fn encode(&self) -> Vec<u8> {",
        "pub fn epoch(&self, node: NodeId) -> Option<Epoch> {",
        "pub fn events(&self) -> &[Event] {",
        "pub fn events(&self) -> &[Event] {",
        "pub fn into_journal(self) -> Journal {",
        "pub fn log(&self, node: NodeId) -> Option<&[Entry]> {",
        "pub fn new(config: StorageConfig) -> Self {",
        "pub fn pending(&self) -> impl Iterator<Item = TicketId> + '_ {",
        "pub fn replay(&self) -> Result<Self, RunRefusal> {",
        "pub fn run(config: StorageConfig, steps: &[Step]) -> Result<Journal, RunRefusal> {",
        "pub fn stable_len(&self, node: NodeId) -> Option<u32> {",
        "pub fn ticket(&self, ticket: TicketId) -> Option<(NodeId, Epoch, u32, bool)> {",
        "pub fn ticket_count(&self) -> usize {",
        "pub host: HostQualification,",
        "pub independence: IndependenceClaim,",
        "pub index: usize,",
        "pub major: u16,",
        "pub minor: u16,",
        "pub mod lab;",
        "pub mod profile;",
        "pub mod refusal;",
        "pub mod step;",
        "pub name: &'static str,",
        "pub patch: u16,",
        "pub refusal: Refusal,",
        "pub struct Epoch(pub u32);",
        "pub struct FidelityProfile {",
        "pub struct Journal {",
        "pub struct NodeId(pub u8);",
        "pub struct NodeSet(pub u64);",
        "pub struct ProfileVersion {",
        "pub struct RunRefusal {",
        "pub struct Storage {",
        "pub struct StorageConfig {",
        "pub struct TicketId(pub u32);",
        "pub struct Value(pub u32);",
        "pub use lab::{Journal, Storage, run};",
        "pub use profile::{APPEND_LOG_V0, FidelityClass, FidelityProfile, Semantic, Support};",
        "pub use refusal::{Refusal, RefusalClass, RunRefusal};",
        "pub use step::{Entry, Epoch, Event, NodeId, NodeSet, Step, StorageConfig, TicketId, Value};",
        "pub version: ProfileVersion,",
        "pub(crate) fn put_str(out: &mut Vec<u8>, text: &str) {",
        "pub(crate) fn u32_len(len: usize) -> u32 {",
    ];

    /// `continuum-effects-storage` stopped being a zero-pub-item scaffold when bn-2fk3
    /// landed the `storage/append-log-v0` Lab pack (PR-15 / IMPL-03). The property the
    /// scaffold sweep guards is re-established against the surface that now exists,
    /// with the same four legs as the network and process packs:
    ///
    /// 1. the audited surface is exactly [`EFFECTS_STORAGE_PUBLIC_SURFACE`];
    /// 2. no host effect is possible, by the compiler: the crate is `#![no_std]`
    ///    unconditionally, and its compiler lane `tests/pr15_no_std_lane.rs` — a copy with
    ///    a planted `std::net` use must not compile — exists and is not ignored;
    /// 3. no declared dependency and no build script;
    /// 4. it is unreachable from an agent connection: `continuumd` does not declare it,
    ///    and no `continuumd` source names it.
    ///
    /// A storage pack is where "open a file" or "write to disk" would land; any leg
    /// turning red means it has acquired authority the docs/49 audit has never examined.
    #[test]
    fn boundary_effects_storage_grew_a_lab_pack_and_it_reaches_no_host_effect() {
        // 1. The audited surface, exactly.
        let mut items: Vec<String> = effects_storage_sources()
            .iter()
            .flat_map(|(_, text)| {
                pub_items(text)
                    .into_iter()
                    .map(str::to_owned)
                    .collect::<Vec<String>>()
            })
            .collect();
        items.sort();
        let audited: Vec<String> = EFFECTS_STORAGE_PUBLIC_SURFACE
            .iter()
            .map(|it| (*it).to_owned())
            .collect();
        assert_eq!(
            items, audited,
            "continuum-effects-storage's public surface is not the one INV-015 audited; \
             record the new items here and re-derive this test's four legs against them"
        );

        // 2. No host effect, by the compiler rather than by a source scan.
        if let Err(why) = no_std_unconditional(
            &effects_storage_sources(),
            EFFECTS_STORAGE_MANIFEST,
            EFFECTS_STORAGE_NO_STD_LANE,
            STORAGE_NO_STD_LANE_TEST,
        ) {
            panic!("continuum-effects-storage is not provably free of host effects: {why}");
        }

        // 3. No declared dependency, no build script, no build dependency.
        assert_eq!(
            real_pack_problem("continuum-effects-storage"),
            Ok(()),
            "continuum-effects-storage gained a dependency; re-derive what it can reach"
        );
        assert_eq!(
            real_pack_problem("continuum-effects-storage"),
            Ok(()),
            "continuum-effects-storage declares a build script or build dependency"
        );
        assert!(
            !std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../continuum-effects-storage/build.rs")
                .exists(),
            "continuum-effects-storage grew a build.rs"
        );

        // 4. Unreachable from an agent connection.
        assert!(
            !CONTINUUMD_MANIFEST.contains("continuum-effects-storage"),
            "continuumd now declares continuum-effects-storage"
        );
        for (path, text) in continuumd_sources() {
            let callers: Vec<&str> = text
                .lines()
                .map(str::trim_start)
                .filter(|line| !line.starts_with("//"))
                .filter(|line| line.contains("continuum_effects_storage"))
                .collect();
            assert!(
                callers.is_empty(),
                "{} names continuum_effects_storage: {callers:?}",
                path.display()
            );
        }

        // And the crate still declares the contract this file holds it to.
        pin(
            "continuum-effects-storage/src/lib.rs",
            EFFECTS_STORAGE_LIB,
            &[
                "**No host semantics** (docs/09 T06)",
                "This crate has no dependencies.",
            ],
        );
    }
}

// --- the docs/49 worker-isolation audit ----------------------------------------------------

mod isolation_controls {
    use super::{
        DOCS_49, DX14_CANCELLATION, INV003_EVIDENCE, INV005_EVIDENCE, LEAN_TOOLCHAIN, OBLIGATION,
        PLAN_MD, RUST_TOOLCHAIN, TASK_OPERATIONS_TESTS, continuumd_sources, isolation_bullets, pin,
    };

    /// A live daemon/task-grain enforcement of the same idea one layer above the
    /// (nonexistent) worker: the site that enforces it and the test that holds it.
    pub(super) struct Cousin {
        /// Where the daemon-grain enforcement lives.
        pub site: &'static str,
        /// The already-landed evidence, cited by content below.
        pub citation: &'static str,
    }

    /// One docs/49 worker-isolation control, audited honestly.
    ///
    /// `worker_scoped_producer` is `false` on every row today: no worker execution
    /// environment exists, so nothing mounts, unmounts, allowlists, or kills a worker.
    pub(super) struct ControlRow {
        /// The bullet exactly as docs/49 spells it (stripped of `- ` and `;`).
        pub bullet: &'static str,
        /// Whether anything in this workspace launches a worker under this control.
        pub worker_scoped_producer: bool,
        /// The daemon/task-grain cousin, where one is live.
        pub cousin: Option<Cousin>,
    }

    /// The audit, in docs/49's own order.
    pub(super) const CONTROLS: [ControlRow; 8] = [
        ControlRow {
            bullet: "pinned image/toolchain",
            worker_scoped_producer: false,
            // The repository's own toolchains are pinned (rust-toolchain.toml,
            // lean/lean-toolchain) — that is docs/49's *supply-chain* section at repo
            // grain, not a worker launch site; recorded as adjacent, not as this
            // control.
            cousin: None,
        },
        ControlRow {
            bullet: "read-only input mount",
            worker_scoped_producer: false,
            cousin: None,
        },
        ControlRow {
            bullet: "no ambient credentials",
            worker_scoped_producer: false,
            // INV-005's discipline is the in-process cousin: time and entropy are
            // injected capabilities, never ambient — but it governs the daemon, not a
            // worker mount, so it does not claim this row either.
            cousin: None,
        },
        ControlRow {
            bullet: "resource limits",
            worker_scoped_producer: false,
            cousin: Some(Cousin {
                site: "daemon/obligation.rs + daemon/budget.rs",
                citation: "fn budget_exhaustion_parks_a_continuation_that_update_budget_and_resume_complete",
            }),
        },
        ControlRow {
            bullet: "network disabled unless required and allowlisted",
            worker_scoped_producer: false,
            // The only transport that exists is local; the network effect pack is a
            // scaffold. Nothing to allowlist yet.
            cousin: None,
        },
        ControlRow {
            bullet: "output size/schema limits",
            worker_scoped_producer: false,
            // The schema half is live workspace-wide (INV-003's closure); the *size*
            // half is declared on the handshake and read by no dispatch path — see
            // `boundary_the_size_limit_half_is_declared_but_unread_by_dispatch`.
            cousin: Some(Cousin {
                site: "schema closure (INV-003), size half declared-only",
                citation: "fn the_closure_check_is_not_vacuous",
            }),
        },
        ControlRow {
            bullet: "cancellation and hard-kill fallback",
            worker_scoped_producer: false,
            // Cancellation is live at task grain; a hard-kill fallback presumes a
            // process to kill, and there is none.
            cousin: Some(Cousin {
                site: "task.cancel at every lifecycle point (dx14)",
                citation: "fn cancellation_is_correct_at_every_lifecycle_point_a_caller_can_reach",
            }),
        },
        ControlRow {
            bullet: "audit trace",
            worker_scoped_producer: false,
            cousin: Some(Cousin {
                site: "daemon/obligation.rs audit_required + dispatcher denial audit",
                citation: "fn every_admission_decision_is_recorded_whichever_way_it_went",
            }),
        },
    ];

    /// The table above is docs/49's own list, row for row and in order — a hand edit
    /// to either fails here (the inv003 anti-drift device).
    #[test]
    fn positive_the_audit_table_matches_docs49s_own_bullet_list() {
        let bullets = isolation_bullets(DOCS_49);
        assert_eq!(
            bullets.len(),
            CONTROLS.len(),
            "docs/49's worker-isolation list is no longer eight controls"
        );
        for (row, bullet) in CONTROLS.iter().zip(&bullets) {
            assert_eq!(row.bullet, bullet, "the audit table drifted from docs/49");
        }
    }

    /// Every claimed cousin cites a test that exists, in the suite its site names.
    #[test]
    fn positive_every_daemon_grain_cousin_is_a_live_citation() {
        let suites = [
            ("daemon_task_operations.rs", TASK_OPERATIONS_TESTS),
            ("dx14_cancellation_matrix.rs", DX14_CANCELLATION),
            ("inv003_no_prose_only_evidence.rs", INV003_EVIDENCE),
            ("daemon_operations.rs", super::DAEMON_OPERATIONS_TESTS),
        ];
        for row in &CONTROLS {
            let Some(cousin) = &row.cousin else { continue };
            assert!(
                suites
                    .iter()
                    .any(|(_, text)| text.contains(cousin.citation)),
                "no suite contains {:?} (cited for {:?} at {})",
                cousin.citation,
                row.bullet,
                cousin.site
            );
        }
        // The audit cousin's obligation half, pinned at its enforcement site.
        pin(
            "daemon/obligation.rs",
            OBLIGATION,
            &["on every result of an `@audit_recorded` operation"],
        );
        // The no-ambient-credentials *adjacent* discipline is a real, cited suite even
        // though it claims no row: INV-005's evidence file exists and is non-empty.
        assert!(
            INV005_EVIDENCE.contains("INV-005"),
            "the INV-005 evidence file no longer names its invariant"
        );
    }

    /// The honest headline: zero of the eight controls have a worker-scoped producer,
    /// four have live daemon/task-grain cousins — and the reason this does not falsify
    /// INV-015 today is pinned from plan §24.5's own frontier row: autonomous
    /// promotion stays disabled until the Phase B red-team corpus (one escape attempt
    /// per control) clears, and no worker execution environment exists for an agent to
    /// escape from in the first place.
    #[test]
    fn boundary_no_worker_execution_environment_exists_today() {
        assert!(
            CONTROLS.iter().all(|row| !row.worker_scoped_producer),
            "a worker-scoped producer appeared; this audit is stale"
        );
        assert_eq!(
            CONTROLS.iter().filter(|row| row.cousin.is_some()).count(),
            4,
            "the daemon-grain cousin count moved; re-audit the table"
        );
        pin(
            "plan.md",
            PLAN_MD,
            &[
                "one escape attempt against each of the eight worker-isolation controls of docs/49",
                "Autonomous promotion stays disabled",
                "leaving human-approved promotion as the standing fallback",
            ],
        );
        // The supply-chain-grain pins adjacent to control one, recorded as facts.
        pin(
            "rust-toolchain.toml",
            RUST_TOOLCHAIN,
            &["channel = \"1.97.0\""],
        );
        pin(
            "lean/lean-toolchain",
            LEAN_TOOLCHAIN,
            &["leanprover/lean4:"],
        );
    }

    /// Gap row five, swept live: `max_result_bytes` is declared on the handshake and
    /// read by no `daemon/` source. When enforcement lands this fails, and the
    /// output-size half of control six flips from declared to live.
    #[test]
    fn boundary_the_size_limit_half_is_declared_but_unread_by_dispatch() {
        assert!(
            super::HANDSHAKE.contains("max_result_bytes"),
            "the declaration itself moved"
        );
        for (path, text) in continuumd_sources() {
            if path.components().any(|part| part.as_os_str() == "daemon") {
                assert!(
                    !text.contains("max_result_bytes"),
                    "{} now reads max_result_bytes; flip the output-size row to live",
                    path.display()
                );
            }
        }
    }
}

// --- the adapter grammar (continuum-mcp), cited --------------------------------------------

mod mcp_grammar {
    use super::{MCP_CLIENT, MCP_LIB, MCP_REGISTER, MCP_TYPED_SURFACE, pin};

    /// The eight rows of `register::OPERATIONS`, pinned individually and counted, so
    /// the grammar this map describes is the grammar that ships. Typed operations over
    /// explicit handles: every requirement names a handle the caller holds, never an
    /// authority the daemon owns.
    #[test]
    fn positive_the_register_is_a_typed_grammar_of_exactly_the_client_surface() {
        pin(
            "continuum-mcp/src/register.rs",
            MCP_REGISTER,
            &[
                r#"operation: "workspace.create""#,
                r#"operation: "workspace.fork""#,
                r#"operation: "workspace.seal""#,
                r#"operation: "verification.start""#,
                r#"operation: "verification.result""#,
                r#"operation: "task.status""#,
                r#"operation: "task.cancel""#,
                r#"operation: "task.resume""#,
                "requires: Requirement::SealedSnapshot",
                "Preconditions are *necessary*, never merely typical.",
            ],
        );
        assert_eq!(
            MCP_REGISTER.matches(r#"operation: ""#).count(),
            8,
            "the register grew or shrank; re-audit the adapter surface"
        );
    }

    /// The adapter narrows and never widens: its own contract sentences, and the live
    /// test in its own suite that dispatches the widening attempt and is refused.
    #[test]
    fn positive_the_adapter_narrows_and_never_widens() {
        pin(
            "continuum-mcp/src/lib.rs",
            MCP_LIB,
            &["may only narrow capabilities, never"],
        );
        pin(
            "continuum-mcp/src/client.rs",
            MCP_CLIENT,
            &["nothing here can widen it"],
        );
        pin(
            "continuum-mcp/tests/typed_surface.rs",
            MCP_TYPED_SURFACE,
            &["fn speaking_as_a_reader_cannot_buy_an_execute_operation"],
        );
    }
}

// --- anti-vacuity: every evaluating leg detects its mutant ---------------------------------

mod mutants {
    use std::collections::BTreeSet;

    use super::{
        ADMISSION, CAPABILITY, DOCS_49, EFFECTS_TIME_LIB, EVIDENCE, FORGE_MANIFEST,
        NON_FILESYSTEM_FACILITIES, SECURITY_MANIFEST, administrative_callers,
        declared_dependencies, forge_sources, host_effect_lines, is_signing_source,
        isolation_bullets, missing_pin, pub_items, security_sources, unexempted_callers,
    };
    use crate::privileged_perimeter::{PRIVILEGED, privileged_names};
    use std::path::Path;

    /// Dropping one bullet from a copy of docs/49 must break the extraction count, and
    /// an unrelated document must extract nothing.
    #[test]
    fn negative_the_docs49_bullet_extraction_is_not_vacuous() {
        let doctored = DOCS_49.replacen("- resource limits;\n", "", 1);
        assert_ne!(doctored, DOCS_49, "the doctoring did nothing");
        assert_eq!(
            isolation_bullets(&doctored).len(),
            7,
            "a missing control went undetected"
        );
    }

    /// The privileged-set comparison catches both directions: a smuggled member and a
    /// dropped one.
    #[test]
    fn negative_the_privileged_set_comparison_is_not_vacuous() {
        let expected: BTreeSet<&str> = PRIVILEGED.into_iter().collect();

        let smuggled = privileged_names(
            PRIVILEGED
                .into_iter()
                .chain(std::iter::once("workspace.create")),
        );
        assert_ne!(
            smuggled, expected,
            "a sixth privileged operation went undetected"
        );

        let dropped = privileged_names(
            PRIVILEGED
                .into_iter()
                .filter(|name| *name != "repair.promote"),
        );
        assert_ne!(
            dropped, expected,
            "a dropped privileged operation went undetected"
        );
    }

    /// Each load-bearing source pin, re-run against a copy with the pinned line
    /// stripped, must report the very needle that was stripped.
    #[test]
    fn negative_the_source_pins_are_not_vacuous() {
        let pins: [(&str, &str); 4] = [
            (CAPABILITY, "no operation in this protocol can widen"),
            (
                EVIDENCE,
                "Its fields are private to this module and it has no public constructor",
            ),
            (ADMISSION, "An absent profile is the fail-closed reading"),
            (EVIDENCE, "the caller is not a `service:` actor"),
        ];
        for (source, needle) in pins {
            assert_eq!(
                missing_pin(source, &[needle]),
                None,
                "the pin does not hold against the real source"
            );
            let doctored = source.replacen(needle, "", 1);
            assert_eq!(
                missing_pin(&doctored, &[needle]),
                Some(needle),
                "stripping the pinned text went undetected"
            );
        }
    }

    /// A planted administrative caller is found, and comment lines are not counted —
    /// the sweep reads code, not prose about code.
    #[test]
    fn negative_the_mint_sweep_detects_a_planted_caller() {
        let planted =
            "fn escalate(store: &ReferenceStore) {\n    let _ = store.mint(admin, wide);\n}\n";
        assert_eq!(administrative_callers(planted).len(), 1);
        let commented = "// a comment saying store.mint( is out of band\n";
        assert!(administrative_callers(commented).is_empty());
        // The signer-administration exemption (protocol 3.8) is exactly that wide: only the
        // two pinned lines, only on a `.registry` receiver, and only in `daemon/signing.rs`.
        let signing = Path::new("crates/continuumd/src/daemon/signing.rs");
        let signer = "    let key = authority\n        .registry\n        .mint(&actor, entropy)\n        .map_err(entropy_fault)?;\n";
        assert!(unexempted_callers(signing, signer).is_empty());
        assert_eq!(
            unexempted_callers(Path::new("src/daemon/other.rs"), signer).len(),
            1
        );
        let other_receiver = "    let key = authority\n        .grants\n        .mint(&actor, entropy)\n        .map_err(entropy_fault)?;\n";
        assert_eq!(unexempted_callers(signing, other_receiver).len(), 1);
        let other_call = "    authority\n        .registry\n        .revoke(&cap);\n";
        assert_eq!(unexempted_callers(signing, other_call).len(), 1);
        // A second identical call on another receiver is judged by its own line.
        let twice = format!("{signer}    grants\n        .mint(&actor, entropy)\n");
        assert_eq!(unexempted_callers(signing, &twice).len(), 1);
    }

    /// A scaffold that grows a public item is detected by the same scanner the
    /// boundary test runs.
    #[test]
    fn negative_the_scaffold_sweep_detects_a_grown_crate() {
        assert!(
            pub_items(EFFECTS_TIME_LIB).is_empty(),
            "the real scaffold is clean"
        );
        let grown = format!("{EFFECTS_TIME_LIB}\npub fn spawn_worker() {{}}\n");
        assert_eq!(pub_items(&grown), vec!["pub fn spawn_worker() {}"]);
    }

    /// The Forge audit's own legs are shown capable of failing, so the crate that
    /// *stopped* being a scaffold is held by a live check rather than by a recorded
    /// hope. Each leg is re-run against a doctored copy of a real input.
    #[test]
    fn negative_the_forge_host_effect_audit_detects_its_mutants() {
        // Leg 2: a planted effect is found, and prose about an effect is not.
        let planted =
            "fn escape() {\n    let _ = std::process::Command::new(\"sh\").status();\n}\n";
        assert_eq!(host_effect_lines(planted).len(), 1);
        assert!(
            host_effect_lines("// this lane never calls std::process::Command\n").is_empty(),
            "a comment about an effect is not an effect"
        );
        // And the real crate is clean by the same predicate.
        for (_, text) in forge_sources() {
            assert!(host_effect_lines(&text).is_empty());
        }

        // Leg 1: an added public item is detected.
        let (_, first) = forge_sources()
            .into_iter()
            .next()
            .expect("continuum-forge has at least one source");
        let grown = format!("{first}\npub fn run_candidate() {{}}\n");
        assert!(
            pub_items(&grown).contains(&"pub fn run_candidate() {}"),
            "an added public item must be visible to the inventory"
        );

        // Leg 3: a widened dependency set is detected.
        let widened = FORGE_MANIFEST.replacen(
            "[dependencies]\ncontinuum-intent",
            "[dependencies]\ncontinuum-effects-process = { path = \"../continuum-effects-process\" }\ncontinuum-intent",
            1,
        );
        assert_ne!(widened, FORGE_MANIFEST, "the doctoring did nothing");
        assert!(
            declared_dependencies(&widened).contains(&"continuum-effects-process".to_owned()),
            "a planted effect dependency must be visible to the manifest read"
        );

        // Leg 4: a planted daemon caller is detected by the same line predicate.
        let caller = "fn dispatch() {\n    continuum_forge::task::assemble(&contract, &env);\n}\n";
        assert_eq!(
            caller
                .lines()
                .map(str::trim_start)
                .filter(|line| !line.starts_with("//"))
                .filter(|line| line.contains("continuum_forge"))
                .count(),
            1
        );
    }

    /// The corpus crate's audit is held to the same standard as Forge's: each of its four
    /// legs is re-run against a doctored copy of a real input and shown to fail.
    ///
    /// Leg 2 carries the extra weight here. `continuum-security`'s payloads are prose
    /// *about* spawning processes and exfiltrating files, so the predicate has to separate
    /// a payload that talks about an effect from code that performs one — and the real
    /// corpus has to be clean under it, which is asserted rather than assumed.
    #[test]
    fn negative_the_security_corpus_audit_detects_its_mutants() {
        // Leg 2: a planted effect is found, prose about an effect is not, and a corpus
        // payload describing an escape is not mistaken for one.
        let planted =
            "fn exfiltrate() {\n    let _ = std::process::Command::new(\"nc\").status();\n}\n";
        assert_eq!(host_effect_lines(planted).len(), 1);
        assert!(
            host_effect_lines("// the corpus never calls std::process::Command\n").is_empty(),
            "a comment about an effect is not an effect"
        );
        // And the real crate is clean by the same predicate — the corpus's hostile strings
        // included.
        for (path, text) in security_sources() {
            if !is_signing_source(&path) {
                assert!(host_effect_lines(&text).is_empty());
            }
        }
        // And a process spawn planted in a signing source is still caught.
        let spawned = "let _ = std::process::Command::new(\"sh\").status();\n";
        assert!(
            host_effect_lines(spawned)
                .iter()
                .any(|line| NON_FILESYSTEM_FACILITIES.iter().any(|f| line.contains(f)))
        );

        // Leg 1: an added public item is detected.
        let (_, first) = security_sources()
            .into_iter()
            .next()
            .expect("continuum-security has at least one source");
        let grown = format!("{first}\npub fn run_payload() {{}}\n");
        assert!(
            pub_items(&grown).contains(&"pub fn run_payload() {}"),
            "an added public item must be visible to the inventory"
        );

        // Leg 3: a widened dependency set is detected.
        let widened = SECURITY_MANIFEST.replacen(
            "[dependencies]\ncontinuum-evidence",
            "[dependencies]\ncontinuum-effects-process = { path = \"../continuum-effects-process\" }\ncontinuum-evidence",
            1,
        );
        assert_ne!(widened, SECURITY_MANIFEST, "the doctoring did nothing");
        assert!(
            declared_dependencies(&widened).contains(&"continuum-effects-process".to_owned()),
            "a planted effect dependency must be visible to the manifest read"
        );

        // Leg 4: a planted daemon caller is detected by the same line predicate.
        let caller = "fn dispatch() {\n    continuum_security::injection::CASES.len();\n}\n";
        assert_eq!(
            caller
                .lines()
                .map(str::trim_start)
                .filter(|line| !line.starts_with("//"))
                .filter(|line| line.contains("continuum_security"))
                .count(),
            1
        );
    }
}

// --- the no_std leg can fail -----------------------------------------------------------------

mod no_std_leg_mutants {
    use std::path::PathBuf;

    use super::{
        EFFECTS_NETWORK_MANIFEST, EFFECTS_NETWORK_NO_STD_LANE, EFFECTS_PROCESS_MANIFEST,
        EFFECTS_PROCESS_NO_STD_LANE, EFFECTS_STORAGE_MANIFEST, EFFECTS_STORAGE_NO_STD_LANE,
        NETWORK_NO_STD_LANE_TEST, PROCESS_NO_STD_LANE_TEST, STORAGE_NO_STD_LANE_TEST,
        effects_network_sources, effects_process_sources, effects_storage_sources,
        no_std_unconditional,
    };

    /// A named doctored copy: sources, manifest, lane.
    type Mutant = (&'static str, Vec<(PathBuf, String)>, String, String);

    fn with_lib(edit: impl Fn(&str) -> String) -> Vec<(PathBuf, String)> {
        effects_network_sources()
            .into_iter()
            .map(|(path, text)| {
                if path.ends_with("lib.rs") {
                    let edited = edit(&text);
                    (path, edited)
                } else {
                    (path, text)
                }
            })
            .collect()
    }

    /// Each way to make `no_std` conditional, or to reach std anyway, or to switch the
    /// lane off, fails the leg; the real crate passes it.
    #[test]
    fn negative_the_no_std_leg_detects_each_way_around_it() {
        let real = effects_network_sources();
        assert_eq!(
            no_std_unconditional(
                &real,
                EFFECTS_NETWORK_MANIFEST,
                EFFECTS_NETWORK_NO_STD_LANE,
                NETWORK_NO_STD_LANE_TEST
            ),
            Ok(())
        );
        let mutants: Vec<Mutant> = vec![
            (
                "no_std removed, left as a string decoy",
                with_lib(|t| {
                    t.replacen("\n#![no_std]\n", "\nconst _D: &str = \"#![no_std]\";\n", 1)
                }),
                EFFECTS_NETWORK_MANIFEST.to_owned(),
                EFFECTS_NETWORK_NO_STD_LANE.to_owned(),
            ),
            (
                "cfg_attr(no_std)",
                with_lib(|t| {
                    t.replacen("\n#![no_std]\n", "\n#![cfg_attr(not(test), no_std)]\n", 1)
                }),
                EFFECTS_NETWORK_MANIFEST.to_owned(),
                EFFECTS_NETWORK_NO_STD_LANE.to_owned(),
            ),
            (
                "extern crate std",
                with_lib(|t| {
                    t.replace(
                        "extern crate alloc;",
                        "extern crate alloc;\nextern crate std;",
                    )
                }),
                EFFECTS_NETWORK_MANIFEST.to_owned(),
                EFFECTS_NETWORK_NO_STD_LANE.to_owned(),
            ),
            (
                "a std feature",
                real.clone(),
                format!("{EFFECTS_NETWORK_MANIFEST}\n[features]\nstd = []\n"),
                EFFECTS_NETWORK_NO_STD_LANE.to_owned(),
            ),
            (
                "a raw-string decoy",
                with_lib(|t| {
                    t.replacen(
                        "\n#![no_std]\n",
                        "\nconst _D: &str = r\"\n#![no_std]\n\";\n",
                        1,
                    )
                }),
                EFFECTS_NETWORK_MANIFEST.to_owned(),
                EFFECTS_NETWORK_NO_STD_LANE.to_owned(),
            ),
            (
                "no_std in a nested module",
                with_lib(|t| t.replacen("\n#![no_std]\n", "\nmod m {\n#![no_std]\n}\n", 1)),
                EFFECTS_NETWORK_MANIFEST.to_owned(),
                EFFECTS_NETWORK_NO_STD_LANE.to_owned(),
            ),
            (
                "extern crate r#std under a cfg",
                with_lib(|t| {
                    t.replacen(
                        "extern crate alloc;",
                        "extern crate alloc;\n#[cfg(not(debug_assertions))]\nextern crate r#std;",
                        1,
                    )
                }),
                EFFECTS_NETWORK_MANIFEST.to_owned(),
                EFFECTS_NETWORK_NO_STD_LANE.to_owned(),
            ),
            (
                "extern crate through a macro",
                with_lib(|t| {
                    format!(
                        "{t}\nmacro_rules! k {{ ($c:ident) => {{ extern crate $c; }} }}\nk!(std);\n"
                    )
                }),
                EFFECTS_NETWORK_MANIFEST.to_owned(),
                EFFECTS_NETWORK_NO_STD_LANE.to_owned(),
            ),
            (
                "a target dependency",
                real.clone(),
                format!(
                    "{EFFECTS_NETWORK_MANIFEST}\n[target.'cfg(windows)'.dependencies]\nx = \"1\"\n"
                ),
                EFFECTS_NETWORK_NO_STD_LANE.to_owned(),
            ),
            (
                "the lane inverted",
                real.clone(),
                EFFECTS_NETWORK_MANIFEST.to_owned(),
                EFFECTS_NETWORK_NO_STD_LANE.replacen("#[test]", "#[test]\n#[should_panic]", 1),
            ),
            (
                "the lane ignored",
                real.clone(),
                EFFECTS_NETWORK_MANIFEST.to_owned(),
                EFFECTS_NETWORK_NO_STD_LANE.replace("#[test]", "#[test]\n#[ignore]"),
            ),
            (
                "the lane gone",
                real.clone(),
                EFFECTS_NETWORK_MANIFEST.to_owned(),
                String::new(),
            ),
        ];
        for (name, sources, manifest, lane) in mutants {
            assert!(
                no_std_unconditional(&sources, &manifest, &lane, NETWORK_NO_STD_LANE_TEST).is_err(),
                "{name} passed the no_std leg"
            );
        }
    }

    /// The process pack's leg is the same predicate over that crate's inputs: the real
    /// crate passes it, and the lane is bound to the process lane's own test name, so
    /// the network lane's text cannot stand in for it.
    #[test]
    fn negative_the_process_no_std_leg_detects_its_mutants() {
        let real = effects_process_sources();
        assert_eq!(
            no_std_unconditional(
                &real,
                EFFECTS_PROCESS_MANIFEST,
                EFFECTS_PROCESS_NO_STD_LANE,
                PROCESS_NO_STD_LANE_TEST
            ),
            Ok(())
        );
        let with_process_lib = |edit: &dyn Fn(&str) -> String| -> Vec<(PathBuf, String)> {
            real.iter()
                .map(|(path, text)| {
                    let text = if path.ends_with("lib.rs") {
                        edit(text)
                    } else {
                        text.clone()
                    };
                    (path.clone(), text)
                })
                .collect()
        };
        let mutants: Vec<Mutant> = vec![
            (
                "cfg_attr(no_std)",
                with_process_lib(&|t| {
                    t.replacen("\n#![no_std]\n", "\n#![cfg_attr(not(test), no_std)]\n", 1)
                }),
                EFFECTS_PROCESS_MANIFEST.to_owned(),
                EFFECTS_PROCESS_NO_STD_LANE.to_owned(),
            ),
            (
                "extern crate std",
                with_process_lib(&|t| {
                    t.replace(
                        "extern crate alloc;",
                        "extern crate alloc;\nextern crate std;",
                    )
                }),
                EFFECTS_PROCESS_MANIFEST.to_owned(),
                EFFECTS_PROCESS_NO_STD_LANE.to_owned(),
            ),
            (
                "a std feature",
                real.clone(),
                format!("{EFFECTS_PROCESS_MANIFEST}\n[features]\nstd = []\n"),
                EFFECTS_PROCESS_NO_STD_LANE.to_owned(),
            ),
            (
                "the lane ignored",
                real.clone(),
                EFFECTS_PROCESS_MANIFEST.to_owned(),
                EFFECTS_PROCESS_NO_STD_LANE.replace("#[test]", "#[test]\n#[ignore]"),
            ),
            (
                "the network lane in its place",
                real.clone(),
                EFFECTS_PROCESS_MANIFEST.to_owned(),
                EFFECTS_NETWORK_NO_STD_LANE.to_owned(),
            ),
        ];
        for (name, sources, manifest, lane) in mutants {
            assert!(
                no_std_unconditional(&sources, &manifest, &lane, PROCESS_NO_STD_LANE_TEST).is_err(),
                "{name} passed the process no_std leg"
            );
        }
    }

    /// The storage pack's leg is the same predicate over that crate's inputs: the real
    /// crate passes it, and the lane is bound to the storage lane's own test name, so
    /// neither the network nor the process lane's text can stand in for it.
    #[test]
    fn negative_the_storage_no_std_leg_detects_its_mutants() {
        let real = effects_storage_sources();
        assert_eq!(
            no_std_unconditional(
                &real,
                EFFECTS_STORAGE_MANIFEST,
                EFFECTS_STORAGE_NO_STD_LANE,
                STORAGE_NO_STD_LANE_TEST
            ),
            Ok(())
        );
        let with_storage_lib = |edit: &dyn Fn(&str) -> String| -> Vec<(PathBuf, String)> {
            real.iter()
                .map(|(path, text)| {
                    let text = if path.ends_with("lib.rs") {
                        edit(text)
                    } else {
                        text.clone()
                    };
                    (path.clone(), text)
                })
                .collect()
        };
        let mutants: Vec<Mutant> = vec![
            (
                "cfg_attr(no_std)",
                with_storage_lib(&|t| {
                    t.replacen("\n#![no_std]\n", "\n#![cfg_attr(not(test), no_std)]\n", 1)
                }),
                EFFECTS_STORAGE_MANIFEST.to_owned(),
                EFFECTS_STORAGE_NO_STD_LANE.to_owned(),
            ),
            (
                "extern crate std",
                with_storage_lib(&|t| {
                    t.replace(
                        "extern crate alloc;",
                        "extern crate alloc;\nextern crate std;",
                    )
                }),
                EFFECTS_STORAGE_MANIFEST.to_owned(),
                EFFECTS_STORAGE_NO_STD_LANE.to_owned(),
            ),
            (
                "a std feature",
                real.clone(),
                format!("{EFFECTS_STORAGE_MANIFEST}\n[features]\nstd = []\n"),
                EFFECTS_STORAGE_NO_STD_LANE.to_owned(),
            ),
            (
                "the lane ignored",
                real.clone(),
                EFFECTS_STORAGE_MANIFEST.to_owned(),
                EFFECTS_STORAGE_NO_STD_LANE.replace("#[test]", "#[test]\n#[ignore]"),
            ),
            (
                "the network lane in its place",
                real.clone(),
                EFFECTS_STORAGE_MANIFEST.to_owned(),
                EFFECTS_NETWORK_NO_STD_LANE.to_owned(),
            ),
            (
                "the process lane in its place",
                real.clone(),
                EFFECTS_STORAGE_MANIFEST.to_owned(),
                EFFECTS_PROCESS_NO_STD_LANE.to_owned(),
            ),
        ];
        for (name, sources, manifest, lane) in mutants {
            assert!(
                no_std_unconditional(&sources, &manifest, &lane, STORAGE_NO_STD_LANE_TEST).is_err(),
                "{name} passed the storage no_std leg"
            );
        }
    }

    /// cr-35ujnx: `#![no_std]` does not forbid an explicit `extern crate std`, and the
    /// old line-level rule missed one split across lines. Each pack's leg refuses a
    /// split, aliased `extern crate std` that reaches `s::fs`, one whose `extern` hides
    /// behind a `//` inside a string literal, and an aliased `extern crate core`.
    #[test]
    fn negative_every_pack_leg_refuses_a_split_aliased_extern_std() {
        let packs = [
            (
                effects_network_sources(),
                EFFECTS_NETWORK_MANIFEST,
                EFFECTS_NETWORK_NO_STD_LANE,
                NETWORK_NO_STD_LANE_TEST,
            ),
            (
                effects_process_sources(),
                EFFECTS_PROCESS_MANIFEST,
                EFFECTS_PROCESS_NO_STD_LANE,
                PROCESS_NO_STD_LANE_TEST,
            ),
            (
                effects_storage_sources(),
                EFFECTS_STORAGE_MANIFEST,
                EFFECTS_STORAGE_NO_STD_LANE,
                STORAGE_NO_STD_LANE_TEST,
            ),
        ];
        type Plant<'a> = (&'a str, &'a dyn Fn(&str) -> String);
        let plants: [Plant; 3] = [
            ("split, aliased extern crate std", &|t| {
                format!(
                    "{}\npub fn leak() -> bool {{\n    s::fs::metadata(\"/\").is_ok()\n}}\n",
                    t.replacen(
                        "extern crate alloc;",
                        "extern crate alloc;\nextern\ncrate std as s;",
                        1
                    )
                )
            }),
            ("a string that opens a comment", &|t| {
                format!("{t}\npub const X: &str = \"//\"; extern crate std as s;\n")
            }),
            ("aliased extern crate core", &|t| {
                t.replacen(
                    "extern crate alloc;",
                    "extern crate alloc;\nextern crate core as c;",
                    1,
                )
            }),
        ];
        for (sources, manifest, lane, lane_test) in packs {
            assert_eq!(
                no_std_unconditional(&sources, manifest, lane, lane_test),
                Ok(())
            );
            for (name, plant) in &plants {
                let doctored: Vec<(PathBuf, String)> = sources
                    .iter()
                    .map(|(path, text)| {
                        let text = if path.ends_with("lib.rs") {
                            plant(text)
                        } else {
                            text.clone()
                        };
                        (path.clone(), text)
                    })
                    .collect();
                assert!(
                    no_std_unconditional(&doctored, manifest, lane, lane_test).is_err(),
                    "{name} passed the no_std leg of {lane_test}"
                );
            }
            let blind_lane = lane.replace("with_aliased_std", "without_the_alias");
            assert!(
                no_std_unconditional(&sources, manifest, &blind_lane, lane_test).is_err(),
                "a lane that no longer witnesses the alias passed"
            );
            let cargo_blind_lane =
                lane.replace("cargo_problem(manifest_dir()", "no_problem(manifest_dir()");
            assert!(
                no_std_unconditional(&sources, manifest, &cargo_blind_lane, lane_test).is_err(),
                "a lane that no longer checks Cargo's view of the pack passed"
            );
            let env_blind_lane = lane.replace("ambient_inputs(&control", "no_inputs(&control");
            assert!(
                no_std_unconditional(&sources, manifest, &env_blind_lane, lane_test).is_err(),
                "a lane that no longer checks rustc's dep-info for ambient inputs passed"
            );
            // cr-35ujnx round 4: every ambient compile-time input fails the leg.
            for plant in [
                "pub const A: &str = env!(\"X\");",
                "pub const A: Option<&str> = option_env!(\"X\");",
                "pub const A: Option<&str> = ::core::option_env ! (\"X\");",
                "include!(\"x.rs\");",
                "pub const A: &str = include_str!(\"x\");",
                "pub const A: &[u8] = include_bytes!(\"x\");",
                "pub const A: &str = file!();",
            ] {
                let doctored: Vec<(PathBuf, String)> = sources
                    .iter()
                    .map(|(path, text)| {
                        let text = if path.ends_with("lib.rs") {
                            format!("{text}\n{plant}\n")
                        } else {
                            text.clone()
                        };
                        (path.clone(), text)
                    })
                    .collect();
                assert!(
                    no_std_unconditional(&doctored, manifest, lane, lane_test).is_err(),
                    "`{plant}` passed the no_std leg of {lane_test}"
                );
            }
        }
    }

    /// The literal-kind corpus (cr-35ujnx), one case per Rust literal kind and per way
    /// the scan could fail to read one.
    const LEXER_CASES: &str = include_str!("../../../tools/governance/rust_lexer_cases.txt");

    /// `(name, extern verdict, structure verdict, snippet)` for each corpus case.
    fn lexer_cases() -> Vec<(String, bool, bool, String)> {
        let mut out = Vec::new();
        for block in LEXER_CASES.split("\n=== ").skip(1) {
            let (head, body) = block.split_once('\n').expect("a case has a body");
            let fields: Vec<&str> = head.split_whitespace().collect();
            let [name, ext, st] = fields[..] else {
                panic!("malformed case header {head:?}");
            };
            let verdict = |field: &str, key: &str| match field.strip_prefix(key) {
                Some("pass") => true,
                Some("fail") => false,
                _ => panic!("malformed verdict {field:?}"),
            };
            out.push((
                name.to_owned(),
                verdict(ext, "extern="),
                verdict(st, "structure="),
                body.to_owned(),
            ));
        }
        out
    }

    /// cr-35ujnx round 2: every Rust literal kind — string, byte string, C string, char,
    /// byte char, raw string with and without hashes, raw byte string, raw C string —
    /// plus lifetimes, labels, raw identifiers, line, doc and nested block comments, is
    /// lexed per the Rust Reference. An `extern crate std` inside a literal or comment is
    /// text and passes the `extern` rule; one beside a literal built to fool a scan is
    /// still caught; and source the lexer cannot read (an unterminated literal or block
    /// comment, unbalanced raw hashes, an unknown prefix or escape, a string suffix,
    /// 2024's reserved `#"`) fails every rule. Each case runs through all three packs'
    /// structure rules, and each has its GOV-4-13 fixture, which runs the Python twin.
    #[test]
    fn negative_the_shared_lexer_reads_every_literal_kind_and_fails_closed() {
        let cases = lexer_cases();
        assert!(cases.len() >= 30, "the corpus lost cases");
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let fixtures = root.join("tools/governance/fixtures/obligations/gov-4-pack-kernel");
        let packs = [
            effects_network_sources(),
            effects_process_sources(),
            effects_storage_sources(),
        ];
        for (name, ext, st, snippet) in &cases {
            assert_eq!(
                super::rust_lexer::extern_problem(snippet).is_ok(),
                *ext,
                "{name}: the extern rule gave {:?}",
                super::rust_lexer::extern_problem(snippet)
            );
            for sources in &packs {
                let files: Vec<(String, String)> = sources
                    .iter()
                    .map(|(path, text)| {
                        let text = if path.ends_with("lib.rs") {
                            format!("{text}\n{snippet}")
                        } else {
                            text.clone()
                        };
                        (path.display().to_string(), text)
                    })
                    .collect();
                let verdict = super::rust_lexer::structure(
                    files.iter().map(|(n, t)| (n.as_str(), t.as_str())),
                );
                assert_eq!(
                    verdict.is_ok(),
                    *st,
                    "{name}: the structure rules gave {verdict:?}"
                );
            }
            let dir = fixtures.join(format!("gov4-13-lexer-{name}"));
            let lib = std::fs::read_to_string(
                dir.join("crates_continuum-effects-network_src_lib.rs.fixture"),
            )
            .unwrap_or_else(|_| panic!("{name} has no GOV-4-13 fixture"));
            assert!(
                lib.ends_with(snippet.as_str()),
                "{name}'s fixture drifted from the corpus"
            );
            let fixture = std::fs::read_to_string(dir.join("fixture.json")).unwrap();
            let expect = if *st {
                "\"expect\": \"clean\""
            } else {
                "\"expect\": \"violation\""
            };
            assert!(
                fixture.contains(expect),
                "{name}'s fixture expects the wrong verdict"
            );
        }
    }

    /// The Rust and Python lexers are twins (cr-35ujnx): over every corpus case and every
    /// source file of the three packs, both give the same tokens, or both refuse.
    #[test]
    fn negative_the_rust_and_python_lexers_agree_token_for_token() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let mut inputs: Vec<(String, String)> = lexer_cases()
            .into_iter()
            .map(|(name, _, _, snippet)| (name, snippet))
            .collect();
        for sources in [
            effects_network_sources(),
            effects_process_sources(),
            effects_storage_sources(),
        ] {
            for (path, text) in sources {
                inputs.push((path.display().to_string(), text));
            }
        }
        for (name, text) in inputs {
            let mut child = std::process::Command::new("python3")
                .arg(root.join("tools/governance/rust_lexer.py"))
                .arg("--tokens")
                .stdin(std::process::Stdio::piped())
                .stdout(std::process::Stdio::piped())
                .spawn()
                .expect("python3 runs");
            std::io::Write::write_all(&mut child.stdin.take().unwrap(), text.as_bytes()).unwrap();
            let output = child.wait_with_output().unwrap();
            assert!(output.status.success(), "{name}: the Python lexer crashed");
            let python = String::from_utf8(output.stdout).unwrap();
            let rust = match super::rust_lexer::lex(&text) {
                Ok(tokens) => {
                    let body: Vec<String> = tokens
                        .iter()
                        .map(|t| {
                            format!(
                                "[{:?}, {}, {}]",
                                format!("{:?}", t.kind),
                                json_string(&t.text),
                                t.line
                            )
                        })
                        .collect();
                    format!("{{\"tokens\": [{}]}}", body.join(", "))
                }
                Err(_) => "error".to_owned(),
            };
            if rust == "error" {
                assert!(
                    python.starts_with("{\"error\""),
                    "{name}: only Rust refused"
                );
            } else {
                assert_eq!(python.trim_end(), rust, "{name}: the lexers disagree");
            }
        }
    }

    /// The twins refuse the same identifiers (cr-35ujnx round 4): the ambient
    /// compile-time inputs and the structure escapes are one set in Rust and Python.
    #[test]
    fn negative_the_rust_and_python_twins_refuse_the_same_identifiers() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let output = std::process::Command::new("python3")
            .arg(root.join("tools/governance/rust_lexer.py"))
            .arg("--rules")
            .output()
            .expect("python3 runs");
        assert!(output.status.success());
        let sorted = |set: &[&str]| -> String {
            let mut v = set.to_vec();
            v.sort_unstable();
            let quoted: Vec<String> = v.iter().map(|w| format!("\"{w}\"")).collect();
            format!("[{}]", quoted.join(", "))
        };
        let rust = format!(
            "{{\"ambient\": {}, \"banned\": {}}}",
            sorted(&super::rust_lexer::AMBIENT),
            sorted(&super::rust_lexer::BANNED)
        );
        assert_eq!(String::from_utf8(output.stdout).unwrap().trim_end(), rust);
        for word in [
            "env",
            "option_env",
            "include",
            "include_str",
            "include_bytes",
        ] {
            assert!(
                super::rust_lexer::AMBIENT.contains(&word),
                "{word} is not refused as an ambient compile-time input"
            );
        }
    }

    /// A string as Python's `json.dumps` writes it (ASCII-escaped).
    fn json_string(text: &str) -> String {
        let mut out = String::from("\"");
        for c in text.chars() {
            match c {
                '"' => out.push_str("\\\""),
                '\\' => out.push_str("\\\\"),
                '\n' => out.push_str("\\n"),
                '\r' => out.push_str("\\r"),
                '\t' => out.push_str("\\t"),
                '\u{8}' => out.push_str("\\b"),
                '\u{c}' => out.push_str("\\f"),
                c if (c as u32) < 0x20 || (c as u32) > 0x7f => {
                    let mut buf = [0_u16; 2];
                    for unit in c.encode_utf16(&mut buf) {
                        out.push_str(&format!("\\u{unit:04x}"));
                    }
                }
                c => out.push(c),
            }
        }
        out.push('"');
        out
    }

    /// cr-35ujnx round 5: no manifest text scan carries the no-build-script claim.
    /// For each pack, Cargo's resolved view refuses every way to declare or find a
    /// build script or `links` value: a quoted `build` key, a dotted `package."links"`
    /// key, an inherited `build` key, an inline-table package with a `build` key, and a
    /// `build.rs` found with no key at all. The `[lints]` table is judged structurally,
    /// so a quoted `"workspace" = true` is the same table, and an added override fails.
    #[test]
    fn negative_every_manifest_form_of_a_build_script_is_refused_by_cargos_view() {
        let packs = [
            (
                effects_network_sources(),
                EFFECTS_NETWORK_MANIFEST,
                "continuum-effects-network",
            ),
            (
                effects_process_sources(),
                EFFECTS_PROCESS_MANIFEST,
                "continuum-effects-process",
            ),
            (
                effects_storage_sources(),
                EFFECTS_STORAGE_MANIFEST,
                "continuum-effects-storage",
            ),
        ];
        let script = "fn main() {}\n";
        for (sources, manifest, package) in packs {
            assert_eq!(
                super::manifest_problem(&sources, manifest),
                Ok(()),
                "{package}"
            );
            assert_eq!(
                super::manifest_problem(
                    &sources,
                    &manifest.replace("[lints]\nworkspace = true", "[lints]\n\"workspace\" = true"),
                ),
                Ok(()),
                "{package}: a quoted key is the same table"
            );
            assert!(
                manifest.contains("publish = false"),
                "{package}'s manifest changed shape"
            );
            let inline = format!(
                "package = {{ name = \"{package}\", version = {{ workspace = true }}, \
                 edition = {{ workspace = true }}, publish = false, build = \"x.rs\" }}\n\n\
                 [lints]\nworkspace = true\n"
            );
            type Variant<'a> = (&'a str, String, Vec<(&'a str, &'a str)>);
            let variants: Vec<Variant> = vec![
                (
                    "quoted build key",
                    manifest.replacen(
                        "publish = false",
                        "publish = false\n\"build\" = \"x.rs\"",
                        1,
                    ),
                    vec![("x.rs", script)],
                ),
                (
                    "single-quoted links key with a build script",
                    manifest.replacen("publish = false", "publish = false\n'links' = \"z\"", 1),
                    vec![("build.rs", script)],
                ),
                (
                    "dotted package.\"links\" key",
                    format!("package.\"links\" = \"z\"\n{manifest}"),
                    vec![],
                ),
                (
                    "inherited build key",
                    manifest.replacen(
                        "publish = false",
                        "publish = false\nbuild.workspace = true",
                        1,
                    ),
                    vec![("build.rs", script)],
                ),
                (
                    "inline-table package with a build key",
                    inline,
                    vec![("x.rs", script)],
                ),
                (
                    "build.rs found with no key",
                    manifest.to_owned(),
                    vec![("build.rs", script)],
                ),
                (
                    "lint override",
                    format!("{manifest}\n[lints.rust]\nunsafe_code = \"allow\"\n"),
                    vec![],
                ),
            ];
            for (name, doctored, extra) in variants {
                assert!(
                    super::manifest_problem_with(&sources, &doctored, &extra).is_err(),
                    "{package}: {name} passed Cargo's view"
                );
            }
        }
    }
}
