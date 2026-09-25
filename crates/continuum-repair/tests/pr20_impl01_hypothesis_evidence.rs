//! PR-20 / IMPL-01 evidence: the repair transaction's hypothesis (bn-2d70).
//!
//! The normative sources are `notes/plan/schemas/repair-transaction.schema.json`
//! (shape, INV-003) and RFC 0032 (the hypothesis is untrusted prose, never evidence;
//! the base triple is frozen; an `intent` change is `IntentMutationDenied` at
//! `repair.apply`, INV-011).
//!
//! # Retained artifacts
//!
//! | Artifact id | What it is | Checked by |
//! |---|---|---|
//! | `pr20-impl01-ack-after-sync-draft` | `tests/fixtures/pr20-impl01-ack-after-sync-draft.json`: `repair.begin` on the replicated register's ack-before-sync crashpack | this suite (byte equality) and `notes/plan/tools/validate_dossier.py` (Draft 2020-12 against the schema) |
//! | `pr20-impl01-ack-after-sync-applied` | `tests/fixtures/pr20-impl01-ack-after-sync-applied.json`: `repair.apply` with the move-ack-after-sync hypothesis and one `rust` change | as above |
//!
//! Set `CONTINUUM_REPAIR_BLESS=1` to rewrite both fixtures from the library.
//!
//! # Tests
//!
//! - positive: `positive_*` — the ack-after-sync repair opens and applies.
//! - negative: `negative_*` — an `intent` change (relax a property, remove a fault,
//!   hide an event) is refused; a hypothesis that *says* it weakens intent changes no
//!   typed field; an unknown, foreign, unbound, or unavailable failure opens nothing.
//! - boundary: `boundary_*` — canonical bytes are stable and round-trip; the identity
//!   moves with every field; the schema's closed sets are the crate's.

use std::collections::{BTreeMap, BTreeSet};

use continuum_intent::canonical_json::Json;
use continuum_intent::contract::{IntentContract, IntentId};
use continuum_repair::handle::{CrashpackId, SnapshotId};
use continuum_repair::hypothesis::{Change, ChangeKind, Hypothesis, Proposal};
use continuum_repair::transaction::{
    ApplyRefusal, BeginRefusal, FailureBinding, GateName, GateProfile, GateStatus,
    RepairTransaction, Resolution, SCHEMA_EPOCH, SCHEMA_ID, SealedCandidate, TransactionStatus,
};
use continuum_value::identity::{Blake3Hasher, ContentHasher};

/// Every transaction in this suite is named under BLAKE3.
type Tx = RepairTransaction<Blake3Hasher>;

const SCHEMA: &str = include_str!("../../../notes/plan/schemas/repair-transaction.schema.json");
const REGISTER_CONTRACT: &[u8] =
    include_bytes!("../../continuum-intent/tests/fixtures/replicated-register-contract.json");
const DRAFT_FIXTURE: &str = "tests/fixtures/pr20-impl01-ack-after-sync-draft.json";
const APPLIED_FIXTURE: &str = "tests/fixtures/pr20-impl01-ack-after-sync-applied.json";

const CRASH: &str = "crash_pr20_impl01_ack_before_sync";
const BASE: &str = "ws_pr20_impl01_register_base";
const CANDIDATE: &str = "ws_pr20_impl01_ack_after_sync";
const ACK_AFTER_SYNC: &str = "move the ack after the storage sync: the replica publishes its \
    reply only once the write is durable, so an acknowledged write survives a crash";
/// The patch text the `rust` change's digest names. Deriving change digests is IMPL-02's;
/// this one is a stable stand-in.
const ACK_AFTER_SYNC_PATCH: &str =
    "-    reply.send(Ack);\n-    storage.sync()?;\n+    storage.sync()?;\n+    reply.send(Ack);\n";

// --- fixtures ----------------------------------------------------------------------

/// The register's own intent identity, read from the Intent Contract fixture.
fn register_intent() -> IntentId {
    IntentContract::decode(REGISTER_CONTRACT)
        .expect("the replicated register contract decodes")
        .intent_id()
        .clone()
}

