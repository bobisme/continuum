//! Acceptance re-derivation for release gate **G1-06** (bone `bn-2vbqm`).
//!
//! > artifact publication is transactional (INV-017)
//! >
//! > — `notes/plan/docs/52_RELEASE_GATES_REV3.md:29`, G1 bullet 6
//!
//! The criterion *is* the invariant, so every probe here is derived from the invariant's
//! own sentence rather than from what the implementation happens to do:
//!
//! > **INV-017 — Semantic atomicity of publication.** Artifacts become visible only after
//! > their content, provenance, and references are durably committed.
//! >
//! > — `notes/plan/plan.md` §2, line 382
//!
//! Read as a predicate, that is a three-way conjunction guarding one observable — *visible*
//! — universally quantified over **artifacts**. Three obligations follow, and this file is
//! organised around them.
//!
//! 1. **Universal over publishers.** A probe of one publisher cannot discharge a claim
//!    quantified over artifacts. §1 closes the census of every path by which any daemon
//!    operation can make an artifact visible, mechanically, against the daemon's own source.
//! 2. **The conjunction is the unit.** *Visible* must never hold while any one of content,
//!    provenance, or references does not. §2 builds a detector for that conjunction over an
//!    abstract observation, so the same detector runs against the real store *and* against
//!    hand-built torn states (§7).
//! 3. **"Visible" is a statement about readers, not about bytes.** The invariant governs
//!    what a reader can reach. It says nothing about bytes a reader cannot reach, and
//!    docs/35 chooses deliberately to leave some: a crash between the two commits "leaves
//!    unreachable content, which is garbage-collectable, and never a stale index entry […]
//!    wasted bytes are recoverable, a dangling index entry is a lie about what the store
//!    holds". [`Namespace`] therefore renders **two** fingerprints — the published namespace
//!    and the substrate underneath it — and §3 makes a different, exact claim about each.
//!
//! # What this file is, and what it deliberately is not
//!
//! It is **not** a re-run of the delivering suites.
//! `notes/plan/notes/PHASE_A_EXIT_PACKAGE.md` §7.2 grades G1-06 "evidenced — DX-13 cycle;
//! the GC/publication race was **falsified** and repaired by `bn-2siid`". That cycle lives
//! entirely in `continuum-workspace`, one crate below the daemon, and it is a *depth*
//! argument about a single primitive:
//!
//! | Delivering evidence | Grain | Instrument |
//! |---|---|---|
//! | `continuum-workspace/tests/dx13_falsification.rs` | `ReferenceStore::publish` | up to 48 real threads, `Barrier` contention, adversarial identity seams |
//! | `continuum-workspace/tests/dx13_mutation_campaign.rs` | `ReferenceStore::publish` | a hand-written `MutantStore` with four seeded defects |
//! | `continuum-workspace/tests/publication_schedule_matrix.rs` | the two-phase state machine | 1,680 enumerated single-threaded weavings; **no `StorageFaults` at all** |
//! | `continuumd/tests/g1_crash_recovery_evidence.rs` | `Daemon::dispatch` | nine dispatch boundaries, process death, then `recovery::recover` on the **durable image** |
//!
//! Each answers a question about *one* publisher — the store's own `publish`, or the one
//! daemon operation (`workspace.create`) that reaches it through `SealedWorkspace::seal`.
//! None asks the criterion's own question, which is about the daemon's whole publishing
//! surface: *for every way this daemon can make an artifact visible, is visibility gated on
//! all three conjuncts?* A daemon that grew a seventh publisher tomorrow would keep every
//! row above green.
//!
//! Five method differences carry the independence:
//!
//! 1. **A census derived from the source, not from a test author's memory.**
//!    [`the_publication_seam_census_is_closed_over_the_daemon_source`] scans every
//!    `crates/continuumd/src/**/*.rs` file — whitespace-normalised, so a call `rustfmt` split
//!    across two lines is still a call, and comment-stripped, so prose is not — for the four
//!    spellings by which control can reach the store's write surface, and requires each site
//!    to be adjudicated by the closed [`SEAMS`] table. A new publisher fails the test rather
//!    than escaping it.
//! 2. **Two total read-path fingerprints, not a targeted assertion.** [`Namespace`] reads six
//!    independent paths and renders the published namespace and the substrate separately.
//!    The master assertions are *byte equality of a whole fingerprint*, so a trace left
//!    anywhere is caught, not only one left where the test looked.
//! 3. **The whole abort taxonomy, at every phase, at every seam.** Every landed test in the
//!    tree injects exactly one of the six [`AbortReason`]s — `StorageExhausted`. §3 runs the
//!    full 3 × 6 matrix, and the 5 × 3 seam × phase matrix beside it.
//! 4. **A stroboscopic sweep of a composite publication.** Instead of the store's own
//!    two-phase seam or the dispatch boundary,
//!    [`the_visible_set_is_a_monotone_prefix_of_the_complete_publication`] stops one
//!    composite `workspace.create(seal: true)` at *every* phase check it makes — a fresh
//!    daemon per stop — and reads the whole instrument at each. Deterministic; no threads, no
//!    process death.
//! 5. **Negative controls that are assertions.** §7 hand-builds torn namespaces and requires
//!    the detector to name each tear, and runs a *mutant instrument* — the same fingerprint
//!    with one read path deleted — and requires it to miss a tear the full instrument
//!    catches. That is what makes each read path load-bearing rather than decorative.
//!
//! # By the numbers
//!
//! Every figure below is read off the daemon or the registry at run time, never transcribed.
//!
//! | | |
//! |---|---|
//! | registered operations | 75 ([`OPERATION_COUNT`]) |
//! | `@mutation` operations | 47 |
//! | served by this build | 18 |
//! | that can make an artifact visible | **6** ([`SEAMS`]) |
//! | distinct `(file, call)` store-write sites in `continuumd/src` | 5, over 4 modules |
//! | seams driven end to end | 5 |
//! | independent read paths in the instrument | 6 |
//! | seam × phase abort cells | 15 |
//! | phase × abort-reason cells | 18 |
//! | records in the composite `workspace.create(seal: true)` | 5 |
//! | phase checks the composite makes | 15 — the sweep's stop count |
//! | negative controls that must fire | 7 |
//!
//! # Verdict
//!
//! **SATISFIED-AT-NARROWER-SCOPE.**
//!
//! Across all five drivable publication seams, at every phase, and for every abort reason the
//! store's taxonomy admits, a publication that does not complete leaves the **published
//! namespace byte-identical** to the moment before the request; and no reachable state —
//! completed, failed, or stopped part-way through a composite — shows an artifact visible
//! without its content, its receipt, or its index entry. The narrowings are stated as
//! executable assertions, not as prose:
//!
//! | # | Scope statement | Test |
//! |---|---|---|
//! | 1 | **For one artifact, "nothing is published" is exact; "nothing is written" is false, by design.** An abort at `CommittingIndex` leaves GC-eligible unreachable content in the substrate — bytes no reader can reach. Aborts at `Staging` and `CommittingContent` leave nothing at all. The residue is a *step function of the phase*, is always `UnreachableContent`, and is never `MissingReferent` or `IdentityMismatch`. | [`residue_is_a_step_function_of_the_phase_and_is_never_reachable`] |
//! | 2 | **INV-017 is per artifact, not per operation, and RFC 0026 now says so.** A composite seal that fails at record *k* leaves the records before *k* published, visible, and receipted — each a complete artifact — and never its root. This file first pinned that against RFC 0026's old wire text ("Nothing is published"); bn-12plt corrected RFC 0026 to the per-artifact contract (correction 48), and the test now asserts that contract clause by clause, retry included. | [`a_partial_composite_publication_meets_rfc_0026s_per_artifact_contract`] |
//! | 3 | **The composite's own guarantee is ordering, not a transaction.** The root is published last, so a partial seal is unreachable under the name a caller would fetch it by. That rests on `WorkspaceDescriptor::records` yielding children before parents, one crate away, so the sweep measures it rather than assuming it. | [`the_visible_set_is_a_monotone_prefix_of_the_complete_publication`] |
//! | 4 | **"References" has two halves at the daemon grain and only one is durable.** The store index survives a restart; the daemon-side evidence-graph record naming a published artifact does not. A restarted daemon serves a *visible* artifact whose daemon-side reference record is gone. | [`scope_the_daemon_side_reference_record_of_a_published_artifact_is_volatile`] |
//! | 5 | **There is no concurrency at this grain to probe.** `Daemon::dispatch` takes `&mut self` and every publication completes inside one dispatch, so no two publications can interleave *within* a publication here. Interleaving is probed at the coarsest grain the surface admits — whole dispatches — and the store's own concurrency remains DX-13's. | [`interleaved_publications_at_the_dispatch_grain_never_observe_each_others_intermediates`] |
//!
//! # Absences (INV-007)
//!
//! 1. **`workspace.create_by_reference` is adjudicated but not driven.** It reaches the store
//!    only by delegating into `workspace.create`'s own code path, which §3–§6 drive directly;
//!    provisioning a held `SnapshotComponents` commitment is an out-of-band setup this file
//!    does not perform. [`SEAMS`] carries the row with `driven: false` and
//!    [`the_driven_seams_are_exactly_the_census_rows_marked_driven`] counts it, so the gap is
//!    a measurement rather than an oversight.
//! 2. **`repair.promote` — RFC 0032's "one semantically atomic step (INV-017)" — cannot be
//!    formed at this layer.** It is registered and declares `PublicationAborted`, and the
//!    `Arguments` enum carries no variant for it, so no request naming it can reach a
//!    handler. [`the_publication_shaped_operation_this_build_cannot_even_form`] asserts the
//!    refusal and the untouched namespace instead of describing the gap.
//! 3. **Budget exhaustion is not a publication failure mode at this grain.**
//!    `continuum-workspace` has no budget or quota concept; `PublicationCost` is reported
//!    telemetry, computed from the request alone and never enforced. The seam the brief names
//!    does not exist to be probed, and [`the_store_enforces_no_budget_so_there_is_no_such_seam`]
//!    records that as an assertion about the reported cost.
//! 4. **No disk.** `ReferenceStore` is in-memory — "the semantic reference, not the daemon" —
//!    so *durably committed* is probed as **committed to the substrate that survives
//!    `Daemon::crash`**, which is the strongest reading this build supports.
//! 5. **True concurrency is out of reach**, per scope statement 5.
//! 6. **Sixty-nine of the seventy-five registered operations publish nothing here**, either
//!    because they are served and write only volatile state or because this build serves no
//!    family for them. [`the_mutation_surface_is_partitioned_by_the_census`] counts the
//!    partition rather than describing it.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use continuum_engine_reference::diehard;
use continuum_intent::canonical_json::Json;
use continuum_intent::contract::IntentContract;
use continuum_value::epoch::ProtocolWindow;
use continuum_workspace::artifact_path::{ArtifactClass, ArtifactHandle};
use continuum_workspace::publication::{
    AbortReason, CapabilityToken, ContentIdentifier, IdentityUnavailable, PublicationPhase,
    StorageFaults, StoreDefect,
};
use continuum_workspace::snapshot::WorkspacePath;
use continuumd::daemon::evidence::EvidenceFamily;
use continuumd::daemon::family::{Arguments, Payload};
use continuumd::daemon::identity::{self, Blake3Identity};
use continuumd::daemon::intent::IntentFamily;
use continuumd::daemon::observe::ObserveFamily;
use continuumd::daemon::state::{IntentRecord, RegistryStatus};
use continuumd::daemon::task::TaskFamily;
use continuumd::daemon::verification::{VerificationFamily, model_source};
use continuumd::daemon::workspace::WorkspaceFamily;
use continuumd::daemon::{
    Builder, Daemon, DurableSubstrate, OperationOutcome, OperationRequest, is_mutation,
};
use continuumd::protocol::envelope::{Budget, EpochSet, RequestEnvelope};
use continuumd::protocol::handshake::{
    CapabilityDescriptor, CapabilityProfile, ClientHello, Negotiated, VersionRange, negotiate,
};
use continuumd::protocol::operations::evidence::EvidenceLinkRequest;
use continuumd::protocol::operations::intent::IntentAcceptRequest;
use continuumd::protocol::operations::observe::ObserveIngestRequest;
use continuumd::protocol::operations::verification::VerificationStartRequest;
use continuumd::protocol::operations::workspace::{WorkspaceCreateRequest, WorkspaceSealRequest};
use continuumd::protocol::registry::{ENCODINGS, OPERATION_COUNT, OPERATIONS};
use continuumd::protocol::scalar::{
    ActorId, CapabilityHandle, Commitment, EpochIdentity, EvidenceHandle, IntentHandle, Opaque,
    OperationName, ProtocolVersion, RequestId, Timestamp, WorkspaceHandle,
};
use continuumd::protocol::shared::{SnapshotComponents, SnapshotEpochs, Target};
use continuumd::protocol::spec::{Nullable, Optional};
use continuumd::protocol::vocabulary::{
    AuthorityLevel, DataGrant, Encoding, ErrorCode, Portfolio, ResultStatus, TargetKind,
};

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

