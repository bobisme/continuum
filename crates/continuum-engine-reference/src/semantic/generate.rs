//! The seeded generator of finite semantic systems (docs/19 §2; RFC 0013, "Generated
//! finite universes", at the transition-system level).
//!
//! # What one generated system is
//!
//! `processes` processes share `data_variables` data cells `d0, d1, …`, each ranging
//! over `0..=data_max`. Process `p` owns two more variables:
//!
//! - `p{p}_ob`, an obligation flag in `0..=1` ([`VarKind::Obligation`]);
//! - `p{p}_ph`, a cancellation phase in `0..=3` ([`VarKind::Phase`]), where `0` is
//!   Active, `1` Cancelling, `2` Draining and `3` Cancelled — docs/02 §7's lifecycle,
//!   with its rule that Cancelled is reached only when `obligations == ∅`.
//!
//! Every variable starts at `0`, and process `p` declares these actions:
//!
//! | Action | Guard | Update | Fairness |
//! |---|---|---|---|
//! | `p{p}_acquire` | `ph == 0 ∧ ob == 0` | `ob := 1` | unfair |
//! | `p{p}_release` | `ob == 1 ∧ ph <= 2` | `ob := 0` | weak |
//! | `p{p}_cancel` | `ph == 0` | `ph := 1` | unfair |
//! | `p{p}_drain` | `ph == 1` | `ph := 2` | weak |
//! | `p{p}_finalize` | `ph == 2 ∧ ob == 0` | `ph := 3` | weak |
//! | `p{p}_w{j}`, `j < work_per_process` | `ph == 0 ∧ …` | one data update | unfair |
//!
//! A work step is drawn from four update forms — store a constant, increment under
//! `v < data_max`, decrement under `v > 0`, copy another cell — optionally with one
//! more comparison of a data cell against a constant conjoined to its guard. The safety
//! conjuncts keep every update inside its domain, so no generated model can fail
//! docs/16 PO-MOD-003.
//!
//! Cancellation is an environment choice and is never forced, so `cancel` is unfair;
//! drain, finalize and release are the progress the lifecycle promises, so they are
//! weakly fair (docs/02 §8, `WeakFair`).
//!
//! # What is guaranteed, and why
//!
//! - **Footprints are exact syntax** ([`Footprint::syntactic`]), so the claimed
//!   independence relation of an unmutated system is sound.
//! - **Obligations are discharged.** `release` is enabled whenever the obligation is
//!   open (an open obligation implies the owner is not Cancelled, because `finalize`
//!   requires it discharged and phases only increase), so no quiescent state holds an
//!   open obligation, and no weakly fair cycle can keep one open.
//! - **Every defect class has somewhere to land** when `processes >= 2`: the first work
//!   step of processes 0 and 1 stores two *distinct* constants into `d0` under a guard
//!   that reads only the owner's phase. Both are enabled in the initial state, the two
//!   orders disagree on `d0`, and alternating them is a cycle that needs no fair action.
//!   That is the site of the dependence defect and the loop of the fairness defect.
//!
//! These are claims about the generator; the oracle does not take them on trust, and
//! the negative-control tests run it over generated systems to confirm them.
//!
//! # Determinism
//!
//! Every choice is drawn from one [`SplitMix64`] stream seeded by the caller, in a fixed
//! order, and every collection is ordered, so `generate(seed, shape)` is a function.

use std::collections::{BTreeMap, BTreeSet};

use crate::expr::{BoolExpr, CmpOp, IntExpr};
use crate::model::{ActionDecl, ModelBuilder};

use super::prng::SplitMix64;
use super::system::{
    ActionMeta, Fairness, Footprint, ObligationDecl, PhaseDecl, Role, System, SystemError,
    SystemParts, VarKind, ordered,
};

/// The size parameters of a generated system.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Shape {
    /// Processes, `1..=4`.
    pub processes: u8,
    /// Shared data cells, `1..=4`.
    pub data_variables: u8,
    /// The largest data value, `1..=4`.
    pub data_max: u8,
    /// Work steps per process, `1..=4`.
    pub work_per_process: u8,
    /// Extra declared conflicts drawn at random, `0..=8`.
    pub extra_conflicts: u8,
}

impl Shape {
    /// The default tiny shape: 192 configurations, 12 actions.
    pub const TINY: Self = Self {
        processes: 2,
        data_variables: 1,
        data_max: 2,
        work_per_process: 1,
        extra_conflicts: 1,
    };