/// An in-memory binding: crashpack → snapshot, snapshot → intent.
#[derive(Default)]
struct Binding {
    failures: BTreeMap<String, Resolution<SnapshotId>>,
    intents: BTreeMap<String, Resolution<IntentId>>,
}

impl Binding {
    fn register() -> Self {
        let mut binding = Self::default();
        binding.failures.insert(
            CRASH.to_owned(),
            Resolution::Found(SnapshotId::new(BASE).unwrap()),
        );
        binding
            .intents
            .insert(BASE.to_owned(), Resolution::Found(register_intent()));
        binding
    }
}

impl FailureBinding for Binding {
    fn failure_base(&self, failure: &CrashpackId) -> Resolution<SnapshotId> {
        self.failures
            .get(failure.as_str())
            .cloned()
            .unwrap_or(Resolution::Unknown)
    }

    fn snapshot_intent(&self, snapshot: &SnapshotId) -> Resolution<IntentId> {
        self.intents
            .get(snapshot.as_str())
            .cloned()
            .unwrap_or(Resolution::Unknown)
    }
}

fn crash(handle: &str) -> CrashpackId {
    CrashpackId::new(handle).unwrap()
}

fn candidate(handle: &str) -> SealedCandidate {
    SealedCandidate::unchecked_from(SnapshotId::new(handle).unwrap())
}

fn digest_of(text: &str) -> String {
    format!(
        "blake3-256:{}",
        Blake3Hasher::hash(text.as_bytes()).to_token()
    )
}

fn draft() -> Tx {
    Tx::begin(crash(CRASH), GateProfile::PhaseB, &Binding::register())
        .expect("the ack-before-sync crashpack resolves")
}

fn ack_after_sync() -> Proposal {
    Proposal::new(
        Hypothesis::new(ACK_AFTER_SYNC),
        vec![Change::new(
            ChangeKind::Rust,
            digest_of(ACK_AFTER_SYNC_PATCH),
        )],
    )
}

fn applied() -> Tx {
    draft()
        .apply(&ack_after_sync(), candidate(CANDIDATE))
        .expect("a rust-only proposal applies")
}

fn fixture_matches(relative: &str, bytes: &[u8]) {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(relative);
    if std::env::var_os("CONTINUUM_REPAIR_BLESS").is_some() {
        std::fs::write(&path, bytes).expect("the fixture is writable");
    }
    let retained = std::fs::read(&path).expect("the retained fixture exists");
    assert!(
        retained == bytes,
        "{relative} is stale; rerun with CONTINUUM_REPAIR_BLESS=1\nlibrary:  {}\nretained: {}",
        String::from_utf8_lossy(bytes),
        String::from_utf8_lossy(&retained),
    );
}

fn fields(json: &Json) -> &BTreeMap<String, Json> {
    json.as_object().expect("an object")
}

fn parsed(text: &str) -> Json {
    Json::parse(text.as_bytes()).expect("a normative dossier document parses")
}

fn at<'a>(json: &'a Json, path: &[&str]) -> &'a Json {
    path.iter().fold(json, |cursor, key| {
        fields(cursor)
            .get(*key)
            .unwrap_or_else(|| panic!("the schema declares {key}"))
    })
}

fn tokens(json: &Json) -> Vec<String> {
    json.as_array()
        .expect("an enum is an array")
        .iter()
        .map(|item| item.as_str().expect("a string token").to_owned())
        .collect()
}

// --- positive ----------------------------------------------------------------------

