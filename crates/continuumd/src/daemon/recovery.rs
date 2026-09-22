//! What survives a daemon crash, what does not, and the typed report a restart owes
//! (G1, plan §4.5, docs/35 "Crash safety and the index verifier").
//!
//! > On restart, `Running` tasks resume from their last committed continuation or transition
//! > to `Failed` with a typed reason — never to a silently reconstructed state. An index
//! > verifier (fsck) ships with the daemon.
//! >
//! > — `notes/plan/plan.md` §4.5
//!
//! # The durable/volatile split, stated before anything is built on it
//!
//! This daemon is single-threaded ([`Daemon::dispatch`](super::Daemon::dispatch) takes
//! `&mut self`), draws no ambient anything, and has no disk. A "crash" at this grain is
//! therefore exactly one event: **the process's [`DaemonState`](super::state::DaemonState) is
//! lost mid-dispatch, and what survives is whatever the durable layer holds.** The durable
//! layer is [`ReferenceStore`] — `continuum-workspace`'s publication store, which is the
//! semantic reference docs/35 makes `continuumd` reproduce and whose own crash safety is
//! already evidenced (G0-DX-13's falsification-and-fix cycle: content commits before the
//! index, an in-flight publication pins its content as a collection root from the content
//! commit until the index commit, and a receipt is appended in the *same* critical section as
//! the index entry naming it).
//!
//! So the split is:
//!
//! | | survives a crash | does not |
//! |---|---|---|
//! | **store** | committed content, the index, the receipt ledger, the abort log, the store's own capability registry | — |
//! | **daemon** | — | every field of `DaemonState`: the task table and its continuations, the idempotency ledger, the lineage map, the workspace/intent/evidence records, the admission log, staged content, the region tree, the model catalog |
//!
//! [`VolatileFact`] is that right-hand column as a value, so the declaration is something a
//! report *carries* rather than something a reader has to be told.
//!
//! # What recovery can therefore promise, per column
//!
//! - **no stale index entry.** An index entry naming content the store does not hold is
//!   unconstructible through the store's own API — that is the content-before-index ordering
//!   plus the root pin — and [`recover`] checks it anyway rather than asserting it, because
//!   an fsck that trusts the thing it verifies verifies nothing. A finding is
//!   [`StoreDefect::MissingReferent`], reported and never repaired.
//! - **no half-published artifact.** A *composite* artifact — a sealed workspace is N records
//!   published children before parents — has its root published last, so a crash part-way
//!   through leaves the root absent and the composite unreadable under its own name. The
//!   child records that did land are complete, content-addressed artifacts in their own
//!   right; they are not residue and they are not deleted, and a retry converges on them.
//! - **no orphan task.** The task table is volatile, and docs/35 forbids the obvious
//!   workaround outright: a restart "MUST NOT reconstruct task state by inference". So no
//!   live `TaskEntry` comes back. What *is* durable is the published half: every campaign
//!   record a task committed before the crash is in the store under the identity the task
//!   named it by ([`budget::publication_record`](super::budget::publication_record)), and
//!   that record names its task, its sequence, its snapshot, and whether it closed or
//!   parked. [`resolve_tasks`] is the startup pass over those records (bn-1z09m, plan §4.5
//!   O2). It derives the task set from the store alone and resolves each task to exactly one
//!   typed [`Resolution`]: `Settled` when the head record closed, and `Failed` with a
//!   [`FailureReason`] otherwise. [`Builder::build`](super::Builder::build) runs it on every
//!   restart and loads the result into the task table, so every surviving `task_*` record
//!   has a claimant again.
//!
//!   The resume branch of O2 is **not** reachable, and the pass says so with
//!   [`FailureReason::ContinuationNotDurable`] rather than with a missing arm. The record
//!   carries the frontier and not the continuation's pins (epochs, bounds, intent, model
//!   source), and a continuation without its pins is not a committed continuation.
//!
//! That last row is the honest scope of IMPL-02's durability criterion — "committed partial
//! evidence survives daemon restart; uncommitted partials are absent, never half-visible".
//! The second conjunct holds outright. The first holds *for the evidence*, which is what the
//! criterion names, and not for the task record, which it does not.
//!
//! # Why there is no disk-backed daemon-state store here
//!
//! Because the dossier does not ask for one and the honest answer is better than a
//! fabricated one. plan §4.5's restart contract is about the *store* (crash safety, the index
//! verifier, GC roots) and about tasks resolving to a committed continuation or a typed
//! failure; a daemon whose continuation pins never reached a disk has no committed
//! continuation to resume from, and inventing one from the store would be the "silently
//! reconstructed state" the same paragraph prohibits. The resolution pass therefore reads
//! only what the store already holds, and takes the typed-failure branch. Persisting `DaemonState` is a real piece of work with its
//! own crate-boundary question (plan §20 gives `continuumd` no storage edge beyond
//! `continuum-workspace`), and it is not this bone's. What this bone owes and delivers is the
//! split above, stated and checked, and the machinery that makes a restart's answer typed.
//!
//! # This is not wire surface
//!
//! Nothing here is an operation. Recovery is an operator action on a store, like capability
//! administration and content staging, and RFC 0026 keeps that surface outside the operation
//! registry deliberately:
//!
//! > Minting, scoping, delegating, and revoking capabilities is a separate administrative
//! > surface (plan §4.5) that is out of scope for this IDL version.
//! >
//! > — `schemas/continuumd-native-protocol.idl`, §7
//!
//! [`recover`] takes a capability and goes through [`ReferenceStore::audit_view`], so it is
//! authorized at `promote` and audited by the store's own sink like every other privileged
//! call. No operation is added, no request or response body changes, and no IDL revision
//! record is owed.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use continuum_workspace::artifact_path::{ArtifactClass, ArtifactHandle, ArtifactPath};
use continuum_workspace::publication::{
    CapabilityDenied, CapabilityToken, ReferenceStore, StoreDefect,
};

use crate::protocol::scalar::{TaskHandle, WorkspaceHandle};

// --- crash points ------------------------------------------------------------------------

