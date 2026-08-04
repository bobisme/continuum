//! Evidence for `PR-11-IMPL-06` — byte/token budgets (bn-38p2).
//!
//! > - byte/token budgets.
//! >
//! > — `notes/plan/notes/START_HERE_IMPLEMENTATION.md`, PR 11
//!
//! # The two halves this bullet is
//!
//! **The packer.** RFC 0028's budget row has two branches, and IMPL-04 shipped one of them:
//!
//! > | the budget is exhausted before the expansion completes | error `BudgetExhausted` with
//! > a continuation. **Either nothing is published, or a child pack is published whose
//! > manifest records the shortfall with reason `budget`**; a child claiming completeness
//! > MUST NOT be published (INV-009, INV-017) |
//! >
//! > — RFC 0028, "Expansion protocol", typed outcomes
//!
//! `continuum_context::budget` is the second branch. Which branch a ceiling gets is not a
//! preference: a smaller child is published while one *conforms*, and the boundary is the
//! child that carries the complete manifest and selects nothing, because "the omission
//! manifest […] is reserved before any optional content" and is "never the thing a budget
//! squeezes out (INV-007)".
//!
//! **The measurement.** `content_budget.bytes` is the pack's own measured post-packing size
//! (RFC 0028 correction 17), not the ceiling it ran under. This file checks the number
//! against the document that carries it, and checks the property the correction exists for:
//!
//! > **Expansion accounting.** The ratified FR-01 sentence grades context bytes "counting
//! > every expansion", so a deployment MUST be able to **sum `content_budget.bytes` over a
//! > whole pack family**.
//! >
//! > — RFC 0028, "Expansion protocol"
//!
//! A family of *ceilings* sums to a number no compiler controls; a family of measurements
//! sums to the bytes the family cost. [`the_family_sum_is_the_bytes_the_family_cost`] is that
//! sentence made checkable.
//!
//! **Tokens are absent, deliberately.** `content_budget.tokens` is "advisory and
//! model-relative", and the schema requires `tokenizer_id` beside it — "a token count with no
//! tokenizer is not a measurement" (RFC 0028 F4). No tokenizer exists in this workspace, so a
//! count written here would be fabricated or non-conforming, and this crate writes neither
//! key ([`no_pack_claims_a_token_count`]). The word in the bullet is answered structurally
//! rather than dropped.
//!
//! # Clause → test
//!
//! | Clause | Source | Test |
//! |---|---|---|
//! | an over-budget expansion publishes a smaller child recording the shortfall | the budget row, second branch | [`an_over_budget_expansion_publishes_a_smaller_child_recording_the_shortfall`] |
//! | the conservation law holds on the packed child | RFC 0028, "Counts are exact"; INV-007 | [`the_conservation_law_holds_on_every_packed_child`] |
//! | the manifest is reserved before optional content | INV-007 | [`the_conservation_law_holds_on_every_packed_child`] |
//! | a larger budget extends the prefix and omits less | "Prefix monotonicity"; the frontier | [`ceilings_are_ordered_by_the_frontier_and_never_conflict`] |
//! | one input and one ceiling are one document | `rule ordering.deterministic` | [`one_input_and_one_ceiling_are_byte_identical`] |
//! | below the minimal child, nothing is published | the budget row, first branch | [`below_the_minimal_child_nothing_is_published`] |
//! | measured bytes are the canonical encoding's length | RFC 0028 correction 17 | [`the_recorded_size_is_the_documents_own_length`] |
//! | a family of measurements is summable | "Expansion accounting" | [`the_family_sum_is_the_bytes_the_family_cost`] |
//! | no token count without a tokenizer | RFC 0028 F4; the schema | [`no_pack_claims_a_token_count`] |
//! | the accounting's refusals still fire | bn-28jj's closed accounting | [`the_packer_refuses_what_the_accounting_refuses`] |
//!
//! # OOM hygiene
//!
//! Every fixture is a `const` or a linear `map` over a range. Nothing is built by doubling
//! text, and the largest document here is under 200 KiB (the two-thousand-item adversarial
//! case, which exists to show the search is not quadratic).

