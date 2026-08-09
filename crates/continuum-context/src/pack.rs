//! The two pack documents: the **root** a compile publishes ([`RootPack`]) and the **child**
//! an expansion derives ([`ChildPack`]).
//!
//! # Root assembly, and the identity question it settles (bn-1y4qc)
//!
//! Until this bone the module carried only the child, and said so: "assembling a Context Pack
//! from evidence is the compiler […] nothing here synthesizes a pack field out of nothing."
//! That decline was about *fabrication*, not about writing a document — and now that stages
//! 1–8 are landed there is a compile to write the document *of*. [`RootPack`] therefore
//! synthesizes nothing either: every one of the seventeen required keys is either handed to it
//! as a typed value some checker produced ([`crate::guarantee::SealedGuarantees`],
//! [`crate::omission::Manifest`], [`crate::verdict::Verdict`], [`crate::assurance::Assurance`],
//! [`crate::target::Target`], [`crate::replay::ReplayRef`]) or is the document's own
//! measurement of itself. There is no key it invents and no default it substitutes.
//!
//! **`context_id` is the identity of the question; `content_hash` is the identity of the
//! bytes.** `daemon::context` deliberately left this open — "a store handle is the content
//! identity of the bytes (`ContentIdentifier::identify`), and a pack's `context_id` is the
//! identity of its *question* […] two spellings for one artifact unless something reconciles
//! them, and reconciling them is a decision about pack assembly" — and the reconciliation is
//! that they are **not** two spellings of one thing and must not be made into one:
//!
//! - `context_id` answers *which question is this*. RFC 0028: "the question is part of the
//!   identity", and "two `context.compile` requests differing only in `audience` MUST produce
//!   the same `ctx_*`". It is therefore a function of the compile's question and of nothing
//!   that only rendering depends on — [`RootIdentity::derive`] — exactly as
//!   [`ExpansionHandle::derive`] is for a child. A digest of the bytes could not have this
//!   property: two compiles of one question under different budgets have different bytes and
//!   are required to be *frontier-comparable answers to one question*, not two questions.
//! - `content_hash` answers *which bytes are these* (ADR-0013: "two packs are the same pack iff
//!   their canonical encodings are byte-equal"), and it is exactly the value a content-addressed
//!   store derives when the pack is published into it.
//!
//! Because the artifact carries **both**, a store handle and a `ctx_*` never need a third party
//! to reconcile them: the mapping is one field lookup on a self-describing document, in either
//! direction, and no second identity is minted anywhere. That is why this module publishes
//! nothing into a store and why the daemon does not either — publication would add a *record*,
//! never a new identity, and the record is derivable from the pack.
//!
//! # This is still derivation, not fabrication
//!
//! What [`ChildPack`] does is the operation RFC 0028 actually specifies for an expansion:
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
//! | a root carries the seventeen required keys | `context-pack.schema.json` `required` | `a_root_carries_every_required_key` |
//! | `audience` cannot reach `context_id` | RFC 0028, "Views and rendering" | `the_root_identity_is_a_function_of_the_question_alone` |
//! | the six list fields are deterministically ordered | `rule ordering.deterministic` | `the_six_list_fields_are_deterministically_ordered` |
//! | a root whose counting equation is false is refused | INV-007; RFC 0028, "Omission manifest" | `an_unreconciled_root_is_refused` |
//! | rule C1's record is in the manifest | RFC 0028 C1 | `an_unachieved_request_is_an_unsupported_omission` |
//! | rule C2 is checked at the artifact | RFC 0028 C2 | `a_replay_preserving_root_without_a_replay_is_refused` |
//! | each profile's content constraints | RFC 0028, "Pack profiles" | `each_profile_refuses_its_own_violation` |

use core::fmt;
use core::str::FromStr;
use std::collections::{BTreeMap, BTreeSet};

use continuum_intent::canonical_json::Json;
use continuum_value::identity::ContentHasher;
use continuum_value::value::Name;
use continuum_workspace::artifact_path::{ArtifactClass, ArtifactHandle};

use crate::assurance::Assurance;
use crate::compile::RedactionReason;
use crate::expansion::{ExpansionHandle, ExpansionQuestion};
use crate::guarantee::{Guarantee, SealedGuarantees};
use crate::omission::{Manifest, OmissionRecord};
use crate::replay::ReplayRef;
use crate::selection::{SelectedItem, SelectionKind};
use crate::target::Target;
use crate::verdict::Verdict;

/// The `schema_id` every pack this crate writes declares.
pub const SCHEMA_ID: &str = "https://continuum.dev/schema/context-pack.json";

/// The `schema_epoch` this crate implements (RFC 0028, "Versioning and revision": "epoch 1 is
/// the vocabulary specified here").
pub const SCHEMA_EPOCH: i64 = 1;

/// The compiler identity a root pack records at `compiler.compiler_version`.
///
/// > Version of the Context Pack compiler. Not a release counter: it advances whenever the
/// > compiled output could change for any input.
/// >
/// > — `context-pack.schema.json`, `compiler.compiler_version`
///
/// It names the stage set this crate actually runs, because that is what "the compiled output
/// could change" is a function of: RFC 0028's ten stages, with stage 9 disabled (the register
/// row's named fallback, "plain causal slice + expansion") and stage 10 supplied by
/// [`crate::budget`]. A bone that adds a stage, or changes what one licenses, moves this token.
pub const COMPILER_VERSION: &str = "continuum-context/1.stages-1-8+10";

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

// =====================================================================================
// the root document: what a `context.compile` answer *is*, as an artifact
// =====================================================================================

