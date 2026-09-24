//! The pinned init path (bn-2ri63, RFC 0003 correction 6) against its oracle, the
//! whole-domain enumeration.
//!
//! # What is pinned
//!
//! - **exactness**: over every model of the repository corpus (the fuzz corpus, the
//!   syntax fixtures, the TLA+ ports, and the dossier examples, with and without their
//!   run configurations) and over 600 generated init predicates, the two paths give
//!   the same lowered model — the same initial states in the same order, so the same
//!   identity — or the same error, including evaluation errors with their operands,
//!   `cml.lower.undefined_read`, and the builder's own errors. The only allowed
//!   difference is a resource refusal of the whole-domain enumeration
//!   (`init_domain_too_large`, `work_limit_exceeded`, `output_too_large`) that the
//!   pinned path does not meet. The generator reaches both paths, every outcome class,
//!   and each fallback reason (arithmetic, a map read);
//! - **the domain bound**: the pinned product, not the whole domain, meets
//!   `MAX_INIT_ENUMERATION`, and an unpinned variable keeps its whole domain on its axis,
//!   so no init shape enumerates past the bound;
//! - **charge before work**: a pinned lowering replays under exactly the limits it
//!   reported, and one unit less of work or output is a typed refusal.

use std::path::{Path, PathBuf};

use continuum_cml_elab::config::RunConfig;
use continuum_cml_elab::lower::{
    InitPath, MAX_INIT_ENUMERATION, lower_configured_with_init, lower_with_init,
    pinned_inits_on_this_thread, pinned_trace_on_this_thread,
};
use continuum_cml_elab::{Limits, LowerErrorKind, NormModel, Unlowerable, elaborate_source};
use continuum_model_core::{Model, ModelError};

type Outcome = Result<Model, LowerErrorKind>;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn unconfigured(model: &NormModel, path: InitPath, limits: Limits) -> Outcome {
    lower_with_init(model, limits, path).0.map_err(|e| e.kind)
}

fn configured(model: &NormModel, config: &RunConfig, path: InitPath, limits: Limits) -> Outcome {
    lower_configured_with_init(model, config, limits, path)
        .0
        .map(|c| c.model().clone())
        .map_err(|e| e.kind)
}

/// Whether a refusal is about resources: the paths charge different candidates, so
/// only these may differ.
fn resource(e: &LowerErrorKind) -> bool {
    matches!(
        e,
        LowerErrorKind::Unlowerable(
            Unlowerable::InitDomainTooLarge
                | Unlowerable::WorkLimitExceeded
                | Unlowerable::OutputTooLarge
        )
    )
}

/// How two outcomes relate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Agreement {
    /// The same model, or the same error.
    Same,
    /// The whole-domain enumeration met a resource bound, and the pinned path lowered.
    Relaxed,
}

/// The oracle's outcome against the pinned path's: equal, or relaxed from a resource
/// refusal to a pinned *success*. A pinned error that differs from the oracle's —
/// an evaluation error, `undefined_read`, no initial state, or another resource
/// refusal — fails the test (cr-1s123d).
fn agree(what: &str, oracle: &Outcome, pinned: &Outcome) -> Agreement {
    match (oracle, pinned) {
        (Ok(a), Ok(b)) => {
            assert_eq!(a, b, "{what}: the lowered models differ");
            assert_eq!(a.identity(), b.identity(), "{what}: identities differ");
            Agreement::Same
        }
        (Err(a), Err(b)) if a == b => Agreement::Same,
        (Err(a), Ok(_)) if resource(a) => Agreement::Relaxed,
        _ => panic!("{what}: oracle {oracle:?}, pinned {pinned:?}"),
    }
}

/// Both paths, and whether the pinned one enumerated a pinned space.
fn both(lower: impl Fn(InitPath) -> Outcome) -> (Outcome, Outcome, bool) {
    let oracle = lower(InitPath::Enumerate);
    let before = pinned_inits_on_this_thread();
    let pinned = lower(InitPath::Pinned);
    let used = pinned_inits_on_this_thread() > before;
    (oracle, pinned, used)
}

