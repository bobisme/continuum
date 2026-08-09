//! The whiteboard compiler: structured notes in, typed graph proposals out
//! (plan §11.5, docs/44 "Whiteboard", RFC 0038 "Whiteboard compiler").
//!
//! # What this module is for
//!
//! > For complex invention tasks, agents may use a structured whiteboard […] The compiler
//! > turns whiteboard entries into typed graph proposals, rejecting references to
//! > nonexistent artifacts or unsupported status claims.
//! >
//! > — plan §11.5
//!
//! The direction is note → proposals. A whiteboard is where an agent *works*; the graph is
//! what it may *claim*, and this module is the one-way door between them. Nothing here reads
//! the graph back out: docs/44's "human-friendly whiteboard view" is the opposite direction
//! and is [`crate::view`], which shares this module's [`Section`] vocabulary and nothing
//! else. That module states why a view is not a note (RFC 0038 V6) — the shortest form of
//! the reason is that this format has no status member on purpose, so rendering held graph
//! state into one would erase every status the graph holds.
//!
//! # The seven sections are a schema, not a heading list
//!
//! plan §11.5 states seven headings — `Goal`, `Known facts`, `Candidate invariant`,
//! `Counterexample`, `Unresolved obligation`, `Experiment`, `Decision` — as prose inside a
//! code block. INV-003 says prose does not decide artifact shape, so the input format is
//! `notes/plan/schemas/whiteboard-note.schema.json` and this module is transcribed from it,
//! token for token, exactly as [`crate::node`] is transcribed from the node schema.
//! [`Section`] is that vocabulary, closed at seven, and it carries both spellings:
//! [`Section::heading`] is the plan's and [`Section::schema_key`] is the schema's, so a
//! reader of either document finds the same variant.
//!
//! # What each section compiles to, and why
//!
//! | plan §11.5 heading | schema key | compiles to | why that kind |
//! |---|---|---|---|
//! | `Goal` | `goal` | [`NodeKind::Property`] | plan §11.2 `Property` is "a property to be established", which is what a whiteboard goal is |
//! | `Known facts` | `known_facts` | [`NodeKind::Assumption`] | what an author *knows* is, to the graph, what the author's claims are conditional on — reading it as `observed` would be the promotion docs/44 forbids prose from performing |
//! | `Candidate invariant` | `candidate_invariants` | [`NodeKind::InvariantCandidate`] | the names coincide |
//! | `Counterexample` | `counterexamples` | [`NodeKind::Counterexample`] | the names coincide |
//! | `Unresolved obligation` | `unresolved_obligations` | [`NodeKind::ProofGoal`] | plan §11.2 `ProofGoal` is "a goal handed to a proof worker", which is what an open obligation is |
//! | `Experiment` | `experiments` | a [`TaskProposal`], **not** a node | docs/44 "experiments become task proposals", and plan §11.2's twenty kinds have no task kind — inventing one would grow a closed vocabulary |
//! | `Decision` | `decisions` | [`NodeKind::Decision`] plus `SUPPORTS` edges | plan §11.2 `ReviewDecision`, and docs/44 "conclusions require supporting edges" |
//!
//! Every compiled node carries one [`Label`], `whiteboard:<section>`, so which section of
//! which note a proposal came from is recoverable from the node itself — docs/44's "Credit
//! and provenance […] derivation" in the only identity-visible member the node vocabulary
//! has for it. The label is inside the node's identity on purpose: the same claim offered as
//! a known fact and as a candidate invariant is not the same assertion.
//!
//! # docs/44's six compilation rules, and where each one lives
//!
//! > every claim becomes a proposed node; references must resolve; status words in prose do
//! > not promote status; experiments become task proposals; conclusions require supporting
//! > edges; unresolved contradictions remain visible.
//! >
//! > — docs/44, "Whiteboard"
//!
//! | Rule | Mechanism | Not a check because |
//! |---|---|---|
//! | every claim becomes a proposed node | [`EvidenceNode::propose`] is the only constructor and takes no status | — |
//! | references must resolve | [`WhiteboardNote::compile`] refuses [`CompilationRefusal::UnresolvedReference`] | — |
//! | status words in prose do not promote status | the note format **has no status member**, and [`NoteText`] is never parsed | a prose scanner would make the graph a function of wording, and a word list is a heuristic, not a rule |
//! | experiments become task proposals | [`TaskProposal`], returned beside the nodes | — |
//! | conclusions require supporting edges | [`Decision::new`] refuses an empty reference set, so an unsupported conclusion is unrepresentable | — |
//! | unresolved contradictions remain visible | there is no merge, dedup, or reconciliation step: the compiler proposes everything or refuses everything | dropping an entry would be resolving a contradiction by authoring order |
//!
//! # Three things this compiler cannot do
//!
//! - **It cannot write a status.** Every node it emits is at [`ClaimStatus::BOTTOM`],
//!   because [`EvidenceNode::propose`] has no status parameter. RFC 0038: "rejects […]
//!   status claims without evidence" is met by the absence of a field rather than by a
//!   filter — the same device `observe.ingest` gets from the wire's shape.
//! - **It cannot emit a `CHECKED_BY` edge.** It emits exactly one edge kind,
//!   [`EdgeKind::Supports`], and only for a `Decision`. RFC 0038 D3 makes a check edge the
//!   checker's own artifact, appended by the service that performed the check; a note author
//!   is not that service, whatever the note says.
//! - **It cannot infer a relation from a heading.** A `Counterexample` entry emits no
//!   `REFUTES` and no `COUNTEREXAMPLE_TO` edge, because whether a counterexample actually
//!   refutes a candidate is a semantic judgement over artifact content this crate does not
//!   hold — asserting it from a section heading is the empty-success shape the dossier
//!   rejects (`rule errors.unsupported_surface`). An entry's references become
//!   `provenance.inputs`: derivation, never assertion.
//!
//! # Refusal is whole-note
//!
//! [`WhiteboardNote::compile`] either returns a complete [`Compilation`] or a typed
//! [`CompilationRefusal`] naming the offending section and index. It never emits a partial
//! proposal set: publication is per-artifact atomic (INV-017), and a half-compiled note
//! would make "what did the note say" depend on which entries happened to resolve.
//!
//! # This is the library, and the wire is `whiteboard.compile`
//!
//! When this module landed there was no `whiteboard.compile` operation, and the paragraph
//! here said so. There is one now: RFC 0038 W10–W12 decided the wire at protocol 3.5, and
//! `continuumd`'s `daemon::whiteboard` serves it under `rule whiteboard.compilation`. The
//! two are deliberately not one implementation. That module compiles against the *daemon's*
//! graph, whose keys are the wire's `ev_` handles, while [`WhiteboardNote::compile`] below
//! resolves against an [`EvidenceGraph`], whose keys are the within-graph content keys
//! RFC 0038 D4 says never reach the wire; what it *does* borrow is every decision — the
//! section list, [`Section::node_kind`], [`Section::label`], and the constructors that
//! refuse an unsupported conclusion and a reserved note class — so the mapping cannot
//! drift. What a caller gets here is the compiler as a value: build a note,
//! [`WhiteboardNote::compile`] it against a graph, inspect the proposals, and
//! [`Compilation::apply`] them.
//!
//! # Clause → test
//!
//! | Clause | Source | Test |
//! |---|---|---|
//! | seven sections, the schema's keys | plan §11.5, note schema | `the_sections_are_the_seven_plan_11_5_headings`, `every_section_names_its_schema_key` |
//! | the heading → kind map is invertible | RFC 0038 W2, V2 | `the_kind_map_is_injective_so_it_inverts` |
//! | every claim becomes a proposed node | docs/44 | `every_compiled_node_is_proposed` |
//! | references must resolve | docs/44, RFC 0038 | `an_unresolved_reference_refuses_the_whole_note`, `a_resolved_reference_compiles` |
//! | a subject need not resolve | this module | `a_subject_the_graph_does_not_hold_is_not_a_refusal` |
//! | status words do not promote | docs/44 | `status_words_in_prose_reach_nothing`, module doc `compile_fail` |
//! | experiments become task proposals | docs/44 | `an_experiment_becomes_a_task_proposal_and_not_a_node` |
//! | conclusions require supporting edges | docs/44 | `a_conclusion_without_support_is_unrepresentable`, `a_decision_gains_one_supports_edge_per_supporting_node` |
//! | nothing is dropped or merged | docs/44 | `two_identical_entries_are_both_proposed` |
//! | the only edge kind is SUPPORTS | RFC 0038 D3 | `the_compiler_emits_no_check_edge` |
//! | refusal is whole-note | INV-017 | `a_refused_note_proposes_nothing` |
//! | no clock, no ambient epoch | INV-005 | `the_note_supplies_the_time`, `the_caller_supplies_the_epochs` |
//! | the record is the schema object | note schema | `the_record_is_the_schema_object`, `tests/whiteboard_compiler.rs` |
//! | compilation is a function of the note | INV-005, GOV-1-03 | `compiling_twice_gives_identical_identities` |
//!
//! A whiteboard entry cannot name a status — [`Entry::new`] has no status parameter:
//!
//! ```compile_fail
//! use continuum_evidence::claim_status::ClaimStatus;
//! use continuum_evidence::node::ClaimId;
//! use continuum_evidence::provenance::ArtifactRef;
//! use continuum_evidence::whiteboard::{Entry, NoteText};
//!
//! let _ = Entry::new(
//!     ClaimId::new("claim-1").unwrap(),
//!     ArtifactRef::new("prop_9f").unwrap(),
//!     NoteText::new("this one is validated").unwrap(),
//!     [],
//!     ClaimStatus::Validated,
//! );
//! ```
//!
//! The same entry, with every argument the format admits, compiles — and the prose that
//! says "validated" reaches nothing:
//!
//! ```
//! use continuum_evidence::node::ClaimId;
//! use continuum_evidence::provenance::ArtifactRef;
//! use continuum_evidence::whiteboard::{Entry, NoteText};
//!
//! let entry = Entry::new(
//!     ClaimId::new("claim-1").unwrap(),
//!     ArtifactRef::new("prop_9f").unwrap(),
//!     NoteText::new("this one is validated").unwrap(),
//!     [],
//! );
//! assert_eq!(entry.text().as_str(), "this one is validated");
//! ```