/// A production trace, for the `observe.ingest` seam.
const TRACE: &str = "{\"events\":[{\"at\":0,\"op\":\"fill\"},{\"at\":1,\"op\":\"pour\"}]}\n";

/// A checker receipt, so `evidence.link` publishes different bytes than the ingest.
const CHECK_RECEIPT: &str = "{\"checker\":\"kernel-core\",\"outcome\":\"holds\"}\n";

/// The instrumentation profile the trace was captured under (plan §18.4).
const PROFILE: &str = "otel-1.0/sampled";

// =========================================================================================
// §1. The census: every way this daemon can make an artifact visible
// =========================================================================================

/// One adjudicated publication seam.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Seam {
    /// The wire operation whose handler owns the site.
    operation: &'static str,
    /// The source file, relative to `crates/continuumd/src/`.
    file: &'static str,
    /// The call spelling, as it reads after whitespace normalisation.
    call: &'static str,
    /// One record per operation, or a composite of many.
    shape: Shape,
    /// Whether a test in this file drives it end to end.
    driven: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Shape {
    /// One `stage → commit_content → commit_index` per operation.
    Single,
    /// N records, children before parents, one publication each.
    Composite,
}

/// The closed set of daemon-grain publication seams.
///
/// Derived from the source by [`the_publication_seam_census_is_closed_over_the_daemon_source`]
/// rather than transcribed from documentation. `workspace.create_by_reference` has no site of
/// its own — it delegates into `workspace.create`'s handler — so it carries that handler's
/// call as its site and `driven: false`.
const SEAMS: &[Seam] = &[
    Seam {
        operation: "workspace.create",
        file: "daemon/workspace.rs",
        call: "SealedWorkspace::seal(",
        shape: Shape::Composite,
        driven: true,
    },
    Seam {
        operation: "workspace.seal",
        file: "daemon/workspace.rs",
        call: "seal_current(",
        shape: Shape::Composite,
        driven: true,
    },
    Seam {
        operation: "verification.start",
        file: "daemon/verification.rs",
        call: "store.stage(",
        shape: Shape::Single,
        driven: true,
    },
    Seam {
        operation: "observe.ingest",
        file: "daemon/observe.rs",
        call: "store.publish(",
        shape: Shape::Single,
        driven: true,
    },
    Seam {
        operation: "evidence.link",
        file: "daemon/evidence.rs",
        call: "store.publish(",
        shape: Shape::Single,
        driven: true,
    },
    Seam {
        operation: "workspace.create_by_reference",
        file: "daemon/workspace.rs",
        call: "SealedWorkspace::seal(",
        shape: Shape::Composite,
        driven: false,
    },
];

/// The `@mutation` operations this build serves that reach no store write at all.
///
/// Each may return `PublicationAborted` under `rule errors.common`, and each writes only to
/// the volatile `DaemonState`. Naming them is what makes the census a *partition* rather than
/// a list: an operation is a publisher, a served non-publisher, or unserved, and
/// [`the_mutation_surface_is_partitioned_by_the_census`] requires the three sets to cover the
/// registry's mutations exactly once each.
const SERVED_NON_PUBLISHERS: &[&str] = &[
    "workspace.fork",
    "intent.propose_revision",
    "intent.accept",
    "intent.reject",
    "intent.lock",
    "task.cancel",
    "task.resume",
    "task.update_budget",
    "evidence.verify",
    "context.compile",
    "context.expand",
    "whiteboard.compile",
];

/// The four spellings by which control reaches the store's write surface.
///
/// `ReferenceStore` exposes exactly two write entry points — `stage` and `publish` — and
/// `continuum-workspace` exposes exactly two wrappers over them, `SealedWorkspace::seal` and
/// `staleness::seal_current`. Anything that publishes goes through one of these four, because
/// `StoreState` is private and the crate has no other public mutator.
const WRITE_CALLS: &[&str] = &[
    "store.publish(",
    "store.stage(",
    "SealedWorkspace::seal(",
    "seal_current(",
];

/// **Every publication seam the daemon's own source contains is adjudicated by [`SEAMS`].**
///
/// The census's closure argument, and the reason this file can claim to be universal over
/// publishers rather than over the publishers its author thought of. A seventh publisher
/// added to the daemon fails here, naming the file and line that added it.
///
/// That claim was checked by construction while this file was written: a compiling
/// `store.publish(…)` call was temporarily added to `daemon/output.rs` — the one module that
/// mentions INV-017 in prose and publishes nothing — and this test failed with
/// `daemon/output.rs:144 — store.publish(`, while
/// [`control_the_census_scanner_reads_control_flow_and_not_prose`] failed with the module set
/// grown by one. The injection was reverted; what remains is the check that caught it.
#[test]
fn the_publication_seam_census_is_closed_over_the_daemon_source() {
    let sites = write_call_sites();
    assert!(
        !sites.is_empty(),
        "the scanner found no store writes at all, which would make every other test in \
         this file vacuous"
    );

    let adjudicated: BTreeSet<(&str, &str)> =
        SEAMS.iter().map(|seam| (seam.file, seam.call)).collect();

    let unadjudicated: Vec<String> = sites
        .iter()
        .filter(|(file, _, call)| !adjudicated.contains(&(file.as_str(), call.as_str())))
        .map(|(file, line, call)| format!("{file}:{line} — {call}"))
        .collect();
    assert!(
        unadjudicated.is_empty(),
        "the daemon contains publication seams this file does not adjudicate; add them to \
         `SEAMS` and probe them, or state the absence:\n{}",
        unadjudicated.join("\n")
    );

    // The other direction: a `SEAMS` row whose site was deleted would make the table a
    // description of a past daemon.
    let found: BTreeSet<(&str, &str)> = sites
        .iter()
        .map(|(file, _, call)| (file.as_str(), call.as_str()))
        .collect();
    for seam in SEAMS {
        assert!(
            found.contains(&(seam.file, seam.call)),
            "`SEAMS` names {}:{} but the source has no such call site",
            seam.file,
            seam.call
        );
    }
    assert_eq!(
        found.len(),
        5,
        "five distinct (file, call) sites reach the store's write surface; `SEAMS` has six \
         rows because `workspace.create` and `workspace.create_by_reference` share one"
    );
    assert_eq!(
        SEAMS
            .iter()
            .filter(|seam| seam.shape == Shape::Composite)
            .count(),
        3,
        "three of the six seams publish a composite"
    );
}

/// The registry's `@mutation` surface is partitioned exactly once by the census.
///
/// Every registry mutation is a publisher ([`SEAMS`]), a served non-publisher
/// ([`SERVED_NON_PUBLISHERS`]), or unserved by this build. The three sets are disjoint and
/// cover it. An operation falling out of all three would be one this file made no statement
/// about, which is the failure INV-007 names.
#[test]
fn the_mutation_surface_is_partitioned_by_the_census() {
    assert_eq!(
        OPERATIONS.len(),
        OPERATION_COUNT,
        "the registry's own count and its table agree"
    );

    let publishers: BTreeSet<&str> = SEAMS.iter().map(|seam| seam.operation).collect();
    let non_publishers: BTreeSet<&str> = SERVED_NON_PUBLISHERS.iter().copied().collect();
    for name in &publishers {
        assert!(
            !non_publishers.contains(name),
            "`{name}` is in both halves of the census"
        );
        assert!(
            is_mutation(name),
            "`{name}` publishes, so the registry must call it a `@mutation`"
        );
    }
    for name in &non_publishers {
        assert!(
            is_mutation(name),
            "`{name}` is in the served-mutation half, so it must be a `@mutation`"
        );
    }

    let mutations: BTreeSet<&str> = OPERATIONS
        .iter()
        .map(|spec| spec.name)
        .filter(|name| is_mutation(name))
        .collect();
    assert_eq!(
        mutations.len(),
        47,
        "the registry's `@mutation` count, read from the registry and not transcribed"
    );

    let classified: BTreeSet<&str> = publishers.union(&non_publishers).copied().collect();
    let unserved: BTreeSet<&str> = mutations.difference(&classified).copied().collect();
    assert_eq!(
        classified.len() + unserved.len(),
        mutations.len(),
        "the three sets partition the mutation surface"
    );
    assert_eq!(
        publishers.len() + non_publishers.len(),
        18,
        "eighteen of the forty-seven mutations are served by this build"
    );
    assert_eq!(
        publishers.len(),
        6,
        "six of those eighteen can make an artifact visible"
    );
    assert!(
        unserved.contains("repair.promote"),
        "RFC 0032's `repair.promote` is in the unserved remainder"
    );
    assert_eq!(
        OPERATION_COUNT - publishers.len(),
        69,
        "sixty-nine of the seventy-five registered operations publish nothing here"
    );
}

/// **The publication-shaped operation this build cannot even form.**
///
/// RFC 0032 makes `repair.promote` execute "as one semantically atomic step (INV-017)". It is
/// in the registry, it declares `PublicationAborted`, and the `Arguments` enum carries no
/// variant for it — so a request naming it is refused by the dispatcher's shape check before
/// any handler runs, whatever body it carries. The absence is measured here rather than
/// described: the refusal is typed, and both fingerprints are byte-identical after it.
#[test]
fn the_publication_shaped_operation_this_build_cannot_even_form() {
    let spec = OPERATIONS
        .iter()
        .find(|spec| spec.name == "repair.promote")
        .expect("`repair.promote` is in the registry");
    assert!(
        spec.errors.contains(&ErrorCode::PublicationAborted),
        "`repair.promote` declares the code INV-017 owns"
    );

    let mut rig = Rig::new();
    let before = rig.namespace(&[]);
    let outcome = rig.daemon.dispatch(&OperationRequest {
        envelope: keyed(
            envelope(
                "repair.promote",
                "service:continuumd",
                "cap_root",
                "req_promote",
            ),
            "idem-promote",
        ),
        arguments: Arguments::WorkspaceSeal(WorkspaceSealRequest {
            snapshot: WorkspaceHandle::new("ws_unreachable").expect("a well-formed handle"),
        }),
    });
    assert_eq!(outcome.envelope.status, ResultStatus::Error);
    assert_eq!(
        outcome.error_code(),
        Some(ErrorCode::MalformedRequest),
        "no body can carry this operation, so the shape check refuses before a handler runs"
    );
    let after = rig.namespace(&[]);
    assert_eq!(
        after.render(),
        before.render(),
        "and the refusal leaves the published namespace byte-identical"
    );
    assert_eq!(
        after.render_substrate(),
        before.render_substrate(),
        "and writes nothing to the substrate either"
    );
}

/// The store enforces no budget, so budget exhaustion is not a publication failure seam.
///
/// The brief names "budget exhaustion" as a candidate failure point. It is not one at this
/// grain: `continuum-workspace` has no budget or quota concept, and `PublicationCost` is
/// reported telemetry computed from the request alone — deliberately, because a cost that
/// varied with what the store already held would be an existence oracle. Recorded as an
/// assertion so the absence cannot rot into an assumption.
#[test]
fn the_store_enforces_no_budget_so_there_is_no_such_seam() {
    let mut rig = Rig::new();
    let root = rig.seal_workspace("req_create", "idem-create");
    let token = Rig::operator();
    let handle = identity::workspace_to_store(&root).expect("a `ws_` handle crosses to the store");
    let view = rig
        .daemon
        .store()
        .audit_view(&token)
        .expect("`cap_root` confers audit");
    let receipts = view.receipts(&handle);
    assert_eq!(receipts.len(), 1, "the composite's root has one receipt");
    let cost = receipts[0].cost();
    assert_eq!(
        cost.index_entries(),
        1,
        "a publication accounts for exactly one index entry, always"
    );
    let bytes = rig
        .daemon
        .store()
        .read(&handle, &token)
        .expect("`cap_root` confers read");
    assert_eq!(
        cost.content_bytes(),
        bytes.len() as u64,
        "the reported cost is a pure function of the request's own bytes — nothing is \
         charged against a ceiling, because there is no ceiling"
    );
}

