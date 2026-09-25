//! The incremental engine: content-addressed memoization over parse, elaborate,
//! build, project, explore, and check, with every reuse licensed by a classed edge.
//!
//! Decision: RFC 0030. This is the *custom* side of the PR 22a build-vs-adopt spike:
//! no memoization framework, a `BTreeMap` from canonical key to entry, and one
//! decision function ([`Engine::decide`]) that turns a cache hit into a typed
//! [`Outcome`].
//!
//! # How a reuse is licensed
//!
//! A query is demanded with its canonical key ([`QueryKey`]) and the identities of
//! its *side inputs* — what it reads that its key does not name ([`Keyed::Side`]).
//! On a key hit:
//!
//! - every side identity equal to the cached one: the licence is `Exact`, evidence
//!   `content-identity`;
//! - a side identity changed: the licence is the meet of the changed side edges'
//!   classes (`Conservative`), evidence `conservative-dependency-theorem`, and the
//!   edge records its assumption ([`crate::query::ASSUMPTION_A1`],
//!   [`crate::query::ASSUMPTION_A2`]).
//!
//! On a key miss two stage-specific licences exist:
//!
//! - **`Validated`** (`domain_safety`): the previous revision's closed state set is
//!   emitted as a finite-closure certificate with the rows the reference engine's
//!   emitter computes for the *current* model, and checked from its wire form by
//!   `continuum-certificate` — the kernel, which shares no code with the engine.
//!   `Verified` establishes `Init ⊆ S`, `Post(S) ⊆ S` and `S ⊆ Domain` over those
//!   rows, so every reachable state of the current model lies in `S`; with the
//!   bounds checked against the kernel's own state and transition counts, a clean
//!   exploration closes without an evaluation error, which is exactly the output
//!   [`DomainSafety::Established`]. The certificate is wire epoch 2: it carries the
//!   model, the kernel re-derives every successor row from that carried model, and
//!   [`check_witness`] binds the carried model's canonical identity to the current
//!   model's, byte for byte, with every envelope field. So the closure is about
//!   the engine's model, not about rows an engine computed. What the kernel does
//!   not check is action definedness over `S`: the engine scans it itself and the
//!   reuse records that as assumption V1 ([`crate::query::ASSUMPTION_V1`]). A
//!   witness the kernel does
//!   not verify licenses nothing and the query is recomputed
//!   ([`Cause::WitnessRefused`]). **It saves no work in this pipeline:** every
//!   invariant check demands `explore`, which recomputes whenever the
//!   transition-system projection changes, and the witness costs an emission and a
//!   kernel re-walk on top. It is here to show the class and its checker path
//!   work, and its cost is measured.
//! - **`Experimental`** (`parse`, interactive lane only): a textual heuristic —
//!   strip `//` to end of line, collapse every whitespace run, newlines included —
//!   matches a cached source, and the cached parse is reused *without parsing*. It is
//!   unsound on purpose (line breaks separate CML statements, and `//` may sit
//!   inside a string literal); the audit measures it, and it can never support
//!   promotion.
//!
//! The licence is then checked against the quarantine set (a quarantined triple's
//! reuse drops to `Experimental`, RFC 0030 downgrade rule 1) and against the lane
//! (the promotion lane admits no `Experimental` reuse). A refused licence is a
//! recompute with a typed [`Cause`], never a silent fallback.
//!
//! A result's derivation class is the meet of its inputs' classes and, when reused,
//! its licence (RFC 0030, "Meet rule for composition").
//!
//! # Resource bounds (INV-016: CML source is untrusted)
//!
//! Every revision runs under a [`Limits`] work meter, and each charge is made
//! *before* the work it pays for:
//!
//! - the source length is compared with [`Limits::source_bytes`] before the source
//!   is hashed, normalized, or parsed;
//! - hashing, normalizing and dumping are charged by byte length;
//! - each key is charged its exact encoded length ([`QueryKey::encoded_len`]) before
//!   it is built;
//! - the number of queries a model demands (two per invariant) is checked against
//!   [`Limits::queries`] before any per-invariant query exists;
//! - the transition-system projection clones the normalized model: charged by the
//!   size of the normalized identity first;
//! - exploration is charged its declared bounds (states plus transitions) before it
//!   runs, invariant checks the reachable-state count before the loop, and the
//!   `Validated` witness the old state set's size and transition count before it is
//!   emitted.
//!
//! Elaboration and lowering run under their own pre-charged budgets
//! (`continuum_cml_elab::Limits`); a result that failed *because* of a budget is
//! returned but not memoized ([`Memo::NotMemoized`]), because the budget is not a
//! key component (RFC 0030, "What is not in the key").
//!
//! A refusal ([`Refusal`]) is atomic: every entry a revision computes is staged, and
//! the cache, the index and the quarantine set change only when the whole revision
//! succeeds.

use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;

use continuum_certificate::continuum_kernel_core::verdict::{CertificateKind, PropertyClass};
use continuum_certificate::{KernelVerdict, Outcome as CheckOutcome, check_certificate};
use continuum_cml_elab::NormModel;
use continuum_cml_syntax::SourceFile;
use continuum_engine_reference::bfs::{self, Bounds, Exploration, ExplorationError};
use continuum_engine_reference::certificate::{ClaimEnvelope, ClosedSet, PRODUCER};
use continuum_engine_reference::model::Model;
use continuum_model_core::definedness::{Definedness, definedness_base};
use continuum_value::identity::{Blake3Hasher, ContentHasher, Digest256};

use crate::output::{self, DomainSafety, InvariantVerdict};
use crate::query::{EdgeSpec, Keyed, Label, QueryDefinition, QueryKey, Stage, Triple};
use crate::vocab::{Admission, EvidenceForm, Lane, ReuseClass};

/// The resource limits of one revision.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    /// The largest source accepted, in bytes. Checked before any other work.
    pub source_bytes: usize,
    /// The engine's own work budget, in units (a byte hashed, compared or copied;
    /// a state explored or checked).
    pub work: u64,
    /// The most queries one revision may demand.
    pub queries: usize,
    /// The elaborator's and the lowering's budgets.
    pub elab: continuum_cml_elab::Limits,
    /// The exploration bounds: declared scope configuration, part of the key.
    pub explore: Bounds,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            source_bytes: continuum_cml_syntax::MAX_SOURCE_BYTES as usize,
            work: 1 << 36,
            queries: 4096,
            elab: continuum_cml_elab::Limits::default(),
            explore: Bounds::CERTIFIABLE,
        }
    }
}

/// Whether the `Experimental` parse heuristic may be used.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Heuristic {
    /// Used in the interactive lane.
    Enabled,
    /// Never used.
    Disabled,
}

/// Engine configuration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    /// The semantic epoch every key pins.
    pub semantic_epoch: String,
    /// The resource limits.
    pub limits: Limits,
    /// The `Experimental` parse heuristic.
    pub heuristic: Heuristic,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            semantic_epoch: crate::query::DEFAULT_SEMANTIC_EPOCH.to_owned(),
            limits: Limits::default(),
            heuristic: Heuristic::Enabled,
        }
    }
}

/// Why a revision was refused. Nothing changed in the engine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refusal {
    /// The source is longer than [`Limits::source_bytes`].
    SourceTooLarge {
        /// Its length.
        bytes: usize,
        /// The limit.
        max: usize,
    },
    /// A charge did not fit in what was left of [`Limits::work`].
    WorkExhausted {
        /// Where.
        at: &'static str,
        /// The charge.
        needed: u64,
        /// What was left.
        left: u64,
    },
    /// The model demands more queries than [`Limits::queries`].
    TooManyQueries {
        /// Demanded.
        needed: usize,
        /// The limit.
        max: usize,
    },
    /// Two queries of one revision would share a label (for example, two invariants
    /// with one name, which the elaborator rejects; kept as a typed refusal so the
    /// engine never overwrites a record).
    DuplicateLabel(Label),
    /// A stage with no registered definition was demanded: an engine defect, kept
    /// typed rather than panicking.
    Unregistered(Stage),
}

/// The work meter. Every charge happens before the work.
#[derive(Debug)]
struct Meter {
    left: u64,
    used: u64,
}

impl Meter {
    fn charge(&mut self, n: u64, at: &'static str) -> Result<(), Refusal> {
        match self.left.checked_sub(n) {
            Some(left) => {
                self.left = left;
                self.used = self.used.saturating_add(n);
                Ok(())
            }
            None => Err(Refusal::WorkExhausted {
                at,
                needed: n,
                left: self.left,
            }),
        }
    }

    fn bytes(&mut self, n: usize, at: &'static str) -> Result<(), Refusal> {
        self.charge(u64::try_from(n).unwrap_or(u64::MAX), at)
    }
}

/// Why a query was recomputed rather than reused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Cause {
    /// No cached entry has this key.
    KeyMiss,
    /// A licence existed and the lane refused its class.
    LaneRefused(ReuseClass),
    /// A licence existed and its triple is quarantined, which dropped it to
    /// `Experimental`, which the lane refused.
    Quarantined(Triple),
    /// A `Validated` witness was tried and licensed nothing.
    WitnessRefused(WitnessRefusal),
    /// The key missed, and the stage's alternate licence (the `Experimental`
    /// heuristic or the `Validated` witness) was found but refused by quarantine
    /// or by the lane. The key miss is the cause; this records what was refused.
    AlternateRefused(Box<Cause>),
}

