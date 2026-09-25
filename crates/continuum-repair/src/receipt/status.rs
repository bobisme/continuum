//! PR-22 / IMPL-05: the receipt's refinement and certificate status, derived from the
//! evidence the daemon holds for the candidate snapshot.
//!
//! # Clause → mechanism
//!
//! | Clause | Source | Mechanism |
//! |---|---|---|
//! | gate 8 passes iff the computed coverage delta does not decrease; an uncomputable delta is `inconclusive`, never `passed` | RFC 0032 "Gate 8 is a computed delta"; correction 7 | no refinement checker is deployed (`continuum-refinement` is the PR-17 scaffold), so [`RefinementStatus`] has one member, [`RefinementStatus::Absent`]. `verify_*` refuses a receipt that lists gate 8 `passed` ([`super::VerifyRefusal::GateWithoutEvidence`]) and one that carries a `coverage.refinement_coverage` group ([`super::VerifyRefusal::CoverageNotDerived`]) |
//! | gate 9 is enforced against RFC 0030's freshness predicate over the invalidated certificates | RFC 0032 "Gate 9"; gate table | the invalidation set is not computed (no incremental engine, PR 23), so [`RebuildStatus`] has one member, [`RebuildStatus::Absent`]; gate 9 listed `passed` and a `coverage.certificate_rebuild` group are refused the same way |
//! | a certificate supports a claim about the daemon's model only when the kernel verified it **and** its carried model is the model the daemon holds for the snapshot | RFC 0005 correction 1; bn-35y4f, bn-3hk4v, bn-iu8eh | [`CertificateStatus::VerifiedModelBound`] is reachable only from [`CertificateVerdict::ModelBound`] naming the candidate snapshot itself; every other verdict, and a model-bound verdict about another snapshot, is [`CertificateStatus::Unverified`] with a typed [`UnverifiedReason`] |
//! | the receipt accepts no client-declared status | RFC 0032 "Receipts are … checkable by reference"; PR 22 exit | the status is derived from [`super::CandidateEvidence`], a daemon store, and nothing in a claimed receipt is read into it |
//! | typed inconclusiveness | INV-008 | no status is a boolean; an absence names its reason, and a store that did not answer is not a store that said no |
//!
//! # What the receipt renders
//!
//! Nothing in `coverage`: the schema's `refinement_coverage` group requires numbers no
//! checker computes, and `certificate_rebuild` requires an invalidation count no engine
//! computes. Rendering either would state a measurement nobody made. Every absence and
//! every certificate that did not verify model-bound is instead an entry of `unknowns`
//! (PR-22 / IMPL-06, [`super::unknowns`]).

use core::fmt;

use crate::handle::{MalformedHandle, SnapshotId};
use crate::transaction::GateName;

/// The most certificate records one answer of [`super::CandidateEvidence::certificates`]
/// may hold. It is checked before the answer is sorted or indexed. With
/// [`super::unknowns::MAX_PACK_CASES`] it keeps the largest derived `unknowns` array
/// inside [`super::MAX_CLAIMED_RECEIPT_BYTES`], so a receipt the daemon composes is one
/// its own reader accepts (`super::unknowns::MAX_UNKNOWNS_BYTES`).
pub const MAX_CANDIDATE_CERTIFICATES: usize = 2048;

/// The longest certificate handle [`CertificateNode::new`] accepts, in bytes.
pub const MAX_CERTIFICATE_HANDLE_BYTES: usize = 256;

/// The evidence-graph handle of a certificate node: the receipt schema's generic
/// `$defs.handle` pattern, `^[a-z][a-z0-9_]*_[A-Za-z0-9_-]+$`, at most
/// [`MAX_CERTIFICATE_HANDLE_BYTES`] bytes.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CertificateNode(String);

impl CertificateNode {
    /// The schema pattern.
    pub const PATTERN: &'static str = "^[a-z][a-z0-9_]*_[A-Za-z0-9_-]+$";

