//! C020 hidden-mutant generator (bn-bpz0).
//!
//! A seeded generator of agent-style *weakening* edits over real Intent Contract
//! documents. It is written apart from the classifier on purpose, and this file
//! must stay that way:
//!
//! - It reads and writes only generic JSON (`continuum_intent::canonical_json::Json`,
//!   the parser, not the classifier). It does not name the semantic-diff crate, the
//!   relation vocabulary, the policy table, or any verdict. A source guard in
//!   `c020_hidden_mutant_evaluation.rs` pins that.
//! - Its ground truth is *by construction*. Each operator states the semantic
//!   direction its edit has in plan §5.3's seven G3 dimensions (a property made to
//!   admit more behaviors, an assumption made to admit fewer environments, a bound
//!   made smaller, an observer made coarser, a fault class removed, fairness added or
//!   made stronger, assurance made weaker). It never asks the classifier what the
//!   direction is.
//! - The polarity rule is the only semantics here: a subformula under an odd number
//!   of negations (a `not`, or an `implies`/`leads_to` antecedent) is in negative
//!   position, and loosening it means tightening the node. `iff` operands are mixed
//!   and are never a mutation site.
//!
//! Randomness is a local SplitMix64 stream seeded from the caller's seed, the base
//! index, the operator index, and the variant index. No ambient entropy (INV-005).

use std::collections::BTreeMap;

use continuum_intent::canonical_json::Json;

// --- deterministic PRNG --------------------------------------------------------------------

/// SplitMix64 (Steele, Lea, Flood 2014). Small, seedable, and good enough to spread
/// choices; nothing here is cryptographic.
#[derive(Debug, Clone)]
pub struct Rng(u64);

impl Rng {
    /// A stream from one seed.
    pub const fn new(seed: u64) -> Self {
        Self(seed)
    }

    /// The next 64 bits.
    pub fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// A value in `0..n`. `n` must be non-zero.
    pub fn below(&mut self, n: usize) -> usize {
        let n64 = u64::try_from(n).unwrap_or(u64::MAX);
        usize::try_from(self.next_u64() % n64).unwrap_or(0)
    }

    /// A coin flip.
    pub fn coin(&mut self) -> bool {
        self.next_u64() & 1 == 1
    }

    /// Fisher-Yates.
    pub fn shuffle<T>(&mut self, items: &mut [T]) {
        for i in (1..items.len()).rev() {
            let j = self.below(i + 1);
            items.swap(i, j);
        }
    }
}

fn mix(seed: u64, a: u64, b: u64, c: u64) -> u64 {
    let mut rng = Rng::new(seed ^ a.rotate_left(17) ^ b.rotate_left(33) ^ c.rotate_left(49));
    rng.next_u64()
}

// --- ground-truth vocabulary (the generator's own, not the classifier's) -------------------

/// The protected field group an edit targets, in the generator's own words.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Field {
    /// `claims[]`.
    Properties,
    /// `assumptions[]`.
    Assumptions,
    /// `bounds`.
    Bounds,
    /// `observers[]`.
    Observers,
    /// `fault_model`.
    Faults,
    /// `fairness[]`.
    Fairness,
    /// `assurance`.
    Assurance,
}

/// The semantic direction an edit has by construction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Direction {
    /// A claim now admits at least the behaviors it admitted before (property
    /// weakening), or a claim was removed.
    WeakensProperty,
    /// An assumption now admits at most the environments it admitted before, or an
    /// assumption was added.
    StrengthensAssumption,
    /// A bound component moved down (possibly with another moving up).
    ShrinksBound,
    /// An observer lost an event or a projection element (possibly while gaining an
    /// unrelated one).
    CoarsensObserver,
    /// A fault class or profile was removed (possibly while another was added).
    RemovesFault,
    /// Fairness was added or made stronger.
    StrengthensFairness,
    /// Fairness was removed. Plan §5.3 names "fairness addition or removal" as
    /// privileged, so any non-`unchanged` classification is required.
    ChangesFairness,
    /// The assurance requirement moved down (possibly while another part moved up).
    DowngradesAssurance,
    /// The unordered accepted-evidence-class set changed. RFC 0031 forbids ranking
    /// it, so any non-`unchanged` classification is required.
    ChangesAssurance,
}

/// One field the edit moves, and the direction it moves it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Target {
    /// The field group.
    pub field: Field,
    /// The direction by construction.
    pub direction: Direction,
}

/// What a mutant is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Expectation {
    /// A weakening edit: every target must be classified privileged and the
    /// revision must not be allowed under a locked policy.
    Weakening(Vec<Target>),
    /// A meaning-preserving edit (the precision control).
    Control,
}

/// One generated revision of one base document.
#[derive(Debug, Clone)]
pub struct Mutant {
    /// Stable identity: `C020-<base>-<operator>-<variant>`.
    pub id: String,
    /// The base document's label.
    pub base: String,
    /// The operator name.
    pub operator: &'static str,
    /// The ground truth by construction.
    pub expectation: Expectation,
    /// The after-document.
    pub document: Json,
}

/// A base document.
#[derive(Debug, Clone)]
pub struct Base {
    /// A short label.
    pub label: String,
    /// The committed document.
    pub document: Json,
}

