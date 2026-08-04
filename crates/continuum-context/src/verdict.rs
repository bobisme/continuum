//! The pack's **verdict**, and the INV-008 reason it can never be missing — the second
//! word of PR-11 / IMPL-01 (RFC 0028, "Required fields, reconciled with plan §6.2", the
//! "exact verdict and assurance envelope" row; RFC 0028, "Verdict and assurance").
//!
//! # The closed four
//!
//! > `verdict` is one of `satisfied`, `refuted`, `deadlock`, `inconclusive` — the IDL's
//! > `EvaluationVerdict`, identical to `assurance-result.schema.json`. It names the verdict
//! > of the evaluation the pack compiles evidence *for*. Unsupported semantics and engine
//! > errors are `inconclusive` with reason `Unsupported` or `EngineError`; they are never
//! > verdicts.
//! >
//! > — RFC 0028, "Verdict and assurance"
//!
//! The set is closed (RFC 0028, "Versioning and revision"), so [`Verdict`] carries all four
//! and [`Verdict::from_wire`] fails closed on anything else: "a consumer that reads a
//! guarantee, selection kind, omission reason, inconclusive reason, or expansion relation
//! it does not recognize MUST reject the pack with `MalformedRequest` […] Forward
//! compatibility is achieved by rejecting, never by ignoring."
//!
//! # INV-008 is unrepresentable here, not validated
//!
//! The schema states the pairing as a conditional:
//!
//! ```json
//! { "if":   { "properties": { "verdict": { "const": "inconclusive" } },
//!             "required": ["verdict"] },
//!   "then": { "required": ["inconclusive_reason"] } }
//! ```
//!
//! — `context-pack.schema.json`, `allOf[0]`, described as "INV-008: inconclusive is never
//! untyped (plan §11.4)".
//!
//! A conditional is a thing a validator checks *after* a document exists. Here it is a
//! thing no document can fail: [`Verdict::Inconclusive`] **carries** its
//! [`InconclusiveReason`], so an untyped `inconclusive` has no spelling — the discipline
//! `crate::omission::Retrievability` used for the schema's two conditionals on
//! `expandable`. [`Verdict::to_json_fields`] emits `inconclusive_reason` exactly when the
//! verdict is that variant, so the conditional holds by construction on the way out as well
//! as on the way in.
//!
//! The six reasons are `continuum_value::assurance::InconclusiveReason`, already this
//! workspace's typed spelling of INV-008 ("Both normative schemas […] spell it as a JSON
//! `enum` with exactly these six members and no extension point"), rather than a fourth
//! copy of the vocabulary — the reuse discipline `crate::model` states for
//! `continuum_value::value::Name`.
//!
//! # One narrowing, stated rather than smuggled
//!
//! The schema *permits* `inconclusive_reason` beside a decided verdict: it is a declared
//! property, and the `allOf` conditional only ever adds a requirement. [`Verdict`] cannot
//! spell that, and [`Verdict::from_wire`] refuses it
//! ([`VerdictError::ReasonWithoutInconclusive`]). The reading: a reason is "why a claim
//! could not be decided", so a reason attached to `satisfied` is a document with two
//! readings — the verdict is wrong, or the reason is spurious — and RFC 0028's rule for a
//! token that could mean two things is to reject. This is the same kind of strengthening
//! `crate::source::SourceSpan` took over the IDL's bare `String` file and `crate::omission`
//! took over the schema's free-string `kind`, and like those it is narrower than the schema
//! rather than wider: nothing this type produces is non-conforming, and the instances it
//! declines to read are ones no producer in this workspace writes. A closed
//! `if verdict != inconclusive then not required inconclusive_reason` at that key belongs in
//! the schema sweep; this bone changes no schema.
//!
//! # A verdict carries no artifact, and no order
//!
//! `verdict` is a bare enum string in the schema — there is no handle-shaped slot on it and
//! this type has no field for one, which is the per-kind artifact discipline for this key
//! (`crate::target` carries the one handle this bullet touches; `crate::assurance` carries
//! none either).
//!
//! [`Verdict`] deliberately derives no [`Ord`]. The four are alternatives, not degrees:
//! `refuted` is not "less than" `satisfied`, and `continuum_value::assurance` states the
//! same refusal for the vocabularies the dossier declines to rank. Nothing sorts verdicts,
//! because `verdict` is a scalar key and not one of RFC 0028's six list-valued fields.
//!
//! # Clause → test
//!
//! | Clause | Source | Test |
//! |---|---|---|
//! | the four tokens are the schema's, in the schema's order | `context-pack.schema.json` `verdict.enum` | `the_verdict_set_is_exactly_the_schema_enum` |
//! | every verdict round-trips through its wire token | RFC 0028 | `every_verdict_round_trips_through_its_wire_form` |
//! | an unrecognized token is refused | RFC 0028, "Versioning and revision" | `an_unknown_verdict_token_does_not_parse` |
//! | `inconclusive` without a reason is refused | INV-008; the schema's `allOf[0]` | `an_untyped_inconclusive_is_refused` |
//! | a reason beside a decided verdict is refused | this module's stated reading | `a_reason_without_inconclusive_is_refused` |
//! | `inconclusive_reason` is emitted exactly when inconclusive | the schema's `allOf[0]` | `the_reason_key_appears_exactly_when_inconclusive` |
//! | a verdict never carries an artifact handle | `context-pack.schema.json` (`verdict` is a string enum) | `a_verdict_fragment_is_a_bare_token` |
//!
//! An untyped `inconclusive` does not compile:
//!
//! ```compile_fail
//! use continuum_context::verdict::Verdict;
//!
//! // `Inconclusive` is not a unit variant: INV-008's reason is a field, not a
//! // convention, so there is no value of this type that is inconclusive about nothing.
//! let verdict: Verdict = Verdict::Inconclusive;
//! ```
//!
//! ```
//! use continuum_context::verdict::Verdict;
//! use continuum_value::assurance::InconclusiveReason;
//!
//! // The same line with the reason supplied is the only spelling there is.
//! let verdict = Verdict::Inconclusive(InconclusiveReason::IncompleteProofSearch);
//! assert_eq!(verdict.as_wire_str(), "inconclusive");
//! ```

