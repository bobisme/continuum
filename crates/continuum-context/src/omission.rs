//! The omission manifest (RFC 0028, "Omission manifest"; `context-pack.schema.json`
//! `omissions`) — INV-007's half of the pack: everything the pack dropped, named, counted
//! exactly, and paired with the query that retrieves it.
//!
//! # Scope: PR-11 / IMPL-04
//!
//! > omitted-node counts and expansion queries | `omissions[]`, `expansions[]` | arrays |
//! > Counts are exact
//! >
//! > — RFC 0028, "Required fields, reconciled with plan §6.2"
//!
//! This module owns the record shape and the closed reason vocabulary; [`crate::accounting`]
//! owns the discipline that makes a manifest *complete* rather than merely well-formed, and
//! [`crate::expansion`] owns the query a record points at.
//!
//! # The schema's two conditionals are types here, not validations
//!
//! `context-pack.schema.json` states them as JSON-Schema `if`/`then` pairs:
//!
//! - "INV-007: an expandable omission names the query that retrieves it" — `expandable:
//!   true` requires `expansion`;
//! - "when `expandable` is false the reason MUST be one that explains irretrievability.
//!   `budget`, `heuristic-cutoff`, and `slice-irrelevant` omissions are expandable by
//!   construction, so they cannot be declared irretrievable."
//!
//! Both are unrepresentable here rather than checked. [`Retrievability`] is a two-arm enum:
//! its expandable arm *carries* the [`ExpansionQuery`], so a record silent about retrieval
//! cannot be built, and its irretrievable arm carries an [`IrretrievableReason`] — a
//! two-member enum, `redaction` and `unsupported` — so a `budget` omission declared
//! irretrievable has no spelling. There is no constructor that takes an `expandable: bool`
//! and a reason independently, which is why there is no validation to forget.
//!
//! # `kind` is the closed selection vocabulary, and the schema's free string is wider
//!
//! The schema types `omissions[].kind` as an unconstrained string, and the shipped example
//! used to write a descriptive phrase (`"observer-independent causal predecessor"`) there.
//! RFC 0028 correction 16 settles the reading: `omissions[].kind` names the same closed
//! eleven-member `selected[].kind` vocabulary [`SelectionKind`] is, and the example is
//! corrected to `"event"` (the 196 items are `e_ack`'s causal-predecessor events). This
//! module narrows to [`SelectionKind`] for three reasons, and the narrowing is *inside*
//! what the schema admits, so a pack written here still validates — a closed `enum` at
//! this key is a shape change and is raised for the schema sweep as RFC 0028 F13, not made
//! here:
//!
//! 1. the manifest partitions the **candidates for selection**, and a candidate's kind is
//!    exactly that vocabulary — a partition keyed by free-form phrases cannot be checked
//!    for exactness, which is the one property RFC 0028 requires of it;
//! 2. `rule ordering.deterministic` needs a sort key with one spelling per member;
//! 3. INV-016 — a free-string `kind` is a place source-derived prose could reach the
//!    artifact, and a closed token is not. This is the same strengthening
//!    [`crate::source::SourceRef`] applied to the wire's bare `file: String`.
//!
//! # Clause → test
//!
//! | Clause | Source | Test |
//! |---|---|---|
//! | five reasons, the schema's tokens, in the schema's order | `context-pack.schema.json` `omissions.items.reason` | `the_reason_set_is_exactly_the_schema_enum` |
//! | fail closed on an unrecognized token | RFC 0028, "Versioning and revision" | `an_unknown_reason_token_does_not_parse` |
//! | `expandable` is present on every record | RFC 0028 F12, paid in the schema | `every_record_declares_its_retrievability` |
//! | an irretrievable record's reason explains irretrievability | `context.schema` second conditional | `only_two_reasons_can_spell_irretrievability` |
//! | an expandable record names its query | INV-007 | `an_expandable_record_carries_the_query_that_retrieves_it` |
//! | the record's keys are exactly the schema's, ID5-sorted | `context-pack.schema.json`, RFC 0037 ID5 | `canonical_json_has_exactly_the_schema_keys_in_sorted_order` |
//! | a manifest is deterministically ordered | `rule ordering.deterministic` | `records_sort_into_one_order_whatever_order_they_arrive_in` |
//! | one partition cell is one record | RFC 0028, "Counts are exact" | `a_repeated_partition_cell_is_refused` |

use core::fmt;

use continuum_intent::canonical_json::Json;

