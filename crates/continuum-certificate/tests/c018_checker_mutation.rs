//! C018 checker mutation campaign: corrupted certificates of every checked class,
//! through the real composed checker, judged by an oracle the checker does not share
//! (bn-2npu).
//!
//! # What this file establishes
//!
//! The trusted checking base is `continuum-certificate` and the four
//! `continuum-kernel-*` crates. docs/18 C018 asks for "independent checker audit and
//! mutation". This file is the mutation half that runs over the real kernel code. It
//! does four things for each of the six certificate classes the kernels check today
//! (finite closure, state type, LRAT, SMT proof, ranking, fair-SCC exclusion; the
//! last has a second green with an unreachable trap):
//!
//! 1. **Green.** A hand-encoded certificate whose claim is true verifies, and the
//!    oracle agrees that the claim holds.
//! 2. **Lies.** Each certificate in a named corpus carries a false claim. The oracle
//!    confirms that it is false, so the corpus is not vacuous. The checker must not
//!    verify any of them, and it must reject each one for the reason the lie targets.
//!    One lie is the known exception: an SMT theory lemma is trusted, not checked, so a
//!    false lemma verifies. The claim then reports `trusted-solver` assurance and names
//!    the theory. The ledger records that row as not caught.
//! 3. **Byte sweep.** Every byte of every green certificate is XORed with `0x01`,
//!    `0x80` and `0xff`. For each mutant that verifies, the oracle re-derives the
//!    carried claim from the decoded mutant. No mutant may verify a claim that the
//!    oracle refutes.
//! 4. **Ledger.** Every outcome is rendered into
//!    `tests/golden/c018_checker_ledger.txt` and compared byte for byte.
//!
//! # The oracle, and what it shares with the checker
//!
//! The oracle re-derives each claim by a different method. It is not free of shared
//! assumptions, and they are listed here. SAT and SMT refutations are
//! decided by brute-force enumeration over the variables that occur, not by replaying
//! unit propagation. A ranking claim is decided by cycle and dead-end search plus a
//! longest-path bound, not by comparing ranks. A fair-SCC exclusion claim is decided by
//! enumerating every subset of reachable non-goal states, not by Tarjan's search. The
//! closure conjuncts are re-checked by linear scans, not binary search.
//!
//! The shared parts: the fair-SCC oracle uses the claim's scope (states reached
//! without passing through a goal state), because that scope is what `eventually goal`
//! means. The ranking oracle checks the bound the kernel reports, not a bound of its
//! own. And an accepted mutant can carry a relation that is not the producer's, such as
//! a transition target moved to another table state. The oracle judges the claim about
//! the carried structure, as the kernel does, so it cannot see that. The producer
//! campaign (`c018_producer_corpus.rs`) measures that gap.
//!
//! The oracle reads the mutant through the kernel's own public decoder
//! (`wire::decode`). So it judges the checker's rules, not its decoder. The decoder is
//! covered by the rejection histogram in the ledger and by the source mutants that
//! `tools/check_tcb_audit.py --evidence` applies to a scratch copy of the kernels.
//!
//! # Its role in the source-mutation lane
//!
//! `tools/check_tcb_audit.py --evidence` copies the checking base into a scratch
//! workspace, applies one source mutant at a time from `tools/tcb-audit/mutants.toml`,
//! and runs this test target. The test names that fail classify the kill:
//! `no_lie_verifies` and `no_single_byte_mutant_verifies_a_claim_the_oracle_refutes`
//! are soundness kills, `every_green_certificate_verifies_and_the_oracle_agrees` is a
//! completeness kill, `every_lie_is_rejected_for_the_reason_it_targets` is a
//! precision kill, and `the_c018_checker_ledger_matches_the_golden` alone is a
//! behaviour-drift kill.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::Path;

use continuum_certificate::{KernelVerdict, Outcome, check_certificate};
use continuum_certificate::{
    continuum_kernel_core as kcore, continuum_kernel_sat as ksat, continuum_kernel_smt as ksmt,
    continuum_kernel_temporal as ktmp,
};

const GOLDEN: &str = include_str!("golden/c018_checker_ledger.txt");

/// The committed corpus: every green and every lie, as hex, with its recorded verdict.
const CORPUS: &str = include_str!("c018-corpus/cases.txt");

/// The XOR masks of the byte sweep: the low bit, the sign bit, every bit.
const MASKS: [u8; 3] = [0x01, 0x80, 0xff];

// --- a minimal encoder, written from the kernels' documented grammars -------------------

#[derive(Default)]
struct Bytes(Vec<u8>);

impl Bytes {
    fn u16(&mut self, value: u16) {
        self.0.extend_from_slice(&value.to_be_bytes());
    }

    fn u32(&mut self, value: u32) {
        self.0.extend_from_slice(&value.to_be_bytes());
    }

    fn u64(&mut self, value: u64) {
        self.0.extend_from_slice(&value.to_be_bytes());
    }

    fn i32(&mut self, value: i32) {
        self.0.extend_from_slice(&value.to_be_bytes());
    }

    fn i64(&mut self, value: i64) {
        self.0.extend_from_slice(&value.to_be_bytes());
    }

    fn len32(&mut self, len: usize) {
        self.u32(u32::try_from(len).expect("fixture sequences are short"));
    }

    fn len16(&mut self, len: usize) {
        self.u16(u16::try_from(len).expect("fixture sequences are short"));
    }

    fn token(&mut self, value: &str) {
        self.len16(value.len());
        self.0.extend_from_slice(value.as_bytes());
    }

    fn state(&mut self, state: &[i64]) {
        for value in state {
            self.i64(*value);
        }
    }

    fn clause(&mut self, literals: &[i32]) {
        self.len32(literals.len());
        for literal in literals {
            self.i32(*literal);
        }
    }

    /// Header plus the eight-field RFC 0005 envelope every family shares.
    fn header(&mut self, magic: &[u8; 8], kind: u16, model: &str) {
        self.header_at(magic, 1, kind, model);
    }

    /// As [`Self::header`], at wire epoch `epoch`.
    fn header_at(&mut self, magic: &[u8; 8], epoch: u16, kind: u16, model: &str) {
        self.0.extend_from_slice(magic);
        self.u16(epoch); // wire epoch
        self.u16(kind);
        self.token(model);
        self.token("continuum-semantics-1");
        self.token("blake3:c018-property");
        self.token("blake3:c018-scope");
        self.token("blake3:empty-assumptions");
        self.token("c018-hand-encoder/0");
        self.u16(epoch); // schema epoch
        self.u16(0); // no domain packs
    }

    fn domain(&mut self, variables: &[(&str, i64, i64)]) {
        self.len16(variables.len());
        for (name, lo, hi) in variables {
            self.token(name);
            self.i64(*lo);
            self.i64(*hi);
        }
    }

    fn states(&mut self, states: &[Vec<i64>]) {
        self.len32(states.len());
        for state in states {
            self.state(state);
        }
    }

    fn actions(&mut self, actions: &[&str]) {
        self.len16(actions.len());
        for action in actions {
            self.token(action);
        }
    }

    fn rows(&mut self, rows: &[Vec<(u16, Vec<i64>)>]) {
        for row in rows {
            self.len32(row.len());
            for (action, target) in row {
                self.u16(*action);
                self.state(target);
            }
        }
    }
}

type Rows = Vec<Vec<(u16, Vec<i64>)>>;

/// `CONTCERT`: kind 1 finite closure, kind 2 state type.
#[derive(Clone)]
struct CoreSpec {
    kind: u16,
    variables: Vec<(&'static str, i64, i64)>,
    states: Vec<Vec<i64>>,
    initial: Vec<Vec<i64>>,
    actions: Vec<&'static str>,
    rows: Rows,
}

impl CoreSpec {
    fn encode(&self) -> Vec<u8> {
        let mut out = Bytes::default();
        out.header(b"CONTCERT", self.kind, "blake3:c018-counter");
        out.domain(&self.variables);
        out.states(&self.states);
        if self.kind == 1 {
            out.u16(1); // property class: state-domain
            out.states(&self.initial);
            out.actions(&self.actions);
            out.rows(&self.rows);
        }
        out.0
    }
}

// --- wire epoch 2: the model section (continuum-model/1) and the model-bound body ----

/// Model-section expression bytes, written from the grammar in
/// `continuum-model-core/src/identity.rs`'s documentation.
mod expr {
    pub fn int(value: i64) -> Vec<u8> {
        let mut out = vec![0x01];
        out.extend_from_slice(&value.to_be_bytes());
        out
    }

    pub fn var(name: &str) -> Vec<u8> {
        let mut out = vec![0x02];
        out.extend_from_slice(&(name.len() as u64).to_be_bytes());
        out.extend_from_slice(name.as_bytes());
        out
    }

    pub fn add(a: &[u8], b: &[u8]) -> Vec<u8> {
        [&[0x03, 0][..], a, b].concat()
    }

