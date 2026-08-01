//! Test-only certificate producer.
//!
//! The kernel ships a checker, never an encoder: an encoder in the trusted base is the
//! first step towards a producer and a checker that agree because they share code,
//! which is the common-mode failure docs/03 §8 is written against. This module exists
//! only under `cfg(test)`, is not part of the public surface, and is deliberately
//! written against the *documented* wire grammar rather than against the decoder's
//! internals — it emits whatever [`Plan`] says, including sequences, identifiers and
//! chains the decoder must refuse.
//!
//! # The fixture formula
//!
//! The complete three-variable CNF: all eight clauses over `x₁ x₂ x₃`. It is
//! unsatisfiable because the eight clauses forbid all eight assignments, and its
//! refutation is a resolution tree whose every step is a two-antecedent unit
//! propagation, so a reader can check the whole certificate by hand. That is the
//! property a kernel fixture needs; the Phase A corpus
//! (`notes/plan/corpus/tla-examples/`) is a TLA+ model corpus and contains no CNF
//! instance to port, so the fixture is stated here rather than cited.
//!
//! ```text
//!  1 (x1 ∨ x2 ∨ x3)      9  (x1 ∨ x2)     from 1, 2
//!  2 (x1 ∨ x2 ∨ ¬x3)     10 (x1 ∨ ¬x2)    from 3, 4
//!  3 (x1 ∨ ¬x2 ∨ x3)     11 (x1)          from 9, 10
//!  4 (x1 ∨ ¬x2 ∨ ¬x3)    -- delete 1 2 3 4 9 10
//!  5 (¬x1 ∨ x2 ∨ x3)     12 (¬x1 ∨ x2)    from 5, 6
//!  6 (¬x1 ∨ x2 ∨ ¬x3)    13 (¬x1 ∨ ¬x2)   from 7, 8
//!  7 (¬x1 ∨ ¬x2 ∨ x3)    14 (¬x1)         from 12, 13
//!  8 (¬x1 ∨ ¬x2 ∨ ¬x3)   15 ()            from 11, 14
//! ```

#![allow(
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::arithmetic_side_effects,
    reason = "a test-only producer builds fixtures of known shape; the no-panic \
              covenant is a property of the shipped checker, not of its harness"
)]

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

    /// Append a big-endian `i32`.
    pub(crate) fn i32(&mut self, value: i32) {
        self.out.extend_from_slice(&value.to_be_bytes());
    }

    /// Append a length-prefixed token, exactly as given — including lengths and bytes
    /// the decoder must reject.
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

/// A planned clause addition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PlannedAdd {
    /// The identifier the derived clause takes.
    pub(crate) id: u32,
    /// The derived clause's literals, written verbatim.
    pub(crate) clause: Vec<i32>,
    /// The antecedent chain, written verbatim.
    pub(crate) hints: Vec<u32>,
}

/// A planned proof step.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum PlannedStep {
    /// A clause addition.
    Add(PlannedAdd),
    /// A deletion of the named identifiers.
    Delete(Vec<u32>),
    /// A step whose kind code is written verbatim, with no payload — for exercising
    /// the unsupported-step path.
    RawKind(u16),
}

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
    pub(crate) model_digest: String,
    pub(crate) semantic_epoch: String,
    pub(crate) property_digest: String,
    pub(crate) scope_digest: String,
    pub(crate) assumptions_digest: String,
    pub(crate) producer: String,
    pub(crate) schema_epoch: u16,
    pub(crate) domain_packs: Vec<String>,
    pub(crate) declared_domain_pack_count: Option<u16>,
    pub(crate) variables: u32,
    pub(crate) clauses: Vec<Vec<i32>>,
    pub(crate) declared_clause_count: Option<u32>,
    pub(crate) steps: Vec<PlannedStep>,
    pub(crate) declared_step_count: Option<u32>,
    pub(crate) trailing: Vec<u8>,
}

