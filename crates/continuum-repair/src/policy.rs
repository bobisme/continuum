//! The policy verdict (PR-20 / IMPL-06): a pure, deterministic function from a
//! transaction version's recorded gate statuses, under its gate profile, to a typed
//! verdict bound to that version's `rt_` identity. Never a bare boolean (INV-008).
//!
//! # What the RFC makes the verdict
//!
//! > **Promotion is recomputed, never reported.** `repair.promote` re-computes the policy
//! > verdict server-side from current evidence. No cached, client-supplied, or
//! > agent-asserted verdict is ever trusted (INV-015, B12).
//! >
//! > **An absent gate is never a passed gate.**
//! >
//! > — RFC 0032, "Summary"
//!
//! > 6. any in-profile gate `failed`, or `policy_verdict` is not `allow` ⇒ `blocked`;
//! > 7. any in-profile gate `inconclusive` ⇒ `inconclusive`;
//! > 8. otherwise ⇒ `ready`.
//! >
//! > Failure dominates inconclusiveness: rule 6 precedes rule 7.
//! >
//! > **Gate 12 is the promotion step, not a precondition of it.** `receipt_generation`
//! > is `pending` on a `ready` transaction […] so rule 8 is evaluated over gates 1–11 of
//! > the active profile.
//! >
//! > — RFC 0032, "Status machine"
//!
//! # Clause → mechanism
//!
//! | Clause | Source | Mechanism |
//! |---|---|---|
//! | the verdict is recomputed from the record, never read from a claim | RFC 0032 Summary, "Promotion" step 2 | [`PolicyVerdict::compute`] reads only the version's gates, profile and the typed reasons its evidence records; [`PolicyVerdict::recheck`] recomputes and refuses a claim that disagrees, [`ClaimRefusal::Disagrees`] |
//! | only `passed` supports promotion; `failed`, `pending`, `inconclusive` do not | RFC 0032 "Gate status semantics" | [`decide`]: a gate 1–11 of the profile that is not `passed` never yields [`Verdict::PromoteEligible`] |
//! | failure dominates inconclusiveness | RFC 0032 rules 6, 7 | a failed gate yields [`Verdict::Blocked`] with every failed gate and every undecided gate |
//! | an `inconclusive` gate carries its typed INV-008 reason | RFC 0032 "Gate status semantics"; INV-008 | [`Standing::Inconclusive`] carries the reason the gate's cited evidence records, or the typed absence of one ([`Resolution::Unknown`], [`Resolution::Unavailable`]); never a guess |
//! | gate 12 is the promotion step | RFC 0032 "Status machine" | `receipt_generation` `pending` does not block eligibility; `failed` blocks and `inconclusive` is undecided, as for any gate |
//! | `not_yet_enforced` supports promotion only under a profile that excludes the gate | RFC 0032 "Gate status semantics", "Gate profiles" | a gate list whose in-profile gate is `not_yet_enforced`, or whose out-of-profile gate is anything else, is refused, [`GateListRefusal::ProfileViolation`] |
//! | fail closed on a gate list that is not the twelve gates by identity | RFC 0032 "Versioning and revision" | [`GateListRefusal::Count`], [`GateListRefusal::Duplicate`], [`GateListRefusal::Missing`] |
//! | a protected change is a privileged intent revision, never a repair | RFC 0032 "Intent integrity and reclassification"; INV-001, INV-011 | gate 3 `failed` (its claim, "RFC 0031 classifies no protected change", refuted), or a bound RFC 0031 classification of `review` or `block`, yields [`Verdict::PrivilegedIntentRevisionRequired`], whatever the other gates say |
//! | the verdict is `allow`/`review`/`block`, with no `unknown` value | RFC 0031 "Policy verdict"; IDL `PolicyDecision` | [`Verdict::policy_decision`]: `allow` only for [`Verdict::PromoteEligible`]; no decision for [`Verdict::Inconclusive`] |
//! | the verdict is the version's, and a stale one is refused | RFC 0032 "Concurrency and lineage", "Promotion" step 1 | [`PolicyVerdict`] binds the `rt_` handle and the canonical identity bytes; [`PolicyVerdict::check_current`] refuses any other version, [`StaleVerdict`] |
//! | the hypothesis never contributes to the verdict | RFC 0032 correction 12 | nothing here reads it |
//! | no ambient nondeterminism | INV-005 | the verdict is a function of the gate readings; the gate order of the input does not matter |
//!
//! # Who may authorize `ContinueAndDisclose`
//!
//! Exact replay ([`crate::replay`]) runs gate 4 under the fail-closed
//! [`EvaluationPolicy::Exact`](crate::replay::EvaluationPolicy::Exact) unless a
//! [`PolicyAuthorization`] selects `ContinueAndDisclose`. This module is the only
//! production path that may issue one (the crate-private `PolicyAuthorization::issue` is
//! test-only) ([`authorize_continue_and_disclose`]), and it issues one only on a
//! [`ContinueAndDiscloseGrant`]: an explicit, protected, signed policy input. **No RFC
//! defines that input**, so the type has no values and the answer is always
//! [`AuthorizationAbsence::NoAuthority`]. RFC 0032 names the policy
//! ("continues under the transaction's declared `evaluation_policy`") but does not say
//! who declares it; its open questions leave "the policy DSL surface" and "what
//! `evaluation_policy` should reference" open (flag F3). The decision bn-2vanm must make,
//! in an RFC 0032 correction, before any value of the grant can exist:
//!
//! 1. whether gate 4 may pass at all on a clean candidate run with eliminated or
//!    reordered recorded steps, or only on an exact run (a relaxation of the gate as the
//!    code enforces it today, so it goes to the user);
//! 2. what artifact declares the policy: a content-addressed policy document that
//!    `evaluation_policy` references, and whether it is protected like the Intent
//!    Contract (RFC 0037's revision procedure) or is transaction-scoped;
//! 3. which RFC 0027 authority may declare it (`promote`, `revise-intent`, or a human
//!    only), and which service identity signs it;
//! 4. what it binds: the authorization binds today the `rt_` version, the base and
//!    candidate snapshots and the base intent; whether it must also bind the replayer
//!    identity and the crashpack.
//!
//! # Not here
//!
//! - The verdict is not written into the transaction artifact. `policy_verdict` and the
//!   statuses `ready`, `blocked` and `inconclusive` would move every version's `rt_`
//!   identity, and which status a persisted partial campaign has is an open RFC 0032
//!   question (bn-2vanm). [`crate::transaction::derive_status`] is unchanged.
//! - No production path computes it: `repair.promote` is not served (bn-23pwm).
//! - Gate 3's recorded classification is IMPL-04's (bn-1b69). Until it lands, a caller
//!   hands the RFC 0031 decision in as an [`IntentClassification`] bound to the version's
//!   base, candidate and intent. It can only make the verdict stricter: `allow` never
//!   overrides a recorded gate-3 status, and `review` never lowers a failed gate's
//!   `block`. [`PolicyVerdict::intent_classification`] states whether one was consulted:
//!   RFC 0032 "Promotion" step 2 requires the promotion path to re-classify, so a
//!   verdict computed without one is not promotion-grade once IMPL-04 lands.
//! - A gate-3 `inconclusive` is an undecided gate like any other (rule 7), not a
//!   protected change: RFC 0031 fails a non-affirmative relation closed to `review` or
//!   `block`, which gate 3 records as `failed`, so an `inconclusive` gate 3 is a
//!   classification that did not run, and the verdict does not guess its direction.
//! - The typed reasons of gates other than 1 and 4 are read through [`GateReasons`];
//!   [`ReplayRecords`] reads the exact-replay records only.

