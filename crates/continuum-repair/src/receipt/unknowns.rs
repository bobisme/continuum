//! PR-22 / IMPL-06: the receipt's `unknowns` — every unresolved unknown, explicitly
//! (RFC 0032 receipt composition table; INV-007; INV-008).
//!
//! # Clause → mechanism
//!
//! | Clause | Source | Mechanism |
//! |---|---|---|
//! | every unresolved unknown is listed; an absent array is not an empty one | RFC 0032 receipt table and correction 11; INV-007 | [`Unknowns`] is derived from the transaction record, the [`RefinementCertificateStatus`] and the daemon's pack cases by one function, whose `match`es have no wildcard arm; a claim without `unknowns` is refused `Missing` |
//! | an unknown cannot be omitted, invented or reordered | RFC 0032 "checkable by reference" | `verify_*` re-derives the list and compares it with the claim entry for entry ([`super::VerifyRefusal::UnknownOmitted`], [`super::VerifyRefusal::UnknownNotDerived`], [`super::VerifyRefusal::UnknownDuplicated`], [`super::VerifyRefusal::UnknownsNotCanonical`]) |
//! | distinct outcomes stay distinct | INV-008 | each [`Unknown`] renders one token whose kind prefix names its reason; [`Unknown::from_token`] inverts [`Unknown::token`], so the rendering is injective |
//!
//! # Token grammar
//!
//! The schema types an entry as a string. Each entry is one of:
//!
//! ```text
//! gate_not_yet_enforced:<gate>
//! gate_pending:<gate>
//! gate_inconclusive:<gate>
//! refinement_not_computed:<refinement absence>
//! certificate_rebuild_not_computed:<rebuild absence>
//! certificate_none_verified
//! certificate_unverified:<certificate handle>:<unverified reason>
//! unsupported_pack_case:<pack>:<case>
//! semantic_diff_unclassified:program_layer
//! semantic_diff_undiffable:<undiffable cause>
//! ```
//!
//! No field can hold `:`: gate, reason and cause tokens are closed sets, a certificate handle
//! matches the schema's handle pattern, and a pack or case token is `[a-z0-9./-]`.
//!
//! Canonical order is byte-lexicographic order of the rendered entries, so an independent
//! verifier can reproduce it from the strings alone.
//!
//! # Derivation, one input to one entry
//!
//! | Input | Entry |
//! |---|---|
//! | a gate outside the profile | `gate_not_yet_enforced` |
//! | a gate in the profile, other than gate 12, recorded `pending` | `gate_pending` |
//! | a gate in the profile recorded `inconclusive` | `gate_inconclusive` |
//! | refinement coverage absent while gate 8 is in the profile | `refinement_not_computed` |
//! | certificate rebuild absent while gate 9 is in the profile | `certificate_rebuild_not_computed` (outside the profile the gate's `gate_not_yet_enforced` is its one entry) |
//! | no certificate verified model-bound | `certificate_none_verified` |
//! | a certificate not verified model-bound | `certificate_unverified` |
//! | a declared unsupported case of an effect pack the candidate uses | `unsupported_pack_case` |
//! | the semantic diff computed with its program-side layer not classified (PR-22 / IMPL-03) | `semantic_diff_unclassified` |
//! | no semantic diff computable from the stored inputs (PR-22 / IMPL-03) | `semantic_diff_undiffable` |
//!
//! Gate 12 `pending` is the receipt step itself, not an unknown carried into promotion;
//! a gate recorded `passed` or `failed` is decided. A record whose status disagrees with
//! the profile (an in-profile gate `not_yet_enforced`, or an unenforced gate with any
//! other status) is refused as a record defect rather than classified.

use core::fmt;
use std::collections::BTreeMap;

use continuum_intent::canonical_json::Json;

use super::semantic_diff::{DiffGap, UndiffableCause};
use super::status::{
    CertificateNode, CertificateStatus, EvidenceDefect, EvidenceKind, RebuildAbsence,
    RebuildStatus, RefinementAbsence, RefinementCertificateStatus, RefinementStatus,
    UnverifiedReason,
};
use crate::transaction::{GateName, GateProfile, GateStatus};

/// The most pack cases one answer of [`super::CandidateEvidence::unsupported_pack_cases`]
/// may hold. It is checked before the answer is sorted.
pub const MAX_PACK_CASES: usize = 1024;

/// The longest pack or case token, in bytes.
pub const MAX_PACK_TOKEN_BYTES: usize = 64;

