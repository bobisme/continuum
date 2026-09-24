//! The carried model: an independent decoder and evaluator for `continuum-model/1`
//! (bn-35y4f; RFC 0005 "Kernel layering"; RFC 0005 correction 1).
//!
//! # Why the kernel reads a model at all
//!
//! A wire-epoch-1 finite-closure certificate carries its own successor relation and
//! names the model only by digest, so the kernel can check closure *under the carried
//! relation* but not that the relation is the model's. bn-2npu measured the gap: 7 of
//! 12 producer mutants (evaluator and emitter faults) wrote a closed, in-domain
//! relation that was not the model's, and the kernel verified it. A wire-epoch-2
//! certificate therefore carries the model's canonical encoding, and the kernel
//! re-derives every successor from it.
//!
//! RFC 0005 places exactly this code in this crate:
//!
//! > continuum-kernel-core
//! >   canonical values
//! >   CIR validation
//! >   propositional/first-order expression evaluator
//! >   transition relation evaluator
//! >
//! > — `notes/plan/rfcs/0005-certificates-and-independent-kernel.md`, "Kernel layering"
//!
//! # Independence
//!
//! > The production engine and kernel MUST NOT share optimized evaluator code. […]
//! > semantics-sensitive functions need independent implementations
//! >
//! > — RFC 0005, "Independence"
//!
//! This module is written from the documented grammar of the model identity
//! (`crates/continuum-model-core/src/identity.rs`, module documentation) and from the
//! documented semantics of the expression language
//! (`crates/continuum-model-core/src/expr.rs`, "Two decisions that could have gone
//! either way"), not from that crate's code, and it links nothing: this crate still
//! depends on no crate at all. The representation differs on purpose (docs/03 §8):
//! names are resolved to variable indices once at decode, expressions live in one
//! flat arena addressed by index, and a state is a borrowed `&[i64]` rather than a
//! name-keyed environment. `tests/c018_kernel_evaluator_differential.rs` in
//! `continuum-engine-reference` compares the two evaluators over the corpus.
//!
//! # The grammar this module reads
//!
//! ```text
//! identity   := "continuum-model/1" variables actions initials predicates [fairness]
//! variables  := count (token lo:i64 hi:i64)*
//! actions    := count (token bool outcomes)*
//! outcomes   := count (count (token int)*)*      -- sorted by encoding, no duplicates
//! initials   := count (count i64*)*
//! predicates := count (token bool)*
//! fairness   := count (strength count token*)*  -- only when count >= 1
//! strength   := 0x20 (weak) | 0x21 (strong)
//! count      := u64 big-endian     token := count bytes     i64 := big-endian
//! int        := 0x01 i64 | 0x02 token | 0x03 op int int | 0x04 int int | 0x05 int int
//! op         := 0 add | 1 sub | 2 mul
//! bool       := 0x10 (0|1) | 0x11 cmp int int | 0x12 bool | 0x13 bool bool
//!             | 0x14 bool bool | 0x15 bool bool | 0x16 int lo:i64 hi:i64
//! cmp        := 0 eq | 1 ne | 2 lt | 3 le | 4 gt | 5 ge
//! ```
//!
//! `0x04`/`0x05` are min/max, `0x12`..`0x15` are not/and/or/implies, and `0x16` is
//! the inclusive range test.
//!
//! # Canonicity
//!
//! The encoding is a model's identity (ADR-0013), so the decoder admits exactly one
//! encoding per model: variable, action and predicate names strictly ascending; the
//! outcomes of one action strictly ascending by their encoded bytes; the assignments
//! of one outcome strictly ascending by variable; initial states strictly ascending
//! and of the model's arity; fairness, when present, non-empty, with each scope
//! strictly ascending and the assumptions strictly ascending by `(strength, scope)`;
//! every name an expression, an assignment or a fairness scope mentions declared; and
//! no byte left over.
//!
//! # Semantics
//!
//! - Arithmetic is checked `i64`. An overflow is a fault, never a wrapped value.
//! - Connectives do **not** short-circuit: both operands are evaluated, so a fault in
//!   either operand is a fault of the whole expression.
//! - An action is enabled when its guard is true. Each outcome of an enabled action
//!   evaluates every assignment in the *pre*-state, copies every unassigned variable
//!   (the frame rule), and must stay in the declared domain.
//!
//! A fault anywhere in a state's successor computation means the model has no
//! well-defined successor relation at that state. The reference engine refuses such
//! a model, and the kernel rejects a certificate that claims one.
//!
//! # Bounds
//!
//! The section is at most [`crate::wire::MAX_MODEL_BYTES`] bytes, an expression
//! nests at most [`crate::wire::MAX_EXPRESSION_DEPTH`] deep (the model layer's own
//! bound), and the arena holds at most [`crate::wire::MAX_EXPRESSION_NODES`] nodes.
//! A model beyond a bound is [`Feature::ResourceBound`], never a rejection: it may be
//! a valid model this checker declines to evaluate (INV-008). Decode recursion is
//! bounded by the depth limit, and evaluation recursion by the depth of the arena,
//! which the same limit bounds. Nothing reserves memory from a declared count.