use core::fmt;
use std::collections::BTreeSet;

use continuum_value::epoch::EpochSet;
use continuum_value::value::Value;

use crate::actor::{ActorId, field};
use crate::edge::{EdgeRefusal, EdgeRelation, EvidenceEdge};
use crate::graph::{EvidenceGraph, GraphRefusal, Insertion};
use crate::node::{ClaimId, EvidenceNode, IdempotencyKey, Label, NodeKind, NodeRef};
use crate::provenance::{ArtifactRef, Provenance, Timestamp, Tool};

/// The schema-class identity every whiteboard note declares.
pub const SCHEMA_ID: &str = "https://continuum.dev/schema/whiteboard-note.json";

/// The schema epoch this module is transcribed from.
pub const SCHEMA_EPOCH: u128 = 1;

/// The two artifact classes a note handle may not impersonate.
///
/// `ev_` names an evidence node or edge, and a note appears in `provenance.inputs` where an
/// `ev_` handle would read as graph content; `cap_` tokens "are minted randomly and do
/// confer authority" (plan §4.4) and a note confers none. The note schema states the same
/// pair as a `not`/`pattern`, so the type and the schema refuse the same handles.
pub const RESERVED_NOTE_CLASSES: [&str; 2] = ["ev", "cap"];

/// One of plan §11.5's seven whiteboard sections.
///
/// Declared in the note schema's `required` order, which is plan §11.5's code-block order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Section {
    /// `Goal` — what the invention task is trying to establish.
    Goal,
    /// `Known facts` — what the author takes as given.
    KnownFact,
    /// `Candidate invariant` — an invariant offered for checking.
    CandidateInvariant,
    /// `Counterexample` — a witness the author found.
    Counterexample,
    /// `Unresolved obligation` — a goal still open.
    UnresolvedObligation,
    /// `Experiment` — work the author proposes, not a claim.
    Experiment,
    /// `Decision` — a conclusion, which docs/44 requires to be supported.
    Decision,
}

impl Section {
    /// Every section, in plan §11.5's order.
    pub const ALL: [Self; 7] = [
        Self::Goal,
        Self::KnownFact,
        Self::CandidateInvariant,
        Self::Counterexample,
        Self::UnresolvedObligation,
        Self::Experiment,
        Self::Decision,
    ];

    /// plan §11.5's spelling of this section's heading.
    #[must_use]
    pub const fn heading(self) -> &'static str {
        match self {
            Self::Goal => "Goal",
            Self::KnownFact => "Known facts",
            Self::CandidateInvariant => "Candidate invariant",
            Self::Counterexample => "Counterexample",
            Self::UnresolvedObligation => "Unresolved obligation",
            Self::Experiment => "Experiment",
            Self::Decision => "Decision",
        }
    }

    /// The note schema's property name for this section.
    #[must_use]
    pub const fn schema_key(self) -> &'static str {
        match self {
            Self::Goal => "goal",
            Self::KnownFact => "known_facts",
            Self::CandidateInvariant => "candidate_invariants",
            Self::Counterexample => "counterexamples",
            Self::UnresolvedObligation => "unresolved_obligations",
            Self::Experiment => "experiments",
            Self::Decision => "decisions",
        }
    }

    /// The node kind this section proposes, which is [`None`] exactly for
    /// [`Section::Experiment`].
    #[must_use]
    pub const fn node_kind(self) -> Option<NodeKind> {
        match self {
            Self::Goal => Some(NodeKind::Property),
            Self::KnownFact => Some(NodeKind::Assumption),
            Self::CandidateInvariant => Some(NodeKind::InvariantCandidate),
            Self::Counterexample => Some(NodeKind::Counterexample),
            Self::UnresolvedObligation => Some(NodeKind::ProofGoal),
            Self::Experiment => None,
            Self::Decision => Some(NodeKind::Decision),
        }
    }

    /// The section that proposes a node kind, which is [`Section::node_kind`] read backwards.
    ///
    /// Total on the six kinds W2 names and [`None`] on the other fourteen, because
    /// [`Section::node_kind`] is injective: six proposing sections name six distinct kinds,
    /// which is the same fact `rule whiteboard.compilation` clause 4 rests a proposal's
    /// identity on. That injectivity is what makes a *view* possible at all
    /// ([`crate::view`], RFC 0038 V2) — an ambiguous inverse would mean a held node
    /// belonged in two sections and the view would have to choose, which is the semantic
    /// judgement W7 says this vocabulary may not make.
    ///
    /// [`Section::Experiment`] is unreachable here, and so is any kind outside the six: an
    /// experiment is a task proposal and never a node (W3), so no held node maps to it.
    #[must_use]
    pub fn of_kind(kind: NodeKind) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|section| section.node_kind() == Some(kind))
    }

    /// The label a node compiled from this section carries: `whiteboard:<schema key>`.
    ///
    /// # Panics
    ///
    /// Never: [`Section::schema_key`] is a non-empty compile-time constant.
    #[must_use]
    pub fn label(self) -> Label {
        Label::new(&format!("whiteboard:{}", self.schema_key()))
            .expect("a schema key is a non-empty compile-time constant")
    }
}

