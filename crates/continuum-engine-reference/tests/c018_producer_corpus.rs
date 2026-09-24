//! C018 producer corpus: the reference engine's certificates, checked from bytes by
//! the kernel, as the probe for producer mutants (bn-2npu).
//!
//! # What this file is for
//!
//! docs/18 C018 claims that proof-carrying results shrink the trusted computing base.
//! The test of that claim is whether a *producer* bug is caught by the checker. This
//! file is the probe. For each model of a small corpus it runs the whole producer —
//! `bfs::explore`, `ClosedSet::of`, `certificate::emit_finite_closure`, and the model
//! core's successor evaluator underneath both — and hands the bytes to
//! `continuum_kernel_core::check_certificate`. It prints one line per model:
//!
//! ```text
//! C018-PRODUCER <model> <outcome> fnv=<FNV-1a 64 of the certificate bytes>
//! ```
//!
//! # Its two roles
//!
//! Under `just test` it is a regression test: every certificate verifies, with the
//! pinned claim counts and the pinned byte digest.
//!
//! Under `tools/check_tcb_audit.py --evidence` it is the probe for the producer source
//! mutants in `tools/tcb-audit/mutants.toml`. The tool applies one mutant to a scratch
//! copy of `continuum-engine-reference` or `continuum-model-core`, runs this test with
//! `--nocapture`, and compares the printed lines with the unmutated run:
//!
//! - every line equal: the mutant did not change any certificate (no effect);
//! - some model `rejected:*`: the kernel caught the producer bug;
//! - some model `explore-error` or `emit-error`, none rejected: the producer refused
//!   its own output, and the kernel was never asked;
//! - otherwise some certificate changed and still verified: the kernel missed it.
//!
//! The lines are printed before any assertion, so a failing assertion under a mutant
//! does not hide them.
//!
//! # What the kernel can and cannot see
//!
//! A `CONTCERT` finite-closure certificate carries the reachable set, the initial
//! states, the declared domain and one successor row per state. The model is carried
//! only as a digest in the envelope. So the kernel can refuse a certificate whose set
//! is not closed under its own rows, leaves its own domain, or misses its own initial
//! states. It cannot refuse a certificate whose rows, initial states or domain are not
//! the model's. `CheckedClaim::trusted_components` names exactly that residual:
//! `certificate-model-correspondence` and `envelope-digest-binding`.

use continuum_engine_reference::bfs::{self, Bounds, Exploration};
use continuum_engine_reference::certificate::{self, ClaimEnvelope, ClosedSet, PRODUCER};
use continuum_engine_reference::diehard;
use continuum_engine_reference::expr::{BoolExpr, CmpOp, IntExpr};
use continuum_engine_reference::model::{ActionDecl, Model, ModelBuilder};

use continuum_kernel_core::check_certificate;
use continuum_kernel_core::verdict::Verdict;

fn envelope() -> ClaimEnvelope<'static> {
    ClaimEnvelope {
        model_digest: "blake3:c018-producer-corpus",
        semantic_epoch: "continuum-semantics-1",
        property_digest: "blake3:c018-state-domain",
        scope_digest: "blake3:c018-scope",
        assumptions_digest: "blake3:empty-assumptions",
        producer: PRODUCER,
        domain_pack_digests: &[],
    }
}

/// `x` climbs to `4` by `inc` and returns by `reset`; `flip` toggles `y` once `x ≥ 1`.
/// Two initial states, so a producer that drops one has something to drop.
///
/// Counted by hand: nine reachable states (`(0,1)` is unreachable, because `reset`
/// clears `y`), and 17 transitions — one `inc` at `x = 0`, `flip` and `inc` at each of
/// the six states with `1 ≤ x ≤ 3`, and `flip` and `reset` at both states with `x = 4`.
fn gated_counter() -> Model {
    let x = || IntExpr::var("x");
    let y = || IntExpr::var("y");
    ModelBuilder::new()
        .variable("x", 0, 4)
        .variable("y", 0, 1)
        .initial_state(&[("x", 0), ("y", 0)])
        .initial_state(&[("x", 2), ("y", 1)])
        .action(ActionDecl::deterministic(
            "flip",
            BoolExpr::compare(CmpOp::Ge, x(), IntExpr::constant(1)),
            vec![("y", IntExpr::minus(IntExpr::constant(1), y()))],
        ))
        .action(ActionDecl::deterministic(
            "inc",
            BoolExpr::compare(CmpOp::Lt, x(), IntExpr::constant(4)),
            vec![("x", IntExpr::plus(x(), IntExpr::constant(1)))],
        ))
        .action(ActionDecl::deterministic(
            "reset",
            BoolExpr::compare(CmpOp::Eq, x(), IntExpr::constant(4)),
            vec![("x", IntExpr::constant(0)), ("y", IntExpr::constant(0))],
        ))
        .build()
        .expect("the gated counter is a valid model")
}

