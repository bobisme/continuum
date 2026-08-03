//! The expansion protocol (RFC 0028, "Expansion protocol"): the closed relation
//! vocabulary, the query an omission points at, the depth it is asked to, and the
//! **content-derived handle** the child pack that answers it will carry.
//!
//! # Scope: PR-11 / IMPL-04
//!
//! > `context.expand(context, anchor, relation, depth?)` returns a new immutable pack
//! > referencing its parent. Everything beyond the default result is reachable by
//! > expansion, never by dumping.
//! >
//! > — RFC 0028, "Expansion protocol"
//!
//! # Why the handle is derived and not minted
//!
//! Two normative sentences meet here:
//!
//! > For `context.expand` the pack's `question` is the canonical encoding of the triple
//! > (relation, anchor, depth), so two expansions selecting the same items are distinct
//! > artifacts answering different questions.
//! >
//! > — RFC 0027 C2 (RFC 0028 restates it as "The question is part of the identity")
//!
//! > `context.expand` is a `@mutation`, so the request carries an `idempotency_key`; a
//! > replay with a byte-identical canonical request MUST return the same `ctx_*`.
//! >
//! > — RFC 0028, "Expansion protocol"
//!
//! A minted identity satisfies the second only through the idempotency ledger — that is,
//! only for as long as the ledger remembers, only within one actor's key space, and never
//! across two daemons. [`ExpansionHandle::derive`] makes it a property of the *question*
//! instead: the handle is the ADR-0013 content identity of the canonical encoding of
//! (parent, relation, anchor, depth), so the same request names the same `ctx_*` in every
//! process, and two different questions cannot collide onto one pack even when they select
//! the same items. The ledger still does its own job — refusing one key for two different
//! requests — but idempotency here does not depend on it.
//!
//! That is also what makes an expansion handle **exact** rather than advisory: a parent's
//! omission record names a query, the handle that query resolves to is derivable from the
//! record alone, and a caller can therefore check that the pack it got back is the pack the
//! manifest promised. G0-DX-01's Context Pack shape asks for exact expansion handles; this
//! is the sense in which they are exact.
//!
//! The parent is in the preimage and *not* in the `question` string: RFC 0028 fixes
//! `question` as the triple, and two parents' expansions along one triple are two different
//! packs, so the identity needs the fourth term the question does not carry.
//!
//! # What an expansion payload is, and the invariant it cannot be built without
//!
//! [`ExpansionPayload`] pairs an omission record with the items that record accounts for.
//! Its constructor refuses a payload whose item count disagrees with the record's `count`,
//! and one whose items are not of the record's `kind` — so "counts are exact" is a
//! statement about a value that could not otherwise be constructed, rather than a check
//! somebody has to run. An irretrievable record carries no items at all, which is the same
//! invariant read from the other side.
//!
//! # Clause → test
//!
//! | Clause | Source | Test |
//! |---|---|---|
//! | eleven relations, the IDL's tokens, in its order | `continuumd-native-protocol.idl` `ExpansionRelation`; the schema's `$defs.expansion_relation` | `the_relation_set_is_exactly_the_schema_enum` |
//! | fail closed on an unrecognized relation | RFC 0028 correction 2 | `an_unknown_relation_token_does_not_parse` |
//! | `depth` defaults to 1 | RFC 0028, "Expansion protocol" | `an_absent_depth_is_one` |
//! | a zero depth is not a depth | this bone's reading, recorded below | `a_zero_depth_is_refused` |
//! | the handle is a function of the question alone | RFC 0028 identity + idempotency | `one_question_derives_one_handle_in_any_process` |
//! | different questions are different packs | RFC 0027 C2 | `every_term_of_the_question_moves_the_handle` |
//! | counts are exact | RFC 0028, "Omission manifest" | `a_payload_whose_count_disagrees_with_its_items_is_refused` |

use core::fmt;
use core::num::NonZeroU32;

use continuum_intent::canonical_json::Json;
use continuum_value::identity::ContentHasher;
use continuum_value::value::Name;
use continuum_workspace::artifact_path::{ArtifactClass, ArtifactHandle};

use crate::omission::OmissionRecord;
use crate::selection::{SelectedItem, SelectionKind};

