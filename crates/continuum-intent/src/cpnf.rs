//! CPNF-1: the Continuum Property Normal Form, version 1 (PR-4 / IMPL-01).
//!
//! # What normalization buys
//!
//! > **S1 — Soundness.** Every rule N1–N7 is meaning-preserving, so
//! > `normalize(P) = normalize(Q)` implies `P` and `Q` denote the same set of
//! > behaviors. RFC 0031 MAY classify a property change `unchanged` on exactly this
//! > equality and on nothing weaker.
//! >
//! > — `notes/plan/rfcs/0037-intent-contract.md`, "Soundness of the classification built on CPNF-1"
//!
//! That is the whole load-bearing statement. A property is refactored constantly —
//! a response written as `leads_to` one day and spelled out the next, a bound
//! variable renamed, two conjuncts swapped — and none of those may look like an
//! intent change, or every honest edit becomes a privileged revision and the
//! protection is abandoned as noise. Symmetrically, a *weakening* dressed as a
//! refactor must not reach `unchanged`, or INV-001 is defeated by formatting. N1–N7
//! draw that line, and N8 makes it a byte comparison.
//!
//! > **S2 — Incompleteness is the safe side.** CPNF-1 is sound but NOT complete
//! > […] A false `unknown` costs a review; a false `unchanged` costs the invariant.
//!
//! So this module implements exactly the rules RFC 0037 enumerates and no more. A
//! rewrite that is *valid* but unlisted would still be a defect here: ID5 requires
//! the encoding to be "byte-deterministic across platforms and releases within a
//! schema version", and two conforming implementations that disagree about which
//! tautologies to recognize produce two identities for one contract. Every
//! temptation declined is recorded below.
//!
//! # The pipeline
//!
//! RFC 0037: "Rules N1–N7 are applied bottom-up and re-applied to fixpoint; N8 then
//! encodes the result." [`normalize`] is that sentence:
//!
//! 1. [`Formula::validate`] — a shape the schema does not admit gets a typed error
//!    before any rewriting assigns it a meaning.
//! 2. `alpha_rename` (N5) — every bound variable becomes `v<d>` for its de Bruijn
//!    *level*.
//! 3. `rewrite` — one bottom-up pass applying N1, N2, N3, N4, N6, and N7. Children
//!    are normalized before their parent, so N4's ordering compares operands that
//!    are themselves already in normal form.
//! 4. Repeat 2–3 until the result stops changing, because N7 can *delete* a binder
//!    (`forall b. true` → `true`), which shifts the de Bruijn levels of every binder
//!    inside it and therefore changes what N4 sorts by. Convergence is checked, not
//!    assumed: [`MAX_ROUNDS`] rounds without a fixpoint is
//!    [`NormalizeError::NotConverged`], never a silent partial normalization.
//!
//! Step 2 before step 3 and again after is the only subtle part, and the RFC
//! anticipates the interaction it avoids: "De Bruijn levels depend only on binder
//! nesting, never on sibling order, so N5 and N4 do not interfere."
//!
//! # Recorded readings, where the RFC leaves latitude
//!
//! **A free `var` is rejected, not renamed.** W5 allows a `var` to be "a declared
//! free variable of the model", and N5 says "Free variables keep their declared
//! names." Those two together are unsound under N5's fixed naming scheme: in
//! `forall x. p(x, v0)` with `v0` free, renaming `x` to `v0` captures the free
//! occurrence and changes the property's meaning. Capture-avoidance would need a
//! rule N5 does not state, and inventing one would make this implementation's normal
//! form differ from a conforming reading of the RFC. So [`normalize`] returns
//! [`NormalizeError::UnboundVariable`]. This is also the schema's own reading —
//! `$defs/term_var` is "Reference to a variable bound by an enclosing quantifier" —
//! and under INV-003 the schema decides shape. A model's free names are spelled
//! `state` or `constant` terms, which N5 never touches.
//! [`Formula::free_variables`] is provided for the model-aware W5 check.
//! *Raised as a flag against RFC 0037: N5 needs either a capture-avoiding rename or
//! an explicit statement that a `var` is always bound.*
//!
//! **`not` above a boolean constant folds.** N2 says that after the negation push
//! `not` occurs "only directly above a `boolean`, `predicate`, `action`, or
//! `compare` atom", which reads as permitting `not(boolean(true))` to survive. It is
//! folded here instead, because leaving it gives one constant two spellings — which
//! is exactly what N9 forbids — and because an unfolded `not(true)` blocks N7's unit
//! law from ever firing on the junction containing it. *Raised as a flag against RFC
//! 0037: N2's atom list should exclude `boolean`, or N7 should name the fold.*
//!
//! **Everything else is declined.** Classic Boolean absorption (`a ∧ (a ∨ b) = a`),
//! `always(false) → false`, `eventually(true) → true`, complementary-literal
//! collapse, quantifier reordering, and `apply`-argument reordering are all valid and
//! all *absent*, because N7's enumeration is read as closed and S2 says an
//! unrecognized equivalence costs a review rather than an invariant. RFC 0037's open
//! questions already name the `apply` case ("whether the v1 term sort grows a
//! declared-algebraic-law table").
//!
//! # Seams left open on purpose
//!
//! - **No direction.** This module decides *equality* of normal forms. `strengthened`
//!   and `weakened` need the fragment's decision procedure and belong to RFC 0031's
//!   classifier (PR 12), which S3 forbids from taking a direction out of CPNF-1.
//! - **One fragment.** N1–N8 are meaning-preserving "in every fragment that
//!   interprets these operators standardly" (S3), so identity and `unchanged` use
//!   this form everywhere; per-fragment normalization for `Symbolic`, `Temporal`,
//!   and `Probabilistic` is an RFC 0037 open question and is not guessed at here.
//! - **`cpnf-2` is an epoch advance, not an edit.** RFC 0037: a new normal form
//!   "changes the canonical encoding and therefore the `in_*` identity of every
//!   stored contract, so it MUST NOT be applied in place". [`CPNF_VERSION`] is the
//!   token an artifact declares; changing it here without the epoch machinery would
//!   silently re-identify every contract.

use core::fmt;

use crate::ast::{AstError, Binder, ComparisonOperator, Formula, Identifier, Term};

/// The normal-form token an expression declares in `expression.normal_form`.
///
/// `$defs/property_expression` fixes it as the `const` `"cpnf-1"`.
pub const CPNF_VERSION: &str = "cpnf-1";

/// How many rename/rewrite rounds [`normalize`] runs before giving up.
///
/// Two rounds suffice for every formula in the acceptance corpus — the second only
/// confirms the fixpoint — and the margin is for a shape nobody has written yet. A
/// bound that is reached is a typed error rather than a hang, because a normalizer
/// that does not terminate is a denial of service on the acceptance path.
pub const MAX_ROUNDS: usize = 16;

/// How many nodes a normalized formula may have.
///
/// N1's `iff` expansion duplicates both sides, so nested bi-implications grow
/// exponentially. The RFC states the rewrite without a bound; the bound is here so
/// an adversarial contract is a typed rejection instead of an allocation failure.
///
/// It is checked *before* the rewrite as well as after. [`expansion_bound`] computes
/// an upper bound on the post-N1 size directly from the authored AST, so a formula
/// that would blow up is rejected in time linear in its own size — a bound that is
/// only checked afterwards is not a bound, it is a post-mortem.
pub const MAX_NODES: usize = 1 << 16;

