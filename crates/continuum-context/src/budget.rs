//! Byte-budget packing (RFC 0028, "Budgets and packing"; the typed-outcome table's budget
//! row) — PR-11 / IMPL-06's second branch.
//!
//! # Scope: the branch bn-28jj declined
//!
//! > | the budget is exhausted before the expansion completes | error `BudgetExhausted` with
//! > a continuation. Either nothing is published, or a child pack is published whose
//! > manifest records the shortfall with reason `budget`; a child claiming completeness MUST
//! > NOT be published (INV-009, INV-017) |
//! >
//! > — RFC 0028, "Expansion protocol", typed outcomes
//!
//! IMPL-04 shipped the first branch — [`crate::pack::ChildPack::within`] and a refusal — and
//! recorded the second as this bullet's. This module is the second: given the answer an
//! expansion would return and a ceiling that answer exceeds, it packs a **smaller child**
//! whose manifest accounts for every item the ceiling cost, exactly.
//!
//! # Which branch is an error, and which is an answer
//!
//! The row above names an error and then offers two dispositions, which reads two ways. RFC
//! 0028 settles it twice elsewhere, and both sentences point the same way:
//!
//! > **Expansion is not paginated.** The operation carries no `@paginated` annotation and
//! > MUST NOT gain ad-hoc paging: an expansion too large for the budget is packed and its
//! > remainder recorded in the manifest, **which is the same mechanism as any other pack**.
//! >
//! > — RFC 0028, "Expansion protocol"
//!
//! > **Returning a raw trace when the budget is too small for a guaranteed core.** Rejected:
//! > […] The answer is **a smaller pack with a larger manifest, or `BudgetExhausted` with a
//! > continuation**.
//! >
//! > — RFC 0028, "Rejected alternatives"
//!
//! The second quote contrasts the two as alternatives, so "a smaller pack with a larger
//! manifest" is not itself a `BudgetExhausted`; the first says the mechanism is the ordinary
//! one, and the ordinary mechanism yields a pack. So: **a packed child is an answer**, and
//! `BudgetExhausted` is what is left when no conforming child fits. A caller is not left to
//! infer which happened — the shortfall is a `budget` record in the manifest and, on the
//! wire, an `Omission` with the same reason, which is RFC 0028's own transparency mechanism
//! rather than a status code.
//!
//! # The packing rule
//!
//! > **Packing is lexicographic**, in this order: (1) every item required by a claimed
//! > guarantee; (2) the omission manifest, whose completeness is reserved before any optional
//! > content; (3) highest-utility optional items in the stage-9 order.
//! >
//! > **Prefix monotonicity.** The item order MUST NOT depend on the budget. A larger budget
//! > extends the packed prefix; it never reorders it.
//! >
//! > — RFC 0028, "Budgets and packing"
//!
//! Read against an expansion child, whose `guarantees` is empty by C5/C1
//! ([`crate::pack`]), the three tiers become:
//!
//! 1. **nothing is required by a guarantee**, because the child claims none. Stage 10 here
//!    therefore cannot truncate a guaranteed core, which is the one thing "A guaranteed core
//!    is never truncated" forbids outright. A compiler that *does* claim guarantees must
//!    reserve its core before this packer runs; this module would happily drop items a
//!    guarantee needed, and there is nothing in an expansion child for it to read that from.
//! 2. **the manifest is reserved.** Every residual record survives packing, and the records
//!    this packer adds are added on top; a manifest is "never the thing a budget squeezes
//!    out (INV-007)". That is also where the refusal boundary is — see below.
//! 3. **the items are a prefix**, dropped from the tail.
//!
//! The order the prefix is taken in is [`SelectedItem`]'s own — `id`, then `kind`, then the
//! rest — because **there is no ranker in this workspace**. Stage 9 is "the lane's kill
//! surface" and is explicitly replaceable by configuration, with "plain causal slice +
//! expansion" (stages 1–8 plus this protocol, ranker disabled) as its named fallback; this
//! module runs in exactly that configuration and says so rather than inventing a utility
//! order. What the RFC demands of the *packer* is satisfied either way: the order is a
//! declared sort key (`rule ordering.deterministic`), it does not depend on the budget, and a
//! larger ceiling extends the same prefix. Landing a ranker replaces the order this packer
//! is handed; it does not change this packer.
//!
//! # The refusal boundary
//!
//! The smallest child this module can build is **the complete manifest and an empty
//! selection** — tier 2 with nothing of tier 3. If *that* exceeds the ceiling, there is no
//! conforming smaller child: shrinking further would mean either dropping manifest records
//! (INV-007: "A pack that cannot name what it dropped is malformed, not compact") or
//! shrinking a count (RFC 0028: "Counts are exact"). So the boundary is exact, and it is
//! where [`BudgetError::Exhausted`] is raised and nothing is published. It is a real
//! boundary rather than a formality: the child inherits its parent's `assurance`, `evidence`,
//! `replay` and `redactions[]` verbatim, so a pack family has a floor no budget can cut
//! below.
//!
//! # What retrieves what the ceiling dropped
//!
//! Every record this packer writes is `budget`, expandable — the schema admits nothing else
//! (`budget` omissions "are by construction expandable") — and the query it names is **the
//! question this child answers**. That is the honest answer to "how is it retrieved": the
//! items were dropped by a ceiling, not by the question, so the way to get them is to ask
//! the same question under a larger one.
//!
//! It has a consequence worth stating plainly, because a reader will otherwise think it is a
//! bug: at the default depth, the `ctx_*` that query resolves to is **the child's own**. An
//! expansion handle is derived from `(parent, relation, anchor, depth)` and budget is
//! deliberately not among them —
//!
//! > **Budget is not key material.** Under RFC 0030's budget rule, budget belongs to
//! > execution identity […] The context query definition […] is **budget-sensitive**, so its
//! > committed frontier is part of output identity.
//! >
//! > — RFC 0028, "Budgets and packing"
//!
//! — so one question under two ceilings is one `ctx_*` and two documents, distinguished by
//! their `content_hash` and ordered by their frontier. RFC 0028 requires exactly that they
//! be ordered and not conflicting ("Two compiles of one key under different budgets MUST be
//! related by this order; they MUST NOT be two conflicting answers under one identity"), and
//! they are: the packed child's selection is a subset of the fuller one's, and every manifest
//! count in the fuller one is less than or equal to the packed one's. RFC 0028's determinism
//! clause is stated per *(key, budget)* pair — "For a fixed compile key **and budget** the
//! pack MUST be byte-identical" — so two budgets yielding two encodings is the shape the RFC
//! anticipates, not a violation of it.
//!
//! Declined, and stated: recording each dropped item under the query of the *group* that
//! retrieved it, rather than under this child's own question. It would need the walk to keep
//! the item-to-group map it currently flattens, and it buys nothing — a group query retrieves
//! a superset of the dropped items too, whenever the ceiling cut through a group rather than
//! between groups.
//!
//! # Why the search is linear
//!
//! A pack's size is not monotone in the number of items it keeps: admitting the last item of
//! some kind deletes a whole omission record, which is worth more bytes than the item costs.
//! So the largest admissible prefix cannot be bisected for, and it is found by a scan.
//!
//! The scan is over *arithmetic*, not over documents. Each candidate item is encoded once
//! ([`SelectedItem::to_canonical_bytes`]), the document's fixed part is built once, and the
//! only things that move with the prefix length are a prefix sum and up to eleven exact
//! counts — so a candidate costs a constant number of additions rather than a re-encoding of
//! the pack. The chosen prefix is then **built and measured for real**, and it is that
//! measurement the ceiling admits or rejects: the arithmetic chooses, and never decides. A
//! model that was wrong would cost a step of the scan, never a published pack that overran
//! its ceiling.
//!
//! # Clause → test
//!
//! | Clause | Source | Test |
//! |---|---|---|
//! | an over-budget expansion publishes a smaller child recording the shortfall | RFC 0028's budget row, second branch | `an_over_budget_expansion_publishes_a_smaller_child` |
//! | the conservation law holds on the packed child | RFC 0028, "Counts are exact"; INV-007 | `the_conservation_law_holds_at_every_ceiling` |
//! | the prefix is maximal | this module's packing rule | `the_published_prefix_is_the_largest_one_that_fits` |
//! | a larger budget extends the prefix, never reorders it | RFC 0028, "Prefix monotonicity" | `a_larger_ceiling_extends_the_same_prefix` |
//! | two packings of one input are byte-identical | `rule ordering.deterministic` | `one_input_and_one_ceiling_are_one_document` |
//! | the manifest is never squeezed out | INV-007 | `the_residual_manifest_survives_every_ceiling` |
//! | below the minimal child, nothing is published | RFC 0028's budget row, first branch | `a_ceiling_below_the_minimal_child_publishes_nothing` |
//! | the accounting still refuses what it refused | bn-28jj's closed accounting | `the_packer_cannot_launder_a_repeated_candidate` |