/// One member of the IDL's closed eleven-member `ExpansionRelation` enum.
///
/// Declared in the IDL's own order, which is also the schema's `$defs.expansion_relation`
/// order and this type's [`Ord`]. The IDL decides this vocabulary — "the schema types
/// `expansions[].relation` and `omissions[].expansion.relation` as free strings; the IDL
/// declares a closed eleven-member enum and `context.expand` accepts nothing else" (RFC
/// 0028 correction 2) — and the schema has since been corrected to it, so the two agree
/// token for token and this enum is transcribed from both.
///
/// `continuumd::protocol::vocabulary::ExpansionRelation` is the same eleven tokens on the
/// wire. It is not imported here and cannot be: `continuumd` sits *below* this crate in the
/// dependency islands and depends on it, not the reverse. The two are held to one
/// vocabulary by a test in `continuumd`, which is the one crate that sees both.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ExpansionRelation {
    /// `causal_predecessors` — the anchor's causal past.
    CausalPredecessors,
    /// `causal_successors` — the anchor's causal future.
    CausalSuccessors,
    /// `conflicts_with` — the events that conflict with the anchor.
    ConflictsWith,
    /// `same_owner` — items owned by the anchor's task or node.
    SameOwner,
    /// `property_automaton_step` — the automaton transition the anchor drove.
    PropertyAutomatonStep,
    /// `proof_dependency` — the anchor's proof-dependency slice.
    ProofDependency,
    /// `source_span` — the source spans the anchor corresponds to.
    SourceSpan,
    /// `assumption_uses` — the assumptions the anchor uses.
    AssumptionUses,
    /// `abstraction_of` — the abstract counterpart of the anchor.
    AbstractionOf,
    /// `refinement_of` — the concrete counterpart of the anchor.
    RefinementOf,
    /// `alternate_branch` — a counterfactual branch beside the anchor.
    AlternateBranch,
}

impl ExpansionRelation {
    /// Every relation, in the IDL's declaration order.
    pub const ALL: [Self; 11] = [
        Self::CausalPredecessors,
        Self::CausalSuccessors,
        Self::ConflictsWith,
        Self::SameOwner,
        Self::PropertyAutomatonStep,
        Self::ProofDependency,
        Self::SourceSpan,
        Self::AssumptionUses,
        Self::AbstractionOf,
        Self::RefinementOf,
        Self::AlternateBranch,
    ];

    /// The wire token, byte-identical to the IDL's and the schema's.
    #[must_use]
    pub const fn as_wire_str(self) -> &'static str {
        match self {
            Self::CausalPredecessors => "causal_predecessors",
            Self::CausalSuccessors => "causal_successors",
            Self::ConflictsWith => "conflicts_with",
            Self::SameOwner => "same_owner",
            Self::PropertyAutomatonStep => "property_automaton_step",
            Self::ProofDependency => "proof_dependency",
            Self::SourceSpan => "source_span",
            Self::AssumptionUses => "assumption_uses",
            Self::AbstractionOf => "abstraction_of",
            Self::RefinementOf => "refinement_of",
            Self::AlternateBranch => "alternate_branch",
        }
    }

    /// Recover a relation from its wire token.
    ///
    /// `None` for anything outside the closed set — "`relation` is not a member of
    /// `ExpansionRelation` → error `MalformedRequest` — an unknown member of a closed enum"
    /// (RFC 0028's typed-outcome table).
    #[must_use]
    pub fn from_wire_str(text: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|relation| relation.as_wire_str() == text)
    }
}

impl fmt::Display for ExpansionRelation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_wire_str())
    }
}

/// One expansion query: `{relation, anchor}` — the schema's `expansions[]` item and an
/// omission record's `expansion` object, which are one shape.
///
/// `anchor` is a [`Name`] rather than the schema's bare string, on the reading
/// [`crate::source::SourceRef`] applies to `file`: an anchor "MUST resolve in the parent —
/// it is either a `selected[].id` or an anchor named by one of the parent's own
/// `expansions[]` or `omissions[].expansion` entries", and `selected[].id` is already a
/// `Name` in this crate, so a query anchored at anything a `Name` cannot spell resolves to
/// nothing by construction.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ExpansionQuery {
    relation: ExpansionRelation,
    anchor: Name,
}

impl ExpansionQuery {
    /// Build a query.
    #[must_use]
    pub const fn new(relation: ExpansionRelation, anchor: Name) -> Self {
        Self { relation, anchor }
    }

    /// Which of the eleven relations to follow.
    #[must_use]
    pub const fn relation(&self) -> ExpansionRelation {
        self.relation
    }

