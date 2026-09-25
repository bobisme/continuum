//! Dedicated evidence map for `INV-001` — Protected intent (`notes/plan/plan.md:308`,
//! the invariant bone `bn-34je`).
//!
//! > Ordinary tasks may not mutate intent. Intent changes require a new Intent
//! > Contract, semantic diff, policy decision, and invalidation of dependent evidence.
//! >
//! > — `notes/plan/plan.md`, INV-001's contract sentence
//!
//! One scoping note, recorded because the two spellings are easy to conflate: the
//! sentence above is the authoritative INV-001. The six-verb spelling — "weakening a
//! property, strengthening an assumption, shrinking a bound, hiding an event, removing
//! a fault, or downgrading assurance is a privileged intent revision with a semantic
//! diff, never a repair" — is `AGENTS.md`'s joint summary of INV-001 *and* INV-011,
//! and it is the operational grain the enforcement surface is actually built at, so
//! this file's verb map is keyed to it and anti-drift-checked against it
//! ([`verb_map`]). Both texts are pinned; neither contradicts the other.
//!
//! # The verb map: each verb → its live enforcement sites
//!
//! | INV-001 verb | field | dangerous relation | classifier (production) | policy site (this crate) | campaign rows |
//! |---|---|---|---|---|---|
//! | weakening a property | `properties` | `weakened` | `continuum-semantic-diff/src/properties.rs` (bounded implication oracle) | `locked` denies every non-`unchanged` — `block` | DX02-A01/A02/A16/A17 |
//! | strengthening an assumption | `assumptions` | `strengthened` | `continuum-semantic-diff/src/assumptions.rs` (no Finite inclusion oracle yet: in-place strengthening fails closed to `unknown`; membership is affirmative) | `review` + named reviewer — `review` | DX02-A03/A04/A18 |
//! | shrinking a bound | `bounds` | `contracted` | `continuum-semantic-diff/src/bounds.rs` (componentwise order) | `no-decrease` denies `contracted` — `block` | DX02-A05/A06/A19 |
//! | hiding an event | `observers` | `coarsened` | `continuum-semantic-diff/src/observers.rs` ("hiding is never benign") | `review` + named reviewer — `review` | DX02-A07/A08/A09 |
//! | removing a fault | `faults` | `removed` | `continuum-semantic-diff/src/faults.rs` (set membership) | `no-removal` denies `removed` — `block` | DX02-A10 |
//! | downgrading assurance | `assurance` | `downgraded` | `continuum-semantic-diff/src/assurance.rs` (total order, correction-13 carve-out) | `no-downgrade` denies `downgraded` — `block` | DX02-A11 |
//!
//! The policy column is the replicated-register corpus fixture's *own* policy table,
//! and [`policy_decision`] closes each row into
//! [`PolicyTable::verdict`](continuum_intent::change_policy::PolicyTable::verdict)
//! live — the same production function the G0-DX-02 campaign closed its attacks into.
//! "Never a repair" holds under *every* acceptance path (none of the six ever closes
//! `allow`), and even a field governed `unlocked` is not outside INV-001: RFC 0037
//! correction 11 (quoted at `PolicyTable::all_unlocked`) keeps all fifteen fields
//! classified, in `intent_changes`, and behind `revise-intent`.
//!
//! # The four required elements, sited honestly
//!
//! - **"a new Intent Contract"** — live in this crate: the `in_*` identity *is* the
//!   ID1/ID2 canonical preimage (ADR-0013), the policy table is *in* the preimage
//!   (ID2), contracts are immutable (no setter anywhere), and the daemon's one
//!   governance write (`intent.lock`) mints a successor rather than editing (ID3).
//!   [`new_intent_contract`] proves the identity moves for a protected-field change
//!   and for a governance edit, and does not move for ID2 metadata.
//! - **"semantic diff"** — live at classifier grain in `continuum-semantic-diff`: the
//!   complete eight-module family, attacked at production grain by the G0-DX-02
//!   falsification campaign (all six verbs, nineteen attacks, all caught) and closed
//!   under its quantifier by the PR-12 exit evidence. Those two suites ARE the
//!   INV-001 classifier evidence; [`cited_campaigns`] cites them through freshness
//!   tripwires (the bn-1eqt device) rather than re-running them. The *wire* lane
//!   (`intent.diff` / `intent.propose_revision` returning a `diff_*` artifact) has
//!   not shipped and refuses with typed `UnsupportedSemanticFeature` rather than
//!   guessing — [`daemon_boundary`] pins the refusal.
//! - **"policy decision"** — live in this crate: `PolicyTable::verdict` (P1–P6),
//!   `PolicyField::admits_relation` including RFC 0031 correction 13's `assurance` ×
//!   `incomparable` carve-out (bn-2sngz), and the W1–W7 well-formedness rules.
//!   `tests/change_policy_contract.rs` already proves P1–P6 clause by clause and
//!   every plan-§5.1 gaming move; none of that is re-proved here — this file binds
//!   the six INV-001 verbs specifically to it.
//! - **"invalidation of dependent evidence"** — discharged today **by identity**, not
//!   by a walk: evidence names its governing intent by content identity (plan §4.2,
//!   `daemon/task.rs`: "The governing intent, by identity only"), a revision mints a
//!   new `in_*`, and nothing re-serves old evidence under the new identity (the
//!   verification cache keys on "the same snapshot, intent, target, and epochs").
//!   The *explicit* lane — plan §4.2's "produces a semantic intent diff and
//!   invalidates dependent evidence" — has a typed edge vocabulary
//!   (`continuum-evidence`'s `INVALIDATES`) and **no producer**:
//!   [`dependent_evidence`] pins the absence so it goes red when the producer lands.
//!
//! # The privilege half: WHO may revise
//!
//! Live at the daemon boundary, and cited rather than re-dispatched (this crate does
//! not depend on `continuumd`; every citation is a source-text tripwire that fails
//! loudly on drift):
//!
//! - `intent.accept`, `intent.reject`, `intent.lock` are `@privileged
//!   @audit_recorded` in the protocol registry; `intent.propose_revision` sits at
//!   `authority revise-intent` and is deliberately *not* `@privileged` — docs/49's
//!   "revise intent: proposal only" capability-matrix cell realized as the
//!   propose/accept split (RFC 0027), pinned in [`daemon_boundary`] together with
//!   docs/49's own table row.
//! - T3 (privilege) is decided in `daemon/admission.rs` from
//!   `CapabilityDescriptor.profile.privileged_operations` before any handler runs.
//! - `intent.lock` refuses a policy edit that weakens any field's verb (the lattice's
//!   own order), and `intent.propose_revision` refuses a change set naming a `locked`
//!   field — both live, both pinned.
//! - D1's first-intent-wins convergence (the DX-03 fix wave, bn-n1xou): a re-create
//!   cannot re-govern an existing snapshot, and no operation un-seals — re-governing
//!   would be an intent change without a revision, which is exactly this invariant.
//!
//! # Honest gaps, typed as absences (the INV-008 spirit), each pinned to go red
//!
//! 1. **The daemon propose lane.** `intent.propose_revision` has no propose logic:
//!    after the live locked-field check it closes to `UnsupportedSemanticFeature`.
//!    bn-1604 recorded a rationale-recheck owed "once that operation's real logic
//!    lands"; verified current state: `daemon/intent.rs` still never reads
//!    `rationale` (the wire declares it; the handler contains no such token), so the
//!    recheck is *not yet due* — the pin here fires the moment it becomes due.
//! 2. **P7 accept-time recomputation.** `intent.accept` performs no verdict
//!    recomputation — and no wire path can exploit that, because no operation mints
//!    a `Proposed` record (`put_intent` is reachable only from `intent.lock`'s
//!    successor write; `propose_revision` refuses before minting). The absence is
//!    pinned by a source sweep: no `continuumd` source names `AcceptancePath`.
//! 3. **The explicit invalidation producer** (element four above).
//! 4. **INV-011 reclassification at repair promotion.** `continuum-repair` is a
//!    PR-1/IMPL-01 scaffold with no public items; pinned.
//! 5. **The assumptions inclusion oracle.** In-place strengthening classifies
//!    `unknown` (fail-closed, `review`, never `allow`) until the Finite oracle
//!    reaches `assumptions` — recorded in that module's own doc, cited via DX02-A04.
//!
//! # Mutant story (anti-vacuity)
//!
//! - [`verb_map::negative_the_sentence_extraction_is_not_vacuous`] — dropping
//!   "removing a fault" from a copy of the AGENTS.md sentence yields five phrases and
//!   the comparator reports the mismatch; an unrelated sentence extracts nothing.
//! - [`policy_decision::negative_a_blind_classifier_reading_an_attack_as_unchanged_is_detected`]
//!   — swapping any row's dangerous relation for `unchanged` closes `allow` with zero
//!   reasons, and this file's own never-a-repair predicate rejects that outcome: the
//!   *relation* carries the catch, never the harness.
//! - [`cited_campaigns::negative_the_campaign_tripwires_are_not_vacuous`] and
//!   [`daemon_boundary::negative_the_daemon_tripwires_are_not_vacuous`] — every
//!   citation predicate is re-run against a doctored copy with the load-bearing line
//!   stripped (or a forbidden token planted) and must report the defect.
//!
//! House rules: `src/` untouched, no existing test edited, no new dependency edge
//! (cross-crate citations are `include_str!`/`fs` over tracked files — the
//! inv008/inv014 device), deterministic (fixed inputs, no clock, no entropy, no
//! network).

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use continuum_intent::change_policy::{
    AcceptancePath, ClassificationRecord, PolicyDecision, PolicyField, PolicyTable, PolicyVerb,
    Relation, Verdict,
};
use continuum_intent::contract::{ContractParts, IntentContract};