use core::fmt;

use continuum_intent::canonical_json::Json;
use continuum_value::identity::ContentHasher;

use crate::accounting::{Accounting, AccountingError, CandidateSet, Omitted};
use crate::expansion::ExpansionQuery;
use crate::omission::{Manifest, ManifestError, OmissionReason, OmissionRecord};
use crate::pack::{ChildPack, MEASUREMENT_ROUNDS, PackError};
use crate::selection::{SelectedItem, SelectionKind};

/// How many members the closed selection vocabulary has — this module's array width.
const KINDS: usize = SelectionKind::ALL.len();

/// An expansion, and the byte ceiling it has to fit inside.
#[derive(Debug, Clone, Copy)]
pub struct BudgetPacker<'a> {
    /// The child this expansion would answer with if no ceiling were in force: the items it
    /// retrieved, the manifest of what it leaves outstanding regardless of budget, and the
    /// ceiling itself ([`ChildPack::ceiling`]).
    pub full: ChildPack<'a>,
    /// The query that retrieves whatever the ceiling drops — the question this child
    /// answers. See this module's "What retrieves what the ceiling dropped".
    pub shortfall: &'a ExpansionQuery,
}

impl BudgetPacker<'_> {
    /// Pack the expansion to its ceiling.
    ///
    /// Returns the child that was published, whether or not the ceiling cost it anything:
    /// [`PackedChild::dropped`] is zero when the whole answer fitted.
    ///
    /// # Errors
    ///
    /// [`BudgetError::Exhausted`] when even the minimal child — the complete manifest and an
    /// empty selection — exceeds the ceiling, which is RFC 0028's first branch and the point
    /// at which nothing is published. [`BudgetError::Pack`], [`BudgetError::Accounting`] and
    /// [`BudgetError::Manifest`] when the inputs are not a child this crate would derive at
    /// any ceiling.
    pub fn pack<H: ContentHasher>(&self) -> Result<PackedChild, BudgetError> {
        // The packing order is this module's, not the caller's: a prefix rule over an order
        // somebody else chose is a rule about somebody else's list.
        let mut ordered: Vec<SelectedItem> = self.full.selected.to_vec();
        ordered.sort();

        // The unpacked answer goes through the ledger too, keeping nothing back: a child that
        // fits is not a child that skipped its accounting, so a candidate set that cannot be
        // partitioned is refused at every ceiling rather than only at the ones that pack.
        // With nothing dropped the derived manifest is the residual one, record for record.
        let (whole_manifest, _) = self.account(&ordered, ordered.len())?;
        let whole = self.write::<H>(&ordered, &whole_manifest)?;
        if whole.measured <= self.full.ceiling {
            return Ok(whole);
        }

        let mut kept = Model::new::<H>(self, &ordered)?.largest_fit(self.full.ceiling, &ordered);
        loop {
            let (manifest, dropped) = self.account(&ordered, kept)?;
            let mut packed = self.write::<H>(&ordered[..kept], &manifest)?;
            packed.dropped = dropped;
            if packed.measured <= self.full.ceiling {
                return Ok(packed);
            }
            if kept == 0 {
                return Err(BudgetError::Exhausted {
                    minimal: packed.measured,
                    ceiling: self.full.ceiling,
                });
            }
            kept -= 1;
        }
    }

    /// Write one candidate child and measure it.
    fn write<H: ContentHasher>(
        &self,
        selected: &[SelectedItem],
        manifest: &Manifest,
    ) -> Result<PackedChild, BudgetError> {
        let child = ChildPack {
            selected,
            manifest,
            ..self.full
        };
        let document = child.to_json::<H>().map_err(BudgetError::Pack)?;
        // The measurement is the document's own, and `to_json` has already written it into
        // `content_budget.bytes`; reading the length back is the same number by construction
        // (`crate::pack`'s fixed-point section).
        let measured = document.to_canonical_bytes().len() as u64;
        Ok(PackedChild {
            document,
            manifest: manifest.clone(),
            kept: selected.len(),
            dropped: 0,
            measured,
        })
    }

    /// The manifest of a child that keeps the first `kept` candidates, and how many the
    /// ceiling dropped.
    ///
    /// Read off one ledger, in bn-28jj's discipline: every candidate is dispositioned exactly
    /// once, the records are *derived* from the dispositions rather than supplied beside
    /// them, and [`crate::accounting::ClosedAccounting::reconcile`] recomputes the candidate
    /// count from the two published halves before anything is written. The conservation law
    /// is therefore a property of the only constructor there is, not arithmetic this module
    /// could get wrong.
    fn account(
        &self,
        ordered: &[SelectedItem],
        kept: usize,
    ) -> Result<(Manifest, u32), BudgetError> {
        let candidates =
            CandidateSet::new(ordered.iter().map(|item| (item.id().clone(), item.kind())))
                .map_err(BudgetError::Accounting)?;
        let mut accounting = Accounting::over(candidates);
        for (index, item) in ordered.iter().enumerate() {
            let outcome = if index < kept {
                accounting.select(item.id())
            } else {
                accounting.omit(
                    item.id(),
                    Omitted::Expandable {
                        reason: OmissionReason::Budget,
                        query: self.shortfall.clone(),
                    },
                )
            };
            outcome.map_err(BudgetError::Accounting)?;
        }
        let closed = accounting.close().map_err(BudgetError::Accounting)?;
        closed.reconcile().map_err(BudgetError::Accounting)?;

        // `merged`, not `new`: a residual record and a shortfall record can land in one cell
        // — the same kind, dropped for budget, retrieved by the same query — and they came
        // from disjoint groups, so their counts add rather than disagreeing.
        let manifest = Manifest::merged(
            self.full
                .manifest
                .records()
                .iter()
                .cloned()
                .chain(closed.manifest().records().iter().cloned()),
        )
        .map_err(BudgetError::Manifest)?;
        let dropped = u32::try_from(ordered.len() - kept).unwrap_or(u32::MAX);
        Ok((manifest, dropped))
    }
}