/// Normalize a formula to CPNF-1.
///
/// Total on well-formed ASTs in the fragment, in the sense RFC 0037 asks for:
/// `normalize` never returns a formula outside CPNF-1, and the only failures are
/// inputs the schema does not admit, a free `var` (see the module documentation),
/// and the two resource bounds.
///
/// # Errors
///
/// - [`NormalizeError::Ast`] — the input is not a shape `$defs/formula` admits.
/// - [`NormalizeError::UnboundVariable`] — a `var` node no enclosing binder declares.
/// - [`NormalizeError::TooLarge`] — the rewrite exceeded [`MAX_NODES`].
/// - [`NormalizeError::NotConverged`] — no fixpoint within [`MAX_ROUNDS`].
pub fn normalize(formula: &Formula) -> Result<Formula, NormalizeError> {
    formula.validate()?;
    if expansion_bound(formula) > MAX_NODES {
        return Err(NormalizeError::TooLarge { max: MAX_NODES });
    }
    let mut current = round(formula)?;
    for _ in 1..MAX_ROUNDS {
        let next = round(&current)?;
        if next == current {
            return Ok(next);
        }
        current = next;
    }
    Err(NormalizeError::NotConverged { rounds: MAX_ROUNDS })
}

fn round(formula: &Formula) -> Result<Formula, NormalizeError> {
    let renamed = alpha_rename(formula)?;
    let rewritten = rewrite(&renamed);
    if count_nodes(&rewritten) > MAX_NODES {
        return Err(NormalizeError::TooLarge { max: MAX_NODES });
    }
    Ok(rewritten)
}

/// The N8 canonical encoding of a formula.
///
/// > **N8 — Canonical encoding.** The normalized AST MUST be encoded as JSON with
/// > object keys sorted ascending by Unicode code point, no insignificant
/// > whitespace, UTF-8 output, integers in shortest decimal form without sign or
/// > leading zeros, and no floating-point values. `source`, `normal_form`,
/// > `$comment`, and every label or comment field MUST be excluded from the
/// > encoding. This encoding is what the `in_*` identity above hashes and what N4
/// > and N6 order by.
/// >
/// > — RFC 0037
///
/// The exclusion list is met by the type system: [`Formula`] has no `source`, no
/// `normal_form`, and no comment field to exclude. Those live one level up, on
/// `expression`, and [`crate::property::PropertyExpression`] is where they are
/// dropped from the preimage.
#[must_use]
pub fn encode(formula: &Formula) -> Vec<u8> {
    formula.to_json().to_canonical_bytes()
}

/// Whether a formula is already in CPNF-1.
///
/// This is the check `$defs/property_expression`'s `normal_form` declaration
/// requires: "Checkers MUST verify by re-normalizing (normalization is idempotent)
/// and MUST reject a false assertion."
///
/// # Errors
///
/// Whatever [`normalize`] returns for this formula.
pub fn is_normal(formula: &Formula) -> Result<bool, NormalizeError> {
    Ok(&normalize(formula)? == formula)
}

// --- N5: binder canonicalization -----------------------------------------------------------

/// Rename every bound variable to `v<d>`, `d` its de Bruijn *level*.
///
/// > **N5 — Binder canonicalization.** Each bound variable MUST be renamed to
/// > `v<d>`, where `d` is the number of binders enclosing its own binder (a de
/// > Bruijn level), and every `var` reference MUST be updated.
/// >
/// > — RFC 0037
///
/// A *level* counts enclosing binders, not distance to the binder, which is why it
/// is stable under N4's reordering of siblings: two conjuncts that swap places keep
/// the same enclosing-binder count.
fn alpha_rename(formula: &Formula) -> Result<Formula, NormalizeError> {
    rename_formula(formula, &mut Vec::new())
}

/// `scope` is the stack of `(original name, canonical name)` pairs, innermost last.
fn rename_formula(
    formula: &Formula,
    scope: &mut Vec<(Identifier, Identifier)>,
) -> Result<Formula, NormalizeError> {
    Ok(match formula {
        Formula::Boolean { .. } | Formula::Action { .. } => formula.clone(),
        Formula::Predicate { name, args } => Formula::Predicate {
            name: name.clone(),
            args: rename_terms(args, scope)?,
        },
        Formula::Compare { op, left, right } => Formula::Compare {
            op: *op,
            left: rename_term(left, scope)?,
            right: rename_term(right, scope)?,
        },
        Formula::Not { operand } => Formula::not(rename_formula(operand, scope)?),
        Formula::Always { operand } => Formula::always(rename_formula(operand, scope)?),
        Formula::Eventually { operand } => Formula::eventually(rename_formula(operand, scope)?),
        Formula::And { operands } => Formula::And {
            operands: rename_formulas(operands, scope)?,
        },
        Formula::Or { operands } => Formula::Or {
            operands: rename_formulas(operands, scope)?,
        },
        Formula::Implies {
            antecedent,
            consequent,
        } => Formula::implies(
            rename_formula(antecedent, scope)?,
            rename_formula(consequent, scope)?,
        ),
        Formula::LeadsTo {
            antecedent,
            consequent,
        } => Formula::leads_to(
            rename_formula(antecedent, scope)?,
            rename_formula(consequent, scope)?,
        ),
        Formula::Iff { left, right } => {
            Formula::iff(rename_formula(left, scope)?, rename_formula(right, scope)?)
        }
        Formula::Forall { binder, body } => {
            let (binder, body) = rename_binder(binder, body, scope)?;
            Formula::forall(binder, body)
        }
        Formula::Exists { binder, body } => {
            let (binder, body) = rename_binder(binder, body, scope)?;
            Formula::exists(binder, body)
        }
    })
}

fn rename_binder(
    binder: &Binder,
    body: &Formula,
    scope: &mut Vec<(Identifier, Identifier)>,
) -> Result<(Binder, Formula), NormalizeError> {
    // The domain is evaluated outside the binder's own scope, so it is renamed
    // before the binding is pushed.
    let domain = rename_term(&binder.domain, scope)?;
    let level = scope.len();
    let canonical = Identifier::new(&format!("v{level}")).map_err(NormalizeError::Ast)?;
    scope.push((binder.variable.clone(), canonical.clone()));
    let body = rename_formula(body, scope);
    scope.pop();
    Ok((Binder::new(canonical, domain), body?))
}

fn rename_formulas(
    formulas: &[Formula],
    scope: &mut Vec<(Identifier, Identifier)>,
) -> Result<Vec<Formula>, NormalizeError> {
    formulas
        .iter()
        .map(|formula| rename_formula(formula, scope))
        .collect()
}

fn rename_terms(
    terms: &[Term],
    scope: &mut Vec<(Identifier, Identifier)>,
) -> Result<Vec<Term>, NormalizeError> {
    terms.iter().map(|term| rename_term(term, scope)).collect()
}

fn rename_term(
    term: &Term,
    scope: &mut Vec<(Identifier, Identifier)>,
) -> Result<Term, NormalizeError> {
    Ok(match term {
        Term::Var { name } => {
            // Innermost binding wins: shadowing is resolved by searching the scope
            // stack from the top.
            let bound = scope
                .iter()
                .rev()
                .find(|(original, _)| original == name)
                .map(|(_, canonical)| canonical.clone());
            match bound {
                Some(canonical) => Term::Var { name: canonical },
                None => {
                    return Err(NormalizeError::UnboundVariable {
                        name: name.to_string(),
                    });
                }
            }
        }
        Term::Literal { .. } | Term::Constant { .. } => term.clone(),
        Term::State { name, indices } => Term::State {
            name: name.clone(),
            indices: rename_terms(indices, scope)?,
        },
        Term::Apply { operator, args } => Term::Apply {
            operator: operator.clone(),
            args: rename_terms(args, scope)?,
        },
    })
}