    /// Validate and wrap a handle.
    ///
    /// # Errors
    ///
    /// [`MalformedHandle`] when the value does not match [`Self::PATTERN`] or is longer
    /// than [`MAX_CERTIFICATE_HANDLE_BYTES`]. The value is not echoed.
    pub fn new(handle: &str) -> Result<Self, MalformedHandle> {
        if handle.len() <= MAX_CERTIFICATE_HANDLE_BYTES && matches_generic_handle(handle) {
            Ok(Self(handle.to_owned()))
        } else {
            Err(MalformedHandle {
                pattern: Self::PATTERN,
            })
        }
    }

    /// The handle.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// `^[a-z][a-z0-9_]*_[A-Za-z0-9_-]+$`. Every character before the first `_` is in the
/// prefix, so the pattern matches iff it matches when split at the first `_`.
fn matches_generic_handle(handle: &str) -> bool {
    let bytes = handle.as_bytes();
    let Some((&first, _)) = bytes.split_first() else {
        return false;
    };
    if !first.is_ascii_lowercase() {
        return false;
    }
    let Some(split) = bytes.iter().position(|b| *b == b'_') else {
        return false;
    };
    let (prefix, rest) = bytes.split_at(split);
    let suffix = &rest[1..];
    prefix[1..]
        .iter()
        .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
        && !suffix.is_empty()
        && suffix
            .iter()
            .all(|b| b.is_ascii_alphanumeric() || *b == b'_' || *b == b'-')
}

/// Why `evidence.verify` answered `InsufficientEvidence` for a certificate node
/// (`continuumd` `daemon/evidence.rs`, bn-3hk4v). Each is a distinct absence (INV-008).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum InsufficientReason {
    /// The kernel verified the claim but the claim trusts a model correspondence it did
    /// not check: wire epoch 1, or a state-type claim.
    TrustsModelCorrespondence,
    /// The certificate family has no model-bound path: temporal, SAT (LRAT) or SMT.
    FamilyNotModelBound,
    /// The node names no sealed snapshot, or more than one.
    NoSealedSnapshot,
    /// The daemon holds no model for the snapshot the node names.
    NoHeldModel,
    /// The daemon does not hold the content the node names.
    ContentMissing,
    /// The checker class this evidence needs is not one the daemon serves.
    CheckerNotServed,
    /// The certificate's wire magic or epoch routes to no kernel.
    WireUnsupported,
}

/// What the daemon's own verification settled for one certificate node. Only
/// [`Self::ModelBound`] can make a certificate count, and a daemon store answers it only
/// for a node that `evidence.verify` settled `validated` through the model-binding path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CertificateVerdict {
    /// The routed kernel returned `Verified` for a wire-epoch-2 finite-closure claim, and
    /// the carried `continuum-model/1` encoding equals the canonical identity of the model
    /// the daemon holds for `snapshot`, byte for byte.
    ModelBound {
        /// The sealed snapshot whose model the certificate is bound to.
        snapshot: SnapshotId,
    },
    /// The kernel rejected the certificate, or its model is not the held model
    /// (`CertificateRejected`).
    Rejected,
    /// `InsufficientEvidence`, with its reason.
    InsufficientEvidence(InsufficientReason),
    /// The routed kernel ran and could not decide (`Checked(Unsupported)`).
    KernelInconclusive,
    /// The node's referenced content is redacted; the daemon reports the redaction and
    /// promotes nothing (RFC 0026 "Redacted values").
    Redacted,
    /// The node was settled `validated`, but under a checker identity or an epoch that is
    /// no longer the daemon's current one (RFC 0030 freshness; RFC 0032 gate 9). The store
    /// answers this, never [`Self::ModelBound`], for a stale node.
    Stale,
    /// No daemon verification has settled the node: it was attached and never verified.
    Unchecked,
}

/// One certificate node the daemon holds for the candidate, with its verdict.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CertificateRecord {
    node: CertificateNode,
    verdict: CertificateVerdict,
}

impl CertificateRecord {
    /// A store record.
    #[must_use]
    pub const fn new(node: CertificateNode, verdict: CertificateVerdict) -> Self {
        Self { node, verdict }
    }
}