    const RANGES: [(&'static str, u8, u8); 5] = [
        ("processes", 1, 4),
        ("data-variables", 1, 4),
        ("data-max", 1, 4),
        ("work-per-process", 1, 4),
        ("extra-conflicts", 0, 8),
    ];

    const fn fields(self) -> [u8; 5] {
        [
            self.processes,
            self.data_variables,
            self.data_max,
            self.work_per_process,
            self.extra_conflicts,
        ]
    }

    /// Check every field against its range.
    ///
    /// # Errors
    ///
    /// [`GenerateError::ShapeOutOfRange`] for the first field outside its range.
    pub fn validate(self) -> Result<(), GenerateError> {
        for ((field, lo, hi), value) in Self::RANGES.iter().zip(self.fields()) {
            if value < *lo || value > *hi {
                return Err(GenerateError::ShapeOutOfRange {
                    field,
                    value,
                    lo: *lo,
                    hi: *hi,
                });
            }
        }
        Ok(())
    }
}

/// Why no system was generated.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GenerateError {
    /// A shape field outside its declared range. The generator refuses rather than
    /// clamps, so a shape always means what it says.
    ShapeOutOfRange {
        /// The field.
        field: &'static str,
        /// The value given.
        value: u8,
        /// Inclusive lower bound.
        lo: u8,
        /// Inclusive upper bound.
        hi: u8,
    },
    /// The generated declarations were rejected. A generator defect, reported as data.
    Invalid(SystemError),
}

impl From<SystemError> for GenerateError {
    fn from(source: SystemError) -> Self {
        Self::Invalid(source)
    }
}

