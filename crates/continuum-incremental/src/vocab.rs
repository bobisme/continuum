//! The closed vocabularies of RFC 0030: reuse-edge classes, dependency reasons,
//! auditability classes, reuse-evidence forms, and the independence tri-state.
//!
//! Every set here is closed (RFC 0030, "Versioning and revision"). A token this
//! module does not recognize decodes fail-closed: an unknown reuse-edge class reads
//! as [`ReuseClass::Experimental`], never as [`ReuseClass::Exact`] ("Forward
//! compatibility is achieved by blocking, never by ignoring").

use core::fmt;

/// The four reuse-edge classes (RFC 0030, "Reuse-edge classes").
///
/// The derived order is the trust order: `Experimental < Conservative < Validated <
/// Exact`, so [`ReuseClass::meet`] is `min`. It is a downgrade ladder, not a quality
/// ranking; nothing in this crate raises a class in place.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ReuseClass {
    /// Speed-up only; soundness is not argued. Never supports promotion.
    Experimental,
    /// Invalidation may over-approximate the cone but never under-approximate it,
    /// under assumptions recorded on the edge.
    Conservative,
    /// The output carries a witness an independent checker accepts for this input.
    Validated,
    /// The output is a pure function of the named inputs, and the key matched.
    Exact,
}

impl ReuseClass {
    /// Every member, strongest first.
    pub const ALL: [Self; 4] = [
        Self::Exact,
        Self::Validated,
        Self::Conservative,
        Self::Experimental,
    ];

    /// The meet (weakest) of two classes: RFC 0030's composition rule. One
    /// `Experimental` edge anywhere makes the derivation `Experimental`.
    #[must_use]
    pub fn meet(self, other: Self) -> Self {
        self.min(other)
    }

    /// The wire token.
    #[must_use]
    pub const fn token(self) -> &'static str {
        match self {
            Self::Exact => "Exact",
            Self::Validated => "Validated",
            Self::Conservative => "Conservative",
            Self::Experimental => "Experimental",
        }
    }

    /// Decode a token, failing closed: an unrecognized token is `Experimental`.
    #[must_use]
    pub fn from_token_fail_closed(token: &str) -> Self {
        Self::ALL
            .into_iter()
            .find(|class| class.token() == token)
            .unwrap_or(Self::Experimental)
    }

    /// Whether a result carrying this class may support promotion or a finality
    /// claim. Typed, so no caller reads a bare `bool` as a verdict.
    #[must_use]
    pub const fn promotion(self) -> Promotion {
        match self {
            Self::Experimental => Promotion::NotPromotable,
            Self::Exact | Self::Validated | Self::Conservative => Promotion::Promotable,
        }
    }
}

impl fmt::Display for ReuseClass {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.token())
    }
}

/// Whether a result may support promotion (RFC 0030, "No promotion on
/// `Experimental`").
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Promotion {
    /// The derivation's class is strong or conservative.
    Promotable,
    /// The derivation passed through an `Experimental` edge.
    NotPromotable,
}

/// The nine dependency reasons (RFC 0030, "Semantic dependency reasons").
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DependencyReason {
    /// The query consumed a declaration's type, not its value.
    ReadsType,
    /// The query consumed a definition's value or body.
    ReadsValue,
    /// The query expanded a definition.
    UnfoldsDefinition,
    /// The query resolved a domain-pack instance or a fidelity profile.
    SelectsInstanceOrProfile,
    /// The result depends on which events an observer publishes.
    ObservesEventFamily,
    /// The result is conditional on an assumption, a fairness constraint, or a bound.
    ReliesOnAssumptionFairnessBound,
    /// The query consumed an abstraction or correspondence map component.
    UsesAbstractionComponent,
    /// The result rests on a lemma, a checker identity, or an assurance requirement.
    DependsOnLemmaOrChecker,
    /// The result depends on an encoding, an epoch-scoped encoding, or a
    /// correspondence link.
    ConsumesEncodingEpochOrCorrespondence,
}

impl DependencyReason {
    /// Every member, in the RFC's table order.
    pub const ALL: [Self; 9] = [
        Self::ReadsType,
        Self::ReadsValue,
        Self::UnfoldsDefinition,
        Self::SelectsInstanceOrProfile,
        Self::ObservesEventFamily,
        Self::ReliesOnAssumptionFairnessBound,
        Self::UsesAbstractionComponent,
        Self::DependsOnLemmaOrChecker,
        Self::ConsumesEncodingEpochOrCorrespondence,
    ];

    /// The wire token.
    #[must_use]
    pub const fn token(self) -> &'static str {
        match self {
            Self::ReadsType => "reads-type",
            Self::ReadsValue => "reads-value",
            Self::UnfoldsDefinition => "unfolds-definition",
            Self::SelectsInstanceOrProfile => "selects-instance-or-profile",
            Self::ObservesEventFamily => "observes-event-family",
            Self::ReliesOnAssumptionFairnessBound => "relies-on-assumption-fairness-bound",
            Self::UsesAbstractionComponent => "uses-abstraction-component",
            Self::DependsOnLemmaOrChecker => "depends-on-lemma-or-checker",
            Self::ConsumesEncodingEpochOrCorrespondence => {
                "consumes-encoding-epoch-or-correspondence"
            }
        }
    }

    /// Decode a token. `None` for a token outside the closed set; a caller treats
    /// that edge as `Experimental` with independence [`Independence::Unknown`].
    #[must_use]
    pub fn from_token(token: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|reason| reason.token() == token)
    }
}