use core::fmt;
use core::marker::PhantomData;
use std::collections::BTreeMap;

use continuum_intent::change_policy::PolicyDecision;
use continuum_intent::contract::IntentId;
use continuum_value::assurance::InconclusiveReason;
use continuum_value::identity::ContentHasher;

use crate::handle::{EvidenceRef, RepairId, SnapshotId};
use crate::replay::{AuthorizationAbsence, EvidenceRecord, PolicyAuthorization};
use crate::transaction::{
    GateName, GateProfile, GateStatus, RepairIdentity, RepairTransaction, Resolution,
};

// --- the pure core -------------------------------------------------------------------

/// One gate as the verdict reads it: the recorded status and, for an `inconclusive`
/// gate, the typed INV-008 reason its evidence records. The reason of any other status
/// is not read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GateReading {
    /// The gate.
    pub gate: GateName,
    /// Its recorded status.
    pub status: GateStatus,
    /// For an `inconclusive` gate: the recorded reason, or why there is none.
    pub reason: Resolution<InconclusiveReason>,
}

/// Where an undecided gate stands.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Standing {
    /// In the profile and not yet evaluated.
    Pending,
    /// Evaluated, and the evidence cannot decide: the typed INV-008 reason the evidence
    /// records ([`Resolution::Found`]), or its typed absence. An absent reason is never
    /// filled in.
    Inconclusive(Resolution<InconclusiveReason>),
}

/// A gate of the profile that neither passed nor failed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Undecided {
    /// The gate.
    pub gate: GateName,
    /// Where it stands.
    pub standing: Standing,
}

/// The policy verdict over one gate list. Every list is in gate-number order, whatever
/// the order of the input.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verdict {
    /// Every gate 1–11 of the profile passed, gate 12 is `pending` or `passed`, and no
    /// protected change is classified: ordinary promotion may proceed. `allow`.
    PromoteEligible,
    /// A protected change: gate 3 failed, or a bound RFC 0031 classification is `review`
    /// or `block`. Ordinary repair authority cannot promote it, whatever the other gates
    /// say; the only way on is an explicit reclassification through RFC 0037's revision
    /// procedure, which re-bases the repair as a new transaction (RFC 0032, INV-001,
    /// INV-011).
    PrivilegedIntentRevisionRequired {
        /// The RFC 0031 decision, `review` or `block`, when a bound classification of a
        /// protected change was supplied; `None` when only the recorded gate-3 failure is
        /// known.
        classification: Option<PolicyDecision>,
        /// Every failed gate, gate 3 included when it failed.
        failed: Vec<GateName>,
        /// Every undecided gate.
        undecided: Vec<Undecided>,
    },
    /// A gate failed. `block`.
    Blocked {
        /// Every failed gate. Never empty.
        failed: Vec<GateName>,
        /// Every undecided gate, disclosed beside the failure.
        undecided: Vec<Undecided>,
    },
    /// No gate failed, and a gate is pending or inconclusive. Not eligible, and no
    /// `PolicyDecision`: RFC 0031 has no `unknown` decision.
    Inconclusive {
        /// Every undecided gate. Never empty.
        undecided: Vec<Undecided>,
    },
}