/// A boundary in [`Daemon::dispatch`](super::Daemon::dispatch) at which a harness may kill
/// the daemon.
///
/// One variant per step boundary of the eight-step dispatch, named for the step it precedes,
/// plus the boundary after the handler and before the idempotency ledger's record write. The
/// eight steps and the reasons for their order are in [`daemon`](super)'s documentation.
///
/// # Why all nine, when only one of them can change the durable image
///
/// Steps 1–7 read and write `DaemonState` and touch no store: version negotiation, the
/// registry lookup, shape agreement, the scope claim, admission, the annotation obligations,
/// and the idempotency ledger are all volatile-side work. Only step 8 — the family handler —
/// reaches [`ReferenceStore`]. So the durable image after a crash is a *step function* of the
/// boundary: identical for [`BeforeVersionCheck`](Self::BeforeVersionCheck) through
/// [`BeforeHandler`](Self::BeforeHandler), and changed only at
/// [`AfterHandler`](Self::AfterHandler).
///
/// That is a claim, not a convenience, and instrumenting all nine is what makes it
/// falsifiable: a handler that reached the store from step 5 would show up as two boundaries
/// disagreeing. `crates/continuumd/tests/g1_crash_recovery_evidence.rs` runs the sweep.
///
/// The *finer* grain inside step 8 — between a publication's content commit and its index
/// commit, which is docs/35's own crash-injection point and INV-017's instant — is not
/// duplicated here. It is
/// [`PublicationPhase`](continuum_workspace::publication::PublicationPhase), reached through
/// the store's [`StorageFaults`](continuum_workspace::publication::StorageFaults) seam, and
/// the store proves its own half (G0-DX-13). A second injection point for the same event
/// would be a second answer to when an artifact becomes visible.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CrashPoint {
    /// Before step 1 — the request has not been read at all.
    BeforeVersionCheck,
    /// Before step 2 — the version is negotiated; the operation is not yet looked up.
    BeforeOperationLookup,
    /// Before step 3 — the operation is registered; its shape is not yet agreed.
    BeforeShapeCheck,
    /// Before step 4 — the body is the declared shape; no scope is claimed yet.
    BeforeScopeClaim,
    /// Before step 5 — the scope claim is computed; admission has not run.
    BeforeAdmission,
    /// Before step 6 — the caller is admitted and the audit record is written.
    BeforeObligations,
    /// Before step 7 — the annotation obligations hold; the ledger is not consulted.
    BeforeIdempotencyLedger,
    /// Before step 8 — the ledger admitted the request; the handler has not run.
    BeforeHandler,
    /// After step 8 — the handler ran to completion; the replay record is not written.
    AfterHandler,
}

impl CrashPoint {
    /// Every boundary, in dispatch order.
    pub const ALL: [Self; 9] = [
        Self::BeforeVersionCheck,
        Self::BeforeOperationLookup,
        Self::BeforeShapeCheck,
        Self::BeforeScopeClaim,
        Self::BeforeAdmission,
        Self::BeforeObligations,
        Self::BeforeIdempotencyLedger,
        Self::BeforeHandler,
        Self::AfterHandler,
    ];

    /// The dispatch step this boundary precedes; `9` for the boundary after the handler.
    #[must_use]
    pub const fn step(self) -> u8 {
        match self {
            Self::BeforeVersionCheck => 1,
            Self::BeforeOperationLookup => 2,
            Self::BeforeShapeCheck => 3,
            Self::BeforeScopeClaim => 4,
            Self::BeforeAdmission => 5,
            Self::BeforeObligations => 6,
            Self::BeforeIdempotencyLedger => 7,
            Self::BeforeHandler => 8,
            Self::AfterHandler => 9,
        }
    }

    /// A stable token, for renderings and for a harness's own reporting.
    #[must_use]
    pub const fn token(self) -> &'static str {
        match self {
            Self::BeforeVersionCheck => "before-version-check",
            Self::BeforeOperationLookup => "before-operation-lookup",
            Self::BeforeShapeCheck => "before-shape-check",
            Self::BeforeScopeClaim => "before-scope-claim",
            Self::BeforeAdmission => "before-admission",
            Self::BeforeObligations => "before-obligations",
            Self::BeforeIdempotencyLedger => "before-idempotency-ledger",
            Self::BeforeHandler => "before-handler",
            Self::AfterHandler => "after-handler",
        }
    }

    /// Whether a crash at this boundary can have changed the store.
    ///
    /// True for [`AfterHandler`](Self::AfterHandler) and false for every other boundary,
    /// because step 8 is the only step that reaches the store. Stated as a function so the
    /// sweep in the evidence suite compares against a value rather than against a comment.
    #[must_use]
    pub const fn reaches_the_store(self) -> bool {
        matches!(self, Self::AfterHandler)
    }
}

impl fmt::Display for CrashPoint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.token())
    }
}

/// The seam that makes a whole-daemon crash testable without a process.
///
/// The dispatch-side counterpart of the store's
/// [`StorageFaults`](continuum_workspace::publication::StorageFaults), and it is here for the
/// same reason: docs/35's acceptance list requires "crash recovery tests, including a crash
/// injected between the content commit and the index commit of every publication phase, with
/// fsck run on the survivor", and a seam is the only way to reach those points without a
/// scheduler. [`NoCrash`] is the default and the only implementation this crate ships.
pub trait CrashInjector: Send + Sync {
    /// Whether the daemon dies at `point`.
    ///
    /// Consulted once per boundary per dispatch, in dispatch order. An implementation is
    /// given the boundary and nothing else — no request, no state, no store — so a crash
    /// cannot be made a function of the answer it interrupts.
    fn kills(&self, point: CrashPoint) -> bool;
}

/// A daemon that does not crash.
///
/// [`kills`](CrashInjector::kills) is `false` for every boundary, which is what makes
/// [`Daemon::dispatch`](super::Daemon::dispatch) total: see its documentation for the one
/// line of reasoning that rests on this.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NoCrash;

impl CrashInjector for NoCrash {
    fn kills(&self, _point: CrashPoint) -> bool {
        false
    }
}

/// The daemon died at this boundary, and the request has no answer.
///
/// Deliberately not an [`ErrorCode`](crate::protocol::vocabulary::ErrorCode): a wire error is
/// an *answer*, and a process that is gone gives none. The protocol fixes no code for a
/// daemon that is not there, and inventing one would be a fabricated reply to a caller whose
/// connection died — so this type carries the boundary and no envelope.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Killed {
    /// The boundary the daemon died at.
    pub point: CrashPoint,
}

impl fmt::Display for Killed {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "the daemon died at `{}`", self.point)
    }
}

impl core::error::Error for Killed {}

// --- the volatile declaration ------------------------------------------------------------

/// What a restart does about a fact the crash lost.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Disposition {
    /// The deployment restores it out of band, exactly as it provisioned it the first time.
    ///
    /// Capability administration, content staging, and model registration are not operations
    /// in this protocol version (IDL §7), so they arrive through
    /// [`Daemon::state_mut`](super::Daemon::state_mut) at startup and arrive the same way
    /// after a restart. Nothing is inferred and nothing is lost that the deployment did not
    /// choose to stop providing.
    Reprovisioned,
    /// It is gone, it is named as gone, and it is not reconstructed.
    ///
    /// docs/35: a restart "MUST NOT reconstruct task state by inference […] A silently
    /// reconstructed state is indistinguishable from a fabricated one".
    Declared,
    /// The live entries are gone and are not reconstructed. Its durable trace is read back
    /// at startup, and each member it names is resolved to a typed outcome.
    ///
    /// The task table's disposition since bn-1z09m: [`resolve_tasks`] reads every committed
    /// campaign record and resolves each task to [`Resolution::Settled`] or to
    /// [`Resolution::Failed`] with a [`FailureReason`] (plan §4.5 O2). A resolved task is not
    /// a live `TaskEntry`: its operation, epochs, budget and report were never durable.
    Resolved,
}