    /// The item the expansion is rooted at.
    #[must_use]
    pub const fn anchor(&self) -> &Name {
        &self.anchor
    }

    /// The canonical JSON rendering: `{"anchor", "relation"}` in ID5 key order.
    #[must_use]
    pub fn to_json(&self) -> Json {
        Json::object([
            ("anchor".to_owned(), Json::String(self.anchor.to_string())),
            (
                "relation".to_owned(),
                Json::String(self.relation.as_wire_str().to_owned()),
            ),
        ])
        .expect("the two keys above are distinct string literals")
    }
}

impl fmt::Display for ExpansionQuery {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}@{}", self.relation, self.anchor)
    }
}

/// How far an expansion is asked to reach.
///
/// > `depth` defaults to 1. A depth the engine cannot honour in full is a shortfall,
/// > recorded in the child's manifest; it is never silently clamped.
/// >
/// > — RFC 0028, "Expansion protocol"
///
/// Zero is refused rather than treated as the default or as "no limit". The IDL types
/// `depth: U32 optional`, so zero is spellable on the wire, and neither RFC gives it a
/// meaning; a request whose depth reaches nothing is `MalformedRequest` on the same reading
/// that makes an unknown enum member one — a value inside the type and outside the
/// vocabulary. Absence, which the IDL *does* give a meaning, is [`Depth::DEFAULT`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Depth(NonZeroU32);

impl Depth {
    /// The depth an absent `depth` argument means: one hop.
    pub const DEFAULT: Self = Self(NonZeroU32::MIN);

    /// Build a depth.
    ///
    /// # Errors
    ///
    /// [`ExpansionError::ZeroDepth`] for zero — see the type documentation.
    pub const fn new(value: u32) -> Result<Self, ExpansionError> {
        match NonZeroU32::new(value) {
            Some(value) => Ok(Self(value)),
            None => Err(ExpansionError::ZeroDepth),
        }
    }

    /// The depth an optional wire argument names: [`Depth::DEFAULT`] when absent.
    ///
    /// # Errors
    ///
    /// [`ExpansionError::ZeroDepth`], as [`Depth::new`].
    pub const fn from_optional(value: Option<u32>) -> Result<Self, ExpansionError> {
        match value {
            Some(value) => Self::new(value),
            None => Ok(Self::DEFAULT),
        }
    }

    /// The number of hops.
    #[must_use]
    pub const fn get(self) -> u32 {
        self.0.get()
    }
}

impl fmt::Display for Depth {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// The canonical rendering of an expansion's question — the pack's `question` field.
///
/// > `question` MUST be the canonical rendering of the compiled query: […] for
/// > `context.expand`, the canonical encoding of the triple (relation, anchor, depth).
/// >
/// > — RFC 0028, "Pack identity, lineage, and determinism"
///
/// The canonical encoding is an ID5 canonical-JSON object — this crate's one encoding, the
/// same writer `selected[]` goes through — so the triple has exactly one spelling and the
/// field is comparable byte for byte rather than parsed out of prose.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ExpansionQuestion(String);

impl ExpansionQuestion {
    /// The question a `(query, depth)` pair asks.
    #[must_use]
    pub fn of(query: &ExpansionQuery, depth: Depth) -> Self {
        Self(question_json(query, depth).to_string())
    }

    /// The canonical text, as the pack's `question` field carries it.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ExpansionQuestion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

fn question_json(query: &ExpansionQuery, depth: Depth) -> Json {
    Json::object([
        (
            "anchor".to_owned(),
            Json::String(query.anchor().to_string()),
        ),
        ("depth".to_owned(), Json::Integer(i64::from(depth.get()))),
        (
            "relation".to_owned(),
            Json::String(query.relation().as_wire_str().to_owned()),
        ),
    ])
    .expect("the three keys above are pairwise distinct string literals")
}

/// The `ctx_*` a given expansion of a given parent resolves to.
///
/// Derived, never minted — see this module's "Why the handle is derived and not minted".
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ExpansionHandle(ArtifactHandle);

impl ExpansionHandle {
    /// Derive the handle of the child pack that answers `(query, depth)` against `parent`.
    ///
    /// The preimage is the ID5 canonical encoding of
    /// `{"anchor", "depth", "parent", "relation"}` — the question's three terms plus the
    /// parent's own handle — and the identity is `H`'s digest of it, spelled as the
    /// identity half of a plan §4.4 `ctx_` handle. One canonical object, so no pair of
    /// inputs can produce another pair's preimage by concatenation.
    ///
    /// # Errors
    ///
    /// [`ExpansionError::NotAPack`] when `parent` is not a `ctx_*` handle: an expansion of
    /// something that is not a Context Pack has no meaning, and deriving a `ctx_*` for it
    /// would invent a lineage.
    pub fn derive<H: ContentHasher>(
        parent: &ArtifactHandle,
        query: &ExpansionQuery,
        depth: Depth,
    ) -> Result<Self, ExpansionError> {
        if parent.class() != ArtifactClass::ContextPack {
            return Err(ExpansionError::NotAPack {
                class: parent.class(),
            });
        }
        let mut fields = question_json(query, depth)
            .as_object()
            .cloned()
            .expect("`question_json` builds an object");
        fields.insert("parent".to_owned(), Json::String(parent.to_string()));
        let preimage = Json::Object(fields).to_canonical_bytes();
        let handle =
            ArtifactHandle::new(ArtifactClass::ContextPack, &H::hash(&preimage).to_token())
                .expect("a lowercase-hex digest token is a well-formed artifact identity");
        Ok(Self(handle))
    }