/// One lifecycle action: suffix, guard, single update, role, fairness.
type Lifecycle<'a> = (&'static str, BoolExpr, (&'a str, IntExpr), Role, Fairness);

fn var(name: &str) -> IntExpr {
    IntExpr::var(name)
}

fn int(value: u64) -> IntExpr {
    IntExpr::constant(i64::try_from(value).unwrap_or(i64::MAX))
}

fn cmp(op: CmpOp, name: &str, value: u64) -> BoolExpr {
    BoolExpr::compare(op, var(name), int(value))
}

/// Generate the system for `seed` and `shape`.
///
/// # Errors
///
/// [`GenerateError::ShapeOutOfRange`] for an out-of-range shape, and
/// [`GenerateError::Invalid`] if the declarations fail validation, which would be a
/// generator defect.
pub fn generate(seed: u64, shape: Shape) -> Result<System, GenerateError> {
    shape.validate()?;
    let mut stream = SplitMix64::new(seed);
    let data_max = u64::from(shape.data_max);
    let values = data_max.saturating_add(1);
    let cells: Vec<String> = (0..shape.data_variables).map(|i| format!("d{i}")).collect();
    let pick_cell = |stream: &mut SplitMix64| -> String {
        let index = usize::try_from(stream.below(u64::from(shape.data_variables))).unwrap_or(0);
        cells.get(index).cloned().unwrap_or_else(|| "d0".to_owned())
    };

    let mut builder = ModelBuilder::new();
    let mut kinds: BTreeMap<String, VarKind> = BTreeMap::new();
    let mut initial: Vec<(String, i64)> = Vec::new();
    for cell in &cells {
        builder = builder.variable(cell, 0, i64::from(shape.data_max));
        kinds.insert(cell.clone(), VarKind::Data);
        initial.push((cell.clone(), 0));
    }

    // The planted pair: two distinct constants for `d0`.
    let first_store = stream.below(values);
    let offset = stream.below(data_max).saturating_add(1);
    let second_store = first_store
        .saturating_add(offset)
        .checked_rem(values)
        .unwrap_or(0);

    let mut roles: BTreeMap<String, (u32, Role, Fairness)> = BTreeMap::new();
    let mut obligations: Vec<ObligationDecl> = Vec::new();
    let mut phases: Vec<PhaseDecl> = Vec::new();
    for p in 0..shape.processes {
        let process = u32::from(p);
        let ob = format!("p{p}_ob");
        let ph = format!("p{p}_ph");
        builder = builder.variable(&ob, 0, 1).variable(&ph, 0, 3);
        kinds.insert(ob.clone(), VarKind::Obligation);
        kinds.insert(ph.clone(), VarKind::Phase);
        initial.push((ob.clone(), 0));
        initial.push((ph.clone(), 0));
        obligations.push(ObligationDecl {
            name: format!("p{p}_obligation"),
            variable: ob.clone(),
            owner_phase: Some(ph.clone()),
        });
        phases.push(PhaseDecl {
            variable: ph.clone(),
            final_value: 3,
        });

        let lifecycle: [Lifecycle<'_>; 5] = [
            (
                "acquire",
                BoolExpr::and(cmp(CmpOp::Eq, &ph, 0), cmp(CmpOp::Eq, &ob, 0)),
                (&ob, int(1)),
                Role::Acquire,
                Fairness::Unfair,
            ),
            (
                "release",
                BoolExpr::and(cmp(CmpOp::Eq, &ob, 1), cmp(CmpOp::Le, &ph, 2)),
                (&ob, int(0)),
                Role::Release,
                Fairness::Weak,
            ),
            (
                "cancel",
                cmp(CmpOp::Eq, &ph, 0),
                (&ph, int(1)),
                Role::Cancel,
                Fairness::Unfair,
            ),
            (
                "drain",
                cmp(CmpOp::Eq, &ph, 1),
                (&ph, int(2)),
                Role::Drain,
                Fairness::Weak,
            ),
            (
                "finalize",
                BoolExpr::and(cmp(CmpOp::Eq, &ph, 2), cmp(CmpOp::Eq, &ob, 0)),
                (&ph, int(3)),
                Role::Finalize,
                Fairness::Weak,
            ),
        ];
        for (suffix, guard, update, role, fairness) in lifecycle {
            let name = format!("p{p}_{suffix}");
            builder = builder.action(ActionDecl::deterministic(&name, guard, vec![update]));
            roles.insert(name, (process, role, fairness));
        }

        for j in 0..shape.work_per_process {
            let name = format!("p{p}_w{j}");
            let running = cmp(CmpOp::Eq, &ph, 0);
            let planted = shape.processes >= 2 && p < 2 && j == 0;
            let (guard, target, value) = if planted {
                let constant = if p == 0 { first_store } else { second_store };
                (running, "d0".to_owned(), int(constant))
            } else {
                let target = pick_cell(&mut stream);
                let (guard, value) = match stream.below(4) {
                    0 => (running, int(stream.below(values))),
                    1 => (
                        BoolExpr::and(running, cmp(CmpOp::Lt, &target, data_max)),
                        IntExpr::plus(var(&target), int(1)),
                    ),
                    2 => (
                        BoolExpr::and(running, cmp(CmpOp::Gt, &target, 0)),
                        IntExpr::minus(var(&target), int(1)),
                    ),
                    _ => (running, var(&pick_cell(&mut stream))),
                };
                let guard = if stream.coin() {
                    let tested = pick_cell(&mut stream);
                    let op = match stream.below(4) {
                        0 => CmpOp::Le,
                        1 => CmpOp::Ge,
                        2 => CmpOp::Eq,
                        _ => CmpOp::Ne,
                    };
                    BoolExpr::and(guard, cmp(op, &tested, stream.below(values)))
                } else {
                    guard
                };
                (guard, target, value)
            };
            builder = builder.action(ActionDecl::deterministic(
                &name,
                guard,
                vec![(target.as_str(), value)],
            ));
            roles.insert(name, (process, Role::Work, Fairness::Unfair));
        }
    }
    let borrowed: Vec<(&str, i64)> = initial
        .iter()
        .map(|(name, value)| (name.as_str(), *value))
        .collect();
    let model = builder
        .initial_state(&borrowed)
        .build()
        .map_err(SystemError::Model)?;

    let mut meta: BTreeMap<String, ActionMeta> = BTreeMap::new();
    for action in model.actions() {
        let name = action.name().as_str();
        let (process, role, fairness) =
            roles
                .get(name)
                .copied()
                .unwrap_or((0, Role::Work, Fairness::Unfair));
        meta.insert(
            name.to_owned(),
            ActionMeta {
                process,
                role,
                footprint: Footprint::syntactic(action),
                fairness,
            },
        );
    }

    let names: Vec<&str> = roles.keys().map(String::as_str).collect();
    let count = u64::try_from(names.len()).unwrap_or(0);
    let mut conflicts: BTreeSet<(String, String)> = BTreeSet::new();
    for _ in 0..shape.extra_conflicts {
        let left = usize::try_from(stream.below(count)).unwrap_or(0);
        let right = usize::try_from(stream.below(count)).unwrap_or(0);
        if let (Some(left), Some(right)) = (names.get(left), names.get(right))
            && left != right
        {
            conflicts.insert(ordered(left, right));
        }
    }

    let system = System::new(SystemParts {
        model,
        kinds,
        meta,
        conflicts,
        obligations,
        phases,
        lineage: vec![format!(
            "generate seed={seed} processes={} data-variables={} data-max={} work-per-process={} extra-conflicts={}",
            shape.processes,
            shape.data_variables,
            shape.data_max,
            shape.work_per_process,
            shape.extra_conflicts
        )],
    })?;
    Ok(system)
}
