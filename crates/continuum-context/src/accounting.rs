//! Closed accounting: the discipline that makes a pack plus its manifest account for the
//! *whole* candidate set, so nothing can be dropped silently (INV-007).
//!
//! # The equation, and why it is a type rather than a check
//!
//! > For a compile over a candidate set with a selection and a manifest, the size of the
//! > candidate set equals the size of the selection plus the sum of the manifest counts,
//! > and the partition of the unselected candidates by kind and reason is exact. A count
//! > MUST NOT be an estimate, a sample, or a bucket.
//! >
//! > — RFC 0028, "Omission manifest"
//!
//! A compiler could satisfy that sentence by computing a selection, computing a manifest,
//! and then checking the arithmetic — and would then have three places to be wrong and one
//! place to notice. Here there is one: [`Accounting`] starts from the candidate set and
//! records a *disposition* for each candidate, [`Accounting::close`] refuses to produce a
//! [`ClosedAccounting`] while any candidate has none, and the manifest is **derived** from
//! the dispositions rather than supplied beside them. The equation is then not a property
//! the caller maintains; it is a property of the only constructor there is.
//!
//! This is the discipline `continuum-task`'s budget `MeterSet` set for spend — one ledger
//! that both halves are read off, rather than two ledgers and a reconciliation — applied to
//! the thing INV-007 is about.
//!
//! # Why there is a `reconcile` at all, if the equation holds by construction
//!
//! Because "holds by construction" is a claim about *this* code, and RFC 0028's Validation
//! section asks for "manifest reconciliation against the unsliced candidate set" as an
//! independent check. [`ClosedAccounting::reconcile`] recomputes the candidate count from
//! the two published halves — the selection's length and the manifest's counts — and
//! compares it against the size the candidate set had *before any disposition was
//! recorded*. The two numbers come from different places: one from the input, one from the
//! output. Agreement between them is evidence; a single derivation checked against itself
//! would be a tautology, which is the failure `EvidenceBook::reconcile` documents avoiding
//! for the same reason.
//!
//! # Anti-vacuity
//!
//! The tests below do not only assert that a correct accounting closes. Each of the three
//! ways an item can go missing — never dispositioned, dispositioned twice, or counted into
//! a manifest whose record was then shrunk — is exercised as a *mutant* that must be
//! refused. An accounting discipline nothing can fail is not a discipline.
//!
//! # Clause → test
//!
//! | Clause | Source | Test |
//! |---|---|---|
//! | candidate set = selection + Σ counts | RFC 0028, "Omission manifest" | `the_counting_equation_holds_for_every_closed_accounting` |
//! | nothing is dropped silently | INV-007 | `an_undispositioned_candidate_cannot_be_closed` |
//! | one candidate, one disposition | RFC 0028, "the partition […] is exact" | `a_candidate_cannot_be_dispositioned_twice` |
//! | an empty manifest asserts completeness | RFC 0028, "Omission manifest" | `an_empty_manifest_means_the_selection_is_the_whole_candidate_set` |
//! | a shrunk count is caught | RFC 0028's guarantee-violation mutants | `a_shrunk_manifest_count_fails_reconciliation` |
//! | the reconciliation is not vacuous | this module's own discipline | `a_shrunk_manifest_count_fails_reconciliation`, `an_inflated_manifest_count_fails_reconciliation` |

use core::fmt;
use std::collections::BTreeMap;

use continuum_value::value::Name;

use crate::expansion::ExpansionQuery;
use crate::omission::{
    IrretrievableReason, Manifest, ManifestError, OmissionReason, OmissionRecord,
};
use crate::selection::SelectionKind;

/// Everything a compile considered: the unsliced candidate set, each candidate named and
/// kinded.
///
/// > A compiler that cannot compute the candidate set MUST NOT emit an empty manifest; it
/// > emits an `unsupported` record instead.
/// >
/// > — RFC 0028, "Omission manifest"
///
/// Holding the set as a value rather than a count is what makes that sentence enforceable:
/// a compiler that cannot enumerate its candidates cannot build this type, and therefore
/// cannot reach [`ClosedAccounting`] at all.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CandidateSet {
    members: BTreeMap<Name, SelectionKind>,
}