/// `pr20-impl01-ack-after-sync-draft`: `repair.begin` resolves the base triple from the
/// crashpack alone and opens v1 at `draft` with every gate listed.
#[test]
fn positive_begin_opens_a_draft_on_the_registers_frozen_base_triple() {
    let draft = draft();
    assert_eq!(draft.version(), 1);
    assert_eq!(draft.supersedes(), None);
    assert_eq!(draft.status(), TransactionStatus::Draft);
    assert_eq!(draft.failure().as_str(), CRASH);
    assert_eq!(draft.base_snapshot().as_str(), BASE);
    assert_eq!(draft.base_intent(), &register_intent());
    assert_eq!(draft.candidate_snapshot(), None);
    assert_eq!(draft.hypothesis(), &Hypothesis::unstated());
    assert!(draft.changes().is_empty());
    let statuses: Vec<(GateName, GateStatus)> = draft
        .gates()
        .iter()
        .map(|gate| (gate.name(), gate.status()))
        .collect();
    let expected: Vec<(GateName, GateStatus)> = GateName::ALL
        .into_iter()
        .map(|name| {
            let status = match name {
                GateName::CertificateRebuild | GateName::IncrementalParity => {
                    GateStatus::NotYetEnforced
                }
                _ => GateStatus::Pending,
            };
            (name, status)
        })
        .collect();
    assert_eq!(statuses, expected);
    fixture_matches(DRAFT_FIXTURE, &draft.to_artifact_bytes());
}

/// `pr20-impl01-ack-after-sync-applied`: the move-ack-after-sync hypothesis is recorded
/// verbatim in v2, which supersedes v1 and keeps its base triple and profile.
#[test]
fn positive_apply_records_the_move_ack_after_sync_hypothesis() {
    let draft = draft();
    let applied = applied();
    assert_eq!(applied.version(), 2);
    assert_eq!(applied.supersedes(), Some(draft.repair_id()));
    assert_eq!(applied.status(), TransactionStatus::Applied);
    assert_eq!(applied.failure(), draft.failure());
    assert_eq!(applied.base_snapshot(), draft.base_snapshot());
    assert_eq!(applied.base_intent(), draft.base_intent());
    assert_eq!(applied.gate_profile(), draft.gate_profile());
    assert_eq!(applied.gates(), draft.gates(), "apply resets every gate");
    assert_eq!(
        applied.candidate_snapshot().map(SnapshotId::as_str),
        Some(CANDIDATE)
    );
    assert_eq!(applied.hypothesis().as_prose(), ACK_AFTER_SYNC);
    assert_eq!(applied.changes(), ack_after_sync().changes());
    assert_ne!(applied.repair_id(), draft.repair_id());
    let artifact = applied.artifact_json();
    assert_eq!(
        fields(&artifact).get("hypothesis").and_then(Json::as_str),
        Some(ACK_AFTER_SYNC)
    );
    fixture_matches(APPLIED_FIXTURE, &applied.to_artifact_bytes());
}

// --- negative ----------------------------------------------------------------------

/// Relaxing a property, removing a fault, and hiding an event are each an edit to the
/// Intent Contract, so each arrives as a change of kind `intent`, and each is refused
/// `IntentMutationDenied` wherever it sits in the change list (RFC 0032, INV-011).
#[test]
fn negative_an_intent_weakening_change_is_refused_intent_mutation_denied() {
    let draft = draft();
    for (weakening, patch) in [
        (
            "relax the property",
            "claims.durable_ack: always -> eventually",
        ),
        ("remove a fault", "fault_model.enabled: -crash"),
        ("hide an event", "observers.client.events: -ack"),
    ] {
        for (index, changes) in [
            (0, vec![Change::new(ChangeKind::Intent, digest_of(patch))]),
            (
                1,
                vec![
                    Change::new(ChangeKind::Rust, digest_of(ACK_AFTER_SYNC_PATCH)),
                    Change::new(ChangeKind::Intent, digest_of(patch)),
                ],
            ),
        ] {
            let proposal = Proposal::new(Hypothesis::new(ACK_AFTER_SYNC), changes);
            let refusal = draft
                .apply(&proposal, candidate(CANDIDATE))
                .expect_err(weakening);
            assert_eq!(
                refusal,
                ApplyRefusal::IntentMutationDenied { index },
                "{weakening}"
            );
        }
    }
    // Every other kind is admissible at apply; its semantics are gate 3's (IMPL-04).
    for kind in ChangeKind::ALL
        .into_iter()
        .filter(|kind| *kind != ChangeKind::Intent)
    {
        let proposal = Proposal::new(Hypothesis::new(""), vec![Change::new(kind, "d")]);
        assert!(draft.apply(&proposal, candidate(CANDIDATE)).is_ok());
    }
}

