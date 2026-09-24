//! Canonical output encodings: the bytes the Incremental Parity Audit compares.
//!
//! RFC 0030, "Compared artifacts": comparison is over the canonical encoding, and a
//! difference that is explicitly non-semantic is normalized away by the encoder, not
//! excused by the comparator. Each function here is the *definition* of one query's
//! output identity. The engine ([`crate::engine`]) and the independent clean
//! recomputation ([`crate::audit`]) both call these encoders; neither calls the
//! other's decision logic. A shared encoder is the comparison basis, not a shared
//! decision: an encoder that dropped information would make both lanes agree on a
//! weaker artifact, which is why each encoder below states what it keeps.
//!
//! Every encoding starts with a short ASCII tag and uses length prefixes, so two
//! different outputs never share one spelling (injectivity).

use continuum_cml_elab::NormModel;
use continuum_cml_elab::norm::Invariant;
use continuum_cml_syntax::ParseError;
use continuum_engine_reference::bfs::{Bound, Discovery, Exploration, ExplorationError, Reachable};
use continuum_engine_reference::model::{Model, State};
use continuum_value::identity::{Blake3Hasher, ContentHasher};

use std::collections::BTreeSet;

/// Append a length-prefixed byte string.
pub(crate) fn field(out: &mut Vec<u8>, bytes: &[u8]) {
    out.extend_from_slice(&(bytes.len() as u64).to_be_bytes());
    out.extend_from_slice(bytes);
}

/// The `parse` output: the span-free tree dump on success (two trees dump alike
/// exactly when they are equal apart from spans, `continuum_cml_syntax::dump`), or
/// the error code and its location on failure.
///
/// A parse error keeps its span: `parse` is keyed by the source bytes, so a moved
/// error is a different output, and nothing downstream of a parse error is demanded.
#[must_use]
pub fn parse_ok(dump: &str) -> Vec<u8> {
    let mut out = b"parse-ok/1".to_vec();
    field(&mut out, dump.as_bytes());
    out
}

/// The `parse` output of a failed parse.
#[must_use]
pub fn parse_err(error: &ParseError) -> Vec<u8> {
    let mut out = b"parse-err/1".to_vec();
    field(&mut out, error.code().as_bytes());
    out.extend_from_slice(&error.span.start.to_be_bytes());
    out.extend_from_slice(&error.span.end.to_be_bytes());
    out
}

/// The `elaborate` output on success: the normalized identity. On failure, the
/// error code only.
///
/// The span is deliberately not part of the output: `elaborate` is keyed by the
/// span-free parse output, so a span here would be a function of an input the key
/// does not name (whitespace edits move it). Locating a diagnostic belongs to a
/// query keyed by the source, which this spike does not register.
#[must_use]
pub fn elaborate_ok(model: &NormModel) -> Vec<u8> {
    let mut out = b"elab-ok/1".to_vec();
    field(&mut out, model.identity().as_bytes());
    out
}

/// An upper bound on the bytes of `NormModel::identity` for a model the elaborator
/// built while charging `nodes` output nodes, from a source of `source_bytes`.
///
/// The identity is written one declaration or clause per line, indented at most
/// two levels, with every expression inline. Each rendered node is a keyword or
/// operator and its delimiters (under 32 bytes) plus the text it owns, which the
/// elaborator charged at 32 bytes a node (`BYTES_PER_NODE`). A string literal is
/// rendered escaped (`{:?}`), up to six bytes per source byte (`\u{7f}`), so its
/// text can render as 6 x 32 bytes per charged node: 256 bytes a charged node
/// covers keyword, delimiters and the worst escape. Declaration headers and names come from the source, at most a
/// few times its length. The bound is charged *before* the identity is built, and
/// `tests/engine_classes.rs` checks it on every corpus and fuzz-corpus model.
#[must_use]
pub fn norm_identity_bound(nodes: usize, source_bytes: usize) -> usize {
    nodes
        .saturating_mul(256)
        .saturating_add(source_bytes.saturating_mul(8))
        .saturating_add(64)
}

