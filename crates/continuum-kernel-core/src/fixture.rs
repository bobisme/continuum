//! Test-only certificate producer.
//!
//! The kernel ships a checker, never an encoder: an encoder in the trusted base is
//! the first step towards a producer and a checker that agree because they share
//! code, which is the common-mode failure docs/03 §8 is written against. This module
//! exists only under `cfg(test)`, is not part of the public surface, and is
//! deliberately written against the *documented* wire grammar rather than against
//! the decoder's internals — it emits whatever [`Plan`] says, including sequences
//! and counts the decoder must refuse.
//!
//! The Die Hard model below is the corpus fixture TV-009
//! (`notes/plan/corpus/tla-examples/ports/TV-009/DieHard.ctm`), whose frozen spike
//! facts — 16 reachable states, 96 labelled successor edges — the green-path tests
//! assert against.

#![allow(
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::arithmetic_side_effects,
    reason = "a test-only producer builds fixtures of known shape; the no-panic \
              covenant is a property of the shipped checker, not of its harness"
)]

use std::collections::BTreeSet;

use crate::wire::MAGIC;

/// A byte sink that writes the wire form's primitives, and nothing else.
#[derive(Debug, Default)]
pub(crate) struct Encoder {
    out: Vec<u8>,
}

impl Encoder {
    /// An empty encoder.
    pub(crate) const fn new() -> Self {
        Self { out: Vec::new() }
    }

    /// Append raw bytes.
    pub(crate) fn bytes(&mut self, bytes: &[u8]) {
        self.out.extend_from_slice(bytes);
    }

    /// Append a big-endian `u16`.
    pub(crate) fn u16(&mut self, value: u16) {
        self.out.extend_from_slice(&value.to_be_bytes());
    }

    /// Append a big-endian `u32`.
    pub(crate) fn u32(&mut self, value: u32) {
        self.out.extend_from_slice(&value.to_be_bytes());
    }

    /// Append a big-endian `i64`.
    pub(crate) fn i64(&mut self, value: i64) {
        self.out.extend_from_slice(&value.to_be_bytes());
    }

    /// Append a length-prefixed token, exactly as given — including lengths and
    /// bytes the decoder must reject.
    pub(crate) fn token(&mut self, value: &str) {
        let bytes = value.as_bytes();
        self.u16(u16::try_from(bytes.len()).unwrap_or(u16::MAX));
        self.bytes(bytes);
    }

    /// The bytes written so far.
    pub(crate) fn as_slice(&self) -> &[u8] {
        &self.out
    }

    /// Consume the encoder.
    pub(crate) fn into_bytes(self) -> Vec<u8> {
        self.out
    }
}

/// A declared variable: name, inclusive low bound, inclusive high bound.
pub(crate) type PlannedVariable = (String, i64, i64);

/// A declared transition: action index and target state vector.
pub(crate) type PlannedTransition = (u16, Vec<i64>);

/// Every field of a certificate, independently settable.
///
/// Adversarial tests take a green plan, change one field, and re-encode; the
/// `declared_*` overrides exist so a test can write a count that disagrees with the
/// data that follows it.
#[derive(Debug, Clone)]
pub(crate) struct Plan {
    pub(crate) magic: [u8; 8],
    pub(crate) wire_epoch: u16,
    pub(crate) kind: u16,
    pub(crate) closure_body: bool,
    pub(crate) model_digest: String,
    pub(crate) semantic_epoch: String,
    pub(crate) property_digest: String,
    pub(crate) scope_digest: String,
    pub(crate) assumptions_digest: String,
    pub(crate) producer: String,
    pub(crate) schema_epoch: u16,
    pub(crate) domain_packs: Vec<String>,
    pub(crate) declared_domain_pack_count: Option<u16>,
    pub(crate) variables: Vec<PlannedVariable>,
    pub(crate) declared_variable_count: Option<u16>,
    pub(crate) states: Vec<Vec<i64>>,
    pub(crate) declared_state_count: Option<u32>,
    pub(crate) property_class: u16,
    pub(crate) initial: Vec<Vec<i64>>,
    pub(crate) declared_initial_count: Option<u32>,
    pub(crate) actions: Vec<String>,
    pub(crate) declared_action_count: Option<u16>,
    pub(crate) rows: Vec<Vec<PlannedTransition>>,
    pub(crate) trailing: Vec<u8>,
}