impl fmt::Display for DependencyReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.token())
    }
}

/// The three auditability classes (RFC 0030, "Auditability classes"). A property of
/// the query definition, never of a run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Auditability {
    /// Deterministic; compared bit-for-bit. Divergence always quarantines.
    EqualityAuditable,
    /// Solver-backed; certificates and claim envelopes are compared.
    CertificateAuditable,
    /// Anytime results; monotone honesty only.
    BudgetSensitive,
}

impl Auditability {
    /// The wire token.
    #[must_use]
    pub const fn token(self) -> &'static str {
        match self {
            Self::EqualityAuditable => "equality-auditable",
            Self::CertificateAuditable => "certificate-auditable",
            Self::BudgetSensitive => "budget-sensitive",
        }
    }
}

/// The five reuse-evidence forms (RFC 0030, "Reuse evidence").
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum EvidenceForm {
    /// The key matched byte for byte. Licenses `Exact`.
    ContentIdentity,
    /// A translation-validation witness. Licenses `Validated`.
    TranslationValidationWitness,
    /// A certificate checked from its wire form by the independent kernel. Licenses
    /// `Validated`.
    CheckedCertificate,
    /// An instance of a stated closure theorem, with its assumptions. Licenses
    /// `Conservative`.
    ConservativeDependencyTheorem,
    /// A clean recomputation that agreed. Clears a quarantine; never upgrades.
    CleanComparisonReceipt,
}

impl EvidenceForm {
    /// The wire token.
    #[must_use]
    pub const fn token(self) -> &'static str {
        match self {
            Self::ContentIdentity => "content-identity",
            Self::TranslationValidationWitness => "translation-validation-witness",
            Self::CheckedCertificate => "checked-certificate",
            Self::ConservativeDependencyTheorem => "conservative-dependency-theorem",
            Self::CleanComparisonReceipt => "clean-comparison-receipt",
        }
    }
}

/// Independence between an edit and a query (RFC 0030, "Independence and the
/// invalidation cone"). `Unknown` is dependent.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum Independence {
    /// Independent, with the evidence that establishes it.
    DefinitelyIndependent(EvidenceForm),
    /// Dependent, through the named reason.
    DefinitelyDependent(DependencyReason),
    /// Not decided. Treated as dependent everywhere except the interactive lane's
    /// `Experimental` previews, which never support promotion.
    Unknown,
}

/// The two lanes of the Incremental Parity Audit (RFC 0030, "Incremental Parity
/// Audit"). "Dirty" names nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Lane {
    /// Maximizes reuse, including `Experimental` previews.
    Interactive,
    /// Admits no `Experimental` reuse: every result is recomputed or reused through
    /// a strong or conservative edge.
    Promotion,
}

impl Lane {
    /// The token used in reports.
    #[must_use]
    pub const fn token(self) -> &'static str {
        match self {
            Self::Interactive => "interactive",
            Self::Promotion => "promotion",
        }
    }

    /// Whether this lane admits a reuse licensed by `class`.
    #[must_use]
    pub const fn admits(self, class: ReuseClass) -> Admission {
        match (self, class) {
            (Self::Promotion, ReuseClass::Experimental) => Admission::Refused,
            _ => Admission::Admitted,
        }
    }
}

/// Whether a lane admits a licence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Admission {
    /// The lane admits it.
    Admitted,
    /// The lane refuses it; the query is recomputed.
    Refused,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn meet_is_the_weakest_class_and_experimental_absorbs() {
        for a in ReuseClass::ALL {
            assert_eq!(a.meet(ReuseClass::Experimental), ReuseClass::Experimental);
            assert_eq!(a.meet(ReuseClass::Exact), a);
            for b in ReuseClass::ALL {
                assert_eq!(a.meet(b), b.meet(a));
            }
        }
        assert_eq!(
            ReuseClass::Validated.meet(ReuseClass::Conservative),
            ReuseClass::Conservative
        );
    }

    #[test]
    fn an_unrecognized_class_token_fails_closed_to_experimental() {
        for class in ReuseClass::ALL {
            assert_eq!(ReuseClass::from_token_fail_closed(class.token()), class);
        }
        for token in ["Complete", "exact", "", "Exact ", "Strong"] {
            assert_eq!(
                ReuseClass::from_token_fail_closed(token),
                ReuseClass::Experimental,
                "{token:?}"
            );
        }
    }

    #[test]
    fn the_reason_set_is_the_nine_rfc_tokens() {
        let tokens: Vec<&str> = DependencyReason::ALL.iter().map(|r| r.token()).collect();
        assert_eq!(tokens.len(), 9);
        for reason in DependencyReason::ALL {
            assert_eq!(DependencyReason::from_token(reason.token()), Some(reason));
        }
        assert_eq!(DependencyReason::from_token("READS_TYPE"), None);
    }

    #[test]
    fn only_the_promotion_lane_refuses_experimental() {
        for class in ReuseClass::ALL {
            assert_eq!(Lane::Interactive.admits(class), Admission::Admitted);
            let expected = if class == ReuseClass::Experimental {
                Admission::Refused
            } else {
                Admission::Admitted
            };
            assert_eq!(Lane::Promotion.admits(class), expected);
        }
    }
}
