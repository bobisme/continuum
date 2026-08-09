//! **Stage 8's premise**: the six transcript-licensed minimality classes, the transcript each
//! one's licence comes from, and the search that produces one (RFC 0028, "Compiler pipeline";
//! rule C3; correction 5).
//!
//! # Scope: bn-3ub6i — the compiler's stage group 4 (stage 8)
//!
//! > | 8 | minimal unsatisfied core / correction-set analysis | a solver artifact, where one
//! > exists | the minimality classes, **each with its own transcript** |
//! >
//! > — RFC 0028, "Compiler pipeline"
//!
//! # Evidence, not assertion: a class is licensed by a transcript, and the transcript is checked
//!
//! Every other guarantee in this crate is licensed by a checker that recomputes something
//! ([`crate::causal::CausalOrder::closure_violation`], [`crate::monitor::PropertyMonitor`]). A
//! minimality class is different in kind: what establishes it is a *search* — "removing any one
//! selected item destroys the witness" is a statement about `n` runs that already happened — and
//! a search is not a thing a consumer can re-derive from the artifact. RFC 0028's answer is that
//! the search leaves a **transcript**, and the transcript is what is checked.
//!
//! So this module carries the evidence and [`crate::witness`] carries the check. The split is
//! [`crate::property`]/[`crate::monitor`]'s exactly: the producer here
//! ([`MinimalitySearch::transcribe`]) *searches* and records what it found; the checker there
//! ([`crate::witness::MinimalityAudit`]) never searches — it takes a transcript it was handed,
//! re-derives every recorded outcome from the premise, and checks the transcript covered exactly
//! the removals the class requires. A transcript that claims an outcome the premise does not
//! produce is a forgery and is named as one; a transcript that skipped a removal licenses
//! nothing. **The analyzer's word is never taken.**
//!
//! # Six classes, and the seventh that is not one
//!
//! > **`ExplanationMinimal` is not minimizer-checkable.** […] Normative: `ExplanationMinimal`
//! > requires the docs/48 preregistered study (plan §21.1, G8) and MUST NOT be emitted from a
//! > minimizer transcript.
//! >
//! > — RFC 0028, correction 5
//!
//! [`Guarantee`] has *seven* minimality members ([`MINIMALITY_GUARANTEES`]). [`MinimalityClass`]
//! has **six**, and the missing one is `ExplanationMinimal`. That is the pin, and it is
//! structural rather than a review note:
//!
//! - There is no [`MinimalityClass`] value for it, so no [`MinimalityTranscript`] can be recorded
//!   for it and no [`MinimizerArtifact`] can carry one.
//! - [`MinimalityClass::guarantee`] is total and never returns it —
//!   `no_class_maps_to_explanation_minimal` pins the image of the map.
//! - [`MinimalityClass::of`] is the partial inverse and answers `None` for it, so a caller
//!   holding a [`Guarantee`] cannot turn it into a class to claim.
//! - [`crate::compile`]'s licensing block matches on [`MinimalityClass`], so the `License::issue`
//!   call sites for minimality are exactly six and `ExplanationMinimal` is not among them. The
//!   integration suite reads every call site in the crate and holds it to a recorded inventory —
//!   the device [`crate::proof`] already uses for `ProofRelevant` — and the same scan over a
//!   mutant with the line spliced in catches it, so the pin is not vacuous.
//!
//! This is the same shape as stage 5's: a guarantee whose checker is not here has no spelling
//! here. What is different is *why*. `ProofRelevant` is absent because its producer is not
//! deployed; `ExplanationMinimal` is absent because a minimizer transcript is **the wrong kind of
//! evidence for it** — no proof service arriving in this workspace would change that, because
//! what it needs is a preregistered human/agent study.
//!
//! # Rule C3, and why it is not an accident
//!
//! > **C3 — Minimality classes are independent claims.** A pack MUST claim exactly the classes
//! > whose checks ran and passed, and a consumer MUST NOT infer one class from another on the
//! > wire.
//! >
//! > — RFC 0028, "Guarantee classes"
//!
//! A [`MinimizerArtifact`] is a map *from class to transcript*, and stage 8 licenses class `C`
//! exactly when the artifact holds a transcript recorded under `C` and that transcript verifies.
//! Note what the rule does **not** rest on: the six removal families are not all distinct. A
//! [`MinimalityClass::CardinalityMinimal`] transcript with `max_removed = 1` examines exactly the
//! singletons, which is [`MinimalityClass::OneMinimal`]'s family — and a
//! [`MinimalityClass::ValueMinimal`] attribution that gives every candidate its own element does
//! the same. C3 holds anyway, because the licence is keyed to the class the transcript was
//! *recorded under* and the audit issues that class and no other. Running the one-removal check
//! licenses `OneMinimal`; it licenses nothing else even where the arithmetic coincides.
//!
//! # The removal families, and why each class has its own
//!
//! Every class asks the same question — "does the witness survive this removal?" — of a different
//! family of removals. [`MinimalityClass::removal_family`] is where each family is defined; it is
//! a function of the *premise* (the class, the causal order, the core, the declared attributions,
//! the declared bound) and of nothing the search decided, which is why the checker may call it
//! too without reusing the producer's implementation.
//!
//! | Class | Removal unit | Family over a core `S` |
//! |---|---|---|
//! | `OneMinimal` | one selected item | `{ {x} : x ∈ S }` |
//! | `CardinalityMinimal` | any smaller subset within the declared bound | `{ U ⊆ S : 1 ≤ |U| ≤ bound }` |
//! | `CausallyMinimal` | a maximal element, so the result is still a configuration | `{ {x} : x maximal in S }` |
//! | `ValueMinimal` | every candidate carrying one value-domain element | `{ S ∩ members(v) : v declared in S }` |
//! | `OwnerMinimal` | every candidate carrying one owner | `{ S ∩ members(o) : o declared in S }` |
//! | `FaultMinimal` | every candidate carrying one injected fault | `{ S ∩ members(f) : f declared in S }` |
//!
//! `CausallyMinimal`'s family is the one that needs its reason stated: RFC 0028 calls it "minimal
//! configuration under causal closure", and removing a *non*-maximal element of a downward-closed
//! set does not leave a configuration at all, so it is not a smaller configuration to compare
//! against. Removing a maximal one always does.
//!
//! # `CardinalityMinimal` cannot be claimed by an unbounded search
//!
//! > | `CardinalityMinimal` | no smaller selected set witnesses the failure | exhaustive
//! > smaller-set search transcript within declared bounds | **an unbounded search cannot claim
//! > it** |
//! >
//! > — RFC 0028, "Guarantee classes"
//!
//! [`SearchBound`] is the declaration, and it is *required*: asking for the class without one is
//! [`MinimalityRefusal::UnboundedSearch`] and nothing is licensed. The bound lives in the
//! transcript rather than in the wire token, which is RFC 0028's own open question ("whether an
//! 'exhaustive within declared bounds' qualifier belongs in the token rather than in the
//! transcript") left where the RFC leaves it — inventing a qualified token here would be this
//! crate deciding a wire vocabulary it does not own.
//!
//! # Determinism (INV-005): the removal search order is declared, not discovered
//!
//! The family is a [`BTreeSet`] of [`BTreeSet<Name>`], and the transcript's steps are a
//! [`BTreeMap`] keyed by the removed set. Both orders are `Name`'s own canonical order, which is
//! shortlex ([`continuum_value::value::Name`]) and is a property of the identities rather than of
//! the iteration that produced them. There is no clock, no entropy, no hash map, and no
//! dependence on the order a caller supplied anything: two searches over one premise produce equal
//! transcripts and equal digests. The keying also makes one contradiction unrepresentable rather
//! than validated — a transcript cannot record two answers for one removal, because the second
//! insert replaces the first rather than adding a row a checker would then have to adjudicate.
//!
//! # What is declined here, and named rather than implied
//!
//! - **Narrowing the selection.** Stage 8 analyses; it does not minimise the published pack. RFC
//!   0028 states the classes *about the selected set* ("removing any one **selected** item"), and
//!   a stage that dropped items would owe the manifest a reason for the drop — for which the
//!   closed five-member [`crate::omission::OmissionReason`] vocabulary has no member that is true
//!   (an item whose removal leaves the witness standing is not `slice-irrelevant`, and it is not
//!   `heuristic-cutoff` either, because the stage *did* decide it). [`crate::correspondence`]
//!   already settled the discipline: inventing a manifest cell "would make the counting equation
//!   false in the direction that looks like diligence". So the counting equation is untouched by
//!   this stage, and what a failed minimality check costs is the class.
//! - **A `verdict` consequence.** RFC 0028's failure-pack profile *SHOULD* claim a minimality
//!   class, not MUST, so an unclaimed class does not make the answer unusable and does not owe
//!   `inconclusive` a reason under rule C1's second sentence. Stage 8 adds nothing to
//!   [`crate::compile::Compilation::inconclusive_reason`], and
//!   [`crate::guarantee::RequestedGuarantees::unachieved`] already computes what a requested and
//!   unachieved class owes the manifest.
//! - **`ExplanationMinimal`'s actual checker.** The docs/48 preregistered study is plan §21.1's
//!   G8 objective and is not a compiler artifact at all. Nothing here approximates it.
//! - **A minimizer for a solver artifact this workspace does not have.** RFC 0028's stage-8 input
//!   is "a solver artifact, **where one exists**". [`MinimalityTranscript`] is publicly
//!   constructible precisely so an external solver's transcript can be handed to the audit and get
//!   an answer from the premise alone; [`MinimalitySearch`] is the in-crate producer for the case
//!   where no external one exists, and it is not privileged — its output goes through the same
//!   audit as anyone else's.
//!
//! # Clause → test
//!
//! | Clause | Source | Test |
//! |---|---|---|
//! | six transcript-checkable classes | RFC 0028, "Guarantee classes" | `the_six_classes_are_the_rfcs_transcript_checkable_ones` |
//! | `ExplanationMinimal` is not one of them | RFC 0028 correction 5 | `no_class_maps_to_explanation_minimal` |
//! | each class has its own removal family | RFC 0028, "Guarantee classes" | `each_class_has_its_own_removal_family` |
//! | a maximal removal keeps the configuration | RFC 0028 stage 8 | `causal_minimality_removes_only_maximal_elements` |
//! | an unbounded cardinality search is refused | RFC 0028, "Guarantee classes" | `cardinality_minimality_without_a_bound_is_refused` |
//! | a dimension needs a declared attribution | INV-013's discipline, at stage 8 | `a_dimension_class_needs_its_attribution` |
//! | a core that witnesses nothing is refused | anti-vacuity; INV-008 | `a_core_that_does_not_witness_the_failure_is_refused` |
//! | deterministic (INV-005) | INV-005, ADR-0003 | `two_searches_over_one_premise_agree` |