/// The `ctx_*` a compiled question resolves to.
///
/// Derived, never minted, for [`ExpansionHandle`]'s reason and one more of its own: RFC 0028
/// requires that "two `context.compile` requests differing only in `audience` MUST produce the
/// same `ctx_*`", and a *derived* identity makes that a property of the preimage rather than a
/// rule a daemon has to remember. `audience` is not in the preimage, so it cannot reach the
/// handle; nor can the budget, which RFC 0028 correction 11 puts in execution identity.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RootIdentity(ArtifactHandle);

impl RootIdentity {
    /// Derive the handle of the pack that answers this question.
    ///
    /// The preimage is the ID5 canonical encoding of
    /// `{"evidence", "intent", "question", "semantic_epoch", "snapshot"}` — the five terms that
    /// decide *which question this is*. One canonical object, so no pair of inputs can produce
    /// another pair's preimage by concatenation, and the `evidence` array is sorted so that a
    /// caller's ordering of its roots is not part of the question.
    ///
    /// `semantic_epoch` is in the preimage because "a compile under a different semantic epoch
    /// is a recompile, not an expansion" (RFC 0028, "Expansion protocol") and `ReplayPreserving`
    /// "is defined relative to the pinned epoch" (correction 9); `snapshot` is in it because a
    /// pack is stated against one sealed snapshot (plan §4.2).
    #[must_use]
    pub fn derive<H: ContentHasher>(
        target: &Target,
        snapshot: &str,
        semantic_epoch: &str,
        evidence: &[ArtifactHandle],
    ) -> Self {
        let mut roots: Vec<String> = evidence.iter().map(ToString::to_string).collect();
        roots.sort();
        roots.dedup();
        let preimage = Json::object([
            (
                "evidence".to_owned(),
                Json::Array(roots.into_iter().map(Json::String).collect()),
            ),
            (
                "intent".to_owned(),
                Json::String(target.intent().to_string()),
            ),
            (
                "question".to_owned(),
                Json::String(target.question().as_str().to_owned()),
            ),
            (
                "semantic_epoch".to_owned(),
                Json::String(semantic_epoch.to_owned()),
            ),
            ("snapshot".to_owned(), Json::String(snapshot.to_owned())),
        ])
        .expect("the five keys above are pairwise distinct string literals")
        .to_canonical_bytes();
        let handle =
            ArtifactHandle::new(ArtifactClass::ContextPack, &H::hash(&preimage).to_token())
                .expect("a lowercase-hex digest token is a well-formed artifact identity");
        Self(handle)
    }

    /// The handle, as an artifact handle.
    #[must_use]
    pub const fn handle(&self) -> &ArtifactHandle {
        &self.0
    }
}

impl fmt::Display for RootIdentity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// One `redactions[]` entry: the `Redacted(redacted, reason, commitment, original_class)` stub
/// [`crate::compile`] declined by name ("the pack's stub needs a commitment and an original
/// class, which are properties of the artifact store and of whole-pack assembly").
///
/// The two fields the pre-pass could not supply are supplied here because they are the
/// *store's* facts, not the slicer's: a commitment is the content identity of the original the
/// policy withheld, and `original_class` is that original's plan §4.4 artifact class. Both are
/// carried, never derived — deriving a commitment here would mean hashing content the policy
/// just refused this caller, which is the exfiltration RFC 0028's "packs are an exfiltration
/// surface" paragraph forbids.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct RedactionStub {
    redacted: Name,
    reason: RedactionReason,
    commitment: String,
    original_class: ArtifactClass,
}

impl RedactionStub {
    /// Record a withheld candidate.
    ///
    /// # Errors
    ///
    /// [`PackError::EmptyCommitment`] for a stub with no commitment: the schema requires the
    /// key, and an empty string would be a stub asserting that *nothing* stands behind the
    /// redaction, which is the silence plan §18.4 refuses.
    pub fn new(
        redacted: Name,
        reason: RedactionReason,
        commitment: &str,
        original_class: ArtifactClass,
    ) -> Result<Self, PackError> {
        if commitment.trim().is_empty() {
            return Err(PackError::EmptyCommitment);
        }
        Ok(Self {
            redacted,
            reason,
            commitment: commitment.to_owned(),
            original_class,
        })
    }

    /// The candidate this stub stands for.
    #[must_use]
    pub const fn redacted(&self) -> &Name {
        &self.redacted
    }

    /// Why it was withheld.
    #[must_use]
    pub const fn reason(&self) -> RedactionReason {
        self.reason
    }

    /// The stub, as the schema's `$defs.redacted` object.
    ///
    /// `redacted` is `const: true` in the schema, so it is written as the literal it is
    /// declared to be rather than as a value a caller could choose.
    #[must_use]
    pub fn to_json(&self) -> Json {
        Json::object([
            (
                "commitment".to_owned(),
                Json::String(self.commitment.clone()),
            ),
            (
                "original_class".to_owned(),
                Json::String(self.original_class.token().to_owned()),
            ),
            (
                "reason".to_owned(),
                Json::String(self.reason.as_wire_str().to_owned()),
            ),
            ("redacted".to_owned(), Json::Bool(true)),
        ])
        .expect("the four keys above are pairwise distinct string literals")
    }
}