impl Disposition {
    /// A stable token.
    #[must_use]
    pub const fn token(self) -> &'static str {
        match self {
            Self::Reprovisioned => "reprovisioned",
            Self::Declared => "declared",
            Self::Resolved => "resolved",
        }
    }
}

/// One fact this daemon holds only in memory.
///
/// The list is [`DaemonState`](super::state::DaemonState)'s own fields, audited one at a time,
/// the way [`ReplayKey`](super::state::ReplayKey) audits `RequestEnvelope`'s. A field added
/// there without a variant here is a fact a restart would lose in silence, which is what this
/// enumeration exists to prevent;
/// `crates/continuumd/tests/g1_crash_recovery_evidence.rs` pins the correspondence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum VolatileFact {
    /// `capabilities` — the wire capability registry admission reads.
    WireCapabilityRegistry,
    /// `content` — staged file content, addressed by commitment.
    StagedContent,
    /// `models` — the elaborated models a campaign can run over.
    ModelCatalog,
    /// `workspaces` — the workspace records `ws_*` handles name.
    WorkspaceRecords,
    /// `lineages` — the fork lineages staleness is decided against.
    LineageMap,
    /// `intents` — the Intent Contract registry.
    IntentRegistry,
    /// `evidence` — the evidence graph's nodes and their status histories.
    EvidenceGraph,
    /// `evidence_events` — the committed evidence-graph deltas a subscription drains.
    EvidenceEventLog,
    /// `idempotency` — the per-actor replay ledger.
    IdempotencyLedger,
    /// `admissions` — the RFC 0027 P5 admission audit records.
    AdmissionLog,
    /// `tasks` — the task table.
    TaskTable,
    /// `tasks` — the parked `cont_*` continuations the table holds beside its entries.
    ContinuationTable,
    /// `regions` — the region tree and its finalization ledger.
    RegionTree,
}

impl VolatileFact {
    /// Every fact, in declaration order.
    pub const ALL: [Self; 13] = [
        Self::WireCapabilityRegistry,
        Self::StagedContent,
        Self::ModelCatalog,
        Self::WorkspaceRecords,
        Self::LineageMap,
        Self::IntentRegistry,
        Self::EvidenceGraph,
        Self::EvidenceEventLog,
        Self::IdempotencyLedger,
        Self::AdmissionLog,
        Self::TaskTable,
        Self::ContinuationTable,
        Self::RegionTree,
    ];

    /// A stable token naming the fact.
    #[must_use]
    pub const fn token(self) -> &'static str {
        match self {
            Self::WireCapabilityRegistry => "wire-capability-registry",
            Self::StagedContent => "staged-content",
            Self::ModelCatalog => "model-catalog",
            Self::WorkspaceRecords => "workspace-records",
            Self::LineageMap => "lineage-map",
            Self::IntentRegistry => "intent-registry",
            Self::EvidenceGraph => "evidence-graph",
            Self::EvidenceEventLog => "evidence-event-log",
            Self::IdempotencyLedger => "idempotency-ledger",
            Self::AdmissionLog => "admission-log",
            Self::TaskTable => "task-table",
            Self::ContinuationTable => "continuation-table",
            Self::RegionTree => "region-tree",
        }
    }

    /// What the restart does about it.
    #[must_use]
    pub const fn disposition(self) -> Disposition {
        match self {
            // The three out-of-band provisioning surfaces (IDL §7).
            Self::WireCapabilityRegistry | Self::StagedContent | Self::ModelCatalog => {
                Disposition::Reprovisioned
            }
            // The one fact with a startup pass over its durable trace (plan §4.5 O2).
            Self::TaskTable => Disposition::Resolved,
            _ => Disposition::Declared,
        }
    }

    /// The artifact class whose surviving store artifacts are what remains of this fact, when
    /// any do.
    ///
    /// [`None`] is the common and honest case: most of `DaemonState` is daemon-side
    /// bookkeeping with no published counterpart at all, and naming a class for it would
    /// suggest a recovery path that does not exist. The three that do have one are:
    ///
    /// - the workspace records — a *sealed* workspace's records are in the store, which is
    ///   what sealing means;
    /// - the task table, whose committed campaign records are published under
    ///   [`ArtifactClass::Task`] and are what [`resolve_tasks`] reads;
    /// - the continuation table, whose *frontier* is part of that same record
    ///   ([`budget::publication_record`](super::budget::publication_record) writes it, and
    ///   [`CampaignRecord::frontier`] reads its length back). The continuation's pins — its
    ///   epochs, bounds, intent and model source — are not in the record, so the
    ///   continuation as a whole stays [`Disposition::Declared`]: a frontier without its pins
    ///   is not a committed continuation, and resuming from it would be inference.
    #[must_use]
    pub const fn survives_as(self) -> Option<ArtifactClass> {
        match self {
            Self::WorkspaceRecords => Some(ArtifactClass::WorkspaceSnapshot),
            Self::TaskTable | Self::ContinuationTable => Some(ArtifactClass::Task),
            _ => None,
        }
    }
}

impl fmt::Display for VolatileFact {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.token())
    }
}

// --- the report --------------------------------------------------------------------------

/// What one artifact class holds after the crash.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ClassSummary {
    /// The class.
    pub class: ArtifactClass,
    /// Index entries naming an artifact of this class — docs/35's "results per artifact
    /// class".
    pub published: u64,
    /// Committed content of this class that no index entry names: crash residue.
    pub unreachable: u64,
    /// Bytes of committed content of this class, published and unreachable together.
    pub bytes: u64,
}

/// What a recovery concluded, in one word.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Verdict {
    /// Every index entry resolves, every entry has its receipts, and there is no residue.
    Clean,
    /// As [`Clean`](Self::Clean), plus unreachable content: the ordinary crash residue
    /// docs/35 accepts in exchange for never writing a stale index entry. Reported and
    /// quarantined, never deleted by the recovery itself.
    Residue,
    /// Something the store's own rules forbid was found: a stale index entry, corruption, an
    /// index entry with no receipt, or a receipt for an unpublished identity. Reported, never
    /// repaired.
    Defective,
}

impl Verdict {
    /// A stable token.
    #[must_use]
    pub const fn token(self) -> &'static str {
        match self {
            Self::Clean => "clean",
            Self::Residue => "residue",
            Self::Defective => "defective",
        }
    }
}

