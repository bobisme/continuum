//! docs/09 T09 ("Agent weakens property") — the immutable baseline property digest
//! (`bn-20co`).
//!
//! > Controls: semantic property diff; review gates; **immutable baseline property
//! > digest in CI**; mutation score; agent cannot modify claim ledger or certificate.
//! >
//! > — `notes/plan/docs/09_THREAT_MODEL.md`, T09
//!
//! # What this file enforces
//!
//! `tools/governance/t09-property-baseline.json` is a committed pin of every Intent
//! Contract document in the tree. For each one it records three BLAKE3 digests, each
//! computed here by the real `continuum-intent` decoder and identity code, never by a
//! Python port:
//!
//! - `intent_identity` — the digest of [`IntentIdentity`]'s canonical bytes, the
//!   RFC 0037 ID1/ID2 preimage. Every protected field group is in it, and the
//!   `policy` table is in it too, so a governance edit moves it (INV-001's identity
//!   evidence). ID2 metadata (`name`, `intent_id`, display `source`) is not.
//! - `claims` — the digest of the `claims` group's identity preimage alone: the
//!   property set, so a moved pin names *which* group moved.
//! - `policy` — the digest of the `policy` table's identity preimage alone: the
//!   cheapest path to a weakening is to unlock `properties` first and weaken second,
//!   and this pin makes that first step visible on its own.
//!
//! `just check` runs this file (`just test`) and `tools/governance/check_t09_evidence.py`
//! re-runs it by exact name. A contract whose protected content moves without the pin
//! moving fails here. A pin that moves must carry a fresh, named `revision` (a Bone and
//! a reason), which the Python checker's delta rule enforces against the merge base.
//! The digests make a property change a visible, reviewable diff of a protected file.
//! They do not authenticate the reviewer: that is the review gate's job, and the T09
//! evidence file says so.
//!
//! # Fail-closed shape
//!
//! Every failure mode here is a test failure, never a pass: a contract that no longer
//! decodes, a baseline that no longer parses, a pinned path that disappears, a contract
//! the baseline does not pin, and a digest that disagrees. There is no "skip".
//!
//! # Tests
//!
//! - positive: [`the_baseline_pins_exactly_the_committed_intent_contracts`],
//!   [`every_pinned_contract_still_matches_its_baseline_digests`],
//!   [`the_baseline_names_the_production_hasher`];
//! - hostile fixtures: [`hostile_an_agent_authored_property_weakening_moves_the_claims_pin`]
//!   (the plan §5.1 complementary-disjunct weakening PR-12 classifies `weakened`),
//!   [`hostile_unlocking_the_properties_verb_moves_the_policy_pin`];
//! - boundary: [`boundary_metadata_and_display_source_do_not_move_any_pin`];
//! - anti-vacuity: [`negative_a_flipped_digest_and_a_dropped_entry_are_both_detected`].

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use continuum_intent::canonical_json::Json;
use continuum_intent::contract::IntentContract;
use continuum_value::identity::{Blake3Hasher, ContentHasher};

/// The committed baseline, relative to the repository root.
const BASELINE: &str = "tools/governance/t09-property-baseline.json";

/// The schema header every Intent Contract document carries (W10).
const CONTRACT_SCHEMA_ID: &str = "https://continuum.dev/schema/intent-contract.json";

/// The replicated-register fixture's `v1 == v2` disjunct, and its complement. The same
/// bytes `continuum-semantic-diff`'s
/// `pr12_impl02_property_ast_edit_evidence.rs` uses for the attack it classifies
/// `weakened` and blocks under the fixture's own `locked` verb.
const AGREEMENT_EQ_DISJUNCT: &str = r#"{"kind":"compare","left":{"kind":"var","name":"v1"},"op":"eq","right":{"kind":"var","name":"v2"}}"#;
const AGREEMENT_NE_DISJUNCT: &str = r#"{"kind":"compare","left":{"kind":"var","name":"v1"},"op":"ne","right":{"kind":"var","name":"v2"}}"#;

const REPLICATED_REGISTER: &str =
    "crates/continuum-intent/tests/fixtures/replicated-register-contract.json";
const DIE_HARD: &str = "crates/continuum-intent/tests/fixtures/die-hard-contract.json";