/// Why a `Validated` witness licensed nothing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WitnessRefusal {
    /// The previous result was not established from a closed exploration.
    NoPriorClosedSet,
    /// The certificate could not be emitted for the current model (a successor row
    /// failed, or a count is out of the wire range).
    Emission(String),
    /// The kernel did not verify it; the verdict, rendered.
    NotVerified(String),
    /// The kernel verified a claim, but not the expected one: the first envelope
    /// field (or `kind`) that differs from what the engine asked for.
    WrongClaim(&'static str),
    /// The verified set does not fit the configured bounds, so a clean exploration
    /// could trip one.
    OutsideBounds,
    /// Some action's read is undefined at a state of the witness set.
    UndefinedAction,
    /// Some action's definedness predicate cannot be evaluated at a state of the
    /// witness set; the evaluator's message.
    GuardEvaluation(String),
}

/// Whether a recomputed result was stored.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Memo {
    /// Stored under its key.
    Stored,
    /// Returned but not stored: the result depends on a budget the key does not
    /// name.
    NotMemoized(&'static str),
}

/// How one query's result was obtained.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// Reused through a licence.
    Reused {
        /// The licence's class after quarantine.
        licence: ReuseClass,
        /// The evidence form, or `None` for a heuristic (`Experimental`) reuse.
        evidence: Option<EvidenceForm>,
        /// The quarantine unit of this licence.
        triple: Triple,
        /// The assumptions of the `Conservative` side edges whose inputs differed
        /// from the cached entry's when the decision was made: exactly what the
        /// reuse rests on, recorded on the edge (RFC 0030, `Conservative`).
        assumptions: Vec<&'static str>,
    },
    /// Computed from the current inputs.
    Recomputed {
        /// Why.
        cause: Cause,
        /// Whether it was stored.
        memo: Memo,
    },
}

/// One edge as used in one revision.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EdgeUse {
    /// The query read.
    pub from: Label,
    /// The declared edge.
    pub spec: EdgeSpec,
}

/// A canonical output, shared between records and entries, with its commitment.
///
/// Equality is byte equality of the canonical encoding; the digest only indexes.
#[derive(Debug, Clone)]
pub struct Output {
    bytes: Rc<[u8]>,
    digest: Digest256,
}

impl Output {
    /// The canonical encoding.
    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// The BLAKE3 commitment.
    #[must_use]
    pub const fn digest(&self) -> Digest256 {
        self.digest
    }
}

impl PartialEq for Output {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.bytes, &other.bytes) || self.bytes == other.bytes
    }
}

impl Eq for Output {}

/// One query's record in a revision.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Record {
    /// The query.
    pub label: Label,
    /// Its definition.
    pub definition: &'static QueryDefinition,
    /// Its key's display handle.
    pub handle: String,
    /// Its canonical output.
    pub output: Output,
    /// How it was obtained.
    pub outcome: Outcome,
    /// The derivation's class: the meet of the inputs' classes and, when reused,
    /// the licence.
    pub class: ReuseClass,
    /// The edges it read, in declared order.
    pub edges: Vec<EdgeUse>,
    /// Every licence triple on this result's derivation: its own licence when
    /// reused, the producing licences of the entry it reused, and its inputs'.
    pub provenance: BTreeSet<Triple>,
}

/// One revision: every demanded query's record.
#[derive(Debug, Clone)]
pub struct Revision {
    /// The lane it ran in.
    pub lane: Lane,
    /// The source bytes (read through [`Revision::source`]).
    source: Output,
    /// The records, in label order.
    pub records: BTreeMap<Label, Record>,
    /// Work units charged.
    pub work: u64,
    /// States in the explorations that actually ran (reuses explore nothing).
    pub explored_states: u64,
    /// The configuration the revision ran under: part of the audit basis, which
    /// the audit compares with the basis it derives itself. Private, with the
    /// source and the epoch, so the basis is what the engine ran under; the
    /// records stay public for defect-injection tests, and the audit compares them
    /// against its own recomputation rather than trusting them.
    limits: Limits,
    /// The semantic epoch every key pinned.
    semantic_epoch: String,
}

impl Revision {
    /// The source bytes the revision ran on, with their commitment.
    #[must_use]
    pub const fn source(&self) -> &Output {
        &self.source
    }

    /// The limits the revision ran under.
    #[must_use]
    pub const fn limits(&self) -> &Limits {
        &self.limits
    }

    /// The semantic epoch every key pinned.
    #[must_use]
    pub fn semantic_epoch(&self) -> &str {
        &self.semantic_epoch
    }

    /// The record of `label`.
    #[must_use]
    pub fn get(&self, label: &Label) -> Option<&Record> {
        self.records.get(label)
    }
}

/// A cached value, shared between revisions.
#[derive(Debug, Clone)]
enum Value {
    Ast(Rc<SourceFile>),
    Norm(Rc<NormModel>),
    Model(Rc<Model>),
    Explored(Rc<Result<Exploration, ExplorationError>>),
    /// The closed exploration that established domain safety, when there is one.
    Safety(Option<Rc<Result<Exploration, ExplorationError>>>),
    Opaque,
}

#[derive(Debug, Clone)]
struct Entry {
    output: Output,
    /// The full canonical inputs, in key order: a hit is admitted only when they
    /// equal the current inputs byte for byte.
    inputs: Vec<Rc<[u8]>>,
    /// The side inputs the result was computed under, in side-edge order.
    side: Vec<Rc<[u8]>>,
    value: Value,
    /// For a `parse` entry: the heuristic normalization of its source.
    normalized: Option<Rc<[u8]>>,
    /// The derivation class the result was produced under (the meet of its inputs'
    /// classes when it was computed). A later hit starts from this class, never
    /// from `Exact`: a cache entry cannot launder its origin (RFC 0030, "no in-place
    /// class upgrade").
    class: ReuseClass,
    /// Every licence triple on the derivation that produced the result. A hit is
    /// refused while any of them is quarantined, and quarantining one evicts it.
    provenance: BTreeSet<Triple>,
    generation: u64,
}

/// A computed result, before interning.
struct Computed {
    output: Vec<u8>,
    value: Value,
    memo: Memo,
}

impl Computed {
    const fn stored(output: Vec<u8>, value: Value) -> Self {
        Self {
            output,
            value,
            memo: Memo::Stored,
        }
    }
}

/// A reuse found by a licence: the outcome, the output, the value.
/// A reuse: the outcome, the output, the value, and the origin of the entry it came
/// from (its producing class and licence triples).
type Reuse = (Outcome, Output, Value, Origin);

/// Where a cached result came from.
#[derive(Debug, Clone)]
struct Origin {
    class: ReuseClass,
    provenance: BTreeSet<Triple>,
}

/// The engine.
#[derive(Debug)]
pub struct Engine {
    config: Config,
    cache: BTreeMap<QueryKey, Entry>,
    /// Committed outputs by commitment, so an unchanged recomputed output shares
    /// the cached allocation and later comparisons are pointer comparisons.
    interned: BTreeMap<Digest256, Rc<[u8]>>,
    heuristic_index: BTreeMap<Vec<u8>, QueryKey>,
    quarantine: BTreeSet<Triple>,
    last_safety: Option<QueryKey>,
    generation: u64,
}

/// The staged, uncommitted state of one revision.
struct Staging {
    lane: Lane,
    entries: BTreeMap<QueryKey, Entry>,
    interned: BTreeMap<Digest256, Rc<[u8]>>,
    touched: BTreeSet<QueryKey>,
    records: BTreeMap<Label, Record>,
    values: BTreeMap<Label, Value>,
    meter: Meter,
    explored_states: u64,
    safety_key: Option<QueryKey>,
}

/// The budget-dependent failure codes of the front end: returned, never memoized.
const BUDGET_CODES: [&str; 4] = [
    "cml.limit.elaboration_too_large",
    "cml.limit.work_limit_exceeded",
    "cml.lower.output_too_large",
    "cml.lower.work_limit_exceeded",
];

fn memo_for(code: &str) -> Memo {
    if BUDGET_CODES.contains(&code) {
        Memo::NotMemoized("budget-dependent failure")
    } else {
        Memo::Stored
    }
}

/// The `Experimental` heuristic: drop `//` to end of line, collapse every
/// whitespace run (newlines included) to one space. Deliberately unsound; see the
/// module documentation. Linear in the source.
#[must_use]
pub fn heuristic_normalize(source: &str) -> Vec<u8> {
    let mut out = Vec::with_capacity(source.len());
    for line in source.lines() {
        let code = line.split("//").next().unwrap_or("");
        for word in code.split_whitespace() {
            if !out.is_empty() {
                out.push(b' ');
            }
            out.extend_from_slice(word.as_bytes());
        }
    }
    out
}

/// The canonical `strategy_config` of the bound-taking definitions.
fn strategy(bounds: Bounds) -> Vec<u8> {
    let mut out = b"bounds/1".to_vec();
    out.extend_from_slice(&(bounds.states() as u64).to_be_bytes());
    out.extend_from_slice(&(bounds.depth() as u64).to_be_bytes());
    out.extend_from_slice(&bounds.transitions().to_be_bytes());
    out
}