impl fmt::Display for Section {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.heading())
    }
}

/// The human sentence on one whiteboard line.
///
/// It is carried beside a proposal and never inside one. INV-003: "human text may accompany
/// a machine result, never define it" — so nothing in this string is parsed, matched, or
/// scanned, and no wording in it can reach the graph.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NoteText(String);

impl NoteText {
    /// Write a line.
    ///
    /// # Errors
    ///
    /// [`NoteRefusal::EmptyText`] for the empty string: a section entry with no prose is not
    /// a whiteboard entry, and the note schema's `minLength: 1` says the same thing.
    pub fn new(text: &str) -> Result<Self, NoteRefusal> {
        if text.is_empty() {
            return Err(NoteRefusal::EmptyText);
        }
        Ok(Self(text.to_owned()))
    }

    /// The line.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for NoteText {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// One line of a whiteboard section.
///
/// There is deliberately no status, confidence, verdict, or assurance member. A producer's
/// strongest statement about its own claim is which artifact it is about — the same shape
/// `observe.ingest` has on the wire, and the reason docs/44's "status words in prose do not
/// promote status" needs no enforcement code.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    claim: ClaimId,
    subject: ArtifactRef,
    text: NoteText,
    references: BTreeSet<ArtifactRef>,
}

impl Entry {
    /// Write an entry.
    ///
    /// `subject` is the artifact the entry is *about* and becomes the compiled node's
    /// `artifact`; it is deliberately not required to resolve in the graph, because an entry
    /// whose subject the graph already held could propose nothing new. `references` are the
    /// prior artifacts it is derived from and *are* required to resolve
    /// ([`CompilationRefusal::UnresolvedReference`]).
    ///
    /// Infallible on purpose: every member is already a parsed type, and there is no status,
    /// confidence, or verdict argument to refuse. What an entry *may not* be — a conclusion
    /// with no support — is [`Decision::new`]'s refusal, because that rule is about the
    /// section, not about the line.
    #[must_use]
    pub fn new(
        claim: ClaimId,
        subject: ArtifactRef,
        text: NoteText,
        references: impl IntoIterator<Item = ArtifactRef>,
    ) -> Self {
        Self {
            claim,
            subject,
            text,
            references: references.into_iter().collect(),
        }
    }

    /// The claim identity this entry is about.
    #[must_use]
    pub const fn claim(&self) -> &ClaimId {
        &self.claim
    }

    /// The artifact this entry is about.
    #[must_use]
    pub const fn subject(&self) -> &ArtifactRef {
        &self.subject
    }

    /// The human sentence.
    #[must_use]
    pub const fn text(&self) -> &NoteText {
        &self.text
    }

    /// The prior artifacts this entry is derived from, in canonical ascending order.
    pub fn references(&self) -> impl ExactSizeIterator<Item = &ArtifactRef> {
        self.references.iter()
    }

    /// This entry as the note schema's `entry` object.
    fn to_record(&self) -> Value {
        Value::record([
            (field("claim"), Value::text(self.claim.as_str())),
            (
                field("references"),
                Value::seq(self.references.iter().map(ArtifactRef::to_value))
                    .expect("a reference list nests two levels"),
            ),
            (field("subject"), self.subject.to_value()),
            (field("text"), Value::text(self.text.as_str())),
        ])
        .expect("the four member names are distinct compile-time constants")
    }
}

/// A `Decision` entry: a conclusion, with the support docs/44 requires of one.
///
/// > conclusions require supporting edges
/// >
/// > — docs/44, "Whiteboard"
///
/// A conclusion with no support is unrepresentable rather than refused at compile time: the
/// constructor is the only way to build one and it takes the reference set through the
/// entry it wraps.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Decision(Entry);

impl Decision {
    /// Record a conclusion.
    ///
    /// # Errors
    ///
    /// [`NoteRefusal::ConclusionWithoutSupport`] when the entry cites nothing.
    pub fn new(entry: Entry) -> Result<Self, NoteRefusal> {
        if entry.references.is_empty() {
            return Err(NoteRefusal::ConclusionWithoutSupport);
        }
        Ok(Self(entry))
    }

    /// The entry underneath.
    #[must_use]
    pub const fn entry(&self) -> &Entry {
        &self.0
    }
}

/// An `Experiment` entry: proposed work, not a claim.
///
/// It carries neither a claim identity nor a subject, because it compiles to a
/// [`TaskProposal`] rather than to a node — plan §11.2's twenty node kinds have no task
/// kind, and this module does not grow a closed vocabulary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Experiment {
    text: NoteText,
    references: BTreeSet<ArtifactRef>,
}

impl Experiment {
    /// Propose an experiment.
    #[must_use]
    pub fn new(text: NoteText, references: impl IntoIterator<Item = ArtifactRef>) -> Self {
        Self {
            text,
            references: references.into_iter().collect(),
        }
    }

    /// What the author proposes to run.
    #[must_use]
    pub const fn text(&self) -> &NoteText {
        &self.text
    }

    /// The prior artifacts it would be run against, in canonical ascending order.
    pub fn references(&self) -> impl ExactSizeIterator<Item = &ArtifactRef> {
        self.references.iter()
    }

    /// This experiment as the note schema's `experiment` object.
    fn to_record(&self) -> Value {
        Value::record([
            (
                field("references"),
                Value::seq(self.references.iter().map(ArtifactRef::to_value))
                    .expect("a reference list nests two levels"),
            ),
            (field("text"), Value::text(self.text.as_str())),
        ])
        .expect("the two member names are distinct compile-time constants")
    }
}

/// Why a note, or a line of one, was refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NoteRefusal {
    /// A line with no prose.
    EmptyText,
    /// A `Decision` that cites nothing (docs/44: "conclusions require supporting edges").
    ConclusionWithoutSupport,
    /// A note handle in a class a note may not impersonate ([`RESERVED_NOTE_CLASSES`]).
    ReservedNoteClass {
        /// The class prefix, without its underscore.
        class: String,
    },
}

impl fmt::Display for NoteRefusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyText => f.write_str("a whiteboard line is not the empty string"),
            Self::ConclusionWithoutSupport => {
                f.write_str("a decision cites at least one supporting artifact")
            }
            Self::ReservedNoteClass { class } => write!(
                f,
                "a whiteboard note may not name itself in the `{class}_` artifact class"
            ),
        }
    }
}

