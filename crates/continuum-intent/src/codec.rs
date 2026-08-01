//! The crate's one reader for property expressions and the AST beneath them.
//!
//! # Why one reader
//!
//! Three field groups carry a Finite-core formula, and they carry it in two shapes:
//!
//! - `claims[].expression` and `assumptions[].expression` are
//!   `$defs/property_expression` objects — `{ast, source?, fragment?, normal_form?}`;
//! - `fairness[].condition` is a **bare** `$defs/formula`, with no `source`, no
//!   `fragment`, and no `normal_form` declaration.
//!
//! Both shapes bottom out in the same eleven formula nodes and five term nodes, and
//! the reader for them used to exist three times: once privately in
//! [`crate::property`], once copied into [`crate::assumptions`] because that private
//! reader was out of reach, and once reached by [`crate::fairness`] through a
//! synthetic claim built only to get at it. Two readers for one grammar is how a
//! document comes to have two meanings; three is how it comes to have three. This
//! module is the one reader, and the two public constructors below are the two shapes
//! it is entered by.
//!
//! # What the reader is strict about
//!
//! Everything the schema is strict about, and one thing beyond it:
//!
//! - unknown keys are rejected, because `additionalProperties: false` holds on every
//!   formula and term node and "a reader that ignored [an unknown key] would accept
//!   documents the schema rejects";
//! - an unknown token in a closed vocabulary — a `next` node, a `cpnf-2` declaration,
//!   a seventh comparison operator — is rejected, never ignored: "Forward
//!   compatibility is achieved by rejecting, never by ignoring" (RFC 0037);
//! - a `normal_form` declaration is *verified* by re-normalizing, which is the one
//!   check that is not structural: "a checker MUST verify the claim by re-normalizing
//!   and MUST reject a contract whose declaration does not hold".
//!
//! Recursion is bounded twice over: [`crate::canonical_json`]'s parser refuses a
//! document deeper than [`MAX_DEPTH`] before this reader runs, and the reader carries
//! its own depth so a hand-built [`Json`] value reaches the same typed error rather
//! than the stack.
//!
//! # The error type, and why it is `PropertyDecodeError`
//!
//! A rejection has to name the field and the token that failed, and the field paths
//! this reader knows are the AST's own — `formula.kind`, `compare.op`, `term.kind`,
//! `expression.normal_form`. [`PropertyDecodeError`] is the vocabulary for exactly
//! those, and it is already what [`crate::fairness::FairnessDecodeError::Condition`]
//! carries. [`crate::assumptions`] keeps its own [`AssumptionDecodeError`] — its
//! rejections must keep naming `assumptions[].*` — and converts variant-for-variant
//! through `From<PropertyDecodeError>`, so its messages are unchanged and its
//! vocabulary stays its own.
//!
//! [`AssumptionDecodeError`]: crate::assumptions::AssumptionDecodeError

use std::collections::BTreeMap;

use crate::ast::{
    ActionModality, AstError, Binder, ComparisonOperator, Formula, Fragment, Identifier, Literal,
    Term,
};
use crate::canonical_json::{Json, MAX_DEPTH};
use crate::cpnf::{self, CPNF_VERSION};
use crate::property::{PropertyDecodeError, PropertyExpression};

impl Formula {
    /// Decode a bare `$defs/formula` node.
    ///
    /// This is the AST as authored, *not* normalized: CPNF-1 is
    /// [`crate::cpnf::normalize`]'s job, and a caller that wants the normal form asks
    /// for it — [`PropertyExpression::from_json`] is the constructor that does both.
    /// The distinction matters for [`crate::fairness`], whose `condition` is a bare
    /// formula the schema states no `normal_form` declaration for.
    ///
    /// # Errors
    ///
    /// [`PropertyDecodeError`], naming the field or the token that failed by its
    /// AST-level path (`formula.kind`, `compare.op`, `term.kind`, …).
    pub fn from_json(json: &Json) -> Result<Self, PropertyDecodeError> {
        decode_formula(json, 0)
    }
}