impl Plan {
    /// A green finite-closure certificate for the Die Hard corpus fixture.
    pub(crate) fn diehard_closure() -> Self {
        let (states, rows) = diehard_reachable();
        Self {
            magic: MAGIC,
            wire_epoch: crate::wire::LEGACY_WIRE_EPOCH,
            kind: 1,
            closure_body: true,
            model_digest: "blake3:diehard-model".to_owned(),
            semantic_epoch: "continuum-semantics-1".to_owned(),
            property_digest: "blake3:diehard-typeok".to_owned(),
            scope_digest: "blake3:diehard-scope".to_owned(),
            assumptions_digest: "blake3:empty-assumptions".to_owned(),
            producer: "continuum-engine-reference/0.0.0".to_owned(),
            schema_epoch: crate::wire::LEGACY_WIRE_EPOCH,
            domain_packs: Vec::new(),
            declared_domain_pack_count: None,
            variables: diehard_variables(),
            declared_variable_count: None,
            states,
            declared_state_count: None,
            property_class: 1,
            initial: vec![vec![0, 0]],
            declared_initial_count: None,
            actions: diehard_actions(),
            declared_action_count: None,
            rows,
            trailing: Vec::new(),
        }
    }

    /// A green state-type certificate over the same reachable set.
    pub(crate) fn diehard_type() -> Self {
        let closure = Self::diehard_closure();
        Self {
            kind: 2,
            closure_body: false,
            property_digest: "blake3:diehard-typeok".to_owned(),
            ..closure
        }
    }

    /// Serialize the plan exactly as written.
    pub(crate) fn encode(&self) -> Vec<u8> {
        let mut out = Encoder::new();
        out.bytes(&self.magic);
        out.u16(self.wire_epoch);
        out.u16(self.kind);

        out.token(&self.model_digest);
        out.token(&self.semantic_epoch);
        out.token(&self.property_digest);
        out.token(&self.scope_digest);
        out.token(&self.assumptions_digest);
        out.token(&self.producer);
        out.u16(self.schema_epoch);
        out.u16(
            self.declared_domain_pack_count
                .unwrap_or_else(|| u16::try_from(self.domain_packs.len()).unwrap_or(u16::MAX)),
        );
        for digest in &self.domain_packs {
            out.token(digest);
        }

        out.u16(
            self.declared_variable_count
                .unwrap_or_else(|| u16::try_from(self.variables.len()).unwrap_or(u16::MAX)),
        );
        for (name, lo, hi) in &self.variables {
            out.token(name);
            out.i64(*lo);
            out.i64(*hi);
        }

        out.u32(
            self.declared_state_count
                .unwrap_or_else(|| u32::try_from(self.states.len()).unwrap_or(u32::MAX)),
        );
        for state in &self.states {
            for value in state {
                out.i64(*value);
            }
        }

        if self.closure_body {
            out.u16(self.property_class);
            out.u32(
                self.declared_initial_count
                    .unwrap_or_else(|| u32::try_from(self.initial.len()).unwrap_or(u32::MAX)),
            );
            for state in &self.initial {
                for value in state {
                    out.i64(*value);
                }
            }
            out.u16(
                self.declared_action_count
                    .unwrap_or_else(|| u16::try_from(self.actions.len()).unwrap_or(u16::MAX)),
            );
            for action in &self.actions {
                out.token(action);
            }
            for row in &self.rows {
                out.u32(u32::try_from(row.len()).unwrap_or(u32::MAX));
                for (action, target) in row {
                    out.u16(*action);
                    for value in target {
                        out.i64(*value);
                    }
                }
            }
        }

        out.bytes(&self.trailing);
        out.into_bytes()
    }
}

/// The Die Hard state domain: `big in 0..5 && small in 0..3`, names ascending.
fn diehard_variables() -> Vec<PlannedVariable> {
    vec![("big".to_owned(), 0, 5), ("small".to_owned(), 0, 3)]
}