impl CandidateSet {
    /// Build a candidate set.
    ///
    /// # Errors
    ///
    /// [`AccountingError::RepeatedCandidate`] when one identity appears twice. Two entries
    /// for one candidate would make the set's own size disagree with the number of things
    /// there are to disposition, which is the first number the equation rests on.
    pub fn new(
        candidates: impl IntoIterator<Item = (Name, SelectionKind)>,
    ) -> Result<Self, AccountingError> {
        let mut members = BTreeMap::new();
        for (id, kind) in candidates {
            if members.insert(id.clone(), kind).is_some() {
                return Err(AccountingError::RepeatedCandidate { id });
            }
        }
        Ok(Self { members })
    }

    /// How many candidates there are.
    #[must_use]
    pub fn len(&self) -> usize {
        self.members.len()
    }

    /// Whether the set is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.members.is_empty()
    }

    /// The kind of one candidate.
    #[must_use]
    pub fn kind(&self, id: &Name) -> Option<SelectionKind> {
        self.members.get(id).copied()
    }
}

/// Why a candidate was not selected, and how it is retrieved.
///
/// The two arms carry exactly what the schema's two conditionals allow, so an omission's
/// reason and its retrievability cannot be set independently and cannot disagree — see
/// [`crate::omission::Retrievability`].
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum Omitted {
    /// Dropped for `reason`, and `query` retrieves it.
    Expandable {
        /// Any of the five reasons.
        reason: OmissionReason,
        /// The query that retrieves the group this candidate falls in.
        query: ExpansionQuery,
    },
    /// Gone, for one of the two reasons that explain irretrievability.
    Irretrievable(IrretrievableReason),
}

impl Omitted {
    /// The manifest reason this omission is recorded under.
    #[must_use]
    pub const fn reason(&self) -> OmissionReason {
        match self {
            Self::Expandable { reason, .. } => *reason,
            Self::Irretrievable(reason) => reason.reason(),
        }
    }
}

/// What happened to one candidate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Disposition {
    /// It is in the pack.
    Selected,
    /// It is in the manifest.
    Omitted(Omitted),
}

/// A compile in progress: the candidate set, and a disposition for each candidate decided
/// so far.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Accounting {
    candidates: CandidateSet,
    dispositions: BTreeMap<Name, Disposition>,
}

impl Accounting {
    /// Begin accounting over a candidate set, with nothing decided.
    #[must_use]
    pub fn over(candidates: CandidateSet) -> Self {
        Self {
            candidates,
            dispositions: BTreeMap::new(),
        }
    }

    /// Put a candidate in the pack.
    ///
    /// # Errors
    ///
    /// [`AccountingError::UnknownCandidate`] for an identity the candidate set does not
    /// contain — a selection is a *sub*-set, and an item from nowhere would make the
    /// equation false in the direction that looks like generosity; and
    /// [`AccountingError::AlreadyDispositioned`] for a second decision about one candidate.
    pub fn select(&mut self, id: &Name) -> Result<(), AccountingError> {
        self.disposition(id, Disposition::Selected)
    }

    /// Put a candidate in the manifest.
    ///
    /// # Errors
    ///
    /// As [`Accounting::select`].
    pub fn omit(&mut self, id: &Name, omitted: Omitted) -> Result<(), AccountingError> {
        self.disposition(id, Disposition::Omitted(omitted))
    }

    fn disposition(&mut self, id: &Name, disposition: Disposition) -> Result<(), AccountingError> {
        if self.candidates.kind(id).is_none() {
            return Err(AccountingError::UnknownCandidate { id: id.clone() });
        }
        if self.dispositions.contains_key(id) {
            return Err(AccountingError::AlreadyDispositioned { id: id.clone() });
        }
        self.dispositions.insert(id.clone(), disposition);
        Ok(())
    }

    /// The candidates no decision has been recorded for.
    #[must_use]
    pub fn undecided(&self) -> Vec<Name> {
        self.candidates
            .members
            .keys()
            .filter(|id| !self.dispositions.contains_key(*id))
            .cloned()
            .collect()
    }

