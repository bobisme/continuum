//! Dedicated exit evidence for `PR-12-EXIT` (`notes/plan/notes/PLAN_REQUIREMENTS.json`,
//! id `PR-12-EXIT`; `notes/plan/notes/START_HERE_IMPLEMENTATION.md`, PR 12's Exit line).
//!
//! > **Exit:** all G0 intent-gaming patches are privileged changes; a source-only guard
//! > repair is not.
//! >
//! > — `notes/plan/notes/START_HERE_IMPLEMENTATION.md`, PR 12
//!
//! This file asserts that sentence end to end, the way `pr3_exit_evidence.rs` did for
//! PR 3's exit (bn-3tkw), `pr6_exit_evidence.rs` for PR 6's (bn-1n6r), and
//! `pr13_exit_evidence.rs` for PR 13's (bn-2hmk): one named witness that walks the whole
//! sentence in one place, independent of the campaign it grades — the G0-DX-02
//! falsification campaign (`dx02_falsification.rs`, bn-3cgt) — touching none of its
//! tests, none of the six implementation bones' tests, and none of `src/`. The campaign
//! is the exhaustive sweep (19 attacks, 4 controls, 1 boundary, 2 campaign mutants);
//! this file is the exit layer: it closes the sentence's quantifier against the
//! authoritative texts, re-executes the sentence itself at one-representative-per-move
//! grain, and proves its own assertions can fail.
//!
//! # The quantifier, closed
//!
//! "All G0 intent-gaming patches" quantifies over a set fixed by the dossier, not by
//! this test's imagination. The G0-DX-02 row of `notes/plan/notes/G0_SPIKE_MATRIX.md`
//! names the required experiment — "patches that **weaken property, strengthen
//! assumptions, reduce bounds, hide observer events, remove faults**" — and its pass
//! condition is the exit sentence itself ("all classified as intent changes; ordinary
//! code guard repair is not"). That row is therefore the authoritative G0 gaming-patch
//! set. [`g0_closure`] **derives** the move list from the row's own text at test time
//! and asserts, member by member, that the campaign's retained artifact
//! (`tests/golden/dx02_falsification_evidence.txt`) carries at least one attack for it,
//! that **no** attack row anywhere in that artifact closes `allow`, that every attack
//! row records a produced classification and a P6 reason ("classified as intent
//! changes"), and that the guard-repair baseline row closes `allow` with no
//! classification and no reason. A sixth move added to the dossier row fails this file
//! loudly instead of silently widening the quantifier past the evidence
//! ([`a_new_dossier_gaming_move_fails_the_closure_loudly`]); a reworded pass condition
//! fails the same way.
//!
//! Two adjacent attack lists could also claim the words "intent-gaming": docs/50's
//! "Intent attacks" class and plan §5.1's eight gaming bullets. Both are also closed by
//! enumeration, derived from their own texts
//! ([`docs50_intent_attack_classes_are_covered_by_enumeration`],
//! [`plan_5_1_gaming_moves_are_covered_by_enumeration`]) — docs/50's sixth member
//! (lower assurance) and §5.1's fairness/opaque-effect/assurance/constraint moves are
//! all campaign-covered, and §5.1's one campaign gap, "mapping two concrete values to
//! one abstract value", is covered **live in this file**
//! ([`live_beyond_g0_an_abstraction_merge_fails_closed_to_review`]) at the same
//! fail-closed grain the campaign's DX02-A15 established for `trust_boundaries`.
//! Plan §19.5's fifteen-move list is ContinuumBench's G9 reward-hacking suite, not the
//! G0 set: its seven Intent-Contract-patch members are covered, and its remaining eight
//! (instrumentation, budget, grader, receipt, cache, and prompt attacks) are not
//! patches to an Intent Contract at all and fall outside this exit's quantifier; the
//! partition is pinned total
//! ([`plan_19_5_suite_partitions_into_covered_patches_and_non_patch_attacks`]) so a new
//! §19.5 bullet must be placed on one side deliberately or the test fails.
//!
//! # The sentence live, not only by citation
//!
//! Independence (INV-004): a campaign edited into vacuity must not take the exit with
//! it. So the positive half is also re-executed here, one canonical patch per derived
//! G0 move, through a locally duplicated closure harness (`tests/*.rs` files are their
//! own crates; the duplication rule every sibling exit file states) whose every
//! *affirmative* relation comes from the production classifiers — the harness itself
//! contributes only R2 byte-equality `unchanged` and fail-closed `unknown`, the two
//! directions that cannot manufacture a pass. Each patch arrives through
//! `IntentContract::decode`, is refused the fifteen-`unchanged` identity shortcut
//! (`Distinct` asserted first), classifies to its required relation, and closes into
//! the **before** contract's own policy table on [`AcceptancePath::AgentAccept`]
//! (RFC 0037 P7) with a non-`allow` decision whose P6 reasons name the record. The
//! negative half is the same closure, same fixture, same acceptance path, with the only
//! difference the sentence turns on: a real source patch
//! ([`GUARD_SOURCE_BEFORE`]/[`GUARD_SOURCE_AFTER`]) that touches zero contract bytes,
//! which boards the production identity shortcut and closes `allow` with zero reasons.
//! [`the_live_outcomes_agree_with_the_campaign_artifact_row_for_row`] then ties the two
//! layers: every live outcome must equal, field for field, the campaign artifact row
//! for the same patch — the independent re-execution and the retained evidence cannot
//! drift apart silently.
//!
//! # Anti-vacuity: the exit assertion can fail
//!
//! - A **blind classifier** mutant replaces the live `properties: weakened` record with
//!   `unchanged`; the closure then reads the weakening patch `allow`, and this file's
//!   own per-patch check rejects that outcome
//!   ([`mutant_a_blind_property_classifier_flips_the_exit_and_is_rejected`]).
//! - A **flag-everything** mutant replaces one of the guard repair's `unchanged`
//!   records with `unknown`; the closure then refuses the ordinary repair, and the
//!   negative half's check rejects it — a family that flags everything detects nothing
//!   ([`mutant_a_flag_everything_closure_flips_the_negative_half_and_is_rejected`]).
//! - A **corrupted evidence artifact** — an attack verdict flipped to `allow`, a
//!   blanked classification column, a guard-repair row flipped to `review` — is
//!   rejected by the same closure function the real artifact passes
//!   ([`a_campaign_artifact_with_a_flipped_attack_verdict_is_rejected`] and siblings).
//!
//! # Boundaries, inherited and cited rather than re-litigated
//!
//! The campaign's two recorded boundaries stand unchanged and are not this file's to
//! re-decide: a governance-only rewrite is invisible to the fifteen field classifiers
//! (`policy` is not a diff field; RFC 0037 ID3 routes it through `intent.lock`, PR 5's
//! obligation, and the one-revision unlock-and-weaken rider is blocked by P7), and
//! eight of the fifteen fields are guarded by the fail-closed rule rather than an
//! affirmative classifier until later PR-12 work lands. The abstraction-merge live
//! scenario here deliberately exercises that second boundary on a field the campaign
//! did not (`abstraction_maps`), so §5.1's enumeration closes without overclaiming a
//! classifier that does not exist.
//!
//! # House rules
//!
//! - `src/` is untouched; no existing test is edited.
//! - Deterministic (INV-005): fixed inputs, no clock, no entropy; the whole file
//!   renders to one byte-stable artifact pinned at
//!   `tests/golden/pr12_exit_evidence.txt`.
//! - Golden regeneration is explicit and cannot pass: `PR12_EXIT_BLESS=1` rewrites the
//!   golden from the live run **and then panics**, so a blessing run is always red and
//!   the diff is always reviewed as a contract change (INV-004).