use core::fmt;
use std::collections::{BTreeMap, BTreeSet};

use continuum_intent::canonical_json::Json;
use continuum_value::identity::ContentHasher;
use continuum_value::value::Name;

use crate::causal::CausalOrder;
use crate::guarantee::Guarantee;
use crate::property::{AutomatonVerdict, PropertyAutomaton};

/// The seven members of [`Guarantee`] RFC 0028's table groups as minimality classes, in the
/// schema's enum order.
///
/// Transcribed from the RFC's "Guarantee classes" table so that
/// `the_six_classes_are_the_rfcs_transcript_checkable_ones` can state the difference between this
/// list and [`MinimalityClass::ALL`] as one identity: it is exactly
/// [`Guarantee::ExplanationMinimal`].
pub const MINIMALITY_GUARANTEES: [Guarantee; 7] = [
    Guarantee::OneMinimal,
    Guarantee::CardinalityMinimal,
    Guarantee::CausallyMinimal,
    Guarantee::ValueMinimal,
    Guarantee::OwnerMinimal,
    Guarantee::FaultMinimal,
    Guarantee::ExplanationMinimal,
];

/// One minimality class a **transcript** can license.
///
/// Six members, in the schema's `guarantees` order. `ExplanationMinimal` is deliberately not one
/// of them — see the module documentation's "Six classes, and the seventh that is not one".
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum MinimalityClass {
    /// Removing any one selected item destroys the witness.
    OneMinimal,
    /// No smaller selected set witnesses the failure, within the declared bound.
    CardinalityMinimal,
    /// Minimal configuration under causal closure.
    CausallyMinimal,
    /// Domains and names reduced.
    ValueMinimal,
    /// Tasks and nodes reduced.
    OwnerMinimal,
    /// No unnecessary injected fault.
    FaultMinimal,
}

impl MinimalityClass {
    /// All six, in the schema's `guarantees` order.
    pub const ALL: [Self; 6] = [
        Self::OneMinimal,
        Self::CardinalityMinimal,
        Self::CausallyMinimal,
        Self::ValueMinimal,
        Self::OwnerMinimal,
        Self::FaultMinimal,
    ];

    /// The guarantee a verified transcript for this class licenses.
    ///
    /// Total, and never [`Guarantee::ExplanationMinimal`] — there is no class that maps to it.
    #[must_use]
    pub const fn guarantee(self) -> Guarantee {
        match self {
            Self::OneMinimal => Guarantee::OneMinimal,
            Self::CardinalityMinimal => Guarantee::CardinalityMinimal,
            Self::CausallyMinimal => Guarantee::CausallyMinimal,
            Self::ValueMinimal => Guarantee::ValueMinimal,
            Self::OwnerMinimal => Guarantee::OwnerMinimal,
            Self::FaultMinimal => Guarantee::FaultMinimal,
        }
    }

