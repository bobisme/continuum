//! Definedness chains of the carried model (bn-iu8eh; RFC 0003 "Definedness"; RFC
//! 0013 "Undefined and partial behavior").
//!
//! # Why the kernel reads them
//!
//! A lowered model names each undefined read with a predicate `X#defined` of the
//! action or invariant `X`, and conjoins an action's definedness to its guard, so the
//! lowered relation computes no successor from an undefined read. That makes the
//! action look disabled at such a state, and it is not:
//!
//! > A state that violates `X#defined` is the typed outcome "undefined read in `X`",
//! > never a verdict of a declared invariant, and the verdict of `I` at such a state is
//! > not a CML verdict.
//! >
//! > — RFC 0003, "Definedness"
//!
//! RFC 0013 makes that result "an explicit `Undefined` semantic result that
//! invalidates the model". Two reads matter to a claim. An undefined action read
//! means the table is closed under the lowered relation and not under the model's
//! meaning, so no closure claim over that state is valid. An undefined read in the
//! claimed invariant means the invariant has no value there, so the invariant claim
//! is not valid. The chain of another predicate (`J#defined` under a claim about `I`)
//! does not bear on the claim, as in the reference engine's per-subject scan. The
//! reference engine refuses to emit such a certificate; this module lets the kernel
//! refuse to verify one from any producer (`crate::check`, step 4 of the model-bound
//! check). The kernel reads every table state, not only the reachable ones, as it
//! does for the invariant.
//!
//! # Independence
//!
//! The rule is the one `continuum_model_core::definedness::Definedness::of` states in
//! its documentation and RFC 0003 states. This module restates it over the kernel's
//! own decoded model (bytes, sorted arrays, binary search) and links nothing
//! (INV-004: the kernel depends on no crate).
//! `tests/kernel_definedness_differential.rs` in `continuum-engine-reference` compares
//! the two classifications through [`crate::check_certificate`].
//!
//! # The rule
//!
//! A *definedness predicate* is a predicate whose name ends in [`DEFINED_SUFFIX`] with
//! a non-empty rest. Its *chain base* is the name left after stripping every trailing
//! suffix while a non-empty rest remains, and its *depth* is how many were stripped.
//! All definedness predicates with one base form one *chain*.
//!
//! A chain guards the declared predicate named by its base only when every member is
//! well formed: each name on the walk from the member's own full name down to the base
//! (`I#defined#defined`, `I#defined`, `I`) is a declared predicate, is not an action's
//! name, is not the schema `X` of an action instance `X(…)`, and contains no `(`. Any
//! other chain is an action's chain (fail closed): it is evaluated at every state,
//! whatever the certificate claims. Within a chain, deeper members guard shallower
//! ones, so members are evaluated deepest first.
//!
//! # Cost
//!
//! [`classification_work`] bounds the byte comparisons [`Definedness::of`] makes, and
//! [`evaluation_work`] the expression nodes one state's scan evaluates. The caller
//! charges both before either runs (RFC 0005, "Resource bounds").

use std::collections::BTreeMap;

use crate::model::Model;

/// The suffix of a definedness predicate: `A#defined` for an action `A`, `I#defined`
/// for an invariant `I` (RFC 0003, "Definedness").
pub(crate) const DEFINED_SUFFIX: &[u8] = b"#defined";

/// `name` with one trailing [`DEFINED_SUFFIX`] removed, when a non-empty rest remains.
fn subject(name: &[u8]) -> Option<&[u8]> {
    name.strip_suffix(DEFINED_SUFFIX)
        .filter(|rest| !rest.is_empty())
}

/// The chain base of `name` and the number of suffixes stripped to reach it, or `None`
/// when `name` is not a definedness predicate.
fn base_and_depth(name: &[u8]) -> Option<(&[u8], u64)> {
    let mut base = subject(name)?;
    let mut depth: u64 = 1;
    while let Some(inner) = subject(base) {
        base = inner;
        depth = depth.saturating_add(1);
    }
    Some((base, depth))
}