// --- the cited texts, all tracked in this repository ---------------------------------------

const AGENTS_MD: &str = include_str!("../../../AGENTS.md");
const PLAN_MD: &str = include_str!("../../../notes/plan/plan.md");
const DOCS_49: &str = include_str!("../../../notes/plan/docs/49_SECURITY_FOR_AUTONOMOUS_AGENTS.md");
const RR_FIXTURE: &str = include_str!("fixtures/replicated-register-contract.json");
const DAEMON_INTENT: &str = include_str!("../../continuumd/src/daemon/intent.rs");
const DAEMON_ADMISSION: &str = include_str!("../../continuumd/src/daemon/admission.rs");
const DAEMON_WORKSPACE: &str = include_str!("../../continuumd/src/daemon/workspace.rs");
const DAEMON_TASK: &str = include_str!("../../continuumd/src/daemon/task.rs");
const PROTOCOL_REGISTRY: &str = include_str!("../../continuumd/src/protocol/registry.rs");
const PROTOCOL_SHARED: &str = include_str!("../../continuumd/src/protocol/shared.rs");
const DX02_CAMPAIGN: &str =
    include_str!("../../continuum-semantic-diff/tests/dx02_falsification.rs");
const DX02_GOLDEN: &str =
    include_str!("../../continuum-semantic-diff/tests/golden/dx02_falsification_evidence.txt");
