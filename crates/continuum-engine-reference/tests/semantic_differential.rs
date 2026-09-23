//! Differential cross-check: `tools/test-policy/tsys.py` (TEST §2's Python oracle,
//! bn-1fqu) versus [`continuum_engine_reference::semantic`] (the Rust oracle, bn-1zgs),
//! on one shared seeded corpus (bn-1kgnz, under bn-3ly0 "Semantic differential, replay,
//! and untrusted-input validation harness").
//!
//! # Why this lives in `tests/`, not `src/`
//!
//! Everything here — the JSON reader, the loader from the shared corpus shape into a
//! [`continuum_engine_reference::semantic::System`], the comparator — is built purely
//! from this crate's already-public surface
//! ([`ModelBuilder`][continuum_engine_reference::ModelBuilder],
//! [`ActionDecl`][continuum_engine_reference::ActionDecl],
//! [`SystemParts`][continuum_engine_reference::semantic::SystemParts], …). None of it
//! needs a line of `src/` to change. That matters beyond tidiness: GOV §4
//! (`tools/governance/README-gov4.md`) requires a review record — including an
//! independent, approved security review (GOV-4-06) — for any change to a
//! `SEMANTIC_CORE` crate's `src/`, and `continuum-engine-reference` is one
//! (`tools/governance/check_code_policy.py`'s `SEMANTIC_CORE` set). A `src/`-touching
//! design would need that review before this test could land; a `tests/`-only design
//! does not, per GOV-1-08's own predicate ("a test-only edit … is not a semantic
//! change"). This file is the "small pub fn … behind a test-only path" option bn-1kgnz
//! names explicitly, chosen over adding a `pub fn` to `src/` for exactly that reason.
//!
//! # The common sub-model
//!
//! The full definition — what a shared corpus system may contain, what is excluded and
//! why, and why the corpus is clean (`findings: []`) on both oracles by construction —
//! lives in `tools/test-policy/differential_corpus.py`'s module doc, which this file's
//! loader mirrors field for field. Restated at the Rust boundary specifically:
//!
//! - Variables: `bool` or bounded `int` only. No `enum` — [`Domain`][continuum_engine_reference::Domain]
//!   is an integer range; there is no symbolic domain to load one into.
//! - Expressions: `const`, `var`, `not`/`and`/`or` (binary), `eq`/`ne`/`lt`/`le`/`gt`/`ge`,
//!   `add`/`sub`. No `ite`, `mul`, `min`, `max`, `implies`, or range-form comparisons —
//!   [`BoolExpr`][continuum_engine_reference::BoolExpr]/[`IntExpr`][continuum_engine_reference::IntExpr]
//!   have some of those and `tsys.py` has others; the loader below accepts only their
//!   intersection and panics (a corpus defect, not a typed refusal — this is trusted,
//!   self-generated test input, not an untrusted artifact) on anything else.
//! - Footprints: the JSON's declared `reads`/`writes`, loaded verbatim as
//!   [`Footprint::new`][continuum_engine_reference::semantic::Footprint::new] — exact
//!   syntax by construction on the Python side, so the oracle's own footprint-escape
//!   checks never fire.
//! - Independence: not loaded as a separate relation at all. `System` has no
//!   `independent` field — [`System::claims_independent`][continuum_engine_reference::semantic::System]
//!   derives it from footprints and conflicts, exactly as `tsys.py`'s corpus declares
//!   it (both sides use "cross-process, footprint-disjoint"), so the JSON's
//!   `"independent"` list is Python-only input and is not read here.
//! - Conflicts: loaded into [`SystemParts::conflicts`][continuum_engine_reference::semantic::SystemParts].
//! - Obligations: the shared JSON has no obligation *keys* at all (`tsys.py`'s
//!   `"obligations"` list is always `[]` in this corpus — see the Python module doc for
//!   why). An obligation is instead an ordinary `int(0, 1)` data variable, named
//!   `p<N>_ob` by convention. [`is_obligation_variable`] recognizes the convention and
//!   the loader builds a native [`VarKind::Obligation`][continuum_engine_reference::semantic::VarKind]
//!   variable, plus an [`ObligationDecl`][continuum_engine_reference::semantic::ObligationDecl]
//!   with `owner_phase: None` for it (single owner, no phase — phases are excluded below).
//! - Cancellation phases and fairness: excluded entirely.
//!   [`SystemParts::phases`][continuum_engine_reference::semantic::SystemParts] is
//!   always empty and every [`ActionMeta::fairness`][continuum_engine_reference::semantic::ActionMeta]
//!   is [`Fairness::Unfair`][continuum_engine_reference::semantic::Fairness] — `tsys.py`
//!   has neither concept, so there is nothing on the Python side to compare against.
//!
//! # What is compared, and how
//!
//! For every corpus seed: reachable state count, the semantic dependence relation
//! ([`Artifact::dependent_pairs`][continuum_engine_reference::semantic::Artifact::dependent_pairs]
//! against the Python side's own from-scratch diamond walk — not derived from either
//! side's declared independence, so agreement is evidence, not a tautology), the
//! complete-interleaving count and Mazurkiewicz trace-class count
//! ([`Artifact::interleavings`][continuum_engine_reference::semantic::Artifact::interleavings]),
//! and finding kinds (always `[]` on both sides for this corpus — see the Python module
//! doc's "clean by construction" argument). [`Limits`] below is sized generously enough
//! (`max_depth = 16`) that no corpus interleaving is ever truncated; `run_facts` asserts
//! that rather than assuming it, so a future corpus change that silently exceeds the
//! bound fails loudly instead of comparing a partial count.
//!
//! # Wiring: a committed golden, not a `python3` subprocess
//!
//! No Rust integration test in this workspace shells out to `python3` (checked: no
//! `crates/**/tests/*.rs` does). Introducing that pattern here — for a single
//! differential check, with no other consumer — would make `cargo test` depend on a
//! Python interpreter and `tsys.py`'s import path being available in every environment
//! that runs the Rust suite, including release/CI sandboxes that may not carry Python.
//! The alternative bn-1kgnz's task offers instead is a committed golden: the Python side
//! emits its results for the corpus once (`tools/test-policy/differential_corpus.py`),
//! commits them, and this test reads the committed file, no subprocess and no Python
//! runtime dependency of any kind. The generator, the golden's regeneration, and its
//! staleness check are one file
//! (`tools/test-policy/differential_corpus.py --write` / no flag), the same
//! `--write`-versus-verify shape `tools/test-policy/check_test_policy.py --evidence`
//! already uses, and it is wired into `just test-policy` (see the Justfile) so the
//! golden is regenerated and verified the same way the rest of the harness is.
//!
//! # Known absence
//!
//! Every corpus system is clean on both oracles (see above), so the "finding kinds"
//! axis is exercised only by [`perturbed_golden_is_rejected`], which fabricates a
//! disagreement rather than mutating a real system. Extending the corpus with genuine
//! seeded defects — mirroring [`continuum_engine_reference::semantic::defect`] on a
//! Python-side counterpart — would give that axis non-vacuous coverage; it is a
//! follow-up, not this bone's scope.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::fs;
use std::path::PathBuf;

