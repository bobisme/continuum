//! A finite semantic system: a [`Model`] plus the declarations an optimized engine
//! would *trust* about it.
//!
//! The model part is this crate's own [`Model`] — typed variables with finite domains,
//! guarded deterministic actions, explicit initial states — and nothing here duplicates
//! it. What a model does not carry, and a reduction engine needs, is layered on top by
//! name so that it survives the rebuilds the defect seeder and the shrinker perform:
//!
//! | Declaration | What it claims | Where the oracle tests the claim |
//! |---|---|---|
//! | [`VarKind`] per variable | data, obligation flag, or cancellation phase | structural validation here |
//! | [`Footprint`] per action | the variables the action may read and write | read escape (static), write escape (every reachable transition) |
//! | conflict pairs | two actions are dependent whatever their footprints say | they remove a pair from the claimed independence relation |
//! | [`Fairness`] per action | weak fairness (docs/02 §8 `WeakFair`) | the fair-lasso search |
//! | [`ObligationDecl`] | an obligation flag, and the phase of its owner | quiescence and completion leaks, fair lassos |
//! | [`PhaseDecl`] | a cancellation phase variable and its final value | monotonicity on every reachable transition |
//!
//! A system's fairness is the per-action declaration above, never the model's own
//! assumptions (`crate::fairness`, bn-1ln12): a model that declares any is refused
//! ([`SystemError::ModelFairness`]), so the oracle never ignores one silently. The
//! model-level assumptions are checked by [`crate::liveness`].
//!
//! The **claimed independence relation** is fixed by these declarations and by nothing
//! else: two distinct actions are claimed independent iff their footprints do not
//! interfere ([`Footprint::interferes`]) and the pair is not a declared conflict. That
//! is the relation a partial-order reduction would use (RFC 0014's "conservative
//! dependencies"), and the oracle compares it against the semantic relation it
//! computes.

use std::collections::{BTreeMap, BTreeSet};

use crate::expr::{BoolExpr, IntExpr};
use crate::model::{Action, ActionDecl, Model, ModelBuilder, ModelError};

// ---------------------------------------------------------------------------
// declarations
// ---------------------------------------------------------------------------

/// What a state variable stands for. The domain itself is the model's.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum VarKind {
    /// Ordinary shared data.
    Data,
    /// An obligation flag: `0` is discharged, any other value is open.
    Obligation,
    /// A cancellation phase (docs/02 §7): values only ever increase.
    Phase,
}

impl VarKind {
    const fn token(self) -> &'static str {
        match self {
            Self::Data => "data",
            Self::Obligation => "obligation",
            Self::Phase => "phase",
        }
    }
}

/// The part an action plays in its process's lifecycle.
///
/// Informational for the oracle, which judges behaviour and never role; the defect
/// seeder uses it to find a mutation site.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Role {
    /// A data step.
    Work,
    /// Opens the process's obligation.
    Acquire,
    /// Discharges the process's obligation.
    Release,
    /// Requests cancellation: `Active → Cancelling` (docs/02 §7).
    Cancel,
    /// Drains: `Cancelling → Draining`.
    Drain,
    /// Finalizes: `Draining → Cancelled`, only once obligations are discharged.
    Finalize,
}

impl Role {
    const fn token(self) -> &'static str {
        match self {
            Self::Work => "work",
            Self::Acquire => "acquire",
            Self::Release => "release",
            Self::Cancel => "cancel",
            Self::Drain => "drain",
            Self::Finalize => "finalize",
        }
    }
}

/// A fairness assumption on one action.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Fairness {
    /// No assumption: the scheduler may starve the action forever.
    Unfair,
    /// Weak fairness: an action that is enabled continuously from some point on is
    /// eventually taken.
    Weak,
}

impl Fairness {
    const fn token(self) -> &'static str {
        match self {
            Self::Unfair => "unfair",
            Self::Weak => "weak",
        }
    }
}

/// The variables an action is declared to read and to write.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct Footprint {
    reads: BTreeSet<String>,
    writes: BTreeSet<String>,
}

impl Footprint {
    /// A footprint from explicit sets.
    #[must_use]
    pub const fn new(reads: BTreeSet<String>, writes: BTreeSet<String>) -> Self {
        Self { reads, writes }
    }

