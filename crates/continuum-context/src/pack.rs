//! The expansion child document: what a `context.expand` answer *is*, as an artifact.
//!
//! # This is derivation, not compilation
//!
//! Assembling a Context Pack from evidence is the compiler — stages 1–10 of RFC 0028's
//! pipeline, spread across PR-11's other bullets (target/verdict/assurance, the state and
//! event slice, the replay reference, byte and token budgets) and no single bullet's fence.
//! [`crate::selection`] recorded that decline when IMPL-03 landed, and this module does not
//! reverse it: **nothing here synthesizes a pack field out of nothing.**
//!
//! What it does is the operation RFC 0028 actually specifies for an expansion:
//!
//! > The child inherits identity, not guarantees. `snapshot`, `intent`, and
//! > `semantic_epoch` MUST equal the parent's; a compile under a different semantic epoch
//! > is a recompile, not an expansion. `guarantees` are recomputed for the child (C5).
//! > `question` is the canonical encoding of the expansion query.
//! >
//! > — RFC 0028, "Expansion protocol"
//!
//! So a child is its parent's document with [`DERIVED_KEYS`] replaced and
//! [`INHERITED_KEYS`] copied *verbatim from a document this crate did not write*. If the
//! parent validates against `context-pack.schema.json`, so does the child: every inherited
//! value is already of the shape the schema fixes for its key, and every derived value is
//! one this bullet owns. A parent missing an inherited key is refused
//! ([`PackError::MissingField`]) rather than defaulted — inventing a `verdict` or an
//! `assurance` envelope for a pack is exactly the fabrication the decline above is about.
//!
//! # The three keys the expansion decides, and the reading behind each
//!
//! - **`guarantees` is empty.** C5: "an expansion child's guarantees are computed for the
//!   child. A child MUST NOT inherit its parent's guarantees". This bullet runs no
//!   guarantee checker, and C1's "a daemon MUST NOT echo a requested guarantee it did not
//!   achieve" settles what an unchecked set is: the empty one. Not a gap — a claim, and the
//!   only true one available.
//! - **`verdict` and `assurance` are inherited.** They are statements about *the evaluation
//!   the pack explains*, and an expansion explains the same evaluation — "a child of an
//!   already-decided question" is the IDL's own reason for `context.expand` declaring no
//!   verdict. Inheriting is equal, never stronger, so "a pack MUST NOT report an envelope
//!   stronger than the result's" is preserved by construction.
//! - **`content_budget.bytes` is the ceiling in force**, not a measurement of the bytes
//!   written. The schema calls it "byte budget of the pack"; the caller's request may name
//!   one, and where it does not the parent's stands. What this crate does *not* do is pack
//!   to it — dropping items to fit a byte ceiling is IMPL-06's bullet — so a caller of
//!   [`ChildPack::to_json`] that names a ceiling is obliged to check the result against it
//!   and refuse rather than over-run it; [`ChildPack::within`] is that check.
//!
//! # `content_hash`, and the self-reference every content-addressed document has
//!
//! `content_hash` is "the canonical content identity of the pack (ADR-0013)" and is itself
//! a key of the pack, so hashing the document that contains it is circular. The identity
//! preimage is therefore the child's canonical encoding **with `content_hash` absent and
//! every other key present**, and the digest is written back into that key. Stated once,
//! here, so a reader is never left to guess which of the two obvious readings a token
//! came from.
//!
//! `context_id` is *not* that digest. It is [`crate::expansion::ExpansionHandle`], derived
//! from the question — see that module for why an expansion's identity is a function of its
//! request rather than of its bytes.
//!
//! # Clause → test
//!
//! | Clause | Source | Test |
//! |---|---|---|
//! | the child carries the schema's seventeen required keys | `context-pack.schema.json` `required` | `a_child_carries_every_required_key` |
//! | `snapshot`, `intent`, `semantic_epoch` equal the parent's | RFC 0028, "Expansion protocol" | `the_child_inherits_identity_verbatim` |
//! | guarantees are not inherited | RFC 0028 C5 | `the_child_inherits_no_guarantee` |
//! | the child names its parent | RFC 0028 F1, paid in the schema | `the_child_names_its_parent` |
//! | a parent missing an inherited key is refused | this module's no-fabrication rule | `a_parent_missing_an_inherited_key_is_refused` |
//! | an anchor resolves in the parent | RFC 0028, "Expansion protocol" | `anchors_are_the_parents_own_ids_and_queries` |
//! | the document is deterministic | `rule ordering.deterministic`, docs/19 §7 | `two_builds_of_one_child_are_byte_identical` |