/// Directory names the contract walk never enters: build output and tool state, not
/// source. Hidden directories (`.git`, `.maw`, `.bones`, `.lake`, …) are skipped by
/// the leading-dot rule in [`walk`].
const SKIP_DIRS: [&str; 2] = ["target", "node_modules"];

#[derive(Debug, Clone, PartialEq, Eq)]
struct Pin {
    path: String,
    intent_identity: String,
    claims: String,
    policy: String,
}

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("the repository root resolves")
}

fn read(root: &Path, rel: &str) -> String {
    fs::read_to_string(root.join(rel)).unwrap_or_else(|e| panic!("{rel} is readable: {e}"))
}

fn digest(bytes: &[u8]) -> String {
    Blake3Hasher::hash(bytes).to_token()
}

/// The three pins of one contract document, computed by the real decoder.
fn pins_of(path: &str, text: &str) -> Result<Pin, String> {
    let contract = IntentContract::decode(text.as_bytes())
        .map_err(|e| format!("{path}: the contract no longer decodes: {e:?}"))?;
    Ok(Pin {
        path: path.to_owned(),
        intent_identity: contract.identity().digest::<Blake3Hasher>().to_token(),
        claims: digest(
            &contract
                .claims()
                .identity_preimage_json()
                .to_canonical_bytes(),
        ),
        policy: digest(
            &contract
                .policy()
                .identity_preimage_json()
                .to_canonical_bytes(),
        ),
    })
}

fn field<'a>(object: &'a Json, key: &str, context: &str) -> Result<&'a Json, String> {
    object
        .as_object()
        .ok_or_else(|| format!("{context}: not an object"))?
        .get(key)
        .ok_or_else(|| format!("{context}: missing {key:?}"))
}

fn string_field(object: &Json, key: &str, context: &str) -> Result<String, String> {
    field(object, key, context)?
        .as_str()
        .map(str::to_owned)
        .ok_or_else(|| format!("{context}: {key:?} is not a string"))
}

/// The baseline's `hasher` token and its pins.
fn parse_baseline(text: &str) -> Result<(String, Vec<Pin>), String> {
    let doc = Json::parse(text.as_bytes()).map_err(|e| format!("{BASELINE}: {e:?}"))?;
    let hasher = string_field(&doc, "hasher", BASELINE)?;
    let entries = field(&doc, "entries", BASELINE)?
        .as_array()
        .ok_or_else(|| format!("{BASELINE}: entries is not an array"))?;
    let mut pins = Vec::with_capacity(entries.len());
    for (index, entry) in entries.iter().enumerate() {
        let context = format!("{BASELINE} entries[{index}]");
        pins.push(Pin {
            path: string_field(entry, "path", &context)?,
            intent_identity: string_field(entry, "intent_identity", &context)?,
            claims: string_field(entry, "claims", &context)?,
            policy: string_field(entry, "policy", &context)?,
        });
    }
    Ok((hasher, pins))
}

fn walk(dir: &Path, root: &Path, out: &mut Vec<String>) {
    let mut entries: Vec<_> = fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("{} is listable: {e}", dir.display()))
        .map(|e| e.expect("a directory entry is readable").path())
        .collect();
    entries.sort();
    for path in entries {
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default()
            .to_owned();
        if name.starts_with('.') {
            continue;
        }
        if path.is_dir() {
            if !SKIP_DIRS.contains(&name.as_str()) {
                walk(&path, root, out);
            }
        } else if name.ends_with(".json") {
            let rel = path
                .strip_prefix(root)
                .expect("the walk stays under the root")
                .to_string_lossy()
                .replace('\\', "/");
            out.push(rel);
        }
    }
}

/// Every JSON document in the tree whose top-level `schema_id` is the Intent Contract
/// schema. The schema file itself names the id as its `$id`, not as `schema_id`, and so
/// is not a contract.
fn committed_contracts(root: &Path) -> BTreeSet<String> {
    let mut json_files = Vec::new();
    walk(root, root, &mut json_files);
    json_files
        .into_iter()
        .filter(|rel| {
            let Ok(text) = fs::read_to_string(root.join(rel)) else {
                return false;
            };
            if !text.contains(CONTRACT_SCHEMA_ID) {
                return false;
            }
            Json::parse(text.as_bytes()).is_ok_and(|doc| {
                doc.as_object()
                    .and_then(|o| o.get("schema_id"))
                    .and_then(Json::as_str)
                    == Some(CONTRACT_SCHEMA_ID)
            })
        })
        .collect()
}