/// An upper bound on the canonical bytes of the largest derived `unknowns` array: every
/// entry at its longest, each with two quotes and a comma. Twelve gate entries, two
/// coverage entries, `certificate_none_verified` and the one semantic-diff entry are
/// bounded by 64 bytes each.
pub const MAX_UNKNOWNS_BYTES: usize = 2
    + 16 * 64
    + super::status::MAX_CANDIDATE_CERTIFICATES
        * ("certificate_unverified:".len()
            + super::status::MAX_CERTIFICATE_HANDLE_BYTES
            + ":trusts_model_correspondence".len()
            + 3)
    + MAX_PACK_CASES * ("unsupported_pack_case:".len() + 2 * MAX_PACK_TOKEN_BYTES + 1 + 3);

// A receipt the daemon composes at the bounds is one its own reader accepts, with room
// for the other fields.
const _: () = assert!(MAX_UNKNOWNS_BYTES < super::MAX_CLAIMED_RECEIPT_BYTES * 4 / 5);

/// A pack or case token that is empty, too long, or outside `[a-z0-9./-]`, or that does
/// not start with `[a-z0-9]`. The value is not echoed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MalformedPackCase;

impl fmt::Display for MalformedPackCase {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "a pack case token is not 1 to {MAX_PACK_TOKEN_BYTES} bytes of [a-z0-9./-]"
        )
    }
}

impl std::error::Error for MalformedPackCase {}

fn check_pack_token(token: &str) -> Result<(), MalformedPackCase> {
    let bytes = token.as_bytes();
    let well_formed = !bytes.is_empty()
        && bytes.len() <= MAX_PACK_TOKEN_BYTES
        && (bytes[0].is_ascii_lowercase() || bytes[0].is_ascii_digit())
        && bytes.iter().all(|b| {
            b.is_ascii_lowercase() || b.is_ascii_digit() || matches!(b, b'.' | b'/' | b'-')
        });
    if well_formed {
        Ok(())
    } else {
        Err(MalformedPackCase)
    }
}

/// One declared unsupported case of an effect pack (PR-15 / IMPL-04): the pack profile,
/// for example `storage/append-log-v1`, and the case, for example `flush-dishonesty`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PackCase {
    pack: String,
    case: String,
}

impl PackCase {
    /// Validate and wrap a pack case.
    ///
    /// # Errors
    ///
    /// [`MalformedPackCase`] when either token is off its grammar.
    pub fn new(pack: &str, case: &str) -> Result<Self, MalformedPackCase> {
        check_pack_token(pack)?;
        check_pack_token(case)?;
        Ok(Self {
            pack: pack.to_owned(),
            case: case.to_owned(),
        })
    }

    /// The pack profile.
    #[must_use]
    pub fn pack(&self) -> &str {
        &self.pack
    }

    /// The case.
    #[must_use]
    pub fn case(&self) -> &str {
        &self.case
    }
}

/// The reason class of an unknown: what a refusal may name without echoing a claim.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum UnknownKind {
    /// A gate outside the profile.
    GateNotYetEnforced,
    /// An in-profile gate not evaluated.
    GatePending,
    /// An in-profile gate whose evidence could not decide.
    GateInconclusive,
    /// Refinement coverage not computed.
    RefinementNotComputed,
    /// Certificate-rebuild coverage not computed.
    CertificateRebuildNotComputed,
    /// No certificate verified model-bound.
    NoVerifiedCertificate,
    /// A certificate not verified model-bound.
    CertificateUnverified,
    /// A declared unsupported case of an effect pack.
    UnsupportedPackCase,
    /// The semantic diff's program-side layer is not classified.
    SemanticDiffUnclassified,
    /// No semantic diff could be computed.
    SemanticDiffUndiffable,
}

impl UnknownKind {
    /// The token prefix.
    #[must_use]
    pub const fn prefix(self) -> &'static str {
        match self {
            Self::GateNotYetEnforced => "gate_not_yet_enforced",
            Self::GatePending => "gate_pending",
            Self::GateInconclusive => "gate_inconclusive",
            Self::RefinementNotComputed => "refinement_not_computed",
            Self::CertificateRebuildNotComputed => "certificate_rebuild_not_computed",
            Self::NoVerifiedCertificate => "certificate_none_verified",
            Self::CertificateUnverified => "certificate_unverified",
            Self::UnsupportedPackCase => "unsupported_pack_case",
            Self::SemanticDiffUnclassified => "semantic_diff_unclassified",
            Self::SemanticDiffUndiffable => "semantic_diff_undiffable",
        }
    }
}

