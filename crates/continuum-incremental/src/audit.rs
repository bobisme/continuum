//! The Incremental Parity Audit's clean side (INV-010, plan §9.5, RFC 0030).
//!
//! # Independence from the engine
//!
//! [`clean`] recomputes every compared artifact from the source text in one
//! straight line: parse, elaborate, lower the *whole* model, explore it, and check
//! each invariant against it. It has no cache, no key, no reuse class, no lane, no
//! quarantine, and no projection — the transition-system and invariant projections
//! that let the engine reuse an exploration across a property edit do not exist
//! here, so the audit tests assumptions A1 and A2 instead of repeating them. This
//! module imports nothing from [`crate::engine`] except the read-only [`Revision`]
//! and [`Record`] it compares against (`tests/independence.rs` pins that), and the
//! verdict loops below are written here, not shared.
//!
//! What it does share is [`crate::output`]: the canonical encoders are the
//! *definition* of each compared artifact (RFC 0030, "Compared artifacts": the
//! comparison is over the canonical encoding). A defect in an encoder would make
//! both sides agree on a weaker artifact; each encoder states what it keeps.
//!
//! It also shares `continuum_model_core::definedness::Definedness` (bn-1eoco): the
//! model's own whole-chain classification of its `#defined` predicates (RFC 0003,
//! "Definedness"; bn-24a5c). That is model semantics, not engine decision logic —
//! it names no cache, key, class, lane, or quarantine, and it is the same
//! classifier `continuum-engine-reference`'s checker reads (`tests/independence.rs`
//! admits it: the forbidden-identifier pin below names engine internals, not model
//! types). Calling it here, instead of re-deriving the chain rule, is what makes
//! "the audit agrees with the reference engine on chains" true by construction
//! rather than by two independent derivations staying in sync; the verdict loops
//! that *use* its classification — the per-state precedence over actions, an
//! invariant's own chain, then the invariant — are still written here, not shared
//! with [`crate::engine`].
//!
//! # What is compared
//!
//! The stages [`Stage::compared`] marks: the parse tree, the normalized model, the
//! canonical state graph, the domain-safety verdict, and each invariant verdict.
//! A label present on one side only is a mismatch, not a skip.
//!
//! # Outcomes are typed
//!
//! [`Parity`] and [`ConeSoundness`] distinguish every case; neither is a bare
//! boolean (INV-008).

use std::collections::{BTreeMap, BTreeSet};

use continuum_cml_elab::NormModel;
use continuum_engine_reference::bfs::{self, Exploration, ExplorationError};
use continuum_engine_reference::model::{Model, State};
use continuum_model_core::definedness::{Definedness, definedness_base};

use crate::engine::{Limits, Memo, Outcome, Record, Revision};
use crate::output::{self, DomainSafety, ExplorationStatus, InvariantVerdict};
use crate::query::{Compared, Label, Stage, Triple};
use crate::vocab::{Lane, ReuseClass};

/// The clean recomputation of one source: every compared artifact, by label,
/// whether the run completed, and the basis it ran on.
///
/// Built only by [`clean`], which derives the basis itself from its own arguments
/// (the source and the limits it computes under; the semantic epoch is a label the
/// caller asserts, since no definition here computes anything from it). The fields
/// are private, so no caller can assemble clean evidence or relabel it; the
/// revision's basis is private too, so the audit compares the basis each side
/// actually ran on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CleanRun {
    outputs: BTreeMap<Label, Vec<u8>>,
    explored_states: u64,
    completion: Completion,
    work: u64,
    basis: Basis,
}

/// The comparison basis of a clean run (RFC 0030 correction 14: the same source,
/// the same function versions, the same strategy configuration and limits, and the
/// same consumed epochs).
#[derive(Debug, Clone, PartialEq, Eq)]
struct Basis {
    source: Vec<u8>,
    limits: Limits,
    semantic_epoch: String,
}

/// The definition versions this audit implements, by compared stage. A revision
/// whose records name another version, or a compared stage this list does not name,
/// is not comparable with this audit. `tests/engine_classes.rs` pins it to the
/// registry.
pub const AUDITED_VERSIONS: [(Stage, &str); 5] = [
    (Stage::Parse, "1"),
    (Stage::Elaborate, "1"),
    (Stage::Explore, "1"),
    (Stage::DomainSafety, "2"),
    (Stage::CheckInvariant, "2"),
];

impl CleanRun {
    /// The canonical outputs produced before the run completed or stopped.
    #[must_use]
    pub const fn outputs(&self) -> &BTreeMap<Label, Vec<u8>> {
        &self.outputs
    }

    /// The reachable states of the one exploration that ran (zero if none did).
    #[must_use]
    pub const fn explored_states(&self) -> u64 {
        self.explored_states
    }

    /// Whether every compared artifact was produced within bounds.
    #[must_use]
    pub const fn completion(&self) -> &Completion {
        &self.completion
    }

