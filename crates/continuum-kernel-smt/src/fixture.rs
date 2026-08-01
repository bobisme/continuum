//! Test-only certificate producer.
//!
//! The kernel ships a checker, never an encoder: an encoder in the trusted base is the
//! first step towards a producer and a checker that agree because they share code,
//! which is the common-mode failure docs/03 §8 is written against. This module exists
//! only under `cfg(test)`, is not part of the public surface, and is written against
//! the *documented* wire grammar rather than against the decoder's internals — it emits
//! whatever [`Plan`] says, including atoms, lemmas and chains the decoder must refuse.
//!
//! # The fixture problem
//!
//! `x < y`, `x = y ∨ y < x`, `¬(f(x) = f(y))` — satisfiable as a propositional
//! skeleton, unsatisfiable once ordering and congruence are available. It is the
//! smallest shape that makes the crate's whole point visible: the Boolean part is
//! replayed here in full, the two theory steps are not, and the verdict says so.
//!
//! ```text
//! atoms   1 f-x-eq-f-y   2 x-eq-y   3 x-lt-y   4 y-lt-x
//!
//! 1 (x<y)                    assertion
//! 2 (x=y ∨ y<x)              assertion
//! 3 (¬(f(x)=f(y)))           assertion
//! 4 (¬(x<y) ∨ ¬(y<x))        theory lemma, LIA   -- ordering is antisymmetric
//! 5 (f(x)=f(y) ∨ ¬(x=y))     theory lemma, UF    -- congruence
//! 6 (¬(y<x))                 from 1, 4
//! 7 (¬(x=y))                 from 3, 5
//! 8 ()                       from 6, 7, 2
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

    /// Append a length-prefixed token, exactly as given.
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

/// A planned resolution step.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PlannedResolution {
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
    /// A resolution step.
    Resolve(PlannedResolution),
    /// A deletion of the named identifiers.
    Delete(Vec<u32>),
    /// A step whose kind code is written verbatim, with no payload.
    RawKind(u16),
}

/// A planned theory lemma: theory name and clause.
pub(crate) type PlannedLemma = (String, Vec<i32>);

/// Every field of a certificate, independently settable.
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
    pub(crate) atoms: Vec<String>,
    pub(crate) declared_atom_count: Option<u32>,
    pub(crate) assertions: Vec<Vec<i32>>,
    pub(crate) declared_assertion_count: Option<u32>,
    pub(crate) lemmas: Vec<PlannedLemma>,
    pub(crate) declared_lemma_count: Option<u32>,
    pub(crate) steps: Vec<PlannedStep>,
    pub(crate) declared_step_count: Option<u32>,
    pub(crate) trailing: Vec<u8>,
}

impl Plan {
    /// A skeleton with the envelope filled in and no body.
    fn shell() -> Self {
        Self {
            magic: MAGIC,
            wire_epoch: crate::wire::WIRE_EPOCH,
            kind: 1,
            model_digest: "blake3:ordering-model".to_owned(),
            semantic_epoch: "continuum-semantics-1".to_owned(),
            property_digest: "blake3:ordering-property".to_owned(),
            scope_digest: "blake3:ordering-scope".to_owned(),
            assumptions_digest: "blake3:empty-assumptions".to_owned(),
            producer: "continuum-solver-adapter/0.0.0".to_owned(),
            schema_epoch: crate::wire::WIRE_EPOCH,
            domain_packs: Vec::new(),
            declared_domain_pack_count: None,
            atoms: Vec::new(),
            declared_atom_count: None,
            assertions: Vec::new(),
            declared_assertion_count: None,
            lemmas: Vec::new(),
            declared_lemma_count: None,
            steps: Vec::new(),
            declared_step_count: None,
            trailing: Vec::new(),
        }
    }

    /// The green certificate: an ordering lemma and a congruence lemma, both used.
    pub(crate) fn ordering_and_congruence() -> Self {
        Self {
            atoms: vec![
                "f-x-eq-f-y".to_owned(),
                "x-eq-y".to_owned(),
                "x-lt-y".to_owned(),
                "y-lt-x".to_owned(),
            ],
            assertions: vec![vec![3], vec![2, 4], vec![-1]],
            lemmas: vec![
                ("LIA".to_owned(), vec![-3, -4]),
                ("UF".to_owned(), vec![1, -2]),
            ],
            steps: vec![
                PlannedStep::Resolve(PlannedResolution {
                    id: 6,
                    clause: vec![-4],
                    hints: vec![1, 4],
                }),
                PlannedStep::Resolve(PlannedResolution {
                    id: 7,
                    clause: vec![-2],
                    hints: vec![3, 5],
                }),
                PlannedStep::Resolve(PlannedResolution {
                    id: 8,
                    clause: Vec::new(),
                    hints: vec![6, 7, 2],
                }),
            ],
            ..Self::shell()
        }
    }

    /// A conflict among the assertions alone: no theory is needed, so none is trusted.
    pub(crate) fn propositional_conflict() -> Self {
        Self {
            model_digest: "blake3:trichotomy-model".to_owned(),
            atoms: vec!["x-eq-0".to_owned(), "x-gt-0".to_owned()],
            assertions: vec![vec![1, 2], vec![-1], vec![-2]],
            lemmas: Vec::new(),
            // A gap in the identifier space is legal: identifiers must increase, not
            // be consecutive. The gap keeps this fixture's proof valid when a test
            // appends a lemma and shifts the input range.
            steps: vec![PlannedStep::Resolve(PlannedResolution {
                id: 10,
                clause: Vec::new(),
                hints: vec![2, 3, 1],
            })],
            ..Self::shell()
        }
    }

    /// The degenerate honest case: the solver's answer arrives as a single theory
    /// lemma, and the verdict says so.
    pub(crate) fn solver_says_so() -> Self {
        Self {
            model_digest: "blake3:opaque-model".to_owned(),
            atoms: vec!["goal-holds".to_owned()],
            assertions: vec![vec![1]],
            lemmas: vec![("QF_UFLIA".to_owned(), vec![-1])],
            steps: vec![PlannedStep::Resolve(PlannedResolution {
                id: 5,
                clause: Vec::new(),
                hints: vec![1, 2],
            })],
            ..Self::shell()
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

        out.u32(
            self.declared_atom_count
                .unwrap_or_else(|| u32::try_from(self.atoms.len()).unwrap_or(u32::MAX)),
        );
        for atom in &self.atoms {
            out.token(atom);
        }

        out.u32(
            self.declared_assertion_count
                .unwrap_or_else(|| u32::try_from(self.assertions.len()).unwrap_or(u32::MAX)),
        );
        for clause in &self.assertions {
            encode_clause(&mut out, clause);
        }

        out.u32(
            self.declared_lemma_count
                .unwrap_or_else(|| u32::try_from(self.lemmas.len()).unwrap_or(u32::MAX)),
        );
        for (theory, clause) in &self.lemmas {
            out.token(theory);
            encode_clause(&mut out, clause);
        }

        out.u32(
            self.declared_step_count
                .unwrap_or_else(|| u32::try_from(self.steps.len()).unwrap_or(u32::MAX)),
        );
        for step in &self.steps {
            match step {
                PlannedStep::Resolve(resolution) => {
                    out.u16(1);
                    out.u32(resolution.id);
                    encode_clause(&mut out, &resolution.clause);
                    out.u32(u32::try_from(resolution.hints.len()).unwrap_or(u32::MAX));
                    for hint in &resolution.hints {
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
/// evidence.
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