use continuum_engine_reference::semantic::{
    ActionMeta, Fairness, Footprint, Limits, ObligationDecl, Role, System, SystemParts, VarKind,
    run as run_oracle,
};
use continuum_engine_reference::{ActionDecl, BoolExpr, CmpOp, IntExpr, ModelBuilder};

// ---------------------------------------------------------------------------
// A minimal JSON reader.
//
// No crates.io dependency: the workspace has no JSON library today (checked: no crate
// depends on `serde`/`serde_json`), and adding one needs a recorded dependency audit
// (`tools/governance/dependency-audits.toml`) the lead owns, per this bone's ground
// rules. The golden file is self-generated (`differential_corpus.py`) and read-only
// here, so a small hand-rolled reader that panics on anything unexpected is adequate:
// this is trusted test fixture data, not an untrusted artifact.
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
enum Json {
    Bool(bool),
    Num(i64),
    Str(String),
    Arr(Vec<Json>),
    Obj(BTreeMap<String, Json>),
}

impl Json {
    fn get(&self, key: &str) -> &Json {
        self.try_get(key)
            .unwrap_or_else(|| panic!("golden JSON: missing key {key:?} in {self:?}"))
    }

    fn try_get(&self, key: &str) -> Option<&Json> {
        match self {
            Self::Obj(map) => map.get(key),
            _ => None,
        }
    }

    fn as_str(&self) -> &str {
        match self {
            Self::Str(s) => s,
            other => panic!("golden JSON: expected a string, found {other:?}"),
        }
    }