use crate::verdict::{Feature, Field, Rejection, Resource};
use crate::wire::{
    DecodeFailure, MAX_ACTIONS, MAX_EXPRESSION_DEPTH, MAX_EXPRESSION_NODES, MAX_STATES,
    MAX_VARIABLES, Reader, Token, Variable,
};

/// The version tag every model identity starts with.
pub(crate) const IDENTITY_TAG: &[u8] = b"continuum-model/1";

/// An index into [`Model::nodes`].
type NodeId = u32;

/// One comparison operator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Cmp {
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
}

/// One expression node. Children always have smaller indices than their parent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Node {
    Const(i64),
    Var(u16),
    Add(NodeId, NodeId),
    Sub(NodeId, NodeId),
    Mul(NodeId, NodeId),
    Min(NodeId, NodeId),
    Max(NodeId, NodeId),
    Bool(bool),
    Compare(Cmp, NodeId, NodeId),
    Not(NodeId),
    And(NodeId, NodeId),
    Or(NodeId, NodeId),
    Implies(NodeId, NodeId),
    InRange(NodeId, i64, i64),
}

/// A root expression and the number of nodes under it (its evaluation cost).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Root {
    node: NodeId,
    size: u64,
}

/// One assignment of an outcome: the variable's index and the value's expression.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Assignment {
    variable: u16,
    value: Root,
}

/// One action: its name, its guard and its outcomes.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Action {
    name: Token,
    guard: Root,
    outcomes: Vec<Vec<Assignment>>,
}

/// One named state predicate.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Predicate {
    name: Token,
    body: Root,
}

/// A decoded model. Inert data; only [`decode`] builds one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Model {
    identity: Vec<u8>,
    variables: Vec<Variable>,
    actions: Vec<Action>,
    initial_states: Vec<Vec<i64>>,
    predicates: Vec<Predicate>,
    nodes: Vec<Node>,
}

/// Why one evaluation produced no value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Fault {
    /// Checked `i64` arithmetic overflowed.
    Overflow,
    /// An assignment left the declared domain of its variable.
    OutsideDomain {
        /// The variable's index.
        variable: u16,
        /// The value it would have taken.
        value: i64,
    },
}

impl Model {
    /// The canonical encoding this model was decoded from.
    pub(crate) fn identity(&self) -> &[u8] {
        &self.identity
    }

    /// The declared variables, in canonical order.
    pub(crate) fn variables(&self) -> &[Variable] {
        &self.variables
    }

    /// How many actions the model declares. At most [`MAX_ACTIONS`].
    pub(crate) fn action_count(&self) -> usize {
        self.actions.len()
    }

    /// The declared initial states, strictly ascending.
    pub(crate) fn initial_states(&self) -> &[Vec<i64>] {
        &self.initial_states
    }

    /// The index of the predicate named `name`, if the model declares one.
    pub(crate) fn predicate_index(&self, name: &Token) -> Option<usize> {
        self.predicates
            .binary_search_by(|predicate| predicate.name.cmp(name))
            .ok()
    }

    /// The name of the predicate at `index`.
    pub(crate) fn predicate_name(&self, index: usize) -> Option<&Token> {
        self.predicates.get(index).map(|predicate| &predicate.name)
    }

    /// The node count of the predicate at `index` (its evaluation cost).
    pub(crate) fn predicate_size(&self, index: usize) -> u64 {
        self.predicates
            .get(index)
            .map_or(0, |predicate| predicate.body.size)
    }

    /// An upper bound on the evaluation work of one state's successor computation:
    /// every guard, every assignment of every outcome, a copy of the state per
    /// outcome, and one table lookup per outcome costed at `lookup` units.
    ///
    /// `None` when the bound does not fit in a `u64`.
    pub(crate) fn successor_work(&self, lookup: u64) -> Option<u64> {
        let arity = u64::try_from(self.variables.len()).ok()?;
        let per_outcome_fixed = arity.checked_add(lookup)?;
        let mut work: u64 = 0;
        for action in &self.actions {
            work = work.checked_add(action.guard.size)?;
            for outcome in &action.outcomes {
                work = work.checked_add(per_outcome_fixed)?;
                for assignment in outcome {
                    work = work.checked_add(assignment.value.size)?;
                }
            }
        }
        Some(work)
    }

