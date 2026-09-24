//! Definedness predicates: how a model says that a read is undefined (bn-24a5c).
//!
//! # The convention
//!
//! RFC 0003, "Finite values and quantifiers", "Definedness": `m[k]` is defined only
//! when `k` is a key of `m`. The lowering records each undefined read as a *named
//! predicate* of the model, and nothing else:
//!
//! - an invariant `I` with undefined reads gets the predicate `I#defined`, the
//!   conjunction of its items;
//! - an action `A` with undefined reads gets the predicate `A#defined`, the
//!   conjunction over its expanded instances of `D(G) && (G => D(U))`, and each
//!   instance's guard is conjoined with `D(G)` and `D(U)`, so no successor is computed
//!   from an undefined read.
//!
//! > A state that violates `X#defined` is the typed outcome "undefined read in `X`",
//! > never a verdict of a declared invariant, and the verdict of `I` at such a state is
//! > not a CML verdict.
//! >
//! > — RFC 0003, "Definedness"
//!
//! RFC 0013, "Undefined and partial behavior", gives the effect: an undefined operation
//! elaborates to "an explicit `Undefined` semantic result that invalidates the model".
//! So a state where `A#defined` is false is an error of the model at that state. The
//! action is not merely disabled there: its guard is false only so that the lowered
//! model computes no successor from the undefined read.
//!
//! This module is the one statement of that convention in the model core. The CML
//! elaborator re-exports [`DEFINED_SUFFIX`] and [`definedness_subject`] from here, so
//! the producer of the predicates and every engine that reads them share one spelling.
//! The model core itself gives the predicates no special evaluation: `X#defined` is an
//! ordinary [`crate::model::Predicate`]. Only its *reading* is fixed here.
//!
//! # Every definedness obligation a model carries
//!
//! A [`Model`]'s expressions are total: evaluation either returns a value or a typed
//! [`crate::model::EvaluationError`] (arithmetic overflow, a state outside the domain,
//! an update outside a domain). No expression reads a partial map, and no expression
//! refers to a predicate, so an undefined read cannot hide inside a guard, an update, a
//! conjunction, or an implication as a *syntactic* position. The one channel by which
//! a model carries a definedness obligation is a **named predicate whose name ends in
//! [`DEFINED_SUFFIX`]** — whatever built the model, the CML front end or the
//! programmatic API. So the complete set of obligations is the set of such predicates,
//! and [`Definedness::of`] classifies every one of them; none is skipped by position.
//!
//! A definedness predicate may itself be guarded: `I#defined#defined` says where the
//! reads of `I#defined` are defined (cr-pt5h3a). The lowering never writes such a
//! chain, but the programmatic API accepts one, so the classification follows the
//! chain to its **base**, the name left after stripping every trailing
//! [`DEFINED_SUFFIX`] ([`definedness_base`]). Every predicate of a chain is an
//! obligation of the base: where any is false, the base has an undefined read.
//!
//! # Classification
//!
//! A chain guards the declared predicate named by its base when the chain is well
//! formed: the base names a predicate; every intermediate name (`I#defined` under
//! `I#defined#defined`) is a declared predicate, so the chain has no gap; and no name
//! of the chain — the base, an intermediate, or a member's own full name — is an
//! action `X` or the schema `X` of an instance `X(…)`, and none contains `(`, which
//! reads as an instance name whatever actions are declared. Actions are the only
//! namespace besides predicates that a guard can name (variables are never read as
//! guards). Every other
//! chain is an action's. That rule is exact for a lowered
//! model, because CML declarations share one namespace, so no action and no invariant
//! share a name. For a hand-built model, whose actions and predicates have separate
//! namespaces, it is the fail-closed reading: a chain whose base names nothing, whose
//! names include an action's, or which has a gap, is read as an action's, and an
//! undefined action read invalidates every verdict over the model.
//!
//! Within a chain, the deeper predicates guard the shallower ones, so a reader
//! evaluates a chain deepest first ([`Definedness::guards_of`],
//! [`Definedness::action_chains`]) and stops at the first false one.

use std::collections::{BTreeMap, BTreeSet};

use crate::model::Model;

/// The suffix of a definedness predicate: `A#defined` for an action `A`, `I#defined`
/// for an invariant `I` (RFC 0003, "Definedness"). `#` is not a CML identifier
/// character, so no declared name collides with one.
pub const DEFINED_SUFFIX: &str = "#defined";

