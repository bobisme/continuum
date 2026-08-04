//! `exact equality` — RFC 0031's whole-contract `unchanged` shortcut (PR-12 /
//! IMPL-01).
//!
//! # What this module answers
//!
//! RFC 0031 names "exact equality" as the first of PR 12's six classification
//! bullets and "unchanged intent" as the last of plan §5.3's ten (the two are one
//! bullet under two names — see "Which text this derives from" below). Read
//! against the RFC's own normative prose rather than against the two words of the
//! bullet title, it is this paragraph:
//!
//! > **`unchanged` shortcut.** If `before_intent` and `after_intent` are the same
//! > `in_*` identity, `intent_changes` MUST be empty. Identity is over the
//! > canonical encoding of semantic content (RFC 0037), so equal identity entails
//! > every field unchanged. The converse does not hold: distinct identities MUST
//! > be classified field by field and MUST NOT be reported as a whole-contract
//! > change.
//! >
//! > — `notes/plan/rfcs/0031-semantic-and-intent-diff.md`, "Inputs, preconditions,
//! > and determinism"
//!
//! and R2, restated at the whole-document grain:
//!
//! > **R2 — `unchanged` on equality and nothing weaker.** `unchanged` MUST be
//! > claimed only on equality of the canonical encoding of the classified unit:
//! > the CPNF-1 N8 encoding for units carrying a property AST (RFC 0037 S1), and
//! > RFC 0037's canonical contract encoding otherwise. Textual match, partial
//! > structural match, and label/comment equality MUST NOT be used.
//! >
//! > — RFC 0031, "Classification lattice"
//!
//! R2 names two forms of canonical encoding a classified unit's equality may be
//! judged against: CPNF-1 for a property/assumption/fairness-condition AST, and
//! "RFC 0037's canonical contract encoding otherwise". This module is the
//! "otherwise" branch taken at the coarsest possible unit — the whole contract —
//! which is exactly [`continuum_intent::contract::IntentIdentity`]:
//! `continuum-intent`'s own ID1/ID2 preimage type, already "the canonical
//! encoding of semantic content" the shortcut's own sentence names. [`classify`]
//! does not reimplement that encoding; it is a checked comparison over the type
//! that already carries it, exactly as [`crate::bounds::classify_bounds`]
//! delegates the componentwise order to `Bounds::relation_to` rather than
//! recomputing it.
//!
//! # Which text this derives from, and why the two-word bullet title is not enough
//!
//! `notes/START_HERE_IMPLEMENTATION.md`'s PR-12 lists "exact equality" first, with
//! no further prose of its own — unlike its five siblings, which each got a
//! one-paragraph gloss once delivered. Two words are not a specification, and
//! this module's brief says as much: derive the semantics from RFC 0031's text,
//! not from the bullet's title. Plan §5.3's ten-item list is the crosswalk that
//! fixes the reading. Its first nine items are the seven G3 directional
//! dimensions plus "abstraction merge/split" (RFC 0031's `abstraction_maps`,
//! outside PR-12's six bullets entirely) and "unsupported semantic change" (not a
//! field classification but the fail-closed rule every sibling module already
//! implements for its own field); its **tenth and last item is "unchanged
//! intent"** — the one item with no directional order, no per-field unit, and no
//! sibling bone among PR-12's other five (`property AST edit`, `assumption
//! add/remove`, `bound change`, `observer event change`,
//! `fault/fairness/assurance change` are all directional or membership
//! classifications on ONE of the fifteen fields; "unchanged intent" is not).
//! "Exact equality" is plan §5.3's "unchanged intent" under PR-12's own
//! vocabulary, and RFC 0031's "Inputs, preconditions, and determinism" section is
//! the only place in the dossier that gives "unchanged intent" a normative rule
//! of its own: the `unchanged` shortcut quoted above. No other RFC 0031 passage
//! names a classification at the whole-contract grain — every other rule in
//! "Classification lattice" and "Field-by-field classification rules" is scoped
//! to one of the fifteen fields, which is what the other five PR-12 bullets and
//! bones exist to cover.
//!
//! # Why canonical bytes, not the `in_*` handle string
//!
//! The shortcut's own sentence says "the same `in_*` identity", and RFC 0037's
//! `in_*` handle is literally a hash digest (`IntentContract::mint_intent_id`:
//! `format!("in_{}", self.identity.digest::<H>().to_token())`) — so a *literal*
//! reading could compare the minted wire strings. This module compares
//! [`IntentIdentity`] — the ID1/ID2 canonical-bytes preimage the handle is minted
//! *from* — instead, for a reason internal to this crate's own upstream
//! dependency, not a guess:
//!
//! > Per ADR-0013 this type *is* the preimage bytes … not a digest of them. Those
//! > bytes are the canonical JSON encoding of an object carrying **exactly these
//! > eighteen keys** …
//! >
//! > — `continuum_intent::contract`, module doc
//!
//! and ADR-0013 itself: "Canonical structural encodings define identity. Hashes
//! index and partition; collisions resolve by exact comparison." `IntentId` (the
//! `in_*` wire token) is exactly the kind of thing ADR-0013 says must never *be*
//! the identity — comparing it as if it were would make this module's answer
//! depend on the hasher's collision resistance rather than on content, and RFC
//! 0031's own "Determinism" clause a few lines above the shortcut requires the
//! artifact to be "byte-identical across platforms and releases", which a
//! hash-digest comparison cannot improve on and canonical-byte comparison already
//! gives for free. `IntentContract::identity` is `&IntentIdentity` for exactly
//! this reason (its own doc: "not a digest of them"), so `before_intent`/
//! `after_intent` resolve, in this crate's vocabulary, to `&IntentIdentity`, and
//! that is what [`classify`] takes. This is not read as a divergence from RFC
//! 0031's text — "the same `in_*` identity" and "equal identity" in the same
//! paragraph is RFC 0031 using "identity" for the semantic content the handle
//! names, exactly as its own next sentence glosses it ("Identity is over the
//! canonical encoding of semantic content") — but it is recorded here because a
//! literal reading of the four-character token `in_*` could go the other way,
//! and `continuum-intent`'s own ADR-0013 discipline is what closes the question.
//!
//! # What this module is not
//!
//! - **Not a per-field comparator.** [`classify`] never inspects a `claims[]`
//!   entry, a `bounds` tuple, or any of the other fourteen groups; it compares
//!   one already-computed [`IntentIdentity`] against another. The CPNF-1 half of
//!   R2 — property, assumption, and fairness-condition equality — is `bn-8mlg`
//!   (IMPL-02) and `bn-7vg7` (IMPL-03)'s, and the other twelve fields' own
//!   equality is each field-group type's `PartialEq`, already exact by
//!   construction (a typed field-group value *is* its own canonical form; there
//!   is no second, looser equality anywhere in `continuum-intent` for this
//!   module to be sloppier than). Nothing here duplicates that work.
//! - **Not a source of directions.** When the shortcut does not apply, this
//!   module reports exactly that — [`Equality::Distinct`] — and stops. It never
//!   guesses which fields moved or in which direction; "distinct identities MUST
//!   be classified field by field" is the other five bullets' job, most of them
//!   still open bones.
//! - **Not the wire `intent_changes[]` shape.** RFC 0031 correction 14 is explicit
//!   that the shortcut's wire spelling is an **empty** array, not fifteen
//!   explicit `unchanged` records — a schema example shipped that mistake and was
//!   corrected. [`unchanged_classification`] below returns fifteen records, and
//!   that is *not* correction 14's mistake repeated: those records are not wire
//!   `intent_changes[]` output, they are the complete-classification input
//!   `PolicyTable::verdict` itself demands ("a verdict is computed from a
//!   complete classification, so the caller supplies the `unchanged` records
//!   explicitly" — `continuum_intent::change_policy::PolicyTable::verdict`'s own
//!   doc). Whatever later bone assembles the wire artifact is responsible for
//!   emitting the empty array on the wire while still handing `verdict` a
//!   complete classification internally — the same gap [`crate::bounds`] and
//!   [`crate::observers`] already leave for their own one-field records ("RFC
//!   0031 permits a wire artifact to omit `unchanged` records … but that is an
//!   emission choice for whatever assembles the wire … array, not a property of
//!   the classifier").
//!
//! # Scope: PR-12 / IMPL-01 only
//!
//! PR 12 (`notes/START_HERE_IMPLEMENTATION.md`) lists six classification
//! bullets: **exact equality**, property AST edit, assumption add/remove, bound
//! change, observer event change, and fault/fairness/assurance change. This bone
//! (`bn-b8ru`) delivers the first, and only the first. The other five are
//! separate bones — `bn-8mlg` (IMPL-02, property AST edit), `bn-7vg7` (IMPL-03,
//! assumption add/remove), `bn-ycn6` (IMPL-04, bound change, landed as
//! [`crate::bounds`]), `bn-1sdp` (IMPL-05, observer event change, landed as
//! [`crate::observers`]), `bn-3vxp` (IMPL-06, fault/fairness/assurance change,
//! landed as [`crate::faults`], [`crate::fairness`], [`crate::assurance`]). Also
//! out of scope, honestly: the wire `intent_changes[]` JSON shape and the
//! whole-artifact assembly (see "What this module is not", above); resolving an
//! `in_*` handle against a registry (RFC 0031: "the daemon resolves it" — a
//! `continuumd` concern, not a classifier's); and both snapshots' sealedness
//! check ("An unsealed snapshot is rejected (`StaleSnapshot`, RFC 0026)"), which
//! is a snapshot-layer precondition this module's two identity inputs do not
//! carry enough to enforce.