    /// The class that licenses `guarantee`, where a transcript can license it at all.
    ///
    /// `None` for [`Guarantee::ExplanationMinimal`] — correction 5's "a minimizer transcript never
    /// licenses it", made a property of the map rather than a comment — and `None` for every
    /// non-minimality member.
    #[must_use]
    pub fn of(guarantee: Guarantee) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|class| class.guarantee() == guarantee)
    }

    /// A stable token for an auditable intermediate's rendering (INV-016: a token, never prose).
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::OneMinimal => "one-minimal",
            Self::CardinalityMinimal => "cardinality-minimal",
            Self::CausallyMinimal => "causally-minimal",
            Self::ValueMinimal => "value-minimal",
            Self::OwnerMinimal => "owner-minimal",
            Self::FaultMinimal => "fault-minimal",
        }
    }

    /// The dimension this class minimises over, for the three that minimise over one.
    #[must_use]
    pub const fn dimension(self) -> Option<MinimizedDimension> {
        match self {
            Self::ValueMinimal => Some(MinimizedDimension::Value),
            Self::OwnerMinimal => Some(MinimizedDimension::Owner),
            Self::FaultMinimal => Some(MinimizedDimension::Fault),
            _ => None,
        }
    }

    /// Whether this class's transcript must declare a search bound.
    ///
    /// Exactly [`MinimalityClass::CardinalityMinimal`]: "an unbounded search cannot claim it".
    #[must_use]
    pub const fn takes_bound(self) -> bool {
        matches!(self, Self::CardinalityMinimal)
    }

    /// The removals this class's transcript must examine, over `core`.
    ///
    /// The shared *premise* both [`MinimalitySearch`] and [`crate::witness::MinimalityAudit`]
    /// compute from — the class's own definition, not either side's implementation of it. See the
    /// module documentation's table for each family and the reason it is that family.
    ///
    /// # Errors
    ///
    /// [`MinimalityRefusal::EmptyCore`] for a core with nothing in it;
    /// [`MinimalityRefusal::UnboundedSearch`] for [`MinimalityClass::CardinalityMinimal`] with no
    /// declared bound; [`MinimalityRefusal::DimensionNotDeclared`] when a dimension class is asked
    /// and the question declared no attribution for that dimension at all; and
    /// [`MinimalityRefusal::UndeclaredAttribution`] when it declared one that omits a core member —
    /// two different facts, kept apart because INV-008 asks for exactly that.
    pub fn removal_family(
        self,
        order: &CausalOrder,
        core: &BTreeSet<Name>,
        dimensions: &DimensionSet,
        bound: Option<SearchBound>,
    ) -> Result<BTreeSet<BTreeSet<Name>>, MinimalityRefusal> {
        if core.is_empty() {
            return Err(MinimalityRefusal::EmptyCore);
        }
        match self {
            Self::OneMinimal => Ok(core.iter().map(|id| BTreeSet::from([id.clone()])).collect()),
            Self::CardinalityMinimal => {
                let bound = bound.ok_or(MinimalityRefusal::UnboundedSearch)?;
                Ok(bound.subsets_of(core))
            }
            Self::CausallyMinimal => Ok(maximal_elements(order, core)
                .into_iter()
                .map(|id| BTreeSet::from([id]))
                .collect()),
            Self::ValueMinimal | Self::OwnerMinimal | Self::FaultMinimal => {
                let dimension = self
                    .dimension()
                    .expect("the three dimension classes name their dimension");
                let attribution = dimensions
                    .attribution(dimension)
                    .ok_or(MinimalityRefusal::DimensionNotDeclared { dimension })?;
                attribution.units(core)
            }
        }
    }
}

impl fmt::Display for MinimalityClass {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// The maximal elements of `core` under `order` — those no other core member has as an immediate
/// predecessor.
///
/// Removing one from a downward-closed set leaves a downward-closed set, which is why
/// [`MinimalityClass::CausallyMinimal`]'s family is exactly these.
fn maximal_elements(order: &CausalOrder, core: &BTreeSet<Name>) -> BTreeSet<Name> {
    let mut covered: BTreeSet<Name> = BTreeSet::new();
    for id in core {
        if let Some(predecessors) = order.immediate_predecessors(id) {
            covered.extend(predecessors.iter().filter(|p| core.contains(*p)).cloned());
        }
    }
    core.difference(&covered).cloned().collect()
}

/// One of the three dimensions a minimality class can reduce along.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum MinimizedDimension {
    /// The value domain: "domains and names reduced".
    Value,
    /// The owners: "tasks and nodes reduced".
    Owner,
    /// The injected faults: "no unnecessary injected fault".
    Fault,
}

impl MinimizedDimension {
    /// All three, in [`MinimalityClass::ALL`] order.
    pub const ALL: [Self; 3] = [Self::Value, Self::Owner, Self::Fault];

    /// The class that minimises over this dimension.
    #[must_use]
    pub const fn class(self) -> MinimalityClass {
        match self {
            Self::Value => MinimalityClass::ValueMinimal,
            Self::Owner => MinimalityClass::OwnerMinimal,
            Self::Fault => MinimalityClass::FaultMinimal,
        }
    }

    /// A stable token.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Value => "value",
            Self::Owner => "owner",
            Self::Fault => "fault",
        }
    }
}

impl fmt::Display for MinimizedDimension {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A **declared** attribution of candidates to the elements of one dimension.
///
/// Declared rather than inferred, for [`crate::observer::ObserverProjection`]'s reason: this crate
/// cannot read a candidate's owner or its injected fault off a causal order, and a stage that
/// guessed one would be minimising along a dimension nobody described. A core member the
/// attribution omits is [`MinimalityRefusal::UndeclaredAttribution`] and the class is refused —
/// never silently treated as its own element, which would make every such candidate trivially
/// removable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DimensionAttribution {
    dimension: MinimizedDimension,
    attributed: BTreeMap<Name, Name>,
}

impl DimensionAttribution {
    /// Declare `entries` — `(candidate, element)` pairs — as this dimension's attribution.
    #[must_use]
    pub fn new(
        dimension: MinimizedDimension,
        entries: impl IntoIterator<Item = (Name, Name)>,
    ) -> Self {
        Self {
            dimension,
            attributed: entries.into_iter().collect(),
        }
    }

    /// Which dimension this attribution is about.
    #[must_use]
    pub const fn dimension(&self) -> MinimizedDimension {
        self.dimension
    }

    /// The element this candidate carries, where one was declared.
    #[must_use]
    pub fn element(&self, id: &Name) -> Option<&Name> {
        self.attributed.get(id)
    }

    /// Whether this candidate's element was declared.
    #[must_use]
    pub fn is_attributed(&self, id: &Name) -> bool {
        self.attributed.contains_key(id)
    }

    /// Every declared pair, in canonical order.
    pub fn attributed(&self) -> impl Iterator<Item = (&Name, &Name)> {
        self.attributed.iter()
    }

    /// The elements present in `core`, in canonical order.
    #[must_use]
    pub fn elements_in(&self, core: &BTreeSet<Name>) -> BTreeSet<Name> {
        core.iter()
            .filter_map(|id| self.attributed.get(id).cloned())
            .collect()
    }

    /// The removal family this attribution induces over `core`: one unit per element present,
    /// holding every core member carrying it.
    ///
    /// # Errors
    ///
    /// [`MinimalityRefusal::UndeclaredAttribution`] for the first core member, in canonical order,
    /// whose element was never declared.
    pub fn units(
        &self,
        core: &BTreeSet<Name>,
    ) -> Result<BTreeSet<BTreeSet<Name>>, MinimalityRefusal> {
        let mut by_element: BTreeMap<Name, BTreeSet<Name>> = BTreeMap::new();
        for id in core {
            let element = self.attributed.get(id).ok_or_else(|| {
                MinimalityRefusal::UndeclaredAttribution {
                    dimension: self.dimension,
                    id: id.clone(),
                }
            })?;
            by_element
                .entry(element.clone())
                .or_default()
                .insert(id.clone());
        }
        Ok(by_element.into_values().collect())
    }
}

/// The dimension attributions a question declares, at most one per dimension.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DimensionSet {
    by_dimension: BTreeMap<MinimizedDimension, DimensionAttribution>,
}

impl DimensionSet {
    /// A question that declares no dimension at all.
    #[must_use]
    pub fn empty() -> Self {
        Self {
            by_dimension: BTreeMap::new(),
        }
    }

    /// Declare these attributions. A second attribution for one dimension replaces the first, so
    /// two contradictory declarations for one dimension are unrepresentable.
    #[must_use]
    pub fn of(attributions: impl IntoIterator<Item = DimensionAttribution>) -> Self {
        Self {
            by_dimension: attributions
                .into_iter()
                .map(|attribution| (attribution.dimension(), attribution))
                .collect(),
        }
    }

    /// The attribution declared for `dimension`, where one was.
    #[must_use]
    pub fn attribution(&self, dimension: MinimizedDimension) -> Option<&DimensionAttribution> {
        self.by_dimension.get(&dimension)
    }

    /// The dimensions this question declared, in canonical order.
    #[must_use]
    pub fn declared(&self) -> BTreeSet<MinimizedDimension> {
        self.by_dimension.keys().copied().collect()
    }

    /// Whether nothing was declared.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.by_dimension.is_empty()
    }
}

