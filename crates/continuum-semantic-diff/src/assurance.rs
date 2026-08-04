//! `assurance` field classification: RFC 0031's "Assurance movement" (PR-12 /
//! IMPL-06).
//!
//! # What this module answers
//!
//! Given two [`AssurancePolicy`]s — an Intent Contract's `assurance` before and
//! after a proposed revision — classify the requirement triple's movement into RFC
//! 0031's closed relation vocabulary:
//!
//! > The `assurance` field is the one totally ordered field... The classified unit is
//! > the requirement triple (`minimum`, `independent_checker`, `clean_recompute`) —
//! > `AssuranceRequirement`. The relation is `AssuranceChange`, whose three members
//! > are `unchanged`, `upgraded`, `downgraded`: the schema's sixteen-relation set
//! > restricted to this field.
//! >
//! > A change **weakens** iff `minimum` falls, or `independent_checker` goes
//! > true→false, or `clean_recompute` goes true→false. A change **strengthens** iff
//! > `minimum` rises, or either flag goes false→true.
//! >
//! > **Fail closed in both directions.** A change that both weakens and strengthens
//! > MUST classify `downgraded`. A net direction MUST NOT be computed. This is what
//! > stops a contract from dropping its independent checker in exchange for a
//! > nominally higher `minimum` and passing `no-downgrade`...
//! >
//! > `incomparable` MUST NOT be emitted for `assurance`. The field is totally
//! > ordered, unlike `bounds`; the only three outcomes are the three above.
//! >
//! > A change in the *declaredness* of `independent_checker` or `clean_recompute` —
//! > declared on one side of a revision and undeclared on the other — has no member
//! > among `unchanged`, `upgraded`, `downgraded`... Normative: such a change
//! > classifies `assurance` as `unknown` and fails closed — the companion of RFC
//! > 0037 correction 14, and the same answer `bounds` gives a declaredness change on
//! > `nodes`/`faults`.
//! >
//! > — `notes/plan/rfcs/0031-semantic-and-intent-diff.md`, "Assurance movement"
//!
//! restated from the contract-shape side by RFC 0037, in the same words `bounds`'
//! declaredness rule uses:
//!
//! > An assurance checker flag — `assurance.independent_checker` or
//! > `assurance.clean_recompute` — has no null form and no default, exactly as
//! > `bounds`' `nodes` and `faults` do not: a change in the *declaredness* of either
//! > (declared on one side of a revision, undeclared on the other) MUST classify
//! > `assurance` as `unknown` and fail closed. RFC 0031's assurance relation has
//! > exactly three members (`unchanged`, `upgraded`, `downgraded`; "Assurance
//! > movement") and none of them names a declaredness change, so there is no
//! > direction to report, and reading the undeclared side as `false` would invent a
//! > value the schema left optional.
//! >
//! > — `notes/plan/rfcs/0037-intent-contract.md`, "Absence, null, and defaults"
//! > (correction 14 states the same rule as a correction, citing this same text)
//!
//! and the field's own gaming-move row:
//!
//! > | Gaming move (plan §5.1) | Field | Relation | Blocking verb (plan §5.4) |
//! > |---|---|---|---|
//! > | lower assurance from exhaustive to sampled | `assurance` | `downgraded` |
//! > `no-downgrade` |
//! >
//! > — RFC 0031, "Completeness guarantee"
//!
//! # Closed over `continuum-intent`, not recomputed
//!
//! `continuum_intent::assurance_policy::AssurancePolicy::change_to` already
//! implements every rule quoted above — the total order, the "fail closed in both
//! directions" mixed-movement clause (`continuum_value::assurance::
//! AssuranceRequirement::change_to`, which it delegates to), and the declaredness
//! refusal (`AssuranceComparisonError::CheckerDeclarednessChanged`). [`classify_assurance`]
//! is a total, checked *projection* of that result onto
//! `continuum_intent::change_policy::Relation` — this crate's one classification
//! vocabulary — and duplicates none of the decision logic, exactly the discipline
//! [`crate::bounds`]'s sibling module (`bn-ycn6`) documents for `Bounds::relation_to`.
//! Nothing in this module decides whether a change weakens, strengthens, or is
//! undecidable; that question is `continuum-intent`'s and is answered once.
//!
//! `accepted_evidence_classes` membership is `AssurancePolicy::evidence_class_changes`'s
//! job, already landed and already returning `(EvidenceClass, Relation)` pairs ready
//! for `ClassificationRecord::new(PolicyField::Assurance, relation)` — there is
//! nothing left in RFC 0031's `assurance` rule for this crate to add for that half of
//! the field, so this module does not wrap it a second time; a caller assembling a
//! full `assurance` classification calls both functions and folds their records
//! together (see `tests/pr12_impl06_fault_fairness_assurance_evidence.rs`).
//!
//! # Bridging two closed vocabularies without a second dependency
//!
//! `AssurancePolicy::change_to` returns `continuum_value::assurance::AssuranceChange`
//! (`Unchanged`/`Upgraded`/`Downgraded`) on success — a *different* type from this
//! module's own [`AssuranceChange`], despite the shared name (this module is named
//! `assurance`, so `assurance::AssuranceChange` reads the same way
//! `crate::bounds::BoundsChange` does for its field; the collision is with an
//! upstream type this module never imports, not with itself). This crate's
//! documented *production* dependency contract is "one declared edge,
//! `continuum-intent`, ... downward" (`lib.rs`) — `tools/check_crate_boundaries.py`
//! enforces `[dependencies]`/`[build-dependencies]` and only *reports*
//! `[dev-dependencies]`, so that contract is specifically about what this crate's
//! `src/` may import. Pulling `continuum-value` into `[dependencies]` to name the
//! upstream `AssuranceChange` and match on its three variants would widen that
//! contract for a type [`classify_assurance`] only ever needs to *read*, not
//! construct. `to_relation` instead bridges the two vocabularies through their
//! shared wire spelling — `continuum_value::assurance::AssuranceChange::as_str()`
//! and `continuum_intent::change_policy::Relation::from_wire()` agree token for
//! token on `unchanged`/`upgraded`/`downgraded` (RFC 0031's own text: "The relation
//! is `AssuranceChange`, whose three members are `unchanged`, `upgraded`,
//! `downgraded`: the schema's sixteen-relation set restricted to this field") — so
//! the upstream type's own stable accessor method is enough, and its name never has
//! to appear anywhere in this crate — including its `#[cfg(test)]` blocks, which
//! build every test fixture's `minimum` by calling `continuum_intent::
//! assurance_policy::level_from_wire` on a wire token (`"sampled"`, `"validated"`,
//! ...) rather than naming `AssuranceLevel` directly, so `continuum-value` does not
//! need to appear in `Cargo.toml` at all, not even under `[dev-dependencies]`. (An
//! earlier draft of this bone did add it there, on the reasoning that
//! `[dev-dependencies]` are only *reported*, not *enforced*, by
//! `tools/check_crate_boundaries.py`; that is true of the boundary checker, but
//! `tools/governance/check_code_policy.py`'s GOV-1-07 dependency-rationale check
//! reads every manifest dependency, dev included, and failed `just check` until the
//! test-only edge was removed rather than reconciled with
//! `tools/governance/dependency-rationale.toml` — avoiding the edge entirely turned
//! out simpler than accounting for it twice.)
//!
//! # Scope: PR-12 / IMPL-06 only
//!
//! See [`crate::faults`]'s module doc, "Scope: PR-12 / IMPL-06 only" — the same
//! boundary applies here and to [`crate::fairness`], all three landing together
//! under `bn-3vxp`.

