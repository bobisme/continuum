//! Seeded defects and the shrinker (docs/19 §4, "Semantic engine" mutants; RFC 0013's
//! requirement that every differential arrow carry "mutation tests").
//!
//! # The mutations
//!
//! Each [`DefectClass`] has one mutation operator, taken from the docs/19 §4 list:
//!
//! | Class | docs/19 §4 mutant | Operator |
//! |---|---|---|
//! | [`DefectClass::Dependence`] | "declare conflicting events independent" | Pick two work steps of different processes that store distinct constants into one cell under a guard reading only their own phase. Remove the cell from both footprints and drop any declared conflict between them. |
//! | [`DefectClass::Fairness`] | "omit fairness edge" | Pick a weakly fair `release` and declare it unfair. |
//! | [`DefectClass::Obligation`] | "skip obligation discharge" | Pick a `finalize` and drop its "obligation discharged" conjunct, so the owner can reach Cancelled while the obligation is open. |
//! | [`DefectClass::Cancellation`] | a lifecycle regression | Pick a `drain` and make it write phase `0` (Active) where it wrote `2` (Draining). |
//!
//! The site is chosen from the candidates, in canonical name order, by the caller's
//! seed. An operator changes declarations or syntax only. It never consults the oracle,
//! so "the oracle detected it" is a result, not a tautology.
//!
//! # Shrinking
//!
//! [`shrink`] is greedy one-at-a-time deletion (delta debugging's `1-minimal` reduction)
//! over three kinds of component, in this order and repeated to a fixed point:
//! actions, then variables that no remaining action mentions, then declared conflicts.
//! A deletion is kept iff the oracle still reports, on the smaller system, a finding of
//! every kind of the requested class the input showed; actions the caller protects
//! (normally the mutation sites) are never deleted. The result is **1-minimal**:
//! deleting any single remaining unprotected action, unmentioned variable, or conflict
//! loses one of those kinds. It is not claimed to be
//! globally minimal. The witnesses inside the shrunk artifact are shortest paths by
//! construction ([`crate::witness::shortest`]).

use std::collections::BTreeSet;

use crate::expr::{BoolExpr, CmpOp, IntExpr};
use crate::model::ActionDecl;

use super::oracle::{self, Artifact, DefectClass, Finding, Limits, Refusal};
use super::prng::SplitMix64;
use super::system::{Fairness, Footprint, Role, System, SystemError, ordered};

/// Why no defect was seeded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SeedError {
    /// The system has no site the class's operator applies to.
    NoSite {
        /// The class.
        class: DefectClass,
    },
    /// The mutated declarations were rejected.
    Invalid(SystemError),
}

impl From<SystemError> for SeedError {
    fn from(source: SystemError) -> Self {
        Self::Invalid(source)
    }
}

fn pick<T: Clone>(candidates: &[T], seed: u64) -> Option<T> {
    let count = u64::try_from(candidates.len()).ok()?;
    let index = usize::try_from(SplitMix64::new(seed).below(count)).ok()?;
    candidates.get(index).cloned()
}

fn names_with_role(system: &System, role: Role) -> Vec<String> {
    system
        .meta()
        .iter()
        .filter(|(_, meta)| meta.role == role)
        .map(|(name, _)| name.clone())
        .collect()
}

fn phase_of(system: &System, action: &str) -> Option<String> {
    let process = system.meta().get(action)?.process;
    Some(format!("p{process}_ph"))
}

/// A constant store `cell := c` whose guard reads only `phase`.
fn plain_store(system: &System, action: &str) -> Option<(String, i64)> {
    let model = system.model();
    let declared = model.actions().get(model.action_index(action)?)?;
    let phase = phase_of(system, action)?;
    let mut guard_reads: Vec<String> = Vec::new();
    declared.guard().variables(&mut guard_reads);
    if guard_reads.iter().any(|name| *name != phase) {
        return None;
    }
    let [outcome] = declared.outcomes() else {
        return None;
    };
    let [assignment] = outcome.assignments() else {
        return None;
    };
    match assignment.value() {
        IntExpr::Const(value) => Some((assignment.variable().as_str().to_owned(), *value)),
        _ => None,
    }
}