/// One unresolved unknown the receipt carries into promotion.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Unknown {
    /// A gate outside the profile: nothing checked it.
    GateNotYetEnforced(GateName),
    /// An in-profile gate recorded `pending`.
    GatePending(GateName),
    /// An in-profile gate recorded `inconclusive`.
    GateInconclusive(GateName),
    /// Refinement coverage was not computed.
    RefinementNotComputed(RefinementAbsence),
    /// Certificate-rebuild coverage was not computed.
    CertificateRebuildNotComputed(RebuildAbsence),
    /// No certificate the daemon holds for the candidate verified model-bound, including
    /// the case where it holds none.
    NoVerifiedCertificate,
    /// A certificate that did not verify model-bound.
    CertificateUnverified {
        /// The certificate node.
        certificate: CertificateNode,
        /// Why it does not count.
        reason: UnverifiedReason,
    },
    /// A declared unsupported case of an effect pack.
    UnsupportedPackCase(PackCase),
    /// The semantic diff was computed with its program-side layer not classified: no
    /// program change was classified, so no semantic preservation is established.
    SemanticDiffUnclassified,
    /// No semantic diff could be computed from the stored inputs.
    SemanticDiffUndiffable(UndiffableCause),
}

/// The only field of `semantic_diff_unclassified`: which layer is not classified.
const PROGRAM_LAYER: &str = "program_layer";

fn gate_from_token(token: &str) -> Option<GateName> {
    GateName::ALL.into_iter().find(|gate| gate.token() == token)
}

impl Unknown {
    /// The reason class.
    #[must_use]
    pub const fn kind(&self) -> UnknownKind {
        match self {
            Self::GateNotYetEnforced(_) => UnknownKind::GateNotYetEnforced,
            Self::GatePending(_) => UnknownKind::GatePending,
            Self::GateInconclusive(_) => UnknownKind::GateInconclusive,
            Self::RefinementNotComputed(_) => UnknownKind::RefinementNotComputed,
            Self::CertificateRebuildNotComputed(_) => UnknownKind::CertificateRebuildNotComputed,
            Self::NoVerifiedCertificate => UnknownKind::NoVerifiedCertificate,
            Self::CertificateUnverified { .. } => UnknownKind::CertificateUnverified,
            Self::UnsupportedPackCase(_) => UnknownKind::UnsupportedPackCase,
            Self::SemanticDiffUnclassified => UnknownKind::SemanticDiffUnclassified,
            Self::SemanticDiffUndiffable(_) => UnknownKind::SemanticDiffUndiffable,
        }
    }

    /// The `unknowns[]` entry.
    #[must_use]
    pub fn token(&self) -> String {
        let prefix = self.kind().prefix();
        match self {
            Self::GateNotYetEnforced(gate)
            | Self::GatePending(gate)
            | Self::GateInconclusive(gate) => format!("{prefix}:{}", gate.token()),
            Self::RefinementNotComputed(absence) => format!("{prefix}:{}", absence.token()),
            Self::CertificateRebuildNotComputed(absence) => {
                format!("{prefix}:{}", absence.token())
            }
            Self::NoVerifiedCertificate => prefix.to_owned(),
            Self::CertificateUnverified {
                certificate,
                reason,
            } => format!("{prefix}:{}:{}", certificate.as_str(), reason.token()),
            Self::UnsupportedPackCase(case) => format!("{prefix}:{}:{}", case.pack, case.case),
            Self::SemanticDiffUnclassified => format!("{prefix}:{PROGRAM_LAYER}"),
            Self::SemanticDiffUndiffable(cause) => format!("{prefix}:{}", cause.token()),
        }
    }

    /// Read an entry back. The inverse of [`Self::token`]: `None` for any string no
    /// unknown renders.
    #[must_use]
    pub fn from_token(token: &str) -> Option<Self> {
        let mut parts = token.split(':');
        let prefix = parts.next()?;
        let fields: Vec<&str> = parts.collect();
        let unknown = match (prefix, fields.as_slice()) {
            ("gate_not_yet_enforced", [gate]) => Self::GateNotYetEnforced(gate_from_token(gate)?),
            ("gate_pending", [gate]) => Self::GatePending(gate_from_token(gate)?),
            ("gate_inconclusive", [gate]) => Self::GateInconclusive(gate_from_token(gate)?),
            ("refinement_not_computed", ["checker_not_deployed"]) => {
                Self::RefinementNotComputed(RefinementAbsence::CheckerNotDeployed)
            }
            ("certificate_rebuild_not_computed", ["invalidation_set_not_computed"]) => {
                Self::CertificateRebuildNotComputed(RebuildAbsence::InvalidationSetNotComputed)
            }
            ("certificate_none_verified", []) => Self::NoVerifiedCertificate,
            ("certificate_unverified", [certificate, reason]) => Self::CertificateUnverified {
                certificate: CertificateNode::new(certificate).ok()?,
                reason: UnverifiedReason::from_token(reason)?,
            },
            ("unsupported_pack_case", [pack, case]) => {
                Self::UnsupportedPackCase(PackCase::new(pack, case).ok()?)
            }
            ("semantic_diff_unclassified", [PROGRAM_LAYER]) => Self::SemanticDiffUnclassified,
            ("semantic_diff_undiffable", [cause]) => {
                Self::SemanticDiffUndiffable(UndiffableCause::from_token(cause)?)
            }
            _ => return None,
        };
        Some(unknown)
    }
}