/// The size weight of one model evaluation step: the model's canonical identity,
/// eight bytes a unit, at least one.
fn model_units(model: &Output) -> u64 {
    usize_units(model.bytes().len() / 8).saturating_add(1)
}

fn usize_units(n: usize) -> u64 {
    u64::try_from(n).unwrap_or(u64::MAX)
}

/// Byte equality of two shared encodings, charged by length unless they are one
/// allocation.
fn same(meter: &mut Meter, a: &Rc<[u8]>, b: &Rc<[u8]>) -> Result<bool, Refusal> {
    if Rc::ptr_eq(a, b) {
        return Ok(true);
    }
    if a.len() != b.len() {
        return Ok(false);
    }
    meter.bytes(a.len(), "compare canonical bytes")?;
    Ok(a == b)
}

impl Engine {
    /// A cold engine.
    #[must_use]
    pub fn new(config: Config) -> Self {
        Self {
            config,
            cache: BTreeMap::new(),
            interned: BTreeMap::new(),
            heuristic_index: BTreeMap::new(),
            quarantine: BTreeSet::new(),
            last_safety: None,
            generation: 0,
        }
    }

    /// The configuration.
    #[must_use]
    pub const fn config(&self) -> &Config {
        &self.config
    }

    /// The number of cached entries.
    #[must_use]
    pub fn cached_entries(&self) -> usize {
        self.cache.len()
    }

    /// Quarantine a triple (RFC 0030, downgrade rule 1). Nothing in this crate
    /// clears one: clearing needs clean evidence the spike does not model.
    pub fn quarantine(&mut self, triple: Triple) {
        self.quarantine.insert(triple);
        // A result produced under the triple is re-derived, never served again: its
        // entry is evicted, whatever licence a later hit would present.
        self.cache
            .retain(|_, entry| !entry.provenance.contains(&triple));
        let cache = &self.cache;
        self.heuristic_index
            .retain(|_, key| cache.contains_key(key));
        if let Some(key) = &self.last_safety
            && !self.cache.contains_key(key)
        {
            self.last_safety = None;
        }
    }

    /// The quarantined triples, in order.
    #[must_use]
    pub fn quarantined(&self) -> Vec<Triple> {
        self.quarantine.iter().copied().collect()
    }

    /// Compute one revision of `source` in `lane`.
    ///
    /// # Errors
    ///
    /// Every arm of [`Refusal`]. A refusal changes nothing in the engine.
    pub fn revise(&mut self, source: &str, lane: Lane) -> Result<Revision, Refusal> {
        let max = self.config.limits.source_bytes;
        if source.len() > max {
            return Err(Refusal::SourceTooLarge {
                bytes: source.len(),
                max,
            });
        }
        let mut st = Staging {
            lane,
            entries: BTreeMap::new(),
            interned: BTreeMap::new(),
            touched: BTreeSet::new(),
            records: BTreeMap::new(),
            values: BTreeMap::new(),
            meter: Meter {
                left: self.config.limits.work,
                used: 0,
            },
            explored_states: 0,
            safety_key: None,
        };
        let source_output = self.intern(&mut st, source.as_bytes().to_vec())?;
        self.pipeline(source, &source_output, &mut st)?;
        let revision = Revision {
            lane,
            source: source_output,
            records: std::mem::take(&mut st.records),
            work: st.meter.used,
            explored_states: st.explored_states,
            limits: self.config.limits,
            semantic_epoch: self.config.semantic_epoch.clone(),
        };
        self.commit(st);
        Ok(revision)
    }

    fn commit(&mut self, st: Staging) {
        self.generation = self.generation.saturating_add(1);
        let generation = self.generation;
        for key in &st.touched {
            if let Some(entry) = self.cache.get_mut(key) {
                entry.generation = generation;
            }
        }
        for (digest, bytes) in st.interned {
            self.interned.entry(digest).or_insert(bytes);
        }
        for (key, mut entry) in st.entries {
            entry.generation = generation;
            if let Some(normalized) = &entry.normalized {
                self.heuristic_index
                    .insert(normalized.to_vec(), key.clone());
            }
            self.cache.insert(key, entry);
        }
        if st.safety_key.is_some() {
            self.last_safety = st.safety_key;
        }
        // Keep the current and the previous revision's entries. Eviction costs
        // recomputation, never correctness (RFC 0030, "GC interaction").
        let floor = generation.saturating_sub(1);
        self.cache.retain(|_, entry| entry.generation >= floor);
        let cache = &self.cache;
        self.heuristic_index
            .retain(|_, key| cache.contains_key(key));
        let live: BTreeSet<Digest256> = cache.values().map(|entry| entry.output.digest).collect();
        self.interned.retain(|digest, _| live.contains(digest));
        if let Some(key) = &self.last_safety
            && !self.cache.contains_key(key)
        {
            self.last_safety = None;
        }
    }

    /// Intern one output: hash it (charged by length), and share an equal committed
    /// or staged allocation when one exists.
    fn intern(&self, st: &mut Staging, bytes: Vec<u8>) -> Result<Output, Refusal> {
        st.meter.bytes(bytes.len(), "hash output")?;
        let digest = Blake3Hasher::hash(&bytes);
        let known = st
            .interned
            .get(&digest)
            .or_else(|| self.interned.get(&digest))
            .cloned();
        if let Some(shared) = known {
            st.meter.bytes(bytes.len(), "compare interned output")?;
            if *shared == *bytes {
                return Ok(Output {
                    bytes: shared,
                    digest,
                });
            }
            // A digest collision: keep the bytes apart; equality stays canonical.
            return Ok(Output {
                bytes: Rc::from(bytes),
                digest,
            });
        }
        let shared: Rc<[u8]> = Rc::from(bytes);
        st.interned.insert(digest, Rc::clone(&shared));
        Ok(Output {
            bytes: shared,
            digest,
        })
    }

    /// Quarantine and lane admission for one licence.
    fn admit(
        &self,
        lane: Lane,
        definition: &'static QueryDefinition,
        licence: ReuseClass,
        evidence: Option<EvidenceForm>,
        provenance: &BTreeSet<Triple>,
    ) -> Result<Outcome, Cause> {
        let triple = Triple {
            class: licence,
            function_id: definition.function_id,
            function_version: definition.function_version,
        };
        // Quarantine is checked against the producing triples too, not only the
        // licence presented now: a result derived under a quarantined licence
        // drops to `Experimental` through any later edge, exactly as a
        // quarantined licence does (downgrade rule 1).
        // A quarantined producing triple is handled as a quarantined licence: the
        // reuse drops to `Experimental` (downgrade rule 1).
        let origin = provenance
            .iter()
            .find(|origin| self.quarantine.contains(origin))
            .copied();
        let quarantined = origin.is_some() || self.quarantine.contains(&triple);
        let named = origin.unwrap_or(triple);
        // A quarantined `Experimental` triple serves no reuse at all: its reuse
        // cannot drop any further, and the heuristic that failed is not retried.
        if quarantined && licence == ReuseClass::Experimental {
            return Err(Cause::Quarantined(named));
        }
        let effective = if quarantined {
            ReuseClass::Experimental
        } else {
            licence
        };
        match lane.admits(effective) {
            Admission::Admitted => Ok(Outcome::Reused {
                licence: effective,
                evidence: if quarantined { None } else { evidence },
                triple,
                assumptions: Vec::new(),
            }),
            Admission::Refused if quarantined => Err(Cause::Quarantined(named)),
            Admission::Refused => Err(Cause::LaneRefused(effective)),
        }
    }

    /// The one reuse decision on a key hit: exact input comparison, then the side
    /// inputs to a licence, then quarantine and lane. `Ok(None)` when there is no
    /// admissible hit (no entry, or a commitment collision).
    fn decide(
        &self,
        st: &mut Staging,
        definition: &'static QueryDefinition,
        key: &QueryKey,
        inputs: &[&Output],
        side: &[&Output],
    ) -> Result<Option<Result<Reuse, Cause>>, Refusal> {
        let Some(entry) = st.entries.get(key).or_else(|| self.cache.get(key)).cloned() else {
            return Ok(None);
        };
        if entry.inputs.len() != inputs.len() || entry.side.len() != side.len() {
            return Ok(None);
        }
        for (then, now) in entry.inputs.iter().zip(inputs) {
            if !same(&mut st.meter, then, &now.bytes)? {
                return Ok(None);
            }
        }
        // The licence is the edge's own: `Exact`, met with the class of every side
        // edge whose input differs. The entry's producing class is kept apart (it
        // enters the record's derivation class and the lane check, not the
        // quarantine triple), so a quarantine reaches exactly the reuses of its
        // edge class (RFC 0030, "Quarantine").
        let mut licence = ReuseClass::Exact;
        let mut evidence = EvidenceForm::ContentIdentity;
        let mut assumptions = Vec::new();
        let side_edges = definition
            .edges
            .iter()
            .filter(|edge| edge.keyed == Keyed::Side);
        for ((edge, then), now) in side_edges.zip(&entry.side).zip(side) {
            if !same(&mut st.meter, then, &now.bytes)? {
                licence = licence.meet(edge.class);
                evidence = EvidenceForm::ConservativeDependencyTheorem;
                assumptions.push(edge.assumption);
            }
        }
        // The entry is never refreshed to the new side inputs: the comparison is
        // always against what the result was computed under, so a Conservative
        // reuse stays Conservative (with its assumption) until it is recomputed.
        st.touched.insert(key.clone());
        // A result produced under an `Experimental` derivation is never served to
        // the promotion lane, whatever licence the hit presents now.
        if st.lane.admits(entry.class) == Admission::Refused {
            return Ok(Some(Err(Cause::LaneRefused(entry.class))));
        }
        let decision = self
            .admit(
                st.lane,
                definition,
                licence,
                Some(evidence),
                &entry.provenance,
            )
            .map(|outcome| match outcome {
                Outcome::Reused {
                    licence,
                    evidence,
                    triple,
                    ..
                } => Outcome::Reused {
                    licence,
                    evidence,
                    triple,
                    assumptions,
                },
                other => other,
            });
        Ok(Some(decision.map(|outcome| {
            (
                outcome,
                entry.output.clone(),
                entry.value.clone(),
                Origin {
                    class: entry.class,
                    provenance: entry.provenance.clone(),
                },
            )
        })))
    }