    /// The handle, as an artifact handle.
    #[must_use]
    pub const fn handle(&self) -> &ArtifactHandle {
        &self.0
    }
}

impl fmt::Display for ExpansionHandle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// What one omission record accounts for: the record, and the items it names.
///
/// The pairing is the whole point. A manifest record on its own is a *claim* that some
/// number of items of some kind were dropped; a payload is that claim beside the items, and
/// its constructor refuses every disagreement between the two. A compiler that dropped an
/// item without counting it, or counted an item it did not drop, cannot produce a payload
/// at all.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExpansionPayload {
    record: OmissionRecord,
    items: Vec<SelectedItem>,
}

impl ExpansionPayload {
    /// Pair a record with the items it accounts for.
    ///
    /// # Errors
    ///
    /// - [`ExpansionError::CountDisagrees`] when the record's `count` is not the number of
    ///   items — "counts are exact", checked where the two meet;
    /// - [`ExpansionError::KindDisagrees`] when an item is not of the record's `kind` — the
    ///   manifest partitions candidates *by kind*, so an item of another kind is accounted
    ///   for in the wrong cell;
    /// - [`ExpansionError::IrretrievableCarriesItems`] when an irretrievable record is
    ///   given items. Content that is gone cannot also be here.
    pub fn new(record: OmissionRecord, items: Vec<SelectedItem>) -> Result<Self, ExpansionError> {
        if let Some(item) = items.iter().find(|item| item.kind() != record.kind()) {
            return Err(ExpansionError::KindDisagrees {
                record: record.kind(),
                item: item.kind(),
            });
        }
        if record.retrievability().is_expandable() {
            let counted = usize::try_from(record.count()).unwrap_or(usize::MAX);
            if counted != items.len() {
                return Err(ExpansionError::CountDisagrees {
                    counted: record.count(),
                    held: items.len(),
                });
            }
        } else if !items.is_empty() {
            // An irretrievable record's `count` is a claim about content that is *gone*, so
            // there is nothing to compare it against — the exactness this arm can enforce is
            // that nothing is here. A record claiming three purged items and carrying two of
            // them is the contradiction, not the arithmetic.
            return Err(ExpansionError::IrretrievableCarriesItems { items: items.len() });
        }
        Ok(Self { record, items })
    }

    /// An irretrievable group: a record and, necessarily, nothing.
    ///
    /// # Errors
    ///
    /// As [`ExpansionPayload::new`].
    pub fn irretrievable(record: OmissionRecord) -> Result<Self, ExpansionError> {
        Self::new(record, Vec::new())
    }

    /// The manifest record.
    #[must_use]
    pub const fn record(&self) -> &OmissionRecord {
        &self.record
    }

    /// The items the record accounts for.
    #[must_use]
    pub fn items(&self) -> &[SelectedItem] {
        &self.items
    }