/// The evidence artifact a restart produces: what was found, what was reconciled, and what
/// was declared lost.
///
/// # Deterministic by construction
///
/// Every field is a sorted `Vec` or a [`BTreeMap`]-derived one, and every input is read from
/// the store under one audit view, so [`render`](Self::render) is a function of the store's
/// contents and of nothing else — no clock, no counter, no iteration order that depends on
/// insertion history. Two recoveries from one crashed state render byte-identically, and so
/// do two independently restarted daemons over one store: the two-fresh-daemons device
/// (`rule ordering.deterministic`) applied to recovery.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecoveryReport {
    classes: Vec<ClassSummary>,
    reconciled: Vec<(ArtifactPath, ArtifactHandle)>,
    receipts: Vec<(ArtifactHandle, u64)>,
    quarantined: Vec<ArtifactHandle>,
    receiptless: Vec<ArtifactHandle>,
    orphan_receipts: Vec<ArtifactHandle>,
    defects: Vec<StoreDefect>,
    volatile: Vec<VolatileFact>,
}

impl RecoveryReport {
    /// Per-class holdings, in class order.
    #[must_use]
    pub fn classes(&self) -> &[ClassSummary] {
        &self.classes
    }

    /// Every index entry that resolved, as `(path, identity)`, in path order.
    ///
    /// "Resolved" is the whole check: the content the entry names is present *and* identifies
    /// to the handle it is filed under. An entry that fails either half is not here — it is a
    /// [`StoreDefect`] in [`defects`](Self::defects).
    #[must_use]
    pub fn reconciled(&self) -> &[(ArtifactPath, ArtifactHandle)] {
        &self.reconciled
    }

    /// Receipt counts per published identity, in identity order.
    ///
    /// G0-DX-13's "no lost receipts" clause, read after a crash: N publishers of one identity
    /// leave N receipts, and the ledger is append-only, so a restart can count them.
    #[must_use]
    pub fn receipts(&self) -> &[(ArtifactHandle, u64)] {
        &self.receipts
    }

    /// Content no index entry names: crash residue, reported and left in place.
    ///
    /// This is the docs/35 asymmetry as a list — "wasted bytes are recoverable, a dangling
    /// index entry is a lie about what the store holds". Recovery **never deletes**:
    /// reclaiming these is
    /// [`ReferenceStore::collect_garbage`](continuum_workspace::publication::ReferenceStore::collect_garbage),
    /// an explicit operator action under its own capability, and it is deliberately not run
    /// from here — a recovery that silently reclaimed would destroy the evidence that a crash
    /// happened at all.
    #[must_use]
    pub fn quarantined(&self) -> &[ArtifactHandle] {
        &self.quarantined
    }

    /// Published identities with no receipt. MUST be empty.
    ///
    /// The store appends a receipt in the same critical section that writes the index entry
    /// naming it, so an index entry without one is a lost receipt. Checked rather than
    /// assumed.
    #[must_use]
    pub fn receiptless(&self) -> &[ArtifactHandle] {
        &self.receiptless
    }

    /// Receipts naming an identity the index does not. MUST be empty.
    ///
    /// A receipt is proof that an artifact *was published*; one naming an unpublished
    /// identity would be a forged receipt. The store cannot issue one — the two writes share a
    /// critical section — and this is the check that says so after a crash rather than before.
    #[must_use]
    pub fn orphan_receipts(&self) -> &[ArtifactHandle] {
        &self.orphan_receipts
    }

    /// docs/35's three defect classes, in the verifier's own deterministic order.
    ///
    /// This is [`StoreAudit::fsck`](continuum_workspace::publication::StoreAudit::fsck)'s
    /// result verbatim. The classification is closed — unreachable content, missing referent,
    /// identity mismatch — and this report does not widen it:
    /// [`receiptless`](Self::receiptless) and [`orphan_receipts`](Self::orphan_receipts) are
    /// ledger-versus-index findings, which is a different obligation (G0-DX-13's) from the
    /// content-versus-index one docs/35 fixes the three classes for, and they are reported
    /// beside the defect list rather than inside it.
    #[must_use]
    pub fn defects(&self) -> &[StoreDefect] {
        &self.defects
    }

    /// Every fact the crash lost, and what the restart does about each.
    #[must_use]
    pub fn volatile(&self) -> &[VolatileFact] {
        &self.volatile
    }

    /// The published identities of one class, in identity order.
    ///
    /// [`ArtifactClass::Task`] is the interesting one: those are the campaign records
    /// committed publications were published as, and returning them is the recoverable half
    /// of "this task's record did not survive; its published artifacts did".
    #[must_use]
    pub fn surviving(&self, class: ArtifactClass) -> Vec<ArtifactHandle> {
        self.reconciled
            .iter()
            .map(|(_, handle)| handle)
            .filter(|handle| handle.class() == class)
            .cloned()
            .collect()
    }

    /// The one-word conclusion. See [`Verdict`].
    #[must_use]
    pub fn verdict(&self) -> Verdict {
        let forbidden = self
            .defects
            .iter()
            .any(|defect| !matches!(defect, StoreDefect::UnreachableContent(_)));
        if forbidden || !self.receiptless.is_empty() || !self.orphan_receipts.is_empty() {
            return Verdict::Defective;
        }
        if self.quarantined.is_empty() {
            Verdict::Clean
        } else {
            Verdict::Residue
        }
    }

    /// The canonical rendering: one fact per line, in a fixed section order.
    ///
    /// The byte-stable form the acceptance criterion asks for. It is a *total* projection —
    /// every field above appears — so two reports that render identically are equal, and the
    /// evidence suite compares bytes rather than fields.
    #[must_use]
    pub fn render(&self) -> String {
        let mut out = String::new();
        out.push_str("recovery-report 1\n");
        out.push_str(&format!("verdict {}\n", self.verdict().token()));
        for summary in &self.classes {
            out.push_str(&format!(
                "class {} published={} unreachable={} bytes={}\n",
                summary.class.token(),
                summary.published,
                summary.unreachable,
                summary.bytes
            ));
        }
        for (path, handle) in &self.reconciled {
            out.push_str(&format!("reconciled {path} {handle}\n"));
        }
        for (handle, count) in &self.receipts {
            out.push_str(&format!("receipts {handle} {count}\n"));
        }
        for handle in &self.quarantined {
            out.push_str(&format!("quarantined {handle}\n"));
        }
        for handle in &self.receiptless {
            out.push_str(&format!("receiptless {handle}\n"));
        }
        for handle in &self.orphan_receipts {
            out.push_str(&format!("orphan-receipt {handle}\n"));
        }
        for defect in &self.defects {
            out.push_str(&format!("defect {defect}\n"));
        }
        for fact in &self.volatile {
            out.push_str(&format!(
                "volatile {} {} {}\n",
                fact.token(),
                fact.disposition().token(),
                fact.survives_as().map_or(
                    "-",
                    continuum_workspace::artifact_path::ArtifactClass::token
                )
            ));
        }
        out
    }
}