    fn as_i64(&self) -> i64 {
        match self {
            Self::Num(n) => *n,
            other => panic!("golden JSON: expected a number, found {other:?}"),
        }
    }

    /// The value of a `const`/init binding as an `i64`: a JSON bool maps to `1`/`0`
    /// (the common sub-model's `bool` variables are domain-`(0, 1)` int variables on
    /// this side), a JSON number maps to itself.
    fn as_value(&self) -> i64 {
        match self {
            Self::Bool(b) => i64::from(*b),
            Self::Num(n) => *n,
            other => panic!("golden JSON: expected a value (bool or number), found {other:?}"),
        }
    }

    fn as_arr(&self) -> &[Json] {
        match self {
            Self::Arr(items) => items,
            other => panic!("golden JSON: expected an array, found {other:?}"),
        }
    }

    fn as_obj(&self) -> &BTreeMap<String, Json> {
        match self {
            Self::Obj(map) => map,
            other => panic!("golden JSON: expected an object, found {other:?}"),
        }
    }
}

struct Parser<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl<'a> Parser<'a> {
    fn new(input: &'a str) -> Self {
        Self {
            bytes: input.as_bytes(),
            pos: 0,
        }
    }

    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.pos).copied()
    }

    fn bump(&mut self) -> Option<u8> {
        let byte = self.peek();
        if byte.is_some() {
            self.pos += 1;
        }
        byte
    }

    fn skip_ws(&mut self) {
        while matches!(self.peek(), Some(b' ' | b'\t' | b'\n' | b'\r')) {
            self.pos += 1;
        }
    }

    fn expect(&mut self, byte: u8) {
        let found = self.bump();
        assert!(
            found == Some(byte),
            "golden JSON: expected {:?} at byte {}, found {:?}",
            byte as char,
            self.pos,
            found.map(|b| b as char)
        );
    }

    fn parse_value(&mut self) -> Json {
        self.skip_ws();
        match self.peek() {
            Some(b'{') => self.parse_object(),
            Some(b'[') => self.parse_array(),
            Some(b'"') => Json::Str(self.parse_string()),
            Some(b't') => {
                self.expect_literal("true");
                Json::Bool(true)
            }
            Some(b'f') => {
                self.expect_literal("false");
                Json::Bool(false)
            }
            Some(b'n') => {
                self.expect_literal("null");
                Json::Bool(false) // never produced by this corpus; kept total rather than panicking
            }
            Some(b'-' | b'0'..=b'9') => self.parse_number(),
            other => panic!("golden JSON: unexpected byte {other:?} at {}", self.pos),
        }
    }

    fn expect_literal(&mut self, literal: &str) {
        for expected in literal.bytes() {
            self.expect(expected);
        }
    }

    fn parse_object(&mut self) -> Json {
        self.expect(b'{');
        let mut map = BTreeMap::new();
        self.skip_ws();
        if self.peek() == Some(b'}') {
            self.pos += 1;
            return Json::Obj(map);
        }
        loop {
            self.skip_ws();
            let key = self.parse_string();
            self.skip_ws();
            self.expect(b':');
            let value = self.parse_value();
            map.insert(key, value);
            self.skip_ws();
            match self.bump() {
                Some(b',') => continue,
                Some(b'}') => break,
                other => panic!("golden JSON: expected ',' or '}}' in object, found {other:?}"),
            }
        }
        Json::Obj(map)
    }

    fn parse_array(&mut self) -> Json {
        self.expect(b'[');
        let mut items = Vec::new();
        self.skip_ws();
        if self.peek() == Some(b']') {
            self.pos += 1;
            return Json::Arr(items);
        }
        loop {
            items.push(self.parse_value());
            self.skip_ws();
            match self.bump() {
                Some(b',') => continue,
                Some(b']') => break,
                other => panic!("golden JSON: expected ',' or ']' in array, found {other:?}"),
            }
        }
        Json::Arr(items)
    }

    fn parse_string(&mut self) -> String {
        self.expect(b'"');
        let mut out = String::new();
        loop {
            match self.bump() {
                Some(b'"') => break,
                Some(b'\\') => match self.bump() {
                    Some(b'"') => out.push('"'),
                    Some(b'\\') => out.push('\\'),
                    Some(b'/') => out.push('/'),
                    Some(b'n') => out.push('\n'),
                    Some(b't') => out.push('\t'),
                    other => panic!("golden JSON: unsupported escape {other:?}"),
                },
                Some(byte) => out.push(byte as char),
                None => panic!("golden JSON: unterminated string"),
            }
        }
        out
    }

    fn parse_number(&mut self) -> Json {
        let start = self.pos;
        if self.peek() == Some(b'-') {
            self.pos += 1;
        }
        while matches!(self.peek(), Some(b'0'..=b'9')) {
            self.pos += 1;
        }
        let text = std::str::from_utf8(&self.bytes[start..self.pos])
            .unwrap_or_else(|e| panic!("golden JSON: non-UTF8 number: {e}"));
        let value: i64 = text
            .parse()
            .unwrap_or_else(|e| panic!("golden JSON: bad number {text:?}: {e}"));
        Json::Num(value)
    }
}