/// The declared bound of an exhaustive smaller-set search.
///
/// "An unbounded search cannot claim it" (RFC 0028), so this type is required for
/// [`MinimalityClass::CardinalityMinimal`] and is what makes the claim a *qualified* one: the
/// transcript establishes that no set obtained by removing up to [`SearchBound::max_removed`]
/// items witnesses the failure, and says so.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SearchBound {
    max_removed: usize,
}

impl SearchBound {
    /// Declare that every removal of up to `max_removed` items was examined.
    ///
    /// `None` for a bound of zero: a search that removes nothing examines no smaller set at all,
    /// and would license the class on an empty family — the vacuity
    /// [`crate::monitor::Inapplicable::EmptyAlphabet`] refuses in its own place.
    #[must_use]
    pub const fn of(max_removed: usize) -> Option<Self> {
        if max_removed == 0 {
            None
        } else {
            Some(Self { max_removed })
        }
    }

    /// How many items at most one examined removal took away.
    #[must_use]
    pub const fn max_removed(self) -> usize {
        self.max_removed
    }

    /// Every non-empty subset of `core` of size at most [`SearchBound::max_removed`], in canonical
    /// order.
    #[must_use]
    pub fn subsets_of(self, core: &BTreeSet<Name>) -> BTreeSet<BTreeSet<Name>> {
        let items: Vec<Name> = core.iter().cloned().collect();
        let ceiling = self.max_removed.min(items.len());
        let mut family: BTreeSet<BTreeSet<Name>> = BTreeSet::new();
        let mut chosen: Vec<Name> = Vec::new();
        for size in 1..=ceiling {
            collect_subsets(&items, size, 0, &mut chosen, &mut family);
        }
        family
    }
}

impl fmt::Display for SearchBound {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "at most {} removed", self.max_removed)
    }
}

/// Enumerate the size-`size` subsets of `items[from..]`, appending each to `family`.
fn collect_subsets(
    items: &[Name],
    size: usize,
    from: usize,
    chosen: &mut Vec<Name>,
    family: &mut BTreeSet<BTreeSet<Name>>,
) {
    if chosen.len() == size {
        family.insert(chosen.iter().cloned().collect());
        return;
    }
    for index in from..items.len() {
        chosen.push(items[index].clone());
        collect_subsets(items, size, index + 1, chosen, family);
        chosen.pop();
    }
}

/// What one removal did to the witness.
///
/// Two members, because a finite run concludes exactly one of them, and the *names* are the
/// RFC's: "removing any one selected item **destroys the witness**".
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum WitnessOutcome {
    /// The reduced set no longer witnesses the failure. This is what a minimality class needs of
    /// every removal in its family.
    Destroyed,
    /// The reduced set still witnesses the failure, so the removed items were not necessary — a
    /// removal that survives *refutes* the class.
    Survived,
}

impl WitnessOutcome {
    /// Both members, in declaration order.
    pub const ALL: [Self; 2] = [Self::Destroyed, Self::Survived];

    /// Read an outcome off a reduced set's automaton verdict.
    ///
    /// The witness is the property automaton's `Violated` verdict over the core (RFC 0028's
    /// failure-pack profile: "a question about a `refuted` or `deadlock` verdict"), so a reduced
    /// set that still runs to `Violated` still witnesses it.
    #[must_use]
    pub const fn of(verdict: AutomatonVerdict) -> Self {
        match verdict {
            AutomatonVerdict::Violated => Self::Survived,
            AutomatonVerdict::Satisfied => Self::Destroyed,
        }
    }

    /// A stable token.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Destroyed => "destroyed",
            Self::Survived => "survived",
        }
    }
}

impl fmt::Display for WitnessOutcome {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// One class's evidence: the core, the declared bound where the class takes one, and what happened
/// to the witness under every removal that was examined.
///
/// **Publicly constructible on purpose.** [`crate::scope::ScopeReduction`]'s reason, at this stage:
/// the audit is only a checker if a transcript [`MinimalitySearch`] would never produce — a forged
/// outcome, a skipped removal, a padded family, a core that is not the pack's — can be handed to it
/// and get an answer decided from the premise alone.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MinimalityTranscript {
    class: MinimalityClass,
    core: BTreeSet<Name>,
    bound: Option<SearchBound>,
    steps: BTreeMap<BTreeSet<Name>, WitnessOutcome>,
}

impl MinimalityTranscript {
    /// Record a transcript for `class` over `core`.
    ///
    /// `steps` are `(removed, outcome)` pairs. Keying by the removed set is what makes "two answers
    /// for one removal" unrepresentable rather than a case a checker has to adjudicate.
    #[must_use]
    pub fn new(
        class: MinimalityClass,
        core: impl IntoIterator<Item = Name>,
        bound: Option<SearchBound>,
        steps: impl IntoIterator<Item = (BTreeSet<Name>, WitnessOutcome)>,
    ) -> Self {
        Self {
            class,
            core: core.into_iter().collect(),
            bound,
            steps: steps.into_iter().collect(),
        }
    }

    /// The class this transcript is evidence for — and the only class it can license (rule C3).
    #[must_use]
    pub const fn class(&self) -> MinimalityClass {
        self.class
    }

    /// The selected set the claim is about.
    #[must_use]
    pub const fn core(&self) -> &BTreeSet<Name> {
        &self.core
    }

    /// The declared search bound, where the class takes one.
    #[must_use]
    pub const fn bound(&self) -> Option<SearchBound> {
        self.bound
    }

    /// Every examined removal and its recorded outcome, in canonical order.
    pub fn steps(&self) -> impl Iterator<Item = (&BTreeSet<Name>, WitnessOutcome)> {
        self.steps
            .iter()
            .map(|(removed, outcome)| (removed, *outcome))
    }

    /// The removals this transcript examined, in canonical order.
    #[must_use]
    pub fn examined(&self) -> BTreeSet<BTreeSet<Name>> {
        self.steps.keys().cloned().collect()
    }

    /// What this transcript recorded for one removal, where it examined it.
    #[must_use]
    pub fn outcome(&self, removed: &BTreeSet<Name>) -> Option<WitnessOutcome> {
        self.steps.get(removed).copied()
    }

    /// How many removals were examined.
    #[must_use]
    pub fn len(&self) -> usize {
        self.steps.len()
    }

    /// Whether nothing was examined at all.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.steps.is_empty()
    }

    /// The auditable rendering: `{bound?, class, core, steps}` in canonical JSON.
    #[must_use]
    pub fn to_json(&self) -> Json {
        let mut fields: BTreeMap<String, Json> = BTreeMap::new();
        if let Some(bound) = self.bound {
            fields.insert(
                "bound".to_owned(),
                Json::Integer(i64::try_from(bound.max_removed()).unwrap_or(i64::MAX)),
            );
        }
        fields.insert(
            "class".to_owned(),
            Json::String(self.class.as_str().to_owned()),
        );
        fields.insert("core".to_owned(), names_to_json(&self.core));
        fields.insert(
            "steps".to_owned(),
            Json::Array(
                self.steps
                    .iter()
                    .map(|(removed, outcome)| {
                        let mut step: BTreeMap<String, Json> = BTreeMap::new();
                        step.insert(
                            "outcome".to_owned(),
                            Json::String(outcome.as_str().to_owned()),
                        );
                        step.insert("removed".to_owned(), names_to_json(removed));
                        Json::Object(step)
                    })
                    .collect(),
            ),
        );
        Json::Object(fields)
    }

    /// The content identity of this transcript (ADR-0013), as a digest token.
    #[must_use]
    pub fn digest<H: ContentHasher>(&self) -> String {
        H::hash(&self.to_json().to_canonical_bytes()).to_token()
    }
}