/// Reconcile a store after a crash, and report what was found.
///
/// The fsck-style sweep, in one pass over one audit view:
///
/// 1. **every index entry resolves.** The path an entry is filed under is a pure function of
///    the handle ([`ArtifactPath::for_handle`]), so the entry, the content, and the identity
///    are three statements that must agree; the ones that do are
///    [`reconciled`](RecoveryReport::reconciled) and the ones that do not are
///    [`StoreDefect`]s.
/// 2. **every index entry has its receipts, and no receipt names an unpublished identity.**
///    Ledger against index, in both directions.
/// 3. **content no entry names is quarantined**, named in the report, and left exactly where
///    it is.
/// 4. **what did not survive is declared**, per [`VolatileFact`], rather than inferred.
///
/// Nothing here writes: `recover` takes `&ReferenceStore` and calls no mutating method, so a
/// recovery cannot itself be the thing that damaged the store. Repair is not attempted for
/// any finding — docs/35 forbids it for corruption outright ("MUST report it and MUST NOT
/// silently repair it"), and for residue the reclaim is
/// [`collect_garbage`](continuum_workspace::publication::ReferenceStore::collect_garbage), a
/// separate operator decision under its own capability.
///
/// # Errors
///
/// [`CapabilityDenied`] when `operator` does not confer
/// [`Action::Audit`](continuum_workspace::publication::Action::Audit). Recovery discloses
/// everything the store holds, so it sits at the same `promote` level the receipt ledger and
/// the index verifier do — and it is audited by the store's own sink on the way in.
pub fn recover(
    store: &ReferenceStore,
    operator: &CapabilityToken,
) -> Result<RecoveryReport, CapabilityDenied> {
    let audit = store.audit_view(operator)?;

    let defects = audit.fsck();
    let published: Vec<ArtifactHandle> = audit.identities();
    let indexed: BTreeSet<ArtifactHandle> = published.iter().cloned().collect();
    let mut quarantined: Vec<ArtifactHandle> = defects
        .iter()
        .filter_map(|defect| match defect {
            StoreDefect::UnreachableContent(handle) => Some(handle.clone()),
            _ => None,
        })
        .collect();
    quarantined.sort();
    quarantined.dedup();

    // An entry is reconciled when it resolves. `fsck` has already decided that: a
    // `MissingReferent` names the path of an entry whose content is absent, and an
    // `IdentityMismatch` names content filed under an identity it does not derive to.
    let missing: BTreeSet<&ArtifactPath> = defects
        .iter()
        .filter_map(|defect| match defect {
            StoreDefect::MissingReferent(path) => Some(path),
            _ => None,
        })
        .collect();
    let mismatched: BTreeSet<&ArtifactHandle> = defects
        .iter()
        .filter_map(|defect| match defect {
            StoreDefect::IdentityMismatch(handle) => Some(handle),
            _ => None,
        })
        .collect();

    let mut reconciled = Vec::new();
    let mut receipts = Vec::new();
    let mut receiptless = Vec::new();
    for handle in &published {
        // Total for every class the index can hold: `commit_index` files an entry under
        // exactly this path, and a class with no derived path (`cap_*`) is refused at
        // staging, so it never reaches the index.
        let Ok(path) = ArtifactPath::for_handle(handle) else {
            continue;
        };
        let count = audit.receipts(handle).len() as u64;
        if count == 0 {
            receiptless.push(handle.clone());
        } else {
            receipts.push((handle.clone(), count));
        }
        if !missing.contains(&path) && !mismatched.contains(handle) {
            reconciled.push((path, handle.clone()));
        }
    }

    // The other direction: a receipt for an identity the index does not name. Probed over the
    // content the index does *not* reach, which is where such a receipt would have to point.
    let mut orphan_receipts: Vec<ArtifactHandle> = quarantined
        .iter()
        .filter(|handle| !indexed.contains(*handle) && !audit.receipts(handle).is_empty())
        .cloned()
        .collect();
    orphan_receipts.sort();

    reconciled.sort();
    receipts.sort();
    receiptless.sort();

    let bytes = audit.storage_attribution();
    let mut counts: BTreeMap<ArtifactClass, (u64, u64)> = BTreeMap::new();
    for handle in &published {
        counts.entry(handle.class()).or_insert((0, 0)).0 += 1;
    }
    for handle in &quarantined {
        counts.entry(handle.class()).or_insert((0, 0)).1 += 1;
    }
    for class in bytes.keys() {
        counts.entry(*class).or_insert((0, 0));
    }
    let classes = counts
        .into_iter()
        .map(|(class, (published, unreachable))| ClassSummary {
            class,
            published,
            unreachable,
            bytes: bytes.get(&class).copied().unwrap_or(0),
        })
        .collect();

    Ok(RecoveryReport {
        classes,
        reconciled,
        receipts,
        quarantined,
        receiptless,
        orphan_receipts,
        defects,
        volatile: VolatileFact::ALL.to_vec(),
    })
}

// --- the startup task-resolution pass (plan §4.5 O2) -------------------------------------

/// Why one `task_*` index entry could not be read as a campaign record.
///
/// Typed per cause (INV-008). A record that does not decode cannot be attributed to a task,
/// so it is reported beside the resolved tasks and never silently dropped.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RecordDefect {
    /// The read path refused an identity the index names. Unreachable while the store keeps
    /// its own rules; `recover` reports the same entry as a [`StoreDefect`].
    Unreadable,
    /// The bytes end inside a length-prefixed part, or a part is missing.
    Truncated,
    /// Bytes remain after the last part the record format declares.
    TrailingBytes,
    /// A numeric part does not have its declared width.
    FieldWidth,
    /// The task part is not a well-formed `task_*` handle.
    TaskHandle,
    /// The snapshot part is neither `-` nor a well-formed `ws_*` handle.
    SnapshotHandle,
    /// The closure part is neither `closed` nor `bounded`.
    ClosureToken,
    /// The frontier component count is not a whole number of states.
    FrontierShape,
}

impl RecordDefect {
    /// A stable token.
    #[must_use]
    pub const fn token(self) -> &'static str {
        match self {
            Self::Unreadable => "unreadable",
            Self::Truncated => "truncated",
            Self::TrailingBytes => "trailing-bytes",
            Self::FieldWidth => "field-width",
            Self::TaskHandle => "task-handle",
            Self::SnapshotHandle => "snapshot-handle",
            Self::ClosureToken => "closure-token",
            Self::FrontierShape => "frontier-shape",
        }
    }
}

impl fmt::Display for RecordDefect {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.token())
    }
}

/// One published campaign record, decoded from the bytes the store holds.
///
/// The inverse of [`budget::publication_record`](super::budget::publication_record), field
/// for field. Every field is read from the committed bytes. Nothing is supplied from the
/// daemon's current configuration, so nothing here is inferred.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct CampaignRecord {
    /// The store identity the record is published under.
    pub identity: ArtifactHandle,
    /// The task that committed it.
    pub task: TaskHandle,
    /// Its position in that task's commit order, from zero.
    pub sequence: u32,
    /// The snapshot the run was over, or [`None`] when the record says `-`.
    pub snapshot: Option<WorkspaceHandle>,
    /// How many states the run explored.
    pub states: u64,
    /// Whether the exploration closed (`true`) or a bound tripped (`false`).
    pub closed: bool,
    /// How many unexpanded states the run parked with.
    pub frontier: u64,
}