use crate::expansion::ExpansionQuery;
use crate::selection::SelectionKind;

/// One member of `context-pack.schema.json`'s closed `omissions[].reason` enum — the IDL's
/// `OmissionReason`, token for token.
///
/// Declared in the schema's own enum order, which is also this type's [`Ord`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum OmissionReason {
    /// `budget` — dropped by budget packing (RFC 0028, "Budgets and packing").
    Budget,
    /// `redaction` — withheld by field policy from *this* caller (plan §18.4).
    Redaction,
    /// `unsupported` — outside the engine's semantics (`rule errors.unsupported_surface`).
    Unsupported,
    /// `heuristic-cutoff` — dropped by the stage-9 ranker; the compiler could not decide.
    HeuristicCutoff,
    /// `slice-irrelevant` — *provably* outside the property-directed slice. Reserved for
    /// exactly that: "an item dropped because the compiler could not decide is
    /// `heuristic-cutoff`, never `slice-irrelevant`" (RFC 0028).
    SliceIrrelevant,
}

impl OmissionReason {
    /// Every reason, in the schema's declaration order.
    pub const ALL: [Self; 5] = [
        Self::Budget,
        Self::Redaction,
        Self::Unsupported,
        Self::HeuristicCutoff,
        Self::SliceIrrelevant,
    ];

    /// The wire token, byte-identical to the schema's and the IDL's.
    #[must_use]
    pub const fn as_wire_str(self) -> &'static str {
        match self {
            Self::Budget => "budget",
            Self::Redaction => "redaction",
            Self::Unsupported => "unsupported",
            Self::HeuristicCutoff => "heuristic-cutoff",
            Self::SliceIrrelevant => "slice-irrelevant",
        }
    }

    /// Recover a reason from its wire token.
    ///
    /// `None` for anything outside the closed set: a consumer that reads an omission reason
    /// it does not recognize "MUST reject the pack with `MalformedRequest` […] and MUST NOT
    /// treat the unknown token as absent" (RFC 0028, "Fail closed on unrecognized tokens").
    #[must_use]
    pub fn from_wire_str(text: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|reason| reason.as_wire_str() == text)
    }

    /// Whether this reason can explain *irretrievability*.
    ///
    /// True for `redaction` and `unsupported` alone — the schema's second conditional.
    /// `budget`, `heuristic-cutoff`, and `slice-irrelevant` omissions "are expandable by
    /// construction, so they cannot be declared irretrievable".
    #[must_use]
    pub const fn admits_irretrievability(self) -> bool {
        matches!(self, Self::Redaction | Self::Unsupported)
    }
}

impl fmt::Display for OmissionReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_wire_str())
    }
}

/// The two reasons that can stand behind `expandable: false`.
///
/// A separate type rather than a runtime check on [`OmissionReason`]: the schema's
/// conditional is a statement about which reasons may appear in that position, and a type
/// whose members are exactly those reasons makes the other three unspellable there.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum IrretrievableReason {
    /// The content was withheld by field policy (plan §4.5, §18.4). The pack's
    /// `redactions[]` carries the typed `Redacted(reason, commitment)` stub.
    Redaction,
    /// The engine cannot produce the content in this protocol version.
    Unsupported,
}

impl IrretrievableReason {
    /// Both members, in [`OmissionReason`]'s order.
    pub const ALL: [Self; 2] = [Self::Redaction, Self::Unsupported];

    /// The manifest reason this irretrievability is recorded as.
    #[must_use]
    pub const fn reason(self) -> OmissionReason {
        match self {
            Self::Redaction => OmissionReason::Redaction,
            Self::Unsupported => OmissionReason::Unsupported,
        }
    }
}

impl fmt::Display for IrretrievableReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.reason().as_wire_str())
    }
}

/// Whether an omitted group can be retrieved, and how — the typed form of the schema's
/// `expandable`/`expansion` pair.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum Retrievability {
    /// `expandable: true`, and the query that retrieves the group.
    Expandable(ExpansionQuery),
    /// `expandable: false`, and the typed reason nothing retrieves it.
    Irretrievable(IrretrievableReason),
}

impl Retrievability {
    /// The schema's `expandable` boolean.
    #[must_use]
    pub const fn is_expandable(&self) -> bool {
        matches!(self, Self::Expandable(_))
    }