/// Why a certificate does not count for the receipt.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum UnverifiedReason {
    /// No daemon verification has settled it.
    Unchecked,
    /// The daemon does not hold its content.
    ContentMissing,
    /// The kernel rejected it, or its model is not the held model.
    Rejected,
    /// It trusts a model correspondence nobody checked.
    TrustsModelCorrespondence,
    /// Its family has no model-bound path.
    FamilyNotModelBound,
    /// It names no single sealed snapshot.
    NoSealedSnapshot,
    /// The daemon holds no model for its snapshot.
    NoHeldModel,
    /// It is model-bound, but to a snapshot other than the candidate.
    BoundToOtherSnapshot,
    /// The checker class it needs is not served.
    CheckerNotServed,
    /// Its wire magic or epoch routes to no kernel.
    WireUnsupported,
    /// The kernel could not decide it.
    KernelInconclusive,
    /// Its content is redacted.
    Redacted,
    /// It was validated under a checker identity or epoch that is no longer current.
    Stale,
}

impl UnverifiedReason {
    /// Every reason.
    pub const ALL: [Self; 13] = [
        Self::Unchecked,
        Self::ContentMissing,
        Self::Rejected,
        Self::TrustsModelCorrespondence,
        Self::FamilyNotModelBound,
        Self::NoSealedSnapshot,
        Self::NoHeldModel,
        Self::BoundToOtherSnapshot,
        Self::CheckerNotServed,
        Self::WireUnsupported,
        Self::KernelInconclusive,
        Self::Redacted,
        Self::Stale,
    ];

    /// The token the `unknowns` entry carries.
    #[must_use]
    pub const fn token(self) -> &'static str {
        match self {
            Self::Unchecked => "unchecked",
            Self::ContentMissing => "content_missing",
            Self::Rejected => "rejected",
            Self::TrustsModelCorrespondence => "trusts_model_correspondence",
            Self::FamilyNotModelBound => "family_not_model_bound",
            Self::NoSealedSnapshot => "no_sealed_snapshot",
            Self::NoHeldModel => "no_held_model",
            Self::BoundToOtherSnapshot => "bound_to_other_snapshot",
            Self::CheckerNotServed => "checker_not_served",
            Self::WireUnsupported => "wire_unsupported",
            Self::KernelInconclusive => "kernel_inconclusive",
            Self::Redacted => "redacted",
            Self::Stale => "stale",
        }
    }

    pub(crate) fn from_token(token: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|reason| reason.token() == token)
    }
}

/// A certificate's standing on the receipt.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CertificateStatus {
    /// Kernel-verified and bound to the model the daemon holds for the candidate.
    VerifiedModelBound,
    /// Not verified for the candidate's model, and why.
    Unverified(UnverifiedReason),
}

/// One certificate node and its status. Built only by the derivation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CertificateEntry {
    node: CertificateNode,
    status: CertificateStatus,
}

impl CertificateEntry {
    /// The certificate node.
    #[must_use]
    pub const fn node(&self) -> &CertificateNode {
        &self.node
    }

    /// Its status.
    #[must_use]
    pub const fn status(&self) -> CertificateStatus {
        self.status
    }
}

/// Why no refinement coverage was computed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RefinementAbsence {
    /// This deployment has no refinement checker (`continuum-refinement` is the PR-17
    /// scaffold): unsupported semantics, not a zero delta.
    CheckerNotDeployed,
}

impl RefinementAbsence {
    /// The token the `unknowns` entry carries.
    #[must_use]
    pub const fn token(self) -> &'static str {
        match self {
            Self::CheckerNotDeployed => "checker_not_deployed",
        }
    }
}

/// The refinement-coverage status (gate 8). There is no computed member: nothing in this
/// deployment computes a coverage delta, so no receipt can carry one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RefinementStatus {
    /// Not computed, and why.
    Absent(RefinementAbsence),
}

/// Why no certificate-rebuild coverage was computed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RebuildAbsence {
    /// No engine computes the set of certificates the patch invalidates (RFC 0030, PR 23),
    /// so `invalidated`, `rebuilt` and `stale` have no source.
    InvalidationSetNotComputed,
}

impl RebuildAbsence {
    /// The token the `unknowns` entry carries.
    #[must_use]
    pub const fn token(self) -> &'static str {
        match self {
            Self::InvalidationSetNotComputed => "invalidation_set_not_computed",
        }
    }
}

/// The certificate-rebuild status (gate 9). There is no computed member.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RebuildStatus {
    /// Not computed, and why.
    Absent(RebuildAbsence),
}