fn ctm_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let mut entries: Vec<PathBuf> = std::fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("{}: {e}", dir.display()))
        .map(|e| e.expect("entry").path())
        .collect();
    entries.sort();
    for p in entries {
        if p.is_dir() {
            ctm_files(&p, out);
        } else if p.extension().is_some_and(|x| x == "ctm") {
            out.push(p);
        }
    }
}

// ---------------------------------------------------------------------------
// exactness over the corpus
// ---------------------------------------------------------------------------

#[test]
fn every_corpus_model_lowers_the_same_on_both_paths() {
    let mut files = Vec::new();
    for dir in [
        "crates/continuum-cml-elab/tests/cml-fuzz-corpus/cases",
        "crates/continuum-cml-elab/tests/configs",
        "crates/continuum-cml-syntax/tests/fixtures",
        "notes/plan/corpus",
        "notes/plan/examples",
    ] {
        ctm_files(&root().join(dir), &mut files);
    }
    assert!(files.len() >= 70, "the corpus is present: {}", files.len());
    let (mut elaborated, mut same, mut relaxed, mut used) = (0, 0, 0, 0);
    for path in &files {
        let src = std::fs::read_to_string(path).expect("reads");
        let Ok(model) = elaborate_source(&src) else {
            continue;
        };
        elaborated += 1;
        let (oracle, pinned, pin) = both(|p| unconfigured(&model, p, Limits::default()));
        used += usize::from(pin);
        match agree(&path.display().to_string(), &oracle, &pinned) {
            Agreement::Same => same += 1,
            Agreement::Relaxed => relaxed += 1,
        }
    }
    assert!(elaborated >= 20, "{elaborated} models elaborate");
    assert!(used >= 5, "the pinned path ran on {used} corpus models");
    assert_eq!(same + relaxed, elaborated);
}

#[test]
fn every_configured_example_lowers_the_same_on_both_paths() {
    let pairs = [
        (
            "notes/plan/examples/abstract_register.ctm",
            "notes/plan/examples/abstract_register.run-config.json",
        ),
        (
            "notes/plan/examples/replicated_register.ctm",
            "notes/plan/schemas/examples/replicated-register.run-config.json",
        ),
        (
            "crates/continuum-cml-elab/tests/configs/Ring.ctm",
            "crates/continuum-cml-elab/tests/configs/ring.run-config.json",
        ),
        (
            "notes/plan/examples/durable_register.ctm",
            "notes/plan/examples/durable_register.run-config.json",
        ),
    ];
    let mut relaxed = Vec::new();
    for (src, cfg) in pairs {
        let text = std::fs::read_to_string(root().join(src)).expect("reads");
        let model = elaborate_source(&text).unwrap_or_else(|e| panic!("{src}: {e}"));
        let bytes = std::fs::read(root().join(cfg)).expect("reads");
        let config = RunConfig::parse(&bytes).unwrap_or_else(|e| panic!("{cfg}: {e}"));
        let (oracle, pinned, used) = both(|p| configured(&model, &config, p, Limits::default()));
        assert!(used, "{src}: its init pins every slot");
        if agree(src, &oracle, &pinned) == Agreement::Relaxed {
            relaxed.push(src);
        }
    }
    // The durable register needs a 2^34 work budget to enumerate its whole 2^22 init
    // domain (bn-2rsp, `bnd-01`); pinned, the default budget lowers it.
    assert_eq!(relaxed, ["notes/plan/examples/durable_register.ctm"]);
}

// ---------------------------------------------------------------------------
// exactness over generated init predicates
// ---------------------------------------------------------------------------

const HEAD: &str = r#""schema_id":"https://continuum.dev/schema/run-config.json","schema_epoch":1"#;

/// `Nat` bounded to `0..=1`, `Int` to `-3..=3`.
fn frame_config() -> RunConfig {
    let text = format!(
        r#"{{{HEAD},"model":"G","bounds":{{"Nat":{{"max":1}},"Int":{{"min":-3,"max":3}}}},"sorts":{{}},"constants":{{}}}}"#
    );
    RunConfig::parse(text.as_bytes()).expect("reads")
}

