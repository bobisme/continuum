//! The pack's **assurance**: a class and the nine-dimension B11 envelope — the third word
//! of PR-11 / IMPL-01 (RFC 0028, "Required fields, reconciled with plan §6.2", the "exact
//! verdict and assurance envelope" row; RFC 0028, "Verdict and assurance").
//!
//! # The two required members
//!
//! > `assurance.class` is one of the five `AssuranceClass` tokens; `assurance.envelope`
//! > carries all nine B11 dimensions.
//! >
//! > — RFC 0028, "Required fields, reconciled with plan §6.2"
//!
//! `context-pack.schema.json` types `assurance` as an object with `required: [class,
//! envelope]` and `additionalProperties: false`, so [`Assurance`] has exactly two fields
//! and no room for a third.
//!
//! # Both members are reused, not restated
//!
//! `continuum_value::assurance` is this workspace's home for the B11 vocabulary, and both
//! halves of this key are already there:
//!
//! - **The class** is [`AssuranceLevel`], whose five tokens are the schema's
//!   (`observed`, `sampled`, `bounded`, `validated`, `proved`) and whose derived [`Ord`] is
//!   declaration order — which is exactly what RFC 0028 asks of this field: "`assurance.class`
//!   is an assurance *class*: `observed < sampled < bounded < validated < proved`, RFC 0031's
//!   total order." A fourth copy of the ladder here would have to re-derive that order, and
//!   the one type that already carries it with RFC 0031 cited is the right one to name. The
//!   type's own documentation frames it as what an Intent Contract *requires*; the schema
//!   spells the same five tokens under the same RFC 0031 order for what a result *reached*,
//!   and RFC 0028 names no distinction between the two orders. (`continuumd` declares its
//!   own `AssuranceClass` beside it, but for a reason that does not apply here: its
//!   `protocol_enum!` types exist to generate a wire codec with `ProtocolEnum::from_wire`,
//!   not to state the vocabulary.)
//! - **The envelope** is [`AssuranceEnvelope`], which is the mirror of
//!   `assurance-result.schema.json`'s `assurance_envelope` — the same object the pack's
//!   schema carries with the comment "mirrored from assurance-result.schema.json;
//!   identity-checked by the validator". It cannot be constructed with a dimension missing:
//!   "every constructor names all nine dimensions, and a dimension either names its
//!   producing engine or carries a typed `UnsupportedReason`". That is `rule
//!   envelope.assurance_required` ("Silent omission of a dimension is forbidden") as a
//!   property of the type rather than a check over a document, so
//!   [`Assurance::to_json`] has nine keys for the same reason it has no validation step.
//!
//! # What this key does *not* rank, and does not carry
//!
//! > Evidence *classes* — RFC 0010's thirteen — are a partial order, not a ladder, and MUST
//! > NOT be ranked or collapsed into the class field. A production observation is not
//! > "higher" or "lower" than an inductive proof.
//! >
//! > — RFC 0028, "Verdict and assurance"
//!
//! [`Assurance`] has no evidence-class field, and `continuum_value::assurance::EvidenceClass`
//! deliberately derives no [`Ord`], so there is no expression in this crate that ranks two
//! evidence classes or folds one into [`Assurance::class`]. Evidence roots reach a pack
//! through its `evidence[]` key (`ev_*` handles), which is not this bullet's.
//!
//! An assurance block carries **no artifact handle**: the schema's `assurance` object has
//! exactly `class` and `envelope`, and this type has no field one could occupy. That is the
//! per-key artifact discipline for this key — [`crate::target`] carries the one handle this
//! bullet touches (`in_*`, class-checked), and [`crate::verdict`] carries none either.
//!
//! [`Assurance`] derives no [`Ord`] for a second reason of its own: [`AssuranceEnvelope`]
//! does not, and an envelope is a product of nine independent dimensions — "these dimensions
//! form a product lattice; they are not collapsed into a single 'level 5'" (docs/03 §3).
//! Ordering the *class* is licensed; ordering an assurance *block* is not.
//!
//! # What is declined
//!
//! - **The identity check against `assurance-result.schema.json`.** RFC 0028: "the schema's
//!   copy is identity-checked against `assurance-result.schema.json` by the dossier
//!   validator (plan §25 SD-12), so the two cannot drift." That validator is
//!   `notes/plan/tools/validate_dossier.py`, runs in `just check`, and compares the two
//!   schema documents. This module reuses the one Rust type both documents describe, which
//!   is how it stays on the right side of that check; it does not re-implement it.
//! - **"A pack MUST NOT report an envelope stronger than the result's."** That is a
//!   comparison between a pack and the `assurance-result` artifact it explains. No result
//!   artifact reaches this crate — `continuum-context` depends on `continuum-value`,
//!   `continuum-workspace` and `continuum-intent` and on nothing that produces one — so the
//!   comparison has no second operand here and is `continuumd`'s (it is the same wire rule
//!   RFC 0028's "Wire surface" states as `EvaluationVerdictValue.assurance_class` equalling
//!   the pack's `assurance.class`).
//! - **Reporting a narrowing in the manifest.** "A compile that narrows the envelope —
//!   through redaction, or through a budget-dropped dimension — MUST report the narrowing in
//!   the manifest." The manifest is [`crate::omission`] (IMPL-04) and the narrowing is
//!   produced by a compile stage no bullet has built; this type is the value that would be
//!   narrowed, not the stage that narrows it.
//!
//! # Clause → test
//!
//! | Clause | Source | Test |
//! |---|---|---|
//! | the block is exactly `{class, envelope}` | `context-pack.schema.json` `assurance` | `an_assurance_block_has_exactly_the_two_required_members` |
//! | the five class tokens are the schema's, in RFC 0031's order | `context-pack.schema.json`; RFC 0031 | `the_class_vocabulary_is_the_schema_enum_in_the_ratified_order` |
//! | all nine dimensions are present, always | plan §3 B11; `rule envelope.assurance_required` | `every_envelope_names_all_nine_dimensions` |
//! | a dimension names an engine or carries a typed reason, never both and never neither | `context-pack.schema.json` `$defs.envelope_dimension` | `each_dimension_is_one_of_the_two_admitted_shapes` |
//! | the encoding matches the shipped example's shape | `schemas/examples/context-pack.example.json` | `the_encoding_matches_the_shipped_examples_shape` |
//! | an assurance block carries no artifact handle | `context-pack.schema.json` (`additionalProperties: false`) | `an_assurance_block_carries_no_handle` |
//! | two independent builds are byte-identical, and distinct blocks never are | `rule ordering.deterministic`, docs/19 §7 | `two_builds_of_one_block_are_byte_identical` |
//! | dimension order in the encoding does not depend on the order they were set | RFC 0028, "Determinism" | `the_encoding_does_not_depend_on_the_order_dimensions_were_set` |

