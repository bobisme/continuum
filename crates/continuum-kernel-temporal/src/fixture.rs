//! Test-only certificate producer.
//!
//! The kernel ships a checker, never an encoder: an encoder in the trusted base is the
//! first step towards a producer and a checker that agree because they share code,
//! which is the common-mode failure docs/03 §8 is written against. This module exists
//! only under `cfg(test)`, is not part of the public surface, and is written against
//! the *documented* wire grammar rather than against the decoder's internals.
//!
//! # The fixtures
//!
//! **`drain_ranking`** — a two-counter drain: `a, b ∈ 0..2`, actions `dec-a` and
//! `dec-b`, goal `a = 0`. Progress needs no fairness at all (every action decreases
//! `3a + b`), which is what makes a ranking certificate the right artifact for it. The
//! ranking carried is the *minimal* one — each state's longest distance to the goal set
//! — so no rank can shrink without breaking a decrease obligation.
//!
//! **`fair_progress_exclusion`** and **`broken_progress_exclusion`** — the Revision 2
//! fair-lasso spike's two models, transcribed into wire form.
//! `notes/plan/spikes/advanced_spikes.py` defines `ProgressModel(broken)` with actions
//! `Wait`, `Complete` and `StayDone`, and asks whether a lasso witnesses a violation of
//! `eventually(done)` under weak fairness for `Complete`. Its frozen results
//! (`notes/plan/spikes/results/spike-results.json`, `advanced.temporal_fair_lasso`) are
//! three facts, and the check tests reproduce all three from bytes:
//!
//! ```text
//! unfair_wait_is_raw_liveness_counterexample : true
//! weak_fairness_rejects_wait_cycle           : true
//! broken_model_has_fair_counterexample       : true
//! ```
//!
//! **`unreachable_trap_exclusion`** — a fair cycle that no execution can enter, so that
//! the reachability half of docs/16 PO-LIV-004 is exercised in both directions.

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

    /// Append a big-endian `u64`.
    pub(crate) fn u64(&mut self, value: u64) {
        self.out.extend_from_slice(&value.to_be_bytes());
    }

    /// Append a big-endian `i64`.
    pub(crate) fn i64(&mut self, value: i64) {
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

/// A declared variable: name, inclusive low bound, inclusive high bound.
pub(crate) type PlannedVariable = (String, i64, i64);

/// A declared transition: action index and target state vector.
pub(crate) type PlannedTransition = (u16, Vec<i64>);

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
    pub(crate) variables: Vec<PlannedVariable>,
    pub(crate) declared_variable_count: Option<u16>,
    pub(crate) states: Vec<Vec<i64>>,
    pub(crate) declared_state_count: Option<u32>,
    pub(crate) property_class: u16,
    pub(crate) goal: Vec<Vec<i64>>,
    pub(crate) declared_goal_count: Option<u32>,
    pub(crate) initial: Vec<Vec<i64>>,
    pub(crate) declared_initial_count: Option<u32>,
    pub(crate) actions: Vec<String>,
    pub(crate) declared_action_count: Option<u16>,
    pub(crate) fairness_class: u16,
    pub(crate) fair_actions: Vec<u16>,
    pub(crate) declared_fair_count: Option<u16>,
    pub(crate) rows: Vec<Vec<PlannedTransition>>,
    pub(crate) ranks: Vec<u64>,
    pub(crate) trailing: Vec<u8>,
}

impl Plan {
    /// A skeleton with the envelope filled in and no body.
    fn shell() -> Self {
        Self {
            magic: MAGIC,
            wire_epoch: crate::wire::WIRE_EPOCH,
            kind: 1,
            model_digest: "blake3:temporal-model".to_owned(),
            semantic_epoch: "continuum-semantics-1".to_owned(),
            property_digest: "blake3:eventually-goal".to_owned(),
            scope_digest: "blake3:temporal-scope".to_owned(),
            assumptions_digest: "blake3:empty-assumptions".to_owned(),
            producer: "continuum-engine-liveness/0.0.0".to_owned(),
            schema_epoch: crate::wire::WIRE_EPOCH,
            domain_packs: Vec::new(),
            declared_domain_pack_count: None,
            variables: Vec::new(),
            declared_variable_count: None,
            states: Vec::new(),
            declared_state_count: None,
            property_class: 1,
            goal: Vec::new(),
            declared_goal_count: None,
            initial: Vec::new(),
            declared_initial_count: None,
            actions: Vec::new(),
            declared_action_count: None,
            fairness_class: 1,
            fair_actions: Vec::new(),
            declared_fair_count: None,
            rows: Vec::new(),
            ranks: Vec::new(),
            trailing: Vec::new(),
        }
    }

    /// The green ranking certificate: a two-counter drain with its minimal ranking.
    pub(crate) fn drain_ranking() -> Self {
        let mut states: Vec<Vec<i64>> = Vec::new();
        let mut rows: Vec<Vec<PlannedTransition>> = Vec::new();
        let mut goal: Vec<Vec<i64>> = Vec::new();
        let mut ranks: Vec<u64> = Vec::new();
        for a in 0..=2_i64 {
            for b in 0..=2_i64 {
                states.push(vec![a, b]);
                let mut row: Vec<PlannedTransition> = Vec::new();
                if a > 0 {
                    row.push((0, vec![a - 1, b]));
                }
                if b > 0 {
                    row.push((1, vec![a, b - 1]));
                }
                row.sort();
                rows.push(row);
                if a == 0 {
                    goal.push(vec![a, b]);
                    ranks.push(0);
                } else {
                    // The longest distance to `a = 0`: one `dec-a` per unit of `a`,
                    // plus one `dec-b` per unit of `b` if they are taken first.
                    ranks.push(u64::try_from(a + b).expect("small rank"));
                }
            }
        }
        Self {
            kind: 1,
            model_digest: "blake3:drain-model".to_owned(),
            variables: vec![("a".to_owned(), 0, 2), ("b".to_owned(), 0, 2)],
            states,
            goal,
            initial: vec![vec![2, 2]],
            actions: vec!["dec-a".to_owned(), "dec-b".to_owned()],
            rows,
            ranks,
            ..Self::shell()
        }
    }