/// Scalars, an option, a set, and a map: 24,192 flat states, 15,120 of them canonical.
fn frame(init: &str) -> String {
    format!(
        "module G\nenum Phase {{ Idle, Busy }}\nstate {{\n  x: Nat\n  y: Int\n  b: Phase\n  o: Option[Nat]\n  p: Option[Set[Bool]]\n  s: Set[Nat]\n  m: Map[Nat, Nat]\n}}\ninit {{\n  {init}\n}}\naction Stay {{\n  unchanged x, y, b, o, p, s, m\n}}\n"
    )
}

/// The clause kinds the generator draws from: pins (with constants inside and outside
/// the domains, repeated, and in disjunctions; a negative constant lowers as `0 - n`,
/// so its pin takes the whole domain), non-pinning comparisons, composite
/// equalities, arithmetic that may overflow at some states only, map reads (which
/// need the whole domain for `undefined_read`), and constants.
const CLAUSES: &[&str] = &[
    "x == 0",
    "x == 1",
    "1 == x",
    "x == 5",
    "x == 0 || x == 1",
    "x == 1 || x == 1 || x == 1",
    "x != 0",
    "x <= 0",
    "x == y",
    "y == -3",
    "y == 2",
    "-1 == y",
    "y == 4",
    "y == 0 || y == 2 || y == -2",
    "y == 1 || x == 1",
    "y >= 0",
    "b == Idle",
    "b != Busy",
    "b == Idle || b == Busy",
    "o == None",
    "o == Some(1)",
    "o == None || o == Some(0)",
    "p == None",
    "p == Some({true})",
    "s == {}",
    "0 in s",
    "s == {0}",
    "m == {}",
    "m == {0 -> 0}",
    "m == {0 -> 0, 1 -> 1}",
    "m.get(0) == None",
    "m[0] == 0",
    "m[1] == x",
    "y * 4611686018427387904 == 0",
    "x + 1 == 1",
    "min(y, 0) == 0",
    "true",
    "false",
    "exists k in 0..1: k in s",
    "forall k in s: m.get(k) != None",
];

/// Every variable pinned to one value.
const PINNED: &str =
    "x == 0 && y == 1 && b == Busy && o == Some(1) && p == None && s == {1} && m == {}";

/// A splitmix64 stream: deterministic, seeded, no dependency.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }

    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
}

#[derive(Debug, Default)]
struct Tally {
    elaborated: usize,
    pinned: usize,
    accepted: usize,
    several: usize,
    none: usize,
    evaluation: usize,
    undefined: usize,
    refused: usize,
}

#[test]
fn generated_init_predicates_lower_the_same_on_both_paths() {
    let config = frame_config();
    let mut rng = Rng(0x2e16_3000);
    let mut t = Tally::default();
    for case in 0..600 {
        let n = 1 + rng.below(6);
        let mut clauses: Vec<&str> = (0..n).map(|_| CLAUSES[rng.below(CLAUSES.len())]).collect();
        // One case in four also pins every variable, so single initial states occur.
        if rng.below(4) == 0 {
            clauses.push(PINNED);
        }
        let src = frame(&clauses.join(" &&\n  "));
        let Ok(model) = elaborate_source(&src) else {
            continue;
        };
        t.elaborated += 1;
        let (oracle, pinned, used) = both(|p| configured(&model, &config, p, Limits::default()));
        let what = format!("case {case}: {}", clauses.join(" && "));
        assert_eq!(agree(&what, &oracle, &pinned), Agreement::Same, "{what}");
        t.pinned += usize::from(used);
        match &oracle {
            Ok(m) if m.initial_states().len() == 1 => t.accepted += 1,
            Ok(_) => t.several += 1,
            Err(LowerErrorKind::Model(ModelError::NoInitialStates)) => t.none += 1,
            Err(LowerErrorKind::Evaluation(_)) => t.evaluation += 1,
            Err(LowerErrorKind::Unlowerable(Unlowerable::UndefinedRead)) => t.undefined += 1,
            Err(_) => t.refused += 1,
        }
    }
    // Not vacuous: most cases elaborate, both paths run, and every outcome class occurs.
    assert!(t.elaborated >= 500, "{t:?}");
    assert!(t.pinned >= 100 && t.pinned + 100 <= t.elaborated, "{t:?}");
    assert!(t.accepted >= 5, "{t:?}");
    assert!(t.several >= 20, "{t:?}");
    assert!(t.none >= 20, "{t:?}");
    assert!(t.evaluation >= 5, "{t:?}");
    assert!(t.undefined >= 5, "{t:?}");
}