impl Verdict {
    /// The transaction-level decision this verdict carries, in the IDL's closed
    /// `PolicyDecision` vocabulary: `allow` exactly for [`Self::PromoteEligible`]; `block`
    /// for [`Self::Blocked`]; none for [`Self::Inconclusive`], since RFC 0031 has no
    /// `unknown` decision. For [`Self::PrivilegedIntentRevisionRequired`] it is the join
    /// (RFC 0031 P5, `allow < review < block`) of `block` for every failed gate, gate 3
    /// included, and the classification's decision: so `review` only when no gate failed
    /// and a bound classification says `review`. A classification never lowers it.
    ///
    /// This is not RFC 0031's per-field decision over a diff, which names its reasons per
    /// field; it is the verdict's aggregate, which the failed and undecided gates explain.
    /// Whether the transaction's `policy_verdict` field carries this value or the RFC 0031
    /// decision alone is open (RFC 0032 rule 6 reads both), and nothing writes it yet.
    #[must_use]
    pub fn policy_decision(&self) -> Option<PolicyDecision> {
        match self {
            Self::PromoteEligible => Some(PolicyDecision::Allow),
            Self::PrivilegedIntentRevisionRequired {
                classification,
                failed,
                ..
            } => Some(if failed.is_empty() {
                classification
                    .filter(|decision| *decision != PolicyDecision::Allow)
                    .unwrap_or(PolicyDecision::Block)
            } else {
                PolicyDecision::Block
            }),
            Self::Blocked { .. } => Some(PolicyDecision::Block),
            Self::Inconclusive { .. } => None,
        }
    }
}

/// Why a gate list has no verdict. The list is not the twelve gates by identity under
/// the profile, and it is not read further (RFC 0032: fail closed, never ignore).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GateListRefusal {
    /// More than twelve entries.
    Count {
        /// The number of entries.
        entries: usize,
    },
    /// A gate is listed twice.
    Duplicate {
        /// The gate.
        gate: GateName,
    },
    /// A gate is not listed. An absent gate is never a passed gate.
    Missing {
        /// The first missing gate, in gate-number order.
        gate: GateName,
    },
    /// An in-profile gate is `not_yet_enforced`, or an out-of-profile gate is not.
    ProfileViolation {
        /// The gate.
        gate: GateName,
        /// Its status.
        status: GateStatus,
    },
}

impl fmt::Display for GateListRefusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Count { entries } => write!(f, "{entries} gate entries, not twelve"),
            Self::Duplicate { gate } => write!(f, "gate `{}` is listed twice", gate.token()),
            Self::Missing { gate } => write!(f, "gate `{}` is not listed", gate.token()),
            Self::ProfileViolation { gate, status } => write!(
                f,
                "gate `{}` is `{}`, which its profile does not admit",
                gate.token(),
                status.token()
            ),
        }
    }
}

impl std::error::Error for GateListRefusal {}

const fn gate_index(gate: GateName) -> usize {
    match gate {
        GateName::BaseReplay => 0,
        GateName::PatchApplication => 1,
        GateName::IntentIntegrity => 2,
        GateName::ExactRegression => 3,
        GateName::Neighborhood => 4,
        GateName::PropertyMutation => 5,
        GateName::DefectMutants => 6,
        GateName::RefinementCoverage => 7,
        GateName::CertificateRebuild => 8,
        GateName::IncrementalParity => 9,
        GateName::CodeAndSecurity => 10,
        GateName::ReceiptGeneration => 11,
    }
}