use continuum_context::accounting::{Accounting, AccountingError, CandidateSet, Omitted};
use continuum_context::budget::{BudgetError, BudgetPacker, PackedChild};
use continuum_context::expansion::{
    Depth, ExpansionHandle, ExpansionQuery, ExpansionQuestion, ExpansionRelation,
};
use continuum_context::omission::{Manifest, OmissionReason, OmissionRecord};
use continuum_context::pack::{self, ChildPack};
use continuum_context::selection::{SelectedItem, SelectionKind};
use continuum_context::source::{SourceRef, SourceSpan};
use continuum_intent::canonical_json::Json;
use continuum_value::identity::Blake3Hasher;
use continuum_value::value::Name;
use continuum_workspace::snapshot::WorkspacePath;

/// A conforming parent pack, written as a document rather than constructed: a fixture that
/// built one through the code under test could agree with itself about a shape neither has.
const PARENT_PACK: &str = r#"{
  "assurance": {"class": "bounded", "envelope": {}},
  "content_budget": {"bytes": 16384},
  "content_hash": "blake3-256:parentplaceholder",
  "context_id": "ctx_parent01",
  "evidence": ["ev_failure1"],
  "expansions": [{"anchor": "e_ack", "relation": "source_span"}],
  "guarantees": ["ReplayPreserving", "CausallyClosed"],
  "intent": "in_ack_v1",
  "omissions": [{"count": 12, "expandable": true,
                 "expansion": {"anchor": "e_ack", "relation": "source_span"},
                 "kind": "source", "reason": "budget"}],
  "parent": null,
  "question": "why did AckImpliesDurable fail?",
  "replay": "crash_demo1",
  "schema_epoch": 1,
  "schema_id": "https://continuum.dev/schema/context-pack.json",
  "selected": [{"artifact": "ev_ack1", "id": "e_ack", "kind": "event",
                "summary": "reply published before stable write"}],
  "semantic_epoch": "sem3-r3-demo",
  "snapshot": "ws_demo1",
  "verdict": "refuted"
}"#;

fn parent() -> Json {
    Json::parse(PARENT_PACK.as_bytes()).expect("the fixture is admissible canonical JSON")
}

fn name(text: &str) -> Name {
    Name::new(text).expect("a canonical identifier")
}

fn query() -> ExpansionQuery {
    ExpansionQuery::new(ExpansionRelation::SourceSpan, name("e_ack"))
}

/// `count` distinct `source` items, in one linear pass.
fn items(count: u32) -> Vec<SelectedItem> {
    (0..count)
        .map(|index| {
            let line = index + 1;
            let span = SourceSpan::new(
                WorkspacePath::new("src/ack.rs").expect("a repo-relative path"),
                line,
                1,
                line,
                40,
            )
            .expect("a well-formed span");
            SourceRef::new(span).into_selected_item(name(&format!("s_{index:04}")))
        })
        .collect()
}

/// One group this expansion does not reach: outstanding at every ceiling.
fn residual() -> Manifest {
    Manifest::new([OmissionRecord::expandable(
        SelectionKind::Event,
        5,
        OmissionReason::HeuristicCutoff,
        ExpansionQuery::new(ExpansionRelation::CausalPredecessors, name("e_ack")),
    )])
    .expect("one cell")
}