    /// Work units the clean run charged against `Limits::work`.
    #[must_use]
    pub const fn work(&self) -> u64 {
        self.work
    }
}

/// Whether a run (clean or incremental) produced every compared artifact within
/// its bounds (INV-008: an incomplete run is a typed outcome, never a parity).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Completion {
    /// Every artifact the source defines was produced, and no bound decided one.
    Complete,
    /// The run stopped early, or a bound decided an artifact.
    Incomplete(Incompleteness),
}

/// Why a run is incomplete.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Incompleteness {
    /// The source is longer than `Limits::source_bytes`: refused before any work.
    SourceTooLarge {
        /// Its length.
        bytes: usize,
        /// The limit.
        max: usize,
    },
    /// A charge did not fit in what was left of `Limits::work`: refused before the
    /// work it would have paid for.
    WorkExhausted {
        /// Where.
        at: &'static str,
    },
    /// The model defines more queries than `Limits::queries`: refused before any
    /// per-invariant work.
    TooManyQueries {
        /// Defined.
        needed: usize,
        /// The limit.
        max: usize,
    },
    /// The front end stopped on its own budget (`Limits::elab`); the code.
    FrontEndBudget(&'static str),
    /// The exploration tripped a declared bound, so no verdict over the whole state
    /// space exists.
    ExplorationBound,
    /// The incremental revision holds a result that failed on a budget (not
    /// memoized); its label.
    EngineBudget(Label),
    /// The revision and the clean run do not share a comparison basis: the named
    /// component (source, limits, semantic epoch, or a function version) differs.
    /// Nothing is compared across bases.
    BasisMismatch(&'static str),
}

/// The audit's own work meter: a countdown charged before each piece of work. It
/// shares the configured `Limits` with the engine and nothing else.
struct Budget {
    left: u64,
    used: u64,
}

impl Budget {
    fn take(&mut self, units: u64, at: &'static str) -> Result<(), Incompleteness> {
        if units > self.left {
            return Err(Incompleteness::WorkExhausted { at });
        }
        self.left -= units;
        self.used += units;
        Ok(())
    }
}

fn units(n: usize) -> u64 {
    u64::try_from(n).unwrap_or(u64::MAX)
}

/// The front end's budget-exhaustion codes, as the audit reads them: a failure
/// under one of these is the budget's answer, not the model's.
fn front_end_budget(code: &'static str) -> Option<&'static str> {
    matches!(
        code,
        "cml.limit.elaboration_too_large"
            | "cml.limit.work_limit_exceeded"
            | "cml.lower.output_too_large"
            | "cml.lower.work_limit_exceeded"
    )
    .then_some(code)
}

/// Recompute every compared artifact of `source` from scratch, under `limits`.
///
/// Every charge is made before its work, against the audit's own meter: the
/// source length first (refused before it is read), then parsing and the dump
/// (bounded by the nesting depth), the query count (before any per-invariant
/// work), the guard classification, the exploration (declared bounds times the
/// model's size), its successor rows, and every state-by-guard and
/// state-by-invariant scan. A run that stops, or whose answer a bound decided, is
/// [`Completion::Incomplete`] with its reason.
#[must_use]
pub fn clean(source: &str, limits: &Limits, semantic_epoch: &str) -> CleanRun {
    let mut run = CleanRun {
        outputs: BTreeMap::new(),
        explored_states: 0,
        completion: Completion::Complete,
        work: 0,
        basis: Basis {
            // Kept only within the source limit; an oversized source is refused
            // below and its basis records no bytes.
            source: if source.len() <= limits.source_bytes {
                source.as_bytes().to_vec()
            } else {
                Vec::new()
            },
            limits: *limits,
            semantic_epoch: semantic_epoch.to_owned(),
        },
    };
    let mut budget = Budget {
        left: limits.work,
        used: 0,
    };
    let result = clean_into(source, limits, &mut run, &mut budget);
    run.work = budget.used;
    if let Err(why) = result {
        run.completion = Completion::Incomplete(why);
    }
    run
}

#[allow(clippy::too_many_lines)]
fn clean_into(
    source: &str,
    limits: &Limits,
    run: &mut CleanRun,
    budget: &mut Budget,
) -> Result<(), Incompleteness> {
    if source.len() > limits.source_bytes {
        return Err(Incompleteness::SourceTooLarge {
            bytes: source.len(),
            max: limits.source_bytes,
        });
    }
    budget.take(units(source.len()), "parse")?;
    let file = match continuum_cml_syntax::parse(source) {
        Ok(file) => file,
        Err(error) => {
            run.outputs
                .insert(Label::of(Stage::Parse), output::parse_err(&error));
            return Ok(());
        }
    };
    // The dump can outgrow the source by two columns per nesting level per line.
    budget.take(
        units(source.len()).saturating_mul(2 * u64::from(continuum_cml_syntax::MAX_NESTING) + 2),
        "dump",
    )?;
    let dump = continuum_cml_syntax::dump::dump_file(&file);
    run.outputs
        .insert(Label::of(Stage::Parse), output::parse_ok(&dump));

    budget.take(units(dump.len()), "elaborate")?;
    let (elaborated, usage) = continuum_cml_elab::elaborate_with(&file, limits.elab);
    let (norm, identity_len): (NormModel, usize) = match elaborated {
        Ok(norm) => {
            // The normalized identity, charged by its stated bound first, and
            // built once.
            budget.take(
                units(output::norm_identity_bound(usage.nodes, source.len())),
                "normalized identity",
            )?;
            let encoded = output::elaborate_ok(&norm);
            let identity_len = encoded.len();
            run.outputs.insert(Label::of(Stage::Elaborate), encoded);
            (norm, identity_len)
        }
        Err(error) => {
            run.outputs.insert(
                Label::of(Stage::Elaborate),
                output::failed("elab", error.code()),
            );
            return front_end_budget(error.code())
                .map_or(Ok(()), |code| Err(Incompleteness::FrontEndBudget(code)));
        }
    };
    // The queries the model defines, checked before any per-invariant work: the
    // six stage queries and two per invariant (the engine's count, stated here).
    let needed = norm.invariants.len().saturating_mul(2).saturating_add(6);
    if needed > limits.queries {
        return Err(Incompleteness::TooManyQueries {
            needed,
            max: limits.queries,
        });
    }
    // Lowering is bounded by `limits.elab`; the audit also charges the normalized
    // model it hands over, as the engine does.
    budget.take(units(identity_len), "lower")?;
    let model = match continuum_cml_elab::lower_with(&norm, limits.elab).0 {
        Ok(model) => model,
        // No lowered model: nothing downstream exists, on either side. A budget
        // failure is incomplete; any other refusal is the model's answer.
        Err(error) => {
            return front_end_budget(error.code())
                .map_or(Ok(()), |code| Err(Incompleteness::FrontEndBudget(code)));
        }
    };
    // One step of any evaluation over this model is weighted by its size.
    let weight = units(model.identity_alloc_bound() / 8).saturating_add(1);
    budget.take(
        units(limits.explore.states())
            .saturating_add(limits.explore.transitions())
            .saturating_mul(weight),
        "explore",
    )?;
    let explored = bfs::explore(&model, limits.explore);
    if let Ok(exploration) = &explored {
        run.explored_states = exploration.reachable().len() as u64;
        // The canonical graph re-derives every expanded row and writes it.
        budget.take(
            units(exploration.reachable().len())
                .saturating_mul(units(model.actions().len()).saturating_add(1))
                .saturating_mul(weight)
                .saturating_add(
                    exploration
                        .reachable()
                        .transitions()
                        .saturating_mul(units(model.arity() * 16 + 64)),
                )
                // Each state (and frontier state) and its predecessor, a depth and
                // an action.
                .saturating_add(
                    units(exploration.reachable().len())
                        .saturating_add(match exploration {
                            Exploration::Exhausted(partial) => units(partial.frontier().len()),
                            Exploration::Complete(_) => 0,
                        })
                        .saturating_mul(units(model.arity() * 16 + 64)),
                ),
            "encode exploration",
        )?;
    }
    run.outputs.insert(
        Label::of(Stage::Explore),
        output::exploration(&explored, &model),
    );
    // The model's definedness predicates, classified by the whole-chain rule
    // (bn-24a5c, `continuum_model_core::definedness::Definedness`): every
    // `#defined` predicate, grouped by the base of its chain, deepest member
    // first, fail closed on a gap or a name shared with an action. Its own doc
    // bounds the cost at `O((p + a) log (p + a))` times the chain-depth bound (at
    // most `MAX_IDENT_BYTES / 8`, 16): charged first, plus building
    // `action_names` — up to two entries per action (its own name and, for an
    // instance, its schema), each an ordered-set insertion of `levels`
    // comparisons (Codex: a flat `action_count * 128` undercounts this once
    // `action_count` alone exceeds the predicate count).
    let predicate_count = model.predicates().len();
    let action_count = model.actions().len();
    let levels = u64::from(units(predicate_count.saturating_add(action_count).max(2)).ilog2() + 1);
    const CHAIN_DEPTH_BOUND: u64 = 128 / 8; // MAX_IDENT_BYTES / 8
    const BYTE_COMPARISON_UNIT: u64 = 128 / 8;
    budget.take(
        units(predicate_count)
            .saturating_mul(CHAIN_DEPTH_BOUND)
            .saturating_mul(levels)
            .saturating_mul(BYTE_COMPARISON_UNIT)
            .saturating_add(
                units(action_count)
                    .saturating_mul(2)
                    .saturating_mul(levels)
                    .saturating_mul(BYTE_COMPARISON_UNIT),
            ),
        "classify guards",
    )?;
    let definedness = Definedness::of(&model);
    let action_guard_count: usize = definedness.action_chains().map(<[usize]>::len).sum();
    let states = explored.as_ref().map_or(0, |e| e.reachable().len());
    budget.take(
        units(states)
            .saturating_mul(units(action_guard_count))
            .saturating_mul(weight),
        "action definedness",
    )?;
    let action_fault = explored
        .as_ref()
        .ok()
        .and_then(|exploration| undefined_action(&model, &definedness, exploration));
    let safety = match (&explored, &action_fault) {
        (Err(error), _) => DomainSafety::ExplorationFailed(error.to_string()),
        (Ok(_), Some(Err(message))) => DomainSafety::Evaluation(message.clone()),
        (Ok(_), Some(Ok((action, depth, state)))) => DomainSafety::UndefinedAction {
            action: action.clone(),
            depth: *depth,
            state: state.clone(),
        },
        (Ok(Exploration::Complete(_)), None) => DomainSafety::Established,
        (Ok(Exploration::Exhausted(partial)), None) => {
            DomainSafety::Inconclusive(partial.tripped())
        }
    };
    run.outputs.insert(
        Label::of(Stage::DomainSafety),
        output::domain_safety(&safety),
    );
    for invariant in &norm.invariants {
        // Per state: the action guards, the invariant's own chain, the invariant.
        let own_depth = model
            .predicate_index(invariant.name.as_str())
            .map_or(0, |index| definedness.guards_of(index).len());
        budget.take(
            units(states)
                .saturating_mul(
                    units(action_guard_count)
                        .saturating_add(units(own_depth))
                        .saturating_add(1),
                )
                .saturating_mul(weight),
            "check invariant",
        )?;
        let verdict = match &explored {
            Ok(exploration) => verdict(&model, &invariant.name, &definedness, exploration),
            Err(error) => InvariantVerdict::ExplorationFailed(error.to_string()),
        };
        run.outputs.insert(
            Label::named(Stage::CheckInvariant, &invariant.name),
            output::invariant_verdict(&verdict),
        );
    }
    // Every artifact exists; a bound that decided one leaves the run incomplete:
    // a tripped bound, or a state bound that cannot hold the initial states.
    if matches!(
        explored,
        Ok(Exploration::Exhausted(_)) | Err(ExplorationError::InitialStatesExceedBound { .. })
    ) {
        return Err(Incompleteness::ExplorationBound);
    }
    Ok(())
}

/// The declared action or predicate a false definedness-chain member names: the
/// base of its own chain (`continuum_model_core::definedness::definedness_base`),
/// the same for every member of the chain whatever depth is false.
fn definedness_name(model: &Model, guard: usize) -> String {
    model
        .predicates()
        .get(guard)
        .map(|predicate| predicate.name().as_str())
        .and_then(definedness_base)
        .unwrap_or_default()
        .to_owned()
}

/// The first explored state (least depth, then canonical order) where some
/// action's whole definedness chain (deepest member first, in the order of its
/// shallowest member) has a false member, and the declared action or predicate
/// that chain names; or the first guard that cannot be evaluated, in canonical
/// order. `None` when every guard holds everywhere.
fn undefined_action(
    model: &Model,
    definedness: &Definedness,
    exploration: &Exploration,
) -> Option<Result<(String, usize, State), String>> {
    let reachable = exploration.reachable();
    let mut best: Option<(usize, usize, State)> = None; // (depth, guard, state)
    for (position, state) in reachable.states().iter().enumerate() {
        let depth = reachable.depths()[position];
        let mut failed = None;
        'chains: for chain in definedness.action_chains() {
            for &guard in chain {
                match model.evaluate_predicate(guard, state) {
                    Err(error) => return Some(Err(error.to_string())),
                    Ok(false) => {
                        failed = Some(guard);
                        break 'chains;
                    }
                    Ok(true) => {}
                }
            }
        }
        if let Some(guard) = failed
            && best.as_ref().is_none_or(|(d, _, _)| depth < *d)
        {
            best = Some((depth, guard, state.clone()));
        }
    }
    best.map(|(depth, guard, state)| Ok((definedness_name(model, guard), depth, state)))
}

/// One invariant over the full model's exploration, written independently of the
/// engine's check. Per state in canonical order: the actions' whole definedness
/// chains (a false member marks the state and nothing else is read there), then
/// the invariant's own whole chain, deepest first (a false member marks the
/// state), then the invariant; the first evaluation error of any of them wins.
/// After the scan: an undefined action read, then an undefined invariant read,
/// then the least failing depth, then `Holds` for a closed run. Each "first" is
/// least depth, then canonical order.
fn verdict(
    model: &Model,
    name: &str,
    definedness: &Definedness,
    exploration: &Exploration,
) -> InvariantVerdict {
    let Some(index) = model.predicate_index(name) else {
        return InvariantVerdict::NoPredicate;
    };
    // `#` is not a CML identifier character, so a declared invariant's name never
    // itself ends in `#defined`: `index` is always a chain's base, never a
    // member — this function does not implement the reference scanner's separate
    // case for a *subject* that is itself a guard
    // (`continuum_engine_reference::definedness::scan`'s `is_guard` branch),
    // because `name` is always a declared CML invariant here, never a `#defined`
    // predicate's own name.
    let own_chain = definedness.guards_of(index);
    let reachable = exploration.reachable();
    let mut first_action: Option<(String, usize, State)> = None;
    let mut first_undefined: Option<(usize, State)> = None;
    let mut least: Option<(usize, State)> = None;
    'states: for (position, state) in reachable.states().iter().enumerate() {
        let depth = reachable.depths()[position];
        for chain in definedness.action_chains() {
            for &guard in chain {
                match model.evaluate_predicate(guard, state) {
                    Err(error) => return InvariantVerdict::Evaluation(error.to_string()),
                    Ok(false) => {
                        if first_action.as_ref().is_none_or(|(_, d, _)| depth < *d) {
                            first_action =
                                Some((definedness_name(model, guard), depth, state.clone()));
                        }
                        continue 'states;
                    }
                    Ok(true) => {}
                }
            }
        }
        for &guard in own_chain {
            match model.evaluate_predicate(guard, state) {
                Err(error) => return InvariantVerdict::Evaluation(error.to_string()),
                Ok(false) => {
                    if first_undefined
                        .as_ref()
                        .is_none_or(|(best, _)| depth < *best)
                    {
                        first_undefined = Some((depth, state.clone()));
                    }
                    continue 'states;
                }
                Ok(true) => {}
            }
        }
        match model.evaluate_predicate(index, state) {
            Err(error) => return InvariantVerdict::Evaluation(error.to_string()),
            Ok(true) => {}
            Ok(false) => match &least {
                Some((best, _)) if *best <= depth => {}
                _ => least = Some((depth, state.clone())),
            },
        }
    }
    if let Some((action, depth, state)) = first_action {
        return InvariantVerdict::UndefinedAction {
            action,
            depth,
            state,
        };
    }
    if let Some((depth, state)) = first_undefined {
        return InvariantVerdict::Undefined { depth, state };
    }
    match (least, exploration) {
        (Some((depth, state)), _) => InvariantVerdict::Violated { depth, state },
        (None, Exploration::Complete(closed)) => InvariantVerdict::Holds {
            states: closed.len(),
        },
        (None, Exploration::Exhausted(partial)) => {
            InvariantVerdict::Inconclusive(partial.tripped())
        }
    }
}

