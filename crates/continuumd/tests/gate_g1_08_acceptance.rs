//! G1-08 acceptance (bn-q80m7): an **independent re-derivation** of "daemon crash recovery
//! leaves no stale index entries or orphan tasks (plan §4.5)".
//!
//! This file is not the delivering suite re-run. `bn-3dr` delivered
//! `crates/continuumd/src/daemon/recovery.rs` and
//! `crates/continuumd/tests/g1_crash_recovery_evidence.rs`, and both are read here as the
//! *subject*, never as the oracle: every conclusion below is re-derived from store and
//! daemon primitives — `ReferenceStore::read`, `StoreAudit::identities`,
//! `StoreAudit::receipts`, `StoreAudit::storage_attribution`, `Blake3Identity`, and the
//! daemon's own state accessors — and `recovery::recover` and `StoreAudit::fsck` are then
//! *compared against* that census rather than consulted to build it.
//!
//! # The derived scope, before any test
//!
//! G1-08's cited text (`notes/plan/docs/52_RELEASE_GATES_REV3.md:22`) is one sentence with
//! two nouns. Plan §4.5's Crash-safety bullet is what it points at, and that bullet is three
//! obligations, not one:
//!
//! | | plan §4.5 says | this build has |
//! |---|---|---|
//! | **O1** | "Publication commits content before index; a crash leaves unreachable content eligible for GC, never a stale index entry." | **built.** The two-phase protocol, the in-flight root pin, `StoreAudit::fsck`, `recovery::recover`, and a real restart seam (`Daemon::crash` → `Builder::over`). |
//! | **O2** | "On restart, `Running` tasks resume from their last committed continuation or transition to `Failed` with a typed reason — never to a silently reconstructed state." | **unbuilt.** There is no durable task record. Ten of thirteen `VolatileFact`s are `Disposition::Declared`, the task table and the continuation table among them. |
//! | **O3** | "An index verifier (fsck) ships with the daemon." | **built.** `StoreAudit::fsck`. |
//!
//! So the two nouns of G1-08 land on opposite sides of that table, and the honest verdict is
//! per conjunct rather than per criterion:
//!
//! - **"no stale index entries"** is O1 + O3. Recovery machinery exists, so this conjunct is
//!   *testable* and is tested here by independent census
//!   ([`the_independent_census_of_a_crashed_store_finds_no_stale_entry_and_no_orphan_task`],
//!   [`the_three_read_paths_reconcile_exactly`],
//!   [`the_independent_census_and_the_stores_own_fsck_agree`]).
//! - **"no orphan tasks"** is O2. After a daemon drop the task table is *empty*, so the
//!   property holds by the absence of its subject. That is vacuous truth, not evidence: the
//!   positive obligation O2 states — resolve each `Running` task to its committed
//!   continuation or to `Failed` with a typed reason — has no machinery and no subject. This
//!   file therefore **pins the loss** mechanically over all thirteen facts
//!   ([`derived_scope_every_declared_fact_that_had_a_witness_is_gone_after_the_restart`]) so
//!   a future persistence change flips the pin, and states the debt below.
//!
//! ## The reachable analog, labelled as such
//!
//! The nearest non-vacuous property this architecture *can* express is the **within-lifetime
//! consistency floor**: inside one daemon lifetime, after any mix of served, refused and
//! aborted operations, no index entry lacks backing and no task claims an artifact the store
//! does not hold ([`after_a_refused_and_aborted_operation_mix_the_within_lifetime_census_is_exact`],
//! [`no_task_claims_an_artifact_the_store_does_not_hold_across_the_mix`]). This is **not**
//! crash recovery. It is the state O2's recovery would have to *restore*, and it is measured
//! here so that the debt spec below has a target.
//!
//! ## The debt spec — what O2 would require
//!
//! Actionable, in the shape `bn-3dr` left it and this file measures:
//!
//! 1. **A durable task record.** `TaskEntry` — at minimum `handle`, `operation`, `status`,
//!    `snapshot`, `intent`, `epochs`, the ledger's checkpoints, and `Publications::committed`
//!    — written before the operation that changes it answers. Today `VolatileFact::TaskTable`
//!    is `Declared` and `survives_as() == Some(ArtifactClass::Task)` names only the published
//!    *evidence*, never the record.
//! 2. **A durable continuation.** `VolatileFact::ContinuationTable` has
//!    `survives_as() == None`: nothing of a parked continuation reaches the store at all, so
//!    "resume from the last committed continuation" has no referent across a restart.
//! 3. **A startup resolution pass.** For every durable record in a non-terminal state:
//!    resume it under its pinned epochs, or write `Failed` with a typed `ErrorCode`. Neither
//!    branch exists; `recovery::recover` takes `&ReferenceStore` and no task input at all,
//!    which is this file's mechanical evidence that the pass is unbuilt
//!    ([`derived_scope_the_recovery_surface_is_store_shaped`]).
//! 4. **An orphan reaper with something to reap.** The census in this file *is* the reaper's
//!    detector, and [`negative_control_a_planted_orphan_task_is_detected`] shows it fires. It
//!    has nothing to run against after a restart because there are no tasks.
//! 5. **A crate-boundary decision.** Plan §20 gives `continuumd` no storage edge beyond
//!    `continuum-workspace`, so 1–3 need either a task artifact class in that store or a new
//!    edge. This is a dossier decision, not an implementation detail.
//!
//! ## Absences stated (INV-007)
//!
//! - **`StoreDefect::MissingReferent` is unconstructible** through the store's public API:
//!   content is committed before the index, is pinned while in flight, and is removed only by
//!   `collect_garbage`, which reclaims nothing the index names. So the "stale index entry" of
//!   G1-08's first noun cannot be *planted* here; the negative control shows the census's
//!   resolution primitive is two-sided instead
//!   ([`negative_control_the_resolution_primitive_is_two_sided`]).
//! - **The receipt ledger is not enumerable** through any public API. "No receipt names an
//!   unpublished identity" is therefore re-derived only over identities this census can name
//!   independently — the index, plus the residue handles a driven crash lets it predict — and
//!   not over the ledger's own key set.
//! - **The crash grain is a dropped value, not a dropped process.** `Daemon::crash` consumes
//!   the daemon and hands back `DurableSubstrate`; the store is in memory and survives because
//!   the value moved. Below `fsync` is out of reach for the same reason `bn-3dr` recorded.
//! - **Two of thirteen volatile facts have no witness in this drive** — the evidence graph and
//!   the evidence event log are not populated by the operations reachable here — so the pin
//!   over them is vacuous and is reported as such rather than counted.
//! - **Single-threaded.** `Daemon::dispatch` takes `&mut self`; no concurrent-daemon claim is
//!   made or attempted.
//!
//! # Method
//!
//! One census, three read paths, run over a store nobody told it anything about:
//!
//! 1. **the index** — `StoreAudit::identities` and `StoreAudit::published_count`;
//! 2. **the content** — `ReferenceStore::read` for every identity, under an operator
//!    capability that covers every class, with the bytes re-derived through `Blake3Identity`,
//!    the daemon's *declared* ADR-0013 seam. This is deliberately not the store's own
//!    identifier: `fsck` asks the identity seam to check itself, so a substituted seam is
//!    invisible to it and visible here ([`negative_control_a_substituted_identity_seam_is_caught_by_the_census_and_missed_by_fsck`]);
//! 3. **the ledger** — `StoreAudit::receipts` per identity, checking the count *and* that
//!    each receipt names the identity it is filed under.
//!
//! Residue is derived by arithmetic over two independent readings — `storage_attribution`
//! minus the bytes the read path returned — so the census detects unreachable content without
//! consulting `fsck` at all.
//!
//! # The verdict this file delivers (INV-008)
//!
//! - **"no stale index entries" — SATISFIED at a narrowed scope.** Independently re-derived
//!   over a dropped-and-restarted daemon and over a within-lifetime operation mix; the census
//!   and the store's own `fsck` agree on the clean case and on the residue case. The narrowing
//!   is the crash grain (a dropped value, not a dropped process, and no `fsync` below it) and
//!   the single-threaded dispatch.
//! - **"no orphan tasks" — UNSUPPORTED (recovery-machinery-unbuilt).** True only of an empty
//!   set. The detector exists and fires ([`negative_control_a_planted_orphan_task_is_detected`]);
//!   the subject does not survive a restart, and the positive obligation O2 states has no
//!   implementation. The debt spec above is the actionable form.
//!
//! # Four findings this re-derivation produced that the delivering evidence does not carry
//!
//! 1. **`fsck` cannot see a substituted identity seam.** It re-derives with the store's own
//!    identifier, so a store whose identifier is pure, self-consistent and *not* the declared
//!    `Blake3Identity` passes its own audit with every entry misnamed. A census pinned to the
//!    declared seam catches all of them. This is a property of the instrument, not a defect of
//!    this build — every daemon in this tree supplies `Blake3Identity`.
//! 2. **Residue is derivable without a defect list.** `storage_attribution` minus the read
//!    path's bytes prices unreachable content per class, with no access to `fsck` at all, and
//!    agrees with `fsck` handle for handle on the driven case.
//! 3. **Two of the ten `Declared` facts come back after a restart** — the intent registry and
//!    the admission log — not because anything was recovered but because the *deployment's own
//!    startup work* rewrites them. `Disposition` is a statement about what a restart
//!    reconstructs; it is not a statement about what a running deployment happens to redo. Of
//!    the three `Reprovisioned` facts, only the wire capability registry is restored by
//!    `Builder` itself; staged content and the model catalog wait for the deployment.
//! 4. **A daemon dropped mid-dispatch leaves an artifact with no claimant.** Killed at
//!    `CrashPoint::AfterHandler` of the campaign-publishing dispatch, the store holds a
//!    complete, resolvable, receipted `task_*` record that no surviving task claims. It is
//!    neither a stale entry nor residue nor an orphan task — it is O2's absence seen from the
//!    other side, and it is what a startup resolution pass would have to reconnect.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use continuum_engine_reference::diehard;
use continuum_intent::canonical_json::Json;
use continuum_intent::contract::IntentContract;
use continuum_value::epoch::ProtocolWindow;
use continuum_workspace::artifact_path::{ArtifactClass, ArtifactHandle};
use continuum_workspace::lineage::ForkName;
use continuum_workspace::publication::{
    AbortReason, ActorId as StoreActor, AuditLog, AuthorityLevel as StoreLevel, CapabilityToken,
    ContentIdentifier, IdentityUnavailable, PublicationPhase, ReferenceStore, StorageFaults,
    StoreDefect,
};
use continuum_workspace::snapshot::WorkspacePath;
use continuumd::daemon::budget::Publications;
use continuumd::daemon::family::{Arguments, Payload};
use continuumd::daemon::identity::Blake3Identity;
use continuumd::daemon::intent::IntentFamily;
use continuumd::daemon::recovery::{self, CrashInjector, CrashPoint, Disposition, VolatileFact};
use continuumd::daemon::state::{IntentRecord, RegistryStatus};
use continuumd::daemon::task::TaskFamily;
use continuumd::daemon::verification::{VerificationFamily, model_source};
use continuumd::daemon::workspace::WorkspaceFamily;
use continuumd::daemon::{Builder, Daemon, DurableSubstrate, OperationRequest};
use continuumd::protocol::envelope::{Budget, EpochSet, RequestEnvelope};
use continuumd::protocol::handshake::{
    CapabilityDescriptor, CapabilityProfile, ClientHello, Negotiated, VersionRange, negotiate,
};
use continuumd::protocol::operations::intent::IntentAcceptRequest;
use continuumd::protocol::operations::task::{TaskCancelRequest, TaskStatusRequest};
use continuumd::protocol::operations::verification::VerificationStartRequest;
use continuumd::protocol::operations::workspace::WorkspaceCreateRequest;
use continuumd::protocol::registry::ENCODINGS;
use continuumd::protocol::scalar::{
    ActorId, CapabilityHandle, Commitment, ContinuationHandle, EpochIdentity, IntentHandle, Opaque,
    OperationName, ProtocolVersion, RequestId, TaskHandle, Timestamp, WorkspaceHandle,
};
use continuumd::protocol::shared::{SnapshotComponents, SnapshotEpochs, Target};
use continuumd::protocol::spec::{Nullable, Optional};
use continuumd::protocol::vocabulary::{
    AuthorityLevel, Encoding, ErrorCode, Portfolio, ResultStatus, TargetKind, TaskStatus,
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

// =============================================================================================
// the independent census
// =============================================================================================

/// One inconsistency the census found, named by what disagreed.
///
/// Deliberately *not* `StoreDefect`: that vocabulary is the store's own and is what this file
/// re-derives rather than reuses. The two are compared in
/// [`the_independent_census_and_the_stores_own_fsck_agree`], which is only a comparison
/// because they are separate values built by separate code.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
enum Finding {
    /// The index names an identity the ordinary read path will not return: a stale entry.
    Unresolvable(ArtifactHandle),
    /// The bytes behind an index entry do not re-derive to the identity they are filed under,
    /// through the daemon's declared identity seam.
    IdentityDisagreement {
        /// The identity the entry is filed under.
        filed: ArtifactHandle,
        /// What `Blake3Identity` derives from the bytes the read path returned.
        derived: String,
    },
    /// A published identity the receipt ledger does not name.
    Receiptless(ArtifactHandle),
    /// A receipt filed under one identity that names another.
    MisattributedReceipt(ArtifactHandle),
    /// A task claiming an artifact the store does not hold: the orphan-task detector.
    OrphanTask {
        /// The task making the claim.
        task: String,
        /// The identity it claims and the store cannot resolve.
        claimed: ArtifactHandle,
    },
}

impl Finding {
    /// A stable one-line rendering.
    fn render(&self) -> String {
        match self {
            Self::Unresolvable(handle) => format!("unresolvable {handle}"),
            Self::IdentityDisagreement { filed, derived } => {
                format!("identity-disagreement filed={filed} derived={derived}")
            }
            Self::Receiptless(handle) => format!("receiptless {handle}"),
            Self::MisattributedReceipt(handle) => format!("misattributed-receipt {handle}"),
            Self::OrphanTask { task, claimed } => format!("orphan-task {task} claims {claimed}"),
        }
    }
}

/// What one task claims of the store: the identities its committed publications named.
type Claim = (String, ArtifactHandle);

/// An independent accounting of a store, and of the tasks that claim things in it.
///
/// Built from primitives only. Nothing here calls `recovery::recover` or `StoreAudit::fsck`.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Census {
    /// Every identity the index names, in identity order.
    entries: Vec<ArtifactHandle>,
    /// The store's own count of index entries, read separately from `entries`.
    published_count: usize,
    /// Bytes the read path returned, per class.
    read_bytes: BTreeMap<ArtifactClass, u64>,
    /// Bytes the store attributes to each class — published and unreachable together.
    attributed_bytes: BTreeMap<ArtifactClass, u64>,
    /// Receipts per published identity.
    receipts: BTreeMap<ArtifactHandle, u64>,
    /// The task claims this census was asked to check.
    claims: Vec<Claim>,
    /// Everything that disagreed, in a deterministic order.
    findings: Vec<Finding>,
}