    fn edges(definition: &'static QueryDefinition, name: &str) -> Vec<EdgeUse> {
        definition
            .edges
            .iter()
            .map(|spec| EdgeUse {
                from: match spec.from {
                    Stage::ProjectInvariant => Label::named(Stage::ProjectInvariant, name),
                    stage => Label::of(stage),
                },
                spec: *spec,
            })
            .collect()
    }

    /// Demand one query: reuse through a licence, or compute. `alternate` is the
    /// stage-specific licence tried when there is no admissible key hit.
    #[allow(clippy::too_many_arguments)]
    fn demand(
        &self,
        st: &mut Staging,
        label: Label,
        inputs: &[&Output],
        strategy: &[u8],
        side: &[&Output],
        alternate: impl FnOnce(&Self, &mut Staging) -> Result<Option<Result<Reuse, Cause>>, Refusal>,
        compute: impl FnOnce(&mut Staging) -> Result<Computed, Refusal>,
    ) -> Result<(Output, QueryKey), Refusal> {
        let Some(definition) = label.stage.definition() else {
            return Err(Refusal::Unregistered(label.stage));
        };
        if st.records.contains_key(&label) {
            return Err(Refusal::DuplicateLabel(label));
        }
        let key_name = if label.stage == Stage::ProjectInvariant {
            label.name.as_str()
        } else {
            ""
        };
        let digests: Vec<Digest256> = inputs.iter().map(|input| input.digest).collect();
        let key_len = QueryKey::encoded_len(
            definition,
            key_name,
            &digests,
            &self.config.semantic_epoch,
            strategy,
        );
        st.meter
            .bytes(key_len.saturating_mul(2), "build and hash key")?;
        let key = QueryKey::new(
            definition,
            key_name,
            &digests,
            &self.config.semantic_epoch,
            strategy,
        );
        let edges = Self::edges(definition, &label.name);
        let inputs_class = edges
            .iter()
            .filter_map(|edge| st.records.get(&edge.from).map(|record| record.class))
            .fold(ReuseClass::Exact, ReuseClass::meet);
        let mut provenance: BTreeSet<Triple> = edges
            .iter()
            .filter_map(|edge| st.records.get(&edge.from))
            .flat_map(|record| record.provenance.iter().copied())
            .collect();

        let decided = match self.decide(st, definition, &key, inputs, side)? {
            Some(decision) => decision,
            None => match alternate(self, st)? {
                Some(Ok(reuse)) => Ok(reuse),
                Some(Err(cause @ Cause::WitnessRefused(_))) => Err(cause),
                Some(Err(cause)) => Err(Cause::AlternateRefused(Box::new(cause))),
                None => Err(Cause::KeyMiss),
            },
        };
        let mut origin_class = ReuseClass::Exact;
        // Only a recomputation is stored. A reuse of any class keeps the entry it
        // came from, with that entry's own origin; a `Validated` or `Experimental`
        // reuse is never written under the current key, so no later hit can
        // present it as a fresh computation.
        let (outcome, output, value) = match decided {
            Ok((outcome, output, value, origin)) => {
                origin_class = origin.class;
                provenance.extend(origin.provenance);
                if let Outcome::Reused { triple, .. } = &outcome {
                    provenance.insert(*triple);
                }
                (outcome, output, value)
            }
            Err(cause) => {
                let computed = compute(st)?;
                let output = self.intern(st, computed.output)?;
                if computed.memo == Memo::Stored {
                    st.entries.insert(
                        key.clone(),
                        Entry {
                            output: output.clone(),
                            inputs: inputs.iter().map(|input| Rc::clone(&input.bytes)).collect(),
                            side: side.iter().map(|input| Rc::clone(&input.bytes)).collect(),
                            value: computed.value.clone(),
                            normalized: None,
                            class: inputs_class,
                            provenance: provenance.clone(),
                            generation: 0,
                        },
                    );
                }
                (
                    Outcome::Recomputed {
                        cause,
                        memo: computed.memo,
                    },
                    output,
                    computed.value,
                )
            }
        };
        let class = match &outcome {
            Outcome::Reused { licence, .. } => inputs_class.meet(*licence).meet(origin_class),
            Outcome::Recomputed { .. } => inputs_class,
        };
        st.records.insert(
            label.clone(),
            Record {
                label: label.clone(),
                definition,
                handle: key.handle_for(&label),
                output: output.clone(),
                outcome,
                class,
                edges,
                provenance,
            },
        );
        st.values.insert(label, value);
        Ok((output, key))
    }

    fn value(st: &Staging, label: &Label) -> Option<Value> {
        st.values.get(label).cloned()
    }