/// The audit verdict for one compared label.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Parity {
    /// The canonical outputs are byte-equal.
    Agrees,
    /// They differ.
    Diverges,
    /// The engine produced it and the clean run did not.
    OnlyInEngine,
    /// The clean run produced it and the engine did not.
    OnlyInClean,
}

/// One divergence, attributed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mismatch {
    /// The label.
    pub label: Label,
    /// How it diverged.
    pub parity: Parity,
    /// The engine record's derivation class, when it has one.
    pub class: Option<ReuseClass>,
}

/// Who the first divergence is attributed to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Blame {
    /// A reuse licensed under this triple: quarantine it.
    Reuse(Triple),
    /// Several internal reuses could explain it: reported, nothing quarantined on a
    /// guess.
    Ambiguous(Vec<Triple>),
    /// A recomputation diverged with no reused input upstream: an engine defect,
    /// not a reuse defect. Nothing is quarantined; the defect is reported.
    Recompute(Label),
}

/// The audit of one revision against its clean recomputation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParityReport {
    /// Every compared label, with its verdict.
    pub labels: BTreeMap<Label, Parity>,
    /// The divergent ones, in label order.
    pub mismatches: Vec<Mismatch>,
    /// The first divergent query in stage order, attributed (RFC 0030, "On
    /// mismatch": the minimizer's "first divergent query"). `None` when the audit
    /// is inconclusive: nothing is blamed on a budget.
    pub first: Option<(Label, Blame)>,
    /// Why either run is incomplete; empty when both completed within bounds.
    pub incomplete: Vec<Incompleteness>,
}