/// Seed one defect of `class` into `system`, at a site chosen by `seed`.
///
/// # Errors
///
/// [`SeedError::NoSite`] when the operator has nowhere to apply, and
/// [`SeedError::Invalid`] if the mutated declarations fail validation.
pub fn seed_defect(system: &System, class: DefectClass, seed: u64) -> Result<Seeded, SeedError> {
    let no_site = || SeedError::NoSite { class };
    let (mutated, sites) = match class {
        DefectClass::Dependence => {
            let works = names_with_role(system, Role::Work);
            let mut pairs: Vec<(String, String, String)> = Vec::new();
            for (i, first) in works.iter().enumerate() {
                for second in works.iter().skip(i.saturating_add(1)) {
                    let (Some(left), Some(right)) =
                        (plain_store(system, first), plain_store(system, second))
                    else {
                        continue;
                    };
                    let processes = (
                        system.meta().get(first).map(|meta| meta.process),
                        system.meta().get(second).map(|meta| meta.process),
                    );
                    if left.0 == right.0 && left.1 != right.1 && processes.0 != processes.1 {
                        pairs.push((first.clone(), second.clone(), left.0));
                    }
                }
            }
            let (first, second, cell) = pick(&pairs, seed).ok_or_else(no_site)?;
            let mut mutated = system.without_conflict(&ordered(&first, &second))?;
            for action in [&first, &second] {
                let Some(meta) = mutated.meta().get(action.as_str()).cloned() else {
                    return Err(no_site());
                };
                let mut meta = meta;
                let mut reads = meta.footprint.reads().clone();
                let mut writes = meta.footprint.writes().clone();
                reads.remove(&cell);
                writes.remove(&cell);
                meta.footprint = Footprint::new(reads, writes);
                mutated = mutated.with_meta(action, meta)?;
            }
            (mutated, BTreeSet::from([first, second]))
        }
        DefectClass::Fairness => {
            let releases: Vec<String> = names_with_role(system, Role::Release)
                .into_iter()
                .filter(|name| {
                    system
                        .meta()
                        .get(name)
                        .is_some_and(|meta| meta.fairness == Fairness::Weak)
                })
                .collect();
            let action = pick(&releases, seed).ok_or_else(no_site)?;
            let mut meta = system.meta().get(&action).cloned().ok_or_else(no_site)?;
            meta.fairness = Fairness::Unfair;
            (system.with_meta(&action, meta)?, BTreeSet::from([action]))
        }
        DefectClass::Obligation => {
            let action =
                pick(&names_with_role(system, Role::Finalize), seed).ok_or_else(no_site)?;
            let phase = phase_of(system, &action).ok_or_else(no_site)?;
            let declared = system
                .model()
                .action_index(&action)
                .and_then(|index| system.model().actions().get(index))
                .ok_or_else(no_site)?;
            let outcomes: Vec<Vec<(&str, IntExpr)>> = declared
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
            let guard = BoolExpr::compare(CmpOp::Eq, IntExpr::var(&phase), IntExpr::constant(2));
            let decl = ActionDecl::enumerated(&action, guard, outcomes);
            (system.with_action(decl, &action)?, BTreeSet::from([action]))
        }
        DefectClass::Cancellation => {
            let action = pick(&names_with_role(system, Role::Drain), seed).ok_or_else(no_site)?;
            let phase = phase_of(system, &action).ok_or_else(no_site)?;
            let declared = system
                .model()
                .action_index(&action)
                .and_then(|index| system.model().actions().get(index))
                .ok_or_else(no_site)?;
            let decl = ActionDecl::deterministic(
                &action,
                declared.guard().clone(),
                vec![(phase.as_str(), IntExpr::constant(0))],
            );
            (system.with_action(decl, &action)?, BTreeSet::from([action]))
        }
    };
    let site = sites.iter().cloned().collect::<Vec<_>>().join(",");
    let system = mutated.with_lineage(format!(
        "seed-defect class={} seed={seed} site={site}",
        class.token()
    ))?;
    Ok(Seeded {
        system,
        class,
        sites,
    })
}