    /// The query that retrieves this group, when one does.
    #[must_use]
    pub const fn query(&self) -> Option<&ExpansionQuery> {
        self.record.retrievability().query()
    }
}

/// A value the expansion protocol refuses to build.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExpansionError {
    /// A depth of zero, which reaches nothing and means nothing.
    ZeroDepth,
    /// A handle offered as an expansion parent that is not a Context Pack.
    NotAPack {
        /// The class the offered handle actually carries.
        class: ArtifactClass,
    },
    /// A record's exact count is not the number of items it was paired with.
    CountDisagrees {
        /// What the record claims.
        counted: u32,
        /// What it was given.
        held: usize,
    },
    /// An item is not of the kind its record accounts for.
    KindDisagrees {
        /// The record's kind.
        record: SelectionKind,
        /// The offending item's kind.
        item: SelectionKind,
    },
    /// An irretrievable record was paired with items.
    IrretrievableCarriesItems {
        /// How many.
        items: usize,
    },
}

impl fmt::Display for ExpansionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ZeroDepth => f.write_str("an expansion depth of zero reaches nothing"),
            Self::NotAPack { class } => {
                write!(f, "an expansion parent is a `ctx_*` pack, not a `{class}`")
            }
            Self::CountDisagrees { counted, held } => write!(
                f,
                "the omission record counts {counted} items and was paired with {held}; a \
                 manifest count is exact"
            ),
            Self::KindDisagrees { record, item } => write!(
                f,
                "the omission record accounts for `{record}` items and was paired with a \
                 `{item}` item"
            ),
            Self::IrretrievableCarriesItems { items } => write!(
                f,
                "an irretrievable omission was paired with {items} items; content that is \
                 gone cannot also be present"
            ),
        }
    }
}

impl core::error::Error for ExpansionError {}

#[cfg(test)]
mod tests {
    use continuum_value::identity::Blake3Hasher;
    use continuum_workspace::snapshot::WorkspacePath;

    use super::*;
    use crate::omission::{IrretrievableReason, OmissionReason};
    use crate::source::{SourceRef, SourceSpan};

    /// Transcribed from `notes/plan/schemas/context-pack.schema.json`,
    /// `$defs.expansion_relation.enum`, in file order — which the schema's own `$comment`
    /// records as corrected to the IDL's `ExpansionRelation`.
    const SCHEMA_RELATION_ENUM: [&str; 11] = [
        "causal_predecessors",
        "causal_successors",
        "conflicts_with",
        "same_owner",
        "property_automaton_step",
        "proof_dependency",
        "source_span",
        "assumption_uses",
        "abstraction_of",
        "refinement_of",
        "alternate_branch",
    ];

    fn name(text: &str) -> Name {
        Name::new(text).expect("well formed")
    }

    fn parent() -> ArtifactHandle {
        ArtifactHandle::new(ArtifactClass::ContextPack, "parent1").expect("well formed")
    }

    fn query() -> ExpansionQuery {
        ExpansionQuery::new(ExpansionRelation::SourceSpan, name("e_1"))
    }

    fn source_item(id: &str) -> SelectedItem {
        let span = SourceSpan::new(
            WorkspacePath::new("src/lib.rs").expect("a repo-relative path"),
            1,
            1,
            2,
            1,
        )
        .expect("a well-formed span");
        SourceRef::new(span).into_selected_item(name(id))
    }

    #[test]
    fn the_relation_set_is_exactly_the_schema_enum() {
        let ours: Vec<&str> = ExpansionRelation::ALL
            .iter()
            .map(|relation| relation.as_wire_str())
            .collect();
        assert_eq!(ours, SCHEMA_RELATION_ENUM);
    }

    #[test]
    fn an_unknown_relation_token_does_not_parse() {
        for bad in [
            "",
            "source",
            "SourceSpan",
            "source_span ",
            "causal-predecessors",
        ] {
            assert_eq!(
                ExpansionRelation::from_wire_str(bad),
                None,
                "{bad:?} must be refused"
            );
        }
        for relation in ExpansionRelation::ALL {
            assert_eq!(
                ExpansionRelation::from_wire_str(relation.as_wire_str()),
                Some(relation)
            );
        }
    }

    #[test]
    fn an_absent_depth_is_one() {
        assert_eq!(Depth::from_optional(None), Ok(Depth::DEFAULT));
        assert_eq!(Depth::DEFAULT.get(), 1);
        assert_eq!(Depth::from_optional(Some(3)).map(Depth::get), Ok(3));
    }

    #[test]
    fn a_zero_depth_is_refused() {
        assert_eq!(Depth::new(0), Err(ExpansionError::ZeroDepth));
        assert_eq!(
            Depth::from_optional(Some(0)),
            Err(ExpansionError::ZeroDepth)
        );
    }