impl ParityReport {
    /// Whether every compared label agrees. Typed.
    #[must_use]
    pub fn verdict(&self) -> AuditVerdict {
        if !self.incomplete.is_empty() {
            AuditVerdict::Inconclusive
        } else if self.mismatches.is_empty() {
            AuditVerdict::Parity
        } else {
            AuditVerdict::Mismatch
        }
    }
}

/// The audit's overall verdict.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuditVerdict {
    /// Both runs completed within bounds, and every compared artifact agrees.
    Parity,
    /// Both runs completed within bounds, and at least one artifact diverges.
    Mismatch,
    /// One run or both did not complete within bounds ([`ParityReport::incomplete`]
    /// says why). Neither held nor failed parity (RFC 0030: budget exhaustion is
    /// not a parity outcome), and never counted as either.
    Inconclusive,
}

/// Attribute the first divergent label.
///
/// Its own licence when it was reused. Otherwise the reuses among the *internal*
/// records above it (the projections and the lowered model, which have no clean
/// counterpart): a compared record above the first divergence agrees with the clean
/// run by construction (it is earlier in stage order), so it is never blamed. One
/// such reuse is blamed; several are reported as ambiguous and nothing is
/// quarantined on a guess; none makes it a recompute defect.
fn blame(revision: &Revision, label: &Label) -> Blame {
    let Some(first) = revision.get(label) else {
        return Blame::Recompute(label.clone());
    };
    if let Outcome::Reused { triple, .. } = &first.outcome {
        return Blame::Reuse(*triple);
    }
    let mut frontier: Vec<&Record> = vec![first];
    let mut seen = BTreeSet::new();
    let mut suspects = BTreeSet::new();
    while let Some(record) = frontier.pop() {
        for edge in &record.edges {
            if edge.from.stage.compared() == Compared::Yes || !seen.insert(edge.from.clone()) {
                continue;
            }
            let Some(parent) = revision.get(&edge.from) else {
                continue;
            };
            if let Outcome::Reused { triple, .. } = &parent.outcome {
                suspects.insert(*triple);
            }
            frontier.push(parent);
        }
    }
    let mut suspects: Vec<Triple> = suspects.into_iter().collect();
    match suspects.len() {
        0 => Blame::Recompute(label.clone()),
        1 => Blame::Reuse(suspects.remove(0)),
        _ => Blame::Ambiguous(suspects),
    }
}

