//! The pack's **target**: which Intent Contract, and which question — the first word of
//! PR-11 / IMPL-01 (RFC 0028, "Required fields, reconciled with plan §6.2", the "target
//! intent and property" row).
//!
//! # Which keys the target is, and why not a third
//!
//! > | plan §6.2 bullet | Schema home | Type | Rule |
//! > |---|---|---|---|
//! > | target intent and property | `intent`, `question` | `^in_` handle; string | Both
//! > required. `intent` MUST resolve in the daemon's intent registry (RFC 0037) |
//! >
//! > — RFC 0028, "Required fields, reconciled with plan §6.2"
//!
//! That mapping "is normative, and a bullet with no schema home is a defect in one of the
//! two documents, never a licence to invent a field", so [`Target`] is exactly those two
//! keys and nothing else. In particular it is **not** the IDL's `Target`:
//! `continuumd-native-protocol.idl` declares `struct Target { kind: TargetKind, id: String
//! }` — "what an operation is aimed at", the argument `verification.start`,
//! `verification.await` and `model.check` carry — and that is a *request* shape.
//! `context-pack.schema.json` has no `target` key and declares `additionalProperties:
//! false`, so a `TargetKind`/`id` pair written into a pack would be a document the schema
//! rejects; the pack records what it was compiled *for* by naming the contract and the
//! question, which is what the mapping table says.
//!
//! `snapshot` is not part of the target either, for the same textual reason: RFC 0028
//! lists it among the four required properties "plan §6.2 does not name", sourced to plan
//! §4.2 ("the sealed snapshot the pack is stated against") rather than to this bullet.
//!
//! # The intent handle is class-checked, not a string
//!
//! The schema types `intent` as `^in_[A-Za-z0-9_-]+$`, which is plan §4.4's registered
//! prefix for an Intent Contract. [`Target::new`] therefore takes a real
//! [`ArtifactHandle`] and refuses any class but
//! [`ArtifactClass::IntentContract`](continuum_workspace::artifact_path::ArtifactClass::IntentContract)
//! — the discipline [`crate::model::ModelActionRef::with_model`] set for `model_*`, so
//! that a handle of the wrong class is a refusal at construction rather than a pack that
//! validates against a pattern while naming the wrong artifact.
//!
//! **Unlike IMPL-03's `model_*`, this handle has a producer in-tree today.**
//! `continuum_intent::contract::IntentContract::mint_intent_id` derives `in_<digest>` from
//! a contract's canonical encoding (RFC 0037 ID1), and
//! `continuum_intent::contract::IntentId` is that crate's newtype over the same plan §4.4
//! grammar. This module does not add a third copy of the grammar — the same reasoning
//! `crate::model` gave for reusing `continuum_value::value::Name` — it takes the
//! workspace's handle type, and `an_intent_id_drops_straight_into_the_slot` in the
//! evidence suite pins that the two spellings agree. What is declined is *resolution*:
//! "`intent` MUST resolve in the daemon's intent registry" is a statement about a registry
//! this crate has no edge to, so a well-formed handle of the right class is accepted and
//! whether it resolves is `continuumd`'s check, not this one's.
//!
//! # The question has exactly two forms, and both are typed
//!
//! > `question` MUST be the canonical rendering of the compiled query: for
//! > `context.compile`, the caller's question; for `context.expand`, the canonical
//! > encoding of the triple (relation, anchor, depth).
//! >
//! > — RFC 0028, "Pack identity, lineage, and determinism"
//!
//! [`Question`] has exactly those two constructors and no third:
//! [`Question::compiled`] for the caller's question and [`Question::of_expansion`] for an
//! already-built [`ExpansionQuestion`](crate::expansion::ExpansionQuestion) — the canonical
//! triple encoding IMPL-04 landed. A `String` field with a free setter would admit a third
//! form the RFC does not name.
//!
//! **The spelling restriction, and what it costs.** "The question is part of the
//! identity", and under ADR-0013 "two packs are the same pack iff their canonical
//! encodings are byte-equal". [`Question::compiled`] therefore restricts the text to
//! non-empty, trimmed, printable ASCII with spaces — the restriction
//! `continuum_value::value::Name` and `continuum_value::assurance::UnsupportedReason` state
//! for identity-bearing strings ("Unicode that normalizes several ways would let two
//! spellings denote one name — and therefore two encodings denote one value"), widened by
//! exactly the space character because a question is a sentence and a name is not. It is
//! narrower than the schema's unconstrained `string`, which is the strengthening
//! [`crate::source::SourceSpan`]'s `file` already took over the IDL's bare `String` and
//! `crate::omission`'s `kind` took over the schema's free string. The cost is real and is
//! recorded rather than hidden: a question written in a non-Latin script has no spelling
//! here today, because this crate carries no Unicode normalizer and inventing one is not
//! this bullet's work — when one lands, this is the one constructor that widens.
//!
//! # INV-016: the question is transcribed, never summarized
//!
//! > Source is untrusted data and MUST NOT be interpolated into any description
//! > (INV-016).
//! >
//! > — RFC 0028
//!
//! A [`Question`] is the one caller-authored string this bullet carries, so the structural
//! question is whether it can reach a `selected[]` item's `summary`. It cannot:
//! [`crate::selection::SelectedItem::new`] is `pub(crate)`, this module never calls it, and
//! the only two callers that exist — `SourceRef::into_selected_item` and
//! `ModelActionRef::into_selected_item` — take no string parameter (see
//! `crate::selection`'s "Why a `SelectedItem` cannot carry caller-supplied prose"). The
//! `compile_fail` doctest below pins that route closed from outside the crate, and the
//! doctest above it is its anti-vacuity companion: everything except the fabrication
//! compiles.
//!
//! A question is also never interpolated into an *error*: [`QuestionError`] names the
//! offending byte offset and character, never the rejected text, so a refusal cannot
//! become the echo INV-016 forbids.
//!
//! # Clause → test
//!
//! | Clause | Source | Test |
//! |---|---|---|
//! | the target is `intent` + `question`, those two keys and no others | RFC 0028's mapping table | `a_target_contributes_exactly_the_two_schema_keys` |
//! | `intent` is a class-checked `in_*` handle | `context-pack.schema.json`; plan §4.4 | `an_intent_handle_of_the_wrong_class_is_refused`, anti-vacuity `an_intent_contract_handle_is_accepted` |
//! | the question is non-empty, trimmed, printable ASCII | ADR-0013 identity; `Name`'s own reading | `an_empty_or_untrimmed_or_control_question_is_refused`, anti-vacuity `a_plain_question_is_accepted` |
//! | an expansion's question is the canonical triple, not prose | RFC 0028, "Pack identity, lineage, and determinism" | `an_expansion_question_is_the_canonical_triple` |
//! | the canonical fragment is ID5-sorted and reuses the one writer | RFC 0037 ID5 | `the_fragment_is_canonical_and_sorted` |
//! | two independent builds of one target are byte-identical | `rule ordering.deterministic`, docs/19 §7 | `two_builds_of_one_target_are_byte_identical` |
//! | a question cannot reach a `selected[].summary` | INV-016 | the `compile_fail` doctest below |
//!
//! A [`Question`] is built and read like this:
//!
//! ```
//! use continuum_context::target::Question;
//!
//! let question = Question::compiled("why did AckImpliesDurable fail?").unwrap();
//! assert_eq!(question.as_str(), "why did AckImpliesDurable fail?");
//! ```
//!
//! ```compile_fail
//! // …but it cannot become a `selected[]` item's summary: `SelectedItem::new` is
//! // `pub(crate)`, so no caller-authored string reaches a `summary` from outside.
//! use continuum_context::selection::{SelectedItem, SelectionKind};
//! use continuum_context::target::Question;
//! use continuum_value::value::Name;
//!
//! let question = Question::compiled("why did AckImpliesDurable fail?").unwrap();
//! let _ = SelectedItem::new(
//!     Name::new("e_1").unwrap(),
//!     SelectionKind::Unknown,
//!     question.as_str().to_owned(),
//!     None,
//! );
//! ```