fn parse_json(input: &str) -> Json {
    let mut parser = Parser::new(input);
    let value = parser.parse_value();
    parser.skip_ws();
    assert_eq!(
        parser.pos,
        parser.bytes.len(),
        "golden JSON: trailing bytes after the top-level value"
    );
    value
}

// ---------------------------------------------------------------------------
// The obligation-variable naming convention (see the module doc).
// ---------------------------------------------------------------------------

/// Whether `name` is `p<digits>_ob` — the shared corpus's convention for "this data
/// variable is an obligation flag, `0` discharged, non-zero open."
fn is_obligation_variable(name: &str) -> bool {
    let Some(rest) = name.strip_prefix('p') else {
        return false;
    };
    let digits_end = rest
        .find(|c: char| !c.is_ascii_digit())
        .unwrap_or(rest.len());
    digits_end > 0 && rest.get(digits_end..) == Some("_ob")
}

// ---------------------------------------------------------------------------
// The loader: shared JSON -> System.
// ---------------------------------------------------------------------------

fn to_int_expr(json: &Json) -> IntExpr {
    if let Some(constant) = json.try_get("const") {
        return IntExpr::constant(constant.as_value());
    }
    if let Some(name) = json.try_get("var") {
        return IntExpr::var(name.as_str());
    }
    let op = json.get("op").as_str();
    let args = json.get("args").as_arr();
    match op {
        "add" => IntExpr::plus(to_int_expr(&args[0]), to_int_expr(&args[1])),
        "sub" => IntExpr::minus(to_int_expr(&args[0]), to_int_expr(&args[1])),
        other => panic!("int expr: op {other:?} is outside the common sub-model"),
    }
}

fn to_bool_expr(json: &Json) -> BoolExpr {
    if let Some(constant) = json.try_get("const") {
        return match constant {
            Json::Bool(value) => BoolExpr::constant(*value),
            other => panic!("bool expr: `const` must be a JSON bool, found {other:?}"),
        };
    }
    let op = json.get("op").as_str();
    let args = json.get("args").as_arr();
    match op {
        "not" => BoolExpr::negate(to_bool_expr(&args[0])),
        "and" => BoolExpr::and(to_bool_expr(&args[0]), to_bool_expr(&args[1])),
        "or" => BoolExpr::or(to_bool_expr(&args[0]), to_bool_expr(&args[1])),
        "eq" => BoolExpr::compare(CmpOp::Eq, to_int_expr(&args[0]), to_int_expr(&args[1])),
        "ne" => BoolExpr::compare(CmpOp::Ne, to_int_expr(&args[0]), to_int_expr(&args[1])),
        "lt" => BoolExpr::compare(CmpOp::Lt, to_int_expr(&args[0]), to_int_expr(&args[1])),
        "le" => BoolExpr::compare(CmpOp::Le, to_int_expr(&args[0]), to_int_expr(&args[1])),
        "gt" => BoolExpr::compare(CmpOp::Gt, to_int_expr(&args[0]), to_int_expr(&args[1])),
        "ge" => BoolExpr::compare(CmpOp::Ge, to_int_expr(&args[0]), to_int_expr(&args[1])),
        other => panic!("bool expr: op {other:?} is outside the common sub-model"),
    }
}

fn ordered_pair(a: &str, b: &str) -> (String, String) {
    if a <= b {
        (a.to_owned(), b.to_owned())
    } else {
        (b.to_owned(), a.to_owned())
    }
}