/// Why a revision and its clean runs cannot be judged, or nothing.
///
/// The basis first: `runs[last]` must be the clean run of the revision's own
/// source, and every run must share its limits and semantic epoch; the revision's
/// compared records must name the definition versions this audit implements. Then
/// the completion of each clean run, and of the revision: a result that failed on
/// a budget (returned, not memoized), or an exploration a bound decided (its typed
/// status, fail-closed on bytes it does not recognize). One check, used by both
/// [`parity`] and [`cone`].
fn completeness(revision: &Revision, runs: &[&CleanRun]) -> Vec<Incompleteness> {
    let mut incomplete = Vec::new();
    let mut basis = |why| {
        if !incomplete.contains(&Incompleteness::BasisMismatch(why)) {
            incomplete.push(Incompleteness::BasisMismatch(why));
        }
    };
    if let Some(own) = runs.last()
        && own.basis.source.as_slice() != revision.source().bytes()
    {
        basis("source");
    }
    for run in runs {
        if run.basis.limits != *revision.limits() {
            basis("limits");
        }
        if run.basis.semantic_epoch != revision.semantic_epoch() {
            basis("semantic_epoch");
        }
    }
    for record in revision.records.values() {
        if record.label.stage.compared() != Compared::Yes {
            continue;
        }
        let audited = AUDITED_VERSIONS
            .iter()
            .find(|(stage, _)| *stage == record.label.stage);
        // A compared stage this audit does not name fails closed.
        if audited.is_none_or(|(_, version)| record.definition.function_version != *version) {
            basis("function_version");
        }
    }
    for run in runs {
        if let Completion::Incomplete(why) = &run.completion
            && !incomplete.contains(why)
        {
            incomplete.push(why.clone());
        }
    }
    for (label, record) in &revision.records {
        if let Outcome::Recomputed {
            memo: Memo::NotMemoized(_),
            ..
        } = record.outcome
        {
            incomplete.push(Incompleteness::EngineBudget(label.clone()));
        }
        if label.stage == Stage::Explore
            && matches!(
                output::exploration_status(record.output.bytes()),
                ExplorationStatus::BoundDecided | ExplorationStatus::Unrecognized
            )
            && !incomplete.contains(&Incompleteness::ExplorationBound)
        {
            incomplete.push(Incompleteness::ExplorationBound);
        }
    }
    incomplete
}