impl Census {
    /// Residue bytes per class: what the store attributes minus what the read path returned.
    ///
    /// The independent residue detector. Content no index entry names is invisible to
    /// [`ReferenceStore::read`] and visible to
    /// [`StoreAudit::storage_attribution`](continuum_workspace::publication::StoreAudit::storage_attribution),
    /// so the difference is exactly the unreachable content — derived without asking `fsck`
    /// for a defect list.
    fn residue_bytes(&self) -> BTreeMap<ArtifactClass, u64> {
        let mut residue = BTreeMap::new();
        for (class, attributed) in &self.attributed_bytes {
            let read = self.read_bytes.get(class).copied().unwrap_or(0);
            if *attributed > read {
                residue.insert(*class, attributed - read);
            }
        }
        residue
    }

    /// Total residue bytes across every class.
    fn residue_total(&self) -> u64 {
        self.residue_bytes().values().sum()
    }

    /// Whether the read path returned strictly fewer bytes than the store attributes, for
    /// every class — the arithmetic that must never go the other way.
    fn attribution_covers_every_read(&self) -> bool {
        self.read_bytes
            .iter()
            .all(|(class, read)| self.attributed_bytes.get(class).copied().unwrap_or(0) >= *read)
    }

    /// A canonical rendering: one fact per line, fixed section order, every list sorted.
    ///
    /// No clock, no counter and no insertion order, so two censuses of one store are equal
    /// iff they render equal.
    fn render(&self) -> String {
        let mut out = String::new();
        out.push_str("census 1\n");
        let _ = writeln!(out, "entries {}", self.entries.len());
        let _ = writeln!(out, "published-count {}", self.published_count);
        for handle in &self.entries {
            let _ = writeln!(out, "entry {handle}");
        }
        for (class, bytes) in &self.attributed_bytes {
            let _ = writeln!(
                out,
                "class {} attributed={} read={} residue={}",
                class.token(),
                bytes,
                self.read_bytes.get(class).copied().unwrap_or(0),
                self.residue_bytes().get(class).copied().unwrap_or(0)
            );
        }
        for (handle, count) in &self.receipts {
            let _ = writeln!(out, "receipts {handle} {count}");
        }
        for (task, claimed) in &self.claims {
            let _ = writeln!(out, "claim {task} {claimed}");
        }
        for finding in &self.findings {
            let _ = writeln!(out, "finding {}", finding.render());
        }
        out
    }
}