fn load_system(system_json: &Json) -> System {
    let name = system_json.get("name").as_str();

    let mut builder = ModelBuilder::new();
    let mut kinds: BTreeMap<String, VarKind> = BTreeMap::new();
    for variable in system_json.get("variables").as_arr() {
        let var_name = variable.get("name").as_str();
        let type_obj = variable.get("type");
        let kind_tag = type_obj.get("kind").as_str();
        let (lo, hi) = match kind_tag {
            "bool" => (0, 1),
            "int" => (type_obj.get("min").as_i64(), type_obj.get("max").as_i64()),
            other => panic!(
                "{name}: variable {var_name}: kind {other:?} is outside the common sub-model \
                 (no enum domain on the Rust side)"
            ),
        };
        builder = builder.variable(var_name, lo, hi);
        let kind = if is_obligation_variable(var_name) {
            VarKind::Obligation
        } else {
            VarKind::Data
        };
        kinds.insert(var_name.to_owned(), kind);
    }

    let mut meta: BTreeMap<String, ActionMeta> = BTreeMap::new();
    for transition in system_json.get("transitions").as_arr() {
        let action_name = transition.get("name").as_str();
        let process = transition.get("process").as_str();
        let guard = to_bool_expr(transition.get("guard"));
        let updates_obj = transition.get("updates").as_obj();
        let updates: Vec<(&str, IntExpr)> = updates_obj
            .iter()
            .map(|(target, expr)| (target.as_str(), to_int_expr(expr)))
            .collect();
        builder = builder.action(ActionDecl::deterministic(action_name, guard, updates));

        let reads: BTreeSet<String> = transition
            .get("reads")
            .as_arr()
            .iter()
            .map(|j| j.as_str().to_owned())
            .collect();
        let writes: BTreeSet<String> = transition
            .get("writes")
            .as_arr()
            .iter()
            .map(|j| j.as_str().to_owned())
            .collect();
        let process_index: u32 = process
            .strip_prefix('p')
            .and_then(|digits| digits.parse().ok())
            .unwrap_or(0);
        meta.insert(
            action_name.to_owned(),
            ActionMeta {
                process: process_index,
                role: Role::Work,
                footprint: Footprint::new(reads, writes),
                fairness: Fairness::Unfair,
            },
        );
    }

    let init_obj = system_json.get("init").as_obj();
    let initial: Vec<(String, i64)> = init_obj
        .iter()
        .map(|(name, value)| (name.clone(), value.as_value()))
        .collect();
    let borrowed: Vec<(&str, i64)> = initial.iter().map(|(k, v)| (k.as_str(), *v)).collect();
    builder = builder.initial_state(&borrowed);

    let model = builder
        .build()
        .unwrap_or_else(|e| panic!("{name}: the loaded model does not build: {e}"));

    let mut conflicts: BTreeSet<(String, String)> = BTreeSet::new();
    for pair in system_json.get("conflicts").as_arr() {
        let items = pair.as_arr();
        conflicts.insert(ordered_pair(items[0].as_str(), items[1].as_str()));
    }

    let obligations: Vec<ObligationDecl> = kinds
        .iter()
        .filter(|(_, kind)| **kind == VarKind::Obligation)
        .map(|(variable, _)| ObligationDecl {
            name: format!("obligation:{variable}"),
            variable: variable.clone(),
            owner_phase: None,
        })
        .collect();

    System::new(SystemParts {
        model,
        kinds,
        meta,
        conflicts,
        obligations,
        phases: Vec::new(),
        lineage: vec![format!("differential-golden {name}")],
    })
    .unwrap_or_else(|e| panic!("{name}: the loaded declarations do not validate: {e:?}"))
}

// ---------------------------------------------------------------------------
// Running the Rust oracle and extracting the compared facts.
// ---------------------------------------------------------------------------

/// Generous enough that no corpus system's interleaving enumeration is ever truncated
/// (`run_facts` asserts `truncated == 0` rather than assuming it) or refused for size —
/// the corpus's declared state-space product is in the low hundreds at most, and its
/// longest single-process action sequence is at most 4 steps (see the Python module
/// doc's shape description), so a depth of 16 covers both processes' full sequences
/// with headroom.
const LIMITS: Limits = Limits {
    max_variables: 16,
    max_actions: 32,
    max_states: 4096,
    max_depth: 16,
    max_interleavings: 1 << 20,
};

#[derive(Debug, Clone)]
struct Facts {
    states: usize,
    dependent: Vec<(String, String)>,
    interleavings_complete: u64,
    trace_classes: u64,
    findings: Vec<String>,
}

