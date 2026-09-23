//! KILL-09 leading indicator and falsification assay: *intent diff cannot reliably
//! expose gaming in supported fragments* (`notes/plan/plan.md` §24; bn-3kql).
//!
//! # What this file is
//!
//! Plan §24 names the kill criterion and states no threshold. The bar it is graded
//! against is the one the dossier states for the same guarantee: plan §5.3 and
//! RFC 0031 "Completeness guarantee" ("within declared supported fragments, every
//! property weakening, assumption strengthening, bound decrease, observer
//! coarsening, fault removal, fairness addition or removal, and assurance downgrade
//! MUST be classified as a privileged intent change"), and G3's "every gaming
//! mutation … that falls in a supported fragment". "Every" is the threshold. This
//! file invents none.
//!
//! It produces one retained, byte-stable artifact,
//! `continuum-semantic-diff kill09-indicator v1`
//! (`tests/golden/kill09_intent_diff_indicator.txt`), in four parts:
//!
//! 1. **The leading indicator** — the exposure-miss rate and the silent-pass rate of
//!    the production diff over the C020 hidden-mutant corpus (the same seeded
//!    generator, `support/c020_mutant_generator.rs`, primary seed plus the sixteen
//!    held-out seeds), broken down by the seven G3 dimensions, with the
//!    affirmed-versus-fail-closed split and the control precision. A cross-check
//!    ties the totals to the C020 golden ledger, so the two artifacts cannot drift
//!    apart silently.
//! 2. **A semantic differential falsifier** — random authored formula pairs (an
//!    original and an agent-style edit of it), sent through the real front door
//!    (`cpnf::normalize`, `IntentContract::decode`, `artifact::assemble`), and graded
//!    against a ground truth that does not use the classifier: a direct evaluator
//!    over a fixed set of lasso traces and finite domain interpretations. A trace
//!    the evaluator finds is a genuine counterexample, so any `unchanged`,
//!    `strengthened`, or `weakened` it refutes is an unsound classification, and any
//!    `allow` on a pair it shows admits a new behavior is a silent pass.
//! 3. **The named adversarial assay** — hand-written attacks in the four families
//!    the bone names: adversarial encodings, cross-field compensation,
//!    fragment-boundary tricks, and policy-table edits.
//! 4. **The policy verb matrix** — every W6-admissible verb on every field against
//!    that field's dangerous relations, on the agent acceptance path, so a
//!    protective-looking verb that admits a gaming relation cannot hide.
//!
//! The trend series is `tests/golden/kill09_indicator_trend.txt`. Its last row must
//! equal this revision's measurement, so a change in any indicator figure fails
//! the gate until a new, named trend row is appended.
//!
//! # What this file does not decide
//!
//! It measures. The continue/narrow/defer/kill decision for KILL-09 is a privileged
//! program decision (bn-3kql: "Agents do not take the privileged program
//! decision"); the package is `notes/plan/notes/KILL09_CHECKPOINT.md`.

#[path = "support/c020_mutant_generator.rs"]
#[allow(dead_code)]
mod generator;

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use continuum_intent::assurance_policy::AssuranceLevel;
use continuum_intent::ast::{Binder, ComparisonOperator, Formula, Identifier, Literal, Term};
use continuum_intent::canonical_json::Json;
use continuum_intent::change_policy::{
    AcceptancePath, ClassificationRecord, PolicyDecision, PolicyField, PolicyReviewers,
    PolicyTable, PolicyVerb, Relation,
};
use continuum_intent::contract::{IntentContract, IntentId};
use continuum_intent::cpnf::normalize;
use continuum_semantic_diff::artifact::{DiffArtifact, DiffId, DiffRequest, SnapshotId, assemble};

use generator::{Base, Direction, Expectation, Field, Mutant, Rng, generate};

/// The artifact's stable identity line.
const ARTIFACT_ID: &str = "continuum-semantic-diff kill09-indicator v1";

/// C020's primary seed, variants, and held-out sweep, restated so the indicator
/// measures the same corpus. `the_indicator_agrees_with_the_c020_ledger` pins that.
const C020_SEED: u64 = 0xC020_5EED_0000_0001;
const C020_VARIANTS: usize = 4;
const C020_HELD_OUT_VARIANTS: usize = 2;
const C020_HELD_OUT_SEEDS: [u64; 16] = [
    0xC020_0000_0000_0010,
    0xC020_0000_0000_0011,
    0xC020_0000_0000_0012,
    0xC020_0000_0000_0013,
    0xC020_0000_0000_0014,
    0xC020_0000_0000_0015,
    0xC020_0000_0000_0016,
    0xC020_0000_0000_0017,
    0xC020_0000_0000_0018,
    0xC020_0000_0000_0019,
    0xC020_0000_0000_001A,
    0xC020_0000_0000_001B,
    0xC020_0000_0000_001C,
    0xC020_0000_0000_001D,
    0xC020_0000_0000_001E,
    0xC020_0000_0000_001F,
];

/// The differential falsifier's seed and size. Fixed before the first run.
const DIFFERENTIAL_SEED: u64 = 0x4B09_D1FF_0000_0001;
const DIFFERENTIAL_PAIRS: usize = 3000;

const BASELINE: &str = include_str!("../../../tools/governance/t09-property-baseline.json");
const GOLDEN: &str = include_str!("golden/kill09_intent_diff_indicator.txt");
const TREND: &str = include_str!("golden/kill09_indicator_trend.txt");
const C020_GOLDEN: &str = include_str!("golden/c020_hidden_mutant_evidence.txt");

// --- bases and the diff front door -----------------------------------------------------------

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("the repository root resolves")
}

fn object_field<'a>(json: &'a Json, key: &str) -> &'a Json {
    json.as_object()
        .and_then(|map| map.get(key))
        .unwrap_or_else(|| panic!("missing {key:?}"))
}

fn label_of(path: &str) -> String {
    let stem = Path::new(path)
        .file_name()
        .and_then(|name| name.to_str())
        .expect("a file name");
    stem.trim_end_matches(".json")
        .trim_end_matches("-contract")
        .replace('.', "-")
}

/// The T09-pinned committed contracts, in baseline order (C020's corpus).
fn bases() -> Vec<Base> {
    let baseline = Json::parse(BASELINE.as_bytes()).expect("the T09 baseline parses");
    let entries = object_field(&baseline, "entries")
        .as_array()
        .expect("entries is an array");
    let root = repo_root();
    entries
        .iter()
        .map(|entry| {
            let path = object_field(entry, "path").as_str().expect("a path");
            let text = std::fs::read_to_string(root.join(path))
                .unwrap_or_else(|e| panic!("{path} is readable: {e}"));
            Base {
                label: label_of(path),
                document: Json::parse(text.trim_end().as_bytes())
                    .unwrap_or_else(|e| panic!("{path} parses: {e:?}")),
            }
        })
        .collect()
}

fn base(label: &str) -> Json {
    bases()
        .into_iter()
        .find(|b| b.label == label)
        .unwrap_or_else(|| panic!("no base {label}"))
        .document
}

/// The before-document under the locked measurement policy (C020's).
fn locked(document: &Json) -> Json {
    edit(document, |map| {
        if let Some(Json::Object(policy)) = map.get_mut("policy") {
            for verb in policy.values_mut() {
                *verb = Json::String("locked".to_owned());
            }
        }
    })
}

fn decode(document: &Json) -> Result<IntentContract, String> {
    IntentContract::decode(&document.to_canonical_bytes()).map_err(|e| format!("{e:?}"))
}

fn diff(
    before: &IntentContract,
    after: &IntentContract,
    requested: AssuranceLevel,
    path: AcceptancePath,
) -> Result<DiffArtifact, String> {
    assemble(&DiffRequest {
        diff_id: DiffId::new("diff_kill09").expect("well formed"),
        before_snapshot: SnapshotId::new("ws_kill09_before").expect("well formed"),
        after_snapshot: SnapshotId::new("ws_kill09_after").expect("well formed"),
        before_intent: IntentId::new("in_kill09_before").expect("well formed"),
        after_intent: IntentId::new("in_kill09_after").expect("well formed"),
        before,
        after,
        requested_assurance: requested,
        path,
        semantic_changes: Vec::new(),
        evidence: Vec::new(),
    })
    .map_err(|e| format!("{e:?}"))
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Record {
    field: PolicyField,
    unit: Option<String>,
    relation: Relation,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Outcome {
    RejectedAtDecode,
    Refused(String),
    Assembled {
        records: Vec<Record>,
        locked: PolicyDecision,
        locked_observed: PolicyDecision,
        own: PolicyDecision,
    },
}

/// One after-document against one before-document: the locked measurement policy
/// at `bounded` and at `observed`, and the before-document's own table, all on the
/// agent acceptance path.
fn run(before_own: &Json, after: &Json) -> Outcome {
    let own = decode(before_own).expect("the before-document decodes");
    let locked_before = decode(&locked(before_own)).expect("the locked variant decodes");
    run_prepared(&own, &locked_before, after)
}

fn run_prepared(own: &IntentContract, locked_before: &IntentContract, after: &Json) -> Outcome {
    let Ok(after) = decode(after) else {
        return Outcome::RejectedAtDecode;
    };
    let path = AcceptancePath::AgentAccept;
    let bounded = match diff(locked_before, &after, AssuranceLevel::Bounded, path) {
        Ok(artifact) => artifact,
        Err(e) => return Outcome::Refused(e),
    };
    let observed = match diff(locked_before, &after, AssuranceLevel::Observed, path) {
        Ok(artifact) => artifact.decision(),
        Err(e) => return Outcome::Refused(e),
    };
    let own = match diff(own, &after, AssuranceLevel::Bounded, path) {
        Ok(artifact) => artifact.decision(),
        Err(e) => return Outcome::Refused(e),
    };
    Outcome::Assembled {
        records: records_of(&bounded),
        locked: bounded.decision(),
        locked_observed: observed,
        own,
    }
}

fn records_of(artifact: &DiffArtifact) -> Vec<Record> {
    artifact
        .intent_changes()
        .iter()
        .map(|record| Record {
            field: record.field(),
            unit: record.unit().map(str::to_owned),
            relation: record.relation(),
        })
        .collect()
}

// --- JSON editing ----------------------------------------------------------------------------

fn edit(document: &Json, f: impl FnOnce(&mut BTreeMap<String, Json>)) -> Json {
    let mut out = document.clone();
    if let Json::Object(map) = &mut out {
        f(map);
    }
    out
}

fn obj(json: &mut Json) -> &mut BTreeMap<String, Json> {
    match json {
        Json::Object(map) => map,
        other => panic!("not an object: {other:?}"),
    }
}

fn arr(json: &mut Json) -> &mut Vec<Json> {
    match json {
        Json::Array(items) => items,
        other => panic!("not an array: {other:?}"),
    }
}

fn s(text: &str) -> Json {
    Json::String(text.to_owned())
}

/// The unit of a keyed array (`claims`, `assumptions`, `observers`) with this `id`.
fn unit_mut<'a>(map: &'a mut BTreeMap<String, Json>, group: &str, id: &str) -> &'a mut Json {
    arr(map.get_mut(group).expect("the group exists"))
        .iter_mut()
        .find(|unit| {
            unit.as_object()
                .and_then(|u| u.get("id"))
                .and_then(Json::as_str)
                == Some(id)
        })
        .unwrap_or_else(|| panic!("no {group} unit {id}"))
}

/// Replace a claim or assumption expression with the CPNF-1 form of `ast`.
fn set_expression(map: &mut BTreeMap<String, Json>, group: &str, id: &str, ast: &Formula) {
    let normal = normalize(ast).expect("the attack formula normalizes");
    let unit = obj(unit_mut(map, group, id));
    let expression = obj(unit.get_mut("expression").expect("an expression"));
    expression.insert("ast".to_owned(), normal.to_json());
}

fn ident(name: &str) -> Identifier {
    Identifier::new(name).expect("a well-formed identifier")
}

fn state(name: &str) -> Term {
    Term::State {
        name: ident(name),
        indices: Vec::new(),
    }
}

fn int(value: i64) -> Term {
    Term::Literal {
        value: Literal::Integer(value),
    }
}