// --- N1, N2, N3, N4, N6, N7 ----------------------------------------------------------------

/// One bottom-up rewriting pass. Children are rewritten before their parent.
fn rewrite(formula: &Formula) -> Formula {
    match formula {
        Formula::Boolean { .. } | Formula::Action { .. } | Formula::Predicate { .. } => {
            formula.clone()
        }
        Formula::Compare { op, left, right } => canonical_comparison(*op, left, right),
        // N1: derived-operator elimination. Each rewrite is applied to the *authored*
        // children and the result re-entered, so the eliminated operator can never
        // survive into the output.
        Formula::Implies {
            antecedent,
            consequent,
        } => rewrite(&Formula::Or {
            operands: vec![Formula::not((**antecedent).clone()), (**consequent).clone()],
        }),
        Formula::Iff { left, right } => rewrite(&Formula::And {
            operands: vec![
                Formula::Or {
                    operands: vec![Formula::not((**left).clone()), (**right).clone()],
                },
                Formula::Or {
                    operands: vec![(**left).clone(), Formula::not((**right).clone())],
                },
            ],
        }),
        Formula::LeadsTo {
            antecedent,
            consequent,
        } => rewrite(&Formula::always(Formula::Or {
            operands: vec![
                Formula::not((**antecedent).clone()),
                Formula::eventually((**consequent).clone()),
            ],
        })),
        Formula::Not { operand } => push_negation(operand),
        Formula::And { operands } => junction(true, operands),
        Formula::Or { operands } => junction(false, operands),
        Formula::Always { operand } => temporal(true, rewrite(operand)),
        Formula::Eventually { operand } => temporal(false, rewrite(operand)),
        Formula::Forall { binder, body } => quantifier(true, binder, rewrite(body)),
        Formula::Exists { binder, body } => quantifier(false, binder, rewrite(body)),
    }
}

/// N2 — negation-normal form. `not` is pushed inward to the atoms.
///
/// The `boolean` case folds rather than surviving; see the module documentation's
/// recorded readings.
fn push_negation(operand: &Formula) -> Formula {
    match operand {
        Formula::Boolean { value } => Formula::boolean(!value),
        Formula::Predicate { .. } | Formula::Action { .. } => Formula::not(rewrite(operand)),
        Formula::Compare { op, left, right } => negated_comparison(*op, left, right),
        // `not not φ` → `φ`.
        Formula::Not { operand } => rewrite(operand),
        // De Morgan.
        Formula::And { operands } => junction(
            false,
            &operands
                .iter()
                .map(|inner| Formula::not(inner.clone()))
                .collect::<Vec<_>>(),
        ),
        Formula::Or { operands } => junction(
            true,
            &operands
                .iter()
                .map(|inner| Formula::not(inner.clone()))
                .collect::<Vec<_>>(),
        ),
        // `not always φ` → `eventually not φ`, and dually.
        Formula::Always { operand } => temporal(false, rewrite(&Formula::not((**operand).clone()))),
        Formula::Eventually { operand } => {
            temporal(true, rewrite(&Formula::not((**operand).clone())))
        }
        // `not forall b. φ` → `exists b. not φ`, and dually.
        Formula::Forall { binder, body } => {
            quantifier(false, binder, rewrite(&Formula::not((**body).clone())))
        }
        Formula::Exists { binder, body } => {
            quantifier(true, binder, rewrite(&Formula::not((**body).clone())))
        }
        // The derived operators are eliminated first (N1), then negated (N2). Doing
        // it in this order means no De Morgan rule has to be stated for them.
        Formula::Implies { .. } | Formula::Iff { .. } | Formula::LeadsTo { .. } => {
            push_negation(&rewrite(operand))
        }
    }
}

/// N6 — comparison canonicalization, on a comparison with no `not` above it.
fn canonical_comparison(op: ComparisonOperator, left: &Term, right: &Term) -> Formula {
    // `gt(a, b)` → `lt(b, a)`; `ge(a, b)` → `le(b, a)`.
    let (op, left, right) = match op.swapped() {
        Some(swapped) => (swapped, right.clone(), left.clone()),
        None => (op, left.clone(), right.clone()),
    };
    // The operands of the commutative `eq` and `ne` are ordered by their N8
    // encodings — the same ordering key N4 uses, so there is one comparison rule in
    // the whole normal form, not two.
    if op.is_commutative() && term_key(&right) < term_key(&left) {
        return Formula::compare(op, right, left);
    }
    Formula::compare(op, left, right)
}

/// N6 — a `not` above a comparison.
fn negated_comparison(op: ComparisonOperator, left: &Term, right: &Term) -> Formula {
    // `gt`/`ge` are eliminated first so the negation table only has to cover the six
    // surviving operators.
    let (op, left, right) = match op.swapped() {
        Some(swapped) => (swapped, right.clone(), left.clone()),
        None => (op, left.clone(), right.clone()),
    };
    match op.negated() {
        Some(complement) => canonical_comparison(complement, &left, &right),
        // `lt`/`le` without a declared total order, and `member`/`subset` always:
        // the `not` remains.
        None => Formula::not(canonical_comparison(op, &left, &right)),
    }
}

/// N3, N4, and N7 for one junction. `conjunction` selects `and` over `or`.
fn junction(conjunction: bool, operands: &[Formula]) -> Formula {
    // The unit of the junction (`true` for `and`, `false` for `or`) and its
    // annihilator (the other one).
    let unit = conjunction;
    let mut flat: Vec<Formula> = Vec::with_capacity(operands.len());
    for operand in operands {
        let operand = rewrite(operand);
        match &operand {
            // N7 — `and` containing `false` → `false`; `or` dually.
            Formula::Boolean { value } if *value != unit => return Formula::boolean(!unit),
            // N7 — `true` operands dropped from `and`; `or` dually.
            Formula::Boolean { .. } => {}
            // N3 — a nested same-kind junction is spliced into the parent.
            Formula::And { operands } if conjunction => flat.extend(operands.iter().cloned()),
            Formula::Or { operands } if !conjunction => flat.extend(operands.iter().cloned()),
            _ => flat.push(operand),
        }
    }
    // N4 — sorted ascending by the byte ordering of their own N8 encodings, then
    // adjacent duplicates removed.
    flat.sort_by_cached_key(formula_key);
    flat.dedup();
    match flat.len() {
        // Every operand was the unit, so the junction *is* the unit. This is N7's
        // unit law taken to its limit; the RFC states the drop, and dropping the
        // last one has to leave something behind.
        0 => Formula::boolean(unit),
        // N4 — "A junction left with a single operand MUST be replaced by that
        // operand."
        1 => flat.remove(0),
        _ if conjunction => Formula::And { operands: flat },
        _ => Formula::Or { operands: flat },
    }
}