    /// `cmp`: 0 eq, 1 ne, 2 lt, 3 le, 4 gt, 5 ge.
    pub fn compare(op: u8, a: &[u8], b: &[u8]) -> Vec<u8> {
        [&[0x11, op][..], a, b].concat()
    }

    pub fn within(a: &[u8], lo: i64, hi: i64) -> Vec<u8> {
        [&[0x16][..], a, &lo.to_be_bytes(), &hi.to_be_bytes()].concat()
    }
}

/// One action: name, guard bytes, outcomes of `(variable, value bytes)`.
type ModelAction = (&'static str, Vec<u8>, Vec<Vec<(&'static str, Vec<u8>)>>);

#[derive(Clone)]
struct ModelSpec {
    variables: Vec<(&'static str, i64, i64)>,
    actions: Vec<ModelAction>,
    initial: Vec<Vec<i64>>,
    predicates: Vec<(&'static str, Vec<u8>)>,
}

impl ModelSpec {
    fn encode(&self) -> Vec<u8> {
        let mut out = Bytes::default();
        let name = |out: &mut Bytes, text: &str| {
            out.u64(text.len() as u64);
            out.0.extend_from_slice(text.as_bytes());
        };
        out.0.extend_from_slice(b"continuum-model/1");
        out.u64(self.variables.len() as u64);
        for (variable, lo, hi) in &self.variables {
            name(&mut out, variable);
            out.i64(*lo);
            out.i64(*hi);
        }
        out.u64(self.actions.len() as u64);
        for (action, guard, outcomes) in &self.actions {
            name(&mut out, action);
            out.0.extend_from_slice(guard);
            out.u64(outcomes.len() as u64);
            for outcome in outcomes {
                out.u64(outcome.len() as u64);
                for (variable, value) in outcome {
                    name(&mut out, variable);
                    out.0.extend_from_slice(value);
                }
            }
        }
        out.u64(self.initial.len() as u64);
        for state in &self.initial {
            out.u64(state.len() as u64);
            out.state(state);
        }
        out.u64(self.predicates.len() as u64);
        for (predicate, body) in &self.predicates {
            name(&mut out, predicate);
            out.0.extend_from_slice(body);
        }
        out.0
    }
}

/// `CONTCERT` at wire epoch 2: kind 1 finite closure over a carried model.
#[derive(Clone)]
struct ModelClosureSpec {
    model: ModelSpec,
    invariant: Option<&'static str>,
    states: Vec<Vec<i64>>,
    rows: Vec<Vec<(u16, u32)>>,
}

impl ModelClosureSpec {
    fn encode(&self) -> Vec<u8> {
        let mut out = Bytes::default();
        out.header_at(b"CONTCERT", 2, 1, "blake3:c018-counter");
        let model = self.model.encode();
        out.len32(model.len());
        out.0.extend_from_slice(&model);
        match self.invariant {
            Some(name) => {
                out.u16(2);
                out.token(name);
            }
            None => out.u16(1),
        }
        out.states(&self.states);
        for row in &self.rows {
            out.len32(row.len());
            for (action, target) in row {
                out.u16(*action);
                out.u32(*target);
            }
        }
        out.0
    }
}

#[derive(Clone)]
enum ProofStep {
    Add(u32, Vec<i32>, Vec<u32>),
    Delete(Vec<u32>),
}

fn encode_steps(out: &mut Bytes, steps: &[ProofStep]) {
    out.len32(steps.len());
    for step in steps {
        match step {
            ProofStep::Add(id, clause, hints) => {
                out.u16(1);
                out.u32(*id);
                out.clause(clause);
                out.len32(hints.len());
                for hint in hints {
                    out.u32(*hint);
                }
            }
            ProofStep::Delete(ids) => {
                out.u16(2);
                out.len32(ids.len());
                for id in ids {
                    out.u32(*id);
                }
            }
        }
    }
}

/// `CONTSATC`: kind 1 LRAT.
#[derive(Clone)]
struct SatSpec {
    variables: u32,
    clauses: Vec<Vec<i32>>,
    steps: Vec<ProofStep>,
}

impl SatSpec {
    fn encode(&self) -> Vec<u8> {
        let mut out = Bytes::default();
        out.header(b"CONTSATC", 1, "blake3:c018-xor");
        out.u32(self.variables);
        out.len32(self.clauses.len());
        for clause in &self.clauses {
            out.clause(clause);
        }
        encode_steps(&mut out, &self.steps);
        out.0
    }
}

/// `CONTSMTC`: kind 1 SMT proof.
#[derive(Clone)]
struct SmtSpec {
    atoms: Vec<&'static str>,
    assertions: Vec<Vec<i32>>,
    lemmas: Vec<(&'static str, Vec<i32>)>,
    steps: Vec<ProofStep>,
}

impl SmtSpec {
    fn encode(&self) -> Vec<u8> {
        let mut out = Bytes::default();
        out.header(b"CONTSMTC", 1, "blake3:c018-order");
        out.len32(self.atoms.len());
        for atom in &self.atoms {
            out.token(atom);
        }
        out.len32(self.assertions.len());
        for clause in &self.assertions {
            out.clause(clause);
        }
        out.len32(self.lemmas.len());
        for (theory, clause) in &self.lemmas {
            out.token(theory);
            out.clause(clause);
        }
        encode_steps(&mut out, &self.steps);
        out.0
    }
}

/// `CONTTMPC`: kind 1 ranking, kind 2 fair-SCC exclusion.
#[derive(Clone)]
struct TemporalSpec {
    kind: u16,
    variables: Vec<(&'static str, i64, i64)>,
    states: Vec<Vec<i64>>,
    goal: Vec<Vec<i64>>,
    initial: Vec<Vec<i64>>,
    actions: Vec<&'static str>,
    fair: Vec<u16>,
    rows: Rows,
    ranks: Vec<u64>,
}

impl TemporalSpec {
    fn encode(&self) -> Vec<u8> {
        let mut out = Bytes::default();
        out.header(b"CONTTMPC", self.kind, "blake3:c018-progress");
        out.domain(&self.variables);
        out.states(&self.states);
        out.u16(1); // property class: eventually-state-set
        out.states(&self.goal);
        out.states(&self.initial);
        out.actions(&self.actions);
        out.u16(1); // weak fairness
        out.len16(self.fair.len());
        for action in &self.fair {
            out.u16(*action);
        }
        out.rows(&self.rows);
        if self.kind == 1 {
            for rank in &self.ranks {
                out.u64(*rank);
            }
        }
        out.0
    }
}

// --- the six green certificates --------------------------------------------------------

/// `x` counts `0 → 1 → 2` by `inc` and returns by `reset`; the domain admits `3`.
fn core_closure() -> CoreSpec {
    CoreSpec {
        kind: 1,
        variables: vec![("x", 0, 3)],
        states: vec![vec![0], vec![1], vec![2]],
        initial: vec![vec![0]],
        actions: vec!["inc", "reset"],
        rows: vec![vec![(0, vec![1])], vec![(0, vec![2])], vec![(1, vec![0])]],
    }
}

fn core_state_type() -> CoreSpec {
    CoreSpec {
        kind: 2,
        variables: vec![("x", 0, 3), ("y", -1, 1)],
        states: vec![vec![0, -1], vec![2, 1], vec![3, 0]],
        initial: Vec::new(),
        actions: Vec::new(),
        rows: Vec::new(),
    }
}

/// The counter of [`core_closure`] as a model: `inc` while `x < 2`, `reset` at
/// `x = 2`. `Small` (`x in 0..2`) holds on the reachable set; `NotTwo` does not. `y`
/// is `1` and no action assigns it, so the frame rule is observable.
fn counter_model() -> ModelSpec {
    let x = || expr::var("x");
    ModelSpec {
        variables: vec![("x", 0, 3), ("y", 0, 1)],
        actions: vec![
            (
                "inc",
                expr::compare(2, &x(), &expr::int(2)),
                vec![vec![("x", expr::add(&x(), &expr::int(1)))]],
            ),
            (
                "reset",
                expr::compare(0, &x(), &expr::int(2)),
                vec![vec![("x", expr::int(0))]],
            ),
        ],
        initial: vec![vec![0, 1]],
        predicates: vec![
            ("NotTwo", expr::compare(1, &x(), &expr::int(2))),
            ("Small", expr::within(&x(), 0, 2)),
        ],
    }
}

fn core_model_closure() -> ModelClosureSpec {
    ModelClosureSpec {
        model: counter_model(),
        invariant: None,
        states: vec![vec![0, 1], vec![1, 1], vec![2, 1]],
        rows: vec![vec![(0, 1)], vec![(0, 2)], vec![(1, 0)]],
    }
}

fn core_model_invariant() -> ModelClosureSpec {
    ModelClosureSpec {
        invariant: Some("Small"),
        ..core_model_closure()
    }
}

/// `(x1 ∨ x2)(¬x1 ∨ x2)(x1 ∨ ¬x2)(¬x1 ∨ ¬x2)`: derive `(x2)`, delete clause 1, then
/// derive the empty clause from `(x2)`, clause 3 and clause 4.
fn sat_lrat() -> SatSpec {
    SatSpec {
        variables: 2,
        clauses: vec![vec![1, 2], vec![-1, 2], vec![1, -2], vec![-1, -2]],
        steps: vec![
            ProofStep::Add(5, vec![2], vec![1, 2]),
            ProofStep::Delete(vec![1]),
            ProofStep::Add(6, vec![], vec![5, 3, 4]),
        ],
    }
}

/// `a<b`, `b<c`, `c<a`, refuted by one `LIA` transitivity lemma.
fn smt_proof() -> SmtSpec {
    SmtSpec {
        atoms: vec!["a-lt-b", "b-lt-c", "c-lt-a"],
        assertions: vec![vec![1], vec![2], vec![3]],
        lemmas: vec![("LIA", vec![-1, -2, -3])],
        steps: vec![ProofStep::Add(5, vec![], vec![1, 2, 3, 4])],
    }
}

/// The countdown `n ∈ 0..3` with two actions and a minimal ranking.
fn temporal_ranking() -> TemporalSpec {
    TemporalSpec {
        kind: 1,
        variables: vec![("n", 0, 3)],
        states: vec![vec![0], vec![1], vec![2], vec![3]],
        goal: vec![vec![0]],
        initial: vec![vec![3]],
        actions: vec!["dec", "skip"],
        fair: Vec::new(),
        rows: vec![
            vec![],
            vec![(0, vec![0])],
            vec![(0, vec![1]), (1, vec![0])],
            vec![(0, vec![2]), (1, vec![1])],
        ],
        ranks: vec![0, 1, 2, 3],
    }
}

/// The Revision 2 spike's `FairProgress`: `Wait` loops, `Complete` is always enabled
/// and weakly fair, so the loop is not a fair execution.
fn temporal_fair_scc() -> TemporalSpec {
    TemporalSpec {
        kind: 2,
        variables: vec![("done", 0, 1)],
        states: vec![vec![0], vec![1]],
        goal: vec![vec![1]],
        initial: vec![vec![0]],
        actions: vec!["Complete", "StayDone", "Wait"],
        fair: vec![0],
        rows: vec![vec![(0, vec![1]), (2, vec![0])], vec![(1, vec![1])]],
        ranks: Vec::new(),
    }
}

/// A fair cycle that no execution can enter: `[1,0]` loops on `stay`, where the fair
/// action `go` is disabled, but nothing reaches it. Reachability is re-derived, so the
/// trap must not refute the claim.
fn temporal_fair_scc_unreachable_trap() -> TemporalSpec {
    TemporalSpec {
        kind: 2,
        variables: vec![("a", 0, 1), ("b", 0, 1)],
        states: vec![vec![0, 0], vec![0, 1], vec![1, 0], vec![1, 1]],
        goal: vec![vec![0, 1]],
        initial: vec![vec![0, 0]],
        actions: vec!["go", "stay"],
        fair: vec![0],
        rows: vec![
            vec![(0, vec![0, 1])],
            vec![(1, vec![0, 1])],
            vec![(1, vec![1, 0])],
            vec![(1, vec![1, 1])],
        ],
        ranks: Vec::new(),
    }
}

/// Two dead ends the fair-SCC check must not refuse: the goal `[0,1]` stops (a goal
/// stutter visits the goal forever), and `[1,0]` stops but nothing reaches it. This is
/// the scope of the bn-2npu dead-end rule, held by a green.
fn temporal_fair_scc_harmless_dead_ends() -> TemporalSpec {
    TemporalSpec {
        kind: 2,
        variables: vec![("a", 0, 1), ("b", 0, 1)],
        states: vec![vec![0, 0], vec![0, 1], vec![1, 0]],
        goal: vec![vec![0, 1]],
        initial: vec![vec![0, 0]],
        actions: vec!["go", "stay"],
        fair: vec![0],
        rows: vec![vec![(0, vec![0, 1])], vec![], vec![]],
        ranks: Vec::new(),
    }
}

/// Behaviour after the goal does not bear on `eventually goal` (cr-10mg1y): `n` reaches
/// the goal `1`, then either stops at `2` or loops on `3`.
fn temporal_fair_scc_after_the_goal() -> TemporalSpec {
    TemporalSpec {
        kind: 2,
        variables: vec![("n", 0, 3)],
        states: vec![vec![0], vec![1], vec![2], vec![3]],
        goal: vec![vec![1]],
        initial: vec![vec![0]],
        actions: vec!["loop", "stop"],
        fair: Vec::new(),
        rows: vec![
            vec![(1, vec![1])],
            vec![(0, vec![3]), (1, vec![2])],
            vec![],
            vec![(0, vec![3])],
        ],
        ranks: Vec::new(),
    }
}

fn greens() -> Vec<(&'static str, Vec<u8>)> {
    vec![
        ("core/finite-closure", core_closure().encode()),
        ("core/state-type", core_state_type().encode()),
        ("core/model-closure", core_model_closure().encode()),
        ("core/model-invariant", core_model_invariant().encode()),
        ("sat/lrat", sat_lrat().encode()),
        ("smt/smt-proof", smt_proof().encode()),
        ("temporal/ranking", temporal_ranking().encode()),
        ("temporal/fair-scc-exclusion", temporal_fair_scc().encode()),
        (
            "temporal/fair-scc-unreachable-trap",
            temporal_fair_scc_unreachable_trap().encode(),
        ),
        (
            "temporal/fair-scc-harmless-dead-ends",
            temporal_fair_scc_harmless_dead_ends().encode(),
        ),
        (
            "temporal/fair-scc-after-the-goal",
            temporal_fair_scc_after_the_goal().encode(),
        ),
    ]
}

// --- the lies ----------------------------------------------------------------------------

/// One false claim, and the rejection reason the lie targets.
///
/// `expect` is the reason string of the kernel's `Rejection`, or `verified:TRUSTED_SOLVER`
/// for the one lie the kernel is designed not to catch.
///
/// Three kinds of lie. A *false claim* carries a statement the oracle refutes: the
/// certificate says something untrue about its carried structure. *Broken evidence*
/// carries a true statement with a proof that does not establish it, such as a hint
/// chain that never conflicts. A checker must reject both, because the rule that admits
/// a broken proof of a true formula admits the same proof of a false one. A
/// *trusted-component* lie is false only inside a component the kernel names as
/// trusted (an SMT theory lemma), so neither the kernel nor the oracle checks it.
struct Lie {
    id: &'static str,
    kind: &'static str,
    targets: &'static str,
    expect: &'static str,
    bytes: Vec<u8>,
}

fn lie(id: &'static str, targets: &'static str, expect: &'static str, bytes: Vec<u8>) -> Lie {
    Lie {
        id,
        kind: "false-claim",
        targets,
        expect,
        bytes,
    }
}

fn broken(id: &'static str, targets: &'static str, expect: &'static str, bytes: Vec<u8>) -> Lie {
    Lie {
        kind: "broken-evidence",
        ..lie(id, targets, expect, bytes)
    }
}

fn trusted(id: &'static str, targets: &'static str, expect: &'static str, bytes: Vec<u8>) -> Lie {
    Lie {
        kind: "trusted-component",
        ..lie(id, targets, expect, bytes)
    }
}

#[allow(
    clippy::too_many_lines,
    reason = "one corpus, one place; each entry is three lines and splitting it would \
              only scatter the list"
)]
fn lies() -> Vec<Lie> {
    let mut out = Vec::new();

    // Finite closure: Init ⊆ S, Post(S) ⊆ S, S ⊆ P.
    let mut c = core_closure();
    c.states.pop();
    c.rows.pop();
    out.push(lie(
        "core-fc/drop-reachable-state",
        "Post(S) ⊆ S: a state the explorer lost",
        "closure-failure",
        c.encode(),
    ));
    let mut c = core_closure();
    c.rows[1] = vec![(0, vec![3])];
    out.push(lie(
        "core-fc/retarget-outside-table",
        "Post(S) ⊆ S: a successor outside the carried set",
        "closure-failure",
        c.encode(),
    ));
    let mut c = core_closure();
    c.variables = vec![("x", 0, 1)];
    out.push(lie(
        "core-fc/state-outside-property",
        "S ⊆ P: a carried state the declared domain refuses",
        "property-violated",
        c.encode(),
    ));
    let mut c = core_closure();
    c.initial = vec![vec![3]];
    out.push(lie(
        "core-fc/initial-outside-table",
        "Init ⊆ S",
        "initial-state-not-in-table",
        c.encode(),
    ));
    let mut c = core_closure();
    c.rows[2] = vec![(2, vec![0])];
    out.push(lie(
        "core-fc/undeclared-action",
        "every transition names a declared action",
        "unknown-action",
        c.encode(),
    ));

    // Finite closure at wire epoch 2: the carried relation is the carried model's
    // (bn-35y4f). Each lie is a producer fault bn-2npu found the epoch-1 kernel missing,
    // or one the model-bound rules add.
    let mut c = core_model_closure();
    c.rows[1].clear();
    out.push(lie(
        "core-mc/drop-transition",
        "the carried relation is the model's: a closed sub-relation",
        "relation-mismatch",
        c.encode(),
    ));
    let mut c = core_model_closure();
    c.rows[2] = vec![(1, 1)];
    out.push(lie(
        "core-mc/retarget-inside-table",
        "the carried relation is the model's: a target inside the table",
        "relation-mismatch",
        c.encode(),
    ));
    let mut c = core_model_closure();
    c.model.actions[1].2 = vec![vec![("x", expr::int(1))]];
    out.push(lie(
        "core-mc/rows-from-another-evaluator",
        "the carried relation is the model's: reset writes 1, the rows say 0",
        "relation-mismatch",
        c.encode(),
    ));
    let mut c = core_model_closure();
    c.model.initial = vec![vec![0, 1], vec![3, 1]];
    out.push(lie(
        "core-mc/model-initial-outside-table",
        "Init(M) ⊆ S",
        "initial-state-not-in-table",
        c.encode(),
    ));
    let mut c = core_model_closure();
    c.states.pop();
    c.rows = vec![vec![(0, 1)], vec![(0, 0)]];
    out.push(lie(
        "core-mc/successor-outside-table",
        "Post(S) ⊆ S under the model's relation",
        "successor-not-in-table",
        c.encode(),
    ));
    let mut c = core_model_closure();
    c.states.push(vec![4, 1]);
    c.rows.push(Vec::new());
    out.push(lie(
        "core-mc/state-outside-domain",
        "S ⊆ Dom(M)",
        "property-violated",
        c.encode(),
    ));
    let mut c = core_model_closure();
    c.model.actions[0].2 = vec![vec![("x", expr::add(&expr::var("x"), &expr::int(9)))]];
    out.push(lie(
        "core-mc/update-leaves-domain",
        "the model's relation is defined: an update leaves the domain",
        "update-outside-domain",
        c.encode(),
    ));
    let mut c = core_model_closure();
    c.model.actions[0].1 = expr::compare(
        4,
        &expr::add(
            &expr::add(&expr::var("x"), &expr::int(i64::MAX)),
            &expr::int(9),
        ),
        &expr::int(0),
    );
    out.push(lie(
        "core-mc/guard-overflows",
        "the model's relation is defined: checked arithmetic",
        "evaluation-overflow",
        c.encode(),
    ));
    let mut c = core_model_closure();
    c.model.actions[0].1 = [
        &[0x13, 0x10, 0][..],
        &expr::compare(
            4,
            &expr::add(&expr::int(i64::MAX), &expr::int(1)),
            &expr::int(0),
        ),
    ]
    .concat();
    out.push(lie(
        "core-mc/dead-operand-overflows",
        "the model's relation is defined: connectives do not short-circuit",
        "evaluation-overflow",
        c.encode(),
    ));
    let mut c = core_model_invariant();
    c.invariant = Some("NotTwo");
    out.push(lie(
        "core-mc/invariant-false",
        "S ⊆ P for an invariant of the model",
        "invariant-violated",
        c.encode(),
    ));

    // State type: S ⊆ P.
    let mut c = core_state_type();
    c.variables[1] = ("y", 0, 1);
    out.push(lie(
        "core-st/state-outside-domain",
        "S ⊆ P",
        "property-violated",
        c.encode(),
    ));

    // LRAT.
    let mut s = sat_lrat();
    s.clauses.pop();
    s.steps[2] = ProofStep::Add(6, vec![], vec![5, 3]);
    s.steps[0] = ProofStep::Add(5, vec![2], vec![1, 2]);
    out.push(lie(
        "sat/satisfiable-formula",
        "refutation: the formula without clause 4 is satisfiable",
        "no-conflict",
        s.encode(),
    ));
    let mut s = sat_lrat();
    s.steps[2] = ProofStep::Add(6, vec![], vec![3, 5, 4]);
    out.push(broken(
        "sat/antecedent-not-unit",
        "RUP: an antecedent with two unassigned literals",
        "hint-not-unit",
        s.encode(),
    ));
    let mut s = sat_lrat();
    s.steps[1] = ProofStep::Delete(vec![5]);
    out.push(broken(
        "sat/use-after-delete",
        "the clause store: a deleted antecedent",
        "hint-deleted",
        s.encode(),
    ));
    let mut s = sat_lrat();
    s.steps.pop();
    out.push(broken(
        "sat/no-empty-clause",
        "refutation: the proof stops before the empty clause",
        "empty-clause-not-derived",
        s.encode(),
    ));
    let mut s = sat_lrat();
    s.steps[0] = ProofStep::Add(5, vec![-1, 1], vec![1]);
    out.push(broken(
        "sat/tautology-derived",
        "a vacuous derivation",
        "vacuous-derivation",
        s.encode(),
    ));
    let mut s = sat_lrat();
    s.steps[2] = ProofStep::Add(6, vec![], vec![5, 3, 7]);
    out.push(broken(
        "sat/antecedent-unknown-id",
        "an antecedent that names no clause",
        "hint-out-of-range",
        s.encode(),
    ));
    let mut s = sat_lrat();
    s.steps[0] = ProofStep::Add(5, vec![2], vec![1]);
    out.push(broken(
        "sat/chain-without-conflict",
        "RUP: a chain that never reaches a conflict",
        "no-conflict",
        s.encode(),
    ));
    let mut s = sat_lrat();
    s.steps[2] = ProofStep::Add(6, vec![], vec![5, 3, 4, 4]);
    out.push(broken(
        "sat/conflict-before-end",
        "a chain's conflict is its last antecedent",
        "conflict-before-end",
        s.encode(),
    ));
    let mut s = sat_lrat();
    s.steps.push(ProofStep::Delete(vec![2]));
    out.push(broken(
        "sat/proof-continues",
        "nothing may follow the empty clause",
        "proof-continues-after-empty-clause",
        s.encode(),
    ));

    // SMT proof.
    let mut m = smt_proof();
    m.assertions.pop();
    m.steps[0] = ProofStep::Add(4, vec![], vec![1, 2, 3]);
    out.push(lie(
        "smt/satisfiable-skeleton",
        "refutation: without `c<a` the skeleton and the lemma are satisfiable",
        "no-conflict",
        m.encode(),
    ));
    let mut m = smt_proof();
    m.steps[0] = ProofStep::Add(5, vec![], vec![4, 1, 2, 3]);
    out.push(broken(
        "smt/lemma-before-its-units",
        "RUP: the lemma is not unit before the assertions propagate",
        "hint-not-unit",
        m.encode(),
    ));
    let mut m = smt_proof();
    m.lemmas = vec![("LIA", vec![-1, -2])];
    m.steps[0] = ProofStep::Add(5, vec![], vec![1, 2, 4]);
    out.push(trusted(
        "smt/false-theory-lemma",
        "a theory lemma: `a<b ∧ b<c` is consistent, so `¬a<b ∨ ¬b<c` is not LIA-valid",
        "verified:TRUSTED_SOLVER",
        m.encode(),
    ));

    // Ranking.
    let mut t = temporal_ranking();
    t.ranks[2] = 1;
    out.push(broken(
        "tmp-rank/rank-does-not-decrease",
        "every step out of a non-goal state decreases the rank",
        "rank-does-not-decrease",
        t.encode(),
    ));
    let mut t = temporal_ranking();
    t.ranks[0] = 1;
    out.push(broken(
        "tmp-rank/goal-rank-not-zero",
        "goal states rank zero",
        "goal-rank-not-zero",
        t.encode(),
    ));
    let mut t = temporal_ranking();
    t.rows[1].clear();
    out.push(lie(
        "tmp-rank/dead-end",
        "a non-goal state with no successor never progresses",
        "progress-deadlock",
        t.encode(),
    ));
    let mut t = temporal_ranking();
    t.rows[1] = vec![(0, vec![0]), (1, vec![2])];
    out.push(lie(
        "tmp-rank/cycle",
        "a cycle among non-goal states: `1 → 2 → 1`",
        "rank-does-not-decrease",
        t.encode(),
    ));
    let mut t = temporal_ranking();
    t.variables = vec![("n", 0, 2)];
    out.push(lie(
        "tmp-rank/state-outside-domain",
        "safety side condition: every state in the domain",
        "state-outside-domain",
        t.encode(),
    ));

    // Fair-SCC exclusion.
    let mut t = temporal_fair_scc();
    t.fair.clear();
    out.push(lie(
        "tmp-scc/unfair-wait-loop",
        "without fairness the `Wait` loop is an execution that never completes",
        "fair-cycle-exists",
        t.encode(),
    ));
    let mut t = temporal_fair_scc();
    t.rows[0] = vec![(2, vec![0])];
    out.push(lie(
        "tmp-scc/complete-disabled",
        "`Complete` disabled on the loop discharges weak fairness vacuously",
        "fair-cycle-exists",
        t.encode(),
    ));
    let mut t = temporal_fair_scc();
    t.rows[0].clear();
    out.push(lie(
        "tmp-scc/reachable-dead-end",
        "an execution that stops at a non-goal state (bn-2npu fix)",
        "progress-deadlock",
        t.encode(),
    ));
    let mut t = temporal_fair_scc();
    t.fair = vec![3];
    out.push(lie(
        "tmp-scc/fair-action-undeclared",
        "a fairness assumption over an action outside the alphabet",
        "fair-action-out-of-range",
        t.encode(),
    ));
    let mut t = temporal_fair_scc();
    t.variables = vec![("done", 0, 2)];
    t.rows[0] = vec![(0, vec![2]), (2, vec![0])];
    out.push(lie(
        "tmp-scc/successor-outside-table",
        "every transition lands in the table",
        "closure-failure",
        t.encode(),
    ));
    out
}

// --- verdict rendering -------------------------------------------------------------------

/// The composed answer, rendered as one stable token.
fn render(outcome: &Outcome) -> String {
    match outcome {
        Outcome::Unroutable(fault) => format!("unroutable:{fault:?}"),
        Outcome::Checked(verdict) => match verdict {
            KernelVerdict::Core(v) => match v {
                kcore::Verdict::Verified(_) => "verified".to_owned(),
                kcore::Verdict::Rejected(r) => format!("rejected:{}", r.reason()),
                kcore::Verdict::Unsupported(f) => format!("unsupported:{f:?}"),
            },
            KernelVerdict::Sat(v) => match v {
                ksat::Verdict::Verified(_) => "verified".to_owned(),
                ksat::Verdict::Rejected(r) => format!("rejected:{}", r.reason()),
                ksat::Verdict::Unsupported(f) => format!("unsupported:{f:?}"),
            },
            KernelVerdict::Smt(v) => match v {
                ksmt::Verdict::Verified(claim) => {
                    format!("verified:{}", claim.assurance_class().as_str())
                }
                ksmt::Verdict::Rejected(r) => format!("rejected:{}", r.reason()),
                ksmt::Verdict::Unsupported(f) => format!("unsupported:{f:?}"),
            },
            KernelVerdict::Temporal(v) => match v {
                ktmp::Verdict::Verified(_) => "verified".to_owned(),
                ktmp::Verdict::Rejected(r) => format!("rejected:{}", r.reason()),
                ktmp::Verdict::Unsupported(f) => format!("unsupported:{f:?}"),
            },
        },
    }
}

fn is_verified(outcome: &Outcome) -> bool {
    match outcome {
        Outcome::Unroutable(_) => false,
        Outcome::Checked(KernelVerdict::Core(v)) => v.is_verified(),
        Outcome::Checked(KernelVerdict::Sat(v)) => v.is_verified(),
        Outcome::Checked(KernelVerdict::Smt(v)) => v.is_verified(),
        Outcome::Checked(KernelVerdict::Temporal(v)) => v.is_verified(),
    }
}

// --- the oracle --------------------------------------------------------------------------

/// Whether the claim a verified certificate carries is true of its carried structure.
///
/// `Err` when the bytes do not decode although the checker verified them: that is an
/// inconsistency the caller must fail on.
fn oracle(bytes: &[u8]) -> Result<bool, String> {
    let magic = bytes.get(..8).ok_or("no magic")?;
    match magic {
        b"CONTCERT" => oracle_core(bytes),
        b"CONTSATC" => oracle_sat(bytes),
        b"CONTSMTC" => oracle_smt(bytes),
        b"CONTTMPC" => oracle_temporal(bytes),
        _ => Err("unknown magic".to_owned()),
    }
}

fn in_domain(bounds: &[(i64, i64)], state: &[i64]) -> bool {
    state.len() == bounds.len()
        && state
            .iter()
            .zip(bounds)
            .all(|(value, (lo, hi))| lo <= value && value <= hi)
}

/// The oracle's reading of a carried model: a tree per expression, names resolved by
/// linear scan. A third reading of the grammar, after the model core's and the
/// kernel's, sharing neither's code.
enum Tree {
    Int(i64),
    Var(usize),
    Arith(u8, Box<Tree>, Box<Tree>),
    Pick(bool, Box<Tree>, Box<Tree>),
    Bool(bool),
    Compare(u8, Box<Tree>, Box<Tree>),
    Not(Box<Tree>),
    Join(u8, Box<Tree>, Box<Tree>),
    Within(Box<Tree>, i64, i64),
}

/// One action as the oracle reads it: its guard and its outcomes.
type OracleAction = (Tree, Vec<Vec<(usize, Tree)>>);

struct OracleModel {
    bounds: Vec<(i64, i64)>,
    actions: Vec<OracleAction>,
    initial: Vec<Vec<i64>>,
    predicates: Vec<(String, Tree)>,
}

struct Cursor<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl Cursor<'_> {
    fn take(&mut self, n: usize) -> Result<&[u8], String> {
        let out = self
            .bytes
            .get(self.at..self.at + n)
            .ok_or("model section truncated")?;
        self.at += n;
        Ok(out)
    }

    fn u8(&mut self) -> Result<u8, String> {
        Ok(self.take(1)?[0])
    }

    fn u64(&mut self) -> Result<u64, String> {
        Ok(u64::from_be_bytes(
            self.take(8)?.try_into().expect("8 bytes"),
        ))
    }

    fn i64(&mut self) -> Result<i64, String> {
        Ok(i64::from_be_bytes(
            self.take(8)?.try_into().expect("8 bytes"),
        ))
    }

    fn name(&mut self) -> Result<String, String> {
        let len = usize::try_from(self.u64()?).map_err(|e| e.to_string())?;
        Ok(String::from_utf8_lossy(self.take(len)?).into_owned())
    }

    fn int(&mut self, names: &[String]) -> Result<Tree, String> {
        Ok(match self.u8()? {
            0x01 => Tree::Int(self.i64()?),
            0x02 => {
                let name = self.name()?;
                Tree::Var(names.iter().position(|n| *n == name).ok_or("unbound")?)
            }
            0x03 => {
                let op = self.u8()?;
                Tree::Arith(op, Box::new(self.int(names)?), Box::new(self.int(names)?))
            }
            op @ (0x04 | 0x05) => Tree::Pick(
                op == 0x05,
                Box::new(self.int(names)?),
                Box::new(self.int(names)?),
            ),
            other => return Err(format!("int opcode {other:#04x}")),
        })
    }

    fn boolean(&mut self, names: &[String]) -> Result<Tree, String> {
        Ok(match self.u8()? {
            0x10 => Tree::Bool(self.u8()? == 1),
            0x11 => {
                let op = self.u8()?;
                Tree::Compare(op, Box::new(self.int(names)?), Box::new(self.int(names)?))
            }
            0x12 => Tree::Not(Box::new(self.boolean(names)?)),
            op @ 0x13..=0x15 => Tree::Join(
                op,
                Box::new(self.boolean(names)?),
                Box::new(self.boolean(names)?),
            ),
            0x16 => {
                let inner = self.int(names)?;
                Tree::Within(Box::new(inner), self.i64()?, self.i64()?)
            }
            other => return Err(format!("bool opcode {other:#04x}")),
        })
    }
}

fn read_model(bytes: &[u8]) -> Result<OracleModel, String> {
    let mut c = Cursor { bytes, at: 17 };
    let mut names = Vec::new();
    let mut bounds = Vec::new();
    for _ in 0..c.u64()? {
        names.push(c.name()?);
        bounds.push((c.i64()?, c.i64()?));
    }
    let mut actions = Vec::new();
    for _ in 0..c.u64()? {
        c.name()?;
        let guard = c.boolean(&names)?;
        let mut outcomes = Vec::new();
        for _ in 0..c.u64()? {
            let mut outcome = Vec::new();
            for _ in 0..c.u64()? {
                let name = c.name()?;
                let index = names.iter().position(|n| *n == name).ok_or("unbound")?;
                outcome.push((index, c.int(&names)?));
            }
            outcomes.push(outcome);
        }
        actions.push((guard, outcomes));
    }
    let mut initial = Vec::new();
    for _ in 0..c.u64()? {
        let mut state = Vec::new();
        for _ in 0..c.u64()? {
            state.push(c.i64()?);
        }
        initial.push(state);
    }
    let mut predicates = Vec::new();
    for _ in 0..c.u64()? {
        let name = c.name()?;
        predicates.push((name, c.boolean(&names)?));
    }
    Ok(OracleModel {
        bounds,
        actions,
        initial,
        predicates,
    })
}

/// `None` is an arithmetic overflow. Connectives evaluate both sides.
fn eval_int(tree: &Tree, state: &[i64]) -> Option<i64> {
    match tree {
        Tree::Int(v) => Some(*v),
        Tree::Var(i) => state.get(*i).copied(),
        Tree::Arith(op, a, b) => {
            let (a, b) = (eval_int(a, state)?, eval_int(b, state)?);
            match op {
                0 => a.checked_add(b),
                1 => a.checked_sub(b),
                _ => a.checked_mul(b),
            }
        }
        Tree::Pick(max, a, b) => {
            let (a, b) = (eval_int(a, state)?, eval_int(b, state)?);
            Some(if *max { a.max(b) } else { a.min(b) })
        }
        _ => None,
    }
}

fn eval_bool(tree: &Tree, state: &[i64]) -> Option<bool> {
    match tree {
        Tree::Bool(v) => Some(*v),
        Tree::Compare(op, a, b) => {
            let (a, b) = (eval_int(a, state)?, eval_int(b, state)?);
            Some(match op {
                0 => a == b,
                1 => a != b,
                2 => a < b,
                3 => a <= b,
                4 => a > b,
                _ => a >= b,
            })
        }
        Tree::Not(a) => Some(!eval_bool(a, state)?),
        Tree::Join(op, a, b) => {
            let (a, b) = (eval_bool(a, state), eval_bool(b, state));
            let (a, b) = (a?, b?);
            Some(match op {
                0x13 => a && b,
                0x14 => a || b,
                _ => !a || b,
            })
        }
        Tree::Within(a, lo, hi) => {
            let v = eval_int(a, state)?;
            Some(*lo <= v && v <= *hi)
        }
        _ => None,
    }
}

/// The wire-epoch-2 claim: S ⊆ Dom(M), Init(M) ⊆ S, every row the model's row, and the
/// invariant at every state.
fn oracle_model_closure(body: &kcore::wire::ModelClosureBody) -> Result<bool, String> {
    let model = read_model(body.model_identity())?;
    let states: Vec<Vec<i64>> = (0..body.table().len())
        .map(|i| body.table().state(i).expect("index below len").to_vec())
        .collect();
    let index_of = |s: &[i64]| states.iter().position(|t| t.as_slice() == s);
    if !states.iter().all(|s| in_domain(&model.bounds, s)) {
        return Ok(false);
    }
    if !model.initial.iter().all(|s| index_of(s).is_some()) {
        return Ok(false);
    }
    for (i, state) in states.iter().enumerate() {
        let mut derived: Vec<(u16, u32)> = Vec::new();
        for (a, (guard, outcomes)) in model.actions.iter().enumerate() {
            let Some(enabled) = eval_bool(guard, state) else {
                return Ok(false);
            };
            if !enabled {
                continue;
            }
            for outcome in outcomes {
                let mut next = state.clone();
                for (v, value) in outcome {
                    let Some(value) = eval_int(value, state) else {
                        return Ok(false);
                    };
                    let (lo, hi) = model.bounds[*v];
                    if value < lo || value > hi {
                        return Ok(false);
                    }
                    next[*v] = value;
                }
                let Some(target) = index_of(&next) else {
                    return Ok(false);
                };
                derived.push((
                    u16::try_from(a).expect("few actions"),
                    u32::try_from(target).expect("few states"),
                ));
            }
        }
        derived.sort_unstable();
        derived.dedup();
        let carried = body
            .row(u32::try_from(i).expect("few states"))
            .unwrap_or(&[]);
        if carried != derived.as_slice() {
            return Ok(false);
        }
    }
    if let Some(name) = body.invariant() {
        let Some((_, predicate)) = model.predicates.iter().find(|(n, _)| n == name.as_str()) else {
            return Ok(false);
        };
        for state in &states {
            if eval_bool(predicate, state) != Some(true) {
                return Ok(false);
            }
        }
    }
    Ok(true)
}

fn oracle_core(bytes: &[u8]) -> Result<bool, String> {
    let certificate = kcore::wire::decode(bytes).map_err(|e| format!("{e:?}"))?;
    let table = |t: &kcore::wire::StateTable| -> Vec<Vec<i64>> {
        (0..t.len())
            .map(|i| t.state(i).expect("index below len").to_vec())
            .collect()
    };
    match certificate.body() {
        kcore::wire::Body::ModelClosure(body) => oracle_model_closure(body),
        kcore::wire::Body::StateType(body) => {
            let bounds: Vec<(i64, i64)> = body
                .domain()
                .variables()
                .iter()
                .map(|v| (v.lo(), v.hi()))
                .collect();
            Ok(table(body.table()).iter().all(|s| in_domain(&bounds, s)))
        }
        kcore::wire::Body::FiniteClosure(body) => {
            let bounds: Vec<(i64, i64)> = body
                .domain()
                .variables()
                .iter()
                .map(|v| (v.lo(), v.hi()))
                .collect();
            let states = table(body.table());
            let member = |s: &[i64]| states.iter().any(|t| t.as_slice() == s);
            let actions = body.actions().len();
            let closed = body.rows().iter().all(|row| {
                row.iter()
                    .all(|t| usize::from(t.action()) < actions && member(t.target()))
            });
            Ok(!body.initial_states().is_empty()
                && body.initial_states().iter().all(|s| member(s))
                && closed
                && states.iter().all(|s| in_domain(&bounds, s)))
        }
    }
}

/// Whether a clause set has no satisfying assignment, by enumeration.
fn unsatisfiable(clauses: &[Vec<i32>]) -> Result<bool, String> {
    let mut variables: Vec<u32> = clauses
        .iter()
        .flatten()
        .map(|literal| literal.unsigned_abs())
        .collect();
    variables.sort_unstable();
    variables.dedup();
    if variables.len() > 20 {
        return Err(format!(
            "{} variables is too many to enumerate",
            variables.len()
        ));
    }
    for assignment in 0_u32..(1_u32 << variables.len()) {
        let value = |literal: i32| {
            let position = variables
                .iter()
                .position(|v| *v == literal.unsigned_abs())
                .expect("every literal's variable is collected");
            let bit = assignment >> position & 1 == 1;
            bit == (literal > 0)
        };
        if clauses.iter().all(|c| c.iter().any(|l| value(*l))) {
            return Ok(false);
        }
    }
    Ok(true)
}

fn oracle_sat(bytes: &[u8]) -> Result<bool, String> {
    let certificate = ksat::wire::decode(bytes).map_err(|e| format!("{e:?}"))?;
    let ksat::wire::Body::Lrat(body) = certificate.body();
    let clauses: Vec<Vec<i32>> = body
        .formula()
        .clauses()
        .iter()
        .map(|c| c.literals().to_vec())
        .collect();
    unsatisfiable(&clauses)
}

/// The SMT claim is a refutation of the skeleton *given* the theory lemmas, which the
/// kernel trusts and names. The oracle therefore includes every lemma.
fn oracle_smt(bytes: &[u8]) -> Result<bool, String> {
    let certificate = ksmt::wire::decode(bytes).map_err(|e| format!("{e:?}"))?;
    let ksmt::wire::Body::SmtProof(body) = certificate.body();
    let mut clauses: Vec<Vec<i32>> = body
        .assertions()
        .iter()
        .map(|c| c.literals().to_vec())
        .collect();
    clauses.extend(body.lemmas().iter().map(|l| l.clause().literals().to_vec()));
    unsatisfiable(&clauses)
}

/// The carried graph, over table indices, or `None` if a declared state is not a
/// table state (then no temporal claim about the table can hold).
struct Graph {
    goal: Vec<bool>,
    initial: Vec<usize>,
    edges: Vec<Vec<(u16, usize)>>,
}

fn temporal_graph(body: &ktmp::wire::TemporalBody) -> Option<Graph> {
    let states: Vec<Vec<i64>> = (0..body.table().len())
        .map(|i| body.table().state(i).expect("index below len").to_vec())
        .collect();
    let index = |s: &[i64]| states.iter().position(|t| t.as_slice() == s);
    let bounds: Vec<(i64, i64)> = body
        .domain()
        .variables()
        .iter()
        .map(|v| (v.lo(), v.hi()))
        .collect();
    if !states.iter().all(|s| in_domain(&bounds, s)) {
        return None;
    }
    let actions = body.actions().len();
    if body
        .fair_actions()
        .iter()
        .any(|a| usize::from(*a) >= actions)
    {
        return None;
    }
    let mut goal = vec![false; states.len()];
    for g in body.goal_states() {
        goal[index(g)?] = true;
    }
    let initial = body
        .initial_states()
        .iter()
        .map(|s| index(s))
        .collect::<Option<Vec<_>>>()?;
    let mut edges = Vec::new();
    for row in body.rows() {
        let mut out = Vec::new();
        for t in row {
            if usize::from(t.action()) >= actions {
                return None;
            }
            out.push((t.action(), index(t.target())?));
        }
        edges.push(out);
    }
    Some(Graph {
        goal,
        initial,
        edges,
    })
}

/// The longest path of non-goal steps from `state`, or `None` if a non-goal cycle or a
/// non-goal dead end is reachable from it.
fn longest(graph: &Graph, state: usize, on_path: &mut Vec<bool>) -> Option<u64> {
    if graph.goal[state] {
        return Some(0);
    }
    if graph.edges[state].is_empty() || on_path[state] {
        return None;
    }
    on_path[state] = true;
    let mut best = 0;
    for (_, target) in &graph.edges[state] {
        best = best.max(longest(graph, *target, on_path)? + 1);
    }
    on_path[state] = false;
    Some(best)
}

fn oracle_temporal(bytes: &[u8]) -> Result<bool, String> {
    let verdict = ktmp::check_certificate(bytes);
    let certificate = ktmp::wire::decode(bytes).map_err(|e| format!("{e:?}"))?;
    let body = certificate.body();
    let Some(graph) = temporal_graph(body) else {
        return Ok(false);
    };
    let n = graph.edges.len();
    match certificate.kind() {
        ktmp::verdict::CertificateKind::Ranking => {
            // From every table state, every execution reaches the goal within the
            // claimed bound.
            // A rejected ranking claims no bound; the oracle then asks only whether the
            // goal is reached at all.
            let bound = match verdict {
                ktmp::Verdict::Verified(claim) => claim.progress_bound().unwrap_or(0),
                _ => u64::MAX,
            };
            for state in 0..n {
                match longest(&graph, state, &mut vec![false; n]) {
                    Some(length) if length <= bound => {}
                    _ => return Ok(false),
                }
            }
            Ok(true)
        }
        ktmp::verdict::CertificateKind::FairSccExclusion => {
            // States reached from an initial state without passing through a goal
            // state, by a stack walk that does not expand goal states. The claim is
            // `eventually goal`, so nothing after a goal visit bears on it.
            let mut reach = vec![false; n];
            let mut stack = graph.initial.clone();
            while let Some(s) = stack.pop() {
                if reach[s] {
                    continue;
                }
                reach[s] = true;
                if !graph.goal[s] {
                    stack.extend(graph.edges[s].iter().map(|(_, t)| *t));
                }
            }
            let scope: Vec<usize> = (0..n).filter(|s| reach[*s] && !graph.goal[*s]).collect();
            if scope.iter().any(|s| graph.edges[*s].is_empty()) {
                return Ok(false); // a finite execution stops short of the goal
            }
            if scope.len() > 16 {
                return Err("too many states to enumerate subsets".to_owned());
            }
            let fair = body.fair_actions();
            for mask in 1_u32..(1_u32 << scope.len()) {
                let set: Vec<usize> = (0..scope.len())
                    .filter(|i| mask >> i & 1 == 1)
                    .map(|i| scope[i])
                    .collect();
                let inside = |s: usize| set.contains(&s);
                let internal = |s: usize| {
                    graph.edges[s]
                        .iter()
                        .filter(|(_, t)| inside(*t))
                        .map(|(_, t)| *t)
                        .collect::<Vec<_>>()
                };
                // Strongly connected with at least one internal edge: every member
                // reaches every member through a non-empty internal path.
                let strongly = set.iter().all(|from| {
                    let mut seen: Vec<usize> = Vec::new();
                    let mut work = internal(*from);
                    while let Some(s) = work.pop() {
                        if !seen.contains(&s) {
                            seen.push(s);
                            work.extend(internal(s));
                        }
                    }
                    set.iter().all(|to| seen.contains(to))
                });
                if !strongly {
                    continue;
                }
                let fair_set = fair.iter().all(|action| {
                    let taken = set.iter().any(|s| {
                        graph.edges[*s]
                            .iter()
                            .any(|(a, t)| a == action && inside(*t))
                    });
                    let disabled = set
                        .iter()
                        .any(|s| !graph.edges[*s].iter().any(|(a, _)| a == action));
                    taken || disabled
                });
                if fair_set {
                    return Ok(false); // a fair execution cycles here forever
                }
            }
            Ok(true)
        }
    }
}

// --- the campaign --------------------------------------------------------------------------

struct Sweep {
    class: &'static str,
    bytes: usize,
    outcomes: BTreeMap<String, usize>,
    verified_same: usize,
    verified_relabelled: usize,
    verified_false: Vec<String>,
}

fn claim_text(outcome: &Outcome) -> String {
    format!("{outcome:?}")
}

fn sweep(class: &'static str, green: &[u8]) -> Sweep {
    let green_claim = claim_text(&check_certificate(green));
    let mut result = Sweep {
        class,
        bytes: green.len(),
        outcomes: BTreeMap::new(),
        verified_same: 0,
        verified_relabelled: 0,
        verified_false: Vec::new(),
    };
    for position in 0..green.len() {
        for mask in MASKS {
            let mut mutant = green.to_vec();
            mutant[position] ^= mask;
            let outcome = check_certificate(&mutant);
            if is_verified(&outcome) {
                match oracle(&mutant) {
                    Ok(true) => {}
                    Ok(false) => result
                        .verified_false
                        .push(format!("{class} byte {position} ^ {mask:#04x}")),
                    Err(why) => result.verified_false.push(format!(
                        "{class} byte {position} ^ {mask:#04x}: oracle error {why}"
                    )),
                }
                if claim_text(&outcome) == green_claim {
                    result.verified_same += 1;
                } else {
                    result.verified_relabelled += 1;
                }
                *result.outcomes.entry("verified".to_owned()).or_default() += 1;
            } else {
                let rendered = render(&outcome);
                // The reason or feature name only: payloads (offsets, codes, magics)
                // would make the histogram one bucket per mutant.
                let key = rendered
                    .split([' ', '(', '{'])
                    .next()
                    .unwrap_or_default()
                    .to_owned();
                *result.outcomes.entry(key).or_default() += 1;
            }
        }
    }
    result
}

fn ledger() -> String {
    let mut out = String::new();
    out.push_str(
        "# C018 checker mutation campaign: corrupted certificates of every checked class\n\
         # crates/continuum-certificate/tests/c018_checker_mutation.rs (bn-2npu)\n\
         # checker: continuum_certificate::check_certificate over the four kernels\n\
         # oracle: brute-force re-derivation over the kernel-decoded mutant (see module docs)\n\
         # masks: 0x01 0x80 0xff at every byte of every green certificate\n",
    );
    out.push_str("# GREEN class | verdict | oracle\n");
    for (class, bytes) in greens() {
        let outcome = check_certificate(&bytes);
        let _ = writeln!(
            out,
            "GREEN {class} | {} | {}",
            render(&outcome),
            match oracle(&bytes) {
                Ok(true) => "claim-holds".to_owned(),
                Ok(false) => "claim-false".to_owned(),
                Err(why) => format!("oracle-error:{why}"),
            }
        );
    }
    out.push_str("# LIE id | kind | targets | expected | verdict | oracle | grade\n");
    let mut caught = 0;
    let mut missed = 0;
    for l in lies() {
        let outcome = check_certificate(&l.bytes);
        let rendered = render(&outcome);
        let verdict = rendered
            .strip_prefix("rejected:")
            .unwrap_or(rendered.as_str())
            .to_owned();
        let truth = match oracle(&l.bytes) {
            Ok(true) => "claim-holds",
            Ok(false) => "claim-false",
            Err(_) => "claim-undecodable",
        };
        let grade = if is_verified(&outcome) {
            missed += 1;
            "NOT-CAUGHT"
        } else {
            caught += 1;
            "caught"
        };
        let _ = writeln!(
            out,
            "LIE {} | {} | {} | {} | {} | {} | {}",
            l.id, l.kind, l.targets, l.expect, verdict, truth, grade
        );
    }
    let _ = writeln!(out, "# LIES caught {caught} not-caught {missed}");
    out.push_str(
        "# SWEEP class | bytes | mutants | verified (same claim / relabelled) | \
         verified-false | outcome histogram\n",
    );
    let mut total = 0;
    let mut total_verified = 0;
    for (class, bytes) in greens() {
        let s = sweep(class, &bytes);
        let mutants = s.bytes * MASKS.len();
        total += mutants;
        total_verified += s.verified_same + s.verified_relabelled;
        let histogram = s
            .outcomes
            .iter()
            .map(|(k, v)| format!("{k}={v}"))
            .collect::<Vec<_>>()
            .join(" ");
        let _ = writeln!(
            out,
            "SWEEP {} | {} | {} | {} ({} / {}) | {} | {}",
            s.class,
            s.bytes,
            mutants,
            s.verified_same + s.verified_relabelled,
            s.verified_same,
            s.verified_relabelled,
            s.verified_false.len(),
            histogram
        );
    }
    let _ = writeln!(
        out,
        "# SWEEP total mutants {total}, verified {total_verified}, not verified {}",
        total - total_verified
    );
    out
}

// --- the tests ----------------------------------------------------------------------------

#[test]
fn every_green_certificate_verifies_and_the_oracle_agrees() {
    for (class, bytes) in greens() {
        let outcome = check_certificate(&bytes);
        assert!(is_verified(&outcome), "{class}: {}", render(&outcome));
        assert_eq!(
            oracle(&bytes),
            Ok(true),
            "{class}: the oracle refutes a green claim"
        );
    }
    // The SMT green uses its lemma, so its assurance is named, not assumed.
    assert_eq!(
        render(&check_certificate(&smt_proof().encode())),
        "verified:TRUSTED_SOLVER"
    );
}

#[test]
fn every_lie_is_a_false_claim_so_the_corpus_is_not_vacuous() {
    for l in lies() {
        let truth = oracle(&l.bytes);
        if l.kind == "trusted-component" {
            // The oracle trusts the lemma the same way the kernel does, so it sees a
            // true claim. The lemma itself is false: `a<b ∧ b<c` is satisfiable in
            // integer arithmetic.
            assert_eq!(truth, Ok(true), "{}", l.id);
            continue;
        }
        if l.kind == "false-claim" {
            assert!(
                matches!(truth, Ok(false)),
                "{}: the oracle does not refute this lie ({truth:?}), so it is not one",
                l.id
            );
        } else {
            assert_eq!(
                truth,
                Ok(true),
                "{}: broken evidence is meant to carry a true claim",
                l.id
            );
        }
    }
    let false_claims = lies().iter().filter(|l| l.kind == "false-claim").count();
    assert!(false_claims >= 24, "the corpus keeps its false-claim core");
}

#[test]
fn no_lie_verifies() {
    for l in lies() {
        if l.kind == "trusted-component" {
            continue;
        }
        let outcome = check_certificate(&l.bytes);
        assert!(
            !is_verified(&outcome),
            "{} verified a false claim ({})",
            l.id,
            l.targets
        );
    }
}

#[test]
fn every_lie_is_rejected_for_the_reason_it_targets() {
    for l in lies() {
        let rendered = render(&check_certificate(&l.bytes));
        let got = rendered.strip_prefix("rejected:").unwrap_or(&rendered);
        assert_eq!(got, l.expect, "{} ({})", l.id, l.targets);
    }
}

fn trusted_lemma_lie() -> Lie {
    lies()
        .into_iter()
        .find(|l| l.id == "smt/false-theory-lemma")
        .expect("the corpus carries the trusted-lemma row")
}

/// The kernel does not check theory lemmas, so a false one is accepted. A rejection
/// here is a completeness failure, not a soundness one.
#[test]
fn the_trusted_lemma_is_accepted_because_lemmas_are_not_checked() {
    let outcome = check_certificate(&trusted_lemma_lie().bytes);
    assert!(is_verified(&outcome), "{}", render(&outcome));
}

/// When the kernel accepts a refutation that used a lemma, the claim must say so: the
/// assurance is `TRUSTED_SOLVER` and the theory is named. Reporting
/// `CHECKED_CERTIFICATE` would overstate what was checked. A rejected lemma makes this
/// test pass vacuously; the test above catches that case.
#[test]
fn an_accepted_lemma_names_its_trusted_solver_assurance() {
    let Outcome::Checked(KernelVerdict::Smt(ksmt::Verdict::Verified(claim))) =
        check_certificate(&trusted_lemma_lie().bytes)
    else {
        return;
    };
    assert_eq!(
        claim.assurance_class(),
        ksmt::verdict::AssuranceClass::TrustedSolver
    );
    assert_eq!(
        claim
            .theories()
            .iter()
            .map(ksmt::wire::Token::as_str)
            .collect::<Vec<_>>(),
        ["LIA"]
    );
}

#[test]
fn no_single_byte_mutant_verifies_a_claim_the_oracle_refutes() {
    let mut false_verified = Vec::new();
    let mut verified = 0;
    for (class, bytes) in greens() {
        let s = sweep(class, &bytes);
        verified += s.verified_same + s.verified_relabelled;
        false_verified.extend(s.verified_false);
    }
    assert!(false_verified.is_empty(), "{false_verified:#?}");
    // Anti-vacuity: the oracle ran. Label bytes (names, digests, widened bounds) keep a
    // true claim, so some mutants must verify and be judged.
    assert!(verified > 0, "no mutant verified, so the oracle never ran");
}

/// Writes `contents` to `dir/name`, creating `dir` first.
///
/// `CARGO_TARGET_TMPDIR` is a compile-time constant (`env!`): cargo creates the
/// directory it names when it (re)builds this test binary, but it does not
/// re-create the directory on every `cargo test` run of an already-built binary.
/// A run with a fresh or emptied target directory (a new checkout, a scratch
/// workspace, `TMPDIR` pointed at a not-yet-created path per the project
/// convention) then hits a plain `std::fs::write` before this test's own
/// assertion runs, so the write itself, not the check it supports, fails
/// (bn-1gibz). Creating the directory here removes that dependency on cargo's
/// build-time side effect.
fn write_scratch(dir: &Path, name: &str, contents: &str) -> std::path::PathBuf {
    std::fs::create_dir_all(dir)
        .unwrap_or_else(|error| panic!("{} is not creatable: {error}", dir.display()));
    let out = dir.join(name);
    std::fs::write(&out, contents)
        .unwrap_or_else(|error| panic!("{} is not writable: {error}", out.display()));
    out
}

#[test]
fn the_c018_checker_ledger_matches_the_golden() {
    let actual = ledger();
    let out = write_scratch(
        Path::new(env!("CARGO_TARGET_TMPDIR")),
        "c018_checker_ledger.txt",
        &actual,
    );
    assert!(
        actual == GOLDEN,
        "the C018 checker ledger drifted; the rendered ledger is at {}",
        out.display()
    );
    assert_eq!(actual, ledger(), "INV-005: the campaign is a pure function");
}

/// Regression for bn-1gibz: `write_scratch` must recreate its directory when it
/// is absent, not assume cargo already made it. Uses its own subdirectory so it
/// cannot race the other tests in this binary that write under
/// `CARGO_TARGET_TMPDIR` directly.
#[test]
fn write_scratch_recreates_a_missing_directory() {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("bn-1gibz-missing-dir-regression");
    let _ = std::fs::remove_dir_all(&dir);
    assert!(
        !dir.exists(),
        "setup: the regression directory must start absent"
    );
    let out = write_scratch(&dir, "probe.txt", "bn-1gibz");
    assert_eq!(
        std::fs::read_to_string(&out).expect("the probe file was written"),
        "bn-1gibz"
    );
}

// --- the committed corpus ---------------------------------------------------------------

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn unhex(text: &str) -> Vec<u8> {
    (0..text.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&text[i..i + 2], 16).expect("the corpus is hex"))
        .collect()
}

