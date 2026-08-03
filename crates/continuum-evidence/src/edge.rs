//! The thirteen edge kinds as a typed, closed vocabulary — and the checker a `CHECKED_BY`
//! edge cannot exist without (plan §11.3, PR-7 / IMPL-01).
//!
//! # Seven words, thirteen kinds
//!
//! PR 7's first bullet names seven relations in lower case:
//!
//! > support/refute/depend/refine/explain/repair/check edges
//! >
//! > — `START_HERE_IMPLEMENTATION.md`, PR 7
//!
//! and plan §11.3 names thirteen in the wire's spelling, which the edge schema's `kind` enum
//! transcribes exactly. The seven are a *subset* of the thirteen, not a different
//! vocabulary — [`PR7_BULLET_WORDS`] is that correspondence as data, and
//! `the_bullets_seven_words_name_seven_of_the_thirteen` pins it. Implementing only seven
//! would have been the worse reading of "closed vocabulary": PR 7's own remaining bullets
//! need two of the other six (`CONFLICTS_WITH` for conflict nodes, `SUPERSEDES` for
//! resolution and for plan §4.6's re-derivation link), and an enum missing six members of a
//! closed schema enum is a vocabulary that cannot round-trip a conforming instance.
//!
//! # `CHECKED_BY` requires a checker, at the type level
//!
//! > A `CHECKED_BY` edge names the trusted checker/service that performed the check
//! > (plan §11.3).
//! >
//! > — `evidence-graph-edge.schema.json`, the one conditional in the file
//!
//! > `evidence-graph-edge` requires `checker` on `CHECKED_BY` (INV-004)
//! >
//! > — plan §2, SD-11
//!
//! The schema states it as an `if`/`then`: a conforming instance whose `kind` is
//! `CHECKED_BY` and which omits `checker` is invalid. Restating that as a validation would
//! make it a rule something has to remember to run. [`EdgeRelation`] states it as a sum
//! instead — twelve unit variants and [`EdgeRelation::CheckedBy`] carrying a
//! [`CheckerBinding`], which itself carries a [`ServiceIdentity`], which itself refuses
//! every actor scheme but `service:`. **There is no constructor for a check edge that does
//! not name a checker**, and there is no `Option` anywhere on the path.
//!
//! # What this module deliberately leaves open: RFC 0038's F14
//!
//! > bn-i4aem deferred inventing the surface as F14 because RFC 0038 must first decide:
//! > which node kind a `CHECKED_BY` edge's to-handle names (run/certificate/receipt),
//! > `edge_id` derivation, and edge-creation authority (RFC 0038's Authority section covers
//! > only node creation and status promotion). A 73rd operation is a plan §10.2 + RFC 0027
//! > registry + IDL change together.
//! >
//! > — bn-3sypm
//!
//! That bone owns the **wire surface**, and this module owns the library type. The three
//! questions are handled as follows, none of them answered here:
//!
//! | F14 question | How this module leaves it open |
//! |---|---|
//! | which node kind the to-handle names | [`CheckTargetRule`], a graph-level policy whose default admits any kind; [`CHECKED_BY_TARGET_CANDIDATES`] records the three the bone lists, and each of them is expressible as `CheckTargetRule::one_of` |
//! | `edge_id` derivation | [`crate::identity::EvidenceNaming`] — the identity is the canonical preimage, and the preimage → `ev_…` function is a seam |
//! | edge-creation authority | not modelled: this crate mints no capability for edge creation, and [`crate::authority`] covers exactly what RFC 0038's Authority section covers — node creation and status promotion |
//!
//! Nothing here adds an operation to the IDL, the RFC 0027 registry, or the daemon.
//!
//! # Content identity, and why the checker and the provenance are in it
//!
//! An edge's identity is the canonical encoding of what it asserts: the relation (including
//! the checker, when there is one), both endpoints, the evidence it names, and who asserted
//! it. Provenance is *inside* the identity deliberately. Two agents that independently
//! assert the same `SUPPORTS` edge have produced two pieces of evidence with two producers;
//! collapsing them into one edge would either lose a producer's credit — which docs/44's
//! "Credit and provenance" exists to keep — or make the surviving provenance depend on write
//! order, which is the ambiguity GOV-1-12 forbids and the write-order resolution plan §11.7
//! forbids.
//!
//! # Two things a caller cannot spell
//!
//! `compile_fail` doc tests, which compile as an external crate and are therefore the view a
//! client has of this API. A check edge with no checker does not exist:
//!
//! ```compile_fail
//! use continuum_evidence::edge::EdgeRelation;
//!
//! // `EdgeRelation::CheckedBy` is a constructor taking a `CheckerBinding`, so naming it
//! // alone is a function value and never a relation.
//! let _relation: EdgeRelation = EdgeRelation::CheckedBy;
//! ```
//!
//! …and the checker is not a string:
//!
//! ```compile_fail
//! use continuum_evidence::edge::{CheckerBinding, EdgeRelation};
//!
//! let _ = EdgeRelation::CheckedBy(CheckerBinding::new("service:kernel-core"));
//! ```
//!
//! The spelling that does compile names a `service:` actor, and nothing else will:
//!
//! ```
//! use continuum_evidence::actor::ServiceIdentity;
//! use continuum_evidence::edge::{CheckerBinding, EdgeKind, EdgeRelation};
//!
//! let checker = ServiceIdentity::parse("service:kernel-core").unwrap();
//! let relation = EdgeRelation::CheckedBy(CheckerBinding::new(checker));
//! assert_eq!(relation.kind(), EdgeKind::CheckedBy);
//! assert!(relation.checker().is_some());
//! assert!(ServiceIdentity::parse("agent:swarm-1").is_err());
//! ```
//!
//! # Clause → test
//!
//! | Clause | Source | Test |
//! |---|---|---|
//! | thirteen kinds, the schema's tokens | plan §11.3, edge schema | `the_kind_set_is_exactly_the_schema_enum` |
//! | the bullet's seven are seven of them | PR 7 bullet 1 | `the_bullets_seven_words_name_seven_of_the_thirteen` |
//! | `CHECKED_BY` requires `checker` | edge schema, INV-004 | module doc, both `compile_fail`s; `only_a_check_edge_names_a_checker` |
//! | the checker is a service | plan §11.3, RFC 0027 P3 | `a_checker_is_a_service_identity` |
//! | no stringly edges | GOV-1-12 | `the_kind_set_is_exactly_the_schema_enum`, `an_unknown_token_does_not_parse` |
//! | edges are content-identified | docs/44, ADR-0013 | `what_an_edge_asserts_is_in_its_identity` |
//! | F14's to-handle question stays open | bn-3sypm | `every_candidate_answer_to_f14_is_expressible` |
//! | an edge relates two artifacts | INV-004 | `a_self_edge_is_refused` |