    /// The footprint the action's syntax implies: it reads every variable its guard or
    /// any update expression mentions, and writes every variable it assigns. This is
    /// sound by construction for the model's closed expression language.
    #[must_use]
    pub fn syntactic(action: &Action) -> Self {
        let mut mentioned: Vec<String> = Vec::new();
        action.guard().variables(&mut mentioned);
        let mut writes = BTreeSet::new();
        for outcome in action.outcomes() {
            for assignment in outcome.assignments() {
                assignment.value().variables(&mut mentioned);
                writes.insert(assignment.variable().as_str().to_owned());
            }
        }
        Self {
            reads: mentioned.into_iter().collect(),
            writes,
        }
    }

    /// The declared read set.
    #[must_use]
    pub const fn reads(&self) -> &BTreeSet<String> {
        &self.reads
    }

    /// The declared write set.
    #[must_use]
    pub const fn writes(&self) -> &BTreeSet<String> {
        &self.writes
    }

    /// Whether two footprints interfere: one writes what the other reads or writes.
    #[must_use]
    pub fn interferes(&self, other: &Self) -> bool {
        let hits = |writes: &BTreeSet<String>, other: &Self| {
            writes
                .iter()
                .any(|name| other.reads.contains(name) || other.writes.contains(name))
        };
        hits(&self.writes, other) || hits(&other.writes, self)
    }

    fn forget(&mut self, variable: &str) {
        self.reads.remove(variable);
        self.writes.remove(variable);
    }
}

/// Everything declared about one action beyond its guard and updates.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActionMeta {
    /// The process the action belongs to.
    pub process: u32,
    /// Its lifecycle role.
    pub role: Role,
    /// Its declared footprint.
    pub footprint: Footprint,
    /// Its fairness assumption.
    pub fairness: Fairness,
}

/// An obligation: open while `variable` is non-zero.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ObligationDecl {
    /// The obligation's name.
    pub name: String,
    /// The [`VarKind::Obligation`] variable that holds it.
    pub variable: String,
    /// The [`VarKind::Phase`] variable of its owner, when it has one. Reaching that
    /// phase's final value with the obligation open is a completion leak.
    pub owner_phase: Option<String>,
}

/// A cancellation phase variable and the value that means "finished".
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PhaseDecl {
    /// The [`VarKind::Phase`] variable.
    pub variable: String,
    /// Its final value.
    pub final_value: i64,
}

/// The parts [`System::new`] validates.
#[derive(Debug, Clone)]
pub struct SystemParts {
    /// The transition model.
    pub model: Model,
    /// A kind for every model variable, by name.
    pub kinds: BTreeMap<String, VarKind>,
    /// A declaration for every model action, by name.
    pub meta: BTreeMap<String, ActionMeta>,
    /// Declared conflicts, each pair stored `(smaller name, larger name)`.
    pub conflicts: BTreeSet<(String, String)>,
    /// Declared obligations.
    pub obligations: Vec<ObligationDecl>,
    /// Declared cancellation phases.
    pub phases: Vec<PhaseDecl>,
    /// How the system came to be, one line per step (generation, mutation, shrink).
    pub lineage: Vec<String>,
}

/// Why a set of parts is not a system.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SystemError {
    /// The model itself did not build.
    Model(ModelError),
    /// A model variable has no kind, or a kind names no model variable.
    KindMismatch {
        /// The variable.
        variable: String,
    },
    /// A model action has no declaration, or a declaration names no model action.
    MetaMismatch {
        /// The action.
        action: String,
    },
    /// A declaration names a variable the model does not declare, or one of the wrong
    /// kind.
    BadVariable {
        /// Where the name appeared.
        site: String,
        /// The name.
        variable: String,
    },
    /// A conflict pair that is not two distinct declared actions in canonical order.
    BadConflict {
        /// The pair.
        pair: (String, String),
    },
    /// Two obligations or two phases share a name.
    Duplicate {
        /// The name.
        name: String,
    },
    /// A phase's final value lies outside its variable's domain.
    FinalOutOfDomain {
        /// The variable.
        variable: String,
    },
    /// A lineage line with a line break in it, which would corrupt the encoding.
    BadLineage,
    /// The model declares its own fairness assumptions (bn-1ln12). A system states
    /// fairness per action in [`ActionMeta::fairness`], and the oracle reads only that;
    /// a model-level assumption would be silently ignored, and the rebuilding edits
    /// would drop it, so it is refused instead.
    ModelFairness,
}

