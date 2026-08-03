//! The pack's shared `selected[]` item shape (RFC 0028 "Selection and the causal core";
//! `context-pack.schema.json` `selected`), and the closed eleven-member `kind` vocabulary
//! every selection kind — this bullet's two and the other nine — is drawn from.
//!
//! # Scope: PR-11 / IMPL-03, not the whole array
//!
//! > relevant source spans and model actions | `selected[].kind` in `source`, `model` |
//! > array items | Source is untrusted data and MUST NOT be interpolated into any
//! > description (INV-016)
//! >
//! > — RFC 0028, "Required fields, reconciled with plan §6.2"
//!
//! This bone owns two of the schema's eleven `kind` members. [`SelectionKind`] is
//! nonetheless declared with all eleven, for the reason `continuum-evidence`'s
//! `EdgeRelation` was declared with all thirteen edge kinds when its own bullet needed
//! only seven: "an enum missing [members] of a closed schema enum is a vocabulary that
//! cannot round-trip a conforming instance" (`crates/continuum-evidence/src/edge.rs`).
//! [`SelectedItem`], the shared wire-shape record every kind projects into, is likewise
//! shared infrastructure. What is genuinely this bullet's own is the *typed construction*
//! path: [`SelectedItem::new`] is `pub(crate)`, so nothing outside this crate — and, until
//! their own bones land, nothing inside it either — can build one except through
//! [`crate::source::SourceRef::into_selected_item`] and
//! [`crate::model::ModelActionRef::into_selected_item`]. The other nine kinds have a wire
//! token each (round-trippable, fail-closed) and no typed reference and no way to
//! construct an item of that kind: that is IMPL-01/02/04/05/06's scope, not silently
//! claimed here.
//!
//! # Why a `SelectedItem` cannot carry caller-supplied prose
//!
//! `summary` "is a rendering, never a fact of record. It MUST NOT be the only place a
//! machine-consumable fact appears (INV-003), and MUST NOT interpolate source, logs,
//! model text, or production payloads (INV-016)." The schema cannot enforce that — it
//! types `summary` as an unconstrained string — so it is enforced by *shape*, not by
//! validation: [`SelectedItem::new`] takes an already-built `summary: String` but is
//! private to this crate, and the two callers that exist
//! (`SourceRef::into_selected_item`, `ModelActionRef::into_selected_item`) take no string
//! parameter at all. There is no argument through which foreign text could reach a
//! `source` or `model` item's summary; it is synthesized from the reference's own typed
//! fields (a [`crate::source::SourceSpan`]'s position, a
//! [`crate::model::ModelActionRef`]'s action name and model handle) by `Display`/`format!`
//! over those types alone.
//!
//! # Canonical encoding: reused, not reinvented
//!
//! `context-pack.schema.json` is a JSON Schema document, exactly as
//! `intent-contract.schema.json` is, and RFC 0037 ID5 already answered "what is the one
//! canonical byte spelling of a JSON-schema-normative Continuum artifact": canonical JSON
//! — sorted keys, no insignificant whitespace, one escape spelling — not
//! `continuum_value::value::Value`'s CVNF-1 (that encoding is for the *exact finite value*
//! domain: states, model values; see `continuum_intent::canonical_json`'s own module
//! doc, "Which layer owns which spelling"). [`SelectedItem::to_json`] therefore builds a
//! `continuum_intent::canonical_json::Json` object and reuses that crate's writer —
//! already a cross-crate dependency in this workspace (`continuum-benchmark` imports it
//! directly) — rather than hand-rolling a second canonical-JSON algorithm here.
//!
//! # What is declined
//!
//! - **Per-item content identity (ADR-0013).** The schema gives no `selected[]` item its
//!   own `content_hash`; only the whole pack does, and assembling the pack is outside this
//!   bullet's fence. What this module gives instead is exact structural equality on typed
//!   fields (`SourceRef`, `ModelActionRef`, and `SelectedItem` all derive `PartialEq`/`Ord`)
//!   and a canonical JSON rendering, which is what a future whole-pack assembly needs to
//!   satisfy `rule ordering.deterministic` over `selected[]` — it is not itself that
//!   assembly.
//! - **Expansion-handle integration.** `source_span` is one of `ExpansionRelation`'s eleven
//!   members (the IDL), which is exactly the mechanism a `source` item would anchor. That
//!   machinery — `ExpansionRelation`, `expansions[]`, the omission manifest — is IMPL-04's
//!   (bn-28jj), unopened at the time this bone landed. `SelectedItem::id` is the anchor
//!   shape a future `expansions[].anchor` would name; nothing more is claimed.
//!
//! # Clause → test
//!
//! | Clause | Source | Test |
//! |---|---|---|
//! | eleven kinds, the schema's tokens, in the schema's order | `context-pack.schema.json` `selected.items.kind` | `the_kind_set_is_exactly_the_schema_enum` |
//! | fail closed on an unrecognized token | RFC 0028, "Versioning and revision" | `an_unknown_kind_token_does_not_parse` |
//! | every kind round-trips through its wire token | RFC 0028 | `every_kind_round_trips_through_its_wire_token` |
//! | `artifact` is optional and nullable | `context-pack.schema.json` `selected.items.required` | `artifact_is_present_as_json_null_when_absent` |
//! | the four keys are exactly the schema's, ID5-sorted | `context-pack.schema.json`, RFC 0037 ID5 (reused discipline) | `canonical_json_has_exactly_the_schema_keys_in_sorted_order` |
//! | `SelectedItem::new` is not constructible outside this crate | this bone's INV-016 reading | `compile_fail` doctest below |
//! | two items are equal iff their four fields agree | `rule ordering.deterministic` | `equality_is_structural_on_all_four_fields` |
//!
//! ```compile_fail
//! // `SelectedItem::new` is `pub(crate)`: an external crate cannot fabricate an item
//! // naming an arbitrary summary for a `source` or `model` kind.
//! use continuum_context::selection::{SelectedItem, SelectionKind};
//! use continuum_value::value::Name;
//!
//! let _ = SelectedItem::new(
//!     Name::new("x").unwrap(),
//!     SelectionKind::Source,
//!     "whatever a caller likes".to_owned(),
//!     None,
//! );
//! ```