/// Every site under `crates/continuumd/src/` that calls the store's write surface.
///
/// Returns `(file, line, call)`. Two normalisations make the scan a statement about control
/// flow rather than about formatting:
///
/// - **Comment lines are dropped.** A doc comment naming `store.publish` is prose and cannot
///   publish anything; `daemon/output.rs` contains exactly such a mention of INV-017.
/// - **Whitespace is collapsed.** `rustfmt` writes every one of these calls across two lines
///   (`store\n    .publish(…)`), so a line-wise scan finds none of them. The scanner builds a
///   whitespace-free image of each file with a line number per character, and maps every
///   match back to the line its receiver sits on.
fn write_call_sites() -> Vec<(String, usize, String)> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut sites = Vec::new();
    let mut stack = vec![root.clone()];
    while let Some(directory) = stack.pop() {
        let entries = std::fs::read_dir(&directory).expect("the daemon's own source is readable");
        for entry in entries {
            let path = entry.expect("a readable directory entry").path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            if path.extension().and_then(std::ffi::OsStr::to_str) != Some("rs") {
                continue;
            }
            let relative = path
                .strip_prefix(&root)
                .expect("every visited path is under `src`")
                .to_string_lossy()
                .replace('\\', "/");
            let text = std::fs::read_to_string(&path).expect("a readable source file");

            let mut image = String::new();
            let mut lines = Vec::new();
            for (offset, line) in text.lines().enumerate() {
                if line.trim_start().starts_with("//") {
                    continue;
                }
                for character in line.chars().filter(|character| !character.is_whitespace()) {
                    image.push(character);
                    lines.push(offset + 1);
                }
            }

            for call in WRITE_CALLS {
                let mut from = 0;
                while let Some(found) = image[from..].find(call) {
                    let at = from + found;
                    sites.push((relative.clone(), lines[at], (*call).to_owned()));
                    from = at + 1;
                }
            }
        }
    }
    sites.sort();
    sites.dedup();
    sites
}

// =========================================================================================
// §2. The instrument: the published namespace, and INV-017's conjunction over it
// =========================================================================================

/// INV-017's three conjuncts as sets, decoupled from any store.
///
/// The detector operates on this type and not on a [`Daemon`], which is what lets §7 feed it
/// hand-built torn states. [`Namespace`] is the reading of a live daemon into it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct Observation {
    /// **References.** The index names this identity: `StoreAudit::identities`.
    indexed: BTreeSet<ArtifactHandle>,
    /// **Content.** A capability-holding reader gets bytes back: `ReferenceStore::read`.
    readable: BTreeSet<ArtifactHandle>,
    /// **Provenance.** The append-only ledger holds at least one receipt for it.
    receipted: BTreeSet<ArtifactHandle>,
}

/// One way an observation can contradict INV-017.
///
/// The four are the four edges of the conjunction, each named for the clause it breaks rather
/// than for the data structure it was found in.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
enum Tear {
    /// Indexed with no content behind it — docs/35's `missing referent`, "a lie about what
    /// the store holds".
    ReferenceWithoutContent(ArtifactHandle),
    /// Indexed and readable with no receipt — a visible artifact nobody is accountable for.
    /// INV-017's *provenance* conjunct.
    VisibleWithoutProvenance(ArtifactHandle),
    /// A receipt naming an artifact no reader can reach. RFC 0026 requires a receipt to name a
    /// readable artifact; DX-13 falsified exactly this once, under concurrent GC.
    ProvenanceWithoutVisibility(ArtifactHandle),
    /// Bytes reachable through the read path that the index does not name — visibility
    /// without a reference.
    ContentWithoutReference(ArtifactHandle),
}

impl Observation {
    /// Every way this observation contradicts INV-017, in a deterministic order.
    fn tears(&self) -> Vec<Tear> {
        let mut tears = Vec::new();
        for handle in &self.indexed {
            if self.readable.contains(handle) {
                if !self.receipted.contains(handle) {
                    tears.push(Tear::VisibleWithoutProvenance(handle.clone()));
                }
            } else {
                tears.push(Tear::ReferenceWithoutContent(handle.clone()));
            }
        }
        for handle in &self.receipted {
            if !self.readable.contains(handle) {
                tears.push(Tear::ProvenanceWithoutVisibility(handle.clone()));
            }
        }
        for handle in &self.readable {
            if !self.indexed.contains(handle) {
                tears.push(Tear::ContentWithoutReference(handle.clone()));
            }
        }
        tears.sort();
        tears.dedup();
        tears
    }

    /// The artifacts this observation calls visible: indexed *and* readable.
    fn visible(&self) -> BTreeSet<ArtifactHandle> {
        self.indexed.intersection(&self.readable).cloned().collect()
    }
}

/// One daemon's store, read through six independent paths.
///
/// | # | Path | Layer | What it answers |
/// |---|---|---|---|
/// | 1 | `StoreAudit::identities` | published | which identities the index names |
/// | 2 | `StoreAudit::published_count` | published | how many, counted by the store itself |
/// | 3 | `ReferenceStore::read` | published | which identities a reader can fetch bytes for |
/// | 4 | `StoreAudit::receipts` | published | how many receipts each identity carries |
/// | 5 | `StoreAudit::storage_attribution` | substrate | bytes of committed content per class |
/// | 6 | `StoreAudit::fsck` | substrate | the index verifier's defect classification |
///
/// The split into two layers is not a convenience. INV-017 governs *visibility*, and paths
/// 1–4 are the whole of what a reader can observe. Paths 5 and 6 see bytes no reader can
/// reach — precisely where docs/35 puts the residue of an interrupted publication,
/// deliberately. A single fingerprint over all six would make "a failed publication changes
/// nothing" false by design, and would tempt a test author to narrow the instrument until it
/// passed. Two fingerprints let each layer carry its own exact claim (§3).
///
/// The **authorization audit log** is in neither: a record is written for a refused
/// publication too, and the invariant is about what a reader can see, not about what the
/// daemon wrote down. [`a_refused_publication_is_admission_audited_and_publishes_nothing`]
/// asserts that separation rather than assuming it.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Namespace {
    observation: Observation,
    published_count: usize,
    receipts: BTreeMap<ArtifactHandle, usize>,
    attribution: BTreeMap<ArtifactClass, u64>,
    defects: Vec<StoreDefect>,
}

impl Namespace {
    /// Read one daemon's store.
    ///
    /// `universe` is the set of identities to probe the *read* path with beyond those the
    /// index already names. It is how this instrument answers "is `h` visible?" for an `h`
    /// that has not been published yet — the question INV-017's "only after" needs, and one a
    /// listing alone cannot answer.
    fn of(daemon: &Daemon, universe: &[ArtifactHandle]) -> Self {
        let token = Rig::operator();
        let store = daemon.store();
        let view = store
            .audit_view(&token)
            .expect("`cap_root` confers audit authority");

        let indexed: BTreeSet<ArtifactHandle> = view.identities().into_iter().collect();
        let mut probe: BTreeSet<ArtifactHandle> = indexed.clone();
        probe.extend(universe.iter().cloned());

        let mut readable = BTreeSet::new();
        let mut receipts = BTreeMap::new();
        let mut receipted = BTreeSet::new();
        for handle in &probe {
            if store.read(handle, &token).is_ok() {
                readable.insert(handle.clone());
            }
            let count = view.receipts(handle).len();
            receipts.insert(handle.clone(), count);
            if count > 0 {
                receipted.insert(handle.clone());
            }
        }

        Self {
            observation: Observation {
                indexed,
                readable,
                receipted,
            },
            published_count: view.published_count(),
            receipts,
            attribution: view.storage_attribution(),
            defects: view.fsck(),
        }
    }

    /// The **published namespace**: paths 1–4. Byte equality of two renderings is the master
    /// assertion for "this failed publication left nothing a reader can see".
    fn render(&self) -> String {
        let mut out = String::new();
        out.push_str(&format!("published_count {}\n", self.published_count));
        for handle in &self.observation.indexed {
            out.push_str(&format!("indexed {handle}\n"));
        }
        for handle in &self.observation.readable {
            out.push_str(&format!("readable {handle}\n"));
        }
        for (handle, count) in &self.receipts {
            if *count > 0 {
                out.push_str(&format!("receipts {handle} {count}\n"));
            }
        }
        out
    }

    /// The **substrate**: paths 5–6. Bytes and defects — what an operator sees and a reader
    /// does not.
    fn render_substrate(&self) -> String {
        let mut out = String::new();
        for (class, bytes) in &self.attribution {
            out.push_str(&format!("bytes {} {bytes}\n", class.token()));
        }
        for defect in &self.defects {
            out.push_str(&format!("defect {defect}\n"));
        }
        out
    }

    /// The published namespace with the provenance path removed — the mutant instrument of §7.
    fn render_without_provenance(&self) -> String {
        self.render()
            .lines()
            .filter(|line| !line.starts_with("receipts "))
            .map(|line| format!("{line}\n"))
            .collect()
    }

    /// The artifacts a reader can reach.
    fn visible(&self) -> BTreeSet<ArtifactHandle> {
        self.observation.visible()
    }

    /// The identities the index verifier calls unreachable content: GC-eligible residue.
    fn residue(&self) -> Vec<ArtifactHandle> {
        self.defects
            .iter()
            .filter_map(|defect| match defect {
                StoreDefect::UnreachableContent(handle) => Some(handle.clone()),
                _ => None,
            })
            .collect()
    }

    /// Defects that are *not* residue: a stale index entry or corrupted content. docs/35
    /// forbids the store's own ordering from ever producing either.
    fn hard_defects(&self) -> Vec<&StoreDefect> {
        self.defects
            .iter()
            .filter(|defect| !matches!(defect, StoreDefect::UnreachableContent(_)))
            .collect()
    }

    /// The reconciliation across three independent read paths.
    ///
    /// `published_count` is the store's own count of the index; `indexed` is its own listing
    /// of it; `visible` is what the read path serves; `receipted` is what the ledger accounts
    /// for. The four must be one number, and no defect may be anything but residue.
    fn reconcile(&self) -> Result<usize, String> {
        let indexed = self.observation.indexed.len();
        let visible = self.visible().len();
        let receipted = self.observation.receipted.len();
        if self.published_count != indexed {
            return Err(format!(
                "published_count {} != |indexed| {indexed}",
                self.published_count
            ));
        }
        if indexed != visible {
            return Err(format!("|indexed| {indexed} != |visible| {visible}"));
        }
        if visible != receipted {
            return Err(format!("|visible| {visible} != |receipted| {receipted}"));
        }
        if let Some(defect) = self.hard_defects().first() {
            return Err(format!("fsck reports a non-residue defect: {defect}"));
        }
        Ok(visible)
    }
}

// =========================================================================================
// §3. All-or-nothing, at every seam, phase, and abort reason
// =========================================================================================

/// The seams this file drives end to end.
///
/// Checked against [`SEAMS`] by [`the_driven_seams_are_exactly_the_census_rows_marked_driven`],
/// so a census row added without a driver fails rather than silently going unprobed.
const DRIVEN: &[&str] = &[
    "workspace.create",
    "workspace.seal",
    "verification.start",
    "observe.ingest",
    "evidence.link",
];

/// The drivers cover exactly the census rows marked `driven`.
#[test]
fn the_driven_seams_are_exactly_the_census_rows_marked_driven() {
    let driven: BTreeSet<&str> = DRIVEN.iter().copied().collect();
    let claimed: BTreeSet<&str> = SEAMS
        .iter()
        .filter(|seam| seam.driven)
        .map(|seam| seam.operation)
        .collect();
    assert_eq!(driven, claimed);
    assert_eq!(
        driven.len(),
        5,
        "five of the six census rows are driven end to end"
    );
    assert_eq!(
        SEAMS.iter().filter(|seam| !seam.driven).count(),
        1,
        "and the one that is not is stated as an absence in this file's module doc"
    );
}