/// The child a [`BudgetPacker`] published, and what the ceiling cost it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackedChild {
    document: Json,
    manifest: Manifest,
    kept: usize,
    dropped: u32,
    measured: u64,
}

impl PackedChild {
    /// The published document.
    #[must_use]
    pub const fn document(&self) -> &Json {
        &self.document
    }

    /// The manifest the document carries — the residual records plus this packer's own.
    #[must_use]
    pub const fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    /// How many candidates the child selects.
    #[must_use]
    pub const fn kept(&self) -> usize {
        self.kept
    }

    /// How many candidates the ceiling dropped. Zero when the whole answer fitted.
    #[must_use]
    pub const fn dropped(&self) -> u32 {
        self.dropped
    }

    /// The child's own measured byte size — the value at `content_budget.bytes`, and the
    /// number the ceiling admitted (RFC 0028 correction 17).
    #[must_use]
    pub const fn measured(&self) -> u64 {
        self.measured
    }

    /// Whether the ceiling cost this child anything.
    #[must_use]
    pub const fn is_packed(&self) -> bool {
        self.dropped > 0
    }
}

/// A ceiling this packer cannot pack to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BudgetError {
    /// Even the minimal child — the complete manifest, an empty selection — is larger than
    /// the ceiling. RFC 0028's first branch: nothing is published.
    Exhausted {
        /// What the minimal child measured.
        minimal: u64,
        /// The ceiling it had to fit inside.
        ceiling: u64,
    },
    /// The parent is not a document a child can be derived from at any ceiling.
    Pack(PackError),
    /// The candidates cannot be accounted for: a repeated identity, or a count beyond an
    /// exact one.
    Accounting(AccountingError),
    /// The residual records and this packer's own do not merge into one partition.
    Manifest(ManifestError),
}