    /// Close the accounting: every candidate decided, the selection and the manifest read
    /// off the one ledger.
    ///
    /// # Errors
    ///
    /// [`AccountingError::Undispositioned`] naming the first candidate with no decision —
    /// this is INV-007's "everything dropped is named", enforced as the refusal to publish
    /// at all rather than as a warning beside a pack that already dropped it; and
    /// [`AccountingError::Manifest`] if the derived records do not partition, which
    /// [`Manifest::new`] decides.
    pub fn close(self) -> Result<ClosedAccounting, AccountingError> {
        let mut undecided = self.undecided();
        if !undecided.is_empty() {
            undecided.sort();
            let remaining = undecided.len();
            return Err(AccountingError::Undispositioned {
                first: undecided.swap_remove(0),
                remaining,
            });
        }

        let mut selected = Vec::new();
        let mut cells: BTreeMap<(SelectionKind, Omitted), u32> = BTreeMap::new();
        for (id, disposition) in &self.dispositions {
            let kind = self
                .candidates
                .kind(id)
                .expect("every disposition names a candidate of the set");
            match disposition {
                Disposition::Selected => selected.push(id.clone()),
                Disposition::Omitted(omitted) => {
                    let cell = cells.entry((kind, omitted.clone())).or_insert(0);
                    *cell = cell
                        .checked_add(1)
                        .ok_or(AccountingError::CountOverflow { kind })?;
                }
            }
        }

        let manifest = Manifest::new(cells.into_iter().map(
            |((kind, omitted), count)| match &omitted {
                Omitted::Expandable { reason, query } => {
                    OmissionRecord::expandable(kind, count, *reason, query.clone())
                }
                Omitted::Irretrievable(reason) => {
                    OmissionRecord::irretrievable(kind, count, *reason)
                }
            },
        ))
        .map_err(AccountingError::Manifest)?;

        let candidates =
            u32::try_from(self.candidates.len()).map_err(|_| AccountingError::SetTooLarge {
                size: self.candidates.len(),
            })?;
        // `selected` is built by iterating a `BTreeMap`, so it is already in `Name` order —
        // `rule ordering.deterministic` without a sort to forget.
        Ok(ClosedAccounting {
            candidates,
            selected,
            manifest,
        })
    }
}

/// A compile whose every candidate is accounted for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClosedAccounting {
    candidates: u32,
    selected: Vec<Name>,
    manifest: Manifest,
}

impl ClosedAccounting {
    /// How many candidates the compile considered.
    #[must_use]
    pub const fn candidate_count(&self) -> u32 {
        self.candidates
    }

    /// The selection, in canonical `Name` order.
    #[must_use]
    pub fn selected(&self) -> &[Name] {
        &self.selected
    }

    /// The manifest.
    #[must_use]
    pub const fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    /// Recompute the candidate count from the two published halves and compare it against
    /// the one the candidate set declared.
    ///
    /// # Errors
    ///
    /// [`AccountingError::DoesNotReconcile`] carrying both numbers. See this module's "Why
    /// there is a `reconcile` at all".
    pub fn reconcile(&self) -> Result<(), AccountingError> {
        // `saturating_add` rather than `checked_add`: `candidates` is a `u32` and the
        // manifest's counts are `u32`s summed into a `u64`, so the sum cannot reach the
        // width — and a saturated value would fail the comparison below anyway, which is
        // the direction that matters.
        let accounted = self
            .manifest
            .total()
            .saturating_add(self.selected.len() as u64);
        if accounted != u64::from(self.candidates) {
            return Err(AccountingError::DoesNotReconcile {
                candidates: u64::from(self.candidates),
                accounted,
            });
        }
        Ok(())
    }
}

/// A way an accounting fails to be closed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AccountingError {
    /// One identity appears twice in the candidate set.
    RepeatedCandidate {
        /// The repeated identity.
        id: Name,
    },
    /// A disposition names something that is not a candidate.
    UnknownCandidate {
        /// The identity that is not in the set.
        id: Name,
    },
    /// A second decision was recorded for one candidate.
    AlreadyDispositioned {
        /// The candidate already decided.
        id: Name,
    },
    /// A candidate has no decision, so the pack would drop it silently.
    Undispositioned {
        /// The first such candidate, in canonical order.
        first: Name,
        /// How many candidates have no decision.
        remaining: usize,
    },
    /// The derived records do not partition.
    Manifest(ManifestError),
    /// A count does not fit the width the schema's `integer` is carried in here.
    CountOverflow {
        /// The kind whose cell overflowed.
        kind: SelectionKind,
    },
    /// The candidate set is larger than an exact count can name.
    SetTooLarge {
        /// How many candidates it holds.
        size: usize,
    },
    /// The selection plus the manifest is not the candidate set.
    DoesNotReconcile {
        /// What the candidate set declared.
        candidates: u64,
        /// What the selection and manifest account for.
        accounted: u64,
    },
}