fn cmp(op: ComparisonOperator, left: Term, right: Term) -> Formula {
    Formula::compare(op, left, right)
}

fn pred(name: &str) -> Formula {
    Formula::predicate(ident(name), Vec::new())
}

fn and(operands: Vec<Formula>) -> Formula {
    Formula::and(operands).expect("a well-formed conjunction")
}

fn or(operands: Vec<Formula>) -> Formula {
    Formula::or(operands).expect("a well-formed disjunction")
}

// --- part 1: the leading indicator over the C020 corpus -------------------------------------

/// The relations that read an edit of this direction as harmless (C020's table).
const fn benign(direction: Direction) -> &'static [Relation] {
    match direction {
        Direction::WeakensProperty => {
            &[Relation::Unchanged, Relation::Strengthened, Relation::Added]
        }
        Direction::StrengthensAssumption | Direction::StrengthensFairness => {
            &[Relation::Unchanged, Relation::Weakened, Relation::Removed]
        }
        Direction::ShrinksBound => &[Relation::Unchanged, Relation::Expanded],
        Direction::CoarsensObserver => &[Relation::Unchanged, Relation::Refined, Relation::Added],
        Direction::RemovesFault => &[Relation::Unchanged, Relation::Added],
        Direction::DowngradesAssurance => &[Relation::Unchanged, Relation::Upgraded],
        Direction::ChangesFairness | Direction::ChangesAssurance => &[Relation::Unchanged],
    }
}

/// The affirmed relation each direction names (C020's table).
const fn affirmed(direction: Direction) -> &'static [Relation] {
    match direction {
        Direction::WeakensProperty => &[Relation::Weakened, Relation::Removed],
        Direction::StrengthensAssumption | Direction::StrengthensFairness => {
            &[Relation::Strengthened, Relation::Added]
        }
        Direction::ShrinksBound => &[Relation::Contracted],
        Direction::CoarsensObserver => &[Relation::Coarsened, Relation::Removed],
        Direction::RemovesFault => &[Relation::Removed],
        Direction::DowngradesAssurance => &[Relation::Downgraded],
        Direction::ChangesFairness => &[Relation::Removed],
        Direction::ChangesAssurance => &[Relation::Added],
    }
}

const fn policy_field(field: Field) -> PolicyField {
    match field {
        Field::Properties => PolicyField::Properties,
        Field::Assumptions => PolicyField::Assumptions,
        Field::Bounds => PolicyField::Bounds,
        Field::Observers => PolicyField::Observers,
        Field::Faults => PolicyField::Faults,
        Field::Fairness => PolicyField::Fairness,
        Field::Assurance => PolicyField::Assurance,
    }
}