use core::fmt;
use core::str::FromStr;
use std::collections::{BTreeMap, BTreeSet};

use continuum_intent::canonical_json::Json;
use continuum_value::identity::ContentHasher;
use continuum_value::value::Name;
use continuum_workspace::artifact_path::{ArtifactClass, ArtifactHandle};

use crate::expansion::{ExpansionHandle, ExpansionQuestion};
use crate::omission::Manifest;
use crate::selection::SelectedItem;

/// The schema's `required` list, transcribed from `context-pack.schema.json`.
pub const REQUIRED_KEYS: [&str; 17] = [
    "schema_id",
    "schema_epoch",
    "context_id",
    "snapshot",
    "intent",
    "question",
    "verdict",
    "assurance",
    "selected",
    "omissions",
    "expansions",
    "evidence",
    "replay",
    "guarantees",
    "semantic_epoch",
    "content_hash",
    "content_budget",
];

/// The keys a child takes from its parent verbatim.
///
/// Three of them — `snapshot`, `intent`, `semantic_epoch` — are RFC 0028's own MUST. The
/// other five are inherited on the reading in this module's documentation: they describe
/// the evaluation the pack explains, which an expansion does not change.
pub const INHERITED_KEYS: [&str; 8] = [
    "assurance",
    "evidence",
    "intent",
    "replay",
    "schema_epoch",
    "schema_id",
    "semantic_epoch",
    "snapshot",
];

/// Keys a child inherits when the parent has them, and omits when it does not.
///
/// `verdict` is required by the schema and is in [`INHERITED_KEYS`]; these four are the
/// schema's optional properties, and copying them keeps a child readable by anything that
/// read its parent.
pub const OPTIONAL_INHERITED_KEYS: [&str; 4] = [
    "compiler",
    "debugger_branch",
    "inconclusive_reason",
    "redactions",
];

/// The keys the expansion decides for itself.
pub const DERIVED_KEYS: [&str; 9] = [
    "content_budget",
    "content_hash",
    "context_id",
    "expansions",
    "guarantees",
    "omissions",
    "parent",
    "question",
    "selected",
];

/// The parent's own `context_id`, as a typed handle.
///
/// # Errors
///
/// [`PackError`] when the document is not an object, has no `context_id`, or carries one
/// that is not a `ctx_*` handle.
pub fn identity_of(parent: &Json) -> Result<ArtifactHandle, PackError> {
    let text = string_field(parent, "context_id")?;
    let handle = ArtifactHandle::from_str(text).map_err(|_| PackError::NotAPackHandle)?;
    if handle.class() != ArtifactClass::ContextPack {
        return Err(PackError::NotAPackHandle);
    }
    Ok(handle)
}

/// Whether a document carries the schema's seventeen required keys.
///
/// Not schema *validation* — that is the dossier validator's job over the normative schema
/// document, and re-implementing JSON Schema here would be a second validator to disagree
/// with it. This is the narrower question a derivation has to answer before it starts: is
/// there a value under each key a child inherits or a reader expects? A daemon holding a
/// pack it cannot expand should discover that when the pack is registered, not when a
/// caller asks.
///
/// # Errors
///
/// [`PackError`] naming the first key that is absent.
pub fn required_keys_present(document: &Json) -> Result<(), PackError> {
    for key in REQUIRED_KEYS {
        field(document, key)?;
    }
    Ok(())
}

/// The byte ceiling the parent recorded — `content_budget.bytes`.
///
/// # Errors
///
/// [`PackError`] when `content_budget` is absent or carries no non-negative integer
/// `bytes`. The schema requires both, so a parent without them is not a conforming pack and
/// the honest answer is a refusal rather than a ceiling this crate chose.
pub fn budget_bytes_of(parent: &Json) -> Result<u64, PackError> {
    let budget = field(parent, "content_budget")?;
    let bytes = budget
        .as_object()
        .and_then(|fields| fields.get("bytes"))
        .and_then(Json::as_integer)
        .ok_or(PackError::MissingField {
            key: "content_budget.bytes",
        })?;
    u64::try_from(bytes).map_err(|_| PackError::NegativeBudget { bytes })
}