const PR12_EXIT: &str = include_str!("../../continuum-semantic-diff/tests/pr12_exit_evidence.rs");
const PR12_GOLDEN: &str =
    include_str!("../../continuum-semantic-diff/tests/golden/pr12_exit_evidence.txt");
const EVIDENCE_EDGE: &str = include_str!("../../continuum-evidence/src/edge.rs");
const REPAIR_LIB: &str = include_str!("../../continuum-repair/src/lib.rs");
const REPAIR_TRANSACTION: &str = include_str!("../../continuum-repair/src/transaction.rs");

// --- the verb map --------------------------------------------------------------------------

/// One INV-001 verb, mapped to its live enforcement sites.
struct VerbRow {
    /// The verb phrase exactly as AGENTS.md's INV-001/INV-011 sentence spells it.
    phrase: &'static str,
    /// The RFC 0037 policy field the verb attacks.
    field: PolicyField,
    /// The RFC 0031 relation the attack classifies to.
    dangerous: Relation,
    /// The verb the replicated-register fixture's own policy table governs the field
    /// with.
    governing_verb: PolicyVerb,
    /// The required (never-`allow`) decision when the dangerous relation closes into
    /// the fixture's own table — the same outcome the campaign's table requires.
    decision: PolicyDecision,
    /// The G0-DX-02 campaign rows that attacked this verb at production grain.
    campaign_rows: &'static [&'static str],
    /// The production classifier module (a file name in `continuum-semantic-diff/src`).
    classifier: &'static str,
}

const MAP: [VerbRow; 6] = [
    VerbRow {
        phrase: "weakening a property",
        field: PolicyField::Properties,
        dangerous: Relation::Weakened,
        governing_verb: PolicyVerb::Locked,
        decision: PolicyDecision::Block,
        campaign_rows: &["DX02-A01", "DX02-A02", "DX02-A16", "DX02-A17"],
        classifier: "properties.rs",
    },
    VerbRow {
        phrase: "strengthening an assumption",
        field: PolicyField::Assumptions,
        dangerous: Relation::Strengthened,
        governing_verb: PolicyVerb::Review,
        decision: PolicyDecision::Review,
        campaign_rows: &["DX02-A03", "DX02-A04", "DX02-A18"],
        classifier: "assumptions.rs",
    },
    VerbRow {
        phrase: "shrinking a bound",
        field: PolicyField::Bounds,
        dangerous: Relation::Contracted,
        governing_verb: PolicyVerb::NoDecrease,
        decision: PolicyDecision::Block,
        campaign_rows: &["DX02-A05", "DX02-A06", "DX02-A19"],
        classifier: "bounds.rs",
    },
    VerbRow {
        phrase: "hiding an event",
        field: PolicyField::Observers,
        dangerous: Relation::Coarsened,
        governing_verb: PolicyVerb::Review,
        decision: PolicyDecision::Review,
        campaign_rows: &["DX02-A07", "DX02-A08", "DX02-A09"],
        classifier: "observers.rs",
    },
    VerbRow {
        phrase: "removing a fault",
        field: PolicyField::Faults,
        dangerous: Relation::Removed,
        governing_verb: PolicyVerb::NoRemoval,
        decision: PolicyDecision::Block,
        campaign_rows: &["DX02-A10"],
        classifier: "faults.rs",
    },
    VerbRow {
        phrase: "downgrading assurance",
        field: PolicyField::Assurance,
        dangerous: Relation::Downgraded,
        governing_verb: PolicyVerb::NoDowngrade,
        decision: PolicyDecision::Block,
        campaign_rows: &["DX02-A11"],
        classifier: "assurance.rs",
    },
];

// --- shared helpers ------------------------------------------------------------------------

/// Whitespace-normalized text, so a reflowed paragraph does not defeat a pin.
fn normalized(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// The six verb phrases of AGENTS.md's INV-001/INV-011 sentence, in order.
///
/// `None` when the sentence's frame is not present at all — a rewritten summary must
/// be re-adjudicated, not silently re-parsed.
fn agents_md_verbs(text: &str) -> Option<Vec<String>> {
    let flat = normalized(text);
    let start = "**Protected intent** (INV-001, INV-011) — ";
    let end = " is a privileged intent revision with a semantic diff, never a repair.";
    let begin = flat.find(start)? + start.len();
    let stop = flat[begin..].find(end)? + begin;
    let mut verbs = Vec::new();
    for piece in flat[begin..stop].split(", ") {
        let piece = piece.strip_prefix("or ").unwrap_or(piece);
        if piece.is_empty() {
            return None;
        }
        verbs.push(piece.to_owned());
    }
    Some(verbs)
}

/// The replicated-register corpus fixture, through the front door.
fn fixture() -> IntentContract {
    IntentContract::decode(RR_FIXTURE.as_bytes())
        .expect("the tracked corpus fixture decodes as a whole contract")
}

/// A complete fifteen-record classification: `relation` on `field`, `unchanged`
/// everywhere else — the R2 shape the campaign's harness also produces.
fn records(field: PolicyField, relation: Relation) -> Vec<ClassificationRecord> {
    PolicyField::ALL
        .into_iter()
        .map(|each| {
            let recorded = if each == field {
                relation
            } else {
                Relation::Unchanged
            };
            ClassificationRecord::new(each, recorded)
                .expect("every map row's relation is admissible on its field")
        })
        .collect()
}

/// This file's own reading of "never a repair": the decision forbids `allow`.
fn never_a_repair(verdict: &Verdict) -> bool {
    verdict.decision() != PolicyDecision::Allow
}

/// The campaign rows a text fails to carry, empty when the citation is fresh.
fn missing_campaign_rows(text: &str) -> Vec<&'static str> {
    MAP.iter()
        .flat_map(|row| row.campaign_rows.iter().copied())
        .filter(|id| !text.contains(id))
        .collect()
}