impl From<ModelError> for SystemError {
    fn from(source: ModelError) -> Self {
        Self::Model(source)
    }
}

// ---------------------------------------------------------------------------
// the system
// ---------------------------------------------------------------------------

/// A validated finite semantic system.
#[derive(Debug, Clone)]
pub struct System {
    model: Model,
    kinds: BTreeMap<String, VarKind>,
    meta: BTreeMap<String, ActionMeta>,
    conflicts: BTreeSet<(String, String)>,
    obligations: BTreeMap<String, ObligationDecl>,
    phases: BTreeMap<String, PhaseDecl>,
    lineage: Vec<String>,
}

impl System {
    /// Validate `parts`.
    ///
    /// # Errors
    ///
    /// Every arm of [`SystemError`] except [`SystemError::Model`], which only the
    /// rebuilding edits return.
    pub fn new(parts: SystemParts) -> Result<Self, SystemError> {
        let SystemParts {
            model,
            kinds,
            meta,
            conflicts,
            obligations,
            phases,
            lineage,
        } = parts;
        if !model.fairness().is_empty() {
            return Err(SystemError::ModelFairness);
        }
        let declared: BTreeSet<&str> = model
            .variables()
            .iter()
            .map(|variable| variable.name().as_str())
            .collect();
        for name in &declared {
            if !kinds.contains_key(*name) {
                return Err(SystemError::KindMismatch {
                    variable: (*name).to_owned(),
                });
            }
        }
        for name in kinds.keys() {
            if !declared.contains(name.as_str()) {
                return Err(SystemError::KindMismatch {
                    variable: name.clone(),
                });
            }
        }
        let actions: BTreeSet<&str> = model
            .actions()
            .iter()
            .map(|action| action.name().as_str())
            .collect();
        for name in &actions {
            if !meta.contains_key(*name) {
                return Err(SystemError::MetaMismatch {
                    action: (*name).to_owned(),
                });
            }
        }
        for (name, declaration) in &meta {
            if !actions.contains(name.as_str()) {
                return Err(SystemError::MetaMismatch {
                    action: name.clone(),
                });
            }
            let footprint = &declaration.footprint;
            for variable in footprint.reads.iter().chain(footprint.writes.iter()) {
                if !declared.contains(variable.as_str()) {
                    return Err(SystemError::BadVariable {
                        site: format!("footprint of {name}"),
                        variable: variable.clone(),
                    });
                }
            }
        }
        for pair in &conflicts {
            let (left, right) = pair;
            if left >= right
                || !actions.contains(left.as_str())
                || !actions.contains(right.as_str())
            {
                return Err(SystemError::BadConflict { pair: pair.clone() });
            }
        }
        let kind_is = |variable: &str, kind: VarKind| kinds.get(variable) == Some(&kind);
        let mut phase_map: BTreeMap<String, PhaseDecl> = BTreeMap::new();
        for phase in phases {
            if !kind_is(&phase.variable, VarKind::Phase) {
                return Err(SystemError::BadVariable {
                    site: "phase".to_owned(),
                    variable: phase.variable,
                });
            }
            let admits = model
                .variable_index(&phase.variable)
                .and_then(|index| model.variables().get(index))
                .is_some_and(|variable| variable.domain().admits(phase.final_value));
            if !admits {
                return Err(SystemError::FinalOutOfDomain {
                    variable: phase.variable,
                });
            }
            if phase_map.contains_key(&phase.variable) {
                return Err(SystemError::Duplicate {
                    name: phase.variable,
                });
            }
            phase_map.insert(phase.variable.clone(), phase);
        }
        let mut obligation_map: BTreeMap<String, ObligationDecl> = BTreeMap::new();
        for obligation in obligations {
            if !kind_is(&obligation.variable, VarKind::Obligation) {
                return Err(SystemError::BadVariable {
                    site: format!("obligation {}", obligation.name),
                    variable: obligation.variable,
                });
            }
            if let Some(owner) = &obligation.owner_phase
                && !phase_map.contains_key(owner)
            {
                return Err(SystemError::BadVariable {
                    site: format!("owner phase of obligation {}", obligation.name),
                    variable: owner.clone(),
                });
            }
            if obligation.name.is_empty() || !obligation.name.bytes().all(|b| b.is_ascii_graphic())
            {
                return Err(SystemError::BadVariable {
                    site: "obligation name".to_owned(),
                    variable: obligation.name,
                });
            }
            if obligation_map.contains_key(&obligation.name) {
                return Err(SystemError::Duplicate {
                    name: obligation.name,
                });
            }
            obligation_map.insert(obligation.name.clone(), obligation);
        }
        if lineage.iter().any(|line| line.contains(['\n', '\r'])) {
            return Err(SystemError::BadLineage);
        }
        Ok(Self {
            model,
            kinds,
            meta,
            conflicts,
            obligations: obligation_map,
            phases: phase_map,
            lineage,
        })
    }