use continuum_intent::change_policy::{ClassificationRecord, PolicyField, Relation};
use continuum_intent::contract::IntentIdentity;

/// The result of RFC 0031's `unchanged` shortcut, applied to two Intent Contract
/// identities.
///
/// Two members only — this is not a third copy of [`Relation`], it is the
/// precondition that decides whether any per-field relation needs computing at
/// all. See the module doc, "Not a source of directions".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Equality {
    /// `before` and `after` are the same `in_*` identity (equal canonical
    /// preimage bytes). RFC 0031: "equal identity entails every field
    /// unchanged" — every one of the fifteen protected fields may be classified
    /// [`Relation::Unchanged`] without inspecting a single one of them.
    Unchanged,
    /// `before` and `after` are distinct identities. RFC 0031: "distinct
    /// identities MUST be classified field by field and MUST NOT be reported as
    /// a whole-contract change" — this module makes no further claim, for any
    /// field, in this case.
    Distinct,
}

impl Equality {
    /// Whether the shortcut applies.
    #[must_use]
    pub const fn is_unchanged(self) -> bool {
        matches!(self, Self::Unchanged)
    }
}

/// Classify whether `before` and `after` are RFC 0031's `unchanged` shortcut:
/// the same `in_*` identity.
///
/// Total, and the only comparison this module performs: [`IntentIdentity`]'s
/// [`PartialEq`] is over its canonical preimage bytes (ADR-0013), so this is
/// exactly "equality of the canonical encoding of the classified unit" (R2)
/// where the classified unit is the whole contract — see the module doc, "Why
/// canonical bytes, not the `in_*` handle string".
#[must_use]
pub fn classify(before: &IntentIdentity, after: &IntentIdentity) -> Equality {
    if before == after {
        Equality::Unchanged
    } else {
        Equality::Distinct
    }
}