impl PropertyExpression {
    /// Decode a `$defs/property_expression` object: `{ast, source?, fragment?,
    /// normal_form?}`.
    ///
    /// The AST is normalized on the way in, so an expression decoded from an
    /// authoring form and the same expression decoded from its normal form are one
    /// value with one identity. A `normal_form` declaration, when present, is
    /// verified by re-normalizing rather than believed.
    ///
    /// # Errors
    ///
    /// [`PropertyDecodeError`], naming the field or the token that failed;
    /// [`PropertyDecodeError::FalseNormalForm`] when the document declares
    /// `normal_form: "cpnf-1"` and its AST is not in CPNF-1.
    pub fn from_json(json: &Json) -> Result<Self, PropertyDecodeError> {
        decode_expression(json)
    }
}

// --- object-shape helpers ------------------------------------------------------------------

pub(crate) fn object<'a>(
    json: &'a Json,
    field: &'static str,
) -> Result<&'a BTreeMap<String, Json>, PropertyDecodeError> {
    json.as_object()
        .ok_or_else(|| PropertyDecodeError::TypeMismatch {
            field,
            expected: "object",
            found: json.type_name(),
        })
}

/// Enforce `additionalProperties: false` for one object.
pub(crate) fn known_keys(
    fields: &BTreeMap<String, Json>,
    field: &'static str,
    allowed: &[&str],
) -> Result<(), PropertyDecodeError> {
    for key in fields.keys() {
        if !allowed.contains(&key.as_str()) {
            return Err(PropertyDecodeError::UnknownField {
                field,
                key: key.clone(),
            });
        }
    }
    Ok(())
}

pub(crate) fn string_field<'a>(
    fields: &'a BTreeMap<String, Json>,
    field: &'static str,
) -> Result<&'a str, PropertyDecodeError> {
    let key = field.rsplit('.').next().unwrap_or(field);
    let value = fields
        .get(key)
        .ok_or(PropertyDecodeError::MissingField { field })?;
    value
        .as_str()
        .ok_or_else(|| PropertyDecodeError::TypeMismatch {
            field,
            expected: "string",
            found: value.type_name(),
        })
}

// --- the recursive descent -----------------------------------------------------------------

fn decode_expression(json: &Json) -> Result<PropertyExpression, PropertyDecodeError> {
    let fields = object(json, "expression")?;
    known_keys(
        fields,
        "expression",
        &["ast", "fragment", "normal_form", "source"],
    )?;
    let ast_json = fields.get("ast").ok_or(PropertyDecodeError::MissingField {
        field: "expression.ast",
    })?;
    let ast = decode_formula(ast_json, 0)?;
    let fragment =
        match fields.get("fragment") {
            None => None,
            Some(Json::String(token)) => Some(Fragment::from_wire(token).ok_or_else(|| {
                PropertyDecodeError::UnknownToken {
                    field: "expression.fragment",
                    token: token.clone(),
                }
            })?),
            Some(other) => {
                return Err(PropertyDecodeError::TypeMismatch {
                    field: "expression.fragment",
                    expected: "string",
                    found: other.type_name(),
                });
            }
        };
    let source = match fields.get("source") {
        None => None,
        Some(Json::String(text)) => Some(text.clone()),
        Some(other) => {
            return Err(PropertyDecodeError::TypeMismatch {
                field: "expression.source",
                expected: "string",
                found: other.type_name(),
            });
        }
    };
    // The `normal_form` declaration is a claim about the AST, and RFC 0037 says a
    // checker "MUST verify the claim by re-normalizing and MUST reject a contract
    // whose declaration does not hold".
    if let Some(declared) = fields.get("normal_form") {
        let token = declared
            .as_str()
            .ok_or_else(|| PropertyDecodeError::TypeMismatch {
                field: "expression.normal_form",
                expected: "string",
                found: declared.type_name(),
            })?;
        if token != CPNF_VERSION {
            return Err(PropertyDecodeError::UnknownToken {
                field: "expression.normal_form",
                token: token.to_owned(),
            });
        }
        if !cpnf::is_normal(&ast)? {
            return Err(PropertyDecodeError::FalseNormalForm);
        }
    }
    Ok(PropertyExpression::normalized(&ast, fragment, source)?)
}

