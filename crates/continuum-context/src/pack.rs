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
//! - **`content_budget.bytes` is this child's own measured byte size** (PR-11 / IMPL-06).
//!   RFC 0028 correction 17 reads the field as a measurement — "the pack's counterpart of
//!   the IDL's `Cost.bytes`, not `Budget.bytes`/`OutputPolicy.max_bytes`" — because the
//!   ratified FR-01 sentence is graded by *summing* `content_budget.bytes` over a pack
//!   family, which measures nothing unless each member's value is what that member actually
//!   spent. Until IMPL-06 landed this module wrote the *ceiling* there as a documented
//!   interim placeholder; it now writes the measurement, and the ceiling lives on
//!   [`ChildPack::ceiling`] where admission — [`ChildPack::within`], and
//!   [`crate::budget`]'s packer — is the only thing that reads it. **The reading this
//!   module fixes, once, here:** *measured* is the byte length of the child's own canonical
//!   encoding — the whole published document, the bytes a caller receives and the bytes
//!   ADR-0013 compares ("Two packs are the same pack iff their canonical encodings are
//!   byte-equal") — not the identity preimage, not the payload minus its framing, and not
//!   any subset of the keys. That is the only reading under which RFC 0028's "Expansion
//!   accounting" sums to the number it says it sums to: the context bytes a pack family
//!   actually cost.
//! - **`content_budget.tokens` is absent, and its absence is the honest answer.** Tokens
//!   are "advisory and model-relative", and the schema requires `tokenizer_id` "whenever
//!   `tokens` is present; a token count with no tokenizer is not a measurement" (RFC 0028
//!   F4, the IDL's `Cost.tokenizer_id`). No tokenizer exists in this workspace — `continuumd`
//!   declines the same field for the same reason ("this daemon meters no tokens, so it stays
//!   absent — never a placeholder beside an absent count", `daemon::budget`) — so a count
//!   written here would be either fabricated or non-conforming. This module writes neither
//!   key. When a tokenizer lands, both keys land together or neither does.
//!
//! # `content_budget.bytes` is a fixed point, and which one
//!
//! The measurement is a key of the document it measures, so recording it changes the number
//! it records. That is the same self-reference `content_hash` has (next section), and it is
//! closed the same way: by stating which of the readings the written token came from rather
//! than leaving a reader to guess.
//!
//! A child's canonical encoding is `C + digits(N)` bytes long, where `N` is the value at
//! `content_budget.bytes` and `C` is everything else. `C` is a constant of the document:
//! changing `N` changes `content_hash`'s *value* and not its length, because a digest token
//! is fixed-width. A truthful measurement is therefore a solution of `N = C + digits(N)`,
//! and [`ChildPack::to_json`] finds one by iterating from zero — build, measure, write the
//! measurement back, repeat until the number stops moving.
//!
//! That iteration converges to the **least** solution, and least is a choice rather than an
//! accident: `N ↦ C + digits(N)` is monotone and the iteration starts below every solution,
//! so it climbs to the smallest one. It has to be pinned, because a solution is not always
//! unique — at a decimal boundary two are self-consistent (with `C = 8`, a nine-byte
//! document saying `9` and a ten-byte one saying `10` are both telling the truth about
//! themselves), and a packer free to publish either would have two encodings for one answer
//! and therefore two `content_hash`es, which is the identity defect RFC 0028's determinism
//! clause exists to forbid.
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
//! | `content_budget.bytes` is the child's own canonical length | RFC 0028 correction 17 | `the_recorded_size_is_the_documents_own_canonical_length` |
//! | it is the *least* self-consistent length | this module's fixed-point reading | `the_recorded_size_is_the_least_self_consistent_one` |
//! | the ceiling is not the measurement | RFC 0028 correction 17 | `the_ceiling_is_not_what_the_child_records` |
//! | no token count is written without a tokenizer | RFC 0028 F4; the schema's conditional | `no_child_claims_a_token_count` |

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