    /// The transition model.
    #[must_use]
    pub const fn model(&self) -> &Model {
        &self.model
    }

    /// The kind of every variable.
    #[must_use]
    pub const fn kinds(&self) -> &BTreeMap<String, VarKind> {
        &self.kinds
    }

    /// The declaration of every action.
    #[must_use]
    pub const fn meta(&self) -> &BTreeMap<String, ActionMeta> {
        &self.meta
    }

    /// The declared conflict pairs.
    #[must_use]
    pub const fn conflicts(&self) -> &BTreeSet<(String, String)> {
        &self.conflicts
    }

    /// The declared obligations, by name.
    #[must_use]
    pub const fn obligations(&self) -> &BTreeMap<String, ObligationDecl> {
        &self.obligations
    }

    /// The declared cancellation phases, by variable.
    #[must_use]
    pub const fn phases(&self) -> &BTreeMap<String, PhaseDecl> {
        &self.phases
    }

    /// How the system came to be.
    #[must_use]
    pub fn lineage(&self) -> &[String] {
        &self.lineage
    }

    /// Whether the declarations claim `first` and `second` independent: distinct,
    /// non-interfering footprints, and not a declared conflict. An undeclared name is
    /// never claimed independent of anything.
    #[must_use]
    pub fn claims_independent(&self, first: &str, second: &str) -> bool {
        if first == second {
            return false;
        }
        let (Some(left), Some(right)) = (self.meta.get(first), self.meta.get(second)) else {
            return false;
        };
        let pair = ordered(first, second);
        !left.footprint.interferes(&right.footprint) && !self.conflicts.contains(&pair)
    }

    /// The canonical text encoding: ASCII, LF-terminated lines, every collection in
    /// its canonical order. Two systems are the same system iff these bytes are equal.
    #[must_use]
    pub fn encode(&self) -> String {
        let mut out = String::from("continuum-semantic-system v1\n");
        for line in &self.lineage {
            out.push_str(&format!("lineage {line}\n"));
        }
        for variable in self.model.variables() {
            let name = variable.name().as_str();
            let kind = self.kinds.get(name).map_or("?", |kind| kind.token());
            out.push_str(&format!(
                "variable {name} {kind} {} {}\n",
                variable.domain().lo(),
                variable.domain().hi()
            ));
        }
        for state in self.model.initial_states() {
            out.push_str(&format!("initial {}\n", encode_vector(state.as_slice())));
        }
        for action in self.model.actions() {
            let name = action.name().as_str();
            if let Some(meta) = self.meta.get(name) {
                out.push_str(&format!(
                    "action {name} process {} role {} fairness {}\n",
                    meta.process,
                    meta.role.token(),
                    meta.fairness.token()
                ));
                out.push_str(&format!("  guard {}\n", encode_bool(action.guard())));
                for outcome in action.outcomes() {
                    let updates: Vec<String> = outcome
                        .assignments()
                        .iter()
                        .map(|a| format!("{}={}", a.variable().as_str(), encode_int(a.value())))
                        .collect();
                    out.push_str(&format!("  outcome {}\n", updates.join(" ")));
                }
                out.push_str(&format!(
                    "  reads {}\n",
                    encode_names(&meta.footprint.reads)
                ));
                out.push_str(&format!(
                    "  writes {}\n",
                    encode_names(&meta.footprint.writes)
                ));
            }
        }
        for (left, right) in &self.conflicts {
            out.push_str(&format!("conflict {left} {right}\n"));
        }
        for obligation in self.obligations.values() {
            out.push_str(&format!(
                "obligation {} variable {} owner-phase {}\n",
                obligation.name,
                obligation.variable,
                obligation.owner_phase.as_deref().unwrap_or("-")
            ));
        }
        for phase in self.phases.values() {
            out.push_str(&format!(
                "phase {} final {}\n",
                phase.variable, phase.final_value
            ));
        }
        out.push_str("end-system\n");
        out
    }