    /// The query that retrieves this group, when one exists.
    #[must_use]
    pub const fn query(&self) -> Option<&ExpansionQuery> {
        match self {
            Self::Expandable(query) => Some(query),
            Self::Irretrievable(_) => None,
        }
    }
}

/// One `omissions[]` record: `{kind, count, reason, expandable, expansion?}`.
///
/// Fields are ordered `kind`, `reason`, `retrievability`, `count` so that the derived
/// [`Ord`] is the manifest's sort key — RFC 0028's frontier is "the omission manifest keyed
/// by kind and reason", and `count` sorts last because the first three already identify a
/// partition cell uniquely inside a [`Manifest`].
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct OmissionRecord {
    kind: SelectionKind,
    reason: OmissionReason,
    retrievability: Retrievability,
    count: u32,
}

impl OmissionRecord {
    /// An expandable record: `count` items of `kind` were dropped for `reason`, and `query`
    /// retrieves them.
    ///
    /// Every one of the five reasons may be expandable — `redaction` and `unsupported`
    /// included, since a redaction the caller *is* in scope for and a surface a later
    /// protocol version serves are both retrievable.
    #[must_use]
    pub const fn expandable(
        kind: SelectionKind,
        count: u32,
        reason: OmissionReason,
        query: ExpansionQuery,
    ) -> Self {
        Self {
            kind,
            reason,
            retrievability: Retrievability::Expandable(query),
            count,
        }
    }

    /// An irretrievable record: `count` items of `kind` are gone, and `reason` is why
    /// nothing retrieves them.
    #[must_use]
    pub const fn irretrievable(
        kind: SelectionKind,
        count: u32,
        reason: IrretrievableReason,
    ) -> Self {
        Self {
            kind,
            reason: reason.reason(),
            retrievability: Retrievability::Irretrievable(reason),
            count,
        }
    }

    /// The kind of the omitted items.
    #[must_use]
    pub const fn kind(&self) -> SelectionKind {
        self.kind
    }

    /// The exact count of omitted items. Never an estimate, a sample, or a bucket.
    #[must_use]
    pub const fn count(&self) -> u32 {
        self.count
    }

    /// Why they were omitted.
    #[must_use]
    pub const fn reason(&self) -> OmissionReason {
        self.reason
    }

    /// Whether and how they are retrievable.
    #[must_use]
    pub const fn retrievability(&self) -> &Retrievability {
        &self.retrievability
    }

    /// The partition cell this record accounts for: `(kind, reason, retrievability)`.
    ///
    /// Two records of one manifest MUST NOT share a cell — that would double-count the
    /// cell's items and break the counting equation. [`Manifest::new`] refuses it.
    #[must_use]
    pub fn cell(&self) -> (SelectionKind, OmissionReason, &Retrievability) {
        (self.kind, self.reason, &self.retrievability)
    }

    /// The canonical JSON rendering: `{"count", "expandable", "expansion"?, "kind",
    /// "reason"}` in ID5 key order.
    ///
    /// `expansion` is *absent* on an irretrievable record rather than null: the schema
    /// carries `additionalProperties: false` and requires the key only under `expandable:
    /// true`, so a null there would be a fourth spelling of "no query" beside absent,
    /// `expandable: false`, and the reason.
    #[must_use]
    pub fn to_json(&self) -> Json {
        let mut fields = vec![
            ("count".to_owned(), Json::Integer(i64::from(self.count))),
            (
                "expandable".to_owned(),
                Json::Bool(self.retrievability.is_expandable()),
            ),
            (
                "kind".to_owned(),
                Json::String(self.kind.as_wire_str().to_owned()),
            ),
            (
                "reason".to_owned(),
                Json::String(self.reason.as_wire_str().to_owned()),
            ),
        ];
        if let Some(query) = self.retrievability.query() {
            fields.push(("expansion".to_owned(), query.to_json()));
        }
        Json::object(fields).expect("the five keys above are pairwise distinct string literals")
    }
}

/// The whole manifest: a deterministically ordered set of records, one per partition cell.
///
/// > An empty manifest is an assertion of completeness and is checkable: it says the
/// > selection is the whole candidate set.
/// >
/// > — RFC 0028, "Omission manifest"
///
/// So [`Manifest::empty`] is a *claim*, not a default, and [`crate::accounting`] is where a
/// caller earns the right to make it.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Manifest {
    records: Vec<OmissionRecord>,
}