/// Compare an engine revision with the clean run of the same source.
#[must_use]
pub fn parity(revision: &Revision, clean: &CleanRun) -> ParityReport {
    let produced: BTreeMap<&Label, &Record> = revision
        .records
        .iter()
        .filter(|(label, _)| label.stage.compared() == Compared::Yes)
        .collect();
    let labels: BTreeSet<&Label> = produced
        .keys()
        .copied()
        .chain(clean.outputs.keys())
        .collect();
    let mut verdicts = BTreeMap::new();
    let mut mismatches = Vec::new();
    for label in labels {
        let parity = match (produced.get(label), clean.outputs.get(label)) {
            (Some(record), Some(bytes)) if record.output.bytes() == bytes.as_slice() => {
                Parity::Agrees
            }
            (Some(_), Some(_)) => Parity::Diverges,
            (Some(_), None) => Parity::OnlyInEngine,
            (None, _) => Parity::OnlyInClean,
        };
        if parity != Parity::Agrees {
            mismatches.push(Mismatch {
                label: label.clone(),
                parity: parity.clone(),
                class: produced.get(label).map(|record| record.class),
            });
        }
        verdicts.insert(label.clone(), parity);
    }
    let incomplete = completeness(revision, &[clean]);
    let first = if incomplete.is_empty() {
        mismatches
            .iter()
            .min_by(|a, b| a.label.cmp(&b.label))
            .map(|m| (m.label.clone(), blame(revision, &m.label)))
    } else {
        None
    };
    ParityReport {
        labels: verdicts,
        mismatches,
        first,
        incomplete,
    }
}