/// Pack an expansion of `parent` at `ceiling`.
fn pack_at(
    parent: &Json,
    selected: &[SelectedItem],
    manifest: &Manifest,
    ceiling: u64,
) -> Result<PackedChild, BudgetError> {
    let identity = ExpansionHandle::derive::<Blake3Hasher>(
        &pack::identity_of(parent).expect("a pack handle"),
        &query(),
        Depth::DEFAULT,
    )
    .expect("derives");
    let question = ExpansionQuestion::of(&query(), Depth::DEFAULT);
    let anchor = query();
    BudgetPacker {
        full: ChildPack {
            parent,
            identity: &identity,
            question: &question,
            selected,
            manifest,
            ceiling,
        },
        shortfall: &anchor,
    }
    .pack::<Blake3Hasher>()
}

/// What the whole answer measures — the yardstick every ceiling below is stated against.
fn unpacked(parent: &Json, selected: &[SelectedItem], manifest: &Manifest) -> u64 {
    let packed = pack_at(parent, selected, manifest, u64::MAX).expect("no ceiling refuses");
    assert!(!packed.is_packed());
    packed.measured()
}

fn array<'a>(document: &'a Json, key: &str) -> &'a [Json] {
    document.as_object().expect("object")[key]
        .as_array()
        .expect("array")
}

fn selected_ids(document: &Json) -> Vec<String> {
    array(document, "selected")
        .iter()
        .map(|item| {
            item.as_object().expect("object")["id"]
                .as_str()
                .expect("string")
                .to_owned()
        })
        .collect()
}

/// `|selected| + Σ manifest counts`, read off a published document alone.
fn accounted(document: &Json) -> i64 {
    let selected = i64::try_from(array(document, "selected").len()).expect("small");
    let omitted: i64 = array(document, "omissions")
        .iter()
        .map(|record| {
            record.as_object().expect("object")["count"]
                .as_integer()
                .expect("an exact integer count")
        })
        .sum();
    selected + omitted
}

// --- the second branch --------------------------------------------------------------

#[test]
fn an_over_budget_expansion_publishes_a_smaller_child_recording_the_shortfall() {
    let parent = parent();
    let items = items(12);
    let residual = residual();
    let whole = unpacked(&parent, &items, &residual);

    let packed = pack_at(&parent, &items, &residual, whole - 400).expect("a smaller child");
    assert!(packed.is_packed());
    assert!(packed.kept() < items.len() && packed.kept() > 0);
    assert!(packed.measured() <= whole - 400);

    // Every dropped candidate is in the manifest, under `budget`, with the query that
    // retrieves it — INV-007's two halves, both present.
    let record = packed
        .manifest()
        .records()
        .iter()
        .find(|record| record.reason() == OmissionReason::Budget)
        .expect("the ceiling's own record");
    assert_eq!(record.kind(), SelectionKind::Source);
    assert_eq!(
        u64::from(record.count()),
        (items.len() - packed.kept()) as u64
    );
    assert!(record.retrievability().is_expandable());
    assert_eq!(record.retrievability().query(), Some(&query()));

    // The published document says the same, and is still a pack.
    pack::required_keys_present(packed.document()).expect("a conforming child");
    let published = array(packed.document(), "omissions")
        .iter()
        .find(|record| {
            record.as_object().expect("object")["reason"] == Json::String("budget".to_owned())
        })
        .expect("the shortfall reaches the document");
    assert_eq!(
        published.as_object().expect("object")["expandable"],
        Json::Bool(true)
    );
    assert_eq!(
        published.as_object().expect("object")["expansion"],
        query().to_json()
    );
    // A child claiming completeness MUST NOT be published: this one claims none.
    assert!(!array(packed.document(), "omissions").is_empty());
}