const DIMENSIONS: [Field; 7] = [
    Field::Properties,
    Field::Assumptions,
    Field::Bounds,
    Field::Observers,
    Field::Faults,
    Field::Fairness,
    Field::Assurance,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Grade {
    Caught {
        affirmed: bool,
    },
    /// The target field carries no record outside its benign set: not exposed.
    Unexposed,
    /// Allowed under the locked measurement policy: a silent pass.
    Silent,
    Refused,
    RejectedAtDoor,
    ControlAllowed,
    ControlFlagged,
}

fn grade(expectation: &Expectation, outcome: &Outcome) -> Grade {
    match (expectation, outcome) {
        (Expectation::Weakening(_), Outcome::RejectedAtDecode) => Grade::RejectedAtDoor,
        (Expectation::Weakening(_), Outcome::Refused(_)) => Grade::Refused,
        (
            Expectation::Weakening(targets),
            Outcome::Assembled {
                records,
                locked,
                locked_observed,
                ..
            },
        ) => {
            if *locked == PolicyDecision::Allow || *locked_observed == PolicyDecision::Allow {
                return Grade::Silent;
            }
            let mut all_affirmed = true;
            for target in targets {
                let field = policy_field(target.field);
                let on_field: Vec<Relation> = records
                    .iter()
                    .filter(|record| record.field == field)
                    .map(|record| record.relation)
                    .collect();
                if !on_field
                    .iter()
                    .any(|relation| !benign(target.direction).contains(relation))
                {
                    return Grade::Unexposed;
                }
                all_affirmed &= on_field
                    .iter()
                    .any(|relation| affirmed(target.direction).contains(relation));
            }
            Grade::Caught {
                affirmed: all_affirmed,
            }
        }
        (
            Expectation::Control,
            Outcome::Assembled {
                records, locked, ..
            },
        ) if records.is_empty() && *locked == PolicyDecision::Allow => Grade::ControlAllowed,
        (Expectation::Control, _) => Grade::ControlFlagged,
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
struct Tally {
    weakening: usize,
    reached: usize,
    caught: usize,
    affirmed: usize,
    unexposed: usize,
    silent: usize,
    refused: usize,
    rejected: usize,
    controls: usize,
    controls_allowed: usize,
    own_allow: usize,
}

impl Tally {
    fn add(&mut self, grade: Grade, own_allow: bool) {
        match grade {
            Grade::ControlAllowed | Grade::ControlFlagged => {
                self.controls += 1;
                self.controls_allowed += usize::from(grade == Grade::ControlAllowed);
                return;
            }
            _ => self.weakening += 1,
        }
        match grade {
            Grade::Caught { affirmed } => {
                self.caught += 1;
                self.affirmed += usize::from(affirmed);
            }
            Grade::Unexposed => self.unexposed += 1,
            Grade::Silent => self.silent += 1,
            Grade::Refused => self.refused += 1,
            Grade::RejectedAtDoor => self.rejected += 1,
            Grade::ControlAllowed | Grade::ControlFlagged => {}
        }
        self.reached = self.weakening - self.rejected;
        self.own_allow += usize::from(own_allow);
    }

    fn merge(&mut self, other: &Self) {
        self.weakening += other.weakening;
        self.reached += other.reached;
        self.caught += other.caught;
        self.affirmed += other.affirmed;
        self.unexposed += other.unexposed;
        self.silent += other.silent;
        self.refused += other.refused;
        self.rejected += other.rejected;
        self.controls += other.controls;
        self.controls_allowed += other.controls_allowed;
        self.own_allow += other.own_allow;
    }
}

/// A conservative one-sided 95% upper bound on a rate with zero failures in `n`
/// trials, in parts per million: `1 - 0.05^(1/n) <= -ln(0.05)/n`, and
/// `-ln(0.05) < 2.995733`. Integer arithmetic, so the artifact is byte-stable.
fn zero_failure_bound_ppm(n: usize) -> usize {
    if n == 0 {
        return 1_000_000;
    }
    2_995_733_usize.div_ceil(n)
}

struct IndicatorRow {
    grade: Grade,
    targets: Vec<Field>,
    own_allow: bool,
}

fn indicator_campaign(seed: u64, variants: usize) -> Vec<IndicatorRow> {
    let bases = bases();
    let prepared: BTreeMap<String, (IntentContract, IntentContract)> = bases
        .iter()
        .map(|b| {
            (
                b.label.clone(),
                (
                    decode(&b.document).expect("decodes"),
                    decode(&locked(&b.document)).expect("decodes"),
                ),
            )
        })
        .collect();
    generate(seed, &bases, variants)
        .into_iter()
        .map(|mutant: Mutant| {
            let (own, locked_before) = &prepared[&mutant.base];
            let outcome = run_prepared(own, locked_before, &mutant.document);
            let grade = grade(&mutant.expectation, &outcome);
            let own_allow = matches!(
                (&mutant.expectation, &outcome),
                (
                    Expectation::Weakening(_),
                    Outcome::Assembled {
                        own: PolicyDecision::Allow,
                        ..
                    }
                )
            );
            let targets = match &mutant.expectation {
                Expectation::Weakening(targets) => targets.iter().map(|t| t.field).collect(),
                Expectation::Control => Vec::new(),
            };
            IndicatorRow {
                grade,
                targets,
                own_allow,
            }
        })
        .collect()
}

struct Indicator {
    primary: Tally,
    held_out: Tally,
    per_dimension: BTreeMap<Field, Tally>,
}

fn indicator() -> Indicator {
    let mut per_dimension: BTreeMap<Field, Tally> = BTreeMap::new();
    let mut primary = Tally::default();
    let mut rows = indicator_campaign(C020_SEED, C020_VARIANTS);
    for row in &rows {
        primary.add(row.grade, row.own_allow);
    }
    let mut held_out = Tally::default();
    for seed in C020_HELD_OUT_SEEDS {
        let sweep = indicator_campaign(seed, C020_HELD_OUT_VARIANTS);
        for row in &sweep {
            held_out.add(row.grade, row.own_allow);
        }
        rows.extend(sweep);
    }
    for row in &rows {
        let dims: BTreeSet<Field> = row.targets.iter().copied().collect();
        for dim in dims {
            per_dimension
                .entry(dim)
                .or_default()
                .add(row.grade, row.own_allow);
        }
    }
    Indicator {
        primary,
        held_out,
        per_dimension,
    }
}

// --- part 2: the semantic differential falsifier --------------------------------------------

/// A world: a lasso trace over 4-bit states plus an interpretation of the two
/// domain constants over the two-element universe `{0, 1}`.
///
/// State bits: 0 = `p()`, 1 = `s == 1`, 2 = `r(0)`, 3 = `r(1)`. The trace is
/// `states[0..]` with `states[loop_start..]` repeated forever, so `always` and
/// `eventually` at position `i` range over `min(i, loop_start)..len`.
#[derive(Debug, Clone)]
struct World {
    states: Vec<u8>,
    loop_start: usize,
    domains: [u8; 2],
}

const DOMAIN_NAMES: [&str; 2] = ["D", "C"];

fn worlds(with_domains: bool) -> Vec<World> {
    let mut traces: Vec<(Vec<u8>, usize)> = Vec::new();
    for a in 0..16u8 {
        traces.push((vec![a], 0));
    }
    for a in 0..16u8 {
        for b in 0..16u8 {
            traces.push((vec![a, b], 0));
            traces.push((vec![a, b], 1));
        }
    }
    let domain_choices: Vec<[u8; 2]> = if with_domains {
        let mut out = Vec::new();
        for d in [0b00u8, 0b01, 0b11] {
            for c in [0b00u8, 0b10, 0b11] {
                out.push([d, c]);
            }
        }
        out
    } else {
        vec![[0b11, 0b11]]
    };
    let mut out = Vec::new();
    for (states, loop_start) in traces {
        for domains in &domain_choices {
            out.push(World {
                states: states.clone(),
                loop_start,
                domains: *domains,
            });
        }
    }
    out
}

fn has_quantifier(formula: &Formula) -> bool {
    match formula {
        Formula::Forall { .. } | Formula::Exists { .. } => true,
        Formula::Boolean { .. }
        | Formula::Predicate { .. }
        | Formula::Action { .. }
        | Formula::Compare { .. } => false,
        Formula::Not { operand }
        | Formula::Always { operand }
        | Formula::Eventually { operand } => has_quantifier(operand),
        Formula::And { operands } | Formula::Or { operands } => operands.iter().any(has_quantifier),
        Formula::Implies {
            antecedent,
            consequent,
        }
        | Formula::LeadsTo {
            antecedent,
            consequent,
        } => has_quantifier(antecedent) || has_quantifier(consequent),
        Formula::Iff { left, right } => has_quantifier(left) || has_quantifier(right),
    }
}

fn is_s(term: &Term) -> bool {
    matches!(term, Term::State { name, indices } if name.as_str() == "s" && indices.is_empty())
}

fn is_one(term: &Term) -> bool {
    matches!(
        term,
        Term::Literal {
            value: Literal::Integer(1)
        }
    )
}

/// The ground truth. It interprets the *authored* formula directly, innermost
/// binder wins, and it does not call anything in the classifier or the normalizer.
fn eval(formula: &Formula, world: &World, pos: usize, env: &mut Vec<(String, u8)>) -> bool {
    let bit = |b: u8| world.states[pos] >> b & 1 == 1;
    let future = || world.loop_start.min(pos)..world.states.len();
    match formula {
        Formula::Boolean { value } => *value,
        Formula::Predicate { name, args } => match (name.as_str(), args.as_slice()) {
            ("p", []) => bit(0),
            ("r", [Term::Var { name: var }]) => {
                let element = env
                    .iter()
                    .rev()
                    .find(|(bound, _)| bound == var.as_str())
                    .map(|(_, element)| *element)
                    .expect("a bound variable");
                bit(2 + element)
            }
            other => panic!("the differential generator never builds {other:?}"),
        },
        Formula::Compare { op, left, right } => {
            assert!(
                (is_s(left) && is_one(right)) || (is_one(left) && is_s(right)),
                "the differential generator compares only s with 1"
            );
            match op {
                ComparisonOperator::Eq => bit(1),
                ComparisonOperator::Ne => !bit(1),
                other => panic!("the differential generator never builds {other:?}"),
            }
        }
        Formula::Action { .. } => panic!("the differential generator never builds actions"),
        Formula::Not { operand } => !eval(operand, world, pos, env),
        Formula::And { operands } => operands.iter().all(|f| eval(f, world, pos, env)),
        Formula::Or { operands } => operands.iter().any(|f| eval(f, world, pos, env)),
        Formula::Implies {
            antecedent,
            consequent,
        } => !eval(antecedent, world, pos, env) || eval(consequent, world, pos, env),
        Formula::Iff { left, right } => eval(left, world, pos, env) == eval(right, world, pos, env),
        Formula::Always { operand } => future().all(|i| eval(operand, world, i, env)),
        Formula::Eventually { operand } => future().any(|i| eval(operand, world, i, env)),
        Formula::LeadsTo {
            antecedent,
            consequent,
        } => future().all(|i| {
            !eval(antecedent, world, i, env)
                || (world.loop_start.min(i)..world.states.len())
                    .any(|j| eval(consequent, world, j, env))
        }),
        Formula::Forall { binder, body } | Formula::Exists { binder, body } => {
            let universal = matches!(formula, Formula::Forall { .. });
            let Term::Constant { name } = &binder.domain else {
                panic!("the differential generator binds over constants only")
            };
            let index = DOMAIN_NAMES
                .iter()
                .position(|d| *d == name.as_str())
                .expect("a known domain");
            let mask = world.domains[index];
            let mut result = universal;
            for element in 0..2u8 {
                if mask >> element & 1 == 0 {
                    continue;
                }
                env.push((binder.variable.as_str().to_owned(), element));
                let holds = eval(body, world, pos, env);
                env.pop();
                if universal && !holds {
                    result = false;
                    break;
                }
                if !universal && holds {
                    result = true;
                    break;
                }
            }
            result
        }
    }
}

struct Gen {
    rng: Rng,
}

impl Gen {
    fn atom(&mut self, scope: &[String]) -> Formula {
        let choice = self.rng.below(if scope.is_empty() { 7 } else { 10 });
        match choice {
            0 | 1 => pred("p"),
            2 => cmp(ComparisonOperator::Eq, state("s"), int(1)),
            3 => cmp(ComparisonOperator::Eq, int(1), state("s")),
            4 => cmp(ComparisonOperator::Ne, state("s"), int(1)),
            5 => cmp(ComparisonOperator::Ne, int(1), state("s")),
            6 => Formula::boolean(self.rng.coin()),
            _ => {
                let var = &scope[self.rng.below(scope.len())];
                Formula::predicate(ident("r"), vec![Term::Var { name: ident(var) }])
            }
        }
    }

    fn binder(&mut self) -> Binder {
        let variable = if self.rng.coin() { "x" } else { "y" };
        let domain = DOMAIN_NAMES[self.rng.below(2)];
        Binder::new(
            ident(variable),
            Term::Constant {
                name: ident(domain),
            },
        )
    }

    fn formula(&mut self, depth: usize, scope: &mut Vec<String>) -> Formula {
        if depth == 0 || self.rng.below(4) == 0 {
            return self.atom(scope);
        }
        let d = depth - 1;
        match self.rng.below(10) {
            0 => Formula::not(self.formula(d, scope)),
            1 => {
                let n = 2 + self.rng.below(2);
                and((0..n).map(|_| self.formula(d, scope)).collect())
            }
            2 => {
                let n = 2 + self.rng.below(2);
                or((0..n).map(|_| self.formula(d, scope)).collect())
            }
            3 => Formula::implies(self.formula(d, scope), self.formula(d, scope)),
            4 => Formula::iff(self.formula(d, scope), self.formula(d, scope)),
            5 => Formula::always(self.formula(d, scope)),
            6 => Formula::eventually(self.formula(d, scope)),
            7 => Formula::leads_to(self.formula(d, scope), self.formula(d, scope)),
            _ => {
                let binder = self.binder();
                scope.push(binder.variable.as_str().to_owned());
                let body = self.formula(d, scope);
                scope.pop();
                if self.rng.coin() {
                    Formula::forall(binder, body)
                } else {
                    Formula::exists(binder, body)
                }
            }
        }
    }
}

/// Every subformula position in preorder, with the binder names in scope there.
fn positions(
    formula: &Formula,
    path: &mut Vec<usize>,
    scope: &mut Vec<String>,
    out: &mut Vec<(Vec<usize>, Vec<String>)>,
) {
    out.push((path.clone(), scope.clone()));
    let mut visit = |i: usize, child: &Formula, scope: &mut Vec<String>| {
        path.push(i);
        positions(child, path, scope, out);
        path.pop();
    };
    match formula {
        Formula::Boolean { .. }
        | Formula::Predicate { .. }
        | Formula::Action { .. }
        | Formula::Compare { .. } => {}
        Formula::Not { operand }
        | Formula::Always { operand }
        | Formula::Eventually { operand } => {
            visit(0, operand, scope);
        }
        Formula::And { operands } | Formula::Or { operands } => {
            for (i, operand) in operands.iter().enumerate() {
                visit(i, operand, scope);
            }
        }
        Formula::Implies {
            antecedent,
            consequent,
        }
        | Formula::LeadsTo {
            antecedent,
            consequent,
        } => {
            visit(0, antecedent, scope);
            visit(1, consequent, scope);
        }
        Formula::Iff { left, right } => {
            visit(0, left, scope);
            visit(1, right, scope);
        }
        Formula::Forall { binder, body } | Formula::Exists { binder, body } => {
            scope.push(binder.variable.as_str().to_owned());
            visit(0, body, scope);
            scope.pop();
        }
    }
}

fn replace_at(
    formula: &Formula,
    path: &[usize],
    with: &mut dyn FnMut(&Formula) -> Formula,
) -> Formula {
    let Some((&head, rest)) = path.split_first() else {
        return with(formula);
    };
    let mut child = |f: &Formula| replace_at(f, rest, with);
    match formula {
        Formula::Not { operand } => Formula::not(child(operand)),
        Formula::Always { operand } => Formula::always(child(operand)),
        Formula::Eventually { operand } => Formula::eventually(child(operand)),
        Formula::And { operands } | Formula::Or { operands } => {
            let mut operands = operands.clone();
            operands[head] = child(&operands[head]);
            if matches!(formula, Formula::And { .. }) {
                and(operands)
            } else {
                or(operands)
            }
        }
        Formula::Implies {
            antecedent,
            consequent,
        } => {
            if head == 0 {
                Formula::implies(child(antecedent), (**consequent).clone())
            } else {
                Formula::implies((**antecedent).clone(), child(consequent))
            }
        }
        Formula::LeadsTo {
            antecedent,
            consequent,
        } => {
            if head == 0 {
                Formula::leads_to(child(antecedent), (**consequent).clone())
            } else {
                Formula::leads_to((**antecedent).clone(), child(consequent))
            }
        }
        Formula::Iff { left, right } => {
            if head == 0 {
                Formula::iff(child(left), (**right).clone())
            } else {
                Formula::iff((**left).clone(), child(right))
            }
        }
        Formula::Forall { binder, body } => Formula::forall(binder.clone(), child(body)),
        Formula::Exists { binder, body } => Formula::exists(binder.clone(), child(body)),
        Formula::Boolean { .. }
        | Formula::Predicate { .. }
        | Formula::Action { .. }
        | Formula::Compare { .. } => unreachable!("atoms have no children"),
    }
}

/// Rename the references bound by the outermost binder of `body` named `from` to
/// `to`, naively: an inner binder named `from` stops the walk (it shadows), but an
/// outer `to` that becomes captured is *not* avoided. The ground truth decides what
/// the edit means.
fn rename_refs(formula: &Formula, from: &str, to: &str) -> Formula {
    let term = |t: &Term| match t {
        Term::Var { name } if name.as_str() == from => Term::Var { name: ident(to) },
        other => other.clone(),
    };
    match formula {
        Formula::Predicate { name, args } => {
            Formula::predicate(name.clone(), args.iter().map(term).collect())
        }
        Formula::Boolean { .. } | Formula::Action { .. } | Formula::Compare { .. } => {
            formula.clone()
        }
        Formula::Not { operand } => Formula::not(rename_refs(operand, from, to)),
        Formula::Always { operand } => Formula::always(rename_refs(operand, from, to)),
        Formula::Eventually { operand } => Formula::eventually(rename_refs(operand, from, to)),
        Formula::And { operands } => {
            and(operands.iter().map(|f| rename_refs(f, from, to)).collect())
        }
        Formula::Or { operands } => or(operands.iter().map(|f| rename_refs(f, from, to)).collect()),
        Formula::Implies {
            antecedent,
            consequent,
        } => Formula::implies(
            rename_refs(antecedent, from, to),
            rename_refs(consequent, from, to),
        ),
        Formula::LeadsTo {
            antecedent,
            consequent,
        } => Formula::leads_to(
            rename_refs(antecedent, from, to),
            rename_refs(consequent, from, to),
        ),
        Formula::Iff { left, right } => {
            Formula::iff(rename_refs(left, from, to), rename_refs(right, from, to))
        }
        Formula::Forall { binder, .. } | Formula::Exists { binder, .. }
            if binder.variable.as_str() == from =>
        {
            formula.clone()
        }
        Formula::Forall { binder, body } => {
            Formula::forall(binder.clone(), rename_refs(body, from, to))
        }
        Formula::Exists { binder, body } => {
            Formula::exists(binder.clone(), rename_refs(body, from, to))
        }
    }
}

const MUTATIONS: [&str; 16] = [
    "replace-subtree",
    "or-widen",
    "and-narrow",
    "double-negation",
    "tautology-disjunct",
    "implication-guard",
    "always-wrap",
    "eventually-wrap",
    "vacuous-forall",
    "leads-to-guard",
    "dual-swap",
    "drop-operand",
    "binder-rename",
    "implies-respell",
    "iff-true-respell",
    "duplicate-conjunct",
];

impl Gen {
    fn mutate(&mut self, before: &Formula) -> (usize, Formula) {
        let mut sites = Vec::new();
        positions(before, &mut Vec::new(), &mut Vec::new(), &mut sites);
        let (path, scope) = sites[self.rng.below(sites.len())].clone();
        let op = self.rng.below(MUTATIONS.len());
        let mut scope = scope;
        let fresh = self.formula(2, &mut scope);
        let binder = self.binder();
        let flip_domain = self.rng.coin();
        let after = replace_at(before, &path, &mut |t: &Formula| match op {
            0 => fresh.clone(),
            1 => or(vec![t.clone(), fresh.clone()]),
            2 => and(vec![t.clone(), fresh.clone()]),
            3 => Formula::not(Formula::not(t.clone())),
            4 => or(vec![t.clone(), Formula::iff(pred("p"), pred("p"))]),
            5 => Formula::implies(fresh.clone(), t.clone()),
            6 => Formula::always(t.clone()),
            7 => Formula::eventually(t.clone()),
            8 => Formula::forall(binder.clone(), t.clone()),
            9 => Formula::leads_to(fresh.clone(), t.clone()),
            10 => match t {
                Formula::Always { operand } => Formula::eventually((**operand).clone()),
                Formula::Eventually { operand } => Formula::always((**operand).clone()),
                Formula::And { operands } => or(operands.clone()),
                Formula::Or { operands } => and(operands.clone()),
                Formula::Forall { binder, body } if flip_domain => {
                    Formula::exists(binder.clone(), (**body).clone())
                }
                Formula::Exists { binder, body } if flip_domain => {
                    Formula::forall(binder.clone(), (**body).clone())
                }
                Formula::Forall { binder, body } | Formula::Exists { binder, body } => {
                    let other = if let Term::Constant { name } = &binder.domain
                        && name.as_str() == "D"
                    {
                        "C"
                    } else {
                        "D"
                    };
                    let swapped = Binder::new(
                        binder.variable.clone(),
                        Term::Constant { name: ident(other) },
                    );
                    if matches!(t, Formula::Forall { .. }) {
                        Formula::forall(swapped, (**body).clone())
                    } else {
                        Formula::exists(swapped, (**body).clone())
                    }
                }
                Formula::Compare { op, left, right } => {
                    let op = if *op == ComparisonOperator::Eq {
                        ComparisonOperator::Ne
                    } else {
                        ComparisonOperator::Eq
                    };
                    cmp(op, left.clone(), right.clone())
                }
                _ => fresh.clone(),
            },
            11 => match t {
                Formula::And { operands } | Formula::Or { operands } if operands.len() > 2 => {
                    let mut operands = operands.clone();
                    operands.pop();
                    if matches!(t, Formula::And { .. }) {
                        and(operands)
                    } else {
                        or(operands)
                    }
                }
                Formula::And { operands } | Formula::Or { operands } => operands[0].clone(),
                Formula::Not { operand } => (**operand).clone(),
                _ => fresh.clone(),
            },
            12 => match t {
                Formula::Forall { binder, body } | Formula::Exists { binder, body } => {
                    let from = binder.variable.as_str();
                    let to = if from == "x" { "y" } else { "x" };
                    let renamed = Binder::new(ident(to), binder.domain.clone());
                    let body = rename_refs(body, from, to);
                    if matches!(t, Formula::Forall { .. }) {
                        Formula::forall(renamed, body)
                    } else {
                        Formula::exists(renamed, body)
                    }
                }
                _ => fresh.clone(),
            },
            13 => Formula::implies(Formula::not(t.clone()), Formula::boolean(false)),
            14 => Formula::iff(t.clone(), Formula::boolean(true)),
            _ => and(vec![t.clone(), t.clone()]),
        });
        (op, after)
    }
}

/// What the ground truth says about one pair over the world set.
#[derive(Debug, Clone, Copy, Default)]
struct Truth {
    /// Some world satisfies `after` and not `before`: the edit admits a new
    /// behavior. A genuine counterexample to `after ⊆ before`.
    after_escapes: bool,
    /// Some world satisfies `before` and not `after`.
    before_escapes: bool,
}

fn truth(before: &Formula, after: &Formula, all: &[World], plain: &[World]) -> Truth {
    let set = if has_quantifier(before) || has_quantifier(after) {
        all
    } else {
        plain
    };
    let mut t = Truth::default();
    let mut env = Vec::new();
    for world in set {
        let b = eval(before, world, 0, &mut env);
        let a = eval(after, world, 0, &mut env);
        t.after_escapes |= a && !b;
        t.before_escapes |= b && !a;
        if t.after_escapes && t.before_escapes {
            break;
        }
    }
    t
}

fn with_not_solved(document: &Json, ast: &Formula) -> Json {
    edit(document, |map| {
        set_expression(map, "claims", "NotSolved", ast)
    })
}

#[derive(Debug, Default, Clone)]
struct Differential {
    pairs: usize,
    normalize_rejected: usize,
    normal_form_unsound: usize,
    decode_rejected: usize,
    /// Classifier relation on the edited claim, by wire token.
    relations: BTreeMap<&'static str, usize>,
    /// Ground-truth class: `equivalent`, `strict-weakening`, `strict-strengthening`,
    /// `incomparable` (over the world set).
    truths: BTreeMap<&'static str, usize>,
    unsound: usize,
    silent: usize,
    strict_weakenings: usize,
    strict_weakenings_affirmed: usize,
    per_mutation: BTreeMap<&'static str, (usize, usize)>,
    /// The same pairs placed on the `JugCapacities` assumption, where the dangerous
    /// direction is `strengthened` (RFC 0031: "exactly as for claims").
    assumption_relations: BTreeMap<&'static str, usize>,
    assumption_unsound: usize,
    assumption_silent: usize,
    strict_strengthenings: usize,
    strict_strengthenings_affirmed: usize,
    counterexamples: Vec<String>,
}

fn with_jug_capacities(document: &Json, ast: &Formula) -> Json {
    edit(document, |map| {
        set_expression(map, "assumptions", "JugCapacities", ast)
    })
}

/// Whether the ground truth refutes the relation the classifier affirmed.
const fn refutes(relation: Relation, t: Truth) -> bool {
    match relation {
        Relation::Unchanged => t.after_escapes || t.before_escapes,
        Relation::Strengthened => t.after_escapes,
        Relation::Weakened => t.before_escapes,
        _ => false,
    }
}

fn differential() -> Differential {
    differential_sweep(DIFFERENTIAL_SEED, DIFFERENTIAL_PAIRS)
}

#[allow(clippy::too_many_lines)]
fn differential_sweep(seed: u64, pairs: usize) -> Differential {
    let all = worlds(true);
    let plain = worlds(false);
    let die_hard = base("die-hard");
    let locked_base = locked(&die_hard);
    let mut g = Gen {
        rng: Rng::new(seed),
    };
    let mut out = Differential::default();
    for index in 0..pairs {
        let before = g.formula(3, &mut Vec::new());
        let (op, after) = g.mutate(&before);
        out.pairs += 1;
        let (Ok(nb), Ok(na)) = (normalize(&before), normalize(&after)) else {
            out.normalize_rejected += 1;
            continue;
        };
        // The normalizer on its own: CPNF-1 must preserve meaning (RFC 0037 S1).
        for (authored, normal) in [(&before, &nb), (&after, &na)] {
            let t = truth(authored, normal, &all, &plain);
            if t.after_escapes || t.before_escapes {
                out.normal_form_unsound += 1;
                out.counterexamples.push(format!(
                    "pair {index}: normalize changed meaning: {authored:?} -> {normal:?}"
                ));
            }
        }
        let before_doc = with_not_solved(&locked_base, &before);
        let after_doc = with_not_solved(&locked_base, &after);
        let (Ok(before_contract), Ok(after_contract)) = (decode(&before_doc), decode(&after_doc))
        else {
            out.decode_rejected += 1;
            continue;
        };
        let artifact = diff(
            &before_contract,
            &after_contract,
            AssuranceLevel::Bounded,
            AcceptancePath::AgentAccept,
        )
        .expect("the assembler accepts a well-formed pair");
        let relation = artifact
            .intent_changes()
            .iter()
            .find(|r| r.field() == PolicyField::Properties && r.unit() == Some("NotSolved"))
            .map_or(Relation::Unchanged, |r| r.relation());
        let t = truth(&before, &after, &all, &plain);
        let class = match (t.after_escapes, t.before_escapes) {
            (false, false) => "equivalent",
            (true, false) => "strict-weakening",
            (false, true) => "strict-strengthening",
            (true, true) => "incomparable",
        };
        *out.relations.entry(relation.wire()).or_default() += 1;
        *out.truths.entry(class).or_default() += 1;
        if refutes(relation, t) {
            out.unsound += 1;
            out.counterexamples.push(format!(
                "pair {index}: classified {} but refuted: before {before:?} after {after:?}",
                relation.wire()
            ));
        }
        if t.after_escapes && artifact.decision() == PolicyDecision::Allow {
            out.silent += 1;
            out.counterexamples.push(format!(
                "pair {index}: SILENT PASS: before {before:?} after {after:?}"
            ));
        }
        if class == "strict-weakening" {
            out.strict_weakenings += 1;
            out.strict_weakenings_affirmed += usize::from(relation == Relation::Weakened);
        }
        let entry = out.per_mutation.entry(MUTATIONS[op]).or_default();
        entry.0 += 1;
        entry.1 += usize::from(t.after_escapes);

        // The assumption side: the same pair, the dangerous direction mirrored.
        let before_doc = with_jug_capacities(&locked_base, &before);
        let after_doc = with_jug_capacities(&locked_base, &after);
        let (Ok(before_contract), Ok(after_contract)) = (decode(&before_doc), decode(&after_doc))
        else {
            out.decode_rejected += 1;
            continue;
        };
        let artifact = diff(
            &before_contract,
            &after_contract,
            AssuranceLevel::Bounded,
            AcceptancePath::AgentAccept,
        )
        .expect("the assembler accepts a well-formed pair");
        let relation = artifact
            .intent_changes()
            .iter()
            .find(|r| r.field() == PolicyField::Assumptions && r.unit() == Some("JugCapacities"))
            .map_or(Relation::Unchanged, |r| r.relation());
        *out.assumption_relations.entry(relation.wire()).or_default() += 1;
        if refutes(relation, t) {
            out.assumption_unsound += 1;
            out.counterexamples.push(format!(
                "pair {index}: assumption classified {} but refuted: before {before:?} after {after:?}",
                relation.wire()
            ));
        }
        if t.before_escapes && artifact.decision() == PolicyDecision::Allow {
            out.assumption_silent += 1;
            out.counterexamples.push(format!(
                "pair {index}: SILENT PASS (assumption narrowed): before {before:?} after {after:?}"
            ));
        }
        if class == "strict-strengthening" {
            out.strict_strengthenings += 1;
            out.strict_strengthenings_affirmed += usize::from(relation == Relation::Strengthened);
        }
    }
    out
}

// --- part 3: the named adversarial assay -----------------------------------------------------

/// What a named attack must produce.
#[derive(Debug, Clone)]
enum Expect {
    /// Gaming: each listed field must carry a record outside its benign set, and the
    /// locked policy must not allow the revision.
    Gaming(Vec<(PolicyField, &'static [Relation])>),
    /// A meaning-preserving edit: allowed, no records.
    Control,
}

const WEAKEN_PROPERTY: &[Relation] =
    &[Relation::Unchanged, Relation::Strengthened, Relation::Added];
const STRENGTHEN_ASSUMPTION: &[Relation] =
    &[Relation::Unchanged, Relation::Weakened, Relation::Removed];
const SHRINK_BOUND: &[Relation] = &[Relation::Unchanged, Relation::Expanded];
const COARSEN_OBSERVER: &[Relation] = &[Relation::Unchanged, Relation::Refined, Relation::Added];
const REMOVE_FAULT: &[Relation] = &[Relation::Unchanged, Relation::Added];
const DOWNGRADE_ASSURANCE: &[Relation] = &[Relation::Unchanged, Relation::Upgraded];
const ANY_CHANGE: &[Relation] = &[Relation::Unchanged];

struct Attack {
    id: &'static str,
    family: &'static str,
    base: &'static str,
    what: &'static str,
    expect: Expect,
    after: Json,
}

fn big_ne_4() -> Formula {
    cmp(ComparisonOperator::Ne, state("big"), int(4))
}

fn big_eq_4() -> Formula {
    cmp(ComparisonOperator::Eq, state("big"), int(4))
}

/// The replicated register's `Agreement` claim, rebuilt as a formula. With
/// `swap`, the last disjunct compares `v1` with itself instead of with `v2`.
fn agreement(swap: bool) -> Formula {
    let var = |name: &str| Term::Var { name: ident(name) };
    let apply = |operator: &str, args: Vec<Term>| {
        Term::apply(ident(operator), args).expect("a well-formed application")
    };
    let chosen_at = || apply("get", vec![state("chosen"), var("v0")]);
    let binder = |variable: &str, domain: &str| {
        Binder::new(
            ident(variable),
            Term::Constant {
                name: ident(domain),
            },
        )
    };
    Formula::forall(
        binder("v0", "Epochs"),
        Formula::forall(
            binder("v1", "Values"),
            Formula::forall(
                binder("v2", "Values"),
                or(vec![
                    cmp(
                        ComparisonOperator::Ne,
                        chosen_at(),
                        apply("Some", vec![var("v1")]),
                    ),
                    cmp(
                        ComparisonOperator::Ne,
                        chosen_at(),
                        apply("Some", vec![var("v2")]),
                    ),
                    cmp(
                        ComparisonOperator::Eq,
                        var("v1"),
                        var(if swap { "v1" } else { "v2" }),
                    ),
                ]),
            ),
        ),
    )
}

fn type_ok_body() -> Vec<Formula> {
    vec![
        cmp(ComparisonOperator::Le, state("big"), int(5)),
        cmp(ComparisonOperator::Le, state("small"), int(3)),
        cmp(ComparisonOperator::Le, int(0), state("big")),
        cmp(ComparisonOperator::Le, int(0), state("small")),
    ]
}

#[allow(clippy::too_many_lines)]
fn attacks() -> Vec<Attack> {
    let dh = base("die-hard");
    let rr = base("replicated-register");
    let mut out = Vec::new();
    let mut push = |id, family, base_label: &'static str, what, expect, after| {
        out.push(Attack {
            id,
            family,
            base: base_label,
            what,
            expect,
            after,
        });
    };
    let weaken_not_solved = |doc: &Json, ast: Formula| with_not_solved(doc, &ast);
    let props = || Expect::Gaming(vec![(PolicyField::Properties, WEAKEN_PROPERTY)]);

    // ---- E: adversarial encodings, all within the Finite fragment ----
    push(
        "K09-E01",
        "encoding",
        "die-hard",
        "tautology by bi-implication: always(big != 4 or (p iff p))",
        props(),
        weaken_not_solved(
            &dh,
            Formula::always(or(vec![big_ne_4(), Formula::iff(pred("p"), pred("p"))])),
        ),
    );
    push(
        "K09-E02",
        "encoding",
        "die-hard",
        "complementary disjunct behind a double negation: always(big != 4 or not not (big == 4))",
        props(),
        weaken_not_solved(
            &dh,
            Formula::always(or(vec![big_ne_4(), Formula::not(Formula::not(big_eq_4()))])),
        ),
    );
    push(
        "K09-E03",
        "encoding",
        "die-hard",
        "vacuous response: eventually(big == 4) becomes leads_to(false, big == 4)",
        props(),
        edit(&dh, |map| {
            set_expression(
                map,
                "claims",
                "SolutionReachesFourGallons",
                &Formula::leads_to(Formula::boolean(false), big_eq_4()),
            );
        }),
    );
    push(
        "K09-E04",
        "encoding",
        "die-hard",
        "empty-domain vacuity: always(forall x in Empty. big != 4)",
        props(),
        weaken_not_solved(
            &dh,
            Formula::always(Formula::forall(
                Binder::new(
                    ident("x"),
                    Term::Constant {
                        name: ident("Empty"),
                    },
                ),
                big_ne_4(),
            )),
        ),
    );
    push(
        "K09-E05",
        "encoding",
        "die-hard",
        "temporal dual swap: always(big != 4) becomes eventually(big != 4)",
        props(),
        weaken_not_solved(&dh, Formula::eventually(big_ne_4())),
    );
    push(
        "K09-E06",
        "encoding",
        "die-hard",
        "recurrence in place of invariance: always(eventually(big != 4))",
        props(),
        weaken_not_solved(&dh, Formula::always(Formula::eventually(big_ne_4()))),
    );
    push(
        "K09-E07",
        "encoding",
        "die-hard",
        "guarded invariant: TypeOK becomes always(big == 4 implies TypeOK-body)",
        props(),
        edit(&dh, |map| {
            set_expression(
                map,
                "claims",
                "TypeOK",
                &Formula::always(Formula::implies(big_eq_4(), and(type_ok_body()))),
            );
        }),
    );
    push(
        "K09-E08",
        "encoding",
        "die-hard",
        "kind flip with the expression fixed: NotSolved safety becomes liveness",
        Expect::Gaming(vec![(PolicyField::Properties, ANY_CHANGE)]),
        edit(&dh, |map| {
            obj(unit_mut(map, "claims", "NotSolved")).insert("kind".to_owned(), s("liveness"));
        }),
    );
    push(
        "K09-E09",
        "encoding",
        "die-hard",
        "source lie: the AST is weakened and the display source is left saying big != 4",
        props(),
        weaken_not_solved(
            &dh,
            Formula::always(or(vec![
                big_ne_4(),
                cmp(ComparisonOperator::Le, state("small"), int(3)),
            ])),
        ),
    );
    push(
        "K09-E10",
        "encoding",
        "die-hard",
        "homoglyph rename plus weakening: NotSolved becomes NotSo1ved with a weaker body",
        props(),
        edit(&dh, |map| {
            set_expression(map, "claims", "NotSolved", &Formula::eventually(big_ne_4()));
            obj(unit_mut(map, "claims", "NotSolved")).insert("id".to_owned(), s("NotSo1ved"));
        }),
    );
    push(
        "K09-E11",
        "encoding",
        "die-hard",
        "shadow unit: a second NotSolved entry with a weaker body after the original",
        props(),
        edit(&dh, |map| {
            let mut shadow = unit_mut(map, "claims", "NotSolved").clone();
            let normal = normalize(&Formula::eventually(big_ne_4())).expect("normalizes");
            obj(obj(&mut shadow).get_mut("expression").expect("expression"))
                .insert("ast".to_owned(), normal.to_json());
            arr(map.get_mut("claims").expect("claims")).push(shadow);
        }),
    );
    push(
        "K09-E12",
        "encoding",
        "die-hard",
        "non-normal AST declared cpnf-1: gt(4, big) in place of lt(big, 4) inside a weakening",
        props(),
        edit(&dh, |map| {
            let unit = obj(unit_mut(map, "claims", "NotSolved"));
            let expression = obj(unit.get_mut("expression").expect("expression"));
            expression.insert(
                "ast".to_owned(),
                Formula::eventually(cmp(ComparisonOperator::Gt, int(4), state("big"))).to_json(),
            );
        }),
    );
    push(
        "K09-E13",
        "encoding",
        "replicated-register",
        "binder-reference swap: Agreement's v1 == v2 disjunct becomes v1 == v1 (always true)",
        props(),
        edit(&rr, |map| {
            set_expression(map, "claims", "Agreement", &agreement(true))
        }),
    );
    push(
        "K09-E14",
        "encoding",
        "replicated-register",
        "quantifier domain swap: Agreement's value binder ranges over Epochs",
        props(),
        edit(&rr, |map| {
            let unit = obj(unit_mut(map, "claims", "Agreement"));
            let expression = obj(unit.get_mut("expression").expect("expression"));
            let text = String::from_utf8(expression["ast"].to_canonical_bytes()).expect("utf8");
            let swapped = text.replacen(
                r#""domain":{"kind":"constant","name":"Values"}"#,
                r#""domain":{"kind":"constant","name":"Epochs"}"#,
                1,
            );
            assert_ne!(text, swapped, "the domain exists");
            expression.insert(
                "ast".to_owned(),
                Json::parse(swapped.as_bytes()).expect("parses"),
            );
        }),
    );
    push(
        "K09-E15",
        "encoding",
        "die-hard",
        "literal type confusion: big != \"4\" (a text literal is never equal to an integer)",
        props(),
        weaken_not_solved(
            &dh,
            Formula::always(cmp(
                ComparisonOperator::Ne,
                state("big"),
                Term::Literal {
                    value: Literal::Text("4".to_owned()),
                },
            )),
        ),
    );
    push(
        "K09-E16",
        "encoding",
        "die-hard",
        "control: De Morgan and implication respelling of NotSolved (meaning-preserving)",
        Expect::Control,
        weaken_not_solved(
            &dh,
            Formula::not(Formula::eventually(Formula::implies(
                Formula::not(big_eq_4()),
                Formula::boolean(false),
            ))),
        ),
    );

    // ---- X: cross-field compensation ----
    push(
        "K09-X01",
        "compensation",
        "die-hard",
        "weaken NotSolved and strengthen TypeOK in the same revision",
        props(),
        edit(&dh, |map| {
            set_expression(map, "claims", "NotSolved", &Formula::eventually(big_ne_4()));
            let mut body = type_ok_body();
            body.push(cmp(ComparisonOperator::Le, state("big"), int(4)));
            set_expression(map, "claims", "TypeOK", &Formula::always(and(body)));
        }),
    );
    push(
        "K09-X02",
        "compensation",
        "die-hard",
        "move a guarantee into an assumption: drop TypeOK, assume its body",
        Expect::Gaming(vec![
            (PolicyField::Properties, WEAKEN_PROPERTY),
            (PolicyField::Assumptions, STRENGTHEN_ASSUMPTION),
        ]),
        edit(&dh, |map| {
            let claims = arr(map.get_mut("claims").expect("claims"));
            let moved = claims.remove(2);
            let mut assumption = unit_mut(map, "assumptions", "JugCapacities").clone();
            let a = obj(&mut assumption);
            a.insert("id".to_owned(), s("TypeOKAssumed"));
            a.insert(
                "expression".to_owned(),
                moved.as_object().expect("claim")["expression"].clone(),
            );
            arr(map.get_mut("assumptions").expect("assumptions")).push(assumption);
        }),
    );
    push(
        "K09-X03",
        "compensation",
        "die-hard",
        "weaken NotSolved and shrink bounds.values in the same revision",
        Expect::Gaming(vec![
            (PolicyField::Properties, WEAKEN_PROPERTY),
            (PolicyField::Bounds, SHRINK_BOUND),
        ]),
        edit(&dh, |map| {
            set_expression(map, "claims", "NotSolved", &Formula::eventually(big_ne_4()));
            obj(map.get_mut("bounds").expect("bounds"))
                .insert("values".to_owned(), Json::Integer(4));
        }),
    );
    push(
        "K09-X04",
        "compensation",
        "die-hard",
        "bound trade: values 6 to 4 paid for by nodes 1 to 2",
        Expect::Gaming(vec![(PolicyField::Bounds, SHRINK_BOUND)]),
        edit(&dh, |map| {
            let bounds = obj(map.get_mut("bounds").expect("bounds"));
            bounds.insert("values".to_owned(), Json::Integer(4));
            bounds.insert("nodes".to_owned(), Json::Integer(2));
        }),
    );
    push(
        "K09-X05",
        "compensation",
        "replicated-register",
        "re-point RuntimeToAbstract at a new, coarser observer; the old one is untouched",
        Expect::Gaming(vec![(PolicyField::Properties, ANY_CHANGE)]),
        edit(&rr, |map| {
            let mut lite = unit_mut(map, "observers", "client").clone();
            let l = obj(&mut lite);
            l.insert("id".to_owned(), s("client_lite"));
            l.insert("events".to_owned(), Json::Array(Vec::new()));
            arr(map.get_mut("observers").expect("observers")).push(lite);
            obj(unit_mut(map, "claims", "RuntimeToAbstract"))
                .insert("observer".to_owned(), s("client_lite"));
        }),
    );
    push(
        "K09-X06",
        "compensation",
        "replicated-register",
        "coarsen the observer a claim is stated over; the claim text is untouched",
        Expect::Gaming(vec![(PolicyField::Observers, COARSEN_OBSERVER)]),
        edit(&rr, |map| {
            obj(unit_mut(map, "observers", "client"))
                .insert("events".to_owned(), Json::Array(Vec::new()));
        }),
    );
    push(
        "K09-X07",
        "compensation",
        "die-hard",
        "completion policy: stutter-forever becomes finite-trace-only (liveness can go vacuous)",
        Expect::Gaming(vec![(PolicyField::CompletionPolicy, ANY_CHANGE)]),
        edit(&dh, |map| {
            map.insert("completion_policy".to_owned(), s("finite-trace-only"));
        }),
    );
    push(
        "K09-X08",
        "compensation",
        "die-hard",
        "nondeterminism: the Next choice becomes angelic (the checker picks the good branch)",
        Expect::Gaming(vec![(PolicyField::Nondeterminism, ANY_CHANGE)]),
        edit(&dh, |map| {
            let site = &mut arr(map.get_mut("nondeterminism").expect("nd"))[0];
            obj(site).insert("class".to_owned(), s("angelic"));
        }),
    );
    push(
        "K09-X09",
        "compensation",
        "replicated-register",
        "trust boundary: mark the storage effect opaque",
        Expect::Gaming(vec![(PolicyField::TrustBoundaries, ANY_CHANGE)]),
        edit(&rr, |map| {
            let tb = obj(map.get_mut("trust_boundaries").expect("tb"));
            arr(tb.get_mut("opaque").expect("opaque")).push(s("storage/append-log"));
        }),
    );
    push(
        "K09-X10",
        "compensation",
        "replicated-register",
        "abstraction map rebound to a different map identity",
        Expect::Gaming(vec![(PolicyField::AbstractionMaps, ANY_CHANGE)]),
        edit(&rr, |map| {
            let maps = arr(map.get_mut("abstraction_maps").expect("maps"));
            obj(&mut maps[0]).insert("map_id".to_owned(), s("map_runtime_to_abstract_v2"));
        }),
    );
    push(
        "K09-X11",
        "compensation",
        "die-hard",
        "hard optimization constraint that excludes the failing state",
        Expect::Gaming(vec![(PolicyField::Optimization, ANY_CHANGE)]),
        edit(&dh, |map| {
            let opt = obj(map.get_mut("optimization").expect("opt"));
            arr(opt.get_mut("hard").expect("hard")).push(s("big != 4"));
        }),
    );
    push(
        "K09-X12",
        "compensation",
        "die-hard",
        "non-vacuity witness removed, so NotSolved may hold vacuously",
        Expect::Gaming(vec![(PolicyField::NonVacuity, ANY_CHANGE)]),
        edit(&dh, |map| {
            let opt = obj(map.get_mut("optimization").expect("opt"));
            opt.insert("non_vacuity".to_owned(), Json::Array(Vec::new()));
        }),
    );
    push(
        "K09-X13",
        "compensation",
        "replicated-register",
        "fault moved: crash class removed, a crash-only profile added",
        Expect::Gaming(vec![(PolicyField::Faults, REMOVE_FAULT)]),
        edit(&rr, |map| {
            let fm = obj(map.get_mut("fault_model").expect("faults"));
            arr(fm.get_mut("enabled").expect("enabled")).retain(|c| c.as_str() != Some("crash"));
            arr(fm.get_mut("profiles").expect("profiles")).push(s("process/crash-only-v0"));
        }),
    );
    push(
        "K09-X14",
        "compensation",
        "replicated-register",
        "assurance trade: minimum lowered, an evidence class added",
        Expect::Gaming(vec![(PolicyField::Assurance, DOWNGRADE_ASSURANCE)]),
        edit(&rr, |map| {
            let a = obj(map.get_mut("assurance").expect("assurance"));
            a.insert("minimum".to_owned(), s("bounded"));
            arr(a.get_mut("accepted_evidence_classes").expect("classes")).push(s("sampled"));
        }),
    );
    push(
        "K09-X15",
        "compensation",
        "replicated-register",
        "fairness strengthened in place: weak Recover becomes strong",
        Expect::Gaming(vec![(PolicyField::Fairness, ANY_CHANGE)]),
        edit(&rr, |map| {
            let f = &mut arr(map.get_mut("fairness").expect("fairness"))[0];
            obj(f).insert("kind".to_owned(), s("strong"));
        }),
    );
    push(
        "K09-X16",
        "compensation",
        "die-hard",
        "scope narrowed: the DieHard component removed from scope",
        Expect::Gaming(vec![(PolicyField::Scope, ANY_CHANGE)]),
        edit(&dh, |map| {
            let scope = obj(map.get_mut("scope").expect("scope"));
            scope.insert("components".to_owned(), Json::Array(Vec::new()));
        }),
    );
    push(
        "K09-X17",
        "compensation",
        "die-hard",
        "security policy: the revise-intent capability requirement dropped",
        Expect::Gaming(vec![(PolicyField::SecurityPolicy, ANY_CHANGE)]),
        edit(&dh, |map| {
            let sp = obj(map.get_mut("security_policy").expect("sp"));
            sp.insert(
                "capability_requirements".to_owned(),
                Json::Array(Vec::new()),
            );
        }),
    );

    // ---- F: fragment-boundary tricks ----
    let set_fragment =
        |map: &mut BTreeMap<String, Json>, group: &str, id: &str, fragment: Option<&str>| {
            let unit = obj(unit_mut(map, group, id));
            let expression = obj(unit.get_mut("expression").expect("expression"));
            match fragment {
                Some(f) => {
                    expression.insert("fragment".to_owned(), s(f));
                }
                None => {
                    expression.remove("fragment");
                }
            }
        };
    push(
        "K09-F01",
        "fragment",
        "die-hard",
        "weaken NotSolved and relabel it Temporal (outside the declared scope)",
        props(),
        edit(&dh, |map| {
            set_expression(map, "claims", "NotSolved", &Formula::eventually(big_ne_4()));
            set_fragment(map, "claims", "NotSolved", Some("Temporal"));
        }),
    );
    push(
        "K09-F02",
        "fragment",
        "die-hard",
        "weaken NotSolved and drop its fragment declaration (absent reads as the contract's)",
        props(),
        edit(&dh, |map| {
            set_expression(map, "claims", "NotSolved", &Formula::eventually(big_ne_4()));
            set_fragment(map, "claims", "NotSolved", None);
        }),
    );
    push(
        "K09-F03",
        "fragment",
        "die-hard",
        "declare Temporal in scope, move NotSolved there, and weaken it",
        props(),
        edit(&dh, |map| {
            set_expression(map, "claims", "NotSolved", &Formula::eventually(big_ne_4()));
            set_fragment(map, "claims", "NotSolved", Some("Temporal"));
            let scope = obj(map.get_mut("scope").expect("scope"));
            scope.insert(
                "fragments".to_owned(),
                Json::Array(vec![s("Finite"), s("Temporal")]),
            );
        }),
    );
    push(
        "K09-F04",
        "fragment",
        "die-hard",
        "swap the declared scope Finite to Symbolic, relabel every unit, weaken one",
        props(),
        edit(&dh, |map| {
            set_expression(map, "claims", "NotSolved", &Formula::eventually(big_ne_4()));
            for id in ["NotSolved", "SolutionReachesFourGallons", "TypeOK"] {
                set_fragment(map, "claims", id, Some("Symbolic"));
            }
            set_fragment(map, "assumptions", "JugCapacities", Some("Symbolic"));
            let scope = obj(map.get_mut("scope").expect("scope"));
            scope.insert("fragments".to_owned(), Json::Array(vec![s("Symbolic")]));
        }),
    );
    push(
        "K09-F05",
        "fragment",
        "die-hard",
        "strengthen the JugCapacities assumption and relabel it Symbolic",
        Expect::Gaming(vec![(PolicyField::Assumptions, STRENGTHEN_ASSUMPTION)]),
        edit(&dh, |map| {
            set_expression(
                map,
                "assumptions",
                "JugCapacities",
                &Formula::always(and(vec![
                    cmp(ComparisonOperator::Le, state("big"), int(3)),
                    cmp(ComparisonOperator::Le, state("small"), int(3)),
                ])),
            );
            set_fragment(map, "assumptions", "JugCapacities", Some("Symbolic"));
        }),
    );
    push(
        "K09-F06",
        "fragment",
        "die-hard",
        "control: a claim relabelled nothing, its fragment restated as Finite",
        Expect::Control,
        edit(&dh, |map| {
            set_fragment(map, "claims", "NotSolved", Some("Finite"))
        }),
    );

    out
}

/// A named attack's grade and the facts the ledger prints.
struct AssayRow {
    id: &'static str,
    family: &'static str,
    base: &'static str,
    what: &'static str,
    exposed: &'static str,
    records: String,
    locked: String,
    own: String,
}

fn records_text(records: &[Record]) -> String {
    if records.is_empty() {
        return "-".to_owned();
    }
    records
        .iter()
        .map(|r| match &r.unit {
            Some(unit) => format!("{}[{unit}]:{}", r.field.wire(), r.relation.wire()),
            None => format!("{}:{}", r.field.wire(), r.relation.wire()),
        })
        .collect::<Vec<_>>()
        .join(",")
}

fn assay() -> Vec<AssayRow> {
    attacks()
        .into_iter()
        .map(|attack| {
            let before = base(attack.base);
            let outcome = run(&before, &attack.after);
            let (exposed, records, locked, own) = match (&attack.expect, &outcome) {
                (_, Outcome::RejectedAtDecode) => ("rejected-at-decode", "-".to_owned(), "-", "-"),
                (_, Outcome::Refused(_)) => ("REFUSED", "-".to_owned(), "-", "-"),
                (
                    Expect::Gaming(targets),
                    Outcome::Assembled {
                        records,
                        locked,
                        locked_observed,
                        own,
                    },
                ) => {
                    let flagged = targets.iter().all(|(field, benign)| {
                        records
                            .iter()
                            .any(|r| r.field == *field && !benign.contains(&r.relation))
                    });
                    let silent = *locked == PolicyDecision::Allow
                        || *locked_observed == PolicyDecision::Allow;
                    let verdict = match (flagged, silent) {
                        (true, false) => "exposed",
                        (false, false) => "UNEXPOSED",
                        (_, true) => "SILENT-PASS",
                    };
                    (verdict, records_text(records), locked.wire(), own.wire())
                }
                (
                    Expect::Control,
                    Outcome::Assembled {
                        records,
                        locked,
                        own,
                        ..
                    },
                ) => {
                    let verdict = if records.is_empty() && *locked == PolicyDecision::Allow {
                        "control-allowed"
                    } else {
                        "control-flagged"
                    };
                    (verdict, records_text(records), locked.wire(), own.wire())
                }
            };
            AssayRow {
                id: attack.id,
                family: attack.family,
                base: attack.base,
                what: attack.what,
                exposed,
                records,
                locked: locked.to_owned(),
                own: own.to_owned(),
            }
        })
        .collect()
}

// --- part 3b: policy-table edits ------------------------------------------------------------

struct PolicyAttack {
    id: &'static str,
    what: &'static str,
    /// Decision under the before-table on the agent path.
    decision: PolicyDecision,
    records: String,
    verdict_text: String,
}

fn set_policy(document: &Json, field: &str, verb: &str) -> Json {
    edit(document, |map| {
        obj(map.get_mut("policy").expect("policy")).insert(field.to_owned(), s(verb));
    })
}

fn verdict_of(before: &Json, after: &Json) -> (PolicyDecision, Vec<Record>, BTreeSet<String>) {
    let before = decode(before).expect("the before-document decodes");
    let after = decode(after).expect("the after-document decodes");
    let artifact = diff(
        &before,
        &after,
        AssuranceLevel::Bounded,
        AcceptancePath::AgentAccept,
    )
    .expect("assembles");
    let reviewers: BTreeSet<String> = artifact
        .verdict()
        .reasons()
        .iter()
        .flat_map(|reason| reason.reviewers().iter().cloned())
        .collect();
    (artifact.decision(), records_of(&artifact), reviewers)
}

fn policy_attacks() -> Vec<PolicyAttack> {
    let rr = base("replicated-register");
    let weakened_agreement = |doc: &Json| {
        edit(doc, |map| {
            set_expression(
                map,
                "claims",
                "RuntimeToAbstract",
                &Formula::always(or(vec![
                    pred("stutters"),
                    Formula::action(
                        ident("Choose"),
                        continuum_intent::ast::ActionModality::Occurs,
                    ),
                    Formula::not(Formula::action(
                        ident("StableQuorumTransition"),
                        continuum_intent::ast::ActionModality::Occurs,
                    )),
                    pred("unobserved"),
                ])),
            );
        })
    };
    let mut out = Vec::new();
    let mut push =
        |id,
         what,
         (decision, records, reviewers): (PolicyDecision, Vec<Record>, BTreeSet<String>)| {
            out.push(PolicyAttack {
                id,
                what,
                decision,
                records: records_text(&records),
                verdict_text: if reviewers.is_empty() {
                    "-".to_owned()
                } else {
                    reviewers.into_iter().collect::<Vec<_>>().join(",")
                },
            });
        };

    // P01: one revision that unlocks `properties` and weakens a claim. P7 grades it
    // against the before-table, where properties is `locked`.
    let p01_after = set_policy(&weakened_agreement(&rr), "properties", "unlocked");
    push(
        "K09-P01",
        "self-unlock rider: properties unlocked and RuntimeToAbstract weakened in one revision",
        verdict_of(&rr, &p01_after),
    );
    // P02: the policy-only loosening, no protected field moved (DX02-B14's boundary).
    let p02_after = set_policy(&rr, "properties", "unlocked");
    push(
        "K09-P02",
        "policy-only loosening: properties locked to unlocked, nothing else",
        verdict_of(&rr, &p02_after),
    );
    // P03: the second step of the two-step attack, against the loosened table.
    push(
        "K09-P03",
        "two-step, step 2: RuntimeToAbstract weakened against the P02 table",
        verdict_of(&p02_after, &weakened_agreement(&p02_after)),
    );
    // P04: reviewer injection. The after-document names the agent as the only
    // `assumptions` reviewer while strengthening an assumption under `review`.
    let p04_after = edit(&rr, |map| {
        set_expression(
            map,
            "assumptions",
            "NetworkNoForgery",
            &Formula::always(and(vec![
                pred("was_sent"),
                Formula::not(Formula::action(
                    ident("Deliver"),
                    continuum_intent::ast::ActionModality::Occurs,
                )),
            ])),
        );
        obj(map.get_mut("policy_reviewers").expect("reviewers"))
            .insert("assumptions".to_owned(), Json::Array(vec![s("agent-self")]));
    });
    push(
        "K09-P04",
        "reviewer injection: agent-self named the assumptions reviewer while strengthening an assumption",
        verdict_of(&rr, &p04_after),
    );
    // P05: a protective-looking downgrade of the verb in the same revision as the
    // weakening it would admit (bounds `no-decrease` to `review`, values 2 to 1).
    let p05_after = edit(&set_policy(&rr, "bounds", "review"), |map| {
        obj(map.get_mut("bounds").expect("bounds")).insert("values".to_owned(), Json::Integer(1));
        obj(map.get_mut("policy_reviewers").expect("reviewers"))
            .insert("bounds".to_owned(), Json::Array(vec![s("agent-self")]));
    });
    push(
        "K09-P05",
        "verb swap rider: bounds no-decrease to review (agent-self reviewer) with values 2 to 1",
        verdict_of(&rr, &p05_after),
    );
    out
}

/// The dangerous relations of a field: the ones a gaming move in that field's
/// direction produces (the benign sets above, inverted over the field's row).
fn dangerous(field: PolicyField) -> Vec<Relation> {
    let benign: &[Relation] = match field {
        PolicyField::Properties => WEAKEN_PROPERTY,
        PolicyField::Assumptions => STRENGTHEN_ASSUMPTION,
        PolicyField::Bounds => SHRINK_BOUND,
        PolicyField::Observers => COARSEN_OBSERVER,
        PolicyField::Faults => REMOVE_FAULT,
        PolicyField::Assurance => DOWNGRADE_ASSURANCE,
        // RFC 0031 states a safe direction for three of the eight fields outside
        // the seven G3 dimensions; the other five have none, so every change counts.
        PolicyField::TrustBoundaries => &[Relation::Unchanged, Relation::Contracted],
        PolicyField::SecurityPolicy => {
            &[Relation::Unchanged, Relation::Strengthened, Relation::Added]
        }
        PolicyField::NonVacuity => &[Relation::Unchanged, Relation::Added],
        _ => ANY_CHANGE,
    };
    [
        Relation::Strengthened,
        Relation::Weakened,
        Relation::Expanded,
        Relation::Contracted,
        Relation::Refined,
        Relation::Coarsened,
        Relation::Merged,
        Relation::Split,
        Relation::Upgraded,
        Relation::Downgraded,
        Relation::Added,
        Relation::Removed,
        Relation::Incomparable,
        Relation::Unsupported,
        Relation::Unknown,
    ]
    .into_iter()
    .filter(|r| field.admits_relation(*r) && !benign.contains(r))
    .collect()
}

/// Every W6-admissible (field, verb) against every dangerous relation, on the agent
/// path: the (field, verb, relation) triples that close to `allow`.
fn verb_matrix() -> (usize, Vec<String>) {
    let reviewers = PolicyReviewers::new(
        PolicyField::ALL.map(|field| (field, BTreeSet::from(["owner".to_owned()]))),
    )
    .expect("well formed");
    let mut cells = 0;
    let mut allows = Vec::new();
    for field in PolicyField::ALL {
        for verb in field.admissible_verbs() {
            let table = PolicyTable::new(PolicyField::ALL.map(|f| {
                (
                    f,
                    if f == field {
                        *verb
                    } else {
                        PolicyVerb::Locked
                    },
                )
            }))
            .expect("admissible");
            for relation in dangerous(field) {
                cells += 1;
                let records: Vec<ClassificationRecord> = PolicyField::ALL
                    .map(|f| {
                        ClassificationRecord::new(
                            f,
                            if f == field {
                                relation
                            } else {
                                Relation::Unchanged
                            },
                        )
                        .expect("admissible")
                    })
                    .to_vec();
                let decision = table
                    .verdict(&records, &reviewers, AcceptancePath::AgentAccept)
                    .expect("a complete classification")
                    .decision();
                if decision == PolicyDecision::Allow {
                    allows.push(format!(
                        "{}:{}:{}",
                        field.wire(),
                        verb.wire(),
                        relation.wire()
                    ));
                }
            }
        }
    }
    (cells, allows)
}

// --- rendering -----------------------------------------------------------------------------

fn tally_line(label: &str, t: &Tally) -> String {
    format!(
        "{label} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {}",
        t.weakening,
        t.rejected,
        t.reached,
        t.caught,
        t.affirmed,
        t.caught - t.affirmed,
        t.unexposed,
        t.silent,
        t.refused,
        zero_failure_bound_ppm(t.reached),
    )
}

struct Measurement {
    indicator: Indicator,
    differential: Differential,
    assay: Vec<AssayRow>,
    policy: Vec<PolicyAttack>,
    matrix: (usize, Vec<String>),
}

/// One measurement shared by the tests that read it. The golden test still
/// measures a second time from scratch, which is the determinism check.
fn measured() -> &'static Measurement {
    static CELL: std::sync::OnceLock<Measurement> = std::sync::OnceLock::new();
    CELL.get_or_init(measure)
}

fn measure() -> Measurement {
    Measurement {
        indicator: indicator(),
        differential: differential(),
        assay: assay(),
        policy: policy_attacks(),
        matrix: verb_matrix(),
    }
}

impl Measurement {
    fn combined(&self) -> Tally {
        let mut t = self.indicator.primary;
        t.merge(&self.indicator.held_out);
        t
    }

    fn assay_count(&self, verdict: &str) -> usize {
        self.assay.iter().filter(|r| r.exposed == verdict).count()
    }

    /// The trend row's measured columns, in the trend file's order.
    fn trend_columns(&self) -> String {
        let t = self.combined();
        let d = &self.differential;
        let gaming = self
            .assay
            .iter()
            .filter(|r| !r.exposed.starts_with("control"))
            .count();
        format!(
            "{} | {} | {} | {} | {} | {} | {}/{} | {} | {}/{} | {}/{} | {}",
            t.reached,
            t.unexposed,
            t.silent,
            t.affirmed,
            t.caught - t.affirmed,
            t.own_allow,
            t.controls_allowed,
            t.controls,
            d.pairs,
            d.unsound + d.normal_form_unsound + d.assumption_unsound,
            d.silent + d.assumption_silent,
            self.assay_count("exposed") + self.assay_count("rejected-at-decode"),
            gaming,
            self.matrix.1.len(),
        )
    }
}

fn render(m: &Measurement) -> String {
    let mut out = String::new();
    let t = m.combined();
    let _ = writeln!(out, "{ARTIFACT_ID}");
    let _ = writeln!(
        out,
        "# KILL-09 (plan.md §24): intent diff cannot reliably expose gaming in supported fragments"
    );
    let _ = writeln!(
        out,
        "# crates/continuum-semantic-diff/tests/kill09_intent_diff_indicator.rs (bn-3kql)"
    );
    let _ = writeln!(
        out,
        "# bar: plan §5.3 / RFC 0031 completeness guarantee and G3 — every gaming move in a supported fragment is privileged; zero misses"
    );
    let _ = writeln!(
        out,
        "# This artifact measures. It does not decide (notes/plan/notes/KILL09_CHECKPOINT.md)."
    );

    let _ = writeln!(
        out,
        "## 1 leading indicator: exposure over the C020 hidden-mutant corpus"
    );
    let _ = writeln!(
        out,
        "# generator tests/support/c020_mutant_generator.rs; primary seed {C020_SEED:#018x} x {C020_VARIANTS}; held-out 16 seeds x {C020_HELD_OUT_VARIANTS}"
    );
    let _ = writeln!(
        out,
        "# exposed = every target field carries a record outside its benign set AND the locked policy does not allow, at bounded and at observed"
    );
    let _ = writeln!(
        out,
        "# columns: slice | weakening | rejected-at-decode | reached | exposed | affirmed | fail-closed | unexposed | silent | refused | 95% upper bound on miss rate (ppm, zero-failure)"
    );
    let _ = writeln!(out, "{}", tally_line("primary", &m.indicator.primary));
    let _ = writeln!(out, "{}", tally_line("held-out", &m.indicator.held_out));
    let _ = writeln!(out, "{}", tally_line("combined", &t));
    for dim in DIMENSIONS {
        let tally = m
            .indicator
            .per_dimension
            .get(&dim)
            .copied()
            .unwrap_or_default();
        let _ = writeln!(out, "{}", tally_line(&format!("dim:{dim:?}"), &tally));
    }
    let _ = writeln!(
        out,
        "controls (meaning-preserving) allowed: {} of {}",
        t.controls_allowed, t.controls
    );
    let _ = writeln!(
        out,
        "own committed policy: {} exposed weakening mutants allowed by an owner verb",
        t.own_allow
    );
    let _ = writeln!(
        out,
        "exposure-miss rate {}/{}; silent-pass rate {}/{}; fail-closed share {}/{}",
        t.unexposed + t.refused,
        t.reached,
        t.silent,
        t.reached,
        t.caught - t.affirmed,
        t.caught
    );

    let d = &m.differential;
    let _ = writeln!(out, "## 2 semantic differential falsifier");
    let _ = writeln!(
        out,
        "# seed {DIFFERENTIAL_SEED:#018x}; {DIFFERENTIAL_PAIRS} authored pairs (random Finite formula, one agent-style edit); ground truth = direct evaluation over 528 lasso traces x 9 domain interpretations"
    );
    let _ = writeln!(
        out,
        "pairs {} | normalize-rejected {} | decode-rejected {} | normal-form meaning changes {} | unsound classifications {} | silent passes {}",
        d.pairs,
        d.normalize_rejected,
        d.decode_rejected,
        d.normal_form_unsound,
        d.unsound,
        d.silent
    );
    let _ = writeln!(
        out,
        "classifier relations: {}",
        d.relations
            .iter()
            .map(|(k, v)| format!("{k} {v}"))
            .collect::<Vec<_>>()
            .join(", ")
    );
    let _ = writeln!(
        out,
        "ground truth: {}",
        d.truths
            .iter()
            .map(|(k, v)| format!("{k} {v}"))
            .collect::<Vec<_>>()
            .join(", ")
    );
    let _ = writeln!(
        out,
        "strict weakenings affirmed `weakened`: {} of {} (the rest fail closed)",
        d.strict_weakenings_affirmed, d.strict_weakenings
    );
    let _ = writeln!(
        out,
        "assumption side (same pairs on JugCapacities): relations {}; unsound {}; silent passes (narrowed and allowed) {}; strict strengthenings affirmed `strengthened` {} of {}",
        d.assumption_relations
            .iter()
            .map(|(k, v)| format!("{k} {v}"))
            .collect::<Vec<_>>()
            .join(", "),
        d.assumption_unsound,
        d.assumption_silent,
        d.strict_strengthenings_affirmed,
        d.strict_strengthenings
    );
    let _ = writeln!(
        out,
        "# per edit: edit | pairs | pairs that admit a new behavior"
    );
    for name in MUTATIONS {
        let (n, escapes) = d.per_mutation.get(name).copied().unwrap_or_default();
        let _ = writeln!(out, "{name} | {n} | {escapes}");
    }
    for line in &d.counterexamples {
        let _ = writeln!(out, "COUNTEREXAMPLE {line}");
    }

    let _ = writeln!(out, "## 3 named adversarial assay");
    let _ = writeln!(
        out,
        "# columns: id | family | base | verdict | records (locked run, bounded) | locked | own | attack"
    );
    for row in &m.assay {
        let _ = writeln!(
            out,
            "{} | {} | {} | {} | {} | {} | {} | {}",
            row.id, row.family, row.base, row.exposed, row.records, row.locked, row.own, row.what
        );
    }
    let _ = writeln!(
        out,
        "## 3b policy-table edits (verdict against the before-table, agent path)"
    );
    let _ = writeln!(
        out,
        "# columns: id | decision | records | reviewers named | attack"
    );
    for p in &m.policy {
        let _ = writeln!(
            out,
            "{} | {} | {} | {} | {}",
            p.id,
            p.decision.wire(),
            p.records,
            p.verdict_text,
            p.what
        );
    }
    let _ = writeln!(out, "## 4 policy verb matrix (agent path)");
    let _ = writeln!(
        out,
        "cells {} (W6-admissible field x verb x dangerous relation); allow cells {}",
        m.matrix.0,
        m.matrix.1.len()
    );
    let g3: Vec<&str> = DIMENSIONS.iter().map(|d| policy_field(*d).wire()).collect();
    for cell in &m.matrix.1 {
        let field = cell.split(':').next().unwrap_or_default();
        let note = if cell.contains(":unlocked:") {
            "owner-declared (unlocked)"
        } else if g3.contains(&field) {
            "GAP (protective verb admits a G3 gaming relation)"
        } else {
            "latent (no classifier for this field today; the assembler emits only unknown, P2 gives review)"
        };
        let _ = writeln!(out, "allow {cell} | {note}");
    }
    let _ = writeln!(out, "## trend columns");
    let _ = writeln!(out, "{}", m.trend_columns());
    out
}

// --- tests -----------------------------------------------------------------------------------

#[test]
fn the_indicator_artifact_is_byte_stable_and_matches_the_golden() {
    let first = render(measured());
    let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join("kill09_intent_diff_indicator.txt");
    std::fs::write(&out, &first).expect("the rendered artifact is writable");
    assert!(
        first == GOLDEN,
        "the KILL-09 indicator drifted; the rendered artifact is at {}",
        out.display()
    );
    assert_eq!(
        first,
        render(&measure()),
        "INV-005: the artifact is deterministic"
    );
}

#[test]
fn the_trend_series_ends_at_this_measurement() {
    // The last data row of the trend file must be this revision's figures. A
    // changed figure fails here until a new, named row is appended: the trend is
    // retained, never overwritten.
    let rows: Vec<&str> = TREND
        .lines()
        .filter(|line| !line.starts_with('#') && !line.trim().is_empty())
        .collect();
    assert!(rows.len() >= 2, "a trend has at least two points");
    let last = rows.last().expect("a last row");
    let columns: Vec<&str> = last.splitn(4, " | ").collect();
    assert_eq!(
        columns.len(),
        4,
        "point | bone | commit | measured columns: {last}"
    );
    assert_eq!(columns[1], "bn-3kql");
    assert_eq!(
        columns[3],
        measured().trend_columns(),
        "append a trend row for the new measurement"
    );
}

#[test]
fn zero_silent_passes_and_zero_exposure_misses_in_supported_fragments() {
    let m = measured();
    let t = m.combined();
    assert!(
        t.reached >= 3000,
        "the corpus is not trivially small: {}",
        t.reached
    );
    assert_eq!(t.silent, 0, "a silent pass is an INV-001 defect");
    assert_eq!(
        t.unexposed + t.refused,
        0,
        "every miss is a candidate INV-001 defect"
    );
    assert_eq!(t.controls_allowed, t.controls, "the metric discriminates");
    let d = &m.differential;
    assert_eq!(
        d.unsound + d.normal_form_unsound + d.silent + d.assumption_unsound + d.assumption_silent,
        0,
        "{}",
        d.counterexamples.join("\n")
    );
    assert!(
        d.strict_weakenings >= 300,
        "the falsifier exercises weakenings"
    );
    for row in &m.assay {
        assert!(
            matches!(
                row.exposed,
                "exposed" | "rejected-at-decode" | "control-allowed"
            ),
            "{} {}: {} ({})",
            row.id,
            row.what,
            row.exposed,
            row.records
        );
    }
}

#[test]
fn policy_table_edits_cannot_lift_their_own_block() {
    let policy = policy_attacks();
    let by_id = |id: &str| policy.iter().find(|p| p.id == id).expect("present");
    assert_eq!(
        by_id("K09-P01").decision,
        PolicyDecision::Block,
        "P7: the before-table"
    );
    // P02 is DX02-B14's recorded boundary: the policy table is not one of the
    // fifteen diff fields, so a governance-only revision carries no record.
    assert_eq!(by_id("K09-P02").decision, PolicyDecision::Allow);
    assert_eq!(by_id("K09-P02").records, "-");
    // Its second step is exposed (a record exists) and allowed only because the
    // loosened table says `unlocked`.
    assert!(by_id("K09-P03").records.contains("properties"));
    // Reviewer injection: the before-document's reviewers are named, never the
    // after-document's.
    assert_ne!(by_id("K09-P04").decision, PolicyDecision::Allow);
    assert!(!by_id("K09-P04").verdict_text.contains("agent-self"));
    assert_eq!(by_id("K09-P05").decision, PolicyDecision::Block);
    // On the seven G3 dimensions, no verb other than `unlocked` admits a dangerous
    // relation on the agent path.
    let (cells, allows) = verb_matrix();
    assert!(cells > 100);
    let g3: Vec<&str> = DIMENSIONS.iter().map(|d| policy_field(*d).wire()).collect();
    let mut latent = Vec::new();
    for cell in &allows {
        let field = cell.split(':').next().expect("a field");
        if g3.contains(&field) {
            assert!(cell.contains(":unlocked:"), "{cell}");
        } else if !cell.contains(":unlocked:") {
            latent.push(cell.as_str());
        }
    }
    // Outside the seven, two cells admit a direction under a protective verb. Both
    // are latent: the assembler has no classifier for either field and emits only
    // `unknown` there, which P2 turns into `review` (assay rows K09-X11, K09-X17).
    // A new cell here, or a classifier landing for either field, must be seen.
    assert_eq!(
        latent,
        [
            "optimization:no-removal:added",
            "security_policy:no-removal:weakened"
        ]
    );
}

#[test]
fn the_indicator_agrees_with_the_c020_ledger() {
    // The indicator re-runs C020's corpus through its own harness. The totals must
    // equal C020's retained summary lines, so the two artifacts cannot drift apart.
    let m = indicator();
    let p = m.primary;
    let summary = format!(
        "weakening: caught {} (affirmed direction {}, fail-closed {}), rejected-at-decode {}, misses {}, silent passes {}",
        p.caught,
        p.affirmed,
        p.caught - p.affirmed,
        p.rejected,
        p.unexposed + p.silent + p.refused,
        p.silent
    );
    assert!(C020_GOLDEN.contains(&summary), "{summary}");
    let h = m.held_out;
    let held = format!(
        "weakening {}, caught {} (affirmed {}), rejected-at-decode {}, misses {}, silent passes {}; controls allowed {} of {}",
        h.weakening,
        h.caught,
        h.affirmed,
        h.rejected,
        h.unexposed + h.silent + h.refused,
        h.silent,
        h.controls_allowed,
        h.controls
    );
    assert!(C020_GOLDEN.contains(&held), "{held}");
    assert!(C020_GOLDEN.contains(&format!(
        "own committed policy: {} weakening mutants allowed",
        p.own_allow
    )));
}

#[test]
fn negative_the_ground_truth_refutes_known_unsound_claims() {
    // The falsifier is evidence only if it can refute. Hand it three wrong
    // classifications and require a counterexample for each.
    let all = worlds(true);
    let plain = worlds(false);
    let p = pred("p");
    let e = cmp(ComparisonOperator::Eq, state("s"), int(1));
    // 1. `always p` weakened to `eventually p`: after escapes.
    let t = truth(
        &Formula::always(p.clone()),
        &Formula::eventually(p.clone()),
        &all,
        &plain,
    );
    assert!(t.after_escapes && !t.before_escapes);
    // 2. `forall x in D. p` against `p`: the empty domain makes the quantified form
    //    true where `p` is false, so they are not equivalent.
    let vacuous = Formula::forall(
        Binder::new(ident("x"), Term::Constant { name: ident("D") }),
        p.clone(),
    );
    let t = truth(&p, &vacuous, &all, &plain);
    assert!(t.after_escapes, "the empty-domain world is present");
    // 3. The recurrence/persistence pair is distinguished by a two-state loop.
    let ae = Formula::always(Formula::eventually(e.clone()));
    let ea = Formula::eventually(Formula::always(e));
    let t = truth(&ea, &ae, &all, &plain);
    assert!(t.after_escapes && !t.before_escapes);
    // 4. Shadowing: `forall x in D. exists x in C. r(x)` binds the inner x.
    let r = |v: &str| Formula::predicate(ident("r"), vec![Term::Var { name: ident(v) }]);
    let inner = Formula::forall(
        Binder::new(ident("x"), Term::Constant { name: ident("D") }),
        Formula::exists(
            Binder::new(ident("x"), Term::Constant { name: ident("C") }),
            r("x"),
        ),
    );
    let outer = Formula::forall(
        Binder::new(ident("x"), Term::Constant { name: ident("D") }),
        Formula::exists(
            Binder::new(ident("y"), Term::Constant { name: ident("C") }),
            r("x"),
        ),
    );
    let t = truth(&inner, &outer, &all, &plain);
    assert!(
        t.after_escapes || t.before_escapes,
        "capture is a meaning change"
    );
}

/// The extended falsification lane: eight further seeds, 24,000 pairs. Not in the
/// gate (it is minutes in a debug build); run it with
/// `cargo test --release -p continuum-semantic-diff --test kill09_intent_diff_indicator -- --ignored --nocapture`.
/// The seeds are in the source, so a finding reproduces from the seed alone.
#[test]
#[ignore = "extended lane; see the doc comment"]
fn extended_differential_sweep() {
    let mut totals = (0, 0, 0, 0, 0);
    for k in 0..8u64 {
        let d = differential_sweep(0x4B09_D1FF_0000_0100 + k, 3000);
        assert_eq!(
            d.unsound
                + d.normal_form_unsound
                + d.silent
                + d.assumption_unsound
                + d.assumption_silent,
            0,
            "{}",
            d.counterexamples.join("\n")
        );
        totals.0 += d.pairs;
        totals.1 += d.strict_weakenings;
        totals.2 += d.strict_weakenings_affirmed;
        totals.3 += d.strict_strengthenings;
        totals.4 += d.strict_strengthenings_affirmed;
    }
    println!(
        "extended differential: pairs {}, strict weakenings {} (affirmed {}), strict strengthenings {} (affirmed {}), unsound 0, silent 0",
        totals.0, totals.1, totals.2, totals.3, totals.4
    );
}

#[test]
fn the_rebuilt_agreement_claim_is_the_committed_one() {
    // K09-E13 is only an attack on Agreement if `agreement(false)` is Agreement.
    let rr = base("replicated-register");
    let rebuilt = edit(&rr, |map| {
        set_expression(map, "claims", "Agreement", &agreement(false))
    });
    assert_eq!(rebuilt.to_canonical_bytes(), rr.to_canonical_bytes());
}

#[test]
fn negative_the_assay_grader_flags_a_blind_diff() {
    // An attack graded against an empty record set and an `allow` must not read as
    // exposed.
    let outcome = Outcome::Assembled {
        records: Vec::new(),
        locked: PolicyDecision::Allow,
        locked_observed: PolicyDecision::Allow,
        own: PolicyDecision::Allow,
    };
    let weakening = Expectation::Weakening(vec![generator::Target {
        field: Field::Properties,
        direction: Direction::WeakensProperty,
    }]);
    assert_eq!(grade(&weakening, &outcome), Grade::Silent);
    let blocked_but_blind = Outcome::Assembled {
        records: Vec::new(),
        locked: PolicyDecision::Block,
        locked_observed: PolicyDecision::Block,
        own: PolicyDecision::Block,
    };
    assert_eq!(grade(&weakening, &blocked_but_blind), Grade::Unexposed);
}