/// The registry source for one operation: from its `name:` line to the next one.
fn spec_block<'a>(source: &'a str, operation: &str) -> &'a str {
    let needle = format!("name: \"{operation}\"");
    let begin = source
        .find(&needle)
        .unwrap_or_else(|| panic!("the protocol registry declares {operation}"));
    let rest = &source[begin + needle.len()..];
    let stop = rest.find("name: \"").unwrap_or(rest.len());
    &rest[..stop]
}

/// Every `.rs` source under `continuumd/src`, as (path relative to `src`, text).
fn continuumd_sources() -> Vec<(String, String)> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../continuumd/src");
    let mut files = Vec::new();
    let mut pending = vec![root.clone()];
    while let Some(directory) = pending.pop() {
        for entry in std::fs::read_dir(&directory).expect("continuumd/src is a tracked directory") {
            let path = entry.expect("a readable directory entry").path();
            if path.is_dir() {
                pending.push(path);
            } else if path.extension().is_some_and(|extension| extension == "rs") {
                let relative = path
                    .strip_prefix(&root)
                    .expect("every walked file is under the root")
                    .to_string_lossy()
                    .into_owned();
                let text = std::fs::read_to_string(&path).expect("a readable Rust source");
                files.push((relative, text));
            }
        }
    }
    files.sort();
    files
}

/// The sources (by relative path) whose text contains `token`.
fn sources_containing(sources: &[(String, String)], token: &str) -> Vec<String> {
    sources
        .iter()
        .filter(|(_, text)| text.contains(token))
        .map(|(path, _)| path.clone())
        .collect()
}

/// The live pins on the daemon's propose/lock lane, as one reusable predicate so the
/// mutant below can prove it has teeth.
fn propose_lane_pins(intent_source: &str) -> bool {
    intent_source.contains("the change set names a field whose change policy is `locked`")
        && intent_source.contains("the policy edit weakens a field's change policy")
        && intent_source.contains("is_weaker_or_equal")
        && intent_source.contains("the RFC 0031 classification lane")
        && intent_source.contains("mints a *successor* contract")
        && intent_source.contains("RegistryStatus::Superseded")
}

// --- the sentence, and the map's own admissibility -----------------------------------------

mod verb_map {
    use super::*;

    #[test]
    fn positive_the_six_verb_map_is_agents_mds_inv001_sentence_in_order() {
        let verbs = agents_md_verbs(AGENTS_MD)
            .expect("AGENTS.md still carries the INV-001/INV-011 summary sentence");
        assert_eq!(verbs.len(), MAP.len(), "six verbs, exactly");
        for (from_sentence, row) in verbs.iter().zip(MAP.iter()) {
            assert_eq!(
                from_sentence, row.phrase,
                "the map row order is the sentence's order"
            );
        }
        // The authoritative sentence is plan §2's, and it is pinned separately: the
        // summary is the operational grain, not a replacement.
        assert!(PLAN_MD.contains("### INV-001 — Protected intent"));
        assert!(PLAN_MD.contains(
            "Ordinary tasks may not mutate intent. Intent changes require a new Intent \
             Contract, semantic diff, policy decision, and invalidation of dependent evidence."
        ));
    }

    #[test]
    fn positive_every_row_is_admissible_in_the_crates_own_vocabulary() {
        for row in &MAP {
            assert!(
                row.field.admits_relation(row.dangerous),
                "{}: the dangerous relation is one the field's order can produce",
                row.phrase
            );
            assert!(
                row.governing_verb.is_admissible_on(row.field),
                "{}: the fixture's verb is W6-admissible on the field",
                row.phrase
            );
            if row.decision == PolicyDecision::Block {
                assert!(
                    row.governing_verb.denies(row.dangerous)
                        || row.governing_verb == PolicyVerb::Locked,
                    "{}: a block row is carried by the verb's own denial set",
                    row.phrase
                );
            }
        }
    }

    #[test]
    fn negative_the_sentence_extraction_is_not_vacuous() {
        // Dropping one verb must be *seen*: five phrases, not six.
        let doctored = AGENTS_MD.replacen("removing a fault, or ", "", 1);
        assert_ne!(doctored, AGENTS_MD, "the mutation touched the sentence");
        let verbs = agents_md_verbs(&doctored).expect("the frame is still present");
        assert_eq!(verbs.len(), MAP.len() - 1, "the dropped verb is missing");
        assert_ne!(verbs.len(), MAP.len(), "so the comparison above would fail");
        // An unrelated sentence extracts nothing rather than something.
        assert_eq!(agents_md_verbs("no such sentence here"), None);
    }
}

// --- the policy decision, live -------------------------------------------------------------

mod policy_decision {
    use super::*;

