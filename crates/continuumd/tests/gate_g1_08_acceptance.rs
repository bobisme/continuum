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
//! | **O2** | "On restart, `Running` tasks resume from their last committed continuation or transition to `Failed` with a typed reason — never to a silently reconstructed state." | **built, both branches (bn-1z09m, bn-20142).** A park commits a continuation record (`cont_`) with the continuation's pins and the task fields a resume reads. `recovery::resolve_tasks` runs at every restart and resolves each task the store holds a campaign record for to `Settled`, to `Restored` — a live task and continuation read back from the matching continuation record — or to `Failed` with a typed `FailureReason`. |
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
//! - **"no orphan tasks"** is O2. Before bn-1z09m the successor's task table was *empty*,
//!   so the property held only of an empty set. Now the startup pass derives the task set
//!   from the durable `task_*` and `cont_*` records and resolves each member to a typed
//!   outcome, restoring a parked task live
//!   ([`derived_scope_the_startup_pass_restores_the_parked_task_from_the_store`]), and a
//!   daemon killed at `AfterHandler` has its record reconnected to a claimant
//!   ([`a_daemon_dropped_mid_dispatch_has_its_complete_artifact_reconnected_by_the_restart`]).
//!   The loss pin over the remaining `Declared` facts stays
//!   ([`derived_scope_every_declared_fact_that_had_a_witness_is_gone_after_the_restart`]).
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
//! Actionable, in the shape `bn-3dr` left it and this file measures. Status after bn-1z09m
//! and bn-20142 is given per item.
//!
//! 1. **A durable task record.** `TaskEntry` — at minimum `handle`, `operation`, `status`,
//!    `snapshot`, `intent`, `epochs`, the ledger's checkpoints, and `Publications::committed`
//!    — written before the operation that changes it answers. **Closed for a parked task
//!    (bn-20142).** The continuation record carries every one of those fields plus the
//!    target, portfolio, priority, model source, ceilings, spend and milestones, and it is
//!    written before the park, `task.update_budget` or `task.cancel` answers. The restored
//!    `TaskRecord` is the pre-crash one byte for byte, and `task.status` answers it
//!    ([`a_restored_task_answers_task_status_with_its_pre_crash_record`]). **Still open for a
//!    task that closed:** a closing run publishes no continuation record, so a `Settled` task
//!    has no durable terminal record and `task.status` on it answers `CapabilityDenied` until
//!    a re-issued start re-runs it. What is missing is exactly a terminal record — status,
//!    report and final ledger — for the `Completed` arm. Not durable either, by declaration:
//!    `TaskEntry::events` (hints, `rule subscription.hints_only`) and the engine's
//!    `CheckReport`, which the next resume re-derives.
//! 2. **A durable continuation.** **Closed (bn-20142).** `VolatileFact::ContinuationTable` is
//!    `Resolved` and `survives_as() == Some(ArtifactClass::Continuation)`. The record is an
//!    artifact of the existing `cont_` class, content-addressed, with the pin preimage the
//!    `cont_*` handle is the identity of; the pass re-derives the handle through the
//!    declared seam. The crash matrices resume a restored continuation to the byte-identical
//!    cold record at every boundary (`tests/task_lifecycle_schedule_matrix.rs`), and the
//!    decision table admits it exactly as it admitted the live one
//!    ([`a_restored_continuation_is_admitted_exactly_when_the_live_one_was`]).
//! 3. **A startup resolution pass.** For every durable record in a non-terminal state:
//!    resume it under its pinned epochs, or write `Failed` with a typed `ErrorCode`. **Built,
//!    both branches.** `recovery::resolve_tasks` runs in `Builder::build` on every restart. A
//!    `bounded` head with a matching continuation record resolves to `Restored` at the record's
//!    highest revision; one with none to `Failed(ContinuationNotDurable)`
//!    ([`a_parked_head_without_a_continuation_record_still_resolves_to_failed`]); two records
//!    at one highest revision to `Failed(AmbiguousContinuation)`; a `closed` head to
//!    `Settled`; an unsound history to `Failed(SequenceGap)` or `Failed(AmbiguousHead)`.
//! 4. **An orphan reaper with something to reap.** The census in this file *is* the reaper's
//!    detector, and [`negative_control_a_planted_orphan_task_is_detected`] shows it fires. It
//!    had nothing to run against after a restart because there were no tasks. **Now has a
//!    subject**: the successor's live and resolved tasks claim every surviving record, and the
//!    census runs over those claims. A continuation record no task claims — one published
//!    before a campaign record that then aborted — is reported by the pass as unclaimed.
//! 5. **A crate-boundary decision.** Plan §20 gives `continuumd` no storage edge beyond
//!    `continuum-workspace`, so 1–3 need either a task artifact class in that store or a new
//!    edge. This is a dossier decision, not an implementation detail. **Unchanged, and not
//!    needed.** The pass reads the existing `task` and `cont` classes of the one store edge.
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
//!    the daemon's *declared* ADR-0013 seam. `fsck` (bn-k99dt) now takes that same declared
//!    seam as an explicit argument rather than the store's own configured identifier, so a
//!    substituted seam is caught by both instruments
//!    ([`regression_a_substituted_identity_seam_is_caught_by_fsck_against_the_declared_seam`]);
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
//! - **"no orphan tasks" — SATISFIED at a narrowed scope (bn-1z09m, bn-20142).** Every
//!   `task_*` record in the store has a claimant after a restart, and every task with a
//!   non-terminal head is restored from its committed continuation or is `Failed` with a
//!   typed reason. The detector fires
//!   ([`negative_control_a_planted_orphan_task_is_detected`]), and the pass is two-sided
//!   ([`negative_control_a_restart_that_cannot_audit_resolves_nothing_and_says_so`]). Since
//!   bn-20142 a parked task comes back live from its continuation record and resumes. The
//!   narrowing: a `Settled` task is not visible through `task.status` (debt item 1: no
//!   durable terminal record), and the crash grain is as stated for the first conjunct.
//!
//! # Four findings this re-derivation produced that the delivering evidence does not carry
//!
//! 1. **`fsck` could not see a substituted identity seam (fixed, bn-k99dt).** It used to
//!    re-derive with the store's own identifier, so a store whose identifier was pure,
//!    self-consistent and *not* the declared `Blake3Identity` passed its own audit with every
//!    entry misnamed — only the census, pinned to the declared seam, caught it. `fsck` now
//!    takes the declared identifier as an explicit argument, so it catches the same substitution
//!    the census does
//!    ([`regression_a_substituted_identity_seam_is_caught_by_fsck_against_the_declared_seam`]).
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
//!    complete, resolvable, receipted `task_*` record that no surviving task claimed. It was
//!    neither a stale entry nor residue nor an orphan task — it was O2's absence seen from the
//!    other side. Since bn-1z09m the startup pass reconnects it: the successor resolves the
//!    task that published it, under the handle a healthy twin names. Since bn-20142 it
//!    restores that task live, under the continuation handle the twin minted.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

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
use continuumd::daemon::continuation::ParkState;
use continuumd::daemon::family::{Arguments, Payload};
use continuumd::daemon::identity::Blake3Identity;
use continuumd::daemon::intent::IntentFamily;
use continuumd::daemon::recovery::{
    self, CrashInjector, CrashPoint, Disposition, FailureReason, Resolution, Startup, VolatileFact,
};
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
use continuumd::protocol::operations::task::{
    TaskCancelRequest, TaskResumeRequest, TaskStatusRequest, TaskUpdateBudgetRequest,
};
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
    // A task the startup resolution pass resolved claims every record it committed. This is
    // the successor's half: a resolved task is not a live entry, and it still claims.
    for resolved in daemon.state().tasks().resolved() {
        for handle in resolved.claims() {
            claims.push((resolved.task.as_str().to_owned(), handle));
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
///
/// Armed by ordinal from the arming: since bn-20142 a park publishes its continuation record
/// and then its campaign record, so a test aiming at the campaign record's index commit arms
/// the second check of that phase ([`CAMPAIGN_RECORD`]).
#[derive(Debug, Clone)]
struct ArmedAbort {
    phase: PublicationPhase,
    /// The matching checks left until the fault fires; zero is disarmed.
    countdown: Arc<AtomicU64>,
}

/// The ordinal of a park's campaign record among its publications: the continuation record
/// is first, the campaign record second.
const CAMPAIGN_RECORD: u64 = 2;

impl ArmedAbort {
    fn new(phase: PublicationPhase) -> Self {
        Self {
            phase,
            countdown: Arc::new(AtomicU64::new(0)),
        }
    }

    /// Fire at the `nth` check of the phase from now, counting from one.
    fn arm_at(&self, nth: u64) {
        self.countdown.store(nth, Ordering::SeqCst);
    }
}

impl StorageFaults for ArmedAbort {
    fn check(&self, phase: PublicationPhase) -> Result<(), AbortReason> {
        if phase != self.phase {
            return Ok(());
        }
        let fired = self
            .countdown
            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |left| {
                left.checked_sub(1)
            })
            .is_ok_and(|left| left == 1);
        if fired {
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

/// A store written before continuation records existed: a parked campaign record and no
/// continuation record (bn-20142).
///
/// The production daemon cannot write this any more — a park publishes its continuation
/// record before its campaign record, so a parked head always has one — so the store is
/// planted: a healthy twin runs [`driven`], and its campaign record's bytes are published
/// into a fresh daemon's store under the operator capability, beside the same sealed
/// snapshot. The witnesses are the twin's, and they name the planted task.
fn legacy(mut prepared: Prepared) -> (Prepared, Witnesses) {
    let (twin, witnesses) = driven(prepare(daemon()));
    let claims = claims_of(&twin.daemon);
    assert_eq!(claims.len(), 1, "the twin committed one campaign record");
    let bytes = twin
        .daemon
        .store()
        .read(&claims[0].1, &operator())
        .expect("`cap_root` reads the twin's record");
    let snapshot = seal(&mut prepared, "req_create", "idem-create");
    assert_eq!(
        snapshot, witnesses.snapshot,
        "the same inputs seal the same snapshot"
    );
    let receipt = prepared
        .daemon
        .store()
        .stage(ArtifactClass::Task, bytes, &operator())
        .expect("`cap_root` publishes")
        .commit_content()
        .expect("nothing is injured")
        .commit_index()
        .expect("nothing is injured");
    assert_eq!(
        receipt.handle(),
        &claims[0].1,
        "the planted record is the twin's"
    );
    (prepared, witnesses)
}

// =============================================================================================
// A. derived scope
// =============================================================================================

/// **The startup pass restores the parked task from the store, and nothing else comes back.**
///
/// Regression guard for bn-1z09m and bn-20142 (plan §4.5 O2). Before bn-1z09m the task table
/// was `Declared` lost and the successor's task table was empty, so "no orphan tasks" held
/// only of an empty set. bn-1z09m resolved the parked task to
/// `Failed(ContinuationNotDurable)`; bn-20142 made the continuation durable. This test now
/// guards the resume branch:
///
/// - the store-shaped fsck entry point is unchanged, and beside it `resolve_tasks` also takes
///   only a store and a capability. The task set is derived from durable records, never
///   from a volatile table;
/// - eight facts stay `Declared`. The task table and the continuation table are `Resolved`,
///   and `survives_as()` names the class each is read back from;
/// - the successor holds the parked task **live**, restored from its continuation record,
///   with the pre-crash `TaskRecord` byte for byte, and claims exactly what it committed;
/// - `Daemon::startup` is exactly what `resolve_tasks` derives from the store alone, and it
///   says `Restored(Suspended)`.
#[test]
fn derived_scope_the_startup_pass_restores_the_parked_task_from_the_store() {
    let (prepared, witnesses) = driven(prepare(daemon()));
    let committed = claims_of(&prepared.daemon);
    assert_eq!(
        committed.len(),
        1,
        "the parked campaign committed one record"
    );
    let before = prepared
        .daemon
        .state()
        .tasks()
        .get(&witnesses.task)
        .expect("held")
        .record();
    let durable = prepared.daemon.crash();

    // The fsck entry point, unchanged: a store and a capability.
    let report = recovery::recover(durable.store(), &operator()).expect("`cap_root` audits");
    assert!(
        !report.volatile().is_empty(),
        "the report carries the volatile declaration"
    );
    // The resolution entry point: also a store and a capability, and nothing volatile.
    let derived = recovery::resolve_tasks(durable.store(), &operator()).expect("`cap_root` audits");

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
            "region-tree",
        ],
        "eight of thirteen facts are lost and named as lost"
    );
    let resolved: Vec<&str> = VolatileFact::ALL
        .iter()
        .filter(|fact| fact.disposition() == Disposition::Resolved)
        .map(|fact| fact.token())
        .collect();
    assert_eq!(
        resolved,
        vec!["task-table", "continuation-table"],
        "the task and continuation tables are resolved at startup"
    );
    assert_eq!(
        VolatileFact::ALL.len() - declared.len() - resolved.len(),
        3,
        "the other three are the IDL §7 out-of-band surfaces"
    );
    assert_eq!(
        VolatileFact::ContinuationTable.survives_as(),
        Some(ArtifactClass::Continuation),
        "the continuation survives as its own record, pins included"
    );
    assert_eq!(
        VolatileFact::TaskTable.survives_as(),
        Some(ArtifactClass::Task),
        "the task's durable trace is its published campaign records"
    );

    let restarted = successor(durable);
    assert_eq!(
        restarted.state().tasks().handles(),
        vec![&witnesses.task],
        "exactly the parked task comes back live"
    );
    assert!(
        restarted
            .state()
            .tasks()
            .resolution(&witnesses.task)
            .is_none(),
        "a restored task is live, not in the resolved set"
    );
    assert_eq!(
        restarted
            .state()
            .tasks()
            .get(&witnesses.task)
            .expect("live")
            .record(),
        before,
        "every field of the restored record is read back from the continuation record"
    );
    let Some(resolution) = derived
        .tasks()
        .iter()
        .find(|resolved| resolved.task == witnesses.task)
    else {
        panic!("the pass resolves the task that was `Suspended` when the daemon died");
    };
    assert_eq!(
        resolution.resolution,
        Resolution::Restored(ParkState::Suspended)
    );
    assert_eq!(
        claims_of(&restarted),
        committed,
        "the successor claims exactly what the pre-crash task committed"
    );
    assert_eq!(
        restarted.startup(),
        &Startup::Resolved(derived),
        "the startup pass is the store-only derivation, not something the daemon remembered"
    );
}

/// **The failure branch stays: a parked head with no continuation record is `Failed`.**
///
/// The store is [`legacy`] — the campaign record and no continuation record, as a store
/// written before bn-20142 is. The pass resolves the task to
/// `Failed(ContinuationNotDurable)` and no live entry comes back: without the pins, resuming
/// would take them from the successor's configuration, which is inference.
#[test]
fn a_parked_head_without_a_continuation_record_still_resolves_to_failed() {
    let (prepared, witnesses) = legacy(prepare(daemon()));
    let restarted = successor(prepared.daemon.crash());
    assert!(
        restarted.state().tasks().get(&witnesses.task).is_none(),
        "no live entry comes back"
    );
    assert_eq!(
        restarted
            .state()
            .tasks()
            .resolution(&witnesses.task)
            .map(|resolved| resolved.resolution),
        Some(Resolution::Failed(FailureReason::ContinuationNotDurable))
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
/// This was the executable form of the debt. bn-1z09m flipped its `task-table` row, and
/// bn-20142 flipped its `continuation-table` row — the signal this pin was kept for: O2's
/// resume branch is reachable. Both tables are `Resolved`, so they leave the declared set,
/// and the test asserts the split exactly: every declared witness is gone, and the two
/// resolved witnesses — the live task and its continuation — are back, read from the
/// continuation record.
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
        6,
        "six of the eight declared facts are witnessed here (the two named above are not)"
    );
    for fact in &declared {
        assert_eq!(
            holds(&restarted, *fact, &witnesses),
            Some(false),
            "`{fact}` survived a crash the daemon declares loses it"
        );
    }

    // The resolved facts: the task and its continuation are back, from the durable records.
    for fact in [VolatileFact::TaskTable, VolatileFact::ContinuationTable] {
        assert_eq!(
            holds(&restarted, fact, &witnesses),
            Some(true),
            "`{fact}` is resolved at startup from the continuation record"
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
    // Four of the six declared facts stay gone through the re-provisioning, and the split is
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
    assert_eq!(
        restored.daemon.state().tasks().handles(),
        vec![&witnesses.task],
        "the deployment's own startup work adds no task and loses the restored one"
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

/// **A daemon dropped mid-dispatch has its complete artifact reconnected by the restart.**
///
/// The mid-state the criterion is really about, reached at the one dispatch boundary that can
/// have changed the store: the campaign-publishing handler ran to completion and the daemon
/// died before the replay record was written, so the store holds the campaign record and the
/// task that committed it went with `DaemonState`.
///
/// The census says exactly what that record is, and the answers differ from each other,
/// which is why the distinction matters:
///
/// - it is **not** a stale index entry — the entry resolves, re-derives and is receipted;
/// - it is **not** residue — an index entry names it, so `collect_garbage` cannot touch it;
/// - before the restart it has **no claimant** — O2's absence seen from the other side.
///
/// This test pinned that absence until bn-1z09m. It now guards the fix: the successor's
/// startup pass reads the record and the continuation record the same handler published,
/// and restores the task that published them — under exactly the handle a healthy twin
/// names — live and `Suspended` (bn-20142). The restored task claims the record. The
/// identity is predicted from the twin so the census is not told the answer.
#[test]
fn a_daemon_dropped_mid_dispatch_has_its_complete_artifact_reconnected_by_the_restart() {
    let (healthy, twin) = driven(prepare(daemon()));
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

    // No claimant survives the crash itself, so the census is run with an empty claim set —
    // and separately with the twin's claim, to show the artifact is there under exactly the
    // name a task gave it.
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

    // The restart reconnects it.
    let restarted = successor(durable);
    assert_eq!(
        claims_of(&restarted),
        predicted,
        "the successor claims the record, under the task handle the healthy twin names"
    );
    assert_eq!(
        restarted
            .state()
            .tasks()
            .get(&twin.task)
            .map(|entry| entry.status),
        Some(TaskStatus::Suspended),
        "the task is restored from its committed continuation record, never guessed"
    );
    assert_eq!(
        restarted
            .state()
            .tasks()
            .get(&twin.task)
            .and_then(|entry| entry.continuation.clone()),
        twin.continuation,
        "under the continuation handle the healthy twin minted"
    );
    let reconnected = census(restarted.store(), &claims_of(&restarted));
    assert!(
        reconnected.findings.is_empty(),
        "every successor claim resolves: {:?}",
        reconnected.findings
    );
    let task_entries: Vec<&ArtifactHandle> = reconnected
        .entries
        .iter()
        .filter(|handle| handle.class() == ArtifactClass::Task)
        .collect();
    let claimed: BTreeSet<&ArtifactHandle> = reconnected.claims.iter().map(|(_, h)| h).collect();
    assert!(
        task_entries.iter().all(|handle| claimed.contains(handle)),
        "no `task_` record in the store is left without a claimant"
    );
}

/// The successor's reading of `task` through `task.status`.
fn status_of(daemon: &mut Daemon, task: &TaskHandle, request: &str) -> Option<TaskStatus> {
    let answer = daemon.dispatch(&OperationRequest {
        envelope: envelope("task.status", "agent:runner", "cap_runner", request),
        arguments: Arguments::TaskStatus(TaskStatusRequest { task: task.clone() }),
    });
    match answer.payload {
        Payload::TaskStatus(record) => Some(record.status),
        _ => {
            assert_eq!(answer.error_code(), Some(ErrorCode::CapabilityDenied));
            None
        }
    }
}

/// **A re-issued start after a restart leaves one state per handle: `Failed` stays final.**
///
/// Regression guard for the lead's bn-1z09m review. RFC 0026 "Task lifecycle": "a terminal
/// status never changes", and a terminal, non-`Completed` identity is answered on the
/// observing lane at `ok`, running nothing (F20, paid at 3.4). A task the startup pass
/// resolved to `Failed` is such an identity. Since bn-20142 only a [`legacy`] store holds
/// one: a store this build writes restores the task instead
/// ([`a_restart_then_the_same_start_answers_the_restored_task_on_its_suspended_lane`]). So the identical `verification.start` after the
/// restart answers that lane and creates **no** live entry beside the resolution: the table,
/// `task.status` and `claims_of` all read the one resolved state.
#[test]
fn a_restart_then_the_same_start_leaves_one_state_for_a_failed_resolution() {
    // A `Failed` resolution needs a parked head with no continuation record, which only a
    // store written before bn-20142 holds.
    let (prepared, witnesses) = legacy(prepare(daemon()));
    let mut again = prepare(successor(prepared.daemon.crash()));
    let committed = claims_of(&again.daemon);
    assert_eq!(
        committed.len(),
        1,
        "the resolution claims the planted record"
    );
    let snapshot = seal(&mut again, "req_create", "idem-create");
    assert_eq!(
        snapshot, witnesses.snapshot,
        "the same inputs name the same snapshot"
    );

    let answer = start(&mut again, &snapshot, "req_start", "idem-start", 4);
    assert_eq!(
        answer.envelope.status,
        ResultStatus::Ok,
        "a terminal identity is observed, not started: {:?}",
        answer.envelope.error
    );
    assert_eq!(answer.envelope.task.value(), Some(&witnesses.task));
    match &answer.payload {
        Payload::VerificationStart(response) => {
            assert_eq!(response.task.value(), Some(&witnesses.task));
            assert!(response.result.value().is_none(), "no result to reuse");
        }
        other => panic!("expected a verification.start payload, got {other:?}"),
    }

    let tasks = again.daemon.state().tasks();
    assert!(
        tasks.get(&witnesses.task).is_none(),
        "no live entry was created"
    );
    assert_eq!(
        tasks
            .resolution(&witnesses.task)
            .map(|resolved| resolved.resolution),
        Some(Resolution::Failed(FailureReason::ContinuationNotDurable)),
        "the resolution is unchanged"
    );
    assert_eq!(
        claims_of(&again.daemon),
        committed,
        "one claimant, one record"
    );
    assert_eq!(
        status_of(&mut again.daemon, &witnesses.task, "req_status"),
        None,
        "`task.status` has no live record to project, as before the re-issued start"
    );
}

/// **A re-issued start after a restart leaves one state per handle: `Settled` is superseded.**
///
/// The other arm. A `Settled` task reached `Completed` before the crash and its report was
/// not durable, so the cached-result lane has nothing to return. Re-running a completed
/// identity "could only produce the same answer" (`verification::start`), so the start
/// removes the resolution explicitly and runs. Afterwards the live entry is the only state:
/// `Completed`, claiming the same record the resolution claimed, with no duplicate claim.
#[test]
fn a_restart_then_the_same_start_supersedes_a_settled_resolution() {
    let mut prepared = prepare(daemon());
    let snapshot = seal(&mut prepared, "req_create", "idem-create");
    let task = started_task(&start(
        &mut prepared,
        &snapshot,
        "req_start",
        "idem-start",
        64,
    ));
    assert_eq!(
        prepared
            .daemon
            .state()
            .tasks()
            .get(&task)
            .map(|entry| entry.status),
        Some(TaskStatus::Completed),
        "a 64-state ceiling closes the Die Hard campaign"
    );
    let committed = claims_of(&prepared.daemon);

    let restarted = successor(prepared.daemon.crash());
    assert_eq!(
        restarted
            .state()
            .tasks()
            .resolution(&task)
            .map(|resolved| resolved.resolution),
        Some(Resolution::Settled)
    );
    let mut again = prepare(restarted);
    let snapshot = seal(&mut again, "req_create", "idem-create");
    let answer = start(&mut again, &snapshot, "req_start", "idem-start", 64);
    assert_eq!(
        answer.envelope.status,
        ResultStatus::TaskStarted,
        "{:?}",
        answer.envelope.error
    );
    assert_eq!(
        started_task(&answer),
        task,
        "the same inputs name the same task"
    );

    let tasks = again.daemon.state().tasks();
    assert!(
        tasks.resolution(&task).is_none(),
        "the resolution was superseded"
    );
    assert_eq!(
        tasks.get(&task).map(|entry| entry.status),
        Some(TaskStatus::Completed)
    );
    assert_eq!(
        claims_of(&again.daemon),
        committed,
        "the re-run republished the same record, and it is claimed once"
    );
    assert_eq!(
        status_of(&mut again.daemon, &task, "req_status"),
        Some(TaskStatus::Completed)
    );
}

// =============================================================================================
// A'. the resume branch of O2 (bn-20142)
// =============================================================================================

/// `task.resume` on `continuation`, under `states`, as `agent:runner`.
fn resume_request(continuation: &ContinuationHandle, states: u64, tag: &str) -> OperationRequest {
    let mut request = keyed(
        envelope(
            "task.resume",
            "agent:runner",
            "cap_runner",
            &format!("req_{tag}"),
        ),
        &format!("idem-{tag}"),
    );
    request.budget = Optional::Present(budget(states));
    OperationRequest {
        envelope: request,
        arguments: Arguments::TaskResume(TaskResumeRequest {
            continuation: continuation.clone(),
            budget: Optional::Present(budget(states)),
        }),
    }
}

/// **`task.status` answers a restored task from durable data** (bn-20142, the bone's third
/// item). The successor answers the pre-crash `TaskRecord` byte for byte — operation,
/// epochs, budget, cost, priority, milestones, continuation — none of it taken from the
/// successor's configuration, all of it read from the continuation record.
///
/// What still answers `CapabilityDenied` is a task resolved to `Settled` or `Failed`: a
/// closing run publishes no continuation record, so its terminal `TaskRecord` fields are not
/// durable ([`a_restart_then_the_same_start_supersedes_a_settled_resolution`] reads that arm).
#[test]
fn a_restored_task_answers_task_status_with_its_pre_crash_record() {
    let (prepared, witnesses) = driven(prepare(daemon()));
    let before = prepared
        .daemon
        .state()
        .tasks()
        .get(&witnesses.task)
        .expect("held")
        .record();
    let mut restarted = successor(prepared.daemon.crash());
    let answer = restarted.dispatch(&OperationRequest {
        envelope: envelope("task.status", "agent:runner", "cap_runner", "req_status"),
        arguments: Arguments::TaskStatus(TaskStatusRequest {
            task: witnesses.task.clone(),
        }),
    });
    match &answer.payload {
        Payload::TaskStatus(record) => assert_eq!(record, &before),
        other => panic!("expected a task.status payload, got {other:?}: {answer:?}"),
    }
}

/// **The restored continuation is admitted exactly when the live one was.**
///
/// The resume decision table (RFC 0026) applied to a restored continuation, row by row
/// where a restart can move the input:
///
/// - the deployment has not re-sealed the snapshot: `CapabilityDenied`, the "pinned snapshot
///   is not held" row;
/// - the successor serves a different intent epoch: `ContinuationEpochMismatch` (P1);
/// - the successor serves a different engine identity: `ContinuationEpochMismatch` (P2);
/// - all inputs as before the crash: admitted.
///
/// The model row is not driven here: the deployment's own startup work (`prepare`)
/// registers the model with the content it stages, so a restart cannot move that input
/// without also moving the snapshot.
#[test]
fn a_restored_continuation_is_admitted_exactly_when_the_live_one_was() {
    let run = |configure: &dyn Fn(Builder) -> Builder, reseal: bool| {
        let (prepared, witnesses) = driven(prepare(daemon()));
        let continuation = witnesses.continuation.clone().expect("parked");
        let mut restarted = configure(builder()).over(prepared.daemon.crash()).build();
        if reseal {
            let mut again = prepare(restarted);
            seal(&mut again, "req_create", "idem-create");
            restarted = again.daemon;
        }
        restarted
            .dispatch(&resume_request(&continuation, 64, "resume"))
            .error_code()
    };
    let same = |builder: Builder| builder;
    // The intent epoch, not the semantic one: the snapshot's own components pin the
    // semantic epoch, so a successor serving another one cannot re-seal the snapshot at all,
    // and the resume would stop at the snapshot row.
    let advanced_intent = |builder: Builder| {
        let mut epochs = epochs();
        epochs.intent = Nullable::Value(epoch("intent-2"));
        builder.epochs(epochs)
    };
    let other_engine = |builder: Builder| {
        let mut epochs = epochs();
        epochs.engine = Nullable::Value(epoch("engine-reference-2"));
        builder.epochs(epochs)
    };

    assert_eq!(
        run(&same, false),
        Some(ErrorCode::CapabilityDenied),
        "the pinned snapshot is not held until the deployment re-seals it"
    );
    assert_eq!(
        run(&advanced_intent, true),
        Some(ErrorCode::ContinuationEpochMismatch),
        "P1: an intent-epoch advance refuses the restored continuation"
    );
    assert_eq!(
        run(&other_engine, true),
        Some(ErrorCode::ContinuationEpochMismatch),
        "P2: another engine identity refuses the restored continuation"
    );
    assert_eq!(run(&same, true), None, "all inputs as before: admitted");
}

/// **A restart then the same start answers the restored task on its suspended lane.**
///
/// The re-issued `verification.start` names a live, parked identity, so it answers
/// `task_suspended` with the restored continuation — the answer the pre-crash daemon gave —
/// runs nothing, and creates no second state.
#[test]
fn a_restart_then_the_same_start_answers_the_restored_task_on_its_suspended_lane() {
    let (prepared, witnesses) = driven(prepare(daemon()));
    let committed = claims_of(&prepared.daemon);
    let mut again = prepare(successor(prepared.daemon.crash()));
    let snapshot = seal(&mut again, "req_create", "idem-create");
    let opened = again.daemon.state().regions().opened();
    let answer = start(&mut again, &snapshot, "req_start", "idem-start", 4);
    assert_eq!(
        answer.envelope.status,
        ResultStatus::TaskSuspended,
        "{:?}",
        answer.envelope.error
    );
    assert_eq!(answer.envelope.task.value(), Some(&witnesses.task));
    assert_eq!(
        answer.envelope.continuation.value(),
        witnesses.continuation.as_ref()
    );
    assert_eq!(
        again.daemon.state().regions().opened(),
        opened,
        "nothing ran"
    );
    assert_eq!(
        claims_of(&again.daemon),
        committed,
        "one claimant, one record"
    );
}

/// **A cancel before the crash is a cancel after it** (`rule task.status_monotonic` across a
/// restart). The cancel of a parked task publishes the `cancelled` revision of its
/// continuation record, so the successor restores the task `Cancelled` — never `Suspended`
/// again — with its pre-crash record, and a resume answers the terminal status and writes
/// nothing. A budget update before the crash is likewise durable.
#[test]
fn a_cancel_and_a_budget_update_before_the_crash_survive_it() {
    let (mut prepared, witnesses) = driven(prepare(daemon()));
    let updated = prepared.daemon.dispatch(&OperationRequest {
        envelope: keyed(
            envelope(
                "task.update_budget",
                "agent:runner",
                "cap_runner",
                "req_update",
            ),
            "idem-update",
        ),
        arguments: Arguments::TaskUpdateBudget(TaskUpdateBudgetRequest {
            task: witnesses.task.clone(),
            budget: budget(12),
        }),
    });
    assert_eq!(updated.error_code(), None, "{:?}", updated.envelope.error);
    let cancelled = prepared.daemon.dispatch(&OperationRequest {
        envelope: keyed(
            envelope("task.cancel", "agent:runner", "cap_runner", "req_cancel"),
            "idem-cancel",
        ),
        arguments: Arguments::TaskCancel(TaskCancelRequest {
            task: witnesses.task.clone(),
        }),
    });
    assert_eq!(
        cancelled.error_code(),
        None,
        "{:?}",
        cancelled.envelope.error
    );
    let before = prepared
        .daemon
        .state()
        .tasks()
        .get(&witnesses.task)
        .expect("held")
        .record();
    assert_eq!(before.status, TaskStatus::Cancelled);
    assert_eq!(before.budget.states.value(), Some(&12));

    let restarted = successor(prepared.daemon.crash());
    let Startup::Resolved(resolution) = restarted.startup() else {
        panic!("`cap_root` audits");
    };
    assert_eq!(
        resolution
            .tasks()
            .iter()
            .find(|resolved| resolved.task == witnesses.task)
            .map(|resolved| resolved.resolution),
        Some(Resolution::Restored(ParkState::Cancelled))
    );
    assert_eq!(
        restarted
            .state()
            .tasks()
            .get(&witnesses.task)
            .expect("restored")
            .record(),
        before,
        "the cancelled record, ceiling and milestones included, is the pre-crash one"
    );

    let mut again = prepare(restarted);
    seal(&mut again, "req_create", "idem-create");
    let continuation = witnesses.continuation.clone().expect("parked");
    let answer = again
        .daemon
        .dispatch(&resume_request(&continuation, 64, "resume"));
    assert_eq!(answer.error_code(), None, "{:?}", answer.envelope.error);
    assert_eq!(
        again
            .daemon
            .state()
            .tasks()
            .get(&witnesses.task)
            .expect("held")
            .record(),
        before,
        "a resume of a cancelled task answers the terminal status and writes nothing"
    );
}

/// **Negative control — a restart that cannot audit the store resolves nothing, and says so.**
///
/// The pass is two-sided. Over the same kind of crashed store, a successor whose connection
/// capability is `cap_builder` (`propose`, no audit) cannot run it. The result is the typed
/// `Startup::Refused`, not an empty `Resolved` that would read as "nothing to resolve". The
/// mirror, with `cap_root`, resolves the task.
#[test]
fn negative_control_a_restart_that_cannot_audit_resolves_nothing_and_says_so() {
    let (prepared, witnesses) = driven(prepare(daemon()));
    let durable = prepared.daemon.crash();
    let refused = Daemon::builder(Blake3Identity, negotiated(), cap("cap_builder"))
        .epochs(epochs())
        .now(now())
        .over(durable)
        .build();
    assert!(
        matches!(refused.startup(), Startup::Refused(_)),
        "a successor that cannot audit reports the refusal: {:?}",
        refused.startup()
    );
    assert!(
        refused
            .state()
            .tasks()
            .resolution(&witnesses.task)
            .is_none(),
        "and resolves nothing"
    );

    assert!(
        refused.state().tasks().get(&witnesses.task).is_none(),
        "and restores nothing"
    );

    let (mirror, mirror_witnesses) = driven(prepare(daemon()));
    let restarted = successor(mirror.daemon.crash());
    assert!(matches!(restarted.startup(), Startup::Resolved(_)));
    assert!(
        restarted
            .state()
            .tasks()
            .get(&mirror_witnesses.task)
            .is_some(),
        "the mirror restores the task"
    );
    assert!(
        matches!(daemon().startup(), Startup::Cold),
        "a cold start is its own typed value"
    );
}

/// **Two restarts over one kind of crash resolve byte-identically.**
///
/// The two-fresh-daemons device applied to the resolution pass: nothing in it reads a clock,
/// draws entropy, or depends on insertion order.
#[test]
fn two_independently_restarted_daemons_resolve_identically() {
    let mut renders = Vec::new();
    for _ in 0..2 {
        let (prepared, _) = driven(prepare(daemon()));
        let restarted = successor(prepared.daemon.crash());
        let Startup::Resolved(resolution) = restarted.startup() else {
            panic!("`cap_root` audits");
        };
        renders.push(resolution.render());
    }
    assert_eq!(renders[0], renders[1]);
    assert!(
        renders[0].contains("restored-suspended")
            && renders[0].contains("record 0 task_")
            && renders[0].contains("restored cont_"),
        "anti-vacuity: the rendering names a restored task, its record and its continuation:\n{}",
        renders[0]
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
        .fsck(&Blake3Identity);
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
    fault.arm_at(CAMPAIGN_RECORD);
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
        .fsck(&Blake3Identity);

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
    fault.arm_at(CAMPAIGN_RECORD);
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
        .fsck(&Blake3Identity);
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

    fault.arm_at(CAMPAIGN_RECORD);
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
    let grown: Vec<&ArtifactHandle> = after
        .entries
        .iter()
        .filter(|handle| !before.entries.contains(handle))
        .collect();
    assert!(
        grown
            .iter()
            .all(|handle| handle.class() == ArtifactClass::Continuation)
            && grown.len() == 1,
        "the index grew by the continuation record published before the aborted commit \
         point, and by nothing of the residue's class: residue is content without an entry \
         ({grown:?})"
    );
    assert!(
        after.findings.is_empty(),
        "residue is not a finding — it is the price INV-017 pays to never write a stale entry"
    );
}

/// **Regression — a substituted identity seam is caught by fsck against the declared seam.**
///
/// `StoreAudit::fsck` used to re-derive each artifact's identity with the store's *own*
/// identifier, so a store whose identifier was internally consistent but not the declared one
/// passed its own audit with every entry misnamed — the census, re-derived with
/// `Blake3Identity`, caught what `fsck` could not. `fsck` now takes the declared identifier as
/// an explicit argument instead of trusting `self.store.identifier`, so it is checked here
/// against `Blake3Identity` — the seam `Daemon::builder` is always handed in this tree — and
/// the gap is closed: both instruments now agree.
///
/// Both stores are built here, over the same bytes, and the assertions run both ways: the
/// Blake3 store is clean under both instruments; the substituted store is flagged, entry for
/// entry, by both. This is the flip side of the bug this test used to pin — it now guards
/// against `fsck` regressing back to auditing a store with itself.
#[test]
fn regression_a_substituted_identity_seam_is_caught_by_fsck_against_the_declared_seam() {
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
        .fsck(&Blake3Identity);
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
        .fsck(&Blake3Identity);

    let mismatched: Vec<&ArtifactHandle> = substituted_defects
        .iter()
        .filter_map(|defect| match defect {
            StoreDefect::IdentityMismatch(handle) => Some(handle),
            _ => None,
        })
        .collect();
    assert_eq!(
        mismatched.len(),
        3,
        "fsck, checked against the declared seam, flags every entry: {substituted_defects:?}"
    );
    assert_eq!(
        substituted_defects.len(),
        3,
        "and flags nothing else: {substituted_defects:?}"
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