/// Take an independent census of `store`, checking `claims` against it.
///
/// Three read paths, in order, and each one is a different door into the same store:
/// the index (`identities`/`published_count`), the content (`read`, re-derived through
/// [`Blake3Identity`]), and the ledger (`receipts`). Nothing is repaired, nothing is written,
/// and the operator capability is the unscoped `cap_root`, so a denial from `read` means the
/// store cannot resolve the identity rather than that the caller may not see it — which is the
/// only reading that makes [`Finding::Unresolvable`] mean "stale index entry".
fn census(store: &ReferenceStore, claims: &[Claim]) -> Census {
    let capability = operator();
    let audit = store
        .audit_view(&capability)
        .expect("`cap_root` confers audit");

    let mut entries = audit.identities();
    entries.sort();
    let published_count = audit.published_count();

    let mut read_bytes: BTreeMap<ArtifactClass, u64> = BTreeMap::new();
    let mut receipts: BTreeMap<ArtifactHandle, u64> = BTreeMap::new();
    let mut findings = Vec::new();

    for handle in &entries {
        match store.read(handle, &capability) {
            Err(_) => findings.push(Finding::Unresolvable(handle.clone())),
            Ok(bytes) => {
                *read_bytes.entry(handle.class()).or_insert(0) +=
                    u64::try_from(bytes.len()).unwrap_or(u64::MAX);
                match ContentIdentifier::identify(&Blake3Identity, handle.class(), &bytes) {
                    Ok(derived) if &derived == handle => {}
                    Ok(derived) => findings.push(Finding::IdentityDisagreement {
                        filed: handle.clone(),
                        derived: derived.to_string(),
                    }),
                    Err(IdentityUnavailable) => findings.push(Finding::IdentityDisagreement {
                        filed: handle.clone(),
                        derived: "<unavailable>".to_owned(),
                    }),
                }
            }
        }

        let issued = audit.receipts(handle);
        if issued.is_empty() {
            findings.push(Finding::Receiptless(handle.clone()));
        } else {
            receipts.insert(
                handle.clone(),
                u64::try_from(issued.len()).unwrap_or(u64::MAX),
            );
        }
        if issued.iter().any(|receipt| receipt.handle() != handle) {
            findings.push(Finding::MisattributedReceipt(handle.clone()));
        }
    }

    let mut sorted_claims = claims.to_vec();
    sorted_claims.sort();
    for (task, claimed) in &sorted_claims {
        if store.read(claimed, &capability).is_err() {
            findings.push(Finding::OrphanTask {
                task: task.clone(),
                claimed: claimed.clone(),
            });
        }
    }

    findings.sort();
    Census {
        entries,
        published_count,
        read_bytes,
        attributed_bytes: audit.storage_attribution(),
        receipts,
        claims: sorted_claims,
        findings,
    }
}

/// Whether the store resolves `handle` through the ordinary read path.
///
/// The one primitive both halves of G1-08 are built on: an index entry is stale exactly when
/// this is false for an identity the index names, and a task is an orphan exactly when this is
/// false for an identity the task claims.
fn resolves(store: &ReferenceStore, handle: &ArtifactHandle) -> bool {
    store.read(handle, &operator()).is_ok()
}

/// Every publication every task in `daemon` claims, as store identities.
///
/// A publication's identity is the `commitment` on the record, not the task handle: the
/// commitment is what `budget::publication_record`'s bytes were named by, and it is the only
/// thing the store could have been asked to hold.
fn claims_of(daemon: &Daemon) -> Vec<Claim> {
    let mut claims = Vec::new();
    for task in daemon.state().tasks().handles() {
        let entry = daemon
            .state()
            .tasks()
            .get(task)
            .expect("a handle the table just listed");
        for publication in entry.evidence.committed() {
            let text = publication.commitment().as_str();
            let Some(identity) = text.strip_prefix(ArtifactClass::Task.prefix()) else {
                continue;
            };
            let Ok(handle) = ArtifactHandle::new(ArtifactClass::Task, identity) else {
                continue;
            };
            claims.push((task.as_str().to_owned(), handle));
        }
    }
    claims.sort();
    claims
}

// =============================================================================================
// the volatile-fact witness pin
// =============================================================================================

/// One thing the pre-crash daemon held for each [`VolatileFact`] this drive can reach.
///
/// A witness is a *name*, not a count: "the daemon still holds `ws_…`" is a stronger statement
/// than "the daemon holds some workspace", and it is the statement a restart has to falsify.
#[derive(Debug)]
struct Witnesses {
    capability: CapabilityHandle,
    staged: Commitment,
    model: Commitment,
    snapshot: WorkspaceHandle,
    lineage: ForkName,
    intent: IntentHandle,
    task: TaskHandle,
    continuation: Option<ContinuationHandle>,
    idempotency: (String, String),
}

/// Whether `daemon` still holds the witness for `fact`.
///
/// [`None`] when this drive produced no witness for the fact at all, so a pin over it would be
/// vacuous. The two such facts are reported by name in
/// [`derived_scope_every_declared_fact_that_had_a_witness_is_gone_after_the_restart`] rather
/// than silently counted as passes.
fn holds(daemon: &Daemon, fact: VolatileFact, witnesses: &Witnesses) -> Option<bool> {
    let state = daemon.state();
    Some(match fact {
        VolatileFact::WireCapabilityRegistry => state.grant(&witnesses.capability).is_some(),
        VolatileFact::StagedContent => state.staged(&witnesses.staged).is_some(),
        VolatileFact::ModelCatalog => state.models().get(&witnesses.model).is_some(),
        VolatileFact::WorkspaceRecords => state.workspace(&witnesses.snapshot).is_some(),
        VolatileFact::LineageMap => state.lineage(&witnesses.lineage).is_some(),
        VolatileFact::IntentRegistry => state.intent(&witnesses.intent).is_some(),
        // No operation reachable from this drive appends an evidence node or an evidence
        // event, so there is nothing to lose and nothing to pin. Reported, not counted.
        VolatileFact::EvidenceGraph | VolatileFact::EvidenceEventLog => return None,
        VolatileFact::IdempotencyLedger => state
            .replay(&witnesses.idempotency.0, &witnesses.idempotency.1)
            .is_some(),
        VolatileFact::AdmissionLog => !state.admissions().is_empty(),
        VolatileFact::TaskTable => state.tasks().get(&witnesses.task).is_some(),
        VolatileFact::ContinuationTable => witnesses
            .continuation
            .as_ref()
            .is_some_and(|handle| state.tasks().continuation(handle).is_some()),
        VolatileFact::RegionTree => state.regions().opened() > 0,
    })
}

// =============================================================================================
// seams
// =============================================================================================

/// A daemon that dies at exactly one dispatch boundary.
#[derive(Debug, Clone, Copy)]
struct KillAt(CrashPoint);

impl CrashInjector for KillAt {
    fn kills(&self, point: CrashPoint) -> bool {
        point == self.0
    }
}

/// Storage that refuses one check of one phase, and only while the test has armed it.
#[derive(Debug, Clone)]
struct ArmedAbort {
    phase: PublicationPhase,
    armed: Arc<AtomicBool>,
}

impl ArmedAbort {
    fn new(phase: PublicationPhase) -> Self {
        Self {
            phase,
            armed: Arc::new(AtomicBool::new(false)),
        }
    }

    fn arm(&self) {
        self.armed.store(true, Ordering::SeqCst);
    }
}

impl StorageFaults for ArmedAbort {
    fn check(&self, phase: PublicationPhase) -> Result<(), AbortReason> {
        if phase == self.phase && self.armed.swap(false, Ordering::SeqCst) {
            return Err(AbortReason::StorageExhausted);
        }
        Ok(())
    }
}

/// A *pure* identity seam that is not the declared one.
///
/// Not a corruption: FNV-1a is total, deterministic and process-independent, so a store built
/// over it is internally consistent and `fsck` — which asks the store's own identifier to
/// check the store's own content — finds nothing wrong. It is a **substituted seam**, which is
/// the defect class an fsck structurally cannot see and an independent census can: the daemon
/// declares `Blake3Identity` (ADR-0013), and bytes filed under an identity Blake3 does not
/// derive were named by something else.
#[derive(Debug, Clone, Copy)]
struct SubstitutedIdentity;

impl ContentIdentifier for SubstitutedIdentity {
    fn identify(
        &self,
        class: ArtifactClass,
        content: &[u8],
    ) -> Result<ArtifactHandle, IdentityUnavailable> {
        let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
        for byte in content {
            hash ^= u64::from(*byte);
            hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
        }
        ArtifactHandle::new(class, &format!("{hash:016x}")).map_err(|_| IdentityUnavailable)
    }
}

// =============================================================================================
// fixtures
// =============================================================================================

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