/// A `coverage` group this module owns. A claim that carries one is refused: neither can
/// be derived today.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CoverageGroup {
    /// `coverage.refinement_coverage` (gate 8).
    RefinementCoverage,
    /// `coverage.certificate_rebuild` (gate 9).
    CertificateRebuild,
}

impl CoverageGroup {
    /// Both groups.
    pub const ALL: [Self; 2] = [Self::RefinementCoverage, Self::CertificateRebuild];

    /// The schema property inside `coverage`.
    #[must_use]
    pub const fn property(self) -> &'static str {
        match self {
            Self::RefinementCoverage => "refinement_coverage",
            Self::CertificateRebuild => "certificate_rebuild",
        }
    }

    /// The gate whose evidence the group is.
    #[must_use]
    pub const fn gate(self) -> GateName {
        match self {
            Self::RefinementCoverage => GateName::RefinementCoverage,
            Self::CertificateRebuild => GateName::CertificateRebuild,
        }
    }
}

/// What the evidence derives for one [`CoverageGroup`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GroupDerivation {
    /// Nothing: the group has no source. A claim carrying it is refused, and its gate
    /// listed `passed` is refused.
    Absent,
}

/// Why the daemon's evidence answer could not be used. A store defect, refused rather
/// than trusted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum EvidenceKind {
    /// The certificate records.
    Certificates,
    /// The unsupported pack cases.
    PackCases,
}

/// A defect in a store answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EvidenceDefect {
    /// More entries than the bound; nothing was sorted.
    OverBound(EvidenceKind),
    /// The same entry twice.
    Duplicate(EvidenceKind),
}

/// The receipt's refinement and certificate status (PR-22 / IMPL-05). Built only by the
/// derivation in [`super::ReceiptSkeleton::compose`] and verification; no constructor
/// takes a status.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RefinementCertificateStatus {
    refinement: RefinementStatus,
    rebuild: RebuildStatus,
    certificates: Vec<CertificateEntry>,
}

impl RefinementCertificateStatus {
    /// Derive the status from the daemon's certificate records for `candidate`.
    ///
    /// The bound is checked before the records are sorted; a handle listed twice is a
    /// store defect.
    pub(crate) fn derive(
        mut records: Vec<CertificateRecord>,
        candidate: &SnapshotId,
    ) -> Result<Self, EvidenceDefect> {
        if records.len() > MAX_CANDIDATE_CERTIFICATES {
            return Err(EvidenceDefect::OverBound(EvidenceKind::Certificates));
        }
        records.sort_by(|a, b| a.node.cmp(&b.node));
        if records.windows(2).any(|pair| pair[0].node == pair[1].node) {
            return Err(EvidenceDefect::Duplicate(EvidenceKind::Certificates));
        }
        let certificates = records
            .into_iter()
            .map(|record| {
                let status = match record.verdict {
                    CertificateVerdict::ModelBound { snapshot } if &snapshot == candidate => {
                        CertificateStatus::VerifiedModelBound
                    }
                    CertificateVerdict::ModelBound { .. } => {
                        CertificateStatus::Unverified(UnverifiedReason::BoundToOtherSnapshot)
                    }
                    CertificateVerdict::Rejected => {
                        CertificateStatus::Unverified(UnverifiedReason::Rejected)
                    }
                    CertificateVerdict::Unchecked => {
                        CertificateStatus::Unverified(UnverifiedReason::Unchecked)
                    }
                    CertificateVerdict::KernelInconclusive => {
                        CertificateStatus::Unverified(UnverifiedReason::KernelInconclusive)
                    }
                    CertificateVerdict::Redacted => {
                        CertificateStatus::Unverified(UnverifiedReason::Redacted)
                    }
                    CertificateVerdict::Stale => {
                        CertificateStatus::Unverified(UnverifiedReason::Stale)
                    }
                    CertificateVerdict::InsufficientEvidence(reason) => {
                        CertificateStatus::Unverified(match reason {
                            InsufficientReason::TrustsModelCorrespondence => {
                                UnverifiedReason::TrustsModelCorrespondence
                            }
                            InsufficientReason::FamilyNotModelBound => {
                                UnverifiedReason::FamilyNotModelBound
                            }
                            InsufficientReason::NoSealedSnapshot => {
                                UnverifiedReason::NoSealedSnapshot
                            }
                            InsufficientReason::NoHeldModel => UnverifiedReason::NoHeldModel,
                            InsufficientReason::ContentMissing => UnverifiedReason::ContentMissing,
                            InsufficientReason::CheckerNotServed => {
                                UnverifiedReason::CheckerNotServed
                            }
                            InsufficientReason::WireUnsupported => {
                                UnverifiedReason::WireUnsupported
                            }
                        })
                    }
                };
                CertificateEntry {
                    node: record.node,
                    status,
                }
            })
            .collect();
        Ok(Self {
            refinement: RefinementStatus::Absent(RefinementAbsence::CheckerNotDeployed),
            rebuild: RebuildStatus::Absent(RebuildAbsence::InvalidationSetNotComputed),
            certificates,
        })
    }