/// The complete fifteen-field classification the shortcut licenses, or `None`
/// when it does not apply.
///
/// `Some` only when [`classify`] returns [`Equality::Unchanged`], and then
/// exactly one [`ClassificationRecord`] per [`PolicyField`], every one carrying
/// [`Relation::Unchanged`] — the concrete records RFC 0031's "equal identity
/// entails every field unchanged" licenses, assembled once here so a caller with
/// two equal identities never has to loop over the fifteen fields by hand to
/// satisfy `PolicyTable::verdict`'s P1 completeness rule. This is verdict-input,
/// not wire output — see the module doc, "Not the wire `intent_changes[]`
/// shape" — so it is exactly as many records when the shortcut fires as when it
/// does not: zero on the wire either way, fifteen for `verdict` either way (the
/// non-shortcut fifteen being the other five bullets' job, most still open).
///
/// The `expect` inside cannot fail: every one of the fifteen fields' own
/// admissible-relation row lists `Unchanged` first
/// (`continuum_intent::change_policy::PolicyField::classified_relations`), so
/// `ClassificationRecord::new(field, Relation::Unchanged)` succeeds for every
/// `field` in [`PolicyField::ALL`] — the same reasoning
/// [`continuum_intent::change_policy::PolicyTable::all_unlocked`] already
/// relies on for its own always-succeeds `expect`.
#[must_use]
pub fn unchanged_classification(
    before: &IntentIdentity,
    after: &IntentIdentity,
) -> Option<[ClassificationRecord; 15]> {
    if !classify(before, after).is_unchanged() {
        return None;
    }
    Some(PolicyField::ALL.map(|field| {
        ClassificationRecord::new(field, Relation::Unchanged).expect(
            "`unchanged` is the first relation on every policy field's own admissible-relation \
             row (RFC 0031 R2; PolicyField::classified_relations)",
        )
    }))
}