fn render_corpus() -> String {
    let mut out = String::from(
        "# C018 committed certificate corpus (bn-2npu): id | recorded verdict | hex bytes\n\
         # replayed by c018_checker_mutation.rs::the_committed_corpus_replays_with_its_recorded_verdicts\n",
    );
    for (class, bytes) in greens() {
        let _ = writeln!(
            out,
            "green:{class} | {} | {}",
            render(&check_certificate(&bytes)),
            hex(&bytes)
        );
    }
    for l in lies() {
        let _ = writeln!(
            out,
            "lie:{} | {} | {}",
            l.id,
            render(&check_certificate(&l.bytes)),
            hex(&l.bytes)
        );
    }
    out
}

/// Replays the committed corpus from its bytes, and ties it to the generator: a lie
/// or green that changes shape changes the committed file, so the corpus cannot drift
/// from what the campaign above tests. The corpus is the kernel fuzz corpus a kernel
/// change records (docs/12 §4, GOV-4-18); the `tmp-scc/reachable-dead-end` line is the
/// bn-2npu regression input.
#[test]
fn the_committed_corpus_replays_with_its_recorded_verdicts() {
    let rendered = render_corpus();
    let out = write_scratch(
        Path::new(env!("CARGO_TARGET_TMPDIR")),
        "c018_corpus_cases.txt",
        &rendered,
    );
    let mut replayed = 0;
    for line in CORPUS.lines().filter(|l| !l.starts_with('#')) {
        let mut fields = line.split(" | ");
        let (Some(id), Some(recorded), Some(bytes)) = (fields.next(), fields.next(), fields.next())
        else {
            panic!("malformed corpus line: {line}");
        };
        let verdict = render(&check_certificate(&unhex(bytes)));
        assert_eq!(verdict, recorded, "{id}");
        replayed += 1;
    }
    assert_eq!(replayed, greens().len() + lies().len());
    assert!(
        CORPUS.contains("lie:tmp-scc/reachable-dead-end | rejected:progress-deadlock | "),
        "the bn-2npu regression input is in the corpus"
    );
    // The bn-35y4f inputs: the producer faults bn-2npu found the epoch-1 kernel
    // missing, as epoch-2 certificates the kernel must reject.
    for line in [
        "lie:core-mc/drop-transition | rejected:relation-mismatch | ",
        "lie:core-mc/retarget-inside-table | rejected:relation-mismatch | ",
        "lie:core-mc/rows-from-another-evaluator | rejected:relation-mismatch | ",
        "green:core/model-closure | verified | ",
        "green:core/model-invariant | verified | ",
    ] {
        assert!(CORPUS.contains(line), "the corpus carries {line:?}");
    }
    assert!(
        rendered == CORPUS,
        "the committed corpus drifted from the generator; the rendered corpus is at {}",
        out.display()
    );
}