use continuum_intent::assurance_policy::{AssuranceComparisonError, AssurancePolicy};
use continuum_intent::change_policy::{
    ChangePolicyError, ClassificationRecord, PolicyField, Relation,
};

/// The classified `assurance` requirement-triple movement: the [`Relation`] RFC 0031
/// assigns one `(minimum, independent_checker, clean_recompute)` comparison.
///
/// No unit locator — like [`crate::bounds::BoundsChange`], `assurance` is classified
/// as a whole (`notes/plan/schemas/semantic-diff.schema.json`'s `unit` `$comment`
/// names it as one of the four fields with "no sub-unit to name") — so this type is a
/// thin, checked wrapper over [`Relation`] and nothing more.
///
/// Distinct from `continuum_value::assurance::AssuranceChange`; see the module doc,
/// "Bridging two closed vocabularies without a second dependency".
///
/// The relation is always one [`PolicyField::Assurance`] admits, and — pinned by this
/// module's tests, not merely asserted — is never [`Relation::Incomparable`]: RFC
/// 0031 states outright that "`incomparable` MUST NOT be emitted for `assurance`",
/// and [`classify_assurance`] can only ever produce [`Relation::Unchanged`],
/// [`Relation::Upgraded`], [`Relation::Downgraded`], or [`Relation::Unknown`] (the
/// declaredness-change refusal) — four of the five relations
/// [`PolicyField::Assurance`] admits (see the module doc; `unsupported` is the fifth
/// and does not arise either, for the same "no fragment to fall outside of" reason
/// [`crate::faults`] and [`crate::observers`] give for their own fields —
/// `assurance` carries no `fragment` member in the live schema, confirmed by this
/// module's tests).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AssuranceChange {
    relation: Relation,
}