/// **The anti-vacuity floor.** Each driven seam publishes when nothing is injured.
///
/// A seam that published nothing would make every "the namespace did not change" assertion
/// below trivially true.
#[test]
fn control_every_driven_seam_publishes_when_nothing_is_injured() {
    for seam in DRIVEN {
        let mut rig = Rig::new();
        rig.prepare(seam);
        let before = rig.namespace(&[]);
        let outcome = rig.drive(seam);
        assert!(
            succeeded(&outcome),
            "`{seam}` succeeds on a healthy daemon: {:?}",
            outcome.envelope.error
        );
        let after = rig.namespace(&[]);
        assert!(
            after.published_count > before.published_count,
            "`{seam}` published nothing, so every failure assertion about it would be vacuous"
        );
        assert!(
            after.observation.tears().is_empty(),
            "`{seam}`'s completed publication is torn: {:?}",
            after.observation.tears()
        );
        assert!(
            after.hard_defects().is_empty(),
            "`{seam}` left a defect that is not residue: {:?}",
            after.hard_defects()
        );
    }
}

/// **The master assertion.** A publication that aborts at its first phase check leaves the
/// published namespace byte-identical to the moment before the request.
///
/// Five seams × three phases. The precondition each seam needs is built *before* the fault is
/// armed, so the abort under test is the seam's own and never a missing input, and the fault
/// counts from the arming — check 1 is the first check this operation makes.
///
/// The comparison is the whole published fingerprint — four read paths — not a count and not
/// a membership test, so a trace left anywhere a reader can look is a failure here.
#[test]
fn a_publication_aborted_at_its_first_phase_leaves_the_published_namespace_byte_identical() {
    let mut cells = 0_usize;
    for seam in DRIVEN {
        for phase in PHASES {
            let mut rig = Rig::new();
            rig.prepare(seam);
            let before = rig.namespace(&[]);

            rig.arm(Plan::Nth {
                phase,
                nth: 1,
                reason: AbortReason::StorageExhausted,
            });
            let outcome = rig.drive(seam);
            assert_eq!(
                outcome.envelope.status,
                ResultStatus::Error,
                "`{seam}` must fail when its first {phase} check refuses"
            );
            assert_eq!(
                outcome.error_code(),
                Some(ErrorCode::PublicationAborted),
                "`{seam}` at {phase} must refuse with the code INV-017 owns"
            );

            let after = rig.namespace(&[]);
            assert_eq!(
                after.render(),
                before.render(),
                "`{seam}` aborted at {phase} and left a trace a reader can see"
            );
            assert!(
                after.observation.tears().is_empty(),
                "`{seam}` aborted at {phase} and left a torn namespace: {:?}",
                after.observation.tears()
            );
            assert!(
                after.hard_defects().is_empty(),
                "`{seam}` aborted at {phase} and left a defect that is not residue: {:?}",
                after.hard_defects()
            );
            cells += 1;
        }
    }
    assert_eq!(cells, 15, "five seams by three phases");
}

/// **Scope statement 1.** For the artifact whose publication aborted, "nothing is published"
/// is exact. "Nothing is written" is false, by design, and the difference is a step function
/// of the phase. Every drive here aborts at the first check of its kind, so the aborted
/// artifact is the operation's first record; scope statement 2 covers a later one.
///
/// docs/35 chooses the asymmetry in its own words: a publication interrupted after its
/// content commit leaves "unreachable content, which is garbage-collectable, and never a
/// stale index entry […] wasted bytes are recoverable, a dangling index entry is a lie about
/// what the store holds."
///
/// This measures the choice at the daemon grain, for every seam, in both directions:
///
/// - an abort at `Staging` or `CommittingContent` leaves the **substrate** byte-identical
///   too — nothing at all is written;
/// - an abort at `CommittingIndex` leaves residue, and every residue item is
///   `UnreachableContent`: unindexed, unreadable, and unreceipted;
/// - in no case is a `MissingReferent` or an `IdentityMismatch` produced.
///
/// The second bullet's three negatives are what make the residue more than bookkeeping: it is
/// invisible through every read path INV-017 is about.
#[test]
fn residue_is_a_step_function_of_the_phase_and_is_never_reachable() {
    let token = Rig::operator();
    for seam in DRIVEN {
        for phase in PHASES {
            let mut rig = Rig::new();
            rig.prepare(seam);
            let before = rig.namespace(&[]);

            rig.arm(Plan::Nth {
                phase,
                nth: 1,
                reason: AbortReason::StorageExhausted,
            });
            assert_eq!(rig.drive(seam).envelope.status, ResultStatus::Error);
            let after = rig.namespace(&[]);

            let earlier = before.residue();
            let fresh: Vec<ArtifactHandle> = after
                .residue()
                .into_iter()
                .filter(|handle| !earlier.contains(handle))
                .collect();

            match phase {
                PublicationPhase::Staging | PublicationPhase::CommittingContent => {
                    assert_eq!(
                        after.render_substrate(),
                        before.render_substrate(),
                        "`{seam}` aborted at {phase} and wrote to the substrate; nothing is \
                         durable before the content commit"
                    );
                    assert!(fresh.is_empty());
                }
                PublicationPhase::CommittingIndex => {
                    assert_eq!(
                        fresh.len(),
                        1,
                        "`{seam}` aborted at {phase}: exactly one record's content was \
                         already durable, and it is now residue"
                    );
                }
            }

            for residue in &fresh {
                assert!(
                    !after.observation.indexed.contains(residue),
                    "residue {residue} is named by the index"
                );
                assert!(
                    rig.daemon.store().read(residue, &token).is_err(),
                    "residue {residue} is readable, so it is not residue — it is a \
                     publication nobody accounted for"
                );
                assert_eq!(
                    after.receipts.get(residue).copied().unwrap_or(0),
                    0,
                    "residue {residue} carries a receipt"
                );
            }
            assert!(
                after.hard_defects().is_empty(),
                "`{seam}` at {phase} produced a defect the store's own ordering forbids: {:?}",
                after.hard_defects()
            );
        }
    }
}

/// **The full abort taxonomy, at every phase.** Eighteen cells, none visible to a reader.
///
/// `AbortReason` has six variants and every landed test in this tree injects exactly one of
/// them, `StorageExhausted`. The store's contract is not "`StorageExhausted` aborts
/// atomically", it is "a publication that cannot complete aborts atomically", so the reason
/// must not matter. This runs the whole 3 × 6 matrix against a single-record seam, where a
/// first-check refusal is the whole operation's refusal and the byte-equality claim is exact.
#[test]
fn every_abort_reason_at_every_phase_leaves_the_published_namespace_byte_identical() {
    const REASONS: [AbortReason; 6] = [
        AbortReason::StorageExhausted,
        AbortReason::IdentityUnavailable,
        AbortReason::IdentityClassMismatch,
        AbortReason::NotContentAddressed,
        AbortReason::IdentityCollision,
        AbortReason::Abandoned,
    ];
    let mut cells = 0_usize;
    for phase in PHASES {
        for reason in REASONS {
            let mut rig = Rig::new();
            rig.prepare("observe.ingest");
            let before = rig.namespace(&[]);

            rig.arm(Plan::Nth {
                phase,
                nth: 1,
                reason,
            });
            let outcome = rig.drive("observe.ingest");
            assert_eq!(
                outcome.envelope.status,
                ResultStatus::Error,
                "{reason} at {phase} must abort the publication"
            );
            assert_eq!(
                outcome.error_code(),
                Some(ErrorCode::PublicationAborted),
                "every abort reason maps to the one wire code INV-017 owns"
            );

            let after = rig.namespace(&[]);
            assert_eq!(
                after.render(),
                before.render(),
                "{reason} at {phase} left a trace a reader can see"
            );
            assert!(after.observation.tears().is_empty());
            assert!(after.hard_defects().is_empty());

            // The abort *is* recorded — in the store's abort trail, which is not the
            // published namespace. INV-017 governs what a reader can see, and an abort record
            // is not an artifact. Asserting both halves is what keeps the fingerprint honest:
            // it is silent about aborts by construction, not by accident.
            let view = rig
                .daemon
                .store()
                .audit_view(&Rig::operator())
                .expect("`cap_root` confers audit");
            assert!(
                view.aborts()
                    .iter()
                    .any(|abort| abort.reason() == reason && abort.phase() == phase),
                "the store records {reason} at {phase} in its abort trail"
            );
            cells += 1;
        }
    }
    assert_eq!(cells, 18, "three phases by six abort reasons");
}

/// A publication refused by *authorization* rather than by storage also leaves nothing.
///
/// A different failure point with a different answer: the store never reaches its own state
/// machine, and the wire code is `CapabilityDenied`, not `PublicationAborted`. Driven for
/// every seam under a `read`-level capability, with the preconditions built first so what is
/// refused is the publication and not its inputs.
#[test]
fn a_publication_refused_by_authorization_leaves_the_published_namespace_byte_identical() {
    for seam in DRIVEN {
        let mut rig = Rig::new();
        rig.prepare(seam);
        let before = rig.namespace(&[]);
        rig.demote();
        let outcome = rig.drive(seam);
        assert_eq!(
            outcome.envelope.status,
            ResultStatus::Error,
            "`{seam}` under a read-only capability must be refused"
        );
        assert_eq!(
            outcome.error_code(),
            Some(ErrorCode::CapabilityDenied),
            "`{seam}`'s refusal names the capability, not the storage"
        );
        let after = rig.namespace(&[]);
        assert_eq!(
            after.render(),
            before.render(),
            "`{seam}` was refused and still left a trace"
        );
        assert_eq!(
            after.render_substrate(),
            before.render_substrate(),
            "`{seam}` was refused before the store's state machine, so not even residue"
        );
    }
}

/// The refusal is audited, the published namespace is untouched, and both are true at once.
///
/// If the fingerprint read an audit log, §3's byte-equality assertions would be false for the
/// wrong reason, and a test author could "fix" them by narrowing the instrument. This pins the
/// separation through an audit path the fingerprint deliberately does not read: the daemon's
/// own admission record, written before any store call.
#[test]
fn a_refused_publication_is_admission_audited_and_publishes_nothing() {
    let mut rig = Rig::new();
    rig.prepare("observe.ingest");
    let before = rig.namespace(&[]);
    let audited_before = rig.daemon.state().admissions().len();

    rig.demote();
    let outcome = rig.drive("observe.ingest");
    assert_eq!(outcome.error_code(), Some(ErrorCode::CapabilityDenied));

    let admissions = rig.daemon.state().admissions();
    assert!(
        admissions.len() > audited_before,
        "the daemon recorded the admission decision"
    );
    assert!(
        admissions.last().is_some_and(|record| !record.admitted),
        "and recorded it as a denial"
    );
    assert_eq!(
        rig.namespace(&[]).render(),
        before.render(),
        "and published nothing"
    );
}

// =========================================================================================
// §4. The stroboscopic sweep: what is visible at every phase of one composite publication
// =========================================================================================