/// A span-free failure output of any stage: its tag and a stable error code.
#[must_use]
pub fn failed(tag: &str, code: &str) -> Vec<u8> {
    let mut out = tag.as_bytes().to_vec();
    out.extend_from_slice(b"-err/1");
    field(&mut out, code.as_bytes());
    out
}

/// The `build_model` output: the semantic model identity (`Model::identity`).
#[must_use]
pub fn model_ok(identity: &[u8]) -> Vec<u8> {
    let mut out = b"model-ok/1".to_vec();
    field(&mut out, identity);
    out
}

/// The `project_invariant` output: the canonical encoding of each clause of one
/// invariant, in source order, without its name (a renamed invariant with the same
/// body has the same projection).
#[must_use]
pub fn invariant_projection(invariant: &Invariant) -> Vec<u8> {
    let mut out = b"invariant/1".to_vec();
    out.extend_from_slice(&(invariant.clauses.len() as u64).to_be_bytes());
    for clause in &invariant.clauses {
        field(&mut out, clause.canonical().as_bytes());
    }
    out
}

/// The `project_transition_system` output: the normalized identity of the model with
/// its invariants removed. `projected` must be that model.
#[must_use]
pub fn transition_system(projected: &NormModel) -> Vec<u8> {
    let mut out = b"transition-system/1".to_vec();
    field(&mut out, projected.identity().as_bytes());
    out
}

fn state(out: &mut Vec<u8>, state: &State) {
    let values = state.as_slice();
    out.extend_from_slice(&(values.len() as u64).to_be_bytes());
    for value in values {
        out.extend_from_slice(&value.to_be_bytes());
    }
}

fn reachable(out: &mut Vec<u8>, reachable: &Reachable) {
    let states = reachable.states();
    out.extend_from_slice(&(states.len() as u64).to_be_bytes());
    for ((s, depth), origin) in states
        .iter()
        .zip(reachable.depths())
        .zip(reachable.origins())
    {
        state(out, s);
        out.extend_from_slice(&(*depth as u64).to_be_bytes());
        match origin {
            Discovery::Initial => out.push(0),
            Discovery::Step {
                predecessor,
                action,
            } => {
                out.push(1);
                state(out, predecessor);
                out.extend_from_slice(&(*action as u64).to_be_bytes());
            }
        }
    }
    out.extend_from_slice(&(reachable.expanded() as u64).to_be_bytes());
    out.extend_from_slice(&reachable.transitions().to_be_bytes());
}

fn bound_token(bound: Bound) -> &'static [u8] {
    match bound {
        Bound::States => b"states",
        Bound::Depth => b"depth",
        Bound::Transitions => b"transitions",
    }
}

/// The successor rows of every expanded state, in canonical state order: the row's
/// width and each `(action index, target)` in the model's ascending row order, or
/// the evaluation error that stopped the row. Hashed into the exploration's
/// encoding, so two graphs with one BFS tree, one depth column and one transition
/// count but different non-tree edges encode differently.
fn successor_rows(model: &Model, reachable: &Reachable, frontier: &[State]) -> Vec<u8> {
    let unexpanded: BTreeSet<&State> = frontier.iter().collect();
    let mut rows = b"successor-rows/1".to_vec();
    for s in reachable.states() {
        if unexpanded.contains(s) {
            continue;
        }
        state(&mut rows, s);
        match model.successors(s) {
            Ok(steps) => {
                rows.push(0);
                rows.extend_from_slice(&(steps.len() as u64).to_be_bytes());
                for step in &steps {
                    rows.extend_from_slice(&(step.action() as u64).to_be_bytes());
                    state(&mut rows, step.target());
                }
            }
            Err(error) => {
                rows.push(1);
                field(&mut rows, error.to_string().as_bytes());
            }
        }
    }
    rows
}