    /// The Revision 2 spike's `FairProgress` model: `Complete` is always enabled, so
    /// the `Wait` self-loop is not a fair execution.
    pub(crate) fn fair_progress_exclusion() -> Self {
        Self {
            kind: 2,
            model_digest: "blake3:fair-progress".to_owned(),
            variables: vec![("broken".to_owned(), 0, 1), ("done".to_owned(), 0, 1)],
            states: vec![vec![0, 0], vec![0, 1]],
            goal: vec![vec![0, 1]],
            initial: vec![vec![0, 0]],
            actions: vec![
                "Complete".to_owned(),
                "StayDone".to_owned(),
                "Wait".to_owned(),
            ],
            fair_actions: vec![0],
            rows: vec![
                vec![(0, vec![0, 1]), (2, vec![0, 0])],
                vec![(1, vec![0, 1])],
            ],
            ..Self::shell()
        }
    }

    /// The Revision 2 spike's `BrokenProgress` model: `Complete` is declared but
    /// enabled nowhere, so the `Wait` self-loop discharges weak fairness vacuously and
    /// is a genuine fair lasso.
    pub(crate) fn broken_progress_exclusion() -> Self {
        Self {
            model_digest: "blake3:broken-progress".to_owned(),
            states: vec![vec![1, 0], vec![1, 1]],
            goal: vec![vec![1, 1]],
            initial: vec![vec![1, 0]],
            rows: vec![vec![(2, vec![1, 0])], vec![(1, vec![1, 1])]],
            ..Self::fair_progress_exclusion()
        }
    }

    /// A two-state cycle: `p` toggles between `0` and `1`, and `finish` is enabled at
    /// both.
    ///
    /// The only fixture whose non-goal component has more than one state, so it is the
    /// one that exercises the component search's stack rather than a self-loop. Under
    /// weak fairness for `finish` the toggle cycle is not a legal execution; under weak
    /// fairness for `toggle` alone it is, because `toggle` is taken *inside* the
    /// component and its obligation is therefore discharged by the cycle itself.
    pub(crate) fn ping_pong_exclusion() -> Self {
        Self {
            kind: 2,
            model_digest: "blake3:ping-pong".to_owned(),
            variables: vec![("p".to_owned(), 0, 2)],
            states: vec![vec![0], vec![1], vec![2]],
            goal: vec![vec![2]],
            initial: vec![vec![0]],
            actions: vec!["finish".to_owned(), "toggle".to_owned()],
            fair_actions: vec![0],
            rows: vec![
                vec![(0, vec![2]), (1, vec![1])],
                vec![(0, vec![2]), (1, vec![0])],
                vec![(1, vec![2])],
            ],
            ..Self::shell()
        }
    }

    /// A fair cycle that no execution can enter.
    ///
    /// `[1,0]` sits on a `stay` self-loop where `go` is disabled, so it would be a fair
    /// counterexample — but nothing reaches it from `[0,0]`.
    pub(crate) fn unreachable_trap_exclusion() -> Self {
        Self {
            kind: 2,
            model_digest: "blake3:unreachable-trap".to_owned(),
            variables: vec![("a".to_owned(), 0, 1), ("b".to_owned(), 0, 1)],
            states: vec![vec![0, 0], vec![0, 1], vec![1, 0], vec![1, 1]],
            goal: vec![vec![0, 1]],
            initial: vec![vec![0, 0]],
            actions: vec!["go".to_owned(), "stay".to_owned()],
            fair_actions: vec![0],
            rows: vec![
                vec![(0, vec![0, 1])],
                vec![(1, vec![0, 1])],
                vec![(1, vec![1, 0])],
                vec![(1, vec![1, 1])],
            ],
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
        encode_vectors(&mut out, &self.states);

        out.u16(self.property_class);

        out.u32(
            self.declared_goal_count
                .unwrap_or_else(|| u32::try_from(self.goal.len()).unwrap_or(u32::MAX)),
        );
        encode_vectors(&mut out, &self.goal);

        out.u32(
            self.declared_initial_count
                .unwrap_or_else(|| u32::try_from(self.initial.len()).unwrap_or(u32::MAX)),
        );
        encode_vectors(&mut out, &self.initial);

        out.u16(
            self.declared_action_count
                .unwrap_or_else(|| u16::try_from(self.actions.len()).unwrap_or(u16::MAX)),
        );
        for action in &self.actions {
            out.token(action);
        }

        out.u16(self.fairness_class);
        out.u16(
            self.declared_fair_count
                .unwrap_or_else(|| u16::try_from(self.fair_actions.len()).unwrap_or(u16::MAX)),
        );
        for action in &self.fair_actions {
            out.u16(*action);
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

        if self.kind == 1 {
            for rank in &self.ranks {
                out.u64(*rank);
            }
        }

        out.bytes(&self.trailing);
        out.into_bytes()
    }
}

/// Write a sequence of state vectors, flat.
fn encode_vectors(out: &mut Encoder, states: &[Vec<i64>]) {
    for state in states {
        for value in state {
            out.i64(*value);
        }
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