/// The verdict over `gates` under `profile`, with `classification` the RFC 0031
/// decision of the candidate's diff when one is known.
///
/// Pure and total over its input: the same readings, in any order, give the same
/// verdict. It binds no transaction: a verdict a promotion may rely on is a
/// [`PolicyVerdict`], computed from a version's own record.
///
/// # Errors
///
/// [`GateListRefusal`] when `gates` is not the twelve gates by identity, each admitted by
/// `profile`. The length is checked before any entry is read.
pub fn decide(
    profile: GateProfile,
    gates: &[GateReading],
    classification: Option<PolicyDecision>,
) -> Result<Verdict, GateListRefusal> {
    if gates.len() > GateName::ALL.len() {
        return Err(GateListRefusal::Count {
            entries: gates.len(),
        });
    }
    let mut slots: [Option<&GateReading>; 12] = [None; 12];
    for reading in gates {
        let slot = &mut slots[gate_index(reading.gate)];
        if slot.is_some() {
            return Err(GateListRefusal::Duplicate { gate: reading.gate });
        }
        *slot = Some(reading);
    }
    let mut failed = Vec::new();
    let mut undecided = Vec::new();
    for (gate, slot) in GateName::ALL.into_iter().zip(slots) {
        let Some(reading) = slot else {
            return Err(GateListRefusal::Missing { gate });
        };
        let in_profile = profile.enforces(gate);
        if in_profile == (reading.status == GateStatus::NotYetEnforced) {
            return Err(GateListRefusal::ProfileViolation {
                gate,
                status: reading.status,
            });
        }
        match reading.status {
            GateStatus::Passed | GateStatus::NotYetEnforced => {}
            GateStatus::Failed => failed.push(gate),
            // Gate 12 is `pending` until promotion composes the receipt.
            GateStatus::Pending if gate == GateName::ReceiptGeneration => {}
            GateStatus::Pending => undecided.push(Undecided {
                gate,
                standing: Standing::Pending,
            }),
            GateStatus::Inconclusive => undecided.push(Undecided {
                gate,
                standing: Standing::Inconclusive(reading.reason.clone()),
            }),
        }
    }
    // An `allow` classification overrides nothing, so it is not carried.
    let classification = classification.filter(|decision| *decision != PolicyDecision::Allow);
    let protected = failed.contains(&GateName::IntentIntegrity) || classification.is_some();
    Ok(if protected {
        Verdict::PrivilegedIntentRevisionRequired {
            classification,
            failed,
            undecided,
        }
    } else if !failed.is_empty() {
        Verdict::Blocked { failed, undecided }
    } else if !undecided.is_empty() {
        Verdict::Inconclusive { undecided }
    } else {
        Verdict::PromoteEligible
    })
}

// --- the typed reasons of inconclusive gates ----------------------------------------

/// Where the typed INV-008 reason of an `inconclusive` gate is read from. The schema's
/// gate entry has no field for it: it is in the evidence the gate cites.
///
/// `Found` only for the reason the cited evidence records; `Unknown` when the evidence
/// was read and records none (or disagrees with itself); `Unavailable` when the evidence
/// could not be read. None of them is ever a default.
pub trait GateReasons {
    /// The reason `gate`, `inconclusive` on this version, records in `evidence`.
    fn inconclusive_reason(
        &self,
        gate: GateName,
        evidence: &[EvidenceRef],
    ) -> Resolution<InconclusiveReason>;
}

/// No evidence store: every reason is [`Resolution::Unavailable`].
#[derive(Debug, Clone, Copy, Default)]
pub struct NoReasons;

impl GateReasons for NoReasons {
    fn inconclusive_reason(
        &self,
        _: GateName,
        _: &[EvidenceRef],
    ) -> Resolution<InconclusiveReason> {
        Resolution::Unavailable
    }
}

/// The records an exact-replay evaluation made ([`crate::replay::ExactReplay::into_parts`]),
/// read for the reasons of gates 1 and 4.
///
/// A record counts only when its bytes re-derive to its handle under `H` and it states
/// the gate asked about, `inconclusive`, and a reason. The records are indexed once, so
/// each lookup is logarithmic.
pub struct ReplayRecords<'a, H: ContentHasher> {
    by_handle: BTreeMap<&'a str, &'a EvidenceRecord>,
    hasher: PhantomData<fn() -> H>,
}

impl<'a, H: ContentHasher> ReplayRecords<'a, H> {
    /// Index `records` by handle.
    #[must_use]
    pub fn new(records: &'a [EvidenceRecord]) -> Self {
        let mut by_handle = BTreeMap::new();
        for record in records {
            by_handle.entry(record.handle().as_str()).or_insert(record);
        }
        Self {
            by_handle,
            hasher: PhantomData,
        }
    }
}

impl<H: ContentHasher> fmt::Debug for ReplayRecords<'_, H> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ReplayRecords")
            .field("records", &self.by_handle.len())
            .finish()
    }
}

impl<H: ContentHasher> GateReasons for ReplayRecords<'_, H> {
    fn inconclusive_reason(
        &self,
        gate: GateName,
        evidence: &[EvidenceRef],
    ) -> Resolution<InconclusiveReason> {
        let mut read_any = false;
        let mut missing = false;
        let mut reason = None;
        for cited in evidence {
            let Some(record) = self.by_handle.get(cited.as_str()) else {
                // The crashpack cite (`crash_`) is not a record; a missing replay record
                // is.
                let handle = cited.as_str();
                missing |= handle.starts_with("ev_") || handle.starts_with("defect_");
                continue;
            };
            read_any = true;
            match record.statement::<H>() {
                Some((stated, GateStatus::Inconclusive, Some(stated_reason))) if stated == gate => {
                    if reason.is_some_and(|known| known != stated_reason) {
                        return Resolution::Unknown;
                    }
                    reason = Some(stated_reason);
                }
                _ => return Resolution::Unknown,
            }
        }
        match (read_any, missing, reason) {
            // Some of the cited records were not supplied: what they state is unread.
            (true, true, _) => Resolution::Unknown,
            (false, _, _) => Resolution::Unavailable,
            (true, false, Some(reason)) => Resolution::Found(reason),
            (true, false, None) => Resolution::Unknown,
        }
    }
}

// --- gate 3's classification ----------------------------------------------------------

