//! Fairness lowering (bn-1ln12; RFC 0003 correction 5, RFC 0008 "Fairness scopes",
//! RFC 0015 "Fairness", docs/02 §8).
//!
//! # What is pinned
//!
//! - **the scope**: `fairness s X` is one model-core assumption of strength `s` over
//!   every programmatic action `X` lowers to — each parameter instance and each
//!   relational candidate, and for a choice every action it offers — never one per
//!   instance. Checked against names written out by hand and against the model's own
//!   action list, filtered independently;
//! - **identity**: the lowered model equals the programmatic model written by hand with
//!   the same assumptions (one `Model::identity`), and a stable reordering of the
//!   fairness declarations (and a repeat) keeps it; changing a strength, a target, or
//!   the split of a scope changes it (docs/19 §3's expected non-preservation);
//! - **resources**: the fairness sites spend exactly their plans (the lowering replays
//!   at its reported usage and is refused one unit or node below), and the model-core
//!   limit on assumptions is refused before any scope is built.

use continuum_cml_elab::{
    Limits, LowerErrorKind, Unlowerable, elaborate_source, lower, lower_with,
};
use continuum_model_core::fairness::MAX_FAIRNESS;
use continuum_model_core::model::Symbol;
use continuum_model_core::{ActionDecl, IntExpr, Model, ModelBuilder, ModelError, Strength};

/// `Inc` is a schema over a Boolean and an enumeration (four instances), `Rel` a
/// relational action with parameters (four candidates), `Reset` a plain action, and
/// `Next` a choice of `Inc` and `Reset`.
const BASE: &str = "module F
enum Mode { Lo, Hi }
state {
  x: Nat where x <= 2
  y: Nat where y <= 1
}
init { x == 0 && y == 0 }
action Inc(b: Bool, m: Mode) {
  require x < 2
  next x = x + 1
  unchanged y
}
action Rel(b: Bool) {
  y' >= 0
  unchanged x
}
action Reset {
  require x == 2
  next x = 0
  unchanged y
}
action Next = Inc | Reset
";

fn lowered(fairness: &str) -> Model {
    let src = format!("{BASE}{fairness}\n");
    lower(&elaborate_source(&src).unwrap_or_else(|e| panic!("elaborates: {e}")))
        .unwrap_or_else(|e| panic!("lowers: {e}"))
}

const INC: [&str; 4] = [
    "Inc(b=false,m=Lo)",
    "Inc(b=false,m=Hi)",
    "Inc(b=true,m=Lo)",
    "Inc(b=true,m=Hi)",
];
const REL: [&str; 4] = [
    "Rel(b=false)[y=0]",
    "Rel(b=false)[y=1]",
    "Rel(b=true)[y=0]",
    "Rel(b=true)[y=1]",
];

/// The model's assumptions as `(strength, scope names)`, scope names sorted.
fn assumptions(model: &Model) -> Vec<(Strength, Vec<String>)> {
    model
        .fairness()
        .iter()
        .map(|f| {
            let mut names: Vec<String> = f
                .actions()
                .iter()
                .map(|i| model.actions()[*i].name().as_str().to_owned())
                .collect();
            names.sort();
            (f.strength(), names)
        })
        .collect()
}

fn sorted(names: &[&str]) -> Vec<String> {
    let mut v: Vec<String> = names.iter().map(|s| (*s).to_owned()).collect();
    v.sort();
    v
}

/// The actions whose name starts with one of `prefixes`, found in the model's own
/// action list: an independent reading of "every action `X` lowers to".
fn by_prefix(model: &Model, prefixes: &[&str]) -> Vec<String> {
    let mut v: Vec<String> = model
        .actions()
        .iter()
        .map(|a| a.name().as_str().to_owned())
        .filter(|n| prefixes.iter().any(|p| n.starts_with(p)))
        .collect();
    v.sort();
    v
}