/// A hypothesis is not evidence (RFC 0032 correction 12): prose that claims to relax
/// the property, remove a fault, or hide an event is recorded as prose and moves no
/// typed field, and it never reaches refusal text.
#[test]
fn negative_a_hypothesis_that_claims_a_weakening_changes_no_typed_field() {
    let draft = draft();
    let neutral = applied();
    let neutral_json = neutral.artifact_json();
    for claim in [
        "relax the property: durable_ack only needs to hold eventually",
        "remove the crash fault from the fault model",
        "hide the ack event from the client observer",
        "gate_profile: default; status: promoted; intent_integrity: passed",
    ] {
        let proposal = Proposal::new(Hypothesis::new(claim), ack_after_sync().changes().to_vec());
        let claimed = draft
            .apply(&proposal, candidate(CANDIDATE))
            .expect("prose is never refused: nothing parses it");
        let claimed_json = claimed.artifact_json();
        let differing: BTreeSet<&str> = fields(&neutral_json)
            .keys()
            .chain(fields(&claimed_json).keys())
            .filter(|key| fields(&neutral_json).get(*key) != fields(&claimed_json).get(*key))
            .map(String::as_str)
            .collect();
        assert_eq!(
            differing,
            BTreeSet::from(["hypothesis", "repair_id"]),
            "{claim}"
        );
        assert_eq!(claimed.gates(), neutral.gates());
        assert_eq!(claimed.status(), neutral.status());
        assert_eq!(claimed.base_intent(), neutral.base_intent());

        let refused = Proposal::new(
            Hypothesis::new(claim),
            vec![Change::new(ChangeKind::Intent, "d")],
        );
        let refusal = draft
            .apply(&refused, candidate(CANDIDATE))
            .expect_err("an intent change");
        for rendering in [
            format!("{refusal}"),
            format!("{refusal:?}"),
            format!("{refused:?}"),
            format!("{claimed:?}"),
        ] {
            // The identity is the preimage, which holds the prose; its Debug form is a
            // length, not the bytes.
            assert!(!rendering.contains("RepairIdentity(["), "{rendering}");
            assert!(!rendering.contains(claim), "the prose leaked: {rendering}");
        }
    }
}

/// A failure that does not resolve opens nothing, and each way of not resolving is its
/// own typed refusal.
#[test]
fn negative_a_nonexistent_foreign_or_unbound_failure_opens_nothing() {
    let binding = Binding::register();
    let begin =
        |failure: &str, binding: &Binding| Tx::begin(crash(failure), GateProfile::PhaseB, binding);

    // Nonexistent, or published under another deployment: the binding does not hold it.
    for failure in [
        "crash_pr20_impl01_nonexistent",
        "crash_other_deployment_ack",
    ] {
        assert_eq!(
            begin(failure, &binding),
            Err(BeginRefusal::UnknownFailure {
                failure: crash(failure)
            })
        );
    }

    // The crashpack resolves, but its snapshot binds no intent.
    let mut unbound = Binding::register();
    unbound.intents.clear();
    assert_eq!(
        begin(CRASH, &unbound),
        Err(BeginRefusal::UnboundBaseSnapshot {
            failure: crash(CRASH),
            base_snapshot: SnapshotId::new(BASE).unwrap(),
        })
    );

    // A store that does not answer is not a store that said "absent" (INV-008).
    let mut silent = Binding::register();
    silent
        .failures
        .insert(CRASH.to_owned(), Resolution::Unavailable);
    assert_eq!(
        begin(CRASH, &silent),
        Err(BeginRefusal::BindingUnavailable {
            failure: crash(CRASH)
        })
    );
    let mut silent_intent = Binding::register();
    silent_intent
        .intents
        .insert(BASE.to_owned(), Resolution::Unavailable);
    assert_eq!(
        begin(CRASH, &silent_intent),
        Err(BeginRefusal::BindingUnavailable {
            failure: crash(CRASH)
        })
    );

    // A handle outside the schema's `failure` pattern is not a failure at all.
    for malformed in [
        "crash_",
        "ws_pr20_impl01_register_base",
        "crash_a/b",
        "rt_x",
    ] {
        assert!(CrashpackId::new(malformed).is_err(), "{malformed}");
    }
}