/// **The visible set is a monotone prefix of the complete publication, and it grows only at
/// an index commit.**
///
/// One `workspace.create(seal: true)` is a composite: N records, children before parents,
/// three phase checks each. This stops the *same* publication at every one of those checks —
/// a fresh daemon per stop, so a stop reproduces from its index alone — and reads the whole
/// instrument at each. Four claims come out of it:
///
/// 1. **No intermediate is torn.** At every stop, INV-017's conjunction holds over every read
///    path. This is the invariant's own sentence evaluated at every point of a publication,
///    which is the strongest form the daemon grain admits.
/// 2. **Monotone.** `visible(k) ⊆ visible(k+1)`. A stop never un-publishes.
/// 3. **A prefix.** Everything visible at stop `k` is something the healthy run publishes.
///    Nothing appears that a complete publication would not produce.
/// 4. **A step function of the index commit.** The visible set grows across stop `k` only
///    when check `k − 1` was a `CommittingIndex` check. Content commits make nothing visible,
///    which is INV-017's "only after … references are durably committed" measured rather than
///    asserted.
///
/// The check count and the phase order are read off the daemon through the injector's own
/// timeline, not written down here, so the sweep cannot drift from the publication it sweeps.
#[test]
fn the_visible_set_is_a_monotone_prefix_of_the_complete_publication() {
    // The healthy run first: what a complete publication produces, and the phase sequence it
    // walks.
    let mut healthy = Rig::new();
    healthy.faults.disarm();
    let root = healthy.seal_workspace("req_create", "idem-create");
    let complete = healthy.namespace(&[]);
    let timeline = healthy.faults.trace();
    let checks = timeline.len();

    assert!(
        checks >= 9,
        "a composite of at least three records makes at least nine phase checks; saw {checks}"
    );
    assert_eq!(
        checks % 3,
        0,
        "every record makes exactly three checks, so the total is a multiple of three"
    );
    let records = checks / 3;
    assert_eq!(
        complete.published_count, records,
        "the healthy run published one artifact per record"
    );
    for (offset, phase) in timeline.iter().enumerate() {
        assert_eq!(
            *phase,
            PHASES[offset % 3],
            "check {offset} is out of the store's own staging → content → index order"
        );
    }

    let root_handle =
        identity::workspace_to_store(&root).expect("a `ws_` handle crosses to the store");
    let universe: Vec<ArtifactHandle> = complete
        .observation
        .indexed
        .iter()
        .cloned()
        .chain(std::iter::once(root_handle.clone()))
        .collect();

    let mut previous: BTreeSet<ArtifactHandle> = BTreeSet::new();
    let mut growth_points = Vec::new();
    for stop in 1..=checks {
        let mut rig = Rig::new();
        rig.arm(Plan::Call {
            nth: stop as u64,
            reason: AbortReason::StorageExhausted,
        });
        let outcome = rig.drive("workspace.create");
        assert_eq!(
            outcome.envelope.status,
            ResultStatus::Error,
            "stop {stop} of {checks} must abort the publication"
        );
        let namespace = rig.namespace(&universe);
        let visible = namespace.visible();

        assert!(
            namespace.observation.tears().is_empty(),
            "stop {stop}: an intermediate state of a publication is torn: {:?}",
            namespace.observation.tears()
        );
        assert!(
            previous.is_subset(&visible),
            "stop {stop}: the visible set shrank"
        );
        assert!(
            visible.is_subset(&complete.observation.indexed),
            "stop {stop}: something is visible that a complete publication never publishes"
        );
        assert!(
            !visible.contains(&root_handle),
            "stop {stop}: the composite's own root is visible before the publication finished"
        );
        assert_eq!(
            namespace.published_count,
            visible.len(),
            "stop {stop}: the index count and the readable set disagree"
        );
        assert!(
            namespace.hard_defects().is_empty(),
            "stop {stop}: a defect the store's own ordering forbids"
        );

        if visible.len() > previous.len() {
            growth_points.push(stop);
        }
        previous = visible;
    }

    // Claim 4 as arithmetic rather than as a story. A stop at check `k` runs checks 1..k−1 to
    // completion, so the visible set at stop `k` holds one record per completed index commit
    // among those. Growth therefore happens exactly at the stops just past an index commit —
    // check indices ≡ 1 (mod 3) — and there are `records − 1` of them in range, because the
    // root's own index commit is the last check and no stop lies past it.
    assert_eq!(
        growth_points.len(),
        records - 1,
        "the visible set grew once per record a stop can leave complete: {growth_points:?}"
    );
    for stop in &growth_points {
        assert_eq!(
            stop % 3,
            1,
            "growth was observed at stop {stop}, which is not just past a `CommittingIndex` \
             check; a content commit made something visible"
        );
        assert_eq!(
            timeline[stop - 2],
            PublicationPhase::CommittingIndex,
            "and the check before stop {stop} was indeed an index commit"
        );
    }
    assert_eq!(
        previous.len(),
        records - 1,
        "the last stop leaves every record but the root's own publication complete"
    );
}

/// **Scope statement 2.** A composite publication is atomic per record, not per operation —
/// and that is now RFC 0026's own contract, which this test holds the daemon to.
///
/// INV-017's own sentence quantifies over artifacts, and RFC 0038 restates it as "publication
/// is per-artifact atomic". RFC 0026's wire text for `PublicationAborted` used to read "Nothing
/// is published and nothing is truncated", which a composite does not deliver at operation
/// grain: a `workspace.create(seal: true)` that aborts at record *k* answers
/// `PublicationAborted` while the records before *k* are published. This file pinned that
/// discrepancy; the lead adjudicated it under plan §25 (bn-12plt) by correcting RFC 0026 to the
/// per-artifact contract (correction 48), not by making the composite a transaction.
///
/// The corrected "Atomicity of publication" names five things a client MAY assume about a
/// partial composite. Each is an assertion here:
///
/// 1. records before *k* MAY be published, and each is complete, receipted, and indexed;
/// 2. the root is never published — the `ws_` identity is absent and its read is refused;
/// 3. record *k* and every later record are not published;
/// 4. the residue is a step function of the phase — one GC-eligible unindexed record at
///    `CommittingIndex`, none at `Staging` or `CommittingContent`;
/// 5. a retry converges on the complete publication — under the *same* idempotency key, which
///    the taxonomy names as the route (`PublicationAborted` is retryable, "same idempotency
///    key"). bn-1gj9z made the daemon honour it: the abort says `retryable: true` and does not
///    bind its key, so the same-key retry is a fresh publication and not a replayed abort.
///
/// The root-last ordering rests on `WorkspaceDescriptor::records` yielding children before
/// parents, one crate away, which is why it is measured here and not assumed.
#[test]
fn a_partial_composite_publication_meets_rfc_0026s_per_artifact_contract() {
    let mut healthy = Rig::new();
    let root = healthy.seal_workspace("req_create", "idem-create");
    let root_handle =
        identity::workspace_to_store(&root).expect("a `ws_` handle crosses to the store");
    let complete = healthy.namespace(&[]);

    for phase in PHASES {
        // Fail the second record at `phase`: the first record is already complete.
        let mut rig = Rig::new();
        let before = rig.namespace(&[]);
        rig.arm(Plan::Nth {
            phase,
            nth: 2,
            reason: AbortReason::StorageExhausted,
        });
        let (request, key) = ("req_partial", "idem-partial");
        let outcome = rig.daemon.dispatch(&rig.create_request(request, key, true));
        assert_eq!(
            outcome.error_code(),
            Some(ErrorCode::PublicationAborted),
            "{phase}: a composite that stops part-way answers `PublicationAborted`"
        );
        assert_eq!(
            outcome.envelope.error.value().map(|error| error.retryable),
            Some(true),
            "{phase}: RFC 0026's taxonomy marks `PublicationAborted` retryable under the same key"
        );

        let after = rig.namespace(std::slice::from_ref(&root_handle));
        let visible = after.visible();

        // (1) Earlier records MAY be published, and each is a complete artifact.
        assert_eq!(
            visible.len(),
            1,
            "{phase}: exactly the one record before the abort is published — RFC 0026 \
             correction 48, not \"nothing is published\""
        );
        assert!(
            after.observation.tears().is_empty(),
            "{phase}: no record is partial"
        );
        let landed = visible.iter().next().expect("one visible record").clone();
        assert_eq!(
            after.receipts.get(&landed).copied(),
            Some(1),
            "{phase}: the record that landed carries its own receipt"
        );
        assert!(
            complete.observation.indexed.contains(&landed),
            "{phase}: and it is one of the records a complete publication produces, not debris"
        );

        // (2) The root is never published.
        assert!(
            !visible.contains(&root_handle),
            "{phase}: the composite's root is published last, so the artifact the caller \
             named is absent"
        );
        assert!(
            rig.daemon
                .store()
                .read(&root_handle, &Rig::operator())
                .is_err(),
            "{phase}: and a read of it is refused rather than half-served"
        );

        // (3) Record k and every later record are not published.
        assert_eq!(
            after.published_count,
            before.published_count + 1,
            "{phase}: nothing after the first record reached the index"
        );

        // (4) The residue is a step function of the phase.
        let fresh: Vec<ArtifactHandle> = after
            .residue()
            .into_iter()
            .filter(|handle| !before.residue().contains(handle))
            .collect();
        match phase {
            PublicationPhase::Staging | PublicationPhase::CommittingContent => assert!(
                fresh.is_empty(),
                "{phase}: record k wrote nothing to the substrate, so there is no residue"
            ),
            PublicationPhase::CommittingIndex => {
                assert_eq!(
                    fresh.len(),
                    1,
                    "{phase}: exactly one GC-eligible unindexed record — record k's content"
                );
                let residue = &fresh[0];
                assert!(!after.observation.indexed.contains(residue));
                assert!(rig.daemon.store().read(residue, &Rig::operator()).is_err());
                assert_eq!(after.receipts.get(residue).copied().unwrap_or(0), 0);
            }
        }
        assert!(
            after.hard_defects().is_empty(),
            "{phase}: residue is the only defect a partial composite may leave: {:?}",
            after.hard_defects()
        );

        // (5) A retry converges on the complete publication.
        //
        // Regression guard for bn-1gj9z. The retry uses the *same* idempotency key and the
        // byte-identical request, with a fresh `request_id`. RFC 0026: "The client MAY retry
        // with the same idempotency key, and the retry is a fresh publication". Before the
        // fix, `Daemon::dispatch` step 7 recorded the abort in the replay ledger and
        // `Fault::new` fixed `retryable: false`, so this call replayed the abort.
        rig.faults.disarm();
        let retried = rig
            .daemon
            .dispatch(&rig.create_request("req_retry", key, true));
        assert_eq!(
            retried.envelope.status,
            ResultStatus::Ok,
            "{phase}: the same-key retry is a fresh publication, not a replayed abort: {:?}",
            retried.envelope.error
        );
        let converged = rig.namespace(std::slice::from_ref(&root_handle));
        assert!(
            converged.visible().contains(&root_handle),
            "{phase}: the retry publishes the root"
        );
        assert_eq!(
            converged.observation.indexed, complete.observation.indexed,
            "{phase}: the retry converges on the complete publication's identities, and the \
             record that landed first is not duplicated"
        );

        // The success binds the key: a further same-key call replays it, publishing nothing.
        let replayed = rig
            .daemon
            .dispatch(&rig.create_request("req_replay", key, true));
        assert_eq!(
            replayed.envelope.status,
            ResultStatus::Ok,
            "{phase}: the replay answers the recorded success"
        );
        assert_eq!(
            replayed.envelope.artifacts, retried.envelope.artifacts,
            "{phase}: `rule idempotency.replay` — the same artifact identity"
        );
        let after_replay = rig.namespace(std::slice::from_ref(&root_handle));
        assert_eq!(
            after_replay.published_count, converged.published_count,
            "{phase}: the replay publishes nothing"
        );

        // A fresh key converges on the same identities too.
        let fresh = rig
            .daemon
            .dispatch(&rig.create_request("req_fresh", "idem-fresh", true));
        assert_eq!(
            fresh.envelope.status,
            ResultStatus::Ok,
            "{phase}: a fresh-key publication of the same composite completes: {:?}",
            fresh.envelope.error
        );
        let converged = rig.namespace(std::slice::from_ref(&root_handle));
        assert_eq!(
            converged.observation.indexed, complete.observation.indexed,
            "{phase}: and duplicates nothing"
        );
        assert!(converged.observation.tears().is_empty());
    }
}

/// A seam that cannot name a workspace snapshot, and names every other class as BLAKE3 does.
///
/// The failure is a function of the request alone, so every identical retry meets it again.
#[derive(Debug, Clone, Copy, Default)]
struct NoSnapshotIdentity;

impl ContentIdentifier for NoSnapshotIdentity {
    fn identify(
        &self,
        class: ArtifactClass,
        content: &[u8],
    ) -> Result<ArtifactHandle, IdentityUnavailable> {
        if class == ArtifactClass::WorkspaceSnapshot {
            return Err(IdentityUnavailable);
        }
        Blake3Identity.identify(class, content)
    }
}

/// **A deterministic `PublicationAborted` binds its key.** bn-1gj9z's per-occurrence half.
///
/// RFC 0026 makes the per-occurrence `Error.retryable` authoritative over the taxonomy's
/// per-code guidance. A composite whose snapshot identity cannot be derived fails the same way
/// on every identical retry, so the daemon says `retryable: false`, and the idempotency ledger
/// reads that occurrence value: the key is bound. A different request under the same key is
/// therefore `IdempotencyKeyReused` (`rule idempotency.replay`), which it could not be if the
/// abort had left the key unbound.
#[test]
fn a_deterministic_publication_abort_binds_its_key_and_is_not_retryable() {
    let mut rig = Rig::with_identifier(NoSnapshotIdentity);
    let key = "idem-deterministic";

    let first = rig
        .daemon
        .dispatch(&rig.create_request("req_first", key, true));
    assert_eq!(first.error_code(), Some(ErrorCode::PublicationAborted));
    assert_eq!(
        first.envelope.error.value().map(|error| error.retryable),
        Some(false),
        "an abort that every identical retry meets again says it is not retryable"
    );

    let (actor, _) = rig.publisher();
    assert!(
        rig.daemon.state().replay(actor, key).is_some(),
        "the non-retryable abort is recorded under its key"
    );

    let replayed = rig
        .daemon
        .dispatch(&rig.create_request("req_again", key, true));
    assert_eq!(
        replayed.envelope.error, first.envelope.error,
        "the identical same-key retry replays the recorded abort"
    );
    assert_eq!(replayed.envelope.request_id.as_str(), "req_again");

    let reused = rig
        .daemon
        .dispatch(&rig.create_request("req_other", key, false));
    assert_eq!(
        reused.error_code(),
        Some(ErrorCode::IdempotencyKeyReused),
        "the key is bound, so a different request under it is refused"
    );
}