/// Every anchor that resolves in this pack.
///
/// > `anchor` MUST resolve in the parent: it is either a `selected[].id` or an anchor named
/// > by one of the parent's own `expansions[]` or `omissions[].expansion` entries.
/// > Expansion is navigation over a published pack, not a general graph query.
/// >
/// > — RFC 0028, "Expansion protocol"
///
/// Read off the published artifact rather than off a side table the daemon keeps, because
/// the artifact is what the caller was given and what it navigates by. A daemon whose
/// private index disagreed with the pack it published would be refusing anchors the pack
/// itself advertises.
///
/// # Errors
///
/// [`PackError`] when the document's shape is not the schema's, or when an anchor is not
/// spellable as a [`Name`].
pub fn anchors(parent: &Json) -> Result<BTreeSet<Name>, PackError> {
    let mut out = BTreeSet::new();
    for item in array_field(parent, "selected")? {
        out.insert(anchor_name(string_field(item, "id")?)?);
    }
    for item in array_field(parent, "expansions")? {
        out.insert(anchor_name(string_field(item, "anchor")?)?);
    }
    for item in array_field(parent, "omissions")? {
        let Some(expansion) = item.as_object().and_then(|fields| fields.get("expansion")) else {
            continue;
        };
        out.insert(anchor_name(string_field(expansion, "anchor")?)?);
    }
    Ok(out)
}

/// The child pack an expansion answers with, before it is written.
#[derive(Debug, Clone, Copy)]
pub struct ChildPack<'a> {
    /// The published parent document. Everything not derived is copied from it verbatim.
    pub parent: &'a Json,
    /// The child's own `ctx_*`, derived from the question (see [`ExpansionHandle`]).
    pub identity: &'a ExpansionHandle,
    /// The canonical encoding of the triple this child answers.
    pub question: &'a ExpansionQuestion,
    /// The items the expansion retrieved.
    pub selected: &'a [SelectedItem],
    /// What this child, in turn, leaves out — the residual manifest.
    pub manifest: &'a Manifest,
    /// The byte ceiling in force. Recorded, not enforced here — see [`ChildPack::within`].
    pub budget_bytes: u64,
}

impl ChildPack<'_> {
    /// Write the child document.
    ///
    /// # Errors
    ///
    /// [`PackError`] when the parent is not a conforming pack this child can be derived
    /// from: not an object, missing an inherited key, or carrying a `context_id` that is
    /// not a `ctx_*` handle.
    pub fn to_json<H: ContentHasher>(&self) -> Result<Json, PackError> {
        let parent_id = identity_of(self.parent)?;
        let mut fields: BTreeMap<String, Json> = BTreeMap::new();

        for key in INHERITED_KEYS {
            fields.insert((*key).to_owned(), field(self.parent, key)?.clone());
        }
        // `verdict` is required and inherited, and is listed apart from the eight above only
        // because this module's documentation argues for it separately.
        fields.insert("verdict".to_owned(), field(self.parent, "verdict")?.clone());
        for key in OPTIONAL_INHERITED_KEYS {
            if let Some(value) = self.parent.as_object().and_then(|it| it.get(key)) {
                fields.insert((*key).to_owned(), value.clone());
            }
        }

        fields.insert(
            "context_id".to_owned(),
            Json::String(self.identity.to_string()),
        );
        fields.insert("parent".to_owned(), Json::String(parent_id.to_string()));
        fields.insert(
            "question".to_owned(),
            Json::String(self.question.as_str().to_owned()),
        );
        fields.insert(
            "selected".to_owned(),
            Json::Array(self.selected.iter().map(SelectedItem::to_json).collect()),
        );
        fields.insert("omissions".to_owned(), self.manifest.to_json());
        fields.insert("expansions".to_owned(), self.manifest.expansions_json());
        // C5/C1: a guarantee is a checked claim, and this bullet checked none.
        fields.insert("guarantees".to_owned(), Json::Array(Vec::new()));
        fields.insert(
            "content_budget".to_owned(),
            Json::object([(
                "bytes".to_owned(),
                Json::Integer(i64::try_from(self.budget_bytes).unwrap_or(i64::MAX)),
            )])
            .expect("one key"),
        );

        // The identity preimage: this document, `content_hash` absent, every other key
        // present. See the module documentation.
        let preimage = Json::Object(fields.clone()).to_canonical_bytes();
        fields.insert(
            "content_hash".to_owned(),
            Json::String(format!(
                "{}:{}",
                H::ALGORITHM.token(),
                H::hash(&preimage).to_token()
            )),
        );
        Ok(Json::Object(fields))
    }

    /// Whether a written child fits the ceiling this expansion ran under.
    ///
    /// Separate from [`ChildPack::to_json`] because the two answers are different kinds of
    /// thing: the document is what the expansion found, and this is whether the caller's
    /// budget admits it. RFC 0028's typed-outcome table gives a daemon both branches for a
    /// budget it cannot meet — "either nothing is published, or a child pack is published
    /// whose manifest records the shortfall with reason `budget`" — and the second branch
    /// needs the byte packer that is IMPL-06's bullet, so a caller of this function takes
    /// the first and refuses.
    #[must_use]
    pub fn within(&self, document: &Json) -> bool {
        document.to_canonical_bytes().len() as u64 <= self.budget_bytes
    }
}