impl AssuranceChange {
    fn new(relation: Relation) -> Self {
        debug_assert!(
            PolicyField::Assurance.admits_relation(relation),
            "an assurance classification produced {relation}, which the assurance row does not \
             admit"
        );
        debug_assert_ne!(
            relation,
            Relation::Incomparable,
            "RFC 0031 pins `incomparable` as never emitted for `assurance` (its own total-order \
             rule); this module must never construct it"
        );
        Self { relation }
    }

    /// The RFC 0031 relation this comparison classified to.
    #[must_use]
    pub const fn relation(&self) -> Relation {
        self.relation
    }

    /// This change as a field-level [`ClassificationRecord`], for
    /// [`continuum_intent::change_policy::PolicyTable::verdict`].
    ///
    /// # Errors
    ///
    /// [`ChangePolicyError::InadmissibleRelation`] — never in practice, since every
    /// relation this module produces is on the assurance row (see the struct doc),
    /// but [`ClassificationRecord::new`] is itself fallible and a `debug_assert!` is
    /// not a proof, so the `Result` is threaded rather than unwrapped.
    pub fn to_classification_record(&self) -> Result<ClassificationRecord, ChangePolicyError> {
        ClassificationRecord::new(PolicyField::Assurance, self.relation)
    }
}

/// Classify the `assurance` requirement-triple movement from `before` to `after`,
/// per RFC 0031's "Assurance movement".
///
/// Delegates entirely to [`AssurancePolicy::change_to`]; see the module doc,
/// "Closed over `continuum-intent`, not recomputed". A
/// [`AssuranceComparisonError::CheckerDeclarednessChanged`] refusal — a checker flag
/// declared on one side and undeclared on the other — classifies [`Relation::Unknown`]
/// and fails closed, exactly as the quoted rule requires; no other error variant
/// exists on that type today, and this match is written to name the one that does
/// rather than swallow it with a wildcard, so that a future variant forces a
/// conscious decision here rather than silently inheriting `unknown`.
#[must_use]
pub fn classify_assurance(before: &AssurancePolicy, after: &AssurancePolicy) -> AssuranceChange {
    let relation = match before.change_to(after) {
        Ok(change) => to_relation(change.as_str()),
        Err(AssuranceComparisonError::CheckerDeclarednessChanged { flag: _ }) => Relation::Unknown,
    };
    AssuranceChange::new(relation)
}

/// Bridge `continuum_value::assurance::AssuranceChange::as_str()`'s wire token onto
/// this crate's own [`Relation`] vocabulary, without naming the upstream type (see
/// the module doc).
fn to_relation(wire: &str) -> Relation {
    Relation::from_wire(wire).unwrap_or_else(|| {
        unreachable!(
            "`AssuranceChange::as_str` only ever returns \"unchanged\", \"upgraded\", or \
             \"downgraded\" (RFC 0031: \"whose three members are `unchanged`, `upgraded`, \
             `downgraded`\"), and all three are `Relation` wire tokens; got {wire:?}"
        )
    })
}