    // -----------------------------------------------------------------------
    // edits: each rebuilds and revalidates, so an edited system is a system
    // -----------------------------------------------------------------------

    fn parts(&self) -> SystemParts {
        SystemParts {
            model: self.model.clone(),
            kinds: self.kinds.clone(),
            meta: self.meta.clone(),
            conflicts: self.conflicts.clone(),
            obligations: self.obligations.values().cloned().collect(),
            phases: self.phases.values().cloned().collect(),
            lineage: self.lineage.clone(),
        }
    }

    /// This system with one more lineage line.
    pub(crate) fn with_lineage(&self, line: String) -> Result<Self, SystemError> {
        let mut parts = self.parts();
        parts.lineage.push(line);
        Self::new(parts)
    }

    /// This system with `action`'s declaration replaced.
    pub(crate) fn with_meta(&self, action: &str, meta: ActionMeta) -> Result<Self, SystemError> {
        let mut parts = self.parts();
        parts.meta.insert(action.to_owned(), meta);
        Self::new(parts)
    }

    /// This system without one conflict pair.
    pub fn without_conflict(&self, pair: &(String, String)) -> Result<Self, SystemError> {
        let mut parts = self.parts();
        parts.conflicts.remove(pair);
        Self::new(parts)
    }

    /// This system with the named action's guard and outcomes replaced.
    pub(crate) fn with_action(
        &self,
        replacement: ActionDecl,
        name: &str,
    ) -> Result<Self, SystemError> {
        let mut actions: Vec<ActionDecl> = Vec::new();
        for action in self.model.actions() {
            if action.name().as_str() == name {
                actions.push(replacement.clone());
            } else {
                actions.push(decl_of(action));
            }
        }
        let mut parts = self.parts();
        parts.model = rebuild(&self.model, actions, None)?;
        Self::new(parts)
    }

    /// This system without the named action, its declaration, and its conflicts.
    pub fn without_action(&self, name: &str) -> Result<Self, SystemError> {
        let actions: Vec<ActionDecl> = self
            .model
            .actions()
            .iter()
            .filter(|action| action.name().as_str() != name)
            .map(decl_of)
            .collect();
        let mut parts = self.parts();
        parts.model = rebuild(&self.model, actions, None)?;
        parts.meta.remove(name);
        parts
            .conflicts
            .retain(|(left, right)| left != name && right != name);
        Self::new(parts)
    }

    /// Whether any guard or update expression mentions `variable`.
    pub fn mentions(&self, variable: &str) -> bool {
        self.model.actions().iter().any(|action| {
            Footprint::syntactic(action).reads.contains(variable)
                || Footprint::syntactic(action).writes.contains(variable)
        })
    }

    /// This system without a variable no action mentions, and without every
    /// declaration about it.
    pub fn without_variable(&self, variable: &str) -> Result<Self, SystemError> {
        let actions: Vec<ActionDecl> = self.model.actions().iter().map(decl_of).collect();
        let mut parts = self.parts();
        parts.model = rebuild(&self.model, actions, Some(variable))?;
        parts.kinds.remove(variable);
        for meta in parts.meta.values_mut() {
            meta.footprint.forget(variable);
        }
        parts
            .obligations
            .retain(|obligation| obligation.variable != variable);
        for obligation in &mut parts.obligations {
            if obligation.owner_phase.as_deref() == Some(variable) {
                obligation.owner_phase = None;
            }
        }
        parts.phases.retain(|phase| phase.variable != variable);
        Self::new(parts)
    }
}