    #[allow(clippy::too_many_lines)]
    fn pipeline(
        &self,
        source: &str,
        source_output: &Output,
        st: &mut Staging,
    ) -> Result<(), Refusal> {
        let limits = self.config.limits;
        let bounds_config = strategy(limits.explore);

        // parse -----------------------------------------------------------------
        st.meter.bytes(source.len(), "normalize source")?;
        let normalized = heuristic_normalize(source);
        let parse = Label::of(Stage::Parse);
        let (parse_out, parse_key) = self.demand(
            st,
            parse.clone(),
            &[source_output],
            b"",
            &[],
            |engine, st| engine.experimental_parse(st, &normalized),
            |st| {
                st.meter.bytes(source.len(), "parse")?;
                Ok(match continuum_cml_syntax::parse(source) {
                    Ok(file) => {
                        // The dump indents up to two columns per nesting level on
                        // every line, so it can outgrow the source by the nesting
                        // bound; that bound is charged before the dump is built.
                        st.meter.bytes(
                            source
                                .len()
                                .saturating_mul(2 * continuum_cml_syntax::MAX_NESTING as usize + 2),
                            "dump syntax tree",
                        )?;
                        let dump = continuum_cml_syntax::dump::dump_file(&file);
                        Computed::stored(output::parse_ok(&dump), Value::Ast(Rc::new(file)))
                    }
                    Err(error) => Computed::stored(output::parse_err(&error), Value::Opaque),
                })
            },
        )?;
        if let Some(entry) = st.entries.get_mut(&parse_key) {
            entry.normalized = Some(Rc::from(normalized));
        }
        let Some(Value::Ast(ast)) = Self::value(st, &parse) else {
            return Ok(());
        };

        // elaborate -------------------------------------------------------------
        let elaborate = Label::of(Stage::Elaborate);
        let (elab_out, _) = self.demand(
            st,
            elaborate.clone(),
            &[&parse_out],
            b"",
            &[],
            |_, _| Ok(None),
            |st| {
                // The elaborator charges its own pre-charged budgets (`limits.elab`);
                // the engine charges the input it hands over.
                st.meter.bytes(parse_out.bytes().len(), "elaborate")?;
                let (result, usage) = continuum_cml_elab::elaborate_with(&ast, limits.elab);
                Ok(match result {
                    Ok(norm) => {
                        // The normalized identity is charged by its stated bound
                        // before it is built.
                        st.meter.bytes(
                            output::norm_identity_bound(usage.nodes, source.len()),
                            "normalized identity",
                        )?;
                        Computed::stored(output::elaborate_ok(&norm), Value::Norm(Rc::new(norm)))
                    }
                    Err(error) => Computed {
                        output: output::failed("elab", error.code()),
                        value: Value::Opaque,
                        memo: memo_for(error.code()),
                    },
                })
            },
        )?;
        let Some(Value::Norm(norm)) = Self::value(st, &elaborate) else {
            return Ok(());
        };

        // Every per-invariant query is counted before any exists.
        let needed = norm.invariants.len().saturating_mul(2).saturating_add(6);
        if needed > limits.queries {
            return Err(Refusal::TooManyQueries {
                needed,
                max: limits.queries,
            });
        }

        // build_model -----------------------------------------------------------
        let build = Label::of(Stage::BuildModel);
        let (build_out, _) = self.demand(
            st,
            build.clone(),
            &[&elab_out],
            b"",
            &[],
            |_, _| Ok(None),
            |st| {
                st.meter.bytes(elab_out.bytes().len(), "lower")?;
                let (result, _usage) = continuum_cml_elab::lower_with(&norm, limits.elab);
                match result {
                    Ok(model) => {
                        st.meter
                            .bytes(model.identity_alloc_bound(), "model identity")?;
                        let identity = model.identity();
                        Ok(Computed::stored(
                            output::model_ok(identity.as_bytes()),
                            Value::Model(Rc::new(model)),
                        ))
                    }
                    Err(error) => Ok(Computed {
                        output: output::failed("model", error.code()),
                        value: Value::Opaque,
                        memo: memo_for(error.code()),
                    }),
                }
            },
        )?;

        // project_transition_system ----------------------------------------------
        let ts = Label::of(Stage::ProjectTransitionSystem);
        let (ts_out, _) = self.demand(
            st,
            ts,
            &[&elab_out],
            b"",
            &[],
            |_, _| Ok(None),
            |st| {
                // The clone is charged by the normalized identity's length, the
                // elaborator's own measure of the model's size, before it is made.
                // A stated proxy, not a proof: every node of the normalized model
                // renders at least one byte of its identity, so the model holds at
                // most identity-length nodes, and each node allocates at most 128
                // bytes (`size_of` of an expression node is 104 today). The clone
                // is charged that, and the projection's identity (no larger than
                // the full identity) its length, before either is built.
                st.meter.bytes(
                    elab_out.bytes().len().saturating_mul(128 + 1),
                    "clone and identify normalized model",
                )?;
                let mut projected = NormModel::clone(&norm);
                projected.invariants.clear();
                Ok(Computed::stored(
                    output::transition_system(&projected),
                    Value::Opaque,
                ))
            },
        )?;

        // project_invariant -------------------------------------------------------
        // One charge for every projection. The normalized identity writes a bound
        // variable as a de Bruijn index and a projection writes its name, which is
        // at most `MAX_IDENT_BYTES` (128) bytes, so together the projections are at
        // most 128 times the identity they are cut from.
        st.meter.bytes(
            elab_out.bytes().len().saturating_mul(128),
            "project invariants",
        )?;
        let mut projections: Vec<(String, Output)> = Vec::with_capacity(norm.invariants.len());
        for invariant in &norm.invariants {
            let label = Label::named(Stage::ProjectInvariant, &invariant.name);
            let (out, _) = self.demand(
                st,
                label,
                &[&elab_out],
                b"",
                &[],
                |_, _| Ok(None),
                |_| {
                    Ok(Computed::stored(
                        output::invariant_projection(invariant),
                        Value::Opaque,
                    ))
                },
            )?;
            projections.push((invariant.name.clone(), out));
        }

        // Exploration and checking need a lowered model.
        let Some(Value::Model(model)) = Self::value(st, &build) else {
            return Ok(());
        };
        // The model's definedness predicates, classified by the whole-chain rule
        // (bn-24a5c, `continuum_model_core::definedness::Definedness`, deepest
        // first, fail closed on a gap or a name collision with an action): the
        // one classifier engine-reference checks against, so a nested chain, a
        // gap, or a collision reads the same way on both sides (bn-1eoco). Its
        // own doc bounds the cost at `O((p + a) log (p + a))` times the chain
        // depth (at most `MAX_IDENT_BYTES / 8`, 16): charged first, one
        // comparison-weight per predicate over that depth and the lookup levels,
        // plus building `action_names` — up to two entries per action (its own
        // name and, for an instance, its schema), each an ordered-set insertion
        // of `levels` comparisons (Codex: a flat `action_count * 128` undercounts
        // this once `action_count` alone exceeds the predicate count).
        let predicate_count = model.predicates().len();
        let action_count = model.actions().len();
        let levels =
            u64::from(usize_units(predicate_count.saturating_add(action_count).max(2)).ilog2() + 1);
        const CHAIN_DEPTH_BOUND: u64 = 128 / 8; // MAX_IDENT_BYTES / 8
        const BYTE_COMPARISON_UNIT: u64 = 128 / 8;
        st.meter.charge(
            usize_units(predicate_count)
                .saturating_mul(CHAIN_DEPTH_BOUND)
                .saturating_mul(levels)
                .saturating_mul(BYTE_COMPARISON_UNIT)
                .saturating_add(
                    usize_units(action_count)
                        .saturating_mul(2)
                        .saturating_mul(levels)
                        .saturating_mul(BYTE_COMPARISON_UNIT),
                ),
            "action definedness guards",
        )?;
        let definedness = Definedness::of(&model);
        let action_guard_count: usize = definedness.action_chains().map(<[usize]>::len).sum();

        // explore ---------------------------------------------------------------
        let explore = Label::of(Stage::Explore);
        let (explore_out, _) = self.demand(
            st,
            explore.clone(),
            &[&ts_out],
            &bounds_config,
            &[&build_out],
            |_, _| Ok(None),
            |st| {
                let bounds = limits.explore;
                // Every step the declared bounds admit (a state or a transition)
                // evaluates model expressions, so each is weighted by the model's
                // size: its canonical identity, eight bytes a unit.
                st.meter.charge(
                    usize_units(bounds.states())
                        .saturating_add(bounds.transitions())
                        .saturating_mul(model_units(&build_out)),
                    "explore (declared bounds)",
                )?;
                let result = bfs::explore(&model, bounds);
                if let Ok(exploration) = &result {
                    st.explored_states = st
                        .explored_states
                        .saturating_add(usize_units(exploration.reachable().len()));
                    // The encoding writes each state and its predecessor (arity
                    // values of eight bytes each), a depth and an action: charged
                    // before it is built.
                    let states = exploration
                        .reachable()
                        .len()
                        .saturating_add(match exploration {
                            Exploration::Exhausted(partial) => partial.frontier().len(),
                            Exploration::Complete(_) => 0,
                        });
                    st.meter.bytes(
                        states.saturating_mul(model.arity().saturating_mul(16).saturating_add(64)),
                        "encode exploration",
                    )?;
                    // The encoding recomputes every expanded state's successor row
                    // and writes each transition (an action and a target) before
                    // hashing them: the rows' evaluations and bytes, charged first.
                    let rows = usize_units(exploration.reachable().len())
                        .saturating_mul(usize_units(model.actions().len()).saturating_add(1))
                        .saturating_mul(model_units(&build_out))
                        .saturating_add(exploration.reachable().transitions().saturating_mul(
                            usize_units(model.arity().saturating_mul(8).saturating_add(16)),
                        ));
                    st.meter.charge(rows, "encode successor rows")?;
                }
                Ok(Computed::stored(
                    output::exploration(&result, &model),
                    Value::Explored(Rc::new(result)),
                ))
            },
        )?;
        let Some(Value::Explored(explored)) = Self::value(st, &explore) else {
            return Ok(());
        };

        // domain_safety -----------------------------------------------------------
        let safety = Label::of(Stage::DomainSafety);
        let (_, safety_key) = self.demand(
            st,
            safety,
            &[&ts_out, &explore_out],
            &bounds_config,
            &[&build_out],
            |engine, st| engine.validated_safety(st, &model, &build_out, &definedness),
            |st| {
                let verdict = match explored.as_ref() {
                    Ok(exploration) => {
                        let reachable = exploration.reachable();
                        charge_guards(st, reachable.len(), action_guard_count, &build_out)?;
                        match first_undefined_action(
                            &model,
                            &definedness,
                            reachable.states(),
                            reachable.depths(),
                        ) {
                            ActionRead::Error(message) => DomainSafety::Evaluation(message),
                            ActionRead::Undefined {
                                action,
                                depth,
                                state,
                            } => DomainSafety::UndefinedAction {
                                action,
                                depth,
                                state,
                            },
                            ActionRead::Defined => match exploration {
                                Exploration::Complete(_) => DomainSafety::Established,
                                Exploration::Exhausted(partial) => {
                                    DomainSafety::Inconclusive(partial.tripped())
                                }
                            },
                        }
                    }
                    Err(error) => DomainSafety::ExplorationFailed(error.to_string()),
                };
                let witness =
                    matches!(verdict, DomainSafety::Established).then(|| Rc::clone(&explored));
                Ok(Computed::stored(
                    output::domain_safety(&verdict),
                    Value::Safety(witness),
                ))
            },
        )?;
        // A recomputed result is stored under its key and becomes the next witness
        // source; a Validated reuse is not stored, so the witness source it used
        // stays the source.
        st.safety_key =
            if st.entries.contains_key(&safety_key) || self.cache.contains_key(&safety_key) {
                Some(safety_key)
            } else {
                self.last_safety.clone()
            };

        // check_invariant ---------------------------------------------------------
        for (name, projection) in &projections {
            let label = Label::named(Stage::CheckInvariant, name);
            self.demand(
                st,
                label,
                &[projection, &ts_out, &explore_out],
                b"",
                &[&build_out],
                |_, _| Ok(None),
                |st| {
                    charge_guards(
                        st,
                        explored
                            .as_ref()
                            .as_ref()
                            .map_or(0, |e| e.reachable().len()),
                        action_guard_count,
                        &build_out,
                    )?;
                    let verdict = check(
                        st,
                        &model,
                        name,
                        projection.bytes().len(),
                        &definedness,
                        &explored,
                    )?;
                    Ok(Computed::stored(
                        output::invariant_verdict(&verdict),
                        Value::Opaque,
                    ))
                },
            )?;
        }
        Ok(())
    }