use continuum_intent::canonical_json::Json;
use continuum_value::assurance::{
    AssuranceDimension, AssuranceEnvelope, AssuranceLevel, DimensionEvidence,
};

/// The pack's `assurance` block: an assurance class and the B11 envelope.
///
/// No [`Ord`]: see the module documentation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Assurance {
    class: AssuranceLevel,
    envelope: AssuranceEnvelope,
}

impl Assurance {
    /// The one pack key an assurance block contributes.
    pub const KEYS: [&'static str; 1] = ["assurance"];

    /// The block's two required members, in the schema's spelling.
    pub const MEMBERS: [&'static str; 2] = ["class", "envelope"];

    /// State the class this result is supported at, and the envelope it holds within.
    ///
    /// Infallible on purpose: an [`AssuranceEnvelope`] cannot omit a dimension and an
    /// [`AssuranceLevel`] cannot be a token outside the closed five, so there is no
    /// malformed block to reject.
    #[must_use]
    pub const fn new(class: AssuranceLevel, envelope: AssuranceEnvelope) -> Self {
        Self { class, envelope }
    }

    /// The assurance class.
    #[must_use]
    pub const fn class(&self) -> AssuranceLevel {
        self.class
    }

    /// The B11 envelope: nine dimensions, each naming an engine or a typed reason.
    #[must_use]
    pub const fn envelope(&self) -> &AssuranceEnvelope {
        &self.envelope
    }

    /// The `assurance` object itself: `{"class", "envelope"}`, ID5-sorted.
    ///
    /// # Panics
    ///
    /// Never: [`Self::MEMBERS`] are distinct string literals, and
    /// `AssuranceDimension::ALL`'s nine names are pairwise distinct.
    #[must_use]
    pub fn to_json(&self) -> Json {
        Json::object([
            (
                "class".to_owned(),
                Json::String(self.class.as_str().to_owned()),
            ),
            ("envelope".to_owned(), envelope_json(&self.envelope)),
        ])
        .expect("`class` and `envelope` are distinct string literals")
    }

    /// The `(key, value)` pairs this block contributes to a pack document.
    ///
    /// A *fragment* of a pack, never a pack.
    #[must_use]
    pub fn to_json_fields(&self) -> Vec<(String, Json)> {
        vec![("assurance".to_owned(), self.to_json())]
    }

    /// The canonical ID5 bytes of [`to_json_fields`](Self::to_json_fields) as one object.
    ///
    /// # Panics
    ///
    /// Never: [`Self::KEYS`] is a single string literal.
    #[must_use]
    pub fn to_canonical_bytes(&self) -> Vec<u8> {
        Json::object(self.to_json_fields())
            .expect("`assurance` is a single string literal")
            .to_canonical_bytes()
    }
}

/// Encode the nine dimensions, each in one of the schema's two admitted shapes.
///
/// `{"engine", "summary"}` or `{"unsupported": {"reason"}}` —
/// `context-pack.schema.json`'s `$defs.envelope_dimension` `oneOf`, which admits no third
/// and no absence.
fn envelope_json(envelope: &AssuranceEnvelope) -> Json {
    Json::object(envelope.entries().map(|entry| {
        let value = match entry.evidence {
            DimensionEvidence::Produced(produced) => Json::object([
                (
                    "engine".to_owned(),
                    Json::String(produced.engine().to_owned()),
                ),
                (
                    "summary".to_owned(),
                    Json::String(produced.summary().to_owned()),
                ),
            ])
            .expect("`engine` and `summary` are distinct string literals"),
            DimensionEvidence::Unsupported(reason) => Json::object([(
                "unsupported".to_owned(),
                Json::object([("reason".to_owned(), Json::String(reason.to_string()))])
                    .expect("`reason` is a single string literal"),
            )])
            .expect("`unsupported` is a single string literal"),
        };
        (entry.dimension.as_str().to_owned(), value)
    }))
    .expect("`AssuranceDimension::ALL`'s nine names are pairwise distinct")
}

/// The nine dimension names an envelope always carries, in the schema's `required` order.
///
/// Re-exported from `continuum_value::assurance` so a reader of this module can see the set
/// the schema's `assurance_envelope.required` fixes without leaving it.
#[must_use]
pub const fn dimensions() -> [AssuranceDimension; 9] {
    AssuranceDimension::ALL
}

#[cfg(test)]
mod tests {
    use core::str::FromStr;

    use continuum_value::assurance::UnsupportedReason;
    use continuum_workspace::artifact_path::ArtifactHandle;

    use super::*;

    fn reason(token: &str) -> UnsupportedReason {
        UnsupportedReason::new(token).expect("canonical test token")
    }

    fn produced(engine: &str, summary: &str) -> DimensionEvidence {
        DimensionEvidence::produced(engine, summary).expect("named producer")
    }

    /// The shipped example's block: `bounded`, eight produced dimensions and
    /// `memory_model` reading `Unsupported(sequential-consistency-only)`.
    fn example_block() -> Assurance {
        let envelope = AssuranceEnvelope::all_unsupported(&reason("unattempted"))
            .with(
                AssuranceDimension::Bounds,
                produced("continuum-explicit", "faults=1, nodes=2"),
            )
            .with(
                AssuranceDimension::MemoryModel,
                DimensionEvidence::Unsupported(UnsupportedReason::sequential_consistency_only()),
            );
        Assurance::new(AssuranceLevel::Bounded, envelope)
    }

    /// Transcribed from `notes/plan/schemas/context-pack.schema.json`,
    /// `properties.assurance.properties.class.enum`, in file order.
    const SCHEMA_CLASS_ENUM: [&str; 5] = ["observed", "sampled", "bounded", "validated", "proved"];

    /// Transcribed from the same file, `$defs.assurance_envelope.required`, in file order.
    const SCHEMA_DIMENSIONS: [&str; 9] = [
        "bounds",
        "faults",
        "fairness",
        "values",
        "schedules",
        "memory_model",
        "observer",
        "proof_status",
        "unknowns",
    ];

    #[test]
    fn the_class_vocabulary_is_the_schema_enum_in_the_ratified_order() {
        let ours: Vec<&str> = AssuranceLevel::ALL
            .iter()
            .map(|level| level.as_str())
            .collect();
        assert_eq!(ours, SCHEMA_CLASS_ENUM);
        // RFC 0028: "`observed < sampled < bounded < validated < proved`, RFC 0031's total
        // order" — the field's order, not merely its members.
        let mut sorted = AssuranceLevel::ALL;
        sorted.sort_unstable();
        assert_eq!(sorted, AssuranceLevel::ALL);
    }

    #[test]
    fn an_assurance_block_has_exactly_the_two_required_members() {
        let block = example_block();
        let json = block.to_json();
        let members = json.as_object().expect("object");
        let names: Vec<&str> = members.keys().map(String::as_str).collect();
        assert_eq!(names, Assurance::MEMBERS.to_vec());
        assert_eq!(members["class"].as_str(), Some("bounded"));
        assert_eq!(block.class(), AssuranceLevel::Bounded);
    }

    #[test]
    fn every_envelope_names_all_nine_dimensions() {
        let block = example_block();
        let json = block.to_json();
        let envelope = json.as_object().expect("object")["envelope"]
            .as_object()
            .expect("object");
        assert_eq!(envelope.len(), 9);
        for dimension in SCHEMA_DIMENSIONS {
            assert!(
                envelope.contains_key(dimension),
                "{dimension} must be named"
            );
        }
        assert_eq!(dimensions().len(), 9);
    }

    #[test]
    fn each_dimension_is_one_of_the_two_admitted_shapes() {
        let json = example_block().to_json();
        let envelope = json.as_object().expect("object")["envelope"]
            .as_object()
            .expect("object");
        for (name, value) in envelope {
            let fields = value.as_object().expect("dimension is an object");
            let keys: Vec<&str> = fields.keys().map(String::as_str).collect();
            assert!(
                keys == ["engine", "summary"] || keys == ["unsupported"],
                "{name} has shape {keys:?}, which the schema's `oneOf` does not admit"
            );
            if keys == ["unsupported"] {
                let inner = fields["unsupported"].as_object().expect("object");
                assert_eq!(
                    inner.keys().map(String::as_str).collect::<Vec<_>>(),
                    ["reason"]
                );
                assert!(inner["reason"].as_str().is_some_and(|r| !r.is_empty()));
            }
        }
    }

    #[test]
    fn the_encoding_matches_the_shipped_examples_shape() {
        // `schemas/examples/context-pack.example.json`'s own `assurance` block: a
        // `bounds` dimension naming its engine and a summary, and `memory_model` reading
        // the ADR-0032 reason.
        let json = example_block().to_json();
        let envelope = json.as_object().expect("object")["envelope"]
            .as_object()
            .expect("object");
        let bounds = envelope["bounds"].as_object().expect("object");
        assert_eq!(bounds["engine"].as_str(), Some("continuum-explicit"));
        assert_eq!(bounds["summary"].as_str(), Some("faults=1, nodes=2"));
        let memory = envelope["memory_model"].as_object().expect("object")["unsupported"]
            .as_object()
            .expect("object");
        assert_eq!(
            memory["reason"].as_str(),
            Some("sequential-consistency-only")
        );
    }

    #[test]
    fn an_assurance_block_carries_no_handle() {
        // The schema's `assurance` object is `{class, envelope}` with
        // `additionalProperties: false`, and each dimension is `{engine, summary}` or
        // `{unsupported}` — so there is no key an artifact handle could live under, and
        // this type has no field one could occupy. Checked against the workspace's own
        // handle parser rather than a prefix heuristic: `proof_status` is a dimension
        // *name* that begins with a registered class token and is not a handle.
        let json = example_block().to_json();
        let block = json.as_object().expect("object");
        assert_eq!(
            block.keys().map(String::as_str).collect::<Vec<_>>(),
            Assurance::MEMBERS.to_vec()
        );
        let mut strings = vec![block["class"].as_str().expect("class is a string")];
        let envelope = block["envelope"].as_object().expect("object");
        for value in envelope.values() {
            collect_strings(value, &mut strings);
        }
        assert!(strings.len() >= 10, "class plus nine dimensions");
        for value in strings {
            assert!(
                ArtifactHandle::from_str(value).is_err(),
                "{value:?} must not be a plan §4.4 handle"
            );
        }
    }

    fn collect_strings<'a>(value: &'a Json, into: &mut Vec<&'a str>) {
        match value {
            Json::String(text) => into.push(text),
            Json::Object(fields) => {
                for nested in fields.values() {
                    collect_strings(nested, into);
                }
            }
            _ => panic!("an envelope dimension carries only strings and objects"),
        }
    }

    #[test]
    fn two_builds_of_one_block_are_byte_identical() {
        assert_eq!(
            example_block().to_canonical_bytes(),
            example_block().to_canonical_bytes()
        );
        // And not vacuously: the class and any dimension both move the bytes.
        let other_class = Assurance::new(AssuranceLevel::Proved, example_block().envelope.clone());
        assert_ne!(
            example_block().to_canonical_bytes(),
            other_class.to_canonical_bytes()
        );
        let other_dimension = Assurance::new(
            AssuranceLevel::Bounded,
            example_block().envelope.clone().with(
                AssuranceDimension::Faults,
                produced("continuum-dpor", "f=1"),
            ),
        );
        assert_ne!(
            example_block().to_canonical_bytes(),
            other_dimension.to_canonical_bytes()
        );
    }

    #[test]
    fn the_encoding_does_not_depend_on_the_order_dimensions_were_set() {
        let base = AssuranceEnvelope::all_unsupported(&reason("unattempted"));
        let forwards = base
            .clone()
            .with(AssuranceDimension::Bounds, produced("e1", "s1"))
            .with(AssuranceDimension::Unknowns, produced("e2", "s2"));
        let backwards = base
            .with(AssuranceDimension::Unknowns, produced("e2", "s2"))
            .with(AssuranceDimension::Bounds, produced("e1", "s1"));
        assert_eq!(
            Assurance::new(AssuranceLevel::Observed, forwards).to_canonical_bytes(),
            Assurance::new(AssuranceLevel::Observed, backwards).to_canonical_bytes()
        );
    }
}