/// A document this module will not derive a child from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PackError {
    /// The document is not a JSON object.
    NotAnObject {
        /// What it is instead.
        found: &'static str,
    },
    /// A key the child needs is absent from the parent.
    MissingField {
        /// The key.
        key: &'static str,
    },
    /// A key is present with the wrong JSON type.
    WrongType {
        /// The key.
        key: &'static str,
        /// The type the schema fixes for it.
        expected: &'static str,
    },
    /// `context_id` is not a `ctx_*` handle.
    NotAPackHandle,
    /// `content_budget.bytes` is negative.
    NegativeBudget {
        /// The value read.
        bytes: i64,
    },
    /// An anchor is not spellable as a canonical [`Name`].
    UnnameableAnchor {
        /// The offending text's length, never the text: it came off an artifact and a
        /// typed error carries no interpolated content (INV-016).
        length: usize,
    },
}

impl fmt::Display for PackError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotAnObject { found } => {
                write!(f, "a Context Pack is a JSON object, not a {found}")
            }
            Self::MissingField { key } => write!(
                f,
                "the parent pack declares no `{key}`, and an expansion child inherits it \
                 rather than inventing one"
            ),
            Self::WrongType { key, expected } => {
                write!(f, "`{key}` is not the {expected} the schema declares")
            }
            Self::NotAPackHandle => f.write_str("`context_id` is not a plan §4.4 `ctx_` handle"),
            Self::NegativeBudget { bytes } => {
                write!(
                    f,
                    "`content_budget.bytes` is {bytes}; a byte budget is not negative"
                )
            }
            Self::UnnameableAnchor { length } => write!(
                f,
                "an anchor of {length} bytes is not a canonical identifier, so nothing in \
                 the pack can carry it"
            ),
        }
    }
}

impl core::error::Error for PackError {}

fn field<'a>(document: &'a Json, key: &'static str) -> Result<&'a Json, PackError> {
    document
        .as_object()
        .ok_or(PackError::NotAnObject {
            found: document.type_name(),
        })?
        .get(key)
        .ok_or(PackError::MissingField { key })
}

fn string_field<'a>(document: &'a Json, key: &'static str) -> Result<&'a str, PackError> {
    field(document, key)?.as_str().ok_or(PackError::WrongType {
        key,
        expected: "string",
    })
}

fn array_field<'a>(document: &'a Json, key: &'static str) -> Result<&'a [Json], PackError> {
    field(document, key)?
        .as_array()
        .ok_or(PackError::WrongType {
            key,
            expected: "array",
        })
}

fn anchor_name(text: &str) -> Result<Name, PackError> {
    Name::new(text).map_err(|_| PackError::UnnameableAnchor { length: text.len() })
}

#[cfg(test)]
mod tests {
    use continuum_value::identity::Blake3Hasher;

    use super::*;
    use crate::expansion::{Depth, ExpansionQuery, ExpansionRelation};
    use crate::omission::{OmissionReason, OmissionRecord};
    use crate::selection::SelectionKind;