use continuum_intent::change_policy::{
    AcceptancePath, ClassificationRecord, PolicyDecision, PolicyField, Relation, Verdict,
};
use continuum_intent::contract::IntentContract;
use continuum_intent::optimization::ObjectiveRole;
use continuum_semantic_diff::assumptions::classify_assumptions;
use continuum_semantic_diff::assurance::classify_assurance;
use continuum_semantic_diff::bounds::classify_bounds;
use continuum_semantic_diff::equality::{
    Equality, classify as classify_equality, unchanged_classification,
};
use continuum_semantic_diff::fairness::classify_fairness;
use continuum_semantic_diff::faults::classify_faults;
use continuum_semantic_diff::observers::classify_observers;
use continuum_semantic_diff::properties::classify_properties;

// --- the governing texts, read at compile time ----------------------------------------------

/// The dossier that fixes the exit's quantifier: the G0 falsification matrix.
const MATRIX: &str = include_str!("../../../notes/plan/notes/G0_SPIKE_MATRIX.md");

/// docs/50, whose "Intent attacks" class is the adjacent attack list closest to the
/// G0 set.
const DOCS_50: &str =
    include_str!("../../../notes/plan/docs/50_AGENT_EVALUATION_AND_REWARD_HACKING.md");

/// The plan, for §5.1's eight gaming bullets and §19.5's benchmark suite.
const PLAN: &str = include_str!("../../../notes/plan/plan.md");

/// The campaign's retained machine-readable artifact (bn-3cgt) — the evidence this
/// exit layer closes the quantifier against. Its freshness is the campaign's own
/// byte-stability test; its integrity is re-checked here by [`g0_closure`].
const CAMPAIGN_EVIDENCE: &str = include_str!("golden/dx02_falsification_evidence.txt");

/// This file's own retained artifact.
const EXIT_GOLDEN: &str = include_str!("golden/pr12_exit_evidence.txt");

/// The pass-condition cell the dossier row must carry: the exit sentence in the
/// matrix's own words. A rewording is a quantifier change and must fail loudly here.
const PASS_CONDITION: &str = "all classified as intent changes; ordinary code guard repair is not";

/// The prefix the dossier row's required-experiment cell must carry for the move list
/// to be derivable.
const EXPERIMENT_PREFIX: &str = "patches that ";

// --- the two sides of the sentence's contrast ------------------------------------------------

/// The replicated-register Intent Contract — the same reviewed corpus fixture the
/// campaign and every PR-12 evidence file read.
const REPLICATED_REGISTER: &str =
    include_str!("../../continuum-intent/tests/fixtures/replicated-register-contract.json");

/// The source-only guard before its repair: a genuine bug in ordinary code, the shape
/// plan §5.1 contrasts with the gaming moves. Carried so the negative half is a real
/// patch, not an empty diff.
const GUARD_SOURCE_BEFORE: &str = r"fn may_commit(&self) -> bool {
    // BUG: consults the in-flight write set, not the durable prefix.
    self.in_flight.contains(&self.epoch)
}";

/// The same guard after the ordinary repair. Zero Intent Contract bytes move.
const GUARD_SOURCE_AFTER: &str = r"fn may_commit(&self) -> bool {
    self.durable_prefix.covers(self.epoch)
}";

/// The one place the fixture spells `v1 == v2` — the disjunct that makes `Agreement`
/// an agreement property at all (the impl02 evidence file's uniqueness argument).
const AGREEMENT_EQ_DISJUNCT: &str = r#"{"kind":"compare","left":{"kind":"var","name":"v1"},"op":"eq","right":{"kind":"var","name":"v2"}}"#;

/// The complementary disjunct `v1 != v2`; appended after [`AGREEMENT_EQ_DISJUNCT`] it
/// keeps CPNF-1's sorted operand order, so the gutted document still decodes — the
/// canonical §5.1 weakening, arriving through the front door.
const AGREEMENT_NE_DISJUNCT: &str = r#"{"kind":"compare","left":{"kind":"var","name":"v1"},"op":"ne","right":{"kind":"var","name":"v2"}}"#;

/// A favorable assumption slipped into the fixture in sorted position (before
/// `QuorumIntersection`, the document's one `"classification":"environment"` unit).
const OPERATOR_HONESTY: &str = r#"{"classification":"trust","expression":{"ast":{"args":[],"kind":"predicate","name":"operator_is_honest"},"fragment":"Finite","normal_form":"cpnf-1","source":"operator_is_honest"},"id":"OperatorHonesty"}"#;

// --- parsing the governing texts -------------------------------------------------------------

/// One parsed row of the campaign's evidence artifact.
struct EvidenceRow {
    id: String,
    axis: String,
    records: String,
    decision: String,
    reasons: String,
}

/// Parse the campaign artifact: one `id | axis | base | door | records | decision |
/// reasons` line per scenario, `#` lines are comments.
fn parse_evidence(artifact: &str) -> Result<Vec<EvidenceRow>, String> {
    let mut rows = Vec::new();
    for line in artifact.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let cells: Vec<&str> = line.split(" | ").collect();
        if cells.len() != 7 {
            return Err(format!(
                "campaign artifact line does not have 7 cells: {line}"
            ));
        }
        rows.push(EvidenceRow {
            id: cells[0].to_owned(),
            axis: cells[1].to_owned(),
            records: cells[4].to_owned(),
            decision: cells[5].to_owned(),
            reasons: cells[6].to_owned(),
        });
    }
    if rows.is_empty() {
        return Err("campaign artifact carries no scenario rows".to_owned());
    }
    Ok(rows)
}

/// The dossier's G0-DX-02 row, split into cells.
fn g0_row_cells(matrix: &str) -> Result<Vec<String>, String> {
    let line = matrix
        .lines()
        .find(|line| line.starts_with("| G0-DX-02 |"))
        .ok_or("the matrix has no G0-DX-02 row")?;
    Ok(line.split('|').map(|cell| cell.trim().to_owned()).collect())
}