// --- JSON navigation -----------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
enum Step {
    Key(&'static str),
    Index(usize),
}

fn at_mut<'a>(json: &'a mut Json, path: &[Step]) -> Option<&'a mut Json> {
    let mut node = json;
    for step in path {
        node = match (step, node) {
            (Step::Key(key), Json::Object(map)) => map.get_mut(*key)?,
            (Step::Index(i), Json::Array(items)) => items.get_mut(*i)?,
            _ => return None,
        };
    }
    Some(node)
}

fn at<'a>(json: &'a Json, path: &[Step]) -> Option<&'a Json> {
    let mut node = json;
    for step in path {
        node = match (step, node) {
            (Step::Key(key), Json::Object(map)) => map.get(*key)?,
            (Step::Index(i), Json::Array(items)) => items.get(*i)?,
            _ => return None,
        };
    }
    Some(node)
}

fn get<'a>(json: &'a Json, key: &str) -> Option<&'a Json> {
    match json {
        Json::Object(map) => map.get(key),
        _ => None,
    }
}

fn obj_mut<'a>(json: &'a mut Json, key: &str) -> Option<&'a mut Json> {
    match json {
        Json::Object(map) => map.get_mut(key),
        _ => None,
    }
}

fn array_len(json: &Json, key: &'static str) -> usize {
    match get(json, key) {
        Some(Json::Array(items)) => items.len(),
        _ => 0,
    }
}

fn kind_of(json: &Json) -> Option<&str> {
    get(json, "kind").and_then(Json::as_str)
}

fn object(fields: Vec<(&str, Json)>) -> Json {
    let mut map = BTreeMap::new();
    for (key, value) in fields {
        map.insert(key.to_owned(), value);
    }
    Json::Object(map)
}

fn string(text: &str) -> Json {
    Json::String(text.to_owned())
}

fn fresh_name(rng: &mut Rng, stem: &str) -> String {
    format!("c020_{stem}_{:08x}", rng.next_u64() & 0xFFFF_FFFF)
}

fn fresh_predicate(rng: &mut Rng) -> Json {
    object(vec![
        ("args", Json::Array(Vec::new())),
        ("kind", string("predicate")),
        ("name", Json::String(fresh_name(rng, "p"))),
    ])
}

fn junction(kind: &str, operands: Vec<Json>) -> Json {
    object(vec![
        ("kind", string(kind)),
        ("operands", Json::Array(operands)),
    ])
}

fn negation(operand: Json) -> Json {
    object(vec![("kind", string("not")), ("operand", operand)])
}

fn boolean(value: bool) -> Json {
    object(vec![
        ("kind", string("boolean")),
        ("value", Json::Bool(value)),
    ])
}

// --- formula sites and polarity --------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Polarity {
    Positive,
    Negative,
}

impl Polarity {
    const fn flip(self) -> Self {
        match self {
            Self::Positive => Self::Negative,
            Self::Negative => Self::Positive,
        }
    }
}

const FORMULA_KINDS: [&str; 14] = [
    "boolean",
    "predicate",
    "action",
    "compare",
    "not",
    "and",
    "or",
    "implies",
    "leads_to",
    "iff",
    "always",
    "eventually",
    "forall",
    "exists",
];

#[derive(Debug, Clone)]
struct Site {
    path: Vec<Step>,
    polarity: Polarity,
    kind: String,
}

fn collect_sites(node: &Json, path: &mut Vec<Step>, polarity: Polarity, out: &mut Vec<Site>) {
    let Some(kind) = kind_of(node) else { return };
    if !FORMULA_KINDS.contains(&kind) {
        return;
    }
    out.push(Site {
        path: path.clone(),
        polarity,
        kind: kind.to_owned(),
    });
    let children: Vec<(Vec<Step>, Polarity)> = match kind {
        "not" => vec![(vec![Step::Key("operand")], polarity.flip())],
        "and" | "or" => (0..array_len(node, "operands"))
            .map(|i| (vec![Step::Key("operands"), Step::Index(i)], polarity))
            .collect(),
        "implies" | "leads_to" => vec![
            (vec![Step::Key("antecedent")], polarity.flip()),
            (vec![Step::Key("consequent")], polarity),
        ],
        "always" | "eventually" => vec![(vec![Step::Key("operand")], polarity)],
        "forall" | "exists" => vec![(vec![Step::Key("body")], polarity)],
        // `iff` is mixed polarity; leaves have no formula children.
        _ => Vec::new(),
    };
    for (steps, child_polarity) in children {
        if let Some(child) = at(node, &steps) {
            let len = path.len();
            path.extend(steps.iter().cloned());
            collect_sites(child, path, child_polarity, out);
            path.truncate(len);
        }
    }
}

/// The edit applied at one formula site, in the *loosening* sense: the node after
/// the edit admits at least the behaviors it admitted before, read at the site's
/// effective polarity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Loosen {
    /// `ψ ∨ fresh` (positive) / `ψ ∧ fresh` (negative).
    JoinFresh,
    /// `true` (positive) / `false` (negative).
    Trivialize,
    /// `ψ ∨ ¬ψ` (positive) / `ψ ∧ ¬ψ` (negative): the plan §5.1 move, generalized.
    Complement,
    /// Drop one operand of an `and` (positive) / an `or` (negative).
    DropJunct,
    /// `always` → `eventually` (positive) / `eventually` → `always` (negative).
    TemporalSwap,
}