/// The RFC 0031 decision over the candidate's semantic and intent diff, bound to the
/// transaction it classifies: the base snapshot, the candidate snapshot and the base
/// intent.
///
/// A seam until IMPL-04 records gate 3 (bn-1b69). It can only make a verdict stricter: a
/// `review` or `block` forces the privileged path, and an `allow` overrides nothing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IntentClassification {
    base_snapshot: SnapshotId,
    candidate_snapshot: SnapshotId,
    base_intent: IntentId,
    decision: PolicyDecision,
}

impl IntentClassification {
    /// The decision `decision` over the diff from `base_snapshot` under `base_intent` to
    /// `candidate_snapshot`.
    #[must_use]
    pub const fn new(
        base_snapshot: SnapshotId,
        candidate_snapshot: SnapshotId,
        base_intent: IntentId,
        decision: PolicyDecision,
    ) -> Self {
        Self {
            base_snapshot,
            candidate_snapshot,
            base_intent,
            decision,
        }
    }

    /// The RFC 0031 decision.
    #[must_use]
    pub const fn decision(&self) -> PolicyDecision {
        self.decision
    }
}

/// Which bound field of a classification does not match the version.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClassificationMismatch {
    /// Another base snapshot.
    BaseSnapshot,
    /// Another candidate snapshot, or the version has none.
    CandidateSnapshot,
    /// Another base intent.
    Intent,
}

// --- the bound verdict ----------------------------------------------------------------

/// Why no verdict was computed for a version.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VerdictRefusal {
    /// The version's gate list is not admissible under its profile.
    GateList(GateListRefusal),
    /// The classification is of another transaction.
    ForeignClassification(ClassificationMismatch),
    /// A gate 1–11 is recorded `passed` with no evidence, which RFC 0032 prohibits.
    PassedWithoutEvidence {
        /// The gate.
        gate: GateName,
    },
}

impl fmt::Display for VerdictRefusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::GateList(refusal) => write!(f, "{refusal}"),
            Self::ForeignClassification(mismatch) => {
                write!(
                    f,
                    "the classification is not of this transaction: {mismatch:?}"
                )
            }
            Self::PassedWithoutEvidence { gate } => {
                write!(f, "gate `{}` is passed with no evidence", gate.token())
            }
        }
    }
}

impl std::error::Error for VerdictRefusal {}

/// A verdict held for another version than the one presented.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StaleVerdict {
    /// The version the verdict was computed for.
    pub verdict_for: RepairId,
    /// The version presented.
    pub presented: RepairId,
}

impl fmt::Display for StaleVerdict {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "the verdict is for {}, not {}",
            self.verdict_for, self.presented
        )
    }
}

impl std::error::Error for StaleVerdict {}

/// A version's policy verdict, bound to that version.
///
/// Built only by [`Self::compute`] and [`Self::recheck`], from the version's own record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PolicyVerdict {
    repair_id: RepairId,
    identity: RepairIdentity,
    gate_profile: GateProfile,
    classification: Option<PolicyDecision>,
    verdict: Verdict,
}

impl PolicyVerdict {
    /// Compute the verdict of `transaction` from its recorded gates under its profile,
    /// with the reasons of its `inconclusive` gates read through `reasons` and the RFC
    /// 0031 decision of `classification` when supplied.
    ///
    /// # Errors
    ///
    /// [`VerdictRefusal::ForeignClassification`] when `classification` is bound to another
    /// base, candidate or intent; [`VerdictRefusal::PassedWithoutEvidence`] when a gate
    /// 1–11 passed on no evidence; [`VerdictRefusal::GateList`] when the gate list is not
    /// admissible (no version this crate builds is).
    pub fn compute<H: ContentHasher>(
        transaction: &RepairTransaction<H>,
        reasons: &impl GateReasons,
        classification: Option<&IntentClassification>,
    ) -> Result<Self, VerdictRefusal> {
        if let Some(classification) = classification {
            if &classification.base_snapshot != transaction.base_snapshot() {
                return Err(VerdictRefusal::ForeignClassification(
                    ClassificationMismatch::BaseSnapshot,
                ));
            }
            if transaction.candidate_snapshot() != Some(&classification.candidate_snapshot) {
                return Err(VerdictRefusal::ForeignClassification(
                    ClassificationMismatch::CandidateSnapshot,
                ));
            }
            if &classification.base_intent != transaction.base_intent() {
                return Err(VerdictRefusal::ForeignClassification(
                    ClassificationMismatch::Intent,
                ));
            }
        }
        // RFC 0032: "A gate cannot pass on absent evidence" for gates 1–11.
        if let Some(gate) = transaction.gates().iter().find(|gate| {
            gate.name() != GateName::ReceiptGeneration
                && gate.status() == GateStatus::Passed
                && gate.evidence().is_empty()
        }) {
            return Err(VerdictRefusal::PassedWithoutEvidence { gate: gate.name() });
        }
        let profile = transaction.gate_profile();
        let readings: Vec<GateReading> = transaction
            .gates()
            .iter()
            .map(|gate| GateReading {
                gate: gate.name(),
                status: gate.status(),
                reason: if gate.status() == GateStatus::Inconclusive {
                    reasons.inconclusive_reason(gate.name(), gate.evidence())
                } else {
                    Resolution::Unknown
                },
            })
            .collect();
        let verdict = decide(
            profile,
            &readings,
            classification.map(IntentClassification::decision),
        )
        .map_err(VerdictRefusal::GateList)?;
        Ok(Self {
            repair_id: transaction.repair_id().clone(),
            identity: transaction.identity().clone(),
            gate_profile: profile,
            classification: classification.map(IntentClassification::decision),
            verdict,
        })
    }