use core::fmt;
use std::collections::BTreeSet;

use continuum_value::value::Value;

use crate::actor::{ServiceIdentity, field};
use crate::identity::EvidenceIdentity;
use crate::node::{NodeKind, NodeRef};
use crate::provenance::{ArtifactRef, Provenance};

/// One of plan §11.3's thirteen edge types.
///
/// Declared in the edge schema's enum order, which is plan §11.3's listing order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum EdgeKind {
    /// `SUPPORTS` — the PR 7 bullet's `support`.
    Supports,
    /// `REFUTES` — the bullet's `refute`.
    Refutes,
    /// `DEPENDS_ON` — the bullet's `depend`.
    DependsOn,
    /// `REFINES` — the bullet's `refine`.
    Refines,
    /// `EXPLAINS` — the bullet's `explain`.
    Explains,
    /// `REPAIRS` — the bullet's `repair`.
    Repairs,
    /// `INVALIDATES` — docs/44: "one patch repairs one failure but invalidates a proof".
    Invalidates,
    /// `GENERALIZES` — docs/44: "one theorem generalizes many finite observations".
    Generalizes,
    /// `COUNTEREXAMPLE_TO` — docs/44: "one counterexample refutes several candidates".
    CounterexampleTo,
    /// `CHECKED_BY` — the bullet's `check`. The one kind the schema attaches a conditional
    /// to; see [`EdgeRelation::CheckedBy`].
    CheckedBy,
    /// `DERIVED_FROM`.
    DerivedFrom,
    /// `CONFLICTS_WITH` — plan §11.7's materialized contradiction; see [`crate::conflict`].
    ConflictsWith,
    /// `SUPERSEDES` — plan §4.6: "re-derived artifacts receive new identities linked to
    /// their predecessors by `SUPERSEDES` edges; nothing is rewritten in place".
    Supersedes,
}