impl Manifest {
    /// Build a manifest, sorting the records into their one canonical order.
    ///
    /// # Errors
    ///
    /// [`ManifestError::RepeatedCell`] when two records name one partition cell. The
    /// counting equation "candidate set = selection + the sum of the manifest counts" is
    /// exact only if the cells partition; two rows for one cell double-count it, and a
    /// silent merge would hide the disagreement that produced them.
    pub fn new(records: impl IntoIterator<Item = OmissionRecord>) -> Result<Self, ManifestError> {
        let mut records: Vec<OmissionRecord> = records.into_iter().collect();
        records.sort();
        if let Some(window) = records
            .windows(2)
            .find(|pair| pair[0].cell() == pair[1].cell())
        {
            return Err(ManifestError::RepeatedCell {
                kind: window[0].kind(),
                reason: window[0].reason(),
            });
        }
        Ok(Self { records })
    }

    /// Build a manifest from records that were accounted for *independently*, summing the
    /// counts of any two that land in one cell.
    ///
    /// The companion of [`Manifest::new`], and the difference is which mistake each is
    /// guarding against. `new` takes records a caller already partitioned, so two rows for
    /// one cell are a disagreement and merging them would hide it. `merged` takes records
    /// from disjoint groups — one per expansion payload, each exact over its own items —
    /// where landing in one cell is not a disagreement but the ordinary case: two purged
    /// groups of `source` items are two groups and one `(source, redaction)` partition
    /// cell, and their counts add. The sum is still exact, because the groups were
    /// disjoint.
    ///
    /// # Errors
    ///
    /// [`ManifestError::CountOverflow`] when a summed cell leaves the exact-count width.
    pub fn merged(
        records: impl IntoIterator<Item = OmissionRecord>,
    ) -> Result<Self, ManifestError> {
        let mut cells: Vec<OmissionRecord> = Vec::new();
        for record in records {
            if let Some(existing) = cells
                .iter_mut()
                .find(|existing| existing.cell() == record.cell())
            {
                existing.count = existing.count.checked_add(record.count).ok_or(
                    ManifestError::CountOverflow {
                        kind: record.kind(),
                        reason: record.reason(),
                    },
                )?;
            } else {
                cells.push(record);
            }
        }
        Self::new(cells)
    }

    /// The empty manifest — the assertion that the selection is the whole candidate set.
    #[must_use]
    pub const fn empty() -> Self {
        Self {
            records: Vec::new(),
        }
    }

    /// The records, in canonical order.
    #[must_use]
    pub fn records(&self) -> &[OmissionRecord] {
        &self.records
    }

    /// Whether this manifest asserts completeness.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }

    /// The exact total of everything this manifest accounts for.
    ///
    /// `u64` because the sum of `u32` counts is not a `u32`; the accounting equation adds
    /// this to a selection size and must not wrap on the way.
    #[must_use]
    pub fn total(&self) -> u64 {
        self.records
            .iter()
            .map(|record| u64::from(record.count))
            .sum()
    }

    /// The canonical JSON rendering of `omissions[]`.
    #[must_use]
    pub fn to_json(&self) -> Json {
        Json::Array(self.records.iter().map(OmissionRecord::to_json).collect())
    }

    /// The canonical JSON rendering of the `expansions[]` this manifest makes reachable.
    ///
    /// One entry per expandable record, in the same canonical order, carrying `relation`
    /// and `anchor` and **no `estimated_items`**: that field is "the only estimate in the
    /// artifact and MUST NOT be used to reconcile a manifest", and the exact count is one
    /// key away in `omissions[]`, so writing an estimate beside an exact number would
    /// invite exactly the reconciliation RFC 0028 forbids.
    #[must_use]
    pub fn expansions_json(&self) -> Json {
        Json::Array(
            self.records
                .iter()
                .filter_map(|record| record.retrievability.query())
                .map(ExpansionQuery::to_json)
                .collect(),
        )
    }
}

/// A manifest that does not partition.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ManifestError {
    /// Two records name one `(kind, reason, retrievability)` cell.
    RepeatedCell {
        /// The kind both records name.
        kind: SelectionKind,
        /// The reason both records name.
        reason: OmissionReason,
    },
    /// Merging two groups into one cell leaves the exact-count width.
    CountOverflow {
        /// The cell's kind.
        kind: SelectionKind,
        /// The cell's reason.
        reason: OmissionReason,
    },
}