use core::fmt;

use continuum_intent::canonical_json::Json;
use continuum_value::value::Name;
use continuum_workspace::artifact_path::ArtifactHandle;

/// One member of `context-pack.schema.json`'s closed `selected[].kind` enum.
///
/// Declared in the schema's own enum order, which is also this type's [`Ord`] — a sort by
/// kind is therefore stable across processes and platforms, matching the discipline
/// `continuum_workspace::artifact_path::ArtifactClass` states for the same reason.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SelectionKind {
    /// `event` — an event directly observed by the property, or a causal predecessor.
    Event,
    /// `state_delta` — a concrete or abstract state delta, never a whole state.
    StateDelta,
    /// `source` — a source span (this bullet).
    Source,
    /// `model` — a model action (this bullet).
    Model,
    /// `proof` — an item on a named obligation's proof-dependency slice.
    Proof,
    /// `assumption` — an assumption used, or notably unused, by the evaluation.
    Assumption,
    /// `counterfactual` — a counterfactual safe branch under a named causality model.
    Counterfactual,
    /// `obligation_flow` — a cancellation or resource transfer (B19).
    ObligationFlow,
    /// `order_constraint` — a missing or reversed order constraint.
    OrderConstraint,
    /// `repair_surface` — a candidate repair surface, always heuristic (rule C4).
    RepairSurface,
    /// `unknown` — plan §12.2's typed uncertainty item; the pack-level form of INV-008.
    Unknown,
}

impl SelectionKind {
    /// Every kind, in the schema's declaration order.
    pub const ALL: [Self; 11] = [
        Self::Event,
        Self::StateDelta,
        Self::Source,
        Self::Model,
        Self::Proof,
        Self::Assumption,
        Self::Counterfactual,
        Self::ObligationFlow,
        Self::OrderConstraint,
        Self::RepairSurface,
        Self::Unknown,
    ];

    /// The wire token, byte-identical to the schema's `kind` enum.
    #[must_use]
    pub const fn as_wire_str(self) -> &'static str {
        match self {
            Self::Event => "event",
            Self::StateDelta => "state_delta",
            Self::Source => "source",
            Self::Model => "model",
            Self::Proof => "proof",
            Self::Assumption => "assumption",
            Self::Counterfactual => "counterfactual",
            Self::ObligationFlow => "obligation_flow",
            Self::OrderConstraint => "order_constraint",
            Self::RepairSurface => "repair_surface",
            Self::Unknown => "unknown",
        }
    }

    /// Recover a kind from its wire token.
    ///
    /// `None` for anything not in the closed set — a caller reading a pack MUST reject an
    /// unrecognized token (`MalformedRequest`) rather than treat it as absent or as
    /// `HeuristicRelevant` (RFC 0028, "Fail closed on unrecognized tokens"); returning
    /// `Option` rather than defaulting is what makes that the only reading available.
    #[must_use]
    pub fn from_wire_str(text: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|kind| kind.as_wire_str() == text)
    }
}

impl fmt::Display for SelectionKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_wire_str())
    }
}

/// The generic `selected[]` item shape every kind projects into: `{id, kind, summary,
/// artifact}` (`context-pack.schema.json` `selected.items`).
///
/// `id`, `kind`, and `summary` are required by the schema; `artifact` is optional and
/// nullable ("`null` means the item is derived and has no standalone artifact; it does not
/// mean the item is unverifiable" — RFC 0028, "Selection and the causal core"). This type
/// always emits the `artifact` key, using JSON `null` when absent, matching the shipped
/// example's own convention (`schemas/examples/context-pack.example.json`, the `d_abs`
/// item) rather than omitting a schema-optional key that determinism then has to reason
/// about two ways.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct SelectedItem {
    id: Name,
    kind: SelectionKind,
    summary: String,
    artifact: Option<ArtifactHandle>,
}