impl core::error::Error for NoteRefusal {}

/// A structured whiteboard note: plan §11.5's seven sections, as the note schema closes them.
///
/// Immutable once built. Time is a member rather than a clock read (INV-005, ADR-0003):
/// every node compiled from the note is stamped with the note's own `created_at`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WhiteboardNote {
    note_id: ArtifactRef,
    author: ActorId,
    created_at: Timestamp,
    tool: Option<Tool>,
    goal: Entry,
    known_facts: Vec<Entry>,
    candidate_invariants: Vec<Entry>,
    counterexamples: Vec<Entry>,
    unresolved_obligations: Vec<Entry>,
    experiments: Vec<Experiment>,
    decisions: Vec<Decision>,
}

impl WhiteboardNote {
    /// Open a note on one goal.
    ///
    /// plan §11.5 opens "for complex invention *tasks*", and the `Goal` heading is singular:
    /// a note has exactly one goal, and a note with two goals is two notes.
    ///
    /// # Errors
    ///
    /// [`NoteRefusal::ReservedNoteClass`] when `note_id` is in one of
    /// [`RESERVED_NOTE_CLASSES`].
    pub fn new(
        note_id: ArtifactRef,
        author: ActorId,
        created_at: Timestamp,
        goal: Entry,
    ) -> Result<Self, NoteRefusal> {
        let class = note_id
            .as_str()
            .rsplit_once('_')
            .map_or("", |(class, _)| class);
        if let Some(reserved) = RESERVED_NOTE_CLASSES.iter().find(|it| **it == class) {
            return Err(NoteRefusal::ReservedNoteClass {
                class: (*reserved).to_owned(),
            });
        }
        Ok(Self {
            note_id,
            author,
            created_at,
            tool: None,
            goal,
            known_facts: Vec::new(),
            candidate_invariants: Vec::new(),
            counterexamples: Vec::new(),
            unresolved_obligations: Vec::new(),
            experiments: Vec::new(),
            decisions: Vec::new(),
        })
    }

    /// Name the tool or model version that produced the note (docs/44 "Credit and
    /// provenance").
    #[must_use]
    pub fn with_tool(mut self, tool: Tool) -> Self {
        self.tool = Some(tool);
        self
    }

    /// Add `Known facts` lines.
    #[must_use]
    pub fn with_known_facts(mut self, entries: impl IntoIterator<Item = Entry>) -> Self {
        self.known_facts.extend(entries);
        self
    }

    /// Add `Candidate invariant` lines.
    #[must_use]
    pub fn with_candidate_invariants(mut self, entries: impl IntoIterator<Item = Entry>) -> Self {
        self.candidate_invariants.extend(entries);
        self
    }

    /// Add `Counterexample` lines.
    #[must_use]
    pub fn with_counterexamples(mut self, entries: impl IntoIterator<Item = Entry>) -> Self {
        self.counterexamples.extend(entries);
        self
    }

    /// Add `Unresolved obligation` lines.
    #[must_use]
    pub fn with_unresolved_obligations(mut self, entries: impl IntoIterator<Item = Entry>) -> Self {
        self.unresolved_obligations.extend(entries);
        self
    }

    /// Add `Experiment` lines.
    #[must_use]
    pub fn with_experiments(mut self, experiments: impl IntoIterator<Item = Experiment>) -> Self {
        self.experiments.extend(experiments);
        self
    }

    /// Add `Decision` lines.
    #[must_use]
    pub fn with_decisions(mut self, decisions: impl IntoIterator<Item = Decision>) -> Self {
        self.decisions.extend(decisions);
        self
    }

    /// The handle whoever holds the note names it by.
    #[must_use]
    pub const fn note_id(&self) -> &ArtifactRef {
        &self.note_id
    }

    /// Who wrote it.
    #[must_use]
    pub const fn author(&self) -> &ActorId {
        &self.author
    }

    /// When.
    #[must_use]
    pub const fn created_at(&self) -> &Timestamp {
        &self.created_at
    }

    /// With what tool, when one was named.
    #[must_use]
    pub const fn tool(&self) -> Option<&Tool> {
        self.tool.as_ref()
    }

    /// The single `Goal` line.
    #[must_use]
    pub const fn goal(&self) -> &Entry {
        &self.goal
    }

    /// The lines of one entry-bearing section, in authoring order.
    ///
    /// [`Section::Goal`] yields the one goal; [`Section::Experiment`] and
    /// [`Section::Decision`] yield nothing here — see [`Self::experiments`] and
    /// [`Self::decisions`], whose element types differ.
    #[must_use]
    pub fn entries(&self, section: Section) -> &[Entry] {
        match section {
            Section::Goal => core::slice::from_ref(&self.goal),
            Section::KnownFact => &self.known_facts,
            Section::CandidateInvariant => &self.candidate_invariants,
            Section::Counterexample => &self.counterexamples,
            Section::UnresolvedObligation => &self.unresolved_obligations,
            Section::Experiment | Section::Decision => &[],
        }
    }

    /// The `Experiment` lines, in authoring order.
    #[must_use]
    pub fn experiments(&self) -> &[Experiment] {
        &self.experiments
    }

    /// The `Decision` lines, in authoring order.
    #[must_use]
    pub fn decisions(&self) -> &[Decision] {
        &self.decisions
    }

    /// This note exactly as `whiteboard-note.schema.json` writes it.
    ///
    /// Twelve required members plus `tool` when one was named. Nothing else:
    /// `additionalProperties` is false, so a field written where the schema does not admit
    /// it is as invalid as one missing where it does.
    #[must_use]
    pub fn to_record(&self) -> Value {
        let section = |entries: &[Entry]| {
            Value::seq(entries.iter().map(Entry::to_record)).expect("an entry list nests")
        };
        let mut fields = vec![
            (field("author"), Value::text(self.author.as_str())),
            (
                field("candidate_invariants"),
                section(&self.candidate_invariants),
            ),
            (field("counterexamples"), section(&self.counterexamples)),
            (field("created_at"), self.created_at.to_value()),
            (
                field("decisions"),
                Value::seq(self.decisions.iter().map(|it| it.entry().to_record()))
                    .expect("a decision list nests"),
            ),
            (
                field("experiments"),
                Value::seq(self.experiments.iter().map(Experiment::to_record))
                    .expect("an experiment list nests"),
            ),
            (field("goal"), self.goal.to_record()),
            (field("known_facts"), section(&self.known_facts)),
            (field("note_id"), self.note_id.to_value()),
            (field("schema_epoch"), Value::nat(SCHEMA_EPOCH)),
            (field("schema_id"), Value::text(SCHEMA_ID)),
            (
                field("unresolved_obligations"),
                section(&self.unresolved_obligations),
            ),
        ];
        if let Some(tool) = &self.tool {
            fields.push((field("tool"), Value::text(tool.as_str())));
        }
        Value::record(fields).expect("the member names are distinct compile-time constants")
    }