/// The value the parent recorded under `content_budget.bytes`: per RFC 0028 correction 17,
/// **the parent's own measured byte size**, never a ceiling.
///
/// A caller may still choose to *use* it as a ceiling for a child — `continuumd`'s
/// `context.expand` does, where the request names none — and that is a reading about the
/// caller's default, not about the field: "an expansion may not cost more than the pack it
/// expands unless the caller asks for more" is a defensible default precisely because the
/// number is what the parent spent. What this function does not do is let that reading back
/// into the artifact: what a child *records* is its own measurement ([`ChildPack::to_json`]),
/// and what it is *admitted against* is [`ChildPack::ceiling`].
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
    /// The byte ceiling this expansion runs under — the caller's stated `max_bytes`, or
    /// whatever default the caller resolved for it.
    ///
    /// **Not** what the child records: `content_budget.bytes` is the child's own measured
    /// size (RFC 0028 correction 17; this module's documentation). The ceiling is what
    /// admission is decided against — [`ChildPack::within`] for the one-shot check, and
    /// [`crate::budget::BudgetPacker`] for RFC 0028's second branch, which packs a smaller
    /// child rather than refusing.
    pub ceiling: u64,
}

/// How many times [`ChildPack::to_json`] will build a document before refusing to measure it.
///
/// The measurement iteration converges in two or three rounds for every hasher this
/// workspace has (see the module documentation's fixed-point section: the digit count is
/// non-decreasing, and `u64::MAX` has twenty digits). The bound exists so that a hasher
/// whose token *width* varied with its input would fail typed rather than spin.
pub(crate) const MEASUREMENT_ROUNDS: usize = 24;