/// The fallback cases one by one: a pin beside an overflow keeps the overflow, whose
/// operands are those at the first failing candidate of the whole domain; a pin beside
/// a map read keeps `undefined_read`, also where the read is defined at every pinned
/// candidate; each runs the whole domain.
#[test]
fn a_pin_does_not_hide_an_error_of_the_whole_domain() {
    let config = frame_config();
    for (init, want) in [
        (
            "y == 0 && y * 4611686018427387904 == 0 && x == 0 && b == Busy && o == None && p == None && s == {} && m == {}",
            "overflow",
        ),
        (
            "m[0] == 0 && m == {} && x == 0 && y == 0 && b == Idle && o == None && p == None && s == {}",
            "undefined",
        ),
        // Defined at the one pinned candidate, undefined elsewhere: the whole domain's
        // refusal stands.
        (
            "m[0] == 0 && m == {0 -> 0} && x == 0 && y == 0 && b == Idle && o == None && p == None && s == {}",
            "undefined",
        ),
    ] {
        let model = elaborate_source(&frame(init)).expect("elaborates");
        let (oracle, pinned, used) = both(|p| configured(&model, &config, p, Limits::default()));
        assert!(!used, "{init}: the whole domain");
        assert_eq!(oracle, pinned);
        match (want, &oracle) {
            ("overflow", Err(LowerErrorKind::Evaluation(e))) => {
                assert!(e.to_string().contains("overflow"), "{e}");
            }
            ("undefined", Err(LowerErrorKind::Unlowerable(Unlowerable::UndefinedRead))) => {}
            _ => panic!("{init}: {oracle:?}"),
        }
    }
}

/// A map read with no arithmetic: `m[0]` of a set value selects its slots directly, so
/// the predicate is total and only its definedness condition sends the lowering to the
/// whole domain. The read is defined at the one pinned candidate and undefined at
/// other canonical states, where the whole domain refuses it.
#[test]
fn a_map_read_without_arithmetic_still_runs_the_whole_domain() {
    let config = frame_config();
    let src = |init: &str| {
        format!(
            "module G\nstate {{\n  m: Map[Nat, Set[Nat]]\n  x: Nat\n}}\ninit {{ {init} }}\naction Stay {{ unchanged m, x }}\n"
        )
    };
    for init in [
        "0 in m[0] && m == {0 -> {0}} && x == 0",
        "m[1] == {} && m == {1 -> {}} && x == 1",
    ] {
        let model = elaborate_source(&src(init)).expect("elaborates");
        let (oracle, pinned, used) = both(|p| configured(&model, &config, p, Limits::default()));
        assert!(!used, "{init}: the whole domain");
        assert_eq!(oracle, pinned, "{init}");
        assert_eq!(
            oracle.expect_err("undefined elsewhere"),
            LowerErrorKind::Unlowerable(Unlowerable::UndefinedRead),
            "{init}"
        );
    }
    // Without the read, the same pins take the pinned path.
    let model = elaborate_source(&src("m == {0 -> {0}} && x == 0")).expect("elaborates");
    let (oracle, pinned, used) = both(|p| configured(&model, &config, p, Limits::default()));
    assert!(used);
    assert_eq!(oracle, pinned);
    assert_eq!(pinned.expect("lowers").initial_states().len(), 1);
}