/// A set of identities as a canonical JSON array.
fn names_to_json(ids: &BTreeSet<Name>) -> Json {
    Json::Array(
        ids.iter()
            .map(|id| Json::String(id.as_str().to_owned()))
            .collect(),
    )
}

/// **Stage 8's input**: "a solver artifact, where one exists" — the transcripts a minimizer
/// produced, at most one per class.
///
/// The classes this carries are *claimed*, never achieved: [`crate::guarantee::RequestedGuarantees`]
/// and [`crate::guarantee::GuaranteeSet`]'s distinction, at stage 8. Which of them a compile
/// licenses is [`crate::witness::MinimalityAudit`]'s answer, not this artifact's.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MinimizerArtifact {
    transcripts: BTreeMap<MinimalityClass, MinimalityTranscript>,
}

impl MinimizerArtifact {
    /// No solver artifact — RFC 0028's "where one exists", in the case where one does not.
    #[must_use]
    pub fn none() -> Self {
        Self {
            transcripts: BTreeMap::new(),
        }
    }

    /// Carry these transcripts, each filed under its own class.
    ///
    /// # Errors
    ///
    /// [`MinimalityError::ClassMismatch`] when a transcript is filed under a class that is not its
    /// own. Rule C3 is about a consumer not inferring one class from another; an artifact whose
    /// index disagreed with its contents would make that inference for them.
    pub fn of(
        transcripts: impl IntoIterator<Item = (MinimalityClass, MinimalityTranscript)>,
    ) -> Result<Self, MinimalityError> {
        let mut filed: BTreeMap<MinimalityClass, MinimalityTranscript> = BTreeMap::new();
        for (class, transcript) in transcripts {
            if transcript.class() != class {
                return Err(MinimalityError::ClassMismatch {
                    filed: class,
                    recorded: transcript.class(),
                });
            }
            filed.insert(class, transcript);
        }
        Ok(Self { transcripts: filed })
    }

    /// The transcript for one class, where the artifact carries one.
    #[must_use]
    pub fn transcript(&self, class: MinimalityClass) -> Option<&MinimalityTranscript> {
        self.transcripts.get(&class)
    }

    /// The classes this artifact claims, in schema order.
    #[must_use]
    pub fn classes(&self) -> BTreeSet<MinimalityClass> {
        self.transcripts.keys().copied().collect()
    }

    /// How many transcripts it carries.
    #[must_use]
    pub fn len(&self) -> usize {
        self.transcripts.len()
    }

    /// Whether no solver artifact exists for this compile.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.transcripts.is_empty()
    }
}

/// **Stage 8's question**: which classes to attempt, the attributions the dimension classes need,
/// and the bound a cardinality search declares.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MinimalityQuestion {
    classes: BTreeSet<MinimalityClass>,
    dimensions: DimensionSet,
    bound: Option<SearchBound>,
}

impl MinimalityQuestion {
    /// Ask stage 8 for these classes.
    #[must_use]
    pub fn asking(classes: impl IntoIterator<Item = MinimalityClass>) -> Self {
        Self {
            classes: classes.into_iter().collect(),
            dimensions: DimensionSet::empty(),
            bound: None,
        }
    }

    /// Ask stage 8 to run with no class named.
    ///
    /// [`crate::proof::ProofSlicing::nothing_named`]'s precedent: a stage configured with nothing
    /// to do records that it ran and refused, which is a different fact from a stage that was never
    /// configured.
    #[must_use]
    pub fn nothing_asked() -> Self {
        Self::default()
    }

    /// Declare the dimension attributions the `Value`/`Owner`/`Fault` classes minimise along.
    #[must_use]
    pub fn with_dimensions(mut self, dimensions: DimensionSet) -> Self {
        self.dimensions = dimensions;
        self
    }

    /// Declare the exhaustive search's bound.
    #[must_use]
    pub const fn with_bound(mut self, bound: SearchBound) -> Self {
        self.bound = Some(bound);
        self
    }

    /// The classes asked for, in schema order.
    #[must_use]
    pub const fn classes(&self) -> &BTreeSet<MinimalityClass> {
        &self.classes
    }

    /// The declared dimension attributions.
    #[must_use]
    pub const fn dimensions(&self) -> &DimensionSet {
        &self.dimensions
    }

    /// The declared search bound, where one was declared.
    #[must_use]
    pub const fn bound(&self) -> Option<SearchBound> {
        self.bound
    }

    /// Whether the question named no class at all.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.classes.is_empty()
    }
}

/// **Stage 8's computation**: run the class's removal family and record what happened.
///
/// This is the *producer*, not the checker. Nothing here licenses anything — the licence comes from
/// [`crate::witness::MinimalityAudit`], which reads this function's output and knows nothing about
/// how it was produced (RFC 0028, "Validation").
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MinimalitySearch;

impl MinimalitySearch {
    /// Search `class`'s removal family over `core` and transcribe the result.
    ///
    /// # Errors
    ///
    /// [`MinimalityRefusal::EmptyCore`] for a core with nothing in it;
    /// [`MinimalityRefusal::NoWitness`] when the core does not witness the failure at all, so
    /// "destroys the witness" has no content — the anti-vacuity boundary
    /// [`crate::monitor::Inapplicable::NoObservedEvent`] draws in its own place; and whatever
    /// [`MinimalityClass::removal_family`] refuses.
    pub fn transcribe(
        class: MinimalityClass,
        order: &CausalOrder,
        automaton: &PropertyAutomaton,
        core: &BTreeSet<Name>,
        dimensions: &DimensionSet,
        bound: Option<SearchBound>,
    ) -> Result<MinimalityTranscript, MinimalityRefusal> {
        if core.is_empty() {
            return Err(MinimalityRefusal::EmptyCore);
        }
        // The topological order is a property of the causal order, not of any one removal, so it
        // is computed once. That is a cost decision and not a semantic one: every run below sees
        // the same sequence it would have seen recomputed, which is what INV-005 asks of it.
        let topological = order.topological_order();
        if run_over(automaton, &topological, core) != AutomatonVerdict::Violated {
            return Err(MinimalityRefusal::NoWitness);
        }
        let family = class.removal_family(order, core, dimensions, bound)?;
        let steps: BTreeMap<BTreeSet<Name>, WitnessOutcome> = family
            .into_iter()
            .map(|removed| {
                let remaining: BTreeSet<Name> = core.difference(&removed).cloned().collect();
                let outcome = WitnessOutcome::of(run_over(automaton, &topological, &remaining));
                (removed, outcome)
            })
            .collect();
        Ok(MinimalityTranscript::new(
            class,
            core.iter().cloned(),
            if class.takes_bound() { bound } else { None },
            steps,
        ))
    }
}

/// Run `automaton` over the members of `set`, in the causal order's own topological order.
///
/// The shared *premise* — [`PropertyAutomaton::run`] and [`CausalOrder::topological_order`] — that
/// [`crate::witness`] computes for itself rather than calling this function. See that module's
/// "The independence argument" for why the duplication is the point.
fn run_over(
    automaton: &PropertyAutomaton,
    topological: &[Name],
    set: &BTreeSet<Name>,
) -> AutomatonVerdict {
    let trace: Vec<Name> = topological
        .iter()
        .filter(|id| set.contains(*id))
        .cloned()
        .collect();
    automaton.run(&trace).verdict()
}

/// A stable token for stage 8's auditable intermediate.
///
/// A closed vocabulary member's own spelling, never caller-supplied prose (INV-016) — the
/// discipline [`crate::monitor::MonitorVerdict::as_str`] set for the same slot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum MinimalityOutcome {
    /// Stage 8 was configured with no class to attempt.
    NothingAsked,
    /// Stage 3 did not run, so nothing here can decide whether a set witnesses the failure.
    NoProperty,
    /// At least one class's transcript verified, and that class is licensed.
    Licensed,
    /// Classes were attempted and none verified. The pack is honest and smaller-claimed.
    Unlicensed,
}