impl fmt::Display for BudgetError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Exhausted { minimal, ceiling } => write!(
                f,
                "the smallest conforming child measures {minimal} bytes against a ceiling of \
                 {ceiling}; a manifest is never what a budget squeezes out (INV-007)"
            ),
            Self::Pack(error) => write!(f, "{error}"),
            Self::Accounting(error) => write!(f, "{error}"),
            Self::Manifest(error) => write!(f, "{error}"),
        }
    }
}

impl core::error::Error for BudgetError {}

/// The arithmetic the prefix search runs over: the parts of a child's canonical encoding
/// that move with the prefix length, and the one constant that does not.
///
/// See this module's "Why the search is linear". Nothing here decides what is published —
/// [`BudgetPacker::pack`] builds and measures the prefix this picks.
struct Model {
    /// The document's length less its three arrays and the digits of its own measurement.
    rest: u64,
    /// Prefix sums of the candidates' canonical lengths: `prefix[n]` is the first `n`.
    prefix: Vec<u64>,
    /// Residual records that share no cell with a shortfall record: their total canonical
    /// length, and how many there are.
    plain_records: (u64, u64),
    /// The same, for the `expansions[]` entries those records make reachable.
    plain_queries: (u64, u64),
    /// Residual counts already sitting in a kind's shortfall cell, which a dropped candidate
    /// of that kind adds to rather than opening a second record beside.
    resident: [u64; KINDS],
    /// A shortfall record's canonical length for each kind, less the digits of its count.
    record_base: [u64; KINDS],
    /// The canonical length of one `expansions[]` entry for the shortfall query.
    query_len: u64,
}

impl Model {
    fn new<H: ContentHasher>(
        packer: &BudgetPacker<'_>,
        ordered: &[SelectedItem],
    ) -> Result<Self, BudgetError> {
        // The fixed part, measured rather than modelled: a child with three empty arrays and
        // a single-digit measurement. Subtracting those seven bytes leaves everything the
        // prefix length cannot move.
        let empty = Manifest::empty();
        let probe = ChildPack {
            selected: &[],
            manifest: &empty,
            ..packer.full
        }
        .document_with::<H>(0)
        .map_err(BudgetError::Pack)?;
        let rest = probe.to_canonical_bytes().len() as u64 - 7;

        let mut prefix = Vec::with_capacity(ordered.len() + 1);
        let mut running = 0;
        prefix.push(running);
        for item in ordered {
            running += item.to_canonical_bytes().len() as u64;
            prefix.push(running);
        }

        let mut plain_records = (0, 0);
        let mut plain_queries = (0, 0);
        let mut resident = [0; KINDS];
        for record in packer.full.manifest.records() {
            let shortfall_cell = record.reason() == OmissionReason::Budget
                && record.retrievability().query() == Some(packer.shortfall);
            if shortfall_cell {
                resident[slot(record.kind())] += u64::from(record.count());
                continue;
            }
            plain_records.0 += record.to_json().to_canonical_bytes().len() as u64;
            plain_records.1 += 1;
            if let Some(query) = record.retrievability().query() {
                plain_queries.0 += query.to_json().to_canonical_bytes().len() as u64;
                plain_queries.1 += 1;
            }
        }

        let mut record_base = [0; KINDS];
        for kind in SelectionKind::ALL {
            let record = OmissionRecord::expandable(
                kind,
                0,
                OmissionReason::Budget,
                packer.shortfall.clone(),
            );
            // `digits(0)` is one, and a real count's digits are added back per candidate.
            record_base[slot(kind)] = record.to_json().to_canonical_bytes().len() as u64 - 1;
        }

        Ok(Self {
            rest,
            prefix,
            plain_records,
            plain_queries,
            resident,
            record_base,
            query_len: packer.shortfall.to_json().to_canonical_bytes().len() as u64,
        })
    }