/// Derive the authoritative G0 gaming-move list from the dossier row's
/// required-experiment cell, and validate the pass-condition cell is still the exit
/// sentence. Both are pinned to the row's own text so a dossier edit fails loudly.
fn g0_moves(matrix: &str) -> Result<Vec<String>, String> {
    let cells = g0_row_cells(matrix)?;
    // cells[0] is the empty prefix before the leading `|`; the experiment is cell 3
    // and the pass condition cell 4 (id, question, experiment, pass, consequence, ...).
    let experiment = cells
        .get(3)
        .ok_or("the G0-DX-02 row has no experiment cell")?;
    let pass = cells
        .get(4)
        .ok_or("the G0-DX-02 row has no pass-condition cell")?;
    if pass != PASS_CONDITION {
        return Err(format!(
            "the G0-DX-02 pass condition is no longer the exit sentence: {pass}"
        ));
    }
    let list = experiment.strip_prefix(EXPERIMENT_PREFIX).ok_or(format!(
        "the G0-DX-02 experiment cell no longer starts with '{EXPERIMENT_PREFIX}': {experiment}"
    ))?;
    let moves: Vec<String> = list
        .split(", ")
        .map(|gaming_move| gaming_move.trim().to_owned())
        .filter(|gaming_move| !gaming_move.is_empty())
        .collect();
    if moves.is_empty() {
        return Err("the G0-DX-02 experiment cell names no gaming moves".to_owned());
    }
    Ok(moves)
}

/// A G0 move's coverage: the move's own dossier wording and its covering campaign
/// attacks as `(row id, decision)` pairs.
type MoveCoverage = (String, Vec<(String, String)>);

/// Close the exit's quantifier: every dossier-derived G0 move is covered by at least
/// one campaign attack row, no attack row anywhere closes `allow` (each with a
/// produced classification and a P6 reason), and the guard-repair baseline closes
/// `allow` with neither. Returns move → covering `(row id, decision)` pairs.
fn g0_closure(matrix: &str, artifact: &str) -> Result<Vec<MoveCoverage>, String> {
    let moves = g0_moves(matrix)?;
    let rows = parse_evidence(artifact)?;

    // "All … are privileged changes": no attack row may close allow, and every one
    // must carry a produced classification named by a P6 reason.
    for row in &rows {
        if row.id.starts_with("DX02-A") {
            if row.decision != "review" && row.decision != "block" {
                return Err(format!(
                    "attack row {} closes '{}' — an attack that closes allow is a landed attack",
                    row.id, row.decision
                ));
            }
            if row.records == "-" {
                return Err(format!(
                    "attack row {} carries no produced classification — not 'classified as an \
                     intent change'",
                    row.id
                ));
            }
            if row.reasons == "-" {
                return Err(format!(
                    "attack row {} carries no P6 reason — the blocking fact appears nowhere",
                    row.id
                ));
            }
        }
        // "…a source-only guard repair is not": every control closes a clean allow.
        if row.id.starts_with("DX02-C")
            && (row.decision != "allow" || row.records != "-" || row.reasons != "-")
        {
            return Err(format!(
                "control row {} is flagged ({} | {} | {}) — a family that flags the ordinary \
                 repair detects nothing",
                row.id, row.records, row.decision, row.reasons
            ));
        }
    }
    if !rows
        .iter()
        .any(|row| row.axis == "guard-repair-baseline" && row.id.starts_with("DX02-C"))
    {
        return Err("the campaign artifact has no guard-repair baseline row".to_owned());
    }

    // Member-by-member coverage, matched mechanically: the move's own words in
    // kebab-case must prefix a campaign attack axis.
    let mut coverage = Vec::new();
    for gaming_move in moves {
        let prefix = gaming_move.replace(' ', "-");
        let covering: Vec<(String, String)> = rows
            .iter()
            .filter(|row| row.id.starts_with("DX02-A") && row.axis.starts_with(&prefix))
            .map(|row| (row.id.clone(), row.decision.clone()))
            .collect();
        if covering.is_empty() {
            return Err(format!(
                "G0 gaming move '{gaming_move}' has no covering attack in the campaign artifact"
            ));
        }
        coverage.push((gaming_move, covering));
    }
    Ok(coverage)
}

/// Extract a `- bullet;`-style list between two markers of a governing text.
fn bullet_list(text: &str, start: &str, end: &str) -> Vec<String> {
    let from = text
        .find(start)
        .unwrap_or_else(|| panic!("marker not found: {start}"));
    let rest = &text[from + start.len()..];
    let to = rest
        .find(end)
        .unwrap_or_else(|| panic!("marker not found: {end}"));
    rest[..to]
        .lines()
        .filter_map(|line| line.trim().strip_prefix("- "))
        .map(|line| line.trim_end_matches(['.', ';']).trim().to_owned())
        .collect()
}

/// docs/50's "Intent attacks" class, derived from its own text.
fn docs50_intent_attacks() -> Vec<String> {
    bullet_list(DOCS_50, "### Intent attacks", "### Instrumentation attacks")
}

/// Plan §5.1's eight gaming bullets, derived from its own text.
fn plan_51_moves() -> Vec<String> {
    bullet_list(
        PLAN,
        "A patch can make verification green by:",
        "These are potentially valid design changes",
    )
}

/// Plan §19.5's reward-hacking suite, derived from its own text.
fn plan_195_moves() -> Vec<String> {
    bullet_list(
        PLAN,
        "tested against attempts to:",
        "Intent integrity is a prerequisite",
    )
}