#[test]
fn the_conservation_law_holds_on_every_packed_child() {
    // |selected| + Σ manifest counts = the candidate set, at every ceiling that publishes —
    // and the residual group, which no budget may squeeze out, is in every one of them.
    let parent = parent();
    let items = items(12);
    let residual = residual();
    let whole = unpacked(&parent, &items, &residual);
    let expected = i64::try_from(items.len()).expect("small") + 5;

    let mut packed_seen = 0;
    for ceiling in (whole - 1000..=whole).step_by(40) {
        let Ok(packed) = pack_at(&parent, &items, &residual, ceiling) else {
            continue;
        };
        assert_eq!(
            accounted(packed.document()),
            expected,
            "a ceiling of {ceiling} lost or invented a candidate"
        );
        assert_eq!(
            packed.kept() as u64 + u64::from(packed.dropped()),
            items.len() as u64
        );
        assert!(
            packed
                .manifest()
                .records()
                .iter()
                .any(|record| record.reason() == OmissionReason::HeuristicCutoff
                    && record.count() == 5),
            "a ceiling of {ceiling} squeezed out a manifest record"
        );
        if packed.is_packed() {
            packed_seen += 1;
        }
    }
    assert!(
        packed_seen > 3,
        "the sweep must exercise packing: {packed_seen}"
    );
}

#[test]
fn ceilings_are_ordered_by_the_frontier_and_never_conflict() {
    // RFC 0028: "Two compiles of one key under different budgets MUST be related by this
    // order; they MUST NOT be two conflicting answers under one identity" — the selection of
    // the smaller is a subset of the larger's (here, a prefix: the order does not depend on
    // the budget), and every manifest count in the larger is ≤ the smaller's.
    let parent = parent();
    let items = items(12);
    let residual = residual();
    let whole = unpacked(&parent, &items, &residual);

    let narrow = pack_at(&parent, &items, &residual, whole - 700).expect("a smaller child");
    let wide = pack_at(&parent, &items, &residual, whole - 250).expect("a smaller child");
    let complete = pack_at(&parent, &items, &residual, whole).expect("everything fits");

    let narrow_ids = selected_ids(narrow.document());
    let wide_ids = selected_ids(wide.document());
    let all_ids = selected_ids(complete.document());
    assert!(narrow_ids.len() < wide_ids.len() && wide_ids.len() < all_ids.len());
    assert_eq!(narrow_ids.as_slice(), &wide_ids[..narrow_ids.len()]);
    assert_eq!(wide_ids.as_slice(), &all_ids[..wide_ids.len()]);
    assert!(narrow.manifest().total() > wide.manifest().total());
    assert!(wide.manifest().total() > complete.manifest().total());
    // The frontier's third term: the guarantee set. A child claims none at any ceiling, so
    // packing cannot have truncated a guaranteed core.
    for child in [&narrow, &wide, &complete] {
        assert_eq!(
            child.document().as_object().expect("object")["guarantees"],
            Json::Array(Vec::new())
        );
    }
}

#[test]
fn one_input_and_one_ceiling_are_byte_identical() {
    let parent = parent();
    let items = items(12);
    let residual = residual();
    let whole = unpacked(&parent, &items, &residual);
    let ceiling = whole - 500;

    let first = pack_at(&parent, &items, &residual, ceiling).expect("a smaller child");
    let second = pack_at(&parent, &items, &residual, ceiling).expect("a smaller child");
    assert_eq!(
        first.document().to_canonical_bytes(),
        second.document().to_canonical_bytes()
    );
    // Not vacuous: a different ceiling is a different document under the same `ctx_*`, which
    // is what "budget belongs to execution identity" means for two answers to one question.
    let other = pack_at(&parent, &items, &residual, ceiling - 200).expect("a smaller child");
    assert_ne!(
        first.document().to_canonical_bytes(),
        other.document().to_canonical_bytes()
    );
    assert_eq!(
        first.document().as_object().expect("object")["context_id"],
        other.document().as_object().expect("object")["context_id"]
    );
    assert_ne!(
        first.document().as_object().expect("object")["content_hash"],
        other.document().as_object().expect("object")["content_hash"]
    );
}

// --- the first branch, and the boundary between them ---------------------------------