/// Metamorphic relation "equivalent guard normalization" (docs/19 §3): an init
/// predicate rewritten to an equivalent one — sides of `==` swapped, disjuncts
/// reordered or repeated, conjuncts reordered, a pin stated twice — lowers on the
/// pinned path to the same model as the original, and to the whole domain's model.
#[test]
fn equivalent_guard_normalization_keeps_the_pinned_model() {
    let config = frame_config();
    let base = "(x == 0 || x == 1) && y == 2 && b == Busy && o == None && p == None && s == {0} && m == {}";
    let rewrites = [
        "(1 == x || 0 == x) && 2 == y && b == Busy && o == None && p == None && s == {0} && m == {}",
        "m == {} && s == {0} && p == None && o == None && b == Busy && y == 2 && (x == 1 || x == 0 || x == 1)",
        "y == 2 && (x == 0 || x == 1) && y == 2 && b == Busy && o == None && p == None && s == {0} && m == {}",
    ];
    let lower = |init: &str| {
        let model = elaborate_source(&frame(init)).expect("elaborates");
        let (oracle, pinned, used) = both(|p| configured(&model, &config, p, Limits::default()));
        assert!(used, "{init}: pinned");
        assert_eq!(oracle, pinned, "{init}");
        pinned.expect("lowers")
    };
    let want = lower(base);
    assert_eq!(want.initial_states().len(), 2);
    for r in rewrites {
        assert_eq!(lower(r), want, "{r}");
    }
}

/// A hand-built model that repeats a state name: the evaluator reads the first slot of
/// that name, so a pin on it must not restrict the other. The pinned path does not
/// apply, and both paths give the same outcome.
#[test]
fn a_repeated_slot_name_takes_the_whole_domain() {
    let src = "module D\nstate {\n  x: Nat where x <= 3\n  y: Nat where y <= 3\n}\ninit { x == 1 }\naction Stay { unchanged x, y }\n";
    let mut model = elaborate_source(src).expect("elaborates");
    let first = model.state[0].clone();
    model.state.insert(1, first);
    let (oracle, pinned, used) = both(|p| unconfigured(&model, p, Limits::default()));
    assert!(!used, "a repeated name takes the whole domain");
    assert_eq!(oracle, pinned);
}

// ---------------------------------------------------------------------------
// the domain bound
// ---------------------------------------------------------------------------

/// `n` variables `v0..`, each `0..=hi`, with the given init.
fn wide(n: usize, hi: u64, init: &str) -> NormModel {
    let vars: Vec<String> = (0..n)
        .map(|i| format!("  v{i}: Nat where v{i} <= {hi}"))
        .collect();
    let names: Vec<String> = (0..n).map(|i| format!("v{i}")).collect();
    let src = format!(
        "module W\nstate {{\n{}\n}}\ninit {{ {init} }}\naction Stay {{ unchanged {} }}\n",
        vars.join("\n"),
        names.join(", ")
    );
    elaborate_source(&src).unwrap_or_else(|e| panic!("elaborates: {e}"))
}

fn conj(n: usize, clause: impl Fn(usize) -> String) -> String {
    (0..n).map(clause).collect::<Vec<_>>().join(" && ")
}