/// `model` rebuilt through `ModelBuilder` with its fairness replaced by `fairness`.
fn by_hand(model: &Model, fairness: &[(Strength, Vec<&str>)]) -> Model {
    let mut b = ModelBuilder::new();
    for v in model.variables() {
        b = b.variable(v.name().as_str(), v.domain().lo(), v.domain().hi());
    }
    for a in model.actions() {
        let outcomes: Vec<Vec<(&str, IntExpr)>> = a
            .outcomes()
            .iter()
            .map(|o| {
                o.assignments()
                    .iter()
                    .map(|x| (x.variable().as_str(), x.value().clone()))
                    .collect()
            })
            .collect();
        b = b.action(ActionDecl::enumerated(
            a.name().as_str(),
            a.guard().clone(),
            outcomes,
        ));
    }
    for s in model.initial_states() {
        let bindings: Vec<(&str, i64)> = model
            .variables()
            .iter()
            .zip(s.as_slice())
            .map(|(v, x)| (v.name().as_str(), *x))
            .collect();
        b = b.initial_state(&bindings);
    }
    for p in model.predicates() {
        b = b.predicate(p.name().as_str(), p.body().clone());
    }
    for (strength, scope) in fairness {
        b = b.fairness(*strength, scope.iter().copied());
    }
    b.build().expect("valid")
}

#[test]
fn a_schema_is_one_assumption_over_all_its_instances() {
    let model = lowered("fairness weak Inc");
    assert_eq!(assumptions(&model), vec![(Strength::Weak, sorted(&INC))]);
    assert_eq!(assumptions(&model)[0].1, by_prefix(&model, &["Inc("]));
}

#[test]
fn a_relational_action_covers_every_candidate_of_every_instance() {
    let model = lowered("fairness strong Rel");
    assert_eq!(assumptions(&model), vec![(Strength::Strong, sorted(&REL))]);
    assert_eq!(assumptions(&model)[0].1, by_prefix(&model, &["Rel("]));
}

#[test]
fn a_choice_is_one_assumption_over_the_union_of_its_actions() {
    let model = lowered("fairness weak Next");
    let mut union: Vec<&str> = INC.to_vec();
    union.push("Reset");
    assert_eq!(assumptions(&model), vec![(Strength::Weak, sorted(&union))]);
    assert_eq!(
        assumptions(&model)[0].1,
        by_prefix(&model, &["Inc(", "Reset"])
    );
}

/// `fairness weak A, B` is two assumptions (one per name), as two lines are; a choice
/// is one. Both strengths of one target are two assumptions.
#[test]
fn a_list_is_several_assumptions_and_strengths_are_separate() {
    let model = lowered("fairness weak Inc, Reset\nfairness strong Inc");
    assert_eq!(
        assumptions(&model),
        vec![
            (Strength::Weak, sorted(&INC)),
            (Strength::Weak, sorted(&["Reset"])),
            (Strength::Strong, sorted(&INC)),
        ]
    );
}

/// The lowered model is the programmatic model with the same assumptions written by
/// hand, and a stable reordering of declarations — here the fairness lines, reversed
/// and one repeated — keeps the identity.
#[test]
fn the_lowered_fairness_is_the_hand_written_one_and_reordering_keeps_it() {
    let text = "fairness weak Next\nfairness strong Rel\nfairness weak Reset";
    let model = lowered(text);
    let mut next: Vec<&str> = INC.to_vec();
    next.push("Reset");
    let hand = by_hand(
        &model,
        &[
            (Strength::Strong, REL.to_vec()),
            (Strength::Weak, vec!["Reset"]),
            (Strength::Weak, next),
        ],
    );
    assert_eq!(hand, model);
    assert_eq!(hand.identity(), model.identity());

    // stable reordering of declarations (docs/19 §3): reversed, with a repeat.
    let reordered = lowered(
        "fairness weak Reset\nfairness weak Reset\nfairness strong Rel\nfairness weak Next",
    );
    assert_eq!(reordered.identity(), model.identity());
}

/// Expected non-preservation (docs/19 §3): each change to the fairness changes the
/// identity, and the transition system stays the same.
#[test]
fn changing_the_fairness_changes_the_identity() {
    let variants = [
        "",
        "fairness weak Inc",
        "fairness strong Inc",
        "fairness weak Next",
        "fairness weak Inc\nfairness weak Reset",
        "fairness weak Rel",
    ];
    let models: Vec<Model> = variants.iter().map(|v| lowered(v)).collect();
    for (i, a) in models.iter().enumerate() {
        assert_eq!(a.actions(), models[0].actions());
        for b in models.iter().skip(i + 1) {
            assert_ne!(a.identity(), b.identity());
        }
    }
    // The per-instance reading is a different model from the schema's.
    let per_instance = by_hand(
        &models[1],
        &INC.iter()
            .map(|n| (Strength::Weak, vec![*n]))
            .collect::<Vec<_>>(),
    );
    assert_ne!(per_instance.identity(), models[1].identity());
}