/// A pair in canonical `(smaller, larger)` order.
#[must_use]
pub fn ordered(first: &str, second: &str) -> (String, String) {
    if first <= second {
        (first.to_owned(), second.to_owned())
    } else {
        (second.to_owned(), first.to_owned())
    }
}

/// An action back in declaration form.
pub(crate) fn decl_of(action: &Action) -> ActionDecl {
    let outcomes: Vec<Vec<(&str, IntExpr)>> = action
        .outcomes()
        .iter()
        .map(|outcome| {
            outcome
                .assignments()
                .iter()
                .map(|a| (a.variable().as_str(), a.value().clone()))
                .collect()
        })
        .collect();
    ActionDecl::enumerated(action.name().as_str(), action.guard().clone(), outcomes)
}

/// Rebuild `model` with new actions and, optionally, one variable dropped. Initial
/// states are projected and deduplicated.
fn rebuild(
    model: &Model,
    actions: Vec<ActionDecl>,
    drop: Option<&str>,
) -> Result<Model, ModelError> {
    let mut builder = ModelBuilder::new();
    for variable in model.variables() {
        if Some(variable.name().as_str()) != drop {
            builder = builder.variable(
                variable.name().as_str(),
                variable.domain().lo(),
                variable.domain().hi(),
            );
        }
    }
    for action in actions {
        builder = builder.action(action);
    }
    let mut initial: BTreeSet<Vec<(String, i64)>> = BTreeSet::new();
    for state in model.initial_states() {
        let bindings: Vec<(String, i64)> = model
            .variables()
            .iter()
            .zip(state.as_slice().iter())
            .filter(|(variable, _)| Some(variable.name().as_str()) != drop)
            .map(|(variable, value)| (variable.name().as_str().to_owned(), *value))
            .collect();
        initial.insert(bindings);
    }
    for bindings in &initial {
        let borrowed: Vec<(&str, i64)> = bindings
            .iter()
            .map(|(name, value)| (name.as_str(), *value))
            .collect();
        builder = builder.initial_state(&borrowed);
    }
    builder.build()
}

// ---------------------------------------------------------------------------
// canonical text
// ---------------------------------------------------------------------------

pub(crate) fn encode_vector(values: &[i64]) -> String {
    let parts: Vec<String> = values.iter().map(i64::to_string).collect();
    format!("({})", parts.join(","))
}

fn encode_names(names: &BTreeSet<String>) -> String {
    if names.is_empty() {
        "-".to_owned()
    } else {
        names.iter().cloned().collect::<Vec<_>>().join(",")
    }
}

/// Prefix notation; variables carry a `$` so a name can never read as a literal.
/// Recursion is bounded by `MAX_EXPR_DEPTH`, which every built model satisfies.
pub(crate) fn encode_int(expr: &IntExpr) -> String {
    match expr {
        IntExpr::Const(value) => value.to_string(),
        IntExpr::Var(name) => format!("${name}"),
        IntExpr::Arith(op, left, right) => {
            format!("({op} {} {})", encode_int(left), encode_int(right))
        }
        IntExpr::Min(left, right) => format!("(min {} {})", encode_int(left), encode_int(right)),
        IntExpr::Max(left, right) => format!("(max {} {})", encode_int(left), encode_int(right)),
    }
}

/// See [`encode_int`].
pub(crate) fn encode_bool(expr: &BoolExpr) -> String {
    match expr {
        BoolExpr::Const(value) => value.to_string(),
        BoolExpr::Compare { op, left, right } => {
            format!("({op} {} {})", encode_int(left), encode_int(right))
        }
        BoolExpr::Not(inner) => format!("(not {})", encode_bool(inner)),
        BoolExpr::And(left, right) => format!("(and {} {})", encode_bool(left), encode_bool(right)),
        BoolExpr::Or(left, right) => format!("(or {} {})", encode_bool(left), encode_bool(right)),
        BoolExpr::Implies(left, right) => {
            format!("(implies {} {})", encode_bool(left), encode_bool(right))
        }
        BoolExpr::InRange { expr, lo, hi } => format!("(in {} {lo} {hi})", encode_int(expr)),
    }
}