// --- boundary ----------------------------------------------------------------------

/// Serialization round trip (docs/19 §3): the artifact bytes re-parse and re-encode to
/// themselves, and two independent `begin`/`apply` runs produce the same bytes.
#[test]
fn boundary_canonical_bytes_are_stable_and_survive_a_serialization_round_trip() {
    for transaction in [draft(), applied()] {
        let bytes = transaction.to_artifact_bytes();
        let reparsed = Json::parse(&bytes).expect("the artifact is JSON");
        assert_eq!(
            reparsed.to_canonical_bytes(),
            bytes,
            "serialization round trip"
        );
    }
    // Encoding determinism: the same inputs give the same bytes. This is not a claim that
    // two distinct `begin` calls are one transaction; RFC 0032 says they are two, and the
    // schema has no field to tell them apart (the open question in `lib.rs`).
    assert_eq!(draft().to_artifact_bytes(), draft().to_artifact_bytes());
    assert_eq!(applied().to_artifact_bytes(), applied().to_artifact_bytes());

    // Prose with quotes, controls, and non-ASCII has one spelling and round-trips.
    let awkward = "ack \"after\" sync\n\ttab \u{1} é ∀ \\";
    let proposal = Proposal::new(Hypothesis::new(awkward), Vec::new());
    let transaction = draft().apply(&proposal, candidate(CANDIDATE)).unwrap();
    let reparsed = Json::parse(&transaction.to_artifact_bytes()).unwrap();
    assert_eq!(
        fields(&reparsed).get("hypothesis").and_then(Json::as_str),
        Some(awkward)
    );
    assert_eq!(
        reparsed.to_canonical_bytes(),
        transaction.to_artifact_bytes()
    );
}