/// The six Die Hard actions, named in strictly ascending order.
///
/// The index of a name in this list is the action index the transitions use.
fn diehard_actions() -> Vec<String> {
    vec![
        "big-to-small".to_owned(),
        "empty-big".to_owned(),
        "empty-small".to_owned(),
        "fill-big".to_owned(),
        "fill-small".to_owned(),
        "small-to-big".to_owned(),
    ]
}

/// The six successors of `(big, small)`, in action-index order.
///
/// A direct transcription of the six actions of
/// `notes/plan/corpus/tla-examples/ports/TV-009/DieHard.ctm`. Every action is
/// enabled in every state, so several of these are self-loops — the
/// "stuttering-producing actions" the TV-009 edge count includes.
fn diehard_successors(big: i64, small: i64) -> Vec<PlannedTransition> {
    let poured_to_small = (big + small).min(3);
    let poured_to_big = (big + small).min(5);
    vec![
        (0, vec![big - (poured_to_small - small), poured_to_small]),
        (1, vec![0, small]),
        (2, vec![big, 0]),
        (3, vec![5, small]),
        (4, vec![big, 3]),
        (5, vec![poured_to_big, small - (poured_to_big - big)]),
    ]
}

/// Deterministic breadth-first exploration of Die Hard from `(0, 0)`.
///
/// Returns the canonical state table (strictly ascending vectors) and one successor
/// row per table entry, in table order.
fn diehard_reachable() -> (Vec<Vec<i64>>, Vec<Vec<PlannedTransition>>) {
    let mut seen: BTreeSet<(i64, i64)> = BTreeSet::new();
    let mut frontier: Vec<(i64, i64)> = vec![(0, 0)];
    seen.insert((0, 0));
    while let Some((big, small)) = frontier.pop() {
        for (_, target) in diehard_successors(big, small) {
            let next = (target[0], target[1]);
            if seen.insert(next) {
                frontier.push(next);
            }
        }
    }
    let states: Vec<Vec<i64>> = seen.iter().map(|(big, small)| vec![*big, *small]).collect();
    let rows: Vec<Vec<PlannedTransition>> = seen
        .iter()
        .map(|(big, small)| {
            let mut row = diehard_successors(*big, *small);
            row.sort();
            row
        })
        .collect();
    (states, rows)
}

/// The green finite-closure certificate, as bytes.
pub(crate) fn closure_bytes() -> Vec<u8> {
    Plan::diehard_closure().encode()
}

/// The green state-type certificate, as bytes.
pub(crate) fn type_bytes() -> Vec<u8> {
    Plan::diehard_type().encode()
}

/// A seeded xorshift generator.
///
/// Hand-rolled on purpose: docs/12 §1 forbids ambient randomness in the semantic
/// core (INV-005, ADR-0003), and a fuzz corpus that cannot be replayed byte for byte
/// is not evidence. The same seed always produces the same corpus.
#[derive(Debug)]
pub(crate) struct Xorshift {
    state: u64,
}

impl Xorshift {
    /// Seed the generator. A zero seed is replaced, since zero is xorshift's fixed
    /// point.
    pub(crate) const fn new(seed: u64) -> Self {
        Self {
            state: if seed == 0 {
                0x9e37_79b9_7f4a_7c15
            } else {
                seed
            },
        }
    }

    /// The next 64 bits.
    pub(crate) fn next_u64(&mut self) -> u64 {
        let mut x = self.state;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.state = x;
        x
    }

    /// `len` pseudo-random bytes.
    pub(crate) fn bytes(&mut self, len: usize) -> Vec<u8> {
        let mut out = Vec::new();
        while out.len() < len {
            out.extend_from_slice(&self.next_u64().to_be_bytes());
        }
        out.truncate(len);
        out
    }
}

// ---------------------------------------------------------------------------
// wire epoch 2: the model section and the model-bound certificate
// ---------------------------------------------------------------------------

/// A model expression, as the test producer writes it (`continuum-model/1`).
#[derive(Debug, Clone)]
pub(crate) enum Expr {
    Int(i64),
    Var(String),
    Add(Box<Self>, Box<Self>),
    Sub(Box<Self>, Box<Self>),
    Min(Box<Self>, Box<Self>),
    Bool(bool),
    Eq(Box<Self>, Box<Self>),
    Not(Box<Self>),
    And(Box<Self>, Box<Self>),
    InRange(Box<Self>, i64, i64),
}