// =========================================================================================
// §5. Conservation: the read paths reconcile after a mixed campaign
// =========================================================================================

/// **The accounting identity.** After any mix of completed and failed publications, the index
/// count, the reachable-artifact count, and the receipt ledger reconcile exactly.
///
/// Three campaigns against three stores:
///
/// - **all succeed** — every driven seam, in order, on one daemon;
/// - **all fail** — the same script with every check refused;
/// - **mixed** — the same script with every third index commit refused, so completed and
///   failed publications interleave against one store.
///
/// In each, four numbers taken from four paths must be one number, every byte the read path
/// serves must be accounted for by class, and `fsck` must report only residue.
#[test]
fn the_read_paths_reconcile_after_a_mixed_campaign() {
    // All succeed.
    let mut rig = Rig::new();
    for seam in DRIVEN {
        rig.prepare(seam);
        let outcome = rig.drive(seam);
        assert!(
            succeeded(&outcome),
            "`{seam}` in the campaign's success half: {:?}",
            outcome.envelope.error
        );
    }
    let all_ok = rig.namespace(&[]);
    let published = all_ok.reconcile().expect("the successful half reconciles");
    assert!(
        published >= 5,
        "five seams published at least five artifacts, saw {published}"
    );
    assert!(all_ok.defects.is_empty(), "and left the store clean");

    // All fail.
    let mut injured = Rig::new();
    for seam in DRIVEN {
        injured.prepare(seam);
    }
    let prepared = injured.namespace(&[]);
    injured.arm(Plan::All {
        reason: AbortReason::StorageExhausted,
    });
    for seam in DRIVEN {
        assert_eq!(injured.drive(seam).envelope.status, ResultStatus::Error);
    }
    let after_failures = injured.namespace(&[]);
    assert_eq!(
        after_failures.render(),
        prepared.render(),
        "a campaign in which every publication fails publishes nothing"
    );
    assert!(after_failures.observation.tears().is_empty());
    assert!(after_failures.reconcile().is_ok());

    // Mixed.
    let mut mixed = Rig::new();
    for seam in DRIVEN {
        mixed.prepare(seam);
    }
    mixed.arm(Plan::EveryNthIndexCommit {
        nth: 3,
        reason: AbortReason::StorageExhausted,
    });
    let mut failures = 0_usize;
    for seam in DRIVEN {
        if mixed.drive(seam).envelope.status == ResultStatus::Error {
            failures += 1;
        }
    }
    assert!(
        failures > 0,
        "the mixed campaign must contain a failure, or it is the success campaign again"
    );
    let namespace = mixed.namespace(&[]);
    let reconciled = namespace
        .reconcile()
        .unwrap_or_else(|reason| panic!("the mixed campaign does not reconcile: {reason}"));
    assert!(
        reconciled > 0,
        "the mixed campaign published something, so the reconciliation is not vacuous"
    );
    assert!(
        !namespace.residue().is_empty(),
        "and it left residue, so the substrate half is exercised too"
    );

    // Bytes: every artifact the read path serves is accounted for by class, and the
    // attribution may exceed it only by residue.
    let token = Rig::operator();
    let mut served: BTreeMap<ArtifactClass, u64> = BTreeMap::new();
    for handle in &namespace.observation.indexed {
        let bytes = mixed
            .daemon
            .store()
            .read(handle, &token)
            .expect("an indexed artifact is readable");
        *served.entry(handle.class()).or_insert(0) += bytes.len() as u64;
    }
    for (class, bytes) in &served {
        let attributed = namespace.attribution.get(class).copied().unwrap_or(0);
        assert!(
            attributed >= *bytes,
            "attribution for {} reports {attributed} bytes but the read path serves {bytes}",
            class.token()
        );
    }
    assert!(namespace.hard_defects().is_empty());
}

// =========================================================================================
// §6. Interleaving, and the durability of the reference conjunct
// =========================================================================================

/// **Scope statement 5, measured.** Two publications interleaved at the dispatch grain never
/// observe each other's intermediates, and the namespace is a function of the completed set
/// rather than of the order.
///
/// `Daemon::dispatch` takes `&mut self` and every publication completes inside one dispatch,
/// so there is no *intra*-publication interleaving to construct at this grain; the store's own
/// concurrency is DX-13's 48-thread campaign, one crate down. What is constructible here is
/// the coarsest interleaving the surface admits — two publishing operations with a reader
/// between them, and a failure spliced between them — and the claims it can carry are exactly
/// these three.
#[test]
fn interleaved_publications_at_the_dispatch_grain_never_observe_each_others_intermediates() {
    // Order A: ingest, then link.
    let mut a = Rig::new();
    assert!(succeeded(&a.drive("observe.ingest")));
    let after_first_a = a.namespace(&[]);
    assert!(succeeded(&a.drive("evidence.link")));
    let a_final = a.namespace(&[]);
    assert!(a_final.published_count > after_first_a.published_count);

    // Order B: the same two, with a reader between them, seeing exactly the first.
    let mut b = Rig::new();
    assert!(succeeded(&b.drive("observe.ingest")));
    assert_eq!(
        b.namespace(&[]).render(),
        after_first_a.render(),
        "a reader between the two dispatches sees exactly the first publication — no \
         intermediate of the second, which has not started"
    );
    assert!(succeeded(&b.drive("evidence.link")));
    assert_eq!(
        b.namespace(&[]).render(),
        a_final.render(),
        "the namespace after both is a function of the completed set"
    );

    // A failure spliced between them: the completed publication survives byte-identical and
    // the failing one contributes nothing a reader can see.
    let mut c = Rig::new();
    assert!(succeeded(&c.drive("observe.ingest")));
    let before_failure = c.namespace(&[]);
    c.arm(Plan::All {
        reason: AbortReason::StorageExhausted,
    });
    let failed = c.drive("evidence.link");
    assert_eq!(failed.error_code(), Some(ErrorCode::PublicationAborted));
    assert_eq!(
        c.namespace(&[]).render(),
        before_failure.render(),
        "the failing publication left the completed one byte-identical"
    );
}

/// **Scope statement 4.** The daemon-side reference record of a published artifact is
/// volatile; only the store's own index survives a restart.
///
/// INV-017 gates visibility on *references* being durably committed. At the daemon grain
/// "reference" has two halves: the store index, which survives `Daemon::crash`, and the
/// evidence-graph node naming the published artifact, which lives in `DaemonState` and does
/// not. The exit package §6.4 already declares the volatility of daemon state; what this
/// measures is its reach into INV-017's third conjunct — after a restart the artifact is still
/// *visible* and the daemon-side record that named it is gone.
///
/// Stated as a narrowing rather than as a counterexample because the store index is the
/// reference the invariant's own operational reading names — RFC 0030: "a publishing
/// transaction MUST commit output content before the index entry naming it" — and that half is
/// durable. The evidence node is a second, daemon-level reference that no normative text binds
/// to INV-017.
#[test]
fn scope_the_daemon_side_reference_record_of_a_published_artifact_is_volatile() {
    let mut rig = Rig::new();
    let outcome = rig.drive("observe.ingest");
    assert!(succeeded(&outcome));
    let node = match &outcome.payload {
        Payload::ObserveIngest(response) => response
            .evidence
            .first()
            .cloned()
            .expect("an ingest names its evidence node"),
        other => panic!("expected an observe.ingest payload, got {other:?}"),
    };
    assert!(
        rig.daemon.state().evidence(&node).is_some(),
        "the daemon holds the reference record before the restart"
    );
    assert_eq!(rig.namespace(&[]).published_count, 1);

    let durable = rig.daemon.crash();
    let token = Rig::operator();
    let surviving = durable
        .store()
        .audit_view(&token)
        .expect("`cap_root` confers audit")
        .identities();
    assert_eq!(
        surviving.len(),
        1,
        "the store half of the reference survived the restart"
    );
    assert!(
        durable.store().read(&surviving[0], &token).is_ok(),
        "and the artifact is still visible"
    );

    let restarted = Rig::over(durable);
    assert!(
        restarted.daemon.state().evidence(&node).is_none(),
        "the daemon-side reference record did not survive — the narrowing this test names"
    );
    // The reconciliation still holds over the durable half, which is why this is a scope
    // statement and not a counterexample.
    assert!(
        restarted.namespace(&[]).reconcile().is_ok(),
        "the durable half of the reference conjunct reconciles after the restart"
    );
}

// =========================================================================================
// §7. Negative controls: the instrument can see a tear, and every path is load-bearing
// =========================================================================================

fn handle(token: &str) -> ArtifactHandle {
    ArtifactHandle::new(ArtifactClass::Evidence, token).expect("a well-formed handle")
}

/// The detector names an index entry with no content behind it.
///
/// The production path never produces one — content is committed first and stays pinned until
/// the index entry naming it lands — so it is hand-built here. Without this test the "no
/// tears" assertions above would be consistent with a detector that never fires.
#[test]
fn negative_control_the_detector_flags_a_reference_without_content() {
    let torn = Observation {
        indexed: BTreeSet::from([handle("aaaa")]),
        readable: BTreeSet::new(),
        receipted: BTreeSet::from([handle("aaaa")]),
    };
    assert_eq!(
        torn.tears(),
        vec![
            Tear::ReferenceWithoutContent(handle("aaaa")),
            Tear::ProvenanceWithoutVisibility(handle("aaaa")),
        ]
    );
}

/// The detector names a visible artifact with no receipt — INV-017's provenance conjunct.
#[test]
fn negative_control_the_detector_flags_visibility_without_provenance() {
    let torn = Observation {
        indexed: BTreeSet::from([handle("bbbb")]),
        readable: BTreeSet::from([handle("bbbb")]),
        receipted: BTreeSet::new(),
    };
    assert_eq!(
        torn.tears(),
        vec![Tear::VisibleWithoutProvenance(handle("bbbb"))]
    );
}

/// The detector names content reachable through a path the index does not name.
#[test]
fn negative_control_the_detector_flags_content_without_a_reference() {
    let torn = Observation {
        indexed: BTreeSet::new(),
        readable: BTreeSet::from([handle("cccc")]),
        receipted: BTreeSet::from([handle("cccc")]),
    };
    assert_eq!(
        torn.tears(),
        vec![Tear::ContentWithoutReference(handle("cccc"))]
    );
}

/// A complete artifact produces no tear, so the detector is not simply always-firing.
#[test]
fn control_the_detector_is_silent_on_a_complete_artifact() {
    let whole = Observation {
        indexed: BTreeSet::from([handle("dddd")]),
        readable: BTreeSet::from([handle("dddd")]),
        receipted: BTreeSet::from([handle("dddd")]),
    };
    assert!(whole.tears().is_empty());
    assert_eq!(whole.visible(), BTreeSet::from([handle("dddd")]));
}

/// **The mutant instrument.** A fingerprint that drops the provenance read path is blind to a
/// provenance tear the full fingerprint catches.
///
/// This is what makes the read paths load-bearing rather than decorative. The mutant is the
/// same rendering with one line class deleted, and it must *fail* to distinguish two
/// namespaces the full instrument distinguishes — otherwise the extra path was buying nothing.
#[test]
fn negative_control_a_fingerprint_without_the_provenance_path_is_blind_to_a_provenance_tear() {
    let whole = Namespace {
        observation: Observation {
            indexed: BTreeSet::from([handle("eeee")]),
            readable: BTreeSet::from([handle("eeee")]),
            receipted: BTreeSet::from([handle("eeee")]),
        },
        published_count: 1,
        receipts: BTreeMap::from([(handle("eeee"), 1)]),
        attribution: BTreeMap::from([(ArtifactClass::Evidence, 4)]),
        defects: Vec::new(),
    };
    let torn = Namespace {
        observation: Observation {
            receipted: BTreeSet::new(),
            ..whole.observation.clone()
        },
        receipts: BTreeMap::from([(handle("eeee"), 0)]),
        ..whole.clone()
    };

    assert_ne!(
        whole.render(),
        torn.render(),
        "the full instrument sees the provenance tear"
    );
    assert_eq!(
        whole.render_without_provenance(),
        torn.render_without_provenance(),
        "the mutant instrument is blind to it — which is why the provenance path is in the \
         fingerprint"
    );
    assert_eq!(
        torn.observation.tears(),
        vec![Tear::VisibleWithoutProvenance(handle("eeee"))]
    );
    assert!(torn.reconcile().is_err());
}