/// The identity is the preimage (every field but `repair_id`), so changing any one
/// field — including one byte of the hypothesis — changes the identity and the handle,
/// and the handle is not part of what it names.
#[test]
fn boundary_the_identity_moves_with_every_field() {
    let base = Binding::register();
    let reference = applied();
    let apply_with = |hypothesis: &str, changes: Vec<Change>, target: &str| {
        draft()
            .apply(
                &Proposal::new(Hypothesis::new(hypothesis), changes),
                candidate(target),
            )
            .unwrap()
    };
    let rust = || {
        vec![Change::new(
            ChangeKind::Rust,
            digest_of(ACK_AFTER_SYNC_PATCH),
        )]
    };

    let mut other_failure = Binding::register();
    other_failure.failures.insert(
        "crash_pr20_impl01_other".to_owned(),
        Resolution::Found(SnapshotId::new(BASE).unwrap()),
    );
    let mut other_base = Binding::register();
    other_base.failures.insert(
        CRASH.to_owned(),
        Resolution::Found(SnapshotId::new("ws_pr20_impl01_other_base").unwrap()),
    );
    other_base.intents.insert(
        "ws_pr20_impl01_other_base".to_owned(),
        Resolution::Found(register_intent()),
    );
    let mut other_intent = Binding::register();
    other_intent.intents.insert(
        BASE.to_owned(),
        Resolution::Found(IntentId::new("in_pr20_impl01_other_intent").unwrap()),
    );
    let apply_on = |transaction: Tx| {
        transaction
            .apply(&ack_after_sync(), candidate(CANDIDATE))
            .unwrap()
    };
    let begin = |failure: &str, profile: GateProfile, binding: &Binding| {
        Tx::begin(crash(failure), profile, binding).unwrap()
    };

    let mut variants = vec![
        (
            "hypothesis, one byte",
            apply_with(&format!("{ACK_AFTER_SYNC}."), rust(), CANDIDATE),
        ),
        (
            "changes, digest",
            apply_with(
                ACK_AFTER_SYNC,
                vec![Change::new(ChangeKind::Rust, "x")],
                CANDIDATE,
            ),
        ),
        (
            "changes, kind",
            apply_with(
                ACK_AFTER_SYNC,
                vec![Change::new(
                    ChangeKind::Model,
                    digest_of(ACK_AFTER_SYNC_PATCH),
                )],
                CANDIDATE,
            ),
        ),
        (
            "changes, empty",
            apply_with(ACK_AFTER_SYNC, Vec::new(), CANDIDATE),
        ),
        (
            "candidate_snapshot",
            apply_with(ACK_AFTER_SYNC, rust(), "ws_pr20_impl01_other_candidate"),
        ),
        (
            "failure",
            apply_on(begin(
                "crash_pr20_impl01_other",
                GateProfile::PhaseB,
                &other_failure,
            )),
        ),
        (
            "base_snapshot",
            apply_on(begin(CRASH, GateProfile::PhaseB, &other_base)),
        ),
        (
            "base_intent",
            apply_on(begin(CRASH, GateProfile::PhaseB, &other_intent)),
        ),
        ("version and supersedes", apply_on(reference.clone())),
        ("status and candidate (the draft)", draft()),
    ];
    for profile in [
        GateProfile::PhaseC,
        GateProfile::PhaseD,
        GateProfile::Default,
    ] {
        variants.push(("gate_profile", apply_on(begin(CRASH, profile, &base))));
    }

    let mut identities = BTreeSet::from([reference.identity().clone()]);
    let mut handles = BTreeSet::from([reference.repair_id().clone()]);
    for (field, variant) in &variants {
        assert!(
            identities.insert(variant.identity().clone()),
            "{field}: identity did not move"
        );
        assert!(
            handles.insert(variant.repair_id().clone()),
            "{field}: handle did not move"
        );
    }

    // `repair_id` is excluded from its own preimage.
    for transaction in [draft(), reference] {
        let preimage = transaction.preimage_json();
        assert!(fields(&preimage).get("repair_id").is_none());
        assert_eq!(
            transaction.identity().canonical_bytes(),
            preimage.to_canonical_bytes()
        );
        let token = transaction.repair_id().as_str().as_bytes().to_vec();
        assert!(
            !transaction
                .identity()
                .canonical_bytes()
                .windows(token.len())
                .any(|window| window == token.as_slice())
        );
    }
}