    /// Evaluate the predicate at `index` in `state`.
    pub(crate) fn holds(&self, index: usize, state: &[i64]) -> Result<bool, Fault> {
        let Some(predicate) = self.predicates.get(index) else {
            return Err(Fault::Overflow);
        };
        self.boolean(predicate.body.node, state, MAX_EXPRESSION_DEPTH)
    }

    /// Every successor of `state`, as `(action index, successor vector)`, appended to
    /// `out` in action order. `scratch` is reused for the successor vector.
    ///
    /// `visit` is called once per outcome of every enabled action; it receives the
    /// action index and the successor vector, and may stop the walk by returning an
    /// error.
    pub(crate) fn successors<E>(
        &self,
        state: &[i64],
        scratch: &mut Vec<i64>,
        mut visit: impl FnMut(u16, &[i64]) -> Result<(), E>,
        fault: impl Fn(u16, Fault) -> E,
    ) -> Result<(), E> {
        for (index, action) in self.actions.iter().enumerate() {
            let label = u16::try_from(index).unwrap_or(u16::MAX);
            let enabled = self
                .boolean(action.guard.node, state, MAX_EXPRESSION_DEPTH)
                .map_err(|f| fault(label, f))?;
            if !enabled {
                continue;
            }
            for outcome in &action.outcomes {
                scratch.clear();
                scratch.extend_from_slice(state);
                for assignment in outcome {
                    // Every value is computed in the pre-state, never in `scratch`.
                    let value = self
                        .integer(assignment.value.node, state, MAX_EXPRESSION_DEPTH)
                        .map_err(|f| fault(label, f))?;
                    let admits = self
                        .variables
                        .get(usize::from(assignment.variable))
                        .is_some_and(|variable| variable.admits(value));
                    if !admits {
                        return Err(fault(
                            label,
                            Fault::OutsideDomain {
                                variable: assignment.variable,
                                value,
                            },
                        ));
                    }
                    let Some(slot) = scratch.get_mut(usize::from(assignment.variable)) else {
                        return Err(fault(label, Fault::Overflow));
                    };
                    *slot = value;
                }
                visit(label, scratch)?;
            }
        }
        Ok(())
    }

    fn node(&self, id: NodeId) -> Result<Node, Fault> {
        self.nodes
            .get(usize::try_from(id).unwrap_or(usize::MAX))
            .copied()
            .ok_or(Fault::Overflow)
    }

    /// Integer evaluation. `depth` is the remaining nesting budget; the decoder
    /// guarantees it is never exhausted, and exhaustion is a fault rather than a
    /// panic in case that invariant is ever weakened.
    fn integer(&self, id: NodeId, state: &[i64], depth: usize) -> Result<i64, Fault> {
        let Some(inner) = depth.checked_sub(1) else {
            return Err(Fault::Overflow);
        };
        match self.node(id)? {
            Node::Const(value) => Ok(value),
            Node::Var(index) => state
                .get(usize::from(index))
                .copied()
                .ok_or(Fault::Overflow),
            Node::Add(a, b) => self
                .integer(a, state, inner)?
                .checked_add(self.integer(b, state, inner)?)
                .ok_or(Fault::Overflow),
            Node::Sub(a, b) => self
                .integer(a, state, inner)?
                .checked_sub(self.integer(b, state, inner)?)
                .ok_or(Fault::Overflow),
            Node::Mul(a, b) => self
                .integer(a, state, inner)?
                .checked_mul(self.integer(b, state, inner)?)
                .ok_or(Fault::Overflow),
            Node::Min(a, b) => {
                let (a, b) = (
                    self.integer(a, state, inner)?,
                    self.integer(b, state, inner)?,
                );
                Ok(if a <= b { a } else { b })
            }
            Node::Max(a, b) => {
                let (a, b) = (
                    self.integer(a, state, inner)?,
                    self.integer(b, state, inner)?,
                );
                Ok(if a >= b { a } else { b })
            }
            _ => Err(Fault::Overflow),
        }
    }