fn eligible(site: &Site, edit: Loosen, node: &Json) -> bool {
    match edit {
        Loosen::JoinFresh | Loosen::Trivialize | Loosen::Complement => true,
        Loosen::DropJunct => {
            let wanted = match site.polarity {
                Polarity::Positive => "and",
                Polarity::Negative => "or",
            };
            site.kind == wanted && array_len(node, "operands") >= 2
        }
        Loosen::TemporalSwap => matches!(
            (site.polarity, site.kind.as_str()),
            (Polarity::Positive, "always") | (Polarity::Negative, "eventually")
        ),
    }
}

fn loosen_node(node: &Json, polarity: Polarity, edit: Loosen, rng: &mut Rng) -> Json {
    let (join, trivial) = match polarity {
        Polarity::Positive => ("or", true),
        Polarity::Negative => ("and", false),
    };
    match edit {
        Loosen::JoinFresh => junction(join, vec![node.clone(), fresh_predicate(rng)]),
        Loosen::Trivialize => boolean(trivial),
        Loosen::Complement => junction(join, vec![node.clone(), negation(node.clone())]),
        Loosen::DropJunct => {
            let mut operands = match get(node, "operands") {
                Some(Json::Array(items)) => items.clone(),
                _ => Vec::new(),
            };
            let drop = rng.below(operands.len());
            operands.remove(drop);
            if operands.len() == 1 {
                operands.remove(0)
            } else {
                let kind = kind_of(node).unwrap_or("and").to_owned();
                junction(&kind, operands)
            }
        }
        Loosen::TemporalSwap => {
            let mut swapped = node.clone();
            let to = if polarity == Polarity::Positive {
                "eventually"
            } else {
                "always"
            };
            if let Some(kind) = obj_mut(&mut swapped, "kind") {
                *kind = string(to);
            }
            swapped
        }
    }
}

/// Which way an expression edit must move its unit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Sense {
    /// A claim: loosen at the site's own polarity.
    Loosen,
    /// An assumption: tighten, which is loosening at the flipped polarity.
    Tighten,
}

/// Apply one expression edit to a random eligible site of a random unit of
/// `group`. Returns the unit index and the path (from the document root) of the
/// replaced node.
fn edit_expression(
    doc: &mut Json,
    group: &'static str,
    sense: Sense,
    edit: Loosen,
    rng: &mut Rng,
) -> Option<(usize, Vec<Step>)> {
    let units = array_len(doc, group);
    if units == 0 {
        return None;
    }
    let start = rng.below(units);
    for offset in 0..units {
        let unit = (start + offset) % units;
        let ast_path = vec![
            Step::Key(group),
            Step::Index(unit),
            Step::Key("expression"),
            Step::Key("ast"),
        ];
        let ast = at(doc, &ast_path)?.clone();
        let mut sites = Vec::new();
        collect_sites(&ast, &mut Vec::new(), Polarity::Positive, &mut sites);
        let candidates: Vec<Site> = sites
            .into_iter()
            .map(|mut site| {
                if sense == Sense::Tighten {
                    site.polarity = site.polarity.flip();
                }
                site
            })
            .filter(|site| at(&ast, &site.path).is_some_and(|node| eligible(site, edit, node)))
            .collect();
        if candidates.is_empty() {
            continue;
        }
        let site = &candidates[rng.below(candidates.len())];
        let node = at(&ast, &site.path)?.clone();
        let replacement = loosen_node(&node, site.polarity, edit, rng);
        let mut full = ast_path.clone();
        full.extend(site.path.iter().cloned());
        *at_mut(doc, &full)? = replacement;
        drop_normal_form(doc, group, unit);
        return Some((unit, full));
    }
    None
}

/// The generator writes authoring forms, so it withdraws the `cpnf-1` declaration of
/// every expression it touches. `normal_form` is display-only and outside the
/// identity (RFC 0037 ID2); an agent may do the same.
fn drop_normal_form(doc: &mut Json, group: &'static str, unit: usize) {
    if let Some(Json::Object(expr)) = at_mut(
        doc,
        &[Step::Key(group), Step::Index(unit), Step::Key("expression")],
    ) {
        expr.remove("normal_form");
    }
}

fn permute_operands(node: &mut Json, rng: &mut Rng) {
    match node {
        Json::Object(map) => {
            let is_junction = matches!(map.get("kind").and_then(Json::as_str), Some("and" | "or"));
            for (key, value) in map.iter_mut() {
                if is_junction && key == "operands" {
                    if let Json::Array(items) = value {
                        rng.shuffle(items);
                    }
                }
                permute_operands(value, rng);
            }
        }
        Json::Array(items) => {
            for item in items {
                permute_operands(item, rng);
            }
        }
        _ => {}
    }
}

/// Reorder a whole group: shuffle the unit array and every junction's operands.
fn reorder_group(doc: &mut Json, group: &'static str, rng: &mut Rng) {
    let units = array_len(doc, group);
    for unit in 0..units {
        drop_normal_form(doc, group, unit);
    }
    if let Some(Json::Array(items)) = obj_mut(doc, group) {
        rng.shuffle(items);
        for item in items.iter_mut() {
            permute_operands(item, rng);
        }
    }
}