    /// Recompute the verdict of `head` and hold `claim` against it. The recomputed verdict
    /// is returned; the claim is never.
    ///
    /// # Errors
    ///
    /// [`ClaimRefusal::Stale`] when the claim names another version than `head`, before
    /// anything is computed; [`ClaimRefusal::Verdict`] when no verdict can be computed;
    /// [`ClaimRefusal::Disagrees`] when the claimed decision is not the recomputed one,
    /// including any decision claimed for an inconclusive version.
    pub fn recheck<H: ContentHasher>(
        claim: &VerdictClaim,
        head: &RepairTransaction<H>,
        reasons: &impl GateReasons,
        classification: Option<&IntentClassification>,
    ) -> Result<Self, ClaimRefusal> {
        if &claim.repair_id != head.repair_id() {
            return Err(ClaimRefusal::Stale(StaleVerdict {
                verdict_for: claim.repair_id.clone(),
                presented: head.repair_id().clone(),
            }));
        }
        let recomputed =
            Self::compute(head, reasons, classification).map_err(ClaimRefusal::Verdict)?;
        if recomputed.verdict.policy_decision() != Some(claim.decision) {
            return Err(ClaimRefusal::Disagrees {
                claimed: claim.decision,
                recomputed: Box::new(recomputed),
            });
        }
        Ok(recomputed)
    }

    /// Refuse this verdict for any version but the one it was computed for: another
    /// transaction, or a later (or earlier) version of this one.
    ///
    /// # Errors
    ///
    /// [`StaleVerdict`] when `head` is another version, by handle or by identity bytes.
    pub fn check_current<H: ContentHasher>(
        &self,
        head: &RepairTransaction<H>,
    ) -> Result<(), StaleVerdict> {
        if &self.repair_id == head.repair_id() && &self.identity == head.identity() {
            Ok(())
        } else {
            Err(StaleVerdict {
                verdict_for: self.repair_id.clone(),
                presented: head.repair_id().clone(),
            })
        }
    }

    /// The version this verdict is for.
    #[must_use]
    pub const fn repair_id(&self) -> &RepairId {
        &self.repair_id
    }

    /// The profile it was computed under.
    #[must_use]
    pub const fn gate_profile(&self) -> GateProfile {
        self.gate_profile
    }

    /// The RFC 0031 decision of the bound classification the verdict consulted, or
    /// `None` when it consulted none and rests on the recorded gate-3 status alone.
    #[must_use]
    pub const fn intent_classification(&self) -> Option<PolicyDecision> {
        self.classification
    }

    /// The verdict.
    #[must_use]
    pub const fn verdict(&self) -> &Verdict {
        &self.verdict
    }
}

/// A verdict someone asserts for a version: a cached, client-supplied or agent-asserted
/// `PolicyDecision`. It is data to be checked, never a verdict
/// ([`PolicyVerdict::recheck`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerdictClaim {
    repair_id: RepairId,
    decision: PolicyDecision,
}

impl VerdictClaim {
    /// The claim that `repair_id`'s verdict is `decision`.
    #[must_use]
    pub const fn new(repair_id: RepairId, decision: PolicyDecision) -> Self {
        Self {
            repair_id,
            decision,
        }
    }
}

/// Why a claimed verdict was refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClaimRefusal {
    /// The claim is for another version than the head.
    Stale(StaleVerdict),
    /// No verdict could be recomputed.
    Verdict(VerdictRefusal),
    /// The recomputed verdict does not carry the claimed decision.
    Disagrees {
        /// The decision claimed.
        claimed: PolicyDecision,
        /// The verdict recomputed from the record, with its reasons.
        recomputed: Box<PolicyVerdict>,
    },
}

impl fmt::Display for ClaimRefusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Stale(stale) => write!(f, "stale verdict claim: {stale}"),
            Self::Verdict(refusal) => write!(f, "no verdict: {refusal}"),
            Self::Disagrees {
                claimed,
                recomputed,
            } => match recomputed.verdict.policy_decision() {
                Some(decision) => write!(
                    f,
                    "claimed `{}`, recomputed `{}`",
                    claimed.wire(),
                    decision.wire()
                ),
                None => write!(f, "claimed `{}`, recomputed inconclusive", claimed.wire()),
            },
        }
    }
}

impl std::error::Error for ClaimRefusal {}

// --- the ContinueAndDisclose authority -------------------------------------------------

/// The explicit, protected, signed policy input that alone could authorize
/// `ContinueAndDisclose` for a transaction.
///
/// **It has no values.** No RFC defines the input yet (the module docs list the decision
/// bn-2vanm must make), so nothing can be passed to
/// [`authorize_continue_and_disclose`] but `None`, and the compiler proves no path issues
/// an authorization. Adding a variant is the change that grants the authority, and it
/// must cite the RFC 0032 correction that defines it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContinueAndDiscloseGrant {}

/// Issue a [`PolicyAuthorization`] for `transaction` under `ContinueAndDisclose`, on
/// `grant`. The only place in the crate that may.
///
/// # Errors
///
/// [`AuthorizationAbsence::NoAuthority`], always: [`ContinueAndDiscloseGrant`] has no
/// values.
pub const fn authorize_continue_and_disclose<H: ContentHasher>(
    transaction: &RepairTransaction<H>,
    grant: Option<&ContinueAndDiscloseGrant>,
) -> Result<PolicyAuthorization, AuthorizationAbsence> {
    let _ = transaction;
    match grant {
        None => Err(AuthorizationAbsence::NoAuthority),
        Some(grant) => match *grant {},
    }
}

#[cfg(test)]
mod tests {
    //! The eligible path, which only this crate's tests can reach: in production only an
    //! evaluation writes a gate outcome, and no evaluation of gates 2, 3 and 5–11 exists
    //! yet. `with_recorded_gates_evidenced` records a gate list directly.

    use super::*;
    use crate::hypothesis::{ChangeKind, Hypothesis, Proposal};
    use crate::patch::{DeclaredChange, FileEdit};
    use crate::replay::{WorkspaceContent, WorkspacePath};
    use crate::transaction::FailureBinding;
    use continuum_value::identity::Blake3Hasher;

    type Tx = RepairTransaction<Blake3Hasher>;

    struct Binding;

    impl FailureBinding for Binding {
        fn failure_base(&self, _: &crate::handle::CrashpackId) -> Resolution<SnapshotId> {
            Resolution::Found(base_id())
        }

        fn snapshot_intent(&self, _: &SnapshotId) -> Resolution<IntentId> {
            Resolution::Found(IntentId::new("in_policy_unit").unwrap())
        }
    }

    fn base() -> WorkspaceContent {
        let mut content = WorkspaceContent::new();
        content
            .insert(WorkspacePath::new("src/a.rs").unwrap(), b"a".to_vec())
            .unwrap();
        content
    }

    fn base_id() -> SnapshotId {
        let tree = crate::replay::Snapshot::build(
            &base(),
            &crate::patch::HashedIdentifier::<Blake3Hasher>::new(),
        )
        .unwrap();
        SnapshotId::new(&tree.identity().to_string()).unwrap()
    }

    fn applied(profile: GateProfile) -> Tx {
        let draft = Tx::begin(
            crate::handle::CrashpackId::new("crash_unit").unwrap(),
            profile,
            &Binding,
        )
        .unwrap();
        let change = DeclaredChange::new(
            ChangeKind::Rust,
            [(
                WorkspacePath::new("src/b.rs").unwrap(),
                FileEdit::Create {
                    content: b"b".to_vec(),
                },
            )],
        )
        .unwrap();
        let proposal = Proposal::new(Hypothesis::new("unit"), vec![change]);
        draft.apply(&proposal, &base()).unwrap().into_parts().0
    }

    fn all_passed(profile: GateProfile) -> [GateStatus; 12] {
        GateName::ALL.map(|gate| {
            if !profile.enforces(gate) {
                GateStatus::NotYetEnforced
            } else if gate == GateName::ReceiptGeneration {
                GateStatus::Pending
            } else {
                GateStatus::Passed
            }
        })
    }

    #[test]
    fn every_required_gate_passed_is_promote_eligible_under_every_profile() {
        for profile in GateProfile::ALL {
            let tx = applied(profile).with_recorded_gates_evidenced(all_passed(profile));
            let verdict = PolicyVerdict::compute(&tx, &NoReasons, None).unwrap();
            assert_eq!(verdict.verdict(), &Verdict::PromoteEligible, "{profile:?}");
            assert_eq!(
                verdict.verdict().policy_decision(),
                Some(PolicyDecision::Allow)
            );
            assert_eq!(verdict.repair_id(), tx.repair_id());
            assert_eq!(verdict.gate_profile(), profile);
            verdict.check_current(&tx).unwrap();
            let claim = VerdictClaim::new(tx.repair_id().clone(), PolicyDecision::Allow);
            assert_eq!(
                PolicyVerdict::recheck(&claim, &tx, &NoReasons, None).unwrap(),
                verdict
            );
        }
    }

    #[test]
    fn a_verdict_for_one_version_is_stale_for_its_successor() {
        let profile = GateProfile::PhaseB;
        let eligible = applied(profile).with_recorded_gates_evidenced(all_passed(profile));
        let verdict = PolicyVerdict::compute(&eligible, &NoReasons, None).unwrap();
        let mut statuses = all_passed(profile);
        statuses[gate_index(GateName::Neighborhood)] = GateStatus::Failed;
        let other = applied(profile).with_recorded_gates_evidenced(statuses);
        assert_eq!(
            verdict.check_current(&other),
            Err(StaleVerdict {
                verdict_for: eligible.repair_id().clone(),
                presented: other.repair_id().clone(),
            })
        );
        let stale = VerdictClaim::new(eligible.repair_id().clone(), PolicyDecision::Allow);
        assert!(matches!(
            PolicyVerdict::recheck(&stale, &other, &NoReasons, None),
            Err(ClaimRefusal::Stale(_))
        ));
    }