use core::fmt;

use continuum_intent::canonical_json::Json;
use continuum_value::assurance::InconclusiveReason;

/// The evaluation outcome a pack compiles evidence for.
///
/// The four members of `context-pack.schema.json`'s closed `verdict` enum, in the schema's
/// declaration order — the IDL's `EvaluationVerdict`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Verdict {
    /// `satisfied` — the property held over everything the envelope covers.
    Satisfied,
    /// `refuted` — a counterexample exists.
    Refuted,
    /// `deadlock` — the evaluation reached a state with no successor.
    Deadlock,
    /// `inconclusive` — no verdict, with the INV-008 reason it is missing. "Unsupported
    /// semantics and engine errors are `inconclusive` with reason `Unsupported` or
    /// `EngineError`; they are never verdicts" (RFC 0028).
    Inconclusive(InconclusiveReason),
}

impl Verdict {
    /// The four wire tokens, in the schema's `verdict.enum` order.
    pub const TOKENS: [&'static str; 4] = ["satisfied", "refuted", "deadlock", "inconclusive"];

    /// The keys a verdict contributes: always `verdict`, and `inconclusive_reason` exactly
    /// when the verdict is [`Verdict::Inconclusive`].
    pub const KEYS: [&'static str; 2] = ["verdict", "inconclusive_reason"];

    /// The wire token, byte-identical to the schema's `verdict` enum.
    #[must_use]
    pub const fn as_wire_str(self) -> &'static str {
        match self {
            Self::Satisfied => "satisfied",
            Self::Refuted => "refuted",
            Self::Deadlock => "deadlock",
            Self::Inconclusive(_) => "inconclusive",
        }
    }

    /// The INV-008 reason, present exactly when the verdict is inconclusive.
    #[must_use]
    pub const fn inconclusive_reason(self) -> Option<InconclusiveReason> {
        match self {
            Self::Satisfied | Self::Refuted | Self::Deadlock => None,
            Self::Inconclusive(reason) => Some(reason),
        }
    }

    /// Recover a verdict from a pack's `verdict` token and its `inconclusive_reason`.
    ///
    /// The two are read together because the schema pairs them; reading them apart is what
    /// makes an untyped `inconclusive` expressible in the first place.
    ///
    /// # Errors
    ///
    /// - [`VerdictError::UnknownVerdict`] — the token is not one of [`Self::TOKENS`]. It
    ///   is not echoed back: a refusal names the failure, never the caller's bytes.
    /// - [`VerdictError::UntypedInconclusive`] — `inconclusive` with no reason (INV-008).
    /// - [`VerdictError::ReasonWithoutInconclusive`] — a reason beside a decided verdict;
    ///   see the module documentation's "One narrowing".
    pub fn from_wire(
        verdict: &str,
        inconclusive_reason: Option<InconclusiveReason>,
    ) -> Result<Self, VerdictError> {
        let decided = match verdict {
            "satisfied" => Self::Satisfied,
            "refuted" => Self::Refuted,
            "deadlock" => Self::Deadlock,
            "inconclusive" => {
                return inconclusive_reason
                    .map(Self::Inconclusive)
                    .ok_or(VerdictError::UntypedInconclusive);
            }
            _ => return Err(VerdictError::UnknownVerdict),
        };
        if inconclusive_reason.is_some() {
            return Err(VerdictError::ReasonWithoutInconclusive {
                verdict: decided.as_wire_str(),
            });
        }
        Ok(decided)
    }

    /// The `(key, value)` pairs this verdict contributes to a pack document.
    ///
    /// One pair for a decided verdict, two for an inconclusive one — which is the schema's
    /// INV-008 conditional, discharged by the shape of the value rather than by a check.
    ///
    /// A *fragment* of a pack, never a pack.
    #[must_use]
    pub fn to_json_fields(self) -> Vec<(String, Json)> {
        let mut fields = vec![(
            "verdict".to_owned(),
            Json::String(self.as_wire_str().to_owned()),
        )];
        if let Some(reason) = self.inconclusive_reason() {
            fields.push((
                "inconclusive_reason".to_owned(),
                Json::String(reason.as_str().to_owned()),
            ));
        }
        fields
    }

    /// The canonical ID5 bytes of [`to_json_fields`](Self::to_json_fields) as one object.
    ///
    /// # Panics
    ///
    /// Never: [`Self::KEYS`] are pairwise distinct string literals.
    #[must_use]
    pub fn to_canonical_bytes(self) -> Vec<u8> {
        Json::object(self.to_json_fields())
            .expect("the verdict's two keys are distinct string literals")
            .to_canonical_bytes()
    }
}

impl fmt::Display for Verdict {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_wire_str())
    }
}