fn run_facts(system: &System) -> Facts {
    let artifact = run_oracle(system, &LIMITS)
        .unwrap_or_else(|refusal| panic!("oracle refused a corpus system: {refusal:?}"));
    let interleavings = artifact.interleavings();
    assert_eq!(
        interleavings.truncated, 0,
        "a corpus system produced a truncated interleaving; raise LIMITS.max_depth or shrink the corpus"
    );
    let dependent: Vec<(String, String)> = artifact
        .dependent_pairs()
        .into_iter()
        .map(|(a, b)| (a.to_owned(), b.to_owned()))
        .collect();
    let findings: Vec<String> = artifact
        .findings()
        .iter()
        .map(|finding| finding.kind().to_owned())
        .collect();
    Facts {
        states: artifact.states().len(),
        dependent,
        interleavings_complete: interleavings.complete,
        trace_classes: interleavings.classes,
        findings,
    }
}

// ---------------------------------------------------------------------------
// The golden file and the comparator.
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
struct GoldenOracle {
    states: usize,
    dependent: Vec<(String, String)>,
    interleavings_complete: u64,
    trace_classes: u64,
    findings: Vec<String>,
}

struct GoldenEntry {
    seed: i64,
    system_json: Json,
    oracle: GoldenOracle,
}

fn golden_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tools/test-policy/evidence/differential_corpus.json")
}

fn load_golden() -> Vec<GoldenEntry> {
    let path = golden_path();
    let text = fs::read_to_string(&path).unwrap_or_else(|e| {
        panic!(
            "reading {}: {e} (run `python3 tools/test-policy/differential_corpus.py --write`)",
            path.display()
        )
    });
    let root = parse_json(&text);
    assert_eq!(
        root.get("schema").as_str(),
        "continuum-differential-system-v1",
        "golden schema mismatch; regenerate with tools/test-policy/differential_corpus.py --write"
    );
    root.get("systems")
        .as_arr()
        .iter()
        .map(|entry| {
            let seed = entry.get("seed").as_i64();
            let system_json = entry.get("system").clone();
            let oracle_json = entry.get("oracle");
            let dependent = oracle_json
                .get("dependent")
                .as_arr()
                .iter()
                .map(|pair| {
                    let items = pair.as_arr();
                    (items[0].as_str().to_owned(), items[1].as_str().to_owned())
                })
                .collect();
            let findings = oracle_json
                .get("findings")
                .as_arr()
                .iter()
                .map(|f| f.as_str().to_owned())
                .collect();
            GoldenEntry {
                seed,
                system_json,
                oracle: GoldenOracle {
                    states: usize::try_from(oracle_json.get("states").as_i64())
                        .expect("state count is non-negative"),
                    dependent,
                    interleavings_complete: u64::try_from(
                        oracle_json.get("interleavings_complete").as_i64(),
                    )
                    .expect("interleaving count is non-negative"),
                    trace_classes: u64::try_from(oracle_json.get("trace_classes").as_i64())
                        .expect("trace-class count is non-negative"),
                    findings,
                },
            }
        })
        .collect()
}

/// A comparison mismatch: which system (by seed) and which compared field disagreed.
/// Never a bare boolean (INV-008) — the field it names is what makes this a diagnostic
/// and not just a failed assertion.
#[derive(Debug)]
enum Disagreement {
    States {
        seed: i64,
        python: usize,
        rust: usize,
    },
    Dependent {
        seed: i64,
        python: Vec<(String, String)>,
        rust: Vec<(String, String)>,
    },
    Interleavings {
        seed: i64,
        python: u64,
        rust: u64,
    },
    TraceClasses {
        seed: i64,
        python: u64,
        rust: u64,
    },
    Findings {
        seed: i64,
        python: Vec<String>,
        rust: Vec<String>,
    },
}

impl fmt::Display for Disagreement {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::States { seed, python, rust } => {
                write!(f, "seed {seed}: states: python={python} rust={rust}")
            }
            Self::Dependent { seed, python, rust } => {
                write!(
                    f,
                    "seed {seed}: dependent pairs: python={python:?} rust={rust:?}"
                )
            }
            Self::Interleavings { seed, python, rust } => {
                write!(
                    f,
                    "seed {seed}: complete interleavings: python={python} rust={rust}"
                )
            }
            Self::TraceClasses { seed, python, rust } => {
                write!(
                    f,
                    "seed {seed}: mazurkiewicz trace classes: python={python} rust={rust}"
                )
            }
            Self::Findings { seed, python, rust } => {
                write!(
                    f,
                    "seed {seed}: finding kinds: python={python:?} rust={rust:?}"
                )
            }
        }
    }
}