fn wrap(doc: &mut Json, path: &[Step], f: impl FnOnce(Json) -> Json) -> Option<()> {
    let node = at_mut(doc, path)?;
    let inner = std::mem::replace(node, Json::Null);
    *node = f(inner);
    Some(())
}

fn rename_unit(doc: &mut Json, group: &'static str, unit: usize, rng: &mut Rng) -> Option<String> {
    let id_node = at_mut(doc, &[Step::Key(group), Step::Index(unit), Step::Key("id")])?;
    let old = id_node.as_str()?.to_owned();
    let new = format!("{old}_{}", fresh_name(rng, "r"));
    *id_node = Json::String(new);
    Some(old)
}

fn set_policy(doc: &mut Json, key: &str, verb: &str) -> Option<()> {
    let policy = obj_mut(doc, "policy")?;
    let slot = obj_mut(policy, key)?;
    *slot = string(verb);
    Some(())
}

// --- the operators -----------------------------------------------------------------------------

/// Every operator, in a fixed order. The order is part of the corpus identity.
pub const OPERATORS: [&str; 44] = [
    // properties: one loosening each
    "prop-join-fresh",
    "prop-trivialize",
    "prop-complementary-disjunct",
    "prop-drop-junct",
    "prop-always-to-eventually",
    "prop-remove-claim",
    // properties: disguises over a random loosening
    "prop-rename+weaken",
    "prop-reorder+weaken",
    "prop-double-negation+weaken",
    "prop-vacuous-conjunct+weaken",
    "prop-source-prose+weaken",
    "prop-unlock-rider+weaken",
    // assumptions: one tightening each
    "asm-conjoin-fresh",
    "asm-falsify",
    "asm-drop-disjunct",
    "asm-eventually-to-always",
    "asm-add-assumption",
    // assumptions: disguises
    "asm-rename+strengthen",
    "asm-reorder+strengthen",
    "asm-double-negation+strengthen",
    "asm-vacuous-conjunct+strengthen",
    "asm-unlock-rider+strengthen",
    // bounds
    "bnd-shrink",
    "bnd-shrink+grow-other",
    // observers
    "obs-drop-element",
    "obs-swap-element",
    "obs-rename+coarsen",
    // faults
    "flt-remove-class",
    "flt-remove-profile",
    "flt-swap-class",
    // fairness
    "fair-add",
    "fair-weak-to-strong",
    "fair-condition-to-null",
    "fair-remove",
    // assurance
    "asr-lower-minimum",
    "asr-drop-independent-checker",
    "asr-drop-clean-recompute",
    "asr-raise-minimum+drop-flag",
    "asr-accept-extra-evidence-class",
    // compound
    "cmp-two-fields",
    // controls (meaning-preserving)
    "ctl-reorder",
    "ctl-source-prose",
    "ctl-name-metadata",
    "ctl-double-negation-atom",
];

const PROP_LOOSENINGS: [Loosen; 5] = [
    Loosen::JoinFresh,
    Loosen::Trivialize,
    Loosen::Complement,
    Loosen::DropJunct,
    Loosen::TemporalSwap,
];

/// The single-field weakening operators `cmp-two-fields` draws from, with their field.
const SINGLE_FIELD: [(&str, Field); 17] = [
    ("prop-join-fresh", Field::Properties),
    ("prop-trivialize", Field::Properties),
    ("prop-complementary-disjunct", Field::Properties),
    ("prop-drop-junct", Field::Properties),
    ("asm-conjoin-fresh", Field::Assumptions),
    ("asm-falsify", Field::Assumptions),
    ("asm-add-assumption", Field::Assumptions),
    ("bnd-shrink", Field::Bounds),
    ("obs-drop-element", Field::Observers),
    ("flt-remove-class", Field::Faults),
    ("flt-remove-profile", Field::Faults),
    ("fair-add", Field::Fairness),
    ("fair-weak-to-strong", Field::Fairness),
    ("asr-lower-minimum", Field::Assurance),
    ("asr-drop-independent-checker", Field::Assurance),
    ("asr-drop-clean-recompute", Field::Assurance),
    ("asr-accept-extra-evidence-class", Field::Assurance),
];

fn target(field: Field, direction: Direction) -> Vec<Target> {
    vec![Target { field, direction }]
}

fn weaken_property(doc: &mut Json, edit: Loosen, rng: &mut Rng) -> Option<(usize, Vec<Step>)> {
    edit_expression(doc, "claims", Sense::Loosen, edit, rng)
}

fn random_loosening(doc: &mut Json, sense: Sense, rng: &mut Rng) -> Option<(usize, Vec<Step>)> {
    let group = if sense == Sense::Loosen {
        "claims"
    } else {
        "assumptions"
    };
    let start = rng.below(PROP_LOOSENINGS.len());
    for offset in 0..PROP_LOOSENINGS.len() {
        let edit = PROP_LOOSENINGS[(start + offset) % PROP_LOOSENINGS.len()];
        let mut trial = doc.clone();
        if let Some(found) = edit_expression(&mut trial, group, sense, edit, rng) {
            *doc = trial;
            return Some(found);
        }
    }
    None
}

const LEVELS: [&str; 5] = ["observed", "sampled", "bounded", "validated", "proved"];
const FAULT_CLASSES: [&str; 6] = [
    "crash",
    "delay",
    "duplication",
    "loss",
    "partition",
    "recovery",
];
const EXTRA_EVIDENCE: [&str; 4] = [
    "example",
    "sampled",
    "bounded-schedules",
    "production-observation",
];