impl MinimalityOutcome {
    /// All four, in declaration order.
    pub const ALL: [Self; 4] = [
        Self::NothingAsked,
        Self::NoProperty,
        Self::Licensed,
        Self::Unlicensed,
    ];

    /// The token the trail records.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NothingAsked => "minimality-nothing-asked",
            Self::NoProperty => "minimality-no-property",
            Self::Licensed => "minimality-licensed",
            Self::Unlicensed => "minimality-unlicensed",
        }
    }
}

impl fmt::Display for MinimalityOutcome {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Why a minimality class cannot be claimed in this shape at all.
///
/// Distinct from a transcript that is *wrong* ([`crate::witness::TranscriptDefect`]) and from a
/// transcript whose own evidence *refutes* the class: three different facts, kept apart because
/// INV-008 asks for exactly that. All three cost the class.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum MinimalityRefusal {
    /// Stage 3 did not run, so this compile has no property automaton and nothing here can decide
    /// whether a set still witnesses the failure.
    NoProperty,
    /// The question named no class for stage 8 to attempt.
    NothingAsked,
    /// The core is empty: a minimality claim about nothing is vacuous.
    EmptyCore,
    /// The core does not witness the failure, so "removing an item destroys the witness" has no
    /// content. Anti-vacuity, at stage 8.
    NoWitness,
    /// [`MinimalityClass::CardinalityMinimal`] was asked for with no declared bound: "an unbounded
    /// search cannot claim it" (RFC 0028).
    UnboundedSearch,
    /// A dimension class was asked for and the question declared no attribution for its dimension.
    DimensionNotDeclared {
        /// The dimension nobody described.
        dimension: MinimizedDimension,
    },
    /// The declared attribution omits a core member, so the dimension's removal family cannot be
    /// computed honestly.
    UndeclaredAttribution {
        /// The dimension whose attribution is incomplete.
        dimension: MinimizedDimension,
        /// The first unattributed core member, in canonical order.
        id: Name,
    },
}

impl MinimalityRefusal {
    /// A stable token.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::NoProperty => "no-property",
            Self::NothingAsked => "nothing-asked",
            Self::EmptyCore => "empty-core",
            Self::NoWitness => "no-witness",
            Self::UnboundedSearch => "unbounded-search",
            Self::DimensionNotDeclared { .. } => "dimension-not-declared",
            Self::UndeclaredAttribution { .. } => "undeclared-attribution",
        }
    }
}

impl fmt::Display for MinimalityRefusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoProperty => f.write_str(
                "this compile ran no property automaton, so nothing here decides whether a \
                 reduced set still witnesses the failure",
            ),
            Self::NothingAsked => {
                f.write_str("this question names no minimality class for stage 8 to attempt")
            }
            Self::EmptyCore => {
                f.write_str("a minimality claim about an empty selection is vacuous")
            }
            Self::NoWitness => f.write_str(
                "the selection does not witness the failure, so no removal can destroy a witness \
                 it does not carry",
            ),
            Self::UnboundedSearch => f.write_str(
                "`CardinalityMinimal` needs a declared search bound; an unbounded search cannot \
                 claim it (RFC 0028, \"Guarantee classes\")",
            ),
            Self::DimensionNotDeclared { dimension } => write!(
                f,
                "no `{dimension}` attribution was declared, so there is no `{dimension}` domain to \
                 reduce along"
            ),
            Self::UndeclaredAttribution { dimension, id } => write!(
                f,
                "`{id}` carries no declared `{dimension}`; a class cannot be minimised along a \
                 dimension a selected item was never described in"
            ),
        }
    }
}

impl core::error::Error for MinimalityRefusal {}

/// A way a stage-8 input fails to be one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MinimalityError {
    /// A transcript is filed in a [`MinimizerArtifact`] under a class that is not its own.
    ClassMismatch {
        /// The class it was filed under.
        filed: MinimalityClass,
        /// The class it records.
        recorded: MinimalityClass,
    },
}

impl fmt::Display for MinimalityError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ClassMismatch { filed, recorded } => write!(
                f,
                "a transcript recording `{recorded}` is filed under `{filed}`; a minimality class \
                 is licensed only by its own transcript (RFC 0028 C3)"
            ),
        }
    }
}

impl core::error::Error for MinimalityError {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::property::{AutomatonState, Coverage};
    use crate::selection::SelectionKind;
    use continuum_value::identity::Blake3Hasher;

    fn name(text: &str) -> Name {
        Name::new(text).expect("well formed")
    }

    fn ids(names: &[&str]) -> BTreeSet<Name> {
        names.iter().map(|id| name(id)).collect()
    }

    /// The chain `e_begin -> e_submit -> e_ack -> e_loss`, plus one isolated noise event.
    fn order() -> CausalOrder {
        CausalOrder::new(
            [
                (name("e_begin"), SelectionKind::Event),
                (name("e_submit"), SelectionKind::Event),
                (name("e_ack"), SelectionKind::Event),
                (name("e_loss"), SelectionKind::Event),
                (name("n_beat"), SelectionKind::Event),
            ],
            [
                (name("e_submit"), name("e_begin")),
                (name("e_ack"), name("e_submit")),
                (name("e_loss"), name("e_ack")),
            ],
        )
        .expect("a well-formed order")
    }

    /// Steps on all four chain events in sequence and violates only at the end, so every single
    /// removal breaks the chain.
    fn automaton() -> PropertyAutomaton {
        let state = |text: &str| AutomatonState::new(name(text));
        PropertyAutomaton::new(
            state("q_0"),
            [
                (state("q_0"), name("e_begin"), state("q_1")),
                (state("q_1"), name("e_submit"), state("q_2")),
                (state("q_2"), name("e_ack"), state("q_3")),
                (state("q_3"), name("e_loss"), state("q_bad")),
            ],
            [state("q_bad")],
            [],
            Coverage::Total,
        )
        .expect("a well-formed automaton")
    }

    fn core() -> BTreeSet<Name> {
        ids(&["e_begin", "e_submit", "e_ack", "e_loss"])
    }

    fn dimensions() -> DimensionSet {
        DimensionSet::of([
            DimensionAttribution::new(
                MinimizedDimension::Value,
                [
                    (name("e_begin"), name("v_id")),
                    (name("e_submit"), name("v_id")),
                    (name("e_ack"), name("v_seq")),
                    (name("e_loss"), name("v_seq")),
                ],
            ),
            DimensionAttribution::new(
                MinimizedDimension::Owner,
                [
                    (name("e_begin"), name("o_client")),
                    (name("e_submit"), name("o_client")),
                    (name("e_ack"), name("o_server")),
                    (name("e_loss"), name("o_server")),
                ],
            ),
            DimensionAttribution::new(
                MinimizedDimension::Fault,
                [
                    (name("e_begin"), name("f_none")),
                    (name("e_submit"), name("f_none")),
                    (name("e_ack"), name("f_none")),
                    (name("e_loss"), name("f_drop")),
                ],
            ),
        ])
    }

    #[test]
    fn the_six_classes_are_the_rfcs_transcript_checkable_ones() {
        let licensed: BTreeSet<Guarantee> = MinimalityClass::ALL
            .into_iter()
            .map(MinimalityClass::guarantee)
            .collect();
        let all: BTreeSet<Guarantee> = MINIMALITY_GUARANTEES.into_iter().collect();
        assert_eq!(licensed.len(), 6);
        assert_eq!(all.len(), 7);
        // The one member of the RFC's minimality table no class maps to.
        let unmapped: Vec<Guarantee> = all.difference(&licensed).copied().collect();
        assert_eq!(unmapped, [Guarantee::ExplanationMinimal]);
    }