use core::fmt;

use continuum_intent::canonical_json::Json;
use continuum_workspace::artifact_path::{ArtifactClass, ArtifactHandle};

use crate::expansion::ExpansionQuestion;

/// The pack's `question`: the canonical rendering of the compiled query.
///
/// Exactly two forms exist, because RFC 0028 names exactly two — see the module
/// documentation.
///
/// `Ord` is code-point order on the canonical text, which agrees with structural equality
/// and with the canonical encoding, so a list keyed by question sorts one way in every
/// process (`rule ordering.deterministic`).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Question(String);

impl Question {
    /// The caller's question for `context.compile`.
    ///
    /// # Errors
    ///
    /// [`QuestionError::Empty`] for the empty string, [`QuestionError::Untrimmed`] when
    /// the text has leading or trailing whitespace, and [`QuestionError::NonCanonical`]
    /// for the first character outside printable ASCII (the space character excepted) —
    /// see the module documentation for why an identity-bearing string has one spelling.
    pub fn compiled(text: &str) -> Result<Self, QuestionError> {
        if text.is_empty() {
            return Err(QuestionError::Empty);
        }
        if text.trim() != text {
            return Err(QuestionError::Untrimmed);
        }
        if let Some((index, character)) = text
            .char_indices()
            .find(|(_, c)| !c.is_ascii_graphic() && *c != ' ')
        {
            return Err(QuestionError::NonCanonical { index, character });
        }
        Ok(Self(text.to_owned()))
    }