fn decode_formula(json: &Json, depth: usize) -> Result<Formula, PropertyDecodeError> {
    if depth >= MAX_DEPTH {
        return Err(PropertyDecodeError::Ast(AstError::TooDeep {
            max: MAX_DEPTH,
        }));
    }
    let fields = object(json, "formula")?;
    let kind = string_field(fields, "formula.kind")?;
    match kind {
        "boolean" => {
            known_keys(fields, "formula[boolean]", &["kind", "value"])?;
            Ok(Formula::boolean(bool_field(fields, "boolean.value")?))
        }
        "predicate" => {
            known_keys(fields, "formula[predicate]", &["args", "kind", "name"])?;
            Ok(Formula::predicate(
                identifier_field(fields, "predicate.name")?,
                decode_terms(fields.get("args"), "predicate.args", depth)?,
            ))
        }
        "action" => {
            known_keys(fields, "formula[action]", &["kind", "modality", "name"])?;
            let token = string_field(fields, "action.modality")?;
            let modality = ActionModality::from_wire(token).ok_or_else(|| {
                PropertyDecodeError::UnknownToken {
                    field: "action.modality",
                    token: token.to_owned(),
                }
            })?;
            Ok(Formula::action(
                identifier_field(fields, "action.name")?,
                modality,
            ))
        }
        "compare" => {
            known_keys(fields, "formula[compare]", &["kind", "left", "op", "right"])?;
            let token = string_field(fields, "compare.op")?;
            let op = ComparisonOperator::from_wire(token).ok_or_else(|| {
                PropertyDecodeError::UnknownToken {
                    field: "compare.op",
                    token: token.to_owned(),
                }
            })?;
            Ok(Formula::compare(
                op,
                decode_term(child(fields, "compare.left", "left")?, depth + 1)?,
                decode_term(child(fields, "compare.right", "right")?, depth + 1)?,
            ))
        }
        "not" => {
            known_keys(fields, "formula[not]", &["kind", "operand"])?;
            Ok(Formula::not(decode_formula(
                child(fields, "not.operand", "operand")?,
                depth + 1,
            )?))
        }
        "and" | "or" => {
            known_keys(fields, "formula[junction]", &["kind", "operands"])?;
            let operands = child(fields, "junction.operands", "operands")?
                .as_array()
                .ok_or(PropertyDecodeError::TypeMismatch {
                    field: "junction.operands",
                    expected: "array",
                    found: "non-array",
                })?
                .iter()
                .map(|operand| decode_formula(operand, depth + 1))
                .collect::<Result<Vec<_>, _>>()?;
            if kind == "and" {
                Ok(Formula::and(operands)?)
            } else {
                Ok(Formula::or(operands)?)
            }
        }
        "implies" | "leads_to" => {
            known_keys(
                fields,
                "formula[implication]",
                &["antecedent", "consequent", "kind"],
            )?;
            let antecedent = decode_formula(
                child(fields, "implication.antecedent", "antecedent")?,
                depth + 1,
            )?;
            let consequent = decode_formula(
                child(fields, "implication.consequent", "consequent")?,
                depth + 1,
            )?;
            if kind == "implies" {
                Ok(Formula::implies(antecedent, consequent))
            } else {
                Ok(Formula::leads_to(antecedent, consequent))
            }
        }
        "iff" => {
            known_keys(fields, "formula[iff]", &["kind", "left", "right"])?;
            Ok(Formula::iff(
                decode_formula(child(fields, "iff.left", "left")?, depth + 1)?,
                decode_formula(child(fields, "iff.right", "right")?, depth + 1)?,
            ))
        }
        "always" | "eventually" => {
            known_keys(fields, "formula[temporal]", &["kind", "operand"])?;
            let operand = decode_formula(child(fields, "temporal.operand", "operand")?, depth + 1)?;
            if kind == "always" {
                Ok(Formula::always(operand))
            } else {
                Ok(Formula::eventually(operand))
            }
        }
        "forall" | "exists" => {
            known_keys(
                fields,
                "formula[quantification]",
                &["binder", "body", "kind"],
            )?;
            let binder_json = child(fields, "quantification.binder", "binder")?;
            let binder_fields = object(binder_json, "binder")?;
            known_keys(binder_fields, "binder", &["domain", "variable"])?;
            let binder = Binder::new(
                identifier_field(binder_fields, "binder.variable")?,
                decode_term(child(binder_fields, "binder.domain", "domain")?, depth + 1)?,
            );
            let body = decode_formula(child(fields, "quantification.body", "body")?, depth + 1)?;
            if kind == "forall" {
                Ok(Formula::forall(binder, body))
            } else {
                Ok(Formula::exists(binder, body))
            }
        }
        // `next`, a `cpnf-2` node, or a typo: all the same answer.
        other => Err(PropertyDecodeError::UnknownToken {
            field: "formula.kind",
            token: other.to_owned(),
        }),
    }
}