    #[test]
    fn the_question_is_the_canonical_encoding_of_the_triple() {
        let question = ExpansionQuestion::of(&query(), Depth::DEFAULT);
        assert_eq!(
            question.as_str(),
            r#"{"anchor":"e_1","depth":1,"relation":"source_span"}"#
        );
    }

    #[test]
    fn one_question_derives_one_handle_in_any_process() {
        let first = ExpansionHandle::derive::<Blake3Hasher>(&parent(), &query(), Depth::DEFAULT)
            .expect("derives");
        let second = ExpansionHandle::derive::<Blake3Hasher>(&parent(), &query(), Depth::DEFAULT)
            .expect("derives");
        assert_eq!(first, second);
        assert!(first.to_string().starts_with("ctx_"));
    }

    #[test]
    fn every_term_of_the_question_moves_the_handle() {
        // RFC 0027 C2: two expansions that happen to select the same items are distinct
        // artifacts when they answer different questions, so every term of the question —
        // and the parent, which the question does not carry — must reach the identity.
        let base = ExpansionHandle::derive::<Blake3Hasher>(&parent(), &query(), Depth::DEFAULT)
            .expect("derives");
        let other_relation = ExpansionHandle::derive::<Blake3Hasher>(
            &parent(),
            &ExpansionQuery::new(ExpansionRelation::CausalPredecessors, name("e_1")),
            Depth::DEFAULT,
        )
        .expect("derives");
        let other_anchor = ExpansionHandle::derive::<Blake3Hasher>(
            &parent(),
            &ExpansionQuery::new(ExpansionRelation::SourceSpan, name("e_2")),
            Depth::DEFAULT,
        )
        .expect("derives");
        let other_depth = ExpansionHandle::derive::<Blake3Hasher>(
            &parent(),
            &query(),
            Depth::new(2).expect("nonzero"),
        )
        .expect("derives");
        let other_parent = ExpansionHandle::derive::<Blake3Hasher>(
            &ArtifactHandle::new(ArtifactClass::ContextPack, "parent2").expect("well formed"),
            &query(),
            Depth::DEFAULT,
        )
        .expect("derives");
        let all = [
            &base,
            &other_relation,
            &other_anchor,
            &other_depth,
            &other_parent,
        ];
        for (index, one) in all.iter().enumerate() {
            for other in &all[index + 1..] {
                assert_ne!(one, other, "two distinct questions share a handle");
            }
        }
    }

    #[test]
    fn a_handle_is_derived_only_against_a_pack() {
        let not_a_pack =
            ArtifactHandle::new(ArtifactClass::WorkspaceSnapshot, "abc").expect("well formed");
        assert_eq!(
            ExpansionHandle::derive::<Blake3Hasher>(&not_a_pack, &query(), Depth::DEFAULT),
            Err(ExpansionError::NotAPack {
                class: ArtifactClass::WorkspaceSnapshot,
            })
        );
    }

    #[test]
    fn a_payload_whose_count_disagrees_with_its_items_is_refused() {
        let record =
            OmissionRecord::expandable(SelectionKind::Source, 2, OmissionReason::Budget, query());
        assert_eq!(
            ExpansionPayload::new(record.clone(), vec![source_item("s_1")]),
            Err(ExpansionError::CountDisagrees {
                counted: 2,
                held: 1,
            })
        );
        let payload = ExpansionPayload::new(record, vec![source_item("s_1"), source_item("s_2")])
            .expect("two items for a count of two");
        assert_eq!(payload.items().len(), 2);
    }

    #[test]
    fn a_payload_whose_items_are_of_another_kind_is_refused() {
        let record =
            OmissionRecord::expandable(SelectionKind::Event, 1, OmissionReason::Budget, query());
        assert_eq!(
            ExpansionPayload::new(record, vec![source_item("s_1")]),
            Err(ExpansionError::KindDisagrees {
                record: SelectionKind::Event,
                item: SelectionKind::Source,
            })
        );
    }

    #[test]
    fn an_irretrievable_payload_holds_nothing() {
        let record =
            OmissionRecord::irretrievable(SelectionKind::Source, 1, IrretrievableReason::Redaction);
        assert_eq!(
            ExpansionPayload::new(record.clone(), vec![source_item("s_1")]),
            Err(ExpansionError::IrretrievableCarriesItems { items: 1 })
        );
        let payload = ExpansionPayload::irretrievable(record).expect("no items");
        assert!(payload.items().is_empty());
        assert_eq!(payload.query(), None);
    }
}