#[cfg(test)]
mod tests {
    use continuum_intent::assurance_policy::level_from_wire;
    use continuum_intent::canonical_json::Json;

    use super::*;

    /// The dossier's validated Intent Contract example, included at compile time —
    /// used here only to confirm the live schema still carries no `fragment` member
    /// on `assurance` (the struct doc's "no analogue" reasoning).
    const SCHEMA: &str = include_str!("../../../notes/plan/schemas/intent-contract.schema.json");

    /// Every `assurance.minimum` wire token, weakest first — this module's own
    /// dependency-free stand-in for `continuum_value::assurance::AssuranceLevel::ALL`
    /// (see the module doc, "Bridging two closed vocabularies without a second
    /// dependency": this crate does not depend on `continuum-value` at all, not even
    /// under `[dev-dependencies]`, so these five tests go through
    /// `continuum_intent::assurance_policy::level_from_wire` instead of naming the
    /// upstream enum).
    const LEVELS: [&str; 5] = ["observed", "sampled", "bounded", "validated", "proved"];

    fn policy(
        minimum: &str,
        independent_checker: Option<bool>,
        clean_recompute: Option<bool>,
    ) -> AssurancePolicy {
        let minimum = level_from_wire(minimum)
            .unwrap_or_else(|| panic!("{minimum:?} is not one of the five assurance levels"));
        AssurancePolicy::new(minimum, independent_checker, clean_recompute, [])
            .expect("test assurance policies are well formed")
    }

    // --- the three totally-ordered outcomes ---------------------------------------------------

    #[test]
    fn an_identical_triple_classifies_unchanged() {
        let a = policy("validated", Some(true), Some(true));
        let b = policy("validated", Some(true), Some(true));
        assert_eq!(classify_assurance(&a, &b).relation(), Relation::Unchanged);
    }

    #[test]
    fn a_risen_minimum_with_nothing_dropped_classifies_upgraded() {
        let before = policy("sampled", Some(true), Some(true));
        let after = policy("proved", Some(true), Some(true));
        assert_eq!(
            classify_assurance(&before, &after).relation(),
            Relation::Upgraded
        );
    }

    #[test]
    fn lowering_the_minimum_from_exhaustive_to_sampled_classifies_downgraded() {
        // RFC 0031's own gaming-move example, in spirit: "lower assurance from
        // exhaustive to sampled".
        let before = policy("bounded", Some(true), Some(true));
        let after = policy("sampled", Some(true), Some(true));
        assert_eq!(
            classify_assurance(&before, &after).relation(),
            Relation::Downgraded
        );
    }

    #[test]
    fn dropping_either_checker_alone_classifies_downgraded_even_with_minimum_unchanged() {
        let before = policy("validated", Some(true), Some(true));
        let after_no_independent = policy("validated", Some(false), Some(true));
        assert_eq!(
            classify_assurance(&before, &after_no_independent).relation(),
            Relation::Downgraded
        );
        let after_no_clean = policy("validated", Some(true), Some(false));
        assert_eq!(
            classify_assurance(&before, &after_no_clean).relation(),
            Relation::Downgraded
        );
    }

    #[test]
    fn dropping_the_independent_checker_while_raising_the_minimum_still_classifies_downgraded() {
        // The exact gaming move the module doc quotes: "stops a contract from
        // dropping its independent checker in exchange for a nominally higher
        // `minimum` and passing `no-downgrade`". Fail closed in both directions: no
        // net direction is computed.
        let before = policy("sampled", Some(true), Some(true));
        let after = policy("proved", Some(false), Some(true));
        assert_eq!(
            classify_assurance(&before, &after).relation(),
            Relation::Downgraded
        );
    }

    // --- declaredness fails closed -------------------------------------------------------------