#[test]
fn the_pinned_product_meets_the_enumeration_bound() {
    // 2^23 flat states, one pinned candidate: the oracle refuses, the pinned path lowers.
    let model = wide(23, 1, &conj(23, |i| format!("v{i} == 0")));
    let (oracle, pinned, used) = both(|p| unconfigured(&model, p, Limits::default()));
    assert_eq!(
        oracle.expect_err("2^23"),
        LowerErrorKind::Unlowerable(Unlowerable::InitDomainTooLarge)
    );
    assert!(used);
    assert_eq!(pinned.expect("one candidate").initial_states().len(), 1);

    // Each variable pinned to both its values: the product is the whole domain, and
    // the bound refuses it before any candidate on both paths.
    let model = wide(23, 1, &conj(23, |i| format!("(v{i} == 0 || v{i} == 1)")));
    let (oracle, pinned, used) = both(|p| unconfigured(&model, p, Limits::default()));
    assert!(!used, "refused before the first candidate");
    for o in [oracle, pinned] {
        assert_eq!(
            o.expect_err("2^23 candidates"),
            LowerErrorKind::Unlowerable(Unlowerable::InitDomainTooLarge)
        );
    }

    // An unpinned variable keeps its whole domain: one pinned variable beside one of
    // `MAX_INIT_ENUMERATION + 1` values is still past the bound.
    let hi = u64::try_from(MAX_INIT_ENUMERATION).expect("fits");
    let model = wide(2, hi, "v0 == 0 && v1 >= 0");
    let (oracle, pinned, used) = both(|p| unconfigured(&model, p, Limits::default()));
    assert!(!used);
    for o in [oracle, pinned] {
        assert_eq!(
            o.expect_err("past the bound"),
            LowerErrorKind::Unlowerable(Unlowerable::InitDomainTooLarge)
        );
    }

    // Constants outside the domain, and a contradiction, leave no candidate: nothing is
    // enumerated, and the builder reports no initial state, as the oracle does.
    for init in ["v0 == 7 && v1 == 0", "v0 == 0 && v0 == 1 && v1 == 0"] {
        let model = wide(2, 3, init);
        let (oracle, pinned, used) = both(|p| unconfigured(&model, p, Limits::default()));
        assert!(used, "{init}");
        assert_eq!(oracle, pinned, "{init}");
        assert_eq!(
            pinned.expect_err("no initial state"),
            LowerErrorKind::Model(ModelError::NoInitialStates)
        );
    }
}

// ---------------------------------------------------------------------------
// charge before work
// ---------------------------------------------------------------------------

/// A pinned lowering replays under exactly the limits it reported: every charge is
/// made before the work or the allocation it pays for, and the analysis spends exactly
/// its plan. One unit less of either budget is its typed refusal.
#[test]
fn a_pinned_lowering_replays_under_the_limits_it_reported() {
    let config = frame_config();
    let inits = [
        "x == 0 && y == 0 && b == Idle && o == None && p == None && s == {} && m == {}".to_owned(),
        "(x == 0 || x == 1) && y == 2 && b == Busy && (o == None || o == Some(1)) && p == Some({true}) && s == {0} && m == {}"
            .to_owned(),
    ];
    for init in &inits {
        let model = elaborate_source(&frame(init)).expect("elaborates");
        let before = pinned_inits_on_this_thread();
        let (result, usage) =
            lower_configured_with_init(&model, &config, Limits::default(), InitPath::Pinned);
        assert!(pinned_inits_on_this_thread() > before, "{init}");
        let lowered = result.expect("lowers").model().clone();
        let exact = Limits {
            nodes: usage.nodes,
            work: usage.work,
        };
        let (again, replay) = lower_configured_with_init(&model, &config, exact, InitPath::Pinned);
        assert_eq!(again.expect("replays").model(), &lowered);
        assert_eq!(replay, usage, "{init}: the same spend");
        for (limits, want) in [
            (
                Limits {
                    work: usage.work - 1,
                    ..exact
                },
                Unlowerable::WorkLimitExceeded,
            ),
            (
                Limits {
                    nodes: usage.nodes - 1,
                    ..exact
                },
                Unlowerable::OutputTooLarge,
            ),
        ] {
            let got = lower_configured_with_init(&model, &config, limits, InitPath::Pinned)
                .0
                .expect_err("one unit short");
            assert_eq!(got.kind, LowerErrorKind::Unlowerable(want), "{init}");
        }
    }
}