fn level_index(doc: &Json) -> Option<usize> {
    let minimum = get(get(doc, "assurance")?, "minimum")?.as_str()?;
    LEVELS.iter().position(|level| *level == minimum)
}

fn set_assurance(doc: &mut Json, key: &str, value: Json) -> Option<()> {
    let assurance = obj_mut(doc, "assurance")?;
    match assurance {
        Json::Object(map) => {
            map.insert(key.to_owned(), value);
            Some(())
        }
        _ => None,
    }
}

fn flag_true(doc: &Json, key: &str) -> bool {
    get(doc, "assurance")
        .and_then(|a| get(a, key))
        .and_then(Json::as_bool)
        == Some(true)
}

fn string_items(json: Option<&Json>) -> Vec<String> {
    match json {
        Some(Json::Array(items)) => items
            .iter()
            .filter_map(|item| item.as_str().map(str::to_owned))
            .collect(),
        _ => Vec::new(),
    }
}

fn action_names(node: &Json, out: &mut Vec<String>) {
    match node {
        Json::Object(map) => {
            if map.get("kind").and_then(Json::as_str) == Some("action") {
                if let Some(name) = map.get("name").and_then(Json::as_str) {
                    out.push(name.to_owned());
                }
            }
            for value in map.values() {
                action_names(value, out);
            }
        }
        Json::Array(items) => {
            for item in items {
                action_names(item, out);
            }
        }
        _ => {}
    }
}

fn fairness_keys(doc: &Json) -> Vec<(String, String)> {
    match get(doc, "fairness") {
        Some(Json::Array(items)) => items
            .iter()
            .filter_map(|item| {
                Some((
                    get(item, "kind")?.as_str()?.to_owned(),
                    get(item, "action")?.as_str()?.to_owned(),
                ))
            })
            .collect(),
        _ => Vec::new(),
    }
}