/// Differential: the `rt_` handle and the identity bytes equal an independent
/// construction — the preimage written field by field from the schema in this test and
/// hashed through `continuum_value::identity::Blake3Hasher` — against
/// `continuum_repair::transaction::RepairTransaction`'s own.
#[test]
fn differential_the_handle_matches_an_independent_continuum_value_construction() {
    let string = |value: &str| Json::String(value.to_owned());
    let gates: Vec<Json> = [
        ("base_replay", "pending"),
        ("patch_application", "pending"),
        ("intent_integrity", "pending"),
        ("exact_regression", "pending"),
        ("neighborhood", "pending"),
        ("property_mutation", "pending"),
        ("defect_mutants", "pending"),
        ("refinement_coverage", "pending"),
        ("certificate_rebuild", "not_yet_enforced"),
        ("incremental_parity", "not_yet_enforced"),
        ("code_and_security", "pending"),
        ("receipt_generation", "pending"),
    ]
    .into_iter()
    .map(|(name, status)| {
        Json::object([
            ("name".to_owned(), string(name)),
            ("status".to_owned(), string(status)),
            ("evidence".to_owned(), Json::Array(Vec::new())),
        ])
        .unwrap()
    })
    .collect();
    let ledger = Json::object(
        ["cpu_ms", "wall_ms", "solver_ms", "memory_bytes", "tokens"]
            .map(|key| (key.to_owned(), Json::Integer(0))),
    )
    .unwrap();
    let draft_preimage = Json::object([
        ("schema_id".to_owned(), string(SCHEMA_ID)),
        ("schema_epoch".to_owned(), Json::Integer(1)),
        ("version".to_owned(), Json::Integer(1)),
        ("base_snapshot".to_owned(), string(BASE)),
        ("base_intent".to_owned(), string(register_intent().as_str())),
        ("failure".to_owned(), string(CRASH)),
        ("hypothesis".to_owned(), string("")),
        ("changes".to_owned(), Json::Array(Vec::new())),
        ("candidate_snapshot".to_owned(), Json::Null),
        ("status".to_owned(), string("draft")),
        ("gate_profile".to_owned(), string("phase-b")),
        ("gates".to_owned(), Json::Array(gates.clone())),
        ("cost_ledger".to_owned(), ledger.clone()),
    ])
    .unwrap();
    let bytes = draft_preimage.to_canonical_bytes();
    let handle = format!("rt_{}", Blake3Hasher::hash(&bytes).to_token());
    let draft = draft();
    assert_eq!(draft.identity().canonical_bytes(), bytes.as_slice());
    assert_eq!(draft.repair_id().as_str(), handle);

    let applied_preimage = Json::object([
        ("schema_id".to_owned(), string(SCHEMA_ID)),
        ("schema_epoch".to_owned(), Json::Integer(1)),
        ("version".to_owned(), Json::Integer(2)),
        ("supersedes".to_owned(), string(&handle)),
        ("base_snapshot".to_owned(), string(BASE)),
        ("base_intent".to_owned(), string(register_intent().as_str())),
        ("failure".to_owned(), string(CRASH)),
        ("hypothesis".to_owned(), string(ACK_AFTER_SYNC)),
        (
            "changes".to_owned(),
            Json::Array(vec![
                Json::object([
                    ("kind".to_owned(), string("rust")),
                    (
                        "digest".to_owned(),
                        string(&digest_of(ACK_AFTER_SYNC_PATCH)),
                    ),
                ])
                .unwrap(),
            ]),
        ),
        ("candidate_snapshot".to_owned(), string(CANDIDATE)),
        ("status".to_owned(), string("applied")),
        ("gate_profile".to_owned(), string("phase-b")),
        ("gates".to_owned(), Json::Array(gates)),
        ("cost_ledger".to_owned(), ledger),
    ])
    .unwrap();
    let applied = applied();
    assert_eq!(
        applied.repair_id().as_str(),
        format!(
            "rt_{}",
            Blake3Hasher::hash(&applied_preimage.to_canonical_bytes()).to_token()
        )
    );
}