/// Compare the pinned set with the committed set. Every finding is a failure.
fn coverage_findings(pinned: &[Pin], committed: &BTreeSet<String>) -> Vec<String> {
    let mut findings = Vec::new();
    let mut seen = BTreeSet::new();
    for pin in pinned {
        if !seen.insert(pin.path.clone()) {
            findings.push(format!("{} is pinned twice", pin.path));
        }
    }
    for path in committed.difference(&seen) {
        findings.push(format!(
            "{path} is an Intent Contract the baseline does not pin: add an entry with a revision"
        ));
    }
    for path in seen.difference(committed) {
        findings.push(format!(
            "{path} is pinned but is no longer a committed Intent Contract: retire it with a revision"
        ));
    }
    findings
}

/// Compare each pin with the digests the real decoder computes now.
fn digest_findings(pinned: &[Pin], actual: &[Pin]) -> Vec<String> {
    let mut findings = Vec::new();
    for pin in pinned {
        let Some(now) = actual.iter().find(|a| a.path == pin.path) else {
            findings.push(format!("{}: no computed digests to compare", pin.path));
            continue;
        };
        for (group, want, got) in [
            (
                "intent_identity",
                &pin.intent_identity,
                &now.intent_identity,
            ),
            ("claims", &pin.claims, &now.claims),
            ("policy", &pin.policy, &now.policy),
        ] {
            if want != got {
                findings.push(format!(
                    "{}: the {group} pin moved (baseline {want}, now {got}); a protected \
                     change needs a new pin with a named revision in {BASELINE}",
                    pin.path
                ));
            }
        }
    }
    findings
}

fn baseline() -> (String, Vec<Pin>) {
    parse_baseline(&read(&repo_root(), BASELINE)).unwrap_or_else(|e| panic!("{e}"))
}

fn pinned(path: &str) -> Pin {
    baseline()
        .1
        .into_iter()
        .find(|p| p.path == path)
        .unwrap_or_else(|| panic!("{path} is pinned"))
}

// --- positive -----------------------------------------------------------------------------

#[test]
fn the_baseline_pins_exactly_the_committed_intent_contracts() {
    let root = repo_root();
    let committed = committed_contracts(&root);
    // Non-vacuity: the walk finds the three contracts this tree is known to carry.
    for known in [
        DIE_HARD,
        REPLICATED_REGISTER,
        "notes/plan/schemas/examples/intent-contract.example.json",
    ] {
        assert!(
            committed.contains(known),
            "the contract walk must find {known}"
        );
    }
    assert!(
        !committed.contains("notes/plan/schemas/intent-contract.schema.json"),
        "the schema names the id as $id and is not a contract"
    );
    let findings = coverage_findings(&baseline().1, &committed);
    assert!(findings.is_empty(), "{findings:#?}");
}

#[test]
fn every_pinned_contract_still_matches_its_baseline_digests() {
    let root = repo_root();
    let (_, pins) = baseline();
    assert!(!pins.is_empty(), "an empty baseline pins nothing");
    let mut actual = Vec::new();
    let mut findings = Vec::new();
    for pin in &pins {
        match pins_of(&pin.path, &read(&root, &pin.path)) {
            Ok(now) => actual.push(now),
            Err(e) => findings.push(e),
        }
    }
    findings.extend(digest_findings(&pins, &actual));
    assert!(findings.is_empty(), "{findings:#?}");
}

#[test]
fn the_baseline_names_the_production_hasher() {
    let (hasher, _) = baseline();
    assert!(Blake3Hasher::ALGORITHM.is_cryptographic());
    assert_eq!(hasher, Blake3Hasher::ALGORITHM.to_string());
}

// --- hostile fixtures -----------------------------------------------------------------------