impl Expr {
    fn var(name: &str) -> Self {
        Self::Var(name.to_owned())
    }

    fn add(a: Self, b: Self) -> Self {
        Self::Add(Box::new(a), Box::new(b))
    }

    fn sub(a: Self, b: Self) -> Self {
        Self::Sub(Box::new(a), Box::new(b))
    }

    fn min(a: Self, b: Self) -> Self {
        Self::Min(Box::new(a), Box::new(b))
    }

    fn encode(&self, out: &mut Vec<u8>) {
        let two = |out: &mut Vec<u8>, a: &Self, b: &Self| {
            a.encode(out);
            b.encode(out);
        };
        match self {
            Self::Int(value) => {
                out.push(0x01);
                out.extend_from_slice(&value.to_be_bytes());
            }
            Self::Var(name) => {
                out.push(0x02);
                model_token(out, name);
            }
            Self::Add(a, b) => {
                out.extend_from_slice(&[0x03, 0]);
                two(out, a, b);
            }
            Self::Sub(a, b) => {
                out.extend_from_slice(&[0x03, 1]);
                two(out, a, b);
            }
            Self::Min(a, b) => {
                out.push(0x04);
                two(out, a, b);
            }
            Self::Bool(value) => out.extend_from_slice(&[0x10, u8::from(*value)]),
            Self::Eq(a, b) => {
                out.extend_from_slice(&[0x11, 0]);
                two(out, a, b);
            }
            Self::Not(a) => {
                out.push(0x12);
                a.encode(out);
            }
            Self::And(a, b) => {
                out.push(0x13);
                two(out, a, b);
            }
            Self::InRange(a, lo, hi) => {
                out.push(0x16);
                a.encode(out);
                out.extend_from_slice(&lo.to_be_bytes());
                out.extend_from_slice(&hi.to_be_bytes());
            }
        }
    }

    fn rename(&mut self, from: &str, to: &str) {
        match self {
            Self::Var(name) if name == from => *name = to.to_owned(),
            Self::Add(a, b)
            | Self::Sub(a, b)
            | Self::Min(a, b)
            | Self::Eq(a, b)
            | Self::And(a, b) => {
                a.rename(from, to);
                b.rename(from, to);
            }
            Self::Not(a) | Self::InRange(a, ..) => a.rename(from, to),
            _ => {}
        }
    }
}

fn model_count(out: &mut Vec<u8>, n: usize) {
    out.extend_from_slice(&(n as u64).to_be_bytes());
}

fn model_token(out: &mut Vec<u8>, text: &str) {
    model_count(out, text.len());
    out.extend_from_slice(text.as_bytes());
}

/// One planned action: name, guard, outcomes (each a list of assignments).
#[derive(Debug, Clone)]
pub(crate) struct PlannedAction {
    pub(crate) name: String,
    pub(crate) guard: Expr,
    pub(crate) outcomes: Vec<Vec<(String, Expr)>>,
}

/// Every field of a model section, independently settable.
#[derive(Debug, Clone)]
pub(crate) struct ModelPlan {
    pub(crate) tag: Vec<u8>,
    pub(crate) variables: Vec<PlannedVariable>,
    pub(crate) actions: Vec<PlannedAction>,
    pub(crate) initial: Vec<Vec<i64>>,
    pub(crate) predicates: Vec<(String, Expr)>,
    pub(crate) fairness: Vec<(u8, Vec<String>)>,
    pub(crate) trailing: Vec<u8>,
}