/// The `explore` output: the canonical state graph of `model`. Keeps every state in
/// canonical order with its depth and its first discovery (predecessor and action
/// index), the expansion and transition counts, a BLAKE3 commitment to every
/// expanded state's successor row (the whole relation, tree and non-tree edges),
/// and, for a bounded run, the tripped bound and the frontier. Action *names* are
/// not kept: a renamed action with the same relation explores to the same graph.
///
/// `model` must be the model `result` explored; its rows are recomputed here.
#[must_use]
pub fn exploration(result: &Result<Exploration, ExplorationError>, model: &Model) -> Vec<u8> {
    let mut out = b"explore/2".to_vec();
    match result {
        Ok(Exploration::Complete(closed)) => {
            out.push(0);
            reachable(&mut out, closed);
            let rows = successor_rows(model, closed, &[]);
            out.extend_from_slice(Blake3Hasher::hash(&rows).as_bytes());
        }
        Ok(Exploration::Exhausted(partial)) => {
            out.push(1);
            reachable(&mut out, partial.explored());
            let rows = successor_rows(model, partial.explored(), partial.frontier());
            out.extend_from_slice(Blake3Hasher::hash(&rows).as_bytes());
            field(&mut out, bound_token(partial.tripped()));
            out.extend_from_slice(&(partial.frontier().len() as u64).to_be_bytes());
            for s in partial.frontier() {
                state(&mut out, s);
            }
        }
        // A state bound that cannot hold the initial states is the bound's
        // answer, not the model's: its own tag, so it is never read as a failure.
        Err(error @ ExplorationError::InitialStatesExceedBound { .. }) => {
            out.push(3);
            field(&mut out, error.to_string().as_bytes());
        }
        Err(error) => {
            out.push(2);
            field(&mut out, error.to_string().as_bytes());
        }
    }
    out
}

/// What a canonical exploration encoding says about its bounds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExplorationStatus {
    /// The state space closed within the declared bounds.
    Closed,
    /// The model's exploration failed (a successor row could not be computed).
    Failed,
    /// A declared bound decided the result: a bound tripped (`Exhausted`), or the
    /// state bound cannot hold the initial states.
    BoundDecided,
    /// Not a canonical exploration encoding this crate writes. Read fail-closed as
    /// bound-decided by every caller that asks whether a run completed.
    Unrecognized,
}

/// Decode the status of a canonical exploration encoding: its tag, the version
/// prefix included. Unknown bytes are [`ExplorationStatus::Unrecognized`].
#[must_use]
pub fn exploration_status(bytes: &[u8]) -> ExplorationStatus {
    const PREFIX: &[u8] = b"explore/2";
    match bytes.strip_prefix(PREFIX).and_then(|rest| rest.first()) {
        Some(0) => ExplorationStatus::Closed,
        Some(1 | 3) => ExplorationStatus::BoundDecided,
        Some(2) => ExplorationStatus::Failed,
        _ => ExplorationStatus::Unrecognized,
    }
}

/// The answer of one invariant over one exploration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InvariantVerdict {
    /// The invariant holds at every state of a closed exploration.
    Holds {
        /// The number of states checked.
        states: usize,
    },
    /// The invariant fails. The reported state is the least in (depth, canonical
    /// order) among the failing states.
    Violated {
        /// Its breadth-first depth.
        depth: usize,
        /// The state.
        state: State,
    },
    /// A bound tripped before the state space closed, and no explored state fails.
    Inconclusive(Bound),
    /// The model declares no predicate of this name.
    NoPredicate,
    /// The predicate could not be evaluated at a state (the first in canonical
    /// order); the evaluator's message.
    Evaluation(String),
    /// The exploration itself failed; its message.
    ExplorationFailed(String),
    /// A reachable state reads a map at an absent key inside the invariant: its
    /// definedness predicate `I#defined` is false there. An undefined read is an
    /// error in CML, so the invariant's own value at that state is no verdict, and
    /// no `Holds` or `Violated` is reported. The state is the least in (depth,
    /// canonical order) among the undefined ones.
    Undefined {
        /// Its breadth-first depth.
        depth: usize,
        /// The state.
        state: State,
    },
    /// A reachable state reads a map at an absent key in an action's guard or
    /// updates: that action's definedness predicate `A#defined` is false there. An
    /// undefined read is an error of the model at that state, so the reachable set
    /// the invariant was checked over is not a CML reachable set and no invariant
    /// verdict is given. Least (depth, canonical order), then the first such action
    /// in predicate order.
    UndefinedAction {
        /// The declared action.
        action: String,
        /// Its breadth-first depth.
        depth: usize,
        /// The state.
        state: State,
    },
}