#[test]
fn below_the_minimal_child_nothing_is_published() {
    let parent = parent();
    let items = items(12);
    let residual = residual();

    let minimal = match pack_at(&parent, &items, &residual, 0) {
        Err(BudgetError::Exhausted { minimal, ceiling }) => {
            assert_eq!(ceiling, 0);
            minimal
        }
        other => panic!("a ceiling of zero publishes nothing: {other:?}"),
    };

    // At the boundary: a pack that selects nothing and still accounts for everything.
    let boundary = pack_at(&parent, &items, &residual, minimal).expect("exactly fits");
    assert_eq!(boundary.kept(), 0);
    assert_eq!(boundary.dropped(), 12);
    assert_eq!(boundary.measured(), minimal);
    assert_eq!(accounted(boundary.document()), 17);
    assert!(array(boundary.document(), "selected").is_empty());

    // One byte below it, nothing conforms: shrinking further would mean dropping a manifest
    // record or shrinking a count, and both are forbidden outright.
    assert_eq!(
        pack_at(&parent, &items, &residual, minimal - 1),
        Err(BudgetError::Exhausted {
            minimal,
            ceiling: minimal - 1,
        })
    );
}

#[test]
fn the_packer_refuses_what_the_accounting_refuses() {
    // Anti-vacuity: the positive assertions above are not "the packer publishes whatever it
    // is given". bn-28jj's closed accounting still refuses a candidate set that cannot be
    // partitioned, and it refuses it *inside* the packer.
    let parent = parent();
    let mut repeated = items(6);
    repeated.push(repeated[0].clone());
    let residual = residual();
    let whole = unpacked(&parent, &items(6), &residual);
    // At a ceiling that packs and at one that does not: every published child comes off the
    // ledger, so fitting inside the budget is not a way around the accounting.
    for ceiling in [whole - 150, u64::MAX] {
        assert!(matches!(
            pack_at(&parent, &repeated, &residual, ceiling),
            Err(BudgetError::Accounting(
                AccountingError::RepeatedCandidate { .. }
            ))
        ));
    }

    // And the same equation, recomputed independently over the published halves: a packed
    // child's selection plus its shortfall count is the candidate set, checked by the
    // reconciliation RFC 0028's Validation section asks for rather than by this file's
    // arithmetic.
    let items = items(12);
    let whole = unpacked(&parent, &items, &residual);
    let packed = pack_at(&parent, &items, &residual, whole - 400).expect("a smaller child");
    let kept = selected_ids(packed.document());
    let mut accounting = Accounting::over(
        CandidateSet::new(items.iter().map(|item| (item.id().clone(), item.kind())))
            .expect("distinct candidates"),
    );
    for item in &items {
        let disposition = if kept.iter().any(|id| id == &item.id().to_string()) {
            accounting.select(item.id())
        } else {
            accounting.omit(
                item.id(),
                Omitted::Expandable {
                    reason: OmissionReason::Budget,
                    query: query(),
                },
            )
        };
        disposition.expect("a candidate");
    }
    let closed = accounting.close().expect("every candidate decided");
    closed.reconcile().expect("the two published halves agree");
    assert_eq!(closed.selected().len(), packed.kept());
    assert_eq!(closed.manifest().total(), u64::from(packed.dropped()));
}

// --- the measurement -------------------------------------------------------------------

#[test]
fn the_recorded_size_is_the_documents_own_length() {
    // RFC 0028 correction 17, checkable: the number in the pack is the length of the pack.
    // Under budget and over it, and never the ceiling in force.
    let parent = parent();
    let items = items(12);
    let residual = residual();
    let whole = unpacked(&parent, &items, &residual);

    // `whole` itself is left out of the sweep on purpose: at a ceiling that is exactly the
    // measurement the two numbers agree, and an assertion that they differ there would be
    // asserting a coincidence rather than the reading.
    for ceiling in [u64::MAX, whole - 200, whole - 650] {
        let packed = pack_at(&parent, &items, &residual, ceiling).expect("a child");
        let recorded = pack::budget_bytes_of(packed.document()).expect("a measured child");
        assert_eq!(recorded, packed.measured());
        assert_eq!(
            recorded,
            packed.document().to_canonical_bytes().len() as u64,
            "a ceiling of {ceiling} recorded a size that is not the document's"
        );
        assert!(recorded <= ceiling);
        assert_ne!(recorded, ceiling, "the ceiling is not the measurement");
        // Nor the parent's own recorded number, which is what the interim placeholder wrote.
        assert_ne!(recorded, 16384);
    }
}