    /// The largest prefix whose modelled size fits `ceiling`, scanning down from "everything
    /// but the last candidate".
    ///
    /// Down, rather than up or by bisection, because size is not monotone in the prefix
    /// length: admitting the last candidate of some kind deletes an omission record worth
    /// more bytes than the candidate costs. Zero when nothing is modelled to fit — the caller
    /// then measures the minimal child for real and refuses with its true size.
    fn largest_fit(&self, ceiling: u64, ordered: &[SelectedItem]) -> usize {
        let mut dropped = [0; KINDS];
        let mut kept = ordered.len();
        while kept > 0 {
            kept -= 1;
            dropped[slot(ordered[kept].kind())] += 1;
            if self.length(kept, &dropped) <= ceiling {
                return kept;
            }
        }
        0
    }

    /// The canonical length a child keeping `kept` candidates and dropping `dropped` of each
    /// kind would measure.
    fn length(&self, kept: usize, dropped: &[u64; KINDS]) -> u64 {
        let selected = if kept == 0 {
            2
        } else {
            self.prefix[kept] + kept as u64 + 1
        };
        let mut records = self.plain_records;
        let mut queries = self.plain_queries;
        for ((dropped, resident), base) in dropped.iter().zip(&self.resident).zip(&self.record_base)
        {
            let count = resident + dropped;
            if count == 0 {
                continue;
            }
            records.0 += base + digits(count);
            records.1 += 1;
            queries.0 += self.query_len;
            queries.1 += 1;
        }
        settle(self.rest + selected + array_len(records) + array_len(queries))
    }
}

/// The canonical length of a JSON array from its elements' total length and their number.
const fn array_len(elements: (u64, u64)) -> u64 {
    let (total, count) = elements;
    if count == 0 {
        // `[]`
        2
    } else {
        // `[` + elements + one separator between each pair + `]`
        total + count + 1
    }
}

/// The least self-consistent measurement of a document whose other bytes number `base`.
///
/// The same fixed point [`ChildPack::to_json`] iterates for real, in arithmetic: see
/// `crate::pack`'s "`content_budget.bytes` is a fixed point, and which one".
fn settle(base: u64) -> u64 {
    let mut measured = 0;
    for _ in 0..MEASUREMENT_ROUNDS {
        let next = base + digits(measured);
        if next == measured {
            break;
        }
        measured = next;
    }
    measured
}

/// How many decimal digits a count is written with — `0` is written `0`, so one.
const fn digits(value: u64) -> u64 {
    let mut digits = 1;
    let mut remaining = value;
    while remaining >= 10 {
        remaining /= 10;
        digits += 1;
    }
    digits
}

/// A kind's position in the closed vocabulary, which is this module's array index.
fn slot(kind: SelectionKind) -> usize {
    SelectionKind::ALL
        .iter()
        .position(|member| *member == kind)
        .expect("`SelectionKind::ALL` declares every member")
}

#[cfg(test)]
mod tests {
    use continuum_value::identity::Blake3Hasher;
    use continuum_value::value::Name;
    use continuum_workspace::snapshot::WorkspacePath;

    use super::*;
    use crate::expansion::{Depth, ExpansionHandle, ExpansionQuestion, ExpansionRelation};
    use crate::pack::{self, budget_bytes_of, identity_of};
    use crate::source::{SourceRef, SourceSpan};