#[cfg(test)]
mod tests {
    use continuum_intent::change_policy::PolicyField;
    use continuum_intent::contract::IntentContract;

    use super::*;

    /// The dossier's validated Intent Contract example — the same fixture
    /// `continuum-intent`'s own PR-4 exit and INV-012 evidence use, and the same
    /// one `crate::bounds`/`crate::observers`'s tests read (`crate::bounds`'s test
    /// module reads a different fixture file, the schema; the corpus fixture
    /// itself is `crate::observers`'s and `crate::faults`'s evidence file's
    /// shared baseline). Read at compile time via `include_str!`; every mutation
    /// below is one bounded, linear `.replacen` on this fixed document, never a
    /// loop or a hand-authored contract.
    const FIXTURE: &str =
        include_str!("../../continuum-intent/tests/fixtures/replicated-register-contract.json");

    fn decode(document: &str) -> IntentContract {
        IntentContract::decode(document.trim_end().as_bytes()).expect("the fixture decodes")
    }

    #[test]
    fn the_fixture_decoded_twice_is_unchanged() {
        let a = decode(FIXTURE);
        let b = decode(FIXTURE);
        assert_eq!(classify(a.identity(), b.identity()), Equality::Unchanged);
    }

    #[test]
    fn a_contract_compared_with_itself_is_unchanged() {
        let a = decode(FIXTURE);
        assert_eq!(classify(a.identity(), a.identity()), Equality::Unchanged);
    }

    #[test]
    fn a_change_to_the_declared_name_alone_is_still_unchanged() {
        // ID2: `name` is metadata and MUST NOT affect identity.
        let renamed = FIXTURE.replacen(
            r#""name":"Cancel-Correct Replicated Register""#,
            r#""name":"Something else entirely""#,
            1,
        );
        assert_ne!(renamed, FIXTURE, "the replacement must actually fire");
        let a = decode(FIXTURE);
        let b = decode(&renamed);
        assert_ne!(a.name(), b.name());
        assert_eq!(classify(a.identity(), b.identity()), Equality::Unchanged);
    }

    #[test]
    fn a_one_field_semantic_edit_classifies_distinct() {
        // Shrink `bounds.nodes` by one — a genuine, ID2-preimage change.
        let shrunk = FIXTURE.replacen(r#""nodes":3"#, r#""nodes":2"#, 1);
        assert_ne!(shrunk, FIXTURE, "the replacement must actually fire");
        let a = decode(FIXTURE);
        let b = decode(&shrunk);
        assert_eq!(classify(a.identity(), b.identity()), Equality::Distinct);
    }

    #[test]
    fn unchanged_classification_is_some_with_fifteen_unchanged_records_when_identities_agree() {
        let a = decode(FIXTURE);
        let b = decode(FIXTURE);
        let records =
            unchanged_classification(a.identity(), b.identity()).expect("identities are equal");
        assert_eq!(records.len(), 15);
        for field in PolicyField::ALL {
            let matching: Vec<_> = records
                .iter()
                .filter(|record| record.field() == field)
                .collect();
            assert_eq!(matching.len(), 1, "{field} must appear exactly once");
            assert_eq!(matching[0].relation(), Relation::Unchanged);
        }
    }

    #[test]
    fn unchanged_classification_is_none_when_identities_differ() {
        let shrunk = FIXTURE.replacen(r#""nodes":3"#, r#""nodes":2"#, 1);
        let a = decode(FIXTURE);
        let b = decode(&shrunk);
        assert_eq!(classify(a.identity(), b.identity()), Equality::Distinct);
        assert!(unchanged_classification(a.identity(), b.identity()).is_none());
    }

    #[test]
    fn is_unchanged_agrees_with_the_enum_variant() {
        assert!(Equality::Unchanged.is_unchanged());
        assert!(!Equality::Distinct.is_unchanged());
    }
}