impl fmt::Display for ManifestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RepeatedCell { kind, reason } => write!(
                f,
                "two omission records account for the `{kind}`/`{reason}` cell; the manifest \
                 partition would double-count it"
            ),
            Self::CountOverflow { kind, reason } => write!(
                f,
                "the merged `{kind}`/`{reason}` cell is beyond a 32-bit exact count"
            ),
        }
    }
}

impl core::error::Error for ManifestError {}

#[cfg(test)]
mod tests {
    use continuum_value::value::Name;

    use super::*;
    use crate::expansion::ExpansionRelation;

    /// Transcribed from `notes/plan/schemas/context-pack.schema.json`,
    /// `properties.omissions.items.properties.reason.enum`, in file order.
    const SCHEMA_REASON_ENUM: [&str; 5] = [
        "budget",
        "redaction",
        "unsupported",
        "heuristic-cutoff",
        "slice-irrelevant",
    ];

    /// Transcribed from the same schema's second `if`/`then` on an omission record: when
    /// `expandable` is false the reason is one of these two.
    const SCHEMA_IRRETRIEVABLE_REASONS: [&str; 2] = ["redaction", "unsupported"];

    fn query(anchor: &str) -> ExpansionQuery {
        ExpansionQuery::new(
            ExpansionRelation::CausalPredecessors,
            Name::new(anchor).expect("well formed"),
        )
    }

    #[test]
    fn the_reason_set_is_exactly_the_schema_enum() {
        let ours: Vec<&str> = OmissionReason::ALL
            .iter()
            .map(|reason| reason.as_wire_str())
            .collect();
        assert_eq!(ours, SCHEMA_REASON_ENUM);
    }

    #[test]
    fn an_unknown_reason_token_does_not_parse() {
        for bad in [
            "",
            "Budget",
            "budget ",
            "heuristic_cutoff",
            "sliceirrelevant",
        ] {
            assert_eq!(
                OmissionReason::from_wire_str(bad),
                None,
                "{bad:?} must be refused"
            );
        }
        for reason in OmissionReason::ALL {
            assert_eq!(
                OmissionReason::from_wire_str(reason.as_wire_str()),
                Some(reason)
            );
        }
    }

    #[test]
    fn only_two_reasons_can_spell_irretrievability() {
        let ours: Vec<&str> = IrretrievableReason::ALL
            .iter()
            .map(|reason| reason.reason().as_wire_str())
            .collect();
        assert_eq!(ours, SCHEMA_IRRETRIEVABLE_REASONS);
        // And the predicate on the wider vocabulary agrees with the type, so the two
        // spellings of the schema's conditional cannot drift apart.
        for reason in OmissionReason::ALL {
            assert_eq!(
                reason.admits_irretrievability(),
                SCHEMA_IRRETRIEVABLE_REASONS.contains(&reason.as_wire_str()),
                "{reason} must agree with the schema's conditional"
            );
        }
    }

    #[test]
    fn every_record_declares_its_retrievability() {
        // RFC 0028 F12, paid in the schema: `expandable` is required. Here it is not a
        // required *key* but a required *arm* — every constructor sets one.
        let expandable = OmissionRecord::expandable(
            SelectionKind::Event,
            3,
            OmissionReason::Budget,
            query("e_1"),
        );
        let irretrievable =
            OmissionRecord::irretrievable(SelectionKind::Source, 1, IrretrievableReason::Redaction);
        for record in [&expandable, &irretrievable] {
            let json = record.to_json();
            let object = json.as_object().expect("object");
            assert!(object.contains_key("expandable"));
        }
        assert!(expandable.retrievability().is_expandable());
        assert!(!irretrievable.retrievability().is_expandable());
    }

    #[test]
    fn an_expandable_record_carries_the_query_that_retrieves_it() {
        let record = OmissionRecord::expandable(
            SelectionKind::Event,
            196,
            OmissionReason::SliceIrrelevant,
            query("e_ack"),
        );
        let json = record.to_json();
        let object = json.as_object().expect("object");
        assert_eq!(object["expansion"], query("e_ack").to_json());

        // And the irretrievable arm omits the key entirely rather than nulling it.
        let stub =
            OmissionRecord::irretrievable(SelectionKind::Source, 2, IrretrievableReason::Redaction);
        assert!(
            !stub
                .to_json()
                .as_object()
                .expect("object")
                .contains_key("expansion")
        );
    }