    /// Boolean evaluation. Connectives evaluate both operands (no short-circuit).
    fn boolean(&self, id: NodeId, state: &[i64], depth: usize) -> Result<bool, Fault> {
        let Some(inner) = depth.checked_sub(1) else {
            return Err(Fault::Overflow);
        };
        match self.node(id)? {
            Node::Bool(value) => Ok(value),
            Node::Compare(op, a, b) => {
                let (a, b) = (
                    self.integer(a, state, inner)?,
                    self.integer(b, state, inner)?,
                );
                Ok(match op {
                    Cmp::Eq => a == b,
                    Cmp::Ne => a != b,
                    Cmp::Lt => a < b,
                    Cmp::Le => a <= b,
                    Cmp::Gt => a > b,
                    Cmp::Ge => a >= b,
                })
            }
            Node::Not(a) => Ok(!self.boolean(a, state, inner)?),
            Node::And(a, b) => {
                let (a, b) = (
                    self.boolean(a, state, inner)?,
                    self.boolean(b, state, inner)?,
                );
                Ok(a & b)
            }
            Node::Or(a, b) => {
                let (a, b) = (
                    self.boolean(a, state, inner)?,
                    self.boolean(b, state, inner)?,
                );
                Ok(a | b)
            }
            Node::Implies(a, b) => {
                let (a, b) = (
                    self.boolean(a, state, inner)?,
                    self.boolean(b, state, inner)?,
                );
                Ok(!a | b)
            }
            Node::InRange(a, lo, hi) => {
                let value = self.integer(a, state, inner)?;
                Ok(lo <= value && value <= hi)
            }
            _ => Err(Fault::Overflow),
        }
    }
}

// ---------------------------------------------------------------------------
// decoding
// ---------------------------------------------------------------------------

/// Decode one model section.
///
/// Offsets in a [`Rejection::Truncated`] raised here are relative to the start of
/// the section.
///
/// # Errors
///
/// [`DecodeFailure::Rejected`] when the bytes are not the canonical encoding of a
/// model; [`DecodeFailure::Unsupported`] with [`Feature::ResourceBound`] when a node
/// or depth bound is exceeded.
pub(crate) fn decode(bytes: &[u8]) -> Result<Model, DecodeFailure> {
    let mut decoder = Decoder {
        reader: Reader::new(bytes),
        bytes,
        nodes: Vec::new(),
    };
    let tag = decoder.reader.take(Field::ModelTag, IDENTITY_TAG.len())?;
    if tag != IDENTITY_TAG {
        return Err(Rejection::BadModelTag.into());
    }
    let variables = decoder.variables()?;
    let actions = decoder.actions(&variables)?;
    let initial_states = decoder.initial_states(&variables)?;
    let predicates = decoder.predicates(&variables)?;
    if !decoder.reader.is_empty() {
        decoder.fairness(&actions)?;
    }
    if !decoder.reader.is_empty() {
        return Err(Rejection::ModelTrailingBytes {
            extra: decoder.reader.remaining(),
        }
        .into());
    }
    Ok(Model {
        identity: bytes.to_vec(),
        variables,
        actions,
        initial_states,
        predicates,
        nodes: decoder.nodes,
    })
}

struct Decoder<'a> {
    reader: Reader<'a>,
    bytes: &'a [u8],
    nodes: Vec<Node>,
}