/// `c` steps to `3` and stops: a declared deadlock, so one row is empty.
fn halting_counter() -> Model {
    let c = || IntExpr::var("c");
    ModelBuilder::new()
        .variable("c", 0, 3)
        .initial_state(&[("c", 0)])
        .action(ActionDecl::deterministic(
            "step",
            BoolExpr::compare(CmpOp::Lt, c(), IntExpr::constant(3)),
            vec![("c", IntExpr::plus(c(), IntExpr::constant(1)))],
        ))
        .build()
        .expect("the halting counter is a valid model")
}

fn corpus() -> Vec<(&'static str, Model)> {
    vec![
        (
            "die-hard",
            diehard::model().expect("the Die Hard transcription is a valid model"),
        ),
        ("gated-counter", gated_counter()),
        ("halting-counter", halting_counter()),
    ]
}

/// FNV-1a 64: a dependency-free digest of the certificate bytes.
fn fnv(bytes: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

/// Run the whole producer, then the kernel, and render one stable line.
fn probe(name: &str, model: &Model) -> String {
    let exploration = match bfs::explore(model, Bounds::CERTIFIABLE) {
        Ok(exploration) => exploration,
        Err(error) => return format!("C018-PRODUCER {name} explore-error {error:?}"),
    };
    let Exploration::Complete(_) = &exploration else {
        return format!("C018-PRODUCER {name} explore-exhausted");
    };
    let Some(closed) = ClosedSet::of(&exploration) else {
        return format!("C018-PRODUCER {name} explore-not-closed");
    };
    let bytes = match certificate::emit_finite_closure(model, closed, &envelope()) {
        Ok(bytes) => bytes,
        Err(error) => return format!("C018-PRODUCER {name} emit-error {error}"),
    };
    let outcome = match check_certificate(&bytes) {
        Verdict::Verified(claim) => format!(
            "verified states={} initial={} transitions={}",
            claim.states(),
            claim.initial_states(),
            claim.transitions()
        ),
        Verdict::Rejected(rejection) => format!("rejected:{}", rejection.reason()),
        Verdict::Unsupported(feature) => format!("unsupported:{feature:?}"),
    };
    format!("C018-PRODUCER {name} {outcome} fnv={:016x}", fnv(&bytes))
}

/// The unmutated producer's lines. A deliberate change to the engine or the model core
/// that moves a certificate moves one of these, and C018's producer ledger
/// (`tools/tcb-audit/evidence/c018.json`) must then be regenerated.
const PINNED: [&str; 3] = [
    "C018-PRODUCER die-hard verified states=16 initial=1 transitions=96 fnv=b6c3dc8707bb9236",
    "C018-PRODUCER gated-counter verified states=9 initial=2 transitions=17 fnv=bb8470999157f763",
    "C018-PRODUCER halting-counter verified states=4 initial=1 transitions=3 fnv=c97c55fd0e439065",
];

#[test]
fn the_producer_corpus_verifies_from_bytes_with_pinned_certificates() {
    let lines: Vec<String> = corpus()
        .iter()
        .map(|(name, model)| probe(name, model))
        .collect();
    for line in &lines {
        println!("{line}");
    }
    assert_eq!(lines, PINNED, "a producer certificate moved");
}

#[test]
fn the_probe_is_deterministic() {
    for (name, model) in corpus() {
        assert_eq!(probe(name, &model), probe(name, &model), "INV-005");
    }
}