/// The crate's closed sets are the schema's, read from the schema file itself, and so
/// are the per-profile `not_yet_enforced` sets and the emitted field names.
#[test]
fn boundary_the_closed_sets_and_fields_are_the_schemas() {
    let schema = parsed(SCHEMA);
    assert_eq!(
        tokens(at(&schema, &["$defs", "gate_name", "enum"])),
        GateName::ALL.map(|gate| gate.token().to_owned())
    );
    let gate_item = at(&schema, &["properties", "gates", "items", "properties"]);
    assert_eq!(
        tokens(at(gate_item, &["status", "enum"])),
        GateStatus::ALL.map(|status| status.token().to_owned())
    );
    assert_eq!(
        tokens(at(&schema, &["properties", "gate_profile", "enum"])),
        GateProfile::ALL.map(|profile| profile.token().to_owned())
    );
    assert_eq!(
        tokens(at(&schema, &["properties", "status", "enum"])),
        TransactionStatus::ALL.map(|status| status.token().to_owned())
    );
    assert_eq!(
        tokens(at(
            &schema,
            &[
                "properties",
                "changes",
                "items",
                "properties",
                "kind",
                "enum"
            ]
        )),
        ChangeKind::ALL.map(|kind| kind.token().to_owned())
    );
    assert_eq!(
        at(&schema, &["properties", "schema_id", "const"]).as_str(),
        Some(SCHEMA_ID)
    );
    assert_eq!(
        at(&schema, &["properties", "schema_epoch", "const"]).as_integer(),
        Some(SCHEMA_EPOCH)
    );

    // Per-profile: the gates the schema's conditionals let be `not_yet_enforced`.
    for conditional in at(&schema, &["allOf"]).as_array().unwrap() {
        let Some(profile) = fields(at(conditional, &["if", "properties"]))
            .get("gate_profile")
            .and_then(|property| fields(property).get("const"))
            .and_then(Json::as_str)
        else {
            continue;
        };
        let contains = at(
            conditional,
            &[
                "then",
                "properties",
                "gates",
                "not",
                "contains",
                "properties",
            ],
        );
        let allowed: BTreeSet<String> = fields(contains)
            .get("name")
            .map(|name| tokens(at(name, &["not", "enum"])).into_iter().collect())
            .unwrap_or_default();
        let profile = GateProfile::ALL
            .into_iter()
            .find(|candidate| candidate.token() == profile)
            .expect("a schema profile is a crate profile");
        let unenforced: BTreeSet<String> = GateName::ALL
            .into_iter()
            .filter(|gate| !profile.enforces(*gate))
            .map(|gate| gate.token().to_owned())
            .collect();
        assert_eq!(unenforced, allowed, "{}", profile.token());
    }

    // Every emitted field is declared, and every required field is emitted.
    let declared: BTreeSet<&str> = fields(at(&schema, &["properties"]))
        .keys()
        .map(String::as_str)
        .collect();
    let required: BTreeSet<String> = tokens(at(&schema, &["required"])).into_iter().collect();
    for transaction in [draft(), applied()] {
        let artifact = transaction.artifact_json();
        let emitted: BTreeSet<&str> = fields(&artifact).keys().map(String::as_str).collect();
        assert!(emitted.is_subset(&declared), "{emitted:?}");
        assert!(
            required.iter().all(|key| emitted.contains(key.as_str())),
            "{emitted:?}"
        );
    }
}

/// Under every profile, the gate list `begin` and `apply` emit is the profile's: every
/// gate present in gate-number order, `pending` inside the profile and
/// `not_yet_enforced` outside it, never `not_yet_enforced` inside it. The phase-b
/// fixture alone cannot tell a profile-blind gate list apart.
#[test]
fn boundary_every_profile_emits_its_own_gate_list() {
    let expected_unenforced = |profile: GateProfile| -> Vec<&'static str> {
        match profile {
            GateProfile::PhaseB => vec!["certificate_rebuild", "incremental_parity"],
            GateProfile::PhaseC => vec!["certificate_rebuild"],
            GateProfile::PhaseD | GateProfile::Default => Vec::new(),
        }
    };
    for profile in GateProfile::ALL {
        let draft = Tx::begin(crash(CRASH), profile, &Binding::register()).unwrap();
        let applied = draft
            .apply(&ack_after_sync(), candidate(CANDIDATE))
            .unwrap();
        for transaction in [&draft, &applied] {
            assert_eq!(transaction.gate_profile(), profile);
            let names: Vec<GateName> = transaction.gates().iter().map(|gate| gate.name()).collect();
            assert_eq!(names, GateName::ALL);
            let unenforced: Vec<&str> = transaction
                .gates()
                .iter()
                .filter(|gate| gate.status() == GateStatus::NotYetEnforced)
                .map(|gate| gate.name().token())
                .collect();
            assert_eq!(
                unenforced,
                expected_unenforced(profile),
                "{}",
                profile.token()
            );
            assert!(transaction.gates().iter().all(|gate| matches!(
                gate.status(),
                GateStatus::Pending | GateStatus::NotYetEnforced
            )));
        }
    }
}