    #[test]
    fn positive_each_of_the_six_verbs_forbids_allow_under_the_fixtures_own_policy() {
        let contract = fixture();
        for row in &MAP {
            assert_eq!(
                contract.policy().verb(row.field),
                row.governing_verb,
                "{}: the map's verb column is the fixture's own table",
                row.phrase
            );
            let verdict = contract
                .policy()
                .verdict(
                    &records(row.field, row.dangerous),
                    contract.policy_reviewers(),
                    AcceptancePath::AgentAccept,
                )
                .expect("a complete, admissible classification reaches a verdict");
            assert!(never_a_repair(&verdict), "{}: never a repair", row.phrase);
            assert_eq!(verdict.decision(), row.decision, "{}", row.phrase);
            // P6: the one record that forbade `allow` is named, exactly.
            assert_eq!(verdict.reasons().len(), 1, "{}", row.phrase);
            let reason = &verdict.reasons()[0];
            assert_eq!(reason.field(), row.field);
            assert_eq!(reason.relation(), row.dangerous);
            assert_eq!(reason.verb(), row.governing_verb);
            if row.governing_verb == PolicyVerb::Review {
                assert!(
                    reason.reviewers().contains("verification-lead"),
                    "{}: the fixture names its reviewer and P6 carries it",
                    row.phrase
                );
            }
        }
    }

    #[test]
    fn positive_no_acceptance_path_reads_any_of_the_six_as_a_repair() {
        // The fixture governs none of the six fields `proposal-only`, so a human
        // accepting changes nothing: the decision is identical on both paths, and in
        // particular never `allow` — "never a repair" is a fact about the change, not
        // about who is asking.
        let contract = fixture();
        for row in &MAP {
            for path in [AcceptancePath::AgentAccept, AcceptancePath::HumanAccept] {
                let verdict = contract
                    .policy()
                    .verdict(
                        &records(row.field, row.dangerous),
                        contract.policy_reviewers(),
                        path,
                    )
                    .expect("a complete, admissible classification reaches a verdict");
                assert!(never_a_repair(&verdict), "{}: {path:?}", row.phrase);
                assert_eq!(verdict.decision(), row.decision, "{}: {path:?}", row.phrase);
            }
        }
    }

    #[test]
    fn positive_fail_closed_unknown_forbids_allow_and_correction_13_excludes_assurance_incomparable()
     {
        let contract = fixture();
        // A classifier that cannot answer contributes at least `review` on every one
        // of the six fields — and `block` where the fixture's verb is `locked` (P2).
        for row in &MAP {
            let verdict = contract
                .policy()
                .verdict(
                    &records(row.field, Relation::Unknown),
                    contract.policy_reviewers(),
                    AcceptancePath::AgentAccept,
                )
                .expect("`unknown` is admissible on every field");
            assert!(never_a_repair(&verdict), "{}", row.phrase);
            let floor = if row.governing_verb == PolicyVerb::Locked {
                PolicyDecision::Block
            } else {
                PolicyDecision::Review
            };
            assert_eq!(verdict.decision(), floor, "{}", row.phrase);
        }
        // Correction 13's carve-out (bn-2sngz): `incomparable` is inadmissible on
        // `assurance` alone — its total order leaves no both-inclusions-refuted case —
        // while the other fourteen fields admit it, and `assurance` still admits the
        // other two non-affirmative relations.
        assert!(!PolicyField::Assurance.admits_relation(Relation::Incomparable));
        assert!(ClassificationRecord::new(PolicyField::Assurance, Relation::Incomparable).is_err());
        for field in PolicyField::ALL {
            if field != PolicyField::Assurance {
                assert!(
                    field.admits_relation(Relation::Incomparable),
                    "{field}: every other field admits `incomparable`"
                );
            }
        }
        assert!(PolicyField::Assurance.admits_relation(Relation::Unsupported));
        assert!(PolicyField::Assurance.admits_relation(Relation::Unknown));
    }

    #[test]
    fn negative_a_blind_classifier_reading_an_attack_as_unchanged_is_detected() {
        // The mutant: a classifier that reads every attack `unchanged`. The verdict
        // then closes `allow` with zero reasons — and this file's own predicate
        // rejects exactly that outcome, proving the dangerous *relation* (produced by
        // the production classifiers in the cited campaigns) is what carries the
        // catch, never this harness.
        let contract = fixture();
        for row in &MAP {
            let blind = contract
                .policy()
                .verdict(
                    &records(row.field, Relation::Unchanged),
                    contract.policy_reviewers(),
                    AcceptancePath::AgentAccept,
                )
                .expect("fifteen `unchanged` records always reach a verdict");
            assert_eq!(blind.decision(), PolicyDecision::Allow, "{}", row.phrase);
            assert!(blind.reasons().is_empty(), "{}", row.phrase);
            assert!(
                !never_a_repair(&blind),
                "{}: the blind outcome fails the map's own requirement",
                row.phrase
            );
        }
    }
}

// --- "a new Intent Contract": the identity leg ---------------------------------------------

mod new_intent_contract {
    use super::*;

    /// The fixture's parts, re-spelled with a different policy table.
    fn with_policy(contract: &IntentContract, policy: PolicyTable) -> IntentContract {
        IntentContract::new(ContractParts {
            intent_id: contract.intent_id().clone(),
            name: contract.name().map(str::to_owned),
            claims: contract.claims().clone(),
            assumptions: contract.assumptions().clone(),
            observers: contract.observers().clone(),
            abstraction_maps: contract.abstraction_maps().clone(),
            scope: contract.scope().clone(),
            trust_boundaries: contract.trust_boundaries().clone(),
            bounds: contract.bounds().clone(),
            fault_model: contract.fault_model().clone(),
            fairness: contract.fairness().clone(),
            completion_policy: contract.completion_policy(),
            nondeterminism: contract.nondeterminism().clone(),
            assurance: contract.assurance().clone(),
            optimization: contract.optimization().clone(),
            security_policy: contract.security_policy().clone(),
            policy,
            policy_reviewers: contract.policy_reviewers().clone(),
        })
    }