    /// Compile this note into typed graph proposals.
    ///
    /// `graph` is read, never written: reference resolution asks whether the graph holds a
    /// node *about* each referenced artifact, and a decision's supporting edges are drawn to
    /// the nodes that answer. `epochs` is the pinning the production happened under
    /// (plan §4.6) and is a parameter rather than a default, because a compiler that quietly
    /// pinned nothing would be choosing for its caller.
    ///
    /// # Errors
    ///
    /// - [`CompilationRefusal::UnresolvedReference`] — docs/44 "references must resolve".
    ///   Checked for every section, including experiments, before anything is built, so a
    ///   refused note proposes nothing.
    /// - [`CompilationRefusal::Edge`] — a decision's `SUPPORTS` edge could not be built,
    ///   which happens exactly when the graph already holds the identical decision node and
    ///   it is also the support being drawn from (`EdgeRefusal::SelfEdge`).
    pub fn compile(
        &self,
        graph: &EvidenceGraph,
        epochs: &EpochSet,
    ) -> Result<Compilation, CompilationRefusal> {
        self.check_references(graph)?;

        let mut nodes = Vec::new();
        for section in Section::ALL {
            let Some(kind) = section.node_kind() else {
                continue;
            };
            if section == Section::Decision {
                for (index, decision) in self.decisions.iter().enumerate() {
                    nodes.push(self.compile_entry(section, kind, index, decision.entry(), epochs));
                }
                continue;
            }
            for (index, entry) in self.entries(section).iter().enumerate() {
                nodes.push(self.compile_entry(section, kind, index, entry, epochs));
            }
        }

        let mut edges = Vec::new();
        for compiled in nodes
            .iter()
            .filter(|compiled| compiled.section == Section::Decision)
        {
            let entry = self.decisions[compiled.index].entry();
            let to = NodeRef::of(&compiled.node);
            for reference in &entry.references {
                for (_, supporting) in graph.nodes().filter(|(_, it)| it.artifact() == reference) {
                    let edge = EvidenceEdge::new(
                        EdgeRelation::Supports,
                        NodeRef::of(supporting),
                        to.clone(),
                        [reference.clone()],
                        compiled.node.provenance().clone(),
                    )
                    .map_err(|refusal| CompilationRefusal::Edge {
                        index: compiled.index,
                        refusal,
                    })?;
                    edges.push(edge);
                }
            }
        }

        let tasks = self
            .experiments
            .iter()
            .enumerate()
            .map(|(index, experiment)| TaskProposal {
                index,
                text: experiment.text.clone(),
                references: experiment.references.clone(),
                provenance: self.provenance(experiment.references.iter().cloned(), epochs),
            })
            .collect();

        Ok(Compilation {
            nodes,
            edges,
            tasks,
        })
    }

    /// docs/44's "references must resolve", over every section including experiments.
    fn check_references(&self, graph: &EvidenceGraph) -> Result<(), CompilationRefusal> {
        let resolves =
            |reference: &ArtifactRef| graph.nodes().any(|(_, it)| it.artifact() == reference);
        let check = |section: Section,
                     index: usize,
                     references: &BTreeSet<ArtifactRef>|
         -> Result<(), CompilationRefusal> {
            for reference in references {
                if !resolves(reference) {
                    return Err(CompilationRefusal::UnresolvedReference {
                        section,
                        index,
                        reference: reference.clone(),
                    });
                }
            }
            Ok(())
        };
        for section in Section::ALL {
            for (index, entry) in self.entries(section).iter().enumerate() {
                check(section, index, &entry.references)?;
            }
        }
        for (index, experiment) in self.experiments.iter().enumerate() {
            check(Section::Experiment, index, &experiment.references)?;
        }
        for (index, decision) in self.decisions.iter().enumerate() {
            check(Section::Decision, index, &decision.entry().references)?;
        }
        Ok(())
    }

    fn compile_entry(
        &self,
        section: Section,
        kind: NodeKind,
        index: usize,
        entry: &Entry,
        epochs: &EpochSet,
    ) -> CompiledNode {
        let provenance = self.provenance(entry.references.iter().cloned(), epochs);
        let node = EvidenceNode::propose(
            kind,
            entry.subject.clone(),
            entry.claim.clone(),
            self.idempotency_key(section, index),
            provenance,
        )
        .with_labels([section.label()]);
        CompiledNode {
            section,
            index,
            node,
            text: entry.text.clone(),
        }
    }

    /// The provenance every proposal from one line carries.
    ///
    /// The author and the note's own time, the note handle plus that line's references as
    /// inputs (docs/44 "Credit and provenance […] derivation"), the note's tool when one was
    /// named, and the caller's epoch pinning.
    fn provenance(
        &self,
        references: impl IntoIterator<Item = ArtifactRef>,
        epochs: &EpochSet,
    ) -> Provenance {
        let inputs = core::iter::once(self.note_id.clone()).chain(references);
        let provenance = Provenance::new(self.author.clone(), self.created_at.clone(), inputs)
            .under_epochs(epochs.clone());
        match &self.tool {
            Some(tool) => provenance.with_tool(tool.clone()),
            None => provenance,
        }
    }

    /// The retry key for one line: the note, the section, and the line's position.
    ///
    /// Recompiling the same note is exactly the retry RFC 0026's idempotency keys make safe,
    /// and this key is a function of the note and of nothing else — no counter, no clock.
    /// It is outside the node's identity (plan §11.7), so two holders who name the note
    /// differently still converge on one node.
    ///
    /// # Panics
    ///
    /// Never: the formatted key is non-empty.
    fn idempotency_key(&self, section: Section, index: usize) -> IdempotencyKey {
        IdempotencyKey::new(&format!(
            "{}:{}:{index}",
            self.note_id.as_str(),
            section.schema_key()
        ))
        .expect("a formatted key is non-empty")
    }
}

/// One node a note proposed, with the line it came from.
///
/// The prose is *beside* the node, never inside it: the node vocabulary has no prose member,
/// and INV-003 keeps it that way.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompiledNode {
    section: Section,
    index: usize,
    node: EvidenceNode,
    text: NoteText,
}

impl CompiledNode {
    /// Which section proposed it.
    #[must_use]
    pub const fn section(&self) -> Section {
        self.section
    }

    /// Which line of that section, counting from zero.
    #[must_use]
    pub const fn index(&self) -> usize {
        self.index
    }

    /// The proposal.
    #[must_use]
    pub const fn node(&self) -> &EvidenceNode {
        &self.node
    }

    /// The human sentence that accompanies it.
    #[must_use]
    pub const fn text(&self) -> &NoteText {
        &self.text
    }
}

/// An experiment, compiled.
///
/// > experiments become task proposals
/// >
/// > — docs/44, "Whiteboard"
///
/// A proposal, not a task: nothing here schedules, budgets, or admits anything. plan §20 puts
/// tasks in `continuum-task`, and what that crate does with a proposal is its decision.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskProposal {
    index: usize,
    text: NoteText,
    references: BTreeSet<ArtifactRef>,
    provenance: Provenance,
}

impl TaskProposal {
    /// Which `Experiment` line, counting from zero.
    #[must_use]
    pub const fn index(&self) -> usize {
        self.index
    }

    /// What the author proposes to run.
    #[must_use]
    pub const fn text(&self) -> &NoteText {
        &self.text
    }

    /// The artifacts it would be run against, in canonical ascending order.
    pub fn references(&self) -> impl ExactSizeIterator<Item = &ArtifactRef> {
        self.references.iter()
    }

    /// Who proposed it, when, and from what.
    #[must_use]
    pub const fn provenance(&self) -> &Provenance {
        &self.provenance
    }
}