#[test]
fn hostile_an_agent_authored_property_weakening_moves_the_claims_pin() {
    // Plan §5.1's `properties` gaming move: append `v1 != v2` to `Agreement`'s
    // disjunction, which exempts every remaining state. The document still decodes —
    // the attack arrives through the front door — and PR-12 classifies it `weakened`
    // and blocks it under the fixture's `locked` verb. Here it must also move the pin,
    // so the change cannot land without a visible, named revision of the baseline.
    let root = repo_root();
    let original = read(&root, REPLICATED_REGISTER);
    let weakened = original.replacen(
        AGREEMENT_EQ_DISJUNCT,
        &format!("{AGREEMENT_EQ_DISJUNCT},{AGREEMENT_NE_DISJUNCT}"),
        1,
    );
    assert_ne!(weakened, original, "the weakening must actually fire");

    let pin = pinned(REPLICATED_REGISTER);
    let now = pins_of(REPLICATED_REGISTER, &weakened).expect("the weakened contract decodes");
    assert_ne!(
        now.claims, pin.claims,
        "a weakened claim must move the claims pin"
    );
    assert_ne!(now.intent_identity, pin.intent_identity);
    assert_eq!(now.policy, pin.policy, "only the property set moved");
    let findings = digest_findings(&[pin], &[now]);
    assert_eq!(findings.len(), 2, "{findings:#?}");
    assert!(findings.iter().any(|f| f.contains("the claims pin moved")));
}

#[test]
fn hostile_unlocking_the_properties_verb_moves_the_policy_pin() {
    // The first step of a two-step weakening: relax the governance before touching the
    // property. It changes no claim, and it must still move a pin.
    let root = repo_root();
    let original = read(&root, DIE_HARD);
    let unlocked = original.replacen(r#""properties":"locked""#, r#""properties":"unlocked""#, 1);
    assert_ne!(unlocked, original, "the unlock must actually fire");

    let pin = pinned(DIE_HARD);
    let now = pins_of(DIE_HARD, &unlocked).expect("the unlocked contract decodes");
    assert_eq!(now.claims, pin.claims, "no claim moved");
    assert_ne!(
        now.policy, pin.policy,
        "an unlocked verb must move the policy pin"
    );
    assert_ne!(now.intent_identity, pin.intent_identity);
}

// --- boundary -------------------------------------------------------------------------------

#[test]
fn boundary_metadata_and_display_source_do_not_move_any_pin() {
    // The pin is semantic, not a byte hash: ID2 metadata and a claim's display-only
    // `source` are outside the identity preimage, so rewriting them is not a
    // protected change and must not demand a revision.
    let root = repo_root();
    let original = read(&root, DIE_HARD);
    let cosmetic = original
        .replacen(
            r#""name":"Die Hard — TV-009""#,
            r#""name":"Die Hard (renamed)""#,
            1,
        )
        .replacen(r#""source":"big != 4""#, r#""source":"big is never 4""#, 1);
    assert_ne!(cosmetic, original, "both rewrites must actually fire");
    assert_eq!(
        pins_of(DIE_HARD, &cosmetic).expect("decodes"),
        pinned(DIE_HARD),
        "metadata and display source are outside every pin"
    );
}

// --- anti-vacuity ---------------------------------------------------------------------------

#[test]
fn negative_a_flipped_digest_and_a_dropped_entry_are_both_detected() {
    let root = repo_root();
    let (_, pins) = baseline();
    let actual: Vec<Pin> = pins
        .iter()
        .map(|p| pins_of(&p.path, &read(&root, &p.path)).expect("decodes"))
        .collect();

    // A baseline whose claims digest for one entry is flipped in its last hex digit.
    let mut flipped = pins.clone();
    let last = flipped[0].claims.pop().expect("a digest is non-empty");
    flipped[0].claims.push(if last == '0' { '1' } else { '0' });
    assert_eq!(digest_findings(&flipped, &actual).len(), 1);

    // A baseline that drops one entry leaves that contract unpinned.
    let committed = committed_contracts(&root);
    let dropped = &pins[1..];
    let findings = coverage_findings(dropped, &committed);
    assert_eq!(findings.len(), 1, "{findings:#?}");
    assert!(findings[0].contains("does not pin"));

    // A baseline that pins a path twice is refused.
    let mut doubled = pins.clone();
    doubled.push(pins[0].clone());
    assert!(
        coverage_findings(&doubled, &committed)
            .iter()
            .any(|f| f.contains("pinned twice"))
    );
}