#[test]
fn the_family_sum_is_the_bytes_the_family_cost() {
    // "A deployment MUST be able to sum `content_budget.bytes` over a whole pack family."
    // A family of ceilings sums to a number the compiler does not control; a family of
    // measurements sums to what the family actually cost, and here the two are compared:
    // the sum of the recorded values equals the sum of the documents' lengths.
    let parent = parent();
    let items = items(8);
    let residual = residual();

    let child = pack_at(&parent, &items, &residual, u64::MAX).expect("a child");
    // The child is itself a conforming pack, so it is a parent in turn — a two-generation
    // family, each generation recording what it spent.
    let grandchild =
        pack_at(child.document(), &items[..3], &Manifest::empty(), u64::MAX).expect("a grandchild");

    let family = [child.document(), grandchild.document()];
    let recorded: u64 = family
        .iter()
        .map(|document| pack::budget_bytes_of(document).expect("a measured pack"))
        .sum();
    let measured: u64 = family
        .iter()
        .map(|document| document.to_canonical_bytes().len() as u64)
        .sum();
    assert_eq!(recorded, measured);
    // And the sum is a real reduction claim rather than an inherited constant: under the old
    // ceiling reading, both members would have recorded `u64::MAX`.
    assert!(recorded < 2 * 16384);
    assert!(child.measured() > grandchild.measured());
}

#[test]
fn no_pack_claims_a_token_count() {
    // RFC 0028 F4 and the schema's conditional: `tokens` requires `tokenizer_id`, and no
    // tokenizer exists here. `content_budget` therefore carries exactly one key — an honest
    // absence, not a placeholder.
    let parent = parent();
    let items = items(12);
    let residual = residual();
    let whole = unpacked(&parent, &items, &residual);

    for ceiling in [u64::MAX, whole - 500] {
        let packed = pack_at(&parent, &items, &residual, ceiling).expect("a child");
        let budget = packed.document().as_object().expect("object")["content_budget"]
            .as_object()
            .expect("object");
        assert_eq!(
            budget.keys().map(String::as_str).collect::<Vec<_>>(),
            ["bytes"],
            "a token count with no tokenizer is not a measurement"
        );
    }
}

// --- adversarial size --------------------------------------------------------------------

#[test]
fn a_wide_expansion_packs_in_time_that_is_linear_in_its_candidates() {
    // Two thousand candidates, built linearly. The packer encodes each candidate once and
    // then searches over arithmetic, so this is a scan rather than a re-encoding per
    // candidate; a quadratic search over documents of this size would not finish comfortably
    // inside a test run.
    let parent = parent();
    let items = items(2_000);
    let residual = residual();
    let whole = unpacked(&parent, &items, &residual);

    let packed = pack_at(&parent, &items, &residual, whole / 3).expect("a smaller child");
    assert!(packed.kept() > 100, "kept {}", packed.kept());
    assert!(packed.kept() < 2_000);
    assert_eq!(accounted(packed.document()), 2_005);
    assert!(packed.measured() <= whole / 3);
    assert_eq!(
        pack::budget_bytes_of(packed.document()).expect("measured"),
        packed.document().to_canonical_bytes().len() as u64
    );
    // The prefix is the declared order's, from the front.
    assert_eq!(selected_ids(packed.document())[0], "s_0000");
    assert_eq!(
        selected_ids(packed.document())[packed.kept() - 1],
        format!("s_{:04}", packed.kept() - 1)
    );
}