    /// The refinement-coverage status (gate 8).
    #[must_use]
    pub const fn refinement(&self) -> RefinementStatus {
        self.refinement
    }

    /// The certificate-rebuild status (gate 9).
    #[must_use]
    pub const fn rebuild(&self) -> RebuildStatus {
        self.rebuild
    }

    /// Every certificate node the daemon holds for the candidate, by handle.
    #[must_use]
    pub fn certificates(&self) -> &[CertificateEntry] {
        &self.certificates
    }

    /// The certificates that count: kernel-verified and bound to the candidate's model.
    pub fn verified(&self) -> impl Iterator<Item = &CertificateNode> {
        self.certificates
            .iter()
            .filter(|entry| entry.status == CertificateStatus::VerifiedModelBound)
            .map(|entry| &entry.node)
    }

    /// What the evidence derives for a coverage group. Always
    /// [`GroupDerivation::Absent`] today. The type has no presence-only member: when a
    /// checker ships, its member must carry the derived values, so verification compares
    /// a claimed group's values rather than its presence.
    #[must_use]
    pub const fn coverage(&self, group: CoverageGroup) -> GroupDerivation {
        match group {
            CoverageGroup::RefinementCoverage => match self.refinement {
                RefinementStatus::Absent(_) => GroupDerivation::Absent,
            },
            CoverageGroup::CertificateRebuild => match self.rebuild {
                RebuildStatus::Absent(_) => GroupDerivation::Absent,
            },
        }
    }
}