    #[test]
    fn a_declaredness_change_on_independent_checker_is_unknown_even_when_minimum_rises() {
        let before = policy("sampled", None, Some(true));
        let after = policy("proved", Some(true), Some(true));
        assert_eq!(
            classify_assurance(&before, &after).relation(),
            Relation::Unknown
        );
    }

    #[test]
    fn a_declaredness_change_on_clean_recompute_is_also_unknown() {
        let before = policy("validated", Some(true), Some(true));
        let after = policy("validated", Some(true), None);
        assert_eq!(
            classify_assurance(&before, &after).relation(),
            Relation::Unknown
        );
    }

    #[test]
    fn both_flags_undeclared_on_both_sides_compares_by_minimum_alone() {
        let before = policy("sampled", None, None);
        let after = policy("bounded", None, None);
        assert_eq!(
            classify_assurance(&before, &after).relation(),
            Relation::Upgraded
        );
    }

    // --- the pin: `incomparable` is never emitted ------------------------------------------------

    #[test]
    fn incomparable_is_never_emitted_across_a_battery_of_mixed_and_declaredness_inputs() {
        let flags = [None, Some(false), Some(true)];
        for &before_level in &LEVELS {
            for &after_level in &LEVELS {
                for before_ic in flags {
                    for after_ic in flags {
                        for before_cr in flags {
                            for after_cr in flags {
                                let before = policy(before_level, before_ic, before_cr);
                                let after = policy(after_level, after_ic, after_cr);
                                assert_ne!(
                                    classify_assurance(&before, &after).relation(),
                                    Relation::Incomparable,
                                    "assurance must never emit incomparable: before={before:?} \
                                     after={after:?}"
                                );
                            }
                        }
                    }
                }
            }
        }
    }

    // --- regression guard: `assurance` refuses `incomparable` (RFC 0031 correction 13) --------

    #[test]
    fn admits_relation_now_refuses_incomparable_on_assurance_per_correction_13() {
        // RFC 0031 correction 13 is explicit: "the non-affirmative three are
        // admissible on every row (with `incomparable` excluded from `assurance`
        // alone, per its own total-order rule)".
        //
        // `bn-3vxp` found and pinned (as this test, then named
        // `admits_relation_currently_over_admits_incomparable_on_assurance_a_recorded_continuum_intent_gap`)
        // that `PolicyField::admits_relation` implemented the *first* half
        // unconditionally (`!relation.is_affirmative() || ...`, no field-specific
        // override) but not the parenthetical exclusion: the boolean short-circuited
        // to `true` for every non-affirmative relation on every field, `assurance`
        // included, before the field-specific `classified_relations()` set was ever
        // consulted. `bn-2sngz` closed that gap directly in
        // `crates/continuum-intent/src/change_policy.rs` (`continuum-intent` is no
        // longer read-only for this bone), so the assertion below is now the negation
        // of the one `bn-3vxp` recorded, and this test guards the fix rather than
        // documenting the divergence.
        //
        // [`classify_assurance`] itself never *constructed* `Incomparable` either side
        // of this fix (see
        // `incomparable_is_never_emitted_across_a_battery_of_mixed_and_declaredness_inputs`,
        // above, and the `debug_assert_ne!` in `AssuranceChange::new`), so the gap had
        // no live consequence for PR-12 / IMPL-06; it would only have mattered to a
        // *different* caller that trusted `admits_relation` alone to reject an
        // `Incomparable` record on `assurance` sight unseen — which is exactly what
        // `admits_relation` now does.
        assert!(
            !PolicyField::Assurance.admits_relation(Relation::Incomparable),
            "`assurance` must refuse `incomparable` (RFC 0031 correction 13's parenthetical); if \
             this now returns true, the fix in `continuum-intent`'s `admits_relation` has \
             regressed"
        );
    }

    // --- the field-classification contract --------------------------------------------------

    #[test]
    fn every_relation_this_module_can_produce_is_admissible_on_the_assurance_field() {
        for relation in [
            Relation::Unchanged,
            Relation::Upgraded,
            Relation::Downgraded,
            Relation::Unknown,
        ] {
            assert!(
                PolicyField::Assurance.admits_relation(relation),
                "{relation} must be admissible on `assurance`"
            );
        }
    }