impl EdgeKind {
    /// Every kind, in the edge schema's enum order.
    pub const ALL: [Self; 13] = [
        Self::Supports,
        Self::Refutes,
        Self::DependsOn,
        Self::Refines,
        Self::Explains,
        Self::Repairs,
        Self::Invalidates,
        Self::Generalizes,
        Self::CounterexampleTo,
        Self::CheckedBy,
        Self::DerivedFrom,
        Self::ConflictsWith,
        Self::Supersedes,
    ];

    /// The stable wire token, byte-identical to the edge schema's `kind` enum.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Supports => "SUPPORTS",
            Self::Refutes => "REFUTES",
            Self::DependsOn => "DEPENDS_ON",
            Self::Refines => "REFINES",
            Self::Explains => "EXPLAINS",
            Self::Repairs => "REPAIRS",
            Self::Invalidates => "INVALIDATES",
            Self::Generalizes => "GENERALIZES",
            Self::CounterexampleTo => "COUNTEREXAMPLE_TO",
            Self::CheckedBy => "CHECKED_BY",
            Self::DerivedFrom => "DERIVED_FROM",
            Self::ConflictsWith => "CONFLICTS_WITH",
            Self::Supersedes => "SUPERSEDES",
        }
    }

    /// Parse a wire token. [`None`] for anything outside the thirteen.
    #[must_use]
    pub fn from_token(token: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|kind| kind.as_str() == token)
    }

    /// Whether the schema obliges an edge of this kind to name a checker.
    ///
    /// Exactly `CHECKED_BY`, which is the edge schema's one `if`/`then`. Reported here so a
    /// renderer can be held to the same rule the type already enforces.
    #[must_use]
    pub const fn requires_checker(self) -> bool {
        matches!(self, Self::CheckedBy)
    }
}

impl fmt::Display for EdgeKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// The seven words PR 7's first bullet names, paired with the edge kind each names.
///
/// Data rather than a comment so the correspondence is testable and a later reader does not
/// have to re-derive it from two documents.
pub const PR7_BULLET_WORDS: [(&str, EdgeKind); 7] = [
    ("support", EdgeKind::Supports),
    ("refute", EdgeKind::Refutes),
    ("depend", EdgeKind::DependsOn),
    ("refine", EdgeKind::Refines),
    ("explain", EdgeKind::Explains),
    ("repair", EdgeKind::Repairs),
    ("check", EdgeKind::CheckedBy),
];

/// The trusted checker a `CHECKED_BY` edge names.
///
/// > The checker's service identity (INV-004).
/// >
/// > — the IDL, `evidence.verify`'s `checker` field
///
/// A [`ServiceIdentity`] rather than a string: the schema's `minLength: 1` is the weakest
/// possible reading of "names the trusted checker/service", and an `agent:` actor in that
/// position would be exactly the self-certification INV-004 forbids, wearing a checker's
/// name.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CheckerBinding(ServiceIdentity);

impl CheckerBinding {
    /// Bind a checker.
    #[must_use]
    pub const fn new(checker: ServiceIdentity) -> Self {
        Self(checker)
    }

    /// The bound checker.
    #[must_use]
    pub const fn checker(&self) -> &ServiceIdentity {
        &self.0
    }
}

impl fmt::Display for CheckerBinding {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.0, f)
    }
}

/// What an edge asserts, with the payload its kind requires.
///
/// Twelve unit variants and one that carries a [`CheckerBinding`]. The asymmetry *is* the
/// edge schema's one conditional: there is no way to name [`EdgeKind::CheckedBy`] as a
/// relation without also naming a checker, and no `Option` on the path to it.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum EdgeRelation {
    /// `SUPPORTS`.
    Supports,
    /// `REFUTES`.
    Refutes,
    /// `DEPENDS_ON`.
    DependsOn,
    /// `REFINES`.
    Refines,
    /// `EXPLAINS`.
    Explains,
    /// `REPAIRS`.
    Repairs,
    /// `INVALIDATES`.
    Invalidates,
    /// `GENERALIZES`.
    Generalizes,
    /// `COUNTEREXAMPLE_TO`.
    CounterexampleTo,
    /// `CHECKED_BY`, with the trusted checker that performed the check.
    CheckedBy(CheckerBinding),
    /// `DERIVED_FROM`.
    DerivedFrom,
    /// `CONFLICTS_WITH`.
    ConflictsWith,
    /// `SUPERSEDES`.
    Supersedes,
}