/// `bits(n) + 1`: the probes of one binary search over `n` sorted entries.
fn probes(n: usize) -> Option<u64> {
    let bits = usize::BITS.checked_sub(n.leading_zeros())?;
    u64::from(bits).checked_add(1)
}

/// An upper bound on the byte comparisons [`Definedness::of`] makes over `model`, or
/// `None` when it does not fit in a `u64`.
///
/// Per definedness predicate of name length `len` and depth `d`, the walk visits
/// `d + 1` names. Each visit costs a `(` scan (`len`), a predicate lookup (`len` per
/// probe), an action lookup and a schema lookup (`len + 1` per probe each, plus a
/// prefix copy), charged as `len + 1` per unit. Grouping by base is one more
/// predicate-sized lookup, and sorting a chain is one probe per member. Every other
/// predicate costs one suffix test.
pub(crate) fn classification_work(model: &Model) -> Option<u64> {
    let predicate_probes = probes(model.predicate_count())?;
    let action_probes = probes(model.action_count())?;
    let per_visit = predicate_probes
        .checked_add(action_probes.checked_mul(2)?)?
        .checked_add(3)?;
    let suffix = u64::try_from(DEFINED_SUFFIX.len()).ok()?;
    let mut work: u64 = 0;
    for index in 0..model.predicate_count() {
        let name = model.predicate_bytes(index).unwrap_or(&[]);
        work = work.checked_add(suffix)?;
        let Some((_, depth)) = base_and_depth(name) else {
            continue;
        };
        let len = u64::try_from(name.len()).ok()?.checked_add(1)?;
        let visits = depth.checked_add(1)?;
        work = work
            .checked_add(visits.checked_mul(len)?.checked_mul(per_visit)?)?
            .checked_add(len.checked_mul(predicate_probes)?)?
            .checked_add(predicate_probes)?;
    }
    Some(work)
}

/// An upper bound on the expression nodes one state's definedness scan evaluates: the
/// action chains cover every definedness predicate at most once, and the guards of
/// the claimed predicate `claim` are members of the one chain whose base is the base
/// of `claim` (or `claim` itself). So: the size of every definedness predicate, plus
/// the size of every definedness predicate with that base. One linear pass over the
/// names, before any classification. `None` when it does not fit in a `u64`.
pub(crate) fn evaluation_work(model: &Model, claim: Option<usize>) -> Option<u64> {
    let claimed_base = claim
        .and_then(|index| model.predicate_bytes(index))
        .map(|name| base_and_depth(name).map_or(name, |(base, _)| base));
    let mut work: u64 = 0;
    for index in 0..model.predicate_count() {
        let name = model.predicate_bytes(index).unwrap_or(&[]);
        let Some((base, _)) = base_and_depth(name) else {
            continue;
        };
        let size = model.predicate_size(index);
        work = work.checked_add(size)?;
        if claimed_base == Some(base) {
            work = work.checked_add(size)?;
        }
    }
    Some(work)
}

/// A model's definedness predicates, classified by the base of their chain.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Definedness {
    /// Per predicate index: its chain depth (0 for an ordinary predicate).
    depth: Vec<u64>,
    /// Per predicate index: the chain it belongs to, or, for an ordinary predicate
    /// whose name is a chain base, the chain that guards it.
    chain_of: Vec<Option<usize>>,
    /// Every chain, deepest member first.
    chains: Vec<Vec<usize>>,
    /// The action chains, as indices into `chains`, ordered by their shallowest
    /// member's index.
    actions: Vec<usize>,
}