/// RFC 0028's four pack profiles, and the content constraints each one adds.
///
/// > One schema, four profiles. A profile constrains which fields and kinds MUST be present; it
/// > is not a new artifact class and MUST NOT be encoded as a field the schema does not have.
/// >
/// > — RFC 0028, "Pack profiles"
///
/// So a profile is *not* written into the document. It is the check a compile is held to before
/// its document is published, which is why it lives on [`RootPack`] as a field and nowhere in
/// the JSON. Each arm below states only the profile's own MUSTs; the SHOULDs are not enforced,
/// because a SHOULD refused at the type would be this crate legislating past its RFC.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PackProfile {
    /// "A question about a `refuted` or `deadlock` verdict. `replay` MUST be non-null."
    Failure,
    /// "A question about a search's coverage. `assurance.envelope.bounds` and `.schedules` MUST
    /// name their producing engine — a reachability claim without bounds is not a claim."
    Reachability,
    /// "A question about a candidate invariant or abstraction. It MUST carry `assumption`
    /// items, MUST carry the counterexample-to-induction as `counterfactual` or `state_delta`
    /// items where one exists."
    Invariant,
    /// "A question about what blocks an obligation. […] It MUST claim `ProofRelevant` or state
    /// why it cannot."
    ProofTarget,
}

impl PackProfile {
    /// All four, in the RFC's order.
    pub const ALL: [Self; 4] = [
        Self::Failure,
        Self::Reachability,
        Self::Invariant,
        Self::ProofTarget,
    ];

    /// This profile's name, for a refusal that has to say which profile refused.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Failure => "failure",
            Self::Reachability => "reachability",
            Self::Invariant => "invariant",
            Self::ProofTarget => "proof-target",
        }
    }

    /// Whether an assembled root satisfies this profile's content constraints.
    ///
    /// # Errors
    ///
    /// [`PackError::ProfileViolation`] naming the profile and the clause it failed.
    fn admits(self, root: &RootPack<'_>) -> Result<(), PackError> {
        let violation = |clause: &'static str| {
            Err(PackError::ProfileViolation {
                profile: self,
                clause,
            })
        };
        let claims = |guarantee: Guarantee| root.guarantees.members().contains(&guarantee);
        let carries = |kind: SelectionKind| root.selected.iter().any(|item| item.kind() == kind);
        match self {
            Self::Failure => {
                if !matches!(root.verdict, Verdict::Refuted | Verdict::Deadlock) {
                    return violation("a failure pack answers a `refuted` or `deadlock` verdict");
                }
                if root.replay.is_none() {
                    return violation("a failure pack's `replay` MUST be non-null");
                }
                Ok(())
            }
            Self::Reachability => {
                let envelope = root.assurance.envelope();
                let names_an_engine = |dimension| {
                    envelope
                        .dimension(dimension)
                        .engine()
                        .is_some_and(|engine| !engine.is_empty())
                };
                if !names_an_engine(continuum_value::assurance::AssuranceDimension::Bounds)
                    || !names_an_engine(continuum_value::assurance::AssuranceDimension::Schedules)
                {
                    return violation(
                        "a reachability pack's `bounds` and `schedules` dimensions MUST name a \
                         producing engine",
                    );
                }
                Ok(())
            }
            Self::Invariant => {
                if !carries(SelectionKind::Assumption) {
                    return violation("an invariant pack MUST carry `assumption` items");
                }
                Ok(())
            }
            Self::ProofTarget => {
                if !claims(Guarantee::ProofRelevant)
                    && !matches!(root.verdict, Verdict::Inconclusive(_))
                {
                    return violation(
                        "a proof-target pack MUST claim `ProofRelevant` or state why it cannot, \
                         which is a typed `inconclusive` verdict",
                    );
                }
                Ok(())
            }
        }
    }
}

impl fmt::Display for PackProfile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// The root pack a compile answers with, before it is written.
///
/// Every field is a value some checker or some registered input produced. This struct joins
/// them; it decides nothing about their content.
#[derive(Debug, Clone, Copy)]
pub struct RootPack<'a> {
    /// The identity of the question this pack answers.
    pub identity: &'a RootIdentity,
    /// The `ws_*` snapshot the pack is stated against.
    pub snapshot: &'a str,
    /// The pinned `semantic_epoch`.
    pub semantic_epoch: &'a str,
    /// The `in_*` contract and the question, type-checked together.
    pub target: &'a Target,
    /// The verdict of the evaluation this pack compiles evidence *for*.
    pub verdict: Verdict,
    /// The `{class, envelope}` block, as the result it explains reports it.
    pub assurance: &'a Assurance,
    /// The selected items. Order is imposed here (`rule ordering.deterministic`).
    pub selected: &'a [SelectedItem],
    /// The compile's own manifest, over the compile's own candidate set.
    pub manifest: &'a Manifest,
    /// How many candidates the compile considered — INV-007's left-hand side.
    pub candidates: u32,
    /// The guarantees requested and not achieved (rule C1).
    pub unachieved: &'a [Guarantee],
    /// The `ev_*` roots this pack was compiled from.
    pub evidence: &'a [ArtifactHandle],
    /// The `crash_*` replay handle, where one exists.
    pub replay: Option<&'a ReplayRef>,
    /// The guarantees the checkers established.
    pub guarantees: &'a SealedGuarantees,
    /// The withheld-content stubs the pre-pass earned.
    pub redactions: &'a [RedactionStub],
    /// Which profile's content constraints this compile is held to.
    pub profile: PackProfile,
}