/// What a note compiled to, before any of it is written.
///
/// Ordered: the nodes come first and the edges second, which is the order
/// [`Compilation::apply`] appends them in, because an edge offered against an endpoint the
/// graph does not yet hold is refused ([`GraphRefusal::UnknownEndpoint`]). The same discipline
/// [`crate::conflict::MaterializedConflict`] states for a conflict's edges.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Compilation {
    nodes: Vec<CompiledNode>,
    edges: Vec<EvidenceEdge>,
    tasks: Vec<TaskProposal>,
}

impl Compilation {
    /// The proposed nodes, in section order and then authoring order.
    pub fn nodes(&self) -> impl ExactSizeIterator<Item = &CompiledNode> {
        self.nodes.iter()
    }

    /// The proposed edges, in the order they must be appended.
    pub fn edges(&self) -> impl ExactSizeIterator<Item = &EvidenceEdge> {
        self.edges.iter()
    }

    /// The proposed tasks, in authoring order.
    pub fn tasks(&self) -> impl ExactSizeIterator<Item = &TaskProposal> {
        self.tasks.iter()
    }

    /// Append this compilation to a graph: every node, then every edge.
    ///
    /// Each append is the graph's own put-if-absent, so recompiling and reapplying a note
    /// converges rather than duplicating ([`Insertion::Converged`]).
    ///
    /// # Errors
    ///
    /// [`GraphRefusal`] from [`EvidenceGraph::add_edge`]. The nodes are appended first and
    /// stay: the graph is append-only and has nothing to roll back with, which is the same
    /// statement [`EvidenceGraph::add_conflict`] makes.
    pub fn apply(&self, graph: &mut EvidenceGraph) -> Result<Applied, GraphRefusal> {
        let nodes = self
            .nodes
            .iter()
            .map(|compiled| graph.add_node(compiled.node.clone()))
            .collect();
        let mut edges = Vec::with_capacity(self.edges.len());
        for edge in &self.edges {
            edges.push(graph.add_edge(edge.clone())?);
        }
        Ok(Applied { nodes, edges })
    }
}

/// What applying a compilation did, append by append.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Applied {
    nodes: Vec<Insertion>,
    edges: Vec<Insertion>,
}

impl Applied {
    /// One insertion per proposed node, in [`Compilation::nodes`] order.
    pub fn nodes(&self) -> impl ExactSizeIterator<Item = &Insertion> {
        self.nodes.iter()
    }

    /// One insertion per proposed edge, in [`Compilation::edges`] order.
    pub fn edges(&self) -> impl ExactSizeIterator<Item = &Insertion> {
        self.edges.iter()
    }

    /// How many appends actually wrote something.
    #[must_use]
    pub fn fresh_count(&self) -> usize {
        self.nodes
            .iter()
            .chain(&self.edges)
            .filter(|it| it.is_fresh())
            .count()
    }
}

/// Why a note did not compile.
///
/// Typed, and each variant names the line it is about: a refusal a caller cannot locate is a
/// refusal a caller cannot act on (INV-008's discipline applied to a compiler).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompilationRefusal {
    /// docs/44: "references must resolve" — the graph holds no node about this artifact.
    UnresolvedReference {
        /// Which section the line is in.
        section: Section,
        /// Which line of that section, counting from zero.
        index: usize,
        /// The reference that resolved to nothing.
        reference: ArtifactRef,
    },
    /// A decision's supporting edge could not be built.
    Edge {
        /// Which `Decision` line, counting from zero.
        index: usize,
        /// Why the edge was refused.
        refusal: EdgeRefusal,
    },
}

impl fmt::Display for CompilationRefusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnresolvedReference {
                section,
                index,
                reference,
            } => write!(
                f,
                "`{section}` line {index} references `{reference}`, which the graph holds no node about"
            ),
            Self::Edge { index, refusal } => {
                write!(f, "`Decision` line {index}: {refusal}")
            }
        }
    }
}

impl core::error::Error for CompilationRefusal {}

#[cfg(test)]
pub(crate) mod tests {
    use continuum_value::epoch::EvidenceEpoch;

    use super::*;
    use crate::claim_status::ClaimStatus;
    use crate::edge::EdgeKind;
    use crate::node::tests::node;

    pub(crate) fn text(line: &str) -> NoteText {
        NoteText::new(line).expect("non-empty")
    }

    pub(crate) fn artifact(handle: &str) -> ArtifactRef {
        ArtifactRef::new(handle).expect("well formed")
    }

    pub(crate) fn entry(claim: &str, subject: &str, references: &[&str]) -> Entry {
        Entry::new(
            ClaimId::new(claim).expect("non-empty"),
            artifact(subject),
            text("a line the compiler never reads"),
            references.iter().map(|it| artifact(it)),
        )
    }

    pub(crate) fn author() -> ActorId {
        ActorId::new("agent:swarm-1").expect("well formed")
    }

    pub(crate) fn when() -> Timestamp {
        Timestamp::new("2026-08-01T12:00:00.000Z").expect("canonical")
    }

    pub(crate) fn note() -> WhiteboardNote {
        WhiteboardNote::new(
            artifact("wb_note1"),
            author(),
            when(),
            entry("claim-goal", "prop_9f", &["trace_9f"]),
        )
        .expect("an unreserved class")
        .with_tool(Tool::new("continuum-forge").expect("non-empty"))
    }

    /// A graph holding one node about `trace_9f`, so a reference to it resolves.
    pub(crate) fn seeded() -> EvidenceGraph {
        let mut graph = EvidenceGraph::new();
        graph.add_node(node(NodeKind::Run, "trace_9f", "claim-seed"));
        graph
    }

    fn compile(note: &WhiteboardNote, graph: &EvidenceGraph) -> Compilation {
        note.compile(graph, &EpochSet::unpinned())
            .expect("every reference resolves")
    }

    #[test]
    fn the_sections_are_the_seven_plan_11_5_headings() {
        // plan §11.5's code block, in its own order.
        let headings: Vec<&str> = Section::ALL.iter().map(|it| it.heading()).collect();
        assert_eq!(
            headings,
            [
                "Goal",
                "Known facts",
                "Candidate invariant",
                "Counterexample",
                "Unresolved obligation",
                "Experiment",
                "Decision",
            ]
        );
    }

    #[test]
    fn every_section_names_its_schema_key() {
        // The note schema's `required` list is the same seven, minus the header and the
        // three note-level members.
        let keys: Vec<&str> = Section::ALL.iter().map(|it| it.schema_key()).collect();
        assert_eq!(
            keys,
            [
                "goal",
                "known_facts",
                "candidate_invariants",
                "counterexamples",
                "unresolved_obligations",
                "experiments",
                "decisions",
            ]
        );
        // One section, and exactly one, proposes no node: docs/44's experiments.
        let kindless: Vec<Section> = Section::ALL
            .into_iter()
            .filter(|it| it.node_kind().is_none())
            .collect();
        assert_eq!(kindless, [Section::Experiment]);
    }