    #[test]
    fn positive_a_protected_field_change_and_a_governance_edit_both_move_the_identity() {
        let contract = fixture();
        // Control: an identical re-assembly keeps the identity.
        let rebuilt = with_policy(&contract, contract.policy().clone());
        assert_eq!(
            rebuilt.identity_preimage_bytes(),
            contract.identity_preimage_bytes(),
            "re-assembly alone moves nothing"
        );
        // A bound shrink — the map's third verb — is a *different contract*: the
        // change cannot ride the old `in_*`, which is what "a new Intent Contract"
        // means at the identity layer.
        let shrunk = RR_FIXTURE.replacen("\"nodes\":3", "\"nodes\":2", 1);
        assert_ne!(shrunk, RR_FIXTURE, "the mutation touched the document");
        let shrunk = IntentContract::decode(shrunk.as_bytes())
            .expect("the shrunk contract still decodes through the front door");
        assert_ne!(
            shrunk.identity_preimage_bytes(),
            contract.identity_preimage_bytes(),
            "shrinking a bound moves the identity"
        );
        // A governance-only rewrite also moves it: ID2 puts `policy` in the preimage,
        // which is DX02-B14's identity-layer half asserted live — the classifier
        // family cannot see a policy rewrite (fifteen `unchanged`), but the identity
        // refuses the `unchanged` shortcut, and ID3 routes the edit through
        // `intent.lock`'s successor mint.
        let unlocked = with_policy(&contract, PolicyTable::all_unlocked());
        assert_ne!(
            unlocked.identity_preimage_bytes(),
            contract.identity_preimage_bytes(),
            "a governance edit is an identity move, never an in-place repair"
        );
    }

    #[test]
    fn positive_id2_metadata_never_moves_the_identity() {
        // The other direction, so the identity leg cannot pass by moving on
        // everything: ID2's exclusions — `name`, `expression.source` — change the
        // document without changing the contract, which is what lets an ordinary
        // prose cleanup remain an ordinary repair (DX02-C02/C03's grain, live here).
        let contract = fixture();
        let renamed = RR_FIXTURE.replacen(
            "\"name\":\"Cancel-Correct Replicated Register\"",
            "\"name\":\"Renamed By An Ordinary Task\"",
            1,
        );
        assert_ne!(renamed, RR_FIXTURE, "the mutation touched the document");
        let renamed = IntentContract::decode(renamed.as_bytes()).expect("still a whole contract");
        assert_eq!(
            renamed.identity_preimage_bytes(),
            contract.identity_preimage_bytes(),
            "`name` is metadata (ID2): no new contract"
        );
        assert_ne!(
            renamed.to_artifact_bytes(),
            contract.to_artifact_bytes(),
            "the artifact really changed; only the identity stood still"
        );
        let prose = RR_FIXTURE.replacen(
            "forall epoch, v1, v2: chosen.get(epoch) == Some(v1) && chosen.get(epoch) \
             == Some(v2) => v1 == v2",
            "prose rewrite: ID2 excludes expression.source",
            1,
        );
        assert_ne!(prose, RR_FIXTURE, "the mutation touched the document");
        let prose = IntentContract::decode(prose.as_bytes()).expect("still a whole contract");
        assert_eq!(
            prose.identity_preimage_bytes(),
            contract.identity_preimage_bytes(),
            "`expression.source` is display (ID2): no new contract"
        );
    }
}

// --- the classifier family, cited with freshness tripwires ---------------------------------

mod cited_campaigns {
    use super::*;

    #[test]
    fn boundary_the_dx02_campaign_still_attacks_every_map_row_at_production_grain() {
        // The G0-DX-02 falsification campaign IS the classifier-grain INV-001
        // evidence: every one of the six verbs attacked as a genuine before/after
        // contract pair through `IntentContract::decode`, classified by the
        // production family, closed into the fixture's own table. Cited, not re-run
        // (bn-1eqt's rule); these pins rot loudly.
        assert_eq!(
            missing_campaign_rows(DX02_CAMPAIGN),
            Vec::<&str>::new(),
            "every map row's campaign attacks are still in the campaign"
        );
        assert_eq!(
            missing_campaign_rows(DX02_GOLDEN),
            Vec::<&str>::new(),
            "and in its retained byte-stable evidence artifact"
        );
        assert!(
            DX02_CAMPAIGN.contains("All nineteen attacks were caught"),
            "the campaign's recorded result stands"
        );
        // The two campaign mutants that make this citation non-vacuous at its home:
        // fail-closed is load-bearing, and P7's before-table is load-bearing.
        assert!(
            DX02_CAMPAIGN.contains(
                "mutant_fail_open_closure_would_let_the_unclassified_field_attack_through"
            )
        );
        assert!(DX02_CAMPAIGN.contains(
            "mutant_verdict_against_the_after_table_would_let_the_self_unlock_rider_through"
        ));
        // The recorded governance boundary this file's identity leg completes.
        assert!(DX02_CAMPAIGN.contains("DX02-B14"));
    }

    #[test]
    fn boundary_the_pr12_exit_still_closes_the_quantifier_over_the_g0_set() {
        assert!(
            PR12_EXIT.contains("all G0 intent-gaming patches are privileged changes"),
            "the exit sentence is still the file's subject"
        );
        assert!(
            PR12_EXIT.contains("g0_closure"),
            "the quantifier is still derived from the dossier row's own text"
        );
        assert!(
            PR12_EXIT.contains("mutant_a_blind_property_classifier_flips_the_exit_and_is_rejected"),
            "the exit's own anti-vacuity mutant is still present"
        );
        assert!(
            PR12_EXIT.contains("PR12_EXIT_BLESS"),
            "golden regeneration still cannot pass silently"
        );
        assert!(!PR12_GOLDEN.is_empty(), "the retained exit artifact exists");
    }