    /// The same minimal conforming parent `crate::pack`'s tests use, as a document.
    const PARENT: &str = r#"{
      "assurance": {"class": "bounded", "envelope": {}},
      "content_budget": {"bytes": 16384},
      "content_hash": "blake3-256:aaaa",
      "context_id": "ctx_parent1",
      "evidence": ["ev_1"],
      "expansions": [{"anchor": "e_ack", "relation": "source_span"}],
      "guarantees": ["ReplayPreserving"],
      "intent": "in_ack_v1",
      "omissions": [{"count": 2, "expandable": true,
                     "expansion": {"anchor": "e_ack", "relation": "source_span"},
                     "kind": "source", "reason": "budget"}],
      "parent": null,
      "question": "why did AckImpliesDurable fail?",
      "replay": "crash_1",
      "schema_epoch": 1,
      "schema_id": "https://continuum.dev/schema/context-pack.json",
      "selected": [{"artifact": null, "id": "e_ack", "kind": "event", "summary": "s"}],
      "semantic_epoch": "sem3-r3-demo",
      "snapshot": "ws_demo1",
      "verdict": "refuted"
    }"#;

    fn parent() -> Json {
        Json::parse(PARENT.as_bytes()).expect("the fixture is admissible JSON")
    }

    fn name(text: &str) -> Name {
        Name::new(text).expect("well formed")
    }

    fn query() -> ExpansionQuery {
        ExpansionQuery::new(ExpansionRelation::SourceSpan, name("e_ack"))
    }

    /// `count` distinct `source` items, built in one linear pass — no textual doubling, so a
    /// large fixture costs what it says it costs.
    fn items(count: u32) -> Vec<SelectedItem> {
        (0..count)
            .map(|index| {
                let line = index + 1;
                let span = SourceSpan::new(
                    WorkspacePath::new("src/ack.rs").expect("a repo-relative path"),
                    line,
                    1,
                    line,
                    40,
                )
                .expect("a well-formed span");
                SourceRef::new(span).into_selected_item(name(&format!("s_{index:04}")))
            })
            .collect()
    }

    /// A residual record: one group this expansion did not reach, outstanding at every
    /// ceiling.
    fn residual() -> Manifest {
        Manifest::new([OmissionRecord::expandable(
            SelectionKind::Event,
            7,
            OmissionReason::SliceIrrelevant,
            ExpansionQuery::new(ExpansionRelation::CausalPredecessors, name("e_ack")),
        )])
        .expect("one cell")
    }

    /// Run the packer over `items` at `ceiling`.
    fn pack_at(
        parent: &Json,
        selected: &[SelectedItem],
        manifest: &Manifest,
        ceiling: u64,
    ) -> Result<PackedChild, BudgetError> {
        let identity = ExpansionHandle::derive::<Blake3Hasher>(
            &identity_of(parent).expect("a pack handle"),
            &query(),
            Depth::DEFAULT,
        )
        .expect("derives");
        let question = ExpansionQuestion::of(&query(), Depth::DEFAULT);
        let anchor = query();
        BudgetPacker {
            full: ChildPack {
                parent,
                identity: &identity,
                question: &question,
                selected,
                manifest,
                ceiling,
            },
            shortfall: &anchor,
        }
        .pack::<Blake3Hasher>()
    }

    /// `|selected| + Σ manifest counts`, read off a published document alone.
    fn accounted(document: &Json) -> u64 {
        let fields = document.as_object().expect("object");
        let selected = fields["selected"].as_array().expect("array").len() as u64;
        let omitted: i64 = fields["omissions"]
            .as_array()
            .expect("array")
            .iter()
            .map(|record| {
                record.as_object().expect("object")["count"]
                    .as_integer()
                    .expect("an exact count")
            })
            .sum();
        selected + omitted as u64
    }

    /// The measurement of the child that keeps everything — the packer's starting point.
    fn unpacked(parent: &Json, selected: &[SelectedItem], manifest: &Manifest) -> u64 {
        pack_at(parent, selected, manifest, u64::MAX)
            .expect("no ceiling refuses nothing")
            .measured()
    }

    /// What the child that keeps exactly `kept` of `selected` would measure — the packer's
    /// own two private steps, run without its search, so maximality can be asserted against
    /// the real thing rather than against a bound.
    fn measure_prefix(
        parent: &Json,
        selected: &[SelectedItem],
        manifest: &Manifest,
        kept: usize,
    ) -> u64 {
        let identity = ExpansionHandle::derive::<Blake3Hasher>(
            &identity_of(parent).expect("a pack handle"),
            &query(),
            Depth::DEFAULT,
        )
        .expect("derives");
        let question = ExpansionQuestion::of(&query(), Depth::DEFAULT);
        let anchor = query();
        let packer = BudgetPacker {
            full: ChildPack {
                parent,
                identity: &identity,
                question: &question,
                selected,
                manifest,
                ceiling: u64::MAX,
            },
            shortfall: &anchor,
        };
        let mut ordered = selected.to_vec();
        ordered.sort();
        let (packed, _) = packer.account(&ordered, kept).expect("accounts");
        packer
            .write::<Blake3Hasher>(&ordered[..kept], &packed)
            .expect("writes")
            .measured()
    }

    #[test]
    fn an_over_budget_expansion_publishes_a_smaller_child() {
        let parent = parent();
        let items = items(12);
        let residual = residual();
        let whole = unpacked(&parent, &items, &residual);

        let packed = pack_at(&parent, &items, &residual, whole - 200).expect("a smaller child");
        assert!(packed.is_packed());
        assert!(packed.kept() < items.len());
        assert_eq!(packed.dropped() as usize, items.len() - packed.kept());
        assert!(packed.measured() <= whole - 200);

        // The shortfall is in the manifest, under `budget`, expandable, naming the query
        // that retrieves it — never a silent truncation.
        let shortfall = packed
            .manifest()
            .records()
            .iter()
            .find(|record| record.reason() == OmissionReason::Budget)
            .expect("the ceiling's own record");
        assert_eq!(shortfall.kind(), SelectionKind::Source);
        assert_eq!(shortfall.count(), packed.dropped());
        assert!(shortfall.retrievability().is_expandable());
        assert_eq!(shortfall.retrievability().query(), Some(&query()));

        // And it is a pack: every required key, and a child that claims no completeness.
        pack::required_keys_present(packed.document()).expect("a conforming child");
        assert!(!packed.manifest().is_empty());
    }

    #[test]
    fn the_conservation_law_holds_at_every_ceiling() {
        // `|selected| + Σ counts` is the candidate set plus the residual group, at every
        // ceiling that publishes anything: packing moves an item from one half to the other
        // and creates or destroys nothing.
        let parent = parent();
        let items = items(12);
        let residual = residual();
        let whole = unpacked(&parent, &items, &residual);
        let expected = items.len() as u64 + residual.total();

        let mut seen = 0;
        for ceiling in (whole - 900..=whole).step_by(25) {
            let Ok(packed) = pack_at(&parent, &items, &residual, ceiling) else {
                continue;
            };
            seen += 1;
            assert_eq!(
                accounted(packed.document()),
                expected,
                "a ceiling of {ceiling} accounts for the wrong number of candidates"
            );
            assert_eq!(packed.kept() as u64 + u64::from(packed.dropped()), 12);
            assert!(packed.measured() <= ceiling);
        }
        assert!(seen > 4, "the sweep must actually publish packs: {seen}");
    }

    #[test]
    fn the_published_prefix_is_the_largest_one_that_fits() {
        // Maximality, which is also the check that the arithmetic in `Model` agrees with the
        // encoder: keeping one more candidate must not fit.
        let parent = parent();
        let items = items(12);
        let residual = residual();
        let whole = unpacked(&parent, &items, &residual);

        for ceiling in (whole - 700..whole).step_by(37) {
            let packed = pack_at(&parent, &items, &residual, ceiling).expect("a smaller child");
            assert!(packed.measured() <= ceiling);
            if packed.kept() == items.len() {
                continue;
            }
            assert!(
                measure_prefix(&parent, &items, &residual, packed.kept() + 1) > ceiling,
                "a ceiling of {ceiling} kept {} candidates and {} would have fitted",
                packed.kept(),
                packed.kept() + 1
            );
        }
    }

    #[test]
    fn a_larger_ceiling_extends_the_same_prefix() {
        // RFC 0028's prefix monotonicity, and the frontier order it exists for: the smaller
        // pack's selection is a subset of the larger's, and every manifest count in the
        // larger is less than or equal to the smaller's.
        let parent = parent();
        let items = items(12);
        let residual = residual();
        let whole = unpacked(&parent, &items, &residual);

        let mut previous: Option<PackedChild> = None;
        for ceiling in (whole - 800..=whole).step_by(50) {
            let Ok(packed) = pack_at(&parent, &items, &residual, ceiling) else {
                continue;
            };
            if let Some(smaller) = &previous {
                assert!(
                    smaller.kept() <= packed.kept(),
                    "a larger ceiling kept less"
                );
                let smaller_ids = selected_ids(smaller.document());
                let larger_ids = selected_ids(packed.document());
                assert_eq!(
                    smaller_ids.as_slice(),
                    &larger_ids[..smaller_ids.len()],
                    "a larger ceiling reordered the prefix"
                );
                assert!(
                    smaller.manifest().total() >= packed.manifest().total(),
                    "a larger ceiling omitted more"
                );
            }
            previous = Some(packed);
        }
        let last = previous.expect("the sweep published something");
        assert_eq!(last.kept(), items.len(), "the widest ceiling keeps it all");
    }

    fn selected_ids(document: &Json) -> Vec<String> {
        document.as_object().expect("object")["selected"]
            .as_array()
            .expect("array")
            .iter()
            .map(|item| {
                item.as_object().expect("object")["id"]
                    .as_str()
                    .expect("string")
                    .to_owned()
            })
            .collect()
    }

    #[test]
    fn one_input_and_one_ceiling_are_one_document() {
        let parent = parent();
        let items = items(9);
        let residual = residual();
        let whole = unpacked(&parent, &items, &residual);
        let ceiling = whole - 300;

        let first = pack_at(&parent, &items, &residual, ceiling).expect("a smaller child");
        let second = pack_at(&parent, &items, &residual, ceiling).expect("a smaller child");
        assert_eq!(
            first.document().to_canonical_bytes(),
            second.document().to_canonical_bytes()
        );
        // Not vacuous: the packing order is this module's, so an input shuffled by the caller
        // still packs to the same bytes, and a different ceiling does not.
        let mut shuffled = items.clone();
        shuffled.reverse();
        let third = pack_at(&parent, &shuffled, &residual, ceiling).expect("a smaller child");
        assert_eq!(
            first.document().to_canonical_bytes(),
            third.document().to_canonical_bytes()
        );
        let wider = pack_at(&parent, &items, &residual, whole).expect("everything fits");
        assert_ne!(
            first.document().to_canonical_bytes(),
            wider.document().to_canonical_bytes()
        );
    }

    #[test]
    fn the_residual_manifest_survives_every_ceiling() {
        // INV-007: the manifest is reserved before optional content and is never what a
        // budget squeezes out. The residual group is outstanding in every published child,
        // with its count untouched.
        let parent = parent();
        let items = items(12);
        let residual = residual();
        let whole = unpacked(&parent, &items, &residual);

        for ceiling in (whole - 900..=whole).step_by(30) {
            let Ok(packed) = pack_at(&parent, &items, &residual, ceiling) else {
                continue;
            };
            let carried = packed
                .manifest()
                .records()
                .iter()
                .find(|record| record.reason() == OmissionReason::SliceIrrelevant)
                .expect("the residual record is never dropped");
            assert_eq!(carried.count(), 7);
            assert_eq!(carried.kind(), SelectionKind::Event);
        }
    }

    #[test]
    fn a_ceiling_below_the_minimal_child_publishes_nothing() {
        // The boundary, exactly: the minimal child is the complete manifest and an empty
        // selection. At its own size it is published; one byte below it, nothing is.
        let parent = parent();
        let items = items(12);
        let residual = residual();

        let minimal = match pack_at(&parent, &items, &residual, 0) {
            Err(BudgetError::Exhausted { minimal, ceiling }) => {
                assert_eq!(ceiling, 0);
                minimal
            }
            other => panic!("a ceiling of zero publishes nothing: {other:?}"),
        };

        let at_the_boundary = pack_at(&parent, &items, &residual, minimal).expect("exactly fits");
        assert_eq!(at_the_boundary.kept(), 0);
        assert_eq!(at_the_boundary.dropped(), 12);
        assert_eq!(at_the_boundary.measured(), minimal);
        assert_eq!(accounted(at_the_boundary.document()), 19);

        assert_eq!(
            pack_at(&parent, &items, &residual, minimal - 1),
            Err(BudgetError::Exhausted {
                minimal,
                ceiling: minimal - 1,
            }),
            "one byte below the minimal child, nothing is published"
        );
    }

    #[test]
    fn the_packer_cannot_launder_a_repeated_candidate() {
        // Anti-vacuity for the accounting inside the packer: the refusals bn-28jj built are
        // still the refusals, and a packed child cannot be the thing that slips one past.
        let parent = parent();
        let mut items = items(4);
        items.push(items[0].clone());
        let residual = residual();
        let whole = unpacked(&parent, &items[..4], &residual);
        // At a ceiling that packs *and* at one that does not: every published child comes off
        // the ledger, so neither branch is a way around it.
        for ceiling in [whole - 100, u64::MAX] {
            assert!(matches!(
                pack_at(&parent, &items, &residual, ceiling),
                Err(BudgetError::Accounting(
                    AccountingError::RepeatedCandidate { .. }
                ))
            ));
        }
    }

    #[test]
    fn a_packed_child_records_its_own_size_and_no_token_count() {
        let parent = parent();
        let items = items(12);
        let residual = residual();
        let whole = unpacked(&parent, &items, &residual);
        let packed = pack_at(&parent, &items, &residual, whole - 400).expect("a smaller child");

        let recorded = budget_bytes_of(packed.document()).expect("a measured child");
        assert_eq!(recorded, packed.measured());
        assert_eq!(
            recorded,
            packed.document().to_canonical_bytes().len() as u64
        );
        assert!(recorded <= whole - 400);
        assert_eq!(
            packed.document().as_object().expect("object")["content_budget"]
                .as_object()
                .expect("object")
                .keys()
                .map(String::as_str)
                .collect::<Vec<_>>(),
            ["bytes"]
        );
    }

    #[test]
    fn a_wide_expansion_packs_without_re_encoding_the_pack_per_candidate() {
        // The adversarial size, built in one linear pass. Two thousand candidates is beyond
        // what a quadratic search would finish comfortably, and the assertions are the same
        // ones the small fixture makes.
        let parent = parent();
        let items = items(2_000);
        let residual = Manifest::empty();
        let whole = unpacked(&parent, &items, &residual);

        let packed = pack_at(&parent, &items, &residual, whole / 2).expect("a smaller child");
        assert!(packed.kept() > 500, "kept {}", packed.kept());
        assert!(packed.kept() < 2_000);
        assert_eq!(packed.kept() as u64 + u64::from(packed.dropped()), 2_000);
        assert_eq!(accounted(packed.document()), 2_000);
        assert!(packed.measured() <= whole / 2);
        assert_eq!(selected_ids(packed.document())[0], "s_0000");
    }

    #[test]
    fn a_residual_budget_record_of_one_kind_merges_with_the_shortfall() {
        // The cell collision the manifest's `merged` exists for: a residual group of the same
        // kind, dropped for budget, retrieved by the same query, is one record whose count is
        // the sum — never two rows for one cell, which would double-count it.
        let parent = parent();
        let items = items(8);
        let residual = Manifest::new([OmissionRecord::expandable(
            SelectionKind::Source,
            5,
            OmissionReason::Budget,
            query(),
        )])
        .expect("one cell");
        let whole = unpacked(&parent, &items, &residual);
        let packed = pack_at(&parent, &items, &residual, whole - 250).expect("a smaller child");

        let records: Vec<_> = packed
            .manifest()
            .records()
            .iter()
            .filter(|record| record.reason() == OmissionReason::Budget)
            .collect();
        assert_eq!(records.len(), 1, "one cell, one record");
        assert_eq!(records[0].count(), 5 + packed.dropped());
        assert_eq!(accounted(packed.document()), 13);
    }
}