/// The `check_invariant` output.
#[must_use]
pub fn invariant_verdict(verdict: &InvariantVerdict) -> Vec<u8> {
    let mut out = b"invariant-verdict/1".to_vec();
    match verdict {
        InvariantVerdict::Holds { states } => {
            out.push(0);
            out.extend_from_slice(&(*states as u64).to_be_bytes());
        }
        InvariantVerdict::Violated { depth, state: s } => {
            out.push(1);
            out.extend_from_slice(&(*depth as u64).to_be_bytes());
            state(&mut out, s);
        }
        InvariantVerdict::Inconclusive(bound) => {
            out.push(2);
            field(&mut out, bound_token(*bound));
        }
        InvariantVerdict::NoPredicate => out.push(3),
        InvariantVerdict::Evaluation(message) => {
            out.push(4);
            field(&mut out, message.as_bytes());
        }
        InvariantVerdict::ExplorationFailed(message) => {
            out.push(5);
            field(&mut out, message.as_bytes());
        }
        InvariantVerdict::Undefined { depth, state: s } => {
            out.push(6);
            out.extend_from_slice(&(*depth as u64).to_be_bytes());
            state(&mut out, s);
        }
        InvariantVerdict::UndefinedAction {
            action,
            depth,
            state: s,
        } => {
            out.push(7);
            field(&mut out, action.as_bytes());
            out.extend_from_slice(&(*depth as u64).to_be_bytes());
            state(&mut out, s);
        }
    }
    out
}

/// The answer of the state-domain safety query: every reachable state lies in the
/// declared state domain, the exploration closes, and no successor row fails.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DomainSafety {
    /// Established. Carries no state count, because a `Validated` reuse establishes
    /// it through a closed superset of the reachable states (see
    /// [`crate::engine`], "The `Validated` licence").
    Established,
    /// A bound tripped first.
    Inconclusive(Bound),
    /// The exploration failed (a successor row could not be computed, or the
    /// initial states exceed the state bound); its message.
    ExplorationFailed(String),
    /// An action's definedness predicate is false at an explored state (see
    /// [`InvariantVerdict::UndefinedAction`]).
    UndefinedAction {
        /// The declared action.
        action: String,
        /// Its breadth-first depth.
        depth: usize,
        /// The state.
        state: State,
    },
    /// An action's definedness predicate could not be evaluated; the message.
    Evaluation(String),
}

/// The `domain_safety` output.
#[must_use]
pub fn domain_safety(verdict: &DomainSafety) -> Vec<u8> {
    let mut out = b"domain-safety/1".to_vec();
    match verdict {
        DomainSafety::Established => out.push(0),
        DomainSafety::Inconclusive(bound) => {
            out.push(1);
            field(&mut out, bound_token(*bound));
        }
        DomainSafety::ExplorationFailed(message) => {
            out.push(2);
            field(&mut out, message.as_bytes());
        }
        DomainSafety::UndefinedAction {
            action,
            depth,
            state: s,
        } => {
            out.push(3);
            field(&mut out, action.as_bytes());
            out.extend_from_slice(&(*depth as u64).to_be_bytes());
            state(&mut out, s);
        }
        DomainSafety::Evaluation(message) => {
            out.push(4);
            field(&mut out, message.as_bytes());
        }
    }
    out
}