/// A cursor over one length-prefixed record.
struct Parts<'a>(&'a [u8]);

impl<'a> Parts<'a> {
    fn next(&mut self) -> Result<&'a [u8], RecordDefect> {
        let (width, rest) = self
            .0
            .split_first_chunk::<8>()
            .ok_or(RecordDefect::Truncated)?;
        let width =
            usize::try_from(u64::from_be_bytes(*width)).map_err(|_| RecordDefect::Truncated)?;
        if rest.len() < width {
            return Err(RecordDefect::Truncated);
        }
        let (part, rest) = rest.split_at(width);
        self.0 = rest;
        Ok(part)
    }

    fn text(&mut self) -> Result<&'a str, RecordDefect> {
        std::str::from_utf8(self.next()?).map_err(|_| RecordDefect::Truncated)
    }

    fn u64(&mut self) -> Result<u64, RecordDefect> {
        let bytes: [u8; 8] = self
            .next()?
            .try_into()
            .map_err(|_| RecordDefect::FieldWidth)?;
        Ok(u64::from_be_bytes(bytes))
    }

    fn u32(&mut self) -> Result<u32, RecordDefect> {
        let bytes: [u8; 4] = self
            .next()?
            .try_into()
            .map_err(|_| RecordDefect::FieldWidth)?;
        Ok(u32::from_be_bytes(bytes))
    }
}

/// Decode the bytes a `task_*` index entry names into a [`CampaignRecord`].
///
/// Strict: every part must be present at its declared width, and no byte may remain. A
/// record that does not decode is a [`RecordDefect`], never a best-effort partial.
///
/// # Errors
///
/// The [`RecordDefect`] that names the first part that did not decode.
pub fn decode_campaign_record(
    identity: &ArtifactHandle,
    bytes: &[u8],
) -> Result<CampaignRecord, RecordDefect> {
    let mut parts = Parts(bytes);
    let task = TaskHandle::new(parts.text()?).map_err(|_| RecordDefect::TaskHandle)?;
    let sequence = parts.u32()?;
    let snapshot = match parts.text()? {
        "-" => None,
        text => Some(WorkspaceHandle::new(text).map_err(|_| RecordDefect::SnapshotHandle)?),
    };
    let states = parts.u64()?;
    let closed = match parts.text()? {
        "closed" => true,
        "bounded" => false,
        _ => return Err(RecordDefect::ClosureToken),
    };
    let frontier = parts.u64()?;
    let mut components: u64 = 0;
    while !parts.0.is_empty() {
        parts.u64()?;
        components += 1;
    }
    // A state has at least one component, and every state in one model has the same arity,
    // so the components are a whole multiple of the frontier length. An empty frontier has
    // none.
    let shaped = match frontier {
        0 => components == 0,
        n => components >= n && components % n == 0,
    };
    if !shaped {
        return Err(if frontier == 0 {
            RecordDefect::TrailingBytes
        } else {
            RecordDefect::FrontierShape
        });
    }
    Ok(CampaignRecord {
        identity: identity.clone(),
        task,
        sequence,
        snapshot,
        states,
        closed,
        frontier,
    })
}

/// Why the pass resolved a task to `Failed`.
///
/// docs/35: a task the restart cannot resume "transition[s] to `Failed` with a typed reason
/// (INV-008)". Each variant is one distinct cause. None is a catch-all.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FailureReason {
    /// The head record is `bounded`: the task parked with a frontier, and the frontier is
    /// durable in the record. The continuation's *pins* are not. The epochs, the bounds, the
    /// intent and the model source live only on the volatile `Continuation`. Resuming would
    /// take them from the successor's configuration, which is reconstruction by inference
    /// (docs/35). So the committed continuation this task needs does not exist.
    ContinuationNotDurable,
    /// Two different records claim the head sequence, so the task's last commit is
    /// ambiguous. Neither is chosen.
    AmbiguousHead,
    /// The task's sequences are not contiguous from zero, so a committed record is missing
    /// and the durable history is incomplete.
    SequenceGap,
}

impl FailureReason {
    /// A stable token.
    #[must_use]
    pub const fn token(self) -> &'static str {
        match self {
            Self::ContinuationNotDurable => "continuation-not-durable",
            Self::AmbiguousHead => "ambiguous-head",
            Self::SequenceGap => "sequence-gap",
        }
    }
}

impl fmt::Display for FailureReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.token())
    }
}

/// What the pass concluded about one task.
///
/// There is no `Resumed` arm. The resume branch of plan §4.5 needs a committed
/// continuation, and this build commits the frontier without the pins (see
/// [`FailureReason::ContinuationNotDurable`]). An arm no input can reach would be a claim
/// the pass does not make.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Resolution {
    /// The head record is `closed`: the task finished its exploration and reached a terminal
    /// status before the crash. It was not left `Running` or `Suspended`, so O2 does not
    /// apply. Its committed records are claimed. Its terminal status and its report are not
    /// durable, and are not reconstructed.
    Settled,
    /// The task was non-terminal at its last durable commit, or its durable history is not
    /// sound. It is `Failed`, for the typed reason given.
    Failed(FailureReason),
}

impl Resolution {
    /// A stable token.
    #[must_use]
    pub const fn token(self) -> &'static str {
        match self {
            Self::Settled => "settled",
            Self::Failed(reason) => reason.token(),
        }
    }
}

/// One task the pass found in the store, and what it resolved the task to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedTask {
    /// The task.
    pub task: TaskHandle,
    /// Every record the task committed, in `(sequence, identity)` order.
    pub records: Vec<CampaignRecord>,
    /// The outcome.
    pub resolution: Resolution,
}

impl ResolvedTask {
    /// The store identities this task claims: every record it committed.
    #[must_use]
    pub fn claims(&self) -> Vec<ArtifactHandle> {
        self.records
            .iter()
            .map(|record| record.identity.clone())
            .collect()
    }
}

/// The output of the startup task-resolution pass.
///
/// Deterministic by construction, like [`RecoveryReport`]. The input is the index and the
/// bytes it names, and every list is sorted, so [`render`](Self::render) is a function of
/// the store alone.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TaskResolution {
    tasks: Vec<ResolvedTask>,
    unattributed: Vec<(ArtifactHandle, RecordDefect)>,
}

impl TaskResolution {
    /// Every task the store holds a record for, in task order.
    #[must_use]
    pub fn tasks(&self) -> &[ResolvedTask] {
        &self.tasks
    }

    /// `task_*` index entries that do not decode as a campaign record, with the typed cause.
    #[must_use]
    pub fn unattributed(&self) -> &[(ArtifactHandle, RecordDefect)] {
        &self.unattributed
    }