    /// The question an expansion asks: the canonical encoding of `(relation, anchor,
    /// depth)`.
    ///
    /// Infallible, and not by luck: an [`ExpansionQuestion`] is an ID5 canonical-JSON
    /// object over a closed relation token, a `continuum_value::value::Name` anchor
    /// (non-empty printable ASCII) and an integer depth, so it is always non-empty,
    /// trimmed, and printable ASCII.
    ///
    /// # Panics
    ///
    /// Never, for the reason above.
    #[must_use]
    pub fn of_expansion(question: &ExpansionQuestion) -> Self {
        Self::compiled(question.as_str())
            .expect("an expansion question is canonical JSON over printable-ASCII terms")
    }

    /// The canonical text, exactly as the pack's `question` field carries it.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Question {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Why a [`Question`] was refused.
///
/// No variant carries the rejected text: a refusal names a position, never an echo
/// (INV-016).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuestionError {
    /// The text was empty. A pack whose question is blank has no identity to be part of.
    Empty,
    /// The text had leading or trailing whitespace, so one question would have two
    /// spellings and therefore two pack identities.
    Untrimmed,
    /// The text contained a character outside printable ASCII and the space.
    NonCanonical {
        /// Byte offset of the first offending character.
        index: usize,
        /// The first offending character.
        character: char,
    },
}

impl fmt::Display for QuestionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => f.write_str("question is empty"),
            Self::Untrimmed => f.write_str("question has leading or trailing whitespace"),
            Self::NonCanonical { index, character } => write!(
                f,
                "question has a non-canonical character {character:?} at byte {index}"
            ),
        }
    }
}

impl core::error::Error for QuestionError {}

/// What a pack was compiled for: the Intent Contract it is stated against, and the
/// question it answers.
///
/// Contributes the schema's `intent` and `question` keys, and no others.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Target {
    intent: ArtifactHandle,
    question: Question,
}