/// Why a pack's verdict was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VerdictError {
    /// The token is not a member of the closed four-member set.
    UnknownVerdict,
    /// `inconclusive` with no typed reason — the INV-008 violation the schema's `allOf[0]`
    /// names and this type cannot represent.
    UntypedInconclusive,
    /// A typed reason beside a decided verdict, which the schema permits and this module
    /// refuses.
    ReasonWithoutInconclusive {
        /// The decided verdict the reason was attached to.
        verdict: &'static str,
    },
}

impl fmt::Display for VerdictError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownVerdict => write!(
                f,
                "verdict is not one of {:?} (closed enum; fail closed)",
                Verdict::TOKENS
            ),
            Self::UntypedInconclusive => {
                f.write_str("verdict is `inconclusive` with no typed reason (INV-008)")
            }
            Self::ReasonWithoutInconclusive { verdict } => write!(
                f,
                "verdict `{verdict}` is decided, so it carries no `inconclusive_reason`"
            ),
        }
    }
}

impl core::error::Error for VerdictError {}

#[cfg(test)]
mod tests {
    use super::*;

    /// Transcribed from `notes/plan/schemas/context-pack.schema.json`,
    /// `properties.verdict.enum`, in file order.
    const SCHEMA_VERDICT_ENUM: [&str; 4] = ["satisfied", "refuted", "deadlock", "inconclusive"];

    fn every_verdict() -> Vec<Verdict> {
        let mut all = vec![Verdict::Satisfied, Verdict::Refuted, Verdict::Deadlock];
        all.extend(InconclusiveReason::ALL.map(Verdict::Inconclusive));
        all
    }