impl fmt::Display for AccountingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RepeatedCandidate { id } => {
                write!(f, "`{id}` appears twice in the candidate set")
            }
            Self::UnknownCandidate { id } => {
                write!(f, "`{id}` is not a candidate of this compile")
            }
            Self::AlreadyDispositioned { id } => {
                write!(f, "`{id}` already has a disposition")
            }
            Self::Undispositioned { first, remaining } => write!(
                f,
                "{remaining} candidate(s) have no disposition, starting at `{first}`; a pack \
                 that cannot name what it dropped is malformed, not compact (INV-007)"
            ),
            Self::Manifest(error) => write!(f, "{error}"),
            Self::CountOverflow { kind } => {
                write!(f, "the `{kind}` count does not fit a 32-bit exact count")
            }
            Self::SetTooLarge { size } => {
                write!(
                    f,
                    "a candidate set of {size} is beyond a 32-bit exact count"
                )
            }
            Self::DoesNotReconcile {
                candidates,
                accounted,
            } => write!(
                f,
                "the candidate set has {candidates} members and the pack accounts for \
                 {accounted}"
            ),
        }
    }
}

impl core::error::Error for AccountingError {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::expansion::ExpansionRelation;

    fn name(text: &str) -> Name {
        Name::new(text).expect("well formed")
    }

    fn query(anchor: &str) -> ExpansionQuery {
        ExpansionQuery::new(ExpansionRelation::CausalPredecessors, name(anchor))
    }

    fn candidates() -> CandidateSet {
        CandidateSet::new([
            (name("e_1"), SelectionKind::Event),
            (name("e_2"), SelectionKind::Event),
            (name("e_3"), SelectionKind::Event),
            (name("s_1"), SelectionKind::Source),
        ])
        .expect("four distinct candidates")
    }

    fn budget() -> Omitted {
        Omitted::Expandable {
            reason: OmissionReason::Budget,
            query: query("e_1"),
        }
    }

    #[test]
    fn the_counting_equation_holds_for_every_closed_accounting() {
        let mut accounting = Accounting::over(candidates());
        accounting.select(&name("e_1")).expect("a candidate");
        accounting
            .omit(&name("e_2"), budget())
            .expect("a candidate");
        accounting
            .omit(&name("e_3"), budget())
            .expect("a candidate");
        accounting
            .omit(
                &name("s_1"),
                Omitted::Irretrievable(IrretrievableReason::Redaction),
            )
            .expect("a candidate");
        let closed = accounting.close().expect("every candidate decided");

        assert_eq!(closed.candidate_count(), 4);
        assert_eq!(closed.selected(), [name("e_1")]);
        assert_eq!(closed.manifest().total(), 3);
        // Two cells: two `event`/`budget` items behind one query, one `source`/`redaction`
        // item behind none.
        assert_eq!(closed.manifest().records().len(), 2);
        assert_eq!(closed.manifest().records()[0].count(), 2);
        closed.reconcile().expect("the two halves agree");
    }

    #[test]
    fn an_undispositioned_candidate_cannot_be_closed() {
        let mut accounting = Accounting::over(candidates());
        accounting.select(&name("e_1")).expect("a candidate");
        accounting
            .omit(&name("e_2"), budget())
            .expect("a candidate");
        assert_eq!(
            accounting.clone().close(),
            Err(AccountingError::Undispositioned {
                first: name("e_3"),
                remaining: 2,
            })
        );
        // And the refusal is not a permanent state: deciding the rest closes it.
        accounting
            .omit(&name("e_3"), budget())
            .expect("a candidate");
        accounting.select(&name("s_1")).expect("a candidate");
        accounting.close().expect("now every candidate is decided");
    }

    #[test]
    fn a_candidate_cannot_be_dispositioned_twice() {
        let mut accounting = Accounting::over(candidates());
        accounting.select(&name("e_1")).expect("a candidate");
        assert_eq!(
            accounting.omit(&name("e_1"), budget()),
            Err(AccountingError::AlreadyDispositioned { id: name("e_1") })
        );
        assert_eq!(
            accounting.select(&name("e_1")),
            Err(AccountingError::AlreadyDispositioned { id: name("e_1") })
        );
    }