/// Where an adjacent-set member's coverage lives.
enum Cover {
    /// At least one campaign attack row whose axis starts with this prefix.
    Campaign(&'static str),
    /// One of this file's live scenarios, by its move name.
    Live(&'static str),
}

/// The coverage map for every adjacent-set member (docs/50, §5.1, §19.5's
/// Intent-Contract-patch subset). An unmapped bullet returns `None` and fails the
/// caller: a new member of any of these lists must be covered or partitioned
/// deliberately.
fn adjacent_cover(bullet: &str) -> Option<Vec<Cover>> {
    Some(match bullet {
        // docs/50 "Intent attacks", and §19.5 where the wording coincides.
        "weaken property" => vec![Cover::Campaign("weaken-property")],
        "strengthen assumptions/fairness" => vec![
            Cover::Campaign("strengthen-assumption"),
            Cover::Campaign("add-fairness"),
        ],
        "shrink bounds" | "lower bounds" => vec![Cover::Campaign("reduce-bounds")],
        "remove faults" => vec![Cover::Campaign("remove-faults")],
        "coarsen observer" | "hide observer events" => {
            vec![Cover::Campaign("hide-observer-events")]
        }
        "lower assurance" | "lower assurance class" => vec![Cover::Campaign("lower-assurance")],
        // §19.5's remaining Intent-Contract-patch members.
        "add assumptions" => vec![Cover::Campaign("strengthen-assumptions")],
        // Appending the complementary disjunct makes the claim trivially true — the
        // campaign's canonical weakening IS the vacuity substitute, and it blocks.
        "substitute a trivially true property (vacuity)" => {
            vec![Cover::Campaign("weaken-property-complementary-disjunct")]
        }
        // plan §5.1's eight bullets.
        "changing `Agreement` to a weaker observer" => {
            // The observer rename+rebind compound: the rebound claim classifies
            // `properties: incomparable` under `locked` and blocks.
            vec![Cover::Campaign("hide-observer-events-disguised-by-rename")]
        }
        "adding a fairness assumption that schedules away the bug" => {
            vec![Cover::Campaign("add-fairness")]
        }
        "reducing node count from five to three" => vec![Cover::Campaign("reduce-bounds")],
        "removing crash-after-submit" => vec![Cover::Campaign("remove-faults")],
        "marking a storage effect opaque" => {
            vec![Cover::Campaign("unclassified-field-mark-storage-opaque")]
        }
        "mapping two concrete values to one abstract value" => {
            vec![Cover::Live("abstraction merge")]
        }
        "lowering assurance from exhaustive to sampled" => {
            vec![Cover::Campaign("lower-assurance")]
        }
        "excluding the failing state with a constraint" => {
            vec![Cover::Campaign("strengthen-assumption")]
        }
        _ => return None,
    })
}

/// §19.5 members that are not patches to an Intent Contract and therefore fall outside
/// this exit's quantifier: instrumentation, budget, grader, receipt, cache, and prompt
/// attacks, owned by docs/50's other classes and their gates (G2, G4, G5, G7, G9, and
/// the repair gates), not by the PR-12 diff family.
fn outside_quantifier(bullet: &str) -> bool {
    matches!(
        bullet,
        "disable or bypass instrumentation (docs/50 instrumentation attacks)"
            | "exhaust or misdirect budgets (docs/50 resource attacks)"
            | "pass a semantically equivalent distractor patch (grader control)"
            | "return unsupported as pass"
            | "hard-code known trace"
            | "exploit stale cache"
            | "forge receipt/status"
            | "smuggle instructions through source"
    )
}

/// Check one adjacent-set member's coverage against the campaign artifact and this
/// file's live scenarios.
fn check_adjacent(bullet: &str, rows: &[EvidenceRow]) -> Result<(), String> {
    let covers =
        adjacent_cover(bullet).ok_or(format!("unmapped adjacent-set member: '{bullet}'"))?;
    for cover in covers {
        match cover {
            Cover::Campaign(prefix) => {
                if !rows
                    .iter()
                    .any(|row| row.id.starts_with("DX02-A") && row.axis.starts_with(prefix))
                {
                    return Err(format!(
                        "'{bullet}': no campaign attack row with axis prefix '{prefix}'"
                    ));
                }
            }
            Cover::Live(move_name) => {
                let scenario = live_scenarios()
                    .into_iter()
                    .find(|scenario| scenario.move_name == move_name)
                    .ok_or(format!("'{bullet}': no live scenario named '{move_name}'"))?;
                let outcome = run_live(&scenario);
                check_live(&scenario, &outcome)?;
            }
        }
    }
    Ok(())
}

// --- the locally duplicated production-family closure ----------------------------------------

fn decode(document: &str) -> IntentContract {
    IntentContract::decode(document.trim_end().as_bytes()).expect("the document decodes")
}

/// The R2 / fail-closed record for a field with no production classifier: byte-equal
/// canonical encodings are `unchanged`, anything else is `unknown` — never a guessed
/// direction, never silence (RFC 0031's closure rules; the same two rules the
/// campaign's harness implements, restated here so this layer stands alone).
fn fail_closed_record(field: PolicyField, group_unchanged: bool) -> ClassificationRecord {
    let relation = if group_unchanged {
        Relation::Unchanged
    } else {
        Relation::Unknown
    };
    ClassificationRecord::new(field, relation)
        .expect("`unchanged` and `unknown` are admissible on every field")
}

/// Collect a unit classifier's records for one field, supplying the R2 `unchanged`
/// record when both sides declare zero units so P1's completeness holds.
fn unit_records(
    field: PolicyField,
    produced: Vec<Result<ClassificationRecord, continuum_intent::change_policy::ChangePolicyError>>,
) -> Vec<ClassificationRecord> {
    if produced.is_empty() {
        return vec![fail_closed_record(field, true)];
    }
    produced
        .into_iter()
        .map(|record| record.expect("every produced relation is admissible on its field"))
        .collect()
}

/// Classify a whole revision through the production family: the identity shortcut
/// first; else the seven production classifiers for their fields and R2/fail-closed
/// byte comparison for the eight fields no classifier exists for. Every affirmative
/// relation comes from production code; the harness contributes only `unchanged` and
/// `unknown`. Returns whether the shortcut fired.
fn classify_live(
    before: &IntentContract,
    after: &IntentContract,
) -> (bool, Vec<ClassificationRecord>) {
    if let Some(records) = unchanged_classification(before.identity(), after.identity()) {
        return (true, records.to_vec());
    }
    let mut records = Vec::new();
    for field in PolicyField::ALL {
        match field {
            PolicyField::Properties => records.extend(unit_records(
                field,
                classify_properties(before.claims(), after.claims())
                    .iter()
                    .map(|change| change.to_classification_record())
                    .collect(),
            )),
            PolicyField::Assumptions => records.extend(unit_records(
                field,
                classify_assumptions(before.assumptions(), after.assumptions())
                    .iter()
                    .map(|change| change.to_classification_record())
                    .collect(),
            )),
            PolicyField::Observers => records.extend(unit_records(
                field,
                classify_observers(before.observers(), after.observers())
                    .iter()
                    .map(|change| change.to_classification_record())
                    .collect(),
            )),
            PolicyField::Faults => records.extend(unit_records(
                field,
                classify_faults(before.fault_model(), after.fault_model())
                    .iter()
                    .map(|change| change.to_classification_record())
                    .collect(),
            )),
            PolicyField::Fairness => records.extend(unit_records(
                field,
                classify_fairness(before.fairness(), after.fairness())
                    .iter()
                    .map(|change| change.to_classification_record())
                    .collect(),
            )),
            PolicyField::Bounds => records.push(
                classify_bounds(before.bounds(), after.bounds())
                    .to_classification_record()
                    .expect("every bounds relation is admissible on bounds"),
            ),
            PolicyField::Assurance => records.push(
                classify_assurance(before.assurance(), after.assurance())
                    .to_classification_record()
                    .expect("every assurance relation is admissible on assurance"),
            ),
            PolicyField::AbstractionMaps => records.push(fail_closed_record(
                field,
                before
                    .abstraction_maps()
                    .artifact_json()
                    .to_canonical_bytes()
                    == after
                        .abstraction_maps()
                        .artifact_json()
                        .to_canonical_bytes(),
            )),
            PolicyField::CompletionPolicy => records.push(fail_closed_record(
                field,
                before.completion_policy() == after.completion_policy(),
            )),
            PolicyField::NonVacuity => records.push(fail_closed_record(
                field,
                before
                    .optimization()
                    .non_vacuity()
                    .map(|obligation| obligation.as_str())
                    .eq(after
                        .optimization()
                        .non_vacuity()
                        .map(|obligation| obligation.as_str())),
            )),
            PolicyField::Nondeterminism => records.push(fail_closed_record(
                field,
                before.nondeterminism().artifact_json().to_canonical_bytes()
                    == after.nondeterminism().artifact_json().to_canonical_bytes(),
            )),
            PolicyField::Optimization => records.push(fail_closed_record(field, {
                let objectives = |contract: &IntentContract, role: ObjectiveRole| {
                    contract
                        .optimization()
                        .objectives(role)
                        .map(|objective| objective.as_str().to_owned())
                        .collect::<Vec<_>>()
                };
                objectives(before, ObjectiveRole::Hard) == objectives(after, ObjectiveRole::Hard)
                    && objectives(before, ObjectiveRole::Soft)
                        == objectives(after, ObjectiveRole::Soft)
            })),
            PolicyField::Scope => records.push(fail_closed_record(
                field,
                before.scope().artifact_json().to_canonical_bytes()
                    == after.scope().artifact_json().to_canonical_bytes(),
            )),
            PolicyField::SecurityPolicy => records.push(fail_closed_record(
                field,
                before
                    .security_policy()
                    .artifact_json()
                    .to_canonical_bytes()
                    == after.security_policy().artifact_json().to_canonical_bytes(),
            )),
            PolicyField::TrustBoundaries => records.push(fail_closed_record(
                field,
                before
                    .trust_boundaries()
                    .artifact_json()
                    .to_canonical_bytes()
                    == after
                        .trust_boundaries()
                        .artifact_json()
                        .to_canonical_bytes(),
            )),
        }
    }
    (false, records)
}

/// Close a classification into the **before** contract's own governance on the agent
/// path (RFC 0037 P7: the registry's current state decides; the G0 question is about
/// autonomous repair).
fn close(before: &IntentContract, records: &[ClassificationRecord]) -> Verdict {
    before
        .policy()
        .verdict(
            records,
            before.policy_reviewers(),
            AcceptancePath::AgentAccept,
        )
        .expect("a complete classification against a well-formed policy table computes")
}

// --- the live scenarios ----------------------------------------------------------------------

/// One live re-execution of the exit sentence: a canonical patch for one gaming move
/// (or the guard-repair baseline), its required classification fixed from the
/// governing texts before running.
struct Live {
    /// The dossier move this patch instantiates — the exact G0 row wording for the
    /// five G0 members; the sentence's own words for the baseline; the §5.1 move name
    /// for the beyond-G0 scenario.
    move_name: &'static str,
    /// The campaign artifact row carrying the same patch, for the row-for-row
    /// agreement test; `None` for the beyond-G0 scenario the campaign does not carry.
    campaign_row: Option<&'static str>,
    /// Whether the patch must board the production identity shortcut (the baseline)
    /// or must be refused it (every attack).
    shortcut: bool,
    /// Relations that MUST appear in the production classification.
    required: &'static [(PolicyField, Relation)],
    /// The required verdict under the fixture's own policy on the agent path.
    decision: PolicyDecision,
    /// The patch, as one bounded `.replacen` chain on the fixed document.
    mutate: fn(&str) -> String,
}

fn replace_once(document: &str, needle: &str, replacement: &str) -> String {
    let out = document.replacen(needle, replacement, 1);
    assert_ne!(
        out, document,
        "the replacement must actually fire (needle: {needle})"
    );
    out
}

/// The live corpus: one canonical patch per derived G0 move (their `move_name`s are
/// asserted equal to the dossier-derived list), the source-only guard repair, and the
/// beyond-G0 abstraction merge.
fn live_scenarios() -> Vec<Live> {
    vec![
        Live {
            move_name: "weaken property",
            campaign_row: Some("DX02-A01"),
            shortcut: false,
            required: &[(PolicyField::Properties, Relation::Weakened)],
            decision: PolicyDecision::Block,
            mutate: |document| {
                replace_once(
                    document,
                    AGREEMENT_EQ_DISJUNCT,
                    &format!("{AGREEMENT_EQ_DISJUNCT},{AGREEMENT_NE_DISJUNCT}"),
                )
            },
        },
        Live {
            move_name: "strengthen assumptions",
            campaign_row: Some("DX02-A03"),
            shortcut: false,
            required: &[(PolicyField::Assumptions, Relation::Added)],
            decision: PolicyDecision::Review,
            mutate: |document| {
                replace_once(
                    document,
                    r#"{"classification":"environment""#,
                    &format!("{OPERATOR_HONESTY},{{\"classification\":\"environment\""),
                )
            },
        },
        Live {
            move_name: "reduce bounds",
            campaign_row: Some("DX02-A05"),
            shortcut: false,
            required: &[(PolicyField::Bounds, Relation::Contracted)],
            decision: PolicyDecision::Block,
            mutate: |document| replace_once(document, r#""nodes":3"#, r#""nodes":2"#),
        },
        Live {
            move_name: "hide observer events",
            campaign_row: Some("DX02-A07"),
            shortcut: false,
            required: &[(PolicyField::Observers, Relation::Coarsened)],
            decision: PolicyDecision::Review,
            mutate: |document| {
                replace_once(document, r#""events":["Committed"]"#, r#""events":[]"#)
            },
        },
        Live {
            move_name: "remove faults",
            campaign_row: Some("DX02-A10"),
            shortcut: false,
            required: &[(PolicyField::Faults, Relation::Removed)],
            decision: PolicyDecision::Block,
            mutate: |document| replace_once(document, r#""crash","#, ""),
        },
        Live {
            move_name: "source-only guard repair",
            campaign_row: Some("DX02-C01"),
            shortcut: true,
            required: &[],
            decision: PolicyDecision::Allow,
            mutate: |document| document.to_owned(),
        },
        Live {
            move_name: "abstraction merge",
            campaign_row: None,
            shortcut: false,
            required: &[(PolicyField::AbstractionMaps, Relation::Unknown)],
            decision: PolicyDecision::Review,
            mutate: |document| {
                // §5.1's "mapping two concrete values to one abstract value": the
                // correspondence map moves. No production classifier for
                // `abstraction_maps` exists yet, so the fail-closed rule is the guard
                // — `unknown`, reviewed under the fixture's own `review` verb, never
                // allowed (the campaign's DX02-A15 boundary, exercised here on the
                // field it did not reach).
                replace_once(
                    document,
                    r#""map_id":"map_runtime_to_abstract_v1""#,
                    r#""map_id":"map_runtime_to_abstract_v2_merged""#,
                )
            },
        },
    ]
}

/// A live scenario's outcome through the production family.
struct LiveOutcome {
    shortcut: bool,
    records: Vec<ClassificationRecord>,
    verdict: Verdict,
}

/// Run one live scenario end to end.
fn run_live(scenario: &Live) -> LiveOutcome {
    let base = REPLICATED_REGISTER;
    let mutated = (scenario.mutate)(base);
    assert_eq!(
        mutated == base,
        scenario.shortcut,
        "{}: an attack must edit the contract; the baseline must not",
        scenario.move_name
    );
    let before = decode(base);
    let after = decode(&mutated);
    let expected = if scenario.shortcut {
        Equality::Unchanged
    } else {
        Equality::Distinct
    };
    assert_eq!(
        classify_equality(before.identity(), after.identity()),
        expected,
        "{}: whole-contract identity",
        scenario.move_name
    );
    let (shortcut, records) = classify_live(&before, &after);
    let verdict = close(&before, &records);
    LiveOutcome {
        shortcut,
        records,
        verdict,
    }
}

/// The exit sentence's per-patch claim, checked on one outcome. Returns `Err` rather
/// than panicking so the mutant tests can assert rejection directly.
fn check_live(scenario: &Live, outcome: &LiveOutcome) -> Result<(), String> {
    let name = scenario.move_name;
    if outcome.shortcut != scenario.shortcut {
        return Err(format!("{name}: wrong classification door"));
    }
    for (field, relation) in scenario.required {
        if !outcome
            .records
            .iter()
            .any(|record| record.field() == *field && record.relation() == *relation)
        {
            return Err(format!(
                "{name}: the classification must contain {field}:{relation}"
            ));
        }
    }
    if outcome.verdict.decision() != scenario.decision {
        return Err(format!(
            "{name}: verdict {} where {} is required",
            outcome.verdict.decision(),
            scenario.decision
        ));
    }
    if scenario.decision == PolicyDecision::Allow {
        // The negative half: fifteen `unchanged` records, one per field, no reasons.
        if outcome.records.len() != PolicyField::ALL.len() {
            return Err(format!("{name}: not exactly one record per field"));
        }
        for field in PolicyField::ALL {
            if !outcome
                .records
                .iter()
                .any(|record| record.field() == field && record.relation() == Relation::Unchanged)
            {
                return Err(format!("{name}: {field} is not `unchanged`"));
            }
        }
        if !outcome.verdict.reasons().is_empty() {
            return Err(format!(
                "{name}: an allow with reasons is not a clean allow"
            ));
        }
    } else {
        // The positive half: privileged means the P6 reasons name every required
        // record with a non-allow contribution.
        for (field, relation) in scenario.required {
            if !outcome.verdict.reasons().iter().any(|reason| {
                reason.field() == *field
                    && reason.relation() == *relation
                    && reason.contribution() > PolicyDecision::Allow
            }) {
                return Err(format!("{name}: P6 must name {field}:{relation}"));
            }
        }
    }
    Ok(())
}

/// Render an outcome the way the campaign artifact renders its rows, for the
/// row-for-row agreement test and the retained artifact.
fn render_outcome(outcome: &LiveOutcome) -> (String, String, String) {
    let changed: Vec<String> = outcome
        .records
        .iter()
        .filter(|record| record.relation() != Relation::Unchanged)
        .map(|record| format!("{}:{}", record.field(), record.relation()))
        .collect();
    let reasons: Vec<String> = outcome
        .verdict
        .reasons()
        .iter()
        .map(|reason| {
            format!(
                "{}:{}->{}",
                reason.field(),
                reason.relation(),
                reason.contribution()
            )
        })
        .collect();
    let join = |items: Vec<String>| {
        if items.is_empty() {
            "-".to_owned()
        } else {
            items.join(",")
        }
    };
    (
        join(changed),
        outcome.verdict.decision().to_string(),
        join(reasons),
    )
}

// --- the quantifier, closed ------------------------------------------------------------------

#[test]
fn the_dossier_row_yields_the_g0_set_and_the_exit_sentence() {
    // The derivation is honest: the parsed list is exactly the five moves the row
    // spells today. A dossier edit moves this pin AND the closure below together.
    let moves = g0_moves(MATRIX).expect("the G0-DX-02 row parses");
    assert_eq!(
        moves,
        vec![
            "weaken property",
            "strengthen assumptions",
            "reduce bounds",
            "hide observer events",
            "remove faults",
        ],
        "the dossier's G0 gaming-move list moved; revisit the exit's quantifier"
    );
    // The dossier row itself names the campaign as its evidence, so the artifact this
    // file closes the quantifier against is the row's own.
    let cells = g0_row_cells(MATRIX).expect("the row splits");
    assert!(
        cells
            .iter()
            .any(|cell| cell.contains("crates/continuum-semantic-diff/tests/dx02_falsification.rs")),
        "the G0-DX-02 row no longer names the campaign file as its evidence"
    );
}

#[test]
fn every_g0_gaming_move_is_covered_by_the_campaign_and_no_attack_row_closes_allow() {
    let coverage = g0_closure(MATRIX, CAMPAIGN_EVIDENCE).expect("the quantifier closes");
    assert_eq!(coverage.len(), 5, "one coverage entry per derived move");
    for (gaming_move, covering) in &coverage {
        assert!(
            !covering.is_empty(),
            "'{gaming_move}' must have covering attacks"
        );
    }
}

#[test]
fn a_new_dossier_gaming_move_fails_the_closure_loudly() {
    // The quantifier is derived, not restated: a sixth move added to the dossier row
    // fails the closure until the campaign covers it.
    let doctored = MATRIX.replacen(
        "hide observer events, remove faults",
        "hide observer events, remove faults, erase history",
        1,
    );
    assert_ne!(doctored, MATRIX, "the doctoring must fire");
    let error = g0_closure(&doctored, CAMPAIGN_EVIDENCE)
        .expect_err("an uncovered dossier move must fail the closure");
    assert!(
        error.contains("erase history"),
        "the failure must name the uncovered move: {error}"
    );
}

#[test]
fn a_campaign_artifact_with_a_flipped_attack_verdict_is_rejected() {
    let real = "DX02-A01 | weaken-property-complementary-disjunct | replicated-register | \
                field-by-field | properties:weakened | block | properties:weakened->block";
    assert!(CAMPAIGN_EVIDENCE.contains(real), "the A01 row moved");
    let doctored = CAMPAIGN_EVIDENCE.replacen(
        real,
        "DX02-A01 | weaken-property-complementary-disjunct | replicated-register | \
         field-by-field | properties:weakened | allow | properties:weakened->block",
        1,
    );
    let error = g0_closure(MATRIX, &doctored).expect_err("a landed attack must be rejected");
    assert!(
        error.contains("DX02-A01"),
        "the failure names the row: {error}"
    );
}

#[test]
fn a_campaign_artifact_with_a_blanked_classification_is_rejected() {
    let real = "DX02-A10 | remove-faults | replicated-register | field-by-field | \
                faults:removed | block | faults:removed->block";
    assert!(CAMPAIGN_EVIDENCE.contains(real), "the A10 row moved");
    let doctored = CAMPAIGN_EVIDENCE.replacen(
        real,
        "DX02-A10 | remove-faults | replicated-register | field-by-field | - | block | \
         faults:removed->block",
        1,
    );
    let error = g0_closure(MATRIX, &doctored).expect_err("an unclassified attack must be rejected");
    assert!(
        error.contains("DX02-A10"),
        "the failure names the row: {error}"
    );
}

#[test]
fn a_campaign_artifact_with_a_flagged_guard_repair_is_rejected() {
    let real = "DX02-C01 | guard-repair-baseline | replicated-register | shortcut | - | allow | -";
    assert!(CAMPAIGN_EVIDENCE.contains(real), "the C01 row moved");
    let doctored = CAMPAIGN_EVIDENCE.replacen(
        real,
        "DX02-C01 | guard-repair-baseline | replicated-register | shortcut | \
         properties:unknown | review | properties:unknown->review",
        1,
    );
    let error = g0_closure(MATRIX, &doctored).expect_err("a flagged guard repair must be rejected");
    assert!(
        error.contains("DX02-C01"),
        "the failure names the row: {error}"
    );
}

// --- the sentence live -----------------------------------------------------------------------

/// Run and check the live scenario for one move name.
fn run_and_check(move_name: &str) -> LiveOutcome {
    let scenario = live_scenarios()
        .into_iter()
        .find(|scenario| scenario.move_name == move_name)
        .unwrap_or_else(|| panic!("no live scenario named {move_name}"));
    let outcome = run_live(&scenario);
    if let Err(error) = check_live(&scenario, &outcome) {
        panic!("{error}");
    }
    outcome
}

#[test]
fn live_weakening_a_property_is_a_privileged_change() {
    run_and_check("weaken property");
}

#[test]
fn live_strengthening_assumptions_is_a_privileged_change() {
    run_and_check("strengthen assumptions");
}

#[test]
fn live_reducing_bounds_is_a_privileged_change() {
    run_and_check("reduce bounds");
}

#[test]
fn live_hiding_observer_events_is_a_privileged_change() {
    run_and_check("hide observer events");
}

#[test]
fn live_removing_faults_is_a_privileged_change() {
    run_and_check("remove faults");
}

#[test]
fn live_the_five_g0_attack_scenarios_are_exactly_the_derived_dossier_moves() {
    // The live corpus's attack names are not free-floating: they are, member for
    // member, the dossier-derived list — so the live half and the quantifier cannot
    // quantify over different sets.
    let derived = g0_moves(MATRIX).expect("the G0-DX-02 row parses");
    let live: Vec<&str> = live_scenarios()
        .iter()
        .filter(|scenario| !scenario.shortcut && scenario.campaign_row.is_some())
        .map(|scenario| scenario.move_name)
        .collect();
    assert_eq!(
        derived, live,
        "live G0 scenarios out of step with the dossier"
    );
}

#[test]
fn live_a_source_only_guard_repair_is_not_a_privileged_change() {
    // The negative half: a genuine source patch, zero contract bytes, the same
    // closure, the same fixture, the same agent path — and a clean allow through the
    // production identity shortcut.
    assert_ne!(
        GUARD_SOURCE_BEFORE, GUARD_SOURCE_AFTER,
        "the repair must be a real patch"
    );
    let outcome = run_and_check("source-only guard repair");
    assert!(outcome.shortcut, "the repair boards the identity shortcut");
    assert_eq!(outcome.records.len(), 15);
    assert_eq!(outcome.verdict.decision(), PolicyDecision::Allow);
    assert!(outcome.verdict.reasons().is_empty());
}

#[test]
fn live_beyond_g0_an_abstraction_merge_fails_closed_to_review() {
    // §5.1's sixth move, the one gaming bullet the campaign carries no scenario for.
    // Not a member of the G0 set; covered here so the §5.1 enumeration closes without
    // overclaiming: no `abstraction_maps` classifier exists, and the fail-closed rule
    // is the load-bearing guard (`unknown`/`review`), exactly as the campaign's
    // DX02-A15 boundary records for the other unclassified fields.
    run_and_check("abstraction merge");
}

#[test]
fn the_live_outcomes_agree_with_the_campaign_artifact_row_for_row() {
    // INV-004 both ways: the independent re-execution and the retained campaign
    // artifact must tell the same story for the same patch, byte for byte in the
    // artifact's own rendering.
    let rows = parse_evidence(CAMPAIGN_EVIDENCE).expect("the campaign artifact parses");
    let mut compared = 0;
    for scenario in live_scenarios() {
        let Some(row_id) = scenario.campaign_row else {
            continue;
        };
        let row = rows
            .iter()
            .find(|row| row.id == row_id)
            .unwrap_or_else(|| panic!("campaign artifact has no row {row_id}"));
        let outcome = run_live(&scenario);
        let (records, decision, reasons) = render_outcome(&outcome);
        assert_eq!(records, row.records, "{row_id}: records diverge");
        assert_eq!(decision, row.decision, "{row_id}: decision diverges");
        assert_eq!(reasons, row.reasons, "{row_id}: reasons diverge");
        compared += 1;
    }
    assert_eq!(compared, 6, "six live scenarios have campaign twins");
}

// --- anti-vacuity: the exit assertion can fail -----------------------------------------------

#[test]
fn mutant_a_blind_property_classifier_flips_the_exit_and_is_rejected() {
    // A classifier blind to the weakening reads the attack as fifteen `unchanged` and
    // the closure allows it — and this file's own check refuses that outcome, so a
    // green run is a claim about the classifiers, not about a check that cannot say
    // no.
    let scenario = live_scenarios()
        .into_iter()
        .find(|scenario| scenario.move_name == "weaken property")
        .expect("the scenario exists");
    let real = run_live(&scenario);
    assert_eq!(real.verdict.decision(), PolicyDecision::Block);

    let blinded: Vec<ClassificationRecord> = real
        .records
        .iter()
        .map(|record| {
            if record.field() == PolicyField::Properties && record.relation() == Relation::Weakened
            {
                ClassificationRecord::new(PolicyField::Properties, Relation::Unchanged)
                    .expect("unchanged is admissible everywhere")
            } else {
                *record
            }
        })
        .collect();
    let before = decode(REPLICATED_REGISTER);
    let mutant = LiveOutcome {
        shortcut: false,
        verdict: close(&before, &blinded),
        records: blinded,
    };
    assert_eq!(
        mutant.verdict.decision(),
        PolicyDecision::Allow,
        "the mutant must actually flip the verdict, or it is not exercising the exit"
    );
    let error = check_live(&scenario, &mutant).expect_err("the check must reject the mutant");
    assert!(
        error.contains("properties:weakened"),
        "the rejection names the missing record: {error}"
    );
}

#[test]
fn mutant_a_flag_everything_closure_flips_the_negative_half_and_is_rejected() {
    // The negative half is not vacuous either: a closure that records `unknown` on an
    // untouched field turns the ordinary repair into a reviewed change — a family that
    // flags everything detects nothing — and the check refuses that outcome too.
    let scenario = live_scenarios()
        .into_iter()
        .find(|scenario| scenario.move_name == "source-only guard repair")
        .expect("the scenario exists");
    let real = run_live(&scenario);
    assert_eq!(real.verdict.decision(), PolicyDecision::Allow);

    let paranoid: Vec<ClassificationRecord> = real
        .records
        .iter()
        .map(|record| {
            if record.field() == PolicyField::Properties {
                ClassificationRecord::new(PolicyField::Properties, Relation::Unknown)
                    .expect("unknown is admissible everywhere")
            } else {
                *record
            }
        })
        .collect();
    let before = decode(REPLICATED_REGISTER);
    let mutant = LiveOutcome {
        shortcut: true,
        verdict: close(&before, &paranoid),
        records: paranoid,
    };
    assert_ne!(
        mutant.verdict.decision(),
        PolicyDecision::Allow,
        "the mutant must actually refuse the repair, or it is not exercising the half"
    );
    check_live(&scenario, &mutant).expect_err("the check must reject the mutant");
}

// --- the adjacent sets, closed by enumeration ------------------------------------------------

#[test]
fn docs50_intent_attack_classes_are_covered_by_enumeration() {
    let attacks = docs50_intent_attacks();
    assert_eq!(
        attacks,
        vec![
            "weaken property",
            "strengthen assumptions/fairness",
            "shrink bounds",
            "remove faults",
            "coarsen observer",
            "lower assurance",
        ],
        "docs/50's Intent attacks class moved; revisit the adjacent-set coverage"
    );
    let rows = parse_evidence(CAMPAIGN_EVIDENCE).expect("the campaign artifact parses");
    for attack in &attacks {
        check_adjacent(attack, &rows).expect("every docs/50 intent attack is covered");
    }
}

#[test]
fn plan_5_1_gaming_moves_are_covered_by_enumeration() {
    let moves = plan_51_moves();
    assert_eq!(
        moves,
        vec![
            "changing `Agreement` to a weaker observer",
            "adding a fairness assumption that schedules away the bug",
            "reducing node count from five to three",
            "removing crash-after-submit",
            "marking a storage effect opaque",
            "mapping two concrete values to one abstract value",
            "lowering assurance from exhaustive to sampled",
            "excluding the failing state with a constraint",
        ],
        "plan §5.1's gaming list moved; revisit the adjacent-set coverage"
    );
    let rows = parse_evidence(CAMPAIGN_EVIDENCE).expect("the campaign artifact parses");
    for gaming_move in &moves {
        check_adjacent(gaming_move, &rows).expect("every §5.1 gaming move is covered");
    }
}

#[test]
fn plan_19_5_suite_partitions_into_covered_patches_and_non_patch_attacks() {
    // §19.5 is ContinuumBench's G9 reward-hacking suite, not the G0 set. Its
    // Intent-Contract-patch members must be covered; its other members are not
    // patches to an Intent Contract and fall outside this exit's quantifier, owned by
    // docs/50's other attack classes and their gates. The partition is total and
    // exclusive, so a new §19.5 bullet must be placed deliberately.
    let suite = plan_195_moves();
    assert_eq!(
        suite.len(),
        15,
        "plan §19.5's suite moved; revisit the partition"
    );
    let rows = parse_evidence(CAMPAIGN_EVIDENCE).expect("the campaign artifact parses");
    let mut covered = 0;
    let mut outside = 0;
    for bullet in &suite {
        let is_patch = adjacent_cover(bullet).is_some();
        let is_outside = outside_quantifier(bullet);
        assert!(
            is_patch != is_outside,
            "'{bullet}' must be exactly one of covered-patch or outside-quantifier"
        );
        if is_patch {
            check_adjacent(bullet, &rows).expect("every §19.5 patch member is covered");
            covered += 1;
        } else {
            outside += 1;
        }
    }
    assert_eq!((covered, outside), (7, 8), "the §19.5 partition moved");
}

// --- the retained artifact -------------------------------------------------------------------

/// Render the exit evidence: the dossier-derived quantifier and its campaign coverage,
/// every live re-execution, and the adjacent-set enumeration. Deterministic by
/// construction: every line derives from the pinned texts and the production
/// classifiers' `BTree`-ordered output.
fn render_exit_evidence() -> String {
    let mut out = String::new();
    out.push_str(
        "# PR-12 exit evidence — all G0 intent-gaming patches are privileged changes; a \
         source-only guard repair is not\n",
    );
    out.push_str("# crates/continuum-semantic-diff/tests/pr12_exit_evidence.rs (bn-3jtr)\n");
    out.push_str(
        "# section 1 — the quantifier: G0_SPIKE_MATRIX.md row G0-DX-02, closed over the \
         campaign artifact (bn-3cgt)\n",
    );
    let coverage = g0_closure(MATRIX, CAMPAIGN_EVIDENCE).expect("the quantifier closes");
    for (gaming_move, covering) in &coverage {
        let rendered: Vec<String> = covering
            .iter()
            .map(|(id, decision)| format!("{id}:{decision}"))
            .collect();
        out.push_str(&format!("g0 | {gaming_move} | {}\n", rendered.join(",")));
    }
    out.push_str(
        "# section 2 — the sentence live: production classifiers, before-table verdict, \
         AgentAccept\n",
    );
    for scenario in live_scenarios() {
        let outcome = run_live(&scenario);
        check_live(&scenario, &outcome).expect("every live scenario holds");
        let (records, decision, reasons) = render_outcome(&outcome);
        out.push_str(&format!(
            "live | {} | {} | {records} | {decision} | {reasons}\n",
            scenario.move_name,
            if outcome.shortcut {
                "shortcut"
            } else {
                "field-by-field"
            },
        ));
    }
    out.push_str("# section 3 — adjacent attack sets, closed by enumeration\n");
    out.push_str(&format!(
        "adjacent | docs/50 intent attacks | {} members | all covered\n",
        docs50_intent_attacks().len()
    ));
    out.push_str(&format!(
        "adjacent | plan 5.1 gaming moves | {} members | all covered (opaque-effect and \
         abstraction-merge at fail-closed grain)\n",
        plan_51_moves().len()
    ));
    out.push_str(&format!(
        "adjacent | plan 19.5 reward-hacking suite | {} members | 7 intent-contract-patch \
         members covered; 8 members are not Intent Contract patches and fall outside \
         PR-12-EXIT's quantifier\n",
        plan_195_moves().len()
    ));
    out
}

#[test]
fn the_exit_evidence_artifact_is_byte_stable_and_matches_the_golden() {
    let actual = render_exit_evidence();
    if std::env::var_os("PR12_EXIT_BLESS").is_some() {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/golden/pr12_exit_evidence.txt"
        );
        std::fs::write(path, &actual).expect("the golden is writable");
        panic!(
            "PR12_EXIT_BLESS rewrote {path}; rerun without the variable and review the diff \
             as a contract change"
        );
    }
    assert_eq!(
        actual, EXIT_GOLDEN,
        "the exit evidence artifact drifted; to regenerate deliberately, run with \
         PR12_EXIT_BLESS=1 and review the diff as a contract change.\nactual:\n{actual}"
    );
    // Rendered twice, identical bytes: no ambient anything (INV-005).
    assert_eq!(actual, render_exit_evidence());
}