/// N7 — temporal simplification. `universal` selects `always` over `eventually`.
fn temporal(universal: bool, operand: Formula) -> Formula {
    match (&operand, universal) {
        // `always(always φ)` → `always φ`.
        (Formula::Always { .. }, true) | (Formula::Eventually { .. }, false) => operand,
        // `eventually(always(eventually φ))` → `always(eventually φ)`, and dually
        // `always(eventually(always φ))` → `eventually(always φ)`.
        (Formula::Always { operand: inner }, false)
            if matches!(**inner, Formula::Eventually { .. }) =>
        {
            operand
        }
        (Formula::Eventually { operand: inner }, true)
            if matches!(**inner, Formula::Always { .. }) =>
        {
            operand
        }
        // `always(true)` → `true`. The RFC names only this one; `always(false)`,
        // `eventually(true)`, and `eventually(false)` are valid and unlisted, so
        // they are deliberately not applied (module documentation, "Everything else
        // is declined").
        (Formula::Boolean { value: true }, true) => Formula::boolean(true),
        _ if universal => Formula::always(operand),
        _ => Formula::eventually(operand),
    }
}

/// N7 — one-sided quantifier collapse. `universal` selects `forall` over `exists`.
fn quantifier(universal: bool, binder: &Binder, body: Formula) -> Formula {
    match (&body, universal) {
        // `forall b. true` → `true`; `exists b. false` → `false`.
        //
        // The converses — `exists b. true` and `forall b. false` — are NOT
        // collapsed, "because they depend on the domain being non-empty".
        (Formula::Boolean { value: true }, true) => Formula::boolean(true),
        (Formula::Boolean { value: false }, false) => Formula::boolean(false),
        _ if universal => Formula::forall(binder.clone(), body),
        _ => Formula::exists(binder.clone(), body),
    }
}

// --- ordering keys -------------------------------------------------------------------------

/// N4's and N6's ordering key: the operand's own N8 encoding, compared bytewise.
fn formula_key(formula: &Formula) -> Vec<u8> {
    encode(formula)
}

fn term_key(term: &Term) -> Vec<u8> {
    term.to_json().to_canonical_bytes()
}

/// An upper bound on the node count after N1 eliminates the derived operators.
///
/// Saturating throughout, so an adversarial nesting reports [`MAX_NODES`] rather
/// than overflowing. Each arm mirrors the N1 rewrite it stands for: `implies(p, q)`
/// becomes `or(not p, q)` (two new nodes), `leads_to(p, q)` becomes
/// `always(or(not p, eventually q))` (three), and `iff(p, q)` becomes
/// `and(or(not p, q), or(p, not q))`, which is where both sides are duplicated.
fn expansion_bound(formula: &Formula) -> usize {
    match formula {
        Formula::Boolean { .. } | Formula::Action { .. } => 1,
        Formula::Predicate { args, .. } => 1 + args.iter().map(count_term_nodes).sum::<usize>(),
        Formula::Compare { left, right, .. } => {
            1 + count_term_nodes(left) + count_term_nodes(right)
        }
        Formula::Not { operand }
        | Formula::Always { operand }
        | Formula::Eventually { operand } => expansion_bound(operand).saturating_add(1),
        Formula::And { operands } | Formula::Or { operands } => {
            operands.iter().fold(1usize, |total, operand| {
                total.saturating_add(expansion_bound(operand))
            })
        }
        Formula::Implies {
            antecedent,
            consequent,
        } => expansion_bound(antecedent)
            .saturating_add(expansion_bound(consequent))
            .saturating_add(2),
        Formula::LeadsTo {
            antecedent,
            consequent,
        } => expansion_bound(antecedent)
            .saturating_add(expansion_bound(consequent))
            .saturating_add(3),
        Formula::Iff { left, right } => expansion_bound(left)
            .saturating_add(expansion_bound(right))
            .saturating_mul(2)
            .saturating_add(5),
        Formula::Forall { binder, body } | Formula::Exists { binder, body } => {
            expansion_bound(body)
                .saturating_add(count_term_nodes(&binder.domain))
                .saturating_add(1)
        }
    }
}

fn count_nodes(formula: &Formula) -> usize {
    1 + match formula {
        Formula::Boolean { .. } | Formula::Action { .. } => 0,
        Formula::Predicate { args, .. } => args.iter().map(count_term_nodes).sum(),
        Formula::Compare { left, right, .. } => count_term_nodes(left) + count_term_nodes(right),
        Formula::Not { operand }
        | Formula::Always { operand }
        | Formula::Eventually { operand } => count_nodes(operand),
        Formula::And { operands } | Formula::Or { operands } => {
            operands.iter().map(count_nodes).sum()
        }
        Formula::Implies {
            antecedent,
            consequent,
        }
        | Formula::LeadsTo {
            antecedent,
            consequent,
        } => count_nodes(antecedent) + count_nodes(consequent),
        Formula::Iff { left, right } => count_nodes(left) + count_nodes(right),
        Formula::Forall { binder, body } | Formula::Exists { binder, body } => {
            count_term_nodes(&binder.domain) + count_nodes(body)
        }
    }
}

fn count_term_nodes(term: &Term) -> usize {
    1 + match term {
        Term::Var { .. } | Term::Literal { .. } | Term::Constant { .. } => 0,
        Term::State { indices, .. } => indices.iter().map(count_term_nodes).sum(),
        Term::Apply { args, .. } => args.iter().map(count_term_nodes).sum(),
    }
}

/// Why a formula could not be normalized.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NormalizeError {
    /// The input is not a shape `$defs/formula` admits.
    Ast(AstError),
    /// A `var` node no enclosing binder declares.
    ///
    /// See the module documentation: N5's fixed `v<d>` naming cannot admit a free
    /// `var` without a capture-avoidance rule the RFC does not state.
    UnboundVariable {
        /// The unbound name.
        name: String,
    },
    /// The rewritten formula exceeded [`MAX_NODES`].
    TooLarge {
        /// The bound.
        max: usize,
    },
    /// No fixpoint within [`MAX_ROUNDS`].
    ///
    /// A partial normalization is never returned: an AST that is *almost* in CPNF-1
    /// would carry an identity nobody else computes, which is worse than a rejection.
    NotConverged {
        /// The bound.
        rounds: usize,
    },
}

impl fmt::Display for NormalizeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Ast(error) => error.fmt(f),
            Self::UnboundVariable { name } => write!(
                f,
                "the variable {name:?} is not bound by an enclosing quantifier; CPNF-1's N5 \
                 renames binders to fixed names and cannot admit a free `var` without capture \
                 (RFC 0037 W5, N5)"
            ),
            Self::TooLarge { max } => write!(
                f,
                "normalizing this formula exceeds the {max}-node bound; N1's `iff` expansion \
                 duplicates both sides"
            ),
            Self::NotConverged { rounds } => write!(
                f,
                "CPNF-1 normalization did not reach a fixpoint in {rounds} rounds; a partial \
                 normal form is never returned"
            ),
        }
    }
}

impl core::error::Error for NormalizeError {}