/// Whether the engine's invalidation cone contained the true one.
///
/// Queries are content-addressed, so a label is not the unit of staleness: a renamed
/// invariant is a new label whose key, and result, are the old one's. A reuse is
/// stale exactly when the reused output differs from the clean recomputation of the
/// edited source; that is the under-invalidation this checks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConeSoundness {
    /// Compared labels whose clean output changed between the two sources, or that
    /// exist in one clean run only.
    pub true_cone: Vec<Label>,
    /// Compared labels the engine recomputed.
    pub computed_cone: Vec<Label>,
    /// Reused through an `Exact`, `Validated` or `Conservative` derivation, and
    /// different from the clean output: under-invalidation, the one unrecoverable
    /// error.
    pub under_invalidated: Vec<Label>,
    /// Reused on an `Experimental` derivation and different from the clean output:
    /// measured, never promotable.
    pub experimental_misses: Vec<Label>,
    /// Recomputed although the label's clean output is the same in both runs:
    /// over-invalidation, which costs latency only.
    pub over_invalidated: Vec<Label>,
    /// Why the revision or either clean run is incomplete, or why they do not share
    /// a basis. When non-empty the cone is not judged (every list above is empty):
    /// inconclusive, not sound and not unsound.
    pub incomplete: Vec<Incompleteness>,
}

/// Check the engine's cone for the edit `before_revision → revision` against the
/// clean runs of both sources.
#[must_use]
pub fn cone(
    before: &CleanRun,
    after: &CleanRun,
    before_revision: &Revision,
    revision: &Revision,
) -> ConeSoundness {
    // Soundness is judged only on one basis and between complete runs, on both
    // sides: a missing artifact of a run that stopped says nothing about
    // staleness. `before` must be the clean run of the earlier revision's source,
    // and `after` of the later one's.
    let mut incomplete = completeness(revision, &[before, after]);
    if before.basis.source.as_slice() != before_revision.source().bytes()
        && !incomplete.contains(&Incompleteness::BasisMismatch("source"))
    {
        incomplete.push(Incompleteness::BasisMismatch("source"));
    }
    if !incomplete.is_empty() {
        return ConeSoundness {
            true_cone: Vec::new(),
            computed_cone: Vec::new(),
            under_invalidated: Vec::new(),
            experimental_misses: Vec::new(),
            over_invalidated: Vec::new(),
            incomplete,
        };
    }
    let compared = revision
        .records
        .keys()
        .filter(|label| label.stage.compared() == Compared::Yes);
    let labels: BTreeSet<&Label> = before
        .outputs
        .keys()
        .chain(after.outputs.keys())
        .chain(compared)
        .collect();
    let mut soundness = ConeSoundness {
        true_cone: Vec::new(),
        computed_cone: Vec::new(),
        under_invalidated: Vec::new(),
        experimental_misses: Vec::new(),
        over_invalidated: Vec::new(),
        incomplete: Vec::new(),
    };
    for label in labels {
        let clean_before = before.outputs.get(label);
        let clean_after = after.outputs.get(label);
        if clean_before != clean_after {
            soundness.true_cone.push(label.clone());
        }
        let Some(record) = revision.get(label) else {
            continue;
        };
        let agrees = clean_after.is_some_and(|bytes| bytes.as_slice() == record.output.bytes());
        match (&record.outcome, record.class, agrees) {
            (Outcome::Reused { .. }, _, true) => {}
            // Only the interactive lane serves `Experimental` derivations; in the
            // promotion lane every stale reuse is under-invalidation, whatever
            // class the record claims.
            (Outcome::Reused { .. }, ReuseClass::Experimental, false)
                if revision.lane == Lane::Interactive =>
            {
                soundness.experimental_misses.push(label.clone());
            }
            (Outcome::Reused { .. }, _, false) => soundness.under_invalidated.push(label.clone()),
            (Outcome::Recomputed { .. }, _, _) => {
                soundness.computed_cone.push(label.clone());
                if clean_before.is_some() && clean_before == clean_after {
                    soundness.over_invalidated.push(label.clone());
                }
            }
        }
    }
    soundness
}