/// **The reconciliation detects a planted mismatch**, in each of its arms independently.
#[test]
fn negative_control_the_reconciliation_detects_a_planted_mismatch() {
    let sound = Namespace {
        observation: Observation {
            indexed: BTreeSet::from([handle("ffff")]),
            readable: BTreeSet::from([handle("ffff")]),
            receipted: BTreeSet::from([handle("ffff")]),
        },
        published_count: 1,
        receipts: BTreeMap::from([(handle("ffff"), 1)]),
        attribution: BTreeMap::new(),
        defects: Vec::new(),
    };
    assert_eq!(sound.reconcile(), Ok(1));

    let miscounted = Namespace {
        published_count: 2,
        ..sound.clone()
    };
    assert!(
        miscounted.reconcile().is_err(),
        "a count that disagrees with the listing is caught"
    );

    let unreadable = Namespace {
        observation: Observation {
            readable: BTreeSet::new(),
            ..sound.observation.clone()
        },
        ..sound.clone()
    };
    assert!(
        unreadable.reconcile().is_err(),
        "an indexed identity the read path cannot serve is caught"
    );

    let unreceipted = Namespace {
        observation: Observation {
            receipted: BTreeSet::new(),
            ..sound.observation.clone()
        },
        ..sound.clone()
    };
    assert!(
        unreceipted.reconcile().is_err(),
        "a visible artifact with no receipt is caught"
    );

    let corrupt = Namespace {
        defects: vec![StoreDefect::IdentityMismatch(handle("ffff"))],
        ..sound.clone()
    };
    assert!(
        corrupt.reconcile().is_err(),
        "a defect that is not crash residue is caught"
    );

    let residual = Namespace {
        defects: vec![StoreDefect::UnreachableContent(handle("gggg"))],
        ..sound.clone()
    };
    assert_eq!(
        residual.reconcile(),
        Ok(1),
        "and residue, which docs/35 chooses deliberately, is not a mismatch"
    );
}

/// **The sweep's own anti-vacuity control.** Without the injected fault, every assertion the
/// sweep makes about absence flips to presence.
#[test]
fn control_without_the_fault_the_composite_completes_and_the_root_is_visible() {
    let mut rig = Rig::new();
    let root = rig.seal_workspace("req_create", "idem-create");
    let root_handle =
        identity::workspace_to_store(&root).expect("a `ws_` handle crosses to the store");
    let namespace = rig.namespace(&[]);
    assert!(
        namespace.visible().contains(&root_handle),
        "the composite's root is visible when nothing is injured"
    );
    assert!(namespace.observation.tears().is_empty());
    assert!(
        namespace.defects.is_empty(),
        "and the store is clean — not even residue"
    );
    assert!(namespace.reconcile().is_ok());
}

/// **The census scanner's own control.** It finds the sites this file expects, it reads calls
/// split across lines, and it skips prose — so a green census is a statement about control
/// flow.
#[test]
fn control_the_census_scanner_reads_control_flow_and_not_prose() {
    let sites = write_call_sites();
    let files: BTreeSet<&str> = sites.iter().map(|(file, _, _)| file.as_str()).collect();
    assert_eq!(
        files,
        BTreeSet::from([
            "daemon/evidence.rs",
            "daemon/observe.rs",
            "daemon/verification.rs",
            "daemon/workspace.rs",
        ]),
        "exactly four daemon modules reach the store's write surface"
    );
    // `daemon/output.rs` names `PublicationAborted` and INV-017 in prose and publishes
    // nothing. If the scanner counted comments it would appear above.
    assert!(!files.contains("daemon/output.rs"));

    // Whitespace normalisation is load-bearing: `rustfmt` puts the receiver on the previous
    // line, so a line-wise scanner finds none of these and the census would pass vacuously.
    let source = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/daemon/observe.rs"),
    )
    .expect("the daemon's own source is readable");
    assert!(
        !source.contains("store.publish("),
        "the call this scanner finds in `observe.rs` is not on one line, which is the whole \
         reason the scanner normalises whitespace"
    );
    assert!(
        sites
            .iter()
            .any(|(file, _, call)| file == "daemon/observe.rs" && call == "store.publish("),
        "and the scanner finds it anyway"
    );
}

// =========================================================================================
// The rig
// =========================================================================================

/// The store's three phases, in the order a publication walks them.
const PHASES: [PublicationPhase; 3] = [
    PublicationPhase::Staging,
    PublicationPhase::CommittingContent,
    PublicationPhase::CommittingIndex,
];

/// A dispatch that did the work: `Ok` for a synchronous operation, `TaskStarted` for a
/// `@task_starting` one. Both mean the handler ran and published.
fn succeeded(outcome: &OperationOutcome) -> bool {
    matches!(
        outcome.envelope.status,
        ResultStatus::Ok | ResultStatus::TaskStarted
    )
}

/// A daemon prepared to the point where a named seam is one dispatch away.
struct Rig {
    daemon: Daemon,
    faults: Injector,
    intent: IntentHandle,
    files: Vec<Commitment>,
    configuration: Commitment,
    trace: Commitment,
    receipt: Commitment,
    /// A sealed snapshot, once one exists.
    snapshot: Option<WorkspaceHandle>,
    /// An unsealed snapshot, once one exists — `workspace.seal`'s subject.
    unsealed: Option<WorkspaceHandle>,
    /// An evidence node, once one exists — `evidence.link`'s subject.
    subject: Option<EvidenceHandle>,
    /// Whether the publishing capability has been swapped for a `read`-level one.
    demoted: bool,
    /// Bumped on every drive, so a repeated drive of one seam is a *fresh* request.
    ///
    /// Load-bearing rather than cosmetic: RFC 0026 makes a replay under one idempotency key
    /// return the original answer, so a campaign that drove `observe.ingest` twice under one
    /// key would be measuring the ledger rather than the publication. The mixed campaign of
    /// §5 drives every seam twice — once healthy, once injured — and would otherwise read the
    /// healthy answer back the second time.
    nonce: u32,
}

impl Rig {
    fn new() -> Self {
        let faults = Injector::default();
        Self::assemble(builder().store_faults(faults.clone()), faults)
    }

    /// Restart over what a crash left behind. `Builder::over` adopts the surviving store, so
    /// the fault seam is the store's own and a fresh injector would be ignored.
    fn over(durable: DurableSubstrate) -> Self {
        Self::assemble(builder().over(durable), Injector::default())
    }

    /// A rig whose content-identity seam is `identifier`, with no store faults armed.
    fn with_identifier<I>(identifier: I) -> Self
    where
        I: ContentIdentifier + Clone + 'static,
    {
        let faults = Injector::default();
        Self::assemble(
            builder_with(identifier).store_faults(faults.clone()),
            faults,
        )
    }