/// The unscoped `promote` capability every read and every audit view in this file goes through.
fn operator() -> CapabilityToken {
    continuumd::daemon::identity::capability_to_store(&cap("cap_root"))
        .expect("`cap_root` is a well-formed store token")
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

fn profile(privileged: &[&str]) -> CapabilityProfile {
    CapabilityProfile {
        privileged_operations: privileged.iter().map(|entry| name(entry)).collect(),
        denied_operations: Vec::new(),
        data_grants: Vec::new(),
        cross_principal_sharing: true,
    }
}

fn negotiated() -> Negotiated {
    let hello = ClientHello {
        protocol_versions: VersionRange {
            low: version(),
            high: version(),
        },
        encodings: vec![Encoding::CanonicalJson],
        client: "continuumd-g1-08-acceptance".to_owned(),
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

fn now() -> Timestamp {
    Timestamp::new("2026-08-01T00:00:00.000Z").expect("a well-formed timestamp")
}

/// The one builder a cold start and a restart both go through.
fn builder() -> Builder {
    let root = Some(cap("cap_root"));
    Daemon::builder(Blake3Identity, negotiated(), cap("cap_root"))
        .epochs(epochs())
        .now(now())
        .capability(
            grant(
                "cap_root",
                "service:continuumd",
                AuthorityLevel::Promote,
                4,
                Optional::Present(profile(&["intent.accept", "intent.reject", "intent.lock"])),
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
                "cap_steward",
                "human:steward",
                AuthorityLevel::ReviseIntent,
                3,
                Optional::Present(profile(&["intent.accept", "intent.reject", "intent.lock"])),
            ),
            root,
        )
        .family(WorkspaceFamily)
        .family(IntentFamily)
        .family(TaskFamily)
        .family(VerificationFamily)
}

fn daemon() -> Daemon {
    builder().build()
}

fn daemon_with(faults: impl StorageFaults + 'static) -> Daemon {
    builder().store_faults(faults).build()
}

/// The successor: a daemon built over what a crash left behind.
fn successor(durable: DurableSubstrate) -> Daemon {
    builder().over(durable).build()
}

/// A daemon with the intent accepted, the files staged and the model registered.
struct Prepared {
    daemon: Daemon,
    intent: IntentHandle,
    files: Vec<Commitment>,
    configuration: Commitment,
}

fn prepare(mut daemon: Daemon) -> Prepared {
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

    let mut files = Vec::new();
    for (path, content) in [(MODULE_PATH, DIE_HARD_MODEL), ("README.md", "# TV-009\n")] {
        files.push(
            daemon
                .state_mut()
                .stage(
                    &Blake3Identity,
                    WorkspacePath::new(path).expect("a workspace path"),
                    content.as_bytes().to_vec(),
                )
                .expect("staging names its content"),
        );
    }
    let configuration = daemon
        .state_mut()
        .stage(
            &Blake3Identity,
            WorkspacePath::new("default.model.toml").expect("a workspace path"),
            DIE_HARD_CONFIG.as_bytes().to_vec(),
        )
        .expect("staging names its content");

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
    assert_eq!(accepted.envelope.status, ResultStatus::Ok);

    Prepared {
        daemon,
        intent,
        files,
        configuration,
    }
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
    continuumd::daemon::identity::intent_to_wire(&stored).expect("an `in_` handle")
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

fn on(mut envelope: RequestEnvelope, snapshot: &WorkspaceHandle) -> RequestEnvelope {
    envelope.snapshot = Nullable::Value(snapshot.clone());
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

// --- driving --------------------------------------------------------------------------------

fn create_request(prepared: &Prepared, request: &str, key: &str) -> OperationRequest {
    OperationRequest {
        envelope: keyed(
            envelope("workspace.create", "agent:builder", "cap_builder", request),
            key,
        ),
        arguments: Arguments::WorkspaceCreate(WorkspaceCreateRequest {
            components: SnapshotComponents {
                files: prepared.files.clone(),
                cml_modules: Vec::new(),
                rust_extraction: Vec::new(),
                domain_packs: Vec::new(),
                dependencies: Vec::new(),
                epochs: SnapshotEpochs {
                    semantic: epoch("semantic-1"),
                    proof: epoch("proof-1"),
                    toolchain: Optional::Absent,
                },
                intent: prepared.intent.clone(),
                correspondence: Vec::new(),
                proof_environment: Vec::new(),
                configuration: vec![prepared.configuration.clone()],
                file_components: Optional::Absent,
            },
            overlay: Optional::Absent,
            seal: Optional::Present(true),
        }),
    }
}

fn seal(prepared: &mut Prepared, request: &str, key: &str) -> WorkspaceHandle {
    let created = prepared
        .daemon
        .dispatch(&create_request(prepared, request, key));
    assert_eq!(
        created.envelope.status,
        ResultStatus::Ok,
        "{:?}",
        created.envelope.error
    );
    match &created.payload {
        Payload::WorkspaceCreate(response) => response.snapshot.clone(),
        other => panic!("expected a workspace.create payload, got {other:?}"),
    }
}

fn start_request(
    snapshot: &WorkspaceHandle,
    request: &str,
    key: &str,
    states: u64,
) -> OperationRequest {
    let mut request_envelope = keyed(
        envelope("verification.start", "agent:runner", "cap_runner", request),
        key,
    );
    request_envelope.budget = Optional::Present(budget(states));
    OperationRequest {
        envelope: on(request_envelope, snapshot),
        arguments: Arguments::VerificationStart(VerificationStartRequest {
            target: Target {
                kind: TargetKind::AllClaims,
                id: "DieHard".to_owned(),
            },
            portfolio: Portfolio::Interactive,
            context_policy: Optional::Absent,
            priority_class: Optional::Absent,
        }),
    }
}

fn start(
    prepared: &mut Prepared,
    snapshot: &WorkspaceHandle,
    request: &str,
    key: &str,
    states: u64,
) -> continuumd::daemon::OperationOutcome {
    prepared
        .daemon
        .dispatch(&start_request(snapshot, request, key, states))
}

fn started_task(outcome: &continuumd::daemon::OperationOutcome) -> TaskHandle {
    match &outcome.payload {
        Payload::VerificationStart(response) => response
            .task
            .value()
            .cloned()
            .expect("a fresh start names a task"),
        other => panic!("expected a verification.start payload, got {other:?}"),
    }
}

/// The drive every crash test in this file uses: seal a snapshot, park a campaign on it.
///
/// Returns the prepared daemon plus the witnesses a restart has to falsify.
fn driven(mut prepared: Prepared) -> (Prepared, Witnesses) {
    let snapshot = seal(&mut prepared, "req_create", "idem-create");
    let outcome = start(&mut prepared, &snapshot, "req_start", "idem-start", 4);
    let task = started_task(&outcome);
    let entry = prepared
        .daemon
        .state()
        .tasks()
        .get(&task)
        .expect("the task is held");
    assert_eq!(
        entry.status,
        TaskStatus::Suspended,
        "a four-state ceiling parks the Die Hard campaign"
    );
    let continuation = entry.continuation.clone();
    let witnesses = Witnesses {
        capability: cap("cap_builder"),
        staged: prepared.configuration.clone(),
        model: die_hard_source(),
        snapshot: snapshot.clone(),
        lineage: ForkName::new(snapshot.as_str()).expect("a fork name"),
        intent: prepared.intent.clone(),
        task,
        continuation,
        idempotency: ("agent:builder".to_owned(), "idem-create".to_owned()),
    };
    (prepared, witnesses)
}

// =============================================================================================
// A. derived scope
// =============================================================================================

/// **The recovery surface that exists is store-shaped; the task half is declared lost.**
///
/// The inventory, mechanically: `recovery::recover`'s only input is a store and a capability —
/// there is no task table, no continuation, and no `Running`-task argument anywhere in the
/// recovery API — and the thirteen `VolatileFact`s partition exactly three `Reprovisioned`
/// against ten `Declared`, with the task and continuation tables in the second group. The
/// continuation table has no surviving store trace at all (`survives_as() == None`), which is
/// why plan §4.5's "resume from the last committed continuation" has no referent across a
/// restart.
///
/// This is the boundary of the derived scope, stated as assertions rather than prose.
#[test]
fn derived_scope_the_recovery_surface_is_store_shaped() {
    let (prepared, witnesses) = driven(prepare(daemon()));
    let durable = prepared.daemon.crash();

    // The whole recovery entry point, exercised: a store and a capability, and nothing else
    // is available to hand it. A signature that grew a task input would not compile here.
    let report = recovery::recover(durable.store(), &operator()).expect("`cap_root` audits");
    assert!(
        !report.volatile().is_empty(),
        "the report carries the volatile declaration"
    );

    let declared: Vec<&str> = VolatileFact::ALL
        .iter()
        .filter(|fact| fact.disposition() == Disposition::Declared)
        .map(|fact| fact.token())
        .collect();
    assert_eq!(
        declared,
        vec![
            "workspace-records",
            "lineage-map",
            "intent-registry",
            "evidence-graph",
            "evidence-event-log",
            "idempotency-ledger",
            "admission-log",
            "task-table",
            "continuation-table",
            "region-tree",
        ],
        "ten of thirteen facts are lost and named as lost"
    );
    assert_eq!(
        VolatileFact::ALL.len() - declared.len(),
        3,
        "the other three are the IDL §7 out-of-band surfaces"
    );
    assert_eq!(
        VolatileFact::ContinuationTable.survives_as(),
        None,
        "no part of a parked continuation reaches the store, so O2 has no referent"
    );
    assert_eq!(
        VolatileFact::TaskTable.survives_as(),
        Some(ArtifactClass::Task),
        "only the task's published evidence survives, never the record"
    );

    // And the vacuity is exhibited, not argued: the successor's task table is empty, so
    // "no orphan tasks" is true of a set with no members.
    let restarted = successor(durable);
    assert!(
        restarted.state().tasks().handles().is_empty(),
        "the successor holds no task at all"
    );
    assert!(
        restarted.state().tasks().get(&witnesses.task).is_none(),
        "including the one that was `Suspended` when the daemon died"
    );
}

/// **The loss declaration, pinned per fact, with the vacuous entries named.**
///
/// For each of the thirteen `VolatileFact`s this drive can witness: the pre-crash daemon holds
/// the witness, and the successor built over the surviving substrate does not. The pin is
/// two-sided on purpose — an assertion that the successor lacks something the original never
/// had would pass for the wrong reason — so the "held before" half is asserted first and the
/// facts with no witness are listed rather than counted.
///
/// This is the executable form of the debt: the day a disk-backed task table lands, the
/// `task-table` and `continuation-table` rows of this test fail, and that failure is the
/// signal that O2 became testable.
#[test]
fn derived_scope_every_declared_fact_that_had_a_witness_is_gone_after_the_restart() {
    let (prepared, witnesses) = driven(prepare(daemon()));

    let mut witnessed: Vec<VolatileFact> = Vec::new();
    let mut unwitnessed: Vec<&str> = Vec::new();
    for fact in VolatileFact::ALL {
        match holds(&prepared.daemon, fact, &witnesses) {
            None => unwitnessed.push(fact.token()),
            Some(true) => witnessed.push(fact),
            Some(false) => panic!(
                "no witness for `{fact}` before the crash — the pin over it would be vacuous"
            ),
        }
    }
    assert_eq!(
        unwitnessed,
        vec!["evidence-graph", "evidence-event-log"],
        "exactly two facts are unreachable from this drive, and they are named (INV-007)"
    );
    assert_eq!(
        witnessed.len(),
        11,
        "eleven of thirteen facts have a live witness before the crash"
    );

    let durable = prepared.daemon.crash();
    let restarted = successor(durable);

    // The pin: every `Declared` fact this drive witnessed is gone.
    let declared: Vec<VolatileFact> = witnessed
        .iter()
        .copied()
        .filter(|fact| fact.disposition() == Disposition::Declared)
        .collect();
    assert_eq!(
        declared.len(),
        8,
        "eight of the ten declared facts are witnessed here (the two named above are not)"
    );
    for fact in &declared {
        assert_eq!(
            holds(&restarted, *fact, &witnesses),
            Some(false),
            "`{fact}` survived a crash the daemon declares loses it"
        );
    }

    // `Reprovisioned` is a claim about the *deployment*, not about the restart, and the three
    // arrive by two different doors — so the bare successor is measured before anything is
    // restored, and the split is asserted rather than assumed. `Builder::capability` is part
    // of the startup path, so the wire registry is already back; content staging and model
    // registration go through `Daemon::state_mut` and are not.
    let already_back: Vec<&str> = VolatileFact::ALL
        .into_iter()
        .filter(|fact| fact.disposition() == Disposition::Reprovisioned)
        .filter(|fact| holds(&restarted, *fact, &witnesses) == Some(true))
        .map(VolatileFact::token)
        .collect();
    assert_eq!(
        already_back,
        vec!["wire-capability-registry"],
        "only the registry the builder itself provisions is back before the deployment acts"
    );

    // And after the deployment restores the out-of-band surface, all three hold again.
    let restored = prepare(restarted);
    for fact in VolatileFact::ALL
        .into_iter()
        .filter(|fact| fact.disposition() == Disposition::Reprovisioned)
    {
        assert_eq!(
            holds(&restored.daemon, fact, &witnesses),
            Some(true),
            "`{fact}` is the out-of-band surface a deployment restores, and it did not restore"
        );
    }
    // Six of the eight declared facts stay gone through the re-provisioning, and the split is
    // asserted exactly rather than asserted uniformly. The other two — the intent registry and
    // the admission log — come back, and the reason is worth naming because it is *not*
    // recovery: `prepare` is the deployment's own startup work, it re-registers the contract
    // through `Daemon::state_mut`, and the `intent.accept` it dispatches writes an admission
    // record. `Disposition` is a statement about what a *restart* reconstructs; a deployment
    // redoing its own startup beside it restores whatever that startup wrote the first time.
    // Nothing here rebuilds a fact from the store, which is the prohibition that matters.
    let rewritten_by_the_deployment = [VolatileFact::IntentRegistry, VolatileFact::AdmissionLog];
    for fact in &declared {
        assert_eq!(
            holds(&restored.daemon, *fact, &witnesses),
            Some(rewritten_by_the_deployment.contains(fact)),
            "`{fact}` disagreed with the deployment-rewrites-its-own-startup split"
        );
    }
    assert!(
        restored.daemon.state().tasks().handles().is_empty(),
        "and no amount of re-provisioning resurrects a task: that is the unbuilt half"
    );
}

// =============================================================================================
// B. the independent census over a crashed store
// =============================================================================================

/// **Independent census: no stale index entry, no orphan task, no half-published artifact.**
///
/// The daemon is dropped at the value grain — `Daemon::crash` consumes it, so `DaemonState`
/// and every one of its thirteen facts go with it — and the surviving store is censused from
/// primitives. Every index entry resolves through the ordinary read path, every one of those
/// reads re-derives to the identity it is filed under under `Blake3Identity`, every entry
/// carries at least one receipt, and every receipt names the identity it is filed under.
///
/// The task claims censused are the ones the *pre-crash* daemon made, carried across the drop
/// by value: this is the strongest available reading of "no orphan task" in a build with no
/// durable task record — the artifacts a task claimed before the crash must still be there
/// afterwards, even though the claimant is gone.
#[test]
fn the_independent_census_of_a_crashed_store_finds_no_stale_entry_and_no_orphan_task() {
    let (prepared, _) = driven(prepare(daemon()));
    let claims = claims_of(&prepared.daemon);
    assert_eq!(
        claims.len(),
        1,
        "the parked campaign committed exactly one publication"
    );

    let durable = prepared.daemon.crash();
    let taken = census(durable.store(), &claims);

    assert!(
        taken.findings.is_empty(),
        "independent census found: {:?}",
        taken
            .findings
            .iter()
            .map(Finding::render)
            .collect::<Vec<_>>()
    );
    assert!(
        taken.entries.len() > 1,
        "anti-vacuity: the census ran over a populated index, not an empty one ({} entries)",
        taken.entries.len()
    );
    assert_eq!(
        taken.residue_total(),
        0,
        "a daemon dropped between dispatches leaves nothing half-published"
    );
    assert!(
        taken.entries.contains(&claims[0].1),
        "the task's committed publication is one of the index entries"
    );
    assert!(
        taken
            .entries
            .iter()
            .any(|handle| handle.class() == ArtifactClass::WorkspaceSnapshot),
        "and the sealed snapshot's records are there too"
    );
}

/// **A daemon dropped mid-dispatch leaves a complete artifact that no task claims.**
///
/// The mid-state the criterion is really about, reached at the one dispatch boundary that can
/// have changed the store: the campaign-publishing handler ran to completion and the daemon
/// died before the replay record was written, so the store holds the campaign record and the
/// task that committed it went with `DaemonState`.
///
/// The census says exactly what that is, and the three answers are all different from each
/// other, which is why the distinction matters:
///
/// - it is **not** a stale index entry — the entry resolves, re-derives and is receipted;
/// - it is **not** residue — an index entry names it, so `collect_garbage` cannot touch it;
/// - it is **not** an orphan task — there is no task, which is O2's absence again, seen from
///   the other side: an artifact with no claimant rather than a claimant with no artifact.
///
/// This is the state a startup resolution pass would have to reconnect to a durable task
/// record, and the reason item 3 of the debt spec is a *pass* rather than a data structure.
/// The identity is predicted from a healthy twin so the census is not told the answer.
#[test]
fn a_daemon_dropped_mid_dispatch_leaves_a_complete_artifact_that_no_task_claims() {
    let (healthy, _) = driven(prepare(daemon()));
    let predicted = claims_of(&healthy.daemon);
    assert_eq!(predicted.len(), 1, "the twin names one campaign record");

    let mut killed = prepare(daemon());
    let snapshot = seal(&mut killed, "req_create", "idem-create");
    let outcome = killed.daemon.dispatch_or_die(
        &start_request(&snapshot, "req_start", "idem-start", 4),
        &KillAt(CrashPoint::AfterHandler),
    );
    let Err(dead) = outcome else {
        panic!("the injector kills after the handler");
    };
    assert_eq!(dead.point, CrashPoint::AfterHandler);
    assert!(
        dead.point.reaches_the_store(),
        "this is the only boundary at which the store can have changed"
    );

    let durable = killed.daemon.crash();
    let store = durable.store();

    // No claimant survives, so the census is run with an empty claim set — and separately with
    // the twin's claim, to show the artifact is there under exactly the name a task gave it.
    let unclaimed = census(store, &[]);
    assert!(unclaimed.findings.is_empty(), "{:?}", unclaimed.findings);
    assert_eq!(
        unclaimed.residue_total(),
        0,
        "the handler completed, so nothing was caught between the two commits"
    );
    assert!(
        unclaimed.entries.contains(&predicted[0].1),
        "the campaign record the dying dispatch published is an index entry"
    );
    assert!(
        resolves(store, &predicted[0].1),
        "and it resolves: a complete artifact, not a fragment"
    );
    assert_eq!(
        unclaimed.receipts.get(&predicted[0].1).copied(),
        Some(1),
        "with the receipt its publication issued"
    );

    let as_claimed = census(store, &predicted);
    assert!(
        as_claimed.findings.is_empty(),
        "the twin's claim is satisfiable against this store: {:?}",
        as_claimed.findings
    );

    // The claimant, on the other hand, is gone — and no successor recovers it.
    let restarted = successor(durable);
    assert!(
        restarted.state().tasks().handles().is_empty(),
        "the task that published it did not survive the dispatch it died in"
    );
    assert!(
        claims_of(&restarted).is_empty(),
        "so the store's `task_` entry is an artifact with no claimant (O2's absence)"
    );
}

/// **The three read paths reconcile exactly.**
///
/// Index, content and ledger are three doors into one store, and a store that is consistent
/// answers the same through all three. The identity of the counts is the check: as many index
/// entries as the store's own entry count, as many readable artifacts as index entries, as
/// many receipted identities as index entries, and total attributed bytes equal to the bytes
/// the read path returned plus the residue the arithmetic derives.
#[test]
fn the_three_read_paths_reconcile_exactly() {
    let (prepared, _) = driven(prepare(daemon()));
    let claims = claims_of(&prepared.daemon);
    let durable = prepared.daemon.crash();
    let store = durable.store();
    let taken = census(store, &claims);

    // Path A against the store's own count of the same thing.
    assert_eq!(
        taken.entries.len(),
        taken.published_count,
        "every index entry names a distinct identity"
    );
    let distinct: BTreeSet<&ArtifactHandle> = taken.entries.iter().collect();
    assert_eq!(distinct.len(), taken.entries.len(), "and no identity twice");

    // Path B: every entry dereferences.
    assert!(
        taken.entries.iter().all(|handle| resolves(store, handle)),
        "every index entry resolves through the ordinary read path"
    );

    // Path C: every entry is receipted, and the receipt map covers exactly the index.
    assert_eq!(
        taken.receipts.len(),
        taken.entries.len(),
        "the ledger names every published identity and no other"
    );
    assert!(
        taken.receipts.values().all(|count| *count >= 1),
        "and each with at least the receipt its publication issued"
    );

    // The arithmetic that ties B to the attribution, in the only direction it can run.
    assert!(
        taken.attribution_covers_every_read(),
        "the store cannot attribute fewer bytes than the read path returned"
    );
    let read_total: u64 = taken.read_bytes.values().sum();
    let attributed_total: u64 = taken.attributed_bytes.values().sum();
    assert_eq!(
        attributed_total,
        read_total + taken.residue_total(),
        "attribution = readable + residue, with no third bucket"
    );
    assert!(read_total > 0, "anti-vacuity: bytes were actually read");
}

/// **The independent census and the store's own fsck agree — clean and dirty.**
///
/// Two instruments, built separately, over the same two stores. On the undamaged store both
/// say nothing is wrong. On a store whose campaign record was caught between its two commits
/// both say the same thing about the same identity: `fsck` calls it
/// `StoreDefect::UnreachableContent` and the census's arithmetic prices it as residue in the
/// `task` class, and neither reports a stale index entry, because there is none.
///
/// Agreement is the result, not the method: the census never calls `fsck`.
#[test]
fn the_independent_census_and_the_stores_own_fsck_agree() {
    // Clean.
    let (prepared, _) = driven(prepare(daemon()));
    let claims = claims_of(&prepared.daemon);
    let durable = prepared.daemon.crash();
    let clean = census(durable.store(), &claims);
    let clean_defects = durable
        .store()
        .audit_view(&operator())
        .expect("`cap_root` audits")
        .fsck();
    assert!(clean.findings.is_empty());
    assert!(clean_defects.is_empty(), "{clean_defects:?}");
    assert_eq!(clean.residue_total(), 0);

    // Dirty: the campaign record's index commit is refused, so its content is durable and
    // nothing names it. The identity is predicted from a healthy twin rather than read off
    // the injured daemon, so the census is not told the answer it is checking.
    let (healthy, _) = driven(prepare(daemon()));
    let predicted = claims_of(&healthy.daemon);
    assert_eq!(predicted.len(), 1);

    let fault = ArmedAbort::new(PublicationPhase::CommittingIndex);
    let mut injured = prepare(daemon_with(fault.clone()));
    let snapshot = seal(&mut injured, "req_create", "idem-create");
    fault.arm();
    let refused = start(&mut injured, &snapshot, "req_start", "idem-start", 4);
    assert_eq!(
        refused.envelope.status,
        ResultStatus::Error,
        "the caller is told the publication aborted rather than handed a task"
    );
    assert_eq!(
        refused.envelope.error.value().map(|error| error.code),
        Some(ErrorCode::PublicationAborted)
    );
    assert!(
        claims_of(&injured.daemon).is_empty(),
        "and the task claims nothing, because nothing committed"
    );

    let durable = injured.daemon.crash();
    let dirty = census(durable.store(), &predicted);
    let dirty_defects = durable
        .store()
        .audit_view(&operator())
        .expect("`cap_root` audits")
        .fsck();

    // The census, alone: residue exists, it is `task`-class, and the predicted identity is
    // exactly what does not resolve.
    assert!(dirty.residue_total() > 0, "the aborted record is residue");
    assert_eq!(
        dirty.residue_bytes().keys().copied().collect::<Vec<_>>(),
        vec![ArtifactClass::Task],
        "and it is priced to the class it belongs to"
    );
    assert!(
        !resolves(durable.store(), &predicted[0].1),
        "the identity the campaign record would have had is unreachable"
    );
    assert!(
        !dirty.entries.contains(&predicted[0].1),
        "no index entry names it — this is INV-017's asymmetry, not a stale entry"
    );
    assert!(
        dirty
            .findings
            .iter()
            .all(|finding| !matches!(finding, Finding::Unresolvable(_))),
        "and every entry the index *does* name still resolves"
    );

    // fsck, separately: the same identity, in its own vocabulary.
    let unreachable: Vec<&ArtifactHandle> = dirty_defects
        .iter()
        .filter_map(|defect| match defect {
            StoreDefect::UnreachableContent(handle) => Some(handle),
            _ => None,
        })
        .collect();
    assert_eq!(unreachable, vec![&predicted[0].1], "{dirty_defects:?}");
    assert!(
        dirty_defects
            .iter()
            .all(|defect| matches!(defect, StoreDefect::UnreachableContent(_))),
        "and fsck finds no stale entry and no corruption either: {dirty_defects:?}"
    );
}

/// **The successor adopts the store and does not change it.**
///
/// A restart that silently rewrote what it inherited would make every recovery claim above a
/// claim about the restart rather than about the crash. So the census is taken twice: once
/// over the substrate before anything is built on it, and once over the same store as the
/// successor holds it. The two render byte-identically.
///
/// The successor is then driven again — a second seal, under a fresh request id — and the
/// census grows without any earlier entry moving: recovery adds, and never rewrites.
#[test]
fn the_successor_adopts_the_store_without_changing_it() {
    let (prepared, _) = driven(prepare(daemon()));
    let claims = claims_of(&prepared.daemon);
    let durable = prepared.daemon.crash();
    let before = census(durable.store(), &claims);

    let restarted = successor(durable);
    let after = census(restarted.store(), &claims);
    assert_eq!(
        before.render(),
        after.render(),
        "building a daemon over a store changed the store"
    );

    let mut redriven = prepare(restarted);
    let _ = seal(&mut redriven, "req_recreate", "idem-recreate");
    let later = census(redriven.daemon.store(), &claims);
    assert!(later.findings.is_empty(), "{:?}", later.findings);
    for handle in &before.entries {
        assert!(
            later.entries.contains(handle),
            "the retry dropped an entry that had already landed: {handle}"
        );
    }
    assert_eq!(
        later.residue_total(),
        0,
        "a converging retry leaves no new residue"
    );
}

// =============================================================================================
// C. the within-lifetime consistency floor
// =============================================================================================

/// **The reachable analog: after a mix of served, refused and aborted operations, the census
/// is still exact.**
///
/// Not crash recovery — this is one daemon lifetime, and it is labelled as the floor that O2's
/// recovery would have to restore. The mix is deliberately adversarial for an index: one
/// successful seal, one operation refused for an unregistered capability, one refused for an
/// unknown operation, one campaign publication aborted between its two commits, one served
/// campaign, and one cancel. After all of it, every index entry still resolves, every entry is
/// still receipted, the only residue is the aborted publication, and the daemon's own region
/// accounting reports no orphan worker and no unfinalized scope.
#[test]
fn after_a_refused_and_aborted_operation_mix_the_within_lifetime_census_is_exact() {
    let fault = ArmedAbort::new(PublicationPhase::CommittingIndex);
    let mut prepared = prepare(daemon_with(fault.clone()));

    // served
    let snapshot = seal(&mut prepared, "req_create", "idem-create");
    let baseline = census(prepared.daemon.store(), &[]);
    assert!(baseline.findings.is_empty());
    assert_eq!(baseline.residue_total(), 0);

    // refused: a capability nobody registered
    let ghost = prepared.daemon.dispatch(&OperationRequest {
        envelope: keyed(
            envelope("workspace.create", "agent:ghost", "cap_ghost", "req_ghost"),
            "idem-ghost",
        ),
        arguments: Arguments::WorkspaceCreate(
            match create_request(&prepared, "req_ghost", "idem-ghost").arguments {
                Arguments::WorkspaceCreate(request) => request,
                _ => unreachable!("just built"),
            },
        ),
    });
    assert_eq!(ghost.envelope.status, ResultStatus::Error);

    // refused: a task nobody started
    let phantom = prepared.daemon.dispatch(&OperationRequest {
        envelope: keyed(
            envelope("task.status", "agent:runner", "cap_runner", "req_phantom"),
            "idem-phantom",
        ),
        arguments: Arguments::TaskStatus(TaskStatusRequest {
            task: TaskHandle::new(
                "task_0000000000000000000000000000000000000000000000000000000000000000",
            )
            .expect("a well-formed task handle"),
        }),
    });
    assert_eq!(phantom.envelope.status, ResultStatus::Error);

    // aborted: the campaign record's index commit is refused
    fault.arm();
    let aborted = start(&mut prepared, &snapshot, "req_abort", "idem-abort", 4);
    assert_eq!(aborted.envelope.status, ResultStatus::Error);
    assert_eq!(
        aborted.envelope.error.value().map(|error| error.code),
        Some(ErrorCode::PublicationAborted)
    );

    // served: a second campaign over the same snapshot, unimpeded. The ceiling differs from
    // the aborted one so the two campaigns are two tasks and two records: a task identity is
    // content-derived, and two starts under one ceiling name one task.
    let outcome = start(&mut prepared, &snapshot, "req_start", "idem-start", 8);
    assert_ne!(
        outcome.envelope.status,
        ResultStatus::Error,
        "{:?}",
        outcome.envelope.error
    );
    let task = started_task(&outcome);

    // cancelled
    let cancelled = prepared.daemon.dispatch(&OperationRequest {
        envelope: keyed(
            envelope("task.cancel", "agent:runner", "cap_runner", "req_cancel"),
            "idem-cancel",
        ),
        arguments: Arguments::TaskCancel(TaskCancelRequest { task: task.clone() }),
    });
    assert_ne!(
        cancelled.envelope.status,
        ResultStatus::Error,
        "{:?}",
        cancelled.envelope.error
    );

    let claims = claims_of(&prepared.daemon);
    let taken = census(prepared.daemon.store(), &claims);

    assert!(
        taken.findings.is_empty(),
        "within-lifetime census found: {:?}",
        taken
            .findings
            .iter()
            .map(Finding::render)
            .collect::<Vec<_>>()
    );
    assert!(
        taken.entries.len() > baseline.entries.len(),
        "anti-vacuity: the mix added published artifacts: baseline={:?} taken={:?} claims={:?}",
        baseline.entries,
        taken.entries,
        claims
    );
    assert_eq!(
        taken.residue_bytes().keys().copied().collect::<Vec<_>>(),
        vec![ArtifactClass::Task],
        "the only residue is the publication the abort caught"
    );

    // The daemon's own independent accounting of the same "no orphan" question, one layer
    // down: the region tree's worker states, not the reports it handed back.
    let regions = prepared.daemon.state().regions();
    assert!(regions.orphans().is_empty(), "{:?}", regions.orphans());
    assert!(
        regions.unfinalized().is_empty(),
        "{:?}",
        regions.unfinalized()
    );
    assert!(regions.defects().is_empty(), "{:?}", regions.defects());
    assert_eq!(
        regions.opened() as usize,
        regions.finalizations().len(),
        "every scope opened for task work was torn down"
    );
}

/// **No task claims an artifact the store does not hold, across the mix.**
///
/// The orphan-task detector run positively, over every task the daemon holds and every
/// publication each of them claims. The anti-vacuity guard matters more here than anywhere
/// else in the file: a detector with nothing to check passes for free, so the claim set is
/// asserted non-empty and asserted to name identities that are genuinely in the index.
#[test]
fn no_task_claims_an_artifact_the_store_does_not_hold_across_the_mix() {
    let mut prepared = prepare(daemon());
    let snapshot = seal(&mut prepared, "req_create", "idem-create");
    let first = started_task(&start(
        &mut prepared,
        &snapshot,
        "req_start_a",
        "idem-start-a",
        4,
    ));
    let second = started_task(&start(
        &mut prepared,
        &snapshot,
        "req_start_b",
        "idem-start-b",
        8,
    ));
    assert_ne!(first, second, "two campaigns, two tasks");

    let claims = claims_of(&prepared.daemon);
    assert!(
        claims.len() >= 2,
        "anti-vacuity: at least one claim per task ({claims:?})"
    );

    let store = prepared.daemon.store();
    let taken = census(store, &claims);
    assert!(taken.findings.is_empty(), "{:?}", taken.findings);
    for (task, claimed) in &claims {
        assert!(
            resolves(store, claimed),
            "task {task} claims {claimed}, which the store does not hold"
        );
        assert!(
            taken.entries.contains(claimed),
            "and the claim is an index entry, not merely readable content"
        );
    }
}

// =============================================================================================
// D. negative controls
// =============================================================================================

/// **Negative control — a planted orphan task is detected, and the withheld mirror is clean.**
///
/// The plant goes through the out-of-band administration seam (`Daemon::state_mut`), which is
/// the only door to a task record that does not go through a handler: a publication is staged
/// and committed onto a live task under a `task_*` commitment whose bytes were never published.
/// That is precisely an orphan task — a task claiming an artifact the store does not hold — and
/// it is the state O2's recovery would have to reap.
///
/// The mirror is the same daemon, the same drive, without the plant. Both halves are asserted,
/// so a detector that always fires and a detector that never fires both fail this test.
#[test]
fn negative_control_a_planted_orphan_task_is_detected() {
    // Mirror: no plant.
    let (mirror, _) = driven(prepare(daemon()));
    let mirror_claims = claims_of(&mirror.daemon);
    let mirror_census = census(mirror.daemon.store(), &mirror_claims);
    assert!(
        mirror_census.findings.is_empty(),
        "the unplanted mirror is clean: {:?}",
        mirror_census.findings
    );
    assert_eq!(mirror_claims.len(), 1);

    // Planted.
    let (mut planted, witnesses) = driven(prepare(daemon()));
    let checkpoint = *planted
        .daemon
        .state()
        .tasks()
        .get(&witnesses.task)
        .expect("the task is held")
        .evidence
        .committed()
        .first()
        .expect("the parked run committed one publication")
        .checkpoint();

    let fabricated = ArtifactHandle::new(
        ArtifactClass::Task,
        "0000000000000000000000000000000000000000000000000000000000000000",
    )
    .expect("a well-formed task identity");
    assert!(
        !resolves(planted.daemon.store(), &fabricated),
        "the fabricated identity names nothing the store holds"
    );

    let entry = planted
        .daemon
        .state_mut()
        .tasks_mut()
        .get_mut(&witnesses.task)
        .expect("the task is held");
    let evidence: &mut Publications = &mut entry.evidence;
    evidence.stage(Commitment::new(&fabricated.to_string()));
    assert!(
        evidence.commit(checkpoint).is_some(),
        "the plant landed on the record"
    );

    let claims = claims_of(&planted.daemon);
    assert_eq!(claims.len(), 2, "one real claim and one fabricated one");
    let taken = census(planted.daemon.store(), &claims);
    assert_eq!(
        taken.findings,
        vec![Finding::OrphanTask {
            task: witnesses.task.as_str().to_owned(),
            claimed: fabricated.clone(),
        }],
        "the census reports exactly the planted orphan and nothing else"
    );

    // And the plant is invisible to the store's own audit, which is the point of checking the
    // claim side at all: `fsck` verifies content against the index and knows nothing about
    // what a task believes.
    let defects = planted
        .daemon
        .store()
        .audit_view(&operator())
        .expect("`cap_root` audits")
        .fsck();
    assert!(
        defects.is_empty(),
        "an orphan task is not a store defect: {defects:?}"
    );
}

/// **Negative control — residue is detected by arithmetic alone, and the mirror is zero.**
///
/// The residue detector never consults `fsck`: it subtracts the bytes the read path returned
/// from the bytes the store attributes, per class. With the index commit refused the difference
/// is positive and lands in the `task` class; with the fault withheld the difference is exactly
/// zero in every class. Two-sided, and derived from two independent readings of one store.
#[test]
fn negative_control_residue_is_detected_by_arithmetic_alone() {
    // Mirror: no fault.
    let (mirror, _) = driven(prepare(daemon()));
    let mirror_census = census(mirror.daemon.store(), &[]);
    assert_eq!(
        mirror_census.residue_total(),
        0,
        "no fault, no residue: {:?}",
        mirror_census.residue_bytes()
    );
    assert!(mirror_census.read_bytes.values().sum::<u64>() > 0);

    // Planted.
    let fault = ArmedAbort::new(PublicationPhase::CommittingIndex);
    let mut injured = prepare(daemon_with(fault.clone()));
    let snapshot = seal(&mut injured, "req_create", "idem-create");
    let before = census(injured.daemon.store(), &[]);
    assert_eq!(before.residue_total(), 0, "the seal itself left nothing");

    fault.arm();
    let refused = start(&mut injured, &snapshot, "req_start", "idem-start", 4);
    assert_eq!(refused.envelope.status, ResultStatus::Error);

    let after = census(injured.daemon.store(), &[]);
    assert!(
        after.residue_total() > 0,
        "the aborted publication's bytes are attributed and unreadable"
    );
    assert_eq!(
        after.residue_bytes().keys().copied().collect::<Vec<_>>(),
        vec![ArtifactClass::Task]
    );
    assert_eq!(
        after.entries, before.entries,
        "and the index did not grow: residue is content without an entry"
    );
    assert!(
        after.findings.is_empty(),
        "residue is not a finding — it is the price INV-017 pays to never write a stale entry"
    );
}

/// **Negative control — a substituted identity seam is caught by the census and missed by
/// fsck.**
///
/// `StoreAudit::fsck` re-derives each artifact's identity with the store's *own* identifier,
/// so a store whose identifier is internally consistent but is not the declared one passes its
/// own audit. The census re-derives with `Blake3Identity` — the seam `Daemon::builder` is
/// always handed in this tree — and therefore sees what `fsck` cannot.
///
/// Both stores are built here, over the same bytes, and the assertions run both ways: the
/// Blake3 store is clean under both instruments; the substituted store is clean under `fsck`
/// and flagged, entry for entry, by the census. This is the one place the census is strictly
/// stronger than the delivered recovery pass, and it is a statement about the *instrument*,
/// not a defect report against this build — every daemon in this tree supplies `Blake3Identity`.
#[test]
fn negative_control_a_substituted_identity_seam_is_caught_by_the_census_and_missed_by_fsck() {
    /// One store, one capability, the same three payloads.
    fn stocked(identifier: impl ContentIdentifier + 'static) -> ReferenceStore {
        let token = operator();
        let store = ReferenceStore::builder(identifier, AuditLog::new())
            .capability(continuum_workspace::publication::CapabilityDescriptor::new(
                token.clone(),
                StoreActor::new("service:continuumd"),
                StoreLevel::Promote,
            ))
            .build();
        for payload in [b"alpha".as_slice(), b"beta".as_slice(), b"gamma".as_slice()] {
            store
                .publish(ArtifactClass::Task, payload.to_vec(), &token)
                .expect("the operator may publish a `task_` artifact");
        }
        store
    }

    // Mirror: the declared seam.
    let honest = stocked(Blake3Identity);
    let honest_census = census(&honest, &[]);
    let honest_defects = honest
        .audit_view(&operator())
        .expect("`cap_root` audits")
        .fsck();
    assert_eq!(honest_census.entries.len(), 3);
    assert!(
        honest_census.findings.is_empty(),
        "{:?}",
        honest_census.findings
    );
    assert!(honest_defects.is_empty(), "{honest_defects:?}");

    // Substituted: pure, self-consistent, and not Blake3.
    let substituted = stocked(SubstitutedIdentity);
    let substituted_census = census(&substituted, &[]);
    let substituted_defects = substituted
        .audit_view(&operator())
        .expect("`cap_root` audits")
        .fsck();

    assert!(
        substituted_defects.is_empty(),
        "fsck asks the substituted seam to check itself, so it finds nothing: {substituted_defects:?}"
    );
    assert_eq!(
        substituted_census.entries.len(),
        3,
        "the store is populated and every entry resolves"
    );
    assert_eq!(
        substituted_census.findings.len(),
        3,
        "the census flags every entry: {:?}",
        substituted_census
            .findings
            .iter()
            .map(Finding::render)
            .collect::<Vec<_>>()
    );
    assert!(
        substituted_census
            .findings
            .iter()
            .all(|finding| matches!(finding, Finding::IdentityDisagreement { .. })),
        "and it flags them for the right reason"
    );
    assert!(
        substituted_census
            .findings
            .iter()
            .all(|finding| !matches!(finding, Finding::Unresolvable(_))),
        "the entries are readable; it is their names that disagree"
    );
    assert_eq!(
        substituted_census.residue_total(),
        0,
        "and there is no residue: this is a naming defect, not a lost write"
    );
}

/// **Negative control — the resolution primitive is two-sided, and a stale index entry is
/// unconstructible.**
///
/// `StoreDefect::MissingReferent` cannot be planted through this store's public API: content
/// is committed before the index, pinned while in flight, and removed only by
/// `collect_garbage`, whose root set is the index itself. So the first noun of G1-08 is
/// defended by construction, and what a control can show is that the *detector* would fire:
/// [`resolves`] is false for an identity the store does not hold and true for every identity
/// the index names.
///
/// The second half runs the only removal path the store has, against a populated index, and
/// shows it reclaims nothing reachable — which is the assertion that the one seam that could
/// create a stale entry does not.
#[test]
fn negative_control_the_resolution_primitive_is_two_sided() {
    let (prepared, _) = driven(prepare(daemon()));
    let store = prepared.daemon.store();
    let taken = census(store, &[]);
    assert!(taken.entries.len() > 1, "anti-vacuity");

    // True side: every index entry.
    for handle in &taken.entries {
        assert!(
            resolves(store, handle),
            "{handle} is indexed and unreadable"
        );
    }
    // False side: a well-formed identity of the same class that names nothing.
    for class in [ArtifactClass::Task, ArtifactClass::WorkspaceSnapshot] {
        let absent = ArtifactHandle::new(
            class,
            "1111111111111111111111111111111111111111111111111111111111111111",
        )
        .expect("a well-formed identity");
        assert!(
            !taken.entries.contains(&absent),
            "the fixture must not have published this identity"
        );
        assert!(
            !resolves(store, &absent),
            "the detector reported an identity the store does not hold as resolvable"
        );
    }

    // The only removal path, against a populated index.
    let reclaimed = store
        .collect_garbage(&operator())
        .expect("`cap_root` administers");
    assert!(
        reclaimed.is_empty(),
        "nothing was unreachable, so nothing was reclaimed: {reclaimed:?}"
    );
    let after = census(store, &[]);
    assert!(
        after.findings.is_empty(),
        "garbage collection cannot create a stale index entry: {:?}",
        after.findings
    );
    assert_eq!(
        after.render(),
        taken.render(),
        "and it changed nothing at all"
    );
}

// =============================================================================================
// E. determinism
// =============================================================================================

/// **Two censuses of one crashed store render byte-identically.**
///
/// The census must be a function of the store and of nothing else — no clock, no counter, no
/// iteration order that depends on insertion history — or none of the comparisons above mean
/// anything. Asserted on non-trivial bytes, so a census that rendered a constant would not pass.
#[test]
fn two_censuses_of_one_crashed_store_render_byte_identically() {
    let (prepared, _) = driven(prepare(daemon()));
    let claims = claims_of(&prepared.daemon);
    let durable = prepared.daemon.crash();

    let first = census(durable.store(), &claims).render();
    let second = census(durable.store(), &claims).render();
    assert_eq!(first, second);
    assert!(
        first.contains("published-count")
            && first.contains("claim task_")
            && first.contains("entry ws_"),
        "anti-vacuity: the rendering carries real content:\n{first}"
    );
}

/// **Two independently crashed daemons census identically.**
///
/// The two-fresh-daemons device applied to the census: two daemons, built and driven
/// separately, dropped separately, produce the same accounting of their two separate stores.
/// A census that leaked anything process-local — an address, an insertion order, a counter —
/// would differ here.
#[test]
fn two_independently_crashed_daemons_census_identically() {
    let mut renders = Vec::new();
    for _ in 0..2 {
        let (prepared, _) = driven(prepare(daemon()));
        let claims = claims_of(&prepared.daemon);
        let durable = prepared.daemon.crash();
        renders.push(census(durable.store(), &claims).render());
    }
    assert_eq!(renders[0], renders[1]);
    assert!(
        renders[0].lines().count() > 6,
        "anti-vacuity: the rendering is not a stub:\n{}",
        renders[0]
    );
}