#[cfg(test)]
mod tests {
    use continuum_engine_reference::bfs::Bounds;
    use continuum_engine_reference::checking::{self, CheckOutcome, DeadlockPolicy, Obligations};
    use continuum_engine_reference::{ActionDecl, BoolExpr, CmpOp, Guarded, IntExpr, ModelBuilder};

    use super::*;

    // bn-1eoco: the whole-chain rule, differentially, against hand-built models —
    // the only way to give `clean`'s own scan a chain deeper than depth 1, a gap,
    // or a collision, since `clean` only ever lowers CML source, and `#` is not a
    // CML identifier character (so lowering never writes one). Each case mirrors
    // `continuum-engine-reference`'s own `tests/definedness_nested.rs` (cr-pt5h3a)
    // and is checked against its `checking::check`, the same oracle
    // `tests/differential.rs` compares this crate's engine against — not a
    // re-derivation.

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

    /// The reference engine's own verdict for `name`, converted the way
    /// `tests/differential.rs`'s oracle is.
    fn oracle_verdict(model: &Model, exploration: &Exploration, name: &str) -> InvariantVerdict {
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

    /// Assert the audit's own `verdict` — the production code path, not a copy of
    /// it — and the reference checker agree over `model`'s whole exploration, for
    /// every name in `names`.
    fn assert_agrees(model: &Model, names: &[&str]) {
        let exploration = bfs::explore(model, Bounds::CERTIFIABLE).expect("explores");
        let definedness = Definedness::of(model);
        for &name in names {
            let expected = oracle_verdict(model, &exploration, name);
            let got = verdict(model, name, &definedness, &exploration);
            assert_eq!(got, expected, "{name}");
        }
    }

    /// Codex's case: `I = true`, `I#defined = true`, `I#defined#defined` false at
    /// `x = 1`: the audit follows the whole chain, not just depth 1.
    ///
    /// Only `I` is checked here, not `I#defined` itself: the oracle's own `scan`
    /// treats a *subject* that is itself a guard specially (a false value there
    /// is `Undefined`, never `Violated`, its `is_guard` branch), which `verdict`
    /// does not implement — `name` is always a declared CML invariant, and `#` is
    /// not a CML identifier character, so `verdict` never receives a name that is
    /// itself a definedness predicate in production.
    #[test]
    fn a_false_guard_of_a_guard_is_undefined_in_the_audit_too() {
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
    fn a_false_guard_of_an_action_guard_is_undefined_action_in_the_audit_too() {
        let model = model(&[
            ("Ok", BoolExpr::constant(true)),
            ("Step#defined", BoolExpr::constant(true)),
            ("Step#defined#defined", x_is(0)),
        ]);
        assert_agrees(&model, &["Ok"]);
    }

    /// A gap (`I#defined#defined` with no `I#defined`) is not a well-formed guard
    /// of `I`: fail closed as an action read, in the audit too.
    #[test]
    fn a_chain_with_a_gap_fails_closed_in_the_audit_too() {
        let model = model(&[
            ("I", BoolExpr::constant(true)),
            ("J", BoolExpr::constant(true)),
            ("I#defined#defined", x_is(0)),
        ]);
        assert_agrees(&model, &["I", "J"]);
    }

    /// An action sharing a chain member's full name (`I#defined`) reads the whole
    /// chain as the action's, in the audit too.
    #[test]
    fn a_chain_through_an_action_name_fails_closed_in_the_audit_too() {
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

    /// A nested guard that cannot be evaluated is an evaluation error, in the
    /// audit too — reported before the shallower guard is read.
    #[test]
    fn a_nested_evaluation_error_is_reported_in_the_audit_too() {
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
        let definedness = Definedness::of(&model);
        let got = verdict(&model, "I", &definedness, &exploration);
        assert!(matches!(got, InvariantVerdict::Evaluation(_)), "{got:?}");
    }

    /// `undefined_action` alone (the `domain_safety` path's own scan): reports the
    /// chain's base name, agreeing with the reference checker's classification.
    #[test]
    fn undefined_action_reports_the_chains_base_name() {
        let model = model(&[
            ("Ok", BoolExpr::constant(true)),
            ("Step#defined", BoolExpr::constant(true)),
            ("Step#defined#defined", x_is(0)),
        ]);
        let exploration = bfs::explore(&model, Bounds::CERTIFIABLE).expect("explores");
        let definedness = Definedness::of(&model);
        let (action, depth, _) = undefined_action(&model, &definedness, &exploration)
            .expect("found")
            .expect("evaluates");
        assert_eq!(action, "Step");
        assert_eq!(depth, 1);
    }
}