    #[test]
    fn a_selection_cannot_contain_something_that_was_never_a_candidate() {
        let mut accounting = Accounting::over(candidates());
        assert_eq!(
            accounting.select(&name("e_9")),
            Err(AccountingError::UnknownCandidate { id: name("e_9") })
        );
    }

    #[test]
    fn an_empty_manifest_means_the_selection_is_the_whole_candidate_set() {
        let mut accounting = Accounting::over(candidates());
        for id in ["e_1", "e_2", "e_3", "s_1"] {
            accounting.select(&name(id)).expect("a candidate");
        }
        let closed = accounting.close().expect("every candidate decided");
        assert!(closed.manifest().is_empty());
        assert_eq!(closed.selected().len(), 4);
        closed.reconcile().expect("the two halves agree");
    }

    #[test]
    fn an_empty_candidate_set_closes_to_an_empty_pack_and_an_empty_manifest() {
        let closed = Accounting::over(CandidateSet::new([]).expect("no candidates"))
            .close()
            .expect("nothing to decide");
        assert_eq!(closed.candidate_count(), 0);
        assert!(closed.selected().is_empty());
        assert!(closed.manifest().is_empty());
        closed.reconcile().expect("0 = 0 + 0");
    }

    /// The anti-vacuity mutant RFC 0028's guarantee-violation list names by name ("a shrunk
    /// manifest count"). The accounting cannot produce one, so the test mutates the
    /// *published* value — exactly the artifact a consumer would receive — and requires the
    /// independent reconciliation to catch it. Without this, `reconcile` returning `Ok` on
    /// every honest input would prove nothing.
    #[test]
    fn a_shrunk_manifest_count_fails_reconciliation() {
        let mut accounting = Accounting::over(candidates());
        accounting.select(&name("e_1")).expect("a candidate");
        for id in ["e_2", "e_3", "s_1"] {
            accounting.omit(&name(id), budget()).expect("a candidate");
        }
        let honest = accounting.close().expect("every candidate decided");
        honest
            .reconcile()
            .expect("the honest accounting reconciles");

        let shrunk = ClosedAccounting {
            candidates: honest.candidate_count(),
            selected: honest.selected().to_vec(),
            manifest: Manifest::new(honest.manifest().records().iter().map(|record| {
                OmissionRecord::expandable(
                    record.kind(),
                    record.count() - 1,
                    record.reason(),
                    query("e_1"),
                )
            }))
            .expect("still one cell"),
        };
        // Two cells — two `event`s and one `source`, all dropped for budget — so shrinking
        // each by one loses two of the four candidates.
        assert_eq!(
            shrunk.reconcile(),
            Err(AccountingError::DoesNotReconcile {
                candidates: 4,
                accounted: 2,
            })
        );
    }

    #[test]
    fn an_inflated_manifest_count_fails_reconciliation() {
        // The other direction: a manifest that over-claims is as wrong as one that
        // under-claims, and a check that only caught shrinkage would pass a pack claiming
        // to have dropped things it never held.
        let mut accounting = Accounting::over(candidates());
        for id in ["e_1", "e_2", "e_3", "s_1"] {
            accounting.omit(&name(id), budget()).expect("a candidate");
        }
        let honest = accounting.close().expect("every candidate decided");
        let inflated = ClosedAccounting {
            candidates: honest.candidate_count(),
            selected: honest.selected().to_vec(),
            manifest: Manifest::new(honest.manifest().records().iter().map(|record| {
                OmissionRecord::expandable(
                    record.kind(),
                    record.count() + 5,
                    record.reason(),
                    query("e_1"),
                )
            }))
            .expect("still the same cells"),
        };
        assert!(matches!(
            inflated.reconcile(),
            Err(AccountingError::DoesNotReconcile { .. })
        ));
    }

    #[test]
    fn a_repeated_candidate_is_refused() {
        assert_eq!(
            CandidateSet::new([
                (name("e_1"), SelectionKind::Event),
                (name("e_1"), SelectionKind::Source),
            ]),
            Err(AccountingError::RepeatedCandidate { id: name("e_1") })
        );
    }
}