impl Plan {
    /// The green refutation of the complete three-variable CNF.
    pub(crate) fn three_variable_refutation() -> Self {
        Self {
            magic: MAGIC,
            wire_epoch: crate::wire::WIRE_EPOCH,
            kind: 1,
            model_digest: "blake3:three-variable-cnf".to_owned(),
            semantic_epoch: "continuum-semantics-1".to_owned(),
            property_digest: "blake3:unsatisfiable".to_owned(),
            scope_digest: "blake3:three-variables".to_owned(),
            assumptions_digest: "blake3:empty-assumptions".to_owned(),
            producer: "continuum-solver-adapter/0.0.0".to_owned(),
            schema_epoch: crate::wire::WIRE_EPOCH,
            domain_packs: Vec::new(),
            declared_domain_pack_count: None,
            variables: 3,
            clauses: vec![
                vec![1, 2, 3],
                vec![1, 2, -3],
                vec![1, -2, 3],
                vec![1, -2, -3],
                vec![-1, 2, 3],
                vec![-1, 2, -3],
                vec![-1, -2, 3],
                vec![-1, -2, -3],
            ],
            declared_clause_count: None,
            steps: vec![
                PlannedStep::Add(PlannedAdd {
                    id: 9,
                    clause: vec![1, 2],
                    hints: vec![1, 2],
                }),
                PlannedStep::Add(PlannedAdd {
                    id: 10,
                    clause: vec![1, -2],
                    hints: vec![3, 4],
                }),
                PlannedStep::Add(PlannedAdd {
                    id: 11,
                    clause: vec![1],
                    hints: vec![9, 10],
                }),
                PlannedStep::Delete(vec![1, 2, 3, 4, 9, 10]),
                PlannedStep::Add(PlannedAdd {
                    id: 12,
                    clause: vec![-1, 2],
                    hints: vec![5, 6],
                }),
                PlannedStep::Add(PlannedAdd {
                    id: 13,
                    clause: vec![-1, -2],
                    hints: vec![7, 8],
                }),
                PlannedStep::Add(PlannedAdd {
                    id: 14,
                    clause: vec![-1],
                    hints: vec![12, 13],
                }),
                PlannedStep::Add(PlannedAdd {
                    id: 15,
                    clause: Vec::new(),
                    hints: vec![11, 14],
                }),
            ],
            declared_step_count: None,
            trailing: Vec::new(),
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

        out.u32(self.variables);
        out.u32(
            self.declared_clause_count
                .unwrap_or_else(|| u32::try_from(self.clauses.len()).unwrap_or(u32::MAX)),
        );
        for clause in &self.clauses {
            encode_clause(&mut out, clause);
        }

        out.u32(
            self.declared_step_count
                .unwrap_or_else(|| u32::try_from(self.steps.len()).unwrap_or(u32::MAX)),
        );
        for step in &self.steps {
            match step {
                PlannedStep::Add(addition) => {
                    out.u16(1);
                    out.u32(addition.id);
                    encode_clause(&mut out, &addition.clause);
                    out.u32(u32::try_from(addition.hints.len()).unwrap_or(u32::MAX));
                    for hint in &addition.hints {
                        out.u32(*hint);
                    }
                }
                PlannedStep::Delete(ids) => {
                    out.u16(2);
                    out.u32(u32::try_from(ids.len()).unwrap_or(u32::MAX));
                    for id in ids {
                        out.u32(*id);
                    }
                }
                PlannedStep::RawKind(code) => out.u16(*code),
            }
        }

        out.bytes(&self.trailing);
        out.into_bytes()
    }
}

/// Write one length-prefixed clause.
fn encode_clause(out: &mut Encoder, literals: &[i32]) {
    out.u32(u32::try_from(literals.len()).unwrap_or(u32::MAX));
    for literal in literals {
        out.i32(*literal);
    }
}

/// A seeded xorshift generator.
///
/// Hand-rolled on purpose: docs/12 §1 forbids ambient randomness in the semantic core
/// (INV-005, ADR-0003), and a fuzz corpus that cannot be replayed byte for byte is not
/// evidence. The same seed always produces the same corpus.
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
