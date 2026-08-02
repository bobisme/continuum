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
//!   workaround outright: a restart "MUST NOT reconstruct task state by inference". So this
//!   daemon does not resolve a `Running` task to `Failed`, because after the crash there is
//!   no task record to resolve — and it says so, per fact, in [`RecoveryReport::volatile`],
//!   rather than reporting an empty task table as though that were an achievement. What *is*
//!   recovered is the published half: every campaign record a task committed before the
//!   crash is in the store under the identity the task named it by
//!   ([`budget::publication_record`](super::budget::publication_record)), and
//!   [`RecoveryReport::surviving`] returns them.
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
//! failure; a daemon whose task table never reached a disk has no committed continuation to
//! resume from, and inventing one from the store would be the "silently reconstructed state"
//! the same paragraph prohibits. Persisting `DaemonState` is a real piece of work with its
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
}

impl Disposition {
    /// A stable token.
    #[must_use]
    pub const fn token(self) -> &'static str {
        match self {
            Self::Reprovisioned => "reprovisioned",
            Self::Declared => "declared",
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
            _ => Disposition::Declared,
        }
    }

    /// The artifact class whose surviving store artifacts are what remains of this fact, when
    /// any do.
    ///
    /// [`None`] is the common and honest case: most of `DaemonState` is daemon-side
    /// bookkeeping with no published counterpart at all, and naming a class for it would
    /// suggest a recovery path that does not exist. The two that do have one are the workspace
    /// records — a *sealed* workspace's records are in the store, which is what sealing means
    /// — and the task table, whose committed campaign records are published under
    /// [`ArtifactClass::Task`].
    #[must_use]
    pub const fn survives_as(self) -> Option<ArtifactClass> {
        match self {
            Self::WorkspaceRecords => Some(ArtifactClass::WorkspaceSnapshot),
            Self::TaskTable => Some(ArtifactClass::Task),
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
}