/// A transaction record whose gate status disagrees with its profile.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum UnknownsDefect {
    /// The gate's recorded status contradicts the profile.
    GateRecord(GateName),
    /// A store answer is unusable.
    Evidence(EvidenceDefect),
}

/// The receipt's `unknowns` (PR-22 / IMPL-06): sorted, each entry once. Built only by
/// the derivation; no constructor takes an entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unknowns {
    entries: Vec<Unknown>,
}

impl Unknowns {
    /// Derive the list. Each input contributes exactly one entry (module table).
    pub(crate) fn derive(
        profile: GateProfile,
        gates: [(GateName, GateStatus); 12],
        status: &RefinementCertificateStatus,
        mut pack_cases: Vec<PackCase>,
        diff: Option<DiffGap>,
    ) -> Result<Self, UnknownsDefect> {
        if pack_cases.len() > MAX_PACK_CASES {
            return Err(UnknownsDefect::Evidence(EvidenceDefect::OverBound(
                EvidenceKind::PackCases,
            )));
        }
        pack_cases.sort();
        if pack_cases.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err(UnknownsDefect::Evidence(EvidenceDefect::Duplicate(
                EvidenceKind::PackCases,
            )));
        }

        let mut entries = Vec::new();
        for (name, recorded) in gates {
            let entry = match (profile.enforces(name), recorded) {
                (false, GateStatus::NotYetEnforced) => Some(Unknown::GateNotYetEnforced(name)),
                (
                    false,
                    GateStatus::Passed
                    | GateStatus::Failed
                    | GateStatus::Pending
                    | GateStatus::Inconclusive,
                )
                | (true, GateStatus::NotYetEnforced) => {
                    return Err(UnknownsDefect::GateRecord(name));
                }
                (true, GateStatus::Pending) if name == GateName::ReceiptGeneration => None,
                (true, GateStatus::Pending) => Some(Unknown::GatePending(name)),
                (true, GateStatus::Inconclusive) => Some(Unknown::GateInconclusive(name)),
                (true, GateStatus::Passed | GateStatus::Failed) => None,
            };
            entries.extend(entry);
        }
        if profile.enforces(GateName::RefinementCoverage) {
            match status.refinement() {
                RefinementStatus::Absent(absence) => {
                    entries.push(Unknown::RefinementNotComputed(absence));
                }
            }
        }
        if profile.enforces(GateName::CertificateRebuild) {
            match status.rebuild() {
                RebuildStatus::Absent(absence) => {
                    entries.push(Unknown::CertificateRebuildNotComputed(absence));
                }
            }
        }
        let mut verified = 0_usize;
        for certificate in status.certificates() {
            match certificate.status() {
                CertificateStatus::VerifiedModelBound => verified += 1,
                CertificateStatus::Unverified(reason) => {
                    entries.push(Unknown::CertificateUnverified {
                        certificate: certificate.node().clone(),
                        reason,
                    });
                }
            }
        }
        if verified == 0 {
            entries.push(Unknown::NoVerifiedCertificate);
        }
        entries.extend(pack_cases.into_iter().map(Unknown::UnsupportedPackCase));
        match diff {
            None => {}
            Some(DiffGap::ProgramLayerUnclassified) => {
                entries.push(Unknown::SemanticDiffUnclassified);
            }
            Some(DiffGap::Undiffable(cause)) => {
                entries.push(Unknown::SemanticDiffUndiffable(cause))
            }
        }