    /// The canonical rendering: one fact per line, in a fixed section order.
    #[must_use]
    pub fn render(&self) -> String {
        let mut out = String::new();
        out.push_str("task-resolution 1\n");
        for resolved in &self.tasks {
            out.push_str(&format!(
                "task {} {}\n",
                resolved.task.as_str(),
                resolved.resolution.token()
            ));
            for record in &resolved.records {
                out.push_str(&format!(
                    "record {} {} {} states={} {} frontier={}\n",
                    record.sequence,
                    record.identity,
                    record
                        .snapshot
                        .as_ref()
                        .map_or("-", WorkspaceHandle::as_str),
                    record.states,
                    if record.closed { "closed" } else { "bounded" },
                    record.frontier
                ));
            }
        }
        for (handle, defect) in &self.unattributed {
            out.push_str(&format!("unattributed {handle} {defect}\n"));
        }
        out
    }
}

/// Resolve every task the store holds a record for (plan §4.5 O2, docs/35).
///
/// The recoverable set is derived from **durable records only**: every `task_*` identity
/// the index names, read through the ordinary read path and decoded. No volatile table is
/// consulted, because after a crash there is none. Per task, in this order:
///
/// 1. sequences not contiguous from zero → `Failed(SequenceGap)`;
/// 2. more than one record at the head sequence → `Failed(AmbiguousHead)`;
/// 3. head record `closed` → [`Resolution::Settled`];
/// 4. head record `bounded` → `Failed(ContinuationNotDurable)`.
///
/// The pass writes nothing, reads no clock and draws no entropy.
///
/// # Errors
///
/// [`CapabilityDenied`] when `operator` does not confer
/// [`Action::Audit`](continuum_workspace::publication::Action::Audit), as for [`recover`].
pub fn resolve_tasks(
    store: &ReferenceStore,
    operator: &CapabilityToken,
) -> Result<TaskResolution, CapabilityDenied> {
    let audit = store.audit_view(operator)?;
    let mut identities: Vec<ArtifactHandle> = audit
        .identities()
        .into_iter()
        .filter(|handle| handle.class() == ArtifactClass::Task)
        .collect();
    identities.sort();

    let mut records = Vec::new();
    let mut unattributed = Vec::new();
    for identity in identities {
        match store.read(&identity, operator) {
            Err(CapabilityDenied) => unattributed.push((identity, RecordDefect::Unreadable)),
            Ok(bytes) => match decode_campaign_record(&identity, &bytes) {
                Ok(record) => records.push(record),
                Err(defect) => unattributed.push((identity, defect)),
            },
        }
    }
    Ok(resolve_records(records, unattributed))
}

/// The pure half of [`resolve_tasks`]: group decoded records by task and resolve each.
#[must_use]
pub fn resolve_records(
    records: Vec<CampaignRecord>,
    mut unattributed: Vec<(ArtifactHandle, RecordDefect)>,
) -> TaskResolution {
    let mut by_task: BTreeMap<TaskHandle, Vec<CampaignRecord>> = BTreeMap::new();
    for record in records {
        by_task.entry(record.task.clone()).or_default().push(record);
    }
    let tasks = by_task
        .into_iter()
        .map(|(task, mut records)| {
            records.sort_by(|a, b| (a.sequence, &a.identity).cmp(&(b.sequence, &b.identity)));
            records.dedup_by(|a, b| a.identity == b.identity);
            let resolution = resolve_one(&records);
            ResolvedTask {
                task,
                records,
                resolution,
            }
        })
        .collect();
    unattributed.sort();
    unattributed.dedup();
    TaskResolution {
        tasks,
        unattributed,
    }
}

/// One task's resolution, from its records in `(sequence, identity)` order. See
/// [`resolve_tasks`] for the rule order.
fn resolve_one(records: &[CampaignRecord]) -> Resolution {
    let sequences: BTreeSet<u32> = records.iter().map(|record| record.sequence).collect();
    let Some(head) = sequences.last().copied() else {
        // Unreachable: a group exists only because a record put it there.
        return Resolution::Failed(FailureReason::SequenceGap);
    };
    let contiguous = u32::try_from(sequences.len()).is_ok_and(|count| count == head + 1);
    if !contiguous {
        return Resolution::Failed(FailureReason::SequenceGap);
    }
    let heads: Vec<&CampaignRecord> = records
        .iter()
        .filter(|record| record.sequence == head)
        .collect();
    match heads.as_slice() {
        [only] if only.closed => Resolution::Settled,
        [_] => Resolution::Failed(FailureReason::ContinuationNotDurable),
        _ => Resolution::Failed(FailureReason::AmbiguousHead),
    }
}