/// The action or invariant whose definedness predicate `predicate` is, or `None` for
/// any other predicate. A state that violates `X#defined` is the typed outcome
/// "undefined read in `X`": it is not a verdict of a declared invariant, and the
/// verdict of `X` at that state is not a CML verdict.
#[must_use]
pub fn definedness_subject(predicate: &str) -> Option<&str> {
    predicate
        .strip_suffix(DEFINED_SUFFIX)
        .filter(|s| !s.is_empty())
}

/// The base of a definedness chain: `name` with every trailing [`DEFINED_SUFFIX`]
/// stripped while a non-empty name remains, or `None` when `name` is not a definedness
/// predicate. `I#defined#defined` has base `I`; `#defined` is an ordinary name.
#[must_use]
pub fn definedness_base(name: &str) -> Option<&str> {
    let mut base = definedness_subject(name)?;
    while let Some(inner) = definedness_subject(base) {
        base = inner;
    }
    Some(base)
}

/// How many [`DEFINED_SUFFIX`]es [`definedness_base`] strips from `name`.
fn chain_depth(name: &str) -> usize {
    let mut depth: usize = 0;
    let mut here = name;
    while let Some(inner) = definedness_subject(here) {
        depth = depth.saturating_add(1);
        here = inner;
    }
    depth
}

/// What a definedness predicate guards: the base of its chain.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Guarded {
    /// The declared predicate at this index (an invariant `I`, for `I#defined` and any
    /// deeper `I#defined#defined`).
    Predicate(usize),
    /// An action (`A#defined`, and any deeper one), or any chain that is not a
    /// well-formed guard of a predicate by the rule of [`Definedness::of`]: a base that
    /// names no predicate, a gap, or a name of the chain that is also an action's name
    /// or schema or contains `(`.
    Action,
}

/// A model's definedness predicates, classified by the base of their chain.
///
/// Computed once per model in `O((p + a) log (p + a))` for `p` predicates and `a`
/// actions, times the chain depth (at most `MAX_IDENT_BYTES / 8`), with no evaluation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Definedness {
    /// Per predicate index: what it guards, if it is a definedness predicate.
    guards: Vec<Option<Guarded>>,
    /// Per predicate index: its chain depth (0 for an ordinary predicate).
    depth: Vec<usize>,
    /// Per predicate index: the chain of its base, if its base has one (a definedness
    /// predicate, or an ordinary predicate whose name is the base of a chain).
    chain_of: Vec<Option<usize>>,
    /// Every chain, deepest first, then by index.
    chains: Vec<Vec<usize>>,
    /// The action chains, as indices into `chains`, ordered by their shallowest member
    /// (for a lowered model, `A#defined` in predicate order).
    actions: Vec<usize>,
}