impl ChildPack<'_> {
    /// Write the child document, measured.
    ///
    /// `content_budget.bytes` carries the byte length of the returned document's own
    /// canonical encoding — the least self-consistent one, see the module documentation.
    ///
    /// # Errors
    ///
    /// [`PackError`] when the parent is not a conforming pack this child can be derived
    /// from: not an object, missing an inherited key, or carrying a `context_id` that is
    /// not a `ctx_*` handle; and [`PackError::UnmeasurableSize`] if the measurement does not
    /// settle, which no hasher in this workspace can cause.
    pub fn to_json<H: ContentHasher>(&self) -> Result<Json, PackError> {
        let mut measured: u64 = 0;
        for _ in 0..MEASUREMENT_ROUNDS {
            let document = self.document_with::<H>(measured)?;
            let length = document.to_canonical_bytes().len() as u64;
            if length == measured {
                return Ok(document);
            }
            measured = length;
        }
        Err(PackError::UnmeasurableSize)
    }

    /// The child document with `bytes` written at `content_budget.bytes`, whatever `bytes`
    /// is.
    ///
    /// The building block [`ChildPack::to_json`] iterates to a fixed point, and the one
    /// [`crate::budget`] measures a document's fixed part with. Deliberately not public: a
    /// pack whose recorded size is not its size is not a pack this crate publishes.
    ///
    /// # Errors
    ///
    /// As [`ChildPack::to_json`], less the measurement failure.
    pub(crate) fn document_with<H: ContentHasher>(&self, bytes: u64) -> Result<Json, PackError> {
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
        // One key, and one key only: `tokens`/`tokenizer_id` are absent because no
        // tokenizer exists to count under, and `nodes` is `OutputPolicy.max_nodes`'s
        // ceiling, which no expansion in this workspace is given. See the module
        // documentation.
        fields.insert(
            "content_budget".to_owned(),
            Json::object([(
                "bytes".to_owned(),
                Json::Integer(i64::try_from(bytes).unwrap_or(i64::MAX)),
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
    /// thing: the document is what the expansion found and what it measured, and this is
    /// whether the caller's budget admits it. The comparison is between the two numbers
    /// RFC 0028 correction 17 separates — the document's measured size, which is exactly
    /// what `content_budget.bytes` records, against [`ChildPack::ceiling`].
    ///
    /// RFC 0028's typed-outcome table gives a daemon both branches for a budget it cannot
    /// meet — "either nothing is published, or a child pack is published whose manifest
    /// records the shortfall with reason `budget`". This function decides the *first*
    /// question only ("does the whole answer fit"); [`crate::budget::BudgetPacker`] owns
    /// the second branch and calls this to decide when it is needed.
    #[must_use]
    pub fn within(&self, document: &Json) -> bool {
        document.to_canonical_bytes().len() as u64 <= self.ceiling
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
    /// The child's own byte size did not settle to a self-consistent value.
    ///
    /// Unreachable for a fixed-width digest token, which every hasher in this workspace
    /// has; a typed refusal rather than a panic because a pack whose recorded size is not
    /// its size is not a pack this crate publishes. See the module documentation's
    /// fixed-point section.
    UnmeasurableSize,
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
            Self::UnmeasurableSize => f.write_str(
                "the child's own byte size does not settle: no value of `content_budget.bytes` \
                 is the length of the document that records it",
            ),
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
            ceiling: 16384,
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
                ceiling: 16384,
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
            ceiling: 16384,
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
        // The child's own value is *its* measurement, not the parent's — correction 17.
        assert_eq!(
            budget_bytes_of(&document),
            Ok(document.to_canonical_bytes().len() as u64)
        );
    }

    #[test]
    fn the_recorded_size_is_the_documents_own_canonical_length() {
        // RFC 0028 correction 17: "the field is a measurement, the pack's counterpart of the
        // IDL's `Cost.bytes`". This module's reading of *measured*: the byte length of the
        // published canonical encoding, which is checkable against the document itself.
        for manifest in [
            Manifest::empty(),
            Manifest::new([OmissionRecord::expandable(
                SelectionKind::Event,
                196,
                OmissionReason::Budget,
                ExpansionQuery::new(ExpansionRelation::CausalPredecessors, name("e_ack")),
            )])
            .expect("one cell"),
        ] {
            let document = child(&parent(), &manifest);
            let recorded = budget_bytes_of(&document).expect("a measured child");
            assert_eq!(
                recorded,
                document.to_canonical_bytes().len() as u64,
                "the recorded size is the size of the document that records it"
            );
        }
    }

    #[test]
    fn the_recorded_size_is_the_least_self_consistent_one() {
        // Anti-vacuity for the fixed-point reading. A neighbouring value is either not a
        // length at all (the usual case) or is a *larger* self-consistent one at a decimal
        // boundary; neither may be what the child publishes, so no smaller value than the
        // recorded one is self-consistent and the recorded one is.
        let document = child(&parent(), &Manifest::empty());
        let recorded = budget_bytes_of(&document).expect("a measured child");
        let mut fields = document.as_object().cloned().expect("object");
        for candidate in (0..recorded).rev().take(64) {
            fields.insert(
                "content_budget".to_owned(),
                Json::object([("bytes".to_owned(), Json::Integer(candidate as i64))])
                    .expect("one key"),
            );
            // The `content_hash` of the perturbed document is stale, but its *length* is
            // fixed-width, so the comparison below is about the only thing that moves.
            let length = Json::Object(fields.clone()).to_canonical_bytes().len() as u64;
            assert_ne!(
                length, candidate,
                "{candidate} is self-consistent and smaller than the recorded {recorded}"
            );
        }
    }

    #[test]
    fn the_ceiling_is_not_what_the_child_records() {
        // The correction-17 flip, as a difference rather than as prose: two children of one
        // question under two ceilings record one number — their own size — and neither
        // records the ceiling it ran under.
        let parent = parent();
        let manifest = Manifest::empty();
        let identity = ExpansionHandle::derive::<Blake3Hasher>(
            &identity_of(&parent).expect("a pack handle"),
            &query(),
            Depth::DEFAULT,
        )
        .expect("derives");
        let question = ExpansionQuestion::of(&query(), Depth::DEFAULT);
        let build = |ceiling: u64| {
            ChildPack {
                parent: &parent,
                identity: &identity,
                question: &question,
                selected: &[],
                manifest: &manifest,
                ceiling,
            }
            .to_json::<Blake3Hasher>()
            .expect("conforming")
        };
        let narrow = build(16384);
        let wide = build(1 << 20);
        assert_eq!(narrow.to_canonical_bytes(), wide.to_canonical_bytes());
        let recorded = budget_bytes_of(&narrow).expect("measured");
        assert_ne!(recorded, 16384);
        assert_ne!(recorded, 1 << 20);
        assert!(
            recorded < 16384,
            "the fixture's child is under both ceilings"
        );
    }

    #[test]
    fn no_child_claims_a_token_count() {
        // RFC 0028 F4 and the schema's conditional: a token count owes a tokenizer identity,
        // and this workspace has no tokenizer. `content_budget` therefore carries exactly
        // one key.
        let document = child(&parent(), &Manifest::empty());
        let budget = document.as_object().expect("object")["content_budget"]
            .as_object()
            .expect("object");
        assert_eq!(
            budget.keys().map(String::as_str).collect::<Vec<_>>(),
            ["bytes"]
        );
    }
}