impl SelectedItem {
    /// Build an item.
    ///
    /// `pub(crate)`, and reached today only by
    /// [`crate::source::SourceRef::into_selected_item`] and
    /// [`crate::model::ModelActionRef::into_selected_item`] — see the module
    /// documentation's "Why a `SelectedItem` cannot carry caller-supplied prose".
    pub(crate) fn new(
        id: Name,
        kind: SelectionKind,
        summary: String,
        artifact: Option<ArtifactHandle>,
    ) -> Self {
        Self {
            id,
            kind,
            summary,
            artifact,
        }
    }

    /// The item's `selected[].id` — the anchor a future `expansions[].anchor` would name.
    #[must_use]
    pub const fn id(&self) -> &Name {
        &self.id
    }

    /// The item's closed `kind`.
    #[must_use]
    pub const fn kind(&self) -> SelectionKind {
        self.kind
    }

    /// The rendering-only summary. Never the only place a fact of record appears
    /// (INV-003), and never interpolated source, log, or model text (INV-016) — see the
    /// module documentation.
    #[must_use]
    pub fn summary(&self) -> &str {
        &self.summary
    }

    /// The item's content-addressed artifact, when it has a standalone one.
    #[must_use]
    pub const fn artifact(&self) -> Option<&ArtifactHandle> {
        self.artifact.as_ref()
    }

    /// The canonical JSON rendering of this item: `{"artifact", "id", "kind", "summary"}`,
    /// in that (ID5-sorted) key order, via `continuum_intent::canonical_json::Json` — this
    /// crate's one writer, not a second encoding.
    #[must_use]
    pub fn to_json(&self) -> Json {
        Json::object([
            (
                "artifact".to_owned(),
                self.artifact
                    .as_ref()
                    .map_or(Json::Null, |handle| Json::String(handle.to_string())),
            ),
            ("id".to_owned(), Json::String(self.id.to_string())),
            (
                "kind".to_owned(),
                Json::String(self.kind.as_wire_str().to_owned()),
            ),
            ("summary".to_owned(), Json::String(self.summary.clone())),
        ])
        .expect("the four keys above are pairwise distinct string literals")
    }

    /// The canonical JSON bytes of [`to_json`](Self::to_json).
    #[must_use]
    pub fn to_canonical_bytes(&self) -> Vec<u8> {
        self.to_json().to_canonical_bytes()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Transcribed from `notes/plan/schemas/context-pack.schema.json`,
    /// `properties.selected.items.properties.kind.enum`, in file order.
    const SCHEMA_KIND_ENUM: [&str; 11] = [
        "event",
        "state_delta",
        "source",
        "model",
        "proof",
        "assumption",
        "counterfactual",
        "obligation_flow",
        "order_constraint",
        "repair_surface",
        "unknown",
    ];

    #[test]
    fn the_kind_set_is_exactly_the_schema_enum() {
        let ours: Vec<&str> = SelectionKind::ALL.iter().map(|k| k.as_wire_str()).collect();
        assert_eq!(ours, SCHEMA_KIND_ENUM);
    }

    #[test]
    fn every_kind_round_trips_through_its_wire_token() {
        for kind in SelectionKind::ALL {
            assert_eq!(SelectionKind::from_wire_str(kind.as_wire_str()), Some(kind));
        }
    }

    #[test]
    fn an_unknown_kind_token_does_not_parse() {
        // Anti-vacuity companion to the round-trip test above: `from_wire_str` must
        // actually distinguish valid from invalid tokens, not accept everything.
        for bad in ["", "Source", "source ", "sources", "SOURCE", "unknown2"] {
            assert_eq!(
                SelectionKind::from_wire_str(bad),
                None,
                "{bad:?} must be refused"
            );
        }
    }

    fn item(kind: SelectionKind, artifact: Option<ArtifactHandle>) -> SelectedItem {
        SelectedItem::new(
            Name::new("x_1").expect("well formed"),
            kind,
            "a summary".to_owned(),
            artifact,
        )
    }

    #[test]
    fn artifact_is_present_as_json_null_when_absent() {
        let with_none = item(SelectionKind::Source, None);
        let fields = with_none.to_json();
        let object = fields.as_object().expect("object");
        assert!(object.contains_key("artifact"));
        assert!(object["artifact"].is_null());
    }

    #[test]
    fn canonical_json_has_exactly_the_schema_keys_in_sorted_order() {
        let value = item(SelectionKind::Model, None);
        let bytes = value.to_canonical_bytes();
        let text = String::from_utf8(bytes).expect("utf8");
        assert_eq!(
            text,
            r#"{"artifact":null,"id":"x_1","kind":"model","summary":"a summary"}"#
        );
    }

    #[test]
    fn equality_is_structural_on_all_four_fields() {
        let a = item(SelectionKind::Source, None);
        let b = item(SelectionKind::Source, None);
        assert_eq!(a, b);
        assert_eq!(a.to_canonical_bytes(), b.to_canonical_bytes());

        let different_kind = item(SelectionKind::Model, None);
        assert_ne!(a, different_kind);
        assert_ne!(a.to_canonical_bytes(), different_kind.to_canonical_bytes());
    }
}