impl From<AstError> for NormalizeError {
    fn from(error: AstError) -> Self {
        Self::Ast(error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::{ActionModality, Literal};

    fn ident(name: &str) -> Identifier {
        Identifier::new(name).expect("a test identifier is well formed")
    }

    fn predicate(name: &str) -> Formula {
        Formula::predicate(ident(name), Vec::new())
    }

    fn predicate_of(name: &str, argument: &str) -> Formula {
        Formula::predicate(
            ident(name),
            vec![Term::Var {
                name: ident(argument),
            }],
        )
    }

    fn constant(name: &str) -> Term {
        Term::Constant { name: ident(name) }
    }

    fn integer(value: i64) -> Term {
        Term::Literal {
            value: Literal::Integer(value),
        }
    }

    fn state(name: &str) -> Term {
        Term::State {
            name: ident(name),
            indices: Vec::new(),
        }
    }

    /// The N8 encoding of the two boolean constants. A `boolean` node encodes as an
    /// object like every other node; there is no bare `true` in CPNF-1.
    const TRUE: &str = r#"{"kind":"boolean","value":true}"#;
    const FALSE: &str = r#"{"kind":"boolean","value":false}"#;

    fn text(formula: &Formula) -> String {
        String::from_utf8(encode(formula)).expect("N8 output is UTF-8")
    }

    fn normal_text(formula: &Formula) -> String {
        text(&normalize(formula).expect("normalizes"))
    }

    /// Assert that every spelling in `corpus` reaches one CPNF-1 encoding.
    fn all_agree(corpus: &[Formula]) -> String {
        let mut encodings = corpus.iter().map(normal_text);
        let first = encodings.next().expect("a non-empty corpus");
        for (index, encoding) in encodings.enumerate() {
            assert_eq!(
                encoding,
                first,
                "corpus entry {} normalized differently",
                index + 1
            );
        }
        first
    }

    // --- N1: derived-operator elimination ---------------------------------------

    #[test]
    fn n1_eliminates_implies_iff_and_leads_to() {
        // `implies(p, q)` and `or(not p, q)` are one property.
        let expanded = all_agree(&[
            Formula::implies(predicate("p"), predicate("q")),
            Formula::or(vec![Formula::not(predicate("p")), predicate("q")]).expect("two operands"),
        ]);
        assert!(!expanded.contains("implies"), "{expanded}");
        assert!(expanded.starts_with(r#"{"kind":"or""#), "{expanded}");

        // `iff(p, q)` is the conjunction of the two implications, however spelled.
        let equivalence = all_agree(&[
            Formula::iff(predicate("p"), predicate("q")),
            Formula::and(vec![
                Formula::implies(predicate("p"), predicate("q")),
                Formula::implies(predicate("q"), predicate("p")),
            ])
            .expect("two operands"),
        ]);
        assert!(!equivalence.contains("iff"), "{equivalence}");

        // `leads_to(p, q)` is `always(or(not p, eventually q))`, however spelled.
        // "A response written as a `leads_to` node and the same response written out
        // by hand therefore normalize identically."
        let response = all_agree(&[
            Formula::leads_to(predicate("p"), predicate("q")),
            Formula::always(
                Formula::or(vec![
                    Formula::not(predicate("p")),
                    Formula::eventually(predicate("q")),
                ])
                .expect("two operands"),
            ),
            Formula::always(Formula::implies(
                predicate("p"),
                Formula::eventually(predicate("q")),
            )),
        ]);
        assert!(!response.contains("leads_to"), "{response}");
        assert!(response.starts_with(r#"{"kind":"always""#), "{response}");
    }

    // --- N2: negation-normal form -----------------------------------------------

    #[test]
    fn n2_pushes_negation_to_the_atoms() {
        // De Morgan, double negation, temporal duality, and quantifier duality all
        // land on one form.
        let encoding = all_agree(&[
            Formula::not(Formula::and(vec![predicate("p"), predicate("q")]).expect("two operands")),
            Formula::or(vec![
                Formula::not(predicate("p")),
                Formula::not(predicate("q")),
            ])
            .expect("two operands"),
            Formula::not(Formula::not(
                Formula::or(vec![
                    Formula::not(predicate("p")),
                    Formula::not(predicate("q")),
                ])
                .expect("two operands"),
            )),
        ]);
        assert_eq!(
            encoding,
            concat!(
                r#"{"kind":"or","operands":["#,
                r#"{"kind":"not","operand":{"args":[],"kind":"predicate","name":"p"}},"#,
                r#"{"kind":"not","operand":{"args":[],"kind":"predicate","name":"q"}}]}"#,
            )
        );

        // `not always φ` → `eventually not φ`.
        assert_eq!(
            normal_text(&Formula::not(Formula::always(predicate("p")))),
            normal_text(&Formula::eventually(Formula::not(predicate("p"))))
        );
        // `not eventually φ` → `always not φ`.
        assert_eq!(
            normal_text(&Formula::not(Formula::eventually(predicate("p")))),
            normal_text(&Formula::always(Formula::not(predicate("p"))))
        );
        // `not forall b. φ` → `exists b. not φ` and dually.
        let binder = Binder::new(ident("x"), constant("D"));
        assert_eq!(
            normal_text(&Formula::not(Formula::forall(
                binder.clone(),
                predicate_of("p", "x")
            ))),
            normal_text(&Formula::exists(
                binder.clone(),
                Formula::not(predicate_of("p", "x"))
            ))
        );
        assert_eq!(
            normal_text(&Formula::not(Formula::exists(
                binder.clone(),
                predicate_of("p", "x")
            ))),
            normal_text(&Formula::forall(
                binder,
                Formula::not(predicate_of("p", "x"))
            ))
        );
    }

    #[test]
    fn after_n2_a_negation_stands_only_above_an_atom() {
        fn check(formula: &Formula) {
            if let Formula::Not { operand } = formula {
                assert!(
                    matches!(
                        **operand,
                        Formula::Predicate { .. }
                            | Formula::Action { .. }
                            | Formula::Compare { .. }
                    ),
                    "a `not` survived above {}",
                    operand.kind()
                );
            }
            match formula {
                Formula::Not { operand }
                | Formula::Always { operand }
                | Formula::Eventually { operand } => check(operand),
                Formula::And { operands } | Formula::Or { operands } => {
                    operands.iter().for_each(check);
                }
                Formula::Forall { body, .. } | Formula::Exists { body, .. } => check(body),
                _ => {}
            }
        }
        let hostile = Formula::not(Formula::always(Formula::iff(
            Formula::leads_to(predicate("p"), predicate("q")),
            Formula::not(Formula::exists(
                Binder::new(ident("x"), constant("D")),
                Formula::implies(predicate_of("r", "x"), predicate("s")),
            )),
        )));
        check(&normalize(&hostile).expect("normalizes"));
    }

    #[test]
    fn a_negated_boolean_constant_folds_rather_than_surviving() {
        // A recorded reading of N2/N7 — see the module documentation.
        assert_eq!(normal_text(&Formula::not(Formula::boolean(true))), FALSE);
        assert_eq!(normal_text(&Formula::not(Formula::boolean(false))), TRUE);
        assert_eq!(
            normal_text(
                &Formula::and(vec![Formula::not(Formula::boolean(true)), predicate("p")])
                    .expect("two operands")
            ),
            FALSE
        );
    }

    // --- N3, N4: flattening, ordering, idempotence -------------------------------

    #[test]
    fn n3_flattens_and_n4_orders_and_deduplicates() {
        let flat = all_agree(&[
            // Nested one way.
            Formula::and(vec![
                predicate("a"),
                Formula::and(vec![predicate("b"), predicate("c")]).expect("two operands"),
            ])
            .expect("two operands"),
            // Nested the other way.
            Formula::and(vec![
                Formula::and(vec![predicate("a"), predicate("b")]).expect("two operands"),
                predicate("c"),
            ])
            .expect("two operands"),
            // Reordered.
            Formula::and(vec![predicate("c"), predicate("b"), predicate("a")])
                .expect("three operands"),
            // Duplicated.
            Formula::and(vec![
                predicate("b"),
                predicate("a"),
                predicate("b"),
                predicate("c"),
                predicate("a"),
            ])
            .expect("five operands"),
        ]);
        // One n-ary node, operands ascending by their own N8 encodings.
        assert_eq!(flat.matches(r#""kind":"and""#).count(), 1);
        let a = flat.find(r#""name":"a""#).expect("a is present");
        let b = flat.find(r#""name":"b""#).expect("b is present");
        let c = flat.find(r#""name":"c""#).expect("c is present");
        assert!(a < b && b < c, "operands are not in N8 byte order: {flat}");
        assert_eq!(flat.matches(r#""name":"a""#).count(), 1);
    }

    #[test]
    fn a_junction_reduced_to_one_operand_is_replaced_by_it() {
        assert_eq!(
            normal_text(&Formula::and(vec![predicate("p"), predicate("p")]).expect("two operands")),
            normal_text(&predicate("p"))
        );
        assert_eq!(
            normal_text(
                &Formula::or(vec![predicate("p"), Formula::boolean(false)]).expect("two operands")
            ),
            normal_text(&predicate("p"))
        );
    }

    // --- N5: binder canonicalization ---------------------------------------------

    #[test]
    fn n5_renames_bound_variables_to_their_de_bruijn_level() {
        let renamed = normalize(&Formula::forall(
            Binder::new(ident("outer"), constant("D")),
            Formula::exists(
                Binder::new(ident("inner"), constant("E")),
                Formula::predicate(
                    ident("p"),
                    vec![
                        Term::Var {
                            name: ident("outer"),
                        },
                        Term::Var {
                            name: ident("inner"),
                        },
                    ],
                ),
            ),
        ))
        .expect("normalizes");
        let encoded = text(&renamed);
        assert!(encoded.contains(r#""variable":"v0""#), "{encoded}");
        assert!(encoded.contains(r#""variable":"v1""#), "{encoded}");
        assert!(!encoded.contains("outer"), "{encoded}");
        assert!(!encoded.contains("inner"), "{encoded}");
    }

    #[test]
    fn alpha_equivalent_properties_have_one_normal_form() {
        // "renaming a bound variable cannot change identity"
        let with_e = Formula::forall(
            Binder::new(ident("e"), constant("Events")),
            predicate_of("p", "e"),
        );
        let with_x = Formula::forall(
            Binder::new(ident("x"), constant("Events")),
            predicate_of("p", "x"),
        );
        assert_eq!(normal_text(&with_e), normal_text(&with_x));
    }

    #[test]
    fn shadowing_resolves_to_the_innermost_binder() {
        // Both binders declare `x`; the inner reference must reach the inner binder.
        let shadowed = Formula::forall(
            Binder::new(ident("x"), constant("D")),
            Formula::exists(
                Binder::new(ident("x"), constant("E")),
                predicate_of("p", "x"),
            ),
        );
        let renamed = Formula::forall(
            Binder::new(ident("a"), constant("D")),
            Formula::exists(
                Binder::new(ident("b"), constant("E")),
                predicate_of("p", "b"),
            ),
        );
        assert_eq!(normal_text(&shadowed), normal_text(&renamed));
        // And *not* to the outer one, which would be a different property.
        let outer_reference = Formula::forall(
            Binder::new(ident("a"), constant("D")),
            Formula::exists(
                Binder::new(ident("b"), constant("E")),
                predicate_of("p", "a"),
            ),
        );
        assert_ne!(normal_text(&shadowed), normal_text(&outer_reference));
    }

    #[test]
    fn a_free_var_is_rejected_rather_than_captured() {
        // The capture N5 would commit if free `var` names were admitted: `v0` is
        // exactly the name N5 hands the enclosing binder.
        let hazard = Formula::forall(
            Binder::new(ident("x"), constant("D")),
            Formula::predicate(
                ident("p"),
                vec![
                    Term::Var { name: ident("x") },
                    Term::Var { name: ident("v0") },
                ],
            ),
        );
        assert_eq!(
            normalize(&hazard),
            Err(NormalizeError::UnboundVariable {
                name: "v0".to_owned()
            })
        );
        // A bare free variable is rejected the same way.
        assert_eq!(
            normalize(&predicate_of("p", "y")),
            Err(NormalizeError::UnboundVariable {
                name: "y".to_owned()
            })
        );
        // A binder's *domain* is outside its own scope, so a `var` there is free.
        assert_eq!(
            normalize(&Formula::forall(
                Binder::new(ident("x"), Term::Var { name: ident("x") }),
                predicate_of("p", "x"),
            )),
            Err(NormalizeError::UnboundVariable {
                name: "x".to_owned()
            })
        );
        // Model-level names spelled as `state`/`constant` terms are untouched.
        assert!(
            normalize(&Formula::compare(
                ComparisonOperator::Le,
                state("big"),
                integer(5)
            ))
            .is_ok()
        );
    }

    // --- N6: comparison canonicalization -----------------------------------------

    #[test]
    fn n6_rewrites_gt_and_ge_by_swapping_operands() {
        assert_eq!(
            normal_text(&Formula::compare(
                ComparisonOperator::Gt,
                state("big"),
                integer(4)
            )),
            normal_text(&Formula::compare(
                ComparisonOperator::Lt,
                integer(4),
                state("big")
            ))
        );
        assert_eq!(
            normal_text(&Formula::compare(
                ComparisonOperator::Ge,
                state("big"),
                integer(0)
            )),
            normal_text(&Formula::compare(
                ComparisonOperator::Le,
                integer(0),
                state("big")
            ))
        );
        // Only six operators survive.
        for op in [ComparisonOperator::Gt, ComparisonOperator::Ge] {
            let encoded = normal_text(&Formula::compare(op, state("s"), integer(1)));
            assert!(!encoded.contains(op.wire()), "{encoded}");
        }
    }

    #[test]
    fn n6_orders_the_commutative_comparisons_and_absorbs_the_right_negations() {
        // `eq`/`ne` operands are ordered by their N8 encodings.
        assert_eq!(
            normal_text(&Formula::compare(
                ComparisonOperator::Eq,
                state("big"),
                integer(4)
            )),
            normal_text(&Formula::compare(
                ComparisonOperator::Eq,
                integer(4),
                state("big")
            ))
        );
        // `not eq` ↔ `ne`.
        assert_eq!(
            normal_text(&Formula::not(Formula::compare(
                ComparisonOperator::Eq,
                state("big"),
                integer(4)
            ))),
            normal_text(&Formula::compare(
                ComparisonOperator::Ne,
                state("big"),
                integer(4)
            ))
        );
        assert_eq!(
            normal_text(&Formula::not(Formula::compare(
                ComparisonOperator::Ne,
                state("big"),
                integer(4)
            ))),
            normal_text(&Formula::compare(
                ComparisonOperator::Eq,
                state("big"),
                integer(4)
            ))
        );
        // A `not` above `lt`/`le` survives — no declared total order — and above
        // `member`/`subset` it must.
        for op in [
            ComparisonOperator::Lt,
            ComparisonOperator::Le,
            ComparisonOperator::Member,
            ComparisonOperator::Subset,
        ] {
            let encoded = normal_text(&Formula::not(Formula::compare(op, state("s"), integer(1))));
            assert!(
                encoded.starts_with(r#"{"kind":"not""#),
                "the negation of {} was absorbed: {encoded}",
                op.wire()
            );
        }
    }

    // --- N7: unit, absorption, and the collapses that are declined ----------------

    #[test]
    fn n7_applies_the_unit_laws() {
        assert_eq!(
            normal_text(
                &Formula::and(vec![predicate("p"), Formula::boolean(false)]).expect("two operands")
            ),
            FALSE
        );
        assert_eq!(
            normal_text(
                &Formula::or(vec![predicate("p"), Formula::boolean(true)]).expect("two operands")
            ),
            TRUE
        );
        assert_eq!(
            normal_text(
                &Formula::and(vec![predicate("p"), Formula::boolean(true)]).expect("two operands")
            ),
            normal_text(&predicate("p"))
        );
        assert_eq!(
            normal_text(
                &Formula::or(vec![predicate("p"), Formula::boolean(false)]).expect("two operands")
            ),
            normal_text(&predicate("p"))
        );
        // Every operand dropped leaves the unit behind.
        assert_eq!(
            normal_text(
                &Formula::and(vec![Formula::boolean(true), Formula::boolean(true)])
                    .expect("two operands")
            ),
            TRUE
        );
        assert_eq!(
            normal_text(
                &Formula::or(vec![Formula::boolean(false), Formula::boolean(false)])
                    .expect("two operands")
            ),
            FALSE
        );
    }

    #[test]
    fn n7_collapses_the_named_temporal_towers_and_no_others() {
        let p = predicate("p");
        assert_eq!(
            normal_text(&Formula::always(Formula::always(p.clone()))),
            normal_text(&Formula::always(p.clone()))
        );
        assert_eq!(
            normal_text(&Formula::eventually(Formula::eventually(p.clone()))),
            normal_text(&Formula::eventually(p.clone()))
        );
        // `FGF φ ≡ GF φ` and `GFG φ ≡ FG φ`.
        let recurrence = Formula::always(Formula::eventually(p.clone()));
        assert_eq!(
            normal_text(&Formula::eventually(recurrence.clone())),
            normal_text(&recurrence)
        );
        let persistence = Formula::eventually(Formula::always(p.clone()));
        assert_eq!(
            normal_text(&Formula::always(persistence.clone())),
            normal_text(&persistence)
        );
        assert_eq!(normal_text(&Formula::always(Formula::boolean(true))), TRUE);
        // Declined, deliberately: valid but unlisted in N7.
        assert_ne!(
            normal_text(&Formula::always(Formula::boolean(false))),
            FALSE
        );
        assert_ne!(
            normal_text(&Formula::eventually(Formula::boolean(true))),
            TRUE
        );
        // `always(eventually φ)` is not itself collapsed — it is the canonical
        // spelling of recurrence, which has no node of its own.
        assert!(normal_text(&recurrence).starts_with(r#"{"kind":"always""#));
    }

    #[test]
    fn n7s_quantifier_collapse_is_one_sided() {
        let binder = Binder::new(ident("x"), constant("D"));
        assert_eq!(
            normal_text(&Formula::forall(binder.clone(), Formula::boolean(true))),
            TRUE
        );
        assert_eq!(
            normal_text(&Formula::exists(binder.clone(), Formula::boolean(false))),
            FALSE
        );
        // "MUST NOT be collapsed, because they depend on the domain being
        // non-empty."
        assert_ne!(
            normal_text(&Formula::exists(binder.clone(), Formula::boolean(true))),
            TRUE
        );
        assert_ne!(
            normal_text(&Formula::forall(binder, Formula::boolean(false))),
            FALSE
        );
    }

    #[test]
    fn the_tautologies_beyond_n7_are_deliberately_unrecognized() {
        // S2: "propositional tautologies beyond N7 are not recognized […] Distinct
        // normal forms MUST NOT be read as a direction." Each pair below is
        // semantically equal and normalizes apart, on purpose.
        let absorption_left = Formula::and(vec![
            predicate("a"),
            Formula::or(vec![predicate("a"), predicate("b")]).expect("two operands"),
        ])
        .expect("two operands");
        assert_ne!(normal_text(&absorption_left), normal_text(&predicate("a")));

        let contradiction =
            Formula::and(vec![predicate("a"), Formula::not(predicate("a"))]).expect("two operands");
        assert_ne!(normal_text(&contradiction), FALSE);

        // Same-kind quantifiers are not reordered.
        let xy = Formula::forall(
            Binder::new(ident("x"), constant("D")),
            Formula::forall(
                Binder::new(ident("y"), constant("E")),
                Formula::predicate(
                    ident("p"),
                    vec![
                        Term::Var { name: ident("x") },
                        Term::Var { name: ident("y") },
                    ],
                ),
            ),
        );
        let yx = Formula::forall(
            Binder::new(ident("y"), constant("E")),
            Formula::forall(
                Binder::new(ident("x"), constant("D")),
                Formula::predicate(
                    ident("p"),
                    vec![
                        Term::Var { name: ident("x") },
                        Term::Var { name: ident("y") },
                    ],
                ),
            ),
        );
        assert_ne!(normal_text(&xy), normal_text(&yx));

        // `apply` arguments are not reordered: "no algebraic law is assumed for an
        // operator whose laws are not declared".
        let plus_ab = Formula::compare(
            ComparisonOperator::Lt,
            Term::apply(ident("plus"), vec![state("a"), state("b")]).expect("two args"),
            integer(1),
        );
        let plus_ba = Formula::compare(
            ComparisonOperator::Lt,
            Term::apply(ident("plus"), vec![state("b"), state("a")]).expect("two args"),
            integer(1),
        );
        assert_ne!(normal_text(&plus_ab), normal_text(&plus_ba));
    }

    // --- N9: idempotence and determinism -----------------------------------------

    #[test]
    fn n9_normalize_is_idempotent_over_the_rewrite_corpus() {
        for formula in rewrite_corpus() {
            let once = normalize(&formula).expect("normalizes");
            let twice = normalize(&once).expect("re-normalizes");
            assert_eq!(
                encode(&once),
                encode(&twice),
                "normalize is not idempotent for {}",
                text(&formula)
            );
            assert!(is_normal(&once).expect("checks"));
            assert!(
                !is_normal(&formula).expect("checks") || once == formula,
                "is_normal disagreed with normalize"
            );
        }
    }

    #[test]
    fn n9_normalization_is_a_function_of_content_alone() {
        // Ten runs over a shuffled-by-construction corpus: nothing here reads a
        // clock, an allocator address, or a `std` hasher, so the encoding is fixed.
        let formula = rewrite_corpus().remove(0);
        let expected = normal_text(&formula);
        for _ in 0..10 {
            assert_eq!(normal_text(&formula.clone()), expected);
        }
    }

    /// The RFC's acceptance corpus, verbatim: "`implies`/`leads_to` expansion, De
    /// Morgan pushes, bound-variable renaming, junction reordering and duplication,
    /// `gt`/`ge` swaps — MUST normalize to identical CPNF-1 encodings".
    fn rewrite_corpus() -> Vec<Formula> {
        let acknowledged = |var: &str| predicate_of("acknowledged", var);
        let durable = |var: &str| predicate_of("durable", var);
        let events = || constant("Events");
        vec![
            // The RFC's own worked example: `always (forall e in Events:
            // acknowledged[e] => durable[e])`.
            Formula::always(Formula::forall(
                Binder::new(ident("e"), events()),
                Formula::implies(acknowledged("e"), durable("e")),
            )),
            // "Rewriting the same claim as `not(exists e in Events: acknowledged[e]
            // and not durable[e])`" …
            Formula::not(Formula::eventually(Formula::exists(
                Binder::new(ident("e"), events()),
                Formula::and(vec![acknowledged("e"), Formula::not(durable("e"))])
                    .expect("two operands"),
            ))),
            // … "or renaming `e` to `x`" …
            Formula::always(Formula::forall(
                Binder::new(ident("x"), events()),
                Formula::implies(acknowledged("x"), durable("x")),
            )),
            // … "or swapping the two disjuncts".
            Formula::always(Formula::forall(
                Binder::new(ident("e"), events()),
                Formula::or(vec![durable("e"), Formula::not(acknowledged("e"))])
                    .expect("two operands"),
            )),
            // Duplication, nesting, and a `gt` swap on top.
            Formula::always(Formula::forall(
                Binder::new(ident("e"), events()),
                Formula::and(vec![
                    Formula::or(vec![
                        Formula::not(acknowledged("e")),
                        durable("e"),
                        Formula::not(acknowledged("e")),
                    ])
                    .expect("three operands"),
                    Formula::boolean(true),
                ])
                .expect("two operands"),
            )),
        ]
    }

    #[test]
    fn the_whole_rewrite_corpus_reaches_one_encoding() {
        let encoding = all_agree(&rewrite_corpus());
        // The RFC's N8 line for the worked example, with the outer shape pinned.
        assert!(
            encoding.starts_with(r#"{"kind":"always","operand":{"binder":"#),
            "{encoding}"
        );
        assert!(encoding.contains(r#""variable":"v0""#), "{encoding}");
        assert!(encoding.contains(r#""kind":"forall""#), "{encoding}");
        assert!(!encoding.contains("implies"), "{encoding}");
        assert!(!encoding.contains(r#""variable":"e""#), "{encoding}");
    }

    #[test]
    fn a_weakening_does_not_reach_the_same_encoding() {
        // "Replacing `durable` with a weaker predicate does not [classify
        // unchanged]" — the docs/50 "weaken property" attack, at the level this
        // module is responsible for.
        let base = rewrite_corpus().remove(0);
        let weakened = Formula::always(Formula::forall(
            Binder::new(ident("e"), constant("Events")),
            Formula::implies(
                predicate_of("acknowledged", "e"),
                predicate_of("eventually_durable", "e"),
            ),
        ));
        assert_ne!(normal_text(&base), normal_text(&weakened));

        // Dropping a conjunct is a weakening and moves the encoding.
        let both = Formula::always(
            Formula::and(vec![predicate("safe"), predicate("live")]).expect("two operands"),
        );
        let one = Formula::always(predicate("safe"));
        assert_ne!(normal_text(&both), normal_text(&one));

        // Guarding a claim behind an extra hypothesis is a weakening and moves it.
        let guarded = Formula::always(Formula::implies(predicate("guard"), predicate("safe")));
        assert_ne!(normal_text(&one), normal_text(&guarded));
    }

    // --- adversarial identifiers and resource bounds ------------------------------

    #[test]
    fn distinct_properties_never_share_an_encoding() {
        // The house adversarial-identifier test: shapes whose *concatenations* are
        // equal, or whose names embed the encoding's own punctuation, must stay
        // apart. A naive "join the names with a separator" encoder collides on
        // several of these.
        let distinct = vec![
            Formula::predicate(ident("ab"), Vec::new()),
            Formula::predicate(ident("a"), vec![Term::Constant { name: ident("b") }]),
            Formula::predicate(ident("a.b"), Vec::new()),
            Formula::predicate(
                ident("a"),
                vec![Term::Literal {
                    value: Literal::Text("b".to_owned()),
                }],
            ),
            Formula::predicate(
                ident("a"),
                vec![Term::Literal {
                    value: Literal::Text(r#"","kind":"predicate","name":"z"#.to_owned()),
                }],
            ),
            Formula::predicate(
                ident("a"),
                vec![Term::Literal {
                    value: Literal::Integer(1),
                }],
            ),
            Formula::predicate(
                ident("a"),
                vec![Term::Literal {
                    value: Literal::Text("1".to_owned()),
                }],
            ),
            Formula::predicate(
                ident("a"),
                vec![Term::Literal {
                    value: Literal::Boolean(true),
                }],
            ),
            Formula::predicate(
                ident("a"),
                vec![Term::Literal {
                    value: Literal::Null,
                }],
            ),
            Formula::action(ident("a"), ActionModality::Occurs),
            Formula::action(ident("a"), ActionModality::Enabled),
            Formula::always(predicate("a")),
            Formula::eventually(predicate("a")),
            Formula::not(predicate("a")),
            predicate("a"),
            Formula::compare(ComparisonOperator::Member, state("a"), state("b")),
            Formula::compare(ComparisonOperator::Subset, state("a"), state("b")),
            Formula::compare(ComparisonOperator::Lt, state("a"), state("b")),
            Formula::compare(ComparisonOperator::Le, state("a"), state("b")),
            Formula::compare(ComparisonOperator::Eq, state("a"), state("b")),
            Formula::compare(ComparisonOperator::Ne, state("a"), state("b")),
            Term::State {
                name: ident("a"),
                indices: vec![state("b")],
            }
            .pipe_compare(),
            Formula::and(vec![predicate("a"), predicate("b")]).expect("two operands"),
            Formula::or(vec![predicate("a"), predicate("b")]).expect("two operands"),
        ];
        let mut seen: std::collections::BTreeMap<String, usize> = std::collections::BTreeMap::new();
        for (index, formula) in distinct.iter().enumerate() {
            let encoding = normal_text(formula);
            if let Some(previous) = seen.insert(encoding.clone(), index) {
                panic!("entries {previous} and {index} collided on {encoding}");
            }
        }
        assert_eq!(seen.len(), distinct.len());
    }

    /// Small helper so an indexed `state` term can appear in the corpus above as a
    /// formula rather than a term.
    trait PipeCompare {
        fn pipe_compare(self) -> Formula;
    }

    impl PipeCompare for Term {
        fn pipe_compare(self) -> Formula {
            Formula::compare(
                ComparisonOperator::Eq,
                self,
                Term::Literal {
                    value: Literal::Integer(0),
                },
            )
        }
    }

    #[test]
    fn an_invalid_ast_is_rejected_before_it_is_rewritten() {
        assert_eq!(
            normalize(&Formula::And {
                operands: Vec::new()
            }),
            Err(NormalizeError::Ast(AstError::JunctionArity {
                kind: "and",
                found: 0
            }))
        );
    }

    #[test]
    fn a_bi_implication_tower_hits_the_node_bound_rather_than_the_allocator() {
        let mut formula = predicate("p0");
        for index in 1..20 {
            let next = Formula::predicate(
                Identifier::new(&format!("p{index}")).expect("well formed"),
                Vec::new(),
            );
            formula = Formula::iff(formula, next);
        }
        assert_eq!(
            normalize(&formula),
            Err(NormalizeError::TooLarge { max: MAX_NODES })
        );
    }

    #[test]
    fn the_normal_form_token_is_the_schemas_const() {
        assert_eq!(CPNF_VERSION, "cpnf-1");
    }
}