impl EdgeRelation {
    /// The kind this relation is.
    #[must_use]
    pub const fn kind(&self) -> EdgeKind {
        match self {
            Self::Supports => EdgeKind::Supports,
            Self::Refutes => EdgeKind::Refutes,
            Self::DependsOn => EdgeKind::DependsOn,
            Self::Refines => EdgeKind::Refines,
            Self::Explains => EdgeKind::Explains,
            Self::Repairs => EdgeKind::Repairs,
            Self::Invalidates => EdgeKind::Invalidates,
            Self::Generalizes => EdgeKind::Generalizes,
            Self::CounterexampleTo => EdgeKind::CounterexampleTo,
            Self::CheckedBy(_) => EdgeKind::CheckedBy,
            Self::DerivedFrom => EdgeKind::DerivedFrom,
            Self::ConflictsWith => EdgeKind::ConflictsWith,
            Self::Supersedes => EdgeKind::Supersedes,
        }
    }

    /// The checker this relation names, which is [`Some`] exactly for `CHECKED_BY`.
    #[must_use]
    pub const fn checker(&self) -> Option<&ServiceIdentity> {
        match self {
            Self::CheckedBy(binding) => Some(binding.checker()),
            _ => None,
        }
    }
}

impl fmt::Display for EdgeRelation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.kind(), f)
    }
}

/// The node kinds RFC 0038 has not chosen between for a `CHECKED_BY` edge's to-handle.
///
/// bn-3sypm lists them: `run`, `certificate`, `receipt`. Recorded here as the *candidates*
/// of an open question, never as a rule — [`CheckTargetRule::Undecided`] is the default and
/// admits any node kind, and each candidate answer is expressible with
/// [`CheckTargetRule::one_of`] the day the RFC decides.
pub const CHECKED_BY_TARGET_CANDIDATES: [NodeKind; 3] =
    [NodeKind::Run, NodeKind::Certificate, NodeKind::Receipt];

/// Which node kinds a `CHECKED_BY` edge's to-handle may name.
///
/// A deployment policy rather than a property of the edge type, because RFC 0038 states no
/// rule and inventing one would answer F14 on bn-3sypm's behalf. The default is
/// [`Undecided`](Self::Undecided): the graph admits any kind and says, in this type's name,
/// that the question is open.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum CheckTargetRule {
    /// RFC 0038 has not decided (F14). Any node kind is admitted.
    #[default]
    Undecided,
    /// The decision, once made: a `CHECKED_BY` edge's to-handle names one of these kinds.
    OneOf(BTreeSet<NodeKind>),
}

impl CheckTargetRule {
    /// The rule that admits exactly these kinds.
    #[must_use]
    pub fn one_of(kinds: impl IntoIterator<Item = NodeKind>) -> Self {
        Self::OneOf(kinds.into_iter().collect())
    }

    /// Whether this rule admits a to-handle of `kind`.
    #[must_use]
    pub fn admits(&self, kind: NodeKind) -> bool {
        match self {
            Self::Undecided => true,
            Self::OneOf(kinds) => kinds.contains(&kind),
        }
    }

    /// Whether the rule is still the open question's placeholder.
    #[must_use]
    pub const fn is_undecided(&self) -> bool {
        matches!(self, Self::Undecided)
    }
}

/// A typed, immutable evidence edge.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvidenceEdge {
    relation: EdgeRelation,
    from: NodeRef,
    to: NodeRef,
    evidence: BTreeSet<ArtifactRef>,
    provenance: Provenance,
}

impl EvidenceEdge {
    /// Assert a relation between two nodes.
    ///
    /// The only constructor. It takes a [`Provenance`] by value, so a producer-less edge is
    /// unrepresentable, and both endpoints as [`NodeRef`]s, which can only be taken from a
    /// node.
    ///
    /// # Errors
    ///
    /// [`EdgeRefusal::SelfEdge`] when both endpoints are the same identity. Every one of the
    /// thirteen relations is a statement *between* two artifacts; an artifact that supports,
    /// refutes, explains, repairs or checks itself asserts nothing a reader could check, and
    /// in the `CHECKED_BY` case it is precisely INV-004's self-certification.
    pub fn new(
        relation: EdgeRelation,
        from: NodeRef,
        to: NodeRef,
        evidence: impl IntoIterator<Item = ArtifactRef>,
        provenance: Provenance,
    ) -> Result<Self, EdgeRefusal> {
        if from.identity() == to.identity() {
            return Err(EdgeRefusal::SelfEdge {
                kind: relation.kind(),
            });
        }
        Ok(Self {
            relation,
            from,
            to,
            evidence: evidence.into_iter().collect(),
            provenance,
        })
    }