impl Target {
    /// The two pack keys a target contributes, in the schema's spelling.
    pub const KEYS: [&'static str; 2] = ["intent", "question"];

    /// Name the contract and the question.
    ///
    /// # Errors
    ///
    /// [`TargetError::WrongArtifactClass`] when `intent`'s class is not
    /// [`ArtifactClass::IntentContract`] — the schema's `^in_` pattern, plan §4.4's
    /// registered prefix.
    pub fn new(intent: ArtifactHandle, question: Question) -> Result<Self, TargetError> {
        if intent.class() != ArtifactClass::IntentContract {
            return Err(TargetError::WrongArtifactClass(intent.class()));
        }
        Ok(Self { intent, question })
    }

    /// The Intent Contract this pack is stated against.
    #[must_use]
    pub const fn intent(&self) -> &ArtifactHandle {
        &self.intent
    }

    /// The question this pack answers.
    #[must_use]
    pub const fn question(&self) -> &Question {
        &self.question
    }

    /// The `(key, value)` pairs this target contributes to a pack document.
    ///
    /// A *fragment* of a pack, never a pack: assembling one is the compiler's, and no
    /// single IMPL bullet's (see [`crate::selection`]'s module documentation).
    #[must_use]
    pub fn to_json_fields(&self) -> Vec<(String, Json)> {
        vec![
            ("intent".to_owned(), Json::String(self.intent.to_string())),
            (
                "question".to_owned(),
                Json::String(self.question.as_str().to_owned()),
            ),
        ]
    }

    /// The canonical ID5 bytes of [`to_json_fields`](Self::to_json_fields) as one object.
    ///
    /// # Panics
    ///
    /// Never: [`Self::KEYS`] are pairwise distinct string literals.
    #[must_use]
    pub fn to_canonical_bytes(&self) -> Vec<u8> {
        Json::object(self.to_json_fields())
            .expect("the target's two keys are distinct string literals")
            .to_canonical_bytes()
    }
}

/// Why a [`Target`] was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetError {
    /// The supplied handle's class was not [`ArtifactClass::IntentContract`].
    WrongArtifactClass(ArtifactClass),
}

impl fmt::Display for TargetError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongArtifactClass(class) => write!(
                f,
                "a pack's target intent must be class `{}` (the schema's `^in_` pattern), not `{class}`",
                ArtifactClass::IntentContract
            ),
        }
    }
}

impl core::error::Error for TargetError {}

#[cfg(test)]
mod tests {
    use continuum_value::value::Name;

    use super::*;
    use crate::expansion::{Depth, ExpansionQuery, ExpansionRelation};

    fn intent(identity: &str) -> ArtifactHandle {
        ArtifactHandle::new(ArtifactClass::IntentContract, identity).expect("well-formed handle")
    }

    fn question(text: &str) -> Question {
        Question::compiled(text).expect("well-formed test question")
    }

    #[test]
    fn a_plain_question_is_accepted() {
        // Anti-vacuity companion to the refusals below: the validator must admit an
        // ordinary sentence, not reject everything.
        let asked = question("why did AckImpliesDurable fail?");
        assert_eq!(asked.as_str(), "why did AckImpliesDurable fail?");
        assert_eq!(asked.to_string(), "why did AckImpliesDurable fail?");
        assert!(Question::compiled("a").is_ok());
        assert!(Question::compiled("does Fill(big) reach 4?").is_ok());
    }

    #[test]
    fn an_empty_or_untrimmed_or_control_question_is_refused() {
        assert_eq!(Question::compiled(""), Err(QuestionError::Empty));
        assert_eq!(Question::compiled(" why?"), Err(QuestionError::Untrimmed));
        assert_eq!(Question::compiled("why? "), Err(QuestionError::Untrimmed));
        assert_eq!(
            Question::compiled("why\nnow?"),
            Err(QuestionError::NonCanonical {
                index: 3,
                character: '\n'
            })
        );
        // Non-ASCII is refused for the identity reason stated in the module
        // documentation, and the refusal names a position rather than the text.
        assert!(matches!(
            Question::compiled("pourquoi échoue-t-il ?"),
            Err(QuestionError::NonCanonical { .. })
        ));
    }