    /// The `Experimental` licence of `parse`: the heuristic normalization matches a
    /// committed parse entry. Interactive lane only.
    fn experimental_parse(
        &self,
        st: &mut Staging,
        normalized: &[u8],
    ) -> Result<Option<Result<Reuse, Cause>>, Refusal> {
        // The promotion lane admits no `Experimental` reuse, so it never looks.
        if self.config.heuristic == Heuristic::Disabled || st.lane == Lane::Promotion {
            return Ok(None);
        }
        st.meter.bytes(normalized.len(), "heuristic lookup")?;
        let Some(key) = self.heuristic_index.get(normalized) else {
            return Ok(None);
        };
        let Some(entry) = self.cache.get(key) else {
            return Ok(None);
        };
        let definition = Stage::Parse
            .definition()
            .ok_or(Refusal::Unregistered(Stage::Parse))?;
        let decision = self.admit(
            st.lane,
            definition,
            ReuseClass::Experimental,
            None,
            &entry.provenance,
        );
        if decision.is_ok() {
            st.touched.insert(key.clone());
        }
        Ok(Some(decision.map(|outcome| {
            (
                outcome,
                entry.output.clone(),
                entry.value.clone(),
                Origin {
                    class: entry.class,
                    provenance: entry.provenance.clone(),
                },
            )
        })))
    }

    /// The `Validated` licence of `domain_safety`: the previous closed state set,
    /// certified for the current model and verified by the independent kernel.
    fn validated_safety(
        &self,
        st: &mut Staging,
        model: &Model,
        build_out: &Output,
        definedness: &Definedness,
    ) -> Result<Option<Result<Reuse, Cause>>, Refusal> {
        let Some(key) = &self.last_safety else {
            return Ok(None);
        };
        let Some(entry) = self.cache.get(key) else {
            return Ok(None);
        };
        let refused = |why| Ok(Some(Err(Cause::WitnessRefused(why))));
        let Value::Safety(Some(prior)) = &entry.value else {
            return refused(WitnessRefusal::NoPriorClosedSet);
        };
        let Ok(exploration) = prior.as_ref() else {
            return refused(WitnessRefusal::NoPriorClosedSet);
        };
        let Some(closed) = ClosedSet::of(exploration) else {
            return refused(WitnessRefusal::NoPriorClosedSet);
        };
        let reachable = closed.reachable();
        // The closure certificate says nothing about definedness: a witness set
        // that holds a state where some action's read is undefined licenses
        // nothing (every reachable state is in the set, so a set with none such
        // guarantees a clean run meets none).
        let action_guard_count: usize = definedness.action_chains().map(<[usize]>::len).sum();
        charge_guards(st, reachable.len(), action_guard_count, build_out)?;
        match first_undefined_action(model, definedness, reachable.states(), reachable.depths()) {
            ActionRead::Defined => {}
            ActionRead::Undefined { .. } => return refused(WitnessRefusal::UndefinedAction),
            ActionRead::Error(message) => return refused(WitnessRefusal::GuardEvaluation(message)),
        }
        // Emission recomputes every row of `S` under the current model, and the
        // kernel re-walks every one: both are charged before either runs.
        st.meter.charge(
            usize_units(reachable.len())
                .saturating_mul(usize_units(model.actions().len()).saturating_add(1))
                .saturating_mul(2)
                .saturating_mul(model_units(build_out)),
            "emit and check closure witness",
        )?;
        let model_digest = format!("blake3:{}", build_out.digest.to_token());
        let envelope = ClaimEnvelope {
            model_digest: &model_digest,
            semantic_epoch: &self.config.semantic_epoch,
            property_digest: "state-domain",
            scope_digest: "incremental-validated-reuse",
            assumptions_digest: "none",
            producer: PRODUCER,
            domain_pack_digests: &[],
        };
        let bytes = match continuum_engine_reference::certificate::emit_finite_closure(
            model, closed, &envelope,
        ) {
            Ok(bytes) => bytes,
            Err(error) => return refused(WitnessRefusal::Emission(error.to_string())),
        };
        // The identity the carried model must have: the current model's own,
        // charged by its allocation bound before it is built.
        st.meter.bytes(
            model.identity_alloc_bound(),
            "model identity for the witness",
        )?;
        let identity = model.identity();
        let claim = match check_witness(&bytes, &envelope, identity.as_bytes()) {
            Ok(claim) => claim,
            Err(why) => return refused(why),
        };
        // Every reachable state of the current model lies in the verified set, so a
        // clean breadth-first run discovers at most `states` states, reaches depth
        // at most `states - 1`, and walks at most `transitions` transitions.
        let bounds = self.config.limits.explore;
        let states = claim.states as usize;
        if states > bounds.states()
            || states > bounds.depth().saturating_add(1)
            || claim.transitions > bounds.transitions()
        {
            return refused(WitnessRefusal::OutsideBounds);
        }
        let definition = Stage::DomainSafety
            .definition()
            .ok_or(Refusal::Unregistered(Stage::DomainSafety))?;
        // The certificate covers closure within the state domain. The other half of
        // `Established`, action definedness over the witness set, is the engine's
        // own scan above; it is recorded as the assumption it is, never presented
        // as covered by the certificate.
        let decision = self
            .admit(
                st.lane,
                definition,
                ReuseClass::Validated,
                Some(EvidenceForm::CheckedCertificate),
                &entry.provenance,
            )
            .map(|outcome| match outcome {
                Outcome::Reused {
                    licence,
                    evidence,
                    triple,
                    ..
                } => Outcome::Reused {
                    licence,
                    evidence,
                    triple,
                    assumptions: vec![crate::query::ASSUMPTION_V1],
                },
                other => other,
            });
        let output = self.intern(st, output::domain_safety(&DomainSafety::Established))?;
        if decision.is_ok() {
            // The witness entry stays the next revision's witness source.
            st.touched.insert(key.clone());
        }
        Ok(Some(decision.map(|outcome| {
            (
                outcome,
                output,
                Value::Safety(Some(Rc::clone(prior))),
                // The witness source's producing class and licences both carry.
                Origin {
                    class: entry.class,
                    provenance: entry.provenance.clone(),
                },
            )
        })))
    }
}

/// What a verified finite-closure witness established, as the kernel counted it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WitnessClaim {
    /// Distinct states in the certificate's table.
    pub states: u32,
    /// Labelled transitions the kernel walked for the closure obligation.
    pub transitions: u64,
}