    #[test]
    fn the_verdict_set_is_exactly_the_schema_enum() {
        assert_eq!(Verdict::TOKENS, SCHEMA_VERDICT_ENUM);
        assert_eq!(
            Verdict::Inconclusive(InconclusiveReason::EngineError).as_wire_str(),
            "inconclusive"
        );
        assert_eq!(Verdict::Refuted.to_string(), "refuted");
    }

    #[test]
    fn every_verdict_round_trips_through_its_wire_form() {
        for verdict in every_verdict() {
            let recovered =
                Verdict::from_wire(verdict.as_wire_str(), verdict.inconclusive_reason());
            assert_eq!(recovered, Ok(verdict), "{verdict} must round-trip");
        }
        // Anti-vacuity: the six reasons are distinguished, not collapsed into one
        // `inconclusive`.
        for reason in InconclusiveReason::ALL {
            assert_eq!(
                Verdict::from_wire("inconclusive", Some(reason)),
                Ok(Verdict::Inconclusive(reason))
            );
        }
    }

    #[test]
    fn an_unknown_verdict_token_does_not_parse() {
        for bad in [
            "",
            "Satisfied",
            "satisfied ",
            "established",
            "SATISFIED",
            "unsupported",
            "budget-exhausted",
        ] {
            assert_eq!(
                Verdict::from_wire(bad, None),
                Err(VerdictError::UnknownVerdict),
                "{bad:?} must be refused"
            );
        }
    }

    #[test]
    fn an_untyped_inconclusive_is_refused() {
        // INV-008, and the schema's own `allOf[0]`.
        assert_eq!(
            Verdict::from_wire("inconclusive", None),
            Err(VerdictError::UntypedInconclusive)
        );
    }

    #[test]
    fn a_reason_without_inconclusive_is_refused() {
        for decided in [Verdict::Satisfied, Verdict::Refuted, Verdict::Deadlock] {
            assert_eq!(
                Verdict::from_wire(decided.as_wire_str(), Some(InconclusiveReason::EngineError)),
                Err(VerdictError::ReasonWithoutInconclusive {
                    verdict: decided.as_wire_str()
                }),
                "{decided} carries no reason"
            );
        }
    }

    #[test]
    fn the_reason_key_appears_exactly_when_inconclusive() {
        for verdict in every_verdict() {
            let keys: Vec<String> = verdict
                .to_json_fields()
                .into_iter()
                .map(|(key, _)| key)
                .collect();
            match verdict.inconclusive_reason() {
                Some(_) => assert_eq!(keys, vec!["verdict", "inconclusive_reason"]),
                None => assert_eq!(keys, vec!["verdict"]),
            }
        }
    }

    #[test]
    fn a_verdict_fragment_is_a_bare_token() {
        assert_eq!(
            String::from_utf8(Verdict::Refuted.to_canonical_bytes()).expect("utf8"),
            r#"{"verdict":"refuted"}"#
        );
        assert_eq!(
            String::from_utf8(
                Verdict::Inconclusive(InconclusiveReason::IncompleteProofSearch)
                    .to_canonical_bytes()
            )
            .expect("utf8"),
            r#"{"inconclusive_reason":"IncompleteProofSearch","verdict":"inconclusive"}"#
        );
    }

    #[test]
    fn distinct_verdicts_never_share_an_encoding() {
        let all = every_verdict();
        for (index, verdict) in all.iter().enumerate() {
            for other in &all[index + 1..] {
                assert_ne!(
                    verdict.to_canonical_bytes(),
                    other.to_canonical_bytes(),
                    "{verdict} and {other} must differ"
                );
            }
        }
    }

    #[test]
    fn error_types_render_and_implement_error() {
        fn rendered<E: core::error::Error>(error: &E) -> String {
            error.to_string()
        }
        assert!(rendered(&VerdictError::UnknownVerdict).contains("satisfied"));
        assert!(rendered(&VerdictError::UntypedInconclusive).contains("INV-008"));
        assert!(
            rendered(&VerdictError::ReasonWithoutInconclusive {
                verdict: "satisfied"
            })
            .contains("satisfied")
        );
    }
}