impl ModelPlan {
    /// Die Hard, transcribed into the model grammar. Action names match
    /// [`diehard_actions`], so action indices agree with the epoch-1 fixture.
    pub(crate) fn diehard() -> Self {
        let big = || Expr::var("big");
        let small = || Expr::var("small");
        let to_small = || Expr::min(Expr::add(big(), small()), Expr::Int(3));
        let to_big = || Expr::min(Expr::add(big(), small()), Expr::Int(5));
        let action = |name: &str, outcome: Vec<(&str, Expr)>| PlannedAction {
            name: name.to_owned(),
            guard: Expr::Bool(true),
            outcomes: vec![
                outcome
                    .into_iter()
                    .map(|(v, e)| (v.to_owned(), e))
                    .collect(),
            ],
        };
        Self {
            tag: b"continuum-model/1".to_vec(),
            variables: diehard_variables(),
            actions: vec![
                action(
                    "big-to-small",
                    vec![
                        ("big", Expr::sub(big(), Expr::sub(to_small(), small()))),
                        ("small", to_small()),
                    ],
                ),
                action("empty-big", vec![("big", Expr::Int(0))]),
                action("empty-small", vec![("small", Expr::Int(0))]),
                action("fill-big", vec![("big", Expr::Int(5))]),
                action("fill-small", vec![("small", Expr::Int(3))]),
                action(
                    "small-to-big",
                    vec![
                        ("big", to_big()),
                        ("small", Expr::sub(small(), Expr::sub(to_big(), big()))),
                    ],
                ),
            ],
            initial: vec![vec![0, 0]],
            predicates: vec![
                (
                    "BigNotFour".to_owned(),
                    Expr::Not(Box::new(Expr::Eq(Box::new(big()), Box::new(Expr::Int(4))))),
                ),
                (
                    "TypeOK".to_owned(),
                    Expr::And(
                        Box::new(Expr::InRange(Box::new(big()), 0, 5)),
                        Box::new(Expr::InRange(Box::new(small()), 0, 3)),
                    ),
                ),
            ],
            fairness: Vec::new(),
            trailing: Vec::new(),
        }
    }

    /// One variable, one action with two outcomes.
    pub(crate) fn two_outcomes() -> Self {
        Self {
            tag: b"continuum-model/1".to_vec(),
            variables: vec![("x".to_owned(), 0, 3)],
            actions: vec![PlannedAction {
                name: "Pick".to_owned(),
                guard: Expr::Bool(true),
                outcomes: vec![
                    vec![("x".to_owned(), Expr::Int(1))],
                    vec![("x".to_owned(), Expr::Int(2))],
                ],
            }],
            initial: vec![vec![0]],
            predicates: Vec::new(),
            fairness: Vec::new(),
            trailing: Vec::new(),
        }
    }

    /// Rename a variable wherever an expression mentions it (not its declaration).
    pub(crate) fn rename_variable_in_expressions(&mut self, from: &str, to: &str) {
        for action in &mut self.actions {
            action.guard.rename(from, to);
            for outcome in &mut action.outcomes {
                for (_, value) in outcome {
                    value.rename(from, to);
                }
            }
        }
    }

    /// Replace the first guard with `true` nested under `depth - 1` negations, an
    /// expression of exactly `depth` (true when `depth` is odd).
    pub(crate) fn deep_guard(&mut self, depth: usize) {
        let mut guard = Expr::Bool(true);
        for _ in 1..depth {
            guard = Expr::Not(Box::new(guard));
        }
        self.actions[0].guard = guard;
    }

    /// A first guard that overflows in its right operand while its left is `false`.
    pub(crate) fn overflowing_guard(&mut self) {
        self.actions[0].guard = Expr::And(
            Box::new(Expr::Bool(false)),
            Box::new(Expr::Eq(
                Box::new(Expr::add(Expr::Int(i64::MAX), Expr::Int(1))),
                Box::new(Expr::Int(0)),
            )),
        );
    }

    /// The byte offset of the first guard's opcode.
    pub(crate) fn first_guard_offset(&self) -> usize {
        let mut out = self.tag.clone();
        self.encode_variables(&mut out);
        model_count(&mut out, self.actions.len());
        model_token(&mut out, &self.actions[0].name);
        out.len()
    }

    fn encode_variables(&self, out: &mut Vec<u8>) {
        model_count(out, self.variables.len());
        for (name, lo, hi) in &self.variables {
            model_token(out, name);
            out.extend_from_slice(&lo.to_be_bytes());
            out.extend_from_slice(&hi.to_be_bytes());
        }
    }