/// A system with one seeded defect.
#[derive(Debug, Clone)]
pub struct Seeded {
    /// The mutated system.
    pub system: System,
    /// The class seeded.
    pub class: DefectClass,
    /// The actions the mutation changed. [`shrink`] can be told to keep them, so the
    /// minimal witness still contains the mutation rather than some other way of
    /// showing the same class.
    pub sites: BTreeSet<String>,
}

/// A shrunk system and its oracle artifact.
#[derive(Debug, Clone)]
pub struct Shrunk {
    /// The 1-minimal system.
    pub system: System,
    /// The oracle's artifact for it; it reports every kind of the requested class the
    /// input reported.
    pub artifact: Artifact,
    /// How many deletions were kept.
    pub deletions: u32,
}

/// Why nothing was shrunk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShrinkError {
    /// The input system shows no finding of the class, so there is nothing to keep.
    NotDetected {
        /// The class.
        class: DefectClass,
    },
    /// The oracle refused the input system.
    Refused(Refusal),
    /// Recording the shrink in the lineage failed.
    Invalid(SystemError),
}

fn kinds_of(artifact: &Artifact, class: DefectClass) -> BTreeSet<&'static str> {
    artifact
        .findings()
        .iter()
        .filter(|finding| finding.class() == class)
        .map(Finding::kind)
        .collect()
}

fn shows(
    candidate: &System,
    class: DefectClass,
    kinds: &BTreeSet<&'static str>,
    limits: &Limits,
) -> bool {
    oracle::run(candidate, limits)
        .is_ok_and(|artifact| kinds.is_subset(&kinds_of(&artifact, class)))
}

/// Shrink `system` to a 1-minimal system that still shows every *kind* of `class`
/// finding the input shows ([`Finding::kind`]), never deleting an action in `keep`.
///
/// Keeping every kind, rather than just the class, stops the shrinker from trading the
/// seeded defect for a cheaper one of the same class — deleting a `release` is also an
/// obligation defect, but not the one that was planted. `keep` does the same for the
/// mutation site; pass [`Seeded::sites`], or an empty set to shrink freely.
///
/// # Errors
///
/// [`ShrinkError::NotDetected`] when the input shows no such finding, and
/// [`ShrinkError::Refused`] when the oracle refuses the input.
pub fn shrink(
    system: &System,
    class: DefectClass,
    keep: &BTreeSet<String>,
    limits: &Limits,
) -> Result<Shrunk, ShrinkError> {
    let original = oracle::run(system, limits).map_err(ShrinkError::Refused)?;
    let kinds = kinds_of(&original, class);
    if kinds.is_empty() {
        return Err(ShrinkError::NotDetected { class });
    }
    let mut current = system.clone();
    let mut deletions = 0_u32;
    loop {
        let mut candidates: Vec<System> = Vec::new();
        for action in current.model().actions() {
            if keep.contains(action.name().as_str()) {
                continue;
            }
            if let Ok(smaller) = current.without_action(action.name().as_str()) {
                candidates.push(smaller);
            }
        }
        for variable in current.model().variables() {
            let name = variable.name().as_str();
            if !current.mentions(name)
                && let Ok(smaller) = current.without_variable(name)
            {
                candidates.push(smaller);
            }
        }
        for pair in current.conflicts() {
            if let Ok(smaller) = current.without_conflict(pair) {
                candidates.push(smaller);
            }
        }
        let Some(smaller) = candidates
            .into_iter()
            .find(|candidate| shows(candidate, class, &kinds, limits))
        else {
            break;
        };
        current = smaller;
        deletions = deletions.saturating_add(1);
    }
    let system = current
        .with_lineage(format!(
            "shrink class={} deletions={deletions}",
            class.token()
        ))
        .map_err(ShrinkError::Invalid)?;
    let artifact = oracle::run(&system, limits).map_err(ShrinkError::Refused)?;
    Ok(Shrunk {
        system,
        artifact,
        deletions,
    })
}