    /// What the edge asserts.
    #[must_use]
    pub const fn relation(&self) -> &EdgeRelation {
        &self.relation
    }

    /// The edge kind.
    #[must_use]
    pub const fn kind(&self) -> EdgeKind {
        self.relation.kind()
    }

    /// The source endpoint.
    #[must_use]
    pub const fn from(&self) -> &NodeRef {
        &self.from
    }

    /// The target endpoint.
    #[must_use]
    pub const fn to(&self) -> &NodeRef {
        &self.to
    }

    /// The checker, present exactly for `CHECKED_BY`.
    #[must_use]
    pub const fn checker(&self) -> Option<&ServiceIdentity> {
        self.relation.checker()
    }

    /// The evidence artifacts this edge names, in canonical ascending order (edge schema
    /// `evidence`).
    pub fn evidence(&self) -> impl ExactSizeIterator<Item = &ArtifactRef> {
        self.evidence.iter()
    }

    /// Who asserted it, from what, when, and under which epochs.
    #[must_use]
    pub const fn provenance(&self) -> &Provenance {
        &self.provenance
    }

    /// This edge's content identity.
    #[must_use]
    pub fn identity(&self) -> EvidenceIdentity {
        EvidenceIdentity::of(&self.to_preimage())
    }

    /// The canonical preimage this edge's identity is the encoding of.
    ///
    /// Both endpoints contribute their identity **and** their kind, because two edges
    /// between the same identities under different declared kinds would be two different
    /// assertions about the graph's shape.
    #[must_use]
    pub fn to_preimage(&self) -> Value {
        let endpoint = |reference: &NodeRef| {
            Value::record([
                (
                    field("identity"),
                    Value::bytes(reference.identity().canonical_bytes().to_vec()),
                ),
                (field("kind"), Value::text(reference.kind().as_str())),
            ])
            .expect("two distinct compile-time field names")
        };
        let checker = self
            .relation
            .checker()
            .map_or(Value::Null, |service| Value::text(service.as_str()));
        Value::record([
            (field("checker"), checker),
            (
                field("evidence"),
                Value::seq(self.evidence.iter().map(ArtifactRef::to_value))
                    .expect("an evidence list nests two levels"),
            ),
            (field("from"), endpoint(&self.from)),
            (field("kind"), Value::text(self.kind().as_str())),
            (field("provenance"), self.provenance.to_preimage()),
            (field("to"), endpoint(&self.to)),
        ])
        .expect("six distinct compile-time field names")
    }

    /// This edge as `evidence-graph-edge.schema.json` writes it, given the three handles it
    /// and its endpoints are filed under.
    ///
    /// `checker` is emitted exactly for `CHECKED_BY` — which is the schema's one `if`/`then`
    /// — and `evidence` only when the edge names any, because `additionalProperties` is
    /// false.
    #[must_use]
    pub fn to_record(
        &self,
        handle: &crate::identity::EvidenceHandle,
        from: &crate::identity::EvidenceHandle,
        to: &crate::identity::EvidenceHandle,
    ) -> Value {
        let mut fields = vec![
            (
                field("schema_id"),
                Value::text("https://continuum.dev/schema/evidence-graph-edge.json"),
            ),
            (field("schema_epoch"), Value::nat(1)),
            (field("edge_id"), Value::text(handle.as_str())),
            (field("kind"), Value::text(self.kind().as_str())),
            (field("from"), Value::text(from.as_str())),
            (field("to"), Value::text(to.as_str())),
            (field("provenance"), self.provenance.to_record()),
        ];
        if let Some(service) = self.relation.checker() {
            fields.push((field("checker"), Value::text(service.as_str())));
        }
        if !self.evidence.is_empty() {
            fields.push((
                field("evidence"),
                Value::seq(self.evidence.iter().map(ArtifactRef::to_value))
                    .expect("an evidence list nests two levels"),
            ));
        }
        Value::record(fields).expect("the member names are distinct compile-time constants")
    }
}