    #[test]
    fn the_kind_map_is_injective_so_it_inverts() {
        // W2 maps six sections onto six kinds, and `Section::of_kind` is that map read
        // backwards. Injectivity is what makes the inverse a function rather than a choice,
        // and it is the premise RFC 0038 V2 rests the view's section membership on — so it
        // is asserted here, at the mapping, and not only where it is used.
        let mut proposed: Vec<NodeKind> = Section::ALL
            .into_iter()
            .filter_map(Section::node_kind)
            .collect();
        assert_eq!(proposed.len(), 6);
        let before = proposed.len();
        proposed.sort_unstable();
        proposed.dedup();
        assert_eq!(proposed.len(), before, "two sections named one kind");
        for section in Section::ALL {
            assert_eq!(
                section.node_kind().and_then(Section::of_kind),
                section.node_kind().map(|_| section),
                "{section}"
            );
        }
        // The fourteen kinds no section names invert to nothing, rather than to a default.
        let unmapped = NodeKind::ALL
            .into_iter()
            .filter(|kind| Section::of_kind(*kind).is_none())
            .count();
        assert_eq!(unmapped, 14);
    }

    #[test]
    fn every_compiled_node_is_proposed() {
        // docs/44: "every claim becomes a proposed node", and creation → proposed is the
        // only transition an author has.
        let note = note()
            .with_known_facts([entry("claim-fact", "model_9f", &[])])
            .with_candidate_invariants([entry("claim-inv", "inv_9f", &[])])
            .with_counterexamples([entry("claim-cex", "crash_9f", &[])])
            .with_unresolved_obligations([entry("claim-obl", "goal_9f", &[])]);
        let compilation = compile(&note, &seeded());
        assert_eq!(compilation.nodes().len(), 5);
        for compiled in compilation.nodes() {
            assert_eq!(compiled.node().status(), ClaimStatus::Proposed);
            // Creation and nothing else: no promotion has landed.
            assert_eq!(compiled.node().version(), 0);
            assert_eq!(compiled.node().status_history(), [ClaimStatus::Proposed]);
        }
        let kinds: Vec<NodeKind> = compilation
            .nodes()
            .map(|compiled| compiled.node().kind())
            .collect();
        assert_eq!(
            kinds,
            [
                NodeKind::Property,
                NodeKind::Assumption,
                NodeKind::InvariantCandidate,
                NodeKind::Counterexample,
                NodeKind::ProofGoal,
            ]
        );
    }

    #[test]
    fn a_compiled_node_carries_its_section_label_and_the_note_as_an_input() {
        let compilation = compile(&note(), &seeded());
        let goal = compilation.nodes().next().expect("the goal");
        let labels: Vec<&str> = goal.node().labels().map(Label::as_str).collect();
        assert_eq!(labels, ["whiteboard:goal"]);
        assert!(goal.node().provenance().has_input(&artifact("wb_note1")));
        assert!(goal.node().provenance().has_input(&artifact("trace_9f")));
        assert_eq!(
            goal.node().provenance().tool().map(Tool::as_str),
            Some("continuum-forge")
        );
    }

    #[test]
    fn an_unresolved_reference_refuses_the_whole_note() {
        // docs/44: "references must resolve".
        let note = note().with_known_facts([entry("claim-fact", "model_9f", &["trace_missing"])]);
        let refusal = note
            .compile(&seeded(), &EpochSet::unpinned())
            .expect_err("the reference resolves to nothing");
        assert_eq!(
            refusal,
            CompilationRefusal::UnresolvedReference {
                section: Section::KnownFact,
                index: 0,
                reference: artifact("trace_missing"),
            }
        );
    }

    #[test]
    fn a_refused_note_proposes_nothing() {
        // INV-017: a half-compiled note would make "what did the note say" depend on which
        // lines happened to resolve. The refusal is whole-note and the graph is untouched.
        let mut graph = seeded();
        let before = graph.node_count();
        let refused =
            note().with_counterexamples([entry("claim-cex", "crash_9f", &["ws_missing"])]);
        assert!(refused.compile(&graph, &EpochSet::unpinned()).is_err());
        assert_eq!(graph.node_count(), before);
        assert_eq!(graph.edge_count(), 0);
        // Anti-vacuity: the same note with the reference removed compiles and does write.
        let ok = note().with_counterexamples([entry("claim-cex", "crash_9f", &[])]);
        let compilation = compile(&ok, &graph);
        let applied = compilation.apply(&mut graph).expect("appended");
        assert_eq!(applied.fresh_count(), 2);
    }

    #[test]
    fn a_resolved_reference_compiles() {
        // Anti-vacuity for the refusal above: the seeded graph holds a node *about*
        // `trace_9f`, which is what "resolves" means.
        let compilation = compile(&note(), &seeded());
        assert_eq!(compilation.nodes().len(), 1);
    }

    #[test]
    fn a_subject_the_graph_does_not_hold_is_not_a_refusal() {
        // The subject is what the entry proposes; requiring it to be held already would make
        // proposing anything new impossible.
        let note = note().with_candidate_invariants([entry("claim-inv", "inv_brand_new", &[])]);
        let compilation = compile(&note, &seeded());
        let invariant = compilation.nodes().nth(1).expect("the candidate");
        assert_eq!(invariant.node().artifact(), &artifact("inv_brand_new"));
    }

    #[test]
    fn status_words_in_prose_reach_nothing() {
        // docs/44: "status words in prose do not promote status". There is no field to fill
        // and no scanner to defeat — the two notes differ only in prose and produce the
        // identical node identity.
        let loud = Entry::new(
            ClaimId::new("claim-goal").expect("non-empty"),
            artifact("prop_9f"),
            text("PROVED, validated, certified by a trusted solver"),
            [artifact("trace_9f")],
        );
        let quiet = Entry::new(
            ClaimId::new("claim-goal").expect("non-empty"),
            artifact("prop_9f"),
            text("still open"),
            [artifact("trace_9f")],
        );
        let graph = seeded();
        let of = |goal: Entry| {
            let note = WhiteboardNote::new(artifact("wb_note1"), author(), when(), goal)
                .expect("an unreserved class")
                .with_tool(Tool::new("continuum-forge").expect("non-empty"));
            let compilation = compile(&note, &graph);
            let first = compilation.nodes().next().expect("the goal");
            (first.node().identity(), first.node().status())
        };
        let (loud_identity, loud_status) = of(loud);
        let (quiet_identity, quiet_status) = of(quiet);
        assert_eq!(loud_identity, quiet_identity);
        assert_eq!(loud_status, ClaimStatus::Proposed);
        assert_eq!(quiet_status, ClaimStatus::Proposed);
    }

    #[test]
    fn an_experiment_becomes_a_task_proposal_and_not_a_node() {
        // docs/44: "experiments become task proposals".
        let note = note().with_experiments([Experiment::new(
            text("re-run the exploration under the durable-visibility observer"),
            [artifact("trace_9f")],
        )]);
        let compilation = compile(&note, &seeded());
        assert_eq!(compilation.nodes().len(), 1, "the goal, and nothing else");
        assert_eq!(compilation.tasks().len(), 1);
        let task = compilation.tasks().next().expect("the experiment");
        assert_eq!(task.index(), 0);
        assert!(task.provenance().has_input(&artifact("wb_note1")));
        assert!(task.provenance().has_input(&artifact("trace_9f")));
    }