/// Apply one operator. `None` when it does not apply to this base.
#[allow(clippy::too_many_lines)]
fn apply(operator: &str, base: &Json, rng: &mut Rng) -> Option<(Json, Expectation)> {
    let mut doc = base.clone();
    let weak_prop =
        || Expectation::Weakening(target(Field::Properties, Direction::WeakensProperty));
    let strong_asm =
        || Expectation::Weakening(target(Field::Assumptions, Direction::StrengthensAssumption));
    let expectation = match operator {
        "prop-join-fresh" => {
            weaken_property(&mut doc, Loosen::JoinFresh, rng)?;
            weak_prop()
        }
        "prop-trivialize" => {
            weaken_property(&mut doc, Loosen::Trivialize, rng)?;
            weak_prop()
        }
        "prop-complementary-disjunct" => {
            weaken_property(&mut doc, Loosen::Complement, rng)?;
            weak_prop()
        }
        "prop-drop-junct" => {
            weaken_property(&mut doc, Loosen::DropJunct, rng)?;
            weak_prop()
        }
        "prop-always-to-eventually" => {
            weaken_property(&mut doc, Loosen::TemporalSwap, rng)?;
            weak_prop()
        }
        "prop-remove-claim" => {
            let n = array_len(&doc, "claims");
            if n == 0 {
                return None;
            }
            let drop = rng.below(n);
            if let Some(Json::Array(items)) = obj_mut(&mut doc, "claims") {
                items.remove(drop);
            }
            weak_prop()
        }
        "prop-rename+weaken" | "asm-rename+strengthen" => {
            let (sense, group) = if operator.starts_with("prop") {
                (Sense::Loosen, "claims")
            } else {
                (Sense::Tighten, "assumptions")
            };
            let (unit, _) = random_loosening(&mut doc, sense, rng)?;
            rename_unit(&mut doc, group, unit, rng)?;
            if sense == Sense::Loosen {
                weak_prop()
            } else {
                strong_asm()
            }
        }
        "prop-reorder+weaken" | "asm-reorder+strengthen" => {
            let (sense, group) = if operator.starts_with("prop") {
                (Sense::Loosen, "claims")
            } else {
                (Sense::Tighten, "assumptions")
            };
            random_loosening(&mut doc, sense, rng)?;
            reorder_group(&mut doc, group, rng);
            if sense == Sense::Loosen {
                weak_prop()
            } else {
                strong_asm()
            }
        }
        "prop-double-negation+weaken" | "asm-double-negation+strengthen" => {
            let sense = if operator.starts_with("prop") {
                Sense::Loosen
            } else {
                Sense::Tighten
            };
            let (_, path) = random_loosening(&mut doc, sense, rng)?;
            wrap(&mut doc, &path, |node| negation(negation(node)))?;
            if sense == Sense::Loosen {
                weak_prop()
            } else {
                strong_asm()
            }
        }
        "prop-vacuous-conjunct+weaken" | "asm-vacuous-conjunct+strengthen" => {
            let sense = if operator.starts_with("prop") {
                Sense::Loosen
            } else {
                Sense::Tighten
            };
            let (_, path) = random_loosening(&mut doc, sense, rng)?;
            let q = fresh_predicate(rng);
            wrap(&mut doc, &path, |node| {
                junction(
                    "and",
                    vec![node, junction("or", vec![q.clone(), negation(q)])],
                )
            })?;
            if sense == Sense::Loosen {
                weak_prop()
            } else {
                strong_asm()
            }
        }
        "prop-source-prose+weaken" => {
            let (unit, _) = random_loosening(&mut doc, Sense::Loosen, rng)?;
            if let Some(Json::Object(expr)) = at_mut(
                &mut doc,
                &[
                    Step::Key("claims"),
                    Step::Index(unit),
                    Step::Key("expression"),
                ],
            ) {
                expr.insert(
                    "source".to_owned(),
                    string("cosmetic refactor; the property is unchanged"),
                );
            }
            weak_prop()
        }
        "prop-unlock-rider+weaken" => {
            random_loosening(&mut doc, Sense::Loosen, rng)?;
            set_policy(&mut doc, "properties", "unlocked")?;
            weak_prop()
        }
        "asm-conjoin-fresh" => {
            edit_expression(
                &mut doc,
                "assumptions",
                Sense::Tighten,
                Loosen::JoinFresh,
                rng,
            )?;
            strong_asm()
        }
        "asm-falsify" => {
            edit_expression(
                &mut doc,
                "assumptions",
                Sense::Tighten,
                Loosen::Trivialize,
                rng,
            )?;
            strong_asm()
        }
        "asm-drop-disjunct" => {
            edit_expression(
                &mut doc,
                "assumptions",
                Sense::Tighten,
                Loosen::DropJunct,
                rng,
            )?;
            strong_asm()
        }
        "asm-eventually-to-always" => {
            edit_expression(
                &mut doc,
                "assumptions",
                Sense::Tighten,
                Loosen::TemporalSwap,
                rng,
            )?;
            strong_asm()
        }
        "asm-add-assumption" => {
            let body = object(vec![
                ("kind", string("always")),
                ("operand", fresh_predicate(rng)),
            ]);
            let unit = object(vec![
                ("classification", string("environment")),
                (
                    "expression",
                    object(vec![("ast", body), ("fragment", string("Finite"))]),
                ),
                ("id", Json::String(fresh_name(rng, "asm"))),
            ]);
            match obj_mut(&mut doc, "assumptions")? {
                Json::Array(items) => {
                    let at = rng.below(items.len() + 1);
                    items.insert(at, unit);
                }
                _ => return None,
            }
            strong_asm()
        }
        "asm-unlock-rider+strengthen" => {
            random_loosening(&mut doc, Sense::Tighten, rng)?;
            set_policy(&mut doc, "assumptions", "unlocked")?;
            strong_asm()
        }
        "bnd-shrink" | "bnd-shrink+grow-other" => {
            let bounds = obj_mut(&mut doc, "bounds")?;
            let Json::Object(map) = bounds else {
                return None;
            };
            let shrinkable: Vec<String> = map
                .iter()
                .filter(|(key, value)| match value {
                    Json::Integer(v) => *v > i64::from(key.as_str() == "nodes"),
                    Json::Null => key.as_str() == "values" || key.as_str() == "depth",
                    _ => false,
                })
                .map(|(key, _)| key.clone())
                .collect();
            if shrinkable.is_empty() {
                return None;
            }
            let key = shrinkable[rng.below(shrinkable.len())].clone();
            let floor = i64::from(key == "nodes");
            let slot = map.get_mut(&key)?;
            let shrunk = match &*slot {
                Json::Integer(v) => {
                    let span = usize::try_from(*v - floor).unwrap_or(1).max(1);
                    let step = i64::try_from(1 + rng.below(span)).unwrap_or(1);
                    Json::Integer(*v - step)
                }
                _ => Json::Integer(i64::try_from(1 + rng.below(8)).unwrap_or(1)),
            };
            *slot = shrunk;
            if operator == "bnd-shrink+grow-other" {
                let others: Vec<String> = map.keys().filter(|k| **k != key).cloned().collect();
                let other = others[rng.below(others.len())].clone();
                let slot = map.get_mut(&other)?;
                let grown = match &*slot {
                    Json::Integer(v) => Json::Integer(*v + 1 + i64::from(rng.coin())),
                    _ => return None,
                };
                *slot = grown;
            }
            Expectation::Weakening(target(Field::Bounds, Direction::ShrinksBound))
        }
        "obs-drop-element" | "obs-swap-element" | "obs-rename+coarsen" => {
            let n = array_len(&doc, "observers");
            if n == 0 {
                return None;
            }
            let unit = rng.below(n);
            let sets = [
                "events",
                "state_projection",
                "knowledge_projection",
                "security_projection",
            ];
            let observer = at_mut(&mut doc, &[Step::Key("observers"), Step::Index(unit)])?;
            let non_empty: Vec<&str> = sets
                .iter()
                .copied()
                .filter(|set| !string_items(get(observer, set)).is_empty())
                .collect();
            if non_empty.is_empty() {
                return None;
            }
            let set = non_empty[rng.below(non_empty.len())];
            let Json::Array(items) = obj_mut(observer, set)? else {
                return None;
            };
            let drop = rng.below(items.len());
            items.remove(drop);
            if operator == "obs-swap-element" {
                items.push(Json::String(fresh_name(rng, "obs")));
            }
            if operator == "obs-rename+coarsen" {
                let old = rename_unit(&mut doc, "observers", unit, rng)?;
                let new = at(
                    &doc,
                    &[Step::Key("observers"), Step::Index(unit), Step::Key("id")],
                )?
                .clone();
                // Keep the document well formed: claims that name the observer follow it.
                if let Some(Json::Array(claims)) = obj_mut(&mut doc, "claims") {
                    for claim in claims {
                        if let Some(slot) = obj_mut(claim, "observer") {
                            if slot.as_str() == Some(old.as_str()) {
                                *slot = new.clone();
                            }
                        }
                    }
                }
            }
            Expectation::Weakening(target(Field::Observers, Direction::CoarsensObserver))
        }
        "flt-remove-class" | "flt-remove-profile" | "flt-swap-class" => {
            let key = if operator == "flt-remove-profile" {
                "profiles"
            } else {
                "enabled"
            };
            let model = obj_mut(&mut doc, "fault_model")?;
            let Json::Array(items) = obj_mut(model, key)? else {
                return None;
            };
            if items.is_empty() {
                return None;
            }
            let drop = rng.below(items.len());
            let removed = items.remove(drop);
            if operator == "flt-swap-class" {
                let present: Vec<String> = items
                    .iter()
                    .filter_map(|i| i.as_str().map(str::to_owned))
                    .collect();
                let absent: Vec<&str> = FAULT_CLASSES
                    .iter()
                    .copied()
                    .filter(|c| !present.iter().any(|p| p == c) && Some(*c) != removed.as_str())
                    .collect();
                if absent.is_empty() {
                    return None;
                }
                items.push(string(absent[rng.below(absent.len())]));
                items.sort_by(|a, b| a.as_str().cmp(&b.as_str()));
            }
            Expectation::Weakening(target(Field::Faults, Direction::RemovesFault))
        }
        "fair-add" => {
            let mut actions = Vec::new();
            action_names(base, &mut actions);
            actions.sort();
            actions.dedup();
            actions.push(fresh_name(rng, "act"));
            let keys = fairness_keys(&doc);
            let kind = if rng.coin() { "strong" } else { "weak" };
            let action = actions[rng.below(actions.len())].clone();
            if keys.iter().any(|(_, a)| *a == action) {
                return None;
            }
            let condition = if rng.coin() {
                Json::Null
            } else {
                fresh_predicate(rng)
            };
            let unit = object(vec![
                ("action", Json::String(action)),
                ("condition", condition),
                ("kind", string(kind)),
            ]);
            let Json::Array(items) = obj_mut(&mut doc, "fairness")? else {
                return None;
            };
            let at = rng.below(items.len() + 1);
            items.insert(at, unit);
            Expectation::Weakening(target(Field::Fairness, Direction::StrengthensFairness))
        }
        "fair-weak-to-strong" | "fair-condition-to-null" | "fair-remove" => {
            let keys = fairness_keys(&doc);
            let candidates: Vec<usize> = (0..keys.len())
                .filter(|i| match operator {
                    "fair-weak-to-strong" => {
                        keys[*i].0 == "weak"
                            && !keys.iter().any(|(k, a)| k == "strong" && *a == keys[*i].1)
                    }
                    "fair-condition-to-null" => at(
                        &doc,
                        &[
                            Step::Key("fairness"),
                            Step::Index(*i),
                            Step::Key("condition"),
                        ],
                    )
                    .is_some_and(|c| !c.is_null()),
                    _ => true,
                })
                .collect();
            if candidates.is_empty() {
                return None;
            }
            let unit = candidates[rng.below(candidates.len())];
            let direction = match operator {
                "fair-weak-to-strong" => {
                    *at_mut(
                        &mut doc,
                        &[Step::Key("fairness"), Step::Index(unit), Step::Key("kind")],
                    )? = string("strong");
                    Direction::StrengthensFairness
                }
                "fair-condition-to-null" => {
                    *at_mut(
                        &mut doc,
                        &[
                            Step::Key("fairness"),
                            Step::Index(unit),
                            Step::Key("condition"),
                        ],
                    )? = Json::Null;
                    Direction::StrengthensFairness
                }
                _ => {
                    let Json::Array(items) = obj_mut(&mut doc, "fairness")? else {
                        return None;
                    };
                    items.remove(unit);
                    Direction::ChangesFairness
                }
            };
            Expectation::Weakening(target(Field::Fairness, direction))
        }
        "asr-lower-minimum" => {
            let level = level_index(&doc)?;
            if level == 0 {
                return None;
            }
            let lower = rng.below(level);
            set_assurance(&mut doc, "minimum", string(LEVELS[lower]))?;
            Expectation::Weakening(target(Field::Assurance, Direction::DowngradesAssurance))
        }
        "asr-drop-independent-checker" | "asr-drop-clean-recompute" => {
            let key = if operator == "asr-drop-independent-checker" {
                "independent_checker"
            } else {
                "clean_recompute"
            };
            if !flag_true(&doc, key) {
                return None;
            }
            set_assurance(&mut doc, key, Json::Bool(false))?;
            Expectation::Weakening(target(Field::Assurance, Direction::DowngradesAssurance))
        }
        "asr-raise-minimum+drop-flag" => {
            let level = level_index(&doc)?;
            if level + 1 >= LEVELS.len() {
                return None;
            }
            let flags: Vec<&str> = ["independent_checker", "clean_recompute"]
                .into_iter()
                .filter(|f| flag_true(&doc, f))
                .collect();
            if flags.is_empty() {
                return None;
            }
            let higher = level + 1 + rng.below(LEVELS.len() - level - 1);
            set_assurance(&mut doc, "minimum", string(LEVELS[higher]))?;
            set_assurance(&mut doc, flags[rng.below(flags.len())], Json::Bool(false))?;
            Expectation::Weakening(target(Field::Assurance, Direction::DowngradesAssurance))
        }
        "asr-accept-extra-evidence-class" => {
            let assurance = get(&doc, "assurance")?;
            let present = get(assurance, "accepted_evidence_classes")?;
            let mut classes = string_items(Some(present));
            let absent: Vec<&str> = EXTRA_EVIDENCE
                .iter()
                .copied()
                .filter(|c| !classes.iter().any(|p| p == c))
                .collect();
            if absent.is_empty() {
                return None;
            }
            classes.push(absent[rng.below(absent.len())].to_owned());
            classes.sort();
            set_assurance(
                &mut doc,
                "accepted_evidence_classes",
                Json::Array(classes.into_iter().map(Json::String).collect()),
            )?;
            Expectation::Weakening(target(Field::Assurance, Direction::ChangesAssurance))
        }
        "cmp-two-fields" => {
            let first = SINGLE_FIELD[rng.below(SINGLE_FIELD.len())];
            let others: Vec<(&str, Field)> = SINGLE_FIELD
                .iter()
                .copied()
                .filter(|(_, field)| *field != first.1)
                .collect();
            let second = others[rng.below(others.len())];
            let (after_first, mut e1) = apply(first.0, &doc, rng)?;
            let (after_second, e2) = apply(second.0, &after_first, rng)?;
            let (Expectation::Weakening(t1), Expectation::Weakening(t2)) = (&mut e1, e2) else {
                return None;
            };
            t1.extend(t2);
            t1.sort();
            doc = after_second;
            e1
        }
        "ctl-reorder" => {
            reorder_group(&mut doc, "claims", rng);
            reorder_group(&mut doc, "assumptions", rng);
            Expectation::Control
        }
        "ctl-source-prose" => {
            for group in ["claims", "assumptions"] {
                for unit in 0..array_len(&doc, group) {
                    if let Some(Json::Object(expr)) = at_mut(
                        &mut doc,
                        &[Step::Key(group), Step::Index(unit), Step::Key("expression")],
                    ) {
                        expr.insert(
                            "source".to_owned(),
                            Json::String(format!("reworded prose {}", fresh_name(rng, "s"))),
                        );
                    }
                }
            }
            Expectation::Control
        }
        "ctl-name-metadata" => {
            if let Json::Object(map) = &mut doc {
                map.insert("name".to_owned(), Json::String(fresh_name(rng, "name")));
            }
            Expectation::Control
        }
        "ctl-double-negation-atom" => {
            let group = if rng.coin() { "claims" } else { "assumptions" };
            let units = array_len(&doc, group);
            if units == 0 {
                return None;
            }
            let unit = rng.below(units);
            let ast_path = vec![
                Step::Key(group),
                Step::Index(unit),
                Step::Key("expression"),
                Step::Key("ast"),
            ];
            let ast = at(&doc, &ast_path)?.clone();
            let mut sites = Vec::new();
            collect_sites(&ast, &mut Vec::new(), Polarity::Positive, &mut sites);
            let leaves: Vec<Site> = sites
                .into_iter()
                .filter(|s| matches!(s.kind.as_str(), "predicate" | "action" | "compare"))
                .collect();
            if leaves.is_empty() {
                return None;
            }
            let site = &leaves[rng.below(leaves.len())];
            let mut full = ast_path;
            full.extend(site.path.iter().cloned());
            wrap(&mut doc, &full, |node| negation(negation(node)))?;
            drop_normal_form(&mut doc, group, unit);
            Expectation::Control
        }
        _ => return None,
    };
    Some((doc, expectation))
}