impl fmt::Display for CoverageGroup {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "coverage.{}", self.property())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(text: &str) -> CertificateNode {
        CertificateNode::new(text).unwrap()
    }

    fn snapshot(text: &str) -> SnapshotId {
        SnapshotId::new(text).unwrap()
    }

    #[test]
    fn the_generic_handle_pattern_is_the_schemas() {
        for good in ["ev_a", "ev_A-b_c", "a1_x", "cert_x_y", "a__b", "a_-"] {
            assert!(CertificateNode::new(good).is_ok(), "{good}");
        }
        for bad in [
            "", "ev_", "_ev", "Ev_a", "1v_a", "eva", "ev-a_b", "ev_a:b", "ev_a b", "eV_a",
        ] {
            assert!(CertificateNode::new(bad).is_err(), "{bad}");
        }
        let long = format!("ev_{}", "a".repeat(MAX_CERTIFICATE_HANDLE_BYTES - 3));
        assert!(CertificateNode::new(&long).is_ok());
        assert!(CertificateNode::new(&format!("{long}a")).is_err());
    }

    /// pr22-impl05: every verdict maps to exactly one status, and only a model-bound
    /// verdict naming the candidate itself verifies.
    #[test]
    fn pr22_impl05_only_a_model_bound_verdict_on_the_candidate_verifies() {
        let candidate = snapshot("ws_candidate");
        let cases = [
            (
                CertificateVerdict::ModelBound {
                    snapshot: candidate.clone(),
                },
                CertificateStatus::VerifiedModelBound,
            ),
            (
                CertificateVerdict::ModelBound {
                    snapshot: snapshot("ws_base"),
                },
                CertificateStatus::Unverified(UnverifiedReason::BoundToOtherSnapshot),
            ),
            (
                CertificateVerdict::Rejected,
                CertificateStatus::Unverified(UnverifiedReason::Rejected),
            ),
            (
                CertificateVerdict::Unchecked,
                CertificateStatus::Unverified(UnverifiedReason::Unchecked),
            ),
            (
                CertificateVerdict::InsufficientEvidence(
                    InsufficientReason::TrustsModelCorrespondence,
                ),
                CertificateStatus::Unverified(UnverifiedReason::TrustsModelCorrespondence),
            ),
            (
                CertificateVerdict::InsufficientEvidence(InsufficientReason::FamilyNotModelBound),
                CertificateStatus::Unverified(UnverifiedReason::FamilyNotModelBound),
            ),
            (
                CertificateVerdict::InsufficientEvidence(InsufficientReason::NoSealedSnapshot),
                CertificateStatus::Unverified(UnverifiedReason::NoSealedSnapshot),
            ),
            (
                CertificateVerdict::InsufficientEvidence(InsufficientReason::NoHeldModel),
                CertificateStatus::Unverified(UnverifiedReason::NoHeldModel),
            ),
            (
                CertificateVerdict::InsufficientEvidence(InsufficientReason::ContentMissing),
                CertificateStatus::Unverified(UnverifiedReason::ContentMissing),
            ),
            (
                CertificateVerdict::InsufficientEvidence(InsufficientReason::CheckerNotServed),
                CertificateStatus::Unverified(UnverifiedReason::CheckerNotServed),
            ),
            (
                CertificateVerdict::InsufficientEvidence(InsufficientReason::WireUnsupported),
                CertificateStatus::Unverified(UnverifiedReason::WireUnsupported),
            ),
            (
                CertificateVerdict::KernelInconclusive,
                CertificateStatus::Unverified(UnverifiedReason::KernelInconclusive),
            ),
            (
                CertificateVerdict::Redacted,
                CertificateStatus::Unverified(UnverifiedReason::Redacted),
            ),
            (
                CertificateVerdict::Stale,
                CertificateStatus::Unverified(UnverifiedReason::Stale),
            ),
        ];
        for (verdict, expected) in cases {
            let status = RefinementCertificateStatus::derive(
                vec![CertificateRecord::new(node("ev_one"), verdict.clone())],
                &candidate,
            )
            .unwrap();
            assert_eq!(status.certificates()[0].status(), expected, "{verdict:?}");
            assert_eq!(
                status.verified().count(),
                usize::from(expected == CertificateStatus::VerifiedModelBound)
            );
        }
        for reason in UnverifiedReason::ALL {
            assert_eq!(UnverifiedReason::from_token(reason.token()), Some(reason));
        }
    }

    #[test]
    fn pr22_impl05_the_record_bound_is_checked_and_duplicates_are_refused() {
        let candidate = snapshot("ws_candidate");
        let at_bound: Vec<CertificateRecord> = (0..MAX_CANDIDATE_CERTIFICATES)
            .map(|i| {
                CertificateRecord::new(node(&format!("ev_{i}")), CertificateVerdict::Unchecked)
            })
            .collect();
        assert!(RefinementCertificateStatus::derive(at_bound.clone(), &candidate).is_ok());
        let mut over = at_bound;
        over.push(CertificateRecord::new(
            node("ev_over"),
            CertificateVerdict::Unchecked,
        ));
        assert_eq!(
            RefinementCertificateStatus::derive(over, &candidate),
            Err(EvidenceDefect::OverBound(EvidenceKind::Certificates))
        );
        let twice = vec![
            CertificateRecord::new(node("ev_a"), CertificateVerdict::Unchecked),
            CertificateRecord::new(node("ev_a"), CertificateVerdict::Rejected),
        ];
        assert_eq!(
            RefinementCertificateStatus::derive(twice, &candidate),
            Err(EvidenceDefect::Duplicate(EvidenceKind::Certificates))
        );
    }

    #[test]
    fn pr22_impl05_no_coverage_group_is_derivable_today() {
        let status = RefinementCertificateStatus::derive(Vec::new(), &snapshot("ws_c")).unwrap();
        for group in CoverageGroup::ALL {
            assert_eq!(status.coverage(group), GroupDerivation::Absent);
        }
        assert_eq!(
            status.refinement(),
            RefinementStatus::Absent(RefinementAbsence::CheckerNotDeployed)
        );
    }
}