    #[test]
    fn an_expansion_question_is_the_canonical_triple() {
        let query = ExpansionQuery::new(
            ExpansionRelation::SourceSpan,
            Name::new("e_1").expect("well-formed"),
        );
        let expansion = ExpansionQuestion::of(&query, Depth::DEFAULT);
        let asked = Question::of_expansion(&expansion);
        assert_eq!(
            asked.as_str(),
            r#"{"anchor":"e_1","depth":1,"relation":"source_span"}"#
        );
        // The two forms are the same type and are therefore comparable, which is what
        // makes `question` one field rather than two.
        assert_ne!(asked, question("why did AckImpliesDurable fail?"));
    }

    #[test]
    fn an_intent_contract_handle_is_accepted() {
        let handle = intent("ack_v1");
        let target = Target::new(handle.clone(), question("why?")).expect("in_* is accepted");
        assert_eq!(target.intent(), &handle);
        assert_eq!(target.question().as_str(), "why?");
    }

    #[test]
    fn an_intent_handle_of_the_wrong_class_is_refused() {
        // Anti-vacuity companion to the accept case: the class check must discriminate.
        for class in [
            ArtifactClass::WorkspaceSnapshot,
            ArtifactClass::SignedIntentBundle,
            ArtifactClass::ContextPack,
            ArtifactClass::Crashpack,
        ] {
            let wrong = ArtifactHandle::new(class, "x1").expect("well-formed");
            assert_eq!(
                Target::new(wrong, question("why?")),
                Err(TargetError::WrongArtifactClass(class)),
                "{class} must be refused"
            );
        }
    }

    #[test]
    fn a_target_contributes_exactly_the_two_schema_keys() {
        let target = Target::new(intent("ack_v1"), question("why?")).expect("accepted");
        let keys: Vec<String> = target
            .to_json_fields()
            .into_iter()
            .map(|(key, _)| key)
            .collect();
        assert_eq!(keys, Target::KEYS.to_vec());
    }

    #[test]
    fn the_fragment_is_canonical_and_sorted() {
        let target = Target::new(intent("ack_v1"), question("why?")).expect("accepted");
        let text = String::from_utf8(target.to_canonical_bytes()).expect("utf8");
        assert_eq!(text, r#"{"intent":"in_ack_v1","question":"why?"}"#);
    }

    #[test]
    fn two_builds_of_one_target_are_byte_identical() {
        let build =
            || Target::new(intent("ack_v1"), question("why did it fail?")).expect("accepted");
        assert_eq!(build().to_canonical_bytes(), build().to_canonical_bytes());

        // And not vacuously: a different contract, or a different question, is different
        // bytes.
        let other_intent =
            Target::new(intent("ack_v2"), question("why did it fail?")).expect("accepted");
        let other_question =
            Target::new(intent("ack_v1"), question("why did it stall?")).expect("accepted");
        assert_ne!(
            build().to_canonical_bytes(),
            other_intent.to_canonical_bytes()
        );
        assert_ne!(
            build().to_canonical_bytes(),
            other_question.to_canonical_bytes()
        );
    }

    #[test]
    fn error_types_render_and_implement_error() {
        fn rendered<E: core::error::Error>(error: &E) -> String {
            error.to_string()
        }
        assert!(!rendered(&QuestionError::Empty).is_empty());
        assert!(!rendered(&QuestionError::Untrimmed).is_empty());
        assert!(
            !rendered(&QuestionError::NonCanonical {
                index: 0,
                character: '\t'
            })
            .is_empty()
        );
        let refusal = rendered(&TargetError::WrongArtifactClass(ArtifactClass::Crashpack));
        assert!(refusal.contains("in"));
        assert!(refusal.contains("crash"));
    }
}