    #[test]
    fn a_conclusion_without_support_is_unrepresentable() {
        // docs/44: "conclusions require supporting edges".
        let refusal = Decision::new(entry("claim-dec", "dec_9f", &[]))
            .expect_err("a decision cites something");
        assert_eq!(refusal, NoteRefusal::ConclusionWithoutSupport);
        // Anti-vacuity: one reference is enough.
        assert!(Decision::new(entry("claim-dec", "dec_9f", &["trace_9f"])).is_ok());
    }

    #[test]
    fn a_decision_gains_one_supports_edge_per_supporting_node() {
        let mut graph = seeded();
        // A second node about the same artifact: both support the conclusion, and choosing
        // between them is not the compiler's to make.
        graph.add_node(node(NodeKind::Certificate, "trace_9f", "claim-second"));
        let note =
            note().with_decisions([
                Decision::new(entry("claim-dec", "dec_9f", &["trace_9f"])).expect("supported")
            ]);
        let compilation = compile(&note, &graph);
        assert_eq!(compilation.edges().len(), 2);
        for edge in compilation.edges() {
            assert_eq!(edge.kind(), EdgeKind::Supports);
            let evidence: Vec<&ArtifactRef> = edge.evidence().collect();
            assert_eq!(evidence, [&artifact("trace_9f")]);
        }
        // The edges point *into* the decision node.
        let decision = compilation
            .nodes()
            .find(|it| it.section() == Section::Decision)
            .expect("the decision");
        for edge in compilation.edges() {
            assert_eq!(edge.to().identity(), &decision.node().identity());
        }
    }

    #[test]
    fn the_compiler_emits_no_check_edge() {
        // RFC 0038 D3: a `CHECKED_BY` edge is the checker's own artifact. A note author is
        // not a checker, so the compiler's edge vocabulary is one kind wide.
        let mut graph = seeded();
        graph.add_node(node(NodeKind::Receipt, "receipt_9f", "claim-receipt"));
        let note = note().with_decisions([Decision::new(entry(
            "claim-dec",
            "dec_9f",
            &["trace_9f", "receipt_9f"],
        ))
        .expect("supported")]);
        let compilation = compile(&note, &graph);
        assert!(compilation.edges().len() >= 2);
        for edge in compilation.edges() {
            assert_eq!(edge.kind(), EdgeKind::Supports);
            assert!(edge.checker().is_none());
            assert!(!edge.kind().requires_checker());
        }
    }

    #[test]
    fn two_identical_entries_are_both_proposed() {
        // docs/44: "unresolved contradictions remain visible" — the compiler has no merge,
        // dedup, or reconciliation step. Two identical lines are two proposals, and it is
        // the graph's put-if-absent, not the compiler, that converges them.
        let duplicate = entry("claim-fact", "model_9f", &[]);
        let note = note().with_known_facts([duplicate.clone(), duplicate]);
        let compilation = compile(&note, &seeded());
        assert_eq!(compilation.nodes().len(), 3);
        let mut graph = seeded();
        let applied = compilation.apply(&mut graph).expect("appended");
        let fresh: Vec<bool> = applied.nodes().map(Insertion::is_fresh).collect();
        assert_eq!(fresh, [true, true, false]);
    }

    #[test]
    fn a_note_may_not_name_itself_evidence_or_a_capability() {
        for class in RESERVED_NOTE_CLASSES {
            let refusal = WhiteboardNote::new(
                artifact(&format!("{class}_9f")),
                author(),
                when(),
                entry("claim-goal", "prop_9f", &[]),
            )
            .expect_err("a reserved class");
            assert_eq!(
                refusal,
                NoteRefusal::ReservedNoteClass {
                    class: class.to_owned()
                }
            );
        }
        // Anti-vacuity: a neighbouring class is fine.
        assert!(
            WhiteboardNote::new(
                artifact("evidence_9f"),
                author(),
                when(),
                entry("claim-goal", "prop_9f", &[]),
            )
            .is_ok()
        );
    }

    #[test]
    fn an_empty_line_is_refused() {
        assert_eq!(NoteText::new(""), Err(NoteRefusal::EmptyText));
        assert!(NoteText::new(" ").is_ok(), "one space is a line");
    }

    #[test]
    fn the_note_supplies_the_time() {
        // INV-005: no clock. The stamp on every proposal is the note's own.
        let compilation = compile(&note(), &seeded());
        for compiled in compilation.nodes() {
            assert_eq!(compiled.node().provenance().created_at(), &when());
        }
    }

    #[test]
    fn the_caller_supplies_the_epochs() {
        // plan §4.6: what a production was pinned under is part of what was published, so
        // the pinning has to move the identity — and the compiler chooses no default, which
        // is why it is a parameter and not a field.
        let note = note();
        let graph = seeded();
        let unpinned = note
            .compile(&graph, &EpochSet::unpinned())
            .expect("compiles");
        let pinned = note
            .compile(
                &graph,
                &EpochSet::unpinned()
                    .with_evidence(EvidenceEpoch::new("ev-1").expect("a well-formed token")),
            )
            .expect("compiles");
        let identity = |compilation: &Compilation| {
            compilation
                .nodes()
                .next()
                .expect("the goal")
                .node()
                .identity()
        };
        assert_ne!(identity(&unpinned), identity(&pinned));
        assert_eq!(
            unpinned
                .nodes()
                .next()
                .expect("the goal")
                .node()
                .provenance()
                .epochs(),
            &EpochSet::unpinned()
        );
    }

    #[test]
    fn compiling_twice_gives_identical_identities() {
        // GOV-1-03, INV-005: the compilation is a function of the note and the graph.
        let note =
            note().with_decisions([
                Decision::new(entry("claim-dec", "dec_9f", &["trace_9f"])).expect("supported")
            ]);
        let graph = seeded();
        assert_eq!(compile(&note, &graph), compile(&note, &graph));
    }

    #[test]
    fn the_record_is_the_schema_object() {
        // `whiteboard-note.schema.json`'s twelve required members, plus `tool`.
        let note = note()
            .with_known_facts([entry("claim-fact", "model_9f", &[])])
            .with_experiments([Experiment::new(text("run it"), [])])
            .with_decisions([
                Decision::new(entry("claim-dec", "dec_9f", &["trace_9f"])).expect("supported")
            ]);
        let Value::Record(record) = note.to_record() else {
            panic!("a note renders as a record");
        };
        // A `Value::Record` keys on `Name`, whose order is CVNF's shortlex, not the schema
        // document's; the claim here is which members exist, so they are compared sorted.
        let mut members: Vec<String> = record.keys().map(|it| it.as_str().to_owned()).collect();
        members.sort();
        assert_eq!(
            members,
            [
                "author",
                "candidate_invariants",
                "counterexamples",
                "created_at",
                "decisions",
                "experiments",
                "goal",
                "known_facts",
                "note_id",
                "schema_epoch",
                "schema_id",
                "tool",
                "unresolved_obligations",
            ]
        );
        // `tool` is present exactly when one was named, as `provenance.tool` is.
        let bare = WhiteboardNote::new(
            artifact("wb_note1"),
            author(),
            when(),
            entry("claim-goal", "prop_9f", &[]),
        )
        .expect("an unreserved class");
        let Value::Record(record) = bare.to_record() else {
            panic!("a note renders as a record");
        };
        assert!(!record.keys().any(|it| it.as_str() == "tool"));
    }
}