        entries.sort_by_cached_key(Unknown::token);
        // Unique by construction: distinct gates, distinct certificate handles (checked
        // by the status derivation), distinct pack cases (checked above).
        debug_assert!(entries.windows(2).all(|pair| pair[0] != pair[1]));
        Ok(Self { entries })
    }

    /// The entries, in canonical order.
    #[must_use]
    pub fn entries(&self) -> &[Unknown] {
        &self.entries
    }

    /// The rendered entries, in canonical order.
    #[must_use]
    pub fn tokens(&self) -> Vec<String> {
        self.entries.iter().map(Unknown::token).collect()
    }

    /// The `unknowns` array.
    #[must_use]
    pub fn entries_json(&self) -> Json {
        Json::Array(
            self.entries
                .iter()
                .map(|u| Json::String(u.token()))
                .collect(),
        )
    }

    /// The entries the IMPL-05 status contributed, as a JSON array. It is a retained
    /// evidence fragment only, a strict subset of [`Self::entries_json`]: it is never a
    /// receipt's `unknowns`, and `ReceiptSkeleton::fields_json` does not merge it.
    #[must_use]
    pub fn status_entries_json(&self) -> Json {
        Json::Array(
            self.entries
                .iter()
                .filter(|u| {
                    matches!(
                        u.kind(),
                        UnknownKind::RefinementNotComputed
                            | UnknownKind::CertificateRebuildNotComputed
                            | UnknownKind::NoVerifiedCertificate
                            | UnknownKind::CertificateUnverified
                    )
                })
                .map(|u| Json::String(u.token()))
                .collect(),
        )
    }

    /// The entries the IMPL-03 semantic diff contributed (zero or one), as a JSON array.
    /// A retained evidence fragment only, like [`Self::status_entries_json`].
    #[must_use]
    pub fn diff_entries_json(&self) -> Json {
        Json::Array(
            self.entries
                .iter()
                .filter(|u| {
                    matches!(
                        u.kind(),
                        UnknownKind::SemanticDiffUnclassified | UnknownKind::SemanticDiffUndiffable
                    )
                })
                .map(|u| Json::String(u.token()))
                .collect(),
        )
    }
}

/// Why a claimed `unknowns` list is not the derived one. No variant echoes a claimed
/// string.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum UnknownsMismatch {
    Duplicated,
    Omitted(UnknownKind),
    NotDerived,
    NotCanonical,
}