/// A reference to an edge: its content identity together with the relation it carries.
///
/// The dual of [`NodeRef`], and it exists for one reason: a conflict may be *between two
/// assertions* rather than between two artifacts — a `SUPPORTS` and a `REFUTES` over one
/// claim is [`crate::graph::EvidenceGraph::contradictions`]' whole output — so
/// [`crate::conflict::ConflictSubject`] has to be able to name one. An edge is never an
/// endpoint of another edge; see [`crate::conflict`] for what is done instead.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct EdgeRef {
    identity: EvidenceIdentity,
    kind: EdgeKind,
}

impl EdgeRef {
    /// The reference to an edge.
    #[must_use]
    pub fn of(edge: &EvidenceEdge) -> Self {
        Self {
            identity: edge.identity(),
            kind: edge.kind(),
        }
    }

    /// The referenced identity.
    #[must_use]
    pub const fn identity(&self) -> &EvidenceIdentity {
        &self.identity
    }

    /// The referenced edge's kind.
    #[must_use]
    pub const fn kind(&self) -> EdgeKind {
        self.kind
    }
}

/// Why an edge was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum EdgeRefusal {
    /// Both endpoints name one identity.
    SelfEdge {
        /// The relation that was asserted.
        kind: EdgeKind,
    },
}

impl fmt::Display for EdgeRefusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SelfEdge { kind } => write!(
                f,
                "a {kind} edge relates two artifacts; both endpoints name one identity"
            ),
        }
    }
}