    fn assemble(builder: Builder, faults: Injector) -> Self {
        let mut daemon = builder.build();

        let contract = die_hard_contract();
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

        let stage = |daemon: &mut Daemon, path: &str, content: &str| {
            daemon
                .state_mut()
                .stage(
                    &Blake3Identity,
                    WorkspacePath::new(path).expect("a workspace path"),
                    content.as_bytes().to_vec(),
                )
                .expect("staging names its content")
        };
        let files = vec![
            stage(&mut daemon, MODULE_PATH, DIE_HARD_MODEL),
            stage(&mut daemon, "README.md", "# TV-009\n"),
        ];
        let configuration = stage(&mut daemon, "default.model.toml", DIE_HARD_CONFIG);
        let trace = stage(&mut daemon, "traces/die-hard.jsonl", TRACE);
        let receipt = stage(&mut daemon, "receipts/kernel-core.json", CHECK_RECEIPT);

        daemon.state_mut().models_mut().register(
            die_hard_source(),
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

        Self {
            daemon,
            faults,
            intent,
            files,
            configuration,
            trace,
            receipt,
            snapshot: None,
            unsealed: None,
            subject: None,
            demoted: false,
            nonce: 0,
        }
    }

    /// The store token `cap_root` names — read and audit authority over every class.
    fn operator() -> CapabilityToken {
        identity::capability_to_store(&cap("cap_root")).expect("`cap_root` is a store token")
    }

    /// Read this daemon's store, probing `universe` beyond what the index names.
    fn namespace(&self, universe: &[ArtifactHandle]) -> Namespace {
        Namespace::of(&self.daemon, universe)
    }

    /// Arm the fault seam, resetting its counters. Checks are counted from here, so `nth: 1`
    /// means "the first check the *next* operation makes".
    fn arm(&self, plan: Plan) {
        self.faults.arm(plan);
    }

    /// Publish under a capability that cannot publish.
    ///
    /// `cap_reader` sits at `read`, below `Action::Publish`'s `propose` floor, so the daemon
    /// refuses admission before the store's state machine runs. One switch covers all five
    /// seams because every driver consults it.
    fn demote(&mut self) {
        self.demoted = true;
    }

    /// Build whatever `seam` needs before it can publish, with the fault seam disarmed, so a
    /// later abort is the seam's own and never a missing input.
    fn prepare(&mut self, seam: &str) {
        self.faults.disarm();
        match seam {
            "workspace.create" | "observe.ingest" => {}
            "workspace.seal" => {
                self.unsealed_workspace();
            }
            "verification.start" => {
                self.sealed_workspace();
            }
            "evidence.link" => {
                let outcome = self.drive("observe.ingest");
                assert!(
                    succeeded(&outcome),
                    "`evidence.link` needs a subject node: {:?}",
                    outcome.envelope.error
                );
            }
            other => panic!("no preparation is defined for `{other}`"),
        }
        self.faults.disarm();
    }

    /// Drive one seam, once, under a request identity no earlier drive used.
    fn drive(&mut self, seam: &str) -> OperationOutcome {
        self.nonce += 1;
        match seam {
            "workspace.create" => self.drive_workspace_create(),
            "workspace.seal" => self.drive_workspace_seal(),
            "verification.start" => self.drive_verification_start(),
            "observe.ingest" => self.drive_observe_ingest(),
            "evidence.link" => self.drive_evidence_link(),
            other => panic!("no driver is defined for `{other}`"),
        }
    }

    fn publisher(&self) -> (&'static str, &'static str) {
        if self.demoted {
            ("agent:reader", "cap_reader")
        } else {
            ("agent:builder", "cap_builder")
        }
    }

    // --- the drivers ---------------------------------------------------------------------

    fn create_request(&self, request: &str, key: &str, seal: bool) -> OperationRequest {
        let (actor, capability) = self.publisher();
        OperationRequest {
            envelope: keyed(
                envelope("workspace.create", actor, capability, request),
                key,
            ),
            arguments: Arguments::WorkspaceCreate(WorkspaceCreateRequest {
                components: SnapshotComponents {
                    files: self.files.clone(),
                    cml_modules: Vec::new(),
                    rust_extraction: Vec::new(),
                    domain_packs: Vec::new(),
                    dependencies: Vec::new(),
                    epochs: SnapshotEpochs {
                        semantic: epoch("semantic-1"),
                        proof: epoch("proof-1"),
                        toolchain: Optional::Absent,
                    },
                    intent: self.intent.clone(),
                    correspondence: Vec::new(),
                    proof_environment: Vec::new(),
                    configuration: vec![self.configuration.clone()],
                    file_components: Optional::Absent,
                },
                overlay: Optional::Absent,
                seal: Optional::Present(seal),
            }),
        }
    }

    fn drive_workspace_create(&mut self) -> OperationOutcome {
        let (request, key) = self.tag("create");
        let request = self.create_request(&request, &key, true);
        self.daemon.dispatch(&request)
    }

    /// A request identity and idempotency key no earlier drive on this rig used.
    fn tag(&self, seam: &str) -> (String, String) {
        (
            format!("req_{seam}_{}", self.nonce),
            format!("idem-{seam}-{}", self.nonce),
        )
    }

    /// Create-and-seal, expecting success, answering with the workspace handle.
    fn seal_workspace(&mut self, request: &str, key: &str) -> WorkspaceHandle {
        let request = self.create_request(request, key, true);
        let created = self.daemon.dispatch(&request);
        assert_eq!(
            created.envelope.status,
            ResultStatus::Ok,
            "{:?}",
            created.envelope.error
        );
        let handle = match &created.payload {
            Payload::WorkspaceCreate(response) => response.snapshot.clone(),
            other => panic!("expected a workspace.create payload, got {other:?}"),
        };
        self.snapshot = Some(handle.clone());
        handle
    }

    /// A sealed snapshot, created once. It publishes, so callers that measure must read the
    /// namespace after it.
    fn sealed_workspace(&mut self) -> WorkspaceHandle {
        match self.snapshot.clone() {
            Some(handle) => handle,
            None => self.seal_workspace("req_sealed", "idem-sealed"),
        }
    }

    /// An unsealed snapshot, created once. `seal: false` reaches no store write, so this
    /// publishes nothing.
    fn unsealed_workspace(&mut self) -> WorkspaceHandle {
        if let Some(handle) = self.unsealed.clone() {
            return handle;
        }
        let request = self.create_request("req_unsealed", "idem-unsealed", false);
        let created = self.daemon.dispatch(&request);
        assert_eq!(
            created.envelope.status,
            ResultStatus::Ok,
            "an unsealed create publishes nothing and must succeed: {:?}",
            created.envelope.error
        );
        let handle = match &created.payload {
            Payload::WorkspaceCreate(response) => response.snapshot.clone(),
            other => panic!("expected a workspace.create payload, got {other:?}"),
        };
        self.unsealed = Some(handle.clone());
        handle
    }

    fn drive_workspace_seal(&mut self) -> OperationOutcome {
        let snapshot = self.unsealed_workspace();
        let (actor, capability) = self.publisher();
        let (request, key) = self.tag("seal");
        let mut request_envelope = keyed(
            envelope("workspace.seal", actor, capability, &request),
            &key,
        );
        request_envelope.snapshot = Nullable::Value(snapshot.clone());
        self.daemon.dispatch(&OperationRequest {
            envelope: request_envelope,
            arguments: Arguments::WorkspaceSeal(WorkspaceSealRequest { snapshot }),
        })
    }

    /// `verification.start` over a sealed snapshot — the campaign-record publication.
    fn drive_verification_start(&mut self) -> OperationOutcome {
        let snapshot = self.sealed_workspace();
        let (actor, capability) = if self.demoted {
            ("agent:reader", "cap_reader")
        } else {
            ("agent:runner", "cap_runner")
        };
        let (request, key) = self.tag("start");
        let mut request_envelope = keyed(
            envelope("verification.start", actor, capability, &request),
            &key,
        );
        request_envelope.budget = Optional::Present(budget(64));
        request_envelope.snapshot = Nullable::Value(snapshot);
        self.daemon.dispatch(&OperationRequest {
            envelope: request_envelope,
            arguments: Arguments::VerificationStart(VerificationStartRequest {
                target: target(TargetKind::AllClaims, "DieHard"),
                portfolio: Portfolio::Interactive,
                context_policy: Optional::Absent,
                priority_class: Optional::Absent,
            }),
        })
    }

    /// `observe.ingest` — a single-record publication of a staged production trace.
    fn drive_observe_ingest(&mut self) -> OperationOutcome {
        let (actor, capability) = if self.demoted {
            ("agent:reader", "cap_reader")
        } else {
            ("agent:observer", "cap_observer")
        };
        let (request, key) = self.tag("ingest");
        let mut request_envelope = keyed(
            envelope("observe.ingest", actor, capability, &request),
            &key,
        );
        request_envelope.budget = Optional::Present(budget(64));
        let outcome = self.daemon.dispatch(&OperationRequest {
            envelope: request_envelope,
            arguments: Arguments::ObserveIngest(ObserveIngestRequest {
                trace: self.trace.clone(),
                instrumentation_profile: PROFILE.to_owned(),
            }),
        });
        if let Payload::ObserveIngest(response) = &outcome.payload
            && let Some(node) = response.evidence.first()
        {
            self.subject = Some(node.clone());
        }
        outcome
    }

    /// `evidence.link` — a single-record publication of a checker receipt.
    fn drive_evidence_link(&mut self) -> OperationOutcome {
        let subject = self
            .subject
            .clone()
            .expect("`evidence.link` is prepared with an ingest, which names its subject");
        let (actor, capability) = if self.demoted {
            ("agent:reader", "cap_reader")
        } else {
            ("service:kernel-core", "cap_checker")
        };
        let (request, key) = self.tag("link");
        let mut request_envelope =
            keyed(envelope("evidence.link", actor, capability, &request), &key);
        request_envelope.budget = Optional::Present(budget(64));
        self.daemon.dispatch(&OperationRequest {
            envelope: request_envelope,
            arguments: Arguments::EvidenceLink(EvidenceLinkRequest {
                subject,
                receipt: self.receipt.clone(),
                checker_profile: PROFILE.to_owned(),
            }),
        })
    }
}

// =========================================================================================
// The fault seam
// =========================================================================================

/// What the injector should refuse, and when. Counting starts at [`Injector::arm`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Plan {
    /// Refuse the `nth` check of `phase`.
    Nth {
        phase: PublicationPhase,
        nth: u64,
        reason: AbortReason,
    },
    /// Refuse the `nth` check of any phase — the stroboscopic stop. Counting across phases
    /// rather than within one makes the index a position in the publication's own timeline:
    /// check 1 is the first record's staging, check 2 its content commit, check 3 its index
    /// commit, check 4 the second record's staging, and so on.
    Call { nth: u64, reason: AbortReason },
    /// Refuse every index commit whose ordinal is a multiple of `nth` — the mixed campaign.
    EveryNthIndexCommit { nth: u64, reason: AbortReason },
    /// Refuse every check.
    All { reason: AbortReason },
}

/// One `StorageFaults` seam that every test in this file shares.
///
/// A single armable seam rather than five fault types, for a reason that is not convenience:
/// `Builder::store_faults` is a build-time option, so a fault installed at construction fires
/// on a seam's *preconditions* as readily as on the seam itself. An injector that starts inert
/// and is armed after the preconditions are built is what makes "the first check this
/// operation makes" a statement anything can act on.
///
/// It also records the phase sequence it sees, which is how §4 learns the shape of a composite
/// publication from the daemon instead of from a constant.
#[derive(Debug, Default, Clone)]
struct Injector {
    inner: Arc<std::sync::Mutex<InjectorState>>,
}

#[derive(Debug, Default)]
struct InjectorState {
    plan: Option<Plan>,
    /// Every phase check since the last arm or disarm, in order.
    timeline: Vec<PublicationPhase>,
    per_phase: BTreeMap<PublicationPhase, u64>,
    index_commits: u64,
}

impl Injector {
    fn arm(&self, plan: Plan) {
        *self.inner.lock().expect("an unpoisoned lock") = InjectorState {
            plan: Some(plan),
            ..InjectorState::default()
        };
    }

    fn disarm(&self) {
        *self.inner.lock().expect("an unpoisoned lock") = InjectorState::default();
    }

    /// The phase sequence seen since the last arm or disarm.
    fn trace(&self) -> Vec<PublicationPhase> {
        self.inner
            .lock()
            .expect("an unpoisoned lock")
            .timeline
            .clone()
    }
}

impl StorageFaults for Injector {
    fn check(&self, phase: PublicationPhase) -> Result<(), AbortReason> {
        let mut state = self.inner.lock().expect("an unpoisoned lock");
        state.timeline.push(phase);
        let calls = state.timeline.len() as u64;
        let seen = {
            let counter = state.per_phase.entry(phase).or_insert(0);
            *counter += 1;
            *counter
        };
        if phase == PublicationPhase::CommittingIndex {
            state.index_commits += 1;
        }
        match state.plan {
            Some(Plan::Nth {
                phase: wanted,
                nth,
                reason,
            }) if phase == wanted && seen == nth => Err(reason),
            Some(Plan::Call { nth, reason }) if calls == nth => Err(reason),
            Some(Plan::EveryNthIndexCommit { nth, reason })
                if phase == PublicationPhase::CommittingIndex && state.index_commits % nth == 0 =>
            {
                Err(reason)
            }
            Some(Plan::All { reason }) => Err(reason),
            _ => Ok(()),
        }
    }
}

// =========================================================================================
// Fixtures
// =========================================================================================

fn version() -> ProtocolVersion {
    ProtocolVersion::new(3, 1)
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

fn now() -> Timestamp {
    Timestamp::new("2026-08-01T00:00:00.000Z").expect("a well-formed timestamp")
}

fn profile(privileged: &[&str], grants: &[DataGrant]) -> CapabilityProfile {
    CapabilityProfile {
        privileged_operations: privileged.iter().map(|entry| name(entry)).collect(),
        denied_operations: Vec::new(),
        data_grants: grants.to_vec(),
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
        client: "continuumd-g1-06-acceptance".to_owned(),
        actor: who("service:continuumd"),
        capability: cap("cap_root"),
        features: Optional::Absent,
    };
    negotiate(&[version()], ProtocolWindow::new(3), ENCODINGS, &hello).expect("3.1 is served")
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

/// The builder every rig goes through, so two rigs differ in exactly one thing: the seam.
fn builder() -> Builder {
    builder_with(Blake3Identity)
}

/// [`builder`] over a chosen content-identity seam.
fn builder_with<I>(identifier: I) -> Builder
where
    I: ContentIdentifier + Clone + 'static,
{
    let root = Some(cap("cap_root"));
    let privileged = ["intent.accept", "intent.reject", "intent.lock"];
    Daemon::builder(identifier, negotiated(), cap("cap_root"))
        .epochs(epochs())
        .now(now())
        .capability(
            grant(
                "cap_root",
                "service:continuumd",
                AuthorityLevel::Promote,
                4,
                Optional::Present(profile(&privileged, &[DataGrant::ProductionTrace])),
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
                "cap_observer",
                "agent:observer",
                AuthorityLevel::Execute,
                3,
                Optional::Present(profile(&[], &[DataGrant::ProductionTrace])),
            ),
            root.clone(),
        )
        .capability(
            grant(
                "cap_checker",
                "service:kernel-core",
                AuthorityLevel::Execute,
                3,
                Optional::Present(profile(&[], &[DataGrant::ProductionTrace])),
            ),
            root.clone(),
        )
        .capability(
            grant(
                "cap_reader",
                "agent:reader",
                AuthorityLevel::Read,
                3,
                Optional::Present(profile(&[], &[DataGrant::ProductionTrace])),
            ),
            root.clone(),
        )
        .capability(
            grant(
                "cap_steward",
                "human:steward",
                AuthorityLevel::ReviseIntent,
                3,
                Optional::Present(profile(&privileged, &[])),
            ),
            root,
        )
        .family(WorkspaceFamily)
        .family(IntentFamily)
        .family(TaskFamily)
        .family(VerificationFamily)
        .family(EvidenceFamily::new())
        .family(ObserveFamily)
}

fn die_hard_contract() -> IntentContract {
    IntentContract::decode(DIE_HARD_CONTRACT.trim_end().as_bytes()).expect("the fixture decodes")
}

fn intent_handle(contract: &IntentContract) -> IntentHandle {
    let stored = ContentIdentifier::identify(
        &Blake3Identity,
        ArtifactClass::IntentContract,
        &contract.identity_preimage_bytes(),
    )
    .expect("blake3 names every input");
    identity::intent_to_wire(&stored).expect("an `in_` handle")
}

fn die_hard_source() -> Commitment {
    model_source(&Blake3Identity, [(MODULE_PATH, DIE_HARD_MODEL.as_bytes())])
        .expect("blake3 names the module set")
}

fn acceptance_bytes() -> Opaque {
    let mut fields: BTreeMap<String, Json> = BTreeMap::new();
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

fn budget(states: u64) -> Budget {
    Budget {
        wall_ms: Optional::Absent,
        cpu_ms: Optional::Absent,
        memory_bytes: Optional::Absent,
        states: Optional::Present(states),
        solver_ms: Optional::Absent,
        proof_ms: Optional::Absent,
        tokens: Optional::Absent,
        candidates: Optional::Absent,
        bytes: Optional::Absent,
    }
}

fn target(kind: TargetKind, id: &str) -> Target {
    Target {
        kind,
        id: id.to_owned(),
    }
}