fn compare(seed: i64, golden: &GoldenOracle, facts: &Facts) -> Result<(), Disagreement> {
    if golden.states != facts.states {
        return Err(Disagreement::States {
            seed,
            python: golden.states,
            rust: facts.states,
        });
    }
    if golden.dependent != facts.dependent {
        return Err(Disagreement::Dependent {
            seed,
            python: golden.dependent.clone(),
            rust: facts.dependent.clone(),
        });
    }
    if golden.interleavings_complete != facts.interleavings_complete {
        return Err(Disagreement::Interleavings {
            seed,
            python: golden.interleavings_complete,
            rust: facts.interleavings_complete,
        });
    }
    if golden.trace_classes != facts.trace_classes {
        return Err(Disagreement::TraceClasses {
            seed,
            python: golden.trace_classes,
            rust: facts.trace_classes,
        });
    }
    let mut python_findings = golden.findings.clone();
    python_findings.sort();
    let mut rust_findings = facts.findings.clone();
    rust_findings.sort();
    if python_findings != rust_findings {
        return Err(Disagreement::Findings {
            seed,
            python: python_findings,
            rust: rust_findings,
        });
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// The tests.
// ---------------------------------------------------------------------------

#[test]
fn differential_corpus_agrees_with_python_oracle() {
    let golden = load_golden();
    assert!(
        golden.len() >= 32,
        "corpus suspiciously small ({} systems); tools/test-policy/differential_corpus.py's CORPUS_SIZE changed?",
        golden.len()
    );
    let mut disagreements: Vec<String> = Vec::new();
    for entry in &golden {
        let system = load_system(&entry.system_json);
        let facts = run_facts(&system);
        if let Err(disagreement) = compare(entry.seed, &entry.oracle, &facts) {
            disagreements.push(disagreement.to_string());
        }
    }
    assert!(
        disagreements.is_empty(),
        "{} of {} corpus system(s) disagree:\n{}",
        disagreements.len(),
        golden.len(),
        disagreements.join("\n")
    );
}

/// Negative control (bn-1kgnz): the comparator must fail closed, and name the field it
/// failed on, rather than silently accept a corrupted golden entry. Each perturbation
/// below touches exactly one compared field, so a comparator that collapsed all
/// disagreements into one variant (or, worse, swallowed one of them) would be caught
/// here, not just "some assertion elsewhere failed".
#[test]
fn perturbed_golden_is_rejected() {
    let golden = load_golden();
    let entry = golden.first().expect("the committed corpus is non-empty");
    let system = load_system(&entry.system_json);
    let facts = run_facts(&system);
    assert!(
        compare(entry.seed, &entry.oracle, &facts).is_ok(),
        "sanity check: the unperturbed golden entry must agree with the Rust oracle first"
    );

    let mut perturbed = entry.oracle.clone();
    perturbed.states = perturbed.states.wrapping_add(1);
    assert!(matches!(
        compare(entry.seed, &perturbed, &facts),
        Err(Disagreement::States { .. })
    ));

    let mut perturbed = entry.oracle.clone();
    perturbed
        .dependent
        .push(("__perturbed_a".to_owned(), "__perturbed_b".to_owned()));
    assert!(matches!(
        compare(entry.seed, &perturbed, &facts),
        Err(Disagreement::Dependent { .. })
    ));

    let mut perturbed = entry.oracle.clone();
    perturbed.interleavings_complete = perturbed.interleavings_complete.wrapping_add(1);
    assert!(matches!(
        compare(entry.seed, &perturbed, &facts),
        Err(Disagreement::Interleavings { .. })
    ));

    let mut perturbed = entry.oracle.clone();
    perturbed.trace_classes = perturbed.trace_classes.wrapping_add(1);
    assert!(matches!(
        compare(entry.seed, &perturbed, &facts),
        Err(Disagreement::TraceClasses { .. })
    ));

    let mut perturbed = entry.oracle.clone();
    perturbed
        .findings
        .push("fabricated-finding-kind".to_owned());
    assert!(matches!(
        compare(entry.seed, &perturbed, &facts),
        Err(Disagreement::Findings { .. })
    ));
}