/// The analysis costs a scan, not an enumeration: the durable register's pinned
/// lowering spends a small fraction of the whole domain's charge.
#[test]
fn the_pinned_path_spends_far_less_than_the_whole_domain() {
    let text = std::fs::read_to_string(root().join("notes/plan/examples/durable_register.ctm"))
        .expect("reads");
    let model = elaborate_source(&text).expect("elaborates");
    let bytes = std::fs::read(root().join("notes/plan/examples/durable_register.run-config.json"))
        .expect("reads");
    let config = RunConfig::parse(&bytes).expect("reads");
    let (result, usage) =
        lower_configured_with_init(&model, &config, Limits::default(), InitPath::Pinned);
    assert_eq!(result.expect("lowers").model().initial_states().len(), 1);
    assert!(usage.work < 1 << 24, "{usage:?}");
}

// ---------------------------------------------------------------------------
// allocations within their precharge (cr-1s123d)
// ---------------------------------------------------------------------------

/// `x in {members}` over `x: Nat where x <= hi`: membership lowers to a balanced
/// disjunction of `x == m`, one pinning leaf per member.
fn disjunction(members: usize, hi: usize) -> NormModel {
    let list: Vec<String> = (0..members).map(|m| m.to_string()).collect();
    let src = format!(
        "module D\nstate {{\n  x: Nat where x <= {hi}\n  y: Nat where y <= 1\n}}\ninit {{ x in {{{}}} && y == 0 }}\naction Stay {{ unchanged x, y }}\n",
        list.join(", ")
    );
    elaborate_source(&src).unwrap_or_else(|e| panic!("elaborates: {e}"))
}

/// Large disjunctions around powers of two, with members inside and outside the
/// domain (the domain filter drops the latter): every vector the analysis allocates
/// holds no more than its precharge; the lowering replays at its exact limits; and
/// one output node short of the axes' charge is a typed refusal before any axis is
/// allocated.
#[test]
fn pinned_allocations_stay_within_their_precharge_around_powers_of_two() {
    for members in [63, 64, 65, 127, 128, 129, 1023, 1024, 1025] {
        for hi in [members - 1, members / 2] {
            let model = disjunction(members, hi);
            let before = pinned_trace_on_this_thread().builds;
            let (result, usage) = lower_with_init(&model, Limits::default(), InitPath::Pinned);
            let lowered = result.unwrap_or_else(|e| panic!("{members}/{hi}: {e:?}"));
            let trace = pinned_trace_on_this_thread();
            assert_eq!(trace.builds, before + 1, "{members}/{hi}: pinned");
            assert_eq!(
                trace.charged_values,
                2 * (members + 1),
                "{members}: two per leaf, y too"
            );
            assert!(
                trace.held_values <= trace.charged_values,
                "{members}/{hi}: {trace:?}"
            );
            assert!(
                trace.held_entries <= trace.charged_entries,
                "{members}/{hi}: {trace:?}"
            );
            assert_eq!(lowered.initial_states().len(), hi + 1, "{members}/{hi}");

            // Exact-limit replay.
            let exact = Limits {
                nodes: usage.nodes,
                work: usage.work,
            };
            let (again, replay) = lower_with_init(&model, exact, InitPath::Pinned);
            assert_eq!(again.expect("replays"), lowered);
            assert_eq!(replay, usage);

            // One node short of the axes' charge: refused, and no axis is built.
            let short = Limits {
                nodes: trace.nodes_after_charge - 1,
                work: usage.work,
            };
            let builds = pinned_trace_on_this_thread().builds;
            let refused = lower_with_init(&model, short, InitPath::Pinned)
                .0
                .expect_err("one node short");
            assert_eq!(
                refused.kind,
                LowerErrorKind::Unlowerable(Unlowerable::OutputTooLarge),
                "{members}/{hi}"
            );
            assert_eq!(
                pinned_trace_on_this_thread().builds,
                builds,
                "{members}/{hi}: refused before the axes were allocated"
            );
            // At exactly the axes' charge the analysis builds its axes.
            let at = Limits {
                nodes: trace.nodes_after_charge,
                work: usage.work,
            };
            let _ = lower_with_init(&model, at, InitPath::Pinned);
            assert_eq!(
                pinned_trace_on_this_thread().builds,
                builds + 1,
                "{members}/{hi}: the axes fit their charge"
            );
        }
    }
}