    #[test]
    fn boundary_the_eight_classifier_modules_are_still_exactly_the_landed_family() {
        // Bidirectional, against the real directory: a ninth module (or a vanished
        // sixth) re-opens this map's classifier column.
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../continuum-semantic-diff/src");
        let mut found = BTreeSet::new();
        for entry in std::fs::read_dir(&root).expect("continuum-semantic-diff/src is tracked") {
            let path: PathBuf = entry.expect("a readable directory entry").path();
            if path.extension().is_some_and(|extension| extension == "rs") {
                found.insert(
                    path.file_name()
                        .expect("a file has a name")
                        .to_string_lossy()
                        .into_owned(),
                );
            }
        }
        let expected: BTreeSet<String> = [
            "assumptions.rs",
            "assurance.rs",
            "bounds.rs",
            "equality.rs",
            "fairness.rs",
            "faults.rs",
            "lib.rs",
            "observers.rs",
            "properties.rs",
            // Not classifiers: the PR-12 assembler halves (bn-7ek41). `artifact.rs`
            // assembles the family's records into the wire `diff_*` shape and
            // `impact.rs` computes RFC 0031's impact set; neither owns a
            // classification, so neither may appear in the map's classifier column
            // (asserted below).
            "artifact.rs",
            "impact.rs",
        ]
        .into_iter()
        .map(str::to_owned)
        .collect();
        assert_eq!(
            found, expected,
            "the eight-module classifier family (plus lib.rs and the two bn-7ek41 \
             assembler modules), exactly — update the map on drift"
        );
        for row in &MAP {
            assert!(
                expected.contains(row.classifier),
                "{}: the classifier column names a real module",
                row.phrase
            );
        }
    }

    #[test]
    fn negative_the_campaign_tripwires_are_not_vacuous() {
        let doctored = DX02_CAMPAIGN.replace("DX02-A10", "DX02-XXX");
        assert_ne!(doctored, DX02_CAMPAIGN, "the mutation touched the text");
        assert_eq!(
            missing_campaign_rows(&doctored),
            vec!["DX02-A10"],
            "gutting one cited attack is reported by row id"
        );
    }
}

// --- the daemon boundary: WHO may revise, and what the wire refuses ------------------------

mod daemon_boundary {
    use super::*;

    #[test]
    fn boundary_accept_reject_lock_are_privileged_and_proposing_is_not() {
        // The registry's declarations are the wire's own statement of the privilege
        // half — docs/49's "revise intent: proposal only" cell as the propose/accept
        // split, not a level.
        let propose = spec_block(PROTOCOL_REGISTRY, "intent.propose_revision");
        assert!(propose.contains("AuthorityLevel::ReviseIntent"));
        assert!(
            !propose.contains("Annotation::Privileged"),
            "proposing is deliberately not privileged (RFC 0027)"
        );
        for operation in ["intent.accept", "intent.reject", "intent.lock"] {
            let block = spec_block(PROTOCOL_REGISTRY, operation);
            assert!(
                block.contains("Annotation::Privileged"),
                "{operation} is @privileged"
            );
            assert!(
                block.contains("Annotation::AuditRecorded"),
                "{operation} is @audit_recorded"
            );
        }
        // T3 is decided in admission, before any handler, from the capability
        // profile's explicit grant.
        assert!(DAEMON_ADMISSION.contains("T3 (privilege)"));
        assert!(DAEMON_ADMISSION.contains("privileged_operations"));
        // docs/49's authority row, and the daemon module doc that realizes it.
        assert!(DOCS_49.contains(
            "| revise intent | proposal only | no | no | proposal only | policy-dependent |"
        ));
        assert!(DAEMON_INTENT.contains("revise intent: proposal only"));
    }

    #[test]
    fn boundary_the_propose_lane_refuses_rather_than_classifies_and_the_lock_checks_are_live() {
        // The live half: the locked-field refusal, the weaken-refusing lock edit
        // (the lattice's own order), and ID3's successor mint.
        assert!(propose_lane_pins(DAEMON_INTENT));
        // The absent half, typed: no classification is guessed — the lane closes to
        // the declared `UnsupportedSemanticFeature`. This pin plus the rationale pin
        // below IS bn-1604's owed recheck trigger: `rationale` is declared on the
        // wire and still read by nothing in the handler, so "never read at all"
        // remains true; the moment propose logic lands, one of these two pins breaks
        // and the recheck comes due.
        assert!(DAEMON_INTENT.contains("UnsupportedSemanticFeature"));
        assert!(PROTOCOL_SHARED.contains("rationale: String required;"));
        assert!(
            !DAEMON_INTENT.contains("rationale"),
            "bn-1604's 'never read at all' finding still holds in the handler"
        );
        // Drafts gain protection only on acceptance (plan §4): a snapshot cannot be
        // governed by a proposal whose intent can still change, nor by a contract a later
        // acceptance or lock superseded (RFC 0037 correction 23, bn-1mgcv).
        assert!(DAEMON_WORKSPACE.contains("super::intent::accepted_head(intent)"));
        assert!(
            DAEMON_WORKSPACE
                .contains("the governing intent is not the accepted head of its lineage")
        );
        assert!(
            normalized(PLAN_MD)
                .contains("they gain protection (INV-001) only on explicit acceptance")
        );
    }