impl core::error::Error for EdgeRefusal {}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::node::tests::{node, provenance};
    use crate::node::{EvidenceNode, NodeKind};

    pub(crate) fn checker(name: &str) -> CheckerBinding {
        CheckerBinding::new(ServiceIdentity::parse(name).expect("a service"))
    }

    pub(crate) fn edge(
        relation: EdgeRelation,
        from: &EvidenceNode,
        to: &EvidenceNode,
    ) -> EvidenceEdge {
        EvidenceEdge::new(
            relation,
            NodeRef::of(from),
            NodeRef::of(to),
            [],
            provenance("agent:swarm-1"),
        )
        .expect("distinct endpoints")
    }

    #[test]
    fn the_kind_set_is_exactly_the_schema_enum() {
        let tokens: Vec<&str> = EdgeKind::ALL.iter().map(|kind| kind.as_str()).collect();
        assert_eq!(
            tokens,
            [
                "SUPPORTS",
                "REFUTES",
                "DEPENDS_ON",
                "REFINES",
                "EXPLAINS",
                "REPAIRS",
                "INVALIDATES",
                "GENERALIZES",
                "COUNTEREXAMPLE_TO",
                "CHECKED_BY",
                "DERIVED_FROM",
                "CONFLICTS_WITH",
                "SUPERSEDES"
            ]
        );
        assert_eq!(EdgeKind::ALL.len(), 13);
        for kind in EdgeKind::ALL {
            assert_eq!(EdgeKind::from_token(kind.as_str()), Some(kind));
            assert_eq!(kind.to_string(), kind.as_str());
        }
    }

    #[test]
    fn an_unknown_token_does_not_parse() {
        // No stringly edges (GOV-1-12): the vocabulary is closed and lenient parsing is how
        // a closed vocabulary stops being one.
        for token in ["supports", "Supports", "CHECKS", "CHECKED", "", "SUPPORTS "] {
            assert_eq!(EdgeKind::from_token(token), None, "{token}");
        }
    }

    #[test]
    fn the_bullets_seven_words_name_seven_of_the_thirteen() {
        assert_eq!(PR7_BULLET_WORDS.len(), 7);
        let named: BTreeSet<EdgeKind> = PR7_BULLET_WORDS.iter().map(|(_, kind)| *kind).collect();
        assert_eq!(named.len(), 7, "the seven words name seven distinct kinds");
        for (word, kind) in PR7_BULLET_WORDS {
            assert!(EdgeKind::ALL.contains(&kind), "{word}");
            // Each bullet word is a prefix of its kind's token, lower-cased — the
            // correspondence is not arbitrary.
            let token = kind.as_str().to_ascii_lowercase();
            assert!(token.starts_with(word), "{word} vs {token}");
        }
        // …and the other six are the ones the bullet does not name.
        let unnamed: Vec<&str> = EdgeKind::ALL
            .into_iter()
            .filter(|kind| !named.contains(kind))
            .map(EdgeKind::as_str)
            .collect();
        assert_eq!(
            unnamed,
            [
                "INVALIDATES",
                "GENERALIZES",
                "COUNTEREXAMPLE_TO",
                "DERIVED_FROM",
                "CONFLICTS_WITH",
                "SUPERSEDES"
            ]
        );
    }

    #[test]
    fn only_a_check_edge_names_a_checker() {
        for kind in EdgeKind::ALL {
            assert_eq!(kind.requires_checker(), kind == EdgeKind::CheckedBy);
        }
        let relations = [
            EdgeRelation::Supports,
            EdgeRelation::Refutes,
            EdgeRelation::DependsOn,
            EdgeRelation::Refines,
            EdgeRelation::Explains,
            EdgeRelation::Repairs,
            EdgeRelation::Invalidates,
            EdgeRelation::Generalizes,
            EdgeRelation::CounterexampleTo,
            EdgeRelation::CheckedBy(checker("service:kernel-core")),
            EdgeRelation::DerivedFrom,
            EdgeRelation::ConflictsWith,
            EdgeRelation::Supersedes,
        ];
        // Every kind is reachable as a relation, and exactly one of them carries a checker.
        let kinds: Vec<EdgeKind> = relations.iter().map(EdgeRelation::kind).collect();
        assert_eq!(kinds, EdgeKind::ALL.to_vec());
        for relation in &relations {
            assert_eq!(
                relation.checker().is_some(),
                relation.kind().requires_checker(),
                "{}",
                relation.kind()
            );
        }
    }

    #[test]
    fn a_checker_is_a_service_identity() {
        // plan §11.3 "names the trusted checker/service"; RFC 0027 P3 "performed by a
        // trusted service identity".
        let binding = checker("service:kernel-core");
        assert_eq!(binding.checker().as_str(), "service:kernel-core");
        assert_eq!(binding.to_string(), "service:kernel-core");
        assert!(ServiceIdentity::parse("agent:swarm-1").is_err());
        assert!(ServiceIdentity::parse("human:ada").is_err());
        assert!(ServiceIdentity::parse("ci:nightly").is_err());
    }

    #[test]
    fn a_self_edge_is_refused() {
        let node = node(NodeKind::Run, "trace_9f", "claim-1");
        for relation in [
            EdgeRelation::Supports,
            EdgeRelation::CheckedBy(checker("service:kernel-core")),
            EdgeRelation::ConflictsWith,
        ] {
            let kind = relation.kind();
            let refused = EvidenceEdge::new(
                relation,
                NodeRef::of(&node),
                NodeRef::of(&node),
                [],
                provenance("agent:swarm-1"),
            )
            .expect_err("an edge relates two artifacts");
            assert_eq!(refused, EdgeRefusal::SelfEdge { kind });
        }
        assert_eq!(
            EdgeRefusal::SelfEdge {
                kind: EdgeKind::CheckedBy
            }
            .to_string(),
            "a CHECKED_BY edge relates two artifacts; both endpoints name one identity"
        );
    }

    #[test]
    fn what_an_edge_asserts_is_in_its_identity() {
        let claim = node(NodeKind::Property, "prop_9f", "claim-1");
        let run = node(NodeKind::Run, "trace_9f", "claim-1");
        let other = node(NodeKind::Run, "trace_aa", "claim-1");
        let base = edge(EdgeRelation::Supports, &run, &claim);

        // The relation moves it.
        assert_ne!(
            base.identity(),
            edge(EdgeRelation::Refutes, &run, &claim).identity()
        );
        // The checker moves it — two checkers are two claims about who checked.
        assert_ne!(
            edge(EdgeRelation::CheckedBy(checker("service:a")), &run, &claim).identity(),
            edge(EdgeRelation::CheckedBy(checker("service:b")), &run, &claim).identity()
        );
        // The endpoints move it, and so does their direction.
        assert_ne!(
            base.identity(),
            edge(EdgeRelation::Supports, &other, &claim).identity()
        );
        assert_ne!(
            base.identity(),
            edge(EdgeRelation::Supports, &claim, &run).identity()
        );
        // The evidence moves it.
        let with_evidence = EvidenceEdge::new(
            EdgeRelation::Supports,
            NodeRef::of(&run),
            NodeRef::of(&claim),
            [ArtifactRef::new("cert_9f").expect("well formed")],
            provenance("agent:swarm-1"),
        )
        .expect("distinct endpoints");
        assert_ne!(base.identity(), with_evidence.identity());
        // The producer moves it: docs/44 keeps credit, and plan §11.7 forbids resolving by
        // write order, so two producers are two edges rather than one edge with a winner.
        let other_producer = EvidenceEdge::new(
            EdgeRelation::Supports,
            NodeRef::of(&run),
            NodeRef::of(&claim),
            [],
            provenance("agent:swarm-2"),
        )
        .expect("distinct endpoints");
        assert_ne!(base.identity(), other_producer.identity());
        // …and an identical assertion is one edge.
        assert_eq!(
            base.identity(),
            edge(EdgeRelation::Supports, &run, &claim).identity()
        );
    }

    #[test]
    fn evidence_is_a_set_in_canonical_order() {
        let claim = node(NodeKind::Property, "prop_9f", "claim-1");
        let run = node(NodeKind::Run, "trace_9f", "claim-1");
        let build = |order: [&str; 3]| {
            EvidenceEdge::new(
                EdgeRelation::Supports,
                NodeRef::of(&run),
                NodeRef::of(&claim),
                order.map(|text| ArtifactRef::new(text).expect("well formed")),
                provenance("agent:swarm-1"),
            )
            .expect("distinct endpoints")
        };
        let forward = build(["cert_a", "cert_b", "cert_a"]);
        let backward = build(["cert_b", "cert_a", "cert_b"]);
        let listed: Vec<&str> = forward.evidence().map(ArtifactRef::as_str).collect();
        assert_eq!(listed, ["cert_a", "cert_b"]);
        assert_eq!(forward.identity(), backward.identity());
    }

    #[test]
    fn every_candidate_answer_to_f14_is_expressible() {
        // bn-3sypm's three candidates for the CHECKED_BY to-handle's node kind. The default
        // answers none of them.
        assert_eq!(
            CHECKED_BY_TARGET_CANDIDATES,
            [NodeKind::Run, NodeKind::Certificate, NodeKind::Receipt]
        );
        let undecided = CheckTargetRule::default();
        assert!(undecided.is_undecided());
        for kind in NodeKind::ALL {
            assert!(undecided.admits(kind), "{kind}");
        }
        // Each candidate answer, once RFC 0038 makes it, is one value of this type — and
        // each of them refuses the other two, so the rule is not vacuous.
        for candidate in CHECKED_BY_TARGET_CANDIDATES {
            let rule = CheckTargetRule::one_of([candidate]);
            assert!(!rule.is_undecided());
            assert!(rule.admits(candidate));
            for other in CHECKED_BY_TARGET_CANDIDATES {
                assert_eq!(rule.admits(other), other == candidate);
            }
        }
        // …as is the union, if that is what the RFC decides.
        let union = CheckTargetRule::one_of(CHECKED_BY_TARGET_CANDIDATES);
        for kind in NodeKind::ALL {
            assert_eq!(
                union.admits(kind),
                CHECKED_BY_TARGET_CANDIDATES.contains(&kind),
                "{kind}"
            );
        }
    }

    #[test]
    fn the_record_is_the_schema_object() {
        let handle = |text: &str| crate::identity::EvidenceHandle::new(text).expect("well formed");
        let claim = node(NodeKind::Property, "prop_9f", "claim-1");
        let run = node(NodeKind::Run, "trace_9f", "claim-1");
        let members = |value: &Value| -> Vec<String> {
            match value {
                Value::Record(fields) => {
                    let mut names: Vec<String> =
                        fields.keys().map(|name| name.as_str().to_owned()).collect();
                    names.sort();
                    names
                }
                other => panic!("expected a record, got {other:?}"),
            }
        };

        let plain = edge(EdgeRelation::Supports, &run, &claim);
        assert_eq!(
            members(&plain.to_record(&handle("ev_e"), &handle("ev_a"), &handle("ev_b"))),
            [
                "edge_id",
                "from",
                "kind",
                "provenance",
                "schema_epoch",
                "schema_id",
                "to"
            ]
        );
        // `checker` appears exactly for CHECKED_BY — the schema's one conditional.
        let checked = edge(
            EdgeRelation::CheckedBy(checker("service:kernel-core")),
            &run,
            &claim,
        );
        assert!(
            members(&checked.to_record(&handle("ev_e"), &handle("ev_a"), &handle("ev_b")))
                .contains(&"checker".to_owned())
        );
        assert!(
            !members(&plain.to_record(&handle("ev_e"), &handle("ev_a"), &handle("ev_b")))
                .contains(&"checker".to_owned())
        );
    }
}