    #[test]
    fn to_classification_record_never_fails_for_a_relation_this_module_produced() {
        let before = policy("bounded", Some(true), Some(true));
        let after = policy("sampled", Some(true), Some(true));
        let change = classify_assurance(&before, &after);
        let record = change
            .to_classification_record()
            .expect("this module's own relations are always admissible on `assurance`");
        assert_eq!(record.field(), PolicyField::Assurance);
        assert_eq!(record.relation(), change.relation());
    }

    #[test]
    fn the_live_schema_carries_no_fragment_member_on_assurance() {
        let schema = Json::parse(SCHEMA.as_bytes()).expect("the schema itself is canonical JSON");
        let assurance_properties = schema
            .as_object()
            .expect("schema root is an object")
            .get("properties")
            .and_then(Json::as_object)
            .expect("schema declares properties")
            .get("assurance")
            .and_then(Json::as_object)
            .expect("schema declares assurance")
            .get("properties")
            .and_then(Json::as_object)
            .expect("assurance is an object schema with properties");
        assert!(
            !assurance_properties.contains_key("fragment"),
            "assurance gained a `fragment` member; this module's reasoning about \
             `unsupported` depends on its absence and must be revisited"
        );
        for expected in ["minimum", "independent_checker", "clean_recompute"] {
            assert!(
                assurance_properties.contains_key(expected),
                "assurance is missing its documented component {expected:?}"
            );
        }
    }

    // --- anti-vacuity mutants, both directions --------------------------------------------------

    /// A plausible, *wrong* classifier: decides purely by `minimum`, ignoring both
    /// checker flags entirely. Wrong in the *lenient* direction: it misses the
    /// checker-dropping attack.
    fn mutant_ignores_checkers(before: &AssurancePolicy, after: &AssurancePolicy) -> &'static str {
        use core::cmp::Ordering;
        match before.minimum().cmp(&after.minimum()) {
            Ordering::Equal => "unchanged",
            Ordering::Less => "upgraded",
            Ordering::Greater => "downgraded",
        }
    }

    #[test]
    fn negative_mutant_ignoring_checkers_would_wrongly_pass_a_dropped_checker_as_unchanged() {
        let before = policy("validated", Some(true), Some(true));
        let after = policy("validated", Some(false), Some(true));

        assert_eq!(
            classify_assurance(&before, &after).relation(),
            Relation::Downgraded,
            "the real classifier sees the dropped independent checker"
        );
        assert_eq!(
            mutant_ignores_checkers(&before, &after),
            "unchanged",
            "the mutant must actually get this wrong, or it is not exercising the bug"
        );
    }

    /// A plausible, *wrong* classifier in the opposite direction: treats any
    /// inequality at all as a downgrade, ignoring the requirement's own order. Wrong
    /// in the *paranoid* direction: it would falsely flag a genuine, benign upgrade.
    fn mutant_any_inequality_is_a_downgrade(
        before: &AssurancePolicy,
        after: &AssurancePolicy,
    ) -> &'static str {
        if before.minimum() == after.minimum()
            && before.independent_checker() == after.independent_checker()
            && before.clean_recompute() == after.clean_recompute()
        {
            "unchanged"
        } else {
            "downgraded"
        }
    }

    #[test]
    fn negative_mutant_flagging_any_inequality_would_wrongly_call_a_benign_upgrade_a_downgrade() {
        let before = policy("sampled", Some(true), Some(true));
        let after = policy("proved", Some(true), Some(true));

        assert_eq!(
            classify_assurance(&before, &after).relation(),
            Relation::Upgraded,
            "the real classifier correctly reads a pure rise in `minimum` as an upgrade"
        );
        assert_eq!(
            mutant_any_inequality_is_a_downgrade(&before, &after),
            "downgraded",
            "the mutant must actually get this wrong (a false alarm on a benign change), or it \
             is not exercising the bug"
        );
    }
}