    #[test]
    fn canonical_json_has_exactly_the_schema_keys_in_sorted_order() {
        let record = OmissionRecord::expandable(
            SelectionKind::Event,
            196,
            OmissionReason::SliceIrrelevant,
            query("e_ack"),
        );
        let text = String::from_utf8(record.to_json().to_canonical_bytes()).expect("utf8");
        assert_eq!(
            text,
            r#"{"count":196,"expandable":true,"expansion":{"anchor":"e_ack","relation":"causal_predecessors"},"kind":"event","reason":"slice-irrelevant"}"#
        );
    }

    #[test]
    fn records_sort_into_one_order_whatever_order_they_arrive_in() {
        let a = OmissionRecord::expandable(
            SelectionKind::Event,
            2,
            OmissionReason::Budget,
            query("e_1"),
        );
        let b =
            OmissionRecord::irretrievable(SelectionKind::Source, 1, IrretrievableReason::Redaction);
        let c = OmissionRecord::expandable(
            SelectionKind::Model,
            4,
            OmissionReason::HeuristicCutoff,
            query("m_1"),
        );
        let forward = Manifest::new([a.clone(), b.clone(), c.clone()]).expect("partitions");
        let backward = Manifest::new([c, b, a]).expect("partitions");
        assert_eq!(forward, backward);
        assert_eq!(
            forward.to_json().to_canonical_bytes(),
            backward.to_json().to_canonical_bytes()
        );
        assert_eq!(forward.total(), 7);
    }

    #[test]
    fn a_repeated_partition_cell_is_refused() {
        let one = OmissionRecord::expandable(
            SelectionKind::Event,
            2,
            OmissionReason::Budget,
            query("e_1"),
        );
        let two = OmissionRecord::expandable(
            SelectionKind::Event,
            5,
            OmissionReason::Budget,
            query("e_1"),
        );
        assert_eq!(
            Manifest::new([one, two]),
            Err(ManifestError::RepeatedCell {
                kind: SelectionKind::Event,
                reason: OmissionReason::Budget,
            })
        );
    }

    #[test]
    fn one_cell_per_query_still_admits_two_queries_of_one_kind_and_reason() {
        // The narrower key is the *cell*, not `(kind, reason)`: two groups of one kind
        // dropped for one reason but reachable by different queries are two records, and
        // their counts still sum exactly.
        let one = OmissionRecord::expandable(
            SelectionKind::Event,
            2,
            OmissionReason::Budget,
            query("e_1"),
        );
        let two = OmissionRecord::expandable(
            SelectionKind::Event,
            5,
            OmissionReason::Budget,
            query("e_2"),
        );
        let manifest = Manifest::new([one, two]).expect("two cells");
        assert_eq!(manifest.records().len(), 2);
        assert_eq!(manifest.total(), 7);
        assert_eq!(
            manifest.expansions_json().as_array().expect("array").len(),
            2
        );
    }

    #[test]
    fn merging_independent_groups_sums_one_cell_and_refuses_nothing() {
        // Two purged `source` groups are two groups and one partition cell. `new` would
        // call that a disagreement; `merged` adds them, because the groups are disjoint.
        let one =
            OmissionRecord::irretrievable(SelectionKind::Source, 2, IrretrievableReason::Redaction);
        let two =
            OmissionRecord::irretrievable(SelectionKind::Source, 3, IrretrievableReason::Redaction);
        assert!(matches!(
            Manifest::new([one.clone(), two.clone()]),
            Err(ManifestError::RepeatedCell { .. })
        ));
        let merged = Manifest::merged([one, two]).expect("one cell, summed");
        assert_eq!(merged.records().len(), 1);
        assert_eq!(merged.records()[0].count(), 5);
        assert_eq!(merged.total(), 5);
    }

    #[test]
    fn merging_leaves_distinct_cells_alone() {
        let source =
            OmissionRecord::irretrievable(SelectionKind::Source, 2, IrretrievableReason::Redaction);
        let event = OmissionRecord::expandable(
            SelectionKind::Event,
            1,
            OmissionReason::Budget,
            query("e_1"),
        );
        let merged = Manifest::merged([source, event]).expect("two cells");
        assert_eq!(merged.records().len(), 2);
        assert_eq!(merged.total(), 3);
    }

    #[test]
    fn an_empty_manifest_is_an_assertion_and_renders_as_one() {
        let manifest = Manifest::empty();
        assert!(manifest.is_empty());
        assert_eq!(manifest.total(), 0);
        assert_eq!(
            String::from_utf8(manifest.to_json().to_canonical_bytes()).expect("utf8"),
            "[]"
        );
    }
}