    #[test]
    fn a_forged_allow_claim_is_refused_by_recompute() {
        let profile = GateProfile::PhaseC;
        let mut statuses = all_passed(profile);
        statuses[gate_index(GateName::DefectMutants)] = GateStatus::Failed;
        let tx = applied(profile).with_recorded_gates_evidenced(statuses);
        let forged = VerdictClaim::new(tx.repair_id().clone(), PolicyDecision::Allow);
        let Err(ClaimRefusal::Disagrees {
            claimed,
            recomputed,
        }) = PolicyVerdict::recheck(&forged, &tx, &NoReasons, None)
        else {
            panic!("a forged allow must be refused");
        };
        assert_eq!(claimed, PolicyDecision::Allow);
        assert_eq!(
            recomputed.verdict(),
            &Verdict::Blocked {
                failed: vec![GateName::DefectMutants],
                undecided: Vec::new(),
            }
        );
    }

    #[test]
    fn a_classification_of_another_transaction_is_refused() {
        let profile = GateProfile::PhaseB;
        let tx = applied(profile).with_recorded_gates_evidenced(all_passed(profile));
        let candidate = tx.candidate_snapshot().unwrap().clone();
        let intent = tx.base_intent().clone();
        let other = SnapshotId::new("ws_other").unwrap();
        let cases = [
            (
                IntentClassification::new(
                    other.clone(),
                    candidate.clone(),
                    intent.clone(),
                    PolicyDecision::Allow,
                ),
                ClassificationMismatch::BaseSnapshot,
            ),
            (
                IntentClassification::new(base_id(), other, intent.clone(), PolicyDecision::Allow),
                ClassificationMismatch::CandidateSnapshot,
            ),
            (
                IntentClassification::new(
                    base_id(),
                    candidate.clone(),
                    IntentId::new("in_other").unwrap(),
                    PolicyDecision::Allow,
                ),
                ClassificationMismatch::Intent,
            ),
        ];
        for (classification, mismatch) in cases {
            assert_eq!(
                PolicyVerdict::compute(&tx, &NoReasons, Some(&classification)),
                Err(VerdictRefusal::ForeignClassification(mismatch))
            );
        }
        // A bound `review` forces the privileged path even with every gate passed.
        let review =
            IntentClassification::new(base_id(), candidate, intent, PolicyDecision::Review);
        let verdict = PolicyVerdict::compute(&tx, &NoReasons, Some(&review)).unwrap();
        assert_eq!(
            verdict.verdict(),
            &Verdict::PrivilegedIntentRevisionRequired {
                classification: Some(PolicyDecision::Review),
                failed: Vec::new(),
                undecided: Vec::new(),
            }
        );
        assert_eq!(
            verdict.verdict().policy_decision(),
            Some(PolicyDecision::Review)
        );
    }

    #[test]
    fn no_grant_exists_so_no_authorization_is_issued() {
        let tx = applied(GateProfile::PhaseB);
        assert_eq!(
            authorize_continue_and_disclose(&tx, None),
            Err(AuthorizationAbsence::NoAuthority)
        );
        assert_eq!(
            crate::replay::request_continue_and_disclose(&tx),
            Err(AuthorizationAbsence::NoAuthority)
        );
    }

    #[test]
    fn a_passed_gate_without_evidence_is_refused() {
        let profile = GateProfile::PhaseB;
        let bare = applied(profile).with_recorded_gates(all_passed(profile));
        assert_eq!(
            PolicyVerdict::compute(&bare, &NoReasons, None),
            Err(VerdictRefusal::PassedWithoutEvidence {
                gate: GateName::BaseReplay
            })
        );
    }

    #[test]
    fn a_review_classification_never_lowers_a_failed_gate_3() {
        let profile = GateProfile::PhaseB;
        let mut statuses = all_passed(profile);
        statuses[gate_index(GateName::IntentIntegrity)] = GateStatus::Failed;
        let tx = applied(profile).with_recorded_gates_evidenced(statuses);
        let review = IntentClassification::new(
            base_id(),
            tx.candidate_snapshot().unwrap().clone(),
            tx.base_intent().clone(),
            PolicyDecision::Review,
        );
        let verdict = PolicyVerdict::compute(&tx, &NoReasons, Some(&review)).unwrap();
        assert_eq!(
            verdict.intent_classification(),
            Some(PolicyDecision::Review)
        );
        assert_eq!(
            verdict.verdict().policy_decision(),
            Some(PolicyDecision::Block)
        );
        let without = PolicyVerdict::compute(&tx, &NoReasons, None).unwrap();
        assert_eq!(without.intent_classification(), None);
        assert_eq!(
            without.verdict().policy_decision(),
            Some(PolicyDecision::Block)
        );
        let claim = VerdictClaim::new(tx.repair_id().clone(), PolicyDecision::Review);
        assert!(matches!(
            PolicyVerdict::recheck(&claim, &tx, &NoReasons, Some(&review)),
            Err(ClaimRefusal::Disagrees { .. })
        ));
    }
}