    #[test]
    fn no_class_maps_to_explanation_minimal() {
        // Correction 5, as a property of the map rather than as a comment.
        assert!(
            !MinimalityClass::ALL
                .into_iter()
                .any(|class| class.guarantee() == Guarantee::ExplanationMinimal)
        );
        assert_eq!(MinimalityClass::of(Guarantee::ExplanationMinimal), None);
        // And the inverse is exact everywhere else, so the `None` above is not a hole in the map.
        for class in MinimalityClass::ALL {
            assert_eq!(MinimalityClass::of(class.guarantee()), Some(class));
        }
        // A non-minimality member is not a class either.
        assert_eq!(MinimalityClass::of(Guarantee::CausallyClosed), None);
        assert_eq!(MinimalityClass::of(Guarantee::HeuristicRelevant), None);
    }

    #[test]
    fn every_class_token_is_distinct_and_names_its_class() {
        let tokens: BTreeSet<&str> = MinimalityClass::ALL
            .into_iter()
            .map(MinimalityClass::as_str)
            .collect();
        assert_eq!(tokens.len(), 6);
        // Rule C3's rendering half: "`minimal` without a class is prohibited output in every
        // rendering". Every token carries its class, and none of them is the bare word.
        for token in tokens {
            assert_ne!(token, "minimal");
            assert!(token.ends_with("-minimal"), "{token:?}");
            assert!(token.len() > "-minimal".len(), "{token:?}");
        }
    }

    #[test]
    fn each_class_has_its_own_removal_family() {
        let order = order();
        let bound = SearchBound::of(2).expect("a positive bound");
        let families: Vec<BTreeSet<BTreeSet<Name>>> = MinimalityClass::ALL
            .into_iter()
            .map(|class| {
                class
                    .removal_family(&order, &core(), &dimensions(), Some(bound))
                    .expect("every class's family is computable here")
            })
            .collect();
        // One item at a time.
        assert_eq!(families[0].len(), 4);
        // Every subset of size 1 or 2: 4 + 6.
        assert_eq!(families[1].len(), 10);
        // A chain has exactly one maximal element.
        assert_eq!(families[2], BTreeSet::from([ids(&["e_loss"])]));
        // Two value domains, two owners, two faults — with different partitions.
        assert_eq!(
            families[3],
            BTreeSet::from([ids(&["e_begin", "e_submit"]), ids(&["e_ack", "e_loss"])])
        );
        assert_eq!(families[4], families[3]);
        assert_eq!(
            families[5],
            BTreeSet::from([ids(&["e_begin", "e_submit", "e_ack"]), ids(&["e_loss"])])
        );
    }

    #[test]
    fn causal_minimality_removes_only_maximal_elements() {
        // Removing `e_loss` from the chain's closure leaves a configuration; removing `e_ack`
        // would not, so it is not in the family at all.
        let family = MinimalityClass::CausallyMinimal
            .removal_family(&order(), &core(), &DimensionSet::empty(), None)
            .expect("a chain has a maximal element");
        assert_eq!(family, BTreeSet::from([ids(&["e_loss"])]));
        // Two incomparable tails give two maximal elements.
        let wide = CausalOrder::new(
            [
                (name("e_begin"), SelectionKind::Event),
                (name("e_left"), SelectionKind::Event),
                (name("e_right"), SelectionKind::Event),
            ],
            [
                (name("e_left"), name("e_begin")),
                (name("e_right"), name("e_begin")),
            ],
        )
        .expect("a well-formed order");
        assert_eq!(
            MinimalityClass::CausallyMinimal
                .removal_family(
                    &wide,
                    &ids(&["e_begin", "e_left", "e_right"]),
                    &DimensionSet::empty(),
                    None,
                )
                .expect("two maximal elements"),
            BTreeSet::from([ids(&["e_left"]), ids(&["e_right"])])
        );
    }

    #[test]
    fn cardinality_minimality_without_a_bound_is_refused() {
        assert_eq!(
            MinimalityClass::CardinalityMinimal.removal_family(
                &order(),
                &core(),
                &DimensionSet::empty(),
                None
            ),
            Err(MinimalityRefusal::UnboundedSearch)
        );
        // And the other five do not need one.
        for class in MinimalityClass::ALL {
            if class == MinimalityClass::CardinalityMinimal {
                assert!(class.takes_bound());
                continue;
            }
            assert!(!class.takes_bound());
            assert!(
                class
                    .removal_family(&order(), &core(), &dimensions(), None)
                    .is_ok()
            );
        }
    }

    #[test]
    fn a_bound_of_zero_is_not_a_bound() {
        assert_eq!(SearchBound::of(0), None);
        assert_eq!(SearchBound::of(3).expect("positive").max_removed(), 3);
    }

    #[test]
    fn a_bound_larger_than_the_core_stops_at_the_core() {
        // Boundary: the family tops out at removing everything, which leaves the empty set.
        let family = SearchBound::of(9)
            .expect("positive")
            .subsets_of(&ids(&["a_1", "a_2"]));
        assert_eq!(
            family,
            BTreeSet::from([ids(&["a_1"]), ids(&["a_2"]), ids(&["a_1", "a_2"])])
        );
    }

    #[test]
    fn a_dimension_class_needs_its_attribution() {
        // Nothing declared at all.
        assert_eq!(
            MinimalityClass::ValueMinimal.removal_family(
                &order(),
                &core(),
                &DimensionSet::empty(),
                None
            ),
            Err(MinimalityRefusal::DimensionNotDeclared {
                dimension: MinimizedDimension::Value
            })
        );
        // Declared, and incomplete — a different fact (INV-008).
        let partial = DimensionSet::of([DimensionAttribution::new(
            MinimizedDimension::Value,
            [(name("e_begin"), name("v_id"))],
        )]);
        assert_eq!(
            MinimalityClass::ValueMinimal.removal_family(&order(), &core(), &partial, None),
            Err(MinimalityRefusal::UndeclaredAttribution {
                dimension: MinimizedDimension::Value,
                // Shortlex: `e_ack` sorts before `e_loss` and both before `e_submit`.
                id: name("e_ack"),
            })
        );
    }

    #[test]
    fn an_empty_core_has_no_family() {
        for class in MinimalityClass::ALL {
            assert_eq!(
                class.removal_family(
                    &order(),
                    &BTreeSet::new(),
                    &dimensions(),
                    SearchBound::of(1)
                ),
                Err(MinimalityRefusal::EmptyCore)
            );
        }
    }

    #[test]
    fn the_honest_search_records_a_destroyed_witness_everywhere() {
        let transcript = MinimalitySearch::transcribe(
            MinimalityClass::OneMinimal,
            &order(),
            &automaton(),
            &core(),
            &DimensionSet::empty(),
            None,
        )
        .expect("the core witnesses the failure");
        assert_eq!(transcript.class(), MinimalityClass::OneMinimal);
        assert_eq!(transcript.core(), &core());
        assert_eq!(transcript.len(), 4);
        assert_eq!(transcript.bound(), None);
        for (_, outcome) in transcript.steps() {
            assert_eq!(outcome, WitnessOutcome::Destroyed);
        }
    }