impl Definedness {
    /// Classify every definedness predicate of `model`.
    ///
    /// A chain guards the declared predicate named by its base only when the whole
    /// chain is well formed: the base names a predicate; every intermediate name
    /// (`I#defined` under `I#defined#defined`) is itself a declared predicate, so the
    /// chain has no gap; and no name of it — base, intermediate, or any member's own
    /// full name — is an action's name or schema, or contains `(`. Any other chain is
    /// read as an action's (fail closed): always evaluated, never skipped.
    #[must_use]
    pub fn of(model: &Model) -> Self {
        let predicates = model.predicates();
        let by_name: BTreeMap<&str, usize> = predicates
            .iter()
            .enumerate()
            .map(|(index, predicate)| (predicate.name().as_str(), index))
            .collect();
        // The names a prefix may take to be an action's: the action's own name, and
        // the schema name `X` of an instance `X(…)`.
        let action_names: BTreeSet<&str> = model
            .actions()
            .iter()
            .flat_map(|action| {
                let name = action.name().as_str();
                let schema = name.split_once('(').map(|(schema, _)| schema);
                core::iter::once(name).chain(schema)
            })
            .collect();
        let mut depth: Vec<usize> = vec![0; predicates.len()];
        // Chains by base name; a base is an action's unless every member says otherwise.
        let mut by_base: BTreeMap<&str, (Vec<usize>, bool)> = BTreeMap::new();
        for (index, predicate) in predicates.iter().enumerate() {
            let name = predicate.name().as_str();
            let Some(base) = definedness_base(name) else {
                continue;
            };
            if let Some(slot) = depth.get_mut(index) {
                *slot = chain_depth(name);
            }
            // Walk every name of the chain from the member itself down to the base: `name`,
            // `name` stripped once, twice, … Each must be a declared predicate (so the
            // chain has no gap) and none may also be an action's name or schema, at any
            // position, the member's own full name included (cr-pt5h3a).
            let mut well_formed = true;
            let mut here = Some(name);
            while let Some(current) = here {
                // A name with `(` reads as an action instance `X(…)` whatever the
                // declared actions are, so it is never a predicate's guard name.
                if action_names.contains(current)
                    || current.contains('(')
                    || !by_name.contains_key(current)
                {
                    well_formed = false;
                }
                here = definedness_subject(current);
            }
            let entry = by_base.entry(base).or_insert_with(|| (Vec::new(), true));
            entry.0.push(index);
            entry.1 &= well_formed;
        }
        let mut guards: Vec<Option<Guarded>> = vec![None; predicates.len()];
        let mut chain_of: Vec<Option<usize>> = vec![None; predicates.len()];
        let mut chains: Vec<Vec<usize>> = Vec::with_capacity(by_base.len());
        let mut actions: Vec<usize> = Vec::new();
        for (base, (mut members, well_formed)) in by_base {
            members.sort_by_key(|&i| (core::cmp::Reverse(depth.get(i).copied()), i));
            let id = chains.len();
            let target = by_name.get(base).copied().filter(|_| well_formed);
            let guarded = match target {
                Some(target) => {
                    if let Some(slot) = chain_of.get_mut(target) {
                        *slot = Some(id);
                    }
                    Guarded::Predicate(target)
                }
                None => {
                    // A predicate named like the base of an action chain is guarded by
                    // that chain too, so a witness to it respects the chain.
                    if let Some(slot) = by_name.get(base).and_then(|&i| chain_of.get_mut(i)) {
                        *slot = Some(id);
                    }
                    actions.push(id);
                    Guarded::Action
                }
            };
            for &member in &members {
                if let Some(slot) = guards.get_mut(member) {
                    *slot = Some(guarded);
                }
                if let Some(slot) = chain_of.get_mut(member) {
                    *slot = Some(id);
                }
            }
            chains.push(members);
        }
        // Order the action chains by their shallowest member, the predicate order of
        // `A#defined` for a lowered model.
        actions.sort_by_key(|&id| chains.get(id).and_then(|chain| chain.last().copied()));
        Self {
            guards,
            depth,
            chain_of,
            chains,
            actions,
        }
    }

    /// The definedness predicates that guard the reads of the predicate at `index`,
    /// deepest first: for an ordinary predicate `X` that is the base of a chain, the
    /// whole chain (`X#defined`, `X#defined#defined`, …); for a definedness predicate,
    /// the members of its chain strictly deeper than it. Empty when every read is
    /// defined, or when `index` is out of range.
    #[must_use]
    pub fn guards_of(&self, index: usize) -> &[usize] {
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
        // Deepest first, so the members strictly deeper than `own` are a prefix.
        let deeper = chain
            .iter()
            .take_while(|&&i| self.depth.get(i).copied().unwrap_or(0) > own)
            .count();
        chain.get(..deeper).unwrap_or(&[])
    }

    /// What the predicate at `index` guards, or `None` when it is not a definedness
    /// predicate, or `index` is out of range.
    #[must_use]
    pub fn guards(&self, index: usize) -> Option<Guarded> {
        self.guards.get(index).copied().flatten()
    }

    /// The actions' definedness chains, each deepest first, ordered by their
    /// shallowest member's index.
    pub fn action_chains(&self) -> impl Iterator<Item = &[usize]> {
        self.actions
            .iter()
            .filter_map(|&id| self.chains.get(id).map(Vec::as_slice))
    }

    /// Whether the model declares no definedness predicate at all.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.chains.is_empty()
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing)]
mod tests {
    use super::*;

    #[test]
    fn the_subject_strips_exactly_the_suffix() {
        assert_eq!(definedness_subject("Choose#defined"), Some("Choose"));
        assert_eq!(definedness_subject("#defined"), None);
        assert_eq!(definedness_subject("Choose"), None);
        assert_eq!(definedness_subject("I#defined#defined"), Some("I#defined"));
    }

    #[test]
    fn the_base_strips_every_suffix() {
        assert_eq!(definedness_base("I#defined#defined"), Some("I"));
        assert_eq!(definedness_base("I#defined"), Some("I"));
        assert_eq!(definedness_base("#defined#defined"), Some("#defined"));
        assert_eq!(definedness_base("#defined"), None);
        assert_eq!(definedness_base("I"), None);
        assert_eq!(chain_depth("I#defined#defined"), 2);
    }