fn decode_terms(
    json: Option<&Json>,
    field: &'static str,
    depth: usize,
) -> Result<Vec<Term>, PropertyDecodeError> {
    match json {
        // An absent optional list reads as empty; the encoder always writes it.
        None => Ok(Vec::new()),
        Some(Json::Array(items)) => items
            .iter()
            .map(|item| decode_term(item, depth + 1))
            .collect(),
        Some(other) => Err(PropertyDecodeError::TypeMismatch {
            field,
            expected: "array",
            found: other.type_name(),
        }),
    }
}

fn decode_term(json: &Json, depth: usize) -> Result<Term, PropertyDecodeError> {
    if depth >= MAX_DEPTH {
        return Err(PropertyDecodeError::Ast(AstError::TooDeep {
            max: MAX_DEPTH,
        }));
    }
    let fields = object(json, "term")?;
    let kind = string_field(fields, "term.kind")?;
    match kind {
        "var" => {
            known_keys(fields, "term[var]", &["kind", "name"])?;
            Ok(Term::Var {
                name: identifier_field(fields, "var.name")?,
            })
        }
        "constant" => {
            known_keys(fields, "term[constant]", &["kind", "name"])?;
            Ok(Term::Constant {
                name: identifier_field(fields, "constant.name")?,
            })
        }
        "literal" => {
            known_keys(fields, "term[literal]", &["kind", "value"])?;
            let value = fields
                .get("value")
                .ok_or(PropertyDecodeError::MissingField {
                    field: "literal.value",
                })?;
            let literal = match value {
                Json::Bool(v) => Literal::Boolean(*v),
                Json::Integer(v) => Literal::Integer(*v),
                Json::String(v) => Literal::Text(v.clone()),
                Json::Null => Literal::Null,
                other => {
                    return Err(PropertyDecodeError::TypeMismatch {
                        field: "literal.value",
                        expected: "boolean, integer, string, or null",
                        found: other.type_name(),
                    });
                }
            };
            Ok(Term::Literal { value: literal })
        }
        "state" => {
            known_keys(fields, "term[state]", &["indices", "kind", "name"])?;
            Ok(Term::State {
                name: identifier_field(fields, "state.name")?,
                indices: decode_terms(fields.get("indices"), "state.indices", depth)?,
            })
        }
        "apply" => {
            known_keys(fields, "term[apply]", &["args", "kind", "operator"])?;
            let args = decode_terms(fields.get("args"), "apply.args", depth)?;
            Ok(Term::apply(
                identifier_field(fields, "apply.operator")?,
                args,
            )?)
        }
        other => Err(PropertyDecodeError::UnknownToken {
            field: "term.kind",
            token: other.to_owned(),
        }),
    }
}

fn child<'a>(
    fields: &'a BTreeMap<String, Json>,
    field: &'static str,
    key: &str,
) -> Result<&'a Json, PropertyDecodeError> {
    fields
        .get(key)
        .ok_or(PropertyDecodeError::MissingField { field })
}

fn bool_field(
    fields: &BTreeMap<String, Json>,
    field: &'static str,
) -> Result<bool, PropertyDecodeError> {
    let key = field.rsplit('.').next().unwrap_or(field);
    let value = fields
        .get(key)
        .ok_or(PropertyDecodeError::MissingField { field })?;
    value
        .as_bool()
        .ok_or_else(|| PropertyDecodeError::TypeMismatch {
            field,
            expected: "boolean",
            found: value.type_name(),
        })
}

fn identifier_field(
    fields: &BTreeMap<String, Json>,
    field: &'static str,
) -> Result<Identifier, PropertyDecodeError> {
    Ok(Identifier::new(string_field(fields, field)?)?)
}