/// Check a finite-closure witness from its wire form through `continuum-certificate`
/// and bind the verified claim to the envelope the engine expected, field by field.
///
/// A kernel `Verified` says the bytes are an internally consistent closure
/// certificate relative to *their own* envelope and *their own* carried model; it
/// says nothing about which query they speak for. So:
///
/// - the claim must be a wire-epoch-2 claim that carries a model, and that model's
///   canonical identity (`continuum-model/1`) must equal `model_identity`, the
///   identity of the model the engine asked about, byte for byte (ADR-0013). At
///   epoch 2 the kernel re-derives every successor row from that carried model, so
///   this binds the closure to the engine's model's semantics, not only to a label;
/// - every envelope field — model digest, semantic epoch, property digest, scope,
///   assumptions, producer, schema epoch, and the domain-pack list — must equal the
///   expected one;
/// - the kind must be finite closure and the property class the state domain; any
///   class this licence does not handle (the invariant class included) is refused.
///
/// Otherwise the witness licenses nothing ([`WitnessRefusal::WrongClaim`] naming
/// the first binding that differs).
///
/// # Errors
///
/// [`WitnessRefusal::NotVerified`] when no kernel verified the bytes, and
/// [`WitnessRefusal::WrongClaim`] when the verified claim is not the expected one.
pub fn check_witness(
    bytes: &[u8],
    expected: &ClaimEnvelope<'_>,
    model_identity: &[u8],
) -> Result<WitnessClaim, WitnessRefusal> {
    let verdict = check_certificate(bytes);
    let CheckOutcome::Checked(KernelVerdict::Core(core)) = &verdict else {
        return Err(WitnessRefusal::NotVerified(format!("{verdict:?}")));
    };
    let Some(claim) = core.claim() else {
        return Err(WitnessRefusal::NotVerified(format!("{core:?}")));
    };
    if claim.kind() != CertificateKind::FiniteClosure {
        return Err(WitnessRefusal::WrongClaim("kind"));
    }
    // Only the state-domain class is this licence's claim: any other class (the
    // kernel may grow more) is not the expected claim, whatever the envelope's
    // property string says.
    if claim.property() != PropertyClass::StateDomain {
        return Err(WitnessRefusal::WrongClaim("property"));
    }
    if claim.wire_epoch() != WITNESS_SCHEMA_EPOCH {
        return Err(WitnessRefusal::WrongClaim("wire_epoch"));
    }
    if claim.model_identity() != Some(model_identity) {
        return Err(WitnessRefusal::WrongClaim("model_identity"));
    }
    let got = claim.envelope();
    let fields: [(&'static str, &str, &str); 6] = [
        (
            "model_digest",
            got.model_digest().as_str(),
            expected.model_digest,
        ),
        (
            "semantic_epoch",
            got.semantic_epoch().as_str(),
            expected.semantic_epoch,
        ),
        (
            "property_digest",
            got.property_digest().as_str(),
            expected.property_digest,
        ),
        (
            "scope_digest",
            got.scope_digest().as_str(),
            expected.scope_digest,
        ),
        (
            "assumptions_digest",
            got.assumptions_digest().as_str(),
            expected.assumptions_digest,
        ),
        ("producer", got.producer().as_str(), expected.producer),
    ];
    if let Some((field, _, _)) = fields.iter().find(|(_, got, want)| got != want) {
        return Err(WitnessRefusal::WrongClaim(field));
    }
    if got.schema_epoch() != WITNESS_SCHEMA_EPOCH {
        return Err(WitnessRefusal::WrongClaim("schema_epoch"));
    }
    let packs = got.domain_pack_digests();
    if packs.len() != expected.domain_pack_digests.len()
        || packs
            .iter()
            .zip(expected.domain_pack_digests)
            .any(|(got, want)| got.as_str() != *want)
    {
        return Err(WitnessRefusal::WrongClaim("domain_pack_digests"));
    }
    Ok(WitnessClaim {
        states: claim.states(),
        transitions: claim.transitions(),
    })
}

/// The certificate schema epoch the witness path expects: the wire epoch the
/// reference emitter writes.
const WITNESS_SCHEMA_EPOCH: u16 = continuum_engine_reference::certificate::WIRE_EPOCH;

/// One invariant over one exploration: the first evaluation error in canonical
/// order, else the least failing state in (depth, canonical order), else `Holds`
/// for a closed run, else `Inconclusive`.
fn check(
    st: &mut Staging,
    model: &Model,
    name: &str,
    body_bytes: usize,
    definedness: &Definedness,
    explored: &Result<Exploration, ExplorationError>,
) -> Result<InvariantVerdict, Refusal> {
    let Some(index) = model.predicate_index(name) else {
        return Ok(InvariantVerdict::NoPredicate);
    };
    // The invariant's own definedness chain, deepest first (cr-pt5h3a: a
    // predicate's own guard is an obligation of the same base, so no depth is
    // skipped). `#` is not a CML identifier character (`DEFINED_SUFFIX`'s doc), so
    // a declared invariant's name never itself ends in it; `index` is never a
    // chain member, only ever a chain's base — this function does not implement
    // the reference scanner's separate case for a *subject* that is itself a
    // guard (`continuum_engine_reference::definedness::scan`'s `is_guard`
    // branch), because `name` is always a declared CML invariant here, never a
    // `#defined` predicate's own name.
    let own_chain = definedness.guards_of(index);
    let exploration = match explored {
        Ok(exploration) => exploration,
        Err(error) => return Ok(InvariantVerdict::ExplorationFailed(error.to_string())),
    };
    let reachable = exploration.reachable();
    // Up to `own_chain.len() + 1` evaluations per state (the invariant's own
    // chain, then the invariant), each proportional to the invariant's size: the
    // canonical projection's length, eight bytes a unit, stands for it. The
    // action guards are charged by the caller (`charge_guards`).
    st.meter.charge(
        usize_units(reachable.len())
            .saturating_mul(usize_units(own_chain.len()).saturating_add(1))
            .saturating_mul(usize_units(body_bytes / 8).saturating_add(1)),
        "check invariant",
    )?;
    type At<'a> = Option<(usize, &'a continuum_engine_reference::model::State)>;
    let mut failing: At<'_> = None;
    let mut undefined: At<'_> = None;
    let mut undefined_action: Option<(usize, &continuum_engine_reference::model::State, usize)> =
        None;
    'states: for (state, depth) in reachable.states().iter().zip(reachable.depths()) {
        // The actions' reads first, whole chain, deepest member first: at a state
        // where one is undefined the model itself is in error, and nothing else
        // is evaluated there.
        for chain in definedness.action_chains() {
            for &guard in chain {
                match model.evaluate_predicate(guard, state) {
                    Ok(true) => {}
                    Ok(false) => {
                        if undefined_action.is_none_or(|(best, _, _)| *depth < best) {
                            undefined_action = Some((*depth, state, guard));
                        }
                        continue 'states;
                    }
                    Err(error) => return Ok(InvariantVerdict::Evaluation(error.to_string())),
                }
            }
        }
        // Then the invariant's own whole chain, deepest first: at an undefined
        // state its value is not a CML value, so it is neither evaluated nor
        // counted.
        for &guard in own_chain {
            match model.evaluate_predicate(guard, state) {
                Ok(true) => {}
                Ok(false) => {
                    if undefined.is_none_or(|(best, _)| *depth < best) {
                        undefined = Some((*depth, state));
                    }
                    continue 'states;
                }
                Err(error) => return Ok(InvariantVerdict::Evaluation(error.to_string())),
            }
        }
        match model.evaluate_predicate(index, state) {
            Ok(true) => {}
            Ok(false) => {
                if failing.is_none_or(|(best, _)| *depth < best) {
                    failing = Some((*depth, state));
                }
            }
            Err(error) => return Ok(InvariantVerdict::Evaluation(error.to_string())),
        }
    }
    // Precedence: an evaluation error (returned above, first in canonical order),
    // then an undefined action read, then an undefined invariant read, then a
    // violation, then `Holds` or `Inconclusive`.
    if let Some((depth, state, guard)) = undefined_action {
        return Ok(InvariantVerdict::UndefinedAction {
            action: definedness_name(model, guard),
            depth,
            state: state.clone(),
        });
    }
    Ok(match (undefined, failing, exploration) {
        (Some((depth, state)), _, _) => InvariantVerdict::Undefined {
            depth,
            state: state.clone(),
        },
        (None, Some((depth, state)), _) => InvariantVerdict::Violated {
            depth,
            state: state.clone(),
        },
        (None, None, Exploration::Complete(closed)) => InvariantVerdict::Holds {
            states: closed.len(),
        },
        (None, None, Exploration::Exhausted(partial)) => {
            InvariantVerdict::Inconclusive(partial.tripped())
        }
    })
}

/// Charge the action-guard evaluations over `states` states before they run: one
/// evaluation per guard per state, each weighted by the model's size.
fn charge_guards(
    st: &mut Staging,
    states: usize,
    guards: usize,
    model: &Output,
) -> Result<(), Refusal> {
    st.meter.charge(
        usize_units(states)
            .saturating_mul(usize_units(guards))
            .saturating_mul(model_units(model)),
        "action definedness",
    )
}

/// The declared action or predicate a false definedness-chain member names: the
/// base of its own chain (`continuum_model_core::definedness::definedness_base`),
/// which is the same for every member of one chain, whatever depth is false.
fn definedness_name(model: &Model, guard: usize) -> String {
    model
        .predicates()
        .get(guard)
        .map(|predicate| predicate.name().as_str())
        .and_then(definedness_base)
        .unwrap_or_default()
        .to_owned()
}

/// The first undefined action read over some states.
#[derive(Debug, Clone, PartialEq, Eq)]
enum ActionRead {
    /// Every guard holds at every state.
    Defined,
    /// The least (depth, canonical order) state where some action's whole
    /// definedness chain (deepest member first) has a false member, and the
    /// declared action or predicate its chain names.
    Undefined {
        action: String,
        depth: usize,
        state: continuum_engine_reference::model::State,
    },
    /// A guard could not be evaluated (the first such, in canonical order).
    Error(String),
}