    /// Serialize the plan exactly as written (outcomes in the order given).
    pub(crate) fn encode(&self) -> Vec<u8> {
        let mut out = self.tag.clone();
        self.encode_variables(&mut out);
        model_count(&mut out, self.actions.len());
        for action in &self.actions {
            model_token(&mut out, &action.name);
            action.guard.encode(&mut out);
            model_count(&mut out, action.outcomes.len());
            for outcome in &action.outcomes {
                model_count(&mut out, outcome.len());
                for (variable, value) in outcome {
                    model_token(&mut out, variable);
                    value.encode(&mut out);
                }
            }
        }
        model_count(&mut out, self.initial.len());
        for state in &self.initial {
            model_count(&mut out, state.len());
            for value in state {
                out.extend_from_slice(&value.to_be_bytes());
            }
        }
        model_count(&mut out, self.predicates.len());
        for (name, body) in &self.predicates {
            model_token(&mut out, name);
            body.encode(&mut out);
        }
        if !self.fairness.is_empty() {
            model_count(&mut out, self.fairness.len());
            for (strength, scope) in &self.fairness {
                out.push(*strength);
                model_count(&mut out, scope.len());
                for name in scope {
                    model_token(&mut out, name);
                }
            }
        }
        out.extend_from_slice(&self.trailing);
        out
    }
}

/// Every field of a wire-epoch-2 finite-closure certificate, independently settable.
#[derive(Debug, Clone)]
pub(crate) struct PlanV2 {
    pub(crate) wire_epoch: u16,
    pub(crate) kind: u16,
    pub(crate) model: Vec<u8>,
    pub(crate) declared_model_len: Option<u32>,
    pub(crate) property_class: u16,
    pub(crate) predicate: Option<String>,
    pub(crate) states: Vec<Vec<i64>>,
    pub(crate) rows: Vec<Vec<(u16, u32)>>,
    pub(crate) trailing: Vec<u8>,
}

impl PlanV2 {
    /// The green Die Hard certificate: the reachable set of the epoch-1 fixture, with
    /// its rows as table indices, over [`ModelPlan::diehard`].
    pub(crate) fn diehard() -> Self {
        let (states, rows) = diehard_reachable();
        let index = |target: &Vec<i64>| {
            u32::try_from(states.iter().position(|s| s == target).unwrap()).unwrap()
        };
        let rows = rows
            .iter()
            .map(|row| {
                let mut row: Vec<(u16, u32)> = row.iter().map(|(a, t)| (*a, index(t))).collect();
                row.sort_unstable();
                row.dedup();
                row
            })
            .collect();
        Self {
            wire_epoch: crate::wire::WIRE_EPOCH,
            kind: 1,
            model: ModelPlan::diehard().encode(),
            declared_model_len: None,
            property_class: 1,
            predicate: None,
            states,
            rows,
            trailing: Vec::new(),
        }
    }

    /// The same certificate for the invariant `name`.
    pub(crate) fn diehard_invariant(name: &str) -> Self {
        Self {
            property_class: 2,
            predicate: Some(name.to_owned()),
            ..Self::diehard()
        }
    }

    /// Serialize the plan exactly as written, with the epoch-1 fixture's envelope.
    pub(crate) fn encode(&self) -> Vec<u8> {
        let envelope = Plan::diehard_closure();
        let mut out = Encoder::new();
        out.bytes(&MAGIC);
        out.u16(self.wire_epoch);
        out.u16(self.kind);
        out.token(&envelope.model_digest);
        out.token(&envelope.semantic_epoch);
        out.token(&envelope.property_digest);
        out.token(&envelope.scope_digest);
        out.token(&envelope.assumptions_digest);
        out.token(&envelope.producer);
        out.u16(self.wire_epoch);
        out.u16(0);
        out.u32(
            self.declared_model_len
                .unwrap_or_else(|| u32::try_from(self.model.len()).unwrap()),
        );
        out.bytes(&self.model);
        out.u16(self.property_class);
        if let Some(name) = &self.predicate {
            out.token(name);
        }
        out.u32(u32::try_from(self.states.len()).unwrap());
        for state in &self.states {
            for value in state {
                out.i64(*value);
            }
        }
        for row in &self.rows {
            out.u32(u32::try_from(row.len()).unwrap());
            for (action, target) in row {
                out.u16(*action);
                out.u32(*target);
            }
        }
        out.bytes(&self.trailing);
        out.into_bytes()
    }
}