/// The fairness sites spend exactly their plans: the lowering replays at the usage it
/// reported, and one output node or one unit of work less is refused, typed.
#[test]
fn fairness_lowers_at_its_measured_limits() {
    let src = format!("{BASE}fairness weak Next\nfairness strong Rel, Inc\n");
    let m = elaborate_source(&src).expect("elaborates");
    let (result, used) = lower_with(&m, Limits::default());
    result.expect("lowers");
    let at = |nodes: usize, work: u64| {
        lower_with(&m, Limits { nodes, work })
            .0
            .map(|_| ())
            .map_err(|e| e.kind)
    };
    assert_eq!(at(used.nodes, used.work), Ok(()));
    assert_eq!(
        at(used.nodes - 1, used.work),
        Err(LowerErrorKind::Unlowerable(Unlowerable::OutputTooLarge))
    );
    assert_eq!(
        at(used.nodes, used.work - 1),
        Err(LowerErrorKind::Unlowerable(Unlowerable::WorkLimitExceeded))
    );
    // The fairness costs something: the same model without it spends less.
    let (_, plain) = lower_with(
        &elaborate_source(BASE).expect("elaborates"),
        Limits::default(),
    );
    assert!(plain.nodes < used.nodes && plain.work < used.work);
}

/// More assumptions than the model core admits is the model core's own refusal, made
/// in the preflight, before any scope is built; exactly the limit lowers.
#[test]
fn too_many_assumptions_are_refused_before_any_scope_is_built() {
    let source = |choices: usize| {
        let mut s = String::from(
            "module M\nstate { x: Nat where x <= 1 }\ninit { x == 0 }\naction A { next x = 1 - x }\n",
        );
        for i in 0..choices {
            s.push_str(&format!("action C{i} = A\nfairness weak C{i}\n"));
        }
        s
    };
    let over = elaborate_source(&source(MAX_FAIRNESS + 1)).expect("elaborates");
    let err = lower(&over).expect_err("too many");
    assert_eq!(
        err.kind,
        LowerErrorKind::Model(ModelError::TooMany {
            symbol: Symbol::Fairness,
            count: MAX_FAIRNESS + 1,
            max: MAX_FAIRNESS,
        })
    );
    let at = elaborate_source(&source(MAX_FAIRNESS)).expect("elaborates");
    let model = lower(&at).expect("at the limit");
    // Every choice offers only `A`: the assumptions are one assumption.
    assert_eq!(model.fairness().len(), 1);
}

/// A hand-built normalized model can bypass elaboration: a fairness target that names
/// nothing, and a choice that offers nothing, are the model core's own typed refusals,
/// located at the fairness declaration, never a silent drop.
#[test]
fn hand_built_fairness_targets_fail_closed_at_the_declaration() {
    let src = format!("{BASE}fairness weak Next\n");
    let good = elaborate_source(&src).expect("elaborates");
    let at = good.fairness[0].span;

    let mut empty = good.clone();
    empty.choices[0].actions.clear();
    let err = lower(&empty).expect_err("an empty choice");
    assert_eq!(
        err.kind,
        LowerErrorKind::Model(ModelError::EmptyFairness { index: 0 })
    );
    assert_eq!(err.span, at);

    let mut nowhere = good.clone();
    nowhere.fairness[0].action = "Missing".to_owned();
    let err = lower(&nowhere).expect_err("an unknown target");
    assert!(matches!(
        err.kind,
        LowerErrorKind::Model(ModelError::UnknownAction { fairness: 0, .. })
    ));
    assert_eq!(err.span, at);

    let mut stray = good;
    stray.choices[0].actions.push("Missing".to_owned());
    assert!(matches!(
        lower(&stray)
            .expect_err("a choice offering an unknown action")
            .kind,
        LowerErrorKind::Model(ModelError::UnknownAction { fairness: 0, .. })
    ));
}