impl Definedness {
    /// Classify every definedness predicate of `model` by the rule of the module
    /// documentation. The caller charges [`classification_work`] first.
    pub(crate) fn of(model: &Model) -> Self {
        let count = model.predicate_count();
        let mut depth: Vec<u64> = vec![0; count];
        // Chains by base; a chain is a predicate's only while every member says so.
        let mut by_base: BTreeMap<&[u8], (Vec<usize>, bool)> = BTreeMap::new();
        for index in 0..count {
            let Some(name) = model.predicate_bytes(index) else {
                continue;
            };
            let Some((base, member_depth)) = base_and_depth(name) else {
                continue;
            };
            if let Some(slot) = depth.get_mut(index) {
                *slot = member_depth;
            }
            let mut well_formed = true;
            let mut here = Some(name);
            while let Some(current) = here {
                if current.contains(&b'(')
                    || model.predicate_by_bytes(current).is_none()
                    || model.is_action_or_schema(current)
                {
                    well_formed = false;
                }
                here = subject(current);
            }
            let entry = by_base.entry(base).or_insert_with(|| (Vec::new(), true));
            entry.0.push(index);
            entry.1 &= well_formed;
        }
        let mut chain_of: Vec<Option<usize>> = vec![None; count];
        let mut chains: Vec<Vec<usize>> = Vec::new();
        let mut actions: Vec<usize> = Vec::new();
        for (base, (mut members, well_formed)) in by_base {
            // Deepest first; depth is unique within a chain, the index breaks no tie.
            members.sort_by_key(|&i| (core::cmp::Reverse(depth.get(i).copied()), i));
            let id = chains.len();
            let named = model.predicate_by_bytes(base);
            if let Some(slot) = named.and_then(|i| chain_of.get_mut(i)) {
                *slot = Some(id);
            }
            if !well_formed || named.is_none() {
                actions.push(id);
            }
            for &member in &members {
                if let Some(slot) = chain_of.get_mut(member) {
                    *slot = Some(id);
                }
            }
            chains.push(members);
        }
        actions.sort_by_key(|&id| chains.get(id).and_then(|chain| chain.last().copied()));
        Self {
            depth,
            chain_of,
            chains,
            actions,
        }
    }