    fn model(actions: &[&str], predicates: &[&str]) -> Model {
        use crate::expr::BoolExpr;
        use crate::model::{ActionDecl, ModelBuilder};
        let mut b = ModelBuilder::new().variable("x", 0, 1);
        for a in actions {
            b = b.action(ActionDecl::deterministic(
                a,
                BoolExpr::constant(true),
                vec![],
            ));
        }
        for p in predicates {
            b = b.predicate(p, BoolExpr::constant(true));
        }
        b.initial_state(&[("x", 0)]).build().unwrap()
    }

    #[test]
    fn a_well_formed_chain_guards_its_base_deepest_first() {
        let m = model(
            &["A"],
            &["I", "I#defined", "I#defined#defined", "A#defined"],
        );
        let d = Definedness::of(&m);
        let at = |n: &str| m.predicate_index(n).unwrap();
        assert_eq!(
            d.guards_of(at("I")),
            &[at("I#defined#defined"), at("I#defined")]
        );
        assert_eq!(d.guards_of(at("I#defined")), &[at("I#defined#defined")]);
        assert_eq!(d.guards_of(at("I#defined#defined")), &[] as &[usize]);
        assert_eq!(
            d.guards(at("I#defined#defined")),
            Some(Guarded::Predicate(at("I")))
        );
        assert_eq!(
            d.action_chains().collect::<Vec<_>>(),
            vec![&[at("A#defined")][..]]
        );
    }

    #[test]
    fn a_gap_or_an_action_name_in_a_chain_reads_it_as_an_action() {
        let m = model(&["Step"], &["I", "I#defined#defined"]);
        let d = Definedness::of(&m);
        let g = m.predicate_index("I#defined#defined").unwrap();
        assert_eq!(d.guards(g), Some(Guarded::Action));
        assert_eq!(d.guards_of(m.predicate_index("I").unwrap()), &[g]);

        let m = model(&["I#defined"], &["I", "I#defined#defined"]);
        let d = Definedness::of(&m);
        let g = m.predicate_index("I#defined#defined").unwrap();
        assert_eq!(d.guards(g), Some(Guarded::Action));
    }

    #[test]
    fn a_collision_at_any_chain_name_reads_the_chain_as_an_action() {
        // (action, a false member): the base, the member's own full name at depth 1 and
        // 2, an intermediate, and schema instances of each.
        for action in [
            "I",
            "I#defined",
            "I#defined#defined",
            "I(k=0)",
            "I#defined(k=0)",
            "I#defined#defined(k=0)",
        ] {
            let m = model(&[action], &["I", "I#defined", "I#defined#defined"]);
            let d = Definedness::of(&m);
            for member in ["I#defined", "I#defined#defined"] {
                let g = m.predicate_index(member).unwrap();
                assert_eq!(d.guards(g), Some(Guarded::Action), "{action} {member}");
            }
            assert_eq!(d.action_chains().count(), 1, "{action}");
        }
    }

    #[test]
    fn a_chain_name_that_reads_as_an_instance_fails_closed() {
        // `A(k=0)#defined` reads as the definedness of instance `A(k=0)`, whether the
        // declared action is `A`, another instance of `A`, or a nested `P(a)(b)`.
        for (action, base) in [("A", "A(k=0)"), ("A(k=1)", "A(k=0)"), ("P(a)(b)", "P(a)")] {
            let guard = format!("{base}#defined");
            let m = model(&[action], &[base, &guard]);
            let d = Definedness::of(&m);
            let g = m.predicate_index(&guard).unwrap();
            assert_eq!(d.guards(g), Some(Guarded::Action), "{action} {base}");
        }
    }

    #[test]
    fn action_chains_are_ordered_by_their_shallowest_member() {
        let m = model(
            &["A", "B"],
            &[
                "B#defined",
                "A#defined#defined",
                "A#defined",
                "B#defined#defined",
            ],
        );
        let d = Definedness::of(&m);
        let at = |n: &str| m.predicate_index(n).unwrap();
        let chains: Vec<&[usize]> = d.action_chains().collect();
        assert_eq!(
            chains,
            vec![
                &[at("A#defined#defined"), at("A#defined")][..],
                &[at("B#defined#defined"), at("B#defined")][..],
            ]
        );
    }
}