impl Decoder<'_> {
    /// A `u64` count in `min..=max`.
    fn count(&mut self, field: Field, min: u64, max: u64) -> Result<u64, Rejection> {
        let found = self.reader.u64(field)?;
        if found < min || found > max {
            return Err(Rejection::CountOutOfRange {
                field,
                found,
                min,
                max,
            });
        }
        Ok(found)
    }

    /// A `u64`-length-prefixed token under the certificate's token grammar.
    fn token(&mut self, field: Field) -> Result<Token, Rejection> {
        let max = u64::try_from(crate::wire::MAX_TOKEN_BYTES).unwrap_or(u64::MAX);
        let len = self.reader.u64(field)?;
        let fault = if len == 0 {
            Some(crate::verdict::TokenFault::Empty)
        } else if len > max {
            Some(crate::verdict::TokenFault::TooLong {
                found: usize::try_from(len).unwrap_or(usize::MAX),
            })
        } else {
            None
        };
        if let Some(fault) = fault {
            return Err(Rejection::MalformedToken { field, fault });
        }
        let bytes = self
            .reader
            .take(field, usize::try_from(len).unwrap_or(usize::MAX))?;
        Token::from_bytes(field, bytes)
    }

    fn variables(&mut self) -> Result<Vec<Variable>, Rejection> {
        let count = self.count(Field::ModelVariable, 1, u64::from(MAX_VARIABLES))?;
        let mut variables: Vec<Variable> = Vec::new();
        for index in 0..count {
            let position = u16::try_from(index).unwrap_or(u16::MAX);
            let name = self.token(Field::ModelVariable)?;
            if variables
                .last()
                .is_some_and(|previous| *previous.name() >= name)
            {
                return Err(Rejection::NotStrictlyAscending {
                    field: Field::ModelVariable,
                    index: u32::from(position),
                });
            }
            let lo = self.reader.i64(Field::ModelVariable)?;
            let hi = self.reader.i64(Field::ModelVariable)?;
            if lo > hi {
                return Err(Rejection::InvertedVariableRange { variable: position });
            }
            variables.push(Variable::new(name, lo, hi));
        }
        Ok(variables)
    }

    fn actions(&mut self, variables: &[Variable]) -> Result<Vec<Action>, DecodeFailure> {
        let count = self.count(Field::ModelAction, 1, u64::from(MAX_ACTIONS))?;
        let mut actions: Vec<Action> = Vec::new();
        for index in 0..count {
            let name = self.token(Field::ModelAction)?;
            if actions.last().is_some_and(|previous| previous.name >= name) {
                return Err(Rejection::NotStrictlyAscending {
                    field: Field::ModelAction,
                    index: u32::try_from(index).unwrap_or(u32::MAX),
                }
                .into());
            }
            let guard = self.boolean(variables, 1)?;
            // At least one outcome: an action with none has no successor even when
            // enabled, which the model layer refuses to declare.
            let outcome_count = self.count(Field::ModelOutcome, 1, u64::MAX)?;
            let mut outcomes: Vec<Vec<Assignment>> = Vec::new();
            let mut previous: Option<(usize, usize)> = None;
            for outcome_index in 0..outcome_count {
                let start = self.reader.offset();
                let outcome = self.outcome(variables)?;
                let end = self.reader.offset();
                if let Some((before_start, before_end)) = previous {
                    let before = self.bytes.get(before_start..before_end);
                    let here = self.bytes.get(start..end);
                    if !matches!((before, here), (Some(b), Some(h)) if b < h) {
                        return Err(Rejection::NotStrictlyAscending {
                            field: Field::ModelOutcome,
                            index: u32::try_from(outcome_index).unwrap_or(u32::MAX),
                        }
                        .into());
                    }
                }
                previous = Some((start, end));
                outcomes.push(outcome);
            }
            actions.push(Action {
                name,
                guard,
                outcomes,
            });
        }
        Ok(actions)
    }

    fn outcome(&mut self, variables: &[Variable]) -> Result<Vec<Assignment>, DecodeFailure> {
        let arity = u64::try_from(variables.len()).unwrap_or(u64::MAX);
        let count = self.count(Field::ModelOutcome, 0, arity)?;
        let mut assignments: Vec<Assignment> = Vec::new();
        for index in 0..count {
            let name = self.token(Field::ModelOutcome)?;
            let variable = resolve(variables, &name, Field::ModelOutcome)?;
            if assignments
                .last()
                .is_some_and(|previous| previous.variable >= variable)
            {
                return Err(Rejection::NotStrictlyAscending {
                    field: Field::ModelOutcome,
                    index: u32::try_from(index).unwrap_or(u32::MAX),
                }
                .into());
            }
            let value = self.integer(variables, 1)?;
            assignments.push(Assignment { variable, value });
        }
        Ok(assignments)
    }

    fn initial_states(&mut self, variables: &[Variable]) -> Result<Vec<Vec<i64>>, Rejection> {
        let arity = u64::try_from(variables.len()).unwrap_or(u64::MAX);
        let count = self.count(Field::ModelInitialState, 1, u64::from(MAX_STATES))?;
        let mut states: Vec<Vec<i64>> = Vec::new();
        for index in 0..count {
            self.count(Field::ModelInitialState, arity, arity)?;
            let mut state: Vec<i64> = Vec::new();
            for _ in 0..arity {
                state.push(self.reader.i64(Field::ModelInitialState)?);
            }
            if states.last().is_some_and(|previous| *previous >= state) {
                return Err(Rejection::NotStrictlyAscending {
                    field: Field::ModelInitialState,
                    index: u32::try_from(index).unwrap_or(u32::MAX),
                });
            }
            states.push(state);
        }
        Ok(states)
    }

    fn predicates(&mut self, variables: &[Variable]) -> Result<Vec<Predicate>, DecodeFailure> {
        let count = self.count(Field::ModelPredicate, 0, u64::from(u32::MAX))?;
        let mut predicates: Vec<Predicate> = Vec::new();
        for index in 0..count {
            let name = self.token(Field::ModelPredicate)?;
            if predicates
                .last()
                .is_some_and(|previous| previous.name >= name)
            {
                return Err(Rejection::NotStrictlyAscending {
                    field: Field::ModelPredicate,
                    index: u32::try_from(index).unwrap_or(u32::MAX),
                }
                .into());
            }
            let body = self.boolean(variables, 1)?;
            predicates.push(Predicate { name, body });
        }
        Ok(predicates)
    }

    /// The optional fairness section. Checked for canonicity and name resolution; its
    /// content does not enter any obligation this crate discharges.
    fn fairness(&mut self, actions: &[Action]) -> Result<(), Rejection> {
        let action_count = u64::try_from(actions.len()).unwrap_or(u64::MAX);
        let count = self.count(Field::ModelFairness, 1, u64::from(MAX_ACTIONS))?;
        let mut previous: Option<(u8, Vec<u16>)> = None;
        for index in 0..count {
            let strength = self.reader.take(Field::ModelFairness, 1)?;
            let strength = strength.first().copied().unwrap_or(0);
            if strength != 0x20 && strength != 0x21 {
                return Err(Rejection::UnknownOpcode {
                    field: Field::ModelFairness,
                    opcode: strength,
                });
            }
            let scope_count = self.count(Field::ModelFairness, 1, action_count)?;
            let mut scope: Vec<u16> = Vec::new();
            for _ in 0..scope_count {
                let name = self.token(Field::ModelFairness)?;
                let position = actions
                    .binary_search_by(|action| action.name.cmp(&name))
                    .map_err(|_| Rejection::UnresolvedName {
                        field: Field::ModelFairness,
                    })?;
                let position = u16::try_from(position).unwrap_or(u16::MAX);
                if scope.last().is_some_and(|last| *last >= position) {
                    return Err(Rejection::NotStrictlyAscending {
                        field: Field::ModelFairness,
                        index: u32::try_from(index).unwrap_or(u32::MAX),
                    });
                }
                scope.push(position);
            }
            let here = (strength, scope);
            if previous.as_ref().is_some_and(|before| *before >= here) {
                return Err(Rejection::NotStrictlyAscending {
                    field: Field::ModelFairness,
                    index: u32::try_from(index).unwrap_or(u32::MAX),
                });
            }
            previous = Some(here);
        }
        Ok(())
    }

    /// Append one node, within the node bound.
    fn push(&mut self, node: Node) -> Result<NodeId, DecodeFailure> {
        let limit = MAX_EXPRESSION_NODES;
        if self.nodes.len() >= limit as usize {
            return Err(DecodeFailure::Unsupported(Feature::ResourceBound {
                resource: Resource::ExpressionNodes,
                needed: u64::from(limit).saturating_add(1),
                limit: u64::from(limit),
            }));
        }
        let id = NodeId::try_from(self.nodes.len()).unwrap_or(NodeId::MAX);
        self.nodes.push(node);
        Ok(id)
    }

    fn opcode(&mut self) -> Result<u8, Rejection> {
        let byte = self.reader.take(Field::ModelExpression, 1)?;
        Ok(byte.first().copied().unwrap_or(0))
    }

    fn depth_bound(depth: usize) -> Result<(), DecodeFailure> {
        if depth > MAX_EXPRESSION_DEPTH {
            return Err(DecodeFailure::Unsupported(Feature::ResourceBound {
                resource: Resource::ExpressionDepth,
                needed: u64::try_from(depth).unwrap_or(u64::MAX),
                limit: u64::try_from(MAX_EXPRESSION_DEPTH).unwrap_or(u64::MAX),
            }));
        }
        Ok(())
    }

    /// Two operands of the same sort, and the node that combines them.
    fn pair(
        &mut self,
        variables: &[Variable],
        depth: usize,
        boolean: bool,
        make: impl FnOnce(NodeId, NodeId) -> Node,
    ) -> Result<Root, DecodeFailure> {
        let next = depth.saturating_add(1);
        let (a, b) = if boolean {
            (
                self.boolean(variables, next)?,
                self.boolean(variables, next)?,
            )
        } else {
            (
                self.integer(variables, next)?,
                self.integer(variables, next)?,
            )
        };
        let node = self.push(make(a.node, b.node))?;
        Ok(Root {
            node,
            size: a.size.saturating_add(b.size).saturating_add(1),
        })
    }

    /// One integer expression at nesting depth `depth` (a leaf at the root has
    /// depth 1, as in the model layer).
    fn integer(&mut self, variables: &[Variable], depth: usize) -> Result<Root, DecodeFailure> {
        Self::depth_bound(depth)?;
        match self.opcode()? {
            0x01 => {
                let value = self.reader.i64(Field::ModelExpression)?;
                Ok(Root {
                    node: self.push(Node::Const(value))?,
                    size: 1,
                })
            }
            0x02 => {
                let name = self.token(Field::ModelExpression)?;
                let index = resolve(variables, &name, Field::ModelExpression)?;
                Ok(Root {
                    node: self.push(Node::Var(index))?,
                    size: 1,
                })
            }
            0x03 => match self.opcode()? {
                0 => self.pair(variables, depth, false, Node::Add),
                1 => self.pair(variables, depth, false, Node::Sub),
                2 => self.pair(variables, depth, false, Node::Mul),
                other => Err(Rejection::UnknownOpcode {
                    field: Field::ModelExpression,
                    opcode: other,
                }
                .into()),
            },
            0x04 => self.pair(variables, depth, false, Node::Min),
            0x05 => self.pair(variables, depth, false, Node::Max),
            other => Err(Rejection::UnknownOpcode {
                field: Field::ModelExpression,
                opcode: other,
            }
            .into()),
        }
    }

    /// One boolean expression at nesting depth `depth`.
    fn boolean(&mut self, variables: &[Variable], depth: usize) -> Result<Root, DecodeFailure> {
        Self::depth_bound(depth)?;
        let next = depth.saturating_add(1);
        match self.opcode()? {
            0x10 => {
                let value = match self.opcode()? {
                    0 => false,
                    1 => true,
                    other => {
                        return Err(Rejection::UnknownOpcode {
                            field: Field::ModelExpression,
                            opcode: other,
                        }
                        .into());
                    }
                };
                Ok(Root {
                    node: self.push(Node::Bool(value))?,
                    size: 1,
                })
            }
            0x11 => {
                let op = match self.opcode()? {
                    0 => Cmp::Eq,
                    1 => Cmp::Ne,
                    2 => Cmp::Lt,
                    3 => Cmp::Le,
                    4 => Cmp::Gt,
                    5 => Cmp::Ge,
                    other => {
                        return Err(Rejection::UnknownOpcode {
                            field: Field::ModelExpression,
                            opcode: other,
                        }
                        .into());
                    }
                };
                self.pair(variables, depth, false, |a, b| Node::Compare(op, a, b))
            }
            0x12 => {
                let inner = self.boolean(variables, next)?;
                Ok(Root {
                    node: self.push(Node::Not(inner.node))?,
                    size: inner.size.saturating_add(1),
                })
            }
            0x13 => self.pair(variables, depth, true, Node::And),
            0x14 => self.pair(variables, depth, true, Node::Or),
            0x15 => self.pair(variables, depth, true, Node::Implies),
            0x16 => {
                let inner = self.integer(variables, next)?;
                let lo = self.reader.i64(Field::ModelExpression)?;
                let hi = self.reader.i64(Field::ModelExpression)?;
                Ok(Root {
                    node: self.push(Node::InRange(inner.node, lo, hi))?,
                    size: inner.size.saturating_add(1),
                })
            }
            other => Err(Rejection::UnknownOpcode {
                field: Field::ModelExpression,
                opcode: other,
            }
            .into()),
        }
    }
}