    /// Every predicate of every action chain, chain by chain in the order of their
    /// shallowest member, each chain deepest first.
    pub(crate) fn action_guards(&self) -> impl Iterator<Item = usize> + '_ {
        self.actions
            .iter()
            .filter_map(|&id| self.chains.get(id))
            .flatten()
            .copied()
    }

    /// The definedness predicates that guard the reads of the predicate at `index`,
    /// deepest first: for an ordinary predicate that is a chain base, the whole chain;
    /// for a definedness predicate, the members of its chain strictly deeper than it.
    /// Empty otherwise.
    pub(crate) fn guards_of(&self, index: usize) -> &[usize] {
        let Some(chain) = self
            .chain_of
            .get(index)
            .copied()
            .flatten()
            .and_then(|id| self.chains.get(id))
        else {
            return &[];
        };
        let own = self.depth.get(index).copied().unwrap_or(0);
        let deeper = chain
            .iter()
            .take_while(|&&i| self.depth.get(i).copied().unwrap_or(0) > own)
            .count();
        chain.get(..deeper).unwrap_or(&[])
    }

    /// Whether the predicate at `index` is itself a definedness predicate, whose false
    /// value is an undefined read rather than a violation.
    pub(crate) fn is_guard(&self, index: usize) -> bool {
        self.depth.get(index).is_some_and(|&depth| depth > 0)
    }
}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::indexing_slicing,
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::arithmetic_side_effects,
        reason = "test bodies assert on known-shaped fixtures"
    )]

    use super::*;
    use crate::fixture::{Expr, ModelPlan};

    fn model(actions: &[&str], predicates: &[&str]) -> Model {
        let mut plan = ModelPlan::two_outcomes();
        plan.actions.truncate(1);
        let template = plan.actions[0].clone();
        let mut names: Vec<&str> = actions.to_vec();
        names.sort_unstable();
        plan.actions = names
            .iter()
            .map(|name| crate::fixture::PlannedAction {
                name: (*name).to_owned(),
                ..template.clone()
            })
            .collect();
        let mut predicates: Vec<&str> = predicates.to_vec();
        predicates.sort_unstable();
        plan.predicates = predicates
            .iter()
            .map(|name| ((*name).to_owned(), Expr::Bool(true)))
            .collect();
        plan.fairness = Vec::new();
        crate::model::decode(&plan.encode()).expect("the plan decodes")
    }

    fn at(model: &Model, name: &str) -> usize {
        model.predicate_by_bytes(name.as_bytes()).unwrap()
    }

    #[test]
    fn the_base_strips_every_suffix_and_counts_them() {
        assert_eq!(base_and_depth(b"I#defined#defined"), Some((&b"I"[..], 2)));
        assert_eq!(base_and_depth(b"I#defined"), Some((&b"I"[..], 1)));
        assert_eq!(
            base_and_depth(b"#defined#defined"),
            Some((&b"#defined"[..], 1))
        );
        assert_eq!(base_and_depth(b"#defined"), None);
        assert_eq!(base_and_depth(b"I"), None);
    }

    #[test]
    fn a_well_formed_chain_guards_its_base_deepest_first() {
        let m = model(
            &["A"],
            &["I", "I#defined", "I#defined#defined", "A#defined"],
        );
        let d = Definedness::of(&m);
        assert_eq!(
            d.guards_of(at(&m, "I")),
            &[at(&m, "I#defined#defined"), at(&m, "I#defined")]
        );
        assert_eq!(
            d.guards_of(at(&m, "I#defined")),
            &[at(&m, "I#defined#defined")]
        );
        assert!(d.guards_of(at(&m, "I#defined#defined")).is_empty());
        assert!(d.is_guard(at(&m, "I#defined")));
        assert!(!d.is_guard(at(&m, "I")));
        assert_eq!(
            d.action_guards().collect::<Vec<_>>(),
            vec![at(&m, "A#defined")]
        );
    }

    #[test]
    fn a_gap_a_collision_or_an_instance_name_reads_the_chain_as_an_actions() {
        let m = model(&["Step"], &["I", "I#defined#defined"]);
        let d = Definedness::of(&m);
        assert_eq!(
            d.action_guards().collect::<Vec<_>>(),
            vec![at(&m, "I#defined#defined")]
        );
        for action in ["I", "I#defined", "I(k=0)", "I#defined(k=0)"] {
            let m = model(&[action], &["I", "I#defined", "I#defined#defined"]);
            let d = Definedness::of(&m);
            assert_eq!(d.action_guards().count(), 2, "{action}");
        }
        for (action, base) in [("A", "A(k=0)"), ("A(k=1)", "A(k=0)"), ("P(a)(b)", "P(a)")] {
            let guard = format!("{base}#defined");
            let m = model(&[action], &[base, &guard]);
            let d = Definedness::of(&m);
            assert_eq!(
                d.action_guards().collect::<Vec<_>>(),
                vec![at(&m, &guard)],
                "{action} {base}"
            );
        }
    }

    #[test]
    fn action_chains_are_ordered_by_their_shallowest_member() {
        let m = model(
            &["A", "B"],
            &[
                "A#defined",
                "A#defined#defined",
                "B#defined",
                "B#defined#defined",
            ],
        );
        let d = Definedness::of(&m);
        assert_eq!(
            d.action_guards().collect::<Vec<_>>(),
            vec![
                at(&m, "A#defined#defined"),
                at(&m, "A#defined"),
                at(&m, "B#defined#defined"),
                at(&m, "B#defined"),
            ]
        );
    }

    #[test]
    fn the_charges_cover_only_definedness_predicates() {
        let plain = model(&["A"], &["I", "J"]);
        assert_eq!(evaluation_work(&plain, Some(0)), Some(0));
        let guarded = model(&["A"], &["I", "I#defined", "J", "J#defined"]);
        // Every definedness predicate once; the claimed predicate's chain twice.
        assert_eq!(evaluation_work(&guarded, None), Some(2));
        assert_eq!(evaluation_work(&guarded, Some(at(&guarded, "I"))), Some(3));
        assert_eq!(
            evaluation_work(&guarded, Some(at(&guarded, "I#defined"))),
            Some(3)
        );
        assert!(classification_work(&guarded) > classification_work(&plain));
    }
}