    #[test]
    fn boundary_no_wire_path_mints_a_proposal_and_no_daemon_code_names_an_acceptance_path() {
        // P7 — recomputation at `intent.accept` time — has no producer, and no
        // exploit: the only `put_intent` call site is `intent.lock`'s successor
        // write, so no wire path mints the `Proposed` record an unrecomputed accept
        // would need. Both facts are swept over the real source tree; either landing
        // turns this red and re-opens the row.
        let sources = continuumd_sources();
        let minting = sources_containing(&sources, "put_intent");
        assert_eq!(
            minting,
            vec!["daemon/intent.rs".to_owned(), "daemon/state.rs".to_owned()],
            "the definition, and the lock successor: nothing else writes the registry"
        );
        assert_eq!(
            sources_containing(&sources, "AcceptancePath"),
            Vec::<String>::new(),
            "no daemon code computes a policy verdict yet — the typed absence"
        );
    }

    #[test]
    fn boundary_first_intent_wins_and_a_seal_cannot_be_lifted() {
        // D1's disposition from the DX-03 fix wave (bn-n1xou), ratified via INV-001:
        // re-governing an existing snapshot would be an intent change without a
        // revision, so identical creates converge on the FIRST intent, and no
        // operation un-seals.
        assert!(DAEMON_WORKSPACE.contains("converge on the *first* intent"));
        // The sentence wraps across a comment line in the source, so the pin is its
        // contiguous head: "A re-create cannot re-govern a snapshot that [exists]".
        assert!(DAEMON_WORKSPACE.contains("A re-create cannot re-govern a snapshot"));
        assert!(DAEMON_WORKSPACE.contains("There is no un-seal operation in this protocol"));
        // Tasks carry intent by identity only — the binding a fork preserves and a
        // revision therefore cannot silently inherit.
        assert!(
            DAEMON_TASK.contains("The governing intent, by identity only (plan §4.2, INV-001)")
        );
        assert!(normalized(PLAN_MD).contains(
            "rebinding a snapshot lineage to a different intent is a privileged operation \
             that produces a semantic intent diff and invalidates dependent evidence (INV-001)"
        ));
    }

    #[test]
    fn negative_the_daemon_tripwires_are_not_vacuous() {
        // Strip the locked-field refusal: the propose-lane predicate must fail.
        let doctored = DAEMON_INTENT.replace(
            "the change set names a field whose change policy is `locked`",
            "",
        );
        assert_ne!(doctored, DAEMON_INTENT, "the mutation touched the source");
        assert!(
            !propose_lane_pins(&doctored),
            "gutting the live refusal is detected"
        );
        // Plant the forbidden token: the sweep must report the planted file.
        let planted = vec![(
            "daemon/mutant.rs".to_owned(),
            "let path = AcceptancePath::AgentAccept;".to_owned(),
        )];
        assert_eq!(
            sources_containing(&planted, "AcceptancePath"),
            vec!["daemon/mutant.rs".to_owned()],
            "a landed AcceptancePath use is detected"
        );
    }
}

// --- the fourth element: invalidation of dependent evidence --------------------------------

mod dependent_evidence {
    use super::*;

    #[test]
    fn boundary_invalidation_is_discharged_by_identity_and_the_explicit_lane_has_no_producer() {
        // The typed vocabulary exists: `continuum-evidence` declares the
        // `INVALIDATES` edge kind (docs/44's "one patch repairs one failure but
        // invalidates a proof").
        assert!(EVIDENCE_EDGE.contains("Invalidates"));
        assert!(EVIDENCE_EDGE.contains("\"INVALIDATES\""));
        // And the intent lane produces none of it: nothing in the daemon's intent
        // family names invalidation. Today the element is discharged by identity —
        // a revision mints a new `in_*` ([`new_intent_contract`]), evidence and
        // tasks bind intent by identity only ([`daemon_boundary`]), so prior
        // evidence is never re-served for a revised intent rather than being walked
        // and marked. This pin goes red when the explicit producer lands, which is
        // the moment this row must be rewritten as a positive.
        assert!(
            !DAEMON_INTENT.contains("nvalidat"),
            "no invalidation producer in the intent family yet — the typed absence"
        );
    }

    #[test]
    fn boundary_inv011_apply_refuses_intent_changes_and_promotion_reclassification_is_absent() {
        // INV-011's half of the AGENTS.md sentence belongs to `continuum-repair`
        // (plan §8, PR 20). bn-2d70 (PR-20 / IMPL-01) landed its first half: an
        // `intent`-kind change through ordinary repair authority is refused
        // `IntentMutationDenied` at `repair.apply` (RFC 0032, "Intent integrity and
        // reclassification"), and a hypothesis is prose that nothing reads. This guard
        // keeps that refusal in place.
        assert!(REPAIR_LIB.contains("INV-011"));
        assert!(REPAIR_LIB.contains("reclassified and blocked rather than merged"));
        assert!(REPAIR_TRANSACTION.contains("IntentMutationDenied { index }"));
        assert!(REPAIR_TRANSACTION.contains("change.kind() == ChangeKind::Intent"));
        // The second half is still absent, and this pin goes red when it lands: a
        // `model` or `rust` change that weakens a property is classified only by the
        // RFC 0031 diff behind gate 3, and there is no evaluate, promote, or
        // reclassification path in the crate yet (IMPL-04, IMPL-06).
        for absent in ["pub fn evaluate", "pub fn promote", "pub fn reclassify"] {
            assert!(
                !REPAIR_TRANSACTION.contains(absent),
                "`{absent}` landed; rewrite this row as a positive"
            );
        }
    }
}