fn first_undefined_action(
    model: &Model,
    definedness: &Definedness,
    states: &[continuum_engine_reference::model::State],
    depths: &[usize],
) -> ActionRead {
    let mut found: Option<(usize, usize, usize)> = None;
    'states: for (position, (state, depth)) in states.iter().zip(depths).enumerate() {
        for chain in definedness.action_chains() {
            for &guard in chain {
                match model.evaluate_predicate(guard, state) {
                    Ok(true) => {}
                    Ok(false) => {
                        if found.is_none_or(|(best, _, _)| *depth < best) {
                            found = Some((*depth, position, guard));
                        }
                        continue 'states;
                    }
                    Err(error) => return ActionRead::Error(error.to_string()),
                }
            }
        }
    }
    match found {
        Some((depth, position, guard)) => ActionRead::Undefined {
            action: definedness_name(model, guard),
            depth,
            state: states[position].clone(),
        },
        None => ActionRead::Defined,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DIEHARD: &str =
        include_str!("../../../notes/plan/corpus/tla-examples/ports/TV-009/DieHard.ctm");

    /// cr-1jv75r: a hit starts from the class its entry was produced under. An entry
    /// produced under an `Experimental` derivation is refused by the promotion lane
    /// and keeps the interactive record `Experimental`, whatever licence the hit
    /// presents.
    #[test]
    fn an_entry_keeps_its_producing_class() {
        let mut engine = Engine::new(Config::default());
        engine.revise(DIEHARD, Lane::Promotion).expect("fits");
        for entry in engine.cache.values_mut() {
            entry.class = ReuseClass::Experimental;
        }
        let promoted = engine.revise(DIEHARD, Lane::Promotion).expect("fits");
        let explore = promoted.get(&Label::of(Stage::Explore)).expect("demanded");
        assert!(
            matches!(
                explore.outcome,
                Outcome::Recomputed {
                    cause: Cause::LaneRefused(ReuseClass::Experimental),
                    ..
                }
            ),
            "{:?}",
            explore.outcome
        );

        let mut engine = Engine::new(Config::default());
        engine.revise(DIEHARD, Lane::Interactive).expect("fits");
        for entry in engine.cache.values_mut() {
            entry.class = ReuseClass::Experimental;
        }
        let previewed = engine.revise(DIEHARD, Lane::Interactive).expect("fits");
        for record in previewed.records.values() {
            assert_eq!(record.class, ReuseClass::Experimental, "{}", record.label);
        }
    }

    /// bn-1eoco: the whole-chain rule, differentially, against hand-built models —
    /// the only way to give this engine a chain deeper than depth 1, a gap, or a
    /// collision, since the CML front end never writes one (`#` is not a CML
    /// identifier character, and lowering never nests a `#defined` chain). Each
    /// case mirrors `continuum-engine-reference`'s own `tests/definedness_nested.rs`
    /// (cr-pt5h3a) and is checked against its `checking::check`, the reference
    /// oracle `tests/differential.rs` also compares against — not a re-derivation.
    mod chain_rule {
        use continuum_engine_reference::checking::{
            self, CheckOutcome, DeadlockPolicy, Obligations,
        };
        use continuum_engine_reference::{
            ActionDecl, BoolExpr, CmpOp, Guarded, IntExpr, ModelBuilder,
        };

        use super::*;

        fn x_is(value: i64) -> BoolExpr {
            BoolExpr::compare(CmpOp::Eq, IntExpr::var("x"), IntExpr::constant(value))
        }

        /// `x` steps 0 → 1 and stops. Extra predicates are added by `with`.
        fn model(with: &[(&str, BoolExpr)]) -> Model {
            let mut builder = ModelBuilder::new()
                .variable("x", 0, 1)
                .action(ActionDecl::deterministic(
                    "Step",
                    x_is(0),
                    vec![("x", IntExpr::constant(1))],
                ))
                .initial_state(&[("x", 0)]);
            for (name, body) in with {
                builder = builder.predicate(name, body.clone());
            }
            builder.build().expect("builds")
        }

        /// A `Staging` with a budget large enough that no test here trips it.
        fn staging() -> Staging {
            Staging {
                lane: Lane::Promotion,
                entries: BTreeMap::new(),
                interned: BTreeMap::new(),
                touched: BTreeSet::new(),
                records: BTreeMap::new(),
                values: BTreeMap::new(),
                meter: Meter {
                    left: u64::MAX / 2,
                    used: 0,
                },
                explored_states: 0,
                safety_key: None,
            }
        }

        /// This crate's own verdict for `name`, over `exploration` — the production
        /// code path (`check`), not a copy of it.
        fn subject_verdict(
            model: &Model,
            name: &str,
            exploration: Exploration,
        ) -> InvariantVerdict {
            let definedness = Definedness::of(model);
            let explored: Result<Exploration, ExplorationError> = Ok(exploration);
            let mut st = staging();
            check(&mut st, model, name, 8, &definedness, &explored).expect("charges")
        }

        /// The reference engine's own verdict for `name`, over the same exploration,
        /// converted the way `tests/differential.rs`'s oracle is.
        fn oracle_verdict(
            model: &Model,
            exploration: &Exploration,
            name: &str,
        ) -> InvariantVerdict {
            let index = model.predicate_index(name).expect("declared");
            let report = checking::check(
                model,
                exploration,
                &Obligations::new(DeadlockPolicy::Allowed).invariant(index),
            )
            .expect("declared");
            match report.invariant(index).expect("checked").outcome() {
                CheckOutcome::Holds { states } => InvariantVerdict::Holds { states: *states },
                CheckOutcome::Violated { state, depth, .. } => InvariantVerdict::Violated {
                    depth: *depth,
                    state: state.clone(),
                },
                CheckOutcome::Inconclusive(why) => panic!("{name}: {why:?}"),
                CheckOutcome::Undefined(undefined) => match undefined.read() {
                    Guarded::Action => InvariantVerdict::UndefinedAction {
                        action: undefined.subject().to_owned(),
                        depth: undefined.depth(),
                        state: undefined.state().clone(),
                    },
                    Guarded::Predicate(_) => InvariantVerdict::Undefined {
                        depth: undefined.depth(),
                        state: undefined.state().clone(),
                    },
                },
            }
        }

        /// Assert this crate's engine and the reference checker agree over
        /// `model`'s whole exploration, for every name in `names`.
        fn assert_agrees(model: &Model, names: &[&str]) {
            let exploration = bfs::explore(model, Bounds::CERTIFIABLE).expect("explores");
            for &name in names {
                let expected = oracle_verdict(model, &exploration, name);
                let got = subject_verdict(model, name, exploration.clone());
                assert_eq!(got, expected, "{name}");
            }
        }

        /// Codex's case: `I = true`, `I#defined = true`, `I#defined#defined` false at
        /// `x = 1`: this engine follows the whole chain, not just depth 1.
        ///
        /// Only `I` is checked here, not `I#defined` itself: the oracle's own
        /// `scan` treats a *subject* that is itself a guard specially (a false
        /// value there is `Undefined`, never `Violated`, its `is_guard` branch),
        /// which `check` does not implement — `name` is always a declared CML
        /// invariant, and `#` is not a CML identifier character, so `check` never
        /// receives a name that is itself a definedness predicate in production.
        #[test]
        fn a_false_guard_of_a_guard_is_undefined_in_the_engine_too() {
            let model = model(&[
                ("I", BoolExpr::constant(true)),
                ("I#defined", BoolExpr::constant(true)),
                ("I#defined#defined", x_is(0)),
            ]);
            assert_agrees(&model, &["I"]);
        }

        /// `Step#defined` holds, `Step#defined#defined` is false at `x = 1`: an
        /// undefined action read that dominates.
        #[test]
        fn a_false_guard_of_an_action_guard_is_undefined_action_in_the_engine_too() {
            let model = model(&[
                ("Ok", BoolExpr::constant(true)),
                ("Step#defined", BoolExpr::constant(true)),
                ("Step#defined#defined", x_is(0)),
            ]);
            assert_agrees(&model, &["Ok"]);
        }

        /// A gap (`I#defined#defined` with no `I#defined`) is not a well-formed
        /// guard of `I`: fail closed as an action read, in this engine too.
        #[test]
        fn a_chain_with_a_gap_fails_closed_in_the_engine_too() {
            let model = model(&[
                ("I", BoolExpr::constant(true)),
                ("J", BoolExpr::constant(true)),
                ("I#defined#defined", x_is(0)),
            ]);
            assert_agrees(&model, &["I", "J"]);
        }

        /// An action sharing a chain member's full name (`I#defined`) reads the
        /// whole chain as the action's, in this engine too.
        #[test]
        fn a_chain_through_an_action_name_fails_closed_in_the_engine_too() {
            let model = ModelBuilder::new()
                .variable("x", 0, 1)
                .action(ActionDecl::deterministic(
                    "I#defined",
                    x_is(0),
                    vec![("x", IntExpr::constant(1))],
                ))
                .predicate("I", BoolExpr::constant(true))
                .predicate("J", BoolExpr::constant(true))
                .predicate("I#defined#defined", x_is(0))
                .initial_state(&[("x", 0)])
                .build()
                .expect("builds");
            // Both `I` (the chain's own base) and `J` (unrelated): the malformed
            // chain invalidates every invariant's checking, not just `I`'s.
            assert_agrees(&model, &["I", "J"]);
        }

        /// A nested guard that cannot be evaluated is an evaluation error, in this
        /// engine too — reported before the shallower guard is read.
        #[test]
        fn a_nested_evaluation_error_is_reported_in_the_engine_too() {
            let overflow = BoolExpr::compare(
                CmpOp::Eq,
                IntExpr::times(IntExpr::constant(i64::MAX), IntExpr::constant(2)),
                IntExpr::constant(0),
            );
            let model = model(&[
                ("I", BoolExpr::constant(true)),
                ("I#defined", BoolExpr::constant(true)),
                ("I#defined#defined", overflow),
            ]);
            let exploration = bfs::explore(&model, Bounds::CERTIFIABLE).expect("explores");
            let got = subject_verdict(&model, "I", exploration.clone());
            assert!(matches!(got, InvariantVerdict::Evaluation(_)), "{got:?}");
            let index = model.predicate_index("I").unwrap();
            let report = checking::check(
                &model,
                &exploration,
                &Obligations::new(DeadlockPolicy::Allowed).invariant(index),
            )
            .expect("declared");
            assert!(
                matches!(
                    report.invariant(index).unwrap().outcome(),
                    CheckOutcome::Inconclusive(_)
                ),
                "{report}"
            );
        }
    }
}