/// What the daemon did about tasks when it started.
///
/// A typed value rather than a flag: a cold start, a restart that ran the pass, and a
/// restart whose connection capability could not audit the store are three different
/// facts, and the third is not a success.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Startup {
    /// A cold start: no durable substrate was adopted, so there was nothing to resolve.
    Cold,
    /// A restart over a durable substrate, and the pass ran.
    Resolved(TaskResolution),
    /// A restart over a durable substrate, and the connection capability could not audit
    /// the store, so the pass did not run. No task was resolved.
    Refused(CapabilityDenied),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_crash_kills_at_no_boundary() {
        // `Daemon::dispatch`'s totality rests on exactly this, so it is a checked fact rather
        // than a comment beside the `expect`.
        for point in CrashPoint::ALL {
            assert!(!NoCrash.kills(point), "`NoCrash` killed at {point}");
        }
    }

    #[test]
    fn the_boundaries_are_the_eight_dispatch_steps_and_the_one_after() {
        let steps: Vec<u8> = CrashPoint::ALL.iter().map(|point| point.step()).collect();
        assert_eq!(steps, vec![1, 2, 3, 4, 5, 6, 7, 8, 9]);
        assert_eq!(
            CrashPoint::ALL
                .iter()
                .filter(|point| point.reaches_the_store())
                .count(),
            1,
            "step 8 is the only step that reaches the store"
        );
    }

    #[test]
    fn every_volatile_fact_has_a_distinct_token() {
        let tokens: BTreeSet<&str> = VolatileFact::ALL.iter().map(|fact| fact.token()).collect();
        assert_eq!(tokens.len(), VolatileFact::ALL.len());
    }

    #[test]
    fn the_three_out_of_band_provisioning_surfaces_are_the_reprovisioned_ones() {
        let reprovisioned: Vec<&str> = VolatileFact::ALL
            .iter()
            .filter(|fact| fact.disposition() == Disposition::Reprovisioned)
            .map(|fact| fact.token())
            .collect();
        assert_eq!(
            reprovisioned,
            vec![
                "wire-capability-registry",
                "staged-content",
                "model-catalog"
            ],
            "IDL §7's out-of-band surface, and nothing else, is restored by the deployment"
        );
    }

    // --- the startup task-resolution pass -------------------------------------------------

    use super::super::budget::publication_record;
    use super::super::task::Preimage;

    fn task(name: &str) -> TaskHandle {
        TaskHandle::new(name).expect("a well-formed task handle")
    }

    fn identity(name: &str) -> ArtifactHandle {
        ArtifactHandle::new(ArtifactClass::Task, name).expect("a well-formed identity")
    }

    /// A record as `publication_record` writes it, with `frontier` states of `arity`
    /// components each. Built by hand because the engine's `State` has no public constructor.
    fn record_bytes(
        handle: &str,
        sequence: u32,
        snapshot: &str,
        closed: bool,
        frontier: u64,
        arity: u64,
    ) -> Vec<u8> {
        let mut preimage = Preimage::new();
        preimage.text(handle);
        preimage.push(&sequence.to_be_bytes());
        preimage.text(snapshot);
        preimage.push(&7_u64.to_be_bytes());
        preimage.text(if closed { "closed" } else { "bounded" });
        preimage.push(&frontier.to_be_bytes());
        for component in 0..frontier * arity {
            preimage.push(&(component as i64).to_be_bytes());
        }
        preimage.bytes().to_vec()
    }

    fn record(handle: &str, name: &str, sequence: u32, closed: bool) -> CampaignRecord {
        decode_campaign_record(
            &identity(name),
            &record_bytes(handle, sequence, "ws_a", closed, u64::from(!closed), 2),
        )
        .expect("a well-formed record decodes")
    }

    #[test]
    fn the_decoder_is_the_inverse_of_the_record_writer() {
        let bytes = publication_record(&task("task_t"), 3, Some("ws_s"), 16, true, &[]);
        let decoded = decode_campaign_record(&identity("r"), &bytes).expect("decodes");
        assert_eq!(decoded.task, task("task_t"));
        assert_eq!(decoded.sequence, 3);
        assert_eq!(
            decoded.snapshot.as_ref().map(WorkspaceHandle::as_str),
            Some("ws_s")
        );
        assert_eq!(decoded.states, 16);
        assert!(decoded.closed);
        assert_eq!(decoded.frontier, 0);

        let bytes = publication_record(&task("task_t"), 0, None, 4, false, &[]);
        let decoded = decode_campaign_record(&identity("r"), &bytes).expect("decodes");
        assert_eq!(decoded.snapshot, None);
        assert!(!decoded.closed);

        let bytes = record_bytes("task_t", 1, "ws_s", false, 3, 2);
        let decoded = decode_campaign_record(&identity("r"), &bytes).expect("decodes");
        assert_eq!(decoded.frontier, 3);
    }

    #[test]
    fn a_record_that_does_not_decode_is_a_typed_defect_per_cause() {
        let good = record_bytes("task_t", 0, "ws_s", false, 2, 2);
        let cases: Vec<(Vec<u8>, RecordDefect)> = vec![
            (good[..good.len() - 1].to_vec(), RecordDefect::Truncated),
            (Vec::new(), RecordDefect::Truncated),
            (
                record_bytes("not-a-task", 0, "ws_s", true, 0, 0),
                RecordDefect::TaskHandle,
            ),
            (
                record_bytes("task_t", 0, "nope", true, 0, 0),
                RecordDefect::SnapshotHandle,
            ),
            (
                record_bytes("task_t", 0, "ws_s", false, 3, 0),
                RecordDefect::FrontierShape,
            ),
            (
                {
                    let mut bytes = record_bytes("task_t", 0, "ws_s", true, 0, 0);
                    bytes.extend_from_slice(&8_u64.to_be_bytes());
                    bytes.extend_from_slice(&1_i64.to_be_bytes());
                    bytes
                },
                RecordDefect::TrailingBytes,
            ),
            (
                {
                    let mut preimage = Preimage::new();
                    preimage.text("task_t");
                    preimage.push(&0_u64.to_be_bytes());
                    preimage.bytes().to_vec()
                },
                RecordDefect::FieldWidth,
            ),
            (
                {
                    let mut preimage = Preimage::new();
                    preimage.text("task_t");
                    preimage.push(&0_u32.to_be_bytes());
                    preimage.text("-");
                    preimage.push(&0_u64.to_be_bytes());
                    preimage.text("open");
                    preimage.push(&0_u64.to_be_bytes());
                    preimage.bytes().to_vec()
                },
                RecordDefect::ClosureToken,
            ),
        ];
        for (bytes, expected) in cases {
            assert_eq!(
                decode_campaign_record(&identity("r"), &bytes),
                Err(expected),
                "expected `{expected}`"
            );
        }
    }

    #[test]
    fn each_resolution_rule_fires_on_its_own_input_and_on_no_other() {
        let resolution = resolve_records(
            vec![
                // Parked: one bounded record.
                record("task_parked", "p0", 0, false),
                // Settled: bounded, then resumed to closure.
                record("task_settled", "s0", 0, false),
                record("task_settled", "s1", 1, true),
                // Gap: sequence 1 is missing.
                record("task_gap", "g0", 0, false),
                record("task_gap", "g2", 2, true),
                // Ambiguous: two different records at the head sequence.
                record("task_ambiguous", "a0", 0, false),
                record("task_ambiguous", "a1", 0, true),
            ],
            vec![(identity("junk"), RecordDefect::Truncated)],
        );
        let outcomes: Vec<(&str, Resolution)> = resolution
            .tasks()
            .iter()
            .map(|resolved| (resolved.task.as_str(), resolved.resolution))
            .collect();
        assert_eq!(
            outcomes,
            vec![
                (
                    "task_ambiguous",
                    Resolution::Failed(FailureReason::AmbiguousHead)
                ),
                ("task_gap", Resolution::Failed(FailureReason::SequenceGap)),
                (
                    "task_parked",
                    Resolution::Failed(FailureReason::ContinuationNotDurable)
                ),
                ("task_settled", Resolution::Settled),
            ]
        );
        assert_eq!(
            resolution.unattributed(),
            &[(identity("junk"), RecordDefect::Truncated)]
        );
        let settled = &resolution.tasks()[3];
        assert_eq!(settled.claims(), vec![identity("s0"), identity("s1")]);
    }

    #[test]
    fn the_resolution_is_independent_of_input_order() {
        let forward = vec![
            record("task_x", "x0", 0, false),
            record("task_x", "x1", 1, false),
            record("task_y", "y0", 0, true),
        ];
        let mut backward = forward.clone();
        backward.reverse();
        assert_eq!(
            resolve_records(forward, Vec::new()).render(),
            resolve_records(backward, Vec::new()).render()
        );
    }

    #[test]
    fn only_the_task_table_is_resolved_and_the_continuation_frontier_survives_in_the_record() {
        let resolved: Vec<&str> = VolatileFact::ALL
            .iter()
            .filter(|fact| fact.disposition() == Disposition::Resolved)
            .map(|fact| fact.token())
            .collect();
        assert_eq!(resolved, vec!["task-table"]);
        assert_eq!(
            VolatileFact::ContinuationTable.disposition(),
            Disposition::Declared,
            "a frontier without its pins is not a committed continuation"
        );
        assert_eq!(
            VolatileFact::ContinuationTable.survives_as(),
            Some(ArtifactClass::Task)
        );
    }
}