/// Compare a claimed list with the derived one. The claim is indexed once, O(n log n) in
/// its length, which the claim's byte bound caps.
pub(crate) fn compare(claimed: &[String], derived: &Unknowns) -> Result<(), UnknownsMismatch> {
    let mut seen: BTreeMap<&str, ()> = BTreeMap::new();
    for entry in claimed {
        if seen.insert(entry.as_str(), ()).is_some() {
            return Err(UnknownsMismatch::Duplicated);
        }
    }
    let tokens = derived.tokens();
    for (unknown, token) in derived.entries.iter().zip(&tokens) {
        if !seen.contains_key(token.as_str()) {
            return Err(UnknownsMismatch::Omitted(unknown.kind()));
        }
    }
    if claimed.len() != tokens.len() {
        return Err(UnknownsMismatch::NotDerived);
    }
    if claimed.iter().zip(&tokens).any(|(a, b)| a != b) {
        return Err(UnknownsMismatch::NotCanonical);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::handle::SnapshotId;
    use crate::receipt::status::{CertificateRecord, CertificateVerdict, InsufficientReason};

    fn node(text: &str) -> CertificateNode {
        CertificateNode::new(text).unwrap()
    }

    fn every_kind() -> Vec<Unknown> {
        let mut all = Vec::new();
        for gate in GateName::ALL {
            all.push(Unknown::GateNotYetEnforced(gate));
            all.push(Unknown::GatePending(gate));
            all.push(Unknown::GateInconclusive(gate));
        }
        all.push(Unknown::RefinementNotComputed(
            RefinementAbsence::CheckerNotDeployed,
        ));
        all.push(Unknown::CertificateRebuildNotComputed(
            RebuildAbsence::InvalidationSetNotComputed,
        ));
        all.push(Unknown::NoVerifiedCertificate);
        for reason in UnverifiedReason::ALL {
            for handle in ["ev_a", "ev_a_b", "cert_x-y"] {
                all.push(Unknown::CertificateUnverified {
                    certificate: node(handle),
                    reason,
                });
            }
        }
        for (pack, case) in [
            ("storage/append-log-v1", "flush-dishonesty"),
            ("storage/append-log-v1", "sector-corruption"),
            ("time/clock-v1", "flush-dishonesty"),
        ] {
            all.push(Unknown::UnsupportedPackCase(
                PackCase::new(pack, case).unwrap(),
            ));
        }
        all.push(Unknown::SemanticDiffUnclassified);
        for cause in UndiffableCause::ALL {
            all.push(Unknown::SemanticDiffUndiffable(cause));
        }
        all
    }

    /// pr22-impl06, serialization round trip: the rendering is injective — every entry
    /// reads back as itself, and no two entries render alike.
    #[test]
    fn pr22_impl06_the_token_rendering_round_trips_and_is_injective() {
        let all = every_kind();
        let mut rendered = std::collections::BTreeSet::new();
        for unknown in &all {
            let token = unknown.token();
            assert!(token.starts_with(unknown.kind().prefix()));
            assert_eq!(
                Unknown::from_token(&token).as_ref(),
                Some(unknown),
                "{token}"
            );
            assert!(rendered.insert(token));
        }
        for junk in [
            "",
            "gate_pending",
            "gate_pending:gate_thirteen",
            "gate_pending:base_replay:x",
            "certificate_none_verified:x",
            "certificate_unverified:ev_a",
            "certificate_unverified:ev_a:not_a_reason",
            "unsupported_pack_case:Storage:x",
            "refinement_not_computed:zero",
            "production storage profile assumed (contractual)",
            "semantic_diff_unclassified",
            "semantic_diff_unclassified:model_layer",
            "semantic_diff_undiffable",
            "semantic_diff_undiffable:impact",
            "semantic_diff_undiffable:unrenderable:x",
        ] {
            assert_eq!(Unknown::from_token(junk), None, "{junk}");
        }
    }

    #[test]
    fn pr22_impl06_pack_tokens_follow_their_grammar() {
        assert!(PackCase::new("storage/append-log-v1", "flush-dishonesty").is_ok());
        for bad in ["", "-x", "A", "a:b", "a b", "a_b"] {
            assert_eq!(PackCase::new(bad, "x"), Err(MalformedPackCase), "{bad}");
            assert_eq!(PackCase::new("x", bad), Err(MalformedPackCase), "{bad}");
        }
        let long = "a".repeat(MAX_PACK_TOKEN_BYTES);
        assert!(PackCase::new(&long, "x").is_ok());
        assert!(PackCase::new(&format!("{long}a"), "x").is_err());
    }

    /// The record a fresh version starts from under `profile`.
    fn initial_gates(profile: GateProfile) -> [(GateName, GateStatus); 12] {
        GateName::ALL.map(|gate| {
            let status = if profile.enforces(gate) {
                GateStatus::Pending
            } else {
                GateStatus::NotYetEnforced
            };
            (gate, status)
        })
    }

    fn status(records: Vec<CertificateRecord>) -> RefinementCertificateStatus {
        RefinementCertificateStatus::derive(records, &SnapshotId::new("ws_c").unwrap()).unwrap()
    }

    /// pr22-impl06 completeness: over every gate status each profile admits, every
    /// certificate verdict and a set of pack cases, each absent or inconclusive input
    /// appears exactly once, and no entry comes from anywhere else.
    #[test]
    fn pr22_impl06_every_absent_or_inconclusive_input_appears_exactly_once() {
        let candidate = SnapshotId::new("ws_c").unwrap();
        let verdicts = [
            CertificateVerdict::ModelBound {
                snapshot: candidate.clone(),
            },
            CertificateVerdict::ModelBound {
                snapshot: SnapshotId::new("ws_other").unwrap(),
            },
            CertificateVerdict::Rejected,
            CertificateVerdict::Unchecked,
            CertificateVerdict::InsufficientEvidence(InsufficientReason::TrustsModelCorrespondence),
            CertificateVerdict::InsufficientEvidence(InsufficientReason::FamilyNotModelBound),
            CertificateVerdict::InsufficientEvidence(InsufficientReason::NoSealedSnapshot),
            CertificateVerdict::InsufficientEvidence(InsufficientReason::NoHeldModel),
            CertificateVerdict::InsufficientEvidence(InsufficientReason::ContentMissing),
            CertificateVerdict::InsufficientEvidence(InsufficientReason::CheckerNotServed),
            CertificateVerdict::InsufficientEvidence(InsufficientReason::WireUnsupported),
            CertificateVerdict::KernelInconclusive,
            CertificateVerdict::Redacted,
            CertificateVerdict::Stale,
        ];
        let packs = vec![
            PackCase::new("storage/append-log-v1", "flush-dishonesty").unwrap(),
            PackCase::new("time/clock-v1", "leap-second").unwrap(),
        ];
        let in_profile = [
            GateStatus::Passed,
            GateStatus::Failed,
            GateStatus::Pending,
            GateStatus::Inconclusive,
        ];
        for profile in GateProfile::ALL {
            // Each in-profile gate takes each status in turn, the others rotating, so
            // every (gate, status) pair occurs.
            for shift in 0..in_profile.len() {
                let mut gates = initial_gates(profile);
                for (index, (gate, status)) in gates.iter_mut().enumerate() {
                    if profile.enforces(*gate) {
                        *status = in_profile[(index + shift) % in_profile.len()];
                    }
                }
                // Certificate subsets: none, each alone, all together.
                let mut subsets: Vec<Vec<usize>> = vec![Vec::new()];
                subsets.extend((0..verdicts.len()).map(|i| vec![i]));
                subsets.push((0..verdicts.len()).collect());
                for subset in subsets {
                    for (pack_cases, diff) in [
                        (Vec::new(), None),
                        (packs.clone(), Some(DiffGap::ProgramLayerUnclassified)),
                        (
                            Vec::new(),
                            Some(DiffGap::Undiffable(UndiffableCause::ImpactScope)),
                        ),
                    ] {
                        let records: Vec<CertificateRecord> = subset
                            .iter()
                            .map(|i| {
                                CertificateRecord::new(
                                    node(&format!("ev_{i}")),
                                    verdicts[*i].clone(),
                                )
                            })
                            .collect();
                        let derived_status = status(records);
                        let unknowns = Unknowns::derive(
                            profile,
                            gates,
                            &derived_status,
                            pack_cases.clone(),
                            diff,
                        )
                        .unwrap();

                        // The expected multiset, one per input, built independently.
                        let mut expected: Vec<Unknown> = Vec::new();
                        for (name, recorded) in gates {
                            if !profile.enforces(name) {
                                expected.push(Unknown::GateNotYetEnforced(name));
                            } else if recorded == GateStatus::Inconclusive {
                                expected.push(Unknown::GateInconclusive(name));
                            } else if recorded == GateStatus::Pending
                                && name != GateName::ReceiptGeneration
                            {
                                expected.push(Unknown::GatePending(name));
                            }
                        }
                        if profile.enforces(GateName::RefinementCoverage) {
                            expected.push(Unknown::RefinementNotComputed(
                                RefinementAbsence::CheckerNotDeployed,
                            ));
                        }
                        if profile.enforces(GateName::CertificateRebuild) {
                            expected.push(Unknown::CertificateRebuildNotComputed(
                                RebuildAbsence::InvalidationSetNotComputed,
                            ));
                        }
                        let mut any_verified = false;
                        for i in &subset {
                            if *i == 0 {
                                any_verified = true;
                            } else {
                                let reason = match &verdicts[*i] {
                                    CertificateVerdict::ModelBound { .. } => {
                                        UnverifiedReason::BoundToOtherSnapshot
                                    }
                                    CertificateVerdict::Rejected => UnverifiedReason::Rejected,
                                    CertificateVerdict::Unchecked => UnverifiedReason::Unchecked,
                                    CertificateVerdict::KernelInconclusive => {
                                        UnverifiedReason::KernelInconclusive
                                    }
                                    CertificateVerdict::Redacted => UnverifiedReason::Redacted,
                                    CertificateVerdict::Stale => UnverifiedReason::Stale,
                                    CertificateVerdict::InsufficientEvidence(r) => match r {
                                        InsufficientReason::TrustsModelCorrespondence => {
                                            UnverifiedReason::TrustsModelCorrespondence
                                        }
                                        InsufficientReason::FamilyNotModelBound => {
                                            UnverifiedReason::FamilyNotModelBound
                                        }
                                        InsufficientReason::NoSealedSnapshot => {
                                            UnverifiedReason::NoSealedSnapshot
                                        }
                                        InsufficientReason::NoHeldModel => {
                                            UnverifiedReason::NoHeldModel
                                        }
                                        InsufficientReason::ContentMissing => {
                                            UnverifiedReason::ContentMissing
                                        }
                                        InsufficientReason::CheckerNotServed => {
                                            UnverifiedReason::CheckerNotServed
                                        }
                                        InsufficientReason::WireUnsupported => {
                                            UnverifiedReason::WireUnsupported
                                        }
                                    },
                                };
                                expected.push(Unknown::CertificateUnverified {
                                    certificate: node(&format!("ev_{i}")),
                                    reason,
                                });
                            }
                        }
                        if !any_verified {
                            expected.push(Unknown::NoVerifiedCertificate);
                        }
                        expected.extend(pack_cases.into_iter().map(Unknown::UnsupportedPackCase));
                        match diff {
                            None => {}
                            Some(DiffGap::ProgramLayerUnclassified) => {
                                expected.push(Unknown::SemanticDiffUnclassified);
                            }
                            Some(DiffGap::Undiffable(cause)) => {
                                expected.push(Unknown::SemanticDiffUndiffable(cause));
                            }
                        }

                        let mut got = unknowns.entries().to_vec();
                        let mut want = expected;
                        // Canonical order is sorted; exactly-once is multiset equality
                        // with no duplicate on either side.
                        let rendered = unknowns.tokens();
                        assert!(
                            rendered.windows(2).all(|p| p[0] < p[1]),
                            "byte-ordered and unique"
                        );
                        got.sort();
                        want.sort();
                        assert_eq!(got, want, "{profile:?} shift {shift} {subset:?}");
                    }
                }
            }
        }
    }

    #[test]
    fn pr22_impl06_a_record_that_contradicts_its_profile_is_refused() {
        let empty = status(Vec::new());
        let mut gates = initial_gates(GateProfile::PhaseB);
        gates[8].1 = GateStatus::Passed; // certificate_rebuild, outside phase-b
        assert_eq!(
            Unknowns::derive(GateProfile::PhaseB, gates, &empty, Vec::new(), None),
            Err(UnknownsDefect::GateRecord(GateName::CertificateRebuild))
        );
        let mut gates = initial_gates(GateProfile::PhaseB);
        gates[0].1 = GateStatus::NotYetEnforced;
        assert_eq!(
            Unknowns::derive(GateProfile::PhaseB, gates, &empty, Vec::new(), None),
            Err(UnknownsDefect::GateRecord(GateName::BaseReplay))
        );
    }

    #[test]
    fn pr22_impl06_the_pack_case_bound_is_checked_and_duplicates_are_refused() {
        let empty = status(Vec::new());
        let gates = initial_gates(GateProfile::PhaseB);
        let at_bound: Vec<PackCase> = (0..MAX_PACK_CASES)
            .map(|i| PackCase::new("storage/append-log-v1", &format!("c{i}")).unwrap())
            .collect();
        assert!(
            Unknowns::derive(GateProfile::PhaseB, gates, &empty, at_bound.clone(), None).is_ok()
        );
        let mut over = at_bound;
        over.push(PackCase::new("x", "y").unwrap());
        assert_eq!(
            Unknowns::derive(GateProfile::PhaseB, gates, &empty, over, None),
            Err(UnknownsDefect::Evidence(EvidenceDefect::OverBound(
                EvidenceKind::PackCases
            )))
        );
        let twice = vec![
            PackCase::new("x", "y").unwrap(),
            PackCase::new("x", "y").unwrap(),
        ];
        assert_eq!(
            Unknowns::derive(GateProfile::PhaseB, gates, &empty, twice, None),
            Err(UnknownsDefect::Evidence(EvidenceDefect::Duplicate(
                EvidenceKind::PackCases
            )))
        );
    }

    #[test]
    fn pr22_impl06_compare_names_the_first_mismatch_without_echoing() {
        let gates = initial_gates(GateProfile::PhaseB);
        let derived = Unknowns::derive(
            GateProfile::PhaseB,
            gates,
            &status(vec![CertificateRecord::new(
                node("ev_x"),
                CertificateVerdict::Unchecked,
            )]),
            Vec::new(),
            None,
        )
        .unwrap();
        let tokens = derived.tokens();
        assert_eq!(compare(&tokens, &derived), Ok(()));

        let mut omitted = tokens.clone();
        let dropped = omitted
            .iter()
            .position(|t| t.starts_with("certificate_unverified"))
            .unwrap();
        omitted.remove(dropped);
        assert_eq!(
            compare(&omitted, &derived),
            Err(UnknownsMismatch::Omitted(
                UnknownKind::CertificateUnverified
            ))
        );

        let mut extra = tokens.clone();
        extra.push("production storage profile assumed (contractual)".to_owned());
        assert_eq!(compare(&extra, &derived), Err(UnknownsMismatch::NotDerived));

        let mut duplicated = tokens.clone();
        duplicated.push(tokens[0].clone());
        assert_eq!(
            compare(&duplicated, &derived),
            Err(UnknownsMismatch::Duplicated)
        );

        let mut reordered = tokens;
        reordered.reverse();
        assert_eq!(
            compare(&reordered, &derived),
            Err(UnknownsMismatch::NotCanonical)
        );
        assert_eq!(
            compare(&[], &derived),
            Err(UnknownsMismatch::Omitted(
                UnknownKind::NoVerifiedCertificate
            ))
        );
    }
}