impl RootPack<'_> {
    /// The manifest the *pack* publishes: the compile's, plus rule C1's record.
    ///
    /// > A requested-but-unachieved guarantee MUST appear in the omission manifest with reason
    /// > `unsupported` (outside the engine's semantics) or `budget` (dropped by packing).
    /// >
    /// > — RFC 0028 C1
    ///
    /// [`crate::compile`] computed *what* is owed and declined to decide the shape, because "the
    /// manifest's `kind` is the closed `SelectionKind` vocabulary and a guarantee is not a
    /// selection kind". The shape is decided here, and it is `unknown`:
    ///
    /// > `unknown` carries plan §12.2's "uncertainty and incompleteness" as a typed item and is
    /// > the pack-level form of INV-008. A compiler MUST use `unknown` rather than omitting a
    /// > fact it detected but could not classify.
    /// >
    /// > — RFC 0028, "Required fields, reconciled with plan §6.2"
    ///
    /// A guarantee the compile ran a checker for and did not get is exactly such a fact: it was
    /// detected (the checker declined) and it has no other kind. The reason is `unsupported` —
    /// C1's first branch, "outside the engine's semantics" — and the record is **irretrievable**,
    /// because no expansion in any deployment retrieves a claim that was never established.
    ///
    /// The candidate universe grows by one per owed record, and [`RootPack::to_json`] checks the
    /// counting equation over the grown universe. That is the only reading under which both of
    /// RFC 0028's MUSTs hold at once: a record outside the equation would make "candidate set =
    /// selection + Σ manifest counts" false, and no record at all would fail C1.
    ///
    /// # Errors
    ///
    /// [`PackError::Manifest`] when the merged records overflow an exact count.
    pub fn published_manifest(&self) -> Result<Manifest, PackError> {
        if self.unachieved.is_empty() {
            return Ok(self.manifest.clone());
        }
        let owed = u32::try_from(self.unachieved.len()).map_err(|_| PackError::Unreconciled {
            candidates: u64::from(self.candidates),
            accounted: u64::MAX,
        })?;
        Manifest::merged(self.manifest.records().iter().cloned().chain([
            OmissionRecord::irretrievable(
                SelectionKind::Unknown,
                owed,
                crate::omission::IrretrievableReason::Unsupported,
            ),
        ]))
        .map_err(PackError::Manifest)
    }

    /// The candidate universe the published manifest partitions: the compile's, plus one per
    /// owed rule-C1 record. See [`RootPack::published_manifest`].
    ///
    /// # Errors
    ///
    /// [`PackError::Unreconciled`] when the count leaves the exact-count width.
    pub fn published_candidates(&self) -> Result<u64, PackError> {
        Ok(u64::from(self.candidates) + self.unachieved.len() as u64)
    }

    /// Write the root document, measured.
    ///
    /// `content_budget.bytes` carries the byte length of the returned document's own canonical
    /// encoding — the least self-consistent one, by exactly the fixed point
    /// [`ChildPack::to_json`] uses and for exactly its reasons.
    ///
    /// # Errors
    ///
    /// [`PackError::Unreconciled`] when INV-007's counting equation is false — a pack whose
    /// manifest does not account for its own candidate set is not published at all, which is the
    /// direction `Accounting::close` already takes one layer down;
    /// [`PackError::RepeatedSelection`] for one identity selected twice;
    /// [`PackError::UnclassedHandle`] for an `evidence` entry that is not `ev_*` or a `snapshot`
    /// that is not `ws_*`;
    /// [`PackError::EmptyEpoch`] for an absent `semantic_epoch`;
    /// [`PackError::ReplayPreservingWithoutReplay`] for rule C2's artifact half;
    /// [`PackError::ProfileViolation`] when the profile's content constraints are not met;
    /// [`PackError::Manifest`] from [`RootPack::published_manifest`]; and
    /// [`PackError::UnmeasurableSize`] if the measurement does not settle.
    pub fn to_json<H: ContentHasher>(&self) -> Result<Json, PackError> {
        self.check()?;
        let manifest = self.published_manifest()?;
        let mut measured: u64 = 0;
        for _ in 0..MEASUREMENT_ROUNDS {
            let document = self.document_with::<H>(&manifest, measured);
            let length = document.to_canonical_bytes().len() as u64;
            if length == measured {
                return Ok(document);
            }
            measured = length;
        }
        Err(PackError::UnmeasurableSize)
    }

    /// Every refusal that is about the *inputs* rather than about the writing.
    fn check(&self) -> Result<(), PackError> {
        if !self.snapshot.starts_with("ws_") {
            return Err(PackError::UnclassedHandle { key: "snapshot" });
        }
        if self.semantic_epoch.trim().is_empty() {
            return Err(PackError::EmptyEpoch);
        }
        for handle in self.evidence {
            if handle.class() != ArtifactClass::Evidence {
                return Err(PackError::UnclassedHandle { key: "evidence" });
            }
        }
        let mut seen: BTreeSet<&Name> = BTreeSet::new();
        for item in self.selected {
            if !seen.insert(item.id()) {
                return Err(PackError::RepeatedSelection {
                    id: item.id().clone(),
                });
            }
        }
        // Rule C2's artifact half. `GuaranteeSet::seal` already refuses `ReplayPreserving`
        // without `CausallyClosed`; the other conjunct — "and MUST carry a non-null `replay`" —
        // is a statement about the document, so it is checked where the document is written.
        if self
            .guarantees
            .members()
            .contains(&Guarantee::ReplayPreserving)
            && self.replay.is_none()
        {
            return Err(PackError::ReplayPreservingWithoutReplay);
        }
        let manifest = self.published_manifest()?;
        let accounted = manifest.total().saturating_add(self.selected.len() as u64);
        let candidates = self.published_candidates()?;
        if accounted != candidates {
            return Err(PackError::Unreconciled {
                candidates,
                accounted,
            });
        }
        self.profile.admits(self)
    }

    /// The root document with `bytes` written at `content_budget.bytes`, whatever `bytes` is.
    fn document_with<H: ContentHasher>(&self, manifest: &Manifest, bytes: u64) -> Json {
        let mut fields: BTreeMap<String, Json> = BTreeMap::new();
        fields.insert("schema_id".to_owned(), Json::String(SCHEMA_ID.to_owned()));
        fields.insert("schema_epoch".to_owned(), Json::Integer(SCHEMA_EPOCH));
        fields.insert(
            "context_id".to_owned(),
            Json::String(self.identity.to_string()),
        );
        fields.insert(
            "snapshot".to_owned(),
            Json::String(self.snapshot.to_owned()),
        );
        for (key, value) in self.target.to_json_fields() {
            fields.insert(key, value);
        }
        for (key, value) in self.verdict.to_json_fields() {
            fields.insert(key, value);
        }
        for (key, value) in self.assurance.to_json_fields() {
            fields.insert(key, value);
        }
        // `rule ordering.deterministic`, one field at a time. `selected` sorts by the item's own
        // canonical order (identity first); `evidence` by handle; `redactions` by the candidate
        // they stand for; `omissions`/`expansions` are the manifest's own canonical order; and
        // `guarantees` is the schema's declared enum order, which `SealedGuarantees` holds.
        let mut selected: Vec<&SelectedItem> = self.selected.iter().collect();
        selected.sort();
        fields.insert(
            "selected".to_owned(),
            Json::Array(selected.into_iter().map(SelectedItem::to_json).collect()),
        );
        fields.insert("omissions".to_owned(), manifest.to_json());
        fields.insert("expansions".to_owned(), manifest.expansions_json());
        let mut evidence: Vec<String> = self.evidence.iter().map(ToString::to_string).collect();
        evidence.sort();
        evidence.dedup();
        fields.insert(
            "evidence".to_owned(),
            Json::Array(evidence.into_iter().map(Json::String).collect()),
        );
        fields.insert(
            "replay".to_owned(),
            self.replay.map_or(Json::Null, ReplayRef::to_json),
        );
        fields.insert("guarantees".to_owned(), self.guarantees.to_json());
        fields.insert(
            "semantic_epoch".to_owned(),
            Json::String(self.semantic_epoch.to_owned()),
        );
        // A root has no parent, and the schema's `parent` is nullable: `null` is the statement
        // "this pack is a root", which an absent key would not make (RFC 0028 F1's own reading —
        // "an absent `parent` is still indistinguishable from an unrecorded one").
        fields.insert("parent".to_owned(), Json::Null);
        if !self.redactions.is_empty() {
            let mut stubs: Vec<&RedactionStub> = self.redactions.iter().collect();
            stubs.sort();
            fields.insert(
                "redactions".to_owned(),
                Json::Array(stubs.into_iter().map(RedactionStub::to_json).collect()),
            );
        }
        // RFC 0028 F3, paid in the schema: the compile's own identity. `ranker_id` is `null`
        // rather than absent because stage 9 being *disabled* is a fact — the register row's
        // named fallback configuration — and the schema types the key `["string", "null"]`
        // precisely to carry it; `ranker_config` is absent because there is no configuration of
        // a ranker that did not run.
        fields.insert(
            "compiler".to_owned(),
            Json::object([
                (
                    "compiler_version".to_owned(),
                    Json::String(COMPILER_VERSION.to_owned()),
                ),
                ("ranker_id".to_owned(), Json::Null),
            ])
            .expect("two distinct string literals"),
        );
        // One key, for `ChildPack::document_with`'s reasons: no tokenizer exists in this
        // workspace, and `nodes` is a ceiling nothing here is given.
        fields.insert(
            "content_budget".to_owned(),
            Json::object([(
                "bytes".to_owned(),
                Json::Integer(i64::try_from(bytes).unwrap_or(i64::MAX)),
            )])
            .expect("one key"),
        );

        // The identity preimage: this document, `content_hash` absent, every other key present
        // — the same reading `ChildPack` fixes, stated once in the module documentation.
        let preimage = Json::Object(fields.clone()).to_canonical_bytes();
        fields.insert(
            "content_hash".to_owned(),
            Json::String(format!(
                "{}:{}",
                H::ALGORITHM.token(),
                H::hash(&preimage).to_token()
            )),
        );
        Json::Object(fields)
    }
}