/// The index of the declared variable named `name`.
fn resolve(variables: &[Variable], name: &Token, field: Field) -> Result<u16, Rejection> {
    let index = variables
        .binary_search_by(|variable| variable.name().cmp(name))
        .map_err(|_| Rejection::UnresolvedName { field })?;
    Ok(u16::try_from(index).unwrap_or(u16::MAX))
}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::indexing_slicing,
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::arithmetic_side_effects,
        reason = "test bodies assert on known-shaped fixtures"
    )]

    use super::*;
    use crate::fixture::ModelPlan;

    fn decoded(plan: &ModelPlan) -> Result<Model, DecodeFailure> {
        decode(&plan.encode())
    }

    fn rejected(plan: &ModelPlan) -> Rejection {
        match decoded(plan) {
            Err(DecodeFailure::Rejected(rejection)) => rejection,
            other => panic!("expected a rejection, got {other:?}"),
        }
    }

    #[test]
    fn the_die_hard_model_decodes_and_evaluates() {
        let model = decoded(&ModelPlan::diehard()).expect("green model decodes");
        assert_eq!(model.variables().len(), 2);
        assert_eq!(model.action_count(), 6);
        assert_eq!(model.initial_states(), &[vec![0, 0]]);
        let mut scratch = Vec::new();
        let mut seen: Vec<(u16, Vec<i64>)> = Vec::new();
        model
            .successors(
                &[5, 1],
                &mut scratch,
                |action, target| {
                    seen.push((action, target.to_vec()));
                    Ok::<(), Fault>(())
                },
                |_, fault| fault,
            )
            .unwrap();
        // big-to-small, empty-big, empty-small, fill-big, fill-small, small-to-big.
        assert_eq!(
            seen,
            vec![
                (0, vec![3, 3]),
                (1, vec![0, 1]),
                (2, vec![5, 0]),
                (3, vec![5, 1]),
                (4, vec![5, 3]),
                (5, vec![5, 1]),
            ]
        );
        let type_ok = model
            .predicate_index(&Token::from_bytes(Field::ModelPredicate, b"TypeOK").unwrap())
            .expect("TypeOK is declared");
        assert_eq!(model.holds(type_ok, &[5, 3]), Ok(true));
        assert_eq!(model.holds(type_ok, &[6, 3]), Ok(false));
    }

    #[test]
    fn the_tag_must_be_exact() {
        let mut plan = ModelPlan::diehard();
        plan.tag = b"continuum-model/2".to_vec();
        assert_eq!(rejected(&plan), Rejection::BadModelTag);
    }

    #[test]
    fn names_must_be_declared_and_ascending() {
        let mut plan = ModelPlan::diehard();
        plan.variables.reverse();
        assert!(matches!(
            rejected(&plan),
            Rejection::NotStrictlyAscending {
                field: Field::ModelVariable,
                ..
            }
        ));

        let mut plan = ModelPlan::diehard();
        plan.actions.swap(0, 1);
        assert!(matches!(
            rejected(&plan),
            Rejection::NotStrictlyAscending {
                field: Field::ModelAction,
                ..
            }
        ));

        let mut plan = ModelPlan::diehard();
        plan.rename_variable_in_expressions("big", "huge");
        assert_eq!(
            rejected(&plan),
            Rejection::UnresolvedName {
                field: Field::ModelExpression
            }
        );
    }

    #[test]
    fn outcomes_must_be_strictly_ascending_by_encoding() {
        let mut plan = ModelPlan::two_outcomes();
        plan.actions[0].outcomes.reverse();
        assert!(matches!(
            rejected(&plan),
            Rejection::NotStrictlyAscending {
                field: Field::ModelOutcome,
                ..
            }
        ));
        let mut plan = ModelPlan::two_outcomes();
        let first = plan.actions[0].outcomes[0].clone();
        plan.actions[0].outcomes[1] = first;
        assert!(matches!(
            rejected(&plan),
            Rejection::NotStrictlyAscending {
                field: Field::ModelOutcome,
                ..
            }
        ));
    }

    #[test]
    fn unknown_opcodes_and_trailing_bytes_are_rejected() {
        let mut plan = ModelPlan::diehard();
        plan.trailing = vec![0x07];
        assert!(matches!(
            rejected(&plan),
            Rejection::CountOutOfRange {
                field: Field::ModelFairness,
                ..
            } | Rejection::Truncated { .. }
        ));
        let mut bytes = ModelPlan::diehard().encode();
        let guard_at = ModelPlan::diehard().first_guard_offset();
        bytes[guard_at] = 0x7f;
        assert_eq!(
            decode(&bytes),
            Err(DecodeFailure::Rejected(Rejection::UnknownOpcode {
                field: Field::ModelExpression,
                opcode: 0x7f
            }))
        );
    }

    #[test]
    fn fairness_is_decoded_canonically() {
        let mut plan = ModelPlan::diehard();
        plan.fairness = vec![(0x20, vec!["fill-big".to_owned()])];
        assert!(decoded(&plan).is_ok());
        plan.fairness = vec![
            (0x21, vec!["fill-big".to_owned()]),
            (0x20, vec!["fill-big".to_owned()]),
        ];
        assert!(matches!(
            rejected(&plan),
            Rejection::NotStrictlyAscending {
                field: Field::ModelFairness,
                ..
            }
        ));
        plan.fairness = vec![(0x22, vec!["fill-big".to_owned()])];
        assert!(matches!(
            rejected(&plan),
            Rejection::UnknownOpcode {
                field: Field::ModelFairness,
                opcode: 0x22
            }
        ));
        plan.fairness = vec![(0x20, vec!["no-such".to_owned()])];
        assert_eq!(
            rejected(&plan),
            Rejection::UnresolvedName {
                field: Field::ModelFairness
            }
        );
    }

    #[test]
    fn expression_depth_beyond_the_bound_is_unsupported_not_rejected() {
        let mut plan = ModelPlan::diehard();
        plan.deep_guard(MAX_EXPRESSION_DEPTH + 1);
        assert!(matches!(
            decoded(&plan),
            Err(DecodeFailure::Unsupported(Feature::ResourceBound {
                resource: Resource::ExpressionDepth,
                ..
            }))
        ));
        let mut plan = ModelPlan::diehard();
        plan.deep_guard(MAX_EXPRESSION_DEPTH);
        assert!(decoded(&plan).is_ok());
    }

    #[test]
    fn evaluation_is_checked_and_does_not_short_circuit() {
        let mut plan = ModelPlan::diehard();
        plan.overflowing_guard();
        let model = decoded(&plan).unwrap();
        let mut scratch = Vec::new();
        let outcome = model.successors(
            &[0, 0],
            &mut scratch,
            |_, _| Ok::<(), Fault>(()),
            |_, fault| fault,
        );
        assert_eq!(outcome, Err(Fault::Overflow));
    }

    #[test]
    fn every_prefix_of_a_model_is_rejected() {
        let bytes = ModelPlan::diehard().encode();
        for cut in 0..bytes.len() {
            assert!(decode(&bytes[..cut]).is_err(), "prefix {cut} decoded");
        }
    }
}