    /// A minimal conforming parent, written here as text so the test's fixture is a
    /// *document* rather than a construction this crate could get wrong in both places.
    const PARENT: &str = r#"{
      "assurance": {"class": "bounded", "envelope": {}},
      "content_budget": {"bytes": 16384},
      "content_hash": "blake3-256:aaaa",
      "context_id": "ctx_parent1",
      "evidence": ["ev_1"],
      "expansions": [{"anchor": "e_ack", "relation": "causal_predecessors"}],
      "guarantees": ["ReplayPreserving", "CausallyClosed"],
      "intent": "in_ack_v1",
      "omissions": [{"count": 2, "expandable": true,
                     "expansion": {"anchor": "e_ack", "relation": "source_span"},
                     "kind": "source", "reason": "budget"}],
      "parent": null,
      "question": "why did AckImpliesDurable fail?",
      "replay": "crash_1",
      "schema_epoch": 1,
      "schema_id": "https://continuum.dev/schema/context-pack.json",
      "selected": [{"artifact": null, "id": "e_ack", "kind": "event", "summary": "s"}],
      "semantic_epoch": "sem3-r3-demo",
      "snapshot": "ws_demo1",
      "verdict": "refuted"
    }"#;

    fn parent() -> Json {
        Json::parse(PARENT.as_bytes()).expect("the fixture is admissible JSON")
    }

    fn name(text: &str) -> Name {
        Name::new(text).expect("well formed")
    }

    fn query() -> ExpansionQuery {
        ExpansionQuery::new(ExpansionRelation::SourceSpan, name("e_ack"))
    }

    fn child(parent: &Json, manifest: &Manifest) -> Json {
        let identity = ExpansionHandle::derive::<Blake3Hasher>(
            &identity_of(parent).expect("a pack handle"),
            &query(),
            Depth::DEFAULT,
        )
        .expect("derives");
        let question = ExpansionQuestion::of(&query(), Depth::DEFAULT);
        ChildPack {
            parent,
            identity: &identity,
            question: &question,
            selected: &[],
            manifest,
            budget_bytes: 16384,
        }
        .to_json::<Blake3Hasher>()
        .expect("the parent is conforming")
    }

    #[test]
    fn a_child_carries_every_required_key() {
        let document = child(&parent(), &Manifest::empty());
        let fields = document.as_object().expect("object");
        for key in REQUIRED_KEYS {
            assert!(fields.contains_key(key), "the child has no `{key}`");
        }
        // And exactly the keys the schema admits: the seventeen required, `parent`, and
        // whichever optional keys the parent carried (this fixture carries none of the
        // four).
        let expected: BTreeSet<&str> = REQUIRED_KEYS.into_iter().chain(["parent"]).collect();
        let found: BTreeSet<&str> = fields.keys().map(String::as_str).collect();
        assert_eq!(found, expected);
    }

    #[test]
    fn the_child_inherits_identity_verbatim() {
        let parent = parent();
        let document = child(&parent, &Manifest::empty());
        let from = parent.as_object().expect("object");
        let to = document.as_object().expect("object");
        for key in INHERITED_KEYS {
            assert_eq!(to[key], from[key], "`{key}` must equal the parent's");
        }
        assert_eq!(to["verdict"], from["verdict"]);
    }

    #[test]
    fn the_child_inherits_no_guarantee() {
        let parent = parent();
        let document = child(&parent, &Manifest::empty());
        assert_eq!(
            parent.as_object().expect("object")["guarantees"]
                .as_array()
                .expect("array")
                .len(),
            2,
            "the fixture's parent claims two guarantees"
        );
        assert_eq!(
            document.as_object().expect("object")["guarantees"],
            Json::Array(Vec::new()),
            "C5: guarantees are recomputed for the child, and none was checked"
        );
    }

    #[test]
    fn the_child_names_its_parent() {
        let document = child(&parent(), &Manifest::empty());
        let fields = document.as_object().expect("object");
        assert_eq!(fields["parent"], Json::String("ctx_parent1".to_owned()));
        assert_ne!(fields["context_id"], fields["parent"]);
        assert!(
            fields["context_id"]
                .as_str()
                .expect("string")
                .starts_with("ctx_")
        );
    }

    #[test]
    fn the_childs_question_is_the_expansion_and_not_the_parents() {
        let parent = parent();
        let document = child(&parent, &Manifest::empty());
        assert_eq!(
            document.as_object().expect("object")["question"],
            Json::String(r#"{"anchor":"e_ack","depth":1,"relation":"source_span"}"#.to_owned())
        );
        assert_ne!(
            document.as_object().expect("object")["question"],
            parent.as_object().expect("object")["question"]
        );
    }

    #[test]
    fn the_manifest_and_the_expansions_it_makes_reachable_agree() {
        let manifest = Manifest::new([OmissionRecord::expandable(
            SelectionKind::Event,
            4,
            OmissionReason::Budget,
            ExpansionQuery::new(ExpansionRelation::CausalPredecessors, name("e_ack")),
        )])
        .expect("one cell");
        let document = child(&parent(), &manifest);
        let fields = document.as_object().expect("object");
        assert_eq!(fields["omissions"], manifest.to_json());
        assert_eq!(fields["expansions"], manifest.expansions_json());
        assert_eq!(fields["expansions"].as_array().expect("array").len(), 1);
    }

    #[test]
    fn a_parent_missing_an_inherited_key_is_refused() {
        for key in INHERITED_KEYS {
            let mut fields = parent().as_object().cloned().expect("object");
            fields.remove(key);
            let stripped = Json::Object(fields);
            let identity = ExpansionHandle::derive::<Blake3Hasher>(
                &ArtifactHandle::from_str("ctx_parent1").expect("a handle"),
                &query(),
                Depth::DEFAULT,
            )
            .expect("derives");
            let question = ExpansionQuestion::of(&query(), Depth::DEFAULT);
            let manifest = Manifest::empty();
            let outcome = ChildPack {
                parent: &stripped,
                identity: &identity,
                question: &question,
                selected: &[],
                manifest: &manifest,
                budget_bytes: 16384,
            }
            .to_json::<Blake3Hasher>();
            assert_eq!(
                outcome,
                Err(PackError::MissingField { key }),
                "a parent with no `{key}` must be refused, never defaulted"
            );
        }
    }

    #[test]
    fn anchors_are_the_parents_own_ids_and_queries() {
        // The fixture names `e_ack` three ways — as a `selected[].id`, as an
        // `expansions[].anchor`, and inside an omission's `expansion` — and nothing else.
        let found = anchors(&parent()).expect("a conforming parent");
        assert_eq!(found, BTreeSet::from([name("e_ack")]));
        assert!(!found.contains(&name("e_never")));
    }

    #[test]
    fn two_builds_of_one_child_are_byte_identical() {
        let parent = parent();
        let first = child(&parent, &Manifest::empty()).to_canonical_bytes();
        let second = child(&parent, &Manifest::empty()).to_canonical_bytes();
        assert_eq!(first, second);
        // And not vacuously: a child of a different question is different bytes.
        let identity = ExpansionHandle::derive::<Blake3Hasher>(
            &identity_of(&parent).expect("a pack handle"),
            &query(),
            Depth::new(2).expect("nonzero"),
        )
        .expect("derives");
        let question = ExpansionQuestion::of(&query(), Depth::new(2).expect("nonzero"));
        let manifest = Manifest::empty();
        let deeper = ChildPack {
            parent: &parent,
            identity: &identity,
            question: &question,
            selected: &[],
            manifest: &manifest,
            budget_bytes: 16384,
        }
        .to_json::<Blake3Hasher>()
        .expect("conforming")
        .to_canonical_bytes();
        assert_ne!(first, deeper);
    }

    #[test]
    fn the_content_hash_is_the_documents_own_identity_with_the_key_absent() {
        let document = child(&parent(), &Manifest::empty());
        let mut fields = document.as_object().cloned().expect("object");
        let recorded = fields
            .remove("content_hash")
            .expect("the key is present")
            .as_str()
            .expect("string")
            .to_owned();
        let preimage = Json::Object(fields).to_canonical_bytes();
        assert_eq!(
            recorded,
            format!(
                "blake3-256:{}",
                continuum_value::identity::Blake3Hasher::hash(&preimage).to_token()
            ),
            "the recorded identity is re-derivable from the published document"
        );
    }

    #[test]
    fn the_budget_the_parent_recorded_is_readable() {
        assert_eq!(budget_bytes_of(&parent()), Ok(16384));
        let document = child(&parent(), &Manifest::empty());
        assert_eq!(budget_bytes_of(&document), Ok(16384));
    }
}