/// A document this module will not derive a child from, or a root it will not write.
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
    /// INV-007's counting equation is false for the root about to be written.
    Unreconciled {
        /// The candidate universe the manifest partitions.
        candidates: u64,
        /// What the selection and the manifest actually account for.
        accounted: u64,
    },
    /// One identity is selected twice, so the selection is not a set.
    RepeatedSelection {
        /// The repeated identity.
        id: Name,
    },
    /// A handle is not of the class the key requires.
    UnclassedHandle {
        /// The key.
        key: &'static str,
    },
    /// `semantic_epoch` is absent, and `ReplayPreserving` is defined relative to it.
    EmptyEpoch,
    /// Rule C2's artifact half: `ReplayPreserving` with a null `replay`.
    ReplayPreservingWithoutReplay,
    /// A redaction stub with no commitment behind it.
    EmptyCommitment,
    /// The manifest the root would publish is not one manifest.
    Manifest(crate::omission::ManifestError),
    /// A profile's content constraints are not met by the compile it was asked of.
    ProfileViolation {
        /// The profile.
        profile: PackProfile,
        /// The clause it failed, verbatim from RFC 0028's "Pack profiles".
        clause: &'static str,
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
            Self::Unreconciled {
                candidates,
                accounted,
            } => write!(
                f,
                "INV-007 fails: {candidates} candidates against {accounted} accounted for by \
                 the selection and the manifest; a pack that cannot name what it dropped is \
                 malformed, not compact"
            ),
            Self::RepeatedSelection { id } => {
                write!(f, "`{id}` is selected twice; a selection is a set")
            }
            Self::UnclassedHandle { key } => {
                write!(f, "`{key}` names a handle of the wrong plan §4.4 class")
            }
            Self::EmptyEpoch => f.write_str(
                "`semantic_epoch` is empty, and every preserved guarantee is defined relative \
                 to it (RFC 0028 correction 9)",
            ),
            Self::ReplayPreservingWithoutReplay => f.write_str(
                "rule C2: a pack claiming `ReplayPreserving` MUST carry a non-null `replay`",
            ),
            Self::EmptyCommitment => f.write_str(
                "a `Redacted` stub with no commitment asserts that nothing stands behind the \
                 redaction (plan §18.4)",
            ),
            Self::Manifest(error) => write!(f, "{error}"),
            Self::ProfileViolation { profile, clause } => {
                write!(f, "the {profile} profile requires that {clause}")
            }
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

    // --- the root document -------------------------------------------------------------

    fn handle(class: ArtifactClass, token: &str) -> ArtifactHandle {
        ArtifactHandle::new(class, &Blake3Hasher::hash(token.as_bytes()).to_token())
            .expect("a digest token is a well-formed identity")
    }

    fn target() -> Target {
        Target::new(
            handle(ArtifactClass::IntentContract, "root-intent"),
            crate::target::Question::compiled("why did AckImpliesDurable fail?")
                .expect("a plain question"),
        )
        .expect("an in_* handle")
    }

    fn assurance() -> Assurance {
        Assurance::new(
            continuum_value::assurance::AssuranceLevel::Bounded,
            continuum_value::assurance::AssuranceEnvelope::all_unsupported(
                &continuum_value::assurance::UnsupportedReason::new("outside-this-test")
                    .expect("a plain token"),
            ),
        )
    }

    fn item(id: &str) -> SelectedItem {
        crate::state_delta::StateDeltaRef::new(
            name("client_acked"),
            crate::state_delta::StateDeltaClass::Concrete,
            Some(continuum_value::value::Value::int(0)),
            continuum_value::value::Value::int(1),
        )
        .expect("a real delta")
        .into_selected_item(name(id))
    }

    fn replay() -> ReplayRef {
        ReplayRef::new(handle(ArtifactClass::Crashpack, "root-crashpack")).expect("a crash_*")
    }

    /// The fixture: one selected item, one expandable omission of two, and nothing owed.
    struct RootFixture {
        identity: RootIdentity,
        target: Target,
        assurance: Assurance,
        selected: Vec<SelectedItem>,
        manifest: Manifest,
        evidence: Vec<ArtifactHandle>,
        replay: ReplayRef,
        guarantees: SealedGuarantees,
        redactions: Vec<RedactionStub>,
    }

    impl RootFixture {
        fn new() -> Self {
            let target = target();
            let evidence = vec![handle(ArtifactClass::Evidence, "root-evidence")];
            Self {
                identity: RootIdentity::derive::<Blake3Hasher>(
                    &target,
                    "ws_root1",
                    "sem3-root",
                    &evidence,
                ),
                target,
                assurance: assurance(),
                selected: vec![item("d_ack")],
                manifest: Manifest::new([OmissionRecord::expandable(
                    SelectionKind::StateDelta,
                    2,
                    OmissionReason::SliceIrrelevant,
                    query(),
                )])
                .expect("one cell"),
                evidence,
                replay: replay(),
                guarantees: crate::guarantee::GuaranteeSet::empty()
                    .seal()
                    .expect("the empty set seals"),
                redactions: Vec::new(),
            }
        }

        fn pack(&self) -> RootPack<'_> {
            RootPack {
                identity: &self.identity,
                snapshot: "ws_root1",
                semantic_epoch: "sem3-root",
                target: &self.target,
                verdict: Verdict::Refuted,
                assurance: &self.assurance,
                selected: &self.selected,
                manifest: &self.manifest,
                candidates: 3,
                unachieved: &[],
                evidence: &self.evidence,
                replay: Some(&self.replay),
                guarantees: &self.guarantees,
                redactions: &self.redactions,
                profile: PackProfile::Failure,
            }
        }
    }

    #[test]
    fn a_root_carries_every_required_key() {
        let fixture = RootFixture::new();
        let document = fixture
            .pack()
            .to_json::<Blake3Hasher>()
            .expect("conforming");
        required_keys_present(&document).expect("all seventeen required keys");
        let fields = document.as_object().expect("object");
        assert_eq!(fields["parent"], Json::Null, "a root has no parent");
        assert_eq!(
            identity_of(&document).expect("a pack handle"),
            *fixture.identity.handle()
        );
        // The measurement is the document's own canonical length, exactly as a child's is.
        assert_eq!(
            budget_bytes_of(&document).expect("measured"),
            document.to_canonical_bytes().len() as u64
        );
        // And it is deterministic.
        assert_eq!(
            document,
            fixture
                .pack()
                .to_json::<Blake3Hasher>()
                .expect("conforming")
        );
    }

    #[test]
    fn the_root_identity_is_a_function_of_the_question_alone() {
        // RFC 0028: "two `context.compile` requests differing only in `audience` MUST produce
        // the same `ctx_*`". `audience` has no path into the preimage at all — the strongest
        // form of that rule — and the anti-vacuity half is that the five terms that *are* in
        // it each move the handle.
        let evidence = vec![handle(ArtifactClass::Evidence, "root-evidence")];
        let base =
            RootIdentity::derive::<Blake3Hasher>(&target(), "ws_root1", "sem3-root", &evidence);
        // Root order is not part of the question.
        let two = vec![
            handle(ArtifactClass::Evidence, "b"),
            handle(ArtifactClass::Evidence, "a"),
        ];
        let mut reversed = two.clone();
        reversed.reverse();
        assert_eq!(
            RootIdentity::derive::<Blake3Hasher>(&target(), "ws_root1", "sem3-root", &two),
            RootIdentity::derive::<Blake3Hasher>(&target(), "ws_root1", "sem3-root", &reversed)
        );
        // Each of the five terms moves it.
        let other_question = Target::new(
            handle(ArtifactClass::IntentContract, "root-intent"),
            crate::target::Question::compiled("why did the replica stay durable?")
                .expect("a plain question"),
        )
        .expect("an in_* handle");
        assert_ne!(
            base,
            RootIdentity::derive::<Blake3Hasher>(
                &other_question,
                "ws_root1",
                "sem3-root",
                &evidence
            )
        );
        assert_ne!(
            base,
            RootIdentity::derive::<Blake3Hasher>(&target(), "ws_other", "sem3-root", &evidence)
        );
        assert_ne!(
            base,
            RootIdentity::derive::<Blake3Hasher>(&target(), "ws_root1", "sem3-other", &evidence)
        );
        assert_ne!(
            base,
            RootIdentity::derive::<Blake3Hasher>(&target(), "ws_root1", "sem3-root", &two)
        );
    }

    #[test]
    fn the_six_list_fields_are_deterministically_ordered() {
        // `rule ordering.deterministic`: the six list-valued fields are ordered by a declared
        // sort key, so a caller's ordering of its inputs is not part of the artifact.
        let mut fixture = RootFixture::new();
        fixture.selected = vec![item("d_write"), item("d_ack"), item("d_begin")];
        fixture.evidence = vec![
            handle(ArtifactClass::Evidence, "z"),
            handle(ArtifactClass::Evidence, "a"),
        ];
        fixture.identity = RootIdentity::derive::<Blake3Hasher>(
            &fixture.target,
            "ws_root1",
            "sem3-root",
            &fixture.evidence,
        );
        fixture.redactions = vec![
            RedactionStub::new(
                name("d_zeta"),
                RedactionReason::Purged,
                "blake3-256:zzzz",
                ArtifactClass::Evidence,
            )
            .expect("a commitment"),
            RedactionStub::new(
                name("d_alpha"),
                RedactionReason::Summarized,
                "blake3-256:aaaa",
                ArtifactClass::Evidence,
            )
            .expect("a commitment"),
        ];
        let mut pack = fixture.pack();
        pack.candidates = 5;
        let document = pack.to_json::<Blake3Hasher>().expect("conforming");
        let fields = document.as_object().expect("object");
        let ids: Vec<&str> = fields["selected"]
            .as_array()
            .expect("array")
            .iter()
            .map(|item| {
                item.as_object().expect("object")["id"]
                    .as_str()
                    .expect("a string")
            })
            .collect();
        let mut sorted = ids.clone();
        sorted.sort_by_key(|id| name(id));
        assert_eq!(
            ids, sorted,
            "`selected` is in the items' own canonical order"
        );
        let evidence: Vec<&str> = fields["evidence"]
            .as_array()
            .expect("array")
            .iter()
            .map(|handle| handle.as_str().expect("a string"))
            .collect();
        let mut sorted_evidence = evidence.clone();
        sorted_evidence.sort_unstable();
        assert_eq!(evidence, sorted_evidence, "`evidence` is sorted");
        let redacted: Vec<&str> = fields["redactions"]
            .as_array()
            .expect("array")
            .iter()
            .map(|stub| {
                stub.as_object().expect("object")["commitment"]
                    .as_str()
                    .expect("a string")
            })
            .collect();
        // `d_zeta` before `d_alpha`: `continuum_value::value::Name` orders **shortlex**
        // (`compare_blobs`), so the shorter identity sorts first and this is the declared sort
        // key doing its job rather than a lexicographic accident.
        assert_eq!(
            redacted,
            ["blake3-256:zzzz", "blake3-256:aaaa"],
            "`redactions` is sorted by the candidate each stands for, in `Name` order"
        );
        assert!(
            name("d_zeta") < name("d_alpha"),
            "shortlex, not lexicographic"
        );
    }

    #[test]
    fn an_unreconciled_root_is_refused() {
        // INV-007 is checked before the document exists: a pack that cannot name what it
        // dropped is no answer, not a smaller one.
        let fixture = RootFixture::new();
        let mut pack = fixture.pack();
        pack.candidates = 4;
        assert_eq!(
            pack.to_json::<Blake3Hasher>(),
            Err(PackError::Unreconciled {
                candidates: 4,
                accounted: 3
            })
        );
        // Anti-vacuity: the honest count writes.
        assert!(fixture.pack().to_json::<Blake3Hasher>().is_ok());
    }

    #[test]
    fn an_unachieved_request_is_an_unsupported_omission() {
        // Rule C1's record, and the candidate universe it grows.
        let fixture = RootFixture::new();
        let unachieved = [Guarantee::ReplayPreserving, Guarantee::PropertyPreserving];
        let mut pack = fixture.pack();
        pack.unachieved = &unachieved;
        let manifest = pack.published_manifest().expect("merges");
        let owed = manifest
            .records()
            .iter()
            .find(|record| record.kind() == SelectionKind::Unknown)
            .expect("rule C1's record");
        assert_eq!(owed.count(), 2);
        assert_eq!(owed.reason(), OmissionReason::Unsupported);
        assert!(
            !owed.retrievability().is_expandable(),
            "no expansion retrieves a claim that was never established"
        );
        assert_eq!(pack.published_candidates().expect("counts"), 5);
        let document = pack.to_json::<Blake3Hasher>().expect("conforming");
        assert!(
            !document.as_object().expect("object")["guarantees"]
                .as_array()
                .expect("array")
                .iter()
                .any(|claim| claim.as_str() == Some("ReplayPreserving")),
            "a daemon MUST NOT echo a requested guarantee it did not achieve"
        );
    }

    #[test]
    fn a_replay_preserving_root_without_a_replay_is_refused() {
        // Rule C2's artifact half. The other half — `ReplayPreserving` without
        // `CausallyClosed` — is unspellable: `GuaranteeSet::seal` refuses it, so the only
        // sealed set reaching here that claims replay preservation also claims closure.
        let fixture = RootFixture::new();
        let mut guarantees = crate::guarantee::GuaranteeSet::empty();
        guarantees.claim(crate::guarantee::License::issue(Guarantee::CausallyClosed));
        guarantees.claim(crate::guarantee::License::issue(
            Guarantee::ReplayPreserving,
        ));
        let sealed = guarantees.seal().expect("C2 is satisfied");
        let mut pack = fixture.pack();
        pack.guarantees = &sealed;
        pack.replay = None;
        assert_eq!(
            pack.to_json::<Blake3Hasher>(),
            Err(PackError::ReplayPreservingWithoutReplay)
        );
        pack.replay = Some(&fixture.replay);
        assert!(pack.to_json::<Blake3Hasher>().is_ok());
    }

    #[test]
    fn each_profile_refuses_its_own_violation() {
        let fixture = RootFixture::new();
        let violated = |pack: RootPack<'_>, profile: PackProfile| match pack
            .to_json::<Blake3Hasher>()
        {
            Err(PackError::ProfileViolation { profile: named, .. }) => assert_eq!(named, profile),
            other => panic!("the {profile} profile must refuse: {other:?}"),
        };
        // Failure: a satisfied verdict is not a failure pack's question, and a null `replay`
        // is not a failure pack's replay.
        let mut pack = fixture.pack();
        pack.verdict = Verdict::Satisfied;
        violated(pack, PackProfile::Failure);
        let mut pack = fixture.pack();
        pack.replay = None;
        violated(pack, PackProfile::Failure);
        // Reachability: bounds and schedules must name a producing engine.
        let mut pack = fixture.pack();
        pack.profile = PackProfile::Reachability;
        violated(pack, PackProfile::Reachability);
        // Invariant: assumption items are required, and this fixture selects a delta.
        let mut pack = fixture.pack();
        pack.profile = PackProfile::Invariant;
        violated(pack, PackProfile::Invariant);
        // Proof-target: `ProofRelevant` or a typed `inconclusive`. This fixture is `refuted`
        // and claims nothing, so it must refuse — and the typed-inconclusive escape admits it.
        let mut pack = fixture.pack();
        pack.profile = PackProfile::ProofTarget;
        violated(pack, PackProfile::ProofTarget);
        let mut pack = fixture.pack();
        pack.profile = PackProfile::ProofTarget;
        pack.verdict = Verdict::Inconclusive(
            continuum_value::assurance::InconclusiveReason::IncompleteProofSearch,
        );
        assert!(
            pack.to_json::<Blake3Hasher>().is_ok(),
            "a proof-target pack that states why it cannot claim `ProofRelevant` is admitted"
        );
        // And the honest failure pack still writes.
        assert!(fixture.pack().to_json::<Blake3Hasher>().is_ok());
    }

    #[test]
    fn a_root_refuses_an_unclassed_handle_an_empty_epoch_and_a_repeated_selection() {
        let fixture = RootFixture::new();
        let mut pack = fixture.pack();
        pack.snapshot = "not_a_snapshot";
        assert_eq!(
            pack.to_json::<Blake3Hasher>(),
            Err(PackError::UnclassedHandle { key: "snapshot" })
        );
        let mut pack = fixture.pack();
        pack.semantic_epoch = "  ";
        assert_eq!(pack.to_json::<Blake3Hasher>(), Err(PackError::EmptyEpoch));
        let not_evidence = vec![handle(ArtifactClass::Crashpack, "wrong-class")];
        let mut pack = fixture.pack();
        pack.evidence = &not_evidence;
        assert_eq!(
            pack.to_json::<Blake3Hasher>(),
            Err(PackError::UnclassedHandle { key: "evidence" })
        );
        let twice = vec![item("d_ack"), item("d_ack")];
        let mut pack = fixture.pack();
        pack.selected = &twice;
        pack.candidates = 4;
        assert_eq!(
            pack.to_json::<Blake3Hasher>(),
            Err(PackError::RepeatedSelection { id: name("d_ack") })
        );
        // A redaction stub with no commitment asserts that nothing stands behind it.
        assert_eq!(
            RedactionStub::new(
                name("d_x"),
                RedactionReason::Lost,
                "   ",
                ArtifactClass::Evidence
            ),
            Err(PackError::EmptyCommitment)
        );
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