    #[test]
    fn a_survivable_removal_is_recorded_as_one() {
        // `n_beat` is in no transition, so a core carrying it is not one-minimal — and the
        // transcript says so rather than hiding it.
        let padded = ids(&["e_begin", "e_submit", "e_ack", "e_loss", "n_beat"]);
        let transcript = MinimalitySearch::transcribe(
            MinimalityClass::OneMinimal,
            &order(),
            &automaton(),
            &padded,
            &DimensionSet::empty(),
            None,
        )
        .expect("the core witnesses the failure");
        assert_eq!(
            transcript.outcome(&ids(&["n_beat"])),
            Some(WitnessOutcome::Survived)
        );
        assert_eq!(
            transcript.outcome(&ids(&["e_ack"])),
            Some(WitnessOutcome::Destroyed)
        );
    }

    #[test]
    fn a_core_that_does_not_witness_the_failure_is_refused() {
        // The chain's prefix runs to `q_3`, which is not violating: there is no witness to
        // destroy, and a transcript claiming otherwise would be licensing vacuity.
        assert_eq!(
            MinimalitySearch::transcribe(
                MinimalityClass::OneMinimal,
                &order(),
                &automaton(),
                &ids(&["e_begin", "e_submit", "e_ack"]),
                &DimensionSet::empty(),
                None,
            ),
            Err(MinimalityRefusal::NoWitness)
        );
    }

    #[test]
    fn a_bound_travels_only_with_the_class_that_takes_one() {
        let bound = SearchBound::of(2).expect("positive");
        let cardinality = MinimalitySearch::transcribe(
            MinimalityClass::CardinalityMinimal,
            &order(),
            &automaton(),
            &core(),
            &DimensionSet::empty(),
            Some(bound),
        )
        .expect("bounded");
        assert_eq!(cardinality.bound(), Some(bound));
        assert_eq!(cardinality.len(), 10);
        // The same bound offered to a class that does not take one is not recorded.
        let one = MinimalitySearch::transcribe(
            MinimalityClass::OneMinimal,
            &order(),
            &automaton(),
            &core(),
            &DimensionSet::empty(),
            Some(bound),
        )
        .expect("unbounded classes ignore the offer");
        assert_eq!(one.bound(), None);
    }

    #[test]
    fn an_artifact_files_a_transcript_under_its_own_class() {
        let one = MinimalitySearch::transcribe(
            MinimalityClass::OneMinimal,
            &order(),
            &automaton(),
            &core(),
            &DimensionSet::empty(),
            None,
        )
        .expect("transcribed");
        let artifact = MinimizerArtifact::of([(MinimalityClass::OneMinimal, one.clone())])
            .expect("filed correctly");
        assert_eq!(
            artifact.classes(),
            BTreeSet::from([MinimalityClass::OneMinimal])
        );
        assert_eq!(artifact.len(), 1);
        assert!(!artifact.is_empty());
        assert!(artifact.transcript(MinimalityClass::ValueMinimal).is_none());
        // Filing it under another class is refused: an index that disagreed with its contents
        // would make rule C3's forbidden inference for the consumer.
        assert_eq!(
            MinimizerArtifact::of([(MinimalityClass::ValueMinimal, one)]),
            Err(MinimalityError::ClassMismatch {
                filed: MinimalityClass::ValueMinimal,
                recorded: MinimalityClass::OneMinimal,
            })
        );
        assert!(MinimizerArtifact::none().is_empty());
    }

    #[test]
    fn a_question_reports_its_own_shape() {
        assert!(MinimalityQuestion::nothing_asked().is_empty());
        let question = MinimalityQuestion::asking([
            MinimalityClass::CardinalityMinimal,
            MinimalityClass::OneMinimal,
        ])
        .with_dimensions(dimensions())
        .with_bound(SearchBound::of(2).expect("positive"));
        assert_eq!(
            question.classes().iter().copied().collect::<Vec<_>>(),
            [
                MinimalityClass::OneMinimal,
                MinimalityClass::CardinalityMinimal
            ]
        );
        assert_eq!(question.bound(), SearchBound::of(2));
        assert_eq!(
            question.dimensions().declared(),
            BTreeSet::from(MinimizedDimension::ALL)
        );
        assert!(!question.dimensions().is_empty());
        assert!(DimensionSet::empty().is_empty());
    }

    #[test]
    fn every_outcome_and_refusal_token_is_a_distinct_token() {
        let notes: BTreeSet<&str> = MinimalityOutcome::ALL
            .into_iter()
            .map(MinimalityOutcome::as_str)
            .collect();
        assert_eq!(notes.len(), 4);
        for note in notes {
            assert!(!note.contains(' '), "{note:?} must be a token");
        }
        let refusals = [
            MinimalityRefusal::NoProperty,
            MinimalityRefusal::NothingAsked,
            MinimalityRefusal::EmptyCore,
            MinimalityRefusal::NoWitness,
            MinimalityRefusal::UnboundedSearch,
            MinimalityRefusal::DimensionNotDeclared {
                dimension: MinimizedDimension::Value,
            },
            MinimalityRefusal::UndeclaredAttribution {
                dimension: MinimizedDimension::Owner,
                id: name("e_ack"),
            },
        ];
        let tokens: BTreeSet<&str> = refusals.iter().map(MinimalityRefusal::as_str).collect();
        assert_eq!(tokens.len(), 7);
        for refusal in &refusals {
            assert!(!refusal.to_string().is_empty());
        }
    }

    #[test]
    fn the_rendering_carries_the_class_the_core_and_every_step() {
        let transcript = MinimalityTranscript::new(
            MinimalityClass::CardinalityMinimal,
            ids(&["a_2", "a_1"]),
            SearchBound::of(1),
            [
                (ids(&["a_1"]), WitnessOutcome::Destroyed),
                (ids(&["a_2"]), WitnessOutcome::Survived),
            ],
        );
        assert_eq!(
            transcript.to_json().to_canonical_bytes(),
            br#"{"bound":1,"class":"cardinality-minimal","core":["a_1","a_2"],"steps":[{"outcome":"destroyed","removed":["a_1"]},{"outcome":"survived","removed":["a_2"]}]}"#
        );
    }

    #[test]
    fn two_searches_over_one_premise_agree() {
        let search = || {
            MinimalitySearch::transcribe(
                MinimalityClass::ValueMinimal,
                &order(),
                &automaton(),
                &core(),
                &dimensions(),
                None,
            )
            .expect("transcribed")
        };
        assert_eq!(search(), search());
        assert_eq!(
            search().digest::<Blake3Hasher>(),
            search().digest::<Blake3Hasher>()
        );
        // And a different transcript is a different digest, so the identity is not vacuous.
        let other = MinimalitySearch::transcribe(
            MinimalityClass::OwnerMinimal,
            &order(),
            &automaton(),
            &core(),
            &dimensions(),
            None,
        )
        .expect("transcribed");
        assert_ne!(
            search().digest::<Blake3Hasher>(),
            other.digest::<Blake3Hasher>()
        );
        // The two families coincide here — the value and owner partitions are the same — and the
        // transcripts still differ, because a transcript names the class it is evidence for.
        assert_eq!(search().examined(), other.examined());
    }

    #[test]
    fn insertion_order_does_not_move_a_transcript() {
        let one = MinimalityTranscript::new(
            MinimalityClass::OneMinimal,
            ids(&["a_1", "a_2"]),
            None,
            [
                (ids(&["a_1"]), WitnessOutcome::Destroyed),
                (ids(&["a_2"]), WitnessOutcome::Destroyed),
            ],
        );
        let other = MinimalityTranscript::new(
            MinimalityClass::OneMinimal,
            ids(&["a_2", "a_1"]),
            None,
            [
                (ids(&["a_2"]), WitnessOutcome::Destroyed),
                (ids(&["a_1"]), WitnessOutcome::Destroyed),
            ],
        );
        assert_eq!(one, other);
        assert_eq!(one.digest::<Blake3Hasher>(), other.digest::<Blake3Hasher>());
        assert!(!one.is_empty());
    }
}