/// Generate the corpus: for every base and every operator, up to `variants`
/// distinct mutants. An operator that cannot apply to a base (for example a fault
/// removal on a contract with no faults) contributes none for that base.
pub fn generate(seed: u64, bases: &[Base], variants: usize) -> Vec<Mutant> {
    let mut out = Vec::new();
    for (b, base) in bases.iter().enumerate() {
        for (o, operator) in OPERATORS.iter().enumerate() {
            let mut rng = Rng::new(mix(
                seed,
                u64::try_from(b).unwrap_or(0),
                u64::try_from(o).unwrap_or(0),
                0,
            ));
            let mut seen: Vec<Vec<u8>> = vec![base.document.to_canonical_bytes()];
            let mut produced = 0;
            // A bounded number of attempts: an operator with few sites yields fewer
            // distinct variants, never an unbounded loop.
            for _attempt in 0..variants * 4 {
                if produced == variants {
                    break;
                }
                let Some((document, expectation)) = apply(operator, &base.document, &mut rng)
                else {
                    continue;
                };
                let bytes = document.to_canonical_bytes();
                if seen.contains(&bytes) {
                    continue;
                }
                seen.push(bytes);
                out.push(Mutant {
                    id: format!("C020-{}-{operator}-{produced}", base.label),
                    base: base.label.clone(),
                    operator,
                    expectation,
                    document,
                });
                produced += 1;
            }
        }
    }
    out
}
